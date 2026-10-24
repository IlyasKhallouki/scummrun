"""E003 step 2: the game end, read from the IR: a cutscene the game never leaves.

usage: uv run python experiments/E003-path-through-cloud/end.py   (writes results/end-game.json and .log)
       from end import game_ends; ends, counts = game_ends(ir.load())   (the matches, with their goal facts)

**Rule** (plan.md step 2, "Game end"): a script opens a cutscene and, with that cutscene still open,
starts its last loop, a loop it never leaves. This was found by reading MI1's ending (plan.md C2,
disclosed exception), so it is fitted on one game; the code names no game number.

Over one IR unit (``ir.load()``), as a control-flow rule:

* the **graph**: instruction i goes to ``next`` and to a jump's ``target`` (ir.py). A conditional jump
  whose condition folds to a constant keeps only the side it takes (counted). An index equal to
  ``len(unit.ins)`` is the script's end;
* a **loop** is a strongly connected set of instructions (more than one, or one that jumps to itself)
  reached from a start of the unit (index 0, and every verb entry of an object). Its **ways out**:
  an edge that leaves it (the script's end counts as leaving); an instruction that ends the script
  (``ends`` "always"); a room change (``ends`` "room") in a local, entry or exit unit, whose slot the
  room change kills (room.cpp:58-72, script.cpp:1083-1105); an ``if_self`` that stops this unit
  (``stop_reach``); a restart, ``systemOps(1)`` (script_v5.cpp:2201-2202), whose resetScumm empties
  the cutscene stack (scumm.cpp:4260, 2122). A loop with no way out is the last loop the script runs:
  once it starts, no instruction after it runs, so "last loop" needs no ordering by offset;
* **run-time ways out** are kept apart, as their own kind of loop: an ``if_self`` whose target is read
  at run time and may be this unit, and a room change in an object unit, which ends the slot only
  when the object was not owned by an actor when the script started (an owned object's script runs as
  an inventory slot and survives the room change: object.cpp:361-367, script.cpp:1088-1105; ir.py);
* ``stop_reach``: each stop reaches one kind of slot only. stopScript, a startScript without R
  (runScript stops a running copy first, script.cpp:65-66) and doSentence(STOP) (stopScript of
  VAR_SENTENCE_SCRIPT, script_v5.cpp:1000-1010) stop only global and local slots (script.cpp:270-272);
  startObject (always without R, script.cpp:117-118), stopObjectScript and setOwnerOf(o, 0)
  (clearOwnerOf, object.cpp:132-133, 180) stop only object slots (script.cpp:304-306). Entry and exit
  code runs in a room slot under kScriptNumENCD/EXCD (script.h:121-122), numbers no stop names. A stop
  that cannot reach the unit's slot kind is never a way out, whatever its target. Of the right kind: a constant target is a way
  out when it is this unit's number (startScript(0) does nothing, script.cpp:60-61; stopScript(0) is
  ``ends`` "always"), and a target read at run time is a run-time way out;
* the loop must **yield** (``yields`` frame, delay or wait): one that does not hangs the engine in one
  frame, which is not an end of the game but a freeze; those are counted apart;
* **the cutscene is open** on every path into it: the levels this unit has pushed, followed along the
  graph from every start, are 1 or more at every instruction of the loop. ``cutscene`` pushes a level
  and ``endCutscene`` pops one (beginCutscene, endCutscene: script.cpp:1632-1687). The stack is the
  engine's one global stack, ``vm.cutSceneStackPointer``: endCutscene pops its top whichever script
  runs it, and errors only when the global pointer is 0 (script.cpp:1668-1681). So an endCutscene at
  the unit's own depth 0 pops a level some other script pushed (or underflows when none is open); the
  unit's own depth stays 0 and the path goes on. These sites are counted over the whole game
  (``foreign_pops``): when there is none, every pop in the game matches an earlier push by the same
  script, so the stack never drops below the levels a script still holds, and no other script, no
  hook the engine runs nested (VAR_CUTSCENE_START_SCRIPT on ``cutscene``, script.cpp:1645-1646; the
  exit and entry code and hooks on a room change, script.cpp:989-1060; the inventory and scroll hooks,
  ir.py) and no script the unit starts can close the matched level. That is the checked assumption
  behind "the game never leaves it". A push past kMaxCutsceneNum - 1 (5, script.h:111) is the engine's
  "Cutscene stack overflow" error() (script.cpp:1637-1638): the path ends there, counted (the unit's
  own depth only, so a lower bound);
* **Esc cannot leave it**: no override armed in the loop (``beginOverride``, which ``endOverride``
  and ``endCutscene`` clear, script.cpp:1711-1737, 1650-1687) whose jump (ir.py ``esc``) goes outside
  the loop. abortCutscene moves the script to that jump (script.cpp:1689-1709);
* **input is not turned back on in the loop**: the engine takes a click or a verb key whenever
  ``_userPut`` > 0, with or without a cutscene (checkExecVerbs, verbs.cpp:590), so a loop that runs
  UserputOn or UserputSoftOn (script_v5.cpp:866-868, 882-884) is counted apart. That input is off
  when the loop starts rests on the cutscene start hook and on the unit's own ops before the loop
  (``input_ops_before_loop``, listed per match); no other script turning input on while the loop runs
  is not checked.

Stops from outside, per match: stopScript of a slot that holds a cutscene level or an override is an
error() in v5 (``cutsceneOverride``, script.cpp:274-276, 308-310), and so is a start without R of the
running script, which stops it first. So once the matched script has opened its cutscene, a stop or
restart from another script is an engine error, not a way on; the sites are listed
(``stop_sites``, ``restart_sites``). A restart of the game (``systemOps(1)``) by another script is
not checked; its sites are counted (``restart_game_sites``).

Near misses, counted by kind (``KINDS``): loops with a way out, split by whether the cutscene is open on
every path (wait loops); closed loops with a run-time way out; closed loops with no yield (hangs);
closed loops with no cutscene open (ambient loops) or open on some paths only; closed cutscene loops
that Esc can leave or that turn input on.

**Goal facts** (for step 4), per match:

* ``goal_any``: the slot is at one of the loop's resume points, as facts.py writes it: ``at:<unit>:W``
  with W the instruction after each breakHere or delay in the loop, and each wait in it (a blocked
  wait resumes at itself). This is a disjunction, and the bridge has no stop condition on a slot's
  code offset (speedrun_conditions.cpp);
* ``must_reach``: the instructions from which every path enters the loop, a least fixpoint from the
  loop: an instruction joins when it has no way out (also no run-time one), it has a successor, all
  its successors are in the set, and every override armed there jumps into the set. Least, so a path
  from it cannot circle elsewhere for ever;
* ``goal_detectable``: when every start of the unit is in ``must_reach`` and the unit has a running
  fact (``script:N`` for a global, ``local:N`` for a local with its room in ``var:22``,
  VAR_ROOM_RESOURCE, vars.cpp:58; ``objscript:O`` for an object; facts.py), that fact = 1. It is
  equivalent to the loop goal (the script, once running, reaches the loop on every path, and the
  stops above are engine errors), it is one fact, so it fits Fast Downward's goal, which is a
  conjunction, and the bridge can stop on it: its ``script``
  condition is isScriptRunning (speedrun_conditions.cpp:147, 294-295), checked every frame with or
  without a cutscene (speedrun_control.cpp:170, onDecisionPoint at scumm.cpp:3254). Step 4 should use
  it; ``goal_any`` would need a goal action. ``delay_before_loop`` is the least sum of the constant
  delay arguments on a path from a start to the loop, the head start the running fact has.
"""

import collections
import heapq
import json
import os
from pathlib import Path

import ir

HERE = Path(__file__).resolve().parent
OUT = HERE / "results" / "end-game.json"
LOG = HERE / "results" / "end-game.log"
MAX_CUTSCENE = 5  # kMaxCutsceneNum, script.h:111: beginCutscene errors when the stack pointer reaches it
VAR_ROOM_RESOURCE = 22  # vars.cpp:58
RESTART = 1  # systemOps sub-op SO_RESTART, script_v5.cpp:2201-2202
LOG_SHOWN = 12  # places the log lists per near-miss kind (display only; the JSON lists them all)
YIELDS = ("frame", "delay", "wait")
INPUT_ON = ("UserputOn", "UserputSoftOn")  # _userPut = 1 and _userPut++, script_v5.cpp:866-868, 882-884
INPUT_OPS = ("UserputOn", "UserputOff", "UserputSoftOn", "UserputSoftOff")
# Ops that run another script nested (the cites are ir.py's): the hook, or the room's code and hooks.
HOOK_OPS = {"cutscene": "VAR_CUTSCENE_START_SCRIPT, script.cpp:1645-1646",
            "endCutscene": "VAR_CUTSCENE_END_SCRIPT, script.cpp:1685-1686",
            "loadRoom": "exit and entry code and hooks, script.cpp:989-1060",
            "loadRoomWithEgo": "exit and entry code and hooks, script.cpp:989-1060; camera hook",
            "actorFollowCamera": "camera hook camera.cpp:85; a room change, script.cpp:989-1060",
            "pickupObject": "VAR_INVENTORY_SCRIPT, script_v5.cpp:2033",
            "setOwnerOf": "VAR_INVENTORY_SCRIPT, object.cpp:173",
            "setObjectName": "VAR_INVENTORY_SCRIPT, object.cpp:1314",
            "setCameraAt": "VAR_SCROLL_SCRIPT, camera.cpp:52-55"}
WAIT, CLOSED_RUNTIME = "cutscene open, has a way out (a wait loop)", "closed but for a way out read at run time"
KINDS = ("match", "closed, no yield (a hang)", "closed, cutscene open on some paths only",
         "closed, no cutscene open (ambient)", "closed, cutscene open, Esc leaves it",
         "closed, cutscene open, the loop turns input on", CLOSED_RUNTIME, WAIT, "other loop")
RUNNING = {"global": "script", "local": "local", "obj": "objscript"}  # facts.py's running-slot keys
OVERFLOW = "cutscene stack overflow at the unit's own depth (engine error, path ends)"
OBJ_ROOM = "room change in an object unit (ends a room slot, not an inventory slot)"
ODD_STOP = "if_self of an unknown form, kept as a run-time way out"
_CMP = {"==": lambda a, b: a == b, "!=": lambda a, b: a != b, "<": lambda a, b: a < b,
        "<=": lambda a, b: a <= b, ">": lambda a, b: a > b, ">=": lambda a, b: a >= b}


def fold(cond) -> bool | None:
    """The value of a condition built only from constants, else None."""
    if isinstance(cond, ir.Const):
        return cond.value != 0
    if isinstance(cond, ir.Truth):
        return fold(cond.x)
    if isinstance(cond, ir.Not):
        v = fold(cond.x)
        return None if v is None else not v
    if isinstance(cond, ir.Cmp) and isinstance(cond.a, ir.Const) and isinstance(cond.b, ir.Const):
        return _CMP[cond.op](cond.a.value, cond.b.value)
    return None


def successors(u: ir.Unit, i: int, stats: collections.Counter) -> list[int]:
    """Where instruction i goes: ``next`` and a jump's ``target``; ``len(u.ins)`` is the end. A jump on a
    constant condition keeps the side it takes (it jumps when the condition is false, ir.py ``Jump``)."""
    x = u.ins[i]
    if x.ends == "always":
        return []
    out = [j for j in (x.next, x.target) if j is not None]
    if isinstance(x.st, ir.Jump) and x.st.cond is not None:
        v = fold(x.st.cond)
        if v is not None:
            stats["constant conditions folded"] += 1
            out = [x.next] if v else [x.target]
    return out


def unit_name(key: tuple) -> str:
    """The facts.py ``at:`` unit of an IR unit key: script:N, local:N, obj:O, entry, exit."""
    return {"global": f"script:{key[-1]}", "local": f"local:{key[-1]}", "obj": f"obj:{key[-1]}"}.get(key[0], key[0])


def starts_of(u: ir.Unit) -> list[int]:
    return sorted({0} | set(u.entries.values())) if u.ins else []


def op_name(x: ir.Ins) -> str | None:
    return x.st.name if isinstance(x.st, (ir.Op, ir.Start)) else None


def is_restart(x: ir.Ins) -> bool:
    return op_name(x) == "systemOps" and x.st.args == (ir.Const(RESTART),)


def stacks(u: ir.Unit, stats: collections.Counter) -> dict[int, set[tuple]]:
    """The cutscene levels this unit can hold at each instruction, from every start: a tuple with one
    entry per level (its own level 0 first, the levels below it that are not its own), each the index
    of the override jump armed there or None. An endCutscene at its own depth 0 pops a level it did not
    push (module docstring): the tuple stays as it is and the path goes on."""
    seen: dict[int, set[tuple]] = collections.defaultdict(set)
    work = [(s, (None,)) for s in starts_of(u)]
    while work:
        i, st = work.pop()
        if i >= len(u.ins) or st in seen[i]:
            continue
        seen[i].add(st)
        x = u.ins[i]
        name = op_name(x)
        if name == "cutscene":
            if len(st) >= MAX_CUTSCENE:  # the stack pointer would reach kMaxCutsceneNum
                stats[OVERFLOW] += 1
                continue
            st = st + (None,)
        elif name == "endCutscene" and len(st) > 1:
            st = st[:-1]
        elif name == "beginOverride":
            st = st[:-1] + (x.esc,)
        elif name == "endOverride":
            st = st[:-1] + (None,)
        for j in successors(u, i, collections.Counter()):
            work.append((j, st))
    return seen


def foreign_pops(u: ir.Unit, st: dict[int, set[tuple]]) -> list[str]:
    """Cites of the endCutscenes this unit can reach at its own depth 0 (module docstring)."""
    return [u.ins[i].cite() for i in sorted(st) if i < len(u.ins) and op_name(u.ins[i]) == "endCutscene"
            and any(len(s) == 1 for s in st[i])]


def sccs(nodes: list[int], edges: dict[int, list[int]]) -> list[list[int]]:
    """Strongly connected components (Tarjan, iterative)."""
    index, low, on, stack, out, n = {}, {}, set(), [], [], 0
    for root in nodes:
        if root in index:
            continue
        work = [(root, iter(edges[root]))]
        index[root] = low[root] = n
        n += 1
        stack.append(root)
        on.add(root)
        while work:
            v, it = work[-1]
            for w in it:
                if w not in edges:
                    continue
                if w not in index:
                    index[w] = low[w] = n
                    n += 1
                    stack.append(w)
                    on.add(w)
                    work.append((w, iter(edges[w])))
                    break
                if w in on:
                    low[v] = min(low[v], index[w])
            else:
                work.pop()
                if work:
                    low[work[-1][0]] = min(low[work[-1][0]], low[v])
                if low[v] == index[v]:
                    comp = []
                    while True:
                        w = stack.pop()
                        on.discard(w)
                        comp.append(w)
                        if w == v:
                            break
                    out.append(sorted(comp))
    return out


def stop_reach(key: tuple, x: ir.Ins, stats: collections.Counter) -> str | None:
    """What an ``if_self`` instruction can do to this unit's own slot: "self" (it stops it), "runtime"
    (it may: the target is read at run time), None (it cannot). Counted in ``stats`` by outcome."""
    st, name = x.st, op_name(x)
    if isinstance(st, ir.Start):
        obj, tgt = name == "startObject", (st.obj if name == "startObject" else st.script)
    elif name in ("stopScript", "stopObjectScript", "setOwnerOf") and st.args:
        obj, tgt = name != "stopScript", st.args[0]
    elif name == "doSentence":
        obj, tgt = False, None  # stopScript(VAR(VAR_SENTENCE_SCRIPT)), script_v5.cpp:1000-1010
    else:
        stats[ODD_STOP] += 1
        return "runtime"
    if not (key[0] == "obj" if obj else key[0] in ("global", "local")):
        stats["if_self that cannot reach this unit's slot kind (not a way out)"] += 1
        return None
    if not isinstance(tgt, ir.Const):
        stats["if_self of this slot kind with a target read at run time (a run-time way out)"] += 1
        return "runtime"
    hit = tgt.value == key[-1] and tgt.value != 0
    stats["if_self on this unit's own number" if hit else "if_self on another constant script or object"] += 1
    return "self" if hit else None


def ways_of(key: tuple, u: ir.Unit, i: int, stats: collections.Counter) -> tuple[str | None, str | None]:
    """(way out, run-time way out) of instruction i, each a short label or None (module docstring)."""
    x = u.ins[i]
    if x.ends == "always":
        return "always", None
    if x.ends == "room":
        if key[0] in ("local", "entry", "exit"):
            return "room", None
        if key[0] == "obj":
            stats[OBJ_ROOM] += 1
            return None, "room (unless an inventory slot)"
    if x.ends == "if_self":
        reach = stop_reach(key, x, stats)
        return ("if_self", None) if reach == "self" else (None, "if_self" if reach == "runtime" else None)
    if is_restart(x):
        return "restart", None
    return None, None


def loops(key: tuple, u: ir.Unit, stats: collections.Counter, st: dict | None = None) -> list[dict]:
    """Every loop of the unit reached from a start, classified by the rule (module docstring)."""
    st = st if st is not None else stacks(u, stats)
    reached = sorted(i for i in st if i < len(u.ins))
    edges = {i: successors(u, i, stats) for i in reached}
    out = []
    for comp in sccs(reached, edges):
        inside = set(comp)
        if len(comp) == 1 and comp[0] not in edges[comp[0]]:
            continue  # not a loop
        leaves = [(i, j) for i in comp for j in edges[i] if j not in inside]
        ways = {i: ways_of(key, u, i, stats) for i in comp}
        ends = [i for i in comp if ways[i][0]]
        runtime = [i for i in comp if ways[i][1]]
        ways_out = sorted({"an edge out"} if leaves else set()) + sorted({ways[i][0] for i in ends})
        yields = [i for i in comp if u.ins[i].yields in YIELDS]
        depth = {len(s) - 1 for i in comp for s in st[i]}
        esc_out = sorted({s[-1] for i in comp for s in st[i] if s[-1] is not None
                          and u.ins[s[-1]].target not in inside})
        input_on = [i for i in comp if op_name(u.ins[i]) in INPUT_ON]
        open_ = min(depth) >= 1
        if leaves or ends:
            kind = WAIT if open_ else "other loop"
        elif runtime:
            kind = CLOSED_RUNTIME
        elif not yields:
            kind = "closed, no yield (a hang)"
        elif not open_:
            kind = "closed, cutscene open on some paths only" if max(depth) >= 1 else "closed, no cutscene open (ambient)"
        elif esc_out:
            kind = "closed, cutscene open, Esc leaves it"
        elif input_on:
            kind = "closed, cutscene open, the loop turns input on"
        else:
            kind = "match"
        out.append({"kind": kind, "comp": comp, "depth": sorted(depth), "yields": yields, "leaves": leaves,
                    "ends": ends, "runtime": [(i, ways[i][1]) for i in runtime], "esc_out": esc_out,
                    "input_on": input_on, "ways_out": ways_out})
    return out


def must_reach(key: tuple, u: ir.Unit, comp: list[int], st: dict[int, set[tuple]]) -> set[int]:
    """The instructions from which every path enters the loop ``comp`` (module docstring)."""
    stats = collections.Counter()
    edges = {i: successors(u, i, stats) for i in st if i < len(u.ins)}
    out, changed = set(comp), True
    while changed:
        changed = False
        for i, succ in edges.items():
            if i in out or not succ or any(ways_of(key, u, i, stats)):
                continue
            armed = {lv for s in st[i] for lv in s if lv is not None}
            if all(j in out for j in succ) and all(j in out for j in armed):
                out.add(i)
                changed = True
    return out


def delay_before(u: ir.Unit, comp: list[int], stats: collections.Counter) -> int | None:
    """The least sum of constant delay arguments on a path from a start to the loop (Dijkstra); None
    when no path reaches it."""
    def cost(i):
        x = u.ins[i]
        return x.st.args[0].value if op_name(x) == "delay" and x.st.args and isinstance(x.st.args[0], ir.Const) else 0
    best, heap, inside = {}, [(0, s) for s in starts_of(u)], set(comp)
    while heap:
        d, i = heapq.heappop(heap)
        if i >= len(u.ins) or i in best:
            continue
        best[i] = d
        if i in inside:
            return d
        for j in successors(u, i, stats):
            heapq.heappush(heap, (d + cost(i), j))
    return None


def _target_number(x: ir.Ins):
    """A start's or stopScript's constant script number, else None."""
    if isinstance(x.st, ir.Start) and x.st.name in ("startScript", "chainScript") and isinstance(x.st.script, ir.Const):
        return x.st.script.value
    if op_name(x) == "stopScript" and x.st.args and isinstance(x.st.args[0], ir.Const):
        return x.st.args[0].value
    return None


def sites(units: dict, key: tuple) -> tuple[list[str], list[str], list[str]]:
    """Start sites of a global or local unit (startScript or chainScript of its number; a local one only
    from its own room), the starts among them without R (they stop a running copy first), and stop
    sites (stopScript of its number)."""
    if key[0] not in ("global", "local"):
        return [], [], []
    n = key[-1]
    started, restarted, stopped = [], [], []
    for k, u in units.items():
        if key[0] == "local" and u.room != key[1]:
            continue
        for x in u.ins:
            if _target_number(x) != n:
                continue
            if isinstance(x.st, ir.Start):
                started.append(x.cite())
                if not x.st.recursive:
                    restarted.append(x.cite())
            else:
                stopped.append(x.cite())
    return started, restarted, stopped


def resume_points(u: ir.Unit, comp: list[int]) -> list[int]:
    """The W of facts.py's ``at:`` keys a slot in the loop can show: after a breakHere or delay, the
    next instruction; at a wait, the wait itself."""
    out = set()
    for i in comp:
        x = u.ins[i]
        if x.yields in ("frame", "delay") and x.next is not None and x.next < len(u.ins):
            out.add(u.ins[x.next].offset)
        elif x.yields == "wait":
            out.add(x.offset)
    return sorted(out)


def describe(key: tuple, u: ir.Unit, lp: dict, st: dict, units: dict, stats: collections.Counter) -> dict:
    """A match: where the loop is, what runs before it, who starts and stops it, and its goal facts."""
    comp = lp["comp"]
    before = [i for i in sorted(st) if i < len(u.ins) and i not in set(comp)]
    nested = [f"{u.ins[i].cite()} {op_name(u.ins[i])}" + (f" ({HOOK_OPS[op_name(u.ins[i])]})"
                                                          if op_name(u.ins[i]) in HOOK_OPS else "")
              for i in before if isinstance(u.ins[i].st, ir.Start) or op_name(u.ins[i]) in HOOK_OPS]
    reach = must_reach(key, u, comp, st)
    started, restarted, stopped = sites(units, key)
    room = key[1] if key[0] in ("local", "entry", "exit") else None
    detectable = None
    if key[0] in RUNNING and all(s in reach for s in starts_of(u)):
        detectable = {f"{RUNNING[key[0]]}:{key[-1]}": 1} | ({f"var:{VAR_ROOM_RESOURCE}": room} if room is not None else {})
    return {
        "unit": list(key), "file": u.file, "loop": [u.ins[comp[0]].cite(), u.ins[comp[-1]].cite()],
        "instructions": len(comp), "depth": lp["depth"], "yields": [u.ins[i].cite() for i in lp["yields"]],
        "cutscene_ops_before": [u.ins[i].cite() for i in before if op_name(u.ins[i]) == "cutscene"],
        "nested_runs_before_loop": nested,
        "input_ops_before_loop": [f"{u.ins[i].cite()} {op_name(u.ins[i])}" for i in before
                                  if op_name(u.ins[i]) in INPUT_OPS],
        "start_sites": started, "restart_sites": restarted, "stop_sites": stopped,
        "goal_any": [f"at:{unit_name(key)}:{w}" for w in resume_points(u, comp)],
        "room_resource": room,
        "must_reach_from": [u.ins[i].cite() for i in sorted(reach) if i in st
                            and (i in starts_of(u) or any(j not in reach for j in st if j < len(u.ins)
                                                         and i in successors(u, j, collections.Counter())))],
        "goal_detectable": detectable,
        "delay_before_loop": delay_before(u, comp, stats),
        "after_loop_never_runs": [x.cite() for x in u.ins[comp[-1] + 1:]],
    }


def game_ends(units: dict) -> tuple[list[dict], dict]:
    """The units that match the rule, each with its loop and goal facts, and the counts: every loop kind,
    the near misses, the game-wide foreign pops and game restarts."""
    stats, matches, near = collections.Counter({OVERFLOW: 0, OBJ_ROOM: 0, ODD_STOP: 0}), [], collections.defaultdict(list)
    kinds, ways, depths = collections.Counter({k: 0 for k in KINDS}), collections.Counter(), collections.Counter()
    pops, restarts = [], []
    for key, u in units.items():
        if not u.ins:
            continue
        st = stacks(u, stats)
        pops += foreign_pops(u, st)
        restarts += [x.cite() for x in u.ins if is_restart(x)]
        for lp in loops(key, u, stats, st):
            kinds[lp["kind"]] += 1
            comp = lp["comp"]
            where = f"{u.file} [{u.ins[comp[0]].offset:04X}]-[{u.ins[comp[-1]].offset:04X}]"
            if lp["kind"] == WAIT:
                ways[" + ".join(lp["ways_out"])] += 1
            if lp["kind"] == CLOSED_RUNTIME:
                depths[str(lp["depth"])] += 1
                where += f" depth {lp['depth']} via " + ", ".join(f"{u.ins[i].cite()} {w}" for i, w in lp["runtime"])
            if lp["kind"] != "match":
                near[lp["kind"]].append(where)
                continue
            matches.append(describe(key, u, lp, st, units, stats))
    return matches, {"kinds": {k: kinds[k] for k in KINDS}, "near": {k: v for k, v in sorted(near.items())},
                     "wait_loop_ways_out": dict(ways.most_common()), "runtime_loop_depths": dict(sorted(depths.items())),
                     "foreign_pops": pops, "restart_game_sites": restarts, "stats": dict(sorted(stats.items()))}


def write_atomic(path: Path, text: str) -> None:
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(text)
    os.replace(tmp, path)


def main() -> None:
    units = ir.load()
    matches, counts = game_ends(units)
    OUT.parent.mkdir(exist_ok=True)
    write_atomic(OUT, json.dumps({
        "rule": "plan.md step 2 game end; end.py",
        "goal_note": "goal_detectable is one fact (Fast Downward goals are conjunctions) the bridge can stop on "
                     "(its script condition); goal_any is a disjunction of at: facts the bridge cannot see",
        "matches": matches, "loop_kinds": counts["kinds"], "near_misses": counts["near"],
        "foreign_pops": counts["foreign_pops"], "restart_game_sites": counts["restart_game_sites"],
        "stats": counts["stats"]}, indent=1) + "\n")
    log = [f"units: {len(units)}, with instructions: {sum(bool(u.ins) for u in units.values())}",
           f"loops reached from a start: {sum(counts['kinds'].values())}, by kind: {counts['kinds']}",
           f"wait loops (cutscene open on every path) by their ways out: {counts['wait_loop_ways_out']}",
           f"closed loops with a run-time way out, by the cutscene depths on their paths: {counts['runtime_loop_depths']}",
           f"checked assumption: endCutscenes that pop a level their script did not push, over the whole game: "
           f"{len(counts['foreign_pops'])} {counts['foreign_pops']}"
           + (" (none: no script, hook or callee can close another script's level)" if not counts["foreign_pops"] else ""),
           f"not checked: game restarts (systemOps({RESTART})) by other scripts while a matched loop runs: "
           f"{len(counts['restart_game_sites'])} sites {counts['restart_game_sites']}",
           f"engine errors, folds and if_self outcomes: {counts['stats'] or 'none'}",
           f"matches: {len(matches)}"]
    for m in matches:
        log += [f"  {m['unit']} {m['file']}: loop {m['loop'][0]} .. {m['loop'][1]} ({m['instructions']} instructions, "
                f"cutscene depth {m['depth']}, {len(m['yields'])} yields)",
                f"    cutscene opened at {m['cutscene_ops_before']}; nested runs before the loop (covered by the "
                f"game-wide pop check): {len(m['nested_runs_before_loop'])} {m['nested_runs_before_loop']}",
                f"    input ops before the loop: {m['input_ops_before_loop'] or 'none'} (input off in the loop rests on "
                f"the cutscene start hook; no other script turning it on is not checked)",
                f"    started from {len(m['start_sites'])} sites {m['start_sites']}, {len(m['restart_sites'])} of them "
                f"without R; stop sites (stopScript): {len(m['stop_sites'])} {m['stop_sites']} (with the cutscene open, "
                f"a stop or a start without R of it is an engine error, script.cpp:274-276)",
                f"    never runs (after the loop): {m['after_loop_never_runs']}",
                f"    every path enters the loop from: {m['must_reach_from']}; least delay from a start to the loop: "
                f"{m['delay_before_loop']}",
                f"    goal, detectable (one fact, the bridge's script condition): {m['goal_detectable']}",
                f"    goal, at the loop (a disjunction, not a bridge condition): any of {m['goal_any']}"
                + (f" with var:{VAR_ROOM_RESOURCE} = {m['room_resource']}" if m["room_resource"] is not None else "")]
    for kind, where in counts["near"].items():
        log.append(f"near miss: {kind}: {len(where)}" + (f" {where}" if len(where) <= LOG_SHOWN else f" {where[:LOG_SHOWN]} ..."))
    text = "\n".join(log) + "\n"
    write_atomic(LOG, text)
    print(text, end="")


if __name__ == "__main__":
    main()
