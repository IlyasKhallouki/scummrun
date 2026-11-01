"""E007's check of Lemma R (plan.md): a reading A and the same reading A2 with one more variable made a fact (reader
switch ``--fact-vars``), compared record by record (the readings' golden files: every record as made, before the packed
file folds equal ones).

* Each record of A2 must refine a record of A: the same source, label and slot ends; a precondition that on A's facts
  lies inside A's; A's checks less those on the new fact, plus at most sides of tests A took either way; nothing taken
  either way that A took one way; effects on A's facts that are A's or narrower (a set inside a guess, a guess inside a
  guess, a write A2's precondition already holds).
* Each record of A with a check on the new fact must be refined by a record of A2, or be infeasible: no value of the
  new fact's domain passes its checks on the fact together. Those checks are evaluated for the sentence script's
  records, whose locals 0 to 2 the engine binds to the sentence's verb and objects (script.cpp,
  checkAndRunSentenceScript); a record of another unit is counted as not evaluated.

usage (from experiments/E003-path-through-cloud, with ../../.venv/bin/python):
       ../E007-fact-refinement/lemma_check.py A/reading.json A2/reading.json --var N [--json OUT]

It names no game number: the variable is the argument, the sentence script is the source's.
"""

import argparse
import collections
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path.cwd()))
import packed_reading as PR  # noqa: E402

_COND = re.compile(r"^(!?)Var\[(\d+)\](?: (==|!=|<|<=|>|>=) (Local\[(\d+)\]|-?\d+))?$")


def golden_of(reading: Path) -> Path:
    doc = json.loads(reading.read_text())
    return Path(doc["from_golden"]["golden"])


def records(reading: Path) -> list:
    it = PR._golden_lines(golden_of(reading))
    next(it)  # the head
    return [PR._dec(ln["record"]) for ln in it if ln.get("kind") == "record"]


def on_var(check, n: int) -> bool:
    """A check A made on the new fact: its reason names it as a non-fact (reader.py's ``?(not a fact: var:N)``)."""
    return f"not a fact: var:{n})" in str(check[3]) if len(check) > 3 else False


def shape(r) -> tuple:
    ends = frozenset((k, e[1]) for k, e in r["effects"].items() if k.startswith("pos:") and e[0] == "set")
    return json.dumps(r["source"], sort_keys=True, default=str), ends, r["label"]


def why_not(r2, r, n: int) -> str | None:
    """None when record ``r2`` of A2 refines record ``r`` of A (module docstring); MERGED when it does but for checks
    of ``r`` that its path lacks while ``r`` took some test either way (the reader merged paths with equal effects into
    ``r`` and kept one path's checks, which the other path never runs); else the first reason it does not."""
    key = f"var:{n}"
    merged = False
    for k, vs in r["pre"].items():
        got = r2["pre"].get(k)
        if got is None:
            return "pre: a key of A's missing"
        if not set(got) <= set(vs):
            return "pre: wider than A's"
    either = {tuple(e[:2]) for e in r.get("either", ())}
    mine = {tuple(c[:3]) for c in r2["checks"]}
    theirs = {tuple(c[:3]) for c in r["checks"] if not on_var(c, n)}
    if not theirs <= mine:
        if not either:
            return "checks: one of A's missing"
        merged = True
    if any(c[:2] not in either for c in mine - theirs):
        return "checks: a side A did not take"
    if any(tuple(e[:2]) not in either for e in r2.get("either", ())):
        return "either: a test A took one way"
    e2s, es = r2["effects"], r["effects"]
    for k, e2 in e2s.items():
        if k == key or k.startswith("args:"):
            continue
        e = es.get(k)
        if e == e2:
            continue
        if e is None:
            return "effect: a key A does not write"
        if e2[0] == "set" and e[0] == "guess" and e2[1] in e[1]:
            continue
        if e2[0] == "guess" and e[0] == "guess" and set(e2[1]) <= set(e[1]):
            continue
        return f"effect: {e2[0]} against A's {e[0]}"
    for k, e in es.items():
        if k == key or k in e2s or k.startswith("args:"):
            continue
        if e[0] == "set" and set(r2["pre"].get(k, ())) == {e[1]}:
            continue  # the write keeps the value A2's precondition gives the fact (the reader's SAME)
        return "effect: an A write A2 lacks"
    return MERGED if merged else None


MERGED = "merged"


def feasible(r, n: int, dom: list) -> bool | None:
    """Whether some value of ``dom`` passes every check of ``r`` on var n, with the sentence's verb and objects as
    the sentence script's locals 0-2; None when the record is not the sentence script's or a check does not parse."""
    src = r["source"]
    if src[0] != "sentence":
        return None
    loc = {0: src[1], 1: src[2], 2: src[3] or 0}
    tests = []
    for c in r["checks"]:
        if not on_var(c, n):
            continue
        cite, cond, side = c[0], c[1], c[2]
        if not cite.startswith(("global/", )):
            return None
        mt = _COND.match(cond)
        if mt is None or int(mt[2]) != n:
            return None
        neg, op, rhs, ln = mt[1], mt[3], mt[4], mt[5]
        other = loc.get(int(ln)) if ln is not None else (int(rhs) if rhs is not None else None)
        tests.append((neg, op, other, side))

    def holds(x):
        for neg, op, other, side in tests:
            if op is None:
                t = (x == 0) if neg else (x != 0)
            else:
                t = {"==": x == other, "!=": x != other, "<": x < other, "<=": x <= other, ">": x > other,
                     ">=": x >= other}[op]
            if t != side:
                return False
        return True
    return any(holds(x) for x in dom if isinstance(x, int))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("a", type=Path)
    ap.add_argument("a2", type=Path)
    ap.add_argument("--var", type=int, required=True)
    ap.add_argument("--json", type=Path)
    o = ap.parse_args()
    n, key = o.var, f"var:{o.var}"
    ra, rb = records(o.a), records(o.a2)
    rep = next(x for x in json.loads(o.a2.read_text()).get("fact_vars_report") or () if x["var"] == n)
    dom = rep["domain"]
    groups = collections.defaultdict(list)
    for i, r in enumerate(ra):
        groups[shape(r)].append(i)
    unrefined = collections.Counter()
    merged_n = 0
    covered = set()
    examples = []
    sets = collections.Counter()
    for r2 in rb:
        pre_v = r2["pre"].get(key)
        sets["no precondition on it" if pre_v is None else "one value" if len(pre_v) == 1 else
             "every value" if len(pre_v) == len(dom) else "a set of values"] += 1
        cands = groups.get(shape(r2), [])
        verdicts = [(i, why_not(r2, ra[i], n)) for i in cands]
        hits = [i for i, v in verdicts if v is None]
        if not hits:
            hits = [i for i, v in verdicts if v == MERGED]
            if hits:
                merged_n += 1
        if not hits:
            reasons = collections.Counter(why_not(r2, ra[i], n) for i in cands)
            why = reasons.most_common(1)[0][0] if reasons else "no record of A with its source, label and ends"
            unrefined[why] += 1
            if len(examples) < 5:
                examples.append({"record": r2["label"], "source": r2["source"], "why": why, "candidates": len(cands)})
            continue
        covered.update(hits)
    checked = [i for i, r in enumerate(ra) if any(on_var(c, n) for c in r["checks"])]
    gap = collections.Counter()
    gap_examples = []
    for i in checked:
        if i in covered:
            gap["refined by a record of A2"] += 1
            continue
        f = feasible(ra[i], n, dom)
        what = {False: "infeasible: no value of the domain passes its checks on the fact",
                True: "feasible, and refined by no record of A2", None: "not evaluated"}[f]
        gap[what] += 1
        if f is not False and len(gap_examples) < 5:
            gap_examples.append({"record": ra[i]["label"], "checks": [c[:3] for c in ra[i]["checks"]], "what": what})
    out = {"A": str(o.a), "A2": str(o.a2), "fact": key, "domain": dom,
           "records": {"A": len(ra), "A2": len(rb)},
           "A2 records refining no record of A": sum(unrefined.values()), "why": dict(unrefined),
           "of the others, refining a merged record of A only": merged_n,
           "examples": examples, "A2's preconditions on the fact": dict(sets),
           "A records with a check on the fact": len(checked), "those records": dict(gap),
           "examples of those not refined": gap_examples}
    text = json.dumps(out, indent=1, default=str)
    print(text)
    if o.json:
        o.json.write_text(text + "\n")


if __name__ == "__main__":
    main()
