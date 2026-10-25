"""E003 step 5 on Part I: per seed, boot, take the start from the engine, plan to the reading's goal, play the plans with
the replay harness under plan.md's limits, verify from boot, and report.

usage (from this folder, with ../../.venv/bin/python, under nice):
       replay_part1.py export ROOMS --seed N --goal FACT=V[,FACT=V] --name NAME --out DIR
                       (a reading from the Python reader: ROOMS comma-separated, at most two rooms; the start is
                       results/start-part1.jsonl's seed N; ``--goal part1`` is results/end-part1.json)
       replay_part1.py export-pickle PICKLE --goal ... --name NAME --start-name TEXT --out DIR   (a pickled reading)
       replay_part1.py control DIR/reading.json           (the control's reading next to it: DIR/control.json)
       replay_part1.py start SEED [SEED ...] --reading DIR/reading.json [--results DIR]
                       (the start check only, no goal run)
       replay_part1.py run SEED --reading DIR/reading.json [--control] [--replan-limit 500] [--time-limit 86400]
                       [--plan-time 600] [--plan-memory 6G] [--results DIR] [--deadline EPOCH]
       replay_part1.py log NAME [--results DIR]           (the log of NAME's runs and start checks, from the json)
       score_part1.py RESULT.json ...                     (after the run: the key's Part I goals, from boot)

**The input** is a reading as files (packed_reading.py): ``reading.json``, the packed records file in taskcore's
format and its action lines. The Rust reader (branch reader-b) writes the packed file and its records in golden.py's
format; ``packed_reading.py from-golden`` makes the other two from those, with no reading (tested on the kitchen: the
Rust reader's packed file gives build_task's task, action by action). ``export`` and ``export-pickle`` make the files
from the Python reader, for readings it can make on this laptop (one room and a neighbour at most; ``export`` refuses
more than two rooms: the Python reader needs 16 GB at k=1). The goal is the packed file's: for Part I, the Part I end
(results/end-part1.json, plan.md step 2, a disclosed human input); a test may give a nearer goal.

**Per seed** (plan.md step 5):

1. **Boot** with plan.md's settings (start.py ``config``: the DOS CD release on the patched build, boot parameter 0,
   no debug flag, enhancements off, the bridge's automatic '.' and Esc off, no input) and run to the bridge's first
   idle stop (``Branch.run(idle=True)``), the start (plan.md step 2).
2. **The start check**: start.py's checks (an idle stop of kind sentence; the dump's seed and boot parameter; VAR_DEBUGMODE
   0; only the trace records start.py allows, so no input reached the engine); the facts against the reading's
   start (the map its domains were built from) and against results/start-part1.jsonl's row for the seed, when there
   is one; and taskcore's start check on the reading's packed file (``--start-check input`` and ``needed``: the
   facts whose start value a domain lacks, and of them those reduction (e) needs, on which the harness would stop).
3. **The run**: replay.Harness from that state, planning through taskcore (lama-first, ``--plan-time`` seconds a
   plan), with plan.md's stop rules: the goal, no plan, ``--replan-limit`` replans (500), ``--time-limit`` seconds
   (24 hours); the stop reason is recorded.
4. **Verify**: the harness's inputs from boot in a fresh engine with the same settings, no fork, no change to the
   random state, compared at every checkpoint (replay.verify), and the goal at the end.

**The reading and the seed.** The reader builds each fact's domain from its start (plan.md step 1: the start value
closed under the writes), so a reading serves the seed it was read from: ``run`` refuses a seed whose start differs
from the reading's on a fact the packed file holds (``start_check``: "task facts differ"), and ``start`` reports it.
On Part I that is every seed but the reading's own (a variable the boot script draws at random differs on every
seed, and Part I's scripts read it), so each seed needs its own reading, as plan.md step 6 runs steps 1, 2, 4 and 5
per seed.

**The control** (``--control``): the same run on the control's reading (export_records.control_records, made once
next to the reading: control.tc, control.actions.jsonl, control.json): each action's precondition cut to its engine
conditions and its steps' room, the scripts' tests dropped, and its action lines with no replay checks, so the
harness neither waits for the scripts' position tests nor reads them to tell a retry from a ban. Its bans follow the
same rule on its own records, which test nothing the scripts test: a ban names the room alone, and so removes that
action in that room for the rest of the run (the control knows no narrower point; plan.md's rule, on the control's
model).

**The report per seed** (plan.md step 5), in ``results/replay-part1-<name>-seed<N>[-control].json`` and the log:
reached the end or not (in the run and verified from boot), the stop reason, replans, mismatches per executed action,
the share of executed actions whose observed effects matched the model, actions in the final path, time per step,
and the landmarks of the first plan (Fast Downward's debug output, plan_fd.parse_landmarks). Definitions
(``summary``):

* **executed**: the steps whose input reached the engine (a try that sent an input, or an event's wait), the events
  the engine ran on through within the step before, and the events that had already happened at their turn. A step
  whose precondition never held, an Executor error before any input and a step of several paths with no path are
  not executed (listed under outcomes);
* **writes held**: the record's predicted writes all held (outcomes "matched", "matched, the engine ran on" (the
  next events rewrote what differed), "already happened"; "ran on in the step before" and "goal" when none of their
  writes was missed);
* **matched** (plan.md's match: "the predicted writes, and the changes to facts the model tracks"): the writes held
  and no tracked fact ended the step at another value with no predicted write (``History.unplanned``'s ``net``; a
  fact that changed and came back is listed, not counted);
* **mismatches per executed action**: the predicted writes not seen (kinds "write" and "guess": outside the model's
  values) plus the tracked facts changed with no predicted write, over the executed steps; a guessed value inside the
  model's values but not the plan's choice ("choice") is counted apart;
* **the final path**: the steps whose writes held, in order: the actions the engine carried out, from the start to
  the stop;
* **time per step**: per harness step its wall time and its frames, per plan its writing, search and read-back
  times, and the pieces of the run (boot, start check, harness, planning within it, verify).

**Part I goals the path passes through** (of 99) are scored after the run, by score_part1.py, from the recorded
inputs replayed from boot: the run never reads E001's key.

**A long run** keeps its record as it goes (``<run dir>/partial.json`` after every plan, outside the repo), and an
error inside the harness stops it with its reason and traceback in the record; verify still plays what was recorded.

C2: this file names no game number. The seeds are plan.md's; ``run`` refuses the held-out seeds 31-35 (only with
``--test-seeds``, for the Part I result itself) and the Any% seed always; ``start`` allows 31-35 (a start check
plays nothing).
"""

import argparse
import collections
import hashlib
import json
import pickle
import statistics
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import export_records  # noqa: E402
import packed_reading as PR  # noqa: E402
import plan_fd  # noqa: E402
import reader as R  # noqa: E402
import replay  # noqa: E402
import start as S  # noqa: E402
from facts import diff, facts_of  # noqa: E402
from speedrun import paths, releases  # noqa: E402
from speedrun.control import Controller  # noqa: E402
from speedrun.engine import EngineConfig  # noqa: E402

OUT = paths.OUT_DIR / "E003-part1"
RESULTS = HERE / "results"
START_ROWS = HERE / "results" / "start-part1.jsonl"
END_PART1 = HERE / "results" / "end-part1.json"
HELD_OUT = frozenset(range(31, 36))  # plan.md: the test seeds
ANY_SEED = 836557856  # plan.md: the Any% seed, never used by the system's runs
MAX_PY_ROOMS = 2  # the Python reader's scope on this laptop (one room plus one neighbour at most)
SOURCES = ("replay_part1.py", "replay.py", "packed_reading.py", "export_records.py", "plan_fd.py", "reader.py",
           "model.py", "task.py", "domains.py", "facts.py", "start.py")


def say(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr, flush=True)


def sha(path: Path) -> str:
    return hashlib.sha1(Path(path).read_bytes()).hexdigest()[:12]


def config(seed: int, out_dir: Path, inputs: Path | None = None) -> EngineConfig:
    """plan.md's engine settings for ``seed`` (start.py ``config``), with ``inputs`` as the raw input file (verify).
    The Any% seed is refused."""
    if seed == ANY_SEED:
        raise ValueError("the Any% seed is never used by the system's runs (plan.md)")
    rel = releases.get(S.RELEASE)
    return EngineConfig(out_dir=out_dir, game_path=releases.game_dir(rel), release=S.RELEASE, seed=seed,
                        boot_param=S.BOOT_PARAM, max_ticks=S.MAX_TICKS, timer_next="any", skip_text=False,
                        skip_cutscenes=False, raw_inputs=inputs)


def goal_of(text: str, domain) -> dict:
    """``--goal``: "part1" (results/end-part1.json: goal_all, and goal_not as every other value of the domain) or
    "FACT=V,FACT=V"."""
    if text == "part1":
        doc = json.loads(END_PART1.read_text())
        goal = {k: {v} for k, v in doc["goal_all"].items()}
        for k, v in doc["goal_not"].items():
            goal[k] = {x for x in domain(k).values if x != v}
        return goal
    out = {}
    for part in text.split(","):
        k, v = part.split("=")
        out[k.strip()] = {int(v)}
    return out


def start_row(seed: int) -> dict | None:
    if not START_ROWS.exists():
        return None
    return next((r for r in map(json.loads, START_ROWS.read_text().splitlines()) if r["seed"] == seed), None)


# --- the readings ----------------------------------------------------------------------------------------------------

def export_python(rooms: tuple, seed: int, goal_text: str, name: str, out: Path, workers: int = 1) -> dict:
    """A reading from the Python reader (module docstring): the scope of ``rooms`` from results/start-part1.jsonl's
    seed ``seed``, read by reader.py, written as files."""
    import domains as D
    import model
    if len(rooms) > MAX_PY_ROOMS:
        raise SystemExit(f"the Python reader reads at most {MAX_PY_ROOMS} rooms on this laptop (16 GB at k=1)")
    row = start_row(seed)
    if row is None:
        raise SystemExit(f"no start for seed {seed} in {START_ROWS}")
    pre = row["facts"]
    t0 = time.monotonic()
    game = model.Game.load()
    sc = R.Scope.of(game, rooms, pre)
    b = R.Builder(R.Reader(sc, D.build(pre), game))
    recs = b.read_all(workers=workers)
    secs = time.monotonic() - t0
    goal = goal_of(goal_text, b.table.domain)
    t1 = time.monotonic()
    info = PR.export_reading(out, b, recs, goal, f"results/start-part1.jsonl seed {seed}",
                             {"name": name, "rooms": list(rooms), "read_s": round(secs, 1),
                              "sources": {n: sha(HERE / n) for n in ("reader.py", "model.py", "domains.py", "ir.py",
                                                                     "task.py", "packed_reading.py",
                                                                     "export_records.py")}})
    info.update(records=len(recs), read_s=round(secs, 1), export_s=round(time.monotonic() - t1, 1))
    return info


def export_pickle(pickle_path: Path, goal_text: str, name: str, start_name: str, out: Path) -> dict:
    b, recs, secs = pickle.loads(Path(pickle_path).read_bytes())
    goal = goal_of(goal_text, b.table.domain)
    t1 = time.monotonic()
    info = PR.export_reading(out, b, recs, goal, start_name, {"name": name, "pickle": str(pickle_path),
                                                              "sources": {}})
    info.update(records=len(recs), export_s=round(time.monotonic() - t1, 1))
    return info


def make_control(reading: Path) -> Path:
    """The control's reading next to ``reading`` (module docstring): control.tc, control.actions.jsonl and
    control.json, made once."""
    reading = Path(reading)
    doc = json.loads(reading.read_text())
    if doc.get("control"):
        return reading
    out = reading.with_name("control.json")
    src, lines = reading.parent / doc["packed"], reading.parent / doc["actions"]
    if out.exists():
        old = json.loads(out.read_text())
        if old.get("source_sha1") == sha(src) and old.get("lines_sha1") == sha(lines) and \
                old.get("actions") == "control.actions.jsonl" and (reading.parent / old["actions"]).is_file():
            return out
    t = time.monotonic()
    counts = export_records.control_records(src, lines, reading.parent / "control.tc",
                                            reading.parent / "control.actions.jsonl")
    ctl = dict(doc, packed="control.tc", actions="control.actions.jsonl", control=True, source_sha1=sha(src),
               lines_sha1=sha(lines), control_counts=counts, control_s=round(time.monotonic() - t, 2))
    out.write_text(json.dumps(ctl, indent=1) + "\n")
    return out


# --- the start ---------------------------------------------------------------------------------------------------------

def taskcore_start_check(rd, start_facts: dict, work: Path) -> dict:
    """taskcore's start check on the reading's packed file (module docstring): per mode, "ok" or the facts outside
    their domains (the first ten) and how many, and the seconds."""
    work.mkdir(parents=True, exist_ok=True)
    st = replay.task_start(rd.b, start_facts, rd.task)
    sf = export_records.write_start(work / "start.txt", rd.task.facts, st)
    out = {}
    # the two modes at once, each in its own folder: at Part I's scale each is a whole taskcore pass over the file
    procs = {}
    for mode in ("input", "needed"):
        (work / mode).mkdir(exist_ok=True)
        cmd = [str(plan_fd.TASKCORE), "sas", "--task", str(rd.packed), "--start-file", str(sf), "--out",
               str(work / mode / "task.sas"), "--stats", str(work / mode / "stats.json"), "--start-check", mode]
        procs[mode] = (time.monotonic(), subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                                          text=True))
    for mode, (t, proc) in procs.items():
        stdout, stderr = proc.communicate()
        p = subprocess.CompletedProcess(proc.args, proc.returncode, stdout, stderr)
        row = {"seconds": round(time.monotonic() - t, 2)}
        if p.returncode == 0:
            row["status"] = "ok"
        else:
            import re
            m = re.search(r"(\d+) facts out of their domains: \[(.*)\]", p.stderr)
            row["status"] = "out of domain" if m else "error"
            row["error"] = p.stderr.strip()[-400:] if not m else None
            if m:
                keys = re.findall(r'"((?:[^"\\]|\\.)*)"', m[2])
                row["misses"] = int(m[1])
                row["first"] = [(k, start_facts.get(k, 0)) for k in keys[:10]]
        out[mode] = row
        (work / mode / "task.sas").unlink(missing_ok=True)
    return out


def boot(seed: int, out_dir: Path) -> tuple:
    """A fresh engine on ``seed`` run to its first idle stop: (Controller, root Branch, row). The row holds start.py's
    fields and checks (module docstring)."""
    t = time.monotonic()
    ctl = Controller(config(seed, out_dir), timeout_s=S.TIMEOUT_S)
    ctl.start()
    b = ctl.root
    st = b.run(idle=True, max_frame=S.MAX_FRAME)
    if "end" in st:
        ctl.close()
        raise RuntimeError(f"seed {seed}: the run ended before an idle stop: {st}")
    row = {"seed": seed, "stop": {k: st.get(k) for k in ("stop", "frame", "tick", "room", "idle", "rng", "up", "cs",
                                                          "ego")}}
    s = b.state()
    row["dump"] = {k: s.get(k) for k in ("frame", "tick", "room", "ego", "ego_pos", "seed", "boot_param")}
    row["facts"] = facts_of(s)
    row["trace"] = S.trace_records(out_dir / "trace.jsonl", st["frame"])
    f = row["facts"]
    row["checks"] = [why for ok, why in (
        (st.get("stop") == "idle", f"stop {st.get('stop')!r}, not idle"),
        (st.get("idle") == "sentence", f"idle kind {st.get('idle')!r}, not sentence"),
        (row["dump"]["seed"] == seed, f"dump seed {row['dump']['seed']}"),
        (row["dump"]["boot_param"] == S.BOOT_PARAM, f"dump boot_param {row['dump']['boot_param']}"),
        (f.get(f"var:{S.VAR_DEBUGMODE}", 0) == 0, f"VAR_DEBUGMODE is {f.get(f'var:{S.VAR_DEBUGMODE}')}"),
        (set(row["trace"]) <= S.TRACE_TYPES, f"trace records not allowed: {sorted(set(row['trace']) - S.TRACE_TYPES)}"),
    ) if not ok]
    row["boot_s"] = round(time.monotonic() - t, 1)
    return ctl, b, row


def start_check(row: dict, rd, work: Path) -> dict:
    """The start against the reading's start (every fact, and the facts the packed file holds: a reading serves the
    start its domains were built from, module docstring), results/start-part1.jsonl's row, and taskcore's check."""
    f = row["facts"]
    out = {"boot checks": row["checks"] or "ok"}
    d = diff(rd.doc["start"]["facts"], f)
    task_d = sorted((k, v) for k, v in d.items() if k in rd.task.facts)
    out["reading start"] = {"name": rd.doc["start"]["name"], "differ": len(d), "first": sorted(d.items())[:20],
                            "task facts differ": len(task_d), "task first": task_d[:20]}
    known = start_row(row["seed"])
    if known is not None:
        d2 = diff(known["facts"], f)
        out["start-part1"] = {"differ": len(d2), "frame": known["stop"]["frame"],
                              "same frame": known["stop"]["frame"] == row["stop"]["frame"]}
    t = time.monotonic()
    out["taskcore"] = taskcore_start_check(rd, f, work)
    out["taskcore_s"] = round(time.monotonic() - t, 2)
    return out


def cmd_start(seeds: list, reading: Path, results: Path = RESULTS, out_root: Path = OUT) -> list:
    rd = PR.load_reading(reading)
    name = rd.doc.get("name", reading.parent.name)
    rows = []
    results.mkdir(parents=True, exist_ok=True)
    for seed in seeds:
        t0 = time.monotonic()
        run_dir = out_root / f"{name}-start-seed{seed}"
        say(f"seed {seed}: booting to the first idle stop")
        ctl, b, row = boot(seed, run_dir / "run")
        ctl.close()
        say(f"seed {seed}: idle stop at frame {row['stop']['frame']}; checking the start")
        chk = start_check(row, rd, run_dir / "taskcore")
        doc = {"seed": seed, "reading": str(reading), "name": name, "stop": row["stop"], "dump": row["dump"],
               "trace": row["trace"], "boot_s": row["boot_s"], "check": chk, "seconds": round(time.monotonic() - t0, 1),
               "sources": {n: sha(HERE / n) for n in SOURCES}}
        (results / f"replay-part1-{name}-start-seed{seed}.json").write_text(json.dumps(doc, indent=1) + "\n")
        rows.append(doc)
        rs = chk["reading start"]
        say(f"seed {seed}: checks {chk['boot checks']}; {rs['differ']} facts differ from the reading's start, "
            f"{rs['task facts differ']} of them the task's; taskcore {chk['taskcore']['input']['status']} / "
            f"{chk['taskcore']['needed']['status']}")
    rd.close()
    return rows


# --- the report ----------------------------------------------------------------------------------------------------------

WRITES_HELD = ("matched", "matched, the engine ran on", "already happened")  # every predicted write held
EXECUTED_UNPLAYED = ("ran on in the step before", "already happened")  # events done without a try of their own
MODEL_KINDS = ("write", "guess")  # a predicted write not seen: outside the model's values (replay.match)


def executed(s: dict) -> bool:
    """The step's input reached the engine, or it is an event the harness waited for or found done (module
    docstring)."""
    if s["outcome"] in EXECUTED_UNPLAYED:
        return True
    return any(s["kind"] == "event" or t.get("inputs") for t in s["tries"])


def writes_held(s: dict) -> bool:
    if s["outcome"] in WRITES_HELD:
        return True
    if s["outcome"] in ("ran on in the step before", "goal"):
        return not any(m["kind"] in MODEL_KINDS for m in s["mismatches"])
    return False


def net(u: dict) -> bool:
    """An unplanned change (replay.History.unplanned) that ended the step at another value. A record from before the
    harness kept ``net``: from its changes when it holds all of them, else counted."""
    if "net" in u:
        return u["net"]
    if u["count"] <= len(u["changes"]):
        return u["changes"][-1][1] != u["start"]
    return True


def step_mismatches(s: dict) -> tuple:
    """(the predicted writes not seen, the tracked facts changed with no predicted write) of an executed step."""
    if s["outcome"] == "matched, the engine ran on":
        writes = []  # what differed was rewritten as the next events predict (their own misses are theirs)
    else:
        writes = [m for m in s["mismatches"] if m["kind"] in MODEL_KINDS]
    return writes, [u for u in s.get("unplanned", []) if net(u)]


def summary(res: dict) -> dict:
    """The report's numbers from the harness's record (module docstring)."""
    steps = res["steps"]
    outcomes = collections.Counter(s["outcome"] for s in steps)
    ex = [s for s in steps if executed(s)]
    n = len(ex)
    held = [s for s in ex if writes_held(s)]
    per = [(s, *step_mismatches(s)) for s in ex]
    matched = [s for s, w, u in per if writes_held(s) and not u]
    n_writes = sum(len(w) for _, w, _ in per)
    n_unplanned = sum(len(u) for _, _, u in per)
    unplanned_keys = collections.Counter(x["key"] for _, _, u in per for x in u)
    choice = [s for s in held if any(m["kind"] == "choice" or (m["kind"] == "guess" and len(m["predicted"]) == 1
                                                                and s["outcome"] in WRITES_HELD)
                                     for m in s["mismatches"])]
    not_matched = collections.Counter(("writes held, tracked facts changed" if writes_held(s) else s["outcome"])
                                      for s, w, u in per if not (writes_held(s) and not u))
    played = [s for s in steps if s["tries"]]
    secs = [s.get("seconds", 0.0) for s in played]
    frames = [s["end"] - s["start"] for s in played if s["end"] is not None]
    plans = res["plans"]
    plan_s = [p.get("write_s", 0) + p.get("seconds", 0) + p.get("decode_s", 0) for p in plans]

    def stats(xs):
        return {"n": len(xs), "mean": round(statistics.mean(xs), 2), "median": round(statistics.median(xs), 2),
                "max": round(max(xs), 2), "sum": round(sum(xs), 1)} if xs else {"n": 0}

    def share(k):
        return round(k / n, 3) if n else None
    return {
        "steps": len(steps), "executed": n, "played": len(played), "outcomes": dict(sorted(outcomes.items())),
        "writes_held": len(held), "writes_held_share": share(len(held)),
        "matched": len(matched), "matched_share": share(len(matched)),
        "not_matched": dict(sorted(not_matched.items())),
        "mismatches": n_writes + n_unplanned, "mismatches_per_executed": share(n_writes + n_unplanned),
        "mismatched_writes": n_writes, "mismatched_writes_per_executed": share(n_writes),
        "unplanned_changes": n_unplanned, "unplanned_changes_per_executed": share(n_unplanned),
        "unplanned_most": unplanned_keys.most_common(12),
        "another_guess": len(choice),
        "final_path": [s["name"] for s in held], "final_path_actions": len(held),
        "inputs": len([e for e in res["inputs"] if replay._is_input(e)]),
        "time": {"step_seconds": stats(secs), "step_frames": stats(frames), "plan_seconds": stats(plan_s),
                 "plan_write_s": stats([p.get("write_s", 0) for p in plans]),
                 "plan_search_s": stats([p.get("seconds", 0) for p in plans]),
                 "harness_s": round(res["seconds"], 1), "planning_s": round(sum(plan_s), 1),
                 "playing_s": round(res["seconds"] - sum(plan_s), 1)},
    }


# --- the run -----------------------------------------------------------------------------------------------------------

def cmd_run(seed: int, reading: Path, control: bool, replan_limit: int, time_limit: float, plan_time: int,
            test_seeds: bool = False, out_root: Path = OUT, results: Path = RESULTS,
            plan_memory: str = replay.PLAN_MEMORY, deadline: float | None = None) -> dict:
    if seed == ANY_SEED or (seed in HELD_OUT and not test_seeds):
        raise SystemExit(f"seed {seed}: the run refuses the Any% seed and the test seeds 31-35 (module docstring)")
    t0 = time.monotonic()
    path = make_control(reading) if control else Path(reading)
    rd = PR.load_reading(path)
    name = rd.doc.get("name", Path(reading).parent.name)
    tag = f"{name}-seed{seed}" + ("-control" if control else "")
    run_dir = out_root / tag
    run_dir.mkdir(parents=True, exist_ok=True)
    goal = rd.task.goal
    end = json.loads(END_PART1.read_text())
    out = {"seed": seed, "reading": str(path), "name": name, "control": control,
           "goal": {k: sorted(v, key=str) for k, v in goal.items()},
           "goal_is_part1_end": goal == {k: {v} for k, v in end["goal_all"].items()} and not end["goal_not"],
           "task": {"actions": len(rd.task.actions), "facts": len(rd.task.facts), "records": rd.doc.get("n_records"),
                    "folded": rd.doc.get("folded"), "packed_sha1": sha(rd.packed), "packed_bytes": rd.packed.stat().st_size,
                    "lines_sha1": sha(rd.lines)},
           "control_counts": rd.doc.get("control_counts"),
           "limits": {"replans": replan_limit, "seconds": time_limit, "plan_time": plan_time, "plan_memory": plan_memory,
                      "input": replay.INPUT_LIMIT, "wait": replay.WAIT_LIMIT, "retries": replay.RETRIES,
                      "retry_wait": replay.RETRY_WAIT},
           "sources": {n: sha(HERE / n) for n in SOURCES}, "taskcore_binary": sha(plan_fd.TASKCORE)}
    out["load_s"] = round(time.monotonic() - t0, 2)
    say(f"{tag}: booting to the first idle stop")
    ctl, branch, row = boot(seed, run_dir / "run")
    h = None
    try:
        out["start"] = {"stop": row["stop"], "dump": row["dump"], "trace": row["trace"], "boot_s": row["boot_s"]}
        say(f"{tag}: idle stop at frame {row['stop']['frame']}; the start check")
        chk = out["start"]["check"] = start_check(row, rd, run_dir / "start-check")
        if row["checks"]:
            raise RuntimeError(f"seed {seed}: the start fails start.py's checks: {row['checks']}")
        if chk["reading start"]["task facts differ"]:
            raise SystemExit(f"seed {seed}: the reading was read from another start ({rd.doc['start']['name']}): "
                             f"{chk['reading start']['task facts differ']} facts of the task differ, first "
                             f"{chk['reading start']['task first'][:5]}; read the scope from this seed's start "
                             f"(module docstring)")
        partial = run_dir / "partial.json"

        def snapshot(hh):
            partial.write_text(json.dumps(dict(out, run=hh.result(), landmarks=hh.landmarks), default=str) + "\n")
        if deadline is not None:  # the harness's time limit ends at the deadline at the latest, counted from here
            time_limit = min(time_limit, max(0.0, deadline - time.time()))
            out["limits"].update(seconds=round(time_limit, 1), deadline=deadline)
        h = replay.Harness(rd.b, rd.task, rd.task, branch, goal, replan_limit=replan_limit, time_limit=time_limit,
                           plan_time=plan_time, progress=say, planner="taskcore", packed=rd.packed,
                           workdir=run_dir / "plans", memory_limit=plan_memory, on_plan=snapshot)
        res = h.run()
        checkpoints = h.checkpoints
    finally:
        ctl.close()
    out["run"] = res
    out["landmarks"] = h.landmarks
    out["summary"] = summary(res)
    say(f"{tag}: stop {res['stop']!r}; verifying from boot ({len(checkpoints)} checkpoints)")
    t = time.monotonic()
    try:
        ver = replay.verify(lambda o, i: config(seed, o, i), res["inputs"], checkpoints, sorted(goal),
                            run_dir / "verify")
        ver["goal"] = all(ver.get("final", {}).get(k) in v for k, v in goal.items())
    except Exception as e:  # noqa: BLE001 (the run's record is written either way)
        ver = {"error": f"{type(e).__name__}: {e}", "goal": False, "differ": [], "inputs": None, "checkpoints": None}
    ver["seconds"] = round(time.monotonic() - t, 1)
    out["verify"] = ver
    out["seconds"] = round(time.monotonic() - t0, 1)
    out["game_seconds"] = releases.get(S.RELEASE).seconds(res["ticks"])
    rd.close()
    results.mkdir(parents=True, exist_ok=True)
    (results / f"replay-part1-{tag}.json").write_text(json.dumps(out, indent=1, default=str) + "\n")
    say(f"wrote {results / f'replay-part1-{tag}.json'}")
    return out


# --- the log -------------------------------------------------------------------------------------------------------------

def run_lines(o: dict) -> list:
    r, s, v = o["run"], o["summary"], o["verify"]
    t = s["time"]
    chk = o["start"]["check"]
    rs = chk["reading start"]
    lines = [f"=== seed {o['seed']}{' (control)' if o['control'] else ''} ===",
             f"reading {o['reading']}: {o['task']['actions']:,} actions, {o['task']['facts']:,} facts, packed file "
             f"{o['task']['packed_bytes']:,} bytes (sha1 {o['task']['packed_sha1']}); goal {o['goal']} "
             f"({'the Part I end' if o['goal_is_part1_end'] else 'not the Part I end'})"]
    if o.get("control_counts"):
        c = o["control_counts"]
        lines.append(f"control: precondition sets kept {c['sets kept']:,}, dropped {c['sets dropped']:,}; replay "
                     f"checks dropped {c.get('checks dropped')}; stepped facts made engine conditions "
                     f"{c.get('stepped facts made engine conditions')}")
    lines += [f"start: idle stop at frame {o['start']['stop']['frame']} ({o['start']['stop']['idle']}), room "
              f"{o['start']['stop']['room']}, boot {o['start']['boot_s']} s; checks {chk['boot checks']}; against the "
              f"reading's start ({rs['name']}): {rs['differ']} facts differ, {rs.get('task facts differ')} of them "
              f"the task's; taskcore start check: input {chk['taskcore']['input']['status']}, needed "
              f"{chk['taskcore']['needed']['status']}",
              f"result: stop {r['stop']!r}; goal reached {r['goal']}; verify from boot: {len(v['differ'])} differences "
              f"at {v['checkpoints']} checkpoints ({v['inputs']} inputs, {v['seconds']} s), the goal at its end "
              f"{v['goal']}"
              + (f"; verify error {v['error']}" if v.get("error") else ""),
              f"replans {r['replans']}; bans {len(r['bans'])}; plans {len(r['plans'])}; frames {r['frames']} "
              f"(game time {o['game_seconds']:.1f} s); player inputs {s['inputs']}",
              f"steps: {s['steps']} in the log, {s['executed']} executed ({s['played']} played); outcomes {s['outcomes']}",
              f"matched (plan.md: the predicted writes and no other tracked fact changed): {s['matched']} of "
              f"{s['executed']} executed ({s['matched_share']}); predicted writes held: {s['writes_held']} "
              f"({s['writes_held_share']}); not matched: {s['not_matched']}",
              f"mismatches: {s['mismatches']} ({s['mismatches_per_executed']} per executed action): "
              f"{s['mismatched_writes']} predicted writes not seen ({s['mismatched_writes_per_executed']}), "
              f"{s['unplanned_changes']} tracked facts changed with no prediction "
              f"({s['unplanned_changes_per_executed']}; most often {s['unplanned_most'][:8]}); "
              f"{s['another_guess']} steps with another guessed value than the plan's",
              f"final path: {s['final_path_actions']} actions {s['final_path'][:30]}"
              + (" ..." if len(s["final_path"]) > 30 else "")]
    sc = o.get("score")
    if sc:
        lines.append(f"Part I goals passed through (score_part1.py, from boot, every frame): {sc['reached_in_the_run']} "
                     f"of {sc['of']} reached in the run {sc['reached']}; {sc['held_at_start']} held at the start "
                     f"{sc['at_start']} (not counted); the Part I end (#97-99) {sc['part1_end']}")
    else:
        lines.append("Part I goals passed through: not scored yet (score_part1.py)")
    lines += [f"time: run {o['seconds']} s: load {o['load_s']} s, boot {o['start']['boot_s']} s, start check "
              f"{chk['taskcore_s']} s, harness {t['harness_s']} s (planning {t['planning_s']} s, playing "
              f"{t['playing_s']} s), verify {v['seconds']} s" + (f"; scoring {sc['seconds']} s" if sc else ""),
              f"  per step (played): seconds {t['step_seconds']}; frames {t['step_frames']}",
              f"  per plan: all {t['plan_seconds']}; taskcore {t['plan_write_s']}; lama-first {t['plan_search_s']}"]
    lm = o.get("landmarks")
    if lm:
        game = [n for n in lm["nodes"] if not n["aux"]]
        lines.append(f"landmarks (plan {lm['plan']}, frame {lm['frame']}): {len(lm['nodes'])} nodes, {len(game)} on "
                     f"game facts, {len(lm['orderings'])} orderings; the game facts: "
                     + "; ".join(" | ".join(f"{k}={x}" for k, x in n["facts"]) + (" (start)" if n["initial"] else "")
                                 + (" (goal)" if n["goal"] else "") for n in game[:40])
                     + (f" (+{len(game) - 40})" if len(game) > 40 else ""))
    else:
        lines.append("landmarks: none dumped")
    for p in r["plans"]:
        lines.append(f"  plan {p['plan']} at frame {p['frame']}: {p.get('status')}, {len(p.get('steps', []))} steps, "
                     f"taskcore {p.get('write_s', 0):.2f} s, lama-first {p.get('seconds', 0):.2f} s, "
                     f"{p.get('operators') or 0:,} operators, {p['bans']} bans, landmarks {p.get('landmarks')}"
                     + (f"; {p['error']}" if p.get("error") else "") + f": {p.get('steps', [])[:12]}")
    for b in r["bans"]:
        lines.append(f"  ban: {b['label']} at {dict(b['values'])}")
    for st in r["steps"]:
        ins = [(e["frame"], e.get("click") or e.get("key")) for t_ in st["tries"] for e in t_.get("inputs", [])]
        w, u = step_mismatches(st) if executed(st) else ([], [])
        lines.append(f"  {st['plan']}.{st['index'] + 1} {st['name']} [{st['kind']}] {st['input']}: frames "
                     f"{st['start']}/{st['end']}, {st.get('seconds', 0):.1f} s; {st['outcome']} -> {st['then']}; "
                     f"{len(st['tries'])} tries" + (f"; inputs {ins}" if ins else "")
                     + (f"; mismatches {[(m['key'], m['predicted'], m['observed']) for m in st['mismatches'][:6]]}"
                        if st["mismatches"] else "")
                     + (f"; changed with no prediction {[(x['key'], x['start'], x.get('end')) for x in u[:6]]}"
                        + (f" (+{len(u) - 6})" if len(u) > 6 else "") if u else "")
                     + (f"; {st['note']}" if st["note"] else ""))
    if r.get("error"):
        lines.append(f"  error: {r['error'].strip().splitlines()[-1]}")
    for d in v["differ"][:5]:
        lines.append(f"  verify differs at {d}")
    return lines


def start_lines(o: dict) -> list:
    chk = o["check"]
    rs = chk["reading start"]
    tc = chk["taskcore"]
    known = chk.get("start-part1")
    return [f"=== start check, seed {o['seed']} (no goal run) ===",
            f"idle stop at frame {o['stop']['frame']} (tick {o['stop']['tick']}), kind {o['stop']['idle']}, room "
            f"{o['stop']['room']}, ego {o['dump']['ego']} at {o['dump']['ego_pos']}, rng {o['stop']['rng']}; boot "
            f"{o['boot_s']} s; start.py's checks: {chk['boot checks']}; trace records {o['trace']}",
            f"against the reading's start ({rs['name']}): {rs['differ']} facts differ: {rs['first']}; of them the "
            f"task's: {rs.get('task facts differ')} {rs.get('task first')} (a goal run "
            f"{'refuses' if rs.get('task facts differ') else 'takes'} this seed with this reading)",
            (f"against results/start-part1.jsonl: {known['differ']} facts differ, the same frame {known['same frame']}"
             if known else "results/start-part1.jsonl has no row for this seed"),
            f"taskcore's start check on the reading: input {tc['input']['status']}"
            + (f" ({tc['input']['misses']} facts, first {tc['input']['first']})" if tc["input"].get("misses") else "")
            + f", {tc['input']['seconds']} s; needed {tc['needed']['status']}"
            + (f" ({tc['needed']['misses']} facts, first {tc['needed']['first']})" if tc["needed"].get("misses") else "")
            + f", {tc['needed']['seconds']} s",
            f"time: {o['seconds']} s in all"]


def cmd_log(name: str, results: Path = RESULTS) -> None:
    files = sorted(results.glob(f"replay-part1-{name}-*.json"))
    if not files:
        raise SystemExit(f"no results for {name}")
    lines = [f"E003 step 5 on Part I: the runner (replay_part1.py) on the reading {name!r}", ""]
    for f in files:
        o = json.loads(f.read_text())
        lines += [f"({f.name}; sources {o.get('sources')})"]
        lines += (start_lines(o) if "check" in o else run_lines(o)) + [""]
    (results / f"replay-part1-{name}.log").write_text("\n".join(lines))
    print("\n".join(lines))


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    e = sub.add_parser("export")
    e.add_argument("rooms")
    e.add_argument("--seed", type=int, required=True)
    e.add_argument("--goal", required=True)
    e.add_argument("--name", required=True)
    e.add_argument("--out", type=Path, required=True)
    e.add_argument("--workers", type=int, default=1)
    ep = sub.add_parser("export-pickle")
    ep.add_argument("pickle", type=Path)
    ep.add_argument("--goal", required=True)
    ep.add_argument("--name", required=True)
    ep.add_argument("--start-name", required=True)
    ep.add_argument("--out", type=Path, required=True)
    c = sub.add_parser("control")
    c.add_argument("reading", type=Path)
    s = sub.add_parser("start")
    s.add_argument("seeds", type=int, nargs="+")
    s.add_argument("--reading", type=Path, required=True)
    s.add_argument("--results", type=Path, default=RESULTS)
    r = sub.add_parser("run")
    r.add_argument("seed", type=int)
    r.add_argument("--reading", type=Path, required=True)
    r.add_argument("--control", action="store_true")
    r.add_argument("--replan-limit", type=int, default=500)
    r.add_argument("--time-limit", type=float, default=24 * 3600)
    r.add_argument("--plan-time", type=int, default=replay.PLAN_TIME)
    r.add_argument("--plan-memory", default=replay.PLAN_MEMORY)
    r.add_argument("--results", type=Path, default=RESULTS)
    r.add_argument("--test-seeds", action="store_true", help="allow the test seeds 31-35 (the Part I result only)")
    r.add_argument("--deadline", type=float, default=None,
                   help="UTC epoch seconds: the harness's time limit ends there at the latest (a VM's end, less a plan)")
    lg = sub.add_parser("log")
    lg.add_argument("name")
    lg.add_argument("--results", type=Path, default=RESULTS)
    a = ap.parse_args()
    if a.cmd == "export":
        print(json.dumps(export_python(tuple(int(x) for x in a.rooms.split(",")), a.seed, a.goal, a.name, a.out,
                                       a.workers), indent=1))
    elif a.cmd == "export-pickle":
        print(json.dumps(export_pickle(a.pickle, a.goal, a.name, a.start_name, a.out), indent=1))
    elif a.cmd == "control":
        out = make_control(a.reading)
        print(out, json.dumps(json.loads(out.read_text()).get("control_counts")))
    elif a.cmd == "start":
        cmd_start(a.seeds, a.reading, a.results)
    elif a.cmd == "run":
        cmd_run(a.seed, a.reading, a.control, a.replan_limit, a.time_limit, a.plan_time, a.test_seeds,
                results=a.results, plan_memory=a.plan_memory, deadline=a.deadline)
    else:
        cmd_log(a.name, a.results)


if __name__ == "__main__":
    main()
