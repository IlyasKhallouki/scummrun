"""Tests of the harness's switch ``unobserved_actor_rooms`` (replay.py, "The actors' rooms"): the reader's
``actorroom:`` facts are not in the state dump, so with the switch they are not matched or waited for, the plan's
values are kept, and a plan's start gives them the plan's value, or not known as the reader starts them. Off, nothing
changes.

usage: pytest test_actor_rooms_unobserved.py   (the taskcore case needs taskcore's release binary)
"""

import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import plan_fd  # noqa: E402
import replay as RP  # noqa: E402
import task as T  # noqa: E402
import test_replay_review as TR  # noqa: E402
import test_replay_taskcore as RT  # noqa: E402
from domains import Domain  # noqa: E402
from export_records import export_task, record_naming  # noqa: E402

ROOMS = RP.UNOBSERVED + (RP.ACTOR_ROOM,)


def test_the_functions_skip_the_facts_they_are_told_are_unobserved():
    assert RP.holds({"actorroom:6": {28}, "room": {28}}, {"room": 28}) == ["actorroom:6"]
    assert RP.holds({"actorroom:6": {28}, "room": {28}}, {"room": 28}, ROOMS) == []
    r = TR.rec("event", "event(local:211@33)", {}, {"actorroom:6": ("set", 28), "bit:6": ("set", 1)}, (),
               ("event", "local:211", 33, 0, False), set())
    doms = {"actorroom:6": Domain.of({-1, 28}), "bit:6": Domain.of({0, 1})}
    assert set(RP.expectations(r, {"actorroom:6": -1, "bit:6": 0}, doms)) == {"actorroom:6", "bit:6"}
    assert set(RP.expectations(r, {"actorroom:6": -1, "bit:6": 0}, doms, (), ROOMS)) == {"bit:6"}
    assert RP.predict({}, r, {}) == {}
    assert RP.predict({}, r, {}, RP.PREDICTED + (RP.ACTOR_ROOM,)) == {"actorroom:6": 28}
    b = type("B", (), {"table": type("T", (), {"points": {"at": {}, "esc": {}}})(), "counts": {}})()
    assert RP.view(b, doms, {"bit:6": 1}) == {"actorroom:6": None, "bit:6": 1}  # 0 is no value of the domain
    assert RP.view(b, doms, {"bit:6": 1}, ROOMS) == {"bit:6": 1}


def _task():
    """test_replay_taskcore's harness task with the cook's room: the walk needs it not known (the reader's start), the
    timer's event moves the cook to room 28."""
    b, (walk, other, flip), task = RT._harness_task()
    facts = dict(task.facts, **{"actorroom:6": Domain.of({-1, 28})})
    acts = []
    for a in task.actions:
        pre = dict(a.pre, **({"actorroom:6": {-1}} if a.name.startswith("walk_to") else {}))
        acts.append(T.Action(a.name, pre=pre, effects=a.effects, what=a.what))
    return b, walk, T.Task(facts, acts, task.goal)


def _harness(tmp_path, on: bool):
    b, walk, task = _task()
    export_task(tmp_path / "t.tc", task, {}, meta=lambda a: (a.what.kind, a.what.end, ""), naming=record_naming)
    h = RP.Harness(b, task, task, RT._fake_branch(), {"state:316": {1}}, packed=tmp_path / "t.tc",
                   workdir=tmp_path / f"plans-{on}", unobserved_actor_rooms=on)
    h.observe()
    return h, walk


def test_the_start_gives_the_plans_value_or_not_known(tmp_path):
    h, _ = _harness(tmp_path, True)
    assert "actorroom:6" not in h.now  # not observed
    start = {"room": 28, "actorroom:6": 0}
    assert h._actor_rooms_start(start, first=True)["actorroom:6"] == RP.ACTOR_ROOM_NOT_KNOWN
    h.pred = {"actorroom:6": 28}
    assert h._actor_rooms_start(start, first=False)["actorroom:6"] == 28
    h.pred = {}
    assert h._actor_rooms_start(start, first=False)["actorroom:6"] == RP.ACTOR_ROOM_NOT_KNOWN
    assert start == {"room": 28, "actorroom:6": 0}
    off, _ = _harness(tmp_path, False)
    assert off.now["actorroom:6"] is None and off.unobserved == RP.UNOBSERVED and off.predicted == RP.PREDICTED


@pytest.mark.skipif(not plan_fd.TASKCORE.exists(), reason="no taskcore binary")
def test_with_the_switch_the_first_plan_starts_the_rooms_not_known(tmp_path):
    on, _ = _harness(tmp_path, True)
    assert [s.name for s in on.plan(first=True)] == ["walk_to(316)#0"]
    assert (tmp_path / "plans-True" / "plan-000" / "start.txt").read_text().splitlines()[0] == "actorroom:6\t-1"
    assert "actorroom:6" in on.pred  # kept from the plan's start, for the replan
    off, _ = _harness(tmp_path, False)
    # off, the room reads as 0, which the domain lacks: the start is outside it, as before
    assert off.plan(first=True) == []
    assert (tmp_path / "plans-False" / "plan-000" / "start.txt").read_text().splitlines()[0] == "actorroom:6\t"
