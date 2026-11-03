"""Gate 5 of plan.md's amendment "targets with no plan made cheap": the deepening on fake steps.

usage (from the repo root): .venv/bin/python experiments/E011-speed/deadtarget/deepen_check.py --fmi BIN [--out DIR]

On E009's test archive (rotation on; a guide with the two targets below and one room), ``fmi explore run
--fake`` runs with FMI_PLAN_DEEPEN=10 and FAKE_SLOW giving one target 15 s of planning (found at the second limit)
and one no plan ("inf"). Checked:

1. a probe's limit is 10 * 2^n for a target deferred n times before, and a timed-out probe defers it (n + 1);
2. within a try, the probes come undeferred first, then by n; a deferred target is planned only after every target
   before it in the try timed out (its entry has no undeferred target left);
3. the 15 s target is found at 20 s; the target with no plan is never "tried" (never dropped), its n only grows;
4. an entry whose try deferred a target is not exhausted;
5. a run killed after two committed tries and resumed writes the same log lines (time fields left out) as a run
   that was not killed.

This is a test tool (fake steps only); it edits no reference file.
"""

import argparse
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT / "experiments" / "E011-speed" / "explore"))
import g9_driver as G  # noqa: E402

T9, T, X = G.T9, G.T, G.X
L0 = 10
SLOW = {"state:316=1": 15, "bit:7=1": float("inf")}


def env_for(base: Path) -> dict:
    return G.env_of(base, {"FMI_PLAN_DEEPEN": str(L0), "FAKE_SLOW": ",".join(f"{k}:{v}" for k, v in SLOW.items()),
                           "FAKE_GOAL_AFTER": "40", "FAKE_SLEEP_S": "0"})


def make(base: Path) -> Path:
    """E009's test archive (test_e009.make_e009, rotation on) with a guide that offers the slow target, the one with
    no plan, and room 28 (one step of progress), so that entries are tried again with their deferred targets."""
    shutil.rmtree(base, ignore_errors=True)
    base.mkdir(parents=True)
    guide = dict(T9.GUIDE, atoms=[["owner:566", 1, 0], ["state:316", 1, 2], ["bit:7", 1, 3], ["room", 28, 3]],
                 keys={"owner:566": 0, "state:316": 2, "bit:7": 3, "room": 3})
    gp = base / "GUIDE.json"
    gp.write_text(json.dumps(guide))
    a = X.Archive.create(base / "arch", "test", {"owner:566": [1]}, gp, [28, 33, 35, 38, 41], 2,
                         {"frame": 100, "tick": 0, "qf": 1000, "room": 33}, {"rotate": True, "stop_jobs": 14})
    a.add(None, 100, 0, [], [T9.cp(100, dict(T9.ROOT_FACTS), 0)], None, "boot")
    a.write_cells()
    return a.root


def results(arch: Path) -> list:
    out = []
    for d in sorted((arch / "jobs").iterdir()):
        p = d / "result.json"
        if p.exists():
            out.append((d.name, json.loads((d / "job.json").read_text()), json.loads(p.read_text())))
    return out


def check(arch: Path) -> list:
    bad = []
    logs = [json.loads(x) for x in (arch / "log.jsonl").read_text().splitlines()]
    found_slow = False
    dead_n = {}
    for name, job, res in results(arch):
        probes = res.get("probes", [])
        ns = []
        for i, p in enumerate(probes):
            n = p.get("deferred_before")
            if n is None or p.get("limit_s") != L0 * 2 ** n:
                bad.append(f"{name}: probe {i} limit {p.get('limit_s')} for n {n}")
                continue
            ns.append(n)
            kv = f"{p['target'][0]}={p['target'][1]}"
            if n >= 1 and any(q["status"] != "time" for q in probes[:i]):
                bad.append(f"{name}: {kv} deferred {n} times planned after a probe that did not time out")
            if p["status"] == "time":
                if [p["target"][0], p["target"][1], n + 1] not in res.get("deferred", []):
                    bad.append(f"{name}: {kv} timed out and is not deferred with n {n + 1}")
            if kv == "state:316=1" and p["status"] == "plan":
                found_slow = found_slow or p["limit_s"] >= SLOW[kv]
            if kv == "bit:7=1":
                e = res["entry"]
                if n < dead_n.get(e, 0):
                    bad.append(f"{name}: bit:7=1 at entry {e} went back to n {n}")
                dead_n[e] = n + 1
        if ns != sorted(ns):
            bad.append(f"{name}: probes not in deferral order {ns}")
        if res.get("deferred") and res.get("exhausted"):
            bad.append(f"{name}: deferred targets and exhausted")
    for x in logs:
        if any(t[:2] == ["bit:7", 1] for t in (x.get("tried") or [])):
            bad.append(f"{x['job']}: bit:7=1 tried (dropped)")
        if x.get("exhausted") and x.get("deferred"):
            bad.append(f"{x['job']}: entry {x['entry']} exhausted with deferred targets")
    if not found_slow:
        bad.append("state:316=1 was never found at a limit of 15 s or more")
    if not any(n >= 2 for n in dead_n.values()):
        bad.append(f"bit:7=1 was not retried with a doubled limit: {dead_n}")
    return bad


def run(fmi: str, arch: Path, env: dict) -> None:
    p = subprocess.run([fmi, "explore", "run", str(arch), "--workers", "1", "--no-verify", "--fake"], env=env,
                       capture_output=True, text=True, timeout=600)
    if p.returncode != 0:
        raise SystemExit(f"fmi failed: {p.stderr[-2000:]}")


def run_killed(fmi: str, arch: Path, env: dict) -> None:
    cmd = [fmi, "explore", "run", str(arch), "--workers", "1", "--no-verify", "--fake"]
    t = time.monotonic()
    p = subprocess.Popen(cmd, env=dict(env, FAKE_SLEEP_S="0.3"), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    while len((arch / "log.jsonl").read_text().splitlines()) < 2 and time.monotonic() - t < 60:
        time.sleep(0.02)
    p.kill()
    p.wait()
    run(fmi, arch, env)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fmi", required=True)
    ap.add_argument("--out", default="/tmp/deepen-check")
    a = ap.parse_args()
    out = Path(a.out)
    one = make(out / "once")
    run(a.fmi, one, env_for(out / "once"))
    bad = check(one)
    two = make(out / "killed")
    run_killed(a.fmi, two, env_for(out / "killed"))
    bad += [f"killed run: {b}" for b in check(two)]
    norm = lambda arch: [json.dumps(G.strip(json.loads(x), "log"), sort_keys=True).replace(str(arch.parent), "BASE")
                         for x in (arch / "log.jsonl").read_text().splitlines()]
    if norm(one) != norm(two):
        bad.append("the killed and resumed run's log differs from the uninterrupted one")
    logs = [json.loads(x) for x in (one / "log.jsonl").read_text().splitlines()]
    print(f"{len(logs)} tries; deferred lines {sum(1 for x in logs if x.get('deferred'))}; "
          f"probes {sum(len(r.get('probes', [])) for _, _, r in results(one))}")
    for b in bad:
        print("BAD", b)
    print("deepen check:", "pass" if not bad else f"FAIL ({len(bad)})")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
