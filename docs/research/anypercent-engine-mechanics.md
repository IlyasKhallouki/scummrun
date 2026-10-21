# Any% engine mechanics: clock, walking, timer var, audio, RNG, input

This note covers what ScummVM `v2026.3.0` does under speedrun.com Any% rules: all glitches allowed, pixel clicks allowed, save/load allowed, chosen seed allowed, timer-rate cheats banned. The target is MI1 DOS VGA floppy (`GID_MONKEY_VGA`, SCUMM v4), with notes on DOS CD (`GID_MONKEY`, v5).

**Citation conventions**
- Engine paths are relative to `third_party/scummvm/`.
- `scumm.cpp`, `scumm.h` and `input.cpp` line numbers are from the patched working tree. The bridge adds 1 line in `input.cpp`, 3 in `scumm.h` and 7 in `scumm.cpp`. All other engine files are stock.
- DOS VGA scripts are cited as `data/scripts-dos-vga/<file> [XXXX]` (the `descumm -4` dump).
- "Inference:" marks anything not read directly from code or data.
- Scratch tools mentioned below are in `/tmp/claude-1000/-home-ilyask-projects-fast-mi/fddeb01d-e1c6-4709-aec7-b9adf056eb93/scratchpad/`. They are not part of the repo.

**Facts that frame everything else**
- Our `game/dos/000.LFL` md5 is `15e03ffb…`, which is `scumm-md5.h:95`: English DOS VGA, `kPlatformDOS`.
- v4 creates plain `Actor` objects. `Actor_v3` is used only when `version == 3` (`scumm.cpp:2084-2097`). The walking code below is therefore the v4-6 code path.
- `ScummEngine_v4` subclasses `ScummEngine_v5` (`scumm_v4.h`). The only v4 overrides relevant here are `o4_oldRoomEffect` (`script_v4.cpp:118-142`, opcodes 0x5C/0xDC at `:42-43`) and the v4 resource and variable setup.

---

## Summary for the bot

1. **The RTA clock counts more than ticks.** Real time is Σ(every quarter-frame passed to `waitForTimer`) ÷ f, plus modal-GUI time.
   - f = 1193182/5041 = 236.6955 Hz on DOS v4.
   - f = (1193182/4096)·(3433/4167) = 239.9922 Hz on DOS v5.
   - A bridge that sums only `delta` misses room-transition effects (0, 18 or 104 quarter-frames per room change on VGA), banners and dialogs (§1).
2. **Walking has two engine-level exploits** (§2.4, §2.5).
   - **(a) Truncated clamp.** The x-speed clamp in `calcMovementFactor` compares a *truncated* quotient. A diagonal of slope just under (speedx+1):speedy therefore moves almost speedx+1 px per frame in x, against speedx when walking straight. That is +12% for Guybrush's (8,2), +50% on the Mêlée map (2,2) and +100% on the Monkey Island map (1,1).
   - **(b) Fraction reset.** Every new walk target resets the 16-bit fraction accumulators. Each axis is then rounded *away from zero* when moving left or up, and *toward zero* when moving right or down. In MI1 VGA's perspective-scaled rooms this makes per-frame re-clicking much faster for left and up moves, and much slower (down to 0 px vertically) for right and down moves.
   - Neither exploit gives an extra step per frame.
3. **Only the logo glitch gets `VAR_TIMER_NEXT` below 6 for good** (§3). The var is still var 19 on v4. The engine never writes it for MI1.
   - The circus script that sets it to 1 is shielded: user input is off and no override is live.
   - The boot script's 0 never spans a frame.
   - A load restores whatever value was saved.
4. **Audio** (§4).
   - `music_driver=null` gives the shortest waits. That is ScummVM's "No music", which is banned.
   - Of the real music options, **PCjr/Tandy (`pcjr`)** gives the shortest blocking waits: the intro music wait is skipped, and the Part IV wait is 8.75 s against 9.35 s on AdLib.
   - Determinism needs a mixer-driven player plus the bridge's tick-locked pumping.
   - The pump rate should be 22050·4·delta/f, not 22050·delta/60. The current rate runs music 1.4% slow in ticks, which lengthens sound waits.
5. **RNG and snapshots** (§5).
   - The RNG state is a single `uint32` that is **not saved**, so a hotkey load is an RNG reroll.
   - v4 with the null mixer has no game-logic threads and no wall-clock input.
   - `fork()` is an exact snapshot if the child calls `closeRoom()` and opens its own output files.
6. **Input** (§6). One click per frame at most. It is accepted in the same frame, from the frame after a cutscene ends, and from the room-entry frame onward.
7. **New levers to clear with the rules** (§1.5, §6.4).
   - **Ctrl+R snap scroll** is an original v4 interpreter key that makes every camera move instant.
   - **Ctrl+W** is a script-level "win" key. It is almost certainly banned.
   - `copy_protection=false` skips the Dial-a-Pirate script 152.

---

## 1. Frame clock and RTA

### 1.1 How the main loop spends wall time

- `go()` reads `delta = VAR(VAR_TIMER_NEXT)` at the top of every iteration (`scumm.cpp:2818`). It raises values below 1 to 1 (`:2831-2832`), so var 19 = 0 runs at delta 1. No MI1 rewrite of delta applies:
  - the FM-Towns `_scrollDeltaAdjust` (`:2826-2829`);
  - v0 and MM v1 (`:2841-2852`);
  - the `kEnhUIUX` rewrites for Indy3, Loom and Zak (`:2859-2873`);
  - COMI (`:2877-2879`).
- It calls `waitForTimer(delta * 4)` (`:2886`), then `scummLoop(delta)` only if `!isPaused()` (`:2889-2890`).
- `waitForTimer(q)` (`scumm.cpp:2916-2971`):
  - `msecDelay = getIntegralTime(q * (1000 / getTimerFrequency()))` (`:2918`). `getIntegralTime` carries the fractional milliseconds forward (`:2973-2982`), so the average error is zero.
  - The delay is measured from `_lastWaitTime`, i.e. `msecDelay - (now - _lastWaitTime)` (`:2925-2927`).
  - On exit, `_lastWaitTime = endTime`, or `now` if more than 50 ms late (`:2970`).
- **Consequence: every `waitForTimer` call adds its full q to wall time.**
  - An effect wait inside `scummLoop` sets `_lastWaitTime` to its own end.
  - The next main-loop wait then waits a full `4·delta` after that point.
  - CPU time spent in `scummLoop` is absorbed into the next wait, unless a frame overruns by more than 50 ms. In that case the excess becomes real time (`:2970`).

### 1.2 Timer frequency

From `setTimerAndShakeFrequency()` (`scumm.cpp:2984-3057`) and the constants at `scumm.h:315-325`:

| Release | Formula | f (quarter-frames/s) | 1 jiffy (4 qf) | delta 6 frame | delta 5 frame |
|---|---|---|---|---|---|
| DOS VGA v4 | `1193182 / 5041` (`:2999-3002`) | 236.6955 | 16.8993 ms | 101.396 ms | 84.497 ms |
| DOS CD v5 | `1193182/4096 · 3433/4167` (`:3003-3006`) | 239.9922 | 16.6672 ms | 100.003 ms | 83.336 ms |
| Mac (reference) | default 240.0 (`:2985`) | 240.0 | 16.6667 ms | 100.000 ms | 83.333 ms |

- The VGA detection row has platform UNK, but our files resolve to DOS (`scumm-md5.h:95`), so the PIT branch applies.
- `getTimerFrequency()` returns `_timerFrequency` except in a ZAK FM-Towns hack (`scumm.cpp:3060-3088`).
- **A VGA tick is 1.39% longer than a CD tick.** Convert ticks to seconds per release before comparing routes.

### 1.3 Every wait outside the main-loop wait

"Σdelta misses it" means a bridge that sums `delta` in `onFrameBegin` undercounts RTA by this amount.

| Site | Citation | MI1 VGA v4 | MI1 CD v5 | Quarter-frames | Σdelta misses it? |
|---|---|---|---|---|---|
| Room-change fade-out | `startScene` → `fadeOut(_switchRoomEffect2)` (`room.cpp:55`), run inside `runAllScripts` | yes | yes | depends on effect, see below | yes |
| Room-change fade-in | `_doEffect` (`room.cpp:271`) → `scummLoop_handleEffects` → `fadeIn(_newEffect)` + `clearClickedStatus()` (`scumm.cpp:4117-4121`), same frame | yes | yes | depends on effect | yes |
| `transitionEffect` (effects 1-6) | `gfx.cpp:4521-4620` | effect 1 (iris) only | same | iterations × delay. delay = `kPictureDelay` = 4 on v4, because `VAR_FADE_DELAY` is 0xFF there (`gfx.cpp:4530-4537`, `:79`; the var is assigned only for v5+, `vars.cpp:100`). delay = `VAR(59)`, default 3, on v5 (`vars.cpp:827-828`). Iris: 13 iterations; box wipes: 25 (`gfx.cpp:105-170`, `:4555-4559`). The wait is skipped only when `_fastMode` is set (`:4609-4611`). | yes |
| `dissolveEffect` | `gfx.cpp:4623-4777`. Effect 128 → `dissolveEffectSelector` → `dissolveEffect(8,4)` on DOS (`:4963-4978`); returns at once if `_macScreen` (`:4625-4626`) | yes | yes | 8×4: 18 waits of 1 qf. Only offsets `< vs->h` (144) halt (`:4764`), which is 18 strips. 1×1 (134): 18. 1×h (135): 144 | yes. It also draws w·h−1 RNG numbers (`:4667`, `:4687`) |
| `scrollEffect` (effects 130-133) | `gfx.cpp:4780-4960` | not used by MI1 | not used | 17 (vertical) or 39 (horizontal) iterations × delay | yes |
| `o4_oldRoomEffect(0)` → `fadeIn` | `script_v4.cpp:139-141` | never: all 41 VGA uses are `-set` | n/a | — | — |
| Screen shake | `updateScreenShakeEffect`, called inside `waitForTimer` (`scumm.cpp:2937`, `gfx.cpp:4980-5007`). Uses `getMillis`, only sets the shake offset | yes | yes | 0 | no time |
| Palette cycling / `palManipulate` | `scumm.cpp:4114-4116`, `palette.cpp:779-780` | yes | yes | 0 | — |
| Banners (`showBannerAndPause`, `showOldStyleBannerAndPause`, `printMessageAndPause`) | `gfx_gui.cpp:82`, `:474`, `:360`. They wait in `waitForBannerInput` (`gfx_gui.cpp:576`), which loops `waitForTimer(1)` until *wall-clock* time passes `waitTime` jiffies or a key or click arrives (`input.cpp:627-660`) | only with `original_gui=true`: pause (`input.cpp:1031-1047`), restart, snap scroll (§1.5), version | same | open-ended. The engine is paused (`pauseEngine`, `gfx_gui.cpp:508`), so no game frames run | yes. Summing qf here is meaningless; avoid banners |
| Original-GUI main menu | `gfx_gui.cpp:2521` loop; 60 qf per button press (`:2610`); 60/300/60 qf on save/load operations (`:2903`, `:2961`, `:2972`) | only with original GUI | same | as listed | yes |
| ScummVM GUI dialogs (`runDialog`) | pause = `pauseDialog` (`scumm.cpp:4212-4214`, `:4436-4440`); F8 `confirmRestartDialog` (`:4456-4461`); F5 global menu (`input.cpp:1400-1411`); `messageDialog` (`scumm.cpp:4429-4434`) | with `original_gui=false` | same | wall clock until dismissed | yes |
| Quicksave confirmation | `TimedMessageDialog` for 1500 ms after a non-autosave save when not using original GUI (`scumm.cpp:3453-3458`) | yes | yes | 1500 ms | yes |
| Load | `loadState`: no fades or waits (fork report: `saveload.cpp:662-951`); `clearClickedStatus` afterwards (`scumm.cpp:3461-3462`) | yes | yes | 0 (file I/O only) | — |
| Restart | `restart()` (`scumm.cpp:4216-4258`): no waits, but it re-runs the boot script and the whole logo | yes | yes | 0 | — |
| Paused main-loop iterations | `go()` still calls `waitForTimer(delta*4)` when `isPaused()` but skips `scummLoop` (`:2886-2890`) | possible | possible | 4·delta each | yes |
| CD audio loop | `_sound->updateCD()` inside `waitForTimer` (`:2934`) | no | yes | 0 | — |
| Mac GUI | `_macGui->update` / `updateWindowManager` (`scumm.cpp:2896-2897`, `:2944-2945`); Mac dialog `delayMillis` (`macgui/*.cpp`) | `_macGui` is null on DOS | n/a | — | — |
| INSANE / SMUSH | `scumm.cpp:2729-2741` (Rebel), `smush/smush_player.cpp:1333-1600` | v7+ only | n/a | — | — |
| FM-Towns, NES, HE | `gfx_towns.cpp:219-222` (guarded by `_townsScreen`), `gfx_nes.cpp:29-200`, `he/cup_player_he.cpp:101-193` | n/a | n/a | — | — |

**What MI1 actually triggers on a room change.** `o4_oldRoomEffect(a)` sets `_switchRoomEffect = a & 0xFF` (fade-in) and `_switchRoomEffect2 = a >> 8` (fade-out) (`script_v4.cpp:136-138`).

| Script value | Bytes | Fade-out | Fade-in | Cost v4 | Cost v5 | Where |
|---|---|---|---|---|---|---|
| −32383 | 0x8181 | 129: blit black, no wait (`gfx.cpp:4491-4496`) | 129: nothing (`:4421`) | **0 qf** | 0 | Boot default (`global/script-001.txt [069D]`) and 29 more sites |
| −32384 | 0x8180 | 129 | 128: dissolve 8×4 | **18 qf** (76 ms) | 18 qf | `global/script-088/089/090` (cu-bar close-ups), `script-049` (Meathook), `script-100/101/137` (village), `script-116` (cu-gov), `room-083-cu-dock/entry [0023]` |
| 257 | 0x0101 | 1: iris | 1: iris | **104 qf** (439 ms) | 78 qf (325 ms) | `global/script-105 [010C]` (galley), `room-083-cu-dock/local-200 [0C7C]` |

- The fade-out effect runs only if `_screenEffectFlag` is set and the effect is non-zero (`gfx.cpp:4471`).
- The Mac v5 dump has the same distribution (31× −32383, 9× −32384, 1× 257) and never writes `VAR_FADE_DELAY`. Inference: DOS CD is the same.

### 1.4 The RTA formula and how to measure it

For ScummVM on DOS VGA, assuming no frame overruns by more than 50 ms:

```
RTA = ( Σ_frames 4·d_i  +  Σ_effects q_j ) / f   +  Σ modal-GUI wall time
d_i = max(1, VAR(19))        read before frame i            (scumm.cpp:2818, 2831)
q_j = quarter-frames passed to every other waitForTimer call (table above)
f   = 1193182/5041 = 236.6955 Hz (v4)   |   1193182·3433/(4096·4167) = 239.9922 Hz (v5)
```

- For a pure v4 run with no dialogs, this is `RTA[ms] = 4.224837 · Σ q`, with q summed over **all** `waitForTimer` calls.
- **Recommended bridge change.** Count `quarterFrames` at the top of `waitForTimer`, before the `skipWaits` zeroing at `scumm.cpp:2920`.
  - Report two numbers: main-loop qf (= 4·Σdelta) and other qf.
  - Effect waits still go through `waitForTimer` when `skipWaits` is on, because `skipWaits` only zeroes the sleep. Only `transitionEffect` checks `_fastMode` (`gfx.cpp:4609`), and `_fastMode` must stay 0. So the counter is exact in headless runs too.
  - Treat any banner or dialog as a hard error, rather than trying to time it.
- **Audio pump rate (from the audio fork).** The bridge currently pumps 22050·delta/60 samples per frame. On v4 one tick lasts 4/f s, so real playback is 22050·4·delta/236.6955 samples per frame: 372.6 per tick against 367.5. The 60 Hz pump therefore feeds music 1.4% *slower* than real time. Every song takes about 1.4% *more* ticks, so `isSoundRunning` and var-14 waits are longer than on real timing. Pumping 22050·4·delta/f (carrying the remainder) is both faithful and shorter.

### 1.5 Snap scroll (Ctrl+R): a camera lever to clear with the rules

- **v4 only.** `snapScrollKeyEnabled = (version >= 2 && version <= 4)` (`input.cpp:968`).
- **The key.**
  - With the original GUI, Ctrl+R toggles `_snapScroll`, shows the "Snap scroll on" old-style banner (≤90 jiffies, dismissable by any key or click), and sets `VAR(VAR_CAMERA_FAST_X=26)` (`input.cpp:1262-1286`, `vars.cpp:62`).
  - With `original_gui=false`, ScummVM's handler opens a modal `messageDialog` instead (`input.cpp:1429-1439`).
- **Effect.** With `snapToX` set (`camera.cpp:97`):
  - the camera jumps straight to its destination instead of moving 8 px per frame (`camera.cpp:146-153`);
  - follow-mode re-centres by ±80 px at once (`:126-131`);
  - out-of-range clamps snap (`:101-117`).
- **What speeds up:**
  - `SO_WAIT_FOR_CAMERA` finishes at once (`script_v5.cpp:3292-3295`);
  - `displayDialog` no longer stalls while the camera moves (`string.cpp:1045-1049`). That stall delays both the end and the start of text lines.
- **Data check.** No MI1 VGA script writes var 26. One debug-looking print reads it as a string number (`room-035-low-stree/local-217 [003C]`).
- Inference: this emulates the original interpreter's Ctrl+R. If the rules accept it as an in-game option, press it once right after boot for a camera-pan saving in every scrolling cutscene and room. With the original GUI the press costs about one `waitForTimer(1)` if a second key closes the banner straight away.

---

## 2. Walking mechanics ("shmoovement", weird dances, early exits)

### 2.1 Frame order and the per-frame step

- `walkActors()` runs once per frame, after `runAllScripts`, `checkExecVerbs` and `checkAndRunSentenceScript` (`scumm.cpp:3227-3251`, `:3280`). It calls `walkActor()` for every actor in the room (`actor.cpp:475-480`).
- A click therefore acts in the same frame it is read. `startWalkActor` from the input or sentence script only *arms* the walk, and the step happens later in the same frame.
- The walking scale is refreshed every redrawn frame. `drawActorCostume` → `setupActorScale()` (`actor.cpp:2533`, `:451-467`) runs in `scummLoop_handleActors` (`scumm.cpp:3288`) after `walkActors`. The scale for frame N+1's step is `getScale(walkbox, x, y)` at the end of frame N (`boxes.cpp:247-275`).

**Speeds**
- Default `(speedx, speedy) = (8, 2)` (`actor.cpp:170-171`, `:211`).
- Scripts set others through `ActorOps WalkSpeed` → `setActorWalkSpeed` (`script_v5.cpp:479-483`, `actor.cpp:494-506`). The VGA data uses:
  - (1,1) for ego on the Monkey Island map rooms (`room-002-monkey-1/entry [0072]`, `room-004-monkey-3/entry [0024]`, `room-005-monkey-4/entry [0015]`);
  - (2,2) on the Mêlée map (`room-085-melee/entry [00A9]`);
  - (12,4) inside dock boxes ≤5, restored to (8,2) outside them (`room-033-dock/local-202 [0000]-[0028]`).
- `setActorWalkSpeed` while moving calls `calcMovementFactor`, which gives one extra step that frame (`actor.cpp:503-504`). That happens once per box crossing on the dock and cannot be triggered by input.

**Scale.** The box scale is the box's `scale` word (`boxes.cpp:313-323`). If bit 15 is set, it is a linear-in-y scale slot from the room's `SCAL` (`boxes.cpp:258-275`, `:277-311`, `room.cpp:603-626`).
- On v4 the blocks are the small-header `BX` and `SA`: `resource.cpp` maps `SCAL` to `SA` and `BOXD` to `BX`. A `BX` block is a count byte followed by 20-byte boxes (`boxes.cpp`, `getBoxBaseAddr` small-header branch).
- A census of the DOS VGA export (all 83 rooms, 703 boxes; scratch `boxscale_v4.py` over `dosblocks/**/BX` and `SA`):
  - 418 boxes are at 255;
  - 219 boxes use slots;
  - 66 use fixed values from 30 to 210.
- The slot values match the Mac v5 data room for room.

| Room | Slot (s1 at y1 → s2 at y2) |
|---|---|
| 33 dock | 32 @76 → 210 @131 |
| 35 low street | 14 @74 → 255 @124 |
| 25 village | 255 @133 → 30 @104 |
| 34 high street | 25 @35 → 75 @110 |
| 52 circus grounds | 255 @95 → 50 @135 |

- Every box is at 255 in 50 rooms, including the maps (rooms 2-6 and 85), the bar (28) and the forest (58).
- Pixel speed scales with s, so walking lower on screen (larger s) is several times faster in perspective rooms.

**One step** (`actorWalkStep`, `actor.cpp:633-681`):

```
x += ((deltaXFactor >> 8) * scale + xfrac) >> 16,  xfrac = low 16 bits   (actor.cpp:662-664)
y likewise with deltaYFactor, yfrac                                      (actor.cpp:666-668)
each axis clamped so it never passes next.x / next.y                     (actor.cpp:670-674)
v4-6: reaching next exactly ends the leg in the same call                (actor.cpp:676-679)
```

`>> 16` is a floor. Negative steps therefore round away from zero.

**Direction factors** (`calcMovementFactor`, `actor.cpp:520-576`):

```
dY = ±speedy<<16 ;  dX = dY*diffX/diffY           (C division, truncates)   (:529-539)
if (|dX / 0x10000| (C-truncated) > speedx) {       // the clamp test         (:547)
    dX = ±speedx<<16 ;  dY = dX*diffY/diffX }                                (:548-556)
xfrac = yfrac = 0 ; cur = pos ; next = target                                (:560-565)
_targetFacing = |dY|*3 > |dX| ? up/down : left/right   (v<7)                 (:572)
return actorWalkStep()            // the arming frame moves exactly one step (:575)
```

At full scale Guybrush moves (7.97, 1.99) px per frame on a 4:1 diagonal, 7.97 px horizontally and 1.99 px vertically. Each axis is capped separately (a box, not a circle), so a 4:1 diagonal costs nothing in x.

### 2.2 What a new walk command does (and does not do)

`startWalkActor` (`actor.cpp:850-917`):
- On v4 the raw target is used if it lies inside the previous `destbox`. Otherwise `adjustXYToBeInBox` snaps it to the nearest box point (`:858-861`, `:884-888`, `:1983-2084`).
- **Same target, same direction while moving: ignored** (`:890-891`). A repeated click on the same pixel, or on another pixel that snaps to the same point, does nothing.
- **Target equals the current position:** only `turnToDirection(dir)`, and `_moving` is untouched (`:894-900`). A floor click passes dir = −1, which `turnToDirection` ignores (`:1674-1676`).
- Otherwise it stores the target and sets `_moving = (_moving & MF_IN_LEG) | MF_NEW_LEG` (`:903-914`). It moves nothing.

In the same frame, `walkActor` (`actor.cpp:945-1015`) runs:
- `MF_NEW_LEG` **skips** the in-leg step (`:952-954`);
- it plans the next box or gate (`:982-1011`);
- it calls `calcMovementFactor`, which does **exactly one** step after resetting the fractions (`:1007`, `:1014`).

Therefore:
- **No extra or free step.** A re-click frame moves one step, the same as a normal walking frame.
- **No turn frames to skip while walking.** Turning happens inside `actorWalkStep`: `updateActorDirection(true)` moves at most one 90° notch per step through `actorTurnInterpolateTable` (`actor.cpp:641-645`, `:1544-1555`, `:1502-1505`), and the position still advances that frame.
- **It does change the factors.** The direction is recomputed from the current position, which matters for §2.4. The fractions are reset, which matters for §2.5.
- **Walk start:** the click frame already moves.
- **Walk end:** the stop happens in the frame of the last step, inside the in-leg branch (`:953-963`). If the arming frame itself reaches the target, the stop is detected one frame later (`:953-957`), and `_moving` stays non-zero for that extra frame.

### 2.3 Leg boundaries (box gates)

- When an in-leg step lands exactly on an intermediate waypoint, `actorWalkStep` returns 0. `walkActor` then sets the box and plans the next leg **in the same frame**, and `calcMovementFactor` takes another step (`:953`, `:980-981`, `:1001-1014`).
- So a frame that crosses a waypoint moves the leftover distance to the waypoint **plus** one full step.
- Inference: a straight walk split at a waypoint never takes more frames than the unsplit walk, and saves one frame when the two legs' fractional remainders add up to more than one step.
- `_walkbox` changes when a step *starts* inside the next box (`:647-648`), or at a leg end. A box-polling script therefore usually sees the new box one frame after the position enters it.

### 2.4 Exploit A: the truncated clamp (diagonals beat straight lines)

- The clamp at `actor.cpp:547` compares `trunc(|dX|/65536)` with `speedx`. Any `dX` below `(speedx+1)<<16` passes unchanged.
- Code comment for bug #14582 (`:541-546`): v4-6 disassembly divides by 0x10000. Inference: the original interpreter behaves the same way.
- With `dY = speedy<<16`, a direction with `diffX/diffY` just below `(speedx+1)/speedy` gives x ≈ (speedx+1)·s/256 and y = speedy·s/256 per frame.
- Walking straight sideways (`diffY = 0`) or at a steeper ratio is clamped to exactly speedx (`:548-556`).
- The y cap is exact in both branches: speedy at most.

Scratch harness `zigzag.py`: a line-by-line mirror of `calcMovementFactor`/`actorWalkStep`/`walkActor` for a single box at constant scale, with no gates, `setBox` or per-frame scale refresh. 120 frames, px per frame.

| Speeds (context) | Scale | Straight sideways, one click | Best single-click diagonal (slope) | Zig-zag, flipping y every 10 frames, right / left |
|---|---|---|---|---|
| (8,2) Guybrush | 255 | 7.97 | 8.93 (148:33) | 8.90 / 9.00 |
| (8,2) | 160 | 5.00 | 5.60 | 5.60 / 5.70 |
| (8,2) | 100 | 3.13 | 3.50 | 3.50 / 3.60 |
| (12,4) dock boxes ≤5 | 255 | 11.95 | 12.92 (107:33) | 12.90 / 13.00 |
| (2,2) Mêlée map | 255 | 1.99 | 2.96 (55:37) | 2.90 / 3.00 |
| (1,1) Monkey Island map | 255 | 0.99 | 1.96 (63:32) | 1.90 / 2.00 |

**Rule.** For mostly-horizontal travel, never click a level target. Click targets at slope `diffX:diffY` just under `(speedx+1):speedy`:
- below 4.5:1 for Guybrush (e.g. 40:9, 49:11);
- below 1.5:1 on the Mêlée map;
- below 2:1 on the Monkey Island map.

Flip the vertical sign whenever the walkable band runs out, which is a "weird dance". The band needed is about speedy·s/256·k px per k frames, roughly 25 px for 10 frames at full scale.
- Targets must be inside the current box polygon, or snap to a point with the right slope.
- A target in another box makes the first leg head for a gate point (`findPathTowards`), not along the chosen slope.
- Clicks are limited to the visible screen: `_mouse` is clamped to 320×200 and `VAR_VIRT_MOUSE = mouse + xstart` (`input.cpp:416-434`).

### 2.5 Exploit B: fraction reset on every new target ("shmoovement")

- `calcMovementFactor` zeroes `xfrac`/`yfrac` (`actor.cpp:560-561`), and the first step after it floors the scaled step. In effect:
  - per-frame re-clicks give `ceil(step)` on axes moving **left or up** and `floor(step)` on axes moving **right or down**;
  - holding one target gives the exact average.
- Scratch `walksim.py`, 120 frames, px per frame (hold = one click; reclick = a new target every frame, alternating between two points 1 px apart so `:890-891` never ignores the click):

| Scale | Exact step x / y | Left: hold / reclick | Right: hold / reclick | Up: hold / reclick | Down: hold / reclick |
|---|---|---|---|---|---|
| 255 | 7.97 / 1.99 | 7.97 / 8.00 | 7.97 / **7.00** | 2.00 / 2.00 | 1.99 / **1.00** |
| 210 | 6.56 / 1.64 | 6.57 / 7.00 | 6.56 / 6.00 | 1.64 / 2.00 | 1.63 / 1.00 |
| 140 | 4.38 / 1.09 | 4.38 / 5.00 | 4.38 / 4.00 | 1.10 / **2.00** | 1.09 / 1.00 |
| 100 | 3.12 / 0.78 | 3.12 / 4.00 | 3.12 / 3.00 | 0.78 / 1.00 | 0.78 / **0.00** |
| 50 | 1.56 / 0.39 | 1.57 / 2.00 | 1.56 / 1.00 | 0.39 / **1.00** | 0.38 / 0.00 |
| 32 | 1.00 / 0.25 | 1.00 / 1.00 | 1.00 / 1.00 | 0.25 / **1.00** | 0.25 / 0.00 |

- In unscaled rooms (s = 255) the gain is negligible: +1 px per 32 frames leftward.
- In perspective-scaled rooms it is large. Moving up at s = 50 goes from 0.39 to 1.00 px per frame.
- Clicking every frame while walking down at s ≤ 127 freezes vertical motion: Guybrush walks in place.

**Rules**
- Re-click every frame only while every moving axis is left or up.
- When moving right or down, re-click only to change direction, i.e. the zig-zag flips of §2.4.
- Combine with A: for up-left use slope ≈ 4.45:1 and re-click every frame. For s = 255 the harness gives (−9, −2) px per frame against (−7.97, −1.99) straight.

**Inference:** "shmoovement" as runners describe it is A, B or both. Repeated clicks above and below the line of travel make Guybrush zig-zag ("weird dances") while gaining up to 1/speedx in x. Re-clicks toward upper-left in scaled rooms gain more.

**Bot recipe.** For each walk segment, run a beam or DP search over (position, xfrac, yfrac, facing) with per-frame actions:
- no click;
- a click at (x, y) inside the walkable area, distinct from the current target after snapping.

The scratch `walksim.py` mirrors the code for one box at constant scale. Extending it with box gates (`findPathTowards`), `setBox`, and per-frame scale from the room's slots gives an exact per-room cost model. The planner should replace its straight-line walk times with these.

**Validation before the planner relies on it** (not done here): in one room, inject alternating slope-4.45 clicks through the bridge's raw-click path (`speedrun/speedrun_inputs.cpp:134-176`). Compare ego x per frame against a single level click. Repeat on the Mêlée map at slope ≈ 1.45, and in a scaled room (e.g. the dock) for the up/down asymmetry. The table values are mirror results, not engine runs.

### 2.6 Walk-to-object versus floor click

**Floor click** with Walk to selected (`Var[107]==10`, no object): input script 4 runs `doSentence(STOP)`, `stopScript(2)` if needed, then `walkActorTo(ego, VAR_VIRT_MOUSE_X, VAR_VIRT_MOUSE_Y)` (`global/script-004.txt [045C]-[0475]`).
- dir = −1 (`script_v5.cpp:3359`), so there is no final turn.
- A floor click kills any running sentence, including a walk to a door.

**Object click:** `doSentence(STOP)` and then `doSentence(Var[107], Var[108], Var[109])` (`script-004 [0440]-[044B]`). Sentence script 2 starts in the same frame (`scumm.cpp:3251`). For an object in the room it does:
1. `walkActorToObject(ego, obj)` (`script-002 [02DB]`). The target is the object's OBCD `walk_x`/`walk_y` (`object.cpp:506-508`), snapped by `startWalkActor`. dir = `oldDirToNewDir(actordir & 3)` (`object.cpp:510-513`, `script_v5.cpp:3417-3428`).
2. `WaitForActor(ego)` (`[02E0]`). This waits while `a->_moving != 0`, including `MF_TURN` (`script_v5.cpp:3281-3284`).
3. `getDist(ego, obj) <= 16`, a Chebyshev distance with the object point snapped into ego's box (`[02E4]-[02EB]`, `object.cpp:522-553`, `:516-520`). If it fails: "I can't reach it."
4. For classes 142-144, a 20-jiffy reach cutscene, except for verbs 9, 10 and 13 (`[032A]-[0397]`).
5. `startObject(obj, verb)` (`[039D]`).

**Class exceptions** (initial classes from `000.LFL` block `0O`, parsed per `resource_v4.cpp` `readGlobalObjects`; scratch `classes.py`):
- **Class 138: walk to the clicked pixel, no distance check** (`[02C1]-[02D5]`, `[01AC]-[01CD]`). 46 objects have it, including:
  - the Monkey Island map hotspots (rooms 2-6);
  - the "jungle" exits on beach room 1 and main beach room 20 (`room-020-main-beac/obj-0269-jungle`);
  - the Scumm Bar curtain 334 and object 328.
  - Hellmaze objects get it at run time (`room-039-hellmaze/local-200 [0413]` etc.).
- **Class 136: no walk at all, act next frame** (`[02B4]-[02BE]`). Examples: the sun at the fork, Low Street clock 457, the crossing cables, bar objects 328 and 334. The 136 test comes first, so the curtain is "no walk".

### 2.7 Exits and "early door entry"

1. **Sentence doors.** These are most doors, e.g. dock door 437, whose Walk-to entry runs `loadRoomWithEgo(322,28)` when open (`room-033-dock/obj-0437…` `[0039]-[003F]`).
   - The room changes in the frame after ego stops at the walk point, plus 2 frames for a 90° final turn or 3 for 180°.
   - Turn mechanics: on arrival `walkActor` calls `turnToDirection(destdir)` only if `_targetFacing != destdir` (`actor.cpp:957-963`). That sets `_moving = MF_TURN` (`:1684-1685`). Each following frame turns one interpolated 90° notch, and `_moving` clears one frame after the facing matches (`:966-977`).
   - **Early entry recipe:**
     - make the final leg's `_targetFacing` (`actor.cpp:572`) equal the door's `actordir`: a mostly-vertical approach for up/down doors, mostly-horizontal for left/right;
     - shmoove up to the walk point with floor clicks (§2.4-2.5);
     - click the door on the last approach frame. The door click is a new target, so it resets the fractions too. Alternate floor and door clicks to keep resetting.
   - If ego is already *on* the walk point facing `actordir` when the door is clicked, `startWalkActor` returns without setting `_moving` (`:894-900`). `WaitForActor` falls through, and the door fires in the **click frame**.
   - The 16-px tolerance is not exploitable for sentence doors, because `walkActorToObject` always targets the walk point exactly.
2. **Class-138 exits** (the beach and jungle exits, the map hotspots). The target is the clicked pixel and there is no distance check. Clicking the nearest pixel of the hotspot rectangle (`findObject`) that snaps to a reachable box point ends the walk early. The exit fires one frame after the stop, with no turn.
3. **Walkbox triggers.** These are local scripts polling `getActorWalkBox(VAR_EGO)`, e.g.:
   - `room-011-vista-1/local-204` (box 7 → script 205);
   - `room-052-circus-gr/local-202` (box 6 and x > 200);
   - `room-048-crossing`, `room-029-fortune` and `room-018-crack` scripts.
   - The trigger is `_walkbox` changing. That happens at the first step *starting* inside the box, or at a leg end landing in it (§2.3). The polling script reacts in the next frame's `runAllScripts`.
   - Inference: aim for a leg end exactly on the box edge so the change registers in the landing frame.

---

## 3. `VAR_TIMER_NEXT`

**Index.** Var 19 for all versions that use `ScummEngine::setupScummVars` (`vars.cpp:55`). The v4 block (`:72-79`) and v5 block (`:81-110`) do not move it. Note that `VAR_TALKSTOP_KEY` exists only in v5 (`:99`), so on v4 `'.'` is always enabled (`input.cpp:965`).

**Engine-side writes for MI1: none.**
- Every engine reference is a read: `scumm.cpp:2818`, `palette.cpp:779-780`, `gfx_towns.cpp:249`.
- The writers at `script.cpp:766` and `:786` (`kEnhTimingChanges`) apply to SAMNMAX and Loom only.
- `resetScummVars` does not touch it.
- `restart()` zeroes all vars (`scumm.cpp:4245-4246`) and re-runs the boot script.
- A load restores it with the other vars.

**DOS VGA script writers** (the only ones, from a grep of the dump):

| Script | Write | Restore | Notes |
|---|---|---|---|
| `global/script-001` boot | `[0019]` = 0 | `[0741]` = 6 | No `breakHere`, `delay` or `wait` between them, and nested `startScript` returns, so 0 is never read by `go()`. `restart()` repeats this within one call. |
| `room-010-logo/local-204` | `[0000]` = 5 | `[0097]` = 6 | **Logo glitch.** Started by logo script 149 `[00B0]` while 149's override `[007F]`→`goto 0A38` is live (`global/script-149.txt`). Esc → 149 ends (`[0A3E]-[0A3F]`) → boot `loadRoom(0)` `[0784]` → `startScene` → `killScriptsAndResources` kills `WIO_LOCAL` 204 (`room.cpp:80`, `script.cpp:1083-1104`). Var 19 stays 5. |
| `room-051-circus-te/local-208` | `[0005]` = 1 | `[005F]` = saved | 14 frames at delta 1, started and awaited by local 207 `[03C4]-[03CC]`. |
| `global/script-097` (fighting) | `[0005]` = 10 | `[014E]` | Slower; killing it would hurt. |
| `global/script-044` (fork) | `[00F9]` = 12 | `[0133]` | Slower. |

- Rooms 52 and 59 only read it, to scale their own delays (`room-052-circus-gr/local-200 [0023]`, `room-059-stans/local-202 [001A]`).
- On VGA, script **152 is the Dial-a-Pirate copy protection**, not the logo. ScummVM refuses to start it when `copy_protection=false` (`script_v5.cpp:2964-2965`), so boot falls through to logo script **149** (`script-001 [0753]-[0768]`).
- Script 149 has an unskippable music wait *before* its override: `VAR_MUSIC_TIMER >= 8` when `VAR_SOUNDCARD` is 3 or 4 (`[0016]-[004C]`). See §4.

**Can anything leave var 19 below 5?**
- **Override or room-kill shape.** Only local 208 qualifies, and it is shielded:
  - Choosing the dialogue line that precedes it runs `UserputOff` in dialogue input script 14 (`global/script-014.txt [00E2]`), so no click or verb can start a room change during its 14 frames.
  - Local 207 has no override between `endOverride [01F2]` and `beginOverride [042E]`.
  - The only theoretical path is Esc reaching a *stale* level-0 `cutScenePtr` (the stale-pointer hazard in `skips-engine.md` §4) whose slot index happens to be 208's. That cannot be decided statically.
  - Inference: worth one runtime probe that logs `cutScenePtr[0]` and its slot during those frames. If it is 0, the path is closed.
- **Restart:** ends at 6, or 5 again if the logo glitch is redone after the restart.
- **Save/load:** a load sets var 19 to the saved value.
  - Every save made after the logo glitch carries 5.
  - Loading a pre-glitch save undoes the glitch.
  - Saving and loading are impossible during the logo: `VAR_MAINMENU_KEY` (50) is 0 from boot `[0050]` until `[077F]`, and both hotkeys need it non-zero (fork report: `saveload.cpp:95-169`, `:245`).
- **Pause:** no effect.
- **Verdict:** 5 (the logo glitch) is the floor reachable by play.

**Enhancements** (default `enhancements = kEnhGameBreakingBugFixes | kEnhGrp1` = 7, `engines/engine.cpp:201`; bits at `engines/enhancements.h:120-170`). None of them changes `delta`: the `kEnhUIUX` delta rewrites are Indy3/Loom/Zak only (`scumm.cpp:2859-2873`), and `kEnhTimingChanges` is SAMNMAX/Loom only.

For `GID_MONKEY_VGA`:

| Site | Bit (default) | Effect on route or timing |
|---|---|---|
| `script.cpp:1283-1294` | Minor (on) | "Give note to Herman" with the note still in the room becomes "Pick up". Matters only for that invalid action. |
| `script_v5.cpp:1550-1556` | GameBreaking (on) | Looking at the map in Meathook's house no longer crashes. |
| `script_v5.cpp:1738-1746` | RestoredContent (off) | Hold message in room 8. On, it adds a line. |
| `script_v5.cpp:2990-3000+` | RestoredContent (off) | Restores the Hellmaze actor-12 setup in script 211. |
| `script_v5.cpp:3856-3870` | RestoredContent (off) | Storekeeper `WaitForMessage`. On, it adds a wait. |
| `script_v5.cpp:3901-3910+` | VisualChanges (off) | Jolly Roger flag. |
| `resource_v4.cpp:208`, `gfx.cpp:2506` | Text/Visual | French only. |

For `GID_MONKEY` (CD), the following apply on top:
- `script_v5.cpp:1467` (room 25 `isScriptRunning(164)`, GameBreaking, on);
- `:2709` (script 68 `setObjectName` waits while inventory cutscenes run, GameBreaking, on; this can add frames);
- `:772` (clock tower, RestoredContent);
- `:3327` (storekeeper door order, VisualChanges);
- `:2851` (AudioChanges);
- `resource.cpp:1849` (cannibal script patch, RestoredContent);
- visual and colour fixes at `script_v5.cpp:474`, `554`, `658`, `2111`, `2430`, `3682`, `3694`, `palette.cpp:1646`.

**Recommendation:** pin `enhancements` explicitly. 0 is closest to the original interpreter. 1 keeps only the crash fixes. Never turn on RestoredContent.

---

## 4. Sound and music

Summarised from the audio fork. Its citations were spot-checked: `vars.cpp:860-905` and `script-128 [003B]-[0044]` are confirmed.

### 4.1 Driver → player → `VAR_SOUNDCARD` (VGA, `setupMusic` `scumm.cpp:2344-2381`, `:2511-2577`)

| `music_driver` | Player | Clock source | `VAR_SOUNDCARD` (`vars.cpp:860-905`) |
|---|---|---|---|
| `pcspk` | `Player_V2` (`scumm.cpp:2511-2512`), a mixer stream that ticks in `readBuffer` | mixer | 0 |
| `pcjr` | `Player_V2` with pcjr | mixer | 1 |
| `cms` | `Player_V2CMS` (`:2513-2514`) | mixer | 2. Boot rewrites it to 3 (`script-001 [0009]-[0010]`); a load resets it to 2 (`saveload.cpp:2159-2160`). |
| `adlib` | iMuse + AdLib MIDI driver (`:2557-2577`). `Player_AD` is Loom/Indy3 only (`:2521-2529`). | mixer (emulated OPL is an `EmulatedChip`, `audio/chip.cpp:112-146`) | 3 |
| `mt32` / `midi` | iMuse + MT-32/GM (`:2550-2555`). No AdLib part without `multi_midi`, so every SFX is silent (`imuse/imuse.cpp:204-207`, `:627-629`). | mixer if emulated (munt, FluidSynth); **SDL timer thread** for real MIDI devices | 4 (`vars.cpp:901-903`) |
| `null` | iMuse over the null MIDI driver | MPU-401 timer on the SDL timer thread (`audio/mpu401.cpp:148`); nothing ever starts | 0 |
| `auto` | AdLib, because `mt32_device` defaults to null | mixer | 3 |

### 4.2 `isSoundRunning` and var 14

- **`isSoundRunning`** (`sound.cpp:859-882`) checks, in order:
  1. CD;
  2. mixer;
  3. still in the sound queue, which returns 1 on the start frame (`startSound` only queues, `:120-137`; the queue is drained at the end of `scummLoop`, `scumm.cpp:3311`);
  4. resource not loaded, which returns 0;
  5. the player.
- **`VAR_MUSIC_TIMER`** (var 14) = `getMusicTimer() · f/240`, truncated (`sound.cpp:2219`), and updated before scripts run (`scumm.cpp:3196`). On v4 the factor is 0.986, so a raw value of 9 is needed to read 8.
- **Under `--disable-sdl-audio` with no pumping:** mixer-driven songs never end and var 14 stays 0. The logo then hangs at `script-149 [0027]` on AdLib, CMS and Roland, and the Part IV card hangs forever.
- **With the bridge's pump:** everything ends in tick time. Fix the rate as in §1.4.

### 4.3 Blocking music waits in VGA scripts

**Unskippable**
- **Logo, before the override** (`script-149 [0016]-[004C]`):
  - AdLib ≈ 2.98 s;
  - CMS ≈ 3.09 s;
  - Roland ≈ 2.67 s;
  - **0 for speaker, pcjr and null.** With `VAR_SOUNDCARD` < 3 the script takes the `>= 0` branch and does not even start the theme.
- **Part IV card** (`global/script-128.txt [0000]`, `[003B]-[0044]`, a cutscene without override that waits on `isSoundRunning(Var[84])`):
  - AdLib 9.35 s;
  - CMS 9.21 s;
  - Roland 9.06 s;
  - **speaker and pcjr 8.75 s**;
  - null 1 frame.
- **Glowing crate** (`room-075-gh-hull/obj-0781-glowing-crate [019E]-[01AC]`), skipped when `VAR_SOUNDCARD==4`:
  - speaker 2.75 s;
  - AdLib 1.83 s;
  - CMS 1.56 s;
  - **pcjr 1.33 s**;
  - Roland 0.

**Skippable with Esc:** `script-149 [0A2D]`, `room-096-part1/local-200 [000C]`, `script-119 [0018]`, `script-104 [00E9]`, `script-105 [00CE]`, `script-057 [1377]`.

**Driver-dependent RNG:** `room-008-sh-hull/local-200 [000E]` and `room-041-kitchen/local-201 [0000]` draw random numbers only when `VAR_SOUNDCARD==3`. Pin the driver for any seed search.

### 4.4 Verdict

- **Shortest waits:**
  - `null` (≈ 3 s + 8.7 s + 1.3 s saved against AdLib);
  - then **pcjr** (intro 0, Part IV 8.75 s, crate 1.33 s);
  - then pcspk (crate 2.75 s);
  - then Roland (intro 2.67 s, Part IV 9.06 s, crate 0).
  - Unless the route uses the crate, pcspk and pcjr tie.
- **Why "No Music" is banned:** with no music every `isSoundRunning` wait lasts one frame, and `VAR_SOUNDCARD` = 0 removes the logo wait. Muting does not do this: `mute` and `music_mute` change volume only (`scumm.cpp:2621-2629`, `audio/mixer.cpp:403-410`).
- **DOS CD v5:**
  - Music is CD tracks. `isSoundRunning` asks `pollCD` → `AudioCDManager::isPlaying` (`soundcd.cpp:183-186`). A missing track file "ends" at once.
  - Var 14 comes from a `TimerManager` callback (`soundcd.cpp:79-110`, `sound.cpp:2207-2208`), which is wall clock and not deterministic. The CD release needs its own deterministic CD timer before it can be searched.

---

## 5. RNG, determinism, snapshots

Summarised from the input/RNG fork. Spot-checked: `saveload.cpp` contains only `skip(4)` placeholders for the old `_randSeed1/2` at `:1618-1619`, and the hotkeys are at `input.cpp:137-150`.

**State**
- `_rnd` (`scumm.h:558`, constructed `scumm.cpp:140`) is one `uint32 _randSeed` (`common/random.h:46`), advanced by xorshift (`common/random.cpp:87-93`).
- The seed comes from `random_seed` (`random.cpp:42-44`); 0 becomes 1 (`:57-58`).
- `o5_getRandomNr` makes one draw (`script_v5.cpp:1438-1442`).

**Saves do not contain the RNG**
- `saveLoadWithSerializer` saves vars, scripts, `_talkDelay`, the sentence queue, the cutscene stack, `_userPut` and so on, but not the RNG.
- `loadState` neither reseeds nor draws (`saveload.cpp:662-951`). `restart()` does not reseed either.
- **A hotkey load is therefore an RNG reroll.** The loaded game continues from whatever state the process RNG has. Which stream it gets depends on how many draws happened before the load: dissolves, map frames, ambient scripts.

**Hotkeys**
- Ctrl+digit loads and Alt+digit saves (`input.cpp:137-150`); slot 0 becomes slot 10.
- A load costs no quarter-frames. It runs at `load_game:` inside the current frame (`scumm.cpp:3209-3210`) after that frame's delay decrement, and it drops that frame's click (`:3461-3462`).
- A save with `original_gui=false` costs a 1.5 s modal dialog (`scumm.cpp:3453-3458`). With the original GUI that dialog is skipped.
- Scripts cannot see a hotkey load: `VAR_GAME_LOADED` does not exist on v4 (`vars.cpp:108`).

**Engine draws independent of scripts on v4**
- Only `dissolveEffect`, at w·h−1 draws: 1439 for effect 128 on the 144-line screen (`gfx.cpp:4667`, `:4687`).
- The `setObjectState(0xFE)`, AKOS and HE sites are not used by v4.
- Script draws:
  - global 24 makes one draw per frame on map rooms;
  - ambient loops with `delay` make a few per second;
  - 220 `getRandomNr` sites across 90 files.
- Inference: each frame's draw count is a pure function of which scripts run on which frame.

**Determinism**
- No wall-clock value reaches v4 DOS game logic.
  - The `getMillis` uses in game code are Indy3 Mac, Loom Mac (`script.cpp:1500-1506`) and SegaCD cursors (`cursor.cpp:355-361`).
  - Banners use the wall clock but are modal; avoid them.
- An SDL timer thread always exists (`backends/timer/sdl/sdl-timer.cpp:52`). v4 installs no SCUMM timer procs; only the v5-7 speech timer (`sound.cpp:1235-1236`) and the CD timer (`soundcd.cpp:108-109`) do.
- `--disable-sdl-audio` gives a `NullMixerManager` with no thread.
- **v4 is deterministic given:** the seed, the pinned config (talkspeed, enhancements, driver, subtitles, `copy_protection`, snap-scroll state), identical per-frame inputs, and audio pumped on ticks. The driver must be mixer-driven (pcspk, pcjr, cms, adlib, munt), never `null` or real MIDI, which tick on the timer thread.

**`fork()` snapshot**
- Only the calling thread survives the fork. `DefaultTimerManager::_mutex` is taken every 10 ms by the timer thread (`backends/timer/default/default-timer.cpp:82-83`).
  - v4 never installs or removes timer procs in steady state, so the child never touches that mutex.
  - CD v5 calls `startCDTimer` on every track start (`soundcd.cpp:142`, `:483-485`), so it is at risk.
- **Resource reads.** They use a stdio `FILE*` that parent and child share. Every load seeks to an absolute position (`resource.cpp:673`), but glibc can serve an in-buffer seek without `lseek` and then `read()` at the shared kernel offset.
  - **The child must call `_vm->closeRoom()`** (`resource.cpp:139-152`), so the next `openRoom` reopens the file privately. `loadState` already does this (`saveload.cpp:776`).
- The child needs its own trace path and save slots, and must exit with `_exit()`.
- **Verdict.** Fork at a frame boundary on the main thread, with no mixer lock held: for example inside `onFrameBegin` after the audio pump. Under the conditions above, the child is an exact logic snapshot for v4, for both serial and parallel search. Without `closeRoom`, serial forks are probably fine, but parallel children are unsafe.

---

## 6. Raw click input path (v4)

### 6.1 Click → sentence, all within frame N

1. **Event.** `LBUTTONDOWN` sets `_leftBtnPressed |= msClicked|msDown` and `_mouse` (`input.cpp:226-234`). VGA uses raw 320×200 coordinates. The bridge injects the same fields (`speedrun/speedrun_inputs.cpp:134-176`).
2. **`processInput`** (`scumm.cpp:3173`, `input.cpp:409-536`):
   - clamps the mouse;
   - `_virtualMouse = mouse + vs->xstart`, `y − topline`, or −1 outside the 144-line main screen (`:426-434`);
   - `_mouseAndKeyboardStat = MBS_LEFT_CLICK` or `MBS_RIGHT_CLICK` (`:456-458`); both buttons together become Esc (`:441-447`);
   - clears `msClicked` (`:530-531`), so a click is never buffered to a later frame.
3. **Mouse vars.** `VAR_VIRT_MOUSE_X/Y` and `VAR_MOUSE_X/Y` are copied (`scumm.cpp:3381-3384`).
4. **Hover.** In `runAllScripts` (`scumm.cpp:3227`), global hover script 23 sets:
   - `Var[108] = findObject(virt mouse)`, or an inventory row (x > 168, y > 152 → `Var[134+(y−152)/8]`);
   - the default verb `Var[181]` (`script-023 [0007]-[0132]`).
5. **`checkExecVerbs`** (`scumm.cpp:3228`, `verbs.cpp:586-689`):
   - gate `_userPut > 0` (`:590`);
   - a visible verb's key fires that verb (`:610-616`), any other key goes to area 4 (`:664`);
   - the mouse gives `findVerbAtPos` → area 1 (verb), otherwise area 2 (scene) (`:683-689`).
6. **Input script 4** (boot `[077A]`):
   - **Right click:** default verb `Var[181]` (`[0000]-[0016]`).
   - **Verb click:** sets `Var[107]`. Inventory verbs 101-106 fire immediately; 107/108 scroll (`[02F0]-[037C]`).
   - **Scene click on an object:** `doSentence(STOP)` + `doSentence(verb, obj, obj2)` (`[0440]-[044B]`).
   - **Scene click on the floor:** `walkActorTo` (§2.6).
   - **Use and Give** need a third click for the second object (`[03B8]-[03DB]`).
7. **Sentence script 2** starts in the same frame (`scumm.cpp:3251`), then follows §2.6.

### 6.2 The v4 interface

- Twelve verbs built by global 22 (`[0003]-[00D8]`), with keys o c s y w p t g u l n f.
- The text inventory is verbs 101-106 (keys '1'-'6'), the arrows are 107/108 ('q'/'a'), and the sentence line is verb 100.
- Dialogue choices are verbs 120-128 (keys '1'-'9', `script-017 [00CA]-[0142]`).
- Inference on frame cost:
  - Walk to, or a right-click default verb: 1 frame.
  - Verb + object: 2 frames, or 1 if the verb's key is pressed the frame before the object click.
  - Use or Give X with Y: 3 frames.

### 6.3 Earliest accepted click

`_userPut` is tested after `runAllScripts`, so a `UserputOn` in frame N's scripts accepts frame N's click. The exceptions:
- **Cutscene end.** `endCutscene` runs global 19 nested (`script.cpp:1685-1686`): `UserputSoftOn`, then `doSentence(STOP)` unless the cutscene argument is 2 (`script-019 [0007]`, `[0026]-[002D]`). `doSentence(STOP)` clears clicks (`script_v5.cpp:1004-1008`, `input.cpp:378-385`), so **the earliest click is frame N+1**. A `cutscene([2])` end does not clear, so a same-frame click works.
- **Room change.** `startScene` does not touch userput. The fade-in `clearClickedStatus` (`scumm.cpp:4117-4121`) runs *after* `checkExecVerbs`. A click in the room-change frame itself is accepted, but it uses `VAR_VIRT_MOUSE` computed with the old camera (Inference). From N+1 a click is always accepted.
- **After a load:** frame N+1 (`scumm.cpp:3461-3462`).

**Two clicks in one frame: no.**
- Button flags are OR'd and the position is overwritten, so the last position wins.
- `processInput` reads the buttons once, and left + right together becomes Esc.
- A key overwrites the click's `_mouseAndKeyboardStat` (`input.cpp:1558`). The exception is `'.'`, which only zeroes `_talkDelay` (`:1418-1421`). So `'.'` plus a click both take effect in one frame.

### 6.4 Developer key to clear with the rules

- Ctrl+letter reaches the input script as codes 1-26 (`input.cpp:1482-1490`).
- Ctrl+W (23) prints "Are you sure you want to win? (Y/N)" and then `startScript(127,[1])`, the ending, **if `VAR_RESULT` already holds 121 or 89** (`script-004 [0287]-[02D6]`). No debug-mode check gates this path, unlike the neighbouring debug keys (`[0042]`, `[00CB]`).
- In ScummVM, print 252 does not read a key (`script_v5.cpp:3448`), so the test reads whatever `VAR_RESULT` holds.
- Inference: it is a developer cheat and almost certainly outside Any%. Rule on it explicitly.

---

## Scratch tools (not in the repo)

All under `/tmp/claude-1000/-home-ilyask-projects-fast-mi/fddeb01d-e1c6-4709-aec7-b9adf056eb93/scratchpad/`:

- `walksim.py`: a single-box, constant-scale mirror of `startWalkActor` / `walkActor` / `calcMovementFactor` / `actorWalkStep`. It produced the tables in §2.4 and §2.5.
- `zigzag.py`: the slope search and zig-zag patterns behind §2.4.
- `walktable.md`: the §2.5 table.
- `boxscale_v4.py`: box scales and `SA` slots for all DOS VGA rooms, read from `dosblocks/` (the DOS block export made by the audio fork).
  - `boxscale.py` is an earlier version run on `build/blocks`, which is the Mac v5 export.
- `classes.py`: initial object classes 136, 138 and 142-144 from `000.LFL`.
- `tools/songlen.py` and `tools/rolen.py`: song lengths for WA, AD/iMuse, CMS and RO.
