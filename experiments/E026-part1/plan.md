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
