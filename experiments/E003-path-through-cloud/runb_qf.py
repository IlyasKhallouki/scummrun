"""E003: run B's recorded inputs on the bridge's run clock: the quarter-frames from boot to the first frame a goal holds,
at the first frame and the first idle stop in each room, and at the frames ``--at``.

usage (from this folder, with ../../.venv/bin/python, under nice):
       runb_qf.py --seed N --goal FACT=V|FACT=var:K[,...] --to F1 [--at F,...] [--out FILE.json]

An instrument, as runb_reference.py (whose settings, goal syntax and seed rule it uses): one engine with run B's
settings and ``--seed``, run B's recorded inputs, run frame by frame from boot to ``F1``. Per frame it reads the stop
status's ``qf`` (every quarter-frame waited since boot, SpeedrunBridge::onWait) and the bridge's idle rule (the status's
``idle``). Seconds are qf over the release's timer rate. The file names no game number.
"""

import argparse
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import replay  # noqa: E402
import replay_kitchen  # noqa: E402
import runb_reference as RB  # noqa: E402
from facts import facts_of  # noqa: E402
from speedrun import releases  # noqa: E402
from speedrun.control import Controller  # noqa: E402


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, required=True)
    ap.add_argument("--goal", required=True)
    ap.add_argument("--to", dest="f1", type=int, required=True)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--at", default="", help="frames whose qf is reported too")
    ap.add_argument("--work", type=Path, default=replay_kitchen.OUT / "runb-qf")
    a = ap.parse_args()
    if a.seed == RB.ANY_SEED:
        raise SystemExit("the Any% seed is not used (plan.md)")
    goal = RB.goal_of(a.goal)
    hz = releases.get("dos-cd").timer_hz
    t0 = time.monotonic()
    events = [e for e in replay_kitchen.prefix_events() if e["frame"] <= a.f1]
    run_dir = a.work / "run"
    run_dir.mkdir(parents=True, exist_ok=True)
    cfg = replay_kitchen.cfg_of(run_dir, replay.write_events(run_dir / "inputs.jsonl", events), seed=a.seed)
    rooms, idle, first, at = {}, {}, None, {}
    want = {int(x) for x in a.at.split(",") if x}
    with Controller(cfg, timeout_s=600) as c:
        b = c.root
        st = b.status
        while True:
            f, q = st["frame"], st.get("qf")
            facts = facts_of(b.state())
            room = facts.get("room")
            if f in want:
                at[str(f)] = q
            rooms.setdefault(room, [f, q])
            if "idle" in st:
                idle.setdefault(room, [f, q])
            if RB.holds(goal, facts):
                first = [f, q]
                break
            if f >= a.f1:
                break
            st = b.run(frame=f + 1)
            if "end" in st:
                break
    out = {"seed": a.seed, "goal": a.goal, "to": a.f1, "timer_hz": hz,
           "goal_first": None if first is None else {"frame": first[0], "qf": first[1],
                                                     "seconds": round(first[1] / hz, 2)},
           "first_frame_in_room": {str(k): {"frame": v[0], "qf": v[1]} for k, v in rooms.items()},
           "first_idle_in_room": {str(k): {"frame": v[0], "qf": v[1]} for k, v in idle.items()},
           "qf_at": at,
           "inputs": sum(1 for e in events if replay._is_input(e) and e["frame"] <= (first or [a.f1])[0]),
           "seconds": round(time.monotonic() - t0, 1)}
    text = json.dumps(out, indent=1)
    print(text)
    if a.out:
        a.out.write_text(text + "\n")


if __name__ == "__main__":
    main()
