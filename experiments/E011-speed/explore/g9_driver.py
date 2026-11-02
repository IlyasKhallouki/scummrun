"""G9, the driver's part (experiments/E011-speed/plan.md, "Exactness gates"): on fake steps, ``fmi explore run``
writes the same archive as explore.py with fake_step.py, for each switch set of test_explore.py and test_e009.py
(and E010's two switches).

usage (from the repo root): .venv/bin/python experiments/E011-speed/explore/g9_driver.py [--fmi BIN] [--only NAME]
       [--out DIR] [--modes inproc,proc]

Each scenario makes its archive once with E008's own fixtures (test_explore.make_archive, test_e009.make_e009),
runs explore.py with fake_step.py on it, then the same archive, at the same path, with ``fmi explore run --fake``
(the fake step inside fmi) and with ``fmi explore run --step-cmd "fmi fake-step"`` (a process per job, as explore.py
runs it). The archives are compared file by file: entries.jsonl, cells.json, log.jsonl, summary.json and
readings.jsonl, every jobs/J/job.json, select.json and result.json, and the entries' and new entries' files
(decompressed), with the time and pid fields left out (log.jsonl started, ended, driver_wall_s, rusage, guard, and the step's wall_s;
summary.json time; select.json time; result.json wall_s), and log.jsonl's lines are also compared as bytes with those
fields' values blanked. For goal-2w the run is also killed once two jobs are committed (the driver alone, or the driver
and its jobs) and resumed, and the result compared with explore.py's uninterrupted run, with rc left out too (a job a
resume finds ended has no exit code, in explore.py as here). The driver's numbers go to the out dir.

This is a test tool: it runs the frozen reference as it is and edits none of its files.
"""

import argparse
import re
import gzip
import json
import os
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
E008 = ROOT / "experiments" / "E008-go-explore"
sys.path.insert(0, str(E008))

import explore as X  # noqa: E402
import test_e009 as T9  # noqa: E402
import test_explore as T  # noqa: E402

PY = sys.executable
TIME_FIELDS = {"log": {"started", "ended", "driver_wall_s", "rusage", "guard", "wall_s"}, "summary": {"time"},
               "select": {"time"}, "result": {"wall_s"}}


def scenarios():
    """(name, how to make the archive, env, workers): the loop tests of test_explore.py and test_e009.py, and the
    switch sets they use, plus E010's."""
    g = T.GUIDE
    out = [
        ("goal-2w", lambda d: T.make_archive(d), {}, 2),
        ("guard-once", lambda d: T.make_archive(d), {"FAKE_GUARD_ONCE": "@MARKER"}, 2),
        ("fail-2", lambda d: T.make_archive(d), {"FAKE_FAIL": "2"}, 2),
        ("fail-root", lambda d: T.make_archive(d), {"FAKE_FAIL": "0"}, 2),
        ("differ-root", lambda d: T.make_archive(d), {"FAKE_DIFFER": "0"}, 2),
        ("exhausted", lambda d: T.make_archive(d, guide=dict(g, atoms=[["owner:566", 1, 0]],
                                                               keys={"owner:566": 0})), {}, 2),
        ("floor", lambda d: T.make_archive(d), {"E008_FLOOR_GB": "1e9"}, 2),
        ("stagnation", lambda d: T.make_archive(d), {"FAKE_NO_PROGRESS": "state:316"}, 2),
        ("idle-off", lambda d: T.make_archive(d),
         {"FAKE_NO_PROGRESS": "state:316", "FAKE_IDLE_KEY": "var:103", "FAKE_GOAL_AFTER": "7"}, 2),
        ("idle-on", lambda d: T9.make_e009(d),
         {"FAKE_NO_PROGRESS": "state:316", "FAKE_IDLE_KEY": "var:103", "FAKE_GOAL_AFTER": "7"}, 2),
        ("bound", lambda d: T9.make_e009(d, stop_jobs=3), {"FAKE_GOAL_AFTER": "50"}, 2),
        ("bound-goal", lambda d: T9.make_e009(d, stop_jobs=3), {"FAKE_GOAL_AFTER": "2"}, 2),
        ("e009-all", lambda d: T9.make_e009(d, idle_frames=1600, rotate=True, evict_by_size=True,
                                            reuse_nonfacts=True, stop_jobs=12),
         {"FAKE_IDLE_KEY": "var:103", "FAKE_GOAL_AFTER": "9"}, 2),
        ("e009-rotate", lambda d: T9.make_e009(d, rotate=True), {"FAKE_GOAL_AFTER": "8"}, 2),
        ("e010-ready", lambda d: e010(d, ready=True), {"FAKE_GOAL_AFTER": "6"}, 2),
        ("e010-commit-finished-1w", lambda d: e010(d, ready=True, commit_finished=True), {"FAKE_GOAL_AFTER": "6"}, 1),
        ("goal-1w", lambda d: T.make_archive(d), {"FAKE_GOAL_AFTER": "5"}, 1),
        ("goal-3w", lambda d: T.make_archive(d), {"FAKE_GOAL_AFTER": "8"}, 3),
    ]
    return out


def e010(tmp: Path, **sw) -> X.Archive:
    """An archive with E010's switches in stage.json (the E010 patch's ``Archive.create(..., e010=...)``)."""
    a = T9.make_e009(tmp, idle_frames=1600)
    st = json.loads((a.root / "stage.json").read_text())
    st["e010"] = dict(sw)
    (a.root / "stage.json").write_text(json.dumps(st, indent=1) + "\n")
    return X.Archive.open(a.root)


def env_of(base: Path, extra: dict) -> dict:
    env = T.loop_env(base)
    for k, v in extra.items():
        env[k] = str(v).replace("@MARKER", str(base / "marker"))
    return env


def e010_reference(out: Path) -> Path:
    """explore.py with E010's patch (its explore.py part; ~/.cache/fast-mi-e011/e010-play-when-ready-wip.patch), in a
    copy of E008's folder under the out dir: the reference for E010's switches."""
    d = out / "e010-reference"
    shutil.rmtree(d, ignore_errors=True)
    shutil.copytree(E008, d / "experiments" / "E008-go-explore", ignore=shutil.ignore_patterns("__pycache__"))
    patch = Path.home() / ".cache/fast-mi-e011/e010-play-when-ready-wip.patch"
    p = subprocess.run(["git", "apply", "--include=experiments/E008-go-explore/explore.py", str(patch)], cwd=d,
                       capture_output=True, text=True)
    if p.returncode != 0:
        raise SystemExit(f"the E010 patch does not apply: {p.stderr}")
    return d / "experiments" / "E008-go-explore"


def run_py(arch: Path, env: dict, workers: int, here: Path = E008) -> float:
    t = time.monotonic()
    cmd = [PY, "-u", str(here / "explore.py"), "run", str(arch), "--workers", str(workers), "--no-verify",
           "--step-cmd", f"{PY} -u {E008 / 'fake_step.py'}"]
    p = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=600)
    if p.returncode != 0:
        raise SystemExit(f"explore.py failed: {p.stderr[-2000:]}")
    return time.monotonic() - t


def run_fmi(fmi: str, arch: Path, env: dict, workers: int, mode: str) -> tuple:
    cmd = [fmi, "explore", "run", str(arch), "--workers", str(workers), "--no-verify"]
    cmd += ["--fake"] if mode == "inproc" else ["--step-cmd", f"{fmi} fake-step"]
    t = time.monotonic()
    p = subprocess.run(cmd, env=dict(env, FMI_DRIVER_STATS="1"), capture_output=True, text=True, timeout=600)
    if p.returncode != 0:
        raise SystemExit(f"fmi failed: {p.stderr[-2000:]}")
    stats = [ln for ln in p.stderr.splitlines() if ln.startswith("driver:")]
    return time.monotonic() - t, stats[-1] if stats else ""


def strip(d: dict, kind: str) -> dict:
    return {k: v for k, v in d.items() if k not in TIME_FIELDS[kind]}


def gz_text(p: Path) -> str:
    with gzip.open(p, "rt") as f:
        return f.read()


def snapshot(arch: Path) -> dict:
    """Every compared file of an archive, as text or parsed and stripped of the time and pid fields."""
    out = {}
    for name in ("entries.jsonl", "cells.json", "readings.jsonl"):
        p = arch / name
        out[name] = p.read_text() if p.exists() else None
    out["log.jsonl"] = [strip(json.loads(x), "log") for x in (arch / "log.jsonl").read_text().splitlines()]
    out["summary.json"] = strip(json.loads((arch / "summary.json").read_text()), "summary")
    out["summary.text"] = [ln for ln in (arch / "summary.json").read_text().splitlines() if '"time"' not in ln]
    for p in sorted((arch / "entries").glob("*.json.gz")):
        out[f"entries/{p.name}"] = json.loads(gz_text(p))  # the gz bytes hold an mtime; the JSON is compared
    for d in sorted((arch / "jobs").iterdir()):
        for f in ("job.json", "select.json", "result.json"):
            p = d / f
            if not p.exists():
                out[f"jobs/{d.name}/{f}"] = None
                continue
            if f == "job.json":
                out[f"jobs/{d.name}/{f}"] = p.read_text()
            else:
                kind = "select" if f == "select.json" else "result"
                out[f"jobs/{d.name}/{f}"] = strip(json.loads(p.read_text()), kind)
        for p in sorted((d / "new").glob("*.json.gz")) if (d / "new").exists() else ():
            out[f"jobs/{d.name}/new/{p.name}"] = gz_text(p)
    rd = arch / "readings"
    for p in sorted(rd.rglob("reading.json")) if rd.exists() else ():
        out[f"readings/{p.relative_to(rd)}"] = p.read_text()
    return out


TIME_TEXT = [re.compile(r'"(started|ended|driver_wall_s|wall_s)":(-?[0-9.eE+-]+|null)'),
             re.compile(r'"(rusage)":(\{[^}]*\}|null)'), re.compile(r'"(guard)":("(?:[^"\\]|\\.)*"|null)')]


def lines_text(arch: Path) -> list:
    """log.jsonl's lines as their bytes, the time fields' values replaced by "T": a byte comparison of the writer."""
    out = []
    for x in (arch / "log.jsonl").read_text().splitlines():
        for r in TIME_TEXT:
            x = r.sub(lambda m: f'"{m[1]}":"T"', x)
        out.append(x)
    return out


def run_fmi_killed(fmi: str, arch: Path, env: dict, workers: int, mode: str, how: str) -> float:
    """fmi run killed once two jobs are committed (``how`` "driver": the driver alone, its job processes go on;
    "tree": the driver and its jobs), then run again to its stop (test_explore.py's resume test)."""
    cmd = [fmi, "explore", "run", str(arch), "--workers", str(workers), "--no-verify"]
    cmd += ["--fake"] if mode == "inproc" else ["--step-cmd", f"{fmi} fake-step"]
    t = time.monotonic()
    p = subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    while len((arch / "log.jsonl").read_text().splitlines()) < 2 and time.monotonic() - t < 60:
        time.sleep(0.02)
    time.sleep(0.1)
    if how == "driver":
        p.send_signal(signal.SIGKILL)
    else:
        X.kill_tree(p.pid)
    p.wait()
    if (arch / "summary.json").exists():
        raise SystemExit("the run stopped before the kill")
    t_rs, _ = run_fmi(fmi, arch, env, workers, mode)
    return time.monotonic() - t


def diff(a: dict, b: dict) -> list:
    out = []
    for k in sorted(set(a) | set(b)):
        if a.get(k) != b.get(k):
            out.append(k)
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fmi", default=os.environ.get("FMI_BIN") or "/var/tmp/fast-mi/target-e011-explore/release/fmi")
    ap.add_argument("--only", default=None)
    ap.add_argument("--out", default=str(Path.home() / ".cache/fast-mi-e011/explore/g9"))
    ap.add_argument("--modes", default="inproc,proc")
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    modes = a.modes.split(",")
    rows, bad = [], 0
    ref010 = None
    for name, make, extra, workers in scenarios():
        if a.only and a.only != name:
            continue
        base = out / name
        shutil.rmtree(base, ignore_errors=True)
        base.mkdir(parents=True)
        arch = make(base).root
        fresh = base / "fresh"
        shutil.copytree(arch, fresh, symlinks=True)
        env = env_of(base, extra)
        (base / "marker").unlink(missing_ok=True)
        if name.startswith("e010") and ref010 is None:
            ref010 = e010_reference(out)
        t_py = run_py(arch, env, workers, ref010 if name.startswith("e010") else E008)
        ref = snapshot(arch)
        ref_lines = lines_text(arch)
        shutil.move(arch, base / "ref")
        row = {"scenario": name, "workers": workers, "env": extra, "stop": ref["summary.json"].get("stop"),
               "jobs": ref["summary.json"].get("jobs"), "py_s": round(t_py, 2)}
        for mode in modes:
            shutil.rmtree(arch, ignore_errors=True)
            shutil.copytree(fresh, arch, symlinks=True)
            (base / "marker").unlink(missing_ok=True)
            t_rs, stats = run_fmi(a.fmi, arch, env, workers, mode)
            got = snapshot(arch)
            d = diff(ref, got)
            same_text = lines_text(arch) == ref_lines
            row[mode] = {"same": not d and same_text, "differ": d[:12], "fmi_s": round(t_rs, 2), "stats": stats}
            if d or not same_text:
                bad += 1
            shutil.move(arch, base / f"fmi-{mode}")
        if name == "goal-2w":  # test_explore.py: a run killed mid-way resumes to the archive an uninterrupted run gives
            for mode in modes:
                for how in (("driver", "tree") if mode == "proc" else ("tree",)):
                    shutil.rmtree(arch, ignore_errors=True)
                    shutil.copytree(fresh, arch, symlinks=True)
                    t_rs = run_fmi_killed(a.fmi, arch, env, workers, mode, how)
                    # a job a resume found ended has no exit code (explore.py's too): rc is left out here
                    got, want = snapshot(arch), dict(ref)
                    for s_ in (got, want):
                        s_["log.jsonl"] = [{k: v for k, v in x.items() if k != "rc"} for x in s_["log.jsonl"]]
                    d = diff(want, got)
                    same_text = [re.sub(r'"rc":(-?\d+|null)', '"rc":"P"', x) for x in lines_text(arch)] == \
                        [re.sub(r'"rc":(-?\d+|null)', '"rc":"P"', x) for x in ref_lines]
                    row[f"{mode}-killed-{how}"] = {"same": not d and same_text, "differ": d[:12], "fmi_s": round(t_rs, 2)}
                    if d or not same_text:
                        bad += 1
                    shutil.move(arch, base / f"fmi-{mode}-killed-{how}")
        rows.append(row)
        print(json.dumps(row), flush=True)
    (out / "g9-driver.jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))
    print(f"G9 driver: {len(rows)} scenarios x {len(modes)} modes, {bad} differing")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
