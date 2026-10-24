"""E003 step 1 (the reader) for step 4: the scripts as actions with preconditions and effects, from model.Machine.

usage: from reader import Scope, Reader, Builder, build_task, task_start, replan_start
       scope = Scope.of(game, rooms, start)               # start: a facts.py map
       b = Builder(Reader(scope, domains.build(start)))
       recs = b.read_all(workers=7)                       # one Record per path of every input and event
       task = build_task(b, recs, goal)                   # task.Task; its start state is task_start(b, start)
       st, counts = replan_start(b, observed, predicted, task)   # a replan's start (plan step 5)

**What an action is** (plan step 1). A record is one path of ``model.Machine`` (``Reader``, a subclass, with the
features of rows 0-2: the click's frame, calls, room changes; no settle, D5) from one player input or one event: a
**precondition** ({fact: set of values}), the **effects** (the path's last write to each fact) and the **replay
checks** (the conditions on non-facts it assumed: plan step 1, "counted, and checked at replay"). A handler that can
end early becomes one action per path. The plan's other choice, conditional effects for branches that meet again,
is not used: one action per path is the simpler of the two that is correct, because two paths that assumed
different values of a fact never merge (the join signature holds the assumed values, ``Reader._join``). Paths that
assumed the same values still merge where the machine merges them (``Machine._drive``); what differs between them
then becomes a value set, which the task writes as a guess.

**The symbolic oracle** (``SymOracle``, a ``model.Oracle``) and the **fork on read** (``Reader.read``). A fact the
path has not written is read from the path's own assumptions (``SymPath.pre``) first. Otherwise, by its class
(``FactTable.kind``):

* a **constant**: a fact no code of the scope writes (``scope_writes``; a write whose target does not fold makes
  every key it may hit written) and the engine does not change as frames pass. Its value is the start's, and it is
  no precondition. This keeps the chain of ``classOfIs`` tests in the sentence script from forking;
* a **fact**: one with a finite domain (domains.py, plus the additions below) that some code writes. Its first read
  forks the path once per value it can still have, and the value each path took is its precondition. The fork is at
  the read, not at the jump: ``VAR_RESULT = getObjectOwner(O)`` copies the fact before the test reads the copy.
  ``Reader._advance`` snapshots the path before each step, catches the read (``Fork``) and runs the step again
  from the snapshot once per value. A domain of one value is that value, with no fork. VAR_ROOM and
  VAR_ROOM_RESOURCE are read as the room (room.cpp:149-172 writes them with it);
* a **non-fact**: a variable with no finite domain (domains.py ``why``) or one the engine changes as frames pass
  (domains.ENGINE, model.TIME_VARS, the mouse): an ``Unknown``, so the machine forks on it as it does on positions,
  timers and random draws, and the side taken is a replay check.

**One frame reads a non-fact once** (``Reader.test``, ``_jump``, ``getter``). Actors move in walkActors, after
runAllScripts (scumm.cpp:3231, 3284), so within one frame an actor's room or position, read twice with no op between
that puts the actor somewhere, is the same value; so is a variable with no domain that nothing wrote. A path keeps
the tests it forked on (``SymPath.nf``: the getter with its arguments, or the variable) and decides a later test on
the same value when only one side agrees with them; an op that may change the value (MEMO_GETTERS) or a write whose
target does not fold drops them. Without it a record could take local 216's getActorRoom(6) test as 28 and, two
nested starts later in the same frame, as not 28. Where paths that took a test different ways merge, the merged path
forgets that test, as the machine's merge forgets which branch ran, and where they hold any value differently it
forgets them all (``_merge``; a later test forks again and is listed in ``Record.either``). A value computed from a
non-fact is not that non-fact (``_arith_at``). Counted.

**Lazy reads** (an ``Unknown`` with the fact's values, no fork): the engine's walks over every slot or owner
(``_live_slots``, ``_kill_room``, the freeze counts, the room change's bookkeeping), a verb's slot test in VerbOps
and saveVerbs, and endCutscene's test for an armed override. A fork there would be a product over every script.
What they cannot decide becomes a guessed effect, counted by reason. ``clear`` of one unit's wait points or override
writes one mark, ``at:<unit>:*`` = 0, which ``read`` consults (a room change clears every unit of the room).

**Counters stepped by ops** (``STEP_OPS``: the cutscene depth, VAR_USERPUT, VAR_CURSORSTATE, which cutscene,
endCutscene and the cursor ops step, script_v5.cpp:917-938, script.cpp:1632-1687) are kept as a step until something
else reads them: a cutscene opened and closed in one path changes nothing, and an open one is an ``add`` effect, not
a fork over the depth. endCutscene with no level open is the engine's error(), so a step down needs a depth of at
least 1 at the start, a precondition.

**Waits** (``Reader._wait``). A WaitFor op the run meets blocks: the slot yields there, and what follows is a later
event (D5). Only the wait an event resumes at falls through: that is the event, and its condition (the actor
stopped, the message gone) is a replay check. So no run assumes the ego already where it walks, and a click's
frame stops where the engine's would wait.

**Kinds** (plan step 1):

* ``sentence``: (verb, A) and (verb, A, B), through ``Machine.run_sentence``: the sentence script installed in the
  state (VAR_SENTENCE_SCRIPT, one variant per value of its domain), as the machine's dispatch runs it; the input
  script is not run (model.py). Verbs: those the scripts give a button (VerbOps New with Text and On, a constant
  number) and some object's event table names. A: every object the scope's rooms define, as a room object (room,
  owner OF_OWNER_ROOM, class Untouchable clear: findObject, object.cpp:557-597) and, when its owner's domain holds
  VAR_EGO's value, as an inventory object. B: the constants A's entry for the verb compares its first argument
  with that are objects of the scope, plus one "other" (``Scope.other_b``, a stand-in object, counted). Engine
  conditions (actions.usable): VAR_USERPUT > 0, ``verb:V`` shown, A reachable, and no slot of the sentence
  script's number live (checkAndRunSentenceScript holds the sentence otherwise, script.cpp:1186-1191);
  Two engine variants (``Builder._sentence_base``): no slot of the sentence script's number live, or the global one
  live and frozen ("~frozen": it does not hold the sentence, and runScript without R stops it first). B also ranges
  over actors the entry compares with (their reachability a replay check) and over the scope's objects with an
  entry for a verb the sentence script starts on B itself (``_b_verbs``: Give runs B's own entry, plan step 1). A
  room object's room is any room that loads its room resource (a pseudo room too); a parent's state is not in the
  index, so an object with a parent carries that as a replay check (actions.reachable's PARENT);
* ``walk_to``: the sentence with the walk verb on a room object, with no verb button. The walk verb is read from the
  scripts (``walk_verbs``: created with New, never turned On, named by an event table), not from the executor's
  profile (C2);
* ``dialog_line``: a left click on a verb the scripts create with New that no event table names (not the walk verb,
  not an inventory slot, SetToObject): the input script runs with [kVerbClickArea, V, 1] (``Machine._eng_input``,
  verbs.cpp:683-686, script.cpp:1478-1511), one variant per value of VAR_VERB_SCRIPT and per room of the scope. A
  sentence it queues is not run here: the sentence queue is not a fact without settle (a note);
* ``skip_cutscene``: Esc while an override is armed (``escpos:<unit>`` = W) and VAR_CUTSCENEEXIT_KEY is not 0
  (input.cpp:966, 1423-1424): abortCutscene (script.cpp:1689-1709) sets VAR_OVERRIDE = 1, uses the override up,
  sets the slot's freeze count to 0, lowers its cutsceneOverride by one (1703-1704), and the slot runs on from W in
  the same frame (processInput comes before runAllScripts, scumm.cpp:3177, 3231). The key code then goes on to
  checkExecVerbs and the input script (input.cpp:1427-1428): not run, listed on each record (``ESC_KEY``);
* ``event``: a running script's next turn, plan step 1 with D5: its slot live, at W, not frozen, plus what the turn
  tests (``_eng_resume_at``: runAllScripts, script.cpp:968-987, from W to the next yield or the end). Events are
  actions of their own, which the planner interleaves with the player's: nothing runs the scripts to a fixed point.
  After a delay, the event is the side where the delay is over (a replay check); a turn that changes nothing (a
  poll whose test still fails) is no action, counted. The engine runs a slot's turn every frame whether the plan
  picks its event or not: the planner may put off an event the engine will run anyway (a timer's), so replay
  (step 5) has to expect events that happen without being chosen.

**Locals across a yield** (an addition to the plan, counted). The plan makes them non-facts. But a slot an action
started has the locals its path gave it, and the sentence script's later part needs them: without its verb and
objects, ``startObject(Local[1], Local[0], ...)`` has no object and no handler would ever run. So a path that leaves
a slot waiting with known locals writes ``args:<unit>`` = the number of that list (48 bits of its SHA-1, collisions
counted), the list also holding the cutscenes the slot began and their data (endCutscene hands it to the end hook,
script.cpp:1686), and the slot's events are built per list, with the precondition ``args:<unit>`` = its number. A
slot live at the start has unknown locals: its events have ``args:<unit>`` = 0, as the plan says. Past
``SIG_LIMIT`` lists for one wait point, further lists are folded into 0, counted. A replan starts from the
engine's state, whose slot dump has no locals (the bridge's), so every wait point an action leaves a slot at also
gets its event with unknown locals (args 0), and ``replan_start`` copies a slot's ``args:`` from the plan's
prediction when the slot is where the plan put it.

**Slot positions.** facts.py's ``at:<unit>:W`` and ``esc:<unit>:W`` are written to the task as one fact per unit,
``pos:<unit>`` and ``escpos:<unit>``, the offset or NOWHERE (a slot waits at one place: ``Builder.positions``,
``task_start``); an event's other wait points are taken as 0 (one instance per unit, assumed).

**Domains** are domains.py's (``domains.build(start)``), with these additions, each counted: the hook variables
that domains.py leaves out as copies get their start value and the constants written to them (a copy is taken to
restore one of them, domains.py's own assumption); VAR_USERPUT and VAR_CURSORSTATE, which only the cursor ops write
(o5_cursorCommand, script_v5.cpp:917-938), get the start, 0 and 1 widened by 1 each way, BELOW and ABOVE past that
(a cutscene nested deeper than that makes their steps guesses, ``Domain.add``); ``room`` gets the scope's rooms, the start's and every constant room change in the scope;
``args:``, ``pos:``, ``escpos:`` and ``saved:`` are the reader's own; and a value written outside an exact domain is added to
it, after which the whole reading runs again (``read_all`` passes). A collapsed value (ABOVE, BELOW) is read as its
representative, ``high + 1`` or ``low - 1``, as ``Domain.test`` decides it; a step on it is a guess
(``Domain.add``), counted.

**Conditional writes** (``Cond``, ``Copy``): a write that depends on a binary fact the path did not read is kept as
one, and the task writes it as conditional effects: ``Cond`` (key := v where the fact is 1, unchanged elsewhere),
``Copy`` (key := the fact's value).

**Freezes** (``Reader._freeze``, ``_unfreeze``). freezeScripts adds 1 to every live slot's count (script.cpp:914-941),
but a slot's liveness off the stack is read lazily: its ``frozen:`` becomes 1 where its slot fact is 1 (a ``Cond``,
counted). When the same path then unfreezes it as often (unfreezeScripts, script.cpp:943-966), its ``frozen:`` goes
back to its value before. An unfreeze of a slot frozen before the path (its count is not a fact) takes the count as 1,
so the slot is unfrozen: an assumption, counted (a cutscene nested in another would leave it frozen).

**Saved verbs** (``Reader._op_saveVerbs``, ``_op_restoreVerbs``). saveVerbs(a, b, id) hides the verbs a..b with save
id 0 and restoreVerbs(a, b, id) shows those saved with ``id`` again (verbs.cpp saveVerbs, restoreVerbs). The reader
keeps whether each verb was shown when it was saved as ``saved:<verb>:<id>`` (its own binary fact: a ``Copy`` of
``verb:V``), and a restore shows the verb where it is 1 (a ``Cond``). VerbOps On on a verb whose slot is not known,
and a restore of a range that does not fold, stay guesses (the machine's rule).

**Replay checks** are the conditions on non-facts a record's path took one side of. A condition whose two sides are
both on one record (branches the machine merged, or one test met twice in a frame with an op between that may change
its value, ``MEMO_GETTERS``) has no single side to check: it is listed apart (``Record.either``) and counted, not
checked.

**The inventory hook** (``Reader._eng_script``: the script VAR_INVENTORY_SCRIPT holds, wherever it is started, by
the engine or by a script) is not run: it draws the inventory from the engine's inventory list, which is not a fact
(model.py), and its loops over unknown counts fork on every slot. Its writes (the variables, bits and verbs its code
names, 16 variables after an indexed write's base, domains.py's assumption, and what the object entries it starts
write, ``entry_writes``) are guessed, counted. getInventoryCount and findInventory are non-facts for the same reason.

**Writes whose target does not fold** (model.py's ``unknown_writes``) are counted by statement and site, listed on
their record (``Record.unknown``) for replay, and left out of the task: domains.py's assumption, that they reach no
fact the task tracks. Where the code bounds the facts such a write may hit (``Builder.unknown_targets``: verbs the
scripts create, the scope's objects, the objects with an entry for a constant verb) replay knows which to check.
They are not guessed: a guess on every candidate is a free write the planner takes. Tried on the kitchen scope, the
main plan opened door 316 through an ambient animation's drawObject on an unknown object (local 206) instead of the
cook's scripts.

**The task** (``build_task``, ``prune``, ``task_start``): one task.py Action per record; a step (``add``) needs room in
its domain (a precondition and the effect's condition: beginCutscene past kMaxCutsceneNum is the engine's error(),
script.cpp:1632-1646). ``prune`` keeps relevance's actions and drops their effects on facts no kept precondition,
effect condition or goal reads (relevance.py's own argument: such a write changes no plan).

**The scope** (``Scope``): the units of the caller's rooms and every global script they reach (constant starts, hook
variables' constants, slots live at the start). A path whose room change leaves the scope's rooms ends there, "left
the scope", counted and no action: the task has nothing in those rooms.

**Limits**, each counted: ``PATH_LIMIT`` paths per run (forks past it end the path, "limit"; a run that hits it is
run again once per value of its first fork on a read, each part with its own limit, down to SPLIT_DEPTH levels and
SPLIT_BUDGET runs, ``Builder.run``), the machine's STEP_LIMIT and nesting limit, MAX_VALUES (value sets made plain
unknowns), ``SIG_LIMIT``. Path ends that are no action (``DROP_ENDS``): the engine's errors,
spins, limits, unmodelled statements, "deferral unknown", "left the scope", "no value" (a fact with no value left).

What model.Machine lacks that this module adds (to move into model.py later): resuming a slot with given locals,
its cutscenes and its wait (``_eng_resume_at``); the Esc rule without settle and without the key's input-script run
(``Event.abort``, ``ESC_KEY``; model.py's ``run_esc``, row 5, runs both with settle: the two are still to be
reconciled); the fork on read and the lazy reads; the join on assumptions; one frame's memory of
non-fact tests (``test``); the record of a finished frame's locals (``_finish``); the step counters; the clear mark;
the conditional writes for freezes and saved verbs; the inventory hook's summary. ``check_machine`` checks at import
that every Machine method this module wraps, overrides or calls by name still exists with the same parameters.

C2: nothing here names a game's object, script, room, verb or variable; the numbers are ScummVM's, cited. The rooms
and the goal are the caller's; the walk verb is read from the scripts.
"""

import collections
import dataclasses
import functools
import hashlib
import inspect
import multiprocessing
import time
from dataclasses import dataclass, field

import domains as D
import ir
import model as M
import task as T
from ir import Assign, Bit, Cmp, Const, Embed, Index, Jump, List, Local, Not, Op, Start, Truth, Var
from model import Call, Machine, Oracle, Unknown, slot_name, unit_name

ROW = frozenset({"own_entry", "dispatch", "calls", "room_change"})  # the click's frame; no settle (D5)
R_FACT = "fact"  # a fact a lazy read did not fork on (its values in ``Unknown.values``)
R_NONFACT = "not a fact"  # a variable with no finite domain, or one the engine changes as frames pass
R_COLLAPSED = "collapsed value"  # arithmetic on ABOVE or BELOW
POS_KINDS = {"pos": "at", "escpos": "esc"}  # one fact per unit for facts.py's at: and esc: keys (``Builder.positions``)
NOWHERE = -1  # pos: and escpos: of a unit with no live slot, or no override armed  # a value the engine's walks over every slot or owner left open
VAR_CUTSCENEEXIT_KEY = 24  # vars.cpp:60
VAR_OVERRIDE = M.VAR_OVERRIDE
VERB_CLICK_AREA, LEFT_CLICK = M.VERB_CLICK_AREA, M.LEFT_CLICK
CURSOR_VARS = (M.VAR_CURSORSTATE, M.VAR_USERPUT)  # o5_cursorCommand's (script_v5.cpp:917-938)
CURSOR_OPS = {"UserputOn": (M.VAR_USERPUT, "=", 1), "UserputOff": (M.VAR_USERPUT, "=", 0),
              "UserputSoftOn": (M.VAR_USERPUT, "+", 1), "UserputSoftOff": (M.VAR_USERPUT, "+", -1),
              "CursorShow": (M.VAR_CURSORSTATE, "=", 1), "CursorHide": (M.VAR_CURSORSTATE, "=", 0),
              "CursorSoftOn": (M.VAR_CURSORSTATE, "+", 1), "CursorSoftOff": (M.VAR_CURSORSTATE, "+", -1)}
PATH_LIMIT = 1024  # paths per run (model.PATH_LIMIT is 256): forks past it end the path with "limit", counted
SIG_LIMIT = 1024  # lists of locals per wait point past which a slot's locals are taken as unknown, counted
LAZY_METHODS = ("may_stop", "_live_slots", "_set_freeze", "_kill_room", "_eng_scene_begin",
                "_eng_scene_load", "_eng_scene_ego", "_eng_scroll_note", "_op_VerbOps")
SENTENCE_LOCALS = {"verb": 0, "a": 1, "b": 2}  # checkAndRunSentenceScript's runScript(..., [verb, A, B]) (script.cpp:1240-1246)
SPLIT_DEPTH = 2  # a run past PATH_LIMIT is split on its first fork on a read at most this deep (``Builder.run``)
SPLIT_BUDGET = 4  # and into at most this many runs per input or event, counted when used up
# Non-fact getters whose value one engine frame does not change unless an op of the frame moves an actor
# (``Reader.test``): the getter and the ops after which a value read before is no longer known. Actors move in
# walkActors after runAllScripts (scumm.cpp:3231, 3284), so within a frame only an op that puts an actor somewhere,
# changes its room, its costume, its facing or its walk changes them. findObject, actorFromPos (the mouse and every
# object's state) and getRandomNr (a new draw each call) are not kept.
MEMO_GETTERS = {"getActorRoom": ("putactor", "loadroom"),
                "getActorX": ("actor", "loadroom"), "getActorY": ("actor", "loadroom"),
                "getActorWalkBox": ("actor", "loadroom"), "getActorElevation": ("actor", "loadroom"),
                "getDist": ("actor", "loadroom", "drawobject"), "getActorCostume": ("actor", "loadroom", "animate"),
                "getActorFacing": ("actor", "loadroom", "animate"), "getActorMoving": ("actor", "loadroom", "animate"),
                "getActorWidth": ("actor", "loadroom")}
FLIP = {"==": "==", "!=": "!=", "<": ">", "<=": ">=", ">": "<", ">=": "<="}
# The Esc key's code goes on to checkExecVerbs and the input script after abortCutscene (input.cpp:1427-1428,
# verbs.cpp:586-664). The reader does not run that part: the input script dispatches on the sentence it is building
# (variables no fact holds), so every Esc would fork on them. Run that way (``Machine._eng_key`` after the slot's turn)
# the kitchen scope's Esc records tripled to 29,945 and made 770k of its 1.12M operators, more than the machine could
# write or plan. Each Esc record lists it as a write left out (``Record.unknown``), for replay to check.
ESC_KEY = ("runInputScript", "input.cpp:1427-1428", "the Esc key code: checkExecVerbs and the input script, not run")


# A counter stepped by an op whose value nobody has read yet (``Reader._step_ops``): an Unknown whose values are
# {d, d + STEP_BIG}, so that model._arith's +1 and -1 keep it a two-value set and its least value is the step d.
R_STEP = "step"
STEP_BIG = 1 << 20
STEP0 = Unknown(R_STEP, "", frozenset({0, STEP_BIG}))
STEP_OPS = {"_op_cutscene": "cutscene", "_op_endCutscene": "cutscene",
            "_op_UserputSoftOn": f"var:{M.VAR_USERPUT}", "_op_UserputSoftOff": f"var:{M.VAR_USERPUT}",
            "_op_CursorSoftOn": f"var:{M.VAR_CURSORSTATE}", "_op_CursorSoftOff": f"var:{M.VAR_CURSORSTATE}"}


def is_step(v) -> bool:
    return isinstance(v, Unknown) and v.reason == R_STEP


def step_of(v) -> int:
    return min(v.values)


R_COND, R_COPY = "conditional write", "copy"


@dataclass(frozen=True)
class Cond(Unknown):
    """A write that happens only where binary fact ``what`` is 1 at the action's start: key := the one value in
    ``values`` there, unchanged elsewhere (a freeze of a slot whose liveness is read lazily, a restoreVerbs)."""


@dataclass(frozen=True)
class Copy(Unknown):
    """key := the value binary fact ``what`` has at the action's start (saveVerbs)."""


def _consistent(tests) -> bool:
    """Whether some integer passes every test in ``tests``, (((op, constant), side), ...): ``side`` False is the
    test's negation."""
    lo, hi, eq, ne = -(1 << 40), 1 << 40, set(), set()
    for (op, k), side in tests:
        if not side:
            op = {"==": "!=", "!=": "==", "<": ">=", "<=": ">", ">": "<=", ">=": "<"}[op]
        if op == "==":
            eq.add(k)
        elif op == "!=":
            ne.add(k)
        elif op == "<":
            hi = min(hi, k - 1)
        elif op == "<=":
            hi = min(hi, k)
        elif op == ">":
            lo = max(lo, k + 1)
        else:
            lo = max(lo, k)
    if eq:
        return len(eq) == 1 and lo <= next(iter(eq)) <= hi and not eq & ne
    return hi - lo + 1 > len({k for k in ne if lo <= k <= hi})


def _live_bound(u, env: dict) -> list:
    """domains.live from the unit's start, with the locals ``env`` ({n: frozenset}) bound too: a jump whose condition
    the settings and those locals decide goes one way."""
    settings = {n: frozenset({v}) for n, v in D.SETTINGS.items()}
    seen, todo = set(), [0]
    while todo:
        i = todo.pop()
        if i in seen or i >= len(u.ins):
            continue
        seen.add(i)
        x = u.ins[i]
        if isinstance(x.st, Jump) and x.st.cond is not None:
            d = None
            match x.st.cond:
                case Cmp(op, a, b):
                    sa, sb = D.fold(a, env, settings), D.fold(b, env, settings)
                    if sa is not None and sb is not None:
                        r = {M.CMP[op](M.i16(y), M.i16(z)) for y in sa for z in sb}
                        d = r.pop() if len(r) == 1 else None
                case Truth(a) | Not(a):
                    s = D.fold(a, env, settings)
                    if s is not None:
                        r = {(v != 0) == isinstance(x.st.cond, Truth) for v in s}
                        d = r.pop() if len(r) == 1 else None
            if d is not None:
                todo.append(x.next if d else x.target)
                continue
        todo += [j for j in (x.next, x.target, x.esc) if j is not None]
    return [u.ins[i] for i in sorted(seen)]


class Fork(Exception):
    """A step read a fact with more than one possible value: re-run it once per value (``Reader._advance``)."""

    def __init__(self, key: str):
        super().__init__(key)
        self.key = key


# --- what the scope's code can write ---------------------------------------------------------------------------

@dataclass
class Writes:
    """The facts some code of the scope can write, read from the IR. ``exact``: keys; ``wild``: key prefixes a write
    whose target does not fold may hit (``state:``, ``class:``, ``var:`` from a base, ...). Slot facts of the scope's
    units are always written (their events run them)."""
    exact: set = field(default_factory=set)
    wild: set = field(default_factory=set)
    var_windows: list = field(default_factory=list)  # (base, cite): an indexed variable write whose index does not
    # fold; taken to hit the variables in base..base+16 (domains.py's own assumption)
    classes: set = field(default_factory=set)  # classes C a setClass on an object that does not fold sets or clears
    counts: collections.Counter = field(default_factory=collections.Counter)

    def has(self, key: str) -> bool:
        if key in self.exact:
            return True
        if any(key.startswith(w) for w in self.wild):
            return True
        if key.startswith("class:") and int(key.rsplit(":", 1)[1]) in self.classes:
            return True
        if key.startswith("var:"):
            n = int(key[4:])
            return any(b <= n < b + 16 for b, _ in self.var_windows)
        return False


def scope_writes(game: M.Game, units: dict, starts: dict | None = None) -> Writes:
    """``Writes`` of the units ``units`` ({key: ir.Unit}), with their locals bound from the call sites in them
    (domains.py's rule); ``starts`` ({key: instruction indices}) reads a unit only from those (one entry of an
    object)."""
    out = Writes()
    code = {k: D.live(u, {n: frozenset({v}) for n, v in D.SETTINGS.items()}, (starts or {}).get(k))
            for k, u in units.items()}
    rd = D._Reader(units, code, out.counts, [])
    env, _ = rd.locals(rd.hook_units())

    def objs(e, key):
        s = D.fold(e, env.get(key, {}))
        return None if s is None else sorted(s)

    def target(e, key, kind):
        match e:
            case Var(n):
                out.exact.add(f"var:{n}")
            case Bit(n):
                out.exact.add(f"bit:{n}")
            case Index(k, base, off) if k in ("var", "bit"):
                s = D.fold(off, env.get(key, {}))
                if s is None:
                    if k == "var":
                        out.var_windows.append((base, kind))
                    else:
                        out.wild.add("bit:")
                    out.counts[f"indexed {k} writes whose index does not fold"] += 1
                else:
                    out.exact.update(f"{k}:{base + v}" for v in s)
    for key, xs in code.items():
        for x in xs:
            st = x.st
            for y in D._walk(st):
                if isinstance(y, Embed):
                    target(y.st.lhs, key, x.cite())
            if isinstance(st, Assign):
                target(st.lhs, key, x.cite())
            elif isinstance(st, Op):
                a = st.args
                if st.name == "setVarRange":
                    first = a[0]
                    if isinstance(first, (Var, Bit)):
                        kind = "var" if isinstance(first, Var) else "bit"
                        out.exact.update(f"{kind}:{first.n + i}" for i in range(len(a[2].items)))
                    elif not isinstance(first, Local):
                        target(first, key, x.cite())
                elif st.name in ("setState", "drawObject", "pickupObject", "setOwnerOf", "setClass"):
                    os_ = objs(a[0], key)
                    if st.name == "setClass":
                        cls = [D.fold(c, env.get(key, {})) for c in a[1].items]
                        cs = None if any(c is None for c in cls) else sorted({v & 0x7F for c in cls for v in c})
                        if os_ is None:
                            out.counts["setClass on an object that does not fold"] += 1
                            if cs is None or 0 in cs:
                                out.wild.add("class:")
                            else:
                                out.classes.update(cs)  # class C of any object
                        else:
                            for o in os_:
                                if cs is None or 0 in cs:
                                    out.wild.add(f"class:{o}:")
                                else:
                                    out.exact.update(f"class:{o}:{c}" for c in cs)
                        continue
                    kinds = {"setState": ("state",), "drawObject": ("state",), "setOwnerOf": ("owner",),
                             "pickupObject": ("owner", "state")}[st.name]
                    if os_ is None:
                        out.counts[f"{st.name} on an object that does not fold"] += 1
                        out.wild.update(f"{k}:" for k in kinds)
                        if st.name == "pickupObject":
                            out.classes.add(M.CLASS_UNTOUCHABLE)
                    else:
                        for o in os_:
                            out.exact.update(f"{k}:{o}" for k in kinds)
                            if st.name == "pickupObject":
                                out.exact.add(f"class:{o}:{M.CLASS_UNTOUCHABLE}")
                    if st.name == "drawObject":  # it clears objects drawn at the same place (script_v5.cpp:1122-1126)
                        r = units[key].room
                        out.exact.update(f"state:{o}" for o, rr in game.object_room.items() if rr == r)
                elif st.name in ("VerbOps", "saveVerbs", "restoreVerbs"):
                    vs = objs(a[0], key)
                    if vs is None or st.name != "VerbOps":
                        out.wild.add("verb:")
                    else:
                        out.exact.update(f"verb:{v}" for v in vs)
                elif st.name in CURSOR_OPS:
                    out.exact.add(f"var:{CURSOR_OPS[st.name][0]}")
                elif st.name in ("print", "printEgo"):
                    out.exact.add(f"var:{M.VAR_KEYPRESS}")
    # the engine's own writes inside opcodes the scope runs (model.py's rules): rooms, cutscenes, overrides, the
    # walk-to object, the camera bounds, stopTalk
    out.exact.update({"room", "cutscene", "ego", f"var:{M.VAR_ROOM}", f"var:{M.VAR_ROOM_RESOURCE}",
                      f"var:{M.VAR_NEW_ROOM}", f"var:{M.VAR_ROOM_FLAG}", f"var:{M.VAR_WALKTO_OBJ}",
                      f"var:{M.VAR_OVERRIDE}", f"var:{M.VAR_CAMERA_MIN_X}", f"var:{M.VAR_CAMERA_MAX_X}",
                      f"var:{M.VAR_TALK_ACTOR}", f"var:{M.VAR_RESULT}"})
    out.wild.update({"esc:", "frozen:"})
    for k in units:
        if k[0] == "global":
            out.exact.add(f"script:{k[1]}")
        elif k[0] == "local":
            out.exact.add(f"local:{k[2]}")
        elif k[0] == "obj":
            out.exact.add(f"objscript:{k[1]}")
        out.wild.add(f"at:{unit_name(k)}:")
    return out


_ENTRY_WRITES = {}  # (id(game), verb) -> ``entry_writes``


def entry_writes(game: M.Game, verb: int) -> tuple:
    """The facts the ``verb`` entries of every object of the game write by a constant number (variables, bits, verbs,
    owners, states, classes): what a startObject(O, verb) whose O does not fold may write (``Builder.unknown_targets``,
    the inventory hook's summary). Each entry is read from its own start (not the whole object). Cached."""
    key = (id(game), verb)
    if key not in _ENTRY_WRITES:
        engine = scope_writes(game, {}).exact
        keys = set()
        for k, u in game.units.items():
            if k[0] == "obj" and verb in u.entries:
                w = scope_writes(game, {k: u}, {k: [u.entries[verb]]})
                keys |= {x for x in w.exact - engine
                         if x.startswith(("var:", "bit:", "verb:", "owner:", "state:", "class:"))}
        _ENTRY_WRITES[key] = tuple(sorted(keys))
    return _ENTRY_WRITES[key]


# --- the scope ----------------------------------------------------------------------------------------------------

def made_verbs(game: M.Game) -> dict:
    """{verb: the VerbOps sub-op names any script gives it} for every verb number some script creates with New (a
    constant number)."""
    made = {}
    for u in game.units.values():
        for x in u.ins:
            st = x.st
            if isinstance(st, Op) and st.name == "VerbOps" and isinstance(st.args[0], Const):
                made.setdefault(st.args[0].value, set()).update(s.name for s in st.subops)
    return {v: n for v, n in made.items() if "New" in n}


def walk_verbs(game: M.Game) -> list:
    """The walk verb, read from the scripts (plan.md C2; the executor's profile is the replay harness's): a verb some
    script creates with New that no script turns On, so it never has a button (findVerbAtPos takes shown verbs only,
    verbs.cpp:1009-1032), and that some object's event table names. A click on the room with no verb chosen gives it,
    the input script's default (actions.py, walk_to). The list of such verbs; ``Scope.of`` takes one only when the list
    has one (counted)."""
    evs = {v for k, u in game.units.items() if k[0] == "obj" for v in u.entries}
    return sorted(v for v, n in made_verbs(game).items() if "On" not in n and v in evs)


def stop_points(u: ir.Unit) -> list:
    """The offsets where a slot of unit ``u`` can wait (facts.py ``at:``): after a breakHere or a delay, at a WaitFor
    op, right after any other opcode (one that runs a script nested parks its caller there: a superset)."""
    out = set()
    for x in u.ins:
        if x.yields == "wait":
            out.add(x.offset)
        if x.next is not None and x.next < len(u.ins) and not isinstance(x.st, (Assign, Jump)):
            out.add(u.ins[x.next].offset)
    return sorted(out)


@dataclass
class Scope:
    """What the reader builds actions for: the units of the rooms given and every global script they reach (a
    constant start, a hook variable's constant), the objects those rooms define, and the verbs."""
    rooms: tuple
    units: dict  # key -> ir.Unit
    objects: list  # objects the rooms define
    verbs: list  # sentence verbs: a button (VerbOps New, Text, On) and in some event table
    dialog_verbs: list  # verbs created with New that no event table names (not the walk verb, not SetToObject)
    walk_verb: int | None
    start: dict  # the facts at the start (a facts.py map)
    writes: Writes = None
    universe: list = field(default_factory=list)  # every slot fact key the oracle lists (``SymOracle.keys``)
    counts: collections.Counter = field(default_factory=collections.Counter)

    @classmethod
    def of(cls, game: M.Game, rooms, start: dict) -> "Scope":
        """The scope of ``rooms`` from the start ``start``; the walk verb comes from the scripts (``walk_verbs``)."""
        rooms = tuple(sorted(rooms))
        walks = walk_verbs(game)
        walk_verb = walks[0] if len(walks) == 1 else None
        units = {k: u for k, u in game.units.items() if
                 (k[0] in ("local", "entry", "exit") and k[1] in rooms) or
                 (k[0] == "obj" and game.object_room.get(k[1]) in rooms)}
        hooks = {start.get(f"var:{n}", 0) for n in M.HOOK_VARS} - {0}  # hook scripts installed at the start
        todo = [("global", n) for n in sorted(hooks) if n < game.first_local and ("global", n) in game.units]
        live = [k for k in start if k.startswith("script:") and start[k] == 1]  # global slots live at the start
        todo += [("global", int(k[7:])) for k in live]
        todo += list(units)
        seen = set()
        counts = collections.Counter()
        while todo:
            k = todo.pop()
            if k in seen or k not in game.units:
                continue
            seen.add(k)
            if k[0] == "global":
                units[k] = game.units[k]
            for x in game.units[k].ins:
                st = x.st
                if isinstance(st, Start) and st.name != "startObject":
                    if isinstance(st.script, Const):
                        n = st.script.value
                        if 0 < n < game.first_local:
                            todo.append(("global", n))
                    else:
                        counts["starts with a script number that does not fold (callee not in the scope)"] += 1
                elif isinstance(st, Assign) and isinstance(st.lhs, Var) and st.lhs.n in M.HOOK_VARS:
                    if isinstance(st.rhs, Const) and 0 < st.rhs.value < game.first_local:
                        todo.append(("global", st.rhs.value))
        counts["walk verb candidates (walk_verbs; one is taken only when there is one)"] = len(walks)
        objects = sorted(o for o, r in game.object_room.items() if r in rooms)
        evs = collections.Counter()
        for k, u in game.units.items():
            if k[0] == "obj":
                evs.update(u.entries)
        made = made_verbs(game)
        verbs = sorted(v for v, n in made.items() if {"Text", "On"} <= n and evs.get(v) and v != walk_verb)
        made_here = {x.st.args[0].value for u in units.values() for x in u.ins
                     if isinstance(x.st, Op) and x.st.name == "VerbOps" and isinstance(x.st.args[0], Const)
                     and "New" in {s.name for s in x.st.subops}}  # created by the scope's own code
        dialog = sorted(v for v, n in made.items() if not evs.get(v) and v != walk_verb and "SetToObject" not in n
                        and v in made_here)
        sc = cls(rooms, units, objects, verbs, dialog, walk_verb, start, counts=counts)
        sc.writes = scope_writes(game, units)
        uni = set()
        for k, u in units.items():
            name = unit_name(k)
            uni.update(f"at:{name}:{w}" for w in stop_points(u))
            uni.update(f"esc:{name}:{u.ins[x.esc].offset}" for x in u.ins if x.esc is not None and x.esc < len(u.ins))
            lk = M.lifecycle_key(k, game)
            if lk:
                uni.add(lk)
        uni.update(k for k in start if k.startswith(("at:", "esc:", "script:", "local:", "objscript:"))
                   and start[k] == 1)
        sc.universe = sorted(uni)
        return sc

    def other_b(self, a: int, consts: set, game: M.Game):
        """B = "other": the lowest object of A's room with no number among ``consts`` (a stand-in, counted)."""
        room = game.object_room.get(a)
        return next((o for o in self.objects if game.object_room.get(o) == room and o != a and o not in consts), None)


# --- facts --------------------------------------------------------------------------------------------------------

class FactTable:
    """How the reader reads each fact: its class (``kind``), its domain, and how a domain value is a machine value."""

    def __init__(self, scope: Scope, doms: D.Domains, game: M.Game):
        self.scope, self.doms, self.game = scope, doms, game
        # the engine variables the run's settings fix (domains.SETTINGS: VAR_DEBUGMODE is 0 with no debug flag,
        # scumm.cpp:3391) are constants with that value, as domains.py reads them
        self.start = dict(scope.start, **{f"var:{n}": v for n, v in D.SETTINGS.items()})
        self.extra = {}  # the reader's domains (module docstring)
        self.counts = collections.Counter()
        self.args_vals = collections.defaultdict(set)  # args:<unit> -> its list numbers
        w = scope.writes
        for n in M.HOOK_VARS:
            k = f"var:{n}"
            if doms.get(k) is None:
                vals = {self.start.get(k, 0)}
                for u in scope.units.values():
                    for x in u.ins:
                        if isinstance(x.st, Assign) and x.st.lhs == Var(n) and isinstance(x.st.rhs, Const):
                            vals.add(x.st.rhs.value)
                self.extra[k] = D.Domain.of(vals)
                self.counts["hook variables given the domain start + constant writes (reader)"] += 1
        for n in CURSOR_VARS:
            k = f"var:{n}"
            seeds = {self.start.get(k, 0), 0, 1}
            lo, hi = min(seeds) - 1, max(seeds) + 1
            self.extra[k] = D.Domain.of(range(lo - 1, hi + 2), lo, hi)
            self.counts["cursor variables given a window of +-1 around start, 0 and 1 (reader)"] += 1
        rooms = set(scope.rooms) | {self.start.get("room", 0)}
        wild = 0
        for u in scope.units.values():
            for x in u.ins:
                if isinstance(x.st, Op) and x.st.name in ("loadRoom", "loadRoomWithEgo"):
                    a = x.st.args[0 if x.st.name == "loadRoom" else 1]
                    if isinstance(a, Const):
                        rooms.add(a.value)
                    else:
                        wild += 1
        self.counts["room changes in the scope that do not fold (a guess among the scope's rooms)"] = wild
        self.extra["room"] = D.Domain.of(rooms)
        for n in (M.VAR_ROOM, M.VAR_ROOM_RESOURCE, M.VAR_NEW_ROOM):
            self.extra[f"var:{n}"] = D.Domain.of(rooms | {self.start.get(f"var:{n}", 0)})
        self.engine = set(D.ENGINE) | set(M.TIME_VARS) | set(M.MOUSE_VARS)

    def domain(self, key: str):
        if key.startswith("args:"):
            return D.Domain.of({0} | self.args_vals[key])
        if key.startswith("saved:"):
            return D.BINARY
        if key.startswith(tuple(f"{k}:" for k in POS_KINDS)):
            kind, name = key.split(":", 1)
            return D.Domain.of({NOWHERE} | set(self.points[POS_KINDS[kind]].get(name, ())))
        return self.extra.get(key) or self.doms.get(key)

    @functools.cached_property
    def points(self) -> dict:
        """{"at"/"esc": {unit name: offsets}} from the oracle's list and the start (``pos:`` and ``escpos:``)."""
        out = {"at": collections.defaultdict(set), "esc": collections.defaultdict(set)}
        for k in list(self.scope.universe) + [k for k in self.start if k.startswith(("at:", "esc:"))]:
            if k.startswith(("at:", "esc:")):
                kind, rest = k.split(":", 1)
                name, w = rest.rsplit(":", 1)
                out[kind][name].add(int(w))
        return out

    def kind(self, key: str) -> str:
        """"const", "fact" or "nonfact" (module docstring)."""
        if key.startswith(("args:", "saved:") + tuple(f"{k}:" for k in POS_KINDS)):
            return "fact"
        if key.startswith("var:") and int(key[4:]) in D.SETTINGS:
            return "const"
        if key.startswith("var:") and int(key[4:]) in self.engine:
            return "nonfact"
        if not self.scope.writes.has(key):
            return "const"
        return "fact" if self.domain(key) is not None else "nonfact"

    def value(self, key: str, label):
        """The machine value of a domain value: an int, or a collapsed value's representative (module docstring)."""
        if isinstance(label, int):
            return label
        dom = self.domain(key)
        rep = dom.high + 1 if label == D.ABOVE else dom.low - 1
        return Unknown(R_COLLAPSED, key, frozenset({rep}))

    def lazy(self, key: str, labels):
        dom = self.domain(key)
        labels = dom.values if labels is None else labels
        vals = set()
        for lab in labels:
            v = self.value(key, lab)
            vals |= set(v.values) if isinstance(v, Unknown) else {v}
        return Unknown(R_FACT, key, frozenset(vals))

    def label(self, key: str, v: int):
        dom = self.domain(key)
        return None if dom is None else dom.label(v)


class SymOracle(Oracle):
    """The oracle of a reader run: the run's fixed values (``pins``) first, then the constants, then an ``Unknown``
    (a fact's values, or a non-fact). It never forks: ``Reader.read`` does. It lists the scope's slot facts."""

    def __init__(self, table: FactTable, pins: dict, universe: list):
        self.table, self.pins, self.universe = table, pins, universe
        self.view = None  # a path's assumptions while the machine merges paths (``Reader._merge``)
        self._keys = {}

    def get(self, key: str):
        if key in self.pins:
            return self.pins[key]
        if self.view is not None and key in self.view and len(self.view[key]) == 1:
            return self.table.value(key, next(iter(self.view[key])))
        k = self.table.kind(key)
        if k == "const":
            return self.table.start.get(key, 0)
        if k == "fact":
            return self.table.lazy(key, self.view.get(key) if self.view else None)
        return Unknown(R_NONFACT, key)

    def keys(self, prefix: str) -> list:
        if prefix not in self._keys:
            self._keys[prefix] = [k for k in self.universe if k.startswith(prefix)] + \
                                 sorted(k for k in self.pins if k.startswith(prefix) and k not in self.universe)
        return self._keys[prefix]

    def tracks(self, kind: str) -> bool:
        return True


@dataclass
class SymPath(M.Path):
    pre: dict = field(default_factory=dict)  # fact -> frozenset of domain values the path assumed (preconditions)
    kept: dict = field(default_factory=dict)  # unit name -> (unit, W, locals, args known, R, F, owned, cutscenes it
    # began and has not ended, their cutscene data) of a slot it
    # left waiting
    fz: dict = field(default_factory=dict)  # unit name -> [freezes not yet undone, frozen: value before]
    resume_wait: tuple | None = None  # (unit, offset): the WaitFor op an event resumes at, which falls through
    nf: dict = field(default_factory=dict)  # (reason, what, mode) of a non-fact value -> the tests on it the path took
    # in this frame, ((op, constant), side) (``Reader.test``)

    def fork(self) -> "SymPath":
        q = super().fork()
        q.pre, q.kept, q.fz = dict(self.pre), dict(self.kept), {k: list(v) for k, v in self.fz.items()}
        q.nf = dict(self.nf)
        return q


def _to_sym(p: M.Path, pre: dict) -> SymPath:
    return SymPath(**{f.name: getattr(p, f.name) for f in dataclasses.fields(M.Path)}, pre=dict(pre))


# --- the records ------------------------------------------------------------------------------------------------------

@dataclass
class Event:
    """A slot's later part: unit ``unit`` (unit name ``name``) waiting at offset ``w`` with locals list ``sig``."""
    name: str
    unit: tuple
    w: int
    sig: int
    locals: dict
    args_known: bool
    recursive: object
    resistant: object
    owned: object
    abort: bool = False  # Esc: abortCutscene's resume at an override (``w`` the jump after beginOverride)
    override: int = 0  # cutscenes the slot began and has not ended (Frame.override, script.cpp:1634)
    cut_data: tuple = ()  # their cutSceneData, last on top (endCutscene gives it to the end hook, script.cpp:1686)

    def label(self) -> str:
        what = "esc" if self.abort else "event"
        return f"{what}({self.name}@{self.w}" + (f"#{self.sig:x}" if self.sig else "") + ")"


@dataclass
class Record:
    kind: str  # sentence, walk_to, dialog_line, skip_cutscene, event
    label: str
    pre: dict  # fact -> frozenset of domain values
    effects: dict  # fact -> int, Unknown (a guess), or ("copy", key)
    checks: list  # (cite, condition, side, why): conditions on non-facts, checked at replay
    end: str
    notes: list = field(default_factory=list)
    next_events: list = field(default_factory=list)  # Events this path leaves (continuations)
    source: tuple = ()  # what was run
    either: list = field(default_factory=list)  # (cite, condition) met on both sides (``Builder.record``)
    engine_pre: frozenset = frozenset()  # the precondition's facts that are the input's engine conditions (the rest
    # are what the scripts tested)
    unknown: list = field(default_factory=list)  # (statement, cite, target) writes whose target the reader cannot work
    # out, left out of the task and listed here for replay (``Builder.unknown_targets`` gives what they may hit)


class Reader(Machine):
    """model.Machine with the fork on read (module docstring). One run at a time."""

    def __init__(self, scope: Scope, doms: D.Domains, game: M.Game | None = None, path_limit: int = PATH_LIMIT):
        super().__init__(ROW, game, path_limit=path_limit)
        self.scope = scope
        self.table = FactTable(scope, doms, self.game)
        self.counts = collections.Counter()
        self.forks = collections.Counter()  # fact family -> forks
        self.fork_keys = collections.Counter()  # fact -> forks
        self._lazy = 0
        self._step_key = None
        self._seed = {}
        self.sigs = {}  # (name, unit, locals as text, args known) -> number
        self.sig_by_wait = collections.defaultdict(set)
        self.oracle = SymOracle(self.table, {}, scope.universe)
        self.scope_rooms = {self.game.resource(r) for r in scope.rooms}
        self.first_fork = None  # the first fact the current run forked on (``Builder.run`` splits on it)
        self._pending = None  # the non-fact test ``test`` just met, for ``_jump`` to record on each side

    # --- the fork on read -------------------------------------------------------------------------------------------

    def read(self, p, key, oracle):
        if not isinstance(p, SymPath):
            return p.writes[key] if key in p.writes else oracle.get(key)
        stepping = key == self._step_key
        if key in p.writes:
            v = p.writes[key]
            if is_step(v) and not stepping:  # a counter the path stepped is read: its value is needed now
                base = p.pre.get(key)
                if base is not None and len(base) == 1:
                    return self._stepped(key, next(iter(base)), step_of(v))
                raise Fork(key)
            return v
        if key.startswith(("at:", "esc:")):
            mark = key.rsplit(":", 1)[0] + ":*"
            if mark in p.writes:  # cleared by this path (``clear``)
                return p.writes[mark]
        if key in p.pre:
            s = p.pre[key]
            if len(s) == 1:
                return self.table.value(key, next(iter(s)))
            if stepping:
                return STEP0
            if self._lazy:
                return self.table.lazy(key, s)
            raise Fork(key)
        if key in self.oracle.pins:
            return self.oracle.pins[key]
        if key == f"var:{M.VAR_ROOM}":  # the engine writes it with the room (room.cpp:149, 155): the same fact
            return self.read(p, "room", oracle)
        if key == f"var:{M.VAR_ROOM_RESOURCE}":  # the room's resource (room.cpp:172)
            return self.game.resource(self.read(p, "room", oracle))
        kind = self.table.kind(key)
        if kind == "const":
            return self.table.start.get(key, 0)
        if kind == "fact":
            dom = self.table.domain(key)
            if len(dom.values) == 1:  # one value: no fork, no precondition
                return self.table.value(key, dom.values[0])
            if stepping:
                return STEP0
            if self._lazy:
                return self.table.lazy(key, None)
            raise Fork(key)
        return Unknown(R_NONFACT, key)

    def _stepped(self, key: str, base, d: int):
        """The value of a counter with domain value ``base`` after steps adding up to ``d``: a collapsed base is a
        guess (``Domain.add``), counted."""
        if isinstance(base, int):
            return base + d
        self.counts["steps on a collapsed value (a guess)"] += 1
        vals = self.table.domain(key).add(base, d)
        return self.table.lazy(key, vals) if len(vals) > 1 else self.table.value(key, next(iter(vals)))

    def _advance(self, p, oracle):
        if not isinstance(p, SymPath):
            return super()._advance(p, oracle)
        snap = p.fork()
        op = self._op_at(p) if p.nf else None
        unknown = len(p.unknown_writes)
        try:
            more = super()._advance(p, oracle)
        except Fork as f:
            key = f.key
            dom = self.table.domain(key)
            vals = [v for v in dom.values if v in p.pre[key]] if key in p.pre else list(dom.values)
            if self.first_fork is None:
                self.first_fork = key
            self.forks[M.family(key)] += 1
            self.fork_keys[key] += 1
            self.counts["forks on read"] += 1
            self.counts["paths from forks on read"] += len(vals)
            if not vals:  # no value of the fact is left: the path cannot happen
                self.counts[f"paths with no value left for a fact ({M.family(key)})"] += 1
                p.__dict__.update(snap.__dict__)
                self._halt(p, "no value", key)
                return []
            forks = []
            for i, v in enumerate(vals):
                q = snap if i == 0 else snap.fork() if i < len(vals) - 1 else snap.fork()
                q.pre[key] = frozenset({v})
                forks.append(q)
            first = forks[0]
            p.__dict__.update(first.__dict__)
            return forks[1:]
        if op is not None or len(p.unknown_writes) != unknown:
            for q in [p, *more]:
                if isinstance(q, SymPath) and q.nf:
                    self._forget(q, op, len(q.unknown_writes) != unknown)
        return more

    # --- non-fact values read twice in one frame ----------------------------------------------------------------------

    def _op_at(self, p):
        """The name of the op the path's next step runs, or None."""
        f = p.stack[-1] if p.stack else None
        if not isinstance(f, M.Frame):
            return None
        u = self.game.units.get(f.unit)
        if u is None or f.pc is None or f.pc >= len(u.ins):
            return None
        st = u.ins[f.pc].st
        return st.name if isinstance(st, (Op, Start)) else None

    def _forget(self, p: SymPath, op, var_writes: bool) -> None:
        """Drop the tests kept on values the op just run may have changed (MEMO_GETTERS); a write whose target does
        not fold may have hit a variable, so after one the tests on variables go too. Counted."""
        name = (op or "").lower()
        for ident in list(p.nf):
            what = ident[1]
            getter = what.split("(", 1)[0]
            subs = MEMO_GETTERS.get(getter)
            if (subs is not None and name and any(s in name for s in subs)) or \
                    (var_writes and what.startswith("var:")):
                del p.nf[ident]
                self.counts["non-fact tests forgotten after an op that may change them"] += 1

    def getter(self, p, name, args, oracle, x=None):
        """Machine.getter; a non-fact getter of MEMO_GETTERS whose arguments are known ints names them, so that a
        second read of the same value in the frame is the same value (``test``)."""
        v = super().getter(p, name, args, oracle, x)
        if isinstance(p, SymPath) and name in MEMO_GETTERS and isinstance(v, Unknown) and v.values is None \
                and v.what == name:
            vals = [self.val(p, a, oracle, x) for a in args if not isinstance(a, List)]
            if all(isinstance(a, int) for a in vals):
                return Unknown(v.reason, f"{name}({', '.join(map(str, vals))})")
        return v

    def _arith_at(self, p, op, a, b, x=None):
        """Machine._arith_at; model._arith hands back a plain unknown operand as the result, so a value computed
        from a non-fact (getActorX(a) + 10, Var[N] + 1) would keep that non-fact's name: it gets a name of its own,
        which ``_memo_ident`` does not keep."""
        out = super()._arith_at(p, op, a, b, x)
        if isinstance(out, Unknown) and out.values is None and isinstance(p, SymPath):
            return Unknown(out.reason, f"({out.what}) {op} ...")
        return out

    def val(self, p, e, oracle, x=None):
        """Machine.val; a negated non-fact gets a name of its own (``_arith_at``)."""
        v = super().val(p, e, oracle, x)
        if isinstance(e, ir.Neg) and isinstance(v, Unknown) and v.values is None and isinstance(p, SymPath):
            return Unknown(v.reason, f"-({v.what})")
        return v

    def _memo_ident(self, u: Unknown, mode: str):
        """The identity of a non-fact value one frame keeps: a MEMO_GETTERS getter with its arguments, or a variable
        with no finite domain that the engine does not change itself (``FactTable.engine``); None for the rest."""
        if u.values is not None:
            return None
        if u.reason == R_NONFACT and u.what.startswith("var:") and int(u.what[4:]) not in self.table.engine:
            return (u.reason, u.what, mode)
        if u.reason in (M.R_ACTOR, M.R_POSITION) and "(" in u.what and u.what.split("(", 1)[0] in MEMO_GETTERS:
            return (u.reason, u.what, mode)
        return None

    def test(self, p, c, oracle, x=None):
        """Machine.test; a test on a non-fact value the path already tested in this frame is decided by those tests
        when only one side agrees with them (a getter of an actor's room or position, a variable with no domain: one
        frame does not change them unless an op moves an actor or writes the variable, ``_forget``). Without this the
        path could take a test one way and the same test the other way a few nested runs later in the same frame (an
        actor's room read in a script and again in a script it starts), which the engine cannot do. Counted."""
        if not isinstance(p, SymPath):
            return super().test(p, c, oracle, x)
        self._pending = None
        match c:
            case Cmp(op, a, b):
                va, vb = self.val(p, a, oracle, x), self.val(p, b, oracle, x)
                out = M._decide(lambda y, z: M.CMP[op](M.i16(y), M.i16(z)), va, vb)
                pred = ((op, M.i16(vb)) if out is va and isinstance(vb, int) else
                        (FLIP[op], M.i16(va)) if out is vb and isinstance(va, int) else None)
                mode = "i16"
            case Not(a):
                v = self.val(p, a, oracle, x)
                out, pred, mode = M._decide(lambda y: y == 0, v), ("==", 0), "full"
                pred = pred if out is v else None
            case Truth(a):
                v = self.val(p, a, oracle, x)
                out, pred, mode = M._decide(lambda y: y != 0, v), ("!=", 0), "full"
                pred = pred if out is v else None
            case _:
                return super().test(p, c, oracle, x)
        if not isinstance(out, Unknown) or pred is None:
            return out
        ident = self._memo_ident(out, mode)
        if ident is None:
            return out
        have = p.nf.get(ident, ())
        sides = [s for s in (True, False) if _consistent(have + ((pred, s),))]
        if len(sides) == 1:
            self.counts["non-fact tests decided by the same test earlier in the frame"] += 1
            return sides[0]
        self._pending = (ident, pred)
        return out

    def _jump(self, p, x, st, oracle, idx=None):
        """Machine._jump; a fork on a non-fact test ``test`` keeps (``SymPath.nf``) records the side each path took."""
        self._pending = None
        more = super()._jump(p, x, st, oracle, idx)
        pend, self._pending = self._pending, None
        if more:
            self.counts["forks on non-fact tests"] += 1
        if pend is not None and isinstance(p, SymPath) and len(more) == 1 and isinstance(more[0], SymPath):
            ident, pred = pend
            p.nf[ident] = p.nf.get(ident, ()) + ((pred, True),)  # p goes on at x.next: the condition held
            more[0].nf[ident] = more[0].nf.get(ident, ()) + ((pred, False),)
        return more

    def _join(self, p):
        sig = super()._join(p)
        if sig is None or not isinstance(p, SymPath):
            return sig
        return sig + (tuple(sorted((k, tuple(sorted(map(str, v)))) for k, v in p.pre.items())),
                      tuple(sorted((k, step_of(v)) for k, v in p.writes.items() if is_step(v))),
                      tuple(sorted((k, v[0], v[1], v[3]) for k, v in p.kept.items())),
                      tuple(sorted((k, tuple(v)) for k, v in p.fz.items())))

    def _merge(self, group, sig):
        """The machine's merge, with the assumed values as the oracle's view (all paths of a group share them), and
        the locals of the slots each path left waiting joined as the machine joins a frame's (value sets)."""
        self.oracle.view = group[0].pre if isinstance(group[0], SymPath) else None
        try:
            q = super()._merge(group, sig)
        finally:
            self.oracle.view = None
        if isinstance(q, SymPath) and len(group) > 1:
            # the non-fact tests every merged path took the same way stay known; one that differs is forgotten, as
            # the machine's merge forgets which side of a branch ran (a later test forks again: ``Record.either``).
            # Where the merged paths hold some value differently, the machine joins it (model._join_values), and a
            # plain unknown there may stand for another value on one of them: then no kept test is trusted
            frames0 = [f.locals for f in self._frames(group[0])]
            same = all(g.writes == group[0].writes and [f.locals for f in self._frames(g)] == frames0
                       for g in group[1:])
            nf = {}
            for ident, tests in (group[0].nf.items() if same else ()):
                common = tuple(t for t in tests if all(t in g.nf.get(ident, ()) for g in group[1:]))
                if common:
                    nf[ident] = common
            if nf != group[0].nf:
                self.counts["non-fact tests forgotten where merged paths differ"] += 1
            q.nf = nf
            for name, (unit, w, locals_, known, rec, res, owned, ov, cd) in list(q.kept.items()):
                others = [g.kept[name][2] for g in group]
                joined = {n: M._join_values([o.get(n, 0 if known else Unknown(M.R_LOCAL, f"Local[{n}]")) for o in others])
                          for n in sorted(set().union(*others))}
                q.kept[name] = (unit, w, joined, known, rec, res, owned, ov, cd)
        return q

    def clear(self, p, prefix, x=None, head=""):
        """Machine.clear: every fact with the prefix that is not 0 goes to 0. For one unit's wait points or override
        (``at:<unit>:``, ``esc:<unit>:``) it is one mark, ``<prefix>*`` = 0, which ``read`` consults for the keys the
        path has not written since (a stop or a room change clears every unit of the room: one key each, not one
        per wait point)."""
        if isinstance(p, SymPath) and prefix.startswith(("at:", "esc:")) and prefix.count(":") == 2:
            for k in [k for k in p.writes if k.startswith(prefix)]:
                del p.writes[k]
            self.write(p, prefix + "*", 0)
            return
        self._lazy += 1
        try:
            return super().clear(p, prefix, x, head)
        finally:
            self._lazy -= 1

    def _op_saveVerbs(self, p, x, st, oracle):
        """saveVerbs(a, b, id) (verbs.cpp saveVerbs, script_v5.cpp:2420-2435): every verb in a..b with save id 0 gets
        save id ``id``, so it is not shown. The reader keeps whether it was shown as ``saved:<verb>:<id>`` (its own
        fact), which restoreVerbs reads."""
        a, b, sid = (self.val(p, e, oracle, x) for e in st.args)
        if not isinstance(p, SymPath) or any(isinstance(v, Unknown) for v in (a, b, sid)):
            return self._lazily("_op_saveVerbs", p, x, st, oracle)
        for v in range(a, b + 1):
            self._lazy += 1
            try:
                now = self.read(p, f"verb:{v}", oracle)
            finally:
                self._lazy -= 1
            self.write(p, f"saved:{v}:{sid}", now if isinstance(now, int) else
                       now if isinstance(now, Copy) else Copy(R_COPY, f"verb:{v}", frozenset({0, 1})))
            self.write(p, f"verb:{v}", 0)

    def _op_restoreVerbs(self, p, x, st, oracle):
        """restoreVerbs(a, b, id) (verbs.cpp restoreVerbs): the verbs in a..b saved with ``id`` get save id 0 again,
        so a saved verb that was shown is shown (``saved:<verb>:<id>``); the others stay as they are."""
        a, b, sid = (self.val(p, e, oracle, x) for e in st.args)
        if not isinstance(p, SymPath) or any(isinstance(v, Unknown) for v in (a, b, sid)):
            return super()._op_restoreVerbs(p, x, st, oracle)
        for v in range(a, b + 1):
            key = f"saved:{v}:{sid}"
            self._lazy += 1
            try:
                s = self.read(p, key, oracle)
            finally:
                self._lazy -= 1
            if isinstance(s, int):
                if s:
                    self.write(p, f"verb:{v}", 1)
            elif isinstance(s, Copy):  # saved in this path: back to the value it had at the start
                self.write(p, f"verb:{v}", s)
            else:
                self.write(p, f"verb:{v}", Cond(R_COND, key, frozenset({1})))
            self.write(p, key, 0)

    def _owned_by(self, p, owner, oracle):
        self.counts["inventory reads left unknown (getInventoryCount, findInventory)"] += 1
        return (None, Unknown(M.R_INVENTORY, "the inventory is not enumerated by the reader"))

    def _freeze(self, p, flag, x):
        """freezeScripts: the machine's rule, read lazily; a slot whose liveness is not known gets its freeze
        recorded so that a later unfreeze in the same path undoes it (module docstring)."""
        before = {k: p.writes.get(k, "unwritten") for k in p.writes if k.startswith("frozen:")} if isinstance(p, SymPath) else {}
        self._lazy += 1
        try:
            super()._freeze(p, flag, x)
        finally:
            self._lazy -= 1
        if not isinstance(p, SymPath):
            return
        for name, n in p.freeze.items():
            k = f"frozen:{name}"
            if n is None and isinstance(p.writes.get(k), Unknown):
                fz = p.fz.setdefault(name, [0, before.get(k, "unwritten")])
                fz[0] += 1
                p.freeze[name] = ("sym", fz[0])
                live = name if name.startswith(("script:", "local:")) else \
                    "objscript:" + name[4:] if name.startswith("obj:") else None
                if live is not None and not isinstance(p.writes.get(live), int):
                    p.writes[k] = Cond(R_COND, live, frozenset({1}))  # frozen where the slot is live
                    self.counts["freezes of a slot whose liveness is not known (frozen: where live)"] += 1

    def _unfreeze(self, p, x):
        self._lazy += 1
        try:
            if isinstance(p, SymPath):
                saved = {name: p.freeze.pop(name) for name in list(p.freeze) if name in p.fz}
            super()._unfreeze(p, x)
        finally:
            self._lazy -= 1
        if not isinstance(p, SymPath):
            return
        for name in list(p.fz):
            fz = p.fz[name]
            fz[0] -= 1
            if fz[0] > 0:
                p.freeze[name] = saved.get(name, None)
                continue
            k = f"frozen:{name}"
            if fz[1] == "unwritten":
                p.writes.pop(k, None)
            else:
                p.writes[k] = fz[1]
            p.freeze.pop(name, None)
            del p.fz[name]
            self.counts["freezes undone in the same path (frozen: restored)"] += 1
        # a slot frozen before this path (its count not known): taken as frozen once, so the unfreeze ends it
        # (an assumption, counted: a cutscene nested in another would leave it frozen)
        self._lazy += 1
        try:
            slots = self._live_slots(p)
        finally:
            self._lazy -= 1
        for name in slots:
            k = f"frozen:{name}"
            if name in p.fz or name in p.freeze or k in p.writes:
                continue
            p.writes[k] = 0
            self.counts["unfreezes of a slot frozen before the path (count taken as 1)"] += 1

    def _count(self, p, name):
        """Machine._count, read lazily; a freeze this path recorded (``_freeze``) has no count."""
        n = p.freeze.get(name) if hasattr(p, "freeze") else None
        if isinstance(n, tuple) and n and n[0] == "sym":
            return None
        return self._lazily("_count", p, name)

    def _finish(self, p, how, at, resume=None):
        f = p.stack[-1]
        super()._finish(p, how, at, resume)
        if not isinstance(p, SymPath):
            return
        name = unit_name(f.unit)
        if how in ("yield", "parked"):
            u = self.game.units[f.unit]
            w = resume if how == "yield" else (u.ins[f.pc].offset if f.pc is not None and f.pc < len(u.ins) else None)
            if w is not None:
                cd = tuple(p.cut_data[-f.override:]) if f.override and len(p.cut_data) >= f.override else ()
                p.kept[name] = (f.unit, w, dict(f.locals), f.args_known, f.recursive, f.resistant, f.owned,
                                f.override if len(cd) == f.override else 0, cd)
        else:
            p.kept.pop(name, None)

    def _eng_scene(self, p, call, oracle, room, ego_obj):
        """startScene; a room change to a room outside the scope ends the path there ("left the scope", counted):
        the scope's task has no actions in that room, and its entry code is not the scope's."""
        if isinstance(p, SymPath) and self.scope_rooms is not None:
            res = self.game.resource(room)
            if isinstance(res, Unknown) or res not in self.scope_rooms:
                self._halt(p, "left the scope", f"{call.cite}: room {room}")
                return []
        return super()._eng_scene(p, call, oracle, room, ego_obj)

    def _eng_script(self, p, call, oracle, n, resistant, recursive, locals_):
        """runScript(N, F, R, args) (script.cpp:38-107). The inventory hook, the script VAR_INVENTORY_SCRIPT holds
        (run by the engine on pickupObject, setOwnerOf, setObjectName and a room change with the ego, and by any
        script that starts that number itself), is not run: it draws the inventory from the engine's inventory list,
        which is not a fact (model.py), and run over unknown counts its loops fork on every slot, and the object
        entries it starts on what findInventory returns have no number. Its writes are guessed instead: every
        variable, bit and verb its code writes by a constant number, the variables after an indexed write's base
        (domains.py's 16), and what the object entries it starts write (``Builder.entry_writes``), counted. Without R
        it stops a running copy first, as runScript does; it is taken to end in the frame (no slot left)."""
        if isinstance(p, SymPath) and isinstance(n, int) and n and ("global", n) in self.game.units:
            self._lazy += 1
            try:
                hook = self.read(p, f"var:{M.VAR_INVENTORY_SCRIPT}", oracle)
            finally:
                self._lazy -= 1
            if hook == n:
                if not recursive:
                    self._stop_script(p, n, call.cite, call.head)
                self.counts[f"inventory hook runs summarized (writes guessed): {call.head}"] += 1
                for k in self._hook_writes(n):
                    self.write(p, k, Unknown(M.R_INVENTORY, f"written by the inventory hook {n} (not run)"))
                self._note(p, call.cite, "inventory hook", f"script {n} not run: its writes are guessed")
                return
        return super()._eng_script(p, call, oracle, n, resistant, recursive, locals_)

    def _hook_writes(self, n: int) -> list:
        cache = self.__dict__.setdefault("_hook_cache", {})
        if n not in cache:
            u = self.game.units[("global", n)]
            w = scope_writes(self.game, {("global", n): u})
            engine = scope_writes(self.game, {}).exact
            keys = {k for k in w.exact - engine if k.startswith(("var:", "bit:", "verb:"))}
            keys |= {f"var:{b + i}" for b, _ in w.var_windows for i in range(16)}
            for x in u.ins:  # the object entries it starts: startObject(O, V) with V constant
                if isinstance(x.st, Start) and x.st.name == "startObject" and isinstance(x.st.script, Const):
                    keys |= set(entry_writes(self.game, x.st.script.value))
            cache[n] = sorted(keys)
        return cache[n]

    # --- engine steps the machine lacks ---------------------------------------------------------------------------

    def _eng_resume_at(self, p, call, oracle, ev: Event) -> list:
        """runAllScripts (script.cpp:968-987) gives slot ``ev`` its turn from offset ``ev.w``, with its locals: as
        ``Machine._eng_resume``, with the locals given. After a delay the slot runs only once the delay is over
        (script_v5.cpp:972-987, script.cpp:1513-1530): that side only, a replay check (the other side has no turn,
        no event). With ``ev.abort`` it is abortCutscene (script.cpp:1689-1709): VAR_OVERRIDE = 1, the override is
        used up, the freeze count is 0, the slot's cutsceneOverride is one lower when above 0 (1703-1704), and the
        slot runs on from the override jump in this frame. The key code then goes on to checkExecVerbs and the input
        script (input.cpp:1427-1428); that is not run here (``ESC_KEY``): it is listed on the path as a write left
        out."""
        u = self.game.units[ev.unit]
        idx = u.at.get(ev.w)
        key = M.lifecycle_key(ev.unit, self.game)
        if ev.abort:
            self.write(p, f"var:{VAR_OVERRIDE}", 1)
            self.clear(p, f"esc:{ev.name}:", None, "abortCutscene")
            self._set_freeze(p, ev.name, (0, True))
            p.unknown_writes.append(ESC_KEY)
            self.counts["Esc keys whose input-script run is left out (ESC_KEY)"] += 1
        prev = u.ins[idx - 1] if idx > 0 else None
        if not ev.abort and prev is not None and prev.next == idx and prev.st is not None \
                and ir.head(prev.st) in ("delay", "delayVariable"):
            p.assumptions.append((prev.cite(), f"{ev.name}'s delay is over", True,
                                  str(Unknown(M.R_TIMER, f"{ev.name}'s delay count"))))
        self.clear(p, f"at:{ev.name}:", None, "resume")
        if u.ins[idx].yields == "wait":
            p.resume_wait = (ev.unit, ev.w)
        self._push_frame(p, call, ev.unit, idx, dict(ev.locals), ev.owned, ev.recursive, ev.resistant, key,
                         args_known=ev.args_known, nested=False)
        if ev.override and len(ev.cut_data) == ev.override and p.end is None:
            # the cutscenes this slot began in an earlier turn, and their data, which its endCutscene hands to the
            # end hook (taken to be the top levels of the stack: counted when another level may sit above them)
            p.stack[-1].override = ev.override - 1 if ev.abort else ev.override  # abortCutscene, script.cpp:1703-1704
            p.cut_data = list(ev.cut_data)
            self.counts["events resuming a slot with its own open cutscenes (cutscene data carried)"] += 1
        return []

    def _wait(self, p, x, head):
        """o5_wait (script_v5.cpp:3272-3310). The wait an event resumes at falls through: that is the event (its
        condition, the actor stopped or the message gone, is a replay check). Any other wait the run meets blocks:
        the slot yields there and what follows is a later event (D5), so a run never assumes an actor already where
        it walks (counted)."""
        if not isinstance(p, SymPath):
            return super()._wait(p, x, head)
        f = p.frame
        if p.resume_wait is not None and p.resume_wait == (f.unit, x.offset):
            p.resume_wait = None
            reason, what = M.WAITS[head]
            p.assumptions.append((x.cite(), f"{x.text.strip()} blocks", False, str(Unknown(reason, what))))
            return []
        self.counts["WaitFor ops that yield (their later part is an event)"] += 1
        self._done(p, "yield", x.cite(), resume=x.offset)
        return []

    # --- runs ---------------------------------------------------------------------------------------------------------

    def _go(self, pins: dict, pre: dict, first: M.Path, start: str, unit) -> M.Run:
        self.oracle = SymOracle(self.table, pins, self.scope.universe)
        self._pre = ()
        self._seed = pre
        p = _to_sym(first, pre) if not isinstance(first, SymPath) else first
        return self._drive(self.oracle, p, start, unit)

    def _drive(self, oracle, first, start, unit):
        if not isinstance(first, SymPath):
            first = _to_sym(first, self._seed)
        return super()._drive(oracle, first, start, unit)

    def run_sentence_sym(self, pins: dict, pre: dict, verb: int, a: int, b) -> M.Run:
        self.oracle = SymOracle(self.table, pins, self.scope.universe)
        self._seed = pre
        self._pre = ()
        return self.run_sentence(self.oracle, verb, a, b)

    def run_click_verb(self, pins: dict, pre: dict, verb: int) -> M.Run:
        """A dialog line: checkExecVerbs' click on verb ``verb`` runs the input script (``Machine._eng_input``,
        verbs.cpp:683-686, script.cpp:1478-1511); a sentence it queues is not run here (a later frame's
        checkAndRunSentenceScript: the sentence queue is not a fact without settle, counted as a note)."""
        call = Call("click", "scumm.cpp:3175-3255", (("input", VERB_CLICK_AREA, verb, LEFT_CLICK),), caller=None,
                    active=False)
        return self._go(pins, pre, M.Path([call]), f"click on verb {verb}", None)

    def run_event(self, pins: dict, pre: dict, ev: Event) -> M.Run:
        call = Call("runAllScripts", "script.cpp:968-987", (("resume_at", ev),), caller=None, active=False)
        return self._go(pins, pre, M.Path([call]), ev.label(), ev.unit)

    # --- lazy wrappers ------------------------------------------------------------------------------------------------

    def _lazily(self, name, *args):
        self._lazy += 1
        try:
            return getattr(super(), name)(*args)
        finally:
            self._lazy -= 1


def _make_lazy(name):
    def method(self, *args):
        return self._lazily(name, *args)
    method.__name__ = name
    method.__doc__ = f"Machine.{name}, with lazy reads (module docstring)."
    return method


for _name in LAZY_METHODS:
    if _name not in Reader.__dict__:
        setattr(Reader, _name, _make_lazy(_name))


def _make_step(name: str, key: str):
    def method(self, p, x, st, oracle):
        if not isinstance(p, SymPath):
            return getattr(Machine, name)(self, p, x, st, oracle)
        self._step_key = key
        if name == "_op_endCutscene":  # its test of an armed override reads every esc: key: lazily (module docstring)
            self._lazy += 1
        try:
            out = getattr(Machine, name)(self, p, x, st, oracle)
        finally:
            self._step_key = None
            if name == "_op_endCutscene":
                self._lazy -= 1
        v = p.writes.get(key)
        if name == "_op_endCutscene" and is_step(v):
            # endCutscene with no cutscene open is the engine's error() (script.cpp:1668-1681): the depth before it,
            # the start's plus the steps before, is at least 1, a precondition
            d = step_of(v) + 1
            dom = self.table.domain(key)
            have = p.pre.get(key, frozenset(dom.values))
            ok = frozenset(u for u in have if u == D.ABOVE or (isinstance(u, int) and u + d >= 1))
            if not ok:
                self._error(p, "cutscene stack underflow", x.cite(), "endCutscene's error() (script.cpp:1668-1681)",
                            fatal=True)
            elif ok != frozenset(dom.values):
                p.pre[key] = ok
        return out
    method.__name__ = name
    method.__doc__ = f"Machine.{name}, with the counter it steps kept as a step until something reads it (R_STEP)."
    return method


for _name, _key in STEP_OPS.items():
    setattr(Reader, _name, _make_step(_name, _key))

# Machine methods the reader calls by name (besides the ones it overrides and wraps)
CALLED = ("_set_freeze", "_push_frame", "_halt", "_note", "_error", "_done", "_live_slots", "_stop_script",
          "_kill_room", "read", "val", "write", "run_sentence", "_count", "_freeze", "_unfreeze", "getter", "test")
PATH_FIELDS = ("freeze", "cut_data", "cleared", "unknown_writes", "assumptions", "notes", "writes", "stack", "end")
FRAME_FIELDS = ("override", "locals", "args_known", "recursive", "resistant", "owned", "pc", "unit", "sid")


def check_machine() -> list:
    """reader.py wraps (LAZY_METHODS, STEP_OPS), overrides and calls model.Machine methods by name. A rename or a
    changed parameter list in model.py would leave a wrapper that is never called (the fork on read would then spread
    into the engine's walks over every slot) with no error. Returns what does not match: a name Machine lacks, or an
    override whose parameters differ from Machine's; checked at import (``ImportError``)."""
    bad = [f"Machine has no {n}" for n in LAZY_METHODS + tuple(STEP_OPS) + CALLED if not hasattr(Machine, n)]
    for name, fn in vars(Reader).items():
        if name.startswith("__") or name in LAZY_METHODS or not callable(fn) or not hasattr(Machine, name):
            continue
        mine = list(inspect.signature(fn).parameters)
        theirs = list(inspect.signature(getattr(Machine, name)).parameters)
        if mine != theirs:
            bad.append(f"Reader.{name}{tuple(mine)} overrides Machine.{name}{tuple(theirs)}")
    path = {f.name for f in dataclasses.fields(M.Path)}
    bad += [f"model.Path has a field {n} that SymPath adds" for n in ("pre", "kept", "fz", "nf", "resume_wait")
            if n in path]
    bad += [f"model.Path has no field {n}" for n in PATH_FIELDS if n not in path]
    frame = {f.name for f in dataclasses.fields(M.Frame)}
    bad += [f"model.Frame has no field {n}" for n in FRAME_FIELDS if n not in frame]
    return bad


if _bad := check_machine():
    raise ImportError("reader.py no longer matches model.Machine: " + "; ".join(_bad))


# --- from paths to records -------------------------------------------------------------------------------------------

DROP_ENDS = M.INFEASIBLE + ("limit", "unmodelled", "deferral unknown", "nothing ran", "left the scope", "no value")  # path ends that are no action
OF_OWNER_ROOM = M.OF_OWNER_ROOM
SAME = object()  # an effect that leaves the fact as the precondition has it


def _why(s: str) -> str:
    """The reason of an assumption's ``why`` (model.Unknown's text, ``?(reason: what)``)."""
    s = s[2:] if s.startswith("?(") else s
    return s.split(":", 1)[0].rstrip(")")


class Builder:
    """Runs the scope's actions and events on a ``Reader`` and keeps one ``Record`` per path that is an action
    (module docstring): ``read_all``. The counts it keeps are the step's report."""

    def __init__(self, rd: Reader, sig_limit: int = SIG_LIMIT):
        self.rd, self.game, self.table, self.scope = rd, rd.game, rd.table, rd.scope
        self.sig_limit = sig_limit
        self.records: list = []
        self.counts = collections.Counter()
        self.ends = collections.Counter()  # (kind, path end) for every path
        self.drops = collections.Counter()  # (what was run, path end) for every path that is no action
        self.nonfact_checks = collections.Counter()  # reason -> conditions on non-facts (replay checks)
        self.guesses = collections.Counter()  # reason -> guessed effects
        self.unknown_writes = collections.Counter()  # statement -> writes whose target does not fold (``record``)
        self.unknown_sites = collections.Counter()  # (statement, cite) -> the same, by site
        self.slow = collections.Counter()  # label -> seconds, for runs (with their splits) past 5 s
        self.out_of_domain = collections.defaultdict(set)  # key -> written values its domain lacks
        self.seconds = collections.Counter()
        self.sig_ids = {}  # number -> the texts of the lists of locals with that number (``_sig``)
        self.sig_seen = collections.defaultdict(list)  # (name, w) -> lists of locals scheduled (SIG_LIMIT)
        self.folded = set()  # lists of locals folded into 0
        self.events_done = set()
        self.todo = []
        self.progress = None  # a callable (what, jobs, records, seconds) told after each batch of runs, or None

    # --- engine conditions (actions.usable) and the inputs ----------------------------------------------------------

    def ego_values(self) -> list:
        v = self.rd.read(SymPath([]), f"var:{M.VAR_EGO}", self.rd.oracle)
        dom = self.table.domain(f"var:{M.VAR_EGO}")
        return [v] if isinstance(v, int) else list(dom.values if dom else [])

    def _pin(self, pins: dict, pre: dict, key: str, values) -> bool:
        """Add an engine condition: a constant is checked against the start (False: never usable), a fact goes into
        the precondition; a single value is also a pin, so the machine's own direct reads see it."""
        values = frozenset(values)
        if self.table.kind(key) == "const":
            return self.table.start.get(key, 0) in values
        dom = self.table.domain(key)
        if dom is not None:
            values = frozenset(dom.label(v) if isinstance(v, int) else v for v in values) - {None}
            if not values:
                return False
            if values != frozenset(dom.values):
                pre[key] = values & frozenset(pre.get(key, values))
        if len(values) == 1:
            v = next(iter(values))
            pins[key] = self.table.value(key, v) if dom is not None else v
        return True

    def _sentence_base(self, n: int) -> list:
        """The (variant, ok, pins, precondition) every sentence shares, per variant: the sentence script ``n``
        installed (VAR_SENTENCE_SCRIPT) and input on (VAR_USERPUT > 0, verbs.cpp:590); checkAndRunSentenceScript holds
        the sentence while a slot of its number is live with freeze count 0 (script.cpp:1186-1191), so either no such
        slot is live ("") or the global one is live and frozen ("~frozen": runScript without R then stops it first,
        script.cpp:65-66)."""
        out = []
        key = self.game.script_key(n)
        for variant, live, frozen in (("", 0, 0), ("~frozen", 1, 1)):
            pins, pre = {}, {}
            ok = self._pin(pins, pre, f"var:{M.VAR_SENTENCE_SCRIPT}", {n})
            for k, v in ((key, live), (f"frozen:{slot_name(key)}", frozen), (f"objscript:{n}", 0),
                         (f"frozen:obj:{n}", 0)):
                ok = ok and self._pin(pins, pre, k, {v})
            dom = self.table.domain(f"var:{M.VAR_USERPUT}")
            ok = ok and self._pin(pins, pre, f"var:{M.VAR_USERPUT}",
                                  {v for v in dom.values if v == D.ABOVE or (isinstance(v, int) and v > 0)})
            out.append((variant, ok, pins, pre))
        return out

    def placements(self, a: int) -> list:
        """Where object A can be clicked, (place, conditions, replay checks): as a room object (findObject,
        object.cpp:557-597: owner OF_OWNER_ROOM, class Untouchable clear, the room in use loads A's room resource,
        through a pseudo room too, room.cpp:166-169), and in the inventory when its owner can be VAR_EGO's value. An
        actor (a number below _numActors, object.cpp:1620-1622) is found by getActorFromPos (actor.cpp:2115-2147): not
        Untouchable, and drawn in this room, which is not a fact (a replay check). findObject also hides an object
        whose parent is not in its parent state (object.cpp:567-590); the parent state is not in the index, so that
        is a replay check too (actions.reachable's PARENT), counted."""
        if a < M.NUM_ACTORS:
            return [("actor", {f"class:{a}:{M.CLASS_UNTOUCHABLE}": {0}},
                     [("actor.cpp:2115-2147", f"actor {a} drawn in the room under the click", True,
                       str(Unknown(M.R_ACTOR, f"getActorFromPos({a})")))])]
        out = []
        home = self.game.object_room.get(a)
        rdom = self.table.domain("room")
        rooms = {r for r in (rdom.values if rdom else ()) if isinstance(r, int) and self.game.resource(r) == home} \
            or {home}
        parent = self.game.object_parent.get(a, 0)
        checks = [("object.cpp:567-590", f"object {a}'s parent {parent} in its parent state", True,
                   str(Unknown(M.R_HELD, "the parent state is not in the index")))] if parent else []
        out.append(("room", {"room": rooms, f"owner:{a}": {OF_OWNER_ROOM}, f"class:{a}:{M.CLASS_UNTOUCHABLE}": {0}},
                    checks))
        dom = self.table.domain(f"owner:{a}")
        ego = [v for v in self.ego_values() if isinstance(v, int)]
        if dom is not None and any(v in dom.values for v in ego):
            out.append(("inventory", {f"owner:{a}": set(ego)}, []))
        return out

    def _b_verbs(self, n: int, verb: int) -> set:
        """The verbs V of the startObject(B, V) calls the sentence script ``n`` can reach with ``verb`` in its verb
        local (SENTENCE_LOCALS; the jumps that local decides go one way), where B is its object-B local: the sentence
        script runs B's own entry (as with Give, plan step 1). Cached."""
        cache = self.__dict__.setdefault("_bverb_cache", {})
        if (n, verb) not in cache:
            u = self.game.units.get(("global", n)) if n < self.game.first_local else None
            out = set()
            if u is not None:
                vl, bl = Local(SENTENCE_LOCALS["verb"]), Local(SENTENCE_LOCALS["b"])
                bound = not any(isinstance(x.st, Assign) and x.st.lhs == vl for x in u.ins)
                env = {vl.n: frozenset({verb})} if bound else {}
                for x in _live_bound(u, env):
                    st = x.st
                    if isinstance(st, Start) and st.name == "startObject" and st.obj == bl \
                            and isinstance(st.script, Const):
                        out.add(st.script.value)
            cache[(n, verb)] = out
        return cache[(n, verb)]

    def b_values(self, a: int, verb: int, ns=()) -> tuple:
        """B for (verb, A) (plan step 1: "B ranges over the constants the reached handler compares it with, plus one
        other"): the constants A's entry for the verb compares its first argument with that are objects of the scope
        or actors (below _numActors); the objects of the scope with an entry for a verb V the sentence scripts ``ns``
        start on B itself (``_b_verbs``: that entry is the reached handler); plus "other" (``Scope.other_b``). ()
        when there is none of these (a one-object sentence only)."""
        consts = set()
        u = self.game.units.get(("obj", a))
        idx = self.game.entry(a, verb)
        if u is not None and idx is not None:
            for x in D.live(u, {n: frozenset({v}) for n, v in D.SETTINGS.items()}, [idx]):
                if isinstance(x.st, Jump) and isinstance(x.st.cond, Cmp):
                    c = x.st.cond
                    for l, r in ((c.a, c.b), (c.b, c.a)):
                        if l == Local(0) and isinstance(r, Const):
                            consts.add(r.value)
        handlers = set()
        for n in ns:
            for v in self._b_verbs(n, verb):
                handlers |= {o for o in self.scope.objects if o != a and self.game.entry(o, v) is not None
                             and v in self.game.units[("obj", o)].entries}
        if not consts and not handlers:
            return ()
        inside = sorted(b for b in consts if b in self.scope.objects or 0 < b < M.NUM_ACTORS)
        self.counts["B constants outside the scope (not built)"] += len(consts) - len(inside)
        self.counts["B objects whose own entry the sentence script starts"] += len(handlers - set(inside))
        self.counts["B actors (reachability a replay check)"] += sum(1 for b in inside if b < M.NUM_ACTORS)
        other = self.scope.other_b(a, consts | handlers, self.game)
        if other is not None:
            self.counts["B = other (a stand-in object)"] += 1
        return tuple(sorted(set(inside) | handlers)) + ((other,) if other is not None else ())

    def sentence_scripts(self) -> list:
        dom33 = self.table.domain(f"var:{M.VAR_SENTENCE_SCRIPT}")
        return [v for v in (dom33.values if dom33 else [self.table.start.get(f"var:{M.VAR_SENTENCE_SCRIPT}", 0)])
                if isinstance(v, int) and v]

    def inputs(self):
        """Every player input of the scope: (kind, label, pins, pre, runner, replay checks)."""
        ns = self.sentence_scripts()
        for n in ns:
            for variant, ok, base_pins, base_pre in self._sentence_base(n):
                if not ok:
                    self.counts[f"sentence scripts whose base conditions never hold{variant}"] += 1
                    continue
                for a in self.scope.objects:
                    for place, conds, checks in self.placements(a):
                        verbs = [(v, False) for v in self.scope.verbs]
                        if place == "room" and self.scope.walk_verb is not None:
                            verbs.append((self.scope.walk_verb, True))
                        for verb, walk in verbs:
                            pins, pre = dict(base_pins), {k: set(v) for k, v in base_pre.items()}
                            ok = all(self._pin(pins, pre, k, v) for k, v in conds.items())
                            if not walk:
                                ok = ok and self._pin(pins, pre, f"verb:{verb}", {1})
                            if not ok:
                                self.counts["inputs whose engine conditions never hold (constants)"] += 1
                                continue
                            pre = {k: frozenset(v) for k, v in pre.items()}
                            kind = "walk_to" if walk else "sentence"
                            for b in (None,) + (() if walk else self.b_values(a, verb, ns)):
                                label = f"walk_to({a})" if walk else f"sentence({verb},{a}" + (f",{b})" if b else ")")
                                if place == "inventory":
                                    label += "[inv]"
                                label += variant
                                if variant:
                                    self.counts[f"sentence inputs{variant}"] += 1
                                if b is not None:
                                    pb, prb = dict(pins), dict(pre)
                                    pl = [x for x in self.placements(b) if x[0] != "inventory"] or self.placements(b)
                                    okb = all(self._pin(pb, prb, k, v) for k, v in pl[0][1].items())
                                    if not okb:
                                        continue
                                    yield kind, label, pb, {k: frozenset(v) for k, v in prb.items()}, \
                                        ("sentence", verb, a, b), checks + pl[0][2]
                                else:
                                    yield kind, label, pins, pre, ("sentence", verb, a, None), checks
        dom32 = self.table.domain(f"var:{M.VAR_VERB_SCRIPT}")
        vs = [v for v in (dom32.values if dom32 else [self.table.start.get(f"var:{M.VAR_VERB_SCRIPT}", 0)])
              if isinstance(v, int) and v]
        for v in self.scope.dialog_verbs:
            for n in vs:
                for room in self.scope.rooms:
                    pins, pre = {}, {}
                    dom = self.table.domain(f"var:{M.VAR_USERPUT}")
                    ok = self._pin(pins, pre, f"var:{M.VAR_VERB_SCRIPT}", {n}) and \
                        self._pin(pins, pre, f"verb:{v}", {1}) and self._pin(pins, pre, "room", {room}) and \
                        self._pin(pins, pre, f"var:{M.VAR_USERPUT}",
                                  {u for u in dom.values if u == D.ABOVE or (isinstance(u, int) and u > 0)})
                    if ok:
                        yield "dialog_line", f"dialog_line({v})[{n}@{room}]", pins, \
                            {k: frozenset(s) for k, s in pre.items()}, ("verb", v), []

    # --- events --------------------------------------------------------------------------------------------------------

    def start_events(self) -> list:
        """The slots live at the start, with unknown locals (``args:`` 0)."""
        out = []
        st = self.table.start
        res = self.game.resource(st.get("room", 0))
        for k in sorted(st):
            if not k.startswith("at:") or st[k] != 1:
                continue
            name, w = k[3:].rsplit(":", 1)
            w = int(w)
            if name.startswith("script:"):
                unit = ("global", int(name[7:]))
            elif name.startswith("local:"):
                unit = ("local", res, int(name[6:]))
            elif name.startswith("obj:"):
                unit = ("obj", int(name[4:]))
            else:
                unit = (name, res)
            if unit not in self.scope.units:
                self.counts["slots live at the start outside the scope (no events)"] += 1
                continue
            owned = False
            if unit[0] == "obj":
                owned = st.get(f"owner:{unit[1]}", 0) != OF_OWNER_ROOM
            res_flag = self.game.resistant(M.lifecycle_key(unit, self.game) or name, res)
            out.append(Event(name, unit, w, 0, {}, False, None, res_flag, owned))
        return out

    def _sig(self, name: str, w: int, locals_: dict, args_known: bool) -> int:
        """The number of a list of locals: 48 bits of its text's SHA-1, the same in every process (0: not known).
        The text is kept, so that two lists with one number are counted (``read_all``)."""
        if not args_known:
            return 0
        text = repr((name, tuple(sorted((str(i), str(v)) for i, v in locals_.items()))))
        sig = int(hashlib.sha1(text.encode()).hexdigest()[:12], 16) or 1
        self.sig_ids.setdefault(sig, set()).add(text)
        return sig

    def event_pins(self, ev: Event):
        pins, pre = {}, {}
        key = M.lifecycle_key(ev.unit, self.game)
        ok = True
        if key:
            ok = self._pin(pins, pre, key, {1})
        if ev.abort:
            ok = ok and self._pin(pins, pre, f"esc:{ev.name}:{ev.w}", {1})
            dom = self.table.domain(f"var:{VAR_CUTSCENEEXIT_KEY}")
            if self.table.kind(f"var:{VAR_CUTSCENEEXIT_KEY}") == "const":
                ok = ok and self.table.start.get(f"var:{VAR_CUTSCENEEXIT_KEY}", 0) != 0
            elif dom is not None:
                ok = ok and self._pin(pins, pre, f"var:{VAR_CUTSCENEEXIT_KEY}", {v for v in dom.values if v != 0})
        else:
            ok = ok and self._pin(pins, pre, f"at:{ev.name}:{ev.w}", {1})
            ok = ok and self._pin(pins, pre, f"frozen:{ev.name}", {0})
            for k in self.rd.oracle.keys(f"at:{ev.name}:"):  # one instance: its other wait points are 0 (assumed)
                if k != f"at:{ev.name}:{ev.w}":
                    pins[k] = 0
        ok = ok and self._pin(pins, pre, f"args:{ev.name}", {ev.sig})
        if ev.unit[0] in ("local", "entry", "exit"):
            rooms = [r for r in self.scope.rooms if self.game.resource(r) == ev.unit[1]]
            ok = ok and self._pin(pins, pre, "room", set(rooms))
        return ok, pins, {k: frozenset(v) for k, v in pre.items()}

    # --- one run ------------------------------------------------------------------------------------------------------

    def run(self, kind: str, label: str, pins: dict, pre: dict, what, checks=(), budget=None) -> list:
        """The records of one input or event. A run that hits PATH_LIMIT is run again once per value of the first
        fact it forked on (the value a precondition), each part with its own PATH_LIMIT, down to SPLIT_DEPTH levels
        and within SPLIT_BUDGET runs per input (``budget``), unless it forked more on non-facts than on facts (a split
        on a fact would not cut those); what is still cut is counted. ``checks``: the input's
        own replay checks (``placements``), added to every record."""
        root = budget is None
        if root:
            budget = [SPLIT_BUDGET, 0, frozenset(pre)]  # runs left, depth, the input's own precondition facts
            self.counts[f"runs: {kind}"] += 1
            t = time.monotonic()
            out = self.run(kind, label, pins, pre, what, checks, budget)
            if (secs := time.monotonic() - t) > 5:
                self.slow[label] += secs
            return out
        t0 = time.monotonic()
        rd = self.rd
        rd.first_fork = None
        facts0, nonfacts0 = rd.counts["forks on read"], rd.counts["forks on non-fact tests"]
        if what[0] == "sentence":
            run = rd.run_sentence_sym(pins, pre, what[1], what[2], what[3])
        elif what[0] == "verb":
            run = rd.run_click_verb(pins, pre, what[1])
        else:
            run = rd.run_event(pins, pre, what[1])
        self.seconds[kind] += time.monotonic() - t0
        self.counts["value sets past model.MAX_VALUES (made plain unknowns)"] += sum(run.value_caps.values())
        key = rd.first_fork
        facts, nonfacts = rd.counts["forks on read"] - facts0, rd.counts["forks on non-fact tests"] - nonfacts0
        if run.limit and key is not None and facts < nonfacts:
            # forks on non-facts (unknown locals, positions, ...) drive it: a split on a fact would not cut them
            self.counts["runs past the path limit not split (more forks on non-facts than on facts)"] += 1
        elif run.limit and key is not None:
            dom = self.table.domain(key)
            vals = [v for v in dom.values if v in pre.get(key, frozenset(dom.values))]
            if len(vals) > 1 and budget[1] < SPLIT_DEPTH and budget[0] >= len(vals):
                budget[0] -= len(vals)
                budget[1] += 1
                self.counts["runs past the path limit split on their first fork on a read"] += 1
                out = []
                for v in vals:
                    p2, pre2 = dict(pins), dict(pre)
                    pre2[key] = frozenset({v})
                    p2[key] = self.table.value(key, v)
                    out += self.run(kind, label, p2, pre2, what, checks, budget)
                budget[1] -= 1
                return out
            self.counts["runs past the path limit not split (depth or budget used up, or no fork on a read)"] += 1
        self.counts["paths ended by the path limit"] += run.limit
        out = []
        for p in run.paths:
            self.ends[(kind, p.end)] += 1
            if p.end in DROP_ENDS or not isinstance(p, SymPath):
                self.drops[(label.split("#")[0].split("@")[0] + (")" if "@" in label else ""), p.end)] += 1
                continue
            rec = self.record(kind, label, p, pins, what)
            if rec is None:
                self.counts[f"no-op paths (no effect): {kind}"] += 1
                continue
            for c in checks:
                rec.checks.append(c)
                self.nonfact_checks[_why(c[3])] += 1
            own = {f"pos:{what[1].name}", f"escpos:{what[1].name}"} if what[0] == "event" else set()
            rec.engine_pre = frozenset(k for k in rec.pre if k in budget[2] or k in own)
            out.append(rec)
        return out

    def record(self, kind, label, p: SymPath, pins: dict, what):
        pre = {}
        for k, s in p.pre.items():
            dom = self.table.domain(k)
            if dom is not None and frozenset(s) != frozenset(dom.values):
                pre[k] = frozenset(s)
        effects = {}
        for k, v in sorted(p.writes.items()):
            e = self.effect(k, v, p)
            if e is not None and e is not SAME:
                effects[k] = e
        nexts = []
        for name, (unit, w, locals_, known, rec_flag, res_flag, owned, ov, cd) in sorted(p.kept.items()):
            if p.writes.get(f"at:{name}:{w}") != 1:
                continue
            locs = {i: (v if not isinstance(v, Unknown) or v.values is None or v.reason != R_FACT
                        else Unknown(M.R_LOCAL, f"Local[{i}] from a fact")) for i, v in locals_.items()}
            cd = tuple(c if not isinstance(c, Unknown) else str(c) for c in cd)
            sig = self._sig(name, w, dict(locs, override=ov, cut_data=cd), known)
            sig = 0 if sig in self.folded else sig
            ev = Event(name, unit, w, sig, locs, known and sig != 0, rec_flag, res_flag, owned,
                       override=ov if sig else 0, cut_data=tuple(c for c in cd if isinstance(c, int)) if sig else ())
            nexts.append(ev)
            effects[f"args:{name}"] = ("set", sig)
            self.table.args_vals[f"args:{name}"].add(sig)  # the domain grows as lists appear (the parent's: _run_jobs)
            for k in [k for k, v in p.writes.items() if k.startswith(f"esc:{name}:") and v == 1]:
                nexts.append(dataclasses.replace(ev, w=int(k.rsplit(":", 1)[1]), abort=True))
        pre, effects = self.positions(p, pre, effects)
        unknown = []
        for w in p.unknown_writes:  # writes whose target the machine could not work out (model.py's unknown_writes)
            self.unknown_writes[w[0]] += 1
            self.unknown_sites[(w[0], w[1])] += 1
            # left out of the task and listed on the record for replay; ``unknown_targets`` gives the facts it may
            # have hit where the code bounds them (replay checks those). Not guessed: a guess on every candidate is a
            # free write the planner takes (module docstring)
            unknown.append(w)
            keys = self.unknown_targets(w)
            which = "no candidate list" if keys is None else "with a candidate list"
            self.counts[f"unknown-target writes left out and listed on the record ({which}): {w[0]}"] += 1
        effects = {k: e for k, e in effects.items()
                   if not (e[0] == "set" and k in pre and pre[k] == frozenset({e[1]}))}
        if not effects:
            return None
        checks, sides = [], collections.defaultdict(set)
        for a in p.assumptions:
            sides[(a[0], a[1])].add(str(a[2]))
        either = []
        for a in p.assumptions:
            if len(sides[(a[0], a[1])]) == 1:
                if a not in checks:
                    checks.append(a)
                    self.nonfact_checks[_why(a[3])] += 1
            elif (a[0], a[1]) not in either:
                # both sides on one record: branches the machine merged, or one test met twice in a frame with an op
                # between that may change it (``Reader.test``): no single side to check, so it is listed apart, not
                # checked (counted)
                either.append((a[0], a[1]))
                self.counts["conditions with both sides on one record (listed apart, not checked)"] += 1
        return Record(kind, label, pre, effects, checks, p.end, list(p.notes[-5:]), nexts, what, either,
                      unknown=unknown)

    def unknown_targets(self, w: tuple):
        """The facts a write whose target does not fold (``Path.unknown_writes``: (statement, cite, target)) may
        hit, or None when the reader has no list of candidates: what replay checks for a record that lists it
        (``Record.unknown``):

        * VerbOps on a verb number that does not fold: ``verb:V`` of every verb some script creates, when one of
          its sub-ops changes whether a verb shows (New, On, Off, Dim, Delete); else it changes no fact ([]);
          saveVerbs and restoreVerbs: every such ``verb:V``;
        * drawObject, setState, setOwnerOf, pickupObject, setClass on an object that does not fold: that fact of
          every object of the scope (drawObject reaches only the room's objects, getObjectIndex, script_v5.cpp:
          1101-1103; for the others an object outside the scope's rooms is taken as not hit, an assumption);
        * startObject(O, V) with V constant and O not: ``objscript:O`` of every object with an entry for V, and what
          those entries write (``entry_writes``);
        * a start of a script number that does not fold, a stop, a hook, a sentence or the Esc key's input script:
          None."""
        head, cite = w[0], w[1]
        cache = self.__dict__.setdefault("_unknown_cache", {})
        if (head, cite) in cache:
            return cache[(head, cite)]
        x = self._ins_at(cite)
        st = x.st if x is not None else None
        out = None
        if head in ("VerbOps", "saveVerbs", "restoreVerbs"):
            changes = head != "VerbOps" or (isinstance(st, Op) and any(
                s.name in ("New", "On", "Off", "Dim", "Delete") for s in st.subops))
            out = [f"verb:{v}" for v in sorted(made_verbs(self.game))] if changes else []
        elif head in ("drawObject", "setState"):
            out = [f"state:{o}" for o in self.scope.objects]
        elif head == "setOwnerOf":
            out = [f"owner:{o}" for o in self.scope.objects]
        elif head == "pickupObject":
            out = [f"{k}:{o}" for o in self.scope.objects for k in ("owner", "state")] + \
                [f"class:{o}:{M.CLASS_UNTOUCHABLE}" for o in self.scope.objects]
        elif head == "setClass" and isinstance(st, Op) and len(st.args) > 1 and isinstance(st.args[1], List):
            cls = [c.value & 0x7F for c in st.args[1].items if isinstance(c, Const)]
            if len(cls) == len(st.args[1].items) and 0 not in cls:
                out = [f"class:{o}:{c}" for o in self.scope.objects for c in cls]
        elif head == "startObject" and isinstance(st, Start) and isinstance(st.script, Const):
            v = st.script.value
            objs = sorted(k[1] for k, u in self.game.units.items() if k[0] == "obj" and v in u.entries)
            out = [f"objscript:{o}" for o in objs] + list(entry_writes(self.game, v))
        cache[(head, cite)] = out
        return out

    def _ins_at(self, cite: str):
        """The instruction whose cite is ``cite`` (x.cite()), or None."""
        idx = self.__dict__.get("_cites")
        if idx is None:
            idx = self.__dict__["_cites"] = {x.cite(): x for u in self.game.units.values() for x in u.ins}
        return idx.get(cite)

    def positions(self, p: SymPath, pre: dict, effects: dict) -> tuple:
        """facts.py's ``at:<unit>:W`` and ``esc:<unit>:W`` keys as one fact per unit, ``pos:<unit>`` and
        ``escpos:<unit>``: the offset, or NOWHERE (module docstring). A slot waits at one place, so a write of 1 is
        the new value, and writes of 0 alone (a stop, a kill, a clear before a start) are NOWHERE."""
        for kind, pos in (("at", "pos"), ("esc", "escpos")):
            ones, zeros = collections.defaultdict(list), set()
            for k in [k for k in effects if k.startswith(kind + ":")]:
                e = effects.pop(k)
                name, w = k[len(kind) + 1:].rsplit(":", 1)
                if w == "*":
                    zeros.add(name)
                elif e[0] == "set" and e[1] == 1:
                    ones[name].append(int(w))
                elif e[0] == "set" and e[1] == 0:
                    zeros.add(name)
                else:
                    self.counts[f"{kind}: writes not 0 or 1 (left out)"] += 1
            for name in sorted(set(ones) | zeros):
                if len(ones.get(name, ())) > 1:
                    self.counts[f"{kind}: more than one place written for one unit (a guess)"] += 1
                    effects[f"{pos}:{name}"] = ("guess", frozenset(ones[name]))
                else:
                    effects[f"{pos}:{name}"] = ("set", ones[name][0] if ones.get(name) else NOWHERE)
            need = collections.defaultdict(dict)
            for k in [k for k in pre if k.startswith(kind + ":")]:
                s = pre.pop(k)
                name, w = k[len(kind) + 1:].rsplit(":", 1)
                need[name][int(w)] = s
            for name, ws in need.items():
                ok = set(self.table.domain(f"{pos}:{name}").values)
                for w, s in ws.items():
                    if s == frozenset({1}):
                        ok &= {w}
                    elif s == frozenset({0}):
                        ok -= {w}
                pre[f"{pos}:{name}"] = frozenset(ok)
        return pre, effects

    def effect(self, key: str, v, p: SymPath):
        """The effect of the path's last write ``v`` to ``key``: ("set", value), ("guess", values), ("add", d), SAME
        or None (no domain: not a fact, counted)."""
        dom = self.table.domain(key)
        if dom is None or self.table.kind(key) == "nonfact":
            self.counts["writes to non-facts (left out of the task)"] += 1
            return None
        if isinstance(v, (Cond, Copy)):
            src, s = v.what, None
            if self.table.kind(src) == "const":
                s = self.table.start.get(src, 0)
            elif src in p.pre and len(p.pre[src]) == 1:
                s = next(iter(p.pre[src]))
            if s is not None:  # the source's value is known on this path
                if isinstance(v, Cond):
                    return ("set", next(iter(v.values))) if s == 1 else SAME
                return SAME if src == key else ("set", s)
            if src == key:
                return SAME
            return ("when", src, next(iter(v.values))) if isinstance(v, Cond) else ("copy", src)
        if is_step(v):
            d = step_of(v)
            base = p.pre.get(key)
            if d == 0:
                return SAME
            if base is not None and len(base) == 1:
                return self.effect(key, self.rd._stepped(key, next(iter(base)), d), p)
            return ("add", d)
        if isinstance(v, int):
            lab = dom.label(v)
            if lab is None:
                self.out_of_domain[key].add(v)
                return None
            return ("set", lab)
        if v.reason == R_COLLAPSED and v.values and len(v.values) == 1:
            return ("set", dom.label(next(iter(v.values))))
        if v.values is not None:
            labs = {dom.label(x) for x in v.values}
            if None in labs:
                self.out_of_domain[key] |= {x for x in v.values if dom.label(x) is None}
                labs.discard(None)
            if v.reason == R_FACT and v.what == key and frozenset(labs) == frozenset(p.pre.get(key, dom.values)):
                return SAME  # a lazy read written back
            self.guesses[v.reason] += 1
            return ("guess", frozenset(labs)) if len(labs) > 1 else ("set", next(iter(labs))) if labs else None
        self.guesses[v.reason] += 1
        return ("guess", frozenset(dom.values))

    # --- everything ---------------------------------------------------------------------------------------------------

    def read_all(self, start_events: bool = True, skip=None, workers: int = 1, passes: int = 2) -> list:
        """Every input, then events until no new one appears (a worklist in rounds; each wait point and list of
        locals once), on ``workers`` processes. ``skip``: a predicate on Events left out (a disclosed variant,
        counted). A value written outside its domain adds it to the domain, and the whole reading runs again with
        the wider domains (at most ``passes`` times; what is still outside is counted and left out of the task)."""
        for n in range(passes):
            self._read_once(start_events, skip, workers)
            self.counts[f"pass {n + 1}: records"] = len(self.records)
            if not self.out_of_domain or n == passes - 1:
                break
            added = self.extend_domains()
            self.counts[f"pass {n + 1}: domain values added for the next pass"] = added
            self._reset()
        self.counts["records"] = len(self.records)
        return self.records

    def _reset(self) -> None:
        keep = {k: v for k, v in self.counts.items() if k.startswith("pass ")}
        self.records, self.events_done, self.sig_ids = [], set(), {}
        for c in (self.counts, self.ends, self.drops, self.nonfact_checks, self.guesses, self.seconds, self.rd.counts,
                  self.rd.forks, self.rd.fork_keys, self.unknown_writes, self.unknown_sites, self.slow):
            c.clear()
        self.counts.update(keep)
        self.out_of_domain.clear()
        self.table.args_vals.clear()
        self.sig_seen, self.folded = collections.defaultdict(list), set()

    def extend_domains(self) -> int:
        """Add the written values a domain lacked (module docstring); returns how many."""
        added = 0
        for key, vals in sorted(self.out_of_domain.items()):
            dom = self.table.domain(key)
            ints = {v for v in dom.values if isinstance(v, int)} | set(vals)
            if D.ABOVE in dom.values:
                ints.add(dom.high + 1)
            if D.BELOW in dom.values:
                ints.add(dom.low - 1)
            new = D.Domain.of(ints, dom.low if D.BELOW in dom.values else None,
                              dom.high if D.ABOVE in dom.values else None)
            added += len(new.values) - len(dom.values)
            self.table.extra[key] = new
        return added

    def _read_once(self, start_events, skip, workers) -> None:
        self.sig_seen, self.folded = collections.defaultdict(list), set()
        t0 = time.monotonic()
        jobs = list(self.inputs())
        self.records = self._run_jobs(jobs, workers)
        if self.progress:
            self.progress("inputs", len(jobs), len(self.records), time.monotonic() - t0)
        todo = list(self.start_events()) if start_events else []
        for r in self.records:
            todo += r.next_events
        rounds = 0
        while todo:
            rounds += 1
            jobs = []
            added = {(ev.name, ev.unit, ev.w, ev.abort) for ev in todo if not ev.sig}
            for ev in list(todo):
                # a replan starts from the engine's state, whose slot dump has no locals (the bridge's): every wait
                # point an action leaves a slot at also gets the event with its locals unknown (args 0), counted
                k = (ev.name, ev.unit, ev.w, ev.abort)
                if ev.sig and k not in added and (ev.name, ev.unit, ev.w, 0, ev.abort) not in self.events_done:
                    added.add(k)
                    todo.append(dataclasses.replace(ev, sig=0, locals={}, args_known=False, recursive=None,
                                                    override=0, cut_data=()))
                    self.counts["events with unknown locals (args 0) added for replans"] += 1
            for ev in todo:
                ev = self._limit(ev)
                k = (ev.name, ev.unit, ev.w, ev.sig, ev.abort)
                if k in self.events_done:
                    continue
                self.events_done.add(k)
                if skip is not None and skip(ev):
                    self.counts["events left out by the caller"] += 1
                    continue
                ok, pins, pre = self.event_pins(ev)
                if not ok:
                    self.counts["events whose conditions never hold (constants)"] += 1
                    continue
                jobs.append(("skip_cutscene" if ev.abort else "event", ev.label(), pins, pre, ("event", ev), []))
            t0 = time.monotonic()
            recs = self._run_jobs(jobs, workers)
            self.records += recs
            if self.progress:
                self.progress(f"event round {rounds}", len(jobs), len(recs), time.monotonic() - t0)
            todo = [e for r in recs for e in r.next_events]
        self.counts["event rounds"] = rounds
        for r in self.records:  # a list of locals folded into 0 after the path that wrote it was read
            for k, e in list(r.effects.items()):
                if k.startswith("args:") and e[1] in self.folded:
                    r.effects[k] = ("set", 0)
            for k, s in list(r.pre.items()):
                if k.startswith("args:") and s & self.folded:
                    r.pre[k] = frozenset({0})
        for r in self.records:
            for k, e in r.effects.items():
                if k.startswith("args:"):
                    self.table.args_vals[k].add(e[1])
        self.counts["lists of locals sharing a number (hash collisions)"] = sum(len(t) - 1 for t in self.sig_ids.values())

    def _limit(self, ev: Event) -> Event:
        """SIG_LIMIT: past it, a wait point's further lists of locals are folded into 0 (locals unknown), counted."""
        if not ev.sig:
            return ev
        seen = self.sig_seen[(ev.name, ev.w)]
        if ev.sig in seen:
            return ev
        if ev.sig not in self.folded and len(seen) < self.sig_limit:
            seen.append(ev.sig)
            return ev
        if ev.sig not in self.folded:
            self.folded.add(ev.sig)
            self.counts["lists of locals past SIG_LIMIT (taken as unknown)"] += 1
        return dataclasses.replace(ev, sig=0, locals={}, args_known=False, override=0, cut_data=())

    def _run_jobs(self, jobs: list, workers: int) -> list:
        """Run ``jobs`` here, or on ``workers`` forked processes (each returns its records and its counts, added
        here in the jobs' order, so the result does not depend on the number of workers)."""
        for job in jobs:  # the parent registers the lists of locals the events pin
            what = job[4]
            if what[0] == "event" and what[1].sig:
                self.table.args_vals[f"args:{what[1].name}"].add(what[1].sig)
        if workers <= 1 or len(jobs) < 2 * workers:
            out = []
            for job in jobs:
                out += self.run(*job)
            for r in out:
                for k, e in r.effects.items():
                    if k.startswith("args:"):
                        self.table.args_vals[k].add(e[1])
            return out
        global _WORKER
        _WORKER = self
        size = max(1, len(jobs) // (workers * 8))
        chunks = [jobs[i:i + size] for i in range(0, len(jobs), size)]
        with multiprocessing.get_context("fork").Pool(workers) as pool:
            results = pool.map(_run_chunk, chunks)
        out = []
        for recs, harvest in results:
            out += recs
            for name, c in harvest.items():
                if name == "out_of_domain":
                    for k, v in c.items():
                        self.out_of_domain[k] |= v
                elif name == "sig_ids":
                    for sig, texts in c.items():
                        self.sig_ids.setdefault(sig, set()).update(texts)
                else:
                    self._counter(name).update(c)
        for r in out:
            for k, e in r.effects.items():
                if k.startswith("args:"):
                    self.table.args_vals[k].add(e[1])
        return out

    def _counter(self, name: str):
        return getattr(self.rd, name[3:]) if name.startswith("rd.") else getattr(self, name)


HARVEST = ("counts", "ends", "drops", "nonfact_checks", "guesses", "seconds", "unknown_writes", "unknown_sites",
           "slow", "rd.counts", "rd.forks", "rd.fork_keys")
_WORKER = None  # the Builder a forked worker runs on (``Builder._run_jobs``)


def _run_chunk(jobs: list) -> tuple:
    b = _WORKER
    for name in HARVEST:
        b._counter(name).clear()
    b.out_of_domain.clear()
    b.sig_ids = {}
    out = []
    for job in jobs:
        out += b.run(*job)
    harvest = {name: collections.Counter(b._counter(name)) for name in HARVEST}
    harvest["out_of_domain"] = {k: set(v) for k, v in b.out_of_domain.items()}
    harvest["sig_ids"] = dict(b.sig_ids)
    return out, harvest


def build_task(b: Builder, records: list, goal: dict) -> T.Task:
    """The planner's task (task.py) from the records: one action per record, except that records of one input or
    event (one label) with the same precondition and effects are one action: the planner cannot tell them apart,
    and they differ only in their replay checks (paths through non-fact tests that change no fact), which are
    alternatives at replay (the first record is the action's ``what``; the rest stay in ``records``). Counted in
    the task's ``folded``."""
    facts, actions = {}, []
    for k in goal:
        facts[k] = b.table.domain(k)
    seen, folded = set(), 0
    for i, r in enumerate(records):
        same = (r.label, tuple(sorted((k, tuple(sorted(map(str, v)))) for k, v in r.pre.items())),
                tuple(sorted((k, str(e)) for k, e in r.effects.items())))
        if same in seen:
            folded += 1
            continue
        seen.add(same)
        effects, pre = [], {k: set(v) for k, v in r.pre.items()}
        for k, e in r.effects.items():
            dom = facts.setdefault(k, b.table.domain(k))
            if e[0] == "set":
                effects.append(T.Effect(k, value=e[1]))
            elif e[0] == "guess":
                effects.append(T.Effect(k, values=e[1]))
            elif e[0] == "when":
                facts.setdefault(e[1], b.table.domain(e[1]))
                effects.append(T.Effect(k, value=e[2], when=((e[1], frozenset({1})),)))
            elif e[0] == "copy":
                facts.setdefault(e[1], b.table.domain(e[1]))
                effects += [T.Effect(k, value=u, when=((e[1], frozenset({u})),)) for u in (0, 1)]
            else:
                # a step past the domain's ends is the engine's error() (beginCutscene past kMaxCutsceneNum,
                # script.cpp:1632-1646) or outside the reader's window: the start value must leave room for it
                ok = {v for v in pre.get(k, dom.values) if dom.add(v, e[1])}
                if ok != set(pre.get(k, dom.values)):
                    pre[k] = ok
                    b.counts["steps that need room in their domain (a precondition)"] += 1
                effects.append(T.Effect(k, add=e[1], when=((k, frozenset(ok)),)))
        for k in r.pre:
            facts.setdefault(k, b.table.domain(k))
        actions.append(T.Action(f"{r.label}#{i}", pre=pre, effects=tuple(effects), what=r))
    for k in list(facts):
        if k.startswith("args:"):
            facts[k] = b.table.domain(k)
    task = T.Task(facts, actions, goal)
    task.folded = folded  # records folded into an action with the same label, precondition and effects
    return task


def prune(task: T.Task, rel) -> T.Task:
    """The task with relevance's kept actions (relevance.restrict) and, in them, no effect on a fact no kept
    action's precondition, no effect's condition and no goal reads (``rel.facts``): such a write can change no
    plan (relevance.py's own argument), and the writer's size grows with every effect. Counted."""
    small = relevance_restrict(task, rel)
    needed = rel.facts
    actions = []
    for a in small.actions:
        effects = tuple(e for e in a.effects if e.key in needed)
        actions.append(dataclasses.replace(a, effects=effects))
    facts = {k: d for k, d in small.facts.items() if k in needed or any(k in a.pre for a in actions)}
    return T.Task(facts, actions, small.goal)


def relevance_restrict(task: T.Task, rel) -> T.Task:
    import relevance
    return relevance.restrict(task, rel)


def replan_start(b: Builder, observed: dict, predicted: dict, task: T.Task | None = None) -> tuple:
    """The task's start at a replan (plan step 5), from the engine's state ``observed`` (a facts.py map) and the
    plan's predicted state ``predicted`` (the task's fact values after the steps played): ``task_start``, with each
    ``args:<unit>`` copied from the prediction when the slot is where the plan put it (observed ``pos:<unit>`` equal to
    the predicted one, not NOWHERE): it is then the slot a step left, with the locals that step gave it. The bridge's
    slot dump has no locals (patches/0001-scumm-add-speedrun-bridge.patch, the slot dump), so otherwise ``args`` is
    0, the locals unknown, and the events with args 0 (``Builder._read_once``) continue the slot. Returns (start,
    counts)."""
    out = task_start(b, observed, task)
    counts = collections.Counter()
    keys = [k for k in (task.facts if task is not None else predicted) if k.startswith("args:")]
    for k in keys:
        name = k[5:]
        pos = out.get(f"pos:{name}", NOWHERE)
        if pos != NOWHERE and predicted.get(f"pos:{name}") == pos and predicted.get(k, 0):
            out[k] = predicted[k]
            counts["args copied from the plan (the slot is where the plan put it)"] += 1
        else:
            out[k] = 0
            if pos != NOWHERE:
                counts["live slots with unknown locals (args 0)"] += 1
    return out, counts


def task_start(b: Builder, start: dict, task: T.Task | None = None) -> dict:
    """The start as the task reads it: facts.py's map with ``pos:`` and ``escpos:`` (``Builder.positions``); a unit
    the task names that the oracle's list lacks is NOWHERE."""
    out = dict(start)
    for k in (task.facts if task is not None else ()):
        if k.startswith(tuple(f"{p}:" for p in POS_KINDS)):
            out[k] = NOWHERE
    for kind, pos in (("at", "pos"), ("esc", "escpos")):
        for name in b.table.points[kind]:
            ws = [w for w in b.table.points[kind][name] if start.get(f"{kind}:{name}:{w}") == 1]
            if len(ws) > 1:
                b.counts[f"{kind}: more than one place at the start for one unit (the first taken)"] += 1
            out[f"{pos}:{name}"] = min(ws) if ws else NOWHERE
    return out
