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
* the matcher checks the writes a later step reads (Lemma 8, ``checked_keys``); a click's record must show in its
  frame or the next (Lemma 9), else another record of the same click (``source_index``, ``other_path``) or the child
  is spent; a child whose input took effect and whose plan then failed is a deviation, an end point at the next frame
  the engine takes a click (section 3).
* ``Search.play_plan``: the windows of one plan in order; the chosen child's snapshot, taken at its last input frame,
  is the next window's parent; at a deviation the deviating child's branch is the engine and the plan is made again
  (section 7). It returns "goal", "replan" (with ``replan_cause``) or "stop".

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
    # window-search.md section 9: B1's strict matcher, the click decided at s + 1, ties, Lemma 7′ and the causes
    # (strict.py); off by default, and off the search is sections 1-7
    strict: bool = False
    repeat_stop: int = 2  # section 9.5 (strict only): the run stops at this many empty windows of one input and cause
    # section 9.7: a child's state dump and hit tests kept until its engine runs again, the first seeded with the
    # parent's dump at the fork; off by default
    cache: bool = False


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


def is_deviation(m: Matcher, input_rec, before_input: set, took="n/a") -> bool:
    """Section 3, deviation: every needed record preceding the input has matched, and the input took effect:
    ``took`` is "plan" (a click whose record showed, Lemma 9), another of its paths with a change a later step reads
    (that record's name), or None; for an input that is no click, its record matched."""
    if input_rec is None or not all(i in m.matched for i in before_input):
        return False
    return took is not None if took != "n/a" else input_rec.i in m.matched


def _plain(o):
    """A cause or a record as JSON values (runs and values as dicts, sets as sorted lists)."""
    if isinstance(o, dict):
        return {str(k): _plain(v) for k, v in o.items()}
    if isinstance(o, (list, tuple)):
        return [_plain(x) for x in o]
    if isinstance(o, (set, frozenset)):
        return sorted((_plain(x) for x in o), key=str)
    if dataclasses.is_dataclass(o) and not isinstance(o, type):
        return _plain(dataclasses.asdict(o))
    return o


def strict_stop(best_q: int, best_deviation: bool, qf: int, b_qf: int, b_dev_qf: int) -> bool:
    """Section 9.3, the parent's stop at a tie: no fork at Q(g) = ``qf`` or later can end before the best (``best_q``):
    with a success the best, Q(g) + b ≥ c* (a later fork loses a tie to the earlier); with a deviation, a later success
    at Q = c_d still wins, so Q(g) + b > c_d and Q(g) + 4·δ_lo·n ≥ c_d."""
    if not best_deviation:
        return qf + b_qf >= best_q
    return qf + b_qf > best_q and qf + b_dev_qf >= best_q


def click_gate(st: dict) -> bool:
    """The engine takes a click on the next frame: input on, no cutscene, a room (the Executor's gate)."""
    return st.get("up", 0) > 0 and st.get("cs", 1) == 0 and bool(st.get("room", 0))


def end_key(ch) -> tuple:
    """Section 5, Stop: the earliest end point, a success (or the goal) before a deviation at equal Q, then the earliest
    fork."""
    return ch.q_end, ch.outcome == "deviation", ch.g


LEMMA9 = "another path of the click, no change a later step reads"
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


class _Cached:
    """A child's branch whose state dump and hit tests are kept until the engine runs again (window-search.md section
    9.7): the engine's state changes only while it runs a frame (the bridge reads commands only while it is stopped,
    and scheduled inputs land on later frames), so a second ``state`` or ``at`` at the same stop gives the first's
    answer. The first dump is the parent's at the fork (A0: the child is the parent at that stop)."""

    def __init__(self, b, seed: dict | None = None):
        self._b, self._state, self._at = b, seed, {}
        self.hits = collections.Counter()

    def __getattr__(self, name):
        return getattr(self._b, name)

    def run(self, **kw) -> dict:
        self._state, self._at = None, {}
        return self._b.run(**kw)

    def set_rng(self, state: int) -> None:
        self._state, self._at = None, {}
        self._b.set_rng(state)

    def state(self) -> dict:
        if self._state is None:
            self._state = self._b.state()
            self.hits["state, asked"] += 1
        else:
            self.hits["state, kept"] += 1
        return self._state

    def at(self, x: int, y: int) -> dict:
        got = self._at.get((x, y))
        if got is None:
            got = self._at[(x, y)] = self._b.at(x, y)
            self.hits["at, asked"] += 1
        else:
            self.hits["at, kept"] += 1
        return got


class _FirstWatched(RP._Watched):
    """replay._Watched that also keeps the frame and the state dump before the first input event it schedules."""

    def __init__(self, b):
        super().__init__(b)
        self.first_before = None

    def inputs(self, events: list) -> None:
        if self.first_before is None and any(RP._is_input(e) for e in events):
            self.first_before = (self._b.frame, self._b.state())
        super().inputs(events)


class _Stepper:
    """A child's branch while the input code plays, one frame at a time (section 9.1): after each frame ``on_frame``
    reads its runs, explain log and view. A run to a frame, or to a condition with a frame limit, is played as single
    frames with the same stop rule on each (the bridge's ``until`` or ``control`` with ``max_frame`` one frame on)."""

    def __init__(self, b, on_frame):
        self._b, self._on = b, on_frame

    def __getattr__(self, name):
        return getattr(self._b, name)

    def run(self, *, frame=None, until=None, control=False, max_frame=None, idle=False, until_any=None,
            menu=False) -> dict:
        b = self._b
        if idle or until_any or menu or (frame is None and max_frame is None):
            f0 = b.frame
            st = b.run(frame=frame, until=until, control=control, max_frame=max_frame, idle=idle,
                       until_any=until_any, menu=menu)
            if "end" not in st and b.frame != f0:
                self._on()
            return st
        if frame is not None and not until and not control:
            if frame <= b.frame:
                f0 = b.frame
                st = b.run(frame=frame)
                if "end" not in st and b.frame != f0:
                    self._on()
                return st
            st = b.status
            while b.frame < frame:
                st = b.run(frame=b.frame + 1)
                if "end" in st:
                    return st
                self._on()
            return st
        end = max_frame if max_frame is not None else frame
        while True:
            st = b.run(until=until, control=control, max_frame=b.frame + 1)
            if "end" in st:
                return st
            self._on()
            if st.get("stop") in ("until", "control") or b.frame >= end:
                return st


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
    dev_stop: int | None = None  # a deviation's stop frame (its end point is the next click gate)
    took: str | None = "n/a"  # a click: "plan" (its record showed, Lemma 9), another path's name, or None
    # the strict search (section 9): the branch's strict matches, at the snapshot's frame, its first failure
    track: object = None
    track_issue: object = None
    fail: dict | None = None
    choice_fail: dict | None = None
    d0: dict | None = None  # the parent's state dump at the fork

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
        # the strict search (section 9): the reading as B1 reads it, the IR, this plan's strict parts and its strict
        # matches carried from window to window, the causes, the Python CPU by part
        self.sr = self.code = self.splan = self.strack = None
        self.causes: list = []
        self.empty_keys: collections.Counter = collections.Counter()
        self.prof: collections.Counter = collections.Counter()

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
        if cfg.cache:  # section 9.7
            cb = _Cached(cb, ch.d0)
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
                if w["click_paths"] is not None and f in (ch.issue, ch.issue + 1) and ch.took in ("n/a", None):
                    # Lemma 9: the click's record shows in its frame or the next one (the verb script may queue the
                    # sentence there), or another of its paths does, or it did nothing a later step reads
                    if path_shows(w["click_paths"], now):
                        ch.took = "plan"
                    else:
                        ch.alt = other_path(w["alts"], now, ch.full_at_fork)
                        ch.took = ch.alt
                        if ch.took is None and f == ch.issue + 1:
                            ch.outcome, ch.why = "spent", LEMMA9
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
                if (goal or m.complete()) and ch.took in ("n/a", "plan"):
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
            if ch.outcome in ("spent", "mismatch", "limit") and ch.why != LEMMA9 and \
                    is_deviation(m, w["input"], w["before_input"], ch.took):
                ch.outcome = "deviation"  # section 3: the input took effect, then the plan's prediction ended
                ch.dev_stop = cb.frame
                # its end point: the first frame the engine takes a click again, where the replan starts (section 7)
                while not click_gate(cb.status) and cb.frame - ch.dev_stop < cfg.window_frames:
                    with self._lock:
                        best = w["best"]
                    if best is not None and cb.status["qf"] >= best.q_end:
                        ch.outcome = "pruned"
                        break
                    st = cb.run(frame=cb.frame + 1)
                    if "end" in st:
                        ch.outcome, ch.why = "ended", st["end"]
                        break
                if ch.outcome == "deviation" and cb.frame != ch.dev_stop:
                    s = cb.state()
                    obs = F.facts_of(s)
                ch.end, ch.q_end = cb.frame, cb.status.get("qf")
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
            # the slots the record leaves at wait points that no later tracked record resumes (Lemma 9): a wait the
            # plan's next record of that slot resumes from may not block, and the frame end then shows the later value
            later = set().union(*[q.writes for q in tracked if q.i > first.i])
            click_paths = [{kk: x for kk, x in e.items() if kk.startswith("pos:") and kk in doms_all and kk not in later}
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
                        if cfg.cache:
                            ch.d0 = s
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

    # --- the strict search (window-search.md section 9) ---

    def _strict_setup(self) -> None:
        """The reading as B1 reads it and the IR, once per run (strict.py)."""
        if self.sr is not None:
            return
        import model as M_
        import strict as S
        import trace_explain as TE
        h = self.h
        lines = getattr(getattr(h.base, "lines", None), "path", None)
        if lines is None:
            raise RP.Stop("the strict window search needs the reading's lines file (a packed reading)")
        unobserved = ("saved:",) + ((RP.ACTOR_ROOM,) if h.unobserved_actor_rooms else ())
        self.sr = S.Reading(h.base, h.doms, lines, h.lists, unobserved)
        self.code = TE.Code(M_.Game.load())

    def _tick(self, part: str, t0: float) -> float:
        t = time.thread_time()
        with self._lock:
            self.prof[part] += t - t0
        return t

    def _strict_alternatives(self, first: MRec, recs: list) -> list:
        """The click's other records (section 3, 9.3): (name, action index, the units their ``pos:`` writes name, their
        writes a later step reads)."""
        h = self.h
        if self._by_source is None:
            self._by_source = source_index(h.base.lines.path)
        own = {p.name for p in first.paths}
        out = []
        for i in self._by_source.get(tuple(first.paths[0].what.source), ()):
            a = h.base.actions[i]
            if a.name in own:
                continue
            eff = self.sr.rec(i)[4]
            linked = later_reads(recs, first.i, [k for k in eff if not k.startswith("pos:")])
            out.append((a.name, i, [k[4:] for k in eff if k.startswith("pos:")], linked))
        return out

    def window_strict(self, recs: list, members: list, tracked: list, needed: set, parent, plan_no: int,
                      k: int) -> dict:
        """``window`` with section 9: the parent matched strictly on its own runs (Lemma 7′ for the end), the children
        by ``_child_strict``, the ties of 9.3, the cause of an empty window or a deviation (9.5). Returns as
        ``window``, with "track" (the parent's strict matches at the end) and "cause"."""
        import strict as S
        h, cfg, sp = self.h, self.cfg, self.splan
        t0, cpu0 = time.monotonic(), time.process_time()
        first = recs[members[0]]
        has_input = first.kind == "input"
        inp = RP.step_input(first.paths[0].what) if has_input else None
        alts = self._strict_alternatives(first, recs) if has_input and inp.kind == "click" else []
        reads = set().union(*[recs[j].reads for j in members])
        extra = set()
        for _, i, _, _ in alts:
            r_ = self.sr.rec(i)
            extra |= set(r_[3]) | set(r_[4])
        keys = set().union(*[r.writes | r.reads for r in tracked]) | reads | set(h.goal) | {"room"} | \
            {f"pos:{r.unit}" for r in tracked if r.unit} | extra
        args_units = sorted({kk[5:] for kk in keys if kk.startswith("args:")})
        keys |= {f"pos:{u}" for u in args_units}
        doms = {kk: h.doms[kk] for kk in keys if kk in h.doms and not kk.startswith(h.unobserved)}
        full_doms = {kk: d for kk, d in h.doms.items() if not kk.startswith(h.unobserved)}
        fv, av = FastView(h.b, doms, h.unobserved), S.ArgsView(h.b, h.lists, args_units)

        def sview(obs, s):
            v = fv(obs)
            v.update(av(s, v))
            return v
        observed_reads = sorted(kk for kk in reads if kk in doms)
        before_input = {r.i for r in tracked if has_input and r.i in first.preds}
        input_reads = sorted(kk for kk in (first.reads if has_input else ()) if kk in doms)
        issued = {r.i for r in tracked if r.kind == "input" and r.macro < k}
        n = input_clicks(first.paths[0].what) if has_input else 0
        chain, chain_names = chain_frames(recs, members) if has_input else (0, [])
        b_frames = n + chain
        b_qf = 4 * cfg.delta_lo * b_frames
        goal_writers = {r.i for r in tracked if r.i in self.goal_writers}
        # the units the children's explain is armed for: the tracked records' and the input's pos: writes'
        units = {r.unit for r in tracked if r.unit}
        in_units = {}
        if has_input:
            for nn, idx in enumerate(sp.idx[first.i]):
                in_units[nn] = [kk[4:] for kk in self.sr.rec(idx)[4] if kk.startswith("pos:")] if idx is not None else []
                units |= set(in_units[nn])
        for _, _, us, _ in alts:
            units |= set(us)
        explain_units = [wn for u in sorted(units) for wn in S.unit_where(u)]
        pm = Matcher(tracked, needed, None, issued=issued)  # the mismatch rule's predicted positions (section 3)
        pm.plan_pending = [r for r in recs if not r.done]
        w = {"goal_writers": goal_writers, "plan_pending": pm.plan_pending, "inp": inp, "name": first.name,
             "tracked": tracked, "needed": needed, "input": first if has_input else None, "doms": doms,
             "full_doms": full_doms, "fv": fv, "sview": sview, "full_fv": FastView(h.b, full_doms, h.unobserved),
             "best": None, "firsts": set(), "b_qf": b_qf, "bound_violations": [], "delta_violation": None,
             "issued": issued, "before_input": before_input, "alts": alts, "in_units": in_units,
             "lemma9_stops": 0, "b_dev_qf": 4 * cfg.delta_lo * n, "explain_units": explain_units,
             "predicted_keys": set().union(*[set(e) for r in tracked for e in r.exps]),
             "failures": collections.Counter(), "failure_of": {}, "k": k, "first_g": {}, "running": 0,
             # a tracked record that does not follow the input: its own run may come while the input code plays
             "early_recs": {r.i for r in tracked if not (has_input and first.i in r.preds) and r.i != first.i}}
        rec = {"plan": plan_no, "macro": k, "input": first.name if has_input else None, "strict": True,
               "records": [recs[j].name for j in members], "needed": [r.name for r in tracked if r.i in needed],
               "tracked": [r.name for r in tracked], "start": parent.frame, "start_qf": parent.status.get("qf"),
               "b_frames": b_frames, "b_qf": b_qf, "n_clicks": n, "chain": chain_names, "delta_lo": cfg.delta_lo,
               "observed_reads": len(observed_reads), "before_input": len(before_input), "open": None,
               "alternatives": len(alts), "args_units": len(args_units)}
        # the parent: segment on, explain on every unit, its strict matches from the plan's so far
        parent.segment(True)
        parent.runs()
        parent.explain(parent.frame + 1, parent.frame + cfg.window_frames + cfg.child_limit + 2, None)
        parent.explain_log()
        ptrack = self.strack.copy()
        pkept = S.Kept()
        s = parent.state()
        obs = F.facts_of(s)
        now = sview(obs, s)
        prev_view, prev_dump = now, s
        pkept.frame(parent.frame, [], s, now)
        pool = ThreadPoolExecutor(max_workers=cfg.workers)
        futures, children = [], []
        end_rule, prev, end_reason, gate_shut = EndRule(), {}, None, 0
        w0 = parent.frame
        parent_fail, pgv = None, None
        try:
            while True:
                st = parent.status
                g = parent.frame
                if cfg.delta_lo > 1 and max(1, st.get("v19", 0)) < cfg.delta_lo:
                    w["delta_violation"] = [g, st.get("v19")]
                    end_reason = "delta"
                    break
                ref = [i for i in needed if i in ptrack.refuted]
                if ref:
                    i = ref[0]
                    f_, why_, run_ = ptrack.refuted[i]
                    parent_fail = {"kind": "refuted", "record": recs[i].name, "frame": f_, "why": why_, "run": run_,
                                   "material": pkept.freeze()}
                    end_reason = "a needed record refuted in the parent"
                    break
                if not h._holds(h.goal, now):
                    credited = bool(goal_writers) and all(i in ptrack.matched for i in goal_writers)
                    end_reason = "goal in the parent" + ("" if credited else ", not from the plan")
                    if not credited:
                        parent_fail = self._goal_writer_fail(pkept, g, pgv, now)
                    break
                pgv = now
                if not has_input:
                    if all(i in ptrack.matched for i in needed):
                        end_reason = "matched in the parent"
                        break
                else:
                    belief = dict(now, **{kk: v for kk, v in h.kept.items() if not kk.startswith("args:")})
                    pre_now = any(not h._holds(a.pre, belief) for a in first.paths)
                    ready = all(i in ptrack.matched for i in before_input)  # Lemma 7′
                    if end_rule.closed(ready, pre_now):
                        end_reason = "the input's precondition closed in the parent after its window opened"
                        rec["end_change"] = [[kk, prev.get(kk), now.get(kk)] for kk in input_reads
                                             if prev.get(kk) != now.get(kk)][:8]
                        break
                    if rec["open"] is None and end_rule.open:
                        rec["open"] = g
                    prev = now
                    with self._lock:
                        best = w["best"]
                    # section 9.3: the parent's stop at a tie
                    if best is not None and strict_stop(best.q_end, best.outcome == "deviation", st["qf"], b_qf,
                                                        w["b_dev_qf"]):
                        end_reason = "stop: Q(g) + b reached the best " + \
                            ("deviation" if best.outcome == "deviation" else "success")
                        break
                if g - w0 >= cfg.window_frames:
                    end_reason = f"{cfg.window_frames} frames"
                    break
                if has_input:
                    if self._gate(inp, st, s, parent):
                        while w["running"] >= cfg.workers:
                            time.sleep(0.002)
                        while mem_available_kb() < cfg.mem_floor_kb:
                            if not any(not x.done() for x in futures):
                                raise RP.Stop(f"memory: MemAvailable {mem_available_kb() // 1024} MB, under "
                                              f"{cfg.mem_floor_kb // 1024} MB")
                            time.sleep(0.05)
                        cb = self.fork(parent, "child")
                        ch = Child(g, st["qf"], not pre_now, frozenset(ptrack.matched), dict(now), None)
                        ch.track, ch.d0 = ptrack.copy(), s
                        children.append(ch)
                        with self._lock:
                            w["running"] += 1
                        futures.append(pool.submit(self._child_strict, w, cb, ch))
                        if len(children) % 100 == 0:
                            done_ = collections.Counter(c.outcome for c in children if c.outcome)
                            h.progress(f"    window {k + 1}: {len(children)} children by frame {g}, {dict(done_)}; "
                                       f"failures {w['failures'].most_common(2)}")
                    else:
                        gate_shut += 1
                st = parent.run(frame=g + 1)
                if "end" in st:
                    end_reason = f"engine ended: {st['end']}"
                    break
                rows = parent.runs()["runs"]
                ents = parent.explain_log()["explain"]
                s = parent.state()
                obs = F.facts_of(s)
                now = sview(obs, s)
                R_ = S.by_frame(S.runs_of(rows)).get(g + 1, [])
                for run_, v_ in ptrack.verdicts(R_, prev_view, now, prev_dump):
                    ptrack.apply(g + 1, run_, v_, now)
                pkept.add_entries(ents)
                pkept.frame(g + 1, R_, s, now)
                prev_view, prev_dump = now, s
        finally:
            for x in futures:
                x.result()
            pool.shutdown(wait=True)
        best = w["best"]
        if best is not None and best.first in w["first_g"]:  # the earliest fork of the same run (Lemma 5)
            best.g = min(best.g, w["first_g"][best.first])
        counts = collections.Counter(c.outcome for c in children)
        # section 9.5: the cause, on the kept failure of the chosen deviation or of the failure most children share
        cause, fail = None, None
        if best is not None and best.outcome == "deviation":
            fail = best.fail
        elif best is None:
            fail = parent_fail
            if fail is None and w["failures"]:
                sig = w["failures"].most_common(1)[0][0]
                fail = w["failure_of"][sig]
        if fail is not None:
            cause = S.explain(self.code, self.sr, fail)
        rec.update(end=parent.frame, end_qf=parent.status.get("qf"), end_reason=end_reason, forks=len(children),
                   gate_shut=gate_shut, outcomes=dict(sorted(counts.items())),
                   child_frames=sum(c.frames for c in children),
                   early_successes=sum(1 for c in children if c.early and c.outcome in ("success", "goal")),
                   lemma9_stops=w["lemma9_stops"], other_paths=collections.Counter(c.alt for c in children if c.alt),
                   bound_violations=w["bound_violations"], delta_violation=w["delta_violation"],
                   failures=[[list(sig), nn] for sig, nn in w["failures"].most_common(8)],
                   mismatch_keys=collections.Counter(str(c.why[0]) for c in children if c.outcome == "mismatch"
                                                     and isinstance(c.why, tuple)).most_common(8),
                   children=[c.row() for c in children[:CHILD_ROWS]],
                   wall_s=round(time.monotonic() - t0, 2), python_cpu_s=round(time.process_time() - cpu0, 2),
                   cause=_plain(cause))
        by = {r.i: r.name for r in recs}
        if best is not None:
            rec["best"] = {"g": best.g, "first": best.first, "issue": best.issue, "end": best.end, "q0": best.q0,
                           "q_end": best.q_end, "outcome": best.outcome, "early": best.early, "took": best.took,
                           "frames_issue_to_end": best.end - best.issue, "arrival": best.arrival,
                           "walk_frames": (best.arrival - best.issue) if best.arrival else None,
                           "matched": {by.get(i, str(i)): f for i, (f, _, _) in best.track.matched.items()
                                       if i not in ptrack.matched or ptrack.matched[i][0] is None},
                           "unplanned": len(best.unplanned)}
        rec["parent_matched"] = {by.get(i, str(i)): f for i, (f, _, _) in ptrack.matched.items()}
        rec["parent_refuted"] = {by.get(i, str(i)): [f, why] for i, (f, why, _) in ptrack.refuted.items()}
        self.py_cpu += time.process_time() - cpu0
        self.windows.append(rec)
        if h.workdir is not None:
            with open(Path(h.workdir).parent / "windows.jsonl", "a") as fh:
                fh.write(json.dumps(rec, default=str) + "\n")
        parent.explain_off()
        return {"rec": rec, "best": best, "track": ptrack, "cause": cause,
                "parent_matched": {i: (v[0], v[1]) for i, v in ptrack.matched.items() if v[0] is not None}}

    def _goal_writer_fail(self, pkept, g: int, before: dict | None, now: dict) -> dict | None:
        """The run of frame g that wrote a goal fact the plan's goal records did not (9.5), from the parent's kept
        runs and its explain log of every unit."""
        import strict as S
        facts = [kk for kk in self.h.goal if before is None or before.get(kk) != now.get(kk)]
        mat = pkept.freeze()
        eng = S._Eng(mat["entries"])
        for run in mat["runs"].get(g, []):
            key = self.code.key(run.where, run.number, run.room)
            lg = S._logged(eng, run)
            if key is None or lg is None:
                continue
            idxs = {self.code.index(key, op.code) for op in lg.ops}
            for fact in facts:
                if idxs & self.code.write_sites(key, fact):
                    return {"kind": "goal writer", "record": None, "frame": g, "why": f"wrote {fact}", "run": run,
                            "fact": fact, "material": mat}
        return None

    def _child_strict(self, w: dict, cb, ch: Child) -> Child:
        """Section 3 with section 9 for one child forked at ``ch.g``."""
        import strict as S
        h, cfg, sp, sr = self.h, self.cfg, self.splan, self.sr
        snap = None
        t_start = tt = time.thread_time()
        kept = S.Kept()
        frames_seen: dict = {}  # frame -> (view, dump), the frames this child observed
        track = ch.track
        first = w["input"]
        if cfg.cache:  # section 9.7
            cb = _Cached(cb, ch.d0)
        try:
            cb.explain(ch.g + 1, ch.g + cfg.child_limit + cfg.window_frames + 2, w["explain_units"])
            observe_play = any(i not in track.matched and i not in track.refuted for i in w["early_recs"])
            rows_all: list = []

            def on_frame():
                t_ = time.thread_time()
                rows_all.extend(cb.runs()["runs"])
                kept.add_entries(cb.explain_log()["explain"])
                s_ = cb.state()
                frames_seen[cb.frame] = (w["sview"](F.facts_of(s_), s_), s_)
                self._tick("observe", t_)
            branch = _Stepper(cb, on_frame) if observe_play else cb
            ctx = _Ctx(branch, self.release, self.stone_fork if self.stones else None)
            ctx.watched = ctx.ex.b = _FirstWatched(branch)  # the view before the first input event (I1)
            tt = self._tick("setup", tt)
            try:
                self._play(ctx, w["inp"], w["name"])
            except RouteError as e:
                ch.outcome, ch.why = "interface", str(e)
                return ch
            tt = self._tick("play", tt)
            ins = [e for e in ctx.ex.movie if RP._is_input(e)]
            ch.events = list(ctx.ex.movie)
            if not ins:
                ch.outcome, ch.why = "interface", "no input event"
                return ch
            ch.first, ch.issue = min(e["frame"] for e in ins), max(e["frame"] for e in ins)
            with self._lock:
                if ch.first in w["first_g"]:  # Lemma 5: the same run as the child whose first event lands there
                    w["first_g"][ch.first] = min(w["first_g"][ch.first], ch.g)
                    ch.outcome = "duplicate"
                    return ch
                w["first_g"][ch.first] = ch.g
            if cb.frame != ch.issue:
                st = branch.run(frame=ch.issue)
                if "end" in st:
                    ch.outcome, ch.why = "ended", st["end"]
                    return ch
            s_issue = ch.issue
            if s_issue not in frames_seen:
                on_frame()
            else:
                rows_all.extend(cb.runs()["runs"])
                kept.add_entries(cb.explain_log()["explain"])
            st_issue = dict(cb.status)
            d_issue = frames_seen[s_issue][1]
            snap = self.fork(cb, "snapshot")  # the next window's parent (section 7), its buffers empty
            st = cb.run(frame=s_issue + 1)
            if "end" in st:
                ch.outcome, ch.why = "ended", st["end"]
                return ch
            on_frame()
            tt = time.thread_time()
            Rk = S.by_frame(S.runs_of(rows_all))
            frames_seen.setdefault(ch.g, (ch.at_fork, ch.d0))
            for f in sorted(Rk):
                kept.frame(f, Rk[f], frames_seen.get(f, (None, None))[1], frames_seen.get(f, (None, None))[0])
            v0, d0 = ch.at_fork, ch.d0
            fb = ctx.watched.first_before
            if fb is not None and fb[0] != ch.g:  # the input code waited before its first event: V_{f_1−1}
                v0, d0 = w["sview"](F.facts_of(fb[1]), fb[1]), fb[1]
                frames_seen.setdefault(fb[0], (v0, d0))
            # 9.2: the click's record and its other records, each decided once its runs have come
            cands = [("plan", nn, idx, w["in_units"][nn], None) for nn, idx in enumerate(sp.idx[first.i])
                     if idx is not None] + [("alt", name, idx, us, linked) for name, idx, us, linked in w["alts"]]
            needs = [[kk[4:] for kk, e in sr.rec(c[2])[4].items() if kk.startswith("pos:") and e[0] == "set"
                      and isinstance(e[1], int) and e[1] >= 0] for c in cands]
            own = [{} for _ in cands]
            decided: list = [None] * len(cands)
            matched_writes: list = []  # (seq of the run, its record's set writes), the event records matched here

            def decide(n_, f, va, runs_f):
                c = cands[n_]
                seqs = [r.seq for r in own[n_].values()]
                lo, hi = (min(seqs), max(seqs)) if seqs else (None, None)
                applied = {}
                for sq, wr in matched_writes:
                    if lo is None or sq < lo:
                        applied.update(wr)
                later = [r for r in runs_f if hi is not None and r.seq > hi]
                decided[n_] = S.input_match(sr, sp, c[2], c[3], v0, d0, applied, own[n_], va, later)
                decided[n_]["frame"] = f

            def settle(final: bool = False) -> None:
                """The click's outcome once its candidates are decided (Lemma 9′); ``final`` at the idle stop or the
                child's limit, where a candidate whose runs have not come fails."""
                if ch.took != "n/a":
                    return
                plan_c = [n_ for n_, c in enumerate(cands) if c[0] == "plan"]
                ok = next((n_ for n_ in plan_c if decided[n_] is not None and decided[n_]["ok"]), None)
                if ok is not None:
                    res_ = decided[ok]
                    track.matched[first.i] = (s_issue, cands[ok][1], res_["last_seq"])
                    ch.took = "plan" if res_["took"] else "plan, no change"
                    ch.track_issue = track.copy()
                    return
                if not final and any(decided[n_] is None for n_ in plan_c):
                    return
                alt_c = [n_ for n_, c in enumerate(cands) if c[0] == "alt"]
                for n_ in alt_c:
                    rj, linked = decided[n_], cands[n_][4]
                    if rj is not None and rj["ok"] and rj["took"] and \
                            any(frames_seen[rj["frame"]][0].get(kk) != v0.get(kk) for kk in linked):
                        ch.alt = ch.took = cands[n_][1]
                        ch.track_issue = track.copy()
                        return
                if final or all(decided[n_] is not None for n_ in alt_c):
                    ch.took = None

            def frame_step(f) -> None:
                """R_f in run order: the click's runs collected, a candidate decided once its runs have come (before
                the event runs after them), the event runs matched (Definition S)."""
                vb, va = frames_seen.get(f - 1, (None, None)), frames_seen.get(f, (None, None))
                runs_f = Rk.get(f, [])
                vs = dict((r.seq, v) for r, v in track.verdicts(runs_f, vb[0], va[0], vb[1])) \
                    if vb[0] is not None and va[0] is not None else {}
                for r in runs_f:
                    if r.depth >= 1 and r.frame >= s_issue and ch.took == "n/a":
                        for n_, c in enumerate(cands):
                            if r.unit in c[3] and r.unit not in own[n_]:
                                own[n_][r.unit] = r
                    if r.seq in vs:
                        if ch.took == "n/a" and va[0] is not None:
                            for n_ in range(len(cands)):
                                if decided[n_] is None and own[n_] and all(u in own[n_] for u in needs[n_]) and \
                                        max(x.seq for x in own[n_].values()) < r.seq:
                                    decide(n_, f, va[0], runs_f)
                            settle()
                        for r_, how in track.apply(f, r, vs[r.seq], va[0]):
                            if how == "matched":
                                idx_ = sp.idx[r_.i][track.matched[r_.i][1]]
                                matched_writes.append((r.seq, {kk: e[1] for kk, e in sr.rec(idx_)[4].items()
                                                               if e[0] == "set"}))
                if ch.took == "n/a" and f >= s_issue + 1 and va[0] is not None:
                    for n_ in range(len(cands)):
                        if decided[n_] is None and all(u in own[n_] for u in needs[n_]):
                            decide(n_, f, va[0], runs_f)
                    settle()

            def input_fail(f, why_default="no run") -> None:
                first_c = next((n_ for n_, c in enumerate(cands) if c[0] == "plan"), None)
                res_ = decided[first_c] if first_c is not None else None
                if res_ is None and first_c is not None:
                    miss = [u for u in needs[first_c] if u not in own[first_c]]
                    res_ = {"why": f"no run pos:{miss[0]}" if miss else why_default, "run": None, "engine": None}
                if res_ is None:
                    return
                self._fail(w, ch, {"kind": "input", "record": first.name, "frame": res_.get("frame", f),
                                   "why": res_["why"], "run": res_.get("run"), "want": res_.get("want"),
                                   "engine": res_.get("engine"),
                                   "asks": sr.rec(sp.idx[first.i][0])[3].get(res_["why"].split(" ", 1)[-1])
                                   if sp.idx[first.i][0] is not None else None,
                                   "material": kept.freeze({ch.g: v0}, {ch.g: d0})})

            for f in sorted(Rk):
                if ch.g < f <= s_issue + 1:
                    frame_step(f)
            tt = self._tick("match", tt)
            moved = False
            s, now = frames_seen[s_issue + 1][1], frames_seen[s_issue + 1][0]
            obs = F.facts_of(s)
            m = Matcher(w["tracked"], w["needed"], first, ch.done, issued=w["issued"])
            m.plan_pending = w["plan_pending"]
            while True:
                st = cb.status
                f = cb.frame
                ch.frames = f - ch.g
                if ch.took is None:  # Lemma 9′: neither the click's record nor another of its records can match
                    ch.outcome, ch.why = "spent", "Lemma 9′"
                    with self._lock:
                        w["lemma9_stops"] += 1
                    input_fail(f)
                    break
                if cfg.delta_lo > 1 and max(1, st.get("v19", 0)) < cfg.delta_lo:
                    ch.outcome, ch.why = "delta", st.get("v19")
                    w["delta_violation"] = [f, st.get("v19")]
                    break
                if track.choices and ch.choice_fail is None:
                    i = next(iter(track.choices))
                    fr, path, seq = track.matched[i]
                    run_ = next((r for rs in Rk.values() for r in rs if r.seq == seq), None)
                    ch.choice_fail = {"kind": "choice", "record": next(r.name for r in w["tracked"] if r.i == i),
                                      "frame": fr, "why": f"effect {track.choices[i][0][0]}", "run": run_,
                                      "material": kept.freeze()}
                if st.get("mv"):
                    moved = True
                elif moved and ch.arrival is None:
                    ch.arrival = f
                goal = not h._holds(h.goal, now)
                if goal and not (w["goal_writers"] and all(i in track.matched for i in w["goal_writers"])):
                    ch.outcome, ch.why = "goal, not from the plan", sorted(w["goal_writers"])
                    break
                if goal or all(i in track.matched for i in w["needed"]):
                    ch.outcome = "goal" if goal else "success"
                    break
                ref = [i for i in w["needed"] if i in track.refuted]
                if ref:
                    fr, why_, run_ = track.refuted[ref[0]]
                    ch.outcome, ch.why = "refuted", (next(r.name for r in w["tracked"] if r.i == ref[0]), why_)
                    self._fail(w, ch, {"kind": "refuted", "record": ch.why[0], "frame": fr, "why": why_,
                                       "run": run_, "material": kept.freeze()})
                    break
                m.matched = {i: (v[0], v[1]) for i, v in track.matched.items()}
                mm = m.mismatch(now, ch.at_fork)
                if mm is not None:
                    ch.outcome, ch.why = "mismatch", mm
                    unit = mm[0][4:] if str(mm[0]).startswith("pos:") else None
                    run_ = next((r for r in reversed(Rk.get(f, [])) if r.unit == unit), None) if unit else None
                    self._fail(w, ch, {"kind": "mismatch", "record": None, "frame": f, "why": f"end {mm[0]}",
                                       "run": run_, "material": kept.freeze()})
                    break
                with self._lock:
                    best = w["best"]
                if best is not None and st["qf"] >= best.q_end:  # section 9.3: after the frame's end tests
                    ch.outcome = "pruned"
                    break
                if f > s_issue + 1 and "idle" in st:
                    if ch.took == "n/a":  # Lemma 9′: at the idle stop nothing is left to start the click's runs
                        settle(final=True)
                    if ch.took is None:
                        ch.outcome, ch.why = "spent", "Lemma 9′: no run by the idle stop"
                        with self._lock:
                            w["lemma9_stops"] += 1
                        input_fail(f)
                        break
                    dead = m.dead_needed(now)
                    if dead:
                        ch.outcome, ch.why = "spent", m.unmet(now)
                        self._not_run_fail(w, ch, track, f, kept)
                        break
                if f - s_issue >= cfg.child_limit:
                    ch.outcome, ch.why = "limit", m.unmet(now)
                    if ch.took == "n/a":
                        settle(final=True)
                    if ch.took is None:
                        input_fail(f)
                    else:
                        self._not_run_fail(w, ch, track, f, kept)
                    break
                st = cb.run(frame=f + 1)
                if "end" in st:
                    ch.outcome, ch.why = "ended", st["end"]
                    break
                t_ = time.thread_time()
                rows = cb.runs()["runs"]
                kept.add_entries(cb.explain_log()["explain"])
                s = cb.state()
                obs = F.facts_of(s)
                now = w["sview"](obs, s)
                t_ = self._tick("observe", t_)
                Rk[f + 1] = S.by_frame(S.runs_of(rows)).get(f + 1, [])
                frames_seen[f + 1] = (now, s)
                for old in [x for x in frames_seen if x < f - 1 and x != ch.g]:
                    del frames_seen[old]
                for old in [x for x in Rk if x < f - 2]:
                    del Rk[old]
                kept.frame(f + 1, Rk[f + 1], s, now)
                frame_step(f + 1)
                self._tick("match", t_)
            ch.end, ch.q_end = cb.frame, cb.status.get("qf")
            ch.matched = {i: (v[0], v[1]) for i, v in track.matched.items() if v[0] is not None}
            ch.choices = dict(track.choices)
            took_effect = ch.took == "plan" or (ch.alt is not None and ch.took == ch.alt)  # 9.2, 9.3
            if ch.outcome in ("spent", "mismatch", "limit", "refuted") and \
                    all(i in track.matched for i in w["before_input"]) and took_effect:
                ch.outcome = "deviation"
                ch.dev_stop = cb.frame
                while cb.frame - ch.dev_stop < cfg.window_frames:
                    if click_gate(cb.status):  # section 9.3: the gate first, then the prune
                        break
                    with self._lock:
                        best = w["best"]
                    if best is not None and cb.status["qf"] >= best.q_end:
                        ch.outcome = "pruned"
                        break
                    st = cb.run(frame=cb.frame + 1)
                    if "end" in st:
                        ch.outcome, ch.why = "ended", st["end"]
                        break
                if ch.outcome == "deviation" and cb.frame != ch.dev_stop:
                    cb.runs()
                    cb.explain_log()
                    s = cb.state()
                    obs = F.facts_of(s)
                ch.end, ch.q_end = cb.frame, cb.status.get("qf")
            if ch.outcome in ("success", "goal", "deviation"):
                ch.checkpoints = [(s_issue, F.facts_of(d_issue), RP.extra_of(d_issue, st_issue)),
                                  (cb.frame, dict(obs), RP.extra_of(s, cb.status))]
                if ch.full_at_fork is None:  # the parent's full view at the fork, from its dump there
                    ch.full_at_fork = w["full_fv"](F.facts_of(ch.d0))
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
            self._tick("child total", t_start)
            with self._lock:
                w["running"] -= 1
            if isinstance(cb, _Cached):
                with self._lock:
                    self.prof.update({f"cache: {k}": v for k, v in cb.hits.items()})
            with self._lock:
                if w["best"] is not ch:  # what only the chosen child needs
                    ch.d0 = ch.full_at_fork = ch.at_fork = ch.track_issue = None
                    if ch.fail is not None and w["failure_of"].get(self._sig(ch.fail)) is not ch.fail:
                        ch.fail = dict(ch.fail, material=None)
            for b_ in (snap, cb):
                if b_ is not None:
                    try:
                        b_.quit()
                    except OSError:
                        pass

    @staticmethod
    def _sig(fail: dict) -> tuple:
        return fail["kind"], fail.get("record"), str(fail.get("why"))

    def _fail(self, w: dict, ch: Child, fail: dict) -> None:
        """A child's first failure (9.5), counted by its signature; the first child with a signature keeps it."""
        if ch.fail is not None:
            return
        ch.fail = fail
        sig = self._sig(fail)
        with self._lock:
            w["failures"][sig] += 1
            w["failure_of"].setdefault(sig, fail)

    def _not_run_fail(self, w: dict, ch: Child, track, f: int, kept) -> None:
        """A needed record whose own run did not happen before the child stopped (9.5)."""
        r = next((r for r in w["tracked"] if r.i in w["needed"] and r.i not in track.matched and
                  r.i not in track.refuted), None)
        if r is None:
            return
        self._fail(w, ch, {"kind": "not run", "record": r.name, "frame": f, "why": "its run did not happen",
                           "unit": r.unit, "run": None, "material": kept.freeze()})

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
        strict = self.cfg.strict
        if strict:  # section 9: this plan's strict parts; nothing of it has matched yet
            import strict as S
            self._strict_setup()
            self.splan = S.Plan(self.sr, recs, self.code, {})
            self.strack = S.Track(self.splan, {})
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
            res = (self.window_strict if strict else self.window)(recs, members, tracked, needed, h.branch, n_plan, k)
            wr, best = res["rec"], res["best"]
            cause = res.get("cause")
            if strict and cause is not None:
                h.progress(f"  window {k + 1}: cause: {S.short(cause)}")
            ugs = [c for c in wr["children"] if c[3] == "goal, not from the plan"]
            if ugs:
                self.unplanned_goals.append({"plan": n_plan, "macro": k, "children": len(ugs), "first": ugs[0]})
            h.progress(f"  window {k + 1}: {wr['end_reason']}; {wr['forks']} children {wr['outcomes']}; "
                       f"best {wr.get('best', {}).get('end')} (Q {wr.get('best', {}).get('q_end')}); "
                       f"{wr['wall_s']} s")
            if best is None:
                if strict:
                    self.strack = res["track"]
                    self._strict_mark(recs, self.strack)
                else:
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
                    if strict:
                        self._note_cause(n_plan, k, "goal, not from the plan", cause)
                        h.stop_reason += f"; cause: {S.short(cause)}"
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
                if strict:  # section 9.5: the cause, and the stop at the same empty window again
                    self._note_cause(n_plan, k, "window empty", cause)
                    self.replan_cause += f"; cause: {S.short(cause)}"
                    key = (recs[members[0]].name, (cause or {}).get("kind"), (cause or {}).get("unit"),
                           (cause or {}).get("offset"), (cause or {}).get("why"))
                    self.empty_keys[key] += 1
                    if self.empty_keys[key] >= self.cfg.repeat_stop:
                        h.stop_reason = (f"the same empty window {self.empty_keys[key]} times on "
                                         f"{recs[members[0]].name}: {S.short(cause)}")
                        self._logs(n_plan, recs, members, res, "stop")
                        return "stop"
                self._logs(n_plan, recs, members, res, "replan")
                return "replan"
            # a success: the chosen child's snapshot (at the goal, its live branch) is the engine from here
            h.ex.movie.extend(best.events)
            self.flush()
            if strict:  # section 9: the chosen child's strict matches, at its snapshot (all of them at its end)
                self.strack = (best.track if best.outcome in ("goal", "deviation") else best.track_issue).copy()
                self._strict_mark(recs, self.strack)
            else:
                self._mark(recs, res["parent_matched"], best.g)
            if best.outcome == "deviation":
                # section 7: the deviating child's branch at its stop is the engine; the plan is made again from there
                if not strict:
                    self._mark(recs, best.matched, None)
                if best.snapshot is not None:
                    best.snapshot.quit()
                best.snapshot = None
                h.checkpoints.append(best.checkpoints[0])
                if best.checkpoints[-1][0] != best.checkpoints[0][0]:
                    h.checkpoints.append(best.checkpoints[-1])  # its stop: the next input comes after the replan
                b_, best.live = best.live, None
                # section 7: on to the first frame where the engine takes a click (no input lands before it but a key)
                f0, ran = b_.frame, 0
                while not click_gate(b_.status) and ran < self.cfg.window_frames:
                    st_ = b_.run(frame=b_.frame + 1)
                    ran += 1
                    if "end" in st_:
                        raise RP.Stop(f"engine ended: {st_['end']}")
                if ran:
                    s_ = b_.state()
                    h.checkpoints.append((b_.frame, F.facts_of(s_), RP.extra_of(s_, b_.status)))
                self._set_branch(b_)
                self._predict(recs)
                self._logs(n_plan, recs, members, res, "replan")
                self.replan_cause = f"deviation at frame {best.dev_stop}: {str(best.why)[:300]}"
                if strict:
                    self._note_cause(n_plan, k, "deviation", cause)
                    self.replan_cause += f"; cause: {S.short(cause)}"
                self.deviations.append({"plan": n_plan, "macro": k, "frame": best.dev_stop, "end": best.end,
                                        "qf": best.q_end,
                                        "why": str(best.why)[:300], "replan_frame": b_.frame,
                                        "frames_to_the_click_gate": ran})
                return "replan"
            if best.outcome == "goal":
                if not strict:
                    self._mark(recs, best.matched, None)
                b_, other = best.live, best.snapshot
            else:
                if not strict:
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
                if strict and best.choice_fail is not None:
                    c_ = S.explain(self.code, self.sr, best.choice_fail)
                    self._note_cause(n_plan, k, "a guessed write", c_)
                    self.replan_cause += f"; cause: {S.short(c_)}"
                return "replan"
        self.replan_cause = "the plan played out without the goal"
        if strict:
            self._note_cause(n_plan, len(macros), "the plan played out without the goal",
                             {"cause": None, "kind": "every window succeeded and the goal does not hold"})
        return "replan"

    def _strict_mark(self, recs: list, track) -> None:
        """Section 9: the records a branch matched strictly are done (their frame and path)."""
        for i, (f, path, _seq) in track.matched.items():
            r = recs[i]
            if not r.done:
                r.done, r.done_frame, r.path = True, f, path

    def _note_cause(self, n_plan: int, k: int, what: str, cause) -> None:
        self.causes.append({"plan": n_plan, "macro": k, "what": what, "cause": _plain(cause)})

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
                "config": dataclasses.asdict(self.cfg),
                "causes": list(self.causes), "python_cpu_by_part_s": {k: round(v, 2) for k, v in self.prof.items()},
                "empty_windows": [[list(k), n] for k, n in self.empty_keys.items()]}
