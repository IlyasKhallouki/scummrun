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
- **The VM:** nothing, so the ledger's $9.01 stays untouched. Amendment (7) moved play9 to the VM, at about $5.40
  for the 3.5 h booked.

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
| 6 | `verb_key` (the key of verb id v in the dump) | `click` (`:445`, `:734`) | A visible verb slot from the dump is clicked, at a point where the hit test answers that slot. No key. |
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

### (2) 2026-11-04, A: G3's first run fails on the candidate count; fix 1 (image slots first)

**The first run** (`results/g3/run1/`, tool at a97ef1c, realize at 8bcf853). It started from the item check's 30 clicks,
idle at frame 2980 in room 42 with owner:946 = 1.
- **s3:490: 4 candidates, 10 forks, 16 ms.** The bare click (3 hit points) gives (10, 490, 0); verb slot 1 then 490
  gives (10, 490, 0); slot 5 then 490 gives no sentence; slot 3 then 490 gives (3, 490, 0), accepted. Its clicks are
  at 2981 and 2983, as in the item check. Next idle: 2998.
- **s8:946:491: 1 approach click, then 14 candidates, 15 forks, 26 ms.**
  - At 2998, 491 had no hit point. One floor click at 2999 gave it one.
  - Then verb 8's slot, a's slot in the dump's order, then 491. The verb slots 1, 5, 3, 4, 11, 12, 9, 6 and 7 each
    gave (that slot's id, 491, 0): the slot replaced the verb. The image slots 101-104 gave (8, o, 491) with the object
    each shows. Slot 105 gave (8, 946, 491), accepted, with clicks at 3004, 3006 and 3008.
- **After the steps:** local 212 ran at frame 3011, and class 12 on 491 was cleared at 3011.
- **Checks:** the chain from boot (30 + 6 clicks) to frame 3011 gives 0 differences. The second pass, through
  `executor::play_input`, gives the same 6 events.
  - That pass's fork and a control fork given the same events at once agree (digest equal, 0 facts differ).
  - Both differ from the first pass's root engine only in var 56: 0 on the root, -1 in a fork, at 3011. This is a
    fork-against-root difference that A does not cause (`run1/g3-control.json`). It is reported as an open issue.
- **Verdict as stated: FAIL.** The two steps took 4 + 14 = 18 candidates, above 15. Every other condition holds.
  (Per step, both counts are within 15. The plan's text reads as the total, so the total is what is scored.)

**Fix 1.** In the inventory extension, a's slot (and b's, when b has no hit point) runs over the visible slots of
image type first, then the others, each group in the dump's order.
- The engine fact behind it: the dump's verb `type` is the slot's `VerbSlot::type`. In SCUMM v5 an image slot is
  drawn from an object's image (o5_verbOps' image subop calls setVerbObject with an object number), so a slot that
  shows an object is an image slot whenever the game draws its inventory as images.
- A game that writes its inventory as text has the same order as before.
- Acceptance is unchanged: the Sentence touch (v, a, b).
- Room objects' candidates are unchanged.
- The rerun is G3 as stated, with the same tool (af8bcf6 adds only the control fork). Fixes left after this one: 1.

### (3) 2026-11-04, A: the approach fixed before play9 (G4's two misses)

G4 failed on the approach in wide rooms (`results/g4/g4.md`): walk_to(427) in room 33 and walk_to(316) in room 28.
In both, afford's single floor point per walk box lay behind the actor. Per the retrospective's rule to fix known
gaps before a run (play8's row 27 was known and left in), the approach is fixed before play9, under the same switch
(`FMI_REALIZE=1`). This is G4's fix, decided by the coordinator. G3's fix budget (one left) is not used for it.

**Fix 2, in realize's approach, from engine facts only:**
- **The nearest point of each box.** For each walkable box (afford's rule: index from 1, not locked, not invisible),
  the approach also offers the box's point nearest the target's place. It is built as follows:
  - its y is the target's, clamped to the box's rows and to the main screen's (`Status::screen`);
  - its x is the target's, clamped to the box's columns at that row (between its left edge ul-ll and its right edge
    ur-lr) and to the screen's columns (`Status::xstart` to xstart + 319);
  - it is kept only where the engine's hit test answers no object and no actor (afford's floor test), all points in
    one AtMany.
  
  It is offered alongside the box's usual point. The usual points come first, ordered by distance to the target's
  place as before, so E015 G1's approach is unchanged where it already worked. Then come the nearest points, ordered
  the same way: at most 2 points per box in all. A point at the screen's edge on the target's side can now be a
  candidate.
- **A stepping stone.** When no floor point is accepted, the approach tries a bare click on each room object that has
  a hit point and whose walk point is nearer the target's place than the view's middle column (xstart + 160), the
  nearest to the target first.
  - A stone is accepted by the same test as a floor click: a walk and the camera moving toward the target, or the
    target gaining a hit point.
  - For a stone, the walk is a Walk touch or the click's Sentence touch on the stone. Patch 0007 logs the handler's
    writes, and a walk that the sentence script starts is not among them: the probe's click on 323 gave only
    (11, 323, 0).
- **Bounds:** `max_approach` (6) accepted clicks; per approach click, at most 2 floor points per walkable box and one
  stone per object with a hit point.
- **Toy tests:**
  - a target that only a box's nearest point brings on screen;
  - a target that only a stone brings on screen;
  - the bounds.

**The reruns, with the fix:**
- G4 as stated, both modes (each step from its job's entry, and the whole chain from boot);
- G3 as stated;
- G1's gate (E015 `results/g1/run.sh gate`: play8 entry 0, d120, s10:596, d120, to room 63, verified from boot).

**Predictions:**
- G4 realizes all 14 steps: 65%;
- walk_to(427) is realized by a nearest point: 80%;
- walk_to(316) is realized, by a nearest point or a stone: 60%;
- G3 and G1 give the same inputs as before: 85%.

### (4) 2026-11-04, A: G4's rerun with fix 2; the approach's candidates in one order

**G4 rerun 1** (`results/g4/run2/`, realize at 0c8f7ca): 13 of 14 steps per job, 12 of 14 along the chain.
- **walk_to(316):**
  - **Per job, realized** with 4 approach clicks: the stone 323 twice, then the nearest point of box 8 at the
    screen's right edge, then a usual point. Then 1 candidate gives (11, 316, 0).
  - **Along the chain, a miss.** There the state at 18316 differs a little: the earlier realized steps click a frame
    or two later than the chain did. The right-edge points were not floor, and every stone moved the camera away.
- **walk_to(427), both modes, a miss on the bound.**
  - The nearest point at the right edge moved the camera from 296 to 392.
  - Then the usual points (tried first) moved it about 80 pixels per click: 472, 552, 632.
  - `max_approach` (6) ran out before 427, at room x 984, came on the screen. The chain's executor needed xstart ≥ 665.
- **The chain reaches owner:566 = 1 at 18417 and verifies from boot (0 differences),** with the chain's own clicks for
  the misses.

**Fix 2b.** The usual points and the nearest points form one list, ordered by distance to the target's place. That
is amendment (3)'s first text and G1's own rule for floor points, applied to every point.
- A box's nearest point is never farther from the target than its usual point, so it comes first.
- A walk to the screen's edge on the target's side moves the camera by up to half a screen (SCUMM v5's camera
  follows the actor past its trigger strips), not by about 80 pixels.
- The stones still come only after every floor point fails. The bounds are unchanged.
- This drops amendment (3)'s "usual points first", so G1's and G3's approach clicks may change.

**Reruns:** G4, G3 and G1's gate, as in amendment (3). This is the last change to the approach before the report: if
G4 still misses, the misses are reported, not fixed again.

**Predictions:**
- G4 realizes all 14 steps in both modes: 50%;
- walk_to(427) is realized within 6 approach clicks: 70%;
- G3 and G1 pass with changed or unchanged inputs: 90%.

### (5) 2026-11-04, A: realize accepts what the game did, not only what the handler logged

G4's last miss (walk_to(316) along the chain) is a general mechanic: a click starts a walk, and the sentence, or the
object's handler, runs later from another script. Patch 0007 logs only the input handler's own run (`onSentence`
records a touch only while that run is logged), so realize's test never sees it. This is the last change to realize
in E016. A miss after it is reported, not fixed. No engine patch is changed.

**The probe** (`e016 inlog --runs`, `results/g4/probe-316/`). It took the chain-mode state at 18371 and the chain's own
state at 18364, and read the segment's slot runs of object scripts (where inventory, room or floating object):
- **Chain mode:** the bare click on 316 at 18372 logs no Sentence touch. In the same frame, slot 7 starts 316's script
  at code offset 79, and runs it again at 18390.
- **The chain's own state:** the click at 18365 logs (11, 316, 0). In the same frame, slot 5 starts 316's script at
  code offset 79.

So the engine reports that object a's script started and where in a's code it started (`Run::code0`, the offset from
the script's start). **It does not report the verb.** The verb-to-offset table is the object's verb block in the game's
scripts, and neither the dump nor the runs reply carries it. So no engine signal identifies the verb.

**The new acceptance (fix 3), the next best signal.** A sentence candidate is accepted when either:
- (as now) its input log shows the Sentence touch (v, a, b); or
- its input log shows no Sentence touch at all, and object a's script started after its first click. That is a run in
  the segment's runs reply with where inventory, room or floating object, number a, a slot that held no script of a
  at the step's start (the dump's slots), within the run to the bridge's next idle stop (at most the horizon, W =
  1,600 frames).

Each candidate's fork arms the segment (`Engine::segment`) before its clicks. Segment mode reads the runs and changes
no state (G3's verification ran with it on). Dialog steps are unchanged.

**Its weakness.** The verb is not checked:
- any of a's handlers counts, such as another verb's, or a's script started by a room script or a timer within that
  run;
- a candidate whose own handler queues no sentence, followed by any start of a's script, is accepted as (v, a, b).

The limits: the run counts only when the click logged no sentence (a logged sentence of another verb still rejects the
candidate), only for a slot of a's not live at the start, and only until the next idle stop. The candidates' order
(the bare click, then the verb slots) also bounds it. Mapping `code0` to the verb through the reader's object
entries would remove the weakness. It is left for a later experiment.

**Toy tests:**
- a deferred sentence accepted by the object's run;
- a logged sentence of another verb not overruled by a run;
- no run, a miss.

**Reruns:** G4 (per job, and the chain from boot), G3, G1's gate.

**Predictions:**
- G4 per job stays at 14 of 14: 85%;
- the chain realizes walk_to(316): 75%;
- the chain reaches owner:566 = 1: 35% (the chain-mode probe stays in room 28 for 190 frames after the click);
- G3 and G1 pass with the same inputs: 80%.

### (6) 2026-11-04, A: fix 3 reverted; what goes into Monkey Island's checks and play9

Fix 3 (268a9aa) made realize take verb slot 4 for steps that need verbs 8, 2, 10 and 9. G3 failed, and G4 along the
chain fell to 4 of 14 (`results/g4/g4.md`, run 4). Amendment (5) makes fix 3 the last change to realize in E016, so
it is reverted (2fda77b). Realize is again the code G3, G4 run 3 and G1's rerun ran: fix 2b, 7daacc6.

- **G3: pass** (run 3).
- **G4: fail as stated.**
  - Each step from its job's entry: 14 of 14 realized.
  - Along the chain from boot: 12 of 14, and the goal is not reached.
  - The miss is a deferred sentence: the click starts a walk, and another script queues the sentence, which patch
    0007's log does not show.
  - By the stop rule this is a rule (2) regression. Monkey Island's checks and K1 run anyway, and nothing merges to
    main until it is fixed.
- **Not done in E016:** the fix amendment (5) names. An object's code offset in the runs reply would be mapped to its
  verb through the object's verb table, read from the game's files by the reader. That would let realize accept a
  deferred sentence by its verb.

Monkey Island's checks (`run/mi-check.sh`, `run/k1.sh`) and play9 (`run/laptop.sh`) run on target-e016, built from
this commit's tree.

### (7) 2026-11-04: play9 on the cloud VM; Monkey Island's checks rerun after a missing link

- **Ilyas moved play9 to the VM:** "we do play 9 on the VM."
  - A raise of the project's all-regions CPU quota from 32 to 64 was refused, so the VM is play8's size:
    n2-custom-30-122880, standard, $1.535/h, booked for 3.5 h.
  - The run is `run/laptop.sh`'s (init, run with the ladder's watcher, stalls, ladder) under `run/vm.sh`. It uses
    play8's b = 10 with 2 readings in the background, no heavy lock, and a 2 GB disk floor as play8 had.
  - The wall limit (10,800 s), the 40-minute rule and the predictions are unchanged. b = 10 is play8's setting, not
    the plan's b = 4.
  - Every entry is then verified from boot on the VM, 24 at a time.
- **The first `mi-check.sh` was void.** The worktree had no link to Monkey Island's files (`game/`, never
  committed), so every b3 job waited out the 600 s connect timeout ("engine 0 did not connect") for 1 h 40 min.
  - The links that wt-e014 has (game, data, build, out, .venv, E001's key) are now in wt-e016.
  - `run/env.sh` stops at once on missing demo files.
  - `mi-check.sh` and `k1.sh` check the Monkey Island link before they start.
  - The checks and K1 run again on the laptop, beside play9 on the VM.
- **The VM's copy of SDL3:** ScummVM's sdl2-compat loads SDL3 at run time, which ldd does not list, so `vm.sh push`
  adds it.
- **On the VM's disk:** E014's archives (copies of the laptop's play7 and play8) and the E014 builds were removed,
  which left 16 GB free.

### (8) 2026-11-04: plan F's room scopes were not what the switches gave; two void starts of play9

- **First VM start:** play9's first try panicked in the reader: "a reason the machine does not know: widened loop".
  - The reader's list of reasons that the machine's unknowns can carry back lacked E014's widened loop.
    `FMI_READ_WIDEN=1` makes those unknowns.
  - Fixed in 40a4bef, with a test that every machine reason reads back.
- **Second VM start:** the first try's reading of [24, 42, 48, 49, 63, 68, 69, 75] hit the 300 s bound with nothing
  read, and the driver stopped on stagnation after that one try.
  - Plan F says "Scope growth off: a room's reading is that room." But unsetting `FMI_SCOPE_GROW` gives E009's
    rule, the guide's whole ring, not the room.
  - The same ring readings had the laptop's memory guard kill Monkey Island's smoke run (12 min, no entry) and K1
    (14 min).
  - Fix: `FMI_SCOPE_GROW=room` reads each room alone and never grows it (`grow::room_only`, `Archive::scope_of_room`,
    with the test `room_scopes_are_the_room_and_do_not_grow`). `run/env.sh` sets it in ALLON and A4. This is plan
    F's configuration as written, not a new mechanism.
- Both starts stopped at C0 on defects found before any search ran, so they do not count as play9 or as its rerun
  under the stop rule.
  - Their files are kept on the VM and the laptop under `/var/tmp/fast-mi/e016-failed/`.
  - play9 restarted at 19:50 UTC with room scopes.
  - Monkey Island's checks and K1 run again with room scopes. The first runs had ring scopes, and their files are
    kept as `mi-check-ring`, `k1-ring`. Results of those first runs:
    - b3: 129 new entries, 0 differences;
    - kitchen: pass;
    - smoke: killed with no entry;
    - K1: killed with no entry.

### (9) 2026-11-04: one long plan search stopped every worker; play9 resumed from its archive

- **The finding:** at 20:02 UTC play9 had reached C3 at 5.4 min, then committed no try for 10 minutes.
  - A stack of the busy thread showed `taskcore::lama::search_with` under `WarmPlanner::search`.
  - `Harness` held the one lock over every reading's planner (`PlannerCache`) for the whole search, so the other 9
    workers waited on it.
  - The limits were play8's: `FMI_PLAN_DEEPEN=120` (first plans), 600 s after that.
  - Of the run's tries so far, successful plans took a median of 0.1 s of planning and a p95 of 3.5 s. The tries of
    80-208 s ended mostly with "no plan" or a repeated empty window on a walk sentence.
- **Fixes:**
  - Each reading's planner has its own lock (`SharedPlanner`). The cache's lock is held only to find the planner.
  - `FMI_PLAN_DEEPEN=10` is added to E16: the first limit is 10 s, doubled per deferral as before.
  - fmi-explore's nogood and free_explore tests pass (19).
- **The resume:** Ilyas: "yes do it", "from the last checkpoint ofc". So play9 was not restarted.
  - The driver was killed at 20:28 UTC. The archive (331 entries, C3) was run on from its files with the new binary
    and switches (`vm.sh resume`). The tries cut off by the kill are restarted by the driver.
  - The ladder's 40-minute rule counts from the resume's watcher.
  - play9's report gives the two segments: 0-37 min with play8's plan limits, then the resume.
- **Monkey Island's checks** with room scopes, before this amendment's changes: kitchen pass; smoke pass (23 entries,
  0 differences). K1 was killed by the laptop's memory guard while a build ran beside it. The checks and K1 run again
  with this amendment's set.

### (10) 2026-11-04: what a try keeps; a held object's place; play9 resumed again

- **The finding:** play9 stayed at C3 from 5.4 min on.
  - All 25 entries that hold 946 are in room 69. The selection gave room 69 its first tries by 19:58 and none after,
    because room 49 kept making untried cells.
  - That is not the block. Free tries from 946-holding entries (j00083, j00085, j00087, j00095) walked ego into room
    63 at 38 points and kept nothing.
  - A try keeps a state only for single-atom novelty: an atom of T or a room that no live entry holds
    (`Novel::novel`). "Room 63 holding 946" has neither, since room 63 and owner:946=1 are each already held.
  - So C3 to C4 and C5, a known room entered with a new owned object, cannot be kept, whatever the selection does.
    Diagnosed by two investigators, each checked by a second agent; the check found this cause.
- **Fix 1, `FMI_NOVEL_PLACE=1`:** a state is also novel when its place is one no live entry holds. The place is the
  room with the objects ego owns, the cell key's first two fields. Both are engine facts, and no room, object or
  verb is named.
  - `progress::place_text`, `Progress.places`, `novelty_json`'s "places", `Seen.places`, `Novel::novel_place`.
  - Test: `a_seen_room_with_a_new_owned_set_is_a_novel_place`.
- **Fix 2, realize's `place_of`:** ScummVM's pickup (o5_pickupObject) leaves the object in the room's object table.
  Realize therefore placed held 946 at its old walk point and tried to approach it: "approach (0 approach clicks, 0
  candidates, 17 forks)", the empty window of sentence(8,946,940).
  - An object whose owner is not the room now has no place in the room, as whereIsObject has it. The inventory
    search then forms (8,946,940); offline, the click order v8, v105, t940 shows the touch.
  - Test: `a_held_object_has_no_place_in_the_room`.
- **Not fixed, noted:** the room-69 targets var:106=940 and var:104=3 are the input handler's own sentence
  variables. The sentence sets var 106 only to 0, so these targets are not game progress.
- **Build:** the laptop ran out of memory (the session's own build was stopped by the host), so on Ilyas's word ("do
  the fixes on the vm") the fmi binary was built on the VM from this tree, with cargo 1.99.1 and --locked.
  - fmi-explore's and fmi-play's unit tests pass there: 51 and 23, the two new tests among them.
  - The binary that ran 20:29-21:08 is kept as `target-e016/release/fmi.pre-a10` on the VM.
- **play9 resumed** from its archive at 21:08 UTC with `FMI_NOVEL_PLACE=1` in E16: 652 tries, 1,310 entries, still
  C3. The 40-minute rule had not fired.
- Monkey Island's checks are owed with this set; the last laptop attempt broke under the laptop's load (b3's warm
  process died at load 22, and its watcher held the heavy lock).

### (11) 2026-11-04: places before cells in the selection; play9's amended rerun

- **After (10):** from 21:08 to 21:45 UTC the run made 334+ tries, all from rooms 49 and 63, none from room 69. No
  entry kept a place (`place=` never appears in the log), and the ladder stayed at C3.
  - The place novelty acts only in tries that start where 946 is held, and the selection never went back to room 69:
    room 49 kept making untried cells (3,382 entries at 21:39).
  - This is the first investigator's selection finding of (10). The check called it optional; this run shows it is
    needed.
- **Fix, `FMI_SELECT_PLACE=1`:** the model's and the free selection order places before cells.
  - Per place, the place being the cell key's room and owned objects, n counts the tries since one gave a new atom of
    T or a room, and chosen counts every try, as a cell's counts do (`Archive::place_key`, from the log, cached per
    log length).
  - A place whose tries make new cells but nothing new in T can no longer hold every worker.
  - select.json gives "place" and "place_key".
  - Test: `places_tried_without_yield_come_after_untried_places` (tests/free.rs). fmi-explore's lib (51), free (8),
    grow (10 with the earlier tests), nogood and free_explore tests pass on the VM build.
- **The stop rule:** play9 stands at C3. The operator stopped it at 21:45, 4 minutes before its 40-minute rule.
  - From the archive (about 1,000 tries, 3,382 entries), the run goes on with (9)-(11) as play9's one amended rerun.
  - If it also stops below C5, patching stops and E016 is written up.
- Also seen: at 21:41-21:45 no try committed for 4 minutes before the stop. With every try in one room, the tries
  share one reading and so one planner, whose lock (9) still serializes them; plans after the first keep the 600 s
  limit.

### (12) 2026-11-04: every plan at most 30 s

At 21:46-21:49 the rerun's 10 restarted tries committed nothing for 3 minutes. A stack showed two job threads in
`WarmPlanner::search` (`taskcore::search::Ff::eval`) and the rest waiting on their readings' planners.
`FMI_PLAN_DEEPEN=10` bounds only a try's first plans; later plans had `PLAN_TIME` (600 s). In play9, successful plans
took 0.1 s of planning at the median and 3.5 s at the p95, so `FMI_PLAN_TIME=30` is added to E16. This changes a
setting only, with no new code. The rerun resumed from its archive at 21:49:42 UTC.

### (13) 2026-11-04/05: reach starts without the cleared keys; the C4 resumes

- **C4 at 121.4 min** (e3412, j01004, model-free), after (11) and (12): 946 is held in rooms 49 and 63.
  - Google stopped the VM at its booked time, 22:31 UTC. It was restarted for 2 h (Ilyas: "Just continue") and play9
    resumed at 22:39 UTC.
  - From then to 23:09 it made 820 tries and no new entry. The archive has entries in rooms 49, 63 and 69 only; no
    try has visited room 42.
- **The cause** (two investigators, each checked by a second agent):
  - play8 reached room 42 from a room-49 conversation state, entry 198. A model try planned bit:403=1 there, and a
    free try from that plan's checkpoint left for room 42.
  - play9 has the same state (entries 240 and 549). Every model try from it stopped with "reach extension: ... did
    not reach either: ['bit:404=0']".
  - Engine facts leave out a bit or local that is 0. The planner's start reads the missing key as 0
    (`start_text_in`). `reach_start_text` writes only the keys present, and the reader seeds reached values only from
    keys a start names.
  - So once a reused reading's earlier starts held bit 404 at 1, no extension can make 0 reached. The defect then
    spread through the extended readings that were registered and reused.
  - It cost 132 of play9's 137 "did not reach either" stops, including every model try from the dialog states.
  - A planner-only check on a copy of j00130's reach-03 reading, with `bit:404\t0 1` added to reading.reached, planned
    bit:403=1 in 5 steps. Unchanged, all 8 targets were refused.
- **Fix, `FMI_REACH_ZEROS=1`:** reach and refine starts (`start::reach_start_text_full`, called from
  `Harness::extend` and the refinement) also write every key of the reading's domains that the start lacks, at 0.
  - pos:, escpos: and actorroom: keys are not written, because 0 is not their default.
  - Test: `the_reach_start_writes_the_domains_cleared_keys_at_0`. fmi-explore's lib (52) and integration tests pass
    on the VM build.
- play9 resumed from its archive at 23:09:42 UTC with the switch on.

## Result (2026-11-05): play9 reached C4, verified from boot; below C5, so E016 stops here

**Furthest checkpoint: C4.** The ladder over the whole archive gives:

| Rung | Entry | Try | Kind | Wall (min) |
|---|---|---|---|---:|
| C0 | e3 | j00000 | planned | 2.1 |
| C1 | e128 | j00045 | model-free | 4.6 |
| C2 | e136 | j00048 | model-free | 4.7 |
| C3 | e144 | j00051 | model-free | 5.4 |
| C4 | e3412 | j01004 | model-free | 121.4 |

- **Verified from boot:**
  - The five rung entries were verified on the VM with the run's last build, each with 0 differences.
  - So did a sample of 48 of the other 4,171 entries: 48 of 48 had 0 differences. The sample is not representative:
    all 48 are in room 49, 12 come from one try (j03914), and none holds 946.
  - Verifying every entry (about 8-9 entries a minute at 24 at a time, about 6.8 h for all) did not fit before the
    VM's booked stop.
- **How it stopped:** `stop file`, asked by the ladder's watcher after 40 min of wall with no new rung, at 23:57 UTC.
  The watcher's count restarts at each resume. About 110 min of run wall passed after C4 (21:52) with no new rung, and
  under the rule as written the run would have stopped about 22:41, before (13).
  That was 4,052 tries (2,030 model, 2,022 free), 4,176 entries and 386 cells. Its entries are in rooms 49 (3,996),
  69 (127) and 63 (53). No try visited room 42.
- **The segments**, each a resume from the archive's files:

  | Segment | UTC | Tries | What changed at its start |
  |---|---|---:|---|
  | 1 | 19:50-20:28 | 187 | the start (after two void starts, (8)) |
  | 2 | 20:29-21:08 | 456 | (9): a lock per planner; FMI_PLAN_DEEPEN=10 |
  | 3 | 21:09-21:45 | 340 | (10): FMI_NOVEL_PLACE; realize's place_of |
  | 4 | 21:49-22:31 | 1,080 | (11), (12): FMI_SELECT_PLACE; FMI_PLAN_TIME=30 (C4 at 21:52) |
  | 5 | 22:39-23:09 | 828 | the VM's restart after its booked stop |
  | 6 | 23:16-23:57 | 1,161 | (13): FMI_REACH_ZEROS |

  The resume at 23:09 was refused by the 10 GB start floor and redone at 23:16 with 5 GB.
- **Stalls over the run**, 115,773 s of try wall:

  | Class | Share |
  |---|---:|
  | interface, no yield | 62.9% |
  | model, no yield | 17.3% |
  | no plan | 10.7% |
  | refused input | 4.0% |
  | effect mismatch | 3.9% |
  | productive | 0.6% |
  | wait | 0.3% |
  | model-blind | 0.2% |
  | reading bound | 0.0% |

  The reading bound took 0.0% (play8: 65.0%). The last 20 tries were 99.6% interface with no yield.
- **The reach fix (13): not measured by the run.**
  - "Did not reach either" probe stops numbered 137 in 15 of the 2,891 tries before it, and 8 in 3 of the 1,161 tries
    with it. The 8 are all a script local at 0.
  - Before the fix the stops were 106 bit:404=0, 26 local:209 or local:202 at 0, 4 var:104=1 and 1 pos:script:86.
    All of them were in j00042-j00948.
  - After the fix, no try started from the bit:404 entries (23, 238, 240, 552, 553, 1296, 1298), and entry 46 failed
    on local:209=0 as before. So the drop comes from which entries were selected, not from the fix.
  - Its only evidence is the offline planner-only check of (13).
- **Why C5 was not reached:**
  - C5 needs room 42 while holding 946.
  - play8 reached room 42 without 946, from a room-49 conversation, with a model plan and then a free try.
  - E015's item gate found room 42 with 946 only from one of the opening's two dialog choices. play9's conversation
    states took the other one (bit:401=1, entry 139).
  - play9's entries 240 and 549 differ from play8's entry 198 only in bit:401. So the data does not show that the
    reach defect alone kept play9 from room 42.
  - (12)'s 30 s cap is a confound. Of play9's 2,341 found plans, 3 took more than 30 s: j00110's 152 s replan, which
    reached var:416=2, and j00406 and j00552. Each was in a try that reached its target. After the cap, 3 replans
    timed out at 30-31 s.
  - None of play9's 7 entries deep in the conversation (var:416 != 0) holds 946. After (13) no try made a new one.
- **Cost:** two VM bookings, 19:01-22:31 (3.5 h) and 22:35-00:3x (2 h), at $1.535/h. Laptop: the checks below.

### Predictions against the result

| Prediction | Result | Score |
|---|---|---|
| Furthest: median C3, 80% interval C1-C7 | C4 | hit |
| P(at least C1) 80-90% | yes | hit |
| P(at least C3) 50-65% | yes | hit |
| P(at least C5) 25-40% | no | the likelier side (60-75% "no") |
| P(at least C7) 10-25% | no | the likelier side (75-90% "no") |
| P(C9) 5-15% | no | hit |
| The first C1 within 2-20 min | 4.6 min | hit |
| Reading bound 5-30% (play8 65.0%) | 0.0% | better than the range |
| Refused input and effect mismatch together 0-10% | 7.9% | hit |
| Productive 5-25% (play8 1.6%) | 0.6% | miss, worse than play8 |
| G3 passes, 70-85% | pass | hit |
| G4: every kitchen step realized, 60-80% | fail along the chain | miss |
| G5, 95% | pass | hit |
| K1 reaches the kitchen in 60 min, 45-65% | not measured: killed twice by the laptop's memory guard, once on its load | none |
| New assumption rows 2-6 | 0 rows added to the assumptions table; 7 problems found by running ((7)-(13)) | the measure was not kept; counted as problems, a miss |
| Repeated (entry, prefix, target) attempts: 0 by construction | not measured | none |

- **Success threshold:** No (C4 is below C5).
- **Stop rule:** play9 stopped below C5, and so did its one amended rerun, segments 4-6 under (11)-(13). Patching
  stops here. E016 is written up, and the next direction goes to Ilyas.

### What carries forward, and what does not

- **Worked, by measurement:**
  - Fix A, realize, on the run path: exit A at 4.6 min, after play8's 22 failures.
  - Room scopes as plan F meant them (8).
  - The reading bound no longer stalls, at 0.0%.
  - Place novelty and place-first selection, (10) and (11), made C4: no entry outside room 69 held 946 until they were
    on.
  - The reach start's zeros (13) are shown only offline (the run never re-tried the states they block).
- **Found by running, not by design:** five defects in E014-E016 code, all general:
  - the widened reason;
  - unsetting FMI_SCOPE_GROW gives ring scopes;
  - one lock over all planners;
  - realize places a held object;
  - reach starts drop zeros.

  Two missing mechanisms, the place as a novelty unit and in the selection, came from the same runs. That is the
  retrospective's pattern again: each new area of the game showed new defects.
- **Open:**
  - **Rule (2).** Monkey Island's checks (b3, kitchen, smoke) and K1 with the final set ((8)-(13)) are not done.
    Before (9): kitchen and smoke passed with room scopes. After it, b3's run died under the laptop's load. Nothing
    merges to main until they pass. G4's deferred sentence is still open as well.
  - **Verification.** Full verify-from-boot of all 4,176 entries; the archive is kept on the VM's disk, and on the
    laptop without readings.
  - **The C5 route.** The opening's dialog choice that leads to room 42 with 946, per E015's item gate.

### Corrections after the independent check (2026-11-05)

Two checkers re-derived the Result from the pulled archive and read each fix's code. The headline holds: a rerun of
the ladder over all 4,176 entries gives the same rungs, with C4 at 121.4 min, and every count and verification
above matches. The corrections are folded in above. These are added:

- **(9) SharedPlanner has no switch and no test.** It changes every run with b > 1, Monkey Island with every switch
  off included. Plans on different readings now search at once with no memory cap. The cache's lock is still held
  while a planner is built. A poisoned planner is used again. It needs a switch, or b3 and smoke with every switch off
  shown unchanged.
- **(12) is not "every plan at most 30 s".** First plans get 10 x 2^n s (n deferrals; at most 20 s in play9), and
  later plans 30 s. On Monkey Island, FMI_PLAN_DEEPEN=10 is likely to slow K1: E012's kitchen chain has a 24.6 s first
  plan (j00013).
- **(10) changed realize after (5) and (6)** said fix 3 was the last change to realize in E016. G3 and G4 are owed on
  realize at 6902065.
- **Segment builds were made on the VM from the working tree.** Two resumes started seconds before the commit that
  records their code (20:29 against 4d19fe1 at 20:30:59; 21:08 against 6902065 at 21:09:24). The logs hold no build
  revision.
- **Small counts:**
  - (10)'s room-63 trials were 9 points, each with 38 trials ending in room 63 (342 in all), none kept.
  - The resume at 21:08 was at 643 tries and 1,342 entries.
  - At 21:41, 983 tries and 3,386 entries.
  - "22 failures at exit A" is play7 and play8 together, per E015.

### (14) 2026-11-05: the per-planner lock behind a switch; rule (2) owed

- Following the check, (9)'s per-reading planner lock is now behind `FMI_PLANNER_LOCK=reading`, which E16 sets. With
  it unset, a plan holds the planners' cache lock for the whole request, as before (9). Monkey Island with every
  switch off is then unchanged by it.
- With this build (target-e016, laptop) the following run on the laptop: Monkey Island's checks (b3, kitchen, smoke),
  K1, and G3 and G4 on the final realize.
- A video of play9's C4 path replayed from boot was made with the bridge's `SPEEDRUN_VIDEO` capture: 18 clicks,
  131.7 s, ending at the entry's tick (7382) and frame (1646). It is not committed (game images).
