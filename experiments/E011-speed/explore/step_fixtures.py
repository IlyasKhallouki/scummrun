"""Fixtures for G9's step part (experiments/E011-speed/plan.md, "Exactness gates"): step.py's ``targets`` (with E009's
rotation), ``idle_rule`` and ``reusable`` on recorded inputs, their outputs from the frozen reference, for
``fmi check step`` to compare against.

usage (from the repo root): .venv/bin/python experiments/E011-speed/explore/step_fixtures.py [--out DIR]

Inputs, all recorded by earlier runs (nothing is run here but step.py's functions):

* targets: every job.json of the recorded archives (the entry's facts, ``held``, ``tried``, the goal, the guide), each
  with its own reading's reached values when they are still on disk and with two other recorded reached files; once
  without and once with a rotation made from the archive's log (the targets the jobs before it tried, from its cell
  and in all, as explore.py's ``rotation`` counts them);
* reusable: every recorded reading whose packed file is on disk, against the entries of the jobs, through
  ``reusable`` and ``reusable_detail`` with the reading's task facts (the packed file's; their keys are written next to
  the fixtures, as the reading's ``domains`` would give them);
* idle_rule: every recorded entry file with three checkpoints or more, its checkpoints after the first as the frames.

Out: DIR/step-check.jsonl (one case a line: kind, inputs as paths and small values, the reference's output), and
DIR/task-keys/*.json. This is a test tool: it imports the frozen reference and edits none of its files.
"""

import argparse
import gzip
import hashlib
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
E008 = ROOT / "experiments" / "E008-go-explore"
sys.path.insert(0, str(E008))

import step  # noqa: E402

HOME = Path.home()
ARCHIVES = [HOME / ".cache/fast-mi-e008/archives/kitchen", HOME / ".cache/fast-mi-e008/archives/door",
            HOME / ".cache/fast-mi-e008/archives/smoke", HOME / ".cache/fast-mi-e009/archives/door"]
EXTRA_JOBS = [HOME / ".cache/fast-mi-e009/off/step-a/jobs/ta", HOME / ".cache/fast-mi-e008/step-tests/jobs/ta",
              HOME / ".cache/fast-mi-e008/step-tests/jobs/tb"]


def reached_files() -> list:
    out = []
    for base in (HOME / ".cache").glob("fast-mi-e0*"):
        out += sorted(base.rglob("reading.reached"))
    return sorted(set(out))


def jobs() -> list:
    out = []
    for a in ARCHIVES:
        if (a / "jobs").exists():
            out += sorted(p for p in (a / "jobs").iterdir() if (p / "job.json").exists())
    out += [p for p in EXTRA_JOBS if (p / "job.json").exists()]
    return out


def rotations(arch: Path) -> dict:
    """job -> a rotation made from the log: the targets the committed jobs before it tried (their result's
    ``tried``), counted from its entry's cell and in all."""
    log = arch / "log.jsonl"
    if not log.exists():
        return {}
    lines = [json.loads(x) for x in log.read_text().splitlines() if x.strip()]
    rows = [json.loads(x) for x in (arch / "entries.jsonl").read_text().splitlines()] if \
        (arch / "entries.jsonl").exists() else []
    cell = {r["id"]: r["cell"] for r in rows}
    out, seen = {}, []
    for x in lines:
        if x.get("outcome") not in ("ok", "error", "return differs"):
            continue
        res = arch / "jobs" / x["job"] / "result.json"
        tried = json.loads(res.read_text()).get("tried") or [] if res.exists() else []
        c = cell.get(x.get("entry"))
        cc, aa = {}, {}
        for (c2, ts) in seen:
            for t in ts:
                k = (t[0], t[1])
                aa[k] = aa.get(k, 0) + 1
                if c2 == c:
                    cc[k] = cc.get(k, 0) + 1
        out[x["job"]] = {"cell": [[k, v, n] for (k, v), n in sorted(cc.items(), key=lambda kv: json.dumps(list(kv[0])))],
                         "all": [[k, v, n] for (k, v), n in sorted(aa.items(), key=lambda kv: json.dumps(list(kv[0])))]}
        seen.append((c, tried))
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(HOME / ".cache/fast-mi-e011/explore/fixtures/step"))
    a = ap.parse_args()
    out = Path(a.out)
    (out / "task-keys").mkdir(parents=True, exist_ok=True)
    cases = []
    rf = reached_files()
    js = jobs()
    rots = {}
    for arch in ARCHIVES:
        rots.update({(str(arch), k): v for k, v in rotations(arch).items()})
    for n, jd in enumerate(js):
        job = json.loads((jd / "job.json").read_text())
        guide = json.loads(Path(job["guide"]).read_text())
        facts = job["entry"]["checkpoint"][1]
        root = step.root_facts_of(job, jd)
        own = []
        res = jd / "result.json"
        if res.exists():
            rd = (json.loads(res.read_text()).get("reading") or {}).get("path")
            if rd and Path(rd).with_name("reading.reached").exists():
                own.append(Path(rd).with_name("reading.reached"))
        picks = own + [rf[(3 * n) % len(rf)], rf[(3 * n + 1) % len(rf)]]
        goal = {k: list(v) for k, v in job["goal"].items()}
        rot = rots.get((str(jd.parent.parent), job["job"]))
        for p in picks:
            for r in (None, rot) if rot is not None else (None,):
                ts = step.targets(p, guide, facts, root, job.get("held") or (), job.get("tried") or (), goal, r)
                cases.append({"kind": "targets", "job_dir": str(jd), "reached": str(p), "rotation": r,
                              "root_facts": root is not None, "out": [list(t) for t in ts]})
    # reusable: each reading on disk against some jobs' entries
    import packed_reading as PR
    readings = sorted({p.with_name("reading.json") for p in rf if p.with_name("reading.json").exists()
                       and p.with_name("reading.tc").exists()})
    for i, rp in enumerate(readings):
        try:
            rd = PR.load_reading(rp)
        except Exception as e:  # noqa: BLE001 (an older format: left out, and said)
            print(f"{rp}: not loaded ({type(e).__name__}: {e})", file=sys.stderr)
            continue
        keys = sorted(rd.task.facts)
        kf = out / "task-keys" / (hashlib.sha1(str(rp).encode()).hexdigest()[:16] + ".json")
        kf.write_text(json.dumps(keys) + "\n")
        for jd in js[i % 7::max(1, len(js) // 12)][:12]:
            job = json.loads((jd / "job.json").read_text())
            facts = job["entry"]["checkpoint"][1]
            ok, nb = step.reusable(rd.doc, rd.task.facts, facts)
            det = step.reusable_detail(rd.doc, rd.task.facts, facts)
            cases.append({"kind": "reusable", "reading": str(rp), "task_keys": str(kf), "job_dir": str(jd),
                          "out": [ok, nb], "detail": det})
        rd.close()
    # idle_rule on recorded chains of checkpoints
    for arch in ARCHIVES:
        for p in sorted((arch / "entries").glob("*.json.gz")) if (arch / "entries").exists() else ():
            with gzip.open(p, "rt") as f:
                e = json.load(f)
            cps = e["checkpoints"]
            if len(cps) < 3:
                continue
            start = cps[0][1]
            frames = [(c[0] - cps[0][0], c[1]) for c in cps[1:]]
            r = step.idle_rule(start, frames)
            cases.append({"kind": "idle_rule", "entry_file": str(p), "out": r})
    (out / "step-check.jsonl").write_text("".join(json.dumps(c) + "\n" for c in cases))
    kinds = {}
    for c in cases:
        kinds[c["kind"]] = kinds.get(c["kind"], 0) + 1
    print(f"{len(cases)} cases {kinds} from {len(js)} jobs, {len(rf)} reached files, {len(readings)} readings -> "
          f"{out / 'step-check.jsonl'}")


if __name__ == "__main__":
    main()
