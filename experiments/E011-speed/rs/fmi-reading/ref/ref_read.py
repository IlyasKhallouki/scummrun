"""readio's reference reading: E009's read (step.read_fresh, and replay_part1.refiner for a refinement) through
replay_part1.read_rust with stream=True and E009's switches, run with READER_SCOPE_BIN (E009's binary, 2a3cf8a1) and
packed_reading.reading_from_golden. The golden file is deleted after, as step.read_fresh deletes it (--keep-golden
keeps it).

usage: READER_SCOPE_BIN=... python ref_read.py --rooms 33,38,83,85,96 --start-rows ROWS --out DIR
                                               [--fact-vars 103,194] [--reach-also FILE ...] [--goal owner:566=1]

Prints {file: sha1} of the reading's files as JSON.
"""

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
sys.path.insert(0, str(ROOT / "experiments" / "E003-path-through-cloud"))
sys.path.insert(0, str(ROOT / "src"))

import replay_part1 as P  # noqa: E402

SWITCHES = "value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost"  # step.py SWITCHES
FILES = ("reading.json", "reading.tc", "reading.actions.jsonl", "reading.lists.jsonl", "reading.reached")


def sha1(p: Path) -> str:
    h = hashlib.sha1()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rooms", required=True)
    ap.add_argument("--start-rows", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--goal", default="owner:566=1")
    ap.add_argument("--threads", type=int, default=3)
    ap.add_argument("--fact-vars", default="")
    ap.add_argument("--reach-also", type=Path, action="append", default=[])
    ap.add_argument("--keep-golden", action="store_true")
    a = ap.parse_args()
    rooms = tuple(sorted(int(r) for r in a.rooms.split(",")))
    fact_vars = [int(x) for x in a.fact_vars.split(",") if x.strip()]
    out = a.out.resolve()
    t = time.monotonic()
    path = P.read_rust(rooms, 1, a.goal, out.name, out, a.threads, None, "none", elsewhere=True, live_locals=True,
                       split_limit=True, switches=SWITCHES, reach=True, frame_costs=True,
                       reach_also=[p.resolve() for p in a.reach_also] or None, start_rows=a.start_rows.resolve(),
                       fact_vars=fact_vars or None, stream=True)
    wall = time.monotonic() - t
    if not a.keep_golden:
        (out / f"{out.name}.golden.jsonl.gz").unlink(missing_ok=True)
    doc = json.loads(Path(path).read_text())
    print(json.dumps({"wall_s": round(wall, 1), "read_s": doc.get("read_s"),
                      "files": {f: sha1(out / f) for f in FILES if (out / f).exists()}}, indent=1))


if __name__ == "__main__":
    main()
