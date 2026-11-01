# E007: The harness adds the fact a split names, and goes on

Date planned: 2026-11-01
Research question it serves: RQ1 (E003's path through the cloud, under B1's strict matcher and E006's split)

## Question

With E006's switches on (`wait_pass_cost`, `split_wait`), the kitchen from the boot reaches owner:566 = ego from the
plan's records, and k = 2 from the boot still stops at walk_to(428): every child's click record fails (I3) on var 102,
which the sentence script writes at [002B] after its test at [0000] on var 103, and the k = 2 reading holds no fact of
var 103 (B1's class split). The open-door sentence before it deviates on the same cause.

Can the system close that refinement loop itself? When a strict window ends empty, or a step deviates, and B1's cause
is class split on a variable the reading holds no fact of, the harness adds that variable to the reading's facts,
reads the scope again, rebuilds the task, replans from the engine's current state and goes on. Does k = 2 from the boot
then reach state:316 = 1, verified from boot, with no manual step? And does the kitchen still reach its goal?

## Baseline

Branch e006 at 5715940 (`experiments/E006-passed-wait/plan.md`, results section; `notes/log/2026-11-01-e006.md`), seed
1, the harness's boot start (frame 2728, qf 65,284), the strict window search with every cost switch (walk's
`run.sh`) plus `--window-split-wait`, on readings made with `wait_pass_cost`:

| test | stop | goal | frames (qf, s) | plans, replans | children | search CPU s | peak memory |
|---|---|---|---|---|---|---|---|
| kitchen | the goal at 3216 | from the plan's records | 488 (11,712, 48.80) | 3, 2 | 501 | 275.1 | Python 298 MB, taskcore 88 MB |
| k = 2 | the same empty window twice on walk_to(428), at 9996 | no | 7,268 (174,432, 726.82) | 3, 2 | 7,268 | 359.9 | Python 736 MB, taskcore 1,121 MB |

The k = 2 run's causes, as B1 gives them: plan 0's open-door sentence sentence(2,428) deviates at 2796 ("effect
var:102: wrote another value; script:2 [002B]; var 103 = 428 (not a fact of the reading); local 2 = 0 (unknown); class
split"), and both walk_to(428) windows are empty on the same cause. The kitchen's two replans have causes that name no
such variable: a refuted event (class none given) and the cook's x (var 0 = 306, a position, class split).

The readings: kitchen 41,877 records, 40,773 actions (packed sha1 41c31b6d); k = 2 451,987 records, 450,967 actions
(f5ec2354), read in 121 s wall, 373 CPU s, peak 3,948 MB. In the k = 2 reading 14,678 actions carry the non-fact check
on var 103 at [0000] and 17,624 the one on var 104 at [0007]. Plan 0's task: kitchen 44,605 operators, k = 2 143,417.

## Method

### How the reader chooses its facts today

`domains.rs` (domains.py's rules) gives a variable a finite domain when every write of it in the game is a constant, or
itself plus or minus a constant, once call arguments are bound: the start value closed under those writes, up to the
largest constant it is compared with. `FactTable::kind` (table.rs) then makes a key a fact when the scope's code writes
it and it has a domain, a constant (the start's value) when the scope does not write it, and a non-fact otherwise. The
sentence script writes var 103 from its local 1 (the sentence's first object, which the engine binds), so var 103 has
no domain ("a value from an engine-bound local"), the scope writes it, and it is a non-fact: the machine reads it as
an unknown, a test on it becomes a check with both sides read, and its writes are left out of the task.

### Refinement: a variable made a fact (reader switch `--fact-vars`, off by default)

reader-scope `--fact-vars N,...` gives each listed variable var:N that is a non-fact of the reading (the scope writes
it, it is no engine or settings variable, and it has no domain) the domain D_N in the table's `extra`, as
`Builder::extend_domains` gives a widened domain: seeded with the reading's start value of var:N and its value in each
`--reach-also` start, and then widened by the reader's own passes (a value a path writes outside a domain is added
and the reading runs again). A listed variable that is not a non-fact is left as it is and reported as such. The report
says, per variable, what was done, D_N's values, and whether the last pass still wrote a value of var:N outside D_N.
With the switch off nothing changes; reading.json and the run's record say `fact_vars` only when it is on.

### Lemma R (adding a fact refines the reading)

Let A be a reading of scope S from start s₀ with switches Σ, v a non-fact of A, and A′ the reading of S from the same
starts with the same switches and v given D_v as above.

Premise R0: D_v holds every value v has in the engine while S's code runs. What the reading can check of it: s₀'s and
every start's value are seeds, every value a path of A′ writes to v is in D_v at the last pass (the report's
out-of-domain list does not name v), and no write of v is a value the reader cannot work out (a guess over D_v,
counted).

1. **Paths.** A path of A′'s machine runs the same instructions as a path of A. Where A reads v, it gets an unknown;
   a test on it is undecided, so A follows both sides, each as a record with the check (cite, condition, side).
   Where A′ reads v, it forks the path on D_v (per value, or per value set that the comparison reading it separates,
   F12), and in each branch the test is decided by the branch's set. The branches' sets cover D_v. So each record r′ of
   A′ follows the path of a record r of A, with pre(r′) = pre(r) plus `v ∈ S`, checks(r′) = checks(r) less the checks
   on v, and effects(r′) = effects(r) plus v's own writes; an effect A guessed from v's unknown becomes a set or a copy
   of v. A test that does not read v is decided as in A.
2. **Sound and finer.** Given R0, an engine run that a record r of A over-approximates, with v at x at its tests,
   is over-approximated by the record r′ of A′ whose set holds x: the run took at each test on v the side x decides,
   which is the side r′ takes. So A′ over-approximates the engine wherever A did (assumption A1 kept), and each record
   of A′ admits only runs that one record of A admits, those with v in its set: it only splits paths A already had.
3. **Records already played.** Each one the strict search matched is an engine run that matched a record of A on A's
   facts. By 2 that run is a run of a record of A′, so the played prefix stays a run of the refined task. The harness
   keeps the run's inputs and checkpoints (verify plays the inputs from boot, not the records), drops the rest of the
   old plan, and plans again with A′. Nothing played is matched again.
4. **The new fact at the replan.** Its start value is not predicted: it is the engine's value in the dump at the
   replan's frame, read as every fact is at a replan (`facts_of` of the dump, `replay.replan_start`). The refinement
   reads the scope again with the engine's state at that frame as a `--reach-also` start (as the reach extension does),
   so that value is a seed of D_v and the replan's start lies in the refined task's domains and reached values.
5. **The loop ends on outcomes.** Each refinement adds a variable the scope writes and the reading holds no fact of;
   there are finitely many. A cause that names a variable added before means a refinement did not change what the
   reading holds, and the run stops there with that reason. □

*Check of the lemma (before the runs):* the kitchen read with and without `--fact-vars 103` (both cheap). For every
action of A′, an action of A with the same source and the same slot ends whose precondition, on A's facts, holds A′'s,
whose checks hold A′'s other than those on var 103, and whose effects on A's facts hold A′'s (a set equal, a guess no
wider). Counted: actions of A′ with no such action of A, and actions of A whose check on var 103 has no action of A′.
D_103's values and the out-of-domain list.

### The loop (harness switch `--refine-facts`, off by default)

replay_part1.py `run --refine-facts` (needs `--window-strict`, a reading made with `--reach`, and `--reach-extend`).
After a window that ends a plan's play with a replan (a deviation or an empty window: window.py `play_plan`), the
harness reads B1's cause of that replan (`strict.explain`'s values and class):

- **Refine** when the class is split and one or more of the values the test read is a variable whose kind is "not a fact
  of the reading" (trace_explain `reading_kind`). Every such variable is added: the reading's scope is read again
  from its start and every start so far, plus the engine's state now, with the reading's own switches and the fact
  variables so far plus the new ones (`refiner`, as `reach_extender` reads, in run_dir/refine-NN); the harness takes
  the new reading (`Harness._swap`: the packed task, the lists, the tracked facts; the strict search reads the new
  records, window-search.md 9.1), closes the old planner process, and replans from the engine's state at that frame.
- **No refinement** for any other cause: no cause, a class other than split, or a split whose values are positions,
  locals, getters, clocks or a variable outside the reading's code. That cause ends the loop's turn: the harness goes
  on by its own rules (a deviation replans, a second identical empty window stops the run, section 9.5), and the cause
  is reported with its class.
- **Stop** when a cause names a variable the loop added before: "refinement: var N added at plan P is named again".
- Later rereads of the run (the reach extension, F10's) keep the fact variables (reading.json `fact_vars`).

Off, the harness is E006's: no hook runs, and the run's record and log are the same. On, the run's file name ends in
`-refine` and its record has `refinements`: per turn, the plan, the frame, the window's outcome, the cause (short) and
its class, the variables added or why none, and for an addition the reader's wall and CPU seconds and peak RSS, the
records and actions before and after, D_N's values, the out-of-domain check, and the next plan's operators.

### Runs

- Code: branch e007 from origin/e006 (5715940). The bridge: svm-w3b's ScummVM build, read only. The reader built under
  `~/.cache/fast-mi-e007/target-scumm`; taskcore is walk's build, unchanged. Outputs under `~/.cache/fast-mi-e007`.
- Off checks, before any run with a switch on: `reader-port read kitchen --packed` writes sha1 279b7a24; read-rust
  without `--fact-vars` gives the e006 readings' packed sha1 (kitchen 41c31b6d; k = 2 f5ec2354, the reading the k = 2
  test starts from); the kitchen from the boot with `--refine-facts` off gives E006's kitchen log (stop, frames,
  children, outcomes, causes). The reader's and the harness's tests pass, with new ones for the switch and the loop.
- Tests, each from the harness's boot on seed 1 with E006's `run.sh` switches plus `--window-split-wait
  --refine-facts`, each verified from boot in a fresh engine with no fork: k = 2 (state:316 = 1), then the kitchen
  (owner:566 = ego). k = 2 runs under the heavy lock (its rereads peak near 4 GB).
- Reported per run: whether the goal was reached and how (the plan's records, or the engine idling), game time (qf,
  s, frames from 2728), each refinement (variable, cause, records and operators before and after), each replan with
  its cause, the window search's CPU and the rereads' CPU, peak memory (Python RSS, the forks' PSS, the planner, the
  reader). Where a run stops: the next blocker, its cause and class (window-search.md 9.5).
- Seed 1 only. Laptop, nice 10, at most 3 threads; a step stops if MemAvailable falls under 3 GB.

## Prediction (before running)

| | predicted |
|---|---|
| off checks | 279b7a24; kitchen 41c31b6d, k = 2 f5ec2354; the kitchen's run log as E006's |
| lemma check (kitchen, var 103) | 0 actions of A′ without an action of A that holds them; D_103 = start 0 and the sentences' first objects, 25 to 40 values; no value of var 103 outside it after the last pass; records within ±10% of 41,877 |
| k = 2, refinements | 1 (var 103), at plan 0's open-door deviation at about 2796; at most 2 (var 104 next, if a later record takes the [0007] check) |
| k = 2, the reread | D_103 of 77 to 100 values; records and actions within ±10% of 451,987 and 450,967; plan operators within ±15% of 143,417; 120 to 400 s wall, 300 to 700 CPU s, peak 3.5 to 5.5 GB; the replan's start holds var:103 = 428 from the dump |
| k = 2, after it | walk_to(428)'s click record (var 102 = 0) matches part 1, #510 is refuted on M4 (local:201), a deviation at 2798 with no class: no refinement; the next plan is the bar door's, and the goal holds in the idling engine at about 3129 (± 5), written by local 209 (class split on a local: no refinement) |
| k = 2, numbers | goal reached by the engine idling, not from the plan's records; about 401 frames (9,624 qf, 40.1 s); 2 replans (± 1); window search CPU 40 to 150 s; Python under 1 GB, the planner under 1.5 GB |
| kitchen, switch on | no refinement (its causes: a refuted event with no class, and a position); the goal from the plan's records at 3216, 488 frames, 11,712 qf, 2 replans, as E006 |
| verify from boot | 0 differences on both |

## Success threshold

- Yes: with `--refine-facts`, k = 2 from the boot reaches state:316 = 1 (from the plan's records or while the engine
  idles), verified from boot with 0 differences, every refinement made by the harness from a cause it read (no manual
  step); and the kitchen from the boot with the switch still reaches owner:566 = ego from the plan's records, verified
  from boot. The off checks pass.
- Partly: the loop adds at least one fact on its own and k = 2 gets past walk_to(428), then stops at a named blocker
  before state:316 = 1.
- No: k = 2 still stops at walk_to(428), or the loop stops on its guard (a variable named again), or a refined
  reading breaks R0 (a value of the new fact outside its domain after the last pass).

## Stop rule

- An off check that differs: stop, find why, and run nothing with a switch on until it passes.
- The lemma check finding an action of A′ that no action of A holds: stop and write up which, before any run.
- A refined reading whose last pass writes a value of the new fact outside its domain: the run stops there (R0 fails),
  and the write-up names the write.
- The loop's guard (a variable named again) stops the run; a cause that is no split on a variable the reading holds no
  fact of is no refinement, and the harness's own rules go on. Both are reported, not fixed.
- A run that stops further on: name the next blocker (cause and class from 9.5) and stop that test there. E007 makes
  no fix beyond the loop.
- The harness's and the reader's own limits are E006's (replan limit 30, windows and children of 3,600 frames, plan
  memory 3G; the reader's 60-minute and 8 GB watchdog); a run that hits one reports it as its stop. They are not
  decisions of this experiment.

## Amendment (2026-11-01, before the runs, after the lemma check's first reading)

- The first reading of the kitchen with `--fact-vars 103` forked every read of var 103 per value: the sentence script
  compares it with its local 2 (the sentence's second object), not with a constant, so F12's value sets did not apply.
  The records grew from 41,877 to 107,395 (2.56 times); k = 2, with about 80 values, would grow far more. Step 1 of
  Lemma R already says "per value set that the comparison reading it separates": the reader now applies F12's rule to
  a variable `--fact-vars` made a fact when the other operand is a local whose value the path knows (an argument bound
  at the slot's start), which on that path is a constant (machine.rs `compare_sets`, `FactTable::refined`). It touches
  only the listed variables, so every reading without the switch is unchanged. The predictions above stand as written.
- The check compares records, not actions: the packed file folds equal records, so it reads the golden files. A record
  of A that merged two paths with the same effects (a test taken either way) keeps one path's checks; a record of A2
  that refines it lacks those checks on the other path. Such records are counted apart ("refines a merged record").
- A record of A with a check on v that no record of A2 refines must be infeasible: no value of D_v passes its checks on
  v together (A read v twice as two unrelated unknowns). The engine runs no such path, so step 2 holds for it.

## Results

Written after the runs.
