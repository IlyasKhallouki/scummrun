"""E003 step 4: relevance, working backward from the goal (plan step 4).

usage: from relevance import relevant
       kept = relevant(task)          # Relevance: the actions kept, the facts and values they need, counts
       task.Task(task.facts, [task.actions[i] for i in kept.actions], task.goal)

An action is kept when one of its effects can write a **needed** value: a goal value, a value a kept action's
precondition admits, or any value of a fact that is needed **whole**. It removes nothing that could matter,
so it is not a cap (plan step 4). The rules, on task.py's actions (so an action's variants stay together):

* a value an effect can write: ``value``; each value of ``values`` (a guess); for ``add``, ``Domain.add`` of
  every value its conditions admit;
* a kept action's precondition values are needed;
* a kept action's effect on a fact with a needed value, or a needed whole, makes every fact its condition
  reads needed whole, and so does an ``add`` its own fact (the result depends on the value before): removing
  an action that writes such a fact, even to a value no condition admits, could make that effect happen
  where it did not, or not where it did. An effect on a fact nothing needs is left alone. Plain
  backward relevance over values alone misses this (test_task.py has the case);
* the fixpoint runs until no action, value or whole fact is added.

Without conditional effects this is the usual backward relevance over (fact, value) pairs: in a plan,
an action whose writes are all unneeded can be dropped, since no later precondition or goal reads the
values it wrote before another write replaces them.
"""

import collections
from dataclasses import dataclass, field

from task import Task


@dataclass
class Relevance:
    actions: list  # indices of the kept actions, in task order
    values: dict  # fact -> set of needed values
    whole: set  # facts needed whole
    counts: collections.Counter = field(default_factory=collections.Counter)

    @property
    def facts(self) -> set:
        return set(self.values) | self.whole


def _writes(task: Task, e) -> set:
    """The values effect ``e`` can write (module docstring)."""
    dom = task.facts[e.key]
    if e.values is not None:
        return {dom.label(v) if isinstance(v, int) else v for v in e.values}
    if e.add is not None:
        own = dict(e.when).get(e.key)
        return set().union(*(dom.add(u, e.add) for u in dom.values if own is None or u in own))
    return {dom.label(e.value) if isinstance(e.value, int) else e.value}


def relevant(task: Task) -> Relevance:
    needed = collections.defaultdict(set)
    whole = set()
    for k, s in task.goal.items():
        dom = task.facts[k]
        needed[k] |= {dom.label(v) if isinstance(v, int) else v for v in s}
    writes = [[(e, _writes(task, e)) for e in a.effects] for a in task.actions]
    kept = set()
    rounds = 0
    changed = True
    while changed:
        changed = False
        rounds += 1
        for i, a in enumerate(task.actions):
            if i not in kept:
                if not any(e.key in whole or vals & needed.get(e.key, set()) for e, vals in writes[i]):
                    continue
                kept.add(i)
                changed = True
                for k, s in a.pre.items():
                    dom = task.facts[k]
                    new = {dom.label(v) if isinstance(v, int) else v for v in s} - needed[k]
                    if new:
                        needed[k] |= new
            for e, _ in writes[i]:  # conditions of effects on needed facts (rescanned as needs grow)
                if e.key in whole or needed.get(e.key):
                    reads = {k for k, _ in e.when} | ({e.key} if e.add is not None else set())
                    if reads - whole:
                        whole |= reads
                        changed = True
    rel = Relevance(sorted(kept), {k: v for k, v in needed.items() if v}, whole)
    mentioned = set(task.goal)
    for a in task.actions:
        mentioned |= set(a.pre) | {e.key for e in a.effects} | {k for e in a.effects for k, _ in e.when}
    rel.counts.update({"actions": len(task.actions), "actions kept": len(kept), "facts named": len(mentioned),
                       "facts needed": len(rel.facts), "facts needed whole": len(whole),
                       "values needed": sum(len(v) for k, v in rel.values.items() if k not in whole),
                       "rounds": rounds})
    return rel


def restrict(task: Task, rel: Relevance) -> Task:
    """The task with only the kept actions."""
    return Task(task.facts, [task.actions[i] for i in rel.actions], task.goal)
