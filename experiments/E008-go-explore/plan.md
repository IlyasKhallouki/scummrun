# E008: Explore from the real game, an archive of engine states

Date planned: 2026-11-02
Research question it serves: RQ1 (completion from the game files alone, when the scope cannot be read ahead of time)

## Question

Part I's 31 rooms cannot be read ahead of time: E004's exact reading stopped in pass 1, round 5, after 8,001 CPU s,
and E005's rough reading ran out of memory (5.2 GB on the laptop, 113 GB on the VM). Small readings from a concrete
state are cheap (the kitchen in 15 s plus 10 s for the files), and E006-E007 play and verify them from boot.

Can the system reach a goal by exploring from the real game instead? It keeps an archive of exact engine states,
grouped into cells by model facts. It repeats: pick a cell by a fixed order, return to its state exactly, read only the
rooms around it from that state, plan a short step toward a target that a static write map regressed from the goal
ranks, play the step with the strict window search, and add every state the play reaches to the archive. At the end
the inputs of the path to the goal are verified from boot in a fresh engine.

Stage 1 is the k = 2 scope's two goals from the boot, without reading the 10 rooms up front: the bar's kitchen door
(state:316 = 1) and the kitchen (owner:566 = 1, the ego). Stage 2 is the Part I goal.

## Baseline

Main at cb7dbd7, seed 1, the harness's boot start (frame 2728, qf 65,284, room 33), walk's `run.sh` switches with
`--window-split-wait --refine-facts`:

| test | reading read up front | stop | frames (qf, s) from 2728 | replans | window search CPU s | reader CPU s | peak memory |
|---|---|---|---|---|---|---|---|
| k = 2 door | 10 rooms, 451,987 records, 373 CPU s, then a reread of 452,038 records | state:316 = 1 at 3129, while the engine idled | 401 (9,624, 40.10) | 2 | 45.4 | 507.4 (the reread) | Python 1,167 MB, taskcore 1,124 MB, reader 1,217 MB |
| kitchen | 3 rooms (33, 28, 41) chosen by hand, 41,877 records | owner:566 = 1 at 3216, from the plan's records | 488 (11,712, 48.80) | 2 | 263.8 | 0 | Python 291 MB, taskcore 89 MB |

Sources: `experiments/E007-fact-refinement/plan.md` (results), `notes/log/2026-11-01-e007.md`,
`experiments/E006-passed-wait/results/readings.log`.

What else is measured and used below:

- **Part I up front.** E004's exact reading stopped in pass 1, round 5, at 8,001 CPU s; five dialog jobs of room 85's
  local 202 took 3,765 of round 4's 3,785 job seconds (`experiments/E004-fast-exact/plan.md`, results). E005's rough
  reading: out of memory at 5.2 GB (laptop) and 113 GB (VM); the computed `startObject` of globals 9 and 23 names all
  202 objects of the scope (`notes/log/2026-11-01-e005.md`).
- **Return.** No savestate exists; a state is its input movie plus seed 1 and the build. A forked child equals a fresh
  run from boot on every frame (0 differences over 3,000 frames, `notes/log/2026-11-01-e003-a0-fork-exact.md`), and
  every verify from boot of E006-E007's runs, which replays from a file the inputs the harness sent on the socket, has
  0 differences. A boot to 2728 takes 2.1-5.7 s; engines run 500-1,000 frames/s at low load, 150-300 under load.
- **Small readings.** Bar (28, 33): 38,874 records, 14.6 s plus 10.2 s, 575 MB. Kitchen: 41,877, 15.0 s plus 9.4 s,
  594 MB. k = 1 (33, 28, 35, 38) with `--reach`: 51,726 records, 138.3 CPU s, 367 MB, state:316 = 1 reachable
  (`notes/log/2026-10-31-e004-reach.md`). k = 2: 158.8 s plus 136.4 s, 3,948 MB kept, 1,217 MB streamed. On disk a
  reading is 100 MB (kitchen) to 662 MB (k = 2).
- **Rooms from the files.** From seed 1's start the scripts' room changes reach 80 rooms, all 31 Part I rooms among them,
  each within 4 changes (`experiments/E004-fast-exact/results/rough-rooms-from-files.json`, `rooms_from_files.py`).
  A room's code loads 2 rooms in the median; room 83 loads 15 and room 85 12.
- **A quick regression.** E001's baseline (object verb code only, descumm text, a hand-picked seed fact) handed over 241
  facts at depth 14 and held 35 of the 99 Part I key facts, where the dump of every fact a player action touches held
  84 (`notes/log/2026-10-22-e001-step1.md`).
- **Run B's Part I** (the instrument, never an input): 250 moves, frames 45 to 9,539 on its own settings.

## Method

### What is new and what is reused

New, all in this folder: `guide.py` (the static write map and the regression from the goal), `step.py` (one job: return
to an entry, read its rooms, choose targets, play one step, write the states reached) and `explore.py` (the archive,
the cells, the order, the jobs, verify). Reused unchanged: the reader (`read-rust`, `--start-rows`, `--reach`,
`--stream`), the harness (`replay.Harness` with the strict window search, the reach extension and E007's refinement
loop), taskcore and Fast Downward, `replay.boot` and `replay.verify`. E008 edits no existing file. There is no reduction
in E008: the cells, the guide and the scopes only order which (entry, target) pair is tried next; every engine state an
attempt reaches stays in the archive and can be selected (D9).

### Entries

- **An entry** is an engine state the system stopped at: its id, its parent entry, its frame and tick, the input
  events its job sent (the segment), and the checkpoints of that segment (frame, `facts_of` map, `extra_of`: tick,
  random state, actors), the last at the entry's own frame.
- **The root** is the harness's boot: the first idle stop of seed 1 (frame 2728), no events, one checkpoint.
- **Every checkpoint an attempt takes past its start is an entry**: the harness checkpoints at every input, every step
  end and the run's end, and keeps a success-frame checkpoint only when no later input lands at or before it
  (`window.Search.flush`), so each is a state the attempt's inputs reach from boot. A failed attempt's checkpoints are
  entries too (Go-Explore counts what the return path reaches; the research's 8.2).
- **The movie of an entry** is the concatenation of its chain's segments from the root; its checkpoints are the chain's.
- **Nothing is merged and nothing is dropped.** A cell conflates states that need not reach the same things later
  (Go-Explore's own caveat; sim-search surprise 6), so a cell keeps all its entries and only ranks them.

### Return, and Lemma C (a chain's movie gives its states)

A job returns to its entry by booting a fresh engine: the root as the harness boots (`replay_part1.boot`, to the first
idle stop), any other entry by `replay.boot` with the entry's movie (the events at frames up to its frame) run to its
frame. It then compares the engine's state with the entry's checkpoint (`replay.compare_checkpoint`: facts, tick,
random state, actors). This is the replay variant of Go-Explore's return. Restoring is 26 to 260 times cheaper in
frames there, but here a return costs 3-25 s against minutes per job (a boot plus a few thousand frames, the
numbers above), it is exact by the premises below, it survives a restart of the driver, and it lets each job be a
process of its own. Live forks as a cache of returns are an engineering option E008 does not build.

Premises: (G) the engine is deterministic for seed 1 under G1-G3 (`experiments/E003-path-through-cloud/fork-exact.md`):
a run is a function of its input events and their frames, and a stop between frames does not change it (A0); (P) an
event acts the same at its frame whether it was read from SPEEDRUN_INPUTS at boot or sent on the socket before that
frame, which every verify from boot of the harness's runs tests; (S) the bridge refuses an input at or before the
current frame (PATCH:2507), so every input a job sends lands after its entry's frame.

Lemma C. For an entry e with chain e₀, …, e_k = e, a fresh boot that schedules e's movie is, at every checkpoint frame
of the chain, in the state the jobs saw there.

1. k = 0: the root's return boots as the harness did.
2. Step: the job that made e_{i+1} returned to e_i (the state at f_i is e_i's, by the hypothesis, and its return check
   says so), then sent its inputs at frames after f_i (S). e_{i+1}'s movie is e_i's movie plus that job's events at
   frames up to f_{i+1}: the same events at the same frames (P). An event after f_{i+1} acts after f_{i+1} (G). So the
   two runs agree up to f_{i+1} and at each checkpoint on the way (G). □

Every return checks the lemma at one frame, and the final verify at every checkpoint of the goal's chain.

### Cells

- **The key** of an entry is (room, the objects the ego owns, the values of the guide's keys). The objects owned are
  those whose `owner:O` equals the dump's `ego`; the guide's keys are the keys of the atoms the regression from the
  goal reaches (below), of kinds var, bit, owner, state and class. No positions, clocks, slots or non-facts.
- **Why.** Go-Explore's domain-knowledge cells found 255 rooms of Pitfall where downscaled frames found 22, and its
  Nature paper keeps "only features relevant for exploration"; here the features are found by code, as SGFuzz finds its
  state variables, and the guide's keys are the goal's ancestors in the static write map, Fast Downward's relevance rule.
- **The representative** of a cell is its entry with the least (tick, id): Go-Explore's best trajectory per cell, with
  game time as the measure. The others stay.

### The guide: a static write map regressed from the goal

Computed once per goal, from the game files only (`model.Game`, `trace_explain.Code`), as HSPr computes its costs once.

- **Atoms**: (key, value), or (key, any) where the value is not a constant. Keys: `var:N`, `bit:N`, `owner:O`,
  `state:O`, `class:O:C`, `room`.
- **Writers** W(k, v): the units with a direct write site of k whose value is v or not a constant
  (`Code.write_sites(..., direct=True)`); for `state:O` also every unit of O's room with a `drawObject`, which clears
  the objects drawn at the same place (scope.rs's rule, script_v5.cpp:1122-1126; on k = 2 the goal's state:316 was
  written by local 209's drawObject at [0033]); for `room = R`, the units whose code loads R by
  `rooms_from_files.py`'s rules (constant `loadRoom` and `loadRoomWithEgo`, a variable's assigned constants, a
  global's constant argument, `actorFollowCamera` through `putActorInRoom`).
- **Needs** N(u, i) of a write site i of unit u:
  - the tests on the way: every conditional jump of u from which i is reachable (`Code.reachable`, across yields), and
    every fact its condition reads (a variable, a bit, an object's state, owner or class through a getter with a
    constant object, the engine table's room variable as `room`), as (k, c) for each constant c the condition compares
    it with, else (k, any);
  - how u starts: `room = r` for room r's entry, exit and local code; for an object's verb code, `room` = the object's
    room and (`owner:O`, any); for a global script, the needs of each unit that starts it by a constant number, at that
    start site (memoised, cycles cut). A hook's or the boot's script has no start needs.
- **Regression**: the goal's atoms have depth 0; an atom in N(u, i) for a writer u of an atom of depth d, at a site i
  that writes it, has depth d + 1 unless it has one already (breadth first). R is the set of atoms reached, d(a) the
  depth, and the guide's keys are R's keys.
- **Computed starts and indexed writes** are not followed in the map E008 uses. A second map that follows them to every
  candidate (B2's rule) is computed for the measurement only (|R| both ways), because E005's closures through globals 9
  and 23 named every object (the research's section 6 risk).
- **Room ring**: ring(R) = {R} plus the rooms R's code loads (`rooms_from_files.py`'s edges), cut to the stage's allowed
  rooms.
- It is guidance only: it orders, it removes nothing, so it needs no soundness proof (RIFO, AFLGo's static distance).
  E001's key may score R's recall, for evaluation only; it never feeds the system.

### Selection

Over the representatives that are not busy (a job runs on them), not exhausted (below) and in an allowed room, in this
order, smallest first:

1. **#g**: the number of goal keys whose value is not the goal's (BFWS's #g).
2. **not favoured**: a representative is favoured when it is the least-(tick, id) entry holding some atom of R (AFL's
   culling: the cheapest witness per feature; BFWS's novelty 1). An entry holds (k, v) when its value of k is v and
   (k, v) is in R, or (k, any) is in R and v differs from the root's value of k.
3. **n**: how many times the cell was selected since its jobs last produced a new cell or a new representative
   (Go-Explore 2019's "times chosen since new"; MC!Q*BERT's backtracking when a branch stops producing).
4. **−#r**: the number of distinct atoms of R held at some checkpoint of the representative's chain (BFWS(R)'s #r).
5. **tick**, then **id**.

The order has no tuned constant and gives no entry probability zero. With b jobs at a time, the first b
representatives are taken, each of another cell. When a job ends, n of its cell is set to 0 if the job's entries
include a new cell or a new representative of any cell, else n goes up by 1.

### One job (step.py)

1. **Return** to the entry and check it (above). A difference stops the explorer (stop rule).
2. **Scope**: ring(the entry's room).
3. **Reading.** A cached reading of the same scope, newest first, is used when the entry's facts differ from that
   reading's start facts only on the reading's task facts and on slot keys (`at:`, `script:`, `local:`, `objscript:`,
   `esc:`, which the start maps to `pos:` and `escpos:`). Then the reading's constants and non-facts have the same
   values at the entry as at the reading's start, and the entry's task facts are checked by taskcore against the
   domains and the reached values. Otherwise, or when the harness's first plan says "out of domain", the scope is read
   from the entry's own state: `read-rust SCOPE --start-rows ROW --seed 1` with read.sh's switches (`--elsewhere
   --live-locals --split-limit --switches value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost --reach
   --frame-costs`), `--stream`, and `--goal` the stage goal. ROW is the entry's start row
   (start-part1.jsonl's format, `facts_of` of the returned engine's dump).
4. **Targets** from the reading's reached values (`reading.reached`) and the guide:
   - tier 0: the stage goal, when every goal key's goal value is reached;
   - tier 1: atoms of R (or values of a key with (k, any) in R) that the reading reaches, that the entry does not hold,
     and that no entry of the archive holds;
   - tier 2: the same, held by some other entry (the step may get there faster, or with another inventory);
   - within a tier, by (depth, key, value); targets this representative tried before are left out.
5. **Attempts.** For each target in order: a fork of the returned engine, a `replay.Harness` with the target as its
   goal (taskcore `--goal K=V`) and cmd_run's wiring with walk's `run.sh` switches plus `--window-split-wait
   --refine-facts` (strict window search, 3 workers, fork-each 7, cache, lite, serve, lama-first-costs, cost caps,
   reach extension, unobserved actor rooms, approach stones). If its first plan is not a plan, the target is tried with
   no plan and the next one is planned. The first target with a plan is played to the harness's own end (the target,
   or a stop by its rules and limits), and the job ends there: one played step per job (the Nature paper's
   goal-directed exploration found about 4 times the cells of random actions; Hoffmann's landmark control hands the
   planner one subgoal at a time; SIW serialises goals).
6. **Out**: every checkpoint of every attempt past the entry's frame, as entry files; the targets tried; whether the
   job found no target with a plan (then the representative is **exhausted**); the costs.

Readings the harness makes during the attempt (the reach extension's, the refinement's) are registered as readings of
the scope too: each holds the earlier reading's starts and fact variables (reach.md Theorem 1, Lemma R). Readings are
deleted least recently used first when /home's free space falls under 10 GB; no job starts under 8 GB.

### Verify

When an entry holds the stage goal, the explorer stops starting jobs, waits for the running ones, and plays the goal
entry's movie from boot in a fresh engine with no fork (`replay.verify`, `replay_part1.config(1, ...)`), comparing
every checkpoint of its chain. No prefix is spliced: the movie is exactly the chain's segments.

### Parallelism and the machine

- One driver owns the archive (single writer) and runs up to b jobs as processes (`step.py run JOB_DIR`), each with its
  own engines, reader and taskcore. Go-Explore explores batches of cells in parallel; the 3D-game paper ran 16 agents
  on one machine.
- A job: the reader on 3 threads (0.3-1.2 GB streamed), taskcore (0.1-1.1 GB), Python (0.3-1.2 GB), the window
  search's forks (70-80 MB), from E006-E007's measurements. On the laptop b = 2 while MemAvailable is at least 7 GB when
  a job starts, else 1. Each job runs under the guard (it kills its group under 3 GB MemAvailable), at nice 10. A job
  whose scope has 5 rooms or more runs under `/var/tmp/fast-mi/heavy.lock`. A job the guard killed is not counted as
  tried and is run again later.
- The VM (`scripts/cloud.py`, one start) only for stage 2, and only when the laptop's measured rate projects a wall
  time at least four times the VM's at b = 5 (16 threads); the projection is written here, dated, before that run.

### Left out of E008

- Engine-only expansion (IW(1) over the reading's applicable inputs) when no target has a plan, and the window
  search's failed children as entries: both need new code in window.py; an exhausted archive is reported, not repaired.
- Live forks as an archive: returns replay from boot (above).
- Dead-end cuts by a relaxed forward closure over the map (the one proven prune the research names): not built, so
  nothing is removed.
- A disjunction of targets in one plan (one goal operator per disjunct): taskcore takes conjunctions; one target per
  plan, tried in order, gives the same ranking without a taskcore change.

### Interfaces (fixed before the code)

All paths absolute. JSON files are UTF-8; `.json.gz` are gzipped JSON. Each component reads the others' files by these
formats; no component imports another component's module.

**GUIDE.json** (`guide.py map`):

    {"format": "e008-guide-1",
     "goal": {"state:316": [1]},
     "map": "direct",
     "rooms": {"start": 33, "edges": {"33": [28, 35, 38], ...}, "reached": [1, 10, ...]},
     "atoms": [["state:316", 1, 0], ["room", 28, 1], ["var:103", null, 2], ...],
     "keys": {"state:316": 0, "room": 1, "var:103": 2, ...},
     "writers": {"state:316=1": ["local:28:205", "local:28:209"], ...},
     "stats": {"atoms": 0, "keys": 0, "depth_hist": {"0": 1}, "units": 0, "seconds": 0.0,
               "no_writer": [["bit:6", 1], ...], "wide": {"atoms": 0, "keys": 0}}}

`atoms` is R as [key, value or null (any), depth], sorted by (depth, key, value). `keys` is each key's least depth.
Unit names: `global:N`, `local:R:N`, `entry:R`, `exit:R`, `obj:O`. Two rules every reader of the file applies:
depth(k, v) = the least of the depths of [k, v] and [k, null] (none when neither is in R); ring(R, allowed) =
sorted({R} ∪ edges[R]) ∩ allowed.

**The archive** (`explore.py`, its only writer), ARCHIVE/:

- `stage.json`: {"stage", "seed": 1, "goal": {k: [v]}, "guide": path, "allowed_rooms": [...], "workers", "created"}.
- `entries.jsonl`, one line per entry: {"id", "parent" (null for the root), "frame", "tick", "room", "cell" (sha1 of
  the canonical key), "g" (#g), "held" ([[k, v], ...] the R atoms it holds), "r_path" (#r), "job", "how" ("boot",
  "checkpoint" or "end"), "goal" (bool)}.
- `entries/ID.json.gz`: {"id", "parent", "frame", "tick", "events": [the segment's events, frames after the parent's
  frame, up to this frame], "checkpoints": [[frame, facts, extra], ...] (the segment's, the last at this frame)}.
- `cells.json` (rewritten after each job): {cell: {"key", "rep", "entries": [...], "n", "exhausted": [rep ids],
  "tried": {rep id: [[k, v], ...]}}}.
- `readings.jsonl`: {"scope": [...], "path" (reading.json), "start_entry", "job", "bytes", "last_used"}.
- `jobs/JOB/`: `job.json`, `result.json`, `new/`, `run/` (the job's reader and harness directories), `log`.
- `log.jsonl`: per job, its selection key, timings and outcome; `summary.json` at the end.

**job.json** (explore → step):

    {"job": "j00042", "seed": 1, "stage": "kitchen", "goal": {"owner:566": [1]}, "guide": "/abs/GUIDE.json",
     "entry": {"id": 17, "root": false, "frame": 3129, "events": [the full movie from boot],
               "checkpoint": [3129, {facts}, {extra}]},
     "scope": [28, 33, 41], "tried": [["room", 41], ...], "held": [["state:316", 1], ...],
     "readings": [{"path": "/abs/.../reading.json", "start_entry": 3}, ...],
     "reading_dir": "/abs/ARCHIVE/readings/28-33-41-j00042", "threads": 3, "workers": 3}

**result.json** (step → explore), next to `new/K.json.gz` files in the entry file format with "id": null and "parent"
the job's entry:

    {"job", "entry", "ok": true, "error": null,
     "return": {"same": true, "differ": {}, "boot_s": 0.0},
     "reading": {"path", "fresh": true, "why": "no cached reading" | "differs on N keys" | "out of domain", "scope",
                 "records", "actions", "read_wall_s", "read_cpu_s", "peak_rss_mb", "files_s"},
     "targets": {"by_tier": {"0": 0, "1": 0, "2": 0}, "first": [[k, v, tier, depth], ...]},
     "probes": [{"target": [k, v], "status": "plan" | "unsolvable" | ..., "seconds": 0.0}],
     "attempt": null | {"target": [k, v], "stop", "reached": true, "goal_from_the_plan": true, "frames", "qf0",
                        "qf_end", "replans", "refinements", "extensions", "window_summary", "seconds"},
     "readings_made": [{"path", "scope", "kind": "reach" | "refine"}],
     "new": [{"file": "new/0.json.gz", "frame", "tick", "how": "checkpoint" | "end"}],
     "tried": [[k, v], ...], "exhausted": false,
     "cpu_s": {"return", "read", "plan", "play", "total"}, "peak_rss_mb": {"python", "children"}, "wall_s"}

**CLIs** (from this folder, with `../../.venv/bin/python`):

- `guide.py map --goal K=V[,K=V] | part1 --out GUIDE.json [--wide]`; `guide.py stats GUIDE.json [--key KEY.json]`
  (R's recall of the key's facts, evaluation only).
- `step.py run JOB_DIR`.
- `explore.py init ARCHIVE --stage NAME --goal K=V[,K=V] | part1 --guide GUIDE.json --allowed k2 | files | R,R,...`
  (boots the root); `explore.py run ARCHIVE --workers B` (resumes from the archive's files); `explore.py verify
  ARCHIVE [--entry ID]`; `explore.py report ARCHIVE`.
- Environment: FAST_MI_SCUMMVM_BIN (svm-w3b's build), DOWNWARD_DIR, TASKCORE_BIN (walk's build), READER_SCOPE_BIN
  (main's reader built under `~/.cache/fast-mi-e008/target-scumm`). Outputs under `~/.cache/fast-mi-e008/`.

### Runs

- Code: branch e008 from origin/main (cb7dbd7); each component on its own branch from e008, merged into e008. The
  bridge: svm-w3b's ScummVM build, read only. Seed 1 only.
- Off check: E008 edits no existing file, so main's tests pass unchanged (test_replay_part1, test_window, test_strict,
  test_refine, test_reach_extend). The first job from the root on the kitchen's scope (28, 33, 41) with the target
  owner:566 = 1 must give E007's kitchen run (the goal from the plan's records at 3216, 488 frames, 2 replans).
- Step 0, before stage 1: the guide's numbers for the three goals (|R|, the depth histogram, the writers of the goals,
  |R| of the wide map, recall of the 99 Part I key facts for evaluation), written here as a dated amendment.
- Stage 1: the door, then the kitchen, each a new archive from the boot, allowed rooms the k = 2 reading's 10 rooms
  (28, 29, 33, 34, 35, 38, 41, 83, 85, 96), b = 2, until a goal entry or a stop rule; then verify.
- Stage 2 (only when the stop rule allows it): goal `part1` (`results/end-part1.json`, the disclosed human input of
  E003-E005), allowed rooms the 80 rooms from the files.
- Reported per stage: jobs, entries, cells, readings (fresh and reused, scopes, records, CPU, wall, peak), targets by
  tier and how many had a plan, attempts played, refinements, the goal entry's chain (segments, frames, qf from the
  verify engine, inputs), how the goal held (the plan's records or the engine idling), verify, CPU per part (return,
  read, plan, play), peak memory (per job: Python, reader, taskcore; the driver), disk. For stage 2 also the Part I
  rooms entered and the key facts held along the best chain (evaluation only).

## Prediction (before running)

| | predicted |
|---|---|
| guide, door (state:316 = 1) | the writers include room 28's locals 205 and 209; room = 28 at depth 1; \|R\| 20 to 300 atoms, depth at most 8; under 60 s |
| guide, kitchen (owner:566 = 1) | room = 41 at depth 1; \|R\| 30 to 400 atoms |
| guide, Part I | \|R\| 300 to 3,000 atoms over 150 to 1,500 keys, depth 10 to 30; the wide map 2 to 10 times larger; recall of the 99 key facts' keys 50 to 90 (E001: 35); under 5 min |
| off check (kitchen job from the root) | E007's kitchen run: the goal from the plan's records at 3216, 488 frames, 2 replans |
| stage 1, door | 1 job (at most 3). One reading, fresh, of ring(33) = 28, 33, 35, 38 from the boot's state: 40,000 to 70,000 records, 60 to 300 CPU s, peak under 1.5 GB. Target tier 0 (the goal). 0 or 1 refinement (var 103). state:316 = 1 at 3129 ± 10 (401 frames, 9,624 qf ± 10%) while the engine idles, written by local 209, as E006-E007. 4 to 15 entries, 3 to 12 cells. 3 to 15 CPU min in all |
| stage 1, kitchen | 2 jobs (at most 6). Readings: ring(33) as for the door, then the ring of room 28 or 41 cut to k = 2 (2 or 3 rooms; 20,000 to 50,000 records, 20 to 120 CPU s). The first job targets a tier 1 atom (room = 41 if the ring of 33 reaches it), the second the goal. owner:566 = 1 from the plan's records, 480 to 800 frames from 2728 (one plan: 488). 8 to 40 entries, 6 to 30 cells. 10 to 40 CPU min |
| stage 1, both | every return the same as its entry; verify from boot 0 differences; per job Python under 1.2 GB, taskcore under 1.2 GB, reader under 1.5 GB; the readings' disk under 2 GB |
| stage 2, Part I | 100 to 600 jobs before the goal or a stop rule; 40 to 200 fresh readings, 20 to 60% of jobs on a reused reading; 300 to 5,000 cells; per job 1 to 6 CPU min. The hubs: the ring of 83 has 16 rooms and the ring of 85 13; a reading from inside one of them takes 200 to 3,000 CPU s, and at least one is stopped by the reader's watchdogs (room 85's local 202, E004's binding jobs). Rooms entered: 20 to 31 of the 31. The goal not reached: the stop rule fires at a named blocker, with 30 to 80 of the 99 key facts held along the best verified chain (evaluation only). If it is reached: 7,000 to 20,000 frames from 2728 |

## Success threshold

- Stage 1, yes: both goals reached by the explorer from the boot, each goal entry's movie verified from boot with 0
  differences, every reading's scope a ring the explorer chose from the files (no reading of the 10 rooms up front),
  and no manual step. Partly: one of the two. No: neither.
- Stage 2, yes: the Part I goal (owner:901, owner:902 and owner:903 = 1) reached and verified from boot. Partly: not
  reached, but a verified chain holds at least 25 of the 99 Part I key facts (the kitchen's chain holds about 8;
  evaluation only) and enters at least 3 rooms outside the k = 2 scope. No: less.

## Stop rule

- A return whose state differs from its entry's checkpoint, or a verify with a difference: stop the explorer and find
  why (Lemma C's premises; A0's rule). Nothing runs until it is explained.
- The guide for the door lacking room 28's locals 205 and 209 among the goal's writers: stop and fix the map before
  stage 1.
- The off check differing from E007's kitchen run in stop, frames or replans (the twin choice of window 0 aside,
  Lemma 5): stop and find why before stage 1.
- A stage's run ends at its first goal entry (then verify), or when the archive is exhausted (no selectable
  representative with an untried target), or on stagnation: after the last new R atom, as many consecutive jobs as
  there were selectable cells at that moment have added no R atom no earlier entry held (one sweep of that frontier
  with no progress). A job that fails twice on the same entry with the same error stops the run, with that error.
- Stage 2 runs only if stage 1 is yes. Before it, the projection from stage 1's measured job costs and one job from an
  entry inside room 83 and one inside room 85 (if stage 2's archive reaches them first, they are its first jobs there)
  are written here as a dated amendment, with the laptop-or-VM choice. A hub reading that the reader's watchdogs stop
  is not a stop by itself: its cell stays unexpanded, and the write-up names it.
- When a stop rule fires: write up the best chain, the cells left with untried targets and why their jobs ended (no
  plan, the harness's stop and B1's cause, a reading stopped by a watchdog), and stop. E008 tries no further idea (the
  engine-only expansion and the others under "Left out" are not added in E008).
- The harness's, the window search's and the reader's limits are E006's (replan limit 30, windows and children of 3,600
  frames, plan memory 3G; the reader's 60-minute and 8 GB watchdogs), and the guard's 3 GB and the disk's 8 GB floors are
  the machine's: engineering watchdogs, reported where they fire, not decisions of this experiment.

## Sources

- Ecoffet, Huizinga, Lehman, Stanley, Clune, "Go-Explore: a New Approach for Hard-Exploration Problems" (2019,
  arXiv 1901.10995: sections 2.1.5, 3.2, A.5, Table 3) and "First return, then explore" (Nature 590:580-586, 2021,
  arXiv 2004.12919).
- Ammanabrolu, Tien, Hausknecht, Riedl, "How to Avoid Being Eaten by a Grue" (2020, arXiv 2006.07409); Hausknecht et al.,
  "Interactive Fiction Games: A Colossal Adventure" (AAAI 2020, arXiv 1909.05398); Lu, Georgescu, Verwey, "Go-Explore
  Complex 3D Game Environments for Automated Reachability Testing" (2022, arXiv 2209.00570).
- Lipovetzky, Geffner, "Width and Serialization of Classical Planning Problems" (ECAI 2012) and "Best-First Width
  Search" (AAAI 2017); Francès, Ramírez, Lipovetzky, Geffner, "Purely Declarative Action Descriptions are Overrated"
  (IJCAI 2017).
- Hoffmann, Porteous, Sebastia, "Ordered Landmarks in Planning" (JAIR 22, 2004); Helmert, "The Fast Downward Planning
  System" (JAIR 26, 2006); Bonet, Geffner, "Planning as heuristic search" (AIJ 129, 2001); Nebel, Dimopoulos, Koehler,
  "Ignoring irrelevant facts and operators in plan generation" (ECP 1997).
- AFL's technical details ("culling the corpus"); Böhme et al., "Directed Greybox Fuzzing" (CCS 2017); Ba et al.,
  "Stateful Greybox Fuzzing" (USENIX Security 2022).
