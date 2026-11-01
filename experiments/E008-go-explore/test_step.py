"""Tests of step.py (plan.md, "One job"): the targets from a reading's reached values and the guide, the reuse rule, the
entries a played attempt gives, the start row and result.json's keys; with E008_ENGINE=1 also two jobs in the engine
on the kitchen's scope: (a) the off check from the root (E007's kitchen run), (b) a job from one of (a)'s entries,
then (a)'s and (b)'s segments verified from boot as one movie (Lemma C on a chain of two links).

usage (from the repo root): .venv/bin/python -m pytest experiments/E008-go-explore/test_step.py
       E008_ENGINE=1 with FAST_MI_SCUMMVM_BIN, DOWNWARD_DIR, TASKCORE_BIN, READER_SCOPE_BIN set (plan.md,
       "Interfaces"), under the guard and nice; outputs under ~/.cache/fast-mi-e008/step-tests (E008_STEP_OUT), the
       numbers in ~/.cache/fast-mi-e008/step-tests.txt.

The fixtures name MI1's numbers (tests may, C2); step.py does not.
"""

import gzip
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import step  # noqa: E402

PLAN_RESULT_KEYS = {"job", "entry", "ok", "error", "return", "reading", "targets", "probes", "attempt",
                    "readings_made", "new", "tried", "exhausted", "cpu_s", "peak_rss_mb", "wall_s"}
PLAN_READING_KEYS = {"path", "fresh", "why", "scope", "records", "actions", "read_wall_s", "read_cpu_s",
                     "peak_rss_mb", "files_s"}
PLAN_ATTEMPT_KEYS = {"target", "stop", "reached", "goal_from_the_plan", "frames", "qf0", "qf_end", "replans",
                     "refinements", "extensions", "window_summary", "seconds"}
PLAN_ENTRY_KEYS = {"id", "parent", "frame", "tick", "events", "checkpoints"}

REACHED = "\n".join([
    "room\t28\t33\t41",
    "owner:566\t1\t15",
    "state:316\t0\t1",
    "var:103\t0\t1\t2",
    "var:53\tbelow\t-1\t0\t1\t2\tabove",
    "pos:local:215\t0\t1",
    "at:local:203:30\t0\t1",
    "bit:6\t0",
    "class:426:32\t0\t1",
    "owner:566\t1",
]) + "\n"

GUIDE = {"format": "e008-guide-1", "goal": {"owner:566": [1]}, "map": "direct",
         "rooms": {"start": 33, "edges": {"33": [28, 35, 38], "28": [33, 41], "41": [28]}, "reached": [28, 33, 41]},
         "atoms": [["owner:566", 1, 0], ["bit:6", 1, 1], ["room", 41, 1], ["state:316", 1, 1],
                   ["class:426:32", 1, 2], ["var:103", None, 2], ["var:53", 2, 3], ["var:53", None, 4]],
         "keys": {}, "writers": {}, "stats": {}}

ENTRY = {"room": 33, "owner:566": 15, "state:316": 0, "var:103": 0, "var:53": 0}
GOAL = {"owner:566": [1]}


@pytest.fixture
def reached(tmp_path):
    p = tmp_path / "reading.reached"
    p.write_text(REACHED)
    return p


# --- targets ---------------------------------------------------------------------------------------------------------

def test_read_reached_keeps_ints_and_joins_repeated_keys(reached):
    r = step.read_reached(reached)
    assert r["var:53"] == [-1, 0, 1, 2]
    assert r["owner:566"] == [1, 15]
    assert r["room"] == [28, 33, 41]


def test_targets_tiers_and_order(reached):
    ts = step.targets(reached, GUIDE, ENTRY, None, held=[["state:316", 1]], tried=[["var:103", 2]], goal=GOAL)
    assert ts == [
        ("owner:566", 1, 0, 0),  # tier 0: the goal, reached and not held by the entry
        ("room", 41, 1, 1),  # tier 1 by (depth, key, value)
        ("class:426:32", 1, 1, 2),
        ("var:103", 1, 1, 2),  # (k, any) at depth 2; var:103 = 2 was tried
        ("var:53", 2, 1, 3),  # [k, 2] at 3 and [k, null] at 4: the least
        ("var:53", -1, 1, 4),
        ("var:53", 1, 1, 4),
        ("state:316", 1, 2, 1),  # tier 2: an entry of the archive holds it
    ]


def test_targets_leave_out_the_entrys_values_other_kinds_and_atoms_not_in_r(reached):
    ts = step.targets(reached, GUIDE, ENTRY, goal=GOAL)
    keys = {(k, v) for k, v, _, _ in ts}
    assert ("owner:566", 15) not in keys  # the entry's value
    assert ("room", 28) not in keys and ("room", 33) not in keys  # not in R / the entry's room
    assert not any(k.startswith(("pos:", "at:")) for k, _ in keys)  # not a guide kind
    assert ("bit:6", 0) not in keys  # the entry's value (absent is 0)
    assert sum(1 for k, v in keys if (k, v) == ("owner:566", 1)) == 1  # tier 0 only, not again in tier 1


def test_tier_0_needs_every_goal_value_reached_and_the_entry_not_holding_it(reached, tmp_path):
    held_goal = dict(ENTRY, **{"owner:566": 1})
    assert all(t[2] > 0 for t in step.targets(reached, GUIDE, held_goal, goal=GOAL))
    p = tmp_path / "other.reached"
    p.write_text("owner:566\t15\nroom\t33\t41\n")
    ts = step.targets(p, GUIDE, ENTRY, goal=GOAL)
    assert ts == [("room", 41, 1, 1)]
    assert step.targets(reached, GUIDE, ENTRY, goal=GOAL, tried=[["owner:566", 1]])[0] == ("room", 41, 1, 1)


def test_tier_0_of_a_goal_of_several_keys(reached):
    goal = {"owner:566": [1], "room": [41]}
    ts = step.targets(reached, GUIDE, ENTRY, goal=goal)
    assert ts[0] == ("goal", "owner:566=1,room=41", 0, 0)
    assert ("owner:566", 1, 1, 0) in ts and ("room", 41, 1, 1) in ts  # each key's atom a tier 1 target


def test_held_any_atom_and_root_values(reached):
    ts = step.targets(reached, GUIDE, ENTRY, held=[["var:103", None]], goal=GOAL)
    assert ("var:103", 1, 2, 2) in ts and ("var:103", 2, 2, 2) in ts
    # a value of a (k, any) key at its root value holds no atom; a concrete atom at the root's value still counts
    ts = step.targets(reached, GUIDE, dict(ENTRY, **{"var:103": 2}), {"var:103": 1, "var:53": 2}, goal=GOAL)
    keys = {(k, v) for k, v, _, _ in ts}
    assert ("var:103", 1) not in keys and ("var:103", 0) in keys
    assert ("var:53", 2) in keys


# --- reuse ------------------------------------------------------------------------------------------------------------

def test_reusable_only_task_facts_and_slot_keys_may_differ():
    doc = {"start": {"facts": {"var:1": 0, "var:2": 5, "room": 33, "bit:5": 1, "at:local:200:10": 1,
                               "script:3": 1}}}
    task = {"var:1", "room"}
    same = dict(doc["start"]["facts"], **{"var:1": 4, "room": 28, "local:201": 1, "objscript:9": 1,
                                         "esc:script:3:20": 1})
    del same["at:local:200:10"], same["script:3"]
    assert step.reusable(doc, task, same) == (True, 0)
    assert step.reusable(doc, task, dict(same, **{"var:2": 6})) == (False, 1)
    other = dict(same)
    del other["bit:5"]  # absent is 0
    assert step.reusable(doc, task, dict(other, **{"verb:7": 1})) == (False, 2)
    d = step.reusable_detail(doc, task, dict(same, **{"var:2": 6}))
    assert d["other"] == 1 and d["first"] == [["var:2", 5, 6]] and d["task"] == 2


# --- entries -----------------------------------------------------------------------------------------------------------

def cp(frame, tick, **facts):
    return (frame, dict(facts), {"tick": tick, "rng": 7, "actors": [[1, 2, 3]]})


def test_entries_from_a_movie_and_checkpoints():
    inputs = [{"frame": 2736, "click": [1, 2]}, {"frame": 2751, "click": [1, 2]}, {"frame": 2800, "key": 13},
              {"frame": 3000, "click": [5, 6]}]
    cps = [cp(2728, 100, room=33), cp(2736, 150), cp(2751, 240), cp(2751, 240), cp(2798, 500), cp(3216, 2900,
                                                                                                  **{"owner:566": 1})]
    es = step.entries_from(2728, inputs, list(reversed(cps)))
    assert [e["frame"] for e in es] == [2736, 2751, 2798, 3216]
    assert [e["tick"] for e in es] == [150, 240, 500, 2900]
    assert [len(e["events"]) for e in es] == [1, 2, 2, 4]
    assert [[c[0] for c in e["checkpoints"]] for e in es] == [[2736], [2736, 2751], [2736, 2751, 2798],
                                                             [2736, 2751, 2798, 3216]]
    assert [e["how"] for e in es] == ["checkpoint"] * 3 + ["end"]
    assert all(e["id"] is None for e in es)
    assert set(es[0]) - {"how"} == PLAN_ENTRY_KEYS
    assert es[-1]["checkpoints"][-1][1] == {"owner:566": 1}
    with pytest.raises(ValueError):
        step.entries_from(2736, inputs, cps)  # an input at the entry's frame: premise S
    assert step.entries_from(3216, [], cps) == []


def test_entry_files_round_trip(tmp_path):
    e = step.entries_from(10, [{"frame": 12, "key": 1}], [cp(12, 5, room=1)])[0]
    e.pop("how")
    step.write_entry(tmp_path / "new" / "0.json.gz", e)
    with gzip.open(tmp_path / "new" / "0.json.gz", "rt") as f:
        back = json.load(f)
    assert back == json.loads(json.dumps(e)) and back["checkpoints"][0][0] == 12


# --- the start row, the result -----------------------------------------------------------------------------------------

STATE = {"vars": [0] * 60, "bits_set": [6], "owners": {"566": 15}, "states": {"316": 0}, "classes": {}, "room": 33,
         "ego": 1, "slots": [], "cutscene": {"depth": 0, "levels": [{"override": None}]}, "verbs": [], "frame": 2728,
         "tick": 16321, "ego_pos": [346, 133], "seed": 1, "boot_param": 0, "actors": []}
STATUS = {"stop": "idle", "frame": 2728, "tick": 16321, "qf": 65284, "room": 33, "idle": "sentence", "rng": 3,
          "up": 1, "cs": 0, "ego": [346, 133]}


def test_start_row_has_start_part1s_fields_and_refuses_other_seeds():
    row = step.start_row(1, STATE, STATUS)
    assert row["seed"] == 1 and row["frame"] == 2728 and row["stop"]["qf"] == 65284
    assert row["dump"]["ego_pos"] == [346, 133]
    assert row["facts"]["owner:566"] == 15 and row["facts"]["bit:6"] == 1 and row["facts"]["room"] == 33
    for seed in (2, 31, 836557856):
        with pytest.raises(ValueError):
            step.start_row(seed, STATE, STATUS)


def test_result_keys_are_plan_mds():
    r = step.empty_result({"job": "j1", "entry": {"id": 4}})
    assert set(r) == PLAN_RESULT_KEYS and set(r["cpu_s"]) == {"return", "read", "plan", "play", "total"}
    assert set(r["targets"]["by_tier"]) == {"0", "1", "2"} and set(r["peak_rss_mb"]) == {"python", "children"}
    att = step.attempt_record(("room", 41, 1, 1), {"stop": "goal", "frames": 3}, 1.0)
    assert PLAN_ATTEMPT_KEYS <= set(att) and att["target"] == ["room", 41]

    class _Rd:
        path = Path("/x/reading.json")
        doc = {"scope": {"rooms": [28, 33]}, "n_records": 5, "n_actions": 4, "read_s": 2.0, "start": {"name": "s"},
               "reader_scope": {"read_wall_s": 1.5, "read_cpu_s": 3.0, "peak_rss_mb": 10}}
    rec = step.reading_record(_Rd(), True, "no cached reading", 2.5)
    assert PLAN_READING_KEYS <= set(rec) and rec["files_s"] == 0.5 and rec["read_cpu_s"] == 3.0
    assert step.reading_record(_Rd(), False, "reused")["read_cpu_s"] == 0.0


def test_first_plan_status_skips_the_reach_extensions_try():
    assert step.first_plan_status([{"status": "reach extension"}, {"status": "plan"}]) == "plan"
    assert step.first_plan_status([{"status": "unsolvable"}]) == "unsolvable"
    assert step.first_plan_status([]) is None


def test_exit_codes():
    assert step.exit_code({"ok": True}) == 0
    assert step.exit_code({"ok": False, "return": {"same": False}}) == 3
    assert step.exit_code({"ok": False, "return": None}) == 1


def test_a_job_of_another_seed_is_refused(tmp_path):
    job = {"job": "j0", "seed": 2, "stage": "t", "goal": GOAL, "guide": "/nowhere", "scope": [33],
           "entry": {"id": 0, "root": True, "frame": 2728, "events": [], "checkpoint": [2728, {}, {}]},
           "tried": [], "held": [], "readings": [], "reading_dir": str(tmp_path / "rd")}
    (tmp_path / "job.json").write_text(json.dumps(job))
    p = subprocess.run([sys.executable, str(HERE / "step.py"), "run", str(tmp_path)], capture_output=True, text=True)
    assert p.returncode == 1
    r = json.loads((tmp_path / "result.json").read_text())
    assert r["ok"] is False and "seed 2" in r["error"] and set(r) == PLAN_RESULT_KEYS
    assert (tmp_path / "log").exists()


def test_cached_readings_newest_first_and_same_scope(tmp_path):
    paths = []
    for i, rooms in enumerate(([28, 33, 41], [28, 33], [41, 33, 28])):
        d = tmp_path / f"r{i}"
        d.mkdir()
        (d / "reading.json").write_text(json.dumps({"scope": {"rooms": rooms}, "packed": "reading.tc"}))
        (d / "reading.tc").write_text("")
        (d / "reading.reached").write_text("")
        os.utime(d / "reading.json", (1000 + i, 1000 + i))
        paths.append(d / "reading.json")
    (tmp_path / "r3").mkdir()  # files gone
    (tmp_path / "r3" / "reading.json").write_text(json.dumps({"scope": {"rooms": [28, 33, 41]}, "packed": "x.tc"}))
    job = {"scope": [33, 28, 41], "readings": [{"path": str(p)} for p in paths + [tmp_path / "r3" / "reading.json"]]}
    assert step.cached_readings(job) == [paths[2], paths[0]]


# --- in the engine (E008_ENGINE=1) --------------------------------------------------------------------------------------

ENGINE = os.environ.get("E008_ENGINE") == "1"
needs_engine = pytest.mark.skipif(not ENGINE, reason="engine tests: E008_ENGINE=1")
OUT = Path(os.environ.get("E008_STEP_OUT") or Path.home() / ".cache" / "fast-mi-e008" / "step-tests")
NUMBERS = Path.home() / ".cache" / "fast-mi-e008" / "step-tests.txt"
SCOPE = [28, 33, 41]


def run_cli(job_dir: Path) -> tuple:
    t = time.monotonic()
    p = subprocess.run([sys.executable, str(HERE / "step.py"), "run", str(job_dir)], capture_output=True, text=True)
    (job_dir / "cli.err").write_text(p.stderr[-20000:])
    return p.returncode, json.loads((job_dir / "result.json").read_text()), time.monotonic() - t


def job_doc(name: str, entry: dict, readings: list) -> dict:
    return {"job": name, "seed": 1, "stage": "kitchen", "goal": GOAL, "guide": str(OUT / "GUIDE.json"),
            "entry": entry, "scope": SCOPE, "tried": [], "held": [], "readings": readings,
            "reading_dir": str(OUT / "readings" / f"28-33-41-{name}"), "threads": 3, "workers": 3}


def numbers(tag: str, r: dict, wall: float) -> list:
    a, rd = r.get("attempt") or {}, r.get("reading") or {}
    ws = a.get("window_summary") or {}
    return [f"[{tag}] job {r['job']} from entry {r['entry']}: ok {r['ok']}, error {r['error']}, CLI wall {wall:.1f} s, "
            f"job wall {r['wall_s']} s",
            f"  return: same {r['return'] and r['return']['same']}, boot {r['return'] and r['return']['boot_s']} s",
            f"  reading: fresh {rd.get('fresh')}, why {rd.get('why')!r}, scope {rd.get('scope')}, records "
            f"{rd.get('records')}, actions {rd.get('actions')}, read wall {rd.get('read_wall_s')} s, read CPU "
            f"{rd.get('read_cpu_s')} s, reader peak {rd.get('peak_rss_mb')} MB, files {rd.get('files_s')} s, load "
            f"{rd.get('load_s')} s; candidates {[(c['reusable'], c['other'], c['task'], c['slots']) for c in rd.get('candidates') or ()]}",
            f"  targets {r['targets']['by_tier']}, first {r['targets']['first'][:3]}; probes "
            f"{[(p['target'], p['status'], p['seconds']) for p in r['probes']]}",
            f"  attempt: target {a.get('target')}, stop {a.get('stop')!r}, reached {a.get('reached')}, from the plan "
            f"{a.get('goal_from_the_plan')}, frames {a.get('frames')} ({a.get('frame0')} -> {a.get('frame_end')}), qf "
            f"{a.get('qf')} ({a.get('qf0')} -> {a.get('qf_end')}), replans {a.get('replans')}, refinements "
            f"{a.get('refinements')}, extensions {a.get('extensions')}, inputs {a.get('inputs')}, {a.get('seconds')} s",
            f"  window search: {ws.get('windows')} windows, {ws.get('children')} children, engine CPU "
            f"{ws.get('engine_cpu_s')}, Python CPU {ws.get('python_cpu_s')}",
            f"  new entries {len(r['new'])}: {[(n['frame'], n['how']) for n in r['new']]}; readings made "
            f"{r['readings_made']}",
            f"  CPU s {r['cpu_s']}; peak RSS MB {r['peak_rss_mb']}"]


@pytest.fixture(scope="module")
def job_a():
    import replay
    import replay_part1 as P
    from facts import facts_of
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)
    (OUT / "GUIDE.json").write_text(json.dumps(dict(GUIDE, atoms=[["owner:566", 1, 0]], keys={"owner:566": 0})))
    ctl, b, row = P.boot(1, OUT / "root-boot", dump_locals=True)
    try:
        s = b.state()
        root_cp = [b.frame, facts_of(s), replay.extra_of(s, b.status)]
    finally:
        ctl.close()
    jdir = OUT / "jobs" / "ta"
    jdir.mkdir(parents=True)
    entry = {"id": 0, "root": True, "frame": root_cp[0], "events": [], "checkpoint": root_cp}
    (jdir / "job.json").write_text(json.dumps(job_doc("ta", entry, [])))
    rc, r, wall = run_cli(jdir)
    NUMBERS.write_text("\n".join(["E008 step.py engine tests (test_step.py), "
                                  f"{time.strftime('%Y-%m-%d %H:%M')}"] + numbers("a", r, wall)) + "\n")
    return {"rc": rc, "r": r, "dir": jdir, "root_cp": root_cp}


@needs_engine
def test_engine_a_the_off_check_from_the_root(job_a):
    r = job_a["r"]
    assert job_a["rc"] == 0, r["error"]
    assert r["ok"] and r["return"]["same"], r["return"]
    assert r["reading"]["fresh"] and r["reading"]["why"] == "no cached reading"
    assert r["targets"]["first"][0][:3] == ["owner:566", 1, 0]
    a = r["attempt"]
    assert a["stop"] == "goal" and a["goal_from_the_plan"] and a["reached"]
    assert (a["frame0"], a["frame_end"], a["frames"], a["replans"]) == (2728, 3216, 488, 2)  # E007's kitchen run
    assert len(r["new"]) >= 5 and r["new"][-1]["frame"] == 3216 and r["new"][-1]["how"] == "end"
    last = step.read_entry(job_a["dir"] / r["new"][-1]["file"])
    assert last["checkpoints"][-1][1].get("owner:566") == 1 and last["parent"] == 0 and last["id"] is None
    assert set(last) == PLAN_ENTRY_KEYS
    assert all(e["frame"] > 2728 for e in last["events"])
    assert r["tried"] == [["owner:566", 1]] and r["exhausted"] is False


@needs_engine
def test_engine_b_a_job_from_an_entry_and_the_chain_from_boot(job_a):
    import replay
    import replay_part1 as P
    ra = job_a["r"]
    assert ra["ok"]
    mids = [n for n in ra["new"] if 2798 < n["frame"] < 3216] or [n for n in ra["new"] if n["frame"] < 3216]
    ea = step.read_entry(job_a["dir"] / mids[0]["file"])
    jdir = OUT / "jobs" / "tb"
    jdir.mkdir(parents=True)
    entry = {"id": 1, "root": False, "frame": ea["frame"], "events": ea["events"], "checkpoint": ea["checkpoints"][-1]}
    (jdir / "job.json").write_text(json.dumps(job_doc("tb", entry, [{"path": ra["reading"]["path"],
                                                                      "start_entry": 0}])))
    rc, r, wall = run_cli(jdir)
    lines = numbers("b", r, wall)
    try:
        assert rc == 0, r["error"]
        assert r["ok"] and r["return"]["same"], r["return"]
        assert r["reading"]["why"] == "reused" or r["reading"]["fresh"]
        assert r["reading"]["candidates"] and r["reading"]["candidates"][0]["path"] == ra["reading"]["path"]
        a = r["attempt"]
        assert a["stop"] == "goal" and a["reached"]
        eb = step.read_entry(jdir / r["new"][-1]["file"])
        assert eb["parent"] == 1 and eb["checkpoints"][-1][1].get("owner:566") == 1
        assert all(e["frame"] > ea["frame"] for e in eb["events"])
        t = time.monotonic()
        ver = replay.verify(lambda o, i: P.config(1, o, i), ea["events"] + eb["events"],
                            [job_a["root_cp"]] + ea["checkpoints"] + eb["checkpoints"], ["owner:566"], OUT / "verify-ab")
        lines.append(f"[b] verify from boot of (a)'s entry at {ea['frame']} then (b)'s segment to {eb['frame']}: "
                     f"{len(ver['differ'])} differences at {ver['checkpoints']} checkpoints, {ver['inputs']} inputs, "
                     f"final {ver.get('final')}, {time.monotonic() - t:.1f} s")
        assert ver["differ"] == [] and ver["final"] == {"owner:566": 1}
    finally:
        with open(NUMBERS, "a") as f:
            f.write("\n".join(lines) + "\n")
