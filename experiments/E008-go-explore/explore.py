"""E008: the explorer (plan.md "Entries", "Cells", "Selection", "Verify", "Parallelism and the machine",
"Interfaces", "Stop rule"): the archive of engine states, its cells and their order, the job loop with b workers, the
readings cache, the verify from boot and the report.

usage (from this folder, with ../../.venv/bin/python):
  explore.py init ARCHIVE --stage NAME --goal K=V[,K=V] | part1 --guide GUIDE.json --allowed k2 | files | R,R,...
  explore.py run ARCHIVE --workers B [--step-cmd CMD] [--no-verify]
  explore.py verify ARCHIVE [--entry ID]
  explore.py report ARCHIVE

It imports neither guide.py nor step.py: it reads GUIDE.json by its format and runs the step as a process
(``python -u step.py run JOB_DIR``; the command before ``run JOB_DIR`` is env E008_STEP or ``--step-cmd``).

**The archive** (plan.md "Interfaces"), ARCHIVE/: ``stage.json``; ``entries.jsonl`` (light rows) and
``entries/ID.json.gz`` (events and checkpoints); ``cells.json``; ``readings.jsonl``; ``jobs/JOB/`` (``job.json``,
``result.json``, ``new/``, ``run/``, ``log``, and the driver's ``select.json`` and ``pid.json``); ``log.jsonl``;
``summary.json``. The explorer is its only writer. A row also keeps ``seg_held``, the R atoms held at some checkpoint
of its segment, so a chain's #r needs no entry file.

* **Init** boots the root as the harness does (``replay_part1.boot(1, ARCHIVE/root, dump_locals=True)``) and keeps
  its checkpoint (frame, ``facts_of``, ``replay.extra_of``) as entry 0; stage.json also keeps the root's frame, tick
  and qf. ``--allowed k2`` is ``size_scaling.scopes(game, the root's room)[2]`` (computed), ``files`` the guide's
  ``rooms.reached``.
* **Held atoms.** An entry holds (k, v) when v is its value of k (absent is 0) and [k, v] is in R, or [k, null] is in R
  and v differs from the root's value of k. Atoms are counted as these (k, v) pairs: a value of a key with (k, any) in
  R is an atom of its own (plan.md "Targets", tier 1). #r is the number of pairs held at some checkpoint of the
  entry's chain.
* **Cells.** The key is [room, the objects O with owner:O equal to the dump's ego, [[k, value] for the guide's keys of
  kinds var, bit, owner, state, class, sorted]]; the cell is the sha1 of its canonical JSON. The representative is the
  entry with the least (tick, id). An entry is favoured when it is the least-(tick, id) holder of some pair.
* **Selection.** Over the representatives that are not busy, whose cell has no running job, that are not exhausted
  and whose room is allowed: the least (#g, not favoured, n, -#r, tick, id) first. n of the job's cell is set to 0
  when the job's entries add a new cell or a new representative of any cell, else it goes up by 1. ``tried`` and
  ``exhausted`` are kept per representative id.
* **Jobs, in start order.** The driver keeps up to b jobs running and commits their results in the order it started
  them: a job that ends early waits for the ones before it. So the archive after a run depends on the results only,
  not on which job ended first (ids are given in ingest order), and a run killed mid-way resumes to the same archive.
  A job is ``guard.sh JOB_DIR/log python -u step.py run JOB_DIR``, under ``flock /var/tmp/fast-mi/heavy.lock`` when
  its scope has 5 rooms or more; a second concurrent job starts only when MemAvailable is at least E008_MEM_SECOND_GB
  (7). Readings are evicted least recently used first while the free space of E008_DISK_PATH (/home) is under
  E008_EVICT_GB (10); no job starts under E008_FLOOR_GB (8). These are the machine's watchdogs (plan.md).
* **Resume.** A job dir without a committed log line is the driver's unfinished work: if its process still runs it is
  waited for, if it left a result.json that result is committed, else it is run again in place. A job the guard
  killed is not counted as tried and is run again; a second kill of the same job stops the run (the plan's rule for a
  job that fails twice on the same entry with the same error). An ingest cut short is rolled back on open.
* **Stops** (plan.md "Stop rule"): the first goal entry (stop starting, wait for the running jobs, then verify and
  write summary.json); an exhausted archive (nothing selectable and nothing running); stagnation (after the last job
  that added a pair no earlier entry held, as many committed jobs as there were selectable cells right after it, none
  adding one); the same entry failing twice with the same error; a return that differs (stops at once, the running
  jobs are killed and are run again on a later resume).
* **Verify** plays the goal entry's movie from boot in a fresh engine (``replay.verify`` with
  ``replay_part1.config(1, ...)``, every checkpoint of the chain), then boots the movie to the goal frame
  (``replay.boot``) to read the run clock: frames, qf and seconds (qf / the release's timer_hz, as cmd_run) from the
  root. ARCHIVE/verify-ID.json.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
import re
import resource
import shlex
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
E003 = HERE.parent / "E003-path-through-cloud"
SRC = HERE.parents[1] / "src"
SEED = 1
GUIDE_FORMAT = "e008-guide-1"
KEY_KINDS = ("var:", "bit:", "owner:", "state:", "class:")  # the guide's keys a cell's key holds (plan.md "Cells")
HEAVY_ROOMS = 5  # a scope of this many rooms or more runs under the heavy lock (plan.md "Parallelism")
GIB = 1 << 30
COMMITTED = ("ok", "error", "return differs")  # log.jsonl outcomes that commit a job; "guard-killed" does not
THREADS, WINDOW_WORKERS = 3, 3  # job.json "threads" (the reader) and "workers" (the window search): plan.md step 3, 5


def say(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr, flush=True)


# --- files ------------------------------------------------------------------------------------------------------------

def canonical(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))


def read_json(path: Path):
    return json.loads(Path(path).read_text())


def write_json(path: Path, obj, indent: int | None = 1) -> None:
    """Atomic: a temporary file renamed over ``path``."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps(obj, indent=indent, default=str) + "\n")
    os.replace(tmp, path)


def read_gz(path: Path):
    with gzip.open(path, "rt") as f:
        return json.load(f)


def write_gz(path: Path, obj) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    with gzip.open(tmp, "wt") as f:
        json.dump(obj, f, separators=(",", ":"))
    os.replace(tmp, path)


def read_jsonl(path: Path) -> tuple:
    """(rows, clean): the parsed lines, and False when a last line was cut short (a write the driver did not
    finish), which is left out."""
    path = Path(path)
    if not path.exists():
        return [], True
    rows, lines = [], path.read_text().splitlines()
    for i, line in enumerate(lines):
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            if i == len(lines) - 1:
                return rows, False
            raise
    return rows, True


def append_jsonl(path: Path, rows: list) -> None:
    with open(path, "a") as f:
        f.write("".join(json.dumps(r, separators=(",", ":"), default=str) + "\n" for r in rows))
        f.flush()
        os.fsync(f.fileno())


def rewrite_jsonl(path: Path, rows: list) -> None:
    tmp = Path(path).with_name(Path(path).name + ".tmp")
    tmp.write_text("".join(json.dumps(r, separators=(",", ":"), default=str) + "\n" for r in rows))
    os.replace(tmp, path)


def dir_bytes(d: Path) -> int:
    total = 0
    for p, _, files in os.walk(d):
        for f in files:
            try:
                total += os.lstat(os.path.join(p, f)).st_size
            except OSError:
                pass
    return total


# --- the guide (GUIDE.json, plan.md "Interfaces") -----------------------------------------------------------------------

def load_guide(path: Path) -> dict:
    g = read_json(path)
    if g.get("format") != GUIDE_FORMAT:
        raise SystemExit(f"{path}: format {g.get('format')!r}, not {GUIDE_FORMAT}")
    return g


def guide_atoms(guide: dict) -> dict:
    """{key: {value or None (any): depth}} from the guide's ``atoms`` ([key, value or null, depth])."""
    out: dict = {}
    for k, v, d in guide["atoms"]:
        cur = out.setdefault(k, {})
        if v not in cur or d < cur[v]:
            cur[v] = d
    return out


def depth_of(atoms: dict, k: str, v) -> int | None:
    """The guide's rule: the least of the depths of [k, v] and [k, null]; None when neither is in R."""
    vals = atoms.get(k) or {}
    ds = [vals[x] for x in (v, None) if x in vals]
    return min(ds) if ds else None


def ring(edges: dict, room: int, allowed) -> list:
    """The guide's rule: sorted({R} | edges[R]) cut to the allowed rooms."""
    return sorted(({room} | set(edges.get(room, ()))) & set(allowed))


def parse_goal(text: str) -> dict:
    """``--goal``: "part1" (E003's results/end-part1.json goal_all) or "K=V[,K=V]", as {key: [value]}."""
    if text == "part1":
        doc = read_json(E003 / "results" / "end-part1.json")
        if doc.get("goal_not"):
            raise SystemExit("end-part1.json has goal_not values; a stage goal holds values to reach only")
        return {k: [int(v)] for k, v in sorted(doc["goal_all"].items())}
    out: dict = {}
    for part in text.split(","):
        k, v = part.split("=")
        out.setdefault(k.strip(), []).append(int(v))
    return {k: sorted(set(v)) for k, v in sorted(out.items())}


def parse_allowed(text: str, guide: dict, start_room: int) -> list:
    """``--allowed``: kN (``size_scaling.scopes(game, start room)[N]``), files (the guide's rooms.reached) or R,R,..."""
    m = re.fullmatch(r"k(\d+)", text)
    if m:
        _engine_path()
        import model as M
        import size_scaling as SS
        sc = SS.scopes(M.Game.load(), start_room)
        if int(m[1]) not in sc:
            raise SystemExit(f"--allowed {text}: the scopes from room {start_room} end at k = {max(sc)}")
        return sorted(sc[int(m[1])])
    if text == "files":
        return sorted(int(r) for r in guide["rooms"]["reached"])
    return sorted({int(x) for x in text.split(",") if x.strip()})


# --- the machine ------------------------------------------------------------------------------------------------------

def _env_float(name: str, default: float) -> float:
    v = os.environ.get(name)
    return float(v) if v not in (None, "") else default


def machine() -> dict:
    """The machine's thresholds and paths (plan.md "Parallelism and the machine"), from the environment so the VM can
    set its own."""
    return {"mem_second_gb": _env_float("E008_MEM_SECOND_GB", 7.0), "evict_gb": _env_float("E008_EVICT_GB", 10.0),
            "floor_gb": _env_float("E008_FLOOR_GB", 8.0), "disk_path": os.environ.get("E008_DISK_PATH") or "/home",
            "guard": os.environ.get("E008_GUARD") or str(Path.home() / ".cache/fast-mi-e007/guard.sh"),
            "heavy_lock": os.environ.get("E008_HEAVY_LOCK") or "/var/tmp/fast-mi/heavy.lock",
            "poll_s": _env_float("E008_POLL_S", 1.0)}


def mem_available_gb() -> float:
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemAvailable:"):
            return int(line.split()[1]) / (1 << 20)
    return 0.0


def disk_free_fn(path: str):
    return lambda: shutil.disk_usage(path).free


def _proc_stat(pid: int) -> list | None:
    try:
        s = Path(f"/proc/{pid}/stat").read_text()
    except OSError:
        return None
    return s[s.rfind(")") + 2:].split()  # fields from the 3rd (state) on


def proc_start(pid: int) -> int | None:
    st = _proc_stat(pid)
    return int(st[19]) if st else None  # field 22, starttime


def proc_alive(pid: int, start: int | None) -> bool:
    st = _proc_stat(pid)
    return st is not None and st[0] != "Z" and (start is None or int(st[19]) == start)


def proc_tree(pid: int) -> list:
    kids: dict = {}
    for p in os.listdir("/proc"):
        if p.isdigit():
            st = _proc_stat(int(p))
            if st:
                kids.setdefault(int(st[1]), []).append(int(p))
    out, todo = [], [pid]
    while todo:
        p = todo.pop()
        out.append(p)
        todo += kids.get(p, [])
    return out


def kill_tree(pid: int, grace_s: float = 5.0) -> None:
    tree = proc_tree(pid)
    for sig in (signal.SIGTERM, signal.SIGKILL):
        for p in reversed(tree):
            try:
                os.kill(p, sig)
            except OSError:
                pass
        t = time.monotonic()
        while time.monotonic() - t < grace_s and any(proc_alive(p, None) for p in tree):
            time.sleep(0.05)


def plan_env() -> dict:
    """The builds plan.md's "CLIs" names (svm-w3b's ScummVM, main's Fast Downward, walk's taskcore, main's reader
    under ~/.cache/fast-mi-e008), set where the environment does not name them and the build is there; the values in
    use are returned (stage.json keeps them)."""
    home = Path.home()
    for k, v in (("FAST_MI_SCUMMVM_BIN", home / ".cache/fast-mi/svm-w3b/build/scummvm/scummvm"),
                 ("DOWNWARD_DIR", next((d for d in (HERE.parents[1] / "third_party/downward",
                                                     home / "projects/fast-mi/third_party/downward")
                                        if (d / "builds").is_dir()), HERE.parents[1] / "third_party/downward")),
                 ("TASKCORE_BIN", home / ".cache/fast-mi-walk/target/release/taskcore"),
                 ("READER_SCOPE_BIN", home / ".cache/fast-mi-e008/target-scumm/release/reader-scope")):
        if not os.environ.get(k) and v.exists():
            os.environ[k] = str(v)
    return {k: os.environ.get(k) for k in ("FAST_MI_SCUMMVM_BIN", "DOWNWARD_DIR", "TASKCORE_BIN", "READER_SCOPE_BIN")}


def _engine_path() -> None:
    plan_env()
    for p in (str(SRC), str(E003)):
        if p not in sys.path:
            sys.path.insert(0, p)


# --- the archive ------------------------------------------------------------------------------------------------------

class Archive:
    """The archive's files and what follows from them (module docstring). Built by ``create`` or ``open``."""

    def __init__(self, root: Path):
        self.root = Path(root)
        self.stage = read_json(self.root / "stage.json")
        self.goal = {k: list(v) for k, v in self.stage["goal"].items()}
        self.guide = load_guide(self.stage["guide"])
        self.atoms = guide_atoms(self.guide)
        self.cell_keys = sorted(k for k in self.guide["keys"] if k.startswith(KEY_KINDS))
        self.edges = {int(r): [int(x) for x in v] for r, v in self.guide["rooms"]["edges"].items()}
        self.allowed = set(self.stage["allowed_rooms"])
        self.rows: list = []
        self.cellmap: dict = {}
        self.held_by: dict = {}  # (k, v) -> (tick, id) of its least holder
        self.root_facts: dict | None = None
        self.readings: list = []
        self.logs: list = []
        self.busy: dict = {}  # job -> (entry id, cell)
        self._chain_r: dict = {}

    # --- making and opening

    @classmethod
    def create(cls, root: Path, stage: str, goal: dict, guide: Path, allowed, workers: int = 2,
               root_info: dict | None = None) -> "Archive":
        root = Path(root)
        if (root / "stage.json").exists():
            raise SystemExit(f"{root}: an archive is already there")
        load_guide(guide)
        for d in ("entries", "jobs", "readings"):
            (root / d).mkdir(parents=True, exist_ok=True)
        write_json(root / "stage.json", {
            "stage": stage, "seed": SEED, "goal": {k: sorted(v) for k, v in goal.items()},
            "guide": str(Path(guide).resolve()), "allowed_rooms": sorted(int(r) for r in allowed), "workers": workers,
            "created": time.strftime("%Y-%m-%dT%H:%M:%S"), "root": root_info or {}})
        for f in ("entries.jsonl", "log.jsonl", "readings.jsonl"):
            (root / f).touch()
        return cls.open(root)

    @classmethod
    def open(cls, root: Path) -> "Archive":
        a = cls(root)
        a._load()
        return a

    def _load(self) -> None:
        r = self.root
        self.logs, clean = read_jsonl(r / "log.jsonl")
        if not clean:
            rewrite_jsonl(r / "log.jsonl", self.logs)
        committed = {x["job"] for x in self.logs if x.get("outcome") in COMMITTED}
        rows, clean = read_jsonl(r / "entries.jsonl")
        keep = []
        for row in rows:  # an ingest cut short: its rows (always the last ones) and their files go
            if row.get("job") is None or row["job"] in committed:
                if row["id"] != len(keep):
                    raise SystemExit(f"{r}/entries.jsonl: entry {row['id']} at line {len(keep)}")
                keep.append(row)
        if len(keep) != len(rows) or not clean:
            for row in rows[len(keep):]:
                (r / "entries" / f"{row['id']}.json.gz").unlink(missing_ok=True)
            rewrite_jsonl(r / "entries.jsonl", keep)
        for p in (r / "entries").glob("*.json.gz"):  # files written before their row
            if int(p.name.split(".")[0]) >= len(keep):
                p.unlink()
        old_cells = read_json(r / "cells.json") if (r / "cells.json").exists() else {}
        for row in keep:
            if row["parent"] is None:
                self.root_facts = self.entry(0)["checkpoints"][-1][1]
            self._place(row, (old_cells.get(row["cell"]) or {}).get("key"))
        for x in self.logs:
            if x.get("outcome") in COMMITTED and x.get("cell") in self.cellmap:
                c = self.cellmap[x["cell"]]
                c["n"] = x["n"]
                if x.get("tried") is not None:
                    c["tried"][str(x["entry"])] = x["tried"]
                if x.get("exhausted") and x["entry"] not in c["exhausted"]:
                    c["exhausted"].append(x["entry"])
        for c, cell in self.cellmap.items():
            if cell["key"] is None:
                cell["key"] = self.cell_key(self.entry(cell["entries"][0])["checkpoints"][-1][1])
        readings, clean = read_jsonl(r / "readings.jsonl")
        self.readings = [x for x in readings if x.get("job") is None or x["job"] in committed]
        if len(self.readings) != len(readings) or not clean:
            rewrite_jsonl(r / "readings.jsonl", self.readings)

    # --- entries

    def entry(self, i: int) -> dict:
        return read_gz(self.root / "entries" / f"{i}.json.gz")

    def facts(self, i: int) -> dict:
        return self.entry(i)["checkpoints"][-1][1]

    def cell_key(self, facts: dict) -> list:
        ego = facts.get("ego")
        owned = sorted(int(k[6:]) for k, v in facts.items()
                       if k.startswith("owner:") and ego is not None and v == ego)
        return [facts.get("room"), owned, [[k, facts.get(k, 0)] for k in self.cell_keys]]

    @staticmethod
    def cell_id(key: list) -> str:
        return hashlib.sha1(canonical(key).encode()).hexdigest()

    def held(self, facts: dict) -> list:
        """The R atoms ``facts`` holds, as [k, v] pairs (module docstring)."""
        root = self.root_facts if self.root_facts is not None else facts
        out = []
        for k, vals in self.atoms.items():
            v = facts.get(k, 0)
            if v in vals or (None in vals and v != root.get(k, 0)):
                out.append([k, v])
        return sorted(out)

    def g(self, facts: dict) -> int:
        """#g: the goal keys whose value is not the goal's."""
        return sum(1 for k, vals in self.goal.items() if facts.get(k, 0) not in vals)

    def chain(self, i: int) -> list:
        out = []
        while i is not None:
            out.append(i)
            i = self.rows[i]["parent"]
        return out[::-1]

    def chain_r(self, i: int | None) -> frozenset:
        if i is None:
            return frozenset()
        if i not in self._chain_r:
            s: set = set()
            for j in self.chain(i):
                s |= {tuple(p) for p in self.rows[j]["seg_held"]}
            self._chain_r[i] = frozenset(s)
        return self._chain_r[i]

    def movie(self, i: int) -> list:
        """The concatenation of the chain's segments from the root: the events of the entry's movie from boot."""
        out = []
        for j in self.chain(i):
            out += self.entry(j)["events"]
        return out

    def checkpoints(self, i: int) -> list:
        out = []
        for j in self.chain(i):
            out += self.entry(j)["checkpoints"]
        return out

    def add(self, parent: int | None, frame: int, tick, events: list, checkpoints: list, job: str | None = None,
            how: str = "checkpoint") -> int:
        """A new entry (plan.md "Entries"): its file, then its row. Returns its id."""
        i = len(self.rows)
        frame = int(frame)
        cps = [list(c) for c in checkpoints]
        if not cps or cps[-1][0] != frame:
            raise ValueError(f"entry {i}: the last checkpoint is not at the entry's frame {frame}")
        if any(b[0] < a[0] for a, b in zip(cps, cps[1:])):
            raise ValueError(f"entry {i}: checkpoints out of frame order")
        if parent is None:
            if i != 0 or events:
                raise ValueError("only entry 0, the root, has no parent, and it has no events")
        else:
            if not 0 <= parent < i:
                raise ValueError(f"entry {i}: parent {parent} is not in the archive")
            f0 = self.rows[parent]["frame"]
            if any(not f0 < e["frame"] <= frame for e in events):
                raise ValueError(f"entry {i}: an event outside its segment ({f0}, {frame}]")
            if any(c[0] <= f0 for c in cps):
                raise ValueError(f"entry {i}: a checkpoint at or before the parent's frame {f0}")
        facts = cps[-1][1]
        if parent is None:
            self.root_facts = facts
        key = self.cell_key(facts)
        seg = sorted({tuple(p) for c in cps for p in self.held(c[1])})
        g = self.g(facts)
        row = {"id": i, "parent": parent, "frame": frame, "tick": tick, "room": facts.get("room"),
               "cell": self.cell_id(key), "g": g, "held": self.held(facts),
               "r_path": len(self.chain_r(parent) | set(seg)), "job": job, "how": how, "goal": g == 0,
               "seg_held": [list(p) for p in seg]}
        write_gz(self.root / "entries" / f"{i}.json.gz",
                 {"id": i, "parent": parent, "frame": frame, "tick": tick, "events": list(events), "checkpoints": cps})
        append_jsonl(self.root / "entries.jsonl", [row])
        self.last_place = self._place(row, key)
        return i

    def _place(self, row: dict, key) -> tuple:
        """The row in memory: its cell and the least holders. (new cell, new representative)."""
        self.rows.append(row)
        i, c = row["id"], row["cell"]
        new_cell = c not in self.cellmap
        if new_cell:
            self.cellmap[c] = {"key": key, "rep": i, "entries": [], "n": 0, "exhausted": [], "tried": {}}
        cell = self.cellmap[c]
        cell["entries"].append(i)
        new_rep = not new_cell and self._order(i) < self._order(cell["rep"])
        if new_rep:
            cell["rep"] = i
        for p in row["held"]:
            p = tuple(p)
            if p not in self.held_by or self._order(i) < self.held_by[p]:
                self.held_by[p] = self._order(i)
        return new_cell, new_rep

    def _order(self, i: int) -> tuple:
        t = self.rows[i]["tick"]
        return (t if t is not None else float("inf"), i)

    # --- cells and the order

    def cells(self) -> dict:
        """{cell: {"key", "rep", "entries", "n", "exhausted", "tried"}} (cells.json)."""
        return self.cellmap

    def representative(self, cell: str) -> int:
        return self.cellmap[cell]["rep"]

    def favoured(self) -> set:
        return {i for _, i in self.held_by.values()}

    def sel_key(self, i: int, fav: set | None = None) -> tuple:
        row = self.rows[i]
        fav = self.favoured() if fav is None else fav
        t = row["tick"] if row["tick"] is not None else float("inf")
        return (row["g"], 0 if i in fav else 1, self.cellmap[row["cell"]]["n"], -row["r_path"], t, i)

    def selectable(self, cell: str) -> bool:
        c = self.cellmap[cell]
        return c["rep"] not in c["exhausted"] and self.rows[c["rep"]]["room"] in self.allowed

    def n_selectable(self) -> int:
        return sum(1 for c in self.cellmap if self.selectable(c))

    def select(self, b: int) -> list:
        """The first ``b`` representatives in the order (module docstring), each of another cell."""
        busy_ids = {e for e, _ in self.busy.values()}
        busy_cells = {c for _, c in self.busy.values()} | {self.rows[e]["cell"] for e in busy_ids}
        fav = self.favoured()
        cands = [c["rep"] for cid, c in self.cellmap.items()
                 if cid not in busy_cells and c["rep"] not in busy_ids and self.selectable(cid)]
        return sorted(cands, key=lambda i: self.sel_key(i, fav))[:b]

    def goal_entries(self) -> list:
        return [r["id"] for r in self.rows if r["goal"]]

    def stagnant(self) -> bool:
        """After the last committed job that added a pair no earlier entry held, as many committed jobs as there were
        selectable cells right after it (1 at the start, the root's cell), none adding one."""
        done = [x for x in self.logs if x.get("outcome") in COMMITTED]
        last = max((k for k, x in enumerate(done) if x.get("new_r")), default=None)
        frontier = 1 if last is None else max(1, done[last].get("selectable", 1))
        since = len(done) - (0 if last is None else last + 1)
        return since >= frontier

    def failures(self) -> dict:
        """{(entry, error): count} over the committed jobs that failed."""
        out: dict = {}
        for x in self.logs:
            if x.get("outcome") == "error":
                k = (x["entry"], x.get("error"))
                out[k] = out.get(k, 0) + 1
        return out

    # --- readings

    def readings_for(self, scope) -> list:
        """The live readings of ``scope``, newest first."""
        s = sorted(scope)
        return [x for x in reversed(self.readings)
                if x["scope"] == s and not x.get("evicted") and Path(x["path"]).exists()]

    def register_reading(self, scope, path, start_entry: int, job: str | None, kind: str = "fresh",
                         used: int | None = None) -> dict:
        p = Path(path)
        row = {"scope": sorted(int(r) for r in scope), "path": str(p), "start_entry": start_entry, "job": job,
               "bytes": dir_bytes(p.parent) if p.parent.exists() else 0, "last_used": used, "kind": kind}
        self.readings.append(row)
        append_jsonl(self.root / "readings.jsonl", [row])
        return row

    def touch_reading(self, path, used: int) -> None:
        for x in self.readings:
            if x["path"] == str(path):
                x["last_used"] = used
        rewrite_jsonl(self.root / "readings.jsonl", self.readings)

    def evict(self, free_fn, in_use=(), level_gb: float | None = None) -> list:
        """Deletes readings least recently used first, never one in ``in_use``, while ``free_fn()`` is under the
        evict level (E008_EVICT_GB). A reading's files go; its start files stay (START_FILES: other readings'
        --reach-also may name start.txt, and a refinement's or reach extension's reading names its start.jsonl as
        its start rows). Returns the paths evicted."""
        level = (machine()["evict_gb"] if level_gb is None else level_gb) * GIB
        in_use = {str(p) for p in in_use}
        out = []
        order = sorted((k for k, x in enumerate(self.readings) if not x.get("evicted") and x["path"] not in in_use),
                       key=lambda k: (self.readings[k]["last_used"] if self.readings[k]["last_used"] is not None
                                      else -1, k))
        for k in order:
            if free_fn() >= level:
                break
            x = self.readings[k]
            d = Path(x["path"]).parent.resolve()
            live = [Path(y["path"]).parent.resolve() for y in self.readings
                    if y is not x and not y.get("evicted")]
            if self.root.resolve() in d.parents and d not in (self.root.resolve() / "readings",
                                                              self.root.resolve() / "jobs"):
                _evict_dir(d, [q for q in live if q == d or d in q.parents])
            x["evicted"] = True
            out.append(x["path"])
        if out:
            rewrite_jsonl(self.root / "readings.jsonl", self.readings)
        return out

    # --- jobs

    def job_names(self) -> list:
        return sorted(p.name for p in (self.root / "jobs").iterdir() if p.is_dir() and re.fullmatch(r"j\d+", p.name))

    def next_job(self) -> str:
        names = self.job_names()
        return f"j{(int(names[-1][1:]) + 1 if names else 0):05d}"

    def committed_jobs(self) -> set:
        return {x["job"] for x in self.logs if x.get("outcome") in COMMITTED}

    def unfinished_jobs(self) -> list:
        """Job dirs with a job.json and no committed log line, in start order."""
        done = self.committed_jobs()
        return [n for n in self.job_names() if n not in done and (self.root / "jobs" / n / "job.json").exists()]

    def job_doc(self, job: str, i: int) -> dict:
        """job.json (plan.md "Interfaces")."""
        row = self.rows[i]
        cell = self.cellmap[row["cell"]]
        scope = ring(self.edges, row["room"], self.allowed)
        rooms = "-".join(str(r) for r in scope)
        return {"job": job, "seed": SEED, "stage": self.stage["stage"], "goal": self.goal, "guide": self.stage["guide"],
                "entry": {"id": i, "root": row["parent"] is None, "frame": row["frame"], "events": self.movie(i),
                          "checkpoint": self.entry(i)["checkpoints"][-1]},
                "scope": scope, "tried": cell["tried"].get(str(i), []),
                "held": [list(p) for p in sorted(self.held_by)],
                "readings": [{"path": x["path"], "start_entry": x["start_entry"]} for x in self.readings_for(scope)],
                "reading_dir": str(self.root / "readings" / f"{rooms}-{job}"),
                "threads": int(os.environ.get("E008_THREADS") or THREADS),
                "workers": int(os.environ.get("E008_WINDOW_WORKERS") or WINDOW_WORKERS)}

    def prepare_job(self, i: int) -> Path:
        """jobs/JOB/job.json for representative ``i``, and select.json (its selection key, for log.jsonl)."""
        job = self.next_job()
        d = self.root / "jobs" / job
        d.mkdir(parents=True)
        write_json(d / "select.json", {"key": list(self.sel_key(i)), "cell": self.rows[i]["cell"],
                                       "selectable": self.n_selectable(), "time": time.time()})
        write_json(d / "job.json", self.job_doc(job, i))
        return d

    def ingest(self, job_dir: Path, extra: dict | None = None) -> dict:
        """Commits a finished job (plan.md "Interfaces"): its new entries in the result's order (ids in ingest order),
        cells.json, readings.jsonl (the fresh reading and readings_made), then its log.jsonl line, which marks it
        committed. Returns that line."""
        job_dir = Path(job_dir)
        job = read_json(job_dir / "job.json")
        res = read_json(job_dir / "result.json")
        name, e = job["job"], job["entry"]["id"]
        if name in self.committed_jobs():
            raise ValueError(f"{name} is committed already")
        cell = self.rows[e]["cell"]
        sel = read_json(job_dir / "select.json") if (job_dir / "select.json").exists() else {}
        line = {"job": name, "entry": e, "cell": cell, "key": sel.get("key"), "scope": job["scope"]}
        ret = res.get("return") or {}
        error = res.get("error") if not res.get("ok") else None
        new_files = []
        if ret.get("same") is False:
            outcome = "return differs"
        else:
            outcome = "ok" if res.get("ok") else "error"
            try:
                if res.get("job") != name or res.get("entry") != e:
                    raise ValueError(f"result.json names job {res.get('job')} entry {res.get('entry')}")
                for n in res.get("new") or ():
                    doc = read_gz(job_dir / n["file"])
                    if doc.get("parent") != e:
                        raise ValueError(f"{n['file']}: parent {doc.get('parent')}, not the job's entry {e}")
                    f0 = self.rows[e]["frame"]
                    if not doc["checkpoints"] or doc["checkpoints"][-1][0] != doc["frame"] or doc["frame"] <= f0 \
                            or any(not f0 < ev["frame"] <= doc["frame"] for ev in doc["events"]):
                        raise ValueError(f"{n['file']}: not a segment after frame {f0}")
                    new_files.append((n, doc))
            except (ValueError, KeyError, OSError, json.JSONDecodeError) as x:
                outcome, error, new_files = "error", f"ingest: {type(x).__name__}: {x}", []
        before = set(self.held_by)
        ids, new_cells, new_reps = [], 0, 0
        for n, doc in new_files:
            ids.append(self.add(e, doc["frame"], doc.get("tick"), doc["events"], doc["checkpoints"], name,
                                n.get("how") or "checkpoint"))
            nc, nr = self.last_place
            new_cells += nc
            new_reps += nr
        new_r = sorted(list(p) for p in set(self.held_by) - before)
        c = self.cellmap[cell]
        if outcome != "return differs":
            c["n"] = 0 if (new_cells or new_reps) else c["n"] + 1
            tried = {tuple(t) for t in c["tried"].get(str(e), [])} | {tuple(t) for t in res.get("tried") or ()}
            c["tried"][str(e)] = [list(t) for t in sorted(tried, key=canonical)]
            if res.get("exhausted") and e not in c["exhausted"]:
                c["exhausted"].append(e)
        used = len(self.committed_jobs())
        rd = res.get("reading") or {}
        if rd.get("path") and outcome != "return differs":
            if rd.get("fresh"):
                self.register_reading(rd.get("scope") or job["scope"], rd["path"], e, name, "fresh", used)
            else:
                self.touch_reading(rd["path"], used)
        for m in (res.get("readings_made") or ()) if outcome != "return differs" else ():
            self.register_reading(m.get("scope") or job["scope"], m["path"], e, name, m.get("kind", "made"), used)
        self.write_cells()
        att = res.get("attempt") or {}
        line.update({
            "outcome": outcome, "error": error, "new_entries": ids, "new_cells": new_cells, "new_reps": new_reps,
            "new_r": new_r, "n": c["n"], "tried": c["tried"].get(str(e)), "exhausted": bool(res.get("exhausted")),
            "selectable": self.n_selectable(), "goal_entries": [i for i in ids if self.rows[i]["goal"]],
            "return": ret, "reading": {k: rd.get(k) for k in ("path", "fresh", "why", "scope", "records", "actions",
                                                               "read_wall_s", "read_cpu_s", "peak_rss_mb", "files_s")},
            "targets": (res.get("targets") or {}).get("by_tier"), "probes": len(res.get("probes") or ()),
            "attempt": {k: att.get(k) for k in ("target", "stop", "reached", "goal_from_the_plan", "frames", "qf0",
                                                "qf_end", "replans", "refinements", "extensions", "seconds")}
            if att else None,
            "cpu_s": res.get("cpu_s"), "peak_rss_mb": res.get("peak_rss_mb"), "wall_s": res.get("wall_s")})
        line.update(extra or {})
        self.logs.append(line)
        append_jsonl(self.root / "log.jsonl", [line])
        return line

    def log_event(self, line: dict) -> None:
        """A log.jsonl line that commits nothing (a guard's kill)."""
        self.logs.append(line)
        append_jsonl(self.root / "log.jsonl", [line])

    def write_cells(self) -> None:
        write_json(self.root / "cells.json", self.cellmap, indent=None)

    # --- verify and report

    def verify(self, i: int | None = None, out_dir: Path | None = None) -> dict:
        """plan.md "Verify": the entry's movie from boot in a fresh engine against every checkpoint of its chain, then
        the run clock at its frame. Writes ARCHIVE/verify-ID.json."""
        if i is None:
            goals = self.goal_entries()
            if not goals:
                raise SystemExit(f"{self.root}: no goal entry to verify")
            i = goals[0]
        _engine_path()
        import replay
        import replay_part1
        import start as S
        from speedrun import releases
        out_dir = Path(out_dir) if out_dir is not None else self.root / f"verify-{i}"
        movie, cps = self.movie(i), self.checkpoints(i)

        def cfg_of(o, f):
            return replay_part1.config(SEED, o, f)
        t = time.monotonic()
        ver = replay.verify(cfg_of, movie, cps, sorted(self.goal), out_dir)
        ver["seconds"] = round(time.monotonic() - t, 1)
        frame = self.rows[i]["frame"]
        ctl, b = replay.boot(cfg_of, movie, frame, out_dir / "clock")
        try:
            qf, tick = b.status.get("qf"), b.status.get("tick")
        finally:
            ctl.close()
        root = self.stage.get("root") or {}
        hz = releases.get(S.RELEASE).timer_hz
        dq = qf - root["qf"] if qf is not None and root.get("qf") is not None else None
        out = {"entry": i, "chain": self.chain(i), "segments": len(self.chain(i)) - 1, "frame": frame,
               "frames": frame - self.rows[0]["frame"], "qf": dq, "qf_end": qf, "qf_root": root.get("qf"),
               "seconds": dq / hz if dq is not None else None, "tick_end": tick, "inputs": ver.get("inputs"),
               "goal": all(ver.get("final", {}).get(k) in v for k, v in self.goal.items()),
               "same": ver["same"], "verify": ver}
        write_json(self.root / f"verify-{i}.json", out)
        return out

    def report(self) -> dict:
        """plan.md "Runs", reported per stage: from the archive's files."""
        done = [x for x in self.logs if x.get("outcome") in COMMITTED]
        res = {}
        for x in done:
            p = self.root / "jobs" / x["job"] / "result.json"
            res[x["job"]] = read_json(p) if p.exists() else {}
        tiers: dict = {}
        probes: dict = {}
        cpu: dict = {}
        peak: dict = {}
        for r in res.values():
            for t, n in ((r.get("targets") or {}).get("by_tier") or {}).items():
                tiers[t] = tiers.get(t, 0) + n
            for p in r.get("probes") or ():
                probes[p.get("status")] = probes.get(p.get("status"), 0) + 1
            for k, v in (r.get("cpu_s") or {}).items():
                cpu[k] = round(cpu.get(k, 0) + (v or 0), 1)
            for k, v in (r.get("peak_rss_mb") or {}).items():
                peak[k] = max(peak.get(k, 0), v or 0)
        fresh = [x for x in self.readings if x.get("kind") == "fresh"]
        rep = {
            "stage": self.stage["stage"], "goal": self.goal, "allowed_rooms": sorted(self.allowed),
            "jobs": {"committed": len(done), "ok": sum(x["outcome"] == "ok" for x in done),
                     "errors": sum(x["outcome"] == "error" for x in done),
                     "guard_kills": sum(x.get("outcome") == "guard-killed" for x in self.logs),
                     "exhausted": sum(bool(x.get("exhausted")) for x in done)},
            "entries": len(self.rows), "cells": len(self.cellmap), "selectable": self.n_selectable(),
            "held_atoms": len(self.held_by),
            "readings": {"registered": len(self.readings), "fresh": len(fresh),
                         "reused": sum(1 for x in done if (x.get("reading") or {}).get("fresh") is False),
                         "by_kind": _count(x.get("kind") for x in self.readings),
                         "scopes": _count("-".join(map(str, x["scope"])) for x in fresh),
                         "records": sum((x.get("reading") or {}).get("records") or 0 for x in done
                                        if (x.get("reading") or {}).get("fresh")),
                         "read_cpu_s": round(sum((x.get("reading") or {}).get("read_cpu_s") or 0 for x in done), 1),
                         "read_wall_s": round(sum((x.get("reading") or {}).get("read_wall_s") or 0 for x in done), 1),
                         "peak_rss_mb": max([(x.get("reading") or {}).get("peak_rss_mb") or 0 for x in done],
                                            default=0),
                         "bytes": sum(x.get("bytes") or 0 for x in self.readings if not x.get("evicted")),
                         "evicted": sum(1 for x in self.readings if x.get("evicted"))},
            "targets_by_tier": tiers, "probes": probes,
            "attempts": sum(1 for r in res.values() if r.get("attempt")),
            "refinements": sum(_n((r.get("attempt") or {}).get("refinements")) for r in res.values()),
            "cpu_s": cpu, "peak_rss_mb": peak,
            "driver_peak_rss_mb": round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024, 1),
            "disk_bytes": dir_bytes(self.root), "goal_entries": self.goal_entries(),
            "summary": read_json(self.root / "summary.json") if (self.root / "summary.json").exists() else None}
        vs = sorted(self.root.glob("verify-*.json"), key=lambda p: int(p.stem.split("-")[1]))
        target = self.goal_entries()[0] if self.goal_entries() else None
        vf = self.root / f"verify-{target}.json" if target is not None else (vs[-1] if vs else None)
        if vf is not None and vf.exists():
            v = read_json(vf)
            target = v["entry"]
            rep["verified"] = {k: v.get(k) for k in ("entry", "segments", "frame", "frames", "qf", "seconds",
                                                     "inputs", "goal", "same")}
            rep["verified"]["differ"] = v["verify"].get("differ")
        if target is not None:
            row = self.rows[target]
            how = (res.get(row["job"]) or {}).get("attempt") or {}
            rep["goal_chain"] = {"entry": target, "chain": self.chain(target), "frames": row["frame"] -
                                 self.rows[0]["frame"], "inputs": sum(1 for ev in self.movie(target)
                                                                      if any(k in ev for k in
                                                                             ("key", "click", "rclick", "move"))),
                                 "goal_from_the_plan": how.get("goal_from_the_plan"), "stop": how.get("stop")}
        return rep


def _n(x) -> int:
    """A count the result gives as a number or as a list of records."""
    return len(x) if isinstance(x, (list, tuple)) else int(x or 0)


def _count(xs) -> dict:
    out: dict = {}
    for x in xs:
        out[x] = out.get(x, 0) + 1
    return out


START_FILES = ("start.txt", "start.jsonl")  # what an eviction keeps (_evict_dir)


def _evict_dir(d: Path, keep: list) -> None:
    """A reading's files under ``d``, except its start files (START_FILES: start.txt, which other readings'
    ``--reach-also`` may name, and start.jsonl, which the readings a refinement or a reach extension made from it name
    as their start rows) and the dirs of other live readings (``keep``, inside ``d``); nothing when another live
    reading shares ``d``."""
    if d in keep:
        return
    for p, dirs, files in os.walk(d, topdown=False):
        pp = Path(p).resolve()
        if any(pp == q or q in pp.parents for q in keep):
            continue
        for f in files:
            if f not in START_FILES:
                try:
                    os.unlink(os.path.join(p, f))
                except OSError:
                    pass
        try:
            os.rmdir(p)
        except OSError:
            pass


# --- the driver -------------------------------------------------------------------------------------------------------

class Job:
    """One step process (module docstring, "Jobs"), started by this driver or found running on a resume."""

    def __init__(self, a: Archive, name: str):
        self.a, self.name = a, name
        self.dir = a.root / "jobs" / name
        self.doc = read_json(self.dir / "job.json")
        self.entry, self.scope = self.doc["entry"]["id"], self.doc["scope"]
        self.cell = a.rows[self.entry]["cell"]
        self.proc = None
        self.pid = self.pid_start = None
        self.done = False
        self.rc = None
        self.rusage = None
        self.started = self.ended = None
        self.state = None  # None while running; then "finished" or "guard-killed"

    def in_use(self) -> set:
        return {r["path"] for r in self.doc["readings"]} | {str(Path(self.doc["reading_dir"]) / "reading.json")}

    def command(self, step_cmd: list, m: dict) -> list:
        cmd = [m["guard"], str(self.dir / "log")] + step_cmd + ["run", str(self.dir)]
        if len(self.scope) >= HEAVY_ROOMS:
            cmd = ["flock", m["heavy_lock"]] + cmd
        return cmd

    def clear(self) -> None:
        """Before a run (again): what an earlier, unfinished run of the job left, except its logs."""
        for f in ("result.json", "pid.json"):
            (self.dir / f).unlink(missing_ok=True)
        for d in ("new", "run"):
            shutil.rmtree(self.dir / d, ignore_errors=True)
        rd = Path(self.doc["reading_dir"])
        if rd.exists() and self.a.root.resolve() in rd.resolve().parents:
            shutil.rmtree(rd, ignore_errors=True)
        if (self.dir / "log").exists():
            k = 1
            while (self.dir / f"log.{k}").exists():
                k += 1
            os.replace(self.dir / "log", self.dir / f"log.{k}")

    def start(self, step_cmd: list, m: dict) -> None:
        self.clear()
        self.started = time.time()
        with open(self.dir / "spawn.log", "a") as out:
            self.proc = subprocess.Popen(self.command(step_cmd, m), cwd=HERE, stdin=subprocess.DEVNULL, stdout=out,
                                         stderr=out, start_new_session=True)
        self.pid, self.pid_start = self.proc.pid, proc_start(self.proc.pid)
        write_json(self.dir / "pid.json", {"pid": self.pid, "start": self.pid_start, "started": self.started})
        self.done, self.state, self.ended = False, None, None

    def attach(self) -> bool:
        """On a resume: the job's process from an earlier driver, if it still runs."""
        p = self.dir / "pid.json"
        if not p.exists():
            return False
        d = read_json(p)
        if not proc_alive(d["pid"], d.get("start")):
            return False
        self.pid, self.pid_start, self.started = d["pid"], d.get("start"), d.get("started")
        return True

    def poll(self) -> bool:
        if self.done:
            return True
        if self.proc is not None:
            pid, status, ru = os.wait4(self.proc.pid, os.WNOHANG)
            if pid == 0:
                return False
            self.rc = os.waitstatus_to_exitcode(status)
            self.proc.returncode = self.rc
            self.rusage = {"cpu_s": round(ru.ru_utime + ru.ru_stime, 1), "maxrss_mb": round(ru.ru_maxrss / 1024, 1)}
        elif self.pid is not None and proc_alive(self.pid, self.pid_start):
            return False
        self.done, self.ended = True, time.time()
        return True

    def kill(self) -> None:
        if self.pid is not None and not self.done:
            kill_tree(self.pid)
            self.poll()

    def result_ok(self) -> bool:
        try:
            r = read_json(self.dir / "result.json")
        except (OSError, json.JSONDecodeError):
            return False
        return r.get("job") == self.name

    def guard_killed(self) -> bool:
        p = self.dir / "log"
        return p.exists() and any(line.startswith("GUARD: MemAvailable") and "killing group" in line
                                  for line in p.read_text(errors="replace").splitlines())

    def classify(self) -> str:
        """After the process ended: "finished" (a result.json, the step's or one that says it left none) or
        "guard-killed"."""
        if self.result_ok():
            return "finished"
        if self.guard_killed():
            return "guard-killed"
        tail = ""
        if (self.dir / "log").exists():
            tail = " | ".join((self.dir / "log").read_text(errors="replace").splitlines()[-3:])
        write_json(self.dir / "result.json", {
            "job": self.name, "entry": self.entry, "ok": False,
            "error": f"the step ended (exit {self.rc}) with no result.json", "log_tail": tail, "return": None,
            "reading": None, "targets": None, "probes": [], "attempt": None, "readings_made": [], "new": [],
            "tried": [], "exhausted": False})
        return "finished"

    def extra(self) -> dict:
        ended = time.time()
        guard = None
        if (self.dir / "log").exists():
            guard = next((line for line in reversed((self.dir / "log").read_text(errors="replace").splitlines())
                          if line.startswith("GUARD: rc")), None)
        return {"started": self.started, "ended": self.ended or ended,
                "driver_wall_s": round(ended - self.started, 1) if self.started else None,
                "rc": self.rc, "rusage": self.rusage, "guard": guard}


def run(root: Path, workers: int, step_cmd: list | None = None, do_verify: bool = True) -> dict:
    """The job loop (module docstring): until a stop rule fires. Returns summary.json's contents."""
    a = Archive.open(root)
    if (a.root / "summary.json").exists():
        s = read_json(a.root / "summary.json")
        say(f"{a.root}: stopped already ({s.get('stop')}); summary.json")
        return s
    m = machine()
    step_cmd = step_cmd or shlex.split(os.environ.get("E008_STEP") or "") or \
        [sys.executable, "-u", str(HERE / "step.py")]
    a.stage["workers"] = workers
    a.stage["env"] = plan_env()
    write_json(a.root / "stage.json", a.stage)
    free = disk_free_fn(m["disk_path"])
    for name in a.job_names():  # a dir the driver made but did not finish writing: its selection is made again
        if not (a.root / "jobs" / name / "job.json").exists():
            shutil.rmtree(a.root / "jobs" / name)
    running: list = []
    restart: list = []
    kills = _count(x["job"] for x in a.logs if x.get("outcome") == "guard-killed")
    for name in a.unfinished_jobs():  # a resume (module docstring)
        j = Job(a, name)
        a.busy[name] = (j.entry, j.cell)
        running.append(j)
        if j.attach():
            say(f"{name}: still running (pid {j.pid}); waiting for it")
        elif j.result_ok():
            j.done, j.state = True, "finished"
        else:
            restart.append(j)
    stop, detail, stopping = None, None, False

    def can_start() -> bool:
        if free() < m["floor_gb"] * GIB:
            return False
        active = [j for j in running if not j.done and j not in restart]
        return not active or mem_available_gb() >= m["mem_second_gb"]

    while True:
        for j in running:
            if j not in restart and j.poll() and j.state is None:
                j.state = j.classify()
                if j.state == "guard-killed":
                    kills[j.name] = kills.get(j.name, 0) + 1
                    a.log_event({"job": j.name, "entry": j.entry, "outcome": "guard-killed", **j.extra()})
                    say(f"{j.name}: the guard killed it ({kills[j.name]}); not counted as tried")
                    if kills[j.name] >= 2:
                        stop, detail, stopping = "repeated failure", {"job": j.name, "entry": j.entry,
                                                                      "error": "the guard killed the job twice"}, True
                    else:
                        restart.append(j)
        committed = False
        if running and running[0].done and running[0].state == "finished":  # one commit, then its fill
            committed = True
            j = running.pop(0)
            line = a.ingest(j.dir, j.extra())
            a.busy.pop(j.name, None)
            say(f"{j.name}: entry {j.entry} {line['outcome']}; {len(line['new_entries'])} entries, "
                f"{line['new_cells']} new cells, {len(line['new_r'])} new atoms; archive {len(a.rows)} entries, "
                f"{len(a.cellmap)} cells")
            if line["outcome"] == "return differs":
                stop, detail = "return differs", {"job": j.name, "entry": j.entry, "return": line["return"]}
                for k in running:
                    k.kill()
                    a.busy.pop(k.name, None)
                running, restart = [], []
            elif not stopping and line["outcome"] == "error" and a.failures().get((j.entry, line["error"]), 0) >= 2:
                stop, detail, stopping = "repeated failure", {"job": j.name, "entry": j.entry,
                                                              "error": line["error"]}, True
        if stop == "return differs":
            break
        if not stopping and a.goal_entries():
            stop, detail, stopping = "goal", {"entry": a.goal_entries()[0]}, True
        if restart and not stopping:
            for j in list(restart):
                if can_start():
                    restart.remove(j)
                    say(f"{j.name}: run again")
                    j.start(step_cmd, m)
        elif stopping:
            for j in restart:
                running.remove(j)
                a.busy.pop(j.name, None)
            restart = []
        floor = False
        if not stopping and not a.stagnant():
            if free() < m["evict_gb"] * GIB:
                gone = a.evict(free, set().union(*(j.in_use() for j in running)))
                if gone:
                    say(f"evicted {len(gone)} readings")
            while len(running) < workers:
                if free() < m["floor_gb"] * GIB:
                    floor = True
                    break
                if not can_start():
                    break
                ids = a.select(1)
                if not ids:
                    break
                d = a.prepare_job(ids[0])
                j = Job(a, d.name)
                a.busy[j.name] = (j.entry, j.cell)
                j.start(step_cmd, m)
                running.append(j)
                say(f"{j.name}: entry {j.entry} (room {a.rows[j.entry]['room']}), scope {j.scope}, key "
                    f"{read_json(d / 'select.json')['key']}")
        if not running:
            if stop is None:
                if not a.select(1):
                    stop = "exhausted"
                elif a.stagnant():
                    stop = "stagnation"
                elif floor:
                    stop, detail = "disk floor", {"free_gb": round(free() / GIB, 1), "floor_gb": m["floor_gb"]}
                else:
                    stop = "no job could start"
            break
        if not committed:
            time.sleep(m["poll_s"])
    summary = {"stop": stop, "detail": detail, "jobs": len(a.committed_jobs()), "entries": len(a.rows),
               "cells": len(a.cellmap), "goal_entries": a.goal_entries(), "time": time.strftime("%Y-%m-%dT%H:%M:%S")}
    if stop == "goal":
        if do_verify:
            say(f"goal entry {detail['entry']}: verifying its movie from boot")
            v = a.verify(detail["entry"])
            summary["verify"] = {k: v.get(k) for k in ("entry", "same", "goal", "frames", "qf", "seconds",
                                                       "inputs", "segments")}
        else:
            summary["verify"] = None
    write_json(a.root / "summary.json", summary)
    say(f"stop: {stop} {detail or ''}")
    return summary


# --- the commands -----------------------------------------------------------------------------------------------------

def boot_root(out_dir: Path) -> tuple:
    """The root (module docstring, "Init"): its checkpoint and stage.json's root fields."""
    _engine_path()
    import replay
    import replay_part1
    ctl, b, row = replay_part1.boot(SEED, out_dir, dump_locals=True)
    try:
        if row["checks"]:
            raise SystemExit(f"the boot fails start.py's checks: {row['checks']}")
        s, st = b.state(), dict(b.status)
        cp = [st["frame"], row["facts"], replay.extra_of(s, st)]
    finally:
        ctl.close()
    return cp, {"frame": st["frame"], "tick": st.get("tick"), "qf": st.get("qf"), "room": row["facts"].get("room"),
                "boot_s": row["boot_s"], "stop": row["stop"]}


def cmd_init(args) -> None:
    root = Path(args.archive)
    if (root / "stage.json").exists():
        raise SystemExit(f"{root}: an archive is already there")
    guide = load_guide(args.guide)
    goal = parse_goal(args.goal)
    root.mkdir(parents=True, exist_ok=True)
    say(f"{root}: booting the root")
    cp, info = boot_root(root / "root")
    allowed = parse_allowed(args.allowed, guide, info["room"])
    a = Archive.create(root, args.stage, goal, Path(args.guide), allowed, args.workers, info)
    a.add(None, cp[0], info["tick"], [], [cp], None, "boot")
    a.write_cells()
    say(f"{root}: root at frame {cp[0]} (room {info['room']}, qf {info['qf']}); allowed rooms {allowed}")


def report_lines(rep: dict) -> list:
    out = [f"stage {rep['stage']}: goal {rep['goal']}, allowed rooms {rep['allowed_rooms']}",
           f"jobs {rep['jobs']}", f"entries {rep['entries']}, cells {rep['cells']} ({rep['selectable']} selectable), "
           f"R atoms held {rep['held_atoms']}", f"readings {rep['readings']}",
           f"targets by tier {rep['targets_by_tier']}, probes {rep['probes']}, attempts {rep['attempts']}, "
           f"refinements {rep['refinements']}", f"CPU s {rep['cpu_s']}, peak MB {rep['peak_rss_mb']}, driver "
           f"{rep['driver_peak_rss_mb']} MB, disk {rep['disk_bytes'] / 1e6:.1f} MB"]
    if rep.get("goal_chain"):
        out.append(f"goal chain {rep['goal_chain']}")
    if rep.get("verified"):
        v = rep["verified"]
        out.append(f"verified entry {v['entry']}: same {v['same']}, goal {v['goal']}, {v['frames']} frames, "
                   f"{v['qf']} qf, {v['seconds']:.2f} s from the root, {v['inputs']} inputs, {v['segments']} segments"
                   if v.get("seconds") is not None else f"verified entry {v['entry']}: {v}")
    if rep.get("summary"):
        out.append(f"stop {rep['summary'].get('stop')} {rep['summary'].get('detail') or ''}")
    return out


def main(argv=None) -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("init")
    p.add_argument("archive")
    p.add_argument("--stage", required=True)
    p.add_argument("--goal", required=True)
    p.add_argument("--guide", required=True)
    p.add_argument("--allowed", required=True)
    p.add_argument("--workers", type=int, default=2)
    p = sub.add_parser("run")
    p.add_argument("archive")
    p.add_argument("--workers", type=int, default=2)
    p.add_argument("--step-cmd", default=None, help="the step command before 'run JOB_DIR' (default env E008_STEP, "
                                                    "else python -u step.py)")
    p.add_argument("--no-verify", action="store_true", help="a goal stop without the verify (the tests' fake step)")
    p = sub.add_parser("verify")
    p.add_argument("archive")
    p.add_argument("--entry", type=int, default=None)
    p = sub.add_parser("report")
    p.add_argument("archive")
    args = ap.parse_args(argv)
    if args.cmd == "init":
        cmd_init(args)
    elif args.cmd == "run":
        run(Path(args.archive), args.workers, shlex.split(args.step_cmd) if args.step_cmd else None,
            not args.no_verify)
    elif args.cmd == "verify":
        v = Archive.open(Path(args.archive)).verify(args.entry)
        print(json.dumps({k: v.get(k) for k in ("entry", "same", "goal", "frames", "qf", "seconds", "inputs",
                                                "segments")}))
        print(f"differences: {len(v['verify']['differ'])}; final {v['verify'].get('final')}")
    elif args.cmd == "report":
        a = Archive.open(Path(args.archive))
        rep = a.report()
        write_json(a.root / "report.json", rep)
        print("\n".join(report_lines(rep)))


if __name__ == "__main__":
    main()
