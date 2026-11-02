"""E008: one job of the explorer (plan.md, "One job"): return to an entry, read the rooms around it, choose targets,
play one step, write the states reached.

usage (from this folder, with ../../.venv/bin/python, under nice and the guard):
       step.py run JOB_DIR     JOB_DIR/job.json in (plan.md, "Interfaces"); JOB_DIR/result.json, JOB_DIR/new/K.json.gz
                               (the entries reached), JOB_DIR/run/ (the boot, each attempt's plans and the readings the
                               harness made) and JOB_DIR/log out. Exit 0 with "ok": true; 3 when the return differs from
                               the entry; 1 on any other error ("ok": false and the error in result.json).

The steps (plan.md, "One job"):

1. **Return.** The root as the harness boots (replay_part1.boot, to the first idle stop); any other entry by
   replay.boot with the entry's movie (its events from boot, frames up to the entry's frame), run to its frame. The
   engine's state against the entry's checkpoint (replay.compare_checkpoint: facts, tick, random state, actors) and
   its frame: a difference ends the job (exit 3), with nothing played. The returned engine is never played on: every
   attempt plays on a fork of it.
2. **Reading.** The job's readings of the scope, newest first (reading.json's modification time): one is used when the
   entry's facts differ from that reading's start facts only on the reading's task facts and on slot keys (``at:``,
   ``script:``, ``local:``, ``objscript:``, ``esc:``), ``reusable``. Else the scope is read from the entry's own
   state: its start row (``start_row``, start-part1.jsonl's format) in ``reading_dir/start.jsonl`` and read-rust with
   read.sh's switches, ``--reach --frame-costs``, ``--stream`` and the stage goal; the golden lines are deleted once
   the files are made. Why, and the reader's numbers, are in result.json.
3. **Targets** (``targets``) from the reading's reached values (reading.reached) and GUIDE.json's atoms: tier 0 the
   stage goal when every goal key's goal value is reached and the entry does not hold the goal; tier 1 the atoms of R
   (or the values of a key with (k, any) in R) the reading reaches at another value than the entry's, held by no
   entry of the archive (the job's ``held``); tier 2 the same, held by some entry; within a tier by (depth, key,
   value); the job's ``tried`` left out; only the guide's kinds (var, bit, owner, state, class, room).
4. **Attempts.** Per target in order: a fork of the returned engine and replay.Harness with replay_part1.cmd_run's
   wiring for walk's run.sh switches plus ``--window-split-wait --refine-facts`` (taskcore served, lama-first-costs,
   cost caps, the reach extension, unobserved actor rooms, approach stones, the strict window search with 3 workers,
   cache, fork-each 7, lite and split-wait, E007's refinement loop; replan limit 30, plan memory 3G; the reading's
   built lists, which cmd_run always hands over and the strict matcher's ``args:`` views read), the target as the
   goal (taskcore ``--goal K=V``; tier 0 the stage goal). A first plan that is not a plan is a probe, and the next
   target is planned; "out of domain" on a cached reading reads the scope fresh once and plans the same target
   again. The first attempt with a plan is played to the harness's own end, and the job ends there.
5. **Out.** One entry per checkpoint the played attempt took past the entry's frame (``entries_from``), the attempt's
   record, the readings the harness made (its last reading when it is not the one it started on), the targets tried,
   whether no target had a plan (``exhausted``), the CPU per part and the peak memory.

**CPU per part** (result.json ``cpu_s``): getrusage of this process and its ended children, plus the engines' own
CPU from /proc where they are still alive. ``return``: the boot, with the returned engine's CPU; ``read``: the fresh
readings and their loading, and the readers of the harness's reach extensions and refinements (their ``cpu_s``);
``play``: the window search's engine and Python CPU (its ``window_summary``); ``plan``: the rest of the attempts (the
harness's Python outside the window search, taskcore, Fast Downward, and the probes); ``total``: the four parts;
``rusage_total``: getrusage over the whole job once every engine and server has ended.

**E009's switches** (experiments/E009-explore-fixed/plan.md; each absent from job.json means E008's behaviour):

* ``idle_frames`` L (fix a): after the return, unless ``idle_measured``, a fork of the returned engine runs L frames
  with no input; a key is idle when its value comes back to one it had before in those frames, the entry's value
  included (``idle_rule``). result.json ``idle``. The fork is quit before the reading; the job plays as before.
* ``rotation`` {"cell": [[k, v, n], ...], "all": [...]} (fix b): the targets are ordered by (tried in the entry's
  cell, tried in all, depth, then E008's order), ``targets(..., rotation=...)``.
* ``reuse`` "e009" (fix c): a cached reading refused by E008's rule is checked again with the keys K of
  ``reuse_keys`` (non-facts of the reading whose start value no site of the reader reads) allowed to differ too.

Seed 1 only (plan.md); any other seed is refused. C2: this file names no game number; the numbers come from the job.
"""

import argparse
import gzip
import json
import os
import resource
import sys
import time
import traceback
from pathlib import Path

HERE = Path(__file__).resolve().parent
E003 = HERE.parent / "E003-path-through-cloud"
sys.path.insert(0, str(E003))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import packed_reading as PR  # noqa: E402
import plan_fd  # noqa: E402
import replay  # noqa: E402
import replay_part1 as P  # noqa: E402
import window as WS  # noqa: E402
from facts import diff, facts_of  # noqa: E402

SEED = 1  # plan.md: seed 1 only
SLOT_PREFIXES = ("at:", "script:", "local:", "objscript:", "esc:")  # the start maps them to pos:/escpos: (plan.md)
GUIDE_KINDS = ("var:", "bit:", "owner:", "state:", "class:")  # and "room" (GUIDE.json's atoms)
SWITCHES = "value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost"  # read.sh's, with wait_pass_cost
REPLAN_LIMIT, TIME_LIMIT, PLAN_TIME, PLAN_MEMORY = 30, 86400, 600, "3G"  # walk's run.sh (E006's limits)
FORK_EACH = 7  # walk's run.sh --window-fork-each
NOT_A_PLAN_RETRY = "out of domain"  # the first plan's status on which a cached reading is read fresh (plan.md)
RESULT_KEYS = ("job", "entry", "ok", "error", "return", "reading", "targets", "probes", "attempt", "readings_made",
               "new", "tried", "exhausted", "cpu_s", "peak_rss_mb", "wall_s")
READING_KEYS = ("path", "fresh", "why", "scope", "records", "actions", "read_wall_s", "read_cpu_s", "peak_rss_mb",
                "files_s")
ATTEMPT_KEYS = ("target", "stop", "reached", "goal_from_the_plan", "frames", "qf0", "qf_end", "replans",
                "refinements", "extensions", "window_summary", "seconds")


# --- the parts a test can call alone ---------------------------------------------------------------------------------

def guide_kind(k: str) -> bool:
    return k == "room" or k.startswith(GUIDE_KINDS)


def guide_atoms(guide_doc: dict) -> dict:
    """GUIDE.json's atoms as {(key, value or None for any): depth}."""
    out = {}
    for k, v, d in guide_doc["atoms"]:
        key = (k, v)
        out[key] = min(d, out.get(key, d))
    return out


def depth_of(atoms: dict, k: str, v) -> int | None:
    """plan.md's rule: the least of the depths of [k, v] and [k, null]; None when neither is in R."""
    ds = [atoms[a] for a in ((k, v), (k, None)) if a in atoms]
    return min(ds) if ds else None


def read_reached(path) -> dict:
    """reading.reached (lines key<TAB>v1<TAB>v2...) as {key: sorted int values}; non-int values (below, above, ...)
    are left out, and a key on several lines gets the union."""
    out: dict = {}
    for ln in Path(path).read_text().splitlines():
        parts = ln.rstrip("\n").split("\t")
        if not parts or not parts[0]:
            continue
        vals = out.setdefault(parts[0], set())
        for x in parts[1:]:
            try:
                vals.add(int(x))
            except ValueError:
                pass
    return {k: sorted(v) for k, v in out.items()}


def goal_text(goal: dict) -> str:
    """The stage goal ({key: [value]}) as taskcore's and reader-scope's ``--goal`` (one value per key)."""
    return P.goal_text_of(goal)


def goal_target(goal: dict) -> tuple:
    """Tier 0's target as [k, v]: the goal's one atom, or ("goal", K=V,K=V) for a goal of several keys."""
    if len(goal) == 1:
        (k, vs), = goal.items()
        return k, list(vs)[0]
    return "goal", goal_text(goal)


def targets(reached_path, guide_doc: dict, entry_facts: dict, root_facts: dict | None = None, held=(), tried=(),
            goal: dict | None = None, rotation: dict | None = None) -> list:
    """Step 3 (module docstring): [(k, v, tier, depth)], tier 0 first, then by (depth, key, value) within a tier.
    ``root_facts``, when given: a value of a key whose only atom is (k, any) at its root value holds no atom of R
    (plan.md's hold rule), so it is no target. ``held`` and ``tried``: [[k, v], ...] (v null: the any atom held).
    ``rotation`` (E009 fix b): {"cell": [[k, v, n], ...], "all": [...]}, the times each target was tried from the
    entry's cell and in all; the same targets are then ordered by (cell, all, depth, tier, key, value)."""
    reached = read_reached(reached_path) if not isinstance(reached_path, dict) else reached_path
    atoms = guide_atoms(guide_doc)
    held_set = {(k, v) for k, v in held}
    tried_set = {(k, v) for k, v in tried}
    out = []
    first = None
    if goal:
        all_reached = all(any(v in reached.get(k, ()) for v in vs) for k, vs in goal.items())
        holds = all(entry_facts.get(k, 0) in vs for k, vs in goal.items())
        if all_reached and not holds:
            first = goal_target(goal)
            if first not in tried_set:
                out.append((first[0], first[1], 0, 0))
    rest = []
    for k, vals in reached.items():
        if not guide_kind(k):
            continue
        now = entry_facts.get(k, 0)
        for v in vals:
            if v == now or (k, v) == first or (k, v) in tried_set:
                continue
            d = depth_of(atoms, k, v)
            if d is None:
                continue
            concrete = (k, v) in atoms
            if not concrete and root_facts is not None and v == root_facts.get(k, 0):
                continue
            is_held = (k, v) in held_set or (not concrete and (k, None) in held_set)
            rest.append((k, v, 2 if is_held else 1, d))
    rest.sort(key=lambda t: (t[2], t[3], t[0], t[1]))
    if rotation is None:
        return out + rest
    return rotate(out + rest, rotation)


def rotate(ts: list, rotation: dict) -> list:
    """E009 fix b: ``ts`` [(k, v, tier, depth)] ordered by (tried in the cell, tried in all, depth, then E008's order
    (tier, depth, key, value)); the counts are [[k, v, n], ...] lists."""
    cell = {(k, v): n for k, v, n in rotation.get("cell") or ()}
    every = {(k, v): n for k, v, n in rotation.get("all") or ()}


    def order(t):
        a = (t[0], t[1])
        return (cell.get(a, 0), every.get(a, 0), t[3], t[2], t[0], (0, t[1], "") if isinstance(t[1], int) else
                (1, 0, str(t[1])))
    return sorted(ts, key=order)


def reusable(reading_doc: dict, task_fact_keys, entry_facts: dict) -> tuple:
    """Step 2's rule: (whether the entry's facts differ from the reading's start facts only on the reading's task
    facts and on slot keys, the number of keys they differ on besides those)."""
    keys = set(task_fact_keys)
    d = diff(reading_doc["start"]["facts"], entry_facts)
    bad = sorted(k for k in d if k not in keys and not k.startswith(SLOT_PREFIXES))
    return not bad, len(bad)


def reusable_detail(reading_doc: dict, task_fact_keys, entry_facts: dict) -> dict:
    keys = set(task_fact_keys)
    d = diff(reading_doc["start"]["facts"], entry_facts)
    bad = sorted(k for k in d if k not in keys and not k.startswith(SLOT_PREFIXES))
    return {"differ": len(d), "task": sum(1 for k in d if k in keys),
            "slots": sum(1 for k in d if k not in keys and k.startswith(SLOT_PREFIXES)), "other": len(bad),
            "first": [[k, *d[k]] for k in bad[:12]]}


# --- E009 fix c: the keys a reading's start value provably does not enter (plan.md (c)) ---------------------------------

REUSE_KEYS_FILE = "reuse-keys.json"  # next to reading.json: K of the reading, computed once
# domains.build's reason for a counter left with no domain by the closure (it looks at other variables' domains, so
# its reason is not one of the writes alone): such a variable keeps E008's rule (plan.md (c), 1)
COUNTER_WHY = "a counter never compared with a constant"
_GAME = {}


def _game():
    import model as M
    if "game" not in _GAME:
        _GAME["game"] = M.Game.load()
    return _GAME["game"]


def named_start_vars() -> set:
    """The variables whose start value the reader reads by number (plan.md (c), 2): the settings, the hook, cursor,
    room and boolean variables, VAR_WALKTO_OBJ, VAR_VERB_SCRIPT, VAR_CUTSCENEEXIT_KEY, VAR_TIMER_NEXT, VAR_CHARINC,
    from the reader's own tables (domains.py, model.py, reader.py, ir.VARS)."""
    import domains as D
    import ir
    import model as M
    import reader as R
    return (set(D.SETTINGS) | set(M.HOOK_VARS) | set(R.CURSOR_VARS) | set(D.ROOM_VARS) | set(D.BOOL_VARS)
            | {D.VAR_WALKTO_OBJ, M.VAR_VERB_SCRIPT, M.VAR_CUTSCENEEXIT_KEY, ir.VARS["VAR_TIMER_NEXT"],
               ir.VARS["VAR_CHARINC"]})


def hover_guard_vars(units: dict) -> set:
    """plan.md (c), 3: the variables any global unit with findObject(VAR_VIRT_MOUSE_X, VAR_VIRT_MOUSE_Y) compares with
    a constant (or tests alone) in a jump before that findObject whose target lies past it: a superset of the input
    script's guards (scumm/reader/src/inputs.rs hover_roles)."""
    import domains as D
    import ir
    mx, my = ir.VARS["VAR_VIRT_MOUSE_X"], ir.VARS["VAR_VIRT_MOUSE_Y"]
    out = set()
    for key, u in units.items():
        if key[0] != "global":
            continue
        finds = [i for i, x in enumerate(u.ins) if x.st is not None and any(
            isinstance(y, ir.Getter) and y.name == "findObject" and tuple(y.args) == (ir.Var(mx), ir.Var(my))
            for y in D._walk(x.st))]
        for f in finds:
            for j, x in enumerate(u.ins[:f]):
                if not isinstance(x.st, ir.Jump) or x.st.cond is None:
                    continue
                target = x.target if x.target is not None else len(u.ins)
                if target <= f:
                    continue
                c = x.st.cond
                if isinstance(c, ir.Cmp):
                    for a, b in ((c.a, c.b), (c.b, c.a)):
                        if isinstance(a, ir.Var) and isinstance(b, ir.Const):
                            out.add(a.n)
                elif isinstance(c, (ir.Truth, ir.Not)) and isinstance(c.x, ir.Var):
                    out.add(c.x.n)
    return out


def reuse_keys_of(rooms, start: dict, game=None) -> dict:
    """plan.md (c): K, the keys var:N whose start value the reader reads at no site: a non-fact for every start (an
    engine variable, or a variable the domain analysis gives no domain for a reason in its writes that the scope
    writes), none of the named variables and no input script's guard. {"keys": sorted, "named", "guards", "s"}."""
    import domains as D
    import reader as R
    t = time.monotonic()
    game = game or _game()
    doms = D.build(start)
    sc = R.Scope.of(game, tuple(sorted(int(r) for r in rooms)), start)
    named, guards = named_start_vars(), hover_guard_vars(game.units)
    keys = set()
    for n in D.ENGINE:
        if n not in named and n not in guards:
            keys.add(f"var:{n}")
    for k, why in doms.why.items():
        n = int(k[4:])
        if n in named or n in guards or n in D.SETTINGS or why == COUNTER_WHY:
            continue
        if n in D.ENGINE or sc.writes.has(k):
            keys.add(k)
    return {"keys": sorted(keys), "named": sorted(named), "guards": sorted(guards),
            "s": round(time.monotonic() - t, 1)}


def reuse_keys(rd) -> set:
    """K of a loaded reading, cached in REUSE_KEYS_FILE next to its reading.json (written atomically)."""
    p = Path(rd.path).parent / REUSE_KEYS_FILE
    try:
        doc = json.loads(p.read_text())
        if doc.get("start") == rd.doc["start"]["name"]:
            return set(doc["keys"])
    except (OSError, ValueError, KeyError):
        pass
    doc = reuse_keys_of(rd.doc["scope"]["rooms"], rd.doc["start"]["facts"])
    doc["start"] = rd.doc["start"]["name"]
    made = {f"var:{n}" for n in rd.doc.get("fact_vars") or ()}  # facts of this reading (E007's refinement)
    doc["keys"] = sorted(set(doc["keys"]) - made)
    tmp = p.with_name(p.name + f".{os.getpid()}.tmp")
    tmp.write_text(json.dumps(doc) + "\n")
    os.replace(tmp, p)
    return set(doc["keys"])


# --- E009 fix a: idle keys -------------------------------------------------------------------------------------------

def idle_rule(start: dict, frames) -> dict:
    """plan.md (a): ``frames`` the facts at each frame after the entry ([(offset, facts)], in order). A key is idle when
    its value comes back to a value it had before (the entry's value included). {"returns": sorted idle keys,
    "first_return": {k: offset}, "changes": {k: n}}."""
    seen: dict = {}
    first: dict = {}
    changes: dict = {}
    prev = dict(start)
    for off, fa in frames:
        for k in set(fa) | set(prev):
            v = fa.get(k, 0)
            if v == prev.get(k, 0):
                continue
            changes[k] = changes.get(k, 0) + 1
            s = seen.setdefault(k, {start.get(k, 0)})
            if v in s and k not in first:
                first[k] = off
            s.add(v)
        prev = fa
    return {"returns": sorted(first), "first_return": first, "changes": changes}


def measure_idle(ctl, root, frames: int) -> dict:
    """Fix a: a fork of the returned engine run ``frames`` frames with no input, the facts taken at each; idle_rule's
    result with the frames, the seconds, the fork's CPU and the frame it ended at. The fork is quit."""
    t = time.monotonic()
    b = ctl.fork(root)
    try:
        f0 = b.frame
        start = facts_of(b.state())

        def seq():
            for i in range(1, frames + 1):
                b.run(frame=f0 + i)
                yield i, facts_of(b.state())
        out = idle_rule(start, seq())
        out["frame_end"] = b.frame
        out["cpu_s"] = round(WS.cpu_of(b.pid), 1)
    finally:
        try:
            b.quit()
        except OSError:
            pass
    out.update(frames=frames, frame0=f0, seconds=round(time.monotonic() - t, 1))
    return out


def entries_from(entry_frame: int, inputs: list, checkpoints: list) -> list:
    """Step 5: one entry per checkpoint with frame > ``entry_frame`` (frame order, one per frame): its events are the
    inputs at frames up to its frame, its checkpoints those in (entry_frame, its frame]; ``how`` "end" for the last,
    "checkpoint" for the others; the tick from the checkpoint's extra. Every input must come after ``entry_frame``
    (premise S of Lemma C); one that does not is an error."""
    early = [e for e in inputs if e["frame"] <= entry_frame]
    if early:
        raise ValueError(f"{len(early)} inputs at or before the entry's frame {entry_frame}: first {early[0]}")
    seen, cps = set(), []
    for c in sorted(checkpoints, key=lambda c: c[0]):
        if c[0] > entry_frame and c[0] not in seen:
            seen.add(c[0])
            cps.append([c[0], c[1], c[2] if len(c) > 2 else {}])
    out = []
    for i, c in enumerate(cps):
        out.append({"id": None, "parent": None, "frame": c[0], "tick": (c[2] or {}).get("tick"),
                    "events": [e for e in inputs if e["frame"] <= c[0]], "checkpoints": cps[:i + 1],
                    "how": "end" if i == len(cps) - 1 else "checkpoint"})
    return out


def start_row(seed: int, state: dict, status: dict) -> dict:
    """The entry's start row in start-part1.jsonl's format (reader-scope ``--start-rows`` reads seed and facts)."""
    if seed != SEED:
        raise ValueError(f"seed {seed}: E008 runs seed {SEED} only (plan.md)")
    return {"seed": seed, "frame": status.get("frame"),
            "stop": {k: status.get(k) for k in ("stop", "frame", "tick", "qf", "room", "idle", "rng", "up", "cs",
                                                 "ego")},
            "dump": {k: state.get(k) for k in ("frame", "tick", "room", "ego", "ego_pos", "seed", "boot_param")},
            "facts": facts_of(state)}


def first_plan_status(plans: list) -> str | None:
    """The status of the harness's first plan: a try the reach extension replaced is not one."""
    for p in plans:
        if p.get("status") != "reach extension":
            return p.get("status")
    return None


def empty_result(job: dict) -> dict:
    return {"job": job.get("job"), "entry": (job.get("entry") or {}).get("id"), "ok": False, "error": None,
            "return": None, "reading": None, "targets": {"by_tier": {"0": 0, "1": 0, "2": 0}, "first": []},
            "probes": [], "attempt": None, "readings_made": [], "new": [], "tried": [], "exhausted": False,
            "cpu_s": {"return": 0.0, "read": 0.0, "plan": 0.0, "play": 0.0, "total": 0.0},
            "peak_rss_mb": {"python": 0, "children": 0}, "wall_s": 0.0}


def exit_code(result: dict) -> int:
    """The CLI's exit (module docstring): 0 ok, 3 the return differs, 1 any other error."""
    if result.get("ok"):
        return 0
    if (result.get("return") or {}).get("same") is False:
        return 3
    return 1


def write_entry(path: Path, entry: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(path, "wt") as f:
        json.dump(entry, f)


def read_entry(path) -> dict:
    with gzip.open(path, "rt") as f:
        return json.load(f)


# --- costs -------------------------------------------------------------------------------------------------------------

def _cpu() -> float:
    a, b = resource.getrusage(resource.RUSAGE_SELF), resource.getrusage(resource.RUSAGE_CHILDREN)
    return a.ru_utime + a.ru_stime + b.ru_utime + b.ru_stime


def _peaks() -> dict:
    return {"python": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss // 1024,
            "children": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss // 1024}


class _Tee:
    """stderr to the job's log as well (the harness's and the reader's progress lines)."""

    def __init__(self, path: Path, other):
        self.f, self.other = open(path, "a", buffering=1), other

    def write(self, s):
        self.f.write(s)
        try:
            self.other.write(s)
        except (OSError, ValueError):
            pass
        return len(s)

    def flush(self):
        self.f.flush()
        try:
            self.other.flush()
        except (OSError, ValueError):
            pass

    def close(self):
        self.f.close()


def say(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] step: {msg}", file=sys.stderr, flush=True)


# --- the job -----------------------------------------------------------------------------------------------------------

class ReturnDiffers(Exception):
    pass


def boot_entry(entry: dict, out_dir: Path) -> tuple:
    """Step 1: (Controller, the returned engine's Branch, the return's record)."""
    t = time.monotonic()
    checks = []
    if entry.get("root"):
        if entry.get("events"):
            raise ValueError("a root entry has no events")
        ctl, b, row = P.boot(SEED, out_dir, dump_locals=True)
        checks = row["checks"]
    else:
        ctl, b = replay.boot(lambda o, i: P.config(SEED, o, i, dump_locals=True), entry["events"], entry["frame"],
                             out_dir)
    d = replay.compare_checkpoint(tuple(entry["checkpoint"]), b.state(), b.status)
    if b.frame != entry["frame"]:
        d["frame"] = {"want": entry["frame"], "got": b.frame}
    if checks:
        d["boot_checks"] = checks
    return ctl, b, {"same": not d, "differ": json.loads(json.dumps(d, default=str)),
                    "boot_s": round(time.monotonic() - t, 2), "frame": b.frame}


def reading_record(rd, fresh: bool, why: str, wall: float = 0.0, load_s: float = 0.0) -> dict:
    doc = rd.doc
    rs = doc.get("reader_scope") or {}
    out = {"path": str(rd.path), "fresh": fresh, "why": why, "scope": list(doc["scope"]["rooms"]),
           "records": doc.get("n_records"), "actions": doc.get("n_actions"),
           "read_wall_s": rs.get("read_wall_s") if fresh else 0.0,
           "read_cpu_s": rs.get("read_cpu_s") if fresh else 0.0,
           "peak_rss_mb": rs.get("peak_rss_mb") if fresh else 0,
           "files_s": round(wall - (doc.get("read_s") or 0.0), 1) if fresh else 0.0}
    out.update(read_s=doc.get("read_s") if fresh else 0.0, wall_s=round(wall, 1), load_s=round(load_s, 2),
               fact_vars=doc.get("fact_vars") or [], start=doc["start"]["name"])
    return out


def read_fresh(job: dict, state: dict, status: dict, threads: int):
    """Step 2's fresh reading from the returned engine's state: (Reading, wall seconds, load seconds)."""
    rdir = Path(job["reading_dir"])
    if not rdir.is_absolute() or any(c.isspace() for c in str(rdir)):
        raise ValueError(f"reading_dir {rdir}: an absolute path with no spaces (the reach extender's start-rows rule)")
    rdir.mkdir(parents=True, exist_ok=True)
    rows = rdir / "start.jsonl"
    rows.write_text(json.dumps(start_row(SEED, state, status)) + "\n")
    name = rdir.name
    scope = tuple(sorted(int(r) for r in job["scope"]))
    say(f"reading {list(scope)} fresh from frame {status.get('frame')} into {rdir}")
    t = time.monotonic()
    path = P.read_rust(scope, SEED, goal_text(job["goal"]), name, rdir, threads, None, "none", elsewhere=True,
                       live_locals=True, split_limit=True, switches=SWITCHES, reach=True, frame_costs=True,
                       start_rows=rows, stream=True)
    wall = time.monotonic() - t
    t = time.monotonic()
    rd = PR.load_reading(path)
    load_s = time.monotonic() - t
    (rdir / f"{name}.golden.jsonl.gz").unlink(missing_ok=True)
    return rd, wall, load_s


def start_rows_of(doc: dict) -> Path | None:
    """The start-rows file a reading names as its start (``PATH seed N``; a refinement reads from it again), or None
    for results/start-part1.jsonl's starts."""
    name = (doc.get("start") or {}).get("name") or ""
    head, sep, _ = name.rpartition(" seed ")
    return Path(head) if sep and head.startswith("/") and head.endswith(".jsonl") else None


def cached_readings(job: dict) -> list:
    """The job's readings of its scope whose files exist, newest first (reading.json's modification time, then the
    job's order). The files include the start-rows file the reading names: a refinement reading keeps its first
    reading's, and an eviction of that reading may have removed it."""
    scope = sorted(int(r) for r in job["scope"])
    out = []
    for i, r in enumerate(job.get("readings") or ()):
        p = Path(r["path"])
        try:
            doc = json.loads(p.read_text())
        except (OSError, ValueError):
            continue
        if sorted(doc.get("scope", {}).get("rooms", ())) != scope:
            continue
        packed = p.parent / doc.get("packed", "reading.tc")
        if not packed.exists() or not packed.with_suffix(".reached").exists():
            continue
        rows = start_rows_of(doc)
        if rows is not None and not rows.exists():
            continue
        out.append((-p.stat().st_mtime, i, p))
    return [p for _, _, p in sorted(out)]


def choose_reading(job: dict, facts: dict) -> tuple:
    """Step 2 on the cached readings: (Reading or None, why, the candidates checked). With job ``reuse`` "e009" a
    reading E008's rule refuses is checked again with ``reuse_keys`` allowed to differ too (fix c)."""
    cands = cached_readings(job)
    if not cands:
        return None, "no cached reading", []
    checked = []
    for p in cands:
        rd = PR.load_reading(p)
        ok, n = reusable(rd.doc, rd.task.facts, facts)
        row = dict(path=str(p), reusable=ok, **reusable_detail(rd.doc, rd.task.facts, facts))
        if not ok and job.get("reuse") == "e009":
            extra = reuse_keys(rd)
            keys = set(rd.task.facts) | extra
            ok, n = reusable(rd.doc, keys, facts)
            row.update(reusable=ok, reuse_keys=len(extra), e009=reusable_detail(rd.doc, keys, facts))
        checked.append(row)
        if ok:
            return rd, "reused" if "e009" not in row else "reused (e009 keys)", checked
        rd.close()
    first = checked[0].get("e009") or checked[0]
    return None, f"differs on {first['other']} keys", checked


def root_facts_of(job: dict, job_dir: Path) -> dict | None:
    """The root's facts, for the hold rule of (k, any) atoms: the job's ``root_facts`` when it has them, the root
    entry itself, or the archive's root entry file (ARCHIVE/jobs/JOB/ -> ARCHIVE/entries.jsonl); else None."""
    if job.get("root_facts") is not None:
        return job["root_facts"]
    if job["entry"].get("root"):
        return job["entry"]["checkpoint"][1]
    arch = job_dir.resolve().parent.parent
    idx = arch / "entries.jsonl"
    try:
        for ln in idx.read_text().splitlines():
            e = json.loads(ln)
            if e.get("parent") is None:
                return read_entry(arch / "entries" / f"{e['id']}.json.gz")["checkpoints"][-1][1]
    except (OSError, ValueError, KeyError, IndexError):
        pass
    return None


def made_reading(h, rd) -> dict | None:
    """The harness's last reading when it is not the one it started on (plan.md: registered as a reading of the
    scope): its reading.json, its scope and its kind (reach, refine or reread, from its folder)."""
    if Path(h.packed) == Path(rd.packed):
        return None
    head = Path(h.packed).parent / "reading.json"
    try:
        doc = json.loads(head.read_text())
        scope = list(doc["scope"]["rooms"])
    except (OSError, ValueError, KeyError):
        scope = None
    name = Path(h.packed).parent.name
    kind = next((k for k in ("reach", "refine", "reread") if name.startswith(k + "-")), "other")
    return {"path": str(head), "scope": scope, "kind": kind}


MADE_KINDS = ("reach", "refine", "reread")  # the folders the harness reads into during an attempt (made_reading)
KEEP_FILES = ("reading.json", "start.txt", "start.jsonl")  # what drop_unused leaves (start files: --reach-also, rows)


def drop_unused(adir: Path, made: dict | None) -> list:
    """The readings the harness made in the attempt's folder ``adir`` and did not end on (a refinement it refused, or
    one a later reading of the attempt replaced). Only ``made`` (made_reading's) is registered, so no job.json can
    name the others and no eviction ever reaches them: their files go now, but for reading.json and the start files,
    which a registered reading may name. Returns [[folder, bytes freed], ...]."""
    keep = Path(made["path"]).parent.resolve() if made else None
    out = []
    for d in (sorted(p for p in adir.iterdir() if p.is_dir()) if adir.is_dir() else ()):
        if d.name.split("-")[0] not in MADE_KINDS or (keep is not None and d.resolve() == keep):
            continue
        freed = 0
        for p, _, files in os.walk(d):
            for f in files:
                if f not in KEEP_FILES:
                    q = Path(p) / f
                    try:
                        freed += q.stat().st_size
                        q.unlink()
                    except OSError:
                        pass
        out.append([d.name, freed])
    return out


def attempt(rd, ctl, root, target: tuple, goal: dict, adir: Path, threads: int, workers: int) -> tuple:
    """Step 4 for one target: (the harness's record, the Harness, the attempt's seconds, its CPU)."""
    k, v, tier, _depth = target
    if tier == 0:
        hgoal, text = {kk: set(vs) for kk, vs in goal.items()}, goal_text(goal)
    else:
        hgoal, text = {k: {v}}, f"{k}={v}"
    frame_costs = rd.doc.get("frame_costs") or {}
    starts: list = []  # the reach extension's start files, which the refinement shares
    facts: list = list(rd.doc.get("fact_vars") or [])  # E007's loop: the attempt's fact variables so far
    adir.mkdir(parents=True, exist_ok=True)
    def snapshot(hh):  # the record so far after each plan, as cmd_run's partial.json: a killed job keeps it
        (adir / "partial.json").write_text(json.dumps(hh.result(), default=str) + "\n")
    cpu0, t0 = _cpu(), time.monotonic()
    branch = ctl.fork(root)
    try:
        wcfg = WS.WindowConfig(workers=workers, delta_lo=WS.delta_lo_of(frame_costs), strict=True, cache=True,
                               fork_each=FORK_EACH, lite=True, split_wait=True)
        h = replay.Harness(rd.b, rd.task, rd.task, branch, hgoal, replan_limit=REPLAN_LIMIT, time_limit=TIME_LIMIT,
                           plan_time=PLAN_TIME, progress=say, planner="taskcore", packed=rd.packed,
                           workdir=adir / "plans", memory_limit=PLAN_MEMORY, on_plan=snapshot, slot_locals=False,
                           lists=rd.lists, taskcore_args=("--goal", text),
                           taskcore_serve=True, search="lama-first-costs", cost_caps=True,
                           cost_dmax=frame_costs.get("dmax"),
                           reach_extend=P.reach_extender(rd, adir, threads, starts, facts),
                           unobserved_actor_rooms=True, stone_fork=ctl.fork, window_search=wcfg, fork=ctl.fork,
                           refine=P.refiner(rd, adir, threads, starts, facts))
        say(f"attempt {adir.name}: target {k}={v} (tier {tier}) from frame {root.frame}")
        res = h.run()
    finally:
        try:
            branch.quit()  # the harness's root: it never quits it
        except OSError:
            pass
        plan_fd.close_servers()  # the served taskcore of this goal: its memory, and its CPU counted
    (adir / "run.json").write_text(json.dumps(res, default=str) + "\n")
    for g in adir.glob("*/*.golden.jsonl.gz"):  # the readings the harness made: their files are made
        g.unlink(missing_ok=True)
    return res, h, time.monotonic() - t0, _cpu() - cpu0


def attempt_record(target: tuple, res: dict, seconds: float) -> dict:
    k, v, tier, depth = target
    cps = res.get("_checkpoints") or []
    return {"target": [k, v], "tier": tier, "depth": depth, "stop": res.get("stop"), "reached": res.get("_reached"),
            "goal_from_the_plan": res.get("goal_from_the_plan"), "frames": res.get("frames"),
            "frame0": res.get("frame0"), "frame_end": res.get("frame_end"), "qf0": res.get("qf0"),
            "qf_end": res.get("qf_end"), "qf": res.get("qf"), "replans": res.get("replans"),
            "refinements": [{k_: x.get(k_) for k_ in ("plan", "frame", "class", "added", "why_not", "seconds",
                                                      "cpu_s")} for x in res.get("refinements") or ()],
            "extensions": [{k_: x.get(k_) for k_ in ("plan", "frame", "n_values", "seconds", "cpu_s", "actions_before",
                                                     "actions")} for x in res.get("extensions") or ()],
            "window_summary": res.get("window_summary"), "seconds": round(seconds, 1),
            "inputs": len(res.get("inputs") or ()), "checkpoints": len(cps), "error": res.get("error")}


def attempt_costs(res: dict, att_cpu: float) -> dict:
    """An attempt's CPU by part (module docstring, "CPU per part"): ``read`` the readers of its reach extensions and
    refinements, ``play`` the window search's engines and Python, ``plan`` the rest of ``att_cpu`` (getrusage over
    the attempt)."""
    ws = res.get("window_summary") or {}
    py = ws.get("python_cpu_s") or 0.0
    reads = sum(x.get("cpu_s") or 0.0 for x in list(res.get("extensions") or []) + list(res.get("refinements") or []))
    return {"read": reads, "play": sum((ws.get("engine_cpu_s") or {}).values()) + py,
            "plan": max(0.0, att_cpu - py - reads)}


def target_holds(target: tuple, goal: dict, facts: dict) -> bool:
    k, v, tier, _ = target
    if tier == 0:
        return all(facts.get(kk, 0) in vs for kk, vs in goal.items())
    return facts.get(k, 0) == v


def run_job(job_dir) -> dict:
    """``step.py run JOB_DIR`` (module docstring): the result as written to JOB_DIR/result.json."""
    job_dir = Path(job_dir).resolve()
    t_job, cpu_job = time.monotonic(), _cpu()
    job = json.loads((job_dir / "job.json").read_text())
    out = empty_result(job)
    tee = _Tee(job_dir / "log", sys.stderr)
    old_err, sys.stderr = sys.stderr, tee
    ctl = rd = None
    cpu = {"return": 0.0, "read": 0.0, "plan": 0.0, "play": 0.0}
    try:
        if job.get("seed") != SEED:
            raise ValueError(f"seed {job.get('seed')}: E008 runs seed {SEED} only (plan.md)")
        entry = job["entry"]
        goal = {k: list(vs) for k, vs in job["goal"].items()}
        threads, workers = int(job.get("threads") or 3), int(job.get("workers") or 3)
        run_dir = job_dir / "run"
        run_dir.mkdir(parents=True, exist_ok=True)
        # 1. return
        say(f"job {job.get('job')}: returning to entry {entry.get('id')} at frame {entry['frame']}")
        c0 = _cpu()
        ctl, root, ret = boot_entry(entry, run_dir / "boot")
        cpu["return"] = _cpu() - c0 + WS.cpu_of(root.pid)
        out["return"] = ret
        if not ret["same"]:
            raise ReturnDiffers(f"the return to entry {entry.get('id')} differs from its checkpoint: {ret['differ']}")
        state, status = root.state(), dict(root.status)
        facts = facts_of(state)
        if job.get("idle_frames") and not job.get("idle_measured"):  # E009 fix a
            say(f"idle: {job['idle_frames']} frames with no input from frame {root.frame}")
            c0 = _cpu()
            out["idle"] = measure_idle(ctl, root, int(job["idle_frames"]))
            cpu["idle"] = _cpu() - c0 + out["idle"]["cpu_s"]
            say(f"idle keys: {out['idle']['returns'][:12]} ({len(out['idle']['returns'])}), "
                f"{out['idle']['seconds']} s")
        # 2. reading
        c0 = _cpu()
        t = time.monotonic()
        rd, why, checked = choose_reading(job, facts)
        load_s = time.monotonic() - t
        if rd is not None:
            out["reading"] = reading_record(rd, False, why, load_s=load_s)
        else:
            plan_fd.close_servers()
            rd, wall, load_s = read_fresh(job, state, status, threads)
            out["reading"] = reading_record(rd, True, why, wall, load_s)
        out["reading"]["candidates"] = checked
        cpu["read"] += _cpu() - c0
        say(f"reading {out['reading']['path']}: {'fresh' if out['reading']['fresh'] else 'reused'} ({why})")
        # 3. targets
        guide = json.loads(Path(job["guide"]).read_text())
        reached = Path(rd.packed).with_suffix(".reached")
        ts = targets(reached, guide, facts, root_facts_of(job, job_dir), job.get("held") or (),
                     job.get("tried") or (), goal, job.get("rotation"))
        out["targets"] = {"by_tier": {str(n): sum(1 for x in ts if x[2] == n) for n in (0, 1, 2)},
                          "first": [list(x) for x in ts[:20]], "n": len(ts)}
        say(f"{len(ts)} targets: {out['targets']['by_tier']}; first {ts[:5]}")
        # 4. attempts
        played, fresh_again = None, False
        queue = list(ts)
        n_att = 0
        while queue:
            tg = queue.pop(0)
            res, h, secs, att_cpu = attempt(rd, ctl, root, tg, goal, run_dir / f"a{n_att:02d}", threads, workers)
            n_att += 1
            status_ = first_plan_status(res.get("plans") or [])
            made = made_reading(h, rd)
            if made is not None:
                out["readings_made"].append(made)
            dropped = drop_unused(run_dir / f"a{n_att - 1:02d}", made)
            for part, x in attempt_costs(res, att_cpu).items():
                cpu[part] += x
            out["probes"].append({"target": [tg[0], tg[1]], "tier": tg[2], "depth": tg[3], "status": status_,
                                  "stop": res.get("stop"), "seconds": round(secs, 1), "attempt": f"a{n_att - 1:02d}",
                                  "reading": str(rd.path), "dropped": dropped})
            if status_ == "plan":
                played = (tg, res, h, secs)
                break
            out["tried"].append([tg[0], tg[1]])
            if status_ == NOT_A_PLAN_RETRY and not out["reading"]["fresh"] and not fresh_again:
                # the cached reading does not cover the entry: the scope read from the entry's state, the same target
                fresh_again = True
                c0 = _cpu()
                old = out["reading"]
                rd.close()
                rd, wall, load_s = read_fresh(job, state, status, threads)
                out["reading"] = reading_record(rd, True, NOT_A_PLAN_RETRY, wall, load_s)
                out["reading"]["replaced"] = {"path": old["path"], "why": old["why"]}
                out["reading"]["candidates"] = checked
                cpu["read"] += _cpu() - c0
                out["tried"].pop()
                queue.insert(0, tg)
        if played is not None:
            tg, res, h, secs = played
            out["tried"].append([tg[0], tg[1]])
            cps = [list(c) for c in h.checkpoints]
            ents = entries_from(entry["frame"], h.inputs(), cps)
            end_facts = cps[-1][1] if cps else facts
            res["_checkpoints"], res["_reached"] = cps, target_holds(tg, goal, end_facts)
            out["attempt"] = attempt_record(tg, res, secs)
            new_dir = job_dir / "new"
            for i, e in enumerate(ents):
                how = e.pop("how")
                e["parent"] = entry.get("id")
                write_entry(new_dir / f"{i}.json.gz", e)
                out["new"].append({"file": f"new/{i}.json.gz", "frame": e["frame"], "tick": e["tick"], "how": how})
            say(f"played {tg[0]}={tg[1]}: stop {res.get('stop')!r}, {res.get('frames')} frames, {len(ents)} entries")
        out["exhausted"] = played is None
        out["ok"] = True
    except ReturnDiffers as e:
        out["error"] = str(e)
    except (Exception, SystemExit) as e:  # noqa: BLE001 (the result says what happened; the log has the traceback)
        out["error"] = f"{type(e).__name__}: {e}"
        print(traceback.format_exc(), file=sys.stderr)
    finally:
        try:
            plan_fd.close_servers()
            if rd is not None:
                rd.close()
            if ctl is not None:
                ctl.close()
        except Exception as e:  # noqa: BLE001
            print(f"closing: {type(e).__name__}: {e}", file=sys.stderr)
        sys.stderr = old_err
        tee.close()
    cpu = {k: round(v, 1) for k, v in cpu.items()}
    cpu["total"] = round(sum(cpu.values()), 1)
    cpu["rusage_total"] = round(_cpu() - cpu_job, 1)
    out["cpu_s"] = cpu
    out["peak_rss_mb"] = _peaks()
    out["wall_s"] = round(time.monotonic() - t_job, 1)
    (job_dir / "result.json").write_text(json.dumps(out, indent=1, default=str) + "\n")
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("job_dir", type=Path)
    a = ap.parse_args()
    sys.exit(exit_code(run_job(a.job_dir)))


if __name__ == "__main__":
    main()
