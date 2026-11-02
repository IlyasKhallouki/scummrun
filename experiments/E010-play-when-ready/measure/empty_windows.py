"""E010 baseline: the strict windows with no end point (empty) and the windows that found one: forks, wall, end."""
import collections, glob, json, sys
files = []
for a in sys.argv[1:]:
    files += sorted(glob.glob(a + "/**/windows.jsonl", recursive=True))
empty, found = [], []
for p in files:
    for ln in open(p):
        w = json.loads(ln)
        if not w.get("strict") or w.get("input") is None:
            continue
        row = (w.get("forks"), w.get("wall_s"), w.get("child_frames"), w.get("end") - w.get("start"),
               (w.get("end_reason") or "")[:60], w.get("outcomes"))
        (found if w.get("best") else empty).append(row)
for name, rows in (("empty", empty), ("with an end point", found)):
    print(name, len(rows), "windows; forks", sum(r[0] for r in rows), "wall s", round(sum(r[1] for r in rows), 1),
          "child frames", sum(r[2] for r in rows))
    print("  end reasons", collections.Counter(r[4] for r in rows).most_common(6))
    for r in sorted(rows, key=lambda r: -r[1])[:6]:
        print("  ", r)
