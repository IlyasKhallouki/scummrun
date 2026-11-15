# E030: the direct search on the Fate of Atlantis demo, to the game's own end

Date planned: 2026-11-15
Research question it serves: RQ4 (generality)

## Question

Does E027's search, with its rules as they finished Monkey Island (full5, binary high8) and one general addition,
play the playable DOS demo of *Indiana Jones and the Fate of Atlantis* from boot to the demo's own end?

The demo was finished before by the earlier system (E022, then shortened by E024). It has served E027 only as a gate
(its first 25 stops, never its end). No rule of the search was made for it.

## The addition: the game's own end as a stop rule

Monkey Island's runs stopped on a value picked by hand (bit 565). The demo has no such value: it ends inside an
object's verb script, by a restart (E022: `restart@1:582:48:0067`, room 48, a room visited long before the end). So
the search gets E014's definition of a game's end, which names nothing of the game: **a trial that the engine ends
by a restart or quit that a script ran** (`systemOps`; the bridge reports it as `restart@SITE` or `quit@SITE` with
`SPEEDRUN_END_SITE=1`, on in every run; a restart or quit key is never an input of the search).

- `--until end`: such a trial ends the run, as a held `--until` value does.
- The trial is written to `ends.jsonl`: the stop it was made from, its own inputs, the end's reason and frame.
- `direct verify-end ENDS IDX OUT`: plays that stop's movie and the trial's inputs from boot in a fresh engine, runs
  on to the engine's end, and requires the same reason (the same site) at the same frame.
- With neither `--until end` nor `verify-end` used, the search does what high8 does: the gate's six digests must be
  high8's (the binary: high9).

## Prediction (before running)

- The gate gives high8's six digests with high9.
- The end is reached within 3 hours of search on the laptop (7 threads, stall 30): about even odds. What may stand
  in the way, from E022 and E024's notes: the quiz (a random draw of questions), the voyage (E024 needed the skips off
  there; the search always skips), and the cave's exit after a random draw.
- If it is reached, the end replays from boot (verify-end).

## Method

- Commit: the one that adds `--until end` (high9), after its gate.
- Command: `experiments/E027-direct/run/chain.sh out/e030 --game DEMO --guide none --rungs none --watch none --until end --hours 4 --stall 30 --workers 7 --bin <high9>`, with keep.py's same options.
- Game: the demo (ScummVM id `atlantis`, Demo/DOS/English, PLAYFATE.000 MD5 035deab53b47bc43abc763560d0f8d4b), its
  scripts' dump (`data/scripts-fate-play-demo`), seed 1, the laptop.
- Held out: the demo's end was never a target of any rule of the search.

## Success threshold

- Yes: an end at a script's site, replayed from boot.
- No: 4 hours with no such end, or the chain's own end rules (three idle legs). Then the notes say where the search
  stood, in mechanics, and whether the blocker is the demo's or the search's.

## Before the run (2026-11-15, 20:45 UTC)

- high9 (this commit): `--until end`, `ends.jsonl`, `verify-end`. Its gate: the six digests of high8 (PASS,
  out/gate-high9). Frozen at /var/tmp/fast-mi/direct/bench/bin/high9.
- Two checks of `verify-end` that could not be made: E024's demo route (`run/seed1-route-004.jsonl`, recorded under
  E022's settings) does not end by frame 60,001 with the search's settings (its clicks fall out of step); Monkey
  Island's end stop does not end by itself within 45,000 frames after it (the game waits at its end). The run's own
  end, if one is found, is the test: it must replay from boot to the same site and frame.
