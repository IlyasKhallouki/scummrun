"""E003 steps 1 and 3: an abstract SCUMM v5 machine over facts, reading the scripts through ir.py.

usage: from model import Machine, FactOracle
       run = Machine().run_entry(FactOracle(facts), obj, verb)   (row 0: the object's own entry)
       run = Machine({"own_entry", "dispatch", "calls"}).run_sentence(FactOracle(facts), verb, a, b)   (row 1-)
       run = Machine(ROW3).run_sentence(...) / .run_wait(FactOracle(facts))   (row 3: then time passes)
       run = Machine(ROW4).run_verb(FactOracle(facts), verb)   (row 4: a click on a verb, a dialog line)
       run = Machine(ROW5).run_esc(FactOracle(facts))   (row 5: the Esc key, a skipped cutscene)

**State.** A path reads a fact (``facts.py``'s keys: ``var:N``, ``bit:N``, ``owner:O``, ``state:O``,
``class:O:C``, ``room``, ``script:N``, ``local:N``, ``objscript:O``, ``verb:V``, ``cutscene``, ...) from
its own **store** of writes first and from the **oracle** otherwise. The oracle is an interface
(``Oracle``): ``FactOracle`` answers from a fact map, where an absent key is 0, the engine's clocks
are timers and a kind the map does not hold is "not held". A symbolic oracle that answers every read
with an unknown and records it as a precondition fits the same interface; it is not built yet.

**Values** are ints or ``Unknown(reason, what)``: a random draw, an actor or object position or
distance, actor state the dump does not give, a timer, a script local the machine was not given, a
fact the oracle does not hold, or a value the machine cannot work out. Arithmetic on an unknown is
unknown. Variables and locals are int32 (``_scummVars``, ``vm.localvar``: script.h:136), bits are 0/1
(script.cpp writeVar), and o5's comparisons cast both sides to int16 (script_v5.cpp:1532-1677; the
zero tests do not). An unknown may carry the set of values it can take (``one_of``, at most
MAX_VALUES): arithmetic, tests and fact getters then work value by value, and a test all values agree
on does not fork. A set, or a product of sets, past MAX_VALUES becomes a plain unknown; each time is
counted (``VALUE_CAPS``, kept per run in ``Run.value_caps``). Sets come from merged paths (below),
from findInventory with calls on (one of the owner's objects for an index in 1..count, else 0:
``_find_inventory``), and from VAR_ROOM_FLAG at a room change ({old, 1}). Row 0 makes none, so its
runs are unchanged.

**Paths.** A frame holds a unit, an instruction index and locals; a path holds a stack of frames and
calls (``Call``: engine code between nested runs), one frame at row 0. A jump whose condition is
unknown **forks**: both successors run, each path records the assumption (where, condition, the side
taken, why it was unknown). At row 0 a path ends at a yield, at the script's end, at a room change
that kills its slot; with calls on, at the end of the first script's turn (yield, end, parked,
stopped, chained, room). Any path also ends at an unmodelled statement, at the engine's nesting limit,
or at a limit: ``PATH_LIMIT`` paths per run (a fork past it ends the path with "limit"; paths that
ended in an engine error, the nesting limit or a spin carry no prediction and are not counted toward
it) and ``STEP_LIMIT`` instructions per path in one frame. All are reported per run. With calls on, paths **merge** where
they meet (``Machine._drive``): at a jump target (the end of an if, a loop's head) and where a nested
run returns to the opcode that started it, paths with the same stack become one, each differing fact
or local the set of its values; a later test on such a value forks again when the values disagree.
This keeps forks inside a nested run from multiplying the paths of everything after it; it can lose a
link between two facts (more possible values), never make a wrong value certain. Also with calls on, a
path that comes back to a jump target in a machine state it had in the same turn **spins**: every way
on from there is run by the path that met the state first, so it ends ("spin", counted, left out of
the predictions; ``Machine._spins``). With calls on, the engine's error() cases end a path as
"error" (or "nesting" past the nesting limit), counted and left out of the predictions: the game
would abort there, and the recorded run did not; its warning() cases are counted only.

**Statements.** Every statement kind the IR has (ir.log: 124) has a rule in ``HEADS``: its effect on
the facts with a ScummVM cite, "no tracked effect" when it only touches what the facts do not hold
(sound, actors, text, camera, walk boxes, resources), "yield", "wait" (a fork: blocked or not),
"end", "deferred" when its effect belongs to a later feature, or "unmodelled", which ends the path
and is counted. Some ops also write engine variables the check leaves out of recall (print and
printEgo: VAR_HAVE_MSG and VAR_TALK_ACTOR; startSound: VAR_LAST_SOUND); the machine does not write
those, and the rule's text says so. Partial effects (drawObject's clearing of same-place objects,
the inventory list behind getInventoryCount, a room change's kills) are kept as notes on the path. A write whose target the machine cannot work out (an index or object number
that is unknown) is kept in ``unknown_writes`` and the path goes on; later reads do not see it.

Engine effects the IR leaves to the interpreter (ir.py's list), all implemented:

* an embedded ``<VAR_RESULT = getter(...)>`` writes var 0 and its value is var 0 (o5_expression case 6,
  script_v5.cpp:1180-1184);
* classOfIs and setClass: a class with the 0x80 bit means "has" / "set", without it "has not" /
  "clear"; setClass with class 0 clears every class (script_v5.cpp:642-689, 1490-1517);
* setVarRange's first argument is the first variable, bit or script local it writes; each value goes
  to the next one (script_v5.cpp:2754-2768; writeVar sends bit and local numbers on, script.cpp:
  805-840);
* hook scripts run on the spot by runScript(hook, 0, 0), without R (script.cpp:38-107):
  VAR_INVENTORY_SCRIPT for pickupObject, setOwnerOf, setObjectName and setCameraFollows,
  VAR_SCROLL_SCRIPT for setCameraAt, VAR_CUTSCENE_START_SCRIPT and VAR_CUTSCENE_END_SCRIPT for cutscene
  and endCutscene. A hook is a start like any other (below);
* a WaitFor* op blocks or falls through on engine state the facts do not hold (o5_wait,
  script_v5.cpp:3272-3310), so it forks like an unknown condition: blocked, the slot yields at the
  wait itself; else the path goes on;
* pickupObject: owner = VAR_EGO's value, class Untouchable set, state 1, then the inventory hook
  (script_v5.cpp:2021-2034); setOwnerOf(o, 0) stops o's scripts (object.cpp:122-181);
* VerbOps New/On/Off/Dim/Delete and saveVerbs/restoreVerbs change ``verb:V``, the verb's visibility
  (verbid set, saveid 0, curmode 1: the bridge's verbVisible, the engine's own hit test);
* getters that read facts are evaluated: getObjectOwner, getObjectState, classOfIs, isScriptRunning,
  getInventoryCount (from owner facts), getVerbEntryPoint (from the IR and owner facts); getters of
  positions, random draws, actor state, sounds and strings are unknown;
* loadRoom and loadRoomWithEgo set ``room``; a non-recursive start stops its target first
  (runScript, runObjectScript: script.cpp:65-66, 117-118), whatever slot runs it.

**Features.** What runs is switched by named features; ``ROWS`` groups them into the growth table's
cumulative rows (row 0 own_entry; row 1 + dispatch and calls, which make sense only together; row 2 +
room_change; row 3 + settle; row 4 + dialog; row 5 + esc):

* ``own_entry`` (row 0): object A's own entry for the verb (``Game.entry``: the
  verb's event, else the 0xFF default, as ir.py resolves it), with locals unknown, against the pre
  state, until the first yield or end, following both sides of unknown conditions. A start (startScript,
  chainScript, startObject, a hook) is recorded as ``script:N`` / ``local:N`` / ``objscript:O`` = 1
  and its body is not run; a stop sets it to 0. A room change sets ``room`` and ends a room, local or
  not-owned object unit's path; the kills and the exit and entry scripts it runs are left to
  ``room_change`` (a note on the path). The unit's own slot is a fact too: when the path yields,
  ``objscript:A`` is 1 and ``at:obj:A:W`` is its one wait point (after breakHere or a delay the
  next instruction, at a blocked wait the wait itself: facts.py); when it ends or is killed, the
  slot, its wait points and any override it armed are gone. A stop also clears the stopped slot's
  ``at:`` and ``esc:`` keys. A start that may stop a running instance (not recursive, or
  chainScript from a slot whose R flag is not known) turns that unit's ``at:`` and ``esc:`` keys
  that hold into unknowns: the old instance is gone, and the new one, not run here, may stop at the
  same place. A recursive start leaves them. An object slot never has R in v5 (o5_startObject and
  the engine's verb code start it without, script_v5.cpp:2889, script.cpp:1432), so a chainScript
  from the entry stops a running copy of its target. ``cutscene`` is the cutscene depth and
  ``esc:<unit>:W`` the top level's armed override: cutscene and endCutscene move the depth and
  hide the override, beginOverride arms one at the jump after it;
* ``dispatch`` (``run_sentence``): the click's frame as the engine runs it (scumm.cpp:3231-3255). The
  pre state is the frame before's decision point (groundtruth.py), so in the click's frame
  runAllScripts (scumm.cpp:3231) runs first, then checkExecVerbs (3232) takes the click and the input
  script queues the sentence, then checkAndRunSentenceScript (script.cpp:1174-1247, after the decision
  point, scumm.cpp:3254-3255) pops it and runs VAR_SENTENCE_SCRIPT. Of runAllScripts, a live and
  unfrozen slot of the sentence script's number has its turn first (``_eng_resume``: from its one
  ``at:`` point, locals unknown; one stopped by a delay forks on its timer), since that turn decides
  whether the sentence waits; the turns of the other live slots are not run (feature settle).
  checkAndRunSentenceScript's checks then read the path's own store (``_eng_sentence``), in order: a
  slot of the sentence script's number still live and not frozen holds the sentence back
  (script.cpp:1186-1191): the path ends **deferred**, counted, and keeps the writes of the slot's turn;
  the queued sentence's own freeze count (script.cpp:1193) is 0, as doSentence sets it
  (script.cpp:1171); the MI1 note workaround (script.cpp:1199, 1275-1297) is an enhancement and off;
  a sentence with an object B equal to A is dropped (script.cpp:1203-1204); VAR_SENTENCE_SCRIPT 0
  runs nothing (script.cpp:1245). Otherwise runScript(VAR_SENTENCE_SCRIPT, 0, 0, [verb, A, B]) with
  no caller to resume (``_currentScript = 0xFF``, script.cpp:1244-1246): not freeze resistant, not
  recursive, so it stops a (frozen) running copy first, and the path starts at its first
  instruction. A walk_to is the sentence with the walk verb and B = 0. Object A's entry is reached
  only through what the sentence script does. The input script (VAR_VERB_SCRIPT), which queues the
  sentence on the click (verbs.cpp checkExecVerbs, script.cpp:1478-1511), is not run: the action is
  the sentence (plan.md step 1), so what the input script does instead of or besides queueing it
  (a doSentence(STOP), a start of its own, a sentence it does not queue) is not seen;
* ``calls``: every start runs its callee nested, as runScriptNested does (script.cpp:344-390), on a
  stack of frames per path. startScript runs global script N, or room-local N from
  _numGlobalScripts on (script.cpp:70-86; a local N the room lacks is the engine's error(), below);
  startObject runs runObjectScript (script.cpp:109-154): it stops the object's slots first, then
  runs its entry for the verb or 0xFF in a slot whose kind is where the object is (whereIsObject,
  object.cpp:355-380), nothing when it is nowhere or has no entry; chainScript kills the running
  slot and runs N with that slot's F and R flags (script_v5.cpp:834-851), so N takes its place
  under the same caller; the hooks run by runScript(VAR(hook), 0, 0, args) (``Call``): the
  inventory hook with Local[0] = 1 for pickupObject (script_v5.cpp:2033) and 0 otherwise
  (object.cpp:173, 1314, camera.cpp:85; v5 passes 0, object.cpp:106), the cutscene start hook with
  the cutscene's own list and the end hook with [the level's cutSceneData] (script.cpp:1632-1687),
  the scroll hook with none (camera.cpp:52-55). Call-site arguments become the callee's locals,
  the rest 0 (getWordVararg, script_v5.cpp:3430-3441). A non-recursive start stops every slot of
  the number first, frames on the stack too (stopScript, stopObjectScript, script.cpp:262-327).
  The callee runs to its first yield or its end; then runScriptNested's test decides whether the
  caller goes on: only if its slot is not dead and its freeze count is 0 (script.cpp:377-390).
  When it does not, the caller's turn ends there: a dead caller is gone, a live one is **parked**
  right after the opcode (its ``at:`` is the next instruction, facts.py's third stop kind), and
  the opcode's remaining engine code still runs (a later nested run in the same opcode no longer
  resumes it: ``_currentScript`` stays 0xFF). A callee that yields stays live (its slot fact 1 and
  ``at:<unit>:W``); one that ends is not live. Freeze counts are kept per slot: freezeScripts(flag)
  adds 1 to every live slot but the running one, skipping freeze-resistant ones below flag 0x80,
  then sets beginCutscene's caller to 0 (script.cpp:914-941); unfreezeScripts takes 1 off every
  frozen slot (script.cpp:943-966). A slot started in the path knows its count and its F flag. A
  slot live before the move has ``frozen:`` 0 or 1, not a count, so unfreezing one that was frozen
  makes its ``frozen:`` unknown, and its F flag comes from the scripts' constant start sites
  (``Game.resistant``: an assumption the path records; a unit started both ways is unknown; a room's
  local script is keyed by its room resource, since a local number names the local of the room in
  use). The
  engine's limit of 15 nested runs (kMaxScriptNesting, script.h:116, script.cpp:351-352) is an
  error, counted, that ends the path ("nesting"); a start of a unit already on the stack is counted
  (the recursion guard). The engine's other error() cases end the path as "error", counted, and
  the predictions leave such paths out (``feasible``): stopping a slot with an open cutscene or
  override (script.cpp:273-275, 307-309), a global or local slot ending with one (stopObjectCode,
  script.cpp:879-880), a local script the room lacks (script.cpp:80-81), a room change from a room,
  flobject or local slot with an open cutscene (room.cpp:61-62, 67-68), loadRoomWithEgo's object not
  in the new room (room.cpp:225-227), endCutscene with no cutscene open (script.cpp:1681) and a
  division by a known 0 (script_v5.cpp:994, 1179). Its warning() cases are counted only: an object
  nowhere at runObjectScript (script.cpp:122-125) and a room change killing a slot with an open
  cutscene (script.cpp:1089-1101);
* ``room_change``: a room change runs what startScene does (room.cpp:42-260), with o5_loadRoom,
  o5_loadRoomWithEgo (script_v5.cpp:1812-1902) and setCameraFollows (camera.cpp:62-86) around it:
  stopTalk (VAR_TALK_ACTOR to 255 when an actor talks, actor.cpp:3573-3582); a running room, flobject
  or local slot no longer resumes (room.cpp:58-72: an open cutscene there is an error(), the path ends);
  VAR_NEW_ROOM; runExitScript (VAR_EXIT_SCRIPT, the old room's EXCD as a room slot, VAR_EXIT_SCRIPT2,
  script.cpp:989-1038); killScriptsAndResources (every room, flobject and local slot dies, inventory
  and global slots survive, script.cpp:1083-1105; an open cutscene there is a warning, counted); the
  room, VAR_ROOM, VAR_ROOM_RESOURCE (a pseudo room through ``Game.pseudo``, room.cpp:149-172),
  VAR_ROOM_FLAG = 1 only when the room resource was not in memory (resource.cpp:640-644; the cache is
  not a fact, so its value is unknown), the camera bounds (room.cpp:202-205, the room's width from
  RMHD); then, unless the room or its resource is 0 (room.cpp:179-182, 218), runEntryScript
  (VAR_ENTRY_SCRIPT, the new room's ENCD, VAR_ENTRY_SCRIPT2, script.cpp:1040-1069). Each script runs
  nested to its first yield as in ``calls``. loadRoomWithEgo sets VAR_WALKTO_OBJ to the object for
  the change and 0 after it, checks that the object is in the new room (room.cpp:222-226, error
  counted), and setCameraFollows runs the inventory hook; actor positions are not facts. A slot kind
  of an object script live before the move is taken from its owner now (room or not), recorded as
  an assumption. Without this feature a room change sets ``room``, ends a room-killed running slot
  and runs only the hooks of the opcode, as row 0;
* ``settle`` (row 3): after the action's immediate part (the click's frame), time passes until the move's
  effects have played out. The engine runs every live slot whose status is ssRunning (not frozen, not
  paused) once per frame, in slot order, each from where it stopped to its next yield (runAllScripts,
  script.cpp:968-987; a slot that already executed in the frame, because it was started nested in it, does
  not run again: didexec, script.cpp:506-507, 972, 979), then checkAndRunSentenceScript (scumm.cpp:
  3231-3255). The model does the same in **rounds** (``_next_round``, ``_eng_turn``), with these rules:

  - *The click's frame* (round 0). runAllScripts comes before checkExecVerbs and the sentence (scumm.cpp:
    3231-3232, 3255), so with settle the click's frame first runs the turn of every slot live at pre, by
    the rules below, in the order of their keys, except that a time-only wait (a WaitFor op, a delay, a
    time-only loop) the slot was in at pre is taken to still hold there, one frame after pre (its turn would
    change nothing; counted as a skip): time resolves it from the next frame on, after the sentence. The
    slots that wait for nothing (below), an ambient loop's pass and a poll whose facts the click changed
    (the mouse) run there. The sentence slot's own turn keeps rows 1-2's rules
    (``_eng_resume``: a WaitFor op forks, a delay forks on its timer), and so does the sentence after them.
    The click moved the mouse: processInput (scumm.cpp:3177) and updateScummVars (3198, 3385-3388) run
    before runAllScripts, so the mouse variables (MOUSE_VARS) are unknown from the click's frame on (R_INPUT).
    A wait has no click: its mouse variables keep pre's values (an assumption: no mouse move during '.').
    "Before the first yield" (``Path.immediate``) is the state at the end of the click's frame.
  - *Which slots run.* (a) Every slot the path started or the action resumed, live and not frozen: the
    click's sentence script and what it started, the sentence slot resumed in the click's frame, and any
    slot settle itself starts (reading the task's "started by the action" as "started on the path"; a wait
    starts with none). Such a slot resumes from the Frame it left (``Path.slots``), with the locals it has.
    (b) A slot live at pre whose wait point is a time-only wait: a WaitFor op for an actor, a message or the
    camera; a delay; a breakHere in a loop whose exit tests read only non-facts (``Loop``: positions, random
    draws, actor state, timers, engine variables that change with time (TIME_VARS), script locals, a counter
    the loop itself steps). Two more kinds run under (b), counted apart, because the engine runs every live,
    unfrozen slot each frame (script.cpp:978-979) and they wait for nothing: a breakHere outside any loop,
    and a slot parked right after an opcode that ran a script nested (the task's list does not name them).
    (c) A slot live at pre at a breakHere in a loop whose exit test reads facts (polls them) runs once the
    path has changed one of the facts it reads (WaitForSentence: the sentence script's slot fact). A delay
    inside a polling loop is (b): the fact test still decides. Rules (b) and (c) gate a pre slot's first
    turn only, at its pre wait point: once it has run, it runs every frame like any live, unfrozen slot
    (script.cpp:978-979), and a poll it meets later is tested on the path's facts. A slot live at pre runs
    with unknown locals and R flag (locals across a yield are not facts) and its F flag from the start
    sites; a frozen slot does not run until unfrozen. Every slot a round passes over is kept with its origin
    and why (``_skip``, ``Path.skips``: dead slots aside, frozen, liveness, freeze count or position not
    known, the rule at its pre wait point, the ambient cap), and counted per move.
  - *What an exit test reads* (``Game._test_reads``) is what its value depends on: a variable or local the
    loop body writes stands for what those writes read; a fact getter reads its facts; a getter of a
    position, a draw or actor state is a non-fact whatever its arguments are (getDist of the ego is a
    position test, so such ego-position triggers count as time-only: the rule as given). A variable the body
    writes only with constants (a reset) so reads nothing; switch ``const_polls`` (OFF) reads it as a fact.
  - *Time-only waits resolve with time* (assumptions, counted by kind): a WaitFor op met in a round
    blocks and falls through when its slot resumes in a later round (R_WAIT_OVER); a delay is over in
    the next round (R_WAIT_OVER); at the exit test of a time-only loop the slot waited in (its yield held
    the slot before this turn), a test whose value is not known is taken to exit, the side leaving the loop
    (R_LOOP_EXIT, with what the test read; ``_jump``), while a known value (a counter the path set, a fact)
    decides as usual; once per turn: a loop entered afresh in a turn, or met again after an outer loop came
    round, is tested as usual, since no time passed in it. An exit test on the camera position
    (VAR_CAMERA_POS_X) is not taken to exit: the camera moves toward a followed actor or a pan's target
    (moveCamera, camera.cpp:93-163), which time may or may not bring past the test, so the first such test
    of a slot's loop forks, and on the side that stays the loop stays for the rest of settle (R_CAMERA). A
    fork on an unknown condition that is not such a loop exit still forks. A loop with no exit at all
    (ambient) runs at most once per slot (``Path.ambient_done``), the cap; since the engine runs it every
    frame, when settle ends what its pass wrote, and the at: keys of a capped slot still live, become
    unknown with the values seen (``_close_settle``). The immediate part's sentence keeps rows 1-2's forks.
  - *When settle ends.* A path ends "settled" when a whole round changes no fact on it (a fixed point; one
    where a kept slot's locals still moved, a counter loop, is told apart and counted);
    "cycle" when a round ends in a state (writes, slot positions, locals, freeze counts, queue) met at
    an earlier round's start, the facts that vary around the cycle made unknown; "round limit" at
    ROUND_LIMIT rounds, the facts its last round changed made unknown (R_SETTLE, counted per move).
  - *Order inside a round.* The engine's slot order is not a fact: the model takes the slots live at
    pre first, in the order of their keys, then the ones the path started, in the order they started
    (``_round_order``), an approximation. Two slots run (or passed over) in one round where one writes a fact
    the other read or wrote are an order conflict, counted (``_round_conflicts``), not bounded.
  - *Two instances.* A recursive start leaves a live instance running (script.cpp:65-66), but the path
    keeps one Frame and one set of at: keys per unit name: such units are counted, and their at: keys (and
    a slot fact the tracked instance cleared) are unknown when settle ends (``_close_settle``).
  - *The sentence queue.* doSentence pushes (verb, A, B) (script.cpp:1145-1172; STOP empties it,
    script_v5.cpp:1005), freezeScripts and unfreezeScripts move every queued sentence's count
    (script.cpp:934-935, 962-963), and each round ends with checkAndRunSentenceScript on it
    (``_eng_sentences``). The click's sentence that had to wait (a live sentence slot) stays queued
    instead of ending the path "deferred". The queue at pre is taken as empty (not a fact).
  - *Engine variables that change as frames pass* (TIME_VARS: the camera position, the message state,
    the last sound, the talking actor) are unknown in the rounds after the click's frame unless the path
    wrote them; in the click's frame they keep pre's values, as rows 1-2 read them.
  - Room changes, calls, freezes and the engine's errors work as in rows 1-2 inside a round; a freeze takes
    a slot's F flag from its kept Frame when the path ran it. Paths
    **merge** at a round's start when their slot positions are the same (every slot, wait point,
    freeze and esc fact, the room, where each kept Frame resumes, the queue, the ambient loops run, the
    camera exits forked):
    when their stores are also the same this is the same state; otherwise the facts that differ, and
    the kept Frames' locals, become value sets, rows 1-2's join rule applied at the frame boundary (it
    loses links between facts, never makes a wrong value certain). Both are counted (``Run``).
    STEP_LIMIT counts the instructions of one frame. A wait ('.') has no immediate part: it is settle
    from pre (``run_wait``);

* ``dialog`` (row 4, ``run_verb``): a dialog line is a verb (``actions.dialog_line``), so the action is a left
  click on that verb, and the model does what the engine does with it. The click's frame (scumm.cpp:3177-3255):
  processInput and updateScummVars put the click in the mouse variables (unknown from the click on, R_INPUT, as
  settle's click); runAllScripts (3231) gives every slot live at pre its turn by settle's rules (round 0's rules:
  a time-only wait held at pre still holds; a poll runs once the path changed what it polls, which nothing has
  yet); checkExecVerbs (3232) takes the click: with _userPut above 0 and the verb found under the mouse
  (verbs.cpp:586-692; the usability check, ``actions.dialog_line``) it calls runInputScript(kVerbClickArea,
  verbid, 1) (verbs.cpp:666, 683-686), which runs the input script VAR_VERB_SCRIPT by runScript(N, 0, 0, [1, V,
  1, 0, ...]) (script.cpp:1478-1511; ``_eng_input``): global, or the local script N of the room in use, nested
  like any start in calls, from its first instruction, with no caller to resume, and with rows 1-2's rules (a
  WaitFor op forks), as the click's sentence. Whatever it does (writes, doSentence, starts, stops) follows rows
  1-3's rules; a sentence it queues is run by checkAndRunSentenceScript at the end of the frame (3255,
  ``_eng_sentences``). Then settle runs: the input script, if it yielded, is a slot the path started (rule a), and
  a slot live at pre that polls a fact the input script wrote (the script waiting on the menu) resumes under rule
  c from the next round on. Nothing names the dialog script, the variable it polls or the verbs of the lines:
  the model finds them by running the scripts. What the input script reads that the facts do not give (the mouse,
  the object under it) is unknown, and the path forks or writes unknown, counted as rows 1-3 count them. N is the
  input script installed at pre; the engine reads VAR_VERB_SCRIPT, _userPut and the verb at checkExecVerbs,
  after the frame's turns, so a path whose turns changed one of them before the click keeps it in
  ``Path.input_drift`` (counted); switch ``input_after_turns`` (OFF) reads them there instead (one decision with
  esc's ``esc_key_at_pre``: Switches, below);

* ``esc`` (row 5, ``run_esc``): a skipped cutscene is the Esc key (``actions.skip_cutscene``), and the model does
  what the engine does with that key in its frame (scumm.cpp:3177-3255). ScummEngine_v5 has no processKeyboard of
  its own (scumm_v5.h:29; only v2, v3, v6, v7 and v8 override it), so processInput (input.cpp:409-537) hands the key
  to ScummEngine::processKeyboard (input.cpp:961-1572). The run's settings keep the original GUI's key handling off
  (isUsingOriginalGUI, scumm.cpp:4271-4282; original_gui=false), and the branches before Esc's are other keys
  (F5, F8, space, '.': input.cpp:1400-1421). Esc's branch (input.cpp:1423-1429) needs VAR_CUTSCENEEXIT_KEY non-zero
  (input.cpp:966) and calls abortCutscene (script.cpp:1689-1709, ``_eng_abort``), which works on the top level of the
  cutscene stack only (idx = vm.cutSceneStackPointer, which may be 0: an override needs no cutscene) and only when an
  override is armed there (``if (offs)``, the ``esc:<unit>:W`` fact; with none it does nothing, and the action is
  not usable, ``actions.py``). Then the slot that armed it (vm.cutSceneScript[idx]) stands at W, the jump after its
  beginOverride (``ss->offs = offs``); it is ssRunning, so a slot a delay paused runs again (decreaseScriptDelay
  counts down only ssPaused slots, script.cpp:1513-1530) and one blocked at a WaitFor op has left it; its freeze count
  is 0; its cutsceneOverride goes down by one when above 0 (for a slot live at pre the count is not a fact in
  facts.py, though the bridge's dump holds it per slot; adding it is held, so the machine keeps it at 0, as for every
  such slot: a note); VAR_OVERRIDE = 1 and the level's override is cleared
  (``vm.cutScenePtr[idx] = 0``). The cutscene depth does not change: the code after the jump ends the cutscene
  itself. Then _mouseAndKeyboardStat = VAR(VAR_CUTSCENEEXIT_KEY) (input.cpp:1427-1428), the key code, read at
  pre; a code of Enter or Tab while the cursor shows becomes a click (input.cpp:1566-1570: not modelled, a note).
  Esc touches no talk (the '.' branch, input.cpp:1418-1421, is apart; abortCutscene's stopTextToSpeech is text to
  speech, script.cpp:1697, no fact) and writes no key variable (VAR_KEYPRESS is written only by the v2
  processKeyboard, input.cpp:931-936, and the message box, string.cpp:164-169). A key moves no mouse: the mouse
  variables keep pre's values, as for a wait (an assumption: no mouse move). runAllScripts (scumm.cpp:3231) gives
  every slot live at pre its turn by round 0's rules (as in a click's frame), except the rewound slot, which runs
  from W by rule a, with rows 1-2's rules as the click's sentence does (``_eng_override``): the jump takes it to the
  code after the cutscene body, which reads VAR_OVERRIDE = 1. checkExecVerbs (scumm.cpp:3232, verbs.cpp:586-664)
  takes the key (``_eng_key``): nothing once a doSentence(STOP) in the frame's turns ran clearClickedStatus, which
  wipes the frame's key (script_v5.cpp:1004-1008, input.cpp:378-386; ``Path.cleared``), nor while _userPut is 0 or
  below (590), which the engine reads there, after the
  frame's turns (the override's code often turns input back on), so the machine reads VAR_USERPUT, its mirror
  (script_v5.cpp:935-938), from the path's store; a shown verb whose key is the code would take it as a click on
  that verb (608-615; a verb's key is not a fact: ``actions.VERB_KEY``, unchecked); else runInputScript(kKeyClickArea,
  code, 1) (664; script.cpp:1478-1511) runs VAR_VERB_SCRIPT, also read there, as dialog's click runs it. Switch
  ``esc_key_at_pre`` (OFF) reads VAR_USERPUT and VAR_VERB_SCRIPT at pre instead, as dialog's click does by default.
  Then the sentence queue (scumm.cpp:3255) and settle. Each way the key goes on a path is kept with the forks the
  path took before it (``Run.key_routes``), so that check.py can count the ways that rest on a fork on a non-fact:
  a cutscene's data (vm.cutSceneData, set by beginCutscene and handed to the end hook, script.cpp:1640, 1658: not a
  fact for a level opened before the move, R_CUTDATA), which the end hook may test before it stops the sentence
  script, or an actor's room (setCameraFollows' room change, camera.cpp:69-70). With VAR_CUTSCENEEXIT_KEY not
  known, whether Esc reaches abortCutscene is not known: the action's usability is unknown (``actions.py``) and
  abortCutscene does not run here;

**Switches** (``SWITCHES``, ``Machine(switches=...)``): a rule beyond the spec, OFF by default, measured on its own
(check.py ``--switches``) so that it can be decided; rows are measured with every switch off. ``input_after_turns``
(dialog, above); ``const_polls`` (settle): a loop that resets a variable or bit to a constant and then, across a
yield, tests it to leave, polls it (a fact), not time (``Game._test_reads``). The loop classes are cached per
setting on the shared ``Game``. ``esc_key_at_pre`` (esc, above). ``input_after_turns`` and ``esc_key_at_pre`` are
one engine read with opposite defaults: checkExecVerbs reads _userPut and VAR_VERB_SCRIPT after the frame's
runAllScripts (scumm.cpp:3231-3232, verbs.cpp:590, script.cpp:1482), for a click on a verb and for a key alike. The
defaults follow the two features' specs: dialog's names the input script installed at pre, esc's asks for what the
engine does with the key in its frame. One decision settles both.

``non_fact_vars`` is the machine's own rule for variables that are not facts: a variable whose every
write in the scripts is unknown by the rules above. Nothing here names a game's object, script,
room, verb or variable; the numbers below are ScummVM engine constants, each cited.
"""

import collections
import dataclasses
import functools
import json
from dataclasses import dataclass, field

import facts as F
import ir
from ir import (Assign, BinOp, Bit, Cmp, Concat, Const, Embed, Getter, Index, Jump, List, Local, Neg, Not, Op,
                Start, Str, Truth, Var, show)

# --- engine constants (ScummVM) -----------------------------------------------------------------
VAR_RESULT = 0  # o5_expression case 6 runs an opcode and reads _scummVars[0] (script_v5.cpp:1180-1184)
VAR_KEYPRESS = 0  # vars.cpp:37: the same variable as VAR_RESULT
VAR_EGO = 1  # vars.cpp:38
VAR_ROOM = 4  # vars.cpp:41
VAR_OVERRIDE = 5  # vars.cpp:42
VAR_CAMERA_MIN_X, VAR_CAMERA_MAX_X = 17, 18  # vars.cpp:53-54
VAR_ROOM_RESOURCE = 22  # vars.cpp:58
VAR_TALK_ACTOR = 25  # vars.cpp:61
VAR_SCROLL_SCRIPT = 27  # vars.cpp:89
VAR_ENTRY_SCRIPT, VAR_ENTRY_SCRIPT2, VAR_EXIT_SCRIPT, VAR_EXIT_SCRIPT2 = 28, 29, 30, 31  # vars.cpp:63-66
VAR_SENTENCE_SCRIPT = 33  # vars.cpp:68
VAR_INVENTORY_SCRIPT = 34  # vars.cpp:69
VAR_CUTSCENE_START_SCRIPT, VAR_CUTSCENE_END_SCRIPT = 35, 36  # vars.cpp:70-71
VAR_VERB_SCRIPT = 32  # vars.cpp:67: the input script, which dispatch does not run (it queues the sentence); dialog
# runs it for a click on a verb (``Machine.run_verb``)
VERB_CLICK_AREA = 1  # kVerbClickArea (verbs.h:35): runInputScript's first argument for a click on a verb
LEFT_CLICK = 1  # checkExecVerbs' button code for a left click in v4 and later (verbs.cpp:666): the third argument
VAR_CUTSCENEEXIT_KEY = 24  # vars.cpp:60: Esc reaches abortCutscene only while it is not 0 (input.cpp:966, 1423), and
# its value is the key code checkExecVerbs then takes (input.cpp:1427-1428)
KEY_CLICK_AREA = 4  # kKeyClickArea (verbs.h:38): runInputScript's first argument for a key (verbs.cpp:664)
MBS_MAX_KEY = 0x200  # scumm.h:161: a _mouseAndKeyboardStat below it is a key (verbs.cpp:593)
KEYCODE_RETURN, KEYCODE_TAB = 13, 9  # Common::KEYCODE_RETURN and KEYCODE_TAB (common/keyboard.h:54, 52): a key code
# processKeyboard turns into a left or right click while the cursor shows (input.cpp:1566-1570)
KEYCODE_ESCAPE = 27  # Common::KEYCODE_ESCAPE (common/keyboard.h:56): the key Esc's branch tests (input.cpp:1423),
# and the code checkExecVerbs gets when that branch is not taken, the key's ASCII value (input.cpp:1553-1558)
VAR_WALKTO_OBJ = 38  # vars.cpp:73
VAR_CURSORSTATE, VAR_USERPUT = 52, 53  # vars.cpp:93-94
VAR_ROOM_FLAG, VAR_NEW_ROOM = 70, 72  # vars.cpp:107, 109
HOOK_VARS = range(27, 37)  # the script numbers the engine starts on its own, VAR_SCROLL_SCRIPT..VAR_CUTSCENE_END_SCRIPT
MAX_NESTING = 15  # kMaxScriptNesting (script.h:116): runScriptNested's error past it (script.cpp:351-352)
OF_OWNER_ROOM = 0x0F  # scumm.cpp:1745 (versions below 7)
MESSAGE_BOX_ACTOR = 252  # print to actor 252 uses text slot 3, the message box (script_v5.cpp:3448-3449)
CLASS_UNTOUCHABLE = 32  # kObjectClassUntouchable, object.h:54
NUM_ACTORS = 13  # _numActors for v5 games other than SAMNMAX (scumm.cpp:1726-1739)
SCREEN_WIDTH = 320  # _screenWidth (scumm.cpp:395)
ROOM_KILLED = ("entry", "exit", "local")  # slot kinds startScene kills (room.cpp:58-72, script.cpp:1083-1105)

FEATURES = ("own_entry", "dispatch", "calls", "room_change", "settle", "dialog", "esc")
ROWS = (("own_entry",), ("dispatch", "calls"), ("room_change",), ("settle",), ("dialog",), ("esc",))  # the growth
# table's rows, cumulative
ROW0 = frozenset({"own_entry"})
ROW3 = frozenset({"own_entry", "dispatch", "calls", "room_change", "settle"})
ROW4 = ROW3 | {"dialog"}
ROW5 = ROW4 | {"esc"}
IMPLEMENTED = frozenset(FEATURES)
# Switches: a rule beyond the spec, OFF by default, each measured on its own (check.py --switches) so it can be decided
SWITCHES = {
    "input_after_turns": "dialog: the click reads VAR_VERB_SCRIPT and VAR_USERPUT (_userPut) from the path's store "
                         "after the click frame's turns, where checkExecVerbs reads them (scumm.cpp:3231-3232), not "
                         "at pre (the spec); a path whose turns turned input off drops the click (verbs.cpp:590). "
                         "One decision with esc_key_at_pre, the same read with the other default",
    "const_polls": "settle: a loop exit test that reads a variable or bit the loop body writes only with constants "
                   "(a reset, Var[N] = 0) reads it as if the body did not write it, a fact the loop polls, not as "
                   "nothing (the spec: a written variable stands for what its writes read); the loop's own writes "
                   "cannot change such a test from one pass to the next, only another script's write while it "
                   "yields can (``Game._test_reads``)",
    "esc_key_at_pre": "esc: checkExecVerbs takes the Esc key code reading _userPut (VAR_USERPUT) and VAR_VERB_SCRIPT at "
                      "pre, as dialog's click does by default, not from the path's store after the Esc frame's turns, "
                      "where the engine reads them (scumm.cpp:3231-3232, the spec: what the engine does with the key). "
                      "One decision with input_after_turns, the same read with the other default",
}
PATH_LIMIT = 256  # paths per run, less those left out of the predictions; a fork past it ends the path with "limit"
STEP_LIMIT = 5000  # instructions per path
ROUND_LIMIT = 400  # settle rounds per path; a path still changing facts there ends with "round limit"
# Engine variables the engine itself changes as frames pass, outside opcodes (ScummVM v5 indices): in settle
# rounds their value is the time's, not pre's (``Machine.read``), and a loop test on them is a time test, except
# the camera position's (R_CAMERA, ``Game._test_reads``).
VAR_CAMERA_POS_X = 2  # vars.cpp:39
TIME_VARS = {VAR_CAMERA_POS_X: "VAR_CAMERA_POS_X: scumm.cpp:3379 every frame (scummLoop_updateScummVars)",
             3: "VAR_HAVE_MSG: scumm.cpp:3382 every frame",
             23: "VAR_LAST_SOUND: actor walk sounds every frame (scumm.cpp:3312, actor.cpp:2267, sound.cpp:121-122)",
             25: "VAR_TALK_ACTOR: stopTalk on the message timer (string.cpp:1125-1128, actor.cpp:3564-3592)"}
TIME_KEYS = frozenset(f"var:{n}" for n in TIME_VARS)
# The mouse variables (VAR_VIRT_MOUSE_X/Y, VAR_MOUSE_X/Y: vars.cpp:56-57, 77-78): the engine copies the mouse
# into them at the start of every frame (scummLoop_updateScummVars, scumm.cpp:3198, 3385-3388), after
# processInput (scumm.cpp:3177) and before runAllScripts (scumm.cpp:3231). A click moved the mouse, so with
# settle a sentence or walk makes them unknown from the click's frame on (``run_sentence``); a wait ('.', a key)
# keeps pre's values, an assumption (the player does not move the mouse during it).
MOUSE_VARS = {20: "VAR_VIRT_MOUSE_X", 21: "VAR_VIRT_MOUSE_Y", 44: "VAR_MOUSE_X", 45: "VAR_MOUSE_Y"}

# Unknown reasons
R_RANDOM, R_POSITION, R_ACTOR, R_TIMER = "random draw", "position", "actor state", "timer"
R_LOCAL, R_HELD, R_WORK = "local", "not held", "cannot work out"
R_SOUND, R_STRING, R_INVENTORY, R_VERB = "sound", "string", "inventory order", "verb slot"
R_MESSAGE, R_SENTENCE, R_KEY = "message state", "sentence queue", "key press"
R_FREEZE, R_CUTDATA, R_CACHE = "freeze count", "cutscene data", "resource cache"
R_BRANCH = "branch"  # a value that depends on which side of an earlier fork ran (paths merged, ``Machine._merge``)
R_LOOP_EXIT = "loop exit by time"  # settle: a time-only loop is taken to exit (``Machine._jump``)
R_WAIT_OVER = "wait over by time"  # settle: a WaitFor op falls through in a later round (``Machine._eng_turn``)
R_SETTLE = "not settled"  # settle: a fact still changing at ROUND_LIMIT, or one that varies around a cycle, or
# one a slot settle stopped running may still change (an ambient loop run once, a second instance: ``_close_settle``)
R_CAMERA = "camera position"  # settle: a loop exit on VAR_CAMERA_POS_X forks once (``Machine._jump``)
R_INPUT = "mouse"  # settle: the mouse variables after a click (MOUSE_VARS)
ROUND = "runAllScripts"  # the head of a settle round's Call
# facts a settle merge at a round's start keeps exact: the room and every slot fact (``Machine._position_key``)
POSITION_KEYS = frozenset({"room", f"var:{VAR_ROOM_RESOURCE}", "cutscene"})
POSITION_PREFIXES = ("script:", "local:", "objscript:", "at:", "frozen:", "esc:")


def row_of(features) -> int | None:
    """The growth-table row of a feature set: the number of ``ROWS`` it holds in full, in order,
    when it holds nothing else; None for any other set."""
    features, have, n = set(features), set(), 0
    for k, group in enumerate(ROWS):
        if not set(group) <= features:
            break
        have |= set(group)
        n = k
    return n if have == features else None

# o5_wait's blocking test per sub-op (script_v5.cpp:3281-3305): what it reads, which no fact holds
WAITS = {"WaitForActor": (R_ACTOR, "whether the actor is moving"),
         "WaitForMessage": (R_MESSAGE, "whether a message shows"),
         "WaitForCamera": (R_POSITION, "whether the camera has reached its target"),
         "WaitForSentence": (R_SENTENCE, "whether a sentence is queued or the sentence script runs")}


@dataclass(frozen=True)
class Unknown:
    reason: str
    what: str = ""
    values: frozenset | None = None  # the values it can take, when the machine knows them (``one_of``)

    def __str__(self) -> str:
        return f"?({self.reason}{': ' + self.what if self.what else ''}" + \
            (f" in {sorted(self.values)}" if self.values is not None else "") + ")"


MAX_VALUES = 64  # a value set larger than this becomes a plain unknown, counted in VALUE_CAPS
VALUE_CAPS = collections.Counter()  # MAX_VALUES hits by site in the current run (Machine._drive keeps them in Run)


def one_of(values, reason: str, what: str = ""):
    """A value known to be one of ``values``: the value itself when there is one, else an unknown that
    carries them (none past MAX_VALUES, counted)."""
    values = frozenset(values)
    if len(values) == 1:
        return next(iter(values))
    if len(values) > MAX_VALUES:
        VALUE_CAPS["value set"] += 1
        return Unknown(reason, what)
    return Unknown(reason, what, values)


def values_of(v):
    """The values ``v`` can take: {v} when known, an unknown's set, or None."""
    return v.values if isinstance(v, Unknown) else frozenset({v})


def _join_values(vals: list):
    """The value of a fact after merging paths that hold ``vals``: the one value when all agree, else
    the set of them (a plain unknown when one of them is one)."""
    first = vals[0]
    if all(v == first for v in vals[1:]):
        return first
    plain = next((v for v in vals if isinstance(v, Unknown) and v.values is None), None)
    if plain is not None:
        return plain
    reason = next((v.reason for v in vals if isinstance(v, Unknown)), R_BRANCH)
    return one_of(frozenset().union(*(values_of(v) for v in vals)), reason, "merged paths")


CMP = {"==": lambda a, b: a == b, "!=": lambda a, b: a != b, "<": lambda a, b: a < b, "<=": lambda a, b: a <= b,
       ">": lambda a, b: a > b, ">=": lambda a, b: a >= b}


def _decide(test, *vals):
    """A test on values: True or False, also on value sets when every combination agrees; else the
    first unknown (a plain one first)."""
    plain = next((v for v in vals if isinstance(v, Unknown) and v.values is None), None)
    if plain is not None:
        return plain
    unknown = next((v for v in vals if isinstance(v, Unknown)), None)
    if unknown is None:
        return test(*vals)
    combos = [()]
    for v in vals:
        combos = [c + (y,) for c in combos for y in values_of(v)]
        if len(combos) > MAX_VALUES:
            VALUE_CAPS["test"] += 1
            return Unknown(unknown.reason, unknown.what)
    out = {test(*c) for c in combos}
    return out.pop() if len(out) == 1 else unknown


def i32(v: int) -> int:
    return ((v + 2**31) % 2**32) - 2**31


def i16(v: int) -> int:
    return ((v + 0x8000) & 0xFFFF) - 0x8000


def kind_of(key: str) -> str:
    return key.split(":", 1)[0]


def family(key: str) -> str:
    """A fact's family: its kind, with ``at:`` and ``esc:`` split by unit kind (at:script, at:local,
    at:obj, at:entry, at:exit), since an older dump gives wait points for global scripts only."""
    parts = key.split(":")
    return ":".join(parts[:2]) if parts[0] in ("at", "esc") else parts[0]


def _arith(op: str, a, b):
    """o5's arithmetic on int32 values (script_v5.cpp:691-779, 989-998, 1152-1194); division truncates
    toward zero as C++ does, and a division by zero is an engine error, so its value is unknown. On
    value sets it works value by value."""
    if isinstance(a, Unknown) and a.values is None:
        return a
    if isinstance(b, Unknown) and b.values is None:
        return b
    if isinstance(a, Unknown) or isinstance(b, Unknown):
        u = a if isinstance(a, Unknown) else b
        if len(values_of(a)) * len(values_of(b)) > MAX_VALUES:
            VALUE_CAPS["arithmetic"] += 1
            return Unknown(u.reason, u.what)
        out = {_arith(op, x, y) for x in values_of(a) for y in values_of(b)}
        if any(isinstance(r, Unknown) for r in out):
            return Unknown(R_WORK, "divide by zero among the values")
        return one_of(out, u.reason, u.what)
    if op == "+":
        return i32(a + b)
    if op == "-":
        return i32(a - b)
    if op == "*":
        return i32(a * b)
    if b == 0:
        return Unknown(R_WORK, "divide by zero")
    q = abs(a) // abs(b)
    return i32(q if (a >= 0) == (b >= 0) else -q)


# --- the oracle -------------------------------------------------------------------------------------

class Oracle:
    """What a path reads when its own store has no write for a key."""

    def get(self, key: str):
        raise NotImplementedError

    def keys(self, prefix: str) -> list[str] | None:
        """The keys with this prefix the oracle holds, or None when it cannot list them."""
        return None

    def tracks(self, kind: str) -> bool:
        return True


class FactOracle(Oracle):
    """A fact map as the oracle: an absent key is 0, the engine's clocks (facts.CLOCKS) are timers,
    and a key whose family (``family``) is outside ``kinds`` (when given) is not held."""

    def __init__(self, facts: dict[str, int], kinds: frozenset[str] | None = None):
        self.facts, self.kinds = facts, kinds
        self._keys, self._tracks, self._got = {}, {}, {}  # caches: the map does not change during a run

    def tracks(self, kind: str) -> bool:
        """Whether the map holds a family: held itself, as a kind of its families ("esc" holds when
        "esc:local" does) or under its kind ("esc:local" holds when "esc" does)."""
        if kind not in self._tracks:
            self._tracks[kind] = self.kinds is None or kind in self.kinds or any(
                k.startswith(kind + ":") or kind.startswith(k + ":") for k in self.kinds)
        return self._tracks[kind]

    def get(self, key: str):
        v = self._got.get(key, self)
        if v is not self:
            return v
        if not self.tracks(family(key)):
            v = Unknown(R_HELD, key)
        elif key.startswith("var:") and int(key[4:]) in F.CLOCKS:
            v = Unknown(R_TIMER, key)
        else:
            v = self.facts.get(key, 0)
        self._got[key] = v
        return v

    def owned(self) -> dict:
        """{owner: sorted objects} from the owner facts, worked out once (findInventory, getInventoryCount)."""
        if "_owned" not in self.__dict__:
            out = collections.defaultdict(list)
            for k in sorted(self.keys("owner:")):
                out[self.facts[k]].append(int(k[6:]))
            self.__dict__["_owned"] = dict(out)
        return self.__dict__["_owned"]

    def keys(self, prefix: str) -> list[str]:
        if prefix not in self._keys:
            self._keys[prefix] = [k for k in self.facts if k.startswith(prefix)]
        return self._keys[prefix]


# --- what the machine knows from the game files -------------------------------------------------------

@dataclass(frozen=True)
class Loop:
    """A loop of a unit that holds a yield (settle). It is the natural loop of a backward jump: the jump,
    its target (the head) and every instruction that reaches the jump without passing the head. An exit
    is a conditional jump in the body with a successor outside it. What the exit tests read decides the
    kind: "ambient" (no exit at all), "time" (every exit test reads only non-facts: a position, a random
    draw, actor state, a timer, an engine variable that changes with time, a script local, a counter the
    loop itself steps), or "facts" (some exit test reads a fact, listed in ``reads``). A time loop whose
    exit tests read the camera position has R_CAMERA in ``non_facts``: settle forks on it once instead of
    taking it to exit (``Machine._jump``)."""
    head: int
    body: frozenset
    exits: tuple  # (jump index, its successor outside the body)
    kind: str
    reads: frozenset  # fact keys, or prefixes ending in ":" for a key the code does not fix, the exit tests read
    non_facts: frozenset  # the reasons of the non-facts they read


@dataclass(frozen=True)
class WaitPoint:
    """Where a live slot stands (facts.py ``at:``), as settle classes it: "wait" (at a WaitFor op),
    "delay" (after a delay), "frame" (after a breakHere), "parked" (right after an opcode that ran a script
    nested), "start" (a script's or an entry's first instruction) or "other"; with the innermost loop
    (``Loop``) that holds its yield, if any."""
    kind: str
    op: str = ""  # the WaitFor op, or the opcode a parked slot ran
    loop: Loop | None = None


@dataclass
class Game:
    """The units, plus what the engine reads from the game data that the IR does not carry: each
    object's room and parent, the offset of its verb block (index.json), each room's width (RMHD),
    and the pseudo-room map that o5_pseudoRoom builds (read from the scripts)."""
    units: dict
    first_local: int  # local scripts are numbered from _numGlobalScripts (script.cpp:70, 78)
    local_rooms: dict  # local script number -> rooms whose resource has it
    object_room: dict  # object -> room resource it is defined in
    object_parent: dict
    verb_offset: dict  # object -> offset of its VERB block in its OBCD (getVerbEntrypoint's verboffs)
    room_width: dict
    pseudo: dict  # pseudo room (>= 0x80) -> room resource
    pseudo_conflicts: list
    resistance: dict = field(default_factory=dict)  # ("global", N) / ("local", room, N) -> F flags it is started with
    wild_starts: list = field(default_factory=list)  # (cite, F flag) of starts whose script number is not a constant

    @classmethod
    @functools.cache
    def load(cls) -> "Game":
        units = ir.load()
        meta = json.loads((ir.SCRIPTS / "index.json").read_text())
        local_rooms = collections.defaultdict(set)
        for k in units:
            if k[0] == "local":
                local_rooms[k[2]].add(k[1])
        widths = {}
        for r, room in meta["rooms"].items():
            path = ir.ROOT / room["lflf"] / "ROOM" / "RMHD"
            if path.exists():
                # after the 8-byte block header, RMHD holds width, height, objects (room.cpp:343)
                widths[int(r)] = int.from_bytes(path.read_bytes()[8:10], "little")
        pseudo, conflicts = {}, []
        for u in units.values():
            for x in u.ins:
                if isinstance(x.st, Op) and x.st.name == "PseudoRoom":
                    # descumm prints each mapped room as j & 0x7F (descumm.cpp:1045-1060); the engine maps
                    # j (>= 0x80) to the resource (script_v5.cpp:2085-2092)
                    res, *rooms = (a.value if isinstance(a, Const) else None for a in x.st.args)
                    for j in rooms:
                        if res is None or j is None:
                            conflicts.append((x.cite(), "not a constant"))
                        elif pseudo.get(j | 0x80, res) != res:
                            conflicts.append((x.cite(), f"room {j | 0x80}: {pseudo[j | 0x80]} and {res}"))
                        else:
                            pseudo[j | 0x80] = res
        objs = meta["objects"]
        g = cls(units=units, first_local=min(local_rooms) if local_rooms else 1 << 16,
                local_rooms=dict(local_rooms), object_room={int(o): v["room"] for o, v in objs.items()},
                object_parent={int(o): v.get("parent", 0) for o, v in objs.items()},
                verb_offset={int(o): v["verb_block_offset_in_obcd"] for o, v in objs.items()
                             if v.get("verb_block_offset_in_obcd") is not None},
                room_width=widths, pseudo=pseudo, pseudo_conflicts=conflicts)
        g.resistance, g.wild_starts = g._resistance()
        return g

    def _resistance(self) -> tuple[dict, list]:
        """The F (freeze resistant) flags each global script's slots, and each room's local script's
        slots, can have, from every place that starts it: startScript with a constant number gives its
        own F (script_v5.cpp:2971, opcode bit 0x20); chainScript passes on the chaining slot's F
        (script_v5.cpp:850), so a chain target gets the flags of its source (a fixpoint); the engine
        starts without F the hooks, whose numbers the scripts write into the hook variables
        (script.cpp:62, 906, 991, 1042, 1245, 1510, camera.cpp:54), objects (script_v5.cpp:2889) and
        room code (script.cpp:998, 1049). Keys are units: ("global", N), or ("local", room resource, N),
        since a local number is the local of the room in use (script.cpp:78-81): a start from a room's
        own code (local, entry, exit) reaches that room's local N, one from global or object code, or a
        hook, every room that has local N. Starts whose number is not a constant are listed, not spread
        over every script."""
        flags, wild, chains = collections.defaultdict(set), [], []
        for key, u in self.units.items():
            room = key[1] if key[0] in ("local", "entry", "exit") else None
            src = key if key[0] in ("global", "local") else None
            for x in u.ins:
                st = x.st
                if isinstance(st, Start) and st.name in ("startScript", "chainScript"):
                    if not isinstance(st.script, Const):
                        wild.append((x.cite(), st.freeze if st.name == "startScript" else None))
                    elif st.name == "startScript":
                        for dst in self._started(st.script.value, room):
                            flags[dst].add(st.freeze)
                    else:
                        chains += [(src, dst) for dst in self._started(st.script.value, room)]
                elif isinstance(st, Assign) and isinstance(st.lhs, Var) and st.lhs.n in HOOK_VARS \
                        and isinstance(st.rhs, Const) and st.rhs.value:
                    for dst in self._started(st.rhs.value, None):
                        flags[dst].add(False)
        changed = True
        while changed:
            changed = False
            for src, dst in chains:
                add = (flags[src] or {False}) if src else {False}
                if not add <= flags[dst]:
                    flags[dst] |= add
                    changed = True
        return dict(flags), wild

    def _started(self, n: int, room) -> list:
        """The units a start of script number ``n`` from code of ``room`` (None: global or object code,
        or a hook) can run: global N, or local N of that room, or of every room that has it."""
        if n < self.first_local:
            return [("global", n)]
        if room is not None:
            return [("local", room, n)]
        return [("local", r, n) for r in sorted(self.local_rooms.get(n, ()))]

    def resistant(self, key: str, res=None):
        """The F flag of a live slot of ``script:N`` / ``local:N`` that the path did not start, ``res``
        the room resource in use (a local slot is that room's): True or False when every constant start
        site agrees, None when they differ or the room is not known. A script no script starts is
        started by the engine, without F. Object and room-code slots never have it."""
        if key.startswith("script:"):
            unit = ("global", int(key[7:]))
        elif key.startswith("local:"):
            if res is None or isinstance(res, Unknown):
                return None
            unit = ("local", res, int(key[6:]))
        else:
            return False
        fl = self.resistance.get(unit, set())
        return None if len(fl) > 1 else (next(iter(fl)) if fl else False)

    def resource(self, room):
        """The room resource a room number loads (room.cpp:166-169): a pseudo room maps through
        _resourceMapper, any other is itself."""
        if isinstance(room, Unknown):
            return room
        if room >= 0x80:
            return self.pseudo.get(room, Unknown(R_WORK, f"pseudo room {room} has no mapping"))
        return room

    def script_key(self, n: int) -> str:
        """A script number's fact: global below _numGlobalScripts, else local (script.cpp:70-86)."""
        return f"local:{n}" if n >= self.first_local else f"script:{n}"

    def join_points(self, unit: tuple) -> frozenset:
        """The instructions of a unit that a jump goes to (ir.py's targets)."""
        cache = self.__dict__.setdefault("_joins", {})
        if unit not in cache:
            cache[unit] = frozenset(x.target for x in self.units[unit].ins if x.target is not None)
        return cache[unit]

    def entry(self, obj: int, verb: int) -> int | None:
        """The instruction index object ``obj`` runs for ``verb``: its event for the verb, else its
        first 0xFF event (script.cpp:242-248, ir.py's entries)."""
        u = self.units.get(("obj", obj))
        if u is None:
            return None
        return u.entries.get(verb, u.entries.get(ir.DEFAULT_VERB))

    # --- settle: loops and wait points, read from the code once --------------------------------------------

    def nonfacts(self) -> dict:
        """``non_fact_vars`` of these units, worked out once."""
        if "_nonfacts" not in self.__dict__:
            self.__dict__["_nonfacts"] = non_fact_vars(self)
        return self.__dict__["_nonfacts"]

    def successors(self, unit: tuple, i: int) -> list:
        """The instructions control can go to from instruction ``i``: a jump's target (and its next one
        when it has a condition), nothing after an op that always ends the script, else the next one; a
        WaitFor op also to itself (it re-runs while it blocks). The end of the code is not listed."""
        u = self.units[unit]
        x = u.ins[i]
        if isinstance(x.st, Jump):
            out = [x.target] if x.st.cond is None else [x.next, x.target]
        elif x.ends == "always":
            out = []
        else:
            out = [x.next]
        if x.yields == "wait":
            out.append(i)
        return [j for j in out if j is not None and j < len(u.ins)]

    def loops(self, unit: tuple, const_polls: bool = False) -> list:
        """The unit's loops that hold a yield (``Loop``), one per backward jump. ``const_polls``: switch const_polls
        (``_test_reads``); each setting has its own cache, since one Game serves every Machine."""
        cache = self.__dict__.setdefault("_loops", {})
        if (unit, const_polls) in cache:
            return cache[(unit, const_polls)]
        u = self.units[unit]
        n = len(u.ins)
        preds = collections.defaultdict(set)
        for i in range(n):
            for j in self.successors(unit, i):
                preds[j].add(i)
        out = []
        for i in range(n):
            x = u.ins[i]
            if not isinstance(x.st, Jump) or x.target is None or x.target > i:
                continue
            head, body, todo = x.target, {x.target, i}, [i]
            while todo:
                for q in preds[todo.pop()]:
                    if q not in body:
                        body.add(q)
                        todo.append(q)
            if not any(u.ins[k].yields for k in body):
                continue
            exits = []
            for k in sorted(body):
                y = u.ins[k]
                if isinstance(y.st, Jump) and y.st.cond is not None:
                    outside = [j for j in (y.next, y.target) if j is None or j >= n or j not in body]
                    if len(outside) == 1:
                        exits.append((k, n if outside[0] is None else outside[0]))
            reads, non = set(), set()
            for k, _ in exits:
                r, nf = self._test_reads(unit, frozenset(body), u.ins[k].st.cond, frozenset(), const_polls)
                reads |= r
                non |= nf
            kind = "ambient" if not exits else ("facts" if reads else "time")
            out.append(Loop(head, frozenset(body), tuple(exits), kind, frozenset(reads), frozenset(non)))
        cache[(unit, const_polls)] = out
        return out

    def const_poll_loops(self) -> list:
        """(unit, head) of every loop of these units that switch const_polls reclassifies: time-only without it,
        polling facts with it. For the log: the switch's reach over the IR."""
        if "_const_poll_loops" not in self.__dict__:
            out = []
            for unit in self.units:  # the same backward jumps in the same order, so the two lists pair up
                out += [(unit, on.head) for off, on in zip(self.loops(unit), self.loops(unit, True))
                        if on.kind != off.kind]
            self.__dict__["_const_poll_loops"] = out
        return self.__dict__["_const_poll_loops"]

    def _test_reads(self, unit: tuple, body: frozenset, e, seen: frozenset,
                    const_polls: bool = False) -> tuple[set, set]:
        """(facts, non-fact reasons) an exit test's value depends on. A variable, bit or local the loop
        body writes stands for what those writes read (a counter the loop steps, ``x op= constant``, is
        time); one it does not write is a fact, except a clock or an engine variable that changes with time
        (TIME_VARS), a variable ``non_fact_vars`` names, and a script local (a local across a yield is no
        fact). A fact getter reads its facts (an argument the code does not fix gives a key prefix); any other
        getter is a non-fact whatever its arguments read (getDist of the ego reads a position).

        By that rule a variable the body writes only with constants (``Var[N] = 0``, a reset) reads nothing, so
        a loop that resets the value it then waits on, across a yield, is time-only. Switch ``const_polls`` (OFF)
        reads such a variable or bit as if the body did not write it: the loop's own writes cannot change the
        test from one pass to the next, only another script's write while it yields can, so it is a fact the
        loop polls. A local stays as it is: no other script writes a slot's locals."""
        facts, non = set(), set()

        def target(kind, n):
            u = self.units[unit]
            writes = []
            for k in body:
                st = u.ins[k].st
                for y in _walk(st):
                    if isinstance(y, Embed):
                        y = y.st
                    if isinstance(y, Assign) and ((kind == "var" and y.lhs == Var(n)) or (kind == "bit" and y.lhs == Bit(n))
                                                  or (kind == "local" and y.lhs == Local(n))):
                        writes.append(y)
            return writes

        def ref(kind, n):
            if (kind, n) in seen:
                return
            writes = target(kind, n)
            if writes and const_polls and kind in ("var", "bit") and \
                    all(w.op == "=" and isinstance(w.rhs, Const) for w in writes):
                writes = []  # switch const_polls: read as if the body did not write it (docstring)
            if writes:
                for w in writes:
                    if w.op != "=" and isinstance(w.rhs, Const):
                        non.add("counter")
                    else:
                        r, nf = self._test_reads(unit, body, w.rhs, seen | {(kind, n)}, const_polls)
                        facts.update(r)
                        non.update(nf)
                        if w.op != "=":
                            non.add("counter")
                return
            if kind == "local":
                non.add(R_LOCAL)
            elif kind == "var" and n in F.CLOCKS:
                non.add(R_TIMER)
            elif kind == "var" and n == VAR_CAMERA_POS_X:
                non.add(R_CAMERA)
            elif kind == "var" and n in TIME_VARS:
                non.add(R_TIMER)
            elif kind == "var" and n in self.nonfacts():
                non.add(self.nonfacts()[n])
            else:
                facts.add(f"{kind}:{n}")

        def const(a):
            return a.value if isinstance(a, Const) else None

        def walk(e):
            match e:
                case Var(n):
                    ref("var", n)
                case Bit(n):
                    ref("bit", n)
                case Local(n):
                    ref("local", n)
                case Index(kind, _, off):
                    if kind == "local":
                        non.add(R_LOCAL)
                    else:
                        facts.add(f"{kind}:")
                    walk(off)
                case BinOp(_, a, b) | Cmp(_, a, b):
                    walk(a)
                    walk(b)
                case Neg(a) | Not(a) | Truth(a):
                    walk(a)
                case Embed(st):
                    walk(st.rhs)
                case Getter(name, args):
                    kind = GETTERS.get(name, (R_WORK,))[0]
                    if kind != "fact":
                        non.add(kind)
                        return
                    o = const(args[0]) if args else None
                    if name == "getObjectOwner":
                        facts.add(f"owner:{o}" if o is not None else "owner:")
                    elif name == "getObjectState":
                        facts.add(f"state:{o}" if o is not None else "state:")
                    elif name == "classOfIs":
                        cs = [const(c) for c in args[1].items] if len(args) > 1 and isinstance(args[1], List) else [None]
                        for c in cs:
                            facts.add(f"class:{o}:{c & 0x7F}" if o is not None and c is not None
                                      else (f"class:{o}:" if o is not None else "class:"))
                    elif name == "isScriptRunning":
                        facts.update([self.script_key(o)] if o is not None else ["script:", "local:"])
                    elif name == "getInventoryCount":
                        facts.add("owner:")
                    elif name == "getVerbEntryPoint":
                        facts.update([f"owner:{o}" if o is not None else "owner:", "room"])
                    else:
                        facts.add(f"{name}:")

        walk(e)
        return facts, non

    def wait_point(self, unit: tuple, idx: int, const_polls: bool = False) -> WaitPoint:
        """How the slot of ``unit`` standing at instruction ``idx`` waits (``WaitPoint``); ``const_polls`` as in
        ``loops``."""
        cache = self.__dict__.setdefault("_waits", {})
        if (unit, idx, const_polls) in cache:
            return cache[(unit, idx, const_polls)]
        u = self.units[unit]
        x = u.ins[idx] if idx < len(u.ins) else None
        prev = u.ins[idx - 1] if 0 < idx <= len(u.ins) and u.ins[idx - 1].next == idx else None

        def innermost(i):
            ls = [lp for lp in self.loops(unit, const_polls) if i in lp.body]
            return min(ls, key=lambda lp: len(lp.body)) if ls else None

        if x is not None and x.yields == "wait":
            wp = WaitPoint("wait", x.st.name, innermost(idx))
        elif prev is not None and prev.yields in ("frame", "delay"):
            wp = WaitPoint("frame" if prev.yields == "frame" else "delay", "", innermost(idx - 1))
        elif prev is not None and isinstance(prev.st, (Start, Op)):
            wp = WaitPoint("parked", prev.st.name)
        elif idx == 0 or idx in u.entries.values():
            wp = WaitPoint("start")
        else:
            wp = WaitPoint("other")
        cache[(unit, idx, const_polls)] = wp
        return wp

    def time_exits(self, unit: tuple, camera: bool = False, const_polls: bool = False) -> dict:
        """{jump index: (its successor outside the loop, the loop's head)} for every exit of the unit's
        time-only loops (the innermost such loop, when the jump leaves several). ``camera``: those of the
        time-only loops whose exit tests read the camera position (R_CAMERA), which settle forks on once
        instead of taking them to exit (``Machine._jump``); without it, the others. ``const_polls`` as in
        ``loops``."""
        cache = self.__dict__.setdefault("_time_exits", {})
        if (unit, camera, const_polls) not in cache:
            out = {}
            # inner loops last, so they win
            for lp in sorted(self.loops(unit, const_polls), key=lambda lp: -len(lp.body)):
                if lp.kind == "time" and (R_CAMERA in lp.non_facts) == camera:
                    out.update({j: (succ, lp.head) for j, succ in lp.exits})
            cache[(unit, camera, const_polls)] = out
        return cache[(unit, camera, const_polls)]

    def time_loops_at(self, unit: tuple, idx: int, const_polls: bool = False) -> tuple:
        """The heads of the time-only loops (not the camera's) that hold the yield a slot standing at ``idx``
        waited at (the WaitFor op itself, else the breakHere or delay before it); ``const_polls`` as in ``loops``."""
        wp = self.wait_point(unit, idx, const_polls)
        y = idx if wp.kind == "wait" else idx - 1
        return tuple(sorted(lp.head for lp in self.loops(unit, const_polls)
                            if lp.kind == "time" and R_CAMERA not in lp.non_facts and y in lp.body))


# --- paths -----------------------------------------------------------------------------------------------

@dataclass
class Frame:
    """A running script's slot: its unit, where it is, its locals and flags."""
    unit: tuple
    pc: int | None
    locals: dict = field(default_factory=dict)
    args_known: bool = False  # False: a local the path has not written is unknown (row 0)
    owned: object = False  # an object script started while its object was not the room's runs as an
    # inventory slot and survives a room change (object.cpp:355-367); True, False or Unknown
    recursive: bool | None = None  # the slot's R flag, which chainScript passes on (script_v5.cpp:834-851);
    # None: not known. An object slot never has it in v5 (script_v5.cpp:2889, script.cpp:1432)
    resistant: object = False  # the slot's F flag (freezeResistant), which chainScript also passes on
    sid: int = 0  # the slot's identity in the path (calls): a stop or a kill finds it by this
    freeze: int = 0  # the slot's freeze count (calls); runScript starts it at 0 (script.cpp:100, 147)
    killed: str | None = None  # why the slot died while its turn was not over: stopped, chained, room
    override: int = 0  # the slot's cutsceneOverride: cutscenes it began and has not ended (script.cpp:1634)
    nested: bool = True  # run by runScriptNested, so it counts toward the nesting limit; False for a slot
    # runAllScripts resumes, which runs it with executeScript directly (script.cpp:978-984)
    origin: str = "path"  # settle: "path" for a slot the path started or the action resumed (rule a), "pre" for
    # a slot live before the move that settle runs by rules b and c
    time_loops: tuple = ()  # settle: heads of the time-only loops it waited in before this turn; each is taken
    # to exit once (``Machine._jump``)


@dataclass
class Call:
    """Engine code between nested script runs (calls): an opcode such as startScript, pickupObject
    or loadRoom, a routine inside one (startScene), or checkAndRunSentenceScript. ``steps`` run in
    order; a step may start a script, which runs on top of the call until its turn ends. ``caller``
    is the slot that ran the opcode (None: the engine's own, as for the sentence script);
    ``active`` is ``_currentScript == caller``: false once a nested run returned to a caller that is
    dead or frozen (runScriptNested, script.cpp:377-390), or once the opcode itself ended it. At the
    end of the call an active caller goes on; one that is not has its turn end there."""
    head: str
    cite: str
    steps: tuple
    caller: int | None = None
    active: bool = True
    i: int = 0


@dataclass
class Path:
    stack: list  # frames and calls, the running one last (row 0: one frame); a Frame is put in a list
    writes: dict = field(default_factory=dict)  # fact -> int or Unknown
    assumptions: list = field(default_factory=list)  # (cite, condition, side taken, why unknown)
    unmodelled: list = field(default_factory=list)  # (statement kind, cite, why)
    notes: list = field(default_factory=list)  # (statement kind, cite, what is left out)
    unknown_writes: list = field(default_factory=list)  # (statement kind, cite, target)
    end: str | None = None  # how the first script's turn ended (yield, end, parked, stopped, chained,
    # room), or deferred, dropped, unmodelled, limit, nesting
    end_at: str | None = None
    steps: int = 0
    freeze: dict = field(default_factory=dict)  # unit name -> (count, exact) of slots off the stack whose
    # count the path set; exact False: at least that many (a slot frozen before the move)
    cut_index: int | None = None  # vm.cutSceneScriptIndex while beginCutscene runs its hook (a sid)
    cut_data: list = field(default_factory=list)  # cutSceneData of the levels this path opened, last on top
    turns: list = field(default_factory=list)  # (unit name, how) of every script turn that ended, in order
    errors: list = field(default_factory=list)  # (what, cite, detail): the engine's error() and warning()
    # cases, counted, not raised
    depth: int = 0  # most frames on the stack at once
    reentries: int = 0  # starts of a unit already on the stack
    next_sid: int = 1
    last: object = None  # the first frame, once its turn is over
    merged: int = 0  # joins this path came out of with other paths folded in (calls)
    joined: tuple | None = None  # the join it was last merged at, so it does not wait there again
    seen: set = field(default_factory=set)  # states met at jump targets in the running frame's turn (spin)
    resumed: bool = False  # just out of a merge: it goes on past the join it waited at
    # --- settle (``Machine._next_round``); a path that does not settle keeps these empty
    settle: bool = False  # the path goes on in rounds once its immediate part is over
    round: int = 0  # 0: the immediate part (the click's frame); n: the n-th settle round
    first_end: str | None = None  # how the immediate part ended (rows 1-2's end)
    immediate: dict | None = None  # the writes when the immediate part ended (None: not yet)
    after: set = field(default_factory=set)  # keys whose value moved after the immediate part on a path merged in
    slots: dict = field(default_factory=dict)  # unit name -> the Frame of a live slot off the stack, where it
    # resumes, with the locals it has (slots the path ran); in the order they became live
    queue: list = field(default_factory=list)  # the sentence queue: (verb, A, B, freeze count), last on top
    round_start: dict = field(default_factory=dict)  # the writes when this round began
    history: list = field(default_factory=list)  # (state hash, writes) at each round's start (cycles)
    round_ran: set = field(default_factory=set)  # unit names that executed in this round (didexec)
    ambient_done: set = field(default_factory=set)  # (unit name, loop head): an ambient loop that ran once
    owner: str | None = None  # the slot whose turn the round is running (order conflicts)
    owner_rule: str = ""  # the rule that slot runs by
    writer: dict = field(default_factory=dict)  # key -> the rule of the settle turn that last wrote it
    rw: dict = field(default_factory=dict)  # owner -> (keys read before it wrote them, keys written), this round
    conflicts: list = field(default_factory=list)  # (round, slot, slot, some keys, number of keys)
    ran_by: dict = field(default_factory=dict)  # unit name -> the rule it first ran by in settle
    not_run: dict = field(default_factory=dict)  # unit name live at pre -> why settle did not run it (last reason)
    skips: set = field(default_factory=set)  # (unit name, origin, why): every slot a round passed over, any round
    sentences: int = 0  # queued sentences settle ran
    steps0: int = 0  # ``steps`` when this round began: STEP_LIMIT counts the instructions of one frame
    timed: bool = False  # settle: the code running is a runAllScripts turn (settle's rules for waits and loop
    # exits), not the click's sentence or the sentence slot's turn before it (rows 1-2's rules)
    camera_done: set = field(default_factory=set)  # (unit name, loop head): a camera exit forked once, this side stays
    ambient_pass: set = field(default_factory=set)  # unit names whose one ambient pass ran in this round
    ambient_vals: dict = field(default_factory=dict)  # key -> values it had around an ambient pass (``_close_settle``)
    ambient_slots: set = field(default_factory=set)  # unit names whose ambient loop settle ran once and capped
    multi: set = field(default_factory=set)  # unit names a recursive start gave a second live instance
    closed: bool = False  # ``_close_settle`` ran
    round_slots: int = 0  # a hash of the kept slots, locals included, when this round began (``_next_round``)
    # --- dialog (``Machine._eng_input``)
    input_drift: tuple = ()  # (key, pre value, value in the store) the click's turns changed before checkExecVerbs
    # --- esc (``Machine._eng_key``)
    cleared: object = False  # a doSentence(STOP) ran in the frame before checkExecVerbs: clearClickedStatus wiped the
    # frame's key or click (script_v5.cpp:1004-1008, input.cpp:378-386); None when merged paths disagree

    def __post_init__(self):
        if isinstance(self.stack, Frame):
            self.stack = [self.stack]

    @property
    def frame(self) -> Frame:
        """The running (top) frame; once the stack is empty, the first frame."""
        for e in reversed(self.stack):
            if isinstance(e, Frame):
                return e
        return self.last

    def fork(self) -> "Path":
        stack = [dataclasses.replace(e, locals=dict(e.locals)) if isinstance(e, Frame) else dataclasses.replace(e)
                 for e in self.stack]
        # a Frame kept in ``slots`` is never changed in place (a turn runs a copy), so the dict is copied shallowly
        return dataclasses.replace(
            self, stack=stack, writes=dict(self.writes), assumptions=list(self.assumptions),
            unmodelled=list(self.unmodelled), notes=list(self.notes), unknown_writes=list(self.unknown_writes),
            freeze=dict(self.freeze), cut_data=list(self.cut_data), turns=list(self.turns), errors=list(self.errors),
            seen=set(self.seen), after=set(self.after), slots=dict(self.slots), queue=list(self.queue),
            history=list(self.history), round_ran=set(self.round_ran), ambient_done=set(self.ambient_done),
            rw={k: (set(r), set(w)) for k, (r, w) in self.rw.items()}, conflicts=list(self.conflicts),
            ran_by=dict(self.ran_by), not_run=dict(self.not_run), writer=dict(self.writer), skips=set(self.skips),
            camera_done=set(self.camera_done), ambient_pass=set(self.ambient_pass),
            ambient_vals={k: list(v) for k, v in self.ambient_vals.items()}, ambient_slots=set(self.ambient_slots),
            multi=set(self.multi))


@dataclass
class Run:
    paths: list
    start: str  # the first instruction's cite, or why nothing ran
    limit: int = 0  # forks ended by PATH_LIMIT
    unit: tuple | None = None
    merges: int = 0  # paths folded into another at a join (calls)
    value_caps: dict = field(default_factory=dict)  # value sets past MAX_VALUES made plain unknowns, by site
    settle_merges: int = 0  # of ``merges``, paths at a round's start with the same slot positions folded (settle)
    pre_slots: tuple = ()  # settle: the unit names of the slots live at pre
    value_merges: int = 0  # of ``settle_merges``, those whose stores differed (value sets); the rest were equal
    key_routes: tuple = ()  # esc: (where the key went, the assumptions the path held then) for every path that reached
    # checkExecVerbs in the Esc frame, as it reached it (``Machine._eng_key``); a fork the path limit undid is not kept


def lifecycle_key(unit: tuple, game: Game) -> str | None:
    """The fact of the unit's own slot: entry and exit code are not facts (facts.py)."""
    if unit[0] == "global":
        return f"script:{unit[1]}"
    if unit[0] == "local":
        return game.script_key(unit[2])
    if unit[0] == "obj":
        return f"objscript:{unit[1]}"
    return None


def unit_name(unit: tuple) -> str:
    """The unit as ``at:`` and ``esc:`` keys write it (facts.unit_of): script:N, local:N, obj:O,
    entry or exit."""
    if unit[0] == "global":
        return f"script:{unit[1]}"
    if unit[0] == "local":
        return f"local:{unit[2]}"
    if unit[0] == "obj":
        return f"obj:{unit[1]}"
    return unit[0]


def slot_name(key: str) -> str:
    """A slot fact's unit name: script:N and local:N stay, objscript:O is obj:O."""
    return "obj:" + key.split(":", 1)[1] if key.startswith("objscript:") else key


# --- the rules ---------------------------------------------------------------------------------------------
# HEADS: every statement kind (ir.head) -> (effect, cite, what). Effects: "control", "write", "yield",
# "wait" (a fork on engine state, WAITS), "end", "none" (no tracked effect), "deferred" (a later
# feature's), "unmodelled".

S5 = "script_v5.cpp"
HEADS = {
    "jump if": ("control", f"{S5}:402-410 jumpRelative", "jumps when the condition is false"),
    "jump unless": ("control", f"{S5}:402-410", "jumps when the condition is false"),
    "jump while": ("control", f"{S5}:402-410", "jumps when the condition is false"),
    "jump elseif": ("control", f"{S5}:402-410", "jumps when the condition is false"),
    "jump goto": ("control", f"{S5}:1792-1794 o5_jumpRelative", "always jumps"),
    "jump goto0": ("control", f"{S5}:1792-1794", "always jumps"),
    "jump else": ("control", f"{S5}:1792-1794", "always jumps"),
    "jump elseif-jump": ("control", f"{S5}:1792-1794", "always jumps (ir.py hidden jump)"),
    "jump else-chain": ("control", f"{S5}:1792-1794", "always jumps (ir.py hidden jump)"),
    "jump while-back": ("control", f"{S5}:1792-1794", "always jumps (ir.py hidden jump)"),
    "assign =": ("write", f"{S5}:1932-1968 o5_move, 1152-1194 o5_expression", "var/bit/local = value"),
    "assign +=": ("write", f"{S5}:691-700 o5_add", "x = x + value"),
    "assign -=": ("write", f"{S5}:3125-3130 o5_subtract", "x = x - value"),
    "assign *=": ("write", f"{S5}:1996-2001 o5_multiply", "x = x * value"),
    "assign /=": ("write", f"{S5}:989-998 o5_divide", "x = x / value, a zero divisor is an error"),
    "assign ++": ("write", f"{S5}:1519-1522 o5_increment", "x = x + 1"),
    "assign --": ("write", f"{S5}:967-970 o5_decrement", "x = x - 1"),
    "breakHere": ("yield", f"{S5}:815-832", "the slot yields"),
    "delay": ("yield", f"{S5}:972-980", "paused, then breakHere"),
    "delayVariable": ("yield", f"{S5}:982-987", "paused, then breakHere"),
    "WaitForActor": ("wait", f"{S5}:3272-3310 o5_wait", "forks: blocked while the actor walks (a yield at "
                     "the wait), else falls through"),
    "WaitForMessage": ("wait", f"{S5}:3272-3310", "forks: blocked while a message shows, else falls through"),
    "WaitForCamera": ("wait", f"{S5}:3272-3310", "forks: blocked while the camera moves, else falls through"),
    "WaitForSentence": ("wait", f"{S5}:3272-3310", "forks: blocked while a sentence is queued or runs, else "
                        "falls through"),
    "stopObjectCode": ("end", f"{S5}:3009-3011", "ends the running script"),
    "chainScript": ("end", f"{S5}:834-851", "ends the running script, starts N with the running slot's own F and "
                    "R flags. Row 0: script/local:N = 1, N's at:/esc: keys unknown unless the slot has R; calls: N "
                    "runs nested in its place"),
    "startScript": ("write", f"{S5}:2892-3007; script.cpp:38-107", "without R the start stops N first; N = 0 does "
                    "nothing. Row 0: script/local:N = 1 (N's at:/esc: keys unknown; the running script ends when it "
                    "is N); calls: N runs nested to its first yield or end"),
    "startObject": ("write", f"{S5}:2881-2890; script.cpp:109-153", "never with R: stops O's scripts first. Row 0: "
                    "objscript:O = 1 when O is in the room or inventory and has an entry for the verb (O's at:/esc: "
                    "keys unknown); calls: that entry runs nested"),
    "stopScript": ("write", f"{S5}:3017-3039; script.cpp:262-293", "script/local:N = 0; N = 0 ends the running script"),
    "stopObjectScript": ("write", f"{S5}:3013-3015; script.cpp:296-327", "objscript:O = 0"),
    "doSentence": ("deferred", f"{S5}:1000-1015; script.cpp:1145-1172", "queues a sentence that VAR_SENTENCE_SCRIPT "
                   "runs at the end of a later frame (rows 0-2: not run, a note; settle: queued, run at a round's "
                   "end); STOP: stops VAR_SENTENCE_SCRIPT (settle: and empties the queue)"),
    "setState": ("write", f"{S5}:2731-2752", "state:O = S"),
    "setClass": ("write", f"{S5}:642-689; object.cpp:255-285", "class:O:C = 1 with 0x80, 0 without; class 0 clears all"),
    "setOwnerOf": ("write", f"{S5}:2722-2729; object.cpp:98-181", "owner:O = A; A = 0 stops O's scripts; inventory "
                   "hook with Local[0] = 0"),
    "pickupObject": ("write", f"{S5}:2021-2034", "owner:O = VAR_EGO, class:O:32 = 1, state:O = 1; inventory hook with "
                     "Local[0] = 1"),
    "setObjectName": ("write", f"{S5}:2699-2720; object.cpp:1296-1320", "the name (not a fact); inventory hook with "
                      "Local[0] = 0"),
    "drawObject": ("write", f"{S5}:1031-1129", "state:O = 1 (or the image) when O is the room's; same-place "
                   "objects go to state 0 (their geometry is not known: a note)"),
    "VerbOps": ("write", f"{S5}:3132-3270", "verb:V after New (hidden), On (shown when the slot exists), Off, "
                "Dim, Delete (hidden); other sub-ops touch looks only"),
    "saveVerbs": ("write", f"{S5}:2646-2693", "verb:V = 0 for V in [a, b]"),
    "restoreVerbs": ("write", f"{S5}:2646-2693", "verb:V for V in [a, b] unknown (the saved copies' modes are "
                     "not facts)"),
    "setVarRange": ("write", f"{S5}:2754-2768; script.cpp:805-840", "consecutive vars, bits or locals from the "
                    "first argument"),
    "loadRoom": ("write", f"{S5}:1812-1855; room.cpp:42-260", "room = R; ends room, local and not-owned object units; "
                 "room_change: all of startScene (the exit and entry scripts, the kills, the engine variables)"),
    "loadRoomWithEgo": ("write", f"{S5}:1857-1902", "room = R; ends room, local and not-owned object units; "
                        "inventory hook (setCameraFollows, camera.cpp:85); room_change: startScene with the ego, "
                        "VAR_WALKTO_OBJ"),
    "actorFollowCamera": ("write", f"{S5}:412-414; camera.cpp:62-86, 201-212", "forks on whether the actor is in this "
                          "room (not a fact); if not, room changes to its room (unknown) and, with calls, the scroll "
                          "hook runs; inventory hook"),
    "setCameraAt": ("write", f"{S5}:2695-2697; camera.cpp:30-59", "the scroll hook runs when VAR_SCROLL_SCRIPT is set"),
    "cutscene": ("write", f"{S5}:940-953; script.cpp:1632-1648", "cutscene + 1, esc: = 0 (the new level has no "
                 "override); VAR_CUTSCENE_START_SCRIPT hook with the cutscene's list (calls: while the caller is "
                 "cutSceneScriptIndex)"),
    "endCutscene": ("write", f"{S5}:955-960; script.cpp:1650-1687", "VAR_OVERRIDE = 0, cutscene - 1, esc: = 0 (the "
                    "level below may show one: a note); VAR_CUTSCENE_END_SCRIPT hook with the level's data"),
    "beginOverride": ("write", f"{S5}:2010-2015; script.cpp:1711-1726", "VAR_OVERRIDE = 0; esc:<unit>:W = 1 for "
                      "the jump after it, any other esc: = 0 (one override per level)"),
    "endOverride": ("write", f"{S5}:2010-2015; script.cpp:1728-1737", "VAR_OVERRIDE = 0; esc: = 0"),
    "UserputOn": ("write", f"{S5}:853-938", "VAR_USERPUT = 1"),
    "UserputOff": ("write", f"{S5}:853-938", "VAR_USERPUT = 0"),
    "UserputSoftOn": ("write", f"{S5}:853-938", "VAR_USERPUT + 1"),
    "UserputSoftOff": ("write", f"{S5}:853-938", "VAR_USERPUT - 1"),
    "CursorShow": ("write", f"{S5}:853-938", "VAR_CURSORSTATE = 1"),
    "CursorHide": ("write", f"{S5}:853-938", "VAR_CURSORSTATE = 0 (1 only on the Mac)"),
    "CursorSoftOn": ("write", f"{S5}:853-938", "VAR_CURSORSTATE + 1"),
    "CursorSoftOff": ("write", f"{S5}:853-938", "VAR_CURSORSTATE - 1"),
    "RoomScroll": ("write", f"{S5}:2350-2380", "VAR_CAMERA_MIN_X/MAX_X, clamped to [160, room width - 160]"),
    "InitCharset": ("none", f"{S5}:853-938", "charset; VAR_CURSORSTATE and VAR_USERPUT are rewritten unchanged"),
    "CursorCommand": ("none", f"{S5}:853-938", "charset colours; VAR_CURSORSTATE and VAR_USERPUT unchanged"),
    "print": ("write", f"{S5}:2058-2076, 3443-3460; string.cpp:119, 164-169", "text to actor 252 shows a message "
              "box whose key press goes to VAR_KEYPRESS (var 0): unknown; to an actor, actorTalk writes VAR_HAVE_MSG "
              "and VAR_TALK_ACTOR (actor.cpp:3368, 3446), engine variables not written here"),
    "printEgo": ("none", f"{S5}:2078-2083; actor.cpp:3368, 3446", "text; actorTalk writes VAR_HAVE_MSG and "
                 "VAR_TALK_ACTOR, engine variables not written here"),
    "animateCostume": ("none", f"{S5}:786-813 o5_animateActor", "actor animation"),
    "ActorOps": ("none", f"{S5}:424-640", "actor costume, palette, name, speed, scale"),
    "putActor": ("none", f"{S5}:2094-2131", "actor position"),
    "putActorInRoom": ("none", f"{S5}:2160-2196", "actor room (not a fact)"),
    "putActorAtObject": ("none", f"{S5}:2133-2158", "actor position"),
    "walkActorTo": ("none", f"{S5}:3312-3389", "actor walk"),
    "walkActorToObject": ("none", f"{S5}:3417-3428", "actor walk"),
    "walkActorToActor": ("none", f"{S5}:3391-3415", "actor walk"),
    "faceActor": ("none", f"{S5}:1196-1202", "actor facing"),
    "setBoxFlags": ("none", f"{S5}:1904-1930 o5_matrixOps", "walk boxes"),
    "createBoxMatrix": ("none", f"{S5}:1904-1930", "walk boxes"),
    "startSound": ("none", f"{S5}:2806-2833; sound.cpp:121-122", "sound; writes VAR_LAST_SOUND, an engine "
                   "variable not written here"),
    "stopSound": ("none", f"{S5}:2839-2864", "sound"),
    "SetStringChar": ("none", f"{S5}:3041-3123 o5_stringOps", "string resources"),
    "PutCodeInString": ("none", f"{S5}:3041-3123", "string resources"),
    "CopyString": ("none", f"{S5}:3041-3123", "string resources"),
    "CreateString": ("none", f"{S5}:3041-3123", "string resources"),
    "SetScreen": ("none", f"{S5}:2350-2644 o5_roomOps", "screen"),
    "SetPalColor": ("none", f"{S5}:2350-2644", "palette"),
    "ShakeOn": ("none", f"{S5}:2350-2644", "screen"),
    "ShakeOff": ("none", f"{S5}:2350-2644", "screen"),
    "RoomIntensity": ("none", f"{S5}:2350-2644", "palette"),
    "setRGBRoomIntensity": ("none", f"{S5}:2350-2644", "palette"),
    "screenEffect": ("none", f"{S5}:2350-2644", "screen transition"),
    "drawBox": ("none", f"{S5}:1017-1029", "screen"),
    "freezeScripts": ("write", f"{S5}:1252-1259; script.cpp:914-966", "calls: a flag not 0 freezes every live slot "
                      "but the running one (freeze-resistant ones only from 0x80), 0 unfreezes; frozen:<unit>. Row 0: "
                      "no effect (a frozen script is still live, facts.py)"),
    "PseudoRoom": ("none", f"{S5}:2085-2092", "the room resource map, read from the scripts once (Game.pseudo)"),
    "panCameraTo": ("none", f"{S5}:2017-2019; camera.cpp:120-170", "camera target; the camera and the scroll "
                    "hook move in later frames"),
    "systemOps": ("unmodelled", f"{S5}:2198-2214", "restart, pause or quit"),
}
for _name in ("loadScript", "lockScript", "unlockScript", "loadSound", "lockSound", "unlockSound", "loadCostume",
              "lockCostume", "unlockCostume", "loadRoom", "lockRoom", "unlockRoom", "loadCharset", "clearHeap"):
    HEADS[f"Resource.{_name}"] = ("none", f"{S5}:2216-2348 o5_resourceRoutines", "resource loading and locking")

# Getters (an assign's right side or inside an expression): "fact" ones are evaluated from facts; the
# others are unknown for the reason given. Text codes appear only inside strings.
GETTERS = {
    "classOfIs": ("fact", f"{S5}:1490-1517; object.cpp:229-250"),
    "getObjectOwner": ("fact", f"{S5}:1428-1431"),
    "getObjectState": ("fact", f"{S5}:1433-1436"),
    "isScriptRunning": ("fact", f"{S5}:1447-1479; script.cpp:1557-1564"),
    "getInventoryCount": ("fact", f"{S5}:1423-1426; object.cpp:87-96"),
    "getVerbEntryPoint": ("fact", f"{S5}:1481-1488; script.cpp:167-235"),
    "getRandomNr": (R_RANDOM, f"{S5}:1438-1445"),
    "getActorX": (R_POSITION, f"{S5}:1347-1357"),
    "getActorY": (R_POSITION, f"{S5}:1359-1369"),
    "getActorWalkBox": (R_POSITION, f"{S5}:1333-1338"),
    "getActorElevation": (R_POSITION, f"{S5}:1268-1273"),
    "getDist": (R_POSITION, f"{S5}:1406-1421"),
    "findObject": (R_POSITION, f"{S5}:1211-1250"),
    "actorFromPos": (R_POSITION, f"{S5}:416-422"),
    "getActorRoom": (R_ACTOR, f"{S5}:1289-1322"),
    "getActorCostume": (R_ACTOR, f"{S5}:1261-1266"),
    "getActorFacing": (R_ACTOR, f"{S5}:1275-1280"),
    "getActorMoving": (R_ACTOR, f"{S5}:1282-1287"),
    "getActorWidth": (R_ACTOR, f"{S5}:1340-1345"),
    "isSoundRunning": (R_SOUND, f"{S5}:2866-2873"),
    "GetStringChar": (R_STRING, f"{S5}:3041-3123"),
    "findInventory": (R_INVENTORY, f"{S5}:1204-1209; object.cpp:77-85 (the inventory list's order is not a fact)"),
}
for _name in ("getInt", "getName", "getString", "getVerb", "keepText", "newline", "unknown8", "wait"):
    GETTERS[_name] = (R_WORK, "a text code inside a string (descumm), never a value")
for _g in GETTERS:
    HEADS.setdefault(f"assign = {_g}", ("write", GETTERS[_g][1], "x = the getter's value"))


class Machine:
    """Runs units over a fact oracle. ``features`` picks the growth-table row (module docstring)."""

    def __init__(self, features=ROW0, game: Game | None = None, path_limit: int = PATH_LIMIT,
                 step_limit: int = STEP_LIMIT, round_limit: int = ROUND_LIMIT, switches=()):
        features, switches = frozenset(features), frozenset(switches)
        if unknown := switches - set(SWITCHES):
            raise ValueError(f"unknown switches {sorted(unknown)}; known: {sorted(SWITCHES)}")
        self.switches = switches
        self.const_polls = "const_polls" in switches  # settle's loop classes (Game.loops) under switch const_polls
        if unknown := features - set(FEATURES):
            raise ValueError(f"unknown features {sorted(unknown)}; known: {FEATURES}")
        if todo := features - IMPLEMENTED:
            raise NotImplementedError(f"features not built yet: {sorted(todo)}")
        if "own_entry" not in features:
            raise ValueError("row 0 (own_entry) is the base every row builds on")
        if ("dispatch" in features) != ("calls" in features):
            raise ValueError("dispatch and calls make sense only together (row 1)")
        if "room_change" in features and "calls" not in features:
            raise ValueError("room_change runs the exit and entry scripts nested: it needs calls")
        if "settle" in features and "room_change" not in features:
            raise ValueError("settle runs the rounds after rows 0-2 (row 3): it needs room_change")
        if "dialog" in features and "settle" not in features:
            raise ValueError("dialog resumes the script waiting on the menu by settle's rule c (row 4): it needs settle")
        if "esc" in features and "settle" not in features:
            raise ValueError("esc runs the Esc frame's turns and then the rounds after it (row 5): it needs settle")
        self.features, self.game = features, game or Game.load()
        self.calls, self.rooms, self.settles = "calls" in features, "room_change" in features, "settle" in features
        self.dialogs, self.escs = "dialog" in features, "esc" in features
        self.path_limit, self.step_limit, self.round_limit = path_limit, step_limit, round_limit
        self._pre = ()  # settle: the slots live at pre of the current run
        self._key_routes = []  # esc: the current run's ``Run.key_routes``
        self._held = {}  # calls: the steps of every Call a join signature of the current run named (``_join``)

    # --- entry points -----------------------------------------------------------------------------------

    def run_entry(self, oracle: Oracle, obj: int, verb: int, locals: dict | None = None) -> Run:
        """Row 0: object ``obj``'s own entry for ``verb``. The slot is an inventory slot when the
        object is not the room's (whereIsObject, object.cpp:355-367), read from the oracle."""
        idx = self.game.entry(obj, verb)
        if idx is None:
            why = "no code" if ("obj", obj) not in self.game.units else f"no entry for verb {verb} or 255"
            return Run([], f"object {obj}: {why}", unit=("obj", obj))
        owner = oracle.get(f"owner:{obj}")
        owned = owner if isinstance(owner, Unknown) else owner != OF_OWNER_ROOM
        # every v5 object slot runs without R: o5_startObject and the engine's own verb code
        # (script_v5.cpp:2889, script.cpp:1432)
        return self.run(oracle, ("obj", obj), idx, locals, owned, recursive=False)

    def run(self, oracle: Oracle, unit: tuple, pc: int, locals: dict | None = None, owned=False,
            recursive: bool | None = None) -> Run:
        """Every path from instruction ``pc`` of ``unit``; ``locals`` None means unknown arguments,
        ``recursive`` None an unknown R flag of the slot. A machine runs one run at a time
        (``self.oracle`` is the run's oracle). With calls on, the unit is the first frame of a stack
        and its old wait points go (it runs from ``pc`` now)."""
        self.oracle = oracle
        self._pre = ()
        u = self.game.units[unit]
        first = Path([Frame(unit, pc, dict(locals or {}), locals is not None, owned, recursive, sid=1)], next_sid=2)
        if self.calls:
            first.depth = 1
            self.clear(first, f"at:{unit_name(unit)}:", None, "run")
        start = u.ins[pc].cite() if pc < len(u.ins) else f"{u.file}: past the end"
        return self._drive(oracle, first, start, unit)

    def run_sentence(self, oracle: Oracle, verb: int, a: int, b: int | None = None) -> Run:
        """Feature dispatch: the sentence (verb, A, B) from the click's frame on (module docstring). A
        live, unfrozen slot of the sentence script's number first has its turn in runAllScripts
        (``_eng_resume``); then checkAndRunSentenceScript (``_eng_sentence``) holds the sentence back
        while such a slot is still live and not frozen, drops it when B is A, and else runs the
        sentence script. A path that does not start the sentence script ends with why: deferred,
        deferral unknown, dropped, no sentence script; it keeps the writes of the turn before. With
        settle, only "deferral unknown" ends it: the others are how its immediate part ended
        (``Path.first_end``; a deferred sentence stays queued) and time passes in rounds after it; and the
        click's frame also runs the other live slots' turns before the sentence, with the mouse variables
        unknown (module docstring, settle)."""
        self.oracle = oracle
        n = oracle.get(f"var:{VAR_SENTENCE_SCRIPT}")
        if isinstance(n, Unknown):
            return Run([], f"VAR_SENTENCE_SCRIPT {n}")
        steps = []
        for k in self._sentence_slots(n):
            live, frozen = oracle.get(k), oracle.get(f"frozen:{slot_name(k)}")
            if live == 0:
                continue
            if isinstance(live, Unknown) or isinstance(frozen, Unknown):
                why = f"whether the sentence waits: {k} {live}, frozen {frozen}"
                return Run([Path([], end="deferral unknown", end_at=why)], why)
            if not frozen:  # a frozen slot does not run (script.cpp:979) and does not hold the sentence
                steps.append(("resume", k))
        if len(steps) > 1:  # runAllScripts runs them in slot order, which is not a fact
            steps.insert(0, ("note", f"{len(steps)} live slots of number {n} run in an order that is not a fact"))
        if self.settles:
            # settle: the click's frame runs every live slot's turn (runAllScripts, scumm.cpp:3231) before the
            # click is taken (checkExecVerbs, 3232) and the sentence runs (checkAndRunSentenceScript, 3255), and
            # the mouse variables already hold the click (processInput 3177, updateScummVars 3198, 3385-3388)
            resumed = {slot_name(st[1]): st for st in steps if st[0] == "resume"}
            turns = [resumed.get(name, ("turn", name)) for name in self._pre_slots(oracle)]
            steps = [("write", f"var:{v}", Unknown(R_INPUT, f"{MOUSE_VARS[v]}: the mouse moved to the click"))
                     for v in sorted(MOUSE_VARS)] + [st for st in steps if st[0] == "note"] + turns
        steps.append(("sentence", verb, a, b))
        call = Call("checkAndRunSentenceScript", "script.cpp:1244-1246", tuple(steps), caller=None, active=False)
        key = self.game.script_key(n) if n else None
        unit = ("global", n) if key and key.startswith("script:") else None
        u = self.game.units.get(unit) if unit else None
        start = u.ins[0].cite() if u is not None and u.ins else f"sentence script {n}"
        if any(st[0] == "resume" for st in steps):
            start += " (after the turn of " + ", ".join(st[1] for st in steps if st[0] == "resume") + ")"
        self._pre = self._pre_slots(oracle) if self.settles else ()
        return self._drive(oracle, Path([call], settle=self.settles), start, unit)

    def run_wait(self, oracle: Oracle) -> Run:
        """Feature settle: the wait action ('.'). Nothing is clicked, so there is no immediate part: time
        passes in rounds (``_next_round``) from the pre state. No slot is resumed by the action itself (rule
        a starts empty); slots live at pre run by rules b and c, and slots settle itself starts run by rule a
        from then on."""
        if not self.settles:
            raise ValueError("a wait runs only with settle")
        self.oracle = oracle
        self._pre = self._pre_slots(oracle)
        p = Path([], settle=True, first_end="wait", immediate={})
        self._next_round(p)
        return self._drive(oracle, p, "wait: runAllScripts each round (script.cpp:968-987)", None)

    def run_verb(self, oracle: Oracle, verb: int, button: int = LEFT_CLICK) -> Run:
        """Feature dialog: a click on verb ``verb`` (a dialog line is a verb, ``actions.dialog_line``) from the
        click's frame on (module docstring, dialog). The frame runs as the engine runs it (scumm.cpp:3177-3255):
        the click moves the mouse (its variables unknown, as settle's click), runAllScripts gives every slot live
        at pre its turn by settle's rules (``_eng_turn``), checkExecVerbs then takes the click and runs the input
        script with [kVerbClickArea, verb, button] (``_eng_input``), and checkAndRunSentenceScript runs a sentence
        the input script queued (``_eng_sentences``). Then time passes in rounds (settle)."""
        if not self.dialogs:
            raise ValueError("a click on a verb runs only with dialog")
        self.oracle = oracle
        self._pre = self._pre_slots(oracle)
        steps = [("write", f"var:{v}", Unknown(R_INPUT, f"{MOUSE_VARS[v]}: the mouse moved to the click"))
                 for v in sorted(MOUSE_VARS)]
        steps += [("turn", name) for name in self._pre]
        steps += [("input", VERB_CLICK_AREA, verb, button), ("sentences",)]
        call = Call("click", "scumm.cpp:3177-3255", tuple(steps), caller=None, active=False)
        n = oracle.get(f"var:{VAR_VERB_SCRIPT}")
        unit = None
        if isinstance(n, int) and n:
            key = self.game.script_key(n)
            res = self.game.resource(oracle.get("room")) if key.startswith("local:") else None
            unit = ("global", n) if key.startswith("script:") else (None if isinstance(res, Unknown) else ("local", res, n))
        u = self.game.units.get(unit) if unit else None
        start = u.ins[0].cite() if u is not None and u.ins else f"input script {n}"
        return self._drive(oracle, Path([call], settle=True), start, unit)

    def _eng_input(self, p: Path, call: Call, oracle: Oracle, area: int, val: int, button: int) -> list:
        """checkExecVerbs' click on a verb (verbs.cpp:683-686) calls runInputScript(kVerbClickArea, verbid, code)
        (script.cpp:1478-1511): VAR_VERB_SCRIPT, when it is not 0, runs by runScript(N, 0, 0, args), args[0] the
        click area, args[1] the verb, args[2] the button and the rest 0 (the HE and Macintosh cases there do not
        apply to a v5 DOS game). N below _numGlobalScripts is global, else the local script of the room in use
        (script.cpp:70-86). runAllScripts leaves _currentScript at 0xFF (script.cpp:972-986: a turn ends at a yield
        or an end, which set it), so no caller resumes after the run (script.cpp:377-390). Not freeze resistant and
        not recursive, so it stops a running copy first. It runs nested like any start (calls), from its first
        instruction, with rows 1-2's rules (the click's immediate part, as the click's sentence: a WaitFor op
        forks). N is the input script installed at pre (the spec's rule); the engine reads VAR_VERB_SCRIPT here,
        after the frame's turns, as it reads _userPut and the verb's state: where the path's store already differs
        from pre on VAR_VERB_SCRIPT, VAR_USERPUT or the clicked verb's ``verb:`` fact, it is kept in
        ``Path.input_drift`` (counted). Switch ``input_after_turns`` (OFF) reads VAR_VERB_SCRIPT and VAR_USERPUT
        from the store instead, as the engine does: input off drops the click (verbs.cpp:590), not known forks;
        a verb the turns hid is only noted (findVerbAtPos would miss it; not modelled)."""
        p.owner, p.timed = None, False
        drift = []
        for k in (f"var:{VAR_VERB_SCRIPT}", f"var:{VAR_USERPUT}", f"verb:{val}"):
            now, before = self.read(p, k, oracle), oracle.get(k)
            if now != before:
                drift.append((k, before, now))
        p.input_drift = tuple(drift)
        more = []
        if "input_after_turns" in self.switches:
            # switch (OFF by default): checkExecVerbs' own reads, on the path's store (verbs.cpp:590, script.cpp:1482)
            off = _decide(lambda y: i32(y) <= 0, self.read(p, f"var:{VAR_USERPUT}", oracle))
            if isinstance(off, Unknown):
                q = p.fork()
                p.assumptions.append((call.cite, "_userPut <= 0: the click is dropped", False, str(off)))
                q.assumptions.append((call.cite, "_userPut <= 0: the click is dropped", True, str(off)))
                self._note(q, call.cite, "checkExecVerbs", "_userPut <= 0 after the turns: the click is dropped "
                                                           "(verbs.cpp:590)")
                more = [q]
            elif off:
                self._note(p, call.cite, "checkExecVerbs", "_userPut <= 0 after the turns: the click is dropped "
                                                           "(verbs.cpp:590)")
                return []
            if self.read(p, f"verb:{val}", oracle) != 1:
                self._note(p, call.cite, "checkExecVerbs", f"verb {val} may not be shown after the turns: "
                                                           "findVerbAtPos would not find it (not modelled)")
            n = self.read(p, f"var:{VAR_VERB_SCRIPT}", oracle)
        else:
            n = oracle.get(f"var:{VAR_VERB_SCRIPT}")
        if isinstance(n, Unknown):
            p.unknown_writes.append(("runInputScript", call.cite, f"input script {n}"))
            return more
        if n == 0:  # if (verbScript) runScript(...) (script.cpp:1509-1510)
            self._note(p, call.cite, "runInputScript", "VAR_VERB_SCRIPT is 0: no input script runs")
            return more
        p.stack.append(Call("runInputScript", "script.cpp:1478-1511", (("script", n, False, False, {0: area, 1: val,
                                                                                                    2: button}),),
                            caller=None, active=False))
        return more

    def run_esc(self, oracle: Oracle) -> Run:
        """Feature esc: the Esc key (a skipped cutscene, ``actions.skip_cutscene``) from its frame on (module docstring,
        esc). The frame runs as the engine runs it (scumm.cpp:3177-3255): processInput's abortCutscene rewinds the slot
        of the override armed at the top level to its override point (``_eng_abort``); runAllScripts gives every slot
        live at pre its turn by settle's rules, the rewound slot from its override point by rule a (``_eng_override``);
        checkExecVerbs takes the key code VAR_CUTSCENEEXIT_KEY (``_eng_key``); checkAndRunSentenceScript runs a sentence
        queued in the frame (``_eng_sentences``). Then time passes in rounds (settle). No mouse moves: the mouse
        variables keep pre's values. The override is the one ``esc:`` fact that is 1 at pre: abortCutscene reads it in
        processInput, before any script of the frame runs (scumm.cpp:3177, 3231)."""
        if not self.escs:
            raise ValueError("the Esc key runs only with esc")
        self.oracle = oracle
        self._pre = self._pre_slots(oracle)
        armed = sorted(k for k in (oracle.keys("esc:") or []) if oracle.get(k) == 1)
        code = oracle.get(f"var:{VAR_CUTSCENEEXIT_KEY}")
        steps, unit, start = [("abort", tuple(armed), code)], None, "Esc: no override armed at the top level"
        name = w = None
        if armed and not isinstance(code, Unknown) and code != 0:
            name, w = armed[0][4:].rsplit(":", 1)
            unit = self._unit_named(Path([]), name, oracle)
            u = self.game.units.get(unit) if unit is not None else None
            idx = u.at.get(int(w)) if u is not None else None
            start = f"Esc: abortCutscene rewinds {name} to " + (u.ins[idx].cite() if idx is not None else f"[{w}]")
        steps += [("override", n, w) if n == name else ("turn", n) for n in self._pre]
        if name is not None and name not in self._pre:  # the override's slot is not a slot live at pre (not expected)
            steps.append(("override", name, w))
        steps += [("key", code), ("sentences",)]
        call = Call("Esc", "scumm.cpp:3177-3255", tuple(steps), caller=None, active=False)
        return self._drive(oracle, Path([call], settle=True), start, unit)

    def _eng_abort(self, p: Path, call: Call, oracle: Oracle, armed: tuple, code) -> None:
        """processInput's Esc (input.cpp:1423-1428): with VAR_CUTSCENEEXIT_KEY not 0 (input.cpp:966), abortCutscene
        (script.cpp:1689-1709) on the top level of the cutscene stack: when an override is armed there (the ``esc:``
        fact), the slot that armed it (vm.cutSceneScript[idx], 1695) stands at the override point W, the jump after its
        beginOverride (ss->offs, 1699), ssRunning (a delay or a WaitFor op it was in is over, 1700), with freeze count 0
        (1701), its cutsceneOverride one lower when above 0 (1703-1704: a note, its count is not a fact in facts.py for
        a slot live at pre, though the bridge's dump holds it), VAR_OVERRIDE = 1 (1706) and no override at the level
        (1707). The cutscene depth stays. With none armed it does nothing (1693-1694). The slot is the one that armed
        the override: facts.py gives no ``esc:`` fact for an override the bridge calls stale, whose slot abortCutscene
        would rewind whatever runs there now. Nothing of it reads what a slot does in the frame: it runs before
        runAllScripts (scumm.cpp:3177, 3231), on the pre state. With VAR_CUTSCENEEXIT_KEY not known nothing is run
        (whether the branch is taken is not known; the action's usability is unknown, ``actions.py``)."""
        p.owner, p.timed = None, False
        if isinstance(code, Unknown):
            self._note(p, call.cite, "Esc", f"VAR_CUTSCENEEXIT_KEY {code}: whether Esc is the cutscene exit key is "
                                            "not known (input.cpp:966, 1423), abortCutscene does not run here (the "
                                            "action's usability is unknown, actions.py)")
            return
        if code == 0:
            self._note(p, call.cite, "Esc", "VAR_CUTSCENEEXIT_KEY 0: Esc is not the cutscene exit key "
                                            "(input.cpp:966, 1423), abortCutscene does not run")
            return
        if not armed:
            self._note(p, call.cite, "Esc", "no override armed at the top level: abortCutscene does nothing "
                                            "(script.cpp:1693-1694)")
            return
        if len(armed) > 1:  # facts.py gives the top level's override only
            self._note(p, call.cite, "Esc", f"{len(armed)} esc: facts are 1; the first is taken")
        key = armed[0]
        name, w = key[4:].rsplit(":", 1)
        self.clear(p, f"at:{name}:", None, "abortCutscene")
        self.write(p, f"at:{name}:{w}", 1)
        self._set_freeze(p, name, (0, True))
        self._note(p, "script.cpp:1703-1704", "assumption: cutsceneOverride",
                   f"{name}: its cutsceneOverride count is not a fact in facts.py (the bridge's dump holds it per "
                   "slot; adding it is held); kept at 0 as for every slot live at pre")
        self.write(p, f"var:{VAR_OVERRIDE}", 1)
        self.write(p, key, 0)

    def _eng_override(self, p: Path, call: Call, oracle: Oracle, name: str, w: str) -> list:
        """The turn of the slot abortCutscene rewound, in runAllScripts (script.cpp:968-987): from its override point
        (the jump after beginOverride), by rule a (resumed by the action), with rows 1-2's rules for what it meets, as
        the click's sentence (a WaitFor op forks). Its locals and R flag are not facts (unknown), its F flag comes from
        the start sites, and runAllScripts runs it with executeScript, not nested (script.cpp:980-983). An earlier turn
        in the frame that stopped, froze or moved it leaves it to settle's own rule (``_eng_turn``)."""
        p.owner = None
        if self.read(p, f"at:{name}:{w}", oracle) != 1 or name in p.round_ran:
            return self._eng_turn(p, call, oracle, name)  # an earlier turn in the frame moved it: settle's own rule
        unit = self._unit_named(p, name, oracle)
        u = self.game.units.get(unit) if unit is not None else None
        idx = u.at.get(int(w)) if u is not None else None
        key = None if name in ("entry", "exit") else ("objscript:" + name[4:] if name.startswith("obj:") else name)
        live = self.read(p, key, oracle) if key is not None else 1
        frozen = self.read(p, f"frozen:{name}", oracle)
        if idx is None or live != 1 or frozen != 0:
            why = "its override point is no instruction of its unit" if idx is None else \
                f"its liveness {live} or freeze {frozen} changed before its turn"
            self._skip(p, name, why)
            return []
        rule = "a: rewound by Esc (abortCutscene)"
        p.owner, p.owner_rule, p.timed = name, rule, False
        p.rw.setdefault(name, (set(), set()))
        p.round_ran.add(name)
        p.ran_by.setdefault(name, rule)
        p.not_run.pop(name, None)
        owned = False
        if unit[0] == "obj":
            owner = self.read(p, f"owner:{unit[1]}", oracle)
            owned = owner if isinstance(owner, Unknown) else owner != OF_OWNER_ROOM
        res = self.game.resource(self.read(p, "room", oracle)) if unit[0] == "local" else None
        resistant = self.game.resistant(key, res) if key is not None else False
        if resistant is None:
            self._note(p, call.cite, "resume", f"{name}: F flag not known")
        self.clear(p, f"at:{name}:", None, "resume")  # it runs from its override point now
        self._push_frame(p, call, unit, idx, {}, owned, None, resistant, key, args_known=False, nested=False)
        return []

    def _eng_key(self, p: Path, call: Call, oracle: Oracle, code) -> list:
        """checkExecVerbs (scumm.cpp:3232, verbs.cpp:586-664) takes the key code the Esc left in _mouseAndKeyboardStat
        (VAR_CUTSCENEEXIT_KEY's value, input.cpp:1427-1428; with it 0 the Esc branch is not taken and the key is
        its ASCII code, KEYCODE_ESCAPE, input.cpp:1553-1558). Nothing when a doSentence(STOP) in the frame's turns ran
        clearClickedStatus, which sets _mouseAndKeyboardStat to 0 (script_v5.cpp:1004-1008, input.cpp:378-386;
        ``Path.cleared``: a cutscene end hook may do it), nor while _userPut is 0 or below (verbs.cpp:590): the engine
        reads it after the frame's turns, so the machine reads VAR_USERPUT, its mirror (script_v5.cpp:935-938), from
        the path's store (switch ``esc_key_at_pre``: at pre), and forks when it is not known. A code of Enter or Tab
        while the cursor shows is a click (input.cpp:1566-1570), and one at or above MBS_MAX_KEY is not a key
        (verbs.cpp:593): not modelled, noted. A shown verb whose key is the code would take it as a click on that verb
        (verbs.cpp:608-615): a verb's key is not a fact (``actions.VERB_KEY``, unchecked), so the machine takes the
        generic branch: runInputScript(kKeyClickArea, code, 1) (verbs.cpp:664, script.cpp:1478-1511) runs
        VAR_VERB_SCRIPT (read where _userPut is), not nested, with no caller to resume, as dialog's click runs it.
        Each way the key goes is kept with the path's assumptions at that point (``_key_route``)."""
        p.owner, p.timed = None, False
        at_pre = "esc_key_at_pre" in self.switches
        more = []
        if p.cleared is not False:  # a doSentence(STOP) in the frame's turns: clearClickedStatus wiped the key
            gone = "a doSentence(STOP) in the frame's turns ran clearClickedStatus: the key is gone before " \
                   "checkExecVerbs (script_v5.cpp:1004-1008, input.cpp:378-386)"
            if p.cleared is None:  # merged paths disagree: both sides
                q = p.fork()
                why = str(Unknown(R_BRANCH, "whether a doSentence(STOP) ran in the frame"))
                p.assumptions.append((call.cite, "clearClickedStatus ran in the frame", False, why))
                q.assumptions.append((call.cite, "clearClickedStatus ran in the frame", True, why))
                self._note(q, call.cite, "Esc key", gone)
                self._key_route(q, "wiped by clearClickedStatus")
                more = [q]
            else:
                self._note(p, call.cite, "Esc key", gone)
                self._key_route(p, "wiped by clearClickedStatus")
                return []
        if code == 0:  # Esc is not the exit key: processKeyboard's default branch gives the key's ASCII code
            code = KEYCODE_ESCAPE
        if isinstance(code, Unknown):
            self._note(p, call.cite, "Esc key", f"the key code {code} is not known: checkExecVerbs not run")
            self._key_route(p, "key code not known")
            return more
        if code in (KEYCODE_RETURN, KEYCODE_TAB):
            cursor = oracle.get(f"var:{VAR_CURSORSTATE}")
            if isinstance(cursor, Unknown) or cursor > 0:
                self._note(p, call.cite, "Esc key", f"key code {code} with cursor state {cursor}: a click "
                                                    "(input.cpp:1566-1570), not modelled")
                self._key_route(p, "a click (Enter or Tab), not modelled")
                return more
        if code >= MBS_MAX_KEY or code < 0:
            self._note(p, call.cite, "Esc key", f"key code {code} is not below MBS_MAX_KEY (verbs.cpp:593): not modelled")
            self._key_route(p, "not a key, not modelled")
            return more
        userput = oracle.get(f"var:{VAR_USERPUT}") if at_pre else self.read(p, f"var:{VAR_USERPUT}", oracle)
        off = _decide(lambda y: i32(y) <= 0, userput)
        if isinstance(off, Unknown):
            q = p.fork()
            p.assumptions.append((call.cite, "_userPut <= 0: the key is dropped", False, str(off)))
            q.assumptions.append((call.cite, "_userPut <= 0: the key is dropped", True, str(off)))
            self._note(q, call.cite, "Esc key", "input off at checkExecVerbs: the key is dropped (verbs.cpp:590)")
            self._key_route(q, "dropped with input off")
            more = more + [q]
        elif off:
            self._note(p, call.cite, "Esc key", "input off at checkExecVerbs: the key is dropped (verbs.cpp:590)")
            self._key_route(p, "dropped with input off")
            return more
        n = oracle.get(f"var:{VAR_VERB_SCRIPT}") if at_pre else self.read(p, f"var:{VAR_VERB_SCRIPT}", oracle)
        if isinstance(n, Unknown):
            p.unknown_writes.append(("runInputScript", call.cite, f"input script {n}"))
            self._note(p, call.cite, "Esc key", f"the input script is not known ({n}): not run")
            self._key_route(p, "input script not known")
            return more
        if n == 0:  # if (verbScript) runScript(...) (script.cpp:1509-1510)
            self._note(p, call.cite, "Esc key", "VAR_VERB_SCRIPT is 0: no input script runs")
            self._key_route(p, "no input script")
            return more
        self._note(p, call.cite, "Esc key", f"input script {n} runs with [{KEY_CLICK_AREA}, {code}, 1]")
        self._key_route(p, f"input script {n}")
        p.stack.append(Call("runInputScript", "script.cpp:1478-1511", (("script", n, False, False, {
            0: KEY_CLICK_AREA, 1: code, 2: 1}),), caller=None, active=False))
        return more

    def _key_route(self, p: Path, fate: str) -> None:
        """Keep where the Esc key went on ``p`` with the assumptions ``p`` holds at that point, the forks the way rests
        on (``Run.key_routes``). Counted when the key is taken, so a path merged later still counts once per way."""
        self._key_routes.append((fate, tuple(p.assumptions)))

    def _sentence_slots(self, n: int) -> list:
        """isScriptInUse matches the number in any slot (script.cpp:1548-1555, 1186-1191): the
        sentence script's slot fact (global or local by number), and an object slot of that number."""
        return [self.game.script_key(n), f"objscript:{n}"] if n else []

    def _drive(self, oracle: Oracle, first: Path, start: str, unit) -> Run:
        """Run every path to its end. With calls on, a path whose nested run returned to the opcode
        that started it waits there (a join: the top of its stack is a call between steps); once no
        path can go on, the deepest waiting paths with the same stack merge into one (``_merge``) and
        go on. Forks inside a nested run so cost one path each, not a product over the scripts."""
        todo, done, waiting, limit, dead = [first], [], collections.defaultdict(list), 0, 0
        self._merges = self._settle_merges = self._value_merges = 0
        self._key_routes = []
        self._held = {}
        VALUE_CAPS.clear()
        while todo or waiting:
            if not todo:
                deepest = max(len(g[0].stack) for g in waiting.values())
                for sig in [s for s, g in waiting.items() if len(g[0].stack) == deepest]:
                    todo.append(self._merge(waiting.pop(sig), sig))
                continue
            p = todo.pop()
            while p.end is None:
                if p.steps - p.steps0 >= self.step_limit:
                    self._halt(p, "limit", f"{self.step_limit} steps")
                    break
                if self.calls and (sig := self._join(p)) is not None:
                    if sig != p.joined and not p.resumed:
                        waiting[sig].append(p)
                        break
                    if isinstance(p.stack[-1], Frame) and self._spins(p, sig):
                        break
                p.resumed = False
                f = p.frame
                n, at, routes = len(p.assumptions), f.pc if f is not None else None, len(self._key_routes)
                more = self._advance(p, oracle)
                if more and len(done) - dead + len(todo) + sum(map(len, waiting.values())) + 1 + len(more) \
                        > self.path_limit:
                    # a fork past the limit: neither side runs; the path ends where it forked
                    limit += len(more)
                    del p.assumptions[n:]
                    del self._key_routes[routes:]
                    p.end, p.end_at = "limit", f"{self.path_limit} paths, at instruction {at}"
                else:
                    todo += more
            if p.end is not None:  # else it waits at a join
                if p.settle and p.end not in INFEASIBLE:
                    self._close_settle(p)
                done.append(p)
                dead += p.end in INFEASIBLE
        return Run(done, start, limit, unit, self._merges, dict(VALUE_CAPS), self._settle_merges, tuple(self._pre),
                   self._value_merges, tuple(self._key_routes))

    def _join(self, p: Path) -> tuple | None:
        """The path's join signature when it stands at a join, else None. Joins: between two steps of
        an opcode's call (after at least one, so a nested run may have returned there), and at an
        instruction some jump goes to (where the two sides of an if, or a loop's entry and its jump
        back, meet: ``Game.join_points``). Paths with one signature have the same stack: frames at the
        same instruction with the same slot state, calls at the same step."""
        top = p.stack[-1] if p.stack else None
        if isinstance(top, Call):
            if top.head == ROUND and top.i == 0:  # a round's start: paths in the same state become one (settle)
                return ("round", p.round, self._position_key(p))
            if top.caller is None or top.i == 0:
                return None
        elif not isinstance(top, Frame) or top.pc not in self.game.join_points(top.unit):
            return None
        sig = [p.cut_index, len(p.cut_data)]
        if p.settle:  # the slots off the stack and the queue are part of the state: paths differing there stay apart
            sig.append((p.round, self._slots_sig(p, full=False), tuple(p.queue), frozenset(p.ambient_done),
                        frozenset(p.round_ran), p.owner, frozenset(p.camera_done), p.timed))
        for e in p.stack:
            if isinstance(e, Frame):
                sig.append(("F", e.sid, e.unit, e.pc, e.killed, e.freeze, e.override, str(e.owned), e.recursive,
                            str(e.resistant), e.args_known) + ((e.time_loops,) if e.time_loops else ()))
            else:
                # a call is named by its steps tuple's id(), which forks share; the tuple is kept to the end of the run,
                # so a later call's steps can never take the address of steps a kept signature names
                self._held[id(e.steps)] = e.steps
                sig.append(("C", e.head, e.cite, id(e.steps), e.i, e.active, e.caller))
        return tuple(sig)

    def _spins(self, p: Path, sig: tuple) -> bool:
        """A path back at a jump target in a machine state it already had in the running frame's turn
        (same stack, locals, writes and freeze counts) adds nothing: every way on from that state is
        run by the path that met it first (the other sides of the forks taken since). The state is the
        machine's, not the engine's: a loop that draws a random number or reads a position again comes
        back to the same machine state although the engine's differs, and leaves the loop on a side the
        first path already took; a loop with no such read would hang the engine (o5 runs a slot until
        it yields, script.cpp:494-523). The path ends as ``spin``, counted, and is left out of the
        predictions (``feasible``; calls). States are kept as 64-bit hashes."""
        f = p.stack[-1]
        state = hash((sig, f.sid, frozenset(f.locals.items()), frozenset(p.writes.items()),
                      frozenset(p.freeze.items())))
        if state in p.seen:
            self._halt(p, "spin", f"{self.game.units[f.unit].ins[f.pc].cite()}: the same state again with no yield")
            return True
        p.seen.add(state)
        return False

    def _merge(self, group: list, sig: tuple) -> Path:
        """One path for the paths waiting at one join: each fact, local and cutscene datum that differs
        becomes the set of their values (``one_of``; the pre value for a path that did not write it),
        a plain unknown when one of them is; the lists the check counts (assumptions, notes, unknown
        writes, errors, turns) keep the first path's and add what each other branch added after the
        history they share. A later test on a merged value forks
        again when its values disagree; links between two merged facts are lost (more possible values,
        never a wrong certain one)."""
        if len(group) == 1:
            group[0].joined, group[0].resumed = sig, True
            return group[0]
        base = group[0]
        q = base.fork()
        for k in sorted(set().union(*(g.writes for g in group))):
            q.writes[k] = _join_values([g.writes[k] if k in g.writes else self.oracle.get(k) for g in group])
        for i, e in enumerate(q.stack):
            if isinstance(e, Frame):
                frames = [g.stack[i] for g in group]
                for n in sorted(set().union(*(f.locals for f in frames))):
                    e.locals[n] = _join_values([f.locals.get(n, 0 if f.args_known else Unknown(R_LOCAL, f"Local[{n}]"))
                                                for f in frames])
        for name, e in list(q.slots.items()):  # settle: a slot off the stack, at the same place in every path
            frames = [g.slots[name] for g in group]
            locals_ = {n: _join_values([f.locals.get(n, 0 if f.args_known else Unknown(R_LOCAL, f"Local[{n}]"))
                                        for f in frames]) for n in sorted(set().union(*(f.locals for f in frames)))}
            q.slots[name] = dataclasses.replace(e, locals=locals_, args_known=all(f.args_known for f in frames))
        same = all(g.writes == base.writes and g.slots == base.slots for g in group[1:])
        q.cut_data = [_join_values([g.cut_data[i] for g in group]) for i in range(len(base.cut_data))]
        for name in sorted(set().union(*(g.freeze for g in group))):
            counts = {g.freeze.get(name, "unset") for g in group}
            q.freeze[name] = counts.pop() if len(counts) == 1 else None
        have = {}  # settle: an entry a merged path already holds is not added again (merges recur every round)
        for g in group[1:]:  # each branch's own additions: what follows the history it shares with the first
            for name in ("assumptions", "unmodelled", "notes", "unknown_writes", "errors", "turns", "conflicts"):
                mine, theirs = getattr(base, name), getattr(g, name)
                k = 0
                while k < len(mine) and k < len(theirs) and mine[k] == theirs[k]:
                    k += 1
                if base.settle and name != "turns":
                    seen = have.setdefault(name, set(getattr(q, name)))
                    add = [e for e in theirs[k:] if e not in seen]
                    seen.update(add)
                    getattr(q, name).extend(add)
                else:
                    getattr(q, name).extend(theirs[k:])
            # settle: a key counts as moved after the immediate part when it did on any branch merged in
            q.after |= g.after
            if g.immediate is not None and base.immediate is not None:
                q.after |= {k for k in set(g.immediate) | set(base.immediate)
                            if g.immediate.get(k, self.oracle.get(k)) != base.immediate.get(k, self.oracle.get(k))}
            for owner, (r, w) in g.rw.items():
                mr, mw = q.rw.setdefault(owner, (set(), set()))
                mr |= r
                mw |= w
            for name, rule in g.ran_by.items():
                q.ran_by.setdefault(name, rule)
            q.skips |= g.skips
            q.ambient_pass |= g.ambient_pass
            q.ambient_slots |= g.ambient_slots
            q.multi |= g.multi
            for k, vs in g.ambient_vals.items():
                mine = q.ambient_vals.setdefault(k, [])
                mine.extend(v for v in vs if v not in mine)
            for k, rule in g.writer.items():
                q.writer[k] = rule if q.writer.get(k, rule) == rule else "mixed"
            q.sentences = max(q.sentences, g.sentences)
        q.input_drift = tuple(sorted(set().union(*(g.input_drift for g in group)), key=str))
        if any(g.cleared != base.cleared for g in group[1:]):
            q.cleared = None
        q.steps = max(g.steps for g in group)
        q.depth = max(g.depth for g in group)
        # settle merges recur every round, so a sum would count the shared history again each time: the most
        q.reentries = max(g.reentries for g in group) if base.settle else sum(g.reentries for g in group)
        q.next_sid = max(g.next_sid for g in group)
        q.merged = max(g.merged for g in group) + 1
        q.joined, q.resumed = sig, True
        self._merges += len(group) - 1
        if sig[0] == "round":  # a round's start: its fixed point and cycle tests compare with the merged state
            self._settle_merges += len(group) - 1
            self._value_merges += 0 if same else len(group) - 1
            q.round_start = dict(q.writes)
            q.round_slots = hash(self._slots_sig(q))
            q.history[-1] = (self._state_key(q), dict(q.writes))
        return q

    def _advance(self, p: Path, oracle: Oracle) -> list:
        top = p.stack[-1] if p.stack else None
        if top is None:
            self._halt(p, "end", "nothing left to run")
            return []
        if isinstance(top, Call):
            return self._call_step(p, top, oracle)
        return self._step(p, oracle)

    # --- reading -------------------------------------------------------------------------------------------

    def read(self, p: Path, key: str, oracle: Oracle):
        """The path's own write first, else the oracle's. In a settle round an engine variable that changes as
        frames pass (TIME_VARS) is unknown unless the path wrote it, and the running slot's read is logged
        (order conflicts) unless it wrote the key first."""
        if key in p.writes:
            v = p.writes[key]
        elif p.round and key in TIME_KEYS:
            v = Unknown(R_TIMER, f"{key}: an engine variable that changes as frames pass")
        else:
            v = oracle.get(key)
        if p.owner is not None:
            r, w = p.rw[p.owner]
            if key not in w:
                r.add(key)
        return v

    def write(self, p: Path, key: str, value) -> None:
        p.writes[key] = value
        if p.owner is not None:
            p.rw[p.owner][1].add(key)
            p.writer[key] = p.owner_rule

    def local(self, p: Path, n: int):
        if n in p.frame.locals:
            return p.frame.locals[n]
        return 0 if p.frame.args_known else Unknown(R_LOCAL, f"Local[{n}]")

    def val(self, p: Path, e, oracle: Oracle, x=None):
        match e:
            case Const(v):
                return v
            case Var(n):
                return self.read(p, f"var:{n}", oracle)
            case Bit(n):
                return self.read(p, f"bit:{n}", oracle)
            case Local(n):
                return self.local(p, n)
            case Index(kind, base, off):
                o = self.val(p, off, oracle, x)
                if isinstance(o, Unknown):
                    return Unknown(R_WORK, f"index of {show(e)} ({o})")
                return self.local(p, base + o) if kind == "local" else self.read(p, f"{kind}:{base + o}", oracle)
            case BinOp(op, a, b):
                return self._arith_at(p, op, self.val(p, a, oracle, x), self.val(p, b, oracle, x), x)
            case Neg(a):
                v = self.val(p, a, oracle, x)
                return v if isinstance(v, Unknown) else i32(-v)
            case Getter(name, args):
                return self.getter(p, name, args, oracle, x)
            case Embed(st):  # an opcode inside an expression: it writes var 0, its value is var 0
                self.assign(p, st, oracle, x)
                return self.val(p, st.lhs, oracle, x)
        return Unknown(R_WORK, show(e) if not isinstance(e, (Str, Concat, List)) else "text")

    def test(self, p: Path, c, oracle: Oracle, x=None):
        """A printed condition: True, False or Unknown. Comparisons cast to int16 (script_v5.cpp:
        1532-1677); the zero tests read the full value (o5_notEqualZero, o5_equalZero)."""
        match c:
            case Cmp(op, a, b):
                va, vb = self.val(p, a, oracle, x), self.val(p, b, oracle, x)
                return _decide(lambda y, z: CMP[op](i16(y), i16(z)), va, vb)
            case Not(a):
                return _decide(lambda y: y == 0, self.val(p, a, oracle, x))
            case Truth(a):
                return _decide(lambda y: y != 0, self.val(p, a, oracle, x))
        return Unknown(R_WORK, f"condition {c!r}")

    def where(self, p: Path, obj: int, oracle: Oracle):
        """whereIsObject (object.cpp:355-380): "inventory" when its owner is not the room (the
        inventory list is taken from the owner: an owner of 0 is nowhere), "room" when it is the
        room's and defined in the room resource in use, else "nowhere"."""
        owner = self.read(p, f"owner:{obj}", oracle)
        if isinstance(owner, Unknown):
            return owner
        if owner != OF_OWNER_ROOM:
            return "inventory" if owner else "nowhere"
        res = self.game.resource(self.read(p, "room", oracle))
        if isinstance(res, Unknown):
            return res
        return "room" if self.game.object_room.get(obj) == res else "nowhere"

    def getter(self, p: Path, name: str, args: tuple, oracle: Oracle, x=None):
        if name == "findInventory" and self.calls:
            return self._find_inventory(p, args, oracle, x)
        kind = GETTERS.get(name, (R_WORK, ""))[0]
        if kind != "fact":
            return Unknown(kind, name)
        vals = [self.val(p, a, oracle, x) for a in args if not isinstance(a, List)]
        if bad := next((v for v in vals if isinstance(v, Unknown) and v.values is None), None):
            return bad
        if sets := [v for v in vals if isinstance(v, Unknown)]:  # value sets: the getter on each combination
            combos = [()]
            for v in vals:
                combos = [c + (y,) for c in combos for y in values_of(v)]
                if len(combos) > MAX_VALUES:
                    VALUE_CAPS["getter"] += 1
                    return Unknown(sets[0].reason, f"{name} of {sets[0].what}")
            out = {self._getter_on(p, name, args, list(c), oracle, x) for c in combos}
            if bad := next((v for v in out if isinstance(v, Unknown)), None):
                return bad
            return one_of(out, sets[0].reason, f"{name} of {sets[0].what}")
        return self._getter_on(p, name, args, vals, oracle, x)

    def _find_inventory(self, p: Path, args: tuple, oracle: Oracle, x=None):
        """findInventory(owner, n) (object.cpp:77-85, calls on): the n-th object of the owner's in the
        inventory list, 0 when n is not in 1..count. The list's order is not a fact, so for n in range
        it is one of the objects the owner facts give the owner (the engine walks its inventory list:
        a note, as getInventoryCount)."""
        owner, n = (self.val(p, a, oracle, x) for a in args)
        if isinstance(owner, Unknown) or isinstance(n, Unknown):
            return Unknown(R_INVENTORY, f"findInventory({owner},{n})")
        owned = self._owned_by(p, owner, oracle)
        if isinstance(owned, tuple):
            k, v = owned
            return v if k is None else Unknown(R_INVENTORY, f"findInventory({owner},{n}): {k} {v}")
        self._note(p, x, "findInventory", "the owner's objects from owner facts; the engine walks its inventory list")
        if not 1 <= n <= len(owned):
            return 0
        return one_of(owned, R_INVENTORY, f"findInventory({owner},{n})")

    def _owned_by(self, p: Path, owner: int, oracle: Oracle):
        """The objects whose owner fact is ``owner`` (the path's writes first), or (key, unknown) for the first
        owner fact, in key order, whose value is not known ((None, unknown) when the oracle cannot list them)."""
        keys = oracle.keys("owner:")
        if keys is None:
            return (None, Unknown(R_HELD, "the owner facts"))
        if not (isinstance(oracle, FactOracle) and oracle.tracks("owner")):
            owned = []
            for k in sorted(set(keys) | {k for k in p.writes if k.startswith("owner:")}):
                v = self.read(p, k, oracle)
                if isinstance(v, Unknown):
                    return (k, v)
                if v == owner:
                    owned.append(int(k[6:]))
            return owned
        written = sorted(k for k in p.writes if k.startswith("owner:"))
        objs = set(oracle.owned().get(owner, ()))
        for k in written:
            v = self.read(p, k, oracle)
            if isinstance(v, Unknown):
                return (k, v)
            (objs.add if v == owner else objs.discard)(int(k[6:]))
        return sorted(objs)

    def _getter_on(self, p: Path, name: str, args: tuple, vals: list, oracle: Oracle, x=None):
        """A fact getter on known argument values."""
        if name == "getObjectOwner":
            return self.read(p, f"owner:{vals[0]}", oracle)
        if name == "getObjectState":
            return self.read(p, f"state:{vals[0]}", oracle)
        if name == "isScriptRunning":
            return 0 if vals[0] == 0 else self.read(p, self.game.script_key(vals[0]), oracle)
        if name == "classOfIs":
            obj, unknown = vals[0], None
            for c in args[1].items:
                cv = self.val(p, c, oracle, x)
                if isinstance(cv, Unknown) or not cv & 0x7F:
                    unknown = cv if isinstance(cv, Unknown) else Unknown(R_WORK, "class 0")
                    continue
                has = self.read(p, f"class:{obj}:{cv & 0x7F}", oracle)
                if isinstance(has, Unknown):
                    unknown = has
                elif bool(has) != bool(cv & 0x80):
                    return 0  # one class that does not match decides
            return unknown if unknown is not None else 1
        if name == "getInventoryCount":
            owned = self._owned_by(p, vals[0], oracle)
            if isinstance(owned, tuple):
                return owned[1]
            self._note(p, x, "getInventoryCount", "counted from owner facts; the engine counts its inventory list")
            return len(owned)
        if name == "getVerbEntryPoint":
            obj, verb = vals
            w = self.where(p, obj, oracle)
            if isinstance(w, Unknown):
                return w
            idx = self.game.entry(obj, verb)
            if w == "nowhere" or idx is None:
                return 0
            return self.game.verb_offset[obj] + self.game.units[("obj", obj)].ins[idx].offset
        return Unknown(R_WORK, name)

    # --- writing ------------------------------------------------------------------------------------------

    def target(self, p: Path, lhs, oracle: Oracle, x=None):
        """("local", n), (fact key) or Unknown: getResultPos resolves the target before the value
        (script_v5.cpp:383-400)."""
        match lhs:
            case Var(n):
                return f"var:{n}"
            case Bit(n):
                return f"bit:{n}"
            case Local(n):
                return ("local", n)
            case Index(kind, base, off):
                o = self.val(p, off, oracle, x)
                if isinstance(o, Unknown):
                    return Unknown(R_WORK, f"index of {show(lhs)} ({o})")
                return ("local", base + o) if kind == "local" else f"{kind}:{base + o}"
        return Unknown(R_WORK, f"target {lhs!r}")

    def put(self, p: Path, t, v, x=None, head: str = "assign") -> None:
        if isinstance(t, Unknown):
            p.unknown_writes.append((head, x.cite() if x else "", str(t)))
        elif isinstance(t, tuple):
            p.frame.locals[t[1]] = v if isinstance(v, Unknown) else i32(v)
        elif t.startswith("bit:"):
            self.write(p, t, v if isinstance(v, Unknown) else int(v != 0))  # writeVar on a bit (script.cpp)
        else:
            self.write(p, t, v if isinstance(v, Unknown) else i32(v))
            if t == f"var:{VAR_EGO}":  # the dump's ego is _scummVars[VAR_EGO] (speedrun_state.cpp:474)
                self.write(p, "ego", p.writes[t])

    def clear(self, p: Path, prefix: str, x=None, head: str = "") -> None:
        """Every fact with this prefix that is not 0 goes to 0: a stopped slot's wait points, an override
        that no longer shows."""
        keys = self.oracle.keys(prefix)
        if keys is None:
            self._note(p, x, head, f"the oracle cannot list {prefix}* facts")
            keys = []
        for k in sorted(set(keys) | {k for k in p.writes if k.startswith(prefix)}):
            if self.read(p, k, self.oracle) != 0:
                self.write(p, k, 0)

    def stopped(self, p: Path, key: str, x=None, head: str = "") -> None:
        """A slot stops: its fact goes to 0, and so do its wait points and an override it had armed
        (the dump gives no esc: key for a dead slot); with calls on, its frozen: too."""
        self.write(p, key, 0)
        self.clear(p, f"at:{slot_name(key)}:", x, head)
        self.clear(p, f"esc:{slot_name(key)}:", x, head)
        if self.calls:
            self._set_freeze(p, slot_name(key), (0, True))
        p.slots.pop(slot_name(key), None)

    def assign(self, p: Path, st: Assign, oracle: Oracle, x=None) -> None:
        t = self.target(p, st.lhs, oracle, x)
        v = self.val(p, st.rhs, oracle, x)
        if st.op != "=":
            old = self.val(p, st.lhs, oracle, x) if not isinstance(t, Unknown) else t
            v = self._arith_at(p, {"+=": "+", "++": "+", "-=": "-", "--": "-", "*=": "*", "/=": "/"}[st.op], old, v, x)
        self.put(p, t, v, x, ir.head(st))

    def _arith_at(self, p: Path, op: str, a, b, x=None):
        """``_arith`` in a path: a division by a known 0 is the engine's error() (o5_divide and
        o5_expression, script_v5.cpp:993-994, 1176-1179), so with calls on the path ends there."""
        if op == "/" and b == 0 and not isinstance(b, Unknown) and self.calls:
            self._error(p, "divide by zero", x.cite() if x is not None else "", "an error() (script_v5.cpp:994, 1179)",
                        fatal=True)
        return _arith(op, a, b)

    # --- row 0's starts: recorded, not run ------------------------------------------------------------------

    def may_stop(self, p: Path, key: str, x, head: str) -> None:
        """Row 0: a start that may stop a running instance of slot ``key`` first (runScript and
        runObjectScript without R, script.cpp:65-66, 117-118): every ``at:`` and ``esc:`` key of the
        unit that is not 0 becomes unknown. The old instance's wait point and override are gone, and
        the new instance, which row 0 does not run, may stop at the same place (an armed override
        stopped this way is an engine error in v5, script.cpp:275, 309)."""
        name = slot_name(key)
        for prefix in (f"at:{name}:", f"esc:{name}:"):
            keys = self.oracle.keys(prefix)
            if keys is None:
                self._note(p, x, head, f"the oracle cannot list {prefix}* facts")
                keys = []
            for k in sorted(set(keys) | {k for k in p.writes if k.startswith(prefix)}):
                if self.read(p, k, self.oracle) != 0:
                    self.write(p, k, Unknown(R_WORK, f"{name} restarted by {head}; the new instance is not run"))

    def started(self, p: Path, key: str, recursive, x, head: str) -> None:
        """Row 0: a start of slot ``key``: its fact is 1 (row 0 does not run the body). Unless the start
        is known to be recursive, it may stop a running instance first (``may_stop``)."""
        if recursive is not True:
            self.may_stop(p, key, x, head)
        self.write(p, key, 1)

    def hook(self, p: Path, var: int, oracle: Oracle, x, head: str) -> None:
        """Row 0: a hook script: runScript(VAR(var), 0, 0) when the variable is non-zero (script.cpp:
        62-63), without R, so a running copy of the hook stops first, the running script too when it is
        the hook."""
        n = self.read(p, f"var:{var}", oracle)
        if isinstance(n, Unknown):
            p.unknown_writes.append((head, x.cite(), f"hook in var {var}: {n}"))
        elif n:
            self.started(p, self.game.script_key(n), False, x, head)
            if self._is_self(p, self.game.script_key(n)):
                self._end(p, "end", f"{x.cite()} (hook restarts the running script)", lifecycle=False)

    # --- one step --------------------------------------------------------------------------------------------

    def _note(self, p: Path, x, head: str, what: str) -> None:
        p.notes.append((head, x.cite() if x is not None and hasattr(x, "cite") else (x or ""), what))

    def _error(self, p: Path, what: str, cite: str, detail: str, fatal: bool = False) -> None:
        """An engine error() (``fatal``: the game aborts, so the path ends as ``error``, which run B never
        did: ``INFEASIBLE``) or warning() (the engine goes on; counted only)."""
        p.errors.append((what, cite, detail))
        if fatal and p.end is None:
            p.end, p.end_at = "error", f"{cite}: {what}"

    def _halt(self, p: Path, reason: str, at: str) -> None:
        """The path stops where it is, with no slot facts written: unmodelled, a limit, an engine
        error that aborts."""
        p.end, p.end_at = reason, at

    def _is_self(self, p: Path, key: str) -> bool:
        return key == lifecycle_key(p.frame.unit, self.game)

    def _end(self, p: Path, reason: str, at: str, lifecycle: bool = True, resume: int | None = None) -> None:
        """Row 0: end a path. ``lifecycle``: write the unit's own slot facts, not after an unmodelled
        statement or a limit. At a yield the slot is live and waits at ``resume`` (its only ``at:``
        key); once it ended or a room change killed it, the slot, its wait points and its override
        are gone."""
        p.end, p.end_at = reason, at
        if not lifecycle or reason not in ("yield", "end", "room"):
            return
        key, name = lifecycle_key(p.frame.unit, self.game), unit_name(p.frame.unit)
        self.clear(p, f"at:{name}:", None, "end")
        if reason == "yield":
            if key:
                self.write(p, key, 1)
            if resume is None:
                self._note(p, None, "yield", f"{at}: the resume offset is past the code; at: not predicted")
            else:
                self.write(p, f"at:{name}:{resume}", 1)
        else:
            if key:
                self.write(p, key, 0)
            self.clear(p, f"esc:{name}:", None, "end")

    def _done(self, p: Path, how: str, at: str, resume: int | None = None) -> None:
        """The running frame's turn ends (yield or end): row 0 ends the path, calls pops the frame."""
        if self.calls:
            self._finish(p, how, at, resume)
        else:
            self._end(p, how, at, resume=resume)

    def _running(self, p: Path, sid: int) -> Frame | None:
        """The frame ``sid`` when it is still the running one."""
        top = p.stack[-1] if p.stack and p.end is None else None
        return top if isinstance(top, Frame) and top.sid == sid else None

    def _step(self, p: Path, oracle: Oracle) -> list:
        """Run one instruction of the running frame; return the paths it forked off (``p`` goes on
        as one side). The frame's PC moves past the instruction first, as the engine's does while
        an opcode runs; jumps, yields and waits say where it goes instead."""
        f = p.frame
        u = self.game.units[f.unit]
        if f.pc is None or f.pc >= len(u.ins):
            self._done(p, "end", f"{u.file}: end of code")
            return []
        idx = f.pc
        x = u.ins[idx]
        p.steps += 1
        st = x.st
        if st is None:
            p.unmodelled.append(("unparsed", x.cite(), x.text))
            self._end(p, "unmodelled", x.cite(), lifecycle=False)
            return []
        f.pc = x.next
        if isinstance(st, Jump):
            return self._jump(p, x, st, oracle, idx)
        head = ir.head(st)
        more = []
        if isinstance(st, Assign):
            self.assign(p, st, oracle, x)
        elif isinstance(st, Start):
            more = self._start(p, x, st, oracle) or []
        else:
            effect = HEADS.get(head, ("unmodelled", "", "no rule"))[0]
            if effect == "yield":
                # after breakHere or a delay the slot resumes at the next instruction (facts.py at:)
                nxt = x.next if x.next is not None and x.next < len(u.ins) else None
                self._done(p, "yield", x.cite(), resume=u.ins[nxt].offset if nxt is not None else None)
            elif effect == "wait":
                more = self._wait(p, x, head)
            elif effect == "unmodelled":
                p.unmodelled.append((head, x.cite(), HEADS.get(head, ("", "", "no rule"))[2]))
                self._end(p, "unmodelled", x.cite(), lifecycle=False)
            elif effect in ("write", "end", "deferred"):
                more = getattr(self, "_op_" + st.name)(p, x, st, oracle) or []
            if effect == "end":
                for q in (p, *more):
                    if (self.calls and self._running(q, f.sid)) or (not self.calls and q.end is None):
                        self._done(q, "end", x.cite())
        if self.calls:
            for q in (p, *more):  # an op that stopped its own slot ends its turn (stopScript and the like)
                if (g := self._running(q, f.sid)) is not None and g.killed:
                    self._finish(q, g.killed, x.cite())
        return more

    def _jump(self, p: Path, x, st: Jump, oracle: Oracle, idx: int | None = None) -> list:
        if st.cond is None:
            p.frame.pc = x.target
            return []
        c = self.test(p, st.cond, oracle, x)
        f = p.frame
        if isinstance(c, Unknown) and idx is not None and (p.round or p.timed):
            exits = self.game.time_exits(f.unit, const_polls=self.const_polls)
            if f.time_loops and (ex := exits.get(idx)) is not None and ex[1] in f.time_loops:
                # settle: a loop whose exit tests read only non-facts, in which the slot waited before this turn,
                # is taken to exit when its test's value is not known: time is assumed to satisfy it. A known
                # value decides as usual (a counter the path set, a fact). Once per loop and turn: met again in
                # the same turn (an outer loop came round with no yield) its test forks as usual.
                out, head = ex
                f.time_loops = tuple(h for h in f.time_loops if h != head)
                p.assumptions.append((x.cite(), show(st.cond), "exit",
                                      str(Unknown(R_LOOP_EXIT, f"{c.reason}; {show(st.cond)}"))))
                f.pc = out
                return []
            if (ex := self.game.time_exits(f.unit, camera=True, const_polls=self.const_polls).get(idx)) is not None:
                # settle: an exit test on the camera position. The camera moves each frame toward its
                # destination, a followed actor or a pan's target (moveCamera, camera.cpp:93-163), so time may
                # bring it there or not: the first such test of the slot's loop forks (one side leaves, the
                # camera got there; the other stays), and on the side that stayed later tests of the loop stay
                # with no fork (it never gets there in this settle). Counted (R_CAMERA).
                out, head = ex
                stay = x.target if out == x.next else x.next
                key = (unit_name(f.unit), head)
                if key in p.camera_done:
                    p.assumptions.append((x.cite(), show(st.cond), "stay",
                                          str(Unknown(R_CAMERA, f"stays, forked once; {show(st.cond)}"))))
                    f.pc = stay
                    return []
                q = p.fork()
                p.camera_done.add(key)
                p.assumptions.append((x.cite(), show(st.cond), "stay", str(Unknown(R_CAMERA, f"{c.reason}; "
                                                                                    f"{show(st.cond)}"))))
                q.assumptions.append((x.cite(), show(st.cond), "exit", str(Unknown(R_CAMERA, f"{c.reason}; "
                                                                                    f"{show(st.cond)}"))))
                p.frame.pc, q.frame.pc = stay, out
                return [q]
        if not isinstance(c, Unknown):
            p.frame.pc = x.next if c else x.target  # o5 jumps when the condition is false
            return []
        q = p.fork()
        p.assumptions.append((x.cite(), show(st.cond), True, str(c)))
        q.assumptions.append((x.cite(), show(st.cond), False, str(c)))
        p.frame.pc, q.frame.pc = x.next, x.target
        return [q]

    def _wait(self, p: Path, x, head: str) -> list:
        """o5_wait (script_v5.cpp:3272-3310): when it blocks, the script backs up to the wait and
        yields, so it re-runs the wait next frame (its ``at:`` key is the wait itself, facts.py);
        otherwise it falls through. What it tests (WAITS) is no fact, so both sides run, each with
        the assumption; the side that falls through goes on at the next instruction."""
        reason, what = WAITS[head]
        if (p.round or p.timed) and head != "WaitForSentence":
            # settle: a wait for an actor, a message or the camera is a time-only wait: it blocks now and falls
            # through when the slot resumes in a later round (``_eng_turn``)
            self._done(p, "yield", x.cite(), resume=x.offset)
            return []
        if p.round or p.timed:  # WaitForSentence reads the queue and the sentence script's slot (script_v5.cpp:3296-3302)
            busy = self._sentence_busy(p)
            if not isinstance(busy, Unknown):
                if busy:
                    self._done(p, "yield", x.cite(), resume=x.offset)
                return []
        q = p.fork()
        cond = f"{x.text.strip()} blocks"
        why = str(Unknown(reason, what))
        p.assumptions.append((x.cite(), cond, True, why))
        q.assumptions.append((x.cite(), cond, False, why))
        self._done(p, "yield", x.cite(), resume=x.offset)
        return [q]

    # --- calls: frames, nested runs and the resume test --------------------------------------------------------

    def _frames(self, p: Path) -> list:
        return [e for e in p.stack if isinstance(e, Frame)]

    def _frame_of(self, p: Path, sid: int | None) -> Frame | None:
        return next((e for e in p.stack if isinstance(e, Frame) and e.sid == sid), None) if sid else None

    def _room_slot(self, f: Frame):
        """Whether a room change kills the slot: room code, local scripts and room or flobject object
        slots (script.cpp:1083-1105); True, False or Unknown (an object slot of unknown kind)."""
        if f.unit[0] in ROOM_KILLED:
            return True
        if f.unit[0] == "obj":
            return f.owned if isinstance(f.owned, Unknown) else not f.owned
        return False

    def _finish(self, p: Path, how: str, at: str, resume: int | None = None) -> None:
        """The running frame's turn is over (calls). ``how``: yield (the slot stays live and waits at
        ``resume``), parked (live, waiting right after the opcode that ran a script nested, its
        freeze count kept), end (the slot dies: an object slot's stopObjectCode stops every slot of
        the object, script.cpp:873-875), or stopped / chained / room (it died earlier; the stop wrote
        its facts). Then runScriptNested's resume test runs for the call below (script.cpp:377-390)."""
        f = p.stack.pop()
        name, key = unit_name(f.unit), lifecycle_key(f.unit, self.game)
        if how in ("yield", "parked"):
            if key:
                self.write(p, key, 1)
            if how == "parked":
                u = self.game.units[f.unit]
                resume = u.ins[f.pc].offset if f.pc is not None and f.pc < len(u.ins) else None
            if resume is None:
                self._note(p, None, how, f"{at}: the resume offset is past the code; at: not predicted")
            else:
                self.write(p, f"at:{name}:{resume}", 1)
            self._set_freeze(p, name, (f.freeze, True))
            if p.settle and any(unit_name(g.unit) == name and not g.killed for g in self._frames(p)):
                p.multi.add(name)  # a recursive start's instance stays live while another runs on the stack
            if p.settle:  # settle resumes it from here with the locals it has (its freeze count is in p.freeze)
                idx = self.game.units[f.unit].at.get(resume) if resume is not None else None
                if idx is None:
                    p.slots.pop(name, None)
                else:
                    p.slots[name] = dataclasses.replace(f, pc=idx, locals=dict(f.locals), killed=None, nested=False,
                                                        time_loops=())
        elif how == "end":
            if f.unit[0] == "obj":
                self._kill_object(p, f.unit[1], at, "stopObjectCode")
            else:
                if f.override:
                    self._error(p, "ended with an open cutscene", at, f"{name}: stopObjectCode's error() (script.cpp:"
                                f"879-880)", fatal=True)
                if key:
                    self.write(p, key, 0)
                self.clear(p, f"esc:{name}:", None, "end")
                self._set_freeze(p, name, (0, True))
                p.slots.pop(name, None)
                if f.recursive:
                    self._note(p, None, "end", f"{name} ended; another instance started with R may still run")
        p.turns.append((name, how))
        top = p.stack[-1] if p.stack else None
        if top is None or (isinstance(top, Call) and top.caller is None and len(p.stack) == 1):
            p.last = f
        if top is None:
            if p.end is None:
                p.end, p.end_at = how, at
        elif isinstance(top, Call) and top.active:
            caller = self._frame_of(p, top.caller)
            if caller is None or caller.killed or caller.freeze > 0:
                top.active = False

    def _call_step(self, p: Path, call: Call, oracle: Oracle) -> list:
        """Run the call's next step; at its end, pop it and settle its caller: an active one goes on,
        a dead one's turn ends as it died, a live one that may not resume is parked."""
        if call.i < len(call.steps):
            step = call.steps[call.i]
            call.i += 1
            return getattr(self, "_eng_" + step[0])(p, call, oracle, *step[1:]) or []
        p.stack.pop()
        if not p.stack and p.settle and p.end is None:  # the frame is over: the next settle round, or the end
            self._next_round(p)
            return []
        if not p.stack:  # the engine's own call: the first script's turn is the run's end
            if p.end is None:
                p.end, p.end_at = (p.turns[-1][1], call.cite) if p.turns else ("nothing ran", call.cite)
            return []
        top = p.stack[-1]
        if isinstance(top, Call):  # a routine inside an opcode: whether the caller resumes goes up
            top.active = top.active and call.active
            return []
        if call.active and not top.killed and top.freeze == 0:
            return []
        self._finish(p, top.killed or "parked", call.cite)
        return []

    def _push_call(self, p: Path, head: str, cite: str, steps: tuple, active: bool = True) -> None:
        p.stack.append(Call(head, cite, steps, caller=p.frame.sid if p.stack else None, active=active))

    def _push_frame(self, p: Path, call: Call, unit: tuple, pc: int, locals_: dict, owned, recursive, resistant,
                    key: str | None, args_known: bool = True, nested: bool = True) -> None:
        depth = sum(1 for f in self._frames(p) if f.nested)  # vm.numNestedScripts before this run
        if nested and depth >= MAX_NESTING:
            self._error(p, "nesting", call.cite, f"Too many nested scripts starting {unit_name(unit)} "
                        f"(script.cpp:351-352)")
            self._halt(p, "nesting", call.cite)
            return
        if any(f.unit == unit for f in self._frames(p)):
            p.reentries += 1
        f = Frame(unit, pc, dict(locals_), args_known, owned, recursive, resistant, sid=p.next_sid, nested=nested)
        p.next_sid += 1
        p.stack.append(f)
        if p.settle and (p.round or p.timed):  # it executes now: no turn of its own later in this frame (didexec,
            p.round_ran.add(unit_name(unit))   # script.cpp:506-507, 979)
        p.depth = max(p.depth, len(self._frames(p)))
        if key:
            self.write(p, key, 1)

    def _eng_script(self, p: Path, call: Call, oracle: Oracle, n, resistant, recursive, locals_: dict) -> None:
        """runScript(N, F, R, args) (script.cpp:38-107): N = 0 does nothing; without R it stops N's
        slots first; N below _numGlobalScripts is global, else a local script of the room in use (one
        the room lacks is the engine's error(), script.cpp:80-81: the path ends)."""
        if isinstance(n, Unknown):
            p.unknown_writes.append((call.head, call.cite, f"script {n}"))
            return
        if n == 0:
            return
        key = self.game.script_key(n)
        if not recursive:
            self._stop_script(p, n, call.cite, call.head)
        elif p.settle and p.writes.get(key, oracle.get(key)) != 0 \
                and not any(unit_name(f.unit) == slot_name(key) and not f.killed for f in self._frames(p)):
            # settle: a recursive start leaves a live instance running (runScript with R, script.cpp:65-66), so two
            # slots run the unit. The path keeps one Frame and one set of at: keys per unit name, so the unit's
            # at: keys, and its slot fact once the tracked instance ends, are unknown when settle ends
            # (``_close_settle``); counted. An instance on the stack is the running one, not a second live slot:
            # that case counts only if the new one yields while it is still there (``_finish``)
            p.multi.add(slot_name(key))
        if key.startswith("local:"):
            res = self.game.resource(self.read(p, "room", oracle))
            if isinstance(res, Unknown):
                p.unknown_writes.append((call.head, call.cite, f"local script {n} of room {res}"))
                self.write(p, key, Unknown(R_WORK, f"local script {n} of room {res}"))
                return
            unit = ("local", res, n)
        else:
            unit = ("global", n)
        if unit not in self.game.units:
            self._error(p, "no such script", call.cite, f"{unit_name(unit)} is not in room {unit[1]} "
                        f"(script.cpp:80-81, an error())" if unit[0] == "local" else f"no global script {n}",
                        fatal=True)
            return
        self._push_frame(p, call, unit, 0, locals_, False, recursive, resistant, key)

    def _eng_object(self, p: Path, call: Call, oracle: Oracle, obj, verb, locals_: dict) -> None:
        """runObjectScript(O, verb, 0, 0, args) (script.cpp:109-154): O = 0 does nothing; never with R
        in v5, so it stops O's slots first; an object nowhere starts nothing (a warning, counted); one
        with no entry for the verb or 0xFF starts nothing; else its entry runs in a slot of the kind
        whereIsObject gives (an inventory slot outlives a room change)."""
        if obj == 0:
            return
        self._kill_object(p, obj, call.cite, call.head)
        w = self.where(p, obj, oracle)
        if isinstance(w, Unknown):
            p.unknown_writes.append((call.head, call.cite, f"objscript of {obj} ({w})"))
            self.write(p, f"objscript:{obj}", w)
            return
        if w == "nowhere":
            self._error(p, "object not here", call.cite, f"Code for object {obj} not in room (script.cpp:122-125, "
                        f"a warning)")
            return
        idx = self.game.entry(obj, verb)
        if idx is None:
            return
        self._push_frame(p, call, ("obj", obj), idx, locals_, w == "inventory", False, False, f"objscript:{obj}")

    def _eng_hook(self, p: Path, call: Call, oracle: Oracle, var: int, locals_: dict) -> None:
        """runScript(VAR(var), 0, 0, args) when the variable is not 0 (script.cpp:62-63)."""
        n = self.read(p, f"var:{var}", oracle)
        if isinstance(n, Unknown):
            p.unknown_writes.append((call.head, call.cite, f"hook in var {var}: {n}"))
        elif n:
            self._eng_script(p, call, oracle, n, False, False, locals_)

    def _eng_resume(self, p: Path, call: Call, oracle: Oracle, key: str) -> list:
        """runAllScripts (script.cpp:968-987), which comes before checkExecVerbs and
        checkAndRunSentenceScript in the click's frame (scumm.cpp:3231-3232, 3255; ``pre`` is the
        frame before's decision point, groundtruth.py): the live, unfrozen slot ``key`` has its turn
        from where it stopped, its one ``at:`` key (after a breakHere, right after an opcode that
        ran a script nested, or at a WaitFor* op, facts.py). A slot stopped by a delay is ssPaused
        and runs only once decreaseScriptDelay ends the delay, a timer (script_v5.cpp:972-987,
        script.cpp:1513-1530, scumm.cpp:3132): a fork, the paused side has no turn. Its locals and R
        flag are not facts (unknown), its F flag comes from the start sites (``Game.resistant``), its
        freeze count is 0, and runAllScripts runs it with executeScript, not nested (script.cpp:
        980-983). A slot with no ``at:`` key, more than one, or one that is no instruction of its unit
        ends the path as "deferral unknown"."""
        name = slot_name(key)
        res = self.game.resource(self.read(p, "room", oracle))
        if key.startswith("script:"):
            unit = ("global", int(key[7:]))
        elif key.startswith("local:"):
            unit = None if isinstance(res, Unknown) else ("local", res, int(key[6:]))
        else:
            unit = ("obj", int(key[10:]))
        u = self.game.units.get(unit) if unit else None
        keys = self.oracle.keys(f"at:{name}:")
        waits = sorted(k for k in (keys or []) if self.read(p, k, oracle) == 1)
        idx = u.at.get(int(waits[0].rsplit(":", 1)[1])) if u is not None and len(waits) == 1 else None
        if idx is None:
            self._halt(p, "deferral unknown", f"{name} is live and not frozen, and its turn cannot be run (unit "
                       f"{unit}, wait points {waits if keys is not None else 'not listed'})")
            return []
        owned = False
        if unit[0] == "obj":
            owner = self.read(p, f"owner:{unit[1]}", oracle)
            owned = owner if isinstance(owner, Unknown) else owner != OF_OWNER_ROOM
        more = []
        prev = u.ins[idx - 1] if idx > 0 else None
        if prev is not None and prev.next == idx and prev.st is not None \
                and ir.head(prev.st) in ("delay", "delayVariable"):
            q = p.fork()
            why = str(Unknown(R_TIMER, f"{name}'s delay count"))
            p.assumptions.append((prev.cite(), f"{name}'s delay is over", True, why))
            q.assumptions.append((prev.cite(), f"{name}'s delay is over", False, why))
            more = [q]  # still paused: no turn this frame
        if p.settle:  # settle: its turn is one of the click frame's runAllScripts turns (order conflicts), with
            # rows 1-2's rules for what it meets (a WaitFor op forks)
            p.owner, p.owner_rule, p.timed = name, "a: started or resumed by the path", False
            p.rw.setdefault(name, (set(), set()))
            p.round_ran.add(name)
        self.clear(p, f"at:{name}:", None, "resume")  # it runs from its wait point now
        resistant = self.game.resistant(key, res)
        if resistant is None:
            self._note(p, call.cite, "resume", f"{name}: F flag not known")
        self._push_frame(p, call, unit, idx, {}, owned, None, resistant, key, args_known=False, nested=False)
        return more

    def _eng_sentence(self, p: Path, call: Call, oracle: Oracle, verb: int, a: int, b) -> None:
        """checkAndRunSentenceScript (script.cpp:1174-1247), on the path's own store after the turns
        before it: a slot of the sentence script's number that is live and not frozen holds the
        sentence back (script.cpp:1186-1191): the path ends "deferred" (with settle the sentence stays
        in the path's queue, ``_eng_sentences``, and the path goes on); the queued sentence's own
        freeze count is 0, as doSentence sets it (script.cpp:1171); the MI1 note workaround
        (script.cpp:1199, 1275-1297) is an enhancement and off; a sentence with an object B equal to
        A is dropped (script.cpp:1203-1204); VAR_SENTENCE_SCRIPT 0 runs nothing (script.cpp:1245).
        Otherwise runScript(VAR_SENTENCE_SCRIPT, 0, 0, [verb, A, B]) with no caller to resume
        (``_currentScript = 0xFF``, script.cpp:1244-1246): not freeze resistant, not recursive, so
        it stops a (frozen) running copy first."""
        p.owner, p.timed = None, False  # settle: the click's sentence runs after every turn, with rows 1-2's rules
        n = self.read(p, f"var:{VAR_SENTENCE_SCRIPT}", oracle)
        if isinstance(n, Unknown):
            self._halt(p, "deferral unknown", f"VAR_SENTENCE_SCRIPT {n}")
            return
        if n == 0:
            if p.settle:  # time still passes (settle); the immediate part ran nothing
                p.first_end = "no sentence script"
                return
            self._halt(p, "no sentence script", "VAR_SENTENCE_SCRIPT is 0: nothing runs (script.cpp:1245)")
            return
        busy, unknown = self._sentence_waits(p, n, oracle)
        if busy and p.settle:  # it stays queued; a later frame's checkAndRunSentenceScript runs it (settle)
            p.queue.append((verb, a, b or 0, 0))
            p.first_end = "deferred"
        elif busy:
            self._halt(p, "deferred", f"{', '.join(busy)} live and not frozen: the sentence waits "
                                      f"(script.cpp:1186-1191)")
        elif unknown:
            self._halt(p, "deferral unknown", f"whether the sentence waits: {'; '.join(unknown)}")
        elif b and b == a and p.settle:
            p.first_end = "dropped"
        elif b and b == a:
            self._halt(p, "dropped", "object B is object A (script.cpp:1203-1204)")
        else:
            self._eng_script(p, call, oracle, n, False, False, {0: verb, 1: a, 2: b or 0})

    def _sentence_waits(self, p: Path, n: int, oracle: Oracle) -> tuple[list, list]:
        """checkAndRunSentenceScript's first test (script.cpp:1186-1191): the slots of the sentence script's
        number that are live and not frozen (the sentence waits), and those whose state is not known."""
        busy, unknown = [], []
        for k in self._sentence_slots(n):
            live, frozen = self.read(p, k, oracle), self.read(p, f"frozen:{slot_name(k)}", oracle)
            if live == 0:
                continue
            if isinstance(live, Unknown) or isinstance(frozen, Unknown):
                unknown.append(f"{k} {live}, frozen {frozen}")
            elif not frozen:
                busy.append(k)
        return busy, unknown

    def _eng_cut_index(self, p: Path, call: Call, oracle: Oracle, sid) -> None:
        p.cut_index = sid

    def _eng_write(self, p: Path, call: Call, oracle: Oracle, key: str, value) -> None:
        self.write(p, key, value)

    def _eng_note(self, p: Path, call: Call, oracle: Oracle, what: str) -> None:
        self._note(p, call.cite, call.head, what)

    def _eng_scroll_note(self, p: Path, call: Call, oracle: Oracle) -> None:
        """setCameraFollows calls setCameraAt, and so the scroll hook (camera.cpp:52-55), when the actor
        stands outside the camera's triggers (camera.cpp:76-79): a position, not a fact, so a note. (After
        a room change it also calls it unconditionally, camera.cpp:69-74: a hook step of its own.)"""
        n = self.read(p, f"var:{VAR_SCROLL_SCRIPT}", oracle)
        if n != 0:
            self._note(p, call.cite, call.head, f"the scroll hook ({n}) may run: the camera position is not a fact")

    # --- calls: stops, kills and freezes ---------------------------------------------------------------------------

    def _stop_script(self, p: Path, n, cite: str, head: str) -> None:
        """stopScript(N) (script.cpp:262-293): every global or local slot of N dies, frames on the stack
        too; one with an open cutscene is the engine's error() (script.cpp:273-275: the path ends;
        cutsceneOverride, which only beginCutscene raises, script.cpp:1634, is known for frames only)."""
        if n == 0:
            return
        key = self.game.script_key(n)
        name = slot_name(key)
        for f in self._frames(p):
            if unit_name(f.unit) == name and f.unit[0] in ("global", "local") and not f.killed:
                if f.override:
                    self._error(p, "stopped with an open cutscene", cite, f"{name} by {head} (script.cpp:273-275, "
                                f"an error())", fatal=True)
                f.killed = "stopped"
        self.stopped(p, key, cite, head)

    def _kill_object(self, p: Path, obj: int, cite: str, head: str) -> None:
        """stopObjectScript(O) (script.cpp:296-327): every object slot of O dies (an open cutscene or
        override there is the engine's error(), script.cpp:307-309: the path ends)."""
        name = f"obj:{obj}"
        for f in self._frames(p):
            if f.unit == ("obj", obj) and not f.killed:
                if f.override:
                    self._error(p, "stopped with an open cutscene", cite, f"{name} by {head} (script.cpp:307-309, "
                                f"an error())", fatal=True)
                f.killed = "stopped"
        self.stopped(p, f"objscript:{obj}", cite, head)

    def _live_slots(self, p: Path) -> dict:
        """The live slots off the stack, by unit name: {name: (slot fact or None, liveness)}. Room
        entry and exit code have no slot fact; each is live when one of its ``at:entry`` / ``at:exit``
        keys holds (an exit slot is live only when this path ran it to a yield: facts.py ``at:``)."""
        out = {}
        for prefix in ("script:", "local:", "objscript:"):
            keys = set(self.oracle.keys(prefix) or []) | {k for k in p.writes if k.startswith(prefix)}
            for k in sorted(keys):  # in a fixed order: the notes and errors it leads to are listed in it
                v = self.read(p, k, self.oracle)
                if v != 0:
                    out[slot_name(k)] = (k, v)
        for code in ("entry", "exit"):
            keys = set(self.oracle.keys(f"at:{code}:") or []) | {k for k in p.writes if k.startswith(f"at:{code}:")}
            vals = [self.read(p, k, self.oracle) for k in sorted(keys)]
            if any(v == 1 for v in vals):
                out[code] = (None, 1)
            elif any(isinstance(v, Unknown) for v in vals):
                out[code] = (None, next(v for v in vals if isinstance(v, Unknown)))
        on_stack = {unit_name(f.unit) for f in self._frames(p)}
        return {n: kv for n, kv in out.items() if n not in on_stack}

    def _count(self, p: Path, name: str):
        """A slot's freeze count off the stack: (count, exact), or None when not known. A slot the
        path has not touched has ``frozen:`` 0 (count 0) or 1 (at least 1)."""
        if name in p.freeze:
            return p.freeze[name]
        v = self.read(p, f"frozen:{name}", self.oracle)
        if isinstance(v, Unknown):
            return None
        return (1, False) if v else (0, True)

    def _set_freeze(self, p: Path, name: str, n, why: str = "") -> None:
        """Set a slot's freeze count off the stack and its ``frozen:`` fact (1 above 0, 0 at an exact
        0, else unknown)."""
        p.freeze[name] = n
        if n is None:
            v = Unknown(R_FREEZE, why or name)
        elif n[0] > 0:
            v = 1
        elif n[1]:
            v = 0
        else:
            v = Unknown(R_FREEZE, f"{name}: frozen before the move, then unfrozen once; its count is not a fact")
        if not (v == 0 and self.read(p, f"frozen:{name}", self.oracle) == 0):
            self.write(p, f"frozen:{name}", v)

    def _freeze(self, p: Path, flag: int, x) -> None:
        """freezeScripts(flag) (script.cpp:914-941, the v5 branch): every live slot but the running one
        gets 1 more on its count, a freeze-resistant one only from flag 0x80 on; then beginCutscene's
        caller, while its hook runs, goes back to 0. A slot off the stack takes its F flag from its kept
        Frame when the path ran it and knows it (settle), else from the start sites (``Game.resistant``).
        The queued sentences' counts are not facts (a note)."""
        cur = p.frame
        every = flag >= 0x80
        for f in self._frames(p):
            if f is not cur and (every or f.resistant is False):
                f.freeze += 1
            elif f is not cur and f.resistant is None:
                self._note(p, x, "freezeScripts", f"{unit_name(f.unit)}: F flag not known; taken as not frozen")
        for name, (key, live) in self._live_slots(p).items():
            kept = p.slots.get(name)  # settle: a slot off the stack that the path ran knows its own F flag
            room = None
            if every:
                res = False
            elif kept is not None and kept.resistant is not None:
                res = kept.resistant
            else:
                room = self.game.resource(self.read(p, "room", self.oracle)) if name.startswith("local:") else None
                res = self.game.resistant(key or name, room)
            if res is True:
                continue
            if res is None or isinstance(live, Unknown):
                why = f"live {live}" if res is not None else (f"room resource {room}" if isinstance(room, Unknown)
                                                              else "its F flag differs between start sites")
                self._set_freeze(p, name, None, f"{name}: {why}")
                continue
            if not every and key and key.startswith(("script:", "local:")) and not (kept is not None
                                                                                   and kept.resistant is not None):
                self._note(p, x, "assumption: F flag", f"{name}: F flag from the constant start sites "
                           f"({len(self.game.wild_starts)} starts with a variable number not counted)")
            n = self._count(p, name)
            self._set_freeze(p, name, (n[0] + 1, n[1]) if n else None, f"{name}: freeze count not known")
        if p.cut_index is not None and (f := self._frame_of(p, p.cut_index)) is not None:
            f.freeze = 0  # script.cpp:937-940
        if p.settle:  # every queued sentence's count goes up too (script.cpp:934-935)
            p.queue = [(v, a, b, n + 1) for v, a, b, n in p.queue]
        self._note(p, x, "freezeScripts", "the queued sentences' freeze counts are not facts")

    def _unfreeze(self, p: Path, x) -> None:
        """unfreezeScripts (script.cpp:943-966): every frozen slot loses 1 from its count."""
        for f in self._frames(p):
            if f.freeze > 0:
                f.freeze -= 1
        for name, (key, live) in self._live_slots(p).items():
            n = self._count(p, name)
            if n is None:
                continue
            if n[0] > 0:
                self._set_freeze(p, name, (n[0] - 1, n[1]))
        if p.settle:  # and every queued sentence's count above 0 goes down (script.cpp:962-965)
            p.queue = [(v, a, b, max(0, n - 1)) for v, a, b, n in p.queue]

    def _kill_room(self, p: Path, call: Call) -> None:
        """killScriptsAndResources (script.cpp:1083-1105): every room, flobject and local slot dies,
        frames on the stack too; inventory and global slots live on. An open cutscene there is a
        warning, counted. An object slot off the stack is a room slot when its object is the room's
        now, an assumption the path records (its kind is not a fact)."""
        for f in self._frames(p):
            kind = self._room_slot(f)
            if isinstance(kind, Unknown):
                self._note(p, call.cite, call.head, f"{unit_name(f.unit)}: slot kind {kind}; taken as surviving")
            elif kind and not f.killed:
                if f.override:
                    self._error(p, "killed with an open cutscene", call.cite, f"{unit_name(f.unit)} (a warning, "
                                f"script.cpp:1089-1093)")
                f.killed = "room"
                self._gone(p, f)
        for name, (key, live) in self._live_slots(p).items():
            if name.startswith("obj:"):
                owner = self.read(p, f"owner:{name[4:]}", self.oracle)
                if isinstance(owner, Unknown):
                    self.write(p, key, Unknown(R_WORK, f"{name}: slot kind not known ({owner})"))
                    continue
                p.assumptions.append((call.cite, f"{name} runs in a {'room' if owner == OF_OWNER_ROOM else 'inventory'}"
                                      f" slot (its owner is {owner})", True, "the slot kind is not a fact"))
                if owner != OF_OWNER_ROOM:
                    continue
            elif not name.startswith(("local:", "entry", "exit")):
                continue
            if key:
                self.write(p, key, 0)
            self.clear(p, f"at:{name}:", None, call.head)
            self.clear(p, f"esc:{name}:", None, call.head)
            self._set_freeze(p, name, (0, True))
            p.slots.pop(name, None)

    def _gone(self, p: Path, f: Frame) -> None:
        """A frame's slot died: its fact, wait points, override and frozen: go."""
        key, name = lifecycle_key(f.unit, self.game), unit_name(f.unit)
        if key:
            self.write(p, key, 0)
        self.clear(p, f"at:{name}:", None, "kill")
        self.clear(p, f"esc:{name}:", None, "kill")
        self._set_freeze(p, name, (0, True))
        p.slots.pop(name, None)

    # --- calls: the room change ---------------------------------------------------------------------------------

    def _eng_room1(self, p: Path, call: Call, oracle: Oracle, room) -> None:
        """Without room_change: the room fact, and the running slot ends when the change kills it (as
        row 0); the kills of other slots and the exit and entry scripts are left out (a note)."""
        self.write(p, "room", room)
        self._note(p, call.cite, call.head, "the room change's kills of other slots and its exit and entry scripts "
                   "are not run (feature room_change)")
        caller = self._frame_of(p, call.caller)
        if caller is None or not call.active:
            return
        kind = self._room_slot(caller)
        if isinstance(kind, Unknown):
            self._note(p, call.cite, call.head, f"slot kind unknown ({kind}); taken as surviving the room change")
        elif kind:
            caller.killed = "room"
            call.active = False
            self._gone(p, caller)

    def _eng_scene(self, p: Path, call: Call, oracle: Oracle, room, ego_obj) -> None:
        """startScene(room, ego, object) (room.cpp:42-260) as a routine of the opcode: its steps."""
        steps = [("scene_begin", room), ("hook", VAR_EXIT_SCRIPT, {}), ("room_code", "exit"),
                 ("hook", VAR_EXIT_SCRIPT2, {}), ("scene_kill",), ("scene_load", room)]
        if ego_obj is not None:
            steps.append(("scene_ego", ego_obj))
        steps += [("hook", VAR_ENTRY_SCRIPT, {}), ("room_code", "entry"), ("hook", VAR_ENTRY_SCRIPT2, {})]
        p.stack.append(Call("startScene", "room.cpp:42-260", tuple(steps), caller=call.caller, active=call.active))

    def _eng_scene_begin(self, p: Path, call: Call, oracle: Oracle, room) -> None:
        """room.cpp:53-76: stopTalk ends an actor's speech (setTalkingActor(0xFF) when VAR_TALK_ACTOR
        is an actor, actor.cpp:3573-3582); a running room, flobject or local slot no longer resumes
        (an open cutscene there is the engine's error(): the path ends); VAR_NEW_ROOM = room."""
        talk = self.read(p, f"var:{VAR_TALK_ACTOR}", oracle)
        if isinstance(talk, Unknown):
            self.write(p, f"var:{VAR_TALK_ACTOR}", Unknown(R_WORK, f"stopTalk with VAR_TALK_ACTOR {talk}"))
        elif 0 < talk < 0x80:
            self.write(p, f"var:{VAR_TALK_ACTOR}", 0xFF)
        caller = self._frame_of(p, call.caller)
        if call.active and caller is not None:
            kind = self._room_slot(caller)
            if isinstance(kind, Unknown):
                self._note(p, call.cite, call.head, f"slot kind unknown ({kind}); taken as surviving the room change")
            elif kind:
                if caller.override:
                    self._error(p, "room change in an open cutscene", call.cite,
                                f"{unit_name(caller.unit)} (room.cpp:61-62, 67-68, an error())", fatal=True)
                call.active = False
        self.write(p, f"var:{VAR_NEW_ROOM}", room)

    def _eng_room_code(self, p: Path, call: Call, oracle: Oracle, which: str) -> None:
        """The room's EXCD or ENCD, run nested in a room slot with no arguments, when the room has it
        (script.cpp:992-1009, 1043-1059)."""
        res = self.game.resource(self.read(p, "room", oracle))
        if isinstance(res, Unknown):
            p.unknown_writes.append((call.head, call.cite, f"the {which} code of room {res}"))
            return
        unit = (which, res)
        if unit in self.game.units:
            self._push_frame(p, call, unit, 0, {}, False, False, False, None)

    def _eng_scene_kill(self, p: Path, call: Call, oracle: Oracle) -> None:
        self._kill_room(p, call)

    def _eng_scene_load(self, p: Path, call: Call, oracle: Oracle, room) -> None:
        """room.cpp:149-218: the room, VAR_ROOM, VAR_ROOM_RESOURCE (a pseudo room maps through
        _resourceMapper), VAR_ROOM_FLAG = 1 when the resource had to be loaded (resource.cpp:640-644;
        whether it was in memory is not a fact), the camera bounds (room.cpp:202-205). Room 0 stops
        here (room.cpp:179-182), and so does a room whose resource is 0 (room.cpp:218)."""
        res = self.game.resource(room)
        self.write(p, "room", room)
        self.write(p, f"var:{VAR_ROOM}", room)
        self.write(p, f"var:{VAR_ROOM_RESOURCE}", res)
        if room == 0:
            call.i = len(call.steps)
            return
        if res != 0:
            flag = self.read(p, f"var:{VAR_ROOM_FLAG}", oracle)
            self.write(p, f"var:{VAR_ROOM_FLAG}", Unknown(R_CACHE, "1 if the room resource was not in memory",
                                                          None if isinstance(flag, Unknown) else frozenset({flag, 1}))
                       if flag != 1 else 1)
        self.write(p, f"var:{VAR_CAMERA_MIN_X}", SCREEN_WIDTH // 2)
        width = self.game.room_width.get(res) if not isinstance(res, Unknown) else None
        self.write(p, f"var:{VAR_CAMERA_MAX_X}", width - SCREEN_WIDTH // 2 if width is not None
                   else Unknown(R_WORK, f"width of room {res}"))
        if res == 0:
            call.i = len(call.steps)

    def _eng_scene_ego(self, p: Path, call: Call, oracle: Oracle, obj) -> None:
        """room.cpp:223-227: the ego's object must be in the new room (the engine's error() otherwise:
        the path ends)."""
        w = self.where(p, obj, oracle) if not isinstance(obj, Unknown) else obj
        if isinstance(w, Unknown):
            self._note(p, call.cite, call.head, f"whether object {obj} is in the new room: {w}")
        elif w != "room":
            self._error(p, "ego object not in room", call.cite, f"startScene: Object {obj} is not in room "
                        f"(room.cpp:225-227, an error())", fatal=True)

    def _room_change(self, p: Path, room, x, head: str) -> None:
        """Row 0: startScene (room.cpp:42-180) sets the room and kills room-object, local and room-code
        slots, the running one too (room.cpp:58-72); an inventory slot goes on. The kills of other slots
        and the exit and entry scripts (script.cpp:1083-1105, 989-1060) are feature room_change's:
        a note on the path."""
        self.write(p, "room", room)
        self._note(p, x, head, "the room change's kills of other slots and its exit and entry scripts are not "
                   "run (feature room_change)")
        unit = p.frame.unit
        if unit[0] in ROOM_KILLED or (unit[0] == "obj" and p.frame.owned is False):
            self._end(p, "room", x.cite())
        elif unit[0] == "obj" and isinstance(p.frame.owned, Unknown):
            self._note(p, x, head, f"slot kind unknown ({p.frame.owned}); taken as surviving the room change")

    def _scene_steps(self, room, ego_obj=None) -> tuple:
        """The room change's steps inside an opcode: the full startScene with room_change, else row 0's."""
        return (("scene", room, ego_obj),) if self.rooms else (("room1", room),)

    # --- settle: time passes in rounds ---------------------------------------------------------------------------

    def _pre_slots(self, oracle: Oracle) -> tuple:
        """The unit names of the slots live at pre, in the order of their fact keys: every script:N,
        local:N and objscript:O that is 1, and the room's entry code when one of its at:entry keys is 1
        (an exit slot is never live at a decision point, facts.py)."""
        keys = []
        for prefix in ("script:", "local:", "objscript:"):
            keys += [k for k in (oracle.keys(prefix) or []) if oracle.get(k) == 1]
        if any(oracle.get(k) == 1 for k in (oracle.keys("at:entry:") or [])):
            keys.append("at:entry")
        return tuple("entry" if k == "at:entry" else slot_name(k) for k in sorted(keys))

    def _slots_sig(self, p: Path, full: bool = True) -> tuple:
        """The slots off the stack: where each resumes and by which rule (``full``: also its locals and
        flags; merges fold locals into value sets, as they do a stack frame's). The sid is left out: a slot
        restarted every round gets a new one, which would hide a cycle."""
        if not full:
            return tuple((n, f.unit, f.pc, f.origin) for n, f in p.slots.items())
        return tuple((n, f.unit, f.pc, frozenset(f.locals.items()), f.args_known, f.origin, str(f.owned),
                      f.recursive, str(f.resistant)) for n, f in p.slots.items())

    def _position_key(self, p: Path) -> int:
        """What paths at a round's start must share to merge (settle): the room, every slot fact (script:,
        local:, objscript:, at:, frozen:, esc:, cutscene), where each slot off the stack resumes and by
        which rule, the freeze counts, the sentence queue, the ambient loops run and the cutscene data's
        length. The other facts and the slots' locals may differ: they become value sets (``_merge``)."""
        o = self.oracle
        slots = frozenset((k, v) for k, v in p.writes.items()
                          if (k in POSITION_KEYS or k.startswith(POSITION_PREFIXES)) and v != o.get(k))
        return hash((slots, self._slots_sig(p, full=False), frozenset(p.freeze.items()), tuple(p.queue),
                     frozenset(p.ambient_done), frozenset(p.camera_done), p.cut_index, len(p.cut_data)))

    def _state_key(self, p: Path) -> int:
        """A 64-bit hash of the path's state between two frames: its writes, the slots off the stack (where
        each resumes, its locals and flags), the freeze counts, the sentence queue, the cutscene data and the
        ambient loops already run."""
        return hash((frozenset(p.writes.items()), self._slots_sig(p), frozenset(p.freeze.items()), tuple(p.queue),
                     tuple(p.cut_data), p.cut_index, frozenset(p.ambient_done), frozenset(p.camera_done)))

    def _changed(self, p: Path, before: dict) -> list:
        """The facts whose value differs between the writes ``before`` and the path's now (pre's value where
        neither wrote it)."""
        o = self.oracle
        return sorted(k for k in set(p.writes) | set(before) if p.writes.get(k, o.get(k)) != before.get(k, o.get(k)))

    def _unsettle(self, p: Path, snapshots: list, why: str) -> None:
        """Every fact whose value differs between the snapshots (writes) becomes unknown, with the values it
        took when they are known: the path stopped before it settled."""
        o = self.oracle
        for k in sorted(set().union(*snapshots)):
            vals = [s.get(k, o.get(k)) for s in snapshots]
            if all(v == vals[-1] for v in vals):
                continue
            if any(isinstance(v, Unknown) and v.values is None for v in vals):
                p.writes[k] = Unknown(R_SETTLE, f"{k} {why}")
            else:
                p.writes[k] = one_of(frozenset().union(*(values_of(v) for v in vals)), R_SETTLE, f"{k} {why}")
            if not isinstance(p.writes[k], Unknown):  # one_of of a single value is the value itself
                p.writes[k] = Unknown(R_SETTLE, f"{k} {why}", frozenset({p.writes[k]}))
            p.writer[k] = R_SETTLE

    def _next_round(self, p: Path) -> None:
        """Between two frames (settle). After the immediate part (the click's frame, rows 1-2's run), its
        writes are kept as the "before the first yield" snapshot. After a round: a round that changed no
        fact on the path is a fixed point (the path ends "settled"); a state (writes, slot positions and
        locals, freeze counts, queue) met at an earlier round's start is a cycle (the facts that vary around
        it become unknown; "cycle"); at ROUND_LIMIT the facts the last round changed become unknown ("round
        limit"). Otherwise the next round starts: runAllScripts over the slots that may run, in their order
        (``_round_order``), then checkAndRunSentenceScript (scumm.cpp:3231-3255, script.cpp:968-987)."""
        self._round_end(p)
        if p.round == 0:
            if p.immediate is None:
                p.immediate = dict(p.writes)
                if p.first_end is None:
                    p.first_end = p.turns[-1][1] if p.turns else "nothing ran"
        else:
            if not self._changed(p, p.round_start):
                # the task's fixed point: no fact changed. A kept slot whose locals still move (a counter loop the
                # path knows) is not at rest in the engine; such ends are told apart in end_at and counted
                moving = hash(self._slots_sig(p)) != p.round_slots
                p.end, p.end_at = "settled", f"round {p.round} changed no fact" + (
                    "; a kept slot's locals still moved" if moving else "")
                return
            state = self._state_key(p)
            earlier = next((i for i, (h, _) in enumerate(p.history) if h == state), None)
            if earlier is not None:
                self._unsettle(p, [w for _, w in p.history[earlier:]] + [p.writes], "varies around a cycle")
                p.end, p.end_at = "cycle", f"after round {p.round}: the state at round {earlier + 1}'s start"
                return
            if p.round >= self.round_limit:
                self._unsettle(p, [p.round_start, p.writes], f"still changing at round {p.round}")
                p.end, p.end_at = "round limit", f"{self.round_limit} rounds"
                return
        p.history.append((self._state_key(p), dict(p.writes)))
        p.round_start = dict(p.writes)
        p.round_slots = hash(self._slots_sig(p))
        p.round += 1
        p.steps0 = p.steps
        p.round_ran, p.rw, p.owner = set(), {}, None
        steps = tuple(("turn", n) for n in self._round_order(p)) + (("sentences",),)
        p.stack.append(Call(ROUND, "script.cpp:968-987; scumm.cpp:3231-3255", steps, caller=None, active=False))

    def _round_order(self, p: Path) -> list:
        """The order runAllScripts runs the slots in. The engine runs them in slot order (script.cpp:978-979),
        which the facts do not give; the model takes the slots live at pre first, in the order of their keys,
        then the ones the path started, in the order they started (their sid): an approximation, whose
        possible effect ``_round_conflicts`` counts."""
        pre = list(self._pre)
        return pre + [n for _, n in sorted((f.sid, n) for n, f in p.slots.items() if n not in pre)]

    def _round_end(self, p: Path) -> None:
        """A frame's end (settle): its order conflicts are counted, and each ambient pass that ran in it keeps
        the values the facts it wrote had before and after it (``_close_settle``). The capped slot's own at:
        keys are left to ``_close_settle``."""
        self._round_conflicts(p)
        o = self.oracle
        for name in sorted(p.ambient_pass):
            for k in sorted(p.rw.get(name, (set(), set()))[1]):
                if k.startswith(f"at:{name}:"):
                    continue
                vals = p.ambient_vals.setdefault(k, [])
                for v in (p.round_start.get(k, o.get(k)), p.writes.get(k, o.get(k))):
                    if v not in vals:
                        vals.append(v)
        p.ambient_pass = set()

    def _round_conflicts(self, p: Path) -> None:
        """Two slots whose turns ran (or were passed over) in one frame, one writing a fact the other read
        (before writing it) or wrote: their order, which is not a fact, could change the result. A slot passed
        over logs its reads of its own liveness, freeze count and wait point, so a slot that froze or stopped it
        earlier in the frame counts. Read-read is no conflict, and the sentence queue, which runs after every
        turn (scumm.cpp:3231, 3255), is no party to one. This counts conflicts; it does not bound their effect."""
        items = sorted((k, v) for k, v in p.rw.items() if k != "sentence")
        for i, (a, (ra, wa)) in enumerate(items):
            for b, (rb, wb) in items[i + 1:]:
                keys = (wa & (rb | wb)) | (wb & ra)
                if keys:
                    p.conflicts.append((p.round, a, b, tuple(sorted(keys)), len(keys)))

    def _loosen(self, p: Path, k: str, vals: list, why: str) -> None:
        """Fact ``k`` becomes unknown with the values ``vals`` (R_SETTLE): a plain unknown when one of them is,
        nothing when they agree on one known value."""
        if all(v == vals[0] for v in vals[1:]) and not isinstance(vals[0], Unknown):
            return
        if any(isinstance(v, Unknown) and v.values is None for v in vals):
            v = Unknown(R_SETTLE, f"{k}: {why}")
        else:
            v = one_of(frozenset().union(*(values_of(v) for v in vals)), R_SETTLE, f"{k}: {why}")
            if not isinstance(v, Unknown):
                v = Unknown(R_SETTLE, f"{k}: {why}", frozenset({v}))
        p.writes[k] = v
        p.writer[k] = R_SETTLE

    def _close_settle(self, p: Path) -> None:
        """What a path that reached settle can no longer claim when it ends (any end the predictions use):
        - an ambient loop (no exit) runs at most once in settle, but the engine runs it every frame: each fact
          its pass wrote is, at done, one of the values it had around a pass (before it, after it, now), and a
          capped slot still live stands anywhere in its loop, so each of its at: keys may be 0 or 1;
        - a unit a recursive start gave a second live instance (``Path.multi``; runScript with R stops none,
          script.cpp:65-66): the path runs one Frame and keeps one set of at: keys per unit name (a turn clears
          the unit's at: keys), so each of its at: keys may be 0 or 1, and its slot fact, once the instance the
          path tracked ended, may still be 1.
        Each becomes unknown with those values (R_SETTLE): a possible prediction, not a certain one."""
        if p.closed or p.immediate is None:
            return
        p.closed = True
        o = self.oracle
        for k, vals in sorted(p.ambient_vals.items()):
            self._loosen(p, k, list(vals) + [p.writes.get(k, o.get(k))],
                         "an ambient loop settle ran once writes it every frame")
        for name in sorted(p.ambient_slots | p.multi):
            key = None if name in ("entry", "exit") else ("objscript:" + name[4:] if name.startswith("obj:") else name)
            prefix = f"at:{name}:"
            ats = sorted(set(o.keys(prefix) or []) | {k for k in p.writes if k.startswith(prefix)})
            live = p.writes.get(key, o.get(key)) if key else int(any(p.writes.get(k, o.get(k)) == 1 for k in ats))
            if name in p.multi:
                if key is not None and live == 0:
                    self._loosen(p, key, [0, 1], "a second instance started with R may still run")
                why = "two instances run it; the path tracks one"
            elif live == 0:
                continue  # a capped slot that died stands nowhere: its at: keys are 0
            else:
                why = "an ambient loop settle ran once: the slot stands anywhere in it"
            for k in ats:
                self._loosen(p, k, [0, 1], why)

    def _unit_named(self, p: Path, name: str, oracle: Oracle):
        """The unit of a slot's name: a local or the room's code is the room resource in use's."""
        if name.startswith("script:"):
            return ("global", int(name[7:]))
        if name.startswith("obj:"):
            return ("obj", int(name[4:]))
        res = self.game.resource(self.read(p, "room", oracle))
        if isinstance(res, Unknown):
            return None
        return ("local", res, int(name[6:])) if name.startswith("local:") else (name, res)

    def _slot_frame(self, p: Path, name: str, oracle: Oracle) -> tuple:
        """(the Frame settle would resume for slot ``name`` or None, why not). A slot that is dead on the
        path is skipped; one that is frozen, or whose liveness, freeze count or position is not known, is not
        run. A slot the path ran resumes from its kept Frame, whose ``at:`` key must still hold; a slot live
        at pre that has not run resumes from its one ``at:`` key with unknown locals and R flag (the F flag
        from the start sites, ``Game.resistant``). Its sid is given when it runs."""
        key = None if name in ("entry", "exit") else ("objscript:" + name[4:] if name.startswith("obj:") else name)
        if key is not None:
            live = self.read(p, key, oracle)
            if isinstance(live, Unknown):
                return None, "its liveness is not known"
            if not live:
                p.slots.pop(name, None)
                return None, ""
        frozen = self.read(p, f"frozen:{name}", oracle)
        if isinstance(frozen, Unknown):
            return None, "its freeze count is not known"
        if frozen:
            return None, "frozen"
        keys = set(self.oracle.keys(f"at:{name}:") or []) | {k for k in p.writes if k.startswith(f"at:{name}:")}
        vals = {k: self.read(p, k, oracle) for k in sorted(keys)}
        waits = [k for k, v in vals.items() if v == 1]
        if name in p.slots:
            f = p.slots[name]
            u = self.game.units[f.unit]
            if f.pc >= len(u.ins) or vals.get(f"at:{name}:{u.ins[f.pc].offset}") != 1:
                return None, "its position is not known"
            return f, ""
        if key is None and not waits:
            return None, ""  # room code that is not running
        if len(waits) != 1 or any(isinstance(v, Unknown) for v in vals.values()):
            return None, "it has no single wait point"
        unit = self._unit_named(p, name, oracle)
        u = self.game.units.get(unit) if unit is not None else None
        idx = u.at.get(int(waits[0].rsplit(":", 1)[1])) if u is not None else None
        if idx is None:
            return None, "its wait point is no instruction of its unit"
        owned = False
        if unit[0] == "obj":
            owner = self.read(p, f"owner:{unit[1]}", oracle)
            owned = owner if isinstance(owner, Unknown) else owner != OF_OWNER_ROOM
        res = self.game.resource(self.read(p, "room", oracle)) if unit[0] == "local" else None
        resistant = self.game.resistant(key, res) if key is not None else False
        return Frame(unit, idx, {}, False, owned, None, resistant, sid=0, nested=False, origin="pre"), ""

    def _changed_any(self, p: Path, reads) -> bool:
        """Whether the path changed one of these facts (keys, or prefixes ending in ":") from pre."""
        o = self.oracle
        for r in reads:
            if r.endswith(":"):
                if any(k.startswith(r) and v != o.get(k) for k, v in p.writes.items()):
                    return True
            elif r in p.writes and p.writes[r] != o.get(r):
                return True
        return False

    def _pre_rule(self, p: Path, wp: WaitPoint, oracle: Oracle) -> tuple:
        """(the rule a slot live at pre runs its first turn by, from its pre wait point, or None; why not;
        ``_eng_turn`` asks only then). Rule b, time-only waits: a WaitFor op
        for an actor, a message or the camera; a delay; a breakHere in a loop whose exit tests read only
        non-facts, or in an ambient loop (no exit; it runs once). Also b, by the engine's own rule that
        runAllScripts runs every live, unfrozen slot each frame (script.cpp:978-979): a breakHere outside any
        loop, and a slot parked after an opcode that ran a script nested (it waits for nothing; the task's
        list does not name these two, so they are counted apart). Rule c: a slot at a breakHere in a loop
        whose exit test reads facts runs once the path has changed one of them (WaitForSentence: the
        sentence script's slot fact). Any other slot is not run."""
        if wp.kind == "wait":
            if wp.op != "WaitForSentence":
                return "b: a WaitFor op", ""
            n = self.read(p, f"var:{VAR_SENTENCE_SCRIPT}", oracle)
            reads = self._sentence_slots(n) if isinstance(n, int) else []
            if self._changed_any(p, reads):
                return "c: WaitForSentence, the sentence slot changed", ""
            return None, "WaitForSentence: the sentence script's slot has not changed"
        if wp.kind == "delay":
            return "b: a delay", ""
        if wp.kind == "parked":
            return "b: parked after an opcode, waits for nothing", ""
        if wp.kind == "frame":
            lp = wp.loop
            if lp is None:
                return "b: a breakHere outside any loop", ""
            if lp.kind == "ambient":
                return "b: an ambient loop", ""
            if lp.kind == "time" and R_CAMERA in lp.non_facts:
                return "b: a loop on the camera position (its exit forks once)", ""
            if lp.kind == "time":
                return "b: a time-only loop", ""
            if self._changed_any(p, lp.reads):
                return "c: polls facts the path changed", ""
            return None, "polls facts the path has not changed"
        return None, f"its wait point is a {wp.kind}"

    def _origin(self, p: Path, name: str) -> str:
        """Where a slot settle meets comes from, for the skip counts: "path" (the path started it, or the action
        resumed it), "pre, first turn" (live at pre, not yet run by settle) or "pre, ran before"."""
        kept = p.slots.get(name)
        if kept is not None:
            return "pre, ran before" if kept.origin == "pre" else "path"
        if name in self._pre:
            return "pre, ran before" if name in p.ran_by else "pre, first turn"
        return "path"

    def _skip(self, p: Path, name: str, why: str) -> None:
        """A slot a round passes over, with why: every one is kept (``Path.skips``, by origin), and a slot live at
        pre that settle never ran is also kept in ``not_run``."""
        p.owner = None
        if not why:
            return
        p.skips.add((name, self._origin(p, name), why))
        if name in self._pre and name not in p.ran_by:
            p.not_run[name] = why

    @staticmethod
    def _time_wait(wp: WaitPoint) -> bool:
        """Whether a wait point is a time-only wait (rule b's first three kinds): a WaitFor op other than
        WaitForSentence, a delay, or a breakHere in a time-only loop (the camera's too)."""
        return (wp.kind == "wait" and wp.op != "WaitForSentence") or wp.kind == "delay" or \
            (wp.kind == "frame" and wp.loop is not None and wp.loop.kind == "time")

    def _eng_turn(self, p: Path, call: Call, oracle: Oracle, name: str) -> list:
        """One slot's turn in runAllScripts (settle). A slot that already executed in this frame (started
        nested in it) does not run again (didexec, script.cpp:506-507, 972, 979). Which slots run: (a) every
        slot the path started or the action resumed, live and not frozen; (b) and (c) for a slot live at pre, at
        its first turn, from its pre wait point (``_pre_rule``); once it has run, it runs every frame like any
        live, unfrozen slot (script.cpp:978-979), whatever it waits at next. Others are skipped, each with why
        (``_skip``: dead, frozen, liveness, freeze count or position not known, the rule at its pre wait point,
        the ambient cap). A slot whose wait point is in an ambient loop runs at most once per settle (the cap,
        counted; ``_close_settle`` makes what it leaves unknown). It resumes where it stopped, not nested
        (executeScript, script.cpp:980-983): a WaitFor op for an actor, a message or the camera falls through and
        a delay is over (time-only waits resolve with time: an assumption of kind R_WAIT_OVER). Its reads of its
        own slot facts are logged, so a slot run before it in the round that froze or stopped it is an order
        conflict (``_round_conflicts``)."""
        p.owner = None
        if name in p.round_ran:
            return []
        p.owner, p.owner_rule = name, ""
        p.rw.setdefault(name, (set(), set()))
        frame, why = self._slot_frame(p, name, oracle)
        if frame is None:
            self._skip(p, name, why)
            return []
        u = self.game.units[frame.unit]
        wp = self.game.wait_point(frame.unit, frame.pc, self.const_polls)
        if frame.origin == "pre" and not frame.sid:  # its first turn, from its pre wait point
            rule, why = self._pre_rule(p, wp, oracle)
            if rule is None:
                self._skip(p, name, why)
                return []
            if p.round == 0 and self._time_wait(wp):
                # the click's frame is one frame after pre: a time-only wait the slot was in at pre is taken to
                # still hold there (its turn changes nothing); time resolves it from the next frame on
                self._skip(p, name, "a time-only wait, held in the click's frame")
                return []
        else:
            rule = p.ran_by.get(name, "a: started or resumed by the path")
        if wp.loop is not None and wp.loop.kind == "ambient":
            if (name, wp.loop.head) in p.ambient_done:
                self._skip(p, name, "the ambient cap: its loop with no exit ran once")
                return []
            p.ambient_done.add((name, wp.loop.head))
            p.ambient_slots.add(name)
            p.ambient_pass.add(name)
        p.ran_by.setdefault(name, rule)
        p.not_run.pop(name, None)
        g = dataclasses.replace(frame, locals=dict(frame.locals), nested=False, killed=None, freeze=0,
                                time_loops=self.game.time_loops_at(frame.unit, frame.pc, self.const_polls))
        if not g.sid:
            g.sid = p.next_sid
            p.next_sid += 1
            if g.resistant is None:
                self._note(p, call.cite, "resume", f"{name}: F flag not known")
        p.owner_rule = rule
        p.timed = True
        self.clear(p, f"at:{name}:", None, "turn")  # it runs from its wait point now
        p.stack.append(g)
        p.depth = max(p.depth, len(self._frames(p)))
        p.round_ran.add(name)
        p.seen = set()
        x = u.ins[g.pc]
        if wp.kind == "wait" and wp.op != "WaitForSentence":
            p.assumptions.append((x.cite(), f"{x.text.strip()} is over", True,
                                  str(Unknown(R_WAIT_OVER, WAITS[wp.op][1]))))
            g.pc = x.next
        elif wp.kind == "delay":
            d = u.ins[g.pc - 1]
            p.assumptions.append((d.cite(), f"{name}'s delay is over", True, str(Unknown(R_WAIT_OVER, "delay count"))))
        return []

    def _eng_sentences(self, p: Path, call: Call, oracle: Oracle) -> None:
        """checkAndRunSentenceScript at a round's end (script.cpp:1174-1247): nothing while a slot of the
        sentence script's number is live and not frozen; else the queue's top, unless its freeze count is
        above 0, is popped; one whose B is its A is dropped (script.cpp:1203-1204); else the sentence script
        runs with [verb, A, B]. The queue holds the click's sentence when it had to wait, and what doSentence
        queued on the path; the queue at pre is taken as empty (it is not a fact)."""
        p.owner = None
        if not p.queue:
            return
        n = self.read(p, f"var:{VAR_SENTENCE_SCRIPT}", oracle)
        if isinstance(n, Unknown):
            self._note(p, call.cite, "sentence queue", f"VAR_SENTENCE_SCRIPT {n}: the queue is not run")
            return
        busy, unknown = self._sentence_waits(p, n, oracle) if n else ([], [])
        if busy:
            return
        if unknown:
            self._note(p, call.cite, "sentence queue", f"whether the sentence waits: {'; '.join(unknown)}")
            return
        verb, a, b, frozen = p.queue[-1]
        if frozen:
            return
        p.queue.pop()
        if b and b == a:
            self._note(p, call.cite, "sentence queue", "a queued sentence whose B is its A is dropped")
            return
        if n:
            p.sentences += 1
            p.owner, p.owner_rule = "sentence", "the sentence queue"
            p.rw.setdefault("sentence", (set(), set()))
            self._eng_script(p, call, oracle, n, False, False, {0: verb, 1: a, 2: b})

    def _sentence_busy(self, p: Path):
        """WaitForSentence's test (script_v5.cpp:3296-3302): it blocks while the sentence script is in use, or
        while a sentence is queued unless the top one is frozen and the script is not in use. True, False or
        an unknown."""
        n = self.read(p, f"var:{VAR_SENTENCE_SCRIPT}", self.oracle)
        if isinstance(n, Unknown):
            return n
        used = [self.read(p, k, self.oracle) for k in self._sentence_slots(n)]
        if bad := next((v for v in used if isinstance(v, Unknown)), None):
            return bad
        in_use = any(used)
        if p.queue:
            return not (p.queue[-1][3] and not in_use)
        return in_use

    # --- starts ---------------------------------------------------------------------------------------------------

    def _start(self, p: Path, x, st: Start, oracle: Oracle) -> list:
        head = st.name
        if self.calls:
            locals_ = {i: self.val(p, a, oracle, x) for i, a in enumerate(st.args)}  # getWordVararg
            if st.name == "startObject":
                obj, verb = self.val(p, st.obj, oracle, x), self.val(p, st.script, oracle, x)
                if isinstance(obj, Unknown) or isinstance(verb, Unknown):
                    p.unknown_writes.append((head, x.cite(), f"objscript of {obj} for verb {verb}"))
                    return []
                self._push_call(p, head, x.cite(), (("object", obj, verb, locals_),))
                return []
            n = self.val(p, st.script, oracle, x)
            if isinstance(n, Unknown):
                p.unknown_writes.append((head, x.cite(), f"script {n}"))
                return []
            f = p.frame
            if st.name == "chainScript":
                # the running slot dies first and _currentScript is 0xFF, then runScript(N) with that
                # slot's F and R (script_v5.cpp:843-850): N takes its place under the same caller
                f.killed = "chained"
                self._gone(p, f)
                self._push_call(p, head, x.cite(), (("script", n, f.resistant, f.recursive, locals_),), active=False)
                return []
            self._push_call(p, head, x.cite(), (("script", n, st.freeze, st.recursive, locals_),))
            return []
        if st.name == "startObject":
            obj, verb = self.val(p, st.obj, oracle, x), self.val(p, st.script, oracle, x)
            if isinstance(obj, Unknown) or isinstance(verb, Unknown):
                p.unknown_writes.append((head, x.cite(), f"objscript of {obj} for verb {verb}"))
                return []
            # runObjectScript(O, verb, 0, 0), never with R (script_v5.cpp:2889): it stops O's scripts first
            # (script.cpp:117-118), then starts a new instance when O is in the room or inventory and has
            # an entry for the verb
            key = f"objscript:{obj}"
            w = self.where(p, obj, oracle)
            if isinstance(w, Unknown):
                self.may_stop(p, key, x, head)
                self.write(p, key, w)
            elif w != "nowhere" and self.game.entry(obj, verb) is not None:
                self.started(p, key, False, x, head)
            else:  # stopped, and no new instance
                self.stopped(p, key, x, head)
            if self._is_self(p, key):
                self._end(p, "end", f"{x.cite()} (startObject stops the running object's scripts)", lifecycle=False)
            return []
        n = self.val(p, st.script, oracle, x)
        if isinstance(n, Unknown):
            p.unknown_writes.append((head, x.cite(), f"script {n}"))
            return []
        if n == 0:  # runScript(0) does nothing (script.cpp:62-63)
            return []
        key = self.game.script_key(n)
        if st.name == "chainScript":
            # the running slot dies first, then runScript with the slot's own R flag (script_v5.cpp:834-851)
            self.started(p, key, p.frame.recursive, x, head)
            self._end(p, "end", x.cite(), lifecycle=not self._is_self(p, key))
            return []
        self.started(p, key, st.recursive, x, head)
        if not st.recursive and self._is_self(p, key):
            self._end(p, "end", f"{x.cite()} (the start stops the running script)", lifecycle=False)
        return []

    # --- op rules (HEADS has the cites) ----------------------------------------------------------------------

    def _op_stopObjectCode(self, p, x, st, oracle):
        pass  # the "end" effect ends the turn

    def _op_stopScript(self, p, x, st, oracle):
        n = self.val(p, st.args[0], oracle, x)
        if isinstance(n, Unknown):
            p.unknown_writes.append(("stopScript", x.cite(), f"script {n}"))
        elif n == 0:  # stopObjectCode (script_v5.cpp:3035-3036)
            self._done(p, "end", x.cite())
        elif self.calls:
            self._stop_script(p, n, x.cite(), "stopScript")
        else:
            key = self.game.script_key(n)
            self.stopped(p, key, x, "stopScript")
            if self._is_self(p, key):
                self._end(p, "end", x.cite(), lifecycle=False)

    def _op_stopObjectScript(self, p, x, st, oracle):
        obj = self.val(p, st.args[0], oracle, x)
        if isinstance(obj, Unknown):
            p.unknown_writes.append(("stopObjectScript", x.cite(), f"objscript {obj}"))
            return
        if self.calls:
            self._kill_object(p, obj, x.cite(), "stopObjectScript")
            return
        self.stopped(p, f"objscript:{obj}", x, "stopObjectScript")
        if self._is_self(p, f"objscript:{obj}"):
            self._end(p, "end", x.cite(), lifecycle=False)

    def _op_doSentence(self, p, x, st, oracle):
        if "STOP" in st.flags:  # stops VAR_SENTENCE_SCRIPT (script_v5.cpp:1004-1008)
            if p.settle:  # it also empties the sentence queue (_sentenceNum = 0, script_v5.cpp:1005)
                p.queue.clear()
            if p.settle and p.round == 0:  # and clearClickedStatus wipes the frame's key or click (script_v5.cpp:1007,
                p.cleared = True           # input.cpp:378-386): read by the Esc key's checkExecVerbs (``_eng_key``)
            n = self.read(p, f"var:{VAR_SENTENCE_SCRIPT}", oracle)
            if isinstance(n, Unknown):
                p.unknown_writes.append(("doSentence", x.cite(), f"sentence script {n}"))
            elif n and self.calls:
                self._stop_script(p, n, x.cite(), "doSentence")
            elif n:
                key = self.game.script_key(n)
                self.stopped(p, key, x, "doSentence")
                if self._is_self(p, key):
                    self._end(p, "end", x.cite(), lifecycle=False)
            return
        if p.settle:
            # doSentence (script.cpp:1145-1172) pushes (verb, A, B) with freeze count 0; v5 drops no duplicate.
            # checkAndRunSentenceScript pops the top at the end of a later frame (``_eng_sentences``)
            vals = [self.val(p, a, oracle, x) for a in st.args]
            if len(vals) == 3 and not any(isinstance(v, Unknown) for v in vals):
                p.queue.append((vals[0], vals[1], vals[2], 0))
                self._note(p, x, "doSentence", "sentence queued; settle runs it at a round's end")
            else:
                p.unknown_writes.append(("doSentence", x.cite(), f"sentence {[str(v) for v in vals]}"))
                self._note(p, x, "doSentence", "a sentence with an unknown verb or object is not queued")
            return
        self._note(p, x, "doSentence", "sentence queued; VAR_SENTENCE_SCRIPT runs it at the end of a later frame "
                   "(not run at this row)")

    def _op_setState(self, p, x, st, oracle):
        obj, s = (self.val(p, a, oracle, x) for a in st.args)
        self.put(p, Unknown(R_WORK, f"object {obj}") if isinstance(obj, Unknown) else f"state:{obj}", s, x,
                 "setState")

    def _op_setClass(self, p, x, st, oracle):
        obj = self.val(p, st.args[0], oracle, x)
        for c in st.args[1].items:
            cv = self.val(p, c, oracle, x)
            if isinstance(obj, Unknown) or isinstance(cv, Unknown):
                p.unknown_writes.append(("setClass", x.cite(), f"class {cv} of {obj}"))
            elif cv == 0:  # class 0 clears all class data (script_v5.cpp:679-681)
                for k in range(1, 33):  # classes 1-32 (object.cpp:263)
                    self.write(p, f"class:{obj}:{k}", 0)
            else:
                self.write(p, f"class:{obj}:{cv & 0x7F}", 1 if cv & 0x80 else 0)

    def _inventory_hook(self, p, x, arg: int, head: str, oracle):
        """The inventory hook: row 0 records the start, calls runs it with Local[0] = ``arg``."""
        if self.calls:
            self._push_call(p, head, x.cite(), (("hook", VAR_INVENTORY_SCRIPT, {0: arg}),))
        else:
            self.hook(p, VAR_INVENTORY_SCRIPT, oracle, x, head)

    def _op_setOwnerOf(self, p, x, st, oracle):
        obj, owner = (self.val(p, a, oracle, x) for a in st.args)
        if isinstance(obj, Unknown):
            p.unknown_writes.append(("setOwnerOf", x.cite(), f"owner of {obj}"))
            self._inventory_hook(p, x, 0, "setOwnerOf", oracle)
            return
        key = f"objscript:{obj}"
        if self.calls:
            more = []
            if owner == 0:  # clearOwnerOf stops the object's scripts (object.cpp:183-184)
                self._kill_object(p, obj, x.cite(), "setOwnerOf")
            elif isinstance(owner, Unknown):
                if any(f.unit == ("obj", obj) for f in self._frames(p)):
                    q = p.fork()
                    p.assumptions.append((x.cite(), f"owner {show(st.args[1])} != 0", True, str(owner)))
                    q.assumptions.append((x.cite(), f"owner {show(st.args[1])} != 0", False, str(owner)))
                    self._kill_object(q, obj, x.cite(), "setOwnerOf")
                    q.writes[f"owner:{obj}"] = 0
                    self._inventory_hook(q, x, 0, "setOwnerOf", oracle)
                    more = [q]
                if self.read(p, key, oracle) != 0:
                    self.write(p, key, owner)
            self.write(p, f"owner:{obj}", owner)
            self._inventory_hook(p, x, 0, "setOwnerOf", oracle)
            return more
        if owner == 0:  # clearOwnerOf stops the object's scripts (object.cpp:183-184)
            self.stopped(p, key, x, "setOwnerOf")
        elif isinstance(owner, Unknown):
            self.write(p, key, owner)
        self.write(p, f"owner:{obj}", owner)
        self.hook(p, VAR_INVENTORY_SCRIPT, oracle, x, "setOwnerOf")
        if p.end is None and self._is_self(p, key):
            if owner == 0:
                self._end(p, "end", f"{x.cite()} (setOwnerOf 0 stops the running object)", lifecycle=False)
            elif isinstance(owner, Unknown):
                q = p.fork()
                p.assumptions.append((x.cite(), f"owner {show(st.args[1])} != 0", True, str(owner)))
                q.assumptions.append((x.cite(), f"owner {show(st.args[1])} != 0", False, str(owner)))
                q.writes[key] = 0
                self._end(q, "end", x.cite(), lifecycle=False)
                return [q]

    def _op_pickupObject(self, p, x, st, oracle):
        obj = self.val(p, st.args[0], oracle, x)
        if isinstance(obj, Unknown):
            p.unknown_writes.append(("pickupObject", x.cite(), f"object {obj}"))
        else:
            self.write(p, f"owner:{obj}", self.read(p, f"var:{VAR_EGO}", oracle))
            self.write(p, f"class:{obj}:{CLASS_UNTOUCHABLE}", 1)
            self.write(p, f"state:{obj}", 1)
        self._inventory_hook(p, x, 1, "pickupObject", oracle)  # runInventoryScript(1), script_v5.cpp:2033

    def _op_print(self, p, x, st, oracle):
        # decodeParseString prints a Text sub-op in slot 3 for actor 252 (script_v5.cpp:3443-3460):
        # a message box whose key press goes to VAR_KEYPRESS, var 0 (string.cpp:119, 164-169)
        if not any(s.name == "Text" for s in st.subops):
            return
        actor = self.val(p, st.args[0], oracle, x)
        if isinstance(actor, Unknown):
            self._note(p, x, "print", f"actor {actor}: if {MESSAGE_BOX_ACTOR}, var 0 gets the key press")
        elif actor == MESSAGE_BOX_ACTOR:
            self.write(p, f"var:{VAR_KEYPRESS}", Unknown(R_KEY, "message box"))

    def _op_setObjectName(self, p, x, st, oracle):
        self._inventory_hook(p, x, 0, "setObjectName", oracle)  # runInventoryScript(0), object.cpp:1314

    def _op_drawObject(self, p, x, st, oracle):
        obj = self.val(p, st.args[0], oracle, x)
        state = 1
        for s in st.subops:
            if s.name == "setImage":
                state = self.val(p, s.args[0], oracle, x)
        if isinstance(obj, Unknown):
            p.unknown_writes.append(("drawObject", x.cite(), f"object {obj}"))
            return
        res = self.game.resource(self.read(p, "room", oracle))
        if isinstance(res, Unknown):
            self.write(p, f"state:{obj}", Unknown(R_WORK, f"room {res}"))
        elif self.game.object_room.get(obj) == res:  # getObjectIndex finds only the room's objects
            self.write(p, f"state:{obj}", state)
            self._note(p, x, "drawObject", "objects with the same position and size go to state 0 (not modelled)")

    def _op_VerbOps(self, p, x, st, oracle):
        v = self.val(p, st.args[0], oracle, x)
        if isinstance(v, Unknown):
            p.unknown_writes.append(("VerbOps", x.cite(), f"verb {v}"))
            return
        key = f"verb:{v}"
        now = self.read(p, key, oracle)
        shown, slot = now, now == 1  # a shown verb has a saveid-0 slot; a hidden one may have none
        changed = False
        for s in st.subops:
            if s.name == "New":
                shown, slot, changed = 0, True, True
            elif s.name == "On":
                shown, changed = (1 if slot else Unknown(R_VERB, f"verb {v}: no shown slot, may have a hidden "
                                                     f"one")), True
            elif s.name in ("Off", "Dim", "Delete"):
                shown, changed = 0, True
                slot = slot and s.name != "Delete"
        if changed and (isinstance(shown, Unknown) or shown != now):
            self.write(p, key, shown)

    def _op_saveVerbs(self, p, x, st, oracle):
        a, b, _ = (self.val(p, e, oracle, x) for e in st.args)
        if isinstance(a, Unknown) or isinstance(b, Unknown):
            p.unknown_writes.append(("saveVerbs", x.cite(), f"verbs {a}..{b}"))
            return
        for v in range(a, b + 1):
            if self.read(p, f"verb:{v}", oracle) != 0:
                self.write(p, f"verb:{v}", 0)

    def _op_restoreVerbs(self, p, x, st, oracle):
        a, b, c = (self.val(p, e, oracle, x) for e in st.args)
        if isinstance(a, Unknown) or isinstance(b, Unknown):
            p.unknown_writes.append(("restoreVerbs", x.cite(), f"verbs {a}..{b}"))
            return
        for v in range(a, b + 1):
            self.write(p, f"verb:{v}", Unknown(R_VERB, f"verb {v} restored from save id {c}"))

    def _op_setVarRange(self, p, x, st, oracle):
        # getResultPos, then setResult and _resultVarNumber++ per value: the next variable, bit or local
        first, _, values = st.args
        t = self.target(p, first, oracle, x)
        for k, e in enumerate(values.items):
            v = self.val(p, e, oracle, x)
            if isinstance(t, Unknown):
                self.put(p, t, v, x, "setVarRange")
            elif isinstance(t, tuple):
                self.put(p, ("local", t[1] + k), v, x, "setVarRange")
            else:
                kind, n = t.split(":")
                self.put(p, f"{kind}:{int(n) + k}", v, x, "setVarRange")

    def _op_loadRoom(self, p, x, st, oracle):
        room = self.val(p, st.args[0], oracle, x)
        if self.calls:  # o5_loadRoom: startScene(room, nullptr, 0) (script_v5.cpp:1849-1850)
            self._push_call(p, "loadRoom", x.cite(), self._scene_steps(room))
        else:
            self._room_change(p, room, x, "loadRoom")

    def _op_loadRoomWithEgo(self, p, x, st, oracle):
        obj, room = self.val(p, st.args[0], oracle, x), self.val(p, st.args[1], oracle, x)
        if self.calls:
            # o5_loadRoomWithEgo (script_v5.cpp:1857-1902): VAR_WALKTO_OBJ = obj, startScene(room, ego, obj),
            # VAR_WALKTO_OBJ = 0, then setCameraFollows(ego): the ego is in the room, so no second room
            # change; a possible setCameraAt (the scroll hook) and the inventory hook (camera.cpp:62-86)
            if self.rooms:
                steps = (("write", f"var:{VAR_WALKTO_OBJ}", obj), *self._scene_steps(room, obj),
                         ("write", f"var:{VAR_WALKTO_OBJ}", 0))
            else:
                steps = self._scene_steps(room)
            steps += (("scroll_note",), ("hook", VAR_INVENTORY_SCRIPT, {0: 0}))
            self._push_call(p, "loadRoomWithEgo", x.cite(), steps)
            return
        self._room_change(p, room, x, "loadRoomWithEgo")
        self.hook(p, VAR_INVENTORY_SCRIPT, oracle, x, "loadRoomWithEgo")
        if p.end is None:
            self._note(p, x, "loadRoomWithEgo", "the scroll hook may run (camera position, not a fact)")

    def _op_actorFollowCamera(self, p, x, st, oracle):
        a = self.val(p, st.args[0], oracle, x)
        q = p.fork()
        why = str(Unknown(R_ACTOR, f"room of actor {a}"))
        p.assumptions.append((x.cite(), f"actor {show(st.args[0])} in the current room", True, why))
        q.assumptions.append((x.cite(), f"actor {show(st.args[0])} in the current room", False, why))
        if self.calls:
            # actorFollowCamera (camera.cpp:201-212): setCameraFollows (camera.cpp:62-86) changes the room
            # when the actor is elsewhere and then always calls setCameraAt, so the scroll hook runs
            # (camera.cpp:69-74, 52-55); it may call setCameraAt again (camera.cpp:76-79, a position: a
            # note), runs the inventory hook; a second inventory hook run when the followed actor changed
            # (camera._follows is not a fact: a note)
            tail = (("scroll_note",), ("hook", VAR_INVENTORY_SCRIPT, {0: 0}),
                    ("note", "a second inventory hook run when the followed actor changed (camera.cpp:207-208) is "
                             "not run: camera._follows is not a fact"))
            self._push_call(p, "actorFollowCamera", x.cite(), tail)
            self._push_call(q, "actorFollowCamera", x.cite(),
                            self._scene_steps(Unknown(R_ACTOR, f"room of actor {a}"))
                            + (("hook", VAR_SCROLL_SCRIPT, {}),) + tail)
            return [q]
        # setCameraFollows runs the inventory hook last, also after a room change (camera.cpp:61-86)
        self.hook(p, VAR_INVENTORY_SCRIPT, oracle, x, "actorFollowCamera")
        self.hook(q, VAR_INVENTORY_SCRIPT, oracle, x, "actorFollowCamera")
        if q.end is None:
            self._room_change(q, Unknown(R_ACTOR, f"room of actor {a}"), x, "actorFollowCamera")
        return [q]

    def _op_setCameraAt(self, p, x, st, oracle):
        if self.calls:  # setCameraAt runs the scroll hook with no arguments (camera.cpp:52-55)
            self._push_call(p, "setCameraAt", x.cite(), (("hook", VAR_SCROLL_SCRIPT, {}),))
        else:
            self.hook(p, VAR_SCROLL_SCRIPT, oracle, x, "setCameraAt")

    def _op_freezeScripts(self, p, x, st, oracle):
        """o5_freezeScripts (script_v5.cpp:1252-1259): a flag not 0 freezes, 0 unfreezes (calls only;
        row 0 has no freeze counts)."""
        if not self.calls:
            return
        flag = self.val(p, st.args[0], oracle, x)
        if isinstance(flag, Unknown):
            p.unknown_writes.append(("freezeScripts", x.cite(), f"flag {flag}"))
        elif flag:
            self._freeze(p, flag, x)
        else:
            self._unfreeze(p, x)

    # cutscene is the stack depth, esc: the override of the top level only (facts.py)

    def _op_cutscene(self, p, x, st, oracle):
        self.write(p, "cutscene", _arith("+", self.read(p, "cutscene", oracle), 1))
        self.clear(p, "esc:", x, "cutscene")  # the new level has no override (script.cpp:1641-1642)
        if not self.calls:
            self.hook(p, VAR_CUTSCENE_START_SCRIPT, oracle, x, "cutscene")
            return
        # beginCutscene (script.cpp:1632-1648): the slot's cutsceneOverride + 1, the level keeps args[0],
        # and the start hook runs with the cutscene's list while vm.cutSceneScriptIndex is this slot
        f = p.frame
        args = [self.val(p, a, oracle, x) for a in st.args[0].items] if st.args and isinstance(st.args[0], List) \
            else []
        f.override += 1
        p.cut_data.append(args[0] if args else 0)
        self._push_call(p, "cutscene", x.cite(), (("cut_index", f.sid),
                                                  ("hook", VAR_CUTSCENE_START_SCRIPT, dict(enumerate(args))),
                                                  ("cut_index", None)))

    def _op_endCutscene(self, p, x, st, oracle):
        depth = self.read(p, "cutscene", oracle)
        armed = self.calls and any(self.read(p, k, oracle) == 1 for k in set(oracle.keys("esc:") or []) | set(p.writes)
                                   if k.startswith("esc:"))
        self.write(p, f"var:{VAR_OVERRIDE}", 0)
        self.write(p, "cutscene", _arith("-", depth, 1))
        self.clear(p, "esc:", x, "endCutscene")
        self._note(p, x, "endCutscene", "an override armed on the level below may show again (not a fact)")
        if not self.calls:
            self.hook(p, VAR_CUTSCENE_END_SCRIPT, oracle, x, "endCutscene")
            return
        # endCutscene (script.cpp:1650-1687): the end hook gets the closed level's cutSceneData, known
        # only for a level this path opened; a stack already at 0 is the engine's error(): the path ends
        f = p.frame
        f.override = max(0, f.override - 1 - (1 if armed and f.override > 1 else 0))  # script.cpp:1654-1664
        if depth == 0:
            self._error(p, "cutscene stack underflow", x.cite(), "endCutscene's error() (script.cpp:1668-1681)",
                        fatal=True)
            return
        data = p.cut_data.pop() if p.cut_data else Unknown(R_CUTDATA, "the data of a cutscene opened before the move")
        self._push_call(p, "endCutscene", x.cite(), (("hook", VAR_CUTSCENE_END_SCRIPT, {0: data}),))

    def _op_beginOverride(self, p, x, st, oracle):
        self.write(p, f"var:{VAR_OVERRIDE}", 0)
        self.clear(p, "esc:", x, "beginOverride")  # one override per level: this one replaces it
        if x.esc is not None:  # Esc resumes at the jump after beginOverride (ir.py esc)
            u = self.game.units[p.frame.unit]
            self.write(p, f"esc:{unit_name(p.frame.unit)}:{u.ins[x.esc].offset}", 1)
        else:
            self._note(p, x, "beginOverride", "no jump after it (ir.py); esc: not predicted")

    def _op_endOverride(self, p, x, st, oracle):
        self.write(p, f"var:{VAR_OVERRIDE}", 0)
        self.clear(p, "esc:", x, "endOverride")

    def _cursor(self, p, var, value):
        self.write(p, f"var:{var}", value)

    def _op_UserputOn(self, p, x, st, oracle):
        self._cursor(p, VAR_USERPUT, 1)

    def _op_UserputOff(self, p, x, st, oracle):
        self._cursor(p, VAR_USERPUT, 0)

    def _op_UserputSoftOn(self, p, x, st, oracle):
        self._cursor(p, VAR_USERPUT, _arith("+", self.read(p, f"var:{VAR_USERPUT}", oracle), 1))

    def _op_UserputSoftOff(self, p, x, st, oracle):
        self._cursor(p, VAR_USERPUT, _arith("-", self.read(p, f"var:{VAR_USERPUT}", oracle), 1))

    def _op_CursorShow(self, p, x, st, oracle):
        self._cursor(p, VAR_CURSORSTATE, 1)

    def _op_CursorHide(self, p, x, st, oracle):
        self._cursor(p, VAR_CURSORSTATE, 0)

    def _op_CursorSoftOn(self, p, x, st, oracle):
        self._cursor(p, VAR_CURSORSTATE, _arith("+", self.read(p, f"var:{VAR_CURSORSTATE}", oracle), 1))

    def _op_CursorSoftOff(self, p, x, st, oracle):
        self._cursor(p, VAR_CURSORSTATE, _arith("-", self.read(p, f"var:{VAR_CURSORSTATE}", oracle), 1))

    def _op_RoomScroll(self, p, x, st, oracle):
        res = self.game.resource(self.read(p, "room", oracle))
        width = self.game.room_width.get(res) if not isinstance(res, Unknown) else None
        for var, e in zip((VAR_CAMERA_MIN_X, VAR_CAMERA_MAX_X), st.args):
            v = self.val(p, e, oracle, x)
            if not isinstance(v, Unknown):
                v = max(v, SCREEN_WIDTH // 2)
                if width is None:
                    v = Unknown(R_WORK, f"width of room {res}")
                else:
                    v = min(v, width - SCREEN_WIDTH // 2)
            self.write(p, f"var:{var}", v)


# --- the machine's own rule for variables that are not facts ------------------------------------------------

def _walk(e):
    yield e
    if isinstance(e, tuple):
        for y in e:
            yield from _walk(y)
    elif dataclasses.is_dataclass(e) and not isinstance(e, type):
        for f in dataclasses.fields(e):
            yield from _walk(getattr(e, f.name))


def script_writes(game: Game | None = None) -> dict[int, list]:
    """Every value the scripts write to each variable: an assignment's value (``x op= v`` as ``x op v``),
    an embedded opcode's value into var 0, and setVarRange's constants. A write to an indexed variable
    is left out (its target is not known statically)."""
    game = game or Game.load()
    out = collections.defaultdict(list)
    ops = {"+=": "+", "++": "+", "-=": "-", "--": "-", "*=": "*", "/=": "/"}
    for u in game.units.values():
        for x in u.ins:
            st = x.st
            if st is None:
                continue
            for e in _walk(st):
                if isinstance(e, Embed) and isinstance(e.st.lhs, Var):
                    out[e.st.lhs.n].append(e.st.rhs)
            if isinstance(st, Assign) and isinstance(st.lhs, Var):
                out[st.lhs.n].append(st.rhs if st.op == "=" else BinOp(ops[st.op], st.lhs, st.rhs))
            elif isinstance(st, Op) and st.name == "setVarRange" and isinstance(st.args[0], Var):
                for k, e in enumerate(st.args[2].items):
                    out[st.args[0].n + k].append(e)
    return out


def non_fact_vars(game: Game | None = None) -> dict[int, str]:
    """Variables whose every script write is unknown by the machine's rules: the value holds an
    unknown getter (a random draw, a position, actor state, a sound, a string, the inventory order),
    a clock (facts.CLOCKS) or a variable already in the set. The least fixpoint, so a counter that
    only adds to itself stays a fact. Returns {var: one reason}."""
    writes = script_writes(game)
    unknown_getters = {g for g, (kind, _) in GETTERS.items() if kind != "fact"}
    out: dict[int, str] = {}

    def why(e) -> str | None:
        for y in _walk(e):
            if isinstance(y, Getter) and y.name in unknown_getters:
                return GETTERS[y.name][0]
            if isinstance(y, Var) and y.n in F.CLOCKS:
                return R_TIMER
            if isinstance(y, Var) and y.n in out:
                return f"var {y.n}"
        return None

    changed = True
    while changed:
        changed = False
        for n, values in writes.items():
            if n in out:
                continue
            reasons = [why(v) for v in values]
            if all(reasons):
                out[n] = collections.Counter(reasons).most_common(1)[0][0]
                changed = True
    return out


# --- predictions -------------------------------------------------------------------------------------------

INFEASIBLE = ("spin", "nesting", "error")  # path ends left out of the predictions (``feasible``)


def feasible(run: Run) -> list:
    """The paths the predictions use: not those that end in an engine error() (``nesting``, past the
    nesting limit, script.cpp:351-352; ``error``, the other error() cases, ``Machine._error``): the game
    would abort there, and run B did not; nor those that ``spin`` (``Machine._spins``): every way on
    from the state they met again is run by the path that met it first, so they add nothing."""
    return [p for p in run.paths if p.end not in INFEASIBLE]


def predict(run: Run, pre: Oracle) -> dict[str, dict]:
    """Per fact some feasible path (``feasible``) writes: the value on every such path (a path that does
    not write it keeps the pre value), ``certain`` when all are the same known value, and the values it
    can take (the known ones and those of value sets)."""
    out = {}
    paths = feasible(run)
    keys = {k for p in paths for k in p.writes}
    for k in sorted(keys):
        before = pre.get(k)
        vals = [p.writes.get(k, before) for p in paths]
        known = [v for v in vals if not isinstance(v, Unknown)]
        certain = len(known) == len(vals) and len(set(known)) == 1
        sets = set(known).union(*(v.values for v in vals if isinstance(v, Unknown) and v.values is not None))
        out[k] = {"certain": certain, "values": sorted(sets), "unknown": len(vals) - len(known),
                  "pre": before if not isinstance(before, Unknown) else str(before)}
    return out
