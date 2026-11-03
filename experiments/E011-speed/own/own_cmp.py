"""E011 round 2, item own: a warm B3 sequence (own_b3.sh) against B0 and the try's own time.

usage: python own_cmp.py B0_SEQ SEQ [SEQ...]      (run after the sequences, never inside them)

Per sequence: G11's comparison (bench/cmp_tries.py: the stop and its class, the attempt, the new entries' frames,
ticks, events and every checkpoint's facts), the idle measurement against B0's (the idle keys, first returns and
change counts, maps compared as maps: the reference's dict order follows its set order), and fmi's own account of each
try (result.json explore_s: wall, ports, own) with the medians over the 15 tries, the 8 ring(38) and the 7 ring(33).
"""

import json
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "bench"))
import cmp_tries  # noqa: E402


def idle_same(a: dict | None, b: dict | None) -> bool:
    if not a or not b:
        return a == b or (not a and not b)
    return (a.get("returns") == b.get("returns") and a.get("first_return") == b.get("first_return")
            and a.get("changes") == b.get("changes") and a.get("frames") == b.get("frames"))


def main() -> None:
    b0 = Path(sys.argv[1])
    for seq in map(Path, sys.argv[2:]):
        rows = []
        for jd in sorted((seq / "jobs").glob("j*")):
            if not (jd / "result.json").exists():
                continue
            c = cmp_tries.compare(b0 / "jobs" / jd.name, jd)
            ra = json.loads((b0 / "jobs" / jd.name / "result.json").read_text())
            rb = json.loads((jd / "result.json").read_text())
            e = rb.get("explore_s") or {}
            ring = 33 if 33 in (rb.get("reading") or {}).get("scope", []) and 28 in (rb.get("reading") or {}).get("scope", []) else 38
            rows.append({"job": jd.name, "ring": ring, "same_class": c["same_class"], "differ": c["differ"],
                         "idle_same": idle_same(ra.get("idle"), rb.get("idle")), "own_ms": round(e.get("own", 0) * 1e3, 2),
                         "ports_s": round(e.get("ports", 0), 3), "wall_s": round(e.get("wall", 0), 3),
                         "idle_s": (rb.get("idle") or {}).get("seconds")})
        print(f"== {seq}")
        for r in rows:
            print(f"  {r['job']} ring({r['ring']}) class {'=' if r['same_class'] else 'DIFFERS'} differ {len(r['differ'])}"
                  f" idle {'=' if r['idle_same'] else 'DIFFERS'} own {r['own_ms']} ms wall {r['wall_s']} s idle {r['idle_s']}")
            for d in r["differ"][:3]:
                print("     ", str(d)[:150])
        med = lambda xs: round(statistics.median(xs), 2) if xs else None
        own = [r["own_ms"] for r in rows]
        print(f"  tries {len(rows)}; G11 same class {sum(r['same_class'] for r in rows)}, no difference "
              f"{sum(not r['differ'] for r in rows)}, idle equal {sum(r['idle_same'] for r in rows)}")
        print(f"  own ms: median {med(own)} (ring(38) {med([r['own_ms'] for r in rows if r['ring'] == 38])}, ring(33) "
              f"{med([r['own_ms'] for r in rows if r['ring'] == 33])}), min {min(own) if own else None}, max {max(own) if own else None}")
        print(f"  wall s: median {med([r['wall_s'] for r in rows])} (ring(38) {med([r['wall_s'] for r in rows if r['ring'] == 38])},"
              f" ring(33) {med([r['wall_s'] for r in rows if r['ring'] == 33])}), sum {round(sum(r['wall_s'] for r in rows), 1)}")
        (seq / "own_cmp.json").write_text(json.dumps(rows, indent=1) + "\n")


if __name__ == "__main__":
    main()
