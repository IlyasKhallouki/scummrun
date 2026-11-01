"""Tests of window.py's parts that need no engine: the macro steps, the chain bound and the matcher (window-search.md)."""

import sys
import types
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import replay as RP  # noqa: E402
import window as W  # noqa: E402


def rec(i, kind, reads=(), writes=(), unit=None, wait=None, cost=1, effects=None, exps=None, pre=None, source=()):
    what = types.SimpleNamespace(kind="event" if kind == "event" else "sentence", effects=effects or {},
                                 source=source, label=f"event({unit}@{wait})" if unit else f"sentence({i})")
    step = types.SimpleNamespace(name=f"s{i}")
    path = types.SimpleNamespace(what=what, cost=cost, pre=pre or {})
    return W.MRec(i, step, [path], kind, frozenset(reads), frozenset(writes), unit, wait, cost, [exps or {}],
                  [pre or {}], {})


def x(key, *vals):
    return RP.Expect(key, frozenset(vals), "set")


def test_macro_steps_assign_events_by_interference():
    recs = [rec(0, "input", writes={"a"}),
            rec(1, "event", reads={"a"}, writes={"b"}, unit="u", wait=1),  # follows input 0
            rec(2, "event", reads={"c"}, writes={"d"}, unit="v", wait=1),  # follows no input, input 1 needs it
            rec(3, "input", reads={"b", "d"}, writes={"e"}),
            rec(4, "event", reads={"e"}, writes={"f"}, unit="u", wait=2),  # follows input 3
            rec(5, "event", reads={"z"}, writes={"y"}, unit="w", wait=1)]  # free: the last macro step
    ms = W.macro_steps(recs)
    assert ms == [[0, 1], [3, 2, 4, 5]]
    assert recs[3].preds == {0, 1, 2}
    assert recs[5].preds == frozenset()


def test_macro_steps_without_input():
    recs = [rec(0, "event", writes={"a"}, unit="u", wait=1), rec(1, "event", reads={"a"}, unit="u", wait=2)]
    assert W.macro_steps(recs) == [[0, 1]]


def test_chain_frames_follows_one_slot():
    eff_in = {"pos:script:2": ("set", 736), "var:1": ("set", 5)}
    recs = [rec(0, "input", writes=set(eff_in), effects=eff_in),
            rec(1, "event", reads={"pos:script:2"}, writes={"pos:script:2"}, unit="script:2", wait=736, cost=1,
                effects={"pos:script:2": ("set", 915)}),
            rec(2, "event", reads={"pos:script:2"}, writes={"pos:script:2"}, unit="script:2", wait=915, cost=5,
                effects={"pos:script:2": ("set", -1)}),
            rec(3, "event", reads={"pos:local:9"}, writes={"pos:local:9"}, unit="local:9", wait=4, cost=50,
                effects={"pos:local:9": ("set", 8)})]
    frames, names = W.chain_frames(recs, [0, 1, 2, 3])
    assert frames == 6 and names == ["s1", "s2"]


def test_chain_frames_needs_the_input():
    recs = [rec(0, "event", unit="script:2", wait=1, effects={"pos:script:2": ("set", 2)})]
    assert W.chain_frames(recs, [0]) == (0, [])


def test_matcher_final_writes_order_and_input():
    r0 = rec(0, "input", writes={"pos:s", "var:1"}, exps={"pos:s": x("pos:s", 10), "var:1": x("var:1", 1)})
    r1 = rec(1, "event", reads={"pos:s"}, writes={"pos:s"}, unit="s", wait=10, exps={"pos:s": x("pos:s", 20)},
             pre={"pos:s": {10}})
    for r in (r0, r1):
        r.preds = frozenset()
    r1.preds = frozenset({0})
    m = W.Matcher([r0, r1], {0, 1}, r0)
    m.note(5, {"pos:s": 20, "var:1": 1})
    assert m.matched == {}  # the input matches only once issued
    m.input_issued = True
    m.note(6, {"pos:s": 10, "var:1": 0})
    assert m.matched == {}  # var:1 is r0's final write; pos:s is r1's
    m.note(7, {"pos:s": 10, "var:1": 1})
    assert m.matched == {0: (7, 0)}
    m.note(8, {"pos:s": 20, "var:1": 1})
    assert m.matched == {0: (7, 0), 1: (8, 0)} and m.complete()


def test_matcher_mismatch_and_dead():
    r0 = rec(0, "input", writes={"pos:s"}, exps={"pos:s": x("pos:s", 10)})
    r1 = rec(1, "event", unit="s", wait=10, writes={"pos:s"}, exps={"pos:s": x("pos:s", 20)}, pre={"pos:s": {10}})
    m = W.Matcher([r0, r1], {0, 1}, r0)
    assert m.mismatch({"pos:s": 10}, {"pos:s": -1}) is None
    assert m.mismatch({"pos:s": 20}, {"pos:s": -1}) is None
    assert m.mismatch({"pos:s": 30}, {"pos:s": -1}) == ("pos:s", 30)
    assert m.mismatch({"pos:s": 30}, {"pos:s": 30}) is None  # held at the fork
    assert m.dead_needed({"pos:s": -1}) == ["s0", "s1"]
    m.input_issued = True
    m.note(3, {"pos:s": 10})
    assert m.dead_needed({"pos:s": 10}) == []
    assert m.dead_needed({"pos:s": -1}) == ["s1"]


def test_delta_lo_of():
    assert W.delta_lo_of({"dmax": 6}) == 6
    assert W.delta_lo_of({"dmax": 15}) == 1
    assert W.delta_lo_of(None) == 1


def test_input_clicks():
    assert W.input_clicks(types.SimpleNamespace(kind="sentence", source=("sentence", 4, 10, 11))) == 2
    assert W.input_clicks(types.SimpleNamespace(kind="sentence", source=("sentence", 4, 10, None))) == 1
    assert W.input_clicks(types.SimpleNamespace(kind="dialog_line", source=("line", 120))) == 1


def test_an_earlier_input_matches_once_issued():
    # the previous macro step's input (r0) matches after its issue frame: a later window's branches may match it
    r0 = rec(0, "input", writes={"var:1"}, exps={"var:1": x("var:1", 1)})
    r1 = rec(1, "input", writes={"var:2"}, exps={"var:2": x("var:2", 1)})
    for r in (r0, r1):
        r.preds = frozenset()
    r1.preds = frozenset({0})
    without = W.Matcher([r0, r1], {0, 1}, r1)
    without.input_issued = True
    without.note(5, {"var:1": 1, "var:2": 1})
    assert not without.complete()  # r0 can never match: the window could never succeed
    m = W.Matcher([r0, r1], {0, 1}, r1, issued={0})
    m.input_issued = True
    m.note(5, {"var:1": 1, "var:2": 1})
    assert m.matched == {0: (5, 0), 1: (5, 0)} and m.complete()
    parent = W.Matcher([r0, r1], {0, 1}, None, issued={0})  # the idling parent: r0, never r1
    parent.note(5, {"var:1": 1, "var:2": 1})
    assert parent.matched == {0: (5, 0)}


def test_the_end_rule_waits_for_the_inputs_predecessors():
    e = W.EndRule()
    assert not e.closed(ready=False, pre_now=True)  # the precondition holds, a predecessor is still to match
    assert not e.closed(ready=False, pre_now=False)  # its own predicted write closes it: not the end
    assert not e.open
    assert not e.closed(ready=True, pre_now=True) and e.open  # the model's window opens
    assert not e.closed(ready=True, pre_now=True)
    assert e.closed(ready=True, pre_now=False)  # and closes: the end


def test_checked_keys_follow_causal_links():
    recs = [rec(0, "input", writes={"pos:script:2", "var:1", "var:9"}),
            rec(1, "event", reads={"pos:script:2"}, writes={"pos:script:2", "room", "pos:local:7", "pos:local:8"},
                unit="script:2", wait=736),
            rec(2, "event", reads={"pos:local:8"}, writes={"pos:local:8", "var:1"}, unit="local:8", wait=4),
            rec(3, "input", reads={"room", "var:1"}, writes={"owner:5"})]
    W.checked_keys(recs, {"owner:5": {1}})
    assert recs[0].checked == {"pos:script:2"}  # var:1 is written again before it is read; var:9 never read
    assert recs[1].checked == {"pos:script:2", "room", "pos:local:8"}  # pos:local:7: no step reads it
    assert recs[2].checked == {"pos:local:8", "var:1"}
    assert recs[3].checked == {"owner:5"}  # the goal


def test_a_slot_no_pending_record_reads_runs_on():
    # the room entry predicts local 7 at 5; the engine shows it at 15: no pending record reads pos:local:7
    r0 = rec(0, "event", unit="script:2", wait=736, writes={"pos:script:2", "pos:local:7"},
             exps={"pos:script:2": x("pos:script:2", -1), "pos:local:7": x("pos:local:7", 5)},
             pre={"pos:script:2": {736}})
    r0.preds = frozenset()
    W.checked_keys([r0], {})
    m = W.Matcher([r0], {0}, None)
    assert m.mismatch({"pos:script:2": 736, "pos:local:7": 15}, {"pos:script:2": 736, "pos:local:7": -1}) is None
    m.note(3, {"pos:script:2": -1, "pos:local:7": 15})
    assert m.matched == {0: (3, 0)}


def test_chain_frames_through_a_started_slot():
    eff_in = {"pos:script:2": ("set", 736)}
    recs = [rec(0, "input", effects=eff_in),
            rec(1, "event", unit="script:2", wait=736, cost=1,
                effects={"pos:script:2": ("set", -1), "pos:local:211": ("set", 4), "room": ("set", 28)}),
            rec(2, "event", unit="local:211", wait=4, cost=1, effects={"pos:local:211": ("set", 33)}),
            rec(3, "event", unit="local:211", wait=33, cost=301, effects={"pos:local:211": ("set", 40)}),
            rec(4, "event", unit="local:9", wait=7, cost=50, effects={"pos:local:9": ("set", 7)})]
    frames, names = W.chain_frames(recs, [0, 1, 2, 3, 4])
    # 1 (the input's slot) + max(0, 1 - 1) (the started timer's first wait) + 301 (its own slot)
    assert frames == 302 and names == ["s1", "s2", "s3"]


def test_chain_frames_skip_a_yield_at_its_resume_point():
    recs = [rec(0, "input", effects={"pos:local:5": ("set", 3)}),
            rec(1, "event", unit="local:5", wait=3, cost=90, effects={"pos:local:5": ("set", 3)})]
    assert W.chain_frames(recs, [0, 1]) == (0, [])


def test_fast_view_is_view():
    import random
    import domains as D
    import packed_reading as PR
    facts = {"pos:local:1": D.Domain((-2, -1, 3, 5)), "pos:script:2": D.Domain((-1, 736, 915)),
             "escpos:local:1": D.Domain((-1, 7)), "pos:local:9": D.Domain((-1, 4)), "var:3": D.Domain((0, 1, 2)),
             "args:local:1": D.Domain((0, 11))}
    b = PR.Points(facts)
    del b.table.points["at"]["local:9"]  # a unit the points do not list: NOWHERE, as reader.task_start gives it
    rng = random.Random(7)
    keys = ["at:local:1:3", "at:local:1:5", "at:local:1:9", "at:script:2:736", "at:script:2:915", "esc:local:1:7",
            "at:local:9:4", "var:3"]
    fv = W.FastView(b, facts)
    for _ in range(300):
        obs = {k: rng.choice((0, 1)) if k != "var:3" else rng.choice((0, 1, 2, 7)) for k in keys if rng.random() < .7}
        assert fv(obs) == RP.view(b, facts, obs), obs


def test_a_deviation_needs_the_input_and_its_predecessors():
    r0 = rec(0, "event", unit="script:2", wait=736, writes={"state:1"}, exps={"state:1": x("state:1", 1)})
    r1 = rec(1, "input", writes={"pos:script:2"}, exps={"pos:script:2": x("pos:script:2", 736)})
    r2 = rec(2, "event", unit="script:2", wait=736, writes={"room"}, exps={"room": x("room", 28)})
    for r in (r0, r1, r2):
        r.preds = frozenset()
    r1.preds, r2.preds = frozenset({0}), frozenset({0, 1})
    m = W.Matcher([r0, r1, r2], {0, 1, 2}, r1)
    m.input_issued = True
    m.note(4, {"state:1": 0, "pos:script:2": 736, "room": 33})
    assert not W.is_deviation(m, r1, {0})  # the door's record has not matched: a failure, not a deviation
    m.note(5, {"state:1": 1, "pos:script:2": 736, "room": 33})
    assert W.is_deviation(m, r1, {0})


def test_the_earliest_end_point_wins_a_success_at_equal_q():
    def c(q, outcome, g):
        return types.SimpleNamespace(q_end=q, outcome=outcome, g=g)
    assert W.end_key(c(100, "deviation", 1)) < W.end_key(c(120, "success", 0))
    assert W.end_key(c(100, "success", 5)) < W.end_key(c(100, "deviation", 1))


def test_a_click_shows_in_its_own_frame():
    paths = [{"pos:local:203": x("pos:local:203", 40)}, {"pos:local:203": x("pos:local:203", 58)}]
    assert W.path_shows(paths, {"pos:local:203": 58})
    assert not W.path_shows(paths, {"pos:local:203": -1})
    assert W.path_shows([{}], {})  # a record that leaves no slot: nothing to tell


def test_another_path_needs_a_change_a_later_step_reads():
    noop = ("walk(1)#1", {"pos:local:203": x("pos:local:203", -1)}, {"local:203": x("local:203", 0)})
    enters = ("walk(1)#2", {"pos:script:2": x("pos:script:2", 736)}, {"script:2": x("script:2", 1)})
    fork = {"pos:local:203": -1, "local:203": 0, "pos:script:2": -1, "script:2": 0}
    now = dict(fork)
    assert W.other_path([noop, enters], now, fork) is None  # nothing shows: the no-op changes nothing
    now.update({"pos:script:2": 736, "script:2": 1})
    assert W.other_path([noop, enters], now, fork) == "walk(1)#2"


def test_later_reads_are_causal_links():
    recs = [rec(0, "input", writes={"a", "b", "c"}), rec(1, "event", reads={"a"}, writes={"b"}),
            rec(2, "input", reads={"b", "c"})]
    assert W.later_reads(recs, 0, ["a", "b", "c", "d"]) == {"a", "c"}
