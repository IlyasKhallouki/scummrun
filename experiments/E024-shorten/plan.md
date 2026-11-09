# E023: the system shortens its own route, re-finding each step from boot with skips on

Date planned: 2026-11-09
Research question it serves: RQ2 (optimization), RQ4 (generality)

## Question

Can the system shorten its own route to the demo's end to under 4:00 of game time (14,400 ticks) on seed 1, with no help from outside tools? The route must still be verified from boot by the engine. To do that, the system replays what each step achieved, not where it clicked, as early as the engine accepts. Text and cutscene skips are on from boot.

## Problem (what E022 left)

- **The run's own route:** `c9/movie-c9-run.jsonl`, 97 clicks, ending at frame 30,785, tick 182,260 (50:38).
- **Four click gaps are waits caused by the explorer's own window search:**
  - 2519→9722 (7,203 frames), 10731→17965 (7,234) and 23352→30553 (7,201).
  - 19338→22979 (3,641), which by its length looks like one more empty window.
  - The game accepts the next click at once: control returns at 2517 and 10711.
- **Why the waits happen:**
  - A ready window runs on to WINDOW_FRAMES (window.rs:32, end at :2632) after its 294 tries have stopped (:1867-1868). Nothing in the ready block (:2641-2673) breaks on that.
  - The affordance step then runs on the attempt's end engine (tryjob.rs:1366-1383), so the trial it keeps carries the wait.
- **The route without the waits:** about 5,500 frames remain (no skips). `compress.py`, which runs outside the system, took the same chain to 5,123 frames plus its waits.
- **Later clicks cannot just be shifted earlier:**
  - Random draws change: room 82's cave (getRandomNr(6)) and the quiz's Var[239]/[240] (global 98).
  - Actors on random walks and the room's scroll position also change.
  - Skips switched on before frame 18741 broke the fixed clicks.
  - So every later step is found again from the new state.

## Prior work (research workflow wf_94553b80-c1c; designs A, B and C with three judges, workflow wf_c878237c-51c, 2026-11-09)

- **Methods this plan uses:**
  - A per-frame earliest scan from a fork, judged locally (TAS greenzone; gb-tas-gen DelayMoveSegment).
  - Inputs anchored to what they act on, not to pixels (SleepReplacer).
  - A drop kept only if the end, verified from boot, comes earlier (ddmin, action elimination; D9).
  - The fastest arrival kept per cell (Go-Explore).
- **Limits:** whether a move works is not monotone in time, so there is no bisection and no sampling. The earliest completion per step is not a global optimum (Orda-Rom). This experiment gives verified improvements, not an RQ2 bound.

## Change

### Part 1: the explorer stops making waits (`FMI_READY_END=1`, off by default)

- **The window ends when its tries are spent.** In the ready block of window.rs, after `self.resolve` (:2642): when `ready.stopped` is set and no try was chosen, the window ends with "ready: forking stopped, no try left".
  - This is safe: resolve has already cancelled every try after the cursor (:1875-1882).
  - E013's refused-input end check (:2857) accepts this reason too.
- **The affordance step runs at P0.**
  - P0 is the start of the attempt's trailing empty windows, read from att.res["windows"] (tryjob.rs:1337).
  - The step runs on an engine forked from the try's root and played to P0, instead of the end engine (tryjob.rs:1366-1383). Checkpoints after P0 are not committed.
  - If the state at P0 differs from the attempt's (the check in engine_at, :1484), it falls back to the end engine.

#### Part 1 as built (amendment 2, 2026-11-09; merge 2c7e305, reviewed: ship)

- **P0 counts only windows that gave up** (mech.rs:242, :250): those that ended at the frame bound, at READY_END or on a refused input. With every empty window counted, j08932 got P0 = 18322 and lost its 18324 and 18802 entries. In that try the 18802 window ended because a precondition closed, so the game had moved on by itself. A delay-fork "refuted" end does not count, which falls back to the old behaviour.
- **G1** (each try re-run with the switches it originally ran with; with the switches off the re-runs match the archive's window records in every field but wall_s):

  | Measure | Plan | Off | On |
  |---|---|---|---|
  | j08920, first empty window ends | near 11059 | 14364 | 11059 |
  | j08920, P0 | 10764 | – | 10764, reached again |
  | j08920, kept entries | ≤ 10800 | 17971–18036 | 10772, 10787, 10837, 10837 (two miss, by 37 frames) |
  | j08758, dialog entry | ≤ 2600 | 9772 | 2574 |
  | Windows that chose a try | unchanged | – | j08920 3/3, j08932 2/2 |

  - Monkey Island kitchen: 8 of 8 entries verified both ways. Smoke: 23 entries identical both ways. Part 1 never fired in either, so these checks show only that nothing else changed.
- **Open:**
  - The 4 entries from afford at P0 on a fork replayed from the root differ from a replay from boot on var:56 (−1 against 0). var:56 is the sound-result variable, E020 amendment (4)'s open mismatch. The run's checks skip it (FMI_SKIP_SOUNDRESULT), but `fmi check verify-new` does not. Afford at the root and the attempt's own entries verify.
  - A reach extension at 11059 that the old path never needed made j08920 cost 247–718 s, against 28.5 s.
  - "The same empty window 2 times" now fires after about 600 frames, not 7,200. The attempt no longer sees anything the game reaches by itself between those. Repeating only on a changed state (A2) stays out of scope.
  - Two fmi-play tests in tests/realize.rs already failed before E024 (28a91e0 changed realize.rs without them). refuse_check.rs and e013_report.rs do not know the new end reason yet.

### Part 2: `fmi shorten` (new crate `fmi-shorten`, subcommand of `fmi`)

It runs as the run's last stage once a goal entry exists (driver.rs:664 stops at the first goal entry today), and as a subcommand on an existing archive. The inputs are a movie from boot, the run's archive folder (its stage atoms, idle keys and readings) and a goal: the game's end at its site, or the archive's goal facts. Each output is a new `route-NNN.jsonl`; earlier versions are never overwritten.

1. **Steps** (chain.rs):
   - One replay of the input movie from boot.
   - At each control stop (Status.idle) after an input, the progress is the room, ego's objects and Archive::held (archive.rs:983).
   - The inputs between two progress changes form one step. A step that changes nothing merges into the next.
   - No archive chain is needed, so the local play10 copy is enough.
2. **Anchors** (label.rs):
   - For each click: its hit at the frame before (at_many, lib.rs:963; afford::Hit, afford.rs:76) and its input-log record after (lib.rs:1004).
   - Anchor kinds: Verb{id}; Object or Actor plus its Sentence touch; Walk{room point, box}; Dialog{verb id}; Key. Raw is used only when there is no hit.
   - Timing class: "at an idle stop", or "as soon as possible after the last taken input".
3. **Draw keys** (label.rs):
   - For each step, one twin fork with a perturbed RNG (set_rng, lib.rs:919, used only for this analysis) replays the step.
   - Keys whose values differ are draw keys. They are judged as "changed from the root", not by value.
   - The result is cross-checked against fmi-reading draws.rs.
4. **Verdicts** (verdict.rs):
   - **Click:** the input log says Taken with the same identity. A refused click costs about 1-3 ms.
   - **Step:** keep(k) is e_k's values on every key the kept steps up to k changed. It runs as RunSpec.until (lib.rs:687) with max_frame = best−1.
   - **Last step:** the goal, with the same end reason and site. All times are Status.tick.
5. **Scan** (scan.rs):
   - From E_{k−1}, fork at every frame t from the last input up to best−1 (fork_each in chunks, lib.rs:929; 8 workers).
   - At each t, re-place the first anchor (afford::offer_cfg, afford.rs:200; a walk uses its room point minus xstart, checked with `at`).
   - Place the inner anchors by their timing class.
   - Keep the earliest completion by (tick, frame).
6. **Fallbacks** (refind.rs), used when nothing completes within the original step's length plus afford::W (afford.rs:60). In order:
   - realize (realize.rs:260);
   - offer trials up to the original step's length (afford.rs:297, :975);
   - a depth-first search over the offered dialog verbs, at most 2,000 leaves;
   - the step again with skips off for its span (Skips{0,0} at its start, {1,1} after it).
   
   If all of them fail, the stage stops and names the step. Clicks are never shifted.
7. **Reductions** (reduce.rs):
   - **Inside a step:** drop each inner input.
   - **Across steps:** drop single steps, room loops (X→…→X) and ddmin halves. They are tried in order of measured waste: idle time from control to input, plus the bridge's skip records.
   - **Testing a drop:** the rest of the route is found again, with a cheap anchored replay first and the full scan only if that replay reaches the goal.
   - A drop is kept only if the verified end comes earlier. At most 3 rounds.
8. **Skips from boot** (host.rs):
   - Add SPEEDRUN_START_FRAME and SPEEDRUN_TIMER_NEXT to the pass-through list (:111). extra_env is applied last (:438), after TIMER_NEXT=any (:411).
   - The segment starts at frame 1 (patch 0001 :663, :979) with Skips{1,1} at frame 1.
   - TIMER_NEXT is set to Status.v19 at the first control of a boot without skips. The value is measured from the game, not written in.
   - If the start fails the var-19 check, the start moves to the first frame where v19 equals that value.
   - The bridge's check at the first idle frame after a cutscene skip (patch 0001 :976) then bans the logo-glitch family on any game.
9. **Verify** (movie.rs):
   - Each version boots fresh twice: once with the bridge's launch, once standalone.
   - Both must end at the same site and tick, with the stage's digest at every step's completion (lib.rs:1008).
   - A movie the stage did not produce has no matching step log, so it fails.

## Gates (offline, before any full stage run)

- **G0, settings (one boot each):**
  - (a) START_FRAME=1 without skips: first control at frame 992 and tick 3,458, with the standard boot's digest.
  - (b) With skips from frame 1: first control at 1,200 ticks or less. Prediction about 900. Design A's reading lists overrides in global 74, room 75 (locals 200, 204, 206) and room 49 (locals 207, 212, 213).
  - (c) v19 at that control equals the value without skips.
  - If (b) is over 2,100 ticks (35 s), stop and write up before building the scan.
- **G1, Part 1:**
  - Fork replays of j08920 (entry 24147) and j08758 (entry 17633).
  - The first empty window ends near 11059, not 14364.
  - The kept captain-talk entry is at frame 2,600 or less (now 9,772). j08920's P0 is 10764 and its kept entries are at frame 10,800 or less.
  - Archived windows that made a choice are unchanged.
  - `cargo test` passes with the switches off. With them on, mi-check.sh `kitchen` and `smoke` lose no entries.
- **G2, labels:**
  - Every click of movie-c9-run gets an anchor.
  - Re-placing each anchor at its own frame gives the same hit and identity: 100% required.
  - Each keep(k) holds at the original completion frame, with the original timing.
  - The twin fork and draws.rs both mark the book step and room 82's entry.
- **G3, fake engines (fmi-shorten/tests):**
  - The scan finds the earliest completion when t=5 fails, t=6 works and t=7 fails.
  - Pruning at best−1 never loses an earlier end.
  - A reduction is kept only for an earlier verified end.
- **G4, the forward pass without skips:**
  - The run's route, found again from boot, ends at 5,600 frames or less. Prediction: about 5,500, with all four waits gone.
  - After reductions: 5,150 frames or less. That is compress.py's 5,123 non-wait frames on the same chain, now matched inside the system.
- **G5, the first stretches under skips:** stretches 1-4 (through the book) end at 9,000 ticks or less. Prediction: about 6,200, against 12,608 now.
- **G6, cost:**
  - A refused candidate costs 3 ms or less.
  - An accepted one costs 3 ms plus at most 0.15 ms per frame.
  - A forward pass takes 10 minutes or less of wall time on 8 workers.
  - Projected to Monkey Island Part I (about 10 times the steps): 2 hours or less.
- **G7, generality:**
  - fmi-shorten contains no room, script, variable or object numbers (checked with grep).
  - On E012's Monkey Island kitchen chain (`/var/tmp/fast-mi/e012-archives/kitchen`, with its goal facts), every click gets an anchor.
  - Under skips from boot, the stage reaches the kitchen goal, verified from boot, no later than the chain's own tick.
  - The var-19 rule either passes or moves the skip start past Monkey Island's logo.

## Run, prediction, stop rule

- **Run (laptop, seed 1):**
  `fmi shorten run --movie experiments/E022-place-context/c9/movie-c9-run.jsonl --archive ~/.cache/fast-mi-e022/play10 --goal end --seed 1 --rounds 3 --workers 8 OUT`
  - Engine settings are those of E022's env.sh.
  - The input is the run's own route. movie-c9-optimized and compress.py's outputs are used only as numbers to compare against.
- **Prediction per stretch, in seconds.** "Now" is the optimized route's trace with no skips (only the last stretch already had skips).

  | Stretch | Now | Under skips, no waits |
  |---|---|---|
  | Boot to first control (rooms 68, 0, 75, 49) | 57.6 | 15 (8-35) |
  | Room 49 opening, mostly walking | 33.0 | 22 |
  | Rooms 63, 69, 63, walking | 27.9 | 26 |
  | Room 49 dialogs and the book | 91.6 | 40 (28-60) |
  | Waits (two in this trace) | 1,440 | 0 |
  | Quiz to departure | 62.2 | 18 (12-30) |
  | Voyage and arrival in room 42 | 27.4 | 8 |
  | Room 42, the suit | 30.0 | 18 |
  | First dive and return | 16.4 | 8 |
  | Room 42 before the second dive | 50.0 | 7 |
  | Second dive, dark room, the end (already skipped) | 71.0 | 57 |

  - The sum is about 219 s.
  - The run's own chain has about 430 more non-wait frames than this trace, about 40 s.
  - Forward pass alone: about 4:20. After reductions: about 3:40 (13,200 ticks), range 3:05 to 4:30.
- **Success:**
  - A route from boot, found by the stage alone and verified twice, that ends the demo at the same site in under 14,400 ticks on seed 1.
  - Then seeds 2 and 3, with the same input movie and the same code: each must reach the end, verified from boot. Their ticks are reported but not tuned for. The objective stays seed 1.
- **Stop rule:**
  - Stop when a round gains less than 1 s, after 3 rounds, or after 2 hours of wall time.
  - If a step fails every fallback: stop, record the step and its cause, and fix it offline. No hand patches.
  - If the forward pass under skips is over 4:30: stop and break the time down per stretch from steps.jsonl before adding any mechanism.

## Out of scope

- **Later additions, only if needed:**
  - Draw-class beams and verdicts that relax and backtrack (design B): only if G4 or G5 shows that a draw or a strict keep(k) costs time.
  - Model tries and window re-solves as fallbacks (design C): the next experiment, if a step fails every fallback above.
- **Other explorer changes:**
  - Repeating an empty window only when the state changed (A2). With Part 1, an empty window costs about 300 frames.
  - Fastest arrival per cell in selection (A4).
  - A bridge patch for skips before the segment start (A5).
  - The reader's auto_esc (A7).
- **Not used at all:**
  - The VM and play10's C8-C9 archive.
  - The ladder, e016 asap and g4, and the Python scripts. They are outside instruments.
  - An RQ2 bound.
