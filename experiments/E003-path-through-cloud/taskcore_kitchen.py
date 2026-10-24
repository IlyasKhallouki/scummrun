"""E003 step 4, the memory work, stage 1: taskcore (the Rust crate) on the kitchen task, against task.py.

usage: READER_PICKLE=<reading.pkl> [TASKCORE_BIN=<binary>] python taskcore_kitchen.py <step> <work dir> [args]
  pyref <pruned|full> <all|no211>   task.py on the kitchen: build_task (and relevance + prune for "pruned", as
                                   test_reader.plan_of), write_sas, the file for each start; times, peak memory and
                                   the files' sha1 (work/pyref-<...>.json); the pruned task also as a packed file
  fdbase                           lama-first on task.py's own file of the pruned task (the baseline on this machine)
  format                           taskcore with no fold, no reduction and task.py's encoding on the same tasks: byte
                                   for byte (against task.py's file, or the sha1 and size pyref recorded for it)
  table                            taskcore with each lever and each reduction, alone and together (work/rows/)
  foldcheck                        taskcore's fold check on the kitchen, strict and every end
  plans ["args" ...]               lama-first on the reduced kitchen tasks (one taskcore configuration per argument, ""
                                   the default), each plan read back and replayed
  anytime [seconds]                anytime LAMA (seq-sat-lama-2011, default 120 s) on the default reduced tasks: its
                                   last (best) plan read back and replayed
  known                            test_reader's three plans run on each reduced task (taskcore validate)
  log                              results/taskcore-kitchen.log from the work directory's results

The work directory holds kitchen.tc and kitchen-no211.tc (export_records.py kitchen). Every big file is deleted once
its sha1 and size are taken.
"""

import collections
import hashlib
import json
import os
import resource
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))
BIN = Path(os.environ.get("TASKCORE_BIN", HERE / "taskcore" / "target" / "release" / "taskcore"))
LOG = HERE / "results" / "taskcore-kitchen.log"


def rss_gb() -> float:
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1e6


def sha1_file(path: Path) -> str:
    h = hashlib.sha1()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _records(which: str):
    import test_reader as TR
    b, recs, _ = TR.built()
    if which == "no211":
        recs = [r for r in recs if r.label != "event(local:211@33)"]
    return b, recs


def _meta(a):
    """(kind, end, cont) of a build_task action, from its record, for export_task."""
    from export_records import _cont
    r = a.what
    return r.kind, r.end, _cont(r)


def pyref(work: Path, variant: str, which: str) -> None:
    """task.py's own pipeline on the kitchen (module docstring)."""
    import reader as R
    import relevance
    import task as T
    import test_reader as TR
    from export_records import export_task
    out = {"variant": variant, "records": which, "seconds": {}, "rss_gb": {}}

    def mark(stage, t0):
        out["seconds"][stage] = round(time.monotonic() - t0, 2)
        out["rss_gb"][stage] = round(rss_gb(), 3)
    t0 = time.monotonic()
    b, recs = _records(which)
    mark("load the reading", t0)
    t0 = time.monotonic()
    task = R.build_task(b, recs, TR.goal())
    mark("build_task", t0)
    out["build_task actions"] = len(task.actions)
    if variant == "pruned":
        t0 = time.monotonic()
        rel = relevance.relevant(task)
        mark("relevance", t0)
        t0 = time.monotonic()
        task = R.prune(task, rel)
        mark("prune", t0)
        out["pruned actions"] = len(task.actions)
    starts = {"s17": R.task_start(b, TR.start(), task)}
    if which == "all":
        starts["replan"] = R.replan_start(b, TR.replan_observed(), {}, task)[0]
    if variant == "pruned":
        t0 = time.monotonic()
        out["export"] = export_task(work / f"kitchen-pruned-{which}.tc", task, starts, meta=_meta)
        mark("export_task", t0)
    del recs
    t0 = time.monotonic()
    sas = T.write_sas(task)
    mark("write_sas", t0)
    out["counts"] = dict(sas.counts)
    out["files"] = {}
    for name, st in starts.items():
        t0 = time.monotonic()
        text = sas.text(st)
        path = work / f"py-{variant}-{which}-{name}.sas"
        path.write_text(text)
        out["files"][name] = {"path": str(path), "bytes": len(text.encode()), "sha1": hashlib.sha1(text.encode()).hexdigest()}
        del text
        mark(f"text {name}", t0)
    out["peak_rss_gb"] = round(rss_gb(), 3)
    (work / f"pyref-{variant}-{which}.json").write_text(json.dumps(out, indent=1))
    print(json.dumps(out, indent=1))


def taskcore(args: list, stats: Path | None = None) -> dict:
    """Run the binary; its stats JSON."""
    cmd = [str(BIN)] + [str(a) for a in args]
    if stats is not None:
        cmd += ["--stats", str(stats)]
    t0 = time.monotonic()
    p = subprocess.run(cmd, capture_output=True, text=True)
    wall = time.monotonic() - t0
    if p.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)}: {p.stderr}")
    st = json.loads(stats.read_text()) if stats is not None else {}
    st["wall"] = wall
    st["child_peak_rss"] = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss * 1024
    return st


FORMAT = [  # (what, packed file, start, task.py's file)
    ("build_task's task, all records, start s17", "kitchen.tc", "s17", "py-full-all-s17.sas"),
    ("build_task's task, all records, start replan", "kitchen.tc", "replan", "py-full-all-replan.sas"),
    ("test_reader's pruned task, all records, start s17", "kitchen-pruned-all.tc", "s17", "py-pruned-all-s17.sas"),
    ("test_reader's pruned task, all records, start replan", "kitchen-pruned-all.tc", "replan", "py-pruned-all-replan.sas"),
    ("test_reader's pruned task, no 211, start s17", "kitchen-pruned-no211.tc", "s17", "py-pruned-no211-s17.sas"),
]


def first_difference(a: Path, b: Path):
    with open(a, "rb") as fa, open(b, "rb") as fb:
        for n, (x, y) in enumerate(zip(fa, fb), 1):
            if x != y:
                return n, x[:200].decode(errors="replace"), y[:200].decode(errors="replace")
    return None


def _recorded(work: Path, pyf: str):
    """(size, sha1) of task.py's file ``pyf`` as pyref recorded it (work/pyref-<variant>-<which>.json), or None."""
    _, variant, which, start = Path(pyf).stem.split("-")
    ref = work / f"pyref-{variant}-{which}.json"
    if not ref.exists():
        return None
    f = json.loads(ref.read_text())["files"].get(start)
    return (f["bytes"], f["sha1"]) if f else None


def format_check(work: Path) -> None:
    """taskcore with no fold, no reduction and task.py's encoding against task.py's files (module docstring): the file
    itself when it is there (a file that matches is deleted, one that does not is kept for a look), else the size and
    sha1 pyref recorded for it (the inputs, the pickle and task.py, unchanged since)."""
    rows = []
    for what, tc, start, pyf in FORMAT:
        py = work / pyf
        rec = None if py.exists() else _recorded(work, pyf)
        if (not py.exists() and rec is None) or not (work / tc).exists():
            rows.append({"what": what, "missing": True})
            continue
        out = work / "fmt.sas"
        st = taskcore(["sas", "--task", work / tc, "--start", start, "--fold", "off", "--reduce", "none",
                       "--encoding", "task", "--out", out], work / "fmt.json")
        size, sha = out.stat().st_size, sha1_file(out)
        if rec is None:
            same = py.stat().st_size == size and sha1_file(py) == sha
        else:
            same = rec == (size, sha)
        row = {"what": what, "same": same, "against": "task.py's file" if rec is None else "the recorded sha1 and size",
               "bytes": size, "sha1": sha, "operators": st["sas"]["operators"], "effects": st["sas"]["effects"],
               "seconds": st["seconds_total"], "peak_rss": st["peak_rss"]}
        if not same and rec is None:
            row["first difference (line, task.py, taskcore)"] = first_difference(py, out)
        elif rec is None:
            py.unlink()
        out.unlink()
        rows.append(row)
        print(json.dumps(row), flush=True)
    (work / "format.json").write_text(json.dumps(rows, indent=1))


def _row(name, *args):
    return name, [str(a) for a in args]


NONE = ("--fold", "off", "--reduce", "none")
ROWS = [
    _row("input: no fold, no reduction (task.py's file, metric 1)", *NONE),
    _row("fold strict alone", "--fold", "strict", "--reduce", "none"),
    _row("fold every end alone (switch)", "--fold", "every", "--reduce", "none"),
    _row("PRODUCT_LIMIT 1 alone (every guess through pick variables)", *NONE, "--product-limit", 1),
] + [_row(f"({r}) alone", "--fold", "off", "--reduce", r) for r in "abcdef"] + [
    _row("(a)-(f), no fold", "--fold", "off", "--reduce", "all"),
    _row("fold strict + (a)", "--fold", "strict", "--reduce", "a"),
    _row("fold strict + (a)(b)", "--fold", "strict", "--reduce", "a,b"),
    _row("fold strict + (a)(b)(c)", "--fold", "strict", "--reduce", "a,b,c"),
    _row("fold strict + (a)-(d)", "--fold", "strict", "--reduce", "a,b,c,d"),
    _row("fold strict + (a)-(e)", "--fold", "strict", "--reduce", "a,b,c,d,e"),
    _row("fold strict + (a)-(f) (the default)", "--fold", "strict", "--reduce", "all"),
    _row("the default + PRODUCT_LIMIT 1", "--fold", "strict", "--reduce", "all", "--product-limit", 1),
    _row("fold every end + (a)-(f)", "--fold", "every", "--reduce", "all"),
    _row("fold every end + (a)-(f) + PRODUCT_LIMIT 1", "--fold", "every", "--reduce", "all", "--product-limit", 1),
]


def table(work: Path) -> None:
    """Each row of ROWS on kitchen.tc from start s17; the file written, measured and deleted."""
    d = work / "rows"
    d.mkdir(exist_ok=True)
    rows = []
    for i, (name, args) in enumerate(ROWS):
        out = d / f"row{i}.sas"
        st = taskcore(["sas", "--task", work / "kitchen.tc", "--start", "s17", "--out", out] + args, d / f"row{i}.json")
        out.unlink()
        st["row"] = name
        rows.append(st)
        print(f"{name}: {st['actions']} actions, {st['sas']['operators']} operators, {st['sas']['effects']} effects, "
              f"{st['sas']['bytes'] / 1e6:.1f} MB, {st['seconds_total']:.1f} s, {st['peak_rss'] / 1e9:.2f} GB", flush=True)
    (work / "table.json").write_text(json.dumps(rows, indent=1))


# --- the plans ------------------------------------------------------------------------------------------------------

UNKNOWN = "<unknown>"  # a value the replay cannot know: a guess the plan did not choose, on a fact taskcore dropped


def _val(s: str):
    return int(s) if s.lstrip("-").isdigit() else s


def replay_strict(task, dsteps: list, st: dict) -> dict:
    """The plan over the full task (build_task's, nothing reduced), test_reader.replay's rules, each step one of its
    paths (the one whose precondition holds: a folded action's paths are disjoint), and every guessed write that
    fires left UNKNOWN, whatever the plan chose; an effect whose condition reads UNKNOWN makes its fact UNKNOWN; a
    precondition or goal that reads UNKNOWN fails. So the plan reaches the goal here only if no step relies on a
    guessed value."""
    state = {k: d.label(st.get(k, 0)) for k, d in task.facts.items()}
    fired, undecided, paths = [], [], []
    for i, s in enumerate(dsteps):
        cands = [m for m in s["members"] if all(state.get(k) in set(v) for k, v in task.actions[m].pre.items())]
        if len(cands) != 1:
            return {"ok": False, "step": i + 1, "why": f"{len(cands)} of its {len(s['members'])} paths apply",
                    "fired": fired, "undecided": undecided, "paths": paths}
        m = cands[0]
        paths.append(m)
        a = task.actions[m]
        new = dict(state)
        for e in a.effects:
            if any(state.get(k) == UNKNOWN for k, _ in e.when):
                new[e.key] = UNKNOWN
                undecided.append((i + 1, e.key))
                continue
            if any(state.get(k) not in set(v) for k, v in e.when):
                continue
            dom = task.facts[e.key]
            if e.values is not None:
                new[e.key] = UNKNOWN
                fired.append((i + 1, e.key))
            elif e.add is not None:
                if state[e.key] == UNKNOWN:
                    new[e.key] = UNKNOWN
                    continue
                vals = dom.add(state[e.key], e.add)
                if len(vals) == 1:
                    new[e.key] = next(iter(vals))
                else:
                    new[e.key] = UNKNOWN
                    fired.append((i + 1, e.key))
            else:
                new[e.key] = dom.label(e.value) if isinstance(e.value, int) else e.value
        state = new
    ok = all(state.get(k) in {task.facts[k].label(v) if isinstance(v, int) else v for v in vals}
             for k, vals in task.goal.items())
    return {"ok": ok, "step": None, "why": None if ok else "the goal does not hold", "fired": fired,
            "undecided": undecided, "paths": paths}


def _fd_numbers(log: str) -> dict:
    import re
    out = {}
    for key, pat in (("peak_kb", r"Peak memory: (\d+) KB"), ("search_s", r"Search time: ([\d.]+)s"),
                     ("total_s", r"Total time: ([\d.]+)s"), ("landmarks", r"Landmark graph contains (\d+) landmarks"),
                     ("expanded", r"Expanded (\d+) state"), ("length", r"Plan length: (\d+) step")):
        m = re.findall(pat, log)
        if m:
            out[key] = float(m[-1]) if "." in m[-1] else int(m[-1])
    return out


VARIANTS = [  # (name, packed file, start, records)
    ("all", "kitchen.tc", "s17", "all"),
    ("no 211", "kitchen-no211.tc", "s17", "no211"),
    ("replan", "kitchen.tc", "replan", "all"),
]


def plans(work: Path, configs: list) -> None:
    """lama-first (plan_fd's search string, through plan_fd.run) on the reduced kitchen tasks, for each taskcore
    configuration in ``configs`` (lists of extra arguments; [] is the default); each plan read back (taskcore decode)
    and replayed over the task's own actions (replay_strict, and test_reader.replay on the task with only the facts
    taskcore keeps). Results in work/plans-<config>.json."""
    import dataclasses
    from types import SimpleNamespace
    import plan_fd
    import reader as R
    import task as T
    import test_reader as TR
    results = {tuple(c): [] for c in configs}
    for which in ("all", "no211"):
        b, recs = _records(which)
        task = R.build_task(b, recs, TR.goal())
        del recs
        for extra in configs:
            for name, tc, start, w in VARIANTS:
                if w != which:
                    continue
                d = work / "plans" / ("-".join([name.replace(" ", "")] + [a.strip("-") for a in extra]))
                d.mkdir(parents=True, exist_ok=True)
                sasf = d / "task.sas"
                st = taskcore(["sas", "--task", work / tc, "--start", start, "--out", sasf, "--kept", d / "kept.json"]
                              + extra, d / "stats.json")
                text = sasf.read_text()
                t0 = time.monotonic()
                code, log, plan_text, secs, _ = plan_fd.run(text, time_limit=600, workdir=str(d / "fd"))
                del text
                fd = _fd_numbers(log)
                fd.update(exit=code, seconds=round(secs, 2), wall=round(time.monotonic() - t0, 2))
                row = {"variant": name, "args": extra,
                       "taskcore": {k: st[k] for k in ("actions", "seconds_total", "peak_rss")}, "sas": st["sas"], "fd": fd}
                row["known plan's actions in the reduced task"] = known_plan(work, name, task, d / "kept.json")
                if code == 0:
                    (d / "sas_plan").write_text(plan_text)
                    taskcore(["decode", "--task", work / tc, "--start", start, "--plan", d / "sas_plan", "--out",
                              d / "steps.json"] + extra)
                    dec = json.loads((d / "steps.json").read_text())
                    stmap = (R.task_start(b, TR.start(), task) if start == "s17"
                             else R.replan_start(b, TR.replan_observed(), {}, task)[0])
                    tracked = set(dec["facts"])
                    strict = replay_strict(task, dec["steps"], stmap)
                    row["replay over the full task"] = {k: strict[k] for k in ("ok", "step", "why", "fired", "undecided")}
                    row["steps"] = [{"op": s["op"], "taskcore": s["name"], "path": task.actions[m].name,
                                     "guessed": s["guessed"]} for s, m in zip(dec["steps"], strict["paths"])]
                    if strict["step"] is None:
                        proj = T.Task(task.facts, [dataclasses.replace(a, effects=tuple(e for e in a.effects
                                                                                        if e.key in tracked))
                                                   for a in task.actions], task.goal)
                        steps = [SimpleNamespace(action=m, name=task.actions[m].name,
                                                 guessed=tuple((k, _val(v), ()) for k, v in s["guessed"]))
                                 for s, m in zip(dec["steps"], strict["paths"])]
                        ok, bad, used, _ = TR.replay(proj, steps, stmap)
                        need = TR.needed_steps(proj, steps, stmap) if ok else steps
                        row["test_reader.replay (facts taskcore keeps)"] = {"ok": ok, "first step that cannot run": bad,
                                                                            "guessed values used": used}
                        row["steps the goal needs"] = [s.name for s in need]
                        del proj
                sasf.unlink()
                results[tuple(extra)].append(row)
                print(json.dumps(row, default=str)[:3000], flush=True)
        del task, b
    for extra, rows in results.items():
        order = {v[0]: i for i, v in enumerate(VARIANTS)}
        rows.sort(key=lambda r: order[r["variant"]])
        tag = "-".join(a.strip("-") for a in extra) or "default"
        (work / f"plans-{tag}.json").write_text(json.dumps(rows, indent=1, default=str))


def anytime(work: Path, seconds: int = 120) -> None:
    """Anytime LAMA (Fast Downward's seq-sat-lama-2011 alias, through plan_fd.run) on the default reduced kitchen
    tasks for ``seconds``: every plan it finds (length, cost, time), and the last one read back and replayed over the
    full task (replay_strict). In the model encoding (the default) its costs are the actions' own: pick operators
    cost 0. Results in work/anytime.json."""
    import re
    import plan_fd
    import reader as R
    import test_reader as TR
    out = []
    for which in ("all", "no211"):
        b, recs = _records(which)
        task = R.build_task(b, recs, TR.goal())
        del recs
        for name, tc, start, w in VARIANTS:
            if w != which:
                continue
            d = work / "anytime" / name.replace(" ", "")
            d.mkdir(parents=True, exist_ok=True)
            for old in d.glob("fd/sas_plan*"):
                old.unlink()
            sasf = d / "task.sas"
            st = taskcore(["sas", "--task", work / tc, "--start", start, "--out", sasf], d / "stats.json")
            text = sasf.read_text()
            metric = text.split("\n", 5)[4]
            code, log, _, secs, _ = plan_fd.run(text, time_limit=seconds, alias="seq-sat-lama-2011",
                                                workdir=str(d / "fd"))
            del text
            found = [(float(t), int(n), int(c)) for t, n, c in
                     re.findall(r"\[t=([\d.]+)s[^\]]*\] Plan length: (\d+) step\(s\)\.\n\[[^\]]*\] Plan cost: (\d+)", log)]
            plans = sorted(d.glob("fd/sas_plan.*"), key=lambda p: int(p.suffix[1:]))
            row = {"variant": name, "seconds": seconds, "exit": code, "wall": round(secs, 1),
                   "plans (time s, length, cost)": found, "metric": metric,
                   "taskcore": {k: st[k] for k in ("actions", "seconds_total", "peak_rss")}, "sas": st["sas"]}
            if plans:
                taskcore(["decode", "--task", work / tc, "--start", start, "--plan", plans[-1], "--out", d / "steps.json"])
                dec = json.loads((d / "steps.json").read_text())
                stmap = (R.task_start(b, TR.start(), task) if start == "s17"
                         else R.replan_start(b, TR.replan_observed(), {}, task)[0])
                strict = replay_strict(task, dec["steps"], stmap)
                row["last plan"] = [task.actions[m].name for m in strict["paths"]]
                row["replay over the full task"] = {k: strict[k] for k in ("ok", "step", "why")}
                row["guessed writes that fired"] = len(strict["fired"])
            sasf.unlink()
            out.append(row)
            print(json.dumps(row)[:2000], flush=True)
        del task, b
    (work / "anytime.json").write_text(json.dumps(out, indent=1))


def known_plan(work: Path, variant: str, task, kept_json: Path) -> list:
    """test_reader's plan for this variant (its cache next to READER_PICKLE, made on task.py's pruned task), step by
    step: is its action in the reduced task (alone or as a path of a folded action), replaced by (f), or dropped."""
    import pickle
    cache = Path(f"{os.environ['READER_PICKLE']}.plan-{variant.replace(' ', '')}-600.pkl")
    if not cache.exists():
        return ["no cached plan"]
    _, res = pickle.loads(cache.read_bytes())
    index = {a.name: i for i, a in enumerate(task.actions)}
    kept = json.loads(kept_json.read_text())
    where = {m: len(ms) for ms in kept["actions"] for m in ms}
    by = dict(kept["replaced"])
    out = []
    for s in res.steps:
        i = index[s.name]
        if i in where:
            out.append(f"{s.name}: kept" + (f" (one of the {where[i]} paths of a folded action)" if where[i] > 1 else ""))
        elif i in by:
            out.append(f"{s.name}: replaced by (f) with {task.actions[by[i]].name}")
        else:
            out.append(f"{s.name}: DROPPED")
    return out


CONFIGS = [[], ["--reduce", "none"], ["--fold", "every", "--product-limit", "1"], ["--fold", "off"]]


def known(work: Path) -> None:
    """test_reader's three plans (its caches next to READER_PICKLE, made on task.py's pruned task) run on each reduced
    task by taskcore validate: each step the reduced action holding that record's action; preconditions, the goal,
    and no guess fired on a fact the reduced task reads."""
    import pickle
    import plan_fd  # noqa: F401  (the pickles' classes)
    out = []
    for name, tc, start, _ in VARIANTS:
        cache = Path(f"{os.environ['READER_PICKLE']}.plan-{name.replace(' ', '')}-600.pkl")
        _, res = pickle.loads(cache.read_bytes())
        names = work / "known.txt"
        names.write_text("\n".join(s.name for s in res.steps) + "\n")
        for extra in CONFIGS:
            subprocess.run([str(BIN), "validate", "--task", str(work / tc), "--start", start, "--names", str(names),
                            "--out", str(work / "known.json")] + extra, check=True)
            r = json.loads((work / "known.json").read_text())
            r.update(variant=name, args=extra, length=len(res.steps))
            out.append(r)
            print(json.dumps(r)[:400], flush=True)
    (work / "known-plans.json").write_text(json.dumps(out, indent=1))


def fold_check(work: Path) -> None:
    """taskcore's fold check on kitchen.tc, strict and every end (fold.rs: corners and random states of each path's
    box, random states of the folded precondition's box)."""
    out = {}
    for mode in ("strict", "every"):
        st = taskcore(["sas", "--task", work / "kitchen.tc", "--start", "s17", "--fold", mode, "--reduce", "none",
                       "--check-fold", "4", "--out", work / "fc.sas"], work / "fc.json")
        (work / "fc.sas").unlink()
        out[f"fold {mode}"] = st["fold_check"]
        print(mode, st["fold_check"], flush=True)
    (work / "foldcheck.json").write_text(json.dumps(out, indent=1))


def fd_base(work: Path) -> None:
    """lama-first on task.py's own file of the pruned task (test_reader's), for FD's time and memory on this machine."""
    import plan_fd
    py = work / "py-pruned-all-s17.sas"
    code, log, plan_text, secs, _ = plan_fd.run(py.read_text(), time_limit=600, workdir=str(work / "fd-base"))
    fd = _fd_numbers(log)
    fd.update(exit=code, seconds=round(secs, 2))
    (work / "fd-base.json").write_text(json.dumps(fd, indent=1))
    print(fd)


def _j(work: Path, name: str):
    p = work / name
    return json.loads(p.read_text()) if p.exists() else None


def _sha(name: str) -> str:
    return hashlib.sha1((HERE / name).read_bytes()).hexdigest()[:12]


def write_log(work: Path) -> None:
    """results/taskcore-kitchen.log from the work directory's JSON files."""
    L = ["E003 step 4, the memory work, stage 1: the task side in Rust (taskcore) on the kitchen task", ""]
    L.append("files (new): export_records.py (the packed file, streaming), taskcore/ (the Rust crate, std only; "
             "README.md has the format), taskcore_kitchen.py (this log), test_taskcore.py (random tasks). Nothing else "
             "changed. This log is after the review's fixes (the last sections say what changed).")
    L.append("sources read (sha1, 12 hex): " + ", ".join(f"{n} {_sha(n)}" for n in
             ("reader.py", "task.py", "relevance.py", "domains.py", "plan_fd.py", "test_reader.py")))
    L.append("input: the pickled kitchen reading (72,256 records, rooms 28 and 41 and 33 global scripts); "
             "reader.build_task with test_reader's goal (owner:566 = 1); starts: s17 = reader.task_start of run B's "
             "S1#7 pre, replan = reader.replan_start of test_reader.replan_observed (as test_reader.plan_of).")
    L.append("machine: the laptop (8 cores, 23 GB). Peak memory: getrusage / VmHWM of the process that does the work.")
    ex = _j(work, "export.json")
    if ex:
        k = ex["kitchen.tc"]
        L += ["", "== the packed file ==",
              f"export_records.py: kitchen.tc {k['bytes'] / 1e6:.1f} MB: {k['actions']:,} actions (72,256 records, "
              f"{k['folded']} folded into an identical one, as build_task), {k['facts']} facts, {k['strings']:,} strings. "
              f"{ex['seconds']['kitchen.tc']:.1f} s to write, peak {ex['peak_rss_gb']:.2f} GB for the whole process "
              f"(loading the pickle alone: {ex['seconds']['load the reading']:.1f} s, about 1.05 GB; the export adds "
              f"almost nothing). kitchen-no211.tc: {ex['kitchen-no211.tc']['actions']:,} actions."]
        det = _j(work, "determinism.json")
        if det:
            L.append(f"The same bytes in every run: exported twice, with PYTHONHASHSEED {det['seeds'][0]} and "
                     f"{det['seeds'][1]}: kitchen.tc {'the same' if det['kitchen.tc'] else 'DIFFERENT'} (sha1 "
                     f"{det['sha1']['kitchen.tc'][:12]}), kitchen-no211.tc {'the same' if det['kitchen-no211.tc'] else 'DIFFERENT'} "
                     f"(sha1 {det['sha1']['kitchen-no211.tc'][:12]}). Before the fix two exports of the same reading "
                     f"differed in {det['before']}.")
    fm = _j(work, "format.json")
    if fm:
        L += ["", "== the format check: taskcore with no fold, no reduction and task.py's encoding (--encoding task) "
              "against task.py's write_sas =="]
        for r in fm:
            if r.get("missing"):
                L.append(f"  {r['what']}: not run")
                continue
            L.append(f"  {r['what']}: {'the same bytes' if r['same'] else 'DIFFERENT'} "
                     f"({r['bytes']:,} bytes, sha1 {r['sha1'][:12]}, {r['operators']:,} operators, "
                     f"{r['effects']:,} effects; against {r.get('against', 'task.py file')})"
                     + ("" if r["same"] else f" first difference {r.get('first difference (line, task.py, taskcore)')}"))
        if any(r.get("against") == "the recorded sha1 and size" for r in fm):
            L.append("  The recorded sha1 and size are pyref's, taken from task.py's own files on the same pickle; task.py, "
                     "reader.py and the pickle are unchanged since (sha1 above).")
        L.append("  No canonical sort was needed: taskcore writes task.py's order (variables by key, operators by action, "
                 "variant and effect order of _expand).")
    py = {v: _j(work, f"pyref-{v}.json") for v in ("pruned-all", "full-all", "pruned-no211")}
    tb = _j(work, "table.json")
    if tb:
        L += ["", "== reductions, kitchen.tc from start s17 ==",
              "Each row a separate taskcore run (load, fold, reductions to their fixpoint, both passes of the writer); "
              "time and peak memory of that run. Operators and effects are the SAS file's (after variant expansion).",
              "The Python pipeline's task (test_reader.plan_of: build_task, relevance.py, reader.prune, write_sas): "
              "71,324 actions, 583,670 operators, 9,324,673 effects, 163.8 MB." +
              (f" task.py took {sum(v for k, v in py['pruned-all']['seconds'].items() if k != 'load the reading'):.0f} s "
               f"and {py['pruned-all']['peak_rss_gb']:.1f} GB for it." if py["pruned-all"] else ""),
              f"  {'row':62} {'actions':>8} {'operators':>10} {'effects':>11} {'MB':>7} {'s':>6} {'GB':>5}"]
        for r in tb:
            L.append(f"  {r['row'][:62]:62} {r['actions']:8,} {r['sas']['operators']:10,} {r['sas']['effects']:11,} "
                     f"{r['sas']['bytes'] / 1e6:7.1f} {r['seconds_total']:6.1f} {r['peak_rss'] / 1e9:5.2f}")
        dflt = next((r for r in tb if "the default" in r["row"]), None)
        if dflt:
            L += ["", "the default run's counts (fold strict, then (a)-(f) in a loop until none changes the task):"]
            fc = dflt.get("fold_counts", {})
            L.append("  fold: " + ", ".join(f"{k.replace('_', ' ')} {v:,}" for k, v in fc.items()))
            L += [f"  {v:9,}  {k}" for k, v in dflt.get("reduce_counts", {}).items()]
            L.append(f"  pick variables {dflt['sas']['pick_variables']}, pick operators {dflt['sas']['pick_operators']}, "
                     f"actions through picks {dflt['sas']['pick_actions']}, variables {dflt['sas']['variables']}, "
                     f"conditional effects {dflt['sas']['conditional_effects']:,}")
    fck = _j(work, "foldcheck.json")
    if fck:
        L += ["", "== the fold check (taskcore --check-fold 4, strict and every end) =="]
        for name, c in fck.items():
            L.append(f"  {name}: {c['folds']:,} folded actions, {c['paths']:,} paths, {c['states']:,} states in the paths' "
                     f"boxes (corners and random) and {c['union_box_states']:,} in the folded precondition's box: "
                     f"mismatches {c['mismatches']} (folded action does not apply {c['folded_does_not_apply']}, another "
                     f"path applies {c['another_path_applies']}, writes differ {c['writes_differ']}, conflicting writes "
                     f"{c['conflicting_writes']}, not exactly one path in the union box {c['union_box_not_exactly_one_path']})"
                     f", {c['seconds']:.0f} s")
    fdb = _j(work, "fd-base.json")
    tags = ["default"] + sorted(p.name[6:-5] for p in work.glob("plans-*.json") if p.name != "plans-default.json")
    for tag in tags:
        pl = _j(work, f"plans-{tag}.json")
        if not pl:
            continue
        L += ["", f"== plans: lama-first (plan_fd.run, its search string with the landmark dump, 600 s), taskcore "
              f"{'default (fold strict, (a)-(f))' if tag == 'default' else tag} =="]
        if fdb:
            L.append(f"  baseline, the same search on task.py's file (pruned task, s17) on this machine: {fdb['length']} steps, "
                     f"Fast Downward {fdb['total_s']:.1f} s (search {fdb['search_s']:.2f} s), peak {fdb['peak_kb'] / 1e6:.2f} GB, "
                     f"{fdb['landmarks']} landmarks. reader-kitchen.log: 7 steps in 171.2 s (all), 15 in 210.7 s "
                     f"(no 211), 8 in 97.8 s (replan).")
        for r in pl:
            fd = r["fd"]
            L.append(f"  {r['variant']}: taskcore {r['taskcore']['actions']:,} actions, {r['sas']['operators']:,} operators, "
                     f"{r['sas']['bytes'] / 1e6:.1f} MB in {r['taskcore']['seconds_total']:.1f} s, "
                     f"{r['taskcore']['peak_rss'] / 1e9:.2f} GB; Fast Downward exit {fd['exit']}, "
                     f"{fd.get('length', '-')} steps, {fd.get('total_s', 0):.1f} s (search {fd.get('search_s', 0):.2f} s), "
                     f"peak {fd.get('peak_kb', 0) / 1e6:.2f} GB, {fd.get('landmarks', '-')} landmarks")
            if "steps" in r:
                for i, s in enumerate(r["steps"], 1):
                    L.append(f"    {i:2}. {s['path']}" + (f"   (in {s['taskcore']})" if s["taskcore"] != s["path"] else ""))
                rs = r["replay over the full task"]

                def keys(xs):
                    ks = sorted({x[1] for x in xs})
                    return f"{len(xs)} ({', '.join(ks[:6])}{', ...' if len(ks) > 6 else ''})" if xs else "none"
                L.append(f"    replay over the full task, every guessed write that fires left unknown: goal reached "
                         f"{rs['ok']}, failing step {rs['step']}; guessed writes that fired {keys(rs['fired'])}, "
                         f"effects whose condition read one {keys(rs['undecided'])}; so no step relies on a guessed "
                         f"value" if rs["ok"] else f"    replay over the full task: FAILS at step {rs['step']}: {rs['why']}")
                tr = r.get("test_reader.replay (facts taskcore keeps)")
                if tr:
                    L.append(f"    test_reader.replay (the full task, effects on facts the file lacks left out): goal "
                             f"reached {tr['ok']}, first step that cannot run {tr['first step that cannot run']}, "
                             f"guessed values used {keys(tr['guessed values used'])}")
                    need = r.get("steps the goal needs", [])
                    L.append(f"    steps the goal needs (test_reader.needed_steps): {len(need)} of {len(r['steps'])}")
            kp = r.get("known plan's actions in the reduced task")
            if kp:
                L.append(f"    test_reader's plan for this variant: {sum(1 for x in kp if ': kept' in x)} of {len(kp)} "
                         f"actions kept ({sum(1 for x in kp if 'folded' in x)} as a path of a folded action), "
                         f"{sum(1 for x in kp if 'replaced' in x)} replaced, {sum(1 for x in kp if 'DROPPED' in x)} dropped")
    at = _j(work, "anytime.json")
    if at:
        L += ["", f"== plans: anytime LAMA (Fast Downward's seq-sat-lama-2011 alias, {at[0]['seconds']} s), taskcore default "
              "(model encoding: pick operators cost 0, so a plan's cost is its number of actions) =="]
        for r in at:
            found = ", ".join(f"{n} steps (cost {c}) at {t:.1f} s" for t, n, c in r["plans (time s, length, cost)"])
            L.append(f"  {r['variant']}: metric {r['metric']}; plans found: {found or 'none'}")
            if "last plan" in r:
                rs = r["replay over the full task"]
                L.append(f"    the last plan: {', '.join(r['last plan'])}")
                L.append(f"    replay over the full task, every guessed write that fires left unknown: goal reached "
                         f"{rs['ok']}" + ("" if rs["ok"] else f", fails at step {rs['step']}: {rs['why']}"))
    kn = _j(work, "known-plans.json")
    if kn:
        L += ["", "== test_reader's plans (7, 15 and 8 steps, found on task.py's pruned task) run on the reduced tasks "
              "(taskcore validate: each step the reduced action that holds its record's action; its precondition must hold; "
              "a guessed write that fires makes its fact unknown, and reading an unknown fails; the goal must hold) =="]
        for r in kn:
            cfg = " ".join(r["args"]) or "default"
            folded = sum(1 for x in r["steps"] if ": in fold:" in x)
            g = r["guesses_fired"]
            gk = sorted({x[1] for x in g})
            L.append(f"  {r['variant']}, {cfg}: {r['length']} steps, runs {r['ok']}, goal {r['goal']}, {folded} steps "
                     f"through a folded action, guessed writes that fired (left unknown) "
                     + (f"{len(g)}, on {len(gk)} facts ({', '.join(gk[:5])}, ...), none read later" if g else "none"))
    L += ["", "== Rust against Python, the same input (the pickled reading) =="]
    for v, what in (("pruned-all", "test_reader's pruned task"), ("full-all", "build_task's task"),
                    ("pruned-no211", "the pruned task without local 211's event")):
        if py[v]:
            sec = py[v]["seconds"]
            L.append(f"  task.py, {what}: " + ", ".join(f"{k} {x:.1f} s" for k, x in sec.items()) +
                     f"; peak {py[v]['peak_rss_gb']:.2f} GB; {py[v]['counts']['operators']:,} operators, "
                     f"{py[v]['counts']['effects']:,} effects")
    if ex:
        L.append(f"  export_records.py (the pickle to kitchen.tc, build_task's rules, streaming): "
                 f"{ex['seconds']['kitchen.tc']:.1f} s; peak {ex['peak_rss_gb']:.2f} GB (the loaded pickle)")
    if fm:
        for r in fm:
            if not r.get("missing"):
                L.append(f"  taskcore, {r['what']}, no fold, no reduction: {r['seconds']:.1f} s, peak "
                         f"{r['peak_rss'] / 1e9:.3f} GB (load and both passes)")
    if tb and dflt:
        L.append(f"  taskcore, the default (fold and reductions, then the file): {dflt['seconds_total']:.1f} s, peak "
                 f"{dflt['peak_rss'] / 1e9:.3f} GB")
    if fdb and _j(work, "plans-default.json"):
        a = _j(work, "plans-default.json")[0]["fd"]
        L.append(f"  Fast Downward lama-first, all, s17: on task.py's file {fdb['total_s']:.1f} s and "
                 f"{fdb['peak_kb'] / 1e6:.2f} GB; on the default reduced file {a['total_s']:.1f} s and "
                 f"{a['peak_kb'] / 1e6:.2f} GB")
    L.append("  The task.py rows are pyref's runs (task.py and reader.py unchanged since); the rest is this run. Times are "
             "wall seconds on the laptop while other jobs ran, so they vary by up to 2x between runs; memory does not.")
    for prop in sorted(work.glob("prop-*.out")):
        n, seed = prop.stem.split("-")[1], prop.stem.split("-s")[-1]
        L += ["", f"== the property test: python test_taskcore.py {n} {seed} ==",
              "  " + prop.read_text().strip().replace("\n", "\n  ")]
    extra = work / "notes.txt"
    if extra.exists():
        L += ["", extra.read_text().rstrip()]
    LOG.write_text("\n".join(L) + "\n")
    print("\n".join(L))


def main() -> None:
    step, work = sys.argv[1], Path(sys.argv[2])
    if step == "pyref":
        pyref(work, sys.argv[3], sys.argv[4])
    elif step == "format":
        format_check(work)
    elif step == "table":
        table(work)
    elif step == "plans":
        # each further argument is one configuration, its taskcore arguments separated by spaces ("" the default)
        plans(work, [a.split() for a in (sys.argv[3:] or [""])])
    elif step == "known":
        known(work)
    elif step == "anytime":
        anytime(work, int(sys.argv[3]) if len(sys.argv) > 3 else 120)
    elif step == "foldcheck":
        fold_check(work)
    elif step == "fdbase":
        fd_base(work)
    elif step == "log":
        write_log(work)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
