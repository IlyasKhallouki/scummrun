"""Tests of E009's fixes to the explorer (experiments/E009-explore-fixed/plan.md, "Method"), each behind its switch:
(a) idle keys measured from the engine, (b) target rotation, (c) readings kept (eviction level from the sizes) and
reused on the keys K, (d) the stop rules (non-idle stagnation, the prediction's falsifier).

usage (from the repo root): .venv/bin/python -m pytest experiments/E008-go-explore/test_e009.py
       E008_ENGINE=1 (with E008's environment) also runs the idle measurement from the boot in the engine.

E008's own tests (test_explore.py, test_step.py, test_guide.py) are the off check: they run with every switch off.
The fixtures name MI1's numbers (tests may, C2); step.py and explore.py do not.
"""

import json
import os
import sys
import types
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import step  # noqa: E402

GAME_FILES = (HERE.parents[1] / "game").exists()
DOOR_START = Path.home() / ".cache/fast-mi-e008/archives/door/readings/28-33-35-38-j00000/start.jsonl"


# --- (a) idle keys: the rule ----------------------------------------------------------------------------------------

def test_idle_rule_a_value_coming_back_is_idle_a_one_shot_change_is_not():
    start = {"var:100": 5, "room": 33, "var:2": 0, "state:316": 0}
    frames = [(1, {"var:100": 6, "room": 33, "var:2": 1, "state:316": 0}),
              (2, {"var:100": 7, "room": 28, "var:2": 2, "state:316": 1}),
              (3, {"var:100": 5, "room": 28, "var:2": 3, "state:316": 1}),   # var 100 back to its start value
              (4, {"var:100": 6, "room": 28, "var:2": 3, "state:316": 0})]   # the door back to 0
    r = step.idle_rule(start, frames)
    assert r["returns"] == ["state:316", "var:100"]
    assert r["first_return"] == {"var:100": 3, "state:316": 4}
    assert r["changes"] == {"var:100": 4, "room": 1, "var:2": 3, "state:316": 2}


def test_idle_rule_absent_is_zero_and_a_key_that_appears_and_goes_returns():
    r = step.idle_rule({"var:1": 0}, [(1, {"var:1": 0, "bit:9": 1}), (2, {"var:1": 0})])
    assert r["returns"] == ["bit:9"] and r["first_return"] == {"bit:9": 2}


class _Branch:
    def __init__(self, frame, seq):
        self.frame, self.seq, self.pid, self.quit_called = frame, seq, os.getpid(), False
        self.ran = []

    def run(self, frame):
        self.ran.append(frame)
        self.frame = frame

    def state(self):
        return self.seq.get(self.frame, self.seq["start"])

    def quit(self):
        self.quit_called = True


def test_measure_idle_runs_a_fork_frame_by_frame_and_quits_it(monkeypatch):
    seq = {"start": {"var:100": 1}, 2729: {"var:100": 2}, 2730: {"var:100": 1}, 2731: {"var:100": 3}}
    b = _Branch(2728, seq)
    ctl = types.SimpleNamespace(fork=lambda root: b)
    monkeypatch.setattr(step, "facts_of", lambda s: dict(s))
    r = step.measure_idle(ctl, object(), 3)
    assert b.ran == [2729, 2730, 2731] and b.quit_called
    assert r["returns"] == ["var:100"] and r["first_return"] == {"var:100": 2}
    assert r["frames"] == 3 and r["frame0"] == 2728 and r["frame_end"] == 2731


# --- (b) rotation ---------------------------------------------------------------------------------------------------

def test_rotate_least_tried_in_the_cell_then_in_all_then_depth_then_e008s_order():
    ts = [("state:316", 1, 0, 0), ("bit:9", 1, 1, 1), ("bit:1", 1, 1, 2), ("room", 28, 1, 2), ("bit:5", 1, 2, 2)]
    rot = {"cell": [["state:316", 1, 1], ["bit:1", 1, 1]], "all": [["bit:9", 1, 3], ["state:316", 1, 1],
                                                                    ["bit:1", 1, 4]]}
    assert [t[:2] for t in step.rotate(ts, rot)] == [("room", 28), ("bit:5", 1), ("bit:9", 1), ("state:316", 1),
                                                    ("bit:1", 1)]  # room (tier 1) before bit:5 (tier 2)
    # no counts: the goal first, then depth, then E008's order (tier before key)
    assert [t[:2] for t in step.rotate(ts, {})] == [("state:316", 1), ("bit:9", 1), ("bit:1", 1), ("room", 28),
                                                   ("bit:5", 1)]


def test_targets_with_rotation_are_e008s_targets_reordered(tmp_path):
    from test_step import ENTRY, GOAL, GUIDE, REACHED
    p = tmp_path / "reading.reached"
    p.write_text(REACHED)
    plain = step.targets(p, GUIDE, ENTRY, held=[["state:316", 1]], tried=[["room", 41]], goal=GOAL)
    rot = {"cell": [["owner:566", 1, 2]], "all": [["owner:566", 1, 2], ["bit:6", 1, 1]]}
    turned = step.targets(p, GUIDE, ENTRY, held=[["state:316", 1]], tried=[["room", 41]], goal=GOAL, rotation=rot)
    assert sorted(plain) == sorted(turned) and turned != plain
    assert turned[-1][:2] == ("owner:566", 1)  # the goal, tried twice from this cell, comes last
    assert ("room", 41) not in [t[:2] for t in turned]  # the entry's own tried targets stay out (E008's rule)


# --- (c) reuse ------------------------------------------------------------------------------------------------------

@pytest.mark.skipif(not (GAME_FILES and DOOR_START.exists()), reason="needs the game files and E008's door reading")
def test_reuse_keys_on_the_doors_ring_33_start():
    start = json.loads(DOOR_START.read_text().splitlines()[0])["facts"]
    d = step.reuse_keys_of([28, 33, 35, 38], start)
    k = set(d["keys"])
    # E008's ring(33) refusals: var 0, 2, 7, 20, 21, 44, 45, 100, 181, 183 (plan.md baseline 4)
    assert {f"var:{n}" for n in (0, 2, 7, 20, 21, 44, 45, 100, 181, 183)} <= k
    assert "var:229" not in k  # a counter: its closure looks at other variables' domains
    for n in (19, 24, 32, 37, 39, 4, 22, 5, 38, 52, 53, 27, 36):  # named by the reader's tables, or a guard
        assert f"var:{n}" not in k, n
    assert set(d["guards"]) <= set(d["named"]) | set(range(0, 1000))
    assert not any(not x.startswith("var:") for x in k)


def test_reuse_keys_cached_next_to_the_reading_without_its_fact_vars(tmp_path, monkeypatch):
    calls = []

    def fake(rooms, start, game=None):
        calls.append(1)
        return {"keys": ["var:100", "var:103", "var:20"], "named": [], "guards": [], "s": 0.0}
    monkeypatch.setattr(step, "reuse_keys_of", fake)
    rd = types.SimpleNamespace(path=tmp_path / "reading.json", doc={
        "scope": {"rooms": [33]}, "start": {"name": "/x/start.jsonl seed 1", "facts": {}}, "fact_vars": [103]})
    assert step.reuse_keys(rd) == {"var:100", "var:20"}
    assert step.reuse_keys(rd) == {"var:100", "var:20"} and len(calls) == 1
    assert json.loads((tmp_path / step.REUSE_KEYS_FILE).read_text())["start"] == "/x/start.jsonl seed 1"


def _reading(tmp_path, start):
    return types.SimpleNamespace(path=tmp_path / "r" / "reading.json", doc={"start": {"facts": start, "name": "s"}},
                                 task=types.SimpleNamespace(facts={"var:103": None}), close=lambda: None)


def test_choose_reading_e009_reuses_on_k_and_refuses_other_keys(tmp_path, monkeypatch):
    start = {"var:100": 3, "var:103": 0, "var:229": 1, "room": 33}
    monkeypatch.setattr(step, "cached_readings", lambda job: [tmp_path / "r" / "reading.json"])
    monkeypatch.setattr(step.PR, "load_reading", lambda p: _reading(tmp_path, start))
    monkeypatch.setattr(step, "reuse_keys", lambda rd: {"var:100"})
    entry = dict(start, **{"var:100": 9, "var:103": 1})
    rd, why, checked = step.choose_reading({"scope": [33]}, entry)  # E008's rule
    assert rd is None and why == "differs on 1 keys" and "e009" not in checked[0]
    rd, why, checked = step.choose_reading({"scope": [33], "reuse": "e009"}, entry)
    assert rd is not None and why == "reused (e009 keys)" and checked[0]["reuse_keys"] == 1
    rd, why, checked = step.choose_reading({"scope": [33], "reuse": "e009"}, dict(entry, **{"var:229": 2}))
    assert rd is None and why == "differs on 1 keys" and checked[0]["e009"]["first"] == [["var:229", 1, 2]]


# --- run_job with the switches -----------------------------------------------------------------------------------------

def test_run_job_measures_idle_and_passes_the_rotation(tmp_path, monkeypatch):
    from test_step import stub_job
    order = []
    monkeypatch.setattr(step, "measure_idle", lambda ctl, root, n: order.append(("idle", n)) or {
        "returns": ["var:100"], "first_return": {"var:100": 7}, "changes": {"var:100": 9}, "frames": n,
        "seconds": 0.5, "cpu_s": 0.3, "frame0": 2728, "frame_end": 2728 + n})
    seen = {}
    real_targets = step.targets

    def targets(*a, **k):
        seen["rotation"] = a[7] if len(a) > 7 else k.get("rotation")
        return real_targets(*a, **k)
    monkeypatch.setattr(step, "targets", targets)
    orig = json.dumps

    def patch_job(jd):
        j = json.loads((jd / "job.json").read_text())
        j.update(idle_frames=1600, rotation={"cell": [["owner:566", 1, 1]], "all": []})
        (jd / "job.json").write_text(orig(j))
    real_run = step.run_job
    monkeypatch.setattr(step, "run_job", lambda jd: (patch_job(Path(jd)), real_run(jd))[1])
    r, calls, jd = stub_job(tmp_path, monkeypatch, ["plan"])
    assert r["ok"], r["error"]
    assert order == [("idle", 1600)] and r["idle"]["returns"] == ["var:100"]
    assert seen["rotation"] == {"cell": [["owner:566", 1, 1]], "all": []}
    assert calls["attempts"][0][0][:2] != ("owner:566", 1)  # the goal, tried from the cell, is not first
    assert "idle" in r["cpu_s"] and r["cpu_s"]["total"] >= r["cpu_s"]["idle"]


def test_run_job_skips_an_entry_measured_before(tmp_path, monkeypatch):
    from test_step import stub_job
    monkeypatch.setattr(step, "measure_idle", lambda *a: pytest.fail("measured again"))
    real_run = step.run_job

    def patch(jd):
        j = json.loads((Path(jd) / "job.json").read_text())
        j.update(idle_frames=1600, idle_measured=True)
        (Path(jd) / "job.json").write_text(json.dumps(j))
        return real_run(jd)
    monkeypatch.setattr(step, "run_job", patch)
    r, calls, jd = stub_job(tmp_path, monkeypatch, ["plan"])
    assert r["ok"] and "idle" not in r


# --- in the engine ----------------------------------------------------------------------------------------------------

@pytest.mark.skipif(os.environ.get("E008_ENGINE") != "1", reason="engine tests: E008_ENGINE=1")
def test_engine_idle_at_the_boot_is_var_100_alone(tmp_path):
    import replay_part1 as P
    ctl, b, row = P.boot(1, tmp_path / "boot", dump_locals=True)
    try:
        r = step.measure_idle(ctl, b, 1600)
    finally:
        ctl.close()
    keys = [k for k in r["returns"] if step.guide_kind(k)]
    assert keys == ["var:100"], keys  # plan.md baseline 1 (kitchen-0)
