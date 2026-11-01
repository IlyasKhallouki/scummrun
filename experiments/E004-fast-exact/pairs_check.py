"""pairs_check.py DUMP [--pair A,B] [--show V@R ...]: A6(a)'s check on a reading. Relaxed reachability (h1: every fact,
deletes ignored, taskcore's (d)) from the dump's start, then h2 restricted to the pairs of facts A and B (default var:32,
the verb script, and room), every other pair taken as reachable (Haslum & Geffner 2000), by the rule of
reach-pairs.md. DUMP is `taskcore dump --fold off --reduce none` of a packed file. Prints the reached pairs, whether
each --show pair is reached, the conditional effects on A or B, and the actions h1 keeps whose (A, B) precondition no
reached pair meets: a plan from the start uses none of them.

A pair (a, b) is reached when the start holds it, or when an action that h1 keeps, applied at a reached pair its
precondition allows, writes a to A and b to B. On each of A and B the action leaves one of: the values of each effect
there that may fire (its conditions on A and B hold at the pair, its other conditions meet h1), and the old value
unless an effect there must fire (its conditions are on A and B only and hold at the pair). A guess writes one of its
values; a value the domain lacks (kind 2) writes nothing. h2 never calls a reachable pair unreachable, and taking the
other pairs as reachable only relaxes it further."""
import json, sys
from collections import Counter, defaultdict

K_SET, K_GUESS, K_BAD = 0, 1, 2


def main():
    args = sys.argv[1:]
    dump = args[0]
    A, B = "var:32", "room"
    show = []
    for i, a in enumerate(args):
        if a == "--pair":
            A, B = args[i + 1].split(",")
        if a == "--show":
            show.append(tuple(args[i + 1].split("@")))
    facts, acts = {}, []
    for line in open(dump):
        x = json.loads(line)
        if "fact" in x:
            facts[x["fact"]] = (x["values"], x["start"])
            continue
        acts.append((x["label"], {k: set(v) for k, v in x["pre"]},
                     [(e[0], e[1], set(e[2]), [(k, set(s)) for k, s in e[3]]) for e in x["eff"]]))
    # h1: taskcore's (d)
    R = {k: ({v[1]} if v[1] is not None else set()) for k, v in facts.items()}
    meets = lambda sets: all(R.get(k, set()) & s for k, s in sets)
    pre_met = lambda pre: meets(pre.items())
    changed = True
    while changed:
        changed = False
        for _, pre, eff in acts:
            if not pre_met(pre):
                continue
            for k, kind, vals, when in eff:
                if kind != K_BAD and meets(when):
                    new = vals - R[k]
                    if new:
                        R[k] |= new
                        changed = True
    dA, dB = set(facts[A][0]), set(facts[B][0])

    def split(when):
        ca = cb = None
        other = []
        for k, s in when:
            if k == A:
                ca = s if ca is None else ca & s
            elif k == B:
                cb = s if cb is None else cb & s
            else:
                other.append((k, s))
        return ca, cb, other

    # each kept action's effects on A and B, split by condition
    pa = []
    cond_eff = Counter()
    for lab, pre, eff in acts:
        if not pre_met(pre):
            continue
        es = []
        for k, kind, vals, when in eff:
            if k not in (A, B):
                continue
            ca, cb, other = split(when)
            if when:
                cond_eff[(k, "other" if other else "pair")] += 1
            es.append((k == A, ca, cb, meets(other), not other, kind, vals))
        if es:
            pa.append((pre.get(A), pre.get(B), es))

    def outcomes(es, on_a, x, y):
        own = x if on_a else y
        out, must = set(), False
        for ea, ca, cb, met, plain, kind, vals in es:
            if ea != on_a:
                continue
            if (ca is not None and x not in ca) or (cb is not None and y not in cb) or not met or kind == K_BAD or not vals:
                continue
            out |= vals
            must = must or plain
        if not must:
            out.add(own)
        return out

    P = {(facts[A][1], facts[B][1])}
    queue = list(P)
    while queue:
        x, y = queue.pop()
        for sa, sb, es in pa:
            if (sa is not None and x not in sa) or (sb is not None and y not in sb):
                continue
            for u in outcomes(es, True, x, y):
                for w in outcomes(es, False, x, y):
                    if (u, w) not in P:
                        P.add((u, w))
                        queue.append((u, w))
    print(f"actions {len(acts)}; {A} reached {sorted(R[A], key=str)}; {B} reached {len(R[B])} values")
    print(f"actions h1 keeps with an effect on {A} or {B}: {len(pa)}")
    for (k, what), n in sorted(cond_eff.items()):
        print(f"  conditional effects on {k} (kept actions), conditions {'on another fact too' if what == 'other' else f'on {A} or {B} only'}: {n}")
    print(f"pairs reached {len(P)} of {len(R[A]) * len(R[B])}")
    for a, b in show:
        print(f"({A} = {a}, {B} = {b}): {'reached' if (a, b) in P else 'not reached'}")
    dead = Counter()
    for lab, pre, eff in acts:
        if pre_met(pre) and not any(a in pre.get(A, dA) and b in pre.get(B, dB) for (a, b) in P):
            dead[lab.split("#")[0]] += 1
    print(f"actions h1 keeps with no reached ({A}, {B}) pair: {sum(dead.values())}")
    for lab, n in dead.most_common(15):
        print(f"  {n} {lab}")
    by = defaultdict(set)
    for a, b in P:
        by[b].add(a)
    for b in sorted(by, key=lambda x: (len(x), x)):
        print(f"  {B} = {b}: {A} in {sorted(by[b], key=lambda x: (len(x), x))}")


main()
