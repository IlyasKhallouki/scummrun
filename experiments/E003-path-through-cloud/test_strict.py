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


def test_a_record_is_armed_by_the_clicks_run_of_its_unit():
    """An event record that follows a click resumes the unit the click's own run of it left there; a later run of the
    click (another unit) does not move that point."""
    sr = FakeReading({3: (3, None, None, {}, {"pos:script:2": ("set", 736)}, []),
                      8: (8, "script:2", 736, {}, {"pos:script:2": ("set", NOWHERE)}, [])})
    i0 = mrec(0, "input", "click#3", writes={"pos:script:2"})
    e1 = mrec(1, "event", "e#8", "script:2", 736, reads={"pos:script:2"}, writes={"pos:script:2"}, preds={0})
    t = S.Track(plan_of(sr, [i0, e1], {"click#3": 3, "e#8": 8}), {})
    t.matched[0] = (30, 0, 50)  # the click's last run: seq 50 (another unit)
    t.unit_seq[0] = {"script:2": 21}  # its run of the sentence script: seq 21
    t.first_seq[0] = 20
    assert t.armed(e1) == 21
    t.apply(31, run(40, "script:2", 736, -1), types.SimpleNamespace(status="explained", record=8, why=[]), {})
    assert 1 in t.matched


# --- (I4), M4's report and the classes (window-search.md 9.2; trace-validation.md sections 3 and 5) ---------------

def test_a_cite_names_its_unit_and_offset():
    assert S.cite_unit("global/script-002.txt:170 [02E0]") == ("script:2", 0x2E0)
    assert S.cite_unit("room-028-bar/local-203.txt:12 [001E]") == ("local:203", 0x1E)
    assert S.cite_unit("room-028-bar/obj-0316-door.txt:5 [0010]") == ("obj:316", 0x10)
    assert S.cite_unit("room-033-kitchen/entry.txt:3 [0007]") == ("entry", 7)
    assert S.cite_unit("local:207's delay is over") is None


class FakeCode:
    """TE.Code's interface over one unit: a conditional jump at [0000] (fall-through [0005], target [0009]), a wait at
    [0005], an op at [0009]."""

    def __init__(self):
        import ir
        jump = types.SimpleNamespace(st=ir.Jump(ir.Var(1), 9, "if"), next=1, target=2, yields=None, offset=0)
        wait = types.SimpleNamespace(st=ir.Op("WaitForActor", (ir.Const(1),)), next=2, target=None, yields="wait",
                                     offset=5)
        op = types.SimpleNamespace(st=None, next=3, target=None, yields=None, offset=9)
        self.u = types.SimpleNamespace(ins=[jump, wait, op], at={0: 0, 5: 1, 9: 2})

    def key(self, where, number, room):
        return ("local", room, number)

    def unit(self, key):
        return self.u

    def index(self, key, code):
        return self.u.at.get(code)


def logged(seq, codes, code1, frame=30, after=0):
    ex = TE.Explained([("b", seq, frame, after, 0, 3, 203, 28, codes[0])] + [("o", seq, c, 0) for c in codes] +
                      [("e", seq, code1, 2, 0, 0)])
    return ex, run(seq, "local:203", codes[0], code1, frame=frame, after=after, depth=1)


def test_a_check_is_decided_on_the_engines_own_opcodes():
    code = FakeCode()
    cite = "room-028-bar/local-203.txt:1 [0000]"
    ex, r = logged(5, [0, 5, 9], -1)  # the condition held (fall-through), the wait passed
    assert S.decide_on_log(code, ex, [r], (cite, "Var[1]", True, "?(position)")) is True
    assert S.decide_on_log(code, ex, [r], (cite, "Var[1]", False, "?(position)")) is False
    wait = ("room-028-bar/local-203.txt:2 [0005]", "WaitForActor(1); blocks", False, "?(actor state)")
    assert S.decide_on_log(code, ex, [r], wait) is True  # it went on: the record's side "does not block"
    ex2, r2 = logged(6, [0, 5], 5)  # it blocked at the wait
    assert S.decide_on_log(code, ex2, [r2], wait) is False
    ex3, r3 = logged(7, [0, 9], -1)  # the jump was taken: the condition was false
    assert S.decide_on_log(code, ex3, [r3], (cite, "Var[1]", True, "?(position)")) is False
    # a run the log does not hold, or no run of the unit: the dump decides, as before
    assert S.decide_on_log(code, TE.Explained(), [r], (cite, "Var[1]", True, "")) is S.NO_LOG
    assert S.decide_on_log(code, ex, [], (cite, "Var[1]", True, "")) is S.NO_LOG
    # a logged run of the unit that did not reach the cite: the engine did not test it there
    assert S.decide_on_log(code, ex3, [r3], wait) is None


def test_an_input_check_is_decided_where_the_engine_tests_it_not_on_the_forks_dump():
    """The check's (I4) case: the fork's dump has the actor at x = 314, the door script read 306 a frame later."""
    check = ("room-028-bar/local-203.txt:1 [001E]", "VAR_RESULT > 310", False, "?(position: getActorX(6))")
    sr = FakeReading({3: (3, None, None, {}, {"pos:local:203": ("set", NOWHERE)}, [check])})
    sr.task.actions = {3: types.SimpleNamespace(what=types.SimpleNamespace(kind="walk_to", source=("event",)))}
    plan = plan_of(sr, [mrec(0, "input", "c#3")], {"c#3": 3})
    v0 = {"pos:local:203": 5}
    d0 = {"vars": [0] * 300, "actors": [{"id": 6, "x": 314, "y": 100}], "room": 28}
    own = {"local:203": run(20, "local:203", 0, -1, depth=1)}
    vk = {"pos:local:203": NOWHERE}
    assert S.input_match(sr, plan, 3, ["local:203"], v0, d0, {}, own, vk, [])["why"] == f"check {check[0]}"
    assert S.input_match(sr, plan, 3, ["local:203"], v0, d0, {}, own, vk, [], lambda c: True)["ok"]
    assert not S.input_match(sr, plan, 3, ["local:203"], v0, d0, {}, own, vk, [], lambda c: False)["ok"]
    assert S.input_match(sr, plan, 3, ["local:203"], v0, d0, {}, own, vk, [], lambda c: S.NO_LOG)["why"] == \
        f"check {check[0]}"


def test_an_effect_outside_its_domain_is_reported_and_the_record_still_matches(monkeypatch):
    sr = FakeReading({7: (7, "local:3", 5, {}, {"pos:local:3": ("set", 9), "var:133": ("set", 0)}, [])})
    r = mrec(0, "event", "e#7", "local:3", 5, writes={"pos:local:3", "var:133"})
    t = S.Track(plan_of(sr, [r], {"e#7": 7}), {})
    ru = run(11, "local:3", 5, 9)
    after = {"pos:local:3": 9, "var:133": None}
    vs = t.verdicts([ru], {"pos:local:3": 5, "var:133": 0}, after, None)
    monkeypatch.setattr(S.F, "facts_of", lambda d: d["facts"])
    t.apply(10, ru, vs[0][1], after, {"facts": {"var:133": 566}})
    assert 0 in t.matched and t.outside[0] == [("var:133", 566)]
    assert t.copy().outside == t.outside
    # an input record's
    sr2 = FakeReading({3: (3, None, None, {}, {"pos:script:4": ("set", NOWHERE), "var:133": ("set", 0)}, [])})
    plan = plan_of(sr2, [mrec(0, "input", "c#3")], {"c#3": 3})
    got = S.input_match(sr2, plan, 3, ["script:4"], {"pos:script:4": 1, "var:133": 0}, None, {},
                        {"script:4": run(20, "script:4", 0, -1, depth=1)}, {"pos:script:4": NOWHERE, "var:133": None},
                        [], None, {"facts": {"var:133": 566}})
    assert got["ok"] and got["outside"] == [("var:133", 566)]


def test_a_read_of_a_variable_the_reading_does_not_hold_is_split():
    """trace-validation.md section 5: split when the value tested is not a fact of the reading. The k = 2 deviation:
    the sentence script wrote var 102 = 0 at [002B], a constant; the test that sent it there, [0000], read var 103,
    which the reading holds no fact of."""
    import model as M
    code = TE.Code(M.Game.load())
    key = ("global", 2)
    ex = TE.Explained([("b", 1, 30, 1, 0, 2, 2, 28, 0), ("o", 1, 0, 0xC8), ("r", 1, 103, 428),
                       ("r", 1, TE.LOCAL_BIT | 2, 0), ("o", 1, 0x2B, 0x1A), ("w", 1, 102, 0),
                       ("e", 1, -1, 0, 0, 0)])
    steps = TE.steps_of(code, key, list(ex.runs.values()))
    vals, m = TE.write_values(code, key, steps, 1, "var:102", None)
    assert m == 0 and steps[m].op.code == 0
    assert any(v.what == "var 103" for v in vals)
    code.facts = frozenset({"var:102", "room"})
    vals, _ = TE.write_values(code, key, steps, 1, "var:102", None)
    assert next(v for v in vals if v.what == "var 103").kind == "not a fact of the reading"
    assert TE.cause_class(vals) == "split"
    code.facts = frozenset({"var:102", "var:103", "room"})
    assert TE.kind_of_var(code, 103) == "fact"


# --- a run that passed a wait (window-search.md 9.9; experiments/E006-passed-wait/plan.md, Lemma P) ----------------

def part_log(seq, codes, code1, frame=30, after=1, depth=1, room=28):
    entries = [("b", seq, frame, after, 0, 3, 203, room, codes[0])] + [("o", seq, c, 0) for c in codes] + \
        [("e", seq, code1, 2 if code1 >= 0 else 0, 0, 0)]
    return entries, run(seq, "local:203", codes[0], code1, frame=frame, after=after, depth=depth)


def test_a_run_that_passed_a_wait_is_cut_before_the_wait():
    code = FakeCode()  # [0000] jump, [0005] a WaitFor op, [0009] an op
    ex = TE.Explained(part_log(5, [0, 5, 9], -1)[0])
    ps = S.parts_of(code, ex.runs[5], resumed=False)
    assert [(p.code0, p.code1, [o.code for o in p.ops]) for p in ps] == [(0, 5, [0]), (5, -1, [5, 9])]
    assert [p.seq for p in ps] == [5 + S.PART_STEP, 5 + 2 * S.PART_STEP]
    assert ps[0].status == TE.SS_RUNNING and ps[1].status == 0
    # a run that blocked at the wait (its last opcode) is one part: no cut
    assert S.parts_of(code, TE.Explained(part_log(6, [0, 5], 5)[0]).runs[6], resumed=False) is None
    # a slot resumed at the wait: the event's record starts at it, no cut there
    assert S.parts_of(code, TE.Explained(part_log(7, [5, 9], -1)[0]).runs[7], resumed=True) is None
    # a run started at the wait (not resumed): the record ends at it
    ps = S.parts_of(code, TE.Explained(part_log(8, [5, 9], -1)[0]).runs[8], resumed=False)
    assert [(p.code0, p.code1, len(p.ops)) for p in ps] == [(5, 5, 0), (5, -1, 2)]
    # the parts go into the kept log as runs of their own
    ex2 = TE.Explained([e for p in S.parts_of(code, ex.runs[5], False) for e in S.part_entries(p)])
    assert sorted(ex2.runs) == [5 + S.PART_STEP, 5 + 2 * S.PART_STEP]
    assert [o.code for o in ex2.runs[5 + 2 * S.PART_STEP].ops] == [5, 9]


def split_case():
    """The kitchen's walk_to(428) case on FakeCode's unit: the click's record ends at the wait [0005]; the plan's next
    event of the unit resumes there, passes the wait and ends the slot; the engine's run did both in one run."""
    wait = ("room-028-bar/local-203.txt:2 [0005]", "WaitForActor(1); blocks", False, "?(actor state)")
    sr = FakeReading({3: (3, None, None, {}, {"pos:local:203": ("set", 5), "var:1": ("set", 1),
                                              "args:local:203": ("set", 77)}, []),
                      8: (8, "local:203", 5, {"pos:local:203": {5}, "var:1": {1}, "args:local:203": {77}},
                          {"pos:local:203": ("set", NOWHERE), "var:2": ("set", 4)}, [wait])})
    i0 = mrec(0, "input", "click#3", writes={"pos:local:203", "var:1", "args:local:203"})
    e1 = mrec(1, "event", "e#8", "local:203", 5, reads={"pos:local:203", "var:1"}, writes={"pos:local:203", "var:2"},
              preds={0})
    sr.by_name = {"click#3": 3, "e#8": 8}
    plan = S.Plan(sr, [i0, e1], FakeCode(), {})
    kept = S.Kept()
    entries, rho = part_log(21, [0, 5, 9], -1)
    kept.add_entries(entries)
    v0 = {"pos:local:203": NOWHERE, "var:1": 0, "var:2": 0, "args:local:203": frozenset({0})}
    vk = {"pos:local:203": NOWHERE, "var:1": 1, "var:2": 4, "args:local:203": frozenset({0})}
    return sr, plan, kept, rho, v0, vk, i0, e1


def test_the_clicks_record_matches_the_part_up_to_the_wait():
    sr, plan, kept, rho, v0, vk, _, _ = split_case()
    own = {"local:203": rho}
    # off: the click's run ended the slot, the record ends at the wait
    off = S.input_match(sr, plan, 3, ["local:203"], v0, None, {}, own, vk, [])
    assert not off["ok"] and off["why"] == "end pos:local:203" and off["engine"] == NOWHERE
    # on: part 1 ends at the wait; var:1 holds after; args: is the later part's (the slot moved on and ended)
    parts = S.Parter(plan.code, kept)
    on = S.input_match(sr, plan, 3, ["local:203"], v0, None, {}, own, vk, [], parts=parts)
    assert on["ok"] and on["took"] and list(on["split"]) == ["local:203"] and len(on["split"]["local:203"]) == 2
    assert parts.splits == 1 and 21 + 2 * S.PART_STEP in kept.ex.runs
    # a record that ends elsewhere than the run's first passed wait does not match part 1
    sr.recs[3] = (3, None, None, {}, {"pos:local:203": ("set", NOWHERE)}, [])
    bad = S.input_match(sr, plan, 3, ["local:203"], v0, None, {}, own, vk, [], parts=parts)
    assert not bad["ok"] and bad["why"] == "end pos:local:203" and bad["run"] is rho


def test_the_plans_next_event_matches_the_rest_in_the_same_frame():
    sr, plan, kept, rho, v0, vk, i0, e1 = split_case()
    parts = S.Parter(plan.code, kept)
    res = S.input_match(sr, plan, 3, ["local:203"], v0, None, {}, {"local:203": rho}, vk, [], parts=parts)
    t = S.Track(plan, {})
    t.matched[0], t.unit_seq[0], t.first_seq[0] = (30, 0, 21), {"local:203": 21}, 21
    rest = res["split"]["local:203"][1:]
    state = {**v0, "pos:local:203": 5, "var:1": 1, "args:local:203": 77}  # the click's record's set writes
    vs = S.part_verdicts(plan, rest, state, vk, [], None, kept.ex, frozenset())
    assert [v.status for _, v in vs] == ["explained"]  # M3: the wait passed, on the part's own opcodes
    out = t.apply(31, vs[0][0], vs[0][1], vk)
    assert out[0][1] == "matched" and t.matched[1][2] == 21 + 2 * S.PART_STEP
    # the event's precondition on the state before the click (the record's writes not applied) fails M2
    vs2 = S.part_verdicts(plan, rest, v0, vk, [], None, kept.ex, frozenset())
    assert vs2[0][1].status == "unexplained"
    # a step ordered between the click and the event that has not run: the event is refuted (9.2's order check)
    t2 = S.Track(plan, {})
    t2.matched[0], t2.unit_seq[0], t2.first_seq[0] = (30, 0, 21), {"local:203": 21}, 21
    e1.preds = frozenset({0, 5})
    plan.idx[5] = [None]
    assert t2.apply(31, vs[0][0], vs[0][1], vk)[0][1] == "refuted"


def test_an_event_run_that_passed_a_later_wait_is_two_events():
    """At depth 0: the slot resumed at [0000], passed the wait at [0005] and ended. The event from [0000] (ends at the
    wait, leaves its locals there) matches part 1, the event from [0005] part 2; off, the run matches neither."""
    code = FakeCode()
    sr = FakeReading({7: (7, "local:203", 0, {"pos:local:203": {0}}, {"pos:local:203": ("set", 5),
                                                                       "args:local:203": ("set", 77)}, []),
                      8: (8, "local:203", 5, {"pos:local:203": {5}}, {"pos:local:203": ("set", NOWHERE),
                                                                       "var:2": ("set", 4)}, [])})
    a = mrec(0, "event", "a#7", "local:203", 0, writes={"pos:local:203", "args:local:203"})
    b = mrec(1, "event", "b#8", "local:203", 5, reads={"pos:local:203"}, writes={"pos:local:203", "var:2"}, preds={0})
    sr.by_name = {"a#7": 7, "b#8": 8}
    plan = S.Plan(sr, [a, b], code, {})
    kept = S.Kept()
    entries, rho = part_log(40, [0, 5, 9], -1, after=0, depth=0)
    kept.add_entries(entries)
    before = {"pos:local:203": 0, "var:2": 0, "args:local:203": frozenset({0})}
    after = {"pos:local:203": NOWHERE, "var:2": 4, "args:local:203": frozenset({0})}
    t = S.Track(plan, {})
    off = t.verdicts([rho], before, after, None, kept.ex)
    assert [v.status for _, v in off] == ["unexplained"]
    on = t.verdicts([rho], before, after, None, kept.ex, S.Parter(code, kept))
    assert [(r.code0, r.code1, v.status) for r, v in on] == [(0, 5, "explained"), (5, -1, "explained")]
    for r_, v_ in on:
        t.apply(41, r_, v_, after)
    assert set(t.matched) == {0, 1}
    assert S.plan_runs(plan, [rho]) == [rho]
