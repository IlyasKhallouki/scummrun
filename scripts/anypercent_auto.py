"""Play the Any% run goal by goal on DOS CD: route hints first, the milestone search as fallback.

usage: uv run python scripts/anypercent_auto.py OUT --prefix RUN [--resume OUT_OLD] [--start S3#91]
                                                [--goals routes/anypercent/noslide] [--mode fallback]

* ``--prefix``: a run directory (movie.jsonl + results.jsonl) whose state up to the move before
  ``--start`` is replayed from boot as raw input.
* Goals: the route files' milestones from ``--start`` (without the slide chain of S3), then, for
  every segment that has a file in ``--goals`` (``S5.json`` ...), that file's goals instead of the
  route's. A goal-list file holds {"goals": [{"name", "target": [C3...], "hint": [moves]}]}.
* ``--mode``: "fallback" searches only when the hint fails; "always" also tries to beat it.
* Records and the movie are written to OUT after every goal, so ``--resume OUT`` continues.

The settings are the run's: seed 836557856, talkspeed 255, the bridge's automatic skips on.
"""

import argparse
import json
import re
import time
from dataclasses import replace
from pathlib import Path

from speedrun import paths, releases, route
from speedrun.autoplay import SLIDE_VARS, AutoPlayer, Milestone, milestones, save
from speedrun.control import Controller
from speedrun.engine import EngineConfig
from speedrun.executor import Executor, Move
from speedrun.scriptindex import ScriptIndex
from speedrun.search import MilestoneSearch

SEED = 836557856
SEGMENT_BASE = {"S5": 5000, "S6": 6000, "S7": 7000}  # goal-list indices sort after the route's


def goal_milestones(goal_dir: Path) -> dict[str, list[Milestone]]:
    out = {}
    for seg, base in SEGMENT_BASE.items():
        f = goal_dir / f"{seg}.json"
        if not f.exists():
            continue
        data = json.loads(f.read_text())
        ms = []
        for k, g in enumerate(data["goals"]):
            hint = [route.to_move({k2: v for k2, v in h.items() if k2 in route._MOVE_FIELDS or k2 in route._DOC_FIELDS
                                   or k2 == "name"}, f"{seg}g{k + 1}") for h in g.get("hint", [])]
            ms.append(Milestone(base + k + 1, f"{seg}g{k + 1} {g['name']}", g["target"], hint))
        out[seg] = ms
    return out


def build_plan(start: str, goal_dir: Path | None, slide: bool = False) -> list[Milestone]:
    moves = route.full_route()
    idx = {m.name.split(" ")[0]: i for i, m in enumerate(moves)}
    if slide:  # the record's route as written: the slide chain, its skip settings and its goals
        return milestones(moves, idx[start])
    # Without the slide: no store visit, compass slide, map trick inside Otis's cutscene, stalls or
    # carry; the jail is entered straight from High Street.
    drop = {i for i in range(idx["S3#91"], idx["S3#118"]) if i not in (idx["S3#97"], idx["S3#98"])}
    goals = goal_milestones(goal_dir) if goal_dir else {}
    if goals:
        first = min(idx[f"{seg}#1"] for seg in goals)
        drop |= set(range(first, len(moves)))
    plan = milestones(moves, idx[start], drop, strip_skips=True)
    for seg in sorted(goals):
        plan += goals[seg]
    return plan


def delay_lookahead(player: AutoPlayer, ex: Executor, ms: Milestone, nxt: Milestone | None, n: int) -> Milestone:
    # A delay can drop the item slide as a search branch can: a candidate must end the next goal
    # with the slide counters (player.keep_vars) of the undelayed branch.
    """``ms`` with the delay (0..n frames on its first hint move) after which ``nxt`` is reached
    first: the catacomb chambers draw their layout and target cave on entry, so the frame of this
    exit decides how long the next chamber takes."""
    best, best_qf, ref = ms, None, None
    for d in range(n + 1):
        cand = replace(ms, hint=[replace(ms.hint[0], delay=ms.hint[0].delay + d)] + ms.hint[1:])
        o1 = player.step(ex, cand, search="never")
        if o1 is None or o1.ex is ex:
            continue
        qf, end = o1.qf, o1.ex
        o2 = None
        if nxt is not None:
            o2 = player.step(o1.ex, nxt, search="never")
            qf = o2.qf if o2 is not None else None
            end = o2.ex if o2 is not None else None
        kept = None if end is None else tuple(end.b.state()["vars"][v] for v in sorted(player.keep_vars))
        if d == 0:
            ref = kept
        if o2 is not None and o2.ex is not o1.ex:
            o2.ex.b.quit()
        o1.ex.b.quit()
        if player.keep_vars and ref is not None and kept != ref:
            continue
        if qf is not None and (best_qf is None or qf < best_qf):
            best, best_qf = cand, qf
    if best is not ms:
        print(f"  lookahead: {ms.name[:40]} delay {best.hint[0].delay - ms.hint[0].delay}", flush=True)
    return best


def skippable(ms: Milestone) -> bool:
    """Only goals of where ego stands may be left out (the oars skip's room shuffles): items, flags
    and counters are needed later."""
    return all(next(iter(c.get("not", c))) in ("room", "actor_room") for c in ms.target)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("out", type=Path)
    ap.add_argument("--prefix", type=Path, required=True)
    ap.add_argument("--resume", type=Path)
    ap.add_argument("--start", default="S3#91")
    ap.add_argument("--goals", type=Path, default=paths.ROOT / "routes" / "anypercent" / "noslide")
    ap.add_argument("--mode", default="fallback")
    ap.add_argument("--stop-after", type=int, default=0, help="stop after this many goals (0: all)")
    ap.add_argument("--slide", action="store_true", help="the item-slide route (A): route files as written")
    ap.add_argument("--talkspeed", default="255")
    ap.add_argument("--end", help="stop after the goal whose name starts with this (e.g. S4#77)")
    ap.add_argument("--lookahead", type=int, default=0,
                    help="for goals matching --lookahead-match: try 0..N frames of delay on the first hint move "
                         "and keep the one that finishes the NEXT goal first (random rooms drawn on entry)")
    ap.add_argument("--lookahead-match", default=r"Catacombs exit|catacombs cave|enter the catacombs")
    ap.add_argument("--early", action="store_true",
                    help="early inputs (Executor._play_early): try each click move's inputs 1-2 frames sooner on forks")
    ap.add_argument("--lookahead-slow", help="RUN:SECONDS: also look ahead before every goal that took at least "
                                             "SECONDS in RUN's results (a slow goal can hide a random wait)")
    args = ap.parse_args()
    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    moves = route.full_route()
    start_i = next(i for i, m in enumerate(moves) if m.name.startswith(args.start + " "))
    slow: set[int] = set()
    if args.lookahead_slow:
        run, secs = args.lookahead_slow.rsplit(":", 1)
        prev = None
        for r in (json.loads(l) for l in (Path(run) / "results.jsonl").read_text().splitlines()):
            if r.get("rta") is not None:
                if prev is not None and r["rta"] - prev >= float(secs):
                    slow.add(r["i"])
                prev = r["rta"]
    plan = build_plan(args.start, None if args.slide or not args.goals.exists() else args.goals, args.slide)
    if args.resume:
        records = [json.loads(l) for l in (args.resume / "results.jsonl").read_text().splitlines()]
        movie_src = args.resume / "movie.jsonl"
    else:
        records = [json.loads(l) for l in (args.prefix / "results.jsonl").read_text().splitlines()][:start_i]
        movie_src = args.prefix / "movie.jsonl"
    stop = records[-1]["done"]
    prefix = [e for e in (json.loads(l) for l in movie_src.read_text().splitlines()) if e["frame"] <= stop]
    (out / "prefix.jsonl").write_text("".join(json.dumps(e) + "\n" for e in prefix))
    plan = [ms for ms in plan if ms.index > records[-1]["i"]]

    rel = releases.get("dos-cd")
    cfg = EngineConfig(out_dir=out, game_path=releases.game_dir(rel), release="dos-cd", timer_next="any",
                       start_frame=46, max_ticks=10**7, seed=SEED, raw_inputs=out / "prefix.jsonl",
                       game_settings={"talkspeed": args.talkspeed})
    ix = ScriptIndex(paths.DATA_DIR / "scripts-dos-cd")
    with Controller(cfg, timeout_s=900) as c:
        b = c.start()
        b.run(frame=stop)
        ex = Executor(b, "dos-cd", c.fork)
        ex.early = args.early
        ex.movie = list(prefix)
        sk = [e["skips"] for e in prefix if "skips" in e]
        if sk:  # the run's automatic-skip setting is the last switch replayed
            ex.skips = dict(sk[-1])
        qf0 = b.status["qf0"]
        s = MilestoneSearch(ix, c.fork, verbose=True)
        player = AutoPlayer(s, c.fork, keep_vars=frozenset(SLIDE_VARS) if args.slide else frozenset())
        save(out, ex, records)
        k, done = 0, 0
        backup = None  # (k, hint Outcome, records) of the last goal the search won: one step of backtracking
        while k < len(plan):
            ms = plan[k]
            t = time.time()
            s.log.clear()
            nxt_slow = k + 1 < len(plan) and plan[k + 1].index in slow
            if args.lookahead and ms.hint and (re.search(args.lookahead_match, ms.name) or nxt_slow):
                ms = delay_lookahead(player, ex, ms, plan[k + 1] if k + 1 < len(plan) else None, args.lookahead)
            o = player.step(ex, ms, search=args.mode)
            for line in s.log:
                if line.startswith("  hint failed"):
                    print(f"{ms.index}{line}", flush=True)
            if o is None and backup is not None and backup[0] == k - 1:
                # The search's faster branch for the previous goal leads nowhere from here (a map
                # landing spot the next exit cannot be reached from): go back to that goal's hint.
                print(f"{ms.index} {ms.name[:52]}: not reached; back to the previous goal's hint branch", flush=True)
                ex.b.quit()
                _, alt, records = backup
                records = list(records) + [{"i": plan[k - 1].index, "name": plan[k - 1].name, "done": alt.ex.b.frame,
                                            "rta": round((alt.qf - qf0) / rel.timer_hz, 3), "how": "hint (backtrack)",
                                            "via": alt.via[-6:]}]
                ex, backup = alt.ex, None
                ex.early = args.early
                save(out, ex, records)
                continue
            if o is None:
                print(f"{ms.index} {ms.name[:60]}: NOT REACHED target {ms.target}", flush=True)
                print("\n".join(s.log[-30:]), flush=True)
                for j in range(k + 1, min(k + 4, len(plan))):
                    if not all(skippable(p) for p in plan[k:j]):
                        break
                    later = plan[j]
                    if ex.b.holds(later.target):
                        continue  # already true here: reaching it says nothing
                    s.log.clear()
                    o = player.step(ex, replace(later, hint=sum((p.hint for p in plan[k:j + 1]), [])), search=args.mode)
                    if o is not None:
                        for p in plan[k:j]:
                            records.append({"i": p.index, "name": p.name, "done": ex.b.frame, "rta": None,
                                            "how": "unreachable"})
                            print(f"{p.index} {p.name[:52]:52s} skipped: unreachable here", flush=True)
                        ms, k = later, j
                        break
                if o is None:
                    break
            if o.alt is not None and k + 1 < len(plan):
                # A search branch can be faster here and slower one goal later (a step the next
                # hint relies on was skipped): keep it only if it also reaches the next goal first.
                nxt = plan[k + 1]
                fin = []
                for br in (o, o.alt):
                    on = player.step(br.ex, nxt, search="never")
                    fin.append(on.qf if on is not None else None)
                    if on is not None and on.ex is not br.ex:
                        on.ex.b.quit()
                if fin[1] is not None and (fin[0] is None or fin[1] <= fin[0]):
                    print(f"  {ms.name[:40]}: search branch loses at the next goal; hint kept", flush=True)
                    o.ex.b.quit()
                    o = o.alt
            if backup is not None:
                backup[1].ex.b.quit()
                backup = None
            if o.alt is not None:
                backup = (k, o.alt, list(records))
            if o.ex is not ex:
                ex.b.quit()
                ex = o.ex
            ex.early = args.early
            rta = (o.qf - qf0) / rel.timer_hz
            records.append({"i": ms.index, "name": ms.name, "done": ex.b.frame, "rta": round(rta, 3), "how": o.how,
                            "via": o.via[-6:]})
            save(out, ex, records)
            print(f"{ms.index} {ms.name[:52]:52s} rta {rta:8.2f}s {o.how:12s} {time.time() - t:5.0f}s "
                  f"{o.via[-3:] if o.how != 'hint' else ''}", flush=True)
            k += 1
            done += 1
            if args.stop_after and done >= args.stop_after:
                break
            if args.end and ms.name.startswith(args.end + " "):
                break
        print("END frame", ex.b.frame, "room", ex.b.status.get("room"), flush=True)


if __name__ == "__main__":
    main()
