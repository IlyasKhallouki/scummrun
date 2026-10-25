"""E003 step 5: score a Part I run (replay_part1.py's result file) against E001's key, after the run.

usage (from this folder, with ../../.venv/bin/python, under nice):
       score_part1.py RESULT.json [RESULT.json ...]   (adds "score" to each result file; then replay_part1.py log NAME)

plan.md step 5's report lists "Part I goals the path passes through (of 99)", and plan.md's metric scores Part I by the
key's goals #97-99. The key is E001's, built from the human route, and end_part1.py maps its facts to facts.py keys:
it is for scoring only, so the run never reads it (end_part1.py: nothing the system runs imports it; replay.py and
replay_part1.py do not import this file). This script takes the run's recorded inputs and plays them from boot in a
fresh engine with plan.md's settings (replay_part1.config: no fork, no change to the random state), as verify does, but
one frame at a time from the start (the first idle stop, the run's ``frame0``) to the run's last frame, and checks the
key's 99 Part I goals on every frame:

* **held at the start**: the goals that hold on the start's frame (default values, the boot script's writes); they are
  listed, not counted as passed through;
* **reached in the run**: the goals that do not hold at the start and hold on some later frame, with the first one;
* **the Part I end**: the frame where #97-99 (end_part1.part_end) first all hold, or None;
* **the end**: the facts at the run's last frame against the result's ``verify.final`` (the same inputs from boot, so
  they must agree; a difference is reported).
"""

import argparse
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import end_part1  # noqa: E402
import replay  # noqa: E402
import replay_part1 as P1  # noqa: E402
from facts import facts_of  # noqa: E402
from speedrun.control import Controller  # noqa: E402


def key_goals() -> tuple:
    """The key's 99 Part I goals ({"#n": (fact, op, value)}, end_part1.as_fact) and the Part I end's names (#97-99)."""
    key = json.loads(end_part1.KEY.read_text())["key"]
    goals = {f"#{n}": end_part1.as_fact(e["fact"]) for n, e in enumerate(key, 1) if e["part"] == end_part1.PART}
    return goals, [f"#{e['key_entry']}" for e in end_part1.part_end(key)]


def first_frames(states, goals: dict, frame0: int, together=()) -> tuple:
    """From ``states`` ((frame, facts.py map), in frame order, the first at ``frame0``): (the goals holding at the start,
    {goal: the first later frame it holds} for the others, the first frame all of ``together`` hold, or None)."""
    at_start, reached, both = set(), {}, None
    for frame, facts in states:
        ok = {name for name, (k, op, v) in goals.items() if end_part1.holds(facts, k, op, v)}
        for name in ok - at_start - reached.keys():
            if frame == frame0:
                at_start.add(name)
            else:
                reached[name] = frame
        if both is None and together and set(together) <= ok:
            both = frame
    return at_start, reached, both


def engine_states(seed: int, inputs: list, frame0: int, frame_end: int, out_dir: Path):
    """(frame, facts) for every frame from ``frame0`` to ``frame_end``, from boot with ``inputs`` (module docstring)."""
    cfg = P1.config(seed, out_dir, replay.write_events(out_dir / "movie.jsonl", inputs))
    with Controller(cfg, timeout_s=600) as c:
        b = c.root
        for f in range(frame0, frame_end + 1):
            st = b.run(frame=f)
            if st.get("frame") != f:
                raise RuntimeError(f"the replay stopped at {st} before frame {f}")
            yield f, facts_of(b.state())


def score(path: Path, out_root: Path = P1.OUT) -> dict:
    o = json.loads(Path(path).read_text())
    r = o["run"]
    goals, end = key_goals()
    tag = Path(path).stem.removeprefix("replay-part1-")
    t = time.monotonic()
    last = {}

    def states():
        for f, facts in engine_states(o["seed"], r["inputs"], r["frame0"], r["frame_end"], out_root / f"{tag}-score"):
            last["frame"], last["facts"] = f, facts
            yield f, facts
    at_start, reached, both = first_frames(states(), goals, r["frame0"], end)
    final = (o.get("verify") or {}).get("final") or {}
    sc = {"of": len(goals), "frames": [r["frame0"], r["frame_end"]], "held_at_start": len(at_start),
          "at_start": sorted(at_start, key=lambda n: int(n[1:])), "reached_in_the_run": len(reached),
          "reached": dict(sorted(reached.items(), key=lambda kv: int(kv[0][1:]))),
          "part1_end": both, "part1_end_goals": end,
          "end_differs_from_verify": {k: [v, last["facts"].get(k, 0)] for k, v in final.items()
                                      if last["facts"].get(k, 0) != v},
          "seconds": round(time.monotonic() - t, 1), "key": str(end_part1.KEY.relative_to(end_part1.ROOT)),
          "sources": {n: P1.sha(HERE / n) for n in ("score_part1.py", "end_part1.py")}}
    o["score"] = sc
    Path(path).write_text(json.dumps(o, indent=1, default=str) + "\n")
    return sc


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("results", type=Path, nargs="+")
    a = ap.parse_args()
    for p in a.results:
        sc = score(p)
        print(p.name, json.dumps({k: sc[k] for k in ("held_at_start", "reached_in_the_run", "reached", "part1_end",
                                                      "end_differs_from_verify", "seconds")}))


if __name__ == "__main__":
    main()
