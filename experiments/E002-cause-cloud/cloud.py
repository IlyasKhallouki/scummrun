"""E002 step 4: the cause cloud for MI1, every way each fact can become true.

usage: uv run python experiments/E002-cause-cloud/cloud.py   (after census.py)

A **cause** is a trigger (a root: something the player or the engine does that starts a
script), the chain of scripts it starts, and one write at the end of it. The roots come from
the engine trigger list (``results/engine-triggers.json``); none of them names an MI1 number:

* ``verb``: an object's verb handler, run when the player clicks a verb on it;
* ``room``: a room's entry and exit code, and the scripts installed in VAR_ENTRY_SCRIPT,
  VAR_ENTRY_SCRIPT2, VAR_EXIT_SCRIPT and VAR_EXIT_SCRIPT2, run on every room change;
* ``input``: the scripts installed in VAR_VERB_SCRIPT (clicks, keys, dialog lines),
  VAR_SENTENCE_SCRIPT and VAR_INVENTORY_SCRIPT;
* ``hook``: the scripts installed in VAR_CUTSCENE_START_SCRIPT, VAR_CUTSCENE_END_SCRIPT and
  VAR_SCROLL_SCRIPT;
* ``boot``: the boot script (global 1) and everything it starts;
* ``engine``: game variables the engine writes by itself (timers, mouse, room), no script.

A hook "installed" means every constant any script assigns to that variable: the game swaps
its input script for dialogs, so all of them count. Local scripts (200 and up) are looked
up in the room whose code installs them, or in every room for a room hook.

The writes are ``ScriptIndex``'s (bit, var, owner, state, class, room, actor room, scripted
walk, script start) plus the kinds the engine table says a script can read back or the engine
checks before it takes player input (``_EXTRA``; ``TABLE_KIND`` gives each the table's name).
Kinds the table marks as never read back (object names, palettes, resources) are not facts.

Per fact the cloud counts its **writers** (the script lines that write it) and its **triggers**
(the roots that reach one of them; a global hook is one trigger, whichever room it runs in).
A key fact has a cause if some writer writes its value, or a value known only at run time;
``exact`` counts the first kind only.

Reading rules on top of ScriptIndex, all general:

* a write through a variable index (``Bit[140 + Var[240]] = 1``) is a write to every index the
  index variable can hold: 0 up to the largest constant any script assigns or compares it with
  (a local: in the same script), at most ``MAX_INDEX``;
* a room's local script (200 and up) called where the caller's room is not known, or has no such
  script, is a call to that script in every room that has it: the engine only runs a local
  script of the current room, so the script must have changed rooms first (loadRoom);
* a script started by a number held in a variable (``startScript(Local[1],...)``, the number
  passed in by the caller, or ``startScript(Var[116],...)``) is the bound value if the caller gave
  one, else every constant any script assigns to that variable (a local: in the same file);
* a script can change rooms (``loadRoom(r)``, ``loadRoomWithEgo(..., r, ...)``): it keeps the set of
  rooms it may be in, its own room plus every room it has loaded so far. Rooms are never taken out
  of the set, because code after a load can be reached by a jump from before it. Its local-script
  calls go to every room in the set that has the script; a global it calls with more than one
  possible room gets none, so that global's local calls go to every room with them;
* ``Resource.loadRoom(n)`` only loads a room's data into memory; it is not a room change
  (ScriptIndex's pattern reads it as one);
* ``startScript(n, [...], F, R)`` with two flags is a call like any other (ScriptIndex's pattern
  takes one flag at most);
* a write whose value is an expression (``Var[n] = (1 + getRandomNr(3))``) writes an unknown value.

Writes ``results/cloud.json`` and prints the growth table and the Part I coverage.
"""

import collections
import json
import re
import statistics
import sys
from pathlib import Path

from census import HERE, SCRIPTS
from speedrun import paths
from speedrun.scriptindex import _LINE, _TERM, EGO, NAMED_VARS, Effect, ScriptIndex

sys.path.insert(0, str(paths.ROOT / "experiments" / "E001-goals-from-scripts"))
from answer_key import classify  # noqa: E402
from score import load_key  # noqa: E402

HOOKS = {
    "VAR_ENTRY_SCRIPT": "room", "VAR_ENTRY_SCRIPT2": "room", "VAR_EXIT_SCRIPT": "room", "VAR_EXIT_SCRIPT2": "room",
    "VAR_VERB_SCRIPT": "input", "VAR_SENTENCE_SCRIPT": "input", "VAR_INVENTORY_SCRIPT": "input",
    "VAR_CUTSCENE_START_SCRIPT": "hook", "VAR_CUTSCENE_END_SCRIPT": "hook", "VAR_SCROLL_SCRIPT": "hook",
}
ROOTS = ("verb", "room", "input", "hook", "boot", "engine")
OLD_KINDS = {"bit", "var", "owner", "state", "class", "room", "actor_room", "walk", "script"}
KEY_KINDS = {"bit", "var", "owner", "owner_not", "state", "class", "script"}  # the kinds E001's key holds

# The writes ScriptIndex does not read, by the engine table's kind (results/engine-opcodes.json).
# Each pattern takes the subject first; the value is a constant where the script gives one.
_EXTRA = [
    ("actor_position", re.compile(r"(?:putActor|walkActorTo|walkActorToObject|walkActorToActor|putActorAtObject)\("
                                  + _TERM + r",")),
    ("actor_costume", re.compile(r"ActorOps\(" + _TERM + r",\[.*?\bCostume\(" + _TERM + r"\)")),
    ("actor_facing", re.compile(r"faceActor\(" + _TERM + r",")),
    ("script_stop", re.compile(r"stopScript\((\d+)\)")),
    ("var_range", re.compile(r"setVarRange\((Var|Bit)\[(\d+)\],(\d+),\[([^\]]*)\]\)")),
    ("var_muldiv", re.compile(r"^Var\[(\d+)\] (?:\*=|/=) ")),
    ("walk_boxes", re.compile(r"setBoxFlags\(" + _TERM + r"," + _TERM + r"\)")),
    ("input_enable", re.compile(r"^Userput(Soft)?(On|Off)\(\)")),
    ("sound", re.compile(r"startSound\(" + _TERM + r"\)")),
    ("string", re.compile(r"(?:CreateString|CopyString|SetStringChar|PutCodeInString)\(" + _TERM + r",")),
    ("verb_ui", re.compile(r"VerbOps\(" + _TERM + r",\[(.*)\]\)")),
]
# The fact kind each cloud kind is, in the engine table's words (for the coverage check).
TABLE_KIND = {"bit": "bit", "var": "var", "owner": "object_owner", "state": "object_state", "class": "object_class",
              "room": "room", "actor_room": "actor_room", "walk": "actor_position", "script": "script_lifecycle",
              "actor_position": "actor_position", "actor_costume": "actor_costume", "actor_facing": "actor_facing",
              "walk_boxes": "walk_boxes", "input_enable": "input_enable", "sound": "sound", "string": "string",
              "verb_ui": "verb_ui"}
# Engine checks before it takes input: walk boxes (where ego can walk), userput (whether clicks
# reach the scripts at all) and verbs (which dialog lines and verbs can be clicked). No opcode
# reads them back, but they decide which player actions exist.
ENGINE_GATES = {"walk_boxes", "input_enable", "verb_ui"}
MAX_INDEX = 64
ROOT_KEYS: list = []
_INDEXED = re.compile(r"^(Var|Bit)\[(\d+)(?: \+ (-?\d+|Local\[\d+\]|Var\[\d+\]))?\]"
                      r"(?: (?:=|\+=|-=|\*=|/=) (.*)|(\+\+|--));$")
_TWO_FLAG_START = re.compile(r"(?:startScript|chainScript)\((\d+),\[(.*)\],\w+,\w+\);")
_LOAD_ROOM = re.compile(r"(?<![.\w])loadRoom\((\d+)\)|loadRoomWithEgo\([^,]+,(\d+),")
_DYNAMIC_START = re.compile(r"(?:startScript|chainScript)\((Local\[\d+\]|Var\[\d+\]),\[(.*)\](?:,\w+)*\);")
_ASSIGN = re.compile(r"((?:Local|Var)\[\d+\]) = (-?\d+);")
_CONSTANT_USE = re.compile(r"((?:Local|Var)\[\d+\]) (?:=|==|!=|<|<=|>|>=) (-?\d+)\b")


class CloudIndex(ScriptIndex):
    """ScriptIndex with the extra write kinds. It reads one script context at a time: a call
    (startScript, startObject, doSentence, a room load's entry code) is recorded as an edge to
    the callee's context instead of being read inline, so each context is read once."""

    extra = True

    def __init__(self, scripts_dir: Path):
        super().__init__(scripts_dir)
        self.edges: dict[tuple, set[tuple]] = collections.defaultdict(set)
        self._stack: list[tuple] = []
        self._rooms: list[set[int]] = []  # the rooms each context on the stack may be in now

    def effects(self, key, args=(), depth=4, room=None, me=None):
        if depth == 0 and self._stack:  # a call from the context being read
            if key[0] == "local" and (key[1] is None or key not in self.handlers):
                here = [r for r in self._rooms[-1] if ("local", r, key[2]) in self.handlers]
                for r in here or self.local_rooms.get(key[2], []):  # else: every room with it
                    self.edges[self._stack[-1]].add((("local", r, key[2]), tuple(args), r, me))
            else:
                self.edges[self._stack[-1]].add((key, tuple(args), room, me))
            return []
        h = self.handlers.get(key)
        self._stack.append((key, tuple(args), room, me))
        start = h.room if h is not None and h.room is not None else room
        self._rooms.append({start} if start is not None else set())
        try:
            return super().effects(key, args, depth, room, me)
        finally:
            self._stack.pop()
            self._rooms.pop()

    def index_values(self, term: str, where: str) -> range:
        """What an index variable can hold: 0 up to the largest constant assigned to or compared
        with it (a local: in the same file), at most MAX_INDEX."""
        if not hasattr(self, "_consts"):
            self._consts = collections.defaultdict(set)  # ("Var[n]", None) or ("Local[i]", file) -> constants
            for path in sorted(self.dir.rglob("*.txt")):
                f = str(path.relative_to(self.dir))
                for raw in path.read_text().splitlines():
                    for t, c in _CONSTANT_USE.findall(raw):
                        self._consts[(t, None if t.startswith("Var") else f)].add(int(c))
        cs = self._consts.get((term, None if term.startswith("Var") else where.split(" [")[0]), set())
        top = max((c for c in cs if c >= 0), default=0)
        return range(min(top, MAX_INDEX) + 1)

    def assigned(self, term: str, where: str) -> set[int]:
        """The constants any script assigns to ``term`` (a local: in the same file)."""
        if not hasattr(self, "_assigned"):
            self._assigned = collections.defaultdict(set)
            for path in sorted(self.dir.rglob("*.txt")):
                f = str(path.relative_to(self.dir))
                for raw in path.read_text().splitlines():
                    for t, c in _ASSIGN.findall(raw):
                        self._assigned[(t, None if t.startswith("Var") else f)].add(int(c))
        return self._assigned.get((term, None if term.startswith("Var") else where.split(" [")[0]), set())

    def own(self, ctx: tuple) -> list[Effect]:
        """The writes ``ctx`` makes itself; its calls land in ``edges[ctx]``."""
        key, args, room, me = ctx
        return self.effects(key, args, 1, room, me)

    def _statement_effects(self, st, args, me, g, where, depth, room):
        if st.startswith("Resource."):  # memory management: no game state
            return []
        if self._rooms:
            rooms = self._rooms[-1]
            room = next(iter(rooms)) if len(rooms) == 1 else None
        out = super()._statement_effects(st, args, me, g, where, depth, room)
        lr = _LOAD_ROOM.search(st)
        if lr and self._rooms:
            self._rooms[-1].add(int(lr[1] or lr[2]))
        if not self.extra:
            return out
        v = lambda t: self._value(t, args, me)  # noqa: E731
        m = _INDEXED.match(st)
        if m and not any(e.kind in ("bit", "var") for e in out):  # ScriptIndex read nothing from it
            base, index, rhs = int(m[2]), m[3], m[4]
            value = v(rhs) if rhs is not None and re.fullmatch(_TERM, rhs) and not m[5] else None
            if index is None:
                offsets = [0]
            elif v(index) is not None:
                offsets = [v(index)]
            else:
                offsets = list(self.index_values(index, where))
            for k in offsets:
                out.append(Effect(m[1].lower(), base + k, value, g, where))
        m = _TWO_FLAG_START.search(st)
        if m and depth > 0:
            n = int(m[1])
            call_args = tuple(self._arg(a.strip(), args, me) for a in m[2].split(",") if a.strip())
            out.append(Effect("script", n, 1, g, where))
            self.effects(("global", n) if n < 200 else ("local", room, n), call_args, depth - 1, room, me)
        m = _DYNAMIC_START.search(st)
        if m and depth > 0:
            bound = v(m[1])
            targets = {bound} if bound is not None else self.assigned(m[1], where)
            call_args = tuple(self._arg(a.strip(), args, me) for a in m[2].split(",") if a.strip())
            for n in sorted(t for t in targets if t and t > 0):
                out.append(Effect("script", n, 1, g, where))
                key = ("global", n) if n < 200 else ("local", room, n)
                self.effects(key, call_args, depth - 1, room, me)  # recorded as a call edge
        for kind, rx in _EXTRA:
            m = rx.search(st)
            if not m:
                continue
            if kind == "actor_position":
                out.append(Effect("actor_position", v(m[1]), None, g, where))
            elif kind == "actor_costume":
                out.append(Effect("actor_costume", v(m[1]), v(m[2]), g, where))
            elif kind == "actor_facing":
                out.append(Effect("actor_facing", v(m[1]), None, g, where))
            elif kind == "script_stop":
                out.append(Effect("script", int(m[1]), 0, g, where))
            elif kind == "var_range":
                values = [x.strip() for x in m[4].split(",")]
                for i, x in enumerate(values[: int(m[3])]):
                    out.append(Effect(m[1].lower(), int(m[2]) + i, v(x), g, where))
            elif kind == "var_muldiv":
                out.append(Effect("var", int(m[1]), None, g, where))
            elif kind == "walk_boxes":
                out.append(Effect("walk_boxes", v(m[1]), v(m[2]), g, where))
            elif kind == "input_enable":
                out.append(Effect("input_enable", None, int(m[2] == "On"), g, where))
            elif kind == "sound":
                out.append(Effect("sound", v(m[1]), 1, g, where))
            elif kind == "string":
                out.append(Effect("string", v(m[1]), None, g, where))
            elif kind == "verb_ui":
                ops = m[2]
                value = 0 if re.search(r"\b(?:Off|Delete)\(\)", ops) else 1 if re.search(r"\bOn\(\)", ops) else None
                out.append(Effect("verb_ui", v(m[1]), value, g, where))
        return out


def installed(scripts: Path) -> dict[str, set[tuple[int, int | None]]]:
    """Hook var -> every (script, room) any script assigns to it; room is None for a global."""
    out = collections.defaultdict(set)
    for path in sorted(scripts.rglob("*.txt")):
        room = int(path.parent.name.split("-")[1]) if path.parent.name.startswith("room-") else None
        for raw in path.read_text().splitlines():
            m = _LINE.match(raw)
            st = m and m[4].strip()
            h = st and re.fullmatch(r"(VAR_[A-Z0-9_]+) = (\d+);", st)
            if h and h[1] in HOOKS:
                n = int(h[2])
                if n:
                    out[h[1]].add((n, room if n >= 200 else None))
    return out


def engine_writes() -> set[int]:
    """Game vars the v5 engine writes by itself in MI1, from the trigger list's checked sites."""
    sites = json.loads((HERE / "results" / "engine-triggers.json").read_text())["sites"]
    out = set()
    for s in sites:
        if s["label"] == "generic_v5" and s["applies_to_mi1_dos_cd"]:
            for name in re.findall(r"VAR_[A-Z0-9_]+", s["starts_or_writes"]):
                if name in NAMED_VARS:
                    out.add(NAMED_VARS[name])
    return out


def facts_of(e: Effect) -> list[tuple]:
    if e.subject is None and e.kind not in ("input_enable", "walk"):
        return []
    q = [(e.kind, e.subject, e.value)]
    if e.kind == "owner" and e.value != EGO:
        q.append(("owner_not", e.subject, EGO))
    return q


def roots(ix: CloudIndex, kinds: tuple[str, ...]) -> list[tuple[str, tuple, int | None]]:
    """(root kind, handler, room) for the chosen root kinds."""
    hooks = installed(ix.dir)
    rooms = sorted({k[1] for k in ix.handlers if k[0] in ("entry", "exit", "local")})
    out = []
    if "verb" in kinds:
        out += [("verb", k, None) for k in ix.actions()]
    if "room" in kinds:
        out += [("room", (kind, r), r) for r in rooms for kind in ("entry", "exit") if (kind, r) in ix.handlers]
    for var, kind in HOOKS.items():
        if kind in kinds and kind != "verb":
            for n, home in sorted(hooks.get(var, ()), key=lambda t: (t[0], t[1] or 0)):
                if n < 200:  # a global hook runs in whatever room ego is in
                    out += [(kind, ("global", n), r) for r in rooms]
                else:
                    out.append((kind, ("local", home, n), home))
    if "boot" in kinds:
        out.append(("boot", ("global", 1), None))
    return out


def graph(ix: CloudIndex) -> tuple[list, dict, dict, dict]:
    """Every context reachable from a root: its own writes, its calls, and its distance from the
    nearest root. Roots get one bit each; ``reach[ctx]`` holds the bits of the roots that get there."""
    rs = roots(ix, ROOTS)
    ROOT_KEYS[:] = [key for _, key, _ in rs]  # a global hook is one trigger, whatever the room
    reach, dist, writes = {}, {}, {}
    queue = collections.deque()
    for i, (kind, key, room) in enumerate(rs):
        ctx = (key, (), room, None)
        reach[ctx] = reach.get(ctx, 0) | (1 << i)
        if ctx not in dist:
            dist[ctx] = 0
            queue.append(ctx)
    while queue:  # distances and the context list
        ctx = queue.popleft()
        writes[ctx] = ix.own(ctx)
        for child in ix.edges[ctx]:
            if child not in dist:
                dist[child] = dist[ctx] + 1
                queue.append(child)
    changed = collections.deque(reach)  # root bits along the calls, to a fixed point
    while changed:
        ctx = changed.popleft()
        for child in ix.edges[ctx]:
            new = reach.get(child, 0) | reach[ctx]
            if new != reach.get(child, 0):
                reach[child] = new
                changed.append(child)
    return rs, reach, dist, writes


def build(g: tuple, kinds: tuple[str, ...], depth: int, extra: bool) -> dict:
    """fact -> {(root kind, root index, write location)} for the chosen root kinds, contexts at most
    ``depth`` calls from a root, and the old or the full set of write kinds."""
    rs, reach, dist, writes = g
    mask = sum(1 << i for i, (kind, _, _) in enumerate(rs) if kind in kinds)
    causes = collections.defaultdict(set)
    for ctx, effs in writes.items():
        bits = reach.get(ctx, 0) & mask
        if not bits or dist[ctx] > depth:
            continue
        idx = [i for i in range(bits.bit_length()) if bits >> i & 1]
        for e in effs:
            if not extra and e.kind not in OLD_KINDS:
                continue
            for q in facts_of(e):
                causes[q].update((rs[i][0], i, e.where) for i in idx)
    if "engine" in kinds:
        for n in engine_writes():
            causes[("var", n, None)].add(("engine", -1, "engine"))
    return causes


def matches(q: tuple, causes: dict) -> set:
    """Causes that can make key fact ``q`` true: the same value, or a value only known at run time."""
    kind, n, v = q
    return causes.get(q, set()) | causes.get((kind, n, None), set())


def summary(causes: dict, key: list[dict]) -> dict:
    writers = [len({w for _, _, w in cs}) for cs in causes.values()]
    triggers = [len({ROOT_KEYS[i] if i >= 0 else "engine" for _, i, _ in cs}) for cs in causes.values()]
    capped = [q for q in causes if q[0] in KEY_KINDS and classify(q) is None]
    parts = ("I", "II", "III", "IV")
    hit = {p: sum(bool(matches(tuple(f["fact"]), causes)) for f in key if f["part"] == p) for p in parts}
    exact = {p: sum(bool(causes.get(tuple(f["fact"]))) for f in key if f["part"] == p) for p in parts}
    return {"facts": len(causes), "key_kind_facts": len(capped),
            "triggers_per_fact": {"mean": round(statistics.mean(triggers), 1), "median": statistics.median(triggers),
                                  "max": max(triggers)},
            "writers_per_fact": {"mean": round(statistics.mean(writers), 2), "median": statistics.median(writers),
                                 "max": max(writers)},
            "by_kind": dict(collections.Counter(q[0] for q in causes).most_common()),
            "key_goals_with_a_cause": hit, "key_goals_exact": exact}


def coverage(causes: dict, key: list[dict]) -> dict:
    """Part I only: the root kinds that can produce each goal."""
    rows, reach, only = [], collections.Counter(), collections.Counter()
    for f in key:
        if f["part"] != "I":
            continue
        ks = sorted({k for k, _, _ in matches(tuple(f["fact"]), causes)}, key=ROOTS.index)
        rows.append({"fact": f["fact"], "means": f["means"], "move": f["move"], "roots": ks,
                     "exact": bool(causes.get(tuple(f["fact"])))})
        reach.update(ks)
        if len(ks) == 1:
            only[ks[0]] += 1
    return {"goals": rows, "reached_by": dict(reach), "only_by": dict(only),
            "no_cause": [r for r in rows if not r["roots"]]}


def line_coverage(ix: CloudIndex) -> dict:
    """Per o5 function that writes a kind the cloud tracks (by the engine table): its script lines,
    how many the extractor reads a fact from, how many only with the subject unknown (an object or
    actor given by a variable), how many it misses, and the tracked kinds the table lists for the
    function that the extractor never produces from it (side effects it does not model).

    Not counted: lines whose only target is scratch (a Local or VAR_RESULT) or an engine VAR_
    (the engine root's), and a script ending itself (stopObjectCode, cutscene, override)."""
    table = json.loads((HERE / "results" / "engine-opcodes.json").read_text())["rows"]
    tracked = set(TABLE_KIND.values())
    kinds_of = {}
    for r in table:
        ks = {w["kind"] for w in r["writes"] if w["read_back_by"] or w["kind"] in ENGINE_GATES} & tracked
        kinds_of.setdefault(r["function"], {})[(r["subop"] or "").lower()] = ks
    from census import opcode_table
    ops = opcode_table()
    tally = collections.defaultdict(lambda: {"lines": 0, "read": 0, "subject_unknown": 0, "missed": 0,
                                             "table_kinds": set(), "read_kinds": set(), "examples": []})
    for path in sorted(SCRIPTS.rglob("*.txt")):
        for raw in path.read_text().splitlines():
            m = _LINE.match(raw)
            if not m or m[2] == "**":
                continue
            fn = ops[int(m[2], 16)][0]
            st = m[4].strip().removeprefix("Exprmode ")
            subs = kinds_of.get(fn, {})
            if "" in subs:
                ks = set(subs[""])
            else:
                names = {x.lower() for x in re.findall(r"(\w+)\(", st)}
                ks = set().union(*(k for sub, k in subs.items() if sub in names))
            if fn in ("o5_stopObjectCode", "o5_cutscene", "o5_endCutscene", "o5_beginOverride"):
                ks.discard("script_lifecycle")
            if not ks or re.match(r"^(Local\[\d+\]|VAR_[A-Z_0-9]+)( |\+\+|--)", st):
                continue
            effs = [e for e in ix._statement_effects(st, (), None, (), str(path.relative_to(SCRIPTS)) + " [0000]", 1, None)
                    if e.kind != "walk"]
            d = tally[fn]
            d["lines"] += 1
            d["table_kinds"] |= ks
            d["read_kinds"] |= {TABLE_KIND.get(e.kind, e.kind) for e in effs}
            if any(e.subject is not None or e.kind == "input_enable" for e in effs):
                d["read"] += 1
            elif effs:
                d["subject_unknown"] += 1
            else:
                d["missed"] += 1
                if len(d["examples"]) < 2:
                    d["examples"].append(st[:80])
    out = {}
    for fn, d in sorted(tally.items(), key=lambda kv: -kv[1]["lines"]):
        out[fn] = {k: d[k] for k in ("lines", "read", "subject_unknown", "missed", "examples")}
        out[fn]["side_effects_not_modelled"] = sorted(d["table_kinds"] - d["read_kinds"])
    return out


def main() -> None:
    key = load_key()
    steps = [
        ("verb handlers, calls 4 deep, ScriptIndex's kinds", ("verb",), 4, False),
        ("+ the engine table's other kinds", ("verb",), 4, True),
        ("+ calls followed to the end", ("verb",), 64, True),
        ("+ room entry and exit", ("verb", "room"), 64, True),
        ("+ input scripts (clicks, keys, dialog)", ("verb", "room", "input"), 64, True),
        ("+ cutscene and scroll hooks", ("verb", "room", "input", "hook"), 64, True),
        ("+ the boot script", ("verb", "room", "input", "hook", "boot"), 64, True),
        ("+ engine-written variables", ROOTS, 64, True),
    ]
    ix = CloudIndex(SCRIPTS)
    g = graph(ix)
    rows = []
    for name, kinds, depth, extra in steps:
        causes = build(g, kinds, depth, extra)
        rows.append({"step": name, **summary(causes, key)})
        r = rows[-1]
        print(f"{name:48} facts {r['facts']:6}  key-kind {r['key_kind_facts']:5}  "
              f"writers/fact {r['writers_per_fact']['mean']:5} (median {r['writers_per_fact']['median']}, "
              f"max {r['writers_per_fact']['max']})  triggers/fact {r['triggers_per_fact']['mean']} "
              f"(median {r['triggers_per_fact']['median']})  goals I {r['key_goals_with_a_cause']['I']}/99 "
              f"(exact {r['key_goals_exact']['I']})  II-IV {sum(r['key_goals_with_a_cause'][p] for p in ('II', 'III', 'IV'))}/71 "
              f"(exact {sum(r['key_goals_exact'][p] for p in ('II', 'III', 'IV'))})")
    depth = {d: len(build(g, ROOTS, d, True)) for d in (0, 1, 2, 4, 8, 16, 32, 64)}
    print("facts by call depth:", depth, " deepest context:", max(g[2].values()))
    visited = {ctx[0] for ctx in g[3]}
    unreached = sorted(k for k in ix.handlers if k[0] in ("global", "local") and k not in visited)
    cov = coverage(causes, key)
    lines = line_coverage(ix)
    out = {"growth": rows, "facts_by_depth": depth, "part1": cov, "line_coverage": lines,
           "unreached_scripts": [list(k) for k in unreached]}
    (HERE / "results" / "cloud.json").write_text(json.dumps(out, indent=1) + "\n")
    print(f"Part I goals reached by: {cov['reached_by']}  only by: {cov['only_by']}  no cause: {len(cov['no_cause'])}")
    for r in cov["no_cause"]:
        print("   no cause:", r["fact"], r["means"], "|", r["move"][:60])
    print(f"scripts no root reaches: {len(unreached)}")
    print("functions writing a tracked kind: lines read / subject unknown / missed of lines; side effects not modelled")
    for fn, d in lines.items():
        print(f"   {fn:24} {d['read']:5} / {d['subject_unknown']:4} / {d['missed']:4} of {d['lines']:5}  "
              f"{','.join(d['side_effects_not_modelled']) or '-'}  {'; '.join(d['examples'])}")


if __name__ == "__main__":
    main()
