"""E003 step 5 on the kitchen: plan with the Python planner, play the plan in the engine (replay.py), verify from boot.

usage (from this folder, with ../../.venv/bin/python, under nice):
       replay_kitchen.py run all|no211 --reading PICKLE   (one plan: results/replay-kitchen-<plan>.json)
       replay_kitchen.py trace all|no211                   (what the engine did, added to the json file)
       replay_kitchen.py noban all|no211 --reading PICKLE  (the planner at the run's end without its bans, likewise)
       replay_kitchen.py log                               (results/replay-kitchen.log from the json files)

An instrument, like test_reader.py's demo, whose task it reuses: it names MI1's numbers (the kitchen's rooms 28 and
41, the meat 566, run B's move S1#7), and reads run B's recorded inputs (groundtruth.py), which the system never
sees. replay.py names none of them.

**The start.** Run B's S1#7 pre state: a fresh engine from boot with run B's settings (groundtruth.replay: DOS CD,
seed 836557856, talkspeed 255, segment start at frame 46, var 19 free, the bridge's automatic '.' and Esc off) and run
B's recorded inputs (verify-b1/movie-explicit.jsonl) up to that frame, run to it. Its facts are compared with the
ground truth's S1#7 pre facts.

**The task** (test_reader.py's ``pruned``): reader.build_task over the reading's records with the goal owner:566 =
VAR_EGO's value at the start, relevance, reader.prune. "all" is every record; "no211" leaves out local 211's own event
at its start point (the cook's timer), so the plan has to provoke the cook. The engine still runs that timer.

**The budget**: ``--replan-limit`` (default 10) and ``--time-limit`` seconds (default 3 hours) for the kitchen, in
place of Part I's 500 replans and 24 hours.
"""

import argparse
import hashlib
import json
import os
import pickle
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

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


def sha(name: str) -> str:
    return hashlib.sha1((HERE / name).read_bytes()).hexdigest()[:12]


def result_path(variant: str) -> Path:
    return HERE / "results" / f"replay-kitchen-{variant}.json"


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
        out_dir: Path | None = None, plan_time: int = replay.PLAN_TIME) -> dict:
    """One plan in the engine and its check from boot; ``write``: the result file (a test passes False and its own
    ``out_dir`` for the engines' files)."""
    t0 = time.monotonic()
    row = next(r for r in groundtruth.load() if r["name"].startswith(MOVE + " "))
    start, frame = row["pre"], row["pre_frame"]
    goal = {f"owner:{MEAT}": {start["var:1"]}}
    out = {"variant": variant, "move": row["name"], "start_frame": frame,
           "goal": {k: sorted(v) for k, v in goal.items()},
           "sources": {n: sha(n) for n in SOURCES}, "reading": str(reading), "replan_limit": replan_limit,
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
    rel = relevance.relevant(full)
    small = R.prune(full, rel)
    small.folded = full.folded
    out["task"] = {"actions": len(full.actions), "facts": len(full.facts), "kept actions": len(small.actions),
                   "kept facts": len(small.facts), "folded": full.folded, "seconds": time.monotonic() - t}
    del recs

    run_dir = out_dir or OUT / f"kitchen-{variant}"
    say(f"booting to {MOVE} pre (frame {frame})")
    prefix = [e for e in prefix_events() if e["frame"] <= frame]
    ctl, branch = replay.boot(cfg_of, prefix, frame, run_dir / "run")
    try:
        d = diff(start, facts_of(branch.state()))
        out["start_check"] = {"facts": len(start), "differ": len(d), "differences": sorted(d.items())[:40]}
        say(f"start check: {len(d)} facts differ from the ground truth's {MOVE} pre")
        h = replay.Harness(b, full, small, branch, goal, replan_limit=replan_limit, time_limit=time_limit,
                           plan_time=plan_time, progress=say)
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
        result_path(variant).write_text(json.dumps(out, indent=1, default=str) + "\n")
        say(f"wrote {result_path(variant)}")
    return out


# --- the trace --------------------------------------------------------------------------------------------------------

TRACE_KEYS = ("room", "state:316", "script:2", "local:203", "local:211", "local:212", "local:214", "local:215",
              "local:216", "local:217", "local:218", "owner:566")  # the door, the sentence script, the cook's scripts
COOK = 6  # the cook's actor (local-216.txt [000B])


def trace(variant: str) -> list:
    """The run's recorded inputs replayed from boot once more (verify's movie), with every frame where the door, the
    sentence script, the cook's scripts, the room or the meat's owner change: what the engine did, for the log."""
    o = json.loads(result_path(variant).read_text())
    run_dir = OUT / f"kitchen-{variant}"
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
    result_path(variant).write_text(json.dumps(o, indent=1, default=str) + "\n")
    return lines


def noban(variant: str, reading: Path, plan_time: int = replay.PLAN_TIME) -> dict:
    """A check of the stop: the planner from the engine's state at the run's end (the recorded inputs replayed from
    boot), on the task without the run's bans. A plan here means the bans, not the state, left no plan."""
    o = json.loads(result_path(variant).read_text())
    b, recs, _ = pickle.loads(reading.read_bytes())
    goal = {k: set(v) for k, v in o["goal"].items()}
    full = R.build_task(b, [r for r in recs if not (variant == "no211" and r.label == TIMER_EVENT)], goal)
    del recs
    small = R.prune(full, relevance.relevant(full))
    run_dir = OUT / f"kitchen-{variant}"
    with Controller(cfg_of(run_dir / "noban", run_dir / "verify" / "movie.jsonl"), timeout_s=600) as c:
        c.root.run(frame=o["run"]["frame_end"])
        obs = facts_of(c.root.state())
    start, counts = R.replan_start(b, obs, {}, small)
    res = plan_fd.plan(T.write_sas(small), start, time_limit=plan_time)
    o["noban"] = {"frame": o["run"]["frame_end"], "status": res.status, "seconds": res.seconds,
                  "steps": [st.name for st in res.steps], "start_counts": dict(counts)}
    result_path(variant).write_text(json.dumps(o, indent=1, default=str) + "\n")
    return o["noban"]


# --- the log ----------------------------------------------------------------------------------------------------------

def _mm(ms: list, n: int = 8) -> str:
    return ", ".join(f"{m['key']} predicted {list(m['predicted'])} observed {m['observed']}" for m in ms[:n]) + \
        (f" (+{len(ms) - n} more)" if len(ms) > n else "")


def plan_lines(o: dict) -> list:
    r = o["run"]
    lines = [f"=== plan {o['variant']} ===",
             f"records {o['records']['kept']} of {o['records']['all']}; task {o['task']['actions']} actions, "
             f"{o['task']['facts']} facts; relevance keeps {o['task']['kept actions']} actions, "
             f"{o['task']['kept facts']} facts ({o['task']['seconds']:.0f} s)",
             f"start: engine at {o['move']} pre (frame {o['start_frame']}) from boot with run B's inputs: "
             f"{o['start_check']['differ']} of {o['start_check']['facts']} facts differ from the ground truth"
             + (f": {o['start_check']['differences']}" if o['start_check']['differ'] else ""),
             f"goal {o['goal']}; budget {o['replan_limit']} replans, {o['time_limit']:.0f} s; limits {o['limits']}",
             f"result: stop {r['stop']!r}; goal reached {r['goal']}; {len(r['steps'])} steps played; "
             f"{r['replans']} replans; {len(r['bans'])} bans; frames {r['frames']} (from {r['frame0']} to "
             f"{r['frame_end']}), game time {o['game_seconds']:.1f} s; wall {o['seconds']:.0f} s "
             f"(harness {r['seconds']:.0f} s)"]
    for p in r["plans"]:
        lines.append(f"  plan {p['plan']} at frame {p['frame']}: {p.get('status')}, {len(p.get('steps', []))} steps, "
                     f"lama-first {p.get('seconds', 0):.0f} s, task written in {p['write_s']:.0f} s, "
                     f"{p['bans']} bans; start {p.get('start_counts')}"
                     + (f"; misses {p['misses']}" if p.get('misses') else ""))
        if p.get("steps"):
            lines.append(f"    {p['steps']}")
    for ban in r["bans"]:
        lines.append(f"  ban: {ban['label']} at {dict(ban['values'])}")
    if o.get("noban"):
        n = o["noban"]
        lines.append(f"  check of the stop: from the same engine state (frame {n['frame']}) without the bans, "
                     f"lama-first: {n['status']} in {n['seconds']:.0f} s, {len(n['steps'])} steps {n['steps']}")
    lines.append("steps (plan.index name [kind] input: frames start/issue/end, first idle or menu stop; "
                 "outcome -> then; tries):")
    for s in r["steps"]:
        lines.append(f"  {s['plan']}.{s['index'] + 1} {s['name']} [{s['kind']}] {s['input']}: frames {s['start']}/"
                     f"{s['issue']}/{s['end']}, settled {s['first_settled']}; {s['outcome']} -> {s['then']}; "
                     f"tries {s['tries']}" + (f"; {s['note']}" if s['note'] else ""))
        if s["mismatches"]:
            lines.append(f"      mismatches ({len(s['mismatches'])}): {_mm(s['mismatches'])}")
        if s["unplanned"]:
            lines.append(f"      unplanned changes to tracked facts ({len(s['unplanned'])}): {_mm(s['unplanned'], 10)}")
        ins = [e for e in s["inputs"] if replay._is_input(e)]
        if ins:
            lines.append(f"      inputs ({len(ins)}): {ins[:12]}" + (" ..." if len(ins) > 12 else ""))
    v = o["verify"]
    lines.append(f"verify from boot (run B's inputs to frame {o['start_frame']} plus the harness's "
                 f"{len([e for e in r['inputs'] if replay._is_input(e)])} inputs, {v['inputs']} in all): "
                 f"{v['checkpoints']} checkpoints, {len(v['differ'])} differ; final {v.get('final')}, goal "
                 f"{v['goal']} ({v['seconds']:.0f} s)")
    for d in v["differ"][:5]:
        lines.append(f"  differs at {d}")
    if o.get("trace"):
        lines.append("what the engine did (the inputs replayed from boot; frames where the room, door 316, the "
                     "sentence script, locals 203 and 211-218 or the meat's owner change; slot positions; the cook's "
                     "x, y):")
        lines += [f"  {t}" for t in o["trace"]]
    return lines


def write_log() -> None:
    parts = [o for o in (json.loads(result_path(v).read_text()) for v in ("all", "no211") if result_path(v).exists())]
    if not parts:
        raise SystemExit("no result files")
    lines = ["E003 step 5: the replay harness (replay.py) on the kitchen", "",
             f"reading: {parts[0]['reading']} (made under {parts[0]['reading_sources']})"]
    lines += [f"sources of the {o['variant']} run (sha1, 12 hex): {o['sources']}" for o in parts] + [""]
    for o in parts:
        lines += plan_lines(o) + [""]
    LOG.write_text("\n".join(lines))
    print("\n".join(lines))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("what", choices=["run", "trace", "noban", "log"])
    ap.add_argument("variant", nargs="?", choices=["all", "no211"])
    ap.add_argument("--reading", type=Path, default=os.environ.get("READER_PICKLE"))
    ap.add_argument("--replan-limit", type=int, default=10)
    ap.add_argument("--time-limit", type=float, default=3 * 3600)
    a = ap.parse_args()
    if a.what == "run":
        if a.variant is None or a.reading is None:
            raise SystemExit("run needs a variant and --reading (or READER_PICKLE)")
        run(a.variant, Path(a.reading), a.replan_limit, a.time_limit)
    elif a.what == "noban":
        print(noban(a.variant, Path(a.reading)))
    elif a.what == "trace":
        print("\n".join(trace(a.variant)))
    else:
        write_log()


if __name__ == "__main__":
    main()
