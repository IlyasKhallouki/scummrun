# Window search: the engine picks the frame (plan step A4)

The replay harness plays a plan step on the first frame the model's precondition holds, waits for its writes up to a
limit, and retries after fixed waits. The window search instead tries the step's input on every frame, each on a forked
child, and keeps the child that completes first on the game clock. This note states what the search returns and why, for
any SCUMM v5 game under the bridge. It was written before the code (`window.py`, the harness switch `window_search`).

## Setting

- E, K, P, C and F are as in `fork-exact.md`: the patched engine under a pinned configuration, a parent branch stopped at a
  frame, its forked child, and a fresh engine with the same inputs.
- Q(f) is the bridge's run clock at the stop of frame f: every quarter-frame waited since boot (`SpeedrunBridge::onWait`,
  the stop status's `qf`). It is the objective. Q only grows.
- A plan is a list of steps s_1 … s_N. Each step is an action of the task whose record r_i is an *input* (sentence,
  walk_to, dialog line, skipped cutscene) or an *event*. R_i and W_i are the facts r_i reads (its precondition, the
  conditions of its conditional writes, the facts it steps or copies from) and the facts it writes.
- Premise A0 (measured, `fork-exact.md`): a child forked at frame g is the run in which the same inputs land on the same
  frames as in the parent, and whatever the child is then given lands on the frames it is given.

## 1. Macro steps

**Interference.** For i < j, s_i ⊳ s_j if W_i ∩ R_j, R_i ∩ W_j or W_i ∩ W_j is not empty. ≺ is the transitive closure
of ⊳. Inputs are kept in plan order: the engine takes one input a frame, and other input orders are C4's subject.

**Lemma 1 (non-interfering steps commute in the model).** If neither of two adjacent steps interferes with the other,
swapping them leaves both applicable and gives the same state. *Proof.* Neither writes a fact the other reads, so each
one's precondition and conditional writes see the same values in both orders. They write disjoint facts, so the final
state is the same. (Plan deordering, Bäckström 1998.) So any order of the steps that respects ≺ is a plan of the task
with the same predicted end state, and an input may be issued before an earlier event it does not interfere with.

**Assignment.** Let I_1 … I_m be the plan's inputs. An event e belongs to

- the macro step of the last input it follows, max{k : I_k ≺ e}, if there is one;
- else the macro step of the first input it precedes, min{k : e ≺ I_k} (a wait that input needs);
- else the last macro step.

Macro step k, M_k, is I_k with its events. A plan with no input has one macro step with no input.

**What a macro step needs.** A record of an earlier macro step that has not matched yet when window k forks is *pending*.
Window k's success needs M_k and every pending record that precedes (≺) a record of M_k. Other pending records stay
pending; the last macro step needs all of them.

## 2. The matcher

Each record's predicted writes are computed from the plan's state before it, the start state with the earlier steps'
predicted writes applied in plan order (`replay.expectations`, `replay.after`). By Lemma 1 this equals the state before
the record in any order that respects ≺, on every fact the record reads or writes.

A record's *final writes* are its predicted writes on facts no later record of the tracked set writes. Writes a later
record overwrites are not checked: within one frame a slot can pass a wait point that does not block, and the frame end
shows only the later value.

**The checked writes (causal links).** Of its final writes, a record's match checks those on facts that a later step
of the plan reads before any later step writes them (a causal link from the record to that step), those on the goal's
facts that no later step writes, and the write of its own slot (`pos:U` for an event of unit U; for an input, the
`pos:` writes of its path). Its other final writes are not checked.
**Lemma 8 (the unchecked writes do not change what the plan needs).** Let the plan's steps after r be s_1 … s_m. If a
fact k that r writes is read by no s_j before some s_i writes it, and is not a goal fact that no later step writes,
then the value k holds after r changes no precondition or condition that s_1 … s_m read, nor the goal's test. *Proof.*
Each s_j reads k only after some s_i (i < j) has written it, so it sees s_i's value, not r's; the goal reads k after
the last write, which is not r's. This is the triangle table's kernel (Fikes, Hart and Nilsson 1972), the condition plan
monitors check during execution (Fritz and McIlraith 2007). □
*Why the checked writes.* The engine runs every event that comes due, the model only the events the plan names: a slot
of a unit no plan step reads runs on, in the same frame or later, through events of the reading the plan leaves out
(its events are may-steps). On the kitchen, the room entry's record predicts a local of the new room at its first wait
point, and the engine shows that local one wait point further at the end of the entry frame, for every child: the first
matcher never matched the record, and 1,576 of the first 1,600 children of that window ended as mismatches on that slot
(`results/window-first-runs.log`). The own slot's write is kept as the evidence that the record ran: the slot yields there
and runs at most once a frame (`runAllScripts`' `didexec`, `script.cpp:971-984`), unless a later tracked record of the
same slot writes it, which the final writes then leave out.

A record *matches* at the first frame, at or after the last input frame of the child, where all its final writes hold
and every tracked record that precedes it has matched at that frame or before. Records are examined in plan order, so a
record and its predecessor can match on the same frame. An input's record matches only in a branch where the input
was given: in the child that plays it, and in every branch of a later window (the parent and its children), since
those all descend from the snapshot taken at that input's last frame. (The first code matched an input's record only
in its own child. An input whose final writes hold only after its last frame then stayed pending, and every later
window needs it, since the next input interferes with it: no later window could succeed. `test_window.py`,
`test_an_earlier_input_matches_once_issued`.) Unobserved facts (`args:`, `saved:`, and `actorroom:` with the
switch for the actors' rooms) are not matched, as in the harness.

## 3. A child and its outcome

At frame g the parent forks C_g. C_g plays the macro step's input through the Executor as the harness does (camera wait,
ego check, the Executor's play or the dialog line's verb click, with the stepping stones on forks when that switch is on).
Its events land on frames after g. It then runs one frame at a time until one of these holds:

- **success**: every record the window needs has matched, at frame h; c = Q(h);
- **goal**: the goal holds at frame h and every tracked record of the plan that writes a goal value (a *goal record*:
  a predicted write of a goal value that changes the fact) has matched; a success with c = Q(h), and the run ends
  there;
- **goal, not from the plan**: the goal holds and a goal record has not matched, or none is tracked. The engine reached
  the goal by a path the plan did not predict, or by waiting. It is not a success; it is logged with its frame and Q as a
  baseline for B1. Where it happens in the idling parent, the run stops there with that reason: the goal came from
  waiting, and the frame is a timeout baseline, not a plan;
- **mismatch**: a slot stands at a wait point (`pos:U`) it did not hold at the fork and that no pending record starts at
  or predicts, where `pos:U` is read by a pending tracked record (its precondition) or U is a pending event's own unit;
  or the room becomes one no pending record predicts. A record outside the plan wrote a fact a pending step needs. The
  child is kept for B1. (The first rule also checked every slot a pending record's predicted writes name; by Lemma 8 a
  slot no pending step reads may run on through events the plan leaves out, so that rule stopped children the plan
  does not depend on.)
- **spent**: an idle stop of the bridge (`Branch.run(idle=True)`'s rule: input on, no cutscene, no sentence, input or
  object script, no message wait, ego standing) more than one frame after the last input, while the input's record is
  unmatched or a needed record belongs to a unit with no live slot. Nothing that could still run the record is alive;
- **deviation**: a child that stops as spent, mismatch or limit after every needed record that precedes the input (≺)
  has matched and the input took effect: for a click, its record showed (Lemma 9, below) or another of its paths showed
  with a change a later step reads; for any other input, its record matched. Then a record the plan does not name changed what a later step needs: the plan's prediction ends
  there, not the engine run. The child runs on to the first frame h at or after its stop where the engine takes a
  click (input on, no cutscene, a room), where the replan starts; its end point is c = Q(h). *Why h and not the stop.*
  From run B's start the earliest deviation by its stop frame was a cutscene that put ego back outside the bar, 591
  frames before a click could land again (`results/window-first-runs.log`);
- **another path** (a click's input): at the frame its last click lands and at the next one, the slots the input's
  record leaves at wait points (its `pos:` writes that no later tracked record writes: the final writes of section 2)
  do not all stand there, on any of its paths. The child then ran another record of the
  reading for the same click (same verb and objects): if one of them shows (its `pos:` writes hold, and one of its
  writes differs from the fork's value) and writes a fact a later step reads to a new value, the child goes on, as a
  deviation candidate; otherwise it is spent at once.
  **Lemma 9 (a click's record shows in its own frame).** In a frame, `scummLoop` takes the input
  (`processInput`), then runs the scripts (`runAllScripts`), then the click: `checkExecVerbs` runs the room's or the
  verb's input script nested until its first wait point, and `checkAndRunSentenceScript` the sentence script likewise
  (`scumm.cpp`, the order in `time.md` §0). The record of a click is that run (the reader's F1b), so at the end of the
  frame it runs in, each slot the record leaves at a wait point stands there; a slot that has yielded runs no more in
  that frame. *Premises:* the record runs in the frame the click lands in or the next one (a verb script may queue the
  sentence one frame later: on the kitchen the door's sentence showed at the next frame, a walk to the kitchen door in
  the click's own), each such wait blocks for the rest of that frame, and no script that runs after it in that frame
  stops or moves that slot. A wait that the plan's next record of the same slot resumes from is left out: it may not
  block (a walk to a door ego already stands at passes the sentence script's wait for ego in the same frame, and the
  first version of this check stopped 3,478 of the first 3,500 children of that window). A record that showed is
  the input taking effect; a match of the record's other writes that already held does not count as that. Under it, a
  child whose record's slots do not stand there at that frame did not run the record's path, and cannot match it
  later. A wrong premise loses children (a success the search would miss), never a route: what the search keeps are
  engine runs. The count of children stopped this way is reported.
  *Why.* A door click whose path depends on where another actor stands (a non-fact the reader reads as a replay check)
  fails on most frames. Each failing child walked to the door and ran to the idle stop, 30 to 80 frames, before it
  counted as spent: the kitchen's first door window from run B's start took 417 children and 12,148 child frames
  (`results/window-first-runs.log`).
- **pruned**: Q ≥ the best end point so far (a success, else a deviation) before it ends. It cannot end lower, since Q
  only grows;
- **limit**: 3,600 frames after the last input;
- **interface**: the Executor raised (no control, the object not clickable).

Success is defined over these outcomes: a completion that passes a mismatch, or needs a dead unit restarted after the game
went idle, is not counted.

## 4. The lower bound b

b is a lower bound, in quarter-frames, on Q(h) − Q(g) for every success of a child forked at any g in the window. It does
not depend on g.

**Lemma 2 (the frame floor).** In SCUMM v5 a frame waits 4 · max(1, var 19) quarter-frames before its scripts run
(`scumm.cpp:2821-2835`, `waitForTimer(delta * 4)`; `_scrollDeltaAdjust` is set only by the FM-Towns renderer,
`gfx_towns.cpp:251`). Room changes and fades wait more, never less. So Q(f+1) − Q(f) ≥ 4·δ_lo when var 19 ≥ δ_lo at
frame f+1. *δ_lo*: the reader keeps var 19 at its start value where no unit of the scope writes it (`costs.rs`
`FrameCosts::of`, `scope.writes`), and the reading records it as `frame_costs.dmax` below 15. Then δ_lo = dmax for every
run that stays in the scope. Otherwise δ_lo = 1. Every stop the search sees checks var 19 (`v19`) against δ_lo; one that
differs ends the run with that reason.

**Lemma 3 (the input floor).** An input with n clicks lands its last event at frame g + n at the earliest: the Executor
schedules its first event for the frame after the fork, and the engine takes one input a frame (`input.cpp:439-459`).
The verb key may be skipped when the verb is already selected, so n counts clicks only (1, or 2 with a second object; a
key or a dialog line is 1). The matcher looks at no frame before the last input frame, so h ≥ g + n.

**Lemma 4 (slot chains), given A1 and A2.** Let the input's record leave unit U's slot at wait point W_1 (its write
`pos:U = W_1`), and let e_2 … e_t be events of U in the macro step, in plan order, with e_j resuming at W_{j−1} and
leaving the slot at W_j. Then e_t's writes appear at frame ≥ g + n + Σ_{j≥2} cost(e_j), where cost is the record's frame
cost (`costs.rs`: the frames from the slot's yield to its turn). *Proof.* The input's record runs at or after frame
g + n and yields at W_1 in that frame. A slot runs at most once a frame (`runAllScripts` sets `didexec`,
`script.cpp:971-984`), so it resumes from W_1 no earlier than cost(e_2) frames later (A2: each cost rule is a lower bound),
yields at W_2 in that frame, and so on. A1 (the reader over-approximates the engine) makes the engine's path one of the
records', and the matcher's success needs e_t's final writes. The chain bound is the largest such sum over the units
the input's record leaves at a wait point. A link through a step with several paths, or through a write that is not a
plain value, ends the chain.

**Lemma 4′ (chains through the slots an event starts), given A1 and A2.** Add links from any event x of the macro step
on a chain: where x writes `pos:V = W` for a unit V other than its own (it starts V, which runs to its first wait point
in the same frame, or puts it there), x links to the first later event of the macro step that is an event of V
resuming at W, has one path and leaves V at a point other than W. Such a link adds max(0, cost(e) − 1). The chain bound
is the largest sum over the paths of links from the input's record. *Proof.* x runs in frame f_x, and V stands at W at
its end or runs once more in f_x: a slot started from inside `runAllScripts` has not run in that pass yet and may be
reached by it (`script.cpp:971-984`), and from then on runs at most once a frame. A paused slot (a delay) is skipped
until a later frame's `decreaseScriptDelay`. So e resumes no earlier than frame f_x + cost(e) − 1, by A2, and each later
link is Lemma 4's. A link to an event whose yield point equals its resume point is not taken: the slot standing at that
point does not show that the event ran. □

So b = 4 · δ_lo · (n + chain). Every success the search sees is checked against it: a success below its fork's Q + b
would refute A2 or the chain's reading, and is reported.
*Why Lemma 4′.* A macro step can hold another slot's long wait that its input starts (a room entry starts a room's
timer). With Lemma 4 alone, b counts only the input's own slot, and the stop forks every frame up to the first
success's frame, each child running to it: about D²/2 child frames for a step of D frames.

**Lemma 6 (the walk floor). Proven under premises the search cannot yet establish, so the stop does not use it.**

*The step.* `Actor::actorWalkStep` (`actor.cpp:633-682`, SCUMM v4-v6) moves the 16.16 position by
(deltaXFactor >> 8) · scalex and (deltaYFactor >> 8) · scaley, with scale ≤ 255 (a byte). `calcMovementFactor`
(`actor.cpp:520-576`) sets |deltaYFactor| = sy · 2^16 and |deltaXFactor| = sy · 2^16 · |dx/dy|, and replaces both when
|deltaXFactor| / 2^16, truncated, exceeds sx: then |deltaXFactor| = sx · 2^16 and |deltaYFactor| < sy · 2^16. Without the
replacement |deltaXFactor| < (sx + 1) · 2^16, because the test truncates. So |deltaXFactor >> 8| ≤ (sx + 1) · 2^8, and one
call moves the 16.16 x by at most (sx + 1) · (2^16 − 2^8). With the fraction below 2^16, the integer x moves by at most
sx + 1. The same argument gives at most sy for y. The clamp at the leg's end only shortens a move.

*The frame.* `Actor::walkActor` (`actor.cpp:944-1015`) calls actorWalkStep once a frame, except where a move lands
exactly on its leg's end: v4-v6 then clear MF_IN_LEG and return 0, and the same call goes on to the next leg and moves
again. So a frame holds k moves only if k − 1 of them end exactly on a leg's end, and over a walk of L legs the frames
with a move number at least the moves less L − 1.

*The bound.* Let ego stand at p at the fork, walk to T, and let the record resume at the sentence script's wait for ego.
Premises:

- (W1) ego's walk speed stays at most (sx, sy) until it arrives;
- (W2) until it arrives, ego moves only by this walk's steps: no `putActor`, room load, other walk or `stopActorMoving`
  on ego, from any script;
- (W3) the walk ends at T: T lies in a box, so `adjustXYToBeInBox` leaves it;
- (W4) the walk has at most L legs.

Then the moves number at least max(⌈|T.x − p.x| / (sx + 1)⌉, ⌈|T.y − p.y| / sy⌉), the first comes no earlier than the
input's last frame g + n (`walkActors` runs after the scripts in `scummLoop`), and the wait passes one frame after the
arrival (the scripts run before `walkActors`, so the wait sees the arrival in the next frame). So the record runs at
frame ≥ g + n + max(⌈|T.x − p.x| / (sx + 1)⌉, ⌈|T.y − p.y| / sy⌉) − (L − 1).

*Why it is not used.* The dump gives ego's position (`ego` in the stop status) and T (the object's walk point in
`room_objects`), but not ego's speed, the room's boxes or the walk's legs. W1 and W2 are properties of every unit the scope
can run: scripts set walk speeds from 1 to 255 (`ActorOps WalkSpeed`) and put, walk or stop actors through variables. No
check of them exists yet. The bound must also hold for children the search never runs, so it cannot be checked after
the fact. For each success the search records the frames from the last input to ego's arrival, which is what the stop
would have saved with such a bound.

## 5. The window

- **Start.** The window of macro step k starts at the frame the previous input's last event landed (the parent is that
  child's snapshot, taken at its last input frame), so the first fork lands its input one frame later (D5). Macro step 1
  starts at the plan's start.
- **Forks.** The parent idles one frame at a time and forks a child at every frame where the engine takes the input on
  the next frame: for a click, the Executor's gate (input on, no cutscene, a room, verbs on screen or a room without
  them); for a dialog line, input on and the verb on screen and hit; for a key, every frame.
  **Lemma 5.** A child that sends no event up to frame f is, at frame f, the parent at frame f (A0, with no input given).
  A child forked at a frame where the gate is shut waits for the gate with no input, so it is the child forked where the
  gate opens, and nothing is lost by not forking it. For the same reason, two children whose first event lands on the same
  frame are the same run (the Executor's choices depend only on the state it reads); the second is dropped.
- **Stop.** With best success c*, the search forks no more once Q(g) + b ≥ c*. Any fork at g' ≥ g succeeds at
  Q ≥ Q(g') + b ≥ Q(g) + b ≥ c*, so it cannot do better (Lemmas 2-4). Children already running finish or are pruned.
  The search keeps the earliest end point, success or deviation (a success at equal Q). With a deviation c_d the
  earliest, it forks no more once Q(g) + 4 · δ_lo · n ≥ c_d: a fork at g' ≥ g deviates after its input took effect,
  so no earlier than frame g' + n (Lemmas 2 and 3), and succeeds no earlier than Q(g') + b ≥ Q(g') + 4 · δ_lo · n.
  *Why deviations.* On the kitchen the room entry's record never matched in any child: in each, a slot of the new room
  that the plan leaves alone started one of several other local scripts (which one varied from child to child), and a
  later step reads that script's state. In the first runs 1,576 of the first 1,600 children ended as mismatches, and in the
  next ones every child after the door opened ended spent: the window ran on with no success (`results/window-first-runs.log`). The plan
  is wrong there, not the frame; the earliest child that shows it is the earliest state to replan from.
- **End.** Let P be the tracked records that precede the input
  (r ≺ I_k, section 1). The parent is *ready* at frame g when every record of P has matched in the parent by g (the
  matcher, section 2, run on the idling parent). The model's window for I_k is open at g when the parent is ready and
  some path of I_k's precondition holds on the parent's view, with the plan's values of the unobserved facts. The
  window ends at the first frame where the model's window was open before and no path's precondition holds, or
  3,600 frames after the start. An empty window is a model flaw for B1, not a retry: the harness replans from the
  parent at the window's end.
  **Lemma 7 (the end never fires on the plan's own writes).** Before the parent is ready, every change the plan
  predicts on the way to I_k is a write of a record of P, and the rule does not look. After it is ready, P has matched,
  so a fact I_k reads that changes and closes its precondition is a change the plan does not place before I_k: either
  a record the plan orders after I_k (I_k ≺ r, so in every order that respects ≺ the input comes first) or no record
  of the plan. In both cases the model's own window for I_k has closed, which is the relative deadline the end
  stands for. *Proof.* The parent after the window's start is the chosen child of the previous window run on with no
  input (A0, Lemma 5), so the records that child matched match in the parent on the same frames. A write the plan
  predicts before I_k on a fact I_k reads belongs to a record r with r ⊳ I_k, hence r ∈ P or r is done. □
  The end is a cut on work, not part of the guarantee (section 6 covers the frames forked before it). A fact read by
  the macro step's events but not by I_k does not end the window: the children run such an event or show the
  mismatch.
  *Why this rule replaced the first.* The first rule ended the window where any fact the macro step reads changed in
  the parent after the input's precondition had held. On the kitchen it ended the second window one frame after its
  start, with one child forked: the previous input's own predicted write (its sentence starting) changed a fact the
  next input reads, before that sentence's later record had matched (`results/window-first-runs.log`). It would also end
  a door's window at the first step of any other actor's slot whose events the macro step holds.
- **Early successes.** A success forked at a frame where the input's precondition did not hold in the parent is kept,
  since it is an engine run, and logged as an A1 counterexample for B1.

## 6. The guarantee

For one macro step, let g_s be the frame the search stopped forking (the stop, the end, or the goal). Every fork frame
g in [start, g_s) is tried, directly or through the child it equals (Lemma 5). The search returns the end point
(success or deviation) with the least Q over them, a success at equal Q, ties to the earliest fork. Given A0, A1, A2
and the scope's δ_lo, no fork at a later frame of the window succeeds or deviates with a lower Q (section 5, Stop).

Not claimed:

- that the earliest finish of each macro step gives the shortest route. A later finish can catch a script at a better
  point for the next step. That holds only where waiting never helps (Orda and Rom 1990); C5 handles the general case;
- anything for runs that leave the scope, or for success outside section 3's definition.

## 7. The run

After a success the chosen child's snapshot at its last input frame becomes the harness's engine, its input events join
the run's inputs, and the records it matched before that frame are done. The rest stay pending for the next window. At
a deviation the chosen child's branch at its end point (the first frame after its stop where the engine takes a click:
input on, no cutscene, a room) becomes the engine, its inputs join the run's, the records it matched are done, and the
harness replans from there. Before that frame only a key can land; a skip (Esc) inside that span is not tried, a stated
loss. *Why.* From run B's start the first deviation stopped inside a cutscene, and the replan from that frame had
found no plan after 680 s of search; from the first frame after it where the engine takes a click, the replan found a
13-step plan in 17 s (`results/window-first-runs.log`, sections 5 and 6). At a mismatch-free success whose guessed write took another
value than the plan's (the harness's rule), at an empty window, and when the plan is played out without the goal, the
harness replans. Checkpoints for verify come only from frames
before the next chosen input lands: the snapshot's frame, and a child's success frame when the next input lands after
it. Verify then plays every chosen input from boot in a fresh engine with no fork.

The run's success is a stop on a goal from the plan's records (section 3), not a goal that merely holds. Its report
counts the records that matched their predictions (their final writes, in order), and, apart from that, the input steps
after which no tracked fact changed without a predicted write (plan.md's strict match, counted once per macro step on
its input).

RETRIES, RETRY_WAIT, waits-first, the camera's precondition wait and the second click try are not used. Bans are not
made: a window's children name no tested value a ban could cut.

## 8. Off

The switch `window_search` (replay_part1.py `--window-search`) is off by default. Off, the harness's code path is
unchanged. Its sub-switches are off by default too: `strict` (section 9, `--window-strict`), `cache` (9.7,
`--window-cache`), and, under `strict`, `fork_each` (`--window-fork-each N`) and `lite` (`--window-lite`). With all of
them off the search is sections 1-7. The bridge's `run` with `fork_each` and `state` with `objects` run only when a
command asks for them; with neither asked and the other controls off, the bridge's status, dumps and trace equal the
shared build's (trace_check.py check-off, `results/check-off-fork-each.log`).

## 9. The strict window search (sub-switch `strict`)

Sections 1-7 count a record as matched at the first frame where its final writes hold. B1 (trace-validation.md) checked
the four runs with its strict matcher: of the 38 steps they counted as matched, 14 match on their own run; records
counted as matched 2 to 11 frames before their run, or with no run. With `strict` on, the search uses B1's matcher.
This section states what changes, for any SCUMM v5 game under the bridge. It was written before the code (`strict.py`).

### 9.1 What a branch observes

- With `segment` on (trace-validation.md section 2; Lemma O: it observes only) a branch reports every slot run. R_k
  are the runs between D_{k−1} and D_k (trace-validation.md section 1). The parent turns `segment` on at the window's
  start and reads R_k after each frame. A child inherits it with an empty buffer (the parent reads its runs before it
  forks) and reads its runs after each frame it observes.
- V_k is the window's view at D_k (FastView), plus `args:U` for each unit U whose `args:` a tracked record reads or
  writes: the set of the reading's built lists at U's wait point that U's dumped locals match, {0} when none does
  (B1's view, trace_check.py `Reading.view`). The engine runs with SPEEDRUN_DUMP_LOCALS, which observes only
  (bridge_locals.py).
- `explain` (Lemma O) is armed in the parent for every unit, and in a child for the units of the window's needed
  records and of the input's `pos:` writes, from the frame after the fork. The logs are read with the runs. They are
  kept only to explain a failure (9.5).
- A child observes every frame from its fork when a tracked record that does not follow the input (≺) is pending at
  the fork (its run may come while the input code plays: the input code then runs one frame at a time); else from the
  frame its last click lands, as before. *Why this is enough.* A record that follows the input cannot match before the
  input's record has, so only the others can have their own run before the input. The view before the input that the
  input's record reads is the fork's (its first event lands on the frame after the fork, section 5).
- Running one frame at a time or several at once gives the same run: a stop at a decision point waits no
  quarter-frame and writes no game state; the bridge reads commands only while the engine is stopped. Verify from boot
  already relies on this: it reaches the window's checkpoints, taken one frame at a time, in larger steps.

### 9.2 Definition S: a record's own run

- **Event.** For an event record r = event(U@W), r's own run in a branch is the first run of U at depth 0 with c0 = W
  that is not a stutter (trace-validation.md section 3: one opcode, a WaitFor op, c1 = c0), after the record that left
  U at W matched, or after the window's start when no tracked record of U before r is pending. r *matches strictly*
  at that run's frame k when M1-M4 hold for r on it (Definition M: V_{k−1}, V_k, R_k and the open keys of
  trace-validation.md section 3) and every tracked record r follows (≺) has matched before. Otherwise r is *refuted*.
  **Lemma S1 (one run per event record).** A refuted record never matches. *Proof.* Any two of the plan's records of U
  write `pos:U`, so they interfere and ≺ orders them in plan order. In the model, U's slot waits at W from the record
  that left it there until r runs. The engine's first run of U from W after that is therefore the step the plan says is
  r's. A later run from W needs U to come back to W, which is a step the plan does not name. □
- **Input.** Let an input record I land its clicks on frames f_1 ≤ … ≤ f_n = s, with f_1 − 1 the fork's frame, and let
  U_I be the units its `pos:` writes name. *The click's runs.* A slot run at depth 0 is a slot `runAllScripts` resumes;
  a run at depth 1 or more came through `runScriptNested`, which starts a script: `runInputScript` from
  `checkExecVerbs`, `checkAndRunSentenceScript` for the sentence on top of the queue, a `startScript` inside them
  (script.cpp; the segment's depth is `vm.numNestedScripts` when the run begins, trace-validation.md section 1). I's run
  of U ∈ U_I is the first run of U at depth 1 or more in a frame from s on. I is *decided* at the first frame k ≥ s + 1
  where each U with a write `pos:U` = w ≥ 0 has had its run; it *matches strictly* at k when:
  - (I1) pre(I) holds on V_{f_1−1}, after the set writes of the tracked event records that matched strictly before
    I's first run (`runAllScripts` runs before `checkExecVerbs` in a frame, so a slot run in R_s comes before the
    click's work);
  - (I2) for each `pos:U` = w of I, I's run of U ends at w; for w = NOWHERE and no run, U has no live slot on V_k;
  - (I3) each other set or guessed write of I holds on V_k, unless a run of R_k after I's last run is at a point whose
    records write that key (M4);
  - (I4) each check of I that the dump at D_{f_1−1} decides (`replay.eval_check`) takes I's side (M3).

  (I1)-(I3) are B1's check of an input step (trace_check.py window-validate), with M2's order of runs and the click's
  own runs: B1's check took the last run of each unit in R_s ∪ R_{s+1}. On the kitchen from run B's S1#7 the click on
  the door comes while the stepping stone's sentence still waits for ego: the sentence script's runs there are that
  sentence's blocked wait at the same point, and the click's sentence starts once it ends. The last run then passes or
  fails for the other sentence. (I4) is M3.
  *I took effect* when it matches strictly and one of its writes changed: a `pos:U` whose run left U elsewhere than
  V_{f_1−1} has it, or another write whose value on V_{f_1−1} is not among its predicted values. A match on writes that
  all held before is no evidence that the click ran its path (the check's item 3: walk_to(316)#10846 passed Lemma 9 on
  `pos:script:4` = NOWHERE, which held before the click).
- **Success.** A child succeeds at h when every record the window needs has matched strictly by h, in the child or, by
  the fork, in the parent. A goal stop needs the goal records matched strictly. Writes that held before the fork never
  complete a macro step, since each needed record needs its own run (the check's item 6: window 2.2 succeeded on its
  click frame).
  **Lemma S2.** On the view's facts, a strict success is a run of the needed records: each ran once, after the
  records it follows, with its precondition before its run and its effects after it (Lemma M, trace-validation.md). □

### 9.3 The child, strictly

- **The click is decided when its runs have come (Lemma 9′).** From frame s + 1 on, at each frame the child collects
  the click's runs. Once I is decided: if it matches strictly, the child goes on; else, if another record J of the same
  click (same verb and objects) is decided, matches strictly and took effect, and one of its writes that changed is a
  fact a later step reads, the child goes on as a deviation candidate; once neither I nor any J can still match, it is
  spent. At the bridge's idle stop (input on, no cutscene, no sentence queued, no input or object script running, ego
  standing) a unit whose run has not come gets none from this click: I fails on "no run".
  *Proof that this loses no success and no deviation.* The click starts its work only through `runScriptNested`, and
  the child gives no other input after it; so a unit's first such run from frame s on is the click's. The first one is
  I's, and so no later run of U is: if I does not match once decided it never does. Every needed record that follows I
  then cannot match, and the window needs I: no success. With neither I nor another record of the click taking effect,
  the child is no deviation (section 3). Until I is decided, no record that follows it can match, so going on loses
  nothing. At the idle stop nothing queued or running is left to start U for this click. □ The first version tried the
  click's other records at frame s, before the plan's own record had its second frame (the check's item 6: a record
  that showed one frame late could never succeed); here the plan's record is decided first.
- **Refuted.** A child stops as *refuted* when a needed record is refuted (Lemma S1: no success). For the deviation
  rule it counts as a mismatch. A needed record refuted in the parent ends the window: no child forked after can
  succeed.
- **Ties (the check's item 6).** At each frame a child first decides success and the goal, then the prune: it is
  pruned at h when Q(h) ≥ c* and it has not ended at h. *Proof.* Q grows by at least 4 quarter-frames a frame (Lemma
  2), so a child that has not ended at h ends at Q > c*. One that ends at h with Q(h) = c* is compared by end_key: a
  success before a deviation, then the earlier fork. □ The deviation's walk to the click gate tests the gate first,
  then the prune. The first version pruned first: a child ending at Q = c* was dropped, and which of two equal ends
  won depended on which thread finished first.
- **The parent's stop at a tie.** With a deviation c_d the best, a later fork can still win with a success at Q = c_d.
  The parent stops forking once Q(g) + b > c_d and Q(g) + 4·δ_lo·n ≥ c_d. With a success c* the best, a later fork
  loses every tie to the earlier one, and Q(g) + b ≥ c* stops as before.

### 9.4 The end, strictly (the check's item 5)

**Lemma 7′.** The parent is ready when every record of P (section 5, End) has matched strictly in it or was done
before the window. Then Lemma 7's proof holds: each record of P has had its own run, so every write the plan predicts
before I_k on a fact I_k reads has happened, and a later change of such a fact comes from a run that is no record of
P. □ In the first version a record of P could count as matched before its run. In window 2.3 the previous step's record
counted as matched before it ran; its run then changed a fact the input reads, and the window ended after one fork, on
the plan's own write.

### 9.5 The cause of each replan and each empty window

A child keeps, for its first strict failure, the frame's runs, explain log, dumps and views. The failure is one of:

- a needed event record refuted: its own run, and the first condition of Definition M that failed for it;
- the input's record not matched once decided: the first of (I1)-(I4) that failed, with the click's run of the
  `pos:` unit it names, or the unit with no run;
- a mismatch (section 3): the run that left the slot where no pending record predicts;
- a needed record whose run did not happen before the child stopped: where its unit's slot waits, and on what.

At a deviation the chosen child's failure is explained; at an empty window, the failure most children share, on the
first child that had it; at a guessed write the engine took otherwise, the run of that record and the key. A goal
that holds with no goal record matched is explained by the run that wrote the goal's fact (the parent's explain log
has every run). The explanation is B1's, on the kept log: trace_check.explain_unexplained (Lemma X, the first opcode
off the records' path; Lemma B, the backward slice; the two classes), and for an input's `pos:` end the same Lemma X
toward the record's own end. It is logged with the replan and the window: the record, the frame, the unit, the code
offset, the opcode, the engine's values and the class.

A run stops, instead of replanning, when a window is empty for the second time on the same input record with the same
cause: the plan made from that state is the same, and so is the window.

### 9.6 The guarantee

Section 6 holds with success read as strict success (9.2): for one macro step, the search returns the strict success
or deviation with the least Q over every fork frame up to its stop, ties to a success and then to the earliest fork.

### 9.7 The cost

**The approach once, in the parent: not exact.** A click on an object off screen plays an approach first (floor clicks,
a stepping stone; executor.py `_approach`). Doing the approach once in the parent and forking only the click would
search the runs "approach from the window's start, click at c", not "approach from g, click at g + a". Those are the
same runs from the click on only when the state at the click frame does not depend on when the approach started. On
the kitchen from boot's door window (window2's run) it does: with the chosen child's approach started at the window's
start (frame 2798) instead of its fork (3076), the state dump at 3149, the frame before the door click, differs in 14
places, among them the camera (440 against 480), the random state, four object states and vars 0, 2, 20 and 100. The
room's camera script pans on ego's x (local 201 at [0000], `getActorX(VAR_EGO)`), so the camera at the click frame
depends on when ego walked. The approach stays in each child (approach_check.py, `results/approach-check.log`).

**The cache (sub-switch `cache`, off by default).** A child's input code asks the engine for its state dump and for
hit tests (`at`) several times at one stop. *Lemma C.* Between two runs of a branch its game state does not change:
the bridge reads commands only while the engine is stopped, a scheduled input lands on a later frame, and `segment`
and `explain` observe only (Lemma O). So a second dump or hit test at the same stop gives the first's answer, and the
child keeps them until its next run. Its first dump is the parent's at the fork: a forked child is the parent at that
stop (A0). □ The search's runs, and so its result, are the same with the cache on or off; only the number of dumps
and hit tests the engine makes changes, and the run reports both counts.

**Forks ahead (sub-switch `fork_each` = N, strict only, off by default; plan step A4's kill (2)).** The parent runs N
frames with one command, and the bridge forks a child at each of their decision points (`run` with `fork_each`).
*Lemma F.* Each such child is the child the `fork` command makes at that decision point: the bridge makes it with the
same steps (it closes the room file, forks, the child closes the parent's files, connects and reports "forked") at the
same place in the frame, and the parent goes on as it would one frame at a time (9.1). So the child at frame f stands
for the parent at f (A0): the window reads frame f's view from it and, where the gate is open, uses it as the child
forked at f; one the window does not use is quit. The parent's runs and explain log for the N frames come in one read
after the command, grouped by frame (R_k). When the window ends at frame f, the child at f is the engine from there.
□ *Test.* On the kitchen from boot's route at frame 3000, seven children from one `fork_each` and seven forked one
frame at a time: full dumps (locals, exact) equal at the fork and 30 frames after the same click on each
(`results/fork-each-check.log`).

**The lite dump (sub-switch `lite`, strict only, off by default).** After its click a child reads its view each frame
from the bridge's `state` with `objects`: the dump's vars, bits, verbs, slots (locals included), cutscene stack and
actors as `state` gives them, with the owners, states and classes of the window's objects only, and no room objects or
inventory. *Lemma D.* For every key of the window's view (FastView) and for `args:` (ArgsView), the lite dump gives the
values the full dump gives: facts_of reads each key from those fields, and the window's objects are every object whose
owner, state or class the view reads, and those whose script's locals it reads. replay.eval_check (M3) reads vars,
actors and the room, which the lite dump has. □ The checkpoints and a child's unplanned facts read every fact, so at an
end point the child takes a full dump. A cause's values on an object outside the window read as "not in the dump".
*Test.* On the kitchen from boot's route, every frame from 2728 to 3219: the lite dump for the reading's 46 objects
against the full dump (`results/lite-check.log`).
