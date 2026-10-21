"""Play the run goal by goal: the route's moves as a hint, the search to find or beat them.

A **milestone** is a route move whose ``until`` has persistent facts (room,
item, owner, bit, state, progress var). The route moves since the previous
milestone are its **hint**. For each milestone the driver

1. plays the hint on a fork (the executor's steering, approach and repairs
   apply), giving a branch and its time, if the hint still works;
2. runs ``MilestoneSearch.reach`` from the current state, bounded by the
   hint's time, so it only reports a branch that is faster or that the hint
   could not reach at all;
3. continues on the faster branch, and appends a result record.

The movie of the winning branches is the run. Records and the movie are
written after every milestone, so a run can resume from its own output.
"""

import json
from dataclasses import dataclass, field, replace
from pathlib import Path

from speedrun.control import ControlError
from speedrun.executor import Executor, Move, RouteError
from speedrun.search import MilestoneSearch, persistent


HINT_FRAMES = 1200  # a hint move that takes longer has failed
HINT_DELAYS = 12  # delay steering tried on a hint move


@dataclass
class Milestone:
    index: int  # the route move that names it
    name: str
    target: list[dict]
    hint: list[Move]
    # The naming move's whole `until`: the target leaves out transient facts (the slide's
    # counters, talking), so it can already hold before the move ("walk toward the cave" until
    # the mushroom line and bit 536 == 0); the milestone is done only when this holds too.
    until: list[dict] = field(default_factory=list)


SLIDE_VARS = {53, 352}  # the userput and cutscene counters the item slide is read from


def _without_slide(conds: list[dict]) -> list[dict]:
    return [c for c in conds if (c.get("not") or c).get("var") not in SLIDE_VARS]


def milestones(moves: list[Move], start: int, drop: set[int] = frozenset(), strip_skips: bool = False) -> list[Milestone]:
    """Milestones from ``moves[start:]``; moves in ``drop`` are neither milestones nor hints.
    ``strip_skips``: a run without the slide: hints keep the bridge's automatic skips on and do
    not wait for the slide's counters."""
    out, pending = [], []
    for i in range(start, len(moves)):
        if i in drop:
            continue
        m = moves[i]
        if strip_skips:
            m = replace(m, skips=None, auto_skips=None, gate="control",
                        after=_without_slide(m.after), until=_without_slide(m.until))
        pending.append(m)
        target = persistent(m.until)
        if target:
            out.append(Milestone(i, m.name, target, pending, list(m.until)))
            pending = []
    return out


@dataclass
class Outcome:
    ex: Executor
    how: str  # "hint", "search" or "hint+search"
    via: list[str]
    qf: int
    alt: "Outcome | None" = None  # the hint's branch when the search beat it (kept for backtracking)


class AutoPlayer:
    def __init__(self, search: MilestoneSearch, fork, release: str = "dos-cd", keep_vars: frozenset[int] = frozenset()):
        self.search = search
        self.fork = fork
        self.release = release
        # Vars a search branch must leave as the hint's branch does (the item slide's counters):
        # a target is reached faster at no visible cost if the slide is dropped on the way.
        self.keep_vars = keep_vars

    def _hint(self, ex: Executor, ms: Milestone) -> Executor | None:
        h = Executor(self.fork(ex.b), self.release, self.fork)
        h.skips, h.movie, h.results = dict(ex.skips), list(ex.movie), []
        h.early = ex.early
        if ex._lag is not None:
            h.b.lag = self.fork(ex._lag)
        try:
            group: list[Move] = []
            for m in ms.hint + [None]:
                if m is not None and m.optional and (not group or group[0].name.split("]")[0] == m.name.split("]")[0]):
                    group.append(m)
                    continue
                if group:
                    h.play_group(group)
                    group = []
                if m is None:
                    break
                if m.optional:
                    group = [m]
                    continue
                h.play_steered(replace(m, max_frames=min(m.max_frames, HINT_FRAMES)), max_delay=HINT_DELAYS,
                               window=HINT_FRAMES)
            if not h.b.holds(ms.target):
                raise RouteError("target does not hold after the hint")
            return h
        except (RouteError, ControlError) as e:
            self.search.log.append(f"  hint failed: {str(e)[:120]}")
            try:
                h.b.quit()
            except ControlError:
                pass
            return None

    def step(self, ex: Executor, ms: Milestone, search: str = "always") -> Outcome | None:
        """The faster of the hint and the search from ``ex``; ``ex`` itself is left as it was.
        ``search``: "always" (try to beat a working hint too), "fallback" (only when the hint
        fails) or "never"."""
        # Already reached only if no hint move has work left, as the executor judges a move done
        # (its `after` and `until` both hold): a hint can carry a move that is no milestone itself
        # (S6's 'Esc the book' before a no-op head put-away), and a target can hold before its event
        # (Stan's var 32 is 4 before the fight sets 129 and the wake-up key sets 4 again).
        if ex.b.holds(ms.target) and ex.b.holds(ms.until or ms.target) and \
                all(ex.b.holds(m.after) and ex.b.holds(m.until) for m in ms.hint if m.until):
            return Outcome(ex, "already", [], ex.b.status["qf"])
        hint = self._hint(ex, ms)
        bound = hint.b.status["qf"] if hint else None
        run_search = search == "always" or (search == "fallback" and hint is None)
        found = self.search.reach(ex, ms.target, bound=bound) if run_search else None
        if found is not None and hint is not None and found.ex is not ex and self.keep_vars:
            hv, fv = hint.b.state()["vars"], found.ex.b.state()["vars"]
            if any(hv[v] != fv[v] for v in self.keep_vars):
                self.search.log.append(f"  search branch dropped: vars {sorted(self.keep_vars)} differ from the hint's")
                found.ex.b.quit()
                found = None
        if found is not None and found.ex is not ex:
            alt = Outcome(hint, "hint", [m.name for m in ms.hint], hint.b.status["qf"]) if hint else None
            return Outcome(found.ex, "search" if hint is None else "search<hint", [m.name for m in found.moves],
                           found.qf, alt)
        if hint:
            return Outcome(hint, "hint", [m.name for m in ms.hint], hint.b.status["qf"])
        return None


def save(out: Path, ex: Executor, records: list[dict]) -> None:
    (out / "movie.jsonl").write_text("".join(json.dumps(e) + "\n" for e in ex.movie))
    (out / "results.jsonl").write_text("".join(json.dumps(r) + "\n" for r in records))
