# E013 `free`, gates D1-D3 (2026-11-03)

Branch e013-free, merged with e013 at 31da643 (E014's pieces and amendment (2)). Method: plan.md section D, free's
amendment and amendment (2) (b).

- **Code:**
  - `fmi-explore/src/free.rs`: switch `FMI_FREE=ρ0`, off by default. It holds the job kind, the free order's port to
    stall (`Shift`), the free try's offers, the delays and the one more level.
  - `archive.rs`: the free counts per cell and the free order.
  - `driver.rs`: the kind at each pick.
  - `tryjob.rs`: a free try plans nothing. It takes a reading for T only, merges T with the archive's T, and runs the
    wait and then the free step.
- **Tests:**
  - `tests/free.rs` (6) and `tests/free_explore.rs` (5, a toy engine).
  - free.rs's own unit tests (3).
  - The whole fmi-explore and fmi suites pass: 131 tests, release profile.
- **Tools and scripts:**
  - `fmi-explore/src/bin/free_check.rs`: `same` for D2, `report` for D3.
  - `E013-robust/free/smoke.sh` and `gates.sh`.
  - The rows are in `d3.jsonl`.
- **Setup:**
  - The laptop, seed 1, nice 10, under the guard, each run holding the heavy lock.
  - The load was 4.3-6.2 while the runs ran (other E013 builders were building).
  - Monkey Island: goal owner:566 = 1, E009's kitchen guide, E012's ten rooms, from the boot.
  - **SET=e12:** ON3 and E12ON on end's engine (patches 0001-0005).
  - **SET=all:** amendment (2) (b)'s one set as far as this branch has it. That is ON3, E12ON, `SPEEDRUN_IDLE=handler`,
    `SPEEDRUN_END_SITE=1`, `FMI_GOAL_END=1`, `FMI_READ_EVENTS=1` and E14P, on E014's engine (0001-0005, 0006-e014).

## Gates

| gate | run | result |
|---|---|---|
| D2 | smoke, b = 1, 4 jobs, `FMI_FREE` unset: this build and target-e012 (E012's merged build), end's engine | same 4 jobs, 15 entries. `free_check same` over 30 files (job.json, select.json and result.json of each job, entries.jsonl, the 15 entry files, cells.json, log.jsonl): **0 differ**. The first comparison found 3 differences, all in `probes[0].dropped[0][1]`, the bytes freed by dropping an attempt's unused refinement reading (e.g. 158,587,198 against 158,587,222). In j00000's kept refinement reading, `cmp` shows the bulk files byte for byte equal (reading.tc, actions, lists, idx, reached, start.txt). The files that differ are the report files (reader-scope.err, refine-01.json, reading.json, cache.json), whose text holds timings, loads and RSS. `same` now keeps the dropped folders' names and not their bytes (a9d98aa). Both runs: 14 of 14 entries verified from boot |
| D1 | SET=e12, `FMI_FREE=0.25`, b = 1, 4 jobs | j00003 free, as the sequence says. 11 of 11 entries verified from boot, 0 differences. The free try kept no entry |
| D1 | SET=e12, `FMI_FREE=1`, b = 2, 4 jobs | stopped after 1 job on E012's stagnation: the root's free try kept nothing, and one cell was selectable. 0 entries to verify |
| D1 | SET=all, `FMI_FREE=0.25`, b = 1, 4 jobs | j00003 free. 23 of 23 entries verified from boot, 0 differences. The free try kept no entry |
| D1 | SET=all, `FMI_FREE=1`, b = 2, 4 jobs | 4 free tries. **12 free entries, all verified from boot, 0 differences**: 11 at the first level, 1 at the second, 0 from delays |
| D3 | the four D1 archives | below |
| units | free's kind, counts, free order, port, off files; the free step on a toy engine | 14 pass |

Rule (2) on this branch: with `FMI_FREE` on, in both sets, every entry of every run verified from boot (35 in SET=all,
11 in SET=e12). That covers every entry check. The rule's other check, that the kitchen is still reached, is K1. K1 runs
on the integrated build, with refuse's and stall's switches on, and is not run here.

## D3, per free try

| run | job | entry, frame | offer points | trials | waits run / reused | delays: counted, delayed inputs, trials, kept | kept: 1 / delay / 2 | free step s | try wall s |
|---|---|---|---:|---:|---|---|---|---:|---:|
| e12, 0.25 | j00003 | 4, 2862 | 1 | 32 | 32 / 0 | 4, 4, 28, 0 | 0 / 0 / 0 | 3.0 | 3.3 |
| e12, 1 | j00000 | 0, 2728 | 1 | 130 | 130 / 0 | 58, 0, 0, 0 | 0 / 0 / 0 | 8.0 | 51.0 |
| all, 0.25 | j00003 | 16, 10086 | 1 | 55 | 55 / 0 | 48, 1, 7, 0 | 0 / 0 / 0 | 4.6 | 4.7 |
| all, 1 | j00000 | 0, 2728 | 12 | 1,434 | 1,434 / 0 | 58, 0, 0, 0 | 11 / 0 / 1 | 105.2 | 151.8 |
| all, 1 | j00001 | 11, 2735 | 1 | 112 | 112 / 0 | 58, 0, 0, 0 | 0 / 0 / 0 | 14.8 | 15.1 |
| all, 1 | j00002 | 1, 2731 | 1 | 130 | 130 / 0 | 58, 0, 0, 0 | 0 / 0 / 0 | 17.5 | 17.8 |
| all, 1 | j00003 | 2, 2731 | 1 | 130 | 130 / 0 | 58, 0, 0, 0 | 0 / 0 / 0 | 12.1 | 12.3 |

No wait was reused: without refuse there is no digest, so every trial that stopped ran its wait. No trial ended the
game, so there were no dead moves and no end entries. None of the 12 kept entries came from a delay.

## What the runs show

- **E012's T, at the entries these smokes reach.** The free tries kept nothing, for three different reasons:
  - **At the root (2728).** 130 trials gave no new atom of the reading's T and no new room. E012's afford measured
    the same entry the same way (128 trials, 0 kept; `E012-game-mechanics/results/afford`).
  - **At entry 4 (2862, a dialog).** Its 4 lines each drew more random calls than the no-input twin. They were tried
    again at the 7 delays (28 trials), and none was novel.
  - **At entry 16 (10086, a room with no verbs and 39 walk points).** The trials reached rooms 33, 38, 48, 52, 57, 85
    and 218, all in the archive already. Rooms 48, 52, 57 and 218 came from the model try just before at the same
    entry, through its affordance step.
- **Why the free order chose entry 16.** Its keys start with the free counts. At that point every cell's free counts
  were (0, 0), so the free order was E012's order, which put entry 16 first.
- **Stagnation with every try free.** With `FMI_FREE=1` and E012's set, E012's stagnation rule stopped the run after
  the root's try: no try had given a new atom, and the sweep was one cell. E014's `FMI_STAG_SWEEP` widens the sweep only
  to the cells selectable at that point, which is also one. So in SET=all the run went on only because the root's try
  kept entries.
- **E014's T at the root.** With `FMI_NOVEL_VERBS` and `FMI_NOVEL_GUIDE` on, T has 1,154 atoms (594 added). The root's
  free try kept 12 entries:
  - 8 states of var:107, one for each of 8 of its values;
  - 4 states of var:100 or var:2.

  It used 12 offer points and 1,434 trials, about 73 ms per trial including the wait after each.

## Deviations from free's amendment

- **D2's comparison.** It sets aside a probe's dropped bytes, as well as the timing and path fields. The reason is in
  the D2 row.
- **D1's sets.** D1 ran in both sets: E012's (as the amendment wrote it) and amendment (2)'s one set.
  - SET=all lacks `FMI_STALLS`, `FMI_FREE_SHIFT`, `SPEEDRUN_INPUT` and `FMI_REFUSE`, the other components' switches.
  - Without stall wired, `FMI_FREE_SHIFT=1` stops the driver by design (`free::check_env`).
  - The amendment's "D1 again with input's and refuse's switches on" is half done:
    - input is met by 0006-e014, and `SPEEDRUN_IDLE=handler` is in SET=all;
    - refuse's verdicts and dedupe are not merged. `free::taken` still counts every trial as taken.
- **The record's shape.** result.json "free" carries "points" as the list stall's reader reads ("where", "frame",
  "level", "seconds", "record", "delays") and "n_points" as the count. This replaces the shape posted in COORD.md at
  16:51. The new shape was posted at 20:15.
- **E014's end entries.** With `FMI_GOAL_END=1`, a free trial that a script's restart or quit ended is also an end
  entry, "how": "end", as in the affordance step. No offer is made from it.
