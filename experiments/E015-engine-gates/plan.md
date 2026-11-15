# E015: three gates in the engine, offline, on play8's archive

Date planned: 2026-11-04
Research question it serves: RQ1 (completion from the game files alone), RQ4 (what carries over to another game)

Written and committed before any code. Nothing here is run on the cloud VM, and no exploration run is made.

## Question

The retrospective of 2026-11-04 (its `report.md` §4.3, §8, §9; `ground-truth.md`; `proposal-minimal-change.md`)
makes three claims about the Fate of Atlantis interactive demo. Each can be checked in minutes on play8's archive, and
the next step depends on all three:

1. **The item claim.** The demo's end needs object 946. Object 946 is reached only through exit A of room 49 (object
   596, local 211, room 63), then room 69. Room 42 local 212, the step that consumes 946, cannot run without it. So
   play8's room-42 branch (316 of its 628 entries) was off the path. This comes from a script reading and has not been
   checked in the engine.
2. **The realization claim (G1).** A planned sentence can be turned into clicks by searching in forks and accepting
   the clicks whose run the engine's own sentence log (patch 0007) shows as that sentence. No interface profile and no
   verb ids are needed. The planner aimed at exit A 22 times, and every attempt died at its first input because the
   executor's profile uses Monkey Island's walk verb 11 where the demo's default verb is 10 (row 27).
3. **The novelty claim (G2, the hypothesis of report §9).** Novelty over what the engine shows (live script units and
   their wait offsets, the objects the hit test can reach, a camera bucket), taken together with T, keeps the state
   that tested facts T alone dropped: j00077's trial t596, the click that opened exit A's dialog. It does so without
   flooding the archive.

## Baseline (what the files already show)

- **Exit A** takes 5 clicks from the boot (`/var/tmp/fast-mi/research-scratch/gt/ev5.jsonl`). The steps are the
  opening dialog's choice, two floor clicks that scroll the camera (xstart 256, 96, 0), a bare click on object 596,
  which gives sentence(10,596), and a dialog choice other than 122. Room 63 loads at frame 1392. [engine]
- **The planner's attempts.** play7 and play8 planned sentence(10,596) 22 times, and all 22 stopped at the executor's
  first input (`executor.rs:20-47`, `DOS_CD.walk_to = 11`; assumptions row 27). [files]
- **The free step's drop.** In play8 j00077 (entry 184, frame 1154, commit 84), the free step ran 264 trials (180
  from the offer and 84 delay trials) and kept 0. Its trial `t596` reaches local 211's dialog at frame 1256, with
  `var:32 = 17`, verbs 120-123 on and `var:108 = 596`
  (`/var/tmp/fast-mi/research-scratch/gt/aff184/trials.tsv`). [files, engine]
  - No atom of T separates that dialog from the opening's. Only the waiting script differs: local 211 against
    local 207. Script-control keys are left out of novelty (`progress.rs:24`, `CONTROL_PREFIXES`).
- **play8's free steps.** 317 steps ran trials: 155 free tries and 82 model tries' free steps in room 49, and 80 free
  tries in room 42. Together they ran 111,626 trials and kept 39 (my recount of `jobs/*/result.json`). The trials' end
  states are not stored; only their counts are.
- **No return to room 49.** No room-49 entry of play8 has a room-42 ancestor (0 of 312). [files]
- **The scripts (grep only; nothing is quoted).**
  - Room 63 loads room 69 and room 49.
  - Room 69 loads room 63.
  - Room 42 loads room 75 (twice) and room 82.
  - Room 75 has four `loadRoom(49)` sites.
  - So room 42 may lead back to room 49 through room 75. Whether a player input does so is unchecked. [files]

## Method

Three gates, in this order: the item gate, then G2, then G1. G2 comes before G1 because it needs the least new code
and alone decides the hypothesis of report §9. G1 needs the most code, and its routine is the one the executor will
use later. No gate depends on another's result.

**Common to all three gates**

- **Builds.** e015 (this branch, from e014's 3e2be93) in `CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015`, release.
  The engine is `/var/tmp/fast-mi/svm-e013/build/scummvm/scummvm` (patches 0001-0007), unchanged.
- **Environment.** The game's settings come from `experiments/E014-fate-demo/a8/run-env.sh`: `FMI_GAME_ID`,
  `FMI_GAME_DIR`, `FMI_GAME_SCRIPTS`, `SPEEDRUN_IDLE=handler`, `SPEEDRUN_END_SITE=1`, `FMI_BOXES=1` and
  `FMI_OFFER_SCREEN=1`. `SPEEDRUN_INPUT=1` is added for patch 0007's input log. Seed 1.
- **Archives are read only.** These are `/var/tmp/fast-mi/e014-archives/play8` (entries, checkpoints, `log.jsonl`
  and `jobs/*/result.json`) and the ground-truth event files. Nothing is written into them.
- **Outputs** go under `/var/tmp/fast-mi/e015-out/{item,g1,g2}`. Results are written as numbers only (rooms,
  objects, scripts, vars and verbs by number; no game text) in `experiments/E015-engine-gates/results/`.
- **Tool.** All three gates run through one measurement binary, fmi-checks' `e015`, with the subcommands `item`,
  `realize`, `rescore` and `depth`. It sits outside the system, like `probe` and `afford-check`. The numbers a gate
  needs (entry ids, the sentence, the target room, object 946) are passed as arguments. The general routines it calls
  name no game number.
- **Processes.** At most 2 engine hosts at a time, each under `nice -n 10`, each running its forks one at a time.
- **Exactness.** Every chain a gate reports as reached is replayed from the boot in a fresh engine. The chain is the
  entry's movie followed by the gate's inputs at absolute frames. The replay is compared at the chain's last frame:
  every fact (control keys included), the room, and patch 0007's state digest. 0 differences is required.

### The item gate: is object 946 on the path, and does local 212 need it?

**Procedure**

1. **Reproduce exit A.** Boot, play `gt/ev5.jsonl`, run to the first idle after room 63 loads. Expected: room 63 at
   frame 1392, idle at about 1405.
2. **Find the way from room 63 to room 69.** At room 63's first idle, take afford's offer (`offer_cfg`: verb slots,
   objects not of class 32, actors, and the walk boxes' floor points). Then search breadth-first over afford's inputs
   in forks:
   - The inputs at each state are each target alone, each verb then each target, and each floor point.
   - Each trial runs to the bridge's next idle.
   - A stop in a new room ends that branch.
   - The search stops at the first stop in room 69. Limits: depth at most 4 inputs, at most 5,000 trials.
   - Every trial is recorded: its inputs, its stop, its room and the random draw (`rng`). Each opening is tried in a
     fork of the same state, so all of them see the same draw. The record shows how many openings lead to room 69
     from this state and what the others lead to.
3. **Take object 946 in room 69.** At room 69's first idle, the same breadth-first search runs, depth at most 4 and
   at most 5,000 trials. It stops when `owner:946` equals the ego's actor (the dump's ego). The search must find a
   sentence the engine accepts; no verb is named.
4. **Verify from the boot.** The chain (ev5, then the inputs from steps 2 and 3) is replayed from the boot in a fresh
   engine, 0 differences.
5. **Show that local 212's step fails without 946.**
   - **Static, grep of the extraction.** List every site that starts room 42's local 212 (`startScript`,
     `chainScript`, or a verb handler that starts it) and every site that clears class 12 on object 491. The claim
     requires that the only start site is in object 946's verb code.
   - **Engine.** Take play8's room-42 entries where `class:491:6 = 1` (the state after the step before), the
     furthest entry (356), and the first room-42 entry (296). That is at most 10 entries. From each, run afford's step
     as a try runs it (the offer, each input, and the third click when the game is left composing), then one more
     level from each stop. That is depth 2, at most 3,000 trials per entry.
   - **What is counted.** The trials whose input log or slots show local 212 started (`Started` touch or `local:212`
     live), and the trials that clear `class:491:12`.
6. **Does room 42 lead back to room 49 by player input?** From the same room-42 entries, the step-5 sweep also
   records every room change. Any stop in room 75 is followed by a depth-2 sweep from that stop, which records whether
   any input loads room 49. Each found chain is verified from the boot.

**Pass**

- 946 is reached: owner:946 = ego on a chain made of exit A, room 63's openings and room 69's inputs, verified from
  the boot with 0 differences.
- Local 212's step needs 946:
  - its only start site is in object 946's code;
  - 0 trials from the room-42 entries start local 212 or clear class 12 on object 491.

**Fail**

- 946 is not reached within the limits, or it is reached by a route other than room 63 → 69.
- Local 212 has a start site outside 946's code, or any trial from the room-42 entries starts it.

A miss on limits only (no room 69 within 5,000 trials) is reported as "not shown". It is not reported as "room 42 is
on the path".

**Not gating**

- Whether room 42 leads back to room 49 by player input: yes, no, or not within the sweep.

**Predictions**

| | predicted |
|---|---|
| exit A reproduces (room 63 at frame 1392) | 97% |
| room 69 reached from room 63 within the limits | 85%; depth 1-2 inputs; exactly 1 of the openings leads there from one state (the draw) |
| owner:946 = ego within the limits | 80% (given room 69); depth 1-3 inputs |
| the whole chain verifies from the boot | 95% (given reached) |
| local 212 has one start site, in 946's code; 0 sweep trials start it | 85% |
| item gate passes | 65% |
| room 42 leads back to room 49 by player input (through room 75) | yes 55%, no 20%, not within the sweep 25% |

**Cost**

- Code: about 40 minutes. This is `e015 item`, about 200-300 lines reusing afford's `offer_cfg`, `inputs_of` and the
  engine's forks.
- Run: under 10 minutes of CPU. The sweeps are at most 5,000 + 5,000 + 10 × 3,000 trials at about 10-90 ms each.
  In practice they are far fewer.

### G2: novelty over engine-visible state

**The feature set, fixed here before any result.** A stop's atoms are the union of five kinds:

| kind | atom | novel when |
|---|---|---|
| T | the tested facts' atoms, exactly as the run judged them (`progress.rs`'s novelty over the job's T and the rooms) | as in the run |
| U, live units | `live:<room>:<kind>:<n>` for each slot alive in the stop's dump: a global script, a room local, or an object's code (fmi-core K_SCRIPT, K_LOCAL, K_OBJSCRIPT; a room's entry and exit code excluded, as fmi-core does) | no state in the archive at the job's time, and no trial kept before it in the step, holds the atom |
| W, wait offsets | `at:<room>:<unit>:<offset>`: the offset where a live unit's slot stopped (fmi-core K_AT) | as for U, except that **ambient units' offsets never count** (below) |
| R, reachable | `reach:<room>:obj:<id>` and `reach:<room>:actor:<id>` for each object and actor the engine's hit test answers at a point on the screen at the stop (afford's `offer_cfg` targets, with `FMI_OFFER_SCREEN`) | as for U |
| C, camera | `cam:<room>:<b>`, where b = the camera's xstart (the engine's status) divided by **160** (integer division) | as for U |

- **The bucket.** 160 pixels is half the 320-pixel screen. Room 49 (640 pixels wide, xstart 0-320) has 3 buckets.
- **Ambient units.**
  - A unit is ambient at an entry when its slot's offset takes 2 or more values over a no-input fork of the entry run
    for 1,600 frames. That is E012's idle measurement, the `idle.keys` a job records, recomputed with the same rule
    where the job did not record it.
  - An ambient unit's presence still counts once, through U. Its offsets never count.
  - This rule is fixed before any result.
  - As a sensitivity check only (reported, not deciding), the scoring is also run with the other rule considered: an
    offset atom counts only the first time per (unit, cell), where the cell is the stop's cell over T.
- **Seen states.** These are every archive entry committed before the job's commit (from `log.jsonl` and the entry
  ids), plus each trial kept earlier in the same step, as `try_all` keeps.
  - An entry's T, U and W come from its stored checkpoint.
  - Its R and C come from booting its movie once and taking the offer at its frame. Each entry is computed once and
    cached in `e015-out/g2/seen/`.

**Procedure**

1. **Re-run j00077.** From entry 184, run the free step as j00077 ran it: `fmi_explore::free` with the job's
   `job.json`, its T (`result.json`'s `tested`, the same 434 keys as `gt/tested-j00077.txt`), and its delays.
   - **Hook.** A per-stop observer added to afford's `try_all` and the delay trials reads each trial's stop: the dump's
     facts and slots, the status's xstart, and one `offer_cfg` on the stop's fork. It changes no trial and no keep.
   - **Output.** One line per trial: name, frame, room, the T verdict, and the atoms of U, W, R and C.
2. **Calibration (G2-0), before any new score is read.**
   - The re-run must reproduce j00077's record: 264 trials (180 + 84) and 0 kept under T. Its trial `t596` must stop
     at frame 1256 with `local:211` live.
   - If it does not, the re-runner is fixed until it does. That is engineering and does not count as a G2 result.
   - If it cannot be made to reproduce within 45 minutes, G2 is reported as "not run", not as failed.
3. **Re-run a sample of play8's other steps.**
   - **Which steps.** The 317 job steps with trials, in job order: every 8th, starting at the first (40 steps), plus
     j00077 if not among them.
   - **Calibration.** Each re-run step must reproduce its recorded trial count and its T-kept count. Steps that do not
     are listed and left out of the score. If more than 10% of the sample do not reproduce, the re-runner is fixed
     first (as in step 2).
4. **Score.** For each trial: T-novel, E-novel (the union above), and novel by each kind alone.
   - Kept under E = E-novel and not held by any trial kept earlier in the step.
   - The kept rate is kept trials over all re-scored trials (j00077 plus the sample). It is also reported per step,
     per room, and separately for j00077.
5. **Depth to exit A (reported, not gating).**
   - **(D1)** Every 13th room-49 entry of play8 in id order (24 entries), plus entries 0 and 184. From each, an IW(1)
     search under E:
     - breadth first over afford's inputs, expanding only stops kept under E;
     - seen = the archive before the entry, plus this search's kept stops;
     - limits: depth at most 5 inputs, at most 4,000 trials, at most 120 s;
     - reported per entry: the depth at which a stop with `local:211` live is first kept, and the depth at which
       room 63 is first reached, or "not within the limits".
   - **(D2)** Along ground truth's route A (`ev5.jsonl` from entry 0, and its last 2 inputs from entry 184), each
     input's stop is marked kept or not under each of E, T∪U∪W (control state alone) and T. Reported: the longest run
     of consecutive unkept inputs before room 63, for each feature set.
   - The retrospective predicts width 1 under E (D1 finds room 63, every stop on the way kept), and at most 3 unkept
     inputs in a row under control state alone (D2).

**Pass:** t596 is kept under E, and the kept rate over all re-scored trials is between 0.5% and 20% inclusive.

**Fail**

- t596 is not kept under E, or the kept rate is below 0.5% or above 20%. Either way, report §9's hypothesis is refuted
  as stated.
- Calibration that cannot be met is "not run", not a fail.

**Predictions**

| | predicted |
|---|---|
| calibration: the j00077 re-run gives 264 trials and 0 kept under T | 80% at the first try; met within the 45 minutes 95% |
| sampled steps that reproduce their counts | at least 36 of 40, 75% |
| t596 kept under E | 85%; kept through U (`local:211`) at least, 90% given kept |
| kept rate over all re-scored trials | 1-8% (median guess 3%); in [0.5%, 20%] 75% |
| kept rate, j00077 alone | 2-15 of 264 |
| the kind that keeps most trials | U or W (R and C mostly seen already in room 49's archive) |
| G2 passes | 65% |
| D1: room 63 reached within the limits with every stop kept (width 1) | in at least 80% of the sampled entries, 60% |
| D1: depth to room 63 | 4-5 inputs from entries with the camera at xstart 256; 2 from the left-edge entries (183, 184 and the like) |
| D2: longest unkept run before room 63 | E 0; T∪U∪W 1-2 (the floor clicks); T 3 (walk, walk, the click on 596) |
| sensitivity rule (first offset per unit per cell) | kept rate 1.5-3× the primary rule's |

**Cost**

- Code: about 1 hour. That covers the observer hook, the feature kinds as a module of fmi-explore
  (`enovel.rs`, behind `FMI_NOVEL_ENGINE=1`, used only by `e015` in E015, with nothing on the run path), and
  `e015 rescore` and `e015 depth`.
- Run:
  - j00077: about 30 s.
  - The 40 sampled steps: about 14,000 trials at 20-90 ms, 5-20 minutes of CPU on 2 hosts.
  - The seen set: about 300 entry boots of under 1 s each.
  - D1: at most 26 × 120 s, split over 2 hosts, about 25 minutes of wall.

### G1: realize a planned sentence through the engine, not a profile

**The routine (general, Rust, behind a switch).** This is the module `fmi-play/src/realize.rs`, switch
`FMI_REALIZE=1`. The switch is off by default and nothing on the run path calls the module in E015.

- **Input.** An engine at an idle state, and a step: sentence(v, a, b), where b may be absent, or a dialog verb v.
- **Output.** The inputs at absolute frames (as the movie's events) and the count of candidates tried. On a miss, the
  reason and the candidates tried.
- **The engine is the only judge.**
  1. **Offer.** Take afford's `offer_cfg` at the state.
  2. **A target off the screen (the approach).** When object a has no hit point on the screen:
     - The candidates are the walk boxes' floor points the offer found, ordered by distance from the object's walk
       point (from the dump) in room coordinates. Each is clicked in a fork with no verb click and run to the next
       idle.
     - A candidate is accepted when the input log records a walk (`Walk` touch) and the camera's xstart moves toward
       the object, or the object gains a hit point.
     - This repeats from the new state, up to 6 approach clicks. Then it misses with "approach".
     - No verb is waited for. No verb id is read.
  3. **The sentence.** The candidates, in order:
     - (i) a bare click on a hit point of a, with the verb the interface has selected;
     - (ii) each offered verb slot, then a;
     - (iii) for a two-object sentence, each of the above followed by a click on b, or on each visible slot when b is
       not on the screen (the inventory case, at most 15 slots).
     - Each candidate is tried at up to 3 of a's hit points (from afford's 5 × 5 grid), in a fork, run until the
       input log has the click's handler run.
     - A candidate is accepted when the log's `Sentence` touch shows (v, a, b). The verb that a bare click gives is the
       default verb as the engine reports it. Nothing names it.
     - The search misses with "no sentence" after all candidates. Limit: 60 candidates.
  4. **A dialog verb v.** The candidate is the offered verb slot whose dump id is v. A SCUMM v5 slot's id is its verb
     number; that is engine semantics, not a game's table. It is clicked in a fork and accepted when the input log
     shows the handler took the click (verdict "taken").
- **Not used.** `DOS_CD`, `selected_verb`, `wait_walk_to`, `DIALOG_FIRST/LAST`, and any verb or variable number.
  - A grep check in the write-up: `realize.rs` has no numeric literal other than its limits and the screen's
    geometry.
- **Tests (fake engine, `fmi-play/tests/realize.rs`).**
  - A bare click gives the sentence.
  - Only a verb slot then the object gives it.
  - The target starts off the screen and comes on screen after 2 floor clicks.
  - A target that never comes on screen misses with "approach".
  - No candidate's log shows the sentence: a miss with "no sentence" within the limit.
  - A dialog verb.
  - The switch off leaves every existing test unchanged.

**Procedure**

1. **Realize exit A's step.** From play8's entry 0 (frame 992, the opening dialog), with no profile:
   - realize the opening's step: dialog verb 120, the choice play8's chain made to reach entry 2 (its click (4,154) at
     frame 994). It is given as the plan's step;
   - realize sentence(10,596), which needs the approach first;
   - realize the dialog verb 120, the step a plan for `var:165 = 120` would give.
   - The sentence and the verbs are the gate's arguments (as a plan would give them), not constants in code.
2. **Run on.** Run until room 63 loads or 600 frames pass.
3. **Verify from the boot.** Entry 0's movie plus the realized inputs, in a fresh engine, 0 differences at the last
   frame (facts, room, digest).
4. **Also reported (not gating).**
   - **(a)** sentence(10,596) from entry 184 (object 596 on the screen): candidates and wall.
   - **(b)** sentence(11,593) from entry 0. Object 593 cannot be clicked on this path (ground truth §2). Expected: a
     bounded miss, with the reason and the candidates tried.
   - **(c)** wall time per realized sentence.

**Pass**

- The input log shows the sentence (10,596).
- Room 63 loads after the dialog choice.
- The chain replays from the boot with 0 differences.
- No verb id and no profile is used, by the grep check and by reading the routine.

**Fail:** any of these does not hold.

**Fixes.** A fix is a change to `realize.rs` after a failed G1 attempt, committed with a dated note giving the cause.
At most two fixes. If G1 fails after the second fix, G1 has failed.

**Predictions**

| | predicted |
|---|---|
| approach clicks before 596 has a hit point | 2-3 (ground truth: 2) |
| candidates for sentence(10,596) once 596 is on the screen | 1 (the bare click), 75%; at most 6, 90% |
| wall per realized sentence | 0.1-3 s |
| room 63 after the dialog verb 120 | at frame 1,350-1,600 (the realized frames differ from ground truth's) |
| verified from the boot, 0 differences | 95% (given reached) |
| (b) sentence(11,593) | a miss, "approach" or "no sentence", within 60 candidates and 10 s |
| G1 passes with no fix | 65% |
| G1 passes within two fixes | 85% |

**Cost**

- Code: about 1.5 hours, about 300-450 lines with the tests. `realize.rs` builds on afford's offer and the input log
  (`Engine::input_log`).
- Run: under a minute.

## Success threshold

- **Pass.** The item gate shows 946 on exit A's path and local 212's need for it; G1 passes; G2 passes.
- **What would stop this line.**
  - **G2 fails.** The engine-first inversion is not built (decision rule 3).
  - **G1 fails after two fixes.** The minimal change stops (decision rule 4).

## Decision rules (from report §8)

1. **If the item gate shows room 42 off the path**, the demo's checkpoint ladder is as the report states. These are
   proposal-minimal-change's C0-C9:
   - C0 the opening;
   - C1 room 63;
   - C2 room 69;
   - C3 owner:946 = ego;
   - C4 room 49 holding 946;
   - C5 room 42 holding 946;
   - C6 class 12 cleared on object 491;
   - C7 room 82;
   - C8 room 48;
   - C9 object 582's restart at 0x0067.

   play8 stands at C0. If the gate does not show it, the ladder is rewritten from the gate's findings before any
   further experiment uses it.
2. **If G1 and G2 pass**, the next experiment builds proposal-minimal-change's fixes A-F:
   - A, engine-confirmed realization;
   - B, live-unit novelty;
   - C, exact-entry failure memory;
   - D, free tries without a fresh reading;
   - E, the end from a verb handler;
   - F, scope growth off and a frontier term.

   Then comes K1 (the Monkey Island kitchen by search, laptop, 60 minutes). Then play9 on the demo, scored by a
   checkpoint scorer outside the system that reads `entries.jsonl` only.
3. **If G2 fails**, the engine-first inversion (report §9) is refuted as stated, and it is not built.
4. **If G1 fails after two fixes**, the minimal change stops. It is written up, and no run is made on it.
5. **Neither of rules 2-4 applies** when G1 passes and G2 is "not run" (calibration not met), or when the item gate is
   only "not shown". The write-up then states which gate is open, and the next step is that gate.

## Stop rule

- **E015 stops after about 4 hours of work in all**, counted from this plan's commit. The planned share is:
  - the item gate, 45 minutes;
  - G2, 1 hour 15 minutes;
  - G1, 1 hour 45 minutes;
  - the write-up, 15 minutes.
- **On the stop.** A gate not finished by then is reported as "not run", with what was done and why.
- **What may not change.** No gate's procedure or criteria change after its first result. A change needed before that
  is a dated amendment, committed before the gate runs.
- **Fixes after a first result.** These are allowed only as named: G1's two fixes, and the re-runner's calibration in
  G2.
- **Disk.**
  - Builds and outputs stay under `/var/tmp/fast-mi`.
  - If / falls under 3 GB, `/var/tmp/fast-mi/target-e014b` may be deleted, after `lsof` shows nothing uses it.
  - Otherwise outputs move to the other disk and the work goes on.
- **Commits.** The code is committed in small pieces, one topic each, on e015, and pushed. The results and a report
  (`report.md`) are committed when the gates stop. Nothing is merged to main.

## Amendments

### (1) 2026-11-04, the item gate: what the procedure leaves open, fixed before its first run

Written after reading the tools and the archive's entry list, before any search of the gate ran.

- **Step 5's entries (the plan cannot be followed as written).** The plan expected at most 8 room-42 entries with
  `class:491:6 = 1`; play8 has 136 (every one also holds `class:491:12 = 1`). The rule: 8 of them evenly spaced in id
  order (the ⌊k·136/8⌋-th, k = 0..7), which are entries 302, 324, 341, 358, 456, 556, 573 and 595, plus 356 and 296.
  That is 10 entries.
- **The inputs of steps 2 and 3.** In a sentence idle: afford's `inputs_of` less the verb slots alone (the plan lists
  each target alone, each verb then each target, each floor point). In a dialog idle, where `inputs_of` gives the
  verb slots alone, those are the inputs (the plan names no dialog case). No third click.
- **The inputs of steps 5 and 6** are `inputs_of` in full with the third click, as a try runs it (`try_all`'s rule:
  `composing` after a verb-then-target trial, then a click on every target of that stop).
- **A trial's run.** The clicks two frames apart from the frame after the stop (afford's timing), then a run to the
  bridge's next idle, or to W = 1,600 frames after the stop (afford's W), whichever comes first. Only an idle stop in the
  search's room is expanded at the next depth. A stop in another room, an ended run or a stop at W ends its branch.
  For step 5's sweep, room 42 is the search's room; a stop in room 75 goes to step 6.
- **Expanding a stop.** The stop's engine is not kept. It is made again from the search's start engine: a fork, the
  path's clicks scheduled, a run to the stop's frame. Its patch-0007 digest must equal the one recorded at the trial's
  stop; a stop that does not is reported and not expanded.
- **Step 5's counts.** As planned: a `Started` touch of script 212 in the input log (the log holds the input handler's
  run only), `local:212` live at the stop, and `class:491:12` absent at the stop (it is 1 at every chosen entry). Also
  reported, not gating: a run of room 42's local 212 anywhere in the trial (the engine's segment of slot runs).
- **Step 6.** The stops in room 75, distinct by digest, in the order found, at most 10. From each, a depth-2 sweep, at
  most 3,000 trials. Step 6 stops at the first stop in room 49, which is verified from the boot.
- **Verification (step 4 and step 6).** A fresh engine boots with the movie (ev5, then every click of the chain at its
  absolute frame) and runs to the chain's last stop frame. Compared there: every fact of the full facts (control keys
  included), the room and the digest.

### (2) 2026-11-04, the item gate: the openings at the goal's state, fixed before its first run

- Step 2 says the search stops at the first stop in room 69, and also that the record shows how many openings lead to
  room 69 from that state. Both hold only if the state's other openings are tried. So at the first goal stop (room 69
  in step 2, owner:946 = ego in step 3) the rest of that node's inputs are still run and recorded, and then the search
  stops. The chain is the first goal trial. The trial limit still applies.

### (G2-1) 2026-11-04, G2: the re-runner and readings, before G2's first score

Written after the calibration (G2-0, which is not a G2 result) and before any trial was scored under E. No criterion,
feature or prediction changes.

- **The readings are not on this machine.** play8's readings (`readings/`, `jobs/*/run/`) stayed on the VM, and the
  plan forbids the VM. The free step needs a reading only for its wait (T's domains and the reading's points). So the
  re-runner (`e015-g2 rerun`, in fmi-checks; the plan's `e015 rescore` under its own name, so as not to collide with
  the other gates' `e015`) does this:
  - **T** is result.json's `tested` atoms, merged with job.json's archive T, exactly as the try merged them.
  - **The offer points** after the entry are the recorded wait's stops (result.json `e012.wait.stops`). They are frames
    of the entry's no-input run, so the engine is run there; this is exact.
  - **The wait after each trial** runs fmi-explore's own wait with domains made from T's atoms, given only to T's own
    keys (the reading's atoms, before E014's extra ones), the verbs and the goal's key. This is the closest rule to the
    recorded `observed` counts, and it is not exact: against them it has 2-3 atoms too few in 211 of the 317 steps
    (room 49), 11 too many in all 80 room-42 steps and 13 room-49 steps, and 6-15 off in the 13 others. A step whose
    T-kept count or trial count differs is excluded, as step 3 says.
- **What a trial's atoms are.** U, W, R and C are read at the trial's own stop (its idle stop, or the horizon), on a
  fork. The wait's later stops are judged under T only, as the run judged them. A trial whose run ended, or whose
  inputs the bridge refused, has no stop: it is kept under E only if the run kept it under T.
- **Kept under E.** Kept under T by the run, or holding an atom of U, W (less the ambient units), R or C that no seen
  state holds. The seen states are the archive entries committed before the job's own line of `log.jsonl` (the root
  included) and the trials kept under E before it in the step, in the run's order (offers, then delays, then the one
  more level). A kept trial's atoms are all held after it.
- **Each kind alone.** Its own sequential keeping over that kind only, T left out. Reported also: of the trials kept
  under E, how many had a novel atom of each kind.
- **Ambient units** are recomputed for every step with one rule: a no-input fork of the job's entry run 1,600 frames,
  every unit whose K_AT offset took 2 or more values (the entry's included). The job's recorded idle keys are listed
  for comparison, not used.
- **The sensitivity rule.** W is replaced by one atom per (room, unit, cell) for every unit with an offset, with no
  ambient rule. The cell is the room and the set of the job's merged T atoms that hold.
- **D1 and D2's T.** j00077's merged T (its reading's atoms and the archive's at commit 84) for every entry. A T atom
  is novel when it holds and no seen state holds it, or the room is new. The seen states are the entries with a lower
  id, the entry itself, and the search's kept stops.
- **D1's search.** Each kept node is expanded with afford's `try_all` (the offer, its inputs, the third click, E013's
  refused inputs, all as the run had them) on a fork at the node. The node is rebuilt as the root's fork with the
  node's clicks, run to its stop frame. The trial and second limits are checked between nodes, so the last node's step
  may pass them; that overshoot is reported.
- **D2's stops.** The route's clicks are scheduled at their recorded absolute frames. Input i's stop is the run to the
  frame after its click, then to the bridge's idle within W frames.

## Result (2026-11-04)

Written after the three gates and their independent checks, from the gates' records on `e015-item`, `e015-g1` and
`e015-g2` (merged into e015) and the checks' `check.md` files. This section stands in for the `report.md` the commit
rule names. Numbers only.

**Overall.** The item gate passes, G1 passes with no fix, and G2 fails as stated. Each check reproduced its gate from a
separate build and output folder, and each verdict holds. The stop rule did not bind: the plan was committed at 13:37,
the last gate result at 14:59 and the last check at 15:25, all inside 4 hours.

### The item gate: PASS

| criterion | result |
|---|---|
| 946 reached on a chain made of exit A, room 63's openings and room 69's inputs | yes: owner:946 = 1 = ego at frame 1548 |
| the chain verifies from the boot | 9 clicks, 0 of 3,850 facts differ, room 69 = 69, digest equal (0babbc2cb2c1a1de) |
| local 212's only start site is in 946's code | yes: 3 literal start sites of a script 212 (room 48 object 587, room 49 local 200, room 69 object 946); the first two start their own room's local 212 |
| 0 sweep trials from the room-42 entries start local 212 or clear class 12 on 491 | 0 of 30,000 (10 entries, depth 2): Started 0, live 0, cleared 0, ran anywhere 0 |

- **Steps 1-3.** Room 63 loads at frame 1392, first idle at 1405. Room 63 to 69: depth 1, 95 trials, 6 openings reach
  room 69, all clicks on object 854 (alone or after verb slot 1, 100, 101, 103 or 104). Room 69 to 946: depth 1,
  135 trials, 1 opening (verb 11, then 946).
- **Step 5's coverage.** Depth 1 is complete (285 trials per entry, 344 for entry 296). Within the 3,000-trial cap,
  depth 2 expanded 9 of 275 depth-1 stops (7 of 335 for entry 296), about 3%.
- **Step 6 (not gating).** No stop reached room 75, so the answer is "not within the sweep".
- **Cost.** Steps 1-4: 2.4 s wall. Steps 5-6: 136 s wall, 26 s CPU, 3.0-5.4 ms per trial. The code is 811 lines
  against the 200-300 planned.

### G1: PASS, no fix

| criterion | result |
|---|---|
| the input log shows sentence(10,596) | yes: `Sentence` touch (10,596,0) after 2 approach clicks and 1 candidate (a bare click) |
| room 63 loads after the dialog choice | yes, at frame 1289, 72 frames after the second dialog click |
| the chain replays from the boot | 5 inputs, 0 of 3,850 facts differ, room, digest, tick 5240 and rng equal; E014's probe (a separate build) also shows room 63 at 1289 |
| no verb id and no profile | the grep check finds none of `DOS_CD`, `selected_verb`, `wait_walk_to`, `DIALOG_FIRST/LAST` or the executor; the only numeric literals are the limits and the screen's half width |

- **Steps.** Dialog verb 120 at frame 993 (1 fork, 1.7 ms). Approach (5,125)@1043 and (21,131)@1069, xstart
  256 → 136 → 0, then the bare click (31,23)@1086 (3 forks, 10.3 ms). Dialog verb 120 at frame 1217 (1.7 ms).
- **(a)** From entry 184: 0 approach clicks, 1 candidate, 2.1 ms. **(b)** sentence(11,593) from entry 0: a miss,
  "approach", 8 forks, 29 ms. **(c)** 2-10 ms per sentence, 1-2 ms per fork.
- **Tests.** Every existing fmi-play and fmi-explore test count is unchanged with the switch off; the 9 new tests pass.

### G2: FAIL as stated

| criterion | result |
|---|---|
| t596 kept under E | **no** |
| kept rate over all re-scored trials in [0.5%, 20%] | yes: 97 of 16,026, 0.61% |

- **Why t596 is not kept.** Trial v1t596 (a verb slot then object 596, seq 15, frame 1258) reaches the same local-211
  dialog, waiting at offset 578, 152 trials earlier in afford's order. It is kept under E for `live:49:local:211` and
  `at:49:local:211:578`, so t596 (seq 167) holds nothing new. T kept nothing from the step. No archive entry holds
  local 211 live, so t596 would have been kept had it come first.
- **Calibration.** G2-0 met (264 trials, 0 kept under T, t596 at 1256 with local 211 live at 578) after two re-runner
  fixes, within the 45 minutes. 40 of 40 sampled steps reproduce their trial and T-kept counts. 628 of 628 seen-set
  boots equal their checkpoints.
- **By kind.** Kept: T 21 (0.13%), T∪U∪W 97, U alone 26, W alone 76, R alone 0, C alone 0. Room 49 keeps 37 of
  9,281 (0.40%), room 42 60 of 6,745 (0.89%). Per step: median 0.36%, maximum 4.8%. j00077 alone: 2 of 264.
  The sensitivity rule keeps 769 (4.80%), 7.9 times the primary rule.
- **D1 (not gating).** IW(1) under E reaches room 63 from 2 of 25 entries (184 at depth 2, 182 at depth 3). 21 run
  out of kept stops after 37-864 trials; 0 and 13 hit the trial limit at depth 3. A walk click's stop is new under no
  kind, because the archive already holds every camera bucket and every reachable target.
- **D2 (not gating).** Along route A, the longest unkept run before room 63 is E 0, T∪U∪W 0, T 2 (from entry 0), and
  E 0, T∪U∪W 0, T 1 (from entry 184).

### Predictions scored

**The item gate.** 6 of 7 hit, 1 missed; the room-75 question has its outcome.

| prediction | outcome |
|---|---|
| exit A reproduces, room 63 at 1392 (97%) | hit |
| room 69 within the limits (85%), depth 1-2 | hit, depth 1 |
| exactly 1 opening leads to room 69 from one state | **miss**: 6 of 95, all on object 854 |
| owner:946 = ego given room 69 (80%), depth 1-3 | hit, depth 1 |
| the chain verifies given reached (95%) | hit, 0 differences |
| local 212 has one start site in 946's code, 0 sweep starts (85%) | hit |
| the gate passes (65%) | hit |
| room 42 back to 49 via 75: yes 55%, no 20%, not within the sweep 25% | not within the sweep |
| cost: code about 40 minutes, 200-300 lines; run under 10 minutes CPU | lines **missed** (811); run hit (26 s CPU) |

**G1.** 7 of 9 hit, 2 missed.

| prediction | outcome |
|---|---|
| 2-3 approach clicks | hit, 2 |
| 1 candidate once 596 is on screen (75%) | hit, 1 |
| at most 6 candidates (90%) | hit, 1 |
| 0.1-3 s per sentence | **miss**: 0.002-0.010 s, 10-50 times faster |
| room 63 at frame 1,350-1,600 | **miss**: 1289, because each step clicks at the first idle frame |
| verifies from the boot (95%) | hit |
| (b) misses within 60 candidates and 10 s | hit: "approach" after 8 forks, 0.029 s |
| passes with no fix (65%) | hit |
| passes within two fixes (85%) | hit |

**G2.** 6 hit, 9 missed (one partly: entry 184's depth 2 was right), 1 not scorable.

| prediction | outcome |
|---|---|
| calibration at the first try (80%) | **miss**: two re-runner fixes |
| calibration met within 45 minutes (95%) | hit |
| at least 36 of 40 sampled steps reproduce (75%) | hit, 40 of 40 |
| t596 kept under E (85%) | **miss** |
| kept through U given kept (90%) | not scorable (v1t596 was kept through U and W) |
| kept rate 1-8%, median 3% | **miss**: 0.61% |
| kept rate in [0.5%, 20%] (75%) | hit |
| j00077 alone 2-15 of 264 | hit, 2 |
| U or W keep the most | hit (W 76, U 26) |
| G2 passes (65%) | **miss** |
| D1 width 1 in at least 80% of entries (60%) | **miss**: 2 of 25 |
| D1 depth 4-5 from xstart-256 entries; 2 from the left edge | **miss** for xstart 256 (never reached); 184 hit (2); 182 took 3 |
| D2: E 0 | hit |
| D2: T∪U∪W 1-2; T 3 | **miss**: 0; **miss**: 2 |
| sensitivity rule 1.5-3 times | **miss**: 7.9 times |

### The checks' issues and how each was settled

**The item gate** (check 883bba6, tool 90a4c72).

- Reproduced: every summary and every trial log identical apart from timings. Settled: the PASS holds.
- The positive half, newly shown in the engine: on play8's opening branch, 946 carried into room 42 (frame 2980)
  then v3t490, v8, v105t491 runs local 212 at frame 3044, clearing `class:491:12`, owner:946 = 0. The 35-click movie
  replays from the boot with 0 differences. The same sentence without 946 runs nothing at the 8 entries where the
  sweep tried it. Settled: C5 → C6 is shown in the engine, not only by the reading.
- Entries 324 and 341 are one state, so the sweep has 9 distinct states and 27,000 distinct trials. Settled: the
  count is corrected here; the verdict does not change.
- **C4 → C5 depends on the opening choice.** On ev5's branch (choice (4,184)), room 42 was not reached from room 49
  holding 946 (depth 2 with 5,000 trials, a verb-slot search with 5,148, exit A's dialog at depth 3 with 4,000).
  play8's room-42 entries made from room 49 descend from choice (4,154). Settled: recorded as a condition on the
  ladder (below); not resolved.
- The static half missed two sites: global script 1 gives 946 to room 69 when its argument is a boot start point,
  and room 49's local 213 loads room 69 when Bit[418] = 1, whose only literal write sets 0. Settled: neither is a
  player route as far as the scripts show; the criterion (start sites of local 212) is unaffected.
- The two "untraced" indirect starts are traced: Var[125] is only read and stays 0; script 72 has no start site.
  Settled: neither can start local 212.
- The `Started` touch cannot see scripts started by object code. Settled: the count rests on live, cleared and ran;
  cleared is robust (only local 212 clears class 12 on 491, and no script sets it).
- Commit order: amendment (1) at 13:42:39, the tool and amendment (2) at 13:46:42, the build at 13:46:30, the first
  output at 13:46:58. An earlier, overwritten run cannot be ruled out from the files. Settled: accepted as a limit of
  the record; reruns are identical.
- The verb-102 "error" runs (5, 8 and 297 trials) are not investigated. Settled: open; they do not hide the sentence
  that matters (v102t491 under the v8 node stops at an idle).

**G1** (check b4ead63).

- Reproduced: events byte-equal, records field-equal apart from wall time, verify and probe trace equal, test counts
  equal. Settled: the PASS holds.
- "No fix" cannot be shown from git (code and results within 5 minutes, after the run). Settled: consistent with no
  fix (realize.rs has one commit; reruns of the committed code give the committed result); accepted as a limit.
- Only candidate (i) ran on the demo; (ii), (iii) and the nearest-first order under a rejection ran only on the toy
  engine. Settled: open; the next use of the routine must meet them on the real engine.
- (b) does not test a hidden object (the opening dialog is up, so no floor click walks). Settled: open; a bounded miss
  from a free-walking state is untested.
- The approach also misses when no floor point is accepted for a click. Settled: a reading of the spec; no criterion
  changed.
- The dialog steps are given as verb ids (120), as the plan allows. Settled: the chain still depends on the planner
  naming that value.
- Wall 1.5-2 times higher under load. Settled: the speed prediction is still a miss; the score stays 7 of 9.
- Room 63 needs all three steps (d120 alone, d120 + s10:596, d120 + d120 all fail to reach it). Settled: supports the
  verdict.
- Cosmetic: the gate built in target-e015-g1, not target-e015. Settled: the merged e015 builds in target-e015
  (below).

**G2** (check d106489).

- Reproduced exactly: seen set, j00077, the 40 steps, score.json, steps.tsv, D1 and D2. An independent Python
  re-scorer gives the same counts. Settled: the FAIL holds.
- The plan was followed: amendment (G2-1) committed at 14:23-14:24, before the sample finished and before any score.
  Settled: no criterion changed after a result.
- t596's non-keep is not an artifact: it shares every U, R and C atom with v1t596, and every W offset except an
  ambient unit's. Settled: the FAIL is literal and correct.
- README error: 40 of 41 recorded initial waits match, not 41 (j00419: recorded 1 stop, the stand-in 0). Settled:
  corrected here; trial counts are unaffected because the offer points come from the recorded stops.
- README imprecision: from entry 184, D2's seen set also holds entries 0-183. Settled: corrected here.
- The criterion names a trial, not a state; E does keep local 211's dialog stop (via v1t596). Settled: the verdict
  stands as specified. The check names D1 (2 of 25) and the next item as the stronger evidence against §9.
- The 0.61% is mostly the same states found again in each step (14 room-49 keeps on `at:49:local:206:117`; the 60
  room-42 keeps on the same 11 atoms), because a stop kept only under E never enters the seen set. Carrying E-novel
  atoms across steps gives about 28 kept (0.17%), below the 0.5% bound. Settled: the plan fixes the seen set as the
  archive, so 0.61% stands; noted as a second way the rule would have failed under a cumulative seen set.
- D1 saves no event chains for its room-63 stops; entry 182's depth-3 chain was not checked separately. Settled:
  open, minor.
- Amendment (G2-1)'s approximations (stand-in wait domains; wait stops' U and W atoms not held) can only make E keep
  more. Settled: they do not change the result.

### The merge

e015-item, e015-g1 and e015-g2 are merged into e015 with no squash. The conflicts were in fmi-checks' `Cargo.toml`
(the bins `e015`, `e015chk`, `e015-g2`), in `src/bin/e015/main.rs` (now one binary with the subcommands `item` and
`realize`) and in this plan's amendments (item (1), (2), then (G2-1)). In `CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015`
the release bins build, and `cargo test` over the default members gives 241 passed, 0 failed, 9 ignored, with
`FMI_REALIZE` and `FMI_NOVEL_ENGINE` unset. fmi-explore's unit tests are 50 (G1's 47 plus enovel's 3); every other
count equals G1's `tests.md`.

### The decision rules, applied

1. **Rule 1 applies.** The item gate shows play8's room-42 branch off the path: room 42's local 212 needs 946, and
   946 is reached only through exit A, room 63 and room 69. So the ladder is C0-C9 as stated, and play8 stands at C0.
   The check adds a condition the ladder does not state: C4 → C5 was shown from the branch of opening choice (4,154)
   and not found from (4,184) within its searches. The ladder is not rewritten (the gate showed it); the condition is
   carried to the next plan.
2. **Rule 2 does not apply.** G1 passes but G2 does not, so fixes A-F, K1 and play9 are not triggered by these rules.
3. **Rule 3 applies.** G2 fails, so the engine-first inversion (report §9) is refuted as stated and is not built.
4. **Rule 4 does not apply.** G1 passed with no fix.
5. **Rule 5 does not apply.** No gate is "not run" or "not shown".

**What the rules leave open.** No rule names the next experiment when G1 passes and G2 fails. Rule 2 tied A-F to both
gates, and its B (live-unit novelty) is the U part of G2's E. What the gates do support: A (engine-confirmed
realization) passed on the demo's blocker, at milliseconds per sentence; the ladder's C0-C6 are shown in the engine
on one branch. What they do not settle: whether B is worth building after G2's failure (the dialog state was kept,
by the earlier trial; D1 reached room 63 from 2 of 25 entries), whether C-F go with A, and the opening-choice
condition on C4 → C5. The choice of the next experiment is the user's.
