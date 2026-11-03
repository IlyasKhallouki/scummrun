"""G7's tail gate, second half: every tail fmi-plan accepted is a plan by taskcore's own `validate` (the old binary,
on the reduced task of the same request): its steps' input actions, from the request's start, to its goal.

usage: tails_validate.py OUT_DIR TASKCORE_BIN
"""
import json
import subprocess
import sys
from pathlib import Path

out, binary = Path(sys.argv[1]), sys.argv[2]
man = {m["name"]: m for m in (json.loads(l) for l in open(out / "manifest.jsonl")) if "name" in m}
n = ok = 0
for f in sorted(out.glob("out-*.jsonl")):
    for line in open(f):
        d = json.loads(line)
        if d.get("status") != "plan" or d.get("source") != "tail":
            continue
        m = man[d["name"]]
        names = out / f"{d['name']}.accepted"
        names.write_text("".join(x.split("~ban:")[0] + "\n" for x in d["tail_names"]))
        cmd = [binary, "validate", "--task", str(Path(m["reading"]) / "reading.tc"), "--start-file", m["start"],
               "--goal", m["goal"], "--start-check", "needed", "--names", str(names),
               "--out", str(out / f"{d['name']}.validate.json")]
        if m.get("caps"):
            cmd += ["--cost-caps", m["caps"]]
        if m.get("bans"):
            cmd += ["--bans", m["bans"]]
        p = subprocess.run(cmd, capture_output=True, text=True)
        v = json.loads((out / f"{d['name']}.validate.json").read_text()) if p.returncode == 0 else {}
        good = v.get("ok") is True and v.get("goal") is True
        n += 1
        ok += good
        print(f"{'valid' if good else 'NOT VALID'}  {d['name']}: {len(d['tail_names'])} steps; {v.get('steps')} "
              f"{p.stderr.strip()[:200]}")
print(f"accepted tails {n}: valid by taskcore validate {ok}")
sys.exit(0 if n == ok else 1)
