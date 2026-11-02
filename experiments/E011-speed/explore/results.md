# E011 explore: round 1 results

The explore component of experiments/E011-speed/plan.md: explore.py, step.py, guide.py, replay.Harness's planning
loop and replay_part1.py's wiring, in Rust (`rs/fmi-explore`, the binary `rs/fmi`). Branch `e011-explore`.

## What is built

| Python (frozen reference) | Rust (`fmi-explore`) |
|---|---|
| explore.py `Archive` (entries, cells, held atoms, selection, ingest, readings, eviction, E009's idle set and rotation, E010's switches) | `archive.rs` |
| explore.py `run` and `Job` (start order or E010's commit as finished, stops, resume, the guard's kills, the floor) | `driver.rs`, `machine.rs` |
| explore.py `cmd_init`, `Archive.verify`, size_scaling.scopes | `explorecmd.rs` |
| fake_step.py | `fake.rs` |
| step.py's parts (targets, rotate, reusable, idle_rule, entries_from, start_row, records) | `step.rs` |
| step.py `run_job` and `attempt` | `tryjob.rs` |
| replay.Harness's loop with the window search (`plan`, `_plan_taskcore`, `_planned`, `_same_input`, `_extend`, `_swap`, `_refine_turn`, `_run_windows`, `result`) | `harness.rs` |
| reader/replay `task_start`, `replan_start`, `elsewhere`, `view`, `prediction`, actor rooms, `cost_caps_of`, `capped_cost`, `refinable`, `reach_misses`, `reach_start`, export_records.write_start | `start.rs` |
| replay_part1 `reader_switches`, `with_facts`, `reach_extender`, `refiner`, `goal_text_of` | `wiring.rs` |
| guide.py (map, stats, the reader's rules) on scumm-ir's `Game` | `guide.rs` |
| Python's json.dumps and explore.py's file helpers | `pyjson.rs`, `files.rs` |

The ports beyond fmi-core's are in `ports.rs` (COORD.md, IFACE 16:55). `fmi` has `explore init|run|verify|status`,
`try JOB_DIR [--targets K=V,...]`, `guide map|stats`, `fake-step run` and the checks below. `rs/fmi/src/wire.rs` is
where the integrator wires the engine, reading, planner and play ports; in round 1 it wires the doubles only.

## Gates

| check | inputs | result |
|---|---|---|
| G9, the driver (`explore/g9_driver.py`) | 18 scenarios: every switch set of test_explore.py and test_e009.py, E010's two, 1-3 workers; each in process and with a process per job; goal-2w also killed (the driver, or the driver and its jobs) and resumed | 0 differing: entries.jsonl, cells.json, readings.jsonl, summary.json, every job.json, select.json, result.json, the entries and new entries (decompressed), log.jsonl as bytes with the time fields blanked |
| G9, the guide (`explore/g9_guide.py`) | door, kitchen and part1 goals, and the kitchen's wide map | guide.py's bytes but `stats.seconds`; `guide stats` the same |
| G9, step's parts (`explore/step_fixtures.py`, `fmi check step`) | 1092 recorded cases: 663 targets (with E009's rotation), 276 reusable, 153 idle_rule | 0 differ |
| E008's and E009's unit tests, ported | test_explore.py and test_e009.py (archive): 28; test_step.py and test_e009.py (step): 19; test_guide.py: 11 | all pass. The loop tests run as G9's scenarios against explore.py; test_guide's two checks against trace_explain.Code are covered by the byte comparison |
| the harness's start rules (`explore/harness_fixtures.py`, `fmi check harness`) | 416 cases on 26 recorded readings and 166 recorded entries | 0 differ (start file, input key, view, counts) |
| re-ingest (`fmi check reingest`) | E008's kitchen (157 jobs, 372 entries), door and smoke archives, E009's door archive | entries.jsonl, cells.json and log.jsonl byte for byte; every job.json equals job_doc at its fill (but "readings", which eviction changed); every entry file equal |
| replay of recorded tries through run_job and the harness, every port a double (`fmi check replay`) | E009's test (a) (3 plans, 2 refinement turns, goal), E008's step-tests ta and tb (tb with a reach extension) | 0 differences: reading, targets, probes, plans, refinements, extension, run record, inputs, entries (content); every plan's start file and reach-01's start file byte for byte |

G10, G11 and G12 need the other components and run at integration.

### A dry run of test (a) with the real ports

Before integration, `fmi try` ran test (a)'s job (rooms 28, 33, 41 from the root, to owner:566 = 1) in a local tree
that merges the six component branches, with thin adapters in `wire.rs`: returns from fmi-engine's pool (lean build),
readings by readio's reader-scope, fmi-plan's planner, fmi-play's strict window search. That tree is not on this
branch.

| settings | outcome | against the reference try (E009, off/step-a ta) |
|---|---|---|
| strict windows, `FdLamaFirstCosts` | stop goal, frames 2728 to 3216 (488), qf 11712, 2 replans, 36 inputs, 8 entries | the same numbers; the 36 inputs equal; the 3 plans have the same lengths and frames; both refinement turns the same; the 8 new entries equal the reference's as JSON |
| strict windows, `Gbfs` | stop "no plan (the planner proved none)" at frame 3174 (446 frames), 3 replans, 29 inputs, 7 entries | another route: plans 0 and 1 have 28 and 13 steps (FD: 30 and 14). The play deviates at 3171 (state:316 = 1 refutes a record's precondition, class semantics). At 3174 the window is empty (class split, on a position). The reach extension adds 6 values (room = 0 among them), and the planner proves no plan. |
| strict windows, `Gbfs`, `--tail` | the same stop, frame and entries; plan 2 is the old tail (11 steps) | |

So G10's first clause holds on this dry run, and its `Gbfs` clause does not: with `Gbfs`, test (a) does not reach
the goal. A return here is a fork of the kept root, whose stop is "forked", not an idle stop. Such a return is held to
the root's checkpoint (facts, tick, random state, actors) and not to a boot's idle-stop checks.

Test (a) with `FdLamaFirstCosts`, before (the reference) and after (`fmi try`, one cold process, load 4-6):

| part | before | after |
|---|---:|---:|
| the try, wall | 548.9 s | 101.8 s |
| return | 3.8 CPU s | 0.38 s wall (a fork of the kept root) |
| read (fresh reading of the scope) | 55.3 s wall, 130.1 CPU s | 14.0 s wall, 45.6 CPU s |
| plan (3 plans, cold planner) | 139.9 CPU s | 13.4 CPU s (71 s wall, FD's first plan 50 s) |
| play (4 windows, 501 children) | 398.8 CPU s | 21.5 CPU s |
| the try's own time outside the ports | | 41.8 ms (the first try of a process: the guide and the reached values are read here) |

## Measures

Laptop, nice 10. The load average was 1.5 to 12 (other builders' heavy steps); each line gives it.

| measure | before (Python) | after (fmi) | target |
|---|---:|---:|---:|
| the guide, door/kitchen/part1 (load ~8) | 34.3-35.2 CPU s, 59-75 s wall | 0.93-1.0 CPU s, 1.0-1.9 s wall (0.75-0.83 s at load ~3) | ≤ 1 s |
| the driver's own work per job on E008's kitchen archive re-ingested (job.json, selection, ingest; 157 jobs; load 6-12) | CPU median 88-210 ms, mean 139-201 ms | CPU median 11.3-16.4 ms, mean 18-29 ms | ≤ 5 ms |
| the ingest of a try of this process (test (a)'s 8 entries; load ~3) | | 2.2 ms (1.8 CPU ms); the whole driver for the job 8-9 ms | ≤ 5 ms |
| a try's own time outside the ports (test (a) replayed with doubles: 3 plans, 2 refinement turns, 8 entries; median of 60-80 tries) | | 10.3-11.2 ms at load 1.5 (min 9.4); 46.6 ms at load 10 | ≤ 10 ms |
| process start per try | 160-173 ms (python, import step.py) | 0 in process (fmi starts in 2 ms) | 0 |
| Game.load, reuse_keys per try | 3.3 s, 9.6 s | 0 (no Game in a try; reuse keys are readio's, cached) | 0 |
| a whole in-process run of one job (open, select, try with doubles, ingest, summary; default poll 1 s) | | 0.107 s | |

Where the time went and what changed:

- **The archive.** Facts are read in place from the checkpoints (no copied maps); a job's entry rows go in one append
  and one sync; cells.json is written from each cell's cached text; floats are read exactly (serde_json
  `float_roundtrip`), so a rewritten file keeps the reference's bytes.
- **A try's entries.** The reference writes each new entry with every checkpoint of its segment, so a job of k entries
  writes about k²/2 checkpoints. Each checkpoint's text is now made and compressed once, and every file is gzip members
  joined (head, the checkpoints' members, tail), which gzip.open reads as one text. The archive's entry files use the
  same members, and the ingest takes a try's entries from memory. Test (a)'s entries: 393 ms, then 9-12 ms, then 3.5 ms.
- **The harness's per-plan work.** Key texts are cached per thread; the start file's key order is made once per
  reading; the input key reuses the last plan's key order.
- **The driver's wait.** An in-process job wakes the driver when it ends, instead of the next poll (1 s by default).

What is left in a try's own time (test (a), load 1.5): the fresh reading's bookkeeping 2.3 ms (the start row, the
record), the targets 1.7 ms (the guide and the reached values, read once per process after the first try), the
harness 3.4 ms (3 input keys, 1 ms each, mostly sha1 over 100 KB), the entries 3.5 ms, the rest 1.9 ms.

## Deviations

- **The files a try writes.** fmi's result.json adds `explore_s` (wall, ports, own). New entry files are compact JSON
  in gzip members, facts in key order (the reference: facts_of's order, default separators). The content is equal as
  JSON, and explore.py reads them.
- **The archive's files.** Entry files are gzip at the fast level, and a try's entry files are gzip members. All other
  archive files are explore.py's bytes. A job's entries.jsonl rows are appended in one write with one fsync (the
  reference: one per row). Each entry file is still written before its row.
- **The plans' entries in run.json.** There is no FD landmark count or serve reply, because PlanResult has none, and
  no write_s or decode_s. `source` and `cpu_s` are added.
- **The guard in process.** guard.sh's rule runs as the driver's: under 3 GB MemAvailable it kills fmi's child
  processes and cancels the running tries, with guard.sh's log lines. The job's rc is guard.sh's own exit (0), as
  explore.py records it.
- **Not ported.** F10's slot locals (`_replan_locals`, `replan_start_locals`) and the step rules without the window
  search are off in E009's settings, so the harness refuses them.
- **Loop tests.** test_explore.py's and test_e009.py's loop tests are not cargo tests. Their scenarios run as G9's
  differential runs against explore.py.
- **Engine-dependent paths.** `fmi explore init` and `verify`, and `fmi try` and `explore run` on real tries, need the
  integrator's wire.rs. On this branch they run on doubles only. `fmi try` ran on the real ports in the dry run
  above; `explore init` and `verify` have not run on an engine.
- **A pooled return's checks.** A return that is a fork of a kept root is not checked for an idle stop of kind
  sentence (start.py's boot checks), since a fork stops as "forked". It is held to the root's checkpoint instead.
