"""The driver's own work per job, before (explore.py) and after (fmi), on a fixed real input: a recorded archive's jobs
ingested again in its log's order into a fresh archive (job.json from the archive before the job, the selection,
then the ingest), as ``fmi check reingest`` does it.

usage (from the repo root): .venv/bin/python experiments/E011-speed/explore/bench_reingest.py ARCHIVE --out DIR
       [--fmi BIN] [--rounds N]

Each round runs explore.py's Archive (this process) and then ``fmi check reingest`` on the same archive, back to back,
so both meet the same load; per job the wall and CPU seconds of explore.py's work, and fmi's own report. A test
tool: it imports the frozen reference and edits none of its files.
"""

import argparse
import gzip
import json
import os
import resource
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT / "experiments" / "E008-go-explore"))

import explore as X  # noqa: E402


def cpu() -> float:
    r = resource.getrusage(resource.RUSAGE_THREAD) if hasattr(resource, "RUSAGE_THREAD") else \
        resource.getrusage(resource.RUSAGE_SELF)
    return r.ru_utime + r.ru_stime


def python_round(rec: Path, out: Path) -> tuple:
    shutil.rmtree(out, ignore_errors=True)
    stage = json.loads((rec / "stage.json").read_text())
    with gzip.open(rec / "entries" / "0.json.gz", "rt") as f:
        root0 = json.load(f)
    a = X.Archive.create(out, stage["stage"], stage["goal"], Path(stage["guide"]), stage["allowed_rooms"],
                         stage.get("workers", 2), stage.get("root"), stage.get("e009"))
    a.add(None, root0["frame"], root0["tick"], [], root0["checkpoints"], None, "boot")
    a.write_cells()
    walls, cpus = [], []
    for x in X.read_jsonl(rec / "log.jsonl")[0]:
        if x.get("outcome") not in X.COMMITTED:
            a.log_event(x)
            continue
        j = x["job"]
        src, dst = rec / "jobs" / j, out / "jobs" / j
        (dst / "new").mkdir(parents=True, exist_ok=True)
        for f in ("job.json", "select.json", "result.json"):
            if (src / f).exists():
                shutil.copy(src / f, dst / f)
        for p in (src / "new").glob("*"):
            shutil.copy(p, dst / "new" / p.name)
        want = json.loads((src / "job.json").read_text())
        t, c = time.monotonic(), cpu()
        e = want["entry"]["id"]
        json.dumps(a.job_doc(j, e), indent=1)
        a.select(1)
        a.sel_key(e)
        a.n_selectable()
        extra = {k: x[k] for k in ("started", "ended", "driver_wall_s", "rc", "rusage", "guard") if k in x}
        a.ingest(dst, extra)
        walls.append(time.monotonic() - t)
        cpus.append(cpu() - c)
    return walls, cpus


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("archive")
    ap.add_argument("--out", required=True)
    ap.add_argument("--fmi", default=os.environ.get("FMI_BIN") or "/var/tmp/fast-mi/target-e011-explore/release/fmi")
    ap.add_argument("--rounds", type=int, default=1)
    a = ap.parse_args()
    rec, out = Path(a.archive), Path(a.out)
    rows = []
    for k in range(a.rounds):
        walls, cpus = python_round(rec, out / "py")
        p = subprocess.run([a.fmi, "check", "reingest", str(rec), "--out", str(out / "fmi")], capture_output=True,
                           text=True)
        cpu_line = next((ln for ln in p.stdout.splitlines() if "CPU per job" in ln), "")
        wall_line = p.stdout.strip().splitlines()[-1] if p.stdout.strip() else ""
        same = [ln for ln in p.stdout.splitlines() if ln.startswith("DIFF")]
        # the two archives against each other: entries, cells and log byte for byte
        files = {f: (out / "py" / f).read_bytes() == (out / "fmi" / f).read_bytes()
                 for f in ("entries.jsonl", "cells.json", "log.jsonl")}
        row = {"round": k, "jobs": len(walls), "py_wall_ms_median": round(statistics.median(walls) * 1e3, 2),
               "py_wall_ms_mean": round(statistics.mean(walls) * 1e3, 2),
               "py_cpu_ms_median": round(statistics.median(cpus) * 1e3, 2),
               "py_cpu_ms_mean": round(statistics.mean(cpus) * 1e3, 2), "fmi_cpu": cpu_line.strip(),
               "fmi_wall": wall_line, "fmi_vs_reference_diffs": same, "py_vs_fmi_files_same": files}
        rows.append(row)
        print(json.dumps(row), flush=True)
    (out / "bench-reingest.jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))


if __name__ == "__main__":
    main()
