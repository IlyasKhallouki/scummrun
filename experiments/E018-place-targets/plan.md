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

## Rarity within a place (added at 10:45)

By 10:24 the room-49 place holding 946 had about 620 entries. Only a few were mid-conversation (dialog flags 403-408
set), and they got 3 model and 20 free tries. `FMI_PLACE_RARITY=1`: within a place, after the cell's own count, entries
holding pairs that few rows at their place hold come first. This is count-based novelty, as Go-Explore weights cells
by their attributes' counts.

## Paused (2026-11-05, 11:10 UTC)

- Paused on Ilyas's word ("pause that"). The run was stopped and the VM released and stopped.
- **The ladder:** C5, room 42 holding 946, reached by entry 7987 (j05927, at 896.9 min of the archive's run wall) and
  verified from boot with 0 differences.
- **The archive:** play10, 8,404 entries, kept on the VM's disk at `/var/tmp/fast-mi/e017-archives/play10`.
- **The VM:** about 44% of 30 cores busy with 10 workers, 7 of 117 GB of memory used. Tries took 18 s at the median
  and 204 s at the p90, about 410 an hour.
- **To resume:** start the VM, take the lock, then run `vm.sh resume W 2` with E018's env.sh and the play10 paths (see
  the session's rebook script). W = 20 would use the idle cores.

## The VM's use, measured (2026-11-05, 11:25-11:52 UTC)

- **The machine:** right-sized from n2-custom-30-122880 ($1.535/h) to n2-custom-30-49152 ($1.214/h, 30 cores, 48 GB).
- **Workers:** the walk down from 30 was cancelled at Ilyas's word ("if 30 works stick with it").
- **CPU and memory**, sampled every 5 s from /proc/stat and MemAvailable:

  | | 10 workers (play9/10, spot readings) | 30 workers (68 samples) |
  |---|---|---|
  | CPU busy | ~44% | 95% median, 95% peak |
  | system time | ~27% | 58% median |
  | user and nice time | ~17% | 37% median |
  | memory used | 7 GB | 6.3 GB median, 6.7 GB peak |
  | committed tries per minute | ~7 (410/h) | ~7.7 |

- **Where the time goes:**
  - The kernel's process counter rose by about 9,700 a second, with about 480,000 context switches a second.
  - The live processes (fmi with 112 threads, about 225 ScummVM processes) account for only about 2.6 cores.
  - The rest is processes too short-lived to be sampled: the engine forks of the trials. Each trial and wait forks
    the game, and the forks' copy-on-write faults and teardown fill the kernel's time.
- **Conclusion:** past about 10 workers, more cores feed the fork overhead, not the search. The throughput lever is
  fewer process forks per trial (branching inside one engine process), not more workers.
- The run was stopped at 11:52 to save the rest of the booking: C5, 8,468 entries.
