# Any% rules

These rules define the Any% category: the full game, played the way the
speedrun.com Any% board (ScummVM subcategory) allows, so that a bot time can be
set beside the human leaderboard. They sit beside `rules/glitchless.md`, which
still governs the Part I glitchless route. Nothing here changes that route or
its results.

The source for every board rule quoted here is `docs/related-work.md` §1.2,
which records the rules text as fetched from speedrun.com's API.

## What the board allows, and what we take from it

| Board rule or practice | Here |
|---|---|
| "Timing starts when the Lucasfilm Games logo is skipped and ends when the game fades to black after the final cut scene" | Same. A run is timed from the frame the logo-skip Esc is delivered (`SPEEDRUN_START_FRAME`) to the first frame of the final fade. |
| "Runs are timed in RTA" | Wall-clock time is computed from engine ticks and the release's timer rate (`speedrun.releases`): DOS v4 ticks last 4 × 5041 / 1193182 s, Mac and DOS CD ticks 1/60 s. The clock must also count every wall-clock wait outside the main loop (fades, transitions); `docs/research/anypercent-engine-mechanics.md` lists them. |
| No glitch is banned in writing; the logo speed glitch, item slides and Credit Early are in active use | Allowed. Any behaviour the unmodified engine produces from player input is allowed, including walkbox exploits, the logo glitch (var 19 left at 5) and any other timer-variable state the scripts can be driven into. |
| "Credit Early*, Saveless and Saves (Using save states) are all allowed"; the guide adds "only save files created during the run may be loaded" | Allowed: saving and loading through the engine's own save system, during the run, with saves made in that run. Load times count. |
| Fixed ScummVM random seeds are disclosed by every top run and accepted | Allowed: the run names its seed, and the seed may be chosen. A route may be built for one seed. |
| "Any speed benefits only possible in ScummVM are allowed" | Allowed for game behaviour (for example ScummVM's instant room loads). Not allowed for anything that changes the timer rate or skips real-time waiting: see Banned 1. |
| "Any Music Device may be used EXCEPT No Music" | A real music driver is always selected and recorded. The driver is a free choice among the release's drivers, because it can change the length of sound waits. |
| ScummVM 2.8 or later | ScummVM v2026.3.0, the build the current record was played on. |
| "Keybindings should stick to the rule of one key press resulting in one game input only" | Every scheduled input is one key press, one click or one mouse move on one frame. Runners bind two-key ScummVM and interpreter shortcuts to single keys: the record's description lists screen snap (Ctrl+R), quicksave (Alt+n), quickload (Ctrl+n) and restart (F8) on single keys (`docs/research/anypercent-runner-sources.md` §5.5). The bot may use the same functions, each as one scheduled input. |
| Screen snap (Ctrl+R) on the floppy releases | Allowed: an original v4 interpreter key that makes camera pans instant, used in floppy record attempts. It does not exist on CD (v5). |

## Inputs

The bot plays through the raw input schedule (bridge contract C8,
`SPEEDRUN_INPUTS`): key presses, left and right clicks at screen coordinates,
and mouse moves, each on an exact frame. These reach the engine in the same
fields a real SDL event fills, right before `processInput()` reads them, so the
game's own input scripts handle them. Pixel-level input (floor clicks, actor
clicks, clicking the nearest pixel of an exit, repeated clicks during walks) is
therefore allowed.

The sentence queue (`rules/glitchless.md`) remains available as a planning
shorthand, but a run that is reported as an Any% time must be replayable from
its raw input schedule alone.

One input per frame per device: at most one key press and one mouse event in a
frame, as a player can produce.

## Banned

1. **Changing the engine's speed.** ScummVM's fast-forward keys (Ctrl+F,
   Ctrl+G), turbo or frame-rate options, editing the timer frequency, and any
   modified build that changes timing. TASVideos rejected a Loom submission for
   Ctrl+G (`docs/related-work.md` §2.2); on the board it would be a different
   game. The bridge's `SPEEDRUN_FAST` only skips the wait in headless runs; the
   reported time is always computed from ticks, never from wall-clock.
2. **Debug keys and cheats**: Ctrl+W (instant win, which also misses the end
   condition), the ScummVM debugger console, boot params, editing variables or
   save files.
3. **Loading saves not made during the run.**
4. **Changes to the game data.**

## Settings

| Setting | Value |
|---|---|
| ScummVM | v2026.3.0 plus the speedrun bridge, which only observes and delivers input |
| release | recorded per run (`speedrun.releases`): `dos-vga` today |
| `enhancements` | `0` by default. Other values are allowed (they are ScummVM options a runner can set) but must be recorded, and routes measured under one value are never pooled with another. |
| `talkspeed` | free choice, recorded |
| `music_driver` | any real driver, recorded |
| `copy_protection` | `false`, the ScummVM default. The board allows it, and the original code-wheel check is not part of a run. |
| `--random-seed` | chosen, recorded |
| `--disable-sdl-audio` | on, with the bridge pumping the mixer on game ticks, so sound-dependent waits last as long as they would with audio |

## Reporting

A reported Any% time names the release, seed, settings, bridge version, and the
raw input schedule. It must replay to the same goal frame in the visible demo.
It is an automated run, so it is compared with the board in our documents and
never submitted to it: the site rules exclude games controlled by outside tools
(`docs/related-work.md` §1.3).
