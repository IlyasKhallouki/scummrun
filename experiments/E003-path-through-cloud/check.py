"""E003 step 3: the model check, the instrument that scores the model against run B's Part I.

usage: uv run python experiments/E003-path-through-cloud/check.py [--features own_entry]
       ... --features own_entry,dispatch,calls               (row 1)
       ... --features own_entry,dispatch,calls,room_change   (row 2)
       ... --features own_entry,dispatch,calls,room_change,settle   (row 3)
       ... --features own_entry,dispatch,calls,room_change,settle,dialog   (row 4)
       ... --features own_entry,dispatch,calls,room_change,settle,dialog,esc   (row 5)
       ... --features ... --switches input_after_turns   (a switch, measured on its own; also const_polls,
                                                           esc_key_at_pre)
       ... --jobs 64 --resume DIR   (score moves in 64 forked processes; keep each finished move's record in DIR)

An instrument, not part of the system: it reads the ground truth (``groundtruth.load()``, run B's
recorded moves and the engine's facts before each, at its done frame and at the next move's start)
and the executor's interface profile. The model (``model.py``, ``actions.py``) never sees either.
The scoring rules are notes/log/2026-10-22-e003-step3-rules.md, fixed before any number.

**Moves.** Each ground-truth row is either excluded (a README glitch move, a skipped door, a
composite row) or counted. A counted move gets an action kind by the rules' table: a verb with objects
is a sentence, a walk to an object a walk-to sentence (the walk verb from the executor's profile,
``PROFILES["dos-cd"].walk_to``), choose a dialog line (the row's ``clicked_verb``; a row without one
has no action yet), Esc a skipped cutscene, '.' a wait, and a walk to a floor point no action.
Moves run B found by search take the action run B played (``VIA``).

**Acceptance.** A move is accepted when its action is usable in the pre state (``actions.usable`` is
True) and the model's run has at least one path that an unmodelled statement did not cut and that
the predictions use (``model.feasible``: not one that ends in an engine error(), where the game would
abort, nor one that spins, which adds nothing; counted). Row 0 runs
object A's own entry (a sentence whose object has no entry for the verb has no path); from row 1
(dispatch) a sentence or walk runs through the sentence script (``model.Machine.run_sentence``): a
live, unfrozen sentence slot first has its turn, and a path on which it is still live after it ends
"deferred" (accepted, counted in the log, with that turn's writes as its predictions). For a sentence or
walk the input script (VAR_VERB_SCRIPT) is not run; the log counts the moves whose input script is a room's
local script, where it may do more than queue the sentence. Dialog lines, skips and waits run nothing in rows 0-2: one empty
path, so they are accepted when usable and predict nothing. From row 3 (settle) time passes in rounds after
a sentence's or walk's immediate part, a path whose sentence had to wait keeps it queued instead of
ending "deferred", and a wait runs the rounds alone (``model.Machine.run_wait``); dialog lines and skips
still run nothing. From row 4 (dialog) a dialog line is a left click on its verb (``model.Machine.run_verb``):
usable when ``actions.usable`` holds for it (input on, the verb shown), it runs the input script installed at
pre with the click's arguments, then settle; skips still run nothing. From row 5 (esc) a skip is the Esc key
(``model.Machine.run_esc``): usable when ``actions.usable`` holds for it (VAR_CUTSCENEEXIT_KEY not 0, an override
armed at the top level at pre), it rewinds the override's slot and runs the Esc frame (the slots' turns, the key's
trip through checkExecVerbs, the sentence queue), then settle. A move with no action kind is not accepted. The
share is over every counted move, and also over the moves with an action.

**Row 3's own counts** (``settle_stats``, ``settle_log``; they change no number): how paths and their
immediate parts end, the fixed points reached while a kept slot's locals still moved, ROUND_LIMIT hits and
cycles, the path limit per move, the assumed loop exits by what their test read, the waits resolved by time,
the slots run by rule, the slots live at pre never run (and why), every slot a round passed over (by origin
and why, frozen ones and the ambient cap apart), the ambient loops capped and the facts settle's end made
unknown for them, the units with a second instance, the camera exits forked, the moves whose mouse the click
made unknown, order conflicts by the conflicting key's group and the certain predictions on a conflicting
key, the merges at a round's start, the queued sentences run, how many of the IR's time-only loops switch const_polls
would read as polling a fact, and precision split by whether a certain
prediction was already made at the end of the click's frame (before the first yield: the frame's other slot
turns, then the sentence) or moved after it, and then by the rule of the settle turn that last wrote it.

**Row 4's own counts** (``click_stats``, ``dialog_log``; they change no number): the input script per move and how
its first turn ended, the decisions it forked on and its writes with an unknown target, the paths whose click-frame
turns changed what checkExecVerbs reads before the click (``model.Path.input_drift``), the slots that resumed by
settle's rule c (the script waiting on the menu) and their kinds, precision and recall split by whether a
room-local or object script was live at pre, the frames from the click to done, and one line per dialog move.

**Row 5's own counts** (``esc_stats``, ``esc_log``; they change no number): per Esc move, the override abortCutscene
rewinds (the slot, its override point and where the jump goes), how the rewound slot's first turn ended, where the
key went at checkExecVerbs (wiped by a doSentence(STOP), dropped with input off, the input script that ran, or not
known), counted on every path as it reached checkExecVerbs (``model.Run.key_routes``), with the kinds of the forks
that path had taken by then (a cutscene's data and an actor's room among them: the key's way rests on them), the
certain predictions that are abortCutscene's own writes and the skips' precision without them, the verb keys in the
IR that could take the key (``actions.VERB_KEY``: a constant, or a variable whose writes the plan's closure from run
B's start value reaches the key code with), and, for each move not accepted, the cutscene depth, the esc: facts and
run B's own inputs up to done.

**Run B's inputs inside a move's window** (``run_b_inputs``, ``inputs_log``, every row; they change no number): the
recorded inputs of the explicit movie the ground truth replays that the scored action does not hold, after the
move's first input up to done and after done up to post, each marked own (the executor's movie) or auto (the
bridge's automatic '.' and Esc). The engine ran them; the model runs only the action. Kept per move in the field
``run_b_inputs`` with the move's frames.

**Predictions** come from accepted moves only, and only in fact kinds the ground truth holds (the
engine's clocks, facts.CLOCKS, are not in it either). Per fact a path writes: certain when every path
gives the same known value (a path that does not write it keeps the pre value), possible otherwise
(the possible values include those of value sets, ``model.predict``). A prediction counts only when the
value differs from pre. Precision: certain predicted changes whose
value the engine shows at the done frame (micro-averaged; the next move's start is the second
number), always given with its counts, certain hits over certain predictions ("83.9% (405/483)"). With no
predictions precision is "n/a". Recall: the tracked facts that changed from pre to
done, less the excluded ones, that the model predicted with that value, certain only and certain or
possible, over every counted move (a move the model cannot run predicts nothing) and over the
accepted moves. Excluded from recall: the variables the engine writes outside script opcode handlers
(``ENGINE_VARS``, each cited at such a site: every frame, input and keys, the talk timer's stopTalk,
save and load, Esc's abortCutscene, actor walk sounds) and the model's own non-facts
(``model.non_fact_vars``). Engine variables written only inside an opcode (the room change's, the
message box's: ``IN_OPCODES``) stay in recall, and the log counts their changes. Nothing is excluded by
a game number. The rules exclude nothing from precision; the log also gives precision
over the facts recall scores, so both numbers rest on one fact set.

**Diagnostics** (they change no number): the engine conditions an accepted move's usability rests on
but does not test (``actions.UNCHECKED``), counted and, for a parent's state, listed (a click on a verb's
own conditions, ``actions.CLICK``, are kept in the field ``unchecked_click``, the object conditions in
``unchecked``, the Esc key's, ``actions.ESC``, in ``unchecked_esc`` on the skips), with the script writes
of VAR_USERPUT in the IR (input on reads it for _userPut); and for each sentence and walk, the objects of the sentence the engine last ran by the done frame, read from the
two variables the installed sentence script (or a script it chains to) copies its objects into
(``engine_sentence``, found in the IR), with the moves where they are not the scored action's. The
log also counts, per run and over moves, everything the model did not decide or stopped at: path ends,
the path limit, unmodelled statements, notes (partial effects and recorded assumptions such as a
slot's F flag), writes with an unknown target, assumptions by kind, script turns by how they ended,
the deepest stack, starts of a unit already on the stack, merges, spins, value sets past
``model.MAX_VALUES``, the F flags taken from start sites, and the engine's error() cases (the path ends,
left out of the predictions) and warning() cases (counted only).

Writes ``results/check-part1.jsonl`` (one counted or excluded move per line), ``results/check-part1.log``,
for a feature set that is a growth row (``model.ROWS``) the same two as ``check-part1-rowN.*``, and a row
of ``results/growth.md`` (replaced when the same features run again). A run with a switch on
(``--switches``, ``model.SWITCHES``) writes only ``check-part1-rowN-<switches>.*``. The ground truth's
hash is taken before and after scoring; a change in between (it is regenerated elsewhere) stops the
run before anything is written.

**Many cores** (``--jobs N``, ``score_parallel``): the parent loads the game, the ground truth and every move's action,
then forks N processes that each run ``score_move`` on whole moves; the parent puts the records back in move order and
runs the same summary and writing code, so the files do not change by one byte (the log's first line, the run's time,
apart). Moves start costliest first, by a times file from an earlier run. ``--jobs 1`` (the default) without
``--resume`` scores the moves one after another in this process, in ``main``'s loop as before ``--jobs`` existed, and
writes nothing more. A run with ``--jobs`` above 1 or ``--resume`` (``score_many``) also writes
``results/<stem>.times.tsv`` (``times_stem``): per counted move its CPU seconds, the scoring process's peak RSS and its
paths. ``--resume DIR`` keeps each finished move's record in DIR with the move's header (its action among them) under
a hash of the features, the switches, the ground truth, the sources, the game's scripts and raw blocks, and the Python
version (``parallel.run_key``), taken before they are read and checked again after; a rerun loads the records there
that match instead of scoring those moves again.
"""

import argparse
import collections
import hashlib
import json
import math
import multiprocessing
import re
import sys
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path

import parallel  # first: it hashes the sources before the model is imported (parallel.AT_IMPORT)

import actions
import facts as F
import groundtruth
import ir
import model
from speedrun.executor import PROFILES

HERE = Path(__file__).resolve().parent
OUT = HERE / "results"
TRIGGERS = HERE.parent / "E002-cause-cloud" / "results" / "engine-triggers.json"
WALK_VERB = PROFILES["dos-cd"].walk_to  # the executor's interface profile (plan.md C2, disclosed)
INV_FIRST_VAR = PROFILES["dos-cd"].inv_first_var

# Run B played these by search (results.jsonl "how": "search"); the rules take the action run B played.
# * S3#97 "auto: walk to exit 434 -> room 31": walk to object 434.
# * S3#98 "auto: verb 7 mug": Use with mug 365 and the cell, 401. The engine shows the sentence it ran:
#   the sentence script (VAR_SENTENCE_SCRIPT, var 33 = 2 at pre) copies its objects into two variables
#   (global/script-002.txt:21-22, [0030] Var[103] = Local[1], [0035] Var[104] = Local[2], their only
#   writers), and at the done frame they hold 365 and 401 (checked below, ``engine_sentence``). The
#   route names mug 366 and the lock 403; run B's first click was inventory slot 4, which shows 365
#   (also checked), and its second landed on the cell. For the mug's Use entry the two are alike
#   (obj-0365-mug.txt [0066], global/script-069.txt [0000]-[001A]), so row 0 does not tell them apart.
# * S3#119 "auto: wait until idle": a wait.
VIA = {"S3#97": ("auto: walk to exit 434 -> room 31", ("walk", 434)),
       "S3#98": ("auto: verb 7 mug", ("sentence", 7, 365, 401)),
       "S3#119": ("auto: wait until idle", ("wait",))}
VIA_SLOT = {"S3#98": (4, 365)}  # inventory slot run B clicked, and the object it must show

# Variables the engine writes outside script opcode handlers (ScummVM v5 indices, vars.cpp:36-109): every
# frame in scummLoop, input and key handling, timers, the talk timer's stopTalk, save and load, Esc's
# abortCutscene, actor walk sounds. Each is cited at a write site outside the opcodes; some are also
# written inside one, and the reason says so. Only these leave recall (the rules file).
ENGINE_VARS = {
    2: ("VAR_CAMERA_POS_X", "scumm.cpp:3379 every frame (scummLoop_updateScummVars); also camera.cpp:164 moveCamera, "
                            "every frame; also camera.cpp:53 setCameraAt, inside opcodes"),
    3: ("VAR_HAVE_MSG", "scumm.cpp:3382 every frame; also actorTalk inside print (actor.cpp:3446)"),
    5: ("VAR_OVERRIDE", "script.cpp:1706 abortCutscene, from Esc in processInput (input.cpp:1423-1424); also "
                        "beginOverride, endOverride and endCutscene, opcodes"),
    20: ("VAR_VIRT_MOUSE_X", "scumm.cpp:3385 every frame"),
    21: ("VAR_VIRT_MOUSE_Y", "scumm.cpp:3386 every frame"),
    23: ("VAR_LAST_SOUND", "sound.cpp:121-122 from actor walk sounds, playActorSounds every frame (scumm.cpp:3312, "
                          "actor.cpp:2267); also startSound, an opcode"),
    25: ("VAR_TALK_ACTOR", "actor.cpp:3368 setTalkingActor, from stopTalk (actor.cpp:3564-3592) on the "
                           "message timer (string.cpp:1125-1128); also actorTalk inside print, and stopTalk inside "
                           "startScene (room.cpp:53)"),
    26: ("VAR_CAMERA_FAST_X", "input.cpp:1283, 1439 (the snap-scroll key toggles it; vars.cpp:62)"),
    37: ("VAR_CHARINC", "input.cpp:1150-1156 (text speed keys)"),
    39: ("VAR_DEBUGMODE", "scumm.cpp:3391 every frame"),
    44: ("VAR_MOUSE_X", "scumm.cpp:3387 every frame"),
    45: ("VAR_MOUSE_Y", "scumm.cpp:3388 every frame"),
    71: ("VAR_GAME_LOADED", "scumm.cpp:3415-3445 on save and load"),
}
# Engine variables the engine writes only as a side effect inside an opcode handler. They stay in recall:
# the model is expected to predict them once the feature that runs that opcode's engine code is on.
# (Row 0's first run left these out too; the rules file's correction note.)
IN_OPCODES = {
    0: ("VAR_KEYPRESS", "showMessageDialog, the message box of a print to actor 252, inside o5_print "
                        "(script_v5.cpp:3448-3449; string.cpp:119, 164-169); input.cpp:931-936 is "
                        "ScummEngine_v2::processKeyboard (input.cpp:887-960), which v5 does not run. Also the "
                        "scripts' result register, VAR_RESULT"),
    4: ("VAR_ROOM", "room.cpp:149, 155 in startScene, inside loadRoom, loadRoomWithEgo (script_v5.cpp:1850, "
                    "1875) and actorFollowCamera (camera.cpp:70)"),
    17: ("VAR_CAMERA_MIN_X", "room.cpp:203 in startScene (the room change opcodes); also the RoomScroll opcode "
                             "(script_v5.cpp:2374)"),
    18: ("VAR_CAMERA_MAX_X", "room.cpp:205 in startScene; also the RoomScroll opcode (script_v5.cpp:2375)"),
    22: ("VAR_ROOM_RESOURCE", "room.cpp:172 in startScene"),
    70: ("VAR_ROOM_FLAG", "resource.cpp:644 when startScene loads the room resource (room.cpp:175)"),
    72: ("VAR_NEW_ROOM", "room.cpp:76 in startScene"),
}
IN_OPCODES_NOTE = ("save and load also call startScene (scumm.cpp:3900) and write VAR_ROOM_FLAG (saveload.cpp:937-938); "
                   "run B never saves or loads, so in this run these variables change only inside opcodes")
# Names engine-triggers.json gives at non-boot sites that are not in ENGINE_VARS, and why.
LEFT_IN = {
    "VAR_CURSORSTATE": "written only by o5_cursorCommand (script_v5.cpp:935-938), an opcode",
    "VAR_USERPUT": "written only by o5_cursorCommand (script_v5.cpp:935-938), an opcode",
    "VAR_WALKTO_OBJ": "written inside o5_loadRoomWithEgo (script_v5.cpp:1874-1876), an opcode",
    "VAR_SOUNDRESULT": "written by o5_soundKludge's iMUSE command (sound.cpp:178-182), an opcode",
    "VAR_RESULT": "the scripts' result register, var 0: in IN_OPCODES as VAR_KEYPRESS",
    "VAR_EGO": "read, not written (pickupObject)",
    "VAR_CUTSCENEEXIT_KEY": "read by the Esc handler, not written",
    "VAR_CHARCOUNT": "not a v5 variable",
    "VAR_CAMERA_MIN": "the site's text for VAR_CAMERA_MIN_X",
}
HOOK_VARS = re.compile(r"VAR_(\w+_SCRIPT2?|TMR_\d|TIMER\w*|MUSIC_TIMER)$")  # hooks the engine reads; clocks


def label(row: dict) -> str:
    return row["name"].split(" ")[0]


def sentence_copies(game: model.Game, n: int, room) -> tuple[tuple[int, int], list[str]] | None:
    """The variables sentence script ``n`` copies its objects into, and the lines. checkAndRunSentenceScript
    starts it with locals [verb, A, B] (script.cpp:1174-1240); this takes the first ``Var[v] = Local[1]``
    and the first ``Var[w] = Local[2]`` in its code, or, when it has none, in the scripts it chains to
    with a constant number (a room's own sentence script may hand the sentence on). None when no script
    reached has both."""
    res = game.resource(room)
    todo, seen = [n], set()
    while todo:
        k = todo.pop(0)
        key = ("local", res, k) if k >= game.first_local else ("global", k)
        if key in seen or key not in game.units:
            continue
        seen.add(key)
        found = {}
        for x in game.units[key].ins:
            st = x.st
            if isinstance(st, ir.Assign) and st.op == "=" and isinstance(st.lhs, ir.Var) \
                    and isinstance(st.rhs, ir.Local) and st.rhs.n in (1, 2) and st.rhs.n not in found:
                found[st.rhs.n] = (st.lhs.n, x.cite())
            elif isinstance(st, ir.Start) and st.name == "chainScript" and isinstance(st.script, ir.Const):
                todo.append(st.script.value)
        if len(found) == 2:
            return (found[1][0], found[2][0]), [found[1][1], found[2][1]]
    return None


def engine_sentence(row: dict, game: model.Game) -> dict | None:
    """The objects of the last sentence the engine ran, before the move and at its done frame, from
    the copies the installed sentence script (VAR_SENTENCE_SCRIPT at pre) makes; None without them."""
    n = row["pre"].get(f"var:{model.VAR_SENTENCE_SCRIPT}", 0)
    copies = sentence_copies(game, n, row["pre"].get("room", 0)) if n else None
    if copies is None:
        return None
    vs, cites = copies
    return {"script": n, "vars": list(vs), "cites": cites, "pre": [row["pre"].get(f"var:{v}", 0) for v in vs],
            "done": [row["done"].get(f"var:{v}", 0) for v in vs]}


def classify(row: dict, game: model.Game) -> tuple[str, actions.Action | None, str]:
    """("excluded" | "counted", action or None, why)."""
    if row["glitch"]:
        return "excluded", None, "glitch (README table)"
    if row["skipped"]:
        return "excluded", None, "skipped door (results.jsonl)"
    if row["composite"]:
        return "excluded", None, "composite (via holds several actions)"
    lab, mv = label(row), row["move"]
    if lab in VIA:
        text, act = VIA[lab]
        if row["via"] != [text]:
            raise SystemExit(f"{lab}: via is {row['via']}, the check expects [{text!r}]")
        if lab in VIA_SLOT:
            k, obj = VIA_SLOT[lab]
            shown = row["pre"].get(f"var:{INV_FIRST_VAR + k}")
            if shown != obj:
                raise SystemExit(f"{lab}: inventory slot {k} shows {shown}, the check expects {obj}")
        if act[0] == "walk":
            return "counted", actions.walk_to(WALK_VERB, act[1]), f"via {text}"
        if act[0] == "sentence":
            es = engine_sentence(row, game)
            if es is None or es["done"] != [act[2], act[3] or 0]:
                raise SystemExit(f"{lab}: the engine's sentence at done is {es}, the check expects {act[1:]}")
            return "counted", actions.sentence(*act[1:]), f"via {text}"
        return "counted", actions.wait(), f"via {text}"
    if mv["choose"] is not None:
        if row.get("clicked_verb") is None:
            return "counted", None, f"dialog line: no clicked_verb ({row.get('clicked_verb_reason', 'not in the ground truth')})"
        return "counted", actions.dialog_line(row["clicked_verb"]), ""
    if mv["key"] == "esc":
        return "counted", actions.skip_cutscene(), ""
    if mv["key"] == ".":
        return "counted", actions.wait(), ""
    if mv["key"] is not None:
        return "counted", None, f"key {mv['key']!r} has no action kind"
    if mv["point"] is not None:
        return "counted", None, "walk to a floor point"
    if mv["obj"] is not None:
        if mv["verb"] in (None, WALK_VERB):
            return "counted", actions.walk_to(WALK_VERB, mv["obj"]), ""
        return "counted", actions.sentence(mv["verb"], mv["obj"], mv["obj2"]), ""
    return "counted", None, "no rule for this move"


AT_FAMILIES = ("at:script", "at:local", "at:obj", "at:entry", "at:exit")
ESC_FAMILIES = ("esc:script", "esc:local", "esc:obj", "esc:entry", "esc:exit")


def held_families(rows: list[dict]) -> frozenset[str]:
    """The fact families this ground truth holds: those its states show. A dump that gives a wait
    point for any slot but a global script holds them for every slot kind (facts.py at:), and one that
    shows an esc: fact holds esc: for every unit kind; an older dump has neither, nor cutscene."""
    seen = {model.family(k) for r in rows for side in ("pre", "done", "post") for k in r[side]}
    if seen & set(AT_FAMILIES[1:]):
        seen |= set(AT_FAMILIES)
    if seen & set(ESC_FAMILIES):
        seen |= set(ESC_FAMILIES)
    return frozenset(seen)


def kind_name(action: actions.Action | None) -> str:
    if action is None:
        return "none"
    return "walk_to" if action.walk else action.kind


def run_model(machine: model.Machine, action: actions.Action, oracle: model.Oracle) -> model.Run:
    """Row 0 runs object A's own entry; from row 1 (dispatch) the sentence runs through the sentence
    script, a walk_to as the sentence with the walk verb and no B, and from row 3 (settle) time then
    passes in rounds. From row 3 a wait runs the rounds alone (``model.Machine.run_wait``). From row 4 (dialog)
    a dialog line is a left click on its verb (``model.Machine.run_verb``). From row 5 (esc) a skip is the Esc key
    (``model.Machine.run_esc``). Before row 5 skips run nothing (their immediate part is a later feature, so they get
    no settle either); before row 4 neither do dialog lines."""
    if action.kind == "sentence":
        if "dispatch" in machine.features:
            return machine.run_sentence(oracle, action.verb, action.a, action.b)
        return machine.run_entry(oracle, action.a, action.verb)
    if action.kind == "wait" and "settle" in machine.features:
        return machine.run_wait(oracle)
    if action.kind == "dialog_line" and "dialog" in machine.features:
        return machine.run_verb(oracle, action.verb)
    if action.kind == "skip_cutscene" and "esc" in machine.features:
        return machine.run_esc(oracle)
    empty = model.Path(model.Frame(("none",), 0), end="no effect at this row")
    return model.Run([empty], f"{action.kind}: nothing runs at this row")


ASSUMPTION_KIND = re.compile(r"^\?\(([^:)]+)")


def assumption_kind(a: tuple) -> str:
    """An assumption's kind: the reason of the unknown it was taken on (random draw, position, actor
    state, ...), or its own text when it is not an unknown (a slot kind)."""
    m = ASSUMPTION_KIND.match(a[3])
    return m[1] if m else a[3]


def excluded_fact(key: str, nonfacts: dict) -> str | None:
    if key.startswith("var:"):
        n = int(key[4:])
        if n in ENGINE_VARS:
            return f"engine var {ENGINE_VARS[n][0]}"
        if n in nonfacts:
            return f"non-fact ({nonfacts[n]})"
    return None


def pct(a: int, b: int) -> str:
    return f"{100 * a / b:.1f}%" if b else "n/a"


def prec(hits: int, n: int) -> str:
    """A precision with its counts: certain hits over certain predictions, as "83.9% (405/483)"."""
    return f"{pct(hits, n)} ({hits}/{n})"


def score_move(row, action, machine, nonfacts, kinds) -> dict:
    pre, done, post = row["pre"], row["done"], row["post"]
    oracle = model.FactOracle(pre, kinds)
    out = {"usable": None, "accepted": False, "why_not": None, "paths": 0, "ends": {}, "limit": 0,
           "unmodelled": [], "notes": {}, "unknown_writes": 0, "assumptions": 0, "certain": {}, "possible": {},
           "hits": [], "misses": [], "hits_post": [], "misses_post": [], "untracked": []}
    changed = {k: v for k, v in F.diff(pre, done).items()}
    out["changed"] = len(changed)
    out["in_opcodes_changed"] = sorted(k for k in changed if k.startswith("var:") and int(k[4:]) in IN_OPCODES)
    out["excluded_changes"] = sorted(k for k in changed if excluded_fact(k, nonfacts))
    scored = {k: v for k, v in changed.items() if not excluded_fact(k, nonfacts)}
    if action is None:
        out["why_not"] = "no action kind"
        out["recall_misses"] = sorted(scored)
        out["recall_total"] = len(scored)
        return out
    u = actions.usable(action, oracle, machine.game)
    out["usable"] = u.value
    out["checks"] = [(c.name, c.holds, c.why) for c in u.checks]
    uc = u.unchecked()
    # the object conditions as rows 0-3 kept them; a click on a verb's own conditions (actions.CLICK) and the Esc
    # key's (actions.ESC, a field of the skips only) apart
    out["unchecked"] = [x for x in uc if x[0] not in actions.CLICK + actions.ESC]
    out["unchecked_click"] = [x for x in uc if x[0] in actions.CLICK]
    if action.kind == "skip_cutscene":
        out["unchecked_esc"] = [x for x in uc if x[0] in actions.ESC]
    if action.kind == "sentence" and (es := engine_sentence(row, machine.game)) is not None:
        es["matches"] = es["done"] == [action.a, action.b or 0]
        out["engine_sentence"] = es
    run = run_model(machine, action, oracle)
    out["start"] = run.start
    out["paths"], out["limit"] = len(run.paths), run.limit
    out["ends"] = dict(collections.Counter(p.end for p in run.paths))
    out["unmodelled"] = sorted({(h, c) for p in run.paths for h, c, _ in p.unmodelled})
    out["notes"] = dict(collections.Counter(h for p in run.paths for h, _, _ in p.notes))
    out["unknown_writes"] = sum(len(p.unknown_writes) for p in run.paths)
    out["assumptions"] = max((len(p.assumptions) for p in run.paths), default=0)
    out["wait_assumptions"] = sorted({a[0] for p in run.paths for a in p.assumptions if a[1].startswith("WaitFor")})
    distinct = {}  # one per decision (where, condition, side taken), not per path that carries it
    for p in run.paths:
        for a in p.assumptions:
            distinct.setdefault(a[:3], assumption_kind(a))
    out["assumption_kinds"] = dict(collections.Counter(distinct.values()))
    out["value_caps"] = dict(getattr(run, "value_caps", {}))
    if (action.kind == "sentence" and "dispatch" in machine.features) or \
            (action.kind == "dialog_line" and "dialog" in machine.features):
        n = pre.get(f"var:{model.VAR_VERB_SCRIPT}", 0)
        out["input_script"] = {"n": n, "local": bool(n) and n >= machine.game.first_local}
    if action.kind == "dialog_line" and "dialog" in machine.features:
        out["click"] = click_stats(run, pre, machine)
    if action.kind == "skip_cutscene" and "esc" in machine.features:
        out["esc"] = esc_stats(run, pre, machine)
    out["turns"] = dict(collections.Counter(how for p in run.paths for _, how in p.turns))
    out["depth"] = max((p.depth for p in run.paths), default=0)
    out["reentries"] = sum(p.reentries for p in run.paths)
    out["errors"] = dict(collections.Counter(w for p in run.paths for w, _, _ in p.errors))
    out["error_sites"] = sorted({(w, c, d) for p in run.paths for w, c, d in p.errors})[:8]
    out["steps"] = max((p.steps for p in run.paths), default=0)
    out["merges"] = run.merges
    cut = all(p.end == "unmodelled" for p in run.paths)
    out["infeasible"] = sum(1 for p in run.paths if p.end in model.INFEASIBLE)
    dead = bool(run.paths) and all(p.end in model.INFEASIBLE or p.end == "unmodelled" for p in run.paths) and not cut
    if u.value is not True:
        out["why_not"] = ("not usable: " + "; ".join(f"{c.name} ({c.why})" for c in u.failing())) if u.value is False \
            else "usability unknown: " + "; ".join(f"{c.name} ({c.why})" for c in u.unknown())
    elif not run.paths:
        out["why_not"] = f"no path: {run.start}"
    elif cut:
        out["why_not"] = "every path cut by an unmodelled statement"
    elif dead:
        out["why_not"] = "every path left out (error, nesting or spin) or cut"
    out["accepted"] = out["why_not"] is None
    if not out["accepted"]:
        out["recall_misses"] = sorted(scored)
        out["recall_total"] = len(scored)
        return out
    for k, pr in model.predict(run, oracle).items():
        if not oracle.tracks(model.family(k)) or (k.startswith("var:") and int(k[4:]) in F.CLOCKS):
            out["untracked"].append(k)  # a kind the ground truth lacks, or a clock it leaves out
            continue
        before = pre.get(k, 0)
        if pr["certain"]:
            if pr["values"][0] != before:
                out["certain"][k] = pr["values"][0]
        else:
            vals = [v for v in pr["values"] if v != before]
            if vals or pr["unknown"]:
                out["possible"][k] = {"values": vals, "unknown_paths": pr["unknown"]}
    for k, v in out["certain"].items():
        (out["hits"] if done.get(k, 0) == v else out["misses"]).append((k, v, done.get(k, 0)))
        (out["hits_post"] if post.get(k, 0) == v else out["misses_post"]).append((k, v, post.get(k, 0)))
    if "settle" in machine.features:
        out |= settle_stats(run, out, pre, done, post, machine)
    rc = [k for k, (_, new) in scored.items() if out["certain"].get(k) == new]
    rp = [k for k, (_, new) in scored.items() if out["certain"].get(k) == new
          or new in out["possible"].get(k, {}).get("values", [])]
    out["recall_total"], out["recall_certain"], out["recall_possible"] = len(scored), len(rc), len(rp)
    out["recall_hits"] = sorted(rc)
    out["recall_misses"] = sorted(k for k in scored if k not in rp)
    # the excluded changes the model got right anyway, for the log's sensitivity line
    out["excluded_hits"] = sorted(k for k in out["excluded_changes"] if out["certain"].get(k) == changed[k][1])
    out["excluded_hits_possible"] = sorted(k for k in out["excluded_changes"] if out["certain"].get(k) == changed[k][1]
                                           or changed[k][1] in out["possible"].get(k, {}).get("values", []))
    return out


def click_stats(run: model.Run, pre: dict, machine: model.Machine) -> dict:
    """Row 4 (dialog), per dialog move: what the click on the line's verb did (the log sums these; they change no
    number). The input script is the one installed at pre (``run.unit``): how its first turn ended on each path,
    the decisions it forked on and the writes whose target it could not work out (at its own instructions), and
    the paths whose click-frame turns changed what checkExecVerbs reads (``model.Path.input_drift``). The slots
    that first ran by settle's rule c (a slot live at pre polling a fact the path changed: the script waiting on
    the menu, found by running the scripts) and their kinds. Whether a room-local or object script was live at
    pre (the rules note's 37 of 60)."""
    paths = model.feasible(run)
    name = model.unit_name(run.unit) if run.unit else None
    u = machine.game.units.get(run.unit) if run.unit else None
    # a path that ended before the click (a spin or an error in a turn of the click's frame) has no input script turn
    first = collections.Counter(next((how for n, how in p.turns if n == name), f"did not run ({p.end})")
                                for p in run.paths)
    forks = {}
    for p in run.paths:
        for a in p.assumptions:
            if u is not None and a[0].startswith(u.file + ":"):
                forks.setdefault(a[:3], assumption_kind(a))
    polled = sorted({n for p in paths for n, r in p.ran_by.items() if r.startswith("c")})
    return {"input_first_turn": dict(sorted(first.items())), "input_forks": dict(collections.Counter(forks.values())),
            "input_unknown_writes": sum(1 for p in run.paths for _, c, _ in p.unknown_writes
                                        if u is not None and c.startswith(u.file + ":")),
            "drift": sorted({d for p in run.paths for d in p.input_drift}, key=str),
            "drift_paths": sum(1 for p in run.paths if p.input_drift),
            "polled_by": polled, "polled_kinds": sorted({n.split(":")[0] for n in polled}),
            "local_or_object_live": any(v == 1 for k, v in pre.items() if k.startswith(("local:", "objscript:")))}


def esc_stats(run: model.Run, pre: dict, machine: model.Machine) -> dict:
    """Row 5 (esc), per Esc move: what the Esc did (the log sums these; they change no number). The override
    abortCutscene rewinds (the esc: fact at pre, the jump at its override point and where it goes), how the rewound
    slot's first turn ended on each path, and where the key went at checkExecVerbs (``model.Machine._eng_key``: wiped
    by clearClickedStatus, dropped with input off, the input script that ran, not known), counted on every path as it
    reached checkExecVerbs (``model.Run.key_routes``; a path merged later counts once per way it carries), plus the
    final paths that ended before it; for each way, the kinds of the forks the path had taken by then (``fork_kind``:
    the way rests on them). The abort's own keys (``abort_keys``): the armed esc: fact, which abortCutscene clears
    (script.cpp:1707), and the rewound slot's wait points at pre, which it leaves (1699)."""
    armed = sorted(k for k, v in pre.items() if k.startswith("esc:") and v == 1)
    out = {"armed": armed, "depth": pre.get("cutscene", 0), "exit_key": pre.get(f"var:{model.VAR_CUTSCENEEXIT_KEY}", 0)}
    if armed and run.unit is not None and run.unit in machine.game.units:
        u = machine.game.units[run.unit]
        idx = u.at.get(int(armed[0].rsplit(":", 1)[1]))
        if idx is not None:
            x = u.ins[idx]
            out["jump"] = x.cite()
            out["target"] = u.ins[x.target].cite() if x.target is not None and x.target < len(u.ins) else None
    name = armed[0][4:].rsplit(":", 1)[0] if armed else None
    out["first_turn"] = dict(sorted(collections.Counter(
        next((how for n, how in p.turns if n == name), f"did not run ({p.end})") for p in run.paths).items()))

    key = collections.Counter(fate for fate, _ in run.key_routes)
    key.update(f"not reached ({p.end})" for p in run.paths if not any(h == "Esc key" for h, _, _ in p.notes))
    out["key"] = dict(sorted(key.items()))
    forks = collections.defaultdict(collections.Counter)  # way -> fork kind -> paths that had taken such a fork
    for fate, assumptions in run.key_routes:
        forks[fate].update({fork_kind(a) for a in assumptions} or {"no fork"})
    out["key_forks"] = {fate: dict(sorted(c.items())) for fate, c in sorted(forks.items())}
    out["abort_keys"] = (armed[:1] + sorted(k for k, v in pre.items() if k.startswith(f"at:{name}:") and v == 1)
                         if armed else [])
    return out


KEY_FORKS = ("cutscene data", "actor room")  # the fork kinds the Esc lines single out (``fork_kind``)


def fork_kind(a: tuple) -> str:
    """A fork's kind for the Esc lines: ``assumption_kind``, with a fork on an actor's room (setCameraFollows' room
    change, camera.cpp:69-70: the room is not a fact) told apart from the other actor-state forks; a cutscene's data
    (vm.cutSceneData of a level opened before the move, script.cpp:1640, 1658) is its own kind already."""
    return "actor room" if "room of actor" in a[3] else assumption_kind(a)


def verb_key_sites(game: model.Game, codes: set, start: dict) -> tuple[dict, list[str]]:
    """The VerbOps Key sub-ops in the IR (o5_verbOps SO_VERB_KEY, script_v5.cpp:3231-3232): with a constant equal to
    one of ``codes``, with another constant, with a variable, and with another operand (its value not worked out).
    For each variable, its writes (``var_writes``) and whether the plan's closure (plan step 1, Facts: the start
    value closed under the writes) reaches one of ``codes`` (``closure_reaches``); ``start`` gives the start value.
    The plan's bound (values above the largest compared constant are one value) excludes no value, so it is left out."""
    out, used = collections.Counter(), collections.Counter()
    for u in game.units.values():
        for x in u.ins:
            if isinstance(x.st, ir.Op) and x.st.name == "VerbOps":
                for sub in x.st.subops:
                    if sub.name == "Key":
                        a = sub.args[0]
                        if isinstance(a, ir.Var):
                            used[a.n] += 1
                        out["a constant equal to the code" if isinstance(a, ir.Const) and a.value in codes else
                            "another constant" if isinstance(a, ir.Const) else
                            "a variable" if isinstance(a, ir.Var) else "another operand"] += 1
    lines = []
    for n, k in sorted(used.items()):
        consts, steps, other = var_writes(game, n)
        s0 = start.get(f"var:{n}", 0)
        reach = any(closure_reaches({s0} | consts, steps, c) for c in codes)
        rest = ", ".join(f"{w} {c}" for w, c in sorted(other.items()))
        verdict = ("the closure of these from the start value reaches the code" if reach else
                   "the closure of these from the start value never reaches the code") + \
            (f"; other writes, not worked out here: {rest}" if other else "")
        lines.append(f"Var[{n}] at {k} sites: start value {s0}, constants {sorted(consts)}, steps "
                     f"{sorted(steps)}: {verdict}")
    return dict(out), lines


def _const(e) -> int | None:
    """A constant expression's value (constants, +, -, * and negation over them), else None."""
    if isinstance(e, ir.Const):
        return e.value
    if isinstance(e, ir.Neg):
        v = _const(e.x)
        return None if v is None else -v
    if isinstance(e, ir.BinOp) and e.op in "+-*":
        a, b = _const(e.a), _const(e.b)
        return None if a is None or b is None else {"+": a + b, "-": a - b, "*": a * b}[e.op]
    return None


def var_writes(game: model.Game, n: int) -> tuple[set, set, collections.Counter]:
    """Var[n]'s writes in the IR as the plan's finite-domain rule reads them (plan step 1, Facts): the constants it is
    set to (an assignment, setVarRange's values, script_v5.cpp:2754-2768), the steps it moves by (+= and -= a
    constant, ++ and --), and the other writes by kind (a value that is not a constant, an indexed write whose target
    may be it: either leaves the variable with no finite domain). Every unit is read, reachable or not."""
    consts, steps, other = set(), set(), collections.Counter()
    for u in game.units.values():
        for x in u.ins:
            st = x.st
            if isinstance(st, ir.Assign) and st.lhs == ir.Var(n):
                v = _const(st.rhs)
                if v is None or st.op not in ("=", "+=", "-=", "++", "--"):
                    other["a value that is not a constant"] += 1
                else:
                    (consts if st.op == "=" else steps).add(v if st.op in ("=", "+=", "++") else -v)
            elif isinstance(st, ir.Assign) and isinstance(st.lhs, ir.Index) and st.lhs.kind == "var" \
                    and _const(st.lhs.offset) in (None, n - st.lhs.base):
                other["an indexed write that may hit it"] += 1
            elif isinstance(st, ir.Op) and st.name == "setVarRange" and st.args and isinstance(st.args[0], ir.Var) \
                    and 0 <= n - st.args[0].n < len(st.args[2].items):
                v = _const(st.args[2].items[n - st.args[0].n])
                if v is None:
                    other["a value that is not a constant"] += 1
                else:
                    consts.add(v)
    return consts, steps, other


def closure_reaches(starts: set, steps: set, code: int) -> bool:
    """Whether ``code`` is in the closure of ``starts`` under ``steps`` (any value plus any step, again and again):
    with steps both ways, every value a multiple of their gcd away; with steps one way, every value a sum of them away
    in that direction; with none, the starts."""
    ups, downs = sorted(d for d in steps if d > 0), sorted(-d for d in steps if d < 0)
    for v in starts:
        diff = code - v
        if diff == 0 or (ups and downs and diff % math.gcd(*ups, *downs) == 0):
            return True
        one_way = ups if diff > 0 and not downs else downs if diff < 0 and not ups else []
        if one_way:
            sums = [True] + [False] * abs(diff)
            for i in range(1, abs(diff) + 1):
                sums[i] = any(d <= i and sums[i - d] for d in one_way)
            if sums[-1]:
                return True
    return False


def esc_log(moves: list, game: model.Game, start: dict) -> list[str]:
    """Row 5's own lines (esc; they change no number): the Esc moves (``esc_stats``), summed and one line each.
    ``start`` is run B's state before its first move, the start value of the verb-key variables' closure."""
    acc = [m for m in moves if m["accepted"]]
    es = [m for m in moves if m.get("esc")]
    log = ["", f"skipped cutscenes (model.Machine.run_esc: the Esc key; abortCutscene rewinds the override armed at the "
               f"top level at pre, then the Esc frame and settle): {len(acc)} of {len(moves)} accepted"]
    for m in moves:
        if m["accepted"]:
            continue
        e, ri = m.get("esc", {}), m["run_b_inputs"]
        log.append(f"  not accepted: {m['label']}: {m['why_not']}; at pre cutscene depth {e.get('depth')}, esc: facts "
                   f"{e.get('armed') or 'none'}; run B's keys after the first up to done: "
                   + (keys_text([t for t in ri["to_done"] if t[1] != "click"]) or "none"))
    depth = collections.Counter(m["esc"]["depth"] for m in es if m["esc"]["armed"])
    log.append("  the top level of the overrides rewound (cutscene depth at pre: moves): "
               + ", ".join(f"{d}: {n}" for d, n in sorted(depth.items())))
    ft = sum((collections.Counter(m["esc"]["first_turn"]) for m in es), collections.Counter())
    log.append("  how the rewound slot's first turn ended (paths): " + ", ".join(f"{k} {n}" for k, n in ft.most_common()))
    kf = sum((collections.Counter(m["esc"]["key"]) for m in es), collections.Counter())
    km = collections.Counter(k for m in es for k in m["esc"]["key"])
    log.append("  where the key went at checkExecVerbs (paths as they reached it, model.Run.key_routes, and the paths "
               "that ended before it; moves): " + ", ".join(f"{k} {n} ({km[k]})" for k, n in kf.most_common()))
    fk = collections.defaultdict(collections.Counter)  # way -> fork kind -> paths
    fm = collections.defaultdict(set)  # (way, fork kind) -> moves
    for m in es:
        for fate, c in m["esc"]["key_forks"].items():
            fk[fate].update(c)
            for kind in c:
                fm[(fate, kind)].add(m["label"])
    log.append("  the forks each way of the key rests on: the kinds of the forks its path had taken before the key "
               "(paths, a path once per kind; moves), " + " and ".join(KEY_FORKS) + " first (a cutscene's data, "
               "model.R_CUTDATA, and an actor's room are not facts):")
    for fate, n in kf.most_common():
        if fate not in fk:
            continue
        c = fk[fate]
        first = [k for k in KEY_FORKS if k in c] + [k for k, _ in c.most_common() if k not in KEY_FORKS]
        log.append(f"    {fate} {n}: " + ", ".join(f"{k} {c[k]} ({len(fm[(fate, k)])})" for k in first))
    for kind in KEY_FORKS:
        ms = sorted({lab for (fate, k), labs in fm.items() if k == kind for lab in labs})
        n = sum(c.get(kind, 0) for c in fk.values())
        log.append(f"  ways of the key resting on a fork on {kind}: {n} paths in {len(ms)} moves"
                   + (f" ({', '.join(ms)})" if ms else ""))
    acc_es = [m for m in acc if m.get("esc")]
    n = sum(len(m["certain"]) for m in acc_es)
    h, hp = sum(len(m["hits"]) for m in acc_es), sum(len(m["hits_post"]) for m in acc_es)
    own = sum(1 for m in acc_es for k in m["certain"] if k in m["esc"]["abort_keys"])
    oh = sum(1 for m in acc_es for k, _, _ in m["hits"] if k in m["esc"]["abort_keys"])
    ohp = sum(1 for m in acc_es for k, _, _ in m["hits_post"] if k in m["esc"]["abort_keys"])
    log.append(f"  certain predictions that are abortCutscene's own writes (the armed esc: fact to 0, script.cpp:1707; "
               f"the rewound slot's wait point at pre to 0, 1699), which hold whenever the skip happens: {own} (hits "
               f"{oh} at done, {ohp} at post); the skips' precision without them: done {prec(h - oh, n - own)}, post "
               f"{prec(hp - ohp, n - own)}; with them: done {prec(h, n)}, post {prec(hp, n)}")
    codes = {m["esc"]["exit_key"] for m in es}
    sites, per_var = verb_key_sites(game, codes, start)
    log.append(f"  VerbOps Key sub-ops in the IR that could give a shown verb the key code {sorted(codes)} (a verb's "
               f"key is not a fact: actions.VERB_KEY): " + ", ".join(f"{k} {n}" for k, n in sorted(sites.items())))
    log += [f"    {line}" for line in per_var]
    log.append("  per move (label, the override rewound: its jump and where it goes, the rewound slot's first turn, "
               "the key and the forks of each way, " + " and ".join(KEY_FORKS) + " only, precision done and post, "
               "paths; run B's keys after the first up to done and after done):")
    for m in es:
        e, ri = m["esc"], m["run_b_inputs"]

        def forks(fate, e=e):
            c = e["key_forks"].get(fate, {})
            on = [f"{k} {c[k]}" for k in KEY_FORKS if k in c]
            return f" [{', '.join(on)}]" if on else ""
        log.append(f"    {m['label']:7} {', '.join(e['armed']) or 'no esc: fact'}"
                   + (f" {e.get('jump', '?')} -> {e.get('target', '?')}" if e["armed"] else "")
                   + f"; turn {', '.join(f'{k} {n}' for k, n in e['first_turn'].items())}"
                   + f"; key {', '.join(f'{k} {n}{forks(k)}' for k, n in e['key'].items())}"
                   + (f"; done {prec(len(m['hits']), len(m['certain']))}, post "
                      f"{prec(len(m['hits_post']), len(m['certain']))}" if m["accepted"] else "; not accepted")
                   + f"; paths {m['paths']}; keys to done: "
                   + (keys_text([t for t in ri["to_done"] if t[1] != "click"]) or "none") + "; after done: "
                   + (keys_text([t for t in ri["to_post"] if t[1] != "click"]) or "none"))
    return log


family = model.family
INNER_REASON = re.compile(r"^\?\([^:]+: ([^;)]+)")  # the unknown an assumed loop exit was taken on
SLOT_FAMILIES = ("script:", "local:", "objscript:", "at:", "frozen:", "esc:")


def conflict_family(key: str) -> str:
    """A conflicting key's group for the log: the result register var 0, a slot or freeze fact, or any
    other fact."""
    if key == f"var:{model.VAR_RESULT}":
        return "var:0"
    return "slot or freeze" if key.startswith(SLOT_FAMILIES) else "other"


def after_keys(p: model.Path, pre: dict) -> set:
    """The keys whose value on path ``p`` moved after its immediate part (the click's frame, up to the first
    yield): the value now differs from the one at the immediate part's end, or did so on a path merged in.
    A path that never got past its immediate part has none; a wait's immediate part is empty."""
    if p.immediate is None:
        return set()
    return set(p.after) | {k for k in set(p.writes) | set(p.immediate)
                           if p.writes.get(k, pre.get(k, 0)) != p.immediate.get(k, pre.get(k, 0))}


def settle_stats(run: model.Run, out: dict, pre: dict, done: dict, post: dict, machine: model.Machine) -> dict:
    """Row 3 (settle): per move, what settle did and what it rests on (the log sums these), and each certain
    prediction marked as made before or after the first yield (by the rule of the turn that last wrote it)."""
    paths = model.feasible(run)
    s = {"first_ends": dict(collections.Counter(str(p.first_end) for p in run.paths)),
         "rounds": max((p.round for p in run.paths), default=0),
         "settle_merges": run.settle_merges, "value_merges": run.value_merges,
         "sentences_run": max((p.sentences for p in run.paths), default=0),
         "settled_moving": sum(1 for p in run.paths if p.end == "settled" and "locals still moved" in (p.end_at or ""))}
    exits, waits = {}, set()
    for p in paths:
        for a in p.assumptions:
            if a[3].startswith(f"?({model.R_LOOP_EXIT}"):
                m = INNER_REASON.match(a[3])
                exits.setdefault((a[0], a[1]), m[1] if m else "?")
            elif a[3].startswith(f"?({model.R_WAIT_OVER}"):
                waits.add((a[0], a[1]))
    s["loop_exits"] = dict(sorted(collections.Counter(exits.values()).items()))
    s["loop_exit_sites"] = sorted(f"{c} {cond}" for c, cond in exits)[:6]
    s["waits_over"] = len(waits)
    ran = sorted(set().union(*(p.ran_by for p in paths))) if paths else []  # sorted: no output rests on set order
    s["ran_by"] = dict(sorted(collections.Counter(next(p.ran_by[n] for p in paths if n in p.ran_by) for n in ran).items()))
    s["not_run"] = {n: next((p.not_run[n] for p in paths if n in p.not_run), "never reached a round")
                    for n in run.pre_slots if n not in ran}
    # every slot a round passed over on some path, with its origin and why (frozen ones too: engine behaviour)
    s["skips"] = sorted(set().union(*(p.skips for p in paths))) if paths else []
    s["multi"] = sorted(set().union(*(p.multi for p in paths))) if paths else []
    s["ambient_slots"] = sorted(set().union(*(p.ambient_slots for p in paths))) if paths else []
    loose = {}  # facts settle's end made unknown (model.Machine._close_settle), by why
    for p in paths:
        for k, v in p.writes.items():
            if isinstance(v, model.Unknown) and v.reason == model.R_SETTLE and ("ambient" in v.what or "instance" in v.what):
                loose.setdefault(k, "an ambient loop run once" if "ambient" in v.what else "two instances")
    s["loosened"] = dict(sorted(collections.Counter(loose.values()).items()))
    s["mouse"] = any(f"var:{v}" in p.writes for p in paths for v in model.MOUSE_VARS)
    fams = collections.Counter()
    keys = collections.Counter()
    for c in sorted({c for p in paths for c in p.conflicts}):
        for f in sorted({conflict_family(k) for k in c[3]}):
            fams[f] += 1
        keys.update(c[3])
    s["conflicts"] = dict(sorted(fams.items()))
    s["conflict_keys"] = dict(sorted(keys.items(), key=lambda kv: (-kv[1], kv[0]))[:5])
    on = sorted(k for k in out["certain"] if k in keys)  # certain predictions on a key that conflicted in this move
    s["conflicted_certain"] = [(k, done.get(k, 0) == out["certain"][k]) for k in on]
    # before / after the first yield, per certain prediction (the precision split)
    aks = [after_keys(p, pre) for p in paths]
    s["split"] = {}
    for k, v in out["certain"].items():
        late = [p for p, ak in zip(paths, aks) if k in ak]
        rule = "before" if not late else (lambda rs: rs.pop() if len(rs) == 1 else "mixed")(
            {p.writer.get(k, "outside a turn") for p in late})
        s["split"][k] = ("after" if late else "before", rule, done.get(k, 0) == v, post.get(k, 0) == v)
    return s


def triggers_check() -> list[str]:
    """Cross-check ENGINE_VARS against the variables engine-triggers.json names at non-boot sites."""
    if not TRIGGERS.exists():
        return [f"engine-triggers.json not found at {TRIGGERS}"]
    sites = json.loads(TRIGGERS.read_text())["sites"]
    names = set()
    for s in sites:
        if s.get("applies_to_mi1_dos_cd") and s["when"] not in ("boot", "save_load", "debug"):
            names |= set(re.findall(r"VAR_[A-Z0-9_]+", s["starts_or_writes"]))
    table = {name for name, _ in ENGINE_VARS.values()}
    inside = {name for name, _ in IN_OPCODES.values()}
    out = []
    missing = sorted(n for n in names - table - inside if n not in LEFT_IN and not HOOK_VARS.match(n))
    out.append(f"  engine-triggers.json names {len(names)} variables at non-boot sites; {len(names & table)} are in "
               f"the table; {len(names & inside)} are written only inside opcodes (IN_OPCODES, left in recall); hook "
               f"numbers and clocks the engine reads: {', '.join(sorted(n for n in names if HOOK_VARS.match(n)))}")
    out += [f"  left in recall: {n}: {LEFT_IN[n]}" for n in sorted(names & set(LEFT_IN))]
    out.append(f"  named there, neither in the table nor explained: {missing or 'none'}")
    extra = sorted(table - names)
    out.append(f"  in the table but not named there (cited from ScummVM directly): {extra or 'none'}")
    return out


def gt_digest() -> str:
    return hashlib.sha256(groundtruth.OUT.read_bytes()).hexdigest()[:12]


def input_name(e: dict) -> str:
    return e["key"] if "key" in e else next(k for k in ("click", "rclick", "move") if k in e)


def run_b_inputs(rows: list[dict]) -> dict[int, dict]:
    """Per ground-truth row (by ``i``): run B's recorded inputs inside the move's window that the scored action does
    not hold, from the explicit movie the ground truth replays (``groundtruth.MOVIE``), with ``frames``, the move's
    first input, done and post frames: ``to_done``, those after the move's first input up to its done frame (the
    first input itself left out; another input in its frame counts), and ``to_post``, those after done up to the
    post frame (the next move's pre). Each is [frame, input, source]:
    the input a key ('.', 'esc', ...) or "click"; the source "own" when the executor's own movie
    (``groundtruth.RUN``/movie.jsonl) has it, else "auto" (the bridge's automatic '.' and Esc, which the explicit
    movie holds as keys). The engine ran them; the model runs only the move's action, so they are not modelled.
    A diagnostic: it changes no number."""
    explicit = [e for e in groundtruth.load_jsonl(groundtruth.MOVIE) if groundtruth.is_input(e)]
    mine = groundtruth.load_jsonl(groundtruth.RUN / "movie.jsonl")
    own = collections.Counter(json.dumps(e, sort_keys=True) for e in mine if groundtruth.is_input(e))
    tagged = []
    for e in explicit:
        k = json.dumps(e, sort_keys=True)
        tagged.append((e["frame"], input_name(e), "own" if own[k] > 0 else "auto"))
        if own[k] > 0:
            own[k] -= 1
    out = {}
    for r in rows:
        first, done, post = r["first_input"], r["done_frame"], r["post_frame"]
        to_done = [list(t) for t in tagged if first is not None and first <= t[0] <= done]
        lead = next((t for t in to_done if t[0] == first and t[2] == r["input_from"]), None)
        if lead is not None:
            to_done.remove(lead)
        out[r["i"]] = {"frames": [first, done, post], "to_done": to_done,
                       "to_post": [list(t) for t in tagged if done < t[0] <= post]}
    return out


def keys_text(inputs: list) -> str:
    """'. 2, esc 1' for the keys of an input list (clicks counted as "click")."""
    c = collections.Counter(name for _, name, _ in inputs)
    return ", ".join(f"{k} {n}" for k, n in sorted(c.items(), key=lambda kv: (-kv[1], kv[0])))


def inputs_log(counted: list) -> list[str]:
    """Run B's inputs inside each move's window that the scored action does not hold (``run_b_inputs``): per kind, the
    moves with a key after the first input up to done, and after done up to post, by key and source, and each such
    move but a dialog line's (the dialog section lists those per move). They change no number."""
    kinds = ["sentence", "walk_to", "dialog_line", "skip_cutscene", "wait", "none"]
    log = ["", "run B's inputs inside each move's window that the scored action does not hold (check.run_b_inputs; "
               "the explicit movie the ground truth replays; own: the executor's movie, auto: the bridge's automatic "
               "'.' and Esc). The engine ran them; the model runs only the action, so they are not modelled:"]
    for side, what in (("to_done", "after the first input, up to done"), ("to_post", "after done, up to post")):
        log.append(f"  {what}:")
        for k in kinds:
            ms = [m for m in counted if m["kind"] == k]
            keyed = [m for m in ms if any(n != "click" for _, n, _ in m["run_b_inputs"][side])]
            clicked = [m for m in ms if any(n == "click" for _, n, _ in m["run_b_inputs"][side])]
            if not ms:
                continue
            c = collections.Counter((n, s) for m in ms for _, n, s in m["run_b_inputs"][side])
            by = sorted(c.items(), key=lambda kv: (-kv[1], kv[0]))
            log.append(f"    {k}: {len(ms)} moves; with a key {len(keyed)}, with a click {len(clicked)}; inputs by kind "
                       f"and source: " + (", ".join(f"{n} {s} {v}" for (n, s), v in by) or "none"))
            if k != "dialog_line" and keyed:
                log.append("      " + "; ".join(
                    f"{m['label']} ({keys_text([t for t in m['run_b_inputs'][side] if t[1] != 'click'])})"
                    for m in keyed))
    return log


def summarize(moves: list[dict], features, machine, nonfacts, kinds, t, digest, start: dict | None = None) \
        -> tuple[list[str], dict]:
    counted = [m for m in moves if m["status"] == "counted"]
    excluded = [m for m in moves if m["status"] == "excluded"]
    gt = groundtruth.OUT
    stamp = time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(gt.stat().st_mtime))
    log = [f"E003 model check, Part I of run B: features {sorted(features)}; path limit {machine.path_limit}, "
           f"step limit {machine.step_limit}; switches on: {', '.join(sorted(machine.switches)) or 'none'}; {t:.1f} s",
           f"ground truth: {len(moves)} rows, {gt.name} ({gt.stat().st_size} bytes, modified {stamp}, sha256 {digest}); "
           f"fact families held: {', '.join(sorted(kinds))}",
           "",
           f"denominator: {len(counted)} counted moves = {len(moves)} rows - {len(excluded)} excluded"]
    log += [f"  excluded {m['label']}: {m['reason']}" for m in excluded]
    none = [m for m in counted if m["kind"] == "none"]
    log.append(f"  counted moves with no action kind: {len(none)}")
    log += [f"    {m['label']}: {m['reason']}" for m in none]
    log += [f"  {m['label']}: {m['reason']}" for m in counted if m["reason"].startswith("via")]

    kinds_order = ["sentence", "walk_to", "dialog_line", "skip_cutscene", "wait", "none"]
    groups = {k: [m for m in counted if m["kind"] == k] for k in kinds_order}
    groups["all"] = counted

    def line(name, ms):
        acc = [m for m in ms if m["accepted"]]
        pc = sum(len(m["certain"]) for m in acc)
        hits = sum(len(m["hits"]) for m in acc)
        hp = sum(len(m["hits_post"]) for m in acc)
        rt = sum(m["recall_total"] for m in ms)
        rc = sum(m.get("recall_certain", 0) for m in ms)
        rp = sum(m.get("recall_possible", 0) for m in ms)
        rta = sum(m["recall_total"] for m in acc)
        rca = sum(m.get("recall_certain", 0) for m in acc)
        rpa = sum(m.get("recall_possible", 0) for m in acc)
        nopred = sum(1 for m in acc if not m["certain"])
        onlyposs = sum(1 for m in acc if not m["certain"] and m["possible"])
        # precision over the facts recall scores: ENGINE_VARS and non-facts left out here too
        pci = sum(1 for m in acc for k in m["certain"] if not excluded_fact(k, nonfacts))
        hitsi = sum(1 for m in acc for k, _, _ in m["hits"] if not excluded_fact(k, nonfacts))
        hpi = sum(1 for m in acc for k, _, _ in m["hits_post"] if not excluded_fact(k, nonfacts))
        return {"name": name, "moves": len(ms), "accepted": len(acc), "predicted": pc, "hits": hits, "hits_post": hp,
                "recall_total": rt, "recall_certain": rc, "recall_possible": rp, "rta": rta, "rca": rca, "rpa": rpa,
                "no_prediction": nopred, "only_possible": onlyposs, "predicted_in": pci, "hits_in": hitsi,
                "hits_post_in": hpi}

    stats = {k: line(k, ms) for k, ms in groups.items()}
    log += ["", f"{'kind':14}{'moves':>6}{'accepted':>16}{'prec. done (hits/certain)':>28}"
                f"{'prec. post (hits/certain)':>28}{'recall':>18}{'recall+possible':>18}{'no pred.':>10}"]
    for k in kinds_order + ["all"]:
        s = stats[k]
        if not s["moves"]:
            continue
        log.append(f"{k:14}{s['moves']:6}{s['accepted']:6} {pct(s['accepted'], s['moves']):>9}"
                   f"{prec(s['hits'], s['predicted']):>28}{prec(s['hits_post'], s['predicted']):>28}"
                   f"{s['recall_certain']:6}/{s['recall_total']:<5}"
                   f"{pct(s['recall_certain'], s['recall_total']):>6}{s['recall_possible']:6}/{s['recall_total']:<5}"
                   f"{pct(s['recall_possible'], s['recall_total']):>6}{s['no_prediction']:10}")
    a = stats["all"]
    with_action = [m for m in counted if m["kind"] != "none"]
    ran = [m for m in counted if m["kind"] in ("sentence", "walk_to")]
    if "esc" in features:
        runs = ran + groups["wait"] + groups["dialog_line"] + groups["skip_cutscene"]
        runs_text = (f"{len(runs)} the model runs at this row (sentence, walk_to, wait, dialog_line, skip_cutscene): "
                     f"{sum(m['accepted'] for m in runs)} ({pct(sum(m['accepted'] for m in runs), len(runs))})")
    elif "dialog" in features:
        runs = ran + groups["wait"] + groups["dialog_line"]
        runs_text = (f"{len(runs)} the model runs at this row (sentence, walk_to, wait, dialog_line): "
                     f"{sum(m['accepted'] for m in runs)} ({pct(sum(m['accepted'] for m in runs), len(runs))}). Skips run "
                     f"nothing at this row (their immediate part is a later feature, so no settle either): they are "
                     f"accepted when usable and predict nothing")
    elif "settle" in features:
        runs_text = (f"{len(ran) + len(groups['wait'])} the model runs at this row (sentence, walk_to, wait): "
                     f"{sum(m['accepted'] for m in ran + groups['wait'])} "
                     f"({pct(sum(m['accepted'] for m in ran + groups['wait']), len(ran) + len(groups['wait']))}). Dialog "
                     f"lines and skips run nothing at this row (their immediate part is a later feature, so no settle "
                     f"either): they are accepted when usable and predict nothing")
    else:
        runs_text = (f"{len(ran)} the model runs at this row (sentence, walk_to): {sum(m['accepted'] for m in ran)} "
                     f"({pct(sum(m['accepted'] for m in ran), len(ran))}). Dialog lines, skips and waits run nothing at "
                     f"this row: they are accepted when usable and predict nothing")
    log += ["",
            f"acceptance: {a['accepted']} of {a['moves']} counted moves ({pct(a['accepted'], a['moves'])}); "
            f"of the {len(with_action)} with an action kind: {pct(a['accepted'], len(with_action))}; of the " + runs_text,
            f"precision (certain predicted changes, micro; hits/certain predictions): done "
            f"{prec(a['hits'], a['predicted'])}; post {prec(a['hits_post'], a['predicted'])}",
            f"precision over the facts recall scores (the engine variables and non-facts below left out of both; "
            f"not the rules' number): done {prec(a['hits_in'], a['predicted_in'])}; "
            f"post {prec(a['hits_post_in'], a['predicted_in'])}",
            f"recall over counted moves: certain {a['recall_certain']}/{a['recall_total']} = "
            f"{pct(a['recall_certain'], a['recall_total'])}; with possible {a['recall_possible']}/{a['recall_total']} = "
            f"{pct(a['recall_possible'], a['recall_total'])}",
            f"recall over accepted moves: certain {a['rca']}/{a['rta']} = {pct(a['rca'], a['rta'])}; with possible "
            f"{a['rpa']}/{a['rta']} = {pct(a['rpa'], a['rta'])}",
            f"accepted moves with no certain prediction: {a['no_prediction']} of {a['accepted']} "
            f"({a['only_possible']} with possible ones only)"]
    why = collections.Counter(re.sub(r"\d+", "N", m["why_not"].split(" (")[0]) for m in counted if not m["accepted"])
    log.append("not accepted, by reason:")
    log += [f"  {n:4} {r}" for r, n in why.most_common()]
    log += [f"    {m['label']} {m['action']}: {m['why_not']}" for m in counted if not m["accepted"] and m["kind"] != "none"]

    log += ["", "recall exclusions:", f"  engine variables written outside opcode handlers ({len(ENGINE_VARS)}):"]
    log += [f"    var {n:3} {name}: {cite}" for n, (name, cite) in ENGINE_VARS.items()]
    log.append(f"  engine variables written only inside an opcode, left in recall ({len(IN_OPCODES)}; row 0's first "
               f"run left them out, the rules file's correction note):")
    log += [f"    var {n:3} {name}: {cite}" for n, (name, cite) in IN_OPCODES.items()]
    log.append(f"    {IN_OPCODES_NOTE}")
    log += triggers_check()
    log.append(f"  the model's non-facts (model.non_fact_vars: every script write unknown by its rules): "
               f"{len(nonfacts)}: " + ", ".join(f"var {n} ({r})" for n, r in sorted(nonfacts.items())))
    exc = collections.Counter(excluded_fact(k, nonfacts) for m in counted for k in m["excluded_changes"])
    log.append(f"  changed facts left out of recall: {sum(exc.values())} of {sum(m['changed'] for m in counted)}; "
               "by variable: " + ", ".join(f"{r} {n}" for r, n in exc.most_common()))
    inside = collections.Counter(IN_OPCODES[int(k[4:])][0] for m in counted for k in m["in_opcodes_changed"])
    hit_in = collections.Counter(IN_OPCODES[int(k[4:])][0] for m in counted for k in m.get("recall_hits", [])
                                 if k.startswith("var:") and int(k[4:]) in IN_OPCODES)
    log.append("  changes of the IN_OPCODES variables, now scored (changed / predicted certain): " + ", ".join(
        f"{name} {inside[name]}/{hit_in[name]}" for name, _ in IN_OPCODES.values()))
    exc_n = sum(1 for m in counted for k in m["excluded_changes"])
    exc_c = sum(len(m.get("excluded_hits", [])) for m in counted)
    log.append(f"  the excluded changes the model predicted anyway (certain): {exc_c} of {exc_n}")

    log += ["", "model runs (counted moves with an action):"]
    paths = [m["paths"] for m in with_action]
    log.append(f"  paths per move: max {max(paths, default=0)}, total {sum(paths)}; moves that hit the path limit: "
               f"{sum(1 for m in with_action if m['limit'])}; path ends: "
               + ", ".join(f"{e} {n}" for e, n in sum((collections.Counter(m['ends']) for m in with_action),
                                                       collections.Counter()).most_common()))
    um = collections.Counter(h for m in with_action for h, _ in m["unmodelled"])
    log.append("  unmodelled statements met (moves): " + (", ".join(f"{h} {n}" for h, n in um.most_common()) or "none"))
    notes = sum((collections.Counter(m["notes"]) for m in with_action), collections.Counter())
    log.append("  notes, partial or deferred effects (paths): " + (", ".join(f"{h} {n}" for h, n in notes.most_common())
                                                                   or "none"))
    log.append(f"  writes with an unknown target: {sum(m['unknown_writes'] for m in with_action)}; predictions in kinds "
               f"the ground truth does not hold (or clocks): {sum(len(m['untracked']) for m in counted)}")
    log.append(f"  forks on a WaitFor* op (moves with one): {sum(1 for m in with_action if m.get('wait_assumptions'))}")
    ak = sum((collections.Counter(m.get("assumption_kinds", {})) for m in with_action), collections.Counter())
    akm = collections.Counter(k for m in with_action for k in m.get("assumption_kinds", {}))
    log.append("  assumptions by kind (distinct decisions per move: where, condition, side taken; moves): " + (", ".join(
        f"{k} {n} ({akm[k]})" for k, n in ak.most_common()) or "none"))
    vc = sum((collections.Counter(m.get("value_caps", {})) for m in with_action), collections.Counter())
    log.append(f"  value sets past model.MAX_VALUES ({model.MAX_VALUES}) made plain unknowns, by site: " + (", ".join(
        f"{k} {n}" for k, n in vc.most_common()) or "none") + f" (moves: {sum(1 for m in with_action if m.get('value_caps'))})")
    g = machine.game
    both = sorted(u for u, fl in g.resistance.items() if len(fl) > 1)
    merged = collections.defaultdict(set)  # the flags of a local number over every room, as before the per-room keys
    for u, fl in g.resistance.items():
        if u[0] == "local":
            merged[u[2]] |= fl
    locals_ = sum(1 for u in g.units if u[0] == "local")
    log.append(f"  F flags from constant start sites (model.Game.resistance): started both ways, so unknown when a freeze "
               f"meets a slot live before the move: {sum(1 for u in both if u[0] == 'global')} global scripts; "
               f"{sum(1 for u in both if u[0] == 'local')} of {locals_} room-local scripts, keyed by room (a start from "
               f"global or object code, or a hook, counts for every room with that local number; merged over rooms by "
               f"number it would be {sum(len(g.local_rooms[n]) for n, fl in merged.items() if len(fl) > 1)}); starts with "
               f"a variable script number, not counted: {len(g.wild_starts)}")
    tu = sum((collections.Counter(m.get("turns", {})) for m in with_action), collections.Counter())
    log.append("  script turns that ended, nested ones included (over paths): " + (", ".join(
        f"{h} {n}" for h, n in tu.most_common()) or "none (row 0 keeps one frame)"))
    log.append(f"  nesting: deepest stack {max((m.get('depth', 0) for m in with_action), default=0)} frames (the engine's "
               f"limit is {model.MAX_NESTING}" + ("" if "settle" not in features else
                                                  " nested runs; a slot runAllScripts resumes is one frame more, not "
                                                  "nested") + "); moves that reached it: "
               f"{sum(1 for m in with_action if m.get('depth', 0) >= model.MAX_NESTING)}; starts of a unit already on the "
               f"stack (recursion guard): {sum(m.get('reentries', 0) for m in with_action)} in "
               f"{sum(1 for m in with_action if m.get('reentries'))} moves; most instructions on one path "
               f"{max((m.get('steps', 0) for m in with_action), default=0)} (limit {machine.step_limit})")
    log.append(f"  paths left out of the predictions (model.INFEASIBLE: an engine error() or the nesting limit aborts "
               f"the game, which run B did not; a spin meets a state again and adds nothing): "
               f"{sum(m.get('infeasible', 0) for m in with_action)} in {sum(1 for m in with_action if m.get('infeasible'))} "
               f"moves (" + ", ".join(f"{e} {sum(m['ends'].get(e, 0) for m in with_action)}" for e in model.INFEASIBLE)
               + f"); moves with no path left: "
               f"{sum(1 for m in counted if (m.get('why_not') or '').startswith('every path left out'))}")
    log.append(f"  merges (paths folded into another where they met): {sum(m.get('merges', 0) for m in with_action)} in "
               f"{sum(1 for m in with_action if m.get('merges'))} moves; paths that spin (back at a jump target in a "
               f"state they had, with no yield): {sum(m['ends'].get('spin', 0) for m in with_action)} in "
               f"{sum(1 for m in with_action if m['ends'].get('spin'))} moves")
    er = sum((collections.Counter(m.get("errors", {})) for m in with_action), collections.Counter())
    log.append("  the engine's error()/warning() cases met, counted not raised (over paths; moves): " + (", ".join(
        f"{k} {n} ({sum(1 for m in with_action if k in m.get('errors', {}))})" for k, n in er.most_common()) or "none"))
    for m in with_action:
        for w, c, d in m.get("error_sites", [])[:2]:
            log.append(f"    {m['label']}: {w} at {c}: {d}")
    for m in with_action:
        flags = {k: v for k, v in (("limit", m["limit"]), ("spin", m["ends"].get("spin", 0)),
                                   ("nesting", m["ends"].get("nesting", 0)), ("step limit", m["ends"].get("limit", 0)
                                                                              - m["limit"] if m["limit"] else
                                                                              m["ends"].get("limit", 0)),
                                   ("unmodelled", len(m["unmodelled"]))) if v}
        if flags:
            log.append(f"    {m['label']}: " + ", ".join(f"{k} {v}" for k, v in flags.items()))
    rs = [m for m in with_action if "(after the turn of" in (m.get("start") or "")]
    log.append(f"  sentence slots live and not frozen at pre, run for their turn before the sentence (runAllScripts): "
               f"{len(rs)} moves" + (": " + ", ".join(m["label"] for m in rs) if rs else ""))
    for end in ("deferred", "deferral unknown", "dropped", "no sentence script"):
        ends = "first_ends" if "settle" in features else "ends"  # settle: how the immediate part ended
        ms = [m for m in with_action if m.get(ends, {}).get(end)]
        if ms:
            log.append(f"  paths {'whose immediate part ends' if ends == 'first_ends' else 'that end'} {end}: "
                       f"{sum(m[ends][end] for m in ms)} in {len(ms)} moves: "
                       + ", ".join(f"{m['label']} ({m[ends][end]} of {m['paths']})" for m in ms))
    ins = [m for m in with_action if m.get("input_script") and m["kind"] in ("sentence", "walk_to")]
    loc = [m for m in ins if m["input_script"]["local"]]
    log.append(f"  the input script (VAR_VERB_SCRIPT) is not run (the action is the sentence): {len(ins)} moves; its "
               f"number at pre is a room's local script in {len(loc)}, where it may do more than queue the sentence "
               f"(a doSentence(STOP), a start of its own, a sentence it does not queue): "
               + (", ".join(f"{m['label']} ({m['input_script']['n']})" for m in loc) or "none"))

    if "settle" in features:
        log += settle_log(counted, with_action, machine, nonfacts)
    if "dialog" in features:
        log += dialog_log(groups["dialog_line"], nonfacts)
    if "esc" in features:
        log += esc_log(groups["skip_cutscene"], machine.game, start or {})
    log += inputs_log(counted)

    acc_run = [m for m in ran if m["accepted"]]
    acc_dl = [m for m in groups["dialog_line"] if m["accepted"]]
    acc_esc = [m for m in groups["skip_cutscene"] if m["accepted"]] if "esc" in features else []
    log += ["", f"engine conditions an accepted move rests on but does not test (actions.UNCHECKED), of {len(acc_run)} "
                f"sentences and walks and {len(acc_dl)} dialog lines (a click on the line's verb: actions.CLICK)"
                + (f" and {len(acc_esc)} skips (the Esc key: actions.ESC)" if "esc" in features else "") + ":"]
    for cond in actions.UNCHECKED:
        if cond in actions.ESC and "esc" not in features:  # the Esc key's: listed from row 5, where skips run
            continue
        ms = [m for m in acc_run + acc_dl + acc_esc if any(
            c == cond for c, _ in m.get("unchecked", []) + m.get("unchecked_click", []) + m.get("unchecked_esc", []))]
        log.append(f"  {cond}: {len(ms)} moves")
        if cond == actions.PARENT:
            log += [f"    {m['label']} {m['action']}: " + ", ".join(n for c, n in m["unchecked"] if c == cond)
                    for m in ms]
    log.append(f"  input on reads VAR_USERPUT for _userPut, which o5_cursorCommand copies into it (script_v5.cpp:935-938): "
               f"script writes of VAR_USERPUT itself in the IR, which would break that: "
               f"{len(model.script_writes(machine.game).get(model.VAR_USERPUT, []))}")

    es = [m for m in ran if m.get("engine_sentence")]
    bad = [m for m in es if not m["engine_sentence"]["matches"]]
    log += ["", f"the engine's last sentence at done (the sentence script's copies of its objects: "
                f"{'; '.join(sorted({', '.join(m['engine_sentence']['cites']) for m in es}))}) against the scored "
                f"action: {len(es)} of "
                f"{len(ran)} sentences and walks read; {len(es) - len(bad)} match; {len(bad)} do not "
                f"({sum(m['accepted'] for m in bad)} of them accepted). The rules' table decides the action; this "
                f"changes no number"]
    log += [f"  {m['label']} {m['action']}: not read (sentence script var {model.VAR_SENTENCE_SCRIPT} has no copies)"
            for m in ran if not m.get("engine_sentence")]
    for m in bad:
        e = m["engine_sentence"]
        how = "no new sentence by done" if e["done"] == e["pre"] else "another sentence ran"
        log.append(f"  {m['label']} {m['action']}: pre {e['pre']}, done {e['done']}: {how}"
                   f"{'' if m['accepted'] else ' (not accepted)'}")

    log += ["", "precision misses (certain, done), by fact kind: " + ", ".join(
        f"{k} {n}" for k, n in collections.Counter(family(k) for m in counted for k, _, _ in m["misses"]).most_common())]
    log.append("  top 10 by fact (moves): " + ", ".join(f"{k} {n}" for k, n in collections.Counter(
        k for m in counted for k, _, _ in m["misses"]).most_common(10)))
    log.append("  top 10 by fact and wrong value (predicted -> done): " + ", ".join(
        f"{k} {v}->{d} {n}" for (k, v, d), n in collections.Counter(
            (k, v, d) for m in counted for k, v, d in m["misses"]).most_common(10)))
    log += [f"  {m['label']} {k}: predicted {v}, done {d}" for m in counted for k, v, d in m["misses"]
            if not k.startswith(("script:", "var:0"))][:40]
    rm = collections.Counter(family(k) for m in counted for k in m["recall_misses"])
    log += ["", f"recall misses by fact kind (counted moves): " + ", ".join(f"{k} {n}" for k, n in rm.most_common())]
    rma = collections.Counter(family(k) for m in counted if m["accepted"] for k in m["recall_misses"])
    log.append(f"recall misses by fact kind (accepted moves): " + ", ".join(f"{k} {n}" for k, n in rma.most_common()))
    top = collections.Counter(k for m in counted if m["accepted"] for k in m["recall_misses"])
    log.append("top 10 missed facts (accepted moves): " + ", ".join(f"{k} {n}" for k, n in top.most_common(10)))
    log.append("next 10: " + ", ".join(f"{k} {n}" for k, n in top.most_common(20)[10:]))
    return log, stats


def settle_log(counted: list, with_action: list, machine: model.Machine, nonfacts: dict) -> list[str]:
    """Row 3's own lines: what settle ran, stopped at, assumed and merged, and precision split by whether
    a certain prediction was made before or after the first yield (and, after it, by the rule of the turn
    that last wrote the fact)."""
    ms = [m for m in with_action if "rounds" in m]
    dialog = "dialog" in machine.features
    runs = {"sentence", "walk_to", "wait"} | ({"dialog_line"} if dialog else set())
    kinds = "the sentences, walks, waits and dialog lines" if dialog else "the sentences, walks and waits"
    n_run = sum(1 for m in ms if m["kind"] in runs)
    log = ["", f"settle (model.Machine._next_round; the accepted moves this row runs, then lets time pass in rounds: "
               f"{n_run}, {kinds}; the counts below also take in the {len(ms) - n_run} accepted moves that run nothing "
               f"at this row, one empty path each):"]
    ends = sum((collections.Counter(m["ends"]) for m in ms), collections.Counter())
    log.append("  how paths end: " + ", ".join(f"{e} {n}" for e, n in ends.most_common()))
    fe = sum((collections.Counter(m["first_ends"]) for m in ms), collections.Counter())
    log.append("  how their immediate part (the click's frame, rows 1-2's run) ended: "
               + ", ".join(f"{e} {n}" for e, n in fe.most_common()))
    lm = [m for m in ms if m.get("settled_moving")]
    log.append(f"  paths settled (no fact changed in a round) while a kept slot's locals still moved (a counter loop "
               f"the path knows; the engine goes on): {sum(m['settled_moving'] for m in lm)} in {len(lm)} moves"
               + (": " + ", ".join(m["label"] for m in lm[:20]) if lm else "") + (" ..." if len(lm) > 20 else ""))
    rl = [m for m in ms if m["ends"].get("round limit")]
    log.append(f"  moves with a path at ROUND_LIMIT ({machine.round_limit} rounds; the facts its last round changed "
               f"made unknown): {len(rl)}" + (": " + ", ".join(f"{m['label']} ({m['ends']['round limit']})" for m in rl)
                                             if rl else ""))
    cy = [m for m in ms if m["ends"].get("cycle")]
    log.append(f"  moves with a path that cycles (a state met again at a round's start; the facts that vary around "
               f"it made unknown): {len(cy)} ({sum(m['ends']['cycle'] for m in cy)} paths)")
    rounds = [m["rounds"] for m in ms]
    log.append(f"  rounds per move: max {max(rounds, default=0)}, median {sorted(rounds)[len(rounds) // 2] if rounds else 0}")
    pl = [m for m in ms if m["limit"]]
    log.append(f"  moves that hit the path limit ({machine.path_limit} paths that carry predictions; paths that "
               f"end in an error, the nesting limit or a spin are not counted toward it): {len(pl)}"
               + (": " + ", ".join(f"{m['label']} ({m['limit']} forks cut)" for m in pl) if pl else ""))
    le = sum((collections.Counter(m["loop_exits"]) for m in ms), collections.Counter())
    lem = sum(1 for m in ms if m["loop_exits"])
    log.append(f"  assumed loop exits (a time-only loop taken to exit; distinct sites per move): {sum(le.values())} in "
               f"{lem} moves; by what the exit test read: " + ", ".join(f"{k} {n}" for k, n in le.most_common()))
    g = machine.game
    cp = g.const_poll_loops()
    tl = sum(1 for u in g.units for lp in g.loops(u) if lp.kind == "time")
    log.append(f"  time-only loops whose exit test reads a variable or bit the body writes only with constants (a reset "
               f"it then waits on; switch const_polls reads it as a polled fact, {'on' if machine.const_polls else 'off'}"
               f" in this run), over the IR: {len(cp)} of {tl} time-only loops, in {len({u for u, _ in cp})} units")
    log.append(f"  waits resolved by time (a WaitFor op that falls through or a delay that ends, distinct sites): "
               f"{sum(m['waits_over'] for m in ms)} in {sum(1 for m in ms if m['waits_over'])} moves")
    rb = sum((collections.Counter(m["ran_by"]) for m in ms), collections.Counter())
    log.append("  slots run, by the rule they first ran by (move, slot pairs): " + ", ".join(
        f"{k} {n}" for k, n in rb.most_common()))
    nr = collections.Counter(why for m in ms for why in m["not_run"].values())
    log.append(f"  slots live at pre that settle never ran (on any path): {sum(nr.values())} in "
               f"{sum(1 for m in ms if m['not_run'])} moves; why: " + ", ".join(f"{k} {n}" for k, n in nr.most_common()))
    sk = collections.Counter((o, w) for m in ms for _, o, w in {tuple(x) for x in m["skips"]})
    pairs = {(m["label"], n, o, w) for m in ms for n, o, w in m["skips"]}
    log.append(f"  slots a round passed over (model.Machine._skip; distinct move, slot, origin and why, any path, any "
               f"round): {len(pairs)} in {sum(1 for m in ms if m['skips'])} moves; by origin and why:")
    for o in ("pre, first turn", "pre, ran before", "path"):
        row = [(w, n) for (oo, w), n in sk.most_common() if oo == o]
        log.append(f"    {o}: {sum(n for _, n in row)}" + (": " + ", ".join(f"{w} {n}" for w, n in row) if row else ""))
    fz = [m for m in ms if any(w == "frozen" for _, _, w in m["skips"])]
    log.append(f"    of these, frozen (engine behaviour: a frozen slot does not run, script.cpp:979): "
               f"{sum(1 for m in ms for _, _, w in m['skips'] if w == 'frozen')} in {len(fz)} moves; the ambient cap: "
               f"{sum(1 for m in ms for _, _, w in m['skips'] if w.startswith('the ambient cap'))} in "
               f"{sum(1 for m in ms if any(w.startswith('the ambient cap') for _, _, w in m['skips']))} moves")
    unk = [m for m in ms if any(w == "its freeze count is not known" for _, _, w in m["skips"])]
    log.append(f"    moves with a slot passed over because its freeze count is not known: {len(unk)}: "
               + ", ".join(f"{m['label']} ({', '.join(sorted({n for n, _, w in m['skips'] if w == 'its freeze count is not known'}))})"
                           for m in unk[:20]) + (" ..." if len(unk) > 20 else ""))
    am = [m for m in ms if m["ambient_slots"]]
    lo = sum((collections.Counter(m["loosened"]) for m in ms), collections.Counter())
    log.append(f"  ambient loops run once (the cap; at settle's end the facts their pass wrote and, while live, their "
               f"at: keys become unknown with the values seen): {sum(len(m['ambient_slots']) for m in am)} slots in "
               f"{len(am)} moves; facts made unknown at settle's end: " + ", ".join(f"{k} {n}" for k, n in lo.most_common()))
    mu = [m for m in ms if m["multi"]]
    log.append(f"  units a recursive start gave a second live instance (one Frame tracked; their at: keys, and a slot fact "
               f"the tracked instance cleared, unknown at settle's end): {sum(len(m['multi']) for m in mu)} in {len(mu)} "
               f"moves" + (": " + ", ".join(f"{m['label']} ({', '.join(m['multi'])})" for m in mu[:12]) if mu else "")
               + (" ..." if len(mu) > 12 else ""))
    cam = sum((collections.Counter({k: n}) for m in ms for k, n in m["assumption_kinds"].items()
               if k == model.R_CAMERA), collections.Counter())
    log.append(f"  loop exits on the camera position (forked once, the side that stays stays): {sum(cam.values())} "
               f"decisions in {sum(1 for m in ms if m['assumption_kinds'].get(model.R_CAMERA))} moves")
    mo = [m for m in ms if m["mouse"]]
    log.append(f"  the mouse variables unknown from the click's frame on (the click moved the mouse): {len(mo)} moves; "
               f"waits that keep pre's mouse (an assumption: no mouse move during '.'): "
               f"{sum(1 for m in ms if m['kind'] == 'wait')} moves")
    cf = sum((collections.Counter(m["conflicts"]) for m in ms), collections.Counter())
    log.append("  order conflicts (two slots run in one round, one writing a fact the other read or wrote; read-read "
               "left out): moves with one, by the conflicting key's group: " + ", ".join(
                   f"{k} {sum(1 for m in ms if m['conflicts'].get(k))}" for k in ("var:0", "slot or freeze", "other"))
               + f"; moves with any: {sum(1 for m in ms if m['conflicts'])}; with one on another fact: "
               f"{sum(1 for m in ms if m['conflicts'].get('other'))}")
    ck = sum((collections.Counter(m["conflict_keys"]) for m in ms), collections.Counter())
    log.append("    top keys (conflicts): " + ", ".join(f"{k} {n}" for k, n in ck.most_common(10)))
    cc = [v for m in ms if m["accepted"] for v in m["conflicted_certain"]]
    log.append(f"    certain predictions on a key that had an order conflict in their move: {len(cc)} "
               f"({sum(h for _, h in cc)} right at done)")
    log.append(f"  merges at a round's start (paths with the same slot positions): {sum(m['settle_merges'] for m in ms)} "
               f"in {sum(1 for m in ms if m['settle_merges'])} moves, of which {sum(m['value_merges'] for m in ms)} "
               f"folded stores that differed into value sets (rows 1-2's join rule at the frame boundary) and the rest "
               f"were the same state")
    log.append(f"  queued sentences settle ran (doSentence, or the click's sentence that had to wait): "
               f"{sum(m['sentences_run'] for m in ms)} in {sum(1 for m in ms if m['sentences_run'])} moves")
    # precision split
    acc = [m for m in ms if m["accepted"]]
    rows = []
    for side in ("before", "after"):
        sp = [v for m in acc for v in m["split"].values() if v[0] == side]
        rows.append(f"{side} the first yield {prec(sum(v[2] for v in sp), len(sp))} "
                    f"(post {prec(sum(v[3] for v in sp), len(sp))})")
    then = "the sentence or the input script" if dialog else "the sentence"
    log.append("  precision (certain, done; hits/certain predictions) by when the prediction was made (\"before the "
               f"first yield\": the click's frame, its runAllScripts turns and then {then}; a "
               "wait has none): " + "; ".join(rows))
    by = collections.defaultdict(list)
    for m in acc:
        for v in m["split"].values():
            if v[0] == "after":
                by[v[1]].append((v[2], v[3]))
    log.append("  after the first yield, by the rule of the settle turn that last wrote the fact (done, post): " + "; ".join(
        f"{r} {prec(sum(d for d, _ in h), len(h))}, {prec(sum(q for _, q in h), len(h))}"
        for r, h in sorted(by.items(), key=lambda kv: -len(kv[1]))))
    miss = collections.Counter((k, v[1]) for m in acc for k, v in m["split"].items() if v[0] == "after" and not v[2])
    log.append("  top misses after the first yield (fact, rule): " + ", ".join(
        f"{k} [{r}] {n}" for (k, r), n in miss.most_common(12)))
    return log


def dialog_log(moves: list, nonfacts: dict) -> list[str]:
    """Row 4's own lines (dialog; they change no number): what the clicks on the lines' verbs ran (``click_stats``),
    which slots resumed by settle's rule c, and precision and recall split by whether a room-local or object script
    was live when the menu showed (the rules note's 37 of 60, here counted from the pre facts)."""
    acc = [m for m in moves if m["accepted"]]
    log = ["", f"dialog lines (model.Machine.run_verb: a left click on the line's verb, so the input script installed at "
               f"pre runs with [kVerbClickArea, verb, 1]): {len(acc)} of {len(moves)} accepted"]
    log += [f"  not accepted: {m['label']} {m['action']}: {m['why_not']}" for m in moves if not m["accepted"]]
    ins = collections.Counter((m["input_script"]["n"], "room-local" if m["input_script"]["local"] else "global")
                              for m in moves if m.get("input_script"))
    log.append("  input script at pre (number, kind: moves): " + ", ".join(f"{n} {k}: {c}" for (n, k), c in
                                                                         sorted(ins.items())))
    cl = [m for m in moves if m.get("click")]
    ft = sum((collections.Counter(m["click"]["input_first_turn"]) for m in cl), collections.Counter())
    log.append("  how the input script's first turn ended (paths; \"did not run\": the path ended in a turn before the "
               "click, by how): " + ", ".join(f"{k} {n}" for k, n in ft.most_common()))
    fk = sum((collections.Counter(m["click"]["input_forks"]) for m in cl), collections.Counter())
    log.append(f"  decisions the input script forked on (distinct per move): {sum(fk.values())} in "
               f"{sum(1 for m in cl if m['click']['input_forks'])} moves, by kind: "
               + (", ".join(f"{k} {n}" for k, n in fk.most_common()) or "none")
               + f"; writes with an unknown target at its instructions: {sum(m['click']['input_unknown_writes'] for m in cl)}"
               f" in {sum(1 for m in cl if m['click']['input_unknown_writes'])} moves")
    dr = [m for m in cl if m["click"]["drift"]]
    dk = collections.Counter(f"{d[0]} {d[1]}->{d[2]}" for m in dr for d in m["click"]["drift"])
    log.append(f"  paths whose click-frame turns changed what checkExecVerbs reads, before the click (model.Path."
               f"input_drift; usable and the input script read them at pre, the spec; switch input_after_turns reads "
               f"them after the turns): {sum(m['click']['drift_paths'] for m in dr)} paths in {len(dr)} moves"
               + (": " + ", ".join(f"{k} ({n} moves)" for k, n in dk.most_common()) + "; "
                  + ", ".join(f"{m['label']} ({m['click']['drift_paths']} of {m['paths']})" for m in dr) if dr else ""))
    pk = collections.Counter(k for m in cl for k in m["click"]["polled_kinds"])
    none = [m for m in cl if not m["click"]["polled_by"]]
    log.append(f"  slots that resumed by settle's rule c (live at pre, polling a fact the path changed: the script waiting "
               f"on the menu, found by running the scripts): in {len(cl) - len(none)} of {len(cl)} moves; moves with one "
               f"of each kind: " + ", ".join(f"{k} {n}" for k, n in sorted(pk.items())))
    log += [f"    {m['label']}: none; slots live at pre not run: "
            + (", ".join(f"{n} ({w})" for n, w in m.get("not_run", {}).items()) or "none") for m in none]
    log.append("  by whether a room-local or object script was live at pre (moves, accepted, precision done and post, "
               "recall certain and with possible):")
    for live in (True, False):
        g = [m for m in cl if m["click"]["local_or_object_live"] == live]
        ga = [m for m in g if m["accepted"]]
        pc = sum(len(m["certain"]) for m in ga)
        rt = sum(m["recall_total"] for m in g)
        log.append(f"    {'a room-local or object script live' if live else 'neither live'}: {len(g)} moves, {len(ga)} "
                   f"accepted; precision done {prec(sum(len(m['hits']) for m in ga), pc)}, post "
                   f"{prec(sum(len(m['hits_post']) for m in ga), pc)}; recall {sum(m.get('recall_certain', 0) for m in g)}"
                   f"/{rt} = {pct(sum(m.get('recall_certain', 0) for m in g), rt)}, with possible "
                   f"{pct(sum(m.get('recall_possible', 0) for m in g), rt)}")
    gap = [m["run_b_inputs"]["frames"][1] - m["run_b_inputs"]["frames"][0] for m in moves]
    near = [m for m, g in zip(moves, gap) if g <= 4]
    log.append(f"  frames from the click to done: median {sorted(gap)[len(gap) // 2] if gap else 0}; within 4 frames in "
               f"{len(near)} moves, {sum(1 for m in near if any(n == '.' for _, n, _ in m['run_b_inputs']['to_done']))} "
               f"of them with a '.' after the click (run B's inputs, below; not modelled)")
    log.append("  per move (label, verb, input script, slots resumed by rule c, precision done and post, paths; frames "
               "from the click to done; run B's keys after the click up to done, and after done up to post):")
    for m in cl:
        c, ri = m["click"], m["run_b_inputs"]
        log.append(f"    {m['label']:7} verb {m['action']['verb']} input {m['input_script']['n']}"
                   f"{' (local)' if m['input_script']['local'] else ''}; rule c: {', '.join(c['polled_by']) or 'none'}; "
                   f"done {prec(len(m['hits']), len(m['certain']))}, post {prec(len(m['hits_post']), len(m['certain']))}; "
                   f"paths {m['paths']}" + (f", {m['limit']} forks cut by the path limit" if m["limit"] else "")
                   + f"; {ri['frames'][1] - ri['frames'][0]} frames; keys to done: "
                   + (keys_text([t for t in ri["to_done"] if t[1] != "click"]) or "none") + "; after done: "
                   + (keys_text([t for t in ri["to_post"] if t[1] != "click"]) or "none"))
    return log


def growth_row(features, stats, n_counted) -> str:
    a = stats["all"]
    row = model.row_of(features)
    row = row if row is not None else "custom"
    return (f"| {row} | {', '.join(f for f in model.FEATURES if f in features)} | {n_counted} | "
            f"{pct(a['accepted'], a['moves'])} | {prec(a['hits'], a['predicted'])} | "
            f"{prec(a['hits_post'], a['predicted'])} | {pct(a['recall_certain'], a['recall_total'])} | "
            f"{pct(a['recall_possible'], a['recall_total'])} | {a['no_prediction']} |")


def write_growth(features, line: str) -> None:
    path = OUT / "growth.md"
    head = ["# E003 model check: growth table", "",
            "One row per feature set (check.py). Percentages over Part I's counted moves; precision over certain "
            "predicted changes at the done frame and at the next move's start, with the certain hits and certain "
            "predictions behind it in brackets; recall over the tracked facts that changed, less the excluded ones "
            "(engine variables written outside opcode handlers, and the model's non-facts). Rows (model.ROWS, "
            "cumulative): 0 own_entry, 1 + dispatch and calls, 2 + room_change, 3 + settle, 4 + dialog, 5 + esc. Rows "
            "0-2 run only sentences and walks, up to the end of the click's frame; row 3 lets time pass after them in "
            "rounds and runs waits the same way; row 4 runs a dialog line as a click on its verb (the input script, "
            "then settle); row 5 runs a skip as the Esc key (abortCutscene rewinds the armed override, the Esc frame "
            "runs, then settle). Dialog lines before row 4, and skips in rows 0-4, run nothing, so they count as "
            "accepted when usable and predict nothing (check-part1-rowN.log gives acceptance over the moves the model "
            "runs, the counts behind every number, the settle counts and precision split from row 3, the dialog "
            "counts from row 4, the Esc counts from row 5, and the ground truth file's hash). Every row runs with all "
            "switches off (model.SWITCHES).", "",
            "| row | features | moves | accepted % | precision done % (hits/certain) | precision post % (hits/certain) "
            "| recall % | recall with possible % | moves with no prediction |",
            "|---|---|---|---|---|---|---|---|---|"]
    rows = [r for r in path.read_text().splitlines() if r.startswith("| ") and not r.startswith(("| row", "|---"))] \
        if path.exists() else []
    key = f"| {', '.join(f for f in model.FEATURES if f in features)} |"
    rows = [r for r in rows if key not in r] + [line]
    # in row order (custom feature sets last, in the order they ran)
    rows.sort(key=lambda r: (0, int(r.split("|")[1])) if r.split("|")[1].strip().isdigit() else (1, 0))
    path.write_text("\n".join(head + rows) + "\n")


# --- scoring the moves on many cores (--jobs above 1, or --resume) -------------------------------------------------

def header(row: dict, game: model.Game, inputs: dict) -> tuple[dict, actions.Action | None]:
    """A move's record before scoring, as ``main``'s loop builds it (the rules' table: ``classify``), and its action."""
    status, action, reason = classify(row, game)
    return {"i": row["i"], "label": label(row), "name": row["name"], "status": status, "reason": reason,
            "kind": kind_name(action), "action": action.as_dict() if action else None,
            "run_b_inputs": inputs[row["i"]]}, action


def head_line(m: dict) -> str:
    """What a kept record was scored for: the move's header (its status, kind and action among them) less run B's
    inputs, which ``score_move`` does not read, as one JSON line."""
    return json.dumps({k: v for k, v in m.items() if k != "run_b_inputs"}, sort_keys=True, default=str)


_JOB = {}  # what a forked worker reads: set by score_parallel before the fork, never changed while workers run


def _score_job(k: int) -> tuple:
    """A worker's task: score row ``k`` of ``_JOB`` with the parent's machine (its copy, from the fork)."""
    j = _JOB
    out, sec, rss = parallel.measure(score_move, j["rows"][k], j["actions"][k], j["machine"], j["nonfacts"],
                                     j["kinds"])
    return k, out, json.dumps(out, default=str), sec, rss


def score_parallel(rows, machine, nonfacts, kinds, inputs, jobs: int, store=None, times=None, prior=None) -> list[dict]:
    """Every row's record, as ``main``'s loop gives it, with the counted moves scored in ``jobs`` forked processes
    (start method fork: each has the parent's game, rows and machine, nothing is loaded again). Each worker scores
    whole moves and returns ``score_move``'s dict and its JSON line; the parent checks the line against the dict it
    received (a pickle round trip must not change what is written), keeps the record in ``store`` as it arrives, and
    builds the records in row order. A record ``store`` holds for the move's label and header (``head_line``) is used
    instead of scoring the move. Moves are submitted costliest first by ``prior`` ({label: seconds}, an earlier run's
    times file; moves it lacks go first, in row order), which changes no record and no order of the output.
    ``times``: filled with label -> (kind, CPU seconds, peak RSS in MB, paths)."""
    heads = [header(row, machine.game, inputs) for row in rows]
    outs, todo = {}, []
    for k, (m, _) in enumerate(heads):
        if m["status"] != "counted":
            continue
        got = store.get(m["label"], head_line(m)) if store else None
        if got is None:
            todo.append(k)
        else:
            outs[k] = got
    prior = prior or {}
    todo.sort(key=lambda k: -prior.get(heads[k][0]["label"], math.inf))  # stable: ties keep row order
    if todo:
        _JOB.update(rows=rows, actions=[a for _, a in heads], machine=machine, nonfacts=nonfacts, kinds=kinds)
        try:
            with ProcessPoolExecutor(max_workers=min(jobs, len(todo)),
                                     mp_context=multiprocessing.get_context("fork")) as ex:
                futures = [ex.submit(_score_job, k) for k in todo]
                try:
                    for f in as_completed(futures):
                        k, out, line, sec, rss = f.result()
                        m = heads[k][0]
                        if json.dumps(out, default=str) != line:
                            raise SystemExit(f"{m['label']}: the record changed on its way from the worker")
                        outs[k] = (out, sec, rss)
                        if store:
                            store.put(m["label"], head_line(m), out, line, sec, rss)
                except BaseException:
                    for f in futures:
                        f.cancel()
                    raise
        finally:
            _JOB.clear()
    moves = []
    for k, (m, _) in enumerate(heads):
        if m["status"] == "counted":
            out, sec, rss = outs[k]
            m |= out
            if times is not None:
                times[m["label"]] = (m["kind"], sec, rss, m["paths"])
        moves.append(m)
    return moves


def score_many(rows, machine, nonfacts, kinds, inputs, features, switches, jobs: int, resume, key, t0) -> list[dict]:
    """``--jobs`` above 1 or ``--resume``: the records by ``score_parallel``, with the folder ``resume`` under ``key``
    (``parallel.run_key``, taken before the game and the ground truth were read: if they or the sources changed since,
    nothing is scored or kept); one line on stderr (``t0``: the run's start) and the times file (``write_times``)."""
    store = None
    if resume:
        if parallel.run_key(features, switches, gt_digest()) != key:
            raise SystemExit("the sources, the game's files or the ground truth changed while they were read; run again")
        store = parallel.Store(resume, key)
    times = {}
    moves = score_parallel(rows, machine, nonfacts, kinds, inputs, jobs, store, times, prior_times(features, switches))
    secs = sorted(((v[1], lab) for lab, v in times.items()), reverse=True)
    print(f"scored {len(times)} moves with {jobs} job(s) in {time.time() - t0:.1f} s; CPU {sum(s for s, _ in secs):.1f} s; "
          f"slowest {', '.join(f'{lab} {s:.1f} s' for s, lab in secs[:3])}; loaded from --resume: "
          f"{store.loaded if store else 0}", file=sys.stderr)
    OUT.mkdir(exist_ok=True)
    write_times(features, switches, moves, times)
    return moves


def times_stem(features, switches) -> str:
    """The stem of a run's files: check-part1-rowN for a growth row, check-part1-rowN-<switches> with a switch on,
    check-part1 for another feature set (as its jsonl)."""
    row = model.row_of(features)
    if switches:
        return f"check-part1-{'row' + str(row) if row is not None else 'custom'}-{'-'.join(sorted(switches))}"
    return f"check-part1-row{row}" if row is not None else "check-part1"


def prior_times(features, switches) -> dict:
    """{label: seconds} from this run's times file of an earlier run, else that of its row without the switches,
    else that of the nearest lower row: only the order moves start in rests on it."""
    row = model.row_of(features)
    stems = [times_stem(features, switches)] + [f"check-part1-row{r}" for r in range(row, -1, -1)] \
        if row is not None else [times_stem(features, switches)]
    return parallel.read_times(OUT / f"{s}.times.tsv" for s in stems)


def write_times(features, switches, moves: list, times: dict) -> None:
    parallel.write_times(OUT / f"{times_stem(features, switches)}.times.tsv",
                         [(m["label"], *times[m["label"]]) for m in moves if m["label"] in times])


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--features", default="own_entry", help="comma-separated model features (model.FEATURES)")
    ap.add_argument("--switches", default="", help="comma-separated model switches to turn on (model.SWITCHES); a run "
                                                  "with any writes only check-part1-<row>-<switches>.*")
    ap.add_argument("--jobs", type=int, default=1, help="processes that score moves (default 1: one after another, "
                                                        "in this process)")
    ap.add_argument("--resume", type=Path, help="folder that keeps each finished move's record; a rerun loads them")
    args = ap.parse_args()
    if args.jobs < 1:
        raise SystemExit("--jobs must be 1 or more")
    features = frozenset(f.strip() for f in args.features.split(",") if f.strip())
    switches = frozenset(f.strip() for f in args.switches.split(",") if f.strip())
    t0 = time.time()
    # --resume: the kept records' key, from the sources as imported and the game and ground truth before they are read
    key = parallel.run_key(features, switches, gt_digest(), parallel.AT_IMPORT) if args.resume else None
    machine = model.Machine(features, switches=switches)
    nonfacts = model.non_fact_vars(machine.game)
    digest = gt_digest()
    rows = groundtruth.load()
    kinds = held_families(rows)
    inputs = run_b_inputs(rows)
    if args.jobs > 1 or args.resume:
        moves = score_many(rows, machine, nonfacts, kinds, inputs, features, switches, args.jobs, args.resume, key, t0)
    else:  # one after another, in this process
        moves = []
        for row in rows:
            status, action, reason = classify(row, machine.game)
            m = {"i": row["i"], "label": label(row), "name": row["name"], "status": status, "reason": reason,
                 "kind": kind_name(action), "action": action.as_dict() if action else None,
                 "run_b_inputs": inputs[row["i"]]}
            if status == "counted":
                m |= score_move(row, action, machine, nonfacts, kinds)
            moves.append(m)
    if (after := gt_digest()) != digest:
        raise SystemExit(f"the ground truth changed while it was scored (sha256 {digest} -> {after}); run again")
    log, stats = summarize(moves, features, machine, nonfacts, kinds, time.time() - t0, digest, rows[0]["pre"])
    OUT.mkdir(exist_ok=True)
    row = model.row_of(features)
    if switches:  # a switch is measured on its own: the rows, the latest run and growth.md stay as they are
        name = f"check-part1-{'row' + str(row) if row is not None else 'custom'}-{'-'.join(sorted(switches))}"
        (OUT / f"{name}.jsonl").write_text("".join(json.dumps(m, default=str) + "\n" for m in moves))
        (OUT / f"{name}.log").write_text("\n".join(log) + "\n")
        print("\n".join(log))
        return
    with (OUT / "check-part1.jsonl").open("w") as fh:
        for m in moves:
            fh.write(json.dumps(m, default=str) + "\n")
    (OUT / "check-part1.log").write_text("\n".join(log) + "\n")
    if row is not None:  # each row's log and moves are kept; check-part1.log/.jsonl are the latest run
        (OUT / f"check-part1-row{row}.log").write_text("\n".join(log) + "\n")
        (OUT / f"check-part1-row{row}.jsonl").write_text((OUT / "check-part1.jsonl").read_text())
    n_counted = sum(1 for m in moves if m["status"] == "counted")
    write_growth(features, growth_row(features, stats, n_counted))
    print("\n".join(log))


if __name__ == "__main__":
    main()
