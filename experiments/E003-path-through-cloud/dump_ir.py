"""E003: the oracle for the Rust port of the script layer (scumm/ir). It writes, as canonical JSON lines, everything
ir.py, facts.py and model.Game compute from the game files alone, before any Machine runs. The Rust side
(``cargo run --release --bin dump_ir``) writes the same lines; the two must agree byte for byte.

usage: ../../.venv/bin/python dump_ir.py [--out DIR]

Writes under DIR (default ROOT/out/scumm-ir/, which git ignores: the dump holds script text):

* ``ir-python.jsonl.gz``: the full dump (below);
* ``ir-python-noblocks.jsonl.gz``: the units again, read without the raw blocks (``ir.load(blocks=None)``), so the
  else-if text rule and the unverified var names are compared too. Unit lines only, IR fields only;
* ``ir-python-synthetic.jsonl.gz``: what the scripts never exercise, from scumm/ir/tests/synthetic.json: statements
  through ir.parse_statement (malformed ones give their error message), and small units through ir._read and
  ir._entries (every note class, every end and yield rule, loops of every kind) gathered in a model.Game built by
  hand, as test_model.py's ``game`` does (below);
* ``ir-python.sources.json``: the sha1 of each Python source the dump ran, of this file and of the two fixtures;
  the Rust test reruns this dumper when one changed.

Canonical form. Each line is ``json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=True)`` and a
"\\n". The gzip wraps the text; compare the text, not the gzip bytes. Every set is a sorted list. A dict with int keys
is a list of [key, value] pairs sorted by key, except a unit's ``entries``, which keeps the event table's order. A
unit key is a list: ["global", 1], ["local", 1, 200], ["entry", 1], ["exit", 1], ["obj", 17]. An IR node is a list:
its class name, then its fields in declaration order, tuples as lists and None as null, so ``Const(5)`` is
``["Const", 5]`` and ``Assign(Var(1), "=", Const(0))`` is ``["Assign", ["Var", 1], "=", ["Const", 0]]``.

Lines, in order:

1. ``kind: "tables"``: the constant tables the port copies: ir.VARS (in its order), ir.VAR_NAME, ir.AMBIGUOUS,
   ir.FLAGS, ir.YIELDS, ir.ROOM_CHANGE, ir.HEADER, model.GETTERS, model.HOOK_VARS, model.TIME_VARS and facts.py's
   constants.
2. ``kind: "game"``: model.Game's fields (the unit keys in load order, first_local, local_rooms, object_room,
   object_parent, verb_offset, room_width, pseudo, pseudo_conflicts, resistance, wild_starts) and the game-wide
   analyses: nonfacts (``Game.nonfacts``), script_writes, const_poll_loops; and the answers of Game.resource for
   rooms 0-255 (null: no mapping) and of Game.resistant for the script of every global unit and resistance key, and
   for the local of every local unit and resistance key with no room, its own room and room 0.
3. one ``kind: "unit"`` line per unit, in ``ir.load`` order: the unit's fields (key, room, file, block, entries,
   shadowed, at, notes) and each instruction's (offset, op, text, file, line, the parsed statement ``st``, next,
   target, esc, yields, ends, hidden), with ``cite``, ``show`` (ir.show of st) and ``head`` (ir.head of st) and
   Game.successors. Then the unit's analyses: unit_name, lifecycle_key, joins (Game.join_points), for an object
   unit ``entry`` (Game.entry for every verb the game names, 0 and 255), and under ``const_polls`` "off" and "on"
   (switch const_polls): loops (Game.loops), wait_points (Game.wait_point for every index 0..len(ins), as
   [kind, op, the index of its loop in ``loops`` or null]), time_exits (Game.time_exits, camera off and on, as
   [jump, successor, head]) and time_loops_at (Game.time_loops_at for every index 0..len(ins)).
4. ``kind: "facts"``: facts.unit_of over a grid of where and number, and facts.facts_of over the synthetic bridge
   states in scumm/ir/tests/fact_states.json, with facts.diff and facts.patch between each state and the next.

The synthetic file has one ``kind: "statement"`` line per statement (``st``, ``show`` and ``head``, or ``error``: the
ParseError, IndexError or ValueError message ir._read would note), then one ``kind: "game"`` line for the hand-built
Game (its units, Game._resistance's result, nonfacts, script_writes, const_poll_loops), then the units' lines as in
the full dump (with ``entry`` for every verb the fixture's tables name, 0 and 255).
"""

import argparse
import gzip
import hashlib
import json
from dataclasses import fields, is_dataclass
from pathlib import Path

import facts as F
import ir
import model

HERE = Path(__file__).resolve().parent
OUT = ir.ROOT / "out" / "scumm-ir"
FACT_STATES = HERE / "scumm" / "ir" / "tests" / "fact_states.json"
SYNTHETIC = HERE / "scumm" / "ir" / "tests" / "synthetic.json"
SOURCES = ("ir.py", "facts.py", "model.py", "dump_ir.py", "scumm/ir/tests/fact_states.json",
           "scumm/ir/tests/synthetic.json")


def enc(x):
    """An IR node (or a field value) as canonical JSON: [class name, fields...]."""
    if x is None or isinstance(x, (bool, int, str)):
        return x
    if isinstance(x, tuple):
        return [enc(y) for y in x]
    if is_dataclass(x):
        return [type(x).__name__] + [enc(getattr(x, f.name)) for f in fields(x)]
    raise TypeError(f"cannot encode {x!r}")


def line(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n"


def pairs(d: dict) -> list:
    return [[k, v] for k, v in sorted(d.items())]


def tables() -> dict:
    return {
        "kind": "tables",
        "vars": [[k, v] for k, v in ir.VARS.items()],
        "var_name": pairs(ir.VAR_NAME),
        "ambiguous": [[k, list(v)] for k, v in ir.AMBIGUOUS.items()],
        "flags": sorted(ir.FLAGS),
        "yields": [[k, v] for k, v in sorted(ir.YIELDS.items())],
        "room_change": sorted(ir.ROOM_CHANGE),
        "header": [[k, v] for k, v in sorted(ir.HEADER.items())],
        "jump": ir.JUMP,
        "default_verb": ir.DEFAULT_VERB,
        "getters": [[name, kind, cite] for name, (kind, cite) in sorted(model.GETTERS.items())],
        "hook_vars": list(model.HOOK_VARS),
        "time_vars": sorted(model.TIME_VARS),
        "clocks": sorted(F.CLOCKS),
        "wio": [F.WIO_INVENTORY, F.WIO_ROOM, F.WIO_GLOBAL, F.WIO_LOCAL, F.WIO_FLOBJECT],
        "room_code": [F.ENCD, F.EXCD],
        "facts_header": F.HEADER,
    }


def all_verbs(meta: dict) -> list:
    """Every verb the game names (index.json's verbs and every object event), with 0 and 255."""
    vs = {int(v) for v in meta["verbs"]} | {0, ir.DEFAULT_VERB}
    vs |= {v["verb"] for ob in meta["objects"].values() for v in ob.get("verbs") or ()}
    return sorted(vs)


def resistant_queries(g: model.Game) -> list:
    """(key, res) pairs: script:N for every global unit and resistance key; local:N with no room, with each room
    the resistance table names for it, and with room 0; objscript:1."""
    qs = set()
    for k in list(g.units) + list(g.resistance):
        if k[0] == "global":
            qs.add((f"script:{k[1]}", -1))
        elif k[0] == "local":
            qs |= {(f"local:{k[2]}", -1), (f"local:{k[2]}", k[1]), (f"local:{k[2]}", 0)}
    out = [[key, None if res < 0 else res, g.resistant(key, None if res < 0 else res)] for key, res in
           sorted(qs, key=lambda q: (q[0].split(":")[0], int(q[0].split(":")[1]), q[1]))]
    return out + [["objscript:1", None, g.resistant("objscript:1")]]


def game_line(g: model.Game, meta: dict) -> dict:
    writes = model.script_writes(g)
    return {
        "kind": "game",
        "units": [list(k) for k in g.units],
        "first_local": g.first_local,
        "local_rooms": [[n, sorted(rs)] for n, rs in sorted(g.local_rooms.items())],
        "object_room": pairs(g.object_room),
        "object_parent": pairs(g.object_parent),
        "verb_offset": pairs(g.verb_offset),
        "room_width": pairs(g.room_width),
        "pseudo": pairs(g.pseudo),
        "pseudo_conflicts": [list(c) for c in g.pseudo_conflicts],
        "resistance": [[list(k), sorted(v)] for k, v in sorted(g.resistance.items())],
        "wild_starts": [list(w) for w in g.wild_starts],
        "nonfacts": pairs(g.nonfacts()),
        "script_writes": [[n, [enc(e) for e in vs]] for n, vs in sorted(writes.items())],
        "const_poll_loops": [[list(u), h] for u, h in g.const_poll_loops()],
        "resource": [[r, None if isinstance(v, model.Unknown) else v] for r in range(256)
                     for v in [g.resource(r)]],
        "resistant": resistant_queries(g),
        "all_verbs": all_verbs(meta),
    }


def ins_obj(x: ir.Ins) -> dict:
    return {"offset": x.offset, "op": x.op, "text": x.text, "file": x.file, "line": x.line, "cite": x.cite(),
            "st": enc(x.st), "show": ir.show(x.st) if x.st is not None else None,
            "head": ir.head(x.st) if x.st is not None else None, "next": x.next, "target": x.target, "esc": x.esc,
            "yields": x.yields, "ends": x.ends, "hidden": x.hidden}


def loop_obj(lp: model.Loop) -> dict:
    return {"head": lp.head, "body": sorted(lp.body), "exits": [list(e) for e in lp.exits], "kind": lp.kind,
            "reads": sorted(lp.reads), "non_facts": sorted(lp.non_facts)}


def analysis(g: model.Game, key: tuple, n: int, cp: bool) -> dict:
    loops = g.loops(key, cp)

    def index(lp):
        return None if lp is None else next(i for i, y in enumerate(loops) if y is lp)

    wps = [g.wait_point(key, idx, cp) for idx in range(n + 1)]
    return {"loops": [loop_obj(lp) for lp in loops],
            "wait_points": [[wp.kind, wp.op, index(wp.loop)] for wp in wps],
            "time_exits": {name: [[j, s, h] for j, (s, h) in sorted(g.time_exits(key, camera, cp).items())]
                           for name, camera in (("camera_off", False), ("camera_on", True))},
            "time_loops_at": [list(g.time_loops_at(key, idx, cp)) for idx in range(n + 1)]}


def unit_line(key: tuple, u: ir.Unit, g: model.Game | None = None, verbs: list = ()) -> dict:
    d = {"kind": "unit", "key": list(key), "room": u.room, "file": u.file, "block": u.block,
         "ins": [ins_obj(x) for x in u.ins],
         "entries": [[v, i] for v, i in u.entries.items()],
         "shadowed": [list(s) for s in u.shadowed],
         "at": [[o, i] for o, i in sorted(u.at.items())],
         "notes": [list(n) for n in u.notes]}
    if g is None:
        return d
    for i, x in enumerate(d["ins"]):
        x["successors"] = g.successors(key, i)
    d["unit_name"] = model.unit_name(key)
    d["lifecycle_key"] = model.lifecycle_key(key, g)
    d["joins"] = sorted(g.join_points(key))
    if key[0] == "obj":
        d["entry"] = [[v, g.entry(key[1], v)] for v in verbs]
    d["const_polls"] = {"off": analysis(g, key, len(u.ins), False), "on": analysis(g, key, len(u.ins), True)}
    return d


def facts_line() -> dict:
    cases = json.loads(FACT_STATES.read_text())
    got = [F.facts_of(c["state"]) for c in cases]
    diffs = []
    for (ca, a), (cb, b) in zip(zip(cases, got), zip(cases[1:], got[1:])):
        d = F.diff(a, b)
        diffs.append({"a": ca["name"], "b": cb["name"], "diff": {k: list(v) for k, v in d.items()},
                      "patch": F.patch(a, d)})
    return {"kind": "facts",
            "unit_of": [[w, n, F.unit_of(w, n)] for w in range(-1, 7) for n in (0, 1, 9, 200, F.EXCD, F.ENCD)],
            "cases": [{"name": c["name"], "facts": f} for c, f in zip(cases, got)],
            "diffs": diffs}


def synthetic_lines() -> list:
    cases = json.loads(SYNTHETIC.read_text())
    out = []
    for text in cases["statements"]:
        try:
            st = ir.parse_statement(text)
        except (ir.ParseError, IndexError, ValueError) as e:
            out.append({"kind": "statement", "text": text, "error": str(e)})
        else:
            out.append({"kind": "statement", "text": text, "st": enc(st), "show": ir.show(st), "head": ir.head(st)})
    units = {}
    for c in cases["units"]:
        key = tuple(c["key"])
        u = ir.Unit(key, c.get("room"), "test.txt", "")
        ir._read(u, c["text"], bytes.fromhex(c["data"]) if c.get("data") else None)
        if c.get("verbs"):
            ir._entries(u, c["verbs"])
        units[key] = u
    gd = cases["game"]
    g = model.Game(units=units, first_local=gd["first_local"], local_rooms={n: set(rs) for n, rs in gd["local_rooms"]},
                   object_room={}, object_parent={}, verb_offset={}, room_width={}, pseudo={}, pseudo_conflicts=[])
    resistance, wild = g._resistance()
    writes = model.script_writes(g)
    out.append({"kind": "game", "units": [list(k) for k in units],
                "resistance": [[list(k), sorted(v)] for k, v in sorted(resistance.items())],
                "wild_starts": [list(w) for w in wild], "nonfacts": pairs(g.nonfacts()),
                "script_writes": [[n, [enc(e) for e in vs]] for n, vs in sorted(writes.items())],
                "const_poll_loops": [[list(u), h] for u, h in g.const_poll_loops()]})
    verbs = sorted({0, ir.DEFAULT_VERB} | {v["verb"] for c in cases["units"] for v in c.get("verbs") or ()})
    out += [unit_line(key, u, g, verbs) for key, u in units.items()]
    return out


def sha1(path: Path) -> str:
    return hashlib.sha1(path.read_bytes()).hexdigest()


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", type=Path, default=OUT)
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    g = model.Game.load()
    meta = json.loads((ir.SCRIPTS / "index.json").read_text())
    verbs = all_verbs(meta)
    with gzip.open(args.out / "ir-python.jsonl.gz", "wt", encoding="ascii", newline="") as f:
        f.write(line(tables()))
        f.write(line(game_line(g, meta)))
        for key, u in g.units.items():
            f.write(line(unit_line(key, u, g, verbs)))
        f.write(line(facts_line()))
    with gzip.open(args.out / "ir-python-noblocks.jsonl.gz", "wt", encoding="ascii", newline="") as f:
        for key, u in ir.load(ir.SCRIPTS, None).items():
            f.write(line(unit_line(key, u)))
    with gzip.open(args.out / "ir-python-synthetic.jsonl.gz", "wt", encoding="ascii", newline="") as f:
        for d in synthetic_lines():
            f.write(line(d))
    (args.out / "ir-python.sources.json").write_text(
        json.dumps({s: sha1(HERE / s) for s in SOURCES}, indent=1, sort_keys=True) + "\n")
    print(f"wrote {args.out}: {len(g.units)} units, {sum(len(u.ins) for u in g.units.values())} instructions")


if __name__ == "__main__":
    main()
