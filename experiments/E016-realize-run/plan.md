# E016: the engine realizes the plan's sentences on the run path, and one demo run scored by an outside ladder

Date planned: 2026-11-04
Research question it serves: RQ1 (completion from the game files alone), RQ4 (what carries over to another game)

Written and committed before any code. No cloud VM: every run here is on the laptop.

## Question

E015 settled three things (its plan's Result, `notes/log/2026-11-04-e015.md`):

- **Object 946 is on the path.** The demo's checkpoint ladder C0-C9 stands, and play8 is at C0.
- **The engine can turn a planned sentence into clicks without a profile** (G1, 2-10 ms per sentence). This removes
  the cause of play8's 22 dead attempts at exit A: the executor's profile uses Monkey Island's walk verb 11.
- **Novelty over engine state does not drive the search** (G2). The inversion is not built, and fix B is dropped.

This experiment asks whether the retrospective's minimal change, without B, moves the demo up the ladder. The change
is fix A on the run path, plus C, D, E and F (the retrospective's `proposal-minimal-change.md` §2). It also asks
whether Monkey Island's checks still pass with all of it switched on (rule (2)).

## Baseline

- **play8** (E014 amendment (8)): 502 tries, 628 entries, 1.85 h, ending at C0.
  - Stall shares: reading bound 65.0% and productive 1.6%; refused input was 34% of its last 20 tries.
  - Sentence(10,596), exit A's click, was planned 22 times, and each attempt died at its first input (row 27).
  - It ran with scope growth, and its {42,48,49,75} reading (518,131 records, 32 GB) bounded 115 of its 122 tries.
- **E015:**
  - G1 realized exit A from play8's entry 0 in 5 clicks.
  - The item check's chain shows C0-C6 in the engine: 35 clicks from boot, room 42 at frame 2980 holding 946, and
    local 212 run at frame 3044. This chain was found hop by hop by a checker, not by the system.
  - C4 → C5 was shown only from the opening choice (4,154). From (4,184) it was not found.
- **Monkey Island:** E012 reached the kitchen by search at try 53 (21.5 min). Since E013, rule (2)'s checks have been
  `experiments/E014-fate-demo/a4/mi-check.sh` b3, kitchen and smoke.

## Method

Every change sits behind its own switch, off by default. Monkey Island's goldens stay byte-identical with the switches
off. The code base is e015 (`experiments/E011-speed/rs`).

### A. Realization on the run path (`FMI_REALIZE=1`)

- **Where it hooks in.** `executor::play_input` hands the step to `realize::realize` (E015's G1 routine):
  - a "click" step, `Step::Sentence(v, a, b)`;
  - a "line" step, `Step::Dialog(v)`.

  It then plays the accepted events in place on the child's branch, so the window search, the frame observer and the
  strict matcher see exactly what they see today. The accepted fork is quit. A miss is the child's interface stall,
  with realize's reason ("approach", "no sentence", "no slot", "not taken").
- **Inventory objects** (the retrospective's A.3). When the first object has no hit point on the screen, each visible
  verb slot is tried as its click: verb, slot, then b. This is bounded by `max_slots`. G1 tested only room objects.
- **Every profile read on the run path is replaced by an engine fact while the switch is on.** That covers `DOS_CD`:
  the verb variable, the walk verb, the inventory layout and the verbless rooms in `window.rs`'s idle test. It also
  covers `DIALOG_FIRST/LAST`, `wait_walk_to` and `selected_verb`. Amendment (1) lists each read with its replacement
  before the first gate run. A read with no engine-fact replacement is reported there, not hidden.
- **Walk steps** (`walk_to` records). Amendment (1) states how patch 0007's log shows such a step's run, and the test
  realize accepts it by, before the first gate run.

### C. Failure memory at the exact entry (`FMI_NOGOOD=1`)

- **The refutation memory** is keyed by (entry id, the step prefix as realized inputs, target). It is sound because
  the engine is deterministic from an exact entry. The planner is asked again with the refuted prefix banned through
  taskcore's `--bans`, and the ban stays with that entry.
- **The no-plan memory** is keyed by (reading id, the start restricted to the reading's tested keys, target). It is
  dropped when that scope's reading is replaced.

### D. Free tries without a fresh reading (`FMI_FREE_T=archive`)

A free try takes T as the union of T over the readings already registered in the archive, and never starts a
reading. Model tries still read.

### E. The end must come from a verb handler (`FMI_GOAL_SITE=verb`)

A script's restart counts as the goal only when the restarting slot runs an object's verb handler. A global script
reached through an override does not count. That rules out global 57, the death screen, which room 82's air timer
starts.

### F. Configuration and selection

- **Scope growth off** (`FMI_SCOPE_GROW` unset): a room's reading is that room.
- **A frontier term** (`FMI_FRONTIER=1`): an entry in a room that no earlier entry held gets the next free worker
  ahead of the selection's order. In play8, entry 199 waited 110 commits.
- **The run's files** go where the disk has space: at least 15 GB free when the run starts, and the run relocates
  once at the disk floor.
- **The wall limit:** `FMI_STOP_WALL=10800`.

### The outside ladder

A scorer in `fmi-checks` (`ladder`) reads the archive's `entries.jsonl` and checkpoints only, and nothing of the
system's own judgement. It reports:
- the furthest checkpoint reached;
- the try that reached it, and whether a planned step or a model-free step produced it;
- the wall time at which each checkpoint was first reached.

The checkpoints are E015's C0-C9:
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

The ladder is the experiment's instrument and lives outside the system, so its game numbers do not breach D9. A
watcher runs it every 5 minutes during the run.

### Gates (offline, before any run)

- **G3 (A on an inventory object, the demo).** From the item check's state (its 35-click movie to room 42 holding
  946), realize s3:490, then s8:946:491. Pass: local 212 runs and class 12 on 491 is cleared within at most 15
  candidates, and the chain verifies from boot.
- **G4 (A on Monkey Island).** Along E012's kitchen goal chain, realize each sentence and dialog step the chain took
  from the entry it started at, with `FMI_REALIZE=1`. Pass: each step gives the chain's Sentence touch, or the dialog
  verb is taken, and the re-realized chain reaches owner:566 = 1, verified from boot.
- **G5 (E).** Unit tests: a forked death-screen restart is not a goal entry, and an object verb handler's restart is.
- **Tests:** every existing test passes with the switches off.

### Monkey Island with every switch on (rule (2))

- `mi-check.sh` b3, kitchen and smoke, with the E016 set added.
- **K1:** the kitchen by search under the E016 set, on the laptop, b = 4, a 60-minute wall.

### The run: play9

- On the demo from boot, seed 1, on the laptop with b = 4.
- Switches: play8's ALLON minus `FMI_SCOPE_GROW`, plus `FMI_REALIZE`, `FMI_NOGOOD`, `FMI_FREE_T=archive`,
  `FMI_GOAL_SITE=verb` and `FMI_FRONTIER`.
- Afterwards, every entry is verified from boot and the stall table is made, as E014's `vm/run.sh verify` and
  `stalls` do. The run report leads with the ladder.

## Predictions (before running)

Calibrated by the project's 2-10x optimism on game progress.

- **Gates:**
  - G3 passes: 70-85%, with 1-15 candidates.
  - G4, every kitchen step realized: 60-80%. The likeliest misses are Monkey Island's hover verb (var 394) and its
    verbless rooms.
  - G5: 95%.
- **K1** reaches the kitchen within 60 min: 45-65%.
- **play9, the furthest checkpoint:** median C3, 80% interval C1 to C7.
  - P(at least C1) 80-90%;
  - P(at least C3) 50-65%;
  - P(at least C5) 25-40%;
  - P(at least C7) 10-25%;
  - P(C9 verified from boot) 5-15%.
- **play9, the first C1:** within 2-20 min of wall.
- **play9's stall shares:**
  - reading bound 5-30% (play8: 65.0%);
  - refused input and effect mismatch together 0-10%;
  - productive 5-25% (play8: 1.6%).
- **Repeated (entry, prefix, target) attempts:** 0, by construction.
- **New assumption rows:** 2-6. The likeliest are:
  - the reading's scope for a carried object's code;
  - the dark room's visibility;
  - a slot that is ambiguous on a page turn;
  - the air timer against the wait's horizon.

## Success threshold

- **Yes:** play9 reaches C9, verified from boot with the system's own inputs.
- **Partly:** play9 reaches C5 or higher.
- **No:** play9 stops below C5.

## Stop rule

- **At the gates:** if G3 fails, fix A at most twice, then stop and write up. If G4, K1 or a `mi-check.sh` check
  fails, the result is reported as a rule (2) regression. The run still goes, and nothing merges to main until it is
  fixed.
- **During the run:** it stops at the first of:
  - the wall limit;
  - 40 minutes of wall without a new ladder checkpoint;
  - the goal (FMI_GOAL_FACT);
  - the disk floor after one relocation.
- **On the line of work:**
  - If play9 stops below C5, one amended rerun is allowed, with the amendment committed before it.
  - If that rerun also stops below C5, this architecture is not patched again. The experiment is written up, and the
    next direction is brought to Ilyas, not chosen by this plan. E015 refuted the inversion, so the remaining
    candidates are the retrospective's other proposals.

## Cost

- **Engineering:**
  - A's hook-in and inventory extension, about half a day;
  - C, D, E, F and the ladder, about half a day.
- **Gates and Monkey Island's checks:** about 1.5 h of wall.
- **play9:** at most 3 h of laptop wall.
- **The VM:** nothing, so the ledger's $9.01 stays untouched.

## Amendments
