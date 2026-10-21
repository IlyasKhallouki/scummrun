"""Play the hand-written Any% route on DOS CD until the first failure; print per-move RTA.

usage: play_route.py OUT SEED [RESUME_DIR FROM_MOVE]
With RESUME_DIR, the movie of that run is replayed (raw inputs, full speed) up to the end of
move FROM_MOVE - 1, and play continues from FROM_MOVE.
"""

import json
import os
import sys
from dataclasses import replace
from pathlib import Path

from speedrun import releases, route
from speedrun.control import Controller, ControlError
from speedrun.engine import EngineConfig
from speedrun.executor import Executor, RouteError

OUT = Path(sys.argv[1])
SEED = int(sys.argv[2])
RESUME = Path(sys.argv[3]) if len(sys.argv) > 3 else None
FROM = int(sys.argv[4]) if len(sys.argv) > 4 else 0

DELAYS = {836557856: {"S1#45 ": 3}}  # seed -> {move name prefix: delay frames}
moves = route.full_route()
moves = [replace(m, delay=next((d for p, d in DELAYS.get(SEED, {}).items() if m.name.startswith(p)), 0))
         for m in moves]

rel = releases.get("dos-cd")
hz = rel.timer_hz
OUT.mkdir(parents=True, exist_ok=True)
prefix, results = [], []
if RESUME:
    old = [json.loads(l) for l in (RESUME / "results.jsonl").read_text().splitlines()]
    results = old[:FROM]
    stop = results[-1]["done"]
    prefix = [e for e in (json.loads(l) for l in (RESUME / "movie.jsonl").read_text().splitlines())
              if e["frame"] <= stop]
    (OUT / "prefix.jsonl").write_text("".join(json.dumps(e) + "\n" for e in prefix))

cfg = EngineConfig(out_dir=OUT, game_path=releases.game_dir(rel), release="dos-cd", timer_next="any",
                   start_frame=46, max_ticks=10**7, seed=SEED, raw_inputs=(OUT / "prefix.jsonl") if RESUME else None,
                   game_settings={"talkspeed": os.environ["TALKSPEED"]} if os.environ.get("TALKSPEED") else {})

with Controller(cfg, timeout_s=600) as c:
    b = c.start()
    bad = []
    for m in moves:
        for which in ("after", "until"):
            conds = getattr(m, which)
            if conds:
                try:
                    b.holds(conds)
                except ControlError as e:
                    bad.append(f"{m.name} {which}: {e}")
    if bad:
        print("CONDITION ERRORS:\n" + "\n".join(bad))
        sys.exit(1)
    ex = Executor(b, "dos-cd", c.fork)
    if RESUME:
        b.run(frame=results[-1]["done"])
        ex.movie = list(prefix)
        # The run's skip setting is the last move-level "skips" switch replayed (auto_skips pairs
        # restore it, so the last event is the run's setting).
        sk = [e["skips"] for e in prefix if "skips" in e]
        if sk:
            ex.skips = dict(sk[-1])
    err = None
    i = FROM
    while i < len(moves):
        group = [moves[i]]
        while moves[i].optional and i + len(group) < len(moves) and moves[i + len(group)].optional                 and moves[i + len(group)].name.split("]")[0].split("#", 1)[1].split(" ", 1)[1] == moves[i].name.split("]")[0].split("#", 1)[1].split(" ", 1)[1]:
            group.append(moves[i + len(group)])
        try:
            rs = ex.play_group(group)
        except (RouteError, ControlError) as e:
            err = f"move {i} {e}"
            break
        b = ex.b  # a steered move or a rolled-back group continues on a snapshot branch
        qf0 = b.status.get("qf0") or 0
        for m, r in zip(group, rs):
            rta = (r.qf - qf0) / hz
            results.append({"i": i, "name": m.name, "issue": r.issue_frame, "done": r.done_frame, "rta": round(rta, 3),
                            "room": b.status.get("room"), "skipped": r.skipped})
            print(f"{i:3d} f{r.issue_frame:6d}->{r.done_frame:6d} rta {rta:8.2f}s room {b.status.get('room')}"
                  f"{' SKIPPED' if r.skipped else ''}  {m.name}", flush=True)
            i += 1
    (OUT / "movie.jsonl").write_text("".join(json.dumps(e) + "\n" for e in ex.movie))
    (OUT / "results.jsonl").write_text("".join(json.dumps(r) + "\n" for r in results))
    if err:
        print("FAILED:", err)
        st = b.state()
        print("status", json.dumps(b.status))
        print("verbs", [(v["id"], v["name"]) for v in st["verbs"]][:30])
        print("inventory", st["inventory"])
        (OUT / "fail-state.json").write_text(json.dumps(st))
