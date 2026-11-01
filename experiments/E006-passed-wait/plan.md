# E006: A run that passes a wait, matched as two records

Date planned: 2026-11-01
Research question it serves: RQ1 (E003's path through the cloud, its step A4 under B1's strict matcher)

## Question

Every strict run from the boot (kitchen, bar door, k = 2) stops at walk_to(428). The click's sentence runs past the
sentence script's WaitForActor at [02E0] in the same run, because ego is not moving, and goes through the door; the
reader ends the click's record at that wait. Reading both branches at every such wait (E004's `wait_both`) holds the
engine's run but makes the kitchen's plan 0 38 times bigger (44,605 to 1,712,065 operators), and the planner ran out
of memory at plan 0.

If the strict matcher splits a slot run at a wait the engine passed without yielding (the run's opcode log shows the
WaitFor op executed and the run going on), so that the click's record matches the part up to the wait and the
reading's existing event record for that wait point matches the rest in the same frame, do the kitchen (owner:566 =
ego) and the bar door (state:316 = 1) from the boot reach their goals from the plan's records, with the reader's
records unchanged? Then k = 2's state:316 = 1.

## Baseline

Branch walk at eaca9ca (E004, `notes/log/2026-11-01-e004-walk.md`), seed 1, the harness's boot start (frame 2728),
the strict window search with every cost switch (`run.sh` of that branch):

| test | stop | frames from the start | children | search CPU s | strict matches |
|---|---|---|---|---|---|
| kitchen | the same empty window twice on walk_to(428)#64, at 9,976 | 7,248 (173,952 qf, 724.8 s) | 7,266 | 262.3 | inputs 1 of 3, events 1 of 5 |
| bar door (trace branch's final code) | no plan, after walk_to(428)'s empty window and walk_to(426)'s deviation into room 38 | 3,784 | 3,801 | 279.2 | inputs 2 of 3, events 1 of 4 |
| k = 2 (trace branch) | the same empty window twice on walk_to(428)#64, at 9,996 | 7,268 | 7,268 | 736.9 | inputs 0 of 3, events 0 of 3 |

Every empty walk_to(428) window has the same cause: "end pos:script:2: the wait did not block; script:2 [02E0]
WaitForActor; getActorMoving(1) = 0; class split". With `wait_both` on, neither the kitchen nor the bar door gets a
plan 0 in 3 GB. The window search without `strict` (sections 1-7 of window-search.md) reaches the kitchen's goal at
frame 3219 (491 frames, 49.10 s, two deviations), and stops on the bar door and k = 2 at 3138 and 3129 with "goal, not
from the plan" (`results/window-ahead-*.log`).

## Method

### Lemma P (a run that passed a wait is two records' runs)

Let ρ be one slot run of unit U (one `executeScript` call, trace-validation.md Lemma S), and let its explain log show
a WaitFor op executed at code offset w and followed by another opcode of ρ. Let ρ₁ be ρ's opcodes before that op and
ρ₂ the rest, from the op on. Then

- (a) the op wrote nothing, and ρ₂ starts from the machine state ρ₁ ends in. o5_wait (script_v5.cpp:3272-3310) either
  returns before any write of its own (each case's `return`), and `executeScript`'s loop runs the next opcode in the
  same call, or it rewinds `_scriptPointer` to the op and calls o5_breakHere, which ends the call. Another opcode of ρ
  followed it, so it returned;
- (b) no opcode of another run comes between ρ₁ and ρ₂: by Lemma S every opcode is the innermost call's, and ρ₁'s last
  opcode and the WaitFor op are consecutive in ρ (a run nested in ρ₁ has returned before the op);
- (c) ρ₁ is the run of a path from ρ's start that stops at w with U's slot there. The reader ends a record at every
  WaitFor op it does not resume at (machine.rs `Reader::wait`), so a record whose path ρ₁ follows has `pos:U = w` as
  its end, if w is the first WaitFor op ρ executed (the record's path meets no WaitFor before its end);
- (d) ρ₂ is the run of a path that resumes at w and passes the op at its first test, which is what an event record
  event(U@w) holds: its path starts at the op, with the check "`<op>` blocks" on the side false (the reader's
  `resume_wait`).

So on the facts the views hold, the engine's ρ is the model's run "a record r₁ that ends at w, then r₂ = event(U@w)
with no step and no frame between them", whenever r₁ matches ρ₁ and r₂ matches ρ₂ strictly. The same holds at each
later WaitFor op ρ passes: ρ splits into ρ₁ … ρ_{m+1} at the m ops it passed, part j ≥ 2 an event record's run from
the op that begins it. An op at ρ's own start is not a split point when ρ resumed at it (c0 is its offset): there the
event's record already starts at the op. □

**Each part matched strictly on its own** (window-search.md 9.2, with the split):

- Part 1 of a click's run of U: (I1) unchanged; (I2) the click's run ends at w by its log (the first WaitFor op it
  executed is at w, and the run went on); (I3) as before, with the later parts counted as later runs between the same
  decision points: the keys their records write, and U's `args:` fact, which part 2 moves with U's slot; (I4) on part
  1's opcodes only.
- Part j ≥ 2: Definition S's own run of the plan's next event record of U at w_j (armed by the record that left U at
  w_j: the click's record for part 2, part j − 1's record after that). M1-M4 as for any event run: M2 on the view at
  the decision point before ρ, with the set writes of the records matched before this part applied in run order (the
  click's record's included; the keys those records guess or step are open); M3 on this part's opcodes; M4 on the
  next decision point's view, unless a later part or a later run between the same decision points writes the key.
- An event's own run at depth 0 that passed a later WaitFor op splits the same way: the event's record matches part 1
  (its end at w by the log), the plan's next event of U at w part 2.
- Nothing matches on writes that held before (9.2 unchanged): part 1 is the click's own run, part j the event's own.

### What the split asks of the plan

Part 2 is matched only to the plan's own next record of U from w, the one Definition S arms with the click's record
(`pos:U` predecessor), and only when every record it follows (≺) has matched and begun before it (9.2's order check,
unchanged). It need not be the next step in plan order: a step between them that interferes with neither commutes
with both (Lemma 1), so the plan with it moved after the pair has the same ≺. A step x with click ≺ x ≺ event fails
the order check, and the event is refuted: the engine ran nothing between the two parts.

The search does not take the event from the reading when the plan names another record of U at w, or none. A strict
success is a run of the plan's needed records (Lemma S2), and the plan's later steps were chosen against that record's
effects (their causal links); a record the plan does not name is what section 3 calls a deviation, after which the
replan picks from the engine's state. No new rule is needed for the case at hand: the kitchen's plan 0 already puts
event(script:2@736#da28…)#510 right after walk_to(428)#64, in the same macro step, since the door's code is in that
event's path. The reader ends every record at a WaitFor op, so any plan that uses the click's sentence past w holds an
event of U from w.

### Costs

Under the split r₂ runs 0 frames after r₁'s end. The costs do not allow that now:

- costs.rs gives an event at a WaitForActor, WaitForCamera or WaitForSentence 1 frame ("the slot ran in an earlier
  frame"; 433 wait points in the game's code) and at a WaitForMessage with no talk before it 1 frame (258), and
  says every cost is a lower bound of the record's own duration;
- window.py's chain (Lemma 4, the bound b) adds max(1, cost) on a link through the input's or the same slot.

Fix, behind switches:

- reader switch `wait_pass_cost` (costs.rs): those wait points cost 0 frames. A WaitForMessage after a talk keeps its
  bound: actorTalk sets VAR_HAVE_MSG to 0xFF (actor.cpp:3466-3540) in the same run before the op, so the op blocks at
  least once. breakHere and delay always yield. Two yields with one wait point keep the lesser bound, now possibly 0.
  It needs `--frame-costs`;
- window sub-switch `split_wait` (below): the chain adds the reading's cost on such a link as it is, 0 allowed. The
  harness refuses `split_wait` on a reading made without `wait_pass_cost`, whose costs would not be lower bounds under
  the split. Lemma 4′'s max(0, cost − 1) is unchanged.

Other places that read the costs: replay.py's cost caps only lower a cost; trace_check's wait check (Lemma W, k − j ≥
cost) reads the reading's costs and holds with the reader switch, and is not run here. The duration rules on
origin/timed are not used.

### The switches

- `split_wait`: window.WindowConfig sub-switch under `strict`, replay_part1.py `--window-split-wait`, off by default
  (strict.py and window.py). Off, every strict run log is unchanged.
- `wait_pass_cost`: reader-scope `--switches`, off by default. Off, the reading is byte for byte the one before.

### Runs

- Code: branch e006 from origin/walk (eaca9ca). The bridge: svm-w3b's ScummVM build, read only. The reader built
  under `~/.cache/fast-mi-e006/target-scumm`; taskcore is walk's build, unchanged.
- Off checks, before any run with the switches on: the reader with `wait_pass_cost` off reads the kitchen to the
  packed sha1 of the walk branch's kitchen reading (47583c54); the strict kitchen from the boot with `split_wait` off,
  on that reading, gives the walk branch's log: stop, windows, children (7,266), outcomes, cause.
- Readings with `wait_pass_cost` (read-rust, the kitchen's switches, `--reach --frame-costs`, 3 threads): kitchen
  (33, 28, 41; owner:566 = 1), bar (28, 33; state:316 = 1), k = 2 (28, 29, 33, 34, 35, 38, 41, 83, 85, 96;
  state:316 = 1).
- Tests, each from the harness's boot on seed 1 with the walk branch's `run.sh` switches plus `--window-split-wait`,
  each verified from boot in a fresh engine with no fork: the kitchen, the bar door, then k = 2.
- Reported per run: whether the goal came from the plan's records, game time (qf, s, frames), strict matches for
  inputs and events, replans with their causes, search CPU, peak memory (Python RSS, the forks' PSS, the planner).
  Where a run stops further on: the next blocker, its cause and class (window-search.md 9.5).
- Seed 1 only. Laptop, nice 10, 3 threads; a step expected above 2 GB peak (the k = 2 reading) runs under the heavy
  lock; a step stops if MemAvailable falls under 3 GB.

## Prediction (before running)

| | predicted |
|---|---|
| off checks | kitchen packed sha1 47583c54; the kitchen's strict log as walk's (7,266 children, the same stop) |
| reading growth with `wait_pass_cost` | records and actions unchanged; only costs change: the 691 wait points of the two rules cost 0 |
| kitchen, walk_to(428)'s window | no child fails on "the wait did not block"; the click's part matches; #510 matches or is refuted on M4 on a local of room 28 that a unit the plan does not name starts in the same frame (the non-strict search's deviation there: local:208); the window ends at frame 2798 ± 2 with under 100 children (3,600 before) |
| kitchen from the boot | the goal from the plan's records at frame 3,200 to 3,300 (470 to 570 frames from the start, 11,300 to 13,700 qf, 47 to 57 s); 2 to 4 replans, the door window to the kitchen among them (the cook's x, as from S1#7); strict matches: inputs 3 to 5, events 3 to 8; search CPU 150 to 600 s; peak memory under 1.5 GB |
| bar door from the boot | past walk_to(428) the same way at about 2798; then the goal holds in the idling parent "not from the plan" at about 3138 (410 frames, 9,840 qf, 41.0 s): the next blocker is the run that wrote state:316, a unit the plan does not name, class split |
| k = 2 from the boot | past walk_to(428) (a deviation at about 2799, after sentence(2,428)'s on var 102); then "goal, not from the plan" at about 3129 (401 frames), as the bar door |
| verify from boot | 0 differences on all three |

## Success threshold

- Yes: with both switches on, no child on any of the three fails with "the wait did not block" at walk_to(428): the
  click's part matches strictly (I1-I4 on part 1) and #510 is matched or refuted on its own part; and the kitchen from
  the boot reaches owner:566 = ego from the plan's records, verified from boot with 0 differences.
- Partly: all three get past walk_to(428), and the kitchen stops before its goal at a named blocker.
- No: a test still stops at walk_to(428) with both switches on. Then the lemma or one of its premises is wrong for
  that run, and the write-up says which.
- The off checks must pass; otherwise nothing measured with the switches on counts.

## Stop rule

- An off check that differs: stop, find why, and do not run with the switches on until it passes.
- The split rejecting the kitchen's walk_to(428) click for a reason the lemma does not name: stop and write up the
  reason; no widening of the split, no other loosening of the strict rule.
- A run that stops further on: name the next blocker (cause and class from 9.5) and stop that test there. E006 makes
  no fix beyond the split and the costs.
- The harness's own limits are the walk branch's (replan limit 30, windows and children of 3,600 frames, plan memory
  3G); a run that hits one reports it as its stop. They are not decisions of this experiment.

## Results (2026-11-01, written after the runs)

Code at 1386267 (branch e006): the reader switch at 75fb3a0, strict.py at 8a7ab11, window.py and replay_part1.py at
1386267. Files: `results/off-kitchen.log`, `results/readings.log`, `results/split-kitchen.log`, `results/split-bar.log`,
`results/split-k2.log`. Lab note: `notes/log/2026-11-01-e006.md`.

Off checks: the reader with `wait_pass_cost` off reads the kitchen to packed sha1 47583c54, with the actions and lists
files byte for byte walk's. The strict kitchen from the boot with `split_wait` off gives walk's run: the same stop,
7,248 frames, 173,952 qf, 7,266 children with the same outcomes and rows, the same 27 inputs, verify 5 checkpoints and
0 differences. The one difference is the frame of the child the cause is read on (2797 against 2798), the first child
to finish with the shared failure, which depends on the workers' order. With the switch on the readings keep their
records and actions; 1,707 of the kitchen's 40,773 actions and 1,292 of the bar's 37,776 cost 0 instead of 1.

| test | stop | goal from the plan's records | frames (qf, s) from 2728 | plans, replans | children | strict matches | search CPU s | peak memory |
|---|---|---|---|---|---|---|---|---|
| kitchen | the goal at 3216 | yes | 488 (11,712, 48.80) | 3, 2 | 501 | inputs 3 of 4, events 3 of 9 | 275.1 (engines 141.0, Python 134.1) | Python 298 MB, forks 78 MB, taskcore 88 MB |
| bar door | goal, not from the plan, at 3129 | no | 401 (9,624, 40.10) | 2, 1 | 420 | inputs 2 of 3, events 1 of 3 | 43.0 (17.8, 25.2) | 276 MB, 64 MB, 72 MB |
| k = 2 | the same empty window twice on walk_to(428)#64, at 9996 | no | 7,268 (174,432, 726.82) | 3, 2 | 7,268 | inputs 0 of 3, events 0 of 3 | 359.9 (95.9, 264.0) | 736 MB, 75 MB, 1,121 MB |

Each verified from boot in a fresh engine with no fork: 9, 6 and 5 checkpoints, 0 differences; the kitchen's goal and
the bar door's hold at the end.

- At walk_to(428), no child of any of the three failed on "the wait did not block". On the kitchen and the bar door the
  window took 22 children (3,600 before); 3 read the click's run as two parts, the click's record matched on part 1
  and took effect, and #510 was refuted on M4 on its part: it predicts local:201 = 0, the engine had 1 at the next
  decision point, and none of the fact's 18 write sites is in the part (the explainer gives no class). The window ended
  in that deviation at 2798, as the non-strict search did there. On k = 2 every one of the 7,200 children read the
  click's run as two parts, and the click's record failed (I3) on var 102, written at the sentence script's [002B]
  after the test at [0000] on var 103, which the k = 2 reading holds no fact of: class split. No other record of the
  click took effect on a fact a later step reads (its part writes var 102, which already held 0, and the slot's facts,
  which part 2 moves on), so no deviation.
- Kitchen: the replan from 2798 goes to the door window, which deviates on the cook's x (local:203 [001E], var 0 = 306,
  class split) at 3204, as from S1#7; the pick-up then reaches the goal at 3216, 3 frames before the non-strict search.
  b fell from 2 frames to 1 on the first two windows (the events at [02E0] cost 0); no bound violation.
- Bar door: after the same deviation at 2798, the goal holds in the idling parent at 3129, written by local 209's
  drawObject at [0033], a unit the plan does not name; class split. That is the next blocker.
- k = 2: the next blocker is (I3) on var 102 at walk_to(428) (class split), behind the wait the split removed.

Against the prediction: the off checks, the readings, the kitchen's window at walk_to(428) (2798, 22 children; local:201
where local:208 was named) and the kitchen's run (3216, 488 frames, 2 replans, 275 s, under 0.5 GB) came as
predicted; the bar door stopped as predicted, 9 frames earlier (3129, not 3138); k = 2 did not get past walk_to(428).
By the threshold, the kitchen answers yes; the bar door gets past walk_to(428) and stops at a named blocker; k = 2
meets the letter of the no clause (it stops at walk_to(428)) but not its diagnosis: Lemma P held in all its children,
and the stop is (I3) on var 102. The stop rule did not fire.
