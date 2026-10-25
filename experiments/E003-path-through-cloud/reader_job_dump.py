"""E003, D7: the spot-check of the Rust reading at Part I's scale. The Rust reading of a scope (``reader-scope --k K
--sample N --seed S --dump FILE``) draws jobs at random, N per room (or "global"), and writes each with what its batch
started with (the ``args:`` values, the folded lists of locals) and the records it made. This script makes each job
again with reader.py's ``Builder.run`` on the same scope (size_scaling.py's start, goal and scopes) and compares the
records, byte for byte in golden.py's encoding. An instrument: it imports reader.py as it is and changes nothing.

usage (from this folder, with ../../.venv/bin/python, under nice 10):
  reader_job_dump.py DUMP [--part I --parts N] [--out FILE]

The scope's later passes start from wider domains: the dump's head lists, per pass that widened them, the values
written outside their domains, and this script widens them with reader.py's own ``extend_domains`` and ``_reset``.
Each job line of the output: {"g", "stratum", "label", "rust", "python", "equal", "first_diff", "py_s"}; the last
line sums them up. Runs under PYTHONHASHSEED=0 (it starts itself again with it), as reader_port_oracle.py does.
"""

import argparse
import collections
import json
import os
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
HASHSEED = "0"


def main() -> None:
    if os.environ.get("PYTHONHASHSEED") != HASHSEED:
        os.execve(sys.executable, [sys.executable, *sys.argv], dict(os.environ, PYTHONHASHSEED=HASHSEED))
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("dump")
    ap.add_argument("--part", type=int, default=0)
    ap.add_argument("--parts", type=int, default=1)
    ap.add_argument("--out")
    a = ap.parse_args()

    import domains as D
    import golden
    import model as M
    import reader as R
    import size_scaling
    from reader_run_dump import dec

    lines = [json.loads(ln) for ln in Path(a.dump).read_text().splitlines()]
    head, jobs = lines[0], lines[1:]
    assert head["kind"] == "head" and head["seed"] == size_scaling.SEED
    jobs = [j for i, j in enumerate(jobs) if i % a.parts == a.part]

    t0 = time.monotonic()
    game = M.Game.load()
    start, gl = size_scaling.start_facts(), size_scaling.goal()
    sc = R.Scope.of(game, head["rooms"], start)
    b = R.Builder(R.Reader(sc, D.build(start, goals=gl), game))
    widened = dec(head["widened"])
    for n in range(head["pass"] - 1):  # read_all between passes: extend_domains, then _reset
        b.out_of_domain = collections.defaultdict(set, {k: set(v) for k, v in widened[n].items()})
        b.extend_domains()
        b._reset()
    setup_s = time.monotonic() - t0

    out = open(a.out, "w") if a.out else sys.stdout
    n_equal = n_recs = 0
    for j in jobs:
        b.table.args_vals = collections.defaultdict(set, {k: set(v) for k, v in dec(j["args_vals"]).items()})
        b.folded = set(dec(j["folded"]))
        kind, label, pins, pre, what, checks = dec(j["job"])
        t = time.monotonic()
        recs = b.run(kind, label, pins, pre, what, checks)
        secs = time.monotonic() - t
        mine = [golden._text(golden.enc(r)) for r in recs]
        rust = [golden._text(r) for r in j["records"]]
        diff = None
        if mine != rust:
            for i, (x, y) in enumerate(zip(rust, mine)):
                if x != y:
                    diff = f"record {i}: {golden.compare_lines([x], [y])}"
                    break
            else:
                diff = f"{len(rust)} records in Rust, {len(mine)} in Python"
        n_equal += diff is None
        n_recs += len(rust)
        out.write(json.dumps({"g": j["g"], "stratum": j["stratum"], "label": label, "rust": len(rust), "python": len(mine),
                              "equal": diff is None, "first_diff": diff, "py_s": round(secs, 2)}) + "\n")
        out.flush()
    out.write(json.dumps({"summary": True, "dump": a.dump, "part": a.part, "parts": a.parts, "jobs": len(jobs),
                          "jobs_equal": n_equal, "records": n_recs, "setup_s": round(setup_s, 1),
                          "pythonhashseed": os.environ.get("PYTHONHASHSEED")}) + "\n")
    if a.out:
        out.close()


if __name__ == "__main__":
    main()
