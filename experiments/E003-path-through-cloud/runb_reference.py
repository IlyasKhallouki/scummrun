"""E003: a reference count of frames from run B's recorded inputs: the first frame at which a goal holds.

usage (from this folder, with ../../.venv/bin/python, under nice):
       runb_reference.py --seed N --goal FACT=V|FACT=var:K[,...] --from F0 --to F1 [--out FILE.json]

An instrument, like groundtruth.py and replay_kitchen.py: it reads run B's recorded inputs (groundtruth.MOVIE), which
the system never sees. It boots one engine with run B's settings (replay_kitchen.cfg_of: DOS CD, talkspeed 255, segment
start at frame 46, the bridge's automatic '.' and Esc off) and ``--seed`` in place of run B's own seed, plays the
recorded inputs, runs to frame ``F0`` and then frame by frame to ``F1``, and reports the first frame at which every
fact of ``--goal`` holds (``var:K``: the value of that fact at the same frame). Run B's own seed is refused: the
reference is played on the system's seed, and where the random draws differ the inputs may not do what they did in
run B. Also reported: the frames of the ground truth's moves (results/groundtruth-part1.jsonl, recorded under run B's
own seed) at which the goal first holds at a move's end.

The file names no game number; the goal and the frames are the caller's.
"""

import argparse
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import groundtruth  # noqa: E402
import replay  # noqa: E402
import replay_kitchen  # noqa: E402
from facts import facts_of  # noqa: E402
from speedrun.control import Controller  # noqa: E402

ANY_SEED = 836557856  # plan.md: the Any% seed, never used here


def goal_of(text: str) -> list:
    out = []
    for part in text.split(","):
        k, v = part.split("=")
        out.append((k.strip(), v.strip()))
    return out


def holds(goal: list, facts: dict) -> bool:
    for k, v in goal:
        want = facts.get(v, 0) if v.startswith("var:") else int(v)
        if facts.get(k, 0) != want:
            return False
    return True


def ground_truth(goal: list) -> dict | None:
    """The first ground-truth move at whose end (``done``) the goal holds, with its frames and its pre's."""
    for r in groundtruth.load():
        if holds(goal, r["done"]):
            return {"move": r["name"].split(" ")[0], "pre_frame": r["pre_frame"], "done_frame": r["done_frame"],
                    "held_at_pre": holds(goal, r["pre"])}
    return None


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, required=True)
    ap.add_argument("--goal", required=True)
    ap.add_argument("--from", dest="f0", type=int, required=True)
    ap.add_argument("--to", dest="f1", type=int, required=True)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--work", type=Path, default=replay_kitchen.OUT / "runb-reference")
    a = ap.parse_args()
    if a.seed == ANY_SEED:
        raise SystemExit("the Any% seed is not used (plan.md)")
    goal = goal_of(a.goal)
    t0 = time.monotonic()
    events = replay_kitchen.prefix_events()
    a.work.mkdir(parents=True, exist_ok=True)
    # every recorded input up to F1 in the engine's input file (replay.boot keeps only those up to its stop frame)
    run_dir = a.work / "run"
    run_dir.mkdir(parents=True, exist_ok=True)
    cfg = replay_kitchen.cfg_of(run_dir, replay.write_events(run_dir / "inputs.jsonl",
                                                             [e for e in events if e["frame"] <= a.f1]), seed=a.seed)
    ctl = Controller(cfg, timeout_s=600)
    ctl.start()
    b = ctl.root
    st = b.run(frame=a.f0)
    if st.get("frame") != a.f0:
        ctl.close()
        raise SystemExit(f"the replay stopped at {st} instead of frame {a.f0}")
    first, rooms, out_stop = None, {}, None
    try:
        f = a.f0
        while f <= a.f1:
            facts = facts_of(b.state())
            rooms.setdefault(facts.get("room"), f)
            if holds(goal, facts):
                first = f
                break
            f += 1
            st = b.run(frame=f)
            if st.get("frame") != f:
                out_stop = st
                break
        else:
            out_stop = None
    finally:
        ctl.close()
    out = {"seed": a.seed, "goal": a.goal, "from": a.f0, "to": a.f1, "first_frame": first,
           "first_frame_in_room": rooms, "inputs": sum(1 for e in events if e["frame"] <= (first or a.f1)),
           "ground_truth": ground_truth(goal), "stopped": out_stop, "seconds": round(time.monotonic() - t0, 1)}
    text = json.dumps(out, indent=1)
    print(text)
    if a.out:
        a.out.write_text(text + "\n")


if __name__ == "__main__":
    main()
