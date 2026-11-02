# E010: Play when ready, commit as finished

Date planned: 2026-11-02
Research question it serves: RQ1 (completion from the game files alone, when the scope cannot be read ahead of time)

## Question

With two switches added to E009's explorer, play when ready (an input is played on one forward copy of the engine,
issued at the first frame the harness's input code can issue it, and checked by B1's strict matcher as now) and commit
as finished (the driver commits each job's result when it ends), does the explorer reach the kitchen (owner:566 = 1)
and the bar's kitchen door (state:316 = 1) from the boot on the 10 rooms of the k = 2 scope, each verified from boot?
How much faster is it per try and in total than E009?

## Baseline

- **E009's kitchen on the VM** (branch e009 at 191e30d; `~/.cache/fast-mi-e009/archives/kitchen/log.jsonl`, the
  laptop's copy, which holds the 15 jobs committed by 11:26 UTC; n2 cores, b = 2). Job wall: median 23.6 s, least
  11.7 s, most 388.5 s (j00000: a fresh ring(33) reading, 235,579 records, and two refinements); 1,400 s in all over
  1,090 s from the first start to the 15th end. The window search's CPU per job is 0.0 to 30.4 s of it; reading and
  planning take the rest. Measured on the VM by E009's run after these 15 (its records stay there): tries whose input
  the harness's input code cannot issue for a long stretch run each window to its 3,600-frame end with about 900
  children marked "interface", about 8 minutes per window and 16 to 20 minutes and more per try. Jobs commit in start
  order, so one such job holds back every later result, and its busy cell is not selected again meanwhile.
- **E009's door on the VM**: 1 job, 364.9 s wall, state:316 = 1 at 3129 while the engine idled (401 frames).
- **E007's kitchen** (step.py's engine test (a): one job from the root on rooms 28, 33, 41 to owner:566 = 1, every
  switch off): the goal from the plan's records at 3216, 488 frames, 11,712 qf, 2 replans, 36 inputs, 4 windows and
  501 or 502 children, verify 0 differences. Job wall on this laptop 471.5 s (E008, 00:26) and 548.9 s (E009, 10:17);
  on the VM 195.0 s. So a job on the laptop takes about 2.8 times its time on the VM (548.9 / 195.0, the same code).
- **Where a window's time goes** (`results/baseline-*.txt`, measured for this plan on the laptop and in the records):
  - From the boot on this laptop: a parent frame as the strict search steps it (one frame, its runs, explain log and
    dump) 10.4 ms; with fork-each 7, 13.9 ms; a bare frame 0.9 ms; a frame and a dump 5.5 ms; a fork and its quit
    2.3 ms. A 3,600-frame window's parent costs under a minute; the children cost the rest.
  - The harness's input code waits inside each child: `Executor._wait_walk_to` up to 120 frames for the verb to come
    back to Walk to, `_wait_verb_visible`, `wait_gate` and `Harness._click_verb` up to 600 (`INPUT_LIMIT`), and the
    child observes every one of those frames when an earlier record is pending. A child forked one frame later waits
    over the same frames again. Children that issue nothing are not merged by Lemma 5 (they have no first event), so a
    stretch of W frames where the input cannot be issued costs about W children times their wait.
  - In the strict windows recorded from E006 to E009 (61 distinct windows with an end point, `results/baseline-tries.txt`):
    the first child forked was at the window's start in all 61, and it was the chosen one in 56. In the other 5 the
    children before the chosen one were 293 (281 spent, 12 mismatch; walk_to(316)[202]#1038, test (a)'s third
    window), 49 (all "interface": object 489 not in the room yet), 33, 20 and 19 (spent, refuted, mismatch). Of 50
    empty window records, 41 ended on a needed record refuted in the parent, 4 at 3,600 frames with 3,600 spent children.
  - Test (a)'s windows (E009's off run): in each of the 4, every child forked before the chosen one failed (none was
    pruned), and the chosen children's fork frames are 2728, 2796, 3091 and 3204. The 293 children before 3091 ran
    30,806 frames; that window's wall was 261 s with 3 workers.

## Method

Two switches, each off by default and with the code E009's when off: `ready` (window.py `WindowConfig.ready`; job.json
`"ready": true`; `explore.py init --ready`) and `commit_finished` (`explore.py init --commit-as-finished`). Both are kept
in stage.json `"e010"`. E009's switches stay as they are.

### (1) Play when ready (`ready`, strict window search only)

A window with an input plays it on one forward copy at a time instead of forking a child at every frame:

- **The first try.** The parent steps from the window's start as now (its strict matching, Lemma 7′, the end rule, the
  goal and refutation checks, each frame), without fork-each. At the first frame where the engine takes the input on
  the next frame (the search's gate, Lemma 5) it forks one child, which runs the harness's input code exactly as a
  child does now: the input code itself waits (camera, verb, gate) until it can issue the input, issues it at that
  first frame, and the child is then checked by B1's matcher (`_child_strict`, unchanged): success, goal, deviation,
  or a failure. The parent waits at that frame while the try runs.
- **The first end point is kept.** A try that ends in success, the goal from the plan's records or a deviation ends the
  window with it: the harness's existing paths then run (the next window from its snapshot, or the replan from the
  deviation's click gate). No later fork is tried for an earlier finish.
- **A try that could not issue the input** (RouteError or no input event, the input code having run from fork frame g
  to frame f with no event): the next try is forked at the first gated frame at or after max(g + 1, f). The try was,
  at f, the parent at f (A0, no input given), and its input code waited over every frame before f.
- **A try that issued its input and failed the strict check** (refuted, mismatch, spent, limit, the goal not from the
  plan, an engine error), its first input event at frame f1: the next try is forked at the first gated frame at or
  after max(g + 1, f1 − 1). A child forked between them whose first event lands at f1 is the same run (Lemma 5); a
  try whose first event lands on the first event of a try already failed is marked duplicate at once.
- **Timing-dependent matches.** Issuing at the first frame can fail where a later frame passes (5 of the 61 recorded
  windows above). After the first try that issued and failed, the window keeps up to `workers` (3) tries in flight at
  the next try frames and resolves them in fork order: tries forked inside a resolved try's skipped frames are
  cancelled and never chosen, so the chosen try does not depend on which thread ends first. **The bound is measured
  from data**: a window makes at most K = 294 issued tries, the most any recorded strict window needed to reach its
  chosen child (293 failed children, then the chosen one, `results/baseline-tries.txt`). Once K tries have failed it
  forks no more, and the parent steps on to the window's own end (the end rule, 3,600 frames, the goal in the parent),
  as an empty window does now. Duplicates do not count.
- **What is kept exact.** Every played input is still checked against the records: the chosen try is a child of the
  same `_child_strict` (Definition S, Lemma S1, the click decided by Lemma 9′; Lemma S2 for a strict success), and a
  deviation is the same deviation (the input took effect, then a record the plan does not name changed what a later
  step reads). Every entry is an engine state reached by the played inputs (Lemma C unchanged), every return is
  compared with its checkpoint, and every goal entry is verified from boot in a fresh engine (`explore.py verify`).
- **What is dropped.** Section 6's guarantee: the chosen try is the first end point in the order of tries, not the
  least Q over every fork frame, and frames inside a try's wait are not forked (a stated loss of runs, never a route
  that was not played). The records say so: each window's record has `"ready"` (the tries, their fork and first event
  frames and outcomes, the cancelled and duplicate children, the bound), and result.json's attempt has `"ready": true`
  and `"earliest": false`. Lemma 2-4's bound b is not used to stop forking.
- Test (a) under this rule: its 4 windows' chosen children are the first end points in fork order and every child
  forked before them failed, so the same 4 children are chosen: the same route.

### (2) Commit as finished (`commit_finished`)

- The driver commits the first running job (in start order) whose process has ended with a result, instead of only
  the oldest running job. Ingest is unchanged: entry ids in ingest order, cells, readings and the log line.
- What changes: ids, cells' n and the order of selection depend on which job ends first (thread and machine timing),
  not on the results alone; two runs of the same archive can differ. A slow job no longer holds back later results
  or the selection of their new entries.
- Resumability holds: a job dir without a committed log line is unfinished work, as before; on a resume the finished
  ones (a result.json) are committed, the running ones waited for, the others run again in place. The archive after a
  resume is consistent (every row's parent committed before it, every log line naming its own new rows), though not
  the one an uninterrupted run would have made.

### Interfaces

- `window.WindowConfig(ready=False, ready_tries=294)`; ready needs `strict` (the Search refuses otherwise).
- step.py: job.json `"ready": true` builds the window search with `ready`; result.json's attempt `"ready"`, `"earliest"`.
- explore.py: `init ... [--ready] [--commit-as-finished]`, kept in stage.json `"e010"`; job.json `"ready"`; log.jsonl
  lines add `"committed"` (the commit's index) and `"started_index"` (the job's place in start order).

### Off check and tests

- With the switches off: E008's and E009's unit tests pass, and step.py's engine test (a) gives E007's kitchen run
  (`CHECK (a): 0 goal True 2728 3216 488 11712 2 36` and 501 or 502 children, verify 0 differences).
- Unit tests: the try order (a try that could not issue, one that failed after issuing, the parallel tries resolved in
  fork order with a later one ending first, cancelled tries, duplicates, the bound K); ready refused without strict;
  job.json and stage.json carry the switches only when given; the driver commits in finish order with the switch on
  and in start order with it off (fake step); a resume with finished and unfinished jobs.

### Runs (this laptop, seed 1, under the guard, nice 10, E008's machine rules)

1. Off check (above).
2. step.py's engine test (a) with `ready` on (one job from the root on rooms 28, 33, 41 to owner:566 = 1): time,
   frames, children, verify from boot.
3. The door, then the kitchen, each from the boot in a new archive under `~/.cache/fast-mi-e010/archives/`: allowed
   rooms k2, b = 2, E009's switches (`--idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts`) and E010's
   (`--ready --commit-as-finished`), `--stop-jobs` 4 for the door and 80 for the kitchen, E008's guides (E009's
   copies), each until a stop rule fires; then `explore.py verify` on each goal entry.

## Prediction (before running)

| | predicted |
|---|---|
| off check | E007's kitchen run, as above |
| test (a), ready on | the same route: goal from the plan's records at 3216, 488 frames, 11,712 qf, 2 replans, 36 inputs; tries 1 + 21 + 294 + 1 = 317 plus at most 3 per window cancelled or duplicate; verify 0 differences; job wall 350 to 650 s (off: 471 to 549 s), the third window's 294 tries most of it |
| door | 1 job (at most 2): state:316 = 1 at 3129 ± 10 (401 frames) while the engine idles, verify 0 differences; job wall 10 to 25 minutes, most of it the ring(33) reading |
| kitchen, jobs | reached; 5 to 40 jobs (most likely 10 to 25) |
| kitchen, per try | job wall median 40 to 200 s (VM-equivalent 15 to 70 s); most 10 to 35 minutes, set by a fresh ring(33) reading and its refinements, not by the window search; the window search's wall per try at most 150 s, and no window over 120 s (E009: about 8 minutes per window, 16 to 20 minutes and more per try on the VM, about 22 and 45 to 56 minutes laptop-equivalent) |
| kitchen, total | 0.5 to 4 hours wall from the first job's start to the goal; the goal chain 488 to 1,500 frames from 2728, 2 to 8 segments; verify 0 differences |
| both | every return the same as its entry; no window record with a cancelled try chosen; per job Python under 1.3 GB, reader under 1.5 GB |

## Success threshold

- Yes: the kitchen and the door both reached by the explorer from the boot with E009's and E010's switches on, each
  goal entry verified from boot with 0 differences, within the falsifier bounds, and no try whose window search took
  more than 5 minutes of wall on this laptop.
- Partly: one goal reached and verified, or both reached with some try's window search over 5 minutes.
- No: neither goal reached.

## Stop rule

- The off check differing from E007's kitchen run in stop, frames or replans, or test (a) with ready on not reaching
  the goal from the plan's records or differing from boot: stop and find why before the runs of step 3.
- Per stage, E009's stop rules: the first goal entry (then verify); an exhausted archive; non-idle stagnation; a job
  failing twice on the same entry with the same error; a return or verify that differs (stops everything until
  explained).
- The prediction's falsifier: no job starts once 80 jobs have started for the kitchen (twice the upper bound of 40),
  4 for the door; the running ones commit. If the goal is not among them the stage is written up as "no".
- When a stop rule fires: write up the per-try times, the windows' tries, the best chain and why the jobs ended, and
  stop. E010 adds no further idea.
- The harness's, the window search's and the reader's limits are E006's; the guard's 3 GB and the disk's 8 GB floors
  are the machine's: engineering watchdogs, reported where they fire.
