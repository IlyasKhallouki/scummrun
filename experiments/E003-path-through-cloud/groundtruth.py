"""E003 step 3: the engine's ground truth for the model check, Part I of run B.

usage: uv run python experiments/E003-path-through-cloud/groundtruth.py [EARLIER.jsonl]
       from groundtruth import load; rows = load()   (the rows with full fact maps)

An instrument, not part of the system: it reads the human route and run B's recorded inputs,
which the system never sees (plan.md, step 3 and C2). Nothing the system runs imports it.

It replays run B's verified explicit-key movie (``runs/anypercent/verify-b1/movie-explicit.jsonl``)
from boot in one engine process, with the run's settings (``scripts/anypercent_verify.py``: DOS
CD, seed 836557856, talkspeed 255, segment start at frame 46, var 19 free, the bridge's automatic
'.'/Esc off because the movie holds them as keys), and walks forward through the frames in order,
reading the engine's state (``facts.facts_of``) at the boundaries between run B's Part I moves.
The moves are the entries of ``runs/anypercent/noslide-p7/results.jsonl`` named S1#, S2# or S3#.
The movie is local (gitignored); to make it again:

    uv run python scripts/anypercent_verify.py runs/anypercent/noslide-p7/movie.jsonl \\
        runs/anypercent/verify-b1 --explicit --goal '[{"room": 0}, {"var": 293, "eq": 161}]'

**Stop rule.** ``run(frame=N)`` stops at frame N's decision point (``SpeedrunBridge::onDecisionPoint``,
scumm.cpp ``scummLoop``). In frame N the bridge first applies the inputs scheduled for N
(``onFrameBegin`` -> ``applyRawInputs``); the engine then reads keys (``processInput``), runs the
scripts (``runAllScripts``) and takes clicks (``checkExecVerbs``), and only then stops. So the stop
at N already holds N's input, and the last stop before an input at frame F takes effect is F - 1.
The sentence script a frame queues starts after its stop (``checkAndRunSentenceScript``).

**Boundaries.** A move's window is the frames after the previous move's done frame up to its own
done frame. Its first input F is the first input of the run's own movie (``noslide-p7/movie.jsonl``,
the executor's keys and clicks) in that window. The explicit movie also holds the bridge's automatic
'.' and Esc; those that come before the move's first own input end the previous move's text or
cutscene, so they belong to the previous move. A move with no own input whose window holds such a key
(the bridge did the move) starts at that key. A move with no input at all (the route's "only if
closed" doors, already open: results.jsonl marks them ``skipped``) takes the next move's boundary.

* ``pre``: the facts at frame F - 1;
* ``done``: the facts at the move's recorded done frame (results.jsonl ``done``), the end frame
  plan.md's step 3 reads; for a move with no input, its pre (it has no state of its own);
* ``post``: the next move's pre, so it adds what happened between the done frame and the next
  input; for the last Part I move, its done frame (S4#1's Esc is the next frame).

Each row also has the route move of that name (``route.full_route()``, matched on the "S1#4"
label; the route file's own ``kind``, ``glitch`` and ``room`` added, ``cite`` and ``note`` left out),
two glitch marks (``glitch`` from the glitch table in ``routes/anypercent/README.md``, the source
plan.md names; ``glitch_route`` from the move's own ``kind``/``glitch`` fields), and from
results.jsonl ``skipped``, ``how`` (how the executor found the move) and ``via`` (what it played).
``composite`` marks a row whose ``via`` holds more than one action: the row stands for several
moves, and ``move`` holds only the last one.

Each choose move also has ``clicked_verb``: the dialog verb run B clicked, found in the pre state's
verbs by the executor's own rule (``executor.py`` ``_choose``): the one visible verb in the
dialog id range with a name whose text contains ``move.choose``, case-insensitive, after
``choose_by`` has picked the line from the variable it names. A key digit (the executor presses
it) picks what the engine picks: the first visible verb, in slot order among all verbs, whose key
is that digit (verbs.cpp:701-710; the dump lists visible verbs in slot order), and it counts only
when that verb is a dialog line. Run B's Part I never uses a key digit, so that branch is untested.
When this does not resolve, ``clicked_verb`` is null and ``clicked_verb_reason`` says why. The
route move and the executor's profile (the dialog id range) are instrument inputs.

Writes ``results/groundtruth-part1.jsonl`` (one move per line) and ``results/groundtruth-part1.log``,
each to a temporary file that then replaces the old one, so a reader never sees half a file.
To keep the file small, the first row holds its full facts as ``start``, and every row holds
``done_diff`` and ``post_diff`` (``facts.diff`` from its pre, keys sorted, so two runs write the
same bytes); a row's pre is the previous row's post. ``load()`` puts the full maps back as
``pre``, ``done`` and ``post``.
The engine's trace, ini and logs go to ``out/E003-groundtruth``.

**Checks** (in the log):

* determinism: every fact kind an earlier file has (``at:script``, ``at:local``, ... count as kinds
  of their own) must be identical in all 750 maps; kinds new to this run are listed. The earlier
  file is ``EARLIER.jsonl``, by default the file this run replaces, read before it is replaced;
* script positions: every ``at:`` key's W is an instruction offset of its ``ir.py`` unit (a local,
  entry or exit key's unit is that of the room resource in VAR_ROOM_RESOURCE, var 22, room.cpp:
  171-172, so a pseudo-room maps to its resource); a global slot's ``code`` is its ``offs`` minus
  the resource header; slots whose ``code`` is -1, by kind; live slots by kind (an exit-code slot
  cannot be live at a dump, facts.py ``at:``);
* freeze counts: the dump's ``frozen`` per live slot, of which ``frozen:`` keeps only whether it is
  above 0, and how many dumps hold a count above 1 (where the count says more than the fact);
* where W lands, per live slot: after a breakHere or delay, at a wait, right after an opcode that
  runs a script nested (``NESTED``; the caller stays there when the callee froze it, facts.py
  ``at:``), at the script's start or an object entry, at an override jump, or none of these
  ("other", listed);
* instances that share a key: slots of one unit at one W in one dump, which ``facts_of`` folds
  into one ``at:`` key;
* Esc: every ``esc:`` key's W is a jump ``ir.py`` marks as an override's (``esc``); the overrides
  the dump calls stale, which ``facts_of`` leaves out; armed overrides while VAR_CUTSCENEEXIT_KEY
  is 0 (Esc off; ``esc:`` does not test it);
* ``clicked_verb``: how many choose moves it resolves, and whether run B's own click lands on that
  line's point (the executor clicks the line's top left corner plus 2, 2).
"""

import collections
import dataclasses
import json
import os
import re
import sys
import time
from pathlib import Path

import ir
from facts import HEADER, WIO_FLOBJECT, WIO_GLOBAL, WIO_INVENTORY, WIO_ROOM, diff, facts_of, patch, unit_of
from speedrun import paths, releases, route
from speedrun.control import Controller
from speedrun.engine import EngineConfig
from speedrun.executor import DIALOG_FIRST, DIALOG_LAST

HERE = Path(__file__).resolve().parent
OUT = HERE / "results" / "groundtruth-part1.jsonl"
RUN = paths.ROOT / "runs" / "anypercent" / "noslide-p7"
MOVIE = paths.ROOT / "runs" / "anypercent" / "verify-b1" / "movie-explicit.jsonl"
KEY = paths.ROOT / "experiments" / "E001-goals-from-scripts" / "key.json"
ENGINE_DIR = paths.OUT_DIR / "E003-groundtruth"
SEED = 836557856
PART_I = ("S1#", "S2#", "S3#")
PERSISTENT = ("bit:", "owner:", "state:", "class:", "room")  # fact kinds a room change or a move keeps
OBJECT_WHERE = {WIO_INVENTORY: "inventory", WIO_ROOM: "room", WIO_FLOBJECT: "flobject"}
SLOT_KINDS = ["script", "local", "entry", "exit"] + [f"obj ({w})" for w in OBJECT_WHERE.values()]
VAR_ROOM_RESOURCE = 22  # vars.cpp:58, set to _roomResource on every room load (room.cpp:171-172)
ROOM_RESOURCE = f"var:{VAR_ROOM_RESOURCE}"
# VAR_CUTSCENEEXIT_KEY (vars.cpp:60): Esc reaches abortCutscene only while it is non-zero (input.cpp:966, 1423)
CUTSCENEEXIT_KEY = 24
# Opcodes that run a script nested, in the same frame, inside the calling script (E002 engine-opcodes.json
# "starts"): startScript and startObject, a room change's exit and entry code (room.cpp:78, 254), the
# inventory hook (pickupObject, setOwnerOf, setObjectName, actorFollowCamera), the scroll hook
# (setCameraAt) and the cutscene hooks. chainScript is left out: its caller is dead (script_v5.cpp:834-851).
# The other rows with "starts" (beginOverride, freezeScripts, stopScript, stopObjectCode, doSentence,
# panCameraTo, soundKludge, walkActorTo, systemOps) run no script inside the caller in that frame.
NESTED = {"startScript", "startObject", "loadRoom", "loadRoomWithEgo", "actorFollowCamera", "pickupObject",
          "setOwnerOf", "setObjectName", "setCameraAt", "cutscene", "endCutscene"}


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def load(path: Path = OUT) -> list[dict]:
    """The rows of ``path`` with the full fact maps ``pre``, ``done`` and ``post`` put back."""
    rows, pre = load_jsonl(path), None
    for r in rows:
        pre = r.get("start", pre)
        r["pre"], r["done"], r["post"] = pre, patch(pre, r["done_diff"]), patch(pre, r["post_diff"])
        pre = r["post"]
    return rows


def is_input(e: dict) -> bool:
    return any(k in e for k in ("key", "click", "rclick", "move"))


def readme_glitches() -> set[str]:
    """Labels ("S1#48") of the README table's glitch rows. The README numbers S1 and S2 moves from 1
    and S3-S7 from 0 (its "Move numbers follow each file's own convention" note); labels are 1-based."""
    text = (route.ROUTE_DIR / "README.md").read_text()
    table = text.split("## RNG points and glitches", 1)[1].split("\n## ", 1)[0]
    out = set()
    for line in table.splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 3 or "glitch" not in cells[2]:
            continue
        for m in re.finditer(r"\b(S\d) (m\d+(?:-\d+)?(?:, m?\d+(?:-\d+)?)*)", cells[1]):
            shift = 0 if m[1] in ("S1", "S2") else 1
            for part in m[2].split(","):
                a, _, b = part.strip().lstrip("m").partition("-")
                out.update(f"{m[1]}#{n + shift}" for n in range(int(a), int(b or a) + 1))
    return out


def route_moves() -> dict[str, dict]:
    """Label -> the route move as a plain dict: executor.Move's fields, plus the route file's
    kind, glitch and room (route.to_move drops them)."""
    raw = {}
    for seg, fname in route.SEGMENTS.items():
        for n, m in enumerate(json.loads((route.ROUTE_DIR / fname).read_text())["moves"], 1):
            raw[f"{seg}#{n}"] = m
    out = {}
    for mv in route.full_route():
        label = mv.name.split(" ")[0]
        out[label] = dataclasses.asdict(mv) | {k: raw[label].get(k) for k in ("kind", "glitch", "room")}
    return out


def first_inputs(moves: list[dict], own: list[dict], explicit: list[dict]) -> list[tuple[int | None, str]]:
    """Per move: the frame of its first input and where it came from ("own", "auto" or "none")."""
    out, prev = [], 0
    for r in moves:
        mine = [e["frame"] for e in own if prev < e["frame"] <= r["done"]]
        auto = [e["frame"] for e in explicit if prev < e["frame"] <= r["done"]]
        out.append((min(mine), "own") if mine else (min(auto), "auto") if auto else (None, "none"))
        prev = r["done"]
    return out


def boundaries(moves: list[dict], firsts: list[int | None]) -> list[tuple[int, int, int]]:
    """(pre, done, post) frames per move: pre is F - 1 of its first input (or of the next move that
    has one), done its done frame (its pre if it has no input), post the next move's pre."""
    pre, nxt = [0] * len(moves), moves[-1]["done"] + 1
    for k in range(len(moves) - 1, -1, -1):
        nxt = firsts[k] if firsts[k] is not None else nxt
        pre[k] = nxt - 1
    done = [r["done"] if f is not None else p for r, f, p in zip(moves, firsts, pre)]
    post = pre[1:] + [moves[-1]["done"]]
    return list(zip(pre, done, post))


def replay(stops: list[int], dumps: set[int]) -> tuple[dict, dict, dict]:
    """Room at every stop frame, and the facts and the raw slots, cutscene stack, verbs and vars
    at the frames in ``dumps``, from one engine run."""
    ENGINE_DIR.mkdir(parents=True, exist_ok=True)
    rel = releases.get("dos-cd")
    cfg = EngineConfig(out_dir=ENGINE_DIR, game_path=releases.game_dir(rel), release="dos-cd", timer_next="any",
                       start_frame=46, max_ticks=10**7, seed=SEED, raw_inputs=MOVIE,
                       skip_text=False, skip_cutscenes=False, game_settings={"talkspeed": "255"})
    rooms, facts, raw = {}, {}, {}
    with Controller(cfg, timeout_s=600) as c:
        b = c.root
        for f in stops:
            st = b.run(frame=f)
            if st.get("frame") != f:
                raise SystemExit(f"the replay stopped at {st} instead of frame {f}")
            rooms[f] = st["room"]
            if f in dumps:
                s = b.state()
                assert s["frame"] == f
                facts[f] = facts_of(s)
                raw[f] = {k: s[k] for k in ("slots", "cutscene", "verbs", "vars")}
    return rooms, facts, raw


def clicked_verb(move: dict, state: dict) -> tuple[int | None, str | None]:
    """The dialog verb ``move`` clicks in ``state``, by executor.py ``_choose``'s rule, or None and
    why. A key digit picks the first visible verb, in slot order among all verbs, whose key is that
    digit, as checkExecVerbs matches a key (verbs.cpp:701-710); ``state["verbs"]`` lists the visible
    verbs in slot order."""
    lines = [v for v in state["verbs"] if DIALOG_FIRST <= v["id"] <= DIALOG_LAST and v["name"]]
    if not lines:
        return None, "no dialog line is visible in the pre state"
    choose = move["choose"]
    if move.get("choose_by"):
        said = state["vars"][move["choose_by"]["var"]]
        choose = move["choose_by"]["map"].get(str(said))
        if choose is None:
            return None, f"choose_by: var {move['choose_by']['var']} is {said}, which has no planned reply"
    if isinstance(choose, int):
        if not 0 <= choose <= 9:
            return None, f"key {choose!r} is not one digit"
        first = next((v for v in state["verbs"] if v["key"] == ord(str(choose))), None)
        if first is None:
            return None, f"no visible verb has key {choose!r}"
        if first not in lines:
            return None, f"key {choose!r} first matches verb {first['id']} {first['name']!r}, not a dialog line"
        return first["id"], None
    hits = [v for v in lines if choose.lower() in v["name"].lower()]
    if len(hits) != 1:
        return None, f"{len(hits)} visible lines match {choose!r}: {[v['name'] for v in lines]}"
    return hits[0]["id"], None


def fact_kind(key: str) -> str:
    """A fact key's kind: its prefix, with the unit kind for ``at:`` and ``esc:`` (at:script, at:local,
    at:obj, at:entry, at:exit, esc:script, ...); a bare key (room, ego, cutscene) is its own kind."""
    head, _, rest = key.partition(":")
    return f"{head}:{rest.split(':')[0]}" if head in ("at", "esc") else head


def ir_unit(unit: str, facts: dict) -> tuple:
    """The ``ir.load()`` key of a fact's unit (script:N, local:N, obj:O, entry, exit) in ``facts``."""
    t, _, n = unit.partition(":")
    if t in ("entry", "exit"):
        return t, facts.get(ROOM_RESOURCE, 0)
    if t == "local":
        return "local", facts.get(ROOM_RESOURCE, 0), int(n)
    return "global" if t == "script" else "obj", int(n)


def slot_kind(slot: dict) -> str:
    """A live slot's kind for the counts: its unit kind, and for an object where its code lives."""
    t = (unit_of(slot["where"], slot["number"]) or "?").split(":")[0]
    return f"obj ({OBJECT_WHERE[slot['where']]})" if t == "obj" else t


def stop_kind(u: ir.Unit, i: int) -> str:
    """Where a live slot waits, by the instruction ``i`` of its IR unit ``u`` that it resumes at
    (facts.py ``at:``)."""
    prev = u.ins[i - 1] if i > 0 else None
    if prev is not None and prev.yields in ("frame", "delay"):
        return "after breakHere or delay"
    if u.ins[i].yields == "wait":
        return "at a wait"
    if i == 0 or i in u.entries.values():
        return "at the start or an entry"
    if any(x.esc == i for x in u.ins):
        return "at an override jump"
    if prev is not None and call_name(prev) in NESTED:
        return "after a nested call"
    return "other"


def call_name(ins: ir.Ins) -> str | None:
    """The opcode name of a Start or Op statement (startScript, loadRoom, ...), else None."""
    return ins.st.name if isinstance(ins.st, (ir.Start, ir.Op)) else None


def split_key(key: str) -> tuple[str, int]:
    """``at:local:205:129`` -> ("local:205", 129); ``esc:entry:40`` -> ("entry", 40)."""
    unit, _, w = key.split(":", 1)[1].rpartition(":")
    return unit, int(w)


def check_positions(rows: list[dict], raw: dict, old: list[dict] | None) -> list[str]:
    """The checks the module docstring lists, as log lines."""
    log, maps = [], [(r["label"], s, r[s]) for r in rows for s in ("pre", "done", "post")]

    # (a) determinism against the file this run replaces
    if old is None:
        log.append("determinism: no earlier file to compare with")
    elif len(old) != len(rows) or any(a["name"] != b["name"] for a, b in zip(old, rows)):
        log.append(f"determinism: the earlier file has other moves ({len(old)} rows); not compared")
    else:
        kinds = {fact_kind(k) for r in old for s in ("pre", "done", "post") for k in r[s]}
        bad = [(r["label"], s, sorted(diff(o[s], {k: v for k, v in r[s].items() if fact_kind(k) in kinds}))[:4])
               for o, r in zip(old, rows) for s in ("pre", "done", "post")
               if diff(o[s], {k: v for k, v in r[s].items() if fact_kind(k) in kinds})]
        new = sorted({fact_kind(k) for _, _, m in maps for k in m} - kinds)
        log.append(f"determinism: {len(kinds)} fact kinds in the earlier file, compared in all {len(maps)} maps: "
                   f"{len(bad)} maps differ {bad[:5]}; kinds new in this file: {new}")

    # (b) at: keys against the IR, (c) esc: keys against the IR's override jumps
    units = ir.load()
    escs = {k: {u.ins[x.esc].offset for x in u.ins if x.esc is not None} for k, u in units.items()}
    for prefix in ("at", "esc"):
        seen = {}  # (fact kind, IR unit, W) -> first label that has it
        for label, _, m in maps:
            for k in m:
                if k.startswith(prefix + ":"):
                    unit, w = split_key(k)
                    seen.setdefault((fact_kind(k), ir_unit(unit, m), w), label)
        by = collections.defaultdict(collections.Counter)
        bad = collections.defaultdict(list)
        for (kd, key, w), label in seen.items():
            if key not in units:
                why = "no IR unit"
            elif prefix == "at" and w not in units[key].at:
                why = "W is not an instruction offset"
            elif prefix == "esc" and w not in escs[key]:
                why = "W is not an override jump"
            else:
                why = "ok"
            by[kd][why] += 1
            if why != "ok":
                bad[kd].append((key, w, why, label))
        what = "an instruction offset of its unit" if prefix == "at" else "an override jump of its unit"
        log.append(f"`{prefix}:` keys (distinct unit and W over all maps) whose W is {what}:")
        for kd in sorted(by):
            wrong = {why: c for why, c in by[kd].items() if why != "ok"}
            log.append(f"  {kd}: {by[kd]['ok']} of {sum(by[kd].values())}; mismatches {dict(sorted(wrong.items()))} "
                       f"{sorted(bad[kd])[:4]}")
        if not by:
            log.append("  none")

    # raw slots: code against offs for global slots, code == -1 by kind, the override stack
    slots = [(f, s) for f in sorted(raw) for s in raw[f]["slots"]]
    by_unit = collections.Counter(slot_kind(s) for _, s in slots)
    unknown = collections.Counter(slot_kind(s) for _, s in slots if s["code"] < 0)
    glob = [(f, s) for f, s in slots if s["where"] == WIO_GLOBAL]
    off = [(f, s["number"], s["offs"], s["code"]) for f, s in glob if s["code"] != s["offs"] - HEADER]
    never = [k + (" (cannot occur: facts.py at:)" if k == "exit" else "") for k in SLOT_KINDS if k not in by_unit]
    log.append(f"live slots over the {len(raw)} dumps: {sum(by_unit.values())}, by unit kind {dict(by_unit)}; "
               f"code == -1: {sum(unknown.values())} {dict(unknown)}; kinds no dump has a live slot of: {never}")
    log.append(f"global slots whose code is not offs - {HEADER}: {len(off)} of {len(glob)} {off[:5]}")
    counts = collections.Counter(s["frozen"] for _, s in slots)
    frozen_kind = collections.Counter(slot_kind(s) for _, s in slots if s["frozen"] > 0)
    log.append(f"freeze counts of the live slots over the dumps (the dump's `frozen`; `frozen:` keeps only whether "
               f"it is above 0): {dict(sorted(counts.items()))}; frozen slots by unit kind {dict(frozen_kind)}; "
               f"dumps with a count above 1: {sum(any(s['frozen'] > 1 for s in raw[f]['slots']) for f in raw)}")

    # where W lands, and slots that share one at: key
    lands, calls, parked, parked_kind, parked_keys, other = (collections.Counter(), collections.Counter(),
                                                            collections.Counter(), collections.Counter(), set(), [])
    shared, shared_dumps = collections.Counter(), 0
    for f in sorted(raw):
        here = {ROOM_RESOURCE: raw[f]["vars"][VAR_ROOM_RESOURCE]}
        per = collections.Counter()
        for s in raw[f]["slots"]:
            unit = unit_of(s["where"], s["number"])
            if unit is None or s["code"] < 0:
                continue
            per[unit, s["code"]] += 1
            u = units.get(ir_unit(unit, here))
            i = u.at.get(s["code"]) if u else None
            if i is None:
                lands["not an instruction (above)"] += 1
                continue
            why = stop_kind(u, i)
            lands[why] += 1
            if why == "after a nested call":
                calls[call_name(u.ins[i - 1])] += 1
                parked["frozen" if s["frozen"] else "not frozen"] += 1
                parked_kind[unit.split(":")[0]] += 1
                parked_keys.add((unit, s["code"]))
            elif why == "other":
                other.append((f, unit, s["code"], u.ins[i].text))
        extra = {k: n - 1 for k, n in per.items() if n > 1}
        shared_dumps += bool(extra)
        for (unit, _), n in extra.items():
            shared[unit.split(":")[0]] += n
    log.append(f"where W lands, over the {sum(lands.values())} live slot readings with a known code: "
               f"{dict(sorted(lands.items()))}")
    log.append(f"  after a nested call (the callee froze the caller): by call {dict(sorted(calls.items()))}; "
               f"at the dump {dict(sorted(parked.items()))}; by unit kind {dict(sorted(parked_kind.items()))}; "
               f"{len(parked_keys)} distinct `at:` keys")
    log.append(f"  other (no rule above): {len(other)} {other[:8]}")
    log.append(f"slots that share one `at:` key with another slot of the same unit at the same W (beyond the "
               f"first): {sum(shared.values())} in {shared_dumps} of {len(raw)} dumps, by unit kind "
               f"{dict(sorted(shared.items()))}")
    tops = [(f, raw[f]["cutscene"]["levels"][-1]["override"]) for f in sorted(raw)]
    armed = [(f, o) for f, o in tops if o is not None]
    lower = sum(lv["override"] is not None for f in raw for lv in raw[f]["cutscene"]["levels"][:-1])
    depth = collections.Counter(raw[f]["cutscene"]["depth"] for f in raw)
    log.append(f"cutscene depth over the dumps: {dict(sorted(depth.items()))}; top level override armed in "
               f"{len(armed)} dumps (stale {sum(o['stale'] for _, o in armed)}, code -1 "
               f"{sum(o['code'] < 0 for _, o in armed)}, by unit kind "
               f"{dict(collections.Counter(unit_of(o['where'], o['number']).split(':')[0] for _, o in armed))}); "
               f"an override armed below the top level: {lower}")
    stale = [(f, o["number"], o["where"], o["status"]) for f, o in armed if o["stale"]]
    if stale:
        log.append(f"  stale overrides (frame, number, where, status): {stale[:8]}")
    exit_key = collections.Counter(raw[f]["vars"][CUTSCENEEXIT_KEY] for f, _ in armed)
    log.append(f"  VAR_CUTSCENEEXIT_KEY (var {CUTSCENEEXIT_KEY}) in the armed dumps: {dict(sorted(exit_key.items()))}; "
               f"armed while it is 0 (Esc off): {exit_key[0]}")
    return log


def main() -> None:
    t0 = time.time()
    moves = [r for r in load_jsonl(RUN / "results.jsonl") if r["name"].startswith(PART_I)]
    explicit = [e for e in load_jsonl(MOVIE) if is_input(e)]
    own = [e for e in load_jsonl(RUN / "movie.jsonl") if is_input(e)]
    firsts = first_inputs(moves, own, explicit)
    bounds = boundaries(moves, [f for f, _ in firsts])
    dumps = {f for frames in bounds for f in frames}
    stops = sorted(dumps | {r["done"] for r in moves})
    rooms, facts, raw = replay(stops, dumps)
    earlier = Path(sys.argv[1]) if len(sys.argv) > 1 else OUT
    old = load(earlier) if earlier.exists() else None  # for the determinism check, read before OUT is replaced

    defs, readme = route_moves(), readme_glitches()
    OUT.parent.mkdir(exist_ok=True)
    rows, tmp = [], OUT.with_name(OUT.name + ".tmp")
    with tmp.open("w") as fh:
        for i, (r, (first, src), (pre, done, post)) in enumerate(zip(moves, firsts, bounds)):
            label = r["name"].split(" ")[0]
            mv = defs.get(label)
            row = {"i": i, "name": r["name"], "result_i": r["i"], "move": mv, "glitch": label in readme,
                   "glitch_route": bool(mv and (mv["kind"] == "glitch" or mv["glitch"])),
                   "skipped": bool(r.get("skipped")), "composite": len(r.get("via") or []) > 1,
                   "how": r.get("how"), "via": r.get("via"), "first_input": first, "input_from": src,
                   "pre_frame": pre, "done_frame": done, "post_frame": post, "room_pre": rooms[pre],
                   "room_done": rooms[r["done"]], "room_post": rooms[post]}
            if mv and mv["kind"] == "choose":
                row["clicked_verb"], why = clicked_verb(mv, raw[pre])
                if why:
                    row["clicked_verb_reason"] = why
            if i == 0:
                row["start"] = facts[pre]
            row["done_diff"] = dict(sorted(diff(facts[pre], facts[done]).items()))  # sorted: diff's order
            row["post_diff"] = dict(sorted(diff(facts[pre], facts[post]).items()))  # comes from a set
            fh.write(json.dumps(row) + "\n")
            rows.append(row | {"room": r.get("room"), "label": label, "recorded_done": r["done"],
                               "pre": facts[pre], "done": facts[done], "post": facts[post]})
    os.replace(tmp, OUT)
    wall = time.time() - t0

    log = [f"run B Part I moves: {len(rows)} (results.jsonl names S1#/S2#/S3#), replayed to frame {stops[-1]} "
           f"({len(stops)} stops, {len(dumps)} state dumps), wall time {wall:.0f} s"]
    back = load()
    bad = sum(bool(diff(a[k], b[k])) for a, b in zip(rows, back) for k in ("pre", "done", "post"))
    log.append(f"{OUT.name}: {OUT.stat().st_size / 1e6:.2f} MB; load() rebuilds every pre, done and post: "
               f"{bad} of {3 * len(rows)} differ")
    unmatched = [r["name"] for r in rows if r["move"] is None]
    log.append(f"matched route moves: {len(rows) - len(unmatched)}; move=null: {len(unmatched)} {unmatched}")
    g_readme = [r["label"] for r in rows if r["glitch"]]
    g_route = [r["label"] for r in rows if r["glitch_route"]]
    log.append(f"glitch moves (README table, used as `glitch`): {len(g_readme)} {g_readme}")
    log.append(f"glitch moves (route kind/glitch fields, `glitch_route`): {len(g_route)} {g_route}")
    log.append(f"  the two disagree on: {sorted(set(g_readme) ^ set(g_route))}")
    log.append(f"  README glitch labels in Part I that run B does not play: "
               f"{sorted(g for g in readme if g.startswith(PART_I) and g not in {r['label'] for r in rows})}")
    via = [(r["label"], r["via"]) for r in rows if r["via"]]
    log.append(f"entries that stand for other moves (results.jsonl `via`): {len(via)}")
    log += [f"  {lab}: {v}" for lab, v in via]
    log.append(f"skipped (results.jsonl): {[r['label'] for r in rows if r['skipped']]}; "
               f"composite (via holds more than one action): {[r['label'] for r in rows if r['composite']]}")
    by_src = {s: [r["label"] for r in rows if r["input_from"] == s] for s in ("own", "auto", "none")}
    log.append(f"first input from the run's own movie: {len(by_src['own'])}; from an automatic skip key "
               f"only: {by_src['auto']}; no input (pre == done == post): {by_src['none']}")
    late = [(r["label"], r["done_frame"], r["post_frame"]) for r in rows[:-1] if r["post_frame"] > r["done_frame"]]
    log.append(f"moves whose post is after their done frame (the next input comes later): {len(late)}; "
               f"largest gaps {sorted(late, key=lambda t: t[1] - t[2])[:5]}")
    after = [(r["label"], len(d)) for r in rows
             if (d := [k for k in diff(r["done"], r["post"]) if k.startswith(PERSISTENT)])]
    log.append(f"  of them, a bit, owner, state, class or room changes between done and post: {len(after)}; "
               f"most changes {sorted(after, key=lambda t: -t[1])[:5]}")

    def room_check(field: str) -> list[str]:
        bad = [r for r in rows if r["room"] is not None and r[field] != r["room"]]
        out = [f"{field} vs results.jsonl room: {len(bad)} disagree of {sum(r['room'] is not None for r in rows)} "
               f"({sum(r['room'] is None for r in rows)} entries have no room)"]
        return out + [f"  {r['label']}: results {r['room']}, {field} {r[field]} (done {r['recorded_done']}, "
                      f"post {r['post_frame']})" for r in bad[:8]]
    log += room_check("room_done") + room_check("room_post")

    same = [r["label"] for r in rows if not r["post_diff"]]
    log.append(f"moves with identical pre and post facts: {len(same)} {same}")
    quiet = [r["label"] for r in rows if r["post_diff"] and not any(k.startswith(PERSISTENT) for k in r["post_diff"])]
    log.append(f"moves that change no bit, owner, state, class or room by post (vars, scripts, verbs only): "
               f"{len(quiet)} {quiet}")
    waits = [r["label"] for r in rows if any(k.startswith("at:script:") for k in r["done_diff"])]
    log.append(f"moves that change a global script's wait point (`at:script:`) by done: {len(waits)}")
    others = [r["label"] for r in rows if any(k.startswith("at:") and not k.startswith("at:script:")
                                              for k in r["done_diff"])]
    log.append(f"moves that change a local, object, entry or exit script's wait point by done: {len(others)}")
    often = collections.Counter(k for r in rows for k in r["post_diff"])
    often = sorted(often.items(), key=lambda t: (-t[1], t[0]))  # ties by name, so the log is stable
    log.append(f"facts changed in the most moves (pre to post): {often[:12]}")

    key = json.loads(KEY.read_text())["key"]
    final = rows[-1]["post"]
    for n in (97, 98, 99):
        kind, subject, value = key[n - 1]["fact"]
        assert kind == "owner" and key[n - 1]["part"] == "I", key[n - 1]
        k = f"owner:{subject}"
        first = next((r["label"] for r in rows if r["post"].get(k, 0) == value), None)
        log.append(f"key #{n} {key[n - 1]['fact']}: final post (frame {rows[-1]['post_frame']}) has {k} = "
                   f"{final.get(k, 0)} -> {'holds' if final.get(k, 0) == value else 'FAILS'}; first holds after {first}")

    log.append(f"determinism check against {earlier}" + (" (the file this run replaces)" if earlier == OUT else ""))
    log += check_positions(rows, raw, old)
    choose = [r for r in rows if "clicked_verb" in r]
    unresolved = [(r["label"], r["clicked_verb_reason"]) for r in choose if r["clicked_verb"] is None]
    clicks = {e["frame"]: e["click"] for e in own if "click" in e}
    hit = 0
    for r in choose:
        verb = next((v for v in raw[r["pre_frame"]]["verbs"] if v["id"] == r["clicked_verb"]), None)
        hit += bool(verb and clicks.get(r["first_input"]) == [verb["x"] + 2, verb["y"] + 2])
    log.append(f"clicked_verb resolved for {len(choose) - len(unresolved)} of {len(choose)} choose moves; "
               f"run B's own first input is a click on that line's point in {hit}; unresolved {unresolved}")
    pre_kinds = collections.Counter(kd for r in choose for kd in {fact_kind(k) for k in r["pre"] if k.startswith("at:")})
    log.append(f"choose moves whose pre state has a script at a known instruction, by unit kind: "
               f"{dict(sorted(pre_kinds.items()))}")

    text = "\n".join(log) + "\n"
    log_path = OUT.parent / "groundtruth-part1.log"
    log_tmp = log_path.with_name(log_path.name + ".tmp")
    log_tmp.write_text(text)
    os.replace(log_tmp, log_path)
    print(text, end="")


if __name__ == "__main__":
    main()
