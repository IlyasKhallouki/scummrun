# E018: targets per place

## Question

Does play10's archive reach C5 (room 42 holding 946) when a try's targets leave out only the atoms held at its own
place (the room with the objects ego owns), not the atoms held anywhere in the archive?

## Baseline (E017)

- Model tries from "room 49 holding 946" had 0 targets in 93 of 94, because the conversation's atoms are held by
  entries without 946 (job.json "held" is archive-wide).
- play8 crossed the conversation by a model plan for bit:403=1 at its end state.

## Method

- `FMI_TARGET_PLACE=1`: job.json's "held" is the union of the held atoms of the entries at the entry's place,
  and (added after 5 min of the run: 0 of 24 tries from places holding 946 had a target) the kept novelty is judged
  per place too (job.json "novelty"."by_place": an atom is novel where no live entry at the state's place holds it)
  (`Archive::job_doc`). This is the targeting side of E016's place novelty (best-first width search's novelty
  partitioned by a progress feature, here the place). No other change.
- Run: resume play10's archive on the VM (b = 10), E017's set plus this switch. 3 h wall limit, or 40 min with no new
  rung.
- Test: `held_atoms_are_the_places_own`.

- Added at 09:25 (amendment): places holding more objects first in the selection (`place_owned_key`), and the
  no-new-rung stop at 90 min (WAIT=5400): room 42 was reached without 946, which the ladder does not count.

## Predictions (before running)

- Model tries from places holding 946 with at least one target: over 80% (E017: 1%).
- A conversation-depth entry holding 946: 60-80%.
- P(C5) 35-50%.

## Success threshold and stop rule

- **Yes:** C5, verified from boot.
- **Partly:** conversation depth holding 946.
- **No:** neither; then write up.

## Reveal (added at 10:15)

From room 49 holding 946, E015's chain opened the conversation in two actions: walk to object 592, then talk to
actor 11. From play10's states, actor 11 is never in the offer (no `t-11` in any try's trials), and the walk's outcome
holds nothing novel, so it is not kept and the talk never follows. `FMI_REVEAL=1`: an affordance or free trial whose
stop has nothing novel but whose hit test holds objects or actors that the step's offer did not is kept (its novelty
text `reveal=<names>`, at most 3 a step). The free try's next level then offers them. This names nothing in the game:
it is the interface's reach, read from the engine's hit test.
