"""E003, D7: golden dumps of the Python Machine (model.py) and Reader (reader.py), the oracle a port must match.

usage (from this folder, with ../../.venv/bin/python, under nice):
       golden.py machine [--runs row0,row1,...] [--jobs 4] [--out DIR] [--no-trace] [--only S1#4,...]
                                                        (the model check's runs; --only: some moves, a partial index)
       golden.py game [--out DIR]                       (the Game's derived tables: loops, wait points, flags)
       golden.py reader-room33 [--out DIR]              (room 33 alone from start-part1 seed 1: 878 records, seconds)
       golden.py reader-kitchen [--out DIR] [--workers 4]   (test_reader.py's kitchen reading, read again)
       golden.py reader-pickle PICKLE --out FILE        (an old pickled reading, dumped the same way)
       golden.py compare CANDIDATE_DIR [--golden DIR] [--runs ...] [--traces] [--all]
       golden.py compare-file GOLDEN_FILE CANDIDATE_FILE
       golden.py format                                 (prints this text)

Goldens go to ``out/golden/`` (git-ignored) unless ``--out`` says otherwise. Nothing here changes model.py, reader.py
or check.py: the tracer is a subclass of ``model.Machine`` (``TraceMachine``) and check.py's ``run_model`` is wrapped
in this process only, to keep the Run it builds.

**What is dumped.** For each run of the model check (``RUNS``: growth rows 0-5, ``model.ROWS``, and the three switch
runs: const_polls and input_after_turns on row 4, esc_key_at_pre on row 5), every counted move of the ground truth
(239 of 250 rows; check.py's ``classify`` decides the action, run B's ``VIA`` moves included) is scored exactly as
check.py scores it (``check.score_move``, one Machine per run, moves on forked workers as ``check.score_parallel``
does), and the dump keeps:

* the Run the machine returned (``run`` line) and every Path in it, in ``run.paths`` order (``path`` lines): every
  dataclass field of ``model.Path`` but four, listed below;
* ``model.predict(run, oracle)`` (``predict`` line);
* check.py's record of the move (``check`` line): the dict check.py writes for the move (header and
  ``score_move``'s output), typed, and the exact JSON line check.py writes to check-part1-rowN.jsonl;
* a turn-level trace of the run (the ``.trace`` file, below).

A port that gives the same ``run`` and ``path`` lines for every move gives check.py the same input: check.py reads
nothing else from a run (``CHECK_READS`` lists the fields and where check.py reads them), and ``predict`` and the
``check`` line follow from them.

**Files.** ``machine/<run>/<label>.jsonl.gz`` per counted move (label: S1#2, ...), ``machine/<run>/<label>.trace.jsonl.gz``
(its trace), ``machine/<run>/index.json`` (the moves in order, each file's sha256 over its uncompressed text, sizes,
CPU seconds, peak RSS, paths, events), ``game.jsonl.gz``, ``reader/room33.jsonl.gz``, ``reader/kitchen.jsonl.gz``,
``manifest.json`` (the sha1 of every module imported, the Python, the ground truth's sha256, run times, sizes,
PYTHONHASHSEED), ``FORMAT.txt`` (this text). Gzip files are written with mtime 0 and no name, so the same text gives
the same bytes.

**Canonical encoding** (``enc``, ``dumps``). Each line is one JSON object, compact (separators "," and ":"), keys
sorted (code point order), ASCII only (non-ASCII as \\uXXXX), no NaN. Python values map as follows:

* None -> null; bool -> true / false; int -> a JSON integer; str -> a JSON string. JSON keeps true apart from 1, and the
  compare checks types (a bool never equals an int). A float would be {"$float": repr}; none occurs.
* list -> a JSON array, in order. tuple -> {"$tuple": [...]}, in order.
* set and frozenset -> {"$set": [...]}, sorted (the order rule below).
* dict whose keys are all str -> a JSON object (keys sorted). Any other dict -> {"$map": [[key, value], ...]}, sorted by
  key (the order rule). An empty dict is {} whatever its key type. No str key starts with "$" (checked).
* a dataclass instance -> {"$<ClassName>": {field: value, ...}} with every field (``model.Unknown`` is
  {"$Unknown": {"reason", "what", "values"}}, reader's ``Cond`` and ``Copy`` are {"$Cond": ...} and {"$Copy": ...},
  ``model.Frame`` {"$Frame": ...}, ``model.Call`` {"$Call": ...}, reader's ``Event`` and ``Record`` the same way).
* Order rule for $set elements and $map keys (taken after encoding): all ints -> numeric order; all strs -> code point
  order; anything else -> by the element's compact canonical JSON text, compared as strings.
* No value rests on id(), a memory address or hash(): the Path fields that do are left out (below).

**Machine file lines** (``<label>.jsonl.gz``), in this order:

1. {"kind": "move", "move": {"run", "features", "switches", "i", "label", "name", "status", "reason", "action_kind",
   "action", "key"}}. ``key``: a hash of the sources, the run and the format (a rerun reuses a file only under it).
2. {"kind": "run", "run": {"start", "limit", "unit", "merges", "value_caps", "settle_merges", "value_merges",
   "pre_slots", "key_routes", "n_paths"}} (absent when the move has no action: the machine did not run).
3. {"kind": "path", "index": k, "path": {...}} per path, k = 0.. in ``run.paths`` order (the order check.py's counters
   see). Every field of ``model.Path`` except ``seen`` (hashes of states), ``round_slots`` (a hash), ``joined`` (a
   join signature holding id() values and hashes) and ``history`` (hashes and stores; its length is kept as
   ``history_len``). ``tid``: the path's id in the trace (null when the trace never named it). ``slots`` is kept as
   a list of [name, Frame] pairs in the dict's insertion order, because that order is part of the machine's state
   (``Machine._slots_sig`` reads it in order, and merges rest on it); every other dict is sorted by key.
4. {"kind": "predict", "predict": {fact: {"certain", "values", "unknown", "pre"}}} (absent without a run).
5. {"kind": "check", "record": {...}, "line": "..."}: the record check.py keeps for the move, typed, and the exact
   line ``json.dumps(m, default=str)`` check.py writes for it.

**Trace file lines** (``<label>.trace.jsonl.gz``): one event per line, in the order they happened in the run:
{"seq", "tid", "type", "steps", "na", ...}. ``tid`` is a path id: ids are given in the order paths first appear in
an event (the run's first path is 0), a fork gives each new path the next id at once (also forks the path limit then
cuts: their ids never appear again and the parent ends "limit"), and a merge of two or more paths makes a new path with
the next id. ``steps`` is the path's ``steps`` and ``na`` its number of assumptions when the event was recorded.
Types and their fields:

* start {start, unit, top}: ``_drive`` begins (``top``: the first path's top of stack, ``where``).
* fork {children, assume, where} on the parent: ``_advance`` returned new paths. ``children``: [[tid, assume], ...]
  per new path; ``assume``: the last assumption the path holds then (the parent's may be undone by the path limit);
  ``where``: the top of the parent's stack after the step, a frame's [name, pc] or a call's [head, step].
* merge {tids, round, where} on the new path: ``_merge`` folded two or more paths waiting at one join (``tids``:
  theirs, in group order). A path that waited at a join alone and goes on gets no event.
* push {sid, unit, name, pc, offset, nested, by}: ``_push_frame`` started a frame (``by``: the call's head).
* turn {name, sid, unit, pc, offset, rule, round} / turn {name, ran: false, why: "didexec"}: ``_eng_turn`` ran a slot
  (pc after a WaitFor op it falls through) or passed it over because it ran in this frame already.
* skip {name, why, origin, round}: ``_skip`` (why "" is a dead slot or room code that is not running).
* finish {sid, name, how, at, resume, pc}: ``_finish`` ended a frame's turn (yield, parked, end, stopped, chained,
  room); ``resume`` the offset passed in, ``pc`` the frame's index when it ended.
* end {reason, at, resume}: row 0's ``_end``. halt {reason, at}: ``_halt``. error {what, cite, detail, fatal}.
* round {round, order}: ``_next_round`` started a round (``order``: the slots it gives a turn, in order);
  settled {end, end_at, round}: it ended the path (settled, cycle, round limit).
* sentence {verb, a, b, first_end, end, queue}: ``_eng_sentence`` after it ran. queue {before, after, round}:
  ``_eng_sentences`` with a non-empty queue. input {area, verb, button, drift}: ``_eng_input``. abort {armed, code}:
  ``_eng_abort``. key {fate}: ``_key_route``.

**Game file** (``game.jsonl.gz``): one {"kind": "game", "game": {...}} line (first_local, local_rooms, object_room,
object_parent, verb_offset, room_width, pseudo, pseudo_conflicts, resistance, wild_starts, nonfacts =
``non_fact_vars``, script_writes_count, const_poll_loops), then one {"kind": "unit", ...} line per unit in
``game.units`` order: its key, name, join_points, loops (``Game.loops`` with const_polls off and on), time_exits
(camera off and on, const_polls off and on: {"$map": [[jump index, {"$tuple": [exit, head]}], ...]}), and wait_points: [index, kind,
op, innermost loop's head or null] for every index 0..len(ins) (``Game.wait_point``; the same with const_polls on,
checked).

**Reader files** (``reader/<name>.jsonl.gz``): {"kind": "reading", "reading": {...}} (the scope, the start's source,
the sources' sha1), then {"kind": "record", "index": i, "record": {"$Record": {...}}} per record in ``read_all``'s
order, then {"kind": "builder", "builder": {...}}: the Builder's and the Reader's counters (``counts``, ``ends``,
``drops``, ``nonfact_checks``, ``guesses``, ``unknown_writes``, ``unknown_sites``, ``out_of_domain``, ``rd.counts``,
``rd.forks``, ``rd.fork_keys``; the timing counters ``seconds`` and ``slow`` are left out).

**Compare** (``compare``): reads CANDIDATE_DIR with the same layout (``.jsonl`` or ``.jsonl.gz``), runs in ``ORDER``
and moves in move order, and prints the first difference: run, move, line (its kind and path index) and the field,
as a path into the JSON (e.g. path[3].assumptions[5].$tuple[2]), with both values. A file whose uncompressed text has
the golden's sha256 is equal without parsing. Values are compared with their JSON types (true is not 1). ``--traces``
also compares the trace files the candidate has; ``--all`` prints the first difference of every differing move.
Exit status 0 when nothing differs, 1 otherwise.
"""

import argparse
import collections
import contextlib
import dataclasses
import gzip
import hashlib
import io
import json
import multiprocessing
import os
import shutil
import sys
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / "src"))

OUT = ROOT / "out" / "golden"
FORMAT_VERSION = 1
FROZEN = {"model.py": "cc630446bdbe", "reader.py": "94d495853f07", "ir.py": "c6ccf01059f1",
          "facts.py": "c06fc3489018", "actions.py": "ddbd1aaa2207"}
STOP_ON_CHANGE = ("model.py", "reader.py")  # D7: if these change while the goldens are made, stop
KITCHEN_ROOMS, KITCHEN_MOVE = (28, 41), "S1#7"  # test_reader.py's demo (ROOMS, MOVE)
ROOM33, ROOM33_SEED = (33,), 1  # size_scaling.py's scope 0 (seed 1's start room)
MIN_FREE_MEM_GB, MIN_FREE_DISK_GB = 3.0, 2.0
TRACE = True

# --- the runs ------------------------------------------------------------------------------------------------------

ROW_FEATURES = {n: frozenset(f for g in __import__("model").ROWS[:n + 1] for f in g) for n in range(6)}
RUNS = {f"row{n}": (ROW_FEATURES[n], frozenset()) for n in range(6)}
RUNS.update({"row4-const_polls": (ROW_FEATURES[4], frozenset({"const_polls"})),
             "row4-input_after_turns": (ROW_FEATURES[4], frozenset({"input_after_turns"})),
             "row5-esc_key_at_pre": (ROW_FEATURES[5], frozenset({"esc_key_at_pre"}))})
ORDER = ["row0", "row1", "row2", "row3", "row4", "row5", "row4-const_polls", "row4-input_after_turns",
         "row5-esc_key_at_pre"]
PATH_SKIP = ("seen", "round_slots", "joined", "history")  # Path fields that rest on hash() or id() (module docstring)

# What check.py reads from a run (check.py at the sha1 the manifest gives; its functions and lines): the field list
# a port must reproduce, all of it in the run and path lines.
CHECK_READS = {
    "Run.start": "score_move (out['start']); summarize: '(after the turn of' in it",
    "Run.paths": "score_move: len, every counter below, model.feasible and model.predict; their order is the order of "
                 "the dict counters (ends, notes, turns, errors, first_ends) and of next() in settle_stats",
    "Run.limit": "score_move (out['limit'])",
    "Run.value_caps": "score_move (out['value_caps'])",
    "Run.merges": "score_move (out['merges'])",
    "Run.unit": "click_stats (the input script's unit and file), esc_stats (the rewound unit)",
    "Run.settle_merges": "settle_stats",
    "Run.value_merges": "settle_stats",
    "Run.pre_slots": "settle_stats (not_run, in this order)",
    "Run.key_routes": "esc_stats (fate, and fork_kind of each assumption's why)",
    "Path.end": "score_move (ends, cut, dead, infeasible), model.feasible, settle_stats (settled_moving), click_stats "
                "and esc_stats ('did not run (end)')",
    "Path.end_at": "settle_stats ('locals still moved' in it)",
    "Path.unmodelled": "score_move ((statement, cite) pairs)",
    "Path.notes": "score_move (statement counts), esc_stats ('Esc key' notes)",
    "Path.unknown_writes": "score_move (count), click_stats (cite under the input script's file)",
    "Path.assumptions": "score_move (count, WaitFor sites, kinds by (cite, condition, side) and why), settle_stats "
                        "(loop exits and waits by why prefix), click_stats (forks at the input script's cites)",
    "Path.turns": "score_move (counts by how), click_stats and esc_stats (the named unit's first turn)",
    "Path.depth": "score_move (max)",
    "Path.reentries": "score_move (sum)",
    "Path.errors": "score_move (counts, sorted sites)",
    "Path.steps": "score_move (max)",
    "Path.writes": "model.predict (feasible paths; Unknown.values), settle_stats (loosened: Unknown.reason and what; "
                   "mouse keys), after_keys",
    "Path.first_end": "settle_stats (str() counts)",
    "Path.round": "settle_stats (max)",
    "Path.sentences": "settle_stats (max)",
    "Path.ran_by": "settle_stats, click_stats (rule 'c' slots)",
    "Path.not_run": "settle_stats",
    "Path.skips": "settle_stats",
    "Path.multi": "settle_stats",
    "Path.ambient_slots": "settle_stats",
    "Path.conflicts": "settle_stats (whole tuples, sorted; their key tuples)",
    "Path.immediate": "after_keys (settle_stats' split)",
    "Path.after": "after_keys",
    "Path.writer": "settle_stats' split (the rule of the turn that last wrote a key)",
    "Path.input_drift": "click_stats",
    "model.predict(run, oracle)": "score_move (certain and possible predictions): from the feasible paths' writes",
}


# --- canonical encoding --------------------------------------------------------------------------------------------

def _text(v) -> str:
    return json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False)


def _order(items: list) -> list:
    """The order rule (module docstring) on encoded values."""
    if all(type(v) is int for v in items) or all(type(v) is str for v in items):
        return sorted(items)
    return sorted(items, key=_text)


def enc(x):
    """A Python value as canonical, typed JSON data (module docstring)."""
    if x is None or isinstance(x, (bool, str)):
        return x
    if isinstance(x, int):
        return int(x)
    if isinstance(x, float):
        return {"$float": repr(x)}
    if isinstance(x, list):
        return [enc(v) for v in x]
    if isinstance(x, tuple):
        return {"$tuple": [enc(v) for v in x]}
    if isinstance(x, (set, frozenset)):
        return {"$set": _order([enc(v) for v in x])}
    if isinstance(x, dict):
        if all(type(k) is str for k in x):
            for k in x:
                if k.startswith("$"):
                    raise ValueError(f"a dict key starts with '$': {k!r}")
            return {k: enc(v) for k, v in x.items()}
        pairs = [[enc(k), enc(v)] for k, v in x.items()]
        keys = _order([k for k, _ in pairs])
        by = {_text(k): p for k, p in zip((p[0] for p in pairs), pairs)}
        return {"$map": [by[_text(k)] for k in keys]}
    if dataclasses.is_dataclass(x) and not isinstance(x, type):
        return {"$" + type(x).__name__: {f.name: enc(getattr(x, f.name)) for f in dataclasses.fields(x)}}
    raise TypeError(f"no canonical encoding for {type(x).__name__}: {x!r}")


def dumps(x) -> str:
    """One canonical line (without the newline)."""
    return _text(enc(x))


def line(obj: dict) -> str:
    """A line whose top level is already a dict of encoded parts plus plain keys."""
    return _text(obj)


class GzWriter:
    """Writes lines to ``path`` (gzip, mtime 0, no name) through a temporary file; keeps the sha256 and size of the
    uncompressed text."""

    def __init__(self, path: Path):
        self.path = Path(path)
        self.tmp = self.path.with_name(self.path.name + f".{os.getpid()}.tmp")
        self.raw = open(self.tmp, "wb")
        self.gz = gzip.GzipFile(filename="", mode="wb", fileobj=self.raw, mtime=0, compresslevel=6)
        self.h, self.size, self.lines = hashlib.sha256(), 0, 0

    def write(self, text: str) -> None:
        b = (text + "\n").encode("ascii")
        self.gz.write(b)
        self.h.update(b)
        self.size += len(b)
        self.lines += 1

    def close(self) -> dict:
        self.gz.close()
        self.raw.close()
        os.replace(self.tmp, self.path)
        return {"sha256": self.h.hexdigest(), "bytes": self.size, "gz_bytes": self.path.stat().st_size,
                "lines": self.lines}


def open_lines(path: Path):
    """The lines of a golden or candidate file (.jsonl.gz or .jsonl), as text."""
    path = Path(path)
    if path.suffix == ".gz":
        with gzip.open(path, "rt", encoding="ascii") as fh:
            return fh.read().splitlines()
    return path.read_text(encoding="ascii").splitlines()


def file_sha(path: Path) -> str:
    h = hashlib.sha256()
    for ln in open_lines(path):
        h.update((ln + "\n").encode("ascii"))
    return h.hexdigest()


# --- sources, resources --------------------------------------------------------------------------------------------

def sha1(path: Path) -> str:
    return hashlib.sha1(Path(path).read_bytes()).hexdigest()


def frozen_check(stop: bool = True) -> dict:
    """The frozen sources' sha1 now; with ``stop``, exit when model.py or reader.py is not the frozen one."""
    now = {n: sha1(HERE / n) for n in FROZEN}
    bad = {n: now[n][:12] for n in FROZEN if now[n][:12] != FROZEN[n]}
    if bad and stop and any(n in STOP_ON_CHANGE for n in bad):
        raise SystemExit(f"frozen sources changed: {bad} (expected {FROZEN}); stopping (D7)")
    if bad:
        print(f"warning: sources differ from the frozen ones: {bad}", file=sys.stderr)
    return now


def imported_modules() -> dict:
    """{module file relative to the repo: sha1} for every module imported from the repo (sources this process read)."""
    out = {}
    for m in list(sys.modules.values()):
        f = getattr(m, "__file__", None)
        if not f:
            continue
        p = Path(f).resolve()
        try:
            rel = p.relative_to(ROOT)
        except ValueError:
            continue
        if ".venv" in rel.parts or p.suffix != ".py":
            continue
        out[rel.as_posix()] = sha1(p)
    return dict(sorted(out.items()))


def free_gb() -> tuple[float, float]:
    mem = 0.0
    for ln in Path("/proc/meminfo").read_text().splitlines():
        if ln.startswith("MemAvailable:"):
            mem = int(ln.split()[1]) / 1024 / 1024
    du = shutil.disk_usage(ROOT)
    return mem, du.free / 1024 ** 3


def guard(what: str) -> None:
    mem, disk = free_gb()
    if mem < MIN_FREE_MEM_GB or disk < MIN_FREE_DISK_GB:
        raise SystemExit(f"{what}: free memory {mem:.1f} GB, free disk {disk:.1f} GB: under the limits "
                         f"({MIN_FREE_MEM_GB} GB, {MIN_FREE_DISK_GB} GB); stopping")


# --- the tracer ----------------------------------------------------------------------------------------------------

import model  # noqa: E402  (after sys.path)


def _where(p) -> list | None:
    """The top of a path's stack: [unit name, pc] for a frame, [call head, step] for a call."""
    top = p.stack[-1] if p.stack else None
    if isinstance(top, model.Frame):
        return [model.unit_name(top.unit), top.pc]
    if isinstance(top, model.Call):
        return [top.head, top.i]
    return None


class TraceMachine(model.Machine):
    """model.Machine with a turn-level event log of the current run (``events``; module docstring, trace). It
    overrides only to record: each override calls the Machine's method and returns what it returns."""

    FLUSH = 20000  # events kept in memory before they go to ``sink``

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.events, self._next_tid, self._outer, self.seq0 = [], 0, 0, 0
        self.tracing = TRACE
        self.sink = None  # a callable (events, first seq) that writes them; None keeps them all in ``events``

    # bookkeeping
    def _tid(self, p) -> int:
        t = p.__dict__.get("_tid")
        if t is None:
            t = p.__dict__["_tid"] = self._next_tid
            self._next_tid += 1
        return t

    def _ev(self, p, typ: str, **kw) -> None:
        if self.tracing:
            self.events.append((self._tid(p), typ, p.steps, len(p.assumptions), kw))
            if self.sink is not None and len(self.events) >= self.FLUSH:
                self.flush()

    def flush(self) -> None:
        if self.sink is not None and self.events:
            self.sink(self.events, self.seq0)
            self.seq0 += len(self.events)
            self.events = []

    @contextlib.contextmanager
    def _entry(self):
        if self._outer == 0:
            self.events, self._next_tid, self.seq0 = [], 0, 0
        self._outer += 1
        try:
            yield
        finally:
            self._outer -= 1

    # entry points: a run's events start empty
    def run(self, *a, **k):
        with self._entry():
            return super().run(*a, **k)

    def run_entry(self, *a, **k):
        with self._entry():
            return super().run_entry(*a, **k)

    def run_sentence(self, *a, **k):
        with self._entry():
            return super().run_sentence(*a, **k)

    def run_wait(self, *a, **k):
        with self._entry():
            return super().run_wait(*a, **k)

    def run_verb(self, *a, **k):
        with self._entry():
            return super().run_verb(*a, **k)

    def run_esc(self, *a, **k):
        with self._entry():
            return super().run_esc(*a, **k)

    # the run
    def _drive(self, oracle, first, start, unit):
        self._ev(first, "start", start=start, unit=unit, top=_where(first))
        return super()._drive(oracle, first, start, unit)

    def _advance(self, p, oracle):
        more = super()._advance(p, oracle)
        if more and self.tracing:
            self._tid(p)
            kids = [[self._tid(q), q.assumptions[-1] if q.assumptions else None] for q in more]
            self._ev(p, "fork", children=kids, assume=p.assumptions[-1] if p.assumptions else None, where=_where(p))
        return more

    def _merge(self, group, sig):
        tids = [self._tid(g) for g in group]
        q = super()._merge(group, sig)
        if len(group) > 1:
            q.__dict__.pop("_tid", None)  # a new path (base.fork()); dataclasses.replace gave it no id
            self._ev(q, "merge", tids=tids, round=q.round, where=_where(q))
        return q

    # frames and turns
    def _push_frame(self, p, call, *args, **kwargs):
        n = len(p.stack)
        super()._push_frame(p, call, *args, **kwargs)
        if len(p.stack) > n and isinstance(p.stack[-1], model.Frame):
            f = p.stack[-1]
            u = self.game.units[f.unit]
            self._ev(p, "push", sid=f.sid, unit=f.unit, name=model.unit_name(f.unit), pc=f.pc,
                     offset=u.ins[f.pc].offset if f.pc is not None and f.pc < len(u.ins) else None,
                     nested=f.nested, by=call.head)

    def _eng_turn(self, p, call, oracle, name):
        n, before = len(p.stack), name in p.round_ran
        out = super()._eng_turn(p, call, oracle, name)
        if len(p.stack) > n and isinstance(p.stack[-1], model.Frame):
            g = p.stack[-1]
            u = self.game.units[g.unit]
            self._ev(p, "turn", name=name, sid=g.sid, unit=g.unit, pc=g.pc,
                     offset=u.ins[g.pc].offset if g.pc is not None and g.pc < len(u.ins) else None,
                     rule=p.owner_rule, round=p.round)
        elif before:
            self._ev(p, "turn", name=name, ran=False, why="didexec")
        return out

    def _skip(self, p, name, why):
        origin = self._origin(p, name) if why else None
        super()._skip(p, name, why)
        self._ev(p, "skip", name=name, why=why, origin=origin, round=p.round)

    def _finish(self, p, how, at, resume=None):
        f = p.stack[-1]
        super()._finish(p, how, at, resume)
        self._ev(p, "finish", sid=f.sid, name=model.unit_name(f.unit), how=how, at=at, resume=resume, pc=f.pc)

    def _end(self, p, reason, at, *a, **k):
        resume = k.get("resume", a[1] if len(a) > 1 else None)
        super()._end(p, reason, at, *a, **k)
        self._ev(p, "end", reason=reason, at=at, resume=resume)

    def _halt(self, p, reason, at):
        super()._halt(p, reason, at)
        self._ev(p, "halt", reason=reason, at=at)

    def _error(self, p, what, cite, detail, fatal=False):
        super()._error(p, what, cite, detail, fatal)
        self._ev(p, "error", what=what, cite=cite, detail=detail, fatal=fatal)

    # settle
    def _next_round(self, p):
        super()._next_round(p)
        if p.end is not None:
            self._ev(p, "settled", end=p.end, end_at=p.end_at, round=p.round)
        else:
            top = p.stack[-1]
            self._ev(p, "round", round=p.round, order=[s[1] for s in top.steps if s[0] == "turn"])

    def _eng_sentence(self, p, call, oracle, verb, a, b):
        out = super()._eng_sentence(p, call, oracle, verb, a, b)
        self._ev(p, "sentence", verb=verb, a=a, b=b, first_end=p.first_end, end=p.end, queue=len(p.queue))
        return out

    def _eng_sentences(self, p, call, oracle):
        before = len(p.queue)
        out = super()._eng_sentences(p, call, oracle)
        if before:
            self._ev(p, "queue", before=before, after=len(p.queue), round=p.round)
        return out

    def _eng_input(self, p, call, oracle, area, val, button):
        out = super()._eng_input(p, call, oracle, area, val, button)
        self._ev(p, "input", area=area, verb=val, button=button, drift=p.input_drift)
        return out

    def _eng_abort(self, p, call, oracle, armed, code):
        out = super()._eng_abort(p, call, oracle, armed, code)
        self._ev(p, "abort", armed=armed, code=code)
        return out

    def _key_route(self, p, fate):
        super()._key_route(p, fate)
        self._ev(p, "key", fate=fate)


# --- machine goldens -----------------------------------------------------------------------------------------------

def path_dict(p) -> dict:
    """A Path's fields as dumped (module docstring)."""
    out = {}
    for f in dataclasses.fields(model.Path):
        if f.name in PATH_SKIP:
            continue
        v = getattr(p, f.name)
        if f.name == "slots":
            out["slots"] = [[enc(n), enc(fr)] for n, fr in v.items()]
        else:
            out[f.name] = enc(v)
    out["history_len"] = len(p.history)
    out["tid"] = p.__dict__.get("_tid")
    return out


def run_dict(run) -> dict:
    return {"start": enc(run.start), "limit": run.limit, "unit": enc(run.unit), "merges": run.merges,
            "value_caps": enc(run.value_caps), "settle_merges": run.settle_merges, "value_merges": run.value_merges,
            "pre_slots": enc(run.pre_slots), "key_routes": enc(run.key_routes), "n_paths": len(run.paths)}


def event_line(seq: int, e: tuple) -> str:
    tid, typ, steps, na, kw = e
    d = {"seq": seq, "tid": tid, "type": typ, "steps": steps, "na": na}
    for k, v in kw.items():
        d[k] = enc(v)
    return line(d)


_CAP = {}  # what check.run_model built for the move being scored (``_capture``)
_JOB = {}  # what a forked worker reads (``machine_run``)


def _capture(orig):
    """check.run_model, keeping the Run, the oracle and the trace (written to ``machine.trace_path`` as it comes)."""
    def run_model(machine, action, oracle):
        machine.events, machine.seq0 = [], 0
        tw = GzWriter(machine.trace_path) if getattr(machine, "trace_path", None) else None
        if tw is not None:
            machine.sink = lambda evs, seq0: [tw.write(event_line(seq0 + i, e)) for i, e in enumerate(evs)]
        try:
            run = orig(machine, action, oracle)
            if tw is not None:
                machine.flush()
        finally:
            machine.sink = None
        n = machine.seq0 + len(machine.events)
        _CAP.update(run=run, oracle=oracle, n_events=n, trace=tw.close() if tw is not None else None)
        machine.events = []
        return run
    return run_model


def _key(name: str, features, switches, gt: str, trace: bool) -> str:
    src = {n: sha1(HERE / n) for n in ("model.py", "ir.py", "facts.py", "actions.py", "check.py", "groundtruth.py",
                                       "parallel.py", "golden.py")}
    return hashlib.sha256(json.dumps([FORMAT_VERSION, name, sorted(features), sorted(switches), gt, src, trace,
                                      sys.version]).encode()).hexdigest()[:16]


def _score_job(k: int) -> dict:
    """A worker's move: score it as check.py does, write its files, return its stats."""
    j = _JOB
    import check
    row, (m, action) = j["rows"][k], j["heads"][k]
    m = dict(m)
    machine = j["machine"]
    _CAP.clear()
    machine.trace_path = j["folder"] / f"{m['label']}.trace.jsonl.gz" if j["trace"] else None
    t0 = time.process_time()
    parallel = sys.modules["parallel"]
    parallel._reset_peak()
    out = check.score_move(row, action, machine, j["nonfacts"], j["kinds"])
    cpu = time.process_time() - t0
    rss = parallel._peak_mb()
    m |= out
    chk_line = json.dumps(m, default=str)
    folder, label = j["folder"], m["label"]
    head = {"run": j["name"], "features": sorted(j["features"]), "switches": sorted(j["switches"]), "i": m["i"],
            "label": label, "name": m["name"], "status": m["status"], "reason": m["reason"],
            "action_kind": m["kind"], "action": m["action"], "key": j["key"]}
    w = GzWriter(folder / f"{label}.jsonl.gz")
    w.write(line({"kind": "move", "move": enc(head)}))
    run = _CAP.get("run")
    if run is not None:
        w.write(line({"kind": "run", "run": run_dict(run)}))
        for i, p in enumerate(run.paths):
            w.write(line({"kind": "path", "index": i, "path": path_dict(p)}))
        w.write(line({"kind": "predict", "predict": enc(model.predict(run, _CAP["oracle"]))}))
    w.write(line({"kind": "check", "record": enc(m), "line": chk_line}))
    stat = {"label": label, "i": m["i"], "file": w.close(), "cpu_s": round(cpu, 3), "peak_rss_mb": round(rss, 1),
            "paths": len(run.paths) if run is not None else 0, "events": _CAP.get("n_events", 0), "key": j["key"]}
    if _CAP.get("trace") is not None:
        stat["trace"] = _CAP["trace"]
    return stat


def _first_line(path: Path) -> dict | None:
    try:
        with gzip.open(path, "rt", encoding="ascii") as fh:
            return json.loads(fh.readline())
    except (OSError, EOFError, ValueError):
        return None


def machine_run(name: str, out_root: Path, jobs: int, trace: bool, rows, inputs, kinds, log, only=None) -> dict:
    """One run (``RUNS``): every counted move's files under ``out_root/machine/name`` (``only``: these labels alone, a
    partial index); returns the run's summary."""
    import check
    import parallel
    features, switches = RUNS[name]
    folder = out_root / "machine" / name
    folder.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    machine = TraceMachine(features, switches=switches)
    machine.tracing = trace
    nonfacts = model.non_fact_vars(machine.game)
    gt = check.gt_digest()
    key = _key(name, features, switches, gt, trace)
    heads = [check.header(row, machine.game, inputs) for row in rows]
    stats_path = folder / "stats.jsonl"
    old = {}
    if stats_path.exists():
        for ln in stats_path.read_text().splitlines():
            d = json.loads(ln)
            old[d["label"]] = d
    todo, done = [], {}
    for k, (m, _) in enumerate(heads):
        if m["status"] != "counted" or (only and m["label"] not in only):
            continue
        lab = m["label"]
        st = old.get(lab)
        ok = st is not None and st.get("key") == key and (folder / f"{lab}.jsonl.gz").exists() \
            and (not trace or st.get("paths") == 0 or (folder / f"{lab}.trace.jsonl.gz").exists())
        if ok:
            first = _first_line(folder / f"{lab}.jsonl.gz")
            ok = first is not None and first.get("move", {}).get("key") == key
        if ok:
            done[k] = st
        else:
            todo.append(k)
    row_n = model.row_of(features)
    prior = parallel.read_times([HERE / "results" / f"check-part1-row{r}.times.tsv"
                                 for r in range(min(row_n, 3), -1, -1)])
    todo.sort(key=lambda k: -prior.get(heads[k][0]["label"], 1e9))
    log(f"[{name}] {len(todo)} moves to score, {len(done)} kept from an earlier run (key {key})")
    if todo:
        _JOB.update(rows=rows, heads=heads, machine=machine, nonfacts=nonfacts, kinds=kinds, folder=folder,
                    name=name, features=features, switches=switches, key=key, trace=trace)
        stats_fh = open(stats_path, "a")
        try:
            with ProcessPoolExecutor(max_workers=min(jobs, len(todo)),
                                     mp_context=multiprocessing.get_context("fork")) as ex:
                futs = {ex.submit(_score_job, k): k for k in todo}
                n = 0
                try:
                    for f in as_completed(futs):
                        st = f.result()
                        done[futs[f]] = st
                        stats_fh.write(json.dumps(st) + "\n")
                        stats_fh.flush()
                        n += 1
                        if st["cpu_s"] > 20 or n % 40 == 0:
                            mem, disk = free_gb()
                            log(f"[{name}] {n}/{len(todo)} {st['label']} {st['cpu_s']:.0f} s, {st['paths']} paths, "
                                f"{st['events']} events; free memory {mem:.1f} GB, disk {disk:.1f} GB")
                        guard(name)
                except BaseException:
                    for f in futs:
                        f.cancel()
                    raise
        finally:
            stats_fh.close()
            _JOB.clear()
    moves = []
    for k, (m, _) in enumerate(heads):
        if only and m["label"] not in only:
            continue
        if m["status"] != "counted":
            moves.append({"label": m["label"], "i": m["i"], "status": m["status"], "reason": m["reason"]})
        else:
            st = dict(done[k])
            st.pop("key", None)
            moves.append({"status": "counted", **st})
    files = [m for m in moves if m["status"] == "counted"]
    summary = {"run": name, "features": sorted(features), "switches": sorted(switches), "key": key,
               "moves": len(files), "excluded": len(moves) - len(files),
               "gz_bytes": sum(m["file"]["gz_bytes"] + m.get("trace", {}).get("gz_bytes", 0) for m in files),
               "bytes": sum(m["file"]["bytes"] + m.get("trace", {}).get("bytes", 0) for m in files),
               "cpu_s": round(sum(m["cpu_s"] for m in files), 1), "wall_s": round(time.time() - t0, 1),
               "check_py": sha1(HERE / "check.py"), "partial": bool(only)}
    (folder / "index.json").write_text(json.dumps({"summary": summary, "moves": moves}, indent=1) + "\n")
    log(f"[{name}] done: {summary['moves']} moves, {summary['gz_bytes'] / 1e6:.1f} MB gz "
        f"({summary['bytes'] / 1e6:.1f} MB raw), CPU {summary['cpu_s']:.0f} s, wall {summary['wall_s']:.0f} s")
    return summary


def cmd_machine(a) -> None:
    global TRACE
    frozen_check()
    import check
    check.run_model = _capture(check.run_model)
    import groundtruth
    out_root = Path(a.out)
    out_root.mkdir(parents=True, exist_ok=True)
    rows = groundtruth.load()
    kinds = check.held_families(rows)
    inputs = check.run_b_inputs(rows)
    names = [n.strip() for n in a.runs.split(",")] if a.runs else ORDER
    logf = open(out_root / "machine.log", "a")

    def log(s: str) -> None:
        s = f"{time.strftime('%H:%M:%S')} {s}"
        print(s, file=sys.stderr, flush=True)
        logf.write(s + "\n")
        logf.flush()

    log(f"machine goldens: runs {names}, jobs {a.jobs}, trace {not a.no_trace}, PYTHONHASHSEED "
        f"{os.environ.get('PYTHONHASHSEED', 'random')}")
    sums = []
    for name in names:
        if name not in RUNS:
            raise SystemExit(f"unknown run {name}; known: {ORDER}")
        guard(name)
        frozen_check()
        only = {x.strip() for x in a.only.split(",")} if a.only else None
        sums.append(machine_run(name, out_root, a.jobs, not a.no_trace, rows, inputs, kinds, log, only))
    frozen_check()
    update_manifest(out_root, {f"machine/{s['run']}": s for s in sums})
    logf.close()


# --- the game's derived tables ---------------------------------------------------------------------------------------

def cmd_game(a) -> None:
    frozen_check()
    out_root = Path(a.out)
    out_root.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    g = model.Game.load()
    head = {"first_local": g.first_local, "local_rooms": g.local_rooms, "object_room": g.object_room,
            "object_parent": g.object_parent, "verb_offset": g.verb_offset, "room_width": g.room_width,
            "pseudo": g.pseudo, "pseudo_conflicts": g.pseudo_conflicts, "resistance": g.resistance,
            "wild_starts": g.wild_starts, "nonfacts": model.non_fact_vars(g),
            "script_writes_count": {n: len(v) for n, v in model.script_writes(g).items()},
            "const_poll_loops": g.const_poll_loops()}
    w = GzWriter(out_root / "game.jsonl.gz")
    w.write(line({"kind": "game", "game": enc(head)}))
    for key, u in g.units.items():
        wps = []
        for idx in range(len(u.ins) + 1):
            wp = g.wait_point(key, idx)
            wp2 = g.wait_point(key, idx, True)
            if (wp.kind, wp.op, wp.loop.head if wp.loop else None) != \
                    (wp2.kind, wp2.op, wp2.loop.head if wp2.loop else None):
                raise SystemExit(f"{key} [{idx}]: wait point differs with const_polls")
            wps.append([idx, wp.kind, wp.op, wp.loop.head if wp.loop else None])
        d = {"unit": enc(key), "name": model.unit_name(key), "join_points": enc(g.join_points(key)),
             "loops": enc(g.loops(key)), "loops_const_polls": enc(g.loops(key, True)),
             "time_exits": enc(g.time_exits(key)), "time_exits_camera": enc(g.time_exits(key, camera=True)),
             "time_exits_const_polls": enc(g.time_exits(key, const_polls=True)),
             "time_exits_camera_const_polls": enc(g.time_exits(key, camera=True, const_polls=True)),
             "wait_points": wps}
        w.write(line({"kind": "unit", **d}))
    info = w.close()
    frozen_check()
    update_manifest(out_root, {"game": {"file": info, "units": len(g.units), "wall_s": round(time.time() - t0, 1)}})
    print(f"game: {len(g.units)} units, {info['gz_bytes'] / 1e6:.2f} MB gz, {time.time() - t0:.0f} s", file=sys.stderr)


# --- reader goldens ------------------------------------------------------------------------------------------------

BUILDER_COUNTERS = ("counts", "ends", "drops", "nonfact_checks", "guesses", "unknown_writes", "unknown_sites",
                    "out_of_domain")
READER_COUNTERS = ("counts", "forks", "fork_keys")


def write_reading(path: Path, b, recs, info: dict) -> dict:
    sc = b.scope
    head = dict(info, rooms=list(sc.rooms), units=sorted(sc.units, key=_text_key), objects=sc.objects, verbs=sc.verbs,
                dialog_verbs=sc.dialog_verbs, walk_verb=sc.walk_verb, universe=sc.universe, scope_counts=sc.counts,
                n_records=len(recs))
    w = GzWriter(path)
    w.write(line({"kind": "reading", "reading": enc(head)}))
    for i, r in enumerate(recs):
        w.write(line({"kind": "record", "index": i, "record": enc(r)}))
    bc = {name: getattr(b, name) for name in BUILDER_COUNTERS}
    bc |= {f"rd.{name}": getattr(b.rd, name) for name in READER_COUNTERS}
    w.write(line({"kind": "builder", "builder": enc(bc)}))
    return w.close()


def _text_key(k):
    return _text(enc(k))


def read_scope(rooms, pre: dict, workers: int, goals=None):
    import domains as D
    import reader as R
    game = model.Game.load()
    doms = D.build(pre, goals=goals)
    sc = R.Scope.of(game, rooms, pre)
    b = R.Builder(R.Reader(sc, doms, game))
    recs = b.read_all(workers=workers)
    return b, recs


def cmd_reader_room33(a) -> None:
    frozen_check()
    out_root = Path(a.out)
    (out_root / "reader").mkdir(parents=True, exist_ok=True)
    start = next(json.loads(ln) for ln in (HERE / "results" / "start-part1.jsonl").read_text().splitlines()
                 if json.loads(ln)["seed"] == ROOM33_SEED)
    pre = start["facts"]
    if pre.get("room") != ROOM33[0]:
        raise SystemExit(f"seed {ROOM33_SEED}'s start room is {pre.get('room')}, not {ROOM33[0]}")
    t0 = time.time()
    b, recs = read_scope(ROOM33, pre, a.workers)
    secs = time.time() - t0
    import reader as R  # noqa: F401
    info = {"name": "room33", "start": f"results/start-part1.jsonl seed {ROOM33_SEED}", "domains_goals": None,
            "workers": a.workers, "sources": {n: sha1(HERE / n) for n in ("reader.py", "model.py", "domains.py",
                                                                           "ir.py", "task.py")}}
    fi = write_reading(out_root / "reader" / "room33.jsonl.gz", b, recs, info)
    frozen_check()
    update_manifest(out_root, {"reader/room33": {"file": fi, "records": len(recs), "wall_s": round(secs, 1)}})
    print(f"room33: {len(recs)} records in {secs:.1f} s, {fi['gz_bytes'] / 1e6:.2f} MB gz", file=sys.stderr)


def cmd_reader_kitchen(a) -> None:
    frozen_check()
    import groundtruth
    out_root = Path(a.out)
    (out_root / "reader").mkdir(parents=True, exist_ok=True)
    pre = next(r for r in groundtruth.load() if r["name"].startswith(KITCHEN_MOVE + " "))["pre"]
    t0 = time.time()
    b, recs = read_scope(KITCHEN_ROOMS, pre, a.workers)
    secs = time.time() - t0
    guard("reader-kitchen")
    info = {"name": "kitchen", "start": f"groundtruth pre of {KITCHEN_MOVE}", "domains_goals": None,
            "workers": a.workers, "sources": {n: sha1(HERE / n) for n in ("reader.py", "model.py", "domains.py",
                                                                           "ir.py", "task.py")}}
    fi = write_reading(out_root / "reader" / "kitchen.jsonl.gz", b, recs, info)
    frozen_check()
    update_manifest(out_root, {"reader/kitchen": {"file": fi, "records": len(recs), "wall_s": round(secs, 1)}})
    print(f"kitchen: {len(recs)} records in {secs:.1f} s, {fi['gz_bytes'] / 1e6:.2f} MB gz", file=sys.stderr)


def cmd_reader_pickle(a) -> None:
    import pickle
    import reader as R  # noqa: F401  (the pickle's classes)
    b, recs, secs = pickle.loads(Path(a.pickle).read_bytes())
    info = {"name": "pickle", "start": str(a.pickle), "domains_goals": None, "workers": None, "sources": {}}
    fi = write_reading(Path(a.out), b, recs, info)
    print(f"{a.pickle}: {len(recs)} records, {fi['gz_bytes'] / 1e6:.2f} MB gz", file=sys.stderr)


# --- manifest ------------------------------------------------------------------------------------------------------

def update_manifest(out_root: Path, parts: dict) -> None:
    path = out_root / "manifest.json"
    man = json.loads(path.read_text()) if path.exists() else {}
    man["format_version"] = FORMAT_VERSION
    man["python"] = sys.version
    man.setdefault("parts", {}).update(parts)
    mods = man.setdefault("modules_sha1", {})
    mods.update(imported_modules())
    man["frozen_expected"] = FROZEN
    gtp = HERE / "results" / "groundtruth-part1.jsonl"
    man["groundtruth_sha256"] = hashlib.sha256(gtp.read_bytes()).hexdigest()
    man.setdefault("pythonhashseed", {})
    for k in parts:
        man["pythonhashseed"][k] = os.environ.get("PYTHONHASHSEED", "random")
    man["check_reads"] = CHECK_READS
    man["sizes_gz_bytes"] = {k: (v.get("gz_bytes") if "gz_bytes" in v else v.get("file", {}).get("gz_bytes"))
                             for k, v in man["parts"].items()}
    man["total_gz_bytes"] = sum(x or 0 for x in man["sizes_gz_bytes"].values())
    path.write_text(json.dumps(man, indent=1, sort_keys=True) + "\n")
    (out_root / "FORMAT.txt").write_text(__doc__)


# --- compare -------------------------------------------------------------------------------------------------------

def _show(v) -> str:
    s = _text(v)
    return s if len(s) <= 200 else s[:200] + "..."


def diff(a, b, at: str = "") -> tuple | None:
    """The first difference between two parsed JSON values, with types: (path, golden, candidate) or None."""
    if type(a) is not type(b):
        return at or "$", a, b
    if isinstance(a, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a or k not in b:
                return f"{at}.{k}" if at else k, a.get(k, "<absent>"), b.get(k, "<absent>")
            d = diff(a[k], b[k], f"{at}.{k}" if at else k)
            if d:
                return d
        return None
    if isinstance(a, list):
        for i, (x, y) in enumerate(zip(a, b)):
            d = diff(x, y, f"{at}[{i}]")
            if d:
                return d
        if len(a) != len(b):
            return f"{at}.length", len(a), len(b)
        return None
    return None if a == b else (at or "$", a, b)


def _line_name(d: dict) -> str:
    k = d.get("kind", "?")
    if "index" in d:
        return f"{k}[{d['index']}]"
    if k == "unit":
        return f"unit {_text(d.get('unit'))}"
    if "seq" in d:
        return f"event[{d['seq']}]"
    return k


def compare_lines(gl: list, cl: list):
    """The first difference between two files' lines: (line name, field path, golden, candidate) or None."""
    for i, (x, y) in enumerate(zip(gl, cl)):
        if x == y:
            continue
        a, b = json.loads(x), json.loads(y)
        d = diff(a, b)
        if d:
            return _line_name(a), d[0], d[1], d[2]
    if len(gl) != len(cl):
        return "file", "number of lines", len(gl), len(cl)
    return None


def _find(folder: Path, stem: str) -> Path | None:
    for suf in (".jsonl.gz", ".jsonl"):
        p = folder / f"{stem}{suf}"
        if p.exists():
            return p
    return None


def compare_file(g: Path, c: Path, sha: str | None = None):
    if sha is not None and file_sha(c) == sha:
        return None
    return compare_lines(open_lines(g), open_lines(c))


def cmd_compare_file(a) -> None:
    d = compare_file(Path(a.golden_file), Path(a.candidate_file))
    if d is None:
        print("equal")
        return
    print(f"first difference: {d[0]}, field {d[1]}: golden {_show(d[2])}, candidate {_show(d[3])}")
    raise SystemExit(1)


def cmd_compare(a) -> None:
    gold, cand = Path(a.golden), Path(a.candidate)
    names = [n.strip() for n in a.runs.split(",")] if a.runs else ORDER
    differ, checked, missing = 0, 0, 0
    first = None
    for name in names:
        gi = gold / "machine" / name / "index.json"
        cdir = cand / "machine" / name
        if not gi.exists() or not cdir.exists():
            continue
        idx = json.loads(gi.read_text())
        run_diff = 0
        for m in idx["moves"]:
            if m["status"] != "counted":
                continue
            lab = m["label"]
            cf = _find(cdir, lab)
            if cf is None:
                missing += 1
                if first is None or a.all:
                    print(f"{name} {lab}: missing in the candidate")
                    first = first or (name, lab)
                continue
            checked += 1
            d = compare_file(_find(gold / "machine" / name, lab), cf, m["file"]["sha256"])
            if d is None and a.traces and m.get("trace"):
                ct = _find(cdir, lab + ".trace")
                if ct is not None:
                    d = compare_file(_find(gold / "machine" / name, lab + ".trace"), ct, m["trace"]["sha256"])
                    d = d and (f"trace {d[0]}",) + d[1:]
            if d is not None:
                differ += 1
                run_diff += 1
                if first is None or a.all:
                    print(f"{name} {lab}: {d[0]}, field {d[1]}: golden {_show(d[2])}, candidate {_show(d[3])}")
                    first = first or (name, lab)
        print(f"{name}: {run_diff} of {idx['summary']['moves']} moves differ")
    for stem in ("game", "reader/room33", "reader/kitchen"):
        g = _find(gold, stem)
        c = _find(cand, stem)
        if g is None or c is None:
            continue
        checked += 1
        d = compare_file(g, c)
        if d is None:
            print(f"{stem}: equal")
        else:
            differ += 1
            print(f"{stem}: {d[0]}, field {d[1]}: golden {_show(d[2])}, candidate {_show(d[3])}")
    print(f"files compared: {checked}; differing: {differ}; missing moves: {missing}")
    if differ or missing or not checked:
        raise SystemExit(1)


# --- main ----------------------------------------------------------------------------------------------------------

def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("machine")
    s.add_argument("--runs", default="", help=f"comma-separated runs (default all, in order: {','.join(ORDER)})")
    s.add_argument("--jobs", type=int, default=4)
    s.add_argument("--out", default=str(OUT))
    s.add_argument("--no-trace", action="store_true")
    s.add_argument("--only", default="", help="comma-separated move labels (a partial run, for tests)")
    s.set_defaults(fn=cmd_machine)
    s = sub.add_parser("game")
    s.add_argument("--out", default=str(OUT))
    s.set_defaults(fn=cmd_game)
    s = sub.add_parser("reader-room33")
    s.add_argument("--out", default=str(OUT))
    s.add_argument("--workers", type=int, default=4)
    s.set_defaults(fn=cmd_reader_room33)
    s = sub.add_parser("reader-kitchen")
    s.add_argument("--out", default=str(OUT))
    s.add_argument("--workers", type=int, default=4)
    s.set_defaults(fn=cmd_reader_kitchen)
    s = sub.add_parser("reader-pickle")
    s.add_argument("pickle")
    s.add_argument("--out", required=True)
    s.set_defaults(fn=cmd_reader_pickle)
    s = sub.add_parser("compare")
    s.add_argument("candidate")
    s.add_argument("--golden", default=str(OUT))
    s.add_argument("--runs", default="")
    s.add_argument("--traces", action="store_true")
    s.add_argument("--all", action="store_true")
    s.set_defaults(fn=cmd_compare)
    s = sub.add_parser("compare-file")
    s.add_argument("golden_file")
    s.add_argument("candidate_file")
    s.set_defaults(fn=cmd_compare_file)
    s = sub.add_parser("format")
    s.set_defaults(fn=lambda a: print(__doc__))
    a = ap.parse_args()
    if a.cmd in ("machine", "reader-room33", "reader-kitchen") and getattr(a, "jobs", getattr(a, "workers", 1)) > 4:
        raise SystemExit("at most 4 worker processes")
    a.fn(a)


if __name__ == "__main__":
    main()
