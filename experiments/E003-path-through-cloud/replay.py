"""E003 step 5: play a plan from the planner in the engine, with real clicks, and replan on what the engine shows.

usage: from replay import Harness, boot, verify
       ctl, branch = boot(cfg_of, prefix_events, frame, out_dir)     # the engine at the test's start, from boot
       h = Harness(builder, full_task, full_task, branch, goal, packed=PACKED, workdir=DIR)   # planner taskcore
       h = Harness(builder, full_task, task, branch, goal, planner="python")   # task: the planner's (pruned) task
       log = h.run()                                                 # plays, matches, bans, replans; log is a dict
       verify(cfg_of, prefix_events + h.inputs(), h.checkpoints, goal_keys, out_dir2)   # the inputs from boot

What the harness does, per plan step (plan.md step 5, with D5):

* **The input.** A step's record (reader.Record) becomes player input through the Executor (``Executor.play``, no
  fork, so no early inputs, no re-clicks and no delay retries): a sentence (verb, A, B) is the verb's key and clicks
  on A and B, a walk_to the walk click on A (the walk verb is the reader's own, read from the scripts), a dialog line
  a left click on its verb, by the verb's number, at the point the engine's verb table and hit test give
  (``verb_point``, ``Harness._click_verb``: no text is read), a skipped cutscene Esc, and an event no input at all:
  the harness waits. ``step_input``, ``to_move`` and ``verb_point`` do the translation.
* **Waits first** (``waits_first``). Before an input step, the event steps that follow it up to the next input step
  and that neither read nor write what it and the steps they pass read or write are played first, in their order.
  The plan's order is one order of independent steps; in real time the input's own continuations (ego's arrival)
  come a few frames after the click, so a wait the plan put between them has to come before the click.
* **The camera.** A click on an object that is off screen while the camera still scrolls waits (no input) until the
  object is on screen or the camera stops (``Harness._wait_camera``): the Executor's approach walks toward a point
  computed from the camera's position, which lies behind ego while the camera catches up with it. The approach
  takes actor ``EXECUTOR_EGO`` as ego (executor.py:1008, 1011, 1045, 1099); where VAR_EGO holds another actor the
  click is an interface failure (``_check_ego``).
* **The precondition.** Before its input, a step waits (no input) until its precondition holds in the engine's
  state, up to ``INPUT_LIMIT`` frames; the input then goes on the first frame the engine takes it (the Executor's
  click gate), not after the game settles (D5). A precondition that never holds is a mismatch of the plan's
  prediction, not of the step: replan, no ban. An event whose effects already hold at its turn (it ran during an
  earlier step) counts as done.
* **The replay checks** (``eval_check``). The record's conditions on non-facts (reader.Record.checks) are checked in
  the engine's state just before the input where the dump shows their operands: in the sentence script's own code,
  global variables and its locals 0-2 (the verb, A and B: script.cpp:1212-1214); anywhere, VAR_EGO and VAR_RESULT
  of getActorX, getActorY or getActorRoom on an actor the dump lists. A variable in other code is not checked: the
  input script and the running scripts (the hover script) run between the click and the test, and may write it. Before the input, a failing check on a position
  or an actor's state whose actor is moving is waited for (no input), up to ``INPUT_LIMIT`` frames.
* **The window.** After the input (or from the step's start, for an event) the harness watches every frame and keeps
  every change to a tracked fact with its frame (``History``). The step ends on the first frame all its predicted
  writes hold. Otherwise it runs on for ``INPUT_LIMIT`` frames after the input (``WAIT_LIMIT`` for an event), or,
  once the engine took the input and none of the record's writes held, until the first idle or menu stop after it
  (the input is spent: no sentence, input or object script runs and ego stands, so the record's path, which starts
  at the input, can no longer start). The first idle or menu stop is logged per try.
* **Taken, started.** The engine took the input when a slot fact (``SLOT_PREFIXES``) changed on the input's frame
  or the next one, against the state just before the input (``_Watched``). The record's path started when one of
  its writes that changes a value held on some frame of the window.
* **The rules** (``verdict``):
  - all writes held: the step is done; a guessed write that differs is a replan (task.py's rule);
  - nothing happened (not taken, not started) or an Executor error: the input is played once more after the click
    gate reopens (plan step 5). A second Executor error is an interface failure: replan, no ban;
  - taken but not started: none of the record's tests ran, so its failure says nothing about the values it tested.
    Retried after ``RETRY_WAIT`` frames, up to ``RETRIES`` times, then a replan with no ban. An event that never
    started is a replan with no ban;
  - started, and the facts that differ were re-written by the plan's next event steps, which the engine ran within
    the window in order (``ran_on``: a script that went on past a predicted wait in the same frame):
    the step and those events are done;
  - started, then a mismatch: a replay check that failed in the engine explains it; retried when it is on time or
    a position (``RETRY_REASONS``), and either way never banned (the test is on a non-fact, which a ban cannot
    name). Otherwise, a check that could not be checked with a reason in ``RETRY_REASONS`` makes it a retry, then a
    ban. Otherwise the mismatch is at a test on facts: the action is banned for the values its path tested (the
    record's precondition facts that are not the input's engine conditions) plus the room, and the harness replans.
* **The match** (``expectations``, ``match``): the record's predicted writes, each against the engine's value, and
  the tracked facts (the full task's facts, facts.py keys from the state dump, slot positions as ``pos:``) that
  changed on any frame of the step without a predicted write (``History.unplanned``, with their frames and the value
  at the step's end; for a step the engine ran on through, the next steps' writes count as predicted). Every
  mismatch keeps the fact, the predicted values and the observed one. The locals of a slot (``args:``) and saved
  verbs (``saved:``) are not in the dump: they are not matched, and the harness keeps the plan's value for them. The
  rules read the predicted writes only: a tracked fact that changed with no prediction names no test of the path, so
  it bans nothing; the next step's precondition and the next replan see it. The report counts it (plan.md's match).
* **Bans** (``Ban``, ``apply_bans``): a ban cuts the banned point out of the precondition of every action of that
  label, so it holds wherever the planner meets those values; it needs the task written again (task.py), a replan
  without a new ban only a new start.
* **The replan**: ``reader.replan_start`` from the engine's facts and the plan's prediction: the slot positions and
  locals the played steps predicted (``Harness.pred``), so ``args:`` is copied only where the engine's slot is where
  the plan put it; then the planner.
* **The planner** (``planner``). "taskcore" (the default): ``plan_fd.plan_taskcore`` on the packed file of the
  reading's records (export_records.py), which is ``task`` (build_task's): each plan gives taskcore that start and
  every ban so far (taskcore cuts them as ``cut`` does, tested byte for byte against it), taskcore writes the reduced
  task for that start, lama-first plans on it (or anytime LAMA for ``plan_time`` seconds, ``search="anytime"``), and
  each step is the path of its reduced action whose precondition holds (or may hold) in the plan's predicted state,
  an action of ``apply_bans(task, bans)`` with its effects cut to the facts reduction (e) needs (the planner's task,
  as the pruned task's effects were cut to relevance's facts; ``waits_first`` reads them). "python": task.py's
  ``write_sas`` on ``task`` with the bans (``task``: the pruned task) and ``plan_fd.plan``; a ``write_sas`` error
  (task.py raises ValueError) stops the run with its reason. Each plan's entry keeps the time to write the task
  (``write_s``), the planner's (``seconds``), the operators, the steps and the landmarks lama-first dumped
  (``landmarks``: how many, and the game facts of the first plan's, ``Harness.landmarks``).
* **A step that is several paths** (taskcore only): a folded step whose paths the plan cannot tell apart, because they
  differ only on facts the reduced file does not hold and whose predicted value is unknown (``plan_fd.predict`` with
  ``choices``), is chosen in the engine when it is played: the path whose precondition holds in the engine's state,
  waited for (no input) up to ``INPUT_LIMIT`` frames. None is a precondition mismatch: a replan with no ban. Several
  that hold (their sets overlap at the engine's values, or they differ only where the dump does not see: ``args:``,
  ``saved:`` with no plan value) share one input, which is played once, and the step is the path whose predicted
  writes the engine shows (``_watch`` with one ``exps`` per path; the one with the most writes held when none holds
  all), judged as a single step from there. Several with different inputs (not met so far: a fold is of one label)
  stop the run ("path choice"): a replan would give the same plan from the same frame.
* **The stop**: the goal (checked on every frame), no plan (the planner proved none, or its search ended without
  one), a planner timeout, the planner's memory limit (``memory_limit``: Fast Downward's search memory, this
  laptop's budget), the replan limit, the time limit, a start outside the task's domains (task.OutOfDomain: the task
  would have to be built again; through taskcore only on a fact reduction (e) needs, as the pruned task held only
  facts relevance needs), a replan whose start and bans are the previous plan's on the same frame (``_same_input``:
  the same plan would be played the same way again), a step of several paths with different inputs, an error the
  harness does not expect ("error: ...", its traceback kept), or, through taskcore, a step whose path is not one
  (``plan_fd.PathChoice``) or a taskcore error (``plan_fd.TaskcoreError``): "planner: ...".
* **The record** (``result``): every input with its frame (the Executor's movie), every step with its tries and
  their frames, its outcome, its mismatches, the tracked facts that changed with no predicted write (each with its
  value at the step's start and end) and its wall time (``seconds``), the plans, the reorders and the bans; the
  engine's facts, tick, random state and actor positions at every input and every step's end are kept for
  ``verify``, which plays the inputs from boot in a fresh engine (no fork, no change to the random state) and
  compares. ``on_plan`` (the caller's) is called after every plan, so a long run can keep ``result`` as it goes.

C1 and C2: the harness gives only player inputs (clicks on verbs, objects and lines, walking, waiting, Esc), and
names no game's number: the verbs, objects and lines come from the plan's records, the walk verb from the reader,
the screen points from the Executor (its disclosed interface profile, with ``EXECUTOR_EGO``) and the engine's hit
test; the engine variables (VAR_EGO, VAR_SENTENCE_SCRIPT) are ScummVM's (model.py).
"""

import collections
import dataclasses
import hashlib
import json
import operator
import re
import tempfile
import time
import types
from dataclasses import dataclass, field
from pathlib import Path

import facts as F
import model as M
import plan_fd
import reader as R
import task as T
from speedrun.control import Branch, ControlError, Controller
from speedrun.executor import DIALOG_FIRST, DIALOG_LAST, MAX_ACTOR, Executor, Move, RouteError

INPUT_LIMIT = 600  # frames an input step may take (plan step 5)
WAIT_LIMIT = 3600  # frames an event (a wait) may take (plan step 5)
RETRIES = 3  # retries of a step whose failure may be a matter of time or position (plan step 5)
RETRY_WAIT = 60  # frames waited before such a retry
PLAN_TIME = 600  # seconds per plan (plan step 4)
PLAN_MEMORY = "6G"  # Fast Downward's search memory per plan: this laptop's budget (23 GB shared), not plan step 4's 32 GB
# the reasons of a replay check (reader._why; model.py's R_* reasons) that time or a position can change: retried
RETRY_REASONS = frozenset({"position", "timer", "random draw", "actor state", "message state", "loop exit by time",
                           "wait over by time", "camera position"})
TIME_CHECKS = frozenset({"position", "actor state"})  # failing checks waited for before the input (actor moving)
UNOBSERVED = ("args:", "saved:")  # facts the state dump does not hold (a slot's locals, the saved verbs)
PREDICTED = ("pos:", "escpos:") + UNOBSERVED  # the plan's values kept for the replan (``Harness.pred``)
SLOT_PREFIXES = ("script:", "local:", "objscript:", "pos:", "escpos:", "frozen:", "cutscene")  # the input taken
EXECUTOR_EGO = 1  # the actor the Executor's approach and stepping stone take as ego (executor.py:1008, 1011, 1099)
HISTORY_CAP = 12  # changes kept per fact and step in the log
# pos: and escpos: of a slot at a place none of the scope's points: the Rust reader's switch for the scope-1 finding
# (scumm/reader/src/table.rs ELSEWHERE, scumm/reader/PORT-NOTES.md "ELSEWHERE"); a reading without it holds no such value
ELSEWHERE = -2
POS_OF = {kind: pos for pos, kind in R.POS_KINDS.items()}  # "at" -> "pos", "esc" -> "escpos"


# --- a step as player input ------------------------------------------------------------------------------------------

@dataclass(frozen=True)
class Input:
    kind: str  # "click" (a sentence or a walk: the verb on A, and B), "line" (a dialog line's verb), "key", "wait"
    verb: int | None = None
    a: int | None = None
    b: int | None = None
    key: str | None = None


def step_input(rec) -> Input:
    """The player input of a record (reader.Record): its kind and what it names."""
    if rec.kind in ("sentence", "walk_to"):
        _, verb, a, b = rec.source
        return Input("click", verb, a, b)
    if rec.kind == "dialog_line":
        return Input("line", rec.source[1])
    if rec.kind == "skip_cutscene":
        return Input("key", key="esc")
    if rec.kind == "event":
        return Input("wait")
    raise ValueError(f"no input for a record of kind {rec.kind!r}")


VERB_OFFSETS = ((2, 2), (4, 3), (1, 1), (0, 0))  # points inside a verb's rectangle, from its top left, tried in order


def verb_point(verb: int, state: dict, at) -> tuple | None:
    """Where a click picks verb ``verb`` now, from the engine's verb table: the state dump lists the verbs a click can
    pick (the bridge's verbVisible: id set, not saved away, mode on, the test findVerbAtPos makes, verbs.cpp), each
    with its slot and its rectangle's top left; the point is the first of ``VERB_OFFSETS`` from there where the
    engine's hit test (``at(x, y)``, findVerbAtPos) gives that slot. None when the verb is not listed, is listed twice
    or is hit at none of them. No text is read."""
    mine = [v for v in state.get("verbs", []) if v["id"] == verb]
    if len(mine) != 1:
        return None
    v = mine[0]
    for dx, dy in VERB_OFFSETS:
        p = (v["x"] + dx, v["y"] + dy)
        if at(*p).get("verb_slot") == v["slot"]:
            return p
    return None


def to_move(inp: Input, name: str, limit: int = INPUT_LIMIT) -> Move | None:
    """The Executor's Move for a click or a key; None for a wait. A dialog line is no Move: the harness clicks its
    verb by number (``verb_point``, ``Harness._click_verb``)."""
    if inp.kind == "click":
        return Move(name, verb=inp.verb, obj=inp.a, obj2=inp.b, max_frames=limit)
    if inp.kind == "key":
        return Move(name, key=inp.key, max_frames=limit)
    if inp.kind == "line":
        raise ValueError(f"{name}: a dialog line is clicked by its verb number (Harness._click_verb), not as a Move")
    return None


# --- the plan's order --------------------------------------------------------------------------------------------------

def reads_writes(a: T.Action) -> tuple:
    """The facts an action reads (its precondition, its conditional effects' conditions, the facts it steps) and the
    facts it writes."""
    reads = set(a.pre) | {k for e in a.effects for k, _ in e.when} | {e.key for e in a.effects if e.add is not None}
    return reads, {e.key for e in a.effects}


def plain_writes(a: T.Action, now: dict) -> set:
    """The facts an action writes with a plain value that they already hold in ``now``: writes that change nothing
    there."""
    return {e.key for e in a.effects if e.value is not None and e.add is None and e.values is None and not e.when
            and now.get(e.key) == e.value}


def _is_wait(task: T.Task, step) -> bool:
    return getattr(task.actions[step.action].what, "kind", None) == "event"


def waits_first(task: T.Task, steps: list, i: int, now: dict | None = None) -> tuple:
    """``steps`` with the event steps after input step ``i``, up to the next input step, that are independent of
    step i and of every step they pass moved before step i, in their order. Independent: neither writes a fact the
    other writes, the event reads no fact the other writes, and the event changes no fact the other reads (a plain
    write of the value the fact has ``now``, the state before step i, changes nothing). Returns (the new steps, the
    moved steps)."""
    if i >= len(steps) or _is_wait(task, steps[i]):
        return steps, []
    stay = [reads_writes(task.actions[steps[i].action])]
    moved, kept = [], []
    j = i + 1
    while j < len(steps) and _is_wait(task, steps[j]):
        a = task.actions[steps[j].action]
        r, w = reads_writes(a)
        changes = w - plain_writes(a, now or {})
        if all(not (r & w2) and not (changes & r2) and not (w & w2) for r2, w2 in stay):
            moved.append(steps[j])
        else:
            kept.append(steps[j])
            stay.append((r, w))
        j += 1
    if not moved:
        return steps, []
    return steps[:i] + moved + [steps[i]] + kept + steps[j:], moved


# --- observing --------------------------------------------------------------------------------------------------------

def task_start(b, obs: dict, task) -> dict:
    """reader.task_start, with the ELSEWHERE start rule (``elsewhere``)."""
    return elsewhere(b, obs, R.task_start(b, obs, task))


def replan_start(b, obs: dict, predicted: dict, task) -> tuple:
    """reader.replan_start, with the ELSEWHERE start rule (``elsewhere``): a slot it moves to ELSEWHERE keeps args 0,
    as reader.replan_start gives a slot at NOWHERE."""
    out, counts = R.replan_start(b, obs, predicted, task)
    return elsewhere(b, obs, out), counts


def elsewhere(b, obs: dict, out: dict) -> dict:
    """The Rust reader's start rule under its switch (scumm/reader/src/task.rs ``task_start``): a unit whose slot the
    engine shows at a place none of the scope's points, and at none of them, starts at ELSEWHERE, not NOWHERE. Only the
    ``pos:``/``escpos:`` facts whose domain holds ELSEWHERE (``b.elsewhere``, packed_reading.Points) change: a reading
    made without the switch is read as before."""
    keys = getattr(b, "elsewhere", None)
    if not keys:
        return out
    pts = b.table.points
    for k, v in obs.items():
        if v != 1 or not k.startswith(("at:", "esc:")):
            continue
        kind, rest = k.split(":", 1)
        name, w = rest.rsplit(":", 1)
        key = f"{POS_OF[kind]}:{name}"
        if key in keys and out.get(key) == R.NOWHERE and w.lstrip("-").isdigit() and \
                int(w) not in pts[kind].get(name, ()):
            out[key] = ELSEWHERE
            b.counts["slots at the start at a place none of the scope's points (ELSEWHERE)"] += 1
    return out


def view(b, doms: dict, obs: dict) -> dict:
    """The engine's facts (a facts.py map) as the task's values: every key of ``doms`` (key -> Domain) but the
    unobserved ones, slot positions as ``pos:``/``escpos:`` (reader.task_start), each value labelled by its domain
    (None: the domain does not hold it)."""
    st = task_start(b, obs, types.SimpleNamespace(facts=doms))
    out = {}
    for k, dom in doms.items():
        if k.startswith(UNOBSERVED) or dom is None:
            continue
        out[k] = dom.label(st.get(k, 0))
    return out


def holds(conds: dict, state: dict) -> list:
    """The keys of ``conds`` ({key: set of values}) that ``state`` does not meet (unobserved keys skipped)."""
    return [k for k, s in conds.items() if not k.startswith(UNOBSERVED) and state.get(k) not in s]


def extra_of(state: dict, status: dict) -> dict:
    """What a checkpoint keeps besides the facts: the tick, the random state and the listed actors' positions."""
    return {"tick": status.get("tick"), "rng": status.get("rng"),
            "actors": sorted([a["id"], a["x"], a["y"]] for a in state.get("actors", []))}


class History:
    """Every change to the tracked facts during a step, with its frame."""

    def __init__(self, start: dict, frame: int):
        self.start, self.last, self.frame0 = dict(start), dict(start), frame
        self.changes: dict = {}  # key -> [(frame, value), ...]

    def note(self, frame: int, now: dict) -> None:
        for k in self.last.keys() | now.keys():
            v = now.get(k)
            if v != self.last.get(k):
                self.changes.setdefault(k, []).append((frame, v))
        self.last = dict(now)

    def slots_changed(self, first: int, last: int) -> list:
        """The slot facts that changed on a frame from ``first`` to ``last``."""
        return sorted(k for k, ch in self.changes.items()
                      if k.startswith(SLOT_PREFIXES) and any(first <= f <= last for f, _ in ch))

    def frames(self) -> list:
        """(frame, state) at the step's start and after every frame that changed a tracked fact, in frame order."""
        by = collections.defaultdict(list)
        for k, ch in self.changes.items():
            for f, v in ch:
                by[f].append((k, v))
        cur, out = dict(self.start), [(self.frame0, dict(self.start))]
        for f in sorted(by):
            cur.update(by[f])
            out.append((f, dict(cur)))
        return out

    def unplanned(self, exps) -> list:
        """The tracked facts that changed on some frame with no predicted write (``exps``: the predicted keys): the
        value at the step's start, the changes with their frames (the first ``HISTORY_CAP``), their count, and the
        value at the end (``end``; ``net``: it differs from the start's)."""
        return [{"key": k, "start": _str(self.start.get(k)), "changes": [(f, _str(v)) for f, v in ch[:HISTORY_CAP]],
                 "count": len(ch), "end": _str(ch[-1][1]), "net": ch[-1][1] != self.start.get(k)}
                for k, ch in sorted(self.changes.items()) if k not in exps]


def _str(v):
    return v if isinstance(v, int) or v is None else str(v)


class _Watched:
    """The Executor's branch, keeping the engine's state just before each input event is scheduled: the Executor
    schedules an input for the next frame, so this is the state on the frame before the input's frame."""

    def __init__(self, b: Branch):
        self._b = b
        self.before_input = None  # (frame, state dump)

    def __getattr__(self, name):
        return getattr(self._b, name)

    def inputs(self, events: list) -> None:
        if any(_is_input(e) for e in events):
            self.before_input = (self._b.frame, self._b.state())
        self._b.inputs(events)


# --- the replay checks ------------------------------------------------------------------------------------------------

_CMP = re.compile(r"^(.+?) (==|!=|<=|>=|<|>) (.+)$")
_OPS = {"==": operator.eq, "!=": operator.ne, "<=": operator.le, ">=": operator.ge, "<": operator.lt,
        ">": operator.gt}
_GETTER = re.compile(r"^(getActorX|getActorY|getActorRoom)\((\d+|VAR_EGO)\)$")


def _what(why: str) -> str:
    """The what of a check's why (``?(reason: what)``)."""
    s = why[2:] if why.startswith("?(") else why
    return s.split(":", 1)[1].strip()[:-1].strip() if ":" in s else ""


def check_getter(check) -> tuple | None:
    """(getter, actor) when the check's VAR_RESULT comes from a getter on one actor, else None."""
    m = _GETTER.match(_what(check[3]))
    return (m[1], m[2]) if m else None


def eval_check(check, rec, state: dict):
    """Whether the engine's ``state`` takes the side the record's path took on a replay check (reader.Record.checks:
    cite, condition, side, why): True, False, or None when the dump does not show the condition's operands
    (module docstring)."""
    cite, cond, side = check[0], check[1], check[2]
    if side not in (True, False):
        return None
    vs = state["vars"]
    locs = None
    if rec.kind in ("sentence", "walk_to") and len(rec.source) == 4 and rec.source[0] == "sentence":
        n = vs[M.VAR_SENTENCE_SCRIPT]
        if cite.split(":", 1)[0] == f"global/script-{n:03d}.txt":
            locs = {0: rec.source[1], 1: rec.source[2], 2: rec.source[3] or 0}
    actors = {a["id"]: a for a in state.get("actors", [])}
    getter = check_getter(check)
    read = re.fullmatch(r"var:(\d+)", _what(check[3]))  # the non-fact the reader read, when it names one

    def val(t: str):
        t = t.strip()
        if re.fullmatch(r"-?\d+", t):
            return int(t)
        m = re.fullmatch(r"Var\[(\d+)\]", t)
        if m:
            if locs is None:
                return None  # outside the sentence script's own code: another script may write it in the frame
            if read is not None and read[1] != m[1]:
                return None  # the reader read another variable there (an indirect read): not the text's
            return vs[int(m[1])] if int(m[1]) < len(vs) else None
        m = re.fullmatch(r"Local\[(\d+)\]", t)
        if m:
            return None if locs is None else locs.get(int(m[1]))
        if t == "VAR_EGO":
            return vs[M.VAR_EGO]
        if t == "VAR_RESULT" and getter is not None:
            g, who = getter
            a = actors.get(vs[M.VAR_EGO] if who == "VAR_EGO" else int(who))
            if a is None:
                return None  # not listed: in another room or not visible
            return {"getActorX": a["x"], "getActorY": a["y"], "getActorRoom": state["room"]}[g]
        return None

    m = _CMP.match(cond)
    if m:
        lv, rv = val(m[1]), val(m[3])
        if lv is None or rv is None:
            return None
        got = _OPS[m[2]](lv, rv)
    else:
        v = val(cond)
        if v is None:
            return None
        got = v != 0
    return got == side


# --- the matcher ------------------------------------------------------------------------------------------------------

@dataclass(frozen=True)
class Expect:
    key: str
    values: frozenset  # the values that match
    kind: str  # "set", "add", "guess", "when" or "copy" (reader.Record effects)
    chosen: object = None  # for a guess: the value the plan chose
    changes: bool = True  # the value before the step is not among ``values``


def expectations(rec, before: dict, doms: dict, guessed=()) -> dict:
    """The record's predicted writes as Expects, from the state ``before`` the step (the task's values): a set, a
    step (``add``: the value before plus the step; several values make it a guess), a guess (with the value the plan
    chose, plan_fd Step.guessed, where its condition holds before), a conditional write and a copy (on the source's
    value before). Unobserved facts are left out."""
    chosen = {k: v for k, v, when in guessed if all(before.get(c) in vs for c, vs in when)}  # guesses that fire
    out = {}
    for k, e in rec.effects.items():
        if k.startswith(UNOBSERVED):
            continue
        b0 = before.get(k)
        if e[0] == "set":
            vals, kind = {e[1]}, "set"
        elif e[0] == "guess":
            vals, kind = set(e[1]), "guess"
        elif e[0] == "add":
            vals = set(doms[k].add(b0, e[1])) if b0 is not None else set()
            kind = "add" if len(vals) == 1 else "guess"
        elif e[0] == "when":
            vals, kind = ({e[2]} if before.get(e[1]) == 1 else {b0}), "when"
        elif e[0] == "copy":
            vals, kind = {before.get(e[1])}, "copy"
        else:
            raise ValueError(f"unknown effect {e!r} on {k}")
        out[k] = Expect(k, frozenset(vals), kind, chosen.get(k), b0 not in vals)
    return out


@dataclass(frozen=True)
class Mismatch:
    key: str
    predicted: tuple  # the values that would match, as text
    observed: object
    kind: str  # "write" (a predicted write not seen), "guess" (a guessed write outside its values), "choice" (a
    # guessed write inside its values, not the one the plan chose), "unplanned", "precondition"


def _txt(vals) -> tuple:
    return tuple(sorted(map(str, vals)))


def match(exps: dict, now: dict) -> list:
    """The predicted writes ``now`` does not show. A guessed write is a "guess" mismatch when the value is outside
    the guess (the model's values), a "choice" when it is inside but not the one the plan chose (the planner's)."""
    out = []
    for k, x in exps.items():
        v = now.get(k)
        if v not in x.values:
            out.append(Mismatch(k, _txt(x.values), v, "guess" if x.kind == "guess" else "write"))
        elif x.kind == "guess" and x.chosen is not None and v != x.chosen:
            out.append(Mismatch(k, _txt([x.chosen]), v, "choice"))
    return out


def unplanned(before: dict, after: dict, exps: dict) -> list:
    """Tracked facts whose value differs from ``before`` to ``after`` with no predicted write (the step's ends only;
    ``History.unplanned`` has every frame)."""
    return [Mismatch(k, _txt([before.get(k)]), after.get(k), "unplanned")
            for k in sorted(set(before) | set(after)) if k not in exps and before.get(k) != after.get(k)]


def verdict(rec, mismatches: list, evals=None) -> tuple:
    """What a step whose path started and then differs leads to (module docstring): (first, after) with first one of
    "replan", "retry", "ban" and after what a retry that keeps failing leads to ("ban" or "replan"). ``evals``: the
    replay checks' results at the input (``eval_check``), one per ``rec.checks``; None: none were checked."""
    if not any(m.kind == "write" for m in mismatches):
        return "replan", None
    evals = list(evals) if evals is not None else [None] * len(rec.checks)
    failed = [c for c, ok in zip(rec.checks, evals) if ok is False]
    if failed:
        return ("retry", "replan") if any(R._why(c[3]) in RETRY_REASONS for c in failed) else ("replan", None)
    unknown = [c for c, ok in zip(rec.checks, evals) if ok is None]
    if any(R._why(c[3]) in RETRY_REASONS for c in unknown):
        return "retry", "ban"
    return "ban", None


def classify(rec, mismatches: list) -> str:
    """``verdict``'s first answer with no replay check checked (the record's check reasons alone)."""
    return verdict(rec, mismatches)[0]


# --- bans -------------------------------------------------------------------------------------------------------------

@dataclass(frozen=True)
class Ban:
    label: str  # the record's label: the input or the event (every action of that label is cut)
    values: tuple  # ((key, value), ...): the point cut out of their preconditions


def ban_of(rec, state: dict) -> Ban:
    """The ban for a record that failed in ``state`` (the task's values before the step): the values of the facts
    its path tested (its precondition facts that are not the input's engine conditions) and the room."""
    keys = sorted((set(rec.pre) - set(rec.engine_pre)) | {"room"})
    return Ban(rec.label, tuple((k, state.get(k)) for k in keys if state.get(k) is not None))


def cut(a: T.Action, ban: Ban, facts: dict) -> list:
    """Action ``a`` with the ban's point cut out of its precondition: disjoint pieces, piece i holding the first i
    keys at the banned values and key i at any other value. ``[a]`` when the precondition misses the point."""
    keys = [(k, v) for k, v in ban.values]
    cur = {k: set(a.pre[k]) if k in a.pre else set(facts[k].values) for k, _ in keys}
    if any(v not in cur[k] for k, v in keys):
        return [a]
    out = []
    for i, (k, v) in enumerate(keys):
        rest = cur[k] - {v}
        if not rest:
            continue
        pre = dict(a.pre)
        for k2, v2 in keys[:i]:
            pre[k2] = {v2}
        pre[k] = rest
        out.append(dataclasses.replace(a, name=f"{a.name}~ban:{k}", pre=pre))
    return out


def ban_pieces(task: T.Task, bans: list) -> list:
    """Per action of ``task``, in order, what it is under every ban of its label (``cut``, ban after ban): its
    pieces, ``[a]`` where no ban cuts it, ``[]`` where the bans leave nothing."""
    by = collections.defaultdict(list)
    for ban in bans:
        by[ban.label].append(ban)
    out = []
    for a in task.actions:
        pieces = [a]
        for ban in by.get(getattr(a.what, "label", None), ()):
            if all(k in task.facts for k, _ in ban.values):
                pieces = [p for q in pieces for p in cut(q, ban, task.facts)]
        out.append(pieces)
    return out


def apply_bans(task: T.Task, bans: list) -> T.Task:
    """The task with every ban applied to every action of the ban's label (``cut``)."""
    out = T.Task(task.facts, [p for ps in ban_pieces(task, bans) for p in ps], task.goal)
    out.folded = getattr(task, "folded", 0)
    return out


# --- the replan's prediction --------------------------------------------------------------------------------------------

def predict(pred: dict, rec, before: dict) -> dict:
    """The plan's prediction ``pred`` (slot positions and the unobserved facts) after ``rec`` was played from the
    state ``before``: its set writes, its conditional writes whose condition holds; a write of any other kind makes
    the value unknown (dropped)."""
    out = dict(pred)
    for k, e in rec.effects.items():
        if not k.startswith(PREDICTED):
            continue
        if e[0] == "set":
            out[k] = e[1]
        elif e[0] == "when":
            if before.get(e[1]) == 1:
                out[k] = e[2]
        else:
            out.pop(k, None)
    return out


def after(state: dict, exps: dict) -> dict:
    """``state`` with every predicted write that has one value applied: the plan's state after the step."""
    out = dict(state)
    out.update({k: next(iter(x.values)) for k, x in exps.items() if len(x.values) == 1})
    return out


def ran_on(task: T.Task, doms: dict, exps: dict, mismatches: list, before: dict, ahead: list, hist: History,
           choices=None):
    """The engine ran on: the facts this step's mismatches name were re-written, within its window, by the plan's
    next steps (``ahead``, the event steps after it up to its next input), to the values those steps predict. Each
    step of that run has its precondition in the plan's state after the steps before it, and holds on some frame no
    earlier than the one before it: its writes of those facts, or, for a step that writes none of them, any write of
    it that changes a value. Returns, when the run explains every mismatch, its steps up to the last one needed, each
    with its own mismatches (its writes that change a value and never held from its frame on); else None. A step
    that is several paths (``choices``, the harness's) is the one whose precondition holds in the plan's state there
    (none or several: None); when the run is returned, ``task.actions`` takes that path and ``choices`` drops it."""
    pending = {m.key for m in mismatches if m.kind == "write"}
    if not pending:
        return None
    state, frames, pos, chain = after(before, exps), hist.frames(), 0, []

    def holds_at(n, es, every=True):
        test = all if every else any
        return test(frames[n][1].get(e.key) in e.values for e in es)

    picked = {}
    for st in ahead:
        a = task.actions[st.action]
        if choices and st.action in choices:
            ok = [p for p in choices[st.action] if not holds(p.pre, state)]
            if len(ok) != 1:
                return None
            a = picked[st.action] = ok[0]
        if holds(a.pre, state):
            return None
        x = expectations(a.what, state, doms, st.guessed)
        need = [x[k] for k in pending if k in x]
        ch = [e for e in x.values() if e.changes]
        hit = next((n for n in range(pos, len(frames)) if (holds_at(n, need) if need else holds_at(n, ch, False))),
                   None) if (need or ch) else None
        if hit is None:
            return None
        pos = hit
        pending -= {e.key for e in need}
        missed = [Mismatch(e.key, _txt(e.values), frames[-1][1].get(e.key), "write") for e in ch
                  if not any(frames[n][1].get(e.key) in e.values for n in range(pos, len(frames)))]
        chain.append((st, missed))
        state = after(state, x)
        if not pending:
            for n, p in picked.items():
                task.actions[n] = p
                choices.pop(n, None)
            return chain
    return None


def prediction(now: dict, pred: dict) -> dict:
    """What ``reader.replan_start`` takes as the plan's predicted state: the engine's values with the plan's slot
    positions and unobserved facts over them, so its guard compares the engine's slot position with the plan's."""
    return dict(now, **pred)


# --- the engine -------------------------------------------------------------------------------------------------------

def write_events(path: Path, events: list) -> Path:
    """Input events as a SPEEDRUN_INPUTS file, in frame order (stable)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(e) + "\n" for e in sorted(events, key=lambda e: e["frame"])))
    return path


def boot(cfg_of, prefix: list, frame: int, out_dir: Path, timeout_s: float = 600) -> tuple:
    """A fresh engine from boot with the ``prefix`` input events (frames up to ``frame``), run to ``frame``'s
    decision point. ``cfg_of(out_dir, inputs_file)`` gives the EngineConfig. Returns (Controller, its root Branch)."""
    events = [e for e in prefix if e["frame"] <= frame]
    cfg = cfg_of(out_dir, write_events(out_dir / "prefix.jsonl", events))
    ctl = Controller(cfg, timeout_s=timeout_s)
    ctl.start()
    st = ctl.root.run(frame=frame)
    if st.get("frame") != frame:
        ctl.close()
        raise RuntimeError(f"the boot replay stopped at {st} instead of frame {frame}")
    return ctl, ctl.root


def compare_checkpoint(want: tuple, state: dict, status: dict) -> dict:
    """The differences between a checkpoint (frame, facts.py map[, extra_of]) and the engine's state and status."""
    out = {}
    d = F.diff(want[1], F.facts_of(state))
    if d:
        out["facts"] = len(d)
        out["first"] = sorted(d.items())[:8]
    if len(want) > 2:
        got = extra_of(state, status)
        for k, v in want[2].items():
            if got.get(k) != v:
                out[k] = {"want": v, "got": got.get(k)}
    return out


def verify(cfg_of, events: list, checkpoints: list, goal_keys, out_dir: Path) -> dict:
    """Play ``events`` from boot in a fresh engine (no fork, no change to the random state) and compare its facts,
    tick, random state and actor positions with ``checkpoints`` ([(frame, facts.py map, extra_of)], in frame
    order). Returns the counts and the first differences."""
    cfg = cfg_of(out_dir, write_events(out_dir / "movie.jsonl", events))
    out = {"checkpoints": len(checkpoints), "differ": [], "inputs": sum(1 for e in events if _is_input(e))}
    with Controller(cfg, timeout_s=600) as c:
        b = c.root
        for cp in checkpoints:
            frame = cp[0]
            st = b.run(frame=frame)
            if st.get("frame") != frame:
                out["differ"].append({"frame": frame, "error": f"stopped at {st}"})
                break
            d = compare_checkpoint(cp, b.state(), b.status)
            if d:
                out["differ"].append(dict(frame=frame, **d))
        if checkpoints:
            last = F.facts_of(b.state())
            out["final"] = {k: last.get(k, 0) for k in goal_keys}
    out["same"] = not out["differ"]
    return out


def _is_input(e: dict) -> bool:
    return any(k in e for k in ("key", "click", "rclick", "move"))


# --- the harness ------------------------------------------------------------------------------------------------------

class Stop(Exception):
    pass


@dataclass
class StepLog:
    plan: int  # which plan (0: the first)
    index: int  # the step's index in that plan (after the waits moved first)
    name: str
    kind: str
    input: str
    start: int  # frame the step began
    issue: int | None = None  # frame of its last input
    end: int | None = None  # frame it ended
    outcome: str = ""  # matched, already happened, precondition, nothing happened, path not started, did not
    # happen, interface failure, mismatch, goal
    then: str = ""  # next, replan, ban, goal
    tries: list = field(default_factory=list)  # per try: kind, start, issue, first settled, end, how, taken, started
    mismatches: list = field(default_factory=list)  # Mismatch as dicts (the last try's)
    unplanned: list = field(default_factory=list)  # History.unplanned (the last try's)
    checks: list = field(default_factory=list)  # the replay checks at the last input: (cite, condition, side, why,
    # result)
    inputs: list = field(default_factory=list)  # the input events it sent
    note: str = ""
    seconds: float = 0.0  # wall time of the step, from its start to its end

    def add(self, note: str) -> None:
        self.note = (self.note + "; " if self.note else "") + note


class Harness:
    """Plays plans of ``task`` (the planner's task: actions whose ``what`` is a reader.Record) in the engine at
    ``branch`` until ``goal`` holds (module docstring). ``full_task``'s facts are the tracked facts and the domains
    the matcher reads."""

    def __init__(self, b, full_task: T.Task, task: T.Task, branch: Branch, goal: dict, *, release: str = "dos-cd",
                 replan_limit: int = 500, time_limit: float = 24 * 3600, plan_time: int = PLAN_TIME,
                 progress=None, planner: str = "taskcore", packed=None, search: str = "lama-first", workdir=None,
                 taskcore_args=(), keep_files: bool = False, memory_limit: str = PLAN_MEMORY, on_plan=None):
        if planner not in ("taskcore", "python"):
            raise ValueError(f"planner {planner!r}: taskcore or python")
        self.b, self.base = b, task
        self.planner, self.packed, self.search, self.taskcore_args = planner, packed, search, tuple(taskcore_args)
        self.memory_limit = memory_limit
        self.on_plan = on_plan  # called with the harness after each plan (the caller's snapshot of ``result``)
        self.workdir = Path(workdir) if workdir is not None else None
        self.keep_files = keep_files
        self.pieces = None  # per action of ``task``, its pieces under the bans (the taskcore planner)
        self.pieces_bans = None
        self.doms = dict(full_task.facts)
        self.doms.update(task.facts)
        self.goal = goal
        self.branch = branch
        self.watched = _Watched(branch)
        self.ex = Executor(self.watched, release, fork=None)
        self.replan_limit, self.time_limit, self.plan_time = replan_limit, time_limit, plan_time
        self.progress = progress or (lambda msg: None)
        self.bans: list = []
        self.task: T.Task | None = None
        self.sas = None
        self.sas_bans = None  # the bans the current SAS file holds
        self.plans: list = []  # per plan: status, seconds, steps, start counts, reorders
        self.steps: list = []  # StepLog per step played
        self.checkpoints: list = []  # (frame, facts.py map, extra_of) at each input and step end, for ``verify``
        self.replans = 0
        self.stop_reason = None
        self.obs: dict = {}
        self.now: dict = {}
        self.pred: dict = {}  # the plan's slot positions and unobserved facts (PREDICTED), for the replan
        self.kept: dict = {}  # the plan's values of the unobserved facts (args:, saved:), for the preconditions
        self.prev_before = None
        self.ran: list = []  # the steps the engine ran on through in the last step's window (``ran_on``)
        self.frame0 = self.tick0 = None
        self.choices: dict = {}  # step's action index -> its paths, chosen in the engine (module docstring)
        self.landmarks = None  # the first plan's landmarks (plan_fd.Landmark), lama-first's dump
        self.t0 = None

    # --- observing ------------------------------------------------------------------------------------------------

    def observe(self) -> None:
        s = self.branch.state()
        self.state = s
        self.obs = F.facts_of(s)
        self.now = view(self.b, self.doms, self.obs)

    def belief(self) -> dict:
        """The engine's values with the plan's values of the facts the dump does not hold."""
        return dict(self.now, **self.kept)

    def advance(self, n: int = 1) -> None:
        st = self.branch.run(frame=self.branch.frame + n)
        if "end" in st:
            raise Stop(f"engine ended: {st['end']}")
        self.observe()

    def goal_holds(self) -> bool:
        return not holds(self.goal, self.now)

    def settled(self) -> bool:
        st = self.branch.status
        menu = st.get("up", 0) > 0 and st.get("cs", 1) == 0 and any(
            DIALOG_FIRST <= v["id"] <= DIALOG_LAST for v in self.state["verbs"])
        return "idle" in st or menu

    def checkpoint(self) -> None:
        if not self.checkpoints or self.checkpoints[-1][0] != self.branch.frame:
            self.checkpoints.append((self.branch.frame, dict(self.obs), extra_of(self.state, self.branch.status)))

    def _set_pred(self, pred: dict) -> None:
        self.pred = pred
        self.kept = {k: v for k, v in pred.items() if k.startswith(UNOBSERVED)}

    # --- planning -------------------------------------------------------------------------------------------------

    def plan(self, first: bool) -> list:
        """A plan from the engine's state (reader.task_start for the first, reader.replan_start after); [] when the
        planner gives none (the stop reason is set)."""
        self.choices = {}
        if self.planner == "taskcore":
            return self._plan_taskcore(first)
        if self.sas is None or self.sas_bans != len(self.bans):
            self.progress(f"writing the task ({len(self.bans)} bans)")
            self.task = apply_bans(self.base, self.bans)
            t = time.monotonic()
            try:
                self.sas = T.write_sas(self.task)
            except ValueError as e:  # task.py's writer: a value outside a domain, PRE_LIMIT, two writes of a fact
                self.plans.append({"plan": len(self.plans), "frame": self.branch.frame, "bans": len(self.bans),
                                   "write_s": time.monotonic() - t, "start_counts": {}, "reorders": [],
                                   "status": "write error", "error": str(e), "steps": []})
                self.sas = None
                self.stop_reason = f"planner: write_sas: {e}"
                return []
            self.sas_bans = len(self.bans)
            write_s = time.monotonic() - t
        else:
            write_s = 0.0
        if first:
            start, counts = task_start(self.b, self.obs, self.task), {}
        else:
            start, counts = replan_start(self.b, self.obs, prediction(self.now, self.pred), self.task)
        entry = {"plan": len(self.plans), "frame": self.branch.frame, "bans": len(self.bans), "write_s": write_s,
                 "start_counts": dict(counts), "reorders": []}
        self.plans.append(entry)
        if self._same_input(entry, start):
            return []
        try:
            self.sas.state(start)
        except T.OutOfDomain as e:
            entry.update(status="out of domain", misses=[(k, v) for k, v in e.misses[:20]], steps=[])
            self.stop_reason = f"the engine's state is outside the task's domains ({len(e.misses)} facts)"
            return []
        self.progress(f"planning (plan {entry['plan']}, frame {self.branch.frame})")
        res = plan_fd.plan(self.sas, start, time_limit=self.plan_time, memory_limit=self.memory_limit)
        entry.update(status=res.status, seconds=res.seconds, steps=[s.name for s in res.steps],
                     operators=self.sas.counts["operators"])
        self._landmarks(entry, res)
        return self._planned(entry, res, start)

    def _same_input(self, entry: dict, start: dict) -> bool:
        """The planner's input (the start, the bans) at this frame is the previous plan's, on the same frame: the
        planner is deterministic, so it would give the same plan, and the engine has not moved, so the harness would
        play it the same way again. The run stops (status "same input") instead of spinning to the replan limit."""
        key = hashlib.sha1(json.dumps([self.branch.frame, len(self.bans), sorted((k, str(v)) for k, v in
                                                                                  start.items())]).encode()).hexdigest()
        entry["input_key"] = key[:16]
        prev = self.plans[-2] if len(self.plans) > 1 else None
        if prev is None or prev.get("input_key") != entry["input_key"]:
            return False
        entry.update(status="same input", steps=[])
        self.stop_reason = (f"replan at frame {self.branch.frame} with plan {prev['plan']}'s start and bans and no frame "
                            f"played since: the same plan again")
        return True

    def _landmarks(self, entry: dict, res) -> None:
        """The landmarks lama-first dumped for this plan: counted per plan, the first plan's kept."""
        game = [lm for lm in res.landmarks if not lm.aux]
        entry.update(landmarks=len(res.landmarks), landmark_orderings=len(res.orderings), game_landmarks=len(game))
        if self.landmarks is None and res.landmarks:
            self.landmarks = {"plan": entry["plan"], "frame": entry["frame"],
                              "nodes": [dataclasses.asdict(lm) for lm in res.landmarks],
                              "orderings": [list(o) for o in res.orderings]}

    def _planned(self, entry: dict, res, start: dict) -> list:
        self.progress(f"plan {entry['plan']}: {res.status}, {len(res.steps)} steps, {res.seconds:.0f} s")
        if res.status != "plan":
            why = {"time": f"planner timeout ({self.plan_time} s)", "unsolvable": "no plan (the planner proved none)",
                   "incomplete": "no plan (the search ended without one)",
                   "memory": f"planner memory limit ({self.memory_limit})"}
            self.stop_reason = why.get(res.status, f"planner {res.status}")
            return []
        # the plan's slot positions and unobserved facts at its start
        self._set_pred({k: self.task.facts[k].label(start.get(k, 0)) for k in self.task.facts
                        if k.startswith(PREDICTED)})
        return list(res.steps)

    def _plan_taskcore(self, first: bool) -> list:
        """``plan`` through taskcore (module docstring): the start and the bans as files, taskcore's reduced task,
        the search, the plan read back to paths of the banned task; ``self.task`` is the plan's steps' actions."""
        if self.pieces is None or self.pieces_bans != len(self.bans):
            # a task read from its packed file (packed_reading.PackedTask) cuts one action at a time, when asked
            self.pieces = self.base.ban_pieces(self.bans) if hasattr(self.base, "ban_pieces") else \
                ban_pieces(self.base, self.bans)
            self.pieces_bans = len(self.bans)
        if first:
            start, counts = task_start(self.b, self.obs, self.base), {}
        else:
            start, counts = replan_start(self.b, self.obs, prediction(self.now, self.pred), self.base)
        n = len(self.plans)
        entry = {"plan": n, "frame": self.branch.frame, "bans": len(self.bans), "start_counts": dict(counts),
                 "reorders": [], "planner": "taskcore", "search": self.search}
        self.plans.append(entry)
        if self._same_input(entry, start):
            return []
        if self.workdir is None:
            self.workdir = Path(tempfile.mkdtemp(prefix="replay-taskcore-"))
        self.progress(f"planning through taskcore (plan {n}, frame {self.branch.frame}, {len(self.bans)} bans)")
        try:
            res = plan_fd.plan_taskcore(self.packed, self.base.facts, self.goal, self.pieces, start, self.bans,
                                        time_limit=self.plan_time, memory_limit=self.memory_limit,
                                        search=self.search, workdir=self.workdir / f"plan-{n:03d}",
                                        args=self.taskcore_args, keep=self.keep_files)
        except (plan_fd.PathChoice, plan_fd.TaskcoreError) as e:
            entry.update(status="path choice" if isinstance(e, plan_fd.PathChoice) else "taskcore error",
                         error=str(e), steps=[])
            self.stop_reason = f"planner: {e}"
            return []
        sc = res.stats.get("start_check", {})
        entry.update(write_s=res.write_s, status=res.status, seconds=res.seconds, steps=[s.name for s in res.steps],
                     operators=res.operators, actions=res.stats.get("actions"), decode_s=res.decode_s,
                     folds=res.folds, predicted_goal=res.predicted_goal, file_facts=len(res.file_facts),
                     needed_facts=len(res.needed_facts),
                     input_check_misses=sc.get("facts_the_input_check_stops_on"),
                     choices={str(i): [p.name for p in ps] for i, ps in res.choices.items()})
        self._landmarks(entry, res)
        if res.status == "out of domain":
            entry.update(misses=res.misses, steps=[])
            self.stop_reason = f"the engine's state is outside the task's domains ({res.n_misses} facts)"
            return []
        self.task = T.Task(self.base.facts, list(res.actions), self.base.goal)
        self.choices = dict(res.choices)
        return self._planned(entry, res, start)

    # --- one step -------------------------------------------------------------------------------------------------

    def _wait_for(self, test, limit: int) -> bool:
        """Run frames (no input) until ``test()`` holds, up to ``limit`` frames; checks the goal on every frame."""
        end = self.branch.frame + limit
        while not test():
            if self.goal_holds():
                return False
            if self.branch.frame >= end:
                return False
            self.advance()
        return True

    def _watch(self, alts: list, start: int, limit: int, t: dict, hist: History, inputs: bool) -> tuple:
        """Every frame from now until all the writes of one of ``alts`` (the step's predicted writes, one ``exps``
        per path: one, or the paths of a step the engine's state cannot tell apart) hold, ``limit`` frames after
        ``start``, or (``inputs``) the input is spent. Returns (how, the state with the most writes held, started,
        taken, which): how is "held", "goal", "limit" or "spent"; which the path whose writes all held (for "held"),
        else the one with the most writes held."""
        end = start + limit
        best, started, taken = None, False, False
        while True:
            hist.note(self.branch.frame, self.now)
            helds = [[k for k, x in exps.items() if self.now.get(k) in x.values] for exps in alts]
            started = started or any(exps[k].changes for exps, held in zip(alts, helds) for k in held)
            if inputs and not taken and self.branch.frame <= start + 1:
                taken = bool(hist.slots_changed(start, start + 1))
            top = max(range(len(alts)), key=lambda i: len(helds[i]))
            if best is None or len(helds[top]) > best[0]:
                best = (len(helds[top]), dict(self.now), top)
            settled = self.branch.frame > start and self.settled()
            if t.get("first_settled") is None and settled:
                t["first_settled"] = self.branch.frame
            full = [i for i, exps in enumerate(alts) if len(helds[i]) == len(exps)]
            if full:
                return "held", dict(self.now), started, taken, full[0]
            if self.goal_holds():
                return "goal", best[1], started, taken, best[2]
            if inputs and taken and not started and settled and self.branch.frame > start + 1 and \
                    any(x.changes for exps in alts for x in exps.values()):
                return "spent", best[1], started, taken, best[2]
            if self.branch.frame >= end:
                return "limit", best[1], started, taken, best[2]
            self.advance()

    def _wait_camera(self, inp: Input, limit: int, log: StepLog) -> None:
        """A click on an object that is off screen while the camera still moves: run frames (no input) until the
        object is on screen or the camera stops. The Executor's approach floor-clicks toward the object's walk point
        clipped to the screen, computed from the camera's position; while the camera is still catching up with ego
        (ego past the screen's edge on a room's scroll), that point lies behind ego and the walk goes the wrong way."""
        objs = [o for o in (inp.a, inp.b) if o is not None]
        end, waited = self.branch.frame + limit, 0
        while self.branch.frame < end:
            if all(o in self.state["inventory"] or self.ex.object_points(o, self.state) for o in objs):
                break
            cam = self.branch.status.get("cam")
            self.advance()
            waited += 1
            if self.branch.status.get("cam") == cam:
                break
        if waited:
            log.add(f"waited {waited} frames for the camera")

    def _check_ego(self, inp: Input) -> None:
        """The Executor approaches an object that is off screen (not in the inventory, not a listed actor) with
        actor ``EXECUTOR_EGO`` as ego: an interface failure where VAR_EGO holds another actor."""
        s = self.state
        for o in (inp.a, inp.b):
            if o is None or o in s["inventory"] or self.ex.object_points(o, s):
                continue
            if o <= MAX_ACTOR and all(r["id"] != o for r in s["room_objects"]):
                continue  # an actor: the Executor waits for it (Executor._wait_actor)
            if s["vars"][M.VAR_EGO] != EXECUTOR_EGO:
                raise RouteError(f"interface: object {o} is off screen and the Executor's approach takes actor "
                                 f"{EXECUTOR_EGO} as ego, but VAR_EGO is {s['vars'][M.VAR_EGO]}")

    def _wait_checks(self, rec, log: StepLog) -> None:
        """Before an input: wait (no input), up to ``INPUT_LIMIT`` frames, while a replay check on a position or an
        actor's state fails in the engine and its actor is moving (``TIME_CHECKS``)."""
        def waiting() -> bool:
            for c in rec.checks:
                g = check_getter(c)
                if R._why(c[3]) not in TIME_CHECKS or g is None or eval_check(c, rec, self.state) is not False:
                    continue
                who = self.state["vars"][M.VAR_EGO] if g[1] == "VAR_EGO" else int(g[1])
                if any(a["id"] == who and a.get("moving") for a in self.state.get("actors", [])):
                    return True
            return False
        f0 = self.branch.frame
        self._wait_for(lambda: not waiting(), INPUT_LIMIT)
        if self.branch.frame > f0:
            log.add(f"waited {self.branch.frame - f0} frames for the record's position checks")

    def _click_verb(self, verb: int, name: str) -> None:
        """A dialog line: a left click on verb ``verb`` (its number, from the record), at the point ``verb_point``
        reads from the engine's verb table, on the first frame the engine takes a verb click (user input on:
        checkExecVerbs, verbs.cpp, and the verb listed and hit), waiting for it (no input) up to ``INPUT_LIMIT``
        frames; then run to the click's frame, as the Executor does for a click. RouteError when it never comes. The
        click goes into the Executor's movie, so verify plays it."""
        end = self.branch.frame + INPUT_LIMIT
        while True:
            p = verb_point(verb, self.state, self.watched.at) if self.branch.status.get("up", 0) > 0 else None
            if p is not None:
                break
            if self.branch.frame >= end:
                raise RouteError(f"{name}: verb {verb} cannot be clicked within {INPUT_LIMIT} frames (not listed in "
                                 f"the verb table, not hit there, or user input off)")
            self.advance()
        ev = [{"frame": self.branch.frame + 1, "click": list(p)}]
        self.watched.inputs(ev)
        self.ex.movie.extend(ev)
        self.advance(1)

    def _play_input(self, inp: Input, name: str, log: StepLog, t: dict) -> bool:
        """The step's input through the Executor (a dialog line: ``_click_verb``); False on an Executor error
        (logged)."""
        mark = len(self.ex.movie)
        self.watched.before_input = None
        try:
            if inp.kind == "click":
                self._wait_camera(inp, INPUT_LIMIT, log)
                self._check_ego(inp)
            if inp.kind == "line":
                self._click_verb(inp.verb, name)
            else:
                self.ex.play(to_move(inp, name))
            ok = True
        except RouteError as e:
            log.add(f"executor: {e}")
            t["error"] = str(e)
            ok = False
        new = self.ex.movie[mark:]
        log.inputs += new
        ins = [e["frame"] for e in new if _is_input(e)]
        t["inputs"] = [e for e in new if _is_input(e)]
        t["issue"] = log.issue = max(ins) if ins else self.branch.frame
        self.observe()
        if ins:
            self.checkpoint()
        return ok

    def play_step(self, n_plan: int, i: int, step, ahead=()) -> str:
        """Play one plan step; returns "next", "replan", "goal" or "stop" (the stop reason set) (module docstring).
        ``ahead``: the plan's event steps after it up to its next input, for ``ran_on`` (the number the engine ran on
        through is ``self.ran``)."""
        a = self.task.actions[step.action]
        rec = a.what
        inp = step_input(rec)
        is_event = inp.kind == "wait"
        limit = WAIT_LIMIT if is_event else INPUT_LIMIT
        log = StepLog(n_plan, i, step.name, rec.kind, _input_text(inp), self.branch.frame)
        self.steps.append(log)
        retries, replays, kind = 0, 0, "first"
        alts = None  # the paths the engine's state cannot tell apart, played as one input (``_choose_path``)
        t_step = time.monotonic()
        try:
            if step.action in self.choices:
                got = self._choose_path(step, log)
                if got is None:
                    return log.then
                if isinstance(got, list):
                    alts, a = got, got[0]
                else:
                    a = got
                rec = a.what
            while True:
                # the precondition (or, for an event, its effects already there)
                bad = holds(a.pre, self.belief())
                if bad:
                    if is_event and self._already(rec, step):
                        self._set_pred(predict(self.pred, rec, self.prev_before or self.belief()))
                        log.outcome, log.then, log.end = "already happened", "next", self.branch.frame
                        return "next"
                    ok = self._wait_for(lambda: not holds(a.pre, self.belief()), INPUT_LIMIT)
                    if self.goal_holds():
                        log.outcome, log.then, log.end = "goal", "goal", self.branch.frame
                        return "goal"
                    if not ok:
                        b = self.belief()
                        log.outcome, log.then, log.end = "precondition", "replan", self.branch.frame
                        log.mismatches = [dataclasses.asdict(Mismatch(k, _txt(a.pre[k]), b.get(k), "precondition"))
                                          for k in holds(a.pre, b)]
                        return "replan"
                if not is_event and rec.checks:
                    self._wait_checks(rec, log)
                t = {"try": len(log.tries) + 1, "kind": kind, "start": self.branch.frame}
                log.tries.append(t)
                before = self.belief()
                hist = History(self.now, self.branch.frame)
                self.prev_before = before
                cands = alts or [a]
                alt_exps = [expectations(p.what, before, self.doms, step.guessed) for p in cands]
                evals = None
                if is_event:
                    ok, start = True, self.branch.frame
                else:
                    ok = self._play_input(inp, step.name, log, t)
                    start = t["issue"]
                    pre = self.watched.before_input
                    if pre is not None:
                        hist.note(pre[0], view(self.b, self.doms, F.facts_of(pre[1])))
                if ok:
                    how, seen, started, taken, which = self._watch(alt_exps, start, limit, t, hist, not is_event)
                else:
                    how, seen, started, taken, which = "error", dict(self.now), False, False, 0
                a, exps = cands[which], alt_exps[which]
                rec = a.what
                if not is_event and self.watched.before_input is not None:
                    evals = [eval_check(c, rec, self.watched.before_input[1]) for c in rec.checks]
                    log.checks = [list(c[:4]) + [ok_] for c, ok_ in zip(rec.checks, evals)]
                if alts:
                    t["path"] = a.name
                t.update(end=self.branch.frame, how=how, started=started, taken=taken)
                log.end = self.branch.frame
                log.unplanned = hist.unplanned(exps)
                if how == "goal":
                    log.outcome, log.then = "goal", "goal"
                    log.mismatches = [dataclasses.asdict(m) for m in match(exps, seen)]
                    return "goal"
                if how == "held":
                    log.outcome = "matched"
                    if alts:
                        self._take_path(step, a, alts, log)
                    guesses = match(exps, seen)
                    log.mismatches = [dataclasses.asdict(m) for m in guesses]
                    self._set_pred(predict(self.pred, rec, before))
                    log.then = "replan" if guesses else "next"
                    return log.then
                mism = match(exps, seen)
                log.mismatches = [dataclasses.asdict(m) for m in mism]
                if how == "error":
                    if replays < 1:
                        replays, kind = replays + 1, "replay"
                        log.add("an Executor error counts as nothing happening: played once more")
                        continue
                    log.outcome, log.then = "interface failure", "replan"
                    log.add("no ban: the Executor could not play the input")
                    return "replan"
                if not started:
                    if is_event:
                        log.outcome, log.then = "did not happen", "replan"
                        log.add("no ban: none of the event's writes held, so its path never ran")
                        return "replan"
                    if not taken and replays < 1:
                        replays, kind = replays + 1, "replay"
                        log.add(f"nothing happened by frame {self.branch.frame}: played once more")
                        continue  # the Executor waits for the click gate
                    log.outcome = "path not started" if taken else "nothing happened"
                    if retries < RETRIES:
                        retries, kind = retries + 1, "retry"
                        log.add(f"retry {retries} after {RETRY_WAIT} frames (the record's path never started)")
                        self.advance(RETRY_WAIT)
                        continue
                    log.then = "replan"
                    log.add("no ban: the record's path never started, so none of its tests ran")
                    return "replan"
                if alts:
                    self._take_path(step, a, alts, log)
                chain = ran_on(self.task, self.doms, exps, mism, before, ahead, hist, self.choices)
                if chain:
                    log.outcome, log.then = "matched, the engine ran on", "next"
                    log.add(f"the facts that differ were re-written within the window by the plan's next steps "
                            f"{[st.name for st, _ in chain]}, which are done too")
                    pred, state, keys = predict(self.pred, rec, before), after(before, exps), set(exps)
                    for st, _ in chain:
                        r2 = self.task.actions[st.action].what
                        x2 = expectations(r2, state, self.doms, st.guessed)
                        pred, state = predict(pred, r2, state), after(state, x2)
                        keys |= set(x2)
                    log.unplanned = hist.unplanned(keys)  # the chain's writes are predicted too
                    self._set_pred(pred)
                    self.ran = chain
                    return "next"
                log.outcome = "mismatch"
                first, then = verdict(rec, mism, evals)
                if first == "retry" and retries < RETRIES:
                    retries, kind = retries + 1, "retry"
                    log.add(f"retry {retries} after {RETRY_WAIT} frames")
                    self.advance(RETRY_WAIT)
                    continue
                v = then if first == "retry" else first
                if v == "ban":
                    ban = ban_of(rec, before)
                    self.bans.append(ban)
                    log.add(f"ban {ban.label} at {dict(ban.values)}")
                    log.then = "ban"
                else:
                    log.then = "replan"
                return "replan"
        finally:
            if log.end is None:
                log.end = self.branch.frame
            log.issue = log.tries[-1].get("issue") if log.tries else None
            log.seconds = time.monotonic() - t_step
            self.checkpoint()  # the engine's facts at the step's end, for verify
            self.progress(f"  {log.outcome} -> {log.then} at frame {log.end} (tries {len(log.tries)}); mismatches "
                          f"{[(m['key'], m['predicted'], m['observed']) for m in log.mismatches[:6]]}; "
                          f"{len(log.unplanned)} unplanned facts changed; {log.note}")

    def _choose_path(self, step, log: StepLog):
        """A step that is several paths (module docstring): the one whose precondition holds in the engine's state
        (and, on the facts the dump does not hold, the plan's value where it has one), waited for (no input) up to
        ``INPUT_LIMIT`` frames; it replaces the step's action. Several that hold (overlapping sets, or sets apart only
        where the engine cannot see: ``args:``, ``saved:``) and share one input are returned as a list, played once,
        and the path whose writes the engine shows is taken (``_watch``, ``_take_path``). None (``log.then`` says what
        follows)
        when none holds (outcome "precondition": replan, no ban), several hold with different inputs (outcome "path
        choice": the run stops, as a replan would give the same plan from the same frame), or the goal came first
        (outcome "goal")."""
        cands = self.choices[step.action]

        def held():
            b = self.belief()
            return [p for p in cands if not holds(p.pre, b)
                    and all(b[k] in vs for k, vs in p.pre.items() if k.startswith(UNOBSERVED) and k in b)]
        if not held():
            self._wait_for(lambda: bool(held()), INPUT_LIMIT)
        ok = held()
        if self.goal_holds():
            log.outcome, log.then, log.end = "goal", "goal", self.branch.frame
            return None
        if len(ok) > 1 and len({step_input(p.what) for p in ok}) == 1:
            log.add(f"the engine's state holds {len(ok)} of the step's {len(cands)} paths {[p.name for p in ok][:4]}, "
                    f"one input: played once, the path is the one whose writes the engine shows")
            return ok
        if len(ok) != 1:
            b = self.belief()
            log.outcome, log.end = ("path choice" if ok else "precondition"), self.branch.frame
            log.add(f"the engine's state holds {len(ok)} of the step's {len(cands)} paths "
                    f"{[p.name for p in (ok or cands)][:4]}: no ban")
            if ok:
                log.then = "stop"
                self.stop_reason = (f"path choice: step {step.name} holds {len(ok)} paths with different inputs in "
                                    f"the engine's state")
            else:
                log.then = "replan"
                keys = sorted({k for p in cands for k in holds(p.pre, b)})
                log.mismatches = [dataclasses.asdict(Mismatch(k, _txt(set().union(*(p.pre.get(k, ()) for p in cands))),
                                                              b.get(k), "precondition")) for k in keys[:12]]
            return None
        self._take_path(step, ok[0], cands, log)
        return ok[0]

    def _take_path(self, step, a, cands, log: StepLog) -> None:
        """The step is path ``a`` of ``cands`` from now on (the plan's task, ``choices``)."""
        self.choices.pop(step.action, None)
        self.task.actions[step.action] = a
        log.add(f"the path the engine took: {a.name}, of {[p.name for p in cands][:6]}")

    def _already(self, rec, step) -> bool:
        """An event whose effects hold at its turn (it ran during the previous step): every write that changes a
        value, judged from the state at the previous step's start."""
        before = self.prev_before or self.belief()
        exps = expectations(rec, before, self.doms, step.guessed)
        changed = [x for x in exps.values() if x.changes]
        return bool(changed) and all(self.now.get(x.key) in x.values for x in changed)

    # --- the run --------------------------------------------------------------------------------------------------

    def run(self) -> dict:
        """Plan and play until a stop (module docstring); the record (``result``). An exception the harness does not
        expect stops the run too, with its reason and traceback in the record, so a long run keeps what it did."""
        self.t0 = time.monotonic()
        self.observe()
        self.frame0, self.tick0 = self.branch.frame, self.branch.status.get("tick")
        self.checkpoint()
        error = None
        try:
            steps = self._plan(first=True)
            n_plan, i = 0, 0
            while True:
                if self.goal_holds():
                    self.stop_reason = "goal"
                    break
                if not steps and self.stop_reason:
                    break
                if time.monotonic() - self.t0 > self.time_limit:
                    self.stop_reason = "time limit"
                    break
                if i >= len(steps):
                    how = "replan"  # the plan is played out and the goal does not hold
                else:
                    steps, moved = waits_first(self.task, steps, i, self.belief())
                    if moved:
                        self.plans[-1]["reorders"].append({"input": steps[i + len(moved)].name,
                                                           "moved": [s.name for s in moved]})
                        self.progress(f"plan {n_plan}: {[s.name for s in moved]} played before "
                                      f"{steps[i + len(moved)].name}")
                    self.progress(f"plan {n_plan} step {i + 1}/{len(steps)}: {steps[i].name} (frame "
                                  f"{self.branch.frame})")
                    ahead = []
                    while i + 1 + len(ahead) < len(steps) and _is_wait(self.task, steps[i + 1 + len(ahead)]):
                        ahead.append(steps[i + 1 + len(ahead)])
                    how = self.play_step(n_plan, i, steps[i], ahead)
                if how == "goal":
                    self.stop_reason = "goal"
                    break
                if how == "stop":
                    break
                if how == "next":
                    for k, (s, missed) in enumerate(self.ran):  # done within the last step's window
                        self.steps.append(StepLog(n_plan, i + 1 + k, s.name, "event", "wait", self.branch.frame,
                                                  end=self.branch.frame, outcome="ran on in the step before",
                                                  then="next", mismatches=[dataclasses.asdict(m) for m in missed]))
                    i += 1 + len(self.ran)
                    self.ran = []
                    continue
                if self.replans >= self.replan_limit:
                    self.stop_reason = "replan limit"
                    break
                self.replans += 1
                steps = self._plan(first=False)
                n_plan, i = len(self.plans) - 1, 0
        except (Stop, ControlError) as e:
            self.stop_reason = f"engine: {e}"
        except Exception as e:  # noqa: BLE001 (the record is kept; the reason and the traceback say what happened)
            import traceback
            self.stop_reason = f"error: {type(e).__name__}: {e}"
            error = traceback.format_exc()
        try:
            self.observe()
            self.checkpoint()
        except (Stop, ControlError) as e:
            error = (error or "") + f"\nthe last observation failed: {e}"
        out = self.result()
        if error:
            out["error"] = error
        return out

    def _plan(self, first: bool) -> list:
        steps = self.plan(first)
        if self.on_plan is not None:
            self.on_plan(self)
        return steps

    def result(self) -> dict:
        """The record of the run so far (module docstring), from what the harness holds: no engine call."""
        st = self.branch.status
        return {
            "stop": self.stop_reason, "goal": not holds(self.goal, self.now), "replans": self.replans,
            "bans": [dataclasses.asdict(b) for b in self.bans], "plans": self.plans,
            "steps": [dataclasses.asdict(s) for s in self.steps],
            "frames": st.get("frame", 0) - (self.frame0 or 0), "ticks": (st.get("tick") or 0) - (self.tick0 or 0),
            "frame0": self.frame0, "frame_end": st.get("frame"),
            "seconds": time.monotonic() - self.t0 if self.t0 is not None else 0.0, "inputs": self.inputs(),
        }

    def inputs(self) -> list:
        """Every input event the harness sent (the Executor's movie), with its frame."""
        return list(self.ex.movie)


def _input_text(inp: Input) -> str:
    if inp.kind == "click":
        return f"click verb {inp.verb} on {inp.a}" + (f" with {inp.b}" if inp.b is not None else "")
    if inp.kind == "line":
        return f"click dialog line {inp.verb}"
    if inp.kind == "key":
        return f"key {inp.key}"
    return "wait"
