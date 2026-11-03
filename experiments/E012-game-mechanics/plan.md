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

## Amendment 2026-11-03, component `end`: the pause hook's place, a test trigger, and the interface

Written before patch 0005's code.

- **Measured** (round 2's engine `svm-e011-r2int`, lean, seed 1):
  - The key "space", which the bridge injects like any key, at the kitchen archive's entry 0 (room 33, frame 2728,
    sentence idle) reaches `pauseGame()` through the engine's pause key (`input.cpp:1416`). The run then makes no
    reply within the command's 15 s timeout: the pause dialog waits for a backend event that the bridge never sends.
    The key "x" at the same frame runs 200 frames in 0.03 s.
  - In this game's scripts (statistics only): SO_PAUSE at 0 sites; SO_RESTART at 9 sites, all in the global script
    that `VAR_POST_SAVELOAD_SCRIPT` names (it runs after the original save menu, which these runs never open); SO_QUIT
    at 2 sites, in two global scripts of one room (the research above counted 1).
  - No input the bridge can inject reaches `restart()`: the restart key (F8, `VAR_RESTART_KEY`) is not an injectable
    key, and the original GUI is off.
  - The recorded movies hold the keys "o", "p" and "t" only (the E008 and E011 kitchen archives and B0's).
- **What changes.**
  1. The pause hook is at the start of `pauseGame()`, which SO_PAUSE calls, instead of at SO_PAUSE alone. The engine's
     pause key then ends the run as "pause" too, where today it blocks the run until the command's timeout.
  2. A test trigger, `SPEEDRUN_TEST_RESTART=<frame>`: the bridge calls `restart()` at the start of that frame, where
     the engine's restart key calls it from `processKeyboard()`. The restart hook's tests need it, since no input
     reaches `restart()` here. Unset, nothing changes, and no run sets it.
  3. `Boxes` is a binary command (0x94, reply 0x12) and a JSON one (`{"cmd": "boxes"}`). A second binary command,
     `BoxesJson` (0x95, reply `Json`), returns the JSON command's text, as `StateJson` does for the dump. The gate
     compares the two, and the corners and flags with the room's BOXD block read from the game files.
  4. With `SPEEDRUN_REPORT_RESTART=1` in `fmi`'s environment, every engine it starts gets it (as `FMI_ENGINE_CUTS`
     passes `SPEEDRUN_CUTS`). `FMI_BOXES=1` is the switch the walk-box affordances read; off, nothing sends `Boxes`.
- **Interface,** added to `fmi-core` (E011 Appendix A) byte for byte. After `struct At`:

  ```rust
  /// E012 (patch 0005): the end reasons that are the game's own end (E012 plan.md, method (5)): the scripts' quit
  /// (SO_QUIT; the bridge reports "quit" when the engine shuts down), `restart()` and `pauseGame()` (both reported only
  /// with SPEEDRUN_REPORT_RESTART=1). An End's Status carries one of them in `reason`.
  pub const GAME_ENDS: [&str; 3] = ["quit", "restart", "pause"];

  /// Whether an end reason is the game's own end (`GAME_ENDS`).
  pub fn is_game_end(reason: &str) -> bool {
      GAME_ENDS.contains(&reason)
  }

  /// E012 (patch 0005): one walk box of the current room as the engine holds it (boxes.cpp `getBoxCoordinates`,
  /// `getBoxFlags`): the corners in room coordinates and the flags (0x08 x flip, 0x10 y flip, 0x20 player only or
  /// ignore scale, 0x40 locked, 0x80 invisible).
  #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
  pub struct WalkBox {
      pub ul: (i32, i32),
      pub ur: (i32, i32),
      pub lr: (i32, i32),
      pub ll: (i32, i32),
      pub flags: u8,
  }

  /// E012 (patch 0005): the Boxes reply, the current room and its walk boxes 0..count-1.
  #[derive(Clone, PartialEq, Eq, Debug, Default)]
  pub struct Boxes {
      pub room: i32,
      pub boxes: Vec<WalkBox>,
  }
  ```

  In `trait Engine`, after `at_many`:

  ```rust
      /// E012 (patch 0005): the current room's walk boxes at this stop (the Boxes command reads and changes nothing).
      /// Default: an error, for an engine without the command.
      fn boxes(&mut self) -> EResult<Boxes> {
          Err(EngineError::Bridge("boxes: this engine has no Boxes command".into()))
      }
  ```

## Amendment 2026-11-03, component `tested`: T exactly, and where a draw's range and share come from

Written before the component's code.

- **T, exactly.** For each action, its record's keys (the line's `pre_keys`) that its precondition holds, each with
  the precondition's value set (`Record::pre`). An action is an event when its source is `event` (the Esc events
  too), and an input otherwise (`sentence`, which holds the walks, and `verb`, which holds the dialog lines). An
  event's own unit's keys are dropped: the slot (`local:N`, `script:N`, `objscript:O`), and the unit's `pos`,
  `escpos`, `args`, `frozen`, `at` and `esc` keys. The goal's atoms are added. An atom is a (key, value set) pair;
  the value sets hold the domain's `below` and `above` where the precondition does.
- **Interface** (fmi-reading, `tested.rs`; no fmi-core addition: fmi-explore takes T as plain atoms and keys, which
  the try passes it):
  - `Tested::of(&dyn ReadingSource) -> Tested { atoms: Vec<(Key, Vec<Vec<Val>>)>, inputs: Vec<Key>, events:
    Vec<Key>, goal: Vec<Key>, counts }`, with `flat()` (every atom as `(Key, Vec<Val>)`) and `keys()`;
  - `draws(&dyn ReadingSource, action, Option<&DrawGame>) -> Vec<Draw { check, cite, cond, side, why, site, range:
    Option<(i32, i32)>, share: Option<f64> }>`, one per check whose reason is a random draw.
- **Why the game is a parameter.** A check names the draw but not its constants (`?(random draw: (getRandomNr) +
  ...)`, the reader's `arith_at`), so range and share need the scripts. `DrawGame` is the game loaded once per process.
  Without it, range and share are None.
- **Range and share** (this replaces "by interval arithmetic, as `costs.rs` prices draws", which follows only the
  straight code before the test and would leave the else-if chains on a draw unpriced):
  - The check's cite gives the jump. Each `Local[n]` or `Var[n]` its test reads is followed back to the one assignment
    that reaches it in the unit. A local is followed through yields (no other script writes it). A variable is
    followed only through jumps and assignments to other targets. The search fails at the unit's start, an object's
    verb entry, an Esc point, an op that writes the target in another way, or two reaching assignments.
  - The test must then depend on exactly one `getRandomNr(n)`, whose `n` resolves to a constant the same way.
    `range` is (0, n) (`o5_getRandomNr`, `common/random.cpp:62`).
  - `share` is the fraction of the n + 1 values for which the test takes the check's side, each value run through the
    resolved arithmetic as the reader's machine does it (int32 arithmetic, comparisons at int16). It is exact under
    the uniformity that gate R2 checks.
  - The resolved operand, printed the reader's way (`getRandomNr`, `(x) op ...`), must equal the check's own text.
    Otherwise range and share are None.
  - `share` is per check. Checks of one action on the same draw site carry the same `site`; a caller that wants the
    joint share of such checks combines them.
- **Measured with T1:** how many draw checks get a range and a share, on 28-33-41-j00065 and on the other kept
  readings, and the time of `Tested::of`.

## Amendment 2026-11-03, component `targets`: one target per key in detail, and how T2 is run

Written after the component's first code (branch e012-targets, cf364b4, 67ac2ae and 03d4b83, unit tests only),
before T2 ran. It fills in what section 3 leaves open; nothing in section 3 changes.

- **The group.** For each reached key of a guide kind, the values left after `targets`' own exclusions (the entry's
  value, tier 0's atom, the values tried from this entry, and for an any-value atom the root's value) split in two:
  a value the guide names (its atom (k, v) is in R) stays its own target; the others, whose only atom is (k, any),
  form one target, the key at any of them (`TV::Values`, sorted). A group of one value stays an int target, the same
  request as before. So the targets' (key, value) pairs are E008's, grouped.
- **Tier and depth of a group.** Tier 1 when one of its values is not held by the archive, else 2; the depth is the
  any-value atom's (every value of the group has it).
- **Records.** A group is tried, deferred and counted as `[k, [v, ...]]` in the try's result, job.json and the log;
  the archive keeps these records as they are. A tried list leaves out each of its values from later targets of the
  same entry, so a value reached only later is a target again.
- **Order.** Tier 0 first, also under E009's rotation (section 3's order); then the targets whose key is in T; then
  E008's order or E009's rotation. In the rotation a target's counts are the records of its key that cover one of
  its values (an int record its value, a list each of its values); on int targets alone this is E009's count.
- **T in the try.** `targets_per_key` takes T as a predicate on the key; the try passes `Tested::of(&reading)`'s keys.
  Until the try does, the switch gives the groups with no T tier.
- **Forced targets** (the benchmark's `--targets`) stay single values; a forced value inside a group gets the group's
  tier and depth.
- **T2, how.** The 19 readings the 65 tries used were evicted from the archive. They are made again from the start
  files kept in the archive, with round 1's reader-scope (the kitchen check's) and the arguments the try and the
  harness gave it (fresh readings; the reach and refine readings with their reach-also start files and fact
  variables, followed through the attempts' records). A reading made again must have the log's record and action
  counts. Then, for each try: with the switch off the targets must equal the log's (by tier, their number, the first
  20), and with it on the targets must have the same (key, value) pairs, with at most one value list per key; var:103
  must give at most one target per try. Reported per try: targets off and on, the keys grouped, the groups that hold
  both held and unheld values.

## Amendment 2026-11-03, component afford: its interface, and what section 4 left open

Written before afford's code is committed. Its first runs (M4's form at entries 203, 205 and 4, against M4's probe)
only checked that the enumeration below is M4's.

- **Interface** (fmi-explore `afford`, no fmi-core change; posted in COORD.md):
  - `offer(engine, &Obs) -> Offer` (the idle kind, the verb slots and targets with their points), `inputs_of(&Offer)`,
    and `affordances(engine, &Obs)`, which is the two.
  - `try_all(engine, &Offer, &[Inputs], tested keys, &dyn Novelty, &AffordCfg, then) -> AffordOut { trials, kept }`.
    The archive provides `Novelty`: the atoms of T and the room a state holds that no archive entry holds. `then`
    gives the wait's stops after a trial's idle stop.
  - `ready(engine, horizon)`: where the trigger's frame is not idle, a fork run with no input to the first idle stop.
    No input is given on the way, so the offer's movie is the entry's.
  - Switch `FMI_AFFORD=1`. The try calls the step; afford does not wire itself into the try.
- **What section 4 left open:**
  - **Clicks.** Click k comes at the offer's frame + 1 + 2k (M4's timing). A third click comes at the frame after its
    two-click trial's idle stop.
  - **Hit points** (M4's). A verb slot gets the first of (2, 2), (4, 3), (1, 1), (0, 0) from its rectangle's top
    left. An object gets the first point of a 5 × 5 grid over its on-screen box, in quarters from the centre out. An
    actor gets the first of 20, 30, 10, 40 pixels above its feet, less its elevation. Screen y adds the main
    screen's topline from the status. Every candidate goes in one AtMany. A point counts when the hit test answers
    that slot, object or actor. The hit cache is not used: an offer is one round trip.
  - **Composing.** The two-click trial stops in a sentence idle, and the slot runs (segment on) from the second click's
    frame on hold a run of the input script (VAR_VERB_SCRIPT's value) and none of the sentence script
    (VAR_SENTENCE_SCRIPT's) or of an object's verb script. These are the units a click can start, the same three that
    the bridge's idle rule names as blockers. The third click goes over the targets the hit test finds at that stop.
  - **Horizon.** The wait's horizon once the try passes it, else W = 1,600 frames after the offer.
  - **Outcome**, a measurement only: the changes on the world keys against a no-input fork at the same frame (M4's).
    The world keys are T's keys, the room, every owner and every bit.
  - **Kept.** Novelty is the archive's. Within the step, an atom kept once is not novel again (IW(1)). A trial whose
    run ends records the end's reason; the try marks it dead (end's `is_game_end`).
  - **Walk boxes** come only with end's Boxes (`FMI_BOXES`) and are not in A1-A3.
- **Gates as run:**
  - A1: every kept entry is verified from boot in a fresh lean engine. The engine plays the entry's movie and the
    trial's clicks to the kept frame, and `compare_checkpoint` compares facts, tick, RNG state and actors.
  - A2: M4's form (fixed horizon, no third click), name for name against the probe's `afford.tsv`: at 203 with 400
    frames, at 205 and 4 with 600.
  - A3: the wall per trial of the step in the try's form (idle stops, third clicks) at 203, on round 2's engine
    (`svm-e011-r2int`, lean, cuts).
  - Novelty stand-in in the gate runs: (key, value) pairs of T's keys and the room, over every archive entry's last
    checkpoint. T's keys come from the plan's list for `readings/28-33-41-j00065`.

## Amendment 2026-11-03, component afford: the walk-box clicks (FMI_BOXES)

Written after end pushed Boxes (e012-end 2f6d84a; its fmi-core and fmi-engine commits are cherry-picked unchanged
into e012-afford) and before afford's code for it.

- **Boxes clicked.** The current room's boxes 1 to count - 1, less those flagged invisible (0x80) or locked (0x40).
  These are the boxes in which the engine lets the player's walk end: `adjustXYToBeInBox` (actor.cpp:1990-2040) starts
  at box 1 in a v5 game and skips invisible boxes for the player, and a locked box ends a walk before it
  (actor.cpp:1284).
- **The point.**
  - Candidates: first the box's centre (the mean of its corners), then the points halfway from the centre to each
    corner (ul, ur, lr, ll).
  - On screen: x less xstart, y plus the main screen's topline; inside [0, 320) and the main screen's [b, h).
  - The first candidate counts where the hit test answers the main screen with no object and no actor: a floor
    click. A box with no such point is not clicked.
  - These candidates go in the offer's one AtMany.
- **The inputs.** Only in a sentence idle: one click per such box, named "w<box>", after M4's inputs. With FMI_BOXES
  off, the inputs are M4's.
- **Switch.** FMI_BOXES=1, the variable end's `boxes_on` reads. Off, the Boxes command is never sent.
- **Measured first,** at the probe's 13 entries on end's engine (patches 0001-0005, read only), with the switch off
  and on:
  - the boxes, the walkable ones, and the ones with a floor point;
  - the extra trials, their outcomes, and their kept entries, each verified from boot;
  - their cost.

  With the switch off, A2 must give the probe's trials again.

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

## Amendment 2026-11-03, component `try`: the order of a try in detail, and how G0 and G-on are run

Written before try's code is committed. It fills in what "How a try runs" leaves open; nothing there changes.

- **Where.** fmi-explore's `tryjob.rs` runs the order; a new module `mech.rs` holds the switches, the ports, the try's
  novelty, the commit of the extra entries, the dead moves and the triggers. fmi wires the ports: T from fmi-reading's
  `tested` (fmi-explore does not depend on fmi-reading), and the wait. `ports.rs` gains `PlayExtras` (T's keys and
  the try's novelty for the delay fork) and a provided `PlayerHost::player_with`, whose default is `player`.
- **No switch of its own.** Each step runs under its component's switch (`FMI_WAIT`, `FMI_TARGET_PER_KEY`,
  `FMI_DELAY_FORK`, `FMI_AFFORD`, `FMI_SELECT=progress`; end's are read where they act). With every switch off the try
  computes no T and writes no new field: it is round 2's.
- **The order,** with some switch on:
  1. Return and reading, as round 2.
  2. T is `Tested::of(reading)`, flattened, made once per reading in the process. With `FMI_SELECT=progress`,
     result.json "tested" holds it as the archive's atoms.
  3. **Wait** (`FMI_WAIT`) on the entry's engine, after the reading. It replaces E009's idle measurement. When the job
     asks for that measurement, the wait's first frames feed the idle rule and its record is result.json "idle" as
     before. A stop that holds a novel atom (below) becomes an entry: no events, the stop's checkpoint.
  4. **Targets.** With `FMI_TARGET_PER_KEY`, the T tier is `Tested::has`.
  5. **Attempts** as round 2. With `FMI_DELAY_FORK`, the window search gets T's keys and the try's novelty.
  6. **The goal again.** The tier-0 attempt from the entry may end with status "unsolvable" or "incomplete" (a proof,
     or a finished search, with no plan) or be deferred by deepening. If the wait has stops, the goal is then attempted
     from each stop in frame order, before the next target, once per try.
     - The engine at a stop is a fork of the entry's, run with no input to the stop's frame. It is held to the stop's
       checkpoint; a difference is an error of the try.
     - If such an attempt plays, it is the try's played attempt. Its entries come from its checkpoints, as any
       attempt's. Its target is not recorded twice in "tried", and its probe carries "from_stop".
  7. **The played attempt's** entries, as round 2. Then:
     - the delay fork's candidates in its window records, each kept when novel;
     - a dead move when its run ended in the game's end (stop "engine: engine ended: R" with `is_game_end(R)`): the
       inputs after its last checkpoint.
  8. **Affordance step** (`FMI_AFFORD`). Its triggers are the played attempt's stop being Lemma R's R0 or "the same
     empty window", or no attempt playing (every target dead, deferred or at its limit, or no target).
     - It runs on the attempt's engine at its end, or on the entry's engine. The kept entries continue the attempt's
       inputs and checkpoints up to that frame.
     - The horizon is W, plus, with the wait on, the frames from the offer to the last due frame there.
     - With the wait on, each trial goes on through the wait's stops (afford's `then`).
     - An input whose movie equals one of job.json's dead moves for the entry is not tried.
     - A kept outcome is an entry: the prefix and the clicks, the prefix's checkpoints and the kept one. A trial that
       ends in the game's end is a dead move.
  9. **Commit.** The extra entries follow the attempt's in result.json "new", with "how" "wait", "delay" or "afford"
     and their novel atoms. result.json "dead" lists the dead moves. "e012" is the record: T's counts, the wait, the
     goal again, the delay fork, the affordance steps, the entries kept by kind, the dead moves and the switches. The
     CPU record gains "wait" and "afford".
- **Novelty.** The archive's: job.json's "novelty" with this try's T (`Novel::of_job`). A job without it (made with
  `FMI_SELECT` off, or B0's) takes the entry's own state as the archive. On top, IW(1) within the try: an atom that an
  entry the try keeps holds is not novel again in this try. Entries are counted in this order: the wait's stops, the
  attempt's checkpoints, the delay candidates, the affordance outcomes.
- **Exhausted** (plan.md 5). Round 2's rule, and with any switch on also: no extra entry is kept, the wait (when on) is
  static, and the affordance step (when on) offered no input.
- **Not here.** The due frame as a planned event's window (plan.md 1, last bullet) belongs to fmi-play's window search
  (delay's `window.rs`).
- **Gates as run.**
  - **G0.** B3's 15 tries from round 2's job folders, one resident fmi per sequence, with every E012 switch unset and
    round 2's switches as round 2 ran them: the fast settings (`b3f/r2-fast`, ON2) and the reference settings
    (`b3/r2-ref`). Compared to round 2's outputs as `cmp_tries.py` compares, with `fmi check tries` (Rust): per try
    the attempt's stop, target, frames, replans, inputs and refinements, and every new entry (frame, tick, how,
    events, every checkpoint's facts).
  - **G-on.** The same 15 tries in the fast settings with ON3 and every E012 switch on (`FMI_WAIT=1`,
    `FMI_TARGET_PER_KEY=1`, `FMI_DELAY_FORK=1`, `FMI_AFFORD=1`, `FMI_SELECT=progress`, `SPEEDRUN_REPORT_RESTART=1`,
    `FMI_BOXES=1`), on end's engine (patches 0001-0005). Every new entry is verified from boot in a fresh lean
    engine (fmi-engine's `replay::verify`: the entry's movie, its checkpoints and the try's entry's), with
    `fmi check verify-new`.
  - **Measured,** off against on: per try the wall and CPU by part (return, read, wait, plan, play, afford), and the
    mechanisms' counts (stops and kept, goal-again requests, delay windows and candidates, affordance steps with
    trials and kept, dead moves).

## Amendment 2026-11-03, component `delay`: the fork's rules in the ready search, its bound, its record and the interface

Written before the delay fork's code.

- **Measured** (statistics of readings at hand, not inputs of the system):
  - Draw checks are on events only. Test (a)'s reading 28-33-41-ta: 8,201 of its 40,773 actions, events of room 28's
    locals 208-210. Ring(38)'s 33-38-83-85-96 (B3's j00001): 165 of 9,286, events of room 85's local 202. Ring(33)'s
    28-33-35-38 (B3's j00000): 6,471 of 234,401, events of rooms 28 and 35 and of script 47. No sentence, walk or verb
    click checks a draw there.
  - A draw check's reason names the draw but not its range (`?(random draw: ((getRandomNr) * ...) + ...)`): the range
    and the share need the script, which is `tested`'s `draws`.
  - No window of B3's 15 tries (b-lama-strict's records) has a member that checks a draw.
- **Where it acts.** Only in E010's play when ready (`ready`), in a window with an input. The strict search without
  ready already forks every frame and keeps the least Q.
- **Trigger.** A record of the window's members (the input's own record and the records after it up to the window's
  end, any of each one's paths) has a check whose reason is "random draw". The window's draws are listed in its record.
- **p and K.**
  - Per draw: its share; else 1/(hi − lo + 1) from its range; else unknown.
  - Per path, the product over its draws (independent draws); per record, the largest over its paths (a path with no
    draw gives 1); per window, the product over its records.
  - K = ceil(ln 0.01 / ln(1 − p)), with a 10⁻⁹ tolerance on the ratio (p = 0.9 gives 2), and K = 1 for p = 1.
  - When some draw has neither share nor range, K is not computed: the record says `"k_max": null`, the fork keeps
    its rules below, and the ready search's own bound (294 failed tries) is the only one.
- **The tries.** k = the try's first input frame minus that of the window's first try that issued an input. Tries
  that issued no input, duplicates and cancelled tries get no k.
  - A try that ends in success or the goal from the plan's records **matches the plan's branch**: the first in fork
    order is chosen.
  - A deviation is not chosen while k < K: it is kept as an outcome and the next try is forked at the frame E010 gives
    a failed try.
  - A try with k ≥ K (forked in parallel before the bound was known) is not counted.
- **Refuted.** When every try left would have k ≥ K and none matched:
  - the first deviation among the tries is chosen, which is E010's choice (the first end point), and the harness
    replans from the observed state;
  - with no deviation, the window ends at once ("delay fork: the plan's branch refuted ..."), an empty window: the
    harness replans with the window's cause.
- **Record.** The window record's `"delay"` (only with `FMI_DELAY_FORK=1`):
  - every window: `"rng"`, the random calls per frame over the parent's frames, counted exactly by stepping xorshift32
    from one Status `rng` to the next (frames, calls, least, most, histogram);
  - a triggered window: `"draws"` (record, check, cite, side, range, share), `"p"`, `"k_max"`, `"first"`, `"chosen"`
    (k or null), `"outcomes"` ([k, digest, novel, outcome] per try with a k), `"refuted"`, `"candidates"`.
- **Outcome digest and candidates.**
  - A try's end state: its last checkpoint when it is an end point; otherwise, when it issued its input, a full dump
    taken where it stopped (one more observation per try of a triggered window).
  - The digest is a hash of the end state's facts on the tested keys (`Search.tested_keys`, given by the try; unset:
    the window's own keys, those of its tracked records, the goal and the room).
  - A candidate is a try other than the chosen one, with an end state, whose digest differs from the chosen try's and
    from every earlier candidate's, and whose novelty is not false (`Search.novel_of`, given by the try: archive
    novelty over T and rooms; unset: null). It carries its inputs (the attempt's movie up to the window, then its own
    events, absolute frames) and its end checkpoint. The try commits it as an entry (A0, Lemma C, verified from boot
    like any entry).
- **Draws from `tested`.** `Search.draws_of`, given by the try or the integration from `tested`'s `draws`; unset: the
  checks whose reason is "random draw", with no range and no share.
- **Per try** (`Search.summary()`'s `"delay"`): windows triggered, tries with a k, chosen k's, refutations,
  candidates, and the calls per frame over every window's parent frames.
- **Gates, as run.**
  - R2: from state 3182679328, the outputs `(s · 0xDEADBF03) mod n` over 100,000 successive calls, for n = 1..233
    (every range 0..max of M6 has n = max + 1 ≤ 233), at call strides 1-10 (M1's calls per frame). χ² per n at 1%,
    and the family of 232 non-trivial n per stride at 1% (Bonferroni). The gate is the family test.
  - R3: the fixture of forced branches is the window search's own resolution of tries (fork order, parallel tries,
    duplicates, tries without an input, deviations, tries past K) with each try's outcome set by a table; over
    every table up to 6 tries and 2,000 random longer ones, the record's chosen k equals the least k < K whose try
    matches, or null and refuted. Also on the engine, below.
  - R1: test (a)'s three plans played on round 2's lean engine in ready mode with the fork on every input window
    (`Search.delay_force_p`, a test setting no run sets, p = 1/2, 1/4 and 1/21). Every candidate and the attempt's
    checkpoints verify from boot with 0 differences, and each record's chosen k is the least k with a matching try.
  - G0 for this switch: with `FMI_DELAY_FORK` unset, test (a)'s play fixtures (strict and ready) and B3's 15 tries
    in the fast settings give round 2's records; with it set, B3's tries give the same records but the new
    `"delay"` field.
- **Interface,** added to `fmi-core` (E011 Appendix A) byte for byte, at the end of the play section:

  ```rust
  /// A random draw one of a record's checks reads (E012 `tested`, `draws(&Reading, action)`): the check's index in the
  /// record's checks (the reading line's "checks"), the draw's range of values when interval arithmetic over the
  /// script gives it, and the share of that range on the record's side of the check (the planned outcome's p).
  #[derive(Clone, PartialEq, Debug)]
  pub struct Draw {
      pub check: usize,
      pub range: Option<(i32, i32)>,
      pub share: Option<f64>,
  }
  ```
- **Prediction.** R1-R3 pass. With the switch on, no window of B3's tries is triggered and the calls per frame are 1
  in rooms 33 and 38 and 2-6 in room 28 (M1). On test (a) with the fork forced, k = 0 is chosen in windows 1, 2 and 4
  (their first try succeeds); window 3 (293 failed tries before the chosen one in the strict search) is refuted for
  every p tried and its harness replans.

## Amendment 2026-11-03, the `wait` component (branch e012-wait): its interface, before its code

- **Where the plan's words are made exact** (ScummVM's `scumm.cpp` 2821 and 3110-3134, `script.cpp` 1535):
  1. **Δ.** The engine's frame delta is var 19 (VAR_TIMER_NEXT) at the stop, at least 1. A paused delay goes down by
     at most 15 in a frame. Only a slot whose status is exactly ssPaused (1) is lowered; a frozen one has 0x80 set.
  2. **Due frame.** In SCUMM v5 the woken slot runs before the frame's decision point (`runAllScripts` comes before
     `checkExecVerbs`). So a slot paused with delay D at the stop of frame f runs in frame f + floor(D / Δ) + 1, and
     its effect is in that frame's state.
  3. **H re-read at each stop.** The stop's own due frames replace the earlier ones: a slot that is no longer paused
     has no due frame. The wait ends at max(last stop, last due frame) + W.
  4. **Truth.** An atom (key, values) holds when the harness's view of the key is in its values: `start::view`
     (replay.view), pos: and escpos: from the at: and esc: facts, every value labelled by its domain. Unobserved keys
     (args:, saved:, actorroom:) are never stops.
- **What the wait gives the try.**
  - Each stop carries its checkpoint (facts, tick, rng, actors, as the harness makes them), so the try can make it an
    entry. The wait itself makes and keeps no entry: novelty is the try's and the archive's.
  - With `idle_frames` > 0, E009's idle rule runs on the wait's first `idle_frames` frames. Those are the idle
    measurement's frames and changes, so its record is the idle measurement's.
- **Engineering watchdog.** A wait's frames have a watchdog (`FMI_WAIT_MAX_FRAMES`). Where it fires, the wait's end is
  `Watchdog`, and it is reported.
- **Split with the other components.**
  - `due` goes into fmi-core, so that fmi-play's window search (`delay`'s window.rs) can give a paused slot's planned
    event its due frame. This branch does not change the window search.
  - The wait's place in the try (after the reading, in place of the idle measurement) is the `try` component's
    (tryjob.rs).
  - This branch adds `fmi_explore::wait` and `fmi check wait`, which runs W1-W4.
  - Until `tested` lands, the check computes T as section 1 defines it, from the reading's records: the union of the
    preconditions' atoms, less each event's atoms on its own unit, plus the goal's. At integration `Tested::of`
    replaces it.
- **The gates' inputs.**
  - Entries 203, 205 and 225 of the kitchen archive (room 28): T from the kept reading 28-33-41-j00065.
  - Entry 4 (room 38): T from a fresh reading of its job's scope (33, 38, 83, 85, 96) from its state, since that
    reading is evicted.
  - The entries are rebuilt from boot from the read-only archive, as the plan's measurements were.

### Appendix A additions: `fmi-core`, the wait

Added at the end of fmi-core's `lib.rs` (the constants and functions) and to the `Engine` trait after `step_changes`
(the provided method).

```rust
// --- E012: time passing (plan.md section 1, the wait component) ---------------------------------------------------

/// A slot's status while it waits out a delay (script.h ssPaused); a frozen slot has 0x80 set on top of it.
pub const SS_PAUSED: i32 = 1;
/// VAR_TIMER_NEXT (var 19 in SCUMM v5): the ticks the engine's next frame lasts.
pub const VAR_TIMER_NEXT: usize = 19;
/// The most a paused slot's delay goes down in one frame (scumm.cpp scummLoop: `if (delta > 15) delta = 15;`).
pub const DELAY_STEP_MAX: i64 = 15;

/// The ticks the next frame takes off every paused slot's delay: the stop's var 19 (the main loop's delta is
/// VAR_TIMER_NEXT, at least 1), at most DELAY_STEP_MAX (scummLoop's cap before decreaseScriptDelay). None when the
/// dump has no var 19.
pub fn delay_step(o: &Obs) -> Option<i64> {
    let v = *o.vars.get(VAR_TIMER_NEXT)? as i64;
    Some(v.clamp(1, DELAY_STEP_MAX))
}

/// The frame in which the dump's slot `slot` runs again, by the engine's own rule (script.cpp decreaseScriptDelay):
/// each frame lowers the delay of every slot whose status is exactly SS_PAUSED (not a frozen one) by the frame's
/// delta, and the slot runs in the frame its delay goes below 0, before that frame's decision point (v5:
/// runAllScripts before checkExecVerbs). So the stop's frame + floor(delay / delta) + 1. None for a slot not in the
/// dump or not paused. Exact while var 19 keeps its value.
pub fn due(o: &Obs, slot: i32) -> Option<u64> {
    let s = o.slots.iter().find(|s| s.slot == slot)?;
    if s.status != SS_PAUSED {
        return None;
    }
    let d = delay_step(o)?;
    Some(o.frame + (i64::from(s.delay.max(0)) / d) as u64 + 1)
}
```

```rust
    /// The wait move's run (E012, plan.md section 1): frames with no new input from this stop, one at a time. After
    /// each, `each` gets its status and the facts' changes from the frame before (as `step_changes(1)` gives them)
    /// and returns true to stop there. At most `frames` frames. Returns the last status and the frames run. Default:
    /// one `step_changes(1)` per frame; a provider may run the same frames in fewer round trips.
    fn wait(&mut self, frames: u64, each: &mut dyn FnMut(&Status, &[(Key, Val, Val)]) -> bool) -> EResult<(Status, u64)> {
        let mut st = self.status().clone();
        let mut n = 0;
        while n < frames {
            let (s, ch) = self.step_changes(1)?;
            n += 1;
            st = s;
            if each(&st, &ch) {
                break;
            }
        }
        Ok((st, n))
    }
```

## Amendment 2026-11-03 (2), component `delay`: no K, no change of choice

Written after the first gate runs, before the code it changes.

- **Why.** `tested`'s T1 resolves none of the draw sites of the kitchen reading j00065 (0 of 6: room 28's animation
  loops) and 27 of 37 on the k = 2 scope. In a window whose draws have no share and no range, the first amendment let
  the fork skip deviations up to the ready search's bound: 294 failed tries or the window's 3,600 frames, the cost of
  the empty windows that the plan sets out to cut (55% of the kitchen check's wall). In the kitchen every triggered
  window would be of that kind.
- **What changes.** When K is not computed, the fork changes no choice: the first end point in the order of tries is
  chosen, as E010 chooses it. The record keeps the draws, `"k_max": null`, and every try's k and outcome up to the
  choice; `"chosen"` is the k of a matching try, null when the choice is a deviation. With K computed, the first
  amendment's rules hold unchanged.
- **Gate R3** gains the case: with no K, the chosen try is the first end point in fork order.

## Amendment 2026-11-03, component `resident`: the reader in fmi's process, the job memo resident, the store's key over the scope's units as a set

Written before the component's code. This is E011's round 3 item for the reading (E011's plan.md, "Results, round
2", "What binds": the reader inside the process with the memo held in memory), built here as an E012 component.

- **Why (measured).**
  - E011 round 2: a fresh ring(33) reading takes 36.4-40.8 s of wall and 108-136 CPU s with round 2's reader (35.0-36.3
    s with round 1's), the slowest part of a try; the reading is 74% of round 2's fast warm sum. Round 2's
    `reader-scope --prep` cut 10-15% of the reader's CPU, not its wall. fmi still runs reader-scope as a child process:
    the reader as a library call and the memo's store in memory (e011-r2-read) were not merged.
  - The job memo across readings (E011 memo.md) takes a stored run only between readings with the same whole-reading
    key (memo.md, item 3). The kitchen check's store (`/var/tmp/fast-mi/e011-archives/run/memo`, 14 folders) holds
    two pairs of folders of the same scope and refinement whose key texts differ only in the line `units`: ring(33)
    (28, 33, 35, 38) unrefined, and (28, 33, 41) unrefined. Each pair's units are the same set (140 and 110 units) in
    another order. The order comes from the walk that adds the global scripts (`Scope::of_with`: the start's running
    scripts and hook variables first). So a reading of the same scope from another entry took no run from the first.
- **What.** One switch, `FMI_READER_LIB=1`, off by default. With it:
  1. **The reading runs in fmi's process** (`fmi_reading::inproc`): `scumm_reader::run::run` with the arguments fmi
     gives reader-scope (`ReadSpec::args` and the extras below), the game loaded once per process (`run::Resident`),
     the progress in the folder's `reader-scope.err`, the try's cancel passed to the reading's stop flag. A reading
     that stops or fails is an error of the read, as a reader-scope that exits non-zero is.
  2. **The allocator** is mimalloc, reader-scope's own, chosen at the process's first allocation from the
     environment; with the switch off, the system allocator, as today.
  3. **The job memo resident**: the store in memory (memo.rs `MemStore`) kept from reading to reading, at most
     `FMI_READER_MEMO_GB` (default 1.5; 0: no memo), its oldest files going to `FMI_MEMO`'s folder when that is set,
     else dropped. reader-scope gains `--memo-mem`: the memo with the resident store alone.
  4. **`--prep` on** (the workers work out the reading thread's and the sink's part of their records), unless
     `FMI_READER_PREP=0`.
  5. **The store's key over the units as a set**: reader-scope gains `--memo-units-set`, which writes the key's
     `units` line as the scope's unit keys sorted. The switch passes it. The argument is memo.md's amendment of this
     date, written before this code.
  - `fmi read FILE`: readings one after another through fmi's readings port (reader-scope as a child, or the process
    itself with the switch), one per line of reader-scope's arguments, each line's wall, CPU and memo counts printed.
    It is the gate's and the measure's driver.
  - With the switch off, fmi runs reader-scope as a child exactly as before, with the system allocator and no added
    argument; reader-scope without the two new flags writes the same files and keys as before.
- **Gates.**
  - **RS1, byte for byte.** G6's recorded inputs read in one fmi process with the switch on, in this order: ring(33)
    j00089 (stores), ring(33) j00119 (takes), the refinement j00089 [103, 194] (stores), the same refinement again
    (takes), ring(38) j00091. Each reading's `reading.tc`, `.reached`, `.actions.jsonl`, `.lists.jsonl` and `.idx`
    have the sha1 of reader-scope's reading of the same arguments (this branch's build, a child process, no memo), and
    G6's recorded sha1 where G6 has one; `reading.json` is the same but for its measured fields (`read_s`, and
    `reader_scope`'s `read_wall_s`, `read_cpu_s` and `peak_rss_mb`).
  - **RS2, the kitchen's pair.** Ring(33) from entry 0 (j00000's start row) and then from a second entry, in one
    process with the switch on: the second takes runs; its files equal reader-scope's for the same arguments. The
    second entry is the first of the check's other fresh ring(33) starts (entry 191, j00057; entry 189, j00059) whose
    key text's units order differs from entry 0's (read from the store's `inputs.txt`); if neither differs, entry 191,
    and RS2's units part is reported as not exercised. The same pair without `--memo-units-set` is reported (how many
    runs the order kept from being taken).
  - **RS3, the memo's argument**: memo.md's amendment; cargo tests of memo.rs, and a test that the key is the same for
    the units in another order and differs for another set.
  - **G0**: with the switch off no code path of a try changes (by construction above); the cargo tests of fmi,
    fmi-reading and scumm-reader pass.
- **Measure.** A fresh ring(33) reading from a second entry (RS2's, after entry 0's), wall and CPU, each
  under the heavy lock and the guard at nice 10, 3 threads, before and after interleaved:
  - before: round 2's way, B3's settings (reader-scope as a child, `--prep`, no memo);
  - before, with round 1's disk memo (`FMI_MEMO`, reader-scope as a child): what the key's units order leaves;
  - after: the switch on;
  - the floor: the same start read again in the process, every run taken (round 1 measured that reading at 12-16 s
    with the store on disk);
  - per reading: wall, CPU (fmi's own in the process; the child's otherwise), runs taken and run, peak memory, and
    the reading's parts (the pool's job seconds, the reading thread's and the sink's CPU).
- **Prediction.** RS1-RS3 and G0 pass. Before: 35-42 s of wall and 105-140 CPU s; with the disk memo, as before (the
  key differs: nothing taken). After: at least 80% of the second reading's runs taken, 5-15 s of wall and 15-45 CPU s.
  The floor: 4-12 s. fmi's peak memory during the readings: at most 3 GB.
- Any further change to the taken path (for example outputs kept decoded in memory) waits for a dated amendment with
  these measurements.

### Amendment 2026-11-03 (resident, after RS1 and the keys, before the measure): the second entry

- **Measured.** RS1 passes (`results/resident/README.md`). The keys step read ring(33) from entries 0, 191 and 189
  in one process with the units in their order: entry 191's key differs from entry 0's in the `units` line alone, and
  entry 189's key is entry 0's. But entry 191's reading (j00057's start, script 2 running) has 0 records, as the
  check's own reading of it had (log.jsonl: 0 records, 0.6 s): its start reaches no job, so it measures nothing.
  Entry 189's reading took 39,115 of its 39,515 runs from entry 0's.
- **What changes.**
  - The measure's second entry is entry 189 (j00059): a fresh ring(33) reading from another entry with 235,568
    records, whose key is entry 0's without the units as a set.
  - RS2's units part runs on the check's other scope with readings of both orders, (28, 33, 41): j00061's start
    (entry 205, dialog, script 159) and then j00062's (entry 204, scripts 159 and 23), both 40,765 records in the
    check. The keys step is repeated on them (the units in their order); if their keys differ in the `units` line
    alone, the pair is read again with the switch on (the second takes runs) and by reader-scope (the files the same).
    The pair is also measured before and after as the measure's second case.
  - Nothing else changes.

### Amendment 2026-11-03 (resident, after B3 with the switch on, before the code it changes): the allocator

- **Measured.** `resident/b3.sh on`: B3's 15 tries in the fast settings with FMI_READER_LIB=1 in one resident fmi,
  each try under the heavy lock. All 15 equal B0's in everything (cmp_tries.py). The median try is 2.8 s (round 2's
  B3: 4.9 s), the fresh ring(38) readings 1.2-3.0 s (4.4-4.8 s), j00006's refinement from another entry 8.2 s (20.6
  s), the reading's sum 124 s (142 s). But the planning of the ring(33) tries on the refined reading takes 10.3-13.3
  s of wall and 8.6-11.2 CPU s (j00008, j00009, j00011, j00014), where round 2's fmi on the laptop at the same time
  (E011's check, its r2-fast and d-fast sequences, 10:36) took 5.5-7.2 s and 4.4-6.0 CPU s; the first try's planning
  22.4 s against 14.8 s. The switch changed the planner's process in one way besides the reading: the allocator
  (mimalloc for the whole process).
- **What changes.** `FMI_READER_ALLOC=system` keeps the system allocator with the switch on (default: mimalloc, as
  above). Measured with it: B3 with the switch on, and the kitchen pair (`after`). The switch's allocator becomes the
  system's if, with it, the ring(33) tries' planning is back within 20% of round 2's concurrent run and the kitchen
  pair's second reading is within 10% of mimalloc's; otherwise the one with the lower B3 sum. Nothing else changes.

### Amendment 2026-11-03 (resident, after B3 with the system allocator, before the code it changes): the allocator by phase

- **Measured.** `b3.sh on-system`: all 15 tries equal B0's. The ring(33) tries' planning is back at 5.4-6.0 s of
  wall (4.6-5.2 CPU s), round 2's concurrent run's 5.5-7.2 s; the first try's 14.5 s (round 2: 14.8 s). But the
  reading is slower with the system allocator: the first try's reading 162.3 s against 108.3 s, the kitchen pair's
  second reading 23.1-23.9 s against 15.7 s (`after-system`, interleaved with `after 3`), its first 55.7-61.3 s
  against 38.1 s; fmi's resident set 4.4 GB against 2.4 GB. By the previous amendment's rule the switch keeps
  mimalloc (B3's sum 212.2 s against 234.3 s), and the planner stays twice as slow in it.
- **What changes.** The allocator by phase, the switch's default (`FMI_READER_ALLOC=phase`; `mimalloc` and `system`
  stay as measured): while a reading runs in the process (`InProc::read`), new blocks come from mimalloc, the
  reader's own; at any other time from the system's, the planner's and the window search's own. A block is freed,
  grown or shrunk by the allocator that made it: mimalloc's own test of its address ranges (`mi_is_in_heap_region`,
  its page map) tells its blocks from the system's.
- **Why it is sound.** No computation reads where its memory comes from; the outputs are the same with either
  allocator (RS1, RS2 and B3, with mimalloc and with the system's). What remains is that every block goes back to its
  maker: mimalloc's page map holds exactly the slices of the regions mimalloc reserved, and the system's blocks lie
  in regions the system mapped, which no reservation of mimalloc's shares; before mimalloc's first block the map is
  empty and answers no.
- **Measured with it:** B3 with the switch on and the kitchen pair (`after`), as above; a unit test of the routing.

## Amendment 2026-11-03, the integration: what is merged, one fix, and how the gates and the measurement run

Written after the merges and before any gate or measurement on the merged build.

- **Merged into e012** (worktree `/var/tmp/fast-mi/wt-e012`):
  - origin/e011 928f437, E011's merged round 2 (the no-plan proofs) results: documents and bench scripts only;
  - e012-try 58cfaef, which holds end d9c7f8d, tested 784095d, targets c6ee4bf, afford 71a5559, archive c02a4f3,
    delay 96c46cf and wait e1560ab unchanged, and the try's order;
  - e012-resident 9395b0d.

  Each component's own files are byte for byte its final head's. The conflicts were plan.md (every component's
  amendment is kept: the others' in try's order, then resident's) and one doc line in fmi-reading's lib.rs.
- **One fix, in E011's code.** `HostVerifier::replay_verify` (`fmi explore verify`, and the driver's verify of a goal
  entry, which the kitchen rerun's falsifier uses) boots to the first checkpoint's frame, then asks the bridge to run
  to that same frame, which it refuses. The boot also keeps only the events up to its frame, so the later events were
  never scheduled. afford and wait measured both (COORD.md). It now does what `waitcheck::verify_from_boot` does: the
  events after the boot's frame are scheduled at that stop, and the first checkpoint is compared where the boot
  stopped. It is checked against E008's `explore.py verify` on E009's door archive (entry 5, `verify-5.json`).
- **Not built by any component:** the due-frame window in E010's window search (section 1, last bullet; "How a try
  runs", step 5). delay and wait each left it out, and `fmi_core::due` is there for it. This integration adds no
  mechanism ("E012 adds no idea beyond its components"), so the merged build runs the window search without it.
- **The build.** `integrate/build.sh` into `/var/tmp/fast-mi/target-e012`: the rs/ workspace, fmi-checks and the
  scumm workspace's readers, release, debug info off. taskcore has not changed since round 2 and its binaries are
  used as they are. The engine is end's build `svm-e012-end` (patches 0001-0005, scummvm sha1 cc6615ea).
- **"Every E012 switch off"** is round 2's switches with the no-plan proofs (E011's ON3) and these unset:
  `FMI_WAIT`, `FMI_TARGET_PER_KEY`, `FMI_DELAY_FORK`, `FMI_AFFORD`, `FMI_SELECT`, `SPEEDRUN_REPORT_RESTART`,
  `FMI_BOXES`, `FMI_READER_LIB`. **"Every one on"** is ON3 with `FMI_WAIT=1 FMI_TARGET_PER_KEY=1 FMI_DELAY_FORK=1
  FMI_AFFORD=1 FMI_SELECT=progress SPEEDRUN_REPORT_RESTART=1 FMI_BOXES=1 FMI_READER_LIB=1` (`integrate/env.sh`).
- **G0, G-on and the measurement** come from one run of B3's 15 tries in the fast settings (`integrate/b3.sh`;
  E011's b3r2.sh with E012's sequences). Three resident processes alternate try by try, and the heavy lock is held
  across the whole run:
  - d: round 2 with the no-plan proofs (its fmi and reader-scope, round 2's engine), ON3;
  - eoff: e012 with every E012 switch off, end's engine;
  - e: e012 with every E012 switch on, end's engine.

  The job folders are round 2's own B3 folders' job.json with the paths changed, as try's gates.sh makes them. The
  reading cache is on as in B3 (B), with its floor at 5 GB; the job memo is off, as in round 2's B3.
  - **G0:** eoff's 15 tries equal d's and round 2's published `r2-fast` in everything (`fmi check tries`). Also,
    the reference settings: eoff alone against round 2's `r2-ref`.
  - **G-on:** every new entry of e's tries is verified from boot in a fresh lean engine (`fmi check verify-new`).
  - **G11's second part:** eoff's new entries are verified from boot the same way.
  - **G12:** the watcher, rooted at the three processes.
  - **The measurement:** e against d, per try, by wall and by part (`fmi check try-report`: return, read, idle or
    wait, plan, play, afford).
- **E011's gates** on the merged build:
  - G1-G5 (engine/check-r2.sh) on end's engine with `SPEEDRUN_REPORT_RESTART` off and on;
  - G6 on e012's reader-scope (the kitchen golden; --prep as fmi runs it, the memo off and both memo pairs), then
    resident's RS1;
  - G7 off and on, G8;
  - G9 with everything off and with everything on;
  - G10: play's fixture tests; `fmi try` on test (a) with the Rust lama-first-costs (strict, ready, fast) with every
    E012 switch off and on; Fast Downward's binary (strict, ready) with everything off;
  - G11 and G12 as above.
- **The components' gates with their switches on**, on the merged build:
  - the cargo tests: S1, T1, R2-R3, the units of afford, targets and wait, end's engine tests, resident's memo tests;
  - end's hooks and boxes; wait's W1-W4; delay's R1 and R3 on the engine; targets' T2; afford's A1-A3 and the walk
    boxes; resident's RS1.

  end's `unreach` (1,122 entries) and archive's S2 (reported, not a gate) are not run again: their code, the
  engine and the inputs are the same bytes as on their branches.
- **Prediction.**
  - Every gate passes with no fix round beyond the one above.
  - G0: 15 of 15 in both settings.
  - G-on: 76 new entries, as try counted them, all verified with 0 differences.
  - The 15 tries' wall sum, with everything on, is 0.80-0.95 of d's in the same run. The reading in process saves
    15-25 s (resident's B3: 173.1 s against round 2's 193.1 s). The E012 steps add 8-15 s: T 3.7 s, the waits 1.1 s,
    the affordance steps 5.1 s in try's measurement.

## Results of the integration (2026-11-03, written after the runs)

Branch e012: origin/e011 (928f437), e012-try (58cfaef: end, tested, targets, afford, archive, delay and wait at their
final heads, and the try) and e012-resident (9395b0d) merged, then the fix of `HostVerifier::replay_verify` and a
report command (`fmi check try-parts`). The merge needed no code change between components: the conflicts were plan.md
and one doc line. Built into `/var/tmp/fast-mi/target-e012`; the engine is end's `svm-e012-end`. Numbers and tables:
`results/integration/README.md`. Laptop, seed 1, nice 10, heavy steps under the heavy lock and the guard; no other
experiment ran on the laptop.

**The fix.** On E009's door archive, entry 5, round 2's fmi verify reports a difference at the first checkpoint and the
goal not held. The fixed one gives E008's `explore.py verify` record: 6 checkpoints, 0 differences, 28 inputs,
state:316 = 1, the same frames, qf and tick.

**Gates.**

- **G0 holds.** With every E012 switch off, B3's 15 tries equal round 2's in everything: in the fast settings, against
  round 2 run beside them, round 2's published `r2-fast` and B0 (15 of 15 each); in the reference settings, against
  `r2-ref` and B0 (15 of 15 each).
- **G-on holds.** With every E012 switch on (`FMI_READER_LIB` included), every attempt of the 15 tries is the same as
  with them off. The layer adds 33 entries: wait 18, afford 15, delay 0. All 76 new entries verify from boot with 0
  differences.
- **E011's gates on the merged build:**
  - G11: every new entry of the off sequences verifies from boot (43 of 43, fast and reference);
  - G12: no python process in an fmi tree;
  - G2-G4 with `SPEEDRUN_REPORT_RESTART` off and on: 0 differences;
  - G5: the line `CHECK (a): 0 goal True 2728 3216 488 11712 2 36`, 8 entries, verify 0 differences, off and on, but
    with 503 children (below);
  - G6: the kitchen golden; ring(38), ring(33) and the refinement with --prep, the memo off and both memo pairs; RS1;
  - G7 off and on, G8;
  - G9 with the switches as round 2 ran it, and the deepening's check;
  - G10: play's fixtures; `fmi try` on test (a) with every E012 switch off and on, round 2's line and every entry
    verified;
  - the cargo tests: 141 in rs/, fmi-play's 17 and R2's table, fmi-engine's 16 with the engine tests, scumm-reader's
    memo tests.
  - G1 (fork_exact.py, 7 load children, the switch off): 6,002 comparisons and 594,383 load comparisons, 0 differ;
    P's trace = F's. With the switch on it is end's run on the same engine binary.
- **The components' gates, on the merged build:**
  - end's hooks (14 cases) and Boxes (6 rooms);
  - T1;
  - W1 (50 of 50 stops), W2 (the door at due(local 211) in 13 of 13 room-28 entries), W3, W4 (140 waits, median 0.109
    s, max 0.210 s);
  - R1 (31 runs), R2, R3;
  - T2 (65 of 65, the component's table);
  - A1 (8 of 8), A2, A3 (6.5-7.9 ms of wall per trial), the walk boxes;
  - S1;
  - RS1.

  end's `unreach` and archive's S2 were not run again: the same engine binary, code and inputs.

**Two gates read differently than E011 recorded them.**

- **G5's children.** The Python reference's strict window search runs children in a thread pool and stops when Q(g)
  + b reaches the best result so far, so how many children are in flight at the stop depends on timing. On end's
  engine window 1 issued two more children than round 2's runs: frames 2798 and 2799, deviations after the best. Every
  child present in both runs has the same outcome, frames and qf; each window's best, the route and the line are the
  same. E011 recorded "501 or 502"; these runs gave 503. A third run on end's engine, with the laptop quieter, gave
  502.
- **G9 with the layer on** is not a reference gate. `FMI_SELECT=progress` changes the cells by design. The recorded
  tries' replay through doubles has no reading to compute T from, and the try refuses to run half wired. Stagnation
  over T stops the fake-step driver before its kill-and-resume scenario's kill. With ON3 the deepening changes job.json,
  as round 2 found; its G9 "on" was ON2 with the proofs.

**The 15 tries with everything on, against round 2** (two resident processes alternating try by try, the heavy lock
held across, the reading cache on and the job memo off as in round 2's B3; wall seconds). The amendment took the
measurement from the three-process run that gave G0 and G-on. That run is reported below; this two-process run was
added after it, because there the planner's keep rule, not the code, set the merged build's planning.

| | round 2 | E012, everything on |
|---|---:|---:|
| sum, 15 tries | 191.8 | 183.8 |
| median, 15 | 4.4 | 3.2 |
| median, ring(33), 7 | 6.1 | 7.0 |
| median, ring(38), 8 | 1.35 | 0.95 |
| read | 140.3 | 117.1 |
| T | | 4.1 |
| idle measurement / wait | 1.20 | 1.14 |
| plan | 48.5 | 54.4 |
| play | 1.42 | 1.54 |
| affordance steps | | 4.97 |
| return and rest | 0.45 | 0.55 |

- **The reading** in fmi's process saves 23.2 s. The four fresh ring(38) readings take 0.5-3.1 s instead of 1.9-4.7 s,
  and j00006's refinement from another entry takes 7.5 s instead of 19.7 s.
- **The layer's own steps** add 9.1 s. T is 1.8-2.1 s per ring(33) reading. The affordance step is 0.26-0.72 s per
  try. The wait costs what the idle measurement it replaces did.
- **The planning** is 5.9 s longer for the same requests and plans: 2.8 s in j00000, 1.0 s in j00006, 0.2-0.6 s in
  each later ring(33) try. Not attributed.
- **With three resident processes** (the run that gave G0 and G-on), the merged build planned j00011 in 11.2 s and
  j00014 in 9.6 s, against 5.7 s and 6.7 s with the switches off. j00012 took 2.5 s instead of 0.07 s.
  - The harness keeps a reading's planner across a refinement only while MemAvailable is at least E008_MEM_SECOND_GB
    (7 GB), and that is the only path that closes one.
  - MemAvailable was 7.1-8.5 GB at the ends of the ring(33) tries, and the merged process was up to 0.4 GB larger
    than the one with the switches off (0.8 GB larger than round 2's in the two-process run): the resident store and
    T.
  - With two processes (8.9-11.9 GB), j00012 planned in 0.01 s.
- **What the mechanisms did.**
  - All 15 attempts stop on Lemma R's R0 in a dialog, so the affordance step ran 15 times: 4 lines each, 1 kept.
  - The waits were static at 6 entries and stopped twice at the other 9.
  - No delay fork triggered: no window checks a draw in rooms 33 and 38, which draw once per frame.
  - No dead move. No goal again: the goal is no target of these tries.

**The driver with every switch on** (`integrate/smoke.sh`, 4 jobs; not the kitchen rerun). `fmi explore run` from the
root with the kitchen check's settings, b = 2, the fast settings and every switch, ran the paths no gate runs end to end
in 168 s:

- job.json carried the archive's novelty, and T grew from the goal's atom to 559, then 576 atoms.
- Each try read it and returned its T in result.json.
- The archive ingested 13 new entries (checkpoint 4, end 4, afford 5) into 8 cells over T. The waits were static at
  all 4 entries.
- The fixed `fmi explore verify` verified all 13 from boot with 0 differences.

**Against the prediction.**

- Hit:
  - G0 (15 of 15 in both settings) and G-on (76 of 76);
  - the reading's saving: 23.2 s, predicted 15-25 s;
  - the layer's steps: 9.1 s, predicted 8-15 s.
- Missed:
  - The sum is 0.96 of round 2's, predicted 0.80-0.95. The planning's 5.9 s was not in the prediction.
  - G5's children were 503 in two runs, outside E011's recorded 501-502, and 502 in a third (above).
  - "No fix round beyond the one above" held for the product's code. Two of the integration's own scripts needed one
    each: the smoke run held the heavy lock its driver takes, and the afford stage's environment file reset the
    output folder of the stage after it.

**Not in the merged build.** The due-frame window of section 1 (a paused slot's planned event gets its due frame in
the window search): no component built it. The kitchen rerun's window search is E010's, with the delay fork.
