# E001: Goals from the scripts

Date planned: 2026-10-22
Research question it serves: RQ1

## Question

How much of *The Secret of Monkey Island*'s goal structure can we recover by working backward from the ending through the game's scripts, with no human route and no language model?

## Prediction (before running)

Ilyas writes this before step 4, after seeing the Part I results and reading the Part II goal list in `key.md`.

- Band for Part II recall (70% or more, 40-70%, or under 40%):
- Why:

## Method

- Commit: recorded when step 4 runs.
- Command(s): `answer_key.py` (the key), `baseline.py` (step 1); later steps add their own.
- Game, release, settings, seed: MI1 DOS CD (SCUMM v5), decompiled scripts in `data/scripts-dos-cd/`. No engine runs are needed: the analysis only reads scripts.
- Held-out data: Parts II-IV of the answer key. The analysis is built and tuned on Part I only. While it is being built, Parts II-IV print totals only, and their per-fact results are read for the first time in step 4.

### Answer key

`answer_key.py` builds the key from the Any% route. Each route move says what it waits for; those facts are the route's goals, minus four kinds of noise, removed by general rules:

- K1, source: the persistent facts of the route's milestones, each under the Part that first needs it.
- K2, travel: rooms, actor rooms, `VAR_ROOM` and the vars the engine's room-change scripts write.
- K3, engine variables: vars the engine owns (from ScummVM's `scumm/vars.cpp`), such as the frame timer.
- K4, steps: inside one Part, a bit or var needed at several values keeps only its last value.

`key.json` is frozen at the sha256 in `key.md`. If the key turns out to be wrong, the report says so; the key is not rebuilt.

### Steps

1. **Baseline.** Score the quick regression (`baseline.py`, the earlier probe, unchanged) against the key.
2. **IR.** Read every script (global, room, room entry and exit, object) into one representation with real conditions and effects. Run the same backward search on it with no new rules, to measure what the IR alone adds.
3. **Engine rules, one at a time,** each measured on Part I only:
   - object presence: an object that is hidden or untouchable can't be used until a script makes it usable;
   - walking connectivity: which rooms connect, from room exits and walk boxes, including exits that need a door or a vehicle;
   - dialogue: each conversation as a small state machine over its choice conditions;
   - scripts that act on their own: room entry scripts, timers, and scripts that throw the player out unless a condition holds.
4. **Held-out run.** Freeze the code and record the commit, Ilyas writes the prediction, then run once on Parts II-IV.

### Constraints

- C1. The search only follows things a player can do: verbs, walking, dialogue choices and Esc. Paths through debug keys or boot parameters are false shortcuts, counted and reported next to recall.
- C2. No rule in the analysis may name an MI1 object, script, room or variable number. Engine constants from ScummVM's source are allowed. This is checked at the freeze.
- C3. From step 2 on, the analysis finds the ending in the scripts itself. The baseline's starting fact, Bit[565], was picked by hand.

## Metric

- Recall per Part: the share of that Part's key facts the analysis reaches, by exact match. The analysis's own starting fact is not scored.
- False shortcuts: paths to the ending that start with something other than a player action.

## Success threshold

On Part II (held out):

- 70% or more: the design works; the planner fills the gaps by searching in the engine.
- 40-70%: keep the design, but exploration in the engine has to do more.
- Under 40%: the scripts alone aren't enough; the system becomes exploration-first and uses the scripts as hints.

Parts III and IV and the pooled II-IV number are reported too. Part II has 21 facts, so one fact is about 5 points. A result with false shortcuts can't count as "works".

Stop before step 4 if Part I recall, where we are allowed to tune, is still under 40% after all four rules. Write up why instead.
