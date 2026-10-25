"""E003 step 5 on the kitchen: plan, play the plan in the engine (replay.py), verify from boot.

usage (from this folder, with ../../.venv/bin/python, under nice):
       replay_kitchen.py run all|no211 --reading PICKLE   (one plan: results/replay-kitchen-taskcore-<plan>.json)
       replay_kitchen.py trace all|no211                   (what the engine did, added to the json file)
       replay_kitchen.py noban all|no211 --reading PICKLE  (the planner at the run's end without its bans, likewise)
       replay_kitchen.py log                               (results/replay-kitchen-taskcore.log from the json files)
       each with --planner taskcore|python (default taskcore) and --search lama-first|anytime (taskcore only)

**The planner.** taskcore (the default): the reading's records are exported once to a packed file
(export_records.py) and every plan goes through taskcore from it (replay.py's module docstring); results
``replay-kitchen-taskcore-<plan>.json`` (``-taskcore-anytime-`` with anytime LAMA) and their log. python: the
Python task and planner as before (task.py's write_sas on test_reader's pruned task, plan_fd.plan); results
``replay-kitchen-<plan>.json`` and ``replay-kitchen.log``. The taskcore log ends with both side by side.

An instrument, like test_reader.py's demo, whose task it reuses: it names MI1's numbers (the kitchen's rooms 28 and
41, the meat 566, run B's move S1#7), and reads run B's recorded inputs (groundtruth.py), which the system never
sees. replay.py names none of them.

**The start.** Run B's S1#7 pre state: a fresh engine from boot with run B's settings (groundtruth.replay: DOS CD,
seed 836557856, talkspeed 255, segment start at frame 46, var 19 free, the bridge's automatic '.' and Esc off) and run
B's recorded inputs (verify-b1/movie-explicit.jsonl) up to that frame, run to it. Its facts are compared with the
ground truth's S1#7 pre facts.

**The task**: reader.build_task over the reading's records with the goal owner:566 = VAR_EGO's value at the start;
for the python planner then relevance and reader.prune (test_reader.py's ``pruned``), for taskcore the same records
as the packed file (taskcore's own reductions do the rest, per start). "all" is every record; "no211" leaves out local
211's own event at its start point (the cook's timer), so the plan has to provoke the cook. The engine still runs that
timer.

**The budget**: ``--replan-limit`` (default 10) and ``--time-limit`` seconds (default 3 hours) for the kitchen, in
place of Part I's 500 replans and 24 hours.
"""

import argparse
import hashlib
import json
import os
import pickle
import shutil
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import export_records  # noqa: E402
import groundtruth  # noqa: E402
import plan_fd  # noqa: E402
import reader as R  # noqa: E402
import relevance  # noqa: E402
import replay  # noqa: E402
import task as T  # noqa: E402
from facts import diff, facts_of  # noqa: E402
from speedrun import paths, releases  # noqa: E402
from speedrun.control import Controller  # noqa: E402
from speedrun.engine import EngineConfig  # noqa: E402

ROOMS = (28, 41)
MOVE = "S1#7"
MEAT = 566
TIMER_EVENT = "event(local:211@33)"  # the cook's own timer at its start point (test_reader's "no 211")
OUT = paths.OUT_DIR / "E003-replay"
LOG = HERE / "results" / "replay-kitchen.log"
SOURCES = ("replay.py", "replay_kitchen.py", "reader.py", "model.py", "task.py", "relevance.py", "plan_fd.py",
           "domains.py", "facts.py")
TASKCORE_SOURCES = ("export_records.py",) + tuple(f"taskcore/src/{n}" for n in
                                                  ("main.rs", "model.rs", "packed.rs", "bans.rs", "fold.rs",
                                                   "reduce.rs", "sas.rs"))


def sha(name: str) -> str:
    return hashlib.sha1((HERE / name).read_bytes()).hexdigest()[:12]


def tag_of(planner: str = "python", search: str = "lama-first") -> str:
    """The results' tag: "" for the python planner, "taskcore" or "taskcore-anytime"."""
    if planner == "python":
        return ""
    return "taskcore" if search == "lama-first" else f"taskcore-{search}"


def result_path(variant: str, tag: str = "") -> Path:
    return HERE / "results" / f"replay-kitchen-{tag + '-' if tag else ''}{variant}.json"


def log_path(tag: str = "") -> Path:
    return LOG if not tag else HERE / "results" / f"replay-kitchen-{tag}.log"


def run_dir_of(variant: str, tag: str = "") -> Path:
    return OUT / f"kitchen-{tag + '-' if tag else ''}{variant}"


def cfg_of(out_dir: Path, inputs: Path) -> EngineConfig:
    """Run B's settings (groundtruth.replay) with ``inputs`` as the raw input file."""
    rel = releases.get("dos-cd")
    return EngineConfig(out_dir=out_dir, game_path=releases.game_dir(rel), release="dos-cd", timer_next="any",
                        start_frame=46, max_ticks=10**7, seed=groundtruth.SEED, raw_inputs=inputs,
                        skip_text=False, skip_cutscenes=False, game_settings={"talkspeed": "255"})


def prefix_events() -> list:
    return [json.loads(x) for x in groundtruth.MOVIE.read_text().splitlines() if x.strip()]


def say(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr, flush=True)


def run(variant: str, reading: Path, replan_limit: int, time_limit: float, write: bool = True,
        out_dir: Path | None = None, plan_time: int = replay.PLAN_TIME, planner: str = "taskcore",
        search: str = "lama-first") -> dict:
    """One plan in the engine and its check from boot; ``write``: the result file (a test passes False and its own
    ``out_dir`` for the engines' files)."""
    t0 = time.monotonic()
    tag = tag_of(planner, search)
    row = next(r for r in groundtruth.load() if r["name"].startswith(MOVE + " "))
    start, frame = row["pre"], row["pre_frame"]
    goal = {f"owner:{MEAT}": {start["var:1"]}}
    out = {"variant": variant, "move": row["name"], "start_frame": frame,
           "goal": {k: sorted(v) for k, v in goal.items()},
           "sources": {n: sha(n) for n in SOURCES + (TASKCORE_SOURCES if planner == "taskcore" else ())},
           "planner": planner, "search": search if planner == "taskcore" else "lama-first",
           "reading": str(reading), "replan_limit": replan_limit,
           "time_limit": time_limit, "limits": {"input": replay.INPUT_LIMIT, "wait": replay.WAIT_LIMIT,
                                                "retries": replay.RETRIES, "retry_wait": replay.RETRY_WAIT,
                                                "plan_time": plan_time}}
    rsrc = Path(str(reading) + ".sources")
    out["reading_sources"] = rsrc.read_text().strip() if rsrc.exists() else None

    say(f"loading the reading {reading}")
    b, recs, secs = pickle.loads(reading.read_bytes())
    kept = [r for r in recs if not (variant == "no211" and r.label == TIMER_EVENT)]
    out["records"] = {"all": len(recs), "kept": len(kept)}
    say("building the task")
    t = time.monotonic()
    full = R.build_task(b, kept, goal)
    run_dir = out_dir or run_dir_of(variant, tag)
    packed = None
    if planner == "python":
        rel = relevance.relevant(full)
        small = R.prune(full, rel)
        small.folded = full.folded
        out["task"] = {"actions": len(full.actions), "facts": len(full.facts), "kept actions": len(small.actions),
                       "kept facts": len(small.facts), "folded": full.folded, "seconds": time.monotonic() - t}
    else:
        small = full
        out["task"] = {"actions": len(full.actions), "facts": len(full.facts), "folded": full.folded,
                       "seconds": time.monotonic() - t}
        say("exporting the records to the packed file")
        t = time.monotonic()
        run_dir.mkdir(parents=True, exist_ok=True)
        shutil.rmtree(run_dir / "plans", ignore_errors=True)  # an earlier run's plan files
        packed = run_dir / "kitchen.tc"
        out["export"] = export_records.export_records(packed, b, kept, goal, starts={})
        out["export"]["seconds"] = time.monotonic() - t
        out["export"]["sha1"] = hashlib.sha1(packed.read_bytes()).hexdigest()[:12]
        out["taskcore_binary"] = hashlib.sha1(plan_fd.TASKCORE.read_bytes()).hexdigest()[:12]
        names = [a.name for a in full.actions]
        assert len(names) == out["export"]["actions"]  # the packed file's actions are build_task's, in its order
    del recs
    say(f"booting to {MOVE} pre (frame {frame})")
    prefix = [e for e in prefix_events() if e["frame"] <= frame]
    ctl, branch = replay.boot(cfg_of, prefix, frame, run_dir / "run")
    try:
        d = diff(start, facts_of(branch.state()))
        out["start_check"] = {"facts": len(start), "differ": len(d), "differences": sorted(d.items())[:40]}
        say(f"start check: {len(d)} facts differ from the ground truth's {MOVE} pre")
        h = replay.Harness(b, full, small, branch, goal, replan_limit=replan_limit, time_limit=time_limit,
                           plan_time=plan_time, progress=say, planner=planner, packed=packed, search=search,
                           workdir=run_dir / "plans")
        res = h.run()
        checkpoints = h.checkpoints
    finally:
        ctl.close()
    out["run"] = res
    out["ban_task_sizes"] = [p.get("write_s") for p in res["plans"]]
    say(f"stop: {res['stop']}; verifying from boot ({len(checkpoints)} checkpoints)")
    t = time.monotonic()
    ver = replay.verify(cfg_of, prefix + res["inputs"], checkpoints, sorted(goal), run_dir / "verify")
    ver["seconds"] = time.monotonic() - t
    ver["goal"] = all(ver.get("final", {}).get(k) in v for k, v in goal.items())
    out["verify"] = ver
    out["seconds"] = time.monotonic() - t0
    out["game_seconds"] = releases.get("dos-cd").seconds(res["ticks"])
    if write:
        result_path(variant, tag).write_text(json.dumps(out, indent=1, default=str) + "\n")
        say(f"wrote {result_path(variant, tag)}")
    return out


# --- the trace --------------------------------------------------------------------------------------------------------

TRACE_KEYS = ("room", "state:316", "script:2", "local:203", "local:211", "local:212", "local:214", "local:215",
              "local:216", "local:217", "local:218", "owner:566")  # the door, the sentence script, the cook's scripts
COOK = 6  # the cook's actor (local-216.txt [000B])


def trace(variant: str, tag: str = "") -> list:
    """The run's recorded inputs replayed from boot once more (verify's movie), with every frame where the door, the
    sentence script, the cook's scripts, the room or the meat's owner change: what the engine did, for the log."""
    o = json.loads(result_path(variant, tag).read_text())
    run_dir = run_dir_of(variant, tag)
    lines = []
    with Controller(cfg_of(run_dir / "trace", run_dir / "verify" / "movie.jsonl"), timeout_s=600) as c:
        b = c.root
        start, end = o["start_frame"], o["run"]["frame_end"]
        b.run(frame=start)
        clicks = {e["frame"]: e for e in o["run"]["inputs"]}
        prev = None
        for f in range(start + 1, end + 1):
            b.run(frame=f)
            s = b.state()
            fa = facts_of(s)
            cur = tuple(fa.get(k, 0) for k in TRACE_KEYS)
            if cur != prev or f in clicks:
                cook = next((a for a in s["actors"] if a["id"] == COOK), None)
                ch = {k: v for k, v, p in zip(TRACE_KEYS, cur, prev or (None,) * len(cur)) if v != p}
                where = sorted(k[3:] for k in fa if k.startswith("at:") and k[3:].rsplit(":", 1)[0] in TRACE_KEYS)
                lines.append(f"{f}: {ch}" + (f" input {clicks[f]}" if f in clicks else "") +
                             f"; at {where}; cook {(cook['x'], cook['y']) if cook else 'not in the room'}; ego "
                             f"{b.status.get('ego')}")
                prev = cur
    o["trace"] = lines
    result_path(variant, tag).write_text(json.dumps(o, indent=1, default=str) + "\n")
    return lines


def noban(variant: str, reading: Path, plan_time: int = replay.PLAN_TIME, tag: str = "") -> dict:
    """A check of the stop: the planner from the engine's state at the run's end (the recorded inputs replayed from
    boot), on the task without the run's bans. A plan here means the bans, not the state, left no plan."""
    o = json.loads(result_path(variant, tag).read_text())
    b, recs, _ = pickle.loads(reading.read_bytes())
    goal = {k: set(v) for k, v in o["goal"].items()}
    full = R.build_task(b, [r for r in recs if not (variant == "no211" and r.label == TIMER_EVENT)], goal)
    del recs
    run_dir = run_dir_of(variant, tag)
    with Controller(cfg_of(run_dir / "noban", run_dir / "verify" / "movie.jsonl"), timeout_s=600) as c:
        c.root.run(frame=o["run"]["frame_end"])
        obs = facts_of(c.root.state())
    if o.get("planner", "python") == "python":
        small = R.prune(full, relevance.relevant(full))
        start, counts = R.replan_start(b, obs, {}, small)
        res = plan_fd.plan(T.write_sas(small), start, time_limit=plan_time)
    else:
        start, counts = R.replan_start(b, obs, {}, full)
        res = plan_fd.plan_taskcore(run_dir / "kitchen.tc", full.facts, goal, [[a] for a in full.actions], start,
                                    time_limit=plan_time, search=o.get("search", "lama-first"),
                                    workdir=run_dir / "noban" / "plan")
    o["noban"] = {"frame": o["run"]["frame_end"], "status": res.status, "seconds": res.seconds,
                  "steps": [st.name for st in res.steps], "start_counts": dict(counts)}
    result_path(variant, tag).write_text(json.dumps(o, indent=1, default=str) + "\n")
    return o["noban"]


# --- the log ----------------------------------------------------------------------------------------------------------

def _mm(ms: list, n: int = 8) -> str:
    return ", ".join(f"{m['key']} predicted {list(m['predicted'])} observed {m['observed']}" for m in ms[:n]) + \
        (f" (+{len(ms) - n} more)" if len(ms) > n else "")


def _task_line(o: dict) -> str:
    t = o["task"]
    if o.get("planner", "python") == "python":
        return (f"records {o['records']['kept']} of {o['records']['all']}; task {t['actions']} actions, "
                f"{t['facts']} facts; relevance keeps {t['kept actions']} actions, {t['kept facts']} facts "
                f"({t['seconds']:.0f} s)")
    ex = o["export"]
    return (f"records {o['records']['kept']} of {o['records']['all']}; task {t['actions']} actions, {t['facts']} facts "
            f"({t['seconds']:.0f} s); packed file (export_records.py) {ex['bytes']:,} bytes, sha1 {ex['sha1']}, "
            f"{ex['seconds']:.0f} s; taskcore binary sha1 {o['taskcore_binary']}; planner taskcore, "
            f"{o['search']}")


def _plan_line(p: dict) -> str:
    if p.get("planner") != "taskcore":
        return (f"  plan {p['plan']} at frame {p['frame']}: {p.get('status')}, {len(p.get('steps', []))} steps, "
                f"lama-first {p.get('seconds', 0):.0f} s, task written in {p['write_s']:.0f} s, "
                f"{p['bans']} bans; start {p.get('start_counts')}" + (f"; misses {p['misses']}" if p.get('misses') else ""))
    search = "lama-first" if p.get("search", "lama-first") == "lama-first" else "anytime LAMA"
    return (f"  plan {p['plan']} at frame {p['frame']}: {p.get('status')}, {len(p.get('steps', []))} steps, taskcore "
            f"{p.get('write_s', 0):.1f} s ({p.get('actions') or 0:,} actions, {p.get('operators') or 0:,} operators), "
            f"{search} {p.get('seconds', 0):.1f} s, read back {p.get('decode_s', 0):.2f} s, {p['bans']} bans; "
            f"start {p.get('start_counts')}" + (f"; misses {p['misses']}" if p.get('misses') else "")
            + (f"; facts: {p['file_facts']} in the file, {p['needed_facts']} needed" if "needed_facts" in p else "")
            + (f"; outside their domains, not needed: {p['input_check_misses']} facts"
               if p.get("input_check_misses") else "")
            + (f"; {p['error']}" if p.get("error") else "")
            + ("; the goal holds in the plan's predicted end state" if p.get("predicted_goal") else
               "; the goal does NOT hold in the plan's predicted end state" if p.get("predicted_goal") is False else ""))


def plan_lines(o: dict) -> list:
    r = o["run"]
    lines = [f"=== plan {o['variant']} ===",
             _task_line(o),
             f"start: engine at {o['move']} pre (frame {o['start_frame']}) from boot with run B's inputs: "
             f"{o['start_check']['differ']} of {o['start_check']['facts']} facts differ from the ground truth"
             + (f": {o['start_check']['differences']}" if o['start_check']['differ'] else ""),
             f"goal {o['goal']}; budget {o['replan_limit']} replans, {o['time_limit']:.0f} s; limits {o['limits']}",
             f"result: stop {r['stop']!r}; goal reached {r['goal']}; {len(r['steps'])} steps played; "
             f"{r['replans']} replans; {len(r['bans'])} bans; frames {r['frames']} (from {r['frame0']} to "
             f"{r['frame_end']}), game time {o['game_seconds']:.1f} s; wall {o['seconds']:.0f} s "
             f"(harness {r['seconds']:.0f} s)"]
    for p in r["plans"]:
        lines.append(_plan_line(p))
        if p.get("steps"):
            lines.append(f"    {p['steps']}")
        if any(p.get("folds") or ()):
            lines.append("    steps through a folded action (step: the reduced action): "
                         + "; ".join(f"{i + 1}: {f}" for i, f in enumerate(p["folds"]) if f))
        for ro in p.get("reorders", []):
            lines.append(f"    waits played first: {ro['moved']} before {ro['input']}")
    for ban in r["bans"]:
        lines.append(f"  ban: {ban['label']} at {dict(ban['values'])}")
    if o.get("noban"):
        n = o["noban"]
        lines.append(f"  check of the stop: from the same engine state (frame {n['frame']}) without the bans, "
                     f"{'lama-first' if o.get('search', 'lama-first') == 'lama-first' else 'anytime LAMA'}: "
                     f"{n['status']} in {n['seconds']:.0f} s, {len(n['steps'])} steps {n['steps']}")
    lines.append("steps (plan.index name [kind] input: frames start/end; outcome -> then), then each try (kind: "
                 "frames start/input/end, first idle or menu stop, how the window ended, taken, started):")
    for s in r["steps"]:
        tries = s["tries"] if isinstance(s["tries"], list) else []
        lines.append(f"  {s['plan']}.{s['index'] + 1} {s['name']} [{s['kind']}] {s['input']}: frames {s['start']}/"
                     f"{s['end']}; {s['outcome']} -> {s['then']}; {len(tries)} tries"
                     + (f"; {s['note']}" if s['note'] else ""))
        for t in tries:
            ins = [(e["frame"], e.get("click") or e.get("key")) for e in t.get("inputs", [])]
            lines.append(f"      try {t['try']} ({t['kind']}): frames {t['start']}/{t.get('issue')}/{t.get('end')}, "
                         f"settled {t.get('first_settled')}, {t.get('how')}, taken {t.get('taken')}, started "
                         f"{t.get('started')}" + (f"; inputs {ins}" if ins else "")
                         + (f"; error {t['error']}" if t.get("error") else ""))
        if s.get("checks"):
            lines.append("      replay checks at the last input (cite, condition, side, result): "
                         + "; ".join(f"{c[0]} {c[1]!r} {c[2]} -> {c[4]}" for c in s["checks"]))
        if s["mismatches"]:
            lines.append(f"      mismatches ({len(s['mismatches'])}): {_mm(s['mismatches'])}")
        if s["unplanned"] and "changes" not in s["unplanned"][0]:  # a result file from before the review's fixes
            lines.append(f"      unplanned changes, step start to end ({len(s['unplanned'])}): {_mm(s['unplanned'], 10)}")
        elif s["unplanned"]:
            un = s["unplanned"]
            lines.append(f"      tracked facts changed with no predicted write, last try ({len(un)}; start value, "
                         f"then frame: value): " + "; ".join(
                             f"{u['key']} {u['start']}, " + ", ".join(f"{f}: {v}" for f, v in u["changes"])
                             + (f" (+{u['count'] - len(u['changes'])})" if u["count"] > len(u["changes"]) else "")
                             for u in un[:24]) + (f" (+{len(un) - 24} facts)" if len(un) > 24 else ""))
    v = o["verify"]
    lines.append(f"verify from boot (run B's inputs to frame {o['start_frame']} plus the harness's "
                 f"{len([e for e in r['inputs'] if replay._is_input(e)])} inputs, {v['inputs']} in all): "
                 f"{v['checkpoints']} checkpoints (facts, tick, random state, actor positions), "
                 f"{len(v['differ'])} differ; final {v.get('final')}, goal {v['goal']} ({v['seconds']:.0f} s)")
    for d in v["differ"][:5]:
        lines.append(f"  differs at {d}")
    if o.get("trace"):
        lines.append("what the engine did (the inputs replayed from boot; frames where the room, door 316, the "
                     "sentence script, locals 203 and 211-218 or the meat's owner change; slot positions; the cook's "
                     "x, y):")
        lines += [f"  {t}" for t in o["trace"]]
    return lines


def _planning(o: dict) -> tuple:
    """Per plan: (the task's write, the planner's search, reading the plan back), in seconds."""
    return [(p.get("write_s", 0), p.get("seconds", 0), p.get("decode_s", 0)) for p in o["run"]["plans"]]


def _span(xs) -> str:
    xs = list(xs)
    return f"{min(xs):.1f}-{max(xs):.1f} s" if xs else "-"


# the python planner's file (task.py's write_sas of test_reader's pruned task): its operators, from the format check
# in results/taskcore-kitchen.log (its runs did not record them)
PY_OPERATORS = {"all": 583670, "no211": 583662}


def compare_lines(new: dict, old: dict) -> list:
    """The taskcore run ``new`` and the python planner's ``old`` on one plan, side by side."""
    rn, ro = new["run"], old["run"]
    pn, po = _planning(new), _planning(old)
    vn, vo = new["verify"], old["verify"]

    def row(what, a, b):
        return f"  {what:34} {str(a):>28}   {str(b):>28}"

    def ins(r):
        return len([e for e in r["inputs"] if replay._is_input(e)])
    return [f"plan {new['variant']}:", row("", "python (replay-kitchen.log)", f"taskcore ({new['search']})"),
            row("stop", repr(ro["stop"]), repr(rn["stop"])), row("goal reached", ro["goal"], rn["goal"]),
            row("replans", ro["replans"], rn["replans"]), row("bans", len(ro["bans"]), len(rn["bans"])),
            row("steps played", len(ro["steps"]), len(rn["steps"])), row("harness inputs", ins(ro), ins(rn)),
            row("frames", ro["frames"], rn["frames"]),
            row("game time", f"{old['game_seconds']:.1f} s", f"{new['game_seconds']:.1f} s"),
            row("plan lengths", [len(p.get("steps", [])) for p in ro["plans"]][:12],
                [len(p.get("steps", [])) for p in rn["plans"]][:12]),
            row("operators", ", ".join(f"{x:,}" for x in sorted({p.get("operators") for p in ro["plans"]} - {None}))
                or f"{PY_OPERATORS.get(new['variant'], 0):,}",
                ", ".join(f"{x:,}" for x in sorted({p.get("operators") for p in rn["plans"]} - {None}))),
            row("task written, per plan", _span(w for w, _, _ in po), _span(w for w, _, _ in pn)),
            row("planner, per plan", _span(s for _, s, _ in po), _span(s for _, s, _ in pn)),
            row("read back and paths, per plan", "-", _span(d for _, _, d in pn)),
            row("planning in all", f"{sum(map(sum, po)):.0f} s", f"{sum(map(sum, pn)):.0f} s"),
            row("harness wall", f"{ro['seconds']:.0f} s", f"{rn['seconds']:.0f} s"),
            row("run wall (with the task, boot, verify)", f"{old['seconds']:.0f} s", f"{new['seconds']:.0f} s"),
            row("verify from boot: inputs", vo["inputs"], vn["inputs"]),
            row("verify from boot: checkpoints", vo["checkpoints"], vn["checkpoints"]),
            row("verify from boot: differ", len(vo["differ"]), len(vn["differ"])),
            row("verify from boot: final", vo.get("final"), vn.get("final")),
            f"  the same inputs at the same frames: {ro['inputs'] == rn['inputs']}; the same steps played (name, "
            f"outcome): {[(s['name'], s['outcome']) for s in ro['steps']] == [(s['name'], s['outcome']) for s in rn['steps']]}"
            f"; the same engine trace from boot: {old.get('trace') == new.get('trace')}"]


def write_log(tag: str = "") -> None:
    parts = [o for o in (json.loads(result_path(v, tag).read_text()) for v in ("all", "no211")
                         if result_path(v, tag).exists())]
    if not parts:
        raise SystemExit("no result files")
    title = "E003 step 5: the replay harness (replay.py) on the kitchen"
    if tag:
        title += f", planning through taskcore ({parts[0]['search']})"
    lines = [title, "",
             f"reading: {parts[0]['reading']} (made under {parts[0]['reading_sources']})"]
    lines += [f"sources of the {o['variant']} run (sha1, 12 hex): {o['sources']}" for o in parts] + [""]
    for o in parts:
        lines += plan_lines(o) + [""]
    olds = {o["variant"]: json.loads(result_path(o["variant"]).read_text()) for o in parts
            if tag and result_path(o["variant"]).exists()}
    if olds:
        lines += ["=== the two planners side by side (per plan: the shortest and the longest; python's first write of "
                  "the task is its only one) ==="]
        for o in parts:
            if o["variant"] in olds:
                lines += compare_lines(o, olds[o["variant"]]) + [""]
    log_path(tag).write_text("\n".join(lines))
    print("\n".join(lines))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("what", choices=["run", "trace", "noban", "log"])
    ap.add_argument("variant", nargs="?", choices=["all", "no211"])
    ap.add_argument("--reading", type=Path, default=os.environ.get("READER_PICKLE"))
    ap.add_argument("--replan-limit", type=int, default=10)
    ap.add_argument("--time-limit", type=float, default=3 * 3600)
    ap.add_argument("--planner", choices=["taskcore", "python"], default="taskcore")
    ap.add_argument("--search", choices=["lama-first", "anytime"], default="lama-first",
                    help="taskcore's search: lama-first, or anytime LAMA for the plan time (--plan-time)")
    ap.add_argument("--plan-time", type=int, default=replay.PLAN_TIME, help="seconds per plan (the anytime cap)")
    a = ap.parse_args()
    tag = tag_of(a.planner, a.search)
    if a.what == "run":
        if a.variant is None or a.reading is None:
            raise SystemExit("run needs a variant and --reading (or READER_PICKLE)")
        run(a.variant, Path(a.reading), a.replan_limit, a.time_limit, plan_time=a.plan_time, planner=a.planner,
            search=a.search)
    elif a.what == "noban":
        print(noban(a.variant, Path(a.reading), plan_time=a.plan_time, tag=tag))
    elif a.what == "trace":
        print("\n".join(trace(a.variant, tag)))
    else:
        write_log(tag)


if __name__ == "__main__":
    main()
