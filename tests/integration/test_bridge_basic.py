"""Speedrun bridge skeleton (task 1.1): boot/end trace records, tick watchdog,
fast mode, determinism of the tick clock, and inertness without SPEEDRUN_OUT."""

import json
import re
import subprocess
import time
from pathlib import Path

import pytest

from speedrun import engine
from speedrun.engine import EngineConfig, build_argv, build_env, run_engine, write_ini

BRIDGE_VERSION = "speedrun-bridge v3"
END_FINGERPRINT = ("room", "audio_frames", "music_timer", "vars_fnv1a")


def _read_trace(out_dir: Path) -> list[dict]:
    trace = out_dir / "trace.jsonl"
    assert trace.exists(), f"no trace written to {trace}"
    text = trace.read_text(encoding="ascii")  # the bridge escapes bytes >= 0x80
    assert text.endswith("\n"), "trace must end with a complete, flushed line"
    return [json.loads(line) for line in text.splitlines()]


def _run_to_max_ticks(out_dir: Path, max_ticks: int = 1800, seed: int = 1):
    cfg = EngineConfig(out_dir=out_dir, fast=True, max_ticks=max_ticks, seed=seed, timeout_s=60)
    start = time.monotonic()
    result = run_engine(cfg)
    elapsed = time.monotonic() - start
    log = result.log_path.read_text(errors="replace")
    assert not result.timed_out, f"engine did not stop by itself:\n{log[-4000:]}"
    return cfg, result, elapsed, log


def _assert_fingerprint(end: dict) -> None:
    for key in END_FINGERPRINT:
        assert key in end, f"end record lacks {key!r}: {end}"
    assert isinstance(end["room"], int)
    assert re.fullmatch(r"[0-9a-f]{8}", end["vars_fnv1a"]), end


@pytest.mark.integration
def test_bridge_boots_and_stops_at_max_ticks(engine_ready, tmp_path, home_scummvm_guard):
    cfg, result, elapsed, log = _run_to_max_ticks(tmp_path / "run", max_ticks=1800, seed=1)
    print(f"1800-tick run: {elapsed:.2f}s wall, rc={result.returncode}")

    records = _read_trace(cfg.out_dir)
    boot, end = records[0], records[-1]

    assert boot["type"] == "boot", records
    assert boot["bridge"] == BRIDGE_VERSION
    assert boot["tick"] == 0 and boot["frame"] == 0
    assert boot["game"] == "monkey"
    assert boot["variant"] == "Mac"
    assert boot["fast"] is True
    assert boot["fast_boot"] is False  # SPEEDRUN_FAST_BOOT unset (C1, C5)
    # Proves the bridge sees --disable-sdl-audio (transient ConfMan domain) ...
    assert boot["audio_pump"] is True
    # ... and --random-seed (session domain).
    assert boot["seed"] == cfg.seed
    assert boot["boot_param"] == 0

    assert end["type"] == "end", records
    assert end["reason"] == "max_ticks"
    assert end["tick"] >= 1800
    assert 0 < end["frame"] <= end["tick"]
    _assert_fingerprint(end)
    # The pump drives the Mac player: its VBL callback advances the music timer.
    assert end["audio_frames"] > 0
    assert end["music_timer"] > 0
    assert [r["type"] for r in records if r["type"] != "room"] == ["boot", "end"], records

    assert result.returncode == 0, log[-4000:]
    assert "ERROR:" not in log, log[-4000:]
    assert elapsed < 20, f"1800 ticks took {elapsed:.1f}s of wall time"


@pytest.mark.integration
def test_ticks_deterministic_boot(engine_ready, tmp_path, home_scummvm_guard):
    ends = []
    for i in range(2):
        cfg, _result, _elapsed, _log = _run_to_max_ticks(tmp_path / f"run{i}", seed=1234)
        end = _read_trace(cfg.out_dir)[-1]
        assert end["type"] == "end" and end["reason"] == "max_ticks", end
        _assert_fingerprint(end)
        ends.append(end)
    assert ends[0] == ends[1]


@pytest.mark.integration
def test_trace_survives_kill(engine_ready, tmp_path, home_scummvm_guard):
    # A crash, SIGKILL or wall-clock kill must keep every record flushed so far
    # in $SPEEDRUN_OUT/trace.jsonl (C5), not lose it in an unrenamed temp file.
    # The engine is launched with run_engine's own isolated ini, argv and env,
    # then killed outright (SIGKILL) once the segment-start record is on disk.
    # Waiting for that record, not a fixed delay, keeps the test independent of
    # machine load.
    cfg = EngineConfig(out_dir=tmp_path / "run", fast=True, max_ticks=10**9)
    cfg.out_dir.mkdir(parents=True)
    engine.write_ini(cfg)
    trace = cfg.out_dir / "trace.jsonl"
    with (cfg.out_dir / "stdout.log").open("wb") as log:
        proc = subprocess.Popen(engine.build_argv(cfg), env=engine.build_env(cfg),
                                stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 120
            while time.monotonic() < deadline and proc.poll() is None:
                if trace.is_file() and '"segment_start"' in trace.read_text(errors="replace"):
                    break
                time.sleep(0.05)
        finally:
            proc.kill()
            proc.wait()
    assert proc.returncode < 0, proc.returncode  # killed, not exited

    records = _read_trace(cfg.out_dir)  # every line parses, the last one is complete
    types = [r["type"] for r in records]
    assert types[0] == "boot", types
    assert records[0]["bridge"] == BRIDGE_VERSION
    assert "end" not in types, types
    # The single-shot dumps are written whole: segment_start follows state-start.json.
    assert "segment_start" in types, types
    assert json.loads((cfg.out_dir / "state-start.json").read_text())["room"] == 33
    assert not list(cfg.out_dir.glob("*.tmp"))


@pytest.mark.integration
def test_bad_env_writes_error_and_end(engine_ready, tmp_path, home_scummvm_guard):
    # An invalid SPEEDRUN_* value goes through ScummVM's error(), so this also
    # exercises the bridge's error handler: both records (properly escaped),
    # then exit instead of the GUI debugger, which hangs under dummy video.
    cfg = EngineConfig(out_dir=tmp_path / "run", fast=True)
    write_ini(cfg)
    env = build_env(cfg)
    bad = 'x"y\\z\x01é'
    env["SPEEDRUN_MAX_TICKS"] = bad
    with (cfg.out_dir / "stdout.log").open("wb") as log:
        proc = subprocess.run(
            build_argv(cfg), stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
            env=env, timeout=20,
        )
    assert proc.returncode != 0

    records = _read_trace(cfg.out_dir)
    assert [r["type"] for r in records] == ["error", "end"], records
    err, end = records
    assert err["code"] == "bad_env"
    assert "SPEEDRUN_MAX_TICKS" in err["message"]
    # Bytes >= 0x80 are escaped one by one (Latin-1), so UTF-8 "é" reads back as "Ã©".
    assert bad.encode().decode("latin-1") in err["message"]
    assert end["reason"] == "error"
    assert (err["tick"], err["frame"]) == (end["tick"], end["frame"]) == (0, 0)
    # The env is checked first thing in init(), before the game's vars exist:
    # the fingerprint fields are present, the var-derived ones null.
    assert set(END_FINGERPRINT) <= end.keys(), end
    assert end["vars_fnv1a"] is None and end["music_timer"] is None, end
    assert end["audio_frames"] == 0


@pytest.mark.integration
def test_bridge_inert_without_out(engine_ready, tmp_path, home_scummvm_guard):
    # run_engine always sets SPEEDRUN_OUT, so launch by hand with the same argv
    # and env minus SPEEDRUN_OUT (every other SPEEDRUN_* var stays set).
    cfg = EngineConfig(out_dir=tmp_path / "run", fast=True, max_ticks=60)
    write_ini(cfg)
    argv = build_argv(cfg)
    env = build_env(cfg)
    del env["SPEEDRUN_OUT"]
    assert env["SPEEDRUN_FAST"] == "1" and env["SPEEDRUN_MAX_TICKS"] == "60"

    log_path = cfg.out_dir / "stdout.log"
    timed_out = False
    with log_path.open("wb") as log:
        proc = subprocess.Popen(
            argv, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, env=env
        )
        try:
            proc.wait(timeout=15)
        except subprocess.TimeoutExpired:
            timed_out = True
        finally:
            if proc.poll() is None:
                proc.kill()
            proc.wait()

    log = log_path.read_text(errors="replace")
    # Inert bridge: no watchdog, so the game keeps running until killed.
    assert timed_out, f"ScummVM exited early (rc={proc.returncode}):\n{log[-4000:]}"
    assert not (cfg.out_dir / "trace.jsonl").exists()
    # xdg/ is the run's own XDG data/cache prefix, created by engine.write_ini (not the bridge).
    assert sorted(p.name for p in cfg.out_dir.iterdir()) == ["scummvm.ini", "stdout.log", "xdg"]
    assert "ERROR:" not in log, log[-4000:]


@pytest.mark.integration
@pytest.mark.parametrize(
    ("name", "value", "fragment"),
    [
        ("SPEEDRUN_SKIP_TEXT", "maybe", "SPEEDRUN_SKIP_TEXT"),
        ("SPEEDRUN_SKIP_CUTSCENES", "2", "SPEEDRUN_SKIP_CUTSCENES"),
        ("SPEEDRUN_STEP_STATES", "x", "SPEEDRUN_STEP_STATES"),
        ("SPEEDRUN_INTERRUPTS", '{"name": "p"}', "JSON array"),
        ("SPEEDRUN_INTERRUPTS", '[{"name": "p", "when": [], "choose": ["x"]}]', "'when' must not be empty"),
        ("SPEEDRUN_INTERRUPTS", '[{"name": "p", "when": [{"room": 49}], "choose": []}]', "'choose' must not be empty"),
        ("SPEEDRUN_INTERRUPTS", '[{"name": "p", "when": [{"room": 49}], "choose": ["x"], "cite": "c"}]',
         "exactly the keys"),
        ("SPEEDRUN_INTERRUPTS", '[{"name": "p", "when": [{"var": 99999, "eq": 1}], "choose": ["x"]}]',
         "SPEEDRUN_INTERRUPTS p when"),  # range checked at boot
    ],
    ids=lambda v: v if len(v) < 30 else v[:30],
)
def test_bad_skip_and_interrupt_env(engine_ready, tmp_path, home_scummvm_guard, name, value, fragment):
    cfg = EngineConfig(out_dir=tmp_path / "run", fast=True, max_ticks=600)
    write_ini(cfg)
    env = build_env(cfg)
    env[name] = value
    with (cfg.out_dir / "stdout.log").open("wb") as log:
        proc = subprocess.run(build_argv(cfg), stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                              env=env, timeout=20)  # fmt: skip
    assert proc.returncode != 0
    records = _read_trace(cfg.out_dir)
    assert [r["type"] for r in records] == ["error", "end"], records
    assert records[0]["code"] == "bad_env" and fragment in records[0]["message"], records[0]
