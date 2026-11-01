# E004: Fast and exact, Part I on the laptop

Date planned: 2026-10-26
Research question it serves: RQ1 (a Part I run that fits), then RQ2 (the shortest route)

## Question

Can proven reductions alone (D9) make the whole pipeline (reading, task, planning, replay) run Part I on this
laptop? Once it does, can it also find the shortest route? Each reduction is built and checked on the kitchen, then
scored on check scopes that were never used to tune it: the bar, the jail, k = 1 and k = 2.

## Target

The target machine is this laptop: 8 cores, 23 GB RAM, 35-47 GB free on /var/tmp. The target is seed 1 on 8
threads, with nothing else heavy running.

| Part I stage | today (measured or projected) | target |
|---|---|---|
| boot and start check | 26-52 GB (taskcore twice) | ≤ 10 GB |
| reading, both passes | 18,000-23,000 CPU s; one job > 4.8 GB | finishes, ≤ 8 GB |
| packed file and action lines | 5.4-11 GB; 16-30 min | ≤ 3 GB |
| task, reduced once | (not done today) | ≤ 10 GB, kept in memory |
| each replan | 26-65 min and 13-26 GB | (d) and the planner on the kept task, no reload |
| play and verify | not measured | Part I's end, 0 differences from boot |
| whole run | 43-540 h | **read, planned and played here, ≤ 16 GB in all** |

If the model cannot reach the Part I end, replans are timed on the Part I key goals it can reach.

## Baseline

The baseline comes from the E004 profile, with `--live-locals` on, nice 10 and 3 threads. CPU s and peak memory
are scored, because wall times moved with loads of 5 to 46.

| | kitchen (dev) | bar | jail | k = 1 | k = 2 |
|---|---|---|---|---|---|
| records | 72,126 | 66,030 | 57,866 | 237,465 | 519,755 |
| reading CPU s / peak | 96 / 0.8 GB | 152 / 0.3 GB | 255 / 0.6 GB | 570 / 1.1 GB | 3,107 / 2.9 GB |
| taskcore CPU s / peak | 13 / 0.1 GB | 7 / 0.1 GB | 11 / 0.3 GB | 22 / 0.7 GB | 104 (wall) / 2.5 GB |
| plan | 10 steps; optimum 7 | door: 6 steps | none | none | not run |

The bar reaches and plays state:316 = 1 (the kitchen door open). k = 1 has 85 actions that write it, but none is
reachable. The cause is unknown.

## Why the kitchen alone could mislead

- **No jail dialogue.** Room 31's local 202 takes 80% of the jail's reading CPU, and it is the unit that blew up
  Part I. Nothing in the kitchen is shaped like it.
- **One kind of loop.** The kitchen has room 28's random-draw counters (8.7% of its CPU). It does not have the
  actor-position and scale loops of rooms 35 and 83, which take 34% of k = 2's event runs.
- **Gains scale with rooms.** Step (d) removes 61% of the kitchen's records but 92% at k = 1. Reading CPU per
  record goes from 0.64 ms (kitchen) to 3.0 (k = 2) and 3.5-3.8 (Part I's inputs).
- **Search is easy there.** The kitchen's optimum takes 0.04 s of search. The bar door task never left f = 5.
- **The bar is not independent for reading.** It shares room 28's local 220 (about 50% of both readings) with
  the kitchen. The jail, k = 1 and k = 2 are independent.

The kitchen proves that a change is exact, and the proofs hold for any scope, so building on it does not overfit.
The gains are measured on the check scopes.

## Reductions, in order

Each one sits behind a switch that is off by default.

1. **Reduce once, replan on the small task** (taskcore).
   - *Proof:* The fold, (a) and (b) keep every plan and do not read the start. Relaxed reachability is
     monotone, so (d) from the first start keeps every plan from a later start whose values it already reached.
     Bans only add preconditions. (c), (e) and (f) keep only optimal plans, so they run again at each replan,
     after the bans. A start with a new value gets today's full run.
   - *Gain:* On the kitchen, a replan's task should cost a small part of today's 13-18 s. Part I would load its
     task once (13-26 GB), not once per replan.
   - *Test:* From the kitchen's 9 committed replan starts and the bar door run's starts, the optimal cost is
     the same.
2. **Pass 2 reruns only the jobs that touched a widened domain** (reader).
   - *Proof:* The machine is deterministic, so a job that read and wrote no widened fact gives the same records.
   - *Gain:* Pass 2 is 47-58% of the reading, and it changes only 2.8-9.1% of the records. Expect 35-45% less
     reading CPU on every scope.
   - *Test:* The output files are byte for byte the same with the switch on.
3. **Forward reachability in the reader.**
   - *Proof:* A job or event runs only once (d) has reached its pinned values, so the reader skips exactly what
     (d) drops. A replan start with new values extends the reading. That is safe because reachability is
     monotone.
   - *Gain:* It drops 61% of records on the kitchen and 92% on k = 1. The files and taskcore's memory shrink by
     about the same share.
   - *Test:* The reader writes exactly the records that (d) keeps from today's reading, and taskcore reduces
     them to the same task.
4. **Exact loops** (reader).
   - *Proof:* The machine never branches on an unknown's text, so canonical names for unknowns keep the same
     paths. A local that reaches no fact, start or wait choice cannot change a record (live-locals' proof).
   - *Gain:* It drops 11.4% of the kitchen's records, and its rounds should fall from 1,028 to under 60. At
     k = 2 it affects 13 wait points and 34% of the event runs. The jail has none of these loops, so it is a
     control. The change also gives back the lists SIG_LIMIT folds away today (4 in the kitchen, 22 at k = 1).
   - *Test:* No list reaches SIG_LIMIT. Every unfolded record is kept, and reachable values grow only through
     the lists that are given back.

## Rule for keeping a reduction

A reduction is kept only if all of these hold:

- Its proof is written down before the code.
- With the switch off, every output on all five scopes is byte for byte today's.
- With the switch on, all five scopes reach the same fact values by (d). On the kitchen and the bar door task,
  the optimal cost is the same (A* with landmark cost partitioning), and every lama-first plan runs on today's
  task. The bar door run from seed 1 reaches goal #2 in the engine, with 0 differences when verified from boot.
- It saves at least 5% CPU or memory, above the noise floor, on two check scopes, one of them k = 1 or k = 2. A
  gain on the kitchen alone does not count.

## Method

- Commit: recorded at each step.
- Runs: on the laptop, nice 10, at most 3 threads while it is shared, seed 1. Each number is the lower of two
  runs. The 8-thread wall time is projected from CPU s and the measured busy share. The kitchen's start is read
  from the committed files and never booted.
- Held-out data: the four check scopes, tuned on never. A bug found there is fixed, all five scopes are rerun,
  and the report counts it. Part I is only counted (pass 1's inputs and round 1) until its final run. Seeds 31-35
  are not used.

## Prediction (before running)

- Reduction 1 makes Fast Downward, not taskcore, the cost of a replan.
- Reduction 3 is the largest gain: 55-65% of records on the kitchen and 85-95% at k = 1 and k = 2.
- With all four, Part I's reading finishes on the laptop, its packed file is under 3 GB, and the whole run fits in
  its memory. After that, Fast Downward's parse and landmarks bind each replan.

## Success threshold and kill criteria

- Yes: Part I is read, planned and played here in ≤ 16 GB, with kept reductions only.
- A reduction that is still not exact after two fixes is dropped and reported as a negative result.
- After reductions 1-4, count Part I's pass-1 inputs and round 1 on the laptop and project the full run.
  - If the projection shows the reading will not finish here, stop and write up the stage that binds. The next
    candidates come from the survey: merging lists that are equal apart from their numbers (11.6% of the kitchen's
    records), h² mutexes, and re-reading only new slot lists. The scope is not capped (D4), and the run does not
    move to a bigger machine.
  - If it finishes but does not fit, make engineering changes that keep the outputs the same: stream taskcore's
    load (2.4 times the packed file today), intern the reader's strings, and keep all 8 threads busy (the
    kitchen uses 1.55 of 3).
  - If it meets the target, run Part I once.

## Then: the shortest route

This phase starts once the run fits. Each record gets the frames of the wait it ends at, and a rule for parallel
waits is planned first. Picks move to just before the operator that reads them, which is exact because picks cost
0. Search uses A* with landmark cost partitioning. On the kitchen this proves 7 steps against lama-first's 10.
The check is the bar door task.

## First three steps

1. **Yardstick, no reduction.**
   - `bench` runs every stage on a scope and records CPU s, peak memory and each output's sha1.
   - `exact` compares two runs on the keep rule's checks.
   - Run today's pipeline twice on four scopes and once on k = 2 to get the noise floor.
   - Look into why k = 1 cannot reach state:316 = 1.
2. **Reduction 1.** Run its test, measure replans on the kitchen, the bar door task, k = 1 and k = 2, and update
   the Part I projection.
3. **Reduction 2.** Check that the outputs are byte for byte the same on five scopes, record the CPU each one
   saves, and update the projection.

## Results

Written after the runs, from the lab notes: notes/log/2026-10-31-e004-reach.md, 2026-11-01-e004-p1r.md,
2026-11-01-e004-rough.md and 2026-11-01-e005.md, and for reduction 1 notes/log/2026-10-26-e003-noreload.md. Laptop,
nice 10, 3 threads, seed 1, unless a line says otherwise.

**What each reduction measured.**

- **Reduction 1, replan on the kept task** (`taskcore serve`). The planner input is the same bytes on every recorded
  replan. A replan costs 0.4 to 2.0 CPU s on k = 2, against 105 to 164 before, and 5.0 to 5.7 on the kitchen, against
  13 to 15.
- **Reduction 2, pass 2 reuse** (`pass2_reuse`). The packed file, records and counters are the same on the bar, k = 1,
  k = 2 and the jail. With `--reach` it saves 15 to 27% of the reading CPU (k = 2: 266.1 to 220.2 CPU s), and costs
  3.5 to 5 times the peak memory (k = 2: 552 MB to 2,751 MB), since pass 1's records are held.
- **Reduction 3, forward reachability** (`--reach`). Reading CPU falls 2.2 times on the kitchen (68.3 to 31.8 s), 11.5
  times on k = 1 (1,583.8 to 138.3 s) and 880 times on the jail (439.7 to 0.5 s, its room is not reached). k = 2 takes
  234.7 CPU s with it; without it the reading was stopped unfinished after 2,707 CPU s. Records fall 61% on the
  kitchen and 83% on k = 1, against the 85-95% predicted. Against (d), the bar and the jail match once two named
  classes are applied; the kitchen differs by 6 records against 8, and k = 1 by 276 against 342, from runs split
  differently. The kitchen's optimal cost is 7 from both readings.
- **Reduction 4, exact loops.** Not built.
- **havoc_cut** (a path the step limit ends becomes a havoc record). The check scopes give the same files, since no
  path there reaches the step limit. On Part I it lets the job that stopped the reading (room 28's local 204, a loop
  on var:164 that has no bound) pass, as 9 havoc records per job.
- **shared_runs** (jobs that differ only in an argument they never read share one run). The output is the same. It
  saves 2.3% of the reading CPU on k = 1 and on k = 2, under the 5% keep rule, so it stays off. It does not apply to
  the `[202@85]` dialog jobs that fill Part I's round 4, because their runs read a dialog verb.
- **The rough reading** (`--step-limit N`, every job read to N steps). On the bar at N = 100 it reaches every value
  the full reading reaches, and 4,344 more; its plan has 6 steps, as the exact one, and the last is a havoc step. On
  Part I no N finished. At N = 100 and 300, havoc records that name every unit (576) fill the reading, and it held 49
  to 56 GB within minutes on the VM. At N = 500 and 1,000 almost no path is cut, so it costs what the full reading
  costs: at N = 1,000 it was in pass 1's round 8 after 22,554 CPU s.
- **E005, havoc_args and havoc_rooms.** Both computed starts resolve (global 73 names 58 and 200, global 23 names
  none), but every Part I havoc record at N = 300 still names 564 of 573 units. That reading reached 113 GB on the VM
  in 250 s, and 5.5 GB in 100 s on the laptop.

**Part I, the exact reading.** With `--reach`, havoc_cut, shared_runs and fix round 1's switches, the reading was
stopped in pass 1's round 5, after 8,001 CPU s, with 23,233 records and a peak of 3.26 GB. In round 4, five dialog
jobs, `dialog_line(120..124)[202@85]`, took 3,765 of the round's 3,785 job seconds.

**The stop rule fired.** Part I's reading did not finish, on the laptop or on the VM, with any reduction or switch. So
there is no Part I task, no plan and nothing played, and the success threshold was not met.

**The stage that binds** is the reading's pass 1.

- In the exact reading, a few jobs that fork on facts and merge slowly take the time: the `[202@85]` dialog family
  (room 85's local 202, then global 114's `loadRoom(49)`), the sentence jobs that reach the same room change (4,061
  job seconds of round 5), and `event(script:114@101)` (1,498 s at N = 1,000). Their paths are under 1,000 steps, so
  the time comes from forks and merges, not path length.
- In the rough reading, the havoc record's size binds. A computed `startObject` in globals 9 and 23 names all 202
  objects' verb entries, and `actorFollowCamera(VAR_EGO)` with the ego's room unknown names every scope room. So each
  record names 564 of 573 units and 13,589 events.

Not measured on Part I: taskcore, the planner and play, because no reading finished.
