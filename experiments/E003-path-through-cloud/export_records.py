"""E003 step 4, the memory work, stage 1: write the planner's task as a packed file for taskcore (the Rust crate).

usage: from export_records import export_records, export_task
       export_records(path, b, records, goal, starts={"s17": start_map})   # reader Records, build_task's rules
       export_task(path, task, starts={"s": start_map}, meta=None)         # a task.Task already built
       write_start(path, facts, start_map); write_bans(path, bans)          # taskcore's --start-file and --bans
       READER_PICKLE=<reading.pkl> python export_records.py kitchen <out dir>   # the kitchen files

**What it writes.** The task that ``reader.build_task`` makes from the records (one action per record; records of
one label with the same precondition and effects folded into the first), with everything ``task.write_sas`` needs:
each fact's domain, preconditions, effects (set, add, guessed writes with their values, conditions), costs, names,
the goal and one or more named start states. The format is in ``taskcore/README.md``.

**Streaming.** ``export_records`` converts one record at a time and writes it out; it keeps no Task, no action list
and no copy of the records. What it keeps: the string table (fact keys, values, labels), one fact table entry per
fact, and for the fold of identical records a 16-byte hash per distinct record (blake2b of build_task's own key)
instead of the key itself. The tables go after the actions, with a footer that points at them, so one pass suffices.

**Bytes.** The same reading gives the same file in every run: every set is written in the order of its values'
text, so Python's hash seed (the iteration order of a set of strings) does not reach the file.

**Values.** A value is the text ``str(v)`` of a domain value (ints, ``above``, ``below``). Precondition and condition
sets are written as given (taskcore cuts them to the domain, as ``task._expand`` does). A written value is the
domain's label (``Domain.label``), as ``_expand`` reads it; one the domain lacks is written as given and taskcore
raises, as ``write_sas`` does. An ``add`` is written as its step; its table (``Domain.add`` of each domain value) goes
in the tables, so taskcore needs no domain arithmetic. Each start is the domain label of each fact's start value
(``SasTask.state``), or "none" where the domain lacks it.
"""

import array
import hashlib
import os
import pickle
import sys
import time
from pathlib import Path

MAGIC, END_MAGIC, VERSION = 0x524F4354, 0x444E4554, 1  # b"TCOR", b"TEND" read as little-endian u32
NONE = 0xFFFFFFFF
OP_SET, OP_GUESS, OP_ADD = 0, 1, 2
assert array.array("I").itemsize == 4 and sys.byteorder == "little"


def _vtext(v) -> str:
    if isinstance(v, bool):
        v = int(v)
    return str(v)


def _sorted(vals) -> list:
    """A set's values in one order every run: by their text. A Python set of strings (``below``, ``above``) iterates
    in an order that depends on the hash seed; the string table gives ids on first sight, so without this the file's
    bytes would change from run to run (taskcore sorts by domain index on reading, so its output would not)."""
    return sorted(vals, key=_vtext)


class Writer:
    """The packed file, written as it goes: actions first, the tables and the footer at ``close``."""

    def __init__(self, path, domain):
        self.f = open(path, "wb")
        self.domain = domain  # fact key -> domains.Domain (the final one; asked again at close)
        self.strings, self.slist = {}, []
        self.facts, self.flist = {}, []
        self.adds = {}  # (fact id, d) -> table id
        self.n_actions = 0
        self.words = 0
        self._w(array.array("I", [MAGIC, VERSION]))
        self.body_start = self.words

    def _w(self, arr):
        self.f.write(arr.tobytes())
        self.words += len(arr)

    def s(self, text: str) -> int:
        i = self.strings.get(text)
        if i is None:
            i = self.strings[text] = len(self.slist)
            self.slist.append(text)
        return i

    def fact(self, key: str) -> int:
        i = self.facts.get(key)
        if i is None:
            i = self.facts[key] = len(self.flist)
            self.flist.append(key)
        return i

    def _set(self, out, key, vals):
        out.append(self.fact(key))
        vs = [self.s(_vtext(v)) for v in _sorted(vals)]
        out.append(len(vs))
        out.extend(vs)

    def action(self, rec: int, label: str, kind: str, end: str, cont: str, cost: int, pre, effects):
        """``pre``: [(key, values)]; ``effects``: [(op, key, payload, when)], payload a value (set), values (guess)
        or the step (add), ``when`` [(key, values)]."""
        out = array.array("I", [rec, self.s(label), self.s(kind), self.s(end), self.s(cont), cost, len(pre)])
        for k, vals in pre:
            self._set(out, k, vals)
        out.append(len(effects))
        for op, k, payload, when in effects:
            dom = self.domain(k)
            out.append(op)
            out.append(self.fact(k))
            if op == OP_SET:
                lab = dom.label(payload) if isinstance(payload, int) else payload
                out.append(self.s(_vtext(payload if lab is None else lab)))
            elif op == OP_GUESS:
                payload = _sorted(payload)
                labs = [(dom.label(v) if isinstance(v, int) else v) for v in payload]
                vs = [self.s(_vtext(v if lab is None else lab)) for v, lab in zip(payload, labs)]
                vs = list(dict.fromkeys(vs))
                out.append(len(vs))
                out.extend(vs)
            else:
                out.append(payload & 0xFFFFFFFF)
                self.adds.setdefault((self.fact(k), payload), len(self.adds))
            out.append(len(when))
            for wk, wv in when:
                self._set(out, wk, wv)
        self._w(array.array("I", [len(out)]))
        self._w(out)
        self.n_actions += 1

    def close(self, goal: dict, starts: dict):
        """``goal``: {key: values}; ``starts``: {name: facts.py map, absent is 0}."""
        for k in goal:
            self.fact(k)
        doms = [self.domain(k) for k in self.flist]

        # facts (before strings: they intern the domain values)
        out = array.array("I", [len(self.flist)])
        for k, d in zip(self.flist, doms):
            out.append(self.s(k))
            out.append(len(d.values))
            out.extend(self.s(_vtext(v)) for v in d.values)
        facts_arr = out
        out = array.array("I", [len(self.adds)])
        for (fid, d), _ in sorted(self.adds.items(), key=lambda kv: kv[1]):
            dom = doms[fid]
            out.extend([fid, d & 0xFFFFFFFF, len(dom.values)])
            for u in dom.values:
                nxt = _sorted(dom.add(u, d))
                out.append(len(nxt))
                out.extend(self.s(_vtext(v)) for v in nxt)
        adds_arr = out
        out = array.array("I", [len(goal)])
        for k, vals in goal.items():
            dom = doms[self.facts[k]]
            labs = [dom.label(v) if isinstance(v, int) else v for v in _sorted(vals)]
            labs = [x for x in dict.fromkeys(labs) if x is not None]
            out.extend([self.facts[k], len(labs)])
            out.extend(self.s(_vtext(v)) for v in labs)
        goal_arr = out
        out = array.array("I", [len(starts)])
        for name, st in starts.items():
            out.append(self.s(name))
            for k, d in zip(self.flist, doms):
                v = st.get(k, 0)
                lab = d.label(v) if isinstance(v, int) else v
                out.append(NONE if lab is None or lab not in d.values else self.s(_vtext(lab)))
        starts_arr = out
        blob = b"".join(t.encode() for t in self.slist)
        out = array.array("I", [len(self.slist)])
        out.extend(len(t.encode()) for t in self.slist)
        strings_arr = out
        offs = {}
        offs["strings"] = self.words
        self._w(strings_arr)
        pad = (-len(blob)) % 4
        self.f.write(blob + b"\0" * pad)
        self.words += (len(blob) + pad) // 4
        for name, arr in (("facts", facts_arr), ("adds", adds_arr), ("goal", goal_arr), ("starts", starts_arr)):
            offs[name] = self.words
            self._w(arr)
        self._w(array.array("I", [self.body_start, self.n_actions, offs["strings"], offs["facts"], offs["adds"],
                                  offs["goal"], offs["starts"], END_MAGIC]))
        self.f.close()



def _cont(r) -> str:
    """A record's continuation: the events its path leaves, by unit, wait point and Esc, sorted (the fold's key)."""
    evs = sorted((e.name, e.w, bool(e.abort)) for e in r.next_events)
    return ";".join(f"{n}@{w}{'!' if ab else ''}" for n, w, ab in evs)


def record_actions(b, records, counts: dict):
    """build_task's actions, one at a time: (rec index, label, kind, end, cont, pre, effects) (module docstring). The
    same rules as reader.build_task (read it: reader.py), with the fold key hashed."""
    seen = set()
    for i, r in enumerate(records):
        same = (r.label, tuple(sorted((k, tuple(sorted(map(str, v)))) for k, v in r.pre.items())),
                tuple(sorted((k, str(e)) for k, e in r.effects.items())))
        h = hashlib.blake2b(repr(same).encode(), digest_size=16).digest()
        if h in seen:
            counts["folded"] = counts.get("folded", 0) + 1
            continue
        seen.add(h)
        effects, pre = [], {k: set(v) for k, v in r.pre.items()}
        for k, e in r.effects.items():
            if e[0] == "set":
                effects.append((OP_SET, k, e[1], ()))
            elif e[0] == "guess":
                effects.append((OP_GUESS, k, e[1], ()))
            elif e[0] == "when":
                effects.append((OP_SET, k, e[2], ((e[1], (1,)),)))
            elif e[0] == "copy":
                effects += [(OP_SET, k, u, ((e[1], (u,)),)) for u in (0, 1)]
            else:
                dom = b.table.domain(k)
                ok = {v for v in pre.get(k, dom.values) if dom.add(v, e[1])}
                if ok != set(pre.get(k, dom.values)):
                    pre[k] = ok
                effects.append((OP_ADD, k, e[1], ((k, tuple(ok)),)))
        yield i, r.label, r.kind, r.end, _cont(r), list(pre.items()), effects


def export_records(path, b, records, goal: dict, starts: dict) -> dict:
    """Write build_task's task from ``records`` (module docstring). Returns counts."""
    counts = {}
    w = Writer(path, b.table.domain)
    for k in goal:
        w.fact(k)
    for i, label, kind, end, cont, pre, effects in record_actions(b, records, counts):
        w.action(i, label, kind, end, cont, 1, pre, effects)
    w.close(goal, starts)
    counts.update(actions=w.n_actions, facts=len(w.flist), strings=len(w.slist), bytes=w.words * 4)
    return counts


def export_task(path, task, starts: dict, meta=None, naming=None) -> dict:
    """Write a task.Task. ``meta(action)`` gives (kind, end, cont) for the fork fold (default: none, so nothing folds
    unless the caller says what each action's input and continuation are); ``naming(action)`` gives (rec, label), the
    name being ``label#rec`` (rec NONE: the label alone; default: the action's name is its label). taskcore's bans
    match the label: ``record_naming`` gives a record's label and index, as export_records writes them."""
    w = Writer(path, lambda k: task.facts[k])
    for k in task.goal:
        w.fact(k)
    for a in task.actions:
        effects = []
        for e in a.effects:
            when = [(k, tuple(s)) for k, s in e.when]
            if e.values is not None:
                effects.append((OP_GUESS, e.key, tuple(e.values), when))
            elif e.add is not None:
                effects.append((OP_ADD, e.key, e.add, when))
            else:
                effects.append((OP_SET, e.key, e.value, when))
        kind, end, cont = meta(a) if meta else ("", "", "")
        rec, label = naming(a) if naming else (NONE, a.name)
        w.action(rec, label, kind, end, cont, a.cost, [(k, tuple(s)) for k, s in a.pre.items()], effects)
    w.close(task.goal, starts)
    return {"actions": w.n_actions, "facts": len(w.flist), "bytes": w.words * 4}


def record_naming(a) -> tuple:
    """(rec, label) of a reader.build_task action (name ``label#rec``, ``what`` its record), for export_task."""
    label, rec = a.name.rsplit("#", 1)
    assert label == a.what.label, (a.name, a.what.label)
    return int(rec), label


# --- the replay's inputs to taskcore (taskcore/src/bans.rs) -------------------------------------------------------------

def _label_text(d, v):
    """The text of a start value ``v`` (a facts.py value, absent is 0) in domain ``d``, "" where ``d`` lacks it: the
    rule of ``Writer.close``'s starts (task.py ``SasTask.state``)."""
    lab = d.label(int(v)) if isinstance(v, (int, bool)) else v
    return "" if lab is None or lab not in d.values else _vtext(lab)


def write_start(path, facts: dict, start: dict) -> Path:
    """taskcore's ``--start-file``: one line per fact of ``facts`` (key -> Domain; the packed file's facts), its key
    and the text of its start value (``start``, a facts.py map, absent is 0), or nothing after the tab where the
    domain lacks it. Keys in sorted order."""
    path = Path(path)
    with open(path, "w") as f:
        for k in sorted(facts):
            if "\t" in k or "\n" in k:
                raise ValueError(f"a fact key with a tab or a newline: {k!r}")
            f.write(f"{k}\t{_label_text(facts[k], start.get(k, 0))}\n")
    return path


def write_bans(path, bans) -> Path:
    """taskcore's ``--bans``: one line per ban (replay.Ban, or (label, ((key, value), ...))), in order: the label,
    then each key and its value's text."""
    path = Path(path)
    with open(path, "w") as f:
        for ban in bans:
            label, values = (ban.label, ban.values) if hasattr(ban, "label") else ban
            parts = [label] + [x for k, v in values for x in (k, _vtext(v))]
            if any("\t" in p or "\n" in p for p in parts):
                raise ValueError(f"a ban with a tab or a newline: {parts!r}")
            f.write("\t".join(parts) + "\n")
    return path


def kitchen(out_dir: str) -> None:
    """The kitchen files from READER_PICKLE (test_reader's demo): kitchen.tc (all records; starts s17, the demo's, and
    replan, test_reader's replan fixture) and kitchen-no211.tc (without local 211's own event; start s17)."""
    import resource
    here = Path(__file__).resolve().parent
    sys.path.insert(0, str(here))
    sys.path.insert(0, str(here.parents[1] / "src"))
    import reader as R
    import test_reader as TR
    t0 = time.monotonic()
    b, recs, _ = pickle.loads(Path(os.environ["READER_PICKLE"]).read_bytes())
    t1 = time.monotonic()
    goal = {f"owner:{TR.MEAT}": {TR.start()["var:1"]}}
    # the starts: task_start and replan_start read only the task's fact keys (pos:/escpos:/args:); every key of the
    # full task is a key of the file, so they get the full table's keys
    keys = set(goal)
    for r in recs:
        keys |= set(r.pre) | set(r.effects) | {e[1] for e in r.effects.values() if e[0] in ("when", "copy")}

    class Keys:
        facts = dict.fromkeys(keys)
    s17 = R.task_start(b, TR.start(), Keys)
    replan = R.replan_start(b, TR.replan_observed(), {}, Keys)[0]
    t2 = time.monotonic()
    out = Path(out_dir)
    c1 = export_records(out / "kitchen.tc", b, recs, goal, {"s17": s17, "replan": replan})
    t3 = time.monotonic()
    kept = [r for r in recs if r.label != "event(local:211@33)"]
    c2 = export_records(out / "kitchen-no211.tc", b, kept, goal, {"s17": s17})
    t4 = time.monotonic()
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1e6
    info = {"seconds": {"load the reading": round(t1 - t0, 2), "starts": round(t2 - t1, 2),
                        "kitchen.tc": round(t3 - t2, 2), "kitchen-no211.tc": round(t4 - t3, 2)},
            "kitchen.tc": c1, "kitchen-no211.tc": c2, "peak_rss_gb": round(rss, 3)}
    import json
    (out / "export.json").write_text(json.dumps(info, indent=1))
    print(json.dumps(info))


if __name__ == "__main__":
    if sys.argv[1:2] == ["kitchen"]:
        kitchen(sys.argv[2])
    else:
        print(__doc__)
