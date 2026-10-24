"""Tests for E003 step 4's machinery: domains.py, task.py, relevance.py, plan_fd.py, on hand-made tasks.

usage: uv run python -m pytest experiments/E003-path-through-cloud/test_task.py -q

The planner tests run Fast Downward and are skipped when it is not built (third_party/downward/builds/release).
A few domain tests read the real IR; they use game numbers as fixtures, which tests may.
"""

import collections

import pytest

import domains as D
import ir
import plan_fd
import relevance as R
from domains import ABOVE, BELOW, BINARY, Domain
from task import Action, Effect, OutOfDomain, Task, write_sas

FD_BUILT = (plan_fd.DOWNWARD / "builds" / plan_fd.BUILD / "bin" / "downward").exists()
needs_fd = pytest.mark.skipif(not FD_BUILT, reason="Fast Downward is not built")


# --- domains ---------------------------------------------------------------------------------------------

def test_domain_collapses_past_the_compared_constants():
    d = Domain.of([0, 1, 2, 3, 9], low=0, high=3)
    assert d.values == (0, 1, 2, 3, ABOVE)
    assert d.label(9) == ABOVE and d.label(100) == ABOVE and d.label(2) == 2 and d.label(-1) is None
    assert d.test(">", 2) == {3, ABOVE}
    assert d.test("==", 3) == {3}
    assert d.test("<", 3) == {0, 1, 2}
    assert d.test("==", 7) is None  # ABOVE cannot decide a comparison past the bound
    assert d.add(3, 1) == {ABOVE}
    assert d.add(ABOVE, 1) == {ABOVE}
    assert d.add(ABOVE, -2) == {2, 3, ABOVE}  # anything in (3 - 2, 3], or still above


def test_domain_below():
    d = Domain.of([-3, 0, 5], low=0, high=5)
    assert d.values == (BELOW, 0, 5)
    assert d.test("<", 0) == {BELOW}
    assert d.add(BELOW, 1) == {BELOW, 0}


def test_closure_counter_constants_and_never_compared():
    assert D._closure(0, set(), {1}, {3}).values == (0, 1, 2, 3, ABOVE)
    assert D._closure(0, set(), {1}, {3}).values == (0, 1, 2, 3, ABOVE)  # not BELOW: 0 is the start
    assert D._closure(0, {5}, {-1}, {2, 4}).values == (BELOW, 0, 1, 2, 3, 4, ABOVE)  # 5 is ABOVE; 0 - 1 is BELOW
    assert D._closure(0, {1, 2}, set(), set()).values == (0, 1, 2)  # constants only, never compared: exact
    assert D._closure(0, set(), {1}, set()) is None  # a counter never compared: no bound


def _unit(key, text, room=None, entries=None):
    u = ir.Unit(key, room, "test", "")
    ir._read(u, text)
    if entries:
        u.entries = entries
    return u


HAND = {
    ("global", 1): _unit(("global", 1), """[0000] (1A) Var[300] = 0;
[0005] (48) if (VAR_DEBUGMODE == 1) /*0011*/ {
[000C] (1A)   Var[301] = 7;
[0011] (**) }
[0011] (5A) Var[300] += 1;
[0016] (44) if (Var[300] > 3) /*0020*/ {
[001D] (80)   breakHere();
[0020] (**) }
[0020] (1A) Var[302] = Var[300];
[0025] (0A) startScript(2,[4]);
[0029] (0A) startScript(2,[6]);
[002D] (A0) stopObjectCode();"""),
    ("global", 2): _unit(("global", 2), """[0000] (1A) Var[303] = Local[0];
[0005] (48) if (Var[303] == 5) /*000F*/ {
[000C] (80)   breakHere();
[000F] (**) }
[000F] (A0) stopObjectCode();"""),
}


def test_domains_on_hand_made_scripts():
    doms = D.build({}, units=HAND)
    assert doms.get("var:300").values == (0, 1, 2, 3, ABOVE)  # a counter compared with 3
    assert doms.get("var:301") is None or 7 not in doms.get("var:301").values  # debug-only write left out
    assert doms.why["var:302"] == "a copy of another variable"
    assert doms.get("var:303").values == (0, 4, ABOVE)  # start 0; Local[0] bound to 4 and 6; compared with 5
    assert doms.lossy.get("var:300")  # collapsed, and copied into var 302
    assert doms.get("bit:9") == BINARY and doms.get("at:script:1:32") == BINARY
    assert doms.get("cutscene").values == (0, 1, 2, 3, 4, 5)


def test_domains_real_ir_ego_and_debug_branch():
    doms = D.build({})
    assert doms.get("var:1").values == (0, 1)  # VAR_EGO: boot's 1; the debug-mode actor switch is left out
    assert doms.get("owner:901") is not None  # an object picked up is owned by VAR_EGO's values
    assert 1 in doms.get("owner:901").values
    assert doms.counts["instructions left out by the settings (only a decided jump leads there)"] > 0


# --- task.py ---------------------------------------------------------------------------------------------

def door_task():
    facts = {"door": BINARY, "room": Domain.of([1, 2])}
    acts = [Action("open door", pre={"room": {1}}, effects=(Effect("door", 1),)),
            Action("walk through", pre={"door": {1}, "room": {1}}, effects=(Effect("room", 2),))]
    return Task(facts, acts, {"room": {2}})


def test_write_sas_door_and_state_reemit():
    sas = write_sas(door_task())
    start = {"room": 1}
    text = sas.text(start)
    assert text.startswith("begin_version\n3\nend_version\nbegin_metric\n0\nend_metric\n2\n")
    assert "Atom door(door=1)" in text and "Atom room(room=2)" in text
    assert sas.counts["operators"] == 2 and sas.counts["variables"] == 2
    other = sas.text({"room": 2, "door": 1})
    # a replan changes only the begin_state ... end_state section, byte for byte
    head, tail = text.split("begin_state")[0], text.split("end_state\n")[1]
    assert other.startswith(head) and other.endswith(tail) and other != text
    assert sas.state({"room": 2, "door": 1}) == "begin_state\n1\n1\nend_state\n"
    with pytest.raises(OutOfDomain):
        sas.state({"room": 7})


def test_set_precondition_becomes_variants():
    facts = {"x": Domain.of([0, 1, 2, 3]), "y": BINARY}
    t = Task(facts, [Action("a", pre={"x": {1, 2}}, effects=(Effect("y", 1),))], {"y": {1}})
    sas = write_sas(t)
    assert sas.counts["variants"] == 2 and sas.counts["operators"] == 2
    assert {op.chosen for op in sas.ops} == {(("x", 1),), (("x", 2),)}


def test_unknown_write_is_one_variant_per_value_and_marked():
    facts = {"x": Domain.of([0, 1, 2]), "y": BINARY}
    t = Task(facts, [Action("draw", effects=(Effect("x", values=frozenset({1, 2})), Effect("y", 1)))], {"x": {2}})
    sas = write_sas(t)
    assert sas.counts["operators"] == 2
    assert sorted(op.guessed[0][1] for op in sas.ops) == [1, 2]
    assert all(op.guessed[0][0] == "x" for op in sas.ops)


def test_step_effect_per_value_and_above_minus_is_a_guess():
    d = D._closure(3, set(), {-1}, {0, 2})  # a counter from 3, stepped down, compared with 0 and 2
    assert d.values == (BELOW, 0, 1, 2, ABOVE)
    t = Task({"n": d}, [Action("dec", effects=(Effect("n", add=-1),))], {"n": {0}})
    sas = write_sas(t)
    assert sas.counts["steps whose result is several values (guesses)"] == 1  # ABOVE - 1: 2 or still above
    assert sas.counts["operators"] == 2  # one variant per guessed value
    assert sorted(str(op.guessed[0][1]) for op in sas.ops) == ["2", "above"]
    # BELOW - 1 in each of the two variants, and ABOVE - 1 guessed as ABOVE
    assert sas.counts["effects that write the value the key already has (left out)"] == 3


def test_conflicting_effects_raise_and_exclusive_ones_pass():
    facts = {"f": BINARY, "g": BINARY}
    bad = Task(facts, [Action("a", effects=(Effect("g", 1), Effect("g", 0, when=(("f", frozenset({1})),))))], {"g": {1}})
    with pytest.raises(ValueError):
        write_sas(bad)
    ok = Task(facts, [Action("a", effects=(Effect("g", 1, when=(("f", frozenset({0})),)),
                                           Effect("g", 0, when=(("f", frozenset({1})),))))], {"g": {1}})
    assert write_sas(ok).counts["conditional effects"] == 2


def test_dropped_actions_are_counted():
    facts = {"f": BINARY}
    t = Task(facts, [Action("noop", pre={"f": {1}}, effects=(Effect("f", 1),)),
                     Action("never", pre={"f": set()}, effects=(Effect("f", 0),)),
                     Action("set", effects=(Effect("f", 1),))], {"f": {1}})
    sas = write_sas(t)
    assert sas.counts["variants with no effect left (dropped)"] == 1
    assert sas.counts["actions whose precondition no value meets (dropped)"] == 1
    assert sas.counts["operators"] == 1


# --- relevance -----------------------------------------------------------------------------------------

def avoid_task():
    """q makes the goal g = 1, but when f = 1 it also clears h, another goal; o moves f out of the way. The plan
    is o then q. Plain relevance over values keeps q (it writes g = 1) and needs f = 1 (q's condition), but
    o writes f = 2, which nothing needs, so it drops o and the task left has no plan."""
    facts = {"f": Domain.of([0, 1, 2]), "g": BINARY, "h": BINARY, "z": BINARY}
    acts = [Action("q", effects=(Effect("g", 1), Effect("h", 0, when=(("f", frozenset({1})),)))),
            Action("o", effects=(Effect("f", 2),)),
            Action("unrelated", effects=(Effect("z", 1),))]
    return Task(facts, acts, {"g": {1}, "h": {1}})


def plain_relevance(task):
    """Backward relevance over values only, with effect conditions as needed values: what the module avoids."""
    needed = collections.defaultdict(set)
    for k, s in task.goal.items():
        needed[k] |= set(s)
    kept, changed = set(), True
    while changed:
        changed = False
        for i, a in enumerate(task.actions):
            if i in kept or not any(R._writes(task, e) & needed[e.key] for e in a.effects):
                continue
            kept.add(i)
            changed = True
            for k, s in a.pre.items():
                needed[k] |= set(s)
            for e in a.effects:
                for k, s in e.when:
                    needed[k] |= set(s)
    return sorted(kept)


def test_relevance_keeps_the_action_that_avoids_a_condition():
    t = avoid_task()
    rel = R.relevant(t)
    assert rel.actions == [0, 1]  # q and o; "unrelated" goes
    assert "f" in rel.whole
    assert plain_relevance(t) == [0]


def test_relevance_chain_through_preconditions():
    t = door_task()
    t.actions.append(Action("paint", effects=(Effect("door", 0),)))
    rel = R.relevant(t)
    assert rel.actions == [0, 1]  # paint writes door = 0, which nothing needs
    assert rel.values["door"] == {1}


@needs_fd
def test_relevance_avoid_case_with_the_planner():
    t = avoid_task()
    start = {"f": 1, "h": 1}
    full = plan_fd.plan(write_sas(R.restrict(t, R.relevant(t))), start, time_limit=60)
    assert full.status == "plan" and [s.name for s in full.steps] == ["o", "q"]
    plain = Task(t.facts, [t.actions[i] for i in plain_relevance(t)], t.goal)
    res = plan_fd.plan(write_sas(plain), start, time_limit=60)
    assert res.status in ("unsolvable", "incomplete")


# --- the planner ---------------------------------------------------------------------------------------------

@needs_fd
def test_lama_first_door_plan_and_landmarks():
    sas = write_sas(door_task())
    res = plan_fd.plan(sas, {"room": 1}, time_limit=60)
    assert res.status == "plan" and res.exit_code == 0
    assert [s.name for s in res.steps] == ["open door", "walk through"]
    lms = {tuple(lm.facts): lm for lm in res.landmarks}
    assert lms[(("door", 1),)].initial is False and lms[(("room", 2),)].goal
    assert any(kind == "gn" for _, _, kind in res.orderings)


@needs_fd
def test_debug_search_string_gives_the_alias_plan():
    for t, start in ((door_task(), {"room": 1}), (avoid_task(), {"f": 1, "h": 1})):
        text = write_sas(t).text(start)
        _, _, with_debug, _, _ = plan_fd.run(text, 60)
        _, _, with_alias, _, _ = plan_fd.run(text, 60, alias="lama-first")
        assert with_debug == with_alias and with_debug


@needs_fd
def test_plan_through_a_guess_reports_the_guess():
    facts = {"x": Domain.of([0, 1, 2]), "y": BINARY}
    t = Task(facts, [Action("draw", effects=(Effect("x", values=frozenset({1, 2})),)),
                     Action("use", pre={"x": {2}}, effects=(Effect("y", 1),))], {"y": {1}})
    res = plan_fd.plan(write_sas(t), {}, time_limit=60)
    assert res.status == "plan"
    assert res.steps[0].name == "draw" and res.steps[0].guessed[0][:2] == ("x", 2)
    assert res.steps[1].name == "use" and res.steps[1].guessed == ()


def test_large_guess_product_goes_through_pick_variables():
    facts = {f"x{i}": Domain.of([0, 1, 2]) for i in range(6)} | {"y": BINARY}
    effects = tuple(Effect(f"x{i}", values=frozenset({1, 2})) for i in range(6))  # 2**6 = 64: still variants
    sas = write_sas(Task(facts, [Action("draw", effects=effects)], {"x0": {2}}))
    assert sas.counts["operators"] == 64 and sas.counts["pick variables"] == 0
    effects += (Effect("y", values=frozenset({0, 1})),)  # 128 > PRODUCT_LIMIT
    sas = write_sas(Task(facts, [Action("draw", effects=effects)], {"x0": {2}}))
    assert sas.counts["actions whose guesses go through pick variables"] == 1
    assert sas.counts["pick variables"] == 7 and sas.counts["pick operators"] == 6 * 3 + 2
    assert sas.counts["operators"] == 1 + 20


def test_op_table_is_json_and_the_file_is_the_same_twice():
    import json
    t = Task({f"x{i}": Domain.of([0, 1, 2]) for i in range(7)},
             [Action("draw", effects=tuple(Effect(f"x{i}", values=frozenset({1, 2})) for i in range(7)))],
             {"x0": {2}})
    a, b = write_sas(t), write_sas(t)
    assert a.text({}) == b.text({})
    rows = json.loads(json.dumps(a.op_table()))
    assert rows[0]["name"] == "draw" and len(rows[0]["picked"]) == 7
    assert any(r["pick"] == ["x0", "2"] for r in rows)


@needs_fd
def test_plan_through_pick_variables_reports_the_guess():
    facts = {f"x{i}": Domain.of([0, 1, 2]) for i in range(7)} | {"y": BINARY}
    acts = [Action("draw", effects=tuple(Effect(f"x{i}", values=frozenset({1, 2})) for i in range(7))),
            Action("use", pre={"x3": {2}, "x5": {1}}, effects=(Effect("y", 1),))]
    sas = write_sas(Task(facts, acts, {"y": {1}}))
    assert sas.counts["actions whose guesses go through pick variables"] == 1
    res = plan_fd.plan(sas, {}, time_limit=60)
    assert res.status == "plan" and res.steps[-1].name == "use"  # lama-first may draw more than once
    guessed = {k: v for k, v, _ in res.steps[-2].guessed}
    assert guessed["x3"] == 2 and guessed["x5"] == 1 and set(guessed) == {f"x{i}" for i in range(7)}
    for lm in res.landmarks:
        assert lm.aux == any(k.startswith("pick:") for k, _ in lm.facts)


@needs_fd
def test_set_goal_and_unsolvable_status():
    facts = {"x": Domain.of([0, 1, 2])}
    t = Task(facts, [Action("up", pre={"x": {0}}, effects=(Effect("x", 1),))], {"x": {1, 2}})
    res = plan_fd.plan(write_sas(t), {}, time_limit=60)
    assert res.status == "plan" and [s.name for s in res.steps] == ["up"]  # the goal operator is left out
    t2 = Task(facts, [Action("up", pre={"x": {0}}, effects=(Effect("x", 1),))], {"x": {2}})
    res2 = plan_fd.plan(write_sas(t2), {}, time_limit=60)
    assert res2.status in ("unsolvable", "incomplete") and res2.steps == []
