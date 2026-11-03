"""G9, the guide's part (experiments/E011-speed/plan.md, "Exactness gates"): ``fmi guide map`` writes guide.py's
bytes, but for ``stats.seconds``, for the door and kitchen goals (and part1, and the wide map, as further checks).

usage (from the repo root): .venv/bin/python experiments/E011-speed/explore/g9_guide.py [--fmi BIN] [--out DIR]

Each goal is mapped by the frozen guide.py and by fmi, one after the other on the same machine; the files are compared
line by line with the stats line's "seconds" blanked, and ``guide.py stats`` / ``fmi guide stats`` are compared too
(their seconds blanked). The wall and CPU seconds of both go to DIR/g9-guide.jsonl.
"""

import argparse
import json
import os
import re
import resource
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
GUIDE_PY = ROOT / "experiments" / "E008-go-explore" / "guide.py"
CASES = [("door", ["--goal", "state:316=1"]), ("kitchen", ["--goal", "owner:566=1"]), ("part1", ["--goal", "part1"]),
         ("kitchen-wide", ["--goal", "owner:566=1", "--wide"])]


def timed(cmd: list) -> tuple:
    r0 = resource.getrusage(resource.RUSAGE_CHILDREN)
    t = time.monotonic()
    p = subprocess.run(cmd, capture_output=True, text=True)
    wall = time.monotonic() - t
    r1 = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu = (r1.ru_utime + r1.ru_stime) - (r0.ru_utime + r0.ru_stime)
    if p.returncode != 0:
        raise SystemExit(f"{cmd[:3]} failed: {p.stderr[-2000:]}")
    return p.stdout, round(wall, 3), round(cpu, 3)


def blank(text: str) -> str:
    text = re.sub(r'"seconds":[0-9.e+-]+', '"seconds":S', text)
    return re.sub(r"writer units (\d+), [0-9.e+-]+ s", r"writer units \1, S s", text)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fmi", default=os.environ.get("FMI_BIN") or "/var/tmp/fast-mi/target-e011-explore/release/fmi")
    ap.add_argument("--out", default=str(Path.home() / ".cache/fast-mi-e011/explore/guide"))
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    rows, bad = [], 0
    for name, args in CASES:
        py_out, rs_out = out / f"{name}.py.json", out / f"{name}.rs.json"
        py_stats, py_wall, py_cpu = timed([sys.executable, str(GUIDE_PY), "map", *args, "--out", str(py_out)])
        rs_stats, rs_wall, rs_cpu = timed([a.fmi, "guide", "map", *args, "--out", str(rs_out)])
        same_file = blank(py_out.read_text()) == blank(rs_out.read_text())
        same_stats = blank(py_stats) == blank(rs_stats)
        st_py, _, _ = timed([sys.executable, str(GUIDE_PY), "stats", str(py_out)])
        st_rs, _, _ = timed([a.fmi, "guide", "stats", str(py_out)])
        same_cli = blank(st_py) == blank(st_rs)
        row = {"goal": name, "same_file": same_file, "same_map_stdout": same_stats, "same_stats_cmd": same_cli,
               "bytes": len(rs_out.read_bytes()), "py_wall_s": py_wall, "py_cpu_s": py_cpu, "fmi_wall_s": rs_wall,
               "fmi_cpu_s": rs_cpu}
        bad += not (same_file and same_stats and same_cli)
        rows.append(row)
        print(json.dumps(row), flush=True)
    (out / "g9-guide.jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))
    print(f"G9 guide: {len(rows)} goals, {bad} differing")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
