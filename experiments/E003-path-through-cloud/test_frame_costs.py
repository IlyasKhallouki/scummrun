"""Tests of frame costs on the planner's side: taskcore's --cost-caps, plan_fd's searches with costs, and the replay
harness's caps (replay.cost_caps_of, capped_cost). The reader's cost rule is tested in Rust
(scumm/reader/tests/frame_costs.rs).

usage: pytest test_frame_costs.py
"""

import json
import sys
import types
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import plan_fd  # noqa: E402
import replay as RP  # noqa: E402
import task as T  # noqa: E402
from domains import BINARY  # noqa: E402
from export_records import NONE, export_task, write_start  # noqa: E402

BIN = plan_fd.TASKCORE


def what(label, kind="event", end="yield", cont=""):
    return types.SimpleNamespace(label=label, kind=kind, end=end, cont=cont)


def timer_task():
    """The goal two ways: a slot's timer (one event that costs 361 frames) or three inputs of 2, 25 and 7."""
    facts = {"bit:1": BINARY, "bit:2": BINARY, "bit:3": BINARY, "door": BINARY}
    acts = [
        T.Action("event(local:211@33)#0", pre={"door": {0}}, effects=(T.Effect("door", value=1),),
                 what=what("event(local:211@33)"), cost=361),
        T.Action("sentence(2,316)#1", pre={"bit:1": {0}}, effects=(T.Effect("bit:1", value=1),),
                 what=what("sentence(2,316)", kind="sentence"), cost=2),
        T.Action("event(local:214@57#ab)#2", pre={"bit:1": {1}}, effects=(T.Effect("bit:2", value=1),),
                 what=what("event(local:214@57#ab)"), cost=25),
        T.Action("event(local:214@15)#3", pre={"bit:2": {1}}, effects=(T.Effect("door", value=1),),
                 what=what("event(local:214@15)"), cost=7),
    ]
    return T.Task(facts, acts, {"door": {1}})


START = {"bit:1": 0, "bit:2": 0, "bit:3": 0, "door": 0}


def export(work: Path, task):
    export_task(work / "t.tc", task, {"s": START}, meta=lambda a: (a.what.kind, a.what.end, a.what.cont),
                naming=lambda a: (NONE, a.what.label), every_fact=True)


def plan(work: Path, task, search, caps=None):
    args = []
    if caps is not None:
        (work / "caps.txt").write_text("".join(f"{k}\t{v}\n" for k, v in caps.items()))
        args = ["--cost-caps", work / "caps.txt"]
    return plan_fd.plan_taskcore(work / "t.tc", task.facts, task.goal, [[a] for a in task.actions], START,
                                 workdir=work / f"plan-{search}-{caps is not None}", search=search, args=args)


def test_searches_with_costs(tmp_path):
    task = timer_task()
    export(tmp_path, task)
    unit = plan(tmp_path, task, "lama-first")
    opt = plan(tmp_path, task, "astar-lmcp")
    greedy = plan(tmp_path, task, "lama-first-costs")
    assert [s.name for s in unit.steps] == ["event(local:211@33)#0"]  # one step: the fewest
    assert unit.plan_cost == 361
    assert [s.name for s in opt.steps] == ["sentence(2,316)#1", "event(local:214@57#ab)#2", "event(local:214@15)#3"]
    assert opt.plan_cost == 34
    assert greedy.status == "plan" and greedy.plan_cost <= 361


def test_cost_caps(tmp_path):
    task = timer_task()
    export(tmp_path, task)
    # the timer's slot shows 10 frames left: its event costs at most 10, and the optimum takes it
    res = plan(tmp_path, task, "astar-lmcp", {"event(local:211@33": 10})
    assert [s.name for s in res.steps] == ["event(local:211@33)#0"] and res.plan_cost == 10
    assert res.stats["cost_caps"] == {"caps": 1, "actions_lowered": 1}
    # a cap above the cost changes nothing; a cap names a label's prefix up to "#" or ")"
    res = plan(tmp_path, task, "astar-lmcp", {"event(local:211@33": 999, "event(local:214@57": 1, "event(local:21": 1})
    assert res.plan_cost == 10 and res.stats["cost_caps"]["actions_lowered"] == 1


def test_no_caps_same_file(tmp_path):
    """An empty caps file gives the file the option's absence gives, byte for byte."""
    task = timer_task()
    export(tmp_path, task)
    import subprocess
    st = write_start(tmp_path / "s.txt", task.facts, START)
    (tmp_path / "empty.txt").write_text("")
    a = subprocess.run([str(BIN), "sas", "--task", str(tmp_path / "t.tc"), "--start-file", str(st), "--out",
                        str(tmp_path / "a.sas")], capture_output=True, text=True)
    b = subprocess.run([str(BIN), "sas", "--task", str(tmp_path / "t.tc"), "--start-file", str(st), "--out",
                        str(tmp_path / "b.sas"), "--cost-caps", str(tmp_path / "empty.txt")], capture_output=True,
                       text=True)
    assert a.returncode == 0 and b.returncode == 0, (a.stderr, b.stderr)
    assert (tmp_path / "a.sas").read_bytes() == (tmp_path / "b.sas").read_bytes()


def test_harness_caps():
    state = {"slots": [
        {"number": 211, "where": 3, "status": 1, "code": 33, "delay": 1940},  # WIO_LOCAL, paused
        {"number": 2, "where": 2, "status": 2, "code": 736, "delay": 0},  # WIO_GLOBAL, running
        {"number": 210, "where": 3, "status": 1, "code": 62, "delay": 6},
        {"number": 209, "where": 3, "status": 1, "code": -1, "delay": 9},  # no code: left out
    ]}
    caps = RP.cost_caps_of(state, 5)
    assert caps == {"event(local:211@33": 1940 // 5 + 1, "event(script:2@736": 1, "event(local:210@62": 2}
    assert RP.capped_cost("event(local:211@33)#30568", 361, caps) == 361  # 389 frames left: above its cost
    assert RP.capped_cost("event(local:210@62)#5", 7, caps) == 2
    assert RP.capped_cost("event(script:2@736#a8c69122caf2)#30915", 3, caps) == 1
    assert RP.capped_cost("sentence(2,564)#11592", 2, caps) == 2


if __name__ == "__main__":
    import pytest
    sys.exit(pytest.main([__file__, "-q"]))
