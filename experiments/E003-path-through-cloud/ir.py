"""E003 step 1: the script IR, the one parser every later step reads the scripts through.

usage: uv run python experiments/E003-path-through-cloud/ir.py   (self-check, writes results/ir.log)

``load()`` reads the descumm text in ``data/scripts-dos-cd/`` into **units**:

* ``("global", n)``, ``("local", room, n)``, ``("entry", room)``, ``("exit", room)``: one script each;
* ``("obj", obj)``: an object's whole verb block, with ``entries = {verb: instruction index}``. Every
  verb in the object's event table is kept, also when several share one offset, and the 0xFF
  default entry is verb 255. A verb's entry is the first event in table order whose verb is that
  verb or 0xFF (``getVerbEntrypoint``, scummvm script.cpp:242-248); a later event for the same verb
  is listed in ``shadowed``. Objects with no verb code have no unit.

An **instruction** (``Ins``) has its byte offset, opcode byte, statement text (``Exprmode `` stripped),
file and line for citations, and the parsed statement. Control flow comes from offsets, not braces:
``next`` is the fallthrough, ``target`` a jump's destination, both instruction indices, where
``len(unit.ins)`` means the end of the script. A jump to an offset no instruction starts at goes to
the next instruction after it and is counted as an anomaly.

Statements: ``Jump(cond, to, form)`` jumps to byte offset ``to`` when ``cond`` is false (``None``:
always), as o5's conditional opcodes do (scummvm script_v5.cpp:402-410, ``jumpRelative(cond)`` jumps
when cond is false); ``form`` is how descumm printed it. ``Assign(lhs, op, rhs)`` for ``=``, ``+=``,
``-=``, ``*=``, ``/=``, ``++``, ``--`` (``rhs`` is ``Const(1)`` for the last two). ``Start`` for
startScript, chainScript and startObject. ``Op(name, args, subops, flags)`` for everything else;
``subops`` is the bracketed list of ActorOps, VerbOps, print, printEgo and drawObject, and ``flags``
are bare words (``doSentence(STOP)``). Expressions: Const, Var (descumm's VAR_ names as numbers),
Bit, Local, Index (a variable index, ``Var[a + Var[b]]``; a constant one is folded as readVar adds
it, scummvm script.cpp:573-580, so ``Bit[n + k]`` is ``Bit(n+k)``), BinOp, Neg, Getter (any call in
expression position, also the text codes in strings), Embed (an opcode run inside an expression: it
writes var 0 and its value is var 0, script_v5.cpp o5_expression case 6), Str (an empty argument is
the empty string, which descumm prints as nothing), Concat (a string built from Str parts and text
codes: descumm puts every arithmetic operator in parentheses, descumm.cpp:868-878, so a bare ``+`` joins
string parts, descumm-common.cpp:280, 361), List. Conditions: Cmp, Not, Truth.

Hidden instructions. descumm prints nothing for some jumps and gives the next printed line their
offset (descumm-tool.cpp:418-430: a line's offset is where the last printed line ended). Three
descumm rules, none tied to a game:

* ``} else if (c) /*T*/ {`` at X is a jump at X (the end of the previous branch, its target not
  printed) and the test at X+3. ``maybeAddElseIf`` (descumm-common.cpp:178-210) accepts the line only
  when that target is T or the target of a jump at T-3, and the text cannot tell which: a branch end
  and an empty branch whose own last jump goes elsewhere print alike. So the target is read from the
  unit's raw block (the 2-byte operand at X+1) when ``load`` has it; otherwise the text rule picks the
  jump at T-3 when it lands at or after T, else T, and the unit's notes say the target is unverified;
* ``while (c) /*T*/ {`` at W has a jump back to W at T-3, printed as the closing ``}`` (descumm.cpp:
  1466-1470 prints nothing for it);
* ``} else /*T*/ {`` at X followed by the next line at X+3k, k > 1, is k jumps: a later pending else
  replaces an unprinted one (descumm.cpp:1458-1464), which ``maybeAddElse`` allows only when the
  earlier jump lands right after the later one (descumm-common.cpp:159-160). Jump j < k-1 goes to
  X+3(j+2), the last to T.

Engine semantics marked per instruction (scummvm script_v5.cpp, MI1 enhancements off: every MI1
workaround in these handlers is gated by ``enhancementEnabled`` or by another release):

* ``yields``: "frame" for breakHere (:815-832, ``_currentScript = 0xFF``); "delay" for delay and
  delayVariable (:972-987, paused, then o5_breakHere); "wait" for the WaitFor* ops (o5_wait,
  :3272-3310), which re-run themselves next frame while the actor walks, a message shows, the camera
  moves or the sentence script runs, and fall through at once otherwise;
* ``ends`` "always" for stopObjectCode (:3009), stopScript(0) (:3031-3036) and chainScript (:834-851,
  the new script inherits the running one's freeze and recursive flags);
* ``ends`` "room" for loadRoom, loadRoomWithEgo and actorFollowCamera (camera.cpp:69-70, when the actor is in
  another room). The room change kills the running script when its slot is a room object's, a local
  script's or the room's entry or exit script's (room.cpp:58-72, script.cpp:1083-1105; entry and exit
  run as room slots, script.cpp:995, 1046), and a slot's kind is fixed when it starts (script.cpp:144).
  So "room" always ends entry, exit and local units and never ends global units. It ends an object unit
  only when the object was not owned by an actor when the script started: an owned object's script runs
  as an inventory slot (object.cpp:361-367) and survives;
* ``ends`` "if_self" for the ops that stop other scripts, which end the running script when it is one of them:
  stopScript(n) and startScript without R stop global or local script n (runScript stops it first,
  script.cpp:65-66, 262-293, and the stopped caller never resumes, :377-384; an n that is 0 at run time
  makes stopScript end the running script and startScript do nothing); startObject (always without R,
  script_v5.cpp:2881-2890) and stopObjectScript stop every script of object o, whatever the verb
  (script.cpp:117-118, 296-327); setOwnerOf(o, 0), and setOwnerOf with an owner that is not a
  constant and may be 0, stops o's scripts (object.cpp:132-133, 180); doSentence(STOP) stops
  VAR_SENTENCE_SCRIPT (:1000-1010);
* ``esc``: beginOverride skips the jump after it (script.cpp:1711-1726), so its ``next`` is the
  instruction after that jump and ``esc`` is the jump, where Esc resumes with VAR_OVERRIDE = 1
  (script.cpp:1689-1709).

Left to the interpreter (engine effects the flags do not carry):

* an embedded ``<VAR_RESULT = getter(...)>`` writes var 0;
* in classOfIs and setClass a class with the 0x80 bit means "has" / "set", without it "has not" / "clear";
* setVarRange's first argument is the first variable it writes, not a value;
* hook scripts run on the spot, each by runScript(hook, 0, 0), without R, so each first stops a running
  copy of its hook, the running script too when it is that hook: VAR_INVENTORY_SCRIPT for pickupObject
  (:2033), setOwnerOf (object.cpp:173), setObjectName (object.cpp:1314), actorFollowCamera and
  loadRoomWithEgo (setCameraFollows, camera.cpp:85, again at :206-208 when the followed actor changes;
  script_v5.cpp:1895); VAR_SCROLL_SCRIPT whenever the camera is set (camera.cpp:52-55) and on every frame it scrolls
  (moveCamera, camera.cpp:163-166); the exit and
  entry hooks on a room change (script.cpp:989-1060); VAR_CUTSCENE_START_SCRIPT and
  VAR_CUTSCENE_END_SCRIPT for cutscene and endCutscene (script.cpp:1646, 1686);
* stopping a script or object that has an open cutscene or override is an error() in v5, also a room
  change from inside one (script.cpp:275, 309; room.cpp:62, 68);
* systemOps (restart, quit) is not marked as an end.

Var names. descumm prints var 41 as VAR_TALK_ACTOR too (var_names2), the name of var 25 in v5
(``AMBIGUOUS``). The parser reads the name as 25; ``_read`` looks for each candidate's 2-byte operand
in the instruction's bytes and uses the one it finds. With no block, or both or neither found, the
unit's notes say the name is unverified.

A unit whose block holds only its header has no instructions; its notes say "empty unit".

The self-check counts units, instructions and statements, lines that do not parse, every anomaly
class, the object verb entries, a round trip (each code line in the dump is in exactly one unit, and
every statement prints back to its line), and, when the raw blocks are in ``build/``, checks every
opcode byte and every jump target against the bytes. When descumm is built, it also compares every
unit with descumm's flat output (``-i -e -f -w -b``: every jump printed as a goto, so no hidden-jump
rule is needed): offsets, opcodes, jump targets and conditions.
"""

import bisect
import collections
import functools
import json
import re
import subprocess
from dataclasses import dataclass, field, fields, is_dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SCRIPTS = ROOT / "data" / "scripts-dos-cd"
DESCUMM = ROOT / "build" / "scummvm-tools" / "descumm"  # only for the self-check's flat comparison

JUMP = 0x18  # o5_jumpRelative, script_v5.cpp:72; its operand is a 2-byte offset, so it is 3 bytes long
DEFAULT_VERB = 0xFF  # the default verb entry (script.cpp:245)
# Code offset of each kind's first opcode in its raw block (data/scripts-dos-cd/index.json "offset_bases").
HEADER = {"global": 8, "entry": 8, "exit": 8, "local": 9, "obj": 0}

# descumm prints var n by name (descumm.cpp:486-500: var_names5 at :342, then var_names4 at :253 for
# n = 54 and var_names2 for n = 41). The numbers are ScummVM's v5 ones (vars.cpp:36-102 and :164-175),
# except var 0, which descumm calls VAR_RESULT (descumm.cpp:344) and ScummVM VAR_KEYPRESS (vars.cpp:37):
# the scripts use it as the result register.
VARS = {
    "VAR_RESULT": 0, "VAR_EGO": 1, "VAR_CAMERA_POS_X": 2, "VAR_HAVE_MSG": 3, "VAR_ROOM": 4, "VAR_OVERRIDE": 5,
    "VAR_MACHINE_SPEED": 6, "VAR_ME": 7, "VAR_NUM_ACTOR": 8, "VAR_CURRENT_LIGHTS": 9, "VAR_CURRENTDRIVE": 10,
    "VAR_TMR_1": 11, "VAR_TMR_2": 12, "VAR_TMR_3": 13, "VAR_MUSIC_TIMER": 14, "VAR_ACTOR_RANGE_MIN": 15,
    "VAR_ACTOR_RANGE_MAX": 16, "VAR_CAMERA_MIN_X": 17, "VAR_CAMERA_MAX_X": 18, "VAR_TIMER_NEXT": 19,
    "VAR_VIRT_MOUSE_X": 20, "VAR_VIRT_MOUSE_Y": 21, "VAR_ROOM_RESOURCE": 22, "VAR_LAST_SOUND": 23,
    "VAR_CUTSCENEEXIT_KEY": 24, "VAR_TALK_ACTOR": 25, "VAR_CAMERA_FAST_X": 26, "VAR_SCROLL_SCRIPT": 27,
    "VAR_ENTRY_SCRIPT": 28, "VAR_ENTRY_SCRIPT2": 29, "VAR_EXIT_SCRIPT": 30, "VAR_EXIT_SCRIPT2": 31,
    "VAR_VERB_SCRIPT": 32, "VAR_SENTENCE_SCRIPT": 33, "VAR_INVENTORY_SCRIPT": 34, "VAR_CUTSCENE_START_SCRIPT": 35,
    "VAR_CUTSCENE_END_SCRIPT": 36, "VAR_CHARINC": 37, "VAR_WALKTO_OBJ": 38, "VAR_DEBUGMODE": 39,
    "VAR_HEAPSPACE": 40, "VAR_RESTART_KEY": 42, "VAR_PAUSE_KEY": 43, "VAR_MOUSE_X": 44, "VAR_MOUSE_Y": 45,
    "VAR_TIMER": 46, "VAR_TIMER_TOTAL": 47, "VAR_SOUNDCARD": 48, "VAR_VIDEOMODE": 49, "VAR_MAINMENU_KEY": 50,
    "VAR_FIXEDDISK": 51, "VAR_CURSORSTATE": 52, "VAR_USERPUT": 53, "VAR_V5_TALK_STRING_Y": 54,
    "VAR_SOUNDRESULT": 56, "VAR_TALKSTOP_KEY": 57, "VAR_FADE_DELAY": 59, "VAR_NOSUBTITLES": 60,
    "VAR_SOUNDPARAM": 64, "VAR_SOUNDPARAM2": 65, "VAR_SOUNDPARAM3": 66, "VAR_INPUTMODE": 67,
    "VAR_MEMORY_PERFORMANCE": 68, "VAR_VIDEO_PERFORMANCE": 69, "VAR_ROOM_FLAG": 70, "VAR_GAME_LOADED": 71,
    "VAR_NEW_ROOM": 72,
}
# Names descumm prints for more than one var: var 41 is VAR_TALK_ACTOR in var_names2. The bytes decide (_read).
AMBIGUOUS = {"VAR_TALK_ACTOR": (25, 41)}
VAR_NAME = {n: name for name, ns in AMBIGUOUS.items() for n in ns} | {n: name for name, n in VARS.items()}
# Bare words descumm prints: startScript's freeze resistant and recursive (descumm.cpp:3084-3089), doSentence(STOP).
FLAGS = {"F", "R", "STOP"}

YIELDS = {"breakHere": "frame", "delay": "delay", "delayVariable": "delay", "WaitForActor": "wait",
          "WaitForMessage": "wait", "WaitForCamera": "wait", "WaitForSentence": "wait"}
ROOM_CHANGE = {"loadRoom", "loadRoomWithEgo", "actorFollowCamera"}


# --- the AST ---------------------------------------------------------------------------------
# Frozen dataclasses, so equal nodes hash alike and nodes of different kinds never compare equal.

@dataclass(frozen=True)
class Const:
    value: int

@dataclass(frozen=True)
class Var:
    n: int

@dataclass(frozen=True)
class Bit:
    n: int

@dataclass(frozen=True)
class Local:
    n: int

@dataclass(frozen=True)
class Index:
    kind: str  # "var", "bit" or "local"
    base: int
    offset: object  # the expression added to base

@dataclass(frozen=True)
class BinOp:
    op: str  # + - * /
    a: object
    b: object

@dataclass(frozen=True)
class Neg:
    x: object

@dataclass(frozen=True)
class Getter:
    name: str
    args: tuple

@dataclass(frozen=True)
class Embed:
    st: object  # the Assign run inside an expression; the value is var 0 afterwards

@dataclass(frozen=True)
class Str:
    text: str  # as descumm printed it, escapes kept

@dataclass(frozen=True)
class Concat:
    parts: tuple  # a string built from Str parts and text codes ("Hi " + getName(Var[n]) + "!")

@dataclass(frozen=True)
class List:
    items: tuple

@dataclass(frozen=True)
class Flag:
    name: str

@dataclass(frozen=True)
class Cmp:
    op: str
    a: object
    b: object

@dataclass(frozen=True)
class Not:
    x: object

@dataclass(frozen=True)
class Truth:
    x: object

@dataclass(frozen=True)
class Assign:
    lhs: object
    op: str
    rhs: object

@dataclass(frozen=True)
class Jump:
    cond: object  # None: always; else the jump is taken when cond is false
    to: int  # byte offset
    form: str  # if, while, elseif, else, goto, goto0, unless; hidden ones: elseif-jump, else-chain, while-back

@dataclass(frozen=True)
class Start:
    name: str  # startScript, chainScript, startObject
    script: object  # the script number, or for startObject the verb entry
    obj: object  # startObject's object, else None
    args: tuple
    freeze: bool  # freeze resistant (F)
    recursive: bool  # recursive (R)

@dataclass(frozen=True)
class Sub:
    name: str
    args: tuple  # for Text, the parts of the concatenation

@dataclass(frozen=True)
class Op:
    name: str
    args: tuple
    subops: tuple = ()
    flags: tuple = ()


@dataclass
class Ins:
    offset: int
    op: int
    text: str
    file: str
    line: int
    st: object
    next: int | None = None  # fallthrough index; None: never falls through
    target: int | None = None  # a Jump's destination index
    esc: int | None = None  # beginOverride: where Esc resumes
    yields: str | None = None  # frame, delay, wait
    ends: str | None = None  # always, room, if_self
    hidden: str | None = None  # why a jump with no line of its own was recovered

    def cite(self) -> str:
        return f"{self.file}:{self.line} [{self.offset:04X}]"


@dataclass
class Unit:
    key: tuple
    room: int | None
    file: str
    block: str  # the raw block under build/, for the byte check
    ins: list[Ins] = field(default_factory=list)
    entries: dict[int, int] = field(default_factory=dict)  # objects: verb -> instruction index
    shadowed: list[tuple[int, int]] = field(default_factory=list)  # (verb, index) of events never used
    at: dict[int, int] = field(default_factory=dict)  # offset -> instruction index
    notes: list[tuple[str, int, str]] = field(default_factory=list)  # anomalies: (class, line, detail)


# --- statements --------------------------------------------------------------------------------

_LINE = re.compile(r"^\[([0-9A-F]{4})\] \((..)\) (\s*)(.*)$")
_TOKEN = re.compile(r'\s*(?:("(?:[^"\\]|\\.)*")|(\d+)|([A-Za-z_][\w.]*)|'
                    r"(==|!=|<=|>=|\+=|-=|\*=|/=|\+\+|--|[-+*/<>=!(),\[\];]))")
_IF = re.compile(r"^(if|while) \((.*)\) /\*([0-9A-F]{4})\*/ \{$")
_ELSE_IF = re.compile(r"^\} else if \((.*)\) /\*([0-9A-F]{4})\*/ \{$")
_ELSE = re.compile(r"^\} else /\*([0-9A-F]{4})\*/ \{$")
_GOTO = re.compile(r"^goto ([0-9A-F]{4});$")
_GOTO0 = re.compile(r"^/\* goto ([0-9A-F]{4}); \*/$")  # a jump of 0 bytes (descumm.cpp:1456-1457)
_UNLESS = re.compile(r"^unless \((.*)\) goto ([0-9A-F]{4});$")
# descumm prints drawObject's sub-op glued to the object: "drawObject(OBJsetXY(,X,Y));" (descumm.cpp:3020-3031)
_DRAW = re.compile(r"^drawObject\((.+?)(setXY|setImage)\(,(.*)\)\);$")
_CMP = ("==", "!=", "<", "<=", ">", ">=")


class ParseError(ValueError):
    pass


class _Reader:
    def __init__(self, text: str):
        self.toks, pos = [], 0
        text = text.rstrip()
        while pos < len(text):
            m = _TOKEN.match(text, pos)
            if not m or m.end() == pos:
                raise ParseError(f"bad character at {pos}: {text[pos:pos + 10]!r}")
            self.toks.append(next(g for g in m.groups() if g is not None))
            pos = m.end()
        self.i = 0

    def peek(self) -> str | None:
        return self.toks[self.i] if self.i < len(self.toks) else None

    def take(self) -> str:
        t = self.peek()
        if t is None:
            raise ParseError("unexpected end")
        self.i += 1
        return t

    def expect(self, t: str) -> None:
        got = self.take()
        if got != t:
            raise ParseError(f"expected {t!r}, got {got!r}")

    def end(self) -> None:
        if self.peek() is not None:
            raise ParseError(f"trailing {self.toks[self.i:]}")


def _ref(r: _Reader, kind: str):
    r.expect("[")
    base = int(r.take())
    off = None
    if r.peek() == "+":
        r.take()
        t = r.take()
        if t.isdigit():
            off = Const(int(t))
        elif t in ("Var", "Bit", "Local"):
            off = _ref(r, t)
        else:
            raise ParseError(f"bad index {t!r}")
    r.expect("]")
    node = {"Var": Var, "Bit": Bit, "Local": Local}[kind]
    if off is None:
        return node(base)
    if isinstance(off, Const):  # readVar adds a constant index (scummvm script.cpp:573-580)
        return node(base + off.value)
    return Index(kind.lower(), base, off)


def _items(r: _Reader, close: str) -> tuple:
    out = []
    if r.peek() == close:
        r.take()
        return ()
    while True:
        out.append(Str("") if r.peek() in (",", close) else _expr(r))  # an empty string prints as nothing
        t = r.take()
        if t == close:
            return tuple(out)
        if t != ",":
            raise ParseError(f"expected ',' or {close!r}, got {t!r}")


def _primary(r: _Reader):
    t = r.take()
    if t == "(":
        e = _expr(r, arith=True)
        r.expect(")")
        return e
    if t == "<":  # <VAR_RESULT = getter(...)>: an opcode inside an expression
        st = _assign(r)
        r.expect(">")
        return Embed(st)
    if t == "[":
        return List(_items(r, "]"))
    if t[0] == '"':
        return Str(t[1:-1])
    if t.isdigit():
        return Const(int(t))
    if t in ("Var", "Bit", "Local") and r.peek() == "[":
        return _ref(r, t)
    if t.startswith("VAR_"):
        if t not in VARS:
            raise ParseError(f"unknown engine variable {t}")
        return Var(VARS[t])
    if r.peek() == "(":
        r.take()
        return Getter(t, _items(r, ")"))
    if t in FLAGS:
        return Flag(t)
    raise ParseError(f"unexpected {t!r}")


def _unary(r: _Reader):
    if r.peek() == "-":
        r.take()
        x = _unary(r)
        return Const(-x.value) if isinstance(x, Const) else Neg(x)
    return _primary(r)


def _term(r: _Reader):
    a = _unary(r)
    while r.peek() in ("*", "/"):
        op = r.take()
        a = BinOp(op, a, _unary(r))
    return a


def _expr(r: _Reader, arith: bool = False):
    """descumm puts every arithmetic operator in parentheses (descumm.cpp:868-878), so a "+" outside
    them joins the parts of a string (descumm-common.cpp:280, 361)."""
    if not arith:
        a = _unary(r)
        while r.peek() == "+":
            r.take()
            a = Concat(_parts(a) + _parts(_unary(r)))
        return a
    a = _term(r)
    while r.peek() in ("+", "-"):
        op = r.take()
        a = BinOp(op, a, _term(r))
    return a


def _assign(r: _Reader) -> Assign:
    lhs = _primary(r)
    if not isinstance(lhs, (Var, Bit, Local, Index)):
        raise ParseError(f"cannot assign to {lhs}")
    op = r.take()
    if op in ("++", "--"):
        return Assign(lhs, op, Const(1))
    if op not in ("=", "+=", "-=", "*=", "/="):
        raise ParseError(f"bad assignment {op!r}")
    return Assign(lhs, op, _expr(r))


def _parts(e) -> tuple:
    return e.parts if isinstance(e, Concat) else (e,)


def _call(r: _Reader):
    name = r.take()
    r.expect("(")
    args = list(_items(r, ")"))
    flags = tuple(a.name for a in args if isinstance(a, Flag))
    args = [a for a in args if not isinstance(a, Flag)]
    subops = ()
    if args and isinstance(args[-1], List) and args[-1].items and all(isinstance(x, Getter) for x in args[-1].items):
        subops = tuple(Sub(g.name, g.args) for g in args.pop().items)
    if name in ("startScript", "chainScript") and len(args) == 2 and isinstance(args[1], List) and not subops:
        return Start(name, args[0], None, args[1].items, "F" in flags, "R" in flags)
    if name == "startObject" and len(args) == 3 and isinstance(args[2], List) and not subops and not flags:
        return Start(name, args[1], args[0], args[2].items, False, False)
    return Op(name, tuple(args), subops, flags)


def parse_expr(text: str):
    r = _Reader(text)
    e = _expr(r)
    r.end()
    return e


def parse_cond(text: str):
    """A printed condition: ``a op b``, ``!a``, ``a`` or ``classOfIs(o,[c])``."""
    r = _Reader(text)
    if r.peek() == "!":
        r.take()
        c = Not(_expr(r))
    else:
        a = _expr(r)
        c = Cmp(r.take(), a, _expr(r)) if r.peek() in _CMP else Truth(a)
    r.end()
    return c


def parse_statement(text: str):
    """One printed line that is not a closer, as a statement (an else-if line as its test)."""
    text = text.removeprefix("Exprmode ")
    if m := _IF.match(text):
        return Jump(parse_cond(m[2]), int(m[3], 16), m[1])
    if m := _ELSE_IF.match(text):
        return Jump(parse_cond(m[1]), int(m[2], 16), "elseif")
    if m := _ELSE.match(text):
        return Jump(None, int(m[1], 16), "else")
    if m := _GOTO.match(text):
        return Jump(None, int(m[1], 16), "goto")
    if m := _GOTO0.match(text):
        return Jump(None, int(m[1], 16), "goto0")
    if m := _UNLESS.match(text):
        return Jump(parse_cond(m[1]), int(m[2], 16), "unless")
    if m := _DRAW.match(text):
        text = f"drawObject({m[1]},[{m[2]}({m[3]})]);"
    r = _Reader(text)
    t = r.peek()
    st = _assign(r) if t in ("Var", "Bit", "Local") or (t or "").startswith("VAR_") else _call(r)
    r.expect(";")
    r.end()
    return st


# --- printing back (for citations and the round trip) -----------------------------------------

def show(x) -> str:
    match x:
        case Const(v):
            return str(v)
        case Var(n):
            return VAR_NAME.get(n, f"Var[{n}]")
        case Bit(n):
            return f"Bit[{n}]"
        case Local(n):
            return f"Local[{n}]"
        case Index(kind, base, off):  # descumm prints the index variable by number (descumm.cpp:502-509)
            return f"{kind.title()}[{base} + {type(off).__name__}[{off.n}]]"
        case BinOp(op, a, b):
            return f"({show(a)} {op} {show(b)})"
        case Neg(v):
            return f"-{show(v)}"
        case Getter(name, args):
            return f"{name}({','.join(map(show, args))})"
        case Embed(st):
            return f"<{show(st)[:-1]}>"
        case Str(s):
            return f'"{s}"' if s else ""  # descumm prints an empty string as nothing
        case Concat(parts):
            return " + ".join(map(show, parts))
        case List(items):
            return f"[{','.join(map(show, items))}]"
        case Flag(name):
            return name
        case Cmp(op, a, b):
            return f"{show(a)} {op} {show(b)}"
        case Not(v):
            return f"!{show(v)}"
        case Truth(v):
            return show(v)
        case Assign(lhs, op, rhs):
            return f"{show(lhs)}{op};" if op in ("++", "--") else f"{show(lhs)} {op} {show(rhs)};"
        case Sub(name, args):
            return f"{name}({','.join(map(show, args))})"
        case Start(name, script, obj, args, freeze, recursive):
            head = [show(obj)] if obj is not None else []
            flags = ",F" * freeze + ",R" * recursive
            return f"{name}({','.join(head + [show(script), show(List(args))])}{flags});"
        case Op(name, args, subops, flags):
            if name == "drawObject" and subops:  # descumm's own spelling
                (s,) = subops
                return f"drawObject({show(args[0])}{s.name}(,{','.join(map(show, s.args))}));"
            parts = [show(a) for a in args] + ([show(List(subops))] if subops else []) + list(flags)
            return f"{name}({','.join(parts)});"
        case Jump(cond, to, form):
            c = show(cond) if cond is not None else ""
            return {"if": f"if ({c}) /*{to:04X}*/ {{", "while": f"while ({c}) /*{to:04X}*/ {{",
                    "elseif": f"}} else if ({c}) /*{to:04X}*/ {{", "else": f"}} else /*{to:04X}*/ {{",
                    "goto0": f"/* goto {to:04X}; */", "unless": f"unless ({c}) goto {to:04X};"}.get(form, f"goto {to:04X};")
    raise TypeError(f"cannot show {x!r}")


# --- files into units ---------------------------------------------------------------------------

def _mark(x: Ins) -> None:
    """Set the engine flags of one instruction from its statement."""
    st = x.st
    name = st.name if isinstance(st, (Op, Start)) else None
    x.yields = YIELDS.get(name)
    if name in ("stopObjectCode", "chainScript"):
        x.ends = "always"
    elif name == "stopScript":
        x.ends = "always" if st.args == (Const(0),) else "if_self"
    elif name in ROOM_CHANGE:
        x.ends = "room"
    elif (name == "doSentence" and "STOP" in st.flags) or name in ("startObject", "stopObjectScript") \
            or (isinstance(st, Start) and name == "startScript" and not st.recursive) \
            or (name == "setOwnerOf" and len(st.args) == 2 and not (isinstance(st.args[1], Const) and st.args[1].value)):
        x.ends = "if_self"


def _swap(x, a: int, b: int):
    """``x`` with every Var(a) in it read as Var(b)."""
    if x == Var(a):
        return Var(b)
    if isinstance(x, tuple):
        return tuple(_swap(y, a, b) for y in x)
    if is_dataclass(x):
        return type(x)(*(_swap(getattr(x, f.name), a, b) for f in fields(x)))
    return x


def _read(u: Unit, text: str, data: bytes | None = None) -> None:
    """Fill ``u.ins`` from one file's text, recovering the jumps descumm did not print. ``data`` is the
    unit's raw block, read only for the else-if jumps' targets and ambiguous var names (the module
    docstring says why)."""
    raw = []  # (offset, line, opcode or None for a closer, statement text)
    for n, s in enumerate(text.splitlines(), 1):
        m = _LINE.match(s)
        if m:
            raw.append((int(m[1], 16), n, None if m[2] == "**" else int(m[2], 16), m[4].strip().removeprefix("Exprmode ")))
    closers = {(offs, n) for offs, n, op, _ in raw if op is None}
    ins, elseifs, whiles = [], [], {}
    for k, (offs, n, op, s) in enumerate(raw):
        nxt = raw[k + 1][0] if k + 1 < len(raw) else None
        if op is None:
            continue
        try:
            st = parse_statement(s)
        except (ParseError, IndexError, ValueError) as e:
            u.notes.append(("unparsed", n, f"{s[:60]} ({e})"))
            ins.append(Ins(offs, op, s, u.file, n, None))
            continue
        if isinstance(st, Jump) and st.form == "elseif":  # the previous branch's jump, then the test
            ins.append(Ins(offs, JUMP, "", u.file, n, None, hidden="elseif-jump"))
            elseifs.append((ins[-1], st.to))
            ins.append(Ins(offs + 3, op, s, u.file, n, st))
            continue
        if isinstance(st, Jump) and st.form == "else" and nxt is not None and nxt > offs + 3:
            k3, rest = divmod(nxt - offs, 3)
            if rest:
                u.notes.append(("else gap not a multiple of 3", n, s))
            for j in range(k3 - 1):  # jumps that were pending elses, each landing after the next
                to = offs + 3 * (j + 2)
                ins.append(Ins(offs + 3 * j, JUMP, f"goto {to:04X};", u.file, n, Jump(None, to, "else-chain"),
                               hidden="else-chain"))
            ins.append(Ins(offs + 3 * (k3 - 1), op, s, u.file, n, st))
            u.notes.append(("else chain", n, f"{k3} jumps"))
            continue
        if isinstance(st, Jump) and st.form in ("goto", "goto0", "else") and nxt is not None and nxt != offs + 3:
            u.notes.append(("jump line not 3 bytes", n, s))
        if isinstance(st, Jump) and st.form == "while":
            whiles[st.to - 3] = offs
        ins.append(Ins(offs, op, s, u.file, n, st))
    for offs, n in sorted(closers):
        if offs in whiles:  # the loop's jump back, printed as its closing brace
            to = whiles.pop(offs)
            ins.append(Ins(offs, JUMP, f"goto {to:04X};", u.file, n, Jump(None, to, "while-back"), hidden="while-back"))
    for offs, w in whiles.items():
        u.notes.append(("while without a closer at its end", 0, f"{w:04X}"))
    ins.sort(key=lambda x: x.offset)
    u.ins = ins
    u.at = {x.offset: i for i, x in enumerate(ins)}
    if not ins:
        u.notes.append(("empty unit", 0, ""))
    if len(u.at) != len(ins):
        u.notes.append(("two instructions at one offset", 0, ""))
    for i, (offs, n, op, s) in enumerate(raw[:-1]):  # bytes no instruction or rule accounts for
        if op is None and raw[i + 1][0] > offs and offs not in u.at:
            u.notes.append(("closer gap", n, f"{offs:04X}..{raw[i + 1][0]:04X}"))
    guess = {}  # the text rule, later ones first: a chain points forward
    for x, t in sorted(elseifs, key=lambda e: -e[0].offset):
        prev = ins[u.at[t - 3]] if t - 3 in u.at else None
        if prev is None:
            to = None
        elif prev.hidden == "elseif-jump":
            to = guess[prev.offset]
        else:
            to = prev.st.to if isinstance(prev.st, Jump) and prev.st.cond is None else None
        guess[x.offset] = to if to is not None and to >= t else t
    base = HEADER[u.key[0]]
    for x, t in elseifs:
        to = guess[x.offset]
        if data is not None and data[base + x.offset] == JUMP:  # the operand descumm did not print
            to = x.offset + 3 + int.from_bytes(data[base + x.offset + 1: base + x.offset + 3], "little", signed=True)
            u.notes.append(("else-if target from bytes, text rule " + ("agrees" if to == guess[x.offset] else "differs"),
                            x.line, f"text {guess[x.offset]:04X}, bytes {to:04X}"))
        else:
            u.notes.append(("else-if target from the text rule, unverified", x.line, f"{to:04X}"))
        x.st, x.text = Jump(None, to, "elseif-jump"), f"goto {to:04X};"
    for i, x in enumerate(ins):  # a name descumm prints for two vars: the 2-byte operand in the bytes decides
        for name, ns in AMBIGUOUS.items():
            if x.st is None or x.hidden or not re.search(rf"\b{name}\b", x.text):
                continue
            found = []
            if data is not None:
                end = ins[i + 1].offset if i + 1 < len(ins) else len(data) - base
                found = [n for n in ns if n.to_bytes(2, "little") in data[base + x.offset + 1: base + end]]
            if len(found) != 1:
                u.notes.append(("ambiguous name, unverified", x.line, f"{name} read as var {VARS[name]}"))
                continue
            if found[0] != VARS[name]:
                x.st = _swap(x.st, VARS[name], found[0])
            u.notes.append(("ambiguous name from bytes", x.line, f"{name} is var {found[0]}"))
    offsets = [x.offset for x in ins]
    for i, x in enumerate(ins):
        if x.st is None:
            x.next = i + 1
            continue
        _mark(x)
        x.next = None if x.ends == "always" else i + 1
        if isinstance(x.st, Jump):
            t = x.st.to
            if t in u.at:
                x.target = u.at[t]
            elif t > offsets[-1]:
                x.target = len(ins)
                u.notes.append(("jump to the end", x.line, f"{t:04X}"))
            else:
                x.target = bisect.bisect_left(offsets, t)
                u.notes.append(("target not an instruction offset", x.line, f"{t:04X}"))
            if x.st.cond is None:
                x.next = None
    for i, x in enumerate(ins):
        if isinstance(x.st, Op) and x.st.name == "beginOverride":
            after = ins[i + 1] if i + 1 < len(ins) else None
            if after is None or not isinstance(after.st, Jump) or after.st.cond is not None or after.offset != x.offset + 2:
                u.notes.append(("beginOverride not followed by a jump", x.line, ""))
            else:
                x.next, x.esc = i + 2, i + 1


def _entries(u: Unit, verbs: list[dict]) -> None:
    """Object entries from the event table, in table order, first match wins (verb or 0xFF)."""
    table = [(v["verb"], int(v["offset"], 16)) for v in verbs]
    used = set()
    for verb in dict.fromkeys(v for v, _ in table):
        p = next(p for p, (v, _) in enumerate(table) if v in (verb, DEFAULT_VERB))
        if table[p][1] not in u.at:
            u.notes.append(("verb entry not an instruction offset", 0, f"verb {verb} @ {table[p][1]:04X}"))
            continue
        u.entries[verb] = u.at[table[p][1]]
        used.add(p)
    u.shadowed = [(v, u.at.get(offs, -1)) for p, (v, offs) in enumerate(table) if p not in used]


@functools.cache
def load(scripts: Path = SCRIPTS, blocks: Path | None = ROOT) -> dict[tuple, Unit]:
    """Every unit of the dump, keyed as the module docstring says. ``blocks`` is where the raw
    block paths of index.json start (the dump pipeline writes them under build/); ``None``, or a
    missing block, leaves the else-if targets to the text rule and ambiguous var names as read, and
    the unit's notes say so."""
    meta = json.loads((scripts / "index.json").read_text())
    units = {}

    def add(key, room, file, block, verbs=None):
        u = Unit(key, room, file, block)
        raw = blocks / block if blocks is not None else None
        _read(u, (scripts / file).read_text(), raw.read_bytes() if raw is not None and raw.exists() else None)
        if verbs:
            _entries(u, verbs)
        units[key] = u

    for n, g in sorted(meta["global_scripts"].items(), key=lambda kv: int(kv[0])):
        add(("global", int(n)), None, g["file"], g["block"])
    for r, room in sorted(meta["rooms"].items(), key=lambda kv: int(kv[0])):
        r = int(r)
        for kind, tag in (("entry", "ENCD"), ("exit", "EXCD")):
            if room.get(kind):
                add((kind, r), r, room[kind]["file"], f"{room['lflf']}/ROOM/{tag}")
        for n, loc in sorted(room.get("local_scripts", {}).items(), key=lambda kv: int(kv[0])):
            add(("local", r, int(n)), r, loc["file"], f"{room['lflf']}/ROOM/LSCR_{int(n):04d}")
    for o, ob in sorted(meta["objects"].items(), key=lambda kv: int(kv[0])):
        if ob.get("verbs"):
            add(("obj", int(o)), ob["room"], ob["file"], ob["verb_block"], ob["verbs"])
    return units


def head(st) -> str:
    """A statement's name for counting."""
    match st:
        case Jump(cond, _, form):
            return f"jump {form}"
        case Assign(_, op, rhs):
            return f"assign {op}" + (f" {rhs.name}" if isinstance(rhs, Getter) else "")
        case Op(name) | Start(name):
            return name
    return "?"


# --- self-check -----------------------------------------------------------------------------------

def _normal(text: str) -> str:
    """A printed line as ``show`` prints it: constant indexes folded."""
    def fold(m):
        n = int(m[2]) + int(m[3])
        return VAR_NAME.get(n, f"Var[{n}]") if m[1] == "Var" else f"{m[1]}[{n}]"
    return re.sub(r"\b(Var|Bit|Local)\[(\d+) \+ (\d+)\]", fold, text)


def byte_check(units: dict[tuple, Unit], blocks: Path = ROOT) -> dict:
    """Every opcode byte and jump target against the raw blocks, when ``build/`` has them."""
    out = collections.Counter()
    bad = []
    for u in units.values():
        path = blocks / u.block
        if not path.exists():
            out["units without a block"] += 1
            continue
        data, base = path.read_bytes(), HEADER[u.key[0]]
        size = len(data) - base
        for i, x in enumerate(u.ins):
            out["instructions"] += 1
            if x.offset >= size or data[base + x.offset] != x.op:
                out["opcode byte differs"] += 1
                bad.append(f"opcode {x.cite()} {x.op:02X} vs {data[base + x.offset] if x.offset < size else None}")
            if not isinstance(x.st, Jump):
                continue
            end = u.ins[i + 1].offset if i + 1 < len(u.ins) else size
            to = end + int.from_bytes(data[base + end - 2: base + end], "little", signed=True)
            mine = u.ins[x.target].offset if x.target < len(u.ins) else None
            ok = to == mine or (mine is None and to > u.ins[-1].offset)
            out["jumps"] += 1
            out["jumps agree"] += ok
            if x.hidden:
                out[f"hidden {x.hidden} agree" if ok else f"hidden {x.hidden} differ"] += 1
            if not ok:
                bad.append(f"jump {x.cite()} {x.text[:50]!r}: bytes {to:04X}, IR {mine}")
    return {"counts": dict(out), "bad": bad}


def flat_check(units: dict[tuple, Unit], blocks: Path = ROOT, descumm: Path = DESCUMM) -> dict:
    """Every unit against descumm's flat output of its raw block, where each jump is a printed goto:
    the same offsets and opcodes, and for each jump the same target and condition."""
    out = collections.Counter()
    bad = []
    for u in units.values():
        path = blocks / u.block
        if not path.exists():
            out["units without a block"] += 1
            continue
        text = subprocess.run([str(descumm), "-5", "-o", "-i", "-e", "-f", "-w", "-b", str(path)],
                              capture_output=True, text=True).stdout
        flat = [(int(m[1], 16), int(m[2], 16), m[4].strip()) for m in map(_LINE.match, text.splitlines())
                if m and m[2] != "**"]
        if [(o, op) for o, op, _ in flat] != [(x.offset, x.op) for x in u.ins]:
            out["units differ"] += 1
            bad.append(f"{u.file}: {len(flat)} flat instructions, IR {len(u.ins)}")
            continue
        out["units same"] += 1
        cond = lambda c: show(c) if c is not None else None  # noqa: E731
        for (_, _, s), x in zip(flat, u.ins):
            fj = parse_statement(s) if _GOTO.match(s) or _GOTO0.match(s) or _UNLESS.match(s) else None
            if fj is None:
                if isinstance(x.st, Jump):
                    out["jumps differ"] += 1
                    bad.append(f"{x.cite()}: flat prints {s[:50]!r}")
                continue
            mine = u.ins[x.target].offset if isinstance(x.st, Jump) and x.target < len(u.ins) else None
            ok = isinstance(x.st, Jump) and (fj.to == mine or (mine is None and fj.to > u.ins[-1].offset)) \
                and cond(fj.cond) == cond(x.st.cond)
            out["jumps same" if ok else "jumps differ"] += 1
            if not ok:
                bad.append(f"{x.cite()}: flat {s[:50]!r}, IR {show(x.st)[:50] if x.st else None!r} to {mine}")
    return {"counts": dict(out), "bad": bad}


def check(units: dict[tuple, Unit] | None = None, scripts: Path = SCRIPTS) -> list[str]:
    units = units or load(scripts)
    log = []
    files = sorted(str(p.relative_to(scripts)) for p in scripts.rglob("*.txt"))
    kinds = collections.Counter(k[0] for k in units)
    ins = [x for u in units.values() for x in u.ins]
    log.append(f"files: {len(files)} .txt; units: {len(units)} ({', '.join(f'{k} {n}' for k, n in kinds.items())})")
    log.append(f"instructions: {len(ins)}, of them recovered jumps: "
               + ", ".join(f"{k} {n}" for k, n in collections.Counter(x.hidden for x in ins if x.hidden).items()))
    heads = collections.Counter(head(x.st) for x in ins if x.st is not None)
    log.append(f"statement kinds: {len(heads)}")
    log += [f"   {n:6} {h}" for h, n in heads.most_common()]
    log.append("yields: " + ", ".join(f"{k} {n}" for k, n in collections.Counter(x.yields for x in ins if x.yields).items())
               + "; ends: " + ", ".join(f"{k} {n}" for k, n in collections.Counter(x.ends for x in ins if x.ends).items())
               + f"; beginOverride with an Esc jump: {sum(x.esc is not None for x in ins)}")
    notes = collections.Counter(c for u in units.values() for c, _, _ in u.notes)
    unparsed = collections.Counter(d.split("(")[0][:30] for u in units.values() for c, _, d in u.notes if c == "unparsed")
    log.append(f"unparsed lines: {sum(unparsed.values())}")
    log += [f"   {n:5} {h}" for h, n in unparsed.most_common()]
    log.append("notes and anomalies by class:")
    log += [f"   {n:5} {c}" for c, n in sorted(notes.items()) if c != "unparsed"]
    for c in ("target not an instruction offset", "else chain", "closer gap", "jump line not 3 bytes",
              "else-if target from bytes, text rule differs", "empty unit", "ambiguous name from bytes",
              "ambiguous name, unverified",
              "beginOverride not followed by a jump", "verb entry not an instruction offset", "jump to the end"):
        for u in units.values():
            for c2, line, d in u.notes:
                if c2 == c:
                    log.append(f"   {c}: {u.file}:{line} {d}")
    # object verb entries
    objs = [u for u in units.values() if u.key[0] == "obj"]
    entries = sum(len(u.entries) for u in objs)
    shared = drop = 0
    for u in objs:
        by_index = collections.Counter(u.entries.values())
        shared += sum(m for m in by_index.values() if m > 1)
        drop += sum(m - 1 for m in by_index.values() if m > 1)
    meta = json.loads((scripts / "index.json").read_text())
    log.append(f"object entries: {entries} verbs in {len(objs)} objects (+{sum(len(u.shadowed) for u in objs)} shadowed "
               f"events = {entries + sum(len(u.shadowed) for u in objs)}, index.json counts "
               f"{meta['counts']['verb_entry_points']}); {shared} share an offset with another verb "
               f"(one entry kept per offset would drop {drop}); default verb {DEFAULT_VERB}: "
               f"{sum(DEFAULT_VERB in u.entries for u in objs)} objects")
    # round trip: every code line in exactly one unit, every statement prints back
    owner = collections.defaultdict(set)
    for u in units.values():
        for x in u.ins:
            if not x.hidden:
                owner[(x.file, x.line)].add(u.key)
    lines = missing = twice = 0
    printed = collections.defaultdict(set)  # file -> offsets descumm printed on code lines
    for f in files:
        for n, s in enumerate((scripts / f).read_text().splitlines(), 1):
            m = _LINE.match(s)
            if m and m[2] != "**":
                lines += 1
                printed[f].add(int(m[1], 16))
                k = len(owner.get((f, n), ()))
                missing += k == 0
                twice += k > 1
    naive = sum(1 for u in units.values() for x in u.ins if isinstance(x.st, Jump) and not x.hidden
                and x.st.to not in printed[u.file] and x.st.to <= max(printed[u.file]))
    own = sum(1 for x in ins if not x.hidden)
    differ = [x for x in ins if not x.hidden and x.st is not None and show(x.st) != _normal(x.text)]
    log.append(f"round trip: {lines} code lines, {own} line instructions; lines in no unit {missing}, "
               f"in two units {twice}; statements that print back differently: {len(differ)}")
    log += [f"   {x.cite()}: {x.text[:70]!r} -> {show(x.st)[:70]!r}" for x in differ[:20]]
    log.append(f"printed jump targets on no printed line (reading the text as printed, without the hidden "
               f"jumps): {naive}; after recovery: {notes['target not an instruction offset']}")
    log.append(f"code-less objects (no unit): {sum(1 for o in meta['objects'].values() if not o.get('verbs'))}")
    b = byte_check(units)
    if b["counts"].get("instructions"):
        log.append("byte check: " + ", ".join(f"{k} {v}" for k, v in sorted(b["counts"].items())))
        log += [f"   {s}" for s in b["bad"][:30]]
    else:
        log.append("byte check: skipped, no raw blocks under build/")
    if DESCUMM.exists() and b["counts"].get("instructions"):
        f = flat_check(units)
        log.append("flat descumm: " + ", ".join(f"{k} {v}" for k, v in sorted(f["counts"].items())))
        log += [f"   {s}" for s in f["bad"][:30]]
    else:
        log.append("flat descumm: skipped, no descumm build or no raw blocks")
    return log


def main() -> None:
    log = check()
    out = HERE / "results"
    out.mkdir(exist_ok=True)
    (out / "ir.log").write_text("\n".join(log) + "\n")
    print("\n".join(log))


if __name__ == "__main__":
    main()
