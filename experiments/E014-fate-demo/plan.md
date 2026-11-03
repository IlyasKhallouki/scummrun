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
