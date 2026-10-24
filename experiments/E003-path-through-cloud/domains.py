"""E003 step 4: the finite domain of each fact the planner's task can hold, read from the IR (plan step 1, Facts).

usage: from domains import build, Domain
       doms = build(start)                     # start: a facts.py map (absent is 0); {} is the engine's init
       doms.get("var:113")                     # a Domain, or None when the fact has no finite domain (doms.why)
       uv run python experiments/E003-path-through-cloud/domains.py   (the census, results/task-domains.log)

**A Domain** is the tuple of a fact's values: ints in ascending order, with ``BELOW`` first and ``ABOVE``
last when they are there. ``ABOVE`` stands for every value above ``high`` and ``BELOW`` for every value
below ``low``. ``label(n)`` gives an engine value's value; ``test(op, c)`` the values for which ``value op
c`` holds (o5's comparisons, script_v5.cpp:1532-1677), or None when ``c`` lies where a collapsed value
cannot decide it; ``add(v, d)`` the values ``v + d`` can be (``ABOVE - 1`` is several: a write the reader
cannot work out, which the task turns into one effect per value, task.py).

**Variables** (plan step 1's rule). A variable whose writes are all constants, or itself plus or minus a
constant, once call arguments are bound, is a finite integer. Its values are the start value closed under
its writes, up to the largest constant it is compared with, plus one ``ABOVE`` value. Read from the IR:

* a write is an assignment to ``Var[n]`` (``x op= v`` as ``x = x op v``), setVarRange's constants
  (script_v5.cpp:2754-2768), or an indexed ``Var[b + i] = v`` whose index ``i`` has values here (its
  targets are ``b + i``). Constant expressions fold (C division, script_v5.cpp:994). ``Local[k]`` is
  bound when every start of its unit passes a value that folds (``locals``): startScript and chainScript
  with a constant number (a room's code reaches its own room's local, other code every room's local of
  that number, script.cpp:70-86), startObject with a constant object (script.cpp:109-154); a missing
  argument is 0 (getWordVararg, script_v5.cpp:3430-3441); room entry and exit code start with every local
  0 (initializeLocals, script.cpp:156-165, 1016, 1054). A unit the engine starts with its own arguments
  (a number written to a hook variable, VAR_SCROLL_SCRIPT..VAR_CUTSCENE_END_SCRIPT, vars.cpp:62-71; the
  sentence script gets [verb, A, B], script.cpp:1240) has engine-bound locals; a local its own unit
  writes is not bound; both are counted. Assumed, and counted with their sites: a hook write that does not
  fold starts no unit but those the constant hook writes name, and a start whose number or object does not
  fold reaches no unit whose locals are bound;
* a comparison is ``Var[n] op c`` in a jump's condition (either side), ``Var[n]`` alone or ``!Var[n]``
  (a test against 0), with ``c`` a value that folds, or a variable whose own domain is exact (no
  ``ABOVE``/``BELOW``). The goal's constants count as comparisons. Any other comparison is counted by kind;
* closure: from the start value and the constants, add every delta until nothing new comes. A value
  above the largest compared constant is ``ABOVE``. **Addition to the plan, counted:** a value below the
  smallest of the start value, the written constants and the compared constants is ``BELOW``: only a step
  down reaches it, and a counter stepped down has no other end. A variable
  never compared with a constant keeps its exact values when they are finite; a counter never compared
  has no bound, and is counted as not fitting;
* not fitting (no domain, ``why``): a write from a getter or an embedded opcode (var 0 is the result
  register), from another variable (a copy: counted apart, as the plan's rule leaves copies out), from an
  unbound local, a multiply or divide of itself; variables the engine writes outside the scripts (clocks,
  the mouse, the camera, the talk timer: ``ENGINE``) or as its own effect inside an opcode, where the rule
  for that variable is given (the room variables, VAR_OVERRIDE, VAR_ROOM_FLAG, VAR_WALKTO_OBJ);
* reads that are not comparisons (a copy into another variable or a local, an argument, an index) make a
  collapsed domain lossy: counted (``lossy``), the domain is kept.

**Other kinds.** ``bit:N``, ``class:O:C``, ``script:N``, ``local:N``, ``objscript:O``, ``at:<unit>:W``,
``frozen:<unit>``, ``esc:<unit>:W``, ``verb:V``: 0 and 1. ``owner:O``: the start value, setOwnerOf's
values (o5_setOwnerOf, script_v5.cpp:2722-2729) and VAR_EGO's for pickupObject (script_v5.cpp:2028).
``state:O``: the start value, setState's values, drawObject's (1, or setImage's: script_v5.cpp:1037,
1051-1053), pickupObject's 1 (script_v5.cpp:2030), and 0 for every object of a room where a drawObject
runs (it clears objects drawn at the same place, script_v5.cpp:1122-1126). A write whose object does not
fold is counted (its target is not known); one whose value does not fold leaves that object's domain
**incomplete** (``incomplete``, listed): a value outside it shows at replay as task.OutOfDomain, and the task
is built again. ``cutscene``: 0 to kMaxCutsceneNum
(script.h:111; beginCutscene errors past it, script.cpp:1632-1646). ``room``: the start, every room a
loadRoom or loadRoomWithEgo names, every room in the data when one does not fold or actorFollowCamera
changes the room (camera.cpp:69-70: the actor's room is not a fact), and the pseudo rooms. ``ego``:
VAR_EGO's values.

**Settings.** Code is read only where a run can go (``live``): from a unit's start or its object's verb
entries, along every jump, except where the run's settings decide the jump's condition (``SETTINGS``:
VAR_DEBUGMODE is 0 with no debug flag). So the debug-only branches (the boot script's, plan step 2; the
input script's actor switch, which writes VAR_EGO from a local) write nothing here. Counted.

The census counts every class above. Nothing here names a game's object, script, room or variable: the
variable numbers are ScummVM's (vars.cpp), each cited.
"""

import collections
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import ir  # noqa: E402
from ir import Assign, BinOp, Cmp, Const, Embed, Getter, Index, Jump, Local, Neg, Not, Op, Start, Truth, Var  # noqa: E402

ABOVE, BELOW = "above", "below"
MAX_CUTSCENE = 5  # kMaxCutsceneNum, script.h:111
HOOKS = range(27, 37)  # VAR_SCROLL_SCRIPT..VAR_CUTSCENE_END_SCRIPT (vars.cpp:62-71): numbers the engine starts itself
VAR_EGO, VAR_ROOM, VAR_OVERRIDE, VAR_ROOM_RESOURCE, VAR_WALKTO_OBJ, VAR_ROOM_FLAG, VAR_NEW_ROOM = 1, 4, 5, 22, 38, 70, 72
# Variables the engine writes outside the scripts, every frame or on input (vars.cpp:36-109 indices): not facts.
ENGINE = {
    2: "VAR_CAMERA_POS_X: scumm.cpp:3379 every frame", 3: "VAR_HAVE_MSG: scumm.cpp:3382 every frame",
    11: "VAR_TMR_1: a clock (facts.CLOCKS)", 12: "VAR_TMR_2: a clock", 13: "VAR_TMR_3: a clock",
    14: "VAR_MUSIC_TIMER: a clock", 46: "VAR_TIMER: a clock", 47: "VAR_TIMER_TOTAL: a clock",
    17: "VAR_CAMERA_MIN_X: room.cpp:203, the room's width", 18: "VAR_CAMERA_MAX_X: room.cpp:205, the room's width",
    20: "VAR_VIRT_MOUSE_X: scumm.cpp:3385", 21: "VAR_VIRT_MOUSE_Y: scumm.cpp:3386",
    23: "VAR_LAST_SOUND: sound.cpp:121-122, actor walk sounds every frame",
    25: "VAR_TALK_ACTOR: actor.cpp:3368, stopTalk on the message timer", 26: "VAR_CAMERA_FAST_X: input.cpp:1283",
    37: "VAR_CHARINC: input.cpp:1150-1156", 39: "VAR_DEBUGMODE: scumm.cpp:3391 every frame",
    44: "VAR_MOUSE_X: scumm.cpp:3387", 45: "VAR_MOUSE_Y: scumm.cpp:3388", 71: "VAR_GAME_LOADED: scumm.cpp:3415-3445",
}
# Variables the engine writes as its own effect inside an opcode, with the rule for their values.
ROOM_VARS = {VAR_ROOM: "room.cpp:149, 155", VAR_ROOM_RESOURCE: "room.cpp:172", VAR_NEW_ROOM: "room.cpp:76"}
BOOL_VARS = {VAR_OVERRIDE: "script.cpp:1706, beginOverride and endOverride: 0 or 1",
             VAR_ROOM_FLAG: "resource.cpp:644: 0 or 1"}
BINARY_PREFIXES = ("bit:", "class:", "script:", "local:", "objscript:", "at:", "frozen:", "esc:", "verb:")
MAX_SET = 4096  # a folded expression with more values than this is not worked out (counted: ``FOLD_CAPS``)
FOLD_CAPS = collections.Counter()  # MAX_SET hits; build() reports the ones of its run in Domains.counts
NUM_LOCALS = 25  # NUM_SCRIPT_LOCAL, script.h:76
# Engine variables the run's settings fix (plan.md Method: no debug flag): VAR_DEBUGMODE (vars.cpp:75) is written
# every frame as _debugMode ? 1 : 0 (scumm.cpp:3389-3392), and _debugMode is gDebugLevel > 0 (scumm.cpp:270).
SETTINGS = {39: 0}


@dataclass(frozen=True)
class Domain:
    values: tuple  # (BELOW?, ints ascending..., ABOVE?)
    low: int | None = None  # values below it are BELOW (when BELOW is in values)
    high: int | None = None  # values above it are ABOVE (when ABOVE is in values)

    @classmethod
    def of(cls, ints, low=None, high=None) -> "Domain":
        """The domain of ``ints`` with values outside [low, high] collapsed into BELOW and ABOVE."""
        vals = {(BELOW if low is not None and n < low else ABOVE if high is not None and n > high else n) for n in ints}
        ordered = ([BELOW] if BELOW in vals else []) + sorted(v for v in vals if isinstance(v, int)) + \
                  ([ABOVE] if ABOVE in vals else [])
        return cls(tuple(ordered), low if BELOW in vals else None, high if ABOVE in vals else None)

    @property
    def exact(self) -> bool:
        return ABOVE not in self.values and BELOW not in self.values

    def label(self, n: int):
        """The value an engine value ``n`` has here, or None when the domain does not hold it."""
        if self.high is not None and n > self.high:
            return ABOVE
        if self.low is not None and n < self.low:
            return BELOW
        return n if n in self.values else None

    def test(self, op: str, c: int) -> frozenset | None:
        """The values ``v`` with ``v op c``. None when a collapsed value cannot decide it: ``c`` above ``high``
        (or below ``low``) for ABOVE (BELOW) with any operator but the ones every collapsed value agrees on."""
        out = set()
        for v in self.values:
            if v == ABOVE:
                if c > self.high:
                    return None
                ok = CMP[op](self.high + 1, c)
            elif v == BELOW:
                if c < self.low:
                    return None
                ok = CMP[op](self.low - 1, c)
            else:
                ok = CMP[op](v, c)
            if ok:
                out.add(v)
        return frozenset(out)

    def add(self, v, d: int) -> frozenset:
        """The values ``v + d`` can have: ABOVE plus a negative step is any value in (high + d, high] or
        ABOVE still, and BELOW plus a positive step likewise; anything else is one value. A value the
        domain does not hold is left out (the closure that built the domain holds every one)."""
        if v == ABOVE:
            if d >= 0:
                return frozenset({ABOVE})
            return frozenset(x for x in self.values if x == ABOVE or (isinstance(x, int) and x > self.high + d))
        if v == BELOW:
            if d <= 0:
                return frozenset({BELOW})
            return frozenset(x for x in self.values if x == BELOW or (isinstance(x, int) and x < self.low + d))
        lab = self.label(v + d)
        return frozenset() if lab is None else frozenset({lab})


BINARY = Domain((0, 1))
CMP = {"==": lambda a, b: a == b, "!=": lambda a, b: a != b, "<": lambda a, b: a < b, "<=": lambda a, b: a <= b,
       ">": lambda a, b: a > b, ">=": lambda a, b: a >= b}
FLIP = {"==": "==", "!=": "!=", "<": ">", "<=": ">=", ">": "<", ">=": "<="}


def _div(a: int, b: int) -> int:
    q = abs(a) // abs(b)  # C division truncates toward zero (script_v5.cpp:994)
    return q if (a >= 0) == (b >= 0) else -q


def fold(e, env: dict, var: dict | None = None) -> frozenset | None:
    """The values of expression ``e``, or None when it does not fold: constants, +, -, *, / and negation over
    them, ``Local[k]`` with ``env[k]`` and ``Var[n]`` with ``var[n]`` (frozensets; absent does not fold). A set
    past MAX_SET is None."""
    match e:
        case Const(v):
            return frozenset({v})
        case Local(k):
            return env.get(k)
        case Var(n):
            return (var or {}).get(n)
        case Neg(x):
            s = fold(x, env, var)
            return None if s is None else frozenset(-v for v in s)
        case BinOp(op, a, b):
            sa, sb = fold(a, env, var), fold(b, env, var)
            if sa is None or sb is None:
                return None
            if len(sa) * len(sb) > MAX_SET:
                FOLD_CAPS["expressions past MAX_SET values (not worked out)"] += 1
                return None
            if op == "/":
                return None if 0 in sb else frozenset(_div(x, y) for x in sa for y in sb)
            f = {"+": lambda x, y: x + y, "-": lambda x, y: x - y, "*": lambda x, y: x * y}[op]
            return frozenset(f(x, y) for x in sa for y in sb)
    return None


def decide(cond, var: dict) -> bool | None:
    """A jump condition's truth from the fixed settings ``var`` ({n: frozenset}), or None when it does not fold
    to one truth value."""
    match cond:
        case Cmp(op, a, b):
            sa, sb = fold(a, {}, var), fold(b, {}, var)
            if sa is None or sb is None:
                return None
            r = {CMP[op](x, y) for x in sa for y in sb}
        case Truth(x) | Not(x):
            s = fold(x, {}, var)
            if s is None:
                return None
            r = {(v != 0) == isinstance(cond, Truth) for v in s}
        case _:
            return None
    return r.pop() if len(r) == 1 else None


def live(u, settings: dict, starts: list | None = None) -> list:
    """The instructions of unit ``u`` a run can reach: from ``starts`` (default its start: instruction 0, or an
    object's verb entries), along ``next``, ``target`` and beginOverride's Esc jump (``esc``), where a jump
    whose condition the settings decide goes one way (o5 jumps when the condition is false, ir.Jump)."""
    if starts is None:
        starts = list(u.entries.values()) if u.key[0] == "obj" else [0]
    seen, todo = set(), [i for i in starts if i < len(u.ins)]
    while todo:
        i = todo.pop()
        if i in seen or i >= len(u.ins):
            continue
        seen.add(i)
        x = u.ins[i]
        if isinstance(x.st, Jump) and x.st.cond is not None:
            d = decide(x.st.cond, settings)
            if d is not None:
                todo.append(x.next if d else x.target)
                continue
        todo += [j for j in (x.next, x.target, x.esc) if j is not None]
    return [u.ins[i] for i in sorted(seen)]


def _walk(e):
    yield e
    if isinstance(e, tuple):
        for y in e:
            yield from _walk(y)
    elif hasattr(e, "__dataclass_fields__") and not isinstance(e, type):
        for name in e.__dataclass_fields__:
            yield from _walk(getattr(e, name))


@dataclass
class Domains:
    doms: dict = field(default_factory=dict)  # fact key -> Domain (variables, owners, states, room, ego, cutscene)
    why: dict = field(default_factory=dict)  # var key -> why it has no domain
    lossy: dict = field(default_factory=dict)  # var key -> a non-comparison read of a collapsed domain
    incomplete: dict = field(default_factory=dict)  # owner/state key -> a write whose value does not fold
    counts: collections.Counter = field(default_factory=collections.Counter)
    notes: list = field(default_factory=list)  # (what, cite)

    def get(self, key: str) -> Domain | None:
        """The domain of a facts.py key; None when it is not a fact with a finite domain (``why`` says why)."""
        if key.startswith(BINARY_PREFIXES):
            return BINARY
        return self.doms.get(key)


class _Reader:
    """Call sites and their arguments, over the instructions a run can reach (``live``)."""

    def __init__(self, units: dict, code: dict, counts, notes):
        self.units, self.code = units, code
        self.first_local = min((k[2] for k in units if k[0] == "local"), default=1 << 16)
        self.local_rooms = collections.defaultdict(set)
        for k in units:
            if k[0] == "local":
                self.local_rooms[k[2]].add(k[1])
        self.counts, self.notes = counts, notes

    def targets(self, caller: tuple, n: int) -> list:
        """The units a start of script ``n`` from ``caller`` runs (script.cpp:70-86)."""
        if n < self.first_local:
            return [("global", n)] if ("global", n) in self.units else []
        if caller[0] in ("local", "entry", "exit"):
            return [("local", caller[1], n)] if ("local", caller[1], n) in self.units else []
        return [("local", r, n) for r in sorted(self.local_rooms.get(n, ()))]

    def written_locals(self, key) -> set:
        out = set()
        for x in self.code[key]:
            st = x.st
            if isinstance(st, Assign) and isinstance(st.lhs, Local):
                out.add(st.lhs.n)
            elif isinstance(st, Op) and st.name == "setVarRange" and st.args and isinstance(st.args[0], Local):
                out.update(range(st.args[0].n, st.args[0].n + len(st.args[2].items)))
            for e in _walk(st):
                if isinstance(e, Index) and e.kind == "local":
                    out.add("indexed")
        return out

    def hook_units(self) -> set:
        """Units the engine starts itself: every constant written to a hook variable (``HOOKS``). A hook write
        that does not fold is counted and listed: the units it could start are assumed to be among these."""
        out = set()
        for key in self.units:
            for x in self.code[key]:
                st = x.st
                if isinstance(st, Assign) and isinstance(st.lhs, Var) and st.lhs.n in HOOKS:
                    vals = fold(st.rhs, {}) if st.op == "=" else None
                    if vals is None:
                        self.counts["hook variable writes that do not fold (assumed to start no other unit)"] += 1
                        self.notes.append(("hook write does not fold", x.cite()))
                    for n in vals or ():
                        out.update(self.targets(key, n))
        return out

    def locals(self, engine_started: set) -> tuple[dict, dict]:
        """({unit: {k: frozenset}} for every bound local, {unit: locals the unit writes}) by a fixpoint over
        the call sites (module docstring)."""
        sites = collections.defaultdict(list)  # callee -> [(caller, args)]
        for key in self.units:
            for x in self.code[key]:
                st = x.st
                if not isinstance(st, Start):
                    continue
                if st.name == "startObject":
                    o = fold(st.obj, {}) if st.obj is not None else None
                    if o is None or len(o) != 1:
                        self.counts["starts of an object that does not fold (assumed to reach no unit whose "
                                    "locals are bound)"] += 1
                        self.notes.append(("startObject: object not a constant", x.cite()))
                        continue
                    callees = [("obj", next(iter(o)))] if ("obj", next(iter(o))) in self.units else []
                else:
                    if not isinstance(st.script, Const):
                        self.counts["starts of a script number that does not fold (assumed to reach no unit "
                                    "whose locals are bound)"] += 1
                        self.notes.append(("start: number not a constant", x.cite()))
                        continue
                    callees = self.targets(key, st.script.value)
                for c in callees:
                    sites[c].append((key, st.args))
        env = {key: {k: frozenset({0}) for k in range(NUM_LOCALS)} for key in self.units if key[0] in ("entry", "exit")}
        written = {key: self.written_locals(key) for key in self.units}
        changed = True
        while changed:
            changed = False
            for callee, calls in sites.items():
                if callee in engine_started or "indexed" in written[callee]:
                    continue
                for k in range(NUM_LOCALS):
                    if k in written[callee]:
                        continue
                    vals = set()
                    for caller, args in calls:
                        v = fold(args[k], env.get(caller, {})) if k < len(args) else frozenset({0})
                        if v is None:
                            vals = None
                            break
                        vals |= v
                    if vals is not None and env.setdefault(callee, {}).get(k) != frozenset(vals):
                        env[callee][k] = frozenset(vals)
                        changed = True
        for key in engine_started:
            env.pop(key, None)
        return env, written


def build(start: dict | None = None, goals: dict | None = None, units: dict | None = None,
          settings: dict | None = None) -> Domains:
    """Domains for every fact kind (module docstring). ``start``: the facts at the start (a facts.py map; absent
    is 0). ``goals``: {key: iterable of values}, whose values count as compared constants. ``settings``: the
    engine variables the run's settings fix ({n: value}; default ``SETTINGS``); code only a decided jump
    leads to is not read (``live``)."""
    start = start or {}
    goals = goals or {}
    units = units if units is not None else ir.load()
    fixed = SETTINGS if settings is None else settings
    settings = {n: frozenset({v}) for n, v in fixed.items()}
    out = Domains()
    caps_before = collections.Counter(FOLD_CAPS)
    code = {key: live(u, settings) for key, u in units.items()}
    out.counts["instructions"] = sum(len(u.ins) for u in units.values())
    anywhere = sum(len(live(u, {})) for u in units.values())
    out.counts["instructions no start reaches, whatever the settings"] = out.counts["instructions"] - anywhere
    out.counts["instructions left out by the settings (only a decided jump leads there)"] = \
        anywhere - sum(len(c) for c in code.values())
    rd = _Reader(units, code, out.counts, out.notes)
    engine_started = rd.hook_units()
    env, written = rd.locals(engine_started)
    out.counts["units the engine starts with its own arguments (engine-bound locals)"] = len(engine_started)

    consts = collections.defaultdict(set)  # var -> constants written
    deltas = collections.defaultdict(set)  # var -> steps written
    bad = collections.defaultdict(collections.Counter)  # var -> why a write does not fit
    copies = collections.defaultdict(set)  # var -> vars it copies
    compared = collections.defaultdict(set)  # var -> constants compared with
    cmp_vars = collections.defaultdict(set)  # var -> vars compared with (resolved after the domains)
    reads = collections.Counter()  # var -> reads that are not comparisons
    later = []  # (kind, key, x, args): owner, state and room writes, read once the variables have domains
    indexed = []

    def local_why(e, key) -> str:
        ks = {y.n for y in _walk(e) if isinstance(y, Local)}
        if any(isinstance(y, (Getter, Embed)) for y in _walk(e)):
            return "a getter"
        if any(isinstance(y, (Var, Index)) for y in _walk(e)):
            return "another variable"
        if ks & written[key]:
            return "a local its unit writes"
        if key in engine_started and ks:
            return "an engine-bound local"
        if ks:
            return "an unbound local"
        return "an expression that does not fold"

    def write(n: int, op: str, rhs, key):
        e = env.get(key, {})
        if op in ("*=", "/="):
            bad[n][f"a {'multiply' if op == '*=' else 'divide'} of itself"] += 1
            return
        if op in ("+=", "-=", "++", "--"):
            s = fold(rhs, e)
            if s is None:
                bad[n][f"a step of {local_why(rhs, key)}"] += 1
            else:
                deltas[n].update(s if op in ("+=", "++") else {-v for v in s})
            return
        s = fold(rhs, e)
        if s is not None:
            consts[n].update(s)
            return
        if isinstance(rhs, BinOp) and rhs.op in "+-" and (rhs.a == Var(n) or (rhs.op == "+" and rhs.b == Var(n))):
            d = fold(rhs.b if rhs.a == Var(n) else rhs.a, e)
            if d is not None:
                deltas[n].update(d if rhs.op == "+" else {-v for v in d})
                return
        if isinstance(rhs, Var):
            copies[n].add(rhs.n)
            bad[n]["a copy of another variable"] += 1
            return
        bad[n][f"a value from {local_why(rhs, key)}"] += 1

    def compare(c, key):
        """Record what a condition compares each variable with."""
        e = env.get(key, {})
        match c:
            case Truth(Var(n)) | Not(Var(n)):
                compared[n].add(0)
            case Cmp(_, a, b):
                for x, y in ((a, b), (b, a)):
                    if isinstance(x, Var):
                        s = fold(y, e)
                        if s is not None:
                            compared[x.n].update(s)
                        elif isinstance(y, Var):
                            cmp_vars[x.n].add(y.n)
                        else:
                            out.counts[f"comparisons of a variable with {local_why(y, key)}"] += 1
        for y in _walk(c):
            if isinstance(y, Index) and y.kind == "var":
                out.counts["comparisons on an indexed variable (left to the reader)"] += 1

    for key, u in units.items():
        for x in code[key]:
            st = x.st
            if isinstance(st, Jump):
                if st.cond is not None:
                    compare(st.cond, key)
                continue
            for y in _walk(st):
                if isinstance(y, Embed) and isinstance(y.st.lhs, Var):
                    bad[y.st.lhs.n]["a value from a getter"] += 1
            if isinstance(st, Assign):
                if isinstance(st.lhs, Var):
                    write(st.lhs.n, st.op, st.rhs, key)
                elif isinstance(st.lhs, Index) and st.lhs.kind == "var":
                    s = fold(st.lhs.offset, env.get(key, {}))
                    if s is None:
                        indexed.append((st.lhs.base, x.cite()))
                    else:
                        for v in s:
                            write(st.lhs.base + v, st.op, st.rhs, key)
                self_step = isinstance(st.lhs, Var) and isinstance(st.rhs, BinOp) and Var(st.lhs.n) in (st.rhs.a, st.rhs.b)
                for y in _walk((st.rhs, st.lhs.offset if isinstance(st.lhs, Index) else None)):
                    if isinstance(y, Var) and not (self_step and y.n == st.lhs.n):
                        reads[y.n] += 1
                continue
            for y in _walk(st):
                if isinstance(y, Var):
                    reads[y.n] += 1
            if isinstance(st, Op):
                a = st.args
                if st.name == "setVarRange" and a and isinstance(a[0], Var):
                    for k, v in enumerate(a[2].items):
                        write(a[0].n + k, "=", v, key)
                elif st.name in ("setOwnerOf", "pickupObject", "setState", "drawObject", "loadRoom",
                                 "loadRoomWithEgo", "actorFollowCamera"):
                    later.append((st.name, key, x))
    for n, vals in goals_by_var(goals).items():
        compared[n].update(vals)

    # variables: the plan's rule, then comparisons with variables whose domains are exact, until nothing changes
    var_nums = set(consts) | set(deltas) | set(bad) | set(compared) | set(cmp_vars)
    var_nums |= {int(k[4:]) for k in start if k.startswith("var:")}
    special = set(ROOM_VARS) | set(BOOL_VARS) | {VAR_WALKTO_OBJ}
    for n in sorted(var_nums - special):
        if n in ENGINE:
            out.why[f"var:{n}"] = f"written by the engine: {ENGINE[n]}"
        elif bad.get(n):
            out.why[f"var:{n}"] = bad[n].most_common(1)[0][0]
    for n in fixed:
        out.why[f"var:{n}"] = f"fixed by the settings (value {fixed[n]}), not a fact"
    rounds, counters = 0, set()
    while True:  # each round can make a variable exact that another one is compared with
        rounds += 1
        before, counters = dict(out.doms), set()
        for n in sorted(var_nums - special):
            key = f"var:{n}"
            if key in out.why:
                continue
            cmp = set(compared[n])
            for m in cmp_vars.get(n, ()):
                d = out.doms.get(f"var:{m}")
                if d is not None and d.exact:
                    cmp.update(d.values)
            dom = _closure(start.get(key, 0), consts[n], deltas[n], cmp)
            if dom is None:
                counters.add(key)
                out.doms.pop(key, None)
            else:
                out.doms[key] = dom
        if out.doms == before:
            break
    out.why |= {key: "a counter never compared with a constant" for key in counters}
    out.counts["rounds of the comparison fixpoint"] = rounds
    exact = {int(k[4:]): frozenset(d.values) for k, d in out.doms.items() if k.startswith("var:") and d.exact}
    for n in sorted(var_nums):
        key = f"var:{n}"
        if key in out.doms:
            if any(f"var:{m}" not in out.doms or not out.doms[f"var:{m}"].exact for m in cmp_vars.get(n, ())):
                out.counts["finite variables compared with a variable whose domain is not exact"] += 1
            if not out.doms[key].exact and reads[n]:
                out.lossy[key] = reads[n]
            if BELOW in out.doms[key].values:
                out.counts["finite variables that need BELOW (the addition)"] += 1
        elif copies.get(n) and set(bad[n]) == {"a copy of another variable"} and \
                all(f"var:{m}" in out.doms for m in copies[n]):
            out.counts["variables left out only for copies of finite variables"] += 1

    # owners, states, rooms: the writes, with variables of exact domains folded
    meta = json.loads((ir.SCRIPTS / "index.json").read_text())
    obj_room = {int(o): v["room"] for o, v in meta["objects"].items()}
    owner_vals, state_vals = collections.defaultdict(set), collections.defaultdict(set)
    owner_ego, draw_rooms, room_vals, room_wild, walkto = set(), set(), set(), [], set()
    ego = out.doms.get(f"var:{VAR_EGO}")
    for name, key, x in later:
        st, e = x.st, env.get(key, {})
        a = st.args

        def f(i):
            return fold(a[i], e, exact) if i < len(a) else None
        if name == "setOwnerOf":
            objs, vals = f(0), f(1)
            if objs is None:
                out.counts["owner writes whose object does not fold (target not known)"] += 1
                out.notes.append(("setOwnerOf: object does not fold", x.cite()))
                continue
            if vals is None:
                out.counts["owner writes whose value does not fold (domain incomplete)"] += 1
                out.incomplete.update({f"owner:{o}": x.cite() for o in objs})
                continue
            for o in objs:
                owner_vals[o] |= vals
        elif name == "pickupObject":
            objs = f(0)
            if objs is None:
                out.counts["owner writes whose object does not fold (target not known)"] += 1
                out.notes.append(("pickupObject: object does not fold", x.cite()))
                continue
            owner_ego |= objs
            for o in objs:
                state_vals[o].add(1)
        elif name in ("setState", "drawObject"):
            objs = f(0)
            if name == "setState":
                vals = f(1)
            else:
                img = [s for s in st.subops if s.name == "setImage"]
                vals = fold(img[0].args[0], e, exact) if img else frozenset({1})
                draw_rooms.add(units[key].room)
            if objs is None:
                out.counts["state writes whose object does not fold (target not known)"] += 1
                out.notes.append((f"{name}: object does not fold", x.cite()))
                continue
            if vals is None:
                out.counts["state writes whose value does not fold (domain incomplete)"] += 1
                out.incomplete.update({f"state:{o}": x.cite() for o in objs})
                continue
            for o in objs:
                state_vals[o] |= vals
        elif name in ("loadRoom", "loadRoomWithEgo"):
            r = f(0 if name == "loadRoom" else 1)
            if r is None:
                room_wild.append(x.cite())
            else:
                room_vals |= r
            if name == "loadRoomWithEgo":
                walkto |= f(0) or set()
        else:
            room_wild.append(x.cite())
    rooms = {int(r) for r in meta["rooms"]}
    pseudo = {a.value | 0x80 for u in units.values() for x in u.ins
              if isinstance(x.st, Op) and x.st.name == "PseudoRoom" for a in x.st.args[1:] if isinstance(a, Const)}
    room_all = room_vals | {start.get("room", 0)}
    if room_wild:
        room_all |= rooms | pseudo
        out.counts["room changes to a room that does not fold (room widened to every room)"] = len(room_wild)
        out.notes += [("room change does not fold", c) for c in room_wild]
    out.doms["room"] = Domain.of(room_all)
    for n in ROOM_VARS:
        out.doms[f"var:{n}"] = Domain.of(room_all | {start.get(f"var:{n}", 0)} | consts.get(n, set()))
    for n in BOOL_VARS:
        out.doms[f"var:{n}"] = Domain.of({0, 1, start.get(f"var:{n}", 0)} | consts.get(n, set()))
    out.doms[f"var:{VAR_WALKTO_OBJ}"] = Domain.of({0, start.get(f"var:{VAR_WALKTO_OBJ}", 0)} | walkto)
    if ego is not None and ego.exact:
        out.doms["ego"] = ego
    elif owner_ego:
        out.counts["pickupObject owners: VAR_EGO has no exact domain"] += len(owner_ego)
    for o in sorted(set(owner_vals) | owner_ego | {int(k[6:]) for k in start if k.startswith("owner:")}):
        vals = owner_vals.get(o, set()) | {start.get(f"owner:{o}", 0)}
        if o in owner_ego and ego is not None and ego.exact:
            vals |= set(ego.values)
        out.doms[f"owner:{o}"] = Domain.of(vals)
    for o in sorted(set(state_vals) | {int(k[6:]) for k in start if k.startswith("state:")} |
                    {o for o, r in obj_room.items() if r in draw_rooms}):
        vals = state_vals.get(o, set()) | {start.get(f"state:{o}", 0)}
        if obj_room.get(o) in draw_rooms:
            vals.add(0)  # drawObject clears objects drawn at the same place (script_v5.cpp:1122-1126)
        out.doms[f"state:{o}"] = Domain.of(vals)
    out.doms["cutscene"] = Domain.of(range(MAX_CUTSCENE + 1))

    out.counts.update(FOLD_CAPS - caps_before)
    out.counts["indexed variable writes whose index does not fold (targets not worked out)"] = len(indexed)
    out.notes += [(f"indexed write Var[{b} + ...]", c) for b, c in indexed]
    out.counts["finite variables within 16 above the base of such a write (could be hit)"] = len(
        {n for b, _ in indexed for n in range(b, b + 16) if f"var:{n}" in out.doms})
    return out


def goals_by_var(goals: dict) -> dict:
    return {int(k[4:]): set(v) for k, v in goals.items() if k.startswith("var:")}


def _closure(s: int, consts: set, deltas: set, compared: set) -> Domain | None:
    """The start value closed under the writes, collapsed past the compared constants (module docstring).
    None for a counter (some delta) never compared with a constant."""
    seeds = {s} | set(consts)
    low = min(seeds | set(compared))
    if not deltas:
        return Domain.of(seeds, low, max(compared)) if compared else Domain.of(seeds)
    if not compared:
        return None
    high = max(compared)
    dom_vals, todo = set(), list(seeds)
    seen = set()
    while todo:
        v = todo.pop()
        lab = BELOW if v < low else ABOVE if v > high else v
        if lab in seen:
            continue
        seen.add(lab)
        if lab == ABOVE:
            nexts = [high + 1 + d for d in deltas if d < 0] + [high + 1 + d for d in deltas if d >= 0]
            # ABOVE minus a step can land anywhere in (high + d, high]: every such value
            nexts += [w for d in deltas if d < 0 for w in range(high + d + 1, high + 1)]
        elif lab == BELOW:
            nexts = [low - 1 + d for d in deltas] + [w for d in deltas if d > 0 for w in range(low, low + d)]
        else:
            nexts = [v + d for d in deltas]
        todo.extend(nexts)
        dom_vals.add(v if isinstance(lab, int) else (high + 1 if lab == ABOVE else low - 1))
    return Domain.of(dom_vals, low, high)


def report(doms: Domains) -> list[str]:
    """The census: domains per kind, sizes, what does not fit and why."""
    kinds = collections.Counter(k.split(":")[0] for k in doms.doms)
    sizes = sorted(len(d.values) for k, d in doms.doms.items() if k.startswith("var:"))
    why = collections.Counter(doms.why.values())
    lines = ["domains (plan step 1, Facts): what has a finite domain, and what does not", ""]
    lines += [f"  {k:10} {n:6}" for k, n in sorted(kinds.items())]
    lines += ["  binary kinds (bit, class, script, local, objscript, at, frozen, esc, verb): 0 and 1, any number", ""]
    n_var = len(sizes) + len(doms.why)
    lines += [f"variables seen in the IR or the start: {n_var}", f"  with a finite domain: {len(sizes)}",
              f"    of them collapsed (ABOVE or BELOW): {sum(1 for k, d in doms.doms.items() if k.startswith('var:') and not d.exact)}",
              f"    of them lossy (collapsed, and read other than by a comparison): {len(doms.lossy)}"]
    if sizes:
        lines += [f"    values per variable: median {sizes[len(sizes) // 2]}, max {sizes[-1]}, total {sum(sizes)}",
                  f"    over 256 values: {sum(1 for s in sizes if s > 256)}"]
    lines += [f"  with no domain (not facts, checked at replay): {len(doms.why)}"]
    lines += [f"    {n:5}  {w}" for w, n in why.most_common()]
    lines += ["", "counts:"] + [f"  {n:6}  {w}" for w, n in sorted(doms.counts.items())]
    owners = [d for k, d in doms.doms.items() if k.startswith("owner:")]
    states = [d for k, d in doms.doms.items() if k.startswith("state:")]
    if owners:
        lines += ["", f"owners: {len(owners)} objects, values per object max {max(len(d.values) for d in owners)}"]
    if states:
        lines += [f"states: {len(states)} objects, values per object max {max(len(d.values) for d in states)}"]
    lines += [f"room: {len(doms.doms['room'].values)} values"]
    lines += [f"owner and state domains incomplete (a write's value does not fold): {len(doms.incomplete)}"]
    lines += [f"  {k}: {c}" for k, c in sorted(doms.incomplete.items())] + [""]
    lines += ["notes (sites):"] + [f"  {w}: {c}" for w, c in doms.notes[:60]]
    if len(doms.notes) > 60:
        lines.append(f"  ... {len(doms.notes) - 60} more (all counted above)")
    return lines


def main() -> None:
    doms = build({})
    lines = report(doms)
    lines.insert(1, "start values: the engine's init (every var 0: absent is 0); plan step 2's start is not built yet")
    (HERE / "results" / "task-domains.log").write_text("\n".join(lines) + "\n")
    print("\n".join(lines[:60]))


if __name__ == "__main__":
    main()
