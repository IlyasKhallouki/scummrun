"""Tests of replay.py's rules after the step 5 review: waits played first, the replay checks, the verdicts, the history
of a step, the replan's prediction, the checkpoint compare, and play_step on a fake engine (no ban when the record's
path never started, a ban when it started and a test on facts failed, no ban when a checked non-fact failed, an
interface failure, a spent input, a step the engine ran past).

usage: uv run pytest experiments/E003-path-through-cloud/test_replay_review.py

The fixtures name MI1's numbers (tests may, C2); replay.py does not.
"""

import collections
import sys
import types
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import reader as R  # noqa: E402
import replay as RP  # noqa: E402
import task as T  # noqa: E402
from domains import Domain  # noqa: E402
from facts import WIO_GLOBAL, WIO_LOCAL  # noqa: E402
from reader import Record  # noqa: E402


def rec(kind="sentence", label="sentence(2,316)", pre=None, effects=None, checks=(), source=(), engine_pre=()):
    return Record(kind, label, pre or {}, effects or {}, list(checks), "yield", source=source,
                  engine_pre=frozenset(engine_pre))


def act(r, i=0, effects=None):
    effs = effects if effects is not None else tuple(T.Effect(k, value=e[1]) for k, e in r.effects.items()
                                                     if e[0] == "set")
    return T.Action(f"{r.label}#{i}", pre={k: set(v) for k, v in r.pre.items()}, effects=effs, what=r)


def step(i, name="s"):
    return types.SimpleNamespace(action=i, name=name, guessed=())


# --- waits first ------------------------------------------------------------------------------------------------------

def test_waits_first_moves_the_independent_waits_before_the_input():
    walk = rec("walk_to", "walk_to(316)", {"script:2": {0}, "room": {28}},
               {"script:2": ("set", 1), "pos:script:2": ("set", 736)})
    timer = rec("event", "event(local:211@33)", {"pos:local:211": {33}, "room": {28}, "local:203": {0}},
                {"state:316": ("set", 1), "pos:local:211": ("set", -1)})
    arrive = rec("event", "event(script:2@736)", {"pos:script:2": {736}, "state:316": {1}},
                 {"room": ("set", 41)})
    kitchen = rec("event", "event(local:218@16)", {"room": {41}}, {"local:218": ("set", 0)})
    pick = rec("sentence", "sentence(9,566)", {"room": {41}}, {"owner:566": ("set", 1)})
    other = rec("event", "event(local:209@4)", {"pos:local:209": {4}}, {"local:208": ("set", 1)})
    task = T.Task({}, [act(r, i) for i, r in enumerate((walk, timer, arrive, kitchen, pick, other))], {})
    plan = [step(0, "walk"), step(1, "timer"), step(2, "arrive"), step(3, "kitchen"), step(4, "pick"),
            step(5, "other")]
    new, moved = RP.waits_first(task, plan, 0)
    assert [s.name for s in new] == ["timer", "walk", "arrive", "kitchen", "pick", "other"]
    assert [s.name for s in moved] == ["timer"]
    # the arrival reads what the walk writes, the kitchen's event what the arrival writes: they stay; the scan
    # stops at the next input, so "other" is not moved before the walk
    assert RP.waits_first(task, new, 1) == (new, [])  # played again at the input: nothing more moves
    assert RP.waits_first(task, new, 0) == (new, [])  # an event at i: unchanged
    # a wait that writes what the input reads stays
    blocker = rec("event", "event(script:5@1)", {"pos:script:5": {1}}, {"script:2": ("set", 1)})
    task2 = T.Task({}, [act(walk, 0), act(blocker, 1)], {})
    assert RP.waits_first(task2, [step(0), step(1)], 0)[1] == []


def test_waits_first_ignores_a_write_of_the_value_a_fact_already_has():
    # the cook's timer record unfreezes every script of the room (frozen:script:2 = 0), which the walk reads
    walk = rec("walk_to", "walk_to(316)", {"script:2": {0}, "frozen:script:2": {0}},
               {"script:2": ("set", 1), "pos:script:2": ("set", 736)})
    timer = rec("event", "event(local:211@33)", {"pos:local:211": {33}},
                {"state:316": ("set", 1), "frozen:script:2": ("set", 0)})
    task = T.Task({}, [act(walk, 0), act(timer, 1)], {})
    plan = [step(0, "walk"), step(1, "timer")]
    assert [s.name for s in RP.waits_first(task, plan, 0, {"frozen:script:2": 0})[1]] == ["timer"]
    assert RP.waits_first(task, plan, 0, {"frozen:script:2": 1})[1] == []  # there it would unfreeze the input's
    assert RP.waits_first(task, plan, 0)[1] == []
    # a step it passes that writes that fact keeps it: the last write would change
    other = rec("event", "event(script:2@1)", {"script:2": {1}}, {"frozen:script:2": ("set", 1)})
    task3 = T.Task({}, [act(walk, 0), act(other, 1), act(timer, 2)], {})
    assert RP.waits_first(task3, [step(0), step(1), step(2)], 0, {"frozen:script:2": 0})[1] == []


# --- the replay checks ------------------------------------------------------------------------------------------------

def state(vars_=None, actors=(), room=28):
    vs = [0] * 400
    vs[1] = 1  # VAR_EGO
    vs[33] = 2  # VAR_SENTENCE_SCRIPT
    for k, v in (vars_ or {}).items():
        vs[k] = v
    return {"vars": vs, "actors": list(actors), "room": room}


def test_eval_check_reads_the_dump_and_the_sentence_scripts_inputs():
    walk = rec("walk_to", "walk_to(316)", source=("sentence", 11, 316, None))
    c104 = ("global/script-002.txt:9 [0007]", "Var[104] == Local[1]", False, "?(not a fact: var:104)")
    assert RP.eval_check(c104, walk, state({104: 0})) is True  # var 104 is not 316: the False side
    assert RP.eval_check(c104, walk, state({104: 316})) is False
    c103 = ("global/script-002.txt:8 [0000]", "Var[103] == Local[2]", True, "?(not a fact: var:103)")
    assert RP.eval_check(c103, walk, state({103: 0})) is True  # B is none: local 2 is 0
    truth = ("global/script-002.txt:13 [001D]", "Var[103]", True, "?(not a fact: var:103)")
    assert RP.eval_check(truth, walk, state({103: 5})) is True
    assert RP.eval_check(truth, walk, state({103: 0})) is False
    # another sentence script installed: its locals are not the input's
    assert RP.eval_check(c104, walk, state({104: 316, 33: 7})) is None
    # an event's locals are never known; an indirect read (the reader read another variable) is not checked
    ev = rec("event", "event(script:23@409)")
    assert RP.eval_check(("global/script-023.txt:58 [00F9]", "Local[0] != Var[181]", False,
                          "?(not a fact: var:181)"), ev, state()) is None
    assert RP.eval_check(("global/script-023.txt:46 [00C9]", "Var[107] == 4", True, "?(not a fact: var:394)"),
                         ev, state({107: 4})) is None


def test_eval_check_reads_actor_getters_and_skips_the_rest():
    r = rec("walk_to", "walk_to(316)", source=("sentence", 11, 316, None))
    cook_x = ("room-028-bar/local-203.txt [001E]", "VAR_RESULT > 310", False, "?(position: getActorX(6))")
    assert RP.check_getter(cook_x) == ("getActorX", "6")
    assert RP.eval_check(cook_x, r, state(actors=[{"id": 6, "x": 298, "y": 135, "moving": 1}])) is True
    assert RP.eval_check(cook_x, r, state(actors=[{"id": 6, "x": 660, "y": 100, "moving": 1}])) is False
    assert RP.eval_check(cook_x, r, state()) is None  # not listed: not in the room or not visible
    room = ("room-028-bar/local-216.txt:10 [0004]", "VAR_RESULT != 28", False, "?(actor state: getActorRoom(6))")
    assert RP.eval_check(room, r, state(actors=[{"id": 6, "x": 1, "y": 1}])) is True
    ego_x = ("room-041-kitchen/local-204.txt:13 [0017]", "VAR_RESULT > 170", True, "?(position: getActorX(VAR_EGO))")
    assert RP.eval_check(ego_x, r, state(actors=[{"id": 1, "x": 200, "y": 1}])) is True
    for c in (("global/script-002.txt:175 [02EB]", "VAR_RESULT > 16", True, "?(position: getDist(1, 316))"),
              ("global/script-002.txt:170 [02E0]", "WaitForActor(Local[3]); blocks", False,
               "?(actor state: whether the actor is moving)"),
              ("x", "WaitForCamera(); blocks", "stay", "?(camera position: stays)"),
              ("x", "Var[N + Local[1]] == 2", True, "?(cannot work out: index)")):
        assert RP.eval_check(c, r, state()) is None, c


# --- the verdicts -----------------------------------------------------------------------------------------------------

POS = ("global/script-002.txt:175 [02EB]", "VAR_RESULT > 16", True, "?(position: getDist(1, 315))")
NONFACT = ("global/script-002.txt:9 [0007]", "Var[104] == Local[1]", False, "?(not a fact: var:104)")
WRITE = [RP.Mismatch("state:316", ("1",), 0, "write")]


def test_verdict_uses_the_checked_results():
    r = rec(checks=[NONFACT])
    assert RP.verdict(r, WRITE, [True]) == ("ban", None)  # every check held: the test on facts failed
    assert RP.verdict(r, WRITE, [False]) == ("replan", None)  # a checked non-fact explains it: no ban
    r2 = rec(checks=[NONFACT, POS])
    assert RP.verdict(r2, WRITE, [True, None]) == ("retry", "ban")  # the position could not be checked
    assert RP.verdict(r2, WRITE, [False, None]) == ("replan", None)
    cook = ("x", "VAR_RESULT > 310", False, "?(position: getActorX(6))")
    assert RP.verdict(rec(checks=[cook]), WRITE, [False]) == ("retry", "replan")  # never a ban
    assert RP.verdict(r2, [RP.Mismatch("var:5", ("2",), 0, "guess")], [False, None]) == ("replan", None)
    assert RP.classify(r2, WRITE) == "retry"  # no results: the reasons alone, as before


def test_time_reasons_are_retried():
    for why in ("loop exit by time", "wait over by time", "camera position"):
        assert RP.classify(rec(checks=[("x", "c", True, f"?({why}: w)")]), WRITE) == "retry", why


# --- the history of a step --------------------------------------------------------------------------------------------

def test_history_keeps_every_change_with_its_frame():
    h = RP.History({"state:316": 0, "local:203": 0, "var:5": 1}, 270)
    h.note(271, {"state:316": 0, "local:203": 1, "var:5": 1})
    h.note(303, {"state:316": 0, "local:203": 0, "var:5": 1})
    h.note(636, {"state:316": 1, "local:203": 0, "var:5": 1})
    h.note(737, {"state:316": 0, "local:203": 0, "var:5": 1})
    un = {u["key"]: u for u in h.unplanned({"var:5": None})}
    assert un["local:203"]["changes"] == [(271, 1), (303, 0)]  # back to its start value: still listed
    assert un["state:316"]["changes"] == [(636, 1), (737, 0)] and un["state:316"]["start"] == 0
    assert "var:5" not in un
    assert h.slots_changed(271, 272) == ["local:203"]
    assert h.slots_changed(300, 302) == []


# --- the engine ran on ------------------------------------------------------------------------------------------------

def test_ran_on_explains_a_wait_the_engine_went_past_in_one_frame():
    # ego already stands at the door: the sentence script does not stop at 736, it goes on to 937 in the click's
    # frame, which the plan's next step (the arrival) predicts; the kitchen's event then changes the room
    doms = {"script:2": Domain.of({0, 1}), "pos:script:2": Domain.of({-1, 736, 937}), "room": Domain.of({28, 41}),
            "local:218": Domain.of({0, 1}), "frozen:local:200": Domain.of({0, 1})}
    walk = rec("walk_to", "walk_to(316)", {"script:2": {0}}, {"script:2": ("set", 1), "pos:script:2": ("set", 736)})
    arrive = rec("event", "event(script:2@736)", {"pos:script:2": {736}},
                 {"pos:script:2": ("set", 937), "local:218": ("set", 1), "frozen:local:200": ("set", 1)})
    kitchen = rec("event", "event(local:218@16)", {"local:218": {1}},
                  {"room": ("set", 41), "local:218": ("set", 0), "script:2": ("set", 0), "pos:script:2": ("set", -1)})
    task = T.Task(doms, [act(walk, 0), act(arrive, 1), act(kitchen, 2)], {})
    before = {"script:2": 0, "pos:script:2": -1, "room": 28, "local:218": 0, "frozen:local:200": 0}
    exps = RP.expectations(walk, before, doms)
    h = RP.History(before, 100)
    h.note(101, dict(before, **{"script:2": 1, "pos:script:2": 937, "local:218": 1}))
    h.note(106, dict(before, room=41))
    best = RP.match(exps, {"script:2": 1, "pos:script:2": 937})
    chain = RP.ran_on(task, doms, exps, best, before, [step(1, "arrive"), step(2, "kitchen")], h)
    assert [(st.name, [m.key for m in ms]) for st, ms in chain] == [("arrive", ["frozen:local:200"])]
    end = RP.match(exps, h.frames()[-1][1])  # at the window's end both of the walk's writes differ
    chain = RP.ran_on(task, doms, exps, end, before, [step(1, "arrive"), step(2, "kitchen")], h)
    assert [st.name for st, _ in chain] == ["arrive", "kitchen"]
    # without the arrival's frame the mismatch stands
    h2 = RP.History(before, 100)
    h2.note(101, dict(before, **{"script:2": 1, "pos:script:2": 500}))
    assert RP.ran_on(task, doms, exps, RP.match(exps, h2.frames()[-1][1]), before, [step(1), step(2)], h2) is None
    assert RP.ran_on(task, doms, exps, [], before, [step(1)], h) is None  # nothing to explain


# --- the replan's prediction ------------------------------------------------------------------------------------------

def test_replan_copies_args_only_where_the_plan_put_the_slot():
    b = types.SimpleNamespace(table=types.SimpleNamespace(points={"at": {"script:2": {736, 915}}, "esc": {}}),
                              counts=collections.Counter())
    doms = {"pos:script:2": Domain.of({-1, 736, 915}), "args:script:2": Domain.of({0, 7, 9})}
    task = T.Task(doms, [], {})
    r = rec("walk_to", "walk_to(316)", effects={"pos:script:2": ("set", 736), "args:script:2": ("set", 7)})
    pred = RP.predict({"pos:script:2": -1, "args:script:2": 0}, r, {})
    assert pred == {"pos:script:2": 736, "args:script:2": 7}
    for obs, want in (({"at:script:2:915": 1}, 0), ({"at:script:2:736": 1}, 7)):
        now = RP.view(b, doms, obs)
        start, _ = R.replan_start(b, obs, RP.prediction(now, pred), task)
        assert start["args:script:2"] == want, obs
    # a write of another kind makes the plan's value unknown
    g = rec("event", "e", effects={"pos:script:2": ("guess", frozenset({736, 915}))})
    assert RP.predict(pred, g, {}) == {"args:script:2": 7}
    w = rec("event", "e", effects={"pos:script:2": ("when", "bit:6", 915)})
    assert RP.predict(pred, w, {"bit:6": 1})["pos:script:2"] == 915
    assert RP.predict(pred, w, {"bit:6": 0})["pos:script:2"] == 736


# --- the checkpoint compare -------------------------------------------------------------------------------------------

def test_compare_checkpoint_covers_tick_random_state_and_actors():
    st = {"vars": [0, 1], "bits_set": [], "owners": {}, "states": {}, "classes": {}, "room": 28, "slots": [],
          "cutscene": {"depth": 0, "levels": [{"override": None}]}, "verbs": [],
          "actors": [{"id": 1, "x": 325, "y": 135}]}
    status = {"tick": 1220, "rng": 99}
    from facts import facts_of
    cp = (235, facts_of(st), RP.extra_of(st, status))
    assert RP.compare_checkpoint(cp, st, status) == {}
    moved = dict(st, actors=[{"id": 1, "x": 326, "y": 135}])
    d = RP.compare_checkpoint(cp, moved, {"tick": 1221, "rng": 99})
    assert set(d) == {"tick", "actors"}
    assert RP.compare_checkpoint((235, facts_of(st)), dict(st, room=41), status)["facts"] == 1  # old form


# --- play_step on a fake engine ---------------------------------------------------------------------------------------

class FakeBranch:
    """A tiny engine: frames, scheduled inputs, a state dump and the hit test of one object. ``react(fb, frame,
    clicks)`` runs on every frame with the clicks of that frame."""

    def __init__(self, react, obj_x=100, ego=1):
        self.f = 100
        self.events = []
        self.react = react
        self.vars = [0] * 400
        self.vars[1] = ego  # VAR_EGO
        self.vars[33] = 2  # VAR_SENTENCE_SCRIPT
        self.vars[107] = 11  # the selected verb: Walk to (the dos-cd profile's verb_var)
        self.slots = {}  # number -> slot dump
        self.busy_until = 0
        self.obj = {"id": 316, "x": obj_x, "y": 50, "w": 20, "h": 30, "walk": [110, 90]}

    @property
    def frame(self):
        return self.f

    @property
    def status(self):
        st = {"frame": self.f, "up": 1, "cs": 0, "room": 28, "cam": 160, "xstart": 0, "room_w": 320,
              "tick": 2 * self.f, "rng": 7, "qf": 0}
        if self.f >= self.busy_until:
            st["idle"] = "sentence"
        return st

    def run(self, frame=None, **kw):
        target = frame if frame is not None else kw.get("max_frame", self.f + 1)
        while self.f < target:
            self.f += 1
            self.react(self, self.f, [e for e in self.events if e["frame"] == self.f and "click" in e])
        return dict(self.status, stop="frame")

    def inputs(self, events):
        self.events += events

    def state(self):
        return {"vars": list(self.vars), "bits_set": [], "owners": {}, "states": {}, "classes": {}, "room": 28,
                "slots": list(self.slots.values()), "cutscene": {"depth": 0, "levels": [{"override": None}]},
                "verbs": [{"id": 11, "name": "Walk to", "key": 0}], "inventory": [], "room_objects": [self.obj],
                "actors": [{"id": 1, "x": 50, "y": 100, "elevation": 0, "moving": 0}]}

    def at(self, x, y):
        o = self.obj
        return {"obj": 316} if o["x"] <= x < o["x"] + o["w"] and o["y"] <= y < o["y"] + o["h"] else {}


def walk_rec(checks=()):
    return rec("walk_to", "walk_to(316)", {"script:2": frozenset({0}), "room": frozenset({28}),
                                           "bit:6": frozenset({0})},
               {"script:2": ("set", 1), "pos:script:2": ("set", 736)}, checks, ("sentence", 11, 316, None),
               {"script:2", "room"})


def harness(react, r, *more, **kw):
    fb = FakeBranch(react, **kw)
    b = types.SimpleNamespace(table=types.SimpleNamespace(
        points={"at": {"script:2": {736, 937}, "local:203": {58}}, "esc": {}}), counts=collections.Counter())
    facts = {"script:2": Domain.of({0, 1}), "pos:script:2": Domain.of({-1, 736, 937}), "local:203": Domain.of({0, 1}),
             "pos:local:203": Domain.of({-1, 58}), "room": Domain.of({28, 41}), "bit:6": Domain.of({0, 1}),
             "state:316": Domain.of({0, 1})}
    task = T.Task(facts, [act(x, i) for i, x in enumerate((r,) + more)], {"state:316": {1}})
    h = RP.Harness(b, task, task, fb, {"state:316": {1}})
    h.task = task
    h.observe()
    h.checkpoint()
    return h, fb


def local_203(fb, f, clicks):
    """The bar's input path: the click starts local 203 for 30 frames; the sentence script never starts."""
    if clicks:
        fb.slots[203] = {"where": WIO_LOCAL, "number": 203, "code": 58, "frozen": 0}
        fb.busy_until = f + 30
    elif 203 in fb.slots and f >= fb.busy_until:
        del fb.slots[203]


def test_a_path_that_never_started_is_retried_then_replanned_without_a_ban():
    h, fb = harness(local_203, walk_rec())
    assert h.play_step(0, 0, step(0, "walk_to(316)#0")) == "replan"
    log = h.steps[-1]
    assert h.bans == [] and log.outcome == "path not started" and log.then == "replan"
    assert [t["kind"] for t in log.tries] == ["first", "retry", "retry", "retry"]
    t1 = log.tries[0]
    assert t1["taken"] and not t1["started"] and t1["how"] == "spent"
    assert t1["end"] == t1["issue"] + 30  # the first idle stop after local 203 ended, not 600 frames on
    assert log.tries[1]["start"] >= t1["end"] + RP.RETRY_WAIT
    assert {u["key"] for u in log.unplanned} >= {"local:203", "pos:local:203"}
    assert len(h.inputs()) == 4


def test_nothing_happened_is_played_once_more_and_never_banned():
    h, fb = harness(lambda fb, f, clicks: None, walk_rec())
    assert h.play_step(0, 0, step(0)) == "replan"
    log = h.steps[-1]
    assert [t["kind"] for t in log.tries] == ["first", "replay", "retry", "retry", "retry"]
    assert not any(t["taken"] for t in log.tries) and h.bans == [] and log.outcome == "nothing happened"
    assert all(t["how"] == "limit" for t in log.tries)


def sentence_then_other(fb, f, clicks):
    """The sentence script starts, but waits somewhere the record does not predict."""
    if clicks:
        fb.slots[2] = {"where": WIO_GLOBAL, "number": 2, "code": 500, "frozen": 0}


def test_a_started_path_that_differs_at_a_test_on_facts_is_banned():
    h, fb = harness(sentence_then_other, walk_rec())
    assert h.play_step(0, 0, step(0)) == "replan"
    log = h.steps[-1]
    assert log.outcome == "mismatch" and log.then == "ban" and len(log.tries) == 1
    assert h.bans == [RP.Ban("walk_to(316)", (("bit:6", 0), ("room", 28)))]
    assert log.tries[0]["started"] and log.tries[0]["how"] == "limit"
    assert {m["key"] for m in log.mismatches} == {"pos:script:2"}


def test_a_started_path_explained_by_a_checked_non_fact_is_not_banned():
    c104 = ("global/script-002.txt:9 [0007]", "Var[104] == Local[1]", False, "?(not a fact: var:104)")
    h, fb = harness(sentence_then_other, walk_rec([c104]))
    fb.vars[104] = 316  # the last sentence's object: the record's path took the other side
    assert h.play_step(0, 0, step(0)) == "replan"
    log = h.steps[-1]
    assert h.bans == [] and log.then == "replan" and log.checks[0][-1] is False


def test_an_executor_error_twice_is_an_interface_failure_without_a_ban():
    h, fb = harness(local_203, walk_rec(), obj_x=500, ego=3)  # the door off screen, ego not actor 1
    assert h.play_step(0, 0, step(0)) == "replan"
    log = h.steps[-1]
    assert log.outcome == "interface failure" and h.bans == [] and h.inputs() == []
    assert [t["kind"] for t in log.tries] == ["first", "replay"] and "interface:" in log.tries[0]["error"]


def test_a_step_the_engine_ran_past_is_matched_with_the_next_event_and_never_banned():
    def past_the_wait(fb, f, clicks):  # the sentence script goes past 736 to 937 in the click's frame
        if clicks:
            fb.slots[2] = {"where": WIO_GLOBAL, "number": 2, "code": 937, "frozen": 0}
    arrive = rec("event", "event(script:2@736)", {"script:2": frozenset({1}), "pos:script:2": frozenset({736})},
                 {"pos:script:2": ("set", 937)})
    h, fb = harness(past_the_wait, walk_rec(), arrive)
    assert h.play_step(0, 0, step(0), [step(1, "arrive")]) == "next"
    log = h.steps[-1]
    assert log.outcome == "matched, the engine ran on" and h.bans == [] and len(log.tries) == 1
    assert [st.name for st, _ in h.ran] == ["arrive"] and h.pred.get("pos:script:2") == 937


def test_the_matched_step_updates_the_plans_prediction():
    def sentence_ok(fb, f, clicks):
        if clicks:
            fb.slots[2] = {"where": WIO_GLOBAL, "number": 2, "code": 736, "frozen": 0}
    r = rec("walk_to", "walk_to(316)", {"script:2": frozenset({0})},
            {"script:2": ("set", 1), "pos:script:2": ("set", 736), "args:script:2": ("set", 9)},
            (), ("sentence", 11, 316, None), {"script:2"})
    h, fb = harness(sentence_ok, r)
    h._set_pred({"pos:script:2": -1, "args:script:2": 0})
    assert h.play_step(0, 0, step(0)) == "next"
    assert h.pred == {"pos:script:2": 736, "args:script:2": 9} and h.kept == {"args:script:2": 9}
    assert h.checkpoints[-1][0] == h.steps[-1].end and len(h.checkpoints[-1]) == 3


if __name__ == "__main__":
    sys.exit(pytest.main([__file__, "-q"]))
