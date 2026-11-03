"""E011 play: the fixtures of fmi-play's differential tests (experiments/E011-speed/plan.md, "play", test 1).

usage (from experiments/E008-go-explore, with ~/.cache/fast-mi-e009/env.sh sourced, under the guard and nice):
       $PY ../E011-speed/play/capture.py OUT_DIR

Runs E009's step test (a) (one job from the root on rooms 28, 33, 41 to owner:566 = 1, step.py's E009 wiring: the
strict window search with 3 workers, cache, fork-each 7, lite and split-wait) in this process, as off_a.py runs it,
with the window search wrapped by monkeypatching. No file of the reference is edited. It writes, under OUT_DIR:

* ``step-a/``: the job (job.json, result.json, run/a00/windows.jsonl, the reading, the plans);
* ``plans.jsonl``: per ``Search.play_plan`` call, its inputs (the plan's steps, the step actions and their choices,
  the harness's prediction and kept values, the goal, the engine's frame and its movie from boot) and its outputs
  (the outcome, the replan cause, the checkpoints and inputs it added, the prediction after);
* ``windows.jsonl``: per ``Search.window_strict`` call, its inputs (the plan's records as the search tracks them, the
  macro step, the tracked and needed records, the parent's frame and movie, the strict matches carried in) and its
  outputs (the window's record, the chosen child with its inputs, the cause, the parent's strict matches);
* ``children.jsonl``: per ``Search._child_strict`` call, the child's fork frame, its inputs from the parent, its
  outcome, first and last input frames, end, Q and why, and every read of the window's best (the child's frame and
  Q then, the best's fork frame, end Q and outcome) and of the first-event table, with times;
* ``stops.jsonl``: every ``strict_stop`` call of a parent (its Q, the best's Q and kind, the answer), with times;
* ``check.txt``: the CHECK (a) line, and the verify from boot of the goal entry.

The engine, the reader, taskcore and Fast Downward are the reference's (env.sh). Seed 1.
"""

import dataclasses
import json
import os
import sys
import threading
import time
from pathlib import Path

OUTD = Path(sys.argv[1]).resolve()
STEP_OUT = OUTD / "step-a"
os.environ["E008_STEP_OUT"] = str(STEP_OUT)
HERE = Path.cwd()  # experiments/E008-go-explore
sys.path.insert(0, str(HERE))

import test_step as T  # noqa: E402
import step  # noqa: E402
import replay  # noqa: E402
import replay_part1 as P  # noqa: E402
import window as WS  # noqa: E402
from facts import facts_of  # noqa: E402

assert T.OUT == STEP_OUT, T.OUT
OUTD.mkdir(parents=True, exist_ok=True)
LOCK = threading.Lock()
FILES = {n: open(OUTD / f"{n}.jsonl", "w") for n in ("plans", "windows", "children", "stops")}
CUR = {"window": -1}
T0 = time.monotonic()


def now() -> float:
    return round(time.monotonic() - T0, 6)


def put(name: str, doc: dict) -> None:
    line = json.dumps(doc, default=_default) + "\n"
    with LOCK:
        FILES[name].write(line)
        FILES[name].flush()


def _default(o):
    if isinstance(o, (set, frozenset)):
        return {"$set": sorted((_plain(x) for x in o), key=str)}
    if dataclasses.is_dataclass(o) and not isinstance(o, type):
        return {"$" + type(o).__name__: _plain(dataclasses.asdict(o))}
    if isinstance(o, tuple):
        return list(o)
    return str(o)


def _plain(o):
    if isinstance(o, dict):
        return {str(k): _plain(v) for k, v in o.items()}
    if isinstance(o, (list, tuple)):
        return [_plain(x) for x in o]
    if isinstance(o, (set, frozenset)):
        return {"$set": sorted((_plain(x) for x in o), key=str)}
    if dataclasses.is_dataclass(o) and not isinstance(o, type):
        return {"$" + type(o).__name__: _plain(dataclasses.asdict(o))}
    return o


def vals(vs) -> list:
    return sorted(vs, key=str)


def view(d: dict | None):
    """A view or a belief: key -> value, a frozenset (args:) marked, None kept."""
    if d is None:
        return None
    return [[k, {"$set": sorted(v)} if isinstance(v, frozenset) else v] for k, v in d.items()]


_BY_NAME: dict = {}


def by_name(task) -> dict:
    key = id(task)
    if key not in _BY_NAME:
        _BY_NAME[key] = {a.name: i for i, a in enumerate(task.actions)}
    return _BY_NAME[key]


def action(a, names: dict) -> dict:
    return {"name": a.name, "pre": [[k, vals(vs)] for k, vs in a.pre.items()],
            "effects": [{"key": e.key, "value": e.value, "add": e.add,
                         "values": vals(e.values) if e.values is not None else None,
                         "when": [[k, vals(vs)] for k, vs in e.when]} for e in a.effects],
            "what": names.get(a.name.split("~ban:")[0]), "cost": a.cost}


def step_doc(s) -> dict:
    return {"op": s.op, "action": s.action, "name": s.name, "chosen": [list(c) for c in s.chosen],
            "guessed": [[k, v, [[c, vals(vs)] for c, vs in w]] for k, v, w in s.guessed]}


def expect(x) -> dict:
    return {"values": vals(x.values), "kind": x.kind, "chosen": x.chosen, "changes": x.changes}


def mrec(r) -> dict:
    return {"i": r.i, "name": r.name, "kind": r.kind, "unit": r.unit, "wait": r.wait, "cost": r.cost,
            "paths": [a.name for a in r.paths], "reads": sorted(r.reads), "writes": sorted(r.writes),
            "exps": [[[k, expect(x)] for k, x in e.items()] for e in r.exps],
            "pres": [[[k, vals(vs)] for k, vs in p.items()] for p in r.pres], "before": view(r.before),
            "preds": sorted(r.preds), "macro": r.macro, "done": r.done, "done_frame": r.done_frame, "path": r.path,
            "checked": sorted(r.checked) if r.checked is not None else None}


def track(t) -> dict | None:
    if t is None:
        return None
    return {"matched": {str(i): [v[0], v[1], v[2]] for i, v in t.matched.items()},
            "refuted": {str(i): [v[0], v[1], _plain(v[2])] for i, v in t.refuted.items()},
            "choices": {str(i): _plain(v) for i, v in t.choices.items()},
            "unit_seq": {str(i): v for i, v in t.unit_seq.items()},
            "first_seq": {str(i): v for i, v in t.first_seq.items()},
            "outside": {str(i): _plain(v) for i, v in t.outside.items()}}


def movie(h) -> list:
    return list(h.ex.movie)


# --- the wrappers ---------------------------------------------------------------------------------------------------

_play_plan = WS.Search.play_plan


def play_plan(self, n_plan, steps):
    h = self.h
    names = by_name(h.base)
    doc = {"n_plan": n_plan, "t0": now(), "frame": h.branch.frame, "qf": h.branch.status.get("qf"),
           "movie": movie(h), "steps": [step_doc(s) for s in steps],
           "actions": [action(a, names) for a in h.task.actions],
           "choices": {str(i): [action(a, names) for a in ps] for i, ps in h.choices.items()},
           "pred": view(h.pred), "kept": view(h.kept), "goal": [[k, vals(vs)] for k, vs in h.goal.items()],
           "unobserved": list(h.unobserved), "checkpoints_before": len(h.checkpoints),
           "reading": str(Path(h.base.lines.path).parent), "doms": len(h.doms)}
    n_cp, n_movie = len(h.checkpoints), len(h.ex.movie)
    out = _play_plan(self, n_plan, steps)
    doc.update(out=out, t1=now(), replan_cause=self.replan_cause, detail=_plain(self.replan_cause_detail),
               checkpoints=[[cp[0], sorted(cp[1].items()), cp[2]] for cp in h.checkpoints[n_cp:]],
               pending_cp=[self.pending_cp[0][0], self.pending_cp[1]] if self.pending_cp else None,
               movie_added=list(h.ex.movie[n_movie:]), pred_after=view(h.pred), kept_after=view(h.kept),
               stop_reason=h.stop_reason, branch_frame=h.branch.frame, branch_qf=h.branch.status.get("qf"),
               goal_credited=self.goal_credited, causes=_plain(self.causes),
               empty_keys=[[list(k), n] for k, n in self.empty_keys.items()],
               unplanned_goals=_plain(self.unplanned_goals), deviations=_plain(self.deviations))
    put("plans", doc)
    return out


_window_strict = WS.Search.window_strict


def window_strict(self, recs, members, tracked, needed, parent, plan_no, k):
    h = self.h
    with LOCK:
        CUR["window"] += 1
        wi = CUR["window"]
    doc = {"wi": wi, "t0": now(), "plan": plan_no, "k": k, "members": list(members),
           "tracked": [r.i for r in tracked], "needed": sorted(needed), "start": parent.frame,
           "start_qf": parent.status.get("qf"), "movie": movie(h), "recs": [mrec(r) for r in recs],
           "strack": track(self.strack), "goal_writers": sorted(self.goal_writers), "kept": view(h.kept),
           "pred": view(h.pred), "splan_idx": {str(i): v for i, v in self.splan.idx.items()}}
    res = _window_strict(self, recs, members, tracked, needed, parent, plan_no, k)
    best = res["best"]
    doc.update(t1=now(), rec=res["rec"], cause=_plain(res["cause"]), track=track(res["track"]),
               parent_matched={str(i): list(v) for i, v in res["parent_matched"].items()},
               end=parent.frame,
               best=None if best is None else {
                   "g": best.g, "q0": best.q0, "first": best.first, "issue": best.issue, "end": best.end,
                   "q_end": best.q_end, "outcome": best.outcome, "took": best.took, "alt": best.alt,
                   "events": best.events, "dev_stop": best.dev_stop, "arrival": best.arrival,
                   "matched": {str(i): list(v) for i, v in best.matched.items()},
                   "choices": _plain(best.choices), "track": track(best.track),
                   "track_issue": track(best.track_issue), "checkpoints": [
                       [cp[0], sorted(cp[1].items()), cp[2]] for cp in best.checkpoints],
                   "unplanned": best.unplanned, "fail": _plain({k_: v for k_, v in (best.fail or {}).items()
                                                                 if k_ != "material"}) if best.fail else None})
    put("windows", doc)
    return res


class _WProxy:
    """The window's shared dict as one child sees it: every read of the best and of the first-event table logged with
    the child's frame and Q at the time."""

    def __init__(self, w, log, cb):
        self._w, self._log, self._cb = w, log, cb

    def __getitem__(self, k):
        v = self._w[k]
        if k == "best":
            st = self._cb.status
            self._log.append(["best", st.get("frame"), st.get("qf"), None if v is None else v.g,
                              None if v is None else v.q_end, None if v is None else v.outcome, now()])
        elif k == "first_g":
            self._log.append(["first_g", self._cb.status.get("frame"), sorted(v.items()), now()])
        return v

    def __setitem__(self, k, v):
        self._w[k] = v

    def get(self, k, d=None):
        return self._w.get(k, d)

    def __contains__(self, k):
        return k in self._w


_child_strict = WS.Search._child_strict


def child_strict(self, w, cb, ch):
    wi = CUR["window"]
    log = []
    doc = {"wi": wi, "g": ch.g, "q0": ch.q0, "early": ch.early, "done": sorted(ch.done), "t0": now(),
           "track": track(ch.track), "at_fork": view(ch.at_fork)}
    out = _child_strict(self, _WProxy(w, log, cb), cb, ch)
    doc.update(t1=now(), row=ch.row(), outcome=ch.outcome, first=ch.first, issue=ch.issue, end=ch.end,
               q_end=ch.q_end, frames=ch.frames, why=str(ch.why) if ch.why is not None else None,
               why_plain=_plain(ch.why), took=ch.took, alt=ch.alt,
               matched={str(i): list(v) for i, v in ch.matched.items()}, choices=_plain(ch.choices),
               fail_sig=list(self._sig(ch.fail)) if ch.fail is not None else None, events=ch.events,
               dev_stop=ch.dev_stop, arrival=ch.arrival, n_unplanned=len(ch.unplanned), reads=log)
    put("children", doc)
    return out


_strict_stop = WS.strict_stop


def strict_stop(best_q, best_deviation, qf, b_qf, b_dev_qf):
    got = _strict_stop(best_q, best_deviation, qf, b_qf, b_dev_qf)
    put("stops", {"wi": CUR["window"], "best_q": best_q, "dev": best_deviation, "qf": qf, "b_qf": b_qf,
                  "b_dev_qf": b_dev_qf, "stop": got, "t": now()})
    return got


WS.Search.play_plan = play_plan
WS.Search.window_strict = window_strict
WS.Search._child_strict = child_strict
WS.strict_stop = strict_stop


# --- step test (a), as off_a.py runs it, in this process -----------------------------------------------------------

def main() -> None:
    STEP_OUT.mkdir(parents=True, exist_ok=True)
    (STEP_OUT / "GUIDE.json").write_text(json.dumps(dict(T.GUIDE, atoms=[["owner:566", 1, 0]],
                                                         keys={"owner:566": 0})))
    ctl, b, row = P.boot(1, STEP_OUT / "root-boot", dump_locals=True)
    s = b.state()
    root_cp = [b.frame, facts_of(s), replay.extra_of(s, b.status)]
    ctl.close()
    jdir = STEP_OUT / "jobs" / "ta"
    jdir.mkdir(parents=True, exist_ok=True)
    entry = {"id": 0, "root": True, "frame": root_cp[0], "events": [], "checkpoint": root_cp}
    (jdir / "job.json").write_text(json.dumps(T.job_doc("ta", entry, [])))
    t = time.monotonic()
    r = step.run_job(jdir)
    wall = time.monotonic() - t
    lines = T.numbers("a", r, wall)
    a = r["attempt"] or {}
    lines.append(" ".join(map(str, ["CHECK (a):", 0 if r.get("ok") else 1, a.get("stop"), a.get("goal_from_the_plan"),
                                    a.get("frame0"), a.get("frame_end"), a.get("frames"), a.get("qf"),
                                    a.get("replans"), a.get("inputs"),
                                    (a.get("window_summary") or {}).get("children"), len(r["new"])])))
    last = step.read_entry(jdir / r["new"][-1]["file"])
    t = time.monotonic()
    ver = replay.verify(lambda o, i: P.config(1, o, i), last["events"], [root_cp] + last["checkpoints"],
                        ["owner:566"], STEP_OUT / "verify-a")
    lines.append(f"CHECK verify (a) from boot: {len(ver['differ'])} differences at {ver['checkpoints']} checkpoints, "
                 f"{ver['inputs']} inputs, final {ver.get('final')}, {time.monotonic() - t:.1f} s")
    (OUTD / "check.txt").write_text("\n".join(lines) + "\n")
    (OUTD / "goal-entry.json").write_text(json.dumps({"events": last["events"], "root": root_cp,
                                                      "checkpoints": last["checkpoints"]}, default=str) + "\n")
    print("\n".join(lines))
    for f in FILES.values():
        f.close()


if __name__ == "__main__":
    main()
