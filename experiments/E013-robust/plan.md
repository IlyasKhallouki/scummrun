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
