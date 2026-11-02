"""E010: the bound of play when ready's retries, measured: over the strict windows recorded (E006-E009), the children
forked before the chosen one (by fork frame), all outcomes but "duplicate" (Lemma 5: the same run as an earlier
child), and the frames from the first child's fork to the chosen one's. Each distinct window counted once (the same
window in several runs of the same route is one row)."""
import collections, glob, json, sys
files = []
for a in sys.argv[1:]:
    files += sorted(glob.glob(a + "/**/windows.jsonl", recursive=True))
seen = {}
for p in files:
    for ln in open(p):
        w = json.loads(ln)
        if not w.get("strict") or w.get("input") is None or not w.get("best"):
            continue
        b = w["best"]
        kids = sorted(w.get("children") or [], key=lambda c: c[0])
        before = [c for c in kids if c[0] < b["g"] and c[3] != "duplicate"]
        key = (w["input"], w["start"], b["g"], b["outcome"])
        row = (len(before), b["g"] - (kids[0][0] if kids else w["start"]), collections.Counter(c[3] for c in before),
               b["outcome"], w["input"], w["start"])
        seen.setdefault(key, (row, p))
rows = sorted((r for r, _ in seen.values()), key=lambda r: -r[0])
print(len(rows), "distinct windows with an end point")
print("children before the chosen one: max", rows[0][0], "; windows with any:", sum(1 for r in rows if r[0]))
for r in rows[:12]:
    print("  ", r)
