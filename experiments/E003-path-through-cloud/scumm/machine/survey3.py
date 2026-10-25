"""E003, D7: what a dump reached of feature settle (row 3): path ends and first ends, rounds, merges at a round's
start, skips, the rules turns ran by, settle's assumptions (loop exits, waits over, camera exits), the facts settle
made unknown, second instances, ambient caps, order conflicts, the sentence queue, and the settle trace events.
Reads golden.py's line format, so it works on a wide.py dump and on a golden or candidate run folder alike.

usage: survey3.py FILE_OR_DIR... [--per-move] [--cover]
   --per-move: one line of counts per file;  --cover: a few cheap moves that together reach every count the files
   reach (a test subset; needs the folder's index.json for the moves' Python seconds)
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
            t = d.get("type")
            if t in ("turn", "skip", "round", "settled", "queue"):
                c[f"event {t}" + (" (didexec)" if t == "turn" and d.get("ran") is False else "")] += 1
            if t == "settled":
                c[f"settled event: {d['end']}"] += 1
            continue
        if d.get("kind") == "run":
            r = d["run"]
            c["runs"] += 1
            c["settle merges"] += r["settle_merges"]
            c["value merges"] += r["value_merges"]
            c["runs with pre slots"] += bool(r["pre_slots"]["$tuple"])
            continue
        if d.get("kind") != "path":
            continue
        p = d["path"]
        if not p["settle"]:
            continue
        c["settle paths"] += 1
        end = p["end"]
        if end == "settled" and "locals still moved" in (p["end_at"] or ""):
            end = "settled, locals still moved"
        c[f"end: {end}"] += 1
        c[f"first end: {p['first_end']}"] += 1
        c[f"round {min(p['round'], 9)}{'+' if p['round'] >= 9 else ''}"] += 1
        for n, origin, why in (s["$tuple"] for s in p["skips"]["$set"]):
            c[f"skip ({origin}): {why}"] += 1
        for rule in p["ran_by"].values():
            c[f"ran by: {rule}"] += 1
        for a in (x["$tuple"] for x in p["assumptions"]):
            why = a[3]
            for kind in ("loop exit by time", "wait over by time", "camera position"):
                if why.startswith(f"?({kind}"):
                    c[f"assumption: {kind}, side {a[2]}" + (" (stays, forked once)" if "forked once" in why else "")] += 1
        c["facts made unknown by settle (writer R_SETTLE)"] += sum(1 for r in p["writer"].values() if r == "not settled")
        c["writer mixed"] += sum(1 for r in p["writer"].values() if r == "mixed")
        c["paths with a second instance"] += bool(p["multi"]["$set"])
        c["paths with an ambient cap"] += bool(p["ambient_slots"]["$set"])
        c["paths with camera exits forked"] += bool(p["camera_done"]["$set"])
        c["order conflicts"] += len(p["conflicts"])
        c["paths that ran queued sentences"] += p["sentences"] > 0
        c["paths with a sentence still queued"] += bool(p["queue"])
        c["paths with ambient values"] += bool(p["ambient_vals"])
        c["kept slots at the end"] += len(p["slots"])
        c["paths whose after is not empty"] += bool(p["after"]["$set"])
        c["slots never run (not_run)"] += len(p["not_run"])
        for h, _, w in (e["$tuple"] for e in p["notes"]):
            if h in ("sentence queue", "doSentence") or w.endswith("F flag not known"):
                c[f"note: {h}: {w[:60]}"] += 1
        for h, _, w in (e["$tuple"] for e in p["unknown_writes"]):
            if h == "doSentence":
                c["unknown write: doSentence"] += 1
    return c


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    files = []
    for a in args:
        p = Path(a)
        files += sorted(p.glob("*.jsonl.gz")) if p.is_dir() else [p]
    total, per = C(), {}
    for f in files:
        c = survey(f)
        per[f] = c
        total.update(c)
    if "--per-move" in sys.argv:
        for f, c in per.items():
            print(f.name, dict(c))
    for k, v in sorted(total.items()):
        print(f"{v:9d}  {k}")
    if "--cover" in sys.argv:
        secs = {}
        for a in args:
            ix = Path(a) / "index.json"
            if ix.exists():
                for m in json.loads(ix.read_text())["moves"]:
                    secs[m["label"]] = m.get("cpu_s") or 0.0
        moves = collections.defaultdict(set)
        for f, c in per.items():
            label = f.name.split(".")[0]
            moves[label] |= {k for k, v in c.items() if v}
        want, pick = set().union(*moves.values()), []
        while want:
            label = max(moves, key=lambda m: (len(moves[m] & want) / (1 + secs.get(m, 0)), -secs.get(m, 0)))
            if not moves[label] & want:
                break
            pick.append(label)
            want -= moves[label]
        print("cover:", ",".join(pick), f"({sum(secs.get(m, 0) for m in pick):.1f} s of Python)")


if __name__ == "__main__":
    main()
