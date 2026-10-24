"""E003 step 5: play a plan from the planner in the engine, with real clicks, and replan on what the engine shows.

usage: from replay import Harness, boot, verify
       ctl, branch = boot(cfg_of, prefix_events, frame, out_dir)     # the engine at the test's start, from boot
       h = Harness(builder, full_task, task, branch, goal, ...)      # task: the planner's (pruned) task
       log = h.run()                                                 # plays, matches, bans, replans; log is a dict
       verify(cfg_of, prefix_events + h.inputs(), h.checkpoints, out_dir2)   # the recorded inputs from boot

What the harness does, per plan step (plan.md step 5, with D5):

* **The input.** A step's record (reader.Record) becomes player input through the Executor (``Executor.play``, no
  fork, so no early inputs, no re-clicks and no delay retries): a sentence (verb, A, B) is the verb's key and clicks
  on A and B, a walk_to the walk click on A (the walk verb is the reader's own, read from the scripts), a dialog line
  a click on the line its verb shows, a skipped cutscene Esc, and an event no input at all: the harness waits.
  ``step_input`` and ``to_move`` do the translation.
* **The camera.** A click on an object that is off screen while the camera still scrolls waits (no input) until the
  object is on screen or the camera stops (``Harness._wait_camera``): the Executor's approach walks toward a point
  computed from the camera's position, which lies behind ego while the camera catches up with it.
* **The precondition.** Before its input, a step waits (no input) until its precondition holds in the engine's
  state, up to ``INPUT_LIMIT`` frames; the input then goes on the first frame the engine takes it (the Executor's
  click gate), not after the game settles (D5). A precondition that never holds is a mismatch of the plan's
  prediction, not of the step: replan, no ban. An event whose effects already hold at its turn (it ran during an
  earlier step) counts as done.
* **The window.** After the input (or from the step's start, for an event) the harness watches every frame: the
  step ends on the first frame all its predicted writes hold. Otherwise it runs on for ``INPUT_LIMIT`` frames after
  the input (``WAIT_LIMIT`` for an event). The first idle or menu stop in the window is logged. If none of the
  step's writes that change a value ever held (nothing happened; an Executor error counts the same), the input is
  played once more, after the click gate reopens.
* **The match** (``expectations``, ``match``): the record's predicted writes, each against the engine's value, and
  the tracked facts (the full task's facts, facts.py keys from the state dump, slot positions as ``pos:``) that
  changed during the step without a predicted write (``unplanned``: events the engine ran without being chosen,
  listed, not acted on). Every mismatch keeps the fact, the predicted values and the observed one. The locals of a
  slot (``args:``) and saved verbs (``saved:``) are not in the dump: they are not matched, and the harness keeps the
  plan's value for them.
* **The rules** (``classify``): a mismatch only on guessed writes is a replan (task.py's rule); a mismatch on a
  record that has a replay check on a position, a timer, a random draw, an actor's state or a message (the reader's
  reasons) is retried after ``RETRY_WAIT`` frames, up to ``RETRIES`` times, then banned; any other mismatch is a
  test on facts: the action is banned for the values its path tested (the record's precondition facts that are not
  the input's engine conditions) plus the room, and the harness replans. A ban (``Ban``, ``apply_bans``) cuts the
  banned point out of the precondition of every action of that label, so it holds wherever the planner meets those
  values; a ban needs the task written again (task.py), a replan without a new ban only a new start.
* **The replan**: ``reader.replan_start`` from the engine's facts and the plan's prediction (``args:`` copied where
  the slot is where the plan put it), then ``plan_fd.plan``.
* **The stop**: the goal (checked on every frame), no plan left (the planner's status), the replan limit, the
  time limit, or a start outside the task's domains (task.OutOfDomain: the task would have to be built again).
* **The record**: every input with its frame (the Executor's movie), every step with its frames, its outcome and
  its mismatches, the plans and the bans; the engine's facts at every step's end are kept for ``verify``, which
  plays the inputs from boot in a fresh engine (no fork, no change to the random state) and compares.

C1 and C2: the harness gives only player inputs (clicks on verbs, objects and lines, walking, waiting, Esc), and
names no game's number: the verbs, objects and lines come from the plan's records, the walk verb from the reader,
the screen points from the Executor (its disclosed interface profile) and the engine's hit test.
"""

import collections
import dataclasses
import json
import time
import types
from dataclasses import dataclass, field
from pathlib import Path

import facts as F
import plan_fd
import reader as R
import task as T
from speedrun.control import Branch, ControlError, Controller
from speedrun.executor import DIALOG_FIRST, DIALOG_LAST, Executor, Move, RouteError

INPUT_LIMIT = 600  # frames an input step may take (plan step 5)
WAIT_LIMIT = 3600  # frames an event (a wait) may take (plan step 5)
RETRIES = 3  # retries of a step whose path tested a position, a timer or a random draw (plan step 5)
RETRY_WAIT = 60  # frames waited before such a retry
PLAN_TIME = 600  # seconds per plan (plan step 4)
# the reader's reasons for a replay check (reader._why) that time or a position can change: retried, not banned
RETRY_REASONS = frozenset({"position", "timer", "random draw", "actor state", "message state"})
UNOBSERVED = ("args:", "saved:")  # facts the state dump does not hold (a slot's locals, the saved verbs)


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


def line_text(verb: int, state: dict) -> str | None:
    """The text a dialog line's verb shows now (the Executor clicks a line by its text), or None when it is not shown
    or another shown line holds the same text (the click would be ambiguous)."""
    lines = [v for v in state["verbs"] if DIALOG_FIRST <= v["id"] <= DIALOG_LAST and v.get("name")]
    mine = [v for v in lines if v["id"] == verb]
    if len(mine) != 1:
        return None
    text = mine[0]["name"]
    return text if sum(text.lower() in v["name"].lower() for v in lines) == 1 else None


def to_move(inp: Input, name: str, state: dict | None = None, limit: int = INPUT_LIMIT) -> Move | None:
    """The Executor's Move for an input (None for a wait). A line needs the state, to read the line's text."""
    if inp.kind == "click":
        return Move(name, verb=inp.verb, obj=inp.a, obj2=inp.b, max_frames=limit)
    if inp.kind == "key":
        return Move(name, key=inp.key, max_frames=limit)
    if inp.kind == "line":
        text = line_text(inp.verb, state or {"verbs": []})
        if text is None:
            raise RouteError(f"{name}: dialog line {inp.verb} is not shown, or its text is not unique")
        return Move(name, choose=text, max_frames=limit)
    return None


# --- observing --------------------------------------------------------------------------------------------------------

def view(b, doms: dict, obs: dict) -> dict:
    """The engine's facts (a facts.py map) as the task's values: every key of ``doms`` (key -> Domain) but the
    unobserved ones, slot positions as ``pos:``/``escpos:`` (reader.task_start), each value labelled by its domain
    (None: the domain does not hold it)."""
    st = R.task_start(b, obs, types.SimpleNamespace(facts=doms))
    out = {}
    for k, dom in doms.items():
        if k.startswith(UNOBSERVED) or dom is None:
            continue
        out[k] = dom.label(st.get(k, 0))
    return out


def holds(conds: dict, state: dict) -> list:
    """The keys of ``conds`` ({key: set of values}) that ``state`` does not meet (unobserved keys skipped)."""
    return [k for k, s in conds.items() if not k.startswith(UNOBSERVED) and state.get(k) not in s]


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
    kind: str  # "write" (a predicted write not seen), "guess" (a guessed write), "unplanned", "precondition"


def _txt(vals) -> tuple:
    return tuple(sorted(map(str, vals)))


def match(exps: dict, now: dict) -> list:
    """The predicted writes ``now`` does not show. A guessed write is a "guess" mismatch when the value is outside
    the guess or not the one the plan chose."""
    out = []
    for k, x in exps.items():
        v = now.get(k)
        if v not in x.values:
            out.append(Mismatch(k, _txt(x.values), v, "guess" if x.kind == "guess" else "write"))
        elif x.kind == "guess" and x.chosen is not None and v != x.chosen:
            out.append(Mismatch(k, _txt([x.chosen]), v, "guess"))
    return out


def unplanned(before: dict, after: dict, exps: dict) -> list:
    """Tracked facts that changed from ``before`` to ``after`` with no predicted write: events the engine ran
    without being chosen, or writes the record does not hold."""
    return [Mismatch(k, _txt([before.get(k)]), after.get(k), "unplanned")
            for k in sorted(set(before) | set(after)) if k not in exps and before.get(k) != after.get(k)]


def classify(rec, mismatches: list) -> str:
    """What a failed step leads to (plan step 5): "replan" when only guessed writes differ, "retry" when the record's
    path took a replay check on a position, a timer, a random draw, an actor's state or a message, else "ban"."""
    if not any(m.kind == "write" for m in mismatches):
        return "replan"
    if any(R._why(c[3]) in RETRY_REASONS for c in rec.checks):
        return "retry"
    return "ban"


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


def apply_bans(task: T.Task, bans: list) -> T.Task:
    """The task with every ban applied to every action of the ban's label (``cut``)."""
    by = collections.defaultdict(list)
    for ban in bans:
        by[ban.label].append(ban)
    actions = []
    for a in task.actions:
        pieces = [a]
        for ban in by.get(getattr(a.what, "label", None), ()):
            if all(k in task.facts for k, _ in ban.values):
                pieces = [p for q in pieces for p in cut(q, ban, task.facts)]
        actions += pieces
    out = T.Task(task.facts, actions, task.goal)
    out.folded = getattr(task, "folded", 0)
    return out


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


def verify(cfg_of, events: list, checkpoints: list, goal_keys, out_dir: Path) -> dict:
    """Play ``events`` from boot in a fresh engine (no fork, no change to the random state) and compare its facts
    with ``checkpoints`` ([(frame, facts.py map)], in frame order). Returns the counts and the first differences."""
    cfg = cfg_of(out_dir, write_events(out_dir / "movie.jsonl", events))
    out = {"checkpoints": len(checkpoints), "differ": [], "inputs": sum(1 for e in events if _is_input(e))}
    with Controller(cfg, timeout_s=600) as c:
        b = c.root
        for frame, facts in checkpoints:
            st = b.run(frame=frame)
            if st.get("frame") != frame:
                out["differ"].append({"frame": frame, "error": f"stopped at {st}"})
                break
            d = F.diff(facts, F.facts_of(b.state()))
            if d:
                out["differ"].append({"frame": frame, "facts": len(d), "first": sorted(d.items())[:8]})
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
    index: int  # the step's index in that plan
    name: str
    kind: str
    input: str
    start: int  # frame the step began
    issue: int | None = None  # frame of its (last) input
    end: int | None = None  # frame it ended
    outcome: str = ""  # matched, already happened, precondition, nothing happened, mismatch, goal
    then: str = ""  # next, replan, retry, ban, goal
    tries: int = 1  # times the input was played (a re-play after nothing happened, retries)
    first_settled: int | None = None  # the first idle or menu stop after the input
    mismatches: list = field(default_factory=list)  # Mismatch as dicts (the last try's)
    unplanned: list = field(default_factory=list)  # Mismatch as dicts
    inputs: list = field(default_factory=list)  # the input events it sent
    note: str = ""


class Harness:
    """Plays plans of ``task`` (the planner's task: actions whose ``what`` is a reader.Record) in the engine at
    ``branch`` until ``goal`` holds (module docstring). ``full_task``'s facts are the tracked facts and the domains
    the matcher reads; ``bans`` start the run with bans already in place."""

    def __init__(self, b, full_task: T.Task, task: T.Task, branch: Branch, goal: dict, *, release: str = "dos-cd",
                 replan_limit: int = 500, time_limit: float = 24 * 3600, plan_time: int = PLAN_TIME,
                 progress=None):
        self.b, self.base = b, task
        self.doms = dict(full_task.facts)
        self.doms.update(task.facts)
        self.goal = goal
        self.branch = branch
        self.ex = Executor(branch, release, fork=None)
        self.replan_limit, self.time_limit, self.plan_time = replan_limit, time_limit, plan_time
        self.progress = progress or (lambda msg: None)
        self.bans: list = []
        self.task: T.Task | None = None
        self.sas = None
        self.sas_bans = None  # the bans the current SAS file holds
        self.plans: list = []  # per plan: status, seconds, steps, start counts
        self.steps: list = []  # StepLog per step played
        self.checkpoints: list = []  # (frame, facts.py map) at each step's end, for ``verify``
        self.replans = 0
        self.stop_reason = None
        self.obs: dict = {}
        self.now: dict = {}
        self.kept: dict = {}  # the plan's values of the unobserved facts (args:, saved:)
        self.frame0 = self.tick0 = None

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

    # --- planning -------------------------------------------------------------------------------------------------

    def plan(self, first: bool) -> list:
        """A plan from the engine's state (reader.task_start for the first, reader.replan_start after); [] when the
        planner gives none (the stop reason is set)."""
        if self.sas is None or self.sas_bans != len(self.bans):
            self.progress(f"writing the task ({len(self.bans)} bans)")
            self.task = apply_bans(self.base, self.bans)
            t = time.monotonic()
            self.sas = T.write_sas(self.task)
            self.sas_bans = len(self.bans)
            write_s = time.monotonic() - t
        else:
            write_s = 0.0
        if first:
            start, counts = R.task_start(self.b, self.obs, self.task), {}
        else:
            start, counts = R.replan_start(self.b, self.obs, self.belief(), self.task)
        entry = {"plan": len(self.plans), "frame": self.branch.frame, "bans": len(self.bans), "write_s": write_s,
                 "start_counts": dict(counts)}
        self.plans.append(entry)
        try:
            self.sas.state(start)
        except T.OutOfDomain as e:
            entry.update(status="out of domain", misses=[(k, v) for k, v in e.misses[:20]], steps=[])
            self.stop_reason = f"the engine's state is outside the task's domains ({len(e.misses)} facts)"
            return []
        self.progress(f"planning (plan {entry['plan']}, frame {self.branch.frame})")
        res = plan_fd.plan(self.sas, start, time_limit=self.plan_time)
        entry.update(status=res.status, seconds=res.seconds, steps=[s.name for s in res.steps])
        self.progress(f"plan {entry['plan']}: {res.status}, {len(res.steps)} steps, {res.seconds:.0f} s")
        if res.status != "plan":
            self.stop_reason = f"no plan left (planner: {res.status})"
            return []
        # the plan's values of the unobserved facts at its start
        self.kept = {k: self.task.facts[k].label(start.get(k, 0)) for k in self.task.facts if k.startswith(UNOBSERVED)}
        return list(res.steps)

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

    def _watch(self, exps: dict, start: int, limit: int, log: StepLog) -> tuple:
        """Every frame from now until all ``exps`` hold or ``limit`` frames after ``start``. Returns (how, the state
        with the most writes held, whether a changed write ever held): how is "held", "goal" or "limit"."""
        end = start + limit
        best, ever = None, False
        while True:
            held = [k for k, x in exps.items() if self.now.get(k) in x.values]
            ever = ever or any(exps[k].changes for k in held)
            if best is None or len(held) > best[0]:
                best = (len(held), dict(self.now))
            if log.first_settled is None and self.branch.frame > start and self.settled():
                log.first_settled = self.branch.frame
            if len(held) == len(exps):
                return "held", dict(self.now), ever
            if self.goal_holds():
                return "goal", best[1], ever
            if self.branch.frame >= end:
                return "limit", best[1], ever
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
            log.note = (log.note + "; " if log.note else "") + f"waited {waited} frames for the camera"

    def _play_input(self, inp: Input, name: str, log: StepLog) -> bool:
        """The step's input through the Executor; False on an Executor error (logged)."""
        mark = len(self.ex.movie)
        try:
            if inp.kind == "click":
                self._wait_camera(inp, INPUT_LIMIT, log)
            move = to_move(inp, name, self.state)
            self.ex.play(move)
            ok = True
        except RouteError as e:
            log.note = (log.note + "; " if log.note else "") + f"executor: {e}"
            ok = False
        new = self.ex.movie[mark:]
        log.inputs += new
        ins = [e["frame"] for e in new if _is_input(e)]
        log.issue = max(ins) if ins else self.branch.frame
        self.observe()
        return ok

    def play_step(self, n_plan: int, i: int, step) -> str:
        """Play one plan step; returns "next", "replan" or "goal" (module docstring)."""
        a = self.task.actions[step.action]
        rec = a.what
        inp = step_input(rec)
        is_event = inp.kind == "wait"
        limit = WAIT_LIMIT if is_event else INPUT_LIMIT
        log = StepLog(n_plan, i, step.name, rec.kind, _input_text(inp), self.branch.frame)
        self.steps.append(log)
        retries, replayed = 0, False
        try:
            while True:
                # the precondition (or, for an event, its effects already there)
                bad = holds(a.pre, self.belief())
                if bad:
                    if is_event and self._already(rec, step):
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
                before = self.belief()
                seen0 = dict(self.now)  # the engine's values alone, for the unplanned changes
                self.prev_before = before
                exps = expectations(rec, before, self.doms, step.guessed)
                if is_event:
                    ok, start = True, self.branch.frame
                else:
                    ok = self._play_input(inp, step.name, log)
                    start = log.issue
                how, seen, ever = self._watch(exps, start, limit, log) if ok else ("error", dict(self.now), False)
                log.end = self.branch.frame
                log.unplanned = [dataclasses.asdict(m) for m in unplanned(seen0, self.now, exps)]
                if how == "goal":
                    log.outcome, log.then = "goal", "goal"
                    return "goal"
                if how == "held":
                    log.outcome = "matched"
                    guesses = match(exps, seen)
                    log.mismatches = [dataclasses.asdict(m) for m in guesses]
                    for k, e in rec.effects.items():  # the plan's values of the unobserved facts
                        if k.startswith(UNOBSERVED) and e[0] == "set":
                            self.kept[k] = e[1]
                    log.then = "replan" if guesses else "next"
                    return log.then
                mism = match(exps, seen)
                log.mismatches = [dataclasses.asdict(m) for m in mism]
                if not ever and not is_event and not replayed:
                    replayed = True
                    log.tries += 1
                    log.note = (log.note + "; " if log.note else "") + f"nothing happened by frame {self.branch.frame}"
                    continue  # once more, after the click gate reopens (the Executor waits for it)
                log.outcome = "nothing happened" if not ever else "mismatch"
                verdict = classify(rec, mism) if mism else "replan"
                if verdict == "retry" and retries < RETRIES:
                    retries += 1
                    log.tries += 1
                    log.note = (log.note + "; " if log.note else "") + f"retry {retries} after {RETRY_WAIT} frames"
                    self.advance(RETRY_WAIT)
                    continue
                if verdict == "retry":
                    verdict = "ban"
                if verdict == "ban":
                    ban = ban_of(rec, before)
                    self.bans.append(ban)
                    log.note = (log.note + "; " if log.note else "") + f"ban {ban.label} at {dict(ban.values)}"
                log.then = verdict if verdict == "ban" else "replan"
                return "replan"
        finally:
            if log.end is None:
                log.end = self.branch.frame
            if self.checkpoints[-1][0] != self.branch.frame:  # the engine's facts at the step's end, for verify
                self.checkpoints.append((self.branch.frame, dict(self.obs)))
            self.progress(f"  {log.outcome} -> {log.then} at frame {log.end} (tries {log.tries}); mismatches "
                          f"{[(m['key'], m['predicted'], m['observed']) for m in log.mismatches[:6]]}; "
                          f"{len(log.unplanned)} unplanned changes; {log.note}")

    def _already(self, rec, step) -> bool:
        """An event whose effects hold at its turn (it ran during the previous step): every write that changes a
        value, judged from the state at the previous step's start."""
        before = getattr(self, "prev_before", None) or self.belief()
        exps = expectations(rec, before, self.doms, step.guessed)
        changed = [x for x in exps.values() if x.changes]
        return bool(changed) and all(self.now.get(x.key) in x.values for x in changed)

    # --- the run --------------------------------------------------------------------------------------------------

    def run(self) -> dict:
        t0 = time.monotonic()
        self.observe()
        self.frame0, self.tick0 = self.branch.frame, self.branch.status.get("tick")
        self.checkpoints.append((self.branch.frame, dict(self.obs)))
        try:
            steps = self.plan(first=True)
            n_plan, i = 0, 0
            while True:
                if self.goal_holds():
                    self.stop_reason = "goal"
                    break
                if not steps and self.stop_reason:
                    break
                if time.monotonic() - t0 > self.time_limit:
                    self.stop_reason = "time limit"
                    break
                if i >= len(steps):
                    how = "replan"  # the plan is played out and the goal does not hold
                else:
                    self.progress(f"plan {n_plan} step {i + 1}/{len(steps)}: {steps[i].name} (frame "
                                  f"{self.branch.frame})")
                    how = self.play_step(n_plan, i, steps[i])
                if how == "goal":
                    self.stop_reason = "goal"
                    break
                if how == "next":
                    i += 1
                    continue
                if self.replans >= self.replan_limit:
                    self.stop_reason = "replan limit"
                    break
                self.replans += 1
                steps = self.plan(first=False)
                n_plan, i = len(self.plans) - 1, 0
        except (Stop, ControlError) as e:
            self.stop_reason = f"engine: {e}"
        self.observe()
        if not self.checkpoints or self.checkpoints[-1][0] != self.branch.frame:
            self.checkpoints.append((self.branch.frame, dict(self.obs)))
        st = self.branch.status
        return {
            "stop": self.stop_reason, "goal": self.goal_holds(), "replans": self.replans,
            "bans": [dataclasses.asdict(b) for b in self.bans], "plans": self.plans,
            "steps": [dataclasses.asdict(s) for s in self.steps],
            "frames": st.get("frame", 0) - self.frame0, "ticks": (st.get("tick") or 0) - (self.tick0 or 0),
            "frame0": self.frame0, "frame_end": st.get("frame"), "seconds": time.monotonic() - t0,
            "inputs": self.inputs(),
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
