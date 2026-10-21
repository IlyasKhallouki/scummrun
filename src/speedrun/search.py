"""Find the inputs that reach a milestone, instead of writing them by hand.

A milestone is a set of persistent facts (room, item held, owner, bit, object
state), taken from the route files' ``until`` conditions. From a fork of the
current engine state the search tries candidate moves and keeps the fastest
branch (in quarter-frames, the RTA clock) on which the milestone holds:

* **who writes the fact**: the player actions whose handlers (followed through
  the scripts they start) write a missing fact (``speedrun.scriptindex``);
* **how to get there**: the next hop along the room graph built from the exit
  handlers, when the action's object is in another room;
* **what is on screen**: every visible dialogue line when a menu is up.

Each candidate runs on its own fork through the executor (which already
handles approaching, inventory scrolling, the hover verb swap and delay
steering) until the milestone, the candidate's own expected fact, or the
bridge's idle rule holds. Branches are deduplicated by room, inventory, bits
and object states; the search is best-first on the RTA clock and bounded.
"""

import re
from collections import deque
from dataclasses import dataclass, field, replace

from speedrun.control import ControlError
from speedrun.executor import DIALOG_FIRST, DIALOG_LAST, Executor, Move, RouteError
from speedrun.scriptindex import EGO, ScriptIndex

PERSISTENT = ("room", "has", "owner", "bit", "state", "var", "actor_room", "class", "script")
# Vars that hold the moment, not progress: the message flag (3), the talking actor (25), the
# sentence's objects and verb (103, 104, 107-110), the default verb (182), the dialogue choice
# (194), the userput and cutscene counters (53, 352), the inventory view row (118), the hover's saved verb (394), the override
# flag (5) and scratch var 100.
TRANSIENT_VARS = {3, 5, 25, 53, 100, 103, 104, 107, 108, 109, 110, 118, 182, 194, 352, 394}
GRACE = 60  # frames run past an idle stop before a move counts as doing nothing
MENU_REPEATS = 4  # times the same dialogue menu may come back within one milestone
STEER_DELAYS = 60  # frames of delay tried when a random outcome is wrong (each frame draws the RNG)
PLAYER_VERBS = {2, 3, 5, 6, 7, 8, 9, 10, 11}  # verb 4 (Give) reaches objects as their verb 80
_OBJ2 = re.compile(r"(?:Local\[0\]|OBJ2) == (\d+)")
WAIT = Move("auto: wait until idle")
VAR_ROOM = 4  # the room number scripts see; the forest screen inside room 58
PREVIOUS_ROOM = 101  # Var[101], set to the room being left by global 7 (global/script-007.txt [0000])
ESC = Move("auto: Esc", key="esc")
STAND = Move("auto: stand and wait")  # for something that comes by itself (a pirate on the map)
STAND_FRAMES = 240
SUBGOAL_DEPTH = 3  # levels of "what does this action need first"
MAX_CANDIDATES = 24
MAX_FRONTIER = 16  # live branches kept between expansions
H_COST = 1200  # quarter-frames (5 s) charged per missing fact when ranking branches


def persistent(conds: list[dict]) -> list[dict]:
    """The facts of a C3 list that stay true once reached (no transient vars or positions)."""
    out = []
    for c in conds:
        inner = c["not"] if "not" in c else c
        k = next(iter(inner))
        if k in PERSISTENT and not (k == "var" and inner["var"] in TRANSIENT_VARS):
            out.append(c)
    return out


_GUARD_FORMS = [
    (re.compile(r"getObjectOwner\((\d+)\) == (\d+)"),
     lambda m: {"has": int(m[1])} if int(m[2]) == EGO else {"owner": int(m[1]), "eq": int(m[2])}),
    (re.compile(r"getObjectState\((\d+)\) == (\d+)"), lambda m: {"state": int(m[1]), "eq": int(m[2])}),
    (re.compile(r"Bit\[(\d+)\]"), lambda m: {"bit": int(m[1]), "eq": 1}),
    (re.compile(r"!Bit\[(\d+)\]"), lambda m: {"bit": int(m[1]), "eq": 0}),
    (re.compile(r"Var\[(\d+)\] == (-?\d+)"), lambda m: {"var": int(m[1]), "eq": int(m[2])}),
    (re.compile(r"Var\[(\d+)\] >= (-?\d+)"), lambda m: {"var": int(m[1]), "ge": int(m[2])}),
    (re.compile(r"Var\[(\d+)\] <= (-?\d+)"), lambda m: {"var": int(m[1]), "le": int(m[2])}),
    (re.compile(r"Var\[(\d+)\] > (-?\d+)"), lambda m: {"var": int(m[1]), "ge": int(m[2]) + 1}),
    (re.compile(r"Var\[(\d+)\] < (-?\d+)"), lambda m: {"var": int(m[1]), "le": int(m[2]) - 1}),
    (re.compile(r"VAR_ROOM == (\d+)"), lambda m: {"var": VAR_ROOM, "eq": int(m[1])}),  # be in that room
    (re.compile(r"isScriptRunning\((\d+)\)"), lambda m: {"script": int(m[1])}),  # the lit fuse (local 251)
    (re.compile(r"!isScriptRunning\((\d+)\)"), lambda m: {"not": {"script": int(m[1])}}),
    (re.compile(r"classOfIs\((\d+),\[(\d+)\]\)"),
     lambda m: {"class": int(m[1]), "set": int(m[2]) & 0x7F} if int(m[2]) & 0x80
     else {"not": {"class": int(m[1]), "set": int(m[2])}}),
]


def guard_facts(guards: tuple[str, ...]) -> list[dict]:
    """The script conditions around a write that read as C3 facts (simple ones only)."""
    out = []
    for g in guards:
        neg = g.startswith("!(") and g.endswith(")") and "&&" not in g and "||" not in g
        inner = g[2:-1] if neg else g
        for rx, make in _GUARD_FORMS:
            m = rx.fullmatch(inner)
            if m:
                c = make(m)
                out.append({"not": c} if neg and "not" not in c else (c["not"] if neg else c))
                break
    return out




def _query(cond: dict) -> tuple | None:
    if "not" in cond:
        inner = cond["not"]
        if "class" in inner:  # class n cleared: the index writes -n
            return ("class", inner["class"], -inner["set"])
        if "has" in inner:  # given away or used up: any owner but ego
            return ("owner_not", inner["has"], EGO)
        return None
    k = next(iter(cond))
    if k == "class":
        return ("class", cond["class"], cond["set"])
    if k == "has":
        return ("owner", cond["has"], EGO)
    if k in ("owner", "bit", "state") and "eq" in cond:
        return (k, cond[k], cond["eq"])
    if k == "var" and "eq" in cond:
        return ("var", cond["var"], cond["eq"])
    if k == "var" and ("ge" in cond or "le" in cond):
        return ("var", cond["var"], None)  # a counter: any write may be progress
    if k == "actor_room" and "eq" in cond:
        return ("actor_room", cond["actor_room"], cond["eq"])
    if k == "script":
        return ("script", cond["script"], 1)
    return None


@dataclass
class Node:
    ex: Executor
    moves: list[Move]
    qf: int
    depth: int
    unmet: int = 0  # target facts still missing (a bit-sum counter counts its bits)

    @property
    def priority(self) -> int:
        return self.qf + H_COST * self.unmet


@dataclass
class SearchResult:
    ex: Executor  # the winning branch, its movie and results
    moves: list[Move]
    qf: int
    tried: int


@dataclass
class MilestoneSearch:
    index: ScriptIndex
    fork: object  # Controller.fork
    release: str = "dos-cd"
    max_depth: int = 12
    max_nodes: int = 150
    window: int = 900  # frames a candidate may take
    log: list[str] = field(default_factory=list)
    verbose: bool = False  # a progress line per expanded node

    def __post_init__(self):
        self.graph = self.index.exits()

    # --- candidates -----------------------------------------------------------------------

    def _hops(self, here: int, there: int) -> list[tuple[int, int]]:
        """The exits to take first on a shortest way from ``here`` to ``there``:
        ``[(object, next room), ...]``, least-guarded first (BFS on the exit graph)."""
        dist = {there: 0}
        q = deque([there])
        back: dict[int, list[int]] = {}
        for r, edges in self.graph.items():
            for _, nxt, _ in edges:
                back.setdefault(nxt, []).append(r)
        while q:
            r = q.popleft()
            for p in back.get(r, []):
                if p not in dist:
                    dist[p] = dist[r] + 1
                    q.append(p)
        if here not in dist or here == there:
            return []
        hops = [(g, obj, nxt) for obj, nxt, g in self.graph.get(here, []) if dist.get(nxt) == dist[here] - 1]
        return [(obj, nxt) for g, obj, nxt in sorted(hops)][:2]

    def _actions_for(self, query: tuple) -> list[tuple[Move, list[dict], object]]:
        """Player moves whose handler writes ``query``, each with the condition it should make true."""
        out = []
        if query[0] == "var" and query[1] == PREVIOUS_ROOM:
            # Global 7 runs as every room's exit script and sets Var[101] = VAR_ROOM: "the last
            # room was R" is reached by any action that visits R (the forest map teleport).
            query = ("room", query[2], None)
        if query[0] == "owner_not":
            found = self.index.writers("owner", query[1], None, exclude=(EGO,))
        else:
            found = self.index.writers(*query)
        for (_, obj, verb), eff in found:
            # The second object: the innermost positive "Local[0] == N" around the write (negated
            # earlier branches of an else-if chain name the other objects).
            hits = [_OBJ2.fullmatch(g) for g in eff.guards if not g.startswith("!(")]
            hits = [h for h in hits if h]
            other = int(hits[-1].group(1)) if hits else None
            kind, subject, value = query
            if kind == "room":
                expect = [{"room": subject}]
            elif kind == "owner_not":
                expect = [{"not": {"has": subject}}]
            elif kind == "actor_room":
                expect = [{"actor_room": subject, "eq": value}]
            elif kind == "script":
                expect = [{"script": subject}]
            elif kind == "owner" and value == EGO:
                expect = [{"has": subject}]
            elif kind == "class":
                expect = [{"class": subject, "set": value}] if value > 0 else [{"not": {"class": subject, "set": -value}}]
            else:
                expect = [{kind: subject, "eq": value}]
            name = self.index.object_name.get(obj, str(obj))
            if verb == 7 and other is not None and other in self.index.items and obj not in self.index.items:
                # "Use <item> with <object>": the item starts the sentence (a room object such as
                # the cannon nozzle has no "with" and would run "Use nozzle" alone); the item's
                # handler hands the sentence on to the object.
                oname = self.index.object_name.get(other, str(other))
                out.append((Move(f"auto: use {oname} with {name}", verb=7, obj=other, obj2=obj), expect, eff))
            elif verb in PLAYER_VERBS:
                out.append((Move(f"auto: verb {verb} {name}", verb=verb, obj=obj, obj2=other), expect, eff))
            elif verb == 255:  # the default entry answers any verb: Walk to is the cheapest to issue
                out.append((Move(f"auto: walk to {name}", obj=obj, obj2=other), expect, eff))
            elif verb == 4 and other is not None:  # Give <item> to a character (actor number)
                out.append((Move(f"auto: give {name} to actor {other}", verb=4, obj=obj, obj2=other), expect, eff))
            elif verb == 80 and other is not None:
                out.append((Move(f"auto: give {other} to {name}", verb=4, obj=other, obj2=obj), expect, eff))
        return out

    def candidates(self, ex: Executor, target: list[dict]) -> list[tuple[Move, list[dict]]]:
        state = ex.b.state()
        lines = [v for v in state["verbs"] if DIALOG_FIRST <= v["id"] <= DIALOG_LAST and v["name"]]
        if lines:
            return [(Move(f"auto: say {v['name'][:40]!r}", choose=v["name"]), []) for v in lines]
        if (not state["verbs"] and state["room"] not in ex.p.verbless_rooms) or ex.b.status.get("up", 0) <= 0:
            # A dialogue or cutscene is playing, or a screen waits for a key: nothing to click yet.
            return [(WAIT, []), (ESC, [])]
        here = state["vars"][VAR_ROOM]  # the room, or the forest screen inside room 58
        inv = set(state["inventory"])
        out: list[tuple[Move, list[dict]]] = []
        seen: set = set()
        for cond in target:
            if not ex.b.holds([cond]):
                self._for_cond(cond, ex, here, inv, seen, out, SUBGOAL_DEPTH, set())
        out.append((STAND, []))
        # Ways between the parts of this room (scripted walks): tried after the direct moves.
        for obj in self.index.scripted_walks(here):
            if (None, obj, None) not in seen:
                seen.add((None, obj, None))
                out.append((Move(f"auto: scripted walk via {self.index.object_name.get(obj, obj)}", obj=obj), []))
        return out

    def _walk_toward(self, here: int, room: int, seen: set, out: list) -> bool:
        hops = self._hops(here, room)
        for obj, nxt in hops:
            if (None, obj, None) not in seen:
                seen.add((None, obj, None))
                out.append((Move(f"auto: walk to exit {obj} -> room {nxt}", obj=obj), [{"var": VAR_ROOM, "eq": nxt}]))
        return bool(hops)

    def _for_cond(self, cond: dict, ex: Executor, here: int, inv: set, seen: set, out: list, depth: int,
                  chain: set) -> None:
        """Candidates that make ``cond`` true: an action that writes it, the way to that action's
        room, or (``depth`` levels down) what the action needs first: the facts its script
        tests, and the objects it uses that are not held yet."""
        key = repr(cond)
        if key in chain or len(out) >= MAX_CANDIDATES:
            return
        chain = chain | {key}
        room = cond.get("room", cond["eq"] if cond.get("var") == VAR_ROOM and "eq" in cond else None)
        if room is not None:
            if self._walk_toward(here, room, seen, out):
                return
            q = ("room", room, None)  # no exit leads there: a script loads it
        else:
            q = _query(cond)
        if q is None:
            return
        if q[0] == "var":
            bits = self._bit_sum(q[1], q[2])
            if bits:  # Var[260] = Bit[427] + ... + Bit[433]: every bit is a goal of its own
                for b in bits:
                    c = {"bit": b, "eq": 1}
                    if not ex.b.holds([c]):
                        self._for_cond(c, ex, here, inv, seen, out, depth, chain)
                return
        # The same move can write the fact on several branches: keep, per move, the branch with
        # the fewest conditions still missing.
        options: dict[tuple, tuple] = {}
        for move, expect, eff in self._actions_for(q):
            k = (move.verb, move.obj, move.obj2)
            missing = sum(1 for c in persistent(guard_facts(eff.guards)) if not ex.b.holds([c]))
            if k not in options or missing < options[k][0]:
                options[k] = (missing, move, expect, eff)
        for _, move, expect, eff in sorted(options.values(), key=lambda o: o[0]):
            if (move.verb, move.obj, move.obj2) in seen:
                continue
            if q[0] == "var" and eff.value is None:
                # A counter (Var[260] += 1 per stew ingredient): any change is progress.
                now = ex.b.state()["vars"][q[1]]
                expect = [{"not": {"var": q[1], "eq": now}}]
            needs = [c for c in persistent(guard_facts(eff.guards)) if not ex.b.holds([c])]
            # A given or used inventory object has to be held first.
            for o in ([move.obj] if move.verb == 4 else [move.obj, move.obj2] if move.verb == 7 else []):
                if o is not None and o not in inv and self.index.writers("owner", o, EGO):
                    if self.index.object_room.get(o) != here or not self._on_screen(ex, o):
                        needs.append({"has": o})
            if needs:
                if depth > 0:
                    for c in needs:
                        self._for_cond(c, ex, here, inv, seen, out, depth - 1, chain)
                continue
            seen.add((move.verb, move.obj, move.obj2))
            rooms = {self.index.object_room.get(o) for o in (move.obj, move.obj2) if o is not None and o not in inv}
            if rooms <= {here}:
                out.append((move, expect))
            elif len(rooms) == 1:
                self._walk_toward(here, rooms.pop(), seen, out)

    def _random(self, cond: dict) -> bool:
        """A condition set by a random draw, not by a player action: an actor's room (the
        storekeeper's errand), or a var that no action writes or that is written from
        getRandomNr (the safe combination, a pirate's insult)."""
        inner = cond["not"] if "not" in cond else cond
        kind = next(iter(inner))
        if kind == "actor_room":  # random unless a script puts the actor there (the fuse)
            return not self.index.writers("actor_room", inner["actor_room"], inner.get("eq"))
        if kind != "var":
            return False
        found = self.index.writers("var", inner["var"], None)
        return not found or all("getRandomNr" in e.expr for _, e in found)

    def _unmet(self, ex: Executor, target: list[dict]) -> int:
        n = 0
        for c in target:
            if ex.b.holds([c]):
                continue
            bits = self._bit_sum(c["var"], c.get("eq")) if "var" in c and "eq" in c else []
            n += sum(1 for b in bits if not ex.b.holds([{"bit": b, "eq": 1}])) if bits else 1
        return n

    def _bit_sum(self, var: int, value: int | None) -> list[int]:
        """The bits whose sum a script assigns to ``var``, when ``value`` needs all of them."""
        for _, eff in self.index.writers("var", var, None):
            terms = re.findall(r"Bit\[(\d+)\]", eff.expr)
            rest = re.sub(r"Bit\[\d+\]|[()+ ]", "", eff.expr)
            if terms and not rest and value == len(terms):
                return [int(t) for t in terms]
        return []

    @staticmethod
    def _on_screen(ex: Executor, obj: int) -> bool:
        return any(o["id"] == obj for o in ex.b.state()["room_objects"])

    # --- evaluation -----------------------------------------------------------------------

    def _try(self, node: Node, move: Move, expect: list[dict], target: list[dict]) -> Node | None:
        child = self._try_once(node, move, expect, target)
        if child is None or move in (WAIT, ESC, STAND) or child.ex.b.holds(target):
            return child
        failing = [c for c in target if not child.ex.b.holds([c])]
        if not (failing and all(self._random(c) for c in failing) and (not expect or child.ex.b.holds(expect))):
            return child
        # The move did its part but a random outcome is wrong: the same move a few frames later
        # draws other numbers.
        for d in range(1, STEER_DELAYS + 1):
            later = self._try_once(node, replace(move, delay=move.delay + d), expect, target)
            if later is not None and later.ex.b.holds(target):
                child.ex.b.quit()
                self.log.append(f"  ~ {move.name}: random outcome steered with {d} frames of delay")
                return later
            if later is not None:
                later.ex.b.quit()
        return child

    def _try_once(self, node: Node, move: Move, expect: list[dict], target: list[dict]) -> Node | None:
        ex = Executor(self.fork(node.ex.b), self.release, self.fork)
        ex.skips = dict(node.ex.skips)
        ex.movie = list(node.ex.movie)
        try:
            if move is WAIT:
                ex.wait_gate(self.window)  # the first frame a click counts, as a player would act
                return Node(ex, node.moves + [move], ex.b.status["qf"], node.depth + 1)
            if move is STAND:
                st = ex.b.run(until_any=[target], menu=True, max_frame=ex.b.frame + STAND_FRAMES)
                if st.get("stop") not in ("until_any:0", "menu"):
                    raise RouteError("nothing came")
                return Node(ex, node.moves + [move], ex.b.status["qf"], node.depth + 1)
            else:
                ex.play(replace(move, until=[], max_frames=self.window))
            groups = [g for g in (target, expect) if g]
            # A dialogue line's result is the next menu: stop on the first frame it takes a click.
            st = ex.b.run(until_any=groups or None, idle=True, menu=move.choose is not None,
                          max_frame=ex.b.frame + self.window)
            if "end" in st:
                raise RouteError(f"run ended: {st['end']}")
            if self._menu(ex) or ex.b.holds(expect or target):
                pass  # a dialogue to answer, or the move's result: either way a new state
            else:
                if st["stop"] == "idle" and expect:
                    # Idle can come between a sentence and the script it starts: give it a moment.
                    ex.b.run(until_any=groups, menu=True, max_frame=ex.b.frame + GRACE)
                if expect and not (self._menu(ex) or ex.b.holds(expect)) and move.key is None and \
                        move.choose is None:
                    # Missed its own effect: a cutscene that began right after the click may
                    # have swallowed the sentence. Once the game is idle again, say it once more.
                    ex.wait_gate(self.window)
                    ex.play(replace(move, until=[], max_frames=self.window))
                    ex.b.run(until_any=groups, idle=True, max_frame=ex.b.frame + self.window)
                    if not ex.b.holds(expect):
                        ex.b.run(until_any=groups, menu=True, max_frame=ex.b.frame + GRACE)
                if expect and not (self._menu(ex) or ex.b.holds(expect)):
                    raise RouteError("no effect")
        except (RouteError, ControlError) as e:
            self.log.append(f"  x {move.name}: {str(e)[:80]}")
            ex.b.quit()
            return None
        return Node(ex, node.moves + [move], ex.b.status["qf"], node.depth + 1)

    @staticmethod
    def _menu(ex: Executor) -> bool:
        return any(DIALOG_FIRST <= v["id"] <= DIALOG_LAST and v["name"] for v in ex.b.state()["verbs"])

    @staticmethod
    def _signature(ex: Executor) -> tuple:
        st = ex.b.state()
        lines = tuple(v["name"] for v in st["verbs"] if DIALOG_FIRST <= v["id"] <= DIALOG_LAST and v["name"])
        actable = (bool(st["verbs"]) or st["room"] in ex.p.verbless_rooms) and ex.b.status.get("up", 0) > 0
        ego = ex.b.status.get("ego") or (0, 0)
        where = (ego[0] // 96, ego[1] // 48)  # coarse: tells the halves of a wide room apart
        return (st["room"], tuple(sorted(st["inventory"])), tuple(sorted(st["bits_set"])),
                tuple(sorted(st["states"].items())), lines, actable, where)

    def reach(self, ex: Executor, target: list[dict], bound: int | None = None) -> SearchResult | None:
        """The fastest branch found from ``ex``'s state on which ``target`` holds, or None. With
        ``bound`` (the quarter-frame clock of a branch found another way), only a branch that
        beats it counts."""
        target = persistent(target)
        root = Node(ex, [], ex.b.status["qf"], 0)
        if not target or ex.b.holds(target):
            return SearchResult(ex, [], root.qf, 0)
        frontier = [root]
        seen = {self._signature(ex)}
        seen_menus: dict[tuple, int] = {}
        best: Node | None = None
        tried = 0
        expanded = 0
        root.unmet = self._unmet(ex, target)
        while frontier and expanded < self.max_nodes:
            frontier.sort(key=lambda n: n.priority)
            node = frontier.pop(0)
            limit = min(x for x in (best.qf if best else None, bound) if x is not None) \
                if (best or bound is not None) else None
            if limit is not None and node.qf >= limit:
                break
            expanded += 1
            if node.depth >= self.max_depth:
                continue
            for move, expect in self.candidates(node.ex, target):
                child = self._try(node, move, expect, target)
                tried += 1
                if child is None:
                    continue
                self.log.append(f"  + {move.name} -> qf {child.qf} room {child.ex.b.status['room']}")
                if bound is not None and child.qf >= bound:
                    child.ex.b.quit()  # already slower than the other way
                    continue
                if child.ex.b.holds(target):
                    if best is None or child.qf < best.qf:
                        if best is not None:
                            best.ex.b.quit()
                        best = child
                    else:
                        child.ex.b.quit()
                    continue
                sig = self._signature(child.ex)
                # A menu can repeat with progress hidden in a script's locals (Smirk counts three
                # "I do so!"), so menus are not deduplicated, only capped.
                repeats = seen_menus.get(sig, 0) if sig[4] else 0
                if (sig in seen and not sig[4]) or repeats >= MENU_REPEATS:
                    child.ex.b.quit()
                    continue
                seen.add(sig)
                if sig[4]:
                    seen_menus[sig] = repeats + 1
                child.unmet = self._unmet(child.ex, target)
                frontier.append(child)
                if len(frontier) > MAX_FRONTIER:  # each branch is a live engine: drop the worst
                    frontier.sort(key=lambda n: n.priority)
                    for n in frontier[MAX_FRONTIER:]:
                        if n is not root:
                            n.ex.b.quit()
                    del frontier[MAX_FRONTIER:]
            if self.verbose:
                print(f"    search: expanded {expanded} tried {tried} frontier {len(frontier)} "
                      f"best {best.qf if best else None} bound {bound}", flush=True)
        for n in frontier:
            if n is not root:
                n.ex.b.quit()
        if best is None:
            return None
        return SearchResult(best.ex, best.moves, best.qf, tried)
