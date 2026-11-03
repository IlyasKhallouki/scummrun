# E011 round 1: results (2026-11-02, written after the runs)

The six components merged into branch e011, wired into one `fmi` process, gated and benchmarked on the laptop (seed 1,
nice 10, heavy steps under the heavy lock and the guard). The benchmark's scripts are `experiments/E011-speed/bench/`;
the raw outputs are under `~/.cache/fast-mi-e011/bench/` (b0, b1, b2, b3, b4) and `~/.cache/fast-mi-e011/integrate/`
(the gates), both on `/` through links. Files here:

- `gates.md`: G1-G12 with their evidence;
- `tries.md`: B3 and B4, every try by part (report.py's output), and `tries.json`;
- this file: the integration, the answer, each part before and after against its floor, and what binds.

## Answer

Partly.

- **Exactness.** Every gate but one passes on the integrated build (gates.md). G10 fails its Gbfs clause: with the Rust
  search (lazy GBFS, h_FF) test (a) takes another route than Fast Downward's lama-first and stops without the goal.
  With FdLamaFirstCosts the Rust run gives E007's line `CHECK (a): 0 goal True 2728 3216 488 11712 2 36` and the goal
  verifies from boot. The 15 tries under the reference settings equal B0's in everything, cold and warm; under the
  fast settings 3 of 15 take another route, and every new entry verifies from boot with 0 differences.
- **Speed.** One resident process runs the 15 tries in a median of 3.8 s per try (fast settings; 29.4 s before, on
  the same load), 17.5 s on ring(33) (186.2 s before) and 1.1 s on ring(38) (23.8 s before). The 15 tries take 241 s
  in all against 1,780 s. A return is a fork (0.01 s against 2.2 s), the idle measurement 0.2 s (8.3 s). What is left
  is the reading (53% of the warm total, most of it the first try's cold readings) and the planner (45%, the first
  request of each new goal on ring(33)).
- **Against the threshold.** "Yes" needs a warm median of 1 s or less and a ring(33) median of 10 s or less: both
  missed (3.8 s and 17.5 s). The warm median is under 10 s, but G10 fails, so the kitchen check is not run.

## Integration

Merged in the stage order (readio, readcore, plan, engine, play, explore), with no conflict: no two components shared
a file but fmi-core, which all six created byte for byte (sha1 db2016b6). Then, each its own commit:

- **reader-scope --memo DIR**: readcore's IFACE patch to readio's reader_scope.rs (the job memo on the command line).
- **The rs/ workspace**: one Cargo.toml and lockfile for the seven crates, profiles at the root (release: LTO thin, one
  codegen unit, debug info level 1); the members' own profiles removed. fmi-checks (play's fixture tests over
  fmi-engine) is a member outside the default build, since it turns on fmi-play's test doubles.
- **One mismatch fixed**: in the workspace serde_json's `preserve_order` (fmi-explore's, fmi-play's) reaches every
  crate. fmi-reading's cache key iterated the start facts of a JSON map expecting them sorted; it now sorts them
  itself. G8 then passes with the workspace's build, so no other output depended on map order.
- **The real ports in fmi** (explore's adapters, `rs/fmi/src/wire_real.rs`): fmi-engine's Host and kept-entry pool
  (an entry the pool does not hold is registered with the nearest earlier entry whose movie is a prefix of its own,
  so it is made by a fork of that one and the rest of its movie); reader-scope with --out-dir, opened by
  fmi-reading, through fmi-reading's exact cache (FMI_READING_CACHE) and with readcore's memo (FMI_MEMO), each off
  unless set; fmi-plan's Host; fmi-play's window search.
- **Warm tries**: `fmi try` takes several job folders, or a FIFO of them (`@FILE`), and runs them in one process;
  job.json may carry the forced targets.
- The engine's check and bench scripts take the workspace's fmi-engine; `~/.cache/fast-mi-e011/env.sh` names every
  binary.

Not applied: the IFACE asks that need code in a provider (fmi-core `Engine::step`, `step_observe`, `at_many`, the
screen geometry for the hit cache). They are round-2 items below.

## B3 and B4: the 15 tries

B0 rebuilt E009's 15 kitchen tries with the Python reference (each the log's entry, scope, reading decision and
targets): all 15 give the log's return frame, reading decision, stop, frames, replans, refinements and new entries
(j00003's reason reads "no cached reading" where the log has "differs on 2 keys": B0 hands a fresh try no candidate).
B3 starts every try from B0's entries. B4 measured the reference again, interleaved with B3 try by try: before, then
the four regimes, then the next try. Walls in seconds; CPU is the try's whole process tree.

| regime | median, 15 tries | ring(33), 7 | ring(38), 8 | mean | max | sum | CPU median | CPU sum |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| before (B4: Python, svm-w3b) | 29.4 | 186.2 | 23.8 | 118.7 | 483.9 | 1,779.8 | 40.8 | 2,746.7 |
| cold, reference (a-ref) | 5.2 | 63.3 | 4.2 | 37.7 | 170.8 | 565.6 | 11.4 | 1,049.2 |
| cold, fast (a-fast) | 4.4 | 38.3 | 2.8 | 25.2 | 133.1 | 378.6 | 10.5 | 884.5 |
| warm, reference (b-ref) | 3.8 | 44.6 | 2.3 | 27.6 | 170.3 | 414.3 | 7.8 | 658.4 |
| warm, fast (b-fast) | **3.8** | **17.5** | **1.1** | 16.0 | 115.0 | 240.6 | 8.6 | 498.4 |

By part, the medians of the walls (the read part counts the try's readings and its refinements):

| part | before, ring(33) | before, ring(38) | warm fast, ring(33) | warm fast, ring(38) | share of the warm fast sum |
|---|---:|---:|---:|---:|---:|
| return | 2.32 | 2.21 | 0.01 | 0.01 | 0.2% |
| idle (1,600 frames) | 8.20 | 8.55 | 0.20 | 0.20 | 1.2% |
| read | 63.63 | 4.81 | 0.03 | 0.41 | 52.5% |
| plan | 105.61 | 5.24 | 16.03 | 0.45 | 44.7% |
| play | 5.21 | 0.30 | 0.40 | 0.02 | 1.4% |
| the rest | 0.21 | 0.06 | 0.00 | 0.00 | 0.1% |

- **The first try** (j00000) is 115 of the warm 241 s: a fresh ring(33) reading (42 s, 120 CPU s at 3 threads) and two
  refinements on an empty memo store (49 s and 20 s), then two plans (17 s). Later ring(33) tries reuse its refined
  reading; their refinements are taken from the reading cache when the refinement's inputs repeat (j00009, j00011,
  j00012, j00014: 0.02 s) and from the memo otherwise (j00006, j00008: 9.5 s and 11.1 s).
- **The ring(33) plan** (the attempt less its window search and refinements) is the first request of a new goal on the
  refined reading: j00006 15.6 s, j00008 22.3 s, j00009 17.0 s, j00011 12.9 s, j00014 16.0 s; the one try whose goal
  repeats an earlier one's plans in 3.2 s (j00012). With FdLamaFirstCosts the same tries take 37.2-46.4 s.
- **ring(38)**: a fresh reading 1.7-1.8 s, a reused one 0.02-0.06 s to open, the refinement 1.3 s or a cache hit, the
  plan 0.4-0.6 s, the idle 0.2 s, the return 0.01 s.
- **The try's own time** outside the ports (fmi's `explore_s.own`): median 78-90 ms over the four regimes (34-290 ms).

## B2: test (a) end to end

One try from the root on rooms 28, 33, 41 to owner:566 = 1 (E009's off-check job), each cold (one process, no store):

| run | CHECK (a) (rc stop goal-from-the-plan frame0 frame_end frames qf replans inputs children entries) | wall | CPU | verify from boot |
|---|---|---:|---:|---|
| before: E009's step test (a), svm-w3b | 0 goal True 2728 3216 488 11712 2 36 503 8 | 288.0 | 319.7 | 0 differences, 9 checkpoints |
| fmi, FdLamaFirstCosts, strict | 0 goal True 2728 3216 488 11712 2 36 501 8 | 88.0 | 111.8 | 0 differences (lean, fresh engine) |
| fmi, FdLamaFirstCosts, ready | 0 goal True 2728 3216 488 11712 2 36 321 8 | 81.3 | 103.2 | 0 differences |
| fmi, Gbfs, strict | 0 "no plan (the planner proved none)" False 2728 3174 446 10704 3 29 464 7 | 78.3 | 125.2 | 0 differences |
| fmi, Gbfs, ready, tail | 0 "planner timeout (600 s)" False 2728 7086 4358 104592 3 29 505 7 | 664.2 | 681.7 | 0 differences |

- The FD runs spend most of their time planning: 48, 9 and 8 s for the three plans; the fresh reading 11 s.
- Gbfs's first plan (28 steps, cost 336) costs 56 s of search on this task (2,430 expansions, h_FF at 23 ms an
  evaluation over 331,165 unary operators), FD's lama-first 48 s; the reductions 3.2 s. Its second plan (13 steps)
  names event(local:217@18)#9633, whose precondition on state:316 the engine refutes at 3171 (class semantics); FD's
  14-step plan does not use it.

## B1: micro-benchmarks

Back to back on the same inputs, before then after. Engine at load 1.5-1.9 (two rounds); readings at load 3-5;
planner at load 4.7-7.4; windows at load 5.5-6.

**Engine** (before: E010's frame_costs.py and chain.py on svm-w3b through JSON; after: fmi-engine on the lean build
through the binary channel; ranges over the two rounds and both chains):

| measure | before | after | target | floor |
|---|---:|---:|---:|---:|
| (a) the parent's observed frame (Step + facts) | 3.9 ms | 0.063-0.101 ms | 0.15 ms | 0.015 ms of game logic |
| (b) the same with fork_each 7 | 6.1-6.2 ms | 0.91-1.25 ms | | |
| (c) a bare frame, one command each | 0.5 ms | 0.064-0.098 ms | | |
| (c) a frame and its state | 2.2-2.6 ms | 0.057-0.090 ms (Step with the block) | | |
| (c) a frame inside 50-frame commands | 0.4 ms | 0.036-0.052 ms (engine CPU 0.036-0.051) | 0.10 ms | 0.015 ms; 0.03-0.04 ms with the two cuts not built |
| the full binary Obs | (JSON dump 1.0-1.35 ms) | 0.090-0.124 ms | 0.10 ms | |
| (d) fork + quit | 1.2-1.3 ms | 0.66-0.77 ms (parent CPU 0.51-0.61) | 1.0 ms of parent CPU | 0.67-1.33 ms |
| (d) fork + frame + state + quit | 2.9-3.0 ms | 1.21-1.42 ms | | |
| start to the first stop | 0.74-0.88 s | 37-64 ms wall (20-60 ms CPU) | 0.1 s | 80-90 ms (profile) |
| return by fork of a kept entry (door entry 5; kitchen entry 7) | 2.3-3.6 s (a replay from boot) | 0.55-0.84 ms | 3 ms | 0.5 ms (the fork) |
| the door chain from boot (3,129 frames) | 2.26-3.13 s | 0.30-0.47 s (median 0.33, 0.43) | 0.5 s | |
| the kitchen chain from boot (3,216 frames) | 2.49-3.60 s | 0.36-0.47 s (median 0.42, 0.43) | | |

The binary channel alone (lean switches off) gives the door chain from boot in 2.3-2.7 s and a frame inside a command
0.51-0.52 ms: the four switches make the frame cheaper, the channel makes the observation cheap.

**Readings** (before: E009's binary through read_rust, with its Python side; after: reader-scope --out-dir; the same
files, sha1 checked):

| reading | before: wall (reader wall, reader CPU) | after, fresh | cache hit: key + open | the memo from j00000's store | opening (strict setup's replacement) |
|---|---:|---:|---:|---:|---:|
| ring(38) j00091 (9,286 actions) | 13.1 s (5.6 s, 17.5 s) | 3.0 s (2.4 s, 7.5 s) | 1.6 + 3.5 ms | | 1.8 ms |
| ring(33) j00089 (234,401 actions) | 123.7 s (71.8 s, 264.1 s) | 34.3 s (32.1 s, 111.7 s) | 3.1 + 4.4 ms | | 2.0 ms |
| the refinement j00089 [103,194] (74,995) | 62.4 s (35.6 s, 116.9 s) | 17.1 s (15.4 s, 47.5 s) | 2.9 + 4.5 ms | 11.1 s (9.4 s, 26.5 s; 15.9 job-s, 31,136 runs taken, 3,190 run) | 2.7 ms |

The sha1 of the reader binary and the game index, once per process: 18-47 ms. action(i) 3.6-5.8 µs, record(i)
8.6-12.6 µs, by_name 0.4-0.5 µs. Peak memory: 127-569 MB fresh.

**Planner** (the plan component's fixed tasks; before: plan_fd.plan_taskcore with taskcore serve and FD lama-first;
after: fmi-plan; wall per request):

| request | ring(38) before | ring(38) Gbfs | ring(38) FdLamaFirstCosts | ring(33) before | ring(33) Gbfs | ring(33) FdLamaFirstCosts |
|---|---:|---:|---:|---:|---:|---:|
| cold (first of the reading and goal) | 1.45 s | 0.21 s | 0.93 s | 33.9 s | 13.8 s (reductions 11.7) | 38.8 s |
| the same request | 1.17 s | 0.3 ms (store) | 0.5 ms | 29.4 s | 1.1 ms | 0.7 ms |
| another start | 1.35 s | 0.12 s | 0.88 s | 29.6 s | 11.7 s (8.0) | 23.3 s |
| other caps | 1.36 s | 0.11 s | 0.80 s | 39.7 s | 8.5 s (6.5) | 27.8 s |
| another goal | 1.53 s | 0.20 s | 1.40 s | 45.3 s (97.0 CPU s) | 16.8 s (12.6) | 28.9 s |

Opening a ring(33) planner: 4.6 s (the reading's files 2.3 s, the warm task 2.3 s). Every request again from the plan
store: 0.2-12.8 ms. The old tail's check (fmi-plan --tail-only, the recorded replans): 1.4-13 ms for most, 135-190 ms
for a tail accepted from a new reading in the process, 1.5-1.7 s twice (the needed facts made on first use).

**Test (a)'s four windows** (play-lean on the capture, through fmi-engine on the lean build; before: B2's reference
run, svm-w3b):

| window | before (Python, strict) | after, strict | after, ready |
|---|---:|---:|---:|
| 0 (67 children; ready: 1 try) | 11.07 s | 1.13 s | 0.08 s |
| 1 (22-24; ready 23) | 1.00 s | 0.10 s | 0.10 s |
| 2 (405; ready 296) | 153.99 s | 14.94 s | 10.83 s |
| 3 (7; ready 1) | 0.30 s | 0.03 s | 0.01 s |

An observed child frame costs 0.22-0.51 ms: the engine run 0.13-0.31, runs 0.01-0.05, explain_log 0.02-0.03, state
0.03-0.06, facts and view 0.02-0.04, match 0.01-0.02. The first run of this section stopped in its second plan's first
window with no window record (strict) and in the first window (ready), while another process on the laptop held 7 GB;
the window search stops so when MemAvailable stays under its 3 GB floor with no child running. The rerun a minute later
(MemAvailable 4.6-4.7 GB before and after) gave the table and passed tests 2-4 again.

## Each part against its floor

Warm, fast settings (B3 b-fast) unless said; "before" is B4 on the same load. The floors are plan.md's (the profiles'
and the components' measured ones); after/floor where both are numbers.

| part | before | after (median; max) | floor | after/floor | target | met |
|---|---:|---:|---:|---:|---:|---|
| route replay by fork, door and kitchen chains (B1) | 2.3-3.6 s from boot | 0.55-0.84 ms | 0.5 ms (fork + quit, parent) | 1.1-1.7 | 10 ms (threshold), 3 ms (engine) | yes |
| route replay from boot, both chains (B1) | 2.26-3.60 s | 0.33-0.43 s | 3,129 frames x 15 µs + start 37 ms = 0.08 s | 4-5 | 1 s (threshold), 0.5 s (engine) | yes |
| a try's return (B3) | 2.2 s | 0.01 s; 0.38 s (the first, a boot) | 0.5 ms | ~20 (boot_s is rounded to 10 ms) | | |
| idle, 1,600 observed frames (B3) | 8.3 s | 0.20 s; 0.40 s | 1,600 x 15 µs = 24 ms (game logic); 1,600 x 0.063 ms = 0.10 s (today's Step) | 8; 2 | | |
| read, ring(33) fresh (B1) | 123.7 s | 34.3 s (111.7 CPU s, 3 threads) | the memo-taken reading 12-16 s (readcore: its sink and merge) | 2.1-2.9 | 60% of the old CPU (readcore) | yes: 42% |
| read, a refinement from another entry with the memo (B1) | 62.4 s | 11.1 s (9.4 s reading) | | | 35% of the old job-s | yes: 15.9 vs 106.5 job-s (15%) |
| read, a cache hit (B1, B3) | 0.05-0.1 s load + 75-114 s strict setup | 5-8 ms (key 1.6-3.1, open 3.5-4.5); B3 0.02-0.06 s | the open, 2 ms | 2.5-4 | 50 ms | yes |
| read, ring(38) fresh (B1) | 13.1 s | 3.0 s | | | | |
| plan, ring(33): a new goal on the refined reading (B3) | 100-126 s | 16.0 s (12.9-22.3) | the per-start reductions 5.5-6.1 s + h_FF 0.45 s | 2.5 | 13 s CPU a new request | no |
| plan, ring(33): another start, same goal (B1) | 29.6 s | 11.7 s (load 5), 8.5 s other caps; 7.6-8.7 s at load 2.5 (plan component) | 6-6.5 s | 1.3-1.8 | 13 s CPU | yes |
| plan, the search on test (a)'s first request (B2) | FD 48 s | Gbfs 56.5 s (2,430 h_FF evaluations at 23 ms) | | | 1 s | no |
| plan, ring(38) (B1) | 1.2-1.5 s | 0.11-0.21 s | 0.16 s (plan component) | 0.7-1.3 | 0.3 s | yes |
| plan, a store hit / the old tail (B1) | | 0.2-12.8 ms / 1.4-13 ms (135 ms-1.7 s on first use) | | | 2 ms / 10 ms | partly |
| play, test (a)'s third window, ready (B1) | 154.0 s (strict) | 10.8 s (296 children, 8,906 observed child frames at 0.51 ms) | 8,906 x 0.15 ms / 3 workers = 0.45 s | 24 | 5 s | no |
| play, an observed child frame (B1) | 10 ms | 0.22-0.51 ms (4 round trips) | 0.15 ms (the engine's Step) | 1.5-3.4 | 0.2 ms | no |
| play, a window whose first try succeeds (B1) | | 10-80 ms | | | 50 ms | partly |
| the try's own time (B3) | 160-173 ms process start alone | 78-90 ms median (34-290) | | | 10 ms | no |
| the guide (G9) | 23-32 s | 0.31-0.60 s | | | 1 s | yes |
| the 15 tries, warm, fast (B3) | median 29.4 s, ring(33) 186.2 s | median 3.8 s, ring(33) 17.5 s, ring(38) 1.1 s; mean 16.0 s | | | median 1 s, ring(33) 10 s | no |

What the try's own time is made of (FMI_TRY_PROFILE on three warm ring(38) tries in one process): the idle rule's own
work over the 1,600 frames' facts 26-67 ms, the checkpoints' texts 19-24 ms, the reading's bookkeeping 5-9 ms, the
targets 2-5 ms, the harness's per-plan work 1-3 ms.

## What binds (round 2)

By share of B3's warm total under the fast settings (241 s): the reading 53%, the planner 45%, play 1.4%, idle 1.2%,
return 0.2%. One gate fails (G10's Gbfs clause), which the kitchen check needs. Each item names its target and gate;
none is built before its exactness argument is written.

1. **read** (53%; 97 s of it the first try's cold readings). A fresh ring(33) reading takes 32-42 s at 3 threads
   (112-120 CPU s); a refinement 15-49 s on an empty memo store, 9-11 s from another entry's. readcore measured the rest
   of a fully taken reading: the sink thread busy 12.1 of 14.3 s, the reading thread's merge and stream 10.4 s. Build:
   the reader as a library inside fmi (no process start, the memo held in memory across readings), the sink's files
   written by several threads, the merge and stream off the reading thread, more threads once G6 shows the same bytes at 3
   and 7. Target: a fresh ring(33) reading at most 15 s
   of wall cold; a refinement or a fresh reading taken from the memo at most 3 s. Gate: G6 (kitchen golden, the three
   readings' packed file, golden body and reached file, the four files) with the memo off and on, at two thread counts.
2. **plan** (45%). The first request of a new goal on the refined ring(33) reading takes 12.9-22.3 s (median 16.0),
   another start 8.5-11.7 s, of which the reductions are 6.5-12.6 s; on test (a)'s first request the search itself
   takes 56.5 s (2,430 h_FF evaluations at 23 ms over 331,165 unary operators). Build: the goal-independent reductions
   once per reading, the goal-dependent first round once per goal, only (d) and the finish per start (plan.md's
   round-2 item); h_FF computed exactly as now (the same values and preferred operators) over flat arrays with only
   the start's changes propagated. Target: a new goal on ring(33) at most 3 s, another start at most 2 s, an h_FF
   evaluation at most 3 ms on test (a)'s task. Gate: G7 (taskcore's command line and serve sequences byte for byte,
   PlanResult = plan_fd's, the Rust plans = FD's lazy h_FF plusone plans byte for byte).
3. **route** (G10). With Gbfs, test (a) stops without the goal (strict: no plan at 3174; ready with the tail: the
   planner's 600 s limit at 7086), and 3 of the 15 tries play another route than B0's. Gbfs's 13-step second plan names
   event(local:217@18)#9633, whose precondition on state:316 the engine refutes at 3171 (class semantics); FD's 14-step
   plan does not. Build: the search FD's lama-first-costs runs (landmarks, the landmark count with h_FF, preferred
   operators, plusone costs, the same tie-breaking) in Rust, on the reduced task in memory, with a time limit it keeps.
   Target: its plans byte for byte FD's lama-first on serve38, serve33 and test (a)'s requests, in at most 20% more
   CPU than Gbfs. Gate: G7 with that equality, then G10 with the fast settings (E007's CHECK line, verify 0
   differences) and G11's fast sequences equal to B0's.
4. **play** (test (a): third window 10.8 s ready, 14.9 s strict). An observed child frame is 0.22-0.51 ms in four round
   trips (run, runs, explain_log, obs); the input code makes 227,624 hit tests in that window. Build: the play
   component's IFACE asks in fmi-core as provided methods (`Engine::step_observe`, `Engine::at_many`), fmi-engine's
   overrides as one message each, the screen geometry in the stop status so the hit cache can be proven (HITCACHE.md).
   Target: an observed child frame at most 0.2 ms; the third window at most 5 s ready; a window whose first try
   succeeds at most 50 ms. Gate: play's tests 2-4 on the lean build (play-lean), G4, G10's FD clauses.
5. **own** (the try's own time 78-90 ms against 10 ms; the idle rule's own work 26-67 ms, the checkpoints' texts
   19-24 ms). Build: the idle rule on the F2 deltas the engine sends, without whole fact maps per frame; checkpoint
   texts made once, when a file needs them. Target: at most 10 ms. Gate: G9 (step's idle_rule cases, the driver
   scenarios, the replays) and B3's entries equal.
6. **engine** (idle 0.2 s a try; a frame inside a command 36-52 µs against 15 µs of game logic). Build: the count-only
   mixer and skipping pixel output, each after its written argument that no sample or pixel value reaches the game
   state, with pixel-free exact dumps for G1-G3. Target: a frame at most 20 µs inside a command, the door chain from
   boot at most 0.15 s. Gate: G1-G5.
