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

A record *matches* at the first frame, at or after the last input frame of the child, where all its final writes hold
and every tracked record that precedes it has matched at that frame or before. Records are examined in plan order, so a
record and its predecessor can match on the same frame. Unobserved facts (`args:`, `saved:`, and `actorroom:` with the
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
- **mismatch**: a slot of a unit with a pending record stands at a wait point (`pos:U`) it did not hold at the fork and
  that no pending record starts at or predicts, or the room becomes one no pending record predicts. A record outside
  the plan ran. The child is kept for B1;
- **spent**: an idle stop of the bridge (`Branch.run(idle=True)`'s rule: input on, no cutscene, no sentence, input or
  object script, no message wait, ego standing) more than one frame after the last input, while the input's record is
  unmatched or a needed record belongs to a unit with no live slot. Nothing that could still run the record is alive;
- **pruned**: Q ≥ the best success so far before it succeeds. It cannot finish lower, since Q only grows;
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

So b = 4 · δ_lo · (n + chain). Every success the search sees is checked against it: a success below its fork's Q + b
would refute A2 or the chain's reading, and is reported.

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
- **End.** The window ends where a fact the macro step reads (observed facts only) changes in the idling parent after
  the input's precondition has held in the parent, or 3,600 frames after the start. An empty window is a model flaw for
  B1, not a retry: the harness replans from the parent at the window's end.
- **Early successes.** A success forked at a frame where the input's precondition did not hold in the parent is kept,
  since it is an engine run, and logged as an A1 counterexample for B1.

## 6. The guarantee

For one macro step, let g_s be the frame the search stopped forking (the stop, the end, or the goal). Every fork frame
g in [start, g_s) is tried, directly or through the child it equals (Lemma 5). The search returns the success with the
least Q over them, ties to the earliest fork. Given A0, A1, A2 and the scope's δ_lo, no fork at a later frame of the
window succeeds with a lower Q (section 5, Stop).

Not claimed:

- that the earliest finish of each macro step gives the shortest route. A later finish can catch a script at a better
  point for the next step. That holds only where waiting never helps (Orda and Rom 1990); C5 handles the general case;
- anything for runs that leave the scope, or for success outside section 3's definition.

## 7. The run

After a success the chosen child's snapshot at its last input frame becomes the harness's engine, its input events join
the run's inputs, and the records it matched before that frame are done. The rest stay pending for the next window. At
a mismatch-free success whose guessed write took another value than the plan's (the harness's rule), at an empty window,
and when the plan is played out without the goal, the harness replans. Checkpoints for verify come only from frames
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
unchanged.
