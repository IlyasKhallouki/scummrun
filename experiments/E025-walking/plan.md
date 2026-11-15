# E025: walking as a game mechanic: the system makes its own walks faster, judged by the engine, in real time

Date planned: 2026-11-09
Research questions it serves: RQ2 (optimization), RQ4 (generality)
Design: A extended (geometry proposes, the engine judges), chosen by the judges' scores (22 against 21 and 14), with grafts from the other two designs.
Base: branch `e024-fixes` at 69944bd. Code paths are under `/var/tmp/fast-mi/wt-e024-fixes/experiments/E011-speed/rs/`, with line numbers at 69944bd. Engine lines are under `/var/tmp/fast-mi/svm-e024/third_party/scummvm/engines/scumm`. Probe folders are under `~/.cache/fast-mi-e025/`.

## Question

Can the system make its own walks faster with no model of the walk code and no game numbers? The live walk boxes propose click sequences. The engine judges each one, first by the walk's arrival and last by the end verified from boot. The unit is a move: the clicks that lead to one destination. The objective is real time (qf), on seed 1 of the demo, from route-004's 13,888.75 jiffies (qf 55,555, 3:51.5).

Hypotheses:
- **H1.** Geometry plus engine judging finds, unaided, what the probes found by hand: 23 frames on room 48's walks 29 and 33, and 68 frames of `dir` re-clicks on 14 walks.
- **H2.** After the last random draw, a local walk gain carries one for one to the verified end. Measured once: 138 jiffies = 54 + 84.
- **H3.** Before a draw, a gain survives by chance (1 of 4 in the probes). The stage measures that rate over every candidate it tests.

## Problem (what E024 left)

- **The route.** route-004 ends at frame 2,580, tick 13,606, qf 55,555, `restart@1:582:48:0067`. That is 13,888.75 jiffies, 3:51.5 at 59.998 jiffies a second.
- **Real time against ticks** (realtime probe).
  - The 282.75 jiffies the tick clock leaves out are 29 screen fades of 39 qf each, all at room changes or in the boot. Skipped cutscenes add nothing. This corrects E024's amendment 3.
  - Under qf, all 33 scan choices and all 3 kept drops are the same.
  - Inside a room, one walking frame is 6 ticks = 24 qf = 6 jiffies.
- **Walking is most of the route** (research §3).
  - 7,740 of 13,606 ticks are walking (56.9%).
  - 29 walks are started by the player with input on: 6,804 ticks.
  - The box-aware estimate of what is left on them is 648 ticks. On the six top walks the engine gave 0.59 of the estimate (282 of 474 ticks).
- **fmi-shorten replays walks as they were.**
  - A walk anchor is its room point less the stop's xstart (`label.rs:338-346`). The game reads the click with the xstart after that frame's moveCamera (scumm.cpp:3256 comes before :3287; input.cpp:426). Walk 2's anchor (128,87) walked to (120,87).
  - `box_of` takes the lowest box and has no bounding-box test (`label.rs:128-130`). The engine takes the highest (actor.cpp:2040-2044, boxes.cpp:535-567).
  - The timing class is "next" or "idle" (`label.rs:255-261`). Nothing proposes another path, and nothing places a click before an arrival.
- **Re-finding a step does not reproduce the route.**
  - `steps_from` places a step that its plan found by a fallback through the strict scan first (`pass.rs:254`). It runs the fallbacks only when nothing completes (`pass.rs:287-310`).
  - From route-004's own prefix, step 25 is re-found at 2,439 instead of 2,151, and the control ends at 2,868, not 2,580 (top-walks probe).
  - E024's step-15 drop was judged "not earlier" (end tick 15,298) on this (`~/.cache/fast-mi-e024/run1.err:189-200`).
  - So every change before step 25 is judged about 288 frames behind.
- **Two passes stop on walking** (stuck probe).
  - G4's cave exit, without skips. Room 82's draw opens a cave at screen x −16..0. Its click grid is empty, and ego does not move without input, so no t gives it a point.
  - G7's kitchen, step 5. 1,532 of the 1,680 inner no-point t's come from the key `o` placed too early. 148 come from walk anchors (camera lag; actors on the point). Beneath that, step 4 completed at 335, before step 3's completion at 341, while the door was still open from ego's entry.
- **A fork label clash** (realtime probe; cause not found). At step 14, the base fork at frame 1407 shows room 83. The first `fork_each` child, also labelled 1407, shows room 49. The stage's choice came from the child.
- **What the base already fixes.**
  - `e024-fixes` (six commits, reviewed: ship) counts an engine error in a fallback trial or a reduction test as that test's outcome (8214d6e, eb9998e).
  - Its seed-1 stage gives route-001 to route-004 byte for byte (`fixes/run-seed1/`, `fixes-review/`).

## Prior work

Research notes: `research/research.md`. Probes: frame-effects and top-walks (`walkprobe/`), reclicks (`reclick/`), stuck (`walkstuck/`), realtime (`realtime-probe/`).

- **The engine's walk model** (research §1).
  - One leg's step count; legs chaining inside a frame; the final turn; snapping; routing by box hops (BOXM, and room 48's rebuild).
  - Gates are axis-aligned shared edges (boxes.cpp:1094-1180).
  - A click while walking costs no frame (actor.cpp:918).
- **Runners** (research §4):
  - waypoint clicks that straighten a path or pick another corridor;
  - re-clicking every step going up or left ("schmoovement");
  - acting while walking.
- **Literature** (research §5).
  - Hershberger & Snoeyink 1994, Thm 6.2: in one corridor at constant scale, the string-pulled path is time-optimal. So the funnel over the gates is the first proposal. Slot scale curves the optimum, so points are sampled along the gates.
  - Lee & Preparata 1984 and Mononen 2010: the funnel over portals. Demyen & Buro 2006: a search over corridors, each scored by its funnel path.
  - Geraerts & Overmars 2007: propose shortcuts and let a checker judge them. Here the engine is the checker.
  - Ho, Sreenivas & Vakili 1992: a rough ranking only orders the work, and the simulator decides. The ranking's agreement is measured, not assumed.
- **What the probes measured in the engine.**

  | Probe | Measured | For this plan |
  |---|---|---|
  | frame-effects | Room 48: a Python port matched the engine frame for frame on 58 of 59 movies; the miss was a click on an object. Gate corner 36→28 frames, zig-zag 19→16, split leg −1. Stale box: a click on the frame after an edge crossing routes from the old box (89 against 59 frames) | The frame effects are 1-8 frames and depend on box shape. Trial finds them; no port is needed. |
  | reclicks | 549 re-click frames, none costing a frame. Re-clicks going down or right lose. NEW_LEG (mv bit 0) at the decision point is the only proof that a click re-planned the walk. 408 ticks route-wide with the `dir` rule, 492 with a width-6 beam. Clicking the object again turns the sentence into a walk-to | Re-clicks are a search over "click or no click" per frame. The object click goes last. |
  | top-walks | Six walks: 474 estimated ticks, 282 measured. Walks 29 + 33 in room 48: −138 jiffies at the end, verified from boot by both launches; the later steps, re-found, absorb the camera change. Before the quiz: walk 12 kept (−78 against its control), walks 2 and 6 cost +1,002 frames, walk 17 dead-ends | After the last draw, a local gain is the end gain. Before it, a lottery. |
  | stuck | The kitchen and cave causes above. A walk to the left edge reaches the cave goal at 4,795 (+48 frames on the movie). Redirects named by their destination: 402 of 441 sampled t's complete, with state 367 left out of the goal (a measurement only) | Approach walks, redirects named by destination, a key placed with its click, a completion floor |
  | realtime | Among a step's completing candidates, qf − 4·tick is constant | Switching to qf changes no E024 choice. |

## Change

All walk behaviour is off by default (`--walks`). Each item is one commit with its gate.

Order of work:
1. qf (Part 1), checked byte for byte first.
2. F1, with its gate from six bases.
3. F2, with the goal step exempt.
4. F3 and F4.
5. Moves as labels, with the walk search off.
6. The walk stage on zone C (room 48), which must reach qf ≤ 55,003.
7. Walks before the last draw, behind local acceptance.
8. The two stuck cases.

### Part 1: the objective is real time (qf)

qf is the bridge's count of quarter jiffies (speedrun_bridge.cpp:522-526). It is in every Status and every end (fmi-core `lib.rs:703`). Fork children inherit it, and it never decreases, so pruning stays safe.

| Where | Change |
|---|---|
| `scan.rs:71-85` `Found`, `key()` | Add qf. The key becomes (qf, frame, t). Fill it at `scan.rs:476`, `:517` and `refind.rs:130`, `:384`, `:396`. |
| `scan.rs:264-300` `Bound` (`open`, `cap`'s pace) and its callers (`:386-391`, `:438`, `:464`, `:512-516`, `:551`, `:580-592`) | Use `status().qf`. |
| `refind.rs:135-141` `better()`; the dialog search at `:383`, `:395` | Compare (qf, frame). |
| `verdict.rs:135-152` `End`, `earlier()` | Add qf; compare (qf, frame). |
| `reduce.rs:100-110` `waste()`; `:39` `MIN_GAIN`; `:382-387` | Waste and gain in qf. `MIN_GAIN` is 240 qf (1 s). |
| `movie.rs` (the end row `:109`, the standalone end `:193`, verify's same-end check) | The end carries qf, and both launches must agree on it. The standalone limit (`:225`) stays in ticks. |
| `fmi/src/shorten.rs:142-153`, `:179-181`, `:205-215`, `:317-337`, `:438` | Read and report qf. `shorten trace` also prints qf, walkbox, mv, up and cs. |

- A faster walk removes no fade, so its gain in qf is 4 × its gain in ticks.
- qf decides only between candidates whose room changes differ.

### Part 2: known gaps, fixed before any walk work

- **F1, a step's plan fallback in later passes** (`pass.rs:209-356`).
  - For a step whose plan came from an offer, realize or dialog fallback, `steps_from` also runs that fallback. It already does this for skips off (`:232-252`), and `cheap` does it (`reduce.rs:220-224`). The earlier result by (qf, frame) is kept.
  - Measured cause: step 25 re-found at 2,439 against 2,151 (top-walks probe), and the step-15 drop.
- **F2, a step completes no earlier than the step before it.**
  - `StepSpec.not_before` is the previous step's completion (`after`, `pass.rs:230`). The scan's verdict (`scan.rs:510-521`) and the fallbacks' `judge` (`refind.rs:128-133`) first run to that frame.
  - The goal step is exempt. Route-004's steps 33 and 35 complete at 2,595, after the end (marked `after_end` since 69944bd), and goal step 36 completes at 2,580.
  - Measured cause: the kitchen's step 4 completed at 335, before step 3's 341. The entry script closes the door at 341 (stuck probe).
  - Not a no-op on the demo. Route-001's step 35 completes at 2,657, before step 34's 2,725 (`~/.cache/fast-mi-e024/run1/route-001.steps.jsonl`). Routes 002-004 are in order except the goal step. Step 34 is the first drop E024 kept.
- **F3, walk labels as the engine sees them.**
  - (a) `box_of` (`label.rs:128-130`, used at `:119`) follows the engine: the highest box that holds the point, after the bounding-box test.
  - (b) A walk anchor's room point is where the engine sent ego: the record's Walk touch (`label.rs:296`), not the stop's `at` point.
  - (c) One-frame look-ahead. `place_hit` for a walk (`label.rs:338-346`) forks, runs one frame, and converts with the xstart there. That is the value processInput uses (input.cpp:426). The reclick probe placed every click this way.
- **F4, the fork label clash.** Its cause is named with file:line. It is then fixed, or each candidate's state is shown equal to a fresh fork run to the same t. No walk click is timed right after a room change before this.
- **K4 for the walk stage.** An engine error in a walk candidate is that candidate's outcome, counted, as it already is for scan candidates, fallback trials and reduction tests.

### Part 3: moves as labels (`label.rs`, `scan.rs`)

- **Input classes,** from the anchors and identities E024 already records (`label.rs:200-224`, `:286-298`):
  - W: a floor click with an ego Walk touch and no sentence;
  - P: a verb or key that queues no sentence and starts no walk;
  - D: an object or actor click that queues a sentence, or a dialog line.
- **A move** is `W* P* D`, or a trailing `W+` whose last W is its destination.
- **Waypoints** are the W's before the destination.
  - They are judged by "taken, and NEW_LEG at the next decision point", never by their room point.
  - A waypoint with no point at its frame is skipped, not failed.
- **A P given while ego walked is placed with its D**, in the frame before D, once D has a point. This replaces the kitchen key's "next" timing. Other P's keep their class.
- **A destination with no point** gets an approach click.
  - The click goes to the floor at the screen edge on the destination's side, at ego's y and at the destination's walk point's y (the obs reply's `room_objects`). If `at` finds no floor there, it takes the nearest floor y in that column.
  - D then goes at the first stop where it has a point.
- **Nothing walks after D.** A walk click cancels the sentence through doSentence(STOP) (script_v5.cpp:1008-1012).
- **New timing classes** in `inner` (`scan.rs:537-606`):
  - `Lead(n)`: n frames before ego stands at the previous click's target with mv = 0. A fork runs to that arrival, and the click goes in n+1 frames before it.
  - `FirstPoint`: the first stop where the anchor has a point, within the step's bound.
- On route-004 the moves cover the five within-step walk-click chains: steps 8, 13 (two), 26 and 28.

### Part 4: the walk stage (`walk.rs`; `fmi shorten walks` and `run --walks`)

It runs after E024's drop rounds, on the best route and its plans, followed by one more drop round.

**4.1 Finding walks.**
- One replay of the best route from boot, with the route's `meta.env`. Without it the skips start at frame 992 (frame-effects probe).
- At each stop it reads frame, qf, room, ego, mv, walkbox, xstart, up and cs.
- A player walk is a run of frames with mv ≠ 0 that a placed input started while up > 0 and cs = 0. It ends at arrival, at a room change, or at the next input. A walk the game queued is not one (a click would cancel its sentence).
- Each walk records its first input C, its end point and done stop, its boxes, its per-frame steps, its turn frames, and its step and move. The demo has 29 (research §3).

**4.2 What is read** (G7: no game numbers in system code).

| Need | Source |
|---|---|
| Box corners, flags, the room | The bridge's `boxes` reply (fmi-core `lib.rs:796-801`), at the walk's first stop and at each room switch. Room 48's scripts change flags and rebuild the matrix. |
| Gates | Shared edges that are exactly horizontal or vertical, computed from the corners (the engine's rule, boxes.cpp:1094-1180) |
| Ego, mv bits, walkbox, xstart, up, cs, qf | Status (`lib.rs:696-726`) |
| Floor, object or actor at a point | `at` |
| Where a click sent ego | The input log's Walk touch |
| An object's rectangle and walk point | The obs reply's `room_objects`; ego's arrival and turn frames in the baseline |
| Speed per box (ranking only) | The largest in-leg \|dx\| and \|dy\| per frame in that box over the baseline; the room's median for a box never walked |
| Draw zones | The twins' marks (`chain.rs:738`) |
| Engine constants | Those already in `afford.rs` (`BOX_INVISIBLE`, `BOX_LOCKED`, `FIRST_VALID_BOX`, `SCREEN_W/H`) |

- Nothing new is read from the game files. There is no bridge patch.
- Not read: BOXM, SCAL, box scales, script-set speeds, CDHD walk points, facing, fractions. The engine judges their effects: room 63's speeds, room 82's script scale, slot scale.

**4.3 Proposals for one walk.** Every click point is on screen at its click frame (F3's look-ahead) and on floor by `at`.
- **G, corridors.**
  - Simple paths over visible boxes from the start box to the end box, up to two boxes longer than the engine's own (read from the trace).
  - Each is string-pulled over its gates and ranked by funnel length over the measured speed per box. The top 4 are judged.
  - Each bend gets a waypoint 2 px inside its gate, and again at ±4 and ±8 px along the gate. The probes' best, (251,98), was such a sample.
  - If the trace's walkbox sequence leaves the corridor, one waypoint is added in the box where it left.
- **T, gate corner.** A baseline leg that moves on one axis alone for 3 or more frames, while both axes have distance left, crosses a gate head-on. A waypoint goes to that gate's corner nearest the destination (36→28; walk 12: 7 frames).
- **S, split leg.** Where one axis arrives while the other still moves, the destination is clicked again on that frame (−1 frame measured).
- **R, re-clicks.**
  - The `dir` rule: click while the last step's limiting axis went up or left.
  - A target must differ from the current one after snapping. A click counts only with NEW_LEG.
  - A width-6 beam runs only where `dir` gains, and only after G8's `dir` part passes. It is reported, not predicted.
- **V, verb walks.** A walk click toward the observed walk point first, then the verb, then the object last.
- **Switch timing.** Each waypoint switch is `Lead(n)`, n = 0..20, found by coordinate descent in 2 rounds (the probes' best were 1-7). The final click's frame is searched too. Stale-box frames are not removed: by the port alone, they lose in 3.0% of cases and win in 3.3%.
- **Screen, then full search.**
  - The screen is `dir`, G on the engine's own corridor, and T: about 30 evaluations, ≤ 5 s.
  - A full search runs only when the screen gains ≥ 1 frame, or when another corridor ranks ≥ 2 frames shorter. It is capped at 600 arrival evaluations.
- **Recorded as an idea, not pixels.**
  - A waypoint is stored as its gate (its two boxes), its offset along the gate, and its `Lead(n)`. A re-click line is stored as {rule, target, from, to}.
  - Both are regenerated at placement from the live `boxes` reply and Status. Box indexes are run data, not code constants.
  - If regeneration fails, the step uses its E024 plan.

**4.4 Judging, in levels.** Each level passes only its best few on.
1. **Arrival.** Fork at C−1 and place the variant. Run until ego stands at the end point or the room changes, at most to the baseline's done stop + 20 frames. Score by (qf, frame). Every waypoint and re-click must be taken with NEW_LEG.
2. **Step** (top 3). Place the step's other inputs by their timing classes, then run `verdict::step` with keep(k) and `ego_at` (`verdict.rs:114-131`).
3. **Next step** (top 1-2). Run `scan::scan` of step k+1 from the new E_k. The walk is kept locally if that step completes at least 1 frame earlier.
4. **Local acceptance,** for a walk with a twin-marked step after it.
   - The steps after the walk's step are re-found by their plans under their strict goals. The relaxed goal and the fallbacks are used only where the baseline used them, with F1.
   - Each step's bound is the baseline's completion of that step less one frame, so each must complete earlier. The first step that misses rejects the walk.
   - It runs through the step after the last twin-marked step.
   - Why so far: the twins mark steps 4, 9, 13 and 24 (`~/.cache/fast-mi-e024/shorten/label4/steps.jsonl`). Under the strict goal, walk 2 kept its 6 frames through step 4 (675 against 681) and step 9 (1,102 against 1,108), and walk 6 through step 9 (1,102). Both lost only at step 13, where the quiz waited until t 2,400 (`walkprobe/runs/c6.log:5-14`, `:39-48`, `:79-83`). A horizon of the next marked step would accept both.
   - Measured step costs on that stretch: 0.0-1.1 s per step, 2.3-3.4 s at step 13, 6-7 s at step 25 (`c6.log`). With the bound, step 24's strict scan stops at the baseline's completion.
5. **End.** The batch test (4.5), verified from boot.

**4.5 Splicing.**
- **Zones.** Zone C is the steps after the step that follows the last twin-marked step (on the demo, steps 26-36). Zone P is the steps before it.
- **Order:** zone C first, then zone P. Later bases are cheaper.
- **A test** is `reduce::attempt` (`reduce.rs:255-326`) with the steps' plans replaced, not dropped:
  1. the cheap anchored replay (`:160-253`);
  2. then `steps_from` with F1;
  3. then verify from boot.
  - It is kept only for an earlier (qf, frame).
- **Zone C:** all local winners in one batch. If it is not kept, ddmin halves it, as E024 does.
- **Zone P:** only locally accepted walks, in one batch. If it is not kept, ddmin halves it, then at most 4 singles in order of local-acceptance gain.
- **A kept plan that fails on a later prefix** is regenerated from its idea. If that fails, the step uses its E024 plan, and the walk is searched again in the next round.
- **Rounds:** at most 2 walk rounds. Stop when a round gains less than 240 qf. Then one drop round.
- **In `reduce.rs`:** `Drop` (`:45-49`) becomes a `Change` with `Walk{k, plan, gain_qf}`. `candidates()` (`:112-157`) takes walk candidates by zone, then by local gain. `rounds()` (`:336`) runs the walk rounds after the drop rounds.

**4.6 Rank check** (a measurement, not a gate on the run). For each walk, all its corridors (at most 8) are judged once at level 1. The report gives the rank of the engine's best under the geometric ranking, per room. The judged count is the smallest k with recall ≥ 90% (default 4).

### Part 5: the two stuck cases

**Demo without skips, step 25 (E024's G4).**
- `by_offer` (`refind.rs:236-297`; one input per trial today) gets two-input approach trials. For each room object with no point at the offer's stop:
  1. a walk click toward its side (Part 3's approach click, and the offer's own walk points);
  2. a fresh offer at the first stop where the object has a point;
  3. each offered input for that object.
- These are judged by the relaxed goal, as the other trials are.
- Which cave is right is never named. A trial completes only if its walk-to loads the goal's room.
- Measured: a walk to (0,84) at 4,619 gives the cave a point at 4,661, room 48 at 4,699, and the goal at 4,795. The offer's walk point gives 4,702 and 4,798. Against the movie, the detour costs 48 frames.

**Monkey Island kitchen, step 5 (E024's G7).**
- The four redirect clicks and object 450 become one move ending at 450. `o` with 428 is a P-D move.
- Redirects are placed with look-ahead, or skipped when they have no point. Actors 4 and 5 cover them in 10 periodic t-ranges.
- 450 has no point when ego stops, so an approach click goes to the screen edge, then 450 at its first point.
- F2 stops step 4 from completing before step 3.
- Measured parts:
  - with `o` placed together with 428, 1,441 of 1,761 t's complete;
  - with the edge walk, 402 of 441 sampled t's complete;
  - the best completion is t 332, frame 485, tick 2,910, the chain's own.
  - Both leave state 367 out of the goal, as a measurement only.
- Unconfirmed: whether step 5 then completes with state 367 in its goal. The door is a script effect, not walking.

## Gates (offline, all before any run)

| Gate | What | Threshold |
|---|---|---|
| G0, base | `e024-fixes` at 69944bd, E024's seed-1 stage | Route-004 byte for byte; end 2,580 / 13,606 / qf 55,555. Measured: pass (`fixes/run-seed1/`). |
| G1, qf | Part 1. The stage re-run on seed 1; a fake engine where a fade makes tick and qf disagree | Routes 001-004 byte for byte, end qf 55,555 (the realtime probe's prediction). The fake picks the earlier qf. Any difference stops the work until it is explained. |
| G2, F1 | `steps_from` with route-004's own plans from j = 0, 9, 14, 21, 25 and 26 | Each ends at 2,580 / 13,606 / qf 55,555, and step 25 completes at 2,151 from every j ≤ 25 (today 2,439). The stage re-run reports its routes. Prediction, unconfirmed: E024's step-15 drop is now kept (−18 ticks). |
| G3, F2 | The kitchen; the demo stage | Kitchen step 4 completes at 341 or later. The goal step is exempt. On the demo the stage still ends at route-004's qf or earlier. Route-001's step 35 moves to 2,725 or later; every other moved completion is listed with its cause. |
| G4, F3 | Labels on movie-c9-run and the kitchen | E024's G2 still 97/97 and 85/85. Every walk anchor's box equals the engine's walkbox on arrival (100%). Walk 2's anchor is (120,87). In the kitchen at t 332, each placed redirect sends ego to its anchor's point (today 0 of 3). |
| G5, F4 | Each `fork_each` child at t against a fresh fork run to t (frame, tick, qf, room, ego, xstart, RNG), on steps 14, 26, 28 and 5 random steps | 100%, or the cause found and fixed first |
| G6, moves | The stage with move labels on and the walk search off; route-004 and the kitchen split into moves | Routes byte for byte against the F1-F4 build; a difference stops the work until it is explained. Each destination placed at its own frame gives the movie's identity (100%). The five within-step chains, the kitchen's [W×4 → 450] and [`o` → 428] are single moves. |
| G7, fake engines | `fmi-shorten/tests` | `Lead(n)` and `FirstPoint` place at the right stop. A waypoint with no point is skipped. Approach, then the destination at its first point. A walk change is kept only for an earlier verified (qf, frame). A failing batch is halved by ddmin. Local acceptance rejects a walk whose later step is not earlier. A plan whose regeneration fails falls back to the E024 plan. One crashed walk candidate leaves the round running. |
| G8, H1 | The stage's levels 1-3 on the probes' walks, with no probe input | Walk 33 ≥ 13 frames and walk 29 ≥ 8 (the probes had 14 and 9). Walks 12, 17, 6 and 2 ≥ 4, 3, 3 and 3 (half of 7, 6, 6, 6). `dir` re-clicks on the 14 probed walks ≥ 68 frames (408 ticks). The beam and the rank check are reported. |
| G9, local acceptance | Level 4 on the probes' variants of walks 2, 6, 12 and 17, as test inputs only | Its verdict equals the full pass's outcome in `walkprobe/runs/c6` for all four: reject 2, 6 and 17, accept 12. Median ≤ 30 s per walk on 3 workers. |
| G10, H2 | Zone C batch: walks 29 + 33 as the stage found them | Verified from boot in both launches, end qf ≤ 55,003 (−138 jiffies). Σ local gain against end gain within 1 frame per walk. |
| G11, clicks | Every kept variant | Each waypoint and re-click taken with NEW_LEG at its decision point. The destination's identity is the movie's. keep(k) and `ego_at` hold. The plan replayed from its own base gives the same events and completion (100%). |
| G12, stuck | G4: the no-skips forward pass. G7: the kitchen under skips from boot | G4: step 25 completes by an approach trial within the movie's step length + 60 frames (measured +48), and the pass ends at ≤ 5,600 frames. G7: step 5 completes on ≥ 80% of every 4th t, and the kitchen goal is reached, verified from boot, no later than the chain's own tick. |
| G13, cost and generality | 3 workers; grep | Screen ≤ 5 s per walk. Full search median ≤ 40 s, max ≤ 120 s. One demo walk round ≤ 1 h. Part I projection ≤ 2 h on 8 workers, from the kitchen's measured per-walk costs. No room, script, variable, object or box numbers in the new code, and no pixel constants beyond `afford.rs`'s. The stage runs on the kitchen's walks without errors. |

- If G8 fails, H1 is refuted. Stop and write up: geometry alone misses the hand gains, which is the case for a port.
- If G12's kitchen part fails only on state 367, the stop names the door, not walking.

## Run, prediction, stop rule

**Run** (laptop, seed 1, 3 workers, nice and taskset as in `fixes/run-seed1.sh`, E024's env.sh, `FMI_ENGINE_BIN=$E13_ENGINE_BIN`, the binary frozen at the merge):
`fmi shorten run --movie experiments/E022-place-context/c9/movie-c9-run.jsonl --archive ~/.cache/fast-mi-e022/play10 --goal end --seed 1 --skips --rounds 3 --walks --walk-rounds 2 --workers 3 OUT`
- The stage runs from the movie: the forward pass, the drop rounds, the walk rounds, then one drop round. No probe movie or probe waypoint is an input.
- OUT is under `~/.cache/fast-mi-e025/run-walks/`. Engine folders are pruned after each test (about 6 GB free on each disk).
- Seeds 2 and 3 run with the same code and are reported only. Seed 2 must still reach the end.

**Prediction for seed 1, in real time.** One walking frame is 6 jiffies. "Now" is route-004 per stretch (realtime probe). The central figure counts engine measurements only. Everything else is in the range.

| Stretch (steps) | Now (jiffies) | Walks | Engine evidence | Central | Range |
|---|---|---|---|---|---|
| Boot to first control | 1,012.25 | – | – | 0 | 0 |
| Room 49 opening (0-3) | 1,008 | 0-3 | Re-clicks +4, +2, +6 and +10 frames; walk 2's waypoint 6 | 0 | in the 0-13 total |
| Rooms 63, 69, 63 (4-8) | 1,950 | 6, 9, 10 | Walk 6: 6 frames, lost at the quiz (+1,002 frames) | 0 | in the 0-13 total |
| Dialogs, the book, the loops (9-13) | 2,719.5 | 12, 14-19 | Walk 12: 7 frames, the one survivor (−78 against its control) | 0 | 0 to −78 for steps 0-13 |
| The quiz (14-19) | 312 | – | – | 0 | 0 |
| The voyage (20) | 1,847 | – | – | 0 | 0 |
| The suit (21-23) | 504 | 22 | +2 frames, before the cave draw | 0 | 0 to −12 |
| The dives (24) | 1,110.75 | script walks only | – | 0 | 0 |
| The cave exit (25) | 851.25 | – | F1 keeps 2,151. The step-15 drop may now be kept (−18 ticks; not walking) | 0 | 0 to −18 |
| Room 48 to the end (26-36) | 2,574 | 29 and 33 (waypoints); 32 and 38 (re-clicks); 30, 34, 35, 37, 39 | −138 verified (29 + 33). Walk 32: 18 frames; walk 38: 5 with `dir` (6 with the beam); each with the next control earlier by the same. The rest not probed (estimate ≤ 2 frames each) | −276 | −138 to −294 |
| **End** | **13,888.75 (3:51.5)** | | | **13,612.75 (3:46.9)** | **13,750.75 (3:49.2) to 13,486.75 (3:44.8)** |

- Central end: qf 54,451, tick about 13,330, frame about 2,534.
- Steps in room 48 complete earlier by each walk's gain. The later steps, re-found, absorb the camera change (measured on walk 33's splice).
- The pre-draw survival rate (H3) is reported: walks accepted at level 4 out of those tested, and kept out of tested at the batch.
- G12: step 25 without skips completes at about 4,795. The kitchen's step 5 completes near frame 485 (unconfirmed with state 367 in its goal).

**Cost** (3 workers).
- Measured units:
  - an accepted candidate costs 14.3 ms + 0.033 ms per frame (`fixes/run-seed1/run-001.json`);
  - walkprobe ran 1,686 arrival evaluations in 30.5 s;
  - `dir` took 23 s for 22 walks.
- One demo walk round:

  | Part | Time |
  |---|---|
  | Trace | 10 s |
  | 29 screens | ≤ 2.5 min |
  | About 14 full searches | ≤ 10 min |
  | Local acceptance on about 18 walks | ≤ 9 min |
  | Zone C batch and verify | about 1.5 min |
  | Zone P batch, ddmin and ≤ 4 singles, each re-found from step 13 or earlier (a pass takes about 3.5 min) | ≤ 20 min |
  | **Total** | **about 45 min** |

- The whole stage: E024's part about 10 min (557 s at 3 workers), two walk rounds and one drop round: about 1.8 h.
- Monkey Island Part I (projection, unmeasured; 10 times the demo's steps):
  - about 290 walks, half with a full search: 290 × 5 s + 145 × 40 s ≈ 40 min on 8 workers;
  - local acceptance runs to the last draw, so its cost grows with the steps after a walk. G13 measures it on the kitchen first;
  - E024's drop rounds (about 3.3 h a round) stay the larger cost.

**Success:** a route found by the stage alone, verified from boot by both launches, ending at `restart@1:582:48:0067` with qf ≤ 55,003. That is at least the 138 jiffies the probes verified. G12 must have passed for both stuck cases.

**Stop rule:**
- **Before any run:** a failed gate is fixed offline first. No run starts with a known gap.
- **During the run:** stop when a walk round gains less than 240 qf, after 2 walk rounds, or after 2 hours of walk rounds.
- **Failures:** a step that fails every fallback inside a test rejects that test. Only a failure of the base pass stops the stage, and it is recorded with its cause. There are no hand patches. Probe movies never become routes.
- **Below the floor:** if the verified end is later than 13,750.75 jiffies, stop. Break the time down per walk and per zone before adding any mechanism.
- **Zone P:** if fewer than 1 in 12 walks pass local acceptance, record the rate. Draw-aware judging is then the next experiment.

## Out of scope

- A walk port (design B). A bridge patch for BOXM, scales, speeds, the walk's target or facing. A reader for BOXD, BOXM, SCAL or CDHD.
- Merging walk chains across steps, or dropping intermediate destinations (design C). This includes room 49's loops (walks 14-16 and 17-19), whose one drop broke a cutscene.
- Relaxing `ego_at`. Destinations chosen among the drawn objects (`OneOf`); the approach trial covers the cave.
- The approach-side family: walk 12's one test came out 1 frame worse, and the end turns are about 8 frames route-wide.
- Draw-aware judging (draws judged as "changed", dialog lines chosen by the drawn values). That is seed 3's step-18 problem, not walking.
- Modelling room 63's script speeds, room 82's script scale, the 180° turn, or vars 465 and 466. The engine judges their effects.
- Any run on Monkey Island Part I; only its cost is projected. The quadratic growth of E024's drop rounds.

## Unconfirmed inputs

- Additivity beyond the chains checked: walks 1→2→3 (21 frames against 20) and walks 29 + 33 (exact). Waypoints and re-clicks on the same walk were never combined.
- Room 48's draws shift in time after walk 32's faster arrival (reclick probe). Whether any later keep(k) sees it is unknown.
- The survival rate before a draw (n = 4), and walk 12's survival once F1 removes the control's 288-frame penalty.
- F2's effect on the demo stage beyond route-001's step 35.
- The cause of the step-14 fork label clash.
- Why the strict scan of step 25 found nothing from base 1839 in 2,614 t's (`run1.err:155`) but found t 2,356 from base 1833 (`run1.err:189`).
- Whether the kitchen's step 5 completes with state 367 in its goal.
- Part I's walk count, draw count and box shapes.
