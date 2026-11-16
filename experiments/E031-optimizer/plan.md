# E031: the optimizer. The system turns its own route into the fastest route it can, glitches allowed

Date planned: 2026-11-16
Research questions it serves: RQ2 (optimisation), RQ3 (exploits)

- **RQ2.** How far can a general optimizer shorten a route the system found by itself, with every result replayed from boot?
- **RQ3.** Can general scans, with no game number in them, reach glitch states and price them? This is served in a limited sense only. The scan shapes (a key at each boot frame, a race against a running input bracket) were chosen by people who know the logo glitch and the item slide exist. A find shows that a general scan can locate the state. It does not show that the system discovered the class. The test of generality is the same scans, unchanged, on Monkey Island 2 and Fate of Atlantis (M8).

This plan replaces the first draft (`scratchpad/e031/plan.md`, stages 0 to 5, expected about 2 h 12 min). The draft timed routes in ticks at bit 565, kept full5's order, and left the item slide out. This plan times every route by the Any% rule, adds re-routing, and adds a glitch lane.

## Question

E027's run full5 finished The Secret of Monkey Island (DOS CD) from boot with the search alone. On the Any% clock it takes about 3 h 02 min. The search optimises for reaching the end, not for time.

Can a separate, fully autonomous optimizer turn that route into the fastest route it can, Any% style: every glitch allowed, walking optimised, re-routed where that is faster? How close does it come to the project's own hand-routed runs (28:01.4, 23:11.320, 21:01.483)?

The owner's target is to beat 21:01.483. The calculated estimate says this plan does not reach it: the model gives no draw under 21:01.483 in 150,000 x 150,000 (below its resolution, not a proof of impossibility: the model cannot see an unknown glitch or a better route order), and about 0.001% for under 30:00 with track 6 removed (see "Overall prediction"). The plan is still worth running for three results:

- a route verified from boot that is 2 to 4 times shorter than full5 (the model's 10th to 90th percentile is 46:39 to 1:37:04 with CD track 6 present);
- the measured tools any later re-routing needs: the clock, the state tables, the re-timer, the drop rounds, the learned table of what the end needs, the anomaly census;
- a measured answer to what the last factor of 2 to 3 needs (fewer progress steps, cheaper actions, the slide chain), instead of a guess.

## What exists

- **full5's route.** `experiments/E027-direct/results/full5/end-stop.jsonl`: 3,916 events = 1,994 clicks + 1,921 pointer moves + 1 skip switch. The bridge pressed 2,045 automatic skips (1,834 text, 211 cutscene). Settings in `results/full5/settings.md`: seed 1, talk speed 255, all CD tracks present, no key before first control. 624 room changes over 90 rooms (reference report, replay of today).
- **full5's tree.** `/var/tmp/fast-mi/direct/full5/leg00..leg03/nodes.jsonl` plus `edges.jsonl`, `offers.jsonl`: 58,276 distinct stops in 58,279 lines, 7,926 with children, depth up to 1,037, end path 1,038 stops. Each line holds parent, room, frame, level, news and the stop's own events. It holds no tick, no ego position, no facts, no digest and no RNG (graph report).
- **A replay trace of the route.** `/var/tmp/fast-mi/direct/video-full5/verify-out/eng/b0/` (trace.jsonl, frames).
- **The true end (the draft's finding, confirmed by replay).** The run reported its end at frame 114,978, tick 694,701 (3:12:58). Bit 565 first holds at frame 108,830, tick 657,813, qf 2,631,539. The 6,148 frames after it are not route time. The October Python controller on today's engine reaches the same frame and tick, with 6,901 of 6,902 timeline records equal to the direct binary's replay (reference report).
- **full5 does not reach the Any% end.** After bit 565 come a cutscene and two menus. The movie has no input after frame 108,580, and its replay sits in the last cutscene until the tick limit. In the hand route the final fade comes 40.44 s after bit 565 (reference report).
- **Where full5's time goes.** First control (frame 2,728, tick 16,321) to bit 565: 106,102 frames, 641,492 ticks, 6.046 ticks a frame.
  - 45% of it is waiting between a control return and the next input group: 48,553 frames in 1,415 groups (the draft, re-checked by its critique). Counting every idle frame with control: 55,261 frames, 51.7%; 53,777 of them are removable at one frame per idle run (execution report).
  - Player walking: 31,801 frames (30%). Input off: 16,084 frames (15.8%). Busy in all: 50,841 frames, 86 min.
  - By step on the end path: 257 progress trials hold 31,068 frames (20,970 without probe padding); 403 room-change steps with no gain hold 34,181; 377 in-room steps with no gain hold 40,853 (24,617 without padding) (graph report).
  - On the Any% clock, boot to bit 565, by class: waiting 5,526.3 s, walking 3,189.1 s, cutscene and text 2,091.9 s, acting 156.1 s; 10,965.2 s in all (`estimate.md`, table 1).
- **What the tree is good for.** Its alternative branches gain under 1% (4 of 257 segments have an earlier equal stop, 233 frames). Its useful content is the list of 257 progress trials and a table of measured walks: 16,660 room-change trials, 371 room pairs, 765 triples keyed by where ego entered. Direct walks in the found order would take 13,280 frames in place of 34,181. The 257 progress trials alone, with zero travel, take 20,970 frames: 35.2 min at 6 ticks a frame, about 29 min at 5 (graph report).
- **The anchors** (`paper/claims.md`, `docs/anypercent/overnight.md`; evaluation reference only):

  | Run | Time | Frame |
  |---|---|---|
  | Hand route, no item slide, track 6 present | 28:01.4 (1,681.413 s) | 19,859 |
  | Hand route, item slide, track 6 present | 23:11.320 (1,391.320 s) | 16,378 |
  | Same route, track 6 removed | 21:01.483 (1,261.483 s = 302,746 qf) | 14,820 |
  | Human record | 21:52.4 | |
  | Hand route at bit 565 (track 6 removed) | 20:21.044 | 14,336 |

- **Today's engine replays the reference.** `build/svm-direct` (patches 0001-0013) with the October controller gives frame 14,820, qf 303,846, 1,261.483 s; the trace's sha256 equals October's (reference report).
- **Engine costs measured on the laptop** (`high9 forkbench` at frame 92,860): fork 4.46 ms, one frame after a fork 1.37 ms, 0.045 ms per further frame, facts dump 0.79 ms, digest 0.18-0.27 ms, 52 MB per engine. A whole replay of the route is about 3 to 5 s.
- **Prior shortening work.** E024's `fmi-shorten` (forward pass, drops, twins, verification by two launches) shortened the Fate of Atlantis demo from 182,260 to 13,606 ticks. On a Monkey Island chain it stopped at step 5 of 12 on walking, and its drop rounds grow with the square of the step count. E025's walk fixes are designed and not built.
- **The reports and the estimate.** Four earlier reports and the draft's critique (`tasks/wzyb8mun6.output`), four new reports (graph, reference, glitches, execution), three designs with three judges, and the calculated estimate with its audit (`scratchpad/e031/estimate.py`, `estimate.md`).

## The metric

One timing rule, applied by one tool to every route: the hand references, full5, and every version the optimizer writes.

- **The rule.** The Any% rule that 21:01.483 was measured by (`rules/anypercent.md`, `scripts/anypercent_verify.py`): time = (qf at the end − qf at the start) / 239.992156448. qf is the bridge's count of quarter-frames. It counts every engine wait: each frame's length, and the fades and transitions outside the main loop.
- **Start.** The first key delivered in the logo room. If no key is delivered there, the frame that room ends (frame 961 in full5).
- **End.** The first frame of the final fade.
  - The optimizer finds the end by a rule with no game number: the first room change after the last input the game takes, after which Status shows user input off for 3,600 ticks (read each frame; the bridge's stall record is not used, because it is written only while a pending step has a blocker, which a raw-input replay does not set).
  - The tool checks it against the reference goal the 21:01.483 run used (room 0 and variable 293 equal to 161). That goal holds whichever final reply is chosen. It lives only in the evaluation tool. If the two disagree, the tool's frame is reported and the disagreement is logged as a bug. The check is a report only: it never accepts, rejects, ranks or steers a version, and the end rule is never tuned to it (that would carry the reference into the system). The end rule is fixed in M0 before the first comparison.
  - A route is valid only if the end flag holds first and the end rule fires after it. A route that merely changes rooms and stalls has no time.
- **The objective is this metric, whole.** Every candidate that changes boot, input timing or the start (M2 above all) is judged by (qf at end − qf at start) with the start rule applied to that candidate, not by qf from boot. Otherwise a boot key that moves the start would be credited or charged wrongly.
- **Raw input only.** The automatic skips are written out as key events and the route is replayed with automatic skips off. At most one key and one mouse event a frame.
- **Second number.** Ticks and frame at the first frame where the search's end flag holds (bit 565, passed as a run parameter, as in full5's settings). It links to E027's numbers (657,813 ticks at frame 108,830) and to the hand route's 20:21.044.
- **Two conditions, reported separately, never mixed.**
  - CD track 6 present (the game's files as shipped): compared with 23:11.320 and 28:01.4.
  - CD track 6 removed: compared with 21:01.483. Removing the track is a files condition, not a find. It shortens the last part's card from 130.004 s to 0.167 s. full5 pays the card twice (intro and last part), about 260 s.
  - Each route is re-timed under its own condition, because later inputs are frame-timed.
- **Recorded with every time.** Seed (1), talk speed (255), game folder hash, track-6 condition, engine build and patch list, the gate's result at the commit.
- **full5 on this rule** (the model's figure, measured in M0): 3:01:50.9 with track 6, 2:57:31.3 without, once the closing menus are answered.

## Hard constraints

1. **The discovery search does not change.** No edit to `fmi-direct`, `fmi-core`, `fmi-engine`, `fmi-explore` or patches 0001-0013. `experiments/E027-direct/run/gate.sh` gives PASS. high8 and high9 in `/var/tmp/fast-mi/direct/bench/bin` stay the paper's binaries (sha256 recorded and re-checked). The optimizer is separate code. It may run the frozen high9 as a subprocess and use the crates `fmi-engine`, `fmi-core`, `fmi-explore`, `fmi-shorten`.
2. **The system finds everything itself.** No hand-written move. No room, object, verb, script or variable number of the game in the optimizer's logic. No language model at run time. Every input of a kept route has a recorded source: an event of full5's movie (kept or moved in time), an event of a tree stop, an input the frozen search made, or an input made by a named general scan with its parameters.
3. **The known Any% route is evaluation reference only.** `routes/anypercent/`, `docs/anypercent/`, `runs/anypercent/` and `docs/human-route.md` are read only by the evaluation tools, never by the optimizer. This covers its driver, its flags, its setups and every number derived from them (a setup's cost, a window length, a threshold): none may become a parameter, threshold or scan range of the optimizer unless it is re-derived from full5's own data or recorded as a free run parameter swept over a stated range.
   - **Disclosure.** The experimenters know the reference. `estimate.py`, the choice of which milestones to build and the shape of scans A to D were made with that knowledge. They are planning and design choices, not run-time inputs: no component of `fmi-opt` reads `estimate.*`. The report and any paper text say so, and say which scans were shaped by known glitches.
   - The end flag (a bit number of full5's settings) is a run parameter of the frozen search's output, not optimizer logic.
4. **Every result is verified by replay from boot.** Model numbers, planned costs and fork results are never reported as results.
5. **Experiment discipline.** Plan, prediction and stop rule before code. Each milestone is a result.
6. **Compute.** Each run fits a few hours on the laptop (4 cores, 8 threads, shared). The cloud VM has about 8 dollars left: one run of about 6 h on 30 vCPU.
7. **No game dialogue or story text** in code comments, notes or reports.

## Architecture

### Spine and grafts

All three judges (feasibility, science, speedrun) chose the **movie-first** design. It is the spine: full5's movie is the route skeleton, and every pass moves inputs in time, deletes inputs, or replaces a stretch with inputs from the tree, the frozen search or a named scan. Every kept version is a real movie, so a late failure still leaves the earlier route.

Grafted, as the judges recommend:

- **The atlas** (graph-first): one walk of the whole tree with forks, reading the true state of every stop. It replaces the files' news, which is only a lower bound on the facts held.
- **The cell** (search-first): one equality used by every pass. A cell is (masked facts, room, entry token, ego position, anomaly signature). The earliest arrival at a cell wins.
- **The needs table** (graph-first): every failed drop is recorded with the keys whose loss broke the route. It is a first-class output: the game's dependency structure, learned from the engine.
- **Measured assumptions first** (all three): the executor on 30 trials, the direct walks, drop tests, before any re-routing build.
- **A small cost-ordered archive, late** (search-first): for re-ordering and for filling timer waits, only where loops and drops have stopped gaining.
- **Timer waits as release times** (graph-first): a wait's measured minimum becomes "at least T ticks after event E", which other needed work may fill.
- **The capped rescue by the frozen search**, and **tick debt with persistence profiles** for anomalies (both).

Where the judges or designs disagree:

| Point | Decision | Why, in one line |
|---|---|---|
| Cell dominance as a merge test only (feasibility) or a small archive (science, speedrun) | The cell key is shared from M0; the archive is built only at M6 | Two of three judges want it late, and the key costs nothing to share early. |
| Corridor cap: "a few", about 6, or the design's 10 | 6 per run | 6 runs of 15 min leave half of a 3 h run for the re-times. |
| Executor stop threshold: 0.5 or 0.6 | 0.5 | Two of three sources, and it is the low end of the model's range for `p_exec`. |
| Drop tests on the 257 progress trials: before M3 (speedrun) or at M5 (the design) | The single tests run in M0 as measurements; the rounds that keep drops stay at M5 | The tests cost 1 to 2.5 CPU-hours (10 to 25 min on 6 workers) and decide whether M5 is built. |
| Timer filling: inside the loop pass (speedrun) or later (science) | At M6, as a move of the archive | It needs the needs table that M5 produces. |
| A planner over a surrogate model (graph-first) | Not built | All three judges: the tree shows independence for 2% of pairs, and the engine alone decides what is valid. |
| Odds of under 60:00: the designs' 46 to 62% | The model's 44.6% (track 6) and 48.4% (no track 6) | The designs built on the first estimate; its audit found idle counted twice and draw repairs priced too easy. |

### Components

New code is one Rust crate, `fmi-opt` (binary `opt`), and one evaluation folder. Sizes are estimates.

| Component | What it does | Reuses | New |
|---|---|---|---|
| `anyclock` (evaluation, Python) | Times any movie by the metric above. Converts automatic skips to explicit keys. Replays in the Python host. Reports both track-6 conditions side by side. Holds the reference goal. | `scripts/anypercent_verify.py` (`replay()`, the qf rule, `explicit_movie()`: the explicit-key writer already exists, from the trace's `skip` records); the scratch wrappers `metric/time_movie.py`, `metric/replay_ref.py` | Parameters for seed, talk speed, game folder, start rule and goal in place of the script's constants; about 300 lines |
| `opt end` | From the end-flag stop: runs until input opens, lists the dialog lines the interface reports, tries each line and Esc at the first open frame, keeps the line whose end comes first, repeats on the second menu. | `fmi-shorten` chain.rs interface(); `fork_each` (host.rs:951); scan.rs | The menu scan and the end rule; about 200 lines |
| `opt table` | One replay of the current route. At every control return and group end: frame, tick, qf, frame length, RNG, digest, ego position, box, room, camera, masked facts as differences. Idle keys from a 200-frame idle fork. Three twins with a perturbed RNG per group mark it draw-dependent and name the group that consumes the draw. The frame log's anomaly baseline. | `fmi-shorten` chain.rs, label.rs twins, draws.rs; `Engine::digest()` (host.rs:1050); `SPEEDRUN_FRAME_LOG` | About 600 lines |
| `opt atlas` | The same reading for all 58,276 stops, by walking the tree: each stop is its parent's fork plus its own events. Adds the entry token (the room entered from, or the last object acted on). Builds the walk index: entry-keyed walks with their events, costs in qf, and the facts they were seen under. | `fmi-engine` fork, digest, facts; `fmi-explore` events.rs | A parallel walker on 6 workers, a store of differences (a full dump is about 400 kB a stop); about 800 lines |
| `opt cell` | The single equality: masked facts, room, entry token, ego position, anomaly signature (input counter against its rest value, cutscene depth with input on, frame length against its rest value, ego absent with input on). Digest equality is counted as an exact rejoin. | `fmi-shorten` verdict.rs (`ego_at`, conds) | About 300 lines |
| `opt ledger` | Maps every event to (input group, path trial, tree stop). Classes trials as progress, room change without gain, in-room without gain. Stores every version (`route-NNN.jsonl` plus steps, never overwritten) and each input's source. Refuses a version with an input of unknown source. | `fmi-shorten` movie.rs write, promote; the research scripts as specification | About 400 lines |
| `opt retime` | The forward pass. Each group's first input goes to control−2, −1, 0 or +1; inner offsets are kept, then compressed to one frame after the input before; pointer-parking moves between consecutive clicks are dropped. The earliest placement that passes the group verdict is kept. On failure: the padding ladder (1/8, 1/4, 1/2, the whole gap), then a delay scan at draw-marked groups judged at the consumer group, then a restore of the original gaps back to the last digest-equal control return. | `fmi-shorten` pass.rs, verdict.rs, draws.rs, scan.rs | Grouping by control return, the ladder, the delay repair, the restore; about 800 lines |
| `opt boot` | One raw key at each boot frame before first control, plus "skips on at frame f". Classes by digest at first control. The 50 earliest are probed over the first 50 groups; the best 3 get a whole re-time. Judged in qf only. | scan.rs; skips.rs's var-19 start check, copied into `fmi-opt` with the required value as a parameter (`fmi-shorten` itself stays as E024 left it) | About 300 lines |
| `opt surgery` | Loop cuts and drops over trials, in rounds: local tests against one base, combined by gain, re-timed end to end, split by ddmin on failure. Writes the needs table. | `fmi-shorten` reduce.rs; `opt retime`; `opt cell` | About 800 lines |
| `opt splice` | Replaces the travel between two progress groups: by a tree walk's own events; by re-placing clicks by identity where M0 measured that to work; by a frozen-search corridor. | The walk index; `fmi-shorten` label.rs (place, place_hit), refind.rs; `fmi-explore` afford.rs; high9 as a subprocess | The splice driver, the stop-file writer for `--from`; about 900 lines |
| `opt archive` | The small cost-ordered archive of M6: earliest arrival per cell, layers in the route's order, re-expansion on an earlier arrival. | `fmi-engine` pool.rs; `opt cell` | About 600 lines |
| `opt tight` | Exit pixels, right clicks, re-clicks during walks, approach clicks, luck by delay. | scan.rs; E025's F1-F3 designs (the known Any% driver is not a specification) | About 700 lines |
| `opt anomaly` | Census, scans A to D, persistence profiles, skip-ahead matching, the debt rule. | `SPEEDRUN_FRAME_LOG`; Status fields; `CondKind::Var`; the state dump's slot blocks | About 900 lines |
| Driver | Runs each milestone as one bounded job with the heavy lock, the inhibit lock and a watcher. Writes results and lab notes. | `scripts/vmrun.sh`, E027's `env.sh`, E024's video tool | About 400 lines |

### Data flow

1. full5's movie and tree, and the game's files, go in. Nothing else does.
2. `anyclock` and `opt end` give the route a real Any% end and time.
3. `opt table` and `opt atlas` read the true state once. `opt ledger` ties every event to its source.
4. Each pass reads the current route and its table, and writes a new `route-NNN.jsonl` with its step log.
5. Every version goes through verification (below). Only then does it become the base of the next pass.
6. After each milestone the measured values replace assumptions in `estimate.py`, and it is re-run.

The optimizer judges candidates by qf at the system's end, read from Status. Local tests judge at the next control return. E024 measured that ticks miss about 0.33 s at each room change, so ticks are reported but never the objective.

### Where the code lives

- `experiments/E011-speed/rs/fmi-opt/`: the new crate, a workspace member that is not a default member (as `fmi-direct` is). It depends on `fmi-core`, `fmi-engine`, `fmi-explore`, `fmi-shorten`. `fmi-direct` does not depend on it.
- `experiments/E031-optimizer/`: `plan.md` (this file), `estimate.py` and `estimate.md` (committed beside the plan and re-run after every milestone), `eval/` (anyclock, the reference comparison; the only code that may read the reference runs), `run/` (run scripts), `results/<milestone>/` (movie, time.json, settings.md, run log).
- Lab notes: `notes/log/` on each milestone's day; `report.md` when the experiment stops.

### Verification of every version

1. **Ledger.** Every input has a source. Otherwise the version is refused.
2. **Explicit keys.** Automatic skips become key events; skips off; at most one key and one mouse event a frame.
3. **Two hosts from boot, each checking what it can report.** (a) The optimizer's own replay (`fmi-engine`, as in `opt table`): frame, qf and `Engine::digest()` at every control return must equal the pass's recorded table. (b) The October Python controller on `build/svm-direct` (`anyclock`): the same end frame and the same qf. It has no digest command, so digests are compared on the Rust side only. (c) The frozen `high9 verify` on a one-line stop file (fields `frame`, `events`, `news`, `ever`; no `movie: own`): the end flag set at its frame B and unset at B−1. It prints frame, tick, room and the asked keys, not qf or digests, and it boots with seed 1 only.
4. **The time.** `anyclock`, both track-6 conditions, each route re-timed under its own condition. Acceptance uses the end flag and the end rule only; the evaluation goal inside `anyclock` is reported and never decides.
5. **Fork check.** Every fork result used in a kept version is checked by a plain replay from the previous kept stop (E025's fork label clash has no known cause).
6. A version its pass accepted that fails from boot stops everything until the cause is found.

## Milestones

Every milestone's route is verified as in "Verification of every version"; a milestone lists only its extra checks. Each prediction below is the model's number for that mechanism (`estimate.md`, tables 4 to 6), or says why it differs. "After" is the model's centre if this milestone and every one before it succeed. The model's staircase applies the logo glitch last; here it comes at M2, so the rows from M2 on are at frame length 5. They were computed from `estimate.py`'s own functions by `experiments/E031-optimizer/stairs.py` (to be folded into `estimate.py` as a table in M0). The final row equals the model's.

| Milestone | Model's mechanism, seconds removed (centre; range) | After, track 6 | After, no track 6 | p(step), model | Build days | Run hours |
|---|---|---|---|---|---|---|
| M0 clock, end, tables, measurements | closing menus: +40.3 | 3:01:50.9 | 2:57:31.3 | 97% | 6 | 2 |
| M1 re-timing | 5,242.7 (4,838 to 5,397) | 1:34:28.6 | 1:30:09.0 | 97% | 3 | 1 |
| M2 boot: intro skip | 167.3 (37.4 without track 6) | 1:31:41.3 | 1:29:31.5 | 97% | 2 | 1.5 |
| M2 boot: logo frame length | 642 on this route (334.6 on the final route) | 1:20:59.1 | 1:18:49.3 | 80% | | |
| M3 loops and no-gain drops, no executor | 583 at frame length 6, 532 at 5 (162 to 1,458) | 1:12:07.1 | 1:09:57.3 | none in the model | 3 | 3 |
| M4 direct walks | 1,352 (902 to 1,728), M3's cuts counted inside | 1:00:25.1 | 58:15.4 | 75% | 4 | 3 |
| M5 drops with re-placement; unneeded progress | 676 (373 to 1,050) and 174 (29 to 376) | 47:29.7 | 45:19.9 | 85% | 2 | 3 |
| M6 order and timer filling | 122 (20 to 364) | 45:38.9 | 43:29.1 | 45% | 4 | 3 |
| M7 tightening and luck | in-visit 70; interactions +79; luck 37.5; tuning 28.3; walking 22.3 | 44:19.9 | 42:10.1 | 60% walk stage, 85% delay scan | 3 | 3 |
| M8 glitch lane | head and necklace 6.5 (30%); feather path 5.0 (24.5%); whole slide chain 148 (0.0377%) | 44:08.4 | 41:58.6 | see "Glitch discovery" | 5 | 4 |
| M9 clean-room run | none | the same | the same | 85% (the designs) | 1 | about 21 |

All route-side steps succeed together in 12.6% of cases at the centre, and with the draw points 3.73% (`estimate.md`, table 6). That is why the model's median (1:03:19.9) lies between the M3 and M4 rows, not at the bottom.

### M0: the clock, the true end, the tables, and the estimate's top assumptions measured

**What the system does by itself**

- **Part A, clock and end.** Times full5 by the metric. Converts its 2,045 automatic skips to explicit keys and replays with skips off. Answers the two closing menus by `opt end` and finds the end by its own rule.
- **Part B, tables.** `opt table` on the path, `opt atlas` on the whole tree, the cell key, the anomaly baseline. Also the identity test: every group placed at its original offset from its control return must give full5's digests at all 1,485 control returns.
- **Part C, measurements.** A minimal forward placer (each group at control+1, the group verdict, no repair) is the probe harness. It is the core of M1's re-timer. With it the system measures the estimate's top assumptions:

  | Assumption (model's range) | Measurement in M0 | Cost |
  |---|---|---|
  | `p_exec` (0.5, 0.75, 0.9): the model's probability that the executor works | 30 of the route's trials, each after a direct walk from another room: (i) the movie's own raw events when ego enters by the movie's entry; (ii) clicks re-placed by identity with the existing `label.rs` and `refind.rs` code when it does not. The measured value is a rate, `r_exec` = trials remade / 30. The executor counts as working when `r_exec` ≥ 0.5 (the stop threshold above): the re-run sets `p_exec` to 1 and scales `v_walk` by `r_exec`; below 0.5 it sets `p_exec` to 0 | under a minute of forks |
  | `v_walk` (0.6, 0.9, 1.15): share of the direct-walk pool realised | each of the route's about 200 direct walks checked once: replay the tree walk's events from the route's stop; does ego reach the room by the expected entry, and in how many qf | about 3 min |
  | `phi_fb` (0.05, 0.18, 0.45): re-routing by raw cuts | count of cell-equal and digest-equal pairs on the path; 30 loop cuts placed forward to the next progress group | seconds, then about 1 min |
  | `q_atom` (0.2, 0.5, 0.85) and `p_drop` | 50 single drop tests on no-news steps that changed a world atom, by the same harness as the next row; wall time of the round projected to the route | about 30 CPU-min, 5 min on 6 workers |
  | `y_need` (0.05, 0.3, 0.65) | 257 single drop tests of the progress trials with the minimal placer, each stopped at its first failing group or at the end flag; each failure records the keys that differed. A failure is counted as "needed" only when the differing keys include a key the dropped trial set and the failing group is not draw-marked; otherwise it is "undecided" (the placer has no repair at M0), and `y_need` is reported as a bracket over the undecided ones | 1 to 2.5 CPU-h (a whole placement is about 1,485 groups × 25 ms ≈ 37 s; failing tests stop early), 10 to 25 min on 6 workers |
  | `r_xwait` (0.6, 0.85, 0.97), `r_inner` (0.8, 0.95, 0.99) | the padding ladder on the 33 wait trials; idle frames left per group on the first 100 groups | seconds |
  | `p_logo_surv` (0.5, 0.8, 0.95), `u_frame` (0.05, 0.35, 0.9) | the first 50 groups placed twice, with and without a boot key that changes the frame length. That key comes from a one-key (Esc) scan of the 960 logo-room frames, judged by qf only (about 1 CPU-min; M2's full scan replaces it); qf compared by class | about 2 min |
  | `q_search` (0.5, 0.8, 0.95) | one corridor of the frozen high9 (`--stops N`, 15 min cap) from a shifted stop at the draw-marked stretch with the most visits. One corridor is one draw: it shows that the mechanism runs and what it costs, not the rate. `q_search` keeps the model's range until M4's corridors (up to 6) measure it | 6 to 15 min |
  | `q_delay` (0.375, 0.59, 0.83), `n_dp` (7, 11, 19) | the twins' count of draw points; a 13-delay scan with look-ahead to the consumer group at up to 6 of them | under 5 min |
  | `p_end` (0.9, 0.97, 0.995) | Part A itself | |

**Deliverable**

- full5's Any% time in both conditions, with the system's own end. `anyclock` also reproduces 21:01.483 and 23:11.320 from the reference movies.
- The state table and the atlas, with exact counts in place of the file proxies: digest-equal pairs, cell-equal pairs, steps that change hidden atoms, draw-marked groups and their consumers.
- `estimate.py` re-run with the measured values in place of the assumptions, and `estimate.md` updated. Both committed.
- A decision table for M3 to M6 (below, "Gate").

**Verified by** the two-host replay of full5 with explicit keys; the identity test; `gate.sh` PASS; the sha256 of high8 and high9 recorded.

**Prediction**

- full5: 3:01:50.9 with track 6, 2:57:31.3 without (model). End flag at frame 108,830, tick 657,813 (already measured).
- The menus are answered and the fade is reached: 97% (model, `p_end`).
- The system's end frame equals the evaluation goal's frame: 85% (the plan's judgment; the risk is a looping line).
- Fewer than 10 digest-equal pairs on the path (the graph report expects this: 200 of 200 idle frames differed in E013).
- The anomaly baseline as the glitch report measured it: 6 (input counter, cutscene depth) pairs, 75 frames with the frame length below its rest value, 0 frames with ego absent and input on, 0 warnings.
- `r_exec` measured at 0.5 or more: about 72% (the model's mean for `p_exec`, the chance the executor works; E024 stopped at step 5 of 12 on exactly this). At 0.75 or more: 60% (the search-first design's figure).

**Build** 6 days. **Run** 2 h (clock replays about 25 min, atlas about 6 min wall on 6 workers (about 30 CPU-min: 58,275 stops, 536 frames each on average, 78 median), twins 80 s, probes about 45 min, the 307 drop tests 15 to 30 min).

**Gate (decides what is built next)**

- `r_exec` under 0.5: re-placement by identity is not built; M4 uses raw splices and corridors only; M5's second drop round is dropped.
- `v_walk` under 0.6: M4 is limited to the 20 longest stretches.
- The cell-equal pairs on the path are worth under 162 s in all (the model's low end): M3's loop part is dropped and only its no-gain drops run.
- No boot key changes the first 50 groups' qf: M2 keeps the intro skip only.
- Cost gate G4 (see "Gates") from the measured unit costs.

**Stop rule**

- The identity test fails: stop and find the non-determinism before anything else.
- The explicit-key replay of full5 (skips off) does not reach the end flag, or differs from the automatic-skip replay at a control return (other than through the dropped pointer-parking moves, shown by a test that drops them alone): stop; the clock rule cannot be applied until the conversion is fixed.
- A measured `p_exec`, `v_walk` or drop rate is a sample (30 trials: a 95% interval of about plus or minus 0.18). A value that would trip a gate below is first re-measured on 100 trials drawn by a recorded seed (never hand-picked) before the gate acts.
- The end solver finds no line that reaches an end: every result is timed at the end flag against 20:21.044, and the reports say so. The plan goes on.
- The re-run estimate's median is over 1:30:00 with track 6: stop and show the owner before M3.

### M1: the re-timed movie

**What the system does by itself.** One forward pass over the 1,485 control returns with `opt retime`: earliest placement, spacing inside a group, the ladder, the delay repair, the restore. Every 50 accepted groups it replays to the next draw-marked group and compares the cell and that group's draw keys; a failure is split by ddmin. The whole route is checked once at the pass's end.

**Deliverable.** route-001, verified, both conditions. Report: share of the 53,777-frame idle pool removed; groups repaired by delay; groups restored, with their room, the keys that differed and whether the RNG moves per frame there; unit costs.

**Prediction**

- Model: re-timing removes 5,242.7 s (4,838 to 5,397). That is 96.4% of full5's waiting above the reference's. The pools: 3,378.9 s after probes and calm (all of it goes), 1,254.3 s between one trial's clicks (95% goes), 893.1 s of explicit waits (85% goes). Route after: 1:34:28.6 with track 6 (1:31:54 to 1:41:13), 1:30:09.0 without.
- That centre assumes every draw point is solved again. At M1 only the delay repair exists. With the model's own parameters (`q_delay` 0.59, `n_dp` 11, `pen_draw` 120 s) about 4.5 points stay unsolved, each left as full5 played it: about 9 min more. Expected at M1: about 1:43 with track 6, 1:39 without. M4's corridors win most of that back.
- A verified route at the pass's end: 97% (model, `p_retime`; the plan's judgment is 85%, because E024's forward pass stopped at step 5 of 12 on a Monkey Island chain and one unrepairable group stops this pass). At or under 1:45 with track 6: 75%. At or under 1:38: 50% (the plan's judgment; the design's author gave 65% for 1:40).
- At least one group fails at a draw: near certain. The first is most likely at the store's first entry (frame 3,081 in full5), where a draw is stored outside the recorded keys and used 17,650 frames later.

**Verified by** the standard verification, plus: the route's click identities equal full5's, input for input, in the input log.

**Build** 3 days. **Run** 1 h. **Gate** G4: at most 40 ms a placed group, 2 min a pass without repairs, 30 min with them.

**Stop rule.** The pass ends over 2:00:00 with track 6: stop the experiment and write up; every later milestone depends on the re-timer. A group that cannot be repaired even by the restore: the pass stops there, the group and its cause are recorded, and the fix is made offline. No hand patch.

### M2: the boot. Intro skip and frame length

**What the system does by itself.** `opt boot` scans one raw key at each boot frame before first control (keys taken from the engine's key handling and the scripts' key reads), each followed by "skips on at the next frame", plus "no key, skips on at frame f". Candidates are classed by digest at first control. The 50 earliest classes are probed by placing the first 50 groups. The best 3 get a whole re-time. The kept boot is the one whose route ends earliest, verified. The system never names the frame-length variable; it sees only qf. Keys that already differ at first control are left out of the group verdict for that route. Under a changed frame length the re-timer re-places inner offsets too.

**Deliverable.** route-002: the kept boot inputs in explicit-key form and the whole route re-timed under them. The first measured pair of one route with and without the changed frame length (the project has none; `paper/claims.md` lists the window only as a probe).

**Prediction**

- Intro skip: 167.3 s with track 6, 37.4 s without; 97% (model, `p_intro`). One Esc at boot frame 46 already reached the game under the search's settings (reference report).
- Frame length 5: the scan finds it (47 of 960 logo frames hit; the model's coverage and signal are 1). The route survives a whole re-time at it: 80% (model, `p_logo_surv`; the design's author gave 65%). M0's 50-group probe replaces this number before the run.
- Saving at this point: about 642 s, by the model's rule (walking × 1/6, other busy time × `u_frame`/6) applied to the re-timed route. The model's 334.6 s is the same rule on the final, shorter route.
- Route after: 1:20:59.1 with track 6, 1:18:49.3 without. Without the frame length: 1:31:41.3 and 1:29:31.5.

**Verified by** the standard verification of both routes of the pair (with and without the kept boot key), in explicit-key form.

**Build** 2 days. **Run** 1.5 h (at most 25,000 forks, 30 CPU-min; then up to 3 whole re-times).

**Gate.** G4's re-time cost holds at frame length 5.

**Stop rule.** If no boot class with a lower probe qf survives a whole re-time, the best verified boot stands (at worst full5's own) and the plan goes on at frame length 6.

### M3: loops and no-gain drops, with no executor

**What the system does by itself.** `opt surgery` deletes movie events only. Candidates, in order of the frames they would save:

1. Exact cuts: two control returns with equal digests.
2. Loops: two control returns in the same cell (masked facts, room, entry token, ego position). Chiefly bounces between two rooms and repeated cycles.
3. Single trials with no gain, then ddmin halves, quarters and eighths over runs of them.
4. Repeats inside a trial (count repeats; repeated sentences).

Each candidate is tested locally: cut, place forward to the next progress group, rejoin by the cell and the draw keys. Each round uses one base: local passes are combined in order of gain, re-timed end to end, and split by ddmin on failure. This avoids E024's square law. A failed drop goes into the needs table with the keys that differed.

**Deliverable.** route-003 and later; the share of the loop pool realised; the needs table's first entries.

**Prediction**

- Model: re-routing without the executor removes 583 s (162 to 1,458) at frame length 6, 532 s at 5. Route after: 1:12:07.1 with track 6 (58:49 to 1:18:31), 1:09:57.3 without. The model's figure also covers corridors, which come in M4, so M3 alone may give less.
- **Where this differs from the design.** The movie-first design expects about 1,560 s here (60% of the busy part of the loop pool; the path's own returns to a held state are 37,426 frames by the files' proxy). That is just above the model's high end. The plan keeps the model's figure, because the proxy is looser than true equality and a cut also needs ego at the same place. M0's count of cell-equal pairs and its 30 cut tests replace `phi_fb` before this milestone is built.
- A verified gain of at least 162 s: 85%. At least 583 s: 50%. At least 1,458 s: 15% (the plan's judgment).

**Verified by** the standard verification of each round's kept combination; the ledger shows that every event of the new route is an event of the old one.

**Build** 3 days. **Run** 3 h, at most 3 rounds.

**Gate.** G4's projection for three rounds fits 3 h.

**Stop rule.** A round that gains under 1% of the route ends the milestone. If M3 gains under 162 s in all, stop and write up before M4: surgery on raw events does not carry, and only the executor path is left. If it gains under 600 s, the estimate is re-run and shown to the owner before M4.

### M4: direct walks

**What the system does by itself.** For each stretch between two kept progress groups whose travel is longer than the walk index says it needs, `opt splice` tries three rungs:

1. **A tree walk's own events**, placed forward. Accepted if ego enters the target room in the cell the next group's movie expects; the movie's own events then go on.
2. **Re-placement by identity**, only if M0 measured `r_exec` at 0.5 or more. Exits are clicked at the nearest pixel with the same hit from wherever ego stands. The trial's clicks are re-placed by their anchors and judged by the input log. Fallbacks are `refind.rs`'s: offer trials, the dialog-line search, skips off for the step.
3. **A corridor of the frozen high9**, unchanged: `--from` a one-line stop file of the current route, `--until` the one key the stretch gains, `--stops N`, `--minutes 15`, `--at-once 1`. At most 6 a run: the longest remaining stretches first, then the draw-ruled stretches M1 restored.

Accepted splices are combined and re-timed in rounds, as in M3.

**Deliverable.** A verified route; `p_exec` and `v_walk` as realised, per rung; corridors with their search counts.

**Prediction**

- Model: direct walks remove 1,352 s (902 to 1,728) when the executor works: 75% (`p_exec`). Route after, M3's cuts counted inside: 1:00:25.1 with track 6, 58:15.4 without.
- If the executor fails, nothing is added beyond M3 and the corridors, and the route stays near 1:12.
- A verified gain of 600 s or more over M3: 50% (the plan's judgment).

**Verified by** the standard verification; the ledger names the tree stop, the scan or the high9 run behind every new input.

**Build** 4 days. **Run** 3 h.

**Gate.** M0's `r_exec` and `v_walk` values; high9's sha256 unchanged.

**Stop rule.** Under half of the first 30 splices rejoin: stop rungs 1 and 2 and spend the run on corridors only. A corridor that finds nothing shorter in two runs is dropped.

### M5: drops with re-placement, and progress the end does not need

**What the system does by itself.** M3's drop rounds again on the steps that failed there only because later raw clicks missed, now with rung 2. Then the 257 progress trials: M0's single tests give the candidates; ddmin over the droppable ones; trims of repeated sentences inside trials. Every trial that cannot go is listed with the keys whose loss broke the route.

**Deliverable.** A verified route. The needs table in full: for each progress trial, needed or not, and what needs it. This is a research result in its own right, and the input any later re-routing would start from.

**Prediction**

- Model: drops of no-gain steps remove 676 s (373 to 1,050), 85% (`p_drop`); unneeded progress 174 s (29 to 376). Route after: 47:29.7 with track 6, 45:19.9 without.
- The 101 progress trials that set only keys no script tests are the pool to watch (394 s busy, the ceiling of `y_need`).

**Verified by** the standard verification; each "needed" entry of the table is a recorded failed replay whose differing keys include a key the dropped trial set, judged with the delay repair and the restore in place, not a guess; a failure at a draw that no repair fixes stays "undecided".

**Build** 2 days. **Run** 3 h.

**Gate.** M0's projected cost of a drop round.

**Stop rule.** As M3. After M5 the owner decides on the follow-up experiment (open decision 1).

### M6: order and timer filling, by a small cost-ordered archive

**What the system does by itself.** `opt archive` holds cells with their earliest arrival (two per cell, keyed by entry token). Layers follow the route's progress order. From a cell it tries: a later progress group played early (up to 12 ahead) where the needs table does not forbid it; other needed work inside a timer wait, whose minimum the padding ladder measured. An earlier arrival replaces the stored one and its descendants are expanded again. It runs only on stretches where M3 to M5 stopped gaining. A draw-ruled stretch stays one block; if the draws before it change, a corridor re-finds it, within the cap of 6.

**Deliverable.** A verified route with any accepted re-ordering; the list of moves tried and why each failed.

**Prediction**

- Model: 122 s (20 to 364), 45% (`p_plan`). Route after: 45:38.9 with track 6, 43:29.1 without.
- The tree bounds re-ordering at 3,000 to 7,000 frames, and the 33 wait trials hold 8,550 frames (graph report).

**Verified by** the standard verification; the archive's qf for the kept route must equal the verifier's to the frame.

**Build** 4 days. **Run** 3 h (beam 24, window 12; if an attempt costs over 0.3 s, beam 12 and window 6).

**Gate.** The needs table shows independent pairs whose swap is worth at least 60 s by the walk index. Otherwise M6 is not built.

**Stop rule.** One run. A sweep that improves no cell by more than 60 frames ends it.

### M7: execution tightening and luck

**What the system does by itself.** `opt tight` on the route's final structure, from the last draw-marked group on first (gains carry one for one there), then backward:

- exit clicks at the nearest pixel with the same hit (up to 40 samples on an 8 px grid; stop after 3 identical arrivals);
- a right click where it queues the same sentence;
- re-clicks during a walk by the direction rule, a width-6 beam only where the rule gains;
- approach clicks for targets off screen;
- luck by delay: at each draw-marked group, delays 0 to 12 judged at the group that consumes the draw, keeping the earliest completion.

Acceptance has three levels: exact rejoin by digest; state equal with the RNG different, repaired by giving back delay; one for one after the last draw.

**Deliverable.** A verified route; gains by lever; the direction rule re-measured on 20 of the route's walks before the full sweep is run.

**Prediction**

- Model: in-visit execution 70 s (walk stage 60%); interactions add 79 s; luck 37.5 s (delay scan 85%); tuning 28.3 s (5.5 to 86.4); walking beyond the hand route 22.3 s (2.5 to 57.3). Route after: 44:19.9 with track 6, 42:10.1 without.
- **Where this differs from the design.** The design expected about 4 min here. The model gives a net of about 1.3 min, because its interaction share is charged at this step and E025 measured that only 1 local gain in 4 survives before a draw.

**Verified by** the standard verification after each lever, so a lever that breaks a later draw is seen at once.

**Build** 3 days. **Run** 3 h.

**Gate.** No structural milestone is pending: every earlier change re-rolls later draws.

**Stop rule.** A lever that gains under 10 s on its first 50 sites is dropped.

### M8: the glitch lane (stretch)

Described under "Glitch discovery". Deliverables in order: the anomaly census of the current route; scans A to D; a slide-type state formed by a general scan and kept as a cell (a result whatever its payoff); persistence profiles; any verified earlier arrival that comes from an anomaly; the same scans unchanged on Monkey Island 2 and Fate of Atlantis. Scans B to D act on a route's visits, and neither game has a finished route of the search, so there: scan A from boot, and B to D on the longest verified route the project holds for each (for the Fate of Atlantis demo, E024's shortened route; for Monkey Island 2, E029's longest verified stop). The report says which game files were used.

**Prediction.** Model: slide state formed 84.7%; head and necklace 6.5 s at 30%; feather path 5.0 s at 24.5%; slide carried into Part III 2.99%; whole chain 148 s at 0.0377%. Route after, at the centre: 44:08.4 with track 6, 41:58.6 without. The expected gain by the model is a few seconds (0.3 × 6.5 s, 0.245 × 5.0 s, under 1 s for the chain); the design's own figure was about 1 min. The lane's value is its results, not its time.

**Verified by** the standard verification of any route that uses an anomaly; an anomaly state itself is shown by a replay from boot that stops in it (the counter read from Status).

**Build** 5 days. **Run** 3 h on the laptop (scans A to C about 2.5 CPU-h; D sampled every fourth frame, about 1.2 CPU-h; up to 6 probes of 15 min), plus one job of about 1 h for the two other games.

**Gate.** The census of the current route is silent, as full5's was; otherwise the baseline is rebuilt first.

**Stop rule.** At most 6 frozen-search probes and one run. An anomaly whose persistence profile dies before any later progress group is dropped. If M8 changes the route, M7's pass runs again on the changed part.

### M9: the clean-room run

**What the system does by itself.** Every derived file is scrapped. The whole chain runs again from the game's files and full5's search output alone: clock, end, tables, re-time, boot, surgery, splices, drops, archive, tightening, anomaly lane.

**Deliverable.** The same final route or a better one, verified from boot twice with byte-identical traces, timed in both conditions; `gate.sh` PASS; high8 and high9 unchanged.

**Prediction.** It reproduces the best earlier result within 1 s: 85% (the designs).

**Build** 1 day. **Run** about 21 h on the laptop: the M0 to M8 runs on Monkey Island again (23.5 h) less M0's measurement probes and drop tests and the other games' scans (about 2.5 h), in 7 jobs of at most 3 h over several days. The VM's one run (6 h on 30 vCPU) replaces them only if the parallel share of M0 to M8, measured as they run, makes the chain fit 6 h there; the serial parts (whole re-times, the Python replays, corridors at `--at-once 1`) do not speed up with more cores.

**Stop rule.** A different route from the same inputs: stop and find the non-determinism.

## Glitch discovery

### The general mechanisms

Glitches are found as states the verified route never shows, and valued by what they let the route do earlier. None is named in the code.

- **Signals with a zero baseline.** Measured on full5 from first control to the end flag (glitch report, one replay with the frame log): the pair (input counter, cutscene depth) takes 6 values; input is never on inside a cutscene; the counter never exceeds its rest value; ego is never absent while input is on; no engine warning. So any new value is new, with no threshold to tune. Two more signals: the frame length at a control stop differing from its rest value, and a timed window that lasts longer than in its twin fork. Ego position jumps are not used as a signal by themselves: full5 has 138, 110 of them in one room.
- **Where the scans act: the frames the discovery search never touches.**
  - **A, boot and frame-length frames.** One key at each boot frame (13,640 tries at most), and at each frame where the frame length is below its rest value (75 frames, 375 tries; two 14-frame stretches at value 1 add 420 tries). M2 already runs the boot part.
  - **B, skip timing.** Esc at each of the first 60 frames of the 574 cutscene intervals, and 5 delays at each text skip: about 19,000 tries.
  - **C, the bracket race.** At visits where a script that was started freeze-resistant and holds an absolute input-on write is alive (found by opcode kind in the scripts' dump and checked in the state dump's slots): skips off, a floor click, then a cutscene-opening input started 0 to 24 frames later. About 14,000 tries a visit; 32 such visits on full5; about 1.9 CPU-hours.
  - **D, open-gate frames.** Inputs while ego walks or a sentence runs: 34,757 frames in 1,478 intervals on full5, about 40 pruned inputs each, 1.39 million tries, about 4.6 CPU-hours. Sampled every fourth frame first.
- **What the scan shapes are, honestly.** Scan C's trigger (a freeze-resistant script holding an absolute input-on write, a race against a started bracket) and its ranges (0 to 24 frames, 60 frames, 5 and 13 delays) were written by people who know the slide. A find shows that a scan of this shape can reach the state; it is not a discovery of the class and is never reported as one. Every range is a run parameter, recorded, and none is the reference's number. The report counts all anomaly states the scans formed (not only the ones that match a known glitch), so a hit is not chosen after the fact. The test of generality is M8's unchanged scans on two other games.
- **Luck is the same lever.** Twins find the draw points; a delay steers them (M1, M7).
- **The anomaly signature is part of the cell**, so a glitch state is never merged into a normal one.

### What the bridge must newly observe

Nothing for the first pass. Status already carries the input counter, the cutscene depth, the frame length, ego (only when ego is in the room), the RNG and qf. The frame log gives them per frame. The input counter can be a run condition (`CondKind::Var`). The state dump gives each script slot's freeze count, delay and overrides. The input log says whether each input was taken or dropped at the gate.

One gap: a forked child closes the frame log (`speedrun_control.cpp:201-203`), so inside a trial only stops are seen. The lane therefore steps anomaly trials frame by frame (1.37 ms a frame). If M8 shows anomalies that open and close between two stops, the fix is **patch 0014**: a read-only list of anomaly events kept in memory, also in forked children, fetched by a new wire command (an absolute input or cursor write while a cutscene is open; a script slot killed before its end; a walk order for an actor not in the room; engine warnings by site). It is off unless asked, the frozen binaries never send the command, and the gate's six digests must be unchanged. It is a change to shared code, so it needs the owner's yes (open decision 4).

### How anomalies are valued

A setup costs time before it pays (the hand route's slide setup cost about 23 s), so a step-local judge would reject every one. Four steps of rising cost:

1. **Persistence profile** (one replay). Continue the route's own inputs from the anomaly state. Count how many groups, rooms and cutscenes the anomaly survives. An anomaly that dies before the next progress group is dropped.
2. **Skip-ahead matching and a window scan.** From the anomaly state, try each of the next 20 progress groups at once, and inputs at frames the baseline could not use. A sequence break shows up as a later group that succeeds early, or a room entered out of route order. Its prize is read from the route: the qf between the skipped groups.
3. **A frozen-search probe.** high9 `--from` a stop whose movie holds the keys and skip switches, about 200 stops, 6 to 15 min. Only for anomalies that change what normal play reaches. At most 6 a run.
4. **A whole re-time, verified from boot.** The only number that counts.

An anomaly cell carries a debt: its qf minus the best normal arrival at the same layer. It stays alive while it survives and its debt is under a cap. The cap is a free run parameter set from full5's own costs (for example a multiple of the median progress group's qf) and swept over a stated range; it is not taken from the reference's setup cost.

### Honest feasibility per known glitch

The reference route is used here only to score. Probabilities are the model's centres (`estimate.md`, table 4), with the glitch report's own odds where they differ.

| Glitch | Kind | Found by | Model's odds | Worth |
|---|---|---|---|---|
| Logo frame length | timing, one key | scan A, M2 | 80% (kept through a whole re-time) | about 642 s on the re-timed route, 334.6 s on the final one |
| Intro skip | boot keys | M2 | 97% | 167.3 s (37.4 s without track 6) |
| Skip timing | timing | scan B | near certain to find what exists | seconds; not in the model |
| Draw steering at exits and fights | timing | twins and delays, M7 | 85% | 37.5 s |
| Early safe opening | ordinary sentences and a draw | already in full5; kept by the delay repair | 89.5% kept (58 s lost if not) | already in the route |
| Slide state formed at the store | a 3-input race, 5-frame window | scan C | 84.7% is the model's figure for a scan shaped with knowledge of the glitch (glitch report: 80 to 90%; one probe at stop 9329 already reached it). Odds that a scan with no such knowledge forms it: not estimated | nothing by itself; a result in its own right, not a discovery |
| Slide regained at the village, feather path | race, then a hold | scan C, then stepwise on the counter | 24.5% (glitch report: 10 to 15%) | 5.0 s net |
| Head variant and necklace | timing, standing point | arrival-frame comparison, M7 | 30% | 6.5 s |
| Slide carried into Part III | 6 keep-alive readings | persistence cells | 2.99% (glitch report: under 10%) | pays the setup (−23 s) |
| Slide uses in Part II | input inside cutscenes | window scan | 1.49% | 21.2 s |
| Oars skip | carried slide, 4 timed clicks | window scan | 1.43% | 18.0 s |
| Monkey skip | carried slide, 4 to 5 timed inputs, a camera condition | window scan | 0.314% | 92.5 s |
| Whole slide chain | all of the above with the order around them | | 0.0377% | about 148 s |
| CD track 6 | a files condition, not a glitch | never searched | | 129.84 s, the second reported condition |

The slide family is worth 290.1 s between the two hand runs. Forming the state is likely. Carrying it to its uses is not. The plan promises the first two tiers (timing glitches; the formed state as a result) and treats the chain as a stretch.

### Evaluation against the reference

`eval/compare.py` reads the reference run's goal times (`runs/anypercent/m2n/results.jsonl`) and the optimizer's route, and prints time by stretch over the estimate's 17 segments. It scores only. The grep gate (G3) checks that `fmi-opt` holds no path to the reference folders.

## Gates before any run

- **G0, the discovery search unchanged.** `git diff` from high9's commit is empty for `fmi-direct`, `fmi-core`, `fmi-engine`, `fmi-explore` and `patches/`. The workspace's `Cargo.toml` gains one member line and `Cargo.lock` gains only that crate's entries. Any edit to `fmi-shorten` (M2's frame-length parameter) is allowed only if `cargo tree -p fmi-direct` does not list it; otherwise the change goes into `fmi-opt`. `cargo tree -p fmi-direct` is the same before and after. The sha256 of high8 and high9 is unchanged. `gate.sh` gives PASS with the frozen high9, and with a fresh build whenever the workspace was rebuilt.
- **G1, fake engines** (`fmi-opt/tests`). The placer picks the earliest passing frame. The ladder returns the first rung that passes. The delay scan finds the first delay that repeats a draw. A draw stored outside the recorded keys fails the verdict at its own group. The restore gives the base's digests from the restored control return on. Two states that differ only in ego position, entry token or anomaly signature are different cells. A drop is kept only for an earlier verified end. A failing combination is split by ddmin. The ledger refuses an input of unknown source.
- **G2, identity.** Every group at its original offset gives full5's digests at all 1,485 control returns and the end flag at frame 108,830. `anyclock` gives 1,261.483 s (frame 14,820, qf 303,846) and 1,391.320 s (frame 16,378) for the reference movies. full5's explicit-key replay reaches the same frame as its automatic-skip replay.
- **G3, generality.** A grep for game numbers cannot tell them from other integers, so the gate is an allow-list: every integer literal in `fmi-opt` other than 0, 1 and −1 is listed in `fmi-opt/LITERALS.md` with its meaning (a cap, a threshold, a frame count, a bridge protocol value), and the gate fails on any literal not listed; the list is reviewed at each milestone. It also finds no path under `routes/anypercent`, `docs/anypercent`, `runs/anypercent` or `docs/human-route.md`. Engine-level variables the bridge reports by name for every v5 game are listed in the report, each use. The end flag is a run parameter. The reference goal is only in `eval/`.
- **G4, cost** (measured in M0, re-checked on the first 100 groups of each pass): a placed group at most 40 ms (a candidate costs about 8.4 ms: fork 4.46, first frame 1.37, about 34 busy frames at 0.045, facts and digest about 1; the offsets −2 and −1 usually fail, so most groups try 3 or 4); a whole pass without repairs at most 2 min, with repairs at most 30 min; the atlas at most 40 CPU-min; three surgery rounds at most 3 h. If G4 fails, the design changes (horizon, candidate caps) before the run.
- **G5, forks.** Every fork result used in a kept version equals a plain replay from the previous kept stop. One mismatch stops the pass.
- **G6, the run itself.** The heavy lock, an inhibit lock against sleep, a watcher. Outputs go to the other disk if the home disk is low.

## Overall prediction

This section is the calculated estimate (`estimate.py`, `estimate.md`; seed 31; audited on 2026-11-16). The numbers are the model's, not the designs'. The first version gave a median of 53:57.1 with track 6. The audit fixed eleven points (a draw-repair rate that was never measured, idle frames counted twice, shared stages drawn as independent). The corrected figures are below.

| | CD track 6 present | CD track 6 removed |
|---|---|---|
| Median final time | 1:03:19.9 | 1:01:04.8 |
| 10th to 90th percentile | 46:39.1 to 1:37:04.4 | 44:24.3 to 1:34:47.4 |
| Odds of under 60:00 | 44.6% | 48.4% |
| Odds of under 30:00 | 7.4e-06% | 0.00117% |
| Odds of under 28:01.4 | 2.4e-08% | 1.5e-05% |
| Odds of under 23:11.3 | none in 150,000 draws (not zero) | none in 150,000 draws (not zero) |
| Odds of under 21:01.483 | none in 150,000 draws (not zero) | none in 150,000 draws (not zero) |
| Every stage works, every parameter at its centre | 44:08.4 | 41:58.6 |
| All assumptions pessimistic: median | 2:22:22.0 | 2:19:41.4 |
| All assumptions optimistic: median | 25:52.4 | 23:42.4 |
| All optimistic: odds of under 21:01.483 | 0 | 2.18% (13.2% if the stages succeed or fail together, rho 0.9) |

- full5 on the same clock: 3:01:50.9. The median is 2.9 times shorter.
- "0" means no pair of draws among 150,000 × 150,000 in any stratum. The best route-side draw is 29:29.8 without track 6; the largest gains-side draw is 292.3 s; even that pair gives 24:37.5.
- In 4.44% of draws the closing menus are never answered. Those draws have no Any% time and count as failing every line.
- If the stages succeed or fail together, the median moves to 57:31.1 (rho 0.5) or 54:15.0 (rho 0.9) with track 6, and the odds of under 60:00 to 54.2% or 62.7%. The odds of 21:01.483 stay 0.
- Odds below about 1% depend on the shape of the assumed ranges. With flat ranges, under 30:00 without track 6 is 0.049%. Read them as orders of magnitude.

**The model.** T = R − G. R is the route brought toward the hand route without the slide (at best 28:01.4). G is what is gained beyond it: luck, tuning, walking, the slide family (at most 290.18 s plus up to 57 s of walking). It reproduces both hand anchors. 35 values are measured, 22 derived, 10 are measured ranges, 42 are assumed.

**Dominant assumptions** (pessimistic to optimistic end; median with track 6; `estimate.md`, tables 10 to 12):

| Assumption | Range (pessimistic, centre, optimistic) | Median moves | Under 60:00 moves | Measured in |
|---|---|---|---|---|
| `p_exec`: a trial can be made again from another place and state | 0.5, 0.75, 0.9 | 1:13:16.8 to 57:56.5 | 31.6% to 55.4% | M0 |
| `v_walk`: share of the direct-walk pool (1,503 s) realised | 0.6, 0.9, 1.15 | 1:09:34.6 to 56:14.9 | 26.7% to 54.6% | M0 |
| `pen_draw`: seconds lost per draw point not solved again | 400, 120, 30 | 1:10:12.8 to 58:19.5 | 29.7% to 51.9% | M1 reports it |
| `q_search`: the frozen search solves a draw-ruled stretch again | 0.5, 0.8, 0.95 | 1:09:13.7 to 58:37.1 | 33.7% to 51.6% | M0 |
| `p_drop`: drop rounds work at this scale | 0.6, 0.85, 0.95 | 1:08:31.4 to 58:52.8 | 34.1% to 52.5% | M0 |
| `iota`: interactions | 0.25, 0.08, 0 | 1:07:18.9 to 1:00:46.0 | 38.6% to 48.9% | after M2 |
| `n_dp`: draw points to solve again | 19, 11, 7 | 1:06:45.7 to 1:00:49.1 | 38.5% to 48.9% | M0 |
| `q_atom`: droppable share of the 207 no-news steps that changed an atom | 0.2, 0.5, 0.85 | 1:06:33.0 to 59:18.2 | 33.1% to 50.5% | M0 |
| `r_xwait`, `r_inner`: waits and inner idle removed | 0.6-0.97, 0.8-0.99 | 1:05:41.1 to 1:01:55.4; 1:05:16.7 to 1:02:31.3 | 40.8% to 47%; 41.6% to 46.1% | M0, M1 |
| `p_logo_surv`: the route survives a re-time at frame length 5 | 0.5, 0.8, 0.95 | 1:04:57.2 to 1:02:06.0 | 41% to 47% | M0, M2 |

For the target itself: with every assumption optimistic the odds are 2.18%, and any one of `v_walk`, `q_atom`, `r_xwait`, `r_inner`, `e_tune`, `sv_draw` at its pessimistic end takes them to 0. With the others drawn, no single assumption lifts them above 0.

**Why the target is out.** It needs the route's structure at hand quality and the whole slide chain at once. full5's 257 progress trials alone take 35.2 min with no travel. The model cannot see three things: an unknown glitch, a route order better than the hand route's, and a set of progress steps smaller than the hand route's. To reach 21:01.483 without track 6, an unknown gain of 1,403 s is needed at the 10th percentile and 2,403 s at the median; 1,200 s found for certain would give odds of 3.76%.

**Where this plan differs from the model, and from the designs.**

- The logo glitch comes at M2, not last. The final time is the same; the rows in between are at frame length 5.
- M3 uses no executor. It is priced by the model's term for raw cuts (583 s), not by the design's 1,560 s.
- M1's expected time (about 1:43) carries the model's own draw penalties, which the staircase lists separately.
- The designs' medians (53 to 58 min without track 6) and their odds of under 30:00 (1.5 to 2%) are not this plan's prediction. The designs' argument for a fatter tail is that many of the 257 progress trials may be unneeded, which the model cannot express. M0's 257 drop tests measure exactly that.

**Success threshold.**

- **Yes:** a route from boot, made by the optimizer alone and verified as above, under 60:00 on the Any% clock in either condition.
- **Strong yes:** under 45:00. A glitch state formed by a scan is reported as a result, not as a strong yes, because the scan's shape was written knowing the glitch.
- **No:** no verified route under 2:00:00 after M1. Then the waste is not in timing, and the experiment stops.
- **The owner's target** (under 21:01.483) is reported against every milestone and is not promised.

**The estimate is a file of the experiment.** `estimate.py` and `estimate.md` are committed beside this plan. After every milestone the measured values replace assumptions and the script is re-run; the lab note records the old and new median and odds. The decision to build each later milestone is taken on the re-run numbers.

## Compute and schedule

| Milestone | Laptop hours | Jobs | What dominates |
|---|---|---|---|
| M0 | 2 | 1 | clock replays, atlas, probes, drop tests, one corridor |
| M1 | 1 | 1 | one pass with repairs, verification |
| M2 | 1.5 | 1 | boot scan 30 CPU-min, up to 3 whole re-times |
| M3 | 3 | 1 | three rounds of end-to-end re-times |
| M4 | 3 | 1 | up to 6 corridors of 15 min, re-times |
| M5 | 3 | 1 | drop rounds |
| M6 | 3 | 1 | one archive run |
| M7 | 3 | 1 | walk sweeps, delay scans |
| M8 | 4 | 2 | scans A to C, sampled D, 6 probes; the two other games |
| M9 | about 21 | 7 | the whole chain again |
| Sum | about 44.5 | 17 | |

- Every job is at most 3 h, holds the heavy lock and an inhibit lock, and has a watcher. One engine job at a time on the laptop.
- Build effort: about 33 days of work in all (6, 3, 2, 3, 4, 2, 4, 3, 5, 1).
- **The VM** (about 8 dollars, one run of about 6 h on 30 vCPU) is not needed by M0 to M7. It is held for one of two uses: M9's chain if the measured parallel share makes it fit 6 h there (otherwise M9 runs as 7 laptop jobs), or a full pass of scan D with a wider carry search if M8's laptop pass shows a slide state that survives. Not both; open decision 5 settles it after M8.
- Order of results: after M0 the metric and the re-run estimate exist; after M2 there is a first verified route near 1:20 to 1:45 and the first logo pair; after M5 the fixed-order work is done and the needs table exists; M6 to M8 are each built only if their gate passes.

## Risks

- **The executor.** Re-making a trial from another place stopped at step 5 of 12 in E024. It decides M4 and half of M5. M0 measures it before anything is built on it.
- **Draws.** Any cut or shift changes how many per-frame draws run before later random outcomes. The store's draw, the room with drawn layouts (21 visits, 13,200 ticks) and the fight choices can all come out differently. A delay steers them only where the RNG moves per frame; the audit found a working delay at only 2 of 6 points. Where the draw decides which inputs are needed, only a corridor helps, and only 6 fit a run.
- **Hidden state in loops.** The recorded news undercounts facts (1,830 stops), and 207 no-news steps change a world atom. Script locals, inventory order, camera and CD state are outside the masked facts. A cut can pass locally and fail later; only the end-to-end re-time catches it, and each catch costs a whole re-time.
- **Frame length 5 and inner offsets.** Tick-paced waits inside a group take more frames. Five inner gaps over 100 frames hold 2,051 frames. The re-timer must re-place inner offsets or the gain is lost.
- **Splices from the tree** are raw clicks made under another camera, ego and room-graph state. Admitting only walks seen with the same objects costs 3,250 frames more. Many will miss.
- **Cost.** Each round's unit is a whole re-time. If repairs push it past 30 min, three rounds do not fit 3 h. The laptop is shared (load 25 to 30 was seen during the research).
- **The fork label clash** (E025 F4) has no known cause, and every pass judges on forks. G5 checks every fork result that is kept.
- **The frozen search in corridors** selects by progress, not time. It may find nothing shorter.
- **The end rule.** The credits loop. A wrong rule could give a time the evaluation goal does not confirm.
- **Explicit keys.** The conversion of automatic skips to keys has not been run on a route of the search. The 1,921 pointer-parking moves may matter to a hover script when they are dropped.
- **Track 6.** `docs/anypercent/findings.md` calls a run with the track not playing invalid; `overnight.md` calls it an open rules question. Both conditions are reported; neither is claimed for the other's reference.
- **The estimate.** Its pool sizes come partly from a report the audit could not re-derive. Its success draws are independent in the main run. It caps the route's structure at the hand route's.
- **Generality claims.** The scan shapes were designed knowing the glitches. Nothing is claimed before the unchanged runs on two other games.
- **The structural floor.** With full5's actions in full5's order, the route cannot go below about 29 to 35 min. Past that, fixed-order work is wasted.

## Open decisions for the owner

1. **What this experiment promises.** The model puts the target (21:01.483) out of reach for this plan. Recommended: run the plan as the verified baseline and tool-builder, and decide after M5, with the needs table in hand, whether to plan a follow-up that searches for new actions and fewer progress steps.
2. **CD track 6.** Recommended: the optimizer works with the full files; each milestone's route is also re-timed with the track removed; both times are reported, each against its own reference.
3. **Seed and talk speed.** The Any% rules allow choosing both. The reference runs used a chosen seed (836557856) and talk speed 85; full5 used seed 1 and 255, so a comparison with 21:01.483 is not like for like. Recommended: keep seed 1 and talk speed 255, as full5, and say the difference wherever the reference is quoted. A slower text speed is tried only inside M8, as a separate pass, if a slide state survives its profile. A searched seed stays out of scope unless the owner decides otherwise.
4. **Patch 0014** (read-only anomaly events in forked children). Recommended: not now. Ask again only if M8 shows anomalies that open and close between stops; then with the gate's six digests unchanged.
5. **The VM's last run** (about 8 dollars, one run of about 6 h). Two uses compete: M9's clean-room chain if it does not fit three laptop jobs, or a wide scan D if M8 shows a surviving slide state. Recommended: hold it for M9; decide after M8.
6. **How the glitch lane may be reported.** Its scans were shaped knowing the glitches. Recommended: report any find as "reached by a scan of known shape", count the lane under RQ3 only in the limited sense stated above, and let M8's unchanged runs on two other games carry any generality claim.

(Where the code lives is not an open decision: a separate crate `fmi-opt`, not a default member, and `experiments/E031-optimizer/`, as the constraints require.)

## Out of scope

- Any change to the discovery search, its selection or its binaries.
- A search for new actions or cheaper solutions between needed facts. It is the likely follow-up (open decision 1), not part of this plan.
- A planner over a surrogate model (generated tasks, an ordering solver over all visits). The engine decides; the small archive of M6 is the only ordering mechanism.
- A full Go-Explore archive from the start. Its first two stages would repeat M1 and M3 at about four times the build.
- A searched seed; saving and loading to re-roll draws; debug keys; any setting the Any% rules ban.
- The known Any% route, its goal lists and its driver's per-move flags as inputs. They are read only by the evaluation tools.
- E024's forward pass and drop rounds as built, and E025's full walk stage (waypoints, corridors, diagonals): about 1 to 3 engine-hours for 6,000 to 10,000 ticks, the lowest gain per compute of the levers (execution report).
- Other games, except the unchanged scans of M8 on Monkey Island 2 and Fate of Atlantis.

## First build step

`experiments/E031-optimizer/eval/anyclock.py`: a copy of `scripts/anypercent_verify.py`'s `replay()` and time arithmetic with five parameters (seed, talk speed, game folder, start rule, goal) in place of its hard-coded values, built from the two scratch wrappers `scratchpad/metric/time_movie.py` and `scratchpad/metric/replay_ref.py`. Its first check, on `build/svm-direct`:

- the folder without track 6 does not exist yet (the October runs removed the file from `game/dos-cd` by hand): `anyclock` makes it as a folder of links to `game/dos-cd` without `track6.flac`, and records both folders' hashes;
- the goal is the one both reference `verify.json` files record, room 0 with variable 293 equal to 161, passed as `--goal` (the script's own default goal is a different one);
- `runs/anypercent/m2n/movie.jsonl` (seed 836557856, talk speed 85, start frame 46, the folder without track 6): frame 14,820, qf 303,846 (qf0 1,100), 1,261.483 s, as `runs/anypercent/m2n-verify/verify.json`;
- `runs/anypercent/m2/movie.jsonl` (the same, full files; a different movie: S7#1's delay is 800 frames there): frame 16,378, qf 335,006, 1,391.320 s, as `runs/anypercent/m2-verify/verify.json`;
- full5's movie (seed 1, talk speed 255, full files) with the end flag as the goal: frame 108,830, tick 657,813, qf 2,631,539.

It is committed together with `plan.md`, `estimate.py` and `estimate.md`. The second step is the explicit-key replay of full5 (about 5 min), then `opt end`.
