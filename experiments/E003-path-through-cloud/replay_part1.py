"""E003 step 5 on Part I: per seed, boot, take the start from the engine, plan to the reading's goal, play the plans with
the replay harness under plan.md's limits, verify from boot, and report.

usage (from this folder, with ../../.venv/bin/python, under nice):
       replay_part1.py export ROOMS --seed N --goal FACT=V[,FACT=V] --name NAME --out DIR
                       (a reading from the Python reader: ROOMS comma-separated, at most two rooms; the start is
                       results/start-part1.jsonl's seed N; ``--goal part1`` is results/end-part1.json)
       replay_part1.py export-pickle PICKLE --goal ... --name NAME --start-name TEXT --out DIR   (a pickled reading)
       replay_part1.py read-rust ROOMS --seed N --goal FACT=V[,FACT=V] --name NAME --out DIR [--threads 3]
                       [--elsewhere] [--live-locals] [--split-limit] [--switches S,...] [--reach] [--frame-costs]
                       [--start-rows FILE]
                       (a reading from the Rust reader, scumm/target/release/reader-scope, with golden.read_scope's
                       domains (no goal), made into the three files and the lists by packed_reading.reading_from_golden;
                       the switches are reader-scope's, off by default)
       replay_part1.py control DIR/reading.json           (the control's reading next to it: DIR/control.json)
       replay_part1.py start SEED [SEED ...] --reading DIR/reading.json [--results DIR] [--serve]
                       (the start check only, no goal run)
       replay_part1.py run SEED --reading DIR/reading.json [--control] [--replan-limit 500] [--time-limit 86400]
                       [--plan-time 600] [--plan-memory 6G] [--results DIR] [--deadline EPOCH]
                       [--slot-locals [--reread]] [--serve] [--search S] [--cost-caps] [--reach-extend]
                       [--unobserved-actor-rooms] [--approach-stones] [--window-search [--workers 7]]
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

**The reader's switches.** A reading the Rust reader made with ``--live-locals`` says so in reading.json
(packed_reading.py); the run's record says it too (``task.live_locals``), and the harness reads the reading's slot
read sets from it (replay.read_view). Nothing else in the run changes: the starts compare no slot's locals. A reading
made with ``--split-limit`` says ``"split_limit": true`` in reading.json, and F10's reread keeps the switch.

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

**The slots' locals** (fix round 1, F10; ``--slot-locals``, off by default): the engine runs with the bridge's
locals dump (EngineConfig.dump_locals; a build with it: FAST_MI_SCUMMVM_BIN or the shared build once switched), and
at a replan the harness gives each live slot the ``args:`` number of the built list its dumped locals match (the
reading's lists file; replay.replan_start_locals). With ``--reread``, a slot whose locals match no list has its events
read again: the Rust reader reads the reading's scope again from its start with those slots' dumped locals
(reader-scope ``--slot-locals``), and the harness goes on with that reading (and its control with ``--control``). The
run's file then ends in ``-locals``. Without the flag the run is as before.

**The reader's other switches** (read-rust ``--switches``, ``--reach``, ``--frame-costs``, off by default): passed to
reader-scope as given (``--switches``: fix round 1's reader switches by name, value_sets, draw_clears, actor_rooms,
input_settled; ``--reach``: forward reachability, experiments/E004-fast-exact/reach.md, which needs ``--split-limit``;
``--frame-costs``: each action costs its record's frames). reading.json says which (``switches``, ``reach``; the frame
costs' parameters as ``frame_costs``), and F10's reread and the reach extension read again with the same. With
``--reach`` the reader writes the reached values next to the packed file, and they are kept next to reading.tc
(``reading.reached``), where taskcore reads them: a start holding a value outside them is refused (reach.md, "A replan
start"). Without the switches the command and the files are as before.

**The search and the costs** (``--search``, ``--cost-caps``): taskcore's search (plan_fd.SEARCHES: lama-first, the
default; lama-first-costs and astar-lmcp plan with a ``--frame-costs`` reading's costs; anytime) and the harness's frame
cost caps (replay.py, "Frame costs"; they need a ``--frame-costs`` reading, whose dmax reading.json has). The run's file
then ends in ``-<search>`` (when it is not lama-first) and ``-caps``.

**The reach extension** (``--reach-extend``, off by default; a ``--reach`` reading): at a replan start the reading does
not cover, the harness (replay.py ``reach_extend``) writes that start, and the Rust reader reads the reading's scope
again from its start and every start extended so far (reader-scope ``--reach-also``, one per start), with the
reading's own switches, in run_dir/reach-NN; the harness goes on with that reading. Each extension is in the record
(``run.extensions``: the plan, the frame, the values, the wall and CPU seconds, the actions before and after). The
run's file then ends in ``-reachx``. Without the flag such a start stops the run as a taskcore error.

**The actors' rooms** (``--unobserved-actor-rooms``, off by default; a reading with the reader switch actor_rooms):
the harness treats the ``actorroom:`` facts as unobserved, with the plan's values, and not known at the first plan
(replay.py, "The actors' rooms"); the start check gives them the same. The run's file then ends in ``-uar``.

**Stepping stones** (``--approach-stones``, off by default): the Executor's stepping stones are tried on forks of the
run's engine (replay.py, "The camera"), for an object off screen that floor clicks do not bring on screen. The run's
file then ends in ``-stones``.

**The window search** (``--window-search``, off by default; plan step A4, window-search.md, window.py): each plan is
played as its macro steps, each input tried on every frame of its window on forked children (``--workers`` at a time,
7), and the child that completes the macro step first on the bridge's clock kept (replay.py, "The window search").
Lemma 2's floor on a frame's quarter-frames comes from the reading's frame costs (``frame_costs.dmax``). The run's
file then ends in ``-window``, and its record has the windows, the replans' causes, the run clock in quarter-frames
(``game_qf``) and the search's CPU.

**The refinement loop** (``--refine-facts``, off by default; E007, experiments/E007-fact-refinement/plan.md; needs
``--window-strict`` and ``--reach-extend``): after a window that ends a plan's play with a replan whose B1 cause is a
split on variables the reading holds no fact of, the harness (replay.py, "The refinement loop") has the scope read
again by the Rust reader with those variables made facts (read-rust ``--fact-vars``, ``refiner``), from the reading's
starts and the engine's state then, with the reading's own switches, in run_dir/refine-NN, and plans from the engine's
state with that reading. The reach extension and F10's reread read with the run's fact variables from then on. The
run's file then ends in ``-refine`` and its record has ``refinements``. Without the flag the run is as before.

**Another start** (``read-rust --start-rows FILE``, and ``cmd_run``'s ``starter``; off by default): the reading's
start is the row of the seed in FILE (start-part1.jsonl's format) in place of results/start-part1.jsonl, and a run
boots through the starter (an instrument's: its boot, its engine config for verify and the inputs it gives before the
start, which verify plays first). runb_start.py is such an instrument. Without them the commands are as before.

**Without reloading** (``--serve``, off by default): taskcore runs as one long-lived process per packed file
(plan_fd ``serve``, taskcore/src/serve.rs), which keeps the loaded reading and what does not depend on the start, for
the start check (its two modes are two requests) and every plan of the run. The files it writes, and so the plans,
are the ones ``taskcore sas`` writes. The run's file then ends in ``-serve`` and its record says ``taskcore_serve``.

C2: this file names no game number. The seeds are plan.md's; ``run`` refuses the held-out seeds 31-35 (only with
``--test-seeds``, for the Part I result itself) and the Any% seed always; ``start`` allows 31-35 (a start check
plays nothing).
"""

import argparse
import collections
import hashlib
import json
import os
import pickle
import re
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


def config(seed: int, out_dir: Path, inputs: Path | None = None, dump_locals: bool = False) -> EngineConfig:
    """plan.md's engine settings for ``seed`` (start.py ``config``), with ``inputs`` as the raw input file (verify)
    and ``dump_locals`` the bridge's locals dump (F10: observation only). The Any% seed is refused."""
    if seed == ANY_SEED:
        raise ValueError("the Any% seed is never used by the system's runs (plan.md)")
    rel = releases.get(S.RELEASE)
    return EngineConfig(out_dir=out_dir, game_path=releases.game_dir(rel), release=S.RELEASE, seed=seed,
                        boot_param=S.BOOT_PARAM, max_ticks=S.MAX_TICKS, timer_next="any", skip_text=False,
                        skip_cutscenes=False, raw_inputs=inputs, dump_locals=dump_locals)


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


def start_row(seed: int, rows: Path = START_ROWS) -> dict | None:
    if not rows.exists():
        return None
    return next((r for r in map(json.loads, rows.read_text().splitlines()) if r["seed"] == seed), None)


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


READER_SCOPE = Path(os.environ.get("READER_SCOPE_BIN") or HERE / "scumm" / "target" / "release" / "reader-scope")


def goal_text_of(goal: dict) -> str:
    """A reading's goal ({fact: [value]}) as reader-scope's ``--goal`` (one value per fact)."""
    parts = []
    for k, vs in sorted(goal.items()):
        if len(vs) != 1:
            raise SystemExit(f"reader-scope's --goal takes one value per fact; {k} has {vs}")
        parts.append(f"{k}={next(iter(vs))}")
    return ",".join(parts)


def read_rust(rooms: tuple, seed: int, goal_text: str, name: str, out: Path, threads: int = 3,
              slot_locals: list | None = None, domain_goals: str = "none", elsewhere: bool = False,
              live_locals: bool = False, split_limit: bool = False, switches: str = "", reach: bool = False,
              frame_costs: bool = False, reach_also: list | None = None, start_rows: Path | None = None,
              fact_vars: list | None = None) -> Path:
    """A reading from the Rust reader (module docstring): reader-scope on ``rooms`` from results/start-part1.jsonl's
    seed ``seed`` (``domain_goals`` "none": golden.read_scope's domains), its records kept and written in golden.py's
    format, then packed_reading.reading_from_golden: the three files and the lists. ``slot_locals`` (F10): slots
    whose events are read from their dumped locals (reader-scope ``--slot-locals``). ``elsewhere`` and
    ``live_locals``: reader-scope's switches ``--elsewhere`` and ``--live-locals`` (reading.json then says
    ``"elsewhere": true``, and has ``live_locals``); ``split_limit``: reader-scope's switch ``--split-limit``
    (reading.json then says ``"split_limit": true``); ``switches``, ``reach``, ``frame_costs``: reader-scope's
    ``--switches``, ``--reach``, ``--frame-costs`` (module docstring, "The reader's other switches"); ``reach_also``:
    further start files (reader-scope ``--reach-also``, with ``reach``); ``start_rows``: the start's row from that
    file (start-part1.jsonl's format) in place of results/start-part1.jsonl (reader-scope ``--start-rows``; off by
    default); ``fact_vars``: variables made facts (reader-scope ``--fact-vars``, E007's refinement; reading.json then
    has ``fact_vars`` and the reader's report of them, ``fact_vars_report``; off by default). Returns reading.json."""
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    row = start_row(seed) if start_rows is None else start_row(seed, Path(start_rows))
    if row is None:
        raise SystemExit(f"no start for seed {seed} in {start_rows or START_ROWS}")
    cmd = ["nice", "-n", "10", str(READER_SCOPE), "--rooms", ",".join(map(str, rooms)), "--start-seed", str(seed),
           "--domain-goals", domain_goals, "--goal", goal_text, "--name", name, "--threads", str(threads),
           "--out", str(out), "--packed", str(out / f"{name}.rust.tc"), "--keep", "--lines",
           str(out / f"{name}.golden.jsonl.gz")]
    if elsewhere:
        cmd.append("--elsewhere")
    if live_locals:
        cmd.append("--live-locals")
    if split_limit:
        cmd.append("--split-limit")
    if switches:
        cmd += ["--switches", switches]
    if reach:
        cmd.append("--reach")
    for f in reach_also or ():
        if not reach:
            raise ValueError("--reach-also needs --reach")
        cmd += ["--reach-also", str(f)]
    if frame_costs:
        cmd.append("--frame-costs")
    if start_rows is not None:
        cmd += ["--start-rows", str(start_rows)]
    if fact_vars:
        cmd += ["--fact-vars", ",".join(str(int(n)) for n in fact_vars)]
    if slot_locals:
        sl = out / "slot-locals.jsonl"
        sl.write_text("".join(json.dumps(x) + "\n" for x in slot_locals))
        cmd += ["--slot-locals", str(sl)]
    t = time.monotonic()
    p = subprocess.run(cmd, capture_output=True, text=True)
    (out / "reader-scope.err").write_text(p.stderr)
    if p.returncode != 0:
        raise RuntimeError(f"reader-scope failed ({p.returncode}): {p.stderr.strip()[-600:]}")
    read_s = time.monotonic() - t
    rep = json.loads((out / f"{name}.json").read_text())
    t = time.monotonic()
    info = PR.reading_from_golden(out / f"{name}.golden.jsonl.gz", out / f"{name}.rust.tc", out, row["facts"],
                                  f"results/start-part1.jsonl seed {seed}" if start_rows is None else
                                  f"{start_rows} seed {seed}", name="reading",
                                  info={"name": name, "rooms": list(rooms), "domain_goals": domain_goals,
                                        "reader": "rust", "read_s": round(read_s, 1),
                                        "reader_scope": {k: rep.get(k) for k in ("records", "read_wall_s",
                                                                                 "peak_rss_mb", "threads",
                                                                                 "slot_locals")
                                                         + (("read_cpu_s",) if reach else ())},
                                        "slot_locals": slot_locals or [],
                                        "sources": {n: sha(HERE / n) for n in ("packed_reading.py",)},
                                        **({"elsewhere": True} if elsewhere else {}),
                                        **({"split_limit": True} if split_limit else {}),
                                        **({"switches": switches} if switches else {}),
                                        **({"reach": True} if reach else {}),
                                        **({"reach_also": [str(f) for f in reach_also]} if reach_also else {}),
                                        **({"fact_vars": [int(n) for n in fact_vars],
                                            "fact_vars_report": rep.get("fact_vars")} if fact_vars else {})})
    (out / f"{name}.rust.tc").unlink(missing_ok=True)  # reading_from_golden copied it to reading.tc
    reached, kept = out / f"{name}.rust.reached", Path(info["packed"]).with_suffix(".reached")
    if reach:
        # taskcore reads the reached values next to the packed file (taskcore bans::reached_file)
        if not reached.exists():
            raise RuntimeError(f"reader-scope --reach wrote no {reached}")
        reached.replace(kept)
    else:
        kept.unlink(missing_ok=True)  # an older reading's, as the reader removes its own
    # the lists the dumped slots' events were read with: no record's next event, so the lists file lacks them
    head = Path(info["json"])
    doc = json.loads(head.read_text())
    lists = out / doc["lists"]
    have = {(x["name"], x["w"], x["sig"]) for x in (json.loads(ln) for ln in lists.read_text().splitlines()
                                                     if ln.strip())}
    extra = [x for x in rep.get("slot_events", []) if (x["name"], x["w"], x["sig"]) not in have]
    with open(lists, "a") as f:
        f.write("".join(json.dumps(x) + "\n" for x in extra))
    doc["n_lists"] += len(extra)
    doc["slot_lists"] = len(rep.get("slot_events", []))
    head.write_text(json.dumps(doc, indent=1, default=str) + "\n")
    info["lists"] += len(extra)
    say(f"read {name}: {rep.get('records')} records in {read_s:.1f} s ({threads} threads, peak "
        f"{rep.get('peak_rss_mb')} MB), files in {time.monotonic() - t:.1f} s: {info['actions']} actions, "
        f"{info['lists']} lists")
    return Path(info["json"])


def rereader(rd, run_dir: Path, control: bool, threads: int, reach_starts: list | None = None,
             facts: list | None = None):
    """F10's ``reread`` for the harness (module docstring): the reading's scope again, from its start, with the
    slots' dumped locals and the reading's own switches (``--elsewhere``: reading.json says so, or its domains hold
    ELSEWHERE; ``--live-locals``: reading.json has ``live_locals``; ``--split-limit``: reading.json says so); its
    control too with ``control``; ``facts``: the run's fact variables so far (E007's loop), when it has any. Each in
    run_dir/reread-NN."""
    doc = rd.doc
    m = re.fullmatch(r"results/start-part1\.jsonl seed (\d+)", doc["start"]["name"])
    if m is None:
        raise SystemExit(f"--reread needs a reading of results/start-part1.jsonl's starts, not {doc['start']['name']}")
    seed, rooms = int(m[1]), tuple(doc["scope"]["rooms"])
    goal = goal_text_of(doc["goal"])
    elsewhere = bool(doc.get("elsewhere")) or bool(getattr(rd.b, "elsewhere", None))
    live_locals = doc.get("live_locals") is not None
    split_limit = bool(doc.get("split_limit"))
    count = [0]

    def reread(slots: list, h) -> PR.Reading:
        count[0] += 1
        # the reading's own switches, and the starts a reach extension added so far (``reach_starts``)
        path = read_rust(rooms, seed, goal, f"reread-{count[0]:02d}", run_dir / f"reread-{count[0]:02d}", threads,
                         slots, doc.get("domain_goals", "none"), elsewhere=elsewhere, live_locals=live_locals,
                         split_limit=split_limit, **with_facts(reader_switches(doc, reach_starts), facts))
        if control:
            path = make_control(path)
        return PR.load_reading(path)
    return reread


def reader_switches(doc: dict, reach_also=None) -> dict:
    """read_rust's ``switches``, ``reach``, ``frame_costs``, ``reach_also`` and ``fact_vars`` for a reading made again
    with a reading's own switches (reading.json, module docstring); none when it had none."""
    out = {}
    if doc.get("fact_vars"):
        out["fact_vars"] = list(doc["fact_vars"])
    if doc.get("switches"):
        out["switches"] = doc["switches"]
    if doc.get("reach"):
        out["reach"] = True
    if doc.get("frame_costs") is not None:
        out["frame_costs"] = True
    if reach_also:
        out["reach_also"] = list(reach_also)
    return out


def with_facts(sw: dict, facts: list | None) -> dict:
    """``reader_switches``' map with the run's fact variables so far (E007's loop, ``refiner``), when it has any."""
    return dict(sw, fact_vars=list(facts)) if facts else sw


def reach_extender(rd, run_dir: Path, threads: int, starts: list, facts: list | None = None):
    """The harness's ``reach_extend`` (module docstring, "The reach extension"): the start as a file, then the reading's
    scope read again by the Rust reader from the reading's start and every start so far (``starts``, the start files,
    which F10's reread shares), with its own switches and the run's fact variables so far (``facts``, E007's loop), in
    run_dir/reach-NN."""
    doc = rd.doc
    if not doc.get("reach"):
        raise SystemExit("--reach-extend needs a reading made with --reach")
    m = re.fullmatch(r"results/start-part1\.jsonl seed (\d+)", doc["start"]["name"])
    # a reading of another start's row (read-rust --start-rows): read again from the same file
    m2 = None if m is not None else re.fullmatch(r"(/\S+\.jsonl) seed (\d+)", doc["start"]["name"])
    if m is None and m2 is None:
        raise SystemExit(f"--reach-extend needs a reading of results/start-part1.jsonl's starts or of a start-rows "
                         f"file, not {doc['start']['name']}")
    seed, rooms = int((m or m2)[1 if m else 2]), tuple(doc["scope"]["rooms"])
    start_rows = Path(m2[1]) if m2 is not None else None
    goal = goal_text_of(doc["goal"])
    elsewhere = bool(doc.get("elsewhere")) or bool(getattr(rd.b, "elsewhere", None))
    starts += [Path(f) for f in doc.get("reach_also") or () if Path(f) not in starts]

    def extend(start: dict, h) -> PR.Reading:
        n = len(starts) + 1
        d = run_dir / f"reach-{n:02d}"
        d.mkdir(parents=True, exist_ok=True)
        f = d / "start.txt"
        f.write_text("".join(f"{k}\t{v}\n" for k, v in start.items()))
        starts.append(f)
        path = read_rust(rooms, seed, goal, f"reach-{n:02d}", d, threads, None, doc.get("domain_goals", "none"),
                         elsewhere=elsewhere, live_locals=doc.get("live_locals") is not None,
                         split_limit=bool(doc.get("split_limit")),
                         **({"start_rows": start_rows} if start_rows is not None else {}),
                         **with_facts(reader_switches(doc, starts), facts))
        return PR.load_reading(path)
    return extend


def refiner(rd, run_dir: Path, threads: int, starts: list, facts: list):
    """E007's refinement (module docstring, "The refinement loop"; replay.py ``refine``): the reading's scope read again
    by the Rust reader with the reading's own switches, from its start, every start so far (``starts``, the reach
    extension's files, which it shares) and the engine's state now as one more ``--reach-also`` start, with the run's
    fact variables (``facts``, which the reach extension and F10's reread read with too) and the new ones made facts
    (``--fact-vars``), in run_dir/refine-NN."""
    doc = rd.doc
    if not doc.get("reach"):
        raise SystemExit("--refine-facts needs a reading made with --reach")
    m = re.fullmatch(r"results/start-part1\.jsonl seed (\d+)", doc["start"]["name"])
    m2 = None if m is not None else re.fullmatch(r"(/\S+\.jsonl) seed (\d+)", doc["start"]["name"])
    if m is None and m2 is None:
        raise SystemExit(f"--refine-facts needs a reading of results/start-part1.jsonl's starts or of a start-rows "
                         f"file, not {doc['start']['name']}")
    seed, rooms = int((m or m2)[1 if m else 2]), tuple(doc["scope"]["rooms"])
    start_rows = Path(m2[1]) if m2 is not None else None
    goal = goal_text_of(doc["goal"])
    elsewhere = bool(doc.get("elsewhere")) or bool(getattr(rd.b, "elsewhere", None))
    count = [0]

    def refine(new: list, start: dict, h) -> PR.Reading:
        count[0] += 1
        d = run_dir / f"refine-{count[0]:02d}"
        d.mkdir(parents=True, exist_ok=True)
        f = d / "start.txt"
        f.write_text("".join(f"{k}\t{v}\n" for k, v in start.items()))
        starts.append(f)
        facts.extend(n for n in new if n not in facts)
        say(f"refinement {count[0]}: reading the scope again with var {facts} made facts, from {len(starts) + 1} "
            f"starts")
        path = read_rust(rooms, seed, goal, f"refine-{count[0]:02d}", d, threads, None,
                         doc.get("domain_goals", "none"), elsewhere=elsewhere,
                         live_locals=doc.get("live_locals") is not None, split_limit=bool(doc.get("split_limit")),
                         **({"start_rows": start_rows} if start_rows is not None else {}),
                         **with_facts(reader_switches(doc, starts), facts))
        return PR.load_reading(path)
    return refine


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

def taskcore_start_check(rd, start_facts: dict, work: Path, serve: bool = False,
                         unobserved_actor_rooms: bool = False) -> dict:
    """taskcore's start check on the reading's packed file (module docstring): per mode, "ok" or the facts outside
    their domains (the first ten) and how many, and the seconds. ``serve``: as two requests to the long-lived
    taskcore (module docstring), one after the other. ``unobserved_actor_rooms``: the actors' rooms not known, as the
    harness's first plan gives them with that switch (replay.py)."""
    work.mkdir(parents=True, exist_ok=True)
    st = replay.task_start(rd.b, start_facts, rd.task)
    if unobserved_actor_rooms:
        st.update({k: replay.ACTOR_ROOM_NOT_KNOWN for k in rd.task.facts if k.startswith(replay.ACTOR_ROOM)})
    sf = export_records.write_start(work / "start.txt", rd.task.facts, st)
    out = {}
    if serve:
        for mode in ("input", "needed"):
            (work / mode).mkdir(exist_ok=True)
            t = time.monotonic()
            ok, reply = plan_fd.server_for(rd.packed).request(
                ["--start-file", sf, "--out", work / mode / "task.sas", "--stats", work / mode / "stats.json",
                 "--start-check", mode])
            row = {"seconds": round(time.monotonic() - t, 2)}
            if ok:
                row["status"] = "ok"
                row["serve"] = reply
            else:
                m = re.search(r"(\d+) facts out of their domains: \[(.*)\]", reply)
                row["status"] = "out of domain" if m else "error"
                row["error"] = reply.strip()[-400:] if not m else None
                if m:
                    keys = re.findall(r'"((?:[^"\\]|\\.)*)"', m[2])
                    row["misses"] = int(m[1])
                    row["first"] = [(k, start_facts.get(k, 0)) for k in keys[:10]]
            out[mode] = row
            (work / mode / "task.sas").unlink(missing_ok=True)
        return out
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


def boot(seed: int, out_dir: Path, dump_locals: bool = False) -> tuple:
    """A fresh engine on ``seed`` run to its first idle stop: (Controller, root Branch, row). The row holds start.py's
    fields and checks (module docstring)."""
    t = time.monotonic()
    ctl = Controller(config(seed, out_dir, dump_locals=dump_locals), timeout_s=S.TIMEOUT_S)
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


def start_check(row: dict, rd, work: Path, serve: bool = False, unobserved_actor_rooms: bool = False,
                rows: Path = START_ROWS) -> dict:
    """The start against the reading's start (every fact, and the facts the packed file holds: a reading serves the
    start its domains were built from, module docstring), results/start-part1.jsonl's row, and taskcore's check."""
    f = row["facts"]
    out = {"boot checks": row["checks"] or "ok"}
    d = diff(rd.doc["start"]["facts"], f)
    task_d = sorted((k, v) for k, v in d.items() if k in rd.task.facts)
    out["reading start"] = {"name": rd.doc["start"]["name"], "differ": len(d), "first": sorted(d.items())[:20],
                            "task facts differ": len(task_d), "task first": task_d[:20]}
    known = start_row(row["seed"], rows)
    if known is not None:
        d2 = diff(known["facts"], f)
        out["start-part1"] = {"differ": len(d2), "frame": known["stop"]["frame"],
                              "same frame": known["stop"]["frame"] == row["stop"]["frame"]}
    t = time.monotonic()
    out["taskcore"] = taskcore_start_check(rd, f, work, serve, unobserved_actor_rooms)
    out["taskcore_s"] = round(time.monotonic() - t, 2)
    return out


def cmd_start(seeds: list, reading: Path, results: Path = RESULTS, out_root: Path = OUT, serve: bool = False) -> list:
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
        chk = start_check(row, rd, run_dir / "taskcore", serve)
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
            plan_memory: str = replay.PLAN_MEMORY, deadline: float | None = None, slot_locals: bool = False,
            reread: bool = False, threads: int = 3, serve: bool = False, search: str = "lama-first",
            cost_caps: bool = False, reach_extend: bool = False, unobserved_actor_rooms: bool = False,
            approach_stones: bool = False, window_search: bool = False, workers: int = 7, starter=None,
            window_strict: bool = False, window_cache: bool = False, window_fork_each: int = 0,
            window_lite: bool = False, window_split_wait: bool = False, refine_facts: bool = False) -> dict:
    if seed == ANY_SEED or (seed in HELD_OUT and not test_seeds):
        raise SystemExit(f"seed {seed}: the run refuses the Any% seed and the test seeds 31-35 (module docstring)")
    t0 = time.monotonic()
    path = make_control(reading) if control else Path(reading)
    rd = PR.load_reading(path)
    name = rd.doc.get("name", Path(reading).parent.name)
    if reread and not slot_locals:
        raise SystemExit("--reread needs --slot-locals")
    if slot_locals and rd.lists is None:
        raise SystemExit(f"{path}: the reading has no lists file (make it with read-rust or export)")
    frame_costs = rd.doc.get("frame_costs")
    if cost_caps and not (frame_costs or {}).get("dmax"):
        raise SystemExit(f"{path}: --cost-caps needs a reading made with --frame-costs (reading.json frame_costs)")
    if reach_extend and (control or not rd.doc.get("reach")):
        raise SystemExit("--reach-extend needs a reading made with --reach, and no --control")
    if refine_facts and not (window_search and window_strict and reach_extend):
        raise SystemExit("--refine-facts needs --window-search --window-strict and --reach-extend (module docstring)")
    tag = f"{name}-seed{seed}" + ("-control" if control else "") + ("-locals" if slot_locals else "") + \
        ("-serve" if serve else "") + (f"-{search}" if search != "lama-first" else "") + \
        ("-caps" if cost_caps else "") + ("-reachx" if reach_extend else "") + \
        ("-uar" if unobserved_actor_rooms else "") + ("-stones" if approach_stones else "") + \
        ("-window" if window_search else "") + ("-strict" if window_strict else "") + ("-cache" if window_cache else "") + \
        (f"-ahead{window_fork_each}" if window_fork_each else "") + ("-lite" if window_lite else "") + \
        ("-split" if window_split_wait else "") + ("-refine" if refine_facts else "") + \
        (f"-{starter.name}" if starter is not None else "")
    run_dir = out_root / tag
    run_dir.mkdir(parents=True, exist_ok=True)
    goal = rd.task.goal
    end = json.loads(END_PART1.read_text())
    out = {"seed": seed, "reading": str(path), "name": name, "control": control,
           "goal": {k: sorted(v, key=str) for k, v in goal.items()},
           "goal_is_part1_end": goal == {k: {v} for k, v in end["goal_all"].items()} and not end["goal_not"],
           "task": {"actions": len(rd.task.actions), "facts": len(rd.task.facts), "records": rd.doc.get("n_records"),
                    "folded": rd.doc.get("folded"), "packed_sha1": sha(rd.packed), "packed_bytes": rd.packed.stat().st_size,
                    "lines_sha1": sha(rd.lines), "live_locals": rd.doc.get("live_locals") is not None},
           "control_counts": rd.doc.get("control_counts"),
           "limits": {"replans": replan_limit, "seconds": time_limit, "plan_time": plan_time, "plan_memory": plan_memory,
                      "input": replay.INPUT_LIMIT, "wait": replay.WAIT_LIMIT, "retries": replay.RETRIES,
                      "retry_wait": replay.RETRY_WAIT},
           "sources": {n: sha(HERE / n) for n in SOURCES}, "taskcore_binary": sha(plan_fd.TASKCORE)}
    if serve:
        out["taskcore_serve"] = True
    if search != "lama-first" or cost_caps or frame_costs is not None:
        out.update(search=search, cost_caps=cost_caps, frame_costs=frame_costs)
    if rd.doc.get("reach") or rd.doc.get("switches"):
        out["reader"] = {k: rd.doc.get(k) for k in ("switches", "reach", "split_limit", "elsewhere", "reach_also")}
        out["reader"]["live_locals"] = rd.doc.get("live_locals") is not None
    if unobserved_actor_rooms:
        out["unobserved_actor_rooms"] = True
    if approach_stones:
        out["approach_stones"] = True
    wcfg = None
    if window_search:
        import window as WS
        if window_lite and not window_strict:
            raise SystemExit("--window-lite is a sub-switch of --window-strict")
        if window_split_wait and not window_strict:
            raise SystemExit("--window-split-wait is a sub-switch of --window-strict")
        if window_split_wait and "wait_pass_cost" not in str(rd.doc.get("switches") or ""):
            # window-search.md 9.9: under the split an event at a wait that passes at once runs 0 frames after the
            # yield, and only the reader switch wait_pass_cost's costs are lower bounds there (Lemma 4's chain)
            raise SystemExit(f"{path}: --window-split-wait needs a reading made with the reader switch wait_pass_cost")
        wcfg = WS.WindowConfig(workers=workers, delta_lo=WS.delta_lo_of(frame_costs), strict=window_strict,
                               cache=window_cache, fork_each=window_fork_each, lite=window_lite,
                               split_wait=window_split_wait)
        out["window_search"] = {k: v for k, v in vars(wcfg).items() if k != "split_wait" or v}  # (9.9: on only)
        out["sources"]["window.py"] = sha(HERE / "window.py")
        if window_strict:  # window-search.md section 9: B1's matcher and explainer
            for n_ in ("strict.py", "trace_explain.py", "trace_check.py"):
                out["sources"][n_] = sha(HERE / n_)
            out["window_strict"] = {"scummvm": str(paths.SCUMMVM_BIN), "scummvm_sha1": sha(paths.SCUMMVM_BIN)}
    elif window_strict or window_cache:
        raise SystemExit("--window-strict and --window-cache are sub-switches of --window-search")
    if reach_extend:
        out["reach_extend"] = {"threads": threads, "reader_scope": str(READER_SCOPE),
                               "reader_scope_sha1": sha(READER_SCOPE)}
    if refine_facts:
        out["refine_facts"] = {"threads": threads, "reader_scope": str(READER_SCOPE),
                               "reader_scope_sha1": sha(READER_SCOPE), "fact_vars": rd.doc.get("fact_vars") or []}
    if slot_locals:
        out["slot_locals"] = {"reread": reread, "threads": threads, "lists": sum(map(len, rd.lists.values())),
                              "scummvm": str(paths.SCUMMVM_BIN), "scummvm_sha1": sha(paths.SCUMMVM_BIN),
                              "reader_scope": str(READER_SCOPE) if reread else None,
                              "reader_scope_sha1": sha(READER_SCOPE) if reread else None}
    out["load_s"] = round(time.monotonic() - t0, 2)
    if starter is None:
        say(f"{tag}: booting to the first idle stop")
        ctl, branch, row = boot(seed, run_dir / "run", dump_locals=slot_locals or window_strict)
    else:  # another start (``starter``: an instrument's boot, its config for verify and its inputs before the start)
        say(f"{tag}: booting to {starter.name}")
        ctl, branch, row = starter.boot(seed, run_dir / "run", dump_locals=window_strict) if window_strict else \
            starter.boot(seed, run_dir / "run")
        out["starter"] = starter.describe()
    h = None
    try:
        out["start"] = {"stop": row["stop"], "dump": row["dump"], "trace": row["trace"], "boot_s": row["boot_s"]}
        say(f"{tag}: start at frame {row['stop']['frame']}; the start check")
        chk = out["start"]["check"] = start_check(row, rd, run_dir / "start-check", serve, unobserved_actor_rooms,
                                                  START_ROWS if starter is None else starter.rows)
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
        starts: list = []  # the reach extension's start files, which F10's reread reads with too
        facts: list = list(rd.doc.get("fact_vars") or [])  # E007's loop: the run's fact variables so far
        h = replay.Harness(rd.b, rd.task, rd.task, branch, goal, replan_limit=replan_limit, time_limit=time_limit,
                           plan_time=plan_time, progress=say, planner="taskcore", packed=rd.packed,
                           workdir=run_dir / "plans", memory_limit=plan_memory, on_plan=snapshot,
                           slot_locals=slot_locals, lists=rd.lists,
                           reread=rereader(rd, run_dir, control, threads, starts, facts) if reread else None,
                           taskcore_serve=serve, search=search, cost_caps=cost_caps,
                           cost_dmax=(frame_costs or {}).get("dmax"),
                           reach_extend=reach_extender(rd, run_dir, threads, starts, facts) if reach_extend else None,
                           unobserved_actor_rooms=unobserved_actor_rooms,
                           stone_fork=ctl.fork if approach_stones else None,
                           window_search=wcfg, fork=ctl.fork if window_search else None,
                           refine=refiner(rd, run_dir, threads, starts, facts) if refine_facts else None)
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
        if starter is None:
            ver = replay.verify(lambda o, i: config(seed, o, i), res["inputs"], checkpoints, sorted(goal),
                                run_dir / "verify")
        else:  # the inputs before the start, then the run's, from boot
            ver = replay.verify(lambda o, i: starter.config(seed, o, i), starter.prefix + res["inputs"], checkpoints,
                                sorted(goal), run_dir / "verify")
        ver["goal"] = all(ver.get("final", {}).get(k) in v for k, v in goal.items())
    except Exception as e:  # noqa: BLE001 (the run's record is written either way)
        ver = {"error": f"{type(e).__name__}: {e}", "goal": False, "differ": [], "inputs": None, "checkpoints": None}
    ver["seconds"] = round(time.monotonic() - t, 1)
    out["verify"] = ver
    out["seconds"] = round(time.monotonic() - t0, 1)
    out["game_seconds"] = releases.get(S.RELEASE).seconds(res["ticks"])
    if "qf" in res:  # the window search's record: the run clock in quarter-frames
        out["game_qf"] = res["qf"]
        out["game_qf_seconds"] = res["qf"] / releases.get(S.RELEASE).timer_hz
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
             f"({'the Part I end' if o['goal_is_part1_end'] else 'not the Part I end'})"
             f"{'; read with --live-locals' if o['task'].get('live_locals') else ''}"]
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
    search = o.get("search", "lama-first")
    if "search" in o:
        fc = o.get("frame_costs") or {}
        lines.append(f"search {search}; cost caps {o.get('cost_caps')}; frame costs "
                     + (f"dmax {fc.get('dmax')}, charinc {fc.get('charinc')}" if fc else "off (every action 1)"))
    if o.get("reader"):
        lines.append(f"reader: {o['reader']}")
    if o.get("window_search"):
        lines += window_lines(o)
    for p in r["plans"]:
        lines.append(f"  plan {p['plan']} at frame {p['frame']}: {p.get('status')}, {len(p.get('steps', []))} steps, "
                     f"taskcore {p.get('write_s', 0):.2f} s, {search} {p.get('seconds', 0):.2f} s, "
                     f"{p.get('operators') or 0:,} operators, {p['bans']} bans, landmarks {p.get('landmarks')}"
                     + (f", predicted {sum(p['costs'])} frames (FD plan cost {p.get('plan_cost')})"
                        if "search" in o and p.get("costs") else "")
                     + (f"; {p['error']}" if p.get("error") else "")
                     + (f"; {p['reach_misses']} values not reached {p['reach_values'][:6]}" if p.get("reach_misses")
                        else "") + f": {p.get('steps', [])[:12]}")
        if p.get("start_counts"):
            lines.append(f"    start: {p['start_counts']}" + (f"; live slots {p['slot_locals']}" if p.get("slot_locals")
                                                              else ""))
    if o.get("slot_locals"):
        sl = o["slot_locals"]
        lines.append(f"  F10, the slots' dumped locals: {sl['lists']:,} built lists; read again {sl['reread']} "
                     f"({sl['threads']} threads); ScummVM {sl['scummvm']} (sha1 {sl['scummvm_sha1']})")
    for x in r.get("rereads", []):
        lines.append(f"  read again at plan {x['plan']}, frame {x['frame']}: {x['slots']} in {x['seconds']} s, "
                     f"{x['actions']:,} actions; before it {x.get('counts_before')}; still unmatched "
                     f"{x['still_unmatched']}")
    if o.get("reach_extend"):
        xs = r.get("extensions", [])
        lines.append(f"  reach extensions: {len(xs)}, wall {sum(x['seconds'] for x in xs):.1f} s, CPU "
                     f"{sum(x['cpu_s'] for x in xs):.1f} s")
    for x in r.get("extensions", []):
        lines.append(f"  reach extension at plan {x['plan']}, frame {x['frame']}: {x['n_values']} values not reached "
                     f"{x['values'][:8]}; {x['actions_before']:,} -> {x['actions']:,} actions in {x['seconds']} s "
                     f"({x['cpu_s']} CPU s)")
    if o.get("refine_facts"):
        ts = r.get("refinements", [])
        adds = [x for x in ts if x.get("added")]
        lines.append(f"  refinement loop: {len(ts)} turns, {len(adds)} with variables added "
                     f"{[n for x in adds for n in x['added']]}; reader wall {sum(x.get('seconds', 0) for x in adds):.1f} "
                     f"s, CPU {sum(x.get('cpu_s', 0) for x in adds):.1f} s")
        ops = {p_["plan"]: p_.get("operators") for p_ in r["plans"]}
        for x in ts:
            head = f"  refinement turn at plan {x['plan']}, frame {x['frame']} ({x['what']}): class {x['class']}; "
            if x.get("added"):
                lines.append(head + f"var {x['added']} added (engine values {x.get('engine_values')}); records "
                             f"{x.get('records_before'):,} -> {x.get('records') or 0:,}, actions "
                             f"{x.get('actions_before'):,} -> {x.get('actions'):,}; operators {x.get('operators_before')} "
                             f"(plan {x['plan']}) -> {ops.get(x['plan'] + 1)} (plan {x['plan'] + 1}); domains "
                             f"{x.get('facts')}; reader {x.get('reader')}, {x.get('seconds')} s wall, {x.get('cpu_s')} "
                             f"CPU s" + (f"; {x['why_not']}" if x.get("why_not") else ""))
            else:
                lines.append(head + f"no variable added: {x.get('why_not')}")
            lines.append(f"    cause: {x['cause']}")
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


def window_lines(o: dict) -> list:
    """The window search's lines (module docstring, "The window search"): the clock, the windows, the replans' causes and
    the search's CPU."""
    r = o["run"]
    ws, sm = r.get("windows", []), r.get("window_summary", {})
    q = o.get("game_qf")
    out = [f"window search {o['window_search']}: run clock {q} quarter-frames ({o.get('game_qf_seconds', 0):.1f} s) "
           f"from frame {r.get('frame0')} (qf {r.get('qf0')}) to {r.get('frame_end')} (qf {r.get('qf_end')})",
           f"  {sm.get('windows')} windows, {sm.get('children')} children, {sm.get('child_frames')} child frames, "
           f"outcomes {sm.get('outcomes')}; early successes {sm.get('early_successes')}; bound violations "
           f"{sm.get('bound_violations')}; engine CPU s {sm.get('engine_cpu_s')}; Python CPU s {sm.get('python_cpu_s')}"]
    for c in r.get("replan_causes", []):
        out.append(f"  replan after plan {c['plan']} at frame {c['frame']}: {c['cause']}")
    for w in ws:
        b = w.get("best") or {}
        out.append(f"  window {w['plan']}.{w['macro'] + 1} {w['input']}: frames {w['start']}-{w['end']} "
                   f"({w['end_reason']}); {w['forks']} children {w['outcomes']}, {w['child_frames']} child frames, "
                   f"{w['wall_s']} s; b {w['b_frames']} frames ({w['b_qf']} qf, chain {len(w['chain'])}); "
                   + (f"best fork {b['g']}, inputs {b['first']}-{b['issue']}, done {b['end']} (qf {b['q_end']}, "
                      f"{b['q_end'] - w['start_qf']} after the window's start), early {b['early']}, walk "
                      f"{b.get('walk_frames')}" if b else "no success")
                   + (f"; mismatches {w['mismatch_keys']}" if w.get("mismatch_keys") else ""))
    return out


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
    rr = sub.add_parser("read-rust")
    rr.add_argument("rooms")
    rr.add_argument("--seed", type=int, required=True)
    rr.add_argument("--goal", required=True)
    rr.add_argument("--name", required=True)
    rr.add_argument("--out", type=Path, required=True)
    rr.add_argument("--threads", type=int, default=3)
    rr.add_argument("--elsewhere", action="store_true", help="reader-scope's switch --elsewhere")
    rr.add_argument("--live-locals", action="store_true", help="reader-scope's switch --live-locals")
    rr.add_argument("--split-limit", action="store_true", help="reader-scope's switch --split-limit")
    rr.add_argument("--switches", default="", help="reader-scope's --switches (reader switches by name)")
    rr.add_argument("--reach", action="store_true", help="reader-scope's switch --reach (needs --split-limit)")
    rr.add_argument("--frame-costs", action="store_true", help="reader-scope's switch --frame-costs")
    rr.add_argument("--start-rows", type=Path, help="the start's row from this file, not results/start-part1.jsonl")
    rr.add_argument("--fact-vars", default="", help="reader-scope's --fact-vars N,... (E007: variables made facts)")
    c = sub.add_parser("control")
    c.add_argument("reading", type=Path)
    s = sub.add_parser("start")
    s.add_argument("seeds", type=int, nargs="+")
    s.add_argument("--reading", type=Path, required=True)
    s.add_argument("--results", type=Path, default=RESULTS)
    s.add_argument("--serve", action="store_true", help="taskcore as one long-lived process (no reload)")
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
    r.add_argument("--slot-locals", action="store_true", help="F10: the slots' dumped locals at a replan")
    r.add_argument("--reread", action="store_true", help="F10: read slots no list matches again (Rust reader)")
    r.add_argument("--threads", type=int, default=3, help="the Rust reader's threads for --reread")
    r.add_argument("--serve", action="store_true", help="taskcore as one long-lived process: replans without reloading")
    r.add_argument("--search", choices=sorted(plan_fd.SEARCHES) + ["anytime"], default="lama-first",
                   help="taskcore's search (plan_fd.SEARCHES)")
    r.add_argument("--cost-caps", action="store_true", help="the harness's frame cost caps (a --frame-costs reading)")
    r.add_argument("--reach-extend", action="store_true",
                   help="extend a --reach reading at a replan start it does not cover (reader-scope --reach-also)")
    r.add_argument("--unobserved-actor-rooms", action="store_true",
                   help="the actors' rooms (reader switch actor_rooms) unobserved, with the plan's values (replay.py)")
    r.add_argument("--approach-stones", action="store_true",
                   help="the Executor's stepping stones on forks, for an object floor clicks do not bring on screen")
    r.add_argument("--window-search", action="store_true",
                   help="plan step A4: each input tried on every frame of its window on forked children (window.py)")
    r.add_argument("--workers", type=int, default=7, help="the window search's children at a time")
    r.add_argument("--window-strict", action="store_true",
                   help="the window search's sub-switch strict: B1's matcher (window-search.md section 9)")
    r.add_argument("--window-fork-each", type=int, default=0,
                   help="the window search's sub-switch fork_each: N frames forked ahead per command (9.7)")
    r.add_argument("--window-lite", action="store_true",
                   help="the strict window search's sub-switch lite: a child's views from the bridge's lite dump (9.7)")
    r.add_argument("--window-cache", action="store_true",
                   help="the window search's sub-switch cache: a child's dump kept until it runs (section 9.7)")
    r.add_argument("--refine-facts", action="store_true",
                   help="E007's loop: a split on a variable the reading holds no fact of reads the scope again with it "
                        "made a fact (needs --window-strict and --reach-extend)")
    r.add_argument("--window-split-wait", action="store_true",
                   help="the strict window search's sub-switch split_wait: a run that passed a wait read as the "
                        "records' parts (9.9); needs a reading with the reader switch wait_pass_cost")
    lg = sub.add_parser("log")
    lg.add_argument("name")
    lg.add_argument("--results", type=Path, default=RESULTS)
    a = ap.parse_args()
    if a.cmd == "export":
        print(json.dumps(export_python(tuple(int(x) for x in a.rooms.split(",")), a.seed, a.goal, a.name, a.out,
                                       a.workers), indent=1))
    elif a.cmd == "export-pickle":
        print(json.dumps(export_pickle(a.pickle, a.goal, a.name, a.start_name, a.out), indent=1))
    elif a.cmd == "read-rust":
        print(read_rust(tuple(int(x) for x in a.rooms.split(",")), a.seed, a.goal, a.name, a.out, a.threads,
                        elsewhere=a.elsewhere, live_locals=a.live_locals, split_limit=a.split_limit,
                        switches=a.switches, reach=a.reach, frame_costs=a.frame_costs, start_rows=a.start_rows,
                        fact_vars=[int(x) for x in a.fact_vars.split(",") if x.strip()] or None))
    elif a.cmd == "control":
        out = make_control(a.reading)
        print(out, json.dumps(json.loads(out.read_text()).get("control_counts")))
    elif a.cmd == "start":
        cmd_start(a.seeds, a.reading, a.results, serve=a.serve)
    elif a.cmd == "run":
        cmd_run(a.seed, a.reading, a.control, a.replan_limit, a.time_limit, a.plan_time, a.test_seeds,
                results=a.results, plan_memory=a.plan_memory, deadline=a.deadline, slot_locals=a.slot_locals,
                reread=a.reread, threads=a.threads, serve=a.serve, search=a.search, cost_caps=a.cost_caps,
                reach_extend=a.reach_extend, unobserved_actor_rooms=a.unobserved_actor_rooms,
                approach_stones=a.approach_stones, window_search=a.window_search, workers=a.workers,
                window_strict=a.window_strict, window_cache=a.window_cache, window_fork_each=a.window_fork_each,
                window_lite=a.window_lite, window_split_wait=a.window_split_wait, refine_facts=a.refine_facts)
    else:
        cmd_log(a.name, a.results)


if __name__ == "__main__":
    main()
