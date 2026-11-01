"""E004 plan step B1: run B's Part I move sequence on another seed, each move's frame chosen by A4's window search.

usage (from this folder, with ../../.venv/bin/python, under nice):
       runb_window.py run --seed 1 --out DIR [--window 60] [--last S3#..]
                     (DIR/movie.jsonl and DIR/results.jsonl in run B's format, DIR/window.jsonl per move)
       runb_window.py explicit --seed 1 --run DIR
                     (DIR/movie-explicit.jsonl: the bridge's automatic '.' and Esc of a replay from boot made explicit
                     keys, as scripts/anypercent_verify.py --explicit does, and the replay with them and no automatic
                     skips checked against the run's done frames)

An instrument, as runb_start.py and trace_check.py: it reads run B's route (routes/anypercent, the moves run B played)
and run B's results (each move's room), which the system never sees. Nothing the system runs imports it.

Why: run B's recorded inputs are frame- and pixel-exact for run B's own seed. On another seed they leave run B's moves
at some move (trace-validation.md, "Run B on another seed"), so B1 checks only the moves before it. Here each move is
played by the route's own executor (``speedrun.executor``, the code that made run B), from a frame the engine picks.

**The window search** (window-search.md, sections 2-3, on one input at a time): from the parent at the frame the
previous move was done, t0, a child is forked at each frame f = t0, t0 + 1, ... (the parent idles one frame at a time)
and plays the move through ``Executor.play`` from there (its inputs land on the first frames at or after f that the
engine takes them, as in run B). A child forked before the frame its first input landed on is the same run as one
forked at any frame up to it (Lemma 5), so the next child is forked at that frame. A child succeeds when the move's ``until`` holds (the executor's own end) and the room
then is run B's room for that move. The success with the least bridge clock (quarter-frames) is kept. Forking stops
once f + 1 reaches the best success's done frame: a move issued at f is done at f + 1 at the earliest, and the clock
grows with the frame, so no later child can finish first. A child that cannot finish before the best one's done frame
is stopped there (``max_frames``). The window ends after ``--window`` frames; a move none of its children plays is
reported, and an optional move is left out (``skipped``), as run B's own play does.

C2: the seed is given; the Any% seed and the test seeds 31-35 are refused.
"""

import argparse
import dataclasses
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))
sys.path.insert(0, str(HERE.parents[1] / "scripts"))

import groundtruth  # noqa: E402
from speedrun import releases, route  # noqa: E402
from speedrun.control import Controller, ControlError  # noqa: E402
from speedrun.engine import EngineConfig  # noqa: E402
from speedrun.executor import Executor, RouteError  # noqa: E402

ANY_SEED = 836557856
TEST_SEEDS = range(31, 36)
WINDOW = 60  # frames a move's window spans at most (play_steered's own delay range)


def say(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr, flush=True)


def check_seed(seed: int) -> None:
    if seed == ANY_SEED or seed in TEST_SEEDS:
        raise SystemExit(f"seed {seed} is not used here (plan.md)")


def config(out: Path, seed: int, inputs: Path | None = None, skips: bool = True) -> EngineConfig:
    """Run B's play settings (scripts/anypercent_play.py: DOS CD, talkspeed 255, segment start at frame 46, var 19
    free, the bridge's automatic '.' and Esc on) with ``seed``; ``skips`` False and ``inputs`` an explicit movie for
    the replay groundtruth.py makes (replay_kitchen.cfg_of's settings)."""
    rel = releases.get("dos-cd")
    return EngineConfig(out_dir=out, game_path=releases.game_dir(rel), release="dos-cd", timer_next="any",
                        start_frame=46, max_ticks=10**7, seed=seed, raw_inputs=inputs, skip_text=skips,
                        skip_cutscenes=skips, game_settings={"talkspeed": "255"})


def part1_moves(last: str | None) -> list:
    moves = [m for m in route.full_route() if m.name.split(" ")[0][:3] in groundtruth.PART_I]
    if last is not None:
        n = next(i for i, m in enumerate(moves) if m.name.split(" ")[0] == last)
        moves = moves[:n + 1]
    return moves


def runb_rooms() -> dict:
    return {json.loads(x)["name"]: json.loads(x).get("room")
            for x in (groundtruth.RUN / "results.jsonl").read_text().splitlines() if x.strip()}


def window(c: Controller, parent, skips: dict, move, room_want, width: int) -> tuple:
    """The window search for one move from ``parent`` (module docstring). Returns (the best child's branch, its
    executor, its result) or None, and the children's rows. The parent is left at the window's last frame."""
    t0 = parent.status["frame"]
    best = None
    rows = []
    f = t0
    while f < t0 + width:
        if best is not None and f + 1 >= best[2].done_frame:
            break
        if f > parent.status["frame"]:
            st = parent.run(frame=f)
            if "end" in st:
                break
        ch = c.fork(parent)
        ex = Executor(ch, "dos-cd", c.fork)
        ex.skips = dict(skips)
        m = move if best is None else dataclasses.replace(move, max_frames=max(1, best[2].done_frame - f))
        try:
            r = ex.play(m)
            err = None
        except (RouteError, ControlError) as e:
            r, err = None, str(e)[:200]
        room = ch.status.get("room") if r is not None else None
        ok = r is not None and (room_want is None or room == room_want)
        rows.append({"f": f, "ok": ok, "done": r.done_frame if r else None, "qf": r.qf if r else None,
                     "issue": r.issue_frame if r else None, "room": room, "error": err})
        if ok and (best is None or r.qf < best[2].qf):
            if best is not None:
                best[0].quit()
            best = (ch, ex, r)
        else:
            ch.quit()
        # window-search.md Lemma 5: a child forked at any frame before the one its first input landed on waits as the
        # parent idles and lands its inputs on the same frames: the same run, so the next child starts there
        f = max(f + 1, r.issue_frame) if r is not None else f + 1
    return best, rows


def cmd_run(a) -> None:
    check_seed(a.seed)
    out = a.out
    out.mkdir(parents=True, exist_ok=True)
    rooms = runb_rooms()
    moves = part1_moves(a.last)
    rel = releases.get("dos-cd")
    movie, results = [], []
    wlog = open(out / "window.jsonl", "w")
    t_all = time.monotonic()
    with Controller(config(out / "engine", a.seed), timeout_s=600) as c:
        b = c.start()
        skips = {"text": 1, "cutscenes": 1}
        qf0 = None
        for i, m in enumerate(moves):
            t = time.monotonic()
            label = m.name.split(" ")[0]
            best, rows = window(c, b, skips, m, rooms.get(m.name), a.window)
            n_ok = sum(1 for r in rows if r["ok"])
            if best is None:
                if m.optional:
                    results.append({"i": i, "name": m.name, "issue": b.frame, "done": b.frame, "rta": None,
                                    "room": b.status.get("room"), "skipped": True})
                    wlog.write(json.dumps({"i": i, "name": m.name, "children": len(rows), "ok": 0, "skipped": True,
                                           "rows": rows[:80], "s": round(time.monotonic() - t, 2)}) + "\n")
                    wlog.flush()
                    say(f"{label}: optional, no child played it in {len(rows)} frames: left out")
                    continue
                wlog.write(json.dumps({"i": i, "name": m.name, "children": len(rows), "ok": 0, "stop": True,
                                       "rows": rows[:80], "s": round(time.monotonic() - t, 2)}) + "\n")
                say(f"{label}: no child played it in {len(rows)} frames ({rows[0]['error'] if rows else ''}); stop")
                break
            ch, ex, r = best
            b.quit()
            b = ch
            movie.extend(ex.movie)
            skips = dict(ex.skips)
            if qf0 is None:
                qf0 = b.status.get("qf0")
            rta = round((r.qf - (b.status.get("qf0") or 0)) / rel.timer_hz, 3) if b.status.get("qf0") else None
            results.append({"i": i, "name": m.name, "issue": r.issue_frame, "done": r.done_frame, "rta": rta,
                            "room": b.status.get("room"), "skipped": r.skipped})
            first_ok = next(x for x in rows if x["ok"])
            wlog.write(json.dumps({"i": i, "name": m.name, "children": len(rows), "ok": n_ok, "chosen_f": None if not rows
                                   else next(x["f"] for x in rows if x["ok"] and x["qf"] == r.qf),
                                   "first_ok_f": first_ok["f"], "first_ok_done": first_ok["done"], "done": r.done_frame,
                                   "qf": r.qf, "room": b.status.get("room"), "rows": rows[:80],
                                   "s": round(time.monotonic() - t, 2)}) + "\n")
            wlog.flush()
            say(f"{label}: done {r.done_frame} (issue {r.issue_frame}), {len(rows)} children, {n_ok} ok, "
                f"{time.monotonic() - t:.1f} s")
            (out / "movie.jsonl").write_text("".join(json.dumps(e) + "\n" for e in movie))
            (out / "results.jsonl").write_text("".join(json.dumps(x) + "\n" for x in results))
        b.quit()
    wlog.close()
    (out / "movie.jsonl").write_text("".join(json.dumps(e) + "\n" for e in movie))
    (out / "results.jsonl").write_text("".join(json.dumps(x) + "\n" for x in results))
    say(f"{len(results)} moves in {time.monotonic() - t_all:.0f} s")


def cmd_explicit(a) -> None:
    """The run's movie from boot with the automatic skips on, its skip records made explicit keys, and that movie
    replayed with the skips off: the done frames of the run's moves must be reached the same (each move's ``until``
    is not rechecked; the frames and the rooms are)."""
    import anypercent_verify as AV
    check_seed(a.seed)
    run = a.run
    movie = run / "movie.jsonl"
    results = [json.loads(x) for x in (run / "results.jsonl").read_text().splitlines() if x.strip()]
    last = max(r["done"] for r in results)
    out1 = run / "auto-skips"
    with Controller(config(out1, a.seed, movie, True), timeout_s=3600) as c:
        b = c.start()
        rooms1 = {}
        for r in sorted(results, key=lambda r: r["done"]):
            st = b.run(frame=r["done"])
            rooms1[r["name"]] = st.get("room")
        b.run(frame=last + 2)
        b.quit()
    path = AV.explicit_movie(movie, out1 / "trace.jsonl", run)
    out2 = run / "explicit"
    with Controller(config(out2, a.seed, path, False), timeout_s=3600) as c:
        b = c.start()
        rooms2 = {}
        for r in sorted(results, key=lambda r: r["done"]):
            st = b.run(frame=r["done"])
            rooms2[r["name"]] = st.get("room")
        b.quit()
    same = sum(1 for r in results if rooms1.get(r["name"]) == r["room"] == rooms2.get(r["name"]))
    doc = {"moves": len(results), "rooms_equal": same,
           "differ": [r["name"] for r in results if not (rooms1.get(r["name"]) == r["room"] == rooms2.get(r["name"]))][:20],
           "explicit": str(path)}
    (run / "explicit.json").write_text(json.dumps(doc, indent=1) + "\n")
    say(f"explicit movie: {path}; rooms at the done frames equal in both replays on {same} of {len(results)} moves")


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("run")
    p.add_argument("--seed", type=int, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--window", type=int, default=WINDOW)
    p.add_argument("--last", default=None, help="the last move label to play (default: Part I's last)")
    p = sub.add_parser("explicit")
    p.add_argument("--seed", type=int, required=True)
    p.add_argument("--run", type=Path, required=True)
    a = ap.parse_args()
    {"run": cmd_run, "explicit": cmd_explicit}[a.cmd](a)


if __name__ == "__main__":
    main()
