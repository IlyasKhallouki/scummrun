# scummrun

scummrun plays SCUMM adventure games by searching the running game. It drives a patched ScummVM, tries what the
game's interface offers, keeps the moments that show something new, and proves every result by replaying its
recorded clicks from boot. No language model runs in the system, and no move is written by hand.

## Results

### The Secret of Monkey Island, from boot to the ending (15 November 2026)

One unattended run of the search played the DOS CD release from the first frame to the end of the game: one
binary, 5.49 hours on a cloud machine with 8 threads, no human input during the run. The ending replays from boot
in a fresh engine in a few seconds: 1,994 clicks, frame 114,978.

What the run was given: the game's files; a decompilation of its scripts, made from those files, with a list derived
from it of the variables the scripts test; the variable that marks the game's end (bit 565, picked by hand, used only
to stop the run); and general rules for adventure-game mechanics, developed while working on this game. The search's
code holds no room, object or variable number of the game (the engine patches keep a few Monkey Island values from
earlier work, switched off for these runs). The checkpoint files used to follow a run's progress are read only to
report it.

- Results, the exact settings, and the whole input movie: [`experiments/E027-direct/results/full5`](experiments/E027-direct/results/full5).
- The lab notebook of the search: [`notes/log/2026-11-10-e027-direct.md`](notes/log/2026-11-10-e027-direct.md).

### Indiana Jones and the Fate of Atlantis, the playable demo (9 November 2026)

[![Indiana Jones and the Fate of Atlantis demo, 3:51](https://img.youtube.com/vi/npbKSDtlLX8/maxresdefault.jpg)](https://youtu.be/npbKSDtlLX8)

The earlier version of the system found a route from boot to the end of the demo (experiment E022), then shortened
its own route (E024) to 92 clicks: 3:46.8 of game time on seed 1, 3:51.5 in real time in the video. The route is
verified from boot, and no click in it was written by hand. [Watch it on YouTube.](https://youtu.be/npbKSDtlLX8)
Plan and results: [`experiments/E024-shorten/plan.md`](experiments/E024-shorten/plan.md).

### Before that: a hand-routed Any% run of Monkey Island (October 2026)

The project began with a bot that replays a route of 532 moves written by hand after the human record's route:
21:01.483 on the DOS CD release with CD track 6 removed, 23:11.3 with it ([video](https://youtu.be/lHQrHf5b_5o)). It is
a baseline, not a result of the search. It is described under "Earlier work" below.

## How the search works

1. The game runs inside ScummVM, patched so a program can drive it (the bridge, `patches/`): it steps the game frame
   by frame on a virtual clock, sends clicks at exact frames, reads the game's state, and copies the running game at
   any moment.
2. The search keeps a set of stops: moments where the game waits for the player.
3. At each stop it tries what the screen offers (waiting, every verb with every object, walking), each try in its own
   copy of the game, and keeps the results that show something new as new stops.
4. Progress is measured mainly by the objects the player has ever held. General rules decide which stop is tried
   next, and send the search to the rooms whose scripts mention what it has just gained.
5. A run goes in legs: when one stops gaining, the next starts from its best stop.
6. Every result replays from boot with its recorded clicks, and every change to the code must leave a fixed set of
   reference stops exactly as they were before it is used for a run.

## Code and notes

- The search: [`experiments/E011-speed/rs/fmi-direct`](experiments/E011-speed/rs/fmi-direct) (Rust; the host that drives
  the engine is `fmi-engine`, the shared types `fmi-core`).
- The engine patches, "the bridge": [`patches/`](patches) (0001 to 0013, applied to ScummVM v2026.3.0).
- The run and check scripts: [`experiments/E027-direct/run`](experiments/E027-direct/run) (`engine.sh`, `gate.sh`,
  `chain.sh`, `keep.py`, `bench.sh`, `env.sh`).
- The lab notes: [`notes/log`](notes/log); every experiment's plan and results: [`experiments/`](experiments).
- The survey of related work: [`docs/related-work.md`](docs/related-work.md); the paper's working files: `paper/`.

## Reproducing the results

Everything runs on Linux. You need git, a C++ compiler with make, CMake, the development files of
SDL2, libFLAC and zlib (libFLAC plays the CD audio; without it the game's timing differs and the build stops), Python 3,
[uv](https://docs.astral.sh/uv/) (the decompiling step runs its Python with it), and Rust with cargo.

**1. Get the code and the game files.**

```sh
git clone --recurse-submodules --shallow-submodules https://github.com/IlyasKhallouki/scummrun.git
cd scummrun
```

The game files are never in the repository; bring your own:

- *The Secret of Monkey Island*, DOS CD release: `MONKEY.000`, `MONKEY.001` and the CD audio as `track2.flac` to
  `track25.flac`, in `game/dos-cd/`. Their checksums are in
  [`experiments/E027-direct/results/full5/settings.md`](experiments/E027-direct/results/full5/settings.md).
- For the gate only: the playable DOS demo of *Indiana Jones and the Fate of Atlantis* (free on
  [ScummVM's demo page](https://www.scummvm.org/demos/); the one whose `PLAYFATE.000` has MD5
  `035deab53b47bc43abc763560d0f8d4b`), in `game/fate-play-demo/`.

**2. Decompile the scripts** (the search reads which rooms' scripts mention what; nothing of them is committed):

```sh
GAME_DIR=game/dos-cd scripts/dump-scripts.sh
GAME_DIR=game/fate-play-demo SCUMMRP_GAME=atlantis INDEX_NAME=PLAYFATE.000 RELEASE=fate-play-demo \
  DUMP_ALLOW_PROBLEMS=1 scripts/dump-scripts.sh
```

The demo's dump lists `PROBLEM: RNAM room N has no LFLF` for the rooms its files do not hold (it has 11 of the game's);
`DUMP_ALLOW_PROBLEMS=1` lets it finish. Both end with a count of files and `"files_with_errors": 0`.

**3. Build the engine and the search.**

```sh
experiments/E027-direct/run/engine.sh                       # ScummVM v2026.3.0 + patches/ -> build/svm-direct
(cd experiments/E011-speed/rs && cargo build --profile fast -p fmi-direct)
```

**4. Run the gate** (5 to 10 minutes). It expands fixed stops of both games one at a time, so its result does not
depend on the machine, and compares it with the runs' digests. `PASS` means the search, the engine, your game files and
the decompiled scripts behave exactly as in the runs.

```sh
experiments/E027-direct/run/gate.sh
```

**5. Replay the ending from boot** (seconds):

```sh
source experiments/E027-direct/run/env.sh && mkdir -p out/verify-full5
env $ON $MI experiments/E011-speed/rs/target/fast/direct verify \
  experiments/E027-direct/results/full5/end-stop.jsonl 7895 out/verify-full5 bit:565=1
```

It prints `"frame":114978`, `"clicks":1994` and `"ok":true`.

**6. Run the search from boot.** The run of 15 November 2026 took 5.5 hours on 8 threads of a cloud machine; a
four-core laptop needs about 14. `keep.py` starts a new chain from the furthest stop if one ends without the game's end.

```sh
experiments/E027-direct/run/chain.sh out/full --hours 12 --stall 30 --workers 8 &
sleep 20; KEEP_STALL=30 KEEP_WORKERS=8 python3 experiments/E027-direct/run/keep.py out/full \
  experiments/E011-speed/rs/target/fast/direct
```

Each leg writes its exact command to `out/full/legNN/cmd.txt`, its progress to `err.txt`, and its stops to
`nodes.jsonl`; `chain.txt` logs the checkpoints as they are first reached. The leg's thread timing decides where a leg
ends, so a rerun may take another route to the end; `direct verify` checks any end stop it finds the same way as above.

## Research

A paper on the search is in preparation. `notes/` on branch `direct` holds the decisions, the open questions and the
lab notebook; `experiments/` holds each experiment's plan (question, prediction, stop rule) and results.

## Earlier work: the planner and the Any% bot (October 2026)

Everything below describes the system before the search: the Any% bot of the video above and the glitchless Part I
planner. It is kept as it was. Its instructions predate patches 0006 to 0013; to rebuild it as it was, check out commit
`6617720`, the last before the search was merged.

### Any%

The route follows saruya's record and uses the same glitches: the logo speed glitch, Credit
Early, the item slide carried into Part II, the oars, monkey and feather skips, and getting
the slide back with the dusty book. It is written as 532 moves in `routes/anypercent/`, and
all but one cite the scripts they rely on.

The driver (`scripts/anypercent_auto.py`) treats each move as a hint, not a script. For every
goal it plays the hint on a fork of the running engine. If the hint fails, it falls back to a
best-first search over forks, where the candidate actions come from a parsed index of the
decompiled scripts that says which action writes which fact (`src/speedrun/scriptindex.py`).
Where the next room or fight is drawn at random (the catacombs, the insult fights, Carla), it
tries 0 to 12 frames of delay first and keeps the one that finishes the next goal soonest.
With `--early` it also sends an input one or two frames earlier whenever the game ends up in
exactly the same state.

The first full run with the item slide took 25:24.4. The driver's random-room lookahead and
fixes to hints that had silently fallen back to the search brought that to 24:08.6. Most of
the rest came from parallel passes, each working on one stretch of the route in its own copy
of the project and measuring every change in the engine; the changes were then merged and the
whole route replayed from boot. `docs/anypercent/overnight.md` lists every verified time and
what changed between them.

To check the time yourself, put your own DOS CD files (`MONKEY.000`, `MONKEY.001` and the CD
audio as `track2.flac` and so on) in a folder under `game/`, remove `track6.flac`, build
ScummVM with `scripts/build-scummvm.sh`, then run:

```sh
uv run python scripts/anypercent_verify.py runs/anypercent/m2n/movie.jsonl out/verify-m2n \
  --goal '[{"room":0},{"var":293,"eq":161}]' --talkspeed 85 --explicit
```

Both replays should report frame 14820 and an RTA of 1261.483 s. `runs/anypercent/m2/` holds
the run with track 6 present (frame 16378, 1391.320 s).

To have the bot play the route from a checkpoint, copy a run directory, cut its
`results.jsonl` down to the goals you want to keep, and pass that copy as `CUT`:

```sh
uv run python scripts/anypercent_auto.py runs/anypercent/mine --slide --talkspeed 85 \
  --prefix runs/anypercent/CUT --resume runs/anypercent/CUT --start S1#4 --lookahead 12 \
  --lookahead-match "Carla|Fight|Catacombs exit|catacombs cave|enter the catacombs" --early
```

It prints one line per goal with the RTA so far, and writes `movie.jsonl` and `results.jsonl`
after every goal so a run can be resumed.

`rules/anypercent.md` has the rules and pinned settings, `docs/anypercent/findings.md` and
`docs/research/anypercent-*.md` the engine and route research, and
`routes/anypercent/README.md` the route notes.

### Glitchless Part I

It works in three stages:

1. A hand-written PDDL model describes the game. Every action cites the decompiled script
   it came from.
2. Every action is measured in the real engine on many seeds, and Fast Downward
   (`astar(lmcut())`) plans for time on those measured costs, keyed by where Guybrush is
   standing. The candidate routes are then raced on seeds 1 to 30, and the winner is
   reported on held-out seeds 31 to 60.
3. A patched ScummVM replays the route. It pushes verb/object sentences into the engine's
   own sentence queue, clicks verb and inventory slots, picks visible dialogue lines, and
   presses `.` and Esc to skip text and cutscenes. Guybrush walks and acts exactly as if a
   player had clicked.

The v1 segment is Part I. It runs from the moment the player gains control on the Mêlée
dock until the treasure trial and the idol trial are both complete. Completion is detected
from bit variables 85 and 86.

```
$ uv run speedrun run part1
...
TOTAL: 22382 ticks (6:13.03 at 60 Hz)
```

The time-optimal route has 67 actions and averages 22,292 ticks (6:11.5) over 30 held-out
seeds, with a stdev of 48. The visible demo reproduces the headless run tick for tick.
`docs/optimization.md` describes the method, and `docs/comparison.md` compares the route
with the human route.

### Quick start

Requirements: Linux, a C++ toolchain, SDL2, CMake, git and
[uv](https://docs.astral.sh/uv/). Python 3.12 is pinned, and uv installs it.

```sh
git submodule update --init --depth 1   # ScummVM v2026.3.0, scummvm-tools v2.9.0, Fast Downward 26.6.0
uv sync
uv run speedrun extract                 # game/ -> game/classic/ (see "Game data")
scripts/build-scummvm.sh                # stock ScummVM + patches/*.patch, scumm engine only
scripts/build-downward.sh               # Fast Downward release build
scripts/dump-scripts.sh                 # optional: decompile every script into data/scripts/
uv run speedrun demo part1              # watch Guybrush play the route hands-free
```

### Commands

| Command | What it does |
|---|---|
| `speedrun extract` | Detects the game data layout under `game/` and extracts the classic files. |
| `speedrun dump-objects` | Dumps every room's objects and the verbs from the engine to `out/objects.json`. |
| `speedrun plan part1` | Runs Fast Downward on `pddl/part1/` and prints cost, actions and transitions. |
| `speedrun compile [part1]` | Turns the plan into the player's JSONL steps, using the object dump for ids. |
| `speedrun run part1` | Replays headless (a few seconds) and prints the per-step table and total ticks. |
| `speedrun demo part1` | Replays in a visible window. It fast-forwards the boot (logo, credits, opening; about 2.6 minutes at real speed) to the dock, then plays the segment at real speed. `--no-fast-boot` plays the boot at real speed too. Tick counts are identical to `run`. |
| `speedrun measure part1` | Replays the plan headless on many seeds in parallel (`--seeds 1-30`) and reports per-action and total ticks. |
| `speedrun optimize part1` | Searches for the plan with the lowest mean ticks over many seeds and installs it as the time plan. |

`run` and `demo` re-plan and re-compile automatically when the model is newer than the
compiled plan. They replay the time plan from `speedrun optimize` when it is fresh, and the
action-count plan otherwise; `--objective actions|time` picks one explicitly. Every run
writes `trace.jsonl`, `state-start.json` and `state-end.json` to
`out/runs/<timestamp>-<mode>/`.

### How it fits together

```
game/ ──extract──► game/classic/ ──descumm──► data/scripts/ (gitignored)
                                                    │ cited by
pddl/part1/{domain,problem}.pddl ◄── hand-written ──┘
        │ Fast Downward astar(lmcut())
out/plans/part1.sas_plan ──compile (steps.toml + objects.json)──► out/plans/part1.jsonl
        │ SPEEDRUN_PLAN
patched ScummVM (engines/scumm/speedrun/) ──► out/runs/<ts>/trace.jsonl ──► tick table
```

The engine bridge comes from `patches/0001-scumm-add-speedrun-bridge.patch`, which adds new
files under `engines/scumm/speedrun/` and changes about a dozen lines in `module.mk`,
`scumm.h`, `scumm.cpp` and `input.cpp`. `SPEEDRUN_*` environment variables configure the
bridge (they are listed in `docs/plan.md` C1), and it does nothing unless `SPEEDRUN_OUT` is
set.

Timing is in engine ticks (1/60 s jiffies, summed over engine frames), never wall-clock
time. The bridge also advances audio in ticks, so headless and windowed runs give the same
count, but this leaves the demo silent.

`rules/glitchless.md` has the rules for the glitchless category, the pinned engine settings
and the click-equivalence rules.

### Game data

You must supply your own copy of the game in `game/`. That folder is gitignored, as are all
derived files, decompiled scripts, dumps and traces. `speedrun extract` detects:

- a classic Mac HFS disk image (`*.img`) holding `MONKEY1.000`/`MONKEY1.001` and the
  `Monkey Island` application. This project was developed against that layout: ScummVM
  gameid `monkey`, variant `Mac`, SCUMM v5, `MONKEY1.000` MD5
  `2ccd8891ce4d3f1a334d21bff6a88ca2`.
- already-extracted classic files `MONKEY1.000`/`MONKEY1.001`.
- the Special Edition `Monkey1.pak`. ScummVM reads it directly, but script dumping needs
  the classic files.

### Tests

```sh
uv run pytest            # unit + integration (real patched ScummVM, real game data)
uv run pytest -m "not slow"
```

If the game data or a build is missing, the integration tests skip loudly: a banner at the
end lists every skipped test, so that a skip is not mistaken for a pass.

### Documents

- `docs/plan.md`: the implementation plan and all interface contracts.
- `docs/part1/`: cited script analysis and `model.md`, which describes the PDDL model.
- `docs/human-route.md`: the current human speedrun route, with sources.
- `docs/comparison.md`: this project's route compared with the human route.
- `docs/next.md`: follow-ups that are out of scope for v1.
- `docs/anypercent/overnight.md`: every verified Any% time and what changed between them.
- `docs/anypercent/findings.md`: Any% engine and route findings, with script citations.
- `docs/research/anypercent-runner-sources.md`: the human Any% route and its sources.
- `docs/research/anypercent-engine-mechanics.md`: timing, input and skip mechanics in the engine.
