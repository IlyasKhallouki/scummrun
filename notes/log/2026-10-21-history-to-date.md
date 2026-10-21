# 2026-10-21: History to date (written after the fact)

This entry reconstructs what happened before the notebook existed, from the commit history and the
existing docs. Treat it as a summary, not as raw records; the linked files are the records.

## Phase 1: glitchless Part I (2026-10-04 to 2026-10-05)

- Goal: plan and replay Part I (dock to the treasure and idol trials) with no glitches.
- Built: the engine bridge (game-tick clock, tick-locked audio, sentence injection, idle detection, skips),
  a hand-written PDDL model of Part I with every action cited to a script, Fast Downward planning on
  costs measured in the engine.
- Result: a time-optimal route averaging 22,292 ticks (6:11.5) over 30 held-out seeds
  (`docs/optimization.md`). A unit-cost plan was about 7.5% slower, so measured costs mattered.
- Phase 7: agents that read only the decompiled scripts produced a model that matched 97 of the hand
  model's action signatures, with 9 hand-only (`docs/extraction-diff.md`). The extraction found an exit
  the hand model lacked (path 686), which gave the route its last improvement.
- Lesson: script-derived models can find things a human misses, but a human still supplied the goal,
  the starting state and the action groups.

## Phase 2: Any% on the DOS CD release (to 2026-10-21)

Successes, in the order they were verified (`docs/anypercent/overnight.md`):

| Run | Time | What changed |
|---|---:|---|
| B, no item slide | 28:01.4 | Hand goal lists for Parts III-IV, inputs found by search |
| A1, item slide | 25:24.4 | Record's item slide carried through Otis's cutscene; oars, monkey and feather skips |
| A2 | 24:35.5 | Delay lookahead on the catacomb exits (chambers are drawn at random on entry) |
| A3 | 24:14.0 | A hint that had silently fallen back to search (the cannon) fixed |
| A4 | 24:13.1 | Part II hints fixed (fire, cooking, book) |
| A5 | 24:08.6 | Lookahead on the pirate fights and Carla |
| A6 | 23:39.2 | Four improvement lanes merged (handoff, island, ghost ship, text speed) |
| A7 | 23:34.7 | Part IV lane (root beer on the spectre) |
| A8 | 23:20.9 | Part I lane (bar exit via the curtain, searched fight-1 intercept point, per-frame walks) |
| A9 | 23:11.3 | Part III lane (early inputs, catacomb delay re-searched after the merge) |
| A9 without track 6 | 21:01.483 | Same route with CD track 6 absent; the Last Part card drops from 130 s to 0.2 s |

Failures and dead ends, each of which taught something:

- Letting the search try to beat every hint (`--mode always`) was slower, twice: a locally faster branch
  often hurt a later goal.
- Lookahead before every slow goal cost 24 s: there is one random stream from boot, so any earlier delay
  reshuffled the catacomb chambers.
- Replaying Part I from hints instead of the recorded prefix was 2.8 s slower and broke Credit Early.
- One run failed at Carla's fourth exchange until Carla was added to the lookahead.
- Merging the Part III lane onto the new Part I left the random stream 317 draws away from the lane's
  target at the catacombs. An exact re-sync was impossible (odd gap), so the catacomb delay was re-searched.
- The first track-6 test still waited 67 s on the dock: an 800-frame delay in the route existed only to
  fit the 130 s card into the driver's search window.
- The first video's audio ran about 1 s short. Cause, found later: screen-transition waits produce video
  frames but no audio pump.

Observations that matter for the research question:

- In the item-slide runs, the engine search won 0 goals by itself; every goal was played from a route
  hint. With correct hand goals in run B, search reached 33 of 83 goals alone.
- Random-event alignment (catacombs, fights) was worth about 52 s and couples every later segment to every
  earlier frame.

## Phase 3: autonomy research (2026-10-21)

- A design study of a system that goes from game files to a verified run with no human route. It still
  contains LLM stages, which D1 rules out.
- Probes during that study: a static scan recovered the DOS CD interface profile; an Esc sweep over every
  boot frame found the logo-skip window (frames 46-92) in 137 s; a script interpreter rebuilt the full
  insult table; backward chaining from the ending recovered about 28% of the hand route's goals (Part II:
  0 of 23). These ran in throwaway sandboxes and need to be redone as proper experiments.

Decisions made today: D1 (no LLM in the system) and D2 (research question and title), see `../decisions.md`.
