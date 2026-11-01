"""Tests of the harness's reach extension (replay.py, "A reading with --reach and a replan start it does not cover";
replay_part1.py ``--reach-extend``): taskcore's refusal of a start a ``--reach`` reading did not reach is read back, the
switch off stops the run as before, the switch on reads again from that start and plans, and the reader's command
carries every start so far.

usage: pytest test_reach_extend.py   (taskcore's release binary; the reader-scope case needs scumm's release binary)
"""

import collections
import json
import subprocess
import sys
import types
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import plan_fd  # noqa: E402
import replay as RP  # noqa: E402
import replay_part1 as P1  # noqa: E402
import test_replay_taskcore as RT  # noqa: E402
from export_records import export_task, record_naming  # noqa: E402

needs_taskcore = pytest.mark.skipif(not plan_fd.TASKCORE.exists(), reason="no taskcore binary")

SAS_MSG = ('taskcore: the start holds 2 values the reading did not reach: ["room=28", "bit:6=0"]; extend the reading '
           '(reader-scope --reach-also)')
SERVE_MSG = 'taskcore: the start holds 1 values the reading did not reach: ["pos:local:202=41"]; extend the reading'


def test_the_refusal_is_read_back_from_both_messages():
    assert RP.reach_misses(SAS_MSG) == (2, ["room=28", "bit:6=0"])
    assert RP.reach_misses(SERVE_MSG) == (1, ["pos:local:202=41"])
    assert RP.reach_misses('taskcore: 1 facts out of their domains: ["room"]') is None
    assert RP.reach_misses("taskcore serve stopped (exit 1): no such file") is None


def test_the_start_for_the_reader_keeps_every_int_value():
    start = {"at:local:202:41": 1, "pos:local:202": 41, "args:local:202": 116169035371989, "room": 33,
             "var:100": 191, "x": "above", "flag": True}
    assert RP.reach_start(start) == {"args:local:202": 116169035371989, "at:local:202:41": 1, "pos:local:202": 41,
                                     "room": 33, "var:100": 191}


def _reached(path: Path, facts: dict, drop=()) -> Path:
    """A reached-values file (taskcore bans::reached_misses): every value of every fact but ``drop``'s."""
    path.write_text("".join("\t".join([k] + [str(v) for v in d.values if (k, v) not in drop]) + "\n"
                            for k, d in sorted(facts.items())))
    return path


def _harness(tmp_path, serve: bool, extend=None, drop=(("room", 28),)):
    """test_replay_taskcore's harness task as a packed file whose reached values lack ``drop`` (the fake engine is in
    room 28), so the first start is refused."""
    b, recs, task = RT._harness_task()
    export_task(tmp_path / "t.tc", task, {}, meta=lambda a: (a.what.kind, a.what.end, ""), naming=record_naming)
    _reached(tmp_path / "t.reached", task.facts, drop)
    h = RP.Harness(b, task, task, RT._fake_branch(), {"state:316": {1}}, packed=tmp_path / "t.tc",
                   workdir=tmp_path / f"plans-{serve}", taskcore_serve=serve, reach_extend=extend)
    h.observe()
    return h, b, task


@needs_taskcore
@pytest.mark.parametrize("serve", [False, True])
def test_off_a_start_the_reading_did_not_reach_stops_the_run(tmp_path, serve):
    h, _, _ = _harness(tmp_path, serve)
    assert h.plan(first=True) == []
    assert h.plans[0]["status"] == "taskcore error" and "did not reach" in h.plans[0]["error"]
    assert h.stop_reason.startswith("planner: ") and "room=28" in h.stop_reason
    assert "extensions" not in h.result()
    plan_fd.close_servers()


@needs_taskcore
@pytest.mark.parametrize("serve", [False, True])
def test_on_the_reading_is_extended_from_that_start_and_the_plan_made(tmp_path, serve):
    calls = []

    def extend(start, hh):
        calls.append(dict(start))
        n = len(calls)
        d = tmp_path / f"reach-{n:02d}"
        d.mkdir()
        packed = d / "reading.tc"
        packed.write_bytes((tmp_path / "t.tc").read_bytes())
        _reached(d / "reading.reached", task.facts)  # the closure from both starts reaches room 28
        return types.SimpleNamespace(task=task, packed=packed, b=b, lists=None, path=d / "reading.json")

    h, b, task = _harness(tmp_path, serve, extend)
    steps = h.plan(first=True)
    assert [s.name for s in steps] == ["walk_to(316)#0"]
    assert [p["status"] for p in h.plans] == ["reach extension", "plan"]
    assert h.plans[0]["reach_misses"] == 1 and h.plans[0]["reach_values"] == ["room=28"]
    assert len(calls) == 1 and calls[0]["room"] == 28  # the engine's facts, as reader-scope --reach-also reads them
    assert all(isinstance(v, int) for v in calls[0].values())
    (x,) = h.extensions
    assert x["plan"] == 0 and x["n_values"] == 1 and x["values"] == ["room=28"] and not x["replan"]
    assert x["actions_before"] == x["actions"] == 3 and x["seconds"] >= 0 and x["cpu_s"] >= 0
    assert h.packed == tmp_path / "reach-01" / "reading.tc"
    assert h.result()["extensions"] == h.extensions
    if serve:
        assert plan_fd._SERVER[0].key[1] == str(h.packed)  # the old server closed, one on the new file
    # a replan from a start the extended reading covers: no extension
    h.bans.append(RP.ban_of(recs_walk(task), h.belief()))
    assert [s.name for s in h.plan(first=False)] == ["sentence(1,7)#2", "sentence(2,316)#1"]
    assert len(calls) == 1 and h.plans[-1]["status"] == "plan"
    plan_fd.close_servers()


def recs_walk(task):
    return task.actions[0].what


@needs_taskcore
@pytest.mark.parametrize("serve", [False, True])
def test_an_extension_that_does_not_cover_the_start_stops_the_run(tmp_path, serve):
    calls = []

    def extend(start, hh):
        calls.append(start)
        return types.SimpleNamespace(task=task, packed=tmp_path / "t.tc", b=b, lists=None)  # the same reached file

    h, b, task = _harness(tmp_path, serve, extend)
    assert h.plan(first=True) == []
    assert len(calls) == 1 and len(h.extensions) == 1
    assert [p["status"] for p in h.plans] == ["reach extension", "reach extension"]
    assert h.stop_reason.startswith("reach extension: ") and "room=28" in h.stop_reason
    plan_fd.close_servers()


def test_the_harness_refuses_the_extension_without_taskcore():
    b, _, task = RT._harness_task()
    with pytest.raises(ValueError, match="taskcore planner only"):
        RP.Harness(b, task, task, RT._fake_branch(), {"state:316": {1}}, planner="python", reach_extend=lambda s, h: 0)


# --- replay_part1: the reader's command -------------------------------------------------------------------------------

def _command(monkeypatch, tmp_path, **kw):
    seen = []

    def run(cmd, **_):
        seen.append(cmd)
        return subprocess.CompletedProcess(cmd, 1, "", "stopped by the test")
    monkeypatch.setattr(P1.subprocess, "run", run)
    with pytest.raises(RuntimeError, match="reader-scope failed"):
        P1.read_rust((33,), 1, "state:316=1", "t", tmp_path, 3, **kw)
    return seen[0]


def test_read_rust_without_the_new_switches_runs_the_command_as_before(monkeypatch, tmp_path):
    cmd = _command(monkeypatch, tmp_path, elsewhere=True, live_locals=True, split_limit=True)
    assert cmd[:3] == ["nice", "-n", "10"] and cmd[3] == str(P1.READER_SCOPE)
    assert cmd[4:] == ["--rooms", "33", "--start-seed", "1", "--domain-goals", "none", "--goal", "state:316=1",
                       "--name", "t", "--threads", "3", "--out", str(tmp_path), "--packed", str(tmp_path / "t.rust.tc"),
                       "--keep", "--lines", str(tmp_path / "t.golden.jsonl.gz"), "--elsewhere", "--live-locals",
                       "--split-limit"]


def test_read_rust_passes_the_switches_and_every_start(monkeypatch, tmp_path):
    cmd = _command(monkeypatch, tmp_path, split_limit=True, switches="value_sets,input_settled", reach=True,
                   frame_costs=True, reach_also=[tmp_path / "a.txt", tmp_path / "b.txt"])
    tail = cmd[cmd.index("--split-limit"):]
    assert tail == ["--split-limit", "--switches", "value_sets,input_settled", "--reach", "--reach-also",
                    str(tmp_path / "a.txt"), "--reach-also", str(tmp_path / "b.txt"), "--frame-costs"]
    with pytest.raises(ValueError, match="needs --reach"):
        P1.read_rust((33,), 1, "state:316=1", "t", tmp_path, 3, reach_also=[tmp_path / "a.txt"])


def test_the_extender_reads_from_every_start_so_far_with_the_readings_switches(monkeypatch, tmp_path):
    calls = []
    monkeypatch.setattr(P1, "read_rust", lambda *a, **kw: calls.append((a, kw)) or tmp_path / "x.json")
    monkeypatch.setattr(P1.PR, "load_reading", lambda p: ("reading", p))
    doc = {"reach": True, "switches": "value_sets", "split_limit": True, "elsewhere": True,
           "frame_costs": {"dmax": 5}, "live_locals": {"switch": "--live-locals"}, "domain_goals": "none",
           "start": {"name": "results/start-part1.jsonl seed 1"}, "scope": {"rooms": [33, 28]},
           "goal": {"owner:566": [1]}}
    rd = types.SimpleNamespace(doc=doc, b=types.SimpleNamespace())
    starts = []
    ext = P1.reach_extender(rd, tmp_path, 3, starts)
    assert ext({"room": 28, "at:local:202:41": 1}, None) == ("reading", tmp_path / "x.json")
    assert ext({"room": 41}, None) == ("reading", tmp_path / "x.json")
    (a1, k1), (a2, k2) = calls
    assert a1[:5] == ((33, 28), 1, "owner:566=1", "reach-01", tmp_path / "reach-01")
    assert k1 == {"elsewhere": True, "live_locals": True, "split_limit": True, "switches": "value_sets",
                  "reach": True, "frame_costs": True, "reach_also": [tmp_path / "reach-01" / "start.txt"]}
    assert k2["reach_also"] == [tmp_path / "reach-01" / "start.txt", tmp_path / "reach-02" / "start.txt"]
    assert starts == k2["reach_also"]  # shared with F10's reread
    assert (tmp_path / "reach-01" / "start.txt").read_text() == "room\t28\nat:local:202:41\t1\n"
    with pytest.raises(SystemExit, match="needs a reading made with --reach"):
        P1.reach_extender(types.SimpleNamespace(doc=dict(doc, reach=None), b=rd.b), tmp_path, 3, [])


def test_reader_switches_of_a_reading_without_them_are_none():
    assert P1.reader_switches({"split_limit": True}) == {}
    assert P1.reader_switches({"reach": True, "frame_costs": {"dmax": 5}}, ["a"]) == \
        {"reach": True, "frame_costs": True, "reach_also": ["a"]}


@pytest.mark.skipif(not P1.READER_SCOPE.exists(), reason="no reader-scope binary")
def test_reader_scope_takes_several_starts(tmp_path):
    """reader-scope on room 33 with --reach, from seed 1's start alone and with that start given twice more as
    --reach-also files (its facts.py map): the same packed file and reached values, and both files in the report."""
    row = P1.start_row(1)
    f = tmp_path / "s.txt"
    f.write_text("".join(f"{k}\t{v}\n" for k, v in RP.reach_start(row["facts"]).items()))
    outs = {}
    for name, extra in (("one", []), ("three", ["--reach-also", str(f), "--reach-also", str(f)])):
        cmd = ["nice", "-n", "10", str(P1.READER_SCOPE), "--rooms", "33", "--start-seed", "1", "--domain-goals",
               "none", "--goal", "state:316=1", "--name", name, "--threads", "2", "--out", str(tmp_path),
               "--split-limit", "--reach"] + extra
        p = subprocess.run(cmd, capture_output=True, text=True)
        assert p.returncode == 0, p.stderr[-2000:]
        outs[name] = ((tmp_path / f"{name}.tc").read_bytes(), (tmp_path / f"{name}.reached").read_text(),
                      json.loads((tmp_path / f"{name}.json").read_text()))
    assert outs["one"][0] == outs["three"][0] and outs["one"][1] == outs["three"][1]
    assert "reach_also" not in outs["one"][2] and outs["three"][2]["reach_also"] == [str(f), str(f)]
    assert collections.Counter(outs["one"][1].split("\n"))  # a reached file with lines
