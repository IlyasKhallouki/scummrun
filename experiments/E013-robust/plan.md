# E013: An unknown mechanic slows progress and never blocks it

Date planned: 2026-11-03
Research question it serves: RQ1 (completion from the game files alone), RQ4 (what carries over to another engine)

## Question

The PI, 2026-11-03: "I do not like how we wait for the problems to show up and then we fix them. What if they never
stop doing that? What if we move to another game and it's another set of problems?"

E008-E012 each found a mechanic the explorer could not handle, then added a fix for it. E013 asks for a system in which
a mechanic the model does not know only slows progress and never blocks it, and which reports its own stalls by cause.
Four parts:

- **(A) The input state from the engine.** What the player can do now comes from the engine's own input code, not from
  verb ids read by hand.
- **(B) Refused inputs.** The engine says when an input is not taken, and the window search stops or moves on at once.
- **(C) Stall accounting.** Every second of a try goes to a cause, from the harness's own outcomes. The table is live
  per run, and the driver uses it.
- **(D) A model-free share of tries.** The interface's inputs, waits and delays from cells chosen by novelty, as a
  standing share of the tries, not only when the model is stuck.

The tests: E012's kitchen rerun again with E013 on, the same with the model off, then Part I on a cloud VM.

## Baseline: E012's kitchen rerun

Sources: `experiments/E012-game-mechanics/plan.md` (its kitchen results and the corrections after the check),
`notes/log/2026-11-03-e012.md` (its Check section), `results/kitchen/report.json`, and the archive
`/var/tmp/fast-mi/e012-archives/kitchen` (read only).

| | measured |
|---|---|
| result | the goal reached at the 53rd committed try, verified from boot; chain 15,688 frames from 2728, 85 inputs |
| try time | 1,658.1 s (27.6 min) over 53 tries; median 15.2 s, max 163.6 s |
| how the tries ended | 24 R0 (Lemma R, a dialog), 13 the same empty window twice, 10 the target reached, 5 no attempt played, 1 the reach extension |
| by part | read 491.8 s (29.7%), plan 362.0 s (21.8%), affordance steps 728.8 s (44.0%), play 45.0 s, T 16.1 s, wait 6.1 s, return and rest 9.9 s |
| affordance steps | 42 steps, 5,767 trials, 33 kept. 29 dialog steps: 14.3 s in all. 13 sentence-idle steps: 32-130 s each, 714.5 s in all; 711.3 s of the steps' time reads each trial's outcome and runs the wait after it (126 ms per trial) |
| tries with no new atom of T and no new room | 34 of 53, 781.8 s (47%) |
| longest stretch without a new atom or room | 10 committed tries (13.3 s); by seconds, 223.9 s over 3 tries |

What the independent check measured (the task's three counts):

1. **The affordance step uses this game's verb ids.** It offers inputs by the bridge's idle kind. The bridge
   (`speedrun_state.cpp`, from E003) calls a frame a dialog when a verb 120-128 is visible, and a sentence idle when a
   verb 2-10 is visible or a room's own input script runs. The ranges were read from the scripts by hand, against D9.
   Entries 142 and 146 on the goal's chain came from dialog steps.
2. **What bound the chain was not a timer.** Four of its windows ran to E010's 3,600-frame bound (14,400 of the
   15,688 frames). Three retried a sentence the interface refused ("verb key did not select verb 10"). The fourth
   retried a walk whose effect on var:102 never matched. 13 of the 53 tries ended on empty windows.
3. **24 of the 53 tries stopped on R0 in a dialog,** each followed by an affordance step.

## Research (sources opened)

### ScummVM's SCUMM v5 input path

From the engine tree E012 built (`/var/tmp/fast-mi/svm-e012-end/third_party/scummvm`, read only).

- **Verb slots.** `struct VerbSlot` (`engines/scumm/verbs.h:47-59`) holds `curRect` (the drawn box), `verbid`,
  `type` (text or image), `curmode`, `saveid` and `key`, among others.
  - A slot reacts to clicks and keys when `verbid != 0`, `saveid == 0` and `curmode == 1`. This is the key loop of
    `checkExecVerbs` (`verbs.cpp:609-618`), and `findVerbAtPos` uses the same test for clicks.
  - The bridge's dump lists each visible slot's id, mode, saveid, type, key and top-left corner, but not the box's
    size.
- **The gate.** `checkExecVerbs` (`verbs.cpp:586-692`) returns at once when `_userPut <= 0` (`:590`).
  - `_userPut` and the cursor state are set only by `o5_cursorCommand` (`script_v5.cpp:853-937`): userput on, off,
    soft on and soft off. `VAR_USERPUT` and `VAR_CURSORSTATE` mirror them.
  - `processInput` (`input.cpp:409`) clears `_mouseAndKeyboardStat` every frame (`:439`) before it reads the frame's
    key or button. So an input that meets a shut gate is gone: it is not queued.
  - In v5, var 0 (`VAR_KEYPRESS`), vars 13-14 (`VAR_VIRT_MOUSE_X`, `_Y`) and vars 15-16 (`VAR_MOUSE_X`, `_Y`) are the
    engine's input registers (`vars.cpp:227-241`).
- **The handler.** With the gate open, `checkExecVerbs` does one of three things:
  - a key equal to a visible slot's `key` runs `runInputScript(kVerbClickArea, verbid, 1)`;
  - any other key runs `runInputScript(kKeyClickArea, key, 1)` (`:664`);
  - a click runs `findVerbAtPos`, then the verb area or the scene area (`:675-690`).

  `runInputScript` (`script.cpp:1500-1533`) starts the script that `VAR_VERB_SCRIPT` names, with (area, value,
  mode), and does nothing when that variable is 0. Everything after that is the game's own script. The engine does not
  know a dialog from a sentence.
- **The sentence machinery.** `doSentence` (`script.cpp:1167`) queues a sentence. `checkAndRunSentenceScript`
  (`script.cpp:1196-1240`) starts `VAR_SENTENCE_SCRIPT` on it, but only while that script is not already running
  unfrozen.
- **Order within a frame.** In v5 the order is `runAllScripts()`, `checkExecVerbs()`, the bridge's decision point,
  then `checkAndRunSentenceScript()` (`scumm.cpp:3232-3257`).
- **The bridge's idle rule** (`speedrun_state.cpp:323-422`) is engine-generic up to its last step. Its blockers are
  save/load, the banner, `_userPut`, a cutscene, the input script, the sentence queue, the sentence script, an object's
  verb script, a message with a real talker, a script parked in WaitForMessage, a fade, and ego moving. The hand-read
  verb ranges act in two places:
  - a visible verb 120-128 makes the frame a "dialog" decision point regardless of the sentence, object and message
    blockers (`:346-354`);
  - otherwise the frame is a "sentence" decision point only when a verb 2-10 is visible or the input script is a
    room's own (`:412-421`).

  The same ranges appear in `speedrun_control.cpp:224` (the stop on a menu) and `speedrun_plan.cpp:42, 439, 473` (E003's
  plan player).
- **The window search's executor** (`fmi-play/src/executor.rs:20-46`, from E003's executor.py) carries a game profile
  read by hand: the selected-verb variable 107, the walk verb 11, the inventory slots 200-207 with variable 133, the
  arrows 208/209, the verbless rooms, and the hover variable 394. "verb key did not select verb 10" is the executor's
  own check of variable 107 after it presses the verb's key (`executor.rs:772-780`).
- **Diagnosis, not an input of the system.** In this game's input script (global 4 in the DOS CD scripts), a click on
  the verb area stores the verb's id as the selected verb in the same run. The exceptions are ids 0 and 100, the
  inventory slots and their arrows. So the refused key did not reach the handler as a click on verb 10 by the frame at
  which the executor read the result. It met a shut gate, went to the key area, or was handled later. M1 below
  measures which.

### What other engine families expose (ScummVM, the same tree)

- **AGI.**
  - Input is enabled and disabled by `accept.input` and `prevent.input` (`engines/agi/op_cmd.cpp:1967`, `:1976`: the
    prompt on and off). The player steers ego under `player.control` and loses it under `program.control` (`:1637`,
    `:1646`).
  - `set.key` maps keys to controller codes (`:2111`), tested by `controller(n)` (`op_test.cpp:286`). Menu items are
    enabled and disabled by `enable.item` and `disable.item` (`op_cmd.cpp:750`, `:756`).
  - A typed line sets flag 2. `said()` (`op_test.cpp:329-385`) answers false when flag 4 is already set, and sets flag
    4 on a match (`agi.h:284-286`; AGI Command Reference, Test Commands, wiki.sierrahelp.com).
  - So the engine itself knows when a typed line was refused: flag 2 set and flag 4 unset after the logics ran.
- **SCI.**
  - Scripts ask for events with `kGetEvent(mask, event)` (`engines/sci/engine/kevent.cpp:44`). The types are mouse,
    key, direction and said (`event.h:31-47`).
  - An Event object's `claimed` property is TRUE once some object has responded to it (Script Classes for Adventure
    Games, Event Class). `kSaid` does not match a claimed event and claims it on a full match. `kParse` claims an
    event whose line has an unknown word or bad syntax (`kparse.cpp:65-79`, `:125-145`).
  - Whether the player may act now is the User object's `input`/`canInput` and `controls` properties, which ScummVM
    itself reads in `GuestAdditions::userHasControl` (`guest_additions.cpp:169-215`).
  - The Mac icon bar is engine-side, with an enabled flag per icon. A press on a disabled icon is swallowed
    (`graphics/maciconbar.cpp:295-316`). The PC icon bar is a script class, which the engine sees only through
    selectors (`script_patches.cpp` names `curIcon`).
  - So "refused" is an event left unclaimed after the handler chain.

All three families answer the same three questions, each from its own structures: is input accepted now, what are the
choices, and was the last input taken. That is the adapter E013 defines. Only SCUMM's is built here.

### Practice

- **TAS lag frames.** In an emulator, a lag frame is "a frame in which input was not polled by the core" (adelikat,
  TASVideos forum, topic 16042). BizHawk's lag marking has a limit: "the game can still poll input but do nothing with
  the input" (RetroEdit, TASVideos forum, post 497490). SCUMM's gate is the poll. The handler's run is where an input
  is "polled but ignored", which an engine report can see and a lag counter cannot.
- **Go-Explore** (Ecoffet et al., arXiv 1901.10995v4, section 2.1.4). After returning to a cell, Phase 1 takes random
  actions for k = 100 training frames, repeating the previous action with probability 0.95. No neural network is used
  in Phase 1. So all exploration from a cell is model-free.
- **Type-GBFS** (Xie, Müller, Holte, Imai, AAAI 2014). It alternates one expansion from the heuristic's open list
  with one from type buckets (a bucket uniformly at random, then a node uniformly in it). This is a standing 1:1 share
  of exploration. ε-GBFS (Valenzano et al. 2014, as described there) explores with probability ε.
- **BFWS** (Lipovetzky and Geffner, AAAI 2017, abstract). Greedy best-first search had been improved "by adding forms
  of exploration when reaching heuristic plateaus". Combining goal-directed search with width-based (novelty)
  exploration beats both.
- **Agent57** (Badia et al., arXiv 2003.13350, section 3). A family of N = 32 policies runs from exploratory to
  exploitative. A meta-controller per actor picks the policy for each episode with a sliding-window UCB bandit
  (window 160 episodes, ε = 0.5 for the actors) on the episode's extrinsic return. The share of exploration thus
  adapts to what pays.

## Measured for this plan (from the E012 archive's files, read only)

- **The windows that ran to the bound.** 19 of the archive's window records ran to E010's 3,600 frames.
  - 16 have every try ending "interface" (296 each, 4,736 tries), all on a sentence with verb 10 (objects 452 and
    382). 12 windows end "verb key did not select verb 10" (3,552 tries). 4 end "object 382 never came on screen (no
    floor path, ...)" (1,184 tries): the executor could not form the input, so no input reached the engine.
  - The other 3 are walks whose 294 tries were all spent with no match (j00048, j00049), or ended "goal, not from the
    plan" (j00031).
  - The 16 windows took 1.1-3.4 s of wall each and are in 8 tries (j00013, j00018, j00019, j00021, j00030, j00032,
    j00045, j00046). Those 8 tries took 584.9 s (35.3% of the try time), nearly all of it in the sentence-idle
    affordance step that followed.
- **By how the tries ended and whether they gave progress** (try wall; progress = a new atom of T or a new room):

  | stop | progress | tries | wall s | share |
  |---|---|---:|---:|---:|
  | Lemma R's R0 | yes | 8 | 343.0 | 20.7% |
  | Lemma R's R0 | no | 16 | 242.6 | 14.6% |
  | the same empty window twice | yes | 6 | 456.8 | 27.5% |
  | the same empty window twice | no | 7 | 248.6 | 15.0% |
  | none played | yes / no | 1 / 4 | 8.2 / 212.4 | 0.5% / 12.8% |
  | target reached (from the plan or not) | yes / no | 4 / 6 | 68.3 / 46.5 | 4.1% / 2.8% |
  | the reach extension | no | 1 | 31.7 | 1.9% |

## What the research changed in the proposal

1. **The engine has no notion of dialog or sentence.** It has a gate (`_userPut`), a handler (`VAR_VERB_SCRIPT`), the
   choices (verb slots with their boxes and keys, the scene) and the sentence machinery. So (A) reports those and
   drops the kinds. Which choices are worth trying is then decided by what the handler does with them, which is (B).
2. **"Refused" has two levels:** not polled (the gate), and polled but ignored (the handler ran and changed nothing).
   The emulator's lag counter sees only the first. The bridge can see both, because it sits inside the engine.
3. **The kitchen's refused key never reached the handler** as a click on verb 10 (the diagnosis above). So the
   engine's record of the key's path (gate, slot match, handler) names the cause, and the next frame worth trying is
   the first one at which that path can change.
4. **The empty windows' cost is in the chain, not the wall.** The 16 bound windows took about 30 s of wall in all.
   But each one carried 3,600 idle frames into the next entry. The time they cost went to the sentence-idle
   affordance step after them (126 ms per trial, most of it the wait after each trial).
5. **A trial's wait is needed once per distinct state.** Equal engine states have equal futures (determinism, A0).
   So an engine digest lets trials with the same outcome share one wait, and lets a trial whose state equals the
   no-input twin's skip it.
6. **A standing share of model-free exploration is the common practice:**
   - Go-Explore explores model-free after every return;
   - Type-GBFS alternates 1:1;
   - BFWS shows that exploration only at plateaus is weaker than exploration built into the search;
   - Agent57 adapts the share to what pays.

   E013 uses a standing share with a deterministic shift rule fed by the stall table. It does not use a bandit, so
   that a run is reproducible from its files.

## Method

Every mechanism below:

- makes entries only from real engine runs, each verified from boot like any entry (A0, Lemma C);
- prunes no state from the archive: a skip or a dedupe only decides which runs are made;
- uses only the engine's own structures (verb slots, `_userPut`, `VAR_VERB_SCRIPT`, the sentence queue, script slots,
  the save state) and the harness's own outcome records.

No verb id, object id, room number or genre rule is written (D9).

### A. The input state from the engine

**Engine patch 0006** (observation only; with its switches unset the engine runs as patches 0001-0005).

1. **The input report** (`Input`, binary and JSON) at any stop:
   - the gate: `_userPut`, the cursor state, the cutscene level, save/load, the banner;
   - the handler: `VAR_VERB_SCRIPT`'s script, whether it is a room's own (local) script, its slot if running;
   - the sentence machinery: `VAR_SENTENCE_SCRIPT`, the queued sentences, the sentence script's slot and freeze count;
   - every verb slot with `verbid != 0`: slot, id, `curmode`, `saveid`, `key`, `curRect` (left, top, right, bottom),
     text or image, and a hash of its rendered text;
   - the click zones: the virtual screens' topline and height (the main screen is the scene area);
   - the player's chain (below), and the decision rule's clauses.
2. **The input log** (`InputLog`): for each input the bridge injected since the last call, its path through
   `checkExecVerbs` and the handler:
   - `dropped: gate` (`_userPut <= 0`), `dropped: zone` (no virtual screen), or `dropped: handler`
     (`VAR_VERB_SCRIPT` is 0);
   - `handled`, with the area, value and mode given to the handler, the slot matched for a key, and the verb slot
     under a click;
   - for a handled input, the handler's run until its slot dies or the frame ends:
     - its writes: variables and bits (`writeVar`), owners, states and classes, verb ops, scripts started, sentences
       queued, walks, the camera, the cursor and `_userPut`, prints;
     - its reads: variables and bits (`readVar`), and the object and actor queries its opcodes made;
     - the verdict: `ignored` when it wrote nothing outside its own locals, else `taken`.
3. **The state digest** (`Digest`): a 64-bit hash over the engine's save state (the bytes
   `saveLoadWithSerializer` writes to a memory stream), the RNG state and the input registers. This is the engine's own
   definition of its state. Two equal digests are taken as two equal futures. Gate B4 tests that.
4. **The decision rule from the engine** (switch `SPEEDRUN_IDLE=engine`; unset, the bridge's rule as today). A frame
   is a decision point when:
   1. the gate is open: no save/load, no banner, `_userPut > 0`, no cutscene;
   2. the handler is set and not running;
   3. the player's last input has played out:
      - no sentence is queued;
      - every unit of its chain is dead or parked. The chain is the handler, the sentence script, an object's verb
        script, and any script one of them started since that input. Parked means running, not in `o5_wait`, at the
        same code position as at the frame before, and having written nothing this frame. That is a loop polling a
        value only another unit can change;
      - no message printed by a chain unit is still showing;
      - no fade is pending, and ego is not moving;
   4. there is something to act on: a visible verb slot, or a room's own handler.

   The status's idle kind is then "input". The input report always carries both rules' answers at its frame (the
   engine rule's clauses and the old rule's kind or blocker). The switch only chooses which rule stops the runs, so
   gate A3 can compare the two on the same states.

   The old rule differs in three clauses:
   - (i) any real talker's message blocks, background speech included, except in a dialog;
   - (ii) a live sentence script or object script blocks even when parked, except in a dialog;
   - (iii) the verbs must be 2-10 or 120-128.

**The Rust side.**

- fmi-core gains the report, log and digest types and `Engine::input_state`, `input_log` and `digest`, added byte for
  byte to E011's Appendix A.
- The family adapter, "what the player can do now", is a trait in fmi-explore:

  ```rust
  pub trait PlayerNow {
      /// the choices at this stop: SCUMM's visible verb slots, hit targets and walk boxes (AGI: the loaded logics'
      /// said() patterns, set.key controllers, enabled menu items; SCI: enabled icons, features, said specs)
      fn offer(&mut self, e: &mut dyn Engine) -> EResult<Offer>;
      /// what the engine did with each input given since the last call (SCUMM: InputLog; AGI: flags 2 and 4 after
      /// the cycle; SCI: the event's claimed property after the handler chain)
      fn verdicts(&mut self, e: &mut dyn Engine) -> EResult<Vec<InputRecord>>;
  }
  ```

  E013 builds the SCUMM adapter only.
- The affordance step's enumeration has no kinds (switch `FMI_INPUT=1`):
  - first clicks over every visible verb slot, every hit target and every walk box;
  - a second click over the targets only after a first click that left the game composing;
  - a third likewise.

  Composing is E012's test without its kind: the handler ran, no sentence script or object script ran, the frame is a
  decision point again, and the verdict was `taken`. In a dialog the line clicks do not compose, so there are no second
  clicks. In a sentence idle the verbs do, so the set is E012's.
- `ready` waits for the engine's decision point. The boot checks accept the kind "input" under the switch.

### B. Refused inputs

**In the window search** (switch `FMI_REFUSE=1`). When a try fails at the interface (the executor's route error),
the child's input log names the cause:

| cause | the next try | the window ends at once when |
|---|---|---|
| `dropped: gate` | at the gate's opening (E010's gate wait, unchanged) | never by B |
| `dropped: zone` or `dropped: handler` | at the first frame where the input report changes | the window's frames end with no such change |
| handled: `ignored`, or `taken` and the executor's own check after it failed | at the first frame where a value the handler's run read, a field the executor's check read, or the input report changes | the same, for those values |
| the executor could not form the input (no point on screen, no path) | at the first frame where the dump's interface part changes: room objects' boxes and states, actors, the camera, verbs, inventory | the same, for that part |

- **The parent still runs** the window's frames, as now, to find the next change; that is cheap (1.1-3.4 s per
  window in the kitchen).
- **When no change comes,** the window ends with its stop at the refusal's frame, not at the bound. The harness
  replans from that state with the cause "refused input: <cause>". The state is the parent's own at that frame (a
  fork kept at the window's start), so the idle frames are not carried into the chain.
- **Exactness.**
  - A skipped try is one whose input would meet the same gate, the same slots, the same handler run and the same
    check: the engine, the scripts and the executor are deterministic, and the skipped frames hold the same values for
    everything the path read.
  - So every skipped try would have been refused, and no state is lost. Any later frame stays reachable by the wait
    move from the window's start.
  - Gate B1 checks the argument on every recorded window. If a read is missing from the handler's reads, B1 shows it.
- **"Moves on" is limited to skipping.** B does not ban the input from the next plan. The second identical window
  still stops the attempt, as E012's rule does.

**In the affordance step and the free tries** (`FMI_REFUSE=1`).

- A trial whose every input is `dropped` or `ignored` ends at once, with no wait.
- A trial's state at its decision point is digested. If it equals the no-input twin's digest, or an earlier trial's,
  the wait already run for that state is reused.
- Only trials with a new state run the wait's stops.

### C. Stall accounting

Switch `FMI_STALLS=1`. Off, the report command still reads any archive's files.

- **Rows.** Every second of a try's wall goes to exactly one row (phase, outcome):
  - the try's parts (return, read, T, wait, plan per request, play per window, affordance and free trials, rest), each
    with its outcome as the harness wrote it;
  - stop reasons, window end reasons and child outcomes, probe statuses, B's causes, and trial verdicts.

  The rest (the wall less the parts) is a row of its own.
- **Classes.** Rows map to classes by a fixed table over the harness's own vocabulary. The table names no game fact.
  - **productive:** a part whose outcome kept an entry holding a new atom of T or a new room. The read, T, plan and play
    seconds of an attempt are productive when the attempt's entries were;
  - **model-blind:** the attempt stopped on Lemma R's R0, or every target stopped on the reach extension;
  - **no plan:** a request proven dead, deferred at its limit, or incomplete;
  - **refused input:** a window ended by B, or trials `dropped` or `ignored`;
  - **effect mismatch:** a window whose tries were spent with no match;
  - **interface, no yield:** taken trials that kept nothing;
  - **wait:** a wait's frames with no kept stop;
  - **overhead:** return and rest.

  A try's non-productive seconds go to its stop's class, except its afford, free and wait parts, which have their own.
- **Live.** After each commit the driver writes `stalls.json` in the archive (per class and per row, over the run and
  over the last 20 committed tries) and prints the top three classes. `fmi explore stalls ARCHIVE` prints the table at
  any time.
- **The driver's use** (switch `FMI_FREE_SHIFT=1`, with D). Both rules are deterministic functions of the archive's
  files:
  - **globally:** when one non-productive class holds at least 50% of the last 20 committed tries' seconds, the share
    of free tries goes from ρ0 to 2ρ0 (at most 0.5). It comes back to ρ0 when that class falls below 30%;
  - **per cell:** a cell whose last two model tries both ended non-productive in the same class gets its next try as a
    free try.

### D. A model-free share of tries

A **free try** (switch `FMI_FREE=ρ0`, the standing share; the kitchen and Part I use ρ0 = 0.25):

1. **Kind.** Job n is free when floor((n + 1)·ρ) > floor(n·ρ), with ρ the current share, or when the cell rule above
   says so. With ρ = 0.25, jobs 3, 7, 11, ... are free; with ρ = 1 (K2), every job.
2. **Cell.** The archive's progress order (E012's cells over T, Go-Explore's counts), with a separate count of free
   tries per cell, so a cell that the model has worn out stays open to free tries.
3. **Return and T.** The return is E012's. T is the scope's reading's (the reading cache, no planning) merged with the
   archive's T.
4. **Wait.** E012's wait. Its stops are candidate entries by novelty.
5. **Offers** at the entry's decision point and at each of the wait's stops (time passing, then acting): A's
   enumeration, with B's verdicts and digest dedupe. Each new state runs the wait's stops (E012's `then`).
6. **Delays.** A taken input whose run made more random calls than the no-input twin's over the same frames is tried
   again 1, 2, 3, 5, 8, 13 and 21 frames later. The calls are counted exactly from the `rng` states. This is TAS's
   frame-delay lever and ALE's no-op start, applied only where a draw was consumed.
7. **Depth.** From each outcome kept in the try, one more level of offers. IW(1) within the try: an atom kept once is
   not novel again.
8. **Commit.** The kept outcomes are entries with "how": "free". The dead moves are as E012's.

**Exactness.** Every kept entry is the entry's movie, then the trial's clicks and delays. It verifies from boot like
any entry. Nothing is pruned from the archive.

**In model tries.** E012's affordance step still runs at R0, at the same empty window and when no attempt plays, now
with A's enumeration and B's verdicts.

### How a try runs, with E013 on

- **A model try** is E012's order. Its decision points are the engine's (A). Its window search stops or skips on
  refused inputs (B). Its affordance step uses A and B, and its seconds are attributed (C).
- **A free try** is D's.

The driver picks the kind (C, D), and writes `stalls.json` after each commit.

## Components (on top of e013 = origin/main 338daab)

Each component is on its own branch `e013-<key>` in its own worktree (`/var/tmp/fast-mi/wt-e013-<key>`), builds into
`/var/tmp/fast-mi/target-e013-<key>` (the engine into `/var/tmp/fast-mi/svm-e013-<key>`) and writes under
`/var/tmp/fast-mi/e013-*` or `~/.cache/fast-mi-e013/`.

- Every switch is off by default.
- Interface changes are posted in `~/.cache/fast-mi-e013/COORD.md`. Shared fmi-core additions are copied byte for byte.
- `input`'s patch and fmi-core additions come first. `refuse` and `free` start from them (cherry-picked unchanged).
  `stall` needs only the result files.

| key | files | interface | switches | gates |
|---|---|---|---|---|
| `input` (A) | `patches/0006-scumm-speedrun-input.patch` (speedrun_state.cpp: report, decision rule, shadow; speedrun_inputs.cpp: the log; speedrun_wire.cpp: `Input` 0x96, `InputLog` 0x97, `Digest` 0x98 and their JSON; hooks in `checkExecVerbs`, `runInputScript`, `runScript`, `doSentence`, `writeVar`, `readVar`, the walk start, `actorTalk`); fmi-core; fmi-engine; fmi-explore `afford.rs`, `player_now.rs`, the boot checks; fmi-checks `input-report`, `idle-gate` | `InputState`, `VerbBox`, `InputRecord { frame, input, verdict: Dropped(why) \| Ignored \| Taken, area, value, slot, writes, reads }`; `Engine::input_state`, `input_log`, `digest`; `trait PlayerNow` | `SPEEDRUN_INPUT=1` (report and log hooks), `SPEEDRUN_IDLE=engine`, `FMI_INPUT=1` | A1-A5 |
| `refuse` (B) | fmi-play `window.rs`, `executor.rs` (the cause on a route error, the next try, the end at the refusal, the kept fork); fmi-explore `afford.rs` (verdicts, digest dedupe); fmi-checks `refuse-check`, `digest-check` | the window record's `"refuse"`: per failed try its cause, the skipped frames, the change that ended a skip; end reason "refused input: <cause>"; per trial its verdict and digest | `FMI_REFUSE=1` | B1-B4 |
| `stall` (C) | fmi-explore `stall.rs`, `driver.rs` (stalls.json, the live line, the shift rules), `explorecmd.rs` (`fmi explore stalls`); fmi-checks `stall-report` | `Stalls::of_try(&result.json) -> Vec<(Row, Class, f64)>`; `Stalls::of_archive(dir)`; `share(&Stalls, ρ0) -> f64`; `free_cell(&Stalls, cell) -> bool` | `FMI_STALLS=1`, `FMI_FREE_SHIFT=1` | C1-C3 |
| `free` (D) | fmi-explore `free.rs`, `tryjob.rs` (the job kind), `driver.rs` (the kind by ρ), `archive.rs` (free counts per cell, "how": "free") | job.json `"kind": "model" \| "free"`; result.json `"free"`: offers, trials by verdict, waits reused, delays, kept | `FMI_FREE=ρ0` | D1-D3 |

**Gates.**

- **A1, the patch is inert.**
  - E011's G1-G4 give 0 differences with patches 0001-0006 and `SPEEDRUN_INPUT` off and on.
  - With both switches off, every recorded movie stops where it did.
  - `SPEEDRUN_IDLE=engine` changes only where runs stop: along G2's and G3's movies the digest at every frame is the
    same with it off and on.
- **A2, the report.**
  - The report's visible slots equal the dump's (slot, id, mode, saveid, key, x, y) at every dump of G2 and G3.
  - For every visible slot, a hit test at its box's inner points answers that slot.
- **A3, the old kinds.** Both rules are read from the input report on the same states, along replays with the old
  rule stopping the runs:
  - at every frame of the goal chain's replay (15,688 frames);
  - at every checkpoint of the E012 and E011 kitchen archives (151 and 226 entries);
  - at the frame of each of E012's 42 affordance offers.

  Reported: the table of old kind (dialog, sentence, blocked by reason) against the new decision. Where they differ,
  the clause (i)-(iii) and the engine fields behind it.

  **Where the old kind was right:** at a frame where the old rule said idle and its enumeration's inputs were taken
  (B's verdicts in forks), the kind-free offer must give a decision point and a taken-input set that contains the
  old one. That is the gate. Frames where the old rule said blocked and the new one says idle are reported with
  whether any offered input is taken there.
- **A4, the chain's dialogs.** At entry 142's frame, with `SPEEDRUN_IDLE=engine` and `FMI_INPUT=1`, the offer
  includes the dialog line whose outcome is entry 146's checkpoint, as a taken, kept trial.
- **A5 (reported, not a gate).** For each constant of the executor's profile, whether A's records give it: the
  variable a verb click's handler run writes the verb id to; the value it writes for a scene click with no verb; the
  image slots; the rooms with a local handler.
- **B1, skips on recorded windows.** Every window record of the E012 kitchen archive and of B3's tries is played again
  with the switch on.
  - Every try B skips was refused in E012's search with the same message.
  - B never ends a window that E012's search ended on an end point.
  - The 16 bound windows are reported one by one: the cause, the skipped frames, where the window ends.
- **B2.** Every entry made after a window that B ended verifies from boot.
- **B3, the verdicts on fixtures** at recorded entries:
  - a key while the gate is shut: `dropped: gate`;
  - a key that no visible slot has: handled in the key area, then whatever the handler did;
  - a dialog line at entry 205: `taken`;
  - a scene click during that dialog: `ignored` or `taken`, reported;
  - a verb click at entry 203: `taken`.
- **B4, the digest.**
  - A fork and a replay from boot at the same frame give equal digests (A0).
  - Two frames of an idle run differ.
  - Every pair of trials with equal digests in the A3 and K1 samples (at least 200 pairs) gives the same stops and
    final checkpoint through the wait. Otherwise the dedupe stays off.
  - The cost per digest is reported.
- **C1.** The report over E012's kitchen archive attributes every try's wall: the rows sum to 1,658.1 s, the rest row
  is at most 1%, and each try's sum equals its wall in report.json. This table is the baseline for the predictions.
- **C2.** Unit tests: the attribution of each outcome kind, the class table, both shift rules (threshold, hysteresis,
  the cell rule), and the job kind sequence being the same when the archive is opened again.
- **C3.** At the end of K1 the live `stalls.json` equals the offline report over the same archive.
- **D1.** Every free entry verifies from boot (verify-all at the end of K1 and K2), 0 differences.
- **D2.** With `FMI_FREE` unset, the driver's job sequence and job files are E012's (G0's driver run).
- **D3 (reported).** Per free try: offers, trials by verdict, waits run and reused, delays, kept, wall.
- **G0, everything off,** on the merged build with patches 0001-0006 and every E013 switch unset:
  - E012's 15 B3 tries equal E012's in everything, against its published on and off runs;
  - the kitchen rerun's goal entry 150 and all 150 entries verify from boot with 0 differences on their checkpoints,
    with `SPEEDRUN_INPUT` off and on;
  - E011's G1-G12 and E012's component gates pass as E012's integration ran them;
  - the driver run: E012's smoke run (`integrate/smoke.sh`, 4 jobs from the boot), with b = 1 so that the commit order
    is fixed, gives the same job files, results and archive rows as E012's merged build run beside it, timing fields
    aside.
- **G-on.** B3's 15 tries with every E013 switch on: every new entry verifies from boot with 0 differences.

## Runs

All runs use seed 1, laptop rules (below), and every E012 switch on as in E012's kitchen rerun.

- **K1, the kitchen rerun with E013 on** (once every gate passes):
  - `fmi explore run` from the boot into a new archive `/var/tmp/fast-mi/e013-archives/kitchen`, to owner:566 = 1;
  - E012's rerun settings: the 10 rooms, b = 2, E009's switches, play when ready, commit as finished, ON3, the
    falsifier at 80 tries;
  - with `SPEEDRUN_INPUT=1 SPEEDRUN_IDLE=engine FMI_INPUT=1 FMI_REFUSE=1 FMI_STALLS=1 FMI_FREE=0.25 FMI_FREE_SHIFT=1`.

  The goal entry is verified from boot in a fresh lean engine and on svm-w3b (E008's `explore.py verify`), and every
  entry by verify-all.
- **K2, the kitchen with the model off:** K1's settings with `FMI_FREE=1` (every try free, no planning, no window
  search), into `/var/tmp/fast-mi/e013-archives/kitchen-free`, the falsifier at 80 tries. It measures whether the
  explorer progresses when the model contributes nothing, the case of a game whose mechanics the model does not know.
- **P, Part I on a cloud VM** (only after K1's yes, and after a dated amendment written before the start):
  - **The amendment** gives the projection from K1's measured costs: model and free tries' mean and median wall, peak
    memory per worker, and the driver's parallel efficiency (E012: 1,658 s of try time in 1,292 s with b = 2). From
    them it fixes b and the hours booked, with a margin of at least 1.5.
  - **The machine.** `scripts/cloud.py start --standard --type n2-custom-30-122880 --hours H` (about $1.53/h; $12.45
    is left under the ledger's stop, so H ≤ 7.5). One start. The VM is standard, not preempted.
  - **The projection now,** from E012's costs: 600 tries × 31.3 s × 2 (Part I's larger scopes and the free tries)
    ÷ (b = 8 × 0.64) is about 2.0 h of driver wall. About 0.7 h more goes to the copy up, the build on the VM and the
    copy back. With the margin of 1.5 that gives H ≈ 4 h, about $6.1.
  - **The run.** `cloud.py up` with e013's head, the game files and the engine source. On the VM: the engine with
    patches 0001-0006 (`--no-reset`), the rs workspace and reader-scope built. Then K1's switches and settings with
    goal `part1` (owner:901, 902, 903 = 1), the 80 rooms from the files, b from the amendment, E009's falsifier of
    600 tries, and stagnation over T.
  - **Copies back.** After every 50 committed tries and at the stop, the archive without its readings goes to a
    staging folder on the VM. At the stop it is copied back (`cloud.py down`), then `cloud.py stop`.
  - **On the laptop:** the best chain by E001's key facts (evaluation only, never an input) is verified from boot
    through `fmi explore verify` and `explore.py verify` on svm-w3b.
  - **Reported:** the Part I rooms entered, the key facts on the best verified chain, the stall table, the shifts, and
    the VM's hours and cost.
- **Reported per run:**
  - the stall table (overall and its last-20 trace);
  - the counts per mechanism: A's decision points and kinds, B's causes, skips and ended windows, verdicts, digests
    reused, D's free tries with their offers, trials, delays and kept;
  - the wall by part and by class, against E012's;
  - the longest stretch without a new atom of T or a new room, in tries and in seconds;
  - the chain's frames and the windows on it.

**Laptop rules.**

- 8 CPUs and 23 GB, shared with the user's desktop.
- Anything above about 1 GB runs under `flock /var/tmp/fast-mi/heavy.lock` and the guard, at nice 10, with
  `cargo -j 2`.
- / and /home keep 5 GB free each. Each component deletes its own build folders once merged.
- ScummVM is never built without `--no-reset`.

## Prediction (before any code)

| | predicted |
|---|---|
| gates | every gate passes, each after at most one fix round |
| M1 (the first step of `refuse`, on patch 0006: the 16 bound windows' first try each, played again with the log) | in all 12 "verb key" windows the log shows the key not reaching the handler as a click on verb 10 (a shut gate, the key area, or a later frame); the 4 "never came on screen" windows give no engine input |
| B1 | every skip agrees with E012's outcomes; B ends 14-16 of the 16 bound windows within 10 frames of their first try |
| A3 | the kind-free offers contain the old kinds' taken inputs at every frame where the old kind was right; the two rules agree on at least 97% of the chain's 15,688 frames, and at least 80% of the disagreements are background speech (clause i) |
| digest | at most 1 ms per digest; every sampled pair with equal digests has the same future |
| K1, reached | yes, verified from boot with 0 differences, in 20-55 tries (most likely 30-45) |
| K1, chain | 1,000-8,000 frames from 2728 (E012: 15,688); no window on it ends at its bound on a refused input |
| K1, try time | 0.25-0.6 h (E012: 0.46 h) |
| K1, sentence-idle steps (affordance and free) | median 8-40 s each (E012: 32-130 s); outcome reading at most 40 ms per trial (E012: 126 ms) |
| K1, free tries | 25-45% of the tries; at least 40% of them keep a novel entry; 0-3 free entries on the goal's chain |
| K1, stall table | at most 1% in the rest row; productive at least 45% of the seconds; no non-productive class above 35%; refused input at most 5% |
| K1, longest stretch with no new atom or room | at most 8 committed tries (E012: 10) |
| K1, shifts | the global shift fires 0-2 times; 3-15 tries made free by the cell rule |
| K2 | not reached in 80 tries; 5-9 of the 10 rooms entered; at least 40% of the tries keep a novel entry; longest stretch with no new atom or room at most 10 tries |
| P | not reached within 600 tries; 14-26 of the 31 Part I rooms entered; 30-60 of the 99 key facts on the best verified chain; no non-productive class above 40% of the run's seconds; 50-90% of the booked hours used |

## Success threshold

- **Yes.** All of the following:
  1. every gate passes;
  2. K1 reaches the kitchen and verifies from boot (0 differences) within 80 tries, with no manual step;
  3. no window in K1 or P ends at its bound on an engine-reported refusal;
  4. in K1, K2 and P every try's seconds are attributed (rest row at most 1%), and the live table equals the offline
     report;
  5. K2, with the model off, keeps a novel entry in at least 25% of its tries and enters at least 5 of the 10 rooms:
     progress continues without the model;
  6. P reaches Part I, or its best verified chain holds at least 30 of the 99 key facts and the run's longest stretch
     with no new atom or room is at most 5% of its tries.
- **Partly.** 1-5 hold, and P falls short of 6 or does not run.
- **No.** Any one of these:
  - K1 does not reach the kitchen within 80 tries;
  - an exactness gate fails and is not fixed;
  - K2 keeps no novel entry in its first 20 tries.

## Stop rule

- **Exactness first.** An entry that differs on verify from boot stops every run until it is explained (A0, Lemma C).
  So does a digest pair whose futures differ (B4): the dedupe is switched off, and the runs that used it are
  reported.
- **Gates.**
  - A component whose gate fails is not merged, and its switch stays off.
  - G0 must hold before any run.
  - If A3 shows the engine rule missing decision points where the old kind was right, the rule is amended (dated)
    before K1.
- **Order.**
  - K1 runs once, under its falsifier and E009's stop rules. K2 runs after it.
  - P runs only after K1's yes and the dated amendment with the projection, once.
- **During a run.** A mechanism that binds (the shift held for more than half of a run, or B ending more than half of
  the windows) is reported with every case. It is not changed during the run.
- **When a stop rule fires,** the write-up gives:
  - the best chain;
  - each mechanism's counts against the predictions;
  - the stall table against E012's;
  - what each try ended on.

  E013 adds no idea beyond its components.
- **Watchdogs.** These are engineering watchdogs, reported where they fire:
  - the harness's, the window search's and the reader's limits (E006's);
  - the guard's 3 GB and the disk floors on the laptop;
  - on the VM, Google's termination time, set by `cloud.py` from the booked hours.

  If the VM's watchdog stops it before a stop rule fires, the last staged archive is the result, and the write-up says
  so.

## Not in E013

- **The executor's game profile** (`executor.rs` `DOS_CD`) stays, and the window search's planned inputs go through it.
  A5 measures how much of it A's records give. Its replacement is a later experiment.
- **The due-frame window** (E012, section 1), still not built.
- **AGI and SCI adapters.** E013 defines `PlayerNow` and builds SCUMM's. No other family's game is run.

## Amendment 2026-11-03, component `free`: what D leaves open, before its code

Written before free's code (branch e013-free). It fills in what section D and the component's row leave open; nothing
there changes.

- **The kind.** Job n is the job's number in the archive (j00003 is n = 3). With ρ the share, the job is free when
  floor((n + 1)·ρ + 10⁻⁹) > floor(n·ρ + 10⁻⁹) (the tolerance keeps ρ = 0.3 exact on the float). ρ is `FMI_FREE`'s
  ρ0, or with `FMI_FREE_SHIFT=1` stall's `share` over the archive at the job's start. When the share does not make the
  job free, the model's candidate is selected as in E012, and with `FMI_FREE_SHIFT=1` stall's cell rule on that
  candidate's cell can make the job free from the same entry (a cell's candidate does not depend on the counts).
  free gives stall's two rules a port (`free::Shift`); without stall wired, `FMI_FREE_SHIFT=1` stops the driver with
  an error. `FMI_FREE` needs `FMI_SELECT=progress` (the cells over T); the driver refuses it alone.
- **Counts.** A committed line carries its job's kind. The model's counts (`n`, `chosen`) are counted over the model's
  tries only, and the free counts (`free_n`, `free_chosen`: the free tries from the cell, and the free tries since one
  from the cell last gave a new atom of T or a new room) over the free tries only. A free job's entry is selected by
  E012's order with the free counts in place of the model's: (free_n, free_chosen, −rooms on the chain, −novel atoms
  held, frame, id). A free try never marks its entry exhausted (it tries no target). Stagnation is E012's over every
  committed try.
- **The reading and T.** A free try plans nothing, so it needs a reading only for T. It takes the first reusable cached
  reading of the scope (E011's choice); when none is reusable, the newest cached reading of the scope ("T only", the
  reading's start differs from the entry's); when the scope has none, a fresh reading, which the archive registers as
  any. The try's T is that reading's, written to result.json "tested" as a model try's. Novelty is over that T merged
  with the archive's T (job.json "novelty"), each atom once. The wait runs on the reading's T, as E012's wait.
- **The order of a free try:** return; the reading and T; the wait (its stops kept by novelty, "how": "wait"); then
  the offers.
  1. **Offer points:** the entry, then each of the wait's stops in frame order (an engine at a stop is a fork of the
     entry run with no input to the stop's frame, held to the stop's checkpoint). At each, E012's affordance step
     (`mech::afford_step`: ready, the offer, the inputs, the trials with the wait's stops after each, IW(1) within the
     try). With input and refuse merged and their switches on, that step's enumeration is A's and its verdicts and
     dedupe are B's; free calls the same step. The step runs whatever `FMI_AFFORD` says (that switch is the model
     tries').
  2. **Delays,** at each first-level offer point: for each trial of one or two clicks that the run did not end (and,
     with refuse, whose verdict is `taken`), its clicks are played again in a fork of the offer's engine to the
     trial's stop frame, and the random calls from the offer's state are counted exactly (xorshift32 steps from the
     offer's `rng`, up to E012's 100,000) for it and for a no-input fork at the same frame. Where the trial's count is
     larger, its clicks are tried again 1, 2, 3, 5, 8, 13 and 21 frames later: the offer's engine run d frames with
     no input, then afford's `try_all` with the same clicks (two-click trials may compose a third click as in E012).
     A count not found within the cap is reported as unknown and gives no delay. Three-click trials get no delay
     (their third click's frame follows the second's stop).
  3. **Depth:** from each outcome kept by 1 or 2 (in the order kept), the engine at its checkpoint (a fork of the
     entry with the outcome's clicks, run to its frame and held to its checkpoint) gets the affordance step once
     more, without delays and without a further level.
  4. **Commit:** the outcomes of 1-3 are entries with "how": "free", their events the entry's movie then the clicks
     (and the delay's idle frames, which have no event). Dead moves as E012's.
- **Records.** With `FMI_FREE` unset nothing below is written and job.json, select.json, result.json, cells.json and
  the log are E012's. With it set: job.json "kind" ("model" or "free"); select.json "kind", "why" ("share" or "cell")
  and "rho"; result.json "kind" and, for a free try, "free": the offer points (where, frame, level, trials, kept,
  seconds, the step's record), the trials (with refuse: by verdict), the waits run (the wait's stops after a trial)
  and reused (the trials that stopped less the waits run), the delays (trials counted, inputs delayed, unknown
  counts, delayed trials, kept), the kept by level and the wall; the log line "kind"; cells.json "free" ([free_n,
  free_chosen]) per cell.
- **Gates as run on the branch** (before input and refuse are cherry-picked, the step is E012's):
  - D2: E012's smoke run (`integrate/smoke.sh`, 4 jobs from the boot) with b = 1, `FMI_FREE` unset, on this build
    and on E012's merged build (`target-e012`), one after the other: the same job.json, select.json and result.json
    (timing and path fields aside), entries.jsonl, cells.json and log.jsonl (timing aside).
  - D1: the smoke run with `FMI_FREE=0.25` (b = 1, 4 jobs: j00003 free) and with `FMI_FREE=1` (b = 2, 4 jobs, every
    job free); every entry verified from boot (`fmi explore verify`), 0 differences.
  - D3 reported from those runs; unit tests of the kind sequence, the counts and the free order, the job files with
    the switch off, the delay rule and the shift port.
  - Once input's and refuse's commits are cherry-picked, D1 again with their switches on.

## Amendment 2026-11-03, component `stall`: the rows, the class table and the shift rules, before its code

Written before stall's code (branch e013-stall). It fills in what section C and the component's row leave open; two
points go beyond them and are marked as such.

- **What a try's seconds are.** The try's wall is result.json `wall_s` (report.json's `wall`). The parts are the try's
  own records, each once, none nested in another:
  - return `return.boot_s`;
  - read: the reading's `wall_s` plus `load_s` when fresh, its `load_s` when reused;
  - T `e012.t.seconds`; the wait `e012.wait.seconds`, or with the wait off E009's `idle.seconds`;
  - per request (each `probes` item, its attempt's `run/aNN/run.json`): each window's `wall_s` (play), each refinement's
    and extension's `seconds` (read), and the request's `seconds` less those (plan, at least 0);
  - each affordance step's `step_seconds`; a free try's `free` record (free's amendment: its offer points' seconds,
    its delays', and the rest of its wall);
  - the rest: the wall less every part above. It is the only row that can be negative (the records round to 0.1 s and
    0.01 s).
- **Rows** are (phase, outcome), the outcome as the harness wrote it, its numbers written N:
  - ("return", same | differs); ("read", the reading's `why`); ("read: refinement", added | none added); ("read:
    extension", replan | no replan); ("T", ""); ("wait", its `end`); ("idle", "");
  - ("plan", the request's status, or its stop's kind when it has none, e.g. "reach extension");
  - ("play", the window's end reason; its children's outcomes but "cancelled"), e.g. ("play", "N frames; interface");
    B's windows end "refused input: CAUSE";
  - ("afford", "WHY, KIND") per step, and with B's verdicts ("afford", "WHY: VERDICT") per verdict with the step's
    other seconds under ("afford", "WHY: other");
  - ("free", "WHERE, level L") per offer point (by verdict likewise), ("free", "delays"), ("free", "other");
  - ("rest", "").
- **Which entries are new.** An entry is new when it is live and holds an atom of the commit's `new_t` (its last facts,
  `TAtom::holds_in`) or its room is in `new_rooms`: the archive's own novelty at the commit (log.jsonl), not the try's.
  result.json `new` item k is the line's `new_entries[k]`. The attempt's entries are those of how "checkpoint", "end"
  and "delay"; the wait's "wait"; an affordance step's the next `kept` items of how "afford", step by step; a free
  try's "free". The report checks that the new entries' atoms and rooms give back each line's `new_t` and `new_rooms`.
- **The class table** (fixed; it names no game fact):

  | rows | class |
  |---|---|
  | return, rest | overhead |
  | read, T, plan, play of a model try whose attempt's entries include a new one | productive |
  | the same, otherwise | the try's stop class (below) |
  | wait or idle | productive when a wait entry is new, else wait |
  | an affordance step or a free try's part, its trials taken or without verdicts | productive when one of its entries is new, else interface, no yield |
  | its trials `dropped` or `ignored` | refused input |
  | read and T of a free try with nothing new | interface, no yield |

  | the try's stop (the played attempt's; else from its requests) | stop class |
  |---|---|
  | "refinement: ..." (Lemma R's R0 and the other refinement stops), "reach extension: ...", "the engine's state is outside the task's domains ...", "... Lemma 2's premise failed"; no attempt played and every request stopped on the reach extension | model-blind |
  | "no plan (...)", "planner timeout ...", "planner ...", "time limit"; no attempt played otherwise (no target, every request dead, deferred or incomplete) | no plan |
  | "the same empty window N times on R: ..." when each empty window on R ended "refused input: ..." or ran to its frame bound with every try failing at the interface (children "interface") | refused input |
  | "the same empty window ..." otherwise, "replan limit", "replan ... the same plan again" | effect mismatch |
  | "goal", "goal, not from the plan ..." (the target reached), "engine: engine ended: R" with R the game's end | model, no yield |
  | "engine: ..." otherwise, "error: ...", an error, the return differs | overhead |

  **Beyond section C:** "model, no yield" is a ninth class. Section C names none for an attempt that reached its target
  (or the game's end) and kept nothing new; E012's kitchen had 6 such tries.
- **A try's class** (the cell rule, the reports): productive when its line gave a `new_t` atom or a new room; else, for a
  model try, its stop class; for a free try, the class with most of its seconds other than overhead. A try's kind is
  the log line's `kind` (free's), "model" when absent.
- **stalls.json** (`FMI_STALLS=1`, written by the driver after each commit, and by `fmi explore stalls ARCHIVE --json`):
  per class and per row (seconds, tries), over the run and over the last 20 committed tries; each try's job, entry,
  kind, wall, class and seconds by class; the trace after each commit (the last 20's share per class, the shift's
  state); the cells the cell rule names. The driver prints one line: the run's top three classes, the last 20's top
  three and the shift. Live and offline are the same function over the same files.
- **The shift rules** (`FMI_FREE_SHIFT=1`), replayed over the committed tries in commit order, so that a reopened archive
  gives the same answers:
  - after each commit, the last min(20, committed) tries' seconds by class. Off: when a non-productive class holds at
    least 50% of them, the shift is on for that class. On: when its class falls below 30%, it is off, and the same
    commit is then looked at again as from off;
  - `share(ρ0)` is ρ0 when off and max(ρ0, min(2ρ0, 0.5)) when on. **Beyond section C:** the max keeps K2's ρ0 = 1
    from being lowered to 0.5;
  - the cell rule: a cell (its current id, cells.json's) is named when its last two committed tries are both model
    tries, neither productive, in the same stop class. A free try in the cell clears it, so the rule cannot hold a
    cell to free tries.
  - free's port (`free::Shift`) is given `share` and `free_cell` over the live table.
- **Gates as run.**
  - C1: `fmi-checks`' `stall-report` over `/var/tmp/fast-mi/e012-archives/kitchen` (read only) and E012's
    `results/kitchen/report.json`: the rows over the 53 tries sum to 1,658.1 s, the rest row is at most 1% (signed and
    absolute), every try's rows sum to its `wall` in report.json, and the novelty check holds on every line. Reported:
    the baseline table by class and by row, by stop, the last-20 trace, where the shift would have fired and which
    cells the cell rule would have named.
  - C2: unit tests on a generated archive (the archive tests' way): each outcome kind's rows and classes, the stop
    table, the shift's threshold and hysteresis, the cell rule, and `share` and `free_cell` after each commit equal to
    the same from the archive opened again at that commit (with free's kind rule, the same kind sequence).
  - C3 at K1's end; before it, a driver run with a scripted step on a generated archive: the stalls.json the driver
    wrote equals `fmi explore stalls`'s over the same files, with `FMI_STALLS` unset nothing is written.

## Amendment 2026-11-03 (2): E014 merged, one code path for both games, the interactive demo after the kitchen

Written and committed before any further code or run of E013. The sections above stay as written: their predictions
are not edited, and their planned measures are the ones scored. Where this amendment changes a setting, the change is
reported with the result.

### What was merged

e014 (head dfacf42) into e013 (merge 31e2d25). Each piece is general and behind a switch that is off by default:

- the mechanic inventory (fmi-checks `inventory`);
- the extraction from a game's settings (`scripts/dump-scripts.sh`; unset, Monkey Island's extraction byte for byte);
- engine patch `0006-e014-scumm-speedrun-handler.patch`: the decision point without verb ids (`SPEEDRUN_IDLE=handler`)
  and the end's script site (`SPEEDRUN_END_SITE=1`);
- the host and the reader for another game id (`FMI_GAME_ID`, `FMI_GAME_DIR`, `FMI_GAME_SCRIPTS`);
- the game's own end as a goal (`FMI_GOAL_END=1`, goal `end = 1`, `fmi guide map --goal end=1 --start-boot`);
- the reader and explorer switches of E014's assumptions table, rows 9-18 (`FMI_READ_EVENTS`, `FMI_EGO_VALUES`,
  `FMI_MERGE_DEDUP`, `FMI_READ_CUT`, `FMI_STAG_SWEEP`, `FMI_VERBS_VAR_ON`, `FMI_NOVEL_VERBS`, `FMI_NOVEL_GUIDE`).

### (a) Component `input` is met by E014's patch 0006

**What 0006-e014 gives.** Its rule (`SPEEDRUN_IDLE=handler`) is A's decision point without verb ids:

- the gate is open, the handler (`VAR_VERB_SCRIPT`) is set and not running, and the old rule's other blockers hold;
- the kind is always "sentence", and there is no menu stop.

So at every decision point the affordance step enumerates E012's sentence set: each verb slot alone (the whole dialog
set), each verb then each target, each target alone, and each walk box. That set contains both old kinds' sets, so it
is A's enumeration without kinds. The offer's points already come from the engine's own hit test (`AtMany`), so the
report's boxes are not needed for it. `FMI_INPUT` is not built.

**The paused e013-input work.**

- The fmi-core types: `~/.cache/fast-mi-e013/paused/e013-input-wip.patch` (273 lines).
- The engine side, uncommitted in `/var/tmp/fast-mi/svm-e013-input` (15 files, +1,294 lines over patch 0005). It is
  copied to `~/.cache/fast-mi-e013/paused/e013-input-engine-wip.patch` (sha1 ac8dcc7d).
- It holds the input report (`Input` 0x96), the input log (`InputLog` 0x97), the digest (`Digest` 0x98) and the
  engine rule with its chain tracking (`SPEEDRUN_IDLE=engine`). It builds; no gate has run on it.

**Still needed from it.** Both parts are general: they read only the engine's structures and name no game number.

1. **The input log** (`SPEEDRUN_INPUT=1`): what became of each injected input (`dropped: gate`, `zone` or `handler`;
   `ignored`; `taken`), with the handler run's reads and writes. B's causes on a route error and its trial verdicts
   come from it. 0006-e014 records nothing of an input's path, so `refuse` cannot be built without the log.
2. **The digest:** B's wait reuse (trials with equal states share one wait), and gate B4.

These two go into patch `0007-scumm-speedrun-input.patch`, on top of 0001-0005 and 0006-e014, taken from the paused
engine work. The fmi-core and fmi-engine side gets `InputRecord`, `Verdict`, `Touch`, `InputLog`, `Digest`,
`Engine::input_log` and `Engine::digest`. `SPEEDRUN_IDLE` keeps 0006-e014's parsing. Refuse's M1 runs on 0007's
engine.

**Not needed now.**

- the input report (verb boxes, zones, the chain);
- `FMI_INPUT`;
- the engine rule and the old rule's shadow.

**Needed only if A3 fails: the engine rule's chain clause.** The handler rule keeps the old rule's blockers and drops
its dialog exception. So on Monkey Island the handler rule blocks two kinds of dialog frame where the old rule gave a
dialog decision point:

- a dialog shown while another script's talker speaks (clause (i));
- a dialog shown while a parked sentence or object script is live (clause (ii)).

A3 is run on the handler rule before K1 (below). If it shows the handler rule missing decision points where the old
kind was right, the engine rule's clause 3 is ported from the paused work into 0007 as `SPEEDRUN_IDLE=engine`. Clause
3 says that only the player's chain blocks, and parked units do not. The port needs a dated amendment before its code.
Under rule (2) below, the rule chosen is on for both games.

**A's gates now.**

- **A1:** as written, for 0006-e014 and 0007.
- **A2:** not run. The report is not built.
- **A3:** on `SPEEDRUN_IDLE=handler`. The same movies as written are replayed twice, once with the switch unset and
  once with `handler`, stopping at every frame and reading the status's idle kind or blocker. The rule only reads, so
  the two replays' checkpoints must be equal. The gate: at every frame where the old rule was idle and its
  enumeration's inputs were taken (E012's kept outcomes, or B's verdicts once 0007 exists), the handler rule is idle
  too. Frames where only one rule is idle are reported with the blocker.
- **A4:** with `SPEEDRUN_IDLE=handler` and the sentence enumeration, in place of `FMI_INPUT=1`.
- **A5:** from the input log's writes.

### (b) The PI's rules for this work

The PI, 2026-11-03:

1. **No manual porting per game.** Every mechanism comes from the engine's or the scripts' own mechanics, never from
   numbers, names or rules read by hand from a game (D9).
2. **One code path for both games.** A fix counts only if it is switched on for Monkey Island too and Monkey Island's
   checks still pass with it on: every entry verifies from boot, and the kitchen is still reached.
3. **A mechanic only the demo uses** is recorded in E014's assumptions table
   (`experiments/E014-fate-demo/results/assumptions.md`), not ported.
4. **Everything that runs is Rust**, with C++ only in the engine patches. Python is only the frozen reference. There
   is no LLM in the running system.

**What follows for E013.**

- **"Every switch on" is one set for both games:**
  - E012's switches, as in its kitchen rerun;
  - E013's: `SPEEDRUN_INPUT=1 FMI_REFUSE=1 FMI_STALLS=1 FMI_FREE=0.25 FMI_FREE_SHIFT=1`;
  - E014's mechanisms: `SPEEDRUN_IDLE=handler` (or `engine`, per (a)), `SPEEDRUN_END_SITE=1`, `FMI_GOAL_END=1`,
    `FMI_READ_EVENTS=1` and play/env.sh's `E14P` (`FMI_EGO_VALUES=1 FMI_MERGE_DEDUP=1 FMI_READ_CUT=1
    FMI_STAG_SWEEP=1 FMI_VERBS_VAR_ON=1 FMI_NOVEL_VERBS=1 FMI_NOVEL_GUIDE=1`).
- **Only the game and its goal differ between the runs.** They are the run's inputs, not mechanisms:
  - the game's files and extraction (`FMI_GAME_ID`, `FMI_GAME_DIR`, `FMI_GAME_SCRIPTS`: unset for Monkey Island,
    play/env.sh's for the demo);
  - the stage, the goal (`owner:566 = 1`; `end = 1`) and its guide;
  - the rooms (E012's ten; the files').

  With Monkey Island's goal, a run that ends at a script's restart or quit gives an end entry, never a goal entry. K1
  reports any such entry.
- **refuse, stall and free are built to rule (2).** None adds a path for one game only. A break that only one game
  shows is a row of the assumptions table.
- **Gates added:**
  - **G0** (everything off) runs on the merged build with patches 0001-0005, 0006-e014 and 0007. It is G0 as written
    plus E014's checks of Monkey Island: the kitchen golden reading (packed sha1 279b7a24) and the cargo tests.
  - **G-on** runs with the whole set above on. On E012's B3 15 tries, every new entry verifies from boot with 0
    differences. The kitchen golden reading is made again with E014's reader switches on and reported: equal, or its
    differences named.
  - **K1** is rule (2)'s check that the kitchen is still reached.
- **Order of the builders.** Each component branch merges e013's head before its next code. 0007 comes first, and
  `refuse` needs it. `stall` and `free` go on from their paused work and do not need 0007.

### (c) The runs: K1, then the interactive demo, then K2

**K1.** Its settings change from "Runs" in two ways:

- `SPEEDRUN_IDLE=handler` replaces `engine`, and `FMI_INPUT` is dropped;
- E014's mechanism switches are on.

Its predictions above stand and are scored as written. Added for what is new:

| | predicted |
|---|---|
| A3 on the handler rule | the two rules agree (idle or not) on at least 95% of the chain's 15,688 frames; the handler rule is idle at 26-29 of the frames of E012's 29 dialog offers (those it misses are under another script's talker); the gate passes: 65% |
| the kitchen golden reading with E014's reader switches on | differs from 279b7a24 (`FMI_MERGE_DEDUP` drops repeated list entries): 70%; `FMI_READ_CUT` cuts no run in the kitchen's scopes |
| K1, end entries | 0 |

**F, the interactive demo again,** after K1 and with no demo-specific code. It runs on the build and engine that K1
ran, unchanged.

- **Settings:** E014 amendment (2)'s run:
  - a new archive `/var/tmp/fast-mi/e013-archives/play`;
  - stage `play`, goal `end = 1`, the guide from `fmi guide map --goal end=1 --start-boot`;
  - the rooms from the files, b = 2;
  - E009's switches, play when ready, commit as finished;
  - the falsifier at 80 committed tries, seed 1;
  - ON3 and the set of (b), with play/env.sh's game settings.
- **Reported:** E013's per-run list ("Runs"), plus E014's:
  - how far each stage of the chain got;
  - the assumptions table's new rows;
  - the best chain.
- **Predictions** (before any code or run of F):

| | predicted |
|---|---|
| how F stops | the falsifier at 80 committed tries: 55%; E009's stagnation before it: 35%; the goal: 10% |
| tries | 25-80 committed tries (E014's scratch archives: at most 7 each) |
| rooms entered, of the 11 in the files | 2-5, most likely 3, counting the root's room 49 (every entry of E014's scratch archives was in room 49) |
| the end | not reached (reached and verified from boot with the system's own inputs: 10%) |
| free tries | 25-45% of the tries; at least 30% of them keep a novel entry |
| the stall table's top causes | 1. interface, no yield (taken trials that keep nothing): 25-50% of the seconds; 2. model-blind or no plan (the reading's cut runs leave the model few targets; E014: every planned attempt went to var:52 = 0): 15-35%; 3. refused input (Monkey Island's executor profile fails on this game's sentences, and B ends those windows): 5-20%; productive 10-35%; the rest row at most 1% |
| longest stretch with no new atom or room | 5-20 committed tries |
| new rows of the assumptions table | 1-4, each recorded and not fixed during F |

- **Stop rule:**
  - F stops at the goal, at E009's stop rules (stagnation over T and rooms, an exhausted archive) or at the
    falsifier, 80 committed tries. It runs once.
  - A goal entry is verified from boot on the lean engine. A goal chain with no player input is reported as the game
    playing itself, not as reached.
  - Exactness comes first, as in the stop rule above.
  - A break F meets is recorded as a row of the assumptions table and is not applied to F.
  - A fix for it comes only by a new dated amendment, and it counts only under rule (2).
  - When F stops, it is written up and scored against these predictions and E014's success threshold. E014's plan
    gets its result section.

**K2** runs after F, as written.

### (d) Part I stays on hold until the demo is finished

The demo is finished when F's write-up is done and the PI decides what follows. Until then P does not run. P's
dated amendment with the projection is still written before it. E013's success threshold is scored as written: while
P has not run, the result is at most "partly".

## Amendment 2026-11-03, component `stall` (2): E014's records, free's record and port, rule (2)

Written after e013 (31da643) was merged into e013-stall, and before the code it changes. Stall's amendment above stands;
this adds what the merge and free's posted record bring.

- **E014's end entries.** With `FMI_GOAL_END=1`, a run that ends at a script's restart or quit gives an entry of how
  "end" (`mech::end_extra`), made by the wait, the attempt or an affordance step. The attempt's own last checkpoint is
  also of how "end"; only the extras' items carry a "novel" list. Such an extra belongs to the part whose dead move the
  same end made (result.json "dead": the move at the same frame with a game end, its "how": wait, attempt, afford or
  free). An afford end belongs to the step whose kept items come before it (a try runs at most one affordance step).
  With no such dead move it is the attempt's, as before.
- **free's record**, as free posted it in COORD.md: result.json "free" holds "seconds" (the whole free step), "delays"
  {"seconds"}, and "at", a list of offer points {"at": entry | stop | kept, "level", "seconds" (the point's offers
  without its delays), "step": its affordance record, with B's "verdict_s" once merged}. The rows are ("free", "AT,
  level L") per point, split by verdict as above; ("free", "delays"); and ("free", "other"), the step's seconds less
  the points' and the delays'. The shape stall's code guessed before free posted (a "points" list, "wall") is dropped.
- **free's port.** free asks for the two rules through `free::Shift`, with the open archive as argument.
  `stall::ArchivePort` answers `share(&Archive, ρ0)` and `free_cell(&Archive, cell)` from a table it keeps over the
  archive's log lines, caught up at each call with the live table's own push: at every commit it equals the table from
  the files, also after a reopen. The integration wires it (`impl free::Shift for stall::ArchivePort`,
  `free::set_shift`). The driver keeps its live table only with `FMI_STALLS=1`.
- **Rule (2) for stall.** Stall changes no run: it reads files after each commit. Its check on Monkey Island is E012's
  smoke run (4 jobs from the boot, the kitchen's settings, b = 1), run twice on one build, with and without
  `FMI_STALLS=1 FMI_FREE_SHIFT=1`, in two switch sets: E012's switches on (E012's engine), and E012's with E014's
  mechanism switches of amendment (2) (b) (E014's engine; the switches of input, refuse and free are not on this
  branch). In each set the two runs give the same tries (`fmi check tries`), job files and archive rows, timing aside;
  every entry verifies from boot; and the run's stalls.json equals stall-report's table over the same files (C3's
  check on a real run). Reported, not a gate: the table over E014's scratch archives on the interactive demo (read
  only), each try's rows summing to its wall.

## Amendment 2026-11-03, component `stall` (3): free's record as committed

Written before the code it changes. free replaced the record shape it had posted (COORD.md, 20:15Z; e013-free ea0e920),
after amendment (2) above was written from the earlier one. In result.json "free", "points" is now the list of offer
points {"where": entry | stop | kept, "frame", "level", "seconds" (the point's offers without its delays), "record"
(its affordance record, with B's "verdict_s" once merged), "delays"}; "n_points" is their count; "seconds" is the whole
free step, and "delays" {"seconds"} the delays'. The rows are amendment (2)'s, read from "points", "where" and "record"
in place of "at", "at" and "step". free's own end entries (`FMI_GOAL_END=1`) are of how "end" with a dead move of how
"free": by amendment (2)'s rule they are the free step's.

## Amendment 2026-11-03 (3): how B reads a try, after M1

Written after M1 (refuse's first step) and before any code of B. The sections above stay as written: no prediction is
edited, no row of B's table is changed, and the planned measures are the ones scored.

### M1 (`results/refuse/m1.jsonl`, `m1-summary.json`; `refuse-check m1` on the E012 kitchen archive, patch 0007)

- The 16 bound windows' first tries, played again with the input log, each give their recorded message.
- **The 12 "verb key" windows.** In all 12 the key was dropped at the gate. In all 12 the parent's gate was open at
  the key's frame: the try's own approach click (taken: it started a walk) led to a closed gate before the key.
  M1's prediction (the key does not reach the handler as a click on the verb) holds, by the first of its three ways.
- **The 4 "never came on screen" windows.** Each issued 3-4 approach clicks, all taken (walks). M1's prediction that
  they give no engine input fails.

### How B reads a failed try

1. **The kind of failure** comes from the executor. Either it could not form the plan's input (an approach, a point,
   an object, an actor, a verb or an arrow not found: the table's last row), or a check after an input it issued
   failed. In the second case the row is chosen by the log record of the checked input.
2. **A drop at the gate.** It is E010's case, where B does nothing, when the parent's gate is shut at that frame. When
   the parent's gate is open there, the gate was shut by the try's own earlier inputs. The try is then read as
   "taken, and the executor's check after it failed", over all of its records: the union of their reads.
3. **A change** is the parent's value at a frame against its value at the refused try's fork frame. Both are the
   parent's, with no input. So an input register that the try's own click set is never compared with the parent's.
4. **The values watched.**
   - A record's reads map to the parent's dump and status:
     - variables, bits, owners, states and classes to themselves;
     - an actor read to that actor's entry in the dump;
     - the object or actor at a point to the room objects, the actors and the camera;
     - a random draw to the RNG state;
     - a script query to the live slots;
     - the gate and the cursor to the status.
   - "The input report" (not built, amendment (2)) is read as the status's gate, cursor, cutscene, room and screen
     layout, and the dump's verb slots. VAR_VERB_SCRIPT is a read of every record.
   - The executor's check names its own fields. The verb-key check names the profile's selected-verb variable and the
     verb slots. Its other checks, and every input it could not form, name the dump's interface part: the room
     objects' boxes and states, the actors, the camera, the verbs and the inventory.
5. **One try at a time.** With `FMI_REFUSE=1` a ready window runs its tries one at a time. E010 runs up to its workers
   in parallel after the first failure. This way each try's cause is known before the parent forks again. The kept
   fork is the parent at the last refused try's fork frame. When the window's frames end with no change since that
   try, the window ends there, with "refused input: <cause>".
6. **What patch 0007 does not log:** the reads of scripts other than the handler's run in its call (a walk's arrival,
   a cutscene). They enter only through the watched parts of the dump. B1 shows a read that is missing.

### A deviation already in patch 0007

Patch 0007 (committed before this amendment) decides "ignored" as "changed nothing outside its own locals", not
"wrote nothing" (plan.md A.2):

- a write of the value already held counts as no write;
- a random draw counts as a change;
- a slot the run started that is still alive counts as a change.

This is reported with the results.

## Integration, as far as it ran (2026-11-04, 00:55): stopped before the kitchen rerun

The integration merged e013-refuse, e013-stall and e013-free (with E014's pieces already in) and built
/var/tmp/fast-mi/target-e013. The gates that ran all passed (results/integration/summary.txt): the Rust tests (172
passed, 0 failed), E011's G2-G5 on the engine with every switch off, on, and with patch 0007's input log (the kitchen's
test (a) still 488 frames, 36 inputs, 0 differences from boot), the kitchen golden (G6) and G6 with --prep, and G10's
play fixtures (501 of 501 children equal, off and on) and its lama clause with E012 off. Not run: the rest of G7, G9
and A3 on, and the kitchen rerun (K1). The PI chose to go straight to the demo (rule (c) of amendment (2)), since the
gates that ran show Monkey Island unchanged with the switches off and working with them on; K1 measured improvement on a
goal already reached. The demo runs next on the cloud VM; the independent check after it covers this integration.
