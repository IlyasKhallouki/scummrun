# E003: A path through the cloud

Date planned: 2026-10-22
Research question it serves: RQ1

## Question

Can a deterministic system turn the scripts' causes into actions with preconditions, find a path through
them from the start of *The Secret of Monkey Island* to its end, and play that path in ScummVM, with no
human route and no language model?

## Prediction (before running)

Part I (written now, by the engineer):
- After general fixes, the model check (step 3) accepts at least 90% of the human route's non-glitch
  Part I moves. Most of the moves it still rejects are gated by the input script or by where an actor
  stands.
- The planner finds a Part I plan. Its first replay fails within its first 30 actions. Replanning from
  the engine's state reaches the end of Part I on at least 4 of the 5 test seeds, and the control run
  (step 5) does not.
- The final Part I path has fewer actions than the human route's non-glitch Part I moves counted in step
  3, because the route was chosen for time, not for count.

Held-out run (step 6): Ilyas writes this before step 6, after seeing the Part I results.
- How far the path gets (Part I, II, III, IV, or finished):
- Why:

## Method

- Commit: recorded when each step runs.
- Command(s): one script per step, listed in the report.
- Game, release, settings: MI1 DOS CD (SCUMM v5), decompiled scripts in `data/scripts-dos-cd/`. Replays
  use the patched ScummVM build (its CD timer runs on engine ticks), boot parameter 0, no debug flag and
  ScummVM's MI1 enhancement workarounds off.
- Seeds, fixed now: tuning uses seeds 1-5. The Part I result and the held-out run use seeds 31-35, which
  are never used for tuning. Once Part I has run on them, there is no more tuning on them; if tuning has
  to resume, new test seeds are fixed first. The system's own runs never use the Any% seed 836557856,
  because it was chosen to suit the human route. Only step 3 uses it, to replay the recorded human run.
- No inputs are given before the first idle stop (step 2), so the intro does not shift the random state.
- Held-out data: Parts II-IV. Everything is built and tuned on Part I, and Parts II-IV are used once, in
  step 6. One exception: the game-end rule (step 2) was found by reading MI1's ending, which is in
  Part IV.

This replaces E001 steps 2-4. With no size cap (D4), the cloud already holds all 170 goals, so E001's
recall measure no longer separates good analyses from bad ones. The 170-goal key stays as a check on
the path.

### Steps

Each step ends with a stop point: its numbers are written down before the next step starts. Steps 1 and
3 are built together: step 3's checker first, then the reader's rules one at a time, with acceptance and
effect precision measured after each, as a growth table like E002's.

1. **Actions.** E003 has its own reader, started from a copy of E002's `CloudIndex`. It turns the
   scripts into actions with preconditions and effects.

   Action kinds:
   - **Sentence**: a verb with object A and, for two-object verbs, object B. The path starts in the
     sentence script installed in the current state, with its inputs bound to the verb, A and B. It
     follows that script into whichever entry it starts: the verb's own entry, the object's default
     entry, or another object's entry (as with Give). B ranges over the constants the reached handler
     compares it with, plus one "other" value. An entry for a verb that no button shows is reached only
     this way, never as an action of its own. Room exits are sentences on exit objects.
   - **Dialog line**: a script that loops on `breakHere(); unless (X) goto back` is waiting on X, and
     "script S waits at W" is a fact. A dialog-line action needs S waiting at W and the line shown (the
     verb command that turns that line on, with its guards). Its effects are the input script's write to
     X, then S's code from W with that value, up to S's next wait or its end.
   - **Wait**, and **walk to** an object: these reach the later parts of running scripts (below) and are
     checked at replay. Walking to a spot that is not an object needs the room's box geometry, which
     the engine state does not give; it is left for later.
   - **Skipped cutscene**: an action whose path passes a `beginOverride` has two variants. Watched: the
     cutscene body, then the code after it with `VAR_OVERRIDE` = 0. Skipped: the writes up to the body's
     first yield, then the code after it with `VAR_OVERRIDE` = 1. The Executor presses Esc for the skipped
     variant.

   How a path is read:
   - in order, against a store of its own writes. A test on a value the path has already written is
     decided by the store; only tests on values not yet written become preconditions;
   - calls keep their call-site conditions and arguments;
   - a started script contributes its writes up to its first yield. Each later part is an event with
     the precondition "script n at yield k" plus the test its loop polls. It is reached by a wait, or by
     a walk when the test reads the player's distance to an object. A script that ends makes
     "script n running" false;
   - a room change runs the old room's exit scripts and then the new room's entry scripts. A room-object
     or room-local script's path ends at the room change, because the engine kills those scripts;
   - a handler that can end early becomes one action per path; other branches become conditional
     effects;
   - constant expressions are folded into values.

   Facts:
   - bits, variables, owners, states, classes, running scripts and wait points, the room, and the current
     values of the hook variables that choose the input and sentence scripts;
   - a variable whose writes are all constants, or itself plus or minus a constant once call arguments
     are bound, is a finite integer. Its values are the start value closed under its writes, up to the
     largest constant it is compared with, plus one "above" value. Every comparison becomes a set of
     values;
   - not facts: tests on actor positions, timers, random draws, script locals across a yield and values
     the reader cannot work out. They are counted, and checked at replay. Walk-box flags and room-local
     scripts reset when a room loads, so they are not kept across rooms.

   Report: actions per kind, facts, preconditions per action, how many conditions could not be facts and
   why, and for each action kind how many Part I goals it can write.
2. **Start and end.**
   - Start: the engine's own state at the first stop from boot where the player can give a sentence
     (the bridge's idle stop, not its control stop), with the settings above. Everything the boot script
     does is inside this state, so boot is not an action. This also excludes the boot script's debug-only
     branch, which writes 31 of the 99 Part I goals directly.
   - Part I end: the key's last three Part I goals (#97-99, the facts written when Part I hands over to
     Part II). This is a human input, used only to tune on Part I, and it breaks E001's rule C3. The
     report says so.
   - Game end: a cutscene that is still open when its script's last loop starts, so the game never
     leaves it. This rule was found by reading MI1's ending, so it is fitted on MI1; Monkey Island 2
     tests it. Report how many MI1 scripts match it.
3. **Model check (gate).** Compare the model with a recorded human run: run B (no item slide,
   `runs/anypercent/noslide-p7`, verified from boot in `verify-b1`), replayed from boot on its own seed
   and stopped at each Part I move's recorded end frame to read the engine's state. The route is a
   measuring instrument here, not an input to the system, and this use counts as Part I tuning.
   - The count is over the route's player-action moves, leaving out the moves tagged as glitches in
     `routes/anypercent/README.md`. Moves with no action kind are listed separately.
   - Acceptance: the share of moves the model accepts in the state the engine is in.
   - Effect precision: the share of the model's predicted facts that the engine shows after the move.
   - Effect recall: the share of the tracked facts the engine changed that the model predicted.
   - Go on to step 4 only when both acceptance and effect precision are at least 90%. Fixes must be
     general rules that name no MI1 number, and no fix may be made to accept a glitch move. If the gate
     still fails after 5 rounds of fixes, stop and report to Ilyas.
4. **Path.**
   - Relevance: working backward from the end, keep the actions that can contribute to it through some
     chain. This removes nothing that could matter, so it is not a cap. Report how many actions and facts
     are left.
   - The action builder writes the task in Fast Downward's finite-domain input format. A write of a value
     the reader cannot work out becomes one effect per possible value, marked so that a mismatch there
     leads to a replan, not a ban. A replan changes only the starting state, so the task is translated
     once.
   - Planner: Fast Downward (already in `third_party/downward`, deterministic), `lama-first`, which looks
     for any path and handles order and facts that become false again. Each replan gets 10 minutes.
     Landmarks, the facts every plan must pass through, are read from its debug output.
   - Report the task's size. Stop and write up if the task cannot be built in 32 GB.
5. **Replay (Part I).** Play the plan with the Executor's real clicks, not with pushed sentences, which
   skip the input script and the click checks. Use `Executor.play()` with no fork, so early inputs,
   reclicks and delay retries are off and every search in the engine shows up in the counts below.
   - An action ends at the first idle or menu stop. It then runs on until its predicted facts hold, or
     for 600 frames (3,600 for a wait).
   - If nothing happened, the action is played once more after the click gate reopens. An Executor error
     counts as nothing happening.
   - Match: the predicted writes, and the changes to facts the model tracks.
   - On a mismatch at a test on facts: ban the action for the values of the facts its path tested, plus
     the room. The ban is a precondition in the task, so it holds wherever the planner meets those values,
     then replan from the engine's state.
   - On a mismatch at a test on positions, timers or random draws: retry after a wait, up to 3 times,
     before banning. Random draws are seen in the engine's state at the next replan.
   - Stop at the end, when no plan is left, at 500 replans, or after 24 hours. Record the stop reason.
   - Control: the same run with the preconditions taken from the scripts removed (only what makes an
     action usable in the engine is kept), on the same seeds and budget.
   - Verify: replay the recorded inputs from boot in a fresh engine, with no fork and no change to the
     random state, and check that it reaches the same end.

   Report per seed: reached the end or not, the stop reason, replans, mismatches per executed action, the
   share of executed actions whose observed effects matched the model, actions in the final path, Part I
   goals the path passes through (of 99), time per step, the same numbers for the control, and the
   landmarks, as input for the optimization step that follows (D4).
6. **Held-out run.** Freeze the code and record the commit, Ilyas writes the prediction, then run steps
   1, 2, 4 and 5 once on seeds 31-35 from the start to the game's end. The game end is the only goal; the
   Part I end is not used. The budget is 2,000 replans or 96 hours per seed; Parts are used only for
   scoring. Afterwards, run step 3 on
   the route's Parts II-IV as a measurement only. It gates nothing.

### Constraints

- C1. Only player inputs: clicks on visible verbs, objects, inventory items and dialog lines, walking,
  waiting, and Esc while a cutscene can be skipped. This excludes cheat keys without naming them. A path
  that uses anything else is a false shortcut and counts as a failure.
- C2. E003's code names no MI1 object, script, room or variable number; engine constants from ScummVM's
  source are allowed. E003 imports nothing from E001, `search.py` or `ScriptIndex`, and reads the
  player's actor number from `VAR_EGO`. The check at the freeze covers every module E003 imports,
  directly or not. Disclosed exceptions:
  - the replay harness: the Executor's MI1 interface profile (rooms with no verb bar, the verb, hover and
    inventory variables, the actor limit, the early-changing variables) and the bridge's verb ranges for
    actions and dialog lines. The Executor was developed on the human route through all four Parts. A
    later experiment must derive the profile from the scripts (RQ4);
  - the Part I end, used only for tuning;
  - the game-end rule, fitted on MI1's ending.
- No human route as input: E003 does not read `routes/`, `steps.toml`, `pddl/part1/` or `docs/part1/`
  while it runs. The route is used only to measure, in step 3 and when scoring.

## Metric

- Primary: whether the engine reaches the end, verified from boot, with no false shortcut. Part I is
  scored by goals #97-99. The game is scored by the human route's final-fade condition, which the system
  never sees.
- Model, not engine: the full model against the control, and mismatches per executed action.
- Secondary: actions in the path, key goals the path passes through, time per step, stop reasons.

## Success threshold

- Part I, yes: on at least 4 of the 5 test seeds, the engine reaches the end of Part I, verified from
  boot, with no false shortcut. The control either fails on those seeds or needs at least twice the
  replans.
- Part I, the engine did it: if the control does as well as the full model, the report says the path
  came from search in the engine, not from the scripts.
- Part I, stop: if the model check stays under 90% after 5 rounds of general fixes, or the planner finds
  no Part I plan, stop before step 5 and write up which moves the model rejects or which preconditions have no
  cause.
- Held-out: the furthest Part reached on each seed, verified from boot. Finishing the game on at least
  4 of the 5 seeds answers RQ1 yes for MI1, under the disclosed inputs above.
