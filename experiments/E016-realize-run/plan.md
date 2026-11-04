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

### (1) 2026-11-04, A: the profile reads on the run path, their engine facts, and walk steps in the input log

Written before any code of A, at 4c40b3c. The run path is a window child's input (`window.rs` `child_strict_` →
`executor::play_input`) and the window's gate (`window.rs` `Search::gate`). The reads were found with
`grep -n 'DOS_CD\|self\.p\.\|DIALOG_\|MAX_ACTOR\|MAIN_H\|EXECUTOR_EGO\|VAR_EGO'` over `fmi-play/src`,
`fmi-explore/src` and `fmi/src`. Only `executor.rs` and `window.rs:3251` read them.

**What the switch changes.** With `FMI_REALIZE=1`, `play_input` gives a "click" step (a sentence or walk_to record)
and a "line" step (a dialog_line record) to realize, and none of `wait_camera`, `check_ego`, `Executor::play` or
`click_verb` runs for it. A "key" step and a "wait" step keep the executor, which reads no profile for them (a key is
scheduled at the next frame).

**The reads and their replacements while `FMI_REALIZE=1`:**

| # | read | where | replacement |
|---|---|---|---|
| 1 | `DOS_CD.verbless_rooms`, with up, cs, room and "a verb is listed" | the gate of a click step (`window.rs:3251`); `wait_gate` (`executor.rs:610`) | The bridge's idle rule, `Status::idle` set. With `SPEEDRUN_IDLE=handler` (patch 0006-e014) it is the click gate open, VAR_VERB_SCRIPT set and idle, and nothing the last input started still running. A child whose fork frame is not idle is run to the next idle stop first, at most afford's W = 1,600 frames. |
| 2 | `DOS_CD.verbless_rooms` and `MAIN_H` (144) | `main_h` (`executor.rs:475`), used by `actor_points`, `object_cands`, `floor_point` and `click`'s point check | The main screen's topline and height from `Status::screen` (patch 0004), as afford's `object_grid` uses them. A point counts only where the engine's hit test answers the object. |
| 3 | the camera formula for xstart (`cam`, the room width) | `xstart` (`executor.rs:464`) | `Status::xstart`, the engine's own. |
| 4 | `verb_var` (107), `hover_saved_verb_var` (394) | `selected_verb` (`:679`), the verb key's check (`:782`, `check_var`), `wait_walk_to` (`:812`), the approach (`:921`) | No variable is read. A candidate is accepted by patch 0007's input log: the handler's Sentence touch (v, a, b). The bare click (whatever verb the engine has selected) is tried first, then the verb slots. `check_var` stays None, so E013's refuse gets no checked variable from a realized step. |
| 5 | `walk_to` (11) | `click`'s default verb (`:712`), `wait_walk_to`, the approach's floor click (`:921`), the stepping stones (`:993`, `:1066`) | The approach clicks a walk box's floor point (FMI_BOXES' Boxes command, afford's offer) with no verb click. It is accepted when the input log shows a Walk touch and `Status::xstart` moved toward the target, or the target gained a hit point (E015 G1). A walk_to step's verb is the record's own (the reader's walk verb, read from the scripts), judged as below. |
| 6 | `verb_key` (the key of verb id v in the dump) | `click` (`:445`, `:739`) | A visible verb slot from the dump is clicked, at a point where the hit test answers that slot. No key. |
| 7 | `inv_first_var` (133), `inv_first_verb` (200), `inv_slots` (8) | `inventory_point` (`:552`) | None of them. The inventory extension (Method A, "Inventory objects") tries each visible verb slot as a's click. The input log decides which slot is a's; no slot is read as a's. |
| 8 | `inv_scroll_var` (118), `inv_row` (4), `inv_up` / `inv_down` (208 / 209) | `scroll_to` (`:649`), the arrow move (`:694`) | **No engine-fact replacement.** Realize does not scroll the inventory, so an object whose slot is scrolled out of view misses with "no sentence". (The arrow move is never formed from a record: no record has an arrow verb and no object.) |
| 9 | `EXECUTOR_EGO` (actor 1) against `VAR_EGO` | `check_ego` (`:1150`), `ego_of` (`:878`), `stepping_stone` (`:946`) | Nothing needed. Realize's approach reads no actor: it judges a click by the input log's Walk touch and the camera. |
| 10 | `MAX_ACTOR` (12): an id of at most 12 not among the room's objects is an actor | `object_points` (`:532`), `click` (`:757`), `check_ego` (`:1157`) | The hit test's own answer: a target is Object(a) where the hit test answers object a, Actor(a) where it answers actor a. |
| 11 | `wait_camera`'s rule: run on while the camera moves, until both objects have a point | `executor.rs:1125` | The run to the idle stop (row 1), then realize's approach. |
| 12 | `DIALOG_FIRST` / `DIALOG_LAST` (120 / 128) | defined at `executor.rs:15-16`; read nowhere | A "line" step already uses `verb_point`: the dump's verb with id v, listed once, at a point whose hit test answers its slot. With the switch on it is realize's dialog step: the offered slot whose dump id is v, accepted when patch 0007's verdict is "taken". |
| 13 | limits, not game facts: `INPUT_LIMIT` (600), `wait_walk_to`'s 120, `STONE_FRAMES` (300) | | Realize's `Cfg`: max_approach 6, max_candidates 60, max_points 3, max_slots 15, horizon W = 1,600, log_frames 8. |

**What the switch needs.** `SPEEDRUN_INPUT=1` (with the input log off, every candidate misses) and `FMI_BOXES=1`
(the approach's floor points). Also `SPEEDRUN_IDLE=handler`: with it unset, the bridge's idle rule reads Monkey
Island's verb ids (assumptions row 4). play9's switches and Monkey Island's `E14M` have all three.

**The inventory extension's order.** When a has no hit point on the screen and no place in the room (an inventory
object), the candidates are: a verb choice, then a slot as a's click, then b. b's click is its hit point, or each
visible slot when b has none.
- The verb choices, in order: the offered slot whose dump id is v (in SCUMM v5 a slot's id is its verb number, as
  the dialog step uses); then no verb click; then the other offered slots in the dump's order.
- For each verb choice, a's slot runs over the visible slots in the dump's order, leaving out the verb choice's own
  slot, at most max_slots = 15 of them.
- A candidate is accepted on the Sentence touch (v, a, b), as before; the order only decides what is tried first.
- The approach's target is a when a has a place in the room (a room object's walk point or an actor's feet), else b
  when b has one.

A room object's candidates keep G1's order.

**The hook.**
- `play_input` hands the step to `realize::realize` on the child's engine.
- It plays the accepted events in place on the child's branch (`Br`): scheduled with `Br::inputs`, then run to the
  last event's frame through `Br::run`, so the frame observer and the strict matcher see every frame as today.
- It quits the accepted fork.
- A miss is `PlayErr::Route("<name>: realize: <reason> ...")` with realize's reason ("approach", "no sentence",
  "no slot", "not taken"). The window records it as the child's interface outcome.
- The switch is read by `realize::on()` in `play_input` and in `Search::gate`. With it unset, neither changes
  anything.

**Walk steps in patch 0007's log.** The probe is `e016 inlog` (fmi-checks; committed after this amendment, before
any gate run). It ran on Monkey Island's E012 kitchen chain: entries 0, 44, 55, 85, 123, 135, 142, 146 and 150, their
events from boot. It used E013's engine, `SPEEDRUN_IDLE=handler`, `SPEEDRUN_INPUT=1` and `FMI_BOXES=1`.
- **walk_to(444)**, one click on the object at frame 10161 with no verb chosen: one record, verdict taken, handler 4.
  Its writes: Sentence (11, 444, 0), var 107 11 → 11, var 108 → 444.
- **walk_to(367)** at frame 10163: Sentence (11, 367, 0).
- **walk_to(316)'s window** (job j00052):
  - the four floor clicks at frames 18318-18324 each give a Walk touch (actor 1 to the click) and no Sentence;
  - the click at 18325, on another object, gives Sentence (11, 323, 0);
  - the click on 316 at 18365 gives a Walk and Sentence (11, 316, 0).
- **For comparison, sentence(9,566):** the key at frame 18405 gives var 107 = 9 and no Sentence; the click at 18406
  gives Sentence (9, 566, 0).

So a walk_to step's run shows as the handler's Sentence touch (w, a, 0), with w the walk verb, in the frame of the
click on a. A floor click shows only a Walk touch.

**The test realize accepts a walk step by.** Realize takes a walk_to step as `Step::Sentence { v: w, a, b: None }`,
with w the record's verb (`step_input`'s src[1]). It accepts a candidate when the input log holds the Sentence touch
(w, a, 0): the same test as for any sentence. The first candidate, the bare click on a, gives that touch whenever the
interface's selected verb is the walk verb. In the demo, none of play8's 502 job results names a walk_to record, so
this test is exercised on Monkey Island (G4).
