"""Tests of E007's refinement loop (experiments/E007-fact-refinement/plan.md; replay.py "The refinement loop"):
which causes add a variable (``refinable``), and one turn of the harness (``Harness._refine_turn``) on hand-made
causes, with the reader and the planner stubbed: a cause that adds nothing, a variable added and the reading swapped,
a variable named again, the reader's premise check, and the switch off.

usage: uv run pytest experiments/E003-path-through-cloud/test_refine.py

The fixtures name MI1's numbers (tests may, C2); the modules tested do not.
"""

import sys
import types
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import replay  # noqa: E402

NOT_A_FACT = replay.NOT_A_FACT


def cause(cls, *values):
    return {"cause": True, "failure": "input", "record": "walk_to(428)#64", "frame": 2798, "why": "effect var:102",
            "kind": "wrote another value", "unit": "script:2", "offset": 43, "opcode": "assign =",
            "values": [{"what": w, "value": v, "source": "", "kind": k} for w, v, k in values], "class": cls}


def test_refinable_takes_the_variables_a_split_names_as_no_fact_of_the_reading():
    c = cause("split", ("var 103", 428, NOT_A_FACT), ("local 2", 0, "unknown"))
    assert replay.refinable(c) == [103]
    assert replay.refinable(cause("split", ("var 103", 428, NOT_A_FACT), ("var 104", 0, NOT_A_FACT),
                                  ("var 103", 428, NOT_A_FACT))) == [103, 104]


@pytest.mark.parametrize("c", [
    None,
    {"cause": None, "kind": "the frame before is not kept"},
    cause("semantics", ("var 103", 428, "fact")),
    cause("split", ("var 0", 306, "position")),
    cause("split", ("local 1", 339, "unknown")),
    cause("split", ("bit 70", 1, NOT_A_FACT)),
    cause(None, ("var 103", 428, NOT_A_FACT)),
])
def test_refinable_adds_nothing_for_any_other_cause(c):
    assert replay.refinable(c) == []


class _Reading:
    def __init__(self, actions, report, records=None):
        self.task = types.SimpleNamespace(actions=[None] * actions, folded=0)
        self.doc = {"n_records": records if records is not None else actions, "fact_vars": [r["var"] for r in report],
                    "fact_vars_report": report,
                    "reader_scope": {"read_wall_s": 1.0, "read_cpu_s": 2.0, "peak_rss_mb": 100}}
        self.path = Path("/nowhere/reading.json")


def harness(detail, refine, monkeypatch):
    """A Harness with only what ``_refine_turn`` reads: the window search's last cause, the plans, the engine's
    facts, and the reading (stubbed)."""
    h = object.__new__(replay.Harness)
    h.windows = types.SimpleNamespace(replan_cause_detail=detail, replan_cause="deviation at frame 2796")
    h.plans = [{"plan": 0, "operators": 143417}]
    h.branch = types.SimpleNamespace(frame=2796, status={"qf": 66916})
    h.refinements, h.refined, h.refine = [], {}, refine
    h.b, h.obs, h.now, h.pred = None, {"var:103": 428, "var:102": 0}, {}, {}
    h.base = types.SimpleNamespace(actions=[None] * 450967, folded=1020)
    h.progress = lambda msg: None
    h.stop_reason = None
    h.swapped = []
    h._swap = lambda new: h.swapped.append(new)
    monkeypatch.setattr(replay, "replan_start", lambda b, obs, pred, task: ({"room": 33, "var:102": 0}, {}))
    monkeypatch.setattr(replay.plan_fd, "close_servers", lambda: None)
    return h


def test_a_cause_that_names_no_such_variable_adds_nothing_and_the_run_goes_on(monkeypatch):
    calls = []
    h = harness({"what": "deviation", "cause": cause("split", ("var 0", 306, "position"))},
                lambda *a: calls.append(a), monkeypatch)
    assert h._refine_turn() is True
    assert calls == [] and h.swapped == [] and h.stop_reason is None
    t = h.refinements[0]
    assert t["added"] == [] and t["why_not"] == "a split on no variable the reading holds no fact of"
    h.windows.replan_cause_detail = {"what": "deviation", "cause": {"cause": None, "kind": "no rule"}}
    assert h._refine_turn() is True and h.refinements[1]["why_not"] == "no cause"
    h.windows.replan_cause_detail = {"what": "window empty", "cause": cause(None, ("var 103", 428, NOT_A_FACT))}
    assert h._refine_turn() is True and h.refinements[2]["why_not"] == "class None"


def test_a_split_on_a_variable_reads_the_scope_again_with_it_a_fact_and_takes_that_reading(monkeypatch):
    got = []
    new = _Reading(451000, [{"var": 103, "what": "added", "domain": list(range(80)), "outside_after_last_pass": []}],
                   records=452000)

    def refine(vars_, start, h):
        got.append((vars_, dict(start)))
        return new
    h = harness({"what": "deviation", "cause": cause("split", ("var 103", 428, NOT_A_FACT), ("local 2", 0, "unknown"))},
                refine, monkeypatch)
    assert h._refine_turn() is True
    # the start the reader reads from: the replan's, with the engine's value of the new fact (Lemma R (4))
    assert got == [([103], {"room": 33, "var:102": 0, "var:103": 428})]
    assert h.swapped == [new] and h.refined == {103: 0}
    t = h.refinements[0]
    assert t["added"] == [103] and t["engine_values"] == {103: 428}
    assert (t["actions_before"], t["records_before"], t["actions"], t["records"]) == (450967, 451987, 451000, 452000)
    assert t["facts"] == {103: {"what": "added", "domain": 80, "outside_after_last_pass": []}}
    assert t["operators_before"] == 143417 and "why_not" not in t


def test_a_variable_named_again_stops_the_run(monkeypatch):
    h = harness({"what": "window empty", "cause": cause("split", ("var 103", 428, NOT_A_FACT))},
                lambda *a: pytest.fail("no reading again"), monkeypatch)
    h.refined = {103: 0}
    assert h._refine_turn() is False
    assert h.stop_reason.startswith("refinement: var 103 added at plan 0 is named again")
    assert h.swapped == []


def test_a_reading_that_writes_the_new_fact_outside_its_domain_stops_the_run(monkeypatch):
    new = _Reading(10, [{"var": 103, "what": "added", "domain": [0, 428], "outside_after_last_pass": [999]}])
    h = harness({"what": "deviation", "cause": cause("split", ("var 103", 428, NOT_A_FACT))},
                lambda *a: new, monkeypatch)
    assert h._refine_turn() is False
    assert "R0" in h.stop_reason and h.swapped == []
    new2 = _Reading(10, [{"var": 103, "what": "the scope does not write it (a constant at its start value)",
                          "domain": [], "outside_after_last_pass": []}])
    h2 = harness({"what": "deviation", "cause": cause("split", ("var 103", 428, NOT_A_FACT))},
                 lambda *a: new2, monkeypatch)
    assert h2._refine_turn() is False and "did not make var [103] a fact" in h2.stop_reason


def test_after_a_stop_the_turn_is_only_recorded(monkeypatch):
    h = harness({"what": "window empty", "cause": cause("split", ("var 104", 0, NOT_A_FACT))},
                lambda *a: pytest.fail("no reading again after a stop"), monkeypatch)
    h.stop_reason = "the same empty window 2 times on walk_to(428)#64"
    assert h._refine_turn(stopped=True) is False
    assert h.refinements[0]["why_not"] == "the run had stopped" and h.stop_reason.startswith("the same empty window")


def test_the_loop_needs_the_strict_window_search_and_taskcore():
    with pytest.raises(ValueError, match="refinement loop"):
        replay.Harness(None, types.SimpleNamespace(facts={}), types.SimpleNamespace(facts={}), None, {},
                       planner="taskcore", refine=lambda *a: None)


def test_off_the_record_has_no_refinements():
    h = object.__new__(replay.Harness)
    h.refine = None
    h.reach_extend, h.slot_locals = None, False
    h.branch = types.SimpleNamespace(status={"frame": 3, "tick": 0})
    h.stop_reason, h.now, h.goal, h.replans, h.bans, h.plans, h.steps = "goal", {}, {}, 0, [], [], []
    h.frame0, h.tick0, h.t0 = 0, 0, None
    h.ex = types.SimpleNamespace(movie=[])
    h._holds = lambda goal, now: True
    assert "refinements" not in h.result()
