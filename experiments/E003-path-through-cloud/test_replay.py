"""Tests of replay.py: the step-to-input translation, the matcher and the ban rule on hand-made cases, and one slow
test that plays the kitchen's main plan in the engine.

usage: uv run pytest experiments/E003-path-through-cloud/test_replay.py -m 'not slow'     (the hand-made cases)
       READER_PICKLE=<reading.pkl> uv run pytest experiments/E003-path-through-cloud/test_replay.py -m slow
       (the kitchen in the engine: about 15 minutes, planner limit 300 s; skipped without a reading)

The fixtures name MI1's numbers (tests may, C2); replay.py does not.
"""

import collections
import itertools
import os
import sys
import types
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import replay as RP  # noqa: E402
import task as T  # noqa: E402
from domains import ABOVE, Domain  # noqa: E402
from reader import Record  # noqa: E402


def rec(kind="sentence", label="sentence(2,316)", pre=None, effects=None, checks=(), source=(), engine_pre=()):
    return Record(kind, label, pre or {}, effects or {}, list(checks), "yield", source=source,
                  engine_pre=frozenset(engine_pre))


# --- the step-to-input translation -----------------------------------------------------------------------------------

def test_sentence_is_a_click_on_its_verb_and_objects():
    inp = RP.step_input(rec(source=("sentence", 2, 316, None)))
    assert inp == RP.Input("click", 2, 316, None)
    m = RP.to_move(inp, "open the door")
    assert (m.verb, m.obj, m.obj2, m.key, m.choose, m.until) == (2, 316, None, None, None, [])
    m2 = RP.to_move(RP.step_input(rec(label="sentence(4,566,322)", source=("sentence", 4, 566, 322))), "give")
    assert (m2.verb, m2.obj, m2.obj2) == (4, 566, 322)


def test_walk_to_is_the_walk_click_with_the_readers_walk_verb():
    m = RP.to_move(RP.step_input(rec(kind="walk_to", label="walk_to(316)", source=("sentence", 11, 316, None))), "w")
    assert (m.verb, m.obj, m.obj2) == (11, 316, None)


def test_dialog_line_is_a_click_on_its_verb_by_number():
    """The verb's point comes from the verb table (its slot and rectangle) and the engine's hit test, never from a
    text: two lines with the same text, an arrow verb (109) outside 120-128, and a verb the table does not list."""
    r = rec(kind="dialog_line", label="dialog_line(121)[33@28]", source=("verb", 121))
    assert RP.step_input(r) == RP.Input("line", 121)
    with pytest.raises(ValueError):  # no Move: the harness clicks it (Harness._click_verb)
        RP.to_move(RP.Input("line", 121), "say")
    verbs = [{"id": 121, "slot": 5, "x": 10, "y": 150, "name": "Same."},
             {"id": 122, "slot": 6, "x": 10, "y": 158, "name": "Same."},
             {"id": 109, "slot": 7, "x": 300, "y": 150, "name": ""}]
    boxes = {v["slot"]: (v["x"], v["y"], v["x"] + 8, v["y"] + 6) for v in verbs}

    def at(x, y):
        return {"verb_slot": next((s for s, (x0, y0, x1, y1) in boxes.items() if x0 <= x < x1 and y0 <= y < y1), 0)}
    state = {"verbs": verbs}
    assert RP.verb_point(121, state, at) == (12, 152) and RP.verb_point(122, state, at) == (12, 160)
    assert RP.verb_point(109, state, at) == (302, 152)
    assert RP.verb_point(123, state, at) is None  # not in the table: not shown, saved away or off
    boxes[7] = (0, 0, 0, 0)  # listed but not hit at any of its points
    assert RP.verb_point(109, state, at) is None
    assert RP.verb_point(121, {"verbs": verbs + [dict(verbs[0], slot=9)]}, at) is None  # listed twice


def test_skip_is_esc_and_an_event_is_a_wait():
    assert RP.to_move(RP.step_input(rec(kind="skip_cutscene", label="esc(script:5@40)")), "skip").key == "esc"
    assert RP.step_input(rec(kind="event", label="event(local:211@33)")) == RP.Input("wait")
    assert RP.to_move(RP.Input("wait"), "wait") is None
    with pytest.raises(ValueError):
        RP.step_input(rec(kind="settle"))


# --- observing --------------------------------------------------------------------------------------------------------

def fake_builder(points):
    return types.SimpleNamespace(table=types.SimpleNamespace(points={"at": points, "esc": {}}),
                                 counts=collections.Counter())


def test_view_labels_the_engines_values_and_slot_positions():
    b = fake_builder({"script:2": {736, 915}, "local:211": {33}})
    doms = {"pos:script:2": Domain.of({-1, 736, 915}), "pos:local:211": Domain.of({-1, 33}),
            "room": Domain.of({28, 41}), "var:53": Domain.of(range(-2, 6), -1, 2), "args:script:2": Domain.of({0, 7})}
    obs = {"at:script:2:915": 1, "room": 41, "var:53": 5}
    v = RP.view(b, doms, obs)
    assert v == {"pos:script:2": 915, "pos:local:211": -1, "room": 41, "var:53": ABOVE}  # args: not observed
    assert RP.view(b, doms, {"room": 33})["room"] is None  # outside the domain
    assert RP.holds({"room": {41}, "args:script:2": {7}}, v) == []
    assert RP.holds({"room": {28}}, v) == ["room"]


# --- the matcher ------------------------------------------------------------------------------------------------------

DOMS = {"state:316": Domain.of({0, 1}), "cutscene": Domain.of(range(6)), "var:52": Domain.of(range(-2, 6), -1, 2),
        "var:5": Domain.of({0, 1, 2}), "bit:6": Domain.of({0, 1}), "local:214": Domain.of({0, 1}),
        "room": Domain.of({28, 41}), "args:script:2": Domain.of({0, 9})}


def test_expectations_by_kind():
    r = rec(effects={"state:316": ("set", 1), "cutscene": ("add", 1), "var:52": ("add", -1),
                     "var:5": ("guess", frozenset({0, 2})), "local:214": ("when", "bit:6", 1),
                     "room": ("copy", "bit:6"), "args:script:2": ("set", 9)})
    before = {"state:316": 0, "cutscene": 0, "var:52": ABOVE, "var:5": 1, "bit:6": 0, "local:214": 0, "room": 28}
    x = RP.expectations(r, before, DOMS, guessed=(("var:5", 2, ()),))
    assert "args:script:2" not in x  # not in the state dump
    assert x["state:316"] == RP.Expect("state:316", frozenset({1}), "set", None, True)
    assert x["cutscene"].values == {1} and x["cutscene"].kind == "add"
    assert x["var:52"].kind == "guess" and x["var:52"].values == {2, ABOVE}  # ABOVE minus 1: several values
    assert x["var:5"].kind == "guess" and x["var:5"].chosen == 2
    assert x["local:214"].values == {0} and not x["local:214"].changes  # bit:6 is 0: no write
    assert x["room"].values == {0}  # a copy of bit:6
    # a guess the plan chose under a condition that does not hold before the step is not the step's choice
    y = RP.expectations(r, before, DOMS, guessed=(("var:5", 2, (("bit:6", frozenset({1})),)),))
    assert y["var:5"].chosen is None


def test_match_reports_the_fact_predicted_and_observed():
    r = rec(effects={"state:316": ("set", 1), "var:5": ("guess", frozenset({0, 2}))})
    x = RP.expectations(r, {"state:316": 0, "var:5": 1}, DOMS, guessed=(("var:5", 2, ()),))
    assert RP.match(x, {"state:316": 1, "var:5": 2}) == []
    assert RP.match(x, {"state:316": 0, "var:5": 2}) == [RP.Mismatch("state:316", ("1",), 0, "write")]
    assert RP.match(x, {"state:316": 1, "var:5": 0}) == [RP.Mismatch("var:5", ("2",), 0, "choice")]  # not the chosen
    assert RP.match(x, {"state:316": 1, "var:5": 1}) == [RP.Mismatch("var:5", ("0", "2"), 1, "guess")]
    un = RP.unplanned({"state:316": 0, "local:214": 0, "room": 28}, {"state:316": 1, "local:214": 1, "room": 28}, x)
    assert un == [RP.Mismatch("local:214", ("0",), 1, "unplanned")]


def test_classify():
    pos = ("global/script-002.txt:175 [02EB]", "VAR_RESULT > 16", True, "?(position: getDist(1, 315))")
    nonfact = ("global/script-002.txt:8 [0000]", "Var[103] == Local[2]", True, "?(not a fact: var:103)")
    timer = ("room-028-bar/local-211.txt:14 [001E]", "local:211's delay is over", True,
             "?(timer: local:211's delay count)")
    write = [RP.Mismatch("state:316", ("1",), 0, "write")]
    guess = [RP.Mismatch("var:5", ("2",), 0, "guess")]
    assert RP.classify(rec(checks=[pos]), write) == "retry"
    assert RP.classify(rec(checks=[timer, nonfact]), write) == "retry"
    assert RP.classify(rec(checks=[nonfact]), write) == "ban"
    assert RP.classify(rec(), write) == "ban"
    assert RP.classify(rec(checks=[pos]), guess) == "replan"  # a guessed write: replan, never a ban


# --- the ban rule -----------------------------------------------------------------------------------------------------

def small_task():
    facts = {"bit:6": Domain.of({0, 1}), "room": Domain.of({28, 41}), "var:53": Domain.of({0, 1, 2}),
             "state:316": Domain.of({0, 1})}
    r1 = rec(label="walk_to(316)", pre={"bit:6": frozenset({0}), "room": frozenset({28}), "var:53": frozenset({1, 2})},
             engine_pre={"room", "var:53"})
    r2 = rec(label="walk_to(316)", pre={"var:53": frozenset({1, 2})}, engine_pre={"var:53"})  # tests nothing
    r3 = rec(label="sentence(2,316)", pre={"room": frozenset({28})}, engine_pre={"room"})
    acts = [T.Action(f"{r.label}#{i}", pre={k: set(v) for k, v in r.pre.items()},
                     effects=(T.Effect("state:316", value=1),), what=r) for i, r in enumerate((r1, r2, r3))]
    return T.Task(facts, acts, {"state:316": {1}}), (r1, r2, r3)


def applies(a, state):
    return all(state[k] in s for k, s in a.pre.items())


def test_ban_is_the_tested_values_plus_the_room():
    _, (r1, r2, _) = small_task()
    state = {"bit:6": 0, "room": 28, "var:53": 1, "state:316": 0}
    assert RP.ban_of(r1, state) == RP.Ban("walk_to(316)", (("bit:6", 0), ("room", 28)))
    assert RP.ban_of(r2, state) == RP.Ban("walk_to(316)", (("room", 28),))  # tested nothing: the room alone


def test_cut_removes_exactly_the_banned_point_from_every_action_of_the_label():
    task, (r1, r2, r3) = small_task()
    ban = RP.Ban("walk_to(316)", (("bit:6", 0), ("room", 28)))
    banned = RP.apply_bans(task, [ban])
    states = [dict(zip(task.facts, vals)) for vals in itertools.product(*(d.values for d in task.facts.values()))]
    for r in (r1, r2, r3):
        old = [a for a in task.actions if a.what is r]
        new = [a for a in banned.actions if a.what is r]
        for s in states:
            was = any(applies(a, s) for a in old)
            point = s["bit:6"] == 0 and s["room"] == 28
            want = was and not (point and r.label == ban.label)
            assert any(applies(a, s) for a in new) == want, (r.label, s)
            assert sum(applies(a, s) for a in new) <= 1  # disjoint pieces: no duplicate operator
    assert [a for a in banned.actions if a.what is r1] == []  # its precondition was inside the point: gone
    assert [a for a in banned.actions if a.what is r3] == [a for a in task.actions if a.what is r3]


def test_ban_writes_a_task_the_planner_reads():
    task, _ = small_task()
    sas = T.write_sas(RP.apply_bans(task, [RP.Ban("walk_to(316)", (("room", 28),))]))
    names = {task.actions[o.action].name if o.action is not None and o.action < len(task.actions) else None
             for o in sas.ops}
    assert sas.counts is not None and names  # written without error
    assert "begin_state" in sas.text({"bit:6": 0, "room": 28, "var:53": 1, "state:316": 0})


# --- F10: the slots' dumped locals at a replan ----------------------------------------------------------------------

def dump(slots, depth=0, levels=None, owners=None):
    """A state dump with the locals (EngineConfig.dump_locals): slots as (where, number, code, locals, override)."""
    return {"room": 28, "owners": owners or {},
            "slots": [{"where": w, "number": n, "code": c, "locals": (list(l) + [0] * 25)[:25], "cutscene_override": o}
                      for w, n, c, l, o in slots],
            "cutscene": {"depth": depth, "levels": levels or [{"level": 0, "data": 0, "override": None}]}}


def test_dumped_slots_name_units_and_take_their_cutscene_data():
    levels = [{"level": 0, "data": 0, "override": None}, {"level": 1, "data": 5, "override": None},
              {"level": 2, "data": 9, "override": {"where": 2, "number": 2, "code": 940, "stale": False}}]
    d = RP.dumped_slots(dump([(2, 2, 736, [2, 428], 1), (3, 211, 33, [], 0), (2, 23, 409, [], 0),
                              (2, 23, 410, [], 0)], depth=2, levels=levels))
    assert set(d["slots"]) == {"script:2", "local:211"}
    assert d["skipped"] == [("two slots", "script:23")] * 2
    s2 = d["slots"]["script:2"]
    assert (s2["w"], s2["esc"], s2["override"], s2["cut_data"], s2["locals"][:3]) == (736, 940, 1, [9], [2, 428, 0])
    assert d["slots"]["local:211"]["esc"] is None
    no = dump([(2, 2, 736, [], 0)])
    del no["slots"][0]["locals"]
    assert RP.dumped_slots(no) is None  # the switch off, or an older build


def test_list_match_numbers_missing_zero_and_unknowns():
    slot = {"name": "script:2", "locals": [2, 428, 0, 1] + [0] * 21, "override": 0, "cut_data": []}
    lst = lambda locs, **kw: {"sig": kw.get("sig", 1), "locals": locs, "override": kw.get("ov", 0), "cut_data": []}  # noqa: E731
    assert RP.list_matches(lst({0: 2, 1: 428, 3: 1}), slot)
    assert not RP.list_matches(lst({0: 2, 1: 428}), slot)  # Local[3] is 1, the list's 0
    assert RP.list_matches(lst({0: 2, 1: {"unknown": "x", "values": None}, 3: 1}), slot)
    assert not RP.list_matches(lst({0: 2, 1: {"unknown": "x", "values": [427]}, 3: 1}), slot)
    assert not RP.list_matches(lst({0: 2, 1: 428, 3: 1}, ov=1), slot)
    lists = {("script:2", 736): [lst({0: 2, 1: {"unknown": "x", "values": None}, 3: 1}, sig=7),
                                 lst({0: 2, 1: 428, 3: 1}, sig=8)]}
    assert RP.match_list(lists, slot, 736)["sig"] == 8  # the most numbers
    assert RP.match_list(lists, slot, 737) is None


def test_replan_start_locals_matches_reads_again_and_falls_back():
    b = fake_builder({"script:2": {736}, "local:211": {33}, "local:207": {15}})
    task = types.SimpleNamespace(facts={k: None for k in ("args:script:2", "args:local:211", "args:local:207",
                                                          "pos:script:2", "pos:local:211", "pos:local:207")})
    obs = {"at:script:2:736": 1, "at:local:211:33": 1, "at:local:207:15": 1}
    slots = {"script:2": {"name": "script:2", "w": 736, "locals": [2, 428] + [0] * 23, "override": 0,
                          "cut_data": []},
             "local:211": {"name": "local:211", "w": 33, "locals": [5] + [0] * 24, "override": 0, "cut_data": []}}
    lists = {("script:2", 736): [{"sig": 8, "locals": {0: 2, 1: 428}, "override": 0, "cut_data": []}]}
    predicted = {"pos:script:2": 736, "args:script:2": 9, "pos:local:207": 15, "args:local:207": 4}
    start, counts, unmatched = RP.replan_start_locals(b, obs, predicted, task, slots, lists)
    assert (start["args:script:2"], start["args:local:211"], start["args:local:207"]) == (8, 0, 4)
    assert [u["name"] for u in unmatched] == ["local:211"]
    assert counts == {"live slots whose dumped locals match a built list": 1,
                      "live slots whose dumped locals are not the plan's list": 1,
                      "live slots with unknown locals (args 0)": 1, "live slots with no locals in the dump": 1,
                      "args copied from the plan (the slot is where the plan put it)": 1}
    # no locals at all: reader.replan_start's values
    import reader as R
    s2, _, _ = RP.replan_start_locals(b, obs, predicted, task, {}, lists)
    s3, _ = R.replan_start(b, obs, predicted, task)
    assert {k: s2[k] for k in task.facts} == {k: s3[k] for k in task.facts}


def test_lists_of_keeps_lists_the_record_still_sets():
    import packed_reading as PR
    ev = lambda sig, w=4, abort=False: {"name": "local:207", "w": w, "sig": sig, "abort": abort,  # noqa: E731
                                        "locals": {0: 9, 1: {"reason": "position", "what": "getActorX(1)",
                                                             "values": None}},
                                        "override": 0, "cut_data": ()}
    out = PR.lists_of([ev(21), ev(21, w=7, abort=True), ev(22)], {"args:local:207": ("set", 21)})
    assert out == [{"name": "local:207", "w": 4, "sig": 21, "override": 0, "cut_data": [],
                    "locals": [[0, 9], [1, {"unknown": "getActorX(1)", "values": None}]]}]



# --- F10 under --live-locals: the slots' flags -----------------------------------------------------------------------

def test_dumped_slots_take_the_flags_when_the_dump_has_them():
    d = dump([(2, 2, 736, [2], 0), (1, 428, 5, [], 0), (0, 429, 7, [], 0)], owners={"428": 15, "429": 1})
    assert all("flags" not in s for s in RP.dumped_slots(d)["slots"].values())  # an older dump: none
    for sl, (res, rec) in zip(d["slots"], [(True, False), (False, True), (False, False)]):
        sl["resistant"], sl["recursive"] = res, rec
    got = RP.dumped_slots(d)["slots"]
    assert got["script:2"]["flags"] == {"resistant": True, "recursive": False, "owned": False}
    assert got["obj:428"]["flags"] == {"resistant": False, "recursive": True, "owned": False}  # where 1: in the room
    assert got["obj:429"]["flags"] == {"resistant": False, "recursive": False, "owned": True}  # where 0: inventory
    assert got["obj:429"]["owned"] is True  # the owner rule, kept for readings without --live-locals


def test_list_match_under_live_locals_reads_and_flags():
    slot = {"name": "script:2", "room": 28, "locals": [2, 428, 0, 1] + [0] * 21, "override": 0, "cut_data": [],
            "flags": {"resistant": False, "recursive": False, "owned": False}}
    flags = {"recursive": None, "resistant": False, "owned": False}
    lst = lambda locs, **kw: {"sig": 1, "locals": locs, "override": 0, "cut_data": [],  # noqa: E731
                              "flags": dict(flags, **kw)}
    live = (frozenset({0, 3}), slot["flags"])
    assert not RP.list_matches(lst({0: 2, 3: 1}), slot)  # off the switch every local is compared
    assert RP.list_matches(lst({0: 2, 3: 1}), slot, live)  # Local[1] is not read
    assert not RP.list_matches(lst({0: 2}), slot, live)  # Local[3] is read and is 1
    assert not RP.list_matches(lst({0: 2, 3: 1}, resistant=True), slot, live)
    assert not RP.list_matches(lst({0: 2, 3: 1}, owned=True), slot, live)
    assert not RP.list_matches(lst({0: 2, 3: 1}, recursive=True), slot, live)
    rec = (frozenset({0, 3}), dict(slot["flags"], recursive=True))
    assert RP.list_matches(lst({0: 2, 3: 1}, recursive=True), slot, rec)
    assert not RP.list_matches(lst({0: 2, 3: 1}), slot, rec)  # a recursive slot: not a list with recursive unknown
    assert not RP.list_matches(lst({0: 2, 3: 1}, owned={"unknown": "x"}), slot, live)
    assert not RP.list_matches(lst({0: 2, 3: 1}), slot, (None, slot["flags"]))  # reads not known: all compared
    with pytest.raises(ValueError):
        RP.list_matches({"sig": 1, "locals": {}, "override": 0, "cut_data": []}, slot, live)  # a lists file w/o flags


def test_replan_start_locals_under_live_locals():
    b = fake_builder({"script:2": {736}})
    b.live_locals = {"switch": "--live-locals", "slots": {"script:2": [0, 3]}}
    b.read_locals = lambda name, room: frozenset(b.live_locals["slots"][name])
    task = types.SimpleNamespace(facts={k: None for k in ("args:script:2", "pos:script:2")})
    obs = {"at:script:2:736": 1}
    slot = {"name": "script:2", "room": 28, "w": 736, "locals": [2, 428, 0, 1] + [0] * 21, "override": 0,
            "cut_data": [], "flags": {"resistant": False, "recursive": False, "owned": False}}
    lists = {("script:2", 736): [{"sig": 8, "locals": {0: 2, 3: 1}, "override": 0, "cut_data": [],
                                  "flags": {"recursive": None, "resistant": False, "owned": False}}]}
    start, counts, unmatched = RP.replan_start_locals(b, obs, {}, task, {"script:2": slot}, lists)
    assert start["args:script:2"] == 8 and not unmatched
    start, _, unmatched = RP.replan_start_locals(b, obs, {}, task, {"script:2": dict(slot, flags=dict(
        slot["flags"], resistant=True))}, lists)
    assert start["args:script:2"] == 0 and [u["name"] for u in unmatched] == ["script:2"]
    no_flags = {k: v for k, v in slot.items() if k != "flags"}
    with pytest.raises(ValueError, match="no flags"):
        RP.replan_start_locals(b, obs, {}, task, {"script:2": no_flags}, lists)


def test_lists_of_gives_the_flags_under_live_locals():
    import packed_reading as PR
    ev = {"name": "local:207", "w": 4, "sig": 21, "abort": False, "locals": {0: 9}, "override": 0, "cut_data": (),
          "recursive": None, "resistant": True, "owned": False}
    out = PR.lists_of([ev], {"args:local:207": ("set", 21)}, flags=True)
    assert out[0]["flags"] == {"recursive": None, "resistant": True, "owned": False}
    assert "flags" not in PR.lists_of([ev], {"args:local:207": ("set", 21)})[0]


# --- the kitchen in the engine (slow) ---------------------------------------------------------------------------------

@pytest.mark.slow
def test_kitchen_main_plan_in_the_engine(tmp_path):
    """The kitchen's main plan played in the engine from run B's S1#7 pre: the engine starts where the ground truth
    says, the run ends with a recorded reason, and its recorded inputs give the same facts from boot."""
    reading = os.environ.get("READER_PICKLE")
    if not reading or not Path(reading).exists():
        pytest.skip("no kitchen reading: set READER_PICKLE (test_reader.py makes one)")
    import replay_kitchen
    out = replay_kitchen.run("all", Path(reading), replan_limit=3, time_limit=3600, write=False, out_dir=tmp_path,
                             plan_time=300)
    assert out["start_check"]["differ"] == 0
    r = out["run"]
    assert r["stop"] and r["steps"] and r["plans"][0]["status"] == "plan"
    assert all(e["frame"] > out["start_frame"] for e in r["inputs"])
    assert out["verify"]["same"], out["verify"]["differ"]
    assert out["verify"]["goal"] == r["goal"]
