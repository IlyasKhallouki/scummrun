"""Fix round 1, F2: the planner rule's premise against the engine, frame by frame (fix-round1.md, F2).

usage: ../../.venv/bin/python due_engine.py --out DIR [--first FRAME] [--last FRAME]
       then: scumm/target/release/reader-port due-engine DIR/frames.jsonl.gz

It replays run B's Part I from boot with its recorded inputs and groundtruth.py's engine settings (one engine
process, ``groundtruth.replay``'s config), stops at every frame from ``--first`` to ``--last`` (default 45 to 9539,
the ground truth's first and last stop) and writes DIR/frames.jsonl.gz: per frame its number, run B's inputs
scheduled for it (the movie's keys and clicks) and the engine's facts at its stop (``facts.facts_of``; the first frame
whole, then ``facts.diff`` against the frame before). A stop at frame N comes after N's processInput, runAllScripts
and checkExecVerbs (groundtruth.py's stop rule), so the slots' turns of frame N are in N's facts.

``reader-port due-engine`` then checks, for every slot live and unfrozen at a wait at frame N - 1's stop, what the
reader's due rule says against what the engine did in frame N (scumm/reader/src/due.rs ``engine_check``).

An instrument, not part of the system, as groundtruth.py: it reads run B's recorded inputs (allowed for the model
check and single-input differential tests from its states, decisions.md D6 and the fix round's rules).
"""

import argparse
import gzip
import json
import sys
import time
from pathlib import Path

import groundtruth as G
from facts import diff, facts_of
from speedrun import releases
from speedrun.control import Controller
from speedrun.engine import EngineConfig


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--first", type=int, default=45)
    ap.add_argument("--last", type=int, default=9539)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    inputs: dict[int, list] = {}
    for e in G.load_jsonl(G.MOVIE):
        if G.is_input(e):
            inputs.setdefault(e["frame"], []).append({k: v for k, v in e.items() if k != "frame"})
    rel = releases.get("dos-cd")
    cfg = EngineConfig(out_dir=a.out / "engine", game_path=releases.game_dir(rel), release="dos-cd", timer_next="any",
                       start_frame=46, max_ticks=10**7, seed=G.SEED, raw_inputs=G.MOVIE,
                       skip_text=False, skip_cutscenes=False, game_settings={"talkspeed": "255"})
    t0 = time.monotonic()
    prev = None
    with Controller(cfg, timeout_s=3600) as c, gzip.open(a.out / "frames.jsonl.gz", "wt", compresslevel=3) as out:
        b = c.root
        for f in range(a.first, a.last + 1):
            st = b.run(frame=f)
            if st.get("frame") != f:
                raise SystemExit(f"the replay stopped at {st} instead of frame {f}")
            s = b.state()
            facts = facts_of(s)
            line = {"frame": f, "inputs": inputs.get(f, [])}
            if prev is None:
                line["facts"] = facts
            else:
                line["diff"] = diff(prev, facts)
            out.write(json.dumps(line, separators=(",", ":"), sort_keys=True) + "\n")
            prev = facts
    print(f"due_engine: frames {a.first}-{a.last} written to {a.out / 'frames.jsonl.gz'} in {time.monotonic() - t0:.1f} s",
          file=sys.stderr)


if __name__ == "__main__":
    main()
