# The metric and the reference route (evaluation only)

A research report made for E031's plan (2026-11-16) by a reading agent; its claims were used in the plan after the plan's critics checked them. Evidence paths are as the agent gave them.

## Findings

### 1. How 21:01.483 was timed. It is a count of quarter-frames, not frames and not ticks/60: RTA = (qf_end - qf_start) / 239.992156448. The bridge adds every engine wait to qf (each frame's 4 x var 19, plus fades and transitions). Start: the logo-skip Esc at boot frame 46 (qf 1,100, tick 275). End: the first frame of the final fade, frame 14,820 (qf 303,846). 302,746 qf = 1261.483 s.

- **Evidence:** /home/ilyask/projects/scummrun/scripts/anypercent_verify.py (replay(): release dos-cd, timer_next any, start_frame 46, seed 836557856, max_ticks 1e7; rta = (qf - qf0)/timer_hz). /home/ilyask/projects/scummrun/runs/anypercent/m2n-verify/verify.json (frame 14820, qf 303846, qf0 1100, talkspeed "85"). Trace boot record: timer_hz 239.992156448, bridge v3, boot_param 0, music_type 8 (AdLib). patches/0001 onWait(): _quarterFrames counts every waitForTimer call. Main-loop ticks over the same span are 75,631 (1260.5 s at 60/s); the other 222 qf (0.93 s) are waits outside the main loop.
- **Why it matters:** The draft plan's metric (ticks from boot to bit 565) uses another clock, another start and another end. A number on that metric cannot be set beside 21:01.483.

### 2. The reference run's settings. Seed 836557856 (solved backwards so the first store entry draws the easiest safe combination with the keeper away). talkspeed 85 (var 37 = 6), needed so readings last long enough for the item slide. No boot parameter. Game folder without track6.flac. Engine: ScummVM v2026.3.0 with patch 0001 only (bridge v3, October). The movie has 2,027 events: 1,561 clicks, 8 right clicks, 113 verb keys, 230 Esc, 101 '.', 14 skip-switch events. The bridge pressed 374 more skips itself (338 text, 36 cutscene). The verifier turned those into keys (2,387 events) and replayed with automatic skips off: same frame 14,820.

- **Evidence:** runs/anypercent/m2n/scummvm.ini (talkspeed=85, path .../fast-mi-sandboxes/nocd/game/dos-cd, 23 flac files against 24); m2n-verify/verify.json (auto_skips and explicit both frame 14820, qf 303846); event counts from m2n/movie.jsonl and m2n-verify/movie-explicit.jsonl; git ls-tree a7e2a8c patches/ shows 0001 only (commit 6617720, the README's rebuild pointer, has 0001-0005). No frame in either movie holds more than one key or one mouse event.
- **Why it matters:** rules/anypercent.md requires a reported time to replay from raw input alone. The optimizer's result needs the same explicit-key replay, and its seed, talk speed and game files must be recorded with the time.

### 3. The end event. The verified runs used the goal 'room 0 and var 293 == 161', not the script's default 'room 0 and var 194 == 122'. Both give frame 14,820. The first holds whichever final reply is chosen; the second holds only for the reply the hand route picks. The fade is the credits script's room load, 60 ticks after the last line is skipped. The credits then loop forever, so Monkey Island has no restart or quit for the search's '--until end' rule to catch.

- **Evidence:** verify.json 'goal'; data/scripts-dos-cd/global/script-138.txt [044C] (var 293 = 161 at the last menu's setup), [06FC]-[0706]; script-130.txt [0000]-[000E] and its last line 'goto 09F6'; routes/anypercent/S7-part4.json last move's note.
- **Why it matters:** The optimizer needs an end event stated without game numbers, and the evaluation needs one that does not depend on which final reply the optimizer picks. The var-293 goal is the right one for evaluation.

### 4. full5's route does not reach the Any% end. Bit 565 is set 40.44 s before the final fade in the hand route (frame 14,336, 1221.044 s, against 14,820, 1261.483 s). After bit 565 come an unskippable cutscene and two menus. full5's movie has no input after frame 108,580, and its replay sits in the last cutscene until the tick limit.

- **Evidence:** Measured today: m2n movie replayed to goal bit 565 == 1 gives frame 14336, qf 294141. script-132.txt [0142] sets the bit before scripts 137 and 138 run. /var/tmp/fast-mi/direct/video-full5/verify-out/eng/b0/trace.jsonl ends with 'stall' records, reason cutscene, room 89, from frame 109,178 on.
- **Why it matters:** To be compared with 21:01.483 the optimizer's movie must also answer the two final menus and reach the fade. Otherwise both routes must be compared at bit 565, where the hand route is 20:21.044.

### 5. Today's engine replays the reference to the same frame. build/svm-direct (patches 0001-0013) with the October Python controller and the game folder without track 6 gives frame 14,820, qf 303,846, 1261.483 s, card 0.167 s. The trace is byte for byte the October one.

- **Evidence:** Run today, 46 s: FAST_MI_SCUMMVM_BIN=build/svm-direct/build/scummvm uv run python experiments/E031-optimizer/research/metric/replay_ref.py (a wrapper that only points the verifier at the no-track-6 folder). sha256 of both trace.jsonl files starts 40f2d7280de48d69.
- **Why it matters:** The reference stays measurable on the engine the optimizer will use. No old build is needed.

### 6. One tool can time both routes today. The October controller on today's engine replayed full5's movie with none of the newer switches set and reached bit 565 at the same frame 108,830, tick 657,813 (qf 2,631,539). Every room, CD-track, skip and input record equals the direct binary's own replay, except one record on the stop frame. So SPEEDRUN_IDLE=handler, CUTS, IMUSE_TICKS, TEMP_SAVES, MEM_FILES, VERB_IMAGES, INPUT, LEAN and the binary channel do not change Monkey Island's timeline.

- **Evidence:** Run today, 4 min 51 s: experiments/E031-optimizer/research/metric/time_movie.py (seed 1, talkspeed 255, no start frame, goal bit 565 == 1). 6,901 of 6,902 timeline records equal; only 'control' records differ (observation).
- **Why it matters:** The comparison does not depend on which host replays the movie. But scripts/anypercent_verify.py hard-codes the seed, start frame 46 and the game folder, and the Rust host refuses seed 836557856 (host.rs:383) and fixes talkspeed at 255. The shared tool needs seed, talk speed, game folder, start event and goal as parameters.

### 7. The settings that differ between the two runs. Hand route: seed 836557856, talkspeed 85, segment start forced to frame 46, three Esc at frames 46, 49, 51 (control on the dock 0.42 s after the logo skip), var 19 = 5 for the whole run, skip switches changed 14 times by movie events (off for the slide, and a 'spare' list for the book), verb keys and right clicks used, track 6 absent. full5: seed 1, talkspeed 255, no key before first control (frame 2,728, tick 16,321, 272.0 s of intro), var 19 = 6, both skips switched on once at frame 1 and never changed (2,045 automatic presses), only clicks (1,994) and pointer moves (1,921), all CD tracks present.

- **Evidence:** experiments/E027-direct/results/full5/settings.md; fmi-engine/src/host.rs:412-461 (the engine's command and environment); src/speedrun/engine.py build_env; boot and segment_start records of both traces (var19 5 against 6, var37 6 against 0).
- **Why it matters:** These are the levers full5 never touched: the logo and intro keys, keys in general, skip switching, text speed, the seed. None needs a change to the search binary; all are inputs or settings of a replay.

### 8. An explicit Esc in the movie reaches the game before first control under the search's settings. With seed 1, talkspeed 255 and the frame-1 skip event, a single added event (Esc at frame 46) left var 19 at 5. The logo room ended at frame 46 instead of 961. The dock was reached at frame 2,039, tick 10,240, instead of frame 2,638, tick 15,781, because the rest of the intro still played.

- **Evidence:** Run today: scratchpad/metric/logo-esc.jsonl (2 events) through time_movie.py; trace room records show var19 5 from frame 46. The hand route uses two more Esc (frames 49 and 51) and has control at frame 51.
- **Why it matters:** The logo glitch and the intro skip can be expressed as three raw key events in the optimizer's movie. No bridge rule or setting has to change.

### 9. full5 on the reference's clock is about 9 times the hand route. To bit 565: full5 3:02:45.1 from boot (2:58:13.1 from first control), hand route 20:21.0. By part, with the same room entries as boundaries: Part I 6,642.9 s against 799.1 s (8.3x). Part II 509.0 against 64.9 (7.8x). Part III 3,231.0 against 281.4 (11.5x). Part IV to bit 565 310.2 against 75.2 (4.1x, or 2.4x without the 130 s card). full5 makes 624 room changes, the hand route 228.

- **Evidence:** qf at first entry of rooms 7, 20 and 95 and at bit 565 in both replays made today (full5: 6,914.9, 7,423.9, 10,654.9, 10,965.1 s from boot; m2n: 799.55, 864.47, 1145.88, 1221.04 s from the logo skip).
- **Why it matters:** This is the measured headroom. full5 also carries 272 s of intro and two 130 s track-6 cards (one in the intro, one before Part IV) that the hand route does not pay.

### 10. The reference's time by stretch (seconds from the logo skip). Part I trials 0-543.05: bar and kitchen 50.1, circus 55.9, petal and return 14.4, map, chicken and first jail visit 50.2, store with the early safe 30.5, mansion, file and underwater idol 99.1, troll and training 51.8, fight 1 40.9, forest and dig 34.0, fights 2-3 41.7, sword master 74.0. Part I crew 543.05-797.96: ship purchase 58.9, sword master joins 34.8, Hook Isle 66.6, mugs and streets 49.9, jail, store slide and carry 44.7. Part II 66.4. Part III 281.4: fort, dam and oars skip 38.1; monkey skip, idol and the hermit 28.6; village, key and rows 85.9; monkey head and catacombs 39.0; head and necklace 15.1; ghost ship 55.3; back to the card 19.4. Part IV 115.7: dock, streets, church 35.7; fight to the last item use 20.2; ending 59.9 (40.4 of it after bit 565).

- **Evidence:** /home/ilyask/projects/scummrun/runs/anypercent/m2n/results.jsonl (407 goals, each with its done frame and rta).
- **Why it matters:** It gives a per-stretch target to hold the optimizer's route against. It also shows where no input helps: the ending's 59.9 s and the cook's wait are fixed by scripts.

### 11. Logo speed glitch. Timing only. An Esc on boot frames 46-92 ends the logo while a room script has the engine's frame length (var 19) at 5; the room change kills that script before it writes 6 back. Every frame then lasts 5 ticks. Walk steps and animation run 6/5 as fast; delays and talk timers count ticks and keep their length.

- **Evidence:** docs/anypercent/findings.md (window 46-92 on DOS CD); docs/research/anypercent-engine-mechanics.md section 3; docs/comparison.md section 1.4 (a forest stretch of 35.4 s becomes 29.5 s at 5/6, against 29.3 s for the human). No pair of runs of one route with and without it exists in the project.
- **Why it matters:** Its saving is bounded by one tick per frame: at most 246 s on the hand route (14,774 frames) and at most 1,768 s on full5 (106,102 frames after first control), if the route is re-timed. That alone can exceed 10% of full5.

### 12. Early safe opening (Credit Early). Changes which states are reachable. On the first store visit the combination is drawn and the keeper is away with chance 1/4; with him away, pull and push sentences on the handle feed a counter that opens the safe and gives the credit note. No engine fault is involved, only ordinary sentences and a favourable draw. It happens at 176.5-185.5 s (frames 2,175-2,283).

- **Evidence:** routes/anypercent/S1-trials.json glitches; findings.md (seed 836557856 with the door opened 3 frames late gives 1-1-1-1, keeper away); runner sources section 3.2: about 58 s saved, from one runner's two EGA runs. Not measured in this project. The draft plan reports full5 already opened the safe by itself (bit 320 at frame 20,735 after 11 handle sentences).
- **Why it matters:** The search already has it on seed 1 with a longer combination. A seed or an entry frame with a shorter one is a timing gain the optimizer can look for.

### 13. Item slide: how it is set up, kept, lost and regained. Changes which states are reachable. The engine's input gate is a counter: hard on and off set it to 1 or 0, cutscene start and end subtract and add 1, and input is taken while it is above 0. Reading an inventory item opens a cutscene (1 to 0). In the store a room script that walks the player back from the counter does a hard off and on (to 1) while the reading is open; the reading's end then makes it 2. From then on input is taken inside cutscenes: floor clicks walk, inventory clicks and verb keys work, room hotspots are dead. Any dialogue menu sets the gate back to 1. It is carried into Part II by keeping the prisoner-release cutscene open across the end scene's hard on. It is lost to the hermit's dialogue after the idol (923-924 s). It is regained at the village by keeping the book's reading open across a dialogue (959.7-976.3 s), and kept across the head dialogue the same way.

- **Evidence:** routes/anypercent/S3-crew-slide.json, S4, S5, S6 'glitches' (script_v5.cpp:858-887, verbs.cpp:590; store local 214; globals 18, 19, 32, 70). Formed at frames 8,917-8,968 (764.8-769.05 s) with the bridge's skips switched off at frame 8,863. Setup cost in m2n: store detour 13.6 s, the extra jail talk about 7 s, a third mug 2.4 s.
- **Why it matters:** It needs four things full5's search does not do: key presses, automatic skips switched off (the automatic Esc ends a reading on its first frame), slow text, and input sent while a cutscene runs. Every large skip in Part III depends on it.

### 14. The skips built on the slide, measured against the project's no-slide run (28:01.4). Oars skip (894.05-902.50 s): during the dam blast the player is off-room, a map click teleports him and an interrupted exit walk lands him in a sea walk box, which gives the boat. Monkey skip (912.75-916.75 s): a reading freezes the gate's script so the gate stays open, then an exit click at the player's own position changes room. Together: Part III start to the monkey-head handoff takes 152.6 s against 349.3 s (-196.8 s). Feather skip (1093.4-1096.0 s): a click inside the rat cutscene issues the frozen pick-up sentence; key to root 35.4 s against 49.25 s (-13.8 s). Part II slide tricks: 66.4 s against 75.75 s (-9.3 s). All change reachable states.

- **Evidence:** runs/anypercent/noslide-p7/results.jsonl against runs/anypercent/m2n/results.jsonl; routes/anypercent/S5 and S6 'glitches'. The no-slide run was at talkspeed 255 and less tuned, so the 196.8 s includes some tuning.
- **Why it matters:** Net of its setup the slide chain is worth about 200 s. A hand-quality route without it would be about 24:20 with track 6 absent (1261.5 + 200), slower than the 21:52 record and the 21:01.483 target. The target needs the slide family or about 200 s found elsewhere.

### 15. Timing-only tricks and luck. Head variant and necklace skip (1055.9-1081.2 s): two scripts return at once, and a 20-frame animation is skipped, when the player stands on a walk-box boundary; 25.3 s against 31.8 s (-6.5 s). Catacombs (1049.0-1055.6 s): each chamber is drawn on entry from the one random stream, which moves only on script draws, so delaying the previous exit 0-12 frames picks the chamber; six exits in 6.6 s against 46.5 s without it (the first look-ahead run gained 48.9 s). Fights: a menu wait draws once a frame, so the choice frame decides the next line; look-ahead on fights and the sword master gained 4.5 s. Forest map teleport, map instant exits, re-clicked walks (7 + 2 frames measured in Part IV) and inputs 1-2 frames early are smaller.

- **Evidence:** docs/anypercent/overnight.md table (1524.408 to 1475.490 for the catacombs, 1453.072 to 1448.572 for the fights); findings.md 'Catacombs'; S6 and S7 'glitches'; results.jsonl of both runs.
- **Why it matters:** These need no game knowledge beyond 'a delay changes a later draw' and walk arithmetic. They are the part of the reference's gain that a general optimizer can reach by search over frames and click points.

### 16. The track 6 card. Timing only, and a files condition, not an input. The last part's title card waits, inside a cutscene with no override, for CD track 6 to end: 130.004 s with the file present, 0.167 s with it missing (the trace then says playing: false). It is the whole difference between 23:11.320 and 21:01.483; the route needs re-timing after the card because later inputs are frame-timed. The same track also plays on the first part's card, which full5 sits through because it does not skip the intro.

- **Evidence:** m2-verify and m2n-verify verify.json (card_s 130.004 and 0.167; rta_without_card 1261.316 in both); today's m2n trace: cd_track 6, playing false, room 95, frame 13,435; full5 trace: track 6 at frames 1,339 and 105,729, both playing true.
- **Why it matters:** The optimizer cannot find this by play. It must be an evaluation condition: report the result with the full files (against 23:11.320) and with track 6 absent (against 21:01.483). findings.md calls a run with playing: false invalid; overnight.md calls it an open rules question.

### 17. How the reference's look-ahead works (anypercent_auto.py). Before a goal whose name matches a pattern (by default the catacomb exits) or that follows a slow goal, it forks the engine, plays the goal's first move with 0..N frames of extra delay, plays the next goal too, and keeps the delay with the lowest qf at the next goal. A candidate must leave the slide counters as the undelayed branch does. With --early it sends a click 1-2 frames sooner when the game ends in exactly the same state. The driver's times are provisional: only the from-boot replay counts.

- **Evidence:** scripts/anypercent_auto.py delay_lookahead() lines 70-100, --lookahead-match default, --early; docs/anypercent/overnight.md 'Tools' ('lookahead before every slow goal' was 24 s slower: it reshuffled the catacombs).
- **Why it matters:** The method is general (fork, delay, compare at the next milestone), but where to apply it was chosen by goal name. The optimizer has to find the draw points itself.

## Numbers

- 21:01.483 = 1261.483 s = 302,746 qf / 239.992156448; start frame 46 (qf 1,100, tick 275), end frame 14,820 (qf 303,846); 14,774 frames; 75,631 main-loop ticks plus 222 qf (0.93 s) of other waits
- Same route with track 6 present: 1391.320 s (23:11.320), frame 16,378, card 130.004 s; without the card both are 1261.316 s
- No-slide hand run: 1681.413 s (28:01.4), 25:51.5 without the card; first slide run 1524.408 s; human record 21:52.4
- Hand route at the search's end flag (bit 565): frame 14,336, qf 294,141, 1221.044 s (20:21.044); bit 565 to the final fade: 40.44 s, 484 frames
- Hand route by part: Part I 797.96 s (trials 543.05, crew 254.91), Part II 66.42 s, Part III 281.42 s, Part IV 115.68 s
- Hand movie: 2,027 events (1,561 clicks, 8 right clicks, 113 verb keys, 230 Esc, 101 '.', 14 skip switches); 374 automatic skips (338 text, 36 cutscene); explicit movie 2,387 events; 228 room changes
- full5 on the same clock: bit 565 at frame 108,830, tick 657,813, qf 2,631,539 = 10,965.10 s from boot (3:02:45.1); 10,693.08 s from first control (2:58:13.1); 10,869.68 s from the logo room's own end (frame 961, qf 22,900)
- full5 intro: first control at frame 2,728, tick 16,321, qf 65,284 = 272.03 s; the hand route has control 0.42 s after the logo skip (frame 51)
- full5 by part (s): Part I 6,642.9, Part II 509.0, Part III 3,231.0, Part IV to bit 565 310.2 (card 130.0); hand: 799.1, 64.9, 281.4, 75.2; ratios 8.3, 7.8, 11.5, 4.1 (2.4 without the card)
- full5 to hand route ratio at bit 565: 8.76 from first control, 8.98 from the frame-46 point
- full5 movie: 3,916 events = 1,994 clicks + 1,921 pointer moves + 1 skip switch; 2,045 automatic skips (1,834 text, 211 cutscene); 624 room changes; 90 rooms visited against 83
- Logo glitch: window boot frames 46-92 (47 frames); bound on saving: 246 s on the hand route, 1,768 s (106,102 ticks, 16% of the total) on full5
- Explicit Esc at frame 46 under the search's settings (seed 1, talkspeed 255): var 19 = 5; dock reached at frame 2,039, tick 10,240, against frame 2,638, tick 15,781
- Track 6: 129.84 s (1391.320 to 1261.483); full5 pays it twice, about 260 s
- Slide setup cost about 23 s (store detour 13.6, extra jail talk about 7, third mug 2.4); slide-dependent gains: Part II 9.3 s, Part III before the monkey head 196.8 s, feather skip 13.8 s; net about 200 s
- Hand-quality estimate without the slide, track 6 absent: about 1,461 s (24:21)
- Catacombs: six exits 6.6 s with delay look-ahead against 46.5 s without; first look-ahead run -48.9 s; fights and sword master look-ahead -4.5 s; head and necklace -6.5 s (40 frames by script count for the necklace)
- Early safe opening: about 58 s by one runner's two EGA runs (not measured here); in the hand route the four handle sentences take 2.3 s
- Replay checks today on build/svm-direct: m2n 46 s wall, trace sha256 40f2d7280de48d69... equal to October's; full5 4 min 51 s wall, 6,901 of 6,902 timeline records equal to the direct binary's replay
- Engine at the reference run: ScummVM v2026.3.0 + patch 0001 only (commit a7e2a8c); commit 6617720 has 0001-0005; today's build has 0001-0013
- Clock: DOS CD 239.992156448 qf/s (59.998 ticks/s); full5's qf exceeds 4 x tick by 287 qf (1.2 s) at bit 565

## Open points

- The optimizer's end event. Monkey Island never restarts or quits (the credits loop), so '--until end' cannot end a run, and bit 565 is 40.4 s short of the Any% end. A general rule is needed, for example the first room change after the last input the game ever accepts. Evaluation can use 'room 0 and var 293 == 161'.
- The start event when the logo is not skipped. Proposed rule for the tool: qf at the first key delivered in the logo's room; if none, qf when that room is left (frame 961 in full5). The tool should read the start from the trace's input record, not from qf0, because SPEEDRUN_START_FRAME also changes when automatic skips begin.
- Explicit-key replay for the optimizer's route. full5's skips are all automatic. The Python verifier's conversion (trace skip records to key events, replay with skips off) has not been run on a route of the search; one check on full5's movie would settle it (about 5 min).
- Whether track 6 absent counts. The owner's target (21:01.483) is the no-track-6 number; the project's own findings.md calls playing: false invalid. Recommended: report both conditions, each against its matching reference.
- Seed. The reference seed is refused by the Rust host and must stay out of the optimizer. Whether the optimizer may search over seeds (the Any% rules allow a chosen seed) or stays on seed 1 is the owner's decision; it decides how much safe-combination and catacomb luck is available.
- Talk speed. 85 was chosen for the slide; the search runs at 255. Whether the optimizer may choose the talk speed, or change it during the run, is undecided.
- The logo glitch's real saving has no measured pair. A re-timed full5 with var 19 = 5 would give the first one.
- Can the slide be found without game knowledge? It needs keys, skips switched off, slow text and input inside a cutscene. Without it the reference suggests about 24:20 at hand quality, which misses the target by about 200 s.
- The no-slide comparison is confounded: that run was at talkspeed 255 and was never tuned like the slide run, so the 196.8 s island figure overstates the skips themselves.
- The shared timing tool does not exist as project code. The two scratch scripts used here are experiments/E031-optimizer/research/metric/replay_ref.py and experiments/E031-optimizer/research/metric/time_movie.py; scripts/anypercent_verify.py needs seed, start event, game folder and goal as parameters to serve both routes.
