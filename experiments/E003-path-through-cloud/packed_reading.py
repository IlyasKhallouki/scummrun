"""E003 step 5: a reading as files, which the Part I runner (replay_part1.py) plans and plays from.

usage: from packed_reading import export_reading, load_reading
       export_reading(out_dir, b, records, goal, start_name)   # a Python reading (reader.py): the three files
       rd = load_reading(out_dir / "reading.json")             # the runner's view: no reader objects needed
       rd.task      # a task.Task whose actions are decoded when asked for (``PackedTask``): build_task's task
       rd.b         # what reader.task_start and reader.replan_start read (``Points``)
       rd.packed    # the packed file, for taskcore

**Why files.** The Python reader cannot read Part I on this laptop; the Rust reader (branch reader-b) writes the
packed file directly. A reading is three files, so that either reader can write it and the runner reads both the
same way, with no reader object in memory:

* ``reading.tc``: the packed records file in taskcore's format (taskcore/README.md, version 1): build_task's task
  (one action per record; records of one label with the same precondition and effects folded into the first), its
  domains and its goal. export_records.py writes it from Python records.
* ``reading.actions.jsonl``: one JSON line per action of the packed file, in its order: what the replay harness
  reads from an action's record that the packed file does not hold.
* ``reading.json``: the reading as a whole.

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
import sys
from dataclasses import dataclass
from pathlib import Path

import domains as D
import reader as R
import task as T
from export_records import END_MAGIC, MAGIC, NONE, OP_ADD, OP_GUESS, OP_SET, VERSION, Writer, record_actions

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
