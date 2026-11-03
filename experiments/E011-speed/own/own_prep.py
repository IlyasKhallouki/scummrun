"""E011 round 2, item own: the fixed tries own_try.sh runs, made from B0's job folders (never run inside a try).

usage: python own_prep.py [B0] [OUT]     (defaults: ~/.cache/fast-mi-e011/bench/b0, ~/.cache/fast-mi-e011/own)

Each job.json is B0's (entry, scope, switches, threads, workers, idle frames) with the reading it reuses copied under
OUT/readings (fmi writes reading.idx next to a reading on its first open; B0's folders stay as they are), its
reading_dir under OUT, the log's targets forced (``forced_targets``, as prep_after.py) and play when ready:
  p    j00004, ring(38): entry 6, B0's j00002 reading, bit:110 = 1, 1,600 idle frames;
  q12  j00012, ring(33): entry 14, B0's j00000 refined reading (run/a00/refine-01), bit:115 = 1, no idle;
  q6   j00006, ring(33): entry 1, the same reading, bit:111 = 1, 1,600 idle frames.
OUT/entries and entries.jsonl link to B0's (the root's facts for the targets).
"""

import json
import os
import shutil
import sys
from pathlib import Path

LOG = Path.home() / ".cache" / "fast-mi-e009" / "archives" / "kitchen" / "log.jsonl"
JOBS = {"p": ("j00004", "readings/33-38-83-85-96-j00002", "33-38-83-85-96-j00002"),
        "q12": ("j00012", "jobs/j00000/run/a00/refine-01", "r33/refine-01"),
        "q6": ("j00006", "jobs/j00000/run/a00/refine-01", "r33/refine-01")}


def main() -> None:
    b0 = Path(sys.argv[1] if len(sys.argv) > 1 else Path.home() / ".cache/fast-mi-e011/bench/b0").resolve()
    out = Path(sys.argv[2] if len(sys.argv) > 2 else Path.home() / ".cache/fast-mi-e011/own").resolve()
    tried = {x["job"]: x["job_tried"] for x in map(json.loads, LOG.read_text().splitlines())}
    for name in ("entries", "entries.jsonl"):
        if not (out / name).exists():
            os.symlink(b0 / name, out / name)
    for key, (job, src, dst) in JOBS.items():
        rd = out / "readings" / dst
        if not rd.exists():
            rd.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(b0 / src, rd, symlinks=True)
        j = json.loads((b0 / "jobs" / job / "job.json").read_text())
        j["readings"] = [{"path": str(rd / "reading.json"), "start_entry": None}]
        j["reading_dir"] = str(out / "readings" / f"r-{key}")
        j["forced_targets"] = tried[job]
        j["ready"] = True
        (out / "jobs" / key).mkdir(parents=True, exist_ok=True)
        (out / "jobs" / key / "job.json").write_text(json.dumps(j) + "\n")
        print(key, job, j["entry"]["id"], j["forced_targets"], j["idle_measured"])


if __name__ == "__main__":
    main()
