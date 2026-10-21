# Any% findings log: DOS VGA floppy

Measured facts for the Any% effort (`rules/anypercent.md`), newest last. Every
number comes from a headless run of ScummVM v2026.3.0 with the speedrun bridge
(v3), release `dos-vga` (ScummVM md5 `15e03ffbfeddb9c2aebc13dcb2a4a8f4`, SCUMM
v4), seed 1, `talkspeed` 255, `enhancements` 0, unless a line says otherwise.
Script citations are to `data/scripts-dos-vga/` (`scripts/dump-scripts.sh` with
`GAME_DIR` pointing at the floppy files).

## Clock

- ScummVM paces DOS v4 at 1193182/5041 = 236.6955 quarter-frames per second
  (`scumm.cpp` `setTimerAndShakeFrequency()`, `PIT_V2_4_DIVISOR`), so one tick
  (jiffy) is 16.8995 ms and a second is 59.174 ticks. Mac and DOS CD (v5) run at
  240 quarter-frames per second, 60 ticks per second. Converting floppy ticks at
  60 Hz would understate times by 1.4%. `speedrun.releases.Release.seconds()`
  does the conversion; the bridge's boot record carries `timer_hz`.
- ScummVM emulates no CPU. Each frame waits `VAR_TIMER_NEXT` ticks on the timer
  and then runs at once, which is what DOSBox's "max cycles" approximates. The
  only difference is that ScummVM's room loads take no time at all.
- Replaying the same input schedule with real-time pacing and with
  `SPEEDRUN_FAST` gives byte-identical end fingerprints.

## The logo glitch on the floppy

- The logo is room 10. Its local script 204 sets `VAR_TIMER_NEXT = 5` first and
  `= 6` at its end (`room-010-logo/local-204.txt [0000]`, `[0097]`); the boot
  script set 6 before that (`global/script-001.txt [0741]`). Esc while 204 runs
  kills it between the two writes and leaves var 19 at 5 for the rest of the
  game.
- With the AdLib driver, room 10 starts on frame 31. Esc on frames 45 to 91
  (both inclusive) leaves var 19 at 5. Esc on frames 40 to 44 or 92 and later
  leaves 6. The window is 47 frames, about 3.8 s.
- With the CMS driver room 10 starts a frame later, and Esc on frame 45 left 6:
  the window's frame numbers depend on the music driver.
- With AdLib, from the logo-skip Esc (frame 45, tick 269) to the first frame of
  control on the dock is 470 ticks (7.94 s) when text and cutscene skips are on
  from the logo skip, and 3170 ticks (53.6 s) without them.

## Arrival on the dock

- After the "Part One" card (room 96) the dock (room 33) starts with
  `loadRoomWithEgo(435,33,346,133)` (`room-096-part1/local-200.txt [0020]`):
  Guybrush walks from the cliff stairs to (346,133). User input is on from the
  first dock frame (frame 49 in the run above): a raw click on frame 50 at
  screen (319,130) replaced the arrival walk.
- The glitchless bridge's idle rule waits for that walk to end (90 frames). Any%
  play must act on the engine's own input gate instead.
- Walking down the cliff stairs moves Guybrush about 1 to 2 pixels per frame;
  on the dock floor about 6.5 pixels per frame.

## Music drivers

Unskipped intro, Esc on frame 45 (`speedrun probe --no-skips --set
music_driver=...`), time from the Esc to dock control:

| driver | music type | to dock control | note |
|---|---:|---:|---|
| adlib (the default) | 8 | 53.571 s | |
| pcspk | 1 | 52.979 s | |
| pcjr | 2 | 52.979 s | |
| cms | 4 | 56.782 s | Esc on frame 45 missed the glitch window (var 19 stayed 6) |
| null ("No Music", banned) | 0 | 44.276 s | the "Part One" card waits for its song (`room-096-part1/local-200.txt [000C]`-[0012]) |
| mt32 | - | - | needs the MT-32 ROMs, which we do not have; ScummVM then blocks on a warning dialog |

So sound waits exist, the board's ban on No Music matters, and PC speaker
music is shorter than AdLib music where the game waits for it.

## Timer-variable writers in the v4 scripts

Every write to `VAR_TIMER_NEXT` in the floppy scripts:

| script | write | note |
|---|---|---|
| `global/script-001.txt [0019]` | 0 | boot, before the logo |
| `global/script-001.txt [0741]` | 6 | boot |
| `room-010-logo/local-204.txt [0000]`, `[0097]` | 5, then 6 | the logo glitch |
| `room-051-circus-te/local-208.txt [0005]`, `[005F]` | saves it, sets **1**, restores it | a 14-frame cannon animation. User input is off then (`local-209.txt [002E]`), so a click cannot leave the room in that window. Whether save/load or anything else can end the script before `[005F]` is open. |
| `global/script-044.txt [00F9]`, `[0133]` | saves, sets 12, restores | slower; useless as a glitch |
| `global/script-097.txt [0005]`, `[014E]` | saves, sets 10, restores | slower; useless as a glitch |

## DOS CD (the record's release)

Release `dos-cd`: the English DOS CD, `MONKEY.000` md5
`2d1e891fe52df707c30185e52c50cd92` (ScummVM's EN DOS CD entry), taken from the
CD image's data track (`ENGLISH/`), with audio tracks 2 to 25 converted
losslessly to `track2.flac` ... `track25.flac` (track 2's 318-sector pregap
dropped, as a CD player starts at INDEX 01). The sound resources name real CD
track numbers (`soundcd.cpp` `getCDTrackIdFromSoundId()`), so the files keep
the CD's numbering.

- **Clock.** 1193182/4096 × 3433/4167 = 239.992 quarter-frames per second, so
  a tick is 1/59.998 s.
- **Determinism.** The CD music timer (`VAR_MUSIC_TIMER`) used to advance on
  SDL's timer thread in wall time. With the bridge, it and the speech timer
  advance once per quarter-frame of game time, at the engine's own waits; track
  files are decoded from memory. A real-time-paced run and a fast run end
  byte-identical (music timer 2377 in both after 2400 ticks), and a branch
  forked while track 17 plays matches the straight replay exactly
  (`tests/integration/test_control.py`).
- **Music check.** Every CD track play is a `cd_track` trace record with
  `"playing"`; a track whose file did not open would play as silence (the
  banned No Music timing), so a run with `"playing": false` is invalid.
- **Same game as Mac.** All 1029 objects match the Mac release (id, room,
  name), and 1679 of 1749 decompiled script files are identical
  (`data/scripts-dos-cd/`). The glitchless Part I plan replays on CD unchanged
  and reaches its goal (`speedrun run part1 --release dos-cd`, 24008 ticks).
- **Logo glitch.** Esc on frames 46 to 92 from boot leaves var 19 at 5.
- **The intro.** With skips on from the logo skip, the dock (room 33) has
  control 0.42 s after the logo Esc. The Lookout has user input on during the
  intro, but no verb bar: a click there (frame 49) cut the intro script and the
  verb bar never came back, so the executor waits for verbs on screen.
- **First stretch.** Logo skip → open the bar door → inside the bar: 13.08 s
  RTA (the floppy: 13.18 s).

## Playing the route (DOS CD, 2026-10-19)

The route files in `routes/anypercent/` play through `speedrun.route` and the
executor, seed 836557856, `talkspeed` 85 for the slide stretch. Measured:

- **RNG.** ScummVM's generator is reproduced offline in `speedrun.rng` and
  matches the engine draw for draw (control status `rng`). The first store
  entry draws the safe combination as draws k..k+3 and the storekeeper as
  draw k+4, with k = 3184 from boot for seed 1. Seeds are solved by running
  the generator backwards (`rng.rewind`), not searched. The draw count before
  the store varies by seed (High Street pirates, circus animations) over about
  55 values, so solved seeds are checked by replay with 0-4 frames of delay.
  Seed 836557856 with the store door opened 3 frames late gives 1-1-1-1 with
  the storekeeper away.
- **Insult fights** are steered by delaying the menu choice; the executor's
  `play_steered` retries a failed move with 1-60 frames of delay. Replies are
  read from Var[240] (`choose_by`), and the move before a reply requires the
  next insult to be answerable (`{"var": 240, "in": [...]}`).
- **Room-entry frames.** On a room's entry frame the stopped status has the
  new camera but the old screen offset; clicks must use `cam - 160`.
- **Grog melts** by global 68: delay(300), a 500 countdown at 1-5 per frame by
  room (High Street 5, Low Street 3, dock 2), then delay(300) to the wad.
  Transfer 1 has to happen on High Street, not in the jail.
- **The slide carry (S3).** Global 70 (Otis's cutscene) passes its first
  endCutscene and opens a second cutscene at [021A], which freezes the hover
  script: after that, exit clicks are floor walks. A compass reading's text
  replaces any line on screen, so Esc'ing a reading also ends the line global
  70 waits on. Readings started right after a floor click freeze global 70 for
  the whole walk (39 frames on High Street at `talkspeed` 85). With two readings per
  street global 70 reaches room 83 at [015A]; the handoff into Part II
  (var 352 == 2 when global 122 starts, README open problem 8) still fails.
- **Timings so far:** Carla's T-shirt 9:18 RTA (record 9:05), Part II starts
  13:44 RTA (record 13:30.5).

## Automated play (search instead of hand-written moves)

`speedrun.scriptindex` reads the decompiled scripts once (who writes which bit,
var, owner, state, class, room; through calls, room entry scripts, forward
gotos and counted loops), and `speedrun.search` reaches each milestone (the
persistent facts of a route move's `until`) by a best-first search over forks:
actions that write a missing fact, the exit graph (forest screens included),
dialogue lines, waits, Esc, and what an action needs first (its guards).
`speedrun.autoplay` uses the route's own moves as a hint and the search as the
fallback. Measured on the no-slide run (seed 836557856, `talkspeed` 255):

- From the dock to the store, the mansion, the forest and the map teleport,
  the search found the same inputs as the hand route, frame for frame; it found
  Credit Early's store delay itself, and a fight-1 encounter 0.8 s faster.
- End of Part I 13:34.0 (record 13:30.5), Part III starts 15:14.4 (record
  14:48), the monkey-head gate is open at 16:00.7.
- Not solved yet: Part III without the slide needs the normal monkey puzzle
  (five bananas given to the monkey actor in the generic jungle, room 21, whose
  clicks go to a room-local verb script, `VAR_VERB_SCRIPT = 204`), and the
  route's goal order puts the idol before the village. The search models
  object verb handlers only and keeps the route's goal order.

Both runs now play end to end (2026-10-20); the B goal lists in
`routes/anypercent/noslide/` solved the monkey puzzle with hints.

## Engine facts found playing Parts III and IV (DOS CD, 2026-10-20)

- **Readings and the automatic '.'.** A reading that the slide keeps open (the
  note at the monkey head, the dusty book) ends in about 35 frames when every
  line gets '.'. The bridge's skip switch takes `"spare": [script, ...]`: no
  automatic '.' on a line only a spared, unfrozen script waits for, and no
  automatic Esc on a spared script's override (`speedrun_skips.cpp`
  `textSpared()`). With the book (global 148) spared, the cannibal visits run
  at full speed and the book still ends after local 213's [1950]: var 53 == 2,
  the slide is back.
- **Slide counters.** Vars 53 and 352 read the same in several states (a
  reading, a carrier cutscene, 202's cutscene), so goals use facts such as
  "global 148 runs" or "ego talks" (var 3) instead.
- **Cannibal encounter.** Local 202 starts at VAR_CAMERA_POS_X >= 270. The
  book's turn stops ego, and the follow camera then centres on ego, so the book
  must open at ego x >= 266 (ego stops near 287).
- **Map rooms** show all 200 rows; room 6's sea edge (object 70) is at y 192.
- **Catacombs (room 39).** The halves are joined only by the middle markers
  543/544, whose walk-to walks to the mouse point and starts local 202's pan;
  when no cave is reachable the head points at a marker, and 209 reruns 206
  after the pan. Each chamber draws its layout and target on entry, so the
  frame of the previous exit decides the chamber: trying 0-12 frames of delay
  per exit with one chamber of lookahead (`--lookahead`) took the six exits
  from 59 s to 12 s.
- **Head dialogue at the cliff (room 70).** Walk box 8 is a line from (72,78)
  to (46,85), and (46,85) is also on box 6's edge. A walk to (46,85) ends in box
  6, so 140's 110 takes the head out and 111 then refuses to put it away in box
  8; the necklace answers "I can't put it on now". (52,83) is on box 8 only.
  The menus come while the book keeps a cutscene open, so they take a click on
  input alone.
- **Hut (room 27).** The loose board 309 has the skull 310 as its parent and is
  hit-testable only once the skull's state is 1 (picked up).
- **Map monkey (room 2).** The tiny map sprite is hit only at its feet.
- **Last Part card (room 95).** `global/script-131.txt [005C]` waits for sound
  107, CD track 6, inside `cutscene([])` with no override: 129.9 s with the CD
  audio playing. The record's card lasted about 6 s, which suggests its CD
  audio was not playing.
