# Where the time goes in full5's route

A research report made for E031's plan (2026-11-16) by a reading agent; its claims were used in the plan after the plan's critics checked them. Evidence paths are as the agent gave them.

## Findings

### 1. The path and the time base. The end stop's parent chain across the four legs has 1,038 stops: leg00 has 344 including the boot root, leg01 90, leg02 444 and leg03 160. Their own events, joined in order, equal end-stop.jsonl event for event. The route after the first input is 112,250 frames: from frame 2,728, where the first input is taken, to frame 114,978.

- **Evidence:** Concatenated path events == end-stop.jsonl events (3,916, True). The root is at frame 2,728, room 33. Before it, 2,727 intro frames run unskipped (tick 16,321, from the guide's trace record). Ticks after the root: 694,701 - 16,321 = 678,380, so 6.04 ticks per frame on average (about 0.10 s per frame). Only the end tick is recorded. Scripts: /var/tmp/fast-mi/e031-research/a1.py to a15.py; the tree cache is tree.pkl in the same folder.
- **Why it matters:** Every bound below is in frames along this exact path. Converting to time uses the 6.04 average, because the files hold no tick or var19 per stop.

### 2. 71% of the route is the game busy after a trial's last input. Input timing itself is nearly free. The 112,250 frames split into four parts: settle (from the last input to the next stop), the time inside multi-input trials, explicit waits, and 1-frame leads.

- **Evidence:** Settle: 79,752 frames (71.0%). Between the first and last input of multi-input trials: 22,719 (20.2%). Explicit waits: 8,775 (7.8%), that is 2,454 in 9 pure wait trials plus 6,321 before the first click in 24 wait+click trials. 1-frame leads: 1,004. The settle median is 73 frames (p90 127). The last trial alone settles 6,398 frames.
- **Why it matters:** Tighter input timing (the lead) can save about 1,000 frames at most. Real savings must come from removing search artefacts (probes, waits, calm), loops, and fewer or shorter actions.

### 3. The largest single search artefact is the probe read-again. 399 of the 1,037 path steps are '~' stops. Each is read PROBE=60 frames plus a 6-frame calm after the trial's own stop, and the next trial starts only from there.

- **Evidence:** The 399 probe steps hold 44,934 frames. Their settle is at least 73 frames (median 99) against at least 7 for ordinary trials, and sum(min(66, settle-7)) = 26,334 = 399 x 66. 240 of the 399 carry no news. By leg: leg00 154, leg01 31, leg02 172, leg03 42.
- **Why it matters:** The upper bound is 26,334 frames (23.5%). Only probes where a still-running script writes the needed value (--runs-on) actually need some of those frames. Re-timing the next trial to the trial's own stop is the cheapest first test, verified by replay.

### 4. Explicit waits come only in three coarse lengths (50, 200 and 800 frames), so most of their length is probably not the game's minimum.

- **Evidence:** 33 wait trials: 15 x wait50, 11 x wait200, 7 x wait800, 8,751 wait frames. The seven wait800s total 5,642 frames: leg00:4080 (from frame 13,046, room 35), leg00:8398 (29,354, room 85 to 49), leg00:8939 (31,282, room 28), leg00:9265 (32,712, room 35), leg00:10761 (34,638, room 28), leg02:296 (52,025, room 35 to 33), leg02:1385 (53,123, room 28). In the stored tree, leg02:284 (plain t450, no wait) reached the same K3-proxy state as path stop leg02:296 (wait800+t450) at frame 52,079 instead of 52,885, 806 frames earlier.
- **Why it matters:** Bound (3a) is 8,751 frames (7.8%). Each wait can be cut to the engine-measured minimum by a bisection of replays. Calm adds about 6 frames per ordinary trial: 629 trials x 6, about 3,800 frames (3.4%).

### 5. Time per room is spread over many revisits. 30 rooms visited 5 or more times take two thirds of the route.

- **Evidence:** 465 room visits (maximal runs) over 82 distinct rooms. Trials begun in rooms visited at least 5 times: 75,457 frames (67.2%). Frames by starting room: 59: 8,878 (6,425 of them are the final trial); 35: 8,869 (12 visits); 48: 8,066 (11); 28: 6,401 (13); 85: 6,302 (37 visits, the hub); 49: 6,133 (12); 30: 5,758 (5); 25: 4,601 (8); 16: 2,930 (18); 11: 2,884 (9). Most-used transitions: 85->49 x12; 6->132 and 132->6 x10 each; 15->16, 16->11, 11->16, 16->15 and 15->4 x9 each. A 'go' walk through several rooms is one trial, so its middle rooms are not counted (52 walks, 251 clicks, 12,520 frames).
- **Why it matters:** Revisits are where loops and repeated cycles live. They show where a shortener should look first.

### 6. Repeated room cycles with no recorded gain.

- **Evidence:** 109 A->B->A bounces take 43,311 frames. 45 of them have no news and the same object set at both ends: 14,701 frames. The most common are 6<->132 (x9), 16<->11 (x7), 85<->48 (x4) and 2<->20 (x4). The chain 4->15->16->11->16->15->4 is walked 5 times (4,638 frames), four of them with no news: leg02:17477 from frame 81,452 (807 frames), leg02:18892 from 84,801 (1,161), leg03:333 from 93,763 (834), leg03:1677 from 95,367 (928). In room 48 the same 5-6-trial click sequence returns to an equal K3 state 5 times: leg00:7883 from 27,118 (517 frames), leg00:8415 from 30,195 (778), leg01:8010 from 49,442 (833), leg02:2695 from 54,659 (796), leg02:9728 from 66,381 (609), together 3,533 frames.
- **Why it matters:** These are the concrete candidates to cut first. Whether each one changed state the proxies cannot see (vars, positions, RNG) needs the engine.

### 7. Time before each object gain. A few long stretches dominate, and three of the five longest cross a leg boundary.

- **Evidence:** There are 57 stops where the object set grew (73 objects at the end). The median gap is 500 frames, and the top 5 gaps total 48,226 frames (43%): (1) 15,416 frames, leg00:9232 (frame 32,712) to leg01:4808 (48,128): 122 stops, 55 room visits, crosses leg00/01; (2) 9,715, leg03:7443 (105,263) to the end, including the 6,425-frame final trial; (3) 8,629, leg00:7460 (24,083) to leg00:9232: 70 stops, 24 visits; (4) 8,087, leg01:4808 to leg02:5540 (56,215): 84 stops, 38 visits, only 7 news keys, crosses leg01/02; (5) 6,379, leg02:18507 (83,785) to leg02:21135 (90,164): 78 stops, 48 visits. The 4,905-frame gap leg02:21135 to leg03:1650 (95,069) crosses leg02/03. Leg01 as a whole: 9,230 frames for 1 object and 41 news keys. The longest stretch with no news at all is 4,760 frames, leg01:10714 (51,455) to leg02:5540 (56,215).
- **Why it matters:** Each leg restarts from the previous leg's top stop (the newest line-beginning stop at the top level) with fresh memory. That stop can lie off the useful route, so the next leg walks back.

### 8. Leg-boundary detours under the K1 proxy (room, object set, all news keys won since boot).

- **Evidence:** Leg01 returns to room 49 in an equal K1 state at leg01:2154 (frame 44,739), 2,235 frames after its root (rooms 49->35->48->85->49). Leg03's path stops at frames 93,357 to 94,935 (rooms 2, 21, 3, 15, 16, 11, 5, 12) have K1 states that off-path leg02 stops reached at frames 90,183 to 90,508. For example, leg02:21273 reached room 12 at frame 90,416, while leg03:1620 reached the same K1 state at 94,935: 4,519 frames later. K3 does not confirm these: the labelled atom values differ.
- **Why it matters:** This is the largest tree shortcut, but only under the loose proxy. It is a direct test case for the engine's digest.

### 9. Bound (1): cutting loops whose two ends are the same state, measured on the path only. The bound depends strongly on how strict the state proxy is.

- **Evidence:** From strictest to loosest (frames cut): (a) same room and objects, with every stop inside kept only for walking or sight (sees, box, cam, room, arrived, iface) and no news: 57 loops, 4,274 (3.8%); (b) K3, with no truncated label list inside: 110 loops, 10,777 (9.6%); (c) K3 = K1 plus the latest labelled value of every var, bit, state, class or owner atom in 'novel' along the ancestry: 136 loops, 24,774 (22.1%); (d) K1: 166 loops, 53,210 greedy (47.4%), 53,662 by the shortest path over path stops merged by K1; (e) room and objects only: 83,953 (74.8%), not credible. Largest K1 loop: leg02:18674 (frame 83,995) to leg02:19900 (86,855), 2,860 frames, 37 stops. Largest K3 loop: leg00:8911 (31,282) to leg00:8942 (32,146), 864 frames: a wait800 trial, then t323, back in room 28.
- **Why it matters:** The defensible range is about 4k to 25k frames (4-22%). The 53k figure ignores every var, dialogue, position and RNG change. Only per-stop state from the engine can choose within this range.

### 10. Bound (2): shortcuts already in the stored tree. All four legs' kept stops (57,279 nodes) are merged by proxy, and the shortest path runs from the boot root to the end state.

- **Evidence:** K3: 364 path stops were reached earlier by another tree stop in an equal state (163 of those sources are off the path). The merged shortest path gives 27,317 frames saved against 24,774 from path loops alone, so off-path stops add 2,543. K1: 600 such path stops (300 sources off the path); 59,261 saved against 53,662 path-only, so off-path stops add 5,599. Room and objects only: adds 7,577. Largest K3 off-path jumps: leg00:8420 (frame 30,349) to path stop leg00:8905 (31,240), 891 frames, room 33; leg00:5841 (21,338) to leg00:6370 (22,162), 824 frames, room 30; leg02:284 (52,079) to leg02:296 (52,885), 806 frames, room 33.
- **Why it matters:** Off-path stops add little beyond cutting loops on the path (2.5k to 5.6k frames). Most of the stored tree's value is already in the path's own revisits. Each splice still needs re-timing, because event frames are absolute, and a replay from boot.

### 11. Combined optimistic bound, and the human scale.

- **Evidence:** The K1 merged route is 52,989 frames (478 steps). Removing its 154 probes (at most 10,164 frames) and its 14 waits (1,534) leaves 41,291 frames, about 69 min of game time, including the 6,398 frames after the last input. The actual route after the first input is 112,250 frames (about 3 h 08 min). The human Any% record is 21:52.
- **Why it matters:** Cuts plus wait removal give at most about 2.7x, even with the loosest credible proxy. Reaching human scale needs a re-search for fewer or different actions between needed gains, not just pruning the found route.

### 12. Clicks against pointer moves, and the gaps between clicks.

- **Evidence:** 3,916 events: 1 skips setting, 1,994 clicks and 1,921 pointer moves (49% of events). Every move comes exactly 1 frame after a click and parks the pointer at (0,199) x1,370 or (319,199) x551, so moves cost no frames of their own. 1,010 clicks are in the scene and 984 in the verb, inventory or dialogue area (y>=144). Gaps between consecutive clicks: median 33 frames, mean 53. There are 336 gaps of 2-3 frames (a verb, then an object), 367 of 4-10, 264 of 11-30, 312 of 31-60 (13,729 frames), 548 of 61-120 (48,642 frames), 154 of 121-300 (24,498) and 12 over 300 (8,505). The largest is 1,330 frames, from the click at frame 105,705 to stop leg03:7560 (room 70->83, news bits 441-453, a scripted sequence). Trial kinds on the path: 929 click trials (79,786 frames), 52 walks (12,520), 23 window trials (9,402), 24 wait+click (8,088), 9 pure waits (2,454). 27 count-repeat trials ('.') take 146 clicks and 8,952 frames.
- **Why it matters:** Pointer moves are free to drop or keep. Click count is not the cost: the cost is in the 61-300-frame gaps, which are game walks, animations and scripts after each sentence.

### 13. Why each path step was kept (from 'news' and 'novel').

- **Evidence:** Steps ending in a stop with news (it won an object, flag or count): 266 steps, 38,810 frames (34.6%). World atoms changed but no news (toggles, object states and classes): 462 steps, 45,312 (40.4%). Flag-line stop with no news: 110 steps, 13,334 (11.9%). Navigation only: 153 steps, 9,791 (8.7%). Var or local values only: 46 steps, 5,003 (4.5%). 627 of the 1,038 path stops have truncated 'novel' lists (+N).
- **Why it matters:** Two thirds of the frames go to steps that won nothing the files record as a gain. Many are needed walks, but this is where removal of unneeded actions (beyond loops) would come from. That needs a dependency test the files do not hold.

### 14. The run's last 6,399 frames have no input.

- **Evidence:** The final trial, leg03:7895 'v4>v7t733t734!' from parent 7864 (frame 108,553, room 59->89), has its last click at frame 108,579. It reached no stop ('!'): the run ran on until bit 565 at frame 114,978. That is 5.7% of the route, about 10.7 min of game time, with the text and cutscene skips on.
- **Why it matters:** No route change touches this part. The board stops its clock at the fade to black after the final cutscene, and where that falls inside these frames is not in the files. It changes the comparable time, not the route.

### 15. What the files do not hold, which a shortener must get from the engine.

- **Evidence:** (a) No per-stop facts, world digest or cell hash: nodes.jsonl holds no line or cell, and the guide's chapter 05 says facts (about 25,000 pairs, about 400 kB per stop) are never stored. The bridge's stateDigest (wire 0x98) covers the full save state plus the RNG, mouse registers and skip switches, and leaves out frame and tick counters. That makes it the exact equality test, but a strict one: positions and RNG must match. The search's cell (room, face, world atoms without ambient keys) is the coarser test. (b) The trial's own stop frame before each probe. (c) Ticks and var19 per stop. (d) Actor position, walk box and camera at each stop (sees, box and cam appear only when new). (e) The minimum length of each wait. (f) Which wins are needed for the end: the files record what was won, not what was used. (g) Toggles are left out of news. (h) The rooms crossed inside 'go' walks. Replaying the whole movie from boot took 2.89 s (verify-end.json), so recording the digest, the cell and positions at all 1,038 stops is one cheap pass.
- **Why it matters:** Every bound above is a proxy. With per-stop digests from a single replay, bounds (1) and (2) become exact counts, and each candidate cut can be verified from boot as the rules require.

## Numbers

- Path: 1,038 stops (leg00 344 incl. root, leg01 90, leg02 444, leg03 160); 1,037 steps; events equal end-stop.jsonl (3,916)
- Route after the first input: 112,250 frames (frame 2,728 to 114,978), about 678,380 ticks, about 3 h 08 min; intro before the first input: 2,727 frames (tick 16,321)
- Average 6.04 ticks per frame (only the end tick is recorded)
- Per leg: leg00 39,776 frames, 710 clicks; leg01 9,230 frames, 163 clicks; leg02 41,126 frames, 811 clicks; leg03 22,118 frames, 310 clicks
- Split: settle 79,752 (71.0%); inside multi-input trials 22,719 (20.2%); explicit waits 8,775 (7.8%); 1-frame leads 1,004
- Probe '~' stops: 399 steps holding 44,934 frames; probe overhead at most 26,334 frames (23.5%)
- Explicit waits: 33 trials (15 x 50, 11 x 200, 7 x 800) = 8,751 frames; the seven wait800s = 5,642
- Calm overhead: about 6 frames x 629 ordinary trials, about 3,800 frames
- Clicks 1,994; pointer moves 1,921 (49% of events, each 1 frame after a click, no frame cost); scene clicks 1,010, verb/inventory clicks 984
- Click gaps: median 33 frames, mean 53; 548 gaps of 61-120 frames hold 48,642 frames
- Room visits 465 over 82 rooms; room 85 visited 37 times; rooms with 5+ visits take 75,457 frames (67.2%)
- A->B->A bounces: 109 (43,311 frames); no gain and same objects: 45 (14,701 frames)
- Cycle 4-15-16-11-16-15-4 walked 5 times = 4,638 frames; room 48 repeated sequence 5 times = 3,533 frames
- Object gains: 57 stops; median gap 500 frames; top 5 gaps 48,226 frames (43%); largest 15,416 (leg00:9232 to leg01:4808)
- Loop bound (1): navigation-only 4,274 (3.8%); K3 with no truncated lists 10,777 (9.6%); K3 24,774 (22.1%); K1 53,210-53,662 (47-48%); room and objects only 83,953 (74.8%, not credible)
- Tree bound (2): K3 merged 27,317 (+2,543 from off-path stops); K1 merged 59,261 (+5,599)
- Combined optimistic (K1 route without probes and waits): 41,291 frames, about 69 min of game time; human Any% record 21:52
- Final trial: 6,399 frames after the last input (frames 108,579 to 114,978), 5.7%
- Whole-route replay from boot: 2.89 s (verify-end.json)

## Open points

- Which proxy loops are real? Replay the path once and record stateDigest and the search's cell at all 1,038 stops; then count exact equal-state pairs, both full digest and cell-equal.
- How many of the 399 probe stops need their 60 frames, because a running script writes the needed value? The trial's own stop frame is not stored.
- What is the minimum length of each of the 33 explicit waits, especially the seven wait800s in rooms 28, 35 and 85? Note leg02:284 reached the K3 state of leg02:296 without the wait.
- Are the leg-boundary detours real (leg01's return to room 49; leg03 re-walking rooms that leg02 off-path stops reached 3,000-4,500 frames earlier)? K1 says yes, K3 says no.
- Does cutting a loop keep later behaviour? Cutting shifts every later input earlier: NPC timers, positions and RNG draws (the digest includes the RNG) may differ, so each cut needs re-timing relative to stops and a replay from boot.
- Where in the last 6,399 input-free frames does the board's end (the fade to black) fall, relative to bit 565?
- Which recorded wins (533 news keys, 73 objects) does the end actually depend on? Without a dependency test, the 40% of frames in no-news world-change steps cannot be bounded.
- Even the loosest credible bound leaves about 69 min against 21:52 for the human record. The larger gain needs a re-search of the stretches between needed gains, not only pruning.
