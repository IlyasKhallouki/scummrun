# Fix round 1: design (proposal, v2)

Fix round 1 collects the model's known errors. By D7 the fixes are made in Rust, after the port matches the
frozen Python byte for byte (model.py cc630446bdbe, reader.py 94d495853f07, check.py fc1c3a935c4e, actions.py
ddbd1aaa2207). ScummVM lines are from third_party/scummvm; script positions are file and offset in
data/scripts-dos-cd/. MI1 numbers are evidence only; no rule names one. No model check or reading was run. The last
section answers the critique point by point.

## How a fix lands

- Each fix is a switch in the Rust machine or reader, off by default. All off: every golden byte for byte.
- Rows 0-5 are frozen and never recomputed. A fix adds new rows and goldens, alone and on top of the fixes before
  it. New Game tables go into game.jsonl only when their switch is on.
- The model check (machine, with run B's trace) and the planner (reader, no trace) are kept apart in every item.

| | item | order |
|---|---|---|
| F1 | an input is one record, and it starts at the input script | after F2, F5, F9, F12 |
| F2 | where in the frame an input is read (open choice), and the planner rule | before F1 |
| F3 | an input's engine conditions are read in its frame (S3#45) | verified on F1, F2, F7 |
| F4 | '.' is the talk-stop key (PI decision on C1) | any time |
| F5 | drawObject clears the objects that share its place | first |
| F6 | loops that reset the value they wait on (open choice) | decision only |
| F7 | the model check without settle (D5) | last |
| F8 | two instances of a script started with R get two slots | with the port |
| F9 | an actor's room is a fact where every write is a constant | before F1 |
| F10 | the bridge dumps slot locals | now (bridge, outside scumm/) |
| F11 | dialog lines are verbs by number | with F1 |
| F12 | forks by value sets; mouse values as ranges | before F1 |

## F1. An input is one record, and it starts at the input script

**Engine.**
- A frame (scumm.cpp:3107-3255): processInput reads key and mouse (3177); updateScummVars copies the mouse into
  VAR_MOUSE_X/Y and VAR_VIRT_MOUSE_X/Y (3198, 3385-3388); every live slot takes its turn (3231); checkExecVerbs takes
  the input (3232); checkAndRunSentenceScript runs a queued sentence (3255).
- checkExecVerbs (verbs.cpp:586-692) drops input while _userPut <= 0 (590). A shown verb's key gives
  runInputScript(kVerbClickArea, verb, 1) (608-615), the same call as a left click on it. Any other key gives
  (kKeyClickArea, key, 1) (663). A click gives (kVerbClickArea, verb, button) on a verb (682-686), else
  (kSceneClickArea, 0, button) in the room (687-689).
- runInputScript runs VAR_VERB_SCRIPT's script with [area, value, button] (script.cpp:1478-1511); from
  _numGlobalScripts on it is the room's local (70-86). runScript first stops a running copy of that number (65-66),
  not a script the copy chained to.
- The virtual mouse is the mouse plus the scroll (input.cpp:425-426), and y is -1 off the main screen (431-434).
- findObject returns the first object, in the room's list order, whose box holds the point, that is not
  Untouchable and whose parents have the right states (object.cpp:557-597). So O is found on its box minus the
  boxes of touchable objects listed before it.

**MI1 evidence.** Global 4 is the default input script; the hover loop, global 23, writes the object under the
mouse each frame ([0075], [0121]); global 4 queues the sentence at [0407]. In the bar, local 202 hands a room click
on 316 to local 203 ([001C]-[0023]); other room clicks with Var[196] < 3 test the cook and may start local 216
([0034]-[005B]). Local 203 has three paths after doSentence(STOP) ([0000]) and the walk: cook in room 28 and
x > 310, start 215 and 11 and stop ([0025]-[002B]); cook in 28 and x <= 310, stop 216 and 217 and chain to 4
([0030]-[0055]); cook not in 28, WaitForActor(ego) ([003A]), then start the door's own entry and stop
([003E]-[0053]), with no sentence script. The step 5 note's "handed on once the cook is away" is wrong.

**Now.** A sentence starts the sentence script with [verb, A, B]; the input script never runs; the mouse is unknown
(model.py:120-129, 1492-1536; reader.py:67-80, 1342). The reader pins sentences to an idle or "~frozen" sentence
script (1514-1532) and findObject's conditions at the start (1534-1560).

**Measured.**
- 8 global input scripts; 15 rooms install 17 local ones, and globals 108, 152 and 140 install locals 201 and 204
  in any room. 10 of the 17 can end a click with no global input script and no sentence. In rooms 2, 18, 20, 21,
  28, 42, 51, 57 and 85 the input script, or one it chains to, can yield.
- They test objects by number (9), the area or verb, the actor under the mouse (rooms 2, 21, 51), the mouse
  position (room 57 [000B]-[0012], room 48 [0038]), findObject's result (rooms 18, 42, 48, 85), actor rooms,
  positions, costumes and walk boxes.
- 26 of the 130 sentence and walk moves have a room-local input script. In 7 of the 130 the sentence script's
  object copies at done do not match the scored action (check-part1-row5.log:426): S1#7, S1#22, S2#9 (room-local)
  and S1#17, S1#96, S3#50, S3#65 (global 4). The step 3 note's 4 counts only misses traced to this.
- Step 5: 41 of 42 walk clicks on the kitchen door failed; the plan reached the goal by timing.
- probe_cells.py: 997 boxed objects in 73 rooms make 750 cells (distinct sets of boxes over a point), 330 under 2+
  boxes. Room 28: 52 objects, 26 cells. Room 41: 13, 11.

**Rule.**
1. Inputs (C1): a left or right click on a target; a shown verb's key, the same input as its left click; Esc; '.'
   only if C1 allows it (F4). Other keys reach the input script as kKeyClickArea and are not inputs. In MI1 those
   are global 4's keys 63, 23 (it starts global 130, the game-end goal, [0257]-[02A6]), 7 and the debug keys, keys
   passed on to scripts 20 or 4 (globals 33, 66, room 51's local 209), and global 158's digits, which belong to the
   copy-protection script 155 that nothing starts. None is a C1 input.
2. A target is a cell of the room's box arrangement (CDHD boxes, object.cpp:1145-1150, and F5's places), a verb's
   button, or an actor. findObject at a cell is decided by facts when the call runs. An actor target is a replay
   check. Floor points stay out; this leaves out room 18's local 201 [0042]-[0093], room 48's local 206
   [002A]-[003F] and global 4's walk to the mouse [0431].
3. One record per input, in one frame (D5): the due events (F2), the frame's turns of every live, unfrozen slot
   waiting in a loop that reads the mouse, checkExecVerbs, the input script VAR_VERB_SCRIPT holds after those turns,
   and checkAndRunSentenceScript with its hold test (script.cpp:1186-1191). Anything after a yield is an event.
4. The sentence-building state is facts: the variables the installed input scripts read and write (in MI1
   Var[107]-[110], [181], [182], [394]). A sentence is whatever the scripts make of the inputs: verb then object, a
   right click's default verb, a click with no verb, a dialog line picked by a room object
   (global/script-014.txt [0000]-[0075]). The Executor's order stays in the harness (C2). The sentence-script pins
   and "~frozen" go.
5. The mouse. A cell gives VAR_VIRT_MOUSE_X/Y as ranges in room coordinates. In a room no wider than the screen
   (RMHD) the camera is fixed and VAR_MOUSE_X/Y follow; in a wider room VAR_MOUSE_X is a position non-fact. A verb
   gives VAR_MOUSE_X/Y from its rectangle and VAR_VIRT_MOUSE_Y = -1.
6. The mouse stays after the input. Its cell is a fact, `mouse`, in a room no wider than the screen, else a
   non-fact after the input's frame. The mouse loops' later turns are events, not no-ops: the hover acts when the
   object under the mouse differs from Var[181] ([00F9]), which global 4 sets to -1 on every verb click ([035B]).
   Their findObject reads `mouse` and facts; their actorFromPos is a replay check.
7. A yielding input script is a slot like any other. A new input stops a running copy of the same number only.
8. Esc's exception (reader.py:236-244) existed because the input script forked on sentence variables no fact held.
   Under F1 they are facts, so it gets a switch and is measured, and dropped if it fits.
9. Inventory items are the slot verbs; their variables are facts. The reader still skips the inventory hook.

**Size first.** v1's "x1 to x3" had no probe, and running global 4 for Esc once tripled the kitchen's Esc records
(770k of 1.12M operators). A click on B forks per (verb, A) and per B cell, since the hover's
startObject(Local[0], 90) ([0147]) uses B as a number. So F1 starts with a count-only reading of room 33 and the
kitchen (records, operators, peak memory; F1 alone and with F12), reported to the PI before the full build.

**Tests.** New rows for the 130 sentence and walk moves, run from run B's own inputs. S1#7, S1#22, S2#9 lose their
var:103 miss; the other four are traced (S3#50 and S3#65 ran sentences on 601 and 600, not 603; room 48's local
206 starts local 205 with exactly those objects, [0013]-[0025], [003F], [007A]). The sentence-script readout
must agree with the engine on 130 moves (123 now). Differential, one input from a run B state: the bar door clicks of S1#7 (frames 236-274); the same with the cook in room 0 (frame 674: 203 waits
for ego, then the door's entry, no sentence script) and at x <= 310; a bar click elsewhere (does 216 start); room
85 on 914 and another exit (S2#9, S1#25); room 57 inside and outside [000B]-[0012]; S1#22's click; key 23 (no
record). Replay: the kitchen door clicks stop failing.

**Cost.** Tables: cells, verb rectangles, room widths, mouse loops. Facts: `mouse` and the input variables. Records:
measured first.

## F2. Where in the frame an input is read (open choice)

**Engine.** processInput acts on two keys itself: Esc's abortCutscene, which tests the armed override before the
frame's scripts (input.cpp:1423-1427), and '.' (1418-1421). Esc's code then goes on to the input script (1428;
verbs.cpp:663). Everything else is read in checkExecVerbs (scumm.cpp:3232), after every slot's turn (3231).

**Now.** Dialog clicks read VAR_VERB_SCRIPT, VAR_USERPUT and the verb's state at pre (model.py:1586-1622), Esc after
the turns (1738-1815). Switches input_after_turns and esc_key_at_pre flip them. The reader pins them at the start.

**Measured.** input_after_turns on row 4: 73.4% (639/870) against 73.2% (702/959); 8 dialog moves lose their
certain predictions because on some paths a click-frame turn turns input off (var:53 1->0 in S1#34, S1#35, S1#53,
S1#54, S1#55, S2#6, S2#7, S2#8). The engine took all 8. esc_key_at_pre keeps the certain predictions, but differs
from row 5 in 13 of 250 moves (paths, ends, possible, esc, turns).

**Recommendation: one rule, "after the frame's scripts"** (input_after_turns on, esc_key_at_pre off), except what
processInput does itself (Esc's override test, '.'). It is the engine's order; F1 needs it (the hover writes the
clicked object in the click's frame); the 8 moves are check errors about where slots stand, which F7's trace
removes.

**Planner rule (new).** The engine runs a slot whose wait is over before the input of the same frame; the planner
could order it after. So a due event goes first:
- A live, unfrozen slot at a wait whose exit test reads only facts is due while the test holds. The reader keeps
  due:<unit> by conditional effects on the operators that write its position or the test's facts (no axioms,
  task.py:39).
- An input whose reads or writes meet that slot's next part requires due:<unit> = 0.
- A time-only wait is never due as a fact. An input it could change carries a replay check (the wait did not end
  this frame). This replaces step 3's assumption that such a wait holds in the click's frame (184 skips).

**Tests.** New rows: rows 4 and 5 with input_after_turns. Reader: counts of due preconditions and time checks.
Replay: no input fails because a due event ran first. **Cost.** One due fact per fact-polled wait that meets an
input; counted in the ir crate first.

## F3. Engine conditions in the input's frame (S3#45)

**Engine.** findObject tests Untouchable when it runs (object.cpp:563). S3#45's pre is frame 7237; its inputs land
at 7325-7327; room 61's local 201 clears the class of 744 on its next turn ([0011]).

**Now.** actions.usable reads the class at pre (actions.py:185-209; check.py:408). In at least 8 of 44 sentences
and 21 of 86 walks the deciding input lands more than a frame after pre (3 and 14 more than 30 frames).

**Rule.** No engine condition of an input is read before its frame: findObject's at the call (F1), checkExecVerbs'
after the turns (F2). In the check an input starts from the engine's state just before its frame. That start state
is a switch, so rows 1-2 still reproduce with it off.

**Tests.** S3#45 accepted; the 29 late-input moves scored from their frame; S3#45's clicks from frame 7324.
**Cost.** No code beyond F1, F2, F7.

## F4. '.' is the talk-stop key (PI decision on C1)

**C1.** C1 does not list '.'. The PI decides whether it is a player input. Without it the model still needs msg
for WaitForMessage, but the planner cannot end text early.

**Engine.**
- The '.' branch works while VAR_TALKSTOP_KEY != 0 (input.cpp:965, 1418-1421). It sets _talkDelay = 0 and does not
  set _mouseAndKeyboardStat, so no script runs. (Esc and the keypad branches do set it; v1 was wrong there.)
- displayDialog (scumm.cpp:3288; string.cpp:1114-1129) then ends the message or prints the next part after a wait
  code. One '.' ends one line. It does nothing while the camera moves (string.cpp:1045-1048).
- An actorTalk in the same frame sets _talkDelay = 0 and _haveMsg = 0xFF (actor.cpp:3529-3531), so that '.' ends
  nothing.
- WaitForMessage reads VAR_HAVE_MSG (script_v5.cpp:3288-3291), copied from _haveMsg at the next frame
  (scumm.cpp:3382) or set to 0xFF by actorTalk at once. Only text slot 0 talks (script_v5.cpp:3448-3458).

**Now.** '.' is scored as a wait that plays the whole cutscene (check.py:22; model.py:1538-1549). VAR_HAVE_MSG is a
time variable (model.py:434-437) and WaitForMessage forks. The kitchen reading has 11,409 "message gone" checks
(reader.py:1311). 20 '.' moves (160 hits, 44 misses at row 5); the bridge pressed '.' 160 more times before done.

**Rule.** msg, the lines left, is a fact: a talk on text slot 0 sets it (wait codes plus one, or unknown for a
built text); stopTalk clears it. A line's end is an event: by time (timer check) or by '.', and it needs the camera
still (replay check). '.' is offered while msg > 0: with no message displayDialog returns at once
(string.cpp:1040-1041), so this prunes a no-op and is labelled as such. WaitForMessage tests msg = 0; the one-frame
lag holds because the slot passes on a later turn.

**Tests.** New rows for the 20 '.' moves. Differential: '.' with a line showing, after one and two frames; per line
of a multi-line print; while the camera moves; in a frame that starts a new message. **Cost.** One fact, one event
kind, no new forks.

## F5. drawObject clears the objects that share its place

**Engine.** o5_drawObject (script_v5.cpp:1031-1129): with setXY the object's place and walk point move (1109-1114);
every object of the room with the same x, y, width and height goes to state 0 (1122-1126); then the object gets its
state (1128). Boxes come from CDHD and reload with the room (room.cpp:195). (1092-1098 is a Loom workaround.)

**Now.** Only the drawn object's state is set; setXY is ignored (model.py:3728-3742). 131 boxes are shared (525
objects, 41 rooms); 427 of 541 constant sites may clear another (651 pairs), 91 decided from constants; 85 objects
have several places, all 409 setXY sites constant. 5 check misses: 324-327 in room 28 (S1#6, S1#12) and 360 (box
shared with 359 and 361).

**Rule.** objpos:O is a fact for one room visit (CDHD place and constant setXY targets), reset on room load.
drawObject(O): set objpos:O on setXY; every object of the room with O's size and place goes to 0; then O's state.
Constant places give plain writes, others conditional effects. Walk-to-object and distance read the moved walk
point.

**Tests.** New rows for paths that draw objects (the 5 misses flip); reader goldens for rooms 28 and 41; a frame of
the bar animation (locals 208, 209); synthetic: two objects with one box, a setXY onto another's place.
**Cost.** Up to 85 facts, no new forks.

## F6. Loops that reset the value they wait on (open choice)

**Engine.** A loop writes X = c, yields, tests X. Only another slot or the input script can change X between passes
(script.cpp:968-987).

**Now.** _test_reads makes such loops time-only, so settle exits them by time (model.py:916-1025). const_polls
reads X as polled: 99 of 314 loops reclassed; row 4 75.1% (696/927) against 73.2%; four moves change (S2#15-17,
S3#14), where settle predicted the third repeat on the first click. The reader already reads X as a fact.

**Recommendation: on.** Engine-true, matches the reader, removes predictions the model could not know. It touches
only settle, which F7 removes, so: record the decision, port the switch with its golden
(check-part1-row4-const_polls.jsonl), recompute nothing.

## F7. The model check without settle (D5)

**Engine.** No settled state: every live, unfrozen slot runs once per frame, and input is read every frame.

**Now.** Rows 3-5 settle (model.py:166-260) and compare at done; the reader has no settle (reader.py:206). Row 3: 58
of 78 misses are values the engine reaches later; before the first yield it is 97.8% (178/182).

**Rule.** An input is scored at the end of its frame; each later turn as an event, in the frame the engine ran it.
When: the engine's trace. What: the model, from the engine's facts and the slot's locals before the turn. Paths
whose slots stop where the engine's did not are dropped; scores are given with and without that pruning, since the
planner has no trace. The model never predicts when a time-only wait ends.

**Ground truth.** Run B's Part I, 9,539 frames: per frame and slot the unit, status, offset, freeze count and
locals (F10); per frame the fact diffs. 992 inputs up to done, 744 in sentence and walk moves (130 first, 614 more;
789 up to post). The bridge stops before checkAndRunSentenceScript (scumm.cpp:3254-3255), so compare at the next
frame's stop.

**Tests.** With every fix and F3's start state off, the done-frame readout reproduces rows 1-2. 1,000 sampled
(frame, slot) turns against the engine. S3#45, the 29 late-input moves, the 7 sentence-script moves, F2's 8 moves.
**Cost.** About 90,000 short turns; a trace under 30 MB.

## F8. Two instances of a script started with R

**Engine.** An R start skips stopScript (script.cpp:65-66) and takes a new slot; stopScript stops all instances
(262-282). **Now.** One slot per number: 50 slots in 31 moves share (step 3 note); the reader assumes one instance
per unit (reader.py:114-116). **Rule.** Machine: one slot per start. Reader: count R starts of a unit that can
still be live, as replay checks until the count says more. The text has 20 R start sites; one is the hover's
start of an actor's script (global/script-023.txt [0155]); it matters only where that script can yield.
**Tests.** Goldens for the 31 moves; two hovers over one actor in consecutive frames.

## F9. An actor's room as a fact

**Engine.** Only o5_putActorInRoom (script_v5.cpp:2160-2196) and, for the ego, o5_loadRoomWithEgo (1857-1883)
change an actor's room. **Now.** getActorRoom is a non-fact (reader.py:232); 32 paths fork on it (rows 4-5 note).
**Rule.** An actor whose every room write is a constant room (with a constant actor, or an actor variable of finite
domain) gets room:<actor>, domain its start and the rooms written. 355 of 499 putActorInRoom sites are constant;
the cook's four are (room-028-bar entry [0076], local-205 [0017], local-211 [0000], local-216 [000B]).
**Tests.** New rows for the 32 paths; local 203's three paths hold room:6. **Cost.** At most one fact per actor.

## F10. The bridge dumps slot locals

**Why.** The bridge dumps no locals (reader.py:102-112), so 2-6 live slots have args 0 at every replan. 31 sites
save VAR_VERB_SCRIPT in a local and 36 restore it (e.g. room-028-bar/local-220.txt [0000]/[19F5], local-222.txt
[00C5]/[0544], global/script-140.txt [00D6]/[0C4E]). The domain then guesses a constant (reader.py:116-119), and
after a bar dialog the planner can pick 4 and skip locals 202 and 203.

**Rule.** The bridge dumps each live slot's locals. At a replan a slot whose locals match a built list takes that
args:<unit>; otherwise its events are read again from the dumped locals and added to the task. **Tests.** After a
bar dialog the restored VAR_VERB_SCRIPT equals the engine's; slots with args 0 at replans go to 0. **Cost.** A
bridge change; a few events per replan.

## F11. Dialog lines are verbs by number

Dialog lines are VerbOps(Var[100], [..., On(), Key(Var[230])]) over a verb-number loop (560 sites); the arrows 109
and 110 are verbs too (global/script-014.txt [0086]-[00A3]); a line's key is a verb key. The reader lists only 109
and 110 (step 5 note, item 6). **Rule.** A VerbOps on a variable verb number makes the verbs of that variable's
domain at the site; every shown verb is an F1 target. **Tests.** Dialog records by number; the 60 dialog moves map
to run B's clicks. **Cost.** None.

## F12. Forks by value sets; mouse values as ranges

**Rule.**
- A fact read that reaches only comparisons forks into the value sets they separate; per value only where used as
  a number (object or script number, index, other arithmetic).
- Mouse values are ranges; +, -, * and / by constants map ranges to ranges (integer division). A test decided on
  the whole range is decided; otherwise the range splits at the bound, each part a replay check on where to click
  (global/script-023.txt [001D]-[0043]; global/script-149.txt [0009]; room-086-cu-naviga/local-204.txt [0009]).
- A mouse value used as a position is a position non-fact: walks to the mouse (global/script-004.txt [0431],
  global/script-002.txt [01B5]), a stored x (room-042-underwate/local-204.txt [0022]), an actor put at the mouse
  (room-018-crack/local-201.txt [0081]-[0087]).
- findObject ranges over 0 and the scope's objects, actorFromPos over 0 and the actors.

**Tests.** Its own switch and new goldens; synthetic: a comparison chain on findObject, a mouse division, a walk to
the mouse. **Cost.** Changes forking everywhere; measured in F1's size probe.

## The two open choices

- const_polls: on (F6). Recorded as a decision; no frozen row changes.
- One input-read rule: after the frame's scripts, except what processInput does itself (F2), with the planner rule
  that a due event goes first.

## Order in Rust

0. Finish the port (D7). F10 and the F6 decision can go now.
1. F5: cells need the places.
2. F9 and F8.
3. F2.
4. F12.
5. F1 with F11: the size probe to the PI, then the build. It blocks reliable planning in the Part I rooms with
   their own input script (8 of 45 rooms, 59 of 250 moves); the kitchen reached the goal only by timing.
6. F4, once the PI decides C1.
7. F7: blocks the gate's numbers and the fix round 2 decision.
8. F3, verified on F1, F2 and F7.

## Answers to the critique

1. Keys: agreed (F1 rule 1). The left-out keys are listed; none is a C1 input; key 23 gets a no-record test.
2. One record per input: agreed (F1 rules 3-4). Disagreement: records may not "stay about as many", because B now
   forks per cell; hence the size probe.
3. Saved input scripts: agreed. I chose the bridge dump (F10): args:<unit> already covers slots the plan started;
   the gap is slots live at a replan, whose locals the engine has.
4. Local 203 and the cook: agreed and checked (F1 evidence, F9). Nine rooms yield, adding room 42 (delay at
   [002F] of local 211). "Those sides merge" was wrong.
5. Camera, arithmetic, mouse as a number, floor clicks, findObject's area: agreed (F1 rules 2, 5; F12; cells).
6. Still mouse: agreed (F1 rule 6). R slots: F8.
7. Planner rule: agreed (F2, due events first). The recommendation stands.
8. Size: agreed, measured first. Esc's exception gets a switch and a measurement, not a blind drop.
9. Goldens: agreed; frozen rows stay; esc_key_at_pre corrected; F12 and the tables have switches; F6 is a decision.
10. F7: agreed; locals and per-frame facts added; scores with and without pruning; F3's start state is a switch.
11. F4: agreed on the engine; C1 goes to the PI. "A line shows" stays, labelled as pruning a no-op.
12. Missing items: F8-F11; the time-only wait assumption is in F2 and F7; F5 moves the walk point; F1 after F5.
13. Counts: 744 inputs; the 7-of-130 source; rooms 18 and 48 added; 324-327 plus 360; model.py:434-437;
    script_v5.cpp:1122-1126; "no room can be planned" softened. A plain grep finds 31 saves, not 32.

## Sources

Notes: step3-rows1-3, step3-rows4-5, step5-replay, task-size (notes/log/2026-10-2x); notes/decisions.md; plan.md.
Results: check-part1-row5.jsonl and .log, check-part1-row4*.log, check-part1-row5-esc_key_at_pre.jsonl,
groundtruth-part1.jsonl, reader-kitchen.log, task-size-kitchen.log. Probes (IR and game files, under 100 MB) in
`probes/`:
probe_input.py, probe_draw.py, probe_cells.py; key, yield, save and R-start counts from greps over
data/scripts-dos-cd/.
