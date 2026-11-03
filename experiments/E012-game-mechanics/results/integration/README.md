# E012 integration: the merged build's gates, and the 15 tries with everything on

Branch e012 (worktree `/var/tmp/fast-mi/wt-e012`). plan.md: "Amendment 2026-11-03, the integration" (written before
the gates ran). Laptop, seed 1, nice 10; heavy steps under the heavy lock and the guard. Data:
`/var/tmp/fast-mi/e012-int` (readings, dumps and engine outputs stay there).

## What was merged and built

- origin/e011 928f437 (E011's merged round 2 results), e012-try 58cfaef (end, tested, targets, afford, archive,
  delay and wait at their final heads, and the try), e012-resident 9395b0d. Each component's own files are byte for
  byte its final head's.
- One fix of E011's code: `HostVerifier::replay_verify`. On E009's door archive, entry 5, round 2's fmi reports a
  difference at the first checkpoint ("frame 2728 is not after the current frame 2728") and the goal not held. The
  fixed fmi gives E008's `explore.py verify` record (`verify-5.json`): 6 checkpoints, 0 differences, 28 inputs,
  state:316 = 1, the same frames, qf and tick.
- One report command: `fmi check try-parts` (each try's wall by part).
- Build: `integrate/build.sh` into `/var/tmp/fast-mi/target-e012`, release, debug info off. fmi ec9da82dacc1 for the
  gates (5845ab5ad6e4 for B3's first two runs: the same code without `try-parts`). Engine: end's `svm-e012-end`
  (patches 0001-0005, scummvm cc6615ea). taskcore: round 2's binaries (unchanged since).

## Gates

"Off" is E011's switches as each gate sets them and every E012 switch unset; "on" adds every E012 switch
(`FMI_WAIT`, `FMI_TARGET_PER_KEY`, `FMI_DELAY_FORK`, `FMI_AFFORD`, `FMI_SELECT=progress`,
`SPEEDRUN_REPORT_RESTART`, `FMI_BOXES`, `FMI_READER_LIB`).

| gate | E012 switches off | on |
|---|---|---|
| G0, fast settings | B3's 15 tries (eoff) = round 2 in the same run (d) = round 2's published `r2-fast` = B0, 15 of 15 in everything | |
| G0, reference settings | eoff = round 2's `r2-ref` = B0, 15 of 15 | |
| G-on | | the 15 tries (e) equal eoff's in every attempt; 76 new entries (attempt 43, wait 18, afford 15, delay 0), 76 verified from boot, 0 differences |
| G11, the new entries from boot | eoff fast 43 of 43, eoff reference 43 of 43, 0 differences | (G-on) |
| G12 | 0 python processes in an fmi tree (567, 191 and 372 samples over the three B3 runs) | the same runs |
| G1, fork_exact.py, 7 load children | C and P against F: 6,002 comparisons, 0 differ; 556,223 load children, 594,383 comparisons, 0 differ; P's trace = F's; the control differs from frame 236 (walk 2,109 s at load 2.0-4.0) | end's run on the same engine binary (not run again) |
| G2, the door chain | 13 runs, 37 dumps each, 0 differ; state:316 = 1 | 0 differ |
| G3, the kitchen chain | 13 runs, 40 dumps each, 0 differ; owner:566 = 1 | 0 differ |
| G4, 400 steps from 2728 | 0 differences | 0 differences |
| G5, E009's test (a) through the Python reference | `CHECK (a): 0 goal True 2728 3216 488 11712 2 36`, 8 entries, verify 0 differences; 503 children, and 502 in a third run (below) | the same line, 503 children |
| G6 | the kitchen golden; ring(38), ring(33) and the refinement with --prep as fmi runs it, the memo off and both memo pairs: every sha1 as recorded | RS1: G6's five readings in one fmi process, byte for byte reader-scope's and G6's |
| G7 | the plan component's checks (taskcore CLI, PlanResult with gbfs, Fast Downward and the Rust lama-first-costs, the store, the Rust searches against Fast Downward's) pass | the fast forms in every check mode with the proofs on pass |
| G8 | r38, ref89, r33: pass | |
| G9 | pass with OFF3, and with ON2 and the proofs (round 2's "on"); the deepening's check passes | not a reference gate (below) |
| G10 | play's fixtures: records equal, 501 of 501 children, strict and ready the same; `fmi try` on test (a): the Rust lama-first-costs strict, ready, fast and Fast Downward's binary strict, ready all print round 2's line (501, 321, 321, 501, 321 children), every entry verified | the fixtures the same; the Rust lama-first-costs strict, ready, fast: round 2's line, every entry verified |
| cargo tests | rs/: 141 passed (T1, S1, afford, targets, wait among them); fmi-play: 17 and R2's table (10 of 10 strides); fmi-engine with the engine tests: 16; scumm-reader's memo: 5; 0 failed | |

The components' gates, every switch of the component on, on the merged build:

| component | gate | result |
|---|---|---|
| end | hooks at entries 0 and 203 | 7 + 7 cases pass |
| end | Boxes in 6 rooms | binary = JSON = the BOXD block's count and corners; flags differ only at room 28's box 11 (set by its entry script) |
| tested | T1 | passes (cargo test with the kitchen reading j00065) |
| wait | W1 | 50 of 50 stops over 13 room-28 entries verified from boot, 0 differences |
| wait | W2 | the door's stop at +461, +411, +324 from 203, 205, 225, equal to `due(local 211)`; equal in 13 of 13 room-28 entries; 37 of 37 paused slots run at their due frame |
| wait | W3 | entry 4 static over 1,600 frames |
| wait | W4 | 140 waits (14 entries × 10) under the heavy lock at load 1.9: median 0.109 s, max 0.210 s, none over 0.3 s; the frames' floor 38-78 µs per frame |
| delay | R1 | 31 runs verified from boot (4 attempts, 27 candidates), 0 differences |
| delay | R2, R3 | R2's table: 10 of 10 strides pass; R3: every forced fork chose the least matching k; the check's lines equal the component's record but for times |
| targets | T2 | 65 of 65 tries; the table equals the component's record |
| afford | A1 | 8 of 8 kept entries verified from boot, 0 differences |
| afford | A2 | M4's form equal to the probe name for name: 203 (246 trials, 8 outcomes), 205 (4, 2), 4 (4, 1) |
| afford | A3 | 270 trials at 203, 5 runs: 6.5-7.9 ms of wall per trial (CPU 2.3-2.8 ms) at load 3.8-4.2 |
| afford | walk boxes | 1,690 trials off, 1,722 on: no new outcome, 8 of 8 kept entries verified both ways |
| archive | S1 | the unit tests pass (13 + 28) |
| resident | RS1, the memo tests | pass; 5 of 5 |
| try | G0, G-on | above |

Not run again: G1 with the switch on and end's `unreach` (the engine binary and fmi-engine's code are the bytes end
ran them on), archive's S2 (a report; its code is unchanged).

## The 15 tries with everything on, against round 2

Run `b3m` (`integrate/b3.sh fast OUT e d`): two resident processes alternating try by try, the heavy lock held
across, the reading cache on and the job memo off as in round 2's B3, load 2.2-4.3. Wall seconds; each part by E011's
B5 rules (`fmi check try-parts`; `parts-two.md`).

| | round 2 (d) | E012, everything on (e) |
|---|---:|---:|
| sum, 15 tries | 191.8 | 183.8 |
| median, 15 | 4.4 | 3.2 |
| median, ring(33), 7 | 6.1 | 7.0 |
| median, ring(38), 8 | 1.35 | 0.95 |
| max | 120.4 | 121.2 |
| return | 0.16 | 0.12 |
| read | 140.3 | 117.1 |
| T | | 4.1 |
| idle measurement / wait | 1.20 | 1.14 |
| plan | 48.5 | 54.4 |
| play | 1.42 | 1.54 |
| affordance steps | | 4.97 |
| rest | 0.29 | 0.43 |
| fmi's resident set at the end | 2.6 GB | 3.4 GB |

- **The reading** is 23.2 s shorter: the fresh ring(38) readings (j00002 2.4 s against 4.7 s, j00003 1.3 s against
  3.9 s, j00007 0.5 s against 1.9 s) and j00006's refinement from another entry (7.5 s against 19.7 s), all from the
  resident store.
- **E012's own steps** add 9.1 s: T once per reading (1.8-2.1 s per ring(33) reading, under 0.1 s per ring(38)
  reading) and the affordance step in all 15 tries (0.26-0.72 s each). The waits replace the idle measurement at the
  same cost (1.14 s against 1.20 s).
- **The planning** is 5.9 s longer: 2.8 s in j00000, 1.0 s in j00006 and 0.2-0.6 s in each later ring(33) try, for
  the same requests and plans. Not attributed.
- **Mechanisms** (`report-e.md`): T 559-577 atoms over 450-451 keys (ring(33)), 97 over 66 (ring(38)); 15 waits,
  static at 6 entries, 18 stops at 9, all kept; delay: 1-2 windows per try, none triggered, 1 random call per frame;
  affordance steps: all at Lemma R's R0 in a dialog, 4 trials, 1 kept each; no dead move; no goal again (no tier-0
  target).

**With three processes** (`b3f`, d, eoff, e alternating, the run that gave G0 and G-on; `parts-three.md`): d 178.2 s,
eoff 193.6 s, e 201.3 s. There e planned j00011 in 11.2 s and j00014 in 9.6 s (eoff: 5.7 s and 6.7 s), and j00012
took 2.5 s to plan a request that eoff served in 0.07 s. The planner of a reading is closed before a refinement unless MemAvailable is at least
E008_MEM_SECOND_GB (7 GB; harness.rs `planner_keep`), and that is the only path that closes one. With three resident
processes MemAvailable was 7.1-8.5 GB at the ends of the ring(33) tries, and e's process was up to 0.4 GB larger than
eoff's (the resident store and T; 0.8 GB larger than d's in the two-process run). With two processes (8.9-11.9 GB)
e's j00012 planned in 0.01 s.

## Found on the way

- **G9 with E012's switches on is not a reference gate.** G9 compares fmi with explore.py and step.py. With
  `FMI_SELECT=progress` the cells and log differ by design; the recorded tries' replay through doubles has no tested
  port, and the try refuses to run half wired ("the E012 switches read T, and no tested port is wired"); the driver's
  kill-and-resume scenario stops before its kill because stagnation over T fires on fake steps. With ON3 the deepening
  changes job.json, as round 2 found (its G9 "on" was ON2 with the proofs, the deepening checked by
  `deepen_check.py`).
- **G5's children: 503 where E011 records 501 or 502.** The Python reference's strict window search runs children in a
  thread pool and stops when Q(g) + b reaches the best result so far. Which children are in flight at the stop
  depends on timing. On end's engine (off and on) window 1 issued two more children (frames 2798 and 2799, both
  deviations after the best). Every child present in both runs has the same outcome, frames and qf; the best of each
  window, the route and the CHECK line are the same. On round 2's engine the same day the count was 501, with
  window 0's and window 3's records also differing from round 2's own in a child's q0 and outcome. A third run on
  end's engine, with E012's switch off and the laptop quieter (load 2.8-4.2), gave 502: inside E011's "501 or 502".
- **The due-frame window** (plan.md section 1, last bullet) was built by no component and is not in the merged build.
- **The driver with every switch on** (`integrate/smoke.sh`; not the kitchen rerun): `fmi explore init` with the
  kitchen check's settings, then `fmi explore run` (b = 2, the fast settings, ON3 and every E012 switch) to 4 jobs,
  168 s.
  - job.json carried the archive's novelty: T grew from the goal's atom to 559, then 576 atoms.
  - Each try read it ("novelty": "archive") and returned its T in result.json.
  - The archive ingested 13 new entries: checkpoint 4, end 4, afford 5. The waits were static at all 4 entries. The
    13 entries sit in 8 cells over T.
  - `fmi explore verify` (the fixed verify) verified all 13 from boot with 0 differences.
