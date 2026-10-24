"""E003 step 4: a coarse fixture task from the IR, to measure relevance and the writer at the game's scale.

usage: uv run python experiments/E003-path-through-cloud/task_fixture.py   (writes results/task-fixture.log)

An instrument, not the system's task. The system's actions (plan step 1) need the reader's symbolic
oracle, which is not built (model.py's docstring), so this task is **hand-made from the IR, flow-insensitive**:

* one action per object verb entry (ir.Unit.entries; 0xFF defaults included), standing for the sentence
  that reaches it;
* its reach: the instructions ``domains.live`` reaches from the entry (the run's settings decide their jumps),
  and the whole live code of every unit it starts with a constant number (startScript, chainScript,
  startObject), transitively;
* its effects: every write in the reach to a fact with a domain (domains.py): a variable or bit written
  with constants (a guess over them when more than one: the "one effect per possible value" rule; one step
  is an ``add``; constants and steps mixed, or two steps, a guess over the whole domain, counted),
  setOwnerOf, pickupObject (owner VAR_EGO's values, state 1, class Untouchable set: script_v5.cpp:2028-2030),
  setState, drawObject, setClass, loadRoom and loadRoomWithEgo (the room). A write whose target or value
  does not fold is counted and left out;
* its precondition: every fact the reach reads (a variable or bit in a condition, getObjectOwner,
  getObjectState, classOfIs, isScriptRunning, and the room), each with **every** value of its domain. So
  the precondition says only "depends on", which is what relevance needs; the writer drops such sets;
* the goal: the Part I end, the E001 key's goals #97-99 (owner 901, 902 and 903 = 1), a disclosed human
  input used only for Part I tuning (plan.md step 2 and C2).

So relevance here keeps every entry linked to the goal through some write-read chain: an upper bound on what
it keeps from the real task, which has preconditions on values. No search runs on this task. Fast Downward
reads the kept part once with the goal already true at the start (it stops before searching), to check the
file at this size.
"""

import collections
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import domains as D  # noqa: E402
import ir  # noqa: E402
import plan_fd  # noqa: E402
import relevance as R  # noqa: E402
from ir import Assign, Bit, Const, Getter, Index, Jump, List, Op, Start, Var  # noqa: E402
from task import Action, Effect, Task, write_sas  # noqa: E402

PART1_END = {"owner:901": {1}, "owner:902": {1}, "owner:903": {1}}  # E001 key #97-99 (plan step 2, C2 exception)
CLASS_UNTOUCHABLE = 32  # kObjectClassUntouchable, object.h:54


def reach(units, settings, key, starts, rd) -> list:
    """(unit key, instruction) pairs the entry can reach, through constant starts (module docstring)."""
    out, seen, todo = [], set(), [(key, tuple(starts))]
    while todo:
        k, st = todo.pop()
        if (k, st) in seen or k not in units:
            continue
        seen.add((k, st))
        for x in D.live(units[k], settings, list(st) if st is not None else None):
            out.append((k, x))
            s = x.st
            if isinstance(s, Start):
                if s.name == "startObject":
                    o = s.obj.value if isinstance(s.obj, Const) else None
                    if o is not None and ("obj", o) in units:
                        todo.append((("obj", o), None))
                elif isinstance(s.script, Const):
                    for t in rd.targets(k, s.script.value):
                        todo.append((t, (0,)))
    return out


def build(doms) -> tuple[Task, collections.Counter]:
    units = ir.load()
    settings = {n: frozenset({v}) for n, v in D.SETTINGS.items()}
    rd = D._Reader(units, {}, collections.Counter(), [])
    counts = collections.Counter()
    ego = doms.get("ego")
    facts, actions = {}, []

    def dom(key):
        d = doms.get(key)
        if d is not None:
            facts[key] = d
        return d

    for okey, u in sorted(units.items()):
        if okey[0] != "obj":
            continue
        for verb, entry in sorted(u.entries.items()):
            reads, consts, steps = set(), collections.defaultdict(set), collections.defaultdict(list)
            for k, x in reach(units, settings, okey, [entry] if entry is not None else None, rd):
                s = x.st
                if isinstance(s, Jump):
                    if s.cond is None:
                        continue
                    for y in D._walk(s.cond):
                        if isinstance(y, Var):
                            reads.add(f"var:{y.n}")
                        elif isinstance(y, Bit):
                            reads.add(f"bit:{y.n}")
                for y in D._walk(s):
                    if isinstance(y, Getter) and y.args and isinstance(y.args[0], Const):
                        o = y.args[0].value
                        if y.name == "getObjectOwner":
                            reads.add(f"owner:{o}")
                        elif y.name == "getObjectState":
                            reads.add(f"state:{o}")
                        elif y.name == "isScriptRunning":
                            reads.add(f"script:{o}")
                        elif y.name == "classOfIs" and isinstance(y.args[1], List):
                            reads.update(f"class:{o}:{c.value & 0x7F}" for c in y.args[1].items if isinstance(c, Const))
                if isinstance(s, Assign) and isinstance(s.lhs, (Var, Bit)):
                    key = f"{'var' if isinstance(s.lhs, Var) else 'bit'}:{s.lhs.n}"
                    v = D.fold(s.rhs, {})
                    if s.op == "=" and v is not None:
                        consts[key] |= v
                    elif s.op in ("+=", "-=", "++", "--") and v is not None and len(v) == 1:
                        steps[key].append(next(iter(v)) * (1 if s.op in ("+=", "++") else -1))
                    else:
                        counts["writes whose value does not fold (left out)"] += 1
                elif isinstance(s, Assign) and isinstance(s.lhs, Index):
                    counts["indexed writes (left out)"] += 1
                elif isinstance(s, Op):
                    a = s.args
                    o = a[0].value if a and isinstance(a[0], Const) else None
                    if s.name == "setOwnerOf":
                        if o is None or not isinstance(a[1], Const):
                            counts["owner, state, class or room writes that do not fold (left out)"] += 1
                        else:
                            consts[f"owner:{o}"].add(a[1].value)
                    elif s.name == "pickupObject":
                        if o is None or ego is None:
                            counts["owner, state, class or room writes that do not fold (left out)"] += 1
                        else:
                            consts[f"owner:{o}"] |= set(ego.values)
                            consts[f"state:{o}"].add(1)
                            consts[f"class:{o}:{CLASS_UNTOUCHABLE}"].add(1)
                    elif s.name in ("setState", "drawObject"):
                        img = [x for x in s.subops if x.name == "setImage"]
                        val = (a[1] if s.name == "setState" else img[0].args[0] if img else Const(1))
                        if o is None or not isinstance(val, Const):
                            counts["owner, state, class or room writes that do not fold (left out)"] += 1
                        else:
                            consts[f"state:{o}"].add(val.value)
                    elif s.name == "setClass":
                        if o is None or not isinstance(a[1], List):
                            counts["owner, state, class or room writes that do not fold (left out)"] += 1
                        else:
                            for c in a[1].items:
                                if isinstance(c, Const) and c.value & 0x7F:
                                    consts[f"class:{o}:{c.value & 0x7F}"].add(1 if c.value & 0x80 else 0)
                                else:
                                    counts["setClass of class 0 or not a constant (left out)"] += 1
                    elif s.name in ("loadRoom", "loadRoomWithEgo"):
                        r = a[0] if s.name == "loadRoom" else a[1]
                        if isinstance(r, Const):
                            consts["room"].add(r.value)
                        else:
                            counts["owner, state, class or room writes that do not fold (left out)"] += 1
            reads.add("room")
            effects = []
            for key in sorted(set(consts) | set(steps)):
                d = dom(key)
                if d is None:
                    counts["writes to a fact with no domain (left out)"] += 1
                    continue
                vals = {d.label(v) for v in consts.get(key, ())}
                if None in vals:
                    counts["written values outside the domain (left out)"] += 1
                    vals.discard(None)
                if key in steps and (vals or len(set(steps[key])) > 1):
                    counts["facts written with constants and steps, or two steps (a guess over the domain)"] += 1
                    effects.append(Effect(key, values=frozenset(d.values)))
                elif key in steps:
                    effects.append(Effect(key, add=steps[key][0]))
                elif len(vals) == 1:
                    effects.append(Effect(key, next(iter(vals))))
                elif vals:
                    effects.append(Effect(key, values=frozenset(vals)))
            pre = {}
            for key in sorted(reads):
                d = dom(key)
                if d is None:
                    counts["reads of a fact with no domain (not a precondition)"] += 1
                else:
                    pre[key] = set(d.values)
            actions.append(Action(f"entry {okey[1]} verb {verb}", pre, tuple(effects), what=(okey[1], verb)))
    for key in PART1_END:
        dom(key)
    counts["object verb entries (actions)"] = len(actions)
    counts["actions with no effect on a fact"] = sum(1 for a in actions if not a.effects)
    return Task(facts, actions, PART1_END), counts


def main() -> None:
    t0 = time.monotonic()
    doms = D.build({}, goals=PART1_END)
    task, counts = build(doms)
    t_build = time.monotonic() - t0
    rel = R.relevant(task)
    kept = R.restrict(task, rel)
    t1 = time.monotonic()
    sas = write_sas(kept)
    t_write = time.monotonic() - t1
    start = dict.fromkeys(PART1_END, 1)  # the goal true at the start: FD reads the file and stops
    text = sas.text(start)
    code, log, plan_text, seconds, work = plan_fd.run(text, time_limit=600)
    fd_lines = [ln for ln in log.splitlines() if any(w in ln for w in (
        "reading input", "done reading", "Variables:", "FactPairs:", "Bytes per state", "Landmark graph contains",
        "Solution found", "Plan length", "Peak memory", "search exit code", "Search time", "Total time"))]
    lines = ["E003 step 4: the coarse fixture (task_fixture.py's docstring: hand-made from the IR, flow-insensitive;",
             "no real actions exist yet, since the model has no symbolic oracle)", "",
             "start: the engine's init (absent is 0); goal: owner 901, 902, 903 = 1 (E001 key #97-99, the Part I end)",
             f"built in {t_build:.1f} s", ""]
    lines += ["fixture (a write or read site counts once per action whose reach holds it):"]
    lines += [f"  {n:7}  {w}" for w, n in sorted(counts.items())]
    lines += [f"  {len(task.facts):7}  facts named (with a domain)"]
    lines += ["", "relevance (relevance.py), backward from the goal:"]
    lines += [f"  {rel.counts[k]:7}  {k}" for k in ("actions", "actions kept", "facts named", "facts needed",
                                                     "facts needed whole", "values needed", "rounds")]
    by_kind = collections.Counter(k.split(":")[0] for k in rel.facts)
    lines += ["  facts needed by kind: " + ", ".join(f"{k} {n}" for k, n in sorted(by_kind.items()))]
    lines += ["", f"SAS file of the kept actions (task.py), written in {t_write:.1f} s, {len(text) / 1e6:.1f} MB:"]
    lines += [f"  {n:7}  {w}" for w, n in sorted(sas.counts.items())]
    lines += ["", f"Fast Downward on it with the goal true at the start (no search): exit {code}, {seconds:.1f} s"]
    lines += [f"  {ln.strip()}" for ln in fd_lines]
    out = HERE / "results" / "task-fixture.log"
    out.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
