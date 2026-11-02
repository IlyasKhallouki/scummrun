"""E011 G11 (experiments/E011-speed/plan.md, "Exactness gates"): each try of a B3 sequence against B0's: the stop and its
class, the attempt's target, frames, replans and refinements, and the new entries (frame, tick, how, every
checkpoint's facts, the inputs).

usage: python cmp_tries.py B0_SEQ SEQ [--json OUT]       (run after the benchmark, never inside it)
"""

import argparse
import gzip
import json
from pathlib import Path


def gz(p: Path):
    with gzip.open(p, "rt") as f:
        return json.loads(f.read())


def stop_class(s) -> str:
    return (s or "").split(":")[0]


def entries(d: Path, res: dict) -> list:
    out = []
    for n in res.get("new") or ():
        e = gz(d / n["file"])
        out.append({"frame": e["frame"], "tick": e.get("tick"), "how": n.get("how"), "events": e["events"],
                    "checkpoints": [[c[0], c[1]] for c in e["checkpoints"]]})
    return out


def compare(a_dir: Path, b_dir: Path) -> dict:
    a, b = json.loads((a_dir / "result.json").read_text()), json.loads((b_dir / "result.json").read_text())
    aa, ba = a.get("attempt") or {}, b.get("attempt") or {}
    out = {"job": a.get("job"), "ok": [a.get("ok"), b.get("ok")], "class": [stop_class(aa.get("stop")),
                                                                           stop_class(ba.get("stop"))]}
    diff = []
    for k in ("stop", "target", "frames", "replans", "frame0", "frame_end", "qf", "inputs"):
        if aa.get(k) != ba.get(k):
            diff.append([k, aa.get(k), ba.get(k)])
    ra = [x.get("added") for x in aa.get("refinements") or ()]
    rb = [x.get("added") for x in ba.get("refinements") or ()]
    if ra != rb:
        diff.append(["refinements", ra, rb])
    ea, eb = entries(a_dir, a), entries(b_dir, b)
    if len(ea) != len(eb):
        diff.append(["new entries", len(ea), len(eb)])
    for i, (x, y) in enumerate(zip(ea, eb)):
        for k in ("frame", "tick", "how", "events", "checkpoints"):
            if x[k] != y[k]:
                diff.append([f"new {i} {k}", str(x[k])[:80], str(y[k])[:80]])
    out.update(new=[e["frame"] for e in eb], same_class=out["class"][0] == out["class"][1], differ=diff)
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("b0", type=Path)
    ap.add_argument("seq", type=Path)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args()
    rows = []
    for d in sorted((a.seq / "jobs").iterdir()):
        if not (d / "result.json").exists():
            continue
        r = compare(a.b0 / "jobs" / d.name, d)
        rows.append(r)
        print(f"{r['job']}: class {r['class'][1]!r} ({'=' if r['same_class'] else '!='} B0's), new {r['new']}, "
              f"{'same as B0' if not r['differ'] else 'differs: ' + json.dumps(r['differ'])[:300]}")
    n_same = sum(1 for r in rows if not r["differ"])
    n_class = sum(1 for r in rows if r["same_class"])
    print(f"G11 {a.seq.name}: {len(rows)} tries, stop class = B0's in {n_class}, everything = B0's in {n_same}")
    if a.json:
        a.json.write_text(json.dumps(rows, indent=1) + "\n")


if __name__ == "__main__":
    main()
