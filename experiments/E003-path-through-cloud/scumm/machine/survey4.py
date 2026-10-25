"""E003, D7: what a dump reached of feature dialog (row 4): the click's input step (golden.py's input events, their
drift), the input script's notes, unknown writes and the switch input_after_turns' assumptions, paths with
``input_drift`` (merged ones too), the Run's unit and start, the runInputScript call left on a stack, and the rules the
slots ran by. Reads golden.py's line format, so it works on a wide.py dump and on a golden or candidate run folder alike.

usage: survey4.py FILE_OR_DIR... [--per-move]
"""

import collections
import gzip
import json
import sys
from pathlib import Path

C = collections.Counter


def survey(f: Path) -> C:
    c = C()
    trace = f.name.endswith(".trace.jsonl.gz")
    for line in gzip.open(f, "rt"):
        d = json.loads(line)
        if trace:
            if d.get("type") == "input":
                drift = d["drift"]["$tuple"]
                c["event input"] += 1
                c[f"event input, drift of {len(drift)}"] += 1
                for k in drift:
                    c[f"drift key {k['$tuple'][0]}"] += 1
            if d.get("type") == "merge":
                c["event merge"] += 1
            continue
        if d.get("kind") == "run":
            r = d["run"]
            u = r["unit"]
            if r["start"].startswith("input script") or (u and "input" in r["start"]):
                c[f"run start: {r['start'][:40]}"] += 1
            if u is not None and u["$tuple"][0] == "local":
                c["run unit: local"] += 1
            continue
        if d.get("kind") != "path":
            continue
        p = d["path"]
        dr = p["input_drift"]["$tuple"]
        if dr:
            c["paths with input_drift"] += 1
            c[f"paths with input_drift of {len(dr)}"] += 1
            if p["merged"]:
                c["merged paths with input_drift"] += 1
            for k in dr:
                k = k["$tuple"]
                kind = "unknown" if isinstance(k[2], dict) else "int"
                c[f"drift entry {k[0]} ({kind} now)"] += 1
        for h, _, w in (e["$tuple"] for e in p["notes"]):
            if h in ("checkExecVerbs", "runInputScript"):
                c[f"note: {h}: {w[:50]}"] += 1
        for h, _, w in (e["$tuple"] for e in p["unknown_writes"]):
            if h == "runInputScript":
                c[f"unknown write: runInputScript: {w[:30]}"] += 1
        for a in (x["$tuple"] for x in p["assumptions"]):
            if a[1].startswith("_userPut"):
                c[f"assumption: {a[1]}, side {a[2]}"] += 1
        for e in p["stack"]:
            if "$Call" in e and e["$Call"]["head"] == "runInputScript":
                c["runInputScript call on the stack at the end"] += 1
        for rule in p["ran_by"].values():
            if rule.startswith("c"):
                c[f"ran by: {rule}"] += 1
    return c


def files(args):
    for a in args:
        p = Path(a)
        if p.is_dir():
            yield from sorted(p.glob("*.jsonl.gz"))
        else:
            yield p


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    total = C()
    for f in files(args):
        c = survey(f)
        if "--per-move" in sys.argv and +c:
            print(f.name, dict(+c))
        total += c
    for k, v in sorted(total.items()):
        print(f"{v:9d}  {k}")


if __name__ == "__main__":
    main()
