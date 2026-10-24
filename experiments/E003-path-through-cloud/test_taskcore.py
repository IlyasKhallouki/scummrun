"""Tests of taskcore (the Rust crate) and export_records.py on small random tasks and on a few fixed ones.

usage: python test_taskcore.py [N] [seed]    (N random tasks, default 300; the summary on stdout)
       pytest test_taskcore.py               (the fixed cases, and the same with N = 120)
       TASKCORE_BIN=<binary>                 (default taskcore/target/release/taskcore: cargo build --release there)

For each random task (task.py's model: finite domains, collapsed ones with BELOW and ABOVE among them, preconditions,
set, guessed and add effects, conditions, goal sets, costs 1 to 3 or all 1, starts with a value outside a domain
now and then, and inputs that fork into paths for the fold):

1. the format: task.write_sas's file and taskcore's with no fold, no reduction and task.py's encoding
   (``--encoding task``) are the same bytes, or both raise;
2. a start outside a domain: where task.py raises OutOfDomain, taskcore raises in every configuration below (the
   fold and the reductions must not hide it);
3. the optimum: the task's own optimal cost (the sum of the plan's actions' costs, every goal set met at the end),
   found by a search over the task's states here (``model_optimum``, which reads no SAS file), against Fast
   Downward's optimal cost (astar(blind()), its own "Plan cost") on taskcore's file in the model encoding (the
   default): with no lever, with each reduction alone, the folds, PRODUCT_LIMIT 1, and all together. Goal sets
   and unit costs included. Fast Downward's optimum on task.py's own file is counted against it too: it differs
   where the file has pick or goal operators and metric 0 (Fast Downward then counts them as cost 1), or a goal set
   (task.py's goal variable is a latch); so is taskcore's file in task.py's encoding with PRODUCT_LIMIT 1, alone
   and with every lever (counted, not a failure);
4. the fold check (taskcore --check-fold): 0 mismatches.
"""

import collections
import heapq
import itertools
import json
import os
import random
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

from domains import BINARY, Domain  # noqa: E402
from export_records import export_task  # noqa: E402
from task import Action, Effect, OutOfDomain, Task, write_sas  # noqa: E402

BIN = Path(os.environ.get("TASKCORE_BIN", HERE / "taskcore" / "target" / "release" / "taskcore"))
DOWNWARD = HERE.parents[1] / "third_party" / "downward" / "builds" / "release" / "bin" / "downward"
RAW = ["--fold", "off", "--reduce", "none"]
CONFIGS = [
    ("no lever", RAW),
    ("a", ["--fold", "off", "--reduce", "a"]),
    ("b", ["--fold", "off", "--reduce", "b"]),
    ("c", ["--fold", "off", "--reduce", "c"]),
    ("d", ["--fold", "off", "--reduce", "d"]),
    ("e", ["--fold", "off", "--reduce", "e"]),
    ("f", ["--fold", "off", "--reduce", "f"]),
    ("fold strict", ["--fold", "strict", "--reduce", "none", "--check-fold", "4"]),
    ("fold every", ["--fold", "every", "--reduce", "none", "--check-fold", "4"]),
    ("PRODUCT_LIMIT 1", RAW + ["--product-limit", "1"]),
    ("all, fold strict", ["--fold", "strict", "--reduce", "all"]),
    ("all, fold every", ["--fold", "every", "--reduce", "all"]),
    ("all, fold strict, PRODUCT_LIMIT 1", ["--fold", "strict", "--reduce", "all", "--product-limit", "1"]),
]


def rand_domain(rng):
    r = rng.random()
    if r < 0.35:
        return BINARY
    if r < 0.75:
        return Domain.of(range(rng.randint(3, 4)))
    return Domain.of(range(0, 6), low=1, high=3)  # below, 1, 2, 3, above


def subset(rng, vals, full_p=0.1, empty_p=0.0):
    if rng.random() < empty_p:
        return set()
    if rng.random() < full_p:
        return set(vals)
    k = rng.randint(1, max(1, len(vals) - 1))
    return set(rng.sample(list(vals), k))


def rand_effects(rng, facts, keys, pre, n):
    """``n`` effects on distinct keys (so two of them never write one key); ``pre`` may be narrowed (an add's room,
    as reader.build_task does)."""
    out = []
    for k in rng.sample(keys, min(n, len(keys))):
        dom = facts[k]
        r = rng.random()
        when = ()
        for _ in range(2):
            if rng.random() < 0.3:
                c = rng.choice(keys)
                if c != k and c not in dict(when):
                    when += ((c, frozenset(subset(rng, facts[c].values, full_p=0.15, empty_p=0.03))),)
        if r < 0.5:
            out.append(Effect(k, value=rng.choice(dom.values), when=when))
        elif r < 0.75 and len(dom.values) > 1:
            out.append(Effect(k, values=frozenset(rng.sample(list(dom.values), rng.randint(2, min(3, len(dom.values))))),
                              when=when))
        else:
            d = rng.choice([-1, 1, 2])
            ok = {v for v in pre.get(k, dom.values) if dom.add(v, d)}
            if ok != set(pre.get(k, dom.values)):
                pre[k] = ok
            out.append(Effect(k, add=d, when=((k, frozenset(ok)),)))
    return tuple(out)


def rand_task(rng):
    nf = rng.randint(3, 6)
    facts = {f"f{i}:{rng.choice('abc')}": rand_domain(rng) for i in range(nf)}
    keys = list(facts)
    unit = rng.random() < 0.4  # every action costs 1: task.py's metric is 0 (the kitchen's case)

    def cost():
        return 1 if unit else rng.randint(1, 3)
    actions = []
    for i in range(rng.randint(4, 12)):
        pre = {k: subset(rng, facts[k].values, empty_p=0.03) for k in rng.sample(keys, rng.randint(0, 2))}
        eff = rand_effects(rng, facts, keys, pre, rng.randint(1, 3))
        actions.append(Action(f"act{i}", pre=pre, effects=eff, cost=cost(), what=("sentence", rng.choice(["yield", "end"]), "")))
        if rng.random() < 0.25:  # a twin: the same effects, a precondition as strong or stronger, any cost (for (f))
            pre2 = {k: set(rng.sample(sorted(s, key=str), rng.randint(1, len(s)))) if s else set() for k, s in pre.items()}
            if rng.random() < 0.5:
                extra = rng.choice(keys)
                pre2.setdefault(extra, subset(rng, facts[extra].values))
            actions.append(Action(f"twin{i}", pre=pre2, effects=eff, cost=cost(), what=("sentence", "end", "")))
        if rng.random() < 0.02 and eff:  # two writes of one key that can both happen: task.py raises
            e = eff[0]
            other = next((v for v in facts[e.key].values if v != e.value), None)
            if e.value is not None and other is not None and not e.when:
                actions.append(Action(f"bad{i}", pre=pre, effects=(e, Effect(e.key, value=other)), cost=cost(),
                                      what=("sentence", "end", "")))
    # inputs that fork into paths: one action per box of a partition of one or two facts' domains
    for j in range(rng.randint(1, 3)):
        fork = rng.sample(keys, rng.randint(1, 2))
        parts = []
        for k in fork:
            vals = list(facts[k].values)
            rng.shuffle(vals)
            cut = rng.randint(1, len(vals) - 1) if len(vals) > 1 else 1
            parts.append([set(vals[:cut]), set(vals[cut:])] if rng.random() < 0.7 else [set(v for v in vals)])
        common = {k: subset(rng, facts[k].values) for k in rng.sample([k for k in keys if k not in fork],
                                                                       min(1, len(keys) - len(fork)))}
        shared = rand_effects(rng, facts, keys, dict(common), rng.randint(0, 1))
        end = rng.choice(["yield", "yield", "end"])
        c = cost()
        boxes = [{}]
        for k, ps in zip(fork, parts):
            boxes = [dict(b, **{k: p}) for b in boxes for p in ps if p]
        if len(boxes) > 1 and rng.random() < 0.2:
            boxes.pop(rng.randrange(len(boxes)))  # a hole
        for b in boxes:
            pre = dict(common)
            pre.update(b)
            own = tuple(e for e in rand_effects(rng, facts, keys, pre, rng.randint(1, 2))
                        if e.key not in {s.key for s in shared})
            actions.append(Action(f"in{j}", pre=pre, effects=shared + own, cost=c, what=("event", end, f"cont{j}")))
    rng.shuffle(actions)
    goal = {}
    for k in rng.sample(keys, rng.randint(1, 2)):
        goal[k] = set(rng.sample(list(facts[k].values), 1 if rng.random() < 0.8 else 2))
    start = {}
    for k, d in facts.items():
        ints = [v for v in range(-1, 7) if d.label(v) is not None]
        start[k] = rng.choice(ints)
    if rng.random() < 0.15:  # a start value outside a domain (a domain with no BELOW or ABOVE)
        exact = [k for k, d in facts.items() if d.exact]
        if exact:
            start[rng.choice(exact)] = 9
    return Task(facts, actions, goal), start


# --- the task's own optimum, from its states (no SAS file) -----------------------------------------------------------

OUT = "<out of the domain>"  # a start value outside its fact's domain (task.py raises unless no operator names it)


class Raises(Exception):
    """The task breaks a rule task.py's writer enforces (it raises there too)."""


def model_optimum(task: Task, start: dict):
    """The optimal cost of ``task`` from ``start`` by Dijkstra over its states (task.py's meaning, module docstring of
    task.py: a set is cut to the domain and holds for every value when it is the whole domain; conditions read the
    state before the action; a guess or an add whose result is several values is the planner's choice; the goal's
    sets must all hold at the end). None when no plan exists. Raises Raises when two effects that fire together write
    one fact differently."""
    keys = list(task.facts)
    dom = {k: task.facts[k].values for k in keys}
    pos = {k: i for i, k in enumerate(keys)}

    def label(k, v):
        lab = task.facts[k].label(v) if isinstance(v, int) else v
        return lab if lab in dom[k] else OUT

    def cut(k, s):  # None: holds for every value
        c = frozenset(s) & frozenset(dom[k])
        return None if len(c) == len(dom[k]) else c
    acts = []
    for a in task.actions:
        pre = [(pos[k], cut(k, s)) for k, s in a.pre.items()]
        if any(c is not None and not c for _, c in pre):
            continue  # a precondition no value meets
        effs = []
        for e in a.effects:
            when = [(k, cut(k, s)) for k, s in e.when]
            when = [(k, c) for k, c in when if c is not None]
            if e.add is not None:
                own = dict(when).get(e.key)
                rest = [(pos[k], c) for k, c in when if k != e.key]
                effs.append((pos[e.key], rest, ("add", e.add, own)))
            elif e.values is not None:
                effs.append((pos[e.key], [(pos[k], c) for k, c in when], ("vals", frozenset(label(e.key, v) for v in e.values))))
            else:
                effs.append((pos[e.key], [(pos[k], c) for k, c in when], ("vals", frozenset({label(e.key, e.value)}))))
        acts.append(([(i, c) for i, c in pre if c is not None], effs, a.cost))
    goal = []
    for k, s in task.goal.items():
        vals = {task.facts[k].label(v) if isinstance(v, int) else v for v in s} & set(dom[k])
        if not vals:
            raise Raises(f"goal {k}: no value of its domain")
        goal.append((pos[k], vals))
    s0 = tuple(label(k, start.get(k, 0)) for k in keys)
    best = {s0: 0}
    heap = [(0, 0, s0)]
    tie = itertools.count(1)
    while heap:
        g, _, s = heapq.heappop(heap)
        if g > best[s]:
            continue
        if all(s[i] in vals for i, vals in goal):
            return g
        for pre, effs, c in acts:
            if not all(s[i] in cs for i, cs in pre):
                continue
            fired = []  # (fact, its possible values), one per effect that fires
            for i, when, (kind, x, *own) in effs:
                if not all(s[j] in cs for j, cs in when):
                    continue
                if kind == "add":
                    if own[0] is not None and s[i] not in own[0]:
                        continue
                    if s[i] == OUT:
                        continue  # task.py's add is one effect per domain value: none fires here
                    vals = frozenset(task.facts[keys[i]].add(s[i], x))
                    if not vals:
                        raise Raises(f"{keys[i]} {s[i]} + {x} is not in the domain")
                else:
                    vals = x
                if OUT in vals:
                    raise Raises(f"a write of a value outside {keys[i]}'s domain")
                fired.append((i, vals))
            # each choice of the guessed values: a write of the value the fact has changes nothing (task.py leaves
            # it out); two other writes of one fact must agree. They may differ only for two equal guesses of one
            # fact, which task.py writes through one pick variable (one choice serves both; as variants it raises)
            for choice in itertools.product(*(sorted(v, key=str) for _, v in fired)):
                t = list(s)
                writes, ok = {}, True
                for (i, vals), v in zip(fired, choice):
                    if v == s[i]:
                        continue
                    if i in writes and writes[i][0] != v:
                        if not (len(vals) > 1 and vals == writes[i][1]):
                            raise Raises(f"two writes of {keys[i]} fire together: {writes[i][0]} and {v}")
                        ok = False
                        break
                    writes[i] = (v, vals)
                if not ok:
                    continue
                for i, (v, _) in writes.items():
                    t[i] = v
                t = tuple(t)
                if g + c < best.get(t, 1 << 60):
                    best[t] = g + c
                    heapq.heappush(heap, (g + c, next(tie), t))
    return None


# --- running things ---------------------------------------------------------------------------------------------------

def run(cmd, **kw):
    return subprocess.run([str(c) for c in cmd], capture_output=True, text=True, **kw)


def fd_optimum(sas: Path, work: Path) -> int | None:
    """astar(blind()) on ``sas``: Fast Downward's own optimal cost ("Plan cost": with metric 0 every operator counts
    1, pick and goal operators too), or None when there is no plan."""
    plan = work / "plan"
    if plan.exists():
        plan.unlink()
    with open(sas) as f:
        p = subprocess.run([str(DOWNWARD), "--search", "astar(blind())", "--internal-plan-file", str(plan)],
                           stdin=f, capture_output=True, text=True, timeout=300)
    if p.returncode in (11, 12):
        return None
    if p.returncode != 0:
        raise RuntimeError(f"downward exit {p.returncode} on {sas}: {p.stdout[-2000:]} {p.stderr[-2000:]}")
    return int(re.findall(r"Plan cost: (\d+)", p.stdout)[-1])


def taskcore(work: Path, args: list):
    """taskcore sas on work/t.tc from start s: (file, stats) or (None, the error)."""
    out, st = work / "r.sas", work / "r.json"
    r = run([BIN, "sas", "--task", work / "t.tc", "--start", "s", "--out", out, "--stats", st] + args)
    if r.returncode != 0:
        return None, r.stderr.strip()
    return out, json.loads(st.read_text())


def check_one(task, start, work: Path, counts: collections.Counter, fails: list) -> None:
    export_task(work / "t.tc", task, {"s": start}, meta=lambda a: a.what)
    sas = None
    try:
        sas = write_sas(task)
        py = sas.text(start)
    except OutOfDomain:
        py = "out of domain"
    except ValueError:
        py = None
    r0, info = taskcore(work, RAW + ["--encoding", "task"])
    if py is None:
        counts["task.py raises (a rule of its writer)"] += 1
        if r0 is not None:
            fails.append(("task.py raises, taskcore does not", task, start))
        return
    if py == "out of domain":
        counts["task.py raises OutOfDomain (a start outside a domain)"] += 1
        ok = True
        for name, args in CONFIGS:
            r, err = taskcore(work, args)
            if r is not None or "out of their domains" not in err:
                fails.append((name, "task.py raises OutOfDomain, taskcore does not", err, task, start))
                ok = False
        counts["OutOfDomain: taskcore raises too, in every configuration"] += ok
        return
    if r0 is None:
        fails.append(("taskcore raises, task.py does not", info, task, start))
        return
    counts["tasks"] += 1
    if r0.read_text() != py:
        fails.append(("format", task, start))
        return
    counts["format: same bytes"] += 1
    if any(task.facts[k].label(v) not in task.facts[k].values for k, v in start.items()):
        counts["starts with a value outside a domain no operator names (task.py writes the file)"] += 1
    try:
        truth = model_optimum(task, start)
    except Raises as e:
        counts["the state search finds a rule broken that the writer let through"] += 1
        fails.append(("state search", str(e), task, start))
        return
    counts["solvable" if truth is not None else "unsolvable"] += 1
    goal_sets = any(len(v) > 1 for v in task.goal.values())
    counts["goal sets" if goal_sets else "single-value goals"] += 1
    metric0 = py.split("\n")[4] == "0"
    picks = sas.counts["pick operators"] > 0
    got_py = fd_optimum(r0, work)
    if got_py != truth:
        why = ("goal set" if goal_sets else "") + (" metric 0 with pick operators" if metric0 and picks else "")
        counts[f"task.py's file: Fast Downward's optimum differs from the task's ({why.strip() or '?'})"] += 1
        if not why:
            fails.append(("task.py's file", f"optimum {got_py} against the task's {truth}", task, start))
    for name, args in (("PRODUCT_LIMIT 1", RAW + ["--product-limit", "1"]),
                       ("all, fold strict, PRODUCT_LIMIT 1", ["--fold", "strict", "--reduce", "all", "--product-limit", "1"])):
        r, _ = taskcore(work, args + ["--encoding", "task"])
        if r is not None and fd_optimum(r, work) != truth:
            counts[f"task.py's encoding, {name}: Fast Downward's optimum differs from the task's"] += 1
    for name, args in CONFIGS:
        r, st = taskcore(work, args)
        if r is None:
            fails.append((name, "taskcore failed", st, task, start))
            continue
        if "fold_check" in st:
            counts["fold check: folds"] += st["fold_check"]["folds"]
            counts["fold check: states"] += st["fold_check"]["states"] + st["fold_check"]["union_box_states"]
            if st["fold_check"]["mismatches"]:
                fails.append((name, "fold check", st["fold_check"], task, start))
        counts[f"{name}: operators"] += st["sas"]["operators"]
        got = fd_optimum(r, work)
        if got != truth:
            fails.append((name, f"optimum {got} against the task's {truth}", task, start))
        else:
            counts[f"{name}: the task's optimum"] += 1


def run_many(n: int, seed: int = 1):
    rng = random.Random(seed)
    counts, fails = collections.Counter(), []
    with tempfile.TemporaryDirectory(prefix="taskcore-test-") as tmp:
        for _ in range(n):
            task, start = rand_task(rng)
            check_one(task, start, Path(tmp), counts, fails)
    return counts, fails


# --- fixed cases (the review of stage 1) -----------------------------------------------------------------------------

Y = ("event", "yield", "c")  # (kind, end, cont): one fold group


def _all_configs(task, start, extra=()):
    """{config: Fast Downward's optimum, or the error} over CONFIGS (+ extra), and the task's own optimum."""
    out = {}
    with tempfile.TemporaryDirectory(prefix="taskcore-case-") as tmp:
        work = Path(tmp)
        export_task(work / "t.tc", task, {"s": start}, meta=lambda a: a.what)
        for name, args in list(CONFIGS) + list(extra):
            r, st = taskcore(work, args)
            out[name] = st if r is None else fd_optimum(r, work)
    return out


def test_start_out_of_domain():
    """(d) read a start value outside a domain as no value: it dropped K=1 as always true (plan [x]); the fold made F
    unread. task.py raises OutOfDomain on both; so must every configuration."""
    k = Task({"K": BINARY, "G": BINARY, "L": BINARY},
             [Action("x", pre={"K": {1}}, effects=(Effect("G", 1),), what=("s", "end", "")),
              Action("z", pre={"L": {0}}, effects=(Effect("K", 1),), what=("s", "end", ""))], {"G": {1}})
    f = Task({"F": BINARY, "G": BINARY},
             [Action("in", pre={"F": {0}}, effects=(Effect("G", 1),), what=Y),
              Action("in", pre={"F": {1}}, effects=(Effect("G", 1),), what=Y)], {"G": {1}})
    for task, start in ((k, {"K": 5}), (f, {"F": 5})):
        try:
            write_sas(task).text(start)
            raise AssertionError("task.py should raise")
        except OutOfDomain:
            pass
        res = _all_configs(task, start)
        assert all(isinstance(v, str) and "out of their domains" in v for v in res.values()), res
    assert set(_all_configs(k, {"K": 0}).values()) == {2}  # z, x


def test_fold_pre_limit():
    """Two paths of 4,096 variants each, split on x0: their fold would have 8,192 and stop the writer; it stays two
    actions."""
    d3 = Domain.of([0, 1, 2])
    facts = {f"x{i}": d3 for i in range(13)} | {"G": BINARY}
    pre = {f"x{i}": {0, 1} for i in range(1, 13)}
    task = Task(facts, [Action("in", pre=dict(pre, x0={0}), effects=(Effect("G", 1),), what=Y),
                        Action("in", pre=dict(pre, x0={1}), effects=(Effect("G", 1),), what=Y)], {"G": {1}})
    start = {k: 0 for k in facts}
    with tempfile.TemporaryDirectory(prefix="taskcore-case-") as tmp:
        work = Path(tmp)
        export_task(work / "t.tc", task, {"s": start}, meta=lambda a: a.what)
        r, st = taskcore(work, ["--fold", "strict", "--reduce", "none"])
        assert r is not None, st
        assert st["fold_counts"]["subgroups_past_pre_limit_left_unfolded"] == 1 and st["actions"] == 2
        assert fd_optimum(r, work) == 1


def test_metric_and_goal_set():
    """With metric 0 Fast Downward counts pick operators; (e) dropping the only cost-2 action flipped the metric.
    task.py's goal set is a latch: x, y meets B in {1, 2} after x and leaves it (cost 4); the task's optimum is
    x, y, x (6). In the model encoding every configuration gives the task's optimum."""
    d5 = Domain.of(range(0, 6), low=1, high=3)
    flip = Task({"G": d5, "X": BINARY},
                [Action("guess", effects=(Effect("G", values=frozenset({"above", 3})),), cost=1, what=("s", "end", "")),
                 Action("other", effects=(Effect("X", 1),), cost=2, what=("s", "end", ""))], {"G": {"above"}})
    latch = Task({"A": BINARY, "B": Domain.of([0, 1, 2])},
                 [Action("x", pre={"B": {0}}, effects=(Effect("B", 1),), cost=2, what=("s", "end", "")),
                  Action("y", pre={"B": {1}}, effects=(Effect("A", 1), Effect("B", 0)), cost=2, what=("s", "end", ""))],
                 {"A": {1}, "B": {1, 2}})
    for task, start, truth in ((flip, {"G": 1, "X": 0}, 1), (latch, {"A": 0, "B": 0}, 6)):
        assert model_optimum(task, start) == truth
        extra = [("(e), PRODUCT_LIMIT 1", ["--fold", "off", "--reduce", "e", "--product-limit", "1"])]
        assert set(_all_configs(task, start, extra).values()) == {truth}
    with tempfile.TemporaryDirectory(prefix="taskcore-case-") as tmp:
        work = Path(tmp)
        export_task(work / "t.tc", latch, {"s": {"A": 0, "B": 0}}, meta=lambda a: a.what)
        r, _ = taskcore(work, RAW + ["--encoding", "task"])
        assert fd_optimum(r, work) == 4  # task.py's encoding: the latch


def test_random_tasks():
    counts, fails = run_many(120)
    assert not fails, fails[:3]
    assert counts["format: same bytes"] == counts["tasks"]
    assert counts["task.py raises (a rule of its writer)"] > 0 and counts["fold check: folds"] > 0
    assert counts["task.py raises OutOfDomain (a start outside a domain)"] > 0 and counts["goal sets"] > 0


if __name__ == "__main__":
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 300
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    for t in (test_start_out_of_domain, test_fold_pre_limit, test_metric_and_goal_set):
        t()
        print(f"{t.__name__}: ok")
    counts, fails = run_many(n, seed)
    for k, v in sorted(counts.items()):
        print(f"{v:8}  {k}")
    print(f"failures: {len(fails)}")
    for f in fails[:10]:
        print("  ", str(f)[:3000])
    sys.exit(1 if fails else 0)
