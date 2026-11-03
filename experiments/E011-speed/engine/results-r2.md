# E011 engine, round 2: the count-only mixer and no pixel output (patch 0003)

What was built:

- **Patch 0003** (`patches/0003-scumm-speedrun-cuts.patch`, sha1 757eceee), on top of 0001 and 0002. It adds two bridge
  settings, off by default:
  - `SPEEDRUN_COUNT_MIXER`: the rate converter keeps every read and count and writes no sample;
  - `SPEEDRUN_NO_PIXELS`: no costume or strip decoding, and no copies of finished pixels.

  `SPEEDRUN_CUTS=1` turns on both. With no pixels, the exact dump is pixel-free (`screens_np`). The argument that
  neither cut reaches the game state is `CUTS.md`, written before the code.
- **The build**: `/var/tmp/fast-mi/svm-e011-r2`, a copy of round 1's tree with 0002 committed and 0003 applied, built
  by plain make (`build.sh --tree`). The gates ran on scummvm sha1 d8d0b63a; the build was remade as 986883ab after a
  comment-only change (see Deviations).
  - `git apply` of 0001, 0002 and 0003 on the tag's tree reproduces its sources exactly (`diff -r`: no difference).
  - svm-w3b and svm-e011 were not touched, and no tree was reset.
- **fmi-engine**: `FMI_ENGINE_CUTS=1` gives every engine `SPEEDRUN_CUTS=1`. `fmi-engine-frames` is a fixed frame
  workload for `perf stat`.
- **Scripts**: `check-r2.sh` (the gates), `play-r2.sh` (play's fixture tests), `bench-r2.sh` with `bench_r2.py` (the
  bench), `compare_np.py` (the pixel-free comparisons) and `perf-engine.sh`.

All runs: the laptop, seed 1, nice 10, 2026-11-02/03. Other round-2 builders kept the load average at 6 to 21, so wall
and CPU times are inflated. Instruction counts do not depend on the load; cycle counts depend on it a little (SMT).

## Gates (patch 0003's build, all six switches unless said)

| gate | check | result |
|---|---|---|
| G1 | E003 `fork_exact.py run` with all six switches (`svm-lean.sh LEAN=all6`): 3,000 frames from 235, 7 load children, pixel-free exact dumps | 6,002 C and P comparisons, 0 differ; 15,892 load children, 48,275 comparisons, 0 differ; P's trace equals F's; the control (one RNG bit flipped) differs from frame 236 |
| G2 | the door chain (entry 5, frame 3129), 37 exact dumps against svm-w3b's, through the Python reference: every switch off; each of the six alone; the four; the four with the mixer; the four with pixels; all six; then fmi-engine over the binary channel with all six | 0 differences in every run. Runs without pixels are compared whole. Runs with pixels are compared on everything but the screen digest, and on `screens_np` against the build with every switch off and the pixel-free dump (whose other fields equal svm-w3b's: 0 differences). state:316 = 1 at 3129 |
| G3 | the kitchen chain (E009 off check, entry 7, frame 3216), 40 dumps, the same runs | 0 differences in every run; owner:566 = 1 at 3216 |
| G4 | fmi-engine walk, 400 frames from 2728 on the kitchen movie, one Step each, with all six: the Step's block equals a State block, its facts `facts_of(Obs)`, the binary Obs the JSON's; a second engine by Step alone ends equal | 0 differences (1,200 runs, 17,600 explain entries, 399 frames followed by delta) |
| G5 | E009's step test (a) through the Python reference with all six (JSON protocol) | `CHECK (a): 0 goal True 2728 3216 488 11712 2 36 501 8` (501 children); verify from boot 0 differences at 9 checkpoints, owner:566 = 1 |
| verify | every new entry of B3's four sequences (a-fast 55, a-ref 43, b-fast 55, b-ref 43) verified from boot in a fresh engine with all six (fmi-engine verify on bench/mkverify.py's archives) | 196 entries, 0 differences |
| play | play's fixture tests 2-4 through fmi-engine with all six (`play-r2.sh`) | records equal. Test 2: 501 of 501 children equal. Test 3 (strict): the 3 plans with 0 differences, every window record equal to a recorded Python run's. Test 4 (ready): the same four children, with 1, 23, 296 and 1 tries |

**Which converters the game uses.** A diagnostic copy of the build (not kept) logged every rate converter made. The
door chain from boot makes:

- the AdLib chip's: 22,050 Hz mono, the copying case;
- the CD track's: 44,100 Hz stereo, the integer-ratio case, five times;
- one 11,025 Hz mono stream: the interpolating case.

So all three count-only paths run in G2's door chain, the mixer alone and with the others. The idle frames after the
root make only the first two.

## Bench: before and after, back to back (`bench-r2.sh 3`, 23:39-23:41, load average 12.8 to 16.8)

- **Before**: round 1's build (svm-e011), with the four lean switches.
- **After**: patch 0003's build, with all six.

Both go through the same fmi-engine binary channel, interleaved round by round. Medians of 3 rounds are given, with
the range in brackets where it matters.

Per-frame user cycles and instructions are fmi-engine-frames' counts: 3,000 frames from the root (frame 2728, no
input) less the boot alone, under `perf stat` of the engine. "At 3.6 GHz" converts cycles to time at the laptop's turbo
clock, as round 1's results.md did. Under this load the engine ran at about 1.3-1.8 GHz.

| measure | before | after | after/before |
|---|---:|---:|---:|
| frame in 50-frame commands: user instructions | 292.7 k | 57.7 k | 0.20 |
| frame in 50-frame commands: user cycles | 176.7 k (171-181) | 50.5 k (46-51) | 0.29 |
| the same at 3.6 GHz | 49 µs | 14.0 µs | |
| frame in 50-frame commands: engine CPU, measured | 93.3 µs (89.8-93.6) | 27.8 µs (27.3-29.6) | 0.30 |
| the same, `fmi-engine bench` (later frames) | 95.0 µs | 28.7 µs (25.6-31.3) | 0.30 |
| observed frame (Step + facts): user instructions | 329.2 k | 94.1 k | 0.29 |
| observed frame: user cycles | 213.7 k | 97.0 k (78-126) | 0.45 |
| observed frame: engine CPU, measured | 146 µs | 73 µs (68-88) | 0.50 |
| observed frame: driver CPU | 42 µs | 36 µs | 0.85 |
| observed frame: wall (fmi-engine-frames / bench) | 342 / 449 µs | 206 / 181 µs (104-404) | 0.60 / 0.40 |
| bare frame, one command: engine CPU (bench) | 125 µs | 77 µs | 0.62 |
| boot to the root (start and 2,728 frames): user instructions | 1,700 M | 310 M | 0.18 |
| the start alone (to the first stop; `fmi-engine-frames start` under perf-engine.sh, once each, outside the rounds) | 126.8 M | 126.7 M | 1.00 |
| door chain from boot (verify, entry 5): user instructions | 1,955 M | 369 M | 0.19 |
| door chain from boot: user cycles | 1,355 M | 335 M (331-358) | 0.25 |
| the same at 3.6 GHz | 0.38 s | 0.093 s | |
| door chain from boot: engine task-clock | 795 ms | 250 ms (247-266) | 0.31 |
| door chain from boot: wall (`fmi-engine bench`, median of 3 per round) | 1,525 ms | 370 ms (307-1,021) | 0.24 |
| start to the first stop: wall (bench) | 303 ms | 209 ms | 0.69 |
| fork + quit: parent CPU | 1.30 ms | 1.25 ms | 0.96 |
| return by fork of a kept entry: wall | 4.0 ms | 3.3 ms | 0.83 |

**Neither cut touches the start, fork or return.**

- The start's instructions are the same: 126.8 M before, 126.7 M after.
- Fork and return differ by the load only.
- Round 1's figures for them (0.66-0.77 ms fork + quit, 0.55-0.84 ms return) were measured at a lower load.

**Each cut alone, on the door chain from boot** (perf stat, user instructions):

| switches | instructions |
|---|---:|
| the four | 1,955 M |
| the four and the mixer | 1,225 M (count-only, batched; 1,419 M with the first, per-sample count) |
| the four and pixels | 1,099 M |
| all six | 369 M |

## Against the targets

**A frame inside a command: at most 20 µs.**

- At the turbo clock it is met: 50.5 k user cycles, 14.0 µs.
- As measured under this night's load it is not met: 25.6-31.3 µs of engine CPU, against 89.8-96.2 µs before at the
  same load.

Round 1 measured 36-52 µs at its own load.

**The door chain from boot: at most 0.15 s.**

- The engine's user cycles come to 0.093 s at the turbo clock.
- Its task-clock is 247-266 ms, with the kernel's share and the slow clock of a loaded machine.
- Its wall time is 307-1,021 ms, median 370 ms.
- So the target is not met as measured.

A third of the chain's instructions are now the start (127 M of 369 M), which neither cut touches. A flat
`perf record` of the start alone splits its samples so:

- the GUI theme's XML parser (`XMLParser`): 10%;
- case-insensitive string hashing and comparison (`tolower`, `hashit_lower`, `scumm_stricmp`, the config and theme
  maps): 11%;
- string building: 5%;
- font scaling (`BdfFont::scaleFont`): 5%;
- dynamic linking (ld.so): 9%;
- the kernel: 8%.

The GUI is never shown in a run.

**An observed frame: at most 0.05 ms.** It is not met. The engine's part is 97 k user cycles (27 µs at the turbo clock),
73 µs of CPU as measured. On top of that come:

- the kernel's socket and wake-up work;
- 34-41 µs of fmi-engine's own CPU;
- two context switches.

The wall time is 104-404 µs under this load.

A Step costs 36 k instructions over a frame in a command. Most of that is the F2 block and its delta (`wireBlock`,
`wireBlockOrDelta`: 7% of a Step's cycles) and the explain rows.

## How close to the floor

**The floor.** The profile measured the game logic at 32 k cycles a frame, about 15 µs (door chain, profile-engine.md
section 1). It priced a frame with both cuts at 60-80 k cycles.

**What was measured.**

- On the door chain, less the start, a frame now takes about 69 k user cycles on average ((335 M − 120 M) / 3,129).
  That is inside the profile's range, and includes the boot's room loads.
- On the idle frames after the root, a frame takes 50.5 k cycles, 1.6 times the logic floor.

**What is left in such a frame.** These are shares of a `perf record` of 20,000 frames after the root, in which the
boot is about a fifth of the samples:

- SCUMM's own logic and bookkeeping: `resetActorBgs` 6%, `updateObjectStates` 2%, the gfx usage bits, the scripts;
- the music's timers and the silent streams: iMUSE's and the AdLib driver's timers, the chip's and the CD stream's
  zero-filled reads, the converters' counting, about 18%;
- the bridge's per-frame idle rule (`evaluateIdle`, `anyVisibleVerb`, `findVerbAtPos`, `findObject`), about 7%;
- uncontended locks (mixer, iMUSE, SDL), about 6%;
- the cursor image, rebuilt and scaled for the backend each frame, about 4%;
- clock reads, about 2%;
- debug calls whose channels are off, about 2%.

Of these, only the cursor image is output in the sense of this round's cuts. It is left out because
`CursorMan.replaceCursor` also keeps the cursor stack that `showMouse` and `isVisible` read. Cutting it needs its own
argument.

## What is left for a later round (named with its number, none built)

1. **The start: 127 M instructions, a third of a replay from boot.** Its largest part is the GUI theme and fonts loaded
   for a GUI that never shows. The engine's fork-based return makes the start matter only for fresh engines: verify,
   the pool's root, and B3's cold regime.
2. **The observed frame's own work.** The Step adds 36 k instructions to a frame and fmi-engine's side takes 34-41 µs of
   CPU. Both ends of the F2 delta could take less. The round trip itself is the FFI or shared-memory item of plan.md.
3. **The cursor image per frame** (about 4%), after an argument about CursorMan's state.

## Deviations

- **Patch 0003, not 0002.** The cuts are a new patch on top of 0002, so round 1's build and patch stay as they were.
  The engine change of another round-2 component would take 0004 (COORD.md).
- **`SPEEDRUN_LEAN=1` still means the four switches.** The cuts are turned on by `SPEEDRUN_CUTS=1` (in fmi:
  `FMI_ENGINE_CUTS=1` with `FMI_ENGINE_BIN` set to the round-2 build). Every gate ran with all six.
- **The no-pixels cut skips the PC path's costume decoder only.** The C64, Amiga and PC Engine decoders are other
  games' paths. Text, verbs, the cursor and the transition effects still draw.
- **`fmi-engine verify` of the kitchen chain** (`kitchen-arch` entry 7) stops at "frame 2776 is not after the current
  frame 2776" on round 1's build exactly as on this one, so it is a limit of the tool on that archive, not of either
  build. The kitchen chain is checked by G3's dumps and G5's verify instead. The B3 entries verify with the tool.
- **The gated binary** had sha1 d8d0b63a. The build was then remade after a comment-only change to patch 0003 (sha1
  757eceee). The new binary (986883ab) has the same `.text` and `.data`; its `.rodata` differs only in the build date.
- **The targets in wall time were measured on a loaded laptop** (load 13-17 from the other round-2 builders' readers and
  compilers). The cycle and instruction counts are the stable comparison.
