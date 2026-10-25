"""E003, D7: the Python half of the Rust machine's candidate folders (scumm/machine/PORT-NOTES.md).

usage (machine-run calls it; the experiment's venv, under nice):
       candidate.py prep RUN --out DIR [--only S1#4,...]    check.py's header of every counted move -> jobs.json
       candidate.py check RUN --out DIR [--only S1#4,...]   check.py's own line of every move, on the Rust run

The Rust machine writes every line golden.py takes from model.py (run, path, predict) and the trace. The last line of
a golden move file is check.py's record of the move: its header (``check.header``: ``classify``, which holds the
route's own moves, run B's VIA, and ``run_b_inputs``) and ``check.score_move``'s output. That harness stays Python:

* ``prep`` writes DIR/machine/RUN/jobs.json: the run's features, switches, key (golden.py ``_key``) and fact families
  (``check.held_families``), the ground truth's path, and per counted move its label, row, golden.py's move line
  and the action as check.py's ``run_model`` reads it;
* ``check`` reads each DIR/machine/RUN/<label>.jsonl.gz the machine wrote, rebuilds the ``model.Run`` from its run and
  path lines (``decode``; a path's four undumped fields keep their defaults: check.py reads none of them), scores the
  move with ``check.score_move`` while ``check.run_model`` returns that Run (as golden.py's capture does), and writes
  the file again with check.py's line last. It stops when the decoded run does not encode back to the same lines, when
  the move line is not golden.py's, or when the predict line is not ``model.predict`` on the decoded run.

Nothing here changes model.py, check.py or golden.py; the frozen sources are checked first.
"""

import argparse
import hashlib
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
EXP = HERE.parents[1]
sys.path.insert(0, str(EXP))

import golden  # noqa: E402  (sets sys.path for the repository's src)
import model  # noqa: E402

FROZEN = {"model.py": "cc630446bdbe", "ir.py": "c6ccf01059f1", "facts.py": "c06fc3489018", "actions.py": "ddbd1aaa2207",
          "check.py": "fc1c3a935c4e", "reader.py": "94d495853f07"}


def frozen() -> None:
    bad = {n: hashlib.sha1((EXP / n).read_bytes()).hexdigest()[:12] for n in FROZEN}
    bad = {n: h for n, h in bad.items() if h != FROZEN[n]}
    if bad:
        raise SystemExit(f"frozen sources changed: {bad} (expected {FROZEN}); stopping")


def setup(a):
    import check
    import groundtruth
    features, switches = golden.RUNS[a.run]
    rows = groundtruth.load()
    kinds = check.held_families(rows)
    inputs = check.run_b_inputs(rows)
    only = {x.strip() for x in a.only.split(",")} if a.only else None
    folder = Path(a.out) / "machine" / a.run
    return check, groundtruth, features, switches, rows, kinds, inputs, only, folder


def head_of(a, m: dict, features, switches, key: str) -> dict:
    """golden.py ``_score_job``'s move line head."""
    return {"run": a.run, "features": sorted(features), "switches": sorted(switches), "i": m["i"], "label": m["label"],
            "name": m["name"], "status": m["status"], "reason": m["reason"], "action_kind": m["kind"],
            "action": m["action"], "key": key}


def cmd_prep(a) -> None:
    frozen()
    check, groundtruth, features, switches, rows, kinds, inputs, only, folder = setup(a)
    folder.mkdir(parents=True, exist_ok=True)
    game = model.Game.load()
    key = golden._key(a.run, features, switches, check.gt_digest(), True)
    moves = []
    for row in rows:
        m, action = check.header(row, game, inputs)
        if m["status"] != "counted" or (only and m["label"] not in only):
            continue
        act = None if action is None else {"kind": action.kind, "verb": action.verb, "a": action.a, "b": action.b,
                                           "walk": action.walk}
        moves.append({"label": m["label"], "i": m["i"], "move": head_of(a, m, features, switches, key),
                      "machine_action": act})
    if only and len(moves) != len(only):
        raise SystemExit(f"--only: {sorted(only - {m['label'] for m in moves})} are not counted moves")
    jobs = {"run": a.run, "features": sorted(features), "switches": sorted(switches), "key": key,
            "kinds": sorted(kinds), "groundtruth": str(groundtruth.OUT), "moves": moves}
    (folder / "jobs.json").write_text(json.dumps(jobs, indent=1) + "\n")


# --- decoding golden.py's canonical encoding back into model.py's objects ---------------------------------------------

def dec(x):
    if isinstance(x, list):
        return [dec(v) for v in x]
    if isinstance(x, dict):
        if len(x) == 1:
            (k, v), = x.items()
            if k == "$tuple":
                return tuple(dec(e) for e in v)
            if k == "$set":
                return {dec(e) for e in v}
            if k == "$map":
                return {dec(kk): dec(vv) for kk, vv in v}
            if k == "$Unknown":
                vals = v["values"]
                return model.Unknown(v["reason"], v["what"], None if vals is None else frozenset(vals["$set"]))
            if k == "$Frame":
                return model.Frame(**{f: dec(e) for f, e in v.items()})
            if k == "$Call":
                return model.Call(**{f: dec(e) for f, e in v.items()})
            if k.startswith("$"):
                raise SystemExit(f"no decoding for {k}")
        return {k: dec(v) for k, v in x.items()}
    return x


def decode(lines: list[dict]) -> model.Run | None:
    run = next((d["run"] for d in lines if d["kind"] == "run"), None)
    if run is None:
        return None
    paths = []
    for d in lines:
        if d["kind"] != "path":
            continue
        f = dict(d["path"])
        f.pop("history_len")
        f.pop("tid")
        slots = f.pop("slots")
        p = model.Path(**{k: dec(v) for k, v in f.items()})
        p.slots = {n: dec(fr) for n, fr in slots}
        paths.append((p, d))
    if len(paths) != run["n_paths"]:
        raise SystemExit(f"{len(paths)} path lines, n_paths {run['n_paths']}")
    r = model.Run([p for p, _ in paths], run["start"], run["limit"], dec(run["unit"]), run["merges"],
                  dec(run["value_caps"]), run["settle_merges"], dec(run["pre_slots"]), run["value_merges"],
                  dec(run["key_routes"]))
    for k, (p, d) in enumerate(paths):  # the decoded path encodes back to the candidate's line
        back = golden.path_dict(p)
        back["history_len"], back["tid"] = d["path"]["history_len"], d["path"]["tid"]
        if golden.line({"kind": "path", "index": k, "path": back}) != golden.line(d):
            raise SystemExit(f"path {k} does not decode back to its line")
    if golden.line({"kind": "run", "run": golden.run_dict(r)}) != golden.line({"kind": "run", "run": run}):
        raise SystemExit("the run line does not decode back")
    return r


def cmd_check(a) -> None:
    frozen()
    check, groundtruth, features, switches, rows, kinds, inputs, only, folder = setup(a)
    jobs = json.loads((folder / "jobs.json").read_text())
    machine = model.Machine(features, switches=switches)
    nonfacts = model.non_fact_vars(machine.game)
    byi = {r["i"]: r for r in rows}
    real = check.run_model
    n = 0
    try:
        for mv in jobs["moves"]:
            if only and mv["label"] not in only:
                continue
            row = byi[mv["i"]]
            m, action = check.header(row, machine.game, inputs)
            path = folder / f"{mv['label']}.jsonl.gz"
            texts = [t for t in golden.open_lines(path) if json.loads(t)["kind"] != "check"]
            lines = [json.loads(t) for t in texts]
            run = decode(lines)
            cap = {}

            def run_model(machine_, action_, oracle, run=run, cap=cap):
                cap["oracle"] = oracle
                if run is None:
                    raise SystemExit(f"{mv['label']}: check.py runs the machine, but the candidate has no run line")
                return run

            check.run_model = run_model
            out = check.score_move(row, action, machine, nonfacts, kinds)
            if run is not None and "oracle" not in cap:
                raise SystemExit(f"{mv['label']}: the candidate has a run, but check.py does not run the machine")
            m |= out
            chk_line = json.dumps(m, default=str)
            want = golden.line({"kind": "move", "move": golden.enc(head_of(a, m, features, switches, jobs["key"]))})
            if texts[0] != want:
                raise SystemExit(f"{mv['label']}: the move line is not golden.py's:\n{texts[0]}\n{want}")
            if run is not None:
                pred = golden.line({"kind": "predict", "predict": golden.enc(model.predict(run, cap["oracle"]))})
                if texts[-1] != pred:
                    raise SystemExit(f"{mv['label']}: the predict line is not model.predict on the run")
            w = golden.GzWriter(path)
            for t in texts:
                w.write(t)
            w.write(golden.line({"kind": "check", "record": golden.enc(m), "line": chk_line}))
            w.close()
            n += 1
    finally:
        check.run_model = real
    print(f"[{a.run}] check lines: {n} moves", file=sys.stderr)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("cmd", choices=("prep", "check"))
    ap.add_argument("run", choices=golden.ORDER)
    ap.add_argument("--out", required=True)
    ap.add_argument("--only", default="")
    a = ap.parse_args()
    (cmd_prep if a.cmd == "prep" else cmd_check)(a)


if __name__ == "__main__":
    main()
