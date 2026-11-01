"""E004 plan step B1: the library of trace validation (trace-validation.md): the engine's segmentation, the IR side of
the explanation (reachability, write sites, the first opcode off a path, the backward slice, the two classes), Lemma L's
bisection over forks, and the strict matcher.

usage: from trace_explain import Code, Runs, Explained, locate, StrictMatcher
       code = Code(model.Game.load())
       runs = Runs(branch.runs()["runs"])                    # the bridge's segment buffer
       ex = Explained(branch.explain_log()["explain"])      # the bridge's explain log

It names no game number. Every unit, offset and variable it reports comes from the engine's log and the IR.
"""

import collections
import dataclasses
import time
from dataclasses import dataclass, field

import facts as F
import ir
import model as M

WIO_INVENTORY, WIO_ROOM, WIO_GLOBAL, WIO_LOCAL, WIO_FLOBJECT = F.WIO_INVENTORY, F.WIO_ROOM, F.WIO_GLOBAL, F.WIO_LOCAL, \
    F.WIO_FLOBJECT
LOCAL_BIT, BIT_BIT = 0x4000, 0x8000  # readVar's local and bit variables (script.cpp:600-706)
RUN_FIELDS = ("frame", "after", "seq", "depth", "slot", "where", "number", "room", "code0", "code1", "status", "frozen",
              "delay", "ops")
SS_DEAD, SS_PAUSED, SS_RUNNING = 0, 1, 2  # script.h:81-83
SLOT_KEYS = ("at:", "frozen:", "local:", "script:", "objscript:", "esc:")
# Variables opcodes write besides their own operands (check.py IN_OPCODES and LEFT_IN): a room change's startScene
# (VAR_ROOM, VAR_CAMERA_MIN_X and _MAX_X, VAR_ROOM_RESOURCE, VAR_ROOM_FLAG, VAR_NEW_ROOM; room.cpp:76-205), and
# o5_cursorCommand's VAR_CURSORSTATE and VAR_USERPUT (script_v5.cpp:935-938; reader.CURSOR_OPS).
ROOM_VARS = (M.VAR_ROOM, 17, 18, M.VAR_ROOM_RESOURCE, 70, 72)
CURSOR_VARS = {"UserputOn": M.VAR_USERPUT, "UserputOff": M.VAR_USERPUT, "UserputSoftOn": M.VAR_USERPUT,
               "UserputSoftOff": M.VAR_USERPUT, "CursorShow": M.VAR_CURSORSTATE, "CursorHide": M.VAR_CURSORSTATE,
               "CursorSoftOn": M.VAR_CURSORSTATE, "CursorSoftOff": M.VAR_CURSORSTATE}


# --- the engine's segmentation (section 1) -------------------------------------------------------------------------

@dataclass(frozen=True)
class Run:
    frame: int
    after: int
    seq: int
    depth: int
    slot: int
    where: int
    number: int
    room: int
    code0: int
    code1: int
    status: int
    frozen: int
    delay: int
    ops: int

    @property
    def unit(self) -> str | None:
        return F.unit_of(self.where, self.number)


class Runs:
    """The bridge's run records, in the order they began (``seq``)."""

    def __init__(self, rows=()):
        self.runs: list[Run] = []
        self.add(rows)

    def add(self, rows) -> None:
        self.runs.extend(Run(*r) for r in rows)
        self.runs.sort(key=lambda r: r.seq)

    def of_unit(self, where: int, number: int) -> list:
        return [r for r in self.runs if r.where == where and r.number == number]

    def between(self, k: int) -> list:
        """The runs between D_{k-1} and D_k: frame k-1 after its decision point, frame k before it."""
        return [r for r in self.runs if (r.frame == k - 1 and r.after) or (r.frame == k and not r.after)]


@dataclass
class Op:
    code: int
    opcode: int
    reads: list = field(default_factory=list)  # (var, value), in order
    writes: list = field(default_factory=list)


@dataclass
class Logged:
    """One explained run: its begin entry and its opcodes with their reads and writes."""
    seq: int
    frame: int
    after: int
    slot: int
    where: int
    number: int
    room: int
    code0: int
    ops: list = field(default_factory=list)
    code1: int | None = None
    status: int | None = None
    frozen: int | None = None
    delay: int | None = None


class Explained:
    """The bridge's explain log, grouped by run."""

    def __init__(self, entries=()):
        self.runs: dict[int, Logged] = {}
        self.add(entries)

    def add(self, entries) -> None:
        for e in entries:
            kind, seq = e[0], e[1]
            if kind == "b":
                self.runs[seq] = Logged(seq, *e[2:9])
                continue
            r = self.runs.get(seq)
            if r is None:
                continue
            if kind == "o":
                r.ops.append(Op(e[2], e[3]))
            elif kind in ("r", "w"):
                if not r.ops:  # a read before the first opcode cannot happen; kept on a pseudo-op
                    r.ops.append(Op(-1, -1))
                (r.ops[-1].reads if kind == "r" else r.ops[-1].writes).append((e[2], e[3]))
            elif kind == "e":
                r.code1, r.status, r.frozen, r.delay = e[2:6]

    def of_unit(self, where: int, number: int) -> list:
        return [r for _, r in sorted(self.runs.items()) if r.where == where and r.number == number]


# --- the IR side (section 5) ------------------------------------------------------------------------------------------

def var_id(e) -> int | None:
    """The readVar/writeVar number of an IR variable, local or bit."""
    if isinstance(e, ir.Var):
        return e.n
    if isinstance(e, ir.Local):
        return LOCAL_BIT | e.n
    if isinstance(e, ir.Bit):
        return BIT_BIT | e.n
    return None


def show_var(n: int) -> str:
    if n & BIT_BIT:
        return f"bit {n & 0x7FFF}"
    if n & LOCAL_BIT:
        return f"local {n & 0xFFF}"
    return f"var {n}"


def leaves(e) -> list:
    """The variables, locals, bits and getters an expression or condition reads."""
    out = []
    for y in M._walk(e):
        if isinstance(y, (ir.Var, ir.Local, ir.Bit, ir.Getter, ir.Index)):
            out.append(y)
    return out


class Code:
    """The game's units with the engine's view of them: a slot's (where, number, room resource) to its unit, a code
    offset to an instruction, the control flow, the write sites of a fact and the starts of a unit."""

    def __init__(self, game: M.Game):
        self.game = game
        self._reach: dict = {}
        self.nonfacts = M.non_fact_vars(game)

    def key(self, where: int, number: int, room: int) -> tuple | None:
        if where == WIO_GLOBAL:
            k = ("global", number)
        elif where == WIO_LOCAL:
            k = ("local", room, number)
        elif where == WIO_ROOM and number in (F.ENCD, F.EXCD):
            k = ("entry" if number == F.ENCD else "exit", room)
        elif where in (WIO_INVENTORY, WIO_ROOM, WIO_FLOBJECT):
            k = ("obj", number)
        else:
            return None
        return k if k in self.game.units else None

    def key_of_name(self, name: str, room: int) -> tuple | None:
        """A unit as facts and the model name it (script:N, local:N, obj:O, entry, exit) in room resource ``room``."""
        kind, _, n = name.partition(":")
        k = {"script": lambda: ("global", int(n)), "local": lambda: ("local", room, int(n)),
             "obj": lambda: ("obj", int(n)), "entry": lambda: ("entry", room), "exit": lambda: ("exit", room)}
        f = k.get(kind)
        if f is None:
            return None
        key = f()
        return key if key in self.game.units else None

    def unit(self, key):
        return self.game.units[key]

    def index(self, key, code: int) -> int | None:
        return self.unit(key).at.get(code)

    def succ(self, key, i: int, cross_yields: bool) -> list:
        """Where control can go after instruction i (len(ins): the script's end). A yield stops a run; with
        ``cross_yields`` the slot's later runs go on from where it resumes."""
        u = self.unit(key)
        x = u.ins[i]
        if x.yields in ("frame", "delay"):
            return [x.next] if cross_yields and x.next is not None else []
        out = []
        if x.yields == "wait":
            out.append(i) if cross_yields else None
        if isinstance(x.st, ir.Jump) and x.st.cond is None:
            return [x.target] if x.target is not None else []
        if x.ends == "always":
            return []
        if x.next is not None:
            out.append(x.next)
        if x.target is not None:
            out.append(x.target)
        return out

    def reachable(self, key, start: int | None, targets: frozenset, cross_yields: bool = True) -> bool:
        """Some target is reachable from instruction ``start`` (inclusive) along the IR's control flow."""
        if start is None or not targets:
            return False
        n = len(self.unit(key).ins)
        memo = self._reach.setdefault((key, targets, cross_yields), {})
        if start in memo:
            return memo[start]
        seen, todo = {start}, [start]
        found = False
        while todo:
            i = todo.pop()
            if i in targets:
                found = True
                break
            if i >= n:
                continue
            for j in self.succ(key, i, cross_yields):
                if j is not None and j not in seen:
                    seen.add(j)
                    todo.append(j)
        memo[start] = found
        return found

    # what writes a fact

    def write_sites(self, key, fact: str, value=None, direct: bool = False) -> frozenset:
        """The instructions of unit ``key`` whose statement can write ``fact`` (to ``value`` when it is a constant
        there): trace-validation.md section 5's write sites. ``direct``: leave out writes through an index and ops
        whose object is not a constant."""
        u = self.unit(key)
        kind, _, rest = fact.partition(":")
        out = set()
        for i, x in enumerate(u.ins):
            st = x.st
            if st is None:
                continue
            if kind in ("var", "bit"):
                n = int(rest)
                want = ir.Var(n) if kind == "var" else ir.Bit(n)
                for y in M._walk(st):
                    if isinstance(y, ir.Embed) and y.st.lhs == want:
                        out.add(i)
                if isinstance(st, ir.Assign) and (st.lhs == want or (not direct and isinstance(st.lhs, ir.Index) and
                                                                     st.lhs.kind == kind)):
                    if value is None or st.op != "=" or not isinstance(st.rhs, ir.Const) or st.rhs.value == value:
                        out.add(i)
                elif isinstance(st, ir.Op) and st.name == "setVarRange" and isinstance(st.args[0], type(want)):
                    if st.args[0].n <= n < st.args[0].n + len(st.args[2].items):
                        out.add(i)
                elif kind == "var" and isinstance(st, ir.Op) and n in ROOM_VARS and st.name in ir.ROOM_CHANGE:
                    out.add(i)
                elif kind == "var" and isinstance(st, ir.Op) and CURSOR_VARS.get(st.name) == n:
                    out.add(i)
            elif kind in ("state", "owner", "class"):
                o = int(rest.split(":")[0])
                names = {"state": ("setState", "drawObject", "pickupObject"), "owner": ("setOwnerOf", "pickupObject"),
                         "class": ("setClass", "pickupObject")}[kind]
                if isinstance(st, ir.Op) and st.name in names and st.args and \
                        ((not direct and not isinstance(st.args[0], ir.Const)) or
                         (isinstance(st.args[0], ir.Const) and st.args[0].value == o)):
                    out.add(i)
            elif kind == "verb":
                if isinstance(st, ir.Op) and st.name in ("VerbOps", "saveVerbs", "restoreVerbs") and \
                        (not st.args or not isinstance(st.args[0], ir.Const) or st.args[0].value == int(rest)):
                    out.add(i)
            elif kind == "room":
                if isinstance(st, ir.Op) and st.name in ir.ROOM_CHANGE:
                    out.add(i)
            elif kind == "cutscene":
                if isinstance(st, ir.Op) and st.name in ("cutscene", "endCutscene"):
                    out.add(i)
        return frozenset(out)

    def start_sites(self, key, target: tuple, direct: bool = True) -> frozenset:
        """The instructions of unit ``key`` that can start unit ``target`` (a start of its number, or a doSentence
        for the sentence script, which checkAndRunSentenceScript then starts). ``direct``: only starts with a
        constant number or object; else only those without."""
        u = self.unit(key)
        out = set()
        for i, x in enumerate(u.ins):
            st = x.st
            if isinstance(st, ir.Start):
                n = st.script.value if isinstance(st.script, ir.Const) else None
                if st.name == "startObject":
                    const = isinstance(st.obj, ir.Const)
                    if target[0] == "obj" and const == direct and (not const or st.obj.value == target[1]):
                        out.add(i)
                elif target[0] in ("global", "local") and (n is not None) == direct and (n is None or n == target[-1]):
                    out.add(i)
            elif isinstance(st, ir.Op) and st.name == "doSentence" and "STOP" not in st.flags and \
                    target == ("sentence",) and direct:
                out.add(i)
        return frozenset(out)

    def yield_sites(self, key, w: int) -> frozenset:
        """The instructions after which a slot of ``key`` waits at code offset ``w``."""
        u = self.unit(key)
        i = u.at.get(w)
        out = set()
        for j, x in enumerate(u.ins):
            if x.yields in ("frame", "delay") and x.next == i:
                out.add(j)
            elif x.yields == "wait" and j == i:
                out.add(j)
            elif isinstance(x.st, ir.Start) and x.next == i:  # a nested call that froze the caller
                out.add(j)
        return frozenset(out)

    def end_sites(self, key) -> frozenset:
        """Where a slot of ``key`` surely ends: the script's end, an op that always ends it, a room change (which
        kills a room's slots, ir.py ``ends``). An op that ends it only when it names itself is left out."""
        u = self.unit(key)
        out = {len(u.ins)}
        out.update(j for j, x in enumerate(u.ins) if x.ends in ("always", "room"))
        return frozenset(out)


# --- the first opcode off the path and the slice (Lemmas X and B) -------------------------------------------------

@dataclass
class Step:
    """One executed opcode of a unit, placed in the IR."""
    run: Logged
    k: int  # its position in run.ops
    i: int | None  # its instruction index
    op: Op


def steps_of(code: Code, key, runs: list) -> list:
    out = []
    for r in runs:
        for k, op in enumerate(r.ops):
            out.append(Step(r, k, code.index(key, op.code) if op.code >= 0 else None, op))
    return out


def taken(code: Code, key, steps: list, n: int) -> tuple | None:
    """For the conditional jump at steps[n]: (the side's start the engine took, the other side's), or None when the
    next step does not tell (the run ended or the next opcode is of a later run)."""
    s = steps[n]
    x = code.unit(key).ins[s.i]
    nxt = steps[n + 1] if n + 1 < len(steps) and steps[n + 1].run is s.run else None
    if nxt is None:
        # the run ended at the jump: it went to the script's end (or yielded, which a jump never does)
        if x.target == len(code.unit(key).ins):
            return x.target, x.next
        return None
    if nxt.i == x.target and nxt.i != x.next:
        return x.target, x.next
    if nxt.i == x.next and nxt.i != x.target:
        return x.next, x.target
    return None


def first_off(code: Code, key, steps: list, targets: frozenset) -> int | None:
    """Lemma X: the index in ``steps`` of the first conditional jump whose taken side cannot reach ``targets`` while
    its other side can."""
    u = code.unit(key)
    for n, s in enumerate(steps):
        if s.i is None:
            continue
        x = u.ins[s.i]
        if not (isinstance(x.st, ir.Jump) and x.st.cond is not None):
            continue
        sides = taken(code, key, steps, n)
        if sides is None:
            continue
        went, other = sides
        if not code.reachable(key, went, targets) and code.reachable(key, other, targets):
            return n
    return None


def last_test(code: Code, key, steps: list) -> int | None:
    """The last conditional jump the steps executed (a polling loop's test, when the slot waits at a breakHere)."""
    u = code.unit(key)
    for n in range(len(steps) - 1, -1, -1):
        s = steps[n]
        if s.i is not None and isinstance(u.ins[s.i].st, ir.Jump) and u.ins[s.i].st.cond is not None:
            return n
    return None


@dataclass
class Value:
    """One engine value a test read, with where it came from (the slice)."""
    what: str  # "var 32", "local 3", "bit 70", or a getter "getActorX(6)"
    value: object
    source: str = ""  # for a local or variable: the getter or variable it was last written from, where
    kind: str = "fact"  # fact, or the model's reason it is not one (position, random draw, ...), or "unknown"

    def as_dict(self) -> dict:
        return dataclasses.asdict(self)


def getter_kind(name: str) -> str:
    return M.GETTERS.get(name, ("unknown", ""))[0]


def slice_test(code: Code, key, steps: list, n: int, dump: dict | None) -> list:
    """Lemma B: the values the test at steps[n] read (for an assignment, its right side), each followed back over
    the logged steps of its unit."""
    u = code.unit(key)
    s = steps[n]
    x = u.ins[s.i]
    out = []
    reads = list(s.op.reads)
    expr = x.st.cond if isinstance(x.st, ir.Jump) else x.st.rhs if isinstance(x.st, ir.Assign) else x.st
    for leaf in leaves(expr):
        if isinstance(leaf, ir.Getter):
            out.append(getter_value(leaf, dump))
            continue
        vid = var_id(leaf)
        if vid is None:
            out.append(Value(ir.show(leaf), None, kind="unknown"))
            continue
        val = next((v for var, v in reads if var == vid), None)
        out.append(trace_back(code, key, steps, n, vid, val, dump))
    return out


def trace_back(code: Code, key, steps: list, n: int, vid: int, val, dump) -> Value:
    """The last logged write of ``vid`` before steps[n] in this unit's runs, and the statement that made it."""
    u = code.unit(key)
    what = show_var(vid)
    for m in range(n - 1, -1, -1):
        s = steps[m]
        if any(var == vid for var, _ in s.op.writes) and s.i is not None:
            st = u.ins[s.i].st
            written = next(v for var, v in s.op.writes if var == vid)
            rhs = st.rhs if isinstance(st, ir.Assign) else None
            if isinstance(rhs, ir.Getter):
                g = getter_value(rhs, dump)
                return Value(what, val if val is not None else written,
                             f"{rhs.name} at [{u.ins[s.i].offset:04X}] wrote {written}; the dump: {g.value}",
                             getter_kind(rhs.name))
            if rhs is not None:
                src = [leaf for leaf in leaves(rhs)]
                kinds = [kind_of_leaf(code, leaf) for leaf in src]
                kind = next((k for k in kinds if k != "fact"), "fact")
                return Value(what, val if val is not None else written,
                             f"[{u.ins[s.i].offset:04X}] wrote {written} from {', '.join(map(ir.show, src)) or 'a constant'}",
                             kind)
            return Value(what, val, f"[{u.ins[s.i].offset:04X}] wrote {written}", "fact")
    # not written in the logged runs: a variable is the oracle's; a local was set before (its start's arguments or an
    # earlier run); the IR's writers of it in this unit name what it can hold
    if vid & LOCAL_BIT and not vid & BIT_BIT:
        n_loc = vid & 0xFFF
        rhs = {ir.show(x.st.rhs) for x in u.ins if isinstance(x.st, ir.Assign) and x.st.lhs == ir.Local(n_loc)}
        gets = [x.st.rhs.name for x in u.ins if isinstance(x.st, ir.Assign) and x.st.lhs == ir.Local(n_loc)
                and isinstance(x.st.rhs, ir.Getter)]
        kind = next((getter_kind(g) for g in gets if getter_kind(g) != "fact"), "unknown" if not rhs else "fact")
        return Value(what, val, f"set before these runs; its writers in the unit: {len(rhs)}", kind)
    return Value(what, val, "", kind_of_var(code, vid))


def kind_of_var(code: Code, vid: int) -> str:
    if vid & BIT_BIT:
        return "fact"
    if vid & LOCAL_BIT:
        return "unknown"
    if vid in F.CLOCKS:
        return "clock"
    return code.nonfacts.get(vid, "fact")


def kind_of_leaf(code: Code, leaf) -> str:
    if isinstance(leaf, ir.Getter):
        return getter_kind(leaf.name)
    vid = var_id(leaf)
    return kind_of_var(code, vid) if vid is not None else "unknown"


def getter_value(g, dump: dict | None) -> Value:
    """A getter's engine value from the state dump at the decision point before the run (DEBUG_VARS does not show
    it): an actor's x, y, elevation or moving state, an object's state, owner or class; else not in the dump."""
    args = [a.value if isinstance(a, ir.Const) else None for a in g.args]
    what = f"{g.name}({', '.join('?' if a is None else str(a) for a in args)})"
    val = None
    if dump is not None and args and args[0] is not None:
        a0 = args[0]
        actor = next((x for x in dump.get("actors", []) if x["id"] == a0), None)
        field_ = {"getActorX": "x", "getActorY": "y", "getActorElevation": "elevation",
                  "getActorMoving": "moving"}.get(g.name)
        if field_ and actor is not None:
            val = actor[field_]
        elif g.name == "getObjectState":
            val = dump.get("states", {}).get(str(a0))
        elif g.name == "getObjectOwner":
            val = dump.get("owners", {}).get(str(a0))
        elif g.name == "classOfIs":
            val = dump.get("classes", {}).get(str(a0))
    return Value(what, val, "the state dump at the decision point before" if val is not None else "not in the dump",
                 getter_kind(g.name))


def wait_values(x, slot_delay: int, dump: dict | None) -> list:
    """What a slot parked at instruction ``x`` waits on: a delay's jiffies left, one frame after a breakHere, a
    WaitFor op's condition from the dump (an actor's moving state, VAR_HAVE_MSG for a message)."""
    name = x.st.name if isinstance(x.st, ir.Op) else ir.head(x.st)
    if x.yields == "delay":
        return [Value("delay", slot_delay, "the slot's delay at its last run (jiffies)", "time")]
    if x.yields == "frame":
        return [Value("frames to its next run", 1, "a breakHere yields one frame", "time")]
    if name == "WaitForActor":
        return [getter_value(ir.Getter("getActorMoving", x.st.args), dump)]
    if name == "WaitForMessage":
        have = dump["vars"][3] if dump is not None else None  # VAR_HAVE_MSG (vars.cpp:40)
        return [Value("var 3", have, "VAR_HAVE_MSG at the decision point before: a message still shows", "time")]
    return [Value(name, None, "a wait the dump does not show", "time")]


def cause_class(values: list) -> str:
    """trace-validation.md section 5: split when a value the test read is not a fact of the reading, else
    semantics."""
    return "split" if any(v.kind != "fact" for v in values) else "semantics"


# --- Lemma L: bisection over forks ------------------------------------------------------------------------------------

def locate(fork, branch, lo: int, hi: int, pred, stats: dict | None = None) -> int:
    """The k in (lo, hi] with pred(D_{k-1}) true and pred(D_k) false, given pred(D_lo) true and pred(D_hi) false.
    ``branch`` stands at lo and is kept; ``pred(child)`` evaluates on a child stopped at a frame. Each step forks once."""
    base, own = branch, False
    while hi - lo > 1:
        mid = (lo + hi) // 2
        child = fork(base)
        st = child.run(frame=mid)
        if "end" in st or child.status.get("frame") != mid:
            child.quit()
            raise RuntimeError(f"bisection: the child did not stop at frame {mid}: {st}")
        if stats is not None:
            stats["forks"] = stats.get("forks", 0) + 1
            stats["frames"] = stats.get("frames", 0) + (mid - lo)
        if pred(child):
            if own:
                base.quit()
            base, own, lo = child, True, mid
        else:
            child.quit()
            hi = mid
    if own:
        base.quit()
    return hi


# --- the strict matcher (section 3) -----------------------------------------------------------------------------------

@dataclass
class Verdict:
    run: Run
    status: str  # explained, ambiguous, unexplained, unplaced (no event record of its unit at its wait point)
    record: object = None  # the matched record (its index among the candidates)
    why: list = field(default_factory=list)  # per candidate: the first condition that failed


class StrictMatcher:
    """Definition M over a set of event records. ``records``: (index, unit name, wait point, pre, effects, checks)
    with pre {key: set of values}, effects {key: ("set", v) | ("guess", values) | ...}. The view's keys are the
    reading's (``replay.view``): ``pos:U`` is U's wait point (NOWHERE when not live)."""

    def __init__(self, records, nowhere, decide_check=None):
        self.by_point = collections.defaultdict(list)
        for rec in records:
            self.by_point[(rec[1], rec[2])].append(rec)
        self.nowhere = nowhere
        self.decide_check = decide_check  # (check, run) -> True, False or None (open)
        self.calls = 0
        self.seconds = 0.0

    @staticmethod
    def _value_ok(eff, got) -> bool:
        if eff[0] == "set":
            return got == eff[1]
        if eff[0] == "guess":
            return got in eff[1]
        return True  # a step or a copy: its value depends on the state, not checked here

    def match(self, runs: list, before: dict, after: dict) -> list:
        """One verdict per run between two decision points, in run order (``runs``: Run, oldest first); ``before`` and
        ``after`` the views at the two decision points."""
        t = time.perf_counter()
        self.calls += 1
        state = dict(before)
        later_writes = collections.Counter()
        cands_of = []
        for r in runs:
            cands = self.by_point.get((r.unit, r.code0), [])
            cands_of.append(cands)
        for cands in cands_of:
            for rec in cands:
                for k in rec[4]:
                    later_writes[k] += 1
        out = []
        for r, cands in zip(runs, cands_of):
            for rec in cands:
                for k in rec[4]:
                    later_writes[k] -= 1
            if not cands:
                out.append(Verdict(r, "unplaced"))
                continue
            ok, why = [], []
            end = r.code1 if r.code1 >= 0 else self.nowhere
            for rec in cands:
                idx, unit, w, pre, eff, checks = rec
                bad = next((k for k, vs in pre.items() if state.get(k) not in vs), None)
                if bad is not None:
                    why.append((idx, f"pre {bad}"))
                    continue
                pos = eff.get(f"pos:{unit}")
                if pos is not None and not self._value_ok(pos, end):
                    why.append((idx, f"end pos:{unit}"))
                    continue
                miss = next((k for k, e in eff.items() if not k.startswith("pos:") and later_writes[k] == 0
                             and not self._value_ok(e, after.get(k))), None)
                if miss is not None:
                    why.append((idx, f"effect {miss}"))
                    continue
                if self.decide_check is not None:
                    failed = next((c for c in checks if self.decide_check(c, r) is False), None)
                    if failed is not None:
                        why.append((idx, f"check {failed[0]}"))
                        continue
                ok.append(rec)
            if len(ok) == 1:
                out.append(Verdict(r, "explained", ok[0][0], why))
                for k, e in ok[0][4].items():
                    if e[0] == "set":
                        state[k] = e[1]
            else:
                out.append(Verdict(r, "ambiguous" if ok else "unexplained", [o[0] for o in ok] or None, why))
        self.seconds += time.perf_counter() - t
        return out
