"""E004 plan step A4 with B1's matcher: the strict window search (window-search.md section 9; window.WindowConfig's
sub-switch ``strict``, off by default).

What it gives window.py (section numbers of window-search.md):

* ``Reading``: the reading as B1's strict matcher and explainer read it (trace_check.Reading's interface), from the
  harness's packed task: the event records by unit and wait point, each action as B1's tuple, the built lists.
* ``ArgsView``: ``args:U`` from the dumped locals, as B1's view has it (9.1).
* ``Plan``: one plan's B1 StrictMatcher over the reading's event records of the plan's units, and which record left
  each event's slot at its wait point (Definition S, Lemma S1).
* ``Track``: one branch's strict matches: each event record of the plan matched or refuted on its own run (9.2).
* ``input_match``: (I1)-(I4) and "took effect" for an input record at s + 1 (9.2, Lemma 9′).
* ``Failure`` and ``explain``: what a child keeps of its first failure, and B1's explanation of it (9.5):
  trace_check.explain_unexplained (Lemmas X and B, the two classes), and Lemma X toward an input record's own end.

It names no game number. Every unit, offset and value it reports comes from the engine's runs and logs and the IR.
"""

import collections
import json
import re
import types

import facts as F
import ir
import model as M
import reader as R
import replay as RP
import trace_explain as TE

_SOURCE = re.compile(rb'^\{"i": (\d+), "rec": -?\d+, "label": "(?:[^"\\\\]|\\\\.)*", "source": (\[[^\]]*\])')
KEEP_FRAMES = 3  # frames of explain entries and runs a branch keeps (an input's record spans R_s and R_{s+1})


# --- the reading as B1 reads it -----------------------------------------------------------------------------------

class Reading:
    """trace_check.Reading's interface over the harness's packed task: ``events`` {(unit, w): [action index]},
    ``rec(i)`` B1's tuple (index, unit, wait point, pre, effects, checks), ``rd.lists`` the built lists, ``doms``."""

    def __init__(self, task, doms: dict, lines_path, lists, unobserved: tuple):
        self.task, self.doms, self.unobserved = task, doms, unobserved
        self.events = collections.defaultdict(list)
        self.by_unit = collections.defaultdict(list)
        with open(lines_path, "rb") as fh:
            for ln in fh:
                m = _SOURCE.match(ln)
                if m is None:
                    continue
                src = json.loads(m[2])
                if src and src[0] == "event" and not (len(src) > 4 and src[4]):
                    i = int(m[1])
                    self.events[(src[1], src[2])].append(i)
                    self.by_unit[src[1]].append(i)
        self.by_name = {a.name: i for i, a in enumerate(task.actions)}
        self.rd = types.SimpleNamespace(lists=lists)
        self._recs: dict = {}

    def rec(self, i: int) -> tuple:
        """B1's (trace_check.Reading.rec): the action's precondition and effects in the task's values, the facts in
        ``unobserved`` left out."""
        if i not in self._recs:
            a = self.task.actions[i]
            w = a.what
            src = w.source
            unit, wp = (src[1], src[2]) if src[0] == "event" else (None, None)
            eff = {}
            for k, e in w.effects.items():
                if k.startswith(self.unobserved):
                    continue
                if e[0] == "set":
                    eff[k] = ("set", e[1])
                elif e[0] == "guess":
                    eff[k] = ("guess", frozenset(e[1]))
                else:
                    eff[k] = (e[0],)
            pre = {k: vs for k, vs in a.pre.items() if not k.startswith(self.unobserved)}
            self._recs[i] = (i, unit, wp, pre, eff, list(w.checks))
        return self._recs[i]


class ArgsView:
    """``args:U`` for the units ``units`` (9.1): the built lists at U's wait point that U's dumped locals match, {0}
    when none does or U has no live slot at one of the scope's points (trace_check.Reading.view)."""

    def __init__(self, b, lists: dict | None, units):
        self.b, self.lists, self.units = b, lists or {}, sorted(units)
        self.cache: dict = {}

    def __call__(self, dump: dict, view: dict) -> dict:
        ds = RP.dumped_slots(dump) if self.units else None
        slots = ds["slots"] if ds is not None else {}
        out = {}
        for u in self.units:
            k = f"args:{u}"
            pos = view.get(f"pos:{u}")
            slot = slots.get(u)
            if slot is None or not isinstance(pos, int) or pos < 0:
                out[k] = frozenset({0})
                continue
            key = (u, pos, tuple(slot["locals"]), slot["override"], tuple(slot["cut_data"]),
                   json.dumps(slot.get("flags"), sort_keys=True))
            sigs = self.cache.get(key)
            if sigs is None:
                live = RP.live_match(self.b, slot)
                sigs = frozenset(lst["sig"] for lst in self.lists.get((u, pos), []) if RP.list_matches(lst, slot, live))
                sigs = sigs or frozenset({0})
                self.cache[key] = sigs
            out[k] = sigs
        return out


# --- one plan ---------------------------------------------------------------------------------------------------------

def is_stutter(code: TE.Code, r: TE.Run) -> bool:
    """trace-validation.md section 3: one opcode, a WaitFor op, and the slot back at its start: a blocked wait."""
    if r.ops != 1 or r.code1 != r.code0:
        return False
    key = code.key(r.where, r.number, r.room)
    i = code.index(key, r.code0) if key is not None else None
    return i is not None and code.unit(key).ins[i].yields == "wait"


def open_keys_of(runs: list, before: dict, after: dict) -> frozenset:
    """trace-validation.md section 3, the open keys: with work after the earlier decision point, the facts that changed
    between the two decision points and the slot facts of the units that ran."""
    if not any(r.after for r in runs):
        return frozenset()
    moved = {k for k in before.keys() | after.keys() if before.get(k) != after.get(k)}
    units = {r.unit for r in runs}
    return frozenset(moved | {f"{p}:{u}" for u in units for p in ("pos", "args", "local", "script", "frozen",
                                                                   "objscript")})


class Plan:
    """One plan's strict parts: ``matcher`` (B1's StrictMatcher over the reading's event records of the plan's event
    units), the action indices of each step's paths, and per step the record before it in plan order that writes the
    ``pos:`` of its unit (the one that left its slot at its wait point, Definition S)."""

    def __init__(self, sr: Reading, recs: list, code: TE.Code, input_units: dict):
        self.sr, self.recs, self.code = sr, recs, code
        self.units = sorted({r.unit for r in recs if r.kind == "event" and r.unit})
        self.idx = {r.i: [sr.by_name.get(a.name) for a in r.paths] for r in recs}
        records = [sr.rec(i) for u in self.units for i in sr.by_unit.get(u, ())]
        self.matcher = TE.StrictMatcher(records, R.NOWHERE)
        self.unit_set = frozenset(self.units)
        self.prev_writer = {}
        for r in recs:
            if r.kind != "event":
                continue
            key = f"pos:{r.unit}"
            prev = [q for q in recs if q.i < r.i and key in q.writes]
            self.prev_writer[r.i] = prev[-1].i if prev else None
        self.input_units = input_units  # MRec.i -> [units its pos: writes name]
        self.args_units = sorted({k[5:] for r in recs for k in r.reads | r.writes if k.startswith("args:")})
        self.events = [r for r in recs if r.kind == "event"]


# --- one branch ---------------------------------------------------------------------------------------------------

class Track:
    """One branch's strict matches (9.2): ``matched`` {MRec.i: (frame, path, seq of its run)}, ``refuted`` {MRec.i:
    (frame, why, Run)}; seq -1: matched before this branch could see (done before the window, or in the plan's earlier
    windows on a run the branch replays)."""

    def __init__(self, plan: Plan, done: dict):
        self.plan = plan
        self.matched = dict(done)
        self.refuted: dict = {}
        self.choices: dict = {}  # MRec.i -> [(key, observed)]: a guessed write the engine took otherwise

    def copy(self) -> "Track":
        t = Track(self.plan, self.matched)
        t.refuted, t.choices = dict(self.refuted), dict(self.choices)
        return t

    def armed(self, r) -> int | None:
        """The seq after which a run of r's unit from its wait point is r's own (Definition S), None while the record
        that leaves r's slot there has not matched."""
        prev = self.plan.prev_writer.get(r.i)
        if prev is None:
            return -1
        m = self.matched.get(prev)
        return m[2] if m is not None else None

    def verdicts(self, runs: list, before: dict, after: dict, dump_before: dict | None) -> list:
        """B1's verdicts on R_k's runs of the plan's units at depth 0 from a wait point, stutters left out: [(Run,
        Verdict)] in run order."""
        plan = self.plan
        mine = [r for r in runs if r.depth == 0 and r.code0 >= 0 and r.unit in plan.unit_set
                and not is_stutter(plan.code, r)]
        if not mine:
            return []
        m = _MatcherView(plan.matcher, dump_before)
        vs = m.match(mine, before, after, open_keys_of(runs, before, after))
        return [(v.run, v) for v in vs]

    def apply(self, k: int, run: TE.Run, v, after: dict) -> list:
        """Definition S for one run: the plan's event record whose own run it is, matched or refuted. Returns [(MRec,
        "matched"|"refuted")]."""
        out = []
        for r in self.plan.events:
            if r.i in self.matched or r.i in self.refuted or r.unit != run.unit or r.wait != run.code0:
                continue
            at = self.armed(r)
            if at is None or run.seq <= at:
                continue
            ok = v.record if v.status == "explained" else (v.record or []) if v.status == "ambiguous" else []
            ok = {ok} if isinstance(ok, int) else set(ok)
            idx = self.plan.idx[r.i]
            path = next((n for n, i in enumerate(idx) if i in ok), None)
            late = [p for p in r.preds if p in self.plan.idx and
                    (p not in self.matched or self.matched[p][2] >= run.seq)]
            if path is not None and not late:
                self.matched[r.i] = (k, path, run.seq)
                ch = [(kk, after.get(kk)) for kk, x in r.exps[path].items()
                      if x.kind == "guess" and x.chosen is not None and after.get(kk) in x.values
                      and after.get(kk) != x.chosen]
                if ch:
                    self.choices[r.i] = ch
                out.append((r, "matched"))
            else:
                if path is not None:
                    why = "order: a record it follows had not matched"
                else:
                    mine = [w for i, w in v.why if i in idx]
                    why = mine[0] if mine else f"end pos:{r.unit}"
                self.refuted[r.i] = (k, why, run)
                out.append((r, "refuted"))
            break  # a run is the own run of one record at most (the records of a unit form a chain)
        return out


class _MatcherView:
    """The plan's StrictMatcher with this branch's check decisions (M3 on the dump before the runs); the index is
    shared, the counters are the branch's."""

    def __init__(self, m: TE.StrictMatcher, dump_before):
        self.__dict__.update(m.__dict__)
        self.calls, self.seconds, self.open_used = 0, 0.0, 0

        def decide(check, run):
            if dump_before is None:
                return None
            return RP.eval_check(check, types.SimpleNamespace(kind="event", source=("event",)), dump_before)
        self.decide_check = decide

    match = TE.StrictMatcher.match
    _value_ok = staticmethod(TE.StrictMatcher._value_ok)


# --- an input record at s + 1 ------------------------------------------------------------------------------------------

def input_match(sr: Reading, plan: Plan, idx: int, units: list, v0: dict, d0: dict | None, applied: dict,
                own: dict, vk: dict, later: list) -> dict:
    """9.2's (I1)-(I4) and "took effect" for the reading's action ``idx``, an input record whose ``pos:`` writes name
    ``units``: v0 the view before its first click (V_{f_1−1}), d0 the dump there, ``applied`` the set writes of the
    event records matched before its first run, ``own`` its runs (unit -> the unit's first run at depth 1 or more from
    the click's frame on), vk the view at the frame it is decided, ``later`` the runs of that frame after its last run.
    Returns {"ok", "took", "why", "run", "engine", "want", "first_seq", "last_seq"}: why the first condition that
    failed, run the click's run of the unit it names."""
    _, _, _, pre, eff, checks = sr.rec(idx)
    state = {**v0, **applied} if applied else v0
    bad = next((k for k, vs in pre.items() if not TE._holds(state.get(k), vs)), None)
    if bad is not None:
        return {"ok": False, "took": False, "why": f"pre {bad}", "run": None, "engine": _show(state.get(bad))}
    changed = False
    for k, e in eff.items():
        if not k.startswith("pos:") or e[0] != "set":
            continue
        r = own.get(k[4:])
        if r is None:
            if e[1] == R.NOWHERE:  # a slot the click stops, or leaves dead: the view says
                if vk.get(k) != R.NOWHERE:
                    return {"ok": False, "took": False, "why": f"end {k}", "run": None, "engine": vk.get(k),
                            "want": e[1]}
                changed = changed or v0.get(k) != R.NOWHERE
                continue
            return {"ok": False, "took": False, "why": f"no run {k}", "run": None, "engine": None, "want": e[1]}
        end = r.code1 if r.code1 >= 0 else R.NOWHERE
        if end != e[1]:
            return {"ok": False, "took": False, "why": f"end {k}", "run": r, "engine": end, "want": e[1]}
        if v0.get(k) != end:
            changed = True
    seqs = [r.seq for r in own.values()]
    mine = sorted(own.values(), key=lambda r: r.seq)
    for k, e in eff.items():
        if k.startswith("pos:") or e[0] not in ("set", "guess") or k not in vk:
            continue
        if not TE.StrictMatcher._value_ok(e, vk[k]):
            if any(k in plan.matcher.keys.get((r.unit, r.code0, r.code1 if r.code1 >= 0 else R.NOWHERE), ()) or
                   k in plan.matcher.keys.get((r.unit, r.code0, None), ()) for r in later):
                continue
            return {"ok": False, "took": False, "why": f"effect {k}", "run": mine[-1] if mine else None,
                    "engine": _show(vk[k])}
        if not TE.StrictMatcher._value_ok(e, v0.get(k)):
            changed = True
    if d0 is not None:
        for c in checks:
            if RP.eval_check(c, sr.task.actions[idx].what, d0) is False:
                return {"ok": False, "took": False, "why": f"check {c[0]}", "run": None, "engine": None}
    return {"ok": True, "took": changed, "why": None, "run": None, "first_seq": min(seqs, default=-1),
            "last_seq": max(seqs, default=-1)}


def _show(v):
    if isinstance(v, frozenset):
        return sorted(v)[:4]
    return v


# --- a failure and its cause (9.5) -----------------------------------------------------------------------------------

class Kept:
    """A branch's last ``KEEP_FRAMES`` frames: its runs, explain entries, dumps and views, for a failure's cause."""

    def __init__(self):
        self.runs: collections.OrderedDict = collections.OrderedDict()  # frame -> [Run] (R_k)
        self.entries: list = []  # explain entries
        self.seq_frame: dict = {}
        self.dumps: dict = {}
        self.views: dict = {}

    def add_entries(self, entries: list) -> None:
        for e in entries:
            if e[0] == "b":
                self.seq_frame[e[1]] = e[2]
        self.entries.extend(entries)

    def frame(self, k: int, runs: list, dump: dict | None, view: dict | None) -> None:
        self.runs[k] = runs
        if dump is not None:
            self.dumps[k] = dump
        if view is not None:
            self.views[k] = view
        lo = k - KEEP_FRAMES
        for d in (self.runs, self.dumps, self.views):
            for f in [f for f in d if f < lo]:
                del d[f]
        if len(self.entries) > 4096:
            keep_seq = {s for s, f in self.seq_frame.items() if f >= lo - 1}
            self.entries = [e for e in self.entries if e[1] in keep_seq]
            self.seq_frame = {s: f for s, f in self.seq_frame.items() if s in keep_seq}

    def freeze(self, extra_views: dict | None = None, extra_dumps: dict | None = None) -> dict:
        return {"runs": {k: list(v) for k, v in self.runs.items()}, "entries": list(self.entries),
                "dumps": {**self.dumps, **(extra_dumps or {})}, "views": {**self.views, **(extra_views or {})}}


class _Eng:
    """trace_check.WindowEngine's ``explain`` over a branch's kept explain log (the units were armed beforehand)."""

    def __init__(self, entries: list):
        self.ex = TE.Explained(entries)

    def explain(self, k, units):
        return self.ex


def explain(code: TE.Code, sr: Reading, fail: dict) -> dict:
    """B1's explanation of one failure (9.5): {"cause": True|None, "kind", "unit", "frame", "offset", "opcode",
    "values", "class", ...}."""
    import trace_check as TC  # the explainer of plan step B1
    kind, why, k = fail["kind"], fail.get("why") or "", fail["frame"]
    mat = fail["material"]
    views, dumps = mat["views"], mat["dumps"]
    eng = _Eng(mat["entries"])
    run = fail.get("run")
    base = {"failure": kind, "record": fail.get("record"), "frame": k, "why": why}
    try:
        if kind in ("refuted", "mismatch") and run is not None and why.split(" ")[0] in ("pre", "end", "effect"):
            if k - 1 not in views or k - 1 not in dumps:
                return base | {"cause": None, "kind": "the frame before is not kept"}
            views = dict(views)
            views.setdefault(k, fail.get("view_after") or {})
            return base | TC.explain_unexplained(code, eng, k, run, why, sr, views, dumps)
        if kind == "input":
            if why.startswith("end pos:") and run is not None:
                return base | input_end_cause(code, eng, run, fail["want"], dumps.get(run.frame - 1 if not run.after
                                                                                   else run.frame))
            if why.startswith("effect") and run is not None:
                kk = run.frame + 1 if run.after else run.frame
                views = dict(views)
                if kk - 1 not in views or kk not in views:
                    return base | {"cause": None, "kind": "the frames are not kept"}
                return base | TC.explain_unexplained(code, eng, kk, run, why, sr, views, dumps)
            if why.startswith("pre"):
                fk = why.split(" ", 1)[1]
                vals = [TE.Value(fk, fail.get("engine"), "the view before the click; the record asks "
                                 f"{sorted(map(str, fail.get('asks') or []))[:6]}", "fact")]
                return base | {"cause": True, "kind": "the input's precondition fails before its first click",
                               "unit": None, "offset": None, "opcode": None, "values": [x.as_dict() for x in vals],
                               "class": None}
            return base | {"cause": None, "kind": why}
        if kind == "not run":
            return base | not_run_cause(code, eng, fail)
        if kind == "choice" and run is not None:
            kk = run.frame + 1 if run.after else run.frame
            return base | TC.explain_unexplained(code, eng, kk, run, why, sr, views, dumps)
        if kind == "goal writer" and run is not None:
            return base | goal_writer_cause(code, eng, run, fail["fact"], dumps)
    except Exception as e:  # noqa: BLE001 (a cause the explainer cannot give is reported as such)
        return base | {"cause": None, "kind": f"explainer error: {type(e).__name__}: {e}"[:300]}
    return base | {"cause": None, "kind": f"no rule for {kind} {why}"}


def _logged(eng: _Eng, run: TE.Run):
    return next((r for r in eng.ex.runs.values() if r.frame == run.frame and r.after == run.after and
                 r.code0 == run.code0 and r.where == run.where and r.number == run.number), None)


def input_end_cause(code: TE.Code, eng: _Eng, run: TE.Run, want: int, dump: dict | None) -> dict:
    """Lemma X toward an input record's own end (9.5): the first opcode where the click's run left the path to the
    record's ``pos:`` end ``want``; trace_check.explain_unexplained's "end" rule with the input record's end."""
    key = code.key(run.where, run.number, run.room)
    if key is None:
        return {"cause": None, "kind": "unit not in the IR"}
    lg = _logged(eng, run)
    if lg is None:
        return {"cause": None, "kind": "the run is not in the explain log", "unit": run.unit}
    steps = TE.steps_of(code, key, [lg])
    targets = frozenset(code.end_sites(key) if want == R.NOWHERE else code.yield_sites(key, want))
    base = {"unit": run.unit, "engine_end": run.code1, "record_end": want}
    n = TE.first_off(code, key, steps, targets, cross_yields=False)
    if n is not None:
        x = code.unit(key).ins[steps[n].i]
        if x.yields == "wait":
            vals = wait_values_read(x, steps[n], dump)
            return base | {"cause": True, "kind": "the wait did not block", "offset": steps[n].op.code,
                           "opcode": ir.head(x.st), "values": [v.as_dict() for v in vals], "class": "split"}
        vals = TE.slice_test(code, key, steps, n, dump)
        return base | {"cause": True, "kind": "left the record's path", "offset": steps[n].op.code,
                       "opcode": ir.head(x.st), "values": [v.as_dict() for v in vals], "class": TE.cause_class(vals)}
    return base | {"cause": None, "kind": "no branch off the record's path found", "steps": len(steps)}


def wait_values_read(x, step, dump: dict | None) -> list:
    """TE.wait_values, with a WaitForActor's actor read from the op's own logged reads when the IR names it through
    a variable or a local (Lemma B: the value readVar returned while the op ran)."""
    if isinstance(x.st, ir.Op) and x.st.name == "WaitForActor" and step is not None:
        args = []
        for a in x.st.args:
            vid = TE.var_id(a)
            got = next((v for var, v in step.op.reads if var == vid), None) if vid is not None else None
            args.append(ir.Const(got) if got is not None else a)
        return [TE.getter_value(ir.Getter("getActorMoving", tuple(args)), dump)]
    return TE.wait_values(x, 0, dump)


def not_run_cause(code: TE.Code, eng: _Eng, fail: dict) -> dict:
    """A needed record whose own run did not happen before the child stopped (9.5): where its unit's slot waits, the
    polling loop's test or the wait it is parked at (trace_check.Explainer.follow_waiting's rule)."""
    unit, dump = fail["unit"], fail["material"]["dumps"].get(fail["frame"])
    runs = [r for rs in fail["material"]["runs"].values() for r in rs if r.unit == unit]
    if dump is not None:
        sl = next((s for s in dump["slots"] if F.unit_of(s["where"], s["number"]) == unit), None)
        if sl is None:
            return {"cause": True, "kind": "the unit has no live slot", "unit": unit, "offset": None,
                    "opcode": None, "values": [], "class": "semantics"}
    if not runs:
        return {"cause": None, "kind": "the unit did not run in the kept frames", "unit": unit}
    last = runs[-1]
    key = code.key(last.where, last.number, last.room)
    if key is None or last.code1 < 0:
        return {"cause": None, "kind": "its last run ended the slot", "unit": unit}
    lg = _logged(eng, last)
    steps = TE.steps_of(code, key, [lg]) if lg is not None else []
    m = TE.last_test(code, key, steps) if steps else None
    d = fail["material"]["dumps"].get(last.frame - 1 if not last.after else last.frame)
    if m is not None:
        vals = TE.slice_test(code, key, steps, m, d)
        x = code.unit(key).ins[steps[m].i]
        return {"cause": True, "kind": "not there yet: the slot polls", "unit": unit, "offset": steps[m].op.code,
                "opcode": ir.head(x.st), "values": [v.as_dict() for v in vals],
                "class": TE.cause_class(vals) if vals else "split", "waits_at": last.code1}
    ys = sorted(code.yield_sites(key, last.code1))
    x = code.unit(key).ins[ys[0]] if ys else None
    vals = TE.wait_values(x, last.delay, d) if x is not None else []
    return {"cause": True, "kind": "not there yet: the slot waits", "unit": unit,
            "offset": x.offset if x is not None else last.code1, "opcode": ir.head(x.st) if x is not None else None,
            "values": [v.as_dict() for v in vals], "class": "split", "waits_at": last.code1}


def goal_writer_cause(code: TE.Code, eng: _Eng, run: TE.Run, fact: str, dumps: dict) -> dict:
    """The run that wrote a goal fact no goal record of the plan predicted (9.5): its write site and the last test the
    run made before it (Lemma B on that test)."""
    key = code.key(run.where, run.number, run.room)
    lg = _logged(eng, run)
    if key is None or lg is None:
        return {"cause": None, "kind": "the writing run is not in the explain log", "unit": run.unit}
    steps = TE.steps_of(code, key, [lg])
    sites = code.write_sites(key, fact)
    n = next((j for j, s in enumerate(steps) if s.i in sites), None)
    d = dumps.get(run.frame - 1 if not run.after else run.frame)
    if n is None:
        return {"cause": None, "kind": "no write site of the fact in the run", "unit": run.unit}
    m = TE.last_test(code, key, steps[:n])
    vals = TE.slice_test(code, key, steps, m, d) if m is not None else []
    x = code.unit(key).ins[steps[n].i]
    return {"cause": True, "kind": "a unit the plan does not name wrote the goal's fact", "unit": run.unit,
            "offset": steps[n].op.code, "opcode": ir.head(x.st), "values": [v.as_dict() for v in vals],
            "test_offset": steps[m].op.code if m is not None else None,
            "class": TE.cause_class(vals) if vals else None}


def short(c: dict | None) -> str:
    """One line of a cause for the log."""
    if not c:
        return "no cause"
    vals = "; ".join(f"{v['what']} = {v['value']}" + (f" ({v['kind']})" if v.get("kind") not in (None, "fact") else "")
                     for v in (c.get("values") or [])[:3])
    off = c.get("offset")
    return (f"{c.get('failure')} {c.get('record') or ''} at {c.get('frame')}: {c.get('why')}: {c.get('kind')}; "
            f"{c.get('unit') or ''} [{off:04X}] {c.get('opcode') or ''}; {vals}; class {c.get('class')}"
            if isinstance(off, int) else
            f"{c.get('failure')} {c.get('record') or ''} at {c.get('frame')}: {c.get('why')}: {c.get('kind')}; "
            f"{c.get('unit') or ''} {c.get('opcode') or ''}; {vals}; class {c.get('class')}")


def runs_of(rows: list) -> list:
    return [TE.Run(*r) for r in rows]


def by_frame(runs: list) -> dict:
    """Runs grouped as R_k: frame k's runs before its decision point and frame k − 1's after it."""
    out = collections.defaultdict(list)
    for r in runs:
        out[r.frame + 1 if r.after else r.frame].append(r)
    for v in out.values():
        v.sort(key=lambda r: r.seq)
    return out


def unit_where(unit: str) -> list:
    """[where, number] pairs the bridge's explain takes for a unit name (an object's script can run from the room or
    the inventory)."""
    kind, _, n = unit.partition(":")
    if kind == "script":
        return [[F.WIO_GLOBAL, int(n)]]
    if kind == "local":
        return [[F.WIO_LOCAL, int(n)]]
    if kind == "obj":
        return [[w, int(n)] for w in (F.WIO_ROOM, F.WIO_INVENTORY, F.WIO_FLOBJECT)]
    if kind in ("entry", "exit"):
        return [[F.WIO_ROOM, F.ENCD if kind == "entry" else F.EXCD]]
    return []


__all__ = ["Reading", "ArgsView", "Plan", "Track", "input_match", "Kept", "explain", "short", "runs_of", "by_frame",
           "unit_where", "M"]
