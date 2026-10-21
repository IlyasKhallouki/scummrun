# Any% plan: the full game, as fast as the engine allows

Goal: an automated run of *The Secret of Monkey Island*, from the logo skip to
the final fade, under the speedrun.com Any% rules (`rules/anypercent.md`),
faster than the human record and than any tool-assisted run could be.

"Faster than any TAS" has no external yardstick: no MI1 TAS exists
(`docs/related-work.md` §2.1). Here it means a run whose every stretch is the
result of a search over the inputs that matter (which room order, which click
point, which frame), with the search bounded and the bound stated. A TAS author
does the same search by hand with savestates; this project does it with exact
process snapshots.

## Where things stand (2026-10-19)

- **Release.** The DOS VGA floppy (SCUMM v4) runs in the patched ScummVM
  v2026.3.0. Its 1651 script blocks are decompiled into
  `data/scripts-dos-vga/` with no descumm errors. The object dump covers all 83
  rooms across the four disks.
- **Bridge v3.** Raw input on exact frames (C8), a per-frame state log, room
  and control records, an RTA clock that counts every engine wait
  (quarter-frames, `qf`), the timer variable check relaxed for Any%, and a
  control socket (C9) that runs the game to a frame or condition and forks it.
  A fork is an exact snapshot (`tests/integration/test_control.py`); three
  branches and a straight replay took under 2 s.
- **Measured** (`docs/anypercent/findings.md`): the logo glitch window (Esc on
  frames 45-91 with AdLib), 470 ticks from the logo skip to control on the
  dock, and the music driver's effect on sound waits.
- **Engine mechanics** (`docs/research/anypercent-engine-mechanics.md`): the RTA
  formula, the walk-step code and two candidate walking exploits, door and exit
  timing, the timer variable, the drivers, RNG and snapshot safety, the input
  path.
- **Runner sources** (`docs/research/anypercent-runner-sources.md`): the human
  route with split times, the trick catalogue (logo glitch, Credit Early, item
  slides, the monkey skip, catacombs RNG, insult fights), the leaderboard and
  the version question.

## The version decision

The record (saruya, 21:52, 2026-08-23) is a DOS CD run built on three item-slide
sequence breaks (oars, monkey, feather) that are documented for CD only. The
same runner's best EGA floppy run, a week earlier, is 68 s slower, almost all
of it in Part III. VGA floppy, the release we hold, sits behind EGA. CD also
runs at 60.00 ticks per second against the floppy's 59.17. So unless the item
slides reproduce on v4, the fastest route is on DOS CD, and the floppy work
should stop at what transfers (the bridge, the clock, the executor, the
search). Two things to settle:

1. Get the DOS CD release (with its CD audio tracks; music is part of the
   game's timing) and bring it up as `dos-cd`.
2. CD audio advances a counter on ScummVM's timer thread, so the CD build is
   not deterministic or fork-safe as is. The bridge must drive that counter
   from game ticks, as it already drives the mixer.

## Architecture

Two levels, as a TAS author works.

**Macro: the route.** Which puzzles, in which order, with which item and
dialogue choices, and which glitches. The sources are the human Any% route and
the floppy scripts, read the way Phase 7 read the Mac scripts. A route is a
list of moves: a verb on an object, a floor click, a dialogue line, a key
(Esc, `.`), a save or load. Where the planner helps (orderings, alternatives),
the route is a PDDL plan over measured costs, as in the glitchless work.

**Micro: frames and pixels.** Each move becomes raw input: a verb key or verb
click, then a click at a screen point, on a frame. The executor drives the
engine through the control socket, issues each move on the first frame the
engine takes it, and forks to try alternatives:

- the click point (the walk target; door approach direction; class-138 exits'
  nearest pixel; zig-zag or re-click patterns where they pay off);
- the frame (for example the logo-skip frame, or waiting a frame so a later
  wait aligns);
- the next move's order when two moves are independent.

The search keeps a beam of branches ranked by RTA quarter-frames to a common
milestone (a room entry, a flag), so every candidate is compared at the same
point of the game. The best branch's inputs are appended to the movie, which
is the deliverable: one JSONL schedule that replays to the final fade.

**RNG.** The seed is free (Any% rules). With an exact engine, random content
(the swordfights, the forest guide, the catacombs, Stan, the storekeeper)
depends on the seed and the exact input frames. The search treats the seed as
one more variable at the points where randomness decides time, and the
Ctrl+digit load (no frame cost, RNG not saved) as a reroll if the rules
discussion in `rules/anypercent.md` keeps it.

## Milestones

1. **Executor (micro level) on Part I's opening.** Moves → raw input with
   earliest-frame issue and forked click-point search, proven on dock → bar →
   kitchen. Compare with the human split for the same stretch.
2. **Full human route as a movie.** Encode the WR route on the floppy as moves
   (from the runner research), play it end to end, and get the first full-game
   Any% time. This is the baseline to beat.
3. **Micro search everywhere.** Run the beam search over every stretch.
4. **Macro search.** Alternatives from the floppy scripts (sequence breaks,
   orders, the other trial routes), RNG-dependent sections by seed search.
5. **Glitches.** Item slides, Credit Early, and anything the scripts and the
   engine allow that the runners do not use yet.
6. **Version check.** If the runner research shows DOS CD is faster for
   Any%, measure it before investing further in the floppy. The CD build is not
   fork-safe as is (its CD timer runs on a thread).

## Settled by runner practice

- **Snap scroll (Ctrl+R)** on floppy, quicksave and quickload, and restart are
  in the record runs' keybinds (`rules/anypercent.md`). In ScummVM, Ctrl+R
  needs `original_gui=true`; otherwise it opens a modal dialog.
- **The music device** is a route parameter. It changes sound waits and, on
  the floppy, script branches and the RNG stream (`VAR_SOUNDCARD`).
- **Text speed** is free. The CD item-slide chain wants slow text, because the
  slides ride on long reading animations.
