# E012: The game's own mechanics: time passing, random draws, the interface's inputs, the game's end

Date planned: 2026-11-03
Research question it serves: RQ1 (completion from the game files alone), RQ4 (what carries over to another engine)

## Question

E008-E011 explore from the real game: return to an entry, read the rooms around it, plan the player's inputs on the
reading, play them in the engine, keep what the play reaches. Everything the system tries comes from a plan. Can the
explorer instead act the way the game is played, splitting each step by who decides it?

- The **player's moves** (verbs, objects, walks, dialog lines) are planned on the reading.
- The **game's moves** (time passing, random draws) are played by the engine, exactly, and never searched by the planner
  nor waited in real time.
- Where the model cannot act, the explorer uses **the inputs the interface itself offers** at that frame.
- The **game's own end states** mark dead moves.

The test is the k = 2 kitchen check that E011 left unreached, then Part I.

The PI, 2026-11-03: think from the games themselves. Waiting is common to adventure games. Random outcomes must be
handled: this game's random opcode, and in Sierra games like King's Quest, pickups and events drawn from an RNG pool
where a player retries actions to get the right one. The explorer must not be blind to what comes up when actually
playing. One coherent fix around every issue found so far.

## Baseline: E011's kitchen check

Source: `experiments/E011-speed/plan.md`, "Kitchen check, result", and the archive
`/var/tmp/fast-mi/e011-archives/kitchen` (log.jsonl, entries, read only).

| | measured |
|---|---|
| result | stopped by hand at 65 committed tries; goal not reached; 226 entries in 6 of the 10 rooms (33, 38, 35, 29, 34, 28) |
| try wall | 6,077 s in all (1.69 h); median 31.6 s, max 998 s |
| where the wall went | 3,355 s (55%) in the 14 tries that ended on "the same empty window 2 times"; 1,273 s (21%) in the 17 tries that ended on Lemma R's R0; 1,084 s in the 28 tries that reached their target |
| R0 stops | all 17 on var 194, the dialog variable (global 14 maps a dialog click to it, `docs/part1/input-scripts.md`): 8 from room-33 entries after 84-134 frames of play, 8 from room-38 entries, 1 from room 28's entry 205 |
| targets | var:103 (the sentence's first object, a fact since its refinement) was the last target of 29 of 65 tries; targets per try 0 to 89 |
| planner limits | 10 requests reached the 180 s limit with no plan |
| the goal | a target once: j00061, from entry 205 (room 28, frame 3443). 180 s with no plan. The next target (room 41) stopped on R0 at 3444, before any input |

The issues this plan takes up, as numbered in the task:

1. a goal that may need a wait, and targets with no plan cost the planner's whole limit;
2. targets that are values of a refined variable;
3. entries the harness cannot act from (R0 on var 194 in a dialog);
4. the guide does not narrow (R about 1,162 atoms for every goal);
5. only planned inputs are ever tried.

## Research (sources opened)

**ScummVM's random number generator** (`third_party/scummvm`, the E011 build's tree at
`/var/tmp/fast-mi/svm-e011/third_party/scummvm`):

- One `Common::RandomSource _rnd` per engine (`engines/scumm/scumm.h:558`). It is seeded from ConfMan's
  `random_seed` (ScummVM's `--random-seed`) when set (`common/random.cpp:42`), and otherwise from the clock.
- `getRandomNumber(max)` runs one xorshift32 step (`scrambleSeed`, 13/21/11, `common/random.cpp:82`) and returns
  `(state * 0xDEADBF03) % (max + 1)` (`:62`). The state advances **per call, never per frame**.
- In SCUMM v5 the calls come from `o5_getRandomNr` (opcodes 0x16/0x96, `script_v5.cpp:1438`, range 0..n inclusive) and
  from the dissolve transitions 134/135 (`gfx.cpp:4623`). The other `_rnd` uses are v6+ or HE only.
- `o5_expression` runs an embedded opcode (its case 6, `script_v5.cpp:1182-1184`), so a draw inside an expression
  bypasses the bridge's per-opcode hook.
- The bridge already reports the state in every Status and F2 block (`rng`). It has a search-only `SetRng`, which never
  goes in a movie.
- AGI (`engines/agi/op_cmd.cpp:156-168`, `cmdRandom`) and SCI (`engines/sci/engine/kmath.cpp:38`, `kRandom`) use the
  same `RandomSource` under ScummVM.
- The original AGI interpreter used a 16-bit LCG reseeded from the clock when its state is 0 (AGI wiki, "Random",
  https://agiwiki.sierrahelp.com/index.php/Random). A TAS of an AGI game could only steer its random item layout by
  the time of day the game started, that is the seed, and notes that ScummVM's generator differs
  (https://tasvideos.org/Forum/Posts/515677). Under our rules the seed is fixed (seed 1). The levers left are in-game:
  the frame of each input, and inputs that consume draws.

**Delays and timers in SCUMM v5:**

- `o5_delay` and `o5_delayVariable` set the slot's `delay` and pause it (`script_v5.cpp:972`, `:982`).
- Each frame, `decreaseScriptDelay(delta)` (`script.cpp:1535`, called from `scumm.cpp:3134`) lowers every paused
  slot's delay by the frame's delta. The slot runs again in the frame its delay goes below 0.
- So the next frame a paused script wakes is in the state: the dump lists every slot with its `delay`
  (`speedrun_state.cpp:568`).
- Other time-driven changes are polled: loops with `breakHere`, waits on actors, messages, the camera or a sentence
  (`o5_wait`), and the clock variables (excluded from facts, `fmi_core::CLOCKS`).
- The game runs on the bridge's tick clock with no real-time pacing, so "time passes" costs engine frames only.

**The interface** (what the bridge can list):

- A verb is a slot. It is visible when `verbid != 0`, `saveid == 0` and `curmode == 1`, which is the engine's own test
  (`speedrun_state.cpp:143`, the same as `findVerbAtPos` in `verbs.cpp:1009`). The dump lists each visible verb's id,
  position and key.
- In this game the dialog lines are verb slots too, as are the inventory slots.
- The room's objects come with box, state, parent, walk point and name. `findObject` (`object.cpp:557`) skips class 32
  (`kObjectClassUntouchable`, `object.h:54`), which the dump's classes give.
- Also available: actors in the room (`speedrun_state.cpp:618`), the inventory, the hit test `At` (round 1) and
  `AtMany` (round 2's patch 0004).
- Not listed: the walk boxes.
- The bridge's idle rule names this game's verb id ranges (2-10, 120-128; `speedrun_state.cpp:40-60`). Nothing in
  E012 uses those ranges.

**End states:**

- `o5_systemOps` has three sub-ops (`script_v5.cpp:2198`): 1 restart, 2 pause, 3 quit.
- `restart()` (`scumm.cpp:4222`) clears the variables and room and runs the boot again in the same process.
- The bridge ends a run on quit. A restart it does not report.
- In this game's scripts (statistics only): restart at 9 sites in one global script, quit at 1 site in another.

**TAS practice** (TASVideos, "Advanced Luck Manipulation", https://tasvideos.org/AdvancedLuckManipulation):

- RNGs are sorted by what advances them: frame-based, use-based ("only changes when a new random number is
  required"), input-based, CPU-cycle-based.
- Manipulation is a savestate plus retries: wait k frames, or act so that draws are consumed, until the outcome is the
  wanted one. A frame-based RNG is detected by waiting a set number of frames and checking whether the result changes.

**Go-Explore** (Ecoffet et al., arXiv 1901.10995v4):

- Exploration returns to a cell by restoring the simulator's state, exploiting determinism ("solve ... by any
  available means (including by introducing determinism)").
- Stochasticity is left to a later robustification phase.
- Cells are chosen by count subscores of the form w/(v + ε)^p: times chosen, times chosen since the cell last led to
  something new, times seen. With domain knowledge, a level weight 0.1^(max level − level) is added. Life loss is
  handled in the cell representation.

**ALE** (Machado et al., "Revisiting the ALE", arXiv 1709.06009):

- The emulator is deterministic, and a "Brute" that memorizes an action sequence exploits this.
- No-op starts and sticky actions were introduced to inject variation.
- A speedrun is exactly such an open-loop movie on a fixed seed, so no robustification phase is needed here. Idle
  frames before an action, the no-op lever, are what steers draws.

**IW** (Lipovetzky, Ramírez, Geffner, IJCAI 2015, "Classical Planning with Simulators: Results on the Atari Video
Games"):

- IW(1) is a breadth-first search that prunes a state unless it is the first to make some atom X = x true. On the
  Atari games the atoms were RAM bytes.
- Actions apply every 5 frames, and novelty is judged right after.
- An effect that shows only later is judged on the state before it shows. E012 judges novelty after the game's own
  reaction (control back, then the wait's stops).

**Text-parser games** (AGI wiki, "Said (AGI)", https://wiki.sierrahelp.com/index.php/Said_(AGI); ScummVM
`engines/agi/op_test.cpp:329` `testSaid`):

- Input is parsed into word groups from WORDS.TOK.
- `said()` compares the groups with its arguments: group 1 is any word, 9999 the rest of the line.
- A parser game's meaningful inputs are therefore the `said()` patterns in its logic resources: finite, and readable
  from the files, like SCUMM's visible verbs × hit-testable objects.

## Measurements for this plan

Method:

- Laptop, seed 1, round 1's lean engine (`/var/tmp/fast-mi/svm-e011`).
- Entries rebuilt from boot from the read-only kitchen archive, each entry's movie being its chain of segments.
- A scratch probe over `fmi-engine`, not committed: Step per frame with the facts and the explain log; the Obs for the
  interface; forks for the trials.
- Tested keys come from the kept reading `readings/28-33-41-j00065`: the union of the actions' `pre_keys`, less each
  event's keys on its own unit. That gives 100 keys: 60 tested by inputs and 49 by other units' events.

**M1, the RNG advances every frame.**

- Every frame of every run had at least one call: global 159 draws once per frame.

  | entries | calls per frame |
  |---|---|
  | room 33 (2728), room 38 (2848, 2862) | 1 |
  | room 29 | 1-2 |
  | rooms 34-35 | 1-10 |
  | the 7 room-28 entries | 2-6 (6,982-10,008 calls in 3,000 frames) |

- Calls between two frames are counted exactly by stepping xorshift32 from one `rng` to the next.
- At entry 205 the explain log showed 6,493 getRandomNr opcodes for 9,997 calls. The rest are draws inside
  expressions (above). So the RNG is read from the state, not from the log.

**M2, time passing in room 28.**

- At entries 203, 205 and 225, room 28's local 211 is paused with delay 2,760, 2,460 and 1,938 ticks. The frame's
  delta is 6 ticks (tick 16,321 at frame 2728, 20,611 at 3443).
- The cook's door (state:316, with state:570, local 211 and local 216) first changes at +461, +411 and +324 frames.
  That is exactly floor(delay / 6) + 1 each time.
- Over 3,000 frames 66-117 keys change. 41 other object states (animations) and var 100 change, many every few frames
  (var 100 takes 233 values in 1,500 frames, E008). None of them is tested.
- Of the tested keys, 5 change at the entries idle in a sentence or dialog: local 208, local 209 (two animation
  scripts, first change at +2 to +13, then 340-450 changes each), local 211, state:316 and state:570.
- 11-23 tested keys change at the entries that start in a cutscene: the verbs, var 32, var 52-53 and script 23 change
  when control comes back, at +11 to +85.
- The door then cycles: open 110-143 frames, closed 312-502 (the next delays are draws).
- Cost: 3,000 frames in 0.85-0.97 s, with a round trip, the facts and the explain log per frame.

**M3, dialogs.**

- Entry 4 (room 38, 2862) idles in a dialog. Over 1,600 frames only var 100 changes, and no slot is paused: time
  changes nothing there.
- **Entry 205, where E011 requested the goal, is a dialog**: idle kind "dialog", 4 lines (verbs 120-123).

**M4, the interface's inputs.**

What the interface offers:

| entries | visible verb slots | room objects (touchable and named) | actors | inventory |
|---|---|---|---|---|
| sentence idle | 18 (9 action verbs, 8 inventory slots, the sentence line) | 4-52 (3-15) | 1-4 | 0-1 |
| dialog | 4 | | | |

Trials, each in a fork:

| entry | trials | horizon | wall | distinct outcomes | what happened |
|---|---|---|---|---|---|
| 205 (room 28 dialog) | 4 lines | 600 frames | 0.40-0.44 s | 2 | line 4 ends the dialog, control back (sentence idle) at +26; lines 1-3 lead to the next menu at +76 to +86 |
| 4 (room 38 dialog) | 4 lines | 600 frames | 0.20 s | 1 on room 28's tested set, rooms, bits and owners | room 38's own tested set was not available (its readings are evicted) |
| 203 (room 28 sentence) | 246: each verb alone, each target alone, each verb then target | 400 frames | 11.7 s (47.5 ms each) | 8 | rooms 72 (20 trials) and 79, 81, 82 (2 each), none in the archive's 6 rooms; 194 trials changed nothing |

At entry 203 the hit test reached 18 verbs and 12 targets.

The same four inputs at 203 delayed by 0-31 frames gave one tested outcome each: no tested outcome in that sample
depends on a draw.

**M5, draws in the reading.**

- 28-33-41-j00065 has 39,661 actions: 23,781 events, 15,712 sentences and 168 verb clicks.
- 8,201 actions check a random draw. All of them are events of room 28's animation locals: 208 and 209 (4,096 each)
  and 210 (9).
- Checks by non-fact reason: random draw 8,207, timer 405.

**M6, draws in the scripts** (statistics only):

- getRandomNr appears at 335 sites in 135 scripts.
- Constant ranges: 0..1 at 145 sites, at most 0..5 at 225 of 302, up to 0..232. 33 sites have a variable range.
- Evidence of draws that gate progress in Part I, from the human analysis notes of the earlier hand model (not inputs
  of the system): the storekeeper is away on a store entry when a 0..3 draw is 0 (`docs/part1/money.md` B3), the cook's
  timer is a 0..20 draw (the same file), and the map's duels are random (`docs/part1/rooms.md`, room 49).

## What the research changed in the proposal

1. **The failure at 3443 was a dialog, not a wait** (M3).
   - Entry 205 is idle in a dialog, and one of its four lines returns control in 26 frames.
   - Every R0 stop of the check is var 194 in a dialog (baseline).
   - So issue 3, not the cook's timer, explains the goal's one failed request. The interface's inputs (mechanism 4)
     are what the case needs.
   - Whether the goal needs the cook from a room-28 entry with control is not measured: no request was made from one
     (entry 203 was never selected). The rerun answers it.
2. **"Until a tracked fact changes" would stop every frame** (M2).
   - Var 100 takes a new value most frames, 41 animation states cycle, and two animation scripts start and stop every
     few frames.
   - The stop is instead the first frame at which **a precondition atom of the reading changes truth for the first time
     since the wait began**: IW(1) novelty along the no-input trajectory.
   - Its horizon is exact from the paused slots' delays.
   - In room 28 the stops are the two animation scripts' first start or stop (+2 to +13), then the cook's door at the
     slot's due frame (3 of 3 entries).
3. **E009's idle set hides the door.** The door cycles, so the idle measurement put state:316 in the idle set (j00061's
   idle keys). Cells and held atoms without it cannot see the door open as progress. E012 keys cells and progress on
   tested atoms instead.
4. **The RNG is advanced per call, but this game draws every frame** (M1).
   - So each frame of delay before an input changes every later draw: TAS's frame-delay lever applies, as well as the
     use-based one.
   - The calls are counted exactly from the state, not from the explain log, which misses draws inside expressions.
   - The reader already marks every branch on a draw (M5).
   - In the kitchen scope no tested outcome measured depends on a draw, so the delay fork matters for Part I (the
     storekeeper, duels), not the kitchen.
5. **Affordances need no new engine view but the walk boxes.** The bridge lists verbs, objects, classes, actors and hit
   tests. Their cost and yield are measured (M4). They generalize as "the inputs the game's own input code tests for":
   SCUMM's verbs × hit targets, AGI's `said()` patterns.
6. **The wall goes to empty windows** (55%). The due frame from a paused slot's delay is an exact bound for a planned
   event of that slot, where E010's window waits 2 × 3,600 frames.
7. **Restart is not reported** by the bridge, so patch 0005 adds it.

## Method: one layer, split by who decides

Every mechanism below:

- makes entries only from real engine runs, each verified from boot like any entry (A0, Lemma C);
- prunes no state from the archive;
- uses only the engine's mechanics (slots and delays, the RNG state, verb slots, hit tests, systemOps) and the reader's
  own records.

No genre, puzzle or speedrun rule is written (D9).

### 1. Time passes: the wait move

From an entry, the engine runs with no input.

- **Tested atoms.** T is the set of (key, values) atoms in the preconditions of the reading's actions (inputs and
  events), less each event's atoms on its own unit (its slot, position, arguments, freeze), plus the goal's atoms.
- **Stops.** A frame is a stop when some atom of T changes truth for the first time since the wait began (its truth at
  the entry counts as seen). The number of stops is at most |T|.
- **Horizon.**
  - H = max over the paused, unfrozen slots of floor(delay / Δ) + 1 frames, with Δ the measured ticks per frame.
    H is re-read at each stop.
  - After the last due wake, W more frames with no stop. W = 1,600, E009's measured length at which every idle set of
    15 entries equalled its 3,200-frame set.
  - If there is no stop by then, the entry is **static** for waiting.
- **Exactness.**
  - The run is a fork of the entry. Each stop's state is the entry's movie run to a later frame, exact by A0.
  - The horizon follows the engine's own rule (`decreaseScriptDelay`: a paused slot runs in the frame its delay goes
    below 0).
  - Nothing is pruned: a stop only decides which frames are offered as candidate entries. Any other frame stays
    reachable by a later wait or by the window search.
- **Kept.** A stop becomes an entry when it holds an atom of T, or a room, that no archive entry holds: IW(1) novelty
  at the archive's level.
- **In the try.** The wait replaces E009's idle measurement and runs in the same fork over the same frames. Its idle
  keys are still reported. Its cost is the idle measurement's (M2: well under a second).
- **The planner plans only up to where the player must wait.**
  - When tier 0 (the goal) is proven dead or deferred from the entry (round 2's dead-target proofs, deepening) and the
    wait has stops, the goal is requested again from each stop, on the same reading with a new start, before the other
    targets.
  - In E010's window search, a planned event of a slot that is paused at the window's start gets the exact due frame
    as its window. The event cannot run before the slot's delay expires, and runs in that frame unless the slot is
    frozen (the freeze count is in the dump).

### 2. Random draws: the delay fork

The reader treats a draw's result as a non-fact (Reason::Random), so the planner may pick either branch of a check
on a draw. For the player, that pick is a requirement on timing.

- **Trigger.** The plan's next input is followed, up to its next checkpoint, by a record whose checks include a random
  draw (`mech-tested`'s draw checks).
- **The fork.**
  - The window search's children at successive frames where the input is taken (E010's play when ready gives the
    first) are run to the checkpoint.
  - The first child whose state matches the plan's branch is taken: the smallest delay, as in TAS.
  - Every other child whose tested outcome differs and holds an archive-novel atom is a candidate entry.
- **Bound.**
  - K(p) = ceil(ln 0.01 / ln(1 − p)) frames, where p is the planned outcome's share of the draw's range. p comes from
    the check by interval arithmetic, as `costs.rs` prices draws, or is 1/(n + 1) when not computable.
  - Examples: K = 7 for p = 1/2, 17 for 1/4, 95 for 1/21.
  - The bound assumes the successive xorshift32 outputs are uniform over the range. Gate R2 checks that.
  - If no child matches by K, the branch is refuted at this entry: the planner gets the observed value. Nothing else
    is pruned.
- **Exactness.** Every child is a real fork at a later frame. `SetRng` is never used outside measurements.
- **Measured.** Per try, the controller reports the calls per frame (exact, from the F2 `rng`) and each fork's
  outcome histogram.
- **Generality.** The same holds for any ScummVM engine whose scripts draw each frame (to be measured per game, as M1
  did here). Where scripts draw only on actions (AGI's room entry, KQ's pools), the lever is the use-based one: the
  delay fork still applies at the input that triggers the draw, and inputs that consume draws come from the planner's
  plans and the affordance step.

### 3. Targets from tested facts

- An atom the guide gives as "any value" (value null, e.g. var:103) and the values a refinement adds give **one target
  per key**: the goal {key: every reached value other than the entry's}. The planner takes a value list.
- A value is its own target only when the guide names it.
- Order: tier 0, then targets whose key is tested by the reading (in T), then E008's (tier, depth, key, value) and
  E009's rotation.
- Soundness: ordering and collapsing prune no state. The one request reaches any of the key's values.
- A key no action of the reading tests cannot enable an action in the scope, so it goes after the tested ones. It
  still serves readings of other scopes.
- On issue 4: R stays the guide (E008 measured that control dependence alone gives 961 atoms). The narrowing that
  matters per try is T: 100 keys in the room-28 reading, against R's 805.

### 4. The interface's own inputs when the model cannot act

**Triggers.**

- Lemma R's R0 stop.
- No target of the try plans (every request proven dead, deferred or at its limit).
- The same empty window twice.

**Enumeration**, from the engine at that frame:

- **Dialog idle:** one click per visible verb slot.
- **Sentence idle:** each visible verb slot alone; each hit-testable target alone (the default verb); each verb then
  each target.
  - Targets are room objects not of class 32 with a hit point found by the hit test (AtMany), and actors in the room.
- **A third click** over all hit targets is added when the second click left the game composing. That is the case when
  only the engine's input script (the unit `VAR_VERB_SCRIPT` names) ran and the bridge is idle again, which covers the
  two-object sentences.
- **Walk boxes:** one click per walkable box, once patch 0005's box list is in (switch, measured first).
- No verb id list and no game knowledge is used. Visibility and hits are the engine's own tests.

**Trials.**

- Each input runs in a fork until the bridge's idle rule holds again, or the wait's horizon passes, then through the
  wait's stops.
- An outcome is kept as an entry when it holds a tested atom or a room no archive entry holds (IW(1) novelty).
- Exactness: each kept outcome is a real run, a movie with its clicks. A dropped one can be made again from the same
  entry.

**Measured cost** (M4, round 1's engine with a fixed horizon): a dialog in 0.2-0.44 s; a room-28 sentence entry in
11.7 s for 246 trials. Predicted on round 2's engine with the idle stop: 3-20 ms per trial.

**Generality.**

- AGI: the `said()` patterns of the loaded logics (the room's and logic 0), over WORDS.TOK's groups. Group 1 and 9999
  are filled from the vocabulary.
- SCI parser games: the Said specs. SCI point-and-click games: verbs × features.

Each engine gets an enumerator over its own input code. The trial, novelty and archive are shared.

### 5. The game's end

- A move whose run ends in quit, or in restart (patch 0005: `restart()` and SO_PAUSE reported by the bridge), is
  marked **dead**. Its entry is never selected.
- Restart clears the variables and the room and runs the boot (`scumm.cpp:4222`). Any state reachable after it is
  reachable from the root in fewer frames, so the move is dominated for the time objective. This is D4 (clarified)'s
  proven removal.
- An entry that is static for waiting, offers no input and has no plannable target is **exhausted**, not dead.

### 6. Selection by real progress

- **Cells:** (room, owned objects, the held atoms of T other than script-control keys: slot alive, position,
  arguments, freeze). Those control keys cycle with animation scripts, the world does not.
- **Order**, deterministic, after Go-Explore's count subscores:
  1. times chosen since the cell last gave a new atom, ascending;
  2. times chosen, ascending;
  3. rooms on the entry's chain, descending;
  4. novel tested atoms held, descending;
  5. frame, then id.
- Dead entries are skipped. Exhausted entries are skipped until T grows.
- **Stagnation:** a full sweep of the selectable cells with no new atom of T and no new room.
- Ordering only, so no soundness argument is needed beyond E009's (every entry stays selectable).

### How a try runs

1. Return: a fork of the kept entry.
2. Reading of the scope (round 2), then T and its draw checks.
3. Wait (1): stops become candidate entries; static or not.
4. Targets (3), then the planner (round 2, dead-target proofs and deepening). The goal is requested again from the
   wait's stops when it is dead from the entry.
5. Play: the window search, with due frames and the delay fork (2).
6. Affordance step (4) at the frame where the model could not act.
7. Commit: the candidate entries kept by novelty, and the dead marks (5).

## Components (on top of round 2 and the dead-target branch)

Code starts from `e011` once round 2 (`e011-r2`) and the dead-target branch (`e011-deadtarget`) are merged into it.
Each component is on its own branch `e012-<name>` with its own worktree. Each switch is off by default. With every
switch off, the 15 B0 tries equal round 2's in every stop, frame, input and entry (gate G0).

| component | where | interface | switch | gates |
|---|---|---|---|---|
| `end` | patch 0005, `fmi-engine` | End reasons "restart" and "pause" from hooks at `restart()` and SO_PAUSE (observation, then end the run). Optional `Boxes` command: count, corners, flags per box (`boxes.cpp`), for walk affordances | `SPEEDRUN_REPORT_RESTART`, `FMI_BOXES` | G1-G4 of E011 unchanged with the patch; the hooks are unreachable on every recorded chain; `Boxes` equals ScummVM's box data in 3 rooms (binary against the JSON dump) |
| `tested` | `fmi-reading` | `Tested::of(&Reading) -> Tested { atoms: per key the tested value sets, inputs, events }`; `draws(&Reading, action) -> Vec<Draw { check, range: Option<(i32, i32)>, share: Option<f64> }>` | none (data) | T1: on 28-33-41-j00065, 60 input keys, 49 event keys, 100 in all, 8,201 actions with a draw check (this plan's numbers); atom counts reported |
| `wait` | `fmi-explore` (and a provided `Engine::wait` over round 2's `step_changes`) | `wait(engine, &Tested, w) -> WaitOut { stops: Vec<(frame, Vec<atom>)>, end: Horizon \| Static \| Ended(reason), frames, seconds }`; due frames for the window search: `due(&Obs, slot) -> Option<frame>` | `FMI_WAIT` | W1 every wait entry verifies from boot, 0 differences; W2 the door's stop at +461, +411, +324 from entries 203, 205, 225 and `due` equal to them; W3 entry 4 static on its scope's T; W4 cost ≤ 0.3 s per wait on round 2's engine |
| `delay` | `fmi-play` (window search) | `DelayFork { k_max(p), chosen: k, outcomes: Vec<(k, digest, novel)> }` in the window's record | `FMI_DELAY_FORK` | R1 entries verify from boot; R2 xorshift32 outputs mod n over successive states, for n = 1..232 from state 3182679328 (the root's), uniform by χ² at 1% (computational); R3 the window record's k equals the smallest matching k on a fixture of forced branches |
| `targets` | `fmi-explore` (step.rs `targets`) | one target per any-key and per refined key (`TV::Values`); the T tier | `FMI_TARGET_PER_KEY` | T2: the 65 logged kitchen tries' targets recomputed from their job files: identical except the collapsed keys; var:103 gives at most 1 target per try |
| `afford` | `fmi-explore`, with `fmi-play`'s hit tests (AtMany, hit cache) | `affordances(engine, &Obs) -> Vec<Inputs>`; `try_all(engine, inputs, &Tested, archive) -> Vec<Kept>` | `FMI_AFFORD` | A1 kept entries verify from boot; A2 at 205 two outcomes, the line that returns control at +26 (this plan's); at 203 the 8 outcomes of M4 with the fixed horizon; A3 ≤ 20 ms per trial on round 2's engine |
| `archive` | `fmi-explore` (archive.rs, driver) | cells over T, novelty over T and rooms, dead and exhausted marks, the order of (6), stagnation over T | `FMI_SELECT=progress` | S1 unit tests of the order and the recount; S2 the E011 archive's entries recounted under the new cells (reported) |
| `try` | `fmi-explore` (tryjob.rs), `fmi` | the try's order above; the triggers of (4); the goal again from the wait's stops | the switches above | G0 off-equality; G-on: the 15 B0 tries with every switch on keep every new entry verifiable from boot |

The interface types go into `fmi-core`'s appendix (E011 Appendix A) as additions only, copied byte for byte by the
components that share them.

## Runs

- **Kitchen rerun** (first, once every gate passes):
  - `fmi explore` from the boot into a new archive `/var/tmp/fast-mi/e012-archives/kitchen`, to owner:566 = 1.
  - The same 10 rooms, b = 2, E009's switches, play when ready and commit as finished.
  - Round 2's fast settings, the dead-target proofs and deepening on.
  - Every E012 switch on. E009's copy of E008's guides.
  - Falsifier: 80 tries. The goal entry is verified from boot twice, in a fresh lean engine and on svm-w3b through
    E008's `explore.py verify`.
- **Part I** (only after the kitchen's yes, and after a dated amendment with the projection from the kitchen's measured
  try costs):
  - Goal `part1` (owner:901, 902, 903 = 1), the 80 rooms from the files, the same switches.
  - E009's falsifier of 600 tries and stagnation over T.
  - Reported: the Part I rooms entered and E001's key facts held on the best verified chain (evaluation only, never an
    input).
- Reported per run:
  - each mechanism's counts: waits, stops, statics, due-frame windows, delay forks with k and outcomes, affordance
    steps with trials and kept, dead marks;
  - the wall by part and by stop kind, against E011's table above;
  - calls per frame;
  - the cases where W or K bound.

## Prediction (before any code)

| | predicted |
|---|---|
| gates | G0, T1, T2, W1-W4, R1-R3, A1-A3, S1 pass, each after at most one fix round |
| wait | ≤ 0.3 s per entry. At room-28 entries with control, a stop at the cook's due frame (exact). Static in room 38's dialog |
| kitchen, reached | yes, verified from boot with 0 differences, in 3-25 tries (most likely 5-15) |
| kitchen, chain | 488-1,300 frames from 2728 (E006's 488 is the best known; a cook cycle adds at most about 500) |
| kitchen, R0 | 0 tries end on R0 without an affordance step. 3-20 affordance steps in the run, ≤ 0.5 s for a dialog, 1-5 s for a sentence-idle room |
| kitchen, targets | ≤ 1 target per key per try for var:103 and every refined key. Tries whose last target is a refined key's value ≤ 10% (E011: 45%) |
| kitchen, planner limits | ≤ 2 requests reach the limit (E011: 10) |
| kitchen, delay forks | 0-2 triggered, k ≤ 3 when triggered |
| kitchen, wall | 0.1-1.0 h of try wall in all; empty-window tries ≤ 20% of it (E011: 55%) |
| Part I | not reached within 600 tries. 12-22 of the 31 Part I rooms entered. 25-55 of the 99 key facts on the best verified chain (E009 predicted 8-20 rooms and 15-45 facts without this layer). 8-20 distinct dialogs entered through the affordance step. 10-100 delay forks, the planned outcome found within K in ≥ 90% of them, median k ≤ 4 frames for draws of share ≥ 1/4. If reached: 7,000-20,000 frames from 2728 |
| calls per frame | ≥ 1 in every room the runs enter (global 159), up to about 10 |

## Success threshold

- **Yes.** All of the following:
  - every gate passes;
  - the kitchen is reached and verified from boot (0 differences) within 80 tries, with no manual step;
  - no try ends on R0 or on an empty window without the affordance step;
  - Part I is reached, or a verified chain holds at least 25 of the 99 key facts and enters at least 3 rooms outside
    the k = 2 scope (E009's "partly" for Part I).
- **Partly.** The kitchen is yes and Part I falls short (or does not run).
- **No.** The kitchen is not reached within 80 tries, or an exactness gate fails and is not fixed.

## Stop rule

- **Exactness first.** An entry made by a wait, a delay fork or an affordance trial that differs on verify from boot
  stops every run until it is explained (A0, Lemma C).
- **Gates.** A component whose gate fails is not merged, and its switch stays off. G0 must hold before any run.
- **Order.**
  - The kitchen rerun runs once, under its falsifier and E009's stop rules.
  - Part I runs only after the kitchen's yes and its dated amendment.
  - A mechanism that binds (W or K reached in more than 10% of its uses) is reported with every such case. It is not
    changed during a run.
- **When a stop rule fires,** the write-up gives:
  - the best chain;
  - each mechanism's counts against the predictions;
  - the wall by part and stop kind against E011's;
  - what each try ended on.

  E012 adds no idea beyond its components.
- **Watchdogs.** The harness's, the window search's and the reader's limits are E006's. The guard's 3 GB and the disk
  floors are the machine's. All are engineering watchdogs, reported where they fire.

## Amendment 2026-11-03, component `archive`: T, novelty, marks, cells and the order in the archive, exactly

Written before the component's code. It fills in what sections 5 and 6 leave open; nothing there changes.

- **Switch.** `FMI_SELECT=progress` when the archive is made. stage.json then keeps `"e012": {"select": "progress"}`,
  so a resume keeps the mode. Off, nothing below runs, and rows, cells.json, job.json and log lines are E011's.
- **An atom in the archive.** The archive holds T's atoms without the reading, so a value set's `below` and `above`
  become the domain's bounds. Atom (k, vs, lo, hi) holds at value v (facts.py's: an absent key is 0) when v is one of
  vs's ints, or lo is set and v < lo (vs held `below`), or hi is set and v > hi (vs held `above`). This is
  `atom_holds` through `Domain::label`, exactly. Its text is `k=a|b|<lo|>hi`.
- **The archive's T.** The goal's atoms, then each committed try's T (result.json "tested"), each atom in the order it
  first came. T only grows.
- **Live entries** are every entry but the dead ones. A new entry is dead when its item in result.json's "new" carries
  `"dead": reason` (`is_game_end`, component end). A dead move with no entry goes in result.json's "dead"
  (`{"reason", "frame", "events", ...}`, kept as given). The log keeps both. job.json gives the entry's dead moves
  back to the try.
- **Novelty.** An atom of T, or a room, is novel for a state when no live entry's last facts hold it. A committed try
  records `new_t`: the atoms of T, after its own T is merged, that its new live entries hold and no earlier live entry
  holds. It also records `new_rooms`.
- **Cells:** [room, owned objects, the texts of T's atoms the entry's last facts hold, sorted]. The atoms on
  script-control keys are left out: the slot (`script:`, `local:`, `objscript:`), the position (`pos:`, `escpos:`,
  `at:`, `esc:`), the arguments (`args:`) and the freeze (`frozen:`). E009's idle set is not used here (finding 3).
  When T grows, every row is keyed again from the entries' last facts, and the counts are counted again from the log.
- **Counts per cell** (Go-Explore's):
  - `chosen`: the committed tries from an entry of the cell;
  - `n`: the tries since one from the cell last gave a `new_t` or `new_rooms` atom (0 then).
  - A "return differs" line counts in neither (E008).
- **Exhausted** (result.json "exhausted", decided by the try by section 5's rule): the mark keeps |T| at the commit.
  The entry is skipped while |T| has not grown since.
- **The order.**
  - A cell's candidate is its least live, not exhausted entry in an allowed room, by (−rooms on its chain, −novel
    atoms held, frame, id).
  - Rooms on the chain: the distinct rooms of the entries from the root to it.
  - Novel atoms held: the atoms of T it holds whose least live holder, by (frame, id), is the entry.
  - Cells are taken by (n, chosen), then their candidates' keys. Busy cells are skipped (E008).
- **Stagnation:** E009's rule with `new_t` and `new_rooms` in place of `new_r`. After the last committed try that gave
  one, it fires after as many committed tries as there were selectable cells right after that try, none giving one.
- **Interface:**
  - fmi-explore `progress`: `TAtom { key, vals, lo, hi }` with `TAtom::of(Key, &[Val], Option<&Domain>)`,
    `t_atoms(&[(Key, Vec<Val>)], &Domains)` (for the try: `t_atoms(&Tested::of(&rd).flat(), rd.domains())`),
    `holds`, `text`, `json`, `of_json`.
  - result.json (from the try): "tested", the try's T as `[[key, [ints], lo, hi], ...]`; "dead" on a "new" item; the
    "dead" list.
  - job.json (from the archive, progress only): "novelty" `{"t": [the archive's T], "rooms": [the live entries'
    rooms], "seen": {key: [the values the live entries' last facts hold]}}` and "dead", the entry's dead moves.
  - `Novel::of_job(&job, Option<Vec<TAtom>>)` (the try's T, or the archive's when None). `Novel::novel(&Facts) ->
    Vec<String>` gives the novel atoms' texts, then `room=R`. `Novel::is_novel(&Facts) -> bool`. afford's `Novelty`
    and delay's `novel_of` are these two methods. The try writes the two adapters, since the archive's branch does
    not hold afford's trait.
  - Log line, progress only: "t_new" (the atoms new to T, as JSON), "t_size", "new_t", "new_rooms", "chosen",
    "dead_entries", "dead".
- **S1** (unit tests): the atom's truth against `atom_holds`; cells over T and the control keys; novelty and `new_t`;
  each key of the order alone; the candidate within a cell; dead and exhausted marks, and an exhausted entry back when
  T grows; the counts and the order unchanged when the archive is opened again; stagnation; switch off gives E011's
  files.
- **S2, how.**
  - The E011 kitchen archive is copied (stage.json, entries.jsonl, entries, log.jsonl, cells.json, readings.jsonl).
    The original stays read only.
  - The copy is opened with the switch on. T is the goal's atoms and the union of T over the kitchen's readings that
    still exist or can be made again from their start files.
  - Its log has no `new_t` or `new_rooms`. They are computed from the rows in commit order: the atoms and rooms of a
    try's new entries that no entry of a lower id holds. Then the counts are counted again.
  - Reported:
    - E011's cells against the new ones, and entries per cell;
    - `chosen` and `n` per cell;
    - the order's first 20 entries;
    - where stagnation would have fired;
    - the novel atoms per room;
    - the cost of the recount and of job.json's novelty.
