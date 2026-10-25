"""E003 step 5: a reading as files, which the Part I runner (replay_part1.py) plans and plays from.

usage: from packed_reading import export_reading, load_reading, reading_from_golden, check_reading
       export_reading(out_dir, b, records, goal, start_name)   # a Python reading (reader.py): the three files
       reading_from_golden(golden, packed, out_dir, start, start_name)   # a reading in golden.py's format and its
                                                               # packed file (any writer, e.g. the Rust reader)
       rd = load_reading(out_dir / "reading.json")             # the runner's view: no reader objects needed
       rd.task      # a task.Task whose actions are decoded when asked for (``PackedTask``): build_task's task
       rd.b         # what reader.task_start and reader.replan_start read (``Points``)
       rd.packed    # the packed file, for taskcore
       check_reading(out_dir / "reading.json")                 # the three files against each other and this format
       python packed_reading.py from-golden GOLDEN PACKED --start-seed N --name NAME --out DIR
       python packed_reading.py check DIR/reading.json

**Why files.** The Python reader cannot read Part I on this laptop; the Rust reader (branch reader-b) writes the
packed file directly. A reading is three files, so that either reader can write it and the runner reads both the
same way, with no reader object in memory:

* ``reading.tc``: the packed records file in taskcore's format (taskcore/README.md, version 1): build_task's task
  (one action per record; records of one label with the same precondition and effects folded into the first), its
  domains and its goal. export_records.py writes it from Python records.
* ``reading.actions.jsonl``: one JSON line per action of the packed file, in its order: what the replay harness
  reads from an action's record that the packed file does not hold.
* ``reading.json``: the reading as a whole.

**From the Rust reader** (``reading_from_golden``). The Rust reader writes the packed file (scumm/reader/src/packed.rs:
``RecordStream``, whose bytes differ from export_records.py's but whose task is the same) and its records in
golden.py's reading format (the lines it is compared with: a head, one ``$Record`` per record in ``read_all``'s order,
the Builder's counters). From those two files, ``reading_from_golden`` writes the other two, with no reading: each
packed action's line from the record its ``rec`` names (so the lines follow the packed file as it is after the
rewrite at ``finish``, whatever its order), and reading.json from the golden head (the scope), the packed file (the
goal, the actions) and the domains' bounds from domains.build and reader.FactTable on the scope and the start (no
reading: a pass only adds values to a domain, reader.Builder.extend_domains, never moves its bounds), each checked
against the packed file's values. The check strings and the event sources are then the records' own, as
``eval_check`` and ``step_input`` read them.

**reading.actions.jsonl**, line i (action i of the packed file), keys:

* ``i``: i; ``rec``: the record's index (the packed action's ``rec``); ``label``: the record's label (checked
  against the packed file's);
* ``source``: the input, as reader.Record.source: ``["sentence", verb, A, B or null]`` (a sentence or a walk),
  ``["verb", V]`` (a dialog line), ``["event", unit name, W, locals number, Esc]`` (an event; Esc true for a
  skipped cutscene);
* ``checks``: ``[[cite, condition, side, why], ...]``, reader.Record.checks: the conditions on non-facts the path
  took one side of (side true or false), checked at replay;
* ``pre_keys``: the facts of the record's own precondition (reader.Record.pre; build_task may add a set for an add's
  room, which is not one of them);
* ``engine_pre``: the facts of ``pre_keys`` that are the input's engine conditions (reader.Record.engine_pre: input
  on, the verb shown, the object reachable, the slot at its wait point, ...); the rest are what the scripts tested.

The precondition sets, the effects, the kind, the path's end and the label's text are the packed file's.

**reading.json**: ``format`` ("E003 reading 1"), ``packed`` and ``actions`` (the two file names, next to it),
``n_actions``, ``n_records`` (records read), ``folded`` (records folded by build_task's rule), ``goal`` ({fact: [values]},
the packed file's), ``start`` ({"name": what it is, "facts": the facts.py map the domains were built from}),
``scope`` ({"rooms", "walk_verb", "verbs", "dialog_verbs"}), ``bounds`` ({fact: [low, high]} for every fact of the
packed file whose domain holds "below" or "above": domains.Domain's low and high, which the packed file does not
hold and the harness needs to label an engine value), ``counts`` (the reader's counters), ``sources`` (what made it),
``control`` (false; true for the control's file, export_records.control_records).

**The task** (``PackedTask``): its facts are domains.Domain (values from the packed file, low and high from
``bounds``), its goal the packed file's, its actions task.Action decoded on demand (``task.actions[i]``), named
``label#rec`` as build_task names them. Each action's ``what`` is a reader.Record rebuilt from the packed action and
its line: ``pre`` the action's sets on ``pre_keys``; ``effects`` in the reader's form (``effects_of``: build_task's
encoding read backwards: a set, a guess and an add are one effect each, a "when" a set under a condition that the
fact is 1, a "copy" the two sets under the source's 0 and 1); ``checks``, ``source``, ``engine_pre`` from the line.
A written value is the packed file's label of it (build_task's raw value where that is an int the domain holds).
The bans' pieces are made per action when asked for (``ban_pieces``), by replay.cut, as replay.ban_pieces makes them
for every action. The packed file is read through mmap and the action lines through an index of offsets, so a
reading far larger than memory costs about 12 bytes per action plus its string table.
"""

import array
import collections
import functools
import json
import mmap
import re
import sys
from dataclasses import dataclass
from pathlib import Path

_HERE = Path(__file__).resolve().parent
for _p in (str(_HERE.parents[1] / "src"), str(_HERE)):
    if _p not in sys.path:
        sys.path.insert(0, _p)

import domains as D  # noqa: E402
import reader as R  # noqa: E402
import task as T  # noqa: E402
from export_records import (END_MAGIC, MAGIC, NONE, OP_ADD, OP_GUESS, OP_SET, VERSION, Writer,  # noqa: E402
                            record_actions)

FORMAT = "E003 reading 1"
assert sys.byteorder == "little"


def _value(text: str):
    """A value's text as the domain holds it: an int, or "above"/"below"."""
    try:
        return int(text)
    except ValueError:
        return text


# --- writing (a Python reading) ---------------------------------------------------------------------------------

def source_of(r) -> list:
    """A record's source as JSON (module docstring)."""
    s = r.source
    if not s:
        return []
    if s[0] == "event":
        ev = s[1]
        return ["event", ev.name, ev.w, ev.sig, bool(ev.abort)]
    return list(s)


def action_line(n: int, i: int, r) -> dict:
    return {"i": n, "rec": i, "label": r.label, "source": source_of(r),
            "checks": [[c[0], c[1], c[2], c[3]] for c in r.checks], "pre_keys": sorted(r.pre),
            "engine_pre": sorted(r.engine_pre)}


def export_reading(out_dir, b, records, goal: dict, start_name: str, info: dict | None = None,
                   name: str = "reading") -> dict:
    """Write a Python reading (reader.Builder ``b`` and its records) as the three files (module docstring), with
    ``goal`` ({fact: values}) as the packed file's goal. One pass over the records, export_records.py's fold.
    Returns the counts and the paths."""
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    tc, acts, head = out / f"{name}.tc", out / f"{name}.actions.jsonl", out / f"{name}.json"
    counts = {}
    w = Writer(tc, b.table.domain)
    for k in goal:
        w.fact(k)
    n = 0
    with open(acts, "w") as fa:
        for i, label, kind, end, cont, pre, effects in record_actions(b, records, counts):
            w.action(i, label, kind, end, cont, 1, pre, effects)
            fa.write(json.dumps(action_line(n, i, records[i]), default=str) + "\n")
            n += 1
    w.close(goal, {})
    bounds = {}
    for k in w.flist:
        d = b.table.domain(k)
        if d is not None and not d.exact:
            bounds[k] = [d.low, d.high]
    sc = b.scope
    doc = {"format": FORMAT, "packed": tc.name, "actions": acts.name, "n_actions": n, "n_records": len(records),
           "folded": counts.get("folded", 0), "goal": {k: sorted(v, key=str) for k, v in goal.items()},
           "start": {"name": start_name, "facts": dict(sorted(sc.start.items()))},
           "scope": {"rooms": list(sc.rooms), "walk_verb": sc.walk_verb, "verbs": list(sc.verbs),
                     "dialog_verbs": list(sc.dialog_verbs)},
           "bounds": bounds, "counts": dict(sorted((str(k), v) for k, v in b.counts.items())),
           "sources": dict((info or {}).get("sources", {})), "control": False}
    doc.update({k: v for k, v in (info or {}).items() if k not in doc})
    head.write_text(json.dumps(doc, indent=1, default=str) + "\n")
    return {"actions": n, "facts": len(w.flist), "strings": len(w.slist), "bytes": w.words * 4,
            "folded": counts.get("folded", 0), "json": str(head), "packed": str(tc), "lines": str(acts)}


# --- reading the packed file ------------------------------------------------------------------------------------

class Packed:
    """A packed file (taskcore/README.md, version 1): its tables read at open, its actions by index on demand."""

    def __init__(self, path):
        self.path = Path(path)
        self._f = open(self.path, "rb")
        self._mm = mmap.mmap(self._f.fileno(), 0, access=mmap.ACCESS_READ)
        w = self.w = memoryview(self._mm).cast("I")
        if len(w) < 10 or w[0] != MAGIC or w[1] != VERSION:
            raise ValueError(f"{self.path}: not a taskcore file of version {VERSION}")
        body, n, o_str, o_facts, o_adds, o_goal, o_starts, end = w[len(w) - 8:]
        if end != END_MAGIC:
            raise ValueError(f"{self.path}: bad footer")
        self.n_actions = n
        ns = w[o_str]
        lens = w[o_str + 1:o_str + 1 + ns]
        pos = (o_str + 1 + ns) * 4
        raw = self._mm
        self.strings = []
        for ln in lens:
            self.strings.append(raw[pos:pos + ln].decode())
            pos += ln
        s = self.strings
        p = o_facts
        nf = w[p]
        p += 1
        self.fact_keys, self.fact_values = [], []
        for _ in range(nf):
            key, size = w[p], w[p + 1]
            self.fact_keys.append(s[key])
            self.fact_values.append(tuple(_value(s[v]) for v in w[p + 2:p + 2 + size]))
            p += 2 + size
        p = o_goal
        self.goal = {}
        for _ in range(w[p]):
            f, m = w[p + 1], w[p + 2]
            self.goal[self.fact_keys[f]] = {_value(s[v]) for v in w[p + 3:p + 3 + m]}
            p += 2 + m
        self.offsets = array.array("Q")  # word offset of each action's length word
        p = body
        for _ in range(n):
            self.offsets.append(p)
            p += 1 + w[p]
        if p != o_str:
            raise ValueError(f"{self.path}: the actions end at word {p}, the strings start at {o_str}")

    def close(self) -> None:
        try:
            self.w.release()
            self._mm.close()
        except BufferError:
            pass  # a view of the map is still held: the map closes with it
        self._f.close()

    def _sets(self, p: int, n: int) -> tuple:
        """``n`` sets [fact, m, value x m] from word ``p``: ([(key, frozenset of values)], the word after)."""
        w, s, out = self.w, self.strings, []
        for _ in range(n):
            f, m = w[p], w[p + 1]
            out.append((self.fact_keys[f], frozenset(_value(s[v]) for v in w[p + 2:p + 2 + m])))
            p += 2 + m
        return out, p

    def raw_action(self, i: int) -> dict:
        """Action ``i`` as the file holds it: rec, label, kind, end, cont, cost, pre [(key, values)], effects
        [(op, key, payload, when)] (payload: a value, a frozenset of values or the add's step)."""
        w, s = self.w, self.strings
        p = self.offsets[i] + 1
        rec, label, kind, end, cont, cost, npre = w[p:p + 7]
        pre, p = self._sets(p + 7, npre)
        effects = []
        ne = w[p]
        p += 1
        for _ in range(ne):
            op, f = w[p], w[p + 1]
            key = self.fact_keys[f]
            if op == OP_SET:
                payload = _value(s[w[p + 2]])
                p += 3
            elif op == OP_GUESS:
                m = w[p + 2]
                payload = frozenset(_value(s[v]) for v in w[p + 3:p + 3 + m])
                p += 3 + m
            elif op == OP_ADD:
                d = w[p + 2]
                payload = d - (1 << 32) if d >= 1 << 31 else d
                p += 3
            else:
                raise ValueError(f"{self.path}: action {i}: effect op {op}")
            nw = w[p]
            when, p = self._sets(p + 1, nw)
            effects.append((op, key, payload, tuple(when)))
        return {"rec": rec, "label": s[label], "kind": s[kind], "end": s[end], "cont": s[cont], "cost": cost,
                "pre": pre, "effects": effects}


def effects_of(effects: list) -> dict:
    """The reader's effects (reader.Record.effects) from the packed action's (module docstring)."""
    by = collections.defaultdict(list)
    for op, key, payload, when in effects:
        by[key].append((op, payload, when))
    out = {}
    for key, es in by.items():
        if len(es) == 2 and all(op == OP_SET and len(w) == 1 for op, _, w in es):
            (_, v0, w0), (_, v1, w1) = sorted(es, key=lambda e: sorted(map(str, e[2][0][1])))
            if w0[0][0] == w1[0][0] and w0[0][1] == {0} and w1[0][1] == {1} and (v0, v1) == (0, 1):
                out[key] = ("copy", w0[0][0])
                continue
        if len(es) != 1:
            raise ValueError(f"{key}: {len(es)} effects that are not a copy")
        op, payload, when = es[0]
        if op == OP_GUESS:
            out[key] = ("guess", payload)
        elif op == OP_ADD:
            out[key] = ("add", payload)
        elif not when:
            out[key] = ("set", payload)
        elif len(when) == 1 and when[0][0] != key and when[0][1] == {1}:
            out[key] = ("when", when[0][0], payload)
        else:
            raise ValueError(f"{key}: a set under {when}, which build_task does not write")
    return out


# --- the action lines ----------------------------------------------------------------------------------------------

class Lines:
    """reading.actions.jsonl with an index of line offsets: line ``i`` on demand."""

    def __init__(self, path):
        self.path = Path(path)
        self.offsets = array.array("Q")
        pos = 0
        with open(self.path, "rb") as f:
            for line in f:
                self.offsets.append(pos)
                pos += len(line)
        self._f = open(self.path, "rb")

    def __len__(self) -> int:
        return len(self.offsets)

    def __getitem__(self, i: int) -> dict:
        self._f.seek(self.offsets[i])
        return json.loads(self._f.readline())

    def __iter__(self):
        with open(self.path) as f:
            for line in f:
                yield json.loads(line)

    def close(self) -> None:
        self._f.close()


# --- the task ----------------------------------------------------------------------------------------------------

class Actions:
    """``PackedTask.actions``: a sequence that decodes action i when asked for (the last ``CACHE`` kept)."""

    def __init__(self, task: "PackedTask"):
        self._task = task

    def __len__(self) -> int:
        return self._task.packed.n_actions

    def __getitem__(self, i):
        if isinstance(i, slice):
            return [self._task.action(j) for j in range(*i.indices(len(self)))]
        if i < 0:
            i += len(self)
        if not 0 <= i < len(self):
            raise IndexError(i)
        return self._task.action(i)

    def __iter__(self):
        for i in range(len(self)):
            yield self._task.action(i)


class Pieces:
    """Per action of the task, its pieces under ``bans`` (replay.ban_pieces, one action at a time)."""

    def __init__(self, task: "PackedTask", bans: list):
        import replay
        self._cut = replay.cut
        self._task = task
        self._by = collections.defaultdict(list)
        for ban in bans:
            self._by[ban.label].append(ban)

    def __len__(self) -> int:
        return len(self._task.actions)

    @functools.lru_cache(maxsize=4096)
    def __getitem__(self, m: int) -> list:
        a = self._task.actions[m]
        pieces = [a]
        for ban in self._by.get(a.what.label, ()):
            if all(k in self._task.facts for k, _ in ban.values):
                pieces = [p for q in pieces for p in self._cut(q, ban, self._task.facts)]
        return pieces


class PackedTask(T.Task):
    """build_task's task read from the packed file and the action lines (module docstring)."""

    CACHE = 4096

    def __init__(self, packed: Packed, lines: Lines, bounds: dict):
        if len(lines) != packed.n_actions:
            raise ValueError(f"{lines.path}: {len(lines)} lines for {packed.n_actions} actions in {packed.path}")
        self.packed, self.lines = packed, lines
        facts = {}
        for k, vals in zip(packed.fact_keys, packed.fact_values):
            lo, hi = bounds.get(k, (None, None))
            facts[k] = D.Domain(tuple(vals), lo if D.BELOW in vals else None, hi if D.ABOVE in vals else None)
        super().__init__(facts, Actions(self), {k: set(v) for k, v in packed.goal.items()})
        self.folded = 0
        self.action = functools.lru_cache(maxsize=self.CACHE)(self._action)

    def _action(self, i: int) -> T.Action:
        raw, line = self.packed.raw_action(i), self.lines[i]
        if line["i"] != i or line["rec"] != raw["rec"] or line["label"] != raw["label"]:
            raise ValueError(f"action {i}: the line {line['i']}, rec {line['rec']}, {line['label']!r} is not the "
                             f"packed file's rec {raw['rec']}, {raw['label']!r}")
        pre = {k: set(v) for k, v in raw["pre"]}
        effects = []
        for op, key, payload, when in raw["effects"]:
            w = tuple((k, frozenset(v)) for k, v in when)
            if op == OP_SET:
                effects.append(T.Effect(key, value=payload, when=w))
            elif op == OP_GUESS:
                effects.append(T.Effect(key, values=frozenset(payload), when=w))
            else:
                effects.append(T.Effect(key, add=payload, when=w))
        rec = R.Record(raw["kind"], raw["label"], {k: frozenset(pre[k]) for k in line["pre_keys"] if k in pre},
                       effects_of(raw["effects"]), [tuple(c) for c in line["checks"]], raw["end"],
                       source=tuple(line["source"]), engine_pre=frozenset(line["engine_pre"]))
        name = f"{raw['label']}#{raw['rec']}" if raw["rec"] != NONE else raw["label"]
        return T.Action(name, pre=pre, effects=tuple(effects), what=rec, cost=raw["cost"])

    def ban_pieces(self, bans: list) -> Pieces:
        return Pieces(self, list(bans))

    def close(self) -> None:
        self.packed.close()
        self.lines.close()


class Points:
    """What reader.task_start and reader.replan_start read from a Builder: the wait points and Esc points of each
    unit (``table.points``), here from the domains of the task's ``pos:`` and ``escpos:`` facts, which build_task
    gives the reader's own domain ({NOWHERE} and those points); and a counter."""

    def __init__(self, facts: dict):
        pts = {"at": {}, "esc": {}}
        for k, d in facts.items():
            kind, _, name = k.partition(":")
            if kind in R.POS_KINDS:
                pts[R.POS_KINDS[kind]][name] = {v for v in d.values if isinstance(v, int) and v != R.NOWHERE}
        self.table = type("Table", (), {"points": pts})()
        self.counts = collections.Counter()


@dataclass
class Reading:
    path: Path
    doc: dict
    task: PackedTask
    b: Points
    packed: Path
    lines: Path

    @property
    def dialog_verbs(self) -> list:
        return list(self.doc["scope"]["dialog_verbs"])

    def close(self) -> None:
        self.task.close()


def load_reading(path) -> Reading:
    """reading.json and its two files (module docstring)."""
    path = Path(path)
    doc = json.loads(path.read_text())
    if doc.get("format") != FORMAT:
        raise ValueError(f"{path}: format {doc.get('format')!r}, not {FORMAT!r}")
    packed, lines = path.parent / doc["packed"], path.parent / doc["actions"]
    task = PackedTask(Packed(packed), Lines(lines), doc.get("bounds", {}))
    task.folded = doc.get("folded", 0)
    goal = {k: set(v) for k, v in doc["goal"].items()}
    if goal != task.goal:
        raise ValueError(f"{path}: goal {doc['goal']} is not the packed file's {task.goal}")
    return Reading(path, doc, task, Points(task.facts), packed, lines)


# --- from a reading in golden.py's format (the Rust reader) ------------------------------------------------------

def _dec(x):
    """golden.py's canonical encoding read back: $tuple a tuple, $set a frozenset, $map a dict, a $Class a dict of its
    fields; anything else as JSON gives it."""
    if isinstance(x, list):
        return [_dec(v) for v in x]
    if not isinstance(x, dict):
        return x
    if len(x) == 1:
        (k, v), = x.items()
        if k == "$tuple":
            return tuple(_dec(e) for e in v)
        if k == "$set":
            return frozenset(_dec(e) for e in v)
        if k == "$map":
            return {_dec(a): _dec(b) for a, b in v}
        if k.startswith("$"):
            return {f: _dec(e) for f, e in v.items()}
    return {k: _dec(v) for k, v in x.items()}


def line_of_golden(n: int, rec: int, r: dict) -> dict:
    """``action_line`` from a decoded golden ``$Record`` (its fields as a dict)."""
    src = r["source"]
    if not src:
        source = []
    elif src[0] == "event":
        ev = src[1]
        source = ["event", ev["name"], ev["w"], ev["sig"], bool(ev["abort"])]
    else:
        source = list(src)
    return {"i": n, "rec": rec, "label": r["label"], "source": source,
            "checks": [[c[0], c[1], c[2], c[3]] for c in r["checks"]], "pre_keys": sorted(r["pre"]),
            "engine_pre": sorted(r["engine_pre"])}


def _golden_lines(path):
    import gzip
    opener = gzip.open if str(path).endswith(".gz") else open
    with opener(path, "rt") as f:
        for ln in f:
            yield json.loads(ln)


def python_domains(rooms, start: dict):
    """``domain(key)`` of the reader's FactTable on ``rooms`` and ``start``, before any reading (module docstring)."""
    import model
    game = model.Game.load()
    return R.FactTable(R.Scope.of(game, tuple(rooms), start), D.build(start), game).domain


def reading_from_golden(golden, packed, out_dir, start: dict, start_name: str, name: str = "reading",
                        domain=None, info: dict | None = None) -> dict:
    """A reading's three files from its records in golden.py's format and its packed file (module docstring).
    ``start``: the facts.py map it was read from; ``domain``: key -> domains.Domain for the bounds (default
    ``python_domains`` on the golden head's rooms and ``start``). The packed file is copied next to the others when it
    is not there. Returns counts and paths."""
    import shutil
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    tc, acts, head_path = out / f"{name}.tc", out / f"{name}.actions.jsonl", out / f"{name}.json"
    if Path(packed).resolve() != tc.resolve():
        shutil.copyfile(packed, tc)
    pk = Packed(tc)
    try:
        want = [(pk.w[o + 1], pk.strings[pk.w[o + 2]]) for o in pk.offsets]  # (rec, label) per action
        keys, values, goal = list(pk.fact_keys), list(pk.fact_values), dict(pk.goal)
    finally:
        pk.close()
    lines_in = _golden_lines(golden)
    head = next(lines_in)
    if head.get("kind") != "reading":
        raise ValueError(f"{golden}: the first line is not a reading's head")
    hd = _dec(head["reading"])
    order = [r for r, _ in want]
    builder = None
    n_records = 0
    if order == sorted(order):
        # one pass: the records come in index order, the actions name them in the same order
        it = iter(enumerate(want))
        nxt = next(it, None)
        with open(acts, "w") as fa:
            for ln in lines_in:
                if ln["kind"] == "builder":
                    builder = _dec(ln["builder"])
                    continue
                n_records += 1
                while nxt is not None and nxt[1][0] == ln["index"]:
                    r = _dec(ln["record"])  # a $Record's fields
                    line = line_of_golden(nxt[0], ln["index"], r)
                    if line["label"] != nxt[1][1]:
                        raise ValueError(f"action {nxt[0]}: record {ln['index']} is {line['label']!r}, the packed "
                                         f"file's {nxt[1][1]!r}")
                    fa.write(json.dumps(line, default=str) + "\n")
                    nxt = next(it, None)
        if nxt is not None:
            raise ValueError(f"{golden}: no record {nxt[1][0]} for action {nxt[0]} of {packed}")
    else:
        need = {}
        for i, (r, _) in enumerate(want):
            need.setdefault(r, []).append(i)
        got = {}
        for ln in lines_in:
            if ln["kind"] == "builder":
                builder = _dec(ln["builder"])
                continue
            n_records += 1
            if ln["index"] in need:
                got[ln["index"]] = _dec(ln["record"])
        with open(acts, "w") as fa:
            for i, (r, label) in enumerate(want):
                if r not in got:
                    raise ValueError(f"{golden}: no record {r} for action {i} of {packed}")
                line = line_of_golden(i, r, got[r])
                if line["label"] != label:
                    raise ValueError(f"action {i}: record {r} is {line['label']!r}, the packed file's {label!r}")
                fa.write(json.dumps(line, default=str) + "\n")
    if hd.get("n_records") is not None and hd["n_records"] != n_records:
        raise ValueError(f"{golden}: {n_records} records, its head says {hd['n_records']}")
    dom = domain or python_domains(hd["rooms"], start)
    bounds, problems = {}, []
    for k, vals in zip(keys, values):
        has_b, has_a = D.BELOW in vals, D.ABOVE in vals
        if not (has_b or has_a):
            continue
        d = dom(k)
        if d is None or (D.BELOW in d.values) != has_b or (D.ABOVE in d.values) != has_a:
            problems.append(f"{k}: the packed file's values {vals}, the domain's {None if d is None else d.values}")
            continue
        ints = [v for v in vals if isinstance(v, int)]
        if (has_b and any(v < d.low for v in ints)) or (has_a and any(v > d.high for v in ints)):
            problems.append(f"{k}: the packed file's values {vals} outside [{d.low}, {d.high}]")
            continue
        bounds[k] = [d.low, d.high]
    if problems:
        raise ValueError(f"{len(problems)} facts whose bounds the domains do not give: {problems[:5]}")
    counts = (builder or {}).get("counts", {})
    doc = {"format": FORMAT, "packed": tc.name, "actions": acts.name, "n_actions": len(want), "n_records": n_records,
           "folded": n_records - len(want), "goal": {k: sorted(v, key=str) for k, v in goal.items()},
           "start": {"name": start_name, "facts": dict(sorted(start.items()))},
           "scope": {"rooms": list(hd["rooms"]), "walk_verb": hd["walk_verb"], "verbs": list(hd["verbs"]),
                     "dialog_verbs": list(hd["dialog_verbs"])},
           "bounds": bounds, "counts": dict(sorted((str(k), v) for k, v in counts.items())),
           "sources": dict(hd.get("sources") or {}), "control": False, "name": hd.get("name"),
           "from_golden": {"golden": str(golden), "packed": str(packed)}}
    doc.update({k: v for k, v in (info or {}).items() if k not in doc})
    head_path.write_text(json.dumps(doc, indent=1, default=str) + "\n")
    return {"actions": len(want), "records": n_records, "facts": len(keys), "bounds": len(bounds),
            "json": str(head_path), "packed": str(tc), "lines": str(acts)}


# --- the check ------------------------------------------------------------------------------------------------------

SOURCE_SHAPES = {"sentence": "sentence", "walk_to": "sentence", "dialog_line": "verb", "event": "event",
                 "skip_cutscene": "event"}


def _source_ok(kind: str, src: list) -> bool:
    shape = SOURCE_SHAPES.get(kind)
    if shape == "sentence":
        return len(src) == 4 and src[0] == "sentence" and all(isinstance(x, int) for x in src[1:3]) and \
            (src[3] is None or isinstance(src[3], int))
    if shape == "verb":
        return len(src) == 2 and src[0] == "verb" and isinstance(src[1], int)
    if shape == "event":
        return len(src) == 5 and src[0] == "event" and isinstance(src[1], str) and isinstance(src[2], int) and \
            isinstance(src[3], int) and isinstance(src[4], bool) and (src[4] if kind == "skip_cutscene" else True)
    return False


def check_reading(path, limit: int = 10) -> dict:
    """The three files against each other and this module's format (module docstring): every action's line (its
    index, record and label; its source's shape for its kind; its checks' shape, and how many read operands
    replay.eval_check can see; ``engine_pre`` within ``pre_keys`` within the action's precondition), the bounds of
    every fact that holds "below" or "above", the goal, the start and the scope. Returns the counts and the first
    ``limit`` problems of each kind; ``ok`` when there are none."""
    import replay
    rd = load_reading(path)
    probs = collections.defaultdict(list)
    counts = collections.Counter()

    def bad(kind, what):
        counts[f"problem: {kind}"] += 1
        if len(probs[kind]) < limit:
            probs[kind].append(what)
    try:
        for k, d in rd.task.facts.items():
            if not d.exact and (k not in rd.doc["bounds"] or
                                (D.ABOVE in d.values and d.high is None) or (D.BELOW in d.values and d.low is None)):
                bad("bounds", k)
            ints = [v for v in d.values if isinstance(v, int)]
            if d.high is not None and any(v > d.high for v in ints) or d.low is not None and any(v < d.low for v in ints):
                bad("bounds", f"{k}: {d.values} outside [{d.low}, {d.high}]")
        for key in ("rooms", "walk_verb", "verbs", "dialog_verbs"):
            if key not in rd.doc.get("scope", {}):
                bad("scope", key)
        if not isinstance(rd.doc.get("start", {}).get("facts"), dict):
            bad("start", "no start facts")
        for i in range(len(rd.task.actions)):
            try:
                a = rd.task.actions[i]
            except ValueError as e:
                bad("line", str(e))
                continue
            r = a.what
            counts[f"kind {r.kind}"] += 1
            if not _source_ok(r.kind, list(r.source)):
                bad("source", f"{a.name}: {r.kind} {list(r.source)}")
            if not set(r.engine_pre) <= set(r.pre) | set(a.pre):
                bad("engine_pre", f"{a.name}: {sorted(set(r.engine_pre) - set(a.pre))}")
            if not set(rd.task.lines[i]["pre_keys"]) <= set(a.pre):
                bad("pre_keys", f"{a.name}: {sorted(set(rd.task.lines[i]['pre_keys']) - set(a.pre))}")
            for c in r.checks:
                counts["checks"] += 1
                if len(c) != 4 or not isinstance(c[0], str) or not isinstance(c[1], str) or \
                        c[2] not in (True, False, None) or not isinstance(c[3], str):
                    bad("check", f"{a.name}: {c}")
                    continue
                m = replay._CMP.match(c[1])
                parts = [m[1], m[3]] if m else [c[1]]
                if all(re.fullmatch(r"-?\d+|Var\[\d+\]|Local\[\d+\]|VAR_EGO|VAR_RESULT", t.strip()) for t in parts):
                    counts["checks of a form eval_check reads"] += 1
    finally:
        rd.close()
    return {"ok": not probs, "actions": len(rd.task.actions), "facts": len(rd.task.facts),
            "counts": dict(sorted(counts.items())), "problems": dict(probs)}


def main() -> None:
    import argparse
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    g = sub.add_parser("from-golden")
    g.add_argument("golden", type=Path)
    g.add_argument("packed", type=Path)
    g.add_argument("--start-seed", type=int, help="the start: results/start-part1.jsonl's row for this seed")
    g.add_argument("--start-file", type=Path, help="the start: a JSON facts.py map")
    g.add_argument("--name", default="reading")
    g.add_argument("--out", type=Path, required=True)
    c = sub.add_parser("check")
    c.add_argument("reading", type=Path)
    a = ap.parse_args()
    if a.cmd == "from-golden":
        here = Path(__file__).resolve().parent
        if a.start_seed is not None:
            row = next(json.loads(x) for x in (here / "results" / "start-part1.jsonl").read_text().splitlines()
                       if json.loads(x)["seed"] == a.start_seed)
            start, start_name = row["facts"], f"results/start-part1.jsonl seed {a.start_seed}"
        else:
            start, start_name = json.loads(a.start_file.read_text()), str(a.start_file)
        print(json.dumps(reading_from_golden(a.golden, a.packed, a.out, start, start_name, a.name), indent=1))
    else:
        out = check_reading(a.reading)
        print(json.dumps(out, indent=1, default=str))
        sys.exit(0 if out["ok"] else 1)


if __name__ == "__main__":
    main()
