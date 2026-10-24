"""E003 step 4: the planner's task, written as Fast Downward's finite-domain input (plan step 4).

usage: from task import Task, Action, Effect, write_sas
       sas = write_sas(Task(facts={key: Domain}, actions=[...], goal={key: {values}}))
       sas.text(start)          # the whole file, with ``start`` (a facts.py map) as the initial state
       sas.state(start)         # only the begin_state ... end_state section (a replan changes nothing else)

**The task.** Facts are facts.py keys, each with a finite ``domains.Domain``. An ``Action`` has a name, the
caller's own description (``what``, opaque here), a precondition ``pre`` ({key: set of values}: a comparison
is a set, domains.Domain.test) and ``effects``. An ``Effect`` writes one key, under ``when`` ({key: set of
values}, read in the state before the action, as every condition is), in one of three ways: ``value`` (a
known value), ``add`` (the key plus a constant: per current value ``Domain.add``) or ``values`` (a write of a
value the reader cannot work out: **one effect per possible value**, plan step 4). Two effects of one action
on one key must not both happen with different results; the writer checks it and raises.

**The encoding** (Fast Downward's translator output format, version 3, docs/translator-output-format.md):

* every key a condition, effect or goal names is a variable; its values are named ``Atom kind(key=value)``,
  because the RHW landmark factory reads a predicate from each value's name and stops on any other form
  (landmark_factory_rpg_sasp.cc:359-376); the names hold no quote, `` & `` or `` | ``, which the landmark dump
  uses (landmarks/util.cc:82-107);
* a precondition set of more than one value cannot be a prevail condition (one value per variable), and
  the plan's "one effect per possible value" cannot be one operator (two effects on one variable): both
  become **variants**, one operator per choice of a precondition value and a guessed value, the product
  over the action's sets and guesses. Each variant records its choices (``Op``), so the plan says which
  values it guessed; at replay a mismatch on a guessed value leads to a replan, not a ban. A set in an
  effect condition stays inside one operator: one conditional effect per value;
* when that product passes ``PRODUCT_LIMIT``, the action's guesses are written through **pick variables**
  instead, so the size stays the sum, not the product: ``pick:<key>`` has the key's values, a pick
  operator sets it to any value at any time, and the action writes ``key := u`` when ``pick:<key> = u``,
  for each value ``u`` (a ``u`` the guess cannot write gives its first value). It reaches the same states;
  the plan parser folds the pick operators into the next step that reads them (``guessed``) and leaves them
  out. One pick variable per guessed key serves every action. Counted (``SasTask.counts``). A product of
  precondition sets alone past ``PRE_LIMIT`` raises;
* ``add`` is one conditional effect per current value (``when key = u: key := u + d``); where ``u + d`` is
  several values (``ABOVE`` minus a step) it is a guess under that condition;
* every precondition is a prevail condition and every effect's own precondition is -1, so the parser never
  sees a variable twice (root_task.cc:402-430 adds an effect's precondition to the operator's);
* no axioms (the landmark sum heuristic is not safe with them, landmark_sum_heuristic.cc:18-30), no mutex
  groups, metric 0 unless an action costs other than 1;
* an operator with no effect left is dropped, since the parser rejects it (root_task.cc:452-458), and so is
  an action whose precondition no value meets; both counted (``SasTask.counts``);
* a goal of a set of values: an extra variable ``goal`` and one operator per choice that sets it; the plan
  parser leaves those steps out (``SasTask.ops[...].goal``).

A replan changes only the initial state, so the task is translated once: ``SasTask`` keeps the sections
before and after the state apart, and ``text`` joins them around a new one.
"""

import collections
import itertools
from dataclasses import dataclass, field

from domains import Domain

GOAL_KEY = "goal"  # the extra variable for set goals
PICK = "pick:"  # pick variables (module docstring)
PRODUCT_LIMIT = 64  # variants of one action past which its guesses go through pick variables
PRE_LIMIT = 4096  # variants of one action from precondition sets alone past which the writer raises


@dataclass(frozen=True)
class Effect:
    key: str
    value: object = None  # a known value
    add: int | None = None  # the key plus this constant
    values: frozenset | None = None  # a write of a value the reader cannot work out: one of these
    when: tuple = ()  # ((key, frozenset of values), ...): all hold before the action


@dataclass
class Action:
    name: str
    pre: dict = field(default_factory=dict)  # key -> set of values
    effects: tuple = ()
    what: object = None  # the caller's action (actions.Action, a path id ...)
    cost: int = 1


@dataclass
class Task:
    facts: dict  # key -> Domain
    actions: list
    goal: dict  # key -> set of values


@dataclass(frozen=True)
class Op:
    """One operator in the file: an action's variant."""
    name: str  # a<N>, its index
    action: int | None  # index into Task.actions; None for a goal operator
    chosen: tuple = ()  # ((key, value), ...) the precondition values this variant takes
    guessed: tuple = ()  # ((key, value, when), ...) the guessed values it writes, and under which condition
    picked: tuple = ()  # ((key, values in order, when), ...): guesses written through pick variables
    goal: bool = False
    pick: tuple | None = None  # a pick operator: (key, value)


class OutOfDomain(ValueError):
    """A state value the task's domains do not hold: the task must be built again with that value."""

    def __init__(self, misses: list):
        super().__init__(f"{len(misses)} facts out of their domains: {misses[:10]}")
        self.misses = misses


def atom(key: str, value) -> str:
    kind = "".join(ch for ch in key.split(":")[0] if ch.isalnum()) or "fact"
    return f"Atom {kind}({key}={value})"


@dataclass
class SasTask:
    head: str  # version, metric, variables, mutexes
    tail: str  # goal, operators, axioms
    keys: list  # variable index -> key
    domains: list  # variable index -> tuple of values
    ops: list  # Op per operator, in file order
    counts: collections.Counter
    task: Task
    facts: dict  # the task's facts and the pick variables
    def state(self, start: dict) -> str:
        """The initial-state section for ``start`` (a facts.py map, absent is 0). Raises OutOfDomain."""
        vals, misses = [], []
        for key, values in zip(self.keys, self.domains):
            if key == GOAL_KEY or key.startswith(PICK):
                vals.append(0)
                continue
            lab = self.task.facts[key].label(start.get(key, 0))
            if lab is None or lab not in values:
                misses.append((key, start.get(key, 0)))
                continue
            vals.append(values.index(lab))
        if misses:
            raise OutOfDomain(misses)
        return "begin_state\n" + "".join(f"{v}\n" for v in vals) + "end_state\n"

    def text(self, start: dict) -> str:
        return self.head + self.state(start) + self.tail

    def op(self, name: str) -> Op:
        return self.ops[int(name.strip("()").split()[0][1:])]

    def op_table(self) -> list:
        """The operators as JSON-ready rows, to keep next to the file: a replay in another process reads the
        plan's ``a<N>`` names back through it."""
        def conds(t):
            return [[k, sorted(map(str, s))] for k, s in t]
        return [{"op": o.name, "action": o.action,
                 "name": self.task.actions[o.action].name if o.action is not None else None,
                 "chosen": [[k, str(v)] for k, v in o.chosen],
                 "guessed": [[k, str(v), conds(w)] for k, v, w in o.guessed],
                 "picked": [[k, [str(v) for v in vals], conds(w)] for k, vals, w in o.picked],
                 "goal": o.goal, "pick": [o.pick[0], str(o.pick[1])] if o.pick else None} for o in self.ops]


def _fixed(conds: tuple, fixed: dict) -> tuple | None:
    """``conds`` under the precondition values ``fixed``: None when one contradicts them, else the rest."""
    rest = []
    for k, s in conds:
        if k in fixed:
            if fixed[k] not in s:
                return None
        else:
            rest.append((k, s))
    return tuple(rest)


def _expand(task: Task, a: Action, counts, facts: dict) -> list:
    """The variants of one action: [(chosen, guessed, picked, effects)], effects as (conds of single values, key,
    value). ``facts`` gets the pick variables the action uses."""
    pre = {}
    for k, s in a.pre.items():
        s = frozenset(s) & frozenset(task.facts[k].values)
        if not s:
            counts["actions whose precondition no value meets (dropped)"] += 1
            return []
        if len(s) == len(task.facts[k].values):
            counts["precondition sets that hold for every value (left out)"] += 1
            continue
        pre[k] = s

    def ordered(k, s):  # a set's values in the domain's order, so the file is the same every run
        return [v for v in facts[k].values if v in s]

    def when_of(when):  # a condition's sets cut to the domain; a set that holds for every value left out
        out = []
        for k, s in when:
            s = frozenset(s) & frozenset(task.facts[k].values)
            if len(s) < len(task.facts[k].values):
                out.append((k, s))
        return tuple(out)
    # steps become conditional sets, or guesses where the step is several values
    effects = []  # (when, key, kind, payload): kind "set" (value) or "guess" (frozenset)
    for e in a.effects:
        dom = task.facts[e.key]
        when = when_of(e.when)
        if e.values is not None:
            vals = frozenset(dom.label(v) if isinstance(v, int) else v for v in e.values) & frozenset(dom.values)
            if vals != frozenset(dom.label(v) if isinstance(v, int) else v for v in e.values):
                raise ValueError(f"{a.name}: {e.key} := one of {sorted(e.values, key=str)}: not all in {dom.values}")
            effects.append((when, e.key, "guess", vals))
        elif e.add is not None:
            own = dict(when).get(e.key)
            rest = tuple((k, s) for k, s in when if k != e.key)
            for u in dom.values:
                if own is not None and u not in own:
                    continue
                nxt = dom.add(u, e.add)
                if len(nxt) == 1:
                    effects.append((rest + ((e.key, frozenset({u})),), e.key, "set", next(iter(nxt))))
                elif nxt:
                    counts["steps whose result is several values (guesses)"] += 1
                    effects.append((rest + ((e.key, frozenset({u})),), e.key, "guess", nxt))
                else:
                    raise ValueError(f"{a.name}: {e.key} {u} + {e.add} is not in its domain {dom.values}")
        else:
            value = dom.label(e.value) if isinstance(e.value, int) else e.value
            if value not in dom.values:
                raise ValueError(f"{a.name}: {e.key} := {e.value} is not in its domain {dom.values}")
            effects.append((when, e.key, "set", value))
    pre_product = 1
    for s in pre.values():
        pre_product *= len(s)
    if pre_product > PRE_LIMIT:
        raise ValueError(f"{a.name}: {pre_product} variants from precondition sets alone (PRE_LIMIT {PRE_LIMIT})")
    guess_product = 1
    for _, _, kind, payload in effects:
        guess_product *= len(payload) if kind == "guess" else 1
    picked = ()
    if pre_product * guess_product > PRODUCT_LIMIT:
        counts["actions whose guesses go through pick variables"] += 1
        sets, picked = [], []
        for when, key, kind, payload in effects:
            if kind != "guess":
                sets.append((when, key, kind, payload))
                continue
            pk = PICK + key
            facts.setdefault(pk, Domain(facts[key].values))
            vals = ordered(key, payload)
            picked.append((key, tuple(vals), when))
            for u in facts[key].values:
                sets.append((when + ((pk, frozenset({u})),), key, "set", u if u in payload else vals[0]))
        effects, picked = sets, tuple(picked)
    variants = []
    pre_keys = sorted(pre)
    for choice in itertools.product(*(ordered(k, pre[k]) for k in pre_keys)):
        fixed = dict(zip(pre_keys, choice))
        live = []
        for when, key, kind, payload in effects:
            w = _fixed(when, fixed)
            if w is not None:
                live.append((w, key, kind, payload))
        guesses = [(i, x) for i, x in enumerate(live) if x[2] == "guess"]
        for picks in itertools.product(*(ordered(x[1], x[3]) for _, x in guesses)):
            chosen_g = dict(zip((i for i, _ in guesses), picks))
            out = []
            for i, (w, key, kind, payload) in enumerate(live):
                value = chosen_g[i] if kind == "guess" else payload
                keys = [k for k, _ in w]
                for vals in itertools.product(*(ordered(k, s) for k, s in w)):
                    conds = tuple(zip(keys, vals))
                    if dict(conds).get(key, fixed.get(key)) == value:
                        counts["effects that write the value the key already has (left out)"] += 1
                        continue
                    out.append((conds, key, value))
            guessed = tuple((live[i][1], v, live[i][0]) for i, v in chosen_g.items())
            variants.append((tuple(sorted(fixed.items())), guessed, picked, out))
    counts["variants"] += len(variants)
    counts["max variants of one action"] = max(counts["max variants of one action"], len(variants))
    return variants


def _check(name: str, effects: list) -> None:
    """Two effects on one key that can both happen must write the same value (module docstring)."""
    by_key = collections.defaultdict(list)
    for conds, key, value in effects:
        by_key[key].append((dict(conds), value))
    for key, es in by_key.items():
        for (c1, v1), (c2, v2) in itertools.combinations(es, 2):
            if v1 != v2 and all(c1[k] == c2[k] for k in c1.keys() & c2.keys()):
                raise ValueError(f"{name}: two effects on {key} can both happen: {v1} when {c1}, {v2} when {c2}")


def write_sas(task: Task) -> SasTask:
    counts = collections.Counter()
    facts = dict(task.facts)
    ops_out = []  # (Op, prevail [(key, value)], effects [(conds, key, value)], cost)
    for i, a in enumerate(task.actions):
        for chosen, guessed, picked, effects in _expand(task, a, counts, facts):
            effects = list(dict.fromkeys(effects))
            if not effects:
                counts["variants with no effect left (dropped)"] += 1
                continue
            _check(a.name, effects)
            ops_out.append((Op("", i, chosen, guessed, picked), list(chosen), effects, a.cost))
    for pk in sorted(k for k in facts if k.startswith(PICK)):
        for u in facts[pk].values:
            ops_out.append((Op("", None, pick=(pk[len(PICK):], u)), [], [((), pk, u)], 0))
            counts["pick operators"] += 1
    goal = {}
    for k, s in task.goal.items():
        dom = task.facts[k]
        vals = {dom.label(v) if isinstance(v, int) else v for v in s} - {None}
        goal[k] = [v for v in dom.values if v in vals]
        if not goal[k]:
            raise ValueError(f"goal {k} in {sorted(s, key=str)}: no value of its domain {dom.values}")
    goal_single = {k: vs[0] for k, vs in goal.items() if len(vs) == 1}
    goal_sets = {k: vs for k, vs in goal.items() if len(vs) > 1}
    keys = set(task.goal)
    for _, prevail, effects, _ in ops_out:
        keys.update(k for k, _ in prevail)
        for conds, k, _ in effects:
            keys.add(k)
            keys.update(c for c, _ in conds)
    keys = sorted(keys)
    domains = [tuple(facts[k].values) for k in keys]
    if goal_sets:
        keys.append(GOAL_KEY)
        domains.append((0, 1))
        for choice in itertools.product(*goal_sets.values()):
            ops_out.append((Op("", None, tuple(zip(goal_sets, choice)), goal=True), list(zip(goal_sets, choice)),
                            [((), GOAL_KEY, 1)], 0))
        goal_single = {k: v for k, v in goal_single.items()} | {GOAL_KEY: 1}
    index = {k: n for n, k in enumerate(keys)}

    def fact(k, v):
        return f"{index[k]} {domains[index[k]].index(v)}"

    metric = int(any(cost != 1 for _, _, _, cost in ops_out if cost))
    head = ["begin_version", "3", "end_version", "begin_metric", str(metric), "end_metric", str(len(keys))]
    for k, vals in zip(keys, domains):
        head += ["begin_variable", k, "-1", str(len(vals))] + [atom(k, v) for v in vals] + ["end_variable"]
    head += ["0"]
    tail = ["begin_goal", str(len(goal_single))] + [fact(k, v) for k, v in goal_single.items()] + ["end_goal"]
    tail.append(str(len(ops_out)))
    ops = []
    for n, (op, prevail, effects, cost) in enumerate(ops_out):
        op = Op(f"a{n}", op.action, op.chosen, op.guessed, op.picked, op.goal, op.pick)
        ops.append(op)
        prevail = sorted(set(prevail))
        tail += ["begin_operator", op.name, str(len(prevail))] + [fact(k, v) for k, v in prevail]
        tail.append(str(len(effects)))
        for conds, k, v in effects:
            tail.append(" ".join([str(len(conds))] + [fact(c, cv) for c, cv in conds] +
                                 [str(index[k]), "-1", str(domains[index[k]].index(v))]))
        tail += [str(cost), "end_operator"]
        counts["conditional effects"] += sum(1 for c, _, _ in effects if c)
        counts["effects"] += len(effects)
    tail.append("0")  # no axioms
    counts["actions"] = len(task.actions)
    counts["operators"] = len(ops)
    counts["variables"] = len(keys)
    counts["values"] = sum(len(d) for d in domains)
    counts["pick variables"] = sum(1 for k in keys if k.startswith(PICK))
    return SasTask("\n".join(head) + "\n", "\n".join(tail) + "\n", keys, domains, ops, counts, task, facts)
