"""Tests of the replay harness's taskcore planner: taskcore's --start-file, --bans and --ops, plan_fd.plan_taskcore,
and replay.Harness planning through it.

usage: python test_replay_taskcore.py [N] [seed]    (N random tasks, default 200; the summary on stdout)
       pytest test_replay_taskcore.py -m 'not slow'  (the fixed cases, and the same with N = 60)
       READER_PICKLE=<reading.pkl> python test_replay_taskcore.py kitchen <work dir> [pruned|full ...]
                                                     (the kitchen: the bans byte for byte, and plans with bans)
       TASKCORE_BIN=<binary>                         (default taskcore/target/release/taskcore)

1. **The bans, byte for byte.** replay.py's ban (``ban_of``'s point, ``apply_bans``/``cut``) applied in Python and
   written by task.py's ``write_sas``, against taskcore given the unbanned task and the bans (``--bans``) with no
   fold, no reduction and task.py's encoding: the same file, or both raise. The start goes as a file
   (``--start-file``, export_records.write_start) and must give the same bytes as the file's own start.
2. **With the fold and the reductions** (the default, fold every, PRODUCT_LIMIT 1): the plan lama-first finds on
   taskcore's file, read back by ``plan_fd.plan_taskcore``, runs on the unreduced banned task (Python, task.py's
   meaning, the plan's guessed values): each step's path is an action of ``apply_bans(task, bans)``, its
   precondition holds, the goal holds at the end, and no step reads a value the plan left unknown. And Fast
   Downward's optimal cost (astar(blind())) on taskcore's banned file is the banned task's own
   (test_taskcore.model_optimum).
3. **The path choice**: a folded step is the one path that holds in the predicted state; none or two raise.
4. **The harness** plans through taskcore on a hand-made task, with a ban, on a fake engine.
"""

import collections
import json
import os
import random
import sys
import tempfile
import types
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import plan_fd  # noqa: E402
import replay as RP  # noqa: E402
import task as T  # noqa: E402
import test_taskcore as TC  # noqa: E402
from domains import BINARY, Domain  # noqa: E402
from export_records import NONE, export_task, record_naming, write_bans, write_start  # noqa: E402

BIN = plan_fd.TASKCORE
RAW = ["--fold", "off", "--reduce", "none", "--encoding", "task"]
REDUCED = [("default", []), ("fold every", ["--fold", "every"]), ("PRODUCT_LIMIT 1", ["--product-limit", "1"])]


def what(label, kind="sentence", end="yield", cont=""):
    return types.SimpleNamespace(label=label, kind=kind, end=end, cont=cont)


def meta(a):
    return a.what.kind, a.what.end, a.what.cont


def by_label(a):
    return NONE, a.what.label


def run(args):
    import subprocess
    return subprocess.run([str(x) for x in args], capture_output=True, text=True)


def python_file(task, bans, start):
    """task.py's file of the banned task, "out of domain", or None (task.py's writer raises)."""
    try:
        return T.write_sas(RP.apply_bans(task, bans)).text(start)
    except T.OutOfDomain:
        return "out of domain"
    except ValueError:
        return None


def taskcore_file(work: Path, bans, start, args=RAW, embedded=False):
    """taskcore's file of work/t.tc with ``bans`` (none: no --bans) from ``start`` as a start file (or the file's own
    start "s"): the text, "out of domain", or None (taskcore raises)."""
    cmd = [BIN, "sas", "--task", work / "t.tc", "--out", work / "r.sas"] + list(args)
    cmd += ["--start", "s"] if embedded else ["--start-file", write_start(work / "start.txt", task_facts(work), start)]
    if bans is not None:
        cmd += ["--bans", write_bans(work / "bans.txt", bans)]
    r = run(cmd)
    if r.returncode != 0:
        return "out of domain" if "out of their domains" in r.stderr else None
    return (work / "r.sas").read_text()


_FACTS = {}


def task_facts(work: Path) -> dict:
    return _FACTS[work]


def export(work: Path, task, start) -> None:
    export_task(work / "t.tc", task, {"s": start}, meta=meta, naming=by_label)
    _FACTS[work] = task.facts


# --- the plan on the unreduced banned task (independent of plan_fd.predict) -----------------------------------------

def run_plan(task, banned, res, start) -> tuple:
    """Run ``res``'s paths on the banned task (module docstring, 2): (ok, why)."""
    unknown = object()

    def ident(a):  # an action of the banned task: its name (a piece's names the ban's key) and precondition
        return a.name, tuple(sorted((k, tuple(sorted(map(str, v)))) for k, v in a.pre.items())), a.effects
    acts = {ident(a) for a in banned.actions}
    dom = {k: d.values for k, d in task.facts.items()}

    def lab(k, v):
        x = task.facts[k].label(v) if isinstance(v, int) else v
        return x if x in dom[k] else unknown
    state = {k: lab(k, start.get(k, 0)) for k in task.facts}
    for i, (a, st) in enumerate(zip(res.paths, res.steps)):
        if ident(a) not in acts:
            return False, f"step {i + 1}: {a.name} is not an action of the banned task"
        if not all(state[k] in set(s) for k, s in a.pre.items()):
            return False, f"step {i + 1}: {a.name}'s precondition does not hold"
        new = dict(state)
        for e in a.effects:
            if any(state[k] is unknown for k, _ in e.when):
                new[e.key] = unknown
                continue
            if not all(state[k] in set(s) for k, s in e.when):
                continue
            d = task.facts[e.key]
            if e.values is None and e.add is None:
                new[e.key] = lab(e.key, e.value)
                continue
            if e.values is not None:
                vals = {lab(e.key, v) for v in e.values}
            else:
                vals = d.add(state[e.key], e.add) if state[e.key] is not unknown else set()
            got = {v for k, v, w in st.guessed if k == e.key and v in vals and all(state[c] in s for c, s in w)}
            new[e.key] = next(iter(vals)) if len(vals) == 1 else next(iter(got)) if len(got) == 1 else unknown
        state = new
    goal = all(state[k] in {task.facts[k].label(v) if isinstance(v, int) else v for v in s}
               for k, s in task.goal.items())
    return goal, None if goal else "the goal does not hold at the end"


# --- random tasks with random bans ------------------------------------------------------------------------------------

def labelled(task):
    """test_taskcore's random task with a label on each action (its name: the paths of one input share it)."""
    acts = [T.Action(a.name, pre=a.pre, effects=a.effects, cost=a.cost, what=what(a.name, *a.what)) for a in task.actions]
    return T.Task(task.facts, acts, task.goal)


def rand_bans(rng, task) -> list:
    """Bans as the harness makes them (a label, the values of some facts there), often on a point an action of
    that label holds; now and then a key the task lacks, or no key at all, or two on one label."""
    out = []
    for _ in range(rng.randint(1, 3)):
        a = rng.choice(task.actions)
        keys = list(a.pre) + rng.sample(list(task.facts), min(len(task.facts), rng.randint(0, 2)))
        keys = sorted(set(rng.sample(keys, min(len(keys), rng.randint(1, 3))))) if keys else []
        point = []
        for k in keys:
            s = [v for v in task.facts[k].values if k not in a.pre or v in a.pre[k] or rng.random() < 0.15]
            point.append((k, rng.choice(s or list(task.facts[k].values))))
        r = rng.random()
        if r < 0.05:
            point.append(("nofact:1", 0))
        elif r < 0.08:
            point = []
        out.append(RP.Ban(a.what.label, tuple(point)))
        if rng.random() < 0.2:
            out.append(out[-1])  # the same ban twice: the second finds the point cut already
    return out


def check_one(task, start, bans, work: Path, counts: collections.Counter, fails: list, reduced=True) -> None:
    export(work, task, start)
    py = python_file(task, bans, start)
    tc = taskcore_file(work, bans, start)
    counts["tasks"] += 1
    if py is None and tc == "out of domain":
        # both raise; task.py's writer checks its rules before the start, taskcore's start check comes first
        counts["both raise: task.py on a rule of its writer, taskcore on the start"] += 1
        return
    if py != tc:
        fails.append(("format", "python " + ("a file" if py and len(py) > 20 else str(py)), "taskcore " +
                      ("a file" if tc and len(tc) > 20 else str(tc)), task, start, bans))
        return
    counts["same bytes" if py and len(py) > 20 else f"both: {py}"] += 1
    if py is None or py == "out of domain":
        return
    banned = RP.apply_bans(task, bans)
    pieces = RP.ban_pieces(task, bans)
    counts["actions the bans cut"] += sum(1 for a, ps in zip(task.actions, pieces) if ps != [a])
    if taskcore_file(work, None, start, embedded=True) != taskcore_file(work, None, start):
        fails.append(("start file", task, start))
    if not reduced:
        return
    truth = TC.model_optimum(banned, start)
    counts["solvable" if truth is not None else "unsolvable"] += 1
    for name, args in REDUCED:
        r = run([BIN, "sas", "--task", work / "t.tc", "--start-file", work / "start.txt", "--bans", work / "bans.txt",
                 "--out", work / "o.sas"] + args)
        if r.returncode != 0:
            fails.append((name, "taskcore failed", r.stderr, task, start, bans))
            continue
        got = TC.fd_optimum(work / "o.sas", work)
        if got != truth:
            fails.append((name, f"optimum {got} against the banned task's {truth}", task, start, bans))
            continue
        counts[f"{name}: the banned task's optimum"] += 1
        if truth is None:
            continue
        try:
            res = plan_fd.plan_taskcore(work / "t.tc", task.facts, task.goal, pieces, start, bans, time_limit=60,
                                        workdir=work / "tcplan", args=args)
        except plan_fd.PathChoice as e:
            fails.append((name, f"path choice: {e}", task, start, bans))
            continue
        if res.status != "plan":
            fails.append((name, f"lama-first: {res.status}", task, start, bans))
            continue
        ok, why = run_plan(task, banned, res, start)
        if not ok:
            fails.append((name, f"the plan on the banned task: {why}", [s.name for s in res.steps], task, start, bans))
            continue
        counts[f"{name}: lama-first's plan runs on the unreduced banned task"] += 1
        counts["plan steps through a folded action"] += sum(1 for f in res.folds if f)


def run_many(n: int, seed: int = 1, reduced=True):
    rng = random.Random(seed)
    counts, fails = collections.Counter(), []
    with tempfile.TemporaryDirectory(prefix="replay-taskcore-test-") as tmp:
        for _ in range(n):
            task, start = TC.rand_task(rng)
            task = labelled(task)
            check_one(task, start, rand_bans(rng, task), Path(tmp), counts, fails, reduced)
    return counts, fails


# --- fixed cases ------------------------------------------------------------------------------------------------------

D3 = Domain.of([0, 1, 2])


def fixed_task():
    """A door, a room, a bit; two inputs, one of them in two paths (a fork on the bit), an add and a guess."""
    facts = {"bit:6": BINARY, "room": Domain.of([28, 41]), "var:53": D3, "state:316": BINARY,
             "var:9": Domain.of(range(0, 6), low=1, high=3)}
    acts = [
        T.Action("walk#0", pre={"bit:6": {0}, "room": {28}, "var:53": {1, 2}},
                 effects=(T.Effect("state:316", value=1),), what=what("walk_to(316)")),
        T.Action("walk#1", pre={"bit:6": {1}, "room": {28, 41}, "var:53": {1, 2}},
                 effects=(T.Effect("state:316", value=1), T.Effect("var:9", add=1, when=(("var:9", frozenset(
                     {"below", 1, 2, 3})),))), what=what("walk_to(316)")),
        T.Action("open#2", pre={"state:316": {1}}, effects=(T.Effect("room", value=41),
                                                             T.Effect("var:53", values=frozenset({0, 2}))),
                 what=what("sentence(2,316)", end="end")),
        T.Action("bit#3", pre={"room": {28}}, effects=(T.Effect("bit:6", value=1),), what=what("sentence(1,7)")),
    ]
    return T.Task(facts, acts, {"room": {41}})


START = {"bit:6": 0, "room": 28, "var:53": 1, "state:316": 0, "var:9": 0}
CASES = [  # (what, bans)
    ("no ban", []),
    ("a point the precondition holds", [RP.Ban("walk_to(316)", (("bit:6", 0), ("room", 28)))]),
    ("a key the action's precondition lacks (the room on the bit's action)", [RP.Ban("sentence(1,7)", (("bit:6", 0),))]),
    ("a point the precondition misses", [RP.Ban("walk_to(316)", (("var:53", 0),))]),
    ("two bans on one label", [RP.Ban("walk_to(316)", (("bit:6", 0), ("room", 28))),
                               RP.Ban("walk_to(316)", (("var:53", 2), ("room", 28)))]),
    ("a key the task lacks: not applied", [RP.Ban("walk_to(316)", (("bit:6", 0), ("var:999", 0)))]),
    ("no key: every action of the label goes", [RP.Ban("sentence(2,316)", ())]),
    ("a set of one value: no piece for that key", [RP.Ban("walk_to(316)", (("room", 28), ("var:53", 1)))]),
    ("the only path to the goal banned", [RP.Ban("sentence(2,316)", (("state:316", 1),))]),
    ("a ban on an add and a guess", [RP.Ban("walk_to(316)", (("bit:6", 1), ("var:9", 0)))]),
]


@pytest.mark.parametrize("name,bans", CASES, ids=[c[0] for c in CASES])
def test_bans_byte_for_byte(name, bans, tmp_path):
    task = fixed_task()
    export(tmp_path, task, START)
    py = python_file(task, bans, START)
    assert py is not None and len(py) > 20
    assert taskcore_file(tmp_path, bans, START) == py


def test_the_start_file_is_the_files_own_start(tmp_path):
    task = fixed_task()
    for start in (START, dict(START, room=41, **{"var:9": 7}), dict(START, room=33)):
        export(tmp_path, task, start)
        a, b = taskcore_file(tmp_path, None, start, embedded=True), taskcore_file(tmp_path, None, start)
        assert a == b == python_file(task, [], start)
    assert b == "out of domain"  # room 33: task.py raises OutOfDomain, so does taskcore


def test_the_ban_counts(tmp_path):
    """A ban on a key the action lacks and a set of three values: two pieces; taskcore's stats count them."""
    task = fixed_task()
    export(tmp_path, task, START)
    bans = [RP.Ban("sentence(1,7)", (("bit:6", 0), ("var:53", 1)))]
    r = run([BIN, "sas", "--task", tmp_path / "t.tc", "--start-file", write_start(tmp_path / "s.txt", task.facts, START),
             "--bans", write_bans(tmp_path / "b.txt", bans), "--out", tmp_path / "o.sas", "--stats", tmp_path / "o.json"]
            + RAW)
    assert r.returncode == 0, r.stderr
    st = json.loads((tmp_path / "o.json").read_text())["bans"]
    pieces = RP.ban_pieces(task, bans)
    assert [len(p) for p in pieces] == [1, 1, 1, 2]
    assert [p.pre for p in pieces[3]] == [{"room": {28}, "bit:6": {1}}, {"room": {28}, "bit:6": {0}, "var:53": {0, 2}}]
    assert (st["bans"], st["not_applied"], st["actions_hit"], st["pieces"], st["actions_after"]) == (1, 0, 1, 2, 5)


def test_plans_with_bans_run_on_the_banned_task(tmp_path):
    task = fixed_task()
    export(tmp_path, task, START)
    bans = [RP.Ban("walk_to(316)", (("bit:6", 0), ("room", 28)))]  # the walk at bit 0: set the bit first
    res = plan_fd.plan_taskcore(tmp_path / "t.tc", task.facts, task.goal, RP.ban_pieces(task, bans), START, bans,
                                workdir=tmp_path / "plan")
    assert res.status == "plan" and [s.name for s in res.steps] == ["bit#3", "walk#1", "open#2"]
    assert run_plan(task, RP.apply_bans(task, bans), res, START) == (True, None)
    assert res.predicted_goal and res.operators > 0 and res.write_s > 0
    assert (res.stats["bans"]["actions_hit"], res.stats["bans"]["pieces"]) == (1, 0)  # walk#0 lies inside the point


def test_the_path_choice_is_the_one_path_that_holds():
    task = fixed_task()
    pieces = [[a] for a in task.actions]
    step = {"op": "a0", "name": "fold:walk_to(316)#0+1", "members": [0, 1], "guessed": ()}
    paths, end = plan_fd.predict(task.facts, pieces, [step], START)
    assert [p.name for p in paths] == ["walk#0"] and end["state:316"] == 1
    paths, end = plan_fd.predict(task.facts, pieces, [step], dict(START, **{"bit:6": 1, "var:9": 3}))
    assert [p.name for p in paths] == ["walk#1"] and end["var:9"] == "above"
    with pytest.raises(plan_fd.PathChoice, match="0 of its 2 paths"):
        plan_fd.predict(task.facts, pieces, [step], dict(START, room=41))
    twin = [[a] for a in task.actions] + [[T.Action("walk#4", pre={"room": {28}}, what=task.actions[0].what)]]
    with pytest.raises(plan_fd.PathChoice, match="2 of its 2 paths"):
        plan_fd.predict(task.facts, twin, [dict(step, members=[0, 4])], START)


def test_a_guess_the_plan_did_not_make_is_unknown():
    task = fixed_task()
    pieces = [[a] for a in task.actions]
    st = dict(START, **{"state:316": 1})
    s = {"op": "a1", "name": "sentence(2,316)", "members": [2], "guessed": ()}
    assert plan_fd.predict(task.facts, pieces, [s], st)[1]["var:53"] == plan_fd.UNKNOWN
    s["guessed"] = (("var:53", 2, ()),)
    assert plan_fd.predict(task.facts, pieces, [s], st)[1]["var:53"] == 2


def test_random_tasks_with_bans():
    counts, fails = run_many(60, seed=7)
    assert not fails, fails[:3]
    assert counts["same bytes"] > 20 and counts["actions the bans cut"] > 20
    assert counts["default: lama-first's plan runs on the unreduced banned task"] > 10


# --- the harness, planning through taskcore on a fake engine -------------------------------------------------------------

def test_the_harness_plans_through_taskcore_with_its_bans(tmp_path):
    import test_replay_review as TR
    r = TR.rec("walk_to", "walk_to(316)", {"script:2": frozenset({0}), "room": frozenset({28}),
                                           "bit:6": frozenset({0})},
               {"script:2": ("set", 1), "pos:script:2": ("set", 736), "state:316": ("set", 1)}, (),
               ("sentence", 11, 316, None), {"script:2", "room"})
    other = TR.rec("sentence", "sentence(2,316)", {"room": frozenset({28}), "bit:6": frozenset({1})},
                   {"state:316": ("set", 1)}, (), ("sentence", 2, 316, None), {"room"})
    flip = TR.rec("sentence", "sentence(1,7)", {"room": frozenset({28})}, {"bit:6": ("set", 1)}, (),
                  ("sentence", 1, 7, None), {"room"})
    fb = TR.FakeBranch(lambda fb, f, clicks: None)
    b = types.SimpleNamespace(table=types.SimpleNamespace(points={"at": {"script:2": {736, 937}}, "esc": {}}),
                              counts=collections.Counter())
    facts = {"script:2": BINARY, "pos:script:2": Domain.of({-1, 736, 937}), "room": Domain.of({28, 41}),
             "bit:6": BINARY, "state:316": BINARY}
    acts = [T.Action(f"{x.label}#{i}", pre={k: set(v) for k, v in x.pre.items()},
                     effects=tuple(T.Effect(k, value=e[1]) for k, e in x.effects.items()), what=x)
            for i, x in enumerate((r, other, flip))]
    task = T.Task(facts, acts, {"state:316": {1}})
    export_task(tmp_path / "k.tc", task, {}, meta=lambda a: (a.what.kind, a.what.end, ""), naming=record_naming)
    h = RP.Harness(b, task, task, fb, {"state:316": {1}}, packed=tmp_path / "k.tc", workdir=tmp_path / "plans")
    h.observe()
    steps = h.plan(first=True)
    assert [s.name for s in steps] == ["walk_to(316)#0"] and h.task.actions[0].what is r
    p = h.plans[0]
    assert p["status"] == "plan" and p["planner"] == "taskcore" and p["operators"] > 0 and p["write_s"] > 0
    assert h.pred == {"pos:script:2": -1}
    h.bans.append(RP.ban_of(r, h.belief()))  # bit 6 = 0 in room 28: the walk is banned there
    assert h.bans[0] == RP.Ban("walk_to(316)", (("bit:6", 0), ("room", 28)))
    steps = h.plan(first=False)
    assert [s.name for s in steps] == ["sentence(1,7)#2", "sentence(2,316)#1"] and h.plans[1]["bans"] == 1
    assert [a.name for a in h.task.actions] == [s.name for s in steps]
    h.bans.append(RP.Ban("sentence(2,316)", (("room", 28),)))
    assert h.plan(first=False) == [] and h.stop_reason.startswith("no plan")


# --- the kitchen ----------------------------------------------------------------------------------------------------------

KITCHEN_BANS = [  # (record label#index, the start it is banned from) as ban_of makes them
    "walk_to(316)#478",  # the walk to the door: the first harness version's ban (bit:6 = 0, room 28)
    "sentence(9,566)#12118",  # picking up the meat
    "event(local:211@33)#30568",  # the cook's timer
    "event(script:2@736#1db02ba5a8ab)#34384",  # a step through a folded action
    "sentence(2,564)#11592",
]


def kitchen(work: Path, which=("pruned", "full")) -> None:
    """The kitchen (module docstring): for four ban sets, task.py's file of the banned task (the pruned task replay.py's
    python planner bans, and build_task's, which taskcore's planner reads) against taskcore's with the bans, sha1
    and size; then lama-first through plan_taskcore with each of the six ban sets and its plan run on the unreduced
    banned task. Writes work/kitchen-bans.json (kitchen-bans-<which>.json)."""
    import hashlib
    import pickle
    import time

    import reader as R
    import relevance
    import test_reader as TRD
    from export_records import export_records
    work.mkdir(parents=True, exist_ok=True)
    b, recs, _ = pickle.loads(Path(os.environ["READER_PICKLE"]).read_bytes())
    goal = TRD.goal()
    full = R.build_task(b, recs, goal)
    start = R.task_start(b, TRD.start(), full)
    state = {k: d.label(start.get(k, 0)) for k, d in full.facts.items()}
    by_name = {a.name: a.what for a in full.actions}
    one = [RP.ban_of(by_name[n], state) for n in KITCHEN_BANS]
    sets = {"the walk": one[:1], "the meat": one[1:2], "the timer": one[2:3], "a folded step": one[3:4],
            "all five": one, "the walk twice (rooms 28 and 41)": [one[0], RP.Ban(one[0].label, tuple(
                (k, 41 if k == "room" else v) for k, v in one[0].values))]}
    out = {"bans": {k: [list(map(list, x.values)) for x in v] + [[x.label for x in v]] for k, v in sets.items()},
           "files": [], "plans": []}

    def sha(text):
        return hashlib.sha1(text.encode()).hexdigest()

    files = ("the walk", "a folded step", "all five", "the walk twice (rooms 28 and 41)")  # task.py: 2-6 minutes each
    tasks = {}
    if "full" in which:
        export_records(work / "kitchen.tc", b, recs, goal, starts={"s17": start})
        tasks["full"] = (full, work / "kitchen.tc")
    if "pruned" in which:
        small = R.prune(full, relevance.relevant(full))
        export_task(work / "kitchen-pruned.tc", small, {"s17": start}, meta=lambda a: (a.what.kind, a.what.end, ""),
                    naming=record_naming)
        tasks["pruned"] = (small, work / "kitchen-pruned.tc")
    del recs
    for tname in [w for w in ("pruned", "full") if w in which]:
        task, tc = tasks[tname]
        for name, bans in [(n, sets[n]) for n in files]:
            t0 = time.monotonic()
            py = T.write_sas(RP.apply_bans(task, bans)).text(start)
            t1 = time.monotonic()
            write_start(work / "start.txt", task.facts, start)
            write_bans(work / "bans.txt", bans)
            r = run([BIN, "sas", "--task", tc, "--start-file", work / "start.txt", "--bans", work / "bans.txt",
                     "--out", work / "k.sas", "--stats", work / "k.json"] + RAW)
            t2 = time.monotonic()
            got = (work / "k.sas").read_text() if r.returncode == 0 else r.stderr
            row = {"task": tname, "bans": name, "same": got == py, "bytes": len(py.encode()), "sha1": sha(py)[:12],
                   "taskcore sha1": sha(got)[:12], "python s": round(t1 - t0, 1), "taskcore s": round(t2 - t1, 1),
                   "taskcore bans": json.loads((work / "k.json").read_text()).get("bans") if r.returncode == 0 else None}
            del py, got
            (work / "k.sas").unlink(missing_ok=True)
            out["files"].append(row)
            print(json.dumps(row), flush=True)
            (work / f"kitchen-bans-{'-'.join(which)}.json").write_text(json.dumps(out, indent=1))
    if "full" in which:
        for name, bans in sets.items():
            pieces = RP.ban_pieces(full, bans)
            res = plan_fd.plan_taskcore(work / "kitchen.tc", full.facts, goal, pieces, start, bans,
                                        workdir=work / "plan")
            row = {"bans": name, "status": res.status, "steps": [s.name for s in res.steps],
                   "taskcore s": round(res.write_s, 1), "lama-first s": round(res.seconds, 1),
                   "operators": res.operators, "folded steps": sum(1 for f in res.folds if f)}
            if res.status == "plan":
                row["runs on the unreduced banned task"] = run_plan(full, RP.apply_bans(full, bans), res, start)
            out["plans"].append(row)
            print(json.dumps(row), flush=True)
            (work / f"kitchen-bans-{'-'.join(which)}.json").write_text(json.dumps(out, indent=1))


if __name__ == "__main__":
    if sys.argv[1:2] == ["kitchen"]:
        kitchen(Path(sys.argv[2]), tuple(sys.argv[3:]) or ("pruned", "full"))
        sys.exit(0)
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    counts, fails = run_many(n, seed)
    for k, v in sorted(counts.items()):
        print(f"{v:8}  {k}")
    print(f"failures: {len(fails)}")
    for f in fails[:10]:
        print("  ", str(f)[:3000])
    sys.exit(1 if fails else 0)
