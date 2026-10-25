"""E003, D7: what a dump reached of feature esc (row 5): the Esc frame's abort step (golden.py's abort events: armed
keys, the key code), the rewound slot's turn (a push by the Esc call, or a skip, or settle's own turn), where the key
went (key events and ``Run.key_routes``), the notes, assumptions and unknown writes of the three steps, ``Path.cleared``
and the runInputScript call left on a stack. Reads golden.py's line format, so it works on a wide.py dump and on a
golden or candidate run folder alike.

usage: survey5.py FILE_OR_DIR... [--per-move]
"""

import collections
import gzip
import json
import sys
from pathlib import Path

C = collections.Counter
ESC_HEADS = ("Esc", "Esc key", "assumption: cutsceneOverride")


def survey(f: Path) -> C:
    c = C()
    trace = f.name.endswith(".trace.jsonl.gz")
    rewound = None
    for line in gzip.open(f, "rt"):
        d = json.loads(line)
        if trace:
            t = d.get("type")
            if t == "start" and d["start"].startswith("Esc: abortCutscene rewinds "):
                rewound = d["start"][len("Esc: abortCutscene rewinds "):].split(" to ")[0]
            if t == "abort":
                armed = d["armed"]["$tuple"]
                code = d["code"]
                kind = "unknown" if isinstance(code, dict) else ("0" if code == 0 else "int")
                c[f"event abort, {len(armed)} armed, code {kind}"] += 1
            elif t == "key":
                c[f"event key: {d['fate']}"] += 1
            elif t == "push" and d["by"] == "Esc":
                c["event push by Esc (the rewound slot's turn)"] += 1
            elif t == "turn" and rewound is not None and d["name"] == rewound and d.get("round") == 0:
                c["event turn of the rewound slot in round 0 (settle's own rule)"] += 1
            elif t == "turn" and d.get("ran") is False and d["name"] == rewound:
                c["event turn of the rewound slot: didexec"] += 1
            elif t == "skip" and d["why"].startswith(("its override point", "its liveness")):
                c[f"event skip: {d['why'][:40]}"] += 1
            elif t == "fork" and d["assume"] and d["assume"]["$tuple"][1] in (
                    "clearClickedStatus ran in the frame", "_userPut <= 0: the key is dropped"):
                c[f"event fork on: {d['assume']['$tuple'][1]}"] += 1
            continue
        if d.get("kind") == "run":
            r = d["run"]
            if r["start"].startswith("Esc"):
                c[f"run start: {r['start'][:36]}"] += 1
                routes = r["key_routes"]["$tuple"]
                c[f"run with {min(len(routes), 3)}{'+' if len(routes) >= 3 else ''} key routes"] += 1
                for fr in routes:
                    fate, assumptions = fr["$tuple"]
                    c[f"key route: {fate[:30]}"] += 1
                    if assumptions["$tuple"]:
                        c["key routes with assumptions"] += 1
            continue
        if d.get("kind") != "path":
            continue
        p = d["path"]
        if p["cleared"] is not False:
            c[f"path cleared {p['cleared']}"] += 1
        for h, _, w in (e["$tuple"] for e in p["notes"]):
            if h in ESC_HEADS:
                c[f"note: {h}: {w[:55]}"] += 1
        for h, _, w in (e["$tuple"] for e in p["unknown_writes"]):
            if h == "runInputScript":
                c[f"unknown write: runInputScript: {w[:30]}"] += 1
        for a in (x["$tuple"] for x in p["assumptions"]):
            if a[1] in ("clearClickedStatus ran in the frame", "_userPut <= 0: the key is dropped"):
                c[f"assumption: {a[1]}, side {a[2]}"] += 1
        for e in p["stack"]:
            if "$Call" in e and e["$Call"]["head"] in ("runInputScript", "Esc"):
                c[f"{e['$Call']['head']} call on the stack at the end"] += 1
        for rule in p["ran_by"].values():
            if "Esc" in rule:
                c[f"ran by: {rule}"] += 1
        for s in p["skips"]["$set"]:
            why = s["$tuple"][2]
            if why.startswith(("its override point", "its liveness")):
                c[f"skip: {why[:40]}"] += 1
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
