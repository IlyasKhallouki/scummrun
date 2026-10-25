"""E003, D7: the machine beyond the goldens. Every object entry of the game, run by model.py's Machine from a few
ground-truth states, dumped as golden.py dumps a move (run, path and predict lines, and the trace), so that the Rust
port can be compared on far more code than the 239 moves of the model check reach (scumm/machine/PORT-NOTES.md).

usage (the experiment's venv, under nice):
       wide.py dump --out DIR [--row 0|1|2|3|4|5] [--rows 3,60,120,180,240|all] [--units] [--sentences] [--waits]
                    [--clicks] [--escs] [--arm] [--keep-armed] [--only-units] [--no-objects] [--switches NAME,...]
                    [--budget SECONDS] [--max-mb MB] [--max-items N] [--path-limit N] [--step-limit N] [--plan]
                    [--drop-kinds FAMILY,...] [--set-prefix PREFIX=V,...] [--set KEY=V,...] [--unheld KEY,...]

Writes DIR/cases.json (the ground truth, the fact families, the machine's features, and the cases, in order) and
DIR/python.jsonl.gz: per case a {"kind": "case", ...} line, then golden.py's run, path and predict lines;
DIR/python.trace.jsonl.gz: per case the same case line, then golden.py's trace events.

``--row`` picks the machine: 0 is model.ROW0 (own_entry), 1 adds dispatch and calls (every start runs nested), 2 adds
room_change (a room change runs startScene: the exit and entry code nested, the kills, the room's variables), 3 adds
settle (time passes in rounds after a sentence, and a wait is settle alone; ``Machine.run`` and ``run_entry`` start a path
that does not settle, so their cases at row 3 are row 2's), 4 adds dialog (a click on a verb, ``Machine.run_verb``; the
other entry points run as at row 3), 5 adds esc (the Esc key, ``Machine.run_esc``). ``--switches`` gives the Machine
model.py's switches (``model.SWITCHES``: const_polls, input_after_turns, esc_key_at_pre; cases.json keeps them for
machine-cases). Cases:

* objects: for every object unit, in key order, each verb of its event table, then verb 0 (no such event: the 0xFF
  default when it has one); plus three object numbers with no code (``Machine.run_entry``; with calls on, the entry is
  the first frame of a stack);
* ``--units``: every other unit (global, local, entry, exit) from its first instruction (``Machine.run`` with unknown
  locals, owned False, R flag not known), on the first row only;
* ``--sentences`` (rows 1 to 3): for every object unit, each verb of its event table as a sentence with A the object and
  no B (``Machine.run_sentence``), then, once per object, its first verb with B the next object (a two-object
  sentence) and with B the object itself (dropped, script.cpp:1203-1204);
* ``--waits`` (row 3): the wait action from each state (``Machine.run_wait``);
* ``--clicks`` (row 4): from each state, a left click on every verb shown at pre (its ``verb:`` fact is 1), in number
  order, then on verb 0 (never shown) (``Machine.run_verb``);
* ``--escs`` (row 5): the Esc key from each state (``Machine.run_esc``);
* ``--arm`` (row 5): from each state, per slot live at pre with one wait point W (its ``at:`` key that is 1, in key
  order), the Esc key with that slot's override armed at W and at W + 1 (``"arm": "esc:<slot>:W"`` in the case): the
  state's other ``esc:`` keys leave it, unless ``--keep-armed`` (then two may be armed). A synthetic state per case, to
  reach overrides of every slot kind, overrides that are no instruction, and two armed keys;
* ``--only-units``: the units alone, no object cases; ``--no-objects``: no object entries (sentences or waits alone).

``--rows all`` takes every ground-truth row's state.

``--budget``: a case whose Python run takes longer is dropped (SIGALRM stops it; the machine resets at its next run),
and cases.json then lists only the kept cases, with the dropped ones under "slow". With calls on, merges in a loop
make the lists of assumptions and unknown writes grow at each merge (model.py ``_merge`` adds each branch's own part
again), so a few units run for minutes and dump gigabytes. 0 (the default) keeps every case. ``--max-mb``: a case
whose run, path and predict lines pass this many MB is dropped too (listed under "big"); 0 keeps every size.
A case whose output may rest on Python's set order (``Machine`` below) is dropped too, listed under "hash_order";
``--keep-order-cases`` keeps it in the dump (still listed), to see whether the order shows in its output.
``--max-items N``: a case whose paths hold more than N list entries in all (assumptions, notes, unknown writes, errors,
turns, conflicts, writes) is dropped before it is encoded (listed under "big"): encoding a run of millions of entries
takes minutes outside the budget's timer. ``--plan``: write cases.json with every case and run nothing (machine-cases
DIR --costs then gives the Rust machine's time and size per case, to pick what Python can afford; ``--from-costs
COSTS --max-rust-ms MS`` keeps the cases the Rust machine ran in at most MS milliseconds, with at most --max-items
list entries; the rest are listed under "costly"). Progress goes to stderr every 200 cases.
``--path-limit`` and ``--step-limit`` give the Machine smaller limits than PATH_LIMIT and STEP_LIMIT (model.py's
constructor takes them), so that forks past the limit, with paths waiting at joins, and step-limit ends with calls on
the stack happen often; cases.json keeps them for machine-cases.

Synthetic states, to reach branches the real states do not (both machines get the same oracle, from cases.json):
``--drop-kinds`` takes fact families out of the held ones (``check.held_families``), so their keys read as not held
(an unknown); ``--set-prefix P=V`` gives every key of the state that starts with P the value V, then ``--set K=V``
sets single keys. A key set to 0 leaves the state (the ground truth's states hold no 0 values). ``--unheld K,...``: the
oracle reads these keys as not held, whatever their family (``Unheld``; model.py has no such rule, so this reaches an
unknown VAR_VERB_SCRIPT or VAR_USERPUT without making every variable unknown).

`machine-cases DIR` writes rust.jsonl.gz and rust.trace.jsonl.gz the same way; `golden.py compare-file` compares them.
"""

import argparse
import json
import signal
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
EXP = HERE.parents[1]
sys.path.insert(0, str(EXP))

import golden  # noqa: E402
import model  # noqa: E402
from candidate import frozen  # noqa: E402

ROW_FEATURES = {0: model.ROW0, 1: frozenset({"own_entry", "dispatch", "calls"}),
                2: frozenset({"own_entry", "dispatch", "calls", "room_change"}), 3: model.ROW3, 4: model.ROW4,
                5: model.ROW5}


class Slow(Exception):
    pass


class Machine(golden.TraceMachine):
    """golden.py's TraceMachine that notes when the run's output rests on Python's set order: a getter on value sets
    whose results hold two or more different unknowns. model.py's ``getter`` returns the first unknown of a set there,
    and that order changes with PYTHONHASHSEED (the same case gives either unknown from one process to the next), so
    no port can match it; the Rust takes the first in combination order (PORT-NOTES.md, "Kept apart"). Also
    endCutscene's ``armed`` in a settle turn, when which ``esc:`` keys its reads log rests on the set order (orderdeps.py's
    test; the Rust reads them in key order). Such cases are left out of the dump and listed under "hash_order"."""
    hash_order = False
    _outs = None

    def _op_endCutscene(self, p, x, st, oracle):
        if self.calls and p.owner is not None:
            r, w = p.rw[p.owner]
            keys = {k for k in set(oracle.keys("esc:") or []) | set(p.writes) if k.startswith("esc:")}
            ones = [k for k in keys if (p.writes[k] if k in p.writes else oracle.get(k)) == 1]
            new = {k for k in keys if k not in w and k not in r}
            if ones and new and not (len(ones) == 1 and new <= set(ones)):
                self.hash_order = True
        return super()._op_endCutscene(p, x, st, oracle)

    def getter(self, p, name, args, oracle, x=None):
        outer, self._outs = self._outs, []
        try:
            return super().getter(p, name, args, oracle, x)
        finally:
            if len({v for v in self._outs if isinstance(v, model.Unknown)}) > 1:
                self.hash_order = True
            self._outs = outer

    def _getter_on(self, p, name, args, vals, oracle, x=None):
        v = super()._getter_on(p, name, args, vals, oracle, x)
        if self._outs is not None:
            self._outs.append(v)
        return v


class Unheld(model.FactOracle):
    """A FactOracle that reads some keys as not held (``--unheld``), whatever their family."""

    def __init__(self, facts, kinds, unheld):
        super().__init__(facts, kinds)
        self.unheld = frozenset(unheld)

    def get(self, key: str):
        if key in self.unheld:
            return model.Unknown(model.R_HELD, key)
        return super().get(key)


def _alarm(signum, frame):
    raise Slow()


def wait_points(pre: dict) -> list:
    """(unit name, W) per slot live at pre (its slot fact 1; entry code with an ``at:entry:`` key 1) whose ``at:`` keys
    hold exactly one 1, in key order of the names."""
    names = sorted({k if not k.startswith("objscript:") else "obj:" + k[10:] for k, v in pre.items()
                    if v == 1 and k.startswith(("script:", "local:", "objscript:"))}
                   | ({"entry"} if any(k.startswith("at:entry:") and v == 1 for k, v in pre.items()) else set()))
    out = []
    for n in names:
        ws = [int(k.rsplit(":", 1)[1]) for k, v in pre.items() if k.startswith(f"at:{n}:") and v == 1]
        if len(ws) == 1:
            out.append((n, ws[0]))
    return out


def armed(pre: dict, key: str, keep: bool) -> dict:
    """The state with override ``key`` armed (``--arm``); the other ``esc:`` keys leave it unless ``keep``."""
    out = {k: v for k, v in pre.items() if keep or not k.startswith("esc:")}
    out[key] = 1
    return out


def cases(game, rows, units: bool, sentences: bool, objects: bool = True, waits: bool = False, clicks=None,
          escs: bool = False, arm=None, keep: bool = False) -> list:
    objs = sorted(k[1] for k in game.units if k[0] == "obj")
    out = []
    if units:
        out += [{"row": rows[0], "unit": list(k), "pc": 0} for k in sorted(game.units) if k[0] != "obj"]
    for r in rows:
        if objects:
            for o in objs:
                for v in list(game.units[("obj", o)].entries) + [0]:
                    out.append({"row": r, "obj": o, "verb": v})
            missing = [n for n in range(1, 10000) if ("obj", n) not in game.units][:3]
            out += [{"row": r, "obj": o, "verb": 1} for o in missing]
        if sentences:
            for k, o in enumerate(objs):
                verbs = list(game.units[("obj", o)].entries)
                out += [{"row": r, "sentence": [v, o, None]} for v in verbs]
                if verbs:
                    out.append({"row": r, "sentence": [verbs[0], o, objs[(k + 1) % len(objs)]]})
                    out.append({"row": r, "sentence": [verbs[0], o, o]})
        if waits:
            out.append({"row": r, "wait": True})
        if clicks is not None:
            shown = sorted(int(k[5:]) for k, v in clicks[r].items() if k.startswith("verb:") and v == 1)
            out += [{"row": r, "click": v} for v in shown + [0]]
        if escs:
            out.append({"row": r, "esc": True})
        if arm is not None:
            out += [{"row": r, "esc": True, "arm": f"esc:{n}:{w + d}", "keep": keep}
                    for n, w in wait_points(arm[r]) for d in (0, 1)]
    return out


def perturb(pre: dict, set_prefix: list, sets: list) -> dict:
    """The state with ``--set-prefix`` and ``--set`` applied, in that order (machine-cases does the same)."""
    out = dict(pre)
    for p, v in set_prefix:
        for k in [k for k in out if k.startswith(p)]:
            out[k] = v
    for k, v in sets:
        out[k] = v
    return {k: v for k, v in out.items() if v != 0}


def cmd_dump(a) -> None:
    frozen()
    import check
    import groundtruth
    if a.sentences and a.row < 1:
        raise SystemExit("--sentences needs --row 1 (dispatch)")
    if a.waits and a.row < 3:
        raise SystemExit("--waits needs --row 3 (settle)")
    if a.clicks and a.row < 4:
        raise SystemExit("--clicks needs --row 4 (dialog)")
    if (a.escs or a.arm) and a.row < 5:
        raise SystemExit("--escs and --arm need --row 5 (esc)")
    rows = groundtruth.load()
    kinds = check.held_families(rows)
    drop = set(a.drop_kinds.split(",")) - {""}
    if drop - kinds:
        raise SystemExit(f"--drop-kinds: not held families {sorted(drop - kinds)}")
    kinds -= drop
    pairs = lambda text: [(k, int(v)) for k, v in (x.split("=") for x in text.split(",") if x)]
    set_prefix, sets = pairs(a.set_prefix), pairs(a.set)
    unheld = [k for k in a.unheld.split(",") if k]
    byi = {r["i"]: perturb(r["pre"], set_prefix, sets) for r in rows}
    want = sorted(byi) if a.rows == "all" else [int(x) for x in a.rows.split(",")]
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    features = ROW_FEATURES[a.row]
    switches = frozenset(a.switches.split(",")) - {""}
    m = Machine(features, path_limit=a.path_limit, step_limit=a.step_limit, switches=switches)
    cs = cases(m.game, want, a.units or a.only_units, a.sentences, not (a.only_units or a.no_objects), a.waits,
               byi if a.clicks else None, a.escs, byi if a.arm else None, a.keep_armed)
    meta = {"groundtruth": str(groundtruth.OUT), "kinds": sorted(kinds), "features": sorted(features),
            "switches": sorted(switches), "path_limit": a.path_limit, "step_limit": a.step_limit, "budget": a.budget,
            "max_mb": a.max_mb, "set_prefix": set_prefix, "sets": sets, "unheld": unheld}
    if a.plan:
        (out / "cases.json").write_text(json.dumps({**meta, "cases": cs, "slow": [], "big": [], "hash_order": []}) + "\n")
        print(f"{len(cs)} cases planned", file=sys.stderr)
        return
    costly = []
    if a.from_costs:
        costs = json.loads(Path(a.from_costs).read_text())["cases"]
        if len(costs) != len(cs):
            raise SystemExit(f"--from-costs: {len(costs)} cases, this plan has {len(cs)}")
        ok = [k["ms"] <= a.max_rust_ms and (not a.max_items or k["items"] <= a.max_items) for k in costs]
        costly = [c for c, y in zip(cs, ok) if not y]
        cs = [c for c, y in zip(cs, ok) if y]
    w = golden.GzWriter(out / "python.jsonl.gz")
    t = golden.GzWriter(out / "python.trace.jsonl.gz")
    signal.signal(signal.SIGALRM, _alarm)
    kept, slow, big, hashed, t0 = [], [], [], [], time.perf_counter()
    for n, c in enumerate(cs):
        if n and n % 200 == 0:
            print(f"{n}/{len(cs)} cases, {time.perf_counter() - t0:.0f} s", file=sys.stderr, flush=True)
        state = armed(byi[c["row"]], c["arm"], c["keep"]) if "arm" in c else byi[c["row"]]
        oracle = Unheld(state, kinds, unheld) if unheld else model.FactOracle(state, kinds)
        m.hash_order = False
        try:  # a one-shot timer: one that fires in the inner finally still drops the case
            signal.setitimer(signal.ITIMER_REAL, a.budget)
            try:
                if "unit" in c:
                    run = m.run(oracle, tuple(c["unit"]), c["pc"])
                elif "sentence" in c:
                    run = m.run_sentence(oracle, *c["sentence"])
                elif "wait" in c:
                    run = m.run_wait(oracle)
                elif "click" in c:
                    run = m.run_verb(oracle, c["click"])
                elif "esc" in c:
                    run = m.run_esc(oracle)
                else:
                    run = m.run_entry(oracle, c["obj"], c["verb"])
            finally:
                signal.setitimer(signal.ITIMER_REAL, 0)
        except Slow:
            slow.append(c)
            continue
        if m.hash_order:
            hashed.append(c)
            if not a.keep_order_cases:
                continue
        if a.max_items and sum(len(p.assumptions) + len(p.notes) + len(p.unknown_writes) + len(p.errors) + len(p.turns)
                               + len(p.conflicts) + len(p.writes) for p in run.paths) > a.max_items:
            big.append(c)
            continue
        lines = [golden.line({"kind": "run", "run": golden.run_dict(run)})]
        lines += [golden.line({"kind": "path", "index": i, "path": golden.path_dict(p)}) for i, p in enumerate(run.paths)]
        lines.append(golden.line({"kind": "predict", "predict": golden.enc(model.predict(run, oracle))}))
        if a.max_mb and sum(map(len, lines)) > a.max_mb * 1e6:
            big.append(c)
            continue
        k = len(kept)
        kept.append(c)
        head = golden.line({"kind": "case", "index": k, **c})
        w.write(head)
        t.write(head)
        for text in lines:
            w.write(text)
        for i, e in enumerate(m.events):
            t.write(golden.event_line(i, e))
    info = w.close()
    t.close()
    (out / "cases.json").write_text(json.dumps({**meta, "cases": kept, "slow": slow, "big": big, "hash_order": hashed,
                                                "costly": costly}) + "\n")
    print(f"{len(kept)} cases kept, {len(costly)} left out by the Rust costs, {len(slow)} over the budget, "
          f"{len(big)} over the size, {len(hashed)} resting on set "
          f"order; {info['bytes'] / 1e6:.1f} MB; "
          f"{time.perf_counter() - t0:.0f} s", file=sys.stderr)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("cmd", choices=("dump",))
    ap.add_argument("--out", required=True)
    ap.add_argument("--row", type=int, default=0, choices=sorted(ROW_FEATURES))
    ap.add_argument("--rows", default="3,60,120,180,240")
    ap.add_argument("--units", action="store_true")
    ap.add_argument("--sentences", action="store_true")
    ap.add_argument("--only-units", action="store_true")
    ap.add_argument("--no-objects", action="store_true")
    ap.add_argument("--waits", action="store_true")
    ap.add_argument("--clicks", action="store_true")
    ap.add_argument("--escs", action="store_true")
    ap.add_argument("--arm", action="store_true")
    ap.add_argument("--keep-armed", action="store_true")
    ap.add_argument("--switches", default="")
    ap.add_argument("--keep-order-cases", action="store_true")
    ap.add_argument("--budget", type=float, default=0.0)
    ap.add_argument("--max-mb", type=float, default=0.0)
    ap.add_argument("--max-items", type=int, default=0)
    ap.add_argument("--plan", action="store_true")
    ap.add_argument("--from-costs", default="")
    ap.add_argument("--max-rust-ms", type=float, default=float("inf"))
    ap.add_argument("--path-limit", type=int, default=model.PATH_LIMIT)
    ap.add_argument("--step-limit", type=int, default=model.STEP_LIMIT)
    ap.add_argument("--drop-kinds", default="")
    ap.add_argument("--set-prefix", default="")
    ap.add_argument("--set", default="")
    ap.add_argument("--unheld", default="")
    cmd_dump(ap.parse_args())


if __name__ == "__main__":
    main()
