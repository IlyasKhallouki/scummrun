"""E004 plan step B1: trace validation with the engine's explanation (trace-validation.md).

usage (from this folder, with ../../.venv/bin/python, under nice):
       trace_check.py check-off --new BIN [--old BIN] [--frames N] [--results]

An instrument: nothing the system runs imports it. It names no game number; the moves, frames and units it reports come
from the runs it reads.

**check-off** (trace-validation.md, Lemma O): three engines play run B's recorded inputs on seed 1 from boot in
lockstep, one frame at a time: the shared build (``--old``), the new build with the bridge's ``segment`` and
``explain`` off, and the new build with ``segment`` on from the first stop and ``explain`` armed for every run of every
frame (each frame's runs and log are read and cleared). At every frame the stop status and the full state dump (with
SPEEDRUN_DUMP_LOCALS and SPEEDRUN_DUMP_EXACT) must be equal across the three; at the end the three trace.jsonl files.
It also counts the runs and log entries the third engine gave, the buffers' overflow counts and Lemma S's premise
(``violations``: opcodes whose ``_currentScript`` was not their run's slot).
"""

import argparse
import dataclasses
import hashlib
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import facts as F  # noqa: E402
import fork_exact as FX  # noqa: E402
import ir  # noqa: E402
import model as M  # noqa: E402
import trace_explain as TE  # noqa: E402
from speedrun import paths  # noqa: E402
from speedrun.control import Controller  # noqa: E402

RESULTS = HERE / "results"
OUT = paths.OUT_DIR / "E004-trace"
SHARED_BIN = Path("/var/tmp/fast-mi/svm-window/build/scummvm/scummvm")


def say(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr, flush=True)


def sha12(path: Path) -> str:
    return hashlib.sha1(Path(path).read_bytes()).hexdigest()[:12]


# --- Lemma O: the controls observe only ------------------------------------------------------------------------------

def cmd_check_off(a) -> dict:
    out = a.out / "check-off"
    inputs = FX.inputs_file(out)
    old, new = Path(a.old), Path(a.new)
    doc = {"old": str(old), "old_sha1": sha12(old), "new": str(new), "new_sha1": sha12(new)}
    engines = {}
    t0 = time.monotonic()
    n = status_diff = off_diff = on_diff = 0
    runs = entries = dropped_runs = dropped_log = violations = 0
    diffs = []
    try:
        for tag, binary in (("old", old), ("new-off", new), ("new-on", new)):
            paths.SCUMMVM_BIN = binary
            c = Controller(FX.config(out / tag, inputs, dump_locals=True, dump_exact=True), timeout_s=600)
            c.start()
            engines[tag] = c
        first = {t: c.root.status.get("frame") for t, c in engines.items()}
        if len(set(first.values())) != 1:
            raise SystemExit(f"first stops differ: {first}")
        frame = first["old"]
        on = engines["new-on"].root
        on.segment(True)
        on.explain(frame + 1, frame + a.frames)
        doc["first_frame"] = frame
        end = frame + a.frames
        while frame < end:
            frame += 1
            st = {t: c.root.run(frame=frame) for t, c in engines.items()}
            if any("end" in s for s in st.values()):
                doc["ended"] = {t: s.get("end", {}).get("reason") for t, s in st.items()}
                break
            n += 1
            r = on.runs()
            lg = on.explain_log()
            runs += len(r["runs"])
            entries += len(lg["explain"])
            dropped_runs += r["dropped"]
            dropped_log += lg["dropped"]
            violations = max(violations, r["violations"], lg["violations"])
            if not (st["old"] == st["new-off"] == st["new-on"]):
                status_diff += 1
                if len(diffs) < a.keep:
                    diffs.append({"frame": frame, "what": "status", "where": FX.first_diff(st["old"], st["new-on"])})
            s_old, s_off, s_on = (engines[t].root.state() for t in ("old", "new-off", "new-on"))
            if FX.text(s_old) != FX.text(s_off):
                off_diff += 1
                if len(diffs) < a.keep:
                    diffs.append({"frame": frame, "what": "controls off", "where": FX.first_diff(s_old, s_off)})
            if FX.text(s_on) != FX.text(s_old):
                on_diff += 1
                if len(diffs) < a.keep:
                    diffs.append({"frame": frame, "what": "controls on", "where": FX.first_diff(s_old, s_on)})
            if n % 500 == 0:
                say(f"check-off: frame {frame}, differences {status_diff}/{off_diff}/{on_diff}, runs {runs}, "
                    f"log entries {entries}")
    finally:
        for c in engines.values():
            c.close()
    traces = {t: sha12(out / t / "trace.jsonl") for t in engines}
    doc.update(frames=n, to_frame=frame, status_differ=status_diff, off_differ=off_diff, on_differ=on_diff,
               trace_sha1=traces, traces_equal=len(set(traces.values())) == 1, runs=runs, log_entries=entries,
               runs_dropped=dropped_runs, log_dropped=dropped_log, violations=violations, differences=diffs,
               seconds=round(time.monotonic() - t0, 1))
    return doc


def check_off_log(doc: dict) -> list:
    lines = [f"B1, Lemma O: the bridge's segment and explain observe only. Shared build {doc['old']} (sha1 "
             f"{doc['old_sha1']}), new build {doc['new']} (sha1 {doc['new_sha1']}); run B's inputs from boot on seed 1, "
             f"every frame from {doc['first_frame']} to {doc['to_frame']}; SPEEDRUN_DUMP_LOCALS and SPEEDRUN_DUMP_EXACT "
             f"on in all three",
             f"{doc['frames']} frames compared; stop status differs on {doc['status_differ']}; the full dump differs, "
             f"new build with the controls off against the shared build, on {doc['off_differ']}; with segment on and "
             f"explain on every run of every frame, on {doc['on_differ']}; trace.jsonl equal: {doc['traces_equal']} "
             f"({doc['trace_sha1']})",
             f"the third engine gave {doc['runs']} slot runs ({doc['runs_dropped']} dropped) and {doc['log_entries']} "
             f"explain entries ({doc['log_dropped']} dropped); opcodes off their run's slot (Lemma S's premise): "
             f"{doc['violations']}; {doc['seconds']} s"]
    for d in doc["differences"]:
        lines.append(f"  differs at frame {d['frame']} ({d['what']}): {d['where']}")
    return lines


# --- run B on seed 1: the ground truth and the model check -------------------------------------------------------

ANY_SEED = 836557856  # plan.md: the Any% seed, never used here
ROW5 = "own_entry,dispatch,calls,room_change,settle,dialog,esc"
FR1 = "actor_rooms,const_polls,draw_clears,input_after_turns,value_sets"  # fix round 1's five, as fr1-merge's row 5


def _truth_at(path: Path) -> None:
    """Point groundtruth.py's file at ``path`` (its ``load`` default too, which Python binds at definition)."""
    import groundtruth
    groundtruth.OUT = path
    groundtruth.load.__defaults__ = (path,)


def cmd_runb_truth(a) -> None:
    """groundtruth.py's ground truth of run B's Part I moves (the same moves, boundaries and dumps), with run B's
    recorded inputs played on ``--seed`` in place of run B's own seed."""
    import groundtruth
    if a.seed == ANY_SEED:
        raise SystemExit("the Any% seed is not used (plan.md)")
    a.out.mkdir(parents=True, exist_ok=True)
    paths.SCUMMVM_BIN = Path(a.bin)
    _truth_at(a.out / f"groundtruth-part1-seed{a.seed}.jsonl")
    groundtruth.SEED = a.seed
    groundtruth.ENGINE_DIR = a.out / "engine"
    sys.argv = [sys.argv[0]]
    groundtruth.main()


def cmd_runb_check(a) -> None:
    """check.py's model check (check_rust.py: the Rust machine) on ``--truth``, row 5 with fix round 1's switches.
    On another seed than run B's, run B's recorded inputs can stop doing what they did in run B; check.py stops on a
    move run B played by search whose engine evidence (the inventory slot, the sentence at done) is not the one it
    expects. Here such a move is excluded instead, with check.py's own message as the reason."""
    _truth_at(Path(a.truth).resolve())
    import check
    import check_rust
    classify = check.classify

    def lenient(row, game):
        try:
            return classify(row, game)
        except SystemExit as e:
            return "excluded", None, f"another seed: {e}"
    check.classify = lenient
    sys.argv = [str(HERE / "check_rust.py"), "--out", str(a.out), "--bin", str(a.machine), "--features", ROW5,
                "--switches", a.switches, "--jobs", str(a.jobs)]
    check_rust.main()


# --- run B's misses explained (sections 4 and 5) ------------------------------------------------------------------

def runb_same_mismatch(log: Path) -> set:
    """The moves whose engine sentence at done is not the scored action's in run B's own ground truth (the committed
    row 5 log's list): the same evidence on another seed is not a sign that the inputs went elsewhere."""
    out = set()
    if log.exists():
        for ln in log.read_text().splitlines():
            ln = ln.strip()
            if ln[:2] in ("S1", "S2", "S3") and ("no new sentence by done" in ln or "another sentence ran" in ln):
                out.add(ln.split(" ")[0])
    return out


def synced_prefix(rows: list, checked: list, results_rooms: dict, same: set, known_room: set) -> tuple:
    """The rows before the first move where run B's inputs on this seed stop doing what they did in run B: the room at
    done or at the next move's start is not run B's (results.jsonl; ``known_room``: moves where run B's own ground
    truth disagrees too), a dialog line run B clicked is not shown, or the sentence the engine ran by done is not the
    scored action's where it was in run B. Returns (n, the first such move and why)."""
    for i, (r, m) in enumerate(zip(rows, checked)):
        lab = r["name"].split(" ")[0]
        room = results_rooms.get(r["name"])
        if room is not None and r["room_done"] != room:
            return i, (lab, f"room at done {r['room_done']}, run B's {room}")
        if room is not None and r["room_post"] != room and lab not in known_room:
            return i, (lab, f"room at the next move's start {r['room_post']}, run B's {room}")
        if m.get("kind") == "none" and "clicked_verb" in m.get("reason", ""):
            return i, (lab, "a dialog line run B clicked is not shown")
        es = m.get("engine_sentence")
        if es is not None and not es.get("matches") and lab not in same:
            return i, (lab, "the engine's sentence by done is not the scored action's")
        if m.get("status") == "counted" and m.get("kind") not in ("none",) and not m.get("accepted") and \
                m.get("why_not") != "no action kind":
            return i, (lab, f"not accepted: {m.get('why_not')}")
    return len(rows), None


class RunB:
    """Run B's recorded inputs on seed 1 in one root engine; a child per question at a move's frames."""

    def __init__(self, binary: Path, work: Path, last_frame: int):
        import replay
        import replay_kitchen
        paths.SCUMMVM_BIN = binary
        events = [e for e in replay_kitchen.prefix_events() if e["frame"] <= last_frame]
        work.mkdir(parents=True, exist_ok=True)
        self.cfg = replay_kitchen.cfg_of(work, replay.write_events(work / "inputs.jsonl", events), seed=1)
        self.cfg = dataclasses.replace(self.cfg, dump_locals=True)
        self.c = Controller(self.cfg, timeout_s=600)
        self.c.start()
        self.root = self.c.root
        self.stats = {"forks": 0, "frames": 0, "explain_entries": 0, "explain_runs": 0}

    def at(self, frame: int) -> None:
        if self.root.status["frame"] < frame:
            self.root.run(frame=frame)
        assert self.root.status["frame"] == frame, self.root.status

    def fork(self, b=None):
        self.stats["forks"] += 1
        return self.c.fork(b or self.root)

    def segment(self, lo: int, hi: int) -> TE.Runs:
        """Every slot run from D_lo to D_hi (the root stands at lo)."""
        ch = self.fork()
        ch.segment(True)
        ch.run(frame=hi)
        out = TE.Runs(ch.runs()["runs"])
        self.stats["frames"] += hi - lo
        ch.quit()
        return out

    def explain(self, lo: int, first: int, last: int, units: list | None, dumps: tuple = ()) -> tuple:
        """The explain log of ``units`` in frames first..last, from a child of the root at lo; and the state dumps
        at the decision points ``dumps`` on the way. The child records the runs from lo on as ``segment`` does, so
        its run numbers (``seq``) are those of ``segment``'s child: the same engine from the same stop."""
        ch = self.fork()
        ch.segment(True)
        got = {}
        for d in sorted(dumps):
            if d > ch.status["frame"]:
                ch.run(frame=d)
            got[d] = ch.state()
        if first - 1 > ch.status["frame"]:
            ch.run(frame=first - 1)
        if first - 1 not in got:
            got[first - 1] = ch.state()
        ch.explain(first, last, units)
        ch.run(frame=last)
        lg = ch.explain_log()
        ch.quit()
        self.stats["frames"] += last - lo
        self.stats["explain_entries"] += len(lg["explain"])
        ex = TE.Explained(lg["explain"])
        self.stats["explain_runs"] += len(ex.runs)
        return ex, got

    def close(self) -> None:
        self.c.close()


WHERE_OF = {"script": F.WIO_GLOBAL, "local": F.WIO_LOCAL}


def unit_where(key: tuple) -> list:
    """The (where, number) pairs a slot of IR unit ``key`` can have."""
    if key[0] == "global":
        return [(F.WIO_GLOBAL, key[1])]
    if key[0] == "local":
        return [(F.WIO_LOCAL, key[2])]
    if key[0] == "entry":
        return [(F.WIO_ROOM, F.ENCD)]
    if key[0] == "exit":
        return [(F.WIO_ROOM, F.EXCD)]
    return [(F.WIO_ROOM, key[1]), (F.WIO_INVENTORY, key[1]), (F.WIO_FLOBJECT, key[1])]


def runs_of_key(runs: TE.Runs, key: tuple) -> list:
    """The runs of unit ``key``: a local's, entry's or exit's only in its own room resource."""
    room = key[1] if key[0] in ("local", "entry", "exit") else None
    out = [r for w, n in unit_where(key) for r in runs.of_unit(w, n) if room is None or r.room == room]
    return sorted(out, key=lambda r: r.seq)


def slot_targets(code: TE.Code, key: tuple, fact: str, value: int) -> frozenset | None:
    """For a slot fact of unit ``key``: the instructions whose execution makes the predicted value (None: no rule)."""
    kind = fact.split(":")[0]
    if kind == "at":
        w = int(fact.rsplit(":", 1)[1])
        here = code.yield_sites(key, w)
        if value == 1:
            return here
        u = code.unit(key)
        others = {j for j, x in enumerate(u.ins) if x.yields is not None or isinstance(x.st, ir.Start)} - here
        return frozenset(others | code.end_sites(key))
    if kind in ("local", "script", "objscript") and value == 0:
        return code.end_sites(key)
    return None


class Explainer:
    """Sections 4 and 5 for one move: its model run, its engine window, and the causes of its misses."""

    def __init__(self, code: TE.Code, eng: RunB, row: dict, run, kinds, depth: int = 3):
        self.code, self.eng, self.row, self.run = code, eng, row, run
        self.pre_f, self.done_f = row["pre_frame"], row["done_frame"]
        self.room = code.game.resource(row["pre"].get("room", 0))
        self.depth = depth
        eng.at(self.pre_f)
        self.runs = eng.segment(self.pre_f, self.done_f) if self.done_f > self.pre_f else TE.Runs()
        self.pre_dump = None
        self.model_units = []
        for p in M.feasible(run) if run is not None else []:
            for name, _how in p.turns:
                k = code.key_of_name(name, self.room)
                if k is not None and k not in self.model_units:
                    self.model_units.append(k)
        self.engine_units = []
        for r in self.runs.runs:
            k = code.key(r.where, r.number, r.room)
            if k is not None and k not in self.engine_units:
                self.engine_units.append(k)

    def dump_pre(self) -> dict:
        if self.pre_dump is None:
            ch = self.eng.fork()
            self.pre_dump = ch.state()
            ch.quit()
        return self.pre_dump

    def explain(self, fact: str, value: int) -> dict:
        pre, done = self.row["pre"], self.row["done"]
        p, u = pre.get(fact, 0), done.get(fact, 0)
        base = {"move": self.row["name"].split(" ")[0], "fact": fact, "predicted": value, "engine": u, "pre": p,
                "pre_frame": self.pre_f, "done_frame": self.done_f}
        if self.done_f <= self.pre_f:
            return base | {"kind": "no frame", "cause": None}
        if u not in (p, value) and not fact.startswith(TE.SLOT_KEYS):
            return base | self.wrote_other(fact, p, value)
        if fact.startswith(TE.SLOT_KEYS):
            name = M.slot_name(fact.split(":", 1)[1].rsplit(":", 1)[0] if fact.startswith(("at:", "esc:")) else
                               fact.split(":", 1)[1] if fact.startswith("frozen:") else fact)
            droom = self.code.game.resource(done.get("room", 0))
            rooms = [droom] + ([self.room] if self.room != droom else [])
            keys = [k for k in (self.code.key_of_name(name, r) for r in rooms) if k is not None]
            if not keys:
                return base | {"kind": "unit not in the IR", "cause": None, "unit": name}
            key = next((k for k in keys if runs_of_key(self.runs, k)), keys[0])
            kind = fact.split(":")[0]
            if kind in ("local", "script", "objscript") and value == 1:
                return base | self.starters(key, 0)
            targets = slot_targets(self.code, key, fact, value)
            if targets is None:
                return base | {"kind": "no rule for this slot fact", "cause": None, "unit": name}
            return base | self.follow(key, targets, 0, fact, value)
        # the candidate writers: units with a write site, those the model ran and the engine ran first, direct
        # sites (not through an index, a constant object) before the rest
        # the model's last writer first: the last unit in a feasible path's turns with a direct write site (the
        # value at the end is the last write's)
        last = []
        for p in (M.feasible(self.run) if self.run is not None else []):
            for name, _how in reversed(p.turns):
                k = self.code.key_of_name(name, self.room)
                if k is not None and self.code.write_sites(k, fact, value, direct=True):
                    if k not in last:
                        last.append(k)
                    break
        both = [k for k in self.model_units if k in self.engine_units and k not in last]
        order = last + both + [k for k in self.model_units if k not in both and k not in last] + \
            [k for k in self.engine_units if k not in self.model_units and k not in last]
        cands = [(k, True) for k in order if self.code.write_sites(k, fact, value, direct=True)] + \
                [(k, False) for k in order if not self.code.write_sites(k, fact, value, direct=True)
                 and self.code.write_sites(k, fact, value)]
        if not cands:
            return base | {"kind": "no unit writes it", "cause": None}
        results = []
        for key, direct in cands[:4]:
            res = self.follow(key, self.code.write_sites(key, fact, value, direct=direct), 0, fact, value)
            res["direct"] = direct
            results.append(res)
        found = [r for r in results if r.get("cause")]
        best = found[0] if found else results[0]
        best["model_last_writer"] = bool(last) and cands[results.index(best)][0] in last
        alts = [{k: r.get(k) for k in ("unit", "kind", "offset", "opcode", "class", "direct")}
                for r in results if r is not best]
        return base | best | {"candidates": len(cands), "alternatives": alts}

    def wrote_other(self, fact: str, p: int, value: int) -> dict:
        """The engine changed the fact to another value: Lemma L finds the frame, explain finds the writer."""
        eng = self.eng
        stats = {}
        k = TE.locate(eng.fork, eng.root, self.pre_f, self.done_f,
                      lambda ch: F.facts_of(ch.state()).get(fact, 0) in (p, value), stats)
        ex, dumps = eng.explain(self.pre_f, k, k, None)
        dump = dumps.get(k - 1)
        kind, _, rest = fact.partition(":")
        for lg in ex.runs.values():
            key = self.code.key(lg.where, lg.number, lg.room)
            if key is None:
                continue
            steps = TE.steps_of(self.code, key, [lg])
            sites = self.code.write_sites(key, fact)
            for n, s in enumerate(steps):
                want = int(rest) if kind == "var" else TE.BIT_BIT | int(rest) if kind == "bit" else None
                hit = any(v == want for v, _ in s.op.writes) if want is not None else (s.i is not None and s.i in sites)
                if not hit:
                    continue
                x = self.code.unit(key).ins[s.i] if s.i is not None else None
                vals = TE.slice_test(self.code, key, steps, n, dump) if x is not None else []
                return {"kind": "wrote another value", "frame": k, "bisection": stats, "unit": M.unit_name(key),
                        "room": lg.room, "offset": s.op.code, "opcode": ir.head(x.st) if x else None,
                        "values": [v.as_dict() for v in vals], "class": TE.cause_class(vals) if vals else "semantics",
                        "cause": True, "model_ran_unit": key in self.model_units}
        return {"kind": "wrote another value", "frame": k, "bisection": stats, "cause": None,
                "why": "no logged run of that frame writes it"}

    def follow(self, key: tuple, targets: frozenset, depth: int, fact: str | None = None, value=None) -> dict:
        """Lemma X on unit ``key``'s runs in the window, toward ``targets`` (the write sites of ``fact``, or the
        starts of a unit); its starters when it never ran."""
        code, eng = self.code, self.eng
        mine = runs_of_key(self.runs, key)
        name = M.unit_name(key)
        if not mine:
            dump = self.dump_pre()
            slot = next((s for s in dump["slots"] if any(s["where"] == w and s["number"] == n
                                                          for w, n in unit_where(key))), None)
            if slot is not None:
                why = "frozen" if slot["frozen"] > 0 else ("paused by a delay" if slot["status"] == TE.SS_PAUSED
                                                           else "not run")
                i = code.index(key, slot["code"])
                ys = [j for j in code.yield_sites(key, slot["code"])] if slot["code"] >= 0 else []
                x = code.unit(key).ins[ys[0]] if ys else None
                vals = [TE.Value("delay", slot["delay"], "the slot's delay at the move's start", "time")] \
                    if why == "paused by a delay" else \
                    [TE.Value("freeze count", slot["frozen"], "the slot's freeze count at the move's start", "fact")]
                return {"kind": f"did not run: {why}", "unit": name, "offset": x.offset if x else slot["code"],
                        "opcode": ir.head(x.st) if x else None, "values": [v.as_dict() for v in vals],
                        "class": "split" if why == "paused by a delay" else "semantics", "cause": True,
                        "depth": depth}
            return self.starters(key, depth)
        first = mine[0]
        if first.code0 >= 0:
            i0 = code.index(key, first.code0)
            if not code.reachable(key, i0, targets) and depth < self.depth:
                res = self.starters(key, depth)  # a new instance must do it
                if res.get("cause"):
                    return res
        units = unit_where(key)
        ex, dumps = eng.explain(self.pre_f, mine[0].frame, mine[-1].frame, [list(u) for u in units])
        logged = [r for w, n in units for r in ex.of_unit(w, n)]
        logged.sort(key=lambda r: r.seq)
        steps = TE.steps_of(code, key, logged)
        want = None
        if fact is not None and fact.split(":")[0] in ("var", "bit"):
            want = int(fact.split(":")[1]) | (TE.BIT_BIT if fact.startswith("bit:") else 0)
        ins = code.unit(key).ins

        def via_writevar(i):  # an assignment, an embedded one or setVarRange: the log has its write
            st = ins[i].st
            return isinstance(st, ir.Assign) or (isinstance(st, ir.Op) and st.name == "setVarRange") or \
                any(isinstance(y, ir.Embed) for y in M._walk(st))
        hit = next((n for n, s in enumerate(steps) if s.i is not None and s.i in targets and
                    (want is None or not via_writevar(s.i) or any(v == want for v, _ in s.op.writes))), None)
        n = TE.first_off(code, key, steps, targets)
        if n is not None and (hit is None or n < hit):
            s = steps[n]
            dump = dumps.get(s.run.frame - 1) or self.dump_at(s.run.frame - 1)
            vals = TE.slice_test(code, key, steps, n, dump)
            x = code.unit(key).ins[s.i]
            return {"kind": "left the path", "unit": name, "room": s.run.room, "frame": s.run.frame,
                    "offset": s.op.code, "opcode": ir.head(x.st), "values": [v.as_dict() for v in vals],
                    "class": TE.cause_class(vals), "cause": True, "depth": depth,
                    "model_ran_unit": key in self.model_units}
        if hit is not None:
            s = steps[hit]
            x = code.unit(key).ins[s.i]
            got = next((v for var, v in s.op.writes if var == want), None) if want is not None else None
            if want is not None and via_writevar(s.i) and got != value:
                if key == ("global", self.row["pre"].get(f"var:{M.VAR_SENTENCE_SCRIPT}", -1)):
                    fresh = next((r for r in mine if r.seq == s.run.seq), None)
                    if fresh is not None and fresh.code0 == 0:
                        # a new instance on another sentence: the one the engine queued
                        q = self.queued_sentence(s.run.frame, s.run.seq)
                        if q is not None:
                            return q | {"wrote": [s.op.code, got], "depth": depth}
                    else:
                        # the sentence script ran on the earlier sentence: the new one waited in the queue
                        own = self.follow_waiting(key, depth)
                        if own is not None:
                            return own | {"wrote": [s.op.code, got]}
                dump = dumps.get(s.run.frame - 1) or self.dump_at(s.run.frame - 1)
                vals = TE.slice_test(code, key, steps, hit, dump)
                return {"kind": "wrote another value", "unit": name, "room": s.run.room, "frame": s.run.frame,
                        "offset": s.op.code, "opcode": ir.head(x.st), "written": got,
                        "values": [v.as_dict() for v in vals], "class": TE.cause_class(vals) if vals else "semantics",
                        "cause": True, "depth": depth, "model_ran_unit": key in self.model_units}
            if fact is not None and not fact.startswith(TE.SLOT_KEYS) and not via_writevar(s.i):
                # an op that writes the fact itself (no writeVar: the log has no value): it ran, and the fact
                # holds another value at done than the model computed
                got = self.row["done"].get(fact, 0)
                vals = [TE.Value(fact.replace(":", " "), got, "the engine's value at done after this op ran", "fact")]
                return {"kind": "ran the write, another value at done", "unit": name, "room": s.run.room,
                        "frame": s.run.frame, "offset": s.op.code, "opcode": ir.head(x.st),
                        "values": [v.as_dict() for v in vals], "class": "semantics", "cause": True, "depth": depth,
                        "model_ran_unit": key in self.model_units}
            again = self.started_again(key, mine, fact)
            if again is not None:
                return again | {"depth": depth}
            return {"kind": "reached the write", "unit": name, "frame": s.run.frame, "offset": s.op.code,
                    "opcode": ir.head(x.st), "writes": s.op.writes, "cause": None, "depth": depth}
        last = mine[-1]
        if last.status != TE.SS_DEAD and last.code1 >= 0 and code.reachable(key, code.index(key, last.code1), targets):
            lruns = [r for r in logged if r.frame == last.frame] or logged[-1:]
            lsteps = TE.steps_of(code, key, lruns)
            m = TE.last_test(code, key, lsteps)
            dump = self.dump_at(last.frame - 1)
            if m is not None:
                vals = TE.slice_test(code, key, lsteps, m, dump)
                x = code.unit(key).ins[lsteps[m].i]
                return {"kind": "still waiting: its loop's test", "unit": name, "frame": last.frame,
                        "offset": lsteps[m].op.code, "opcode": ir.head(x.st),
                        "values": [v.as_dict() for v in vals] + [{"what": "time", "value": self.done_f - self.pre_f,
                                                                   "source": "frames in the move", "kind": "time"}],
                        "class": "split", "cause": True, "depth": depth}
            ys = sorted(code.yield_sites(key, last.code1))
            x = code.unit(key).ins[ys[0]] if ys else None
            vals = TE.wait_values(x, last.delay, self.dump_at(self.done_f)) if x is not None else []
            return {"kind": "still waiting", "unit": name, "frame": last.frame,
                    "offset": x.offset if x else last.code1, "opcode": ir.head(x.st) if x else None,
                    "values": [v.as_dict() for v in vals], "class": "split", "cause": True, "depth": depth}
        return {"kind": "no branch off the path found", "unit": name, "cause": None, "depth": depth,
                "runs": len(mine), "steps": len(steps)}

    def dump_at(self, frame: int) -> dict:
        ch = self.eng.fork()
        if frame > ch.status["frame"]:
            ch.run(frame=frame)
        d = ch.state()
        ch.quit()
        return d

    def starters(self, key: tuple, depth: int) -> dict:
        """The units that can start ``key`` and ran in the window, followed toward their start sites."""
        if depth >= self.depth:
            return {"kind": "not started (depth)", "unit": M.unit_name(key), "cause": None}
        target = key
        if key == ("global", self.row["pre"].get(f"var:{M.VAR_SENTENCE_SCRIPT}", -1)):
            target = ("sentence",)
        both = [k for k in self.engine_units if k in self.model_units]
        order = both + [k for k in self.engine_units if k not in both] + \
            [k for k in self.model_units if k not in self.engine_units]
        for s, direct in [(k, True) for k in order] + [(k, False) for k in order]:
            sites = self.code.start_sites(s, target, direct)
            if not sites:
                continue
            res = self.follow(s, sites, depth + 1, None, None)
            res.setdefault("start_direct", direct)
            if res.get("kind") == "reached the write" and target == ("sentence",):
                # the sentence was queued: the sentence script's own slot kept it waiting
                own = self.follow_waiting(key, depth + 1)
                if own is not None:
                    return own | {"queued_by": res}
            if res.get("cause"):
                return res | {"started": M.unit_name(key)}
        return {"kind": "no starter ran", "unit": M.unit_name(key), "cause": None, "depth": depth}

    def started_again(self, key: tuple, mine: list, fact: str) -> dict | None:
        """A slot fact the engine's slot made as predicted, then holds again at done: a new instance started from
        the unit's start later in the window. The run that encloses the new instance's first run started it."""
        if fact is None or not fact.startswith("at:"):
            return None
        w = int(fact.rsplit(":", 1)[1])
        fresh = [r for r in mine if r.code0 == 0 and r.code1 == w]
        if not fresh:
            return None
        r = fresh[-1]
        outer = [q for q in self.runs.runs if q.seq < r.seq and q.depth < r.depth and q.frame == r.frame]
        caller = max(outer, key=lambda q: (q.depth, q.seq)) if outer else None
        ckey = self.code.key(caller.where, caller.number, caller.room) if caller else None
        if caller is None or ckey is None:
            return None
        ex, dumps = self.eng.explain(self.pre_f, r.frame, r.frame, [[caller.where, caller.number]])
        lg = ex.runs.get(caller.seq)
        op = None
        if lg is not None:
            steps = TE.steps_of(self.code, ckey, [lg])
            for st in steps:  # the caller's opcode that ran the new instance nested: the last one before it
                if st.i is not None:
                    x = self.code.unit(ckey).ins[st.i]
                    if isinstance(x.st, ir.Start) or (isinstance(x.st, ir.Op) and x.st.name in ir.ROOM_CHANGE):
                        op = (st.op.code, ir.head(x.st))
        done_room = self.row["done"].get("room", 0)
        return {"kind": "started again", "unit": M.unit_name(ckey), "room": caller.room, "frame": r.frame,
                "offset": op[0] if op else caller.code0, "opcode": op[1] if op else None,
                "values": [{"what": "room", "value": done_room, "source": "the room at done", "kind": "fact"},
                           {"what": f"{M.unit_name(key)} restarted", "value": r.frame, "source": "its new run's frame",
                            "kind": "fact"}],
                "class": "semantics", "cause": True, "model_ran_unit": ckey in self.model_units}

    def queued_sentence(self, frame: int, before_seq: int) -> dict | None:
        """The doSentence the engine ran last before the sentence script's run ``before_seq`` in ``frame`` (its
        locals 0-2 are the queued verb and objects, checkAndRunSentenceScript, script.cpp:1240-1246), with the
        values of its arguments, each followed back over that frame's logged runs."""
        ex, dumps = self.eng.explain(self.pre_f, frame, frame, None)
        dump = dumps.get(frame - 1)
        best = None
        for seq, lg in sorted(ex.runs.items()):
            if seq >= before_seq:
                break
            key = self.code.key(lg.where, lg.number, lg.room)
            if key is None:
                continue
            steps = TE.steps_of(self.code, key, [lg])
            for n, st in enumerate(steps):
                if st.i is None:
                    continue
                x = self.code.unit(key).ins[st.i]
                if isinstance(x.st, ir.Op) and x.st.name == "doSentence" and "STOP" not in x.st.flags:
                    best = (key, lg, steps, n, x)
        if best is None:
            return None
        key, lg, steps, n, x = best
        vals = TE.slice_test(self.code, key, steps, n, dump)
        return {"kind": "the engine queued another sentence", "unit": M.unit_name(key), "room": lg.room,
                "frame": frame, "offset": x.offset, "opcode": "doSentence", "values": [v.as_dict() for v in vals],
                "class": TE.cause_class(vals), "cause": True, "model_ran_unit": key in self.model_units}

    def follow_waiting(self, key: tuple, depth: int) -> dict | None:
        """Where the slot of ``key`` waited at done (the sentence script still busy with the earlier sentence)."""
        mine = runs_of_key(self.runs, key)
        if not mine:
            return None
        last = mine[-1]
        if last.status == TE.SS_DEAD or last.code1 < 0:
            return None
        units = unit_where(key)
        ex, dumps = self.eng.explain(self.pre_f, last.frame, last.frame, [list(u) for u in units])
        logged = sorted([r for w, n in units for r in ex.of_unit(w, n)], key=lambda r: r.seq)
        steps = TE.steps_of(self.code, key, logged)
        m = TE.last_test(self.code, key, steps)
        dump = dumps.get(last.frame - 1)
        x = None
        vals = []
        if m is not None:
            vals = TE.slice_test(self.code, key, steps, m, dump)
            x = self.code.unit(key).ins[steps[m].i]
        else:
            ys = sorted(self.code.yield_sites(key, last.code1))
            x = self.code.unit(key).ins[ys[0]] if ys else None
            vals = TE.wait_values(x, last.delay, dump) if x is not None else []
        return {"kind": "the sentence waited: its script was busy", "unit": M.unit_name(key), "frame": last.frame,
                "offset": steps[m].op.code if m is not None else (x.offset if x else last.code1),
                "opcode": ir.head(x.st) if x else None, "values": [v.as_dict() for v in vals],
                "class": TE.cause_class(vals) if vals else "split", "cause": True, "depth": depth}


def cmd_runb_explain(a) -> dict:
    """Sections 4 and 5 on run B's moves on seed 1: every move the model rejects and every certain prediction the
    engine refutes, in the moves before run B's inputs stop doing what they did in run B."""
    import check
    import check_rust
    import groundtruth
    t0 = time.monotonic()
    _truth_at(Path(a.truth).resolve())
    check_rust._ARGS.update(bin=Path(a.machine).resolve())
    for k, v in check_rust.RUST_SWITCHES.items():
        M.SWITCHES.setdefault(k, v)
    features = frozenset(ROW5.split(","))
    switches = frozenset(a.switches.split(","))
    machine = M.Machine(features, switches=switches)
    rows = groundtruth.load()
    kinds = check.held_families(rows)
    checked = [json.loads(x) for x in Path(a.checked).read_text().splitlines() if x.strip()]
    results_rooms = {json.loads(x)["name"]: json.loads(x).get("room")
                     for x in (groundtruth.RUN / "results.jsonl").read_text().splitlines() if x.strip()}
    own_log = RESULTS / f"check-part1-row5-{'-'.join(sorted(switches))}.log"
    same = runb_same_mismatch(own_log)
    known_room = {"S1#23"}  # run B's own ground truth: room_post disagrees with results.jsonl (groundtruth-part1.log)
    n_sync, first_bad = synced_prefix(rows, checked, results_rooms, same, known_room)
    code = TE.Code(machine.game)
    eng = RunB(Path(a.bin), a.out / "engine", rows[n_sync - 1]["done_frame"] + 1 if n_sync else 1)
    causes, rejected = [], []
    try:
        for i in range(n_sync):
            row, m = rows[i], checked[i]
            if m["status"] != "counted":
                continue
            lab = m["label"]
            if not m.get("accepted"):
                floor = m.get("reason") == "walk to a floor point"
                rejected.append({"move": lab, "why_not": m.get("why_not"),
                                 "input": "a click on a floor point" if floor else m.get("kind"),
                                 "kind": "outside V" if m.get("why_not") == "no action kind" else "rejected",
                                 "cause": None})
                continue
            if not m.get("misses"):
                continue
            status, action, _ = check.classify(row, machine.game)
            run = check_rust.run_model(machine, action, M.FactOracle(row["pre"], kinds))
            ex = Explainer(code, eng, row, run, kinds)
            for fact, value, engine in m["misses"]:
                t = time.monotonic()
                c = ex.explain(fact, value)
                c["seconds"] = round(time.monotonic() - t, 2)
                causes.append(c)
                say(f"{lab} {fact}: {c.get('kind')} {c.get('unit')} [{c.get('offset')}] {c.get('opcode')} "
                    f"{c.get('class')}")
    finally:
        eng.close()
    doc = {"truth": Path(a.truth).name, "seed": 1, "synced_rows": n_sync, "first_departure": first_bad,
           "counted_in_sync": sum(1 for m in checked[:n_sync] if m["status"] == "counted"),
           "certain_right": sum(len(m.get("hits") or []) for m in checked[:n_sync] if m.get("accepted")),
           "certain_wrong": sum(len(m.get("misses") or []) for m in checked[:n_sync] if m.get("accepted")),
           "rejected": rejected, "causes": causes, "engine": eng.stats, "seconds": round(time.monotonic() - t0, 1)}
    return doc


def cause_key(c: dict) -> tuple:
    return c.get("unit"), c.get("offset"), c.get("opcode"), c.get("class")


def causes_log(doc: dict, title: str) -> list:
    """The cause list's counts: by class, by kind, and the causes (unit, offset, opcode, class) by count."""
    import collections
    cs = doc["causes"]
    got = [c for c in cs if c.get("cause")]
    rej = doc.get("rejected", [])
    outside = [r for r in rej if r["kind"] == "outside V"]
    n = len(cs) + len(rej)
    lines = [title,
             f"misses: {n} = {len(rej)} rejected moves ({len(outside)} outside V: inputs V does not hold) + {len(cs)} "
             f"refuted certain predictions",
             f"with an automatic cause (an opcode and the engine's value): {len(got)} of {n} "
             f"({100 * len(got) / n:.1f}%); of the misses inside V: {len(got)} of {n - len(outside)} "
             f"({100 * len(got) / max(1, n - len(outside)):.1f}%)" if n else "no misses"]
    by_class = collections.Counter(c.get("class") for c in got)
    lines.append("by class: " + ", ".join(f"{k} {v}" for k, v in by_class.most_common()))
    by_kind = collections.Counter(c.get("kind") for c in cs)
    lines.append("by kind: " + ", ".join(f"{k} {v}" for k, v in by_kind.most_common()))
    lines.append("causes by count (unit, code offset, opcode, class; the engine values of the first; the misses):")
    groups = collections.defaultdict(list)
    for c in got:
        groups[cause_key(c)].append(c)
    for key, items in sorted(groups.items(), key=lambda kv: (-len(kv[1]), str(kv[0]))):
        vals = "; ".join(f"{v['what']} = {v['value']} ({v['kind']})" for v in items[0].get("values", []))
        lines.append(f"  {len(items)}: {key[0]} [{key[1]:04X}] {key[2]} ({key[3]}): {vals}; "
                     f"{', '.join(sorted({c['move'] + ' ' + c['fact'] for c in items}))}"
                     if isinstance(key[1], int) else f"  {len(items)}: {key}")
    none = [c for c in cs if not c.get("cause")]
    for c in none:
        lines.append(f"  no cause: {c['move']} {c['fact']}: {c.get('kind')}")
    for r in rej:
        lines.append(f"  rejected: {r['move']}: {r['kind']} ({r['input']})")
    return lines


def write(stem: str, doc: dict, lines: list, results: bool) -> None:
    if results:
        RESULTS.mkdir(exist_ok=True)
        (RESULTS / f"{stem}.json").write_text(json.dumps(doc, indent=1) + "\n")
        (RESULTS / f"{stem}.log").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("check-off")
    p.add_argument("--new", required=True)
    p.add_argument("--old", default=str(SHARED_BIN))
    p.add_argument("--frames", type=int, default=3234)
    p.add_argument("--keep", type=int, default=5)
    p.add_argument("--out", type=Path, default=OUT)
    p.add_argument("--results", action="store_true")
    p = sub.add_parser("runb-truth")
    p.add_argument("--seed", type=int, default=1)
    p.add_argument("--bin", required=True)
    p.add_argument("--out", type=Path, required=True)
    p = sub.add_parser("runb-check")
    p.add_argument("--truth", type=Path, required=True)
    p.add_argument("--machine", type=Path, required=True)
    p.add_argument("--switches", default=FR1)
    p.add_argument("--jobs", type=int, default=3)
    p.add_argument("--out", type=Path, required=True)
    p = sub.add_parser("runb-explain")
    p.add_argument("--truth", type=Path, required=True)
    p.add_argument("--checked", type=Path, required=True, help="runb-check's row 5 jsonl")
    p.add_argument("--machine", type=Path, required=True)
    p.add_argument("--bin", required=True)
    p.add_argument("--switches", default=FR1)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--results", action="store_true")
    a = ap.parse_args()
    if a.cmd == "runb-explain":
        doc = cmd_runb_explain(a)
        (a.out / "runb-causes.json").write_text(json.dumps(doc, indent=1, default=str) + "\n")
        lines = causes_log(doc, f"B1 on run B's recorded inputs on seed 1: row 5 of the model check with fix round 1's "
                                f"switches, the {doc['counted_in_sync']} counted moves of the first {doc['synced_rows']} "
                                f"rows (the inputs leave run B's moves at {doc['first_departure']}); certain "
                                f"predictions right {doc['certain_right']} of "
                                f"{doc['certain_right'] + doc['certain_wrong']}")
        lines.append(f"engine: {doc['engine']}; {doc['seconds']} s")
        write("b1-runb-seed1", doc, lines, a.results)
    elif a.cmd == "runb-truth":
        cmd_runb_truth(a)
    elif a.cmd == "runb-check":
        cmd_runb_check(a)
    elif a.cmd == "check-off":
        doc = cmd_check_off(a)
        write("trace-check-off", doc, check_off_log(doc), a.results)


if __name__ == "__main__":
    main()
