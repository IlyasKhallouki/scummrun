# E031: the system shortens its own route, glitches allowed

Date planned: 2026-11-16
Research question it serves: RQ2 (optimisation). Stage 2's boot scan also serves RQ3, in a limited sense: the system
is given no key, frame or variable for the logo glitch, but the scan's form (one key at each boot frame, judged by
ticks) was chosen by people who know such a glitch exists on this game. A find by stage 2 shows a general scan can
locate it, not that the system discovered the class.

## Question

E027's run full5 finished Monkey Island (DOS CD) from boot with the search alone: 1,994 clicks, bit 565 at frame
114,978, tick 694,701 (3:12:58). The search optimises for reaching the end, not for time.

Can the system shorten that route by itself, under the Any% rules (`rules/anypercent.md`), by at least a quarter of
its game time to bit 565? The conditions:

- no language model anywhere in the running system;
- no move written by hand: every input of a new route comes from full5's movie, from the unchanged search binary,
  or from a general scan the shortener runs;
- the search's code and results unchanged: `experiments/E027-direct/run/gate.sh` gives PASS;
- every version replayed from boot.

The metric is game time in ticks at the first frame where bit 565 holds, and that frame. Real time (qf) is reported
beside it.

## Problem (what full5 left)

| Part of full5 | Frames | Ticks | Source |
|---|---|---|---|
| Boot to first control, intro not skipped | 2,727 | 16,321 | trace `segment_start`, frame 2,728 |
| First control to the first frame of bit 565 | at most 106,102 | at most 641,492 | see the next line |
| After bit 565 to the reported end | at least 6,148 | at least 36,888 | script order, below |
| Idle between a control return and the next input group | 48,553 | 291,318 | trace, 1,415 groups (median 7 frames, p90 73, max 879) |
| inside it: probe padding (399 `~` stops, 60 + 6 frames) | at most 26,334 | | nodes.jsonl |
| partly inside it: explicit waits (15 × 50, 11 × 200, 7 × 800; nominal 8,550) | 8,751 measured | | nodes.jsonl; the seven 800s lie inside (control to click 813-879 frames), most 50s do not (a control return comes 1-53 frames before their click) |
| inside it: calm (about 6 frames per ordinary trial) | about 3,800 | | nodes.jsonl |
| Loops whose ends are equal by a proxy | 4,274 to 24,774 | | route report, proxies (a) to (c) |
| Verb click to object click, gap minus 1 | at most 1,801 | | 418 pairs |
| Last click (108,579) to bit 565 | at most 251 | | trace |

- "Trace" is full5's replay from boot, `/var/tmp/fast-mi/direct/video-full5/verify-out/eng/b0/trace.jsonl`; the
  idle and qf lines were measured from it for this plan. "nodes.jsonl" is full5's tree, `leg00..leg03`.
- **The reported end is late.** Global 132 sets bit 565 at [0142]. At [015D] it starts global 137, whose [004B]
  loadRoom(85) shows in the replay at frame 108,830, tick 657,813. The search checks `--until` only at a trial's end,
  and the last trial ran on to its own bound at 114,978. So bit 565 holds by frame 108,830 at the latest. No break,
  delay or wait lies between the write and that room load (132's [0142] to [015D], then 137's [0000] to [004B]), so
  the write very likely falls in frame 108,830 itself; stage 0 measures it. The 6,148 frames after it are not route
  time.
- **Most of the rest is waiting.** The replay trace has 1,485 control returns (sentence finished). 1,415 input groups
  start after one. The frames between the control return and the group's first input sum to 48,553 (45% of the route
  after first control). 404 of these gaps are 70-79 frames (probe and calm), 813 are under 10.
- **Ticks almost equal real time here.** In the replay trace (fast launch), qf exceeds 4 × tick by only 77.75 jiffies
  (1.3 s) at frame 114,578. On the Fate of Atlantis demo, each fade cost 39 qf in the video launch. Not yet measured
  in Monkey Island's standalone launch.
- **The search tree holds few ready shortcuts.** Merging all 58,279 kept stops by the K3 proxy saves 27,317 frames,
  against 24,774 from the path's own loops. Off-path stops add only 2,543.
- **Scale.** The human Any% record is 21:52.4 (about 78,700 ticks). Even the loosest credible proxy bound (all K1
  loops, probes and waits removed) leaves about 41,300 frames (about 69 min). Pruning and re-timing cannot reach the
  human scale. This experiment measures how far they go.

## Prior work (four research reports for this plan, 2026-11-16)

- **In the project.**
  - E024's forward pass (`experiments/E024-shorten/plan.md`, code in `fmi-shorten/src`) is the only shortener the
    project has run end to end. On the Fate of Atlantis demo: 182,260 ticks to 14,152 in 79 s, then 13,606 after
    drops (3 of 110 kept, −546 ticks). Each version verified from boot by two launches.
  - It does not carry over to full5 as built:
    - On E012's Monkey Island kitchen chain (G7) it stopped at step 5 of 12: clicks made while ego walks had no point
      at 1,680 of 1,761 frames.
    - Its drop rounds grow with the square of the step count. Projected: about 10 h a round at full5's size (about
      670 steps).
    - E025's fixes (F1 to F4, `Lead(n)`, `FirstPoint`, approach clicks) are designed and not built
      (`experiments/E025-walking/plan.md`; F4, the fork label clash, has no known cause).
  - E024's skips-from-boot rule (`skips.rs`) requires the measured var 19. That bans the logo glitch.
  - The old Part I optimiser (`docs/optimization.md`) used a hand-written action model. It cannot be reused here.
    Its lesson: choose by measurement, and key walk costs by where ego starts (−7% on Part I).
- **Glitches and Any% mechanics** (`docs/anypercent/findings.md`, `docs/comparison.md` §1.4, `routes/anypercent/`,
  `docs/research/anypercent-engine-mechanics.md`, `docs/research/anypercent-runner-sources.md`).
  - Logo speed glitch: an Esc on boot frames 46-92 (DOS CD under the bridge) leaves var 19 at 5. Frame-paced motion
    then runs 6/5 as fast; tick waits keep their length. Upper bound on full5: about 1/6 of 641,492 ticks, about
    106,900. The bridge delivers it as one raw key with `SPEEDRUN_TIMER_NEXT=any`.
  - Intro skip, two ways in the bridge. (a) `SPEEDRUN_START_FRAME=1` (patch 0001, E024's `skips.rs`): the segment
    starts at frame 1, so the skips act from the boot. Measured on Monkey Island (commit 054378d): first control at
    frame 110, tick 660, var 19 = 6, against 2,728 / 16,321. (b) Patch 0013 (`speedrun_skip_early`, through
    `FMI_GAME_INI`): the skips press before the segment start. Measured on the MI2 demo (E029) only, not yet on
    Monkey Island.
  - Random draws can be steered by input timing alone: the RNG is one xorshift that advances only through script
    draws and a few engine effects (a dissolve draws once per pixel). Delay look-ahead took room 39's exits from
    59 s to 12 s on the hand-routed A run.
  - Walking arithmetic (truncated clamp on diagonals, fraction reset on re-clicks, nearest pixel of an exit) is
    general engine arithmetic. Re-clicks and waypoints are allowed in Any%.
  - Credit Early is already in full5, found by the search (bit 320 at frame 20,735 after 11 handle sentences).
  - The item slide, and the oars, monkey and feather skips built on it, need setups that only game knowledge finds
    today. The direct search cannot form a slide (it schedules no keys; the auto Esc ends every reading at once).
- **The route** (a python pass over full5's `leg00..leg03/nodes.jsonl`). Summarised in the Problem table. Also: 109
  A→B→A bounces (45 with no gain and the same objects, 14,701 frames); the 4→15→16→11→16→15→4 cycle walked 5 times;
  the five longest gaps between object gains hold 48,226 frames (43%), three of them across a leg boundary.
- **Outside the project.**
  - TAS practice: a movie of frame-exact inputs, a greenzone of savestates, luck by delaying inputs, and brute force
    in narrow windows only (FCEUX TAS Editor; TASVideos LuckManipulation; TMInterface bruteforce; Radeline).
  - Plan post-optimisation: Action Elimination and Plan Neighborhood Graph Search improved 60 of 270 IPC-2008
    plans, best on loosely coupled subtasks (Nakhost and Müller, ICAPS 2010). Exact reduction is NP-hard (Balyo et al.
    2014; Salerno et al. 2023).
  - Reduction that keeps inputs valid: reduce the generator's choices, not raw events (MacIver and Donaldson, ECOOP
    2020; GReduce 2024). Here the generator's choices are the search's trials.
  - Go-Explore keeps the shortest trajectory per cell (Ecoffet et al. 2021). Path shortcutting (Geraerts and
    Overmars 2007) and Sokoban vicinity search (JSoko) search near a found path.
  - Routing is NP-hard in general (Lafond 2018). Edge costs depend on the route so far, which defeats a fixed
    graph search (Groß et al. 2021). Cook et al. (2025) note that TAS runs are made by hand and suggest making time
    part of the objective to find glitches.
  - Sources: https://tasvideos.org/LuckManipulation, https://fceux.com/web/help/taseditor/Glossary.html,
    https://github.com/Kataiser/radeline, https://ojs.aaai.org/index.php/ICAPS/article/view/13402,
    https://ojs.aaai.org/index.php/SOCS/article/view/18318, https://proceedings.kr.org/2023/80/kr2023-0080-salerno-et-al.pdf,
    https://www.st.cs.uni-saarland.de/papers/tse2002/tse2002.pdf, https://www.doc.ic.ac.uk/~afd/papers/2020/ECOOP_Hypothesis.pdf,
    https://arxiv.org/abs/2402.04623, https://arxiv.org/abs/2004.12919, https://arxiv.org/abs/2106.01182,
    https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.FUN.2018.27, https://ceur-ws.org/Vol-4090/paper1.pdf.
- **What this plan takes.** Re-timing against the engine's own control signal (TAS earliest frame). Luck by delay.
  Drops over trials, never raw clicks (Action Elimination, ddmin). The unchanged search as the move generator in
  bounded corridors (PNGS, path shortcutting). Ticks as the only judge. No claim of an optimum.

## Hard constraints

- **No language model.** None in the shortener, none in the search.
- **No move written by hand.** Each input of a new route is one of:
  - an input of full5's movie, kept unchanged or moved to another frame;
  - an input made by the unchanged search binary;
  - an input made by a general rule over the route's own inputs or the engine's key set (the boot scan, re-clicks of
    a click's own target, another pixel with the same hit).

  The shortener's code holds no room, script, variable or object number (checked with grep, as E024's G7). It reads
  no human route (`routes/`, `docs/human-route.md`, `docs/anypercent/findings.md`).
- **The search unchanged.** No change to `fmi-direct`, `fmi-core`, `fmi-engine`, `fmi-explore` or the engine
  patches. New code goes in `fmi-shorten` and the `fmi` binary, which the search does not depend on
  (`fmi-direct/Cargo.toml`). If a shared crate must change, the stage stops and the owner decides.
- **Every result replayed from boot** (below, "Verification").

## Method

### Common parts

- **Inputs.**
  - `experiments/E027-direct/results/full5/end-stop.jsonl`: 3,916 events. It is converted once to one event per line
    (`read_movie`'s format).
  - full5's tree, `/var/tmp/fast-mi/direct/full5/leg00..leg03` (stops, own events, news, edges, toggles).
  - `experiments/E027-direct/guide/guide-end.json`: only its 805 key names.
  - The goal `bit:565=1` as `SPEEDRUN_GOAL='[{"bit":565,"in":[1]}]'`. The bridge ends at the first frame where it
    holds.
- **Engine and settings.** full5's: svm-e027 with patch 0013, E027's `env.sh` (FMI_MEM_FILES,
  FMI_SKIP_SOUNDRESULT), seed 1, text speed as full5 (var 37 = 0), `SPEEDRUN_TIMER_NEXT=any`. Only stage 2 may add
  a boot setting (`speedrun_skip_early` or `SPEEDRUN_START_FRAME`) and boot inputs.
- **Units.**
  - A control return: the bridge's control record (1,485 in full5).
  - An input group: the inputs from one control return to the next (1,415 groups start after a control return).
  - A trial: one kept stop of the search on full5's path (1,037).
- **Group verdict (local).**
  - Every click of the group is taken with the same identity as in full5 (E024's click verdict).
  - At the next control return, the 805 guide keys equal full5's at the matching control return. Idle keys (keys
    that change by themselves, from stage 0) are left out.
  - Plus the group's draw keys: every global variable, bit, owner, state or class (not only the guide's 805) that
    differs in one of the group's twins (stage 0). They must equal full5's, by value first, then "changed from the
    root" (E024 amendment 3). The guide alone misses draws: the store's four combination digits (room 30's local
    205 writes vars 221-224 at its first entry, frame 3,081) are not guide keys, and they are used only at the
    safe, bit 320 at frame 20,735, about 17,650 frames later.
  - Ego stands where full5's ego stood (E024's `ego_at`).
- **Acceptance.** In stages 2 to 5, a candidate is kept only if the route, replayed from the nearest kept fork,
  reaches bit 565 at an earlier tick. Ties are broken by frame. Stage 1 accepts group by group by the group verdict,
  and checks the whole route once at the end of its pass.
- **Verification.**
  - Each version a stage writes is replayed from boot by two launches (the bridge's and the standalone). Both must
    reach bit 565 at the same frame and tick, and give the step log's digest at every control return.
  - `fmi shorten verify` takes the engine's settings only from the step log's first line. That line carries every
    setting the route used: `SPEEDRUN_GOAL`, `SPEEDRUN_TIMER_NEXT=any` (a measured value would fail a var-19 = 5
    route), and the boot setting (`SPEEDRUN_START_FRAME` or `FMI_GAME_INI=speedrun_skip_early=true`).
  - Each stage's last version is also replayed by the search's own binary, unchanged: `direct verify` on a
    one-line stop file, twice. At B (the version's bit-565 frame) with `bit:565=1`, and at B − 1 with `bit:565=0`.
    `direct verify` checks keys only at the frame it is given, so the second run is what shows B is the first frame.
- **Outputs.** Every version is a `route-NNN.jsonl` with its `.steps.jsonl` in E024's format. Earlier versions are
  never overwritten. E024's video tool (`experiments/E024-shorten/video/make.sh`) can then render any version.
- **The laptop.** Each stage is one run of at most 3 h, under the heavy lock (the video work's jobs run serially
  with it), with an inhibit lock against sleep and a watcher on the run.

### Stage 0: measure (changes nothing)

- **What the system does.** It replays full5 from boot with the goal set. At every control return and every stop it
  records frame, tick, qf, var 19, the RNG state, the state digest, the 805 guide keys, ego's position, box and mv,
  room and camera.
- **Forks at each control return (no input).**
  - A 200-frame idle fork: the keys that change by themselves (idle keys), and whether the RNG moves each frame.
    Moving means a delay there can steer later draws.
  - Three twin forks with a perturbed RNG (E024's `twin`, `set_rng`), which replay the next group: the group is
    draw-marked if any global variable, bit, owner, state or class differs at its end, not only a guide key. The
    differing keys are that group's draw keys.
- **The standalone launch once,** for the qf of fades in Monkey Island.
- **Identity test.** The stage-1 re-timer with nothing removed (every group at its original offset) must give full5's
  digests at all 1,485 control returns and the same bit-565 frame.
- **Metric.** B0: the first frame where bit 565 holds, and its tick. All later stages are measured against B0, not
  against 694,701.
- **Cost.** About 6,000 short forks plus three replays: 30 minutes or less.

### Stage 1: re-time every input group against the engine's control signal

- **What the system does.** It walks the route group by group from the boot. It places each group's first input
  one frame after its control return, and keeps the group's inner offsets. This removes probes, calm, waits and
  idle leads wherever the game does not need them.
- **If the group verdict fails,** in order:
  1. Padding ladder: give back 1/8, 1/4, 1/2 of the original gap, then the whole gap. Success is not monotone in the
     gap, so this is a fixed ladder, not a bisection.
  2. If the group is draw-marked: delay scan, d = 1..D frames before the group, value first. D = the frames removed
     since the previous draw-marked group, at most 2,000. This is meant to bring the RNG back to the draw full5 had
     (luck by delay).
  3. If both fail: restore full5's own gaps for every group since the last control return whose state digest
     (RNG included) equalled full5's, at worst since the boot. From there the route replays full5's own inputs
     from an equal state, so the pass goes on and always ends with a complete route. The group is recorded (room,
     kind, the keys that differ, the RNG motion) as a candidate for stage 4. No hand patch.
  4. Only if step 3 still fails (state outside the digest, such as a CD position): stop the pass there.
- **Checks along the way.** After every 50 accepted groups, a replay from the last kept fork to the next
  draw-marked group, with the rest at full5's gaps, comparing the guide keys and that group's draw keys. Not to
  bit 565: an unrepaired later draw would fail that check every time the RNG moves per frame, and ddmin would then
  give back padding that the pass's own delay scan would have repaired. If it fails, ddmin over those 50 groups
  finds the group that broke and restores its padding. The whole route to bit 565 is checked once, at the pass's
  end.
- **May change:** the frame of each input group (earlier, or later by the delay scan).
- **May not change:** any input's kind, pixel or key; the order of inputs; their number; any setting.
- **Pool.** At most 47,138 frames (48,553 minus 1 per group), about 283,000 ticks.
- **Cost.** A pass with no repairs should take 2 minutes or less (1,415 groups of about 75 frames each, plus forks).
  With repairs, 30 minutes or less (gate G4). Wall budget for the stage: 1 h.

### Stage 2: the boot: what to press, and when, before first control

- **What the system does.** A scan of single boot inputs.
  - For each boot frame f from 1 to 2,727, and each key k in the engine's key set, it places one raw key k at frame
    f. The skips are switched on at f + 1, with `speedrun_skip_early` (patch 0013). G2 first checks on Monkey Island
    that 0013 with skips at frame 1 gives E024's first control (frame 110, tick 660, var 19 = 6), measured so far
    only with `SPEEDRUN_START_FRAME=1`. If it does not, the scan uses `SPEEDRUN_START_FRAME` instead.
  - Each candidate is a fork of one boot at frame f − 1, not a fresh boot.
  - The key set is the bridge's raw key names (Esc, space, Enter, the printable keys) that the game's scripts or the
    engine's input handler read. The system takes them from the scripts' dump and the engine's key variables; none
    is written in.
  - Plus "no key, skips on at frame f". That includes the plain intro skip (f = 1).
  - Each candidate runs to first control (at most 2,728 frames, about 0.07 s).
- **How it chooses.**
  - Candidates are grouped by their state digest at first control.
  - The 50 classes with the earliest first-control tick are probed; the rest are counted and reported. Each is
    probed with stage 1's re-timer over the route's first 50 input groups. The probe's tick shows any
    lasting speed change, var 19 among them, without the system knowing what var 19 is.
  - The three best groups by probe tick get a whole-route re-time (stage 1). The kept boot is the one whose route
    reaches bit 565 earliest, verified from boot.
  - Var 19 is only reported.
  - Keys that already differ at first control between the kept boot and full5's boot (var 19 among them, if it
    changed) are left out of the group verdict for that route. Otherwise every group would fail on the boot's own
    change.
- **May change:** the boot inputs before first control, the skip-early setting, and then the timing of every group
  (stage 1's rules).
- **May not change:** any input after first control except by stage 1's rules.
- **Cost.** At most 2,727 × (key count + 1) short forks. With 8 keys, about 25,000, about 30 CPU-minutes at most
  (a whole boot to first control is about 0.07 s at full5's replay speed). With every printable key (about 98) it
  would be about 270,000; the key set then needs the scripts' reading before the scan runs.
  Then up to 3 whole-route re-times: 2 h or less in all.
- **Why here.** Each boot choice moves every later frame and re-rolls later draws. It is settled before drops, so
  drops are found under the final timing.

### Stage 3: drops over trials (Action Elimination, ddmin)

- **What the system does.** It removes whole trials, or inputs inside a trial. It then re-times the rest with stage
  1's rules. It never removes raw clicks across trial bounds.
- **Candidates, in order of the frames they would save:**
  1. Exact cuts: two control returns with an equal state digest. Everything after is then the same game, only
     earlier. These are expected to be rare, since the digest includes the RNG.
  2. Loops whose ends are equal on the 805 guide keys minus idle keys (K3-like). The route report counts 110 to 136
     such loops on full5.
  3. A→B→A bounces with no news and the same objects (45 on full5, 14,701 frames).
  4. Single trials with no news (771 on full5; 818 gain no bit, owner or level), then ddmin halves, quarters and
     eighths over runs of them.
  5. Inputs inside a trial: count repeats (27 trials, 146 clicks), and repeated sentences such as the 11 handle
     sentences before bit 320.
- **Test of one candidate.**
  - Local only: drop, re-time the next trials up to the next trial with news (at most 50 trials). The state there
    must rejoin the base route's on the guide keys and the draw keys (draw keys relaxed as in the group verdict).
  - No candidate gets its own end-to-end run. Each end-to-end run is a whole stage-1 re-time with its repairs
    (up to 30 minutes by G4), so one per candidate would not fit.
- **Rounds.**
  - Each round tests all candidates locally against the same base, in parallel.
  - The local passes are combined in order of local gain and re-timed end to end together, to bit 565. A failing
    combination is split by ddmin. The end-to-end runs a round may use are capped so that three rounds fit the
    stage's 3 h at G4's measured cost of one whole re-time; when they run out, the round keeps the largest
    combination that passed.
  - This avoids E024's square law: one base per round, not one per candidate.
- **May change:** remove trials or inputs; re-time.
- **May not change:** add or alter any input.
- **Cost.** Gate G4 projects it from stage 1's measured costs. Planned: 3 h or less for at most 3 rounds.

### Stage 4: corridors re-searched by the unchanged search

- **Where.**
  - The ten longest stretches between consecutive trials that gained an object or a flag, on the current route.
    On full5 the five longest hold 48,226 frames.
  - Any group that stage 1 could not repair.
  - Before any search, the tree's own earlier equal stops (K3 off-path stops, about 2,543 frames on full5; the
    leg-boundary detours) are tried as splices. Their movies are in full5's original timing, so each is first
    re-timed by stage 1's rules. They cost one end-to-end re-time each.
- **What the system does.**
  - It writes the current route's stop i as a one-line stop file with the fields `--from` reads: `id` 0, `frame`,
    `room`, `news`, `ever` (what was owned on the way) and `events` (its movie from boot).
  - It runs the frozen search binary unchanged: `high9 OUT --from FILE 0 --until KEY=V --stops N --minutes 15
    --workers 7 --slices 7 --batch 1 --at-once 1`, with full5's flags and environment and that leg's edges and
    toggles. high9 is high8 (full5's binary, be09fea) plus E030's `--until end`; the gate's digests are the same.
  - `--until` takes one key. In the search, the run ends when the first of its `--until` keys holds (main.rs, the
    watch loop), not when all hold, and the search's level is not a key. So the key is the stretch's gain: the bit
    or owner that stop j first held.
  - The stop that first held it is read from `rung-stops.jsonl` (its whole line) and spliced in. The rest of the
    route is re-timed after it (stage 1). If the state does not rejoin, the search runs again from that stop, with
    one key that j held and the found stop does not. At most two such runs a corridor.
  - `--stops N` bounds the run by a count, as the gate does. With `--at-once 1` the order is then the same on any
    machine. A bound in minutes alone would cut it at a point that depends on the machine's speed.
  - Kept only for an earlier verified end.
- **May change:** replace a stretch of trials with trials the search itself made.
- **May not change:** use any input the search did not make there.
- **Cost.** At most 10 search runs in all (first runs and reruns together) of 15 minutes each, plus re-times: 3 h
  or less. The search selects by progress, not time, so many corridors are expected to find nothing shorter.

### Stage 5: local tightening in narrow windows (TAS)

- **Buffering.** Each input inside a group is moved earlier, down to one frame after the input before it. It is
  judged by the click verdict and the group verdict. Pool: verb-to-object gaps, at most 1,801 frames.
- **Luck for gain.** Each draw-marked group gets a delay scan d = 0..30 with one group of look-ahead. The delay
  whose next control return comes earliest is kept, not merely the one that repeats full5's draw (room 39: 13,200
  ticks over 21 visits in full5).
- **Walk pixels and re-clicks.** Each scene click that starts a walk gets two perturbations, judged by the next
  control return's frame:
  - other pixels with the same hit (same object, or floor in the same walk box);
  - re-clicks of its own target every k frames, k = 1..8 (fraction reset). The Any% rules allow one mouse event a
    frame, and full5 parks the pointer one frame after every click, so a re-click on that frame replaces the
    parking move, which then follows the last re-click.

  Ranked by the frames walking takes, map rooms first (room 85: 531 s over 60 visits in full5).
- **May change:** frames of inner inputs; the pixel of a scene click within its own hit; re-clicks of a click's own
  target.
- **May not change:** add any other input; change what a click acts on.
- **Cost.** 2 h or less, with windows taken in order of the frames they can save.

## Glitches: in scope and out of scope

| Glitch or trick | Here | Why |
|---|---|---|
| Logo speed glitch (var 19 left at 5) | In, stage 2 | Allowed. A general boot scan judged by ticks can find it. The Esc window 46-92 is known in our docs, but it is not given to the system. The scan's form was chosen knowing the glitch exists (see the RQ3 note at the top). |
| Intro skip (`SPEEDRUN_START_FRAME=1` or patch 0013) | In, stage 2 | A bridge setting, no game knowledge. |
| Input before the search's calm and probe ends | In, stage 1 | Engine timing. The game's own control signal judges it. |
| Luck by delay (draws) | In, stages 1 and 5 | Timing only. Twins with a perturbed RNG find the draws. |
| Buffering, re-clicks during walks, the nearest pixel of an exit | In, stage 5 | Allowed in Any%. General scans in narrow windows. |
| Credit Early | Kept | full5's search found it. Stage 3 may trim its repeated sentences. |
| Whatever the unchanged search finds in a corridor | In, stage 4 | The search is general. |
| Item slide; the oars, monkey and feather skips built on it | Out | No general mechanism in the system forms a slide. The payoff comes hours later, in another part, so a local judge rejects the setup. Using the known setups would be game knowledge. A general detector (index the scripts' hard input-on writes; player-started cutscenes at varied frames; novelty on input being live inside a cutscene) is RQ3 work for a later experiment. Open decision 2. |
| Map teleport by an inventory action in place of a walk | Out | General only if item actions are tried as walk replacements. Stage 4's search may still find it by chance. |
| Head skip, necklace skip, church door window | Not targeted | General window scans could find them. Whatever stage 5 finds is kept. |
| Save and load to re-roll draws | Out | Allowed, but the bridge cannot press modifier keys, and a frame-exact bot does not need it. |
| A chosen seed | Out | Allowed. Each seed means a whole-route re-time. Open decision 3. |
| Screen snap (v4 only), the EGA map teleport, debug keys, No Music | Out | Not on DOS CD, or banned. |
| Running without CD track 6 | Out | A rules question, not a shortening. Open decision 4. |

## Gates (offline, before any stage run)

- **G0, the search unchanged.**
  - `git diff` from high9's commit is empty for `fmi-direct`, `fmi-core`, `fmi-engine`, `fmi-explore` and `patches/`.
  - `Cargo.lock` changes no version of a package the search builds (`cargo tree -p fmi-direct` the same before and
    after).
  - `gate.sh` gives PASS.
  - full5's `verify-end` gives frame 114,978 and tick 694,701.
- **G1, fake engines** (`fmi-shorten/tests`).
  - The re-timer places a group one frame after its control return.
  - The padding ladder returns the first rung that passes.
  - The delay scan finds the first d that repeats a draw.
  - A draw stored in a non-guide key that a twin changes fails the group verdict at its own group.
  - Step 3's restore, after an unrepairable group, gives the base's digests from the restored control return on.
  - A drop is kept only for an earlier verified end.
  - A failing combination is split by ddmin.
- **G2, identity.** Stage 0's identity test passes on full5, on E024's seed-1 route of the Fate of Atlantis demo, and
  on E012's Monkey Island kitchen chain. Also: on Monkey Island, patch 0013 with the skips at frame 1 gives first
  control at frame 110, tick 660, var 19 = 6, as `SPEEDRUN_START_FRAME=1` did (stage 2's boot setting).
- **G3, generality.** grep finds no room, script, variable or object number of the game in the new code. The
  engine's own variables the bridge reports by name for every SCUMM v5 game (var 19, the skip keys var 24 and
  var 57 in the `segment_start` record) are engine facts, not game knowledge; each use is listed in the report.
- **G4, cost**, measured in stage 0 and on the first 100 groups of stage 1:
  - one placed group with no repair: 20 ms or less;
  - a whole re-time pass with no repair: 2 minutes or less;
  - a whole re-time pass with its repairs, projected from the repair rate on the first 100 groups: 30 minutes or
    less (stages 2 and 3 count their end-to-end runs in this unit);
  - stage 3's projected cost: 3 h or less.

  If G4 fails, the design changes (horizon, candidate caps) before the run, as the performance rule requires.
- **G5, forks.** E025's F4 has no known cause. Every fork result used in a kept version is checked against a plain
  replay from the previous kept stop. One mismatch stops the stage.

## Prediction (before running)

Ticks at the first frame where bit 565 holds, from boot, seed 1. "Odds" is the chance the stage keeps any gain.

| Stage | Gain, centre (range) | Route after, centre | Odds |
|---|---|---|---|
| Reported end of full5 | | 694,701 (frame 114,978) | |
| 0, the end measured per frame | −36,888 (to −37,300) | B0 = 657,813 at frame 108,830 (at earliest 108,775) | certain |
| 1, re-timing | −130,000 (−70,000 to −210,000) | ≈ 527,000 | 85% |
| 2, intro skip | −15,600 (0 if the route breaks) | ≈ 512,000 | 60% |
| 2, logo glitch | −55,000 (0 to −80,000) | ≈ 457,000 | 40% |
| 3, drops | −35,000 (−10,000 to −90,000) | ≈ 422,000 | 70% |
| 4, corridors | −15,000 (0 to −60,000) | ≈ 407,000 | 35% |
| 5, local tightening | −15,000 (−5,000 to −35,000) | ≈ 392,000 | 70% |

- **Final.** The centres in the table assume every stage keeps its gain; the odds say they will not all (all six
  together: about 3.5%). Weighted by the odds, the expected end is about 476,000 ticks (2:12:13), 28% under B0. If
  every stage keeps its centre: about 392,000 (1:48:53), 40% under B0. Range 320,000 to 560,000. Either is still
  several times the human record, which is timed from another start (the logo-skip Esc) to another end (the
  final fade), so the ratio is only a scale.
- **Stage 0.** Bit 565 first holds at frame 108,830, tick 657,813: global 132 sets it and then starts global 137,
  whose room load at frame 108,830 comes with no break in between, so both fall in one frame. That is 6,148 frames
  before the reported end. qf in the standalone launch exceeds 4 × tick by less than 1,000 jiffies over the route
  (the fast launch shows 77.75).
- **Stage 1.** 25% to 75% of the 47,138-frame pool goes (70,000 to 212,000 ticks), centre about 46%. Most probe
  padding goes. About half of the waits come back through the padding ladder.
- **Stage 1, draws.** At least one group fails at a draw before the end: 70% odds. The delay scan repairs most of
  them. If the RNG does not move per frame in that room, it cannot, and step 3's restore gives back that stretch's
  gain. The first such group, if any, is most likely at room 30's first entry (frame 3,081 in full5), where the
  four-digit combination is drawn into non-guide keys (caught only through the twins' draw keys), or in room 39.
- **Stage 2.** The boot scan finds a boot class with a lower probe tick. It contains Esc presses on frames near
  46-92 and var 19 = 5: 75% odds. The whole-route re-time under it reaches bit 565: 40% odds.
- **Stage 3.** Exact-digest cuts: fewer than 10. Most of the gain comes from bounces and no-news single trials.
- **Stage 4.** Most corridors find nothing earlier. Any gain comes from the two longest stretches.

## Success threshold

- **Yes.** A route from boot, made by the shortener alone, verified as above, reaching bit 565 at least 25% under B0
  (about 493,400 ticks if B0 is 657,813). Odds about 60%.
- **Strong yes.** At least 40% under B0, or the boot scan finds and keeps a lasting timer change with no key, frame
  or variable given (RQ3, in the limited sense stated at the top).
- **No.** Less than 10% under B0 after stages 1 and 3. Then the waste is not in timing or in removable trials, and
  the next step is a re-search for fewer actions, not pruning.
- **Reported either way.** Ticks, frame and qf at bit 565 per stage and version; the share of each pool removed;
  the groups the passes could not repair, with their mechanics; unit costs.

## Stop rule

- Per stage, the wall budget given above. Over budget, the stage stops and its last verified version stands.
- Within a stage: a round that gains less than 1% of the current route ends the stage. At most 3 rounds.
- Stage 1 gains less than 5% of B0: stop the experiment and write up before building stages 2 to 5. Every later
  stage depends on the re-timer.
- A pass cannot repair a group even by restoring full5's gaps back to the last digest-equal control return (stage
  1, step 4): the stage stops there, records the group and its cause, and the fix is made offline. No hand patch.
- A version that its pass accepted fails its verification from boot: everything stops until the cause
  (non-determinism) is found.
- The gate fails, or a change to the search's crates or the engine seems needed: stop and ask the owner.

## Risks

- **Draws.** Any re-timing changes how many per-frame draws run before later random outcomes. The store's
  combination and presence draws (Credit Early), room 39's chamber layouts and the insult draws can all come out
  differently. A delay steers them only where the RNG moves per frame. Stage 0 measures where it does. Elsewhere
  stage 1's restore (step 3) gives back the gain of that stretch, and only a corridor can win it again.
- **The group verdict.** Too loose: a broken state is caught only at the 50-group check, and found by ddmin.
  Too strict: padding that is not needed comes back. The idle keys measured in stage 0 set the balance. A draw
  written to a key outside every twin's difference (a local variable read later by a script still running) is
  still invisible to it; only the end check finds it.
- **Inner offsets.** Stage 1 keeps the offsets inside a group. In full5 they are short (inner spans total 5,201
  frames; 1,043 of the 1,415 groups hold one click), but 5 inner gaps exceed 100 frames (2,051 frames). Under
  var 19 = 5 a tick-paced wait inside such a group takes more frames, and the padding ladder, which pads only
  before the group, cannot fix it.
- **Inputs during running scripts.** Inputs at control + 1 may hit a scene that is still moving (camera, actors).
  The click verdict catches a different hit. A changed sentence that is still taken is caught at the next control
  return.
- **Forks.** F4's cause is unknown. G5 checks every fork result used. The sound-result variable (var 56) differs on
  forks: skipped as in the run (FMI_SKIP_SOUNDRESULT). FMI_MEM_FILES is required.
- **Cost.** Stage 3's rounds and stage 4's corridors are the expensive parts. G4 projects them before they run.
- **The corridor search is not built for time.** It selects by progress, so it may wander as far as the route did.
  With `--at-once 1` and a `--stops` bound its result does not depend on the machine; under the 15-minute cap a
  slower machine may stop before the bound.
- **The metric.** Bit 565 is the system's end, not the board's. The board's end (the fade after the final cutscene)
  needs menu choices full5's movie never made. Times are not comparable with the board until that end is reached.
- **Real time.** If the standalone launch counts fades that the fast launch does not, qf becomes the objective
  (E025's paused part 1) before any stage judges candidates.
- **Seed 1 only.** Any% names its seed, so this is allowed, but no gain is shown on other seeds.

## Open decisions for the owner

1. **Which end.** Bit 565 (the task's metric; the search's end) or the board's end (first frame of the final fade).
   The board's end needs the final menus found by the system, for example by a stage-4 corridor with the search's
   `--until end`.
2. **The item slide family.** Out, as above, unless the owner allows building a general detector for it in this
   experiment (RQ3). Taking the known setups from the human route would break the no-hand-move rule.
3. **The seed.** Keep seed 1, or let the system scan seeds (each one a whole-route re-time, about 30 minutes).
4. **CD track 6.** Kept present (the Last Part card waits 7,800 ticks for it). Running without it is a rules choice.
5. **Where the code lives.** `fmi-shorten` and `fmi` (planned), or a separate crate. Also whether stage 4 may call
   the frozen search binary as a subprocess.
6. **Budget.** About 12 h of laptop time over six stage runs of at most 3 h each (0.5 + 1 + 2 + 3 + 3 + 2 h).
7. **E025.** Its full walk stage (waypoints, approach clicks) stays a separate experiment. Stage 5 only perturbs
   pixels and re-clicks in narrow windows.

## Method record (filled in when run)

- Commit: the one that adds the shortener's stages, after G0 to G5.
- Commands (planned names):
  - `fmi shorten table --movie full5.jsonl --guide experiments/E027-direct/guide/guide-end.json --goal bit:565=1 --seed 1 OUT/s0`
  - `fmi shorten retime --movie M --table OUT/s0 --goal bit:565=1 --seed 1 --workers 7 OUT/s1`
  - `fmi shorten boot --movie M --table OUT/s1 --workers 7 OUT/s2`
  - `fmi shorten drops --movie M --rounds 3 --workers 7 OUT/s3`
  - `fmi shorten corridor --direct /var/tmp/fast-mi/direct/bench/bin/high9 --tree /var/tmp/fast-mi/direct/full5 --movie M OUT/s4`
  - `fmi shorten local --movie M --workers 7 OUT/s5`
  - `fmi shorten verify` and `direct verify` for each kept version.
- Game: Monkey Island DOS CD (`monkey`, CD, DOS, English; hashes in `results/full5/settings.md`), its scripts' dump,
  seed 1, the laptop.
- Held out: the human routes and runner sources are never read by the shortener. They are only compared with in the
  report.
- Lab notes: `notes/log/` on each stage's day. `report.md` when the experiment stops.

## Out of scope

- E024's forward pass and drop rounds as built: they stopped on Monkey Island walking (G7) and grow with the square
  of the step count. The re-timer keeps each group's raw inputs at their offsets instead of re-placing them by
  anchors.
- Any change to the search's selection (ticks instead of progress). That would change the search's results.
- A global reorder of milestones (A* over a milestone model built from the tree). It needs a model of preconditions
  first, for a gain not yet shown.
- Text speed other than full5's. It matters mainly for the slide, which is out.
