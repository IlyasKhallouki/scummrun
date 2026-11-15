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

## Amendment 2026-11-04 (6): the reading after the opening, diagnosed; loops widened in a run the path limit cut

Written and committed before any code it plans. The sections above stay as written and their predictions are not
edited. No run of the explorer is planned here: the PI decides the next run from this amendment's result.

### The slow reading, reproduced

The {49} reading from entry 3 of `vm-check` (frame 1042), with the run's build (target-e014b), its switches (ALLON and
the game's settings) and the try's arguments (3 threads, `--memo-mem --memo-units-set --prep` as the in-process reader
adds them), run as reader-scope with `READER_TRACE=1`:

- **Where the time goes.** Six event jobs, all resuming one room-local script of the opening's dialog at three of its
  wait points (two of them twice, with different kept lists). Each took 148-375 s; every other job of the reading took
  under 0.1 s. The boot's {49} reading has one such job (39.2 s of its 39.3 s), resuming a global script.
- **What one run of such a job does.** It passes the path limit (1,024 paths): 0 paths end, 13-999 are cut. Its
  7,008 forks on non-fact tests all sit in one global script, the interface's inventory display. That script works out a
  window of inventory slots from the inventory's count, then loops over the window: per slot, the object found there, its
  class, its verb entry and a comparison with one object fork, and so does the loop's exit (488 forks), because the
  window's bounds come from the count. The reader leaves the inventory's count and order unknown (`findInventory` and
  `getInventoryCount`: "the inventory is not enumerated by the reader"; the engine's inventory is the order objects were
  picked up, not a fact). So the loop's exit never decides, the loop runs on until the path limit, and each of its
  iterations sends one more path to the end of the run.
- **Why it takes minutes, not seconds: the split ladder.** Under `split_limit` a run past the path limit is run again
  once per value of its first fork on a read, and each part that passes the limit is split again. Here the forks that
  pass the limit are on non-facts, so no part ever falls under it: the job is run again once per kept value of every
  fact the run reads before the cut, one fact per level. The six jobs: 69-137 splits each, 78-149 parts, 19-22 levels
  deep, most levels keeping 1 value of 2-22. At the bottom, with no fact left, `FMI_READ_CUT=1` reads the part short (no
  record). The six jobs gave 1-3 records each, from the few parts whose pins keep the run out of the inventory display
  (the dialog's branch that leaves the room). 292 runs of 1 s or more, 1,716 CPU s, in pass 1 alone; pass 2 repeats
  round 9.
- **Why each run takes 1-16 s.** Sampling the boot reading's slow job with gdb (18 busy samples of the machine): 15 in
  the merge at a join, 13 of them in the merge's set of assumptions (`FMI_MERGE_DEDUP`), which hashes every assumption's
  text; the unknowns' names grow by one level per iteration of the loop ("((((... the inventory is not enumerated by the
  reader) + ...") so the cost per iteration grows with the iterations.
- **From the boot** the interface is off (cutscene on), so the inventory display runs in one job only; from frame 1042
  the interface is live and every way out of the dialog redraws the inventory.

### The cause, as a mechanic

**An interface loop over values the reader leaves unknown.** Adventure games redraw their interface (an inventory, a
verb or dialog panel) in a loop over slots whose contents and bounds come from engine state that is not a fact here.
Monkey Island's inventory has a fixed window; Fate of Atlantis's scrolls, so its bounds come from the count. The
reachable space is small: the engine draws one window of at most ten slots, once. **The cost is the reader's**:

1. it unrolls a loop whose exit it cannot decide until the path limit, where a loop whose state repeats up to unknown
   values could be summarised once (a missing widening);
2. split_limit then repeats the whole run per fact value, though no fact pin can lift a cut made by non-fact forks
   (redundant work);
3. each iteration costs more than the last (the merge's set over growing names).

### The fix: `FMI_READ_WIDEN=1` (unset: as before)

A run the path limit cut is run again once with its loops widened; if that run ends under the limit, it is the run's
reading and nothing is split. A run the path limit does not cut is never touched, so a reading with no such run (Monkey
Island's B3 readings have none: their reports count no split) is byte for byte the same.

- **Widening (scumm-machine, `Machine::widen_loops`).** At a join point a path reaches again in the same frame (a loop),
  the machine compares the path's state with its last visit there. From the second visit on, every fact and local
  whose value changed and is unknown becomes a plain unknown ("varies around the loop", widened), and the point
  remembers the widened keys. A known value that changed is left as it is (a counter with a known bound runs exactly).
  When a path comes back to the point and every key that changed since the widened visit is a widened key, its state is
  an instance of the widened one: the path ends ("widened", dropped as a spin is), and the paths that left the
  widened iteration stand for every later iteration.
- **Why it is sound.** A plain unknown is any value, so the widened state holds every state the loop can reach after
  it; the widened iteration's paths (both sides of every unknown test) cover every concrete iteration from those
  states, and by induction over the iterations every concrete way out of the loop. A widened fact written by a record is
  a guess over its domain (`Eff::Guess`), a superset of the values written. The reading then holds records for paths the
  cut run dropped, and more general records (fewer pins) for the paths the ladder's parts read. Exactness of the run is
  unchanged: every entry is still its inputs and movie, verified from boot; a record that is too general costs a plan
  the engine refutes, never an entry.
- **The builder (scumm-reader, `Builder::run`).** With the switch on and split_limit on, a run with `limit != 0` is run
  again on the same machine with widening on. Ended under the limit: its paths, held branches and counters are the
  run's. Still cut: the first run goes on to the split as before (each part, if cut, tries the same once).
- **Not in this fix:** the merge's set over long names (cause 3) and the inventory as a fact. Both stay recorded.
- **Tests:** unit tests of the widening (a loop with an unknown exit ends after the widened iteration and its exits
  carry plain unknowns; a loop with a known counter runs exactly; the switch off leaves the run as before) and of the
  builder's choice (a run that fits is not re-run; one that is cut and fits widened is not split).

### Predictions (before any code)

| | predicted |
|---|---|
| the {49} reading from entry 3, switch on | 5-30 s of wall (1,256 s off); the six jobs each one cut run (2-3 s) and one widened run; no split |
| the same reading, switch off, on the new build | as before: 600-1,300 s, the same 908 records |
| the {49} reading from the boot, switch on | 3-15 s (35.6-39.3 s off) |
| a later state (play4's entry 18 or 19), switch on | under 60 s; off: past 300 s |
| the outputs where both end | switch on holds every action family of switch off (same labels, a superset of the facts' values reached); more records where switch off cut runs dropped |
| Monkey Island, rule (2) | B3's 15 tries equal E013's G-on (no B3 reading has a cut run); their new entries verify; the kitchen chain verifies |
| the risk to exactness | none to entries (verified from boot); the reading is more general where the limit cut |

### Stop rule

- Stop and write up if the widened run of the six jobs is still cut (the fix does not apply), if the {49} reading from
  entry 3 takes more than 60 s with the switch on, or if any Monkey Island check differs.
- One revision is allowed, written as a dated note under this amendment before its code. No longer bound and no
  game-specific rule in any case.

## Result (2026-11-04): amendment (6), `FMI_READ_WIDEN=1`, measured on the laptop

### In plain terms

- **The cause** is the reader's, not the game's. From the state after the opening, every way out of the dialog redraws
  the inventory. The inventory display loops over a window of slots whose bounds come from the inventory's count, which
  the reader leaves unknown. So the reader unrolled the loop until the path limit, and split_limit then ran the whole job
  again once per value of every fact read before the cut. No part could fall under the limit: 6 jobs, 69-137 splits
  each, 1-3 records each.
- **The fix.** A run the path limit cut is run once more with its loops widened. A loop whose exit stays unknown is run
  exactly once, then once with every value it moved made unknown. A path that comes back in a state the widened
  iteration holds ends. Runs under the limit are never run again.
- **The effect.** The {49} reading from entry 3 takes 23.6 s, against 926 s with the switch off on the same build
  (1,256 s in amendment (5)). A single try from entry 3 now has a model: it reads, plans and plays in 23.1 s. Before,
  every such try reached the 300 s bound.
- **Not seconds yet, and the boot is slower.** The reading from the boot takes 94.7 s against 37.0 s. The job at the
  boot that the cut used to empty now gives records, so the boot reading holds the states after the opening
  (25,894 records against 37). Runs that pass the limit for other reasons still split.

### The readings ({49}, the try's arguments, 3 threads, `fmi read` on target-e014c, the laptop at a load of 1.7-3.7)

| from | switch | wall | CPU | records | actions | widened runs: whole / still past the limit | splits |
|---|---|---:|---:|---:|---:|---|---:|
| entry 3 of `vm-check` (frame 1042) | off | 926.2 s | 1,662 s | 908 | 649 | - | 439 |
| | on | 23.6 s | 41.5 s | 20,529 | 7,089 | 17 / 13 | 13 (25 parts) |
| the boot (entry 0, frame 992) | off | 37.0 s | 37.0 s | 37 | 32 | - | 27 |
| | on | 94.7 s | 182.4 s | 25,894 | 8,758 | 57 / 130 | 130 (181 parts) |
| play4's entry 19 (frame 1206) | off | 957.0 s | 1,718 s | 907 | 648 | - | 439 |
| | on | 21.2 s | 38.0 s | 20,528 | 7,088 | 17 / 13 | 13 |
| play4's entry 18 (frame 1156, the dialog) | on | 20.6 s | 38.4 s | 37,009 | 18,446 | 14 / 7 | 7 |

- **Switch off** is byte for byte the reading before the change: from entry 3 and from the boot, the packed files' sha1
  equal amendment (5)'s measures (`eccab6b2...` from entry 3).
- The start rows of entries 18 and 19 were made by `fmi check wait --read-scope 49` (the entry rebuilt from boot), stopped
  once its start row was written; no try had read from them.

### The outputs where both end

The on reading holds every value the off reading reaches, and every one of its action labels. This held from entry 3,
from the boot and from entry 19, with 0 values and 0 labels missing.

| from | facts reached (on / off) | values reached | labels | actions |
|---|---|---|---|---|
| entry 3 | 451 / 271 | 698 / 453 | 271 / 190 | 7,089 / 649 |
| the boot | 486 / 79 | 741 / 117 | 288 / 24 | 8,758 / 32 |
| entry 19 | 449 / 269 | 693 / 448 | 270 / 189 | 7,088 / 648 |

**Why the difference is sound.** The off reading drops the paths of every run the limit cut and the ladder could not
lift (`FMI_READ_CUT=1`: no record). Those paths are the ways out of the dialog through the inventory display. The on
reading holds them as records whose widened facts are guesses over their domains: a superset of the values the engine
writes. The ladder's parts' records come back as records of the whole run with fewer pins (more general). Nothing the
off reading holds is missing. An over-general record costs a plan the engine refutes, never an entry: entries are
still their inputs and movie, verified from boot.

### Single tries on this build (scope {49}, the run's switches, `fmi try` on a copy of `vm-check`'s job)

- **From entry 3** (j00002): 23.1 s in all; the fresh reading 20.9 s (not bounded); 6 targets (3 of tier 1, 3 of
  tier 2). The attempt for bit 20 = 1 planned, played 3,536 frames (frames 1042-4578) and stopped at a reach extension.
  1 new entry.
- **From the boot** (j00000): 204.4 s in all, against 40.9 s in amendment (3)'s laptop check. The reading took 86.5 s;
  a refinement 43.1 s; a reach extension 73.2 s (both on the larger reading); 5 targets, 3 new entries.

### Rule (2) on Monkey Island, the switch on (`A4_ENV=a6/env.sh a4/mi-check.sh`; `results/a6/mi-check.txt`)

- **B3.** The 15 tries equal E013's G-on in every field compared (15 of 15). Their 126 new entries verify from boot with 0
  differences.
- **The kitchen.** The goal entry's chain (8 entries) verifies from boot.
- **The smoke run.** Its 21 entries verify from boot; the stall report passes its 5 checks.
- **Why B3 is unchanged.** 5 of B3's 12 readings have runs past the path limit (30 runs), and every one was still past it
  widened. So each was split as before: the outputs are the switch-off ones, at the cost of 30 more runs (B3's wall
  302.6 s against 296.5 s in amendment (5)'s check).

### Tests

- scumm-machine's 4 new tests (`tests/widen_loops.rs`) and 2 unit tests of the diffs.
- scumm-reader's 3 new tests (`tests/read_widen.rs`).
- Every other test of both crates passes, the golden comparisons with model.py included.

### Predictions scored

| | predicted | measured | |
|---|---|---|---|
| the {49} reading from entry 3, on | 5-30 s; each slow job one cut run and one widened run; no split | 23.6 s; 17 runs read whole widened, 13 still past the limit and split (25 parts) | wall hit; "no split" missed |
| the same, off, new build | 600-1,300 s, the same 908 records | 926.2 s, 908 records, the same packed file | hit |
| the boot, on | 3-15 s | 94.7 s (off 37.0 s) | miss |
| a later state, on / off | under 60 s / past 300 s | entry 19: 21.2 s / 957.0 s; entry 18: 20.6 s on | hit |
| the outputs | on holds every action family of off, a superset of the values | 0 values and 0 labels of off missing, at all three states | hit |
| Monkey Island | B3 equal, verified, the kitchen; "no B3 reading has a cut run" | all four checks hold; but 5 of 12 B3 readings have runs past the limit (30), none fit widened | checks hit; the reason missed |
| exactness | none to entries | the switch changes no engine path and no verify; the 2 tries' 4 new entries were not verified (not in an archive) | not scored |

**Misses, stated plainly.**

1. **The boot's reading is slower, 37.0 s to 94.7 s, and the root try 40.9 s to 204.4 s.** The prediction did not see
   that the boot's cut job hid the rest of the opening. Its records now open the interface, and the boot reading then
   covers the dialog states as well.
2. **Runs still split.** After the inventory loop is widened, the runs that end the dialog still pass the limit. They
   multiply forks on facts in the interface's per-frame hover script (the object under the cursor as a set of the
   room's objects, each one's classes and owner) by its tests of variables that are no facts. Paths with different
   facts never merge, so this is no loop for widening to summarise. At entry 3 it costs 13 splits (about 12 s of the
   23.6 s); at the boot, 130.
3. **"Seconds" is not reached.** Readings after the opening take 20-24 s; the plan aimed at seconds.
4. **The written reason for Monkey Island was wrong.** The checks hold because no widened run there fit, not because
   there are no cut runs.
5. **The machine's merge cost stays.** The first (unwidened) run of a cut job still goes to the limit. In gdb samples of
   the reading from entry 3 with the switch on, 26 of 43 were in the merge at joins: the assumption sets of
   `FMI_MERGE_DEDUP`, hashing names that grow with each iteration.

No revision was used. The stop rule did not fire: 23.6 s is under 60 s, and every Monkey Island check holds. Its first
condition held partly: the slow jobs' widened runs fit in pass 1 except one, and 13 runs did not fit over both passes.

### What this leaves (for the PI's choice of the next run)

- With the switch on, tries from the states after the opening read in about 21-24 s, under the 300 s bound, and plan.
- The root's try costs about 3.5 minutes once.
- Not built, each a general change:
  - the hover script's fact forks are the next cost: a split that keeps one value repeats the run; parts of a split
    whose widened run was cut could run widened;
  - the merge's assumption set could hash cheaply;
  - the inventory could be a fact.

## Amendment 2026-11-04 (7): stagnation after a sweep, a refused input as a trial; the run from the boot with `FMI_READ_WIDEN=1`

Written and committed before any code or run it changes. The sections above stay as written and their predictions are
not edited.

### Why

Amendment (6) removed the cost that sent every model try after the opening to the bound: with `FMI_READ_WIDEN=1` the
{49} reading from entry 3 takes 23.6 s, and a try from there plans and plays. The check of play4
(`notes/log/2026-11-04-e014-bound.md`, "Check") found two errors that bear on the next run:

1. **The stagnation stop is a count, not a sweep.** `archive.rs` `stagnant` fires when the committed tries since the
   last try that gave a new atom of T or a new room reach the number of cells selectable at that try. The selection
   orders cells by their counts, then by the entry's key, whose last parts are the frame and the id. In play4 the early
   cells got a model try and a free try each, and the stop came 28 tries after the last new atom with 6 of the 27
   selectable cells never tried. Among them were entry 18, the only state with a dialog open, and entry 19, the
   furthest state.
2. **An input the bridge refuses ends the whole try** (row 21 of the assumptions table). j00032's free step gave the
   bridge an input its input check refuses ("event 0: expected frame > 0, and a click/rclick/move in range"). The error
   ended the try, and its 234 s have no record of their own.

The PI's direction: fix both generally, then run the demo from the boot again with every switch on.

### (a) Stagnation only after a sweep (`FMI_STAG_CELLS=1`; unset: as before)

- **The rule.** Stagnation may fire only once every cell selectable now has had at least one committed try since the
  last committed try that gave a new atom of T or a new room (since the start, when no try has given one). The count
  stays as a second condition, so the rule never fires sooner than before.
- **A try** is a committed line of the log of either kind (model or free), errors included and "return differs"
  excluded, as the cells' counts take them (`recount`). Its cell is its entry's cell now, so a cell split when T grows
  is counted by its own entries.
- **Selectable** is the selection's own test: the cell has a candidate (a live entry, not exhausted under the current
  T, in an allowed room). Without the progress order, E009's selectable cells.
- **Why the run still ends.** A cell untried since the last new atom keeps its counts while the tried cells' counts
  grow, so the selection reaches it (a model try takes the least model counts, a free try the least free counts). New
  cells made with no new atom extend the sweep; the falsifier bounds the run.
- **Switch.** It changes when a run may stop, so it has its own switch, on for both games (rule (2)).

### (b) An input the bridge refuses is one refused trial

- **Where.** The affordance step's trials (`afford::try_all`): a trial's clicks, and the third click. They serve E012's
  affordance step, E013's free step and the bounded try's model-free step.
- **The change.** When the bridge answers a trial's inputs with an error (its input check: the frames, a click's
  coordinates, a key's name), the trial is recorded as refused (reason "refused", verdict "dropped: bridge", its
  seconds), its fork is quit, and the step goes on with the next input. Every other engine error still ends the try.
- **Records.** The step's record lists each such trial ("bridge_refused": the inputs' name, the events and the bridge's
  text), so the offer point that made it is in the files. The stall table classes its seconds as "refused input" (B's
  verdict `dropped: ...`).
- **No switch.** It changes only a try that this error ended before; every other try is as before. Rule (2) checks
  this on Monkey Island.
- **Not changed:** the offer's points. What made such a point is read from the record after the run, not guessed.

### Tests (with the code)

- (a): the count reached with one selectable cell untried since the last new atom (not stagnant), then a try from it
  (stagnant); a new atom starts a new sweep; free tries count; an exhausted cell is not waited for; the switch off
  gives the count alone.
- (b): a toy engine whose input check refuses a click off the screen, as the bridge does: the step records the refused
  trials, runs every other input and lists them in its record.

### Rule (2) on Monkey Island, every switch on

`A4_ENV=a7/env.sh a4/mi-check.sh OUT b3 kitchen smoke` on the new build; `a7/env.sh` is a6/env.sh with the new build
and `FMI_STAG_CELLS=1`. It must pass before the run:

- B3's 15 tries equal E013's G-on in every field `fmi check tries` compares, and their new entries verify from boot;
- the kitchen's goal chain verifies from boot;
- the smoke run's entries verify from boot, and its stall report passes its checks.

### The run

- **The build:** e014's head with (a) and (b), in `/var/tmp/fast-mi/target-e014d`; the engine svm-e013, unchanged.
- **The switches:** a6/env.sh's ALLON (every E012, E013 and E014 switch; the bound `FMI_READ_BOUND=300`; grown scopes;
  `FMI_READ_WIDEN=1`) with `FMI_STAG_CELLS=1`, and the game's settings as before (`a7/env.sh`, `a7/run-env.sh`).
- **The archive:** a new one, `/var/tmp/fast-mi/e014-archives/play7`, from the boot. Seed 1; amendment (4)'s guide
  file. Stage play, goal end = 1, the rooms from the files, E009's switches, play when ready, commit as finished, the
  falsifier at 80 committed tries.
- **Where.**
  1. **The cloud VM, one start:** `python3 scripts/cloud.py start --type n2-custom-30-122880 --hours 3 --standard`
     from main's scripts ($1.5346/h: $4.60 booked of the $12.45 left).
     - b = 10 and 2 readings in the background (amendment (4)'s count for 30 cores: a try about 2.5 cores, a reading
       about 2).
     - Set up, checked and copied back as a4/vm.sh, in `a7/vm.sh`: this build, this archive, and a 2-try check archive
       `vm-check7` made on the laptop first, verified from boot on the VM, run again there and compared.
     - The VM is stopped with cloud.py as soon as the run ends or stops, and the ledger records it.
  2. **If the start is refused:** the laptop, as a4/laptop.sh: b = 2, 1 reading in the background.

  The write-up says which ran.
- **Verify.** Every entry from boot on the lean engine (`fmi explore verify`), 0 differences. For a goal entry, its
  chain must hold the system's own inputs on the way to the end site. A chain of waits only, or the demo playing itself
  to its end, is reported as such and not as reached.
- **Projection.**
  - A model try from the states after the opening reads in about 21-24 s on the laptop and plans; the root's try takes
    about 3.5 minutes (amendment (6)); free tries take 15-120 s (play4). Readings of rooms other than 49 are not
    measured. A mean try of 50-150 s on the laptop.
  - **Laptop, b = 2:** 80 tries of 50-150 s give 2,000-6,000 s of driver wall, 0.6-1.7 h; up to 3 h if readings of
    other rooms are slower.
  - **VM, b = 10**, a core 1.5-2 times the laptop's: 80 tries in 10-30 minutes of driver wall. With the setup, the
    check (about 15 minutes), the verify and the copies: 1-1.5 h used, $1.5-2.3. Booked: 3 h.
- **Stop rule.**
  - The goal.
  - E009's stop rules: stagnation over T and rooms, under (a); an exhausted archive.
  - The falsifier at 80 committed tries.
  - On the VM, its booked time: the driver is stopped 20 minutes before H, and the write-up says so.
  - It runs once. Exactness first. A break that stops the driver is fixed only by a dated amendment and rule (2), and
    the run resumes from its files. Any other break is a row of the assumptions table, recorded and not applied. A stop
    by hand only if something is clearly broken, with the reason written down.

### Predictions (before any code)

| | predicted |
|---|---|
| rule (2) on Monkey Island | B3's 15 tries equal E013's G-on; their new entries, the kitchen chain and the smoke run verify from boot |
| how the run stops | the falsifier at 80 committed tries: 55%; stagnation under (a): 20%; the goal: 15%; the VM's booked time or a break: 10% |
| committed tries | 50-80 |
| rooms entered, of the 11 in the files | 2-5, most likely 3 (room 49 counted) |
| the end | not reached; reached by the system's own inputs and verified from boot: 15% |
| model tries whose reading reaches the bound | 0-20% |
| the stall table | reading bound 0-15% of the seconds; productive 25-60%; interface, no yield 10-40%; model, no yield 5-30%; no plan 0-15%; refused input 0-5%; the rest row at most 1% |
| (a) | if the stop is stagnation, every cell selectable at the stop had a try since the last new atom or room; a kept state with a dialog open gets a try |
| (b) | 0-5 trials refused by the bridge in the run; no try ends on a bridge error on its inputs |
| entries verified from boot | every entry, 0 differences |
| the VM, if it starts | 1-1.5 h used, $1.5-2.3 |
| the laptop, if not | 0.6-3 h of driver wall |

## Result (2026-11-04): amendment (7) on the cloud VM; stopped by the falsifier at 80 tries, all in room 49

### In plain terms

- **Where it ran.** The VM started (08:13 UTC), so the run was on the VM: b = 10, 2 readings in the background. The
  check before the run passed. The run took 460.7 s of driver wall (7.7 minutes) for 80 committed tries, and the VM was
  stopped at 08:36: 0.39 h, $0.60 in the ledger ($17.65 committed, $11.85 left).
- **How it stopped:** by the falsifier at 80 committed tries, the rule, not by hand. The run was still finding new
  atoms: 14 of the 80 tries gave new atoms of T, the last two among them.
- **The end** was not reached. All 175 entries are in room 49: 1 of the 11 rooms in the files. Every entry past the
  root verifies from boot with 0 differences (174 of 174, the lean engine, on the laptop).
- **What changed from play4.** The model now reads and plans from the states after the opening. No reading reached the
  300 s bound (0 of 51 model tries). The run kept 175 entries (play4: 46), found 62 new atoms of T (play4: 40), and
  went 10 inputs deep. No object changed owner or state.
- **What bound it.** The plans from the states after the opening fail on the engine. 38.4% of the seconds are effect
  mismatch: the plan's sentence ran, but the write its record predicted is not in the engine's run. 14.1% are
  model-blind: the start holds a script position the reading does not reach.

### The two fixes in the run

- **(a) `FMI_STAG_CELLS=1`.** The run did not stop by stagnation, so the sweep rule did not decide the stop. On play4's
  files the same rule turns play4's stop off (`fmi explore status`: stagnant false with the switch, true without).
  9 of the 17 entries that hold a dialog state (dialog verbs on, sentence verbs off; the root's included) were a try's
  entry: entries 0, 23, 30, 80, 97, 98, 112, 118 and 126. Entries 23, 97, 98, 112 and 118 gave new atoms.
- **(b) The bridge's refused inputs.** 86 trials in 4 tries (j00065, j00075, j00076, j00078) were refused by the
  bridge and recorded as refused trials. Every one clicks actor 7 at a point left of the screen, as the third click or
  the target (`v12t592t-7`, `t-7`, ...). No try ended on an error (80 of 80 "ok"). Before the run, play4's entry 21 as
  a free try, which had ended on this error, ran 630 trials with 1 refused by the bridge and kept 3 entries.

### The run's numbers

| | |
|---|---|
| committed tries | 80: 51 model, 29 free |
| try wall | 3,429.2 s; model tries 52.5 s on average (median 34.0 s, longest 177.6 s); free tries 26.0 s (median 17.1 s) |
| driver wall | 460.7 s with b = 10 |
| fresh readings | 41: the boot's {49} 50.5 s; the rest 0.5-36.2 s; none bounded |
| scopes | {49}, grown by room 42 (j00025) and room 48 (j00038), both by case 1 (a target needs one) |
| entries, cells | 175 entries, 97 cells; T 454 atoms at the end (play4: 393) |
| rooms entered | 1: room 49 |
| goal entries | none |

### The stall table by cause (`results/a7/stalls-play7.md`, made on the VM by the run's own `stalls` step)

| class | seconds | share |
|---|---:|---:|
| effect mismatch | 1,316.6 | 38.4% |
| interface, no yield | 798.3 | 23.3% |
| model-blind | 482.1 | 14.1% |
| productive | 463.2 | 13.5% |
| no plan | 343.1 | 10.0% |
| refused input | 17.4 | 0.5% |
| wait | 7.8 | 0.2% |
| overhead | 0.7 | 0.0% |
| model, no yield, reading bound | 0.0 | 0.0% |

- **Effect mismatch** grows over the run: 74.7% of the last 20 tries' seconds. Its 9 tries (1,182.9 s) stop on "the
  same empty window" twice on a planned sentence. 7 of them are on verb 10 with object 592 (143.9-177.6 s each): the
  sentence runs, and the refutation says the record's predicted write on the sentence's object argument is not in the
  engine's run. Two more tries stopped on the same sentence but kept new atoms from their model-free step, so they
  are classed productive.
- **Model-blind:** 16 tries stop at a reach extension: the start holds `pos:script:151 = 59`, which the extended
  reading does not reach either.
- **No plan:** 7 tries in which the planner proved the target var 104 = 3 unreachable; 1 planner error (below).
- **Lemma R's premise R0:** 5 tries; the last pass wrote values of var 165 outside its domain.
- The table was also made again on the laptop from the copied archive. The copy has no `jobs/*/run` folders, so the
  plan and reading rows split differently there (effect mismatch 38.9%, refused input 0.0%). The VM's table, made from
  the full files, is the one given.

### The furthest state, in the game's numbers

- **Entry 151:** room 49, frame 12,272. The chain is entries 0, 2, 23, 97, 112, 119, 151, with 10 inputs: 9 clicks and
  the key "o" at frame 1,471. The last input is at frame 1,472. Entries 148-151 are the wait's stops after it (frames
  1,485, 5,072, 8,672 and 12,272).
- **Owners, states, classes:** as at the root. In the whole archive one class bit changed: class 26 of object 934, set
  at entry 17 (frame 1,080).
- **Bits set since the root:** 16, 401, 403, 408 and 411.
- **Vars changed since the root:**

  | var | root | entry 151 |
  |---|---:|---:|
  | 0 | 8 | 31 |
  | 7, 108, 120 | 0, 0, -1 | 592 |
  | 20, 100 | 256, 0 | 363 |
  | 21, 45, 101 | 0 | 79 |
  | 32 | 17 | 4 |
  | 44 | 0 | 107 |
  | 116 | 0 | 3 |
  | 123, 124 | 0 | 9 |
  | 165 | 0 | 124 |
  | 166 | 192 | 202 |
  | 170 | 1 | 0 |
  | 177 | 0 | 11 |
  | 178 | 53 | 54 |
  | 442 | 1 | 11 |
  | 443 | 44 | 0 |
  | 465 | 0 | 416 |
  | 466 | 0 | 105 |

- **Verbs:** 1, 3-12 and 100-104 on.
- **New atoms of T over the run:** 62 distinct. They are bits (15), vars (33), verbs (12), one cutscene atom and one
  script atom. The bits are 16, 18, 20 (= 0), 22, 401, 403-408, 410, 411, 415 and 733.

### Rule (2) on Monkey Island (`results/a7/mi-check.txt`)

- **B3.** The 15 tries equal E013's G-on in every field compared (15 of 15). 10 read on the bound's thread; none was
  bounded. Their 126 new entries verify from boot.
- **The kitchen.** The goal entry's chain (8 entries) verifies from boot.
- **The smoke run.** Its 21 entries verify from boot, and the stall report passes its 5 checks.
- **The disk floor.** The first smoke run stopped at once at the laptop's disk floor (3.9 GB free, with B3's 2.5 GB of
  readings still on disk). B3's readings and this session's incremental build caches were deleted, and the smoke run
  was made again.

### Engineering met on the way (not the method)

- **The VM's lock.** It was held by a stale `e009` lock file from 2026-11-02. The VM had just booted and ran nothing,
  so the lock was released with `cloud.py release --job e009`.
- **SDL3 on the VM.** The engine's libSDL2 is sdl2-compat, which loads SDL3 at run time, so `ldd` does not list it.
  The engine aborted at the setup; the library was copied to the VM, and `a7/vm.sh` now pushes it (63284e0).
- **The laptop's check archive.** `vm-check7` first stopped at the disk floor after one try. It was made again
  (2 tries, 5 entries).

### Predictions scored

| | predicted | measured | |
|---|---|---|---|
| rule (2) on Monkey Island | B3 equal; their entries, the kitchen chain and the smoke run verify | all hold (the smoke run made again after the disk floor) | hit |
| how the run stops | falsifier 55%; stagnation 20%; goal 15%; the VM's time or a break 10% | the falsifier at 80 tries | (55%) |
| committed tries | 50-80 | 80 | hit |
| rooms entered, of 11 | 2-5, most likely 3 | 1 | miss |
| the end | not reached (reached: 15%) | not reached | hit |
| model tries whose reading reaches the bound | 0-20% | 0 of 51 | hit |
| the stall table | reading bound 0-15%; productive 25-60%; interface, no yield 10-40%; model, no yield 5-30%; no plan 0-15%; refused input 0-5%; the rest at most 1% | 0.0%; 13.5%; 23.3%; 0.0%; 10.0%; 0.5%; 0.01% | 5 of 7 |
| (a) | if the stop is stagnation, every selectable cell tried; a dialog state gets a try | no stagnation stop; 9 dialog-state entries got a try | 1 of 1 scored |
| (b) | 0-5 trials refused by the bridge; no try ends on a bridge error | 86 refused trials; no try ended on an error | 1 of 2 |
| entries verified from boot | every entry, 0 differences | 174 of 174 | hit |
| the VM | 1-1.5 h used, $1.5-2.3 | 0.39 h, $0.60 | miss |
| the laptop | 0.6-3 h | not run | not scored |

**Misses, stated plainly.**

1. **Rooms: 1, against 2-5.** Room 49's scope grew to {42, 48, 49}, but no plan or model-free step left the room.
2. **Productive 13.5%, against 25-60%; model, no yield 0.0%, against 5-30%.** The table's largest class, effect
   mismatch at 38.4%, and model-blind at 14.1% were not predicted at all. The plans now exist, and the engine refutes
   them.
3. **86 trials refused by the bridge, against 0-5.** All click actor 7 left of the screen. The offer gives an actor
   the point (x - camera, y), with no check that the point is on the screen. The bridge's hit test answers for it, and
   its input check refuses it. The fix of (b) made each one a refused trial, as planned, so they cost 17.4 s.
4. **The VM: 0.39 h and $0.60, against 1-1.5 h and $1.5-2.3.** The projection's 10-30 minutes of driver wall was
   7.7 minutes: model tries took 52 s on average on the VM, and free tries 26 s.

### Threshold and what bound it

**Partly**, by the success threshold. The explorer ran from the boot with the model working after the opening. It made
progress (62 new atoms of T, 10 inputs deep), and every entry is exact. The end was not reached and no other room was
entered.

1. **The falsifier at 80 tries** stopped a run that was still finding new atoms. With b = 10, 80 tries take
   7.7 minutes on the VM.
2. **Plans the engine refutes.** The plan for verb 10 on object 592 runs, and the record's predicted write is not in
   the engine's run (effect mismatch, 38.4%). Whether the widened records of amendment (6) make these records too
   general is not measured.
3. **Starts the reading does not reach** (`pos:script:151 = 59`, 16 tries) and var 165's values outside its domain at
   refinement (5 tries).
4. **Two new rows**, recorded and not applied: 23 (the refined task lacks the target's fact) and 24 (an actor's offer
   point off the screen).

## Amendment 2026-11-04 (8): plans the engine refutes and starts the reading does not reach, diagnosed; the matcher's step, sound commands, offers on the screen; the run from the boot to a wall limit

Written and committed before the code it plans is committed. The sections above stay as written and their predictions
are not edited. The diagnosis below was measured on play7's tries run again on the laptop (`fmi try` on a copy of the
job with its scope and a fresh reading, the run's switches and build) and with a probe of the engine (fmi-checks'
`probe state` and `probe watch`: the movie from boot, then per frame the slot runs, the explain log's opcodes, reads
and writes of chosen units, the slots' locals and cutscene levels). Fixes (a) and (b) were tried as prototypes on single
tries to confirm the causes; their code is committed after this amendment, with tests.

### The diagnosis, with play7's numbers

**1. Effect mismatch (1,316.6 s, 38.4% of the try wall): the matcher reads a started script's arguments one run too
late.**

- **Where the seconds are.** 1,313.8 s of the 1,316.6 s (99.8%) are one plan: walk to object 592 (verb 10), then the
  sentence script's resumption after its walk, for var 52 = 0 (the cursor off). Nine tries planned it from nine entries
  (33-36, 107, 108, 119-121); seven are classed effect mismatch (1,095.0 s), two productive (their model-free step kept
  atoms; 218.8 s of their rows are effect mismatch). Each stopped at the second "same empty window" after 3 windows of
  up to 3,600 frames (108-178 s per try).
- **Reproduced.** j00074 (entry 36), the target forced to var 52 = 0, on the laptop: the same stop on the same record
  (`event(script:2@882)`, the sentence script after its WaitForActor), at the same frame (8,292), 87.6 s.
- **What the engine runs** (probe watch at the window's click). The sentence script walks the ego to the object. The
  ego already stands at the object's walk point, so WaitForActor does not block (ScummVM's o5_wait goes on when the
  actor is not moving) and the script runs on in the same frame to startObject, which runs the object's verb script at
  once, nested in it (runScriptNested). That script opens a cutscene (the cursor goes off: the goal holds in 249 of the
  window's 294 children, "goal, not from the plan") and parks at its first breakHere (offset 35, its locals the verb and
  the second object). Then, before the next decision point, the engine's slot loop of the next frame runs the object
  script again, 35 to 36. The observed step holds both runs.
- **What the matcher does.** The sentence script's run passed a wait that did not block, so Lemma P splits it, and the
  part after the wait is matched to the record. The record predicts `args:obj:592`, the object script's locals against
  the reading's lists at offset 35. The matcher checks it on the view at the step's end, where the object script is at
  36, which has no list: the view's args read 0 and the record is refuted ("effect args:obj:592: the predicted write
  is not in the run"). A key that a later run of the step writes is exempt only when that run is of the plan's units
  (and a unit's own args); the object script is not one of them. The cause finder has no write site for a slot's args
  either, hence "no cause".
- **The four questions.** *Widening:* no. The record's checked effect is an exact value, its two checks are the walk's
  wait and the distance to the object; the try with `FMI_READ_WIDEN` off is in the result. *A missing script or
  condition:* no, the record is right; the engine writes what it predicts. *Timing:* yes, the matcher's step: its
  last view is after a later run of the started script. *Learning:* no. A refuted record is not banned: the replan in
  the same try plans the same two steps (plan 2 = plan 1) and the try stops at the second empty window. Across tries
  nothing carries over: the same target with the same plan was tried from nine entries. (The same holds for the 7
  tries on var 104 = 3, which the planner proved unreachable each time: 308.6 s.)
- **The mechanic.** A script that another starts runs at once, nested in its starter, up to its first yield; the
  engine's slot loop can run it again before the next decision point. The step a matcher observes between two decision
  points then ends with the started script past the point its starter's record leaves it. Any started script that
  yields at once and runs again in the next frame (a cutscene, an animation loop) meets this.

**2. Model-blind (482.1 s, 14.1%): the reader has no rule for sound commands.**

- **Where the seconds are.** 16 tries (312.8 s of model-blind rows) stopped at a reach extension: the start holds
  `pos:script:151 = 59`, and the extended reading does not reach it either. The rest is Lemma R's R0 on var 165
  (5 tries, 150.9 s) and 4 probes with no attempt (18.4 s).
- **The script.** A global script that the room's entry starts: a delay, then a loop of a sound-running test, a random
  draw, a sound command (`soundKludge`, the iMUSE command queue) and a variable delay, where it parks (offset 59). Every
  state after its first delay holds it there.
- **The reading.** In every reading measured (the boot and entry 3 with widening on and off, entry 36), the script's
  position takes only -1 and 4. The one record of its resumption at 4 is a path that stops it. The reader's machine has
  no rule for `soundKludge` (scumm-machine `heads.rs`: "no rule"), so a path through it ends "unmodelled", which the
  reader drops. Every way to offset 59 passes the sound command; a reading extended from a start at 59 drops the same
  paths. The demo's scripts hold 275 `soundKludge` in 39 units (`results/inventory`): every path through one is lost.
- **Not a suspended script's starting point:** the readings start from the start's slot positions (the boot reading
  holds the script at 4 and reads its resumption); the gap is the op's rule.
- **The mechanic.** Adventure games play sound from their scripts (ambient loops, effects inside verb scripts, music
  cues in cutscenes). A sound command changes nothing a script reads except the sound engine's answer, which the
  engine writes outside the scripts (ScummVM's processSoundQueues writes VAR_SOUNDRESULT). A reader that stops at it
  never sees a script past its first sound.
- **Prototype.** With sound commands as ops with no tracked effect, the reading from entry 36 reaches offset 59 (2 more
  records; 40.3 s against 38.8 s); j00014 (entry 11), which stopped at the reach extension at frame 1,868, plans there
  and plays on.

**3. The refused offers (row 24)** and **4. the refined task without the target's fact (row 23)** are as recorded.

**5. Found, not fixed: the executor's walk-to verb is Monkey Island's.** The executor's profile (fmi-play
`executor.rs`, `DOS_CD`) is Monkey Island's: walk to is verb 11 in the selected-verb variable. The demo's interface
resets that variable to verb 10. To reach an object off the screen the executor walks toward it and first waits for
the selected verb to be walk to; in the demo that never happens ("the verb did not return to Walk to within 120
frames", every child of j00014's windows after fix (b)). In play7, 3 tries stopped on walk to object 596 (54.7 s). A
row of the assumptions table; not applied here (the profile's numbers are not read from the game's files yet).

### The fixes (each a switch, on for both games; rule (2) checks Monkey Island with all on)

- **(a) `FMI_ARGS_LATER=1`** (fmi-play `strict.rs`). A record's predicted `args:U` is not checked against the step's
  last view when a run of U at depth 0 comes later in the step, whatever unit set the plan has: in the strict
  matcher's verdicts, in the parts of a split run, and in the input's match. The step's runs are passed to the matcher
  for this. Unset: as before.
- **(b) `FMI_READ_SOUND=1`** (scumm-machine, scumm-reader). `soundKludge`, `setRoomShadow` (the palette's shadow
  table) and `debug` (a debug print) have no tracked effect, as `startSound` has none (`heads::effect_with`,
  `Machine::sound_ops`); VAR_SOUNDRESULT is a variable the engine writes, not a fact (`domains::engine_why`); the
  reader's memo keeps its own store under the switch. Unset: as before.
- **(c) `FMI_OFFER_SCREEN=1`** (fmi-explore `afford.rs`, row 24). An actor's offer points are kept only on the screen,
  0 <= x < 320 and 0 <= y < 200, as the objects' points are cut to it and the walk boxes' points are cut to the main
  screen. The bridge refuses x or y below 0. Unset: as before.
- **(d) `FMI_GOAL_FACT=1`** (fmi-explore `harness.rs`, row 23). Before a plan, a target fact the task's reading does
  not hold after a refinement stops the attempt as "refinement: the refined task holds no fact of the target", class
  model-blind, without calling the planner. Unset: as before.
- **(e) The run's wall limit, `FMI_STOP_WALL=S`** (fmi-explore `driver.rs`). After S seconds of the driver's wall no
  try starts; the running tries finish and are committed; the stop is "wall limit". Unset: no limit. It changes no
  try.
- **Tests:** (a) a step whose later run is of a unit outside the plan: refuted off, matched on, and a later run of
  another unit's args does not exempt; (b) a unit whose path passes `soundKludge`: ends "unmodelled" off, parks at the
  next wait on; VAR_SOUNDRESULT's reason with the switch; (c) an actor left of the screen: offered off, not offered on;
  (d) a task without the target's fact: the stop's text and class; (e) the driver stops at the wall limit with no try
  cut.

### The run

- **The build:** e014's head with (a)-(e), in `/var/tmp/fast-mi/target-e014e`; the engine svm-e013, unchanged.
- **The switches:** `a8/env.sh` = a7/env.sh (every E012, E013, E014 switch; the bound 300 s; grown scopes;
  `FMI_READ_WIDEN=1`; `FMI_STAG_CELLS=1`) with `FMI_ARGS_LATER=1 FMI_READ_SOUND=1 FMI_OFFER_SCREEN=1 FMI_GOAL_FACT=1`
  added to ALLON and A4; the game's settings as before (`a8/run-env.sh`).
- **The archive:** a new one, `/var/tmp/fast-mi/e014-archives/play8`, from the boot. Seed 1; amendment (4)'s guide
  file. Stage play, goal end = 1, the rooms from the files, E009's switches, play when ready, commit as finished.
- **The falsifier is replaced by a wall limit.** The 80-try falsifier was set when a try took minutes on the laptop
  (amendment (3): 3-5 minutes; a reading after the opening alone took more than 15 minutes before amendment (6)). In
  play7 a try took 42.9 s on average and 80 tries took 460.7 s of driver wall at b = 10 (5.8 s per committed try): the
  falsifier stopped a run that was still finding atoms after 7.7 minutes. The limit now is the VM's wall: at most
  2.5 h up. Setup, push and the check took about 15 minutes in amendment (7); the stall table, the verification
  (24 at a time) and the copies back about 15 minutes; running tries finish within about 10 minutes of the limit (a
  try's reading is bounded at 300 s). So the driver's limit is `FMI_STOP_WALL=6600` (1 h 50 min), and
  `--stop-jobs 100000` (no falsifier in effect). E009's stop rules stay: the goal, stagnation (with `FMI_STAG_CELLS`'s
  sweep), an exhausted archive, the disk floor.
- **Where.** The cloud VM, one start: `python3 scripts/cloud.py start --type n2-custom-30-122880 --hours 3 --standard`
  from main's scripts ($1.5346/h: $4.60 booked of the $11.85 left; at most 2.5 h used, $3.84). b = 10, 2 readings in
  the background; `a8/vm.sh` (a7/vm.sh with this build, environment and archive, and a 2-try check archive `vm-check8`
  made on the laptop first, verified from boot on the VM, run again there and compared). The VM is stopped with
  cloud.py as soon as the run ends, and the ledger records it. If the start is refused: the laptop (a4/laptop.sh's
  pattern, b = 2, 1 reading in the background) with the same wall limit; the write-up says which ran.
- **Disk.** The VM's disk is 30 GB. A try's folder holds 0.3-20 MB, a fresh reading 16-20 MB (evicted by size). If
  the disk floor stops the run, it is E009's stop and is written up as such.
- **Verify.** Every entry from boot on the lean engine (`fmi explore verify`), 0 differences. For a goal entry, its
  chain must hold the system's own inputs on the way to the end site. A chain of waits only, or the demo playing itself
  to its end, is reported as such and not as reached.
- **Projection.** Tries take 40-120 s (play7: 42.9 s; plans that now play run their windows), so 1 h 50 min at b = 10
  gives 550-1,650 tries.

### Stop rule

- The goal; E009's stop rules (stagnation under `FMI_STAG_CELLS`, an exhausted archive, the disk floor); the wall
  limit; Google's stop at the booked time is the hard backstop.
- It runs once. Exactness first. A break that stops the driver is fixed only by a dated amendment and rule (2), and the
  run resumes from its files. Any other break is a row of the assumptions table, recorded and not applied. A stop by
  hand only if something is clearly broken, with the reason written down.
- Before the run: rule (2) on Monkey Island with every switch on (`A4_ENV=a8/env.sh a4/mi-check.sh`): B3's 15 tries
  equal E013's G-on, their new entries verify from boot, the kitchen's goal chain verifies, the smoke run's entries
  verify and its stall report passes. If a check differs, no run: the difference is written up first.

### Predictions (before the code is committed)

| | predicted |
|---|---|
| rule (2) on Monkey Island | B3's 15 tries equal E013's G-on; their new entries, the kitchen chain and the smoke run verify |
| the plan walk to object 592 for var 52 = 0 | reaches the goal from the plan wherever it is planned; 0 stops on `effect args:obj:592` |
| how the run stops | the wall limit 60%; stagnation 20%; the disk floor or a break 15%; the goal 5% |
| committed tries | 550-1,650 |
| rooms entered, of the 11 in the files | 1-3, most likely 2 |
| the end | not reached; reached by the system's own inputs and verified from boot: 5% |
| new atoms of T | 80-250 (play7: 62) |
| the stall table | effect mismatch 0-15% (38.4%); model-blind 0-8% (14.1%); refused input 0-0.2% and no trial refused on an actor's point; interface, no yield 20-50%; no plan 5-25%; productive 5-30%; the rest row at most 1% |
| readings | none reaches the 300 s bound in more than 5% of model tries |
| entries verified from boot | every entry, 0 differences |
| the VM, if it starts | 2.2-2.5 h used, $3.4-3.9 |
| the laptop, if not | 2.5 h of driver wall at most |

## Result (2026-11-04): amendment (8) on the cloud VM; stopped by the disk floor after 502 tries, two rooms entered

### In plain terms

- **Where it ran.** The VM started (10:15 UTC), so the run was on the VM: b = 10, 2 readings in the background. Rule
  (2) on Monkey Island and the check before the run passed. The driver ran 5,509.7 s (1 h 32 min) and the VM was
  stopped at 12:05: 1.85 h, $2.84 in the ledger ($20.49 committed, $9.01 left).
- **How it stopped:** by the disk floor (0.5 GB free against the floor of 2 GB), an E009 stop rule, before the wall
  limit. Not by hand.
- **The end** was not reached. The run entered **room 42**, its second room, at frame 1,965, and kept 316 of its 628
  entries there. All 627 entries past the root verify from boot with 0 differences (the lean engine, on the VM).
- **The two diagnosed causes are gone.** Effect mismatch fell from 38.4% to 0.3% of the try wall: the walk to object
  592 for the cursor off reached its target from the plan in all 26 attempts that played it (play7: 0 of 9), and no
  try was refuted on an object's args. Model-blind fell from 14.1% to 1.9%: no try stopped on the ambient script's
  position. No trial was refused by the bridge (play7: 86).
- **What bound it now: the reading.** Reading the sound commands made the readings of the scope that holds the world
  map (room 75) far larger: 177 tries (82 model, 95 free) reached the 300 s bound, 65.0% of the try wall, and 14
  readings stopped at the 8 GB memory limit. Those readings also filled the VM's disk, which stopped the run.
- **And the executor's interface (row 27).** 40 tries (2,838.7 s) stopped because the executor waits for Monkey
  Island's walk-to verb before it walks: every sentence with verb 11, and every walk to an object off the screen.

### The run's numbers

| | |
|---|---|
| committed tries | 502: 264 model (159 with an attempt), 238 free; 486 "ok", 16 errors |
| try wall | 52,117.3 s; model tries 128.3 s on average (median 33.6 s), free tries 76.6 s (median 31.3 s); the first 139 tries 52.8 s |
| driver wall | 5,509.7 s with b = 10 |
| fresh readings | 76, 0.2-66.0 s; 177 more stopped at the bound, all of scope {42, 48, 49, 75}, from j00140 on |
| scopes | {49} grown by room 42 (j00025), 48 (j00068), 75 (j00130); room 42's by 48, 49, 75 and 68; all by case 1 (a target needs one) |
| entries, cells | 628 entries (312 in room 49, 316 in room 42), 197 cells; T 543 atoms at the end (play7: 454) |
| rooms entered | 2: room 49 and room 42 |
| goal entries | none |
| attempts that reached their target from the plan | 46 (play7: 0): var 52 = 0 26 times, class 26 of object 491 set (class:491:6 = 1) 14 times, bit 446 = 1 6 times |
| errors | 14 "the reading's RSS passed 8.0 GB" (j00132-j00138, j00470-j00484), 2 "the sink thread stopped" at the disk floor (j00498, j00500) |

### The stall table by cause (`results/a8/stalls-play8.md`, made on the VM by the run's own `stalls` step)

| class | seconds | share | play7 |
|---|---:|---:|---:|
| reading bound | 33,900.0 | 65.0% | 0.0% |
| interface, no yield | 10,297.6 | 19.8% | 23.3% |
| refused input | 2,366.1 | 4.5% | 0.5% |
| overhead | 2,172.3 | 4.2% | 0.0% |
| model, no yield | 1,020.3 | 2.0% | 0.0% |
| model-blind | 997.8 | 1.9% | 14.1% |
| productive | 829.5 | 1.6% | 13.5% |
| no plan | 312.7 | 0.6% | 10.0% |
| effect mismatch | 158.6 | 0.3% | 38.4% |
| wait | 62.4 | 0.1% | 0.2% |

- **Reading bound:** 177 tries, each 300 s, all of scope {42, 48, 49, 75}; the try then went on model-free from a
  stand-in reading and kept nothing. Reads of that scope sometimes ended under the bound on the VM (76 fresh readings
  in all), and the tries that reused them went on.
- **Overhead** is the rest row (2,167.8 s, 4.16%): the 16 tries that ended on an error have no rows.
- **Refused input** here is the executor, not the bridge: windows whose children could not form the planned input
  ("the verb did not return to Walk to within 120 frames", row 27; 38 tries).
- **Model, no yield:** 43 tries reached their target (mostly the cursor off again) with no new atom.

### The fixes, measured

- **(a) `FMI_ARGS_LATER`.** 35 attempts targeted var 52 = 0: 26 reached it from the plan (0.1-74.1 s, median 3.2 s;
  play7's 9 tries took 108-178 s and were refuted), 9 found it already holding at the plan's start. No stop names an
  object's args. Widening was not the cause: the reading of entry 36 with `FMI_READ_WIDEN` off holds the same two
  records of the sentence script's resumption, with the same preconditions, effects and checks.
- **(b) `FMI_READ_SOUND`.** No try stopped on the ambient script's position (play7: 16). Its cost was not predicted:
  read from entry 159's start (frame 1,438) on the laptop, scope {42, 48, 49, 75}: off, 269.8 s, 61,406 records,
  159 splits; on, still reading at 1,500 s (stopped), 2,753 splits, 1,901 of them in the interface's per-frame hover
  script's resumption, whose objects' verb scripts in the map room now run past their sound commands. On the VM a
  finished reading of that scope held 518,131 records (a packed file of 619 MB, 505 MB of actions).
- **(c) `FMI_OFFER_SCREEN`.** 0 trials refused by the bridge (play7: 86 in 4 tries).
- **(d) `FMI_GOAL_FACT`.** 2 attempts stopped as "refinement: the refined task holds no fact of the target: bit:20"
  instead of a planner error.
- **(e) `FMI_STOP_WALL`.** Not reached: the disk floor came first, at 5,509.7 s of the 6,600 s.

### The furthest state, in the game's numbers

- **Entry 356:** room 42, frame 19,916. The chain is entries 0, 2, 22, 138, 177, 198, 199, 296, 299, 308, 310, 326,
  338, 345, 356: 18 inputs, the system's own (16 clicks and the keys "u" and "o"); the last at frame 9,112. Entries
  345 and 356 are the wait's stops after it.
- **How room 42 was entered.** Entry 296 (j00180, a model try whose reading was bounded, its model-free step): one
  click on a dialog line at frame 1,574 from entry 199; the room changes at frame 1,965. The eight inputs before it on
  the chain are six clicks on dialog lines (x 4, y 154 or 174, with the dialog's lines on) and two clicks at frames
  1,044 and 1,046. The game changed the room after the dialog the system chose its way through.
- **Since the root:** object states 490 = 1, 492 = 1, 494 = 2, 500 = 1; classes 26 of object 490 and 6 of object 491
  set; bits 16, 403, 408, 410, 411 and 446 set, bits 7-10 and 20 cleared; 54 vars changed; the sentence verbs (1,
  3-12, 100-104) on and the dialog verbs off. No object changed owner: nothing went into the inventory.
- **New atoms of T over the run:** 78 distinct (play7: 62), in 25 tries: vars 44, bits 16, verbs 12, and one each of
  a cutscene depth, a frozen slot, a script, a room script, an object state (490 = 1) and a class (491's 6 = 1). The
  last two tries with new atoms are j00495 and j00499, near the end.

### Rule (2) on Monkey Island (`results/a8/mi-check.txt`)

B3's 15 tries equal E013's G-on in every field compared (15 of 15); their 126 new entries verify from boot; the kitchen's
goal chain (8 entries) verifies; the smoke run's 21 entries verify and its stall report passes its 5 checks. MI's
scripts hold no `soundKludge`, `setRoomShadow`, `debug` or VAR_SOUNDRESULT.

### Engineering met on the way (not the method)

- **The VM's disk.** It had 13 GB free at the start (earlier runs' files: play7's archive with its readings, E009's
  copies), so the floor was set to 2 GB before the run (1012d33). The map scope's readings (one cached reading of 1.1 GB; the
  cache recorded a largest reading of 11.4 GB) took the disk from 7.9 GB free at 11:47 to 0.5 GB at 11:57.
- **GitHub** refused pushes twice ("Internal Server Error"); they went through on a later try.

### Predictions scored

| | predicted | measured | |
|---|---|---|---|
| rule (2) on Monkey Island | all four checks | all hold | hit |
| walk to object 592 for var 52 = 0 | the goal from the plan wherever planned; 0 stops on args | 26 of 26 played reached it; 0 such stops | hit |
| how the run stops | wall limit 60%; stagnation 20%; disk floor or a break 15%; goal 5% | the disk floor | (15%) |
| committed tries | 550-1,650 | 502 | miss |
| rooms entered | 1-3, most likely 2 | 2 | hit |
| the end | not reached (reached 5%) | not reached | hit |
| new atoms of T | 80-250 | 78 | miss |
| effect mismatch | 0-15% | 0.3% | hit |
| model-blind | 0-8% | 1.9% | hit |
| refused input | 0-0.2%, no trial refused on an actor | 4.5% (the executor's refusals); 0 refused by the bridge | miss; hit |
| interface, no yield | 20-50% | 19.8% | miss |
| no plan | 5-25% | 0.6% | miss |
| productive | 5-30% | 1.6% | miss |
| the rest row | at most 1% | 4.16% | miss |
| readings at the bound | at most 5% of model tries | 82 of 264 model tries (31%); 65.0% of the try wall | miss |
| entries verified from boot | every entry, 0 differences | 627 of 627 | hit |
| the VM | 2.2-2.5 h, $3.4-3.9 | 1.85 h, $2.84 | miss |
| the laptop | at most 2.5 h | not run | not scored |

**Misses, stated plainly.**

1. **The reading bound took 65.0% of the try wall, against at most 5% of model tries.** Fix (b) lets the reader run
   past sound commands; the world map room's scripts are full of them (its entry script has 18), and the interface's
   per-frame hover script runs the hovered object's verb script, so the scope with the map room went from 270 s to
   past 1,500 s on the laptop and to readings of 518,131 records. The prediction did not consider that a reading that
   drops paths is cheaper.
2. **The disk floor stopped the run at 502 tries, against 550-1,650 and the wall limit.** The same readings filled the
   VM's disk.
3. **Productive 1.6%, no plan 0.6%, interface 19.8%, the rest 4.16%, all outside their ranges.** The reading bound's
   share pushed every other class down; the rest row is the 16 tries that ended on an error (14 at the reading's
   8 GB memory limit, 2 at the full disk).
4. **Refused input 4.5% against 0-0.2%.** The bridge refused nothing, as predicted; the class's seconds are the
   executor's own refusals of row 27, which this amendment found and did not fix.
5. **78 new atoms, against 80-250.**
6. **The VM: 1.85 h and $2.84, against 2.2-2.5 h and $3.4-3.9,** because the run stopped early.

### Threshold and what bound it

**Partly**, by the success threshold. The run went from the boot into a second room with every entry exact, and the
model's plans reached their targets 46 times (play7: 0). The end was not reached and nothing went into the inventory.

1. **The readings of the scope with the world map** (65.0%; 14 at the memory limit; the disk floor). The hover
   script's fan-out over the room's objects multiplies paths that widening cannot summarise (forks on facts), and the
   split ladder repeats them; with the sound commands read, the map room's objects' scripts run on. Amendment (6)'s
   open item ("the hover script's fact forks are the next cost") is now the first cost. A row of the assumptions table
   (29), recorded and not applied.
2. **The executor's interface numbers are Monkey Island's** (row 27): 40 tries stopped on verb 11 (13, object 593) or
   a walk to an object off the screen (26, object 596); 11 more on inventory sentences and 10 on a two-object sentence
   (verb 8 on objects 496 and 491: the sentence script did not start in the matched step) were not diagnosed. Picking
   up an object never formed an input, so the inventory stayed empty.
3. **Nothing is learned across tries** (row 28): the cursor-off target was reached 26 times and kept being chosen.
