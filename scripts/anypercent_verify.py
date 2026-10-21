"""Verify an Any% movie from boot and report its time (the only source of a reported time).

usage: uv run python scripts/anypercent_verify.py MOVIE OUT [--goal '[C3...]'] [--talkspeed 255]
                                                    [--explicit]

1. Replay MOVIE (raw input JSONL) from boot in one engine process with the run's settings
   (seed 836557856, DOS CD, the bridge's automatic '.'/Esc on), until ``--goal`` holds (default:
   the final fade, room 0 with var 194 == 122). The time runs from the logo-skip Esc (qf0) to the
   goal frame, at the release's timer rate.
2. ``--explicit``: turn every automatic skip the bridge pressed (trace "skip" records) into an
   explicit key event, replay that movie with the automatic skips off, and confirm the same goal
   frame: rules/anypercent.md requires a reported run to replay from its raw input alone.
Reports the "Last Part" card's length (room 95) and the time without it too.
Writes OUT/verify.json.
"""

import argparse
import json
from pathlib import Path

from speedrun import releases
from speedrun.control import Controller
from speedrun.engine import EngineConfig

SEED = 836557856
FINAL = [{"room": 0}, {"var": 194, "eq": 122}]


def replay(movie: Path, out: Path, goal: list, talkspeed: str, skips: bool) -> dict:
    rel = releases.get("dos-cd")
    cfg = EngineConfig(out_dir=out, game_path=releases.game_dir(rel), release="dos-cd", timer_next="any",
                       start_frame=46, max_ticks=10**7, seed=SEED, raw_inputs=movie,
                       skip_text=skips, skip_cutscenes=skips, game_settings={"talkspeed": talkspeed})
    with Controller(cfg, timeout_s=3600) as c:
        b = c.start()
        last = max(json.loads(l)["frame"] for l in movie.read_text().splitlines())
        st = b.run(until=goal, max_frame=last + 20000)
        result = {"stop": st.get("stop"), "frame": st.get("frame"), "room": st.get("room"),
                  "qf": st.get("qf"), "qf0": st.get("qf0")}
        if st.get("stop") == "until":
            result["rta"] = round((st["qf"] - st["qf0"]) / rel.timer_hz, 3)
        b.quit()
    card = last_part_card(out / "trace.jsonl", rel.timer_hz)
    if card is not None:
        result["card_s"] = card
        if "rta" in result:
            result["rta_without_card"] = round(result["rta"] - card, 3)
    return result


def last_part_card(trace: Path, hz: float) -> float | None:
    """Seconds in the "Last Part" card (room 95, global/script-131.txt): it waits for CD track
    6 to end with no override, so its length is the track's, not the player's."""
    rooms = [r for r in (json.loads(l) for l in trace.read_text().splitlines()) if r.get("type") == "room"]
    start = next((r for r in rooms if r["room"] == 95), None)
    if start is None:
        return None
    end = next((r for r in rooms if r["qf"] >= start["qf"] and r["room"] != 95), None)
    return None if end is None else round((end["qf"] - start["qf"]) / hz, 3)


def explicit_movie(movie: Path, trace: Path, out: Path) -> Path:
    events = [json.loads(l) for l in movie.read_text().splitlines()]
    keys = []
    for line in trace.read_text().splitlines():
        r = json.loads(line)
        if r.get("type") == "skip":
            keys.append({"frame": r["frame"], "key": "esc" if r.get("kind") == "cutscene" else "."})
    merged = [e for e in events if "skips" not in e] + keys
    merged.sort(key=lambda e: e["frame"])
    path = out / "movie-explicit.jsonl"
    path.write_text("".join(json.dumps(e) + "\n" for e in merged))
    return path


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("movie", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--goal", default=json.dumps(FINAL))
    ap.add_argument("--talkspeed", default="255")
    ap.add_argument("--explicit", action="store_true")
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    goal = json.loads(args.goal)
    report = {"movie": str(args.movie), "goal": goal, "seed": SEED, "talkspeed": args.talkspeed}
    first = args.out / "auto-skips"
    report["auto_skips"] = replay(args.movie, first, goal, args.talkspeed, True)
    print("auto skips:", report["auto_skips"], flush=True)
    if args.explicit and report["auto_skips"].get("stop") == "until":
        movie2 = explicit_movie(args.movie, first / "trace.jsonl", args.out)
        report["explicit"] = replay(movie2, args.out / "explicit", goal, args.talkspeed, False)
        report["explicit"]["movie"] = str(movie2)
        print("explicit keys:", report["explicit"], flush=True)
    (args.out / "verify.json").write_text(json.dumps(report, indent=2) + "\n")
    if report["auto_skips"].get("stop") != "until":
        raise SystemExit("the goal was not reached")
    if args.explicit and (report["explicit"].get("frame"), report["explicit"].get("qf")) != \
            (report["auto_skips"]["frame"], report["auto_skips"]["qf"]):
        raise SystemExit("the explicit-key replay does not reach the goal on the same frame")


if __name__ == "__main__":
    main()
