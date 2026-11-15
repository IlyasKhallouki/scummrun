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

- Results, the exact settings, and the whole input movie: [`experiments/E027-direct/results/full5`](https://github.com/IlyasKhallouki/scummrun/tree/direct/experiments/E027-direct/results/full5) (branch `direct`).
- The lab notebook of the search: [`notes/log/2026-11-10-e027-direct.md`](https://github.com/IlyasKhallouki/scummrun/blob/direct/notes/log/2026-11-10-e027-direct.md).

### Indiana Jones and the Fate of Atlantis, the playable demo (9 November 2026)

[![Indiana Jones and the Fate of Atlantis demo, 3:51](https://img.youtube.com/vi/npbKSDtlLX8/maxresdefault.jpg)](https://youtu.be/npbKSDtlLX8)

The earlier version of the system found a route from boot to the end of the demo (experiment E022), then shortened
its own route (E024) to 92 clicks: 3:46.8 of game time on seed 1, 3:51.5 in real time in the video. The route is
verified from boot, and no click in it was written by hand. [Watch it on YouTube.](https://youtu.be/npbKSDtlLX8)
Plan and results: [`experiments/E024-shorten/plan.md`](https://github.com/IlyasKhallouki/scummrun/blob/direct/experiments/E024-shorten/plan.md).

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

- Branch [`direct`](https://github.com/IlyasKhallouki/scummrun/tree/direct): the search ([`experiments/E011-speed/rs/fmi-direct`](https://github.com/IlyasKhallouki/scummrun/tree/direct/experiments/E011-speed/rs/fmi-direct)),
  the engine patches ([`patches/`](https://github.com/IlyasKhallouki/scummrun/tree/direct/patches), 0001 to 0013), the run and check scripts
  ([`experiments/E027-direct/run`](https://github.com/IlyasKhallouki/scummrun/tree/direct/experiments/E027-direct/run)), the lab notes ([`notes/log`](https://github.com/IlyasKhallouki/scummrun/tree/direct/notes/log)) and every
  experiment's plan and results ([`experiments/`](https://github.com/IlyasKhallouki/scummrun/tree/direct/experiments)).
- Branch `main` (this page): the earlier planner and Any% bot, the survey of related work
  ([`docs/related-work.md`](docs/related-work.md)), and the paper's working files (`paper/`).

## Checking the Monkey Island result

You need your own DOS CD files (`MONKEY.000`, `MONKEY.001` and the CD audio as `track2.flac` to `track25.flac`; their
checksums are in `results/full5/settings.md`), the patched engine, and the search built from branch `direct`:

```sh
# on branch direct, with the game in game/dos-cd and the engine built (see below)
cd experiments/E011-speed/rs && cargo build --profile fast -p fmi-direct && cd -
export FMI_ENGINE_E027=/path/to/patched/scummvm
source experiments/E027-direct/run/env.sh
mkdir -p out/verify-full5
env $ON $MI experiments/E011-speed/rs/target/fast/direct verify \
  experiments/E027-direct/results/full5/end-stop.jsonl 7895 out/verify-full5 bit:565=1
```

It should print `"frame":114978`, `"clicks":1994` and `"ok":true`.

The engine is ScummVM v2026.3.0 with patches 0001 to 0011 applied (0012 and 0013 act only for other games). The build
the runs used is not yet scripted end to end: its tree holds a small trace hook that is in no patch file and that no
run uses. To rerun the search itself, see `experiments/E027-direct/run/chain.sh` and the notebook; check a new build
first with `experiments/E027-direct/run/bench.sh`, whose digests must match `results/full5/gate-vm.txt`.

## Research

A paper on the search is in preparation. `notes/` on branch `direct` holds the decisions, the open questions and the
lab notebook; `experiments/` holds each experiment's plan (question, prediction, stop rule) and results.

## Earlier work: the planner and the Any% bot (October 2026)

Everything below describes the system on this branch (`main`) before the search: the Any% bot of the video above
and the glitchless Part I planner. It is kept as it was.

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
