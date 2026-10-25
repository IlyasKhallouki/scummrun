"""E003, D7: single machine runs of the reader, dumped whole, to find where the Rust machine half (scumm/reader's
``machine.rs``) first differs from reader.py. An instrument: it imports reader.py as it is and changes nothing.

usage (from this folder, with ../../.venv/bin/python, under nice 10):
  reader_run_dump.py NAME --runs N[,N...] [--out FILE]

NAME is room33 or kitchen; N is a run's place in ``out/reader-port/capture-NAME.jsonl.gz`` (``reader-port runs NAME``
prints it as "run N"). Each chosen run is made again from the captured inputs (pins, precondition, what) on a fresh
Reader of the scope, and written as one line in golden.py's canonical encoding: {"run", "label", "pass", "limit",
"value_caps", "first_fork", "rd", "paths"}, each path with every field the Rust dump has (end, end_at, sym, steps,
pre, writes, kept, assumptions, notes, unknown_writes, errors, unmodelled, turns), all of them, not the last five.
``reader-port dump-runs NAME --runs ...`` writes the same lines from the Rust machine half; compare the two with
``golden.py compare-file``.

The fresh Reader has the first pass's domains: a run of the second pass (``pass`` 2 in the capture) is made with them
too, and a warning says so (the Rust dump does the same, so the two still compare).
"""

import argparse
import collections
import gzip
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / "src"))

import golden  # noqa: E402
from golden import enc  # noqa: E402

OUT = ROOT / "out" / "reader-port"


def dec(x):
    """golden.py's canonical encoding back into Python values (the dataclasses reader.py and model.py define)."""
    import domains as D  # noqa: F401
    import model as M
    import reader as R
    if isinstance(x, list):
        return [dec(v) for v in x]
    if not isinstance(x, dict):
        return x
    if "$tuple" in x:
        return tuple(dec(v) for v in x["$tuple"])
    if "$set" in x:
        return frozenset(dec(v) for v in x["$set"])
    if "$map" in x:
        return {dec(k): dec(v) for k, v in x["$map"]}
    keys = [k for k in x if k.startswith("$")]
    if keys:
        name = keys[0][1:]
        cls = {"Unknown": M.Unknown, "Cond": R.Cond, "Copy": R.Copy, "Event": R.Event}[name]
        fields = {k: dec(v) for k, v in x[keys[0]].items()}
        if name == "Event":
            fields["locals"] = dict(fields["locals"])
        return cls(**fields)
    return {k: dec(v) for k, v in x.items()}


def path_line(p, R) -> dict:
    d = {"end": p.end, "end_at": p.end_at, "sym": isinstance(p, R.SymPath), "steps": p.steps,
         "writes": enc(p.writes), "assumptions": enc(p.assumptions), "notes": enc(p.notes),
         "unknown_writes": enc(p.unknown_writes), "errors": enc(p.errors), "unmodelled": enc(p.unmodelled),
         "turns": enc(p.turns)}
    if isinstance(p, R.SymPath):
        d.update(pre=enc(p.pre), kept=enc(p.kept))
    return d


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("name", choices=("room33", "kitchen"))
    ap.add_argument("--runs", required=True)
    ap.add_argument("--out")
    a = ap.parse_args()
    import domains as D
    import model as M
    import reader as R
    import reader_port_oracle as O
    want = sorted(int(n) for n in a.runs.split(","))
    lines = {}
    with gzip.open(OUT / f"capture-{a.name}.jsonl.gz", "rt") as f:
        for i, ln in enumerate(f):
            if i in want:
                lines[i] = json.loads(ln)
            if i >= want[-1]:
                break
    if a.name == "room33":
        rooms, pre = golden.ROOM33, O.seed1()
    else:
        rooms, pre = golden.KITCHEN_ROOMS, O.kitchen_pre()
    game = M.Game.load()
    rd = R.Reader(R.Scope.of(game, rooms, pre), D.build(pre), game)
    out = open(a.out, "w") if a.out else sys.stdout
    for i in want:
        c = lines[i]
        if c["pass"] != 1:
            print(f"run {i} is of pass {c['pass']}: made with the first pass's domains", file=sys.stderr)
        pins, pre_, what = dec(c["pins"]), dec(c["pre"]), dec(c["what"])
        before = {n: collections.Counter(getattr(rd, n)) for n in ("counts", "forks", "fork_keys")}
        rd.first_fork = None
        run = getattr(rd, what[0])(pins, pre_, *what[1:])
        diff = {n: {k: v - before[n].get(k, 0) for k, v in getattr(rd, n).items()
                    if k not in before[n] or v != before[n][k]} for n in before}
        line = {"run": i, "label": c["label"], "pass": c["pass"], "limit": run.limit,
                "value_caps": sum(run.value_caps.values()), "first_fork": rd.first_fork,
                "rd": {n: {k: v for k, v in d.items() if v} for n, d in diff.items()},
                "paths": [path_line(p, R) for p in run.paths]}
        out.write(golden._text(line) + "\n")
    if a.out:
        out.close()


if __name__ == "__main__":
    main()
