"""E010 baseline: the chosen child's fork frame against the frame the model's window opened (rec "open"), and the
first issued child's fork frame against the window's start, in the strict windows' records."""
import collections, glob, json, sys
files = []
for a in sys.argv[1:]:
    files += sorted(glob.glob(a + "/**/windows.jsonl", recursive=True))
c = collections.Counter()
rows = []
for p in files:
    for ln in open(p):
        w = json.loads(ln)
        if not w.get("strict") or w.get("input") is None:
            continue
        b = w.get("best")
        kids = sorted(w.get("children") or [], key=lambda x: x[0])
        first_kid = kids[0] if kids else None
        c["windows"] += 1
        if b is None:
            c["no best; open " + ("None" if w.get("open") is None else "set")] += 1
            continue
        c[f"best early={b['early']}"] += 1
        if w.get("open") is None:
            c["best, window never opened"] += 1
        elif b["g"] < w["open"]:
            c["best forked before open"] += 1
        else:
            c["best forked at/after open"] += 1
        rows.append((b["g"] - w["start"], (w["open"] - w["start"]) if w.get("open") is not None else None,
                     (first_kid[0] - w["start"]) if first_kid else None, b["outcome"], w.get("forks")))
for k, v in sorted(c.items()):
    print(f"{v:6d} {k}")
print("best.g - start, open - start, first child g - start, outcome, forks (first 60):")
for r in rows[:60]:
    print("  ", r)
