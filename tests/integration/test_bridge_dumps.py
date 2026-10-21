"""Speedrun bridge, task 1.2: idle detection, C3 conditions, objects.json (C6),
segment start and state dumps (C7), goal."""

import json
import random
import subprocess
from pathlib import Path

import pytest

from speedrun import paths
from speedrun.engine import EngineConfig, build_argv, build_env, run_engine, write_ini

# Part I segment start: free control on the dock after the natural boot
# (docs/part1/start.md, "Start condition").
DOCK_START = [
    {"room": 33},
    {"var": 101, "eq": 96},
    {"bit": 395, "eq": 1},
    {"var": 196, "eq": 0},
    {"var": 39, "eq": 0},
]
# Boot to the first free control on the dock, measured with seed 1 and the
# pinned engine settings. Any change to it is a change in boot timing (or in
# the idle predicate) and should be looked at, not just re-pinned.
# It was 12865 with talkspeed 60 (VAR_CHARINC 7); Phase 8 pins the maximum,
# talkspeed 255 (VAR_CHARINC 0, docs/research/skips-engine.md section 5), so the
# intro's text lines end sooner.
BOOT_TO_DOCK_TICKS = 9325
# The segment test runs to this cap: a minute of game time past the dock.
DOCK_MAX_TICKS = BOOT_TO_DOCK_TICKS + 3600
TIMEOUT_S = 120


def _read_trace(out_dir: Path) -> list[dict]:
    trace = out_dir / "trace.jsonl"
    assert trace.exists(), f"no trace written to {trace}"
    text = trace.read_text(encoding="ascii")
    assert text.endswith("\n")
    return [json.loads(line) for line in text.splitlines()]


def _run(cfg: EngineConfig) -> list[dict]:
    result = run_engine(cfg)
    log = result.log_path.read_text(errors="replace")
    assert not result.timed_out, f"engine did not stop by itself:\n{log[-4000:]}"
    return _read_trace(cfg.out_dir)


def _of_type(records: list[dict], kind: str) -> list[dict]:
    return [r for r in records if r["type"] == kind]


@pytest.mark.integration
def test_object_dump(engine_ready, tmp_path, home_scummvm_guard):
    cfg = EngineConfig(out_dir=tmp_path / "run", dump_objects=True, max_ticks=DOCK_MAX_TICKS, timeout_s=TIMEOUT_S)
    records = _run(cfg)
    end = records[-1]
    assert end["type"] == "end" and end["reason"] == "dump_done", records[-3:]

    dump = json.loads((cfg.out_dir / "objects.json").read_text(encoding="ascii"))
    assert dump["game"] == {"gameid": "monkey", "variant": "Mac", "version": 5}
    assert isinstance(dump["ego"], int)

    verbs = {v["id"]: v for v in dump["verbs"]}
    for verb_id, name in ((2, "Open"), (9, "Pick up"), (11, "Walk to"), (10, "Talk to"), (4, "Give")):
        assert verbs[verb_id]["name"] == name, verbs.get(verb_id)
    assert all({"slot", "id", "name", "mode"} <= v.keys() for v in dump["verbs"])
    # Image verbs (inventory slots) have no text; names are always strings (C6).
    assert all(isinstance(v["name"], str) for v in dump["verbs"])
    assert all(v["name"] == "" for v in dump["verbs"] if v["type"] == 1)

    rooms = {r["room"]: r for r in dump["rooms"]}
    assert len(rooms) == len(dump["rooms"]) >= 80
    for room in dump["rooms"]:
        ids = [o["id"] for o in room["objects"]]
        assert len(ids) == len(set(ids)), f"duplicate object ids in room {room['room']}"
        for obj in room["objects"]:
            assert {"id", "name", "owner", "state", "classes", "walk", "x", "y", "w", "h", "parent"} <= obj.keys(), obj
            assert "@" not in obj["name"]
    assert 362 in {o["id"] for o in rooms[28]["objects"]}

    index_path = paths.DATA_DIR / "scripts" / "index.json"
    if not index_path.exists():
        return  # names are cross-checked only when data/scripts was generated
    index = json.loads(index_path.read_text())
    named = sorted(
        (int(room), int(obj), meta["name"])
        for room, rmeta in index["rooms"].items()
        for obj, meta in rmeta["objects"].items()
        if meta["name"]
    )
    for room, obj, name in random.Random(0).sample(named, 20):
        ours = {o["id"]: o["name"] for o in rooms[room]["objects"]}
        # index.json keeps the raw bytes (Latin-1) including '@' padding.
        assert ours[obj] == name.replace("@", ""), (room, obj, ours.get(obj), name)


@pytest.mark.integration
def test_segment_start_dump(engine_ready, tmp_path, home_scummvm_guard):
    cfg = EngineConfig(out_dir=tmp_path / "run", start=DOCK_START, max_ticks=DOCK_MAX_TICKS, timeout_s=TIMEOUT_S)
    records = _run(cfg)
    starts = _of_type(records, "segment_start")
    assert len(starts) == 1, records[-5:]
    start = starts[0]
    print(f"boot -> dock segment start: tick0={start['tick0']} frame={start['frame']}")
    assert start["tick0"] == start["tick"] == BOOT_TO_DOCK_TICKS
    assert start["room"] == 33
    assert records[-1]["type"] == "end" and records[-1]["reason"] == "max_ticks"

    # Boot cutscenes keep input off for minutes: stall diagnostics, at most one
    # per 3600 ticks of not being idle, stamped before the frame's delta (C2).
    stalls = _of_type(records, "stall")
    assert stalls, records[:5]
    assert all(records.index(r) < records.index(start) for r in stalls)
    assert all(r["reason"] == "userput" and r["not_idle_ticks"] >= 3600 for r in stalls), stalls
    ticks = [r["tick"] for r in stalls]
    assert all(b - a >= 3600 for a, b in zip(ticks, ticks[1:])), ticks

    state = json.loads((cfg.out_dir / "state-start.json").read_text(encoding="ascii"))
    assert state["tick"] == start["tick"] and state["frame"] == start["frame"]
    assert state["room"] == 33
    assert state["seed"] == cfg.seed and state["boot_param"] == 0
    assert isinstance(state["ego"], int) and state["ego"] > 0
    assert len(state["ego_pos"]) == 2
    assert len(state["vars"]) >= 800
    assert state["vars"][101] == 96 and state["vars"][196] == 0 and state["vars"][39] == 0
    assert 395 in state["bits_set"]
    assert isinstance(state["inventory"], list)
    assert state["owners"] and state["states"]
    assert all(isinstance(k, str) and isinstance(v, int) for k, v in state["owners"].items())


@pytest.mark.integration
def test_goal_trivial(engine_ready, tmp_path, home_scummvm_guard):
    cfg = EngineConfig(
        out_dir=tmp_path / "run", start=DOCK_START, goal=[{"room": 33}],
        max_ticks=DOCK_MAX_TICKS, timeout_s=TIMEOUT_S,
    )
    records = _run(cfg)
    (start,) = _of_type(records, "segment_start")
    (goal,) = _of_type(records, "goal")
    end = records[-1]
    assert end["type"] == "end" and end["reason"] == "goal", records[-3:]
    assert goal["tick"] >= start["tick0"]
    assert goal["ticks_from_start"] == goal["tick"] - start["tick0"]
    assert records.index(start) < records.index(goal) < len(records) - 1
    state_end = json.loads((cfg.out_dir / "state-end.json").read_text(encoding="ascii"))
    assert state_end["room"] == 33
    assert state_end["tick"] == goal["tick"]


@pytest.mark.integration
def test_goal_all_condition_kinds(engine_ready, tmp_path, home_scummvm_guard):
    # Every C3 kind, true at dock control (ego = actor 1 at (346, 133); the
    # cliffside 426 lies in room 33 with state 0; the inventory is empty).
    # Each kind also appears negated around a false form, so an evaluator
    # stuck at true or at false keeps the goal from firing.
    goal = [
        {"room": 33}, {"not": {"room": 10}}, {"not": {"not": {"room": 33}}},
        {"var": 101, "eq": 96}, {"not": {"var": 101, "eq": 95}},
        {"bit": 395, "eq": 1}, {"not": {"bit": 394, "eq": 1}},
        {"owner": 426, "eq": 15}, {"not": {"owner": 426, "eq": 1}},
        {"state": 426, "eq": 0}, {"not": {"state": 426, "eq": 1}},
        {"not": {"has": 362}},
        {"actor_room": 1, "eq": 33}, {"not": {"actor_room": 1, "eq": 10}},
        {"actor_x": 1, "ge": 340}, {"actor_x": 1, "le": 350}, {"actor_x": 1, "eq": 346},
        {"not": {"actor_x": 1, "le": 340}},
        {"actor_y": 1, "eq": 133}, {"not": {"actor_y": 1, "ge": 134}},
    ]
    cfg = EngineConfig(
        out_dir=tmp_path / "run", start=DOCK_START, goal=goal, max_ticks=DOCK_MAX_TICKS, timeout_s=TIMEOUT_S,
    )
    records = _run(cfg)
    assert records[-1]["reason"] == "goal", records[-3:]
    (goal_rec,) = _of_type(records, "goal")
    assert goal_rec["ticks_from_start"] == 0


@pytest.mark.integration
def test_dump_not_reached_is_an_error(engine_ready, tmp_path, home_scummvm_guard):
    cfg = EngineConfig(out_dir=tmp_path / "run", dump_objects=True, max_ticks=600, timeout_s=TIMEOUT_S)
    records = [r for r in _run(cfg) if r["type"] != "room"]  # room changes are logged from boot on
    assert [r["type"] for r in records] == ["boot", "error", "end"], records
    assert records[1]["code"] == "dump_not_reached"
    assert records[2]["reason"] == "error" and records[2]["tick"] >= 600
    assert not (cfg.out_dir / "objects.json").exists()


@pytest.mark.integration
def test_bad_condition_is_bad_env(engine_ready, tmp_path, home_scummvm_guard):
    cfg = EngineConfig(out_dir=tmp_path / "run", start=[{"bogus": 1}])
    write_ini(cfg)
    with (cfg.out_dir / "stdout.log").open("wb") as log:
        proc = subprocess.run(
            build_argv(cfg), stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
            env=build_env(cfg), timeout=20,
        )
    assert proc.returncode != 0
    records = _read_trace(cfg.out_dir)
    assert [r["type"] for r in records] == ["error", "end"], records
    assert records[0]["code"] == "bad_env"
    assert "SPEEDRUN_START" in records[0]["message"]
    assert records[1]["reason"] == "error"
