"""Tests of taskcore's pick windows (``--pick-windows on``; taskcore/README.md, "Pick windows").

usage: python test_pick_windows.py [N] [seed]       (N random tasks, default 200; the summary on stdout)
       python test_pick_windows.py real TC START [--goal K=V] [--plain-plan F] [--window-plan F] [--args ..]
       pytest test_pick_windows.py                  (N = 60)
       TASKCORE_BIN=<binary>                        (default taskcore/target/release/taskcore)

For each random task (test_taskcore's) and each configuration below, taskcore writes the file with the switch off
(Π) and on (Π^W), each with its operator table, and:

1. the optimum: Fast Downward's astar(blind()) on Π and on Π^W gives the task's own optimum (test_taskcore's
   ``model_optimum``, a search over the task's states), or no plan on both;
2. Theorem 1: Π's optimal plan, mapped by the theorem's construction (each consumer becomes its window, with the pick
   values Π had there), is a plan of Π^W, run here operator by operator on the file (``Sas.run``), with the same
   cost;
3. Theorem 2: Π^W's optimal plan, mapped back (each window's end becomes pick operators with the window's values, then
   the operator), is a plan of Π with the same cost;
4. reading back: ``taskcore decode`` of Π^W's plan gives the same steps (actions, chosen, guessed and picked values)
   as ``decode`` of the mapped plan on Π, and ``plan_fd.decode_ops`` (the harness's reading through the operator
   table) gives taskcore decode's steps from both tables: Π^W's with its window rows, and Π's;
5. a file with no window and no pick operator is the same bytes with the switch on.

``real`` runs 2 to 4 on a packed file: the plans are Fast Downward plan files (``--plain-plan`` on Π, ``--window-plan``
on Π^W), mapped across and run on the other file.
"""

import collections
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

import test_taskcore as TC  # noqa: E402
from export_records import export_task  # noqa: E402

BIN = TC.BIN
ON = ["--pick-windows", "on"]
CONFIGS = [
    ("PRODUCT_LIMIT 1", TC.RAW + ["--product-limit", "1"]),
    ("all, fold strict, PRODUCT_LIMIT 1", ["--fold", "strict", "--reduce", "all", "--product-limit", "1"]),
    ("PRODUCT_LIMIT 2", TC.RAW + ["--product-limit", "2"]),
    ("default", []),
]


class Sas:
    """A SAS file as Fast Downward reads it (root_task.cc): variables, start, goal, operators."""

    def __init__(self, path: Path):
        lines = Path(path).read_text().split("\n")
        it = iter(lines)
        nxt = lambda: next(it)  # noqa: E731
        assert nxt() == "begin_version" and nxt() == "3" and nxt() == "end_version" and nxt() == "begin_metric"
        self.metric = nxt() == "1"
        nxt()
        self.names, self.sizes = [], []
        for _ in range(int(nxt())):
            assert nxt() == "begin_variable"
            self.names.append(nxt())
            nxt()
            n = int(nxt())
            self.sizes.append(n)
            for _ in range(n):
                nxt()
            assert nxt() == "end_variable"
        assert nxt() == "0" and nxt() == "begin_state"
        self.init = tuple(int(nxt()) for _ in self.names)
        assert nxt() == "end_state" and nxt() == "begin_goal"
        self.goal = [tuple(map(int, nxt().split())) for _ in range(int(nxt()))]
        assert nxt() == "end_goal"
        self.ops = []
        for _ in range(int(nxt())):
            assert nxt() == "begin_operator"
            name = nxt()
            prev = [tuple(map(int, nxt().split())) for _ in range(int(nxt()))]
            effs = []
            for _ in range(int(nxt())):
                w = list(map(int, nxt().split()))
                nc = w[0]
                effs.append(([(w[1 + 2 * k], w[2 + 2 * k]) for k in range(nc)], w[1 + 2 * nc], w[2 + 2 * nc], w[3 + 2 * nc]))
            cost = int(nxt())
            assert nxt() == "end_operator"
            self.ops.append((name, prev, effs, cost))

    def applies(self, n: int, s) -> bool:
        _, prev, effs, _ = self.ops[n]
        return all(s[v] == d for v, d in prev) and all(pre == -1 or s[v] == pre for _, v, pre, _ in effs)

    def step(self, n: int, s) -> list:
        t = list(s)
        for conds, v, _, post in self.ops[n][2]:
            if all(s[cv] == cd for cv, cd in conds):
                t[v] = post
        return t

    def run(self, plan: list, end=False):
        """Run the operators ``plan`` (numbers) from the start: (cost, goal reached), or (None, why) where one does not
        apply. Conditions read the state before the operator, as Fast Downward's. With ``end``, the last state too."""
        s = list(self.init)
        cost = 0
        for i, n in enumerate(plan):
            assert self.ops[n][0] == f"a{n}", self.ops[n][0]
            if not self.applies(n, s):
                return (None, f"step {i} (a{n}) does not apply") + ((None,) if end else ())
            s = self.step(n, s)
            cost += self.ops[n][3] if self.metric else 1
        return (cost, all(s[v] == d for v, d in self.goal)) + ((s,) if end else ())

    def facts(self, s) -> dict:
        """A state on the task's facts and ``goal`` (F): every variable but the pick variables and ``lock``."""
        return {k: v for k, v in zip(self.names, s) if not k.startswith("pick:") and k != "lock"}

    def walk(self, rng, length: int, prefer) -> list:
        """A random operator sequence from the start, ``length`` long or until none applies; an applicable operator
        that ``prefer`` names is taken half of the time."""
        s, out = list(self.init), []
        for _ in range(length):
            ok = [n for n in range(len(self.ops)) if self.applies(n, s)]
            if not ok:
                break
            good = [n for n in ok if prefer(n)]
            n = rng.choice(good if good and rng.random() < 0.5 else ok)
            out.append(n)
            s = self.step(n, s)
        return out


class Ops:
    """taskcore's operator table (sas.rs ``write_ops``), indexed for the maps."""

    def __init__(self, path: Path):
        rows = [json.loads(l) for l in Path(path).read_text().splitlines()]
        self.head, self.op, self.acts = rows[0], {}, {}
        self.action_rows, self.goal_rows = [], []  # op numbers, in file order
        self.pick, self.first = {}, {}  # (window or None, key, value) -> op; key -> its first value
        self.begin, self.window_keys = {}, collections.defaultdict(list)
        for d in rows[1:]:
            if "op" not in d:
                self.acts[d["action"]] = d
                continue
            self.op[d["op"]] = d
            if "action" in d:
                self.action_rows.append(d["op"])
            elif d.get("goal"):
                self.goal_rows.append(d["op"])
            elif d.get("begin"):
                self.begin[d["window"]] = d["op"]
            elif "pick" in d:
                k, v = d["pick"]
                j = d.get("window")
                self.pick[(j, k, v)] = d["op"]
                self.first.setdefault(k, v)
                if j is not None and k not in self.window_keys[j]:
                    self.window_keys[j].append(k)
        self.pos = {n: i for i, n in enumerate(self.action_rows)}
        self.gpos = {n: i for i, n in enumerate(self.goal_rows)}


def same_rows(p: Ops, w: Ops) -> bool:
    """The variants are the same, in the same order, in both tables (the window's operator is the variant's own)."""
    if len(p.action_rows) != len(w.action_rows) or len(p.goal_rows) != len(w.goal_rows):
        return False
    strip = lambda d: {k: v for k, v in d.items() if k not in ("op", "window")}  # noqa: E731
    return all(strip(p.op[a]) == strip(w.op[b]) for a, b in zip(p.action_rows, w.action_rows))


def to_windows(plan: list, p: Ops, w: Ops) -> list:
    """Theorem 1's map: Π's plan to Π^W's."""
    picks, out = {}, []
    for n in plan:
        d = p.op[n]
        if "pick" in d:
            picks[d["pick"][0]] = d["pick"][1]
        elif d.get("goal"):
            out.append(w.goal_rows[p.gpos[n]])
        else:
            m = w.action_rows[p.pos[n]]
            j = w.op[m].get("window")
            if j is not None:
                out.append(w.begin[j])
                for k in w.window_keys[j]:
                    out.append(w.pick[(j, k, picks.get(k, w.first[k]))])
            out.append(m)
    return out


def to_plain(plan: list, p: Ops, w: Ops) -> list:
    """Theorem 2's map: Π^W's plan to Π's. Before each operator, every pick variable of its action is given the value
    the window gave it, or its first value (Lemma 2), so that ``decode`` reads the same values on both."""
    picks, out = {}, []
    for n in plan:
        d = w.op[n]
        if d.get("begin"):
            continue
        if "pick" in d:
            picks[d["pick"][0]] = d["pick"][1]
        elif d.get("goal"):
            out.append(p.goal_rows[w.gpos[n]])
        else:
            m = p.action_rows[w.pos[n]]
            keys = []
            for k, _, _ in p.acts[p.op[m]["action"]]["picked"]:
                if k not in keys:
                    keys.append(k)
            out += [p.pick[(None, k, picks.get(k, p.first[k]))] for k in keys]
            out.append(m)
            picks = {}
    return out


def py_decode(ops_path: Path, plan: list) -> list:
    """``plan_fd.decode_ops`` of ``plan`` through the operator table at ``ops_path``, as comparable rows: (action,
    name, members, chosen, guessed), values as their text."""
    import plan_fd
    _, ops, acts = plan_fd.read_ops(ops_path)
    text = "".join(f"(a{n})\n" for n in plan)
    return [(s["action"], s["name"], list(s["members"]), [[k, str(v)] for k, v in s["chosen"]],
             [[k, str(v)] for k, v, _ in s["guessed"]]) for s in plan_fd.decode_ops(text, ops, acts)]


def decode_rows(steps: list) -> list:
    """taskcore decode's steps as ``py_decode``'s rows."""
    return [(s["action"], s["name"], list(s["members"]), [[k, str(v)] for k, v in s["chosen"]],
             [[k, str(v)] for k, v in s["guessed"]]) for s in steps]


def fd_plan(sas: Path, work: Path, search="astar(blind())"):
    """Fast Downward on ``sas``: (cost, operator numbers), or None when it proves there is no plan."""
    plan = work / "fdplan"
    if plan.exists():
        plan.unlink()
    with open(sas) as f:
        r = subprocess.run([str(TC.DOWNWARD), "--search", search, "--internal-plan-file", str(plan)], stdin=f,
                           capture_output=True, text=True, timeout=600)
    if r.returncode in (11, 12):
        return None
    if r.returncode != 0:
        raise RuntimeError(f"downward exit {r.returncode} on {sas}: {r.stdout[-1500:]}")
    return int(re.findall(r"Plan cost: (\d+)", r.stdout)[-1]), read_plan(plan)


def read_plan(path: Path) -> list:
    return [int(l.strip().strip("()").split()[0][1:]) for l in Path(path).read_text().splitlines() if l.startswith("(")]


def decode(work: Path, task_args: list, args: list, plan: list, tag: str) -> list:
    f = work / f"{tag}.plan"
    f.write_text("".join(f"(a{n})\n" for n in plan))
    out = work / f"{tag}.steps.json"
    r = subprocess.run([str(BIN), "decode"] + task_args + args + ["--plan", str(f), "--out", str(out)],
                       capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(r.stderr)
    return [{k: v for k, v in s.items() if k != "op"} for s in json.loads(out.read_text())["steps"]]


def write(work: Path, task_args: list, args: list, tag: str):
    """taskcore sas: (file, operator table, stats), or (None, the error)."""
    sas, ops, st = work / f"{tag}.sas", work / f"{tag}.ops", work / f"{tag}.json"
    r = subprocess.run([str(BIN), "sas"] + task_args + args + ["--out", str(sas), "--ops", str(ops), "--stats", str(st)],
                       capture_output=True, text=True)
    if r.returncode != 0:
        return None, r.stderr.strip()
    return (sas, ops, json.loads(st.read_text())), None


def check_maps(work, task_args, args, plain, win, plain_plan, win_plan, counts, fails, name) -> None:
    """2 to 4 of the module docstring on two written files and a plan of each (either may be None)."""
    P, W = Ops(plain[1]), Ops(win[1])
    if not same_rows(P, W):
        fails.append((name, "the operator tables' variants differ"))
        return
    sp, sw = Sas(plain[0]), Sas(win[0])
    if plain_plan is not None:
        cost, goal = sp.run(plain_plan)
        mapped = to_windows(plain_plan, P, W)
        c2, g2 = sw.run(mapped)
        if cost is None or goal is not True:
            fails.append((name, "Π's plan does not run on Π", goal))
        elif c2 != cost or g2 is not True:
            fails.append((name, "theorem 1: the mapped plan", cost, c2, g2))
        else:
            counts["theorem 1: the mapped plan runs on Π^W, the same cost"] += 1
            nw = sum(1 for n in mapped if W.op[n].get("begin"))
            counts["theorem 1: windows in those plans"] += nw
            counts["theorem 1: plans through a window"] += nw > 0
    if win_plan is not None:
        cost, goal = sw.run(win_plan)
        mapped = to_plain(win_plan, P, W)
        c2, g2 = sp.run(mapped)
        if cost is None or goal is not True:
            fails.append((name, "Π^W's plan does not run on Π^W", goal))
        elif c2 != cost or g2 is not True:
            fails.append((name, "theorem 2: the mapped plan", cost, c2, g2))
        else:
            counts["theorem 2: the mapped plan runs on Π, the same cost"] += 1
            nw = sum(1 for n in win_plan if W.op[n].get("begin"))
            counts["theorem 2: windows in those plans"] += nw
            counts["theorem 2: plans through a window"] += nw > 0
            a = decode(work, task_args, args + ON, win_plan, "w")
            b = decode(work, task_args, args, mapped, "p")
            if a != b:
                fails.append((name, "decode: other steps", a[:3], b[:3]))
            else:
                counts["decode: the same steps"] += 1
            pa, pb, ra = py_decode(win[1], win_plan), py_decode(plain[1], mapped), decode_rows(a)
            if pa != ra or pb != ra:
                fails.append((name, "decode_ops: other steps than taskcore decode", pa[:2], pb[:2], ra[:2]))
            else:
                counts["decode_ops: taskcore decode's steps from both tables"] += 1
                counts["decode_ops: of them through a window"] += nw > 0


def to_consumers(sp: Sas, P: Ops, consumer: set, rng, n: int, cap: int = 20000) -> list:
    """Up to ``n`` operator sequences of Π that end in a consumer: a breadth-first search over Π's other operators
    (picks left out) from the start, up to ``cap`` states; at each state where consumers apply, one of them, after
    random picks of every pick variable of its action."""
    from collections import deque
    plain_ops = [m for m in range(len(sp.ops)) if "pick" not in P.op.get(m, {})]
    seen = {tuple(sp.init): None}
    q, out, used = deque([(tuple(sp.init), [])]), [], set()
    while q and len(out) < n and len(seen) < cap:
        s, path = q.popleft()
        for m in plain_ops:
            if not sp.applies(m, s):
                continue
            if m in consumer and m not in used:
                used.add(m)
                picks = []
                for k, _, _ in P.acts[P.op[m]["action"]]["picked"]:
                    vals = [v for (j, kk, v) in P.pick if j is None and kk == k]
                    picks.append(P.pick[(None, k, rng.choice(vals))])
                out.append(path + picks + [m])
                if len(out) >= n:
                    break
                continue
            t = tuple(sp.step(m, s))
            if t not in seen:
                seen[t] = None
                q.append((t, path + [m]))
    return out


def check_walks(plain, win, rng, n: int, length: int, counts, fails, name, dec=None) -> None:
    """The two maps on random operator sequences rather than plans: each walk on one file, mapped to the other, runs
    there and ends in a state with the same facts (F). Consumers on Π, and windows on Π^W, are preferred. With
    ``dec`` (work, task_args, args), each walk on Π^W is also read back: taskcore decode on Π^W, and decode_ops on
    both tables, give the same steps."""
    P, W = Ops(plain[1]), Ops(win[1])
    sp, sw = Sas(plain[0]), Sas(win[0])
    consumer = {p_op for p_op, w_op in zip(P.action_rows, W.action_rows) if "window" in W.op[w_op]}
    in_window = lambda m: W.op[m].get("begin") or W.op[m].get("window") is not None  # noqa: E731
    for i, walk in enumerate(to_consumers(sp, P, consumer, rng, n)):
        mapped = to_windows(walk, P, W)
        a, b = sp.run(walk, end=True), sw.run(mapped, end=True)
        if a[0] is None or b[0] is None or a[0] != b[0] or sp.facts(a[2]) != sw.facts(b[2]):
            fails.append((name, "theorem 1 on a sequence to a consumer", i, a[:2], b[:2]))
            continue
        counts["sequences of Π to a consumer: mapped, the same facts and cost"] += 1
        back = to_plain(mapped, P, W)
        c = sp.run(back, end=True)
        if c[0] != a[0] or sp.facts(c[2]) != sp.facts(a[2]):
            fails.append((name, "theorem 2 on the window of a sequence to a consumer", i, a[:2], c[:2]))
        else:
            counts["the same, mapped back by theorem 2: the same facts and cost"] += 1
    for i in range(n):
        walk = sp.walk(rng, length, lambda m: m in consumer or "pick" in P.op.get(m, {}))
        mapped = to_windows(walk, P, W)
        a, b = sp.run(walk, end=True), sw.run(mapped, end=True)
        if a[0] is None or b[0] is None or a[0] != b[0] or sp.facts(a[2]) != sw.facts(b[2]):
            fails.append((name, "theorem 1 on a walk", i, a[:2], b[:2]))
        else:
            counts["walks on Π: mapped, the same facts and cost"] += 1
            counts["walks on Π: consumers"] += sum(1 for m in walk if m in consumer)
        walk = sw.walk(rng, length, in_window)
        while walk and (W.op[walk[-1]].get("begin") or "pick" in W.op[walk[-1]]):
            walk.pop()  # end outside a window (a window changes no fact of F: Lemma 2)
        mapped = to_plain(walk, P, W)
        a, b = sw.run(walk, end=True), sp.run(mapped, end=True)
        if a[0] is None or b[0] is None or a[0] != b[0] or sw.facts(a[2]) != sp.facts(b[2]):
            fails.append((name, "theorem 2 on a walk", i, a[:2], b[:2]))
        else:
            counts["walks on Π^W: mapped, the same facts and cost"] += 1
            counts["walks on Π^W: windows closed"] += sum(1 for m in walk if W.op[m].get("window") is not None
                                                          and "action" in W.op[m])
            if dec is not None:
                work, task_args, args = dec
                ra = decode_rows(decode(work, task_args, args + ON, walk, "ww"))
                rb = decode_rows(decode(work, task_args, args, mapped, "wp"))
                pa, pb = py_decode(win[1], walk), py_decode(plain[1], mapped)
                if not (ra == rb == pa == pb):
                    fails.append((name, "theorem 2 on a walk: decode or decode_ops differ", i))
                else:
                    counts["walks on Π^W: decode and decode_ops, the same steps on both"] += 1


def check_one(task, start, work: Path, counts: collections.Counter, fails: list) -> None:
    export_task(work / "t.tc", task, {"s": start}, meta=lambda a: a.what)
    task_args = ["--task", str(work / "t.tc"), "--start", "s"]
    truth = None
    try:
        truth = TC.model_optimum(task, start)
    except TC.Raises:
        truth = "raises"
    for name, args in CONFIGS:
        plain, e1 = write(work, task_args, args, "p")
        win, e2 = write(work, task_args, args + ON, "w")
        if plain is None or win is None:
            if e1 != e2:
                fails.append((name, "one stops, the other does not", e1, e2))
            else:
                counts["both stop, the same message"] += 1
            continue
        if truth == "raises":
            continue
        counts[f"{name}: tasks"] += 1
        nw = win[2]["sas"]["pick_windows"]
        counts[f"{name}: windows"] += nw
        counts[f"{name}: operators off"] += plain[2]["sas"]["operators"]
        counts[f"{name}: operators on"] += win[2]["sas"]["operators"]
        if nw == 0 and plain[2]["sas"]["pick_operators"] == 0:
            if plain[0].read_bytes() != win[0].read_bytes():
                fails.append((name, "no window, no pick: the files differ"))
            else:
                counts["no window, no pick: the same bytes"] += 1
        a, b = fd_plan(plain[0], work), fd_plan(win[0], work)
        got = [None if x is None else x[0] for x in (a, b)]
        if got != [truth, truth]:
            fails.append((name, f"optima off/on {got} against the task's {truth}", task, start))
            continue
        counts[f"{name}: the task's optimum, off and on"] += 1
        if nw > 0:
            counts[f"{name}: tasks with a window"] += 1
        check_maps(work, task_args, args, plain, win, a and a[1], b and b[1], counts, fails, name)


def run_many(n: int, seed: int = 1):
    rng = random.Random(seed)
    counts, fails = collections.Counter(), []
    with tempfile.TemporaryDirectory(prefix="pick-windows-test-") as tmp:
        for _ in range(n):
            task, start = TC.rand_task(rng)
            check_one(task, start, Path(tmp), counts, fails)
    return counts, fails


def real(argv) -> int:
    """``real TC START [--goal G] [--plain-plan F] [--window-plan F] [--args ..]``: 2 to 4 on a packed file."""
    import argparse
    ap = argparse.ArgumentParser(prog="test_pick_windows.py real")
    ap.add_argument("tc")
    ap.add_argument("start")
    ap.add_argument("--goal")
    ap.add_argument("--plain-plan", type=Path)
    ap.add_argument("--window-plan", type=Path)
    ap.add_argument("--work", type=Path)
    ap.add_argument("--walks", type=int, default=0, help="random operator sequences each way (check_walks)")
    ap.add_argument("--length", type=int, default=40)
    ap.add_argument("--args", nargs=argparse.REMAINDER, default=[])
    a = ap.parse_args(argv)
    task_args = ["--task", a.tc] + (["--start-file", a.start] if os.path.exists(a.start) else ["--start", a.start])
    task_args += ["--goal", a.goal] if a.goal else []
    counts, fails = collections.Counter(), []
    with tempfile.TemporaryDirectory(prefix="pick-windows-real-") as tmp:
        work = a.work or Path(tmp)
        work.mkdir(parents=True, exist_ok=True)
        plain, e1 = write(work, task_args, a.args, "p")
        win, e2 = write(work, task_args, a.args + ON, "w")
        if plain is None or win is None:
            print("taskcore stopped:", e1, e2)
            return 1
        print(f"operators off {plain[2]['sas']['operators']}, on {win[2]['sas']['operators']}; windows "
              f"{win[2]['sas']['pick_windows']}, window picks {win[2]['sas']['window_pick_operators']}")
        pp = read_plan(a.plain_plan) if a.plain_plan else None
        wp = read_plan(a.window_plan) if a.window_plan else None
        check_maps(work, task_args, a.args, plain, win, pp, wp, counts, fails, "real")
        if a.walks:
            check_walks(plain, win, random.Random(1), a.walks, a.length, counts, fails, "real",
                        dec=(work, task_args, a.args))
    for k, v in sorted(counts.items()):
        print(f"{v:6d}  {k}")
    print(f"failures: {len(fails)}")
    for f in fails:
        print("  ", str(f)[:2000])
    return 1 if fails else 0


def test_random_tasks():
    counts, fails = run_many(60, seed=3)
    assert not fails, fails[:3]
    assert counts["PRODUCT_LIMIT 1: tasks with a window"] > 0
    assert counts["theorem 2: the mapped plan runs on Π, the same cost"] > 0
    assert counts["decode: the same steps"] > 0 and counts["no window, no pick: the same bytes"] > 0
    assert counts["decode_ops: of them through a window"] > 0
    assert counts["theorem 1: plans through a window"] > 0 and counts["theorem 2: plans through a window"] > 0


def test_task_encoding_refused(tmp_path):
    task, start = TC.rand_task(random.Random(5))
    export_task(tmp_path / "t.tc", task, {"s": start}, meta=lambda a: a.what)
    r = subprocess.run([str(BIN), "sas", "--task", str(tmp_path / "t.tc"), "--start", "s", "--encoding", "task",
                        "--out", str(tmp_path / "x.sas")] + ON, capture_output=True, text=True)
    assert r.returncode != 0 and "needs --encoding model" in r.stderr


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "real":
        sys.exit(real(sys.argv[2:]))
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    counts, fails = run_many(n, seed)
    for k, v in sorted(counts.items()):
        print(f"{v:8}  {k}")
    print(f"failures: {len(fails)}")
    for f in fails[:10]:
        print("  ", str(f)[:3000])
    sys.exit(1 if fails else 0)
