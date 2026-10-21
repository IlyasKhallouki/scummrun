# Overnight log: full Any% run on DOS CD

Updated after every phase. Times are RTA from the logo-skip Esc, measured in
the engine (seed 836557856; `talkspeed` 85 for A, 255 for B). Only a from-boot
replay of one movie (`scripts/anypercent_verify.py`) counts as a reported time.

## Best verified time

| Run | RTA (from the logo-skip Esc) | Without the Last Part card | Verified |
|---|---:|---:|---|
| A + Part I, Part III (second pass) and Part IV lanes, **CD track 6 absent** (`runs/anypercent/m2n/movie.jsonl`; game dir without `track6.flac`; route diff `m2n/route-notrack6.diff`: S7#1 `delay` 800 -> 0) | **21:01.5** (1261.483 s) | 21:01.3 (card 0.2 s) | from boot, auto skips and explicit keys agree, frame 14820 (`runs/anypercent/m2n-verify/verify.json`) |
| A + Part I, Part III (second pass) and Part IV lanes (`runs/anypercent/m2/movie.jsonl`; lane merge `m2/lane-merge.diff`; driver `--early`) | **23:11.3** (1391.320 s) | 21:01.3 (card 130.0 s) | from boot, auto skips and explicit keys agree, frame 16378 (`runs/anypercent/m2-verify/verify.json`) |
| A + Part I lane + Part IV lane, **CD track 6 absent** (`runs/anypercent/n2a/movie.jsonl`; game dir without `track6.flac`; route diff `n2a/route-notrack6.diff`: S7#1 `delay` 800 -> 0) | **21:11.2** (1271.150 s) | 21:11.0 (card 0.2 s) | from boot, auto skips and explicit keys agree, frame 14936 (`runs/anypercent/verify-n2a/verify.json`) |
| A + Part I lane + Part IV lane (`runs/anypercent/i2a/movie.jsonl`) | **23:20.9** (1400.904 s) | 21:11.0 (card 129.9 s) | from boot, auto skips and explicit keys agree, frame 16493 (`runs/anypercent/verify-i2a/verify.json`) |
| A + Part IV lane (`runs/anypercent/slide-p4b/movie.jsonl`; route diff `slide-p4b/lane.diff`) | **23:34.7** (1414.738 s) | **21:24.7** (card 130.0 s) | from boot, auto skips and explicit keys agree (`runs/anypercent/verify-p4b/verify.json`) |
| A + handoff, island, ghost-ship and text-speed lanes (`runs/anypercent/slide-int1/movie.jsonl`) | 23:39.2 (1419.238 s) | 21:29.2 (card 130.0 s) | from boot, auto skips and explicit keys agree, frame 16713 (`runs/anypercent/verify-int1/verify.json`) |
| A + delay lookahead on the fights and Carla (`runs/anypercent/slide-c5/movie.jsonl`) | 24:08.6 (1448.572 s) | 21:58.6 (card 130.0 s) | from boot, auto skips and explicit keys agree, frame 17065 (`runs/anypercent/verify-a5/verify.json`) |
| A + Part II hints fixed (fire, cooking scrolls, book) (`runs/anypercent/slide-c3/movie.jsonl`) | 24:13.1 (1453.072 s) | 22:03.2 (card 129.9 s) | from boot, auto skips and explicit keys agree, frame 17119 (`runs/anypercent/verify-a4/verify.json`) |
| A + cannon fixed (Part II) (`runs/anypercent/slide-c2/movie.jsonl`) | 24:14.0 (1453.989 s) | 22:04.1 (card 129.9 s) | from boot, auto skips and explicit keys agree, frame 17130 (`runs/anypercent/verify-a3/verify.json`) |
| A + catacomb delay lookahead (`runs/anypercent/slide-c1/movie.jsonl`) | 24:35.5 (1475.490 s) | 22:25.6 (card 129.9 s) | from boot, auto skips and explicit keys agree, frame 17388 (`runs/anypercent/verify-a2/verify.json`) |
| A, item slide, talkspeed 85 (`runs/anypercent/slide-b18/movie.jsonl`) | 25:24.4 (1524.408 s) | 23:14.5 (card 129.9 s) | from boot, auto skips and explicit keys agree, frame 17975 (`runs/anypercent/verify-a1/verify.json`) |
| B, no slide (`runs/anypercent/noslide-p7/movie.jsonl`) | 28:01.4 (1681.413 s) | 25:51.5 (card 129.9 s) | from boot, auto skips and explicit keys agree (`runs/anypercent/verify-b1/verify.json`) |

Record: 21:52.4 (saruya). With CD track 6 absent from the game dir, 21:01.5 beats it by 50.9 s on the full clock. The card waits for sound 107, CD track 6 (`global/script-131.txt [0059]`-[0060], no override): 129.9 s when the track plays, 0.2 s when it is missing (the bridge traces `cd_track` 6 with `playing: false`). The record's card lasted about 6 s. The leaderboard bans the "No Music" device; our music driver is unchanged (AdLib) and every other CD track plays, so whether a missing track 6 counts is an open rules question. With the track, the best is 23:11.3. S7#1's 800-frame delay only fitted the card into the driver's hint window; without the card it was 66.7 s of idle time and is set to 0 in the no-track-6 route.

## Phases

| Phase | Goal | Status |
|---|---|---|
| B | No item slide, end to end: goal lists for Parts III-IV (`routes/anypercent/noslide/`), search for the inputs | done: 28:01.4 (`runs/anypercent/noslide-p7`) |
| B verify | Replay the B movie from boot; explicit-key replay with auto-skips off | done, both 1681.413 s |
| A | The record's item slide carried into Part II (Otis's cutscene handoff), then S4-S7 with the slide | done and verified (first: 25:24.4 / 23:14.5 without the card) |
| Improve | Hints that fell back to search fixed (cannon, fire, cooking, book, Stan); delay lookahead on random draws (catacombs, fights, Carla); `--mode always` (twice) and lookahead before every slow goal (24 s slower: it reshuffled the catacomb chambers) tried and dropped | best verified 24:08.6 / 21:58.6 without the card |

## A (item slide) against the record

`runs/anypercent/slide-c5` (talkspeed 85; verification in `runs/anypercent/verify-a5`).

| Point | A (24:08.6 run) | Record | A - record |
|---|---:|---:|---:|
| Low Street archway (Street split, ~) | 2:39.0 | 2:31.0 | +8.0 |
| Troll bridge reached (Bridge split) | 5:17.7 | 5:17.0 | +0.7 |
| Sword Master's clearing left (T-shirts) | 9:08.9 | 9:05.0 | +3.9 |
| SCUMM Bar (crew phase) | 12:06.0 | 12:05.0 | +1.0 |
| Second jail visit (slide held) | 13:10.2 | 13:05.0 | +5.2 |
| Part II starts | 13:43.2 | 13:30.5 | +12.7 |
| Part III starts | 14:52.3 | 14:48.0 | +4.3 |
| Wimpy idol | 15:49.4 | 15:52.0 | -2.6 |
| Necklace | 18:35.0 | 18:29.0 | +6.0 |
| Last Part card starts | 19:58.0 | 19:45.5 | +12.5 |
| Final fade | 24:08.6 | 21:52.4 | +136.2 (card 130.0 s here, ~6 s in the record) |

Without the card A ended near 23:14; the gap was mostly the catacombs (the head points at the middle
markers in two chambers, 13-18 s each). Delay lookahead on the catacomb exits (`--lookahead 12`, the
chambers draw their layout on entry) finished them 47 s sooner: necklace 18:58.6 (record 18:29),
22:25.6 without the card. Part II's cannon then played from its hints (it had fallen back to the search:
the fuse reach's 'after' waited for a var that stays 0, and the nozzle click re-clicked the gunpowder):
Part III starts 15:00.5, 24:14.0 / 22:04.1 without the card. Lookahead on the fights and Carla's
insults: 24:08.6 / 21:58.6. With the record's ~6 s card instead of ours, A is about 12 s behind:
the end of Part I is pinned to Otis's cutscene (30.58 s from the grog on the lock to Part II in
every run), and the rest is spread over Monkey Island and the ghost ship (the crate waits for a CD
sound, like the card).

## B (no slide) splits, early chained resumes

| Point | B | Record (saruya, 21:52.4) |
|---|---:|---:|
| Carla's T-shirt (end of S2) | 9:18.0 | 9:05 |
| End of Part I (room 83) | 13:34.0 | 13:30.5 |
| Part II starts | 13:36.8 | 13:30.5 |
| Part III starts | 14:52.6 (was 15:14.4; flaming-mass goal added to S4#45) | 14:48 |

Slide run (A), talkspeed 85: Part II starts 13:47.7 with the slide carried; Part III
starts 15:10.7 (`runs/anypercent/slide-a5`). Part III with the slide (`slide-b4`):

| Point | A (slide) |
|---|---:|
| Spyglass | 15:27.7 |
| Dam blast, oars skip done | 15:45.1 |
| Monkey skip: idol field | 16:06.3 |
| Book regain: encounters done, slide back (var 53 == 2) | ~17:05 |
| Herman's key | 17:24.3 |
| Idol field again (S5 end, slide held) | 17:57 |
| Catacombs done (room 70) | 19:29.5 |
| Head dialogue done (book kept open) | 19:40.6 |

Fixes that got Part III's slide tricks working:
- Fort shield dropped: the plain exit right after the spyglass wins Herman's race by 7 frames and keeps the slide.
- Readings (monkey skip, books) need the bridge's automatic '.' off for their own lines, or they end in ~35 frames.
  New bridge option: skips `"spare": [148]` keeps a script's own lines and override from the automatic '.' and Esc.
- Slide counters (vars 53/352) are transient and ambiguous; reading goals now use "global 148 running" or "ego talking".
- Book at the cannibals: opened at ego x >= 266 so the camera still reaches 270 (local 202's trigger) after the turn stops ego.
- A milestone counts as already reached only if its move's whole `until` holds (transient parts included).
- Executor: map rooms use all 200 rows (sea-edge exits at y 192); approach clicks avoid other objects' pixels;
  a stepping stone (a visible object nearer the goal, tried on a fork) when no floor path leads on (room 39's
  middle markers); walks to objects re-click after a cutscene or pan stops them, re-reading `obj_from_var`;
  two inventory items in different rows scroll between the clicks; actors on the map are hit-tested at the feet;
  an actor that is not on screen is waited for, not approached.
- Head dialogue: the book keeps a cutscene open, so the choices need gate "input" (was 136 s lost).

## Phase A: the slide handoff into Part II (solved)

Room 83's local 200 must reach `[0C74] startScript(122)` while Otis's global 70
is still inside its first cutscene (before `[0211]`). Lines play one at a time:
never skip Otis's (actor 4) lines, '.' every other line at once, Esc an armed
override while input is off, answer the crew menus inside global 70's cutscene.
At talkspeed 85 global 122 then starts with var 352 == 2 and global 70 at
`[01EC]` (Part II at 13:48.0, frame 9673). Encoded as press rules on S3#119-123.

## Tools

- `scripts/anypercent_auto.py`: goal-by-goal player (route hints, search fallback), resumable.
  `--mode always` lets the search try to beat every hint; when a search branch then dead-ends at the
  next goal (a map exit that lands the boat at sea), the driver goes back to the hint's branch.
  `--lookahead N` tries 0..N frames of delay before goals whose next room is drawn at random (the
  catacomb chambers) and keeps the delay that finishes the next goal first. In slide mode a search
  branch must leave the slide counters (vars 53/352) as the hint's branch does.
- `scripts/anypercent_verify.py`: from-boot replay and timing; explicit-key replay.
- `src/speedrun/scriptindex.py`, `src/speedrun/search.py`, `src/speedrun/autoplay.py`.

## Open questions for the user

- The "Last Part" card plays CD track 6 in full (129.9 s here); the record's card lasted about 6 s,
  so its CD audio was probably not playing (other CD waits, such as the crate fanfare on the ghost
  ship, would then be shorter too). Times are reported with and without the card. Running without
  the CD audio tracks would need a fresh run from boot, since every later input is frame-timed.
