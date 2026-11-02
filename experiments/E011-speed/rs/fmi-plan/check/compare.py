"""Compare two directories of canonical plan results (fmi-plan's and plan_ref.py's), file by file.

usage: compare.py DIR_A DIR_B   -> one line per result: same, or the first differing field
"""
import json
import sys
from pathlib import Path


def norm(d):
    for s in d.get("steps", []):
        s[4] = [[k, v, [[c, sorted(x)] for c, x in w]] for k, v, w in s[4]]
    return d


a, b = Path(sys.argv[1]), Path(sys.argv[2])
bad = 0
for fa in sorted(a.glob("*.json")):
    fb = b / fa.name
    if not fb.exists():
        print(f"MISSING {fa.name} in {b}")
        bad += 1
        continue
    x, y = norm(json.loads(fa.read_text())), norm(json.loads(fb.read_text()))
    if x == y:
        print(f"same  {fa.stem}: {x['status']}, {len(x['steps'])} steps, {len(x['choices'])} choices, "
              f"{len(x['needed_facts'])} needed, {len(x['file_facts'])} file facts")
    else:
        bad += 1
        for k in x:
            if x[k] != y.get(k):
                print(f"DIFF  {fa.stem}: {k}: {str(x[k])[:300]} | {str(y.get(k))[:300]}")
                break
print("ALL SAME" if bad == 0 else f"{bad} DIFFER")
sys.exit(1 if bad else 0)
