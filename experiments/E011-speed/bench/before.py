"""E011 B0/B4: one of E009's 15 kitchen tries again with the Python reference (experiments/E011-speed/plan.md,
"Benchmark").

usage (with ~/.cache/fast-mi-e009/env.sh sourced, under heavy.lock and the guard):
       $PY before.py TRY_DIR [--seq SEQ] [--log LOG]

TRY_DIR's name is the try (j00000 ... j00014); its line in LOG (E009's kitchen log.jsonl) gives the entry, the scope,
the targets the try tried in order (``job_tried``; ``tried`` adds the entry's earlier ones), the reading (fresh, or
the reading another try made) and whether the entry's idle set was
measured already (``idle_measured`` is false where the line has an ``idle`` record). SEQ (default TRY_DIR/../..) holds
the sequence's entries: ``entries/ID.json.gz`` and ``entries.jsonl`` as explore.py writes them, entry 0 E009's root.

job.json is explore.py's job_doc with E009's switches (seed 1, stage kitchen, goal owner:566 = 1, E009's copy of the
kitchen guide, idle_frames 1600, reuse e009, 3 threads, 3 workers); ``readings`` is empty for a fresh reading, else the
named try's reading under SEQ. step.run_job then runs in this process with ``step.targets`` replaced by the log's list
(each with its tier and depth as step.targets gives them; a target it does not give is tier 1, depth 0). No file of
the reference is edited.

When SEQ is TRY_DIR's own sequence (B0), the try's new entries are added to SEQ with the ids the log's new_entries
assigns. Prints one CHECK line against the log (return frame, reading, targets tried, stop, new entries).
"""

import argparse
import gzip
import json
import os
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REF = HERE.parents[1] / "E008-go-explore"  # E009's code (branch e011 is made from e009)
sys.path.insert(0, str(REF))

import step  # noqa: E402

E009 = Path.home() / ".cache" / "fast-mi-e009"
LOG = E009 / "archives" / "kitchen" / "log.jsonl"
ARCH = str(E009 / "archives" / "kitchen")  # the log's paths start here
ROOT = E009 / "archives" / "door" / "entries" / "0.json.gz"
GUIDE = E009 / "guides" / "kitchen.json"
GOAL = {"owner:566": [1]}


def log_lines(path: Path) -> dict:
    return {d["job"]: d for d in map(json.loads, path.read_text().splitlines())}


def read_gz(p: Path) -> dict:
    with gzip.open(p, "rt") as f:
        return json.load(f)


def write_gz(p: Path, doc: dict) -> None:
    p.parent.mkdir(parents=True, exist_ok=True)
    tmp = p.with_suffix(".tmp")
    with gzip.open(tmp, "wt") as f:
        json.dump(doc, f)
    tmp.replace(p)


def ensure_root(seq: Path) -> None:
    """Entry 0, E009's root, as explore.py's init writes it."""
    e = seq / "entries" / "0.json.gz"
    if not e.exists():
        doc = read_gz(ROOT)
        write_gz(e, doc)
        with open(seq / "entries.jsonl", "a") as f:
            f.write(json.dumps({"id": 0, "parent": None, "frame": doc["frame"]}) + "\n")


def rows(seq: Path) -> dict:
    p = seq / "entries.jsonl"
    return {r["id"]: r for r in map(json.loads, p.read_text().splitlines())} if p.exists() else {}


def chain(seq: Path, i: int) -> list:
    rs, out = rows(seq), []
    while i is not None:
        out.append(i)
        i = rs[i]["parent"]
    return out[::-1]


def entry_doc(seq: Path, i: int) -> dict:
    """job.json's entry: explore.py job_doc's (the movie from boot, the last checkpoint)."""
    events = []
    for j in chain(seq, i):
        events += read_gz(seq / "entries" / f"{j}.json.gz")["events"]
    e = read_gz(seq / "entries" / f"{i}.json.gz")
    return {"id": i, "root": rows(seq)[i]["parent"] is None, "frame": e["frame"], "events": events,
            "checkpoint": e["checkpoints"][-1]}


def mapped(path: str, seq: Path) -> str:
    if not path.startswith(ARCH + "/"):
        raise ValueError(f"{path}: not under {ARCH}")
    return str(seq / path[len(ARCH) + 1:])


def job_doc(line: dict, seq: Path, try_dir: Path) -> dict:
    scope = sorted(line["scope"])
    rd = line["reading"]
    readings = [] if rd["fresh"] else [{"path": mapped(rd["path"], seq), "start_entry": None}]
    rooms = "-".join(str(r) for r in scope)
    return {"job": line["job"], "seed": 1, "stage": "kitchen", "goal": GOAL, "guide": str(GUIDE),
            "entry": entry_doc(seq, line["entry"]), "scope": scope, "tried": [], "held": [], "readings": readings,
            "reading_dir": str(try_dir.parent.parent / "readings" / f"{rooms}-{line['job']}"),
            "threads": 3, "workers": 3, "idle_frames": 1600, "idle_measured": line.get("idle") is None,
            "reuse": "e009"}


def forced_targets(forced: list):
    orig = step.targets

    def targets(reached_path, guide_doc, entry_facts, root_facts=None, held=(), tried=(), goal=None, rotation=None):
        ts = orig(reached_path, guide_doc, entry_facts, root_facts, held, tried, goal, rotation)
        idx = {(t[0], t[1]): t for t in ts}
        g = list((goal or {}).keys())
        return [idx.get((k, v)) or (k, v, 0 if len(g) == 1 and g[0] == k else 1, 0) for k, v in forced]
    return targets


def stop_class(stop) -> str:
    return (stop or "").split(":")[0]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("try_dir", type=Path)
    ap.add_argument("--seq", type=Path)
    ap.add_argument("--log", type=Path, default=LOG)
    a = ap.parse_args()
    try_dir = a.try_dir.resolve()
    own = try_dir.parent.parent
    seq = (a.seq or own).resolve()
    line = log_lines(a.log)[try_dir.name]
    ensure_root(seq)
    try_dir.mkdir(parents=True, exist_ok=True)
    job = job_doc(line, seq, try_dir)
    (try_dir / "job.json").write_text(json.dumps(job) + "\n")
    step.targets = forced_targets([(k, v) for k, v in line["job_tried"]])
    t = time.monotonic()
    r = step.run_job(try_dir)
    wall = time.monotonic() - t
    if seq == own and r.get("ok"):
        ids = line["new_entries"]
        if len(ids) != len(r["new"]):
            print(f"NOTE {line['job']}: {len(r['new'])} new entries, the log has {len(ids)}", flush=True)
        have = rows(seq)
        with open(seq / "entries.jsonl", "a") as f:
            for i, n in zip(ids, r["new"]):
                if i in have:
                    continue
                e = read_gz(try_dir / n["file"])
                e["id"], e["parent"] = i, line["entry"]
                write_gz(seq / "entries" / f"{i}.json.gz", e)
                f.write(json.dumps({"id": i, "parent": line["entry"], "frame": e["frame"], "job": line["job"]}) + "\n")
    att, rd = r.get("attempt") or {}, r.get("reading") or {}
    lrd, latt = line["reading"], line["attempt"] or {}
    checks = {
        "return": [(r.get("return") or {}).get("frame"), line["return"]["frame"]],
        "same": [(r.get("return") or {}).get("same"), True],
        "fresh": [rd.get("fresh"), lrd["fresh"]],
        "why": [rd.get("why"), lrd["why"]],
        "tried": [r.get("tried"), line["job_tried"]],
        "stop": [att.get("stop"), latt.get("stop")],
        "frames": [att.get("frames"), latt.get("frames")],
        "replans": [att.get("replans"), latt.get("replans")],
        "added": [[x.get("added") for x in att.get("refinements") or ()],
                  [x.get("added") for x in latt.get("refinements") or ()]],
        "new": [len(r.get("new") or ()), len(line["new_entries"])],
    }
    bad = [k for k, (x, y) in checks.items() if x != y]
    print(f"CHECK {line['job']}: entry {line['entry']} ok {r.get('ok')} error {r.get('error')} wall {wall:.1f} s; "
          f"stop class {stop_class(att.get('stop'))!r}; differ from the log: {bad or 'none'}", flush=True)
    for k in bad:
        print(f"  {k}: got {checks[k][0]!r}, log {checks[k][1]!r}", flush=True)
    sys.exit(step.exit_code(r))


if __name__ == "__main__":
    os.environ.setdefault("PYTHONHASHSEED", "0")
    main()
