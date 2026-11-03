# E011 round 2: results (2026-11-03, written after the runs)

The six round-2 branches merged into branch e011-r2, wired into the one `fmi` process, gated and benchmarked on the
laptop against round 1's binaries (seed 1, nice 10, heavy steps under the heavy lock and the guard). The kitchen check
(round 1's binaries) and another experiment ran on the laptop throughout. Files here:

- `gates.md`: G1-G12 with every round-2 switch off and on;
- `tries-ref.md`, `tries-fast.md`, `tries-off.md` (and `.json`): B3 by try and by part (report.py's output);
- `raw/`: B3's per-try times and loads, G11's comparisons with B0, B1's readings and planner sequence.

## Answer after round 2

Partly, as after round 1, with the ring(33) try now inside the threshold.

- **Exactness.** Every gate passes with every round-2 switch off and on, G10 included: with the Rust port of Fast
  Downward's lama-first-costs (e011-r2-route) in the fast settings, `fmi try` on test (a) prints E007's CHECK line in
  strict, ready and ready-with-the-old-tail modes, and the 15 tries under the fast settings equal B0's in everything.
- **Speed.** In one resident process, with the fast settings, the 15 tries take a median of 4.9 s per try both before
  and after round 2 (that median is a fresh ring(38) reading, which round 2 did not change); the ring(33) median falls
  from 17.6 s to 6.1 s, the ring(38) median from 1.75 s to 1.3 s, and the 15 tries' sum from 264.9 s to 193.1 s (-27%).
  Under the reference settings the sum falls from 424.5 s to 199.2 s (-53%), the ring(33) median from 43.5 s to 5.9 s.
- **What binds now.** The reading: 74% of the fast warm sum, 106 s of it the first try's fresh ring(33) reading and two
  refinements. Then the planner (25%), whose ring(33) requests are now 3.0-4.4 s, two thirds to three quarters of it the
  per-start reductions.

## The integration

Merged in this order with no conflict between the branches: e011-r2-engine b80bf2a, e011-r2-play 71c370c, e011-r2-read
15414f1, e011-r2-plan 661fcb2, e011-r2-route 621e519, e011-r2-own 735faec (its four last commits were not on
origin). Then, each its own commit:

- **e011-r2-own's interface request** (COORD.md, 01:08; its patch was never pushed): fmi-core's provided
  `Engine::step_changes` and fmi-engine's override in one Step read from the delta's chunks. It met e011-r2-play's new
  provided methods in fmi-core's `Engine` and in fmi-engine's `BinEngine` impl: two conflicts, both resolved by keeping
  both sides' methods. Then fmi's use of it (fmi-explore's `step_changes` feature) and the window search's data kept
  for the next player (FMI_PLAY_KEEP).
- **FMI_READER_PREP=1** gives reader-scope e011-r2-read's `--prep` (off unless set).
- **The Rust lama-first-costs with the watched h_FF** when FMI_PLAN_FAST=1 (e011-r2-route's note: the same values and
  preferred operators, so the same plans; G7's check mode compares them on every evaluation).
- **The engine**: e011-r2-engine's tree (0001, 0002, 0003) copied to `/var/tmp/fast-mi/svm-e011-r2int`, 0003
  committed there and 0004 applied, built by engine/build.sh (16 files recompiled; no reset).
- **Bench scripts**: a second env file after env.sh (E011_ENV, `~/.cache/fast-mi-e011/env-r2.sh`), round 2's clauses
  in g6.sh and g7.sh, b3r2.sh (round 1's and round 2's binaries interleaved), b1read.sh, b1plan.sh.

Not in round 2: e011-r2-read's reader inside the fmi process (its worktree holds it uncommitted; its last commit is
marked work in progress), so fmi still runs reader-scope as a child process and the memo's store in memory is used by
reader-seq only; e011-r2-plan and e011-r2-own wrote no results file (their final reports were lost), so their own
measurements are not quoted here.

## B3: the 15 tries, round 1's binaries against round 2's

Warm regime (one resident fmi process per sequence, kept entries, warm planners, reading cache and plan store, empty at
the start), the job memo off in every sequence (plan.md amendment of 2026-11-03: the memo's 8 GB disk floor decided it
by /'s level), the two binaries' tries interleaved one by one at load 1.9-5.1. Walls in seconds; CPU is the resident
process's tree.

| settings | binaries | median, 15 | ring(33), 7 | ring(38), 8 | mean | max | sum | CPU median | CPU sum |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| before (round 1's B4: Python, svm-w3b) | | 29.4 | 186.2 | 23.8 | 118.7 | 483.9 | 1,779.8 | 40.8 | 2,746.7 |
| reference (strict, FdLamaFirstCosts) | round 1 | 6.3 | 43.5 | 2.75 | 28.3 | 158.0 | 424.5 | 14.3 | 697.4 |
| reference | round 2, every switch on (the Rust lama-first-costs) | 4.8 | 5.9 | 1.25 | 13.3 | 128.2 | 199.2 | 7.6 | 462.8 |
| reference | round 2, every switch off (alone, after) | 4.8 | 42.1 | 2.65 | 27.7 | 165.1 | 414.9 | 11.4 | 700.1 |
| fast (ready, the old tail) | round 1, Gbfs | 4.9 | 17.6 | 1.75 | 17.7 | 114.9 | 264.9 | 12.2 | 588.8 |
| fast | round 2, every switch on, the Rust lama-first-costs | **4.9** | **6.1** | **1.3** | 12.9 | 121.0 | 193.1 | 7.8 | 456.2 |

By part, the median and the largest of the 15 tries, and the sum (wall seconds; report.py's parts):

| part | round 1, fast: median / max / sum | round 2, fast: median / max / sum | round 1, ref | round 2, ref |
|---|---|---|---|---|
| return | 0.01 / 0.33 / 0.4 | 0.00 / 0.09 / 0.2 | 0.01 / 0.24 / 0.3 | 0.00 / 0.12 / 0.2 |
| idle (1,600 frames) | 0.20 / 0.40 / 3.1 | 0.10 / 0.10 / 1.2 | 0.20 / 0.70 / 3.6 | 0.10 / 0.10 / 1.2 |
| read | 0.19 / 94.5 / 148.1 | 0.05 / 105.8 / 142.2 | 0.09 / 90.2 / 126.1 | 0.07 / 105.4 / 142.9 |
| plan | 0.71 / 19.3 / 110.1 | 0.29 / 14.8 / 48.1 | 1.63 / 65.4 / 288.9 | 0.31 / 21.4 / 51.9 |
| play | 0.02 / 0.52 / 2.9 | 0.02 / 0.35 / 1.5 | 0.15 / 1.85 / 5.3 | 0.15 / 1.06 / 3.2 |
| the rest | 0.00 / 0.09 / 0.5 | 0.00 / 0.09 / 0.2 | 0.00 / 0.09 / 0.4 | 0.00 / 0.08 / 0.2 |
| the try | 4.9 / 114.9 / 264.9 | 4.9 / 121.0 / 193.1 | 6.3 / 158.0 / 424.5 | 4.8 / 128.2 / 199.2 |

- **The first try** (j00000: a fresh ring(33) reading and two refinements, then two plans) is 121 of round 2's 193 s:
  106 s of reading (round 1: 95 s) and 15 s of planning (19 s). Without it the other 14 tries take 72 s (round 1:
  150 s).
- **ring(33) tries on the refined reading** (j00008, j00009, j00011, j00014): 5.4-6.6 s each, almost all planning
  (5.1-6.3 s). Round 1's Gbfs took 17.1-17.6 s on three of them (planning 16.7-16.9 s); on j00008 it played
  another route, which needed a refinement (19 s more). The reference settings show the planner's gain more: 40.8-44.6 s
  with Fast Downward's binary, 5.2-5.9 s with the Rust port.
- **ring(38) tries**: a fresh reading 4.4-4.8 s (round 1's reader 3.6-4.3 s in the same run); these three tries set
  the 15 tries' median. A try on a reused reading: 0.3-0.6 s (round 1's binary: 0.6-1.0 s);
  j00007, with a 1.7 s refinement, 2.0 s (2.5 s).
- **The try's own time** outside the ports (`explore_s.own`): median 13-14 ms, 8-64 ms (round 1's binary in the same
  run: medians 80 ms (fast) and 113 ms (reference), 36-248 ms).

## B1: the reading and the planner, interleaved

**A fresh ring(33) reading** (j00089's start rows, 3 threads, the files fmi reads, no memo), two rounds at load 2.9-4.7:

| reader | wall | reader CPU | peak memory |
|---|---:|---:|---:|
| round 1's reader-scope | 36.3, 35.0 s | 127.5, 122.0 s | 583, 567 MB |
| round 2's, --prep off | 38.7, 36.4 s | 136.3, 127.5 s | 781, 818 MB |
| round 2's, --prep on | 36.9, 40.8 s | 108.2, 117.2 s | 803, 821 MB |

The same packed and reached files in all six. Round 2's reader is no faster at 3 threads; --prep moves 10-15% of the
CPU off the reading's own threads without changing the wall, and round 2's reader holds 200-250 MB more.

**The plan component's ring(33) sequence** (serve33 on j00089's reading, the store off, one round at load 2.1-3.3; wall
per request; the parts of round 2's fast lama request: reductions / build / landmark graph and relaxed task / search):

| request | round 1, Gbfs | round 1, Fast Downward's binary | round 2, the Rust lama-first-costs, fast forms | round 2, Gbfs, fast forms |
|---|---:|---:|---:|---:|
| cold (the reading's first) | 12.3 | 23.3 | 7.5 (6.3 / 0.31 / 0.74 / 0.09) | 7.0 |
| the same request | 0.5 ms | 0.4 ms | 0.7 ms | 0.5 ms |
| another start | 7.6 | 19.9 | 3.2 (2.3 / 0.26 / 0.62 / 0.09) | 2.9 |
| other caps | 7.0 | 20.4 | 3.2 (2.2 / 0.27 / 0.62 / 0.08) | 3.2 |
| another goal | 11.5 | 26.6 | 4.4 (3.4 / 0.28 / 0.60 / 0.10) | 4.1 |
| other caps, that goal | 7.5 | 22.7 | 3.0 (2.0 / 0.26 / 0.63 / 0.12) | 3.1 |

The reductions fall from 5.2-10.3 s to 2.0-6.3 s, the build from 0.8-0.9 s to 0.26-0.31 s; Fast Downward's 13-18 s
search becomes 0.6-0.7 s (the landmark graph, the relaxed task, the successor trie) plus 0.1 s of search.

**Test (a) end to end** (G10's runs, cold, load 12-15): with Fast Downward's binary 213 s (its three plans 109, 19 and
14 s); with the Rust lama-first-costs 68 s (16.7, 5.5 and 3.2 s). Test (a)'s third window, ready (play-lean on the
capture, load 9-11): 17.1 s with every round-2 switch off, 4.5 s with them on; strict: 25.7 s and 8.1 s for the four
windows.

## Each part against its floor

Round 2, warm, fast settings, unless said. The floors are round 1's (results/README.md); "before" is round 1's binary
in the same run.

| part | before | round 2 (median; max) | floor | round 2 / floor | target | met |
|---|---:|---:|---:|---:|---:|---|
| route replay from boot, the door chain (G2's fmi-engine run, load ~12) | 0.80 s (the four switches) | 0.12 s (all six: start 0.04, 3,129 frames 0.08) | 0.08 s | 1.5 | 1 s (threshold), 0.15 s (engine, round 2) | yes |
| route replay by fork | 0.55-0.84 ms (round 1) | not measured again (the cuts leave fork and return as they were: e011-r2-engine) | 0.5 ms | 1.1-1.7 | 10 ms | yes |
| a try's return | 0.01 s | 0.00 s; 0.09 s (the first, a boot) | 0.5 ms | (rounded to 10 ms) | | |
| idle, 1,600 frames | 0.20 s | 0.10 s; 0.10 s | 1,600 x 14-15 µs = 24 ms | 4 | | |
| read, ring(33) fresh (B1) | 35.0-36.3 s | 36.4-40.8 s | the memo-taken reading 12-16 s | 2.3-3.4 | 15 s cold | no |
| read, the first try's reading and two refinements, no memo (B3) | 94.5 s | 105.8 s | | | | |
| read, ring(38) fresh (B3) | 3.6-4.3 s | 4.4-4.8 s | | | | |
| plan, ring(33): another start (B1) | 7.0-7.6 s (Gbfs) | 3.0-3.2 s | round 1's 6-6.5 s (its reductions and h_FF) | 0.5 | 2 s | no |
| plan, ring(33): another goal (B1) | 11.5 s | 4.4 s | | | 3 s | no |
| plan, ring(33): a new goal on the refined reading (B3) | 16.7-16.9 s (Gbfs), 40-44 s (FD) | 5.1-6.3 s | | | | |
| plan, an h_FF evaluation on test (a)'s task | 23-34 ms | 2.95 ms (e011-r2-route, the watched h_FF) | | | 3 ms | yes |
| plan, ring(38) (B3) | 0.47-0.71 s | 0.17-0.38 s | 0.16 s | 1.1-2.4 | 0.3 s | partly |
| play, test (a)'s third window, ready | 17.1 s (switches off) | 4.5 s | 0.45 s | 10 | 5 s | yes |
| play, a window whose first try succeeds | 0.01-0.13 s | 0.01-0.08 s | | | 50 ms | partly |
| the try's own time | 80 ms (113 ms, reference) | 13-14 ms; 64 ms | | | 10 ms | no |
| the 15 tries, warm, fast | median 4.9 s, ring(33) 17.6 s | median 4.9 s, ring(33) 6.1 s, ring(38) 1.3 s | | | median 1 s, ring(33) 10 s | ring(33) yes, median no |

## What binds (round 3)

By share of round 2's fast warm sum (193.1 s): the reading 73.6%, the planner 24.9%, play 0.8%, idle 0.6%, return
0.1%. Round 2 gained 27% of the fast sum and 53% of the reference sum against round 1's binaries on the same load,
so the 10% rule does not stop the rounds.

1. **read** (142 s): the first try's fresh ring(33) reading and its two refinements (106 s), j00006's refinement from
   another entry (21 s; round 1 measured 9.5-11 s for it with the memo), three fresh ring(38) readings (13.7 s, which
   set the 15 tries' median). Round 2 changed none of these: the reader inside the process with the memo held in
   memory is e011-r2-read's unfinished work, and --prep cuts reader CPU, not wall.
2. **plan** (48 s): the ring(33) requests, 3.0-4.4 s each on the fixed sequence and 5-6 s in B3, two thirds to
   three quarters of it the per-start reductions (2.0-3.4 s); the first request of a reading 7.5 s.
3. The try's own time (13-14 ms against 10 ms) and the idle measurement (0.1 s) are under 1% each.
