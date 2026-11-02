# E011 play component: gates and measurements

What was built: `rs/fmi-play`, the Rust port of window.py's strict path with E010's play when ready, strict.py and
the parts of trace_explain.py it uses (over `scumm-ir`), executor.py's input code and replay.py's per-step functions,
as `fmi_core::Player` (`fmi_play::StrictPlayer` over `fmi_play::Search`). Behind the cargo feature `json-double`:
`JsonEngine` (the JSON control protocol against svm-w3b), `FileReading` (a reading's four files) and the differential
tests (`fixture.rs`, run by the binary `play-fixture`). `play/capture.py` records the fixtures; `HITCACHE.md` gives
the proof of the hit cache and its gap.

All runs: the laptop, seed 1, nice 10, 2026-11-02. Other jobs kept the load average at 7 to 31 throughout, so the
walls are inflated against an idle laptop, by up to 3.5 times between runs of the same test at load 8 and at load 15
(see the lean column); the CPU splits are given where they say more. The final runs (on head 7312233) ran at load 8
to 14.

## The fixtures

`play/capture.py` runs E009's step test (a) in process, as the off check runs it, with `Search.play_plan`,
`window_strict`, `_child_strict` and `strict_stop` wrapped (no file of the reference is edited). It records each plan
played, each window's record, every child's row, every read a child makes of the shared best and first-event frame
(with the child's frame and Q at the read), and every stop. Its run gave `CHECK (a): 0 goal True 2728 3216 488 11712
2 36 501 8` and verify 0 differences; the fixtures are in `~/.cache/fast-mi-e011/fixtures/play/a` (0.1 GB).

The run has 501 children (67, 22, 405, 7 in the four windows), as G5 of the engine component and E009's off check
have (plan.md says 502).

## Gates: tests 2-4 through the JSON double (svm-w3b)

Built with `cargo build --profile fast --features json-double` in `rs/fmi-play`, and run as
`play-fixture recs|children|strict|ready FIXTURES [WINDOWS.jsonl ...]` on the final head (the extra files are the
other recorded runs' window records, for test 3). The logs are in `~/.cache/fast-mi-e011/fixtures/play/final/`.

| test | check | result |
|---|---|---|
| records | each window's records rebuilt from the plan and the reading (build_recs, macro_steps, checked_keys, goal writers, plan index, members) against the capture | pass: 4 windows, 0 differences |
| 2. every child | each child from its fork frame, with the reads of the shared best and first-event frame answered as recorded | pass: 501 of 501 children give the recorded outcome, first-event frame, end frame and Q (67, 22, 405, 7); 197 s |
| 3. strict | play_plan on the three plans of test (a) | pass: outcomes replan, replan, goal with 0 differences in cause, checkpoints, inputs, frames and prediction; the chosen children 2728, 2796, 3091, 3204; every window record equal, field by field but times and pids, to a recorded Python run's (below); 172-176 s |
| 4. ready | the same, in ready mode (E010) | pass: the same three outcomes and four children; tries 1, 21, 294, 1 (failed before the chosen child: 0, 20, 293, 0), 317 in all as E010 predicted; 130 s |

The same four tests also pass on the lean build through `fmi-engine` (a local harness that calls
`fmi_play::fixture::run` with `fmi_engine::Host::from_env()`, not committed; G10 runs them at integration): test 2
501 of 501 in 18.8 s, test 3 in 17.1 s (every window record equal to a recorded run's), test 4 in 13.6 s.

### What "equal" means for the window records

Python's window search is not a function of its inputs: its children read the shared best success and first-event
frame while the other children run, so which children are pruned or duplicates, and the counters, depend on thread
timing. The four recorded Python runs of test (a)'s windows (E009's off check, E008's check, E008's step tests, and
the capture) differ from each other in these fields.

fmi-play gives each window the result of the children run one at a time in fork order: a child reads the best and
the first-event frame as the children forked before it leave them. It still runs the children in parallel (a child
waits only when it reads a value a child forked before it has not settled), and its parent runs on past an open stop
frame and goes back to it if the children decide the stop there. On test (a) every window record fmi-play writes is
equal, field by field but times and pids, to one of the recorded Python runs:

- window 0: E009's off check and E008's check;
- window 1: the capture, E008's check and E008's step tests;
- window 2: all four;
- window 3: E008's step tests.

## Measurements

"Before" is the Python reference on svm-w3b (E009's off check for the windows; E010's `frame_costs.py` for the
observed frame). "After" is fmi-play through the JSON double on svm-w3b, and through `fmi-engine` on the lean build.

| measure | before (Python, svm-w3b) | after, JSON double (svm-w3b) | after, fmi-engine (lean) | target |
|---|---:|---:|---:|---:|
| observed child frame | 10.4 ms (a parent frame) | 1.95-7.1 ms | 0.31-0.57 ms (load 8-10); 0.43-2.3 ms (load 13-17) | ≤ 0.2 ms lean, ≤ 1.5 ms double |
| test (a), strict: windows 0-3 | 21.2, 3.6, 261.4, 0.4 s | 11.8, 1.1, 157.3, 0.2 s | 0.9, 0.09, 15.4, 0.03 s (load 9); 4.1, 0.5, 54.6, 0.1 s (load 15) | |
| test (a)'s third window, ready, 3 workers | (strict: 261.4 s) | 125.5 s | 12.7 s (load 8); 22.5-29.3 s (load 13-14) | ≤ 5 s |
| a window whose first try succeeds (ready, windows 0 and 3) | 0.4-21.2 s (strict) | 0.08 s (window 3), 0.35 s (window 0) | 0.01 s (window 3), 0.09 s (window 0) at load 8; up to 0.05 and 0.3 s at load 13-17 | ≤ 50 ms |

Test (a)'s third window in ready mode tries 294 children (293 fail before the chosen one at fork frame 3091), as E010
predicted; the four windows take 1 + 21 + 294 + 1 = 317 tries. Children forked past the chosen one that had already
started are cancelled (2 each in windows 1 and 2 through the double).

### Where the observed child frame goes (lean, fmi-engine)

Each observed child frame is four round trips today (run one frame, runs, explain_log, the dump) and then fmi-play's
own work:

| part | ms per observed child frame (final runs, load 8-10) | the same at load 13-17 |
|---|---:|---:|
| run (one frame) | 0.16-0.35 | 0.49-0.87 |
| runs | 0.02-0.09 | 0.12-0.58 |
| explain_log | 0.02-0.03 | 0.06-0.18 |
| state (the dump, decoded by fmi-engine) | 0.04-0.07 | 0.09-0.20 |
| fmi-play: add_entries, the window's view from the dump, the matcher | 0.03-0.08 | 0.07-0.14 |

fmi-play's own part is within the 0.2 ms target; the round trips are not. The engine component's Step (F1) does the
frame, runs, explain and state in one round trip (0.18-0.19 ms for an observed parent frame in its bench); fmi-core's
`Engine` has no such call yet (IFACE below).

### Where test (a)'s third window goes in ready mode (lean, fmi-engine)

| CPU s | final run, load 8 (wall 12.7 s) | load 14 (wall 29.3 s) |
|---|---:|---:|
| this process | 13.1 | 16.5 |
| of it, the children's threads | 12.5 | 15.6 |
| of that, the input code | 10.0 | 12.3 |
| of that, engine calls: obs, at, run, the stone trials' fork and quit | 3.6, 3.8, 0.7, 0.03 | 4.0, 5.1, 0.9, 0.04 |
| the children's engines | 17.9 | 23.3 |
| the parent engine | 0.6 | 0.7 |

Each of the 296 children plays the window's input from its fork frame: about 74 frames, 770 hit tests and one stone
trial (40 frames) each, before its first observed frame. The children's engines alone take 17.9 CPU s, about 6 s of
wall over 3 workers, so the 5 s target needs fewer engine frames per child or cheaper ones, not only a cheaper
fmi-play. Of this process's CPU, most is the driver's side of the round trips (obs 3.6 s, at 3.8 s for the 227,624
hit tests); fmi-play's own logic in the input code is about 2 s. The hit cache (`HITCACHE.md`) answers all but about
170 of the hit tests, exactly on test (a), but is off until the engine reports the screen geometry.

## IFACE asks (in COORD.md)

1. `Engine::step_observe(frame, req) -> (Status, RunsReply, ExplainReply, Obs)`, a provided method whose default is
   today's four calls: one round trip for an observed child frame.
2. `Engine::at_many(points) -> Vec<At>`: the input code's hit tests in one round trip.
3. The bridge reports `_screenB`, `_screenH` and `_forceBannerVirtScreen` (stop status or dump): closes the hit
   cache's gap.

## Deviations

- The window records equal a recorded Python run's, not every run's: Python's are timing dependent and fmi-play gives
  the fork-order result (above).
- The observed child frame is 0.31-0.57 ms on lean at load 8-10 (0.43-2.3 ms at load 13-17) against ≤ 0.2 ms, and
  1.95-7.1 ms through the double against ≤ 1.5 ms. Both are the round trips (four per frame, IFACE 1): on lean the
  frame's run alone takes 0.16-0.35 ms; through the double the run takes 1.2-3.7 ms and the JSON dump 0.56-1.47 ms.
  fmi-play's own part is 0.03-0.08 ms on lean.
- Test (a)'s third window in ready mode takes 12.7 s on lean at load 8 (22.5-29.3 s at load 13-14) against ≤ 5 s
  (see the CPU split).
- A window whose first try succeeds takes 10-80 ms when its input is short (window 3: 2 frames), but 0.09 s on lean
  at load 8 (0.35 s through the double) for window 0, whose input code plays 48 frames, with a dump at most of them,
  before the child's first observed frame.
- The hit cache is off by default (its proof needs the screen geometry, IFACE 3).
- Commit ec98c96's message gives the parent's change as "217 s -> 148 s"; the measured window 2 walls through the
  double were 570.6 s with the parent waiting and 148.4 s with the parent going on.
