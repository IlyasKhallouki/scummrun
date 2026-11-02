"""A stand-in for step.py in explore.py's tests (test_explore.py): it reads job.json and writes result.json and new
entries in plan.md's formats, with no engine, reader or planner. What it does depends on job.json and the guide only,
so a run of the explorer gives the same archive however its jobs are timed.

usage: fake_step.py run JOB_DIR

The fake job: the target is the first atom of the guide's R, by (tier, depth, key, value), that the entry does not
hold and the representative did not try, the goal's keys left out (tier 1: no entry holds it; tier 2: some entry
does; a room atom only for a room of the scope; a value of a key with (k, any) in R is the entry's value plus 1).
With no target the job ends exhausted. Else two new entries: a checkpoint 3 frames on where var:990 (no key of the
guide) went up by 1, and the end 7 frames on where the target holds too; the job with number FAKE_GOAL_AFTER - 1 or
later also sets the stage goal there. A reading is made (reading_dir/reading.json) when job.json lists none, else the
first is used.

env: FAKE_GOAL_AFTER (default 4); FAKE_SLEEP_S (default 0; even job numbers sleep it, odd ones a third of it);
     FAKE_FAIL (an entry id whose jobs fail with one error); FAKE_DIFFER (an entry id whose return differs);
     FAKE_GUARD_ONCE (a marker file: the first job to find it absent creates it and exits as the guard's kill does,
     with no result.json and the guard's line in its log); FAKE_NO_PROGRESS (a key: the target is that key at the
     entry's value plus 2, whatever R holds); FAKE_IDLE_KEY (E009: a key the end entry raises by 1 in every job, as
     an idle engine's counter would; with job.json idle_frames and not idle_measured the result's idle lists it and
     the goal's keys as returning).
"""

import gzip
import json
import os
import sys
import time
from pathlib import Path


def write_gz(path: Path, obj) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(path, "wt") as f:
        json.dump(obj, f)


def write_json(path: Path, obj) -> None:
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps(obj, indent=1) + "\n")
    os.replace(tmp, path)


def main() -> None:
    if len(sys.argv) != 3 or sys.argv[1] != "run":
        raise SystemExit(__doc__)
    t0 = time.monotonic()
    d = Path(sys.argv[2])
    job = json.loads((d / "job.json").read_text())
    n = int(job["job"][1:])
    marker = os.environ.get("FAKE_GUARD_ONCE")
    if marker and not Path(marker).exists():
        Path(marker).write_text(job["job"] + "\n")
        print("GUARD: MemAvailable 1000 kB, killing group 1", flush=True)  # the guard's own line (guard.sh)
        sys.exit(143)
    sleep = float(os.environ.get("FAKE_SLEEP_S") or 0)
    time.sleep(sleep if n % 2 == 0 else sleep / 3)
    e = job["entry"]
    frame, facts, extra = e["checkpoint"]
    out = {"job": job["job"], "entry": e["id"], "ok": True, "error": None,
           "return": {"same": True, "differ": {}, "boot_s": 0.0}, "reading": None,
           "targets": {"by_tier": {"0": 0, "1": 0, "2": 0}, "first": []}, "probes": [], "attempt": None,
           "readings_made": [], "new": [], "tried": [], "exhausted": False,
           "cpu_s": {"return": 0.0, "read": 0.0, "plan": 0.0, "play": 0.0, "total": 0.0},
           "peak_rss_mb": {"python": 10.0, "children": 0.0}, "wall_s": 0.0}
    if os.environ.get("FAKE_DIFFER") == str(e["id"]):
        out["return"] = {"same": False, "differ": {"facts": 1, "first": [["var:990", [0, 1]]]}, "boot_s": 0.0}
        out["ok"], out["error"] = False, "the return differs"
        write_json(d / "result.json", out)
        return
    if os.environ.get("FAKE_FAIL") == str(e["id"]):
        out["ok"], out["error"] = False, "fake failure"
        write_json(d / "result.json", out)
        return
    if job["readings"]:
        out["reading"] = {"path": job["readings"][0]["path"], "fresh": False, "why": None, "scope": job["scope"],
                          "records": 10, "actions": 5, "read_wall_s": 0.0, "read_cpu_s": 0.0, "peak_rss_mb": 0.0,
                          "files_s": 0.0}
    else:
        rd = Path(job["reading_dir"])
        rd.mkdir(parents=True, exist_ok=True)
        (rd / "reading.json").write_text(json.dumps({"fake": True, "scope": job["scope"]}) + "\n")
        out["reading"] = {"path": str(rd / "reading.json"), "fresh": True, "why": "no cached reading",
                          "scope": job["scope"], "records": 100, "actions": 50, "read_wall_s": 1.0,
                          "read_cpu_s": 2.0, "peak_rss_mb": 50.0, "files_s": 0.5}
    guide = json.loads(Path(job["guide"]).read_text())
    held = {tuple(p) for p in job["held"]}
    tried = {tuple(t) for t in job["tried"]}
    cands = []
    for k, v, depth in guide["atoms"]:
        if k in job["goal"] or (k == "room" and v not in job["scope"]):
            continue
        v = facts.get(k, 0) + 1 if v is None else v
        if facts.get(k, 0) == v or (k, v) in tried:
            continue
        cands.append((2 if (k, v) in held else 1, depth, k, v))
    cands.sort()
    nop = os.environ.get("FAKE_NO_PROGRESS")
    if nop:
        cands = [(1, 0, nop, facts.get(nop, 0) + 2)]
    out["targets"]["by_tier"] = {"0": 0, "1": sum(c[0] == 1 for c in cands), "2": sum(c[0] == 2 for c in cands)}
    out["targets"]["first"] = [[k, v, t, dp] for t, dp, k, v in cands[:5]]
    if not cands:
        out["exhausted"] = True
        write_json(d / "result.json", out)
        return
    tier, depth, k, v = cands[0]
    out["probes"] = [{"target": [k, v], "status": "plan", "seconds": 0.0}]
    out["tried"] = [[k, v]]
    f1 = dict(facts, **{"var:990": facts.get("var:990", 0) + 1})
    f2 = dict(f1, **{k: v})
    idle_key = os.environ.get("FAKE_IDLE_KEY")
    if idle_key:
        f2[idle_key] = facts.get(idle_key, 0) + 1
        if job.get("idle_frames") and not job.get("idle_measured"):
            out["idle"] = {"returns": sorted({idle_key} | set(job["goal"])), "first_return": {idle_key: 2},
                           "changes": {idle_key: 9}, "frames": job["idle_frames"], "seconds": 0.0, "cpu_s": 0.0}
    if n >= int(os.environ.get("FAKE_GOAL_AFTER") or 4) - 1:
        f2.update({g: vals[0] for g, vals in job["goal"].items()})
    ev1, ev2 = {"frame": frame + 3, "click": [10, 10]}, {"frame": frame + 7, "key": "p"}
    x1 = {"tick": extra["tick"] + 3, "rng": extra.get("rng"), "actors": []}
    x2 = {"tick": extra["tick"] + 7, "rng": extra.get("rng"), "actors": []}
    cp1, cp2 = [frame + 3, f1, x1], [frame + 7, f2, x2]
    write_gz(d / "new" / "0.json.gz", {"id": None, "parent": e["id"], "frame": frame + 3, "tick": x1["tick"],
                                       "events": [ev1], "checkpoints": [cp1]})
    write_gz(d / "new" / "1.json.gz", {"id": None, "parent": e["id"], "frame": frame + 7, "tick": x2["tick"],
                                       "events": [ev1, ev2], "checkpoints": [cp1, cp2]})
    out["new"] = [{"file": "new/0.json.gz", "frame": frame + 3, "tick": x1["tick"], "how": "checkpoint"},
                  {"file": "new/1.json.gz", "frame": frame + 7, "tick": x2["tick"], "how": "end"}]
    out["attempt"] = {"target": [k, v], "stop": "goal", "reached": True, "goal_from_the_plan": True, "frames": 7,
                      "qf0": 0, "qf_end": 168, "replans": 0, "refinements": 0, "extensions": 0,
                      "window_summary": None, "seconds": 0.0}
    out["wall_s"] = round(time.monotonic() - t0, 3)
    write_json(d / "result.json", out)
    print(f"{job['job']}: entry {e['id']} target {k}={v} (tier {tier}, depth {depth})", flush=True)


if __name__ == "__main__":
    main()
