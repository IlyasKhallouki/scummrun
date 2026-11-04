# E014: The Fate of Atlantis demo from its files alone, with the system as it is

Date planned: 2026-11-03
Research question it serves: RQ1 (completion from the game files alone), RQ4 (what carries over to another game)

## Question

Every mechanism of the system so far (E003-E012) was built and checked on one game, The Secret of Monkey Island (DOS
CD). With no knowledge of another game as an input, how far does the current system get on it, and can it reach that
game's own end from boot, verified from boot?

The game: the 1993 talkie demo of Indiana Jones and the Fate of Atlantis (ScummVM id `atlantis`, "Demo/DOS/English",
SCUMM v5), freely distributed by ScummVM. Its files are read only at `/var/tmp/fast-mi/games/fate-cd-demo` (FATE.000,
FATE.001, MONSTER.SOU). From its index: 3 rooms (4 named), 72 global scripts, 41 room scripts, 34 objects with verb
handlers. Nothing else about the game is used.

The PI, 2026-11-03: "do the Fate of Atlantis before the kitchen. I'm curious what would happen if we try it now." And
earlier: "What if we move to another game and it's another set of problems?"

## Baseline

None for this game. For scale, E012's kitchen rerun on Monkey Island (`experiments/E012-game-mechanics/results/kitchen`):
the goal reached from the boot at the 53rd try, 1,658 s of try wall (27.6 min; 21.5 min of driver wall), 151 entries,
every switch of E012 on, b = 2.

## What is known before the run (from our own code, read before this plan)

Places in the system that name Monkey Island, found by reading the code. None has been run on the demo.

| what | where |
|---|---|
| the extraction picks the release by its file names (MONKEY.000, MONKEY1.000, 000.LFL); scummrp's game table names each game's index file | `scripts/dump-scripts.sh`, ScummTR's `scummrp.cpp` game table |
| the reader's game data is `data/scripts-dos-cd` under the repository root | scumm-ir `game.rs`, scumm-reader `domains.rs`, `memo.rs`, fmi-reading `cache.rs` |
| the engine's target is `monkey-cd`, game id `monkey`, files in `game/dos-cd` | fmi-engine `host.rs`, fmi-play `json_engine.rs` |
| the bridge's decision point needs a visible verb 2-10 (sentence) or 120-128 (dialog), verb ids read by hand from MI's scripts | `speedrun_state.cpp` (patch 0001), `speedrun_control.cpp` |
| the window search's executor carries a profile read by hand: the selected-verb variable 107, the walk verb 11, inventory slots 200-207 and variable 133, arrows 208/209, verbless rooms, variables 394 and 118, a 144-line main screen, dialog verbs 120-128 | fmi-play `executor.rs`, `window.rs` |
| a goal is a set of facts (key = value); the guide regresses from facts' write sites | fmi-explore `guide.rs`, `archive.rs` |
| the game's end (restart, quit) marks a move dead (E012) | fmi-explore `mech.rs`, `archive.rs` |
| the guides in use are E009's copies for MI's stages | `~/.cache/fast-mi-e009/guides` |
| the Python reference (`explore.py`, svm-w3b) is configured for MI's release | E008, `src/speedrun` |

The reader's model of the scripts (scumm-ir, scumm-machine, scumm-reader) cites ScummVM's v5 engine, not MI's
scripts, but it was built and checked on MI's scripts only. Its own table says which ops it models
(`machine/src/heads.rs`): any other op ends a path as "unmodelled".

## Method

The demo is taken in order, from its files to its end. Each step that breaks is fixed generally where it can be, and
recorded; nothing is fixed by reading the demo's scripts or numbers by hand.

### (a) The mechanic inventory, before any run

A Rust tool (`fmi-checks`' `inventory`) loads a game's extraction with scumm-ir, as the reader loads it, and lists:

- every opcode byte of every script, by ScummVM's v5 opcode table (`script_v5.cpp`'s `OPCODE` lines, read by the tool
  from the engine's source), with its count and the units that use it;
- every statement head the IR parses (ops, starts, getters and their sub-ops), and for each the machine's class from
  its own table (`heads.rs`: write, yield, wait, end, deferred, none, or unmodelled) and whether the reader evaluates
  the getter;
- the statements the IR could not parse, and the IR's notes;
- the sites of `systemOps` with their sub-op (restart, pause, quit).

It runs on MI's extraction and on the demo's. The table is the demo's column against MI's: what the demo uses that
MI does not (unmet by every check so far), and what both use but the machine leaves unmodelled. The engine side is
listed from ScummVM's source: the branches the engine takes for this game id (`GID_INDY4`), and the bridge's own
constants. The list is written to `results/inventory/` before any run of the bridge, the harness or the explorer on
the demo.

### (b) The extraction

The demo's resources are extracted for the reader by the code that extracted MI's (`scripts/dump-scripts.sh`,
`scripts/dump_scripts.py`: scummrp and descumm), generalized where it breaks, into
`/var/tmp/fast-mi/e014-root/` (`build/blocks-<release>`, `data/scripts-<release>`). Nothing extracted is committed:
no file, no text, no object name. With the generalization's options unset, MI's extraction is byte for byte as before
(checked by `cmp` of index.json on the DOS CD release).

### (c) The system pointed at the game

The bridge, the harness and the explorer are pointed at the demo's game id and files. Every MI assumption found on
the way is recorded in one table, `results/assumptions.md`:

| what | where | why it breaks | the general replacement | fixed? |
|---|---|---|---|---|

- A fix replaces a number read from MI by the engine's or the scripts' own mechanics, behind a switch where it touches
  shared code (`FMI_GAME*` for the game's files and ids, `SPEEDRUN_IDLE=verbs` for the bridge's rule, others as
  found). Each switch's name, default and meaning is recorded in the table.
- With every E014 switch unset, MI's gates stay as they are: E012's G0 (B3's 15 tries equal round 2's), the kitchen
  golden reading (packed sha1 279b7a24), E011's G2-G5 on the engine, and the cargo tests. They run on the final build.
- E013 builds the engine's own input state in parallel (patch 0006, `SPEEDRUN_IDLE=engine`, branches e013-*). Where
  E014 needs a decision point without MI's verb ids, it builds the smallest general version under its own switch,
  says so in the table, and leaves the full version to E013 (the two are merged later).
- A break that needs a change of design (a new model of a mechanism, not a replacement of a number) is written up as a
  blocker with what it would take, and the work goes on with the breaks that do not depend on it.

### (d) The goal: the demo's own end

Derived from the scripts and the engine, never from knowing the demo:

- **The end sites** are the `systemOps` sites of the scripts whose sub-op is restart or quit (the inventory lists
  them). Restart clears the variables and runs the boot again (`scumm.cpp` `restart()`); quit ends the engine. Both
  are the game's own end.
- **Reached** means a run's end report (patch 0005's "restart", the bridge's "quit") comes from a script's
  `systemOps`, not from a player input: the end record names the slot that ran the opcode (a small observation-only
  addition to the end report, engine side, behind its switch). No input of the harness is ever a restart or quit
  key; a run that the engine ends for any other reason is not the goal.
- **In the explorer**, a stage goal `end` (switch `FMI_GOAL_END=1`): an entry whose run ended at an end site is a goal
  entry instead of a dead one (E012's dead mark stays for every other end). The guide regresses from the end sites as
  it does from a fact's write sites: the needs of each site (the tests on the way to it in its unit, how its unit
  starts) at depth 1, and on from there.
- **Verify** plays the goal entry's movie from boot in a fresh lean engine and requires every checkpoint equal and the
  same end, from the same site, at the same frame.

If the scripts hold no restart or quit site, the demo's end is not defined by this rule; E014 says so and the end
question is "no".

### (e) The explorer on the demo

- `fmi explore init` from the boot into a new archive `/var/tmp/fast-mi/e014-archives/demo`, stage `demo`, goal
  `end`, the rooms from the files (`--allowed files`), the guide from (d), b = 2, E009's switches (`--idle-frames 1600
  --rotate --evict-by-size --reuse-nonfacts`), play when ready and commit as finished, the falsifier `--stop-jobs 80`.
- `fmi explore run --workers 2 --search fd --tail` with ON3 (round 2's switches, the no-plan proofs, deepening) and
  every E012 switch on (`integrate/env.sh`'s E12ON), plus E014's switches.
- Seed 1, the laptop, nice 10, under the guard; the heavy lock as the driver takes it.
- Any goal entry is verified from boot on the lean engine, and on svm-w3b through E008's `explore.py verify` if the
  Python reference can run this game without a change to it (it is frozen); if it cannot, that is stated.
- Reported: the inventory; the assumptions table; per stage of the chain (boot, reading, plan, play, wait, affordance
  step, verify) whether it ran on the demo and what stopped it; the run's tries, entries, rooms, cells, what each try
  ended on, the wall by part; the best chain.

## Prediction (before any code)

| | predicted |
|---|---|
| inventory | the demo uses 0-6 opcodes or sub-ops MI never uses; 1-3 of them on paths the explorer meets |
| end sites | 1-3 systemOps restart or quit sites in the demo's scripts |
| assumptions table | 12-20 rows; 9-16 fixed generally behind switches; 1-3 written up as blockers |
| first boot | the bridge's decision point never holds on the demo with MI's verb ids (the boot runs to its end or its frame bound) |
| the extraction | needs only the release's names (scummrp's index name, the output folders); descumm reads every script |
| the reader | reads the demo's rooms; its input rule (F1c) finds no plain input script or no hover in the demo's scripts, so few or no sentence records |
| the executor | its hand-read profile fails on the demo's sentences ("verb key did not select ..." or no point), so the window searches run empty and the affordance step carries the exploration |
| the end | reached and verified from boot: 35%. If reached: in 5-40 tries, 0.1-1.0 h of try wall, 20-300 entries |
| if not reached | the explorer runs on the demo and enters every room of the files (3 or 4) within 80 tries ("partly") |
| svm-w3b | the Python reference cannot run the demo without a change, so the verify is on the lean engine only |
| MI's gates with every E014 switch unset | pass unchanged |

## Success threshold

- **Yes:** the demo's end (d) is reached from the boot by the explorer, with no step by hand, and its goal entry
  verifies from boot with 0 differences.
- **Partly:** the explorer runs on the demo and makes progress (entries past the root: a new room or a new tested
  atom), and the blockers to the end are named.
- **No:** the explorer cannot run on the demo (no root, no reading or no try completes), or an exactness check fails
  and is not explained.

## Stop rule

- **Exactness first.** An entry that differs on verify from boot stops everything until it is explained (A0, Lemma C).
- **Breaks.** Each is fixed generally or written up as a blocker (c). The run (e) starts once the boot, a reading and
  one try complete on the demo.
- **The run** stops at the goal, at E009's stop rules (stagnation over T and rooms, an exhausted archive), or at the
  falsifier, 80 committed tries. It runs once; a fix found during it is recorded and not applied to it.
- **MI first.** If a gate of MI fails with every E014 switch unset, the change that broke it is reverted or put behind
  its switch before anything else.
- When a stop rule fires, the write-up gives the inventory, the assumptions table, how far each stage of the chain
  got, the blockers and the best chain.
- **Watchdogs** are E012's: the harness's, the window search's and the reader's limits, the guard's 3 GB and the disk
  floors. They are engineering, reported where they fire.

## Result so far (2026-11-03): this demo plays itself; stopped

The explorer's archive `a5` reached the stage goal `end = 1` in its first try with no player input: boot (frame 199,
room 4), two waits (frames 617 and 642, room 62), then the game's own restart at frame 1,911, at room 62's local
script 206, offset 0x0AEF. That script is a cutscene from its first statement and ends with `systemOps(1)`
(restart). Room 62's local script 214 starts it directly after starting the music and two other scripts, with no
input on the way. So this release, the 1993 talkie demo, is a self-running showcase: its end is reached by waiting,
and it cannot test finishing a game. The run was stopped here by the PI's session, not by a stop rule; no "reached"
is claimed. What E014 built is general and stays (each behind its switch): the mechanic inventory, the extraction for
another SCUMM v5 game, engine patch 0006's decision point without verb ids, the host and the reader for another game
id, the game's own end as a goal, and the reader with no modelled input. The next game for this question is an
interactive release.

## Amendment 2026-11-03 (2): the interactive demo

Written and committed before any run of the bridge, the harness or the explorer on this game, and before any code
it changes.

### Why the game changes

The talkie demo plays itself to its restart with no input (the result above), so its end says nothing about finishing
a game. The PI, 2026-11-03, wants the system to finish a second game end to end before Part I resumes, asked "What if
we move to another game and it's another set of problems?", and that onboarding a game should not be manual work.
E014 goes on, on the interactive release.

### The game

"Indiana Jones and the Fate of Atlantis, Interactive Demonstration v2" (ScummVM id `atlantis`, Demo/DOS/English,
SCUMM v5). Its files are read only at `/var/tmp/fast-mi/games/fate-play-demo` (PLAYFATE.000, PLAYFATE.001). Its
extraction: `scripts/dump-scripts.sh` with `SCUMMRP_GAME=atlantis INDEX_NAME=PLAYFATE.000 RELEASE=fate-play-demo
OUT_ROOT=/var/tmp/fast-mi/e014-root GAME_DIR=/var/tmp/fast-mi/games/fate-play-demo`, in
`/var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo`: 11 rooms in the files, 93 global scripts, 78 local, 224
objects (66 with verb code, 227 verb entry points), 417 files, 0 errors. Nothing extracted is committed: no file, no
text, no object name; units are named by room, object and script numbers.

### Its inventory (`results/inventory/fate-play-demo/`, README section "The second game")

- Against MI it adds: `soundKludge` (275 statements in 39 units), `saveString`/`loadString` (1, 2), `saveLoad` (2 in
  1 unit), `setRoomShadow` (3), `debug` (1), all without a rule in the machine (a path ends there, "unmodelled");
  `getActorScale` (9, an unknown value); `drawObject` with `setImage` (13, modelled).
- The IR parses every statement (the talkie demo's speech codes do not occur).
- End sites: two `systemOps` restarts, global script 57 at 0D0E and object 582's verb code at 0067; no quit. The
  guide regresses from these sites by itself (`fmi guide map --goal end=1 --start-boot`), as for the talkie demo.
- For the record only, never an input: the PI's session read that the object's site is in a walk-to handler under a
  test of the object's state, and that global 57's site follows an endless loop.

### What runs, in order

1. **The table first.** `results/assumptions.md` holds the assumptions table of method (c): the rows the talkie demo
   fixed (from E014's commits), then a row for each break on this game, in the order met. The extraction's file
   headers and index title name MI when `GAME_TITLE` is unset: that is this game's first row.
2. **Breaks.** `demo/env.sh` (or a sibling `play/env.sh`) points at this game. The boot through the bridge with
   E014's switches (`SPEEDRUN_IDLE=handler`, the game id, the end as the stage goal), the guide, then scratch
   archives of 1-5 tries with every E012 switch on, ON3 and E14ON. Each break is fixed generally, behind a switch
   where it touches shared code, gets its row, and the next scratch starts. A break that needs a new model is a
   blocker (method (c)).
3. **The run** (method (e) with this game): once the boot, a reading and one try complete with no open break, one
   run from a new archive `/var/tmp/fast-mi/e014-archives/play`, stage `play`, goal `end = 1`, the rooms from the
   files, b = 2, E009's switches, play when ready, commit as finished, the falsifier at 80 committed tries, seed 1.
4. **Verify** the goal entry from boot on the lean engine, and on svm-w3b through `explore.py verify` if the frozen
   reference runs this game without a change (else stated).
5. **Not a finish:** a goal entry whose chain holds no player input (only boot and waits) is reported as the game
   playing itself, not as reached.

### Prediction for this game (before any run or code)

| | predicted |
|---|---|
| the extraction | the header and the index title are its only break; re-extracted with the game named from its settings, every script body is byte for byte the same |
| first boot (handler rule) | the decision point holds from boot within 3,000 frames |
| the reader | reads the 11 rooms; `soundKludge` ends paths as unmodelled where it is met, and needs a rule (1-3 reader rules in all) |
| the executor | MI's hand-read profile (selected-verb variable, walk verb, inventory slots) fails on this game's plans, so the window searches run empty and the affordance step carries the exploration |
| new rows of the table on this game | 3-8; 2-6 fixed behind switches; 0-2 blockers |
| the end | reached by the system's own inputs and verified from boot: 25%. If reached: in 10-80 tries, median try 10-90 s, 50-400 entries, under 3 h of try wall |
| if not reached | partly: the explorer enters 4-11 of the 11 rooms within 80 tries, with the blockers named |
| the game plays itself to an end site with no player input | 5% |
| svm-w3b | the Python reference cannot run this game without a change; the verify is on the lean engine only |
| MI's gates with every switch unset | unchanged |

### Success threshold and stop rule

As the plan's. **Yes:** the game's own end (an end site of (d)) reached from the boot by the explorer with the system's
own inputs, no step by hand, and its goal entry verified from boot with 0 differences. **Partly:** the explorer runs on
this game and makes progress (entries past the root: a new room or a new tested atom), and the blockers to the end are
named. **No:** the explorer cannot run (no root, no reading or no try completes), or an exactness check fails and is
not explained. The stop rule is the plan's: exactness first; breaks fixed or written up before the run; the run stops
at the goal, at E009's stop rules or at 80 committed tries, runs once, and a fix found during it is recorded and not
applied to it; MI first; watchdogs as E012's.

## Result so far on the interactive demo (2026-11-03): stopped, the explorer stalls; E013 next

Stopped by the PI's session at 19:05, not by a stop rule: no goal is claimed. The assumptions table has rows up to
18; each fix is behind its own switch (the game named from the extraction's settings, several values of VAR_EGO in a
scope, the merge adding a branch's list entries once, the read cut's modes, sentence verbs turned on by a variable
operand, E012's stagnation counted from the selectable cells, T with the interface's and the guide's atoms).

What bound it, from the last archive (`p6`): 7 tries, 4 of them with nothing new; every attempt that planned went to
the same target (var:52 = 0); 2 to 4 selectable cells. The explorer found no plannable progress, and the work had
moved into the reader's internals to make the model see more. That is the case E013 addresses without a game's own
code: a standing model-free share (the interface's inputs, waits and delays, from cells chosen by novelty) and stall
accounting. So E014 continues after E013, on E013's build, with one more rule: a fix counts only if it is switched on
for Monkey Island too and Monkey Island's checks still pass; a mechanic only this demo uses is recorded, not ported.

## Amendment 2026-11-04 (3): the interactive demo on E013's build, every switch on, on the cloud VM

Written and committed before the run, after the laptop check below. The sections above stay as written; their
predictions are not edited. This is E013's run F (E013 amendment (2) (c)), written up in this plan: F's predictions in
E013's plan stand and are scored there as written; the predictions below are this amendment's own.

### Why now

The PI, 2026-11-04: "Honestly, it feels like we should just move on to the demo then." E013's integration stopped
before its kitchen rerun K1 (E013 plan, last section), and the PI approved (2026-11-03) running the demo on the cloud
VM. The stall table over E014's stalled scratch archives (E013 `results/stall/demo-scratch-p5..p7.md`) put 59-61% of
the seconds in planned moves that reached nothing new and 35-37% in interface inputs that changed nothing tracked; with
the interface's atoms counted as progress (rows 16-18 of the assumptions table) 74% of p7's seconds were productive.

### The build and the switches

- **Code.** e014 merged e013 at 2797fb1 (merge 062d639); no code is added for this run and none is demo-specific.
- **Binaries.** E013's build `/var/tmp/fast-mi/target-e013` (cargo found it current with e013's head; `fmi` sha1
  f5b27870, `reader-scope` 135bf06b).
- **Engine.** `/var/tmp/fast-mi/svm-e013` (patches 0001-0005, 0006-e014 and 0007; the tree's diff over 0006-e014 is
  patch 0007 byte for byte; `scummvm` sha1 e9a809b2).
- **Switches.** E013's `ALLON` (E013 amendment (2) (b)), one set for both games:
  - ON3 (round 2's switches, the no-plan proofs, deepening);
  - every E012 switch (`FMI_WAIT=1 FMI_TARGET_PER_KEY=1 FMI_DELAY_FORK=1 FMI_AFFORD=1 FMI_SELECT=progress
    SPEEDRUN_REPORT_RESTART=1 FMI_BOXES=1 FMI_READER_LIB=1`);
  - E014's mechanisms (`SPEEDRUN_IDLE=handler SPEEDRUN_END_SITE=1 FMI_GOAL_END=1 FMI_READ_EVENTS=1 FMI_EGO_VALUES=1
    FMI_MERGE_DEDUP=1 FMI_READ_CUT=1 FMI_STAG_SWEEP=1 FMI_VERBS_VAR_ON=1 FMI_NOVEL_VERBS=1 FMI_NOVEL_GUIDE=1`);
  - E013's (`SPEEDRUN_INPUT=1 FMI_REFUSE=1 FMI_STALLS=1 FMI_FREE=0.25 FMI_FREE_SHIFT=1`): the input log, refused
    inputs with the shared waits, the stall table, the model-free share and its shift.
- **The game's settings** are the run's inputs, not mechanisms: `FMI_GAME_ID=atlantis`, `FMI_GAME_DIR`,
  `FMI_GAME_SCRIPTS` (amendment (2)'s files and extraction). Scripts: `vm/env.sh`, `vm/run.sh`, `vm/vm.sh`.

### The run

- **Settings.** Amendment (2)'s run: a new archive `/var/tmp/fast-mi/e014-archives/play-vm`, stage `play`, goal
  `end = 1`, the guide from `fmi guide map --goal end=1 --start-boot` (the same file as amendment (2)'s, its timing
  aside), the rooms from the files (the guide's reached rooms cut to the files' rooms: 10 of the 11; room 98 is not
  among the guide's reached rooms), E009's switches, play when ready, commit as finished, the falsifier at 80 committed
  tries, seed 1.
- **Changed from E013's F:**
  - the archive's path;
  - b = 12 on the VM, not 2;
  - the heavy lock off (`E008_HEAVY_LOCK=none`);
  - K1 did not run before it.
- **b.** The laptop check measured a try at about 2.5 cores (the reading's three threads, about 2 cores busy, and the
  driver) and at most 0.5 GB of RSS, so the VM's 30 cores give b = 12; its 120 GB do not bind.
- **The VM.** One start of `n2-custom-30-122880`, standard, through main's `scripts/cloud.py` ($1.5346/h; $12.45 left
  under the ledger's stop).
  - **Set up as E009's `vm_stage1.sh`.** The laptop's binaries and engine go to the same paths, with the copied loader
    and libraries; each binary's code section is checked against the laptop's. The demo's files and extraction go to
    the VM's private disk at the same paths, never into git.
  - **The check before the run.** The laptop's 2-try archive with the run's settings and b = 1 (`vm-check2`: 21
    entries, each verified from boot on the laptop, 0 differences) is verified from boot on the VM. The same 2 tries
    then run on the VM at the same paths, and must give the same files as the laptop's (timing aside).
  - **Copies back.** The archive's `log.jsonl` is copied to the laptop's
    `/var/tmp/fast-mi/e014-archives/play-vm/log.jsonl` every 2 minutes, and the archive without readings every 30
    minutes. At the end, every entry is verified from boot on the VM, the archive is copied back without readings, and
    the VM is stopped.
- **Verify.** Any goal entry is verified from boot on the laptop's lean engine (`fmi explore verify`).
  - E008's `explore.py verify` on svm-w3b cannot run this game: the frozen reference's release table
    (`src/speedrun/releases.py`) holds only Monkey Island's targets.
  - A goal chain with no player input is reported as the game playing itself, not as reached.

### What the laptop check showed before this amendment

Three tries with the run's settings and b = 1 (`/var/tmp/fast-mi/e014-archives/vm-check`):

- j00000 took 40.9 s (its reading 40.7 s) and reached var:52 = 0 by the plan.
- j00001 took 15.6 s: from entry 1, the affordance step kept 18 entries (19 new cells, 94 new atoms of T).
- j00002 started from entry 3, the wait's stop at frame 1042 after the opening dialog. Its fresh reading of the
  8-room scope was still running after 29 minutes, in pass 1's event rounds:
  - rounds 1-3: 572 runs in 30 s;
  - round 9: 13 runs in 638 s;
  - round 10: 7 runs in 190 s.

  The reading has no wall bound, only the path and step limits. This is the first break of this run, row 19 of the
  assumptions table: recorded and not fixed for this run.

### Projection and the hours booked

- **Fresh readings.** A model try from a state after the opening dialog may read for 30-90 minutes on the laptop. An
  n2 core runs a thread about 1.5-2 times as fast as the laptop's (an i5-8350U at 58% of its clock), so 15-60 minutes
  on the VM.
- **Free tries** take the newest cached reading of their scope and read nothing fresh.
- **Driver wall.** 80 tries with about 50 fresh readings over 12 workers: about 1-4 h.
- **Hours booked.** About 0.5 h more goes to the setup, the check, the verify and the copies, so with the margin of
  1.5 the start books H = 6 h ($9.21). The ledger counts the hours used once the VM is stopped.
- **The VM's watchdog.** Google stops the VM at H. If the run has not stopped 20 minutes before that, the driver is
  stopped there. The archive's committed tries are copied back and verified, and the write-up says that the VM's
  booked time, not a stop rule, ended the run.

### Stop rule

As E013's F, with one addition for a break that stops the driver.

- **When the run stops:** at the goal, at E009's stop rules (stagnation over T and rooms, an exhausted archive), or at
  the falsifier (80 committed tries). It runs once. Exactness first: an entry that differs on verify from boot stops
  everything until it is explained.
- **A break that stops the driver** (an error on every try, a crash, "repeated failure") is fixed by a dated amendment
  written before its code. The fix is built and tested on the laptop with rule (2) on Monkey Island: the 15 B3 tries'
  entries verify from boot, and the kitchen chain of E012's archive verifies. It is then pushed to the VM, and the run
  resumes from its files.
- **Any other break** is a row of the assumptions table, recorded and not applied to the run.

### Predictions (before the run)

| | predicted |
|---|---|
| how it stops | the falsifier at 80 committed tries: 40%; E009's stagnation: 25%; the VM's booked time first: 25%; the goal: 10% |
| committed tries | 30-80 |
| rooms entered, of the 11 in the files | 2-5, most likely 3 (room 49, the root's, counted) |
| the end | not reached; reached by the system's own inputs and verified from boot: 10% |
| the stall table's top causes | productive 30-70% of the seconds; then interface, no yield 10-35%; model, no yield 5-30%; no plan 0-15%; refused input 0-10%; the rest row at most 1%; the reading's rows (read, in whichever class) at least 50% of the run's seconds |
| free tries | 25-50% of the committed tries; at least 30% of them keep a novel entry |
| longest stretch with no new atom of T or room | 5-20 committed tries |
| new rows of the assumptions table | 1-3, row 19 included |
| entries verified from boot | every entry, 0 differences |
| the VM | 2-6 h used, $3.1-9.2 |

## Not run (2026-11-04, 01:56): the VM's standard start was refused

- **The start.** `python3 scripts/cloud.py start --type n2-custom-30-122880 --hours 6 --standard` (main's) was refused
  by Google before the VM started:
  > Invalid value for field 'resource.provisioningModel': 'STANDARD'. For preemptible, only allowed provisioning_model
  > value is SPOT.
- **Why.** fast-mi-1 was last started as Spot, so its scheduling holds `preemptible = true`. cloud.py's `start` sets
  `--provisioning-model=STANDARD` without `--no-preemptible`.
- **Cost.** Nothing started and the ledger is unchanged: $17.05 committed, $12.45 left.
- **What was not done.** A change of the VM's scheduling outside cloud.py was not made. The run of amendment (3) waits
  for one of two things: a cloud.py whose standard start also clears the preemptible flag, or a Spot start that the PI
  approves. Amendment (3) and its predictions stand for that run.
- **The laptop check, measured further before it was stopped.**
  - j00002's fresh reading from entry 3 had not ended after 50 minutes. It was in pass 1's event round 11, which had
    run for more than 36 minutes.
  - The run's processes peaked at 1.4 GB of RSS in all.
  - The amendment's 15-60 minutes per fresh reading on the VM is then a lower bound for such states.

## Amendment 2026-11-04 (4): a bound on the reading and scopes grown as needed, before their code

Written and committed before any code or run it changes. The sections above stay as written and their predictions are
not edited. Amendment (3)'s run did not start (the section above); its run is replaced by the run below, which is
amendment (3)'s with the two switches of this amendment and the changes listed under "The run". Amendment (3)'s
predictions are reported beside this run for reference; this amendment's are the ones scored.

### Why

Row 19 of the assumptions table: a reading has no wall bound, only the reader's path and step limits. From the state
after the opening dialog (entry 3 of `vm-check`, frame 1042) the 8-room scope's fresh reading had not ended after 50
minutes. Measured for this amendment on the laptop, with the run's build and switches (`fmi read`, the try's
arguments, 3 threads):

| reading | rooms | from | wall |
|---|---|---|---|
| ring(49), j00000 of `vm-check` | 8 | entry 0, the root (frame 992) | 40.5 s |
| {49} | 1 | entry 0 | 39.4 s |
| ring(49), j00002 of `vm-check` | 8 | entry 3 (frame 1042) | not ended after 50 min |
| {49} | 1 | entry 3 | not ended after 7 min (past round 3 at 23 s; still running at this commit) |

So the slow part of a reading at that state is in its event rounds' runs, not in the number of rooms. Monkey Island's
readings on this build (the laptop): E012's kitchen archive, 28 fresh readings of ring scopes, the longest 43.3 s, the
median 5.2 s; E011's kitchen archive, 14, the longest 42.1 s.

The PI's direction for row 19: two general changes, each behind its own switch, each switched on for Monkey Island too
and checked there (rule (2)).

### (a) A bound on the reading (`FMI_READ_BOUND=S`, seconds; unset or 0: off)

- **What is bounded.** The try's own fresh reading of its scope: step 2's reading, the reading again after "not a
  plan", and a free try's reading when its scope has none. The harness's refinements and reach extensions are not
  bounded; they were not met slow.
- **The watchdog.** The reading runs on a thread of its own, and the try waits for it at most S seconds of wall. At S
  it is stopped through the in-process reader's own stop flag, and its scratch folder is removed. The bound needs the
  in-process reader (`FMI_READER_LIB=1`, on in ALLON); with the reader as a child process the driver refuses it. This
  is engineering: a watchdog with a fallback.
- **The value.** S = 300 s, set from the measured readings above: about 7 times the longest reading of either game that
  ended (Monkey Island's 43.3 s, the demo's 40.5 s), so no measured ring reading of Monkey Island comes within a factor
  of 6 of it, while the demo's readings from the state after the opening dialog passed it by a factor of 10 or more.
  Readings of Monkey Island's Part I hubs have not been measured on this build; the value is measured again when they
  are.
- **The fallback.** The try goes on model-free from the same entry, as E013's free try goes on after its reading step
  (E013's amendment for component `free`):
  1. T and the wait's view from a stand-in reading: the newest registered reading of the scope, else the newest
     registered reading of another scope that holds the entry's room. It serves T and the wait only, never a plan.
  2. The wait, the offers at the entry and at the wait's stops, the delays, one more level, the commit.

  The try's kind stays "model" for the cells' counts, and it never marks its entry exhausted. When no reading can stand
  in, the try keeps nothing (it has no T and no domains for the wait) and commits with its reading's record.
- **In the background** (`FMI_READ_BOUND_BG=N`; default 0: none). Up to N readings that reached the bound go on in the
  background in the driver's process instead of being stopped.
  - When one ends, its files go into the reading cache (`FMI_READING_CACHE`) under the reading's key, as any reading's
    do. A later try that asks for the same reading (the same scope, start row and goal: a try from the same entry) gets
    it from the cache.
  - A try that asks for a reading still running in the background waits for that one, within its own bound, and starts
    no second copy.
  - When N are running, a reading that reaches the bound is stopped. The same reading asked for again in the process is
    then bounded at once, with no wait.
- **Records.** result.json "reading" gets "bound": the bound, the seconds waited, whether the reading went on in the
  background or was bounded at once, and the stand-in's path. Its "why" starts with "bound".
- **The stall table** gets a class of its own, "reading bound":
  - the read row of a bounded try (the seconds waited);
  - the class of a bounded try that kept nothing new (its stop class, for the cell rule and the shift).

  A bounded try's other rows are classed as a free try's: productive when they kept something new, else interface, no
  yield; the wait as before. With the switch unset no try has the class.
- **Exactness.** Every entry is still its movie and inputs, verified from boot. A stopped reading gives nothing. A
  background reading is used only once it has ended, from the cache, as any cached reading.
- **Reproducibility.** A wall-clock watchdog depends on the machine and its load. So which readings are bounded, and
  from there the order and content of the tries, can differ between two runs of the same archive (the laptop and the
  VM). The check before the run uses tries whose readings end well within the bound.

### (b) Scopes grown as needed (`FMI_SCOPE_GROW=1`; unset: E009's ring)

- **The scope** of an entry in room R is {R} ∪ G(R), cut to ring(R) (the guide's rule, cut to the allowed rooms). G(R)
  is the archive's set of rooms grown for R, empty at the start. job.json "scope" is the grown scope, and "ring" the
  whole ring.
- **G(R) grows** by a room R' of ring(R) that is not in the scope, in three cases, each a function of the archive's
  files:
  1. **A target needs one.** A model try from room R in which no attempt played: no target planned, and every request
     was dead, deferred or incomplete. The try names the room:
     - the candidates are the guide's atoms not held at the entry and not reached by the scope's reading;
     - an atom needs a room when none of its writers is a global script or a unit of a room in the scope, and one of
       them is a unit of a room of ring(R) outside the scope;
     - the room is that of the shallowest such atom (the guide's depth), ties broken by the most such atoms at that
       depth, then by the lower room number. One room per try.

     A writer's room is its unit's: a room's local, entry or exit script, or an object's room from the extraction's
     index (the game's own data, read by the extraction, never by hand).
  2. **A plan needs one.** An attempt stopped because the engine's state is outside the task's domains, with the engine
     in a room of ring(R) outside the scope.
  3. **The model-free step leaves the room.** An entry kept by the wait, an affordance step or a free try (or a bounded
     try's model-free step) whose room is in ring(R) and outside the scope.
- **Records.** The try writes cases 1 and 2 in result.json "scope_need". At the commit the archive checks the room
  against ring(R) and the scope, adds it with case 3's rooms, and writes the growth in the commit's log line ("grow":
  the room, the rooms added and why). Opening an archive replays these lines, so a reopened archive has the same G.
- **Soundness: a scope is still a set of rooms read exactly.**
  - The reader reads every unit of the scope's rooms and the global scripts, from the entry's exact state, as before.
  - Actions outside the scope are not in the reading: its domains mark them out of the scope, and a plan whose run
    leaves them stops on "outside the task's domains".
  - A smaller scope's reading is a smaller exact model, not an approximate one. What it plans is played on the engine,
    and only the engine's own checkpoints are kept.
  - Readings are registered and reused by their exact scope, so a reading of {49} never stands for {48, 49}; a grown
    scope gets its own reading. The stand-in of (a) serves T and the wait's view only, as in E013's free try.

### Rule (2) on Monkey Island, both switches on (with ALLON, S = 300, N = 0)

- **B3.** The 15 tries (E013's `integrate/b3.sh`, sequence `all` with the two switches added) give the same tries as
  ALLON's sequence (`fmi check tries`), and every new entry verifies from boot with 0 differences. B3's job.json files
  carry their scopes, so (b) does not change them; they check (a)'s path (the reading on its own thread).
- **The kitchen chain.** E012's archive `/var/tmp/fast-mi/e012-archives/kitchen`: its goal entry's chain verifies from
  boot on the new build.
- **The driver.** E012's smoke run (4 jobs from the boot, the kitchen's settings, b = 1) with ALLON and both switches:
  every entry verifies from boot with 0 differences. Its first scope is {33}, not ring(33).
- **The unit tests,** with tests of the bound (a reading that ends, one stopped, one in the background found again,
  one bounded at once), the stand-in's choice, the growth's three cases, the replay of G from the log lines, and the
  stall rows of a bounded try.

### The run

Amendment (3)'s run, with these changes:

- **The build** is e014's head with (a) and (b), in `/var/tmp/fast-mi/target-e014b`; the engine is svm-e013,
  unchanged.
- **The switches** are ALLON, the game's settings and `FMI_READ_BOUND=300 FMI_READ_BOUND_BG=2 FMI_SCOPE_GROW=1`.
- **b = 10.** A try takes about 2.5 cores (amendment (3)); a background reading measured 2.8 cores on the laptop. So 10
  tries and 2 background readings fill the VM's 30 cores. Memory does not bind: a try at most 0.5 GB of RSS, a reading
  at most 0.5 GB, of 120 GB.
- **The archive** is a new one, `/var/tmp/fast-mi/e014-archives/play-vm4`.
- **The check before the run** is amendment (3)'s with this build and these switches: a new laptop archive of 2 tries
  (`vm-check4`, b = 1, every entry verified from boot) is verified from boot on the VM, then run again there and
  compared.
- **Projection.**
  - A bounded model try costs the bound (300 s) plus its model-free step (15-60 s on the laptop's measures); a free try
    costs 15-60 s; a model try whose reading ends costs 15-90 s.
  - 80 committed tries, three quarters of them model tries and most of those bounded: about 80 × 0.75 × 330 + 80 × 0.25
    × 40 ≈ 21,000 s of try wall, so about 35 minutes of driver wall over 10 workers. Allowing for slower model-free
    steps from richer states, 0.5-1.5 h.
  - With 0.5 h for the setup, the check, the verify and the copies, and a margin of 1.5, the start books H = 3 h
    ($4.60 at $1.5346/h; $12.45 left).
- **Stop rule.** Amendment (3)'s: the goal, E009's stop rules, the falsifier at 80 committed tries, and the VM's booked
  time (the driver is stopped 20 minutes before H). It runs once; a break that stops the driver is fixed only by a
  dated amendment and rule (2).

### Predictions (before any code)

| | predicted |
|---|---|
| rule (2) on Monkey Island | B3's 15 tries equal to ALLON's, every new entry verified from boot; the kitchen chain verifies; the smoke run's entries verify; no reading of Monkey Island reaches the bound |
| how the run stops | the falsifier at 80 committed tries: 45%; E009's stagnation: 35%; the VM's booked time: 10%; the goal: 10% |
| committed tries | 40-80 |
| bounded readings | 40-80% of the model tries from states after the opening dialog; none from the root |
| background readings | 0-30% of them end within the run; 0-3 later tries take one from the cache |
| scopes | the first reading of each room's scope is of 1 room; G(49) grows by 1-4 rooms; 80% of the readings are of at most 3 rooms |
| rooms entered, of the 11 in the files | 2-5, most likely 3 (room 49, the root's, counted) |
| the end | not reached; reached by the system's own inputs and verified from boot: 10% |
| the stall table | reading bound 20-50% of the seconds; productive 20-50%; interface, no yield 10-35%; no plan and model-blind together under 15%; the rest row at most 1% |
| free tries | 25-50% of the committed tries; with the bounded tries' steps, 60-90% of the entries come from model-free steps |
| entries verified from boot | every entry, 0 differences |
| the VM | 1.5-3 h used, $2.3-4.6 |

## Amendment 2026-11-04 (5): the VM could not start; the run on the laptop; the reader's stop fixed

Written and committed before the run. Amendment (4) and its predictions stand; what changes is listed here.

### The VM did not start

`python3 scripts/cloud.py start --type n2-custom-30-122880 --hours 3 --standard` (main's, 02:48 UTC) was refused by
Google before the VM started:

> The zone 'projects/lumind-mvp/zones/us-central1-a' does not have enough resources available to fulfill the
> request. Try a different zone, or try again later. (reason: resource_availability)

Nothing started. The ledger has a closed row of 0.00 h: $17.05 committed, $12.45 left. As the PI's direction gives for
this case, the run goes on the laptop.

### The checks before the run (amendment (4)'s, done)

- **Rule (2) on Monkey Island** (`a4/mi-check.sh`, both switches on, the bound 300 s, no background):
  - B3's 15 tries equal E013's G-on run in every field `fmi check tries` compares (15 of 15). 10 of them read fresh
    on the bound's thread, none reached the bound. Their 126 new entries verify from boot with 0 differences.
  - The kitchen's goal entry 150 and its chain (entries 44, 55, 85, 123, 135, 142, 146, 150) verify from boot with 0
    differences.
  - The smoke run (4 jobs from the boot) read scope {33} in every try and grew nothing. Its 21 entries verify from
    boot with 0 differences, and the stall report passes its 5 checks.
- **The unit tests:** 188 Rust tests pass (fmi-core, fmi-reading, fmi-plan, fmi-explore, fmi); scumm-reader's 44.
- **The laptop's check archive** `vm-check4` (2 tries, b = 1, the run's settings): 21 entries verify from boot with 0
  differences. j00001 named room 42 for room 49's scope (case 1, the guide's atom room = 82, at depth 1).
- **A scratch run on the demo** (`scratch4`, 8 tries, b = 2, the bound lowered to 120 s to reach it sooner): every
  piece ran.
  - Three model tries reached the bound: one reading went on in the background, two were stopped. Each went on
    model-free from the stand-in, the root's reading of {49}.
  - Room 49's scope grew twice by case 1: room 42, then room 48 (the guide's atom state:582 = 1).
  - Its 25 entries verify from boot with 0 differences.
  - The stall table: reading bound 36.9% of the seconds, productive 36.7%, interface, no yield 26.0%; the rest row
    0.03%.
  - Wall: 595.6 s of driver wall for 974 s of try wall.

### The reader's stop, fixed (found by the scratch run)

The in-process reader's stop raised its panic while holding the pool's queue lock, so the lock was poisoned. The pool's
shutdown then panicked on that lock, and the workers that waited for work were never told to end. The stopped
reading's thread and those workers stayed, idle, holding their memory, until the process ended.

The fix is in scumm-reader's `par.rs`:

- the stop, and a worker's failure, let go of the lock before they panic;
- the shutdown takes the lock even when it is poisoned.

A reading that is not stopped runs the same code as before. Checked:

- B3's 15 tries again on the rebuilt binary, against E013's G-on;
- scumm-reader's 44 tests;
- two stops at a 60 s bound on a copy of the scratch archive (`scratch4b`, 2 more tries): each gave one panic, the
  stop's, and no poisoned lock.

### The run on the laptop

Amendment (4)'s run, with these changes:

- **The machine** is the laptop: an i5-8350U, 4 cores and 8 threads, 23 GB of memory. The desktop's other programs
  leave about 8 GB free and a load of 2-3.
- **b = 2 and 1 reading in the background** (`A4_BG=1`). A try takes about 2.5 cores and a background reading 1.7
  cores (the 1-room reading from entry 3: 2,120 CPU s over 1,256 s of wall). The heavy lock is as the driver takes it.
- **The archive** is a new one, `/var/tmp/fast-mi/e014-archives/play4`, with its files under
  `/var/tmp/fast-mi/e014-archives/play4-run` and `/var/tmp/fast-mi/e014-play4`. Seed 1; the guide is amendment (4)'s.
  Every other setting is amendment (4)'s: ALLON with the bound 300 s and grown scopes, the game's settings, the rooms
  from the files, E009's switches, play when ready, commit as finished, the falsifier at 80 committed tries.
- **No check of a second machine** is needed: the run is on the laptop that made `vm-check4`.
- **Projection** from the scratch run, with the bound at 300 s in place of 120 s: about 190 s of try wall per try.
  With b = 2, 80 tries take about 2 h of driver wall, 1.5-4 h in all.
- **Stop rule.** Amendment (4)'s, without the VM's booked time: the goal, E009's stop rules (stagnation over T and
  rooms, an exhausted archive), the falsifier at 80 committed tries. It runs once. Exactness first. A break that stops
  the driver is fixed only by a dated amendment and rule (2), and the run resumes from its files.
- **Verify.** Every entry is verified from boot on the lean engine at the end. Any goal entry is verified from boot
  with `fmi explore verify`. E008's `explore.py verify` cannot run this game, as amendment (3) says.

### Predictions added (before the run)

Amendment (4)'s predictions are scored as written; its row "the VM" is not scored, as no VM ran.

| | predicted |
|---|---|
| the run's driver wall on the laptop | 1.5-4 h |
| background readings | the one slot is taken by the first bounded reading of a state after the opening dialog; it ends within 15-30 minutes on the laptop |

## Result (2026-11-04): amendments (4) and (5) on the laptop; stopped by stagnation after 37 tries, all in room 49

### In plain terms

- **The run.** The explorer ran from the demo's boot on the laptop, because the VM could not start (amendment (5)). It
  had every E012, E013 and E014 switch on, and both of amendment (4)'s. It stopped by E009's stagnation rule after 37
  committed tries and 89 minutes of driver wall. It made 46 entries, every one in room 49: 1 of the 11 rooms in the
  files.
- **The end** was not reached. Every entry verifies from boot with 0 differences (45 entries, the lean engine).
- **What the system did, in the game's numbers:**
  1. It booted to the first decision point (frame 992, room 49) and planned a move of 1 frame that gives var 52 = 0.
  2. It waited through the opening to frame 1042, where verbs 1, 3-12 and 100-104 come on.
  3. From there it tried the interface's own inputs, at most 3 per entry. Some clicks set var 107 to each of 3-12, the
     numbers of the ten verbs shown. Others set bits 16, 18, 22, 403, 415 and 733 and vars 45, 100, 165, 181 and 442.
     One, at frame 1156, turned on verb 124, one of the reading's dialog verbs.
  4. No object changed owner, state or class, and no other room was entered. The latest entry is at frame 1206, 214
     frames after the first decision point.
- **Why it stopped.**
  - Every model try from a state after the opening, 18 of the 21, reached the bound: its reading had not ended in
    300 s. The model-free steps that followed kept 2 entries.
  - The 16 free tries kept 3 entries, all in one try from the root.
  - After the 9th commit no try gave a new atom of T or a room. The stagnation rule fired 28 tries later, at the
    sweep of the 27 selectable cells.

### The stall table by cause (`results/a4/stalls-play4.md`; 37 tries, 9,452.8 s of try wall)

| class | seconds | share |
|---|---:|---:|
| reading bound | 5,400.0 | 57.1% |
| interface, no yield | 3,357.9 | 35.5% |
| productive | 431.3 | 4.6% |
| overhead | 236.9 | 2.5% |
| wait | 26.6 | 0.3% |
| no plan | 0.1 | 0.0% |
| model-blind, refused input, effect mismatch, model, no yield | 0.0 | 0.0% |

- **Overhead** is almost all one try's rest. j00032's free step failed on an input the bridge refuses (row 21), so its
  234 s have no record of their own. That try is also why the rest row is 2.5% and not under 1%.
- **The largest rows:** the bound (18 tries, 5,400 s); the free step's offers at the entry that kept nothing (30
  points, 2,532 s); its delays (554 s).

### The two switches on the demo

- **The bound.**
  - 18 readings reached it: 2 went on in the background and 16 were stopped.
  - Neither background reading gave a reading. The first, from entry 3 (scope {42, 49}, started at 03:16), ended
    without one, most likely at the reader's own wall watchdog of 60 minutes (`--cap-min`, default 60). The slot was
    free again when the second, from entry 41, reached the bound at 04:43, two minutes before the run's end. The run's
    log has 17 stops of a reading: the 16 at the bound and this one. No try took a reading from the cache.
  - So row 19's "no wall bound" was not exact. The reader has had its own watchdog of 60 minutes of wall since E012,
    and amendment (3)'s laptop check stopped its reading at 50 minutes, before it. Without the bound, a try from these
    states waits 60 minutes and then fails with its reading's stop.
  - A reading always stood in: the root's reading of {49}, or j00003's of {42, 49}.
  - The stopped readings did not stop at once (row 20). The reader looks at its stop flag only while it waits for its
    workers, and a worker ends only when its run ends; at these states one run takes minutes. At the 26th try, 14
    reading threads and 8 sets of workers were still running, a load of 18-23 on the laptop's 8 threads from then to
    the end.
- **The scopes.**
  - Room 49's first reading was of {49} (35.6 s from the root).
  - At the 2nd commit the scope grew by room 42 (case 1: the guide's atom room = 82 at depth 1, whose writers are in
    rooms 42 and 75, the lower one taken).
  - The only other reading that ended was of {42, 49} (50.7 s from the root). The 18 bounded readings were of
    {42, 49}.
  - Nothing grew again. A bounded try is model-free, so it names no room (case 1 is a model try's), and no model-free
    entry entered another room (case 3).
  - The measure before the run had already shown that the room count is not what makes these readings slow: {49}
    alone from entry 3 took 1,256 s.

### Rule (2) on Monkey Island, both switches on (amendment (5); `results/a4/mi-check.txt`)

- **B3.** The 15 tries equal E013's G-on in every field compared. 10 of them read on the bound's thread, and none
  reached the bound. Their 126 new entries verify from boot with 0 differences. Both held before and after the
  reader's fix.
- **The kitchen.** The goal entry's chain (8 entries) verifies from boot with 0 differences.
- **The smoke run.** 4 tries from the boot, each reading {33} and growing nothing; its 21 entries verify from boot.

### Exactness and the reference

- Every entry verifies from boot on the lean engine: 45 of 45 here, 21 in `vm-check4` and 25 in `scratch4`, each with 0
  differences.
- There is no goal entry to verify. E008's `explore.py verify` on svm-w3b cannot run this game (amendment (3)).

### Predictions scored

Amendment (4)'s, as written:

| | predicted | measured | |
|---|---|---|---|
| rule (2) on Monkey Island | B3 equal, verified; the kitchen chain verifies; the smoke run verifies; no reading reaches the bound | all four | hit |
| how the run stops | falsifier 45%, stagnation 35%, the VM's time 10%, the goal 10% | stagnation | (35%) |
| committed tries | 40-80 | 37 | miss |
| bounded readings | 40-80% of the model tries after the opening; none from the root | 18 of 18 (100%); none from the root | miss |
| background readings | 0-30% end within the run; 0-3 later tries take one | 0 of 2 gave a reading; 0 tries | hit |
| scopes | first reading of a room 1 room; G(49) grows by 1-4 rooms; 80% of readings at most 3 rooms | {49}; G(49) grew by 1 (room 42); every reading 1-2 rooms | hit |
| rooms entered, of 11 | 2-5, most likely 3 | 1 | miss |
| the end | not reached (reached: 10%) | not reached | hit |
| the stall table | reading bound 20-50%; productive 20-50%; interface, no yield 10-35%; no plan and model-blind under 15%; rest at most 1% | 57.1%; 4.6%; 35.5%; 0.0%; 2.5% | 1 of 5 |
| free tries | 25-50% of the tries; 60-90% of the entries from model-free steps | 16 of 37 (43%); 43 of 45 (96%: wait 2, afford 36, free 5) | 1 of 2 |
| entries verified from boot | every entry, 0 differences | 45 of 45 | hit |
| the VM | 1.5-3 h, $2.3-4.6 | no VM ran | not scored |

Amendment (5)'s added rows: the driver wall 1.5-4 h, measured 1.49 h: a miss by 0.01 h. The background slot was taken
by the first bounded reading after the opening, as predicted, but it gave no reading (predicted: it ends within 15-30
minutes): a miss, under the load of row 20.

For reference, amendment (3)'s and E013's F, written for this run without the two switches:

- held: the stop by stagnation (25% and 35%); 30-80 and 25-80 tries; the end not reached; free tries 25-50% and
  25-45%; 1-3 or 1-4 new rows (rows 20 and 21); every entry verified.
- missed:
  - rooms 2-5 (1);
  - productive 30-70% and 10-35% (4.6%);
  - the longest stretch with no new atom or room, 5-20 tries (28);
  - at least 30% of free tries keeping a novel entry (1 of 16);
  - F's model-blind or no plan at 15-35%, and refused input at 5-20% (both 0%).

### Threshold and what bound it

**Partly**, by the success threshold. The explorer ran on this game and made progress past the root: new atoms of T
from the opening's verbs and flags. The game's end was not reached. Three blockers are named:

1. **The reading at the states after the opening.** At these states the reader's cost is in a few event runs that each
   take minutes; the scope's room count does not change it (1,256 s for {49}). With the bound, the model is absent
   from every state after the opening, so the run is a model-free run in room 49.
2. **The stopped readings keep running** until their workers' runs end (row 20). They took most of the laptop's CPU
   from the tries.
3. **The model-free steps found no change of any object** in 2 hours of the interface's inputs, waits and delays.
   Their offers (verbs, targets, walk boxes) are tried at most 2 levels deep from an entry (1 level after a delay).
   One of them gave the bridge an input it refuses, and that error ended the try (row 21).

Rows 20 and 21 are recorded and not applied to this run.
