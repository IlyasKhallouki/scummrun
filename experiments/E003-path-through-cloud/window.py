"""E003 plan step A4: the window search (window-search.md). The engine picks the frame: a macro step's input is tried on
every frame of its window, each on a forked child, and the child that completes first on the bridge's clock is kept.

usage: through the harness, ``replay.Harness(..., window_search=WindowConfig(...), fork=Controller.fork)``
(replay_part1.py ``--window-search``); the harness calls ``Search.play_plan`` on each plan.

What it does (window-search.md, with its lemma numbers):

* ``build_recs``, ``macro_steps``: the plan's steps with their read and write sets and predicted writes, deordered by
  interference (section 1, Lemma 1); each event goes to the macro step of the last input it follows, else of the first
  input it precedes, else the last one.
* ``Search.window``: from the parent at the window's start, one frame at a time, a child at every frame where the
  engine takes the input on the next frame (Lemma 5), up to ``workers`` at a time. Each child plays the input through
  the harness's own input code and the Executor, then runs a frame at a time until success, goal, mismatch, spent,
  pruned, limit or an Executor error (section 3; ``Matcher``, section 2). Forking stops once Q(g) + b reaches the best
  success (Lemmas 2-4, ``chain_frames``), and the window ends where a fact the macro step reads changes after the
  input's precondition held in the parent, or ``window_frames`` frames after its start (section 5).
* ``Search.play_plan``: the windows of one plan in order; the chosen child's snapshot, taken at its last input frame,
  is the next window's parent (section 7). It returns "goal", "replan" (with ``replan_cause``) or "stop".

C1 and C2: it gives only the harness's player inputs and names no game's number.
"""

import collections
import dataclasses
import json
import os
import re
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path

import facts as F
import reader as R
import replay as RP
from speedrun.control import ControlError
from speedrun.executor import PROFILES, Executor, RouteError

WINDOW_FRAMES = 3600  # section 5: a window ends this many frames after its start at the latest
CHILD_LIMIT = 3600  # section 3: a child stops this many frames after its last input
WORKERS = 7  # children at a time (plan step A4)
DELTA_CAP = 15  # costs.rs DELTA_CAP: dmax at the cap says nothing about var 19
MEM_FLOOR_KB = 3 * 1024 * 1024  # forks wait, then the run stops, while MemAvailable is below 3 GB
CHILD_ROWS = 4000  # children kept per window in the record (the counts cover all)
_EVENT = re.compile(r"^event\(([^@#()]+)@(-?\d+)")
_CLK = os.sysconf("SC_CLK_TCK")


@dataclass
class WindowConfig:
    """The switch ``window_search`` (window-search.md section 8): its settings."""
    workers: int = WORKERS
    delta_lo: int = 1  # Lemma 2's δ_lo, from the reading (``delta_lo_of``)
    window_frames: int = WINDOW_FRAMES
    child_limit: int = CHILD_LIMIT
    mem_floor_kb: int = MEM_FLOOR_KB


def delta_lo_of(frame_costs: dict | None) -> int:
    """Lemma 2: dmax where it is below DELTA_CAP (no unit of the scope writes var 19, which holds dmax), else 1."""
    d = (frame_costs or {}).get("dmax")
    return int(d) if isinstance(d, int) and 1 <= d < DELTA_CAP else 1


def mem_available_kb() -> int:
    try:
        with open("/proc/meminfo") as fh:
            for ln in fh:
                if ln.startswith("MemAvailable:"):
                    return int(ln.split()[1])
    except OSError:
        pass
    return 1 << 40


def _proc_kb(path: str, field_: str) -> int:
    try:
        with open(path) as fh:
            for ln in fh:
                if ln.startswith(field_):
                    return int(ln.split()[1])
    except OSError:
        pass
    return 0


def pss_kb(pid: int) -> int:
    return _proc_kb(f"/proc/{pid}/smaps_rollup", "Pss:")


def rss_kb(pid: int) -> int:
    return _proc_kb(f"/proc/{pid}/status", "VmRSS:")


def cpu_of(pid: int) -> float:
    """CPU seconds (user + system) of a live process, from /proc; 0 when it is gone."""
    try:
        with open(f"/proc/{pid}/stat") as fh:
            txt = fh.read()
        rest = txt[txt.rindex(")") + 2:].split()
        return (int(rest[11]) + int(rest[12])) / _CLK
    except (OSError, ValueError, IndexError):
        return 0.0


class FastView:
    """replay.view(b, doms, obs, unobserved) for fixed ``doms``, computed only on the keys of ``doms``: the same values
    (reader.task_start's positions for the ``pos:``/``escpos:`` keys, the ELSEWHERE rule, each value labelled by its
    domain), without task_start's pass over every unit of the scope (``test_window.py``,
    ``test_fast_view_is_view``). Its counters are not kept."""

    def __init__(self, b, doms: dict, unobserved: tuple = RP.UNOBSERVED):
        pts = b.table.points
        self.elsewhere = getattr(b, "elsewhere", None) or set()
        self.plain, self.pos = [], []
        for k, dom in doms.items():
            if k.startswith(unobserved) or dom is None:
                continue
            kind, _, name = k.partition(":")
            if kind in R.POS_KINDS:
                at = R.POS_KINDS[kind]
                ws = pts[at].get(name)
                cands = None if ws is None else [(f"{at}:{name}:{w}", w) for w in sorted(ws)]
                self.pos.append((k, dom, at, name, cands, set(ws or ())))
            else:
                self.plain.append((k, dom))

    def __call__(self, obs: dict) -> dict:
        out = {k: dom.label(obs.get(k, 0)) for k, dom in self.plain}
        seen = None
        for k, dom, at, name, cands, ws in self.pos:
            v = R.NOWHERE
            if cands is not None:
                v = next((w for key, w in cands if obs.get(key) == 1), R.NOWHERE)
            if v == R.NOWHERE and k in self.elsewhere:
                if seen is None:
                    seen = {}
                    for key, x in obs.items():
                        if x == 1 and key.startswith(("at:", "esc:")):
                            kd, rest = key.split(":", 1)
                            nm, w = rest.rsplit(":", 1)
                            seen.setdefault((kd, nm), []).append(w)
                if any(w.lstrip("-").isdigit() and int(w) not in ws for w in seen.get((at, name), ())):
                    v = RP.ELSEWHERE
            out[k] = dom.label(v)
        return out


# --- section 1: the plan's records and the macro steps -------------------------------------------------------------

def rec_reads_writes(rec) -> tuple:
    """The facts a record reads (its precondition, the facts it steps, its conditions and copy sources) and writes."""
    reads = set(rec.pre)
    for k, e in rec.effects.items():
        if e[0] == "add":
            reads.add(k)
        elif e[0] in ("when", "copy"):
            reads.add(e[1])
    return reads, set(rec.effects)


@dataclass
class MRec:
    """One plan step as the window search tracks it."""
    i: int  # its index in the plan's steps
    step: object  # plan_fd.Step
    paths: list  # the step's actions: one, or the paths the engine chooses between (Harness.choices)
    kind: str  # "input" or "event"
    reads: frozenset
    writes: frozenset
    unit: str | None = None  # an event's unit and wait point (its label)
    wait: int | None = None
    cost: int = 1  # its frame cost (costs.rs), the least over its paths
    exps: list = field(default_factory=list)  # per path: {key: Expect} from the plan's state before it
    pres: list = field(default_factory=list)  # per path: its precondition
    before: dict = field(default_factory=dict)  # the plan's state before it (for the replan's prediction)
    preds: frozenset = frozenset()  # the steps it follows (≺), by index
    macro: int = 0
    done: bool = False  # matched in the run
    done_frame: int | None = None
    path: int = 0  # the path that matched
    checked: frozenset | None = None  # the facts its match checks (section 2, Lemma 8; None: every final write)

    @property
    def name(self) -> str:
        return self.step.name


def interferes(a: MRec, b: MRec) -> bool:
    return bool(a.writes & b.reads or a.reads & b.writes or a.writes & b.writes)


def macro_steps(recs: list) -> list:
    """Section 1: ≺ over ``recs`` (plan order; sets each one's ``preds``) and the macro steps, as lists of indices into
    ``recs``, each with at most one input, the input first."""
    n = len(recs)
    preds = [set() for _ in range(n)]
    for j in range(n):
        for i in range(j):
            if interferes(recs[i], recs[j]):
                preds[j].add(i)
                preds[j] |= preds[i]
    for j, r in enumerate(recs):
        r.preds = frozenset(preds[j])
    inputs = [i for i, r in enumerate(recs) if r.kind == "input"]
    if not inputs:
        for r in recs:
            r.macro = 0
        return [list(range(n))] if n else []
    k_of = {i: k for k, i in enumerate(inputs)}
    for j, r in enumerate(recs):
        if r.kind == "input":
            r.macro = k_of[j]
            continue
        after = [k_of[i] for i in inputs if i in preds[j]]
        before = [k_of[i] for i in inputs if j in preds[i]]
        r.macro = max(after) if after else (min(before) if before else len(inputs) - 1)
    out = [[] for _ in inputs]
    for j, r in enumerate(recs):
        out[r.macro].append(j)
    for k, i in enumerate(inputs):  # the input first, the rest in plan order
        out[k] = [i] + [j for j in out[k] if j != i]
    return out


def checked_keys(recs: list, goal) -> None:
    """Section 2, the checked writes (Lemma 8): per record, the facts it writes that a later step reads before any later
    step writes them, the goal's facts no later step writes, and its own slot (``pos:U`` of an event's unit, an input's
    ``pos:`` writes). Sets each record's ``checked``."""
    for i, r in enumerate(recs):
        keep = set()
        for k in r.writes:
            for q in recs[i + 1:]:
                if k in q.reads:
                    keep.add(k)
                    break
                if k in q.writes:
                    break
            else:
                if k in goal:
                    keep.add(k)
        if r.unit is not None:
            keep.add(f"pos:{r.unit}")
        if r.kind == "input":
            keep |= {k for k in r.writes if k.startswith("pos:")}
        r.checked = frozenset(keep)


def build_recs(h, steps: list) -> list:
    """The plan's steps as MRecs, each path's predicted writes from the plan's state before it (window-search.md
    section 2): the harness's belief at the plan's start with the earlier steps' predicted writes applied."""
    state = h.belief()
    out = []
    for i, st in enumerate(steps):
        paths = list(h.choices.get(st.action) or [h.task.actions[st.action]])
        reads, writes = set(), set()
        for a in paths:
            r1, w1 = rec_reads_writes(a.what)
            r2, w2 = RP.reads_writes(a)
            reads |= r1 | r2
            writes |= w1 | w2
        rec = paths[0].what
        kind = "event" if rec.kind == "event" else "input"
        m = _EVENT.match(rec.label) if kind == "event" else None
        exps = [h._expectations(a.what, state, h.doms, st.guessed) for a in paths]
        out.append(MRec(i, st, paths, kind, frozenset(reads), frozenset(writes), m[1] if m else None,
                        int(m[2]) if m else None, min(a.cost for a in paths), exps, [dict(a.pre) for a in paths],
                        dict(state)))
        pick = next((n for n, a in enumerate(paths) if not h._holds(a.pre, state)), 0)
        state = RP.after(state, exps[pick])
    return out


# --- section 4: the bound b ---------------------------------------------------------------------------------------------

def input_clicks(rec) -> int:
    """Lemma 3's n: the input's clicks (1, or 2 with a second object); a key or a dialog line is 1."""
    if rec.kind in ("sentence", "walk_to") and len(rec.source) == 4 and rec.source[3]:
        return 2
    return 1


def chain_frames(recs: list, members: list) -> tuple:
    """Lemmas 4 and 4′: the largest sum of event costs along the links from the input's record within the macro step
    ``members`` (indices, the input first): a record that leaves unit U at wait point W (``pos:U`` set to W) links to
    the first later event of U resuming at W with one path; through U's own slot (or from the input) it adds the
    event's cost (at least 1), through a slot another event starts max(0, cost - 1). Returns (frames, the chain's step
    names)."""
    first = recs[members[0]]
    if first.kind != "input" or len(first.paths) != 1:
        return 0, []
    rest = [recs[j] for j in members[1:]]

    def leaves(x):
        for key, e in x.paths[0].what.effects.items():
            if key.startswith("pos:") and e[0] == "set" and isinstance(e[1], int) and e[1] >= 0:
                yield key[4:], e[1]

    def best_from(x, seen):
        top, top_names = 0, []
        for unit, w in leaves(x):
            nxt = next((r for r in rest if r.i > x.i and r.kind == "event" and r.unit == unit and r.wait == w), None)
            if nxt is None or len(nxt.paths) != 1 or nxt.i in seen:
                continue
            e2 = nxt.paths[0].what.effects.get(f"pos:{unit}")
            if e2 is not None and e2[0] == "set" and e2[1] == w:
                continue  # Lemma 4′: a yield at its own resume point shows nothing
            own = x.kind == "input" or x.unit == unit
            add = max(1, nxt.cost) if own else max(0, nxt.cost - 1)
            sub, sub_names = best_from(nxt, seen | {nxt.i})
            if add + sub > top:
                top, top_names = add + sub, [nxt.name] + sub_names
        return top, top_names

    return best_from(first, frozenset())


# --- section 2: the matcher ----------------------------------------------------------------------------------------------

class Matcher:
    """Sections 2 and 3 over the tracked records (MRecs in plan order): a record matches at the first frame where its
    final writes hold and the tracked records it follows have matched; ``needed`` are the ones success needs. The input's
    record matches only in the child that played it, once ``input_issued``."""

    def __init__(self, tracked: list, needed: set, input_rec: MRec | None, done=(), issued=()):
        self.tracked, self.needed, self.input = tracked, set(needed), input_rec
        self.issued = set(issued)  # inputs issued before the fork (earlier macro steps): their records may match
        self.matched: dict = {i: (None, 0) for i in done}  # i -> (frame, path); None: matched in the parent before
        idx = {r.i for r in tracked}
        self.final = {}
        for r in tracked:
            later = set().union(*[q.writes for q in tracked if q.i > r.i])
            self.final[r.i] = [{k: x for k, x in exps.items() if k not in later and
                                (r.checked is None or k in r.checked)} for exps in r.exps]
        self.preds = {r.i: r.preds & idx for r in tracked}
        self.input_issued = False
        self.choices: dict = {}  # i -> [(key, observed)]: a guessed write inside its values, not the plan's choice
        self.plan_pending: list = []  # every record of the plan not done (the mismatch rule's predicted positions)

    def note(self, frame: int, now: dict) -> None:
        for r in self.tracked:
            if r.i in self.matched:
                continue
            if r.kind == "input" and r.i not in self.issued and not (self.input_issued and r is self.input):
                continue
            if any(p not in self.matched for p in self.preds[r.i]):
                continue
            for n, fin in enumerate(self.final[r.i]):
                if all(now.get(k) in x.values for k, x in fin.items()):
                    self.matched[r.i] = (frame, n)
                    ch = [(mm.key, mm.observed) for mm in RP.match(fin, now) if mm.kind == "choice"]
                    if ch:
                        self.choices[r.i] = ch
                    break

    def complete(self) -> bool:
        return all(i in self.matched for i in self.needed)

    def pending(self) -> list:
        return [r for r in self.tracked if r.i not in self.matched]

    def mismatch(self, now: dict, at_fork: dict) -> tuple | None:
        """Section 3's mismatch: a slot of a unit with a pending record at a wait point it did not hold at the fork and
        that no pending record starts at or predicts, or a room no pending record predicts. (key, value) or None."""
        allowed = collections.defaultdict(set)
        pend_i = {r.i for r in self.pending()}
        for r in self.pending() + [q for q in self.plan_pending if q.i not in pend_i]:
            for exps, pre in zip(r.exps, r.pres):
                for k, x in exps.items():
                    if k.startswith("pos:") or k == "room":
                        allowed[k] |= set(x.values)
                for k, vs in pre.items():
                    if k.startswith("pos:"):
                        allowed[k] |= set(vs)
        # the slots a pending tracked record reads, and the pending events' own slots (section 3, Lemma 8); the room
        checked = {k for r in self.pending() for pre in r.pres for k in pre if k.startswith("pos:")} | \
            {f"pos:{r.unit}" for r in self.pending() if r.unit} | \
            {"room" for r in self.pending() for exps, pre in zip(r.exps, r.pres) if "room" in exps or "room" in pre}
        for k in sorted(checked):
            v = now.get(k)
            if v is None or v == at_fork.get(k) or v in allowed[k]:
                continue
            return k, v
        return None

    def unmet(self, now: dict, limit: int = 4) -> list:
        """The needed records still pending, each with its checked final writes that do not hold now (key, the
        predicted values, the engine's): what B1 reads of a child that did not succeed."""
        out = []
        for r in self.pending():
            if r.i not in self.needed:
                continue
            miss = [(k, sorted(map(str, x.values)), now.get(k)) for k, x in self.final[r.i][0].items()
                    if now.get(k) not in x.values][:limit]
            out.append((r.name, miss))
        return out[:limit]

    def dead_needed(self, now: dict) -> list:
        """Section 3's spent test at an idle stop: the needed records still pending that nothing alive can run, the
        input's own record or an event whose unit has no live slot (``pos:U`` at NOWHERE)."""
        return [r.name for r in self.pending() if r.i in self.needed and
                (r.kind == "input" or (r.unit is not None and now.get(f"pos:{r.unit}") == R.NOWHERE))]


def is_deviation(m: Matcher, input_rec, before_input: set, alt=None) -> bool:
    """Section 3, deviation: every needed record preceding the input has matched, and the input took effect (its
    record matched, or ``alt``, another of its paths with a change a later step reads, showed)."""
    return input_rec is not None and (input_rec.i in m.matched or alt is not None) and \
        all(i in m.matched for i in before_input)


def end_key(ch) -> tuple:
    """Section 5, Stop: the earliest end point, a success (or the goal) before a deviation at equal Q, then the earliest
    fork."""
    return ch.q_end, ch.outcome == "deviation", ch.g


_SOURCE = re.compile(rb'^\{"i": (\d+), "rec": -?\d+, "label": "(?:[^"\\\\]|\\\\.)*", "source": (\[[^\]]*\])')


def source_index(lines_path) -> dict:
    """The reading's actions by their input (the action lines' ``source``: kind, verb, objects), from one pass over the
    lines file."""
    out = collections.defaultdict(list)
    with open(lines_path, "rb") as fh:
        for ln in fh:
            m = _SOURCE.match(ln)
            if m is not None:
                out[tuple(json.loads(m[2]))].append(int(m[1]))
    return out


def later_reads(recs: list, i: int, keys) -> set:
    """Of ``keys``, those a step after recs[i] reads before any later step writes them (Lemma 8's links)."""
    out = set()
    for k in keys:
        for q in recs[i + 1:]:
            if k in q.reads:
                out.add(k)
                break
            if k in q.writes:
                break
    return out


def path_shows(pos_exps: list, now: dict) -> bool:
    """Lemma 9: on some path, every slot the record leaves at a wait point stands there."""
    return any(all(now.get(k) in x.values for k, x in pe.items()) for pe in pos_exps)


def other_path(alts: list, now: dict, at_fork: dict):
    """Section 3, another path: the first other record of the click that shows (its ``pos:`` writes hold and one of
    its writes differs from the fork's value) and changes a fact a later step reads; None if none does."""
    for name, pos, linked in alts:
        if not all(now.get(k) in x.values for k, x in pos.items()):
            continue
        shown = any(now.get(k) != at_fork.get(k) for k in list(pos) + list(linked))
        change = [k for k, x in linked.items() if now.get(k) in x.values and now.get(k) != at_fork.get(k)]
        if shown and change:
            return name
    return None


class EndRule:
    """Section 5, End (Lemma 7): the model's window for the input opens at the first frame where the parent is ready
    (every tracked record preceding the input has matched in it) and the input's precondition holds; the window ends
    at the first later frame where the precondition holds on no path."""

    def __init__(self):
        self.open = False

    def closed(self, ready: bool, pre_now: bool) -> bool:
        if self.open:
            return not pre_now
        self.open = ready and pre_now
        return False


# --- one child ---------------------------------------------------------------------------------------------------------

class _Log:
    def __init__(self):
        self.note = ""

    def add(self, note: str) -> None:
        self.note = (self.note + "; " if self.note else "") + note


class _Ctx:
    """What the harness's input code reads (Harness._wait_camera, _check_ego, _click_verb), for one child."""

    def __init__(self, branch, release: str, stone_fork):
        self.branch = branch
        self.watched = RP._Watched(branch)
        self.ex = Executor(self.watched, release, fork=None)
        self.ex.stone_fork = stone_fork
        self.state = branch.state()

    def observe(self) -> None:
        self.state = self.branch.state()

    def advance(self, n: int = 1) -> None:
        st = self.branch.run(frame=self.branch.frame + n)
        if "end" in st:
            raise RP.Stop(f"engine ended: {st['end']}")
        self.observe()


@dataclass
class Child:
    g: int  # the fork frame
    q0: int  # Q at the fork
    early: bool  # the input's precondition did not hold in the parent at g
    done: frozenset  # the tracked records the parent had matched by g
    at_fork: dict  # the parent's view at g (the window's facts)
    full_at_fork: dict  # the parent's view at g (every tracked fact)
    outcome: str = ""
    first: int | None = None  # the first input event's frame
    issue: int | None = None  # the last input event's frame
    end: int | None = None
    q_end: int | None = None
    frames: int = 0  # frames run after the fork
    why: object = None
    events: list = field(default_factory=list)
    matched: dict = field(default_factory=dict)
    snapshot: object = None  # the branch at ``issue`` (kept while this child is the best)
    live: object = None  # the branch at ``end`` (kept while this child is the best)
    checkpoints: list = field(default_factory=list)
    arrival: int | None = None  # the first frame after the last input where ego stopped walking
    unplanned: list = field(default_factory=list)
    choices: dict = field(default_factory=dict)  # Matcher.choices
    alt: str | None = None  # another path of the click that showed (section 3), the record's name

    def row(self) -> list:
        return [self.g, self.first, self.issue, self.outcome, self.end, self.q_end, self.frames,
                str(self.why)[:80] if self.why is not None else None]


# --- the search --------------------------------------------------------------------------------------------------------

class Search:
    """The window search for one harness (module docstring)."""

    def __init__(self, h, fork, cfg: WindowConfig):
        self.h, self.cfg = h, cfg
        self._fork, self._lock = fork, threading.Lock()
        self.release = h.ex.release
        self.verbless = PROFILES[self.release].verbless_rooms
        self.stones = getattr(h.ex, "stone_fork", None) is not None
        self.cpu = collections.Counter()  # engine CPU seconds of the forked processes, by role, noted at their quit
        self.forks = collections.Counter()
        self.windows: list = []
        self.py_cpu = 0.0
        self.pending_cp = None  # (a success frame's checkpoint, the movie's length then): valid if no later input
        # lands on or before its frame (``_flush``)
        self.replan_cause = None
        self.goal_writers: set = set()  # the plan's records that write a goal value (``play_plan``)
        self.goal_credited = False  # the goal came from the plan's records (the run's success)
        self.unplanned_goals: list = []  # children and parents where the goal held with no goal record matched
        self.live: set = set()  # the pids of the forked processes alive
        self.mem: dict = {}  # sample_memory's peaks
        self.deviations: list = []  # the windows that ended in a deviation (section 3), each a replan
        self._by_source = None  # the reading's actions by their input (``source_index``), made at the first click

    # --- processes ---

    def fork(self, b, role: str = "child"):
        """Controller.fork under one lock (the controller accepts the new connection on one socket); the branch's quit
        notes its CPU first."""
        with self._lock:
            c = self._fork(b)
            self.live.add(c.pid)
            n = sum(self.forks.values())
        self.forks[role] += 1
        if n % 25 == 0:
            self.sample_memory()
        quit_, pid, cpu, live = c.quit, c.pid, self.cpu, self.live

        def quit_noting():
            cpu[role] += cpu_of(pid)
            live.discard(pid)
            quit_()
        c.quit = quit_noting
        return c

    def sample_memory(self) -> None:
        """The search's memory now: the forked processes' Pss (each one's own pages, its share of the pages it shares
        with the parent), this process's RSS, and MemAvailable; the peaks are kept (``summary``)."""
        pss = sum(pss_kb(p) for p in list(self.live))
        own = rss_kb(os.getpid())
        m = self.mem
        m["forks_pss_kb"] = max(m.get("forks_pss_kb", 0), pss)
        m["python_rss_kb"] = max(m.get("python_rss_kb", 0), own)
        m["live_forks"] = max(m.get("live_forks", 0), len(self.live))
        m["min_available_kb"] = min(m.get("min_available_kb", 1 << 40), mem_available_kb())

    def _alternatives(self, recs: list, first: MRec) -> list:
        """The click's other records in the reading (section 3): (name, their ``pos:`` writes, their writes a later step
        reads), predicted from the plan's state before the click."""
        h = self.h
        lines = getattr(getattr(h.base, "lines", None), "path", None)
        if lines is None:
            return []
        if self._by_source is None:
            self._by_source = source_index(lines)
        own = {p.name for p in first.paths}
        out = []
        for i in self._by_source.get(tuple(first.paths[0].what.source), ()):
            a = h.base.actions[i]
            if a.name in own:
                continue
            e = h._expectations(a.what, first.before, h.doms, ())
            e = {k: x for k, x in e.items() if not k.startswith(h.unobserved) and k in h.doms}
            linked = later_reads(recs, first.i, [k for k in e if not k.startswith("pos:")])
            out.append((a.name, {k: x for k, x in e.items() if k.startswith("pos:")},
                        {k: e[k] for k in linked}))
        return out

    def stone_fork(self, b):
        return self.fork(b, "stone trial")

    def note_cpu(self, b, role: str) -> None:
        """A branch that stays alive past the run's end (the harness's last engine): its CPU so far."""
        if b is not None and b is not self.h.root:
            self.cpu[role] += cpu_of(b.pid)

    def _observe(self, b, fv: FastView) -> tuple:
        s = b.state()
        obs = F.facts_of(s)
        return s, obs, fv(obs)

    def _play(self, ctx: _Ctx, inp: RP.Input, name: str) -> None:
        """The harness's input code (Harness._play_input) on the child: the camera wait and the ego check for a click,
        the verb click for a dialog line, else the Executor's play."""
        log = _Log()
        if inp.kind == "click":
            RP.Harness._wait_camera(ctx, inp, RP.INPUT_LIMIT, log)
            RP.Harness._check_ego(ctx, inp)
        if inp.kind == "line":
            RP.Harness._click_verb(ctx, inp.verb, name)
        else:
            ctx.ex.play(RP.to_move(inp, name))

    def _child(self, w: dict, cb, ch: Child) -> Child:
        """Section 3 for one child forked at ``ch.g``."""
        h, cfg = self.h, self.cfg
        snap = None
        try:
            ctx = _Ctx(cb, self.release, self.stone_fork if self.stones else None)
            try:
                self._play(ctx, w["inp"], w["name"])
            except RouteError as e:
                ch.outcome, ch.why = "interface", str(e)
                return ch
            ins = [e for e in ctx.ex.movie if RP._is_input(e)]
            ch.events = list(ctx.ex.movie)
            if not ins:
                ch.outcome, ch.why = "interface", "no input event"
                return ch
            ch.first, ch.issue = min(e["frame"] for e in ins), max(e["frame"] for e in ins)
            with self._lock:
                if ch.first in w["firsts"]:
                    ch.outcome = "duplicate"  # Lemma 5: the run of the child whose first event lands there too
                    return ch
                w["firsts"].add(ch.first)
            if cb.frame != ch.issue:
                st = cb.run(frame=ch.issue)
                if "end" in st:
                    ch.outcome, ch.why = "ended", st["end"]
                    return ch
            snap = self.fork(cb, "snapshot")
            s, obs, now = self._observe(cb, w["fv"])
            m = Matcher(w["tracked"], w["needed"], w["input"], ch.done, issued=w["issued"])
            m.input_issued = True
            m.plan_pending = w["plan_pending"]
            cps = [(cb.frame, dict(obs), RP.extra_of(s, cb.status))]
            moved = False
            while True:
                st = cb.status
                f = cb.frame
                ch.frames = f - ch.g
                if cfg.delta_lo > 1 and max(1, st.get("v19", 0)) < cfg.delta_lo:
                    ch.outcome, ch.why = "delta", st.get("v19")
                    w["delta_violation"] = [f, st.get("v19")]
                    break
                with self._lock:
                    best = w["best"]
                if best is not None and st["qf"] >= best.q_end:
                    ch.outcome = "pruned"
                    break
                m.note(f, now)
                if f == ch.issue and w["click_paths"] is not None and not path_shows(w["click_paths"], now):
                    # Lemma 9: the record's slots do not stand where it leaves them: another path of the click ran
                    ch.alt = other_path(w["alts"], now, ch.full_at_fork)
                    if ch.alt is None:
                        ch.outcome, ch.why = "spent", "another path of the click, no change a later step reads"
                        w["lemma9_stops"] += 1
                        break
                if st.get("mv"):
                    moved = True
                elif moved and ch.arrival is None:
                    ch.arrival = f
                goal = not h._holds(h.goal, now)
                if goal and not (w["goal_writers"] and all(i in m.matched for i in w["goal_writers"])):
                    # the goal holds, but no record of the plan that writes it has matched: not from the plan
                    ch.outcome, ch.why = "goal, not from the plan", sorted(w["goal_writers"])
                    break
                if (goal or m.complete()) and ch.alt is None:
                    ch.outcome = "goal" if goal else "success"
                    break
                mm = m.mismatch(now, ch.at_fork)
                if mm is not None:
                    ch.outcome, ch.why = "mismatch", mm
                    break
                if f > ch.issue + 1 and "idle" in st:
                    dead = m.dead_needed(now)
                    if dead:
                        ch.outcome, ch.why = "spent", m.unmet(now)
                        break
                if f - ch.issue >= cfg.child_limit:
                    ch.outcome, ch.why = "limit", m.unmet(now)
                    break
                st = cb.run(frame=f + 1)
                if "end" in st:
                    ch.outcome, ch.why = "ended", st["end"]
                    break
                s, obs, now = self._observe(cb, w["fv"])
            ch.end, ch.q_end = cb.frame, cb.status.get("qf")
            ch.matched = {i: v for i, v in m.matched.items() if v[0] is not None}
            ch.choices = dict(m.choices)
            if ch.outcome in ("spent", "mismatch", "limit") and is_deviation(m, w["input"], w["before_input"], ch.alt):
                ch.outcome = "deviation"  # section 3: the input took effect, then the plan's prediction ended
            if ch.outcome in ("success", "goal", "deviation"):
                cps.append((cb.frame, dict(obs), RP.extra_of(s, cb.status)))
                ch.checkpoints = cps
                full = w["full_fv"](obs)
                ch.unplanned = [{"key": x.key, "start": RP._str(ch.full_at_fork.get(x.key)), "changes": [],
                                 "count": 1, "end": RP._str(x.observed), "net": True}
                                for x in RP.unplanned(ch.full_at_fork, full, w["predicted_keys"])][:40]
                floor = w["b_qf"] if ch.outcome != "deviation" else w["b_dev_qf"]
                if ch.q_end < ch.q0 + floor:
                    with self._lock:
                        w["bound_violations"].append([ch.g, ch.q0, ch.q_end, floor, ch.outcome])
                old = None
                with self._lock:
                    best = w["best"]
                    if best is None or end_key(ch) < end_key(best):
                        w["best"], old = ch, best
                        ch.snapshot, ch.live, snap, cb = snap, cb, None, None
                for b_ in ((old.snapshot, old.live) if old is not None else ()):
                    if b_ is not None:
                        b_.quit()
                if old is not None:
                    old.snapshot = old.live = None
            return ch
        except (ControlError, OSError, RP.Stop) as e:
            ch.outcome, ch.why = "error", f"{type(e).__name__}: {e}"
            return ch
        finally:
            for b_ in (snap, cb):
                if b_ is not None:
                    try:
                        b_.quit()
                    except OSError:
                        pass

    # --- one window ---

    def _gate(self, inp: RP.Input, st: dict, s: dict, parent) -> bool:
        """Section 5's fork test: the engine takes the input on the next frame (Lemma 5)."""
        if inp.kind == "key":
            return True
        if st.get("up", 0) <= 0:
            return False
        if inp.kind == "line":
            return RP.verb_point(inp.verb, s, parent.at) is not None
        return st.get("cs", 1) == 0 and st.get("room", 0) != 0 and (st["room"] in self.verbless or bool(s["verbs"]))

    def window(self, recs: list, members: list, tracked: list, needed: set, parent, plan_no: int, k: int) -> dict:
        """Section 5 for the macro step ``members`` (indices into ``recs``, the input first), from the parent's frame.
        Returns {"rec": the window's record, "best": a Child or None, "parent_matched": {i: (frame, path)}}; the parent
        is left at the window's end."""
        h, cfg = self.h, self.cfg
        t0, cpu0 = time.monotonic(), time.process_time()
        first = recs[members[0]]
        has_input = first.kind == "input"
        inp = RP.step_input(first.paths[0].what) if has_input else None
        reads = set().union(*[recs[j].reads for j in members])
        keys = set().union(*[r.writes for r in tracked]) | reads | set(h.goal) | {"room"} | \
            {f"pos:{r.unit}" for r in tracked if r.unit}
        doms = {kk: h.doms[kk] for kk in keys if kk in h.doms and not kk.startswith(h.unobserved)}
        full_doms = {kk: d for kk, d in h.doms.items() if not kk.startswith(h.unobserved)}
        doms_all = full_doms
        observed_reads = sorted(kk for kk in reads if kk in doms)
        # section 5, End: the tracked records that precede the input (P) and the facts the input reads
        before_input = {r.i for r in tracked if has_input and r.i in first.preds}
        input_reads = sorted(kk for kk in (first.reads if has_input else ()) if kk in doms)
        # the inputs of earlier macro steps were issued before this window's start: their records may match
        issued = {r.i for r in tracked if r.kind == "input" and r.macro < k}
        n = input_clicks(first.paths[0].what) if has_input else 0
        chain, chain_names = chain_frames(recs, members) if has_input else (0, [])
        b_frames = n + chain
        b_qf = 4 * cfg.delta_lo * b_frames
        pm = Matcher(tracked, needed, None, issued=issued)
        pm.plan_pending = [r for r in recs if not r.done]
        goal_writers = {r.i for r in tracked if r.i in self.goal_writers}
        click_paths, alts = None, []
        if has_input and inp.kind == "click":  # Lemma 9 and the click's other paths (section 3)
            click_paths = [{kk: x for kk, x in e.items() if kk.startswith("pos:") and kk in doms_all}
                           for e in first.exps]
            alts = self._alternatives(recs, first)
        w = {"goal_writers": goal_writers, "plan_pending": pm.plan_pending, "inp": inp, "name": first.name, "tracked": tracked, "needed": needed, "input": first if has_input else None,
             "doms": doms, "full_doms": full_doms, "fv": FastView(h.b, doms, h.unobserved),
             "full_fv": FastView(h.b, full_doms, h.unobserved), "best": None, "firsts": set(), "b_qf": b_qf,
             "bound_violations": [], "delta_violation": None, "issued": issued, "before_input": before_input,
             "click_paths": click_paths, "alts": alts, "lemma9_stops": 0,
             "b_dev_qf": 4 * cfg.delta_lo * n,
             "predicted_keys": set().union(*[set(e) for r in tracked for e in r.exps])}
        rec = {"plan": plan_no, "macro": k, "input": first.name if has_input else None,
               "records": [recs[j].name for j in members], "needed": [r.name for r in tracked if r.i in needed],
               "tracked": [r.name for r in tracked], "start": parent.frame, "start_qf": parent.status.get("qf"),
               "b_frames": b_frames, "b_qf": b_qf, "n_clicks": n, "chain": chain_names, "delta_lo": cfg.delta_lo,
               "observed_reads": len(observed_reads), "before_input": len(before_input), "open": None}
        pool = ThreadPoolExecutor(max_workers=cfg.workers)
        futures, children = [], []
        end_rule, prev, end_reason, gate_shut = EndRule(), {}, None, 0
        w0 = parent.frame
        try:
            while True:
                st = parent.status
                g = parent.frame
                s, obs, now = self._observe(parent, w["fv"])
                if cfg.delta_lo > 1 and max(1, st.get("v19", 0)) < cfg.delta_lo:
                    w["delta_violation"] = [g, st.get("v19")]
                    end_reason = "delta"
                    break
                pm.note(g, now)
                if not h._holds(h.goal, now):
                    credited = bool(goal_writers) and all(i in pm.matched for i in goal_writers)
                    end_reason = "goal in the parent" + ("" if credited else ", not from the plan")
                    break
                if not has_input:
                    if pm.complete():
                        end_reason = "matched in the parent"
                        break
                else:
                    belief = dict(now, **h.kept)
                    pre_now = any(not h._holds(a.pre, belief) for a in first.paths)
                    ready = all(i in pm.matched for i in before_input)
                    if end_rule.closed(ready, pre_now):
                        # section 5, End (Lemma 7): the model's window for the input closed in the ready parent
                        end_reason = "the input's precondition closed in the parent after its window opened"
                        rec["end_change"] = [[kk, prev.get(kk), now.get(kk)] for kk in input_reads
                                             if prev.get(kk) != now.get(kk)][:8]
                        break
                    if rec["open"] is None and end_rule.open:
                        rec["open"] = g
                    prev = now
                    with self._lock:
                        best = w["best"]
                    if best is not None and st["qf"] + (b_qf if best.outcome != "deviation" else w["b_dev_qf"]) \
                            >= best.q_end:
                        end_reason = "stop: Q(g) + b reached the best " + \
                            ("deviation" if best.outcome == "deviation" else "success")
                        break
                if g - w0 >= cfg.window_frames:
                    end_reason = f"{cfg.window_frames} frames"
                    break
                if has_input:
                    if self._gate(inp, st, s, parent):
                        while sum(1 for x in futures if not x.done()) >= cfg.workers:
                            time.sleep(0.002)
                        while mem_available_kb() < cfg.mem_floor_kb:
                            if not any(not x.done() for x in futures):
                                raise RP.Stop(f"memory: MemAvailable {mem_available_kb() // 1024} MB, under "
                                              f"{cfg.mem_floor_kb // 1024} MB")
                            time.sleep(0.05)
                        cb = self.fork(parent, "child")
                        ch = Child(g, st["qf"], not pre_now, frozenset(i for i, v in pm.matched.items()),
                                   dict(now), w["full_fv"](obs))
                        children.append(ch)
                        futures.append(pool.submit(self._child, w, cb, ch))
                        if len(children) % 100 == 0:
                            done_ = collections.Counter(c.outcome for c in children if c.outcome)
                            mk = collections.Counter(str(c.why)[:160] for c in children
                                                     if c.outcome in ("mismatch", "spent", "limit"))
                            h.progress(f"    window {k + 1}: {len(children)} children by frame {g}, {dict(done_)}"
                                       + (f"; mismatches {mk.most_common(3)}" if mk else ""))
                    else:
                        gate_shut += 1
                st = parent.run(frame=g + 1)
                if "end" in st:
                    end_reason = f"engine ended: {st['end']}"
                    break
        finally:
            for x in futures:
                x.result()
            pool.shutdown(wait=True)
        best = w["best"]
        counts = collections.Counter(c.outcome for c in children)
        rec.update(end=parent.frame, end_qf=parent.status.get("qf"), end_reason=end_reason, forks=len(children),
                   gate_shut=gate_shut, outcomes=dict(sorted(counts.items())),
                   child_frames=sum(c.frames for c in children),
                   early_successes=sum(1 for c in children if c.early and c.outcome in ("success", "goal")),
                   lemma9_stops=w["lemma9_stops"], other_paths=collections.Counter(c.alt for c in children if c.alt),
                   alternatives=len(alts),
                   bound_violations=w["bound_violations"], delta_violation=w["delta_violation"],
                   mismatch_keys=collections.Counter(str(c.why[0]) for c in children if c.outcome == "mismatch"
                                                     and isinstance(c.why, tuple)).most_common(8),
                   children=[c.row() for c in children[:CHILD_ROWS]],
                   wall_s=round(time.monotonic() - t0, 2), python_cpu_s=round(time.process_time() - cpu0, 2))
        by = {r.i: r.name for r in recs}
        if best is not None:
            rec["best"] = {"g": best.g, "first": best.first, "issue": best.issue, "end": best.end, "q0": best.q0,
                           "q_end": best.q_end, "outcome": best.outcome, "early": best.early,
                           "frames_issue_to_end": best.end - best.issue, "arrival": best.arrival,
                           "walk_frames": (best.arrival - best.issue) if best.arrival else None,
                           "matched": {by.get(i, str(i)): f for i, (f, _) in best.matched.items()},
                           "unplanned": len(best.unplanned)}
        rec["parent_matched"] = {by.get(i, str(i)): f for i, (f, _) in pm.matched.items() if f is not None}
        self.py_cpu += time.process_time() - cpu0
        self.windows.append(rec)
        if h.workdir is not None:  # a long run keeps its windows as it goes
            with open(Path(h.workdir).parent / "windows.jsonl", "a") as fh:
                fh.write(json.dumps(rec, default=str) + "\n")
        return {"rec": rec, "best": best, "parent_matched": {i: v for i, v in pm.matched.items() if v[0] is not None}}

    # --- one plan ---

    def _set_branch(self, b) -> None:
        h = self.h
        old = h.branch
        h.branch = b
        h.watched = RP._Watched(b)
        if old is not None and old is not b and old is not h.root:
            old.quit()
        h.observe()

    def _mark(self, recs: list, matched: dict, upto: int | None) -> None:
        """The records matched at or before frame ``upto`` (None: all) are done (section 7)."""
        for i, (f, path) in matched.items():
            r = recs[i]
            if r.done or (upto is not None and f is not None and f > upto):
                continue
            r.done, r.done_frame, r.path = True, f, path

    def _predict(self, recs: list) -> None:
        """The harness's prediction (slot positions and unobserved facts, for the replan) after the done records, in
        plan order."""
        h = self.h
        pred = dict(self.pred0)
        for r in recs:
            if r.done:
                pred = h._predict(pred, r.paths[r.path].what, r.before)
        h._set_pred(pred)

    def _logs(self, n_plan: int, recs: list, members: list, res: dict, then: str) -> None:
        """StepLogs for the harness's record (replay_part1.summary): the macro step's records, the input's try with
        the chosen child's inputs."""
        h, rec, best = self.h, res["rec"], res["best"]
        for j in members:
            r = recs[j]
            a = r.paths[r.path].what
            log = RP.StepLog(n_plan, r.i, r.name, a.kind, RP._input_text(RP.step_input(a)), rec["start"])
            if r.kind == "input":
                if best is not None:
                    log.tries = [{"try": 1, "kind": "window", "start": best.g, "issue": best.issue, "end": best.end,
                                  "how": best.outcome, "inputs": [e for e in best.events if RP._is_input(e)]}]
                    log.inputs = list(best.events)
                    log.issue, log.end = best.issue, best.end
                    log.unplanned = list(best.unplanned)
                log.outcome = ("goal" if best is not None and best.outcome == "goal" else
                               "matched" if r.done or (best is not None and r.i in best.matched) else "window empty")
            else:
                f = r.done_frame if r.done else (best.matched.get(r.i, (None,))[0] if best is not None else None)
                log.start = log.end = f if f is not None else rec["end"]
                log.tries = [{"try": 1, "kind": "window", "start": log.start, "end": log.end}] if f is not None else []
                log.outcome = "matched" if f is not None else "not reached"
            log.then = then
            log.note = f"window {rec['macro']}: {rec['end_reason']}; outcomes {rec['outcomes']}"
            log.seconds = rec["wall_s"] if r.kind == "input" else 0.0
            h.steps.append(log)

    def _goal_writers(self, recs: list) -> set:
        """The plan's records with a predicted write of a goal value that changes the fact (the run's success needs
        one of them matched where the goal holds)."""
        g = self.h.goal
        return {r.i for r in recs if any(k in g and set(x.values) & set(g[k]) and x.changes
                                         for exps in r.exps for k, x in exps.items())}

    def flush(self) -> None:
        """The pending success-frame checkpoint, if no input sent since lands on or before its frame (section 7)."""
        if self.pending_cp is None:
            return
        cp, mark = self.pending_cp
        self.pending_cp = None
        if all(e["frame"] > cp[0] for e in self.h.ex.movie[mark:] if RP._is_input(e)):
            self.h.checkpoints.append(cp)

    def play_plan(self, n_plan: int, steps: list) -> str:
        """The windows of one plan (module docstring). Returns "goal" (from the plan's records), "replan"
        (``replan_cause`` says why) or "stop" (the harness's stop reason set)."""
        h = self.h
        self.replan_cause = None
        recs = build_recs(h, steps)
        macros = macro_steps(recs)
        checked_keys(recs, h.goal)
        self.goal_writers = self._goal_writers(recs)
        self.pred0 = dict(h.pred)
        h.plans[-1]["macro_steps"] = [[recs[j].name for j in m] for m in macros]
        h.plans[-1]["goal_writers"] = [recs[i].name for i in sorted(self.goal_writers)]
        for k, members in enumerate(macros):
            if time.monotonic() - h.t0 > h.time_limit:
                h.stop_reason = "time limit"
                return "stop"
            tracked = [r for r in recs if not r.done and r.macro <= k]
            if k == len(macros) - 1:
                needed = {r.i for r in tracked}
            else:
                needed = {j for j in members if not recs[j].done} | \
                    {r.i for r in tracked if r.macro < k and any(r.i in recs[j].preds for j in members)}
            h.progress(f"plan {n_plan} window {k + 1}/{len(macros)}: {recs[members[0]].name} at frame {h.branch.frame} "
                       f"({len(needed)} records needed, {len(tracked)} tracked)")
            res = self.window(recs, members, tracked, needed, h.branch, n_plan, k)
            wr, best = res["rec"], res["best"]
            ugs = [c for c in wr["children"] if c[3] == "goal, not from the plan"]
            if ugs:
                self.unplanned_goals.append({"plan": n_plan, "macro": k, "children": len(ugs), "first": ugs[0]})
            h.progress(f"  window {k + 1}: {wr['end_reason']}; {wr['forks']} children {wr['outcomes']}; "
                       f"best {wr.get('best', {}).get('end')} (Q {wr.get('best', {}).get('q_end')}); "
                       f"{wr['wall_s']} s")
            if best is None:
                self._mark(recs, res["parent_matched"], None)
                self.flush()
                h.observe()
                h.checkpoint()
                self._predict(recs)
                if wr["end_reason"] == "goal in the parent":  # the plan's own events wrote it
                    self.goal_credited = True
                    self._logs(n_plan, recs, members, res, "goal")
                    return "goal"
                if wr["end_reason"] == "goal in the parent, not from the plan":
                    self.unplanned_goals.append({"plan": n_plan, "macro": k, "parent": h.branch.frame,
                                                 "qf": h.branch.status.get("qf")})
                    h.stop_reason = (f"goal, not from the plan: it held in the idling engine at frame "
                                     f"{h.branch.frame} with none of the plan's goal records matched")
                    self._logs(n_plan, recs, members, res, "stop")
                    return "stop"
                if wr.get("delta_violation"):
                    h.stop_reason = f"var 19 below δ_lo at {wr['delta_violation']}: Lemma 2's premise failed"
                    self._logs(n_plan, recs, members, res, "stop")
                    return "stop"
                if recs[members[0]].kind != "input" and wr["end_reason"] == "matched in the parent":
                    self._logs(n_plan, recs, members, res, "next")
                    continue
                self.replan_cause = f"window empty: {wr['end_reason']}; children {wr['outcomes']}"
                self._logs(n_plan, recs, members, res, "replan")
                return "replan"
            # a success: the chosen child's snapshot (at the goal, its live branch) is the engine from here
            h.ex.movie.extend(best.events)
            self.flush()
            self._mark(recs, res["parent_matched"], best.g)
            if best.outcome == "deviation":
                # section 7: the deviating child's branch at its stop is the engine; the plan is made again from there
                self._mark(recs, best.matched, None)
                if best.snapshot is not None:
                    best.snapshot.quit()
                best.snapshot = None
                h.checkpoints.append(best.checkpoints[0])
                if best.checkpoints[-1][0] != best.checkpoints[0][0]:
                    h.checkpoints.append(best.checkpoints[-1])  # its stop: the next input comes after the replan
                b_, best.live = best.live, None
                self._set_branch(b_)
                self._predict(recs)
                self._logs(n_plan, recs, members, res, "replan")
                self.replan_cause = f"deviation at frame {best.end}: {str(best.why)[:300]}"
                self.deviations.append({"plan": n_plan, "macro": k, "frame": best.end, "qf": best.q_end,
                                        "why": str(best.why)[:300]})
                return "replan"
            if best.outcome == "goal":
                self._mark(recs, best.matched, None)
                b_, other = best.live, best.snapshot
            else:
                self._mark(recs, best.matched, best.issue)
                b_, other = best.snapshot, best.live
                self.pending_cp = (best.checkpoints[-1], len(h.ex.movie))
            if other is not None:
                other.quit()
            best.snapshot = best.live = None
            h.checkpoints.append(best.checkpoints[0])  # the last input's frame: no later input comes on or before it
            if best.outcome == "goal" and best.checkpoints[-1][0] != best.checkpoints[0][0]:
                h.checkpoints.append(best.checkpoints[-1])  # the goal's frame: no input after it
            self._set_branch(b_)
            self._predict(recs)
            choices = {recs[i].name: c for i, c in best.choices.items()}
            self._logs(n_plan, recs, members, res, "goal" if best.outcome == "goal" else
                       ("replan" if choices else "next"))
            if best.outcome == "goal":
                self.goal_credited = True
                return "goal"
            if choices:  # the harness's rule: a guessed write the engine took otherwise is a replan
                self.replan_cause = f"a guessed write took another value than the plan's: {choices}"
                return "replan"
        self.replan_cause = "the plan played out without the goal"
        return "replan"

    def summary(self) -> dict:
        ws = self.windows
        return {"windows": len(ws), "children": sum(w["forks"] for w in ws),
                "child_frames": sum(w["child_frames"] for w in ws),
                "outcomes": dict(sum((collections.Counter(w["outcomes"]) for w in ws), collections.Counter())),
                "engine_cpu_s": {k: round(v, 2) for k, v in sorted(self.cpu.items())},
                "python_cpu_s": round(self.py_cpu, 2), "forks": dict(self.forks),
                "early_successes": sum(w["early_successes"] for w in ws),
                "bound_violations": sum(len(w["bound_violations"]) for w in ws),
                "goal_from_the_plan": self.goal_credited, "unplanned_goals": self.unplanned_goals,
                "memory_kb": dict(self.mem), "deviations": list(self.deviations),
                "config": dataclasses.asdict(self.cfg)}
