# E005: Narrow havoc closures, so the rough reading of Part I can finish

Date planned: 2026-11-01
Research question it serves: RQ1

## Question

E004's rough reading (`--step-limit N`, experiments/E004-fast-exact/rough-reading.md) never finished on Part I. A cut
path becomes a havoc record whose closure is the code the path can still run, and two things made that closure every
unit of the scope:

- (a) a room change in the closure names every scope room's entry and exit code; through room 85's exit that reaches
  global 17 and then global 23;
- (b) two computed script starts do not fold: global 23 starts `Var[396 + Local[0]]` and global 73 starts `Local[1]`.

With these two changes:

- **(A)** a room change in a havoc closure names only the exit code of the room it leaves and the entry code of the
  room it loads, with the set of room numbers its code can load;
- **(B)** a computed script start names only the scripts its callers can pass: from a static scan of the call sites,
  and of the writes to the array it indexes, falling back to every unit only where a caller passes an unknown, and
  reporting where;

does the rough reading of Part I finish, and does lama-first find a plan from it to Part I's end?

## Baseline

E004's rough reading, branch `rough` at babd3f3 (notes/log/2026-11-01-e004-rough.md). Run D's switches
(value_sets, draw_clears, actor_rooms, grow_limit) with havoc_cut and havoc_starts, `--reach --elsewhere
--live-locals --split-limit`, seed 1, goal results/end-part1.json.

| N | extra | end | CPU s | peak RSS | where | records | havoc records | naming every unit |
|---|---|---|---|---|---|---|---|---|
| 100 | | stopped by hand | | 55.8 GB | pass 1, round 2 | 159 | 170,589 | 155 |
| 300 | | stopped by hand | 6,557 | 49.2 GB | pass 1, round 4 | 5,528 | 9,500 | 9,500 |
| 300 | reach_pairs | stopped by hand | 1,513 | 48.9 GB | pass 1, round 5 | 7,147 | 8,832 | 8,832 |
| 500 | | stopped by hand | 5,645 | 4.0 GB | pass 1, round 6 | 25,177 | 79 | 0 |
| 1,000 | | stopped by hand | 22,554 | 6.6 GB | pass 1, round 8 | 67,662 | 88 | 0 |

- A record that names every unit has 2,141 effects and 14,072 events (576 units).
- Every one of them names every unit because of global 73's start (instruction 41) and global 23's start
  (instruction 56). At N = 300 they all come from the five `dialog_line(120..124)[202@85]` jobs, which stand in room
  85's local 202 > global 114 > global 73 at the cut; with reach_pairs, from sentence jobs on two inventory objects.
- At N = 100, 170,421 of the 170,589 havoc records stand in room 58's entry code: one unit each, 70 effects and 359
  events (every wait point of the `entry` slot).
- At N = 500 and 1,000 almost nothing is cut, and the reading costs what the full one costs.
- The bar at N = 100 (rough-reading.md, check 3): 94,833 records, 16,661 havoc records, 2,299 CPU s, 13.5 GB; it
  reaches every value the full reading reaches, and 4,344 more. Its lama-first plan has 6 steps, like the full
  reading's.
- From the game files, room changes reach 80 rooms from seed 1's start, all 31 Part I rooms among them.

## Method

Both changes are havoc_cut switches, off by default: `havoc_rooms` (A) and `havoc_args` (B). Each gets its rule and
proof in this folder before its code, for any SCUMM v5 game: it only removes code no engine run can reach from the
cut point. MI1 numbers appear only in the tests and the notes.

- **A, `havoc_rooms`.** The engine changes the room only in startScene, which `loadRoom`, `loadRoomWithEgo` and
  `actorFollowCamera` (when the actor is in another room) call. It runs the exit code of the current room, kills that
  room's scripts, loads the new room and runs its entry code. So the closure names: the exit code of the room the path
  stands in (each room its precondition allows, or every scope room when it does not say); and for each room change
  in the closure, the entry and exit code of each room it can load. The rooms a change can load: its operand's
  constants; a variable's values (the domain convention havoc_starts already uses); a local's values (B's analysis);
  for `actorFollowCamera`, the rooms the actor can be put in. Anything else: every scope room, counted and reported.
  A pending room step of the cut names the room it loads when the step holds it. Local scripts, drawObject's room and
  the slots a change kills follow the same set of rooms.
- **B, `havoc_args`.** A start whose number is `Local[k]`, `Var[b + Local[k]]` or `Var[b + Var[n]]`:
  - Local[k]'s values at the start come from a flow-sensitive interval and set analysis of its unit: the values its
    launches pass as argument k, the unit's own writes on the way, and the jump conditions on Local[k] on the way
    (`if (Local[0] > 12)` bounds it). Engine results with a known range count as that range (findObject: an object
    number or 0; actorFromPos: an actor number or 0).
  - The launches of a unit: every start in the scope that can name it (a constant, or a computed start's values by
    this same analysis), with its arguments. A unit a hook variable can name, or one a start with unknown values can
    name, has unknown arguments.
  - A variable's values follow the domain convention havoc_starts uses. An index range gives the values of each
    variable in it.
  - Where any of this is unknown, the start names every unit, as today, and the trace says which start.
- **B in the exact reader.** The exact reader does not treat a computed start as every unit: the machine leaves a
  start whose number is unknown out and lists it on the record (`unknown-target writes left out and listed`). So B
  does not change the exact reader. The Part I readings report how often that happens (the counter for
  `startScript`).
- **Tests.** For each switch, a small case where the closure narrows, and the off case byte for byte as before.
- **Off is unchanged.** Every switch off: the kitchen's packed sha1 stays 279b7a24 and the reader's tests pass.
- **Soundness check.** The bar (rooms 33 and 28) and the kitchen, rough at N = 100 with A and B, against their full
  readings: rough_contains.py on `NAME.reached` (rough-reading.md, check 3). Every value the full reading reaches must
  be reached.
- **Part I runs.** `reader-scope --part1 --reach --elsewhere --live-locals --split-limit --switches
  value_sets,draw_clears,actor_rooms,grow_limit,havoc_cut,havoc_starts,reach_pairs,havoc_rooms,havoc_args
  --step-limit N`, seed 1, goal results/end-part1.json, READER_TRACE=1, at N = 300 and 100, and one more N if the
  numbers point to it. Per N: CPU s, wall s, peak memory, records, havoc records and how many name every unit, whether
  it finished. If it finished: rough_plan.py (taskcore, lama-first): plan or not, length, havoc and exact steps, the
  order as rooms and goal facts. For evaluation only, `--key` compares that order with results/groundtruth-part1.jsonl's
  key goals.
- **Machine.** The laptop: 8 cores, 23 GB, shared with another job; nice 10, at most 3 threads; a run expected above
  2 GB peak takes /var/tmp/fast-mi/heavy.lock. A run stops when MemAvailable falls under 3 GB (`--min-free-gb 3`).
  reader-scope's own time cap is set out of the way (`--cap-min 100000`): the runs stop on memory or when they end.
  The cloud VM is used only for a Part I reading the laptop cannot hold: one VM start, through scripts/cloud.py
  (PI's cap $30, ledger $16.13), stopped when the run ends.
- **Data.** Seed 1 only. Not the Any% seed, not seeds 31-35. Branch e005 from origin/rough. Big outputs in
  ~/.cache/fast-mi-e005.

## Prediction (before running)

- **B, from the files.** Global 73's start names 2 scripts: global 58 and the room's local 200 (globals 116 and 114
  pass them). Global 23's start indexes `Var[397..408]`: Local[0] is 1 to 12 there (its guards, and findObject and
  actorFromPos are never negative). No script writes those variables by a constant index, so they hold their start
  value 0 and the start names no unit. No computed start in the Part I scope falls back to every unit.
- **A.** A room change in a Part I closure names the entry and exit code of at most 4 rooms in the median, not 31.
- **N = 300.** 0 havoc records name every unit (baseline: all 9,500). The reading finishes on the laptop, 3 threads:
  under 6 GB peak, under 4,000 CPU s, under 20,000 havoc records.
- **N = 100.** 0 havoc records name every unit (baseline: 155). Room 58's entry code still makes over 100,000 havoc
  records of about 359 events each, A and B do not touch them, and the reading does not fit in the laptop's memory.
- **Soundness.** The bar and the kitchen at N = 100 reach every value their full readings reach. The bar's rough
  reading drops from 13.5 GB to under 4 GB.
- **Plan.** From the N = 300 reading, lama-first finds a plan to Part I's end: at most 15 steps, at least one a havoc
  step. Its order passes at most 10 of the key's Part I goals before the end: a short cut through havoc steps, a
  candidate to check, not a route.

## Success threshold and stop rule

- **Yes:** a Part I rough reading finishes, and lama-first finds a plan from it to Part I's end.
- **Also an answer:** the reading finishes and lama-first proves there is no plan. By Lemma R3 the full model then
  cannot reach Part I's end either from seed 1's start.
- **Stop, and write up the stage that binds:**
  - with A and B on, a havoc record at N = 300 still names every unit: the closure's reasons say which start or step;
  - the N = 300 reading needs more memory than the laptop has (MemAvailable under 3 GB): it runs once on the VM; if
    it needs more than the VM's 125 GB, stop;
  - the soundness check finds a value the full reading reaches and the rough one does not: the switch is wrong; fix it
    and check again, and if it fails again, stop.

## Steps

0. This plan, committed before any code.
1. The rule and proof of each switch (`havoc-rooms.md`, `havoc-args.md`), then the code behind its switch, with its
   tests. Off: the kitchen's sha1 and the tests.
2. The soundness check on the bar and the kitchen.
3. Part I at N = 300, then N = 100, then one more N if the numbers point to it. If one finishes, its plan.
4. The results below and a lab note (notes/log/2026-11-01-e005.md): predicted against measured, and, if the stop rule
   fired, the stage that binds.

## Results

Written after the runs. Nothing above this line changes.
