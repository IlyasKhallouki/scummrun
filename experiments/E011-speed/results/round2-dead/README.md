# E011: the no-plan proofs merged into round 2, results (2026-11-03, written after the runs)

plan.md's two amendments "after round 2: the no-plan proofs merged into round 2" and "during the merged build's
measurement: the heavy lock held across the fast sequence". Branch e011-r2 gets e011-deadtarget (aacf668) and e011
(0bde069). The merged build is in `/var/tmp/fast-mi/target-e011-r2` (`~/.cache/fast-mi-e011/env-r2d.sh`) and round 2's
own binaries in `target-e011-r2/r2`. Laptop, seed 1, nice 10. The heavy lock was shared with another experiment (E012)
throughout. Files here:

- `gates.md`: the gates the merge touches, with everything off and on;
- `tries-fast.md`, `tries-ref.md` (and `.json`): B3 by try and by part (report.py's output);
- `raw/`:
  - B3's and the bit:118 tries' times and loads, the stopped fast starts' times;
  - G11's comparisons with B0 and its verifies;
  - the dead-target gate's report, G12's watcher, the run's log.

"Everything on" (ON3) is round 2's ON2 with `FMI_PLAN_DEAD=1` and `FMI_PLAN_DEEPEN=120`; "everything off" (OFF3) is
round 2's OFF2 with both off.

## In short

- **Exactness.** Every gate the two branches touch passes with everything off and with everything on:
  - G7, G9, G10 and G11;
  - the dead-target gates 1-5.

  The 15 tries with everything on equal B0's in everything, under the fast and the reference settings.
- **The 15 tries.** They have no dead target, so the merge changes nothing in them but the proofs' cost.
  - The median try is 4.3 s, against 4.5 s for round 2 alone and 4.8 s for round 1, all on the same load.
  - The longest is j00000 at 139.1 s, against 121.3 s and 115.6 s. Of its extra 18 s over round 2 alone, 14.6 s is in
    its reading, the same reader binary in both, and 3.2 s in its plans, of which the proofs are 0.25 s.
  - The proofs take 0.25-0.90 s per ring(33) request and under 0.01 s per ring(38) request: 3.2 s over the 15 tries.
- **The check's bit:118 = 1 tries.** With everything on they end in 12.1 s and 25.3 s with "no plan (the planner
  proved none)". Round 1 and round 2 alone take 189.9-203.6 s each and end on the planner's 180 s limit.

## B3, the fast settings

The warm regime with the job memo off. Three resident processes ran their tries alternately, one by one, with the heavy
lock held across the whole sequence, at load 2.0-4.3. Walls in seconds; CPU is the resident process's tree.

| binaries | median, 15 | ring(33), 7 | ring(38), 8 | mean | max | sum | sum without j00000 | CPU median | CPU sum |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| round 1 (r1/fmi, Gbfs) | 4.8 | 17.1 | 1.75 | 17.3 | 115.6 | 260.1 | 144.5 | 12.3 | 579.1 |
| round 2 alone (r2/fmi, ON2, the Rust lama-first-costs) | 4.5 | 5.8 | 1.15 | 12.7 | 121.3 | 190.0 | 68.7 | 8.0 | 452.4 |
| merged, everything on (ON3) | **4.3** | **6.1** | **1.2** | 14.0 | 139.1 | 209.5 | 70.4 | 8.2 | 493.1 |

By part, the median, the largest and the sum over the 15 tries (wall seconds; report.py's parts):

| part | round 1 | round 2 alone | merged, everything on |
|---|---|---|---|
| return | 0.01 / 0.32 / 0.4 | 0.00 / 0.12 / 0.2 | 0.00 / 0.11 / 0.1 |
| idle | 0.20 / 0.30 / 3.0 | 0.10 / 0.10 / 1.1 | 0.10 / 0.10 / 1.2 |
| read | 0.21 / 94.7 / 146.2 | 0.05 / 106.0 / 140.2 | 0.07 / 120.6 / 153.7 |
| plan | 0.66 / 19.7 / 107.1 | 0.33 / 14.8 / 47.3 | 0.40 / 18.0 / 53.0 |
| play | 0.02 / 0.60 / 3.1 | 0.02 / 0.28 / 1.4 | 0.02 / 0.32 / 1.5 |
| the try | 4.8 / 115.6 / 260.1 | 4.5 / 121.3 / 190.0 | 4.3 / 139.1 / 209.5 |

- **The first try, j00000.** It makes a fresh ring(33) reading and two refinements, then two plans. All three
  sequences use the same reader-scope binary, and round 2 alone and the merged build pass it the same switches.
  - Round 2 alone: 40.8 s for the fresh reading, 42.4 s and 22.8 s for the refinements, 7.0 s and 1.2 s for the plans.
  - Merged: 48.6 s, 50.8 s and 21.2 s for the same, then 9.9 s and 1.0 s for the plans. The first plan's proofs took
    0.25 s of that.
  - The reading's CPU rose with its wall: 133 CPU s for the merged sequence's fresh reading, against 115 for round 2's.

  j00000 also varies from start to start (`raw/b3-fast-stopped-starts-times.jsonl`):
  - the first start: 99.6 s for round 1 and 111.9 s for round 2 alone, at load 3.5-3.7;
  - the second: 296.9 s, 187.5 s and 237.5 s for the merged build, at load 8-15.
- **The other 14 tries.** They sum to 70.4 s, against 68.7 s for round 2 alone (+2.5%). Per ring(33) try the merged
  build is 0.0-0.8 s slower; per ring(38) try, -0.2 to +0.1 s.
- **The proofs in the run** (`dead` in each plan's record, the early stop on). 14 searched requests and one cache hit:
  - ring(33), 6 requests of 97 k-155 k operators: 0.25, 0.45, 0.45, 0.48, 0.59 and 0.90 s;
  - ring(38), 8 requests: 0.007-0.010 s;
  - none claimed a target dead.
- **G11.** The merged build's 15 tries equal B0's in everything (stop, target, frames, replans, refinements, inputs and
  new entries), and so do round 2 alone's. Round 1's Gbfs plays another route in 3 tries, as in round 1. The merged
  build's 43 new entries verify from boot with 0 differences. G12: 0 python processes in the benchmark's fmi trees
  (645 samples).

## B3, the reference settings (G11)

The merged build with everything on and with everything off, interleaved try by try under a lock taken per try, at load
4.4-13.9. Both equal B0's in everything in all 15 tries, and 43 + 43 new entries verify from boot with 0 differences.
Times, at that load:

- everything on: median 5.3 s, ring(33) median 11.9 s, max 143.3 s, sum 253.5 s;
- everything off: median 6.2 s, ring(33) median 70.8 s, max 192.9 s, sum 619.4 s.

## The check's bit:118 = 1 tries

Each try is `fmi try` on a copy of the check's try 15 (entry 36, frame 2771) or try 16 (entry 26, frame 2770), with
the target forced. Each runs cold, in its own process, with the check's limit (FMI_PLAN_TIME=180). The refinement
reading is B0's j00000 refine-01, reused by every try ("reused (e009 keys)"). The tries alternate one by one.

| | try 15 | try 16 | how the attempt ends |
|---|---:|---:|---|
| round 1 (r1/fmi, Fast Downward's binary; the check's settings) | 201.1 s (load 10.0) | 203.6 s (load 4.4) | planner timeout (180 s) |
| round 2 alone (ON2, the Rust lama-first-costs) | 203.1 s (load 7.9) | 189.9 s (load 1.3) | planner timeout (180 s) |
| merged, everything on (ON3) | **25.3 s** (load 7.0) | **12.1 s** (load 1.4) | no plan (the planner proved none) |

On try 15 the merged build's request took 19.3 s: reductions and build, then h^1 in 0.13 s and h^2 in 5.9 s. The full
h^2 ran, since the goal never enters its rows. Round 2's Rust lama-first-costs does not end the search on this task
either: it searches the whole 180 s.

## The recorded requests with everything on (dead-target gate 1)

The replay served Fast Downward's groups with the Rust lama-first-costs and the fast forms. It took 7.4 min at load
about 2; the amendment's replay with Fast Downward's binary took 35 min at load 3-15. Of the 26 found plans with that
search, the search itself took a median of 0.08 s (p90 0.46 s, max 0.53 s), plus 0.8-1.8 s for the landmark graph and
the relaxed task. Before, Fast Downward took a median of 37.3 s and a max of 119.8 s.

The check's eight requests with a plan on the refinement reading took Fast Downward 88.9-119.8 s of search at load
3-15. They take the Rust lama-first-costs 0.07-0.53 s of search and 11-16 s per request in all, at load about 2. Of
those 11-16 s, 5-7 s is the gate's own check of the plain h^2 fixpoint.

The amendment's rule sets L0 to the p90 of the found plans' search seconds on the warm path, rounded up to 10 s. On
the merged build's path with everything on, that rule gives 10 s. The runs here used 120 s, the value the amendment
measured with Fast Downward's binary. No target in them was deferred.

## Against the prediction

| predicted | measured |
|---|---|
| every gate passes with everything off and on | yes |
| the merged build's 15 tries equal round 2's in everything | yes: both equal B0's in everything |
| its median within 0.5 s of round 2's | yes: 4.3 s against 4.5 s |
| each ring(33) try 0.4-1.5 s slower than round 2's | no: 0.0-0.8 s on six tries; +17.8 s on j00000, 14.6 s of it in the reading and 3.2 s in the plans (the proofs 0.25 s). The proofs take 0.25-0.90 s per searched ring(33) request |
| its sum 1-5% above round 2's | no: +10.3% (209.5 s against 190.0 s), from j00000's reading; +2.5% without j00000 |
| bit:118: the merged build 15-25 s per try | 12.1 s and 25.3 s, one just under and one just over the range |
| bit:118: round 1 and round 2 alone 185-200 s | 189.9-203.6 s; three of the four just over 200 s |
