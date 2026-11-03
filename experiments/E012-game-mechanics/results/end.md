# E012 component `end`: the game's end reported, and the walk boxes

Branch `e012-end` (worktree `/var/tmp/fast-mi/wt-e012-end`), from `e012` c86d89a. plan.md: the row `end` of
"Components" and the amendment "component `end`" (written before the code). Laptop, seed 1, nice 10.

## What was built

- **Patch 0005** (`patches/0005-scumm-speedrun-end.patch`, 275 lines, sha1 9eb9e2b2), on top of 0001-0004. The build
  is `/var/tmp/fast-mi/svm-e012-end` (a copy of round 2's tree `svm-e011-r2int` with 0004 committed and 0005 applied;
  `end/build.sh`); its scummvm has sha1 cc6615ea.
  - `SPEEDRUN_REPORT_RESTART=1`: the first statement of `restart()` and of `pauseGame()` ends the run through the
    bridge's `finish()`, as "restart" or "pause". The end record holds the room and counters from before either. Then
    `restart()` returns at once and no pause dialog opens. Off, the default, both run as before.
  - `SPEEDRUN_TEST_RESTART=<frame>`, tests only: `restart()` at the start of that frame, where the restart key calls
    it.
  - Boxes (0x94, reply 0x12) and BoxesJson (0x95, reply Json) on the binary channel, and `{"cmd": "boxes"}` on the
    JSON one. Each gives the current room and, per walk box, `getBoxCoordinates()` and `getBoxFlags()`. They only read.
- **fmi-core** (the amendment's interface, byte for byte): `GAME_ENDS`, `is_game_end`, `WalkBox`, `Boxes`, and a
  provided `Engine::boxes`.
- **fmi-engine.**
  - `BinEngine::boxes` and `boxes_json`.
  - `SPEEDRUN_REPORT_RESTART=1` in `fmi`'s environment reaches every engine (`report_restart`). `FMI_BOXES`'s reader
    is `boxes_on`. Both switches are off by default.
  - The archive module: entries with their whole chains from the root.
  - The checks `fmi-engine-end` (hooks, boxes, unreach) and `tests/end.rs`.
- **Scripts:** `end/check.sh` runs the gates; `end/scummvm-report.sh` turns the switch on for E011's gate scripts.

## Gates

| gate | switch off | switch on |
|---|---|---|
| G2, the door chain (E011 `check-r2.sh g2`) | 12 lean variants and fmi-engine with all six: 37 dumps each against svm-w3b, 0 differ; state:316 = 1 | the same, 0 differ |
| G3, the kitchen chain | the same 13 runs, 40 dumps each, 0 differ; owner:566 = 1 | the same, 0 differ |
| G4, 400 frames from 2728, one Step each | 0 differences (399 facts followed by delta) | 0 differences |
| G1, fork_exact.py, 7 load children | C and P against F: 6,002 comparisons, 0 differ; 48,204 load children, 86,893 comparisons, 0 differ; P's trace = F's; the control differs from frame 236 | C and P against F: 6,002, 0 differ; 440,570 load children, 478,565 comparisons, 0 differ; P's trace = F's; the control differs from 236 |
| the hooks unreachable on every recorded chain | 1,122 entries in 20 archives verified from boot (16,811 checkpoints): 1,122 the same | the same 1,122, and no run ended as "restart" or "pause" |
| Boxes against ScummVM's box data | in 6 rooms (28, 29, 33, 34, 35, 38; 3 asked): the binary reply equals the JSON reply, and both have the BOXD block's count and corners. Flags differ only at room 28's box 11, which the room's entry script sets to 128 (`o5_matrixOps`) | |

G1's walk took 594 s off and 2,280 s on. It made 48,204 and 440,570 load children (fork median 6.1 and 3.9 ms; load
average median 7.8 and 5.9). The load children are the test's own load, and the engine runs the same code per frame
either way: the hooks run only inside `restart()` and `pauseGame()`, which no run reached.

In "on", `SPEEDRUN_REPORT_RESTART=1` is in every engine's environment, through the reference's wrapper and through
fmi-engine. The recorded chains are:

- E008: kitchen 372, smoke 16, door 6;
- E008's check: kitchen 20, smoke 16, door 6;
- E009: door 6, laptop-verify door 6, the off check's job (test (a)) 8;
- E011: the kitchen check 226, B0 44, round 1's four B3 sequences 4 × 44, round 2's five B3 sequences 5 × 44.

Each entry is played from boot with its chain's movie (the root's segment first) and compared at every checkpoint of
its chain: 1,122 of 1,122 are the same, both ways.

**The hooks themselves** (`fmi-engine-end hooks`, at the kitchen check's entries 0, room 33, frame 2728, and 203, room
28, frame 3393; each case from boot):

| case | entry 0 | entry 203 |
|---|---|---|
| on, no trigger, at F + 100 | = off: frame, tick, rng, room and 3,653 facts, in the engine and in a fork | = off (3,666 facts) |
| on, `restart()` at F + 5 | End "restart" at 2733, room 33, in the engine and in a fork | End "restart" at 3398, room 28 |
| off, `restart()` at F + 5 | runs on to F + 100, in room 10 (the boot ran again) | the same |
| on, the pause key at F + 1 | End "pause" at 2729, room 33, in the engine and in a fork | End "pause" at 3394, room 28 |
| off, the pause key at F + 1 | no reply within 10 s (the pause dialog waits for an event) | the same |

The cargo tests of fmi-engine pass: 7 unit tests, 6 end tests (2 of them on the engine), and the 3 engine tests.
`cargo check --workspace --all-targets` passes with the new trait method.

## Measured

- **Boxes:**
  - One Boxes round trip takes 18.2 µs (median over 6 × 200 calls; 12-28 µs per room). The JSON text is 0.5-1.7 kB.
  - Boxes per room, with box 0 the room's dummy at (-32000, -32000) in all six: 33: 14; 38: 6; 35: 24 (2 locked or
    invisible); 29: 10; 34: 15; 28: 12 (box 11 invisible). So a walk click per real, open box is 5-21 trials per room.
- **This game's scripts** (statistics only):
  - SO_RESTART: 9 sites, all in the global script `VAR_POST_SAVELOAD_SCRIPT` names. It runs after the original save
    menu, which these runs never open.
  - SO_QUIT: 2 sites, in two global scripts of one room.
  - SO_PAUSE: 0 sites.
  - The engine's own restart paths (the F8 key, the GUI) take no input the bridge can inject. The pause key (space)
    does, and the recorded movies hold the keys "o", "p" and "t" only.
- **Cost when off:** none measured. G1-G4 and the 1,122 verifications are equal with the switch off and on.

## Deviations from plan.md

These are given in the dated amendment, before the code:

- The pause hook is at `pauseGame()`, which SO_PAUSE calls, not at SO_PAUSE alone. It also catches the engine's
  pause key, which otherwise blocks a headless run.
- The restart test trigger was added.
- BoxesJson was added, so that the binary reply can be compared with the JSON one in one engine.
- The plan's research counted 1 quit site; this count found 2.

`restart()` is proven dominated (plan.md (5)). A pause is not: a script's continuation after SO_PAUSE would run once
the dialog closed. In this game SO_PAUSE has no site, so "pause" can come only from the pause key, and the key changes
no game state.
