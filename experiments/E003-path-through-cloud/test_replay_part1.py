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


def test_a_reading_with_elsewhere_starts_a_slot_at_a_place_none_of_its_points_there():
    """The Rust reader's switch (scumm/reader/PORT-NOTES.md, "ELSEWHERE"): where pos:'s domain holds ELSEWHERE, a slot
    the engine shows at a place none of the points, and at none of them, starts at ELSEWHERE (task_start, and
    replan_start with args 0); at a point, or where the domain lacks ELSEWHERE, the start is reader.task_start's."""
    facts = {"pos:script:2": Domain.of({-1, 736, 937}), "pos:local:200": Domain.of({RP.ELSEWHERE, -1, 4, 11}),
             "args:local:200": Domain.of({0, 7})}
    task = types.SimpleNamespace(facts=facts)
    b = PR.Points(facts)
    assert b.elsewhere == {"pos:local:200"}
    assert b.table.points["at"] == {"script:2": {736, 937}, "local:200": {4, 11}}
    obs = {"at:local:200:10": 1, "at:script:2:5": 1}
    assert R.task_start(b, obs, task)["pos:local:200"] == R.NOWHERE  # reader.py's own
    st = RP.task_start(b, obs, task)
    assert st["pos:local:200"] == RP.ELSEWHERE and st["pos:script:2"] == R.NOWHERE  # no ELSEWHERE in its domain
    st, _ = RP.replan_start(b, obs, {"pos:local:200": RP.ELSEWHERE, "args:local:200": 7}, task)
    assert st["pos:local:200"] == RP.ELSEWHERE and st["args:local:200"] == 0
    assert RP.task_start(b, {"at:local:200:11": 1, "at:local:200:10": 1}, task)["pos:local:200"] == 11
    assert RP.task_start(b, {}, task)["pos:local:200"] == R.NOWHERE
    assert RP.view(b, facts, obs)["pos:local:200"] == RP.ELSEWHERE


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


def golden_file(path, recs, head=None):
    """``recs`` in golden.py's reading format (what the Rust reader is compared with, and writes)."""
    import golden
    head = head or {"name": "hand", "rooms": [28, 41], "verbs": [2, 9], "dialog_verbs": [109, 121], "walk_verb": 11,
                    "n_records": len(recs), "sources": {"reader.py": "x"}}
    lines = [golden.line({"kind": "reading", "reading": golden.enc(head)})]
    lines += [golden.line({"kind": "record", "index": i, "record": golden.enc(r)}) for i, r in enumerate(recs)]
    lines.append(golden.line({"kind": "builder", "builder": golden.enc({"counts": {"a counter": 3}})}))
    path.write_text("\n".join(lines) + "\n")
    return path


def test_a_reading_from_golden_format_records_and_a_packed_file_is_the_python_export(tmp_path):
    """The Rust reader's path: its packed file and its records in golden.py's format give the action lines and
    reading.json the Python export writes, byte for byte and field for field, and the runner reads them the same."""
    b, recs, _ = reading(tmp_path / "py")
    g = golden_file(tmp_path / "hand.jsonl", recs)
    out = PR.reading_from_golden(g, tmp_path / "py" / "reading.tc", tmp_path / "g", {"room": 28, "var:53": 1},
                                 "a hand-made start", domain=DOMS.get)
    assert out["actions"] == 3 and out["records"] == 4 and out["bounds"] == 1
    assert (tmp_path / "g" / "reading.actions.jsonl").read_bytes() == \
        (tmp_path / "py" / "reading.actions.jsonl").read_bytes()
    a, p = (json.loads((tmp_path / d / "reading.json").read_text()) for d in ("g", "py"))
    for k in ("format", "packed", "actions", "n_actions", "n_records", "folded", "goal", "start", "scope", "bounds",
              "counts", "control"):
        assert a[k] == p[k], k
    rd, rp = PR.load_reading(tmp_path / "g" / "reading.json"), PR.load_reading(tmp_path / "py" / "reading.json")
    assert [x.name for x in rd.task.actions] == [x.name for x in rp.task.actions]
    assert [x.what.checks for x in rd.task.actions] == [x.what.checks for x in rp.task.actions]
    assert [x.what.source for x in rd.task.actions] == [x.what.source for x in rp.task.actions]
    rd.close()
    rp.close()
    assert PR.check_reading(tmp_path / "g" / "reading.json")["ok"]
    # on worker processes: the same files
    out2 = PR.reading_from_golden(g, tmp_path / "py" / "reading.tc", tmp_path / "w", {"room": 28, "var:53": 1},
                                  "a hand-made start", domain=DOMS.get, workers=3)
    assert out2["actions"] == 3 and out2["records"] == 4
    for f in ("reading.actions.jsonl", "reading.tc"):
        assert (tmp_path / "w" / f).read_bytes() == (tmp_path / "g" / f).read_bytes(), f
    w, a = (json.loads((tmp_path / d / "reading.json").read_text()) for d in ("w", "g"))
    assert w == a


def test_a_reading_with_live_locals_carries_each_slots_read_locals(tmp_path):
    """The Rust reader's switch --live-locals (scumm/reader/PORT-NOTES.md, "Live locals"): the golden head's
    ``live_locals`` goes into reading.json unchanged and the harness reads it (Points.read_locals, replay.read_view):
    a slot's locals as a list holds them are the ones its unit reads; every local for a unit whose reads are not known
    exactly, a slot the reading does not list, or a reading made without the switch (no ``live_locals`` key at all).
    The starts do not change: task_start gives a live slot args 0, replan_start copies the plan's number where the
    slot is where the plan put it."""
    b, recs, _ = reading(tmp_path / "py")
    ll = {"switch": "--live-locals", "slots": {"script:2": [0, 1, 2], "local:202@31": [0], "local:202@52": [],
                                              "script:153": None}}
    head = {"name": "hand", "rooms": [28, 41], "verbs": [2, 9], "dialog_verbs": [109, 121], "walk_verb": 11,
            "n_records": len(recs), "sources": {"reader.py": "x"}, "live_locals": ll}
    on = golden_file(tmp_path / "on.jsonl", recs, head)
    off = golden_file(tmp_path / "off.jsonl", recs)
    for g, d in ((on, "on"), (off, "off")):
        PR.reading_from_golden(g, tmp_path / "py" / "reading.tc", tmp_path / d, {"room": 28, "var:53": 1},
                               "a hand-made start", domain=DOMS.get)
    a, o = (json.loads((tmp_path / d / "reading.json").read_text()) for d in ("on", "off"))
    assert a["live_locals"] == ll and "live_locals" not in o
    assert {k: v for k, v in a.items() if k not in ("live_locals", "from_golden")} == \
        {k: v for k, v in o.items() if k != "from_golden"}
    rd, ro = PR.load_reading(tmp_path / "on" / "reading.json"), PR.load_reading(tmp_path / "off" / "reading.json")
    locs = {0: 202, 1: 7, 2: 9, 3: 1}
    assert rd.b.read_locals("local:202", 31) == frozenset({0}) and rd.b.read_locals("local:202", 52) == frozenset()
    assert RP.read_view(rd.b, "local:202", 31, locs) == {0: 202}
    assert RP.read_view(rd.b, "local:202", 52, locs) == {}
    assert RP.read_view(rd.b, "script:2", 31, locs) == {0: 202, 1: 7, 2: 9}
    assert RP.read_view(rd.b, "script:153", 31, locs) == locs  # reads not known exactly: all
    assert RP.read_view(rd.b, "local:202", 33, locs) == locs  # a slot the reading does not list: all
    assert ro.b.read_locals("local:202", 31) is None and RP.read_view(ro.b, "local:202", 31, locs) == locs
    assert RP.read_view(b, "local:202", 31, locs) == locs  # a Python reading: no switch
    obs, pred = {"at:script:2:736": 1}, {"pos:script:2": 736, "args:script:2": 9}
    assert RP.task_start(rd.b, obs, rd.task) == RP.task_start(ro.b, obs, ro.task)
    assert RP.replan_start(rd.b, obs, pred, rd.task) == RP.replan_start(ro.b, obs, pred, ro.task)
    assert RP.replan_start(rd.b, obs, pred, rd.task)[0]["args:script:2"] == 9
    rd.close()
    ro.close()


def test_the_lines_follow_the_packed_files_order_whatever_it_is(tmp_path):
    """Another writer may put the actions in another order (the Rust RecordStream rewrites the body at finish): each
    line comes from the record its action's ``rec`` names."""
    b, recs, _ = reading(tmp_path / "py")
    full = R.build_task(b, recs, {"state:316": {1}})
    rev = T.Task(full.facts, list(reversed(full.actions)), full.goal)
    E.export_task(tmp_path / "rev.tc", rev, {}, meta=lambda a: (a.what.kind, a.what.end, E._cont(a.what)),
                  naming=E.record_naming)
    g = golden_file(tmp_path / "hand.jsonl", recs)
    PR.reading_from_golden(g, tmp_path / "rev.tc", tmp_path / "g", {"room": 28, "var:53": 1}, "a hand-made start",
                           domain=DOMS.get)
    lines = [json.loads(x) for x in (tmp_path / "g" / "reading.actions.jsonl").read_text().splitlines()]
    assert [(x["i"], x["rec"]) for x in lines] == [(0, 3), (1, 1), (2, 0)]
    rd = PR.load_reading(tmp_path / "g" / "reading.json")
    assert [a.name for a in rd.task.actions] == [a.name for a in rev.actions]
    rd.close()


def test_the_reading_check_names_what_is_wrong(tmp_path):
    reading(tmp_path)
    lines = (tmp_path / "reading.actions.jsonl").read_text().splitlines()
    first = json.loads(lines[0])
    first["source"] = ["sentence", 11, "316", None]  # a number written as text
    first["checks"][0] = first["checks"][0][:3]
    (tmp_path / "reading.actions.jsonl").write_text("\n".join([json.dumps(first)] + lines[1:]) + "\n")
    doc = json.loads((tmp_path / "reading.json").read_text())
    doc["bounds"] = {}
    (tmp_path / "reading.json").write_text(json.dumps(doc))
    out = PR.check_reading(tmp_path / "reading.json")
    assert not out["ok"] and set(out["problems"]) == {"source", "check", "bounds"}
    assert out["problems"]["bounds"] == ["var:53"]


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


def test_the_controls_lines_drop_the_scripts_checks_and_make_its_steps_engine_conditions(tmp_path):
    """The control's action lines: no replay checks (the walk's test on var:104 is the scripts'), the record's own
    facts cut to the kept ones, and a stepped fact the record named (cutscene, its room kept) an engine condition, so
    the control's ban names the room alone; the full model's ban on the same record names bit:6 and cutscene too."""
    b = builder()
    walk = records()[0]
    walk = TR.rec("walk_to", "walk_to(316)", dict(walk.pre, cutscene=frozenset({0, 1, 2, 3})), dict(walk.effects),
                  walk.checks, walk.source, walk.engine_pre)
    PR.export_reading(tmp_path, b, [walk], {"state:316": {1}}, "a hand-made start", {"name": "hand"})
    c = E.control_records(tmp_path / "reading.tc", tmp_path / "reading.actions.jsonl", tmp_path / "control.tc",
                          tmp_path / "control.actions.jsonl")
    assert c["checks dropped"] == 1 and c["stepped facts made engine conditions"] == 1 and c["sets dropped"] == 1
    line = json.loads((tmp_path / "control.actions.jsonl").read_text())
    full = json.loads((tmp_path / "reading.actions.jsonl").read_text())
    assert line["checks"] == [] and len(full["checks"]) == 1
    assert line["pre_keys"] == ["cutscene", "room", "script:2", "var:53"] and "bit:6" in full["pre_keys"]
    assert line["engine_pre"] == ["cutscene", "room", "script:2", "var:53"]
    assert {k: v for k, v in line.items() if k not in ("checks", "pre_keys", "engine_pre")} == \
        {k: v for k, v in full.items() if k not in ("checks", "pre_keys", "engine_pre")}
    rd = PR.load_reading(tmp_path / "reading.json")
    ct = PR.PackedTask(PR.Packed(tmp_path / "control.tc"), PR.Lines(tmp_path / "control.actions.jsonl"),
                       rd.doc["bounds"])
    state = {"room": 28, "bit:6": 0, "script:2": 0, "cutscene": 0, "var:53": 1}
    assert RP.ban_of(ct.actions[0].what, state) == RP.Ban("walk_to(316)", (("room", 28),))
    assert RP.ban_of(rd.task.actions[0].what, state) == RP.Ban("walk_to(316)", (("bit:6", 0), ("cutscene", 0),
                                                                                ("room", 28)))
    assert ct.actions[0].what.checks == [] and ct.actions[0].pre["cutscene"] == rd.task.actions[0].pre["cutscene"]
    ct.close()
    rd.close()


def test_the_runner_makes_the_control_once(tmp_path):
    reading(tmp_path)
    out = P1.make_control(tmp_path / "reading.json")
    doc = json.loads(out.read_text())
    assert out.name == "control.json" and doc["control"] and doc["packed"] == "control.tc"
    assert doc["actions"] == "control.actions.jsonl" and doc["control_counts"]["sets dropped"] == 3
    mtime = (tmp_path / "control.tc").stat().st_mtime_ns
    assert P1.make_control(tmp_path / "reading.json") == out and (tmp_path / "control.tc").stat().st_mtime_ns == mtime
    rd = PR.load_reading(out)
    assert sorted(rd.task.actions[0].pre) == ["cutscene", "room", "script:2", "var:53"]
    assert rd.task.actions[0].what.checks == []  # the control's harness reads none of the scripts' checks
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


def walk_writes(name, args, writes):
    """A walk whose precondition names args:script:2 (a slot's locals: the dump does not show them) and which
    writes ``writes``."""
    rec = TR.rec("walk_to", "walk_to(316)", {"args:script:2": frozenset({args}), "room": frozenset({28})},
                 {k: ("set", v) for k, v in writes.items()}, (), ("sentence", 11, 316, None), {"room"})
    return T.Action(name, pre={"args:script:2": {args}, "room": {28}},
                    effects=tuple(T.Effect(k, value=v) for k, v in writes.items()), what=rec)


def test_several_paths_the_engine_cannot_tell_apart_are_played_once_and_the_writes_choose():
    """Two paths of one step that differ only on args:script:2 (no plan value): both hold, so the walk is clicked once
    and the step is the path whose writes the engine shows (the door: path 2), with no replan."""
    one, two = walk_writes("walk_to(316)#1", 0, {"bit:6": 1}), walk_writes("walk_to(316)#2", 9, {"state:316": 1})

    def opens(fb, f, clicks):
        if clicks:
            fb.door = 1
    h, fb = TR.harness(opens, one.what)
    fb.__class__ = DoorBranch
    h.doms["args:script:2"] = Domain.of({0, 9})
    h.goal = {"bit:6": {1}}  # not reached: the step is judged on its own
    h.task = T.Task(h.doms, [plan_fd.either("fold:walk_to(316)#1+1", [one, two])], h.goal)
    h.choices = {0: [one, two]}
    h.observe()
    assert h.play_step(0, 0, plan_fd.Step("a1", 0, "fold:walk_to(316)#1+1", (), ())) == "next"
    log = h.steps[-1]
    assert log.outcome == "matched" and len(log.tries) == 1 and log.tries[0]["path"] == "walk_to(316)#2"
    assert h.task.actions[0] is two and h.choices == {} and h.bans == []
    assert "played once" in log.note and "walk_to(316)#2" in log.note


def test_the_reviews_two_holding_paths_no_longer_spin():
    """The review's case (two paths whose sets on var:9 overlap, both holding): it replanned at once from the same
    frame until the replan limit; now the click is played once and the run reaches the goal."""
    one, two = walk("walk_to(316)#1", {0, 1}), walk("walk_to(316)#2", {0})

    def opens(fb, f, clicks):
        if clicks:
            fb.door = 1
    h, fb = TR.harness(opens, one.what)
    fb.__class__ = DoorBranch
    fb.vars[9] = 0
    h.doms["var:9"] = Domain.of({0, 1, 2})
    h.doms["state:316"] = BINARY
    h.goal = {"state:316": {1}}
    h.replan_limit = 5

    def plan(first):
        h.task = T.Task(h.doms, [plan_fd.either("fold:walk_to(316)#1+1", [one, two])], {"state:316": {1}})
        h.choices = {0: [one, two]}
        h.plans.append({"plan": len(h.plans), "reorders": []})
        return [plan_fd.Step("a1", 0, "fold:walk_to(316)#1+1", (), ())]
    h.plan = plan
    res = h.run()
    assert res["stop"] == "goal" and res["replans"] == 0 and len(res["plans"]) == 1 and res["bans"] == []
    assert [s["outcome"] for s in res["steps"]] == ["matched"] and res["inputs"]


def test_a_replan_with_the_last_plans_input_on_the_same_frame_stops_the_run(tmp_path):
    """The planner is deterministic: the same start and bans on the same frame give the same plan, played the same
    way. The second plan stops the run; a frame later, or with a new ban, it plans."""
    import test_replay_taskcore as TT
    b, (r, other, flip), task = TT._harness_task()
    E.export_task(tmp_path / "k.tc", task, {}, meta=lambda a: (a.what.kind, a.what.end, ""), naming=E.record_naming)
    h = RP.Harness(b, task, task, TT._fake_branch(), {"state:316": {1}}, packed=tmp_path / "k.tc",
                   workdir=tmp_path / "plans")
    h.observe()
    assert [s.name for s in h.plan(first=True)] == ["walk_to(316)#0"]
    assert h.plan(first=False) == [] and h.plans[1]["status"] == "same input"
    assert h.stop_reason.startswith(f"replan at frame {h.branch.frame} with plan 0's start and bans")
    h.stop_reason = None
    h.bans.append(RP.ban_of(r, h.belief()))
    assert [s.name for s in h.plan(first=False)] == ["sentence(1,7)#2", "sentence(2,316)#1"]
    h.advance(1)
    assert h.plan(first=False) and h.plans[-1]["status"] == "plan"


def test_an_unexpected_error_stops_the_run_and_keeps_its_record():
    h, fb = TR.harness(lambda fb, f, c: None, TR.walk_rec())
    seen = []

    def plan(first):
        h.plans.append({"plan": len(h.plans), "reorders": []})
        raise KeyError("boom")
    h.plan = plan
    h.on_plan = seen.append
    res = h.run()
    assert res["stop"] == "error: KeyError: 'boom'" and "Traceback" in res["error"] and len(res["plans"]) == 1
    assert res["frame0"] == 100 and res["inputs"] == [] and seen == []  # the plan never returned


def test_the_snapshot_after_each_plan_is_the_record_so_far():
    h, fb = TR.harness(lambda fb, f, c: None, TR.walk_rec())
    snaps = []

    def plan(first):
        h.plans.append({"plan": len(h.plans), "reorders": []})
        h.stop_reason = "no plan (the planner proved none)"
        return []
    h.plan = plan
    h.on_plan = lambda hh: snaps.append(hh.result())
    res = h.run()
    assert len(snaps) == 1 and snaps[0]["plans"] == res["plans"] and snaps[0]["stop"] == res["stop"]


def test_a_step_the_engine_ran_on_through_counts_the_next_events_writes_as_predicted():
    """The walk's window holds the arrival's writes (the slot past its wait, the door open): they are the plan's next
    event's, so they are not "changed with no prediction"; local:203, which nothing predicts, is."""
    def past_the_wait(fb, f, clicks):
        if clicks:
            fb.slots[2] = {"where": TR.WIO_GLOBAL, "number": 2, "code": 937, "frozen": 0}
            fb.slots[203] = {"where": TR.WIO_LOCAL, "number": 203, "code": 58, "frozen": 0}
            fb.door = 1
    arrive = TR.rec("event", "event(script:2@736)", {"script:2": frozenset({1}), "pos:script:2": frozenset({736})},
                    {"pos:script:2": ("set", 937), "state:316": ("set", 1)})
    h, fb = TR.harness(past_the_wait, TR.walk_rec(), arrive)
    fb.__class__ = DoorBranch
    h.goal = {"bit:6": {1}}
    h.observe()
    assert h.play_step(0, 0, TR.step(0), [TR.step(1, "arrive")]) == "next"
    log = h.steps[-1]
    assert log.outcome == "matched, the engine ran on"
    assert {u["key"] for u in log.unplanned} == {"local:203", "pos:local:203"}
    assert all(u["net"] and u["end"] == 1 or u["key"] == "pos:local:203" for u in log.unplanned)


def test_history_marks_a_change_that_came_back():
    hist = RP.History({"var:182": 0, "bit:6": 0}, 10)
    hist.note(11, {"var:182": "above", "bit:6": 1})
    hist.note(12, {"var:182": 0, "bit:6": 1})
    un = {u["key"]: u for u in hist.unplanned(set())}
    assert un["var:182"]["net"] is False and un["var:182"]["end"] == 0 and un["var:182"]["count"] == 2
    assert un["bit:6"]["net"] is True and un["bit:6"]["end"] == 1


def test_the_start_check_names_the_task_facts_another_start_holds(tmp_path):
    """A reading serves the start it was read from: a start that differs on a fact of the task (var:53) is named, one
    that differs only elsewhere (var:100) is not."""
    reading(tmp_path)
    rd = PR.load_reading(tmp_path / "reading.json")
    row = {"seed": 1, "stop": {"frame": 2728}, "checks": [], "facts": {"room": 28, "var:53": 1, "var:100": 20}}
    chk = P1.start_check(row, rd, tmp_path / "check")
    assert chk["reading start"]["differ"] == 1 and chk["reading start"]["task facts differ"] == 0
    row["facts"]["var:53"] = 2
    chk = P1.start_check(row, rd, tmp_path / "check")
    assert chk["reading start"]["task facts differ"] == 1 and chk["reading start"]["task first"] == [("var:53", (1, 2))]
    rd.close()


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

def step_dict(name, outcome, tries=1, mismatches=(), seconds=1.0, start=0, end=10, kind="sentence", unplanned=(),
              inputs=True):
    return {"name": name, "outcome": outcome, "kind": kind, "seconds": seconds, "start": start, "end": end,
            "tries": [{"inputs": [{"frame": 1, "click": [1, 2]}] if inputs and kind != "event" else []}] * tries,
            "mismatches": list(mismatches), "unplanned": list(unplanned)}


def change(key, start, end):
    return {"key": key, "start": start, "changes": [[5, end]], "count": 1, "end": end, "net": start != end}


def test_the_summary_counts_as_the_runners_docstring_says():
    """plan.md's match: the predicted writes and the tracked facts' changes. A step whose path never started, did not
    happen or mismatched is a mismatch, and so is a matched step that changed a tracked fact with no prediction; a
    fact that came back is not; a value inside a guess but not the plan's choice is counted apart."""
    w = {"kind": "write", "key": "x", "predicted": ["1"], "observed": 0}
    res = {"steps": [step_dict("a", "matched"),
                     step_dict("b", "matched", mismatches=[{"kind": "choice", "predicted": ["2"]}]),
                     step_dict("c", "ran on in the step before", tries=0, seconds=0, kind="event"),
                     step_dict("d", "mismatch", mismatches=[w, w], seconds=3.0),
                     step_dict("e", "precondition", tries=0, seconds=0),
                     step_dict("f", "already happened", tries=0, seconds=0, kind="event"),
                     step_dict("g", "path not started", tries=4, seconds=2.0, end=40, mismatches=[w]),
                     step_dict("h", "matched", unplanned=[change("var:182", 0, 255), change("bit:9", 0, 0)]),
                     step_dict("i", "interface failure", tries=2, inputs=False),
                     step_dict("j", "did not happen", kind="event", mismatches=[w]),
                     step_dict("k", "matched, the engine ran on", mismatches=[w], kind="event")],
           "plans": [{"write_s": 1.0, "seconds": 2.0, "decode_s": 0.5}, {"write_s": 1.0, "seconds": 1.0}],
           "inputs": [{"frame": 1, "click": [1, 2]}, {"frame": 2, "key": "o"}, {"frame": 3, "skips": {}}],
           "seconds": 20.0}
    s = P1.summary(res)
    # executed: all but the precondition (no try) and the interface failure (no input reached the engine)
    assert s["executed"] == 9 and s["played"] == 8
    assert s["writes_held"] == 6 and s["final_path"] == ["a", "b", "c", "f", "h", "k"]
    assert s["matched"] == 5 and s["matched_share"] == round(5 / 9, 3)
    assert s["not_matched"] == {"did not happen": 1, "mismatch": 1, "path not started": 1,
                                "writes held, tracked facts changed": 1}
    assert s["mismatched_writes"] == 4 and s["unplanned_changes"] == 1 and s["mismatches"] == 5
    assert s["mismatches_per_executed"] == round(5 / 9, 3) and s["unplanned_most"] == [("var:182", 1)]
    assert s["another_guess"] == 1 and s["inputs"] == 2 and s["outcomes"]["precondition"] == 1
    t = s["time"]
    assert t["planning_s"] == 5.5 and t["playing_s"] == 14.5 and t["step_seconds"]["n"] == 8
    assert t["step_frames"]["max"] == 40


def test_the_summary_reads_a_record_from_before_the_net_field():
    old = {"key": "var:182", "start": 0, "changes": [[5, "above"], [9, 0]], "count": 2}
    assert not P1.net(old) and P1.net(dict(old, changes=[[5, "above"]], count=1))
    assert P1.net(dict(old, count=20))  # its last value is not kept: counted


def test_the_part1_goal_and_the_key():
    import score_part1 as SP
    goal = P1.goal_of("part1", lambda k: Domain.of({1, 15}))
    assert goal == {"owner:901": {1}, "owner:902": {1}, "owner:903": {1}}
    assert P1.goal_of("state:428=1,bit:6=0", None) == {"state:428": {1}, "bit:6": {0}}
    keys, end = SP.key_goals()
    assert len(keys) == 99 and keys["#1"] == ("state:428", "==", 1) and keys["#97"] == ("owner:901", "==", 1)
    assert end == ["#97", "#98", "#99"]


def test_the_score_counts_goals_reached_after_the_start_on_every_frame():
    import score_part1 as SP
    goals = {"#1": ("state:428", "==", 1), "#2": ("bit:6", "!=", 1), "#3": ("owner:5", "==", 1),
             "#4": ("bit:7", "==", 1)}
    states = [(100, {"bit:6": 0}), (101, {"state:428": 1}), (102, {"owner:5": 1, "state:428": 1}),
              (103, {"bit:7": 1, "state:428": 0}), (104, {})]
    at_start, reached, both = SP.first_frames(states, goals, 100, ["#1", "#3", "#4"])
    assert at_start == {"#2"} and reached == {"#1": 101, "#3": 102, "#4": 103}  # #4 held for one frame only
    assert both is None  # #1, #3 and #4 never held on one frame
    assert SP.first_frames(states, goals, 100, ["#1", "#3"])[2] == 102


def test_the_runner_never_reads_the_key():
    import ast
    for name in ("replay.py", "replay_part1.py", "packed_reading.py", "export_records.py", "plan_fd.py"):
        tree = ast.parse((HERE / name).read_text())
        mods = {a.name for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names} | \
            {n.module for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
        assert not mods & {"end_part1", "score_part1"}, name


def test_the_runner_refuses_the_any_and_the_test_seeds(tmp_path):
    for seed in (31, 35, P1.ANY_SEED):
        with pytest.raises(SystemExit, match="refuses"):
            P1.cmd_run(seed, tmp_path / "missing.json", False, 1, 1, 1)
    with pytest.raises(ValueError, match="Any%"):
        P1.config(P1.ANY_SEED, tmp_path)


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
    c = E.control_records(work / "reading.tc", work / "reading.actions.jsonl", work / "control.tc",
                          work / "control.actions.jsonl")
    c["seconds"] = round(time.monotonic() - t, 2)
    rd = PR.load_reading(work / "reading.json")
    ct = PR.PackedTask(PR.Packed(work / "control.tc"), PR.Lines(work / "control.actions.jsonl"), rd.doc["bounds"])
    seen = {}
    by_key = collections.Counter()
    for i, line in enumerate(rd.task.lines):
        name = f"{line['label']}#{line['rec']}"
        if name in KITCHEN_CONTROL:
            a, q = rd.task.actions[i], ct.actions[i]
            seen[name] = (sorted(q.pre), sorted(set(a.pre) - set(q.pre)), sorted(a.what.engine_pre),
                          len(a.what.checks), len(q.what.checks), sorted(q.what.engine_pre))
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
             f"control's action lines: replay checks dropped {c['checks dropped']:,}; stepped facts made engine "
             f"conditions {c['stepped facts made engine conditions']:,}",
             f"dropped sets by fact kind: {c['dropped_by_fact']}",
             f"dropped sets by action kind: {c['dropped_by_kind']}",
             f"the facts dropped most often: {out['most_dropped']}", "",
             "the main plan's records (kept by the control | dropped | the record's engine conditions | replay checks "
             "in the full model and the control | the control's engine conditions):"]
    for name, (keep, drop, eng, nc, cc, ceng) in out["records"].items():
        lines.append(f"  {name}: kept {keep} | dropped {drop} | engine {eng} | checks {nc} and {cc} | control engine "
                     f"{ceng}")
    (HERE / "results" / "control-kitchen.log").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    if sys.argv[1:2] == ["control-kitchen"]:
        write_control_log(Path(sys.argv[2]))
    else:
        print(__doc__)
