"""E004 plan step A4, window-search.md section 9.7: is the approach to an off-screen object the same run when it starts at
the window's start instead of the child's fork?

usage (from this folder, with ../../.venv/bin/python, under nice):
       approach_check.py --run RUN.json --w0 F --fork G --click C [--frames F1,F2,...] [--bin SCUMMVM] [--out DIR]

An instrument: nothing the system runs imports it. From a window search run's record (a harness boot's run, replay_part1
``run``), two fresh engines from boot play the run's inputs up to frame ``--w0`` (the window's start), then the chosen
child's approach inputs (its input events after ``--fork`` and before ``--click``, the object click's frame): one at
their own frames, one moved back by fork - w0. The full state dumps (SPEEDRUN_DUMP_LOCALS and SPEEDRUN_DUMP_EXACT) are
compared at each frame of ``--frames`` (default: the frame before the click). It names no game number; the frames are
its arguments.
"""

import argparse
import dataclasses
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import replay as RP  # noqa: E402
import replay_part1 as RP1  # noqa: E402
from speedrun import paths  # noqa: E402
from speedrun.control import Controller  # noqa: E402


def dumps(out: Path, seed: int, events: list, frames: list) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    cfg = dataclasses.replace(RP1.config(seed, out, RP.write_events(out / "movie.jsonl", events)), dump_locals=True,
                              dump_exact=True)
    res = {}
    with Controller(cfg, timeout_s=600) as c:
        b = c.root
        for f in frames:
            b.run(frame=f)
            res[f] = b.state()
    return res


def diff(a, b, path: str = "") -> list:
    """The leaves where two dumps differ (lists of unequal length count as one difference)."""
    if isinstance(a, dict) and isinstance(b, dict):
        return [d for k in sorted(set(a) | set(b), key=str) for d in diff(a.get(k), b.get(k), f"{path}.{k}")]
    if isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        return [d for i, (x, y) in enumerate(zip(a, b)) for d in diff(x, y, f"{path}[{i}]")]
    if a != b:
        return [(path, a if not isinstance(a, (list, dict)) else "...", b if not isinstance(b, (list, dict)) else "...")]
    return []


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", type=Path, required=True)
    ap.add_argument("--w0", type=int, required=True)
    ap.add_argument("--fork", type=int, required=True)
    ap.add_argument("--click", type=int, required=True)
    ap.add_argument("--frames", default="")
    ap.add_argument("--bin", type=Path)
    ap.add_argument("--out", type=Path, default=paths.OUT_DIR / "E004-approach-check")
    a = ap.parse_args()
    if a.bin is not None:
        paths.SCUMMVM_BIN = a.bin
    doc = json.loads(a.run.read_text())
    if doc.get("starter"):
        raise SystemExit("a run from another start than the harness's boot: not handled")
    ins = doc["run"]["inputs"]
    base = [e for e in ins if e["frame"] <= a.w0]
    appr = [e for e in ins if a.fork < e["frame"] < a.click]
    early = [dict(e, frame=e["frame"] - (a.fork - a.w0)) for e in appr]
    frames = [int(x) for x in a.frames.split(",") if x] or [a.click - 1]
    late = dumps(a.out / "at-fork", doc["seed"], base + appr, frames)
    earl = dumps(a.out / "at-start", doc["seed"], base + early, frames)
    print(f"approach_check: {a.run.name}, seed {doc['seed']}: the chosen child's approach ({len(appr)} input events, "
          f"frames {appr[0]['frame'] if appr else None}-{appr[-1]['frame'] if appr else None}) from its fork {a.fork} "
          f"against the same events moved back to the window's start {a.w0}; full dumps (locals, exact) compared")
    for f in frames:
        d = diff(late[f], earl[f])
        print(f"frame {f}: {len(d)} differences (path, from the fork, from the window's start)")
        for p in d[:40]:
            print(f"  {p[0]}: {p[1]} / {p[2]}")


if __name__ == "__main__":
    main()
