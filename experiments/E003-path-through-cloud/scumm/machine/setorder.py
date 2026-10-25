"""E003, D7: does a run's output rest on Python's set order? model.py's ``Machine.getter`` on value sets returns the
first unknown of a set of results, and that order changes with PYTHONHASHSEED when two or more different unknowns are
among them; no port can match it (PORT-NOTES.md, "Kept apart"). This runs the model check's counted moves of a run
with a Machine that counts such getters, untraced.

usage (the experiment's venv, under nice): setorder.py RUN [--only S1#4,...]
"""

import argparse
import collections
import sys
from pathlib import Path

EXP = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(EXP))

import golden  # noqa: E402
import model  # noqa: E402


class Machine(model.Machine):
    """Counts getters on value sets (``set_getters``) and those whose results hold two or more different unknowns
    (``set_hits``, by getter and cite; ``set_moves``, by move); nested getters each keep their own results. (The
    names stay clear of model.Machine's own attributes: ``calls`` there is a feature flag.)"""
    _outs = None

    def __init__(self, *a, **k):
        super().__init__(*a, **k)
        self.set_getters, self.set_hits, self.set_moves = collections.Counter(), collections.Counter(), collections.Counter()
        self.set_label = ""

    def getter(self, p, name, args, oracle, x=None):
        outer, self._outs = self._outs, []
        try:
            return super().getter(p, name, args, oracle, x)
        finally:
            if len(self._outs) > 1:
                self.set_getters[name] += 1
            if len({v for v in self._outs if isinstance(v, model.Unknown)}) > 1:
                self.set_hits[(name, x.cite() if x is not None else "")] += 1
                self.set_moves[self.set_label] += 1
            self._outs = outer

    def _getter_on(self, p, name, args, vals, oracle, x=None):
        v = super()._getter_on(p, name, args, vals, oracle, x)
        if self._outs is not None:
            self._outs.append(v)
        return v


def main() -> None:
    import check
    import groundtruth
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("run", choices=sorted(golden.RUNS))
    ap.add_argument("--only", default="")
    a = ap.parse_args()
    only = set(a.only.split(",")) - {""}
    features, switches = golden.RUNS[a.run]
    rows = groundtruth.load()
    kinds = check.held_families(rows)
    inputs = check.run_b_inputs(rows)
    m = Machine(features, switches=switches)
    for row in rows:
        h, action = check.header(row, m.game, inputs)
        if h["status"] != "counted" or action is None or (only and h["label"] not in only):
            continue
        m.set_label = h["label"]
        check.run_model(m, action, model.FactOracle(row["pre"], kinds))
    print(f"{a.run}: getters on value sets {dict(m.set_getters)}; with two or more different unknown results: "
          f"{dict(m.set_hits)}; moves {dict(m.set_moves)}")


if __name__ == "__main__":
    main()
