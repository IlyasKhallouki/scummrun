"""Plan a packed task (taskcore's file, written by `reader-port read NAME --task-out DIR`) from one of its starts:
taskcore `sas` (its default fold and reductions, the model encoding), then lama-first (plan_fd.py's search string),
the plan read back through taskcore's operator table to the input actions.

usage: python plan_packed.py TASK.tc START WORKDIR [--time S] [--memory 6G]

Prints taskcore's operator count, the planner's status and time, and per step the action's name, kind and end.
Writes WORKDIR/plan.json. Fast Downward is plan_fd.py's (DOWNWARD_DIR for a checkout other than the submodule);
taskcore is TASKCORE_BIN or this experiment's build.
"""

import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import plan_fd  # noqa: E402


def main() -> None:
    task, start, work = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
    args = sys.argv[4:]
    limit = int(args[args.index("--time") + 1]) if "--time" in args else 600
    memory = args[args.index("--memory") + 1] if "--memory" in args else "6G"
    work.mkdir(parents=True, exist_ok=True)
    sas, ops, stats = work / "task.sas", work / "ops.jsonl", work / "stats.json"
    t0 = time.monotonic()
    cmd = [str(plan_fd.TASKCORE), "sas", "--task", str(task), "--start", start, "--out", str(sas), "--stats",
           str(stats), "--ops", str(ops)]
    p = subprocess.run(cmd, capture_output=True, text=True)
    if p.returncode != 0:
        sys.exit(f"taskcore: {p.stderr.strip()}")
    write_s = time.monotonic() - t0
    st = json.loads(stats.read_text())
    code, log, text, seconds, _ = plan_fd.run(sas.read_text(), limit, memory, workdir=str(work / "fd"))
    status = plan_fd.STATUS.get(code, "error")
    out = {"task": str(task), "start": start, "operators": st["sas"]["operators"], "taskcore_s": round(write_s, 1),
           "status": status, "planner_s": round(seconds, 1), "steps": []}
    print(f"{task.name} from {start}: {out['operators']} operators (taskcore {write_s:.1f} s); lama-first: {status} "
          f"in {seconds:.1f} s")
    if status == "plan":
        head, opt, acts = plan_fd.read_ops(ops)
        for s in plan_fd.decode_ops(text, opt, acts):
            out["steps"].append({k: s[k] for k in s if k in ("name", "members")})
            print(f"  {s['name']}")
    (work / "plan.json").write_text(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
