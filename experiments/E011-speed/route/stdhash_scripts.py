"""E011 route: random scripts for stdhash_ref.cc (libstdc++) and `lama stdhash` (taskcore/src/stdhash.rs).

usage: python stdhash_scripts.py OUT_DIR N SEED   writes N scripts OUT_DIR/s<k>.txt: sets and maps built by inserts,
erases, copies, intersections and constructions from vectors (with repeats), of sizes 0 to a few thousand, printed after
every few operations."""
import random
import sys
from pathlib import Path

out, n, seed = Path(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3])
out.mkdir(parents=True, exist_ok=True)
rng = random.Random(seed)
for k in range(n):
    lines = []
    size = rng.choice([3, 12, 14, 30, 100, 400, 3000])
    nvar = rng.choice([2, 10, 180, 1000])
    nval = rng.choice([2, 5, 60, 5000])
    for s in range(4):
        lines.append(f"F {s}")
        lines.append(f"I {s}")
        lines.append(f"M {s}")
    for step in range(rng.randint(1, 3 * size)):
        r = rng.random()
        s = rng.randrange(4)
        if r < 0.40:
            lines.append(f"f {s} {rng.randrange(nvar)} {rng.randrange(nval)}")
        elif r < 0.45:
            lines.append(f"e {s} {rng.randrange(nvar)} {rng.randrange(nval)}")
        elif r < 0.47:
            lines.append(f"c {s} {rng.randrange(4)}")
        elif r < 0.50:
            lines.append(f"x {s} {rng.randrange(4)} {rng.randrange(4)}")
        elif r < 0.52:
            c = rng.randint(0, 2 * size)
            v = " ".join(f"{rng.randrange(nvar)} {rng.randrange(nval)}" for _ in range(c))
            lines.append(f"r {s} {c} {v}")
        elif r < 0.75:
            lines.append(f"i {s} {rng.randrange(-5, 3 * size)}")
        elif r < 0.78:
            lines.append(f"j {s} {rng.randrange(-5, 3 * size)}")
        else:
            lines.append(f"m {s} {rng.randrange(-2, size)} {rng.randrange(nvar)} {rng.randrange(nval)}")
        if rng.random() < 0.05:
            lines += [f"p {s}", f"q {s}", f"o {s}"]
    lines += [f"{c} {s}" for s in range(4) for c in "pqo"]
    (out / f"s{k}.txt").write_text("\n".join(lines) + "\n")
