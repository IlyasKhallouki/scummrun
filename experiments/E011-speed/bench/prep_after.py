"""E011 B3: the job folders of the 15 tries for fmi, from B0's (experiments/E011-speed/plan.md, "Benchmark").

usage: python prep_after.py B0_SEQ OUT_SEQ --settings ref|fast [--log LOG]

Each try's job.json is B0's (the same entry, its movie and checkpoint, scope, switches, threads and workers), with:
* ``reading_dir`` under OUT_SEQ, and the reading it reuses (B0's decision) the one OUT_SEQ's own try made, at the
  same place under OUT_SEQ as B0's under B0_SEQ;
* ``forced_targets``: the log's ``job_tried`` (fmi try reads them from job.json, so one process runs all 15);
* with ``--settings fast``: ``"ready": true`` (E010's play when ready; the search and the tail are fmi's options).
OUT_SEQ/entries and entries.jsonl link to B0's: every try starts from B0's entry (the inputs are fixed). This runs
before the benchmark, never inside it.
"""

import argparse
import json
import os
from pathlib import Path

LOG = Path.home() / ".cache" / "fast-mi-e009" / "archives" / "kitchen" / "log.jsonl"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("b0", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--settings", choices=("ref", "fast"), required=True)
    ap.add_argument("--log", type=Path, default=LOG)
    a = ap.parse_args()
    b0, out = a.b0.resolve(), a.out.resolve()
    (out / "jobs").mkdir(parents=True, exist_ok=True)
    for name in ("entries", "entries.jsonl"):
        if not (out / name).exists():
            os.symlink(b0 / name, out / name)
    for line in map(json.loads, a.log.read_text().splitlines()):
        job = json.loads((b0 / "jobs" / line["job"] / "job.json").read_text())
        job["reading_dir"] = job["reading_dir"].replace(str(b0), str(out), 1)
        job["readings"] = [dict(r, path=r["path"].replace(str(b0), str(out), 1)) for r in job["readings"]]
        job["forced_targets"] = line["job_tried"]
        if a.settings == "fast":
            job["ready"] = True
        d = out / "jobs" / line["job"]
        d.mkdir(parents=True, exist_ok=True)
        (d / "job.json").write_text(json.dumps(job) + "\n")
    print(f"{out}: 15 job folders ({a.settings})")


if __name__ == "__main__":
    main()
