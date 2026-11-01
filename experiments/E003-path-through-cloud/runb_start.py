"""E003: a start from run B's recorded inputs, for the Part I runner (replay_part1.py ``cmd_run``'s ``starter``).

usage (from this folder, with ../../.venv/bin/python, under nice):
       runb_start.py row --seed N --move S1#7 --out ROWS.jsonl
                     (run B's inputs on seed N to the move's pre frame: the start's row, start-part1.jsonl's format)
       replay_part1.py read-rust ROOMS --seed N ... --start-rows ROWS.jsonl   (the reading from that start)
       runb_start.py run --seed N --move S1#7 --rows ROWS.jsonl --reading DIR/reading.json [replay_part1 run's switches]

An instrument, as runb_qf.py and replay_kitchen.py: one engine with run B's settings (replay_kitchen.cfg_of: DOS CD,
talkspeed 255, segment start at frame 46, var 19 free) and ``--seed`` (never the Any% seed), run B's recorded inputs
(groundtruth.MOVIE) up to the move's pre frame in results/groundtruth-part1.jsonl, run to that frame. The run then
plays from that state with the Part I runner's harness and switches, and verify plays run B's inputs up to that frame
and then the run's from boot in a fresh engine with the same settings. It names a move of run B; replay_part1.py names
none. The run's file ends in ``-runb-<move>``.
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
import replay_part1 as P  # noqa: E402
import runb_reference as RB  # noqa: E402
from facts import facts_of  # noqa: E402

GT = HERE / "results" / "groundtruth-part1.jsonl"


def pre_frame(move: str) -> int:
    for ln in GT.read_text().splitlines():
        o = json.loads(ln)
        if o["name"].split(" ")[0] == move:
            return int(o["pre_frame"])
    raise SystemExit(f"no move {move} in {GT}")


class Starter:
    """cmd_run's ``starter``: run B's inputs up to ``frame`` on ``seed``."""

    def __init__(self, move: str, rows: Path):
        self.move, self.rows = move, Path(rows)
        self.frame = pre_frame(move)
        self.name = f"runb-{move.replace('#', '')}"
        self.prefix = [e for e in replay_kitchen.prefix_events() if e["frame"] <= self.frame]

    @staticmethod
    def config(seed: int, out_dir: Path, inputs: Path):
        return replay_kitchen.cfg_of(out_dir, inputs, seed=seed)

    def boot(self, seed: int, out_dir: Path) -> tuple:
        if seed == RB.ANY_SEED:
            raise SystemExit("the Any% seed is not used (plan.md)")
        t = time.monotonic()
        out_dir.mkdir(parents=True, exist_ok=True)
        ctl, b = replay.boot(lambda o, i: self.config(seed, o, i), self.prefix, self.frame, out_dir)
        st, s = b.status, b.state()
        row = {"seed": seed, "frame": self.frame, "move": self.move,
               "stop": {k: st.get(k) for k in ("stop", "frame", "tick", "room", "idle", "rng", "up", "cs", "ego",
                                                 "qf", "v19")},
               "dump": {k: s.get(k) for k in ("frame", "tick", "room", "ego", "ego_pos", "seed", "boot_param")},
               "facts": facts_of(s), "trace": None,
               "checks": [why for ok, why in ((st.get("frame") == self.frame, f"stopped at {st.get('frame')}"),)
                          if not ok],
               "boot_s": round(time.monotonic() - t, 2)}
        return ctl, b, row

    def describe(self) -> dict:
        return {"instrument": "runb_start.py", "move": self.move, "frame": self.frame, "rows": str(self.rows),
                "prefix_inputs": sum(1 for e in self.prefix if replay._is_input(e))}


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("row")
    r.add_argument("--seed", type=int, required=True)
    r.add_argument("--move", required=True)
    r.add_argument("--out", type=Path, required=True)
    r.add_argument("--work", type=Path, default=replay_kitchen.OUT / "runb-start")
    u = sub.add_parser("run")
    u.add_argument("--seed", type=int, required=True)
    u.add_argument("--move", required=True)
    u.add_argument("--rows", type=Path, required=True)
    u.add_argument("--reading", type=Path, required=True)
    u.add_argument("--replan-limit", type=int, default=30)
    u.add_argument("--time-limit", type=float, default=86400)
    u.add_argument("--plan-time", type=int, default=600)
    u.add_argument("--plan-memory", default="3G")
    u.add_argument("--results", type=Path, default=P.RESULTS)
    u.add_argument("--serve", action="store_true")
    u.add_argument("--search", default="lama-first")
    u.add_argument("--cost-caps", action="store_true")
    u.add_argument("--unobserved-actor-rooms", action="store_true")
    u.add_argument("--approach-stones", action="store_true")
    u.add_argument("--reach-extend", action="store_true")
    u.add_argument("--window-search", action="store_true")
    u.add_argument("--workers", type=int, default=7)
    a = ap.parse_args()
    if a.cmd == "row":
        st = Starter(a.move, a.out)
        ctl, b, row = st.boot(a.seed, a.work / f"seed{a.seed}")
        ctl.close()
        if row["checks"]:
            raise SystemExit(f"the boot replay failed: {row['checks']}")
        a.out.parent.mkdir(parents=True, exist_ok=True)
        a.out.write_text(json.dumps(row) + "\n")
        print(json.dumps({k: row[k] for k in ("seed", "frame", "move", "stop", "dump")}))
    else:
        P.cmd_run(a.seed, a.reading, False, a.replan_limit, a.time_limit, a.plan_time, results=a.results,
                  plan_memory=a.plan_memory, serve=a.serve, search=a.search, cost_caps=a.cost_caps,
                  unobserved_actor_rooms=a.unobserved_actor_rooms, approach_stones=a.approach_stones,
                  reach_extend=a.reach_extend,
                  window_search=a.window_search, workers=a.workers, starter=Starter(a.move, a.rows))


if __name__ == "__main__":
    main()
