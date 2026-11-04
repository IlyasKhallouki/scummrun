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

## Amendment (G2-1), 2026-11-04: G2's re-runner and readings, before G2's first score

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
