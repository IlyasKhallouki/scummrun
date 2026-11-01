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
import collections
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


def _run_at(a) -> None:
    """``--run DIR --movie FILE`` (both or neither; default run B's): groundtruth.py's run (results.jsonl and the
    executor's movie.jsonl) and its explicit-key movie, for a run made otherwise from run B's route (runb_window.py)."""
    import groundtruth
    run, movie = getattr(a, "run_dir", None), getattr(a, "movie", None)
    if (run is None) != (movie is None):
        raise SystemExit("--run and --movie go together")
    if run is not None:
        groundtruth.RUN = Path(run).resolve()
        groundtruth.MOVIE = Path(movie).resolve()


def cmd_runb_truth(a) -> None:
    """groundtruth.py's ground truth of run B's Part I moves (the same moves, boundaries and dumps), with run B's
    recorded inputs played on ``--seed`` in place of run B's own seed."""
    import groundtruth
    if a.seed == ANY_SEED:
        raise SystemExit("the Any% seed is not used (plan.md)")
    a.out.mkdir(parents=True, exist_ok=True)
    paths.SCUMMVM_BIN = Path(a.bin)
    _run_at(a)
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
    _run_at(a)
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


# --- the window search's runs, strictly matched (section 3) ----------------------------------------------------------

class Reading:
    """A reading's records as the strict matcher reads them: event records by (unit, wait point), every action by
    its name, the domains and the view."""

    def __init__(self, path: Path):
        import packed_reading as PR
        import replay as RP
        self.RP = RP
        self.rd = PR.load_reading(path)
        self.task = self.rd.task
        self.doms = self.task.facts
        self.by_name, self.events = {}, collections.defaultdict(list)
        # the dump holds every slot's locals (dump_locals), so args: is observed; the saved verbs are not
        self.unobserved = ("saved:",)
        lines = self.rd.lines
        with open(lines, "rb") as fh:
            for ln in fh:
                d = json.loads(ln)
                i, src = d["i"], d["source"]
                if src[0] == "event" and not src[4]:
                    self.events[(src[1], src[2])].append(i)
        for i in range(len(self.task.actions)):
            self.by_name[self.task.actions[i].name] = i
        self._recs = {}

    def rec(self, i: int) -> tuple:
        """(index, unit, wait point, precondition, effects, checks) of action i, for StrictMatcher."""
        if i not in self._recs:
            a = self.task.actions[i]
            w = a.what
            src = w.source
            unit, wp = (src[1], src[2]) if src[0] == "event" else (None, None)
            eff = {}
            for k, e in w.effects.items():
                if k.startswith(self.unobserved):
                    continue
                # the packed reading's effects hold the task's values already (labels such as "above")
                if e[0] == "set":
                    eff[k] = ("set", e[1])
                elif e[0] == "guess":
                    eff[k] = ("guess", frozenset(e[1]))
                else:
                    eff[k] = (e[0],)
            pre = {k: vs for k, vs in a.pre.items() if not k.startswith(self.unobserved)}
            self._recs[i] = (i, unit, wp, pre, eff, list(w.checks))
        return self._recs[i]

    def view(self, facts: dict, dump: dict) -> dict:
        """The reading's view of the engine at a decision point, with each live slot's ``args:`` (the built list
        its dumped locals match, replay.replan_start_locals; 0 when none does)."""
        out = self.RP.view(self.rd.b, self.doms, facts)
        ds = self.RP.dumped_slots(dump)
        if ds is not None and self.rd.lists is not None:
            start, _, _ = self.RP.replan_start_locals(self.rd.b, facts, {}, self.task, ds["slots"], self.rd.lists)
            for k, v in start.items():
                if k.startswith("args:"):
                    name, pos = k[5:], start.get(f"pos:{k[5:]}")
                    slot = ds["slots"].get(name)
                    if slot is None or pos is None or pos < 0:
                        out[k] = frozenset({v})
                        continue
                    live = self.RP.live_match(self.rd.b, slot)
                    sigs = frozenset(l["sig"] for l in self.rd.lists.get((name, pos), [])
                                     if self.RP.list_matches(l, slot, live))
                    out[k] = sigs or frozenset({0})
        return out


def record_fails(rec: tuple, run: TE.Run, views: dict, k: int) -> dict:
    """Every condition of one record that the engine's run between D_{k-1} and D_k fails: the precondition's keys
    with the engine's values, its slot's end, its other effects."""
    before, after = views[k - 1], views.get(k, {})
    pre = [(kk, str(before.get(kk) if not isinstance(before.get(kk), frozenset) else sorted(before.get(kk))[:3]))
           for kk, vs in rec[3].items() if not TE._holds(before.get(kk), vs)]
    pos = rec[4].get(f"pos:{rec[1]}")
    end = run.code1 if run.code1 >= 0 else TE.NOWHERE
    end_bad = pos is not None and not TE.StrictMatcher._value_ok(pos, end)
    eff = [(kk, str(after.get(kk))) for kk, e in rec[4].items()
           if not kk.startswith("pos:") and e[0] in ("set", "guess") and kk in after and
           not TE.StrictMatcher._value_ok(e, after[kk])]
    return {"pre_fails": pre[:6], "end": [end, pos[1] if pos else None] if end_bad else None, "effect_fails": eff[:6]}


def run_engine_cfg(doc: dict, out: Path, events: list):
    """The engine configuration the run's verify used: the harness boot's (replay_part1.config) or, for a start from
    run B's inputs, run B's settings (replay_kitchen.cfg_of) with run B's inputs up to the start before the run's."""
    import replay
    import replay_kitchen
    import replay_part1
    starter = doc.get("starter") or doc["start"].get("starter")
    out.mkdir(parents=True, exist_ok=True)
    if starter:
        prefix = [e for e in replay_kitchen.prefix_events() if e["frame"] <= starter["frame"]]
        cfg = replay_kitchen.cfg_of(out, replay.write_events(out / "movie.jsonl", prefix + events), seed=doc["seed"])
    else:
        cfg = replay_part1.config(doc["seed"], out, replay.write_events(out / "movie.jsonl", events))
    return dataclasses.replace(cfg, dump_locals=True)


class WindowEngine:
    """The run's chosen inputs from boot in a fresh engine, stopped at the run's start; children of it trace the run
    (every slot run and the state dump at every decision point) and explain one frame."""

    def __init__(self, doc: dict, binary: Path, out: Path):
        paths.SCUMMVM_BIN = binary
        r = doc["run"]
        self.f0, self.f1 = r["frame0"], r["frame_end"]
        self.c = Controller(run_engine_cfg(doc, out, r["inputs"]), timeout_s=600)
        self.c.start()
        self.root = self.c.root
        self.root.run(frame=self.f0)
        assert self.root.status["frame"] == self.f0, self.root.status
        self.stats = {"forks": 0, "explain_entries": 0}

    def trace(self) -> tuple:
        t0 = time.monotonic()
        runs, dumps = TE.Runs(), {}
        b = self.c.fork(self.root)
        self.stats["forks"] += 1
        dumps[self.f0] = b.state()
        b.segment(True)
        for f in range(self.f0 + 1, self.f1 + 1):
            st = b.run(frame=f)
            if "end" in st:
                break
            runs.add(b.runs()["runs"])
            dumps[f] = b.state()
        b.quit()
        return runs, dumps, round(time.monotonic() - t0, 1)

    def explain(self, k: int, units) -> TE.Explained:
        ch = self.c.fork(self.root)
        self.stats["forks"] += 1
        ch.segment(True)
        if k - 1 > ch.status["frame"]:
            ch.run(frame=k - 1)
        ch.explain(k, k, units)
        ch.run(frame=k)
        lg = ch.explain_log()
        ch.quit()
        self.stats["explain_entries"] += len(lg["explain"])
        return TE.Explained(lg["explain"])

    def close(self) -> None:
        self.c.close()


def ghost_alpha(rd, runs: TE.Runs, dumps: dict, f0: int, f1: int) -> tuple:
    """--ghosts (E004 b1-refine.md, Lemma G): α on a trace. For each unit the reading's ``ghost_slots`` verdicts name,
    the newest instance is the slot of the unit's latest run from its first instruction (code 0); a run on another
    slot of the unit after that is a ghost run, and a dumped slot of the unit that is not the newest is left out of the
    view. Before the unit's first start in the trace nothing is left out. Returns (the dumps under α, the units, the
    ghost runs' seq numbers, the frames whose dump had a slot left out)."""
    gv = rd.rd.doc.get("ghost_slots")
    if gv is None:
        raise SystemExit("--ghosts needs a reading made with the reader's switch ghost_slots (its verdicts)")
    units = {u for u in gv if not u.startswith("(")}
    newest, newest_at, seqs = {}, {f0: {}}, set()
    for k in range(f0 + 1, f1 + 1):
        for r in sorted(runs.between(k), key=lambda r: r.seq):
            if r.unit not in units:
                continue
            if r.code0 == 0:
                newest[r.unit] = r.slot
            elif r.unit in newest and r.slot != newest[r.unit]:
                seqs.add(r.seq)
        newest_at[k] = dict(newest)
    out, frames = {}, {}
    for k, d in dumps.items():
        nw = newest_at.get(k, {})
        keep = [s for s in d["slots"] if not ((u := F.unit_of(s["where"], s["number"])) in nw and s["slot"] != nw[u])]
        if len(keep) != len(d["slots"]):
            out[k] = dict(d, slots=keep)
            frames[k] = len(d["slots"]) - len(keep)
        else:
            out[k] = d
    return out, units, seqs, frames


def unplaced_check(code: TE.Code, eng: "WindowEngine", k: int, run: TE.Run, rd, slot: int | None = None) -> dict:
    """A run at a wait point the reading has no event record at: whether it changed a fact of the task (a logged write
    of a task variable or bit, an op that writes an object's state, owner or class, a verb, or starts or stops a
    script), or left its slot elsewhere. If not, the task cannot see it: no record is needed."""
    key = code.key(run.where, run.number, run.room)
    if key is None:
        return {"verdict": "unit not in the IR"}
    ex = eng.explain(k, [[run.where, run.number]])
    lg = next((r for r in ex.runs.values() if r.frame == run.frame and r.code0 == run.code0 and
               r.where == run.where and r.number == run.number and (slot is None or r.slot == slot)), None)
    if lg is None:
        return {"verdict": "the run is not in the explain log"}
    u = code.unit(key)
    for op in lg.ops:
        i = u.at.get(op.code)
        for var, val in op.writes:
            fk = f"bit:{var & 0x7FFF}" if var & TE.BIT_BIT else (None if var & TE.LOCAL_BIT else f"var:{var}")
            if fk is not None and fk in rd.doms:
                return {"verdict": "writes a task fact", "offset": op.code, "opcode": ir.head(u.ins[i].st) if i is not
                        None else None, "fact": fk, "value": val, "class": "semantics", "cause": True}
        st = u.ins[i].st if i is not None else None
        if isinstance(st, ir.Start) or (isinstance(st, ir.Op) and st.name in (
                "setState", "setOwnerOf", "setClass", "pickupObject", "drawObject", "VerbOps", "stopScript",
                "freezeScripts", "loadRoom", "loadRoomWithEgo")):
            return {"verdict": "runs an op that writes a task fact", "offset": op.code, "opcode": ir.head(st),
                    "class": "semantics", "cause": True}
    if run.code1 != run.code0:
        return {"verdict": "left its slot at another wait point", "end": run.code1, "class": "semantics",
                "cause": True}
    return {"verdict": "invisible to the task: no task fact written, the slot back at its wait point",
            "ops": len(lg.ops)}


def slot_write_sites(code: TE.Code, key, fact: str) -> frozenset:
    """The instructions of ``key`` that can write ``fact``: write_sites, and for a slot's facts the ops that start,
    stop, freeze or unfreeze scripts."""
    out = set(code.write_sites(key, fact)) if fact.split(":")[0] in ("var", "bit", "state", "owner", "class", "verb",
                                                                       "room", "cutscene") else set()
    kind = fact.split(":")[0]
    own = M.unit_name(key) == fact.split(":", 1)[1]
    if kind in ("local", "script", "objscript") and own:
        out |= code.end_sites(key)
    for i, x in enumerate(code.unit(key).ins):
        st = x.st
        # a freeze count changes on freezeScripts and unfreezeScripts, and a start sets it to 0 (script.cpp:100, 147)
        if kind == "frozen" and (isinstance(st, ir.Start) or (isinstance(st, ir.Op) and
                                                              ("reeze" in st.name or "utscene" in st.name))):
            out.add(i)
        elif kind in ("local", "script", "objscript") and (isinstance(st, ir.Start) or
                                                           (isinstance(st, ir.Op) and st.name.startswith("stop"))):
            out.add(i)
    return frozenset(out)


def C_counter(xs):
    import collections as C
    return C.Counter(x for x in xs if x is not None)


def args_cause(code: TE.Code, key, steps: list, run: TE.Run, fk: str, rd, before: dict, dump: dict, k: int) -> dict:
    """An ``args:`` precondition that fails: the slot's dumped locals against the lists the records ask for (the
    reading's lists at that wait point with those numbers); the first local that differs, the opcode of the run that
    reads it, and the engine's value."""
    name = fk[5:]
    sigs = {v for i in rd.events.get((run.unit, run.code0), []) for v in rd.rec(i)[3].get(fk, ())}
    lists = [l for l in (rd.rd.lists or {}).get((name, run.code0), []) if l["sig"] in sigs]
    slot = next((sl for sl in dump["slots"] if F.unit_of(sl["where"], sl["number"]) == name), None)
    got = before.get(fk)
    if slot is None or "locals" not in slot:
        return {"cause": None, "kind": "the slot's locals are not in the dump", "unit": run.unit, "frame": k}
    diffs = collections.Counter()
    for l in lists:
        for n, x in enumerate(slot["locals"]):
            v = l["locals"].get(n, 0)
            if isinstance(v, dict):
                if v.get("values") is not None and x not in v["values"]:
                    diffs[n] += 1
            elif v != x:
                diffs[n] += 1
        if l["override"] != slot["cutscene_override"]:
            diffs["override"] += 1
    same_unit = [sl for sl in dump["slots"] if F.unit_of(sl["where"], sl["number"]) == name]
    if len(same_unit) > 1:
        vals = [TE.Value(f"live slots of {name}", len(same_unit), "the dump: a recursive start's second instance; "
                         "the reading keeps one slot per unit", "fact")]
        return {"cause": True, "kind": "two instances of the unit", "unit": run.unit, "frame": k, "offset": run.code0,
                "opcode": None, "values": [v.as_dict() for v in vals], "class": "semantics"}
    want = {"recursive": True if slot.get("recursive") else None, "resistant": bool(slot.get("resistant")),
            "owned": slot["where"] == F.WIO_INVENTORY}
    if not diffs and lists and "resistant" in slot and \
            all(json.dumps(l.get("flags"), sort_keys=True) != json.dumps(want, sort_keys=True) for l in lists):
        lf = collections.Counter(json.dumps(l.get("flags"), sort_keys=True) for l in lists)
        vals = [TE.Value("slot flags", json.dumps(want, sort_keys=True),
                         f"the dump's (freezeResistant, recursive, owned); the records' lists hold "
                         f"{[x for x, _ in lf.most_common(3)]}", "fact")]
        return {"cause": True, "kind": "the slot's flags are not the lists'", "unit": run.unit, "frame": k,
                "offset": run.code0, "opcode": None, "values": [v.as_dict() for v in vals], "class": "semantics"}
    if not diffs:
        why = "no list at that wait point has the records' numbers" if not lists else "the flags differ"
        return {"cause": None, "kind": f"args: {why}", "unit": run.unit, "frame": k, "engine_args": got}
    loc = diffs.most_common(1)[0][0]
    if loc == "override":
        vals = [TE.Value("cutscene override count", slot["cutscene_override"], "the dump", "fact")]
        return {"cause": True, "kind": "the slot's locals match no list the records ask for", "unit": run.unit,
                "frame": k, "offset": run.code0, "opcode": None, "values": [v.as_dict() for v in vals],
                "class": "semantics"}
    vid = TE.LOCAL_BIT | loc
    n = next((m for m, st in enumerate(steps) if any(v == vid for v, _ in st.op.reads)), None)
    wanted = sorted({str(l["locals"].get(loc, 0)) if not isinstance(l["locals"].get(loc, 0), dict) else "unknown"
                     for l in lists})
    vals = [TE.Value(f"local {loc}", slot["locals"][loc], f"the dumped slot's local; the records' lists hold "
                     f"{wanted[:6]}", "unknown")]
    x = code.unit(key).ins[steps[n].i] if n is not None and steps[n].i is not None else None
    return {"cause": True, "kind": "the slot's locals match no list the records ask for", "unit": run.unit,
            "frame": k, "offset": steps[n].op.code if n is not None else run.code0,
            "opcode": ir.head(x.st) if x else None, "values": [v.as_dict() for v in vals], "class": "split",
            "note": None if n is not None else "no opcode of the run reads that local"}


def explain_unexplained(code: TE.Code, eng: WindowEngine, k: int, run: TE.Run, why: str, rd, views: dict,
                        dumps: dict) -> dict:
    """Section 5 for one run the strict matcher left unexplained: in a child at D_{k-1}, the run's own opcodes, then
    the opcode behind the condition that failed and the engine's values there."""
    key = code.key(run.where, run.number, run.room)
    if key is None:
        return {"cause": None, "kind": "unit not in the IR"}
    ex = eng.explain(k, [[run.where, run.number]])
    lg = next((r for r in ex.runs.values() if r.frame == run.frame and r.after == run.after and
               r.code0 == run.code0 and r.where == run.where and r.number == run.number), None)
    if lg is None:
        return {"cause": None, "kind": "the run is not in the explain log"}
    steps = TE.steps_of(code, key, [lg])
    dump = dumps.get(k - 1)
    kind, _, fk = why.partition(" ")
    before = views[k - 1]
    if kind == "pre" and fk.startswith(("args:", "pos:")):
        same_unit = [sl for sl in dump["slots"] if F.unit_of(sl["where"], sl["number"]) == fk.split(":", 1)[1]]
        if len(same_unit) > 1:
            vals = [TE.Value(f"live slots of {run.unit}", len(same_unit), "the dump: a recursive start's second "
                             "instance; the reading keeps one slot per unit", "fact")]
            return {"cause": True, "kind": "two instances of the unit", "unit": run.unit, "frame": k,
                    "offset": run.code0, "opcode": None, "values": [v.as_dict() for v in vals], "class": "semantics"}
    if kind == "pre" and fk.startswith("args:"):
        return args_cause(code, key, steps, run, fk, rd, before, dump, k)
    if kind == "pre":
        got = before.get(fk)
        want = sorted({str(v) for i in rd.events.get((run.unit, run.code0), []) for v in rd.rec(i)[3].get(fk, ())})
        vid = None
        if fk.startswith("var:"):
            vid = int(fk[4:])
        elif fk.startswith("bit:"):
            vid = TE.BIT_BIT | int(fk[4:])
        n = next((m for m, st in enumerate(steps) if vid is not None and any(v == vid for v, _ in st.op.reads)), None)
        vals = [TE.Value(fk, got, f"the view at the decision point before; the records ask {want[:6]}",
                         "unknown" if fk.startswith("args:") else "fact")]
        if n is not None:
            x = code.unit(key).ins[steps[n].i]
            return {"cause": True, "kind": "the records' precondition fails", "unit": run.unit, "frame": k,
                    "offset": steps[n].op.code, "opcode": ir.head(x.st), "values": [v.as_dict() for v in vals],
                    "class": TE.cause_class(vals)}
        x = code.unit(key).ins[steps[0].i] if steps and steps[0].i is not None else None
        return {"cause": True, "kind": "the records' precondition fails", "unit": run.unit, "frame": k,
                "offset": run.code0, "opcode": ir.head(x.st) if x else None, "values": [v.as_dict() for v in vals],
                "class": TE.cause_class(vals), "note": "no opcode of the run reads it"}
    if kind == "end":
        ends = set()
        for i in rd.events.get((run.unit, run.code0), []):
            rec = rd.rec(i)
            if all(TE._holds(before.get(kk), vs) for kk, vs in rec[3].items()):
                pos = rec[4].get(f"pos:{run.unit}")
                ends.add(pos[1] if pos and pos[0] == "set" else run.code0)
        targets = set()
        for e in ends:
            targets |= code.end_sites(key) if e == TE.NOWHERE else code.yield_sites(key, e)
        targets = frozenset(targets)
        n = TE.first_off(code, key, steps, targets, cross_yields=False)
        base = {"unit": run.unit, "frame": k, "engine_end": run.code1, "record_ends": sorted(ends)}
        if n is not None:
            x = code.unit(key).ins[steps[n].i]
            if x.yields == "wait":  # the records' paths block at this WaitFor op; the engine's passed it
                vals = TE.wait_values(x, 0, dump)
                return base | {"cause": True, "kind": "the wait did not block", "offset": steps[n].op.code,
                               "opcode": ir.head(x.st), "values": [v.as_dict() for v in vals], "class": "split"}
            vals = TE.slice_test(code, key, steps, n, dump)
            return base | {"cause": True, "kind": "left the records' path", "offset": steps[n].op.code,
                           "opcode": ir.head(x.st), "values": [v.as_dict() for v in vals],
                           "class": TE.cause_class(vals)}
        last = steps[-1] if steps else None
        if last is not None and last.i is not None and code.unit(key).ins[last.i].yields == "wait" and \
                run.code1 == last.op.code and code.reachable(key, code.unit(key).ins[last.i].next, targets, False):
            # the run blocked at a WaitFor op the records' paths pass (their check took "does not block")
            x = code.unit(key).ins[last.i]
            vals = TE.wait_values(x, 0, dumps.get(k))
            return base | {"cause": True, "kind": "the wait blocked", "offset": last.op.code,
                           "opcode": ir.head(x.st), "values": [v.as_dict() for v in vals], "class": "split"}
        if not ends:
            bad = C_counter(next((kk for kk, vs in rd.rec(i)[3].items() if not TE._holds(before.get(kk), vs)), None)
                            for i in rd.events.get((run.unit, run.code0), []))
            fk2 = bad.most_common(1)[0][0] if bad else None
            return base | {"cause": None, "kind": "no record's precondition holds", "most_failing": fk2,
                           "engine": before.get(fk2) if fk2 else None}
        return base | {"cause": None, "kind": "no branch off the records' path found",
                       "steps": len(steps)}
    if kind == "effect":
        after = views.get(k, {})
        got = after.get(fk)
        vid = int(fk[4:]) if fk.startswith("var:") else None
        n = next((m for m, st in enumerate(steps) if vid is not None and any(v == vid for v, _ in st.op.writes)), None)
        if n is not None:
            vals = TE.slice_test(code, key, steps, n, dump)
            x = code.unit(key).ins[steps[n].i]
            return {"cause": True, "kind": "wrote another value", "unit": run.unit, "frame": k,
                    "offset": steps[n].op.code, "opcode": ir.head(x.st), "written": got,
                    "values": [v.as_dict() for v in vals], "class": TE.cause_class(vals)}
        # the write the records predict did not run: Lemma X toward its sites in this run
        targets = slot_write_sites(code, key, fk)
        n = TE.first_off(code, key, steps, targets, cross_yields=False) if targets else None
        if n is not None:
            x = code.unit(key).ins[steps[n].i]
            vals = TE.wait_values(x, 0, dump) if x.yields == "wait" else TE.slice_test(code, key, steps, n, dump)
            return {"cause": True, "kind": "left the path to the predicted write", "unit": run.unit, "frame": k,
                    "offset": steps[n].op.code, "opcode": ir.head(x.st), "values": [v.as_dict() for v in vals],
                    "class": "split" if x.yields == "wait" else TE.cause_class(vals), "fact": fk}
        return {"cause": None, "kind": "the predicted write is not in the run", "unit": run.unit, "frame": k,
                "engine": got, "sites": len(targets)}
    return {"cause": None, "kind": f"no rule for {why}"}


def cmd_window_validate(a) -> dict:
    """Section 3 on one window search run: the strict matcher over every slot run from the run's start to its end,
    against the reading's event records; the plan's steps, each with the run that strictly matches it or why none
    does; the wait check (section 6)."""
    import collections as C
    doc = json.loads(Path(a.run).read_text())
    t0 = time.monotonic()
    rd = Reading(Path(a.reading or doc["reading"]))
    import replay as RP0
    if doc.get("unobserved_actor_rooms"):
        rd.unobserved = rd.unobserved + (RP0.ACTOR_ROOM,)
    load_s = time.monotonic() - t0
    eng = WindowEngine(doc, Path(a.bin), a.out / "engine")
    runs, dumps, trace_s = eng.trace()
    f0, f1 = eng.f0, eng.f1
    ghost_seqs, ghost_units, ghost_frames = set(), set(), {}
    if getattr(a, "ghosts", False):
        dumps, ghost_units, ghost_seqs, ghost_frames = ghost_alpha(rd, runs, dumps, f0, f1)
    import replay as RP
    unobserved = RP.UNOBSERVED + ((RP.ACTOR_ROOM,) if doc.get("unobserved_actor_rooms") else ())
    views = {f: rd.view(F.facts_of(d), d) for f, d in dumps.items()}
    if unobserved != RP.UNOBSERVED:
        for v in views.values():
            for k in [k for k in v if k.startswith(RP.ACTOR_ROOM)]:
                del v[k]

    memo = {}

    def decide(check, run):
        f = run.frame - 1 if not run.after else run.frame
        key = (check[0], check[1], check[2], f)
        if key in memo:
            return memo[key]
        d = dumps.get(f)
        if d is None:
            return None
        memo[key] = got = RP.eval_check(check, type("R", (), {"kind": "event", "source": ("event",)})(), d)
        return got

    cands = {}
    for key, idxs in rd.events.items():
        for i in idxs:
            cands.setdefault(key, []).append(rd.rec(i))
    matcher = TE.StrictMatcher([r for rs in cands.values() for r in rs], TE.NOWHERE, decide)
    code = TE.Code(M.Game.load())
    verdicts = []
    last_at = {}  # (where, number) -> frame where its slot last stopped at code1
    waits = C.Counter()
    below = []
    ghost_verdicts = []
    for k in range(f0 + 1, f1 + 1):
        between = [r for r in runs.between(k) if r.depth == 0 and r.code0 >= 0]
        if ghost_seqs:
            # --ghosts (b1-refine.md, Lemma G): a run on an older instance's slot is a ghost run, not matched
            ghost_verdicts.extend((k, r) for r in between if r.seq in ghost_seqs)
            between = [r for r in between if r.seq not in ghost_seqs]
        if k - 1 not in views or k not in views:
            continue
        # work after the earlier decision point (the sentence script, a room change it makes) is no matched run
        # here: the facts it moved are open at the runs' start (trace-validation.md section 3, the open keys)
        after_dp = [r for r in runs.between(k) if r.frame == k - 1 and r.after]
        open_keys = frozenset()
        if after_dp:
            v0, v1 = views[k - 1], views[k]
            moved = {kk for kk in v0.keys() | v1.keys() if v0.get(kk) != v1.get(kk)}
            units = {r.unit for r in after_dp} | {r.unit for r in runs.between(k)}
            open_keys = frozenset(moved | {f"{p}:{u}" for u in units for p in ("pos", "args", "local", "script",
                                                                               "frozen", "objscript")})
        vs = matcher.match(between, views[k - 1], views[k], open_keys)
        for v in vs:
            r = v.run
            if v.status == "unexplained" and r.code1 == r.code0 and r.ops == 1:
                key = code.key(r.where, r.number, r.room)
                i = code.index(key, r.code0) if key is not None else None
                if i is not None and code.unit(key).ins[i].yields == "wait":
                    v.status = "stutter"  # a blocked WaitFor op that ran again: o5_wait only reads, no step
            verdicts.append((k, v))
            r = v.run
            if v.status == "explained":
                # Lemma W: the frames from the run that left the slot at this wait point to this run (the runs'
                # own frames: a run after a decision point belongs to its frame)
                cost = rd.task.actions[v.record].cost
                j = last_at.get((r.where, r.number, r.room))
                if j is not None and j[1] == r.code0:
                    waits["checked"] += 1
                    if r.frame - j[0] < cost:
                        below.append({"frame": r.frame, "unit": r.unit, "w": r.code0, "waited": r.frame - j[0],
                                      "bound": cost, "record": rd.task.actions[v.record].name})
        for r in runs.between(k):
            if r.code1 >= 0 and r.seq not in ghost_seqs:
                last_at[(r.where, r.number, r.room)] = (r.frame, r.code1)
    counts = C.Counter(v.status for _, v in verdicts)
    unplaced = C.Counter((v.run.unit, v.run.code0) for _, v in verdicts if v.status == "unplaced")
    # what failed, per (unit, wait point), for the unexplained ones; each group's first run explained (section 5)
    fails = C.Counter()
    first_of = {}
    for k, v in verdicts:
        if v.status == "unexplained":
            first = C.Counter(w for _, w in v.why)
            g = (v.run.unit, v.run.code0, first.most_common(1)[0][0] if first else "none")
            fails[g] += 1
            first_of.setdefault(g, (k, v.run))
    causes, invisible = [], []
    first_unplaced = {}
    for k, v in verdicts:
        if v.status == "unplaced":
            first_unplaced.setdefault((v.run.unit, v.run.code0), (k, v.run))
    try:
        for g, n in fails.most_common():
            k, run = first_of[g]
            c = explain_unexplained(code, eng, k, run, g[2], rd, views, dumps)
            causes.append({"unit": g[0], "w": g[1], "why": g[2], "runs": n, "first_frame": k} | c)
        for (u, w), (k, run) in first_unplaced.items():
            invisible.append({"unit": u, "w": w, "runs": unplaced[(u, w)], "first_frame": k} |
                             unplaced_check(code, eng, k, run, rd))
        ghost_checked = []
        for k, run in ghost_verdicts:
            # every ghost run of the trace: α holds on it when it writes no fact of the task (b1-refine.md)
            ghost_checked.append({"unit": run.unit, "slot": run.slot, "frame": run.frame, "w": run.code0,
                                  "end": run.code1} | unplaced_check(code, eng, run.frame, run, rd, slot=run.slot))
    finally:
        eng.close()
    # the matcher's cost when a window search's child asks it only about the units of the plan's steps
    plan_units = {rd.rec(rd.by_name[st["name"]])[1] for st in doc["run"]["steps"] if st["name"] in rd.by_name} - {None}
    m2 = TE.StrictMatcher([r for rs in cands.values() for r in rs if r[1] in plan_units], TE.NOWHERE, decide)
    for k in range(f0 + 1, f1 + 1):
        if k - 1 in views and k in views:
            m2.match([r for r in runs.between(k) if r.depth == 0 and r.code0 >= 0 and r.unit in plan_units],
                     views[k - 1], views[k])
    # the plan's executed event steps: the run that strictly matches each one's record, or why none does
    steps_out = []
    for st in doc["run"]["steps"]:
        name = st["name"]
        i = rd.by_name.get(name)
        if i is None:
            steps_out.append({"step": name, "plan": st["plan"], "outcome": st["outcome"], "strict": "not in the reading"})
            continue
        rec = rd.rec(i)
        lo, hi = (st.get("start") or f0), (st.get("end") or f1)
        if rec[1] is not None:
            mine = [(k, v) for k, v in verdicts if v.run.unit == rec[1] and v.run.code0 == rec[2] and lo <= k <= hi + 1]
            if mine and all(v.status == "stutter" for _, v in mine):
                # a blocked WaitFor op re-ran there: the record's own run is the first one after it that is not
                mine = [next(((k, v) for k, v in verdicts if v.run.unit == rec[1] and v.run.code0 == rec[2]
                              and k > hi + 1 and v.status != "stutter"), mine[0])]
            hit = next(((k, v) for k, v in mine if v.status == "explained" and v.record == i), None)
            if hit is None:
                hit = next(((k, v) for k, v in mine if v.status == "ambiguous" and i in (v.record or ())), None)
            if hit:
                res = {"strict": "matched", "frame": hit[0], "verdict": hit[1].status}
            elif mine:
                k, v = mine[0]
                res = {"strict": "not matched", "frame": k, "verdict": v.status} | record_fails(rec, v.run, views, k)
            else:
                later = next((k for k, v in verdicts if v.run.unit == rec[1] and v.run.code0 == rec[2] and k > hi + 1),
                             None)
                res = {"strict": "its run did not happen", "frames": [lo, hi + 1], "next_run": later}
        else:
            # an input: its record's precondition before its click frame, its effects after it
            iss = st.get("issue")
            first_in = min((e["frame"] for e in st.get("inputs") or []), default=iss)
            if iss is None or iss not in views or first_in - 1 not in views:
                res = {"strict": "not issued"}
            else:
                pre_all = [(kk, str(views[first_in - 1].get(kk))) for kk, vv in rec[3].items()
                           if not TE._holds(views[first_in - 1].get(kk), vv)]
                pre_bad = pre_all[0][0] if pre_all else None
                post = views.get(min(iss + 1, f1), views[iss])
                eff_bad = next((kk for kk, e in rec[4].items() if e[0] in ("set", "guess") and kk in post and not
                                TE.StrictMatcher._value_ok(e, post[kk]) and not kk.startswith("pos:")), None)
                pos_bad = []
                for kk, e in rec[4].items():
                    if kk.startswith("pos:") and e[0] == "set":
                        unit = kk[4:]
                        rr = [r for f in (iss, iss + 1) for r in runs.between(f) if r.unit == unit]
                        end = rr[-1].code1 if rr else None
                        if end is None:
                            pos_bad.append((kk, "no run"))
                        elif end != e[1] and not (end < 0 and e[1] == TE.NOWHERE):
                            pos_bad.append((kk, end))
                ok = pre_bad is None and eff_bad is None and not pos_bad
                res = {"strict": "matched" if ok else "not matched", "frame": iss, "pre_fails": pre_all[:6],
                       "effect": eff_bad, "pos": pos_bad[:3]}
        steps_out.append({"step": name, "plan": st["plan"], "outcome": st["outcome"]} | res)
    per_call = matcher.seconds / max(1, matcher.calls)
    executed = [x for x in steps_out if x["outcome"] in ("matched", "goal")]
    strict_summary = {"loose_matched": len(executed),
                      "strict_matched": sum(1 for x in executed if x["strict"] == "matched"),
                      "inputs_loose": sum(1 for x in executed if not x["step"].startswith("event(")),
                      "inputs_strict": sum(1 for x in executed if not x["step"].startswith("event(")
                                           and x["strict"] == "matched"),
                      "events_without_their_run": sum(1 for x in executed if x["strict"] == "its run did not happen")}
    out = {"run": Path(a.run).name, "reading": Path(a.reading or doc["reading"]).parent.name, "frames": [f0, f1],
           "runs_total": len(runs.runs), "runs_matched_scope": len(verdicts), "verdicts": dict(counts),
           "unexplained_by_point": [[u, w, why, n] for (u, w, why), n in fails.most_common(25)],
           "causes": causes,
           "unplaced_by_point": [[u, w, n] for (u, w), n in unplaced.most_common(25)],
           "unplaced_checked": invisible,
           "matcher_plan_units": {"units": sorted(plan_units), "calls": m2.calls,
                                  "per_call_us": round(m2.seconds / max(1, m2.calls) * 1e6, 1)},
           "open_key_matches": matcher.open_used,
           "ghosts": None if not getattr(a, "ghosts", False) else {
               "units": sorted(ghost_units), "ghost_runs": len(ghost_verdicts),
               "frames_with_a_ghost_slot": len(ghost_frames),
               "invisible": sum(1 for g in ghost_checked if g.get("verdict", "").startswith("invisible")),
               "causes": [g for g in ghost_checked if g.get("cause")],
               "by_unit": dict(C.Counter(g["unit"] for g in ghost_checked))},
           "steps": steps_out, "steps_summary": strict_summary, "wait_check": {"checked": waits["checked"], "below_bound": below[:20],
                                               "below_count": len(below)},
           "matcher": {"calls": matcher.calls, "seconds": round(matcher.seconds, 3),
                       "per_call_us": round(per_call * 1e6, 1),
                       "per_run_us": round(matcher.seconds / max(1, len(verdicts)) * 1e6, 1)},
           "engine_seconds": trace_s, "engine": eng.stats, "reading_load_s": round(load_s, 1)}
    return out


def window_log(d: dict) -> list:
    lines = [f"B1 strict matcher on {d['run']} (reading {d['reading']}), frames {d['frames'][0]}-{d['frames'][1]}: "
             f"{d['runs_total']} slot runs, {d['runs_matched_scope']} resumed at a wait point at depth 0",
             f"verdicts: {d['verdicts']}",
             f"matcher: {d['matcher']['calls']} calls, {d['matcher']['per_call_us']} us per call, "
             f"{d['matcher']['per_run_us']} us per run",
             f"wait check: {d['wait_check']['checked']} explained runs with a known arrival, "
             f"{d['wait_check']['below_count']} waited less than their record's frame cost"]
    for b in d["wait_check"]["below_bound"]:
        lines.append(f"  below: frame {b['frame']} {b['unit']}@{b['w']} waited {b['waited']}, bound {b['bound']}")
    lines.append("unexplained runs by unit and wait point (the condition that failed most often), each group's first "
                 "run explained: the opcode, the engine's values, the class:")
    for c in d["causes"]:
        vals = "; ".join(f"{v['what']} = {v['value']} ({v['kind']})" for v in c.get("values", []))
        where = f"[{c['offset']:04X}] {c.get('opcode')}" if isinstance(c.get("offset"), int) else "-"
        lines.append(f"  {c['runs']}: {c['unit']}@{c['w']}: {c['why']}; first at {c['first_frame']}: {c.get('kind')}; "
                     f"{where}; {vals}; {c.get('class')}")
    lines.append("runs at a wait point the reading has no event record at (unit@W: runs, the first one checked):")
    for c in d["unplaced_checked"]:
        lines.append(f"  {c['runs']}: {c['unit']}@{c['w']}: {c['verdict']}"
                     + (f" ([{c['offset']:04X}] {c.get('opcode')} {c.get('fact', '')} {c.get('value', '')})"
                        if isinstance(c.get("offset"), int) else ""))
    g = d.get("ghosts")
    if g:
        lines.append(f"ghosts (--ghosts, α): {g['ghost_runs']} ghost runs of {len(g['units'])} units, by unit {g['by_unit']}; "
                     f"{g['frames_with_a_ghost_slot']} decision points with an older instance's slot left out; "
                     f"{g['invisible']} ghost runs invisible to the task, {len(g['causes'])} with a cause")
        for c in g["causes"][:10]:
            lines.append(f"  ghost cause: frame {c['frame']} {c['unit']} slot {c['slot']}@{c['w']}: {c.get('verdict')}"
                         + (f" ([{c['offset']:04X}] {c.get('opcode')} {c.get('fact', '')} {c.get('value', '')})"
                            if isinstance(c.get("offset"), int) else ""))
    mp = d["matcher_plan_units"]
    lines.append(f"matcher on the plan's units only ({len(mp['units'])} units): {mp['calls']} calls, "
                 f"{mp['per_call_us']} us per call")
    ss = d["steps_summary"]
    lines.append(f"the plan's steps the run counted as matched: {ss['loose_matched']}; strictly matched (their own run, "
                 f"M1-M4): {ss['strict_matched']}; inputs {ss['inputs_strict']} of {ss['inputs_loose']}; events whose "
                 f"run did not happen there: {ss['events_without_their_run']}")
    lines.append("the plan's executed steps, strictly:")
    for s in d["steps"]:
        extra = {k: v for k, v in s.items() if k not in ("step", "plan", "outcome", "strict")}
        lines.append(f"  plan {s['plan']} {s['step']} ({s['outcome']}): {s['strict']} {extra}")
    return lines


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


def cmd_locate_fact(a) -> dict:
    """Lemma L on a window run: the first frame in (lo, hi] where ``fact`` leaves ``value``, by bisection over forks of
    the run's engine stopped at lo (its chosen inputs, or none after the run's start with ``--idle``), checked by a
    frame-by-frame scan."""
    doc = json.loads(Path(a.run).read_text())
    if a.idle:
        doc = json.loads(json.dumps(doc))
        doc["run"]["inputs"] = [e for e in doc["run"]["inputs"] if e["frame"] <= a.lo]
    eng = WindowEngine(doc, Path(a.bin), a.out / ("locate-idle" if a.idle else "locate"))
    try:
        if a.lo > eng.root.status["frame"]:
            eng.root.run(frame=a.lo)
        stats = {}
        t0 = time.monotonic()
        pred = lambda ch: F.facts_of(ch.state()).get(a.fact, 0) == a.value  # noqa: E731
        lo_ok = pred(eng.root)
        hi_ch = eng.c.fork(eng.root)
        hi_ch.run(frame=a.hi)
        hi_ok = pred(hi_ch)
        hi_ch.quit()
        k = TE.locate(lambda b: eng.c.fork(b), eng.root, a.lo, a.hi, pred, stats) if lo_ok and not hi_ok else None
        bis_s = time.monotonic() - t0
        # the check: every frame, one child
        ch = eng.c.fork(eng.root)
        first = None
        for f in range(a.lo + 1, a.hi + 1):
            ch.run(frame=f)
            if not pred(ch):
                first = f
                break
        ch.quit()
    finally:
        eng.close()
    return {"run": Path(a.run).name, "idle": a.idle, "fact": a.fact, "value": a.value, "lo": a.lo, "hi": a.hi,
            "holds_at_lo": lo_ok, "holds_at_hi": hi_ok, "bisection_frame": k, "scan_frame": first,
            "forks": stats.get("forks", 0), "child_frames": stats.get("frames", 0), "bisection_s": round(bis_s, 2)}


def cmd_causes_tsv(a) -> list:
    """The cause list as one table: per run B miss and per group of unexplained window runs, the unit, the code offset,
    the opcode, the class, the kind and the engine's values; numbers, units and offsets only."""
    rows = [["source", "item", "count", "unit", "offset", "opcode", "class", "kind", "engine values"]]

    def vals(c):
        return "; ".join(f"{v['what']}={v['value']}({v['kind']})" for v in c.get("values", []))

    def off(c):
        return f"{c['offset']:04X}" if isinstance(c.get("offset"), int) else ""
    rb = json.loads((RESULTS / "b1-runb-seed1.json").read_text())
    for c in rb["causes"]:
        rows.append(["run B seed 1", f"{c['move']} {c['fact']}", "1", c.get("unit") or "", off(c), c.get("opcode") or "",
                     c.get("class") or "", c.get("kind") or "", vals(c)])
    for r in rb["rejected"]:
        rows.append(["run B seed 1", f"{r['move']} rejected", "1", "", "", "", "outside V" if r["kind"] == "outside V"
                     else "", r["input"], ""])
    for f in sorted(RESULTS.glob("b1-window-*.json")):
        d = json.loads(f.read_text())
        tag = f.stem.replace("b1-window-", "window ")
        for c in d["causes"]:
            rows.append([tag, f"{c['unit']}@{c['w']} {c['why']}", str(c["runs"]), c.get("unit") or "", off(c),
                         c.get("opcode") or "", c.get("class") or "", c.get("kind") or "", vals(c)])
        for c in d.get("unplaced_checked", []):
            rows.append([tag, f"{c['unit']}@{c['w']} no record", str(c["runs"]), c["unit"], off(c), c.get("opcode") or "",
                         c.get("class") or "invisible", c["verdict"], ""])
    (RESULTS / "b1-causes.tsv").write_text("".join("\t".join(map(str, r)) + "\n" for r in rows))
    return rows


def _plain(o):
    return sorted(o, key=str) if isinstance(o, (set, frozenset)) else str(o)


def write(stem: str, doc: dict, lines: list, results: bool) -> None:
    if results:
        RESULTS.mkdir(exist_ok=True)
        (RESULTS / f"{stem}.json").write_text(json.dumps(doc, indent=1, default=_plain) + "\n")
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
    p.add_argument("--run", dest="run_dir", type=Path, help="a run from run B's route (runb_window.py), not run B")
    p.add_argument("--movie", type=Path, help="that run's explicit-key movie")
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
    p.add_argument("--run", dest="run_dir", type=Path, help="a run from run B's route (runb_window.py), not run B")
    p.add_argument("--movie", type=Path, help="that run's explicit-key movie")
    p = sub.add_parser("window-validate")
    p.add_argument("--run", type=Path, required=True, help="a window search run's result JSON")
    p.add_argument("--ghosts", action="store_true",
                   help="α of E004 b1-refine.md Lemma G: a recursive start's older instances left out (a ghost_slots reading)")
    p.add_argument("--reading", type=Path, help="its reading.json (default: the one the run names)")
    p.add_argument("--bin", required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--tag", required=True)
    p.add_argument("--results", action="store_true")
    sub.add_parser("causes-tsv")
    p = sub.add_parser("locate-fact")
    p.add_argument("--run", type=Path, required=True)
    p.add_argument("--bin", required=True)
    p.add_argument("--fact", required=True)
    p.add_argument("--value", type=int, required=True)
    p.add_argument("--lo", type=int, required=True)
    p.add_argument("--hi", type=int, required=True)
    p.add_argument("--idle", action="store_true", help="no input after the start (the engine idles)")
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--results", action="store_true", help="append the result to results/b1-locate.jsonl")
    a = ap.parse_args()
    if a.cmd == "locate-fact":
        d = cmd_locate_fact(a)
        print(json.dumps(d))
        if a.results:
            with open(RESULTS / "b1-locate.jsonl", "a") as fh:
                fh.write(json.dumps(d) + "\n")
    elif a.cmd == "causes-tsv":
        rows = cmd_causes_tsv(a)
        print(f"{len(rows) - 1} rows")
    elif a.cmd == "window-validate":
        doc = cmd_window_validate(a)
        (a.out / f"window-{a.tag}.json").write_text(json.dumps(doc, indent=1, default=_plain) + "\n")
        write(f"b1-window-{a.tag}", doc, window_log(doc), a.results)
    elif a.cmd == "runb-explain":
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
