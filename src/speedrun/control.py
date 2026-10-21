"""Client for the bridge's control socket (contract C9, ``speedrun_control.cpp``).

The controller listens on a Unix socket and starts one engine with
``SPEEDRUN_CONTROL`` pointing at it. Every engine process, the root and each
branch made with ``fork``, connects on its own and is driven through a
``Branch``: run to a frame or condition, schedule raw input (C8 events),
fork, read state. A branch is an exact copy of the game at the frame it was
forked, RNG included, so trying inputs on branches never drifts from a
straight replay.
"""

import json
import os
import socket
import subprocess
import tempfile
from dataclasses import replace
from pathlib import Path
from typing import Self

from speedrun import releases
from speedrun.engine import EngineConfig, build_argv, build_env, write_ini


class ControlError(Exception):
    pass


class Branch:
    """One engine process, stopped at a decision point between commands."""

    def __init__(self, conn: socket.socket, hello: dict, timeout_s: float):
        self._conn = conn
        self._file = conn.makefile("rb")  # buffered: replies (state dumps) are tens of KB
        self.hello = hello
        self.pid = hello.get("hello")
        self.status: dict = {}
        self.ended: dict | None = None
        self._timeout_s = timeout_s

    def _send(self, msg: dict) -> None:
        self._conn.sendall((json.dumps(msg) + "\n").encode())

    def _recv(self) -> dict:
        line = self._file.readline()
        if not line:
            raise ControlError(f"engine {self.pid} closed the connection")
        msg = json.loads(line)
        if "error" in msg:
            raise ControlError(f"engine {self.pid}: {msg['error']}")
        return msg

    def _await_stop(self) -> dict:
        msg = self._recv()
        if "end" in msg:
            self.ended = msg["end"]
            self.status = {}
            return msg
        self.status = msg
        return msg

    def wait_first_stop(self) -> dict:
        return self._await_stop()

    @property
    def frame(self) -> int:
        return self.status["frame"]

    def inputs(self, events: list[dict]) -> None:
        if events:
            self._send({"cmd": "inputs", "events": events})
            self._recv()

    def run(self, *, frame: int | None = None, until: list | None = None, control: bool = False,
            max_frame: int | None = None, idle: bool = False, until_any: list | None = None,
            menu: bool = False) -> dict:
        """Run until the first stop that holds; returns the stop status (or ``{"end": ...}``).
        ``idle``: the bridge's idle rule; ``menu``: a dialogue menu takes a click; ``until_any``: a list of condition groups, the stop
        reason names the first that holds (``until_any:<i>``)."""
        cmd: dict = {"cmd": "run"}
        if idle:
            cmd["idle"] = True
        if menu:
            cmd["menu"] = True
        if until_any:
            cmd["until_any"] = until_any
        if frame is not None:
            cmd["frame"] = frame
        if until:
            cmd["until"] = until
        if control:
            cmd["control"] = True
        if max_frame is not None:
            cmd["max_frame"] = max_frame
        self._send(cmd)
        return self._await_stop()

    def at(self, x: int, y: int) -> dict:
        """What a click at screen (x, y) would hit now: zone, object (main screen) and verb slot."""
        self._send({"cmd": "at", "x": x, "y": y})
        return self._recv()["at"]

    def state(self) -> dict:
        self._send({"cmd": "state"})
        return self._recv()["state"]

    def set_rng(self, state: int) -> None:
        """Search only: put the RNG in ``state`` (the trace records it, so the run is not valid)."""
        self._send({"cmd": "set_rng", "state": state})
        self._recv()

    def holds(self, until: list) -> bool:
        """Whether the C3 conditions hold now; no frame runs."""
        self._send({"cmd": "holds", "until": until})
        return self._recv()["holds"]

    def quit(self) -> None:
        lag = getattr(self, "lag", None)  # the executor's lag snapshot of this branch (Executor._set_lag)
        if lag is not None:
            self.lag = None
            lag.quit()
        try:
            self._send({"cmd": "quit"})
        except OSError:
            pass
        self.close()

    def close(self) -> None:
        for closer in (self._file.close, self._conn.close):
            try:
                closer()
            except OSError:
                pass


class Controller:
    """Listens on a Unix socket, launches the root engine, and accepts branches."""

    def __init__(self, cfg: EngineConfig, timeout_s: float = 120.0):
        self.cfg = cfg
        self.release = releases.get(cfg.release)
        self._dir = tempfile.mkdtemp(prefix="speedrun-ctl-")
        self.path = os.path.join(self._dir, "control.sock")
        self._server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self._server.bind(self.path)
        self._server.listen(64)
        self._server.settimeout(timeout_s)
        self._timeout_s = timeout_s
        self.proc: subprocess.Popen | None = None
        self.root: Branch | None = None

    def _accept(self) -> Branch:
        conn, _ = self._server.accept()
        conn.settimeout(self._timeout_s)
        branch = Branch(conn, {}, self._timeout_s)
        branch.hello = json.loads(branch._file.readline())
        branch.pid = branch.hello["hello"]
        return branch

    def start(self) -> Branch:
        cfg = self.cfg
        cfg.out_dir.mkdir(parents=True, exist_ok=True)
        write_ini(cfg)
        env = build_env(cfg)
        env["SPEEDRUN_CONTROL"] = self.path
        log = (cfg.out_dir / "stdout.log").open("wb")
        self.proc = subprocess.Popen(build_argv(cfg), stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, env=env)
        self.root = self._accept()
        self.root.wait_first_stop()
        return self.root

    def fork(self, branch: Branch) -> Branch:
        """A new branch at ``branch``'s current stop; ``branch`` stays stopped there."""
        branch._send({"cmd": "fork"})
        reply = branch._recv()
        if "forked" not in reply:
            raise ControlError(f"fork: unexpected reply {reply}")
        child = self._accept()
        child.wait_first_stop()  # its "forked" status
        return child

    def close(self) -> None:
        if self.root is not None:
            self.root.quit()
        if self.proc is not None:
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        self._server.close()
        Path(self.path).unlink(missing_ok=True)
        os.rmdir(self._dir)

    def __enter__(self) -> Self:
        self.start()
        return self

    def __exit__(self, *exc) -> None:
        self.close()


def controlled(cfg: EngineConfig, **overrides) -> Controller:
    """A Controller for ``cfg`` with overrides (e.g. ``seed``), not yet started."""
    return Controller(replace(cfg, **overrides))
