"""Tests of the Part I runner's pieces: a reading as files (packed_reading.py), the control (export_records
.control_records), the dialog line clicked by its verb number, the python planner's write error, a step of several
paths chosen in the engine, the landmarks, the scoring watch and the runner's report.

usage: uv run pytest experiments/E003-path-through-cloud/test_replay_part1.py -m 'not slow'   (hand-made cases)
       READER_PICKLE=<reading.pkl> uv run pytest ... -m slow     (the kitchen: the files and the control, minutes)
       READER_PICKLE=<reading.pkl> python test_replay_part1.py control-kitchen <work dir>
                                                                 (results/control-kitchen.log)

The fixtures name MI1's numbers (tests may, C2); the modules tested do not.
"""

import collections
import json
import os
import pickle
import subprocess
import sys
import types
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import export_records as E  # noqa: E402
import packed_reading as PR  # noqa: E402
import plan_fd  # noqa: E402
import reader as R  # noqa: E402
import replay as RP  # noqa: E402
import replay_part1 as P1  # noqa: E402
import task as T  # noqa: E402
import test_replay_review as TR  # noqa: E402
from domains import ABOVE, BINARY, Domain  # noqa: E402

# --- a hand-made reading ------------------------------------------------------------------------------------------

DOMS = {"script:2": BINARY, "frozen:script:2": BINARY, "room": Domain.of({28, 41}), "bit:6": BINARY,
        "state:316": BINARY, "var:53": Domain.of(range(0, 6), 0, 3), "cutscene": Domain.of(range(0, 5)),
        "pos:script:2": Domain.of({-1, 736, 937}), "args:script:2": Domain.of({0, 9}), "frozen:local:203": BINARY,
        "local:203": BINARY, "var:9": Domain.of({0, 1, 2}), "saved:2:1": BINARY, "verb:2": BINARY}


def builder():
    scope = types.SimpleNamespace(start={"room": 28, "var:53": 1}, rooms=(28, 41), walk_verb=11, verbs=[2, 9],
                                  dialog_verbs=[109, 121])
    return types.SimpleNamespace(table=types.SimpleNamespace(domain=DOMS.get), scope=scope,
                                 counts=collections.Counter({"a counter": 3}))


def records():
    ev = R.Event("script:2", ("global", 2), 736, 9, {}, True, None, None, False)
    c = ("global/script-002.txt:9 [0007]", "Var[104] == Local[1]", False, "?(not a fact: var:104)")
    walk = TR.rec("walk_to", "walk_to(316)", {"script:2": frozenset({0}), "room": frozenset({28}),
                                              "bit:6": frozenset({0}), "var:53": frozenset({1, 2, ABOVE})},
                  {"script:2": ("set", 1), "pos:script:2": ("set", 736), "args:script:2": ("set", 9),
                   "var:53": ("add", -1), "cutscene": ("add", 1)},
                  [c], ("sentence", 11, 316, None), {"script:2", "room", "var:53"})
    arrive = TR.rec("event", "event(script:2@736#9)", {"script:2": frozenset({1}), "pos:script:2": frozenset({736}),
                                                       "args:script:2": frozenset({9}), "var:9": frozenset({0, 1})},
                    {"state:316": ("set", 1), "pos:script:2": ("set", -1), "script:2": ("set", 0),
                     "var:9": ("guess", frozenset({1, 2})), "frozen:local:203": ("when", "local:203", 1),
                     "saved:2:1": ("copy", "verb:2"), "var:53": ("set", 5)},
                    (), ("event", ev), {"script:2", "pos:script:2", "args:script:2"})
    twin = TR.rec("walk_to", "walk_to(316)", dict(walk.pre), dict(walk.effects), [], walk.source, walk.engine_pre)
    line = TR.rec("dialog_line", "dialog_line(121)[33@28]", {"verb:2": frozenset({1}), "room": frozenset({28}),
                                                              "bit:6": frozenset({1})},
                  {"bit:6": ("set", 0)}, (), ("verb", 121), {"verb:2", "room"})
    return [walk, arrive, twin, line]


def reading(tmp_path, goal=None):
    b, recs = builder(), records()
    info = PR.export_reading(tmp_path, b, recs, goal or {"state:316": {1}}, "a hand-made start",
                             {"name": "hand", "sources": {"x": "y"}})
    return b, recs, info


def labelled(effects, facts):
    """build_task's effects with each written value as its domain's label (the packed file writes labels)."""
    out = []
    for e in effects:
        d = facts[e.key]
        if e.value is not None and isinstance(e.value, int) and d.label(e.value) is not None:
            e = T.Effect(e.key, value=d.label(e.value), add=e.add, values=e.values, when=e.when)
        out.append(e)
    return tuple(out)


def test_the_reading_files_give_build_tasks_task(tmp_path):
    """The packed file is export_records.py's, byte for byte; the task read back is build_task's: names, sets,
    effects (a written value as its label: var:53 := 5 is "above"), and each record's kind, label, end, own
    precondition facts, engine conditions, checks, source and effects (set, guess, add, when, copy)."""
    b, recs, info = reading(tmp_path)
    goal = {"state:316": {1}}
    E.export_records(tmp_path / "ref.tc", b, recs, goal, {})
    assert (tmp_path / "ref.tc").read_bytes() == (tmp_path / "reading.tc").read_bytes()
    assert info["actions"] == 3 and info["folded"] == 1  # the twin folds into the walk
    full = R.build_task(b, recs, goal)
    rd = PR.load_reading(tmp_path / "reading.json")
    assert rd.doc["bounds"] == {"var:53": [None, 3]} and rd.doc["scope"]["dialog_verbs"] == [109, 121]
    assert rd.dialog_verbs == [109, 121] and rd.doc["n_records"] == 4 and rd.doc["control"] is False
    assert rd.task.goal == goal and len(rd.task.actions) == len(full.actions) == 3
    assert {k: rd.task.facts[k] for k in full.facts} == full.facts
    for a, p in zip(full.actions, rd.task.actions):
        assert a.name == p.name and {k: set(v) for k, v in a.pre.items()} == p.pre
        assert labelled(a.effects, full.facts) == p.effects
        r, q = a.what, p.what
        assert (r.kind, r.label, r.end, set(r.pre), r.engine_pre) == (q.kind, q.label, q.end, set(q.pre),
                                                                       q.engine_pre)
        assert [tuple(c) for c in r.checks] == q.checks
        assert {k: (("set", full.facts[k].label(e[1])) if e[0] == "set" else e) for k, e in r.effects.items()} == \
            q.effects
    assert rd.task.actions[1].what.effects["var:53"] == ("set", ABOVE) and \
        rd.task.actions[1].what.effects["saved:2:1"] == ("copy", "verb:2")
    assert rd.task.actions[1].what.source == ("event", "script:2", 736, 9, False)
    assert rd.task.actions[0].what.source == ("sentence", 11, 316, None)
    assert rd.task.actions[-1].name == "dialog_line(121)[33@28]#3"
    # the walk's add on var:53 needs room: build_task's set, not one of the record's own facts
    assert rd.task.actions[0].pre["var:53"] == {1, 2, ABOVE} and "cutscene" in rd.task.actions[0].pre
    assert "cutscene" not in rd.task.actions[0].what.pre
    # what task_start and replan_start read: the wait points from pos:'s domain
    assert rd.b.table.points == {"at": {"script:2": {736, 937}}, "esc": {}}
    assert R.task_start(rd.b, {"at:script:2:736": 1}, rd.task)["pos:script:2"] == 736
    rd.close()


def test_a_ban_cuts_the_packed_tasks_actions_as_replays(tmp_path):
    b, recs, _ = reading(tmp_path)
    full = R.build_task(b, recs, {"state:316": {1}})
    rd = PR.load_reading(tmp_path / "reading.json")
    bans = [RP.Ban("walk_to(316)", (("bit:6", 0), ("room", 28)))]
    want = RP.ban_pieces(full, bans)
    got = rd.task.ban_pieces(bans)
    assert len(got) == len(want) and [[p.name for p in got[i]] for i in range(3)] == [[p.name for p in w] for w in want]
    assert [p.pre for p in got[0]] == [p.pre for p in want[0]]
    rd.close()


def test_a_wrong_action_line_is_refused(tmp_path):
    reading(tmp_path)
    lines = (tmp_path / "reading.actions.jsonl").read_text().splitlines()
    (tmp_path / "reading.actions.jsonl").write_text("\n".join([lines[1], lines[0]] + lines[2:]) + "\n")
    rd = PR.load_reading(tmp_path / "reading.json")
    with pytest.raises(ValueError, match="is not the packed file's"):
        rd.task.actions[0]
    with pytest.raises(ValueError, match="is not action 0"):
        E.control_records(tmp_path / "reading.tc", tmp_path / "reading.actions.jsonl", tmp_path / "c.tc")


# --- the control -----------------------------------------------------------------------------------------------------

def test_the_control_keeps_the_engine_conditions_and_the_steps_room(tmp_path):
    """The walk keeps script:2, room (its engine conditions), var:53 (an engine condition it also steps) and cutscene
    (a step's room) and drops bit:6; the event keeps its slot's facts and drops var:9; the dialog line drops bit:6.
    Effects, tables and goal are the source's; taskcore reads the file."""
    reading(tmp_path)
    c = E.control_records(tmp_path / "reading.tc", tmp_path / "reading.actions.jsonl", tmp_path / "control.tc")
    assert c["actions"] == 3 and c["sets dropped"] == 3 and c["dropped_by_fact"] == {"bit": 2, "var": 1}
    assert c["dropped_by_kind"] == {"dialog_line": 1, "event": 1, "walk_to": 1}
    rd = PR.load_reading(tmp_path / "reading.json")
    ct = PR.PackedTask(PR.Packed(tmp_path / "control.tc"), PR.Lines(tmp_path / "reading.actions.jsonl"),
                       rd.doc["bounds"])
    assert ct.facts == rd.task.facts and ct.goal == rd.task.goal
    kept = [sorted(a.pre) for a in ct.actions]
    assert kept == [["cutscene", "room", "script:2", "var:53"], ["args:script:2", "pos:script:2", "script:2"],
                    ["room", "verb:2"]]
    for a, q in zip(rd.task.actions, ct.actions):
        assert a.effects == q.effects and all(q.pre[k] == a.pre[k] for k in q.pre)
        assert set(q.what.pre) == set(a.what.pre) & set(q.pre)  # the control's records: only what it keeps
    # its ban names the room alone: its record tested nothing else
    assert RP.ban_of(ct.actions[0].what, {"room": 28, "bit:6": 0, "script:2": 0}) == RP.Ban("walk_to(316)",
                                                                                             (("room", 28),))
    p = subprocess.run([str(plan_fd.TASKCORE), "info", "--task", str(tmp_path / "control.tc")], capture_output=True,
                       text=True)
    assert p.returncode == 0 and "3 actions" in p.stdout
    ct.close()
    rd.close()


def test_the_runner_makes_the_control_once(tmp_path):
    reading(tmp_path)
    out = P1.make_control(tmp_path / "reading.json")
    doc = json.loads(out.read_text())
    assert out.name == "control.json" and doc["control"] and doc["packed"] == "control.tc"
    assert doc["control_counts"]["sets dropped"] == 3
    mtime = (tmp_path / "control.tc").stat().st_mtime_ns
    assert P1.make_control(tmp_path / "reading.json") == out and (tmp_path / "control.tc").stat().st_mtime_ns == mtime
    rd = PR.load_reading(out)
    assert sorted(rd.task.actions[0].pre) == ["cutscene", "room", "script:2", "var:53"]
    rd.close()


# --- the harness ----------------------------------------------------------------------------------------------------

class VerbBranch(TR.FakeBranch):
    """The review's fake engine with a dialog menu: verb 121 in slot 5 at (10, 150), 8 by 6 pixels."""

    def __init__(self, react, up_from=0):
        super().__init__(react)
        self.up_from = up_from

    @property
    def status(self):
        st = super().status
        st["up"] = int(self.f >= self.up_from)
        return st

    def state(self):
        s = super().state()
        s["verbs"] = [{"id": 121, "slot": 5, "x": 10, "y": 150, "name": "Same.", "key": 0},
                      {"id": 122, "slot": 6, "x": 10, "y": 158, "name": "Same.", "key": 0}]
        return s

    def at(self, x, y):
        return {"verb_slot": 5 if 10 <= x < 18 and 150 <= y < 156 else 6 if 10 <= x < 18 and 158 <= y < 164 else 0}


def test_the_harness_clicks_a_dialog_verb_by_its_number():
    def answer(fb, f, clicks):
        if any(c["click"] == [12, 152] for c in clicks):
            fb.obj = dict(fb.obj)
            fb.answered = f
    r = TR.rec("dialog_line", "dialog_line(121)[33@28]", {"room": frozenset({28})}, {"state:316": ("set", 1)},
               (), ("verb", 121), {"room"})
    h, fb = TR.harness(answer, r)
    fb.__class__ = VerbBranch
    fb.up_from = fb.f + 5  # user input comes on 5 frames later: the click waits for it
    log = RP.StepLog(0, 0, "say", "dialog_line", "click dialog line 121", fb.f)
    t = {}
    assert h._play_input(RP.Input("line", 121), "say", log, t)
    clicks = [e for e in h.inputs() if "click" in e]
    assert clicks == [{"frame": fb.up_from + 1, "click": [12, 152]}] and fb.answered == fb.up_from + 1
    assert h.branch.frame == fb.up_from + 1 and t["issue"] == fb.up_from + 1
    assert h.watched.before_input[0] == fb.up_from  # the state just before the click, for the replay checks


def test_a_dialog_verb_that_never_shows_is_an_executor_error():
    h, fb = TR.harness(lambda fb, f, c: None, TR.walk_rec())
    log = RP.StepLog(0, 0, "say", "dialog_line", "click dialog line 125", fb.f)
    t = {}
    assert not h._play_input(RP.Input("line", 125), "say", log, t)
    assert "verb 125 cannot be clicked" in t["error"] and h.inputs() == []


def test_a_write_sas_error_stops_the_python_planner_with_its_reason():
    r = TR.walk_rec()
    task = T.Task({"script:2": BINARY, "room": Domain.of({28}), "bit:6": BINARY},
                  [T.Action("walk_to(316)#0", pre={"script:2": {0}}, effects=(T.Effect("script:2", value=7),),
                            what=r)], {"script:2": {1}})
    h, fb = TR.harness(lambda fb, f, c: None, r)
    h.base, h.planner = task, "python"
    assert h.plan(first=True) == []
    assert h.stop_reason.startswith("planner: write_sas: walk_to(316)#0: script:2 := 7")
    assert h.plans[-1]["status"] == "write error"
    assert h.run()["stop"].startswith("planner: write_sas")


def walk(name, g, room=28):
    rec = TR.rec("walk_to", "walk_to(316)", {"var:9": frozenset(g), "room": frozenset({room})},
                 {"state:316": ("set", 1)}, (), ("sentence", 11, 316, None), {"room"})
    return T.Action(name, pre={"var:9": set(g), "room": {room}}, effects=(T.Effect("state:316", value=1),), what=rec)


def test_predict_leaves_paths_told_apart_by_an_unknown_dropped_fact_to_the_engine():
    """The review's case (test_replay_taskcore): two paths that differ only on g, which a guess left unknown and the
    file does not hold. Without ``choices`` it raises as before; with it the step is left to the engine, and the
    plan's state after it keeps what both paths write."""
    facts = {"var:9": Domain.of({0, 1, 2}), "room": Domain.of({28}), "state:316": BINARY}
    guess = T.Action("guess#0", effects=(T.Effect("var:9", values=frozenset({1, 2})),), what=TR.rec(label="guess"))
    pieces = [[guess], [walk("walk_to(316)#1", {1, 2})], [walk("walk_to(316)#2", {0})]]
    steps = [{"op": "a0", "name": "guess", "members": [0], "guessed": ()},
             {"op": "a1", "name": "fold:walk_to(316)#1+1", "members": [1, 2], "guessed": ()}]
    start = {"var:9": 0, "room": 28, "state:316": 0}
    with pytest.raises(plan_fd.PathChoice, match="and 2 may hold"):
        plan_fd.predict(facts, pieces, steps, start, held={"room", "state:316"})
    choices = {}
    paths, end = plan_fd.predict(facts, pieces, steps, start, held={"room", "state:316"}, choices=choices)
    assert [p.name for p in choices[1]] == ["walk_to(316)#1", "walk_to(316)#2"]
    assert paths[1].name == "fold:walk_to(316)#1+1" and paths[1].pre == {"var:9": {0, 1, 2}, "room": {28}}
    assert end == {"var:9": plan_fd.UNKNOWN, "room": 28, "state:316": 1}
    # two paths that both surely hold are still the planner's fault
    with pytest.raises(plan_fd.PathChoice, match="2 of its 2 paths hold"):
        plan_fd.predict(facts, [[walk("a#0", {0})], [walk("a#1", {0, 1})]],
                        [{"op": "a0", "name": "a", "members": [0, 1], "guessed": ()}], start, choices={})


class DoorBranch(TR.FakeBranch):
    door = 0

    def state(self):
        return dict(super().state(), states={316: self.door})


def several(var9: int):
    one, two = walk("walk_to(316)#1", {1, 2}), walk("walk_to(316)#2", {0})

    def opens(fb, f, clicks):
        if clicks:
            fb.door = 1
    h, fb = TR.harness(opens, one.what)
    fb.__class__ = DoorBranch
    fb.vars[9] = var9
    h.doms["var:9"] = Domain.of({0, 1, 2})
    h.doms["state:316"] = BINARY
    h.task = T.Task(h.doms, [plan_fd.either("fold:walk_to(316)#1+1", [one, two])], {"state:316": {1}})
    h.choices = {0: [one, two]}
    h.observe()
    return h, fb, one, two


def test_the_harness_takes_the_path_the_engines_state_holds():
    h, fb, one, two = several(0)
    st = plan_fd.Step("a1", 0, "fold:walk_to(316)#1+1", (), ())
    assert h.play_step(0, 0, st) == "next"
    log = h.steps[-1]
    assert h.task.actions[0] is two and h.choices == {} and "walk_to(316)#2" in log.note
    assert log.outcome == "matched" and len(log.tries) == 1 and h.inputs()  # it played the walk


def test_a_step_none_of_whose_paths_holds_is_a_replan_without_a_ban():
    h, fb, one, two = several(7)  # var:9 = 7: neither path's set (the domain lacks it)
    st = plan_fd.Step("a1", 0, "fold:walk_to(316)#1+1", (), ())
    assert h.play_step(0, 0, st) == "replan"
    log = h.steps[-1]
    assert log.outcome == "precondition" and log.then == "replan" and h.bans == [] and not log.tries
    assert log.end - log.start == RP.INPUT_LIMIT and h.inputs() == []
    assert {m["key"] for m in log.mismatches} == {"var:9"}


def test_ran_on_takes_the_path_the_plans_state_holds():
    """A step of several paths among the events the engine ran on through: the path the plan's state there holds."""
    one = T.Action("arrive#1", pre={"pos:script:2": {736}, "var:9": {0}},
                   effects=(T.Effect("pos:script:2", value=937),),
                   what=TR.rec("event", "arrive", {}, {"pos:script:2": ("set", 937)}))
    two = T.Action("arrive#2", pre={"pos:script:2": {736}, "var:9": {1}},
                   effects=(T.Effect("pos:script:2", value=-1),),
                   what=TR.rec("event", "arrive", {}, {"pos:script:2": ("set", -1)}))
    doms = {"pos:script:2": Domain.of({-1, 736, 937}), "var:9": Domain.of({0, 1})}
    task = T.Task(doms, [None, plan_fd.either("fold:arrive", [one, two])], {})
    exps = {"pos:script:2": RP.Expect("pos:script:2", frozenset({736}), "set")}
    hist = RP.History({"pos:script:2": -1, "var:9": 0}, 10)
    hist.note(11, {"pos:script:2": 937, "var:9": 0})
    mism = [RP.Mismatch("pos:script:2", ("736",), 937, "write")]
    choices = {1: [one, two]}
    chain = RP.ran_on(task, doms, exps, mism, {"pos:script:2": -1, "var:9": 0},
                      [plan_fd.Step("a", 1, "fold:arrive", (), ())], hist, choices)
    assert [st.name for st, _ in chain] == ["fold:arrive"] and task.actions[1] is one and choices == {}


def test_the_harness_records_the_first_frame_each_watched_fact_holds():
    def opens(fb, f, clicks):
        if f == 103:
            fb.vars[9] = 2
    h, fb = TR.harness(opens, TR.walk_rec())
    h.watch = {"#1": ("var:9", "==", 2), "#2": ("var:9", "!=", 5), "#3": ("var:10", "==", 1)}
    h.observe()
    h.advance(5)
    for _ in range(4):
        h.advance()
    assert h.reached == {"#2": 100, "#1": 105}  # seen on the frames the harness observes


def test_landmarks_come_back_through_taskcore(tmp_path):
    import test_replay_taskcore as TT
    b, _, task = TT._harness_task()
    E.export_task(tmp_path / "k.tc", task, {}, meta=lambda a: (a.what.kind, a.what.end, ""),
                  naming=E.record_naming)
    h = RP.Harness(b, task, task, TT._fake_branch(), {"state:316": {1}}, packed=tmp_path / "k.tc",
                   workdir=tmp_path / "plans")
    h.observe()
    assert [s.name for s in h.plan(first=True)] == ["walk_to(316)#0"]
    p = h.plans[0]
    assert p["landmarks"] > 0 and p["game_landmarks"] > 0 and p["choices"] == {}
    facts = {tuple(f) for n in h.landmarks["nodes"] for f in n["facts"]}
    assert ("state:316", 1) in facts and h.landmarks["plan"] == 0


# --- the runner's report ------------------------------------------------------------------------------------------

def step_dict(name, outcome, tries=1, mismatches=(), seconds=1.0, start=0, end=10):
    return {"name": name, "outcome": outcome, "tries": [{}] * tries, "mismatches": list(mismatches),
            "seconds": seconds, "start": start, "end": end}


def test_the_summary_counts_as_the_runners_docstring_says():
    res = {"steps": [step_dict("a", "matched"),
                     step_dict("b", "matched", mismatches=[{"kind": "guess"}]),
                     step_dict("c", "ran on in the step before", tries=0, seconds=0),
                     step_dict("d", "mismatch", mismatches=[{"kind": "write"}, {"kind": "write"}], seconds=3.0),
                     step_dict("e", "precondition", tries=0, seconds=0),
                     step_dict("f", "already happened", tries=0, seconds=0),
                     step_dict("g", "path not started", tries=4, seconds=2.0, end=40)],
           "plans": [{"write_s": 1.0, "seconds": 2.0, "decode_s": 0.5}, {"write_s": 1.0, "seconds": 1.0}],
           "inputs": [{"frame": 1, "click": [1, 2]}, {"frame": 2, "key": "o"}, {"frame": 3, "skips": {}}],
           "seconds": 20.0}
    s = P1.summary(res, {"#5": 100, "#1": 150, "#2": 120}, 100, 99)
    assert s["executed"] == 6 and s["played"] == 4 and s["matched"] == 4 and s["matched_share"] == round(4 / 6, 3)
    assert s["matched_with_another_guess"] == 1 and s["mismatch_steps"] == 1 and s["mismatch_facts"] == 2
    assert s["mismatch_facts_per_executed"] == round(2 / 6, 3) and s["final_path"] == ["a", "b", "c", "f"]
    assert s["inputs"] == 2 and s["outcomes"]["precondition"] == 1
    assert s["key_goals"] == {"of": 99, "passed": 3, "held_at_start": 1, "reached_in_the_run": 2, "at_start": ["#5"],
                              "reached": {"#1": 150, "#2": 120}}
    t = s["time"]
    assert t["planning_s"] == 5.5 and t["playing_s"] == 14.5 and t["step_seconds"]["n"] == 4
    assert t["step_frames"]["max"] == 40


def test_the_runner_refuses_the_any_and_the_test_seeds(tmp_path):
    for seed in (31, 35, P1.ANY_SEED):
        with pytest.raises(SystemExit, match="refuses"):
            P1.cmd_run(seed, tmp_path / "missing.json", False, 1, 1, 1)
    with pytest.raises(ValueError, match="Any%"):
        P1.config(P1.ANY_SEED, tmp_path)


def test_the_part1_goal_and_the_key():
    goal = P1.goal_of("part1", lambda k: Domain.of({1, 15}))
    assert goal == {"owner:901": {1}, "owner:902": {1}, "owner:903": {1}}
    assert P1.goal_of("state:428=1,bit:6=0", None) == {"state:428": {1}, "bit:6": {0}}
    keys = P1.key_goals()
    assert len(keys) == 99 and keys["#1"] == ("state:428", "==", 1) and keys["#97"] == ("owner:901", "==", 1)


# --- the kitchen ------------------------------------------------------------------------------------------------------

KITCHEN_CONTROL = {  # record: (the facts the control keeps, the facts it drops)
    "walk_to(316)#478": (["class:316:32", "frozen:obj:2", "frozen:script:2", "room", "script:2", "var:53"],
                         ["bit:6"]),
    "sentence(9,566)#12118": (["class:566:32", "frozen:obj:2", "frozen:script:2", "owner:566", "room", "script:2",
                               "var:53", "verb:9"], ["bit:6"]),
    "event(local:211@33)#30568": (["frozen:local:211", "local:211", "pos:local:211", "room"],
                                  ["class:316:6", "local:203", "local:220", "state:316"]),
}


def kitchen_control(work: Path) -> dict:
    """The kitchen reading as files and its control: the counts, and what the control keeps and drops on the main
    plan's records (the walk to the door, the meat, the cook's timer)."""
    import time
    work.mkdir(parents=True, exist_ok=True)
    b, recs, _ = pickle.loads(Path(os.environ["READER_PICKLE"]).read_bytes())
    t = time.monotonic()
    info = PR.export_reading(work, b, recs, {"owner:566": {1}}, "groundtruth pre of S1#7", {"name": "kitchen"})
    export_s = time.monotonic() - t
    del recs
    t = time.monotonic()
    c = E.control_records(work / "reading.tc", work / "reading.actions.jsonl", work / "control.tc")
    c["seconds"] = round(time.monotonic() - t, 2)
    rd = PR.load_reading(work / "reading.json")
    ct = PR.PackedTask(PR.Packed(work / "control.tc"), PR.Lines(work / "reading.actions.jsonl"), rd.doc["bounds"])
    seen = {}
    by_key = collections.Counter()
    for i, line in enumerate(rd.task.lines):
        name = f"{line['label']}#{line['rec']}"
        if name in KITCHEN_CONTROL:
            a, q = rd.task.actions[i], ct.actions[i]
            seen[name] = (sorted(q.pre), sorted(set(a.pre) - set(q.pre)), sorted(a.what.engine_pre))
    for a, q in zip(rd.task.actions, ct.actions):
        assert a.effects == q.effects and set(q.pre) <= set(a.pre)
        by_key.update(set(a.pre) - set(q.pre))
    rd.close()
    ct.close()
    return {"export": dict(info, seconds=round(export_s, 1)), "control": c, "records": seen,
            "most_dropped": by_key.most_common(15)}


@pytest.mark.slow
@pytest.mark.skipif(not os.environ.get("READER_PICKLE"), reason="needs READER_PICKLE (the kitchen reading)")
def test_the_control_on_the_kitchen(tmp_path):
    out = kitchen_control(tmp_path)
    assert out["control"]["actions"] == 71729 and out["control"]["sets dropped"] > 0
    for name, (keep, drop) in KITCHEN_CONTROL.items():
        assert out["records"][name][:2] == (keep, drop), name


def write_control_log(work: Path) -> None:
    out = kitchen_control(work)
    c = out["control"]
    lines = ["E003 step 5: the control on the kitchen (export_records.control_records; test_replay_part1.py)", "",
             f"reading: READER_PICKLE {os.environ['READER_PICKLE']} as files (packed_reading.export_reading): "
             f"{out['export']['actions']:,} actions, {out['export']['facts']} facts, {out['export']['bytes']:,} bytes, "
             f"{out['export']['seconds']} s",
             f"control: {c['actions']:,} actions; precondition sets kept {c['sets kept']:,}, dropped "
             f"{c['sets dropped']:,} on {c['actions with a set dropped']:,} actions; {c['bytes']:,} bytes; "
             f"{c['seconds']} s",
             f"dropped sets by fact kind: {c['dropped_by_fact']}",
             f"dropped sets by action kind: {c['dropped_by_kind']}",
             f"the facts dropped most often: {out['most_dropped']}", "",
             "the main plan's records (kept by the control | dropped | the record's engine conditions):"]
    for name, (keep, drop, eng) in out["records"].items():
        lines.append(f"  {name}: kept {keep} | dropped {drop} | engine {eng}")
    (HERE / "results" / "control-kitchen.log").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    if sys.argv[1:2] == ["control-kitchen"]:
        write_control_log(Path(sys.argv[2]))
    else:
        print(__doc__)
