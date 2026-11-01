"""Tests of strict.py and window.py's strict parts that need no engine (window-search.md section 9)."""

import sys
import types
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import reader as R  # noqa: E402
import replay as RP  # noqa: E402
import strict as S  # noqa: E402
import trace_explain as TE  # noqa: E402
import window as W  # noqa: E402

NOWHERE = R.NOWHERE


def run(seq, unit, code0, code1, frame=10, after=0, depth=0, ops=3):
    kind, _, n = unit.partition(":")
    where = {"script": 2, "local": 3}[kind]
    return TE.Run(frame, after, seq, depth, 0, where, int(n), 28, code0, code1, 2 if code1 >= 0 else 0, 0, 0, ops)


def mrec(i, kind, name, unit=None, wait=None, reads=(), writes=(), preds=(), exps=None):
    path = types.SimpleNamespace(name=name, what=types.SimpleNamespace(kind=kind, effects={}, source=()), cost=1,
                                 pre={})
    r = W.MRec(i, types.SimpleNamespace(name=name), [path], "event" if kind == "event" else "input",
               frozenset(reads), frozenset(writes), unit, wait, 1, [exps or {}], [{}], {})
    r.preds = frozenset(preds)
    return r


class FakeReading:
    """strict.Reading's interface: B1's tuples by action index."""

    def __init__(self, recs: dict):
        self.recs = recs  # idx -> (idx, unit, w, pre, eff, checks)
        self.by_unit = {}
        for i, t in recs.items():
            if t[1] is not None:
                self.by_unit.setdefault(t[1], []).append(i)
        self.by_name = {}
        self.task = types.SimpleNamespace(actions={})

    def rec(self, i):
        return self.recs[i]


def plan_of(sr, recs, names):
    sr.by_name = names
    return S.Plan(sr, recs, types.SimpleNamespace(), {})


def no_stutter(monkey_code=None):
    S.is_stutter = lambda code, r: r.ops == 1 and r.code1 == r.code0


no_stutter()


def test_an_event_matches_on_its_own_run():
    sr = FakeReading({7: (7, "local:3", 5, {"pos:local:3": {5}}, {"pos:local:3": ("set", 9), "var:1": ("set", 2)},
                          [])})
    r = mrec(0, "event", "e#7", "local:3", 5, reads={"pos:local:3"}, writes={"pos:local:3", "var:1"})
    plan = plan_of(sr, [r], {"e#7": 7})
    t = S.Track(plan, {})
    before, after = {"pos:local:3": 5, "var:1": 0}, {"pos:local:3": 9, "var:1": 2}
    ru = run(11, "local:3", 5, 9)
    vs = t.verdicts([ru], before, after, None)
    assert [v.status for _, v in vs] == ["explained"]
    out = t.apply(10, ru, vs[0][1], after)
    assert out[0][1] == "matched" and t.matched[0] == (10, 0, 11)
    # a later run from the same point is no longer this record's
    assert t.apply(11, run(15, "local:3", 5, 9), vs[0][1], after) == []


def test_an_event_whose_own_run_ends_elsewhere_is_refuted():
    sr = FakeReading({7: (7, "local:3", 5, {}, {"pos:local:3": ("set", 9)}, [])})
    r = mrec(0, "event", "e#7", "local:3", 5, writes={"pos:local:3"})
    t = S.Track(plan_of(sr, [r], {"e#7": 7}), {})
    ru = run(11, "local:3", 5, 12)
    vs = t.verdicts([ru], {"pos:local:3": 5}, {"pos:local:3": 12}, None)
    t.apply(10, ru, vs[0][1], {})
    assert 0 in t.refuted and t.refuted[0][1].startswith("end pos:local:3")
    # Lemma S1: refuted for good, even if a later run would match
    t.apply(12, run(20, "local:3", 5, 9), types.SimpleNamespace(status="explained", record=7, why=[]), {})
    assert 0 not in t.matched


def test_a_record_is_armed_by_the_one_that_left_its_slot_there():
    sr = FakeReading({7: (7, "local:3", 5, {}, {"pos:local:3": ("set", 9)}, []),
                      8: (8, "local:3", 9, {}, {"pos:local:3": ("set", 12)}, [])})
    r1 = mrec(0, "event", "a#7", "local:3", 5, writes={"pos:local:3"})
    r2 = mrec(1, "event", "b#8", "local:3", 9, reads={"pos:local:3"}, writes={"pos:local:3"}, preds={0})
    t = S.Track(plan_of(sr, [r1, r2], {"a#7": 7, "b#8": 8}), {})
    ok8 = types.SimpleNamespace(status="explained", record=8, why=[])
    ok7 = types.SimpleNamespace(status="explained", record=7, why=[])
    assert t.apply(9, run(4, "local:3", 9, 12, frame=9), ok8, {}) == []  # r1 has not matched: not r2's run
    t.apply(10, run(6, "local:3", 5, 9), ok7, {})
    t.apply(11, run(9, "local:3", 9, 12, frame=11), ok8, {})
    assert t.matched[1] == (11, 0, 9)


def test_a_run_before_a_record_it_follows_refutes_it():
    sr = FakeReading({7: (7, "local:3", 5, {}, {"pos:local:3": ("set", 9)}, []),
                      8: (8, "local:4", 1, {}, {"pos:local:4": ("set", 2)}, [])})
    r1 = mrec(0, "event", "a#7", "local:3", 5, writes={"pos:local:3", "var:9"})
    r2 = mrec(1, "event", "b#8", "local:4", 1, reads={"var:9"}, writes={"pos:local:4"}, preds={0})
    t = S.Track(plan_of(sr, [r1, r2], {"a#7": 7, "b#8": 8}), {})
    t.apply(10, run(5, "local:4", 1, 2), types.SimpleNamespace(status="explained", record=8, why=[]), {})
    assert t.refuted[1][1].startswith("order")


def test_the_input_needs_its_own_runs_and_a_change():
    eff = {"pos:script:4": ("set", NOWHERE), "pos:script:2": ("set", 736), "var:5": ("set", 1)}
    sr = FakeReading({3: (3, None, None, {"room": {28}}, eff, [])})
    r = mrec(0, "input", "click#3")
    plan = plan_of(sr, [r], {"click#3": 3})
    v0 = {"room": 28, "pos:script:4": NOWHERE, "pos:script:2": NOWHERE, "var:5": 0}
    own = {"script:4": run(20, "script:4", 0, -1, frame=30, depth=1),
           "script:2": run(21, "script:2", 0, 736, frame=30, after=1, depth=1)}
    v1 = dict(v0, **{"pos:script:2": 736, "var:5": 1})
    got = S.input_match(sr, plan, 3, ["script:4", "script:2"], v0, None, {}, own, v1, [])
    assert got["ok"] and got["took"] and got["last_seq"] == 21
    # the sentence script's start passed its wait (one run, one record): not matched, the end named
    bad = S.input_match(sr, plan, 3, ["script:4", "script:2"], v0, None, {},
                        dict(own, **{"script:2": run(21, "script:2", 0, -1, frame=30, after=1, depth=1)}), v1, [])
    assert not bad["ok"] and bad["why"] == "end pos:script:2" and bad["engine"] == NOWHERE
    # no start of a unit the record leaves at a wait point
    assert S.input_match(sr, plan, 3, ["script:4", "script:2"], v0, None, {}, {"script:4": own["script:4"]}, v1,
                         [])["why"] == "no run pos:script:2"
    # the precondition on the view before, after the writes of the records matched before the click's runs
    assert S.input_match(sr, plan, 3, [], dict(v0, room=33), None, {}, own, v1, [])["why"] == "pre room"
    assert S.input_match(sr, plan, 3, [], dict(v0, room=33), None, {"room": 28}, own, v1, [])["ok"]
    # an effect a later run of the frame writes is not checked (M4)
    assert S.input_match(sr, plan, 3, [], v0, None, {}, own, dict(v1, **{"var:5": 7}), [])["why"] == "effect var:5"


def test_a_match_on_writes_that_held_did_not_take_effect():
    """The check's item 3: a record whose only checked write held before the click."""
    sr = FakeReading({3: (3, None, None, {}, {"pos:script:4": ("set", NOWHERE)}, [])})
    plan = plan_of(sr, [mrec(0, "input", "c#3")], {"c#3": 3})
    v0 = {"pos:script:4": NOWHERE}
    got = S.input_match(sr, plan, 3, ["script:4"], v0, None, {}, {"script:4": run(20, "script:4", 0, -1, depth=1)},
                        v0, [])
    assert got["ok"] and not got["took"]
    # a slot the record leaves dead with no run of it: the view decides
    assert S.input_match(sr, plan, 3, ["script:4"], v0, None, {}, {}, v0, [])["ok"]


def test_open_keys_and_frames():
    rs = [run(1, "script:2", 0, 736, frame=9, after=1), run(2, "local:3", 5, 9, frame=10)]
    assert S.by_frame(rs)[10] == rs
    keys = S.open_keys_of(rs, {"var:1": 0, "pos:local:3": 5}, {"var:1": 1, "pos:local:3": 9})
    assert {"var:1", "pos:local:3", "pos:script:2", "args:local:3"} <= keys
    assert S.open_keys_of([run(2, "local:3", 5, 9, frame=10)], {"var:1": 0}, {"var:1": 1}) == frozenset()


def test_the_parent_stops_at_a_tie_only_when_a_later_fork_cannot_win():
    # a success the best: a later fork at Q(g) + b = c* would only tie, and the earlier fork wins ties
    assert W.strict_stop(100, False, 52, 48, 24)
    assert not W.strict_stop(100, False, 51, 48, 24)
    # a deviation the best: a later success at exactly c_d still wins
    assert not W.strict_stop(100, True, 52, 48, 24)
    assert W.strict_stop(100, True, 77, 48, 24)


class FakeBranch:
    def __init__(self, frame=0, control_at=None):
        self.status = {"frame": frame, "qf": 0}
        self.calls = []
        self.control_at = control_at

    @property
    def frame(self):
        return self.status["frame"]

    def run(self, *, frame=None, until=None, control=False, max_frame=None, idle=False, until_any=None, menu=False):
        self.calls.append((frame, bool(control), max_frame))
        target = frame if frame is not None else max_frame
        stop = "frame"
        if control and self.control_at is not None and self.frame < self.control_at <= target:
            target, stop = self.control_at, "control"
        elif max_frame is not None and frame is None:
            stop = "max_frame"
        self.status = {"frame": target, "qf": 4 * target, "stop": stop}
        return self.status


def test_the_stepper_plays_one_frame_at_a_time_with_the_same_stops():
    b = FakeBranch(10)
    seen = []
    st = W._Stepper(b, lambda: seen.append(b.frame)).run(frame=13)
    assert seen == [11, 12, 13] and st["frame"] == 13
    b2 = FakeBranch(10, control_at=12)
    seen2 = []
    st2 = W._Stepper(b2, lambda: seen2.append(b2.frame)).run(control=True, max_frame=20)
    assert seen2 == [11, 12] and st2["stop"] == "control"
    b3 = FakeBranch(10)
    seen3 = []
    st3 = W._Stepper(b3, lambda: seen3.append(b3.frame)).run(control=True, max_frame=12)
    assert seen3 == [11, 12] and st3["frame"] == 12


def test_kept_frames_keep_the_last_three():
    k = S.Kept()
    for f in range(1, 7):
        k.frame(f, [f], {"f": f}, {"v": f})
    assert sorted(k.runs) == [3, 4, 5, 6] and sorted(k.views) == [3, 4, 5, 6]


def test_plain_turns_runs_and_sets_into_json_values():
    out = W._plain({"run": run(1, "script:2", 0, 5), "s": frozenset({2, 1})})
    assert out["run"]["seq"] == 1 and out["s"] == [1, 2]


def test_kept_freeze_takes_frames_as_keys():
    k = S.Kept()
    k.frame(5, [], {"d": 5}, {"v": 5})
    got = k.freeze({4: {"v": 4}}, {4: {"d": 4}})
    assert sorted(got["views"]) == [4, 5] and sorted(got["dumps"]) == [4, 5]


def test_the_cache_keeps_a_dump_until_the_engine_runs():
    class B(FakeBranch):
        def __init__(self):
            super().__init__(5)
            self.dumps = 0

        def state(self):
            self.dumps += 1
            return {"frame": self.frame}

        def at(self, x, y):
            return {"obj": x + y + self.frame}
    b = B()
    c = W._Cached(b, {"frame": 5, "seeded": True})
    assert c.state()["seeded"] and b.dumps == 0  # the parent's dump at the fork
    assert c.at(1, 2) == c.at(1, 2) == {"obj": 8}
    c.run(frame=6)
    assert c.state() == {"frame": 6} and c.state() is c.state() and b.dumps == 1
    assert c.at(1, 2) == {"obj": 9}


def test_a_value_outside_its_domain_is_not_checked_by_m4():
    """trace-validation.md section 3, facts outside the view: an effect whose fact the view gives no value (outside
    the domain) is not checked."""
    sr = FakeReading({7: (7, "local:3", 5, {}, {"pos:local:3": ("set", 9), "var:133": ("set", 4)}, [])})
    r = mrec(0, "event", "e#7", "local:3", 5, writes={"pos:local:3", "var:133"})
    t = S.Track(plan_of(sr, [r], {"e#7": 7}), {})
    ru = run(11, "local:3", 5, 9)
    vs = t.verdicts([ru], {"pos:local:3": 5, "var:133": 4}, {"pos:local:3": 9, "var:133": None}, None)
    t.apply(10, ru, vs[0][1], {})
    assert 0 in t.matched
