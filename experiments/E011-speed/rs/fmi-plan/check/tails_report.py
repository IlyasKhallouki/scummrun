"""G7's tail gate: fmi-plan's tail check (tails_make.py's requests, `fmi-plan seq --tail-only`) against the recorded
replans: accepted where the recorded new plan was a tail of the old one, and whether the accepted steps are the recorded
plan's (step names with "#rec" left out).

usage: tails_report.py OUT_DIR   (manifest.jsonl and out-*.jsonl in it)
"""
import json
import re
import sys
from pathlib import Path

STRIP = re.compile(r"#\d+$")
out = Path(sys.argv[1])
man = {m["name"]: m for m in (json.loads(l) for l in open(out / "manifest.jsonl")) if "name" in m}
res = {}
for f in sorted(out.glob("out-*.jsonl")):
    for line in open(f):
        d = json.loads(line)
        if "name" in d:
            res[d["name"]] = d
rows = []
for name, m in sorted(man.items()):
    r = res.get(name)
    if r is None:
        rows.append((name, m["recorded_tail"], None, None, "not run"))
        continue
    acc = r["status"] == "plan"
    same = acc and [STRIP.sub("", x.split("~ban:")[0]) for x in r.get("tail_names", [])] == \
        [STRIP.sub("", x) for x in m["new"]]
    rows.append((name, m["recorded_tail"], acc, same, f"{r['tail_s'] * 1000:.1f} ms {r.get('tail', '')} {m['f'][-90:]}"))
for row in rows:
    print(*row)
rec = [r for r in rows if r[1]]
print(f"checked {sum(r[2] is not None for r in rows)}; recorded tails {len(rec)}: accepted {sum(bool(r[2]) for r in rec)}, "
      f"the recorded plan {sum(bool(r[3]) for r in rec)}; recorded non-tails {len(rows) - len(rec)}: accepted "
      f"{sum(bool(r[2]) for r in rows if not r[1])}")
