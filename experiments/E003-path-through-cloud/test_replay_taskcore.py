"""Tests of the replay harness's taskcore planner: taskcore's --start-file, --bans and --ops, plan_fd.plan_taskcore,
and replay.Harness planning through it.

usage: python test_replay_taskcore.py [N] [seed]    (N random tasks, default 200; the summary on stdout)
       pytest test_replay_taskcore.py -m 'not slow'  (the fixed cases, and the same with N = 60)
       READER_PICKLE=<reading.pkl> python test_replay_taskcore.py kitchen <work dir> [pruned|full|plans ...]
                                                     (the kitchen: the bans byte for byte, and plans with bans)
       python test_replay_taskcore.py log <work dir>  (results/taskcore-bans.log from the work dir: the kitchen's
                                                     json files and the random runs' summaries, prop-N-sS.out)
       TASKCORE_BIN=<binary>                         (default taskcore/target/release/taskcore)

1. **The bans, byte for byte.** replay.py's ban (``ban_of``'s point, ``apply_bans``/``cut``) applied in Python and
   written by task.py's ``write_sas``, against taskcore given the unbanned task and the bans (``--bans``) with no
   fold, no reduction and task.py's encoding: the same file, or both raise. The start goes as a file
   (``--start-file``, export_records.write_start) and must give the same bytes as the file's own start.
2. **With the fold and the reductions** (the default, fold every, PRODUCT_LIMIT 1): the plan lama-first finds on
   taskcore's file, read back by ``plan_fd.plan_taskcore``, runs on the unreduced banned task (Python, task.py's
   meaning, the plan's guessed values; a guess the plan did not make, on a fact the file does not hold, may be any
   of its values, and the plan must work for each; a start value outside a domain is in no set but the whole
   domain): each step's path is an action of ``apply_bans(task, bans)``, its precondition holds for every value the
   plan leaves possible, and so does the goal at the end. And Fast Downward's optimal cost (astar(blind())) on
   taskcore's banned file is the banned task's own (test_taskcore.model_optimum).
3. **A start outside a domain** (the random tasks where task.py raises OutOfDomain): with ``--start-check needed``
   (the harness's) taskcore stops, on a fact (e) needs, or its file's optimum is the banned task's own from that
   start (model_optimum: such a value is in no set but the whole domain) and the plan runs as in 2.
4. **The path choice**: a folded step is the one path that holds, or may hold (an unknown value on a fact the file
   does not hold), in the predicted state; none or two raise.
5. **The harness** plans through taskcore, and through the python planner, on a hand-made task, with a ban, on a
   fake engine; a taskcore error stops it with the reason.
6. **Two writes of one fact** (an action task.py writes only by leaving out the write of the value the fact has):
   reduction (d) and the fold keep the set that tells it, so the reductions do not make the writer raise.
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
    export_task(work / "t.tc", task, {"s": start}, meta=meta, naming=by_label, every_fact=True)
    _FACTS[work] = task.facts


# --- the plan on the unreduced banned task (independent of plan_fd.predict) -----------------------------------------

OUT = "<outside the domain>"


def run_plan(task, banned, res, start) -> tuple:
    """Run ``res``'s paths on the banned task (module docstring, 2): (ok, why). Each fact holds the set of values the
    plan leaves possible: one, or several after a guess the plan did not make or an effect whose conditions hold for
    some of them; a set holds when it holds each (task.py: or when it is the whole domain). Two writes of one fact:
    a write of the one value the fact has is left out (task.py), the others must agree."""
    def ident(a):  # an action of the banned task: its name (a piece's names the ban's key) and precondition
        return a.name, tuple(sorted((k, tuple(sorted(map(str, v)))) for k, v in a.pre.items())), a.effects
    acts = {ident(a) for a in banned.actions}
    dom = {k: d.values for k, d in task.facts.items()}

    def lab(k, v):
        x = task.facts[k].label(v) if isinstance(v, int) else v
        return x if x in dom[k] else OUT

    def meets(k, s):  # for every possible value
        return state[k] <= set(s) or set(dom[k]) <= set(s)

    def some(k, s):  # for some possible value
        return bool(state[k] & set(s)) or set(dom[k]) <= set(s)
    state = {k: frozenset({lab(k, start.get(k, 0))}) for k in task.facts}
    for i, (a, st) in enumerate(zip(res.paths, res.steps)):
        if ident(a) not in acts:
            return False, f"step {i + 1}: {a.name} is not an action of the banned task"
        if not all(meets(k, s) for k, s in a.pre.items()):
            return False, f"step {i + 1}: {a.name}'s precondition does not hold"
        writes = collections.defaultdict(list)
        for e in a.effects:
            if not all(some(k, s) for k, s in e.when):
                continue
            d = task.facts[e.key]
            if e.values is None and e.add is None:
                vals = {lab(e.key, e.value)}
            elif e.values is not None:
                vals = {lab(e.key, v) for v in e.values}
            else:  # task.py's add: one write per value of the domain, none outside it
                vals = set().union(*(d.add(u, e.add) if u != OUT else {OUT} for u in state[e.key]))
            if len(vals) > 1:
                got = {v for k, v, w in st.guessed if k == e.key and v in vals and all(state[c] <= s for c, s in w)}
                vals = got if len(got) == 1 else vals
            writes[e.key].append((frozenset(vals), all(meets(k, s) for k, s in e.when)))
        new = dict(state)
        for k, ws in writes.items():
            if len(ws) > 1 and len(state[k]) == 1:
                ws = [w for w in ws if w[0] != state[k]] or ws[:1]
            if len({w[0] for w in ws}) > 1:
                return False, f"step {i + 1}: {a.name} writes {k} twice, differently"
            new[k] = ws[0][0] if all(w[1] for w in ws) else state[k] | ws[0][0]
        state = new
    goal = all(state[k] <= {task.facts[k].label(v) if isinstance(v, int) else v for v in s}
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
    if py == "out of domain" and reduced:
        check_outside(task, start, bans, work, counts, fails)
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
        # the harness's start check changes nothing where the input check passes
        r = run([BIN, "sas", "--task", work / "t.tc", "--start-file", work / "start.txt", "--bans", work / "bans.txt",
                 "--out", work / "n.sas", "--start-check", "needed"] + args)
        if r.returncode != 0 or (work / "n.sas").read_bytes() != (work / "o.sas").read_bytes():
            fails.append((name, "--start-check needed: another file", r.stderr, task, start, bans))
            continue
        counts[f"{name}: --start-check needed, the same file"] += 1
        if truth is None:
            continue
        try:
            res = plan_fd.plan_taskcore(work / "t.tc", task.facts, task.goal, pieces, start, bans, time_limit=60,
                                        workdir=work / "tcplan", args=args)
        except (plan_fd.PathChoice, plan_fd.TaskcoreError) as e:
            fails.append((name, f"{type(e).__name__}: {e}", task, start, bans))
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


def check_outside(task, start, bans, work: Path, counts: collections.Counter, fails: list) -> None:
    """A start outside a domain, where task.py raises OutOfDomain (module docstring, 3)."""
    banned = RP.apply_bans(task, bans)
    try:
        truth = TC.model_optimum(banned, start)
    except TC.Raises:
        counts["outside: the state search finds a rule task.py's writer checks broken (skipped)"] += 1
        return
    counts["outside: tasks"] += 1
    pieces = RP.ban_pieces(task, bans)
    for name, args in REDUCED:
        try:
            res = plan_fd.plan_taskcore(work / "t.tc", task.facts, task.goal, pieces, start, bans, time_limit=60,
                                        workdir=work / "tcout", args=args, keep=True)
        except (plan_fd.PathChoice, plan_fd.TaskcoreError) as e:
            fails.append((name, f"outside: {type(e).__name__}: {e}", task, start, bans))
            continue
        if res.status == "out of domain":
            counts[f"outside, {name}: taskcore stops on a fact (e) needs"] += 1
            continue
        got = TC.fd_optimum(work / "tcout" / "task.sas", work)
        if got != truth:
            fails.append((name, f"outside: optimum {got} against the banned task's {truth}", task, start, bans))
            continue
        counts[f"outside, {name}: taskcore plans on, the banned task's optimum"] += 1
        if truth is None:
            continue
        if res.status != "plan":
            fails.append((name, f"outside: lama-first: {res.status}", task, start, bans))
            continue
        ok, why = run_plan(task, banned, res, start)
        if not ok:
            fails.append((name, f"outside: the plan on the banned task: {why}", [s.name for s in res.steps], task,
                          start, bans))
            continue
        counts[f"outside, {name}: lama-first's plan runs on the unreduced banned task"] += 1


def run_many(n: int, seed: int = 1, reduced=True):
    rng = random.Random(seed)
    counts, fails = collections.Counter(), []
    with tempfile.TemporaryDirectory(prefix="replay-taskcore-test-") as tmp:
        for _ in range(n):
            task, start = TC.rand_task(rng)
            task = labelled(task)
            check_one(task, start, rand_bans(rng, task), Path(tmp), counts, fails, reduced)
    return counts, fails


FX, FY = "fx:z", "fy:z"  # rand_outside's facts: fx:z starts outside its domain; fy:z only its inputs write


def rand_outside(rng):
    """test_taskcore's random task with two more facts: fx:z, which starts outside its domain (9), and fy:z, which
    nothing reads; one to three inputs that read fx:z, in a precondition (now and then two paths of one input that
    fork on it, for the fold) or in a condition, and write fy:z or the task's own facts; now and then an action that
    writes fx:z, so that it can come into its domain. Module docstring, 3: here fx:z is often a fact (e) does not
    need."""
    task, start = TC.rand_task(rng)
    dom = BINARY if rng.random() < 0.5 else D3
    facts = dict(task.facts, **{FX: dom, FY: BINARY})
    keys, acts = list(task.facts), list(task.actions)
    for j in range(rng.randint(1, 3)):
        pre = {k: TC.subset(rng, facts[k].values) for k in rng.sample(keys, rng.randint(0, 1))}
        eff = TC.rand_effects(rng, facts, [FY] if rng.random() < 0.6 else keys, pre, rng.randint(1, 2))
        r = rng.random()
        if r < 0.3:  # two paths of one input, forking on fx:z
            vals = list(dom.values)
            rng.shuffle(vals)
            cut = rng.randint(1, len(vals) - 1)
            for part in (set(vals[:cut]), set(vals[cut:])):
                own = eff if rng.random() < 0.5 else TC.rand_effects(rng, facts, [FY] + keys, dict(pre), 1)
                acts.append(T.Action(f"fx{j}", pre=dict(pre, **{FX: part}), effects=own, cost=1,
                                     what=("event", "yield", f"cfx{j}")))
        elif r < 0.65:
            acts.append(T.Action(f"fx{j}", pre=dict(pre, **{FX: TC.subset(rng, dom.values)}), effects=eff, cost=1,
                                 what=("sentence", "end", "")))
        else:  # a condition on fx:z
            e = eff[0]
            e = T.Effect(e.key, value=e.value, add=e.add, values=e.values,
                         when=tuple(e.when) + ((FX, frozenset(TC.subset(rng, dom.values))),))
            acts.append(T.Action(f"fx{j}", pre=pre, effects=(e,) + tuple(eff[1:]), cost=1,
                                 what=("sentence", "end", "")))
    if rng.random() < 0.5:
        pre = {k: TC.subset(rng, facts[k].values) for k in rng.sample(keys, 1)}
        acts.append(T.Action("setfx", pre=pre, effects=(T.Effect(FX, value=rng.choice(dom.values)),), cost=1,
                             what=("sentence", "end", "")))
    return T.Task(facts, acts, task.goal), dict(start, **{FX: 9, FY: 0})


def run_outside(n: int, seed: int = 1):
    """Module docstring, 3, on rand_outside's tasks, half of them with random bans."""
    rng = random.Random(seed)
    counts, fails = collections.Counter(), []
    with tempfile.TemporaryDirectory(prefix="replay-taskcore-outside-") as tmp:
        work = Path(tmp)
        for _ in range(n):
            task, start = rand_outside(rng)
            task = labelled(task)
            bans = rand_bans(rng, task) if rng.random() < 0.5 else []
            export(work, task, start)
            py = python_file(task, bans, start)
            counts[f"task.py: {'a file' if py and len(py) > 20 else py}"] += 1
            if py is not None:
                check_outside(task, start, bans, work, counts, fails)
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


def test_a_ban_on_a_fact_no_action_names(tmp_path):
    """replay.py applies a ban whose keys are the task's facts, also one no action names: with every fact in the file
    (export_task's every_fact) taskcore does too; without it, the start file names the fact and taskcore stops."""
    task = fixed_task()
    task.facts["var:7"] = BINARY  # no action names it
    bans = [RP.Ban("sentence(1,7)", (("var:7", 0),))]
    export(tmp_path, task, START)
    assert taskcore_file(tmp_path, bans, START) == python_file(task, bans, START) != python_file(task, [], START)
    export_task(tmp_path / "t.tc", task, {"s": START}, meta=meta, naming=by_label)  # var:7 not in the file
    r = run([BIN, "sas", "--task", tmp_path / "t.tc", "--start-file", write_start(tmp_path / "s.txt", task.facts, START),
             "--bans", write_bans(tmp_path / "b.txt", bans), "--out", tmp_path / "o.sas"])
    assert r.returncode != 0 and "the file's facts are not the task's" in r.stderr


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


def test_a_set_of_the_whole_domain_holds_for_an_unknown_value():
    """task.py leaves out a set that holds for every value, so it holds where the plan left the value unknown."""
    task = fixed_task()
    whole = T.Action("any#4", pre={"bit:6": {0, 1}, "room": {28}}, effects=(T.Effect("room", value=41),),
                     what=what("sentence(1,8)"))
    pieces = [[a] for a in task.actions] + [[whole]]
    st = dict(plan_fd.start_state(task.facts, START), **{"bit:6": plan_fd.UNKNOWN})
    assert plan_fd._holds(task.facts, whole.pre, st) and not plan_fd._holds(task.facts, task.actions[0].pre, st)
    s = {"op": "a1", "name": "sentence(1,8)", "members": [4], "guessed": ()}
    assert [p.name for p in plan_fd.predict(task.facts, pieces, [s], dict(START, **{"bit:6": 7}))[0]] == ["any#4"]


def test_a_guess_the_plan_did_not_make_is_unknown():
    task = fixed_task()
    pieces = [[a] for a in task.actions]
    st = dict(START, **{"state:316": 1})
    s = {"op": "a1", "name": "sentence(2,316)", "members": [2], "guessed": ()}
    assert plan_fd.predict(task.facts, pieces, [s], st)[1]["var:53"] == plan_fd.UNKNOWN
    s["guessed"] = (("var:53", 2, ()),)
    assert plan_fd.predict(task.facts, pieces, [s], st)[1]["var:53"] == 2


def test_an_unknown_value_may_hold_only_on_a_fact_the_file_does_not_hold():
    """A guess the plan did not make leaves g unknown. A path that reads g may hold where the file does not hold g,
    and fails where it does; two paths told apart only by g both may hold, and raise; a start value outside g's
    domain is in no set of g."""
    facts = {"g": D3, "b": BINARY}
    acts = [T.Action("guess#0", effects=(T.Effect("g", values=frozenset({1, 2})),), what=what("guess")),
            T.Action("walk#1", pre={"g": {1, 2}, "b": {0}}, effects=(T.Effect("b", value=1),), what=what("walk")),
            T.Action("walk#2", pre={"g": {1, 2}, "b": {1}}, effects=(T.Effect("b", value=0),), what=what("walk")),
            T.Action("walk#3", pre={"g": {0}, "b": {0}}, effects=(T.Effect("b", value=1),), what=what("walk"))]
    pieces = [[a] for a in acts]
    guess = {"op": "a0", "name": "guess", "members": [0], "guessed": ()}
    walk = {"op": "a1", "name": "fold:walk#1+1", "members": [1, 2], "guessed": ()}
    start = {"g": 0, "b": 0}
    paths, end = plan_fd.predict(facts, pieces, [guess, walk], start, held={"b"})
    assert [p.name for p in paths] == ["guess#0", "walk#1"] and end == {"g": plan_fd.UNKNOWN, "b": 1}
    with pytest.raises(plan_fd.PathChoice, match="0 of its 2 paths hold in the plan's predicted state$"):
        plan_fd.predict(facts, pieces, [guess, walk], start, held={"b", "g"})
    with pytest.raises(plan_fd.PathChoice, match="0 of its 2 paths hold .* and 2 may hold"):
        plan_fd.predict(facts, pieces, [guess, dict(walk, members=[1, 3])], start, held={"b"})
    with pytest.raises(plan_fd.PathChoice, match="0 of its 1 paths hold"):
        plan_fd.predict(facts, pieces, [dict(walk, members=[1], name="walk")], dict(start, g=7), held={"b"})
    assert plan_fd.start_state(facts, dict(start, g=7))["g"] == plan_fd.OUTSIDE


def test_a_guess_on_a_fact_the_file_drops_may_hold(tmp_path):
    """The review's case: action 1 guesses g in {0, 1} (domain {0, 1, 2}) and sets x; action 2 needs g in {0, 1} and
    x. From g = 0, (d) cuts g's domain to {0, 1}: action 2's set on g holds for every value, and the file drops g.
    The plan leaves g unknown, action 2's path may hold, and the plan runs for every value of the guess."""
    facts = {"g": D3, "x": BINARY, "done": BINARY, "room": Domain.of([28, 41])}
    acts = [T.Action("guess#0", pre={"room": {28}}, effects=(T.Effect("g", values=frozenset({0, 1})),
                                                             T.Effect("x", value=1)), what=what("guess")),
            T.Action("use#1", pre={"g": {0, 1}, "x": {1}}, effects=(T.Effect("done", value=1),), what=what("use"))]
    task = T.Task(facts, acts, {"done": {1}})
    start = {"g": 0, "x": 0, "done": 0, "room": 28}
    export(tmp_path, task, start)
    res = plan_fd.plan_taskcore(tmp_path / "t.tc", task.facts, task.goal, RP.ban_pieces(task, []), start, [],
                                workdir=tmp_path / "p")
    assert res.status == "plan" and [s.name for s in res.steps] == ["guess#0", "use#1"]
    assert "g" not in res.file_facts and "g" in res.needed_facts
    assert run_plan(task, task, res, start) == (True, None)


def test_a_start_outside_a_domain_stops_only_on_a_fact_the_goal_needs(tmp_path):
    """var:7 (binary) starts at 5, and only an action the goal does not need reads it: the input check stops (task.py
    raises OutOfDomain on the task), the harness's (--start-check needed) plans on, and the plan is the one from an
    in-domain start. The room outside its domain stops both."""
    task = fixed_task()
    task.facts["var:7"] = BINARY
    task.actions.append(T.Action("look#4", pre={"var:7": {1}}, effects=(T.Effect("var:9", value=2),),
                                 what=what("sentence(8,7)")))
    start = dict(START, **{"var:7": 5})
    export(tmp_path, task, start)
    assert python_file(task, [], start) == "out of domain"
    pieces = RP.ban_pieces(task, [])

    def plan(st, check, d):
        return plan_fd.plan_taskcore(tmp_path / "t.tc", task.facts, task.goal, pieces, st, [], workdir=tmp_path / d,
                                     start_check=check)
    res = plan(start, "input", "a")
    assert res.status == "out of domain" and res.misses == [("var:7", 5)]
    res, inside = plan(start, "needed", "b"), plan(dict(start, **{"var:7": 0}), "input", "c")
    assert res.status == inside.status == "plan" and [s.name for s in res.steps] == [s.name for s in inside.steps]
    assert res.stats["start_check"]["facts_the_input_check_stops_on"] == 1
    assert run_plan(task, task, res, start) == (True, None)
    res = plan(dict(start, room=33), "needed", "d")
    assert res.status == "out of domain" and [k for k, _ in res.misses] == ["room"]


def _sas(work: Path, task, start, args):
    """taskcore's file of ``task`` from ``start``: (Fast Downward's optimum, the stats), or (None, the error)."""
    r = run([BIN, "sas", "--task", work / "t.tc", "--start-file", write_start(work / "s.txt", task.facts, start),
             "--out", work / "o.sas", "--stats", work / "o.json"] + list(args))
    if r.returncode != 0:
        return None, r.stderr
    return TC.fd_optimum(work / "o.sas", work), json.loads((work / "o.json").read_text())


TWICE = "(d) facts an action writes twice, kept whole (a set on them would hold for every value)"


def test_two_writes_of_one_fact_keep_the_set_task_py_reads(tmp_path):
    """flip: c in {0, 1} (domain {0, 1, 2}) writes c := 0 and c := 1; task.py's writer leaves out the write of the
    value c has (the variant's precondition value), so flip turns 0 into 1 and 1 into 0. From c = 0, (d) reaches 0
    and 1 only: cutting c's domain to them would make the set hold for every value and the writer raise (the
    review's random task with seed 41). (d) keeps c's domain whole."""
    facts = {"c": D3, "done": BINARY}
    acts = [T.Action("flip#0", pre={"c": {0, 1}}, effects=(T.Effect("c", value=0), T.Effect("c", value=1)),
                     what=what("flip")),
            T.Action("end#1", pre={"c": {1}}, effects=(T.Effect("done", value=1),), what=what("end"))]
    task = T.Task(facts, acts, {"done": {1}})
    start = {"c": 0, "done": 0}
    export(tmp_path, task, start)
    assert python_file(task, [], start) is not None and TC.model_optimum(task, start) == 2
    for args in ([], ["--fold", "off", "--reduce", "d"], ["--fold", "every"]):
        got, st = _sas(tmp_path, task, start, args)
        assert got == 2 and st["reduce_counts"][TWICE] >= 1, (args, st)
    res = plan_fd.plan_taskcore(tmp_path / "t.tc", task.facts, task.goal, RP.ban_pieces(task, []), start, [],
                                workdir=tmp_path / "p")
    assert [s.name for s in res.steps] == ["flip#0", "end#1"] and run_plan(task, task, res, start) == (True, None)


def test_a_fold_that_would_drop_the_set_two_writes_need_is_left_out(tmp_path):
    """Two paths of one input, c = 0 and c = 1 (c binary), each writing c := 0 and c := 1. Folded, the union is c's
    whole domain and is left out, the writes every path has stay unconditional, and the writer would raise:
    taskcore leaves the sub-group unfolded, counted."""
    facts = {"c": BINARY, "done": BINARY}
    flips = (T.Effect("c", value=0), T.Effect("c", value=1))
    acts = [T.Action("flip#0", pre={"c": {0}}, effects=flips, what=what("flip")),
            T.Action("flip#1", pre={"c": {1}}, effects=flips, what=what("flip")),
            T.Action("end#2", pre={"c": {1}}, effects=(T.Effect("done", value=1),), what=what("end"))]
    task = T.Task(facts, acts, {"done": {1}})
    start = {"c": 0, "done": 0}
    export(tmp_path, task, start)
    assert python_file(task, [], start) is not None and TC.model_optimum(task, start) == 2
    for args in (["--reduce", "none"], []):
        got, st = _sas(tmp_path, task, start, args)
        assert got == 2 and st["fold_counts"]["subgroups_writing_a_fact_twice_left_unfolded"] == 1, (args, st)


def test_random_starts_outside_a_domain():
    counts, fails = run_outside(60, seed=5)
    assert not fails, fails[:3]
    assert counts["outside, default: lama-first's plan runs on the unreduced banned task"] > 5
    assert counts["outside, default: taskcore stops on a fact (e) needs"] > 5


def test_random_tasks_with_bans():
    counts, fails = run_many(60, seed=7)
    assert not fails, fails[:3]
    assert counts["same bytes"] > 20 and counts["actions the bans cut"] > 20
    assert counts["default: lama-first's plan runs on the unreduced banned task"] > 10


# --- the harness, planning through taskcore on a fake engine -------------------------------------------------------------

def _harness_task():
    """A walk to the door (banned below), another way to open it once bit 6 is set, and the bit's setter; the bridge's
    slot table for the walk's slot. Returns (builder, records, task)."""
    import test_replay_review as TR
    r = TR.rec("walk_to", "walk_to(316)", {"script:2": frozenset({0}), "room": frozenset({28}),
                                           "bit:6": frozenset({0})},
               {"script:2": ("set", 1), "pos:script:2": ("set", 736), "state:316": ("set", 1)}, (),
               ("sentence", 11, 316, None), {"script:2", "room"})
    other = TR.rec("sentence", "sentence(2,316)", {"room": frozenset({28}), "bit:6": frozenset({1})},
                   {"state:316": ("set", 1)}, (), ("sentence", 2, 316, None), {"room"})
    flip = TR.rec("sentence", "sentence(1,7)", {"room": frozenset({28})}, {"bit:6": ("set", 1)}, (),
                  ("sentence", 1, 7, None), {"room"})
    b = types.SimpleNamespace(table=types.SimpleNamespace(points={"at": {"script:2": {736, 937}}, "esc": {}}),
                              counts=collections.Counter())
    facts = {"script:2": BINARY, "pos:script:2": Domain.of({-1, 736, 937}), "room": Domain.of({28, 41}),
             "bit:6": BINARY, "state:316": BINARY}
    acts = [T.Action(f"{x.label}#{i}", pre={k: set(v) for k, v in x.pre.items()},
                     effects=tuple(T.Effect(k, value=e[1]) for k, e in x.effects.items()), what=x)
            for i, x in enumerate((r, other, flip))]
    return b, (r, other, flip), T.Task(facts, acts, {"state:316": {1}})


def _fake_branch():
    import test_replay_review as TR
    return TR.FakeBranch(lambda fb, f, clicks: None)


def test_the_harness_plans_through_taskcore_with_its_bans(tmp_path):
    b, (r, other, flip), task = _harness_task()
    export_task(tmp_path / "k.tc", task, {}, meta=lambda a: (a.what.kind, a.what.end, ""), naming=record_naming)
    h = RP.Harness(b, task, task, _fake_branch(), {"state:316": {1}}, packed=tmp_path / "k.tc",
                   workdir=tmp_path / "plans")
    h.observe()
    steps = h.plan(first=True)
    assert [s.name for s in steps] == ["walk_to(316)#0"] and h.task.actions[0].what is r
    p = h.plans[0]
    assert p["status"] == "plan" and p["planner"] == "taskcore" and p["operators"] > 0 and p["write_s"] > 0
    assert h.pred == {"pos:script:2": -1}
    # the step's effects: on the facts (e) needs, as the pruned task's on relevance's facts (no pos:script:2)
    import reader as R
    import relevance
    small = R.prune(task, relevance.relevant(task))
    assert [e.key for e in h.task.actions[0].effects] == [e.key for e in small.actions[0].effects] == \
        ["script:2", "state:316"]
    h.bans.append(RP.ban_of(r, h.belief()))  # bit 6 = 0 in room 28: the walk is banned there
    assert h.bans[0] == RP.Ban("walk_to(316)", (("bit:6", 0), ("room", 28)))
    steps = h.plan(first=False)
    assert [s.name for s in steps] == ["sentence(1,7)#2", "sentence(2,316)#1"] and h.plans[1]["bans"] == 1
    assert [a.name for a in h.task.actions] == [s.name for s in steps]
    h.bans.append(RP.Ban("sentence(2,316)", (("room", 28),)))
    assert h.plan(first=False) == [] and h.stop_reason.startswith("no plan")


def test_the_harness_plans_alike_through_the_python_planner():
    """The same task and bans through task.py and plan_fd.plan (``planner="python"``, the pruned task): the same
    steps, the same effects for waits_first, the same stop."""
    import reader as R
    import relevance
    b, (r, other, flip), task = _harness_task()
    small = R.prune(task, relevance.relevant(task))
    h = RP.Harness(b, task, small, _fake_branch(), {"state:316": {1}}, planner="python")
    h.observe()
    assert [s.name for s in h.plan(first=True)] == ["walk_to(316)#0"]
    assert h.plans[0]["status"] == "plan" and "planner" not in h.plans[0] and h.plans[0]["operators"] > 0
    assert [e.key for e in h.task.actions[0].effects] == ["script:2", "state:316"]
    assert h.pred == {}  # the pruned task has no pos:script:2 (taskcore's harness predicts every slot of build_task's)
    h.bans.append(RP.ban_of(r, h.belief()))
    assert [s.name for s in h.plan(first=False)] == ["sentence(1,7)#2", "sentence(2,316)#1"]
    h.bans.append(RP.Ban("sentence(2,316)", (("room", 28),)))
    assert h.plan(first=False) == [] and h.stop_reason.startswith("no plan")


def test_a_taskcore_error_stops_the_harness_with_its_reason(tmp_path):
    b, _, task = _harness_task()
    h = RP.Harness(b, task, task, _fake_branch(), {"state:316": {1}}, packed=tmp_path / "missing.tc",
                   workdir=tmp_path / "plans")
    h.observe()
    assert h.plan(first=True) == [] and h.plans[0]["status"] == "taskcore error"
    assert h.stop_reason.startswith("planner: ") and "missing.tc" in h.stop_reason


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
    banned task ("plans": only those). Writes work/kitchen-bans-<which>.json."""
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
    if "full" in which or "plans" in which:
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
    if "full" in which or "plans" in which:
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


def write_log(work: Path) -> None:
    """results/taskcore-bans.log from the work directory."""
    import hashlib
    L = ["E003 step 5 with taskcore: the harness's bans in taskcore (--bans) against replay.py's own, and plans with bans",
         "", "sources (sha1, 12 hex): " + ", ".join(
             f"{n} {hashlib.sha1((HERE / n).read_bytes()).hexdigest()[:12]}" for n in
             ("replay.py", "plan_fd.py", "export_records.py", "task.py", "taskcore/src/bans.rs", "taskcore/src/main.rs",
              "taskcore/src/sas.rs", "test_replay_taskcore.py")),
         "byte for byte: replay.apply_bans (ban_of's points, cut) + task.write_sas(...).text(start) against taskcore "
         "--bans --start-file --fold off --reduce none --encoding task on the unbanned packed file."]
    rows, plans, bans = [], [], {}
    only = work / "kitchen-bans-plans.json"  # a later run of the plans alone replaces the others' plans
    for f in sorted(work.glob("kitchen-bans-*.json")):
        d = json.loads(f.read_text())
        rows += d["files"]
        if not only.exists() or f == only:
            plans += d["plans"]
        bans.update(d["bans"])
    if bans:
        L += ["", "== the kitchen (the step 5 reading; start s17 = reader.task_start of run B's S1#7 pre) ==",
              "ban sets (ban_of of these records from s17: label, then the banned point):"]
        for name, v in bans.items():
            labels = v[-1]
            L.append(f"  {name}: " + "; ".join(f"{lab} at {dict(map(tuple, pt))}" for lab, pt in zip(labels, v[:-1])))
    if rows:
        L.append("files (task: pruned = test_reader's pruned task, replay.py's python planner; full = build_task's, the "
                 "packed file taskcore's planner reads):")
        for r in rows:
            b = r.get("taskcore bans") or {}
            L.append(f"  {r['task']:6} {r['bans']:34} {'the same bytes' if r['same'] else 'DIFFERENT'} "
                     f"({r['bytes']:,} bytes, sha1 {r['sha1']}; taskcore {r['taskcore sha1']}); task.py "
                     f"{r['python s']:.0f} s, taskcore {r['taskcore s']:.0f} s; actions cut {b.get('actions_hit')}, "
                     f"pieces {b.get('pieces')}, actions after {b.get('actions_after')}")
    if plans:
        L.append("plans: plan_fd.plan_taskcore from s17 with the bans (fold strict, (a)-(f), lama-first), each step "
                 "the path that holds (or may hold) in the predicted state, run on the unreduced banned task "
                 "(build_task's, replay.apply_bans; a guess the plan did not make may be any of its values):")
        for r in plans:
            run_ok = r.get("runs on the unreduced banned task")
            L.append(f"  {r['bans']:34} {r['status']}, {len(r['steps'])} steps ({r['folded steps']} through a folded "
                     f"action), taskcore {r['taskcore s']:.0f} s, {r['operators']:,} operators, lama-first "
                     f"{r['lama-first s']:.0f} s" + (f"; runs on the banned task: {run_ok[0]}" +
                                                       (f" ({run_ok[1]})" if run_ok[1] else "") if run_ok else ""))
            if r["steps"]:
                L.append(f"    {r['steps']}")
        L += ["notes: the second ban of \"the walk twice\" (room 41) misses every walk's precondition (they test room 28), "
              "so its file is the first's. \"time\": with the walk banned at bit:6 = 0 in room 28, lama-first finds no "
              "plan in its 600 s; the python planner found none with that ban either (notes/log/2026-10-25-e003-step5-"
              "replay.md, what the review changed)."]
    if (work / "notes.txt").exists():  # what the work directory's files are from, when they are from several runs
        L += [""] + (work / "notes.txt").read_text().rstrip().splitlines()
    for prop in sorted(work.glob("prop-*.out")):
        n, seed = prop.stem.split("-")[1], prop.stem.split("-s")[-1]
        L += ["", f"== random tasks with random bans: python test_replay_taskcore.py {n} {seed} ==",
              "  (test_taskcore.py's random tasks; 1-3 bans each, as ban_of makes them, on a point an action holds or "
              "misses, now and then a key the task lacks, no key, or the same ban twice)",
              "  " + prop.read_text().rstrip().replace("\n", "\n  ")]
    for prop in sorted(work.glob("outside-*.txt")):
        n, seed = prop.stem.split("-")[1], prop.stem.split("-s")[-1]
        L += ["", f"== starts outside a domain: python test_replay_taskcore.py outside {n} {seed} ==",
              "  (rand_outside: test_taskcore.py's random tasks with fx:z, which starts outside its domain and which one "
              "to three inputs read, half of them with random bans; the harness's --start-check needed)",
              "  " + prop.read_text().rstrip().replace("\n", "\n  ")]
    out = HERE / "results" / "taskcore-bans.log"
    out.write_text("\n".join(L) + "\n")
    print("\n".join(L))


if __name__ == "__main__":
    if sys.argv[1:2] == ["log"]:
        write_log(Path(sys.argv[2]))
        sys.exit(0)
    if sys.argv[1:2] == ["kitchen"]:
        kitchen(Path(sys.argv[2]), tuple(sys.argv[3:]) or ("pruned", "full"))
        sys.exit(0)
    outside = sys.argv[1:2] == ["outside"]
    args = sys.argv[2:] if outside else sys.argv[1:]
    n = int(args[0]) if args else 200
    seed = int(args[1]) if len(args) > 1 else 1
    counts, fails = run_outside(n, seed) if outside else run_many(n, seed)
    for k, v in sorted(counts.items()):
        print(f"{v:8}  {k}")
    print(f"failures: {len(fails)}")
    for f in fails[:10]:
        print("  ", str(f)[:3000])
    sys.exit(1 if fails else 0)
