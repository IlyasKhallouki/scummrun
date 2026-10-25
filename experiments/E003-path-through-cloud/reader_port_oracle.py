"""E003, D7: the oracles for scumm/reader, the Rust port of domains.py and of the half of reader.py that does not
depend on the Machine. An instrument: it imports domains.py, reader.py, golden.py and size_scaling.py as they are and
changes nothing; the capture wraps methods in its own process only.

usage (from this folder, with ../../.venv/bin/python, under nice 10):
  reader_port_oracle.py static  [--out DIR]                                   (about a minute)
  reader_port_oracle.py capture room33|kitchen [--out DIR] [--workers 4] [--pickle FILE]
                                                                (room 33: seconds; the kitchen: about 15 minutes)
  reader_port_oracle.py sas --pickle FILE --work DIR [--out DIR]   (task.py's write_sas on the kitchen: ~30 min)
  reader_port_oracle.py bench [--repeat 3]                        (time of domains.build and Scope.of)

Output goes to ``out/reader-port/`` (git-ignored) unless ``--out`` says otherwise. Every line is one JSON object in
golden.py's canonical encoding (``golden.enc``: tuples, sets, maps and dataclasses typed).

**static** writes ``static.jsonl.gz``:

* ``{"kind": "domains", "name", "start", "start_items", "goals", "domains"}``: ``domains.build(start, goals)`` for room 33's start
  (results/start-part1.jsonl seed 1), the kitchen's (groundtruth pre of S1#7), Part I's with the goal of
  results/end-part1.json (as size_scaling.py builds it) and the engine's init ({}).
* ``{"kind": "scope", "name", "rooms", "start", "start_items", "scope"}``: ``reader.Scope.of`` for room 33, the kitchen,
  size_scaling.py's scopes k = 0..3 from seed 1 and every room with code or objects (``size_scaling.all_rooms``).
* ``{"kind": "hook_writes", ...}``: ``Reader._hook_writes(n)`` for every global script n;
  ``{"kind": "entry_writes", ...}``: ``reader.entry_writes(game, v)`` for every verb an event table names.
* ``{"kind": "builder", "name", ...}`` for room 33 and the kitchen: a Builder as ``read_all``'s first pass starts it:
  the FactTable's own domains, counts and points; the domain and kind of every fact key the golden records name;
  ``inputs()`` and the counts it makes; ``start_events()``; ``ego_values()``; ``unknown_targets`` of every write the
  golden records list.
* ``{"kind": "kitchen_task", "goal", "starts", "no211"}``: test_reader.py's goal, its two starts (s17: the
  demo's, replan: ``replan_observed``) and the label of the event its "no 211" variant leaves out.
* ``{"kind": "check_machine", ...}``: reader.py's lists of the Machine names and fields it relies on.

**capture NAME** reads the scope as golden.py's ``reader-room33`` / ``reader-kitchen`` does (same start, scope,
domains and workers) and keeps every machine run the Builder asks for (``Reader.run_sentence_sym``,
``run_click_verb``, ``run_event``): its inputs (pins, precondition, what) and what ``Builder.run`` and
``Builder.record`` read from it (the run's limit, its value caps, the first fork, the Reader's counter increments
during the run, and per path its end and, for a path that can be a record, its pre, writes, kept slots, unknown
writes, assumptions and last five notes). The runs go to ``capture-NAME.jsonl.gz`` in the order the parent process
handles the jobs (pass, batch, job, run within the job), whatever the number of workers. Each line:
{"pass", "batch", "job", "seq", "label", "pins", "pre", "what", "limit", "value_caps", "first_fork", "rd", "paths"}.
A job is numbered by its place in its batch (the pins carry it into a worker), so two jobs of one batch with one
label (``event(exit@W)`` of two rooms) keep apart. It also writes the reading in golden.py's format (``reading-NAME.jsonl.gz``; equal to out/golden/reader/NAME.jsonl.gz
when the reading is the golden one) and, with ``--pickle``, (builder, records, seconds) as test_reader.py's
READER_PICKLE holds them. A thread stops the run when free memory falls under 3 GB.

**sas** (``--pickle FILE --work DIR``): task.py's ``write_sas`` on the kitchen reading, through taskcore_kitchen.py's
``pyref`` (build_task, and test_reader's relevance + prune for "pruned"; all records, and without local 211's event):
each file's sha1 and size, the times and the peak memory, in ``python-sas.json``. The big files are deleted.

Every command runs under PYTHONHASHSEED=0 (``main`` starts the script again with it when it is not set): build_task
keys its fold on ``str()`` of effects, and the order of a set with 'above' or 'below' in it follows the string hash,
so the same kitchen reading folds 491 to 527 records with a random seed (527 with seed 0, and with every set sorted).
"""

import argparse
import collections
import gzip
import hashlib
import json
import os
import pickle
import shutil
import signal
import sys
import threading
import time
import types
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / "src"))

import golden  # noqa: E402
from golden import dumps, enc, line  # noqa: E402

OUT = ROOT / "out" / "reader-port"
MIN_FREE_GB = 3.0
HASHSEED = "0"


def seed1() -> dict:
    return next(json.loads(ln) for ln in (HERE / "results" / "start-part1.jsonl").read_text().splitlines()
                if json.loads(ln)["seed"] == golden.ROOM33_SEED)["facts"]


def kitchen_pre() -> dict:
    import groundtruth
    return next(r for r in groundtruth.load() if r["name"].startswith(golden.KITCHEN_MOVE + " "))["pre"]


def part1_goal() -> dict:
    import size_scaling
    return size_scaling.goal()


def items(start: dict) -> list:
    """A start map's items in its own order (reader.Scope.of walks the keys in that order)."""
    return [[k, v] for k, v in start.items()]


def _sorted_keys(d) -> list:
    return sorted(d, key=golden._text_key)


# --- static ----------------------------------------------------------------------------------------------------------

def scope_dict(sc) -> dict:
    return {"rooms": list(sc.rooms), "units": _sorted_keys(sc.units), "objects": sc.objects, "verbs": sc.verbs,
            "dialog_verbs": sc.dialog_verbs, "walk_verb": sc.walk_verb, "universe": sc.universe, "counts": sc.counts,
            "writes": {"exact": sc.writes.exact, "wild": sc.writes.wild, "var_windows": sc.writes.var_windows,
                       "classes": sc.writes.classes, "counts": sc.writes.counts}}


def golden_keys(path: Path) -> tuple:
    """The fact keys and the unknown writes the golden records name."""
    keys, unknown = set(), set()
    for ln in golden.open_lines(path):
        d = json.loads(ln)
        if d["kind"] != "record":
            continue
        r = d["record"]["$Record"]
        keys |= set(r["pre"]) | set(r["effects"])
        for e in r["effects"].values():
            t = e["$tuple"]
            if t[0] in ("when", "copy"):
                keys.add(t[1])
        for u in r["unknown"]:
            unknown.add(tuple(u["$tuple"][:2]))
    return keys, unknown


def builder_dict(game, name: str, rooms, pre: dict) -> dict:
    import domains as D
    import reader as R
    doms = D.build(pre)
    sc = R.Scope.of(game, rooms, pre)
    b = R.Builder(R.Reader(sc, doms, game))
    t = b.table
    keys, unknown = golden_keys(ROOT / "out" / "golden" / "reader" / f"{name}.jsonl.gz")
    keys |= set(t.start) | set(t.extra) | {k for k in doms.doms}
    keys |= {f"var:{n}" for n in range(0, 800)} | set(sc.universe) | set(sc.writes.exact)
    facts = {k: {"kind": t.kind(k), "domain": t.domain(k)} for k in sorted(keys)}
    out = {"kind": "builder", "name": name, "rooms": list(rooms), "start": pre, "start_items": items(pre),
           "table": {"extra": t.extra, "counts": t.counts, "points": {k: dict(v) for k, v in t.points.items()},
                     "start": t.start},
           "facts": facts, "sentence_scripts": b.sentence_scripts(), "ego_values": b.ego_values()}
    out["inputs"] = list(b.inputs())
    out["input_counts"] = dict(b.counts)
    b.counts.clear()
    out["start_events"] = b.start_events()
    out["start_event_counts"] = dict(b.counts)
    out["unknown_targets"] = [[list(w), b.unknown_targets(w + ("",))] for w in sorted(unknown)]
    return out


def cmd_static(a) -> None:
    import domains as D
    import model as M
    import reader as R
    import size_scaling
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    w = golden.GzWriter(out / "static.jsonl.gz")
    s1, kp = seed1(), kitchen_pre()
    t0 = time.time()
    for name, start, goals in (("room33", s1, None), ("kitchen", kp, None), ("part1", s1, part1_goal()),
                               ("init", {}, None)):
        doms = D.build(start, goals=goals)
        w.write(line({"kind": "domains", "name": name, "start": enc(start), "start_items": enc(items(start)),
                      "goals": enc(goals), "domains": enc(doms)}))
        print(f"domains {name}: {len(doms.doms)} domains, {time.time() - t0:.1f} s", file=sys.stderr, flush=True)
    game = M.Game.load()
    scopes = {"room33": (golden.ROOM33, s1), "kitchen": (golden.KITCHEN_ROOMS, kp)}
    for k, rooms in size_scaling.scopes(game, s1["room"]).items():
        if k <= 3:
            scopes[f"k{k}"] = (tuple(rooms), s1)
    scopes["all"] = (tuple(size_scaling.all_rooms(game)), s1)
    for name, (rooms, start) in scopes.items():
        sc = R.Scope.of(game, rooms, start)
        w.write(line({"kind": "scope", "name": name, "rooms": list(rooms), "start": enc(start),
                      "start_items": enc(items(start)), "scope": enc(scope_dict(sc))}))
        print(f"scope {name}: {len(sc.units)} units", file=sys.stderr, flush=True)
    ns = types.SimpleNamespace(game=game)
    hooks = {n: R.Reader._hook_writes(ns, n) for (kind, n, *_) in sorted(game.units) if kind == "global"}
    w.write(line({"kind": "hook_writes", "writes": enc(hooks)}))
    verbs = sorted({v for k, u in game.units.items() if k[0] == "obj" for v in u.entries})
    w.write(line({"kind": "entry_writes", "writes": enc({v: R.entry_writes(game, v) for v in verbs})}))
    for name, (rooms, start) in (("room33", scopes["room33"]), ("kitchen", scopes["kitchen"])):
        w.write(line(enc(builder_dict(game, name, rooms, start))))
        print(f"builder {name}", file=sys.stderr, flush=True)
    import test_reader as TR
    w.write(line({"kind": "kitchen_task", "goal": [[k, sorted(v)] for k, v in TR.goal().items()],
                  "starts": {"s17": items(TR.start()), "replan": items(TR.replan_observed())},
                  "no211": "event(local:211@33)"}))
    w.write(line({"kind": "check_machine", "lazy": list(R.LAZY_METHODS), "step_ops": enc(R.STEP_OPS),
                  "called": list(R.CALLED), "path_fields": list(R.PATH_FIELDS), "frame_fields": list(R.FRAME_FIELDS),
                  "drop_ends": list(R.DROP_ENDS), "sources": {n: golden.sha1(HERE / n) for n in
                                                              ("reader.py", "domains.py", "model.py", "task.py")}}))
    info = w.close()
    print(json.dumps(info), file=sys.stderr)


# --- capture ---------------------------------------------------------------------------------------------------------

CAP = {"pass": 0, "batch": 0, "label": None, "job": None, "seq": 0, "fh": None, "dir": None}


class JobPins(dict):
    """A job's pins, with ``job``: the job's place in its batch (pickled along into a forked worker)."""


def _path_dict(p, R) -> dict:
    d = {"end": p.end, "sym": isinstance(p, R.SymPath)}
    if p.end in R.DROP_ENDS or not d["sym"]:
        return d
    d.update(pre=enc(p.pre), writes=enc(p.writes), kept=enc(p.kept), unknown_writes=enc(p.unknown_writes),
             assumptions=enc(p.assumptions), notes=enc(list(p.notes[-5:])))
    return d


def install_capture(R) -> None:
    orig_read_once, orig_run_jobs, orig_run = R.Builder._read_once, R.Builder._run_jobs, R.Builder.run

    def read_once(self, *a, **k):
        CAP["pass"] += 1
        return orig_read_once(self, *a, **k)

    def run_jobs(self, jobs, workers):
        CAP["batch"] += 1
        tagged = []
        for i, job in enumerate(jobs):
            pins = JobPins(job[2])
            pins.job = i
            tagged.append(tuple(job[:2]) + (pins,) + tuple(job[3:]))
        return orig_run_jobs(self, tagged, workers)

    def run(self, kind, label, pins, pre, what, checks=(), budget=None):
        if budget is not None:
            return orig_run(self, kind, label, pins, pre, what, checks, budget)
        CAP["label"], CAP["seq"] = label, 0
        CAP["job"] = pins.job
        pins = dict(pins)
        path = CAP["dir"] / f"{CAP['pass']:02d}-{CAP['batch']:06d}-{CAP['job']:07d}.gz"
        CAP["fh"] = gzip.open(path, "wt", encoding="ascii", compresslevel=6)
        try:
            return orig_run(self, kind, label, pins, pre, what, checks, budget)
        finally:
            CAP["fh"].close()
            CAP["fh"] = None

    R.Builder._read_once, R.Builder._run_jobs, R.Builder.run = read_once, run_jobs, run

    def wrap(name):
        orig = getattr(R.Reader, name)

        def method(self, pins, pre, *args):
            head = {"pass": CAP["pass"], "batch": CAP["batch"], "job": CAP["job"], "seq": CAP["seq"],
                    "label": CAP["label"], "pins": enc(pins), "pre": enc(pre),
                    "what": enc((name,) + tuple(args))}
            before = {n: collections.Counter(getattr(self, n)) for n in ("counts", "forks", "fork_keys")}
            run = orig(self, pins, pre, *args)
            # every key the run changed or added, also one added with 0 (a Counter keeps it: ``c[k] += 0``)
            head["rd"] = {n: {k: v - before[n].get(k, 0) for k, v in getattr(self, n).items()
                              if k not in before[n] or v != before[n][k]} for n in before}
            head.update(limit=run.limit, value_caps=sum(run.value_caps.values()), first_fork=self.first_fork,
                        paths=[_path_dict(p, R) for p in run.paths])
            CAP["fh"].write(golden._text(head) + "\n")
            CAP["seq"] += 1
            return run
        method.__name__ = name
        setattr(R.Reader, name, method)
    for name in ("run_sentence_sym", "run_click_verb", "run_event"):
        wrap(name)


def memory_guard(stop: threading.Event) -> None:
    while not stop.wait(2.0):
        mem, _ = golden.free_gb()
        if mem < MIN_FREE_GB:
            print(f"free memory {mem:.1f} GB under {MIN_FREE_GB} GB: stopping", file=sys.stderr, flush=True)
            os.killpg(os.getpgrp(), signal.SIGKILL)


def cmd_capture(a) -> None:
    import domains as D
    import model as M
    import reader as R
    golden.frozen_check()
    golden.guard("capture")
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    parts = out / f"capture-{a.name}.parts"
    shutil.rmtree(parts, ignore_errors=True)
    parts.mkdir()
    CAP["dir"] = parts
    install_capture(R)
    stop = threading.Event()
    threading.Thread(target=memory_guard, args=(stop,), daemon=True).start()
    if a.name == "room33":
        rooms, pre, start = golden.ROOM33, seed1(), f"results/start-part1.jsonl seed {golden.ROOM33_SEED}"
    else:
        rooms, pre, start = golden.KITCHEN_ROOMS, kitchen_pre(), f"groundtruth pre of {golden.KITCHEN_MOVE}"
    t0 = time.time()
    game = M.Game.load()
    doms = D.build(pre)
    t1 = time.time()
    sc = R.Scope.of(game, rooms, pre)
    b = R.Builder(R.Reader(sc, doms, game))
    recs = b.read_all(workers=a.workers)
    t2 = time.time()
    stop.set()
    info = {"name": a.name, "start": start, "domains_goals": None, "workers": a.workers,
            "sources": {n: golden.sha1(HERE / n) for n in ("reader.py", "model.py", "domains.py", "ir.py", "task.py")}}
    fi = golden.write_reading(out / f"reading-{a.name}.jsonl.gz", b, recs, info)
    with open(out / f"capture-{a.name}.jsonl.gz", "wb") as f:  # gzip members, concatenated in job order
        for p in sorted(parts.iterdir()):
            f.write(p.read_bytes())
    shutil.rmtree(parts)
    gold = json.loads((ROOT / "out" / "golden" / "manifest.json").read_text())["parts"][f"reader/{a.name}"]["file"]
    res = {"name": a.name, "pythonhashseed": os.environ.get("PYTHONHASHSEED"), "records": len(recs), "read_s": round(t2 - t1, 1), "setup_s": round(t1 - t0, 1),
           "reading": fi, "equal_to_golden": fi["sha256"] == gold["sha256"],
           "capture_bytes": (out / f"capture-{a.name}.jsonl.gz").stat().st_size}
    if a.pickle:
        Path(a.pickle).write_bytes(pickle.dumps((b, recs, {"domains": t1 - t0, "read": t2 - t1})))
    golden.frozen_check()
    (out / f"capture-{a.name}.json").write_text(json.dumps(res, indent=1) + "\n")
    print(json.dumps(res), file=sys.stderr)


# --- sas -------------------------------------------------------------------------------------------------------------

VARIANTS = (("full", "all"), ("full", "no211"), ("pruned", "all"), ("pruned", "no211"))


def cmd_sas(a) -> None:
    """task.py's write_sas on the kitchen reading (READER_PICKLE ``--pickle``, as ``capture kitchen --pickle`` writes
    it): taskcore_kitchen.py's ``pyref`` for build_task's task (full) and test_reader's pruned one, all records and
    without local 211's event, each in its own process under nice 10, stopped when free memory falls under 3 GB.
    Keeps each file's sha1 and size, the times and the peak memory (``python-sas.json``); the files are deleted. A
    variant whose pyref JSON is already in ``--work`` is not run again."""
    out, work = Path(a.out), Path(a.work)
    work.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, READER_PICKLE=str(Path(a.pickle).resolve()), PYTHONHASHSEED=HASHSEED)
    res = {"pythonhashseed": HASHSEED}
    for variant, which in VARIANTS:
        js = work / f"pyref-{variant}-{which}.json"
        if not js.exists():
            golden.guard(f"sas {variant} {which}")
            p = __import__("subprocess").Popen(["nice", "-n", "10", sys.executable, "taskcore_kitchen.py", "pyref",
                                                str(work), variant, which], cwd=HERE, env=env,
                                               start_new_session=True)
            while p.poll() is None:
                time.sleep(1)
                mem, disk = golden.free_gb()
                if mem < MIN_FREE_GB or disk < 1.5:
                    os.killpg(p.pid, signal.SIGKILL)
                    raise SystemExit(f"sas {variant} {which}: free memory {mem:.1f} GB, disk {disk:.1f} GB: stopped")
            if p.returncode != 0:
                raise SystemExit(f"pyref {variant} {which} failed ({p.returncode})")
        d = json.loads(js.read_text())
        for f in d["files"].values():
            Path(f["path"]).unlink(missing_ok=True)
        res[f"{variant}-{which}"] = {"files": {n: {"sha1": f["sha1"], "bytes": f["bytes"]} for n, f in d["files"].items()},
                                     "seconds": d["seconds"], "rss_gb": d["rss_gb"], "peak_rss_gb": d["peak_rss_gb"],
                                     "counts": d["counts"],
                                     "actions": d.get("pruned actions", d.get("build_task actions"))}
        print(f"{variant} {which}: {json.dumps(res[f'{variant}-{which}']['files'])}", file=sys.stderr, flush=True)
    (out / "python-sas.json").write_text(json.dumps(res, indent=1, sort_keys=True) + "\n")


# --- bench -----------------------------------------------------------------------------------------------------------

def cmd_bench(a) -> None:
    """Python's time and memory for domains.build (the four starts of ``static``) and Scope.of (its seven scopes), each
    run ``--repeat`` times (the best kept), the game loaded once; peak RSS from getrusage."""
    import resource
    import domains as D
    import model as M
    import reader as R
    import size_scaling
    out = {"domains.build": {}, "Scope.of": {}}
    s1, kp = seed1(), kitchen_pre()
    t0 = time.perf_counter()
    for name, start, goals in (("room33", s1, None), ("kitchen", kp, None), ("part1", s1, part1_goal()),
                               ("init", {}, None)):
        best = min(_timed(lambda: D.build(start, goals=goals)) for _ in range(a.repeat))
        out["domains.build"][name] = round(best, 3)
    t1 = time.perf_counter()
    game = M.Game.load()
    out["Game.load"] = round(time.perf_counter() - t1, 3)
    scopes = {"room33": (golden.ROOM33, s1), "kitchen": (golden.KITCHEN_ROOMS, kp)}
    for k, rooms in size_scaling.scopes(game, s1["room"]).items():
        if k <= 3:
            scopes[f"k{k}"] = (tuple(rooms), s1)
    scopes["all"] = (tuple(size_scaling.all_rooms(game)), s1)
    for name, (rooms, start) in scopes.items():
        best = min(_timed(lambda: R.Scope.of(game, rooms, start)) for _ in range(a.repeat))
        out["Scope.of"][name] = round(best, 3)
    out["peak_rss_mb"] = round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024, 1)
    out["total_s"] = round(time.perf_counter() - t0, 1)
    print(json.dumps(out, indent=1))


def _timed(fn) -> float:
    t = time.perf_counter()
    fn()
    return time.perf_counter() - t


def main() -> None:
    if os.environ.get("PYTHONHASHSEED") != HASHSEED:  # the fold and set orders must not follow a random seed
        os.execve(sys.executable, [sys.executable, *sys.argv], dict(os.environ, PYTHONHASHSEED=HASHSEED))
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("static")
    s.add_argument("--out", default=str(OUT))
    s.set_defaults(fn=cmd_static)
    s = sub.add_parser("capture")
    s.add_argument("name", choices=("room33", "kitchen"))
    s.add_argument("--out", default=str(OUT))
    s.add_argument("--workers", type=int, default=4)
    s.add_argument("--pickle", default="")
    s.set_defaults(fn=cmd_capture)
    s = sub.add_parser("bench")
    s.add_argument("--repeat", type=int, default=3)
    s.set_defaults(fn=cmd_bench)
    s = sub.add_parser("sas")
    s.add_argument("--pickle", required=True)
    s.add_argument("--work", required=True, help="taskcore_kitchen.py's work directory (big files, deleted)")
    s.add_argument("--out", default=str(OUT))
    s.set_defaults(fn=cmd_sas)
    a = ap.parse_args()
    if getattr(a, "workers", 1) > 4:
        raise SystemExit("at most 4 worker processes")
    a.fn(a)


if __name__ == "__main__":
    main()
