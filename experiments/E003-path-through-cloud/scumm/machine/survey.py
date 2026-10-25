"""E003, D7: what a machine dump reached: ends, errors, notes, unknown writes, merges, limits, caps, trace events,
and room_change's counts (startScene's pushes, room-killed turns, `_kill_room`'s assumptions, scene_load's writes).
Reads golden.py's line format, so it works on a wide.py dump and on a golden or candidate run folder alike.

usage: survey.py FILE_OR_DIR...   (a wide.py dump's python.jsonl.gz and python.trace.jsonl.gz, or a run folder: every
                                   *.jsonl.gz in it)
"""

import collections
import gzip
import json
import sys
from pathlib import Path

C = collections.Counter


def main() -> None:
    files = []
    for a in sys.argv[1:]:
        p = Path(a)
        files += sorted(p.glob("*.jsonl.gz")) if p.is_dir() else [p]
    ends, errs, notes, uw, caps, ev, room = C(), C(), C(), C(), C(), C(), C()
    runs = merges = limit = paths = 0
    for f in files:
        trace = f.name.endswith(".trace.jsonl.gz")
        for line in gzip.open(f, "rt"):
            d = json.loads(line)
            if trace:
                if "type" not in d:
                    continue
                ev[d["type"]] += 1
                if d.get("type") == "push" and d.get("by") == "startScene":
                    room[f"push by startScene: {d['unit']['$tuple'][0]}"] += 1
                continue
            if d.get("kind") == "run":
                r = d["run"]
                runs += 1
                merges += r["merges"]
                limit += r["limit"]
                caps.update(r["value_caps"])
            if d.get("kind") != "path":
                continue
            p = d["path"]
            paths += 1
            ends[p["end"]] += 1
            errs.update(e["$tuple"][0] for e in p["errors"])
            notes.update(e["$tuple"][0] for e in p["notes"])
            uw.update(e["$tuple"][0] for e in p["unknown_writes"])
            for h, _, w in (e["$tuple"] for e in p["notes"]):
                if h == "startScene":
                    room["note: " + w.split(";")[0].split(" (")[0][:48]] += 1
            for h, _, w in (e["$tuple"] for e in p["unknown_writes"]):
                if h == "startScene":
                    room["unknown write: " + w.split(" of room")[0]] += 1
            for t in (a["$tuple"] for a in p["assumptions"]):
                if t[3] == "the slot kind is not a fact":
                    room["slot kind assumption: " + ("room" if " room slot" in t[1] else "inventory")] += 1
            w = p["writes"]
            room["paths that wrote VAR_NEW_ROOM"] += "var:72" in w
            v = w.get("var:70")
            if isinstance(v, dict):
                room["VAR_ROOM_FLAG " + ("value set" if v["$Unknown"]["values"] else "unknown")] += 1
            for v in w.values():
                if isinstance(v, dict) and "slot kind not known" in v["$Unknown"]["what"]:
                    room["slot kind not known writes"] += 1
            room["room 0"] += w.get("room") == 0
            room["turns how room"] += sum(t["$tuple"][1] == "room" for t in p["turns"])
    print(f"runs {runs}, paths {paths}, merges {merges}, limit forks {limit}")
    for name, c in (("ends", ends), ("errors", errs), ("unknown writes", uw), ("notes", notes), ("caps", caps),
                    ("events", ev), ("room_change", room)):
        print(name, dict(c.most_common()))


if __name__ == "__main__":
    main()
