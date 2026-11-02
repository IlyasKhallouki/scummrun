"""E008: the guide, a static write map of the game's code regressed from a goal, and the room rings (plan.md, "The
guide" and "Interfaces").

usage: guide.py map --goal K=V[,K=V] | part1 --out GUIDE.json [--wide]
       guide.py stats GUIDE.json [--key ../E001-goals-from-scripts/key.json]
       from guide import load, depth, ring

Computed from the game files only: E003's ``model.Game.load()`` (the IR of every unit, each object's room, the pseudo
room map), ``trace_explain.Code`` (control flow and write sites) and the seed's start row
(``results/start-part1.jsonl``) for the start room and the globals the start runs. It reads no reading, runs no engine
and names no game number; the constants below are the engine's tables.

* **Atoms** are (key, value), value None meaning any value. Keys: ``var:N``, ``bit:N``, ``owner:O``, ``state:O``,
  ``class:O:C`` and ``room``.
* **Writers** W(k, v): the units with a write site of k (``Code.write_sites(unit, k, v, direct=True)``); for
  ``state:O`` also every unit of O's room with a ``drawObject``, which clears the objects drawn at its place
  (scope.rs's rule, script_v5.cpp:1122-1126); for ``room = R`` the units whose own code loads R by
  ``rooms_from_files.py``'s rules (a constant ``loadRoom``/``loadRoomWithEgo`` through the pseudo room map, a
  variable's assigned constants, a global's room argument at a constant call site (the caller loads it),
  ``actorFollowCamera`` through a ``putActorInRoom`` of the same unit); ``room`` with any value: also every other room
  change.
* **Needs** N(u, i) of write site i of unit u: (a) the tests on the way, every fact the condition of a conditional jump
  from which i is reachable (``Code.reachable``, across yields) reads: a variable, a bit, an object's state, owner or
  class through a getter with a constant object, as (k, c) for each constant c the condition compares it with, else
  (k, any); a class test's value is its class operand's set bit (``classOfIs``, 0x80). The unit's registers, its
  locals and the result register VAR_RESULT that getters and embedded opcodes write, are followed back to their writes
  in the unit that reach the jump (a register that reaches the unit's start is the start's argument or another
  script's, and adds nothing). The variables a room change writes (``trace_explain.ROOM_VARS``) read the room: VAR_ROOM,
  VAR_ROOM_RESOURCE and VAR_NEW_ROOM as ``room``; VAR_CAMERA_MIN_X, _MAX_X and VAR_ROOM_FLAG add nothing, entering a
  room sets them. The engine's clocks and ``model.non_fact_vars`` are not facts and add nothing.
  (b) how u starts: ``room = r`` for room r's entry, exit and local code; for an object's verb code its room and
  (owner:O, any); for a global, the needs at every constant start of it (the tests on the way to that start and how
  its unit starts), the least fixpoint over the start graph; the hooks' and the start's running globals have none.
* **Regression**: breadth first from the goal's atoms (depth 0); for an atom of depth d, every atom in N(u, i) of each
  of its writers' sites i that has no depth yet gets d + 1. R is the set of atoms reached.
* **Wide** (``--wide``, and its counts always in ``stats.wide``): writes through an index or to an object that is not
  a constant (``direct=False``), and computed starts (a start whose script or object is not a constant) as starts of
  every global, and of every object's verb code.
* **Rooms**: the start's room, each room's edges as ``rooms_from_files.py`` computes them (its code and the globals it
  starts by a constant number, transitively, plus the start's globals and hooks), the rooms reached from the start.

The file (format ``e008-guide-1``) is the same bytes on every run but for ``stats.seconds``. Two rules every reader of
it applies: depth(k, v) is the least depth of [k, v] and [k, null]; ring(R, allowed) = sorted({R} | edges[R]) & allowed.
"""

import argparse
import collections
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
E003 = HERE.parents[0] / "E003-path-through-cloud"
sys.path.insert(0, str(E003))

import facts as F  # noqa: E402
import ir  # noqa: E402
import model as M  # noqa: E402
import trace_explain as TE  # noqa: E402

FORMAT = "e008-guide-1"
HOOKS = (27, 28, 29, 30, 31, 32, 33, 34, 35, 36)  # VAR_SCROLL_SCRIPT .. VAR_CUTSCENE_END_SCRIPT (vars.cpp:62-71)
CLASS_SET = 0x80  # classOfIs / setClass: a class operand with bit 0x80 means "has" / "set" (script_v5.cpp:1490-1517)
OBJ_KINDS = ("state", "owner", "class")
RESULT = ir.Var(M.VAR_RESULT)  # the scripts' result register: getters and embedded opcodes write it (o5_expression)
# Variables startScene writes on every room change (trace_explain.ROOM_VARS, room.cpp:76-205): VAR_ROOM,
# VAR_ROOM_RESOURCE and VAR_NEW_ROOM hold the room, so a test of them reads the room; VAR_CAMERA_MIN_X, _MAX_X and
# VAR_ROOM_FLAG are set by entering any room, which the room code's own start need (room = r) already asks for.
ROOM_HOLDERS = (M.VAR_ROOM, M.VAR_ROOM_RESOURCE, M.VAR_NEW_ROOM)
ROOM_SET = tuple(n for n in TE.ROOM_VARS if n not in ROOM_HOLDERS)
# the ops that write an object's state, owner and class (trace_explain.Code.write_sites)
WRITE_OPS = {"state": ("setState", "drawObject", "pickupObject"), "owner": ("setOwnerOf", "pickupObject"),
             "class": ("setClass", "pickupObject")}
OBJ_OPS = tuple(sorted({op for ops in WRITE_OPS.values() for op in ops}))
START_ROWS = E003 / "results" / "start-part1.jsonl"
END_PART1 = E003 / "results" / "end-part1.json"


def unit_name(key: tuple) -> str:
    return ":".join(str(x) for x in key)


def atom_str(k: str, v) -> str:
    """A writers map key: 'k=v', 'k=null' for any value."""
    return f"{k}={json.dumps(v)}"


def atom_order(a: tuple) -> tuple:
    return (a[0], str(a[1]))


def start_row(seed: int = 1) -> dict:
    rows = [json.loads(line) for line in START_ROWS.read_text().splitlines() if line.strip()]
    return next(r for r in rows if r["seed"] == seed)["facts"]


def parse_goal(text: str) -> dict:
    """``K=V[,K=V]`` or ``part1`` (E003's results/end-part1.json goal_all) as {key: [values]}."""
    if text == "part1":
        pairs = json.loads(END_PART1.read_text())["goal_all"].items()
    else:
        pairs = [p.split("=", 1) for p in text.split(",") if p.strip()]
    goal = {}
    for k, v in pairs:
        vals = goal.setdefault(k.strip(), [])
        if int(v) not in vals:
            vals.append(int(v))
    return {k: sorted(vs) for k, vs in sorted(goal.items())}


class Map:
    """The static write map of the game's units: writers of an atom, the needs of a write site, the room graph."""

    def __init__(self, game: M.Game, start: dict, wide: bool = False, code: TE.Code | None = None):
        self.g = game
        self.code = code or TE.Code(game)
        self.wide = wide
        self.start = start
        self.res = lambda r: game.pseudo.get(r, r)  # noqa: E731
        self.nonfacts = self.code.nonfacts
        self._preds: dict = {}
        self._tests: dict = {}
        self._conds: dict = {}
        self._writers: dict = {}
        self._room_units()
        self._index()
        self._starts()

    # --- rooms (rooms_from_files.py's rules, per unit) -------------------------------------------------------------

    def _room_units(self) -> None:
        g = self.g
        var_consts = collections.defaultdict(set)  # every constant each variable is assigned anywhere
        arg_rooms = collections.defaultdict(set)  # global -> the locals it loads a room from
        for k, u in g.units.items():
            for x in u.ins:
                st = x.st
                if isinstance(st, ir.Assign) and st.op == "=" and isinstance(st.lhs, ir.Var) and \
                        isinstance(st.rhs, ir.Const):
                    var_consts[st.lhs.n].add(st.rhs.value)
                if k[0] == "global" and isinstance(st, ir.Op) and st.name in ("loadRoom", "loadRoomWithEgo"):
                    arg = st.args[0] if st.name == "loadRoom" else st.args[1]
                    if isinstance(arg, ir.Local):
                        arg_rooms[k[1]].add(arg.n)
        # per unit: the globals it starts by a constant number, {room: the sites that load it}, the room changes
        # that are not constants, every room change op's site
        self.unit_globals, self.unit_rooms, self.unit_wild, self.unit_changes = {}, {}, {}, {}
        for k, u in g.units.items():
            gl, rooms, wild, changes = set(), collections.defaultdict(set), 0, set()
            follow = [i for i, x in enumerate(u.ins) if isinstance(x.st, ir.Op) and x.st.name == "actorFollowCamera"]
            for i, x in enumerate(u.ins):
                st = x.st
                if isinstance(st, ir.Start) and st.name != "startObject" and isinstance(st.script, ir.Const):
                    if 0 < st.script.value < g.first_local:
                        gl.add(st.script.value)
                    for j in sorted(arg_rooms.get(st.script.value, ())):
                        a = st.args[j] if j < len(st.args) else None
                        if isinstance(a, ir.Const) and a.value > 0:
                            rooms[self.res(a.value)].add(i)
                if follow and isinstance(st, ir.Op) and st.name == "putActorInRoom" and \
                        isinstance(st.args[1], ir.Const) and st.args[1].value > 0:
                    rooms[self.res(st.args[1].value)].update(follow)
                if isinstance(st, ir.Op) and st.name in ir.ROOM_CHANGE:
                    changes.add(i)
                if isinstance(st, ir.Op) and st.name in ("loadRoom", "loadRoomWithEgo"):
                    arg = st.args[0] if st.name == "loadRoom" else st.args[1]
                    if isinstance(arg, ir.Const):
                        rooms[self.res(arg.value)].add(i)
                    elif isinstance(arg, ir.Var) and arg.n in var_consts:
                        for v in var_consts[arg.n]:
                            if v > 0:
                                rooms[self.res(v)].add(i)
                    elif not isinstance(arg, ir.Local):
                        wild += 1
            self.unit_globals[k] = gl
            self.unit_rooms[k] = {r: frozenset(s) for r, s in rooms.items()}
            self.unit_wild[k] = wild
            self.unit_changes[k] = frozenset(changes | {i for s in rooms.values() for i in s})
        self._room_by = collections.defaultdict(list)  # room (None: any) -> the units that load it, in order
        for k in sorted(g.units):
            for r in self.unit_rooms[k]:
                self._room_by[r].append(k)
            if self.unit_changes[k]:
                self._room_by[None].append(k)

    def room_code(self, r: int) -> list:
        """Room r's own code: its entry, exit and local scripts and the verb code of its objects."""
        g = self.g
        ks = [k for k in g.units if k[0] in ("entry", "exit", "local") and k[1] == r]
        ks += [("obj", o) for o, rr in g.object_room.items() if rr == r and ("obj", o) in g.units]
        return ks

    def _closure(self, keys) -> set:
        todo, seen, rooms = list(keys), set(), set()
        while todo:
            k = todo.pop()
            if k in seen or k not in self.g.units:
                continue
            seen.add(k)
            rooms |= set(self.unit_rooms[k])
            todo += [("global", n) for n in self.unit_globals[k]]
        return rooms

    def start_globals(self) -> list:
        """The globals the start runs or installs as hooks (rooms_from_files.py's ``everywhere``)."""
        s = self.start
        out = [("global", s.get(f"var:{h}", 0)) for h in HOOKS if s.get(f"var:{h}", 0)]
        out += [("global", int(k.split(":")[1])) for k, v in s.items() if k.startswith("script:") and v == 1]
        return out

    def rooms(self) -> dict:
        common = self._closure(self.start_globals())
        r0 = self.res(self.start["room"])
        reached, todo, edges = {r0}, [r0], {}
        while todo:
            r = todo.pop()
            rs = (self._closure(self.room_code(r)) | common) - {0}
            edges[r] = sorted(rs)
            for x in rs:
                if x not in reached:
                    reached.add(x)
                    todo.append(x)
        return {"start": r0, "edges": {str(r): edges[r] for r in sorted(edges)}, "reached": sorted(reached)}

    # --- writers ---------------------------------------------------------------------------------------------------

    def _index(self) -> None:
        """The instructions that can be a direct write site of a key: a superset of ``Code.write_sites(direct=True)``'s
        sites, per variable, bit and object; and the sites ``direct=False`` adds, a write through an index (with its
        constant value or None) or an op on an object that is not a constant (with its op), for the wide map."""
        g = self.g
        by = collections.defaultdict(lambda: collections.defaultdict(set))  # ("var", n) / ("obj", o) -> unit -> sites
        wild = collections.defaultdict(list)  # "var" / "bit" -> [(unit, site, value)]; "obj" -> [(unit, site, op)]
        self.draws = {}  # unit -> its drawObject sites
        for k, u in g.units.items():
            draws = set()
            for i, x in enumerate(u.ins):
                st = x.st
                if st is None:
                    continue
                for y in M._walk(st):
                    if isinstance(y, ir.Embed):
                        lhs = y.st.lhs
                        if isinstance(lhs, (ir.Var, ir.Bit)):
                            by[("var" if isinstance(lhs, ir.Var) else "bit", lhs.n)][k].add(i)
                if isinstance(st, ir.Assign):
                    if isinstance(st.lhs, (ir.Var, ir.Bit)):
                        by[("var" if isinstance(st.lhs, ir.Var) else "bit", st.lhs.n)][k].add(i)
                    elif isinstance(st.lhs, ir.Index) and st.lhs.kind in ("var", "bit"):
                        v = st.rhs.value if st.op == "=" and isinstance(st.rhs, ir.Const) else None
                        wild[st.lhs.kind].append((k, i, v))
                elif isinstance(st, ir.Op):
                    if st.name == "setVarRange" and isinstance(st.args[0], (ir.Var, ir.Bit)):
                        kind = "var" if isinstance(st.args[0], ir.Var) else "bit"
                        for n in range(st.args[0].n, st.args[0].n + len(st.args[2].items)):
                            by[(kind, n)][k].add(i)
                    if st.name in ir.ROOM_CHANGE:
                        for n in TE.ROOM_VARS:
                            by[("var", n)][k].add(i)
                    if st.name in TE.CURSOR_VARS:
                        by[("var", TE.CURSOR_VARS[st.name])][k].add(i)
                    if st.name in OBJ_OPS and st.args:
                        if isinstance(st.args[0], ir.Const):
                            by[("obj", st.args[0].value)][k].add(i)
                        else:
                            wild["obj"].append((k, i, st.name))
                    if st.name == "drawObject":
                        draws.add(i)
            if draws:
                self.draws[k] = frozenset(draws)
        self._by = {idx: {k: sorted(v[k]) for k in sorted(v)} for idx, v in by.items()}
        self._wild = {k: sorted(v) for k, v in wild.items()}

    def wide_sites(self, key: str, value) -> list:
        """The sites ``Code.write_sites(direct=False)`` adds to the direct ones: [(unit, site)]."""
        kind = key.partition(":")[0]
        if kind in ("var", "bit"):
            return [(k, i) for k, i, v in self._wild.get(kind, ()) if value is None or v is None or v == value]
        if kind in OBJ_KINDS:
            return [(k, i) for k, i, op in self._wild.get("obj", ()) if op in WRITE_OPS[kind]]
        return []

    def candidates(self, key: str) -> dict:
        """{unit: the instructions that can write ``key`` directly}, units in order."""
        kind, _, rest = key.partition(":")
        if kind in ("var", "bit"):
            idx = (kind, int(rest))
        elif kind in OBJ_KINDS:
            idx = ("obj", int(rest.split(":")[0]))
        else:
            return {}
        return self._by.get(idx, {})

    def direct_sites(self, unit, key: str, value) -> frozenset:
        """``Code.write_sites(unit, key, value, direct=True)``, its rule applied to the indexed instructions only."""
        kind, _, rest = key.partition(":")
        ins = self.g.units[unit].ins
        out = set()
        if kind in ("var", "bit"):
            n = int(rest)
            want = ir.Var(n) if kind == "var" else ir.Bit(n)
            for i in self.candidates(key).get(unit, ()):
                st = ins[i].st
                if any(isinstance(y, ir.Embed) and y.st.lhs == want for y in M._walk(st)):
                    out.add(i)
                elif isinstance(st, ir.Assign):
                    if st.lhs == want and (value is None or st.op != "=" or not isinstance(st.rhs, ir.Const) or
                                           st.rhs.value == value):
                        out.add(i)
                elif isinstance(st, ir.Op):
                    if st.name == "setVarRange" and isinstance(st.args[0], type(want)):
                        if st.args[0].n <= n < st.args[0].n + len(st.args[2].items):
                            out.add(i)
                    elif (kind == "var" and n in TE.ROOM_VARS and st.name in ir.ROOM_CHANGE) or \
                            (kind == "var" and TE.CURSOR_VARS.get(st.name) == n):
                        out.add(i)
        elif kind in OBJ_KINDS:
            o = int(rest.split(":")[0])
            for i in self.candidates(key).get(unit, ()):
                st = ins[i].st
                if isinstance(st, ir.Op) and st.name in WRITE_OPS[kind] and st.args and \
                        isinstance(st.args[0], ir.Const) and st.args[0].value == o:
                    out.add(i)
        return frozenset(out)

    def writers(self, key: str, value) -> list:
        """[(unit, sites)] of the atom (key, value), units in sorted order."""
        a = (key, value)
        if a in self._writers:
            return self._writers[a]
        out = {}
        if key == "room":
            for k in self._room_by.get(value, ()):
                out[k] = set(self.unit_changes[k] if value is None else self.unit_rooms[k][value])
        else:
            for k in self.candidates(key):
                sites = self.direct_sites(k, key, value)
                if sites:
                    out.setdefault(k, set()).update(sites)
            if self.wide:
                for k, i in self.wide_sites(key, value):
                    out.setdefault(k, set()).add(i)
            if key.startswith("state:"):
                room = self.g.object_room.get(int(key.split(":")[1]))
                if room is not None:
                    for k in self.room_code(room):
                        if k in self.draws:
                            out.setdefault(k, set()).update(self.draws[k])
        res = [(k, frozenset(out[k])) for k in sorted(out)]
        self._writers[a] = res
        return res

    # --- needs: the tests on the way -------------------------------------------------------------------------------

    def preds(self, key) -> list:
        """Each instruction's predecessors along ``Code.succ`` across yields (the reverse of ``Code.reachable``)."""
        if key not in self._preds:
            n = len(self.g.units[key].ins)
            p = [[] for _ in range(n + 1)]
            for i in range(n):
                for j in self.code.succ(key, i, True):
                    if j is not None and j <= n:
                        p[j].append(i)
            self._preds[key] = p
        return self._preds[key]

    def reaching(self, key, i: int) -> set:
        """The instructions j with ``Code.reachable(key, j, {i}, cross_yields=True)``: i and its ancestors."""
        p = self.preds(key)
        seen, todo = {i}, [i]
        while todo:
            m = todo.pop()
            for j in p[m]:
                if j not in seen:
                    seen.add(j)
                    todo.append(j)
        return seen

    def writes_reg(self, key, m: int, x):
        """The value expression register ``x`` (a local, or VAR_RESULT) gets at instruction m: (op, expression), or
        None when m does not write it. A statement's embedded opcode writes VAR_RESULT (o5_expression case 6)."""
        st = self.g.units[key].ins[m].st
        if st is None:
            return None
        if isinstance(st, ir.Assign) and st.lhs == x:
            return st.op, st.rhs
        if x == RESULT:
            last = None
            for y in M._walk(st):
                if isinstance(y, ir.Embed) and y.st.lhs == x:
                    last = (y.st.op, y.st.rhs)
            return last
        return None

    def defs(self, key, j: int, x) -> list:
        """The writes of register x in unit ``key`` that reach instruction j with no other write of x between:
        [(site, op, expression)], sites in order. A way back that reaches the unit's start adds nothing (a local
        there is the start's argument, VAR_RESULT another script's)."""
        p = self.preds(key)
        seen, todo, out = set(), list(p[j]), []
        while todo:
            m = todo.pop()
            if m in seen:
                continue
            seen.add(m)
            w = self.writes_reg(key, m, x)
            if w is not None:
                out.append((m, w[0], w[1]))
                continue
            todo += p[m]
        return sorted(out, key=lambda t: t[0])

    def reads(self, key, j: int, e, value, out: set, stack: frozenset) -> None:
        """Add to ``out`` the atoms expression e at instruction j reads: (k, value) where e is directly the fact
        (``value`` the constant it is compared with, or None), registers followed back to their writes."""
        if isinstance(e, ir.Var) and e.n != RESULT.n:
            if e.n in ROOM_HOLDERS:
                out.add(("room", None if value is None else self.res(value)))
            elif e.n not in ROOM_SET and e.n not in F.CLOCKS and e.n not in self.nonfacts:
                out.add((f"var:{e.n}", value))
        elif isinstance(e, ir.Bit):
            out.add((f"bit:{e.n}", value))
        elif isinstance(e, ir.Local) or e == RESULT:
            if (key, j, e) in stack:
                return
            for m, op, rhs in self.defs(key, j, e):
                if op == "=":
                    self.reads(key, m, rhs, value, out, stack | {(key, j, e)})
                else:  # x op= rhs reads x too
                    self.reads(key, m, rhs, None, out, stack | {(key, j, e)})
                    self.reads(key, m, e, None, out, stack | {(key, j, e)})
        elif isinstance(e, ir.Embed):
            self.reads(key, j, e.st.rhs, value, out, stack)
        elif isinstance(e, ir.Getter):
            o = e.args[0].value if e.args and isinstance(e.args[0], ir.Const) else None
            if o is not None and e.name == "getObjectState":
                out.add((f"state:{o}", value))
            elif o is not None and e.name == "getObjectOwner":
                out.add((f"owner:{o}", value))
            elif o is not None and e.name == "classOfIs" and len(e.args) > 1:
                items = e.args[1].items if isinstance(e.args[1], ir.List) else (e.args[1],)
                for c in items:
                    if isinstance(c, ir.Const):
                        out.add((f"class:{o}:{c.value & ~CLASS_SET}", 1 if c.value & CLASS_SET else 0))
            for a in e.args:
                self.reads(key, j, a, None, out, stack)
        elif isinstance(e, ir.Cmp):
            if isinstance(e.b, ir.Const):
                self.reads(key, j, e.a, e.b.value, out, stack)
            elif isinstance(e.a, ir.Const):
                self.reads(key, j, e.b, e.a.value, out, stack)
            else:
                self.reads(key, j, e.a, None, out, stack)
                self.reads(key, j, e.b, None, out, stack)
        elif isinstance(e, (ir.Not, ir.Truth, ir.Neg)):
            self.reads(key, j, e.x, None, out, stack)
        elif isinstance(e, ir.BinOp):
            self.reads(key, j, e.a, None, out, stack)
            self.reads(key, j, e.b, None, out, stack)
        elif isinstance(e, ir.Index):
            self.reads(key, j, e.offset, None, out, stack)
        elif isinstance(e, (ir.List, ir.Concat)):
            for y in (e.items if isinstance(e, ir.List) else e.parts):
                self.reads(key, j, y, None, out, stack)
        elif isinstance(e, tuple):
            for y in e:
                self.reads(key, j, y, None, out, stack)

    def cond_atoms(self, key, j: int) -> frozenset:
        """The atoms the condition of conditional jump j reads."""
        ck = (key, j)
        if ck not in self._conds:
            out = set()
            self.reads(key, j, self.g.units[key].ins[j].st.cond, None, out, frozenset())
            self._conds[ck] = frozenset(out)
        return self._conds[ck]

    def tests(self, key, i: int) -> frozenset:
        """Need (a): every fact the conditions of the conditional jumps from which site i is reachable read."""
        tk = (key, i)
        if tk in self._tests:
            return self._tests[tk]
        u = self.g.units[key]
        out = set()
        for j in self.reaching(key, i):
            if j < len(u.ins) and isinstance(u.ins[j].st, ir.Jump) and u.ins[j].st.cond is not None:
                out |= self.cond_atoms(key, j)
        res = frozenset(out)
        self._tests[tk] = res
        return res

    # --- needs: how a unit starts ----------------------------------------------------------------------------------

    def _starts(self) -> None:
        """Need (b) for every unit: the least fixpoint of the start needs over the constant start graph."""
        g = self.g
        hooks = {s for s in (self.start.get(f"var:{h}", 0) for h in HOOKS) if s}
        for k, u in g.units.items():
            for x in u.ins:
                st = x.st
                if isinstance(st, ir.Assign) and isinstance(st.lhs, ir.Var) and st.lhs.n in HOOKS and \
                        isinstance(st.rhs, ir.Const) and st.rhs.value:
                    hooks.add(st.rhs.value)
        self.free = {("global", n) for n in hooks} | set(self.start_globals())
        starters = collections.defaultdict(list)  # global unit -> [(starting unit, site)]
        computed, computed_obj = [], []  # starts whose number / object is not a constant (wide)
        for k, u in g.units.items():
            for i, x in enumerate(u.ins):
                st = x.st
                if not isinstance(st, ir.Start):
                    continue
                if st.name == "startObject":
                    if not isinstance(st.obj, ir.Const):
                        computed_obj.append((k, i))
                elif isinstance(st.script, ir.Const):
                    n = st.script.value
                    if 0 < n < g.first_local and ("global", n) in g.units:
                        starters[("global", n)].append((k, i))
                else:
                    computed.append((k, i))
        base = {}
        for k in g.units:
            if k[0] in ("entry", "exit", "local"):
                base[k] = frozenset({("room", k[1])})
            elif k[0] == "obj":
                r = g.object_room.get(k[1])
                base[k] = frozenset({(f"owner:{k[1]}", None)} | ({("room", r)} if r is not None else set()))
            else:
                base[k] = frozenset()
        sn = dict(base)
        edges = {}
        for k in sorted(g.units):
            if k[0] == "global" and k not in self.free:
                edges[k] = list(starters.get(k, ())) + (computed if self.wide else [])
            elif k[0] == "obj" and self.wide:
                edges[k] = list(computed_obj)
        changed = True
        while changed:
            changed = False
            for k in sorted(edges):
                new = set(sn[k])
                for v, i in edges[k]:
                    new |= self.tests(v, i) | sn[v]
                if len(new) != len(sn[k]):
                    sn[k] = frozenset(new)
                    changed = True
        self.start_needs = sn
        self.starters = starters

    def needs(self, key, i: int) -> frozenset:
        return self.tests(key, i) | self.start_needs[key]


def regress(m: Map, goal: dict) -> tuple:
    """Breadth first from the goal's atoms. Returns ({atom: depth}, {atom: the atom it was first reached from})."""
    depth = {(k, v): 0 for k, vs in goal.items() for v in vs}
    parent = {}
    frontier = sorted(depth, key=atom_order)
    d = 0
    while frontier:
        nxt = []
        for a in frontier:
            for k, sites in m.writers(*a):
                for i in sorted(sites):
                    for b in sorted(m.needs(k, i), key=atom_order):
                        if b not in depth:
                            depth[b] = d + 1
                            parent[b] = a
                            nxt.append(b)
        frontier = sorted(nxt, key=atom_order)
        d += 1
    return depth, parent


def build(goal: dict, wide: bool = False, seed: int = 1, game: M.Game | None = None) -> dict:
    """The GUIDE dict of format e008-guide-1 for ``goal`` ({key: [values]})."""
    t0 = time.time()
    g = game or M.Game.load()
    start = start_row(seed)
    code = TE.Code(g)
    maps = {w: Map(g, start, wide=w, code=code) for w in (False, True)}
    out_map = maps[wide]
    depths = {w: regress(maps[w], goal)[0] for w in (False, True)}
    depth = depths[wide]
    atoms = sorted(depth, key=lambda a: (depth[a],) + atom_order(a))
    keys = {}
    for a in atoms:
        keys.setdefault(a[0], depth[a])
    writers, no_writer, units = {}, [], set()
    for a in atoms:
        ws = out_map.writers(*a)
        if ws:
            writers[atom_str(*a)] = sorted(unit_name(k) for k, _ in ws)
            units |= {k for k, _ in ws}
        else:
            no_writer.append([a[0], a[1]])
    hist = collections.Counter(depth.values())
    wd = depths[True]
    return {
        "format": FORMAT,
        "goal": goal,
        "map": "wide" if wide else "direct",
        "rooms": out_map.rooms(),
        "atoms": [[a[0], a[1], depth[a]] for a in atoms],
        "keys": keys,
        "writers": writers,
        "stats": {"atoms": len(atoms), "keys": len(keys), "depth_hist": {str(d): hist[d] for d in sorted(hist)},
                  "units": len(units), "seconds": round(time.time() - t0, 3), "no_writer": no_writer,
                  "wide": {"atoms": len(wd), "keys": len({a[0] for a in wd})}},
    }


def dumps(guide: dict) -> str:
    """One line per top-level field, compact values: the same bytes for the same dict."""
    items = [f"{json.dumps(k)}: {json.dumps(v, separators=(',', ':'))}" for k, v in guide.items()]
    return "{" + ",\n ".join(items) + "}\n"


# --- the reader's rules -----------------------------------------------------------------------------------------------

def load(path) -> dict:
    g = json.loads(Path(path).read_text())
    if g.get("format") != FORMAT:
        raise ValueError(f"{path}: format {g.get('format')!r}, not {FORMAT}")
    return g


def _depths(guide: dict) -> dict:
    d = guide.get("_depth")
    if d is None:
        d = {}
        for k, v, n in guide["atoms"]:
            d[(k, v)] = min(n, d.get((k, v), n))
        guide["_depth"] = d
    return d


def depth(guide: dict, key: str, value) -> int | None:
    """The least depth of [key, value] and [key, null]; None when neither is in R."""
    d = _depths(guide)
    got = [x for x in (d.get((key, value)), d.get((key, None))) if x is not None]
    return min(got) if got else None


def ring(guide: dict, room: int, allowed=None) -> list:
    """sorted({room} | edges[room]) & allowed (every room when allowed is None)."""
    rs = {room} | set(guide["rooms"]["edges"].get(str(room), ()))
    if allowed is not None:
        rs &= set(allowed)
    return sorted(rs)


# --- stats ------------------------------------------------------------------------------------------------------------

def key_fact(f: list) -> tuple:
    """E001's [kind, n, v] as (key, value, negated): a class fact's sign says set (+C) or cleared (-C); owner_not is
    the owner's key with any other value."""
    kind, n, v = f
    if kind == "class":
        return f"class:{n}:{abs(v)}", 1 if v > 0 else 0, False
    if kind == "owner_not":
        return f"owner:{n}", v, True
    return f"{kind}:{n}", v, False


def recall(guide: dict, key_path) -> dict:
    """Evaluation only: how many of the key's Part I facts have their key in R, and their (key, value) or (key, null)
    in R (for owner_not: (key, null) or a value other than the excluded one)."""
    entries = [e for e in json.loads(Path(key_path).read_text())["key"] if e.get("part") == "I"]
    d = _depths(guide)
    in_key = in_value = 0
    for e in entries:
        k, v, neg = key_fact(e["fact"])
        if k in guide["keys"]:
            in_key += 1
        if neg:
            ok = (k, None) in d or any(kk == k and vv is not None and vv != v for kk, vv in d)
        else:
            ok = (k, v) in d or (k, None) in d
        in_value += ok
    return {"facts": len(entries), "key_in_R": in_key, "atom_in_R": in_value}


def stats_text(guide: dict, key_path=None) -> str:
    s = guide["stats"]
    lines = [f"goal {json.dumps(guide['goal'])}  map {guide['map']}",
             f"|R| {s['atoms']} atoms over {s['keys']} keys, depth at most {max(map(int, s['depth_hist']))}, "
             f"writer units {s['units']}, {s['seconds']} s",
             "depth histogram " + " ".join(f"{d}:{n}" for d, n in s["depth_hist"].items()),
             f"wide map: |R| {s['wide']['atoms']} atoms over {s['wide']['keys']} keys",
             f"atoms with no writer: {len(s['no_writer'])}"]
    for k, vs in guide["goal"].items():
        for v in vs:
            lines.append(f"writers of {atom_str(k, v)}: {' '.join(guide['writers'].get(atom_str(k, v), [])) or 'none'}")
    for r in ("room",):
        got = [f"{v}@{n}" for k, v, n in guide["atoms"] if k == r]
        lines.append(f"room atoms (value@depth): {' '.join(got[:40])}{' ...' if len(got) > 40 else ''}")
    rm = guide["rooms"]
    lines.append(f"rooms: start {rm['start']}, reached {len(rm['reached'])}")
    if key_path:
        r = recall(guide, key_path)
        lines.append(f"key (evaluation only): {r['facts']} Part I facts, key in R {r['key_in_R']}, "
                     f"(key, value) or (key, null) in R {r['atom_in_R']}")
    return "\n".join(lines) + "\n"


def main(argv=None) -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    mp = sub.add_parser("map")
    mp.add_argument("--goal", required=True, help="K=V[,K=V] or part1")
    mp.add_argument("--out", required=True)
    mp.add_argument("--wide", action="store_true")
    mp.add_argument("--seed", type=int, default=1)
    sp = sub.add_parser("stats")
    sp.add_argument("guide")
    sp.add_argument("--key")
    a = ap.parse_args(argv)
    if a.cmd == "map":
        guide = build(parse_goal(a.goal), wide=a.wide, seed=a.seed)
        Path(a.out).write_text(dumps(guide))
        sys.stdout.write(stats_text(guide))
    else:
        sys.stdout.write(stats_text(load(a.guide), a.key))


if __name__ == "__main__":
    main()
