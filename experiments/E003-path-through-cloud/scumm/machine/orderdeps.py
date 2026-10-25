"""E003, D7: does a run's output rest on Python's set order? Two places in model.py read a Python set in its hash
order, which changes with PYTHONHASHSEED (the goldens' setting is random), so no port can match them when the order
shows (PORT-NOTES.md, "Kept apart"):

* ``Machine.getter`` on value sets returns the first unknown of a set of results: the order shows when two or more
  different unknowns are among them (setorder.py counts these alone);
* ``Machine._op_endCutscene``'s ``armed`` (calls on) reads the ``esc:`` keys of a set until the first that is 1. In a
  settle turn each read goes into the turn's read log (``Path.rw``), so which keys are logged rests on the order when
  a key is 1 and another key not yet logged (or a second 1) could come before it.

This runs the model check's counted moves of a run with a Machine that counts both, untraced, on worker processes.

usage (the experiment's venv, under nice): orderdeps.py RUN [--only S1#4,...] [--jobs 4]
"""

import argparse
import collections
import multiprocessing
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
EXP = HERE.parents[1]
sys.path.insert(0, str(EXP))
sys.path.insert(0, str(HERE))

import golden  # noqa: E402,F401  (sets sys.path for the repository's src)
import model  # noqa: E402
import setorder  # noqa: E402


class Machine(setorder.Machine):
    """setorder.py's getter counts, plus endCutscene's ``armed`` reads whose logged keys rest on the set order."""

    def __init__(self, *a, **k):
        super().__init__(*a, **k)
        self.cut_reads, self.cut_hits = 0, collections.Counter()

    def _op_endCutscene(self, p, x, st, oracle):
        if self.calls and p.owner is not None:
            self.cut_reads += 1
            r, w = p.rw[p.owner]
            keys = {k for k in set(oracle.keys("esc:") or []) | set(p.writes) if k.startswith("esc:")}
            vals = {k: p.writes[k] if k in p.writes else oracle.get(k) for k in keys}
            ones = [k for k in keys if vals[k] == 1]
            new = {k for k in keys if k not in w and k not in r}
            if ones and new and not (len(ones) == 1 and new <= set(ones)):
                self.cut_hits[(x.cite(), self.set_label)] += 1
        return super()._op_endCutscene(p, x, st, oracle)


def work(labels):
    import check
    import groundtruth
    features, switches = golden.RUNS[ARGS.run]
    rows = groundtruth.load()
    kinds = check.held_families(rows)
    inputs = check.run_b_inputs(rows)
    m = Machine(features, switches=switches)
    t = time.time()
    for row in rows:
        h, action = check.header(row, m.game, inputs)
        if h["status"] != "counted" or action is None or h["label"] not in labels:
            continue
        m.set_label = h["label"]
        check.run_model(m, action, model.FactOracle(row["pre"], kinds))
    return (dict(m.set_getters), dict(m.set_hits), dict(m.set_moves), m.cut_reads, dict(m.cut_hits), time.time() - t)


ARGS = None


def main() -> None:
    global ARGS
    import check
    import groundtruth
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("run", choices=sorted(golden.RUNS))
    ap.add_argument("--only", default="")
    ap.add_argument("--jobs", type=int, default=4)
    ARGS = ap.parse_args()
    only = set(ARGS.only.split(",")) - {""}
    rows = groundtruth.load()
    inputs = check.run_b_inputs(rows)
    game = model.Game.load()
    labels = []
    for row in rows:
        h, action = check.header(row, game, inputs)
        if h["status"] == "counted" and action is not None and (not only or h["label"] in only):
            labels.append(h["label"])
    # the slow moves first, each on its own worker; the rest spread round robin
    parts = [labels[k::ARGS.jobs * 4] for k in range(ARGS.jobs * 4)]
    getters, hits, moves, reads, cuts, secs = collections.Counter(), collections.Counter(), collections.Counter(), 0, \
        collections.Counter(), 0.0
    with multiprocessing.get_context("fork").Pool(ARGS.jobs) as pool:
        for g, h, mv, r, c, s in pool.imap_unordered(work, [set(p) for p in parts if p]):
            getters.update(g)
            hits.update(h)
            moves.update(mv)
            reads += r
            cuts.update(c)
            secs += s
    print(f"{ARGS.run}: {len(labels)} moves, {secs:.0f} s of workers")
    print(f"  getters on value sets {dict(getters)}; with two or more different unknown results: {dict(hits)}; "
          f"moves {dict(moves)}")
    print(f"  endCutscene in a settle turn: {reads}; whose logged esc: reads rest on the set order: {dict(cuts)}")


if __name__ == "__main__":
    main()
