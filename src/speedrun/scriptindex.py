"""What the game's scripts change, read from the decompiled text (``data/scripts-*``).

The milestone search (``speedrun.search``) needs to know which player action can
make a fact true: which object and verb give an item, set a bit, open a door or
lead to a room. This module reads that off the descumm output once, so nobody
looks it up by hand:

* a **handler** is an object's verb entry (the player's ``verb object``), or a
  room's entry, exit or local script, or a global script;
* each handler's **effects** are the writes it makes (bit, var, owner, state,
  class, room) with the ``if`` conditions around them, as text;
* **calls** (``startScript``, ``chainScript``, ``startObject``, ``doSentence``)
  are followed a few levels, with ``Local[i]`` bound to the call's constant
  arguments and ``VAR_ME`` to the object, so ``Open door`` -> global 25 ->
  ``setState(Local[0], 1)`` becomes ``state 428 = 1``.

Values that are not constants (expressions, variables) are kept as ``None``:
the effect says *something* is written there, and the engine decides the rest.
"""

import json
import re
from dataclasses import dataclass, field
from pathlib import Path

EGO = 1  # Guybrush's actor number (VAR_EGO); pickupObject gives an object to him
OBJ2 = "OBJ2"  # the sentence's second object, as guards name it ("OBJ2 == 403": used with the lock)

# descumm prints the engine's own variables by name; SCUMM v5 numbers them as
# ScummEngine::setupScummVars() and ScummEngine_v5::setupScummVars() do (scumm/vars.cpp).
NAMED_VARS = {
    "VAR_KEYPRESS": 0, "VAR_EGO": 1, "VAR_CAMERA_POS_X": 2, "VAR_HAVE_MSG": 3, "VAR_ROOM": 4, "VAR_OVERRIDE": 5,
    "VAR_MACHINE_SPEED": 6, "VAR_ME": 7, "VAR_NUM_ACTOR": 8, "VAR_CURRENT_LIGHTS": 9, "VAR_CURRENTDRIVE": 10,
    "VAR_TMR_1": 11, "VAR_TMR_2": 12, "VAR_TMR_3": 13, "VAR_MUSIC_TIMER": 14, "VAR_ACTOR_RANGE_MIN": 15,
    "VAR_ACTOR_RANGE_MAX": 16, "VAR_CAMERA_MIN_X": 17, "VAR_CAMERA_MAX_X": 18, "VAR_TIMER_NEXT": 19,
    "VAR_VIRT_MOUSE_X": 20, "VAR_VIRT_MOUSE_Y": 21, "VAR_ROOM_RESOURCE": 22, "VAR_LAST_SOUND": 23,
    "VAR_CUTSCENEEXIT_KEY": 24, "VAR_TALK_ACTOR": 25, "VAR_CAMERA_FAST_X": 26, "VAR_SCROLL_SCRIPT": 27,
    "VAR_ENTRY_SCRIPT": 28, "VAR_ENTRY_SCRIPT2": 29, "VAR_EXIT_SCRIPT": 30, "VAR_EXIT_SCRIPT2": 31,
    "VAR_VERB_SCRIPT": 32, "VAR_SENTENCE_SCRIPT": 33, "VAR_INVENTORY_SCRIPT": 34, "VAR_CUTSCENE_START_SCRIPT": 35,
    "VAR_CUTSCENE_END_SCRIPT": 36, "VAR_CHARINC": 37, "VAR_WALKTO_OBJ": 38, "VAR_DEBUGMODE": 39,
    "VAR_HEAPSPACE": 40, "VAR_ROOM_WIDTH": 41, "VAR_RESTART_KEY": 42, "VAR_PAUSE_KEY": 43, "VAR_MOUSE_X": 44,
    "VAR_MOUSE_Y": 45, "VAR_TIMER": 46, "VAR_TIMER_TOTAL": 47, "VAR_SOUNDCARD": 48, "VAR_VIDEOMODE": 49,
    "VAR_MAINMENU_KEY": 50, "VAR_FIXEDDISK": 51, "VAR_CURSORSTATE": 52, "VAR_USERPUT": 53,
    "VAR_V5_TALK_STRING_Y": 54, "VAR_NOSUBTITLES": 60,
}

_LINE = re.compile(r"^\[([0-9A-F]{4})\] \((..)\) (\s*)(.*)$")
_VERB_ENTRY = re.compile(r"^# ---- verb (\d+) \(.*?\) entry @ ([0-9A-F]{4})( \[shadowed.*\])? ----")
_IF = re.compile(r"^if \((.*)\) /\*[0-9A-F]{4}\*/ \{$")
_ELSE_IF = re.compile(r"^\} else if \((.*)\) /\*[0-9A-F]{4}\*/ \{$")
_ELSE = re.compile(r"^\} else /\*[0-9A-F]{4}\*/ \{$")

_TERM = r"(-?\d+|Local\[\d+\]|VAR_ME|VAR_EGO|VAR_ROOM)"
_RESULT = re.compile(r"^VAR_RESULT = (.+);$")
_GOTO = re.compile(r"^goto ([0-9A-F]{4});$")
_VAR_ROOM_GUARD = re.compile(r"VAR_ROOM == (\d+)")
_LOCAL = re.compile(r"^Local\[(\d+)\] = " + _TERM + r";")
_EFFECTS = [
    # descumm prints indexed writes as Bit[83 + 2] or Bit[83 + Local[0]] (global 71's trials)
    ("bit", re.compile(r"^Bit\[(\d+)(?: \+ " + _TERM + r")?\] = " + _TERM + r";")),
    ("var", re.compile(r"^Var\[(\d+)(?: \+ " + _TERM + r")?\] = " + _TERM + r";")),
    ("named", re.compile(r"^(VAR_[A-Z_0-9]+) = " + _TERM + r";")),
    ("var_any", re.compile(r"^Var\[(\d+)\](?: (?:=|\+=|-=) ?(.*)|(\+\+|--));$")),  # also Var[357]++
    ("pickup", re.compile(r"pickupObject\(" + _TERM + r",")),
    ("owner", re.compile(r"setOwnerOf\(" + _TERM + r"," + _TERM + r"\)")),
    ("state", re.compile(r"setState\(" + _TERM + r"," + _TERM + r"\)")),
    ("draw", re.compile(r"drawObject\(" + _TERM + r"(?:\)|\w)")),  # state 1 (o5_drawObject's default)
    ("class", re.compile(r"setClass\(" + _TERM + r",\[([^\]]*)\]\)")),
    ("room", re.compile(r"(?<!\w)loadRoom\(" + _TERM + r"\)")),
    ("room_ego", re.compile(r"loadRoomWithEgo\(" + _TERM + r"," + _TERM + r",")),
    ("room_put", re.compile(r"putActorInRoom\(VAR_EGO," + _TERM + r"\)")),  # then actorFollowCamera
    ("actor_room", re.compile(r"putActorInRoom\((\d+)," + _TERM + r"\)")),
    ("walk", re.compile(r"walkActorTo\(VAR_EGO,(-?\d+),(-?\d+)\)")),
    # the argument list holds Local[i] terms, so it ends at the last "]" of the call
    ("start", re.compile(r"(?:startScript|chainScript)\((\d+),\[(.*)\](?:,\w+)?\);")),
    ("object", re.compile(r"startObject\(" + _TERM + r"," + _TERM + r",\[(.*)\]\);")),
    ("sentence", re.compile(r"doSentence\(" + _TERM + r"," + _TERM + r"," + _TERM + r"\)")),
]


@dataclass(frozen=True)
class Effect:
    kind: str  # bit, var, owner, state, class, room
    subject: int | None  # bit or var number, object, room
    value: int | None  # None: not a constant
    guards: tuple[str, ...] = ()  # enclosing conditions, innermost last; "!(...)" for else branches
    where: str = ""  # file [offset] of the write
    expr: str = ""  # the right-hand side of a non-constant var write


@dataclass
class Handler:
    key: tuple  # ("obj", obj, verb) | ("global", n) | ("local", room, n) | ("entry", room) | ("exit", room)
    room: int | None
    file: str
    lines: list[tuple[str, int, str]] = field(default_factory=list)  # (offset, depth, statement)


def _parse_lines(text: str) -> list[tuple[str, int, str]]:
    out = []
    for raw in text.splitlines():
        m = _LINE.match(raw)
        if m:
            st = m.group(4).strip()
            if st.startswith("Exprmode "):  # descumm marks expression assignments
                st = st[len("Exprmode "):]
            out.append((m.group(1), len(m.group(3)) // 2, st))
    return out


def _split_object(text: str) -> dict[int, list[tuple[str, int, str]]]:
    """``{verb: lines}`` of an object file's verb entries (shadowed entries left out)."""
    entries: dict[int, list[str]] = {}
    current = None
    for raw in text.splitlines():
        m = _VERB_ENTRY.match(raw)
        if m:
            verb, shadowed = int(m.group(1)), bool(m.group(3))
            current = None if shadowed or verb in entries else verb
            if current is not None:
                entries[current] = []
            continue
        if current is not None:
            entries[current].append(raw)
    return {v: _parse_lines("\n".join(ls)) for v, ls in entries.items()}


MAX_JUMPS = 16  # branches followed into one goto target
MAX_GUARDS = 10  # innermost conditions kept per write (the outer ones of long call chains are dropped)


def _tail(guards: tuple[str, ...]) -> tuple[str, ...]:
    return guards[-MAX_GUARDS:]


def _negate_last(entry: list[str]) -> list[str]:
    """An else branch's guards: the earlier branches' negations, kept flat ("!(a)", "!(b)")."""
    if not entry:
        return []
    negs = [g for g in entry[:-1] if g.startswith("!(")]
    last = entry[-1]
    return negs + ([last[2:-1]] if False else [f"!({last})"]) if not last.startswith("!(") else negs + [last]


_UNLESS = re.compile(r"^unless \(Local\[(\d+)\] > (\d+)\) goto ([0-9A-F]{4});$")


def _loops(lines: list[tuple[str, int, str]]) -> list[tuple[int, int, int, int, int]]:
    """Counted loops ``Local[i] = a; [label] ... Local[i]++; unless (Local[i] > b) goto label``:
    ``(label offset, end offset, i, a, b)``."""
    out = []
    for k, (offs, _, st) in enumerate(lines):
        m = _UNLESS.match(st)
        if not m:
            continue
        i, b, label = int(m.group(1)), int(m.group(2)), int(m.group(3), 16)
        start = None
        for offs2, _, st2 in lines[:k]:
            if int(offs2, 16) >= label:
                break
            m2 = re.match(rf"^Local\[{i}\] = (-?\d+);$", st2)
            if m2:
                start = int(m2.group(1))
        if start is not None and start <= b:
            out.append((label, int(offs, 16), i, start, b))
    return out


class ScriptIndex:
    def __init__(self, scripts_dir: Path):
        self.dir = Path(scripts_dir)
        meta = json.loads((self.dir / "index.json").read_text())
        self.object_room = {int(o): v["room"] for o, v in meta["objects"].items()}
        self.object_name = {int(o): v["name"] for o, v in meta["objects"].items()}
        self.handlers: dict[tuple, Handler] = {}
        for n, g in meta["global_scripts"].items():
            self._add(("global", int(n)), None, g["file"])
        for r, room in meta["rooms"].items():
            r = int(r)
            for kind in ("entry", "exit"):
                if room.get(kind):
                    self._add((kind, r), r, room[kind]["file"])
            for n, loc in room.get("local_scripts", {}).items():
                self._add(("local", r, int(n)), r, loc["file"])
            for o, obj in room.get("objects", {}).items():
                path = self.dir / obj["file"]
                if not path.exists():
                    continue
                for verb, lines in _split_object(path.read_text()).items():
                    self.handlers[("obj", int(o), verb)] = Handler(("obj", int(o), verb), r, obj["file"], lines)
        self._effects: dict[tuple, list[Effect]] = {}
        # Objects that can be carried: their verbs run wherever ego is, so their local-script
        # calls (startScript(250) on the rope) mean the current room's script.
        self.items: set[int] = set()
        for h in self.handlers.values():
            for _, _, st in h.lines:
                m = re.search(r"pickupObject\((\d+)", st)
                if m:
                    self.items.add(int(m.group(1)))
        for key in list(self.handlers):
            if key[0] == "obj" and any("pickupObject(VAR_ME" in st for _, _, st in self.handlers[key].lines):
                self.items.add(key[1])
        self.local_rooms: dict[int, list[int]] = {}
        for key in self.handlers:
            if key[0] == "local":
                self.local_rooms.setdefault(key[2], []).append(key[1])

    def _add(self, key: tuple, room: int | None, file: str) -> None:
        path = self.dir / file
        if path.exists():
            self.handlers[key] = Handler(key, room, file, _parse_lines(path.read_text()))

    # --- effects --------------------------------------------------------------------------

    def _arg(self, term: str, args: tuple, me: int | None):
        """A call argument: its value, or the OBJ2 symbol passed along."""
        m = re.fullmatch(r"Local\[(\d+)\]", term)
        if m:
            i = int(m.group(1))
            return args[i] if i < len(args) else None
        return self._value(term, args, me)

    def _resolve_terms(self, expr: str, args: tuple, me: int | None) -> str:
        """``expr`` with Local[i], VAR_ME and VAR_EGO replaced by their values where known."""
        def sub(m):
            t = m.group(0)
            if t.startswith("Local"):
                i = int(t[6:-1])
                v = args[i] if i < len(args) else None
                return str(v) if v is not None else t  # an int, or the OBJ2 symbol
            v = self._value(t, args, me)
            return str(v) if v is not None else t
        return re.sub(r"Local\[\d+\]|VAR_ME|VAR_EGO", sub, expr)

    @staticmethod
    def _value(term: str, args: tuple, me: int | None) -> int | None:
        if re.fullmatch(r"-?\d+", term):
            return int(term)
        m = re.fullmatch(r"Local\[(\d+)\]", term)
        if m:
            i = int(m.group(1))
            v = args[i] if i < len(args) else None
            return v if isinstance(v, int) else None
        if term == "VAR_ME":
            return me
        if term == "VAR_EGO":
            return EGO
        return None

    def effects(self, key: tuple, args: tuple = (), depth: int = 4, room: int | None = None,
                me: int | None = None) -> list[Effect]:
        """Every write ``key`` makes, following calls ``depth`` levels deep. Script numbers from
        200 up are the current room's local scripts; ``room`` is that room for a global. ``me``
        is VAR_ME, the object whose verb started the chain (scripts called from it see it too)."""
        memo = (key, args, depth, room, me)
        if memo in self._effects:
            return self._effects[memo]
        self._effects[memo] = []  # cycles end here
        h = self.handlers.get(key)
        if h is None:
            return []
        me = key[1] if key[0] == "obj" else me
        room = h.room if h.room is not None else room
        args = list(args)
        if key[0] == "obj" and not args:
            args = [OBJ2]  # a verb entry's Local[0] is the sentence's second object
        out: list[Effect] = []
        guards: list[list[str]] = []  # one entry per open block
        result = None  # what VAR_RESULT last read, so "VAR_RESULT == 15" reads "getObjectOwner(578) == 15"
        loops = _loops(h.lines)
        # Forward gotos: a branch that jumps into another branch's code (the fire's fuels all
        # "goto 01D7", the flaming-mass pickup) makes that code run under the jumper's guards.
        jumps: dict[int, list[tuple[str, ...]]] = {}
        active: list[tuple[int, tuple[str, ...]]] = []  # (guard depth at the target, jumper's guards)
        for offs, _depth, st in h.lines:
            where = f"{h.file} [{offs}]"
            here = int(offs, 16)
            for src in jumps.pop(here, [])[:MAX_JUMPS]:
                active.append((len(guards), src))
            if st.startswith("}") and not (_ELSE.match(st) or _ELSE_IF.match(st)):
                if guards:
                    guards.pop()
                active = [a for a in active if a[0] <= len(guards)]
                continue
            m = _GOTO.match(st)
            if m and int(m.group(1), 16) > here:
                jumps.setdefault(int(m.group(1), 16), []).append(tuple(t for entry in guards for t in entry))
                continue
            m = _ELSE_IF.match(st)
            if m:
                prev = guards.pop() if guards else []
                cond = m.group(1) if result is None else m.group(1).replace("VAR_RESULT", result)
                cond = self._resolve_terms(cond, tuple(args), me)
                guards.append(_negate_last(prev) + [cond])
                continue
            if _ELSE.match(st):
                prev = guards.pop() if guards else []
                guards.append(_negate_last(prev))
                continue
            g = tuple(t for entry in guards for t in entry)
            m = _RESULT.match(st)
            if m:
                result = self._resolve_terms(m.group(1), tuple(args), me)
                continue
            m = _IF.match(st)
            if m:
                cond = m.group(1)
                if result is not None:
                    cond = cond.replace("VAR_RESULT", result)
                guards.append([self._resolve_terms(cond, tuple(args), me)])
                continue
            m = _LOCAL.match(st)
            if m:  # Local[i] = value: bind it when it is still unknown (script 25's Local[0] = VAR_ME)
                i = int(m.group(1))
                while len(args) <= i:
                    args.append(None)
                if args[i] is None:
                    args[i] = self._value(m.group(2), tuple(args), me)
                continue
            loop = next((lp for lp in loops if lp[0] <= int(offs, 16) < lp[1] and f"Local[{lp[2]}]" in st), None)
            if loop is None:
                out.extend(self._statement_effects(st, tuple(args), me, g, where, depth, room))
                for d, src in active:
                    alt = src + tuple(t for entry in guards[d:] for t in entry)
                    out.extend(self._statement_effects(st, tuple(args), me, alt, where, depth, room))
            else:  # a counted loop (Local[i] = a .. b): the write happens for every value
                _, _, i, a, b = loop
                for value in range(a, min(b, a + 40) + 1):
                    bound = list(args) + [None] * max(0, i + 1 - len(args))
                    bound[i] = value
                    out.extend(self._statement_effects(st, tuple(bound), me, g, where, depth, room))
        uniq: dict[tuple, Effect] = {}
        for e in out:
            uniq.setdefault((e.kind, e.subject, e.value, e.guards, e.where), e)
        out = list(uniq.values())
        self._effects[memo] = out
        return out

    def _statement_effects(self, st, args, me, g, where, depth, room) -> list[Effect]:
        v = lambda t: self._value(t, args, me)  # noqa: E731
        out = []
        for kind, rx in _EFFECTS:
            m = rx.search(st)
            if not m:
                continue
            if kind in ("bit", "var"):
                offset = v(m.group(2)) if m.group(2) else 0
                index = int(m.group(1)) + offset if offset is not None else None
                if index is not None:
                    out.append(Effect(kind, index, v(m.group(3)), g, where))
            elif kind == "named":
                if m.group(1) in NAMED_VARS:
                    out.append(Effect("var", NAMED_VARS[m.group(1)], v(m.group(2)), g, where))
            elif kind == "var_any" and not _EFFECTS[1][1].search(st) and not _EFFECTS[2][1].search(st):
                out.append(Effect("var", int(m.group(1)), None, g, where, m.group(2) or m.group(3) or ""))
            elif kind == "pickup":
                out.append(Effect("owner", v(m.group(1)), EGO, g, where))
            elif kind == "owner":
                out.append(Effect("owner", v(m.group(1)), v(m.group(2)), g, where))
            elif kind == "state":
                out.append(Effect("state", v(m.group(1)), v(m.group(2)), g, where))
            elif kind == "draw":
                out.append(Effect("state", v(m.group(1)), 1, g, where))
            elif kind == "class":
                for c in m.group(2).split(","):
                    c = c.strip()
                    if re.fullmatch(r"\d+", c):
                        n = int(c)
                        out.append(Effect("class", v(m.group(1)), (n & 0x7F) if n & 0x80 else -n, g, where))
            elif kind == "room":
                out.append(Effect("room", v(m.group(1)), None, g, where))
                out.extend(self._entry(v(m.group(1)), g, depth))
            elif kind == "walk":  # a scripted walk of ego, as the archway between High Street's halves
                out.append(Effect("walk", None, int(m.group(1)) * 1000 + int(m.group(2)), g, where))
            elif kind == "room_ego":
                out.append(Effect("room", v(m.group(2)), None, g, where))
                out.extend(self._entry(v(m.group(2)), g, depth))
            elif kind == "room_put":
                out.append(Effect("room", v(m.group(1)), None, g, where))
            elif kind == "actor_room":
                out.append(Effect("actor_room", int(m.group(1)), v(m.group(2)), g, where))
            elif depth > 0 and kind == "start":
                call_args = tuple(self._arg(a.strip(), args, me) for a in m.group(2).split(",") if a.strip())
                n = int(m.group(1))
                out.append(Effect("script", n, 1, g, where))  # isScriptRunning(n) now holds
                if n < 200:
                    out.extend(Effect(e.kind, e.subject, e.value, _tail(g + e.guards), e.where)
                               for e in self.effects(("global", n), call_args, depth - 1, room, me))
                elif me in self.items and ("local", room, n) not in self.handlers:
                    # A carried object's local call runs in the room ego is in: every room
                    # with that script, each under its own VAR_ROOM guard.
                    for r in self.local_rooms.get(n, []):
                        out.extend(Effect(e.kind, e.subject, e.value, _tail(g + (f"VAR_ROOM == {r}",) + e.guards), e.where)
                                   for e in self.effects(("local", r, n), call_args, depth - 1, r, me))
                else:
                    out.extend(Effect(e.kind, e.subject, e.value, _tail(g + e.guards), e.where)
                               for e in self.effects(("local", room, n), call_args, depth - 1, room, me))
            elif depth > 0 and kind == "object":
                o, verb = v(m.group(1)), v(m.group(2))
                call_args = tuple(self._arg(a.strip(), args, me) for a in m.group(3).split(",") if a.strip())
                if o is not None and verb is not None:
                    out.extend(Effect(e.kind, e.subject, e.value, _tail(g + e.guards), e.where)
                               for e in self.effects(("obj", o, verb), call_args or (OBJ2,), depth - 1, room))
            elif depth > 0 and kind == "sentence":
                # doSentence(verb, object, second): the second object is the handler's Local[0]
                verb, o = v(m.group(1)), v(m.group(2))
                second = self._arg(m.group(3), args, me)
                if o is not None and verb is not None:
                    out.extend(Effect(e.kind, e.subject, e.value, _tail(g + e.guards), e.where)
                               for e in self.effects(("obj", o, verb), (second if second is not None else OBJ2,),
                                                     depth - 1, room))
            if kind in ("bit", "var", "named", "var_any"):
                break  # one assignment per statement
        return out

    def _entry(self, room: int | None, g: tuple, depth: int) -> list[Effect]:
        """A loaded room's entry script runs next: its writes are the load's too."""
        if room is None or depth <= 0:
            return []
        return [Effect(e.kind, e.subject, e.value, g + e.guards, e.where)
                for e in self.effects(("entry", room), (), depth - 1, room)]

    # --- queries --------------------------------------------------------------------------

    def actions(self) -> list[tuple]:
        """Every player action: ``("obj", object, verb)`` handlers."""
        return [k for k in self.handlers if k[0] == "obj"]

    def writers(self, kind: str, subject: int, value: int | None = None,
                exclude: tuple = ()) -> list[tuple[tuple, Effect]]:
        """Player actions whose effects write ``kind subject`` (= ``value`` when given, or a
        non-constant that might), skipping writes of the ``exclude`` values. One entry per
        action and second object (a pot handler answers each ingredient separately)."""
        out = []
        for key in self.actions():
            seen = set()
            for e in self.effects(key):
                if e.kind == kind and e.subject == subject and (value is None or e.value in (value, None)) \
                        and e.value not in exclude:
                    # One entry per second object and branch: both arms of "if the mass lies in
                    # the galley take it, else pick it up" give the item, under different guards.
                    if e.guards not in seen:
                        seen.add(e.guards)
                        out.append((key, e))
        return out

    def scripted_walks(self, room: int) -> list[int]:
        """Objects of ``room`` whose Walk to moves ego by script without leaving the room: the
        ways between parts of a room that a floor click cannot reach."""
        out = []
        for key in self.actions():
            _, obj, verb = key
            if verb != 11 or self.object_room.get(obj) != room:
                continue
            effs = self.effects(key, depth=1)
            if any(e.kind == "walk" for e in effs) and not any(e.kind == "room" for e in effs):
                out.append(obj)
        return out

    def exits(self) -> dict[int, list[tuple[int, int, int]]]:
        """``{room: [(object, target room, guards), ...]}``: Walk-to handlers that load another
        room, one entry per target, with the number of conditions guarding the load (an exit
        that only redirects under a flag counts its guard)."""
        graph: dict[int, list[tuple[int, int, int]]] = {}
        for key in self.actions():
            _, obj, verb = key
            # Walk to, or the default entry (255) when the object has no Walk to of its own.
            if verb != 11 and not (verb == 255 and ("obj", obj, 11) not in self.handlers):
                continue
            home = self.object_room.get(obj)
            targets: dict[tuple[int, int], int] = {}
            for e in self.effects(key, depth=1):
                if e.kind != "room" or e.subject is None:
                    continue
                # A pseudo-room (the forest's screens share room 58) is named by the innermost
                # positive "VAR_ROOM == N" guard: the edge starts there.
                screens = [int(m.group(1)) for g in e.guards if not g.startswith("!(")
                           for m in [_VAR_ROOM_GUARD.fullmatch(g)] if m]
                here = screens[-1] if screens else home
                if e.subject == here:
                    continue
                other = sum(1 for g in e.guards if not g.startswith("!(") and not _VAR_ROOM_GUARD.fullmatch(g))
                targets[(here, e.subject)] = min(targets.get((here, e.subject), 99), other)
            for (here, room), guards in targets.items():
                graph.setdefault(here, []).append((obj, room, guards))
        return graph
