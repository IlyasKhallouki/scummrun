# fast-mi

An automated speedrunner for *The Secret of Monkey Island* (SCUMM v5), running on a patched
ScummVM.

[![The bot's Any% run, DOS CD, 21:01.483](https://img.youtube.com/vi/lHQrHf5b_5o/maxresdefault.jpg)](https://youtu.be/lHQrHf5b_5o)

The bot's best Any% run on the DOS CD release is 21:01.483, 50.9 s under the human record
(21:52.4 by saruya). [Watch it on YouTube.](https://youtu.be/lHQrHf5b_5o)

The time comes from replaying the run's input file from boot twice: once with the bridge's
automatic text and cutscene skips, and once with every skip written out as a key press. Both
replays end on frame 14820.

That run was made with CD track 6 missing from the game folder. The "Last Part" card waits for
that track to finish, so it lasts 130 s when the track plays and 0.2 s when the file is
missing. In the record's video it lasts about 6 s. With the track present, the same route runs
23:11.3. The leaderboard bans the "No Music" setting. Here the music driver is unchanged and
every other CD track plays, so whether a missing track 6 counts is still an open question.

The repo holds two projects that share the engine bridge: the Any% bot below, and the
original glitchless Part I planner after it.

## Any%

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

## Glitchless Part I

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

## Where this is going

The long-term goal is a system that plans and replays glitchless speedruns for all, or
most, of the games ScummVM supports. Monkey Island Part I is the first target, chosen to
prove the pipeline end to end.

Some parts are already game-agnostic:

- The engine bridge works at the level of the SCUMM engine rather than any one game. It
  pushes sentences into the engine's sentence queue, clicks verb slots through the game's
  input script, answers dialogue menus (which are ordinary verbs), detects when the game
  accepts input, keeps a tick clock, advances audio in ticks, and presses `.` and Esc
  through the engine's own key handling.
- The Python pipeline (extract data, dump objects, plan, compile, measure, optimize) only
  needs a segment definition (`segment.toml`), step templates (`steps.toml`) and a PDDL
  model.
- Phase 7 showed that six blind extractor agents, reading only decompiled scripts, can
  produce a cited model over engine-state atoms. After three defects were fixed, the
  extracted model reached the hand-written model's optimum (`docs/extraction-diff.md`).
  The extraction also found a second exit at the forest fork, path 686, that the
  hand-written model lacked. The route takes it twice, 54 ticks faster each time than the
  usual exit, and it gave the route its last improvement.
  Extracting the model from decompiled scripts is the route to new games without
  hand-writing a model for each one.

Other parts are still specific to Monkey Island: the dialogue verb range (120 to 128), the
inventory slot layout, the island map's hover-label idle rule, the Mac disk extraction, and
the `monkeycdalt` id used by the script dumper. `docs/next.md` lists the steps toward other
games.

## Quick start

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

## Commands

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

## How it fits together

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

## Game data

You must supply your own copy of the game in `game/`. That folder is gitignored, as are all
derived files, decompiled scripts, dumps and traces. `speedrun extract` detects:

- a classic Mac HFS disk image (`*.img`) holding `MONKEY1.000`/`MONKEY1.001` and the
  `Monkey Island` application. This project was developed against that layout: ScummVM
  gameid `monkey`, variant `Mac`, SCUMM v5, `MONKEY1.000` MD5
  `2ccd8891ce4d3f1a334d21bff6a88ca2`.
- already-extracted classic files `MONKEY1.000`/`MONKEY1.001`.
- the Special Edition `Monkey1.pak`. ScummVM reads it directly, but script dumping needs
  the classic files.

## Tests

```sh
uv run pytest            # unit + integration (real patched ScummVM, real game data)
uv run pytest -m "not slow"
```

If the game data or a build is missing, the integration tests skip loudly: a banner at the
end lists every skipped test, so that a skip is not mistaken for a pass.

## Documents

- `docs/plan.md`: the implementation plan and all interface contracts.
- `docs/part1/`: cited script analysis and `model.md`, which describes the PDDL model.
- `docs/human-route.md`: the current human speedrun route, with sources.
- `docs/comparison.md`: this project's route compared with the human route.
- `docs/next.md`: follow-ups that are out of scope for v1.
- `docs/anypercent/overnight.md`: every verified Any% time and what changed between them.
- `docs/anypercent/findings.md`: Any% engine and route findings, with script citations.
- `docs/research/anypercent-runner-sources.md`: the human Any% route and its sources.
- `docs/research/anypercent-engine-mechanics.md`: timing, input and skip mechanics in the engine.
