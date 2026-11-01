"""pairs_check.py DUMP [--pair A,B] [--show V@R ...]: A6(a)'s check on a reading. Relaxed reachability (h1: every fact,
deletes ignored) from the dump's start, then h2 restricted to the pairs of facts A and B (default var:32, the verb
script, and room), every other pair taken as reachable (Haslum & Geffner 2000). DUMP is `taskcore dump --fold off
--reduce none` of a packed file. Prints the reached pairs, whether each --show pair is reached, and the actions h1
keeps whose (A, B) precondition no reached pair meets: a plan from the start uses none of them.

A pair (a, b) is reached when the start holds it, or when an action that h1 keeps, applied in a state with a reached
pair its precondition allows, writes a to A and b to B, or writes one of them and leaves the other. A conditional
effect also may not fire; a guess writes any value of the domain. h2 never calls a reachable pair unreachable, and
taking the other pairs as reachable only relaxes it further."""
import json, sys
from collections import Counter, defaultdict

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
                     [(e[0], e[1], set(e[2]), {k: set(s) for k, s in e[3]}) for e in x["eff"]]))
    R = {k: ({v[1]} if v[1] is not None else set()) for k, v in facts.items()}
    meets = lambda sets: all(R.get(k, set()) & s for k, s in sets.items())
    changed = True
    while changed:
        changed = False
        for _, pre, eff in acts:
            if not meets(pre):
                continue
            for k, kind, vals, when in eff:
                if meets(when):
                    new = (set(facts[k][0]) if kind == 1 else vals) - R[k]
                    if new:
                        R[k] |= new
                        changed = True
    dA, dB = set(facts[A][0]), set(facts[B][0])
    P = {(facts[A][1], facts[B][1])}
    changed = True
    while changed:
        changed = False
        for _, pre, eff in acts:
            if not meets(pre):
                continue
            SA, SB = pre.get(A, dA), pre.get(B, dB)
            W = {(a, b) for (a, b) in P if a in SA and b in SB}
            if not W:
                continue
            eA = [(kind, vals, when) for k, kind, vals, when in eff if k == A and meets(when)] or [None]
            eB = [(kind, vals, when) for k, kind, vals, when in eff if k == B and meets(when)] or [None]
            new = set()
            for ea in eA:
                for eb in eB:
                    for (a, b) in W:
                        ok = all(not (e and e[2].get(F) is not None and v not in e[2][F])
                                 for e in (ea, eb) for F, v in ((A, a), (B, b)))
                        if not ok:
                            continue
                        va = (dA if ea[0] == 1 else ea[1]) if ea else {a}
                        vb = (dB if eb[0] == 1 else eb[1]) if eb else {b}
                        new |= {(x, y) for x in va for y in vb}
            new -= P
            if new:
                P |= new
                changed = True
    print(f"actions {len(acts)}; {A} reached {sorted(R[A], key=str)}; {B} reached {len(R[B])} values")
    print(f"pairs reached {len(P)} of {len(R[A]) * len(R[B])}")
    for a, b in show:
        print(f"({A} = {a}, {B} = {b}): {'reached' if (a, b) in P else 'not reached'}")
    dead = Counter()
    for lab, pre, eff in acts:
        if meets(pre) and not any(a in pre.get(A, dA) and b in pre.get(B, dB) for (a, b) in P):
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
