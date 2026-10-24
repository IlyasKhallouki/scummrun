"""E003 step 2: the start, the engine's own state at the first idle stop from boot.

usage: uv run python experiments/E003-path-through-cloud/start.py [SEED ...]   (default: the tuning seeds 1-5)
       from start import load; starts = load()   ({seed: row}, each row's ``facts`` the full fact map)

**Rule** (plan.md step 2). For each seed, boot ScummVM with the plan's settings and give no input; run
to the bridge's first **idle** stop (``Branch.run(idle=True)``: ``speedrun_control.cpp``
controlAtDecisionPoint, reason "idle", where ``evaluateIdle`` in ``speedrun_state.cpp`` holds) and
read the engine's state there (``Branch.state()``, the C7 dump) as facts (``facts.facts_of``).
Everything the boot script does is inside this state, so boot is not an action. The idle stop is the
bridge's "the player could click now": input on, no cutscene, no sentence queued or sentence, input or
object script running, no text that a script waits for, no fade, ego not walking, and verbs shown (or a
room input script). Its **control** stop is looser (``clickGateOpen``: input on, no cutscene, a room);
it is not the start, and the log gives its frame only to show the two apart. The start must be a
*sentence* idle; a dialog-menu idle (a line to click) would be an input before the start, so the run
stops there and says so.

Settings (plan.md "Method"): the DOS CD release on the patched build (``speedrun.engine``: its CD timer
runs on engine ticks, ``--disable-sdl-audio``, ``vsync=false``), boot parameter 0 (``--boot-param=0``;
ScummVM runs the boot script with it as Local[0] and turns debug mode on for any other value,
scumm.cpp:272-275), no debug flag (no ``-d``: ``_debugMode = gDebugLevel > 0``, scumm.cpp:270, which
the engine writes to VAR_DEBUGMODE, var 39: vars.cpp:90, scumm.cpp:3389-3391), ScummVM's enhancements
off (``enhancements=0`` in the ini: the MI1 workarounds in the opcode handlers are gated by
``enhancementEnabled``, except the cannibal text patch, printPatchedMI1CannibalString, script_v5.cpp:
3704-3705, which changes only printed text), talkspeed 255 (``engine.TALKSPEED``). The bridge's
automatic '.' and Esc are off (``skip_text``, ``skip_cutscenes``), and there is no plan, raw input,
interrupt or segment start frame, so nothing reaches the engine but time (plan.md: "No inputs are given
before the first idle stop"). ``timer_next`` is "any": the bridge then does not end the run on a var 19
value; it changes nothing in the engine.

Seeds: the tuning seeds 1-5 only (plan.md "Seeds, fixed now"). ``config`` refuses the held-out seeds
31-35 and the Any% seed.

C2: the start rests on the bridge's idle test, which uses the bridge's MI1 verb ranges (action verbs
kActionVerbFirst..Last = 2..10, dialog lines kDialogVerbFirst..Last = 120..128, speedrun_state.cpp:
56-57) to tell a sentence idle from a dialog-menu idle. That is plan.md C2's disclosed replay-harness
exception; this file names no game number itself.

**Random facts** (plan.md step 1: random draws are not facts). Each row's ``random`` lists the fact
keys that a script live at the start will write again straight from getRandomNr (o5_getRandomNr,
script_v5.cpp:1438-1445): for every live slot, the instructions reachable from where it resumes (its
``at:`` key; along ``next``, jumps and Esc's override jump, and into every script it starts with a
constant target, transitively), and in them every assignment whose right side draws. Starts whose
target is read at run time and the hook scripts the engine runs are not followed; both are counted
(``live_draws``). Their values in ``facts`` are what the engine showed at the stop, kept as ``facts_of``
gives them; step 4 treats these keys as unknown. ``random_sites`` cites the draws. A live slot with no
``at:`` key, or whose W is no instruction, is read from all its instructions, and an indexed write's
target is unknown; both are counted (``random_counts``).

Checks, in the log: the stop is "idle" and its kind "sentence"; the dump's seed and boot_param; var 39
(VAR_DEBUGMODE) is 0; the trace up to the stop holds only the record types in ``TRACE_TYPES``, none of
them an input (the bridge's input, click, choice, interrupt, skip and rng_set records are not
allowed); one seed booted a second time gives the same facts (determinism, so a difference across
seeds is the seed's); the facts by kind; the keys whose values differ across the seeds, each with its
values, whether it is in the rows' ``random`` (it must be) and whether the IR's data flow shows it
written from a random draw (``random_keys``). Each row stores the sha256 of facts.py
(``facts_py_sha256``), so a test can tell facts written by an older facts.py.

Writes ``results/start-part1.jsonl`` (one line per seed, keys sorted) and ``results/start-part1.log``,
each to a temporary file that then replaces the old one. The engine's trace, ini and logs go to
``out/E003-start/seed-N``.
"""

import collections
import concurrent.futures
import hashlib
import json
import os
import sys
import time
from pathlib import Path

import ir
from end import HOOK_OPS
from facts import facts_of
from speedrun import engine, paths, releases
from speedrun.control import Controller, ControlError
from speedrun.engine import EngineConfig

HERE = Path(__file__).resolve().parent
OUT = HERE / "results" / "start-part1.jsonl"
LOG = HERE / "results" / "start-part1.log"
ENGINE_DIR = paths.OUT_DIR / "E003-start"
RELEASE = "dos-cd"
SEEDS = (1, 2, 3, 4, 5)  # plan.md "Seeds, fixed now": tuning
FORBIDDEN = set(range(31, 36)) | {836557856}  # held out (plan.md), and the Any% seed
BOOT_PARAM = 0
MAX_FRAME = 200_000  # the run stops with reason "max_frame" if no idle comes first; counted, never silent
MAX_TICKS = 10**7  # SPEEDRUN_MAX_TICKS: the bridge ends the run past it (an "end" record, reported)
TIMEOUT_S = 900.0  # socket timeout per command
VAR_RESULT = 0  # the scripts' result register (ir.py VARS; ScummVM's VAR_KEYPRESS slot, vars.cpp:37)
VAR_DEBUGMODE = 39  # vars.cpp:90
VAR_ROOM_RESOURCE = 22  # vars.cpp:58: the room resource whose local scripts run
RANDOM_GETTER = "getRandomNr"  # o5_getRandomNr, script_v5.cpp:1438-1445
# The trace records allowed before the start, none of them an input: boot, room, cd_track, control (the
# player has control again), segment_start, stall (speedrun_bridge.cpp:296, 393, 438, 507, 560, 593).
TRACE_TYPES = {"boot", "room", "cd_track", "control", "segment_start", "stall"}
FACTS_PY = HERE / "facts.py"


def facts_sha256() -> str:
    """The sha256 of facts.py, stored per row: facts written by another facts.py are stale."""
    return hashlib.sha256(FACTS_PY.read_bytes()).hexdigest()


def config(seed: int, out_dir: Path) -> EngineConfig:
    """The plan's engine settings for ``seed`` (module docstring)."""
    if seed in FORBIDDEN:
        raise ValueError(f"seed {seed} is held out or the Any% seed; the start uses the tuning seeds {SEEDS}")
    rel = releases.get(RELEASE)
    return EngineConfig(out_dir=out_dir, game_path=releases.game_dir(rel), release=RELEASE, seed=seed,
                        boot_param=BOOT_PARAM, max_ticks=MAX_TICKS, timer_next="any",
                        skip_text=False, skip_cutscenes=False)


def boot(seed: int, out_dir: Path | None = None, control: bool = True, units: dict | None = None) -> dict:
    """Boot ``seed`` with no input, run to the first idle stop and return the row: the stop status,
    the dump's header, the facts, and (``control``) the frame of the bridge's control stop, from a fork
    taken at the first stop (a fork is an exact copy, so the root's run is the same with or without)."""
    out_dir = out_dir or ENGINE_DIR / f"seed-{seed}"
    cfg = config(seed, out_dir)
    row: dict = {"seed": seed}
    with Controller(cfg, timeout_s=TIMEOUT_S) as c:
        b = c.root
        row["first_stop"] = {k: b.status.get(k) for k in ("stop", "frame", "tick")}
        if control:
            child = c.fork(b)
            st = child.run(control=True, max_frame=MAX_FRAME)
            row["control_stop"] = {k: st.get(k) for k in ("stop", "frame", "tick", "room", "idle", "blk")}
            child.quit()
        st = b.run(idle=True, max_frame=MAX_FRAME)
        if "end" in st:
            raise ControlError(f"seed {seed}: the run ended before an idle stop: {st}")
        row["stop"] = {k: st.get(k) for k in ("stop", "frame", "tick", "qf", "room", "idle", "blk", "rng", "v19",
                                              "up", "cs", "ego", "seg")}
        s = b.state()
        row["dump"] = {k: s.get(k) for k in ("frame", "tick", "room", "ego", "ego_pos", "seed", "boot_param")}
        row["facts"] = dict(sorted(facts_of(s).items()))
    row["facts_py_sha256"] = facts_sha256()
    sites, row["random_counts"] = live_draws(row["facts"], units)
    row["random"], row["random_sites"] = sorted(sites), dict(sorted(sites.items()))
    row["argv"] = engine.build_argv(cfg)
    row["ini"] = engine.ini_path(cfg).read_text()
    row["trace"] = trace_records(out_dir / "trace.jsonl", row["stop"]["frame"])
    return row


def trace_records(path: Path, frame: int) -> dict:
    """Counts of the trace's record types up to ``frame``: each must be in ``TRACE_TYPES``."""
    counts = collections.Counter()
    for line in path.read_text().splitlines():
        rec = json.loads(line)
        if rec.get("frame", 0) <= frame:
            counts[rec.get("type", "?")] += 1
    return dict(sorted(counts.items()))


def fact_kind(key: str) -> str:
    """A fact key's kind: its prefix, with the unit kind for ``at:``, ``esc:`` and ``frozen:``
    (at:script, at:local, at:obj, at:entry, esc:script, frozen:obj, ...); a bare key is its own kind."""
    head, _, rest = key.partition(":")
    return f"{head}:{rest.split(':')[0]}" if head in ("at", "esc", "frozen") else head


def kind_counts(facts: dict[str, int]) -> dict[str, tuple[int, int]]:
    """{kind: (keys, keys whose value is not 0)}."""
    keys, nonzero = collections.Counter(), collections.Counter()
    for k, v in facts.items():
        keys[fact_kind(k)] += 1
        nonzero[fact_kind(k)] += v != 0
    return {k: (keys[k], nonzero[k]) for k in sorted(keys)}


def differing(rows: list[dict]) -> dict[str, list[int]]:
    """{key: [its value per row]} for every key whose value is not the same in all rows (absent is 0)."""
    keys = set().union(*(r["facts"] for r in rows))
    vals = {k: [r["facts"].get(k, 0) for r in rows] for k in sorted(keys)}
    return {k: v for k, v in vals.items() if len(set(v)) > 1}


# --- what the IR writes from a random draw ---------------------------------------------------------

def _reads(x, unit: tuple, tainted: set) -> bool:
    """Whether expression ``x`` draws a random number or reads a tainted var, bit or local of ``unit``.
    Var 0, the result register, is read as the unit's own (``_target``)."""
    if isinstance(x, ir.Getter):
        return x.name == RANDOM_GETTER or any(_reads(a, unit, tainted) for a in x.args)
    if isinstance(x, ir.Var):
        return (("result", unit) if x.n == VAR_RESULT else ("var", x.n)) in tainted
    if isinstance(x, ir.Bit):
        return ("bit", x.n) in tainted
    if isinstance(x, ir.Local):
        return ("local", unit, x.n) in tainted
    if isinstance(x, ir.Embed):
        return _reads(x.st.rhs, unit, tainted)
    if isinstance(x, ir.Index):
        return _reads(x.offset, unit, tainted)  # the slot is unknown; only a tainted index counts here
    if isinstance(x, (ir.BinOp, ir.Cmp)):
        return _reads(x.a, unit, tainted) or _reads(x.b, unit, tainted)
    if isinstance(x, (ir.Neg, ir.Not, ir.Truth)):
        return _reads(x.x, unit, tainted)
    if isinstance(x, (ir.List, ir.Concat)):
        return any(_reads(a, unit, tainted) for a in (x.items if isinstance(x, ir.List) else x.parts))
    return False


def _target(lhs, unit: tuple):
    """What an assignment writes: ("var", n), ("bit", n), ("local", unit, n), or for var 0 ("result",
    unit): the scripts use var 0 as a scratch register, read right after the write in the same script,
    so a draw into it taints only that unit's reads of it (a write elsewhere would taint every
    ``if (VAR_RESULT ...)`` in the game). None for an indexed write: its target is not known."""
    if isinstance(lhs, ir.Var):
        return ("result", unit) if lhs.n == VAR_RESULT else ("var", lhs.n)
    if isinstance(lhs, ir.Bit):
        return ("bit", lhs.n)
    if isinstance(lhs, ir.Local):
        return ("local", unit, lhs.n)
    return None


def _key(t: tuple) -> str | None:
    """The fact key of a ``_target``: var:N, bit:N, var:0 for the result register; None for a local."""
    return {"var": f"var:{t[-1]}", "bit": f"bit:{t[-1]}", "result": f"var:{VAR_RESULT}"}.get(t[0])


def _assigns(x):
    """The Assigns a statement runs: itself, and every Embed inside its expressions."""
    st = x.st
    out = [st] if isinstance(st, ir.Assign) else []
    stack = [st]
    while stack:
        n = stack.pop()
        if isinstance(n, ir.Embed):
            out.append(n.st)
        if isinstance(n, (ir.Assign, ir.Jump, ir.Start, ir.Op, ir.Getter, ir.BinOp, ir.Cmp, ir.Neg, ir.Not,
                          ir.Truth, ir.List, ir.Concat, ir.Embed, ir.Index)):
            for f in getattr(n, "__dataclass_fields__", {}):
                v = getattr(n, f)
                stack.extend(v if isinstance(v, tuple) else [v])
    return out


def random_keys(units: dict | None = None) -> tuple[dict[str, str], int]:
    """Fact keys (``var:N``, ``bit:N``) some script writes from a random draw, by data flow over the
    whole IR to a fixpoint: an assignment whose right side draws (getRandomNr) or reads a var, bit or
    (same unit) local or result register already so written. Value: "data". Then one step of control
    flow: a constant write inside a forward if-block whose test reads such a value (the instructions
    between the jump and its target). Value: "control". This is the wide net, for the log; the start's
    ``random`` is ``live_draws``. Also returns the indexed writes skipped (their target is unknown)."""
    units = units if units is not None else ir.load()
    tainted, indexed = set(), set()
    changed = True
    while changed:
        changed = False
        for key, u in units.items():
            for x in u.ins:
                for a in _assigns(x):
                    t = _target(a.lhs, key)
                    if t is None:
                        indexed.add((key, x.offset))
                        continue
                    if t not in tainted and _reads(a.rhs, key, tainted):
                        tainted.add(t)
                        changed = True
    out = {k: "data" for k in map(_key, tainted) if k}
    for key, u in units.items():
        for i, x in enumerate(u.ins):
            if not (isinstance(x.st, ir.Jump) and x.st.cond is not None and x.target is not None and x.target > i):
                continue
            if not _reads(x.st.cond, key, tainted):
                continue
            for y in u.ins[i + 1:x.target]:
                for a in _assigns(y):
                    t = _target(a.lhs, key)
                    k = _key(t) if t is not None else None
                    if k:
                        out.setdefault(k, "control")
    return out, len(indexed)


def live_units(facts: dict[str, int], units: dict) -> list[tuple[tuple, int | None]]:
    """(IR unit key, resume offset W or None) for every live slot in ``facts`` (facts.py ``at:``,
    ``script:``, ``local:`` and ``objscript:`` keys). Local and entry code is the room resource's in
    VAR_ROOM_RESOURCE (vars.cpp:58). W is None for a live slot with no ``at:`` key."""
    room = facts.get(f"var:{VAR_ROOM_RESOURCE}", 0)
    unit = {"script": lambda n: ("global", n), "local": lambda n: ("local", room, n), "obj": lambda n: ("obj", n)}
    out, seen = [], set()
    for k, v in facts.items():
        if not (v and k.startswith("at:")):
            continue
        parts = k.split(":")
        key = ("entry", room) if parts[1] == "entry" else unit[parts[1]](int(parts[2])) if parts[1] in unit else None
        if key is not None:
            out.append((key, int(parts[-1])))
            seen.add(key)
    running = {"script": "script", "local": "local", "objscript": "obj"}
    for k, v in facts.items():
        head, _, n = k.partition(":")
        if v and head in running and n.isdigit() and unit[running[head]](int(n)) not in seen:
            out.append((unit[running[head]](int(n)), None))
    return sorted(out, key=str)


def callees(x: ir.Ins, units: dict, room: int) -> list[tuple[tuple, int]] | None:
    """(IR unit key, start index) of the script a start runs: startScript or chainScript of a constant
    n runs global n when the IR has it, else the room's local n (runScript, script.cpp:68-80);
    startObject of a constant object runs its entry for the verb, the default entry when the verb has
    none (getVerbEntrypoint, script.cpp:242-248), every entry when the verb is read at run time. None
    when the target is read at run time."""
    st = x.st
    if st.name in ("startScript", "chainScript"):
        if not isinstance(st.script, ir.Const):
            return None
        n = st.script.value
        return [(("global", n) if ("global", n) in units else ("local", room, n), 0)]
    if not isinstance(st.obj, ir.Const):
        return None
    key = ("obj", st.obj.value)
    u = units.get(key)
    if u is None:
        return []
    if isinstance(st.script, ir.Const):
        e = u.entries.get(st.script.value, u.entries.get(ir.DEFAULT_VERB))
        return [(key, e)] if e is not None else []
    return [(key, e) for e in sorted(set(u.entries.values()))]


def live_draws(facts: dict[str, int], units: dict | None = None) -> tuple[dict[str, list[str]], dict[str, int]]:
    """{fact key: cites} of the draws a live script will run again (module docstring, "Random facts"),
    and the counts. From each live slot's W, along ``next``, jumps and Esc's override jump, and into the
    scripts it starts with a constant target, transitively (``callees``). Not followed, counted: starts
    whose target is read at run time, and ops that run a hook script (end.HOOK_OPS: the hook's number is
    a variable). Also counted: live slots, slots read whole (no ``at:`` key, or W no instruction),
    units missing from the IR, indexed draws (target unknown)."""
    units = units if units is not None else ir.load()
    room = facts.get(f"var:{VAR_ROOM_RESOURCE}", 0)
    out = collections.defaultdict(list)
    counts = collections.Counter({k: 0 for k in (
        "live slots", "read whole: no at: key", "read whole: W is no instruction", "units missing from the IR",
        "scripts started with a constant target (followed)", "starts with a target read at run time (not followed)",
        "ops that run a hook script (not followed)", "indexed draws (target unknown)")})
    todo = []
    for key, w in live_units(facts, units):
        counts["live slots"] += 1
        u = units.get(key)
        if u is None or not u.ins:
            counts["units missing from the IR"] += 1
            continue
        at = {x.offset: i for i, x in enumerate(u.ins)}
        if w is None or w not in at:
            counts["read whole: no at: key" if w is None else "read whole: W is no instruction"] += 1
            todo += [(key, i) for i in range(len(u.ins))]
        else:
            todo.append((key, at[w]))
    seen, started = set(), set()
    while todo:
        key, i = todo.pop()
        u = units.get(key)
        if u is None or not u.ins:
            counts["units missing from the IR"] += 1
            continue
        if i >= len(u.ins) or (key, i) in seen:
            continue
        seen.add((key, i))
        x = u.ins[i]
        todo += [(key, j) for j in (x.next, x.target, x.esc) if j is not None] if x.ends != "always" else []
        if isinstance(x.st, ir.Start):
            run = callees(x, units, room)
            if run is None:
                counts["starts with a target read at run time (not followed)"] += 1
            for c in run or []:
                if c not in started:
                    started.add(c)
                    counts["scripts started with a constant target (followed)"] += 1
                    todo.append(c)
        elif isinstance(x.st, ir.Op) and x.st.name in HOOK_OPS:
            counts["ops that run a hook script (not followed)"] += 1
        for a in _assigns(x):
            if not _reads(a.rhs, key, set()):
                continue
            t = _target(a.lhs, key)
            if t is None:
                counts["indexed draws (target unknown)"] += 1
            elif _key(t) and x.cite() not in out[_key(t)]:
                out[_key(t)].append(x.cite())
    return {k: sorted(v) for k, v in out.items()}, dict(counts)


# --- run, write, check -----------------------------------------------------------------------------

def load(path: Path = OUT) -> dict[int, dict]:
    return {r["seed"]: r for r in (json.loads(l) for l in path.read_text().splitlines() if l.strip())}


def write_atomic(path: Path, text: str) -> None:
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(text)
    os.replace(tmp, path)


def report(rows: list[dict], again: dict | None, wall: float) -> list[str]:
    log = [f"start: first idle stop from boot, no input, seeds {[r['seed'] for r in rows]}; wall time {wall:.0f} s",
           f"engine: {rows[0]['argv']}",
           "ini (game section): " + rows[0]["ini"].split("\n\n")[1].replace("\n", "; ").strip("; "),
           f"bridge: {engine.bridge_version()}"]
    for r in rows:
        st, d, f = r["stop"], r["dump"], r["facts"]
        bad = [why for ok, why in (
            (st["stop"] == "idle", f"stop {st['stop']!r}, not idle"),
            (st["idle"] == "sentence", f"idle kind {st['idle']!r}, not sentence"),
            (d["seed"] == r["seed"], f"dump seed {d['seed']}"),
            (d["boot_param"] == BOOT_PARAM, f"dump boot_param {d['boot_param']}"),
            (f.get(f"var:{VAR_DEBUGMODE}", 0) == 0, f"var {VAR_DEBUGMODE} (VAR_DEBUGMODE) is {f.get(f'var:{VAR_DEBUGMODE}')}"),
            (set(r["trace"]) <= TRACE_TYPES, f"trace record types not allowed: {sorted(set(r['trace']) - TRACE_TYPES)}"),
            (r["facts_py_sha256"] == facts_sha256(), "facts.py changed during the run"),
        ) if not ok]
        log.append(f"seed {r['seed']}: idle stop at frame {st['frame']} (tick {st['tick']}), room {st['room']}, "
                   f"kind {st['idle']}, ego {d['ego']} at {d['ego_pos']}, rng {st['rng']}; control stop at frame "
                   f"{r['control_stop']['frame']} (room {r['control_stop']['room']}, "
                   f"{'idle ' + str(r['control_stop']['idle']) if r['control_stop'].get('idle') else 'blocked: ' + str(r['control_stop'].get('blk'))}); "
                   f"facts {len(f)}; checks {'ok' if not bad else 'FAIL ' + str(bad)}")
        log.append(f"  trace records up to the stop: {r['trace']}")
        log.append(f"  random (redrawn by a live script): {r['random']}; counts {r['random_counts']}")
    if again is not None:
        r0 = next(r for r in rows if r["seed"] == again["seed"])
        same = r0["facts"] == again["facts"] and r0["stop"]["frame"] == again["stop"]["frame"]
        log.append(f"determinism: seed {again['seed']} booted a second time: stop frame {again['stop']['frame']}, "
                   f"rng {again['stop']['rng']}; facts {'identical' if same else 'DIFFER'} "
                   f"({len(set(r0['facts'].items()) ^ set(again['facts'].items()))} key/value pairs differ)")
    counts = [kind_counts(r["facts"]) for r in rows]
    kinds = sorted(set().union(*counts))
    log.append("facts by kind (keys / keys not 0), per seed:")
    for k in kinds:
        per = [c.get(k, (0, 0)) for c in counts]
        log.append(f"  {k}: " + (f"{per[0][0]} / {per[0][1]}" if len(set(per)) == 1 else
                                  ", ".join(f"s{r['seed']} {a} / {b}" for r, (a, b) in zip(rows, per))))
    small = {k: v for k, v in rows[0]["facts"].items() if fact_kind(k) not in ("var", "owner", "state", "class")}
    log.append(f"seed {rows[0]['seed']}'s facts other than var, owner, state and class: {small}")
    log.append(f"  states not 0: {[(k, v) for k, v in rows[0]['facts'].items() if k.startswith('state:') and v]}; "
               f"owners that are 0: {sum(k.startswith('owner:') and not v for k, v in rows[0]['facts'].items())}")
    log.append(f"facts.py sha256 {rows[0]['facts_py_sha256'][:16]}... (stored per row)")
    diff = differing(rows)
    rnd, indexed = random_keys()
    by_kind = collections.Counter(fact_kind(k) for k in diff)
    frames = sorted({r["stop"]["frame"] for r in rows})
    log.append(f"stop frames: {frames} ({'all the same' if len(frames) == 1 else 'they differ'})")
    log.append(f"keys whose value differs across the seeds: {len(diff)}, by kind {dict(sorted(by_kind.items()))}")
    log.append(f"the wide net, random-written keys anywhere in the IR (data flow from getRandomNr, var 0 per "
               f"script: {sum(v == 'data' for v in rnd.values())}; inside an if on such a value: "
               f"{sum(v == 'control' for v in rnd.values())}; indexed writes skipped: {indexed})")
    unexplained = []
    for k, vals in diff.items():
        sites = sorted({c for r in rows for c in r["random_sites"].get(k, [])})
        if not all(k in r["random"] for r in rows):
            unexplained.append(k)
        log.append(f"  {k}: {vals}  [in every row's random: {k not in unexplained}; wide net: {rnd.get(k, 'no')}]")
        log.append(f"    draws into it a live script runs again: {sites or 'none'}")
    log.append(f"differing keys that are not redrawn by a live script: {len(unexplained)} {unexplained}")
    same = [k for k in sorted(set().union(*(r["random"] for r in rows))) if k not in diff]
    log.append(f"random keys whose value happens to be the same on every seed: {len(same)} {same}")
    return log


def main(argv: list[str]) -> None:
    seeds = [int(a) for a in argv] or list(SEEDS)
    t0 = time.time()
    units = ir.load()
    jobs = [(s, ENGINE_DIR / f"seed-{s}", True, units) for s in seeds] + [(seeds[0], ENGINE_DIR / f"seed-{seeds[0]}-again", False, units)]
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(jobs)) as pool:
        done = list(pool.map(lambda j: boot(*j), jobs))
    rows, again = done[:-1], done[-1]
    wall = time.time() - t0
    OUT.parent.mkdir(exist_ok=True)
    keep = ("seed", "stop", "control_stop", "first_stop", "dump", "trace", "facts_py_sha256", "random", "random_sites",
            "random_counts", "facts")
    write_atomic(OUT, "".join(json.dumps({k: r[k] for k in keep}) + "\n" for r in rows))
    text = "\n".join(report(rows, again, wall)) + "\n"
    write_atomic(LOG, text)
    print(text, end="")


if __name__ == "__main__":
    main(sys.argv[1:])
