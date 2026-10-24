"""Tests for the abstract SCUMM v5 machine (model.py) and the action checks (actions.py).

usage: uv run pytest experiments/E003-path-through-cloud -q

Most tests run small hand-made units, written as descumm lines and read by ir.py's own reader, so
their control flow is linked the way real units are. A few run over the real IR.
"""

import collections
import dataclasses

import pytest

import actions
import ir
import model
from model import FactOracle, Game, Machine, Unknown


def unit(key: tuple, text: str, room: int | None = None) -> ir.Unit:
    u = ir.Unit(key, room, "test.txt", "")
    ir._read(u, text)
    assert not [n for n in u.notes if n[0] == "unparsed"], u.notes
    return u


def game(*units: ir.Unit, object_room=None) -> Game:
    return Game(units={u.key: u for u in units}, first_local=200, local_rooms={},
                object_room=object_room or {}, object_parent={}, verb_offset={}, room_width={}, pseudo={},
                pseudo_conflicts=[])


ROW1 = frozenset({"own_entry", "dispatch", "calls"})
ROW2 = ROW1 | {"room_change"}
ROW3 = ROW2 | {"settle"}
ROW4 = ROW3 | {"dialog"}
ROW5 = ROW4 | {"esc"}


def run(text: str, facts: dict | None = None, key=("global", 50), **kw) -> model.Run:
    u = unit(key, text)
    return Machine(game=game(u, object_room=kw.pop("object_room", None))).run(FactOracle(facts or {}), key, 0, **kw)


def only(r: model.Run) -> model.Path:
    assert len(r.paths) == 1, [(p.end, p.writes) for p in r.paths]
    return r.paths[0]


def test_reads_its_own_writes_first():
    r = run("""
[0000] (1A) Var[100] = 5;
[0005] (48) if (Var[100] == 5) /*0011*/ {
[000C] (1A)   Var[101] = 1;
[0011] (**) }
[0011] (00) stopObjectCode();
""", {"var:100": 0})
    p = only(r)
    assert p.writes["var:101"] == 1 and p.end == "end" and not p.assumptions


def test_unknown_condition_forks_and_records_the_assumption():
    r = run("""
[0000] (C8) Var[100] = getRandomNr(3);
[0005] (48) if (Var[100] == 1) /*0011*/ {
[000C] (1A)   Bit[5] = 1;
[0011] (**) }
[0011] (00) stopObjectCode();
""")
    assert len(r.paths) == 2
    sides = {p.assumptions[0][2]: p for p in r.paths}
    assert sides[True].writes["bit:5"] == 1 and "bit:5" not in sides[False].writes
    assert all("random draw" in p.assumptions[0][3] for p in r.paths)
    pred = model.predict(r, FactOracle({}))
    assert not pred["bit:5"]["certain"] and pred["bit:5"]["values"] == [0, 1]
    assert isinstance(pred["var:100"]["pre"], int) and pred["var:100"]["unknown"] == 2


def test_class_bit_0x80_means_set_and_has():
    r = run("""
[0000] (5D) setClass(400,[133,6]);
[0007] (1D) if (classOfIs(400,[133])) /*0017*/ {
[0010] (1A)   Var[100] = 1;
[0017] (**) }
[0017] (1D) if (classOfIs(400,[134])) /*0027*/ {
[0020] (1A)   Var[101] = 1;
[0027] (**) }
[0027] (1D) if (classOfIs(400,[6])) /*0037*/ {
[0030] (1A)   Var[102] = 1;
[0037] (**) }
[0037] (00) stopObjectCode();
""", {"class:400:6": 1})
    p = only(r)
    assert p.writes["class:400:5"] == 1 and p.writes["class:400:6"] == 0  # 133 = 0x80 | 5 sets, 6 clears
    assert p.writes["var:100"] == 1  # has 5
    assert "var:101" not in p.writes  # 134 = 0x80 | 6: has 6, which setClass cleared
    assert p.writes["var:102"] == 1  # 6 without 0x80: has not 6


def test_set_class_0_clears_every_class():
    p = only(run("[0000] (5D) setClass(400,[0]);\n[0005] (00) stopObjectCode();\n", {"class:400:7": 1}))
    assert p.writes["class:400:7"] == 0 and p.writes["class:400:32"] == 0


def test_pickup_object_effects_and_inventory_hook():
    p = only(run("[0000] (25) pickupObject(400,0);\n[0004] (00) stopObjectCode();\n",
                 {"var:1": 3, "var:34": 9, "owner:400": 15}))
    assert p.writes["owner:400"] == 3  # VAR_EGO's value, not a constant
    assert p.writes["class:400:32"] == 1 and p.writes["state:400"] == 1
    assert p.writes["script:9"] == 1  # the inventory hook, recorded as a start


def test_embedded_result_write_goes_to_var_0():
    p = only(run("""
[0000] (AC) Exprmode Var[100] = (<VAR_RESULT = getObjectOwner(400)> + 1);
[0010] (00) stopObjectCode();
""", {"owner:400": 7}))
    assert p.writes["var:0"] == 7 and p.writes["var:100"] == 8


def test_non_recursive_start_of_the_running_script_stops_it():
    p = only(run("[0000] (0A) startScript(50,[]);\n[0004] (1A) Var[100] = 1;\n", key=("global", 50)))
    assert p.end == "end" and p.writes["script:50"] == 1 and "var:100" not in p.writes
    p = only(run("[0000] (0A) startScript(50,[],R);\n[0004] (1A) Var[100] = 1;\n", key=("global", 50)))
    assert p.writes["var:100"] == 1  # recursive: the running copy goes on


def test_non_recursive_start_of_another_running_script_stops_it():
    facts = {"script:60": 1, "at:script:60:12": 1, "esc:script:60:9": 1}
    p = only(run("[0000] (0A) startScript(60,[]);\n[0004] (00) stopObjectCode();\n", facts))
    assert p.writes["script:60"] == 1  # the new instance, recorded as a start
    for k in ("at:script:60:12", "esc:script:60:9"):  # the old instance's wait point and override are gone,
        assert isinstance(p.writes[k], Unknown)  # and the new one, not run, may stop there again
    p = only(run("[0000] (4A) startScript(60,[],R);\n[0004] (00) stopObjectCode();\n", facts))
    assert "at:script:60:12" not in p.writes and "esc:script:60:9" not in p.writes  # R: the old one goes on


def test_hook_and_chain_stop_a_running_copy():
    facts = {"var:34": 9, "script:9": 1, "at:script:9:4": 1, "script:70": 1, "at:script:70:2": 1}
    p = only(run("[0000] (25) pickupObject(400,0);\n[0004] (00) stopObjectCode();\n", facts))
    assert isinstance(p.writes["at:script:9:4"], Unknown)  # the inventory hook runs without R
    text = "[0000] (42) chainScript(70,[]);\n"
    p = only(run(text, facts))  # the running slot's R flag is not known
    assert p.end == "end" and p.writes["script:50"] == 0 and isinstance(p.writes["at:script:70:2"], Unknown)
    assert "at:script:70:2" not in only(run(text, facts, recursive=True)).writes  # chained with the slot's R


def test_start_object_stops_the_object_first():
    facts = {"owner:400": 15, "room": 5, "objscript:400": 1, "at:obj:400:7": 1}
    obj = unit(("obj", 400), "[0000] (00) stopObjectCode();\n[0001] (00) stopObjectCode();\n")
    obj.entries = {11: 1}
    u = unit(("global", 50), "[0000] (37) startObject(400,11,[]);\n[0005] (00) stopObjectCode();\n")
    g = game(u, obj, object_room={400: 5})
    p = only(Machine(game=g).run(FactOracle(facts), ("global", 50), 0))
    assert p.writes["objscript:400"] == 1 and isinstance(p.writes["at:obj:400:7"], Unknown)
    p = only(Machine(game=g).run(FactOracle(facts | {"room": 6}), ("global", 50), 0))  # not in the room
    assert p.writes["objscript:400"] == 0 and p.writes["at:obj:400:7"] == 0


def test_set_var_range_writes_locals_bits_and_vars():
    p = only(run("""
[0000] (26) setVarRange(Local[0],3,[6,13,3]);
[0006] (26) setVarRange(Bit[384],2,[0,5]);
[000C] (26) setVarRange(Var[100],2,[7,8]);
[0012] (48) if (Local[1] == 13) /*001E*/ {
[0019] (1A)   Var[200] = 1;
[001E] (**) }
[001E] (00) stopObjectCode();
""", {"bit:384": 1}))
    assert not {"var:0", "var:1", "var:2", "ego"} & set(p.writes)  # a local range writes no variable
    assert p.frame.locals == {0: 6, 1: 13, 2: 3} and p.writes["var:200"] == 1
    assert p.writes["bit:384"] == 0 and p.writes["bit:385"] == 1
    assert p.writes["var:100"] == 7 and p.writes["var:101"] == 8


def test_wait_forks_blocked_or_through():
    r = run("""
[0000] (1A) Var[100] = 1;
[0005] (AE) WaitForActor(VAR_EGO);
[0007] (1A) Var[100] = 2;
[000C] (00) stopObjectCode();
""", {"objscript:400": 1}, key=("obj", 400))
    assert len(r.paths) == 2
    sides = {p.assumptions[0][2]: p for p in r.paths}
    blocked, through = sides[True], sides[False]
    assert blocked.end == "yield" and blocked.writes["var:100"] == 1 and blocked.writes["at:obj:400:5"] == 1
    assert through.end == "end" and through.writes["var:100"] == 2 and through.writes["objscript:400"] == 0
    assert all("actor state" in p.assumptions[0][3] and "blocks" in p.assumptions[0][1] for p in r.paths)
    assert not model.predict(r, FactOracle({"objscript:400": 1}))["objscript:400"]["certain"]


def test_print_to_actor_252_writes_the_key_press():
    p = only(run('[0000] (14) print(252,[Text("Hi")]);\n[0008] (00) stopObjectCode();\n', {"var:0": 4}))
    assert isinstance(p.writes["var:0"], Unknown) and p.writes["var:0"].reason == "key press"
    p = only(run('[0000] (14) print(3,[Text("Hi")]);\n[0008] (00) stopObjectCode();\n', {"var:0": 4}))
    assert "var:0" not in p.writes  # an actor's line: actorTalk's engine variables are not written


def test_start_records_a_fact_and_stop_clears_wait_points():
    p = only(run("""
[0000] (0A) startScript(60,[]);
[0004] (0A) startScript(210,[]);
[0008] (62) stopScript(70);
[000A] (00) stopObjectCode();
""", {"script:70": 1, "at:script:70:12": 1}))
    assert p.writes["script:60"] == 1 and p.writes["local:210"] == 1  # numbers from first_local are local
    assert p.writes["script:70"] == 0 and p.writes["at:script:70:12"] == 0


def test_room_change_ends_a_local_unit():
    p = only(run("[0000] (72) loadRoom(5);\n[0002] (1A) Var[100] = 1;\n", {"local:201": 1, "at:local:201:3": 1},
                 key=("local", 4, 201)))
    assert p.end == "room" and p.writes["room"] == 5 and "var:100" not in p.writes
    assert p.writes["local:201"] == 0 and p.writes["at:local:201:3"] == 0
    assert any(h == "loadRoom" and "room_change" in what for h, _, what in p.notes)  # the kills: a later row


def test_room_change_ends_a_room_object_but_not_an_inventory_one():
    text = "[0000] (72) loadRoom(5);\n[0002] (1A) Var[100] = 1;\n"
    assert only(run(text, key=("obj", 400), owned=False)).end == "room"
    assert only(run(text, key=("obj", 400), owned=True)).writes["var:100"] == 1


def test_yield_marks_the_slot_and_its_wait_point():
    p = only(run("[0000] (1A) Var[100] = 1;\n[0005] (80) breakHere();\n[0006] (1A) Var[100] = 2;\n",
                 {"objscript:400": 1, "at:obj:400:30": 1}, key=("obj", 400)))
    assert p.end == "yield" and p.writes["var:100"] == 1
    assert p.writes["objscript:400"] == 1 and p.writes["at:obj:400:6"] == 1 and p.writes["at:obj:400:30"] == 0


def test_override_and_cutscene_facts():
    text = """
[0000] (40) cutscene([1]);
[0003] (58) beginOverride();
[0005] (18) goto 000E;
[0008] (1A) Var[100] = 1;
[000D] (%s) %s;
[000E] (C0) endCutscene();
[000F] (00) stopObjectCode();
"""
    facts = {"cutscene": 0, "esc:script:9:3": 1, "var:5": 1}
    p = only(run(text % ("80", "breakHere()"), facts))
    assert p.end == "yield" and p.writes["var:100"] == 1  # beginOverride skips the jump after it
    assert p.writes["cutscene"] == 1 and p.writes["var:5"] == 0
    assert p.writes["esc:script:9:3"] == 0  # the new level has no override of its own
    assert p.writes["esc:script:50:5"] == 1  # armed at the jump after beginOverride
    p = only(run(text % ("58", "endOverride()"), facts))
    assert p.end == "end" and p.writes["cutscene"] == 0 and p.writes["esc:script:50:5"] == 0


def test_unknown_locals_and_timers():
    r = run("""
[0000] (48) if (Local[0] == 3) /*000C*/ {
[0007] (1A)   Var[100] = VAR_TMR_1;
[000C] (**) }
[000C] (00) stopObjectCode();
""")
    assert len(r.paths) == 2
    taken = next(p for p in r.paths if p.assumptions[0][2])
    assert isinstance(taken.writes["var:100"], Unknown) and taken.writes["var:100"].reason == "timer"
    r = run("[0000] (48) if (Local[0] == 3) /*000C*/ {\n[0007] (1A)   Var[100] = 1;\n[000C] (**) }\n", locals={0: 3})
    assert only(r).writes["var:100"] == 1


def test_verb_ops_visibility():
    p = only(run("""
[0000] (7A) VerbOps(120,[New(),Text("Hi"),On()]);
[000D] (7A) VerbOps(121,[On()]);
[0012] (7A) VerbOps(2,[Off()]);
[0017] (AB) saveVerbs(3,4,1);
[001C] (00) stopObjectCode();
""", {"verb:2": 1, "verb:3": 1}))
    assert p.writes["verb:120"] == 1  # New makes the slot, On shows it
    assert isinstance(p.writes["verb:121"], Unknown)  # On on a hidden verb: its slot may not exist
    assert p.writes["verb:2"] == 0 and p.writes["verb:3"] == 0 and "verb:4" not in p.writes


def test_path_limit_is_reported():
    lines = "".join(f"[{0x10 * i:04X}] (48) if (Local[{i}] == 1) /*{0x10 * i + 0x10:04X}*/ {{\n"
                    f"[{0x10 * i + 7:04X}] (1A)   Var[{100 + i}] = 1;\n[{0x10 * i + 0x10:04X}] (**) }}\n"
                    for i in range(6))
    u = unit(("global", 50), lines + "[0060] (00) stopObjectCode();\n")
    r = Machine(game=game(u), path_limit=8).run(FactOracle({}), ("global", 50), 0)
    assert r.limit > 0 and len(r.paths) <= 8 and any(p.end == "limit" for p in r.paths)


def test_unbuilt_features_raise():
    assert model.IMPLEMENTED == frozenset(model.FEATURES)  # every feature is built
    with pytest.raises(ValueError):  # esc runs the Esc frame's turns and then settle
        Machine(ROW2 | {"esc"}, game=game())
    with pytest.raises(ValueError):  # the Esc key runs only with esc
        Machine(ROW4, game=game()).run_esc(FactOracle({}))
    with pytest.raises(ValueError):  # dialog is row 4: it resumes the waiting script by settle's rule c
        Machine(ROW2 | {"dialog"}, game=game())
    with pytest.raises(ValueError):
        Machine(ROW4, game=game(), switches={"no such switch"})
    with pytest.raises(ValueError):  # a click on a verb runs only with dialog
        Machine(ROW3, game=game()).run_verb(FactOracle({}), 120)
    with pytest.raises(ValueError):  # settle is row 3: it builds on room_change
        Machine(frozenset({"own_entry", "dispatch", "calls", "settle"}), game=game())
    with pytest.raises(ValueError):
        Machine(frozenset({"own_entry", "no such feature"}), game=game())
    with pytest.raises(ValueError):  # dispatch and calls make one row
        Machine(frozenset({"own_entry", "calls"}), game=game())
    with pytest.raises(ValueError):  # room_change runs scripts nested
        Machine(frozenset({"own_entry", "room_change"}), game=game())
    assert model.row_of(ROW1) == 1 and model.row_of(ROW2) == 2 and model.row_of({"own_entry", "calls"}) is None
    assert model.row_of(ROW3) == 3 and model.row_of(ROW1 | {"settle"}) is None
    assert model.row_of(ROW4) == 4 and model.row_of(ROW2 | {"dialog"}) is None
    assert model.row_of(ROW5) == 5 and model.row_of(ROW3 | {"esc"}) is None


def test_usable_needs_input_verb_and_reachable_objects():
    u = unit(("obj", 400), "[0000] (00) stopObjectCode();\n")
    g = game(u, object_room={400: 5, 401: 6})
    facts = {"var:53": 1, "var:1": 1, "verb:9": 1, "room": 5, "owner:400": 15, "owner:401": 15, "owner:402": 1}
    assert actions.usable(actions.sentence(9, 400), FactOracle(facts), g).value is True
    assert actions.usable(actions.sentence(9, 401), FactOracle(facts), g).value is False  # another room's
    assert actions.usable(actions.sentence(9, 402), FactOracle(facts), g).value is True  # in the inventory
    assert actions.usable(actions.walk_to(11, 402), FactOracle(facts), g).value is False  # not a room object
    assert actions.usable(actions.walk_to(11, 400), FactOracle(facts), g).value is True  # no verb button needed
    assert actions.usable(actions.sentence(8, 400), FactOracle(facts), g).value is False  # verb 8 not shown
    assert actions.usable(actions.sentence(9, 400), FactOracle(facts | {"class:400:32": 1}), g).value is False
    assert actions.usable(actions.sentence(9, 400), FactOracle(facts | {"var:53": 0}), g).value is False
    assert actions.usable(actions.sentence(9, 3), FactOracle(facts), g).value is None  # an actor: room unknown
    assert actions.usable(actions.wait(), FactOracle({}), g).value is True
    assert [c for c, _ in actions.usable(actions.sentence(9, 400), FactOracle(facts), g).unchecked()] == \
        [actions.ON_SCREEN]
    assert [c for c, _ in actions.usable(actions.sentence(9, 402), FactOracle(facts), g).unchecked()] == \
        [actions.SLOT_SHOWN]
    g.object_parent = {400: 3}
    assert actions.PARENT in [c for c, _ in actions.usable(actions.sentence(9, 400), FactOracle(facts), g).unchecked()]


def test_pseudo_room_objects_and_esc():
    g = game(object_room={400: 58})
    g.pseudo = {218: 58}
    facts = {"var:53": 1, "var:1": 1, "verb:9": 1, "room": 218, "owner:400": 15}
    assert actions.usable(actions.sentence(9, 400), FactOracle(facts), g).value is True
    held = frozenset({"var", "esc"})
    assert actions.usable(actions.skip_cutscene(), FactOracle({"var:24": 27}, held), g).value is False
    assert actions.usable(actions.skip_cutscene(), FactOracle({"var:24": 27, "esc:script:9:3": 1}, held), g).value
    assert actions.usable(actions.skip_cutscene(), FactOracle({"var:24": 27}, frozenset({"var"})), g).value is None


# --- over the real IR -------------------------------------------------------------------------------------

@pytest.fixture(scope="module")
def real():
    return Game.load()


def test_every_statement_kind_has_a_rule(real):
    heads = {ir.head(x.st) for u in real.units.values() for x in u.ins if x.st is not None}
    assert not heads - set(model.HEADS)
    for h, (effect, cite, what) in model.HEADS.items():
        assert cite and what, h
        name = h if not h.startswith(("assign", "jump")) else None
        if name and effect in ("write", "end", "deferred") and name not in ("startScript", "startObject", "chainScript"):
            assert hasattr(Machine, "_op_" + name), h


def test_every_object_entry_runs_without_an_error(real):
    m = Machine(game=real)
    ends = collections.Counter()
    for k, u in real.units.items():
        if k[0] == "obj":
            for verb in u.entries:
                for p in m.run_entry(FactOracle({"var:1": 1, "room": u.room or 0}), k[1], verb).paths:
                    ends[p.end] += 1
    assert ends["end"] and "unmodelled" not in ends


def test_a_real_pickup_gives_the_object_to_ego(real):
    m = Machine(game=real)
    for k, u in real.units.items():
        if k[0] != "obj" or 9 not in u.entries:
            continue
        x = u.ins[u.entries[9]]
        if isinstance(x.st, ir.Op) and x.st.name == "pickupObject" and x.st.args[0] == ir.Const(k[1]):
            p = m.run_entry(FactOracle({"var:1": 4, "owner:%d" % k[1]: 15, "room": u.room}), k[1], 9).paths[0]
            assert p.writes[f"owner:{k[1]}"] == 4 and p.writes[f"class:{k[1]}:32"] == 1
            return
    pytest.skip("no object picks itself up as its first statement")


def test_non_fact_rule_is_a_least_fixpoint(real):
    nf = model.non_fact_vars(real)
    writes = model.script_writes(real)
    for n in nf:
        assert writes[n]
    counters = [n for n, vs in writes.items() if vs and all(isinstance(v, ir.BinOp) and v.a == ir.Var(n) and
                                                             isinstance(v.b, ir.Const) for v in vs)]
    assert not set(counters) & set(nf)  # a variable that only counts on itself stays a fact


# --- rows 1 and 2: dispatch, calls, room_change (hand-made units) ------------------------------------------


def calls(*units: ir.Unit, features=ROW1, **kw) -> Machine:
    return Machine(features, game=game(*units, object_room=kw.pop("object_room", None)), **kw)


def test_dispatch_runs_the_sentence_script_with_verb_and_objects():
    s2 = unit(("global", 2), """
[0000] (1A) Var[100] = Local[0];
[0005] (1A) Var[101] = Local[1];
[000A] (1A) Var[102] = Local[2];
[000F] (00) stopObjectCode();
""")
    m = calls(s2)
    p = only(m.run_sentence(FactOracle({"var:33": 2}), 9, 400, 401))
    assert (p.writes["var:100"], p.writes["var:101"], p.writes["var:102"]) == (9, 400, 401)
    assert p.end == "end" and p.writes["script:2"] == 0
    assert only(m.run_sentence(FactOracle({"var:33": 2}), 9, 400)).writes["var:102"] == 0  # no B: 0
    p = only(m.run_sentence(FactOracle({"var:33": 2, "script:2": 1}), 9, 400))
    assert p.end == "deferral unknown" and not p.writes  # live with no wait point: its turn cannot be run
    p = only(m.run_sentence(FactOracle({"var:33": 2, "script:2": 1, "frozen:script:2": 1, "at:script:2:5": 1}), 9, 400))
    assert p.end == "end" and p.writes["at:script:2:5"] == 0 and p.writes["frozen:script:2"] == 0  # a frozen one is stopped
    assert only(m.run_sentence(FactOracle({"var:33": 2}), 4, 400, 400)).end == "dropped"  # B == A
    assert only(m.run_sentence(FactOracle({"var:33": 0}), 4, 400)).end == "no sentence script"


def test_a_live_sentence_script_has_its_turn_before_the_sentence():
    """runAllScripts comes before checkAndRunSentenceScript in the click's frame: the live, unfrozen
    sentence slot first runs from its wait point; the sentence waits only if it is still live after."""
    s2 = unit(("global", 2), """
[0000] (1A) Var[100] = Local[0];
[0005] (1A) Var[101] = Local[1];
[000A] (1A) Var[102] = Local[2];
[000F] (00) stopObjectCode();
[0010] (1A) Var[103] = 1;
[0015] (00) stopObjectCode();
[0016] (80) breakHere();
[0017] (1A) Var[104] = 1;
[001C] (80) breakHere();
[001D] (00) stopObjectCode();
[001E] (2E) delay(20);
[0022] (1A) Var[105] = 1;
[0027] (00) stopObjectCode();
""")
    m = calls(s2)
    live = {"var:33": 2, "script:2": 1}
    # parked or after a breakHere at [0010]: it ends in this turn, so the sentence runs after it
    p = only(m.run_sentence(FactOracle(live | {"at:script:2:16": 1}), 9, 400))
    assert p.writes["var:103"] == 1 and p.writes["var:100"] == 9 and p.writes["at:script:2:16"] == 0
    assert p.turns == [("script:2", "end"), ("script:2", "end")] and p.end == "end"
    # at [0017]: it yields again, so it is still live and the sentence waits; its turn's writes stay
    p = only(m.run_sentence(FactOracle(live | {"at:script:2:23": 1}), 9, 400))
    assert p.end == "deferred" and p.writes["var:104"] == 1 and "var:100" not in p.writes
    assert p.writes["at:script:2:29"] == 1 and p.writes["at:script:2:23"] == 0
    # after a delay: paused until a timer ends it, a fork
    r = m.run_sentence(FactOracle(live | {"at:script:2:34": 1}), 9, 400)
    ends = sorted((p.end, p.writes.get("var:105"), p.writes.get("var:100")) for p in r.paths)
    assert ends == [("deferred", None, None), ("end", 1, 9)]
    assert all(a[3].startswith("?(timer") for p in r.paths for a in p.assumptions)
    # two wait points: the turn is not known
    assert only(m.run_sentence(FactOracle(live | {"at:script:2:16": 1, "at:script:2:23": 1}), 9, 400)).end == \
        "deferral unknown"


def test_calls_run_the_callee_nested_and_resume_the_caller():
    caller = unit(("global", 50), """
[0000] (0A) startScript(60,[7,8]);
[0005] (1A) Var[100] = 1;
[000A] (0A) startScript(61,[]);
[000D] (1A) Var[101] = 1;
[0012] (00) stopObjectCode();
""")
    yields = unit(("global", 60), "[0000] (1A) Var[110] = Local[0];\n[0005] (1A) Var[111] = Local[1];\n"
                                  "[000A] (80) breakHere();\n[000B] (1A) Var[112] = 1;\n")
    ends = unit(("global", 61), "[0000] (1A) Var[120] = 1;\n[0005] (00) stopObjectCode();\n")
    p = only(calls(caller, yields, ends).run(FactOracle({}), ("global", 50), 0))
    assert p.writes["var:110"] == 7 and p.writes["var:111"] == 8 and "var:112" not in p.writes  # args as locals
    assert p.writes["script:60"] == 1 and p.writes["at:script:60:11"] == 1  # yielded: live, after breakHere
    assert p.writes["var:120"] == 1 and p.writes["script:61"] == 0  # ended: not live
    assert p.writes["var:100"] == 1 and p.writes["var:101"] == 1 and p.end == "end"
    assert p.turns == [("script:60", "yield"), ("script:61", "end"), ("script:50", "end")]


def test_pickup_runs_the_inventory_hook_with_local_0_1_and_it_ends():
    u = unit(("global", 50), "[0000] (25) pickupObject(400,0);\n[0004] (00) stopObjectCode();\n")
    hook = unit(("global", 9), "[0000] (1A) Var[110] = Local[0];\n[0005] (00) stopObjectCode();\n")
    p = only(calls(u, hook).run(FactOracle({"var:34": 9, "var:1": 3, "owner:400": 15}), ("global", 50), 0))
    assert p.writes["owner:400"] == 3 and p.writes["var:110"] == 1
    assert p.writes["script:9"] == 0  # the hook ended in the same frame: not running (row 0 had it running)


def test_a_callee_that_freezes_parks_its_caller():
    caller = unit(("global", 50), "[0000] (0A) startScript(60,[]);\n[0003] (1A) Var[100] = 1;\n")
    freezer = unit(("global", 60), "[0000] (60) freezeScripts(127);\n[0002] (80) breakHere();\n"
                                   "[0003] (00) stopObjectCode();\n")
    p = only(calls(caller, freezer).run(FactOracle({"script:70": 1}), ("global", 50), 0))
    assert p.end == "parked" and "var:100" not in p.writes
    assert p.writes["at:script:50:3"] == 1 and p.writes["frozen:script:50"] == 1 and p.writes["script:50"] == 1
    assert p.writes["frozen:script:70"] == 1  # every live slot but the running one
    assert p.writes.get("frozen:script:60", 0) == 0 and p.writes["at:script:60:3"] == 1


def test_freeze_skips_a_freeze_resistant_caller():
    outer = unit(("global", 40), "[0000] (0A) startScript(50,[],F);\n[0004] (1A) Var[99] = 1;\n")
    caller = unit(("global", 50), "[0000] (0A) startScript(60,[]);\n[0003] (1A) Var[100] = 1;\n"
                                  "[0008] (00) stopObjectCode();\n")
    freezer = unit(("global", 60), "[0000] (60) freezeScripts(127);\n[0002] (80) breakHere();\n")
    p = only(calls(outer, caller, freezer).run(FactOracle({}), ("global", 40), 0))
    assert p.writes["var:100"] == 1  # 50 started with F: not frozen, so it resumes
    assert p.end == "parked" and p.writes["at:script:40:4"] == 1 and "var:99" not in p.writes


def test_cutscene_hooks_get_their_data_and_the_caller_is_not_frozen():
    outer = unit(("global", 40), "[0000] (0A) startScript(50,[]);\n[0003] (1A) Var[99] = 1;\n"
                                 "[0008] (00) stopObjectCode();\n")
    caller = unit(("global", 50), "[0000] (40) cutscene([2]);\n[0005] (1A) Var[100] = 1;\n"
                                  "[000A] (C0) endCutscene();\n[000B] (00) stopObjectCode();\n")
    start = unit(("global", 18), "[0000] (1A) Var[110] = Local[0];\n[0005] (60) freezeScripts(127);\n"
                                 "[0007] (00) stopObjectCode();\n")
    end = unit(("global", 19), "[0000] (1A) Var[111] = Local[0];\n[0005] (60) freezeScripts(0);\n"
                               "[0007] (00) stopObjectCode();\n")
    facts = {"var:35": 18, "var:36": 19, "script:70": 1, "cutscene": 0}
    p = only(calls(outer, caller, start, end).run(FactOracle(facts), ("global", 40), 0))
    assert p.writes["var:110"] == 2 and p.writes["var:111"] == 2  # the cutscene's list, then the level's data
    assert p.writes["var:100"] == 1  # beginCutscene's caller goes back to 0: it resumes
    assert p.writes["var:99"] == 1 and p.end == "end"  # 40 was frozen, then unfrozen before 50 ended
    assert p.writes["cutscene"] == 0 and p.writes["frozen:script:70"] == 0  # frozen, then unfrozen (exact count)
    m = calls(unit(("global", 51), "[0000] (C0) endCutscene();\n[0001] (00) stopObjectCode();\n"), end)
    p = only(m.run(FactOracle({"var:36": 19, "cutscene": 1, "script:70": 1, "frozen:script:70": 1}), ("global", 51), 0))
    assert isinstance(p.writes["var:111"], Unknown) and p.writes["var:111"].reason == model.R_CUTDATA
    assert isinstance(p.writes["frozen:script:70"], Unknown)  # frozen before the move: its count is not a fact


def test_chain_script_takes_the_callers_place():
    outer = unit(("global", 40), "[0000] (0A) startScript(50,[]);\n[0003] (1A) Var[99] = 1;\n"
                                 "[0008] (00) stopObjectCode();\n")
    chain = unit(("global", 50), "[0000] (42) chainScript(60,[5]);\n[0006] (1A) Var[100] = 1;\n")
    target = unit(("global", 60), "[0000] (1A) Var[110] = Local[0];\n[0005] (80) breakHere();\n"
                                  "[0006] (00) stopObjectCode();\n")
    p = only(calls(outer, chain, target).run(FactOracle({}), ("global", 40), 0))
    assert p.writes["var:110"] == 5 and "var:100" not in p.writes and p.writes["var:99"] == 1
    assert p.writes["script:50"] == 0 and p.writes["script:60"] == 1 and p.writes["at:script:60:6"] == 1
    assert p.turns == [("script:60", "yield"), ("script:50", "chained"), ("script:40", "end")]


def test_start_object_runs_its_entry_after_stopping_it():
    obj = unit(("obj", 400), "[0000] (1A) Var[100] = Local[0];\n[0005] (80) breakHere();\n"
                             "[0006] (00) stopObjectCode();\n")
    obj.entries = {11: 0}
    caller = unit(("global", 50), "[0000] (37) startObject(400,11,[9]);\n[0005] (1A) Var[101] = 1;\n")
    facts = {"owner:400": 15, "room": 5, "objscript:400": 1, "at:obj:400:30": 1}
    p = only(calls(obj, caller, object_room={400: 5}).run(FactOracle(facts), ("global", 50), 0))
    assert p.writes["var:100"] == 9 and p.writes["objscript:400"] == 1 and p.writes["var:101"] == 1
    assert p.writes["at:obj:400:30"] == 0 and p.writes["at:obj:400:6"] == 1


def test_a_callee_that_stops_its_caller_ends_the_callers_turn():
    outer = unit(("global", 40), "[0000] (0A) startScript(50,[]);\n[0003] (1A) Var[99] = 1;\n"
                                 "[0008] (00) stopObjectCode();\n")
    caller = unit(("global", 50), "[0000] (0A) startScript(60,[]);\n[0003] (1A) Var[100] = 1;\n")
    stopper = unit(("global", 60), "[0000] (62) stopScript(50);\n[0002] (00) stopObjectCode();\n")
    p = only(calls(outer, caller, stopper).run(FactOracle({}), ("global", 40), 0))
    assert "var:100" not in p.writes and p.writes["script:50"] == 0 and p.writes["var:99"] == 1
    assert ("script:50", "stopped") in p.turns


def test_nesting_past_the_engines_limit_is_counted():
    u = unit(("global", 60), "[0000] (4A) startScript(60,[],R);\n[0004] (80) breakHere();\n")
    p = only(calls(u).run(FactOracle({}), ("global", 60), 0))
    assert p.end == "nesting" and p.depth == model.MAX_NESTING and p.errors[0][0] == "nesting"
    assert p.reentries == model.MAX_NESTING - 1


def test_forks_in_a_callee_merge_where_they_meet():
    caller = unit(("global", 50), "[0000] (0A) startScript(60,[]);\n[0003] (1A) Var[101] = Var[110];\n"
                                  "[0008] (00) stopObjectCode();\n")
    callee = unit(("global", 60), """
[0000] (C8) Var[100] = getRandomNr(3);
[0005] (48) if (Var[100] == 1) /*0014*/ {
[000C] (1A)   Var[110] = 1;
[0011] (18) } else /*0019*/ {
[0014] (1A)   Var[110] = 2;
[0019] (**) }
[0019] (80) breakHere();
""")
    r = calls(caller, callee).run(FactOracle({}), ("global", 50), 0)
    p = only(r)  # the two sides meet after the if and go on as one path
    assert p.writes["var:110"].values == frozenset({1, 2}) and p.writes["var:101"].values == frozenset({1, 2})
    assert r.merges == 1 and len(p.assumptions) == 2
    pred = model.predict(r, FactOracle({}))
    assert not pred["var:110"]["certain"] and pred["var:110"]["values"] == [1, 2]


def test_a_loop_that_never_yields_on_one_side_spins():
    u = unit(("global", 60), """
[0000] (D6) VAR_RESULT = getActorMoving(3);
[0004] (28) unless (VAR_RESULT) goto 0000;
[0009] (80) breakHere();
""")
    r = calls(u).run(FactOracle({}), ("global", 60), 0)
    assert any(p.end == "spin" for p in r.paths) and len(r.paths) <= 3
    assert all(p.end in ("spin", "yield") for p in r.paths)


def test_an_engine_error_ends_the_path_and_a_warning_does_not():
    """stopScript on a slot with an open cutscene is the engine's error() (script.cpp:273-275): the game
    would abort, so the path ends as "error" and the predictions leave it out; runObjectScript on an
    object that is nowhere is a warning() (script.cpp:122-125): counted, and the path goes on."""
    u = unit(("global", 50), "[0000] (40) cutscene([2]);\n[0005] (62) stopScript(50);\n[0007] (00) stopObjectCode();\n")
    r = calls(u).run(FactOracle({"cutscene": 0}), ("global", 50), 0)
    p = only(r)
    assert p.end == "error" and p.errors[0][0] == "stopped with an open cutscene"
    assert model.feasible(r) == [] and model.predict(r, FactOracle({})) == {}
    w = unit(("global", 51), "[0000] (37) startObject(400,11,[]);\n[0005] (1A) Var[100] = 1;\n[000A] (00) stopObjectCode();\n")
    p = only(calls(w).run(FactOracle({"owner:400": 0}), ("global", 51), 0))
    assert p.end == "end" and p.errors[0][0] == "object not here" and p.writes["var:100"] == 1


def test_a_room_change_by_the_camera_runs_the_scroll_hook():
    """setCameraFollows calls setCameraAt after it changes the room (camera.cpp:69-74), so the scroll
    hook runs on that side; on the side where the actor is in the room it depends on a position."""
    u = unit(("global", 50), "[0000] (D2) actorFollowCamera(VAR_EGO);\n[0002] (00) stopObjectCode();\n")
    scroll = unit(("global", 6), "[0000] (1A) Var[120] = 1;\n[0005] (00) stopObjectCode();\n")
    r = calls(u, scroll).run(FactOracle({"var:27": 6, "var:1": 1}), ("global", 50), 0)
    sides = {p.assumptions[0][2]: p for p in r.paths}
    assert sides[False].writes.get("var:120") == 1 and "var:120" not in sides[True].writes
    assert any("scroll hook" in what for _, _, what in sides[True].notes)  # a position decides: a note


def test_f_flags_of_local_scripts_are_kept_per_room():
    """A local number is the local of the room in use (script.cpp:78-81): its F flag comes from the
    start sites that can reach that room's script, not from every room's."""
    a = unit(("local", 4, 200), "[0000] (4A) startScript(201,[],F);\n[0004] (00) stopObjectCode();\n")
    b = unit(("local", 5, 202), "[0000] (0A) startScript(201,[]);\n[0003] (00) stopObjectCode();\n")
    c4 = unit(("local", 4, 201), "[0000] (00) stopObjectCode();\n")
    c5 = unit(("local", 5, 201), "[0000] (00) stopObjectCode();\n")
    g = game(a, b, c4, c5)
    g.local_rooms = {200: {4}, 201: {4, 5}, 202: {5}}
    g.resistance, g.wild_starts = g._resistance()
    assert g.resistant("local:201", 4) is True and g.resistant("local:201", 5) is False
    assert g.resistant("local:201", Unknown(model.R_WORK)) is None  # the room is not known
    glob = unit(("global", 60), "[0000] (0A) startScript(201,[]);\n[0003] (00) stopObjectCode();\n")
    g = game(a, b, c4, c5, glob)
    g.local_rooms = {200: {4}, 201: {4, 5}, 202: {5}}
    g.resistance, g.wild_starts = g._resistance()
    assert g.resistant("local:201", 4) is None and g.resistant("local:201", 5) is False  # global code: every room


def test_value_sets_past_the_cap_are_counted():
    model.VALUE_CAPS.clear()
    v = model._arith("+", model.one_of(range(10), "r"), model.one_of(range(10), "r"))
    assert isinstance(v, Unknown) and v.values is None and model.VALUE_CAPS["arithmetic"] == 1
    assert model.one_of(range(model.MAX_VALUES + 1), "r").values is None and model.VALUE_CAPS["value set"] == 1
    r = run("[0000] (1A) Var[100] = 1;\n[0005] (00) stopObjectCode();\n")
    assert r.value_caps == {}  # reset per run


def test_find_inventory_is_one_of_the_owners_objects():
    u = unit(("global", 50), """
[0000] (FD) Local[2] = findInventory(3,1);
[0005] (A8) if (Local[2]) /*0013*/ {
[000A] (1A)   Var[100] = 1;
[0013] (**) }
[0013] (9A) Var[101] = Local[2];
[0018] (FD) Local[3] = findInventory(3,3);
[001D] (9A) Var[102] = Local[3];
[0022] (00) stopObjectCode();
""")
    p = only(calls(u).run(FactOracle({"owner:400": 3, "owner:401": 3, "owner:402": 15}), ("global", 50), 0))
    assert p.writes["var:100"] == 1 and not p.assumptions  # every owned object is not 0: no fork
    assert p.writes["var:101"].values == frozenset({400, 401}) and p.writes["var:102"] == 0  # past the count: 0


def room_units():
    local = unit(("local", 4, 200), "[0000] (72) loadRoom(5);\n[0002] (1A) Var[100] = 1;\n")
    caller = unit(("global", 50), "[0000] (0A) startScript(200,[]);\n[0003] (1A) Var[101] = 1;\n"
                                  "[0008] (00) stopObjectCode();\n")
    exit_hook = unit(("global", 7), "[0000] (9A) Var[110] = VAR_NEW_ROOM;\n[0005] (9A) Var[111] = VAR_ROOM;\n"
                                    "[000A] (00) stopObjectCode();\n")
    exit_code = unit(("exit", 4), "[0000] (1A) Var[112] = 1;\n[0005] (00) stopObjectCode();\n")
    entry_hook = unit(("global", 5), "[0000] (9A) Var[113] = VAR_ROOM;\n[0005] (00) stopObjectCode();\n")
    entry_code = unit(("entry", 5), "[0000] (1A) Var[114] = 1;\n[0005] (80) breakHere();\n"
                                    "[0006] (00) stopObjectCode();\n")
    return local, caller, exit_hook, exit_code, entry_hook, entry_code


ROOM_FACTS = {"room": 4, "var:4": 4, "var:22": 4, "var:30": 7, "var:28": 5, "local:201": 1, "at:local:201:3": 1,
              "script:70": 1, "var:25": 3}


def test_room_change_runs_exit_kills_and_entry_in_order():
    g = game(*room_units())
    g.room_width = {5: 640}
    m = Machine(ROW2, game=g)
    p = only(m.run(FactOracle(ROOM_FACTS), ("global", 50), 0))
    assert p.writes["var:110"] == 5 and p.writes["var:111"] == 4  # VAR_NEW_ROOM before the exit hook, VAR_ROOM after
    assert p.writes["var:112"] == 1  # the old room's exit code
    assert p.writes["local:201"] == 0 and p.writes["at:local:201:3"] == 0 and "script:70" not in p.writes  # kills
    assert p.writes["var:113"] == 5 and p.writes["var:114"] == 1 and p.writes["at:entry:6"] == 1  # entry, after load
    assert (p.writes["room"], p.writes["var:4"], p.writes["var:22"], p.writes["var:72"]) == (5, 5, 5, 5)
    assert p.writes["var:17"] == 160 and p.writes["var:18"] == 480 and p.writes["var:25"] == 255
    assert p.writes["var:70"].values == frozenset({0, 1})  # 1 when the resource had to be loaded: not a fact
    assert "var:100" not in p.writes and ("local:200", "room") in p.turns  # the local slot that changed it dies
    assert p.writes["var:101"] == 1 and p.end == "end"  # its global caller goes on
    # the killed local slot's turn ends when the opcode returns, after the entry scripts (room.cpp:254)
    assert p.turns == [("script:7", "end"), ("exit", "end"), ("script:5", "end"), ("entry", "yield"),
                       ("local:200", "room"), ("script:50", "end")]


def test_room_change_maps_a_pseudo_room_and_row_1_leaves_it_out():
    units = room_units()
    g = game(*units)
    g.pseudo = {200: 5}
    local = unit(("local", 4, 200), "[0000] (72) loadRoom(200);\n[0002] (1A) Var[100] = 1;\n")
    g.units[local.key] = local
    p = only(Machine(ROW2, game=g).run(FactOracle(ROOM_FACTS), ("global", 50), 0))
    assert p.writes["room"] == 200 and p.writes["var:22"] == 5 and p.writes["var:114"] == 1
    p = only(Machine(ROW1, game=g).run(FactOracle(ROOM_FACTS), ("global", 50), 0))
    assert p.writes["room"] == 200 and "var:110" not in p.writes and "var:114" not in p.writes  # row 1: no scripts
    assert "var:100" not in p.writes and p.writes["var:101"] == 1  # the local slot still ends, its caller goes on


def test_frozen_facts_from_the_dump():
    import facts as F
    dump = {"vars": [0] * 4, "bits_set": [], "owners": {}, "states": {}, "classes": {}, "room": 1, "ego": 1,
            "slots": [{"where": 2, "number": 9, "code": 4, "frozen": 1}, {"where": 3, "number": 200, "code": 2,
                                                                          "frozen": 0},
                      {"where": 1, "number": F.ENCD, "code": 6, "frozen": 2}],
            "cutscene": {"depth": 0, "levels": [{"override": None}]}, "verbs": []}
    f = F.facts_of(dump)
    assert f["frozen:script:9"] == 1 and f["frozen:entry"] == 1 and "frozen:local:200" not in f


# --- rows 1 and 2 over the real IR -------------------------------------------------------------------------------

def test_a_real_sentence_picks_up_the_clicked_object_only_without_the_animation(real):
    """The sentence script (the constant the scripts write to VAR_SENTENCE_SCRIPT) sets VAR_ME to object A
    before it starts A's entry, so a pick-up entry that picks up VAR_ME takes A, on the paths that get
    there in the first turn. An object of a class that makes the sentence script play a pick-up animation
    first (MI1's script 2 [033D]: classOfIs(Local[1],[143]), class 15, then cutscene and delay(20) at
    [0385]-[038F]) yields at that delay, so no path of the first turn reaches the pick-up. The three Part I
    pick-ups the task names (S1#38, S2#5, S1#47) have that class at pre: the check does not hold at row 1."""
    n = next(x.st.rhs.value for u in real.units.values() for x in u.ins if isinstance(x.st, ir.Assign)
             and x.st.lhs == ir.Var(model.VAR_SENTENCE_SCRIPT) and isinstance(x.st.rhs, ir.Const) and x.st.rhs.value)
    m = Machine(ROW1, game=real)
    for k, u in real.units.items():
        if k[0] != "obj" or 9 not in u.entries:
            continue
        x = u.ins[u.entries[9]]
        if isinstance(x.st, ir.Op) and x.st.name == "pickupObject" and x.st.args[0] == ir.Var(7):
            o = k[1]
            facts = {"var:33": n, "var:1": 1, "room": u.room, f"owner:{o}": 15}
            run = m.run_sentence(FactOracle(facts), 9, o)
            picks = [p for p in run.paths if f"owner:{o}" in p.writes]
            assert picks and all(p.writes[f"owner:{o}"] == 1 and p.writes["var:7"] == o for p in picks)
            assert not any(key.startswith("owner:") and key != f"owner:{o}" for p in run.paths for key in p.writes)
            run = m.run_sentence(FactOracle(facts | {f"class:{o}:15": 1}), 9, o)
            assert run.paths and not any(f"owner:{o}" in p.writes or "var:7" in p.writes for p in run.paths)
            delay = real.units[("global", n)].ins
            after_delay = {delay[i + 1].offset for i, y in enumerate(delay[:-1]) if y.st is not None
                           and ir.head(y.st) == "delay"}
            assert any(p.end == "yield" and any(p.writes.get(f"at:script:{n}:{w}") == 1 for w in after_delay)
                       for p in run.paths)  # the path that walked and reached it waits after the delay
            return
    pytest.skip("no object picks up VAR_ME as its first statement")


def test_an_exit_script_that_yields_dies_with_the_room():
    """killScriptsAndResources kills room slots, the exit code's too (script.cpp:1089-1096): an exit
    script that yielded in this path does not outlive the room change (facts.py: at:exit never shows)."""
    local, caller, exit_hook, _, entry_hook, entry_code = room_units()
    exit_code = unit(("exit", 4), "[0000] (1A) Var[112] = 1;\n[0005] (80) breakHere();\n[0006] (00) stopObjectCode();\n")
    g = game(local, caller, exit_hook, exit_code, entry_hook, entry_code)
    g.room_width = {5: 640}
    p = only(Machine(ROW2, game=g).run(FactOracle(ROOM_FACTS), ("global", 50), 0))
    assert p.writes["var:112"] == 1 and ("exit", "yield") in p.turns
    assert p.writes["at:exit:6"] == 0  # yielded, then killed by the room change that ran it


def test_every_room_code_runs_without_an_unmodelled_statement(real):
    m = Machine(ROW2, game=real)
    ends = collections.Counter()
    for k in real.units:
        if k[0] in ("entry", "exit"):
            for p in m.run(FactOracle({"var:1": 1, "room": k[1], "var:22": k[1]}), k, 0).paths:
                ends[p.end] += 1
    assert ends and "unmodelled" not in ends


# --- row 3: settle (hand-made units) -------------------------------------------------------------------------------

def settle(*units: ir.Unit, **kw) -> Machine:
    g = game(*units, object_room=kw.pop("object_room", None))
    return Machine(ROW3, game=g, **kw)


SENTENCE_WALKS = """
[0000] (FE) walkActorTo(VAR_EGO,100,100);
[0007] (AE) WaitForActor(VAR_EGO);
[000A] (9A) Var[100] = Local[1];
[000F] (00) stopObjectCode();
"""


def test_loop_kinds_follow_what_the_exit_test_reads():
    """A loop's kind is what its exit tests' values depend on (Game._test_reads): a position read, even of
    the ego, is time; a bit is a fact; a counter the loop steps is time; no exit at all is ambient."""
    u = unit(("global", 60), """
[0000] (80) breakHere();
[0001] (B4) VAR_RESULT = getDist(VAR_EGO,575);
[0008] (78) unless (VAR_RESULT < 3) goto 0000;
[000F] (80) breakHere();
[0010] (A8) unless (Bit[137]) goto 000F;
[0015] (1A) Local[0] = 0;
[001A] (80) breakHere();
[001B] (46) Local[0]++;
[001E] (44) unless (Local[0] > 10) goto 001A;
[0025] (80) breakHere();
[0026] (18) goto 0025;
""")
    g = game(u)
    kinds = {lp.head: (lp.kind, lp.reads, lp.non_facts) for lp in g.loops(("global", 60))}
    assert kinds[0] == ("time", frozenset(), frozenset({model.R_POSITION}))
    assert kinds[3] == ("facts", frozenset({"bit:137"}), frozenset())
    assert kinds[6][0] == "time" and "counter" in kinds[6][2]
    assert kinds[9][0] == "ambient"
    assert g.wait_point(("global", 60), 1).kind == "frame" and g.wait_point(("global", 60), 1).loop.kind == "time"
    assert g.wait_point(("global", 60), 4).loop.reads == frozenset({"bit:137"})
    assert set(g.time_exits(("global", 60))) == {2, 8}  # the exits of the two time-only loops


def test_settle_resumes_a_started_slot_with_its_locals():
    """The sentence script blocks at WaitForActor in the click's frame on one side; settle resumes it from
    its kept Frame, with its locals [verb, A, B], and the wait falls through (R_WAIT_OVER)."""
    s2 = unit(("global", 2), SENTENCE_WALKS)
    r = settle(s2).run_sentence(FactOracle({"var:33": 2}), 9, 400)
    assert {p.first_end for p in r.paths} <= {"yield", "end"} and all(p.end == "settled" for p in r.paths)
    pred = model.predict(r, FactOracle({"var:33": 2}))
    assert pred["var:100"]["certain"] and pred["var:100"]["values"] == [400]  # A, from the kept local
    assert pred["script:2"]["certain"] and pred["script:2"]["values"] == [0]
    blocked = next(p for p in r.paths if p.first_end == "yield")
    assert blocked.immediate.get("at:script:2:7") == 1 and "var:100" not in blocked.immediate  # before the yield
    assert any(a[3].startswith(f"?({model.R_WAIT_OVER}") for a in blocked.assumptions)


def test_row_3_keeps_row_2s_immediate_part():
    """With no other live slot, row 3's click frame is row 2's run, plus the mouse variables, which the click
    moved (updateScummVars, scumm.cpp:3198, 3385-3388): unknown from the click's frame on."""
    s2 = unit(("global", 2), SENTENCE_WALKS)
    facts = {"var:33": 2}
    row2 = Machine(ROW2, game=game(s2)).run_sentence(FactOracle(facts), 9, 400)
    row3 = settle(s2).run_sentence(FactOracle(facts), 9, 400)
    mouse = {f"var:{v}" for v in model.MOUSE_VARS}
    assert all(isinstance(p.immediate[k], Unknown) and p.immediate[k].reason == model.R_INPUT
               for p in row3.paths for k in mouse)
    assert sorted(map(repr, (p.writes for p in row2.paths))) == \
        sorted(map(repr, ({k: v for k, v in p.immediate.items() if k not in mouse} for p in row3.paths)))
    assert not any(k in p.writes for p in model.feasible(settle(s2).run_wait(FactOracle(facts))) for k in mouse)


def test_a_time_only_loop_is_taken_to_exit():
    """A slot live at pre in a loop whose exit test reads a position runs (rule b) and leaves the loop: the
    model assumes time satisfies it, and records the assumption with what the test read."""
    poll = unit(("global", 60), """
[0000] (80) breakHere();
[0001] (C3) VAR_RESULT = getActorX(VAR_EGO);
[0006] (44) unless (VAR_RESULT > 228) goto 0000;
[000D] (1A) Var[110] = 1;
[0012] (00) stopObjectCode();
""")
    facts = {"script:60": 1, "at:script:60:1": 1}
    p = only(settle(poll).run_wait(FactOracle(facts)))
    assert p.writes["var:110"] == 1 and p.writes["script:60"] == 0 and p.end == "settled"
    assert p.ran_by == {"script:60": "b: a time-only loop"}
    exits = [a for a in p.assumptions if a[3].startswith(f"?({model.R_LOOP_EXIT}")]
    assert len(exits) == 1 and model.R_POSITION in exits[0][3]


def test_a_loop_is_taken_to_exit_only_where_the_slot_waited():
    """An outer loop whose only yield is in an inner time-only loop: the slot resumed in the inner loop
    leaves it once, goes round the outer loop and, entering the inner loop afresh, tests it as usual, so
    one side yields there again instead of going round forever (a spin, every path left out)."""
    u = unit(("global", 60), """
[0000] (1A) Var[117] = 1;
[0005] (D6) VAR_RESULT = getActorMoving(3);
[0009] (28) unless (VAR_RESULT) goto 0000;
[000E] (80) breakHere();
[000F] (18) goto 0005;
""")
    facts = {"script:60": 1, "at:script:60:15": 1}
    r = settle(u).run_wait(FactOracle(facts))
    live = model.feasible(r)
    assert live and all(p.writes["var:117"] == 1 and p.writes["at:script:60:15"] == 1 for p in live)
    assert any(p.end == "spin" for p in r.paths)  # the side that went round again with no yield
    assert sum(a[3].startswith(f"?({model.R_LOOP_EXIT}") for p in live for a in p.assumptions) >= 1


def test_a_fact_poller_runs_once_the_path_changes_its_fact():
    poll = unit(("global", 61), """
[0000] (80) breakHere();
[0001] (A8) unless (Bit[137]) goto 0000;
[0006] (1A) Var[111] = 1;
[000B] (00) stopObjectCode();
""")
    s2 = unit(("global", 2), "[0000] (1A) Bit[137] = 1;\n[0005] (00) stopObjectCode();\n")
    facts = {"var:33": 2, "script:61": 1, "at:script:61:1": 1}
    p = only(settle(poll, s2).run_wait(FactOracle(facts)))  # a wait changes no fact: the poller is not run
    assert "var:111" not in p.writes and p.not_run == {"script:61": "polls facts the path has not changed"}
    p = only(settle(poll, s2).run_sentence(FactOracle(facts), 9, 400))  # the sentence sets the bit (rule c)
    assert p.writes["var:111"] == 1 and p.ran_by["script:61"] == "c: polls facts the path changed"


def test_an_ambient_loop_runs_once_and_others_stay():
    """An ambient loop runs once, then the cap passes it over (counted). The engine runs it every frame, so
    what its pass wrote and where it stands become unknown at settle's end, with the values seen: possible
    predictions, not certain ones."""
    ambient = unit(("global", 62), "[0000] (80) breakHere();\n[0001] (46) Var[112]++;\n[0004] (18) goto 0000;\n")
    parked = unit(("global", 63), "[0000] (0A) startScript(64,[]);\n[0003] (1A) Var[113] = 1;\n"
                                  "[0008] (00) stopObjectCode();\n")
    facts = {"script:62": 1, "at:script:62:1": 1, "var:112": 5, "script:63": 1, "at:script:63:3": 1,
             "frozen:script:63": 1}
    p = only(settle(ambient, parked).run_wait(FactOracle(facts)))
    assert p.end == "settled" and p.ambient_slots == {"script:62"}  # one iteration, then it is not run again
    assert p.writes["var:112"] == Unknown(model.R_SETTLE, p.writes["var:112"].what, frozenset({5, 6}))
    assert p.writes["at:script:62:1"].values == frozenset({0, 1})
    pred = model.predict(model.Run([p], ""), FactOracle(facts))
    assert not pred["var:112"]["certain"] and pred["var:112"]["values"] == [5, 6]
    assert p.not_run == {"script:63": "frozen"} and "var:113" not in p.writes
    assert p.skips == {("script:63", "pre, first turn", "frozen"),
                       ("script:62", "pre, ran before", "the ambient cap: its loop with no exit ran once")}
    p = only(settle(ambient, parked).run_wait(FactOracle(facts | {"frozen:script:63": 0})))
    assert p.writes["var:113"] == 1 and p.ran_by["script:63"].startswith("b: parked")  # waits for nothing


def test_settle_stops_at_a_cycle_and_at_the_round_limit():
    """A slot the path started that changes a fact every round never reaches a fixed point: a state met
    again ends the path as a cycle, the facts that vary made unknown; a fact that keeps changing ends it
    at the round limit, made unknown (R_SETTLE)."""
    toggles = unit(("global", 2), """
[0000] (1A) Var[115] = 1;
[0005] (80) breakHere();
[0006] (1A) Var[115] = 2;
[000B] (80) breakHere();
[000C] (A8) unless (Bit[7]) goto 0000;
[0011] (00) stopObjectCode();
""")
    p = only(settle(toggles).run_sentence(FactOracle({"var:33": 2}), 9, 400))
    assert p.end == "cycle" and p.writes["var:115"].values == frozenset({1, 2})
    counts = unit(("global", 2), """
[0000] (46) Var[114]++;
[0003] (80) breakHere();
[0004] (A8) unless (Bit[7]) goto 0000;
[0009] (00) stopObjectCode();
""")
    p = only(settle(counts, round_limit=5).run_sentence(FactOracle({"var:33": 2}), 9, 400))
    assert p.end == "round limit" and p.round == 5
    assert isinstance(p.writes["var:114"], Unknown) and p.writes["var:114"].reason == model.R_SETTLE


def test_queued_sentences_run_at_a_rounds_end():
    """doSentence queues a sentence; checkAndRunSentenceScript runs it at the end of the next round with
    [verb, A, B] (script.cpp:1174-1247). A frozen queued sentence waits for the unfreeze."""
    s2 = unit(("global", 2), """
[0000] (9A) Var[100] = Local[0];
[0005] (48) if (Local[0] == 9) /*0013*/ {
[000C] (00)   stopObjectCode();
[000D] (18) } else /*0013*/ {
[0010] (**) }
[0013] (F9) doSentence(9,Local[1],0);
[001A] (00) stopObjectCode();
""")
    p = only(settle(s2).run_sentence(FactOracle({"var:33": 2}), 4, 400))
    assert p.writes["var:100"] == 9 and p.sentences == 1 and not p.queue and p.end == "settled"


def test_order_conflicts_are_counted():
    w = unit(("global", 64), "[0000] (80) breakHere();\n[0001] (1A) Var[120] = 1;\n[0006] (00) stopObjectCode();\n")
    rd = unit(("global", 65), "[0000] (80) breakHere();\n[0001] (9A) Var[121] = Var[120];\n"
                              "[0006] (00) stopObjectCode();\n")
    facts = {"script:64": 1, "at:script:64:1": 1, "script:65": 1, "at:script:65:1": 1}
    p = only(settle(w, rd).run_wait(FactOracle(facts)))
    assert p.writes["var:121"] == 1  # the slots live at pre run in the order of their keys
    assert [(c[1], c[2], c[3]) for c in p.conflicts] == [("script:64", "script:65", ("var:120",))]


def test_forks_that_meet_inside_a_turn_merge_there():
    """Rows 1-2's join rule inside a settle round: the two sides of a fork meet after the if."""
    s2 = unit(("global", 2), """
[0000] (16) Local[3] = getRandomNr(5);
[0004] (48) if (Local[3] == 1) /*0010*/ {
[000B] (1A)   Var[116] = 1;
[0010] (**) }
[0010] (80) breakHere();
[0011] (80) breakHere();
[0012] (00) stopObjectCode();
""")
    r = settle(s2).run_sentence(FactOracle({"var:33": 2}), 9, 400)
    p = only(r)
    assert r.settle_merges == 0 and p.writes["var:116"].values == frozenset({0, 1}) and p.end == "settled"


def test_paths_with_the_same_slot_positions_merge_at_a_rounds_start():
    """At a round's start paths whose slot facts and kept Frames' positions agree become one: the same state
    when their stores agree too, else the facts and locals that differ become value sets; the merged path's
    fixed-point test then compares with the merged store."""
    s2 = unit(("global", 2), "[0000] (80) breakHere();\n[0001] (00) stopObjectCode();\n")
    m = settle(s2)
    m.oracle = FactOracle({"var:33": 2})
    m._merges = m._settle_merges = m._value_merges = 0
    frame = model.Frame(("global", 2), 1, {3: 7}, True, sid=2)
    paths = []
    for v, local in ((1, 7), (2, 8)):
        p = model.Path([model.Call(model.ROUND, "test", (("sentences",),), caller=None, active=False)], settle=True,
                       round=1, immediate={}, writes={"script:2": 1, "at:script:2:1": 1, "var:116": v})
        p.slots = {"script:2": dataclasses.replace(frame, locals={3: local})}
        p.history = [(0, dict(p.writes))]
        paths.append(p)
    sigs = {m._join(p) for p in paths}
    assert len(sigs) == 1 and next(iter(sigs))[0] == "round"  # same positions: one join
    q = m._merge(paths, sigs.pop())
    assert q.writes["var:116"].values == frozenset({1, 2}) and q.slots["script:2"].locals[3].values == frozenset({7, 8})
    assert q.round_start == q.writes and m._settle_merges == 1 and m._value_merges == 1
    paths[1].writes["at:script:2:1"], paths[1].writes["at:script:2:5"] = 0, 1  # another wait point: no merge
    assert m._join(paths[0]) != m._join(paths[1])


def test_a_known_loop_test_decides_and_only_an_unknown_one_is_taken_to_exit():
    """A counter the path set is known: the loop runs until the counter leaves it, with no assumed exit
    (R_LOOP_EXIT is only for a test whose value is not known)."""
    s2 = unit(("global", 2), """
[0000] (1A) Local[0] = 0;
[0005] (80) breakHere();
[0006] (46) Local[0]++;
[0009] (9A) Var[122] = Local[0];
[000E] (44) unless (Local[0] > 2) goto 0005;
[0015] (1A) Var[121] = 1;
[001A] (00) stopObjectCode();
""")
    p = only(settle(s2).run_sentence(FactOracle({"var:33": 2}), 9, 400))
    assert p.writes["var:122"] == 3 and p.writes["var:121"] == 1 and p.end == "settled"
    assert not any(a[3].startswith(f"?({model.R_LOOP_EXIT}") for a in p.assumptions)


def test_a_pre_slot_that_ran_runs_every_round_and_meets_a_new_poll_on_the_facts():
    """Rules b and c gate a slot live at pre only at its pre wait point. Once it has run it runs every frame
    (script.cpp:978-979): a poll it reaches later is tested on the path's facts, not blocked because the path
    did not change them."""
    u = unit(("global", 66), """
[0000] (AE) WaitForActor(3);
[0002] (80) breakHere();
[0003] (A8) unless (Bit[137]) goto 0002;
[0008] (1A) Var[118] = 1;
[000D] (00) stopObjectCode();
""")
    facts = {"script:66": 1, "at:script:66:0": 1, "bit:137": 1}
    p = only(settle(u).run_wait(FactOracle(facts)))
    assert p.writes["var:118"] == 1 and p.writes["script:66"] == 0 and p.ran_by == {"script:66": "b: a WaitFor op"}
    assert not p.skips


def test_a_freeze_uses_the_kept_slots_own_f_flag_and_every_skip_is_kept():
    """A slot the path started knows its F flag: a freeze leaves it running even when the start sites
    disagree; a slot started without F is frozen, and the round that passes it over keeps the skip (origin
    "path"). A slot that froze another one run later in the same round is an order conflict."""
    s2 = unit(("global", 2), """
[0000] (2A) startScript(70,[],F);
[0003] (0A) startScript(72,[]);
[0006] (80) breakHere();
[0007] (60) freezeScripts(1);
[0009] (80) breakHere();
[000A] (00) stopObjectCode();
""")
    s70 = unit(("global", 70), "[0000] (80) breakHere();\n[0001] (1A) Var[119] = 1;\n[0006] (80) breakHere();\n"
                               "[0007] (1A) Var[119] = 2;\n[000C] (00) stopObjectCode();\n")
    s72 = unit(("global", 72), "[0000] (80) breakHere();\n[0001] (1A) Var[126] = 1;\n[0006] (00) stopObjectCode();\n")
    m = settle(s2, s70, s72)
    m.game.resistance = {("global", 70): {True, False}}  # started both ways elsewhere: the start sites do not say
    p = only(m.run_sentence(FactOracle({"var:33": 2}), 9, 400))
    assert p.writes["var:119"] == 2 and p.writes["script:70"] == 0  # not frozen: it ran to its end
    assert "var:126" not in p.writes and p.writes["frozen:script:72"] == 1
    assert ("script:72", "path", "frozen") in p.skips
    assert any(c[1:3] == ("script:2", "script:72") and "frozen:script:72" in c[3] for c in p.conflicts)


def test_a_camera_loop_exit_forks_once():
    """A loop waiting on the camera position is not taken to exit: one side leaves (the camera got there),
    the other stays for the rest of settle (R_CAMERA)."""
    u = unit(("global", 67), """
[0000] (9A) Local[1] = VAR_CAMERA_POS_X;
[0005] (80) breakHere();
[0006] (F8) unless (VAR_CAMERA_POS_X < Local[1]) goto 0005;
[000D] (1A) Var[123] = 1;
[0012] (00) stopObjectCode();
""")
    assert model.R_CAMERA in settle(u).game.loops(("global", 67))[0].non_facts
    r = settle(u).run_wait(FactOracle({"script:67": 1, "at:script:67:6": 1}))
    live = model.feasible(r)
    assert sorted(str(p.writes.get("var:123", "-")) for p in live) == ["-", "1"]
    stay = next(p for p in live if "var:123" not in p.writes)
    assert stay.camera_done == {("script:67", 1)} and stay.writes.get("at:script:67:6", 1) == 1
    assert all(any(a[3].startswith(f"?({model.R_CAMERA}") for a in p.assumptions) for p in live)
    assert not any(a[3].startswith(f"?({model.R_LOOP_EXIT}") for p in live for a in p.assumptions)


def test_a_second_instance_from_a_recursive_start_is_counted_and_left_open():
    """Two instances of one unit (startScript with R twice): the path tracks one Frame, so the unit's at: keys,
    and its slot fact once the tracked one ended, are unknown when settle ends."""
    s2 = unit(("global", 2), "[0000] (4A) startScript(73,[],R);\n[0004] (4A) startScript(73,[],R);\n"
                             "[0008] (00) stopObjectCode();\n")
    s73 = unit(("global", 73), "[0000] (80) breakHere();\n[0001] (80) breakHere();\n[0002] (00) stopObjectCode();\n")
    p = only(settle(s2, s73).run_sentence(FactOracle({"var:33": 2}), 9, 400))
    assert p.multi == {"script:73"} and p.end == "settled"
    assert p.writes["script:73"].values == frozenset({0, 1})
    assert all(p.writes[k].values == frozenset({0, 1}) for k in ("at:script:73:1", "at:script:73:2"))
    one = only(settle(unit(("global", 2), "[0000] (4A) startScript(73,[],R);\n[0004] (00) stopObjectCode();\n"),
                      s73).run_sentence(FactOracle({"var:33": 2}), 9, 400))
    assert not one.multi and one.writes["script:73"] == 0  # one instance: its end is certain
    s79 = unit(("global", 79), "[0000] (48) if (Local[0] == 0) /*000E*/ {\n[0007] (4A)   startScript(79,[1],R);\n"
                               "[000E] (**) }\n[000E] (00) stopObjectCode();\n")
    nested = only(settle(unit(("global", 2), "[0000] (4A) startScript(79,[],R);\n[0004] (00) stopObjectCode();\n"),
                         s79).run_sentence(FactOracle({"var:33": 2}), 9, 400))
    assert not nested.multi and nested.writes["script:79"] == 0  # the one on the stack ran, it is not a second slot


def test_the_clicks_frame_runs_the_other_slots_before_the_sentence():
    """runAllScripts comes before the sentence in the click's frame (scumm.cpp:3231, 3255): with settle a slot
    live at pre has its turn there, before the sentence script's writes, and its writes are part of the click's
    frame ("before the first yield"). Row 2 does not run it."""
    s2 = unit(("global", 2), "[0000] (1A) Var[100] = 9;\n[0005] (00) stopObjectCode();\n")
    s74 = unit(("global", 74), "[0000] (80) breakHere();\n[0001] (9A) Var[125] = Var[100];\n"
                               "[0006] (00) stopObjectCode();\n")
    facts = {"var:33": 2, "var:100": 5, "script:74": 1, "at:script:74:1": 1}
    p = only(settle(s2, s74).run_sentence(FactOracle(facts), 9, 400))
    assert p.immediate["var:125"] == 5 and p.immediate["var:100"] == 9 and p.immediate["script:74"] == 0
    assert p.ran_by["script:74"] == "b: a breakHere outside any loop"
    q = only(Machine(ROW2, game=game(s2, s74)).run_sentence(FactOracle(facts), 9, 400))
    assert "var:125" not in q.writes and "script:74" not in q.writes


def test_a_slot_started_nested_in_a_round_has_no_turn_of_its_own_there():
    """didexec (script.cpp:506-507, 979): a slot that a turn earlier in the frame restarted ran nested; it does
    not run again in that frame, though its name comes later in the round's order."""
    a = unit(("global", 77), "[0000] (80) breakHere();\n[0001] (0A) startScript(78,[]);\n[0004] (00) stopObjectCode();\n")
    b = unit(("global", 78), "[0000] (46) Var[127]++;\n[0003] (80) breakHere();\n[0004] (00) stopObjectCode();\n")
    facts = {"script:77": 1, "at:script:77:1": 1, "script:78": 1, "at:script:78:4": 1}
    p = only(settle(a, b).run_wait(FactOracle(facts)))
    assert p.writes["var:127"] == 1 and p.writes["script:78"] == 0
    assert p.turns[:3] == [("script:78", "yield"), ("script:77", "end"), ("script:78", "end")]
    assert p.round == 3  # 78's new instance ends in round 2, not in round 1 (round 3 changes nothing)


# --- row 3 over the real IR -----------------------------------------------------------------------------------------

def test_a_real_pickup_with_the_animation_reaches_the_pickup_after_settle(real):
    """Row 1's test above: an object of the class that makes the sentence script play a pick-up animation
    first yields at the animation's delay, so the click's frame never reaches the pick-up. With settle the
    delay ends in a later round and the sentence script goes on to the object's entry: the object is the
    ego's on every path that got there."""
    n = next(x.st.rhs.value for u in real.units.values() for x in u.ins if isinstance(x.st, ir.Assign)
             and x.st.lhs == ir.Var(model.VAR_SENTENCE_SCRIPT) and isinstance(x.st.rhs, ir.Const) and x.st.rhs.value)
    m = Machine(ROW3, game=real)
    for k, u in real.units.items():
        if k[0] != "obj" or 9 not in u.entries:
            continue
        x = u.ins[u.entries[9]]
        if isinstance(x.st, ir.Op) and x.st.name == "pickupObject" and x.st.args[0] == ir.Var(7):
            o = k[1]
            facts = {"var:33": n, "var:1": 1, "room": u.room, f"owner:{o}": 15, f"class:{o}:15": 1}
            run = m.run_sentence(FactOracle(facts), 9, o)
            picks = [p for p in model.feasible(run) if f"owner:{o}" in p.writes]
            assert picks and all(p.writes[f"owner:{o}"] == 1 for p in picks)
            assert all(p.immediate is not None and f"owner:{o}" not in p.immediate for p in picks)  # after the yield
            return
    pytest.skip("no object picks up VAR_ME as its first statement")


def test_every_real_wait_point_has_a_kind(real):
    kinds = collections.Counter()
    for key, u in real.units.items():
        for i, x in enumerate(u.ins):
            if x.yields in ("frame", "delay") and x.next is not None and x.next < len(u.ins):
                kinds[real.wait_point(key, x.next).kind] += 1
    assert kinds["frame"] and kinds["delay"] and not kinds["other"]


# --- row 4: a dialog line is a click on its verb ---------------------------------------------------------------------

def dialog(*units: ir.Unit, **kw) -> Machine:
    g = game(*units, object_room=kw.pop("object_room", None))
    return Machine(ROW4, game=g, **kw)


INPUT = """
[0000] (9A) Var[195] = Local[0];
[0005] (9A) Var[196] = Local[2];
[000A] (9A) Var[194] = Local[1];
[000F] (00) stopObjectCode();
"""
POLL = """
[0000] (80) breakHere();
[0001] (A8) unless (Var[194]) goto 0000;
[0006] (48) if (Var[194] == 120) /*0012*/ {
[000D] (1A)   Bit[78] = 1;
[0012] (**) }
[0012] (00) stopObjectCode();
"""


def test_usable_dialog_line_needs_input_and_the_verb_and_rests_on_the_click():
    """checkExecVerbs (verbs.cpp:586-692): _userPut above 0 and the verb found under the click. The click's own
    conditions (the virtual screen and the verb's rectangle under it) are not facts: listed, not checked."""
    facts = {"var:53": 1, "verb:120": 1}
    u = actions.usable(actions.dialog_line(120), FactOracle(facts), game())
    assert u.value is True and sorted(c for c, _ in u.unchecked()) == sorted(actions.CLICK)
    assert actions.usable(actions.dialog_line(121), FactOracle(facts), game()).value is False
    assert actions.usable(actions.dialog_line(120), FactOracle(facts | {"var:53": 0}), game()).value is False
    assert set(actions.CLICK) <= set(actions.UNCHECKED)


def test_a_click_on_a_verb_runs_the_input_script_with_area_verb_and_button():
    """runInputScript(kVerbClickArea, verbid, 1) (verbs.cpp:666, 685-686; script.cpp:1478-1511): the input
    script installed at pre runs in the click's frame with [1, V, 1]; the mouse is unknown from the click on."""
    s14 = unit(("global", 14), INPUT)
    r = dialog(s14).run_verb(FactOracle({"var:32": 14, "var:53": 1, "verb:120": 1}), 120)
    p = only(r)
    assert r.unit == ("global", 14) and p.end == "settled"
    assert p.immediate["var:194"] == 120 and p.immediate["var:195"] == 1 and p.immediate["var:196"] == 1
    assert p.immediate["script:14"] == 0 and p.turns == [("script:14", "end")]
    assert all(p.writes[f"var:{v}"].reason == model.R_INPUT for v in model.MOUSE_VARS)
    p = only(dialog(s14).run_verb(FactOracle({"var:32": 0}), 120))  # no input script: nothing runs
    assert "var:194" not in p.writes and any("VAR_VERB_SCRIPT is 0" in n[2] for n in p.notes)


def test_the_script_waiting_on_the_menu_resumes_by_rule_c():
    """The script polling the value the input script writes is found by running the scripts: it is live at pre at
    a breakHere in a loop on a fact, so settle's rule c runs it once the path has changed that fact, from the
    round after the click's frame (runAllScripts comes before checkExecVerbs)."""
    s14, s55 = unit(("global", 14), INPUT), unit(("global", 55), POLL)
    facts = {"var:32": 14, "script:55": 1, "at:script:55:1": 1}
    p = only(dialog(s14, s55).run_verb(FactOracle(facts), 120))
    assert "bit:78" not in p.immediate and p.writes["bit:78"] == 1 and p.writes["script:55"] == 0
    assert p.ran_by["script:55"] == "c: polls facts the path changed"
    p = only(dialog(s14, s55).run_verb(FactOracle(facts), 121))  # another line: the script goes on without the bit
    assert "bit:78" not in p.writes and p.writes["script:55"] == 0


def test_a_room_local_input_script_and_a_room_local_waiting_script():
    """VAR_VERB_SCRIPT at or above _numGlobalScripts names the local script of the room in use (script.cpp:70-86),
    and a room-local script can be the one waiting on the menu: the same rules, no special case."""
    s209 = unit(("local", 51, 209), INPUT, room=51)
    s207 = unit(("local", 51, 207), POLL, room=51)
    facts = {"room": 51, "var:32": 209, "local:207": 1, "at:local:207:1": 1}
    r = dialog(s209, s207).run_verb(FactOracle(facts), 120)
    p = only(r)
    assert r.unit == ("local", 51, 209) and p.immediate["var:194"] == 120
    assert p.writes["bit:78"] == 1 and p.ran_by["local:207"] == "c: polls facts the path changed"


def test_a_sentence_the_input_script_queues_runs_at_the_end_of_the_clicks_frame():
    """doSentence in the input script queues the sentence; checkAndRunSentenceScript runs it after checkExecVerbs,
    in the same frame (scumm.cpp:3232, 3255)."""
    s14 = unit(("global", 14), "[0000] (F9) doSentence(9,400,0);\n[0007] (00) stopObjectCode();\n")
    s2 = unit(("global", 2), "[0000] (9A) Var[100] = Local[1];\n[0005] (00) stopObjectCode();\n")
    p = only(dialog(s14, s2).run_verb(FactOracle({"var:32": 14, "var:33": 2}), 120))
    assert p.immediate["var:100"] == 400 and p.sentences == 1 and not p.queue


def test_the_input_script_forks_on_what_the_facts_do_not_give():
    """An input script that reads the object under the mouse (findObject) or waits for a message forks, as rows
    1-3 do (counted assumptions); in the click's frame a WaitFor op forks (rows 1-2's rules), later it is over."""
    s14 = unit(("global", 14), """
[0000] (F5) Local[5] = findObject(VAR_VIRT_MOUSE_X,VAR_VIRT_MOUSE_Y);
[0007] (A8) if (Local[5]) /*0011*/ {
[000C] (1A)   Var[197] = 1;
[0011] (**) }
[0011] (D8) printEgo([Text("x")]);
[0016] (AE) WaitForMessage();
[0018] (9A) Var[194] = Local[1];
[001D] (00) stopObjectCode();
""")
    r = dialog(s14).run_verb(FactOracle({"var:32": 14}), 120)
    ps = model.feasible(r)
    kinds = {a[3].split(":")[0] for p in ps for a in p.assumptions}
    assert "?(position" in kinds and "?(message state" in kinds
    pred = model.predict(r, FactOracle({"var:32": 14}))
    assert pred["var:194"]["certain"] and pred["var:194"]["values"] == [120]
    assert not pred["var:197"]["certain"] and pred["var:197"]["values"] == [0, 1]  # the forks merged: a value set


def test_click_frame_turns_that_change_what_the_click_reads_are_counted_and_the_switch_reads_them():
    """checkExecVerbs reads _userPut after the frame's turns (scumm.cpp:3231-3232). The spec reads it at pre: a turn
    that turned input off is kept in input_drift. Switch input_after_turns (OFF by default) drops the click
    there (verbs.cpp:590)."""
    s14 = unit(("global", 14), INPUT)
    s70 = unit(("global", 70), "[0000] (80) breakHere();\n[0001] (2C) UserputOff();\n[0003] (00) stopObjectCode();\n")
    facts = {"var:32": 14, "var:53": 1, "verb:120": 1, "script:70": 1, "at:script:70:1": 1}
    p = only(dialog(s14, s70).run_verb(FactOracle(facts), 120))
    assert p.input_drift == (("var:53", 1, 0),) and p.immediate["var:194"] == 120
    p = only(dialog(s14, s70, switches={"input_after_turns"}).run_verb(FactOracle(facts), 120))
    assert "var:194" not in p.writes and any("the click is dropped" in n[2] for n in p.notes)
    p = only(dialog(s14, switches={"input_after_turns"}).run_verb(FactOracle({"var:32": 14, "var:53": 1}), 120))
    assert p.immediate["var:194"] == 120  # nothing changed it: the switch changes nothing


def test_every_installed_input_script_runs_on_a_verb_click(real):
    """Over the real IR: every input script number a script installs with a constant (a global one, or a room-local
    one in the room whose own code installs it; global code that installs a local number names no room) runs on
    a click on a verb some script shows with a constant number, with a path the predictions use and no unmodelled
    statement."""
    installs, verbs = set(), set()
    for k, u in real.units.items():
        for x in u.ins:
            if isinstance(x.st, ir.Assign) and x.st.lhs == ir.Var(model.VAR_VERB_SCRIPT) \
                    and isinstance(x.st.rhs, ir.Const) and x.st.rhs.value:
                n = x.st.rhs.value
                if n < real.first_local:
                    installs.add((n, None))
                elif k[0] != "global":
                    installs.add((n, u.room))
            if isinstance(x.st, ir.Op) and x.st.name == "VerbOps" and isinstance(x.st.args[0], ir.Const) \
                    and any(sub.name == "On" for sub in x.st.subops):
                verbs.add(x.st.args[0].value)
    assert installs and verbs
    m = Machine(ROW4, game=real)
    runs = 0
    for n, room in sorted(installs, key=str):
        for v in sorted(verbs)[:: max(1, len(verbs) // 8)]:
            facts = {"var:32": n, "var:53": 1, f"verb:{v}": 1, "var:1": 1} | ({"room": room} if room else {})
            r = m.run_verb(FactOracle(facts), v)
            assert model.feasible(r), (n, room, v, collections.Counter(p.end for p in r.paths))
            assert not [p.unmodelled for p in r.paths if p.unmodelled], (n, room, v)
            runs += 1
    assert runs


MENU = """
[0000] (1A) Local[3] = 0;
[0005] (1A) Var[194] = 0;
[000A] (80) breakHere();
[000B] (A8) unless (Var[194]) goto 000A;
[0010] (48) if (Var[194] == 120) /*0026*/ {
[0017] (5A)   Local[3] += 1;
[001C] (78)   if (Local[3] < 3) /*0026*/ {
[0023] (18)     goto 0005;
[0026] (**)   }
[0026] (**) }
[0026] (1A) Bit[78] = 1;
[002B] (00) stopObjectCode();
"""


def test_switch_const_polls_reads_a_reset_variable_as_a_polled_fact():
    """A menu loop resets the choice to a constant, waits for it in an inner poll and then tests it to leave. By the
    spec's rule (Game._test_reads) a variable the body writes stands for what its writes read, so the reset reads
    nothing and the outer loop is time-only; switch const_polls (OFF) reads it as a fact. Each setting keeps its own
    cache on the one Game."""
    u = unit(("global", 57), MENU)
    g = game(u)

    def outer(lps):  # the loop from [0005]
        return next(lp for lp in lps if lp.head == 1)

    assert outer(g.loops(("global", 57))).kind == "time"
    on = outer(g.loops(("global", 57), True))
    assert on.kind == "facts" and on.reads == frozenset({"var:194"}) and "counter" in on.non_facts
    assert outer(g.loops(("global", 57))).kind == "time"  # off again: unchanged
    assert g.const_poll_loops() == [(("global", 57), 1)]
    assert g.time_loops_at(("global", 57), 3) == (1,) and g.time_loops_at(("global", 57), 3, True) == ()


def test_switch_const_polls_forks_on_the_repeat_counter_instead_of_taking_the_exit():
    """The script waiting on the menu is live at pre, so its repeat counter (a local) is unknown. Off (the spec), the
    outer loop is time-only, its exit on the counter is taken (R_LOOP_EXIT) and the third repeat's outcome is
    certain on the first click; on, the outer loop polls the choice, so the counter test forks and the outcome is
    only possible."""
    s14, s57 = unit(("global", 14), INPUT), unit(("global", 57), MENU)
    facts = {"var:32": 14, "script:57": 1, "at:script:57:11": 1}
    off = dialog(s14, s57).run_verb(FactOracle(facts), 120)
    pred = model.predict(off, FactOracle(facts))
    assert pred["bit:78"]["certain"] and pred["bit:78"]["values"] == [1]
    assert any(a[3].startswith(f"?({model.R_LOOP_EXIT}") for p in model.feasible(off) for a in p.assumptions)
    on = dialog(s14, s57, switches={"const_polls"}).run_verb(FactOracle(facts), 120)
    pred = model.predict(on, FactOracle(facts))
    assert not pred["bit:78"]["certain"] and 1 in pred["bit:78"]["values"]
    assert not any(a[3].startswith(f"?({model.R_LOOP_EXIT}") for p in model.feasible(on) for a in p.assumptions)
    assert any(a[0].endswith("[001C]") and a[3].startswith(f"?({model.R_LOCAL}") for p in on.paths
               for a in p.assumptions)


# --- row 5: the Esc key (esc) --------------------------------------------------------------------------------------

def esc(*units: ir.Unit, **kw) -> Machine:
    g = game(*units, object_room=kw.pop("object_room", None))
    return Machine(ROW5, game=g, **kw)


# A cutscene with an override, stopped in its body; the code after the jump tests VAR_OVERRIDE as MI1's do
CUT = """
[0000] (40) cutscene([2]);
[0003] (58) beginOverride();
[0005] (18) goto 0011;
[0008] (1A) Var[100] = 1;
[000D] (80) breakHere();
[000E] (1A) Var[101] = 1;
[0011] (A8) if (VAR_OVERRIDE) /*001B*/ {
[0016] (1A)   Var[102] = 1;
[001B] (**) }
[001B] (58) endOverride();
[001D] (C0) endCutscene();
[001E] (00) stopObjectCode();
"""
CUT_PRE = {"var:24": 27, "cutscene": 1, "script:60": 1, "at:script:60:14": 1, "esc:script:60:5": 1}


def test_usable_skip_needs_the_exit_key_and_an_override_and_rests_on_the_esc_conditions():
    """Esc skips only with VAR_CUTSCENEEXIT_KEY not 0 (input.cpp:966, 1423) and an override armed at the top level
    (abortCutscene's ``if (offs)``, script.cpp:1693-1694); what it rests on and cannot see is listed (actions.ESC)."""
    held = frozenset({"var", "esc", "cutscene"})
    u = actions.usable(actions.skip_cutscene(), FactOracle(CUT_PRE, held), game())
    assert u.value is True and sorted(c for c, _ in u.unchecked()) == sorted(actions.ESC)
    assert set(actions.ESC) <= set(actions.UNCHECKED)
    no_override = {k: v for k, v in CUT_PRE.items() if not k.startswith("esc:")}
    assert actions.usable(actions.skip_cutscene(), FactOracle(no_override, held), game()).value is False
    assert actions.usable(actions.skip_cutscene(), FactOracle(CUT_PRE | {"var:24": 0}, held), game()).value is False


def test_esc_rewinds_the_override_slot_which_runs_from_its_override_point_in_the_same_frame():
    """abortCutscene (script.cpp:1689-1709): the slot that armed the override jumps from its override point, with
    VAR_OVERRIDE = 1 and the override cleared; the depth is the script's own business (its endCutscene here); the
    body after the yield never runs."""
    s60 = unit(("global", 60), CUT)
    p = only(esc(s60).run_esc(FactOracle(CUT_PRE)))
    assert p.immediate["var:102"] == 1 and "var:101" not in p.immediate  # the skip side, in the Esc frame
    assert p.immediate["esc:script:60:5"] == 0 and p.immediate["script:60"] == 0
    assert p.immediate["cutscene"] == 0  # its own endCutscene; abortCutscene leaves the depth
    assert p.immediate["var:5"] == 0  # endOverride, then endCutscene, set it back to 0
    assert p.ran_by["script:60"] == "a: rewound by Esc (abortCutscene)"
    assert any(n[0] == "assumption: cutsceneOverride" for n in p.notes)


def test_abort_alone_sets_override_1_unfreezes_and_leaves_the_depth():
    """Up to the end of the Esc frame with the rewound slot stopping at a yield right after the jump: VAR_OVERRIDE is
    1, the slot's freeze count 0 (script.cpp:1701, 1706), the depth unchanged, the slot at its next wait point."""
    s60 = unit(("global", 60), """
[0000] (58) beginOverride();
[0002] (18) goto 0006;
[0005] (80) breakHere();
[0006] (80) breakHere();
[0007] (1A) Var[103] = 1;
[000C] (00) stopObjectCode();
""")
    pre = {"var:24": 27, "cutscene": 1, "script:60": 1, "at:script:60:6": 1, "frozen:script:60": 1,
           "esc:script:60:2": 1}
    p = esc(s60).run_esc(FactOracle(pre)).paths[0]
    assert p.immediate["var:5"] == 1 and p.immediate["frozen:script:60"] == 0
    assert "cutscene" not in p.immediate and p.immediate["at:script:60:7"] == 1
    assert p.immediate["esc:script:60:2"] == 0 and "var:103" not in p.immediate
    assert p.writes["var:103"] == 1  # it runs on in the next round, by rule a


def test_a_slot_paused_by_a_delay_runs_after_the_abort():
    """abortCutscene sets the slot ssRunning (script.cpp:1700): a delay it was paused in is over at once, and it runs
    from its override point in the Esc frame."""
    s60 = unit(("global", 60), """
[0000] (58) beginOverride();
[0002] (18) goto 000A;
[0005] (2E) delay(600);
[0009] (80) breakHere();
[000A] (1A) Var[104] = 1;
[000F] (00) stopObjectCode();
""")
    pre = {"var:24": 27, "script:60": 1, "at:script:60:9": 1, "esc:script:60:2": 1}
    p = only(esc(s60).run_esc(FactOracle(pre)))
    assert p.immediate["var:104"] == 1 and p.immediate["script:60"] == 0


def test_with_no_override_armed_esc_runs_the_frame_and_skips_nothing():
    """No esc: fact: abortCutscene does nothing (script.cpp:1693-1694); the frame's slots take their turns as time
    passes; the key still goes to checkExecVerbs."""
    s60 = unit(("global", 60), CUT)
    pre = {k: v for k, v in CUT_PRE.items() if not k.startswith("esc:")}
    p = only(esc(s60).run_esc(FactOracle(pre)))
    assert p.immediate.get("var:5", 0) == 0 and p.immediate["var:101"] == 1 and "var:102" not in p.writes
    assert any("abortCutscene does nothing" in n[2] for n in p.notes)


INPUT_KEY = """
[0000] (48) if (Local[0] == 4) /*000F*/ {
[0007] (9A)   Var[105] = Local[1];
[000C] (1A)   Var[106] = 1;
[000F] (**) }
[000F] (00) stopObjectCode();
"""
SKIP_ON = """
[0000] (58) beginOverride();
[0002] (18) goto 0006;
[0005] (80) breakHere();
[0006] (2C) UserputOn();
[0008] (00) stopObjectCode();
"""


def test_the_key_reaches_the_input_script_only_when_input_is_on_after_the_frames_turns():
    """checkExecVerbs reads _userPut after runAllScripts (scumm.cpp:3231-3232, verbs.cpp:590): the override's code
    turned input on, so runInputScript(kKeyClickArea, VAR_CUTSCENEEXIT_KEY, 1) runs VAR_VERB_SCRIPT (verbs.cpp:664).
    Without the UserputOn the key is dropped; switch esc_key_at_pre reads _userPut at pre."""
    s4, s60 = unit(("global", 4), INPUT_KEY), unit(("global", 60), SKIP_ON)
    pre = {"var:24": 27, "var:32": 4, "var:53": 0, "script:60": 1, "at:script:60:5": 1, "esc:script:60:2": 1}
    p = only(esc(s4, s60).run_esc(FactOracle(pre)))
    assert p.immediate["var:53"] == 1 and p.immediate["var:105"] == 27 and p.immediate["var:106"] == 1
    p = only(esc(s4, s60, switches={"esc_key_at_pre"}).run_esc(FactOracle(pre)))
    assert "var:105" not in p.writes and any("the key is dropped" in n[2] for n in p.notes)
    s60_off = unit(("global", 60), SKIP_ON.replace("(2C) UserputOn();", "(2C) CursorShow();"))
    p = only(esc(s4, s60_off).run_esc(FactOracle(pre)))
    assert "var:105" not in p.writes and any("the key is dropped" in n[2] for n in p.notes)


def test_a_dosentence_stop_in_the_frame_wipes_the_key():
    """doSentence(STOP) runs clearClickedStatus (script_v5.cpp:1004-1008), which sets _mouseAndKeyboardStat to 0
    (input.cpp:378-386): a cutscene end hook that does it before checkExecVerbs leaves no key to take."""
    s4 = unit(("global", 4), INPUT_KEY)
    s60 = unit(("global", 60), SKIP_ON.replace("[0008] (00) stopObjectCode();",
                                               "[0008] (19) doSentence(STOP);\n[000A] (00) stopObjectCode();"))
    pre = {"var:24": 27, "var:32": 4, "var:53": 0, "script:60": 1, "at:script:60:5": 1, "esc:script:60:2": 1}
    p = only(esc(s4, s60).run_esc(FactOracle(pre)))
    assert p.immediate["var:53"] == 1 and "var:105" not in p.writes
    assert any("clearClickedStatus" in n[2] for n in p.notes)


def test_usable_skip_is_unknown_when_the_exit_key_is_unknown():
    """With VAR_CUTSCENEEXIT_KEY not known, whether Esc's branch is taken (input.cpp:966, 1423) is not known: an armed
    override makes the skip's usability unknown, and with none armed it is not usable whatever the key."""
    held = frozenset({"esc", "cutscene"})  # no var: VAR_CUTSCENEEXIT_KEY is not held
    u = actions.usable(actions.skip_cutscene(), FactOracle(CUT_PRE, held), game())
    assert u.value is None and u.unknown()[0].name == "Esc enabled"
    no_override = {k: v for k, v in CUT_PRE.items() if not k.startswith("esc:")}
    assert actions.usable(actions.skip_cutscene(), FactOracle(no_override, held), game()).value is False


# The input script for a key: the area, the key code and the third argument copied out (script.cpp:1478-1511)
INPUT_KEY_ARGS = """
[0000] (48) if (Local[0] == 4) /*0014*/ {
[0007] (9A)   Var[105] = Local[1];
[000C] (9A)   Var[107] = Local[2];
[0011] (1A)   Var[106] = 1;
[0014] (**) }
[0014] (00) stopObjectCode();
"""
KEY_PRE = {"var:24": 27, "var:32": 4, "var:53": 1, "script:60": 1, "at:script:60:5": 1, "esc:script:60:2": 1}


def test_the_key_reaches_the_input_script_as_area_4_the_code_and_1():
    """runInputScript(kKeyClickArea, code, 1) (verbs.cpp:664): the input script gets [4, VAR_CUTSCENEEXIT_KEY, 1]
    (script.cpp:1478-1511), and the way the key went is kept (Run.key_routes)."""
    s4, s60 = unit(("global", 4), INPUT_KEY_ARGS), unit(("global", 60), SKIP_ON)
    r = esc(s4, s60).run_esc(FactOracle(KEY_PRE))
    p = only(r)
    assert (p.immediate["var:105"], p.immediate["var:107"], p.immediate["var:106"]) == (27, 1, 1)
    assert [fate for fate, _ in r.key_routes] == ["input script 4"]


def test_with_the_exit_key_0_nothing_is_aborted_and_the_key_is_escapes_code():
    """VAR_CUTSCENEEXIT_KEY 0: Esc's branch is not taken (input.cpp:966, 1423), so abortCutscene does not run (no
    VAR_OVERRIDE, the override stays armed) and processKeyboard's default branch gives checkExecVerbs the key's own
    code, KEYCODE_ESCAPE (input.cpp:1553-1558)."""
    s4, s60 = unit(("global", 4), INPUT_KEY_ARGS), unit(("global", 60), SKIP_ON)
    p = only(esc(s4, s60).run_esc(FactOracle(KEY_PRE | {"var:24": 0})))
    assert p.immediate["var:105"] == model.KEYCODE_ESCAPE and p.immediate["var:107"] == 1
    assert "var:5" not in p.writes and "esc:script:60:2" not in p.writes and "script:60" not in p.writes
    assert any("Esc is not the cutscene exit key" in n[2] for n in p.notes)


def test_an_exit_key_of_enter_or_tab_is_a_click_while_the_cursor_shows():
    """processKeyboard turns a code of Enter or Tab into a left or right click while the cursor shows (input.cpp:
    1566-1570): not modelled, noted, and no input script runs; with the cursor hidden the code stays a key."""
    s4, s60 = unit(("global", 4), INPUT_KEY_ARGS), unit(("global", 60), SKIP_ON)
    for code in (model.KEYCODE_RETURN, model.KEYCODE_TAB):
        r = esc(s4, s60).run_esc(FactOracle(KEY_PRE | {"var:24": code, "var:52": 1}))
        p = only(r)
        assert "var:105" not in p.writes and any("a click (input.cpp:1566-1570)" in n[2] for n in p.notes)
        assert [fate for fate, _ in r.key_routes] == ["a click (Enter or Tab), not modelled"]
        p = only(esc(s4, s60).run_esc(FactOracle(KEY_PRE | {"var:24": code, "var:52": 0})))
        assert p.immediate["var:105"] == code


def test_paths_merged_with_and_without_a_stop_fork_on_whether_the_key_was_wiped():
    """Two sides of a fork in another slot's turn, one with a doSentence(STOP), merge before checkExecVerbs
    (``Path.cleared`` None): the key forks into wiped and taken, each kept with the forks it rests on."""
    s4, s60 = unit(("global", 4), INPUT_KEY_ARGS), unit(("global", 60), SKIP_ON)
    s61 = unit(("global", 61), """
[0000] (80) breakHere();
[0001] (16) Local[3] = getRandomNr(5);
[0005] (48) if (Local[3] == 1) /*000E*/ {
[000C] (19)   doSentence(STOP);
[000E] (**) }
[000E] (80) breakHere();
[000F] (00) stopObjectCode();
""")
    r = esc(s4, s60, s61).run_esc(FactOracle(KEY_PRE | {"script:61": 1, "at:script:61:1": 1}))
    assert sorted(fate for fate, _ in r.key_routes) == ["input script 4", "wiped by clearClickedStatus"]
    for _, assumptions in r.key_routes:  # each rests on the merge's fork and on the random draw behind it
        assert any(a[1] == "clearClickedStatus ran in the frame" for a in assumptions)
        assert any(a[3].startswith(f"?({model.R_RANDOM}") for a in assumptions)
    taken = [p for p in r.paths if p.writes.get("var:105") == 27]
    assert len(taken) == 1 and len(r.paths) == 2


def test_the_plans_closure_of_a_variables_writes():
    """check.closure_reaches: the start value and the constants closed under the steps (plan step 1, Facts)."""
    import check
    assert check.closure_reaches({0, 49}, {1}, 27) and not check.closure_reaches({49}, {1}, 27)
    assert check.closure_reaches({49}, {-1}, 27) and not check.closure_reaches({49}, {2}, 52)
    assert check.closure_reaches({49}, {2, -4}, 27) and not check.closure_reaches({49}, {4, -6}, 28)
    assert check.closure_reaches({5}, set(), 5) and not check.closure_reaches({5}, set(), 6)
    assert check.closure_reaches({0}, {3, 5}, 8) and not check.closure_reaches({0}, {3, 5}, 7)


def test_other_slots_in_the_esc_frame_follow_the_click_frames_rules():
    """The Esc frame is one frame after pre: a slot live at pre at a time-only wait still holds there (round 0's
    rule), one that waits for nothing runs; the rewound slot runs by rule a whatever its pre wait point was."""
    s60 = unit(("global", 60), CUT)
    s61 = unit(("global", 61), "[0000] (80) breakHere();\n[0001] (1A) Var[110] = 1;\n[0006] (00) stopObjectCode();\n")
    s62 = unit(("global", 62), "[0000] (2E) delay(60);\n[0004] (1A) Var[111] = 1;\n[0009] (00) stopObjectCode();\n")
    pre = CUT_PRE | {"script:61": 1, "at:script:61:1": 1, "script:62": 1, "at:script:62:4": 1}
    p = only(esc(s60, s61, s62).run_esc(FactOracle(pre)))
    assert p.immediate["var:110"] == 1 and "var:111" not in p.immediate and p.writes["var:111"] == 1
    assert ("script:62", "pre, first turn", "a time-only wait, held in the click's frame") in p.skips


def test_every_real_override_point_is_a_jump_the_esc_runs(real):
    """Over the real IR: every beginOverride with a jump after it, armed as the only override at pre, is run from that
    jump by rule a with no unmodelled statement (the code after a cutscene body). The state is a bare one (no room for
    a global script, depth 1), so a path may end in an engine error a real state would not meet; that is allowed."""
    m = Machine(ROW5, game=real)
    bad, n = [], 0
    for key, u in real.units.items():
        if key[0] not in ("global", "local"):
            continue
        for x in u.ins:
            if not (isinstance(x.st, ir.Op) and x.st.name == "beginOverride") or x.esc is None:
                continue
            name = model.unit_name(key)
            facts = {"var:24": 27, "cutscene": 1, f"esc:{name}:{u.ins[x.esc].offset}": 1,
                     f"at:{name}:{u.ins[x.esc + 1].offset if x.esc + 1 < len(u.ins) else 0}": 1}
            if key[0] == "global":
                facts[f"script:{key[1]}"] = 1
            else:
                facts |= {f"local:{key[2]}": 1, "room": key[1]}
            r = m.run_esc(FactOracle(facts))
            rewound = any(p.ran_by.get(name, "").startswith("a: rewound by Esc") for p in r.paths)
            if not rewound or any(p.unmodelled for p in r.paths):
                bad.append((x.cite(), sorted({p.end for p in r.paths})))
            n += 1
    assert n > 100 and not bad, bad[:10]
