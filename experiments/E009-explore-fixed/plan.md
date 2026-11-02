# E009: The explorer with the fixes E008's data calls for

Date planned: 2026-11-02
Research question it serves: RQ1 (completion from the game files alone, when the scope cannot be read ahead of time)

## Question

With four fixes to E008's explorer, each taken from a measurement of E008's runs, does the explorer reach the kitchen
(owner:566 = 1) and the bar's kitchen door (state:316 = 1) from the boot on the 10 rooms of the k = 2 scope, each
verified from boot? If both are reached, how far into Part I does it get?

## Baseline

E008's explorer (branch e008 at 6d03cac), seed 1, the boot (frame 2728, qf 65,284, room 33), allowed rooms 28, 29, 33,
34, 35, 38, 41, 83, 85, 96, b = 2. Sources: `experiments/E008-go-explore/plan.md` ("Results", "Results of the
resume"), `notes/log/2026-11-02-e008.md`, the archives `~/.cache/fast-mi-e008/archives/{door,kitchen,smoke}`.

| goal | jobs | result | frames from 2728 | CPU s |
|---|---|---|---|---|
| door | 1 | reached, verified, 0 differences | 401 (3129, while the engine idled) | 889.9 |
| kitchen | 154 | not reached, no stop rule fired | best chain 170 (2898, room 38) | 87,206 (read 66,394, plan 19,969, play 499, return 343) |
| kitchen, smoke (rooms 28, 33, 41 only) | 2 | reached, verified | 488 | 967 + the goal job |

E008's four measured reasons, and what the archive's files add (measured for this plan from E008's files; not inputs
of the system):

1. **Keys the idle engine changes count as progress.** 74 of the kitchen's 110 new atoms were values of var 100; var
   20, 44 and others also changed. Measured here: each of 15 entries of the three archives (rooms 28, 33, 38, 41) run
   on 3,200 frames with no input. Every key that changed fell in one of two classes. (i) Keys whose value comes back
   to a value it had before, start included: var 100 at all 15 (first return after 6 to 50 frames), var 0 and var
   108 in rooms 28 and 33, 43 object states in room 28 (state:316, the door, among them: it opens at +331 and closes
   at +419 from entry 4 of the door's archive), three in room 41, var 23. (ii) Keys whose every value is new:
   the end of a walk (room, var 101, 113, 4, 7, 18, 22, 72, 182, 183 within 51 frames of an input), and var 2 and 20
   for at most 19 frames after an input. In the dialog (room 38, entries at 2848 to 2898) only var 100 changes. The
   idle sets at 25, 50, ..., 3,200 frames: at 1,600 frames each entry's set equals its set at 3,200; at 800 one entry
   lacks var 23 (first return 921). Files: `results/baseline-idle.txt` (the per-entry table).
2. **The target order is fixed.** bit:108 = 1 was the first target in 153 of 154 jobs; room = 28, which leads to
   ring(28) ∩ k2 = (28, 33, 41) and the kitchen, is 29th of ring(33)'s 54. In the smoke, the second played target from
   the root (bit:348 = 1, 18th) ended in room 28 and the next job reached the goal (488 frames).
3. **The guide does not narrow** (|R| about 1,162 atoms for every goal). Not changed in E009.
4. **The reading cache.** Of the kitchen's 67 fresh readings of ring(33) (235,579 records, about 312 CPU s each), 65
   found **no cached reading**: /home's free space stayed under E008_EVICT_GB = 10 GB (under 7 GB at 04:48, 8.4 GB
   after the run), so every reading not in use was evicted when the next job started (readings.jsonl: all 181
   registered readings evicted). Only 2 were
   refused by the reuse rule, on var 0, 2, 7, 20, 21, 44, 45, 100, 181, 183. Of ring(38)'s 47 fresh readings, 42 were
   refused, every one on var 100 (with var 229, 230, 183, 108, 103 in some); a ring(38) reading costs 3 to 4 s.

Also from E008: in room 38's dialog every job ended on the harness's R0 stop for var 194 before any input. E009 does
not change the harness.

## Method

E009 changes only E008's files (`explore.py`, `step.py`), each fix behind a switch that `explore.py init` writes into
stage.json (`"e009"`) and the driver passes in job.json. With every switch off the code is E008's (off check below).
No hand-written rule: every key list, length and level below is computed from the game files, from the reader's own
tables, or measured from the engine by code.

### (a) Idle keys, measured from the engine (`--idle-frames L`)

- **Measurement.** Each job, after its return and before its reading, forks the returned engine and runs the fork
  `L` frames with no input, taking the facts at every frame. A key is **idle at the entry** when its value comes back,
  within those frames, to a value it had earlier in them (the entry's value included). The result lists the idle
  keys, each key's first return and its number of changes, the frames and the seconds. An entry is measured once (the
  first job on it); the fork is quit and the job plays on its own forks as in E008, so the measurement changes no
  state the job plays from.
- **Why returns, and why L = 1,600** (baseline 1): the keys that come back are the ones that keep making new values
  with no progress (var 100's 233 values, the room's animations); one-shot changes (a walk's end, a room change) never
  come back and stay progress. Over the 15 measured entries every first return came by frame 921, and 1,600 frames
  (the doubling step from 25 at which all 15 idle sets equal their 3,200-frame sets) cost 8 to 30 s per job measured,
  against minutes per job. Measured at every job's entry because the sets differ by state (1 key in the dialog, 3 in
  room 33, 46 in room 28).
- **The archive's idle set** I is the union of the keys measured idle at any entry, less the stage goal's keys (the
  door's goal cycles while the engine idles in room 28, baseline 1). It only grows. The cell key keeps E008's room and
  owned objects; its guide keys leave out I. The held atoms (favoured entries, #r, new atoms, stagnation) leave out
  the atoms of keys in I. When I grows, every entry's cell and held atoms are computed again from its file (the rows
  in entries.jsonl keep their values at the time they were added), and each cell's n is counted again from log.jsonl
  (a committed job on an entry of the cell: 0 when the job added a new cell or representative, else + 1).
- **Nothing is pruned.** With idle keys left out, more entries share a cell. So with this switch a cell offers for
  selection its least (tick, id) entry that is not exhausted, not only its representative: every entry stays
  selectable until a job finds no target with a plan from it. The cells and atoms then only order the jobs and
  measure progress; no entry is dropped, which needs no further soundness argument (D9).

### (b) Target rotation (`--rotate`)

- The driver counts, from log.jsonl, how many times each target was tried (probed or played) by a committed job: per
  cell (jobs on any entry of the cell, by the current cells) and in all. job.json carries both counts.
- The step orders the targets by (tried in this cell, tried in all, depth, then E008's order: tier, depth, key,
  value). Tier 0 (the goal, depth 0) comes first among the targets tried as often as it. E008's rule that an entry
  never tries a target it tried before stays. So no target is tried again from a cell while another one there is
  untried, and targets tried from other cells come after those tried nowhere.
- It reorders the same targets E008 offers: nothing is removed.

### (c) Readings kept and reused (`--evict-by-size`, `--reuse-nonfacts`)

- **Eviction level from the floor and the sizes** (baseline 4, the measured cause). Readings are evicted, least
  recently used first as in E008, only while /home's free space is under floor + 2 × the largest reading registered
  in the archive (a job can register a fresh reading and a refinement, each up to that size); before any reading the
  level is the floor. The floor (8 GB, no job starts under it) is E008's machine rule, unchanged.
- **Reuse.** E008's rule: a cached reading of the scope is used when the entry's facts differ from its start only on
  the reading's task facts and on slot keys. E009 adds the keys K(reading) computed below, for which a different start
  value provably gives the same reading.
- **The argument.** The reader (Rust, `scumm/reader/src`) sees the start only through the start facts (`scope.start`,
  `table.start`). A key's start value is read at these sites: `FactTable::kind` and `machine.rs` `info` give a
  constant its start value (Kind::Const: `machine.rs:759, 897`, `builder.rs:686, 720`, `args.rs:272`,
  `havoc.rs:111, 632, 1031`, `due.rs:213`, `builder.rs:1946`, `scope.rs:659`); the domain closure seeds a fact's
  domain (`domains.rs:777`, and 940-997 for room, the room variables, the boolean variables, VAR_WALKTO_OBJ, owners,
  states, object places); named engine variables (`table.rs:131` the hook variables, 149 the cursor variables,
  156-177 the room variables, `costs.rs:118-119` VAR_TIMER_NEXT and VAR_CHARINC, `inputs.rs:211` and `builder.rs:1131`
  VAR_VERB_SCRIPT, `builder.rs:1376` VAR_CUTSCENEEXIT_KEY, `scope.rs:462` the hook variables); the input script's
  guards (`builder.rs` settled_jobs: a guard variable with no domain is taken at its start value); and the refinement's
  seeds (`table.rs:372`, a reading made later, not the one reused). A key k = var:N is in K(reading) when all hold:
  1. k is a non-fact of the reading for every start: N is an engine variable (`domains.ENGINE`, which holds the time
     and mouse variables; `kind` returns non-fact for them by number), or the domain analysis gives var:N a reason
     for having no domain from its writes in the code (`domains.build(...).why`, a bad write or the engine; computed
     over all units before any closure, so no start value enters it) and the reading's scope writes var:N
     (`reader.Scope.of(...).writes`, the Python twin of the Rust scope the port tests hold equal), so it is not a
     constant. A counter with no domain only because it is never compared is left out (its closure looks at other
     variables' domains);
  2. N is none of the named variables above (the settings, the hook, cursor, room and boolean variables,
     VAR_WALKTO_OBJ, VAR_VERB_SCRIPT, VAR_CUTSCENEEXIT_KEY, VAR_TIMER_NEXT, VAR_CHARINC);
  3. N is no guard of an input script: no global unit with findObject(VAR_VIRT_MOUSE_X, VAR_VIRT_MOUSE_Y) compares
     var:N with a constant in a jump before that findObject whose target lies past it (`inputs.rs` hover_roles; taken
     over every such unit, a superset of the one hover the reader picks).
  Then no site reads k's start value: a non-fact is read in a path as an unknown (`R_NONFACT`), and the side a path
  took is a replay check that the window search tests against the engine while it plays, not a precondition. Two
  starts that differ only on K, the task facts and slot keys give the same constants, domains and non-facts, so the
  reading is the same, and E008's checks of the task facts (taskcore's domains) still apply. Every other key keeps
  E008's rule. K is computed once per reading by `step.py` from the reading's scope and start and cached next to it.

### (d) Stop rules (`--stop-jobs N`)

- E008's: the first goal entry (then verify); an exhausted archive; a return or verify that differs; a job failing
  twice on the same entry with the same error.
- Stagnation over non-idle atoms: after the last job that added an atom no earlier entry held whose key is not in I,
  as many consecutive committed jobs as there were selectable cells right after it, none adding such an atom.
- The prediction's falsifier: no job starts once N jobs have started, N being twice the upper bound of jobs
  predicted below for the stage's goal; the running ones commit. If the goal is not among them the stage is written
  up as "no".

### Interfaces (additions to E008's, all optional; absent means E008's behaviour)

- `explore.py init ... [--idle-frames L] [--rotate] [--evict-by-size] [--reuse-nonfacts] [--stop-jobs N]`, kept in
  stage.json `"e009"`; `explore.py run` reads them from there.
- job.json: `"idle_frames"` (with `"idle_measured"`: the entry was measured before), `"rotation": {"cell": [[k, v,
  n], ...], "all": [...]}`, `"reuse": "e009"`. result.json: `"idle": {"frames", "returns": [keys], "first_return":
  {k: frame}, "changes": {k: n}, "seconds", "cpu_s"}`, and the reading record's `"reuse_keys"` count.
- log.jsonl lines add `"job_tried"` (this job's own targets), `"idle"` (the keys measured, the keys new to I) and the
  idle set's size; summary.json adds the stop `"prediction bound"`.

### Off check and tests

- With the switches off: E008's tests pass (`test_explore.py`, `test_step.py`, `test_guide.py`), and step.py's engine
  test (a) still gives E007's kitchen run (the goal from the plan's records at 3216, 488 frames, 2 replans).
- Unit tests per fix: the idle rule (returns against one-shot changes, the goal's keys kept), cells and held atoms
  without idle keys and their recount when I grows, the next entry of a cell offered once its first is exhausted; the
  rotation's counts and order; the eviction level; K(reading) on the door's ring(33) start (var 100 and the mouse
  variables in, a counter and a named variable out) and the reuse it allows; the falsifier and the non-idle stagnation
  with the fake step.
- An engine test: the idle measurement from the boot gives var 100 and no other key (baseline: kitchen-0).

### Runs

- Code: branch e009 from origin/e008 (6d03cac). Environment as E008's integration (FAST_MI_SCUMMVM_BIN svm-w3b's
  build, DOWNWARD_DIR, TASKCORE_BIN walk's, READER_SCOPE_BIN e008's). Outputs under `~/.cache/fast-mi-e009/`.
- Stage 1: two new archives from the boot, door then kitchen, allowed rooms the k = 2 scope's 10, b = 2 (E008's 7 GB
  MemAvailable rule for the second job), every switch on, `--idle-frames 1600`, `--stop-jobs` 4 for the door and 80
  for the kitchen; then verify each goal entry from boot.
- Stage 2 (only if stage 1 is yes on both): goal `part1`, allowed rooms the 80 rooms from the files, the same switches,
  `--stop-jobs 600`. Before it, a dated amendment here with the projection from stage 1's measured job costs and the
  laptop-or-VM choice (`scripts/cloud.py`).
- Reported per stage: jobs, entries, cells, the idle set (size, keys, where measured), targets tried (per target, the
  first-try spread), readings (fresh, reused, why refused, K sizes, evictions), refinements, CPU by part, peak memory,
  disk, the goal chain and its verify. For stage 2 also the Part I rooms entered and the E001 key facts held along the
  best verified chain (evaluation only, never an input).

## Prediction (before running)

| | predicted |
|---|---|
| idle set, door and kitchen | var 100 at the root; then var 0 and var 108 from room 33's entries; 40 to 50 keys once an entry in room 28 is measured; never the goal's key |
| door | 1 job (at most 2). One fresh reading of ring(33) (235,579 records ± 1%), one refinement (var 103). Tier 0 first. state:316 = 1 at 3129 ± 10 while the engine idles: 401 frames, 9,624 qf ± 10%. 12 to 25 CPU min. Verify 0 differences |
| kitchen, jobs | 3 to 40 (most likely 10 to 25); the first job as E008's (bit:108 = 1 into room 38's dialog); the goal from a job on a room 28 entry with scope (28, 33, 41) |
| kitchen, targets | no target first in more than a third of the jobs |
| kitchen, readings | ring(33): 1 to 5 fresh, the rest reused; ring(38): 1 to 12 fresh; (28, 33, 41): 1 to 3 fresh; refinements 1 to 5; no ring(33) reading refused only on keys of K |
| kitchen, cost and chain | 0.5 to 6 CPU h; the goal chain 480 to 1,500 frames from 2728; 2 to 6 segments; verify 0 differences |
| both | every return the same as its entry; per job Python under 1.2 GB, reader under 1.5 GB |
| Part I (stage 2, if it runs) | not reached; the stop by stagnation or the falsifier after 150 to 600 jobs; 30 to 120 fresh readings (a hub ring of room 83 or 85 at 200 to 3,000 CPU s each); 15 to 60 CPU h; 8 to 20 of the 31 Part I rooms entered; 15 to 45 of the 99 key facts held along the best verified chain (evaluation only). If reached: 150 to 300 jobs, 7,000 to 20,000 frames from 2728 |

## Success threshold

- Stage 1, yes: both goals reached by the explorer from the boot with every E009 switch on, each goal entry's movie
  verified from boot with 0 differences, every reading's scope a ring the explorer chose, no manual step, each within
  its falsifier bound. Partly: one of the two. No: neither.
- Stage 2, yes: the Part I goal (owner:901, 902, 903 = 1) reached and verified from boot. Partly: not reached, but a
  verified chain holds at least 25 of the 99 key facts (evaluation only) and enters at least 3 rooms outside the k = 2
  scope. No: less.

## Stop rule

- Per stage, the stop rules of (d). A return or verify that differs stops everything until it is explained (Lemma C's
  premises).
- The off check differing from E007's kitchen run in stop, frames or replans: stop and find why before stage 1.
- Stage 2 runs only if stage 1 is yes on both goals, and only after its dated amendment.
- When a stop rule fires: write up the best chain, the idle set, the targets tried, why the jobs ended (no plan, the
  harness's stop and B1's cause, a reading stopped by a watchdog), and stop. E009 adds no further idea.
- The harness's, the window search's and the reader's limits are E006's, and the guard's 3 GB and the disk's 8 GB
  floors are the machine's: engineering watchdogs, reported where they fire.
