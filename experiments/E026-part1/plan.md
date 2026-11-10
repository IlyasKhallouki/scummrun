# E026: Monkey Island Part I on the laptop: fix the measured Monkey Island gaps for both games, then play Part I from boot within four hours

Date planned: 2026-11-10
Research questions it serves: RQ1 (completion from the game files alone), RQ4 (generality)
Design: 3 (scale-first), chosen by the judges' scores (23 against 18.5 for design 2 and 16 for design 1, summed over the evidence, generality-cost and user-goal judges), with grafts from the other two designs. Every false claim the judges found is corrected below.
Base: branch `e016` at 3cb3b33 (worktree `/var/tmp/fast-mi/wt-e016`). E026 branches from it as `e026`. Code paths are under `experiments/E011-speed/rs/`, reader paths under `experiments/E003-path-through-cloud/scumm/reader/src/`, with line numbers at 3cb3b33. Scripts are cited under `data/scripts-dos-cd/` by numbers only. Today's measurements are under `~/.cache/fast-mi-e026/`: `mi-checks-now/` (the six checks on 3cb3b33, E024's ALLON, engine svm-e024, fmi sha1 0b9bcdd1…), `size-cost/`, `e016-survey/`, `mi-history/`, `plan/`.

## Step 0: no merge first

- Monkey Island's checks fail on e016's head: b3, smoke's no-bound rule, G4 and K1 (`mi-checks-now/checks/summary.txt`, `g4/g4.json`, `k1/out/run.log`).
- So nothing from E016 on can go to main yet (rule (2): E013 plan.md:828-829; E016 plan.md:784-785). The fixes come first.
- When G13 passes, e016 plus E026 is a merge candidate. The merge is the PI's decision.
- The demo work stops here. E025 (walking) stays paused.

## Question

Can the system play Monkey Island Part I from boot to its end (`owner:901 = owner:902 = owner:903 = 1`, `E003 results/end-part1.json`) on this laptop (8 cores, 23 GB) within 4 hours, with one code path for both games, every switch on and no game numbers in system code? The measured Monkey Island cost terms and gaps are fixed first, for both games. Then: how far does a fresh run get, verified from boot, and what does each stall need?

Hypotheses:
- **H1 (cost per try).** Three measured terms make a Monkey Island try slow, and all three are general:
  - wait_both's passing fork at every WaitFor op no fact decides;
  - a stopped reading whose workers run on;
  - the free tries' trial waits.

  With them fixed, K1's mean try falls from 362.1 s to 30 s or less, and the driver stays under 6 GB.
- **H2 (realize).** Two realize gaps lie on Part I's way: the deferred sentence (G4's miss) and an inventory larger than its 8 visible slots. Run B holds more than 8 objects at 188 of its 250 moves.
- **H3 (progress).** The guide orders nothing on Part I. Two errors cause part of it: global 1's boot-parameter branches count as writers, and indexed writes are missed. Landmarks derived from the scripts, with values, start arguments and counters, are sound on run B's states and on the demo's route. Selection by held landmarks orders the system's own rung entries.
- **H4 (fit).** After H1-H3, K1 reaches the kitchen from boot within 17 minutes. Then 14 milestones at that rate fit 4 hours, and the run is worth making.

## Problem (what e016 shows on Monkey Island today)

### The checks on 3cb3b33

| Check | Threshold | Now | Measured cause |
|---|---|---|---|
| kitchen | E012's goal chain (8 entries) verifies from boot | pass: 0 differences, `owner:566 = 1`, 13 s | – |
| b3 | stop class equal to E013's G-on in 15 of 15; no reading at the bound (E014 plan.md:559) | **fail**: stop class 7 of 15, every field 0 of 15; 7 tries bounded; 116 new entries verify; 1,702.6 s (E016: 1,322.8 s) | the bound: wait_both. The 8 class changes: `FMI_REFINE_WIDEN=3` (E022 env.sh:7), by reading the stop texts only |
| smoke | entries verify; stall report passes; no reading at the bound | 112 entries verify, 5 PASS; **j00001's scope [28] reading at the 300 s bound**; 677.2 s (E016: 237.5 s) | wait_both, inferred from b3's probe |
| G4 | 14 of 14 along the chain, goal reached (E016 plan.md:114) | **fail**: 12 of 14 ("no sentence" ×2), stops at frame 19,006 in room 28 | the deferred sentence |
| K1 | the kitchen from boot within 60 min | **fail**: guard kill at 2,284.9 s; 18 tries, 350 entries, no goal | below |
| G3 (demo) | ≤ 15 candidates | pass, 10 | – |

- **b3's 7 "equal" stop classes show nothing.** They are exactly the 7 bounded tries (j00000, 06, 08, 09, 11, 12, 14), and both runs give them the empty class "" (`checks/b3f/vs-e013-all.txt`).
- **b3's reference is frozen** at E013's 22-switch run. Any later switch meant to change a try's stop cannot pass it.
- **E016's own b3** matched every field in only 8 of 15 tries, unexplained (`/var/tmp/fast-mi/e016-mi-check.out:2`).

### K1, try by try (`mi-checks-now/k1/archive/log.jsonl`, `jobs/*/result.json`)

| Term | Tries | Wall | Share of 6,518.4 s |
|---|---|---|---|
| Room-28 model tries at the 300 s bound | 6 (j00001, 10, 12, 13, 14, 16; j00010 at 0.0 s) | 2,659.3 s: 1,500.2 s at the bound plus 1,159.1 s past it (j00013 +151.4, j00014 +413.9, j00016 +593.8) | 40.8% |
| Free tries | 6 | 3,731.7 s (113.6-841.3 s each), against 999.3 s of their own CPU | 57.2% |
| Room-33 model tries | 6 | 127.4 s (3.4-33.8 s each); 4 end on "the same empty window 2 times on sentence(3,428)", 2 on "no plan" | 2.0% |

- **Memory.** The driver runs every job in its own process. For such a job the log's `rusage.maxrss_mb` is the driver's own high-water mark (`fmi-explore/src/driver.rs:312-313`; `machine.rs:181-188`, RUSAGE_SELF).
  - It was 2,885 MB when j00019 ended.
  - It was 8,373 MB at the kill. The three killed jobs (j00018, j00020, j00021) were all room-28 model tries.
  - So the driver itself took the memory, during the room-28 readings. The guard (`guard.sh:13`, 3,145,728 kB) fired at MemAvailable 3,076,284 kB.
- **Now:** MemAvailable 11,107 MB; swap 8,190 of 8,191 MB used (`free -m`, 2026-11-10). Other programs hold about 12 GB.
- ALLON sets `FMI_READ_MAX_RSS_GB=40` (E020 env.sh:14), above the laptop's RAM. The reader's default is 8 (reader `run/mod.rs:706-709`).
- The C7-C9 runs also used `FMI_REACH_TRY_LIMIT=1` "for memory" (E022 plan.md:483-484, :526). ALLON lacks it.

### The reading at the bound

j00006 (b3, scope [28,33,35,38]), read cold and alone (`mi-checks-now/probe/summary.txt`):

| Setting | Reading | Records | RSS |
|---|---|---|---|
| ALLON as is | stopped at the 300 s bound | – | – |
| probe line B | 75.3 s | 269,081 | 1,505 MB |
| probe line C | 75.0 s | 228,924 | 1,147 MB |
| every reader switch added after E016 off | 36.1 s | 235,618 | 1,080 MB |

- Lines B and C are "`FMI_READ_SWITCHES` unset (no wait_both)" and "E022's (24)-(27) reader group off". `probe.sh` and `p/*/` do not record which override gave which line, so neither label is verified.
- E013 read this scope in 22.2 s, E016 in 78.2 s.
- **The mechanism.** `wait_both` (machine.rs:2406-2441, called from :1436) forks a passing path at every WaitFor op that no fact decides (the `None` arm, :2423-2440). E004 measured its cost: up to k + 1 records through k WaitFor ops in a row (wait-both.md:62-65).
- **Why E022 needs it.** In the demo's dark room, `FMI_READ_QUEUE` holds a sentence. The object re-issues it just before the sentence script's `WaitForActor(VAR_EGO)`. The passing branch ends the script in the same record, and the held sentence runs there (E022 plan.md:590-593). The held sentence is `sx(p).queued` (machine.rs:2302-2322).
- **The bound does not stop the reading.** Workers look at `shutdown` only between tasks (reader `par.rs:550-572`). Only the coordinator reads the abort flag (:314-317). `fmi-explore/src/bound.rs:198-200` says so. K1's log ends with par.rs:317's panic "the reading was stopped".
  - A bounded b3 try burned 819.9 s of CPU in 300 s of wall (`checks/b3f/times.jsonl`).
  - Bounded b3 tries left the process at 983-2,567 MB, against 857-1,254 MB for the others (same file).

### Free tries

- Free tries offer at the entry and at each wait stop, then run one more level from every kept outcome, with no budget (`fmi-explore/src/free.rs:526-602`).
- **K1's j00007** (`jobs/j00007/result.json` .free): 841.3 s; 2,912 trials, 2,778 waits; 57 kept (1 at level 1, 47 at level 2, 9 by delays).
  - One level-2 point: 274 trials, horizon 1,910 frames, read phase 84.9 s.
  - At level-2 points, a trial took 239-340 ms of wall against 68-87 ms of the driver's CPU.
- **The frames do not explain the wall.** 274 trials of at most 1,910 frames each are at most 523,000 frames. At E019's 46 µs a frame that is at most 24 s, against 84.9 s measured. This is an estimate.
  - Candidate causes: CPU contention from the runaway bounded readings above, or round-trip latency. E019 found the wait runs one Step round trip per frame (E019 plan.md:18).
  - The point's `engine_cpu_s` (0.81 s) cannot count the forked engines' frames, so it does not decide.
  - No profile has run.
- **E019's levers** (E019 plan.md:61-86):
  - A trial's wait is set by the offer's dues, not by a floor (:73).
  - A floor of 600 frames gained nothing.
  - E019 named the exact lever: stop a trial's wait when its state digest equals the no-input twin's at the same frame (:61-65). It was never built.
- **On the demo,** level 2 matters.
  - play10's 88 kept free jobs: level 2 took 9,167.4 of 11,544.7 s over 125,716 trials, and made 1,011 of their 1,515 kept states (`plan/play10-free.jsonl`).
  - play9: level 2 took 216 of 1,661 s.
- **A frame cap is ruled out by the data.**
  - Free entries come at most 711 frames after their parent in K1, E016's K1 and play9 (482 entries).
  - But play10 has 13,683 free entries, max 2,927 frames, 62 of them above 1,600 (`plan/freeoff.py`, run today).
  - E012's kitchen has 33 affordance entries, median 331 frames, max 9,241.

### Realize

- **The deferred sentence (G4).** Realize accepts a candidate only on the click's own Sentence touch (`fmi-play/src/realize.rs:681-698`), and otherwise misses at :388.
  - E016's probe shows walk_to(316) along the chain: the bare click logs no Sentence touch, and in the same frame 316's script starts at code offset 79. In the chain's own state, the click logs (11, 316, 0), and 316's script also starts at offset 79 (E016 plan.md:388-398).
  - E016's fix 3 accepted any start of a's script, whatever the verb. G3 failed, G4 fell to 4 of 14, and fix 3 was reverted in 2fda77b (E016 plan.md:400-436).
  - The named fix was not built: map the code offset to the verb through the object's verb table (:447-449).
  - E022 (27) added `runs_sentence` (realize.rs:79-86), which checks the sentence script's first three locals. It also added `ran_object` (:94-97), which checks no verb. `queued_frame_n` accepts on either (:110), within 8 frames.
- **The inventory.**
  - Realize does not scroll the inventory. An object scrolled out of view misses with "no sentence" (E016 plan.md:218, row 8). The DOS_CD profile has 8 slots.
  - Run B holds 9 objects from move 57 (frame 2,400), more than 8 at 188 of its 250 moves, and 16 at its last move (`plan/maxinv.py` on `experiments/E003-path-through-cloud/results/groundtruth-part1.jsonl`, run today; evaluation only).
  - Two fmi-play inventory tests have failed since 28a91e0 ("realize searches the whole slot grid when both objects are held"): `inventory_object_with_an_inventory_object` (15 against 18) and `inventory_image_slots_first` (2 against 3) (`tests/realize.rs:461`, `:490`; `~/.cache/fast-mi-e024/explore/tests-all.log:357-396`).

### The guide orders nothing on Part I

- Part I guide depths: 0:3, 1:15, 2:605, 3:497, 4:38, 5:4, 6:2 (`size-cost/guide-part1.json`, 1,164 atoms).
  - Of 17 milestone facts (owner:566, 396 and 388; bits 72, 65, 98, 85, 19, 84, 20, 86, 304, 51, 89, 88 and 76; var:196), 16 are at depth 2. bit:86 is at 4 (checked today against the guide's `keys`).
  - A guide from any of six milestone goals reaches 1,162 of the 1,164 atoms (`plan/guides/`).
  - The demo's guide is flat too: 1:16, 2:392, 3:113, 4:7.
- **Global 1 counts as a writer of the goal.**
  - It writes 184 of the guide's 781 written atoms, and is the only writer of 11 of them, among them bits 84, 85 and 86 (any value). It is also a writer of owner:901-903, beside room 83's local 200 (`writers`, checked today).
  - It writes these only in branches for a boot parameter other than 0 (`global/script-001.txt:366`, :520-531).
  - The engine starts global 1 once, with args[0] = the boot parameter (scumm.cpp:4296-4322). The host passes 0 (`fmi-engine/src/host.rs:23`). No script starts global 1 (grep: 0 hits in both games).
- **Indexed writes are missed.** The trial bits are written as `Bit[83 + Local[0]]` (`global/script-071.txt [008D]`). Global 71 is started with [1], [2] and [3] (script-116.txt:32, room 42's local-200.txt:14, room 64's local-200.txt:136).
- **The guide keeps which facts a condition reads,** not the value a path needs (guide.py:22-33), and closes under OR (guide.rs:1054-1116).
- **E022 found** that counting held guide atoms does not measure progress (E022 plan.md:333). It named depth as the next measure to try (:335, :387).
- **Engine variables** sit at depth 1 with any value: var 1, 5, 39 and 49 (scumm-ir vars.rs:9-25).

### Part I's shape, from the scripts (numbers only; the instance the landmark gate checks)

- **The end.**
  - Room 83's local 200 picks up 901-903 (local-200.txt:391-395).
  - Its only constant start is room 83's entry under `Var[277] == 3` (entry.txt:29-30).
- **Who writes `Var[277] = 3`.** Object 486 (stairs.txt:18-23), object 917 (village.txt:14-19) and object 450 (archway.txt:24-29) write it. Each write is dominated by `Var[196] >= 3`, `Bit[88]`, `Bit[89]`, `Bit[76]` and `Bit[51]`.
- **`Var[196]`.**
  - It is set to 0 at boot (script-001.txt:354).
  - Global 71 increments it whatever the trial's bit says (script-071.txt:8-18).
  - So the trials count in any order.
- **On run B's recorded states,** these hold in this order (my check on `groundtruth-part1.jsonl`, evaluation only):

  | Fact | Run B frame |
  |---|---|
  | var:196 = 1, 2, 3 | 3,635; 5,201; 6,277 |
  | bit:51 | 7,048 |
  | bit:89 | 7,435 |
  | bit:88 | 8,051 |
  | bit:76 | 9,278 |
  | var:277 = 3 | 9,507 |
  | owner:901 | 9,539 |

- **The goal can state only values.** Goals are value sets per key (archive.rs:389-401). The bridge already reads `{"var": N, "ge": c}` (speedrun_conditions.cpp:104-110).
- **No Part I ladder.** `fmi-checks ladder` hard-codes the demo's rungs (`fmi-checks/src/bin/ladder.rs:9-20`, :45-90).

### Scale

- **Part I against the demo** (`size-cost/`):

  | | Part I | Demo |
  |---|---|---|
  | Rooms | 31 (80 reachable in the files) | 11 |
  | Verb entry points | 606 | 226 |
  | Room scripts | 277 | 100 |
  | Guide atoms | 1,164 | 528 |

- **Demo, tries per rung** (`E022-place-context/c9/ladder-c9.json`): C1 45, C2 3, C3 3, C4 953, C5 4,923, C6 647, C7 2,354, C8 6, C9 813.
  - The 4 stretches that ended only after code fixes held 93% of the tries.
- **E012's kitchen:** 53 tries, 21.5 min at b = 2, mean try 31.3 s (`2026-11-03-e012-kitchen.md:12-17`).
- **A projection at today's cost** (estimate): 14 milestones × 53 tries × 362.1 s ÷ 4 workers = 18.7 h. At 30 s a try it is 1.5 h. So the cost of a try decides whether a run fits.

### Scripts

- E024's env.sh sources a chain through `wt-e014`, `wt-e013` and `~/.cache` files.
- Its `FMI_ENGINE_BIN` resolves to svm-e013, which lacks patches 0008-0010 (`E013-robust/integrate/env.sh:12`).
- b3.sh calls `pgrep -f` (`E013-robust/integrate/b3.sh:59`).
- The check scripts call wt-e014 by a hard-coded path (`a4/mi-check.sh:16`; `E016 run/mi-check.sh:14`).
- `mi-checks-now/scripts/` already fixes all three.

## Prior work

**The project's own**
- **Lemma W** (E004 wait-both.md:44-60): the paths with wait_both are the paths without it, plus passing paths. Any subset of the passing paths therefore over-approximates no less than the switch off does.
- **E019's audit and its named lever** (plan.md:10-86): the rejoin rule above. Its gate tries: play9's j00007, with 3 kept states.
- **READY_END and AFFORD_P0** (E024 Part 1, merged at 2c7e305).
  - They end empty windows after about 600 frames on the demo (E024 plan.md:65).
  - They never fired in the Monkey Island checks (E024 plan.md:61). No E016+ env.sh sets them.
- **Rejected changes:** E016's fix 3 and its revert; E019's floor (no gain).
- **The kitchen,** found by search at try 53 (E012), and E012's "partly" for Part I: at least 25 of 99 key facts and at least 3 rooms outside the k = 2 scope (E012 plan.md:478-482).
- **Seeds:** tuning uses 1-5; the Part I result is held out for 31-35 (E003 plan.md:35).

**Opened by the design passes** (paper text at `plan/hoffmann2004.txt`; web pages as linked in the designs)
- **Hoffmann, Porteous & Sebastia 2004, JAIR 22:215-278.**
  - Landmarks are facts true at some point of every plan. They are found by backchaining over what all achievers share, ordered, and used to split a task.
  - §5.1, disjunctive search control (:1973-1990).
  - The safety net and its completeness only without dead ends (:2075-2090).
  - Plans come out "slightly longer" (:120-128).
- **Boonstoppel, Cadar & Engler, TACAS 2008 (RWset):** prune paths that no later read can tell apart. This is the basis for keeping wait_both's passing path only where a held sentence is read later.
- **Go-Explore** (arXiv 1901.10995; 2004.12919): return to a kept state, then explore with a fixed budget.
- **Gilbert 1989:** the design rule against dead ends. Whether Monkey Island obeys it is unconfirmed.

**Abstracts and metadata checked only**
- Scala, Haslum, Magazzeni & Thiébaux 2017 (IJCAI 4384-4390): numeric landmarks, used for counters.
- Keyder, Richter & Helmert 2010 (ECAI 335-340): landmarks on AND/OR graphs. The write map is one: OR over achievers, AND over a site's literals.

**From memory, not opened:** Richter & Westphal 2010 (LAMA counts landmarks for its estimate); Porteous, Sebastia & Hoffmann 2001; Cooper, Harvey & Kennedy 2001 (dominance).

## Change

Every system change is off by default and on in E026's env.sh for both games. Each item is one commit with its gate. Nothing in run code names a room, script, object, variable or verb number.

Order of work:
1. Part 1 (instruments, scripts), G1.
2. C2, then C1, C3, C4, C5 (Part 2), G2-G5.
3. R1, R2 (Part 3), G6-G7.
4. Rule (2) with Parts 2-3 on, then K1a (the first Monkey Island search since E016), G8-G10.
5. Part 4 (L1-L4), G11-G12.
6. Rule (2) with every switch on, plus the demo checks, G13-G14.
7. Run 1 (Part 5); then the development loop; then the final fresh run.

### Part 1: instruments and scripts (outside the system)

- **I1, `experiments/E026-part1/run/`**: the scripts from `mi-checks-now/scripts/` (b3, smoke, k1, g3, g4, mi-check, guard, probe) and a new `watch.sh`.
  - **`env.sh`** holds ALLON's resolved values (`e016-survey/allon.txt`), then E026's switches. It sources nothing in wt-e013, wt-e014 or `~/.cache`.
    - `FMI_ENGINE_BIN` is svm-e024.
    - `FMI_GAME_ID/DIR/SCRIPTS` are set explicitly for each game, so no run relies on host.rs:19-20's default.
  - Every verify from boot uses the env under test.
  - The scripts check the `game/` link at start (a missing link voided a check; E016 plan.md:464-468).
  - The disk floor is 2 GB on the output disk.
  - No `pgrep -f`.
- **I2, `fmi-checks ladder --rungs FILE`.**
  - A rung is a set of (key, op, value) that must all hold, an optional "at least k of" group, and an optional full-state flag (C6's).
  - Each rung is scored on its own: its first entry, and that entry's chain. The score is the number of rungs one verified entry holds, so order does not matter.
  - The demo's rungs move to `rungs-demo.json`.
  - `rungs-part1.json` (evaluation only; the frames are run B's, which skips the intro):

    | Rung | Facts | Run B frame |
    |---|---|---|
    | P0 | the root (first control) | 2,728 here, without skips |
    | P1 | `owner:566 = 1` (kitchen) | 574 |
    | P2 | `bit:72 = 1` (money) | 1,056 |
    | P3 | `bit:65 = 1` (map) | 1,778 |
    | P4 | `owner:396 = owner:388 = 1`, `bit:98 = 1` (store) | 2,244-2,421 |
    | P5 | `bit:19 = 1` (sword training) | 4,224 |
    | P6-P8 | `var:196 ≥ 1`, `≥ 2`, `≥ 3` (the trials, in any order) | 3,635; 5,201; 6,277 |
    | P9 | `bit:304 = 1` (crew phase) | 6,517 |
    | P10 | `bit:51 = 1` (ship) | 7,048 |
    | P11-P13 | at least 1, 2, 3 of `bit:89`, `bit:88`, `bit:76` (the crew, in any order) | 7,435; 8,051; 9,278 |
    | P14 | `owner:901 = owner:902 = owner:903 = 1` (end) | 9,539 |

- **I3, `fmi-checks keyfacts ARCHIVE`:** E001's 99 Part I key facts held per entry. It is a Rust port of `mi-history/keyfacts.py`. Evaluation only.
- **I4, `watch.sh`:**
  - Every 300 s it scores I2 and I3, and samples MemAvailable and the driver's RSS.
  - It reports the first differing return (`return.same = false` in log.jsonl). `FMI_DIFFERS_DEAD=1` would otherwise hide it, and on Monkey Island none is expected (var 56 has 0 references).
  - The system never reads any of this.
- **I5, `stall-report --project`:** adds the line 14 × T_K1 (G10).

### Part 2: the cost terms

- **C2, a stopped reading stops** (reader `par.rs:544-600`, :182-187; no switch).
  - Each worker's job looks at the abort flag every 4,096 machine steps and ends as "stopped".
  - A stopped reading's files are never used (bound.rs:198-200), so no result can change.
- **C1, reader switch `wait_queued`** (machine.rs:1436, :2423-2440; switch parsing in builder.rs and table.rs:94).
  - In the `None` arm, the passing path is forked only when the path holds a sentence (`sx(p).queued`). Elsewhere the op yields, as with the switch off.
  - WaitForSentence's decided branches (:2414-2422) stay.
  - By Lemma W, any subset of passing paths is still an over-approximation.
  - The token is in the reading cache's key (`fmi-explore/src/step.rs:20-23`).
  - E026's `FMI_READ_SWITCHES` is E022 env.sh:51's list with `wait_queued` in place of `wait_both`.
- **C3, READY_END and AFFORD_P0 on.** No new code (E024 Part 1).
- **C4, the free trial's wait. A profile comes first, then one lever.**
  1. **Profile.** One level-2 point of K1's j00007 (274 trials), re-run alone on an idle laptop after C1 and C2. It records, per trial:
     - the frames run;
     - the Step round trips;
     - the engine processes' CPU (from `/proc`, children included);
     - the driver's CPU;
     - the run-queue wait (`/proc/PID/schedstat`).
  2. **If the trial's frames dominate:** `FMI_FREE_REJOIN=1`, E019's rule.
     - A trial's wait stops at the first checked frame where its state digest equals the no-input twin's at the same frame. Every later frame would be the twin's, whose stops are already read.
     - The digest is E019's (save stream, RNG, input registers, timers).
     - The check interval is set from the profile's digest cost.
     - It loses nothing by construction, has no tuned constant and names no game.
  3. **If the number of trials dominates:** `FMI_FREE_BUDGET=N`.
     - A free try forms at most N trials. Level-2 points run in order of their level-1 outcome's novel atoms.
     - N comes from play10's level-2 kept states (G5), not from play9 alone.
  4. **If contention alone explains the wall,** C2 is the fix, and G5 shows it.
  - No frame cap on the trial's wait (see Problem).
- **C5, laptop settings.**
  - `FMI_READ_MAX_RSS_GB=6`. This is an estimate, above the largest measured reading process (2.6 GB); G9 measures it.
  - `FMI_REACH_TRY_LIMIT=1` (harness.rs:727-730).
  - W, the worker count, comes from G9.
  - Runs start only with MemAvailable ≥ 10 GB (11.1 GB now), under an `idle:sleep` inhibit lock.

### Part 3: realize, and b3's reference

- **R1, `FMI_REALIZE_DEFERRED=1`** (realize.rs:681-698).
  - Applies to a candidate whose records hold ego's Walk touch and no Sentence touch.
  - Its fork runs on with `segment(true)` to the next idle stop, at most `Cfg.horizon` (afford::W = 1,600 frames), with no other input.
  - It is accepted when either:
    - (a) object a's script starts at the code offset of a's entry for verb v. The offset is read from the object's verb table in the extracted game files: E016's named fix (plan.md:447-449);
    - or (b) `runs_sentence` sees (v, a, b).
  - `ran_object` alone never accepts.
  - For two-object sentences, (a) does not check b. This is recorded as a limit.
- **R2, `FMI_REALIZE_SCROLL=1`.**
  1. First the two failing inventory tests are explained against 28a91e0's whole A×B grid, then fixed (the test, if the new count is the grid's; else the code).
  2. When no visible slot takes object a, realize clicks each visible verb that names no object and that the input log takes.
  3. Such a click counts as a scroll if it changes the inventory slots in the dump. G7 measures which dump field changes.
  4. At most 8 clicks per direction, then the slot search again.
  - No verb id is read, so the profile's arrows (208/209) stay unused.
- **R3, b3's reference: a decision for the PI. A D-entry, not recorded here.**
  - Evidence first: b3's 8 class-differing tries are re-run with `FMI_REFINE_WIDEN` unset.
  - Proposal: b3 compares against a reference recorded once on the E026 build with every switch on. Each class change from E013 is listed with its switch. verify-new must still show 0 differences, and no reading may reach the bound.
  - Until the PI decides, G8 and G13 report b3 both ways.

### Part 4: landmarks from the scripts (guide.rs; a new `fmi-explore/src/landmarks.rs` on guide.rs's `Code` and `WMap`)

- **L1, guide fixes, `FMI_GUIDE_ARGS=1`.** Applies to the guide and the landmarks.
  - (a) **Start arguments.** A condition on a local that holds a start argument is decided for each constant start: startScript's or startObject's constant arguments, and the boot's args[0] = the boot parameter (scumm.cpp:4296-4322; host.rs:23). A write site that no start reaches is not a writer. Global 1's boot-parameter branches drop out, and its other writes are start facts.
  - (b) **Indexed writes.** `Bit[c + Local[k]]` with Local[k] a start argument resolves for each constant start. An unresolved index counts every key it can reach.
  - (c) **Engine variables** (`scumm_ir::vars::VARS`) get no writers in the regression. Reported separately from (a) and (b).
- **L2, `fmi guide landmarks`** (design 2's derivation).
  - **Literals per write site.** For each conditional jump j that dominates write site i, where i is reachable (without passing j again) from only one of j's successors, that successor's literal is needed.
    - Literals compare a var, bit, owner, state or class with a constant, with the operator, read as in guide.rs:818-916.
    - An IR Jump is taken when its condition is false (ir.py:245-248).
  - **Start needs:**
    - entry, exit and local code need `room = r`;
    - a local or a global needs the literals shared by all its constant starts (a greatest fixpoint);
    - a computed start counts as starting every unit its variable can name.
  - **Achievers:** every site that can make the literal true, indexed writes included (L1(b)), over-approximated.
  - **Feasibility:** a least fixpoint from the start facts.
  - **Backchaining** from the goal's literals.
    - A landmark's predecessors are the literals all its feasible achievers share (site literals plus start needs) that are not true at the start.
    - They are ordered before it.
  - **Counters:** K ≥ c whose feasible writes are increments by 1 gives K ≥ 1, …, K ≥ c−1, in order.
  - **Branches:** when a landmark's feasible achievers share no literal, each achiever's own landmarks form a branch.
  - **Output:** `landmarks.json`, with each literal's cite (unit:offset), the order edges and the branches. It runs on any game's files.
- **L3, `FMI_SELECT_LANDMARKS=1`.** SELECT_FURTHEST's measure (archive.rs:2129-2140) and SELECT_ADVANCED's front order (:3087-3093) count held landmarks instead of held guide atoms: the trunk's plus the best branch's. If G12 fails, L3 stays off, and the run uses E022's selection. This is recorded.
- **L4, `K>=V` literals.**
  - Supported in `parse_goal` (archive.rs:389-401), the goal test (:1003-1005), verify's final check, and `goal_env` (fmi-shorten chain.rs:315-342), which emits the bridge's `"ge"`.
  - The landmarks use it. The run's goal stays `part1`.

### Part 5: the run, the development loop, and one fallback

- **Run 1** (below) is fresh from boot.
- **Development loop.** If Run 1 stops before P14:
  1. The stop is written up as a dated amendment: the rung, the stop classes, the stall report's top cause, and the mechanism that binds.
  2. The gap is fixed as a general mechanism, off by default and on for both games. Its gate runs offline on the stalled tries (`fmi try` on their job.json, copied out of the archive).
  3. Then kitchen, smoke, G3, G4 and the demo route replay run.
  4. The archive resumes for at most 4 h. This is development only.
  - At most 8 development segments.
- **Final fresh run.** When a development archive reaches P14, the final build runs once from boot.
- **Fallback S1, fixed in advance:** `fmi explore init --start A:I` (design 2's C1).
  - Built only if one archive passes 4 GB or the driver fails G9's memory bound in a run.
  - The root replays entry I of A from boot and runs to its first idle stop. The movie becomes entry 0's events. archive.rs:1067-1071 allows root events only in this case.
  - Gate:
    - Monkey Island: a root at E012's entry 150 replays to frame 18,416 with 0 differences, and a 4-try smoke run's entries verify from boot.
    - Demo: from play10's C7 entry 24381, room 48 is reached within 30 tries (C7→C8 took 6).

## Gates (offline, all before Run 1)

| Gate | What | Threshold |
|---|---|---|
| G0, base | The six checks on 3cb3b33 | Measured; see Problem |
| G1, I1-I5 | `ladder --rungs rungs-demo.json` on play9; `rungs-part1.json` and `keyfacts` on `e012-archives/kitchen` | JSON byte-identical to the built-in ladder. P1 at entry 150, frame 18,416. Key facts #1, #8, #15, #16, #19, #20, #21, #45, #51 (`mi-history/keyfacts.out`) |
| G2, C2 | A room-28 try with the old `wait_both` and a 30 s bound; j00006 with `FMI_READ_SWITCHES` unset | The try ends ≤ bound + 10 s, its reader threads gone (now 151-594 s past). j00006's reading files are byte-identical with and without C2 |
| G3, C1 | j00006 cold and alone; the demo's room-48 job `~/.cache/fast-mi-e022/r48-data/job-28025.json` under `wait_both` and under `wait_queued` | j00006 ≤ 90 s, not bounded. The demo try's attempt events and kept entries are the same both ways, and the counter of held sentences is ≥ 1. If the demo differs, wait_queued is wrong: stop |
| G4, C3 | K1's 6 room-33 model tries; b3's 4 window tries (j00003, 07, 10, 13) | b3's windows end ≤ 1,000 frames after their last entry (now 3,600). Room-33 tries ≤ 10 s each (now 3.4-33.8 s). The kitchen and smoke lose no entries. If READY_END does not fire, its cause is named with file:line before G8 |
| G5, C4 | The profile; then K1's j00007, play9's j00007, and the 10 play10 kept free jobs with the most level-2 kept states, each re-run by `fmi try` with the lever off and on | K1 j00007 ≤ 60 s (now 841.3 s). REJOIN: the same kept states in every job (57; 3; each play10 job's). A budget, if chosen: ≥ 95% of the play10 sample's level-2 kept states, and play9's j00051 still makes `owner:946 = 1` at frame 1,499 if its reading can be rebuilt on the laptop (E019 could not: plan.md:75-76) |
| G6, R1 | `e016 g4`; `e016 g3`; fake-engine tests | G4: 14 of 14 per job and along the chain, `owner:566 = 1` verified from boot. G3 ≤ 15 candidates. Tests: a deferred sentence is accepted by the verb's offset or the sentence locals; a start of a's code at another verb's offset is a miss; a click with neither is a miss |
| G7, R2 | The two inventory tests; a fixture from run B's movie replayed on its own seed (as E003 step 5 did) to frame 2,400, where 9 objects are held. The fixture is a test input only, never a route | Both tests pass with their counts explained. Every held object's "look at" is realized, each verified from the fixture's movie. G3's and G4's candidate counts are unchanged |
| G8, rule (2) with Parts 2-3 on | kitchen, b3, smoke, G3, G4, `cargo test --locked` | Kitchen 8 of 8. Smoke: entries verify, 5 PASS, 0 readings at the bound. b3: 0 bounded, new entries 0 differences, every class change listed with its switch; the 8 tries without REFINE_WIDEN give E013's class in 8 of 8 (evidence for R3). All tests pass, switches off and on |
| G9, K1a and memory | The kitchen from boot, W = 3, 60 min, idle laptop (load < 2, MemAvailable ≥ 10 GB at start) | Goal reached, chain verified from boot. No guard kill, no reading at the bound. Mean try ≤ 30 s. Overrun past `FMI_STOP_WALL` ≤ 5 min. MemAvailable ≥ 4 GB throughout. W for the run = ⌊(MemAvailable at start − 4 GB) ÷ (driver peak per worker)⌋, at most 6 |
| G10, projection | K1a's wall from start to the verified goal entry, T_K1 | 14 × T_K1 ≤ 4 h, so T_K1 ≤ 17.1 min. The model is an estimate: every milestone costs what the kitchen from boot cost |
| G11, L1-L4 sound | (a) Fake units: a dominated outcome, a join, a loop, a start argument, a computed start, a counter, an indexed write. (b) Part I landmarks on run B's recorded states (`groundtruth-part1.jsonl`, not a replay: run B's inputs leave its moves at S1#52 on seed 1, 2026-11-01-e004-trace.md:43). (c) The traced instance. (d) Demo landmarks for goal `end` on route-004's and movie-c9-run's states. (e) Unit tests for `K>=V` | (a) all pass. (b) Every trunk landmark holds before the Part I end, every order edge holds on first-hold moves, and every branch's landmarks hold before that branch's own trial: 100%. (c) var:196 ≥ 1, 2, 3; bits 51, 76, 88, 89; var:277 = 3; room 83: 9 of 9. Global 1 has 0 feasible achievers; global 71 writes bits 84-86. (d) 100%. Derivation ≤ 10 s on Monkey Island, ≤ 5 s on the demo. Reported: trunk, branch and stage counts, and the share of E001's key they cover (evaluation only) |
| G12, L3 useful | Held landmarks along E012's goal chain (8 entries) and over play10's C0-C7 rung entries (3, 128, 136, 144, 3412, 7987, 10534, 24381) | Non-decreasing on both. Reported: Part I milestone depths with L1 on |
| G13, everything on | G8's checks, K1b (as G9), and the demo: route-004 verified from boot by both launches; movie-c9-run; the demo from boot | Rule (2) as G8 and G9. Route-004: 33 of 33 digests, end qf 55,555. movie-c9-run ends at `restart@1:582:48:0067`. The demo reaches C3 within 100 tries (play9: try 51). This is the bar for a merge proposal |
| G14, generality | grep over the changes | No room, script, variable, object or verb numbers in fmi-*, reader or scumm code. Monkey Island's rungs live only in fmi-checks |

- If K1a misses the kitchen in 60 minutes, H1-H2 are refuted as stated. Stop and write up its stall before Part 4.
- If G10 fails, the top cost term of K1a is profiled and fixed before any Part I run.

## Run, prediction, stop rule

**Run 1** (laptop, seed 1, W from G9, nice 10, E026's env.sh, the binary frozen at G13's build, outputs under `~/.cache/fast-mi-e026/part1/`):
- `fmi guide map --goal part1 --start-boot --out guide-part1.json`
- `fmi guide landmarks --goal part1 --out landmarks-part1.json`
- `fmi explore init $A --stage part1 --goal part1 --guide guide-part1.json --allowed files --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts --stop-jobs 100000 --ready --commit-as-finished`
- `guard.sh LOG fmi explore run $A --workers W --search fd --tail`, with `FMI_STOP_WALL=14400` and `watch.sh`.
- Engine folders and reading bulk are pruned as the run goes, as `mi-checks-now` did (235 MB in all).
- At the end, the first entry of each rung, and the entry with the most key facts, are verified from boot.

**Prediction.** Everything except the "Now" column is an estimate.

| Measure | Now (measured) | Central | Range |
|---|---|---|---|
| j00006 reading (C1) | at the 300 s bound | 75 s | 36-90 s |
| A bounded try's time past its bound (C2) | 151-594 s | ≤ 10 s | 0-10 s |
| K1 j00007 (C4) | 841.3 s, 57 kept | 60 s, 57 kept | 30-250 s |
| Driver peak RSS in K1 | 8,373 MB at the kill | 3.5 GB | 2-6 GB |
| G4 along the chain (R1) | 12 of 14 | 14 of 14 | 12-14 |
| K1a tries to the kitchen | killed after 18 | 55 | 25-150 (E012: 53) |
| K1a mean try | 362.1 s | 30 s | 18-60 s (E012: 31.3 s) |
| K1a wall to the goal, T_K1 | killed at 38.1 min | 15 min | 8-60 min (E012: 21.5 min at b = 2) |
| G10's projection, 14 × T_K1 | – | 3.5 h | 1.9-14 h |
| Part I trunk landmarks | 9 traced by hand | 20 | 9-40 |
| Run 1: rungs P1-P14 held by one verified entry | E012's archive: 1 (P1) | 3 | 1-6 |
| Run 1: E001 key facts on the best verified chain | E012: 9 | 20 | 12-35 |
| Run 1: Part I rooms entered | – | 15 of 31 | 10-22 |
| Run 1: Part I end | – | not reached (under 5%) | – |
| Development segments to reach P14 | – | 8 | 4-12 |
| Final fresh run reaches P14 within 4 h | – | 20% | – |

- The central K1a wall assumes W = 3, 55 tries at 30 s (9.2 min of try time per worker) plus E012's overhead ratio (21.5 min of wall for 13.8 min of try time per worker).
- The 8 development segments scale the demo's rate (5 of 9 transitions needed a code change) to 14 rungs.

**Cost.**
- **Builds:** 5 min 4 s, plus 1 min 46 s for the check binaries (measured).
- **Gates:** about 5 h of laptop wall:
  - b3: 28 min now;
  - smoke: 11 min now;
  - K1a and K1b: at most 60 min each;
  - the demo from boot: at most 15 min;
  - profiles and probes: about 45 min;
  - the rest: minutes.
- **Run 1:** 4 h.
- **Each development segment:** at most 4 h, plus about 1 h of checks.
- **Final run:** 4 h.
- **Disk:** an archive of at most 3 GB, by estimate. E016's K1 archive was 3.3 GB for 132 tries; with pruning, entries are about 95 KB each (play10). Free now: 6.0 GB on /, 6.8 GB on /home.
- **Cloud:** $0.

**Success.**
- **Yes:**
  - the final build's fresh run reaches P14 from boot on the laptop within 4 h (plus 5 min overrun);
  - its goal chain verifies from boot with 0 differences;
  - G13 passes on that build;
  - no guard kill, no reading at the bound, no differing return.
- **Partly:** G13 passes (a merge candidate; the PI decides), and the best verified chain meets E012's "partly" (≥ 25 of 99 key facts, ≥ 3 rooms outside the k = 2 scope).
- **No:** K1a fails, or G10 fails after its fixes.

**Stop rule.**
- **Before any run:** a failed gate is fixed offline first. No run starts with a known gap.
- **During a run:**
  - Stop after 60 min with no new rung and no new key fact (outside instruments), at 4 h, or on the guard.
  - Stop at once on an entry that differs from boot, or on the first differing return, until it is explained.
  - A guard kill is not counted: measure memory per worker, fix, and rerun from scratch.
  - Under 2 GB free: move the outputs to the other disk and continue.
- **E026 ends and is written up when:**
  - a stall needs a mechanism no component has (research comes first, and it becomes E027's question);
  - the same stall class comes back after its fix;
  - a fix would need a game number;
  - 8 development segments are spent;
  - or the final run is done.
  - No hand patches. No probe input or run B state ever becomes a route input.
- **Only a fresh run from boot counts.** A resumed archive is development only.

## Out of scope

- **Shortening Part I.** E024's drop rounds take about 3.3 h a round (E024 plan.md:239). The kitchen's step 5 is E025's, and E025 is paused.
- The VM; Parts II-IV; seeds other than 1 (31-35 stay held out); glitches.
- Reading the whole of Part I at once (70 GB, E003 attempt 2).
- Draw-aware dialog judging.
- Changes to SELECT_* constants, `PLACE_CTX`, `READ_HELD` or `GOAL_SITE` (no effect under a fact goal), unless a stall names them.
- Replacing the DOS_CD profile beyond R2.
- Splitting the remaining 36→75 s of j00006's reading.
- A frame cap on trial waits.
- A segment driver beyond S1.
- A Part I end derived from the game: with the game's end as goal, Monkey Island gives end entries, never goal entries (E013 plan.md:849).
- Verifying every entry from boot (about 22 h at 0.8-1.2 s an entry). Only rung, goal and gate chains are verified.

## Unconfirmed inputs

- Tries per milestone beyond the kitchen. This is the projection's dominant unknown, and G10's model (14 × T_K1) rests on it.
- What makes a free trial's wall 3.5 times its frame estimate: contention, latency or frames. The profile decides.
- Whether the rejoin check costs less than the frames it saves.
- Whether every passing path the demo needs holds a sentence. G3 checks room 48 only.
- Whether 316's deferred start is read at the walk_to entry's offset in the files (E016's probe: offset 79 both ways).
- Whether run B's movie replays to frame 2,400 on its own seed with svm-e024.
- How Monkey Island's inventory scroll shows in the dump.
- The landmark counts, the branches before the first trial, and whether selection by landmarks helps at all.
- Whether Monkey Island Part I is free of dead ends.
- Part I mechanics with no demo analogue:
  - insult fights (random opponents, learned comebacks);
  - the store's credit;
  - the circus;
  - the forest pseudo-rooms 201-220;
  - the map (room 85, verbless in the profile).
- Part I's random draws (144 in room code, 111 in globals).
- The two dialog-frame kinds the handler idle rule blocks (E013 plan.md:797-802).
- `READ_HELD`'s size with Part I's inventory (one held object: 1,507 → 424,163 records on the demo; E020 plan.md:28-30).
- SELECT_FURTHEST, SELECT_CONTINUE, SELECT_ADVANCED and PLACE_CTX across 80 rooms.
- The room-33 stop on sentence(3,428) in 4 of 6 K1 model tries.
- Memory at run time: other programs hold about 12 GB, and swap is full.
- That the 8 b3 class changes come from REFINE_WIDEN. G8 tests it.
- E016's 7 unexplained b3 field differences.
- The labels of probe lines B and C.

## Amendment 1 (2026-11-10): after G3

File:line references in this amendment are at 0df2ab4 (branch `e026`), not 3cb3b33. The sources are G3's outputs (`results/g3/`) and three investigations under `~/.cache/fast-mi-e026/build/`. None of the investigations' code is committed.
- **A**, `inv-demo-needs/`: what the demo needs from wait_both's passing paths.
- **B**, `inv-mi-blowup/`: why room 28 is slow under wait_both.
- **C**, `inv-other-levers/`: other levers on K1's room-28 cost.

### G3's result

- **(a) passed.** b3's j00006, cold and alone under wait_queued, read in 62.2 s (228,924 records, the same as with `FMI_READ_SWITCHES` unset). It was not bounded.
- **(b) failed.** The demo's room-48 try (job-28025) was run twice each way, and each way reproduced byte for byte:

  | | wait_both | wait_queued |
  |---|---|---|
  | Reading records | 24,649 | 19,520 |
  | Plan 0 | 28 steps | 34 steps |
  | Attempt | 2 inputs, then a 3,600-frame empty window to frame 22,871 | 9 inputs, frames 19,204-19,374 |
  | Kept entries | 5 | 12, none the same |
  | Stop | "no plan" | "no plan" |

  - Both had the same 21 targets and the same target (`state:582=1`).
  - wait_queued's counts: 376 WaitFor ops also passed on a path holding a sentence, and 3,614 only yielded.
- By G3's rule, wait_queued is wrong as C1. It stays in the code, off (dbdc575).

### What the investigations found

**A. What the demo needs (job-28025, room 48; tables in `inv-demo-needs/out/`).**
- The readings at the try's base (frame 19,203):

  | | Records | Actions | Actions that pass a WaitFor op |
  |---|---|---|---|
  | wait_both | 24,649 | 21,631 | 5,089 |
  | wait_queued | 19,520 | 17,046 | 380, all at script 2's 0x464 |
  | both off | 18,918 | 16,656 | 0 |

- wait_queued's reading is wait_both's minus the passes on paths that hold no sentence.
  - All 16,542 of wait_both's pass-free actions have exact twins under wait_queued.
  - Of wait_both's passing actions, 380 have exact twins and 4,683 are absent.
- **wait_both's plans used passes that hold no sentence.** All of them are at the top level of the sentence script (script 2): 0x372 (a WaitForActor after a walk, 3-4 steps in every plan), 0x464 and 0x3A1.
  - A pass skips the resume event, so the plans are shorter: plan 0 is 28 steps at cost 45, against 34 steps at cost 46 under wait_queued.
- **Both such passes the engine played were wrong.**
  - `#152` (0x372) deviated at frame 19,207: ego walked, and the resume `#981` matched 59 frames later.
  - `#984` (0x464) was refuted: "the wait did not block".
  - None of wait_both's 5 kept entries depends on a pass holding.
- With both switches off the try stops at frame 19,271 on "no plan", with 3 entries.
- **So the demo needs the passes that hold a sentence.** A blocking record would drop the held sentence, which lives on the path (E022 (27)). On this job the demo needs none of the other passes, but the planner prefers them.
- Monkey Island's room 28 holds no sentence at any wait. Under wait_queued: 568 sentences held, 0 passes on a path holding one.
- **Top level against nested calls.**
  - On the demo, 5,078 of the 5,089 passing actions pass at the run's top level. The other 11 pass at obj:582@0x62, a WaitForMessage inside the sentence script's run.
  - In room 28, 71,474 of the 107,025 passing actions pass inside a call that another script started in the same run.

**B. Why room 28 is slow under wait_both (K1's j00001, scope [28], read by reader-scope alone at 3 threads).**

| Run | Records | Wall | Peak RSS |
|---|---|---|---|
| wait_both off | 31,926 | 17.8 s | 326 MB |
| wait_both | 162,826 | 189.3-341.7 s over five runs (A, B; load 3-7) | 1,885-2,295 MB |

- 107,025 of 143,951 actions pass at least one wait.
- The cost comes from object 322's verbs 8 and 10, which start the dialog script local 220 in the same run. Every passed WaitForMessage after that multiplies the paths.
- **How the records grow.**
  - Along a chain of passed waits, the records add up. Across slots in one round, they multiply.
  - No loop re-enters a wait.
  - There are 32,939 distinct effects, against 11,048 with wait_both off. The same dialog continuation is read again for each prefix, verb, rerun and pass.
- `pass2_reuse` (lossless) gave the same reading and no gain (309.8 s).
- **The decided WaitForMessage.** These engine facts decide the op after a talk:
  - The op passes only when VAR_HAVE_MSG is 0 (script_v5.cpp:3316-3319).
  - A print or printEgo text for any actor except 252-254 goes to text slot 0 (script_v5.cpp:3475-3487, :3596-3598).
  - That slot calls actorTalk, which sets VAR_HAVE_MSG to 0xFF (actor.cpp:3541-3542). The only exception is a talking actor that is not in the current room (:3502-3518).
  - Within a frame nothing clears VAR_HAVE_MSG in v5. stopTalk writes it only for DIG and CMI (actor.cpp:3597-3601), and the per-frame copy runs before the scripts (scumm.cpp:3383-3384).
  - The reader's frame costs already rest on this (costs.rs:55-56).

  B's scratch patch (`talk-rule.diff`) on j00001:

  | Variant | Records | Wall | CPU s | Peak RSS |
  |---|---|---|---|---|
  | Any talker (an upper bound) | 86,907 | 65.8 s | 189.1 | 594 MB |
  | Sound rule, today's start row | 162,697 | – | – | – |
  | Sound rule, the dump's actor rooms in the start row | 86,916 | 40.8 s | 118.0 | 623 MB |
  | That start row, rule off (run back to back) | 162,826 | 219.3 s | 643.2 | – |

  - Today's start row holds no actor rooms: every actor's room is NOT_KNOWN (table.rs:148-152). So the sound rule decides almost nothing.
  - The dump lists the visible actors in the current room (speedrun_state.cpp:629-640). j00001's dump lists actors 1 and 3.
  - On the demo's room-48 reading, the rule and its upper bound both give a byte-identical reading (packed sha1 f776e50d). The reading has 4,624 talks, and the rule blocked no WaitForMessage after any of them.
- **The rule does not fix j00006.**
  - j00006 starts in room 33, where actor 3's room is not known. Local 204 writes 28 or 0 when room 28 is entered, so paths that do not hold 28 keep both branches.
  - With the dump's actor rooms, the rule hit an 8-minute cap in pass 2: 48,483 pass-2 records and 1,281 CPU s, against 53,342 records and 1,134 CPU s with the rule off.
  - Even its upper bound read j00006 in 115.4 s (382,691 records), against 59-69 s and 228,924 records with wait_both off.

**C. Other levers on K1's room-28 cost (K1's archive; `inv-other-levers/k1-room28.tsv`).**
- **Where K1's 2,659.3 s went.**
  - 1,500.2 s were spent at the bound (j00001, 12, 13, 14 and 16).
  - 1,159.1 s were spent waiting at the share gate (j00013 152 s, j00014 414 s, j00016 594 s; see Corrections).
  - All six tries kept 0 entries.
  - In the killed tail, j00017 waited 600 s at the gate and then read in 276.4 s.
- **Records are deterministic, but whether a reading hits the bound is not.** j00001's reading took 189.3-341.7 s depending on load.
- **Skipping room-28 tries would lose the kitchen.** Every measured entry into room 41 came from a model try on a room-28 entry whose reading finished: E012's goal entry 150, and E016's 341 and 344.
- **Levers:**

  | Lever | Kind | Saves in K1 | Taken |
  |---|---|---|---|
  | Defer a try whose scope's share gate is held | scheduling, lossless | 1,159.1 s, plus 2,087 s in the killed tail | not now (see below) |
  | Check the stopped set before the gate (bound.rs:120) | lossless | 0 s, plus 908 s in the tail | no |
  | Let a bounded model try fall back to `readings_any` | adds work | 0 s, and costs 113.6-694.6 s per try | no |
  | Lower `FMI_READ_BOUND`, or set `FMI_READ_BOUND_BG` ≥ 1 | a tuned cap (D9), or more load | up to 1,350 s at a 30 s bound, but loses readings like j00017's (276.4 s) | no |
  | Skip room-28 model tries | loses the kitchen | – | no |

### C1', the decided WaitForMessage (replaces C1)

- **The rule** (reader switch `wait_talk`, used with `wait_both`):
  - It applies in wait_both's `None` arm (reader machine.rs:2426-2449).
  - A WaitForMessage op there only yields, with no passing path, when the path ran a talk earlier in the same frame (the path's settle round, `Path::round`; the build checks that a round never spans two frames).
  - A talk is a print or printEgo with a text, by one of:
    - actor 255;
    - an actor whose room the path holds without a fork (its write, its single precondition value, a pin or a one-value domain), equal to the room the path holds.
  - Texts by actors 252-254 are not talks: they go to other slots.
  - The rule does not apply if the path wrote VAR_HAVE_MSG after the talk.
  - Every other WaitFor op is handled as wait_both handles it now.
- **The start row** (same switch). The try's start row gets `actorroom:a = room` for each actor in the entry checkpoint's `actors` list (visible and in the current room). Actors not listed stay NOT_KNOWN.
- **Why it is lossless (D9).**
  - The rule removes only passing paths the engine cannot take. After such a talk in the same frame, VAR_HAVE_MSG is 0xFF, and the op passes only at 0 (B's engine facts).
  - The start row adds only facts the dump shows of the start state.
  - So every engine path that wait_both reads is still read.
  - It is also a subset of wait_both's passing paths, so Lemma W holds too.
- **No game numbers.** Actors 252-255 and VAR_HAVE_MSG belong to the engine (the reader's `domains::ENGINE`, domains.rs:28), not to a game.
- **Switch and keys.**
  - Off by default.
  - When on, it is in the memo's key and in the reading cache's key, through `--switches` and the start row's facts.
  - On in E026's env.sh for both games when G3' allows: `FMI_READ_SWITCHES=value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost,wait_both,wait_talk`.
- **Tests.**
  - A print by actor 255 followed by a WaitForMessage in the same frame: the op only yields.
  - A printEgo by an actor the path holds in the room: the op only yields.
  - Both paths are kept when the actor's room is not known, when the talk was in an earlier frame, when VAR_HAVE_MSG is written in between, or for actor 252.
  - With the switch off, room 33's reading is byte-identical.
- **Not explained.**
  - A's static scan counts 11 demo actions that pass obj:582@0x62: a WaitForMessage right after a printEgo, with no yield between them (the printEgo follows a cutscene op).
  - B's scratch rule left them undecided, even in its upper bound. Why is not checked.
  - The built rule's tests include that shape.
  - job-28025's checkpoint has no actor list, so on that job the sound rule decides ego's talk only where the path holds ego's room.

### Candidates against G3'

| Candidate | D9 | (a) demo try as under wait_both | (b) j00006 ≤ 90 s | (c) room 28 under the bound |
|---|---|---|---|---|
| wait_both (ON now) | – | itself | no: unfinished at 1,500 s (A) | no: bounded 5 times in K1; 189.3-341.7 s alone |
| wait_queued (C1) | Lemma W subset | no: G3(b) | yes: 62.2 s | yes: j00001 in 20.8 s (A) |
| Both off | – | no: "no plan" at frame 19,271, 3 entries (A) | yes: 59.1-69.3 s (G2, `FMI_READ_SWITCHES` unset) | yes: j00001 in 17.8 s (B) |
| `pass2_reuse` | lossless | not run | not run | no: 309.8 s (B) |
| **C1'** | lossless | the reading is byte-identical (B); the try was not run | **no: 8-min cap in pass 2 (B)** | j00001 in 40.8 s (B); 6 starts not run |

- **No candidate measured meets all three thresholds.**
- **C1' is chosen** because it is the only candidate that keeps the demo's reading and brings room 28 under the bound, and it loses nothing. On (b), the evidence is against it.
- **Not measured, so not chosen: passes only at the top level** (dropping the passes inside a nested call).
  - It is a Lemma W subset.
  - Inside a nested call, the engine runs the rest of the called script before the caller's rest. Block-and-resume runs them in the opposite order (A). So it can drop engine paths that wait_both reads.
  - It touches 71,474 of room 28's 107,025 passing actions, and the demo's 11 at obj:582@0x62.

### G3' (replaces G3)

| | What | Threshold |
|---|---|---|
| (a) | The demo's job-28025 by `fmi try`, cold, under ON with C1' and under ON without it, twice each | The same attempt events and kept entries both ways (compared as in G3), and each way reproduces. Reported: records, the count of WaitForMessage ops decided after a talk, and A's test of need (every record a wait_both window matched, and every pass the engine confirmed, has a twin under C1') |
| (b) | b3's j00006, cold and alone, under ON with C1' | ≤ 90 s, not bounded |
| (c) | K1's room-28 model tries, one per distinct start: j00001, 12, 13, 14, 16, 17, 18 (entries 15, 53, 38, 39, 239, 244, 267), each by `fmi try` on its job.json, cold and alone, under ON with C1' | Each reading ends under the 300 s bound. Reported: each reading's wall and records |

- **How it runs.** Each timed part needs quiet_ok, as G3 did, and runs under the heavy lock with MemoryMax=5G.
- **The start rows.** All 7 starts' dumps list actors 1 and 3 in room 28 (checked for this amendment on K1's entries).
- **Prediction** (estimates; the measured inputs are in brackets):
  - (a) passes [the reading was byte-identical in B's runs].
  - (b) misses and the reading is bounded [the 8-min cap in pass 2; the upper bound needed 115.4 s].
  - (c) j00001 reads in about 41 s [40.8 s at load 3-5]. The other 6 read under the bound, but they have not been measured.
- **Stop rule.**
  - If (a) differs: stop, and report which records C1' dropped and whether a played step used one (A's test).
  - If a reading in (c) is bounded: stop, and measure that start.
  - If (b) alone misses: C1' still goes on. It is lossless, and (a) and (c) are what it was chosen for. The next section applies.

### If (b) misses: room 28 in the run, and b3

- **The run's room-28 readings.**
  - Under `FMI_SCOPE_GROW=room`, a room's reading is that room alone (grow.rs:24-29). All 22 of K1's jobs had one-room scopes, 13 of them [28].
  - These readings start in room 28 with the dump's actors, which is j00001's case. They are read under C1'.
  - A reading that still reaches the bound stops there (C2) and keeps 0 entries, and no reading stands in, as today.
  - The bound stays at 300 s. No cap is added.
- **Cost in G9's and G10's terms** (estimates from K1's measured terms):
  - **Under C1'.** Taking j00001's 40.8 s, K1's room-28 tries would spend at most about 330 s reading and waiting at the gate, against 2,659.3 s. That is five fresh readings, plus queued tries that each wait at most one reading.
  - **K1's other terms stay:** free tries 3,731.7 s (C4's) and room-33 tries 127.4 s. K1's mean over its 18 tries would then be about 233 s, so C1' alone does not meet G9's 30 s mean.
  - **A reading still at the bound** costs one worker 300-310 s and keeps nothing.
    - One such reading fails G9's "no reading at the bound".
    - At W = 3, G10's T_K1 ≤ 17.1 min is 3,078 worker-seconds, and one bounded reading uses about 10% of it.
    - It adds about 5 s to a 55-try mean.
- **b3's j00006.**
  - Its kind of scope (several rooms, starting outside room 28) comes from b3's fixed job list, not from the run's scopes. The exception is an entry whose room is outside the guide's ring, which reads the whole ring (grow.rs:32-35).
  - If bounded, it fails b3's "no reading at the bound" (D11) at G8 and G13. By the stop rule, Run 1 then does not start.
  - G9 and G10 measure the run's own readings, so they still run.
  - What remains on j00006 needs research before any rule. Even B's upper bound reads 382,691 records, against 228,924 with wait_both off, so most of the remaining passes are not WaitForMessage's.
  - Nothing is capped in the meantime.
- **The share gate.**
  - With C1', the reading ahead in the gate is j00001's kind, so a queued try waits about 41 s, not 300 s (estimate).
  - C's deferral stays the measured, lossless next lever.
  - G9 reports each try's wait at the gate, from the log's times, as C measured it.

### C2b: a stopped reading stops with `shared_runs` too (no switch)

- **The defect** (from Stage 2's review). It is latent, because ON's `FMI_READ_SWITCHES` does not hold `shared_runs`.
  - A worker can wait in `SharedRuns::look` (reader shared.rs:62-65) for another worker's run of the same key, and while it waits it never sees the stop flag.
  - If that run's owner unwinds at `look_stop` (scumm machine.rs:1495) before `fill` (reader machine.rs:3059), the key stays Running, and the pool never joins.
- **The fix.**
  - The owner holds a guard while its run is made. On unwind, the guard fills the key with no run (`State::Done(None)`).
  - The waiter then takes `Look::Alone`, runs the click itself, and stops at its own `look_stop` within 4,096 steps.
  - A stopped reading's files are never used (bound.rs:198-200), so no result changes.
- **Gate.**
  - A test where an owner unwinds while another thread waits on its key: the waiter returns.
  - A stopped reading with `shared_runs` on joins, at 0 and 3 threads.
  - With `shared_runs` on, room 33's reading, not stopped, is byte-identical before and after the fix.

### Corrections the build found

- **The "151-594 s past the bound" was a queue, not runaway readings.**
  - j00013, j00014 and j00016 waited 152, 414 and 594 s at the share gate (`FMI_READ_SHARE`) before their own 300 s readings (C). The gate's lock is held through `read_fresh` (tryjob.rs:940-957), including the bounder's 300 s.
  - G2 measured the try itself ending 0.3 s past a 30 s bound, with and without C2. C2 cut how long the reading threads live after the try from 10.4 s to 1.1 s (29.8 to 0.9 CPU s).
  - So in Problem's K1 table, the "1,159.1 s past it" is time waiting at the gate. In C2's prediction row and in G2's threshold, "151-594 s" becomes "10.4 s of reading threads after the try".
- **The key-fact baseline is 5 on one chain, not 9.**
  - E012's kitchen holds 9 key facts across the archive: #1, #8, #15, #16, #19, #20, #21, #45 and #51.
  - At most 5 are on one chain (G1): entry 78, with #15, #16, #19, #21 and #51.
  - The "Now" for the prediction row "E001 key facts on the best verified chain" is 5.
- **P0 in the score.**
  - `ladder --score` counts P0, the root, as a rung: E012's kitchen scores 2 of 15 on entry 150's chain.
  - Rung counts for P1-P14 subtract it: the kitchen holds 1 of 14.
  - watch.sh and Run 1's reports give P1-P14.
- **Stage 1 review notes, to fix before K1a:**
  - watch.sh:63/:72: a failing ladder resets the no-progress clock. `held` takes the last seen value, the key facts are then appended to it again, and so it no longer equals `seen`.
  - ladder.rs:285: a null room.

### R3: decided (D11)

- **The reference.** b3 compares against a reference recorded once on the E026 build with every switch on (G13's build).
- **The rules.**
  - Each class change from E013's reference is listed with its switch.
  - New entries verify from boot with 0 differences.
  - No reading reaches the bound, the reference's own included.
- **G8** runs before every switch is on. It lists each class change from E013's with its switch (the 8 tries without REFINE_WIDEN still run), and applies the other rules.
- **G13** records the reference.
- "Report b3 both ways" is dropped.

### What changes in the plan above

- **Order of work, item 2:** C2, C2b and C1', then C3, C4 and C5. The gates are G2, G3', G4 and G5.
- **C1's section** stays as history. C1' replaces it.
- **Prediction table:**
  - "j00006 reading (C1)": the central is now bounded (B), and the range is ≥ 115 s (the rule's upper bound) up to bounded.
  - "A bounded try's time past its bound (C2)": Now is 10.4 s of reading threads after the try (G2's base), and the central is ≤ 2 s (G2: 1.1 s).
- **Unconfirmed inputs:**
  - "Whether every passing path the demo needs holds a sentence" is answered on room 48: no. The planner uses the passes that hold no sentence, but the two it played were wrong (A).
  - New: whether K1's other 6 room-28 starts read under the bound with C1'.
  - New: why B's rule left obj:582@0x62 undecided.
  - New: what the remaining passes on j00006 are.

## Amendment 2 (2026-11-10): after Amendment 1's check

File:line references in this amendment are at ab2e871 (branch `e026`) unless another tree is named. Engine references are to svm-e024's tree (`third_party/scummvm/engines/scumm/`). The sources are under `~/.cache/fast-mi-e026/build/`:
- **The check**, `amend1-check.md`, with its rerun in `/var/tmp/fast-mi/chk-c1p/peek-ar/`.
- **T**, `res-talk-read/`: the decided WaitForMessage, made right (scratch diff `talk-final.diff`).
- **J**, `res-j00006/`: b3's j00006 under wait_both (scratch diff `resj.diff`).
- **L**, `res-lazy-passes/`: read the passing paths only when a try needs them (scratch diff `lazy.diff`).

None of the research code is committed. The research ran on the project VM beside the other tracks' jobs (load 6-21). Records and CPU seconds compare across calls. Wall times compare only within one call.

### The check's verdict, and how each problem is resolved

The check found Amendment 1 not sound. Its engine facts held, G3'(a) was as strict as G3(b), and the numbers matched the files on disk.

| Problem the check found | Resolution |
|---|---|
| 1. The 40.8 s was B's fork variant (`INV_TALK=read`), not the specified no-fork rule. The no-fork rule with the dump's actor rooms gives 162,697 records, 198.8 s, 584.4 CPU s on j00001: no gain | C1'' specifies the fork form: the rooms are read by the machine's ordinary fact reads, at the wait. Its proof is below. Its numbers are T's measurements of that rule (`cl2`, and `cl` where the reading is the same) |
| 2. G3'(c) passes with no reduction (wait_both alone read j00001 in 189.3 s and 296.8 s) | G3''(c1) compares each start with wait_both, back to back. Its thresholds fail at the no-fork rule's 0.999 of wait_both's records |
| 3. Actor rooms in the start row break reuse and the share gate, or, left out of the diff, make reuse unsound | The reuse and share diff adds the same keys to the try's side, from the try's live dump. Measured: no reuse or share decision changes in K1 or play10 |
| 4. A try's start row comes from the live Obs, which carries the dump's actors, so the demo's readings change too | C1'' takes the actors from the live Obs for every reading of a try. With that start row, job-28025's reading was byte-identical. G3''(a2) re-reads the demo's shared readings |
| 5. Build obligations: any write that may reach var 3 ends the mark; restart and load clear it; one round is one frame, across delay and breakHere | Listed under C1'', each with a test |
| Minor: an RSS range, the wait_queued match, a contradiction | See Corrections |

### The work runs as three tracks

| Track | Branch | Worktree | Target dir | Items |
|---|---|---|---|---|
| A | `e026` | `/var/tmp/fast-mi/wt-e026` | `/var/tmp/fast-mi/target-e026` | research, this amendment, C2b, C1'' |
| B | `e026-cost` | `/var/tmp/fast-mi/wt-e026-cost` | `/var/tmp/fast-mi/target-e026-cost` | C3, C4, C5 |
| C | `e026-click` | `/var/tmp/fast-mi/wt-e026-click` | `/var/tmp/fast-mi/target-e026-click` | R1, R2, then L1-L4 |

- The tracks run at once. Each gates its own items.
- B and C merge into `e026` before G8. G8 and every later gate run on the merged build.
- Builds, readings, tries and gates run on the project VM through `scripts/vmrun.sh`, at the laptop's paths. A timed gate takes the whole VM (`-w 6`), so idle_ok and quiet_ok pass. Back-to-back comparisons run in one call.
- Laptop walls from before 10:55 UTC do not compare with the VM's.

### What the research found

**T: the talk rule, made right.**
- B's fork variant had four defects:
  - printEgo never reached the hook. The machine calls `op` only for Write, End and Deferred ops and for putActorInRoom under actor_rooms (scumm machine.rs:1523, :1525). printEgo's effect is None (heads.rs:73).
  - `p.round` is always 0 in the reader, which has no settle rounds (reader machine.rs:2793). So the mark never ended, not even at `queue_end`, though the engine runs other slots before a held sentence (scumm.cpp:3233, :3257).
  - No write to var 3, and no unknown write, ended the mark.
  - A print whose actor is a variable stayed undecided.
- T's decomposition reproduces B's count exactly (86,916: print only, at the talk, the dump's rooms).
- **Deciding at the talk forks at every talk.** On j00001, 55,164 talks land in "not in the room" branches: ego's room forks over {28, 33, 41}, the room over {0, 28, 33, 41, 79, 81, 82}. Deciding at the wait forks only where a talk meets a WaitForMessage.
- j00001, records:

  | Variant | Records |
  |---|---|
  | wait_both, today's start row | 162,827 |
  | print only, at the talk, the dump's rooms | 86,916 |
  | print and printEgo, at the talk, the dump's rooms | 119,743 |
  | print and printEgo, at the wait, the dump's rooms (C1'') | 87,491 |
  | C1'', today's start row | 171,747 |

- **The start row is part of the rule.** On today's row, C1'' reads 5% more records than wait_both. The at-the-talk form on today's row passed the reader's 10-minute cap (laptop, 1,576.3 CPU s, 2,376 MB).

**J: b3's j00006 (scope [28, 33, 35, 38], start in room 33).**
- No measured rule reads it under the bound. VM, 4 threads, B's `j6-in` start row, ON's switches:

  | Rule | Records | Read wall / CPU s | Result |
  |---|---|---|---|
  | wait_both off | 228,924 | 38.2 s / 132.7 | done |
  | wait_both | 53,343 in pass 2 | – | stopped at 30 min |
  | face and talk, no fork | 53,020 in pass 2 | – | stopped at 20 min |
  | face and talk, fork at the talk | 117,394 in pass 1 | – | stopped at 20 min |
  | Upper bound, not sound: every WaitForMessage after any talk only yields | 294,833 | 80.3 s / 294.2 | done |
  | Upper bound, not sound: talk, face, walk and camera | 231,268 | 45.5 s / 158.3 | done |

- Under wait_both, pass 2 stalls on room 28's dialog (`event(local:220@3627)`) and the dialog-choice inputs `sentence(v,442..445)[14]`.
- In pass 1, 156,043 of 221,885 records (70.3%) pass a wait. By what comes before the wait (records with a pass there): a walk 93,683; a talk 55,129; WaitForCamera with no camera op 45,695; faceActor 13,945.
- **The worst single op in pass 2** is the dialog verb script's printEgo then WaitForMessage (script:14@00FE): 96,424 of the talk upper bound's 105,531 dropped forks. The start row holds `actorroom:1 = 33`, while those inputs are read in room 28 or 35. Actor 3 is not listed.
- **Walks and the camera have no rule.**
  - startWalkActor leaves `_moving` unchanged for an actor outside the room or already at its destination (actor.cpp:871-877, :902-906). Positions are not facts of the reading.
  - The camera follows an actor's position (camera.cpp:93ff).
  - Face is lossless (faceToObject sets `_moving = MF_TURN`, actor.cpp:1678-1690), but its upper bound removes 22,028 forks and still stalls.
- **Dedup of continuations is too small.** In pass 1, 60,561 of the 116,063 passing forks' states are distinct (52%). An exact memo saves at most 48% of continuation runs, and no records.
- **The shape is b3's.** All 4,223 tries in K1 (18), E016's K1 (132), E016's kill run (21) and play9 (4,052) had one-room scopes, and none used the ring fallback (grow.rs:32-35). Several-room scopes occur only in E009's ring-scope archives (53 tries) and b3's fixed jobs (15).

**L: read the passing paths only when a try needs them.**
- R0 is the reading without wait_both, R1 today's. A try plans on R0 and reads R1 at its first attempt end that is not "target reached".
- It is sound. R0 is not an over-approximation (Lemma W), so every negative is drawn on R1 only. Positives are engine checkpoints.
- **It escalated in 9 of 9 tries.** Room 28's first target, `room=41`, has no plan on R0 at any of the 7 starts.
- Its cost: +9.9 s on the demo, +13.4 s on j00001 (2.4%), +105.1 s on j00006 before the same bounded R1.
- On the demo it does not keep wait_both's 5 kept entries. "Continue" misses 19,207 and adds three. "Restart" has 22,870 where today has 22,871; base-phase state leaks within the process (not pinned down).
- **Not adopted.** It reduces nothing on room 28, and j00006 stays bounded under it.

### C1'', the decided WaitForMessage, decided at the wait (replaces C1')

**The switch.**
- Reader switch `wait_talk`. It needs `wait_both` and `actor_rooms`, and the reader refuses it without them.
- Off by default. It is in the reading cache's key (step.rs:20-23) and the memo's key, through `--switches`.
- On in E026's env.sh for both games when G3'' passes: `FMI_READ_SWITCHES=value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost,wait_both,wait_talk`.

**At a talk.** A print or printEgo whose sub-ops include a text (SO_TEXTSTRING) is a talk. Its actor a is:
- for print, the path's value of the actor operand, & 0xFF;
- for printEgo, the path's value of `VAR_EGO`, & 0xFF.

Then, by a:
- not an int, 252-254, or not an actor number (1 to `scope::NUM_ACTORS`): nothing is recorded;
- 255: the path is **marked**;
- any other actor whose room is a non-fact now: nothing is recorded. A room is a non-fact when the path has not written ActorRoom(a) and the table holds it as NonFact or as the constant NOT_KNOWN (`room_may_decide`; it looks up the table without adding to the run's touched keys);
- any other actor: the path keeps a **pending talk**: a, the path's write-map entries for ActorRoom(a) and Room, its count of writes to `VAR_HAVE_MSG`, and its count of unknown writes.

**At a WaitForMessage in wait_both's `None` arm.**
- The op only yields (the blocking branch, `h_done "yield"`, as with wait_both off) when either:
  - a mark holds: both counts are unchanged since the talk;
  - or a pending talk decides: both counts are unchanged, ActorRoom(a) and Room have the same write-map entries as at the talk, and `m.read(ActorRoom(a))` and `m.read(Room)` give equal ints.
- Those reads are the machine's ordinary fact reads: a fork splits the path into value branches, or a precondition is recorded.
- On every other branch, the op takes wait_both's both ways.
- Every other WaitFor op is handled as wait_both handles it now.

**Lifetime.** Marks and pending talks live in the path's run state (SymX). They end at `queue_end`, at the run's end (a yield, an end, an unmodelled op) and when either count changes. Both are in the join signature.

**The start row.**
- Under `wait_talk`, every reading of a try gets `actorroom:a = room` for each actor its live dump lists (visible and in the current room; speedrun_state.cpp:629-635). This covers the first reading and any reach or refine reading at a later frame.
- The actors come from the same Obs as the reading's other start facts (tryjob.rs:562-563; step.rs:1197, `write_facts_ordered`). Actors not listed stay NOT_KNOWN.
- The reader keeps today's kinds: a listed actor's room is a constant, unless a scope write may reach it, in which case it is F9's fact.
- The planner's start takes the same keys (start.rs `actor_rooms_listed`, after `actor_rooms_start`; harness.rs).
- Play does not test `actorroom:` keys. They are unobserved and predicted (start.rs:43-60), as today under F9.

**Reuse and the share gate.**
- `choose_reading` (tryjob.rs:427-470) and the share gate (tryjob.rs:940) compare a reading's start facts with the try's live facts (tryjob.rs:883-887). Under `wait_talk`, the try's side gets the same `actorroom:` keys from its live dump (`with_actor_rooms`, before `choose_reading` and the share gate).
- Nothing else changes. Entry identity, novelty and the archive keep today's fact maps.
- A constant actor room must be equal on both sides. An actor listed on one side only is a differing key (Facts::diff counts an absent key as 0; fmi-core lib.rs:335-355), so that reading is not reused. That is conservative.
- A fact-kind actor room is a record precondition, which the planner tests against its start.
- The scratch's `reuse_of` change, which tolerates every `actorroom:` key, is dropped.
- **Measured on the archives** (T's `reuse_actors.py`). Every reuse and share had the same listed actors at the reading's start and at the try's entry:
  - K1: 8 of 8 reuses.
  - play10: 7,918 of 7,918 reuses (972 fresh; 2,377 reach and 4,569 refine, these two with the start entry approximated), and 46 of 46 registered shared readings. One shared reading is not registered, so it cannot be checked.
  - So no reuse or share decision would change in either archive.
- **Rejected designs.**
  - Actor rooms as entry facts: entries would split by actor visibility in both games, changing novelty, the archive and b3's reference.
  - Today's start row: +5% records (T).
  - Listed rooms made facts with a domain: sound, but adds forks for actors no scope write reaches, and no better on j00001 (T).

**Why it is lossless (D9).** It drops only passing branches the engine cannot take.
1. **The wait.** SO_WAIT_FOR_MESSAGE passes only when `VAR(VAR_HAVE_MSG)` is 0. Otherwise it restores the script pointer and breaks (script_v5.cpp:3316-3318, :3336-3337).
2. **A deciding talk sets it to 0xFF.**
   - o5_print and o5_printEgo (script_v5.cpp:2066, :2086) go to decodeParseString, which takes text slot 0 for every actor but 252-254 (:3474-3487).
   - SO_TEXTSTRING (:3596) reaches printString's slot 0, which calls actorTalk.
   - For actor 0xFF (actor.cpp:3492-3496), or an actor in the current room (:3502; `oldact` is then the actor's number, under 0x80, so :3521 does not return), actorTalk writes `VAR(VAR_HAVE_MSG) = 0xFF` (:3542).
   - An actor not in the current room returns at :3521 before that write, and the rule decides nothing for it.
3. **Nothing the path does not see writes var 3 between the talk and the wait in one run.**
   - Script writes go through `Machine::put` and `write` (scumm machine.rs:1239, :832). Known targets are counted, including indexed vars with a known index and every setVarRange element (:2147ff). An unknown target is an unknown write (:1244).
   - Nested scripts run inline, since the reader has calls and room_change on (reader machine.rs:2793), so their writes are on the path. An unknown callee is an unknown write (scumm machine.rs:1370, :1786, :1795, :1820, :1848).
   - actorTalk only writes 0xFF again. stopTalk writes var 3 only for DIG and CMI (actor.cpp:3599-3601). startScene's stopTalk (room.cpp:53) is the same function.
   - The per-frame copy runs before the scripts (scumm.cpp:3200, :3383-3384).
   - Restart (systemOps) is unmodelled (heads.rs:115) and saveLoadGame has no rule (heads.rs:116). The path ends there (scumm machine.rs:1518-1520), and its mark with it.
4. **Deciding at the wait gives the talk's answer.** The reads at the wait see the same ActorRoom(a) and Room as at the talk: their write-map entries are unchanged, and no unknown write happened. An unknown actor in putActorInRoom or loadRoomWithEgo is an unknown write (actor_rooms.rs:25-45, :50ff).
5. **The reads drop nothing.** A fork's branches together are the path. Branches where the actor is not in the room keep both ways. The non-fact filter is exact: `actor_room_value` (reader machine.rs ~1078) turns a NOT_KNOWN or no-domain room into getActorRoom's non-fact, and with no write since the talk, it reads the same at the wait.
6. **What it rests on: F9's actor-room model at the talk.** In v5 CD, `_room` is written within a run only by:
   - putActorInRoom (script_v5.cpp:2195-2203);
   - loadRoomWithEgo (:1875, :1891);
   - startScene's putActor into the new room (room.cpp:230, :261);
   - `Actor::putActor` (actor.cpp:1747).

   initActor clears it only in modes 1 and -1, at boot and restart (actor.cpp:186-188, scumm.cpp:2101). The one other write (script_v5.cpp:3017-3031) is GID_MONKEY_VGA only.

So at a decided wait, VAR_HAVE_MSG ≠ 0 on every engine execution the path stands for. The dropped branch holds no engine path. The reading keeps a subset of wait_both's passing paths that still holds every engine path (Lemma W).

**What a record carries.**
- No new effect. Var 3 is never written into a record (a mark lasts one run), and the reader already lists var 3 in TIME_VARS (table.rs:26).
- A decided wait records the yield, as with wait_both off.
- Where a read forked: `actorroom:a ∈ {v}` and `room ∈ {r}` in `pre`, on fact-kind keys only. A constant-kind actor room is the reading's start, like any constant.

**No game numbers.** `VAR_HAVE_MSG` and `VAR_EGO` (vars.cpp), actors 252-255 and `NUM_ACTORS` are the engine's.

**Build obligations.**
1. printEgo reaches the hooks only under the switch (machine `Switches.talk`, a dispatch arm beside putActorInRoom's). The hook returns no paths for it.
2. The count of writes to `VAR_HAVE_MSG` is kept in `Machine::write`, by the engine constant, not a literal 3.
   - Indexed writes with a known index and setVarRange go through `put` and `write`, so they are counted.
   - Unknown targets and unknown callees are unknown writes. Unknown writes from stopScript, doSentence, setClass and the like also end marks: conservative and sound.
   - Narrowing on a test's sides (scumm machine.rs ~1673) is not an engine write and is not counted.
3. The reader asserts `p.round == 0`, so one run is one frame. A run ends at breakHere, delay and delayVariable (heads.rs:34, scumm machine.rs:1511), at a blocked wait, at an end and at an unmodelled op.
4. Marks and pending talks end at `queue_end`, at the run's end and when either count changes. They stay across a nested callee's yield (it returns to the caller in the same frame) and on wait_both's passing branch (it continues in the same frame).
5. The fmi side keys three changes on `wait_talk` in `FMI_READ_SWITCHES`: the start row (`write_facts_ordered`), the plan start (`actor_rooms_listed`) and the reuse and share diff (`with_actor_rooms`).
6. One code path: off by default, on for both games in env.sh.

**Tests** (fake scripts, switch on unless stated).
- Only yields:
  - a print by actor 255, then a WaitForMessage in the same run;
  - a printEgo by an ego the path holds in the room.
- Ego's room a fact with two values, one of them the room: one branch only yields, the other takes both ways.
- Both ways kept:
  - a non-fact room; actors 252-254;
  - a talk, then breakHere, delay or delayVariable, then the wait;
  - a talk, then a held sentence (`queue_end`);
  - a talk, then a write to VAR_HAVE_MSG, an indexed write that may reach it, a setVarRange over it, or an unknown write;
  - a talk, then a write to ActorRoom(a) or Room;
  - systemOps or saveLoadGame between the talk and the wait (the path ends).
- obj:582@0x62's shape: a cutscene op, a printEgo and a WaitForMessage in one run.
- Reuse: a reading whose start lists an actor the try's dump does not list is not reused, and the other way round.
- Switch off: room 33's reading and the demo's job-28025 reading are byte-identical to ab2e871's.

**What it does not decide.** The demo's obj:582@0x62 (11 passing actions) stays undecided.
- With printEgo reaching the hook, ego's room is a non-fact in room 48's reading. All 5,688 of its talks are non-fact talks.
- F9 leaves the room a non-fact because it cannot work out which of the scope's `putActorInRoom(Var[182], VAR_ROOM)` and three `putActorInRoom(VAR_EGO, VAR_ROOM)` writes it (table.rs `actor_room_domains`, ~477-520).
- Deciding it needs F9 to work out that write. That is outside this rule.

**Measured** (T's `cl`; VM, wait_both then C1'' back to back for each start in one call, beside other jobs, load 6-14). Records / read CPU s:

| Start (entry) | wait_both, today's row | C1'', the dump's rooms | C1'' ÷ wait_both: records; CPU |
|---|---|---|---|
| j00001 (15) | 162,827 / 441.0 | 87,491 / 79.7 | 0.537; 0.181 |
| j00012 (53) | 162,826 / 438.1 | 87,490 / 85.7 | 0.537; 0.196 |
| j00013 (38) | 162,829 / 426.0 | 87,493 / 82.4 | 0.537; 0.193 |
| j00014 (39) | 162,832 / 432.7 | 87,496 / 85.4 | 0.537; 0.197 |
| j00016 (239) | 162,808 / 443.1 | 86,748 / 82.9 | 0.533; 0.187 |
| j00017 (244) | 162,596 / 457.3 | 86,690 / 80.0 | 0.533; 0.175 |
| j00018 (267) | 162,815 / 433.6 | 87,479 / 86.0 | 0.537; 0.198 |

- Read wall: 27.5-29.7 s against 143.9-155.9 s. Peak RSS: 649-697 MB against 1,903-2,217 MB.
- `cl` has no non-fact filter. With the filter (`cl2`, C1'' as specified), j00001's readings are the same: sha 9dac3ff3 with the dump's rooms (87,491 records; 44.6 s, 129.4 CPU s at load 12-17), and cf80cb2d on today's row. The other six starts were not re-read with the filter.
- **No quiet timing of C1''.** The only quiet timing is the laptop's, before the move: j00001 under wait_both 245.3 s / 722.3 CPU s / 1,800 MB, and under the at-the-talk form with the dump's rooms 70.5 s / 206.6 CPU s / 636 MB.
- **The demo's job-28025**, by `fmi try` under ON with the scratch build's start row (`cl2`):
  - reading packed sha f776e50d, 24,649 records, as without the rule;
  - attempts and kept entries as wait_both's, 5 kept.
  - Without the non-fact filter (`cl`), the reading was 24,691 records (sha 3b76c090), because 5,688 pending non-fact talks split joins. Its attempts and kept entries were still wait_both's.
  - The 46 shared readings were not re-read.
- **Tries.** The try under C1'' on room 28 is not measured.
  - j00001 under the at-the-talk form (not C1''): reading 60.7 s, 119,743 records; try 197 s. a00 and a01 "no plan"; a02 stopped at frame 17,230 on "the same empty window 2 times … no record's precondition holds"; 16 kept.
  - j00001 under wait_both (L's VM batch): try 553.2 s, 16 kept, driver peak 13.8 GB. Under MemoryMax=5G it was killed at 196 s, so whether a02's stop also happens without the rule is not known.

### G3'' (replaces G3')

- Each part runs in one `vmrun -w 6` call (quiet_ok passes), cold, under `systemd-run --user --scope -p MemoryMax=… -p MemorySwapMax=0`, with CPU seconds beside wall.
- ON is E026's env.sh. "With C1''" adds `wait_talk`.

| | What | Threshold |
|---|---|---|
| (a1) | The demo's job-28025 by `fmi try`, in this order: ON without C1'', with, without, with (MemoryMax=5G) | All four readings packed sha f776e50d. The same attempt events and kept entries in all four (compared as in G3). Reported: the wait_talk counters |
| (a2) | play10's 46 registered shared readings, each re-read from its registered request with the start row the build writes (actors from its start entry's checkpoint), without and with C1'' | Each pair packed byte-identical. A reading that cannot be re-read fails (a2) |
| (a3) | `reuse_actors.py`, copied into `run/`, on K1's and play10's archives, with the build's diff | No reuse or share decision changes (K1 8; play10 7,918 and 46) |
| (b) | b3's j00006 by `fmi try` on its job.json, without and with C1'' (MemoryMax=16G) | D11's rule: with C1'', the reading is not bounded. Reported: records, CPU s, wall, and the passes left by J's groups |
| (c1) | K1's 7 room-28 starts (j00001, 12, 13, 14, 16, 17, 18): the reading request the build's `fmi try` writes, read by reader-scope, without and then with C1'' for each start (MemoryMax=5G) | With C1'', at every start: records ≤ 0.6 × wait_both's; read CPU s ≤ 0.3 × wait_both's; read wall ≤ 0.3 × wait_both's and ≤ 60 s; not bounded. The build's start row is today's plus `actorroom:1 = 28` and `actorroom:3 = 28` |
| (c2) | j00001's whole try by `fmi try`, without and with C1'' (MemoryMax=16G) | Reported: wall, stops, kept entries, the driver's peak RSS (an input to G9's W) |
| (d) | `cargo test --locked`, switches off and on | All pass, the tests above included |

- **Why these thresholds.**
  - (c1) fails on a rule that reduces nothing: the no-fork rule the check reran gives 0.999 of wait_both's records (162,697 against 162,826). C1'' measured 0.533-0.537 of the records and 0.175-0.198 of the CPU.
  - (a1)-(a3) fail on any change to the demo's reading at room 48, its try, its shared readings, or a reuse or share decision.
- **Prediction** (estimates; measured inputs in brackets).
  - (a1) passes [`cl2` on job-28025: f776e50d, wait_both's attempts, 5 kept].
  - (a2) passes. This is the least supported: only job-28025 was re-read, and actor rooms in the start row alone changed j00001's packed reading (the check's ar-off).
  - (a3) passes [0 changed decisions].
  - (b) misses: bounded. [No at-the-wait form ran on j00006. The fork at the talk stalled in pass 1, with `sentence(8|10,322)[202]` taking 197-219 s each. The unsound talk upper bound, which drops every pass C1'' could drop, finishes in 80.3 s with 294,833 records.]
  - (c1) passes, at about 0.54 of the records and 0.2 of the CPU [the table above].
- **Stop rule.**
  - (a1), (a2) or (a3) differs: stop, and C1'' stays off. Report the differing records, and whether a played step used one (A's test).
  - (c1) misses at any start: stop, and C1'' stays off. Measure which talks that start leaves undecided.
  - (d) fails: fix it first.
  - (b) alone misses: C1'' goes on. It is lossless, and (a) and (c) are what it is for. The next section applies.

### j00006: it stays bounded

- No proven or lossless rule measured reads j00006 under the bound (J). C1'' is not measured on it; G3''(b) does that first.
- If G3''(b) misses, j00006 stays bounded:
  - b3 fails D11's "no reading at the bound" at G8 and G13, and **Run 1 does not start** (the stop rule).
  - Nothing is capped. D11 stands; this amendment does not change it.
  - The run's own readings do not have j00006's shape (see J), so G9 and G10 still run.
- **What the evidence points at.**
  - The stall is the talks. The unsound talk upper bound finishes in 80.3 s with every walk, face and camera pass still in.
  - The other passes have no rule (walks, the camera) or barely fire (face).
  - script:14@00FE alone is 96,424 of the upper bound's 105,531 dropped forks in pass 2. C1'' can decide it only if the reading holds ego's room for inputs read in rooms 28 and 35, while the start row holds room 33.
- **The research that comes next** (Track A; plan, prediction and stop rule written before code):
  1. **Ego's room after a room change inside a several-room scope.** The engine puts ego into the new room at startScene (room.cpp:230, :261), and loadRoomWithEgo writes it (script_v5.cpp:1875, :1891). The question is whether the reader's room change holds ActorRoom(VAR_EGO) as a fact on that path, so that C1'' decides script:14@00FE at its wait. Measure j00006's records and its passes at script:14@00FE against the upper bound.
  2. **Actor 3's room** for a reading started in room 33: local 204 writes 28 or 0 when room 28 is entered (Amendment 1, B). This is F9's domain.
  3. **Only if both leave it bounded:** J's control-state summary of continuations. It is lossless only with exact read sets, it still emits every record, and in J's count it would cut runs 29-76 times.
- Each step meets D9 as C1'' does. Nothing on j00006 is ordered or capped by hand.

### Corrections

- **Amendment 1's B table.** The "wait_both, five runs" row includes B's pass2_reuse run (309.8 s, 3,729 MB) and ar-off (the dump's start row, 219.3 s). Its peak RSS range leaves out pass2_reuse's 3,729 MB. Plain wait_both on today's row read j00001 alone in 189.3 s and 296.8 s.
- **wait_queued against wait_both.** "wait_queued's reading is wait_both's minus the passes on paths that hold no sentence" is not exact. A's match ran one way only. wait_queued has 16,666 pass-free actions against wait_both's 16,542, and event(script:59@783) has 1,615 actions against 1,507.
- **Amendment 1 contradicted itself on the demo's needs.** What is measured: without the passes that hold no sentence, the demo's try changes (G3(b)), because the planner uses them, though both it played were wrong (A). So the answer to "whether every passing path the demo needs holds a sentence" is no. A's "the demo needs the passes that hold a sentence" means only that wait_queued is wrong by G3's rule.
- **The wait's engine lines.** The block is at script_v5.cpp:3336-3337 (T's report gave :3341-3342).
- **Room 28's cost under the talk rule.** Amendment 1's estimates (about 330 s of K1's 2,659.3 s, a 233 s mean, about 41 s at the gate) rest on the fork variant's 40.8 s.
  - Re-estimated from the CPU ratio under C1'' on j00001 (0.181) and the laptop's quiet wait_both j00001 (245.3 s): about 44 s a reading on the laptop.
  - With Amendment 1's arithmetic (five fresh readings, plus three queued tries waiting one reading each), that is about 360 s of K1's room-28 term. Both figures are estimates; G9 measures.
- **Memory.** A room-28 try under today's ON peaks at 13.8 GB of driver RSS (L). C5's `FMI_READ_MAX_RSS_GB=6` covers the reading only. G9's W takes the driver's peak.

### What changes in the plan above

- **Order of work.** Items 2-3 and Part 4's build run at once as three tracks:
  - A: C2, C2b and C1'' (G2, G3'');
  - B: C3, C4 and C5 (G4, G5);
  - C: R1 and R2 (G6, G7), then L1-L4 (G11, G12).
  - B and C merge into `e026` before G8. Items 4, 6 and 7 follow on the merged build.
- **C1'** stays as history. C1'' replaces it, and G3'' replaces G3'.
- **Where jobs run.** Builds, readings, tries and gates run on the project VM through `scripts/vmrun.sh`. "Cost: Builds" and Amendment 1's walls are laptop figures. This amendment does not change where Run 1 runs.
- **Prediction table.**
  - "j00006 reading (C1)": Now, unfinished at 1,500 s under wait_both (A); C1'' not measured. Central: bounded (estimate). Range: about 80 s (the unsound talk upper bound, 80.3 s on the VM under load) up to bounded.
  - New row, "A room-28 reading under C1'', K1's 7 starts": Now, 86,690-87,496 records, 27.5-29.7 s on the VM under load (`cl`). Central: 44 s on the laptop (estimate). Range: 28-71 s (the VM's walls up to the laptop's at-the-talk form).
- **Unconfirmed inputs.**
  - Answered: whether K1's other 6 room-28 starts read under the bound with the talk rule. In scratch, yes: 27.5-29.7 s. G3''(c1) checks the build.
  - Answered: why B's rule left obj:582@0x62 undecided. printEgo never reached the hook; with it, ego's room is a non-fact in room 48's reading.
  - Answered: what the remaining passes on j00006 are (J's groups above).
  - New: C1'' on j00006 (G3''(b)).
  - New: whether play10's 46 shared readings stay byte-identical under C1'' (G3''(a2)).
  - New: the try under C1'' on room 28: kept entries, stops and peak memory; and whether j00001's a02 stop also happens without the rule.
  - New: whether the reader's room change holds ego's room as a fact (j00006's research).

## Amendment 2b (2026-11-10): after Amendment 2's checks

File:line references in this amendment are at 06ff134 (branch `e026`; its code is ab2e871's) unless another tree is named. Engine references are to svm-e024's tree (`third_party/scummvm/engines/scumm/`). The sources are under `~/.cache/fast-mi-e026/build/`:
- **The gates check**, `wf5/check-gates.md`, with its runs in `chk2-gates/` (three play10 readings: rooms 49, 63 and 82).
- **The proof check**, `wf5/check-proof.md`, with its runs in `chk2-proof/` (j00001 back to back; j00016 and j00017).
- **This amendment's scratch**, `amend2b/`: T's `talk-final.diff` plus the changes to C1'' below behind `TALK_MERGE=1` (variant `cl3`; the whole diff is `cl3.diff`), and one VM call (`measure.sh`, `vm-measure.log`, `runs/*/summary.json`).
- **The PI's decisions D12-D14** (`notes/decisions.md` at 06ff134).

None of the scratch code is committed. The scratch ran in one `vmrun -w 2` call, 14:26-14:54 UTC, beside the other tracks' jobs (load 0.2-5.6). Records compare across calls. CPU seconds and walls compare only within one call: the same reading (sha 9dac3ff3) took 79.7, 83.7 and 129.4 CPU s in three calls (the gates check).

### The checks' verdicts

- **The proof check: sound, with minor problems.** It rebuilt T's diff byte for byte. j00001's reading reproduced (87,491 records, sha 9dac3ff3), and with the switch off it was byte-identical to ab2e871's. The engine facts hold in svm-e024.
- **The gates check: not sound.**
  - C1'' changes the demo's readings outside room 48, and G3'' neither compares them nor measures their cost.
  - Reuse and the share gate are unsound for actor rooms of fact kind.
  - G3''(a2) cannot run, and (a3) cannot see a change.
  - No gate fails if a decided wait is wrong in the engine.

Every problem of both checks, and what resolves it:

**The gates check**

| # | Problem | What changes in C1'' | How G3'' covers it (restated below) |
|---|---|---|---|
| G-1 | The rule changes the demo's readings in other rooms: room 49 (41,241 → 40,847 records, 187 waits decided), room 63 (7,763 → 7,789, 0 decided: 1,807 pending talks split joins) and room 82 (1,109 decided). G3'' compared only job-28025, where nothing is decided, and the shared readings | **Where the rule reads nothing at a wait, it changes nothing.** No fact read at the talk; marks and pending talks stay out of the join signature, and a merge keeps one only where every merged path holds it; a look before the wait's reads. Measured below (`cl3`) | (a1) rebuilds every demo job that has a job.json on disk and read fresh in play10's own run (54), plus job-28025. (a2) reads each request the rebuilt tries write, with and without wait_talk: byte-identical wherever the rule decided no wait and made no read |
| G-2 | The prediction "(a2) passes" was very likely wrong: 8 of the 22 shared readings are room 49's, where the rule decides. G3''(a) must ask for byte-identity only where the rule decides nothing, and for engine evidence where it decides | As G-1 | (a2) as above; (e1) and (e2) check the engine where the rule decides |
| G-3 | No gate measures the demo's cost. The start row alone doubles room 82's reading (10,996 → 21,334 records); with the rule it reads 19,241 (1.75 times today's) | None | (f): every rebuilt demo request, today's wait_both against the start row plus the rule, back to back, with thresholds |
| G-4 | Reuse and share are unsound for actor rooms of fact kind. `reuse_of` (step.rs:517-523) passes a differing key that is a task key, and room 28's actors 1 and 3 are F9 facts. A reading whose start lists actor 3 is then reused by a try whose dump does not list it; the plan's start holds -1 for it (start.rs:292-302), which no record's precondition holds, so a "no plan" drawn there is not an over-approximation | **Under wait_talk, any differing `actorroom:` key blocks reuse and share, in both games.** `choose_reading` and `choose_reading_r1` (tryjob.rs:427-470, :504) never count an `actorroom:` key as a task key, the E009 keys included (:452); the share gate takes its candidates through `choose_reading` (tryjob.rs:940-947). The archives lose no reuse: every reuse and share had the same listed actors on both sides (K1 8 of 8; play10 7,918 of 7,918 and 46 of 46; T) | (d): tests on both a constant-kind and a fact-kind actor room, an actor listed on either side only, for a cached reading and for the share gate: never taken |
| G-5 | (a3) cannot see a change in the build's decisions: `reuse_actors.py` compares the archives' listed actors without running `reuse_of`, and approximates 88% of the reuses (reach and refine readings) | None | (a3) is dropped. (d) tests the build's rule. Each G3'' try's log lists the candidates it refused and their differing keys (`first`, step.rs:519-521); the report counts refusals on an `actorroom:` key |
| G-6 | (a2) cannot run and miscounts: play10 has 47 shared uses of 23 readings, 22 of them registered. None of these readings or requests is on disk | None | (a2) is restated over what is on disk ("What is on disk", below) |
| G-7 | No gate fails if a decided wait is wrong in the engine | reading.json counts the decided waits by site (`wait_talk: decided at U@O`) | (e1): an engine trace over route-004, movie-c9-run and Monkey Island's recorded chains, 0 passes allowed. (e2): play, 0 engine passes of a decided wait in any G3'' try |
| G-8 | The cross-reading memo (`FMI_READER_LIB=1`, `--memo-mem`) keys a stored run on the kind, domain and start of its touched keys only (reader memo.rs:1271-1278). The non-fact look (`room_may_decide`) was untracked, so a reading's bytes could depend on what the process read before | **The look is tracked.** It goes through `info` with tracking on (reader machine.rs:712-716): the actor room's kind, domain and start join the stored run's key, and no precondition is added. The actor looks at the talk are tracked the same way | (d): two readings in one process, the actor's room NOT_KNOWN in one and a constant in the other, in both orders: each byte-identical to its cold reading |
| G-9 | Minor: CPU seconds do not compare across calls; the actor range is 1 to 12, not to NUM_ACTORS; the switch is not in the try's reuse key (`cached_readings_with`, step.rs:703-730, and `choose_reading` never compare switches) | **Under wait_talk, a cached or shared reading is taken only when its reading.json `switches` (reader run/mod.rs:770) equal the try's `FMI_READ_SWITCHES`.** With the switch off nothing changes. The wording is fixed (Corrections) | Each G3'' run also gets its own fresh reading folder and reading cache |

**The proof check**

| # | Problem | Resolution |
|---|---|---|
| P-1 | Four of K1's room-28 starts (j00012, 13, 14, 18) were read only with T's `cl`, which lacks the non-fact filter | Read now with `cl2` (the rule as Amendment 2 specified it) and with `cl3` (the rule as amended here): see "Measured". G3''(c1) reads all 7 with the build |
| P-2 | Wording: the spec masks print's operand with `& 0xFF`; "1 to NUM_ACTORS" | **The mask is the engine's.** `_actorToPrintStrFor` is a byte (scumm.h:1337), so o5_print's assignment (script_v5.cpp:2082) truncates, as o5_printEgo's cast does (:2089). The check's "the engine keeps getVarOrDirectByte's full value" does not hold. A truncated value of 13-251 stops the engine in derefActor's error() (isValidActor needs id < _numActors: actor.cpp:2281-2296). The rule records nothing for 0 or 13-251, so it decides nothing there. **Actors are 1 to 12:** `scope::NUM_ACTORS` is 13, v5's _numActors (scope.rs:15) |
| P-3 | Wrong cite: script_v5.cpp:1891 is in loadRoomWithEgo's `version <= 4` branch | v5's writes of an actor's room within a run: putActorInRoom (script_v5.cpp:2195-2203); loadRoomWithEgo's putActor (:1875); startScene's putActor (room.cpp:230, under `version >= 5 && a` at :223; room.cpp:261, under `version 5-6`); and Actor::putActor (actor.cpp:1747). :1891 is dropped |
| P-4 | The scratch differs from the spec: (a) the builder refuses wait_talk only without actor_rooms, and the talk arm also runs under wait_queued alone; (b) the one-frame check `p.round == 0` is a debug_assert; (c) table.rs `kind()` makes start-given actor rooms facts whenever `extra` holds them | Build obligations: (a) the builder refuses wait_talk without wait_both or without actor_rooms, and the talk arm runs only under wait_talk; (b) the check holds in release builds: a talk on a path whose round is not 0 is an error of the reading; (c) no `given_rooms` at all: the kinds are today's. (a2) shows (c): where the rule reads nothing, the reading is byte-identical |
| P-5 | The demo's byte-identity at job-28025 says nothing about the rule (it decides 0 waits there), and no try has played a decided wait | (e2) plays them: the rebuilt demo tries in rooms 42, 49 and 82, where the rule decides (Measured), and j00001's try (c2) |
| P-6 | The label check is partial. Of the 162 labels missing under C1'' on j00001, 67 hold no record that passes a WaitForMessage (continuation events), and the actions file has no next events | (d2) completes it on the packed readings, which hold each record's end: every label C1'' lacks must be reached under wait_both only through records that pass a WaitForMessage the rule decides on some path, directly or along a chain of record ends. Threshold 0 |
| P-7 | A "no plan" at a decided wait rests on the planner's `actorroom:` values, which play never tests (`holds` skips them: start.rs:43-60, :286-288) | Kept, and stated as a limit. Two things bound it. Every reading's start row takes the listed actors from the live dump (observed in the current room); unlisted actors stay NOT_KNOWN and decide nothing until the path writes their room. And (e2) counts every played decided wait the engine passes (0 allowed). A negative drawn at a decided wait that no try plays rests on F9's writes, as F9's other negatives do |

### What changes in C1''

The switch, the rule's conditions and the proof stay as Amendment 2 wrote them. These are added, and they are build obligations with tests in G3''(d).

1. **Where the rule reads nothing at a wait, the reading is byte-identical** to the same request read without wait_talk. Three things give it:
   - **No fact read at the talk.** The actor is a constant operand, a local, or a variable the path wrote or the table holds as a constant (a tracked look, item 3). Any other actor operand keeps no talk: fewer decisions, both ways kept.
   - **Marks and pending talks stay out of the join signature** (reader machine.rs `join`, :1471). At a merge (`merge`, :1538) the merged path keeps a mark or a pending talk only when every merged path holds the same one with the same counts of var-3 and unknown writes, as the merge already keeps only the non-fact tests every merged path took. Dropping one only keeps both ways, so it is sound.
   - **A look before the wait's reads.** The reads of ActorRoom(a) and Room run only when their values on the path (its write, else the table's constant or domain) share a room. Otherwise the wait takes both ways with no read and no fork.
   - Without these, a pending talk split joins where nothing was decided (room 63: +26 records; job-28025 without the filter: +42), and a read at the wait forked actor rooms where no branch could decide.
2. **Reuse and share.** Under wait_talk, any differing `actorroom:` key blocks reuse and share (G-4), and the switch list must match (G-9).
3. **The memo.** Every look the rule makes (the non-fact filter, the actor at the talk, the values before the reads) goes through `info` with tracking on, so the cross-reading memo keys the stored run on those keys' kind, domain and start (G-8).
4. **The builder** refuses wait_talk without wait_both or without actor_rooms; the talk arm runs only under wait_talk; the one-frame check holds in release builds; no `given_rooms` (P-4).
5. **Counters** in reading.json: the decided waits by site (`wait_talk: decided at U@O`), the marks met, and the reads at a WaitForMessage. G3''(a2), (e2) and (f) read them.
6. **One code path:** off by default, on for both games in env.sh, no game numbers (the engine's VAR_HAVE_MSG, VAR_EGO, actors 252-255 and NUM_ACTORS only).

### Measured for this amendment (scratch, one VM call)

- **Variants**, one scratch reader-scope for all, ON's switches, the try's arguments (as chk2-gates' `chk.sh` and T's `read.sh`):
  - `wb`: wait_both on today's start row;
  - `wb-ar`: wait_both on the row plus the entry checkpoint's listed actors (C1'''s start row), no rule;
  - `cl2`: the rule as Amendment 2 specified it, on that row;
  - `cl3`: the rule as amended here, on that row.
- **The demo:** the 7 play10 readings on disk whose entry is known ("What is on disk"). Records / read CPU s (one call):

  | Reading | wb | wb-ar | cl2 | cl3 | cl3: waits decided; reads at a wait | cl3 byte-identical to wb-ar |
  |---|---|---|---|---|---|---|
  | 42-j08918 | 60,673 / 361.3 | 60,673 / 367.4 | 58,083 / 230.7 | 58,007 / 220.7 | 366; 0 | no: it decides |
  | 42-j08928 | 63,897 / 776.2 | 63,897 / 770.9 | 61,296 / 496.2 | 61,236 / 505.9 | 366; 0 | no: it decides |
  | 49-j08916 | 41,241 / 10.0 | 41,241 / 9.9 | 40,847 / 9.9 | 40,866 / 9.8 | 183; 201 | no: it decides |
  | 49-j08922 | 33,299 / 6.2 | 33,299 / 6.2 | 33,319 / 6.2 | 33,299 / 6.2 | 0; 0 | **yes** (30c9ffaa); cl2 differs |
  | 49-j08926 | 33,614 / 6.2 | 33,614 / 6.2 | 33,634 / 6.2 | 33,614 / 6.2 | 0; 0 | **yes** (8446322b); cl2 differs |
  | 63-j08924 | 7,763 / 1.1 | 7,763 / 1.1 | 7,789 / 1.1 | 7,763 / 1.1 | 0; 0 | **yes** (f68f0f00); cl2 differs |
  | 82-j08930 | 10,996 / 3.0 | 21,334 / 8.8 | 19,241 / 7.7 | 19,187 / 7.9 | 1,106; 1,106 | no: it decides |
  | All 7 | 251,483 / 1,164.0 | 261,821 / 1,170.5 | 254,209 / 758.0 | 253,972 / 757.8 | | |

  - **Where cl3 decided nothing and read nothing (3 readings), it is byte-identical** to the same row without the rule. cl2 differs on all 3: its pending talks split joins (+20, +20 and +26 records).
  - **It decides in rooms 42, 49 and 82.** Room 42's decisions are all marks by actor 255 (454 marks: 366 waits decided under cl3 against 454 under cl2, because a merge keeps a mark only where every merged path holds it).
  - **cl3 keeps no talk whose actor needs a fact read:** 8,018 and 7,035 talks in room 42, where cl2's non-fact filter dropped 7,479 and 6,568 anyway, and 3,473 in room 82, where cl2 kept 4,201 pending talks and cl3 1,117. Room 82 still decides 1,106 waits, against cl2's 1,109.
  - **Cost, start row plus rule against today's:** summed, 1.010 times the records and 0.651 times the CPU. Room 82 is the most: 1.745 times the records and 2.6 times the CPU (4.9 s more). There the start row alone reads 1.94 times today's records, and the rule takes back part of it.
  - The demo's room-42 readings are the demo's largest: 125.0-270.7 s read wall under wb at 3 threads, 77.2-175.6 s under cl3.
- **K1's 7 room-28 starts** (T's inputs, the dump's rooms). Records; cl3's read CPU s in this call:

  | Start (entry) | wait_both (T) | cl (T) | cl2 | cl3 | cl3 ÷ wait_both, records | cl3: decided; reads |
  |---|---|---|---|---|---|---|
  | j00001 (15) | 162,827 | 87,491 | 87,491 (proof check) | 84,987 / 75.0 | 0.522 | 6,173; 4,973 |
  | j00012 (53) | 162,826 | 87,490 | 87,490 | 84,986 / 73.2 | 0.522 | 6,173; 4,973 |
  | j00013 (38) | 162,829 | 87,493 | 87,493 | 84,989 / 72.1 | 0.522 | 6,173; 4,973 |
  | j00014 (39) | 162,832 | 87,496 | 87,496 | 84,992 / 73.7 | 0.522 | 6,173; 4,973 |
  | j00016 (239) | 162,808 | 86,748 | 86,748 (proof check) | 84,244 / 78.4 | 0.517 | 7,284; 4,694 |
  | j00017 (244) | 162,596 | 86,690 | 86,690 (proof check) | 84,186 / 72.4 | 0.518 | 6,173; 4,590 |
  | j00018 (267) | 162,815 | 87,479 | 87,479 | 84,975 / 76.8 | 0.522 | 6,173; 4,973 |

  - **P-1:** cl2 on j00012, 13, 14 and 18 reads exactly T's cl record counts.
  - **cl3 keeps room 28's gain and adds to it:** 0.517-0.522 of wait_both's records, against cl2's 0.533-0.537. In room 28 no talk needs a fact read for its actor. At j00001: 6,189 marks by actor 255, 35,510 pending talks, 4,973 reads at a wait, 6,173 waits decided.
  - cl3's read wall was 24.8-27.0 s and its peak RSS 643-705 MB, at load 4.0-5.6.
  - No wait_both reading ran in this call, so cl3 has no CPU ratio of its own. The proof check's within-call ratio for cl2 on j00001 was 0.19 (74.3 against 384.5 CPU s); cl3's 72.1-78.4 CPU s here are the same size (estimate: about 0.2).
- **Not exercised by the scratch:** the reuse and share rules (G-4, G-9) and the memo's tracking across readings (G-8). These are fmi-side or need one process to read twice; the build's tests (d) check them.

### What is on disk for the demo

- **Job files.** play10's own run (jobs j08752-j08932) left 180 job.json files in `~/.cache/fast-mi-e022/play10/jobs/`, each self-contained (the entry's events and checkpoint, the guide path, the scope). 54 of these jobs read fresh in play10: room 42 6, room 49 16, room 63 22, room 69 8, room 82 1, room 83 1. They also made 54 reach and 11 refine readings. job-28025 (room 48) is `~/.cache/fast-mi-e022/r48-data/job-28025.json`. play9's 4,052 job files (`/var/tmp/fast-mi/e016-archives/play9/jobs/`) cover rooms 49, 63 and 69 only.
- **Readings.** Only 8 play10 readings are on disk (`play10/readings/`): 42-j08912, 42-j08918, 42-j08928, 49-j08916, 49-j08922, 49-j08926, 63-j08924, 82-j08930. 42-j08912's job is in neither the log nor `jobs/`, so its entry's actors are unknown. The other 7 are this amendment's scratch inputs.
- **Not on disk:** the 22 registered shared readings and their jobs (j06570-j08080, in the removed `/var/tmp/fast-mi/e017-archives/play10`). A job made up from the log and the entry would not be the job that read them, so they are not rebuilt. The one other shared reading, 63-j08862, was made by j08862, one of the 54.
- **So G3'' rebuilds the demo from job files.** Each job runs by `fmi try` with a fresh reading folder (`jq '.reading_dir = FRESH | .readings = [] | .readings_room = [] | .readings_any = []'`), its own reading cache, plan cache and pool, as gate3.sh ran job-28025. Each request a try writes (fresh, reach and refine: start rows, slot locals, arguments) is kept and read again by reader-scope. The 22 shared readings are replaced by these 55 jobs' readings; the share gate's choice is the reuse rule, which (d) tests.

### G3'' restated (replaces Amendment 2's G3'' table)

- Each part runs in one vmrun call on the VM. The timed parts (c1) and (f) take `-w 6`, so quiet_ok passes; every reading runs under `systemd-run --user --scope -p MemoryMax=… -p MemorySwapMax=0`, with CPU seconds beside wall.
- ON is E026's env.sh (B's and C's switches merged). "With C1''" adds `wait_talk` to `FMI_READ_SWITCHES`. Every run gets a fresh reading folder and reading cache.
- The base build is the merged `e026` just before C1'''s commits. It is built on the VM beside the C1'' build.

| | What | Threshold |
|---|---|---|
| (a0) | **Switch off.** The C1'' build with wait_talk off against the base build, on the requests the base build's `fmi try` writes: K1's 6 room-33 model tries (G4's) and 7 room-28 starts, job-28025, and (a1)'s 55 jobs' fresh readings | Every pair byte-identical: the packed reading, and the start rows the fmi side writes |
| (a1) | **Rebuild the demo.** The 55 jobs ("What is on disk") by `fmi try` under ON with C1'', cold, 6 at a time | Every try ends with no guard kill. Reported: each try's stop, kept entries, reading records, the counters, and any reading at the bound |
| (a2) | **Identity where nothing is decided.** Every request (a1)'s tries wrote, read by reader-scope as written, without and then with wait_talk, back to back | Where the "with" reading decided no wait, met no mark and made no read at a WaitForMessage, the two packed readings are byte-identical: **0 pairs differ** |
| (a3) | job-28025, as G3 compared it: ON without C1'', with, without, with | The same attempt events and kept entries all four times, and all four readings packed sha f776e50d |
| (b) | b3's j00006 by `fmi try` on its job.json, without and with C1'' (MemoryMax=16G) | **Reported only** (D12): records, CPU s, wall, whether it is bounded, the passes left by J's groups. A bounded reading is listed as a known limit |
| (c1) | K1's 7 room-28 starts (j00001, 12, 13, 14, 16, 17, 18): the request the build's `fmi try` writes, read by reader-scope without and then with C1'' for each start (MemoryMax=5G) | With C1'', at every start: records ≤ 0.6 × wait_both's; read CPU s ≤ 0.3 × wait_both's; read wall ≤ 0.3 × wait_both's and ≤ 60 s; not bounded. The start row is today's plus `actorroom:1 = 28` and `actorroom:3 = 28` |
| (c2) | j00001's whole try by `fmi try`, without and with C1'' (MemoryMax=16G) | Reported: wall, stops, kept entries, the driver's peak RSS (G9's W). Its plays count in (e2) |
| (d) | `cargo test --locked`, switches off and on | All pass. The new tests are listed below |
| (d2) | **The label check.** For each (a2) pair and each (c1) start where the rule decided: every label the "with" reading lacks, followed in the "without" reading through its records and their ends (the packed readings) | C1'' adds no label, and **0 labels it lacks** are reached under wait_both by a record that does not pass a WaitForMessage the rule decided on some path, directly or along a chain of record ends |
| (e1) | **The engine, traced.** Patch 0011 (below) on route-004 and movie-c9-run (the demo), and on Monkey Island's recorded chains: E012's kitchen goal chain (`fmi explore verify` on `e012-archives/kitchen`, entry 150, to frame 18,416), and every new entry of K1's archive (`mi-checks-now/k1/archive`) and of b3's reference (`e013-int/b3f/all-fast`) (`fmi check verify-new`), each replayed from boot | **0 passes of a WaitForMessage after a talk that set VAR_HAVE_MSG in the same frame and the same top-level run**, with no script write of var 3 between them. **0 engine writes of var 3 inside a run** other than actorTalk's 0xFF. route-004 with the log on: 33 of 33 digests, end qf 55,555 (the patch only observes) |
| (e2) | **The engine, played.** Every attempt of (a1)'s 55 tries and of (c2)'s try with C1'' | **0 play results** "the wait did not block" (fmi-play check.rs:443, strict.rs:1471) or "left the records' path" at a WaitForMessage whose site the try's reading lists as decided |
| (f) | **The demo's cost.** Every request (a1)'s tries wrote, read twice back to back: today's (the start row without its `actorroom:` keys, ON's switches) and C1'''s (as written, with wait_talk) | Summed over all requests: records ≤ 1.1 × today's, read CPU s ≤ 1.0 × today's. Each request: records ≤ 2.5 × today's, and C1'''s read wall under 300 s (the bound). Reported by room, with the start row's share apart (the request as written, without wait_talk) |

**Patch 0011 (an instrument; off by default; for (e1) only).**
- `SPEEDRUN_TALK_LOG=PATH` writes one JSON line per event:
  - each actorTalk (actor.cpp:3474): frame, slot, script, code offset, `_actorToPrintStrFor`, the actor's `_room`, `_currentRoom`, and whether it reached `VAR(VAR_HAVE_MSG) = 0xFF` (:3542) or returned at :3521;
  - each SO_WAIT_FOR_MESSAGE (script_v5.cpp:3316-3318): frame, slot, script, offset, VAR_HAVE_MSG, passed or blocked;
  - each write of var 3: by writeVar (a script write), or at an engine site (actorTalk, stopTalk, scumm.cpp:3384's copy, any other `VAR(VAR_HAVE_MSG) =`);
  - each top-level run's start and end (the bridge's run stack, as `segment` keeps it: script.cpp's executeScript hooks).
- It observes only, as patch 0004's segment and explain do (Lemma O, E003 trace-validation.md). Its engine is built apart (`/var/tmp/fast-mi/svm-e026t` on the VM); `FMI_ENGINE_BIN` stays svm-e024 for everything else.
- The chains run through `fmi shorten verify` (the demo's routes), `fmi explore verify` and `fmi check verify-new` (Monkey Island's entries), with `FMI_ENGINE_BIN` a wrapper that sets the log path per launch, as E024's `video/engine-wrap.sh` sets the capture.
- `fmi-checks talks LOG...` counts, per game and chain: talks (0xFF written, not in the room, 252-254); WaitForMessage ops after a talk in the same top-level run (blocked, passed); engine writes of var 3 by site.

**New tests in (d)** (fake scripts, switch on unless stated).
- Identity: a talk with no WaitForMessage after it, and a talk whose actor's room cannot equal the room, give the reading without the switch, byte for byte.
- Joins: two paths, one with a mark and one without, join as without the switch; the merged path keeps no mark and its wait takes both ways. Two paths with the same mark keep it.
- No read at the talk: a print whose actor is a fact-kind variable keeps no talk and forks nothing.
- The memo: as in G-8, both orders.
- Reuse and share: as in G-4, on a constant-kind and a fact-kind actor room; a reading made under other switches is not taken.
- The builder refuses wait_talk without wait_both, and without actor_rooms.
- Amendment 2's tests (marks, pending talks, the ends at breakHere, delay, delayVariable, queue_end, var-3 and unknown writes, room writes, systemOps and saveLoadGame; obj:582@0x62's shape; switch off byte-identical).

**Why these thresholds.**
- (a2) is the problem of G-1 and G-2 made exact: identity is asked exactly where the rule did nothing, and the counters say where that is. (e1), (e2) and (d2) cover where it did something.
- (e1) tests the proof's engine steps (1-3) on every WaitForMessage the chains execute, in both games, with the engine's own values: a talk that wrote 0xFF, then a wait in the same run. (e2) tests the reader's decisions where the tries play them, the model's actor rooms included (P-7). (d2) tests that the decisions drop nothing else.
- (f): on the 7 readings, the start row plus the rule read 1.010 times today's records for 0.651 times its CPU, and 1.745 times at most (room 82, where the start row alone reads 1.94 times). The sums fail a rule that adds a tenth to the demo's records, or any CPU in all; they leave room for the rooms not measured (48, 69, 83), since room 42 is 1,137.5 of the sample's 1,164.0 CPU s and the rule cuts it to 0.64 of that. The per-request bound fails a room that blows up, which a sum would hide.
- (c1) is unchanged: it fails a rule that reduces nothing (the no-fork rule's 0.999 of wait_both's records).

**Prediction** (estimates; measured inputs in brackets).
- (a0) passes [the scratch reader with the switch off read j00001 byte-identical to ab2e871's: the proof check].
- (a2) passes [cl3 was byte-identical on 3 of 3 readings where it decided nothing and read nothing; cl2 differed on all 3].
- (a3) passes [cl2 on job-28025: f776e50d, G3's attempts, 5 kept; nothing is decided there, so cl3 reads it the same].
- (c1) passes at about 0.52 of the records [cl3: 0.517-0.522] and about 0.2 of the CPU [cl2's within-call 0.19; cl3 has no within-call ratio].
- (d2) passes [the proof check: C1'' adds no label on j00001; 95 of the 162 missing labels hold passing records, the other 67 are continuation events, not yet followed].
- (e1) passes [the engine facts were read twice, by T and by the proof check; no trace has run].
- (e2) passes [no decided wait has been played yet].
- (f) passes: about 1.01 times the records and 0.65 times the CPU, 1.75 times at most [the 7 readings]. Rooms 48, 69 and 83 are not measured.
- (b) bounded [J: no measured rule reads j00006 under the bound]; listed as a known limit.

**Cost** (estimates). The 55 tries, 6 at a time; each request read three times, room 42's at 1.3-4.5 min a reading [77.2-270.7 s in the scratch]; the base build, the C1'' build and the patched engine; the replays of (e1). About 3 h of VM wall in all.

**Stop rule.**
- (a0), (a2) or (a3) differs: stop; C1'' stays off. Report the differing records.
- (e1), (e2) or (d2) above 0: stop; C1'' stays off. Report the site, the run and the values.
- (f) misses: stop; C1'' stays off. Report by room whether the start row or the rule costs.
- (c1) misses at any start: stop; C1'' stays off. Measure which talks that start leaves undecided.
- (d) fails: fix it first.
- (b) stops nothing (D12).

### The PI's decisions D12-D14 and the gates

| Decision | What it says (notes/decisions.md) | Effect on the plan |
|---|---|---|
| D12 | b3's several-room jobs may reach the bound: j00006's [28, 33, 35, 38] and the [33, 38, 83, 85, 96] jobs. Each that does is listed as a known limit. Nothing is capped; the run's own readings keep the bound and its rules | **G3''(b) stops nothing:** j00006 is reported and listed. **G8 and G13, b3:** all 15 of b3's jobs are several-room (7 of [28, 33, 35, 38] from room 33, 8 of [33, 38, 83, 85, 96] from room 38; J), so "no reading at the bound" no longer applies to b3. Each b3 job whose reading reaches the bound is listed with its scope, records at the stop and wall, and does not fail G8 or G13. b3's other rules stand: new entries verify from boot with 0 differences, each class change from E013's is listed with its switch (a bounded reading's with D12), and the 8 tries without REFINE_WIDEN still run. **Smoke, G9 and G13's K1b keep "no reading at the bound"** (the run's own readings). Amendment 2's "j00006: it stays bounded" no longer holds Run 1; its research (ego's room after a room change) is off Run 1's path |
| D13 | G5's miss does not hold Run 1. C3 and C4 stay off, C5 is on, and the free tries' cost is measured in Run 1 and fixed in a fix round if it binds | **G5 failed** (K1 j00007 292.5 s alone on the VM, 287.8-296.8 s in review; 97.4% of its frames in the trials' waits; REJOIN saves 0; the budget changes nothing on j00007) **and stops nothing.** G9 and G10 report the free tries' share of K1a's wall apart. A miss of G9's mean try or of G10 that the free tries' share alone explains (the threshold is met with that share left out) is D13's case: reported, it does not hold Run 1. G9's other terms hold it as before |
| D14 | R1 stays off, G6 unchanged; R2 is on (G7 passed); L1-L4 start on `e026-click` | **G6 stays failed** (along the chain 12 of 14 with R1 off and 13 of 14 with it on; per job 14 of 14) and R1 stays off. G8's and G13's G4 runs with R1 off and R2 on. Track C's stage 2 is L1-L4 (G11, G12) |

### Corrections

- **Amendment 2's "Records and CPU seconds compare across calls"** is wrong for CPU seconds (G-9). Records compare across calls; CPU seconds and walls only within one call.
- **The spec's wording:** the actor's `& 0xFF` is the engine's own byte (P-2); actors are 1-12 (P-2); the v5 room writes are script_v5.cpp:1875, :2195-2203, room.cpp:230, :261 and actor.cpp:1747 (P-3).
- **The shared readings:** 47 shared uses of 23 readings, 22 of them registered, not "46 registered shared readings" (G-6).
- **The proof check's P-2** says the engine keeps print's full operand; scumm.h:1337 makes it a byte.

### What changes in the plan above

- **G3''** is the restated table above. Amendment 2's (a1)-(a3) are replaced by (a0)-(a3); its (b) stops nothing (D12); its (c1), (c2) and (d) stay; (d2), (e1), (e2) and (f) are new.
- **C1''** gets the changes listed above. Amendment 2's "Reuse and the share gate" is replaced by G-4's rule: under wait_talk an `actorroom:` key is never a task key for reuse or share, and the switch list must match.
- **G8 and G13, b3:** as D12's row. **G5:** failed, holds nothing (D13). **G6:** failed, R1 off (D14). **G9, G10:** the free tries' share reported apart (D13).
- **Order of work.** Track A: C2b and C1'' (G3'' as restated, with patch 0011 and `fmi-checks talks`). Track C: L1-L4 (D14). Track B merges as it is. G8 runs on the merged build.
- **Prediction table.**
  - "j00006 reading (C1)": a known limit (D12). Now bounded under wait_both (J: unfinished at 30 min); central bounded.
  - "K1 j00007 (C4)": Now 292.5 s alone on the VM, 57 kept (G5). Central 290 s, 57 kept: C4 is off (estimate, from G5's runs). Range 288-297 s (G5 and its review).
  - "G4 along the chain (R1)": Now 12 of 14 with R1 off (G6). Central 12 of 14: R1 is off (D14).
  - "A room-28 reading under C1'', K1's 7 starts": Now 84,186-84,992 records, 24.8-27.0 s read wall on the VM at load 4.0-5.6 (cl3). Central 27 s on the VM (estimate). Range 25-45 s (estimate: cl2 read j00001 in 44.6 s at load 12-17).
  - New row, "The demo's readings under C1'', against today's wait_both": Now, on 7 play10 readings, 1.010 times the records and 0.651 times the CPU, 1.745 times at most (cl3, one call). Central 1.0 times the records (estimate). Range 0.95-1.1 times (estimate; rooms 48, 69 and 83 not measured).
- **Unconfirmed inputs.**
  - Answered: whether the rule decides waits in the demo's other rooms. Yes, in rooms 42, 49 and 82 (Measured).
  - New: whether any engine run passes a WaitForMessage after a talk that set VAR_HAVE_MSG in the same run ((e1)).
  - New: the demo's cost under C1'' in rooms 48, 69 and 83, which no scratch reading covered ((f)).
  - New: how far the rebuilt tries follow play10's. G3'' compares the build with itself both ways, not with play10.
