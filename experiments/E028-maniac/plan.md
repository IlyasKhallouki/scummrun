# E028: the direct search on Maniac Mansion (SCUMM v2)

Date planned: 2026-11-14
Research question it serves: RQ4 (generality): does the search that finished Monkey Island (E027) carry to another
SCUMM game, of the engine's oldest family, with changes only where the engine version differs?

## Question

With the bridge taught what SCUMM v2 does differently (and nothing taught about the game), how far does E027's search
get in Maniac Mansion (DOS enhanced, 1988; ScummVM `maniac`, "V2/DOS/English") from boot, in one chain of runs?

## What is known before any change (2026-11-14, read and probed, nothing changed)

A smoke run of the unchanged search (high5, 2 workers): `boot: frame 200000 room 45 idle None`, then 0 trials. Six
readers and a critic mapped every place the bridge, the search and the dump assume v5 (the map: E028's notes entry).
In order of need:

1. **The first stop.** The idle rule reads the input handler and the sentence script from `VAR_VERB_SCRIPT` and
   `VAR_SENTENCE_SCRIPT`; v2 has neither (0xFF: Var[255], 0) and runs global 4 and global 2 by fixed numbers. Every
   frame is `no_handler`; the segment never starts and no skip is pressed.
2. **Sentences.** MM's sentence line is drawn from the sentence variables, not a verb slot; a verb click changes the
   interface's hash only by var 32, so one verb per round survives and no verb + object sentence is made.
3. **Things carried.** v2's inventory is hit areas in C++ (four boxes and two arrows), not verb slots: nothing carried
   can be used.
4. **Actors' units.** v2 actors are kept in v2 units (`getPos()` against `getRealPos()`), so the actor hit points miss.

Beside these, not needed for the first run: the hover loop of the input handler on verb 15, the input log's hooks in
v2's overrides, the handler numbers for the search (`hands()`), the end site of `o2_restart`, a v2 scripts dump for
`--go`, and the overshoot of PROBE_LONG (a separate change, measured on MI).

## Method

- **Engine.** svm-e028: a copy of svm-e027 with patch 0012 (`scumm-speedrun-v2`): items 1-4, each inside a
  `_game.version <= 2` branch or with a v2-only effect (`getPos()` is `getRealPos()` for v > 2). Nothing names a room,
  object, verb or variable of a game; the v2 numbers (scripts 2 and 4) are the engine's own constants for the version.
- **Search.** high6 (commit 46d72a3), unchanged. No guide, no rungs, no scripts dump for MM: the search runs on what
  the engine shows. chain.py takes the game's watch and end values as options (MI's stay its default).
- **Gate before the run** (byte for byte): with the same search binary, the old and the new engine give the same
  digests on bench.sh's five references and the same 25 stops of the Fate of Atlantis demo.
- **Run.** chain.sh on MM from boot with the full flag set, laptop, 7 threads (beside E027's full4), `--stall 60`,
  14 hours at most, keep.py's continuations. Measured: rooms entered, objects owned (the level), kid pairs chosen,
  over time; every claimed stop replayed from boot.

## Prediction (before running)

- After item 1 alone: the first stop at the kid selection, frame < 10 (the probe saw idle at frame 3).
- With items 1-4: the search leaves the kid selection, enters the house (opens the front door) within 30 minutes, and
  owns 5 or more objects within 2 hours. It does not finish the game in 14 hours (no guide, no dump, and MM's
  endings need several kids' items in rooms far apart).

## Stop rule

Gate fails: fix or stop, never run. Run: the chain's own (stall, three idle legs, 14 hours) and keep.py's six
continuations. A blocker that stops every line (no new stop for an hour at the first leg): stop, read, write up.

## Success threshold

Yes: the house entered and objects owned from boot by the search alone, with the engine changes only. No: the search
stays in the first rooms, or the engine changes need a game's numbers.
