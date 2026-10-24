# E002: The cause cloud

Date planned: 2026-10-22
Research question it serves: RQ1, and RQ2 for the cost columns

## Question

In a SCUMM v5 game, what are all the ways a persistent fact can become true, and how large is the full
set of them (the cause cloud) for *The Secret of Monkey Island*?

## Prediction (before running)

None. This experiment counts; it does not test a method. Its numbers feed the size cap for E001 (D3).

## Method

- Commit: recorded at the run.
- Command(s): `census.py` (opcodes), later `cloud.py` (the cloud and its tables).
- Game, release, settings, seed: MI1 DOS CD (SCUMM v5), decompiled scripts in `data/scripts-dos-cd/`.
  Engine: the ScummVM source in `third_party/scummvm/engines/scumm/`.
- Held-out data: Parts II-IV of the E001 key. They get totals only; per-goal results are for Part I.

Steps:

1. **Opcode census.** Every decompiled line carries its opcode byte. Each byte is mapped through
   ScummVM's v5 opcode table (`OPCODE(0xNN, o5_x)` in `script_v5.cpp`), and the census fails if any byte
   in MI1's scripts does not map. It reports which o5 functions MI1 uses and which state-changing ones
   `ScriptIndex` does not read.
2. **Opcode table.** For each o5 function MI1 uses, at the sub-operation level for the multi-purpose ones
   (verbOps, actorOps, roomOps, cursorCommand, stringOps, resourceRoutines, wait, saveLoadGame,
   systemOps, matrixOps):
   - cause columns: what it writes (the fact kind), what it reads, what it starts or waits for;
   - cost columns: whether it blocks (wait, delay, walk, talk, cutscene) and which engine variables or
     settings change how long (the timer, text speed, Esc);
   - random draws are their own kind, because their value depends on the seed.

   Code fills the writes column from each function's C++ body. Reviewers check it and fill the rest,
   citing source lines.
3. **Trigger list.** Every place the v5 engine starts a script or writes a game variable by itself:
   room entry and exit, the hook scripts the boot script assigns, verb handlers through the sentence
   script, the input script on clicks and keys, Esc, timers. Each is labelled generic v5, game-specific
   (by ScummVM's `_game.id` guard) or excluded. The debugger, the save/load screens and boot parameters
   are excluded as false shortcuts (E001 C1).
4. **The cloud.** For MI1, every persistent fact with every cause: a trigger, the chain of scripts it
   starts, the effect, and the conditions around it.

## Output

- Growth table. Start from player verbs on objects (the E001 dump) and add one trigger kind at a time:
  room entry and exit, global and cutscene scripts, timers and waits, random draws, input hooks. Each
  row gives distinct facts, cause edges, mean and median causes per fact, and the facts left after
  E001's travel and engine-bookkeeping rules, the number a cap would apply to.
- Part I: for each of the 99 goals, the trigger kinds that can produce it. Per kind, how many goals it
  reaches and how many goals only it reaches. Goals with no cause at all.
- Parts II-IV: totals only (goals with at least one cause).
- Opcode coverage: the o5 functions MI1 uses, their classification, and what `ScriptIndex` misses.

## Success threshold

This is a measurement, so there is no yes or no on a method. It is usable when:

- every opcode byte in MI1's scripts maps to an o5 function and has a classification;
- every Part I goal either has a cause in the cloud or is explained (a random draw, an engine variable,
  or a gap named in the report);
- the classification cites ScummVM source lines and names no MI1 number (E001 C2).

The raw cloud will contain most of the key, as the dump does, so its recall is not a result. The
results are the sizes, which feed the cap decision, and the per-kind coverage, which shows which causes a
planner cannot do without.
