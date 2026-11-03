# E012: the kitchen rerun

plan.md "Runs" and the amendment "the kitchen rerun: how it runs" (written before the run). Branch e012 at ba63331,
built into its own target folder; the engine is end's `svm-e012-end` (patches 0001-0005, scummvm cc6615ea). Laptop,
seed 1, nice 10, b = 2, under the guard; the driver took the heavy lock for each job with a 5-room scope. Scripts:
`kitchen/run.sh`, `kitchen/verify.sh`, `kitchen/verify-all.sh`. The numbers come from `fmi-checks`' `kitchen-report`
over the archive's files (`report.md`, `report.json`). The archive is `/var/tmp/fast-mi/e012-archives/kitchen`;
its readings and entries stay there.

## Result

- **The goal was reached.** The driver stopped on "goal" at the 53rd committed try (j00052), entry 150, owner:566 = 1.
  No other stop rule fired before it: stagnation over T never fired, 68 cells were still selectable at the end, and the
  falsifier stood at 53 of 80. No step was manual.
- **It verifies from boot** with 0 differences over the chain's 32 checkpoints, owner:566 = 1 at the end, three
  times:
  - the driver's own verify at the goal stop;
  - `fmi explore verify` in a fresh lean engine (end's);
  - E008's `explore.py verify` on svm-w3b, on a copy of the archive's files.

  All three give 15,688 frames from the root (2728 to 18416), qf 376,512, 85 inputs and 8 segments.
- **Every entry of the archive verifies from boot:** 150 of 150 through `fmi explore verify` (`verify-all.txt`).
- **Wall.** The driver ran 1,292 s (21.5 min) from start to goal stop, with two workers. The tries' own wall adds up to
  1,658 s (0.46 h): median 15.2 s, max 163.6 s. 53 tries committed, 0 errors, 0 guard kills. fmi's peak resident set
  was 8.3 GB; the lowest MemAvailable was 5.6 GB.

## The goal's chain

| entry | frame | room | made by | try | what the try did |
|---:|---:|---:|---|---|---|
| 0 | 2728 | 33 | the boot | | |
| 44 | 10102 | 35 | the affordance step (sentence idle, after two empty windows) | j00013 | from the root: 544 trials, 10 kept, rooms 34 and 35 new |
| 55 | 10162 | 29 | an attempt's checkpoint | j00017 | target owner:377 = 1, reached |
| 85 | 13767 | 35 | an attempt's checkpoint | j00021 | an empty window, then a sentence-idle step |
| 123 | 13776 | 35 | an attempt's checkpoint | j00045 | an empty window |
| 135 | 13924 | 33 | an attempt's end | j00047 | target state:428 = 1, reached |
| 142 | 17917 | 28 | the affordance step (dialog, at R0) | j00048 | room 28 new; R0 on var 194, then 4 dialog lines tried, 1 kept |
| 146 | 18056 | 28 | the affordance step (dialog, at R0) | j00051 | the goal proven dead from 142 and from its 3 wait stops; 13 more targets proven dead; R0, then a dialog line kept |
| 150 | 18416 | 41 | an attempt's end | j00052 | the goal, planned in 10.4 s from 146 and played |

- The last two steps are the ones plan.md's research pointed to. From entry 142, a room-28 entry idle in a dialog, h^2
  proves the goal dead in 2.4 s. It also proves it dead from each of the wait's three stops (the goal again). From 146,
  one dialog line later with control back, the same goal plans and plays in 10.4 s.
- The goal's walk to the cook's door is found by E010's play when ready: 253 forks over 263 frames, 2.3 s. The door
  opens at frame 18329. That is entry 146's frame plus the paused slot's due delay, 273 frames, and it is where the
  wait stopped on the door. The due-frame window is not in the build.
- The first room-28 entry with control (138, frame 17526, j00049) got a plan for the goal too. Its walk's window ran
  play when ready's 294 tries, from 17526 to 17821, each spent (the walk's records did not run). The door opened at
  17917, at the paused slot's due frame (+391, where the wait stopped), 96 frames after the last try. The window then
  ran to its 3,600-frame bound. From 146 the due delay was 273 frames, inside the 294 tries.
- **Where the frames went.** Four windows on the chain ran to E010's 3,600-frame bound with no end point: two in
  j00013, one in j00021 and one in j00048. That is 14,400 of the chain's 15,688 frames. Each window's parent idles to
  the bound, and the next entry (an affordance outcome or a checkpoint) continues from the end of that window.

## Against E011's kitchen check

| | E011's check (round 1, stopped by hand) | E012's rerun |
|---|---|---|
| result | 65 tries, goal not reached | goal reached at try 53, verified from boot |
| entries, rooms | 226 entries in 6 rooms | 151 entries in 9 rooms (33, 38, 35, 29, 34, 28, 41, and 31, 32 outside the 10 allowed, entered by affordance trials) |
| try wall | 6,077 s (1.69 h), median 31.6 s, max 998 s | 1,658 s (0.46 h), median 15.2 s, max 163.6 s |
| empty-window tries | 14 tries, 3,355 s (55%) | 13 tries, 705 s (42.5%), 536 s of it in their affordance steps |
| R0 tries | 17 tries, 1,273 s (21%) | 24 tries, 586 s (35.3%), each followed by the affordance step |
| tries that reached their target | 28 tries, 1,084 s | 10 tries, 115 s (6 from the plan, 4 not from the plan) |
| no attempt played | | 5 tries, 221 s (13.3%): in 4, every target stopped on the reading's reach extension; 1 had no target; each then ran the affordance step |
| other | | 1 try, 32 s: the goal from entry 138 (room 28, with control); its walk's window ran empty to the 3,600-frame bound (below), then the reach extension failed |
| var:103 the last target | 29 of 65 | 0 of 53; a refined key's value last in 0 of 53; var:103 attempted at most once per try |
| requests at the planner's limit | 10 at 180 s | 1, at deepening's first limit of 120 s (state:428 = 1, j00044, deferred) |

The wall by part, summed over the 53 tries (s): return 1.1, read 491.8 (29.7%), T 16.1, wait 6.1, plan 362.0 (21.8%),
play 45.0, affordance steps 728.8 (44.0%), rest 8.8.

## Each mechanism

- **The wait.** It ran at all 53 entries. 32 were static, and 21 ended W frames after their last stop: 108 stops in
  all, 2 of them kept as entries (j00033). 36 entries had a paused slot with a due frame. Median 0.087 s, max 0.403 s
  (one wait, 4,491 frames, over 0.3 s). Every wait ended by W, as none ended in the game's end.
  - Room 28: three of the four waits stopped on the cook's door exactly at the paused slot's due frame (entries 138,
    140 and 146, at +391, +341 and +273). The fourth entry (142) sits at the frame the door opened. Its wait stopped
    when the door changed again, 110 frames later.
- **The goal again.** 6 requests from wait stops, in j00050 and j00051 (room 28). h^2 proved each dead, the goal fact
  outside its fixpoint, in 1.8-8.1 s of request wall. None played.
- **Due-frame windows:** 0 (not built).
- **The delay fork.** 84 window records, 2 triggered (j00013 and j00041: one draw check each, of script 47's events in
  room 35's scope). Neither draw had a range or a share, so K was not computed. By amendment (2), the fork changed no
  choice: the first end point, a deviation at k = 0, was taken, and no candidate was kept.
- **The affordance step.** 42 steps, 5,767 trials, 33 kept. No trial ended the run.

  | trigger, idle kind | steps | trials | kept | third clicks | walk clicks | wall s |
  |---|---:|---:|---:|---:|---:|---:|
  | R0, dialog | 24 | 93 | 13 | 0 | 0 | 13.1 |
  | empty window, dialog | 3 | 8 | 2 | 0 | 0 | 0.7 |
  | empty window, sentence | 10 | 4,510 | 15 | 2,088 | 76 | 535.5 |
  | no plan, dialog | 2 | 6 | 1 | 0 | 0 | 0.5 |
  | no plan, sentence | 3 | 1,150 | 2 | 377 | 35 | 179.0 |

  - Dialog steps take a median of 0.43 s and at most 1.27 s. Sentence-idle steps take 32-130 s each.
  - Of the steps' 728.8 s, 711.3 s is the phase that reads each trial's outcome: its facts and novelty, and the wait's
    stops run after each trial (W = 1,600 frames or more, observed frame by frame). Forks take 5.2 s, inputs and runs
    11.4 s. That comes to 126 ms per sentence-idle trial, against A3's 6.5-7.9 ms measured without the wait after
    each trial.
- **Dead marks:** 0. No run ended in quit, restart or pause.
- **Targets per key:** var:103 was attempted at most once per try, and no try's last target was a value list. Of the
  probes, 48 planned, 39 were proven dead, 1 reached deepening's limit, and 142 stopped on the reading's reach
  extension. Those 142 were in the 4 tries from entries whose state the extended reading did not reach.
- **T** grew from 559 atoms after the first try to 976.

## Calls per frame

Exact, from the F2 `rng`, over the window searches' parent frames, by the room of the try's entry:

| room | frames | calls | least | most |
|---:|---:|---:|---:|---:|
| 28 | 3,866 | 9,009 | 1 | 4 |
| 29 | 28,883 | 29,905 | 0 | 11 |
| 33 | 10,806 | 11,263 | 1 | 12 |
| 34 | 340 | 390 | 1 | 13 |
| 35 | 25,216 | 25,938 | 1 | 11 |
| 38 | 17 | 27 | 1 | 2 |

Room 29 had 11 frames with no call (of 28,883), which M1 had not seen. In room 28, 3,219 of 3,866 frames had 2 calls
and 632 had 4.

## Where W or K bound

- W ended every wait (53 of 53). None ended in the game's end or on the watchdog.
- K was not computed in either triggered window: the draw had neither a range nor a share.

## Against the prediction

| | predicted | measured |
|---|---|---|
| gates | pass | pass (the integration) |
| wait | ≤ 0.3 s; the cook's due frame at room-28 entries with control; static in room 38's dialog | 52 of 53 ≤ 0.3 s (max 0.403 s); the door at the due frame from 3 of 4 room-28 entries, the fourth at the door's own frame; room 38's waits static |
| reached | yes, verified, in 3-25 tries (most likely 5-15) | yes, verified three times, 0 differences, in 53 tries |
| chain | 488-1,300 frames from 2728 | 15,688 frames; 14,400 of them are four empty windows' 3,600 frames |
| R0 | none without the affordance step; 3-20 steps; ≤ 0.5 s a dialog, 1-5 s a sentence idle | none without it (24 of 24, and 13 of 13 empty windows); 42 steps; dialog median 0.43 s (max 1.27 s); sentence idle 32-130 s |
| targets | ≤ 1 per key per try; a refined key's value last in ≤ 10% | var:103 at most once; 0% |
| planner limits | ≤ 2 | 1 |
| delay forks | 0-2 triggered, k ≤ 3 | 2 triggered, no K, the k = 0 try taken |
| wall | 0.1-1.0 h; empty-window tries ≤ 20% | 0.46 h; empty-window tries 42.5% |
| calls per frame | ≥ 1 in every room, up to about 10 | 0-13; 11 frames of room 29 with none |

## Files

- `report.md`, `report.json`: `kitchen-report` over the archive: every try with its parts, stop, target, entries and
  mechanisms, then the sums above.
- `summary.json`: the driver's summary at the goal stop.
- `verify-fmi.json`, `verify-py.json`: the goal entry's records from `fmi explore verify` and `explore.py verify`.
- `verify-all.txt`: every entry's line from `verify-all.sh`.
- `run-log.txt`: the driver's lines (the memo's lines left out).
