"""Search the Melee-map point that meets a pirate first (the insult-fight intercepts, route S2#22).

usage: uv run python scripts/anypercent_intercept.py RUN UPTO OUT.jsonl [--delays 0,1,2,...] [--talkspeed 85]

Replays RUN's movie (movie.jsonl, results.jsonl) from boot to the frame of result index UPTO (the
map entry before a fight), then, on a fork per candidate, clicks a map point after d frames and runs
until a fight starts (room 49). The candidates are the points of the map's road walkboxes (room 85
BOXD: line boxes) every 3 px, without an object under them. A pirate fights only a standing ego
within 2 px of it (room-085-melee/local-202.txt [018A]-[01A5]), and the pirates spawn on the map
entry's RNG draws, so the answer holds for one seed and one input prefix only. Rows are ranked by
the fight frame plus the road time back to the fork at WalkSpeed(2,2) (max(|dx|,|dy|)/2 frames),
since script 114 puts ego back where the fight started. Writes all rows to OUT.jsonl.
"""

import argparse
import heapq
import json
import struct
from pathlib import Path

from speedrun import paths, releases
from speedrun.control import Controller
from speedrun.engine import EngineConfig

SEED = 836557856
FORK = (71, 86)  # the fork's road end (911 walk point side), where S1#84 and the forest start


def road_boxes(room: int = 85) -> list[tuple[tuple[int, int], tuple[int, int]]]:
    d = (paths.ROOT / "build" / "blocks-dos-cd" / "DISK_0001" / "LECF" / f"LFLF_{room:04d}" / "ROOM" / "BOXD").read_bytes()
    if d[:4] == b"BOXD":
        d = d[8:]
    segs = []
    for i in range(1, struct.unpack_from("<H", d, 0)[0]):
        v = struct.unpack_from("<8hBBH", d, 2 + i * 20)
        pts = sorted({(v[0], v[1]), (v[2], v[3]), (v[4], v[5]), (v[6], v[7])})
        if len(pts) == 2:
            segs.append((pts[0], pts[1]))
    return segs


def road_time(segs) -> dict:
    graph: dict = {}
    for a, c in segs:
        w = max(abs(a[0] - c[0]), abs(a[1] - c[1])) / 2
        graph.setdefault(a, []).append((c, w))
        graph.setdefault(c, []).append((a, w))
    dist, queue = {FORK: 0.0}, [(0.0, FORK)]
    while queue:
        w, u = heapq.heappop(queue)
        if w > dist[u]:
            continue
        for v, ww in graph.get(u, []):
            if w + ww < dist.get(v, 1e9):
                dist[v] = w + ww
                heapq.heappush(queue, (w + ww, v))
    return dist


def candidates(segs, dist) -> list[tuple[tuple[int, int], float]]:
    out = {}
    for a, c in segs:
        n = max(abs(a[0] - c[0]), abs(a[1] - c[1]))
        for k in range(0, n + 1, 3):
            p = (round(a[0] + (c[0] - a[0]) * k / max(n, 1)), round(a[1] + (c[1] - a[1]) * k / max(n, 1)))
            t = min(dist.get(a, 1e9) + max(abs(p[0] - a[0]), abs(p[1] - a[1])) / 2,
                    dist.get(c, 1e9) + max(abs(p[0] - c[0]), abs(p[1] - c[1])) / 2)
            out[p] = min(t, out.get(p, 1e9))
    return sorted(out.items())


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("run", type=Path)
    ap.add_argument("upto", type=int)
    ap.add_argument("out", type=Path)
    ap.add_argument("--delays", default="0,1,2,3,4,6,8,10,12")
    ap.add_argument("--talkspeed", default="85")
    ap.add_argument("--limit", type=int, default=450, help="frames to wait for a fight per candidate")
    args = ap.parse_args()
    delays = [int(x) for x in args.delays.split(",")]
    res = [json.loads(l) for l in (args.run / "results.jsonl").read_text().splitlines()]
    stop = [r for r in res if r["i"] <= args.upto][-1]["done"]
    work = args.out.with_suffix(".engine")
    work.mkdir(parents=True, exist_ok=True)
    pre = work / "prefix.jsonl"
    pre.write_text("".join(l + "\n" for l in (args.run / "movie.jsonl").read_text().splitlines()
                           if json.loads(l)["frame"] <= stop))
    rel = releases.get("dos-cd")
    cfg = EngineConfig(out_dir=work, game_path=releases.game_dir(rel), release="dos-cd", timer_next="any",
                       start_frame=46, max_ticks=10**7, seed=SEED, raw_inputs=pre,
                       game_settings={"talkspeed": args.talkspeed})
    segs = road_boxes()
    cands = candidates(segs, road_time(segs))
    rows = []
    with Controller(cfg, timeout_s=900) as c:
        root = c.start()
        root.run(frame=stop)
        f0 = root.frame
        for p, road in cands:
            if root.at(*p).get("obj"):
                continue
            for d in delays:
                b = c.fork(root)
                b.inputs([{"frame": f0 + 1 + d, "click": list(p)}])
                st = b.run(until=[{"room": 49}], max_frame=f0 + args.limit)
                fight = b.frame if st.get("stop") == "until" else None
                b.quit()
                rows.append({"point": list(p), "delay": d, "fight": fight, "road": road,
                             "score": None if fight is None else fight - f0 + road})
    rows.sort(key=lambda r: (r["score"] is None, r["score"] or 0))
    args.out.write_text("".join(json.dumps(r) + "\n" for r in rows))
    for r in rows[:15]:
        print(r)


if __name__ == "__main__":
    main()
