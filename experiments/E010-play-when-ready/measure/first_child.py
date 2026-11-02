"""E010 baseline: in the strict window search's records, the first child that issued its input (by fork frame) against
the chosen one. Reads windows.jsonl files given on the command line (or found under the roots)."""
import collections, glob, json, sys

files = []
for a in sys.argv[1:]:
    files += sorted(glob.glob(a + "/**/windows.jsonl", recursive=True)) if not a.endswith(".jsonl") else [a]
tot = collections.Counter()
lag = []
detail = collections.Counter()
per = []
for p in files:
    for ln in open(p):
        w = json.loads(ln)
        if not w.get("strict") or w.get("input") is None:
            continue
        kids = sorted(w.get("children") or [], key=lambda c: c[0])
        issued = [c for c in kids if c[3] not in ("interface", "duplicate")]
        inter = [c for c in kids if c[3] == "interface"]
        best = w.get("best")
        first = issued[0] if issued else None
        tot["windows"] += 1
        if first is None:
            tot["no child issued"] += 1
            tot["no child issued, best " + str(best and best["outcome"])] += 1
            continue
        fo = first[3]
        tot["first issued: " + fo] += 1
        if best is None:
            tot["no best"] += 1
            continue
        same = best["g"] == first[0] or best["first"] == first[1]
        if same:
            tot["best is the first issued"] += 1
        else:
            k = sum(1 for c in issued if c[0] < best["g"])
            lag.append((best["g"] - first[0], k, fo, best["outcome"], p.split("/")[-3] if "jobs" in p else p, w.get("input")))
            tot[f"best later: first {fo} -> best {best['outcome']}"] += 1
        per.append((w.get("wall_s"), len(kids), len(inter), w.get("end_reason")))
print(len(files), "files")
for k, v in sorted(tot.items()):
    print(f"{v:6d}  {k}")
print("best later than the first issued child: (frames after it, issued children before it, first's outcome, best's)")
for x in sorted(lag):
    print("  ", x)
