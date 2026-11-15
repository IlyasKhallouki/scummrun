# E027: a search in the engine itself

Date planned: 2026-11-10
Research questions it serves: RQ1 (completion from the game files alone), RQ4 (generality)
Base: branch `direct`, from `e026` at 9a63b43. Code: `experiments/E011-speed/rs/fmi-direct/` (bin `direct`).

## Disclosure: written after the first runs

This plan was written on 2026-11-10 after the spike's runs r0-r12 and s1-s3 (notes/log/2026-11-10-e027-direct.md).
Run s4 was running and unread. So:

- The method below is what those runs led to, not a design fixed in advance.
- Every prediction below was made knowing r12's and s3's results. They are predictions for the runs after s3 only.
- No prediction was written before r12. That run is a result with no prediction.

## Question

Can a search made in the engine itself, with no model of what an input does, play Monkey Island Part I from boot to
its end (`owner:901 = owner:902 = owner:903 = 1`), every kept state replayed from boot?

Hypotheses:

- **H1 (the engine is the model).** The engine forks in about a millisecond and runs about 19,000 frames a second on
  one core. Trying an input costs less than predicting it: no reading, no plan, and none of the classes of error that
  come from a second model of the engine (waits, talk, actor rooms, timing).
- **H2 (novelty is enough to move).** The game's progress shows in facts the engine holds: rooms, owners, states,
  classes, bits, some variables, the interface's texts, what is on the screen. A state that holds one of them for the
  first time is worth expanding. No goal is needed to leave a room.
- **H3 (the scripts advise).** The scripts are still read, but only to say which keys matter (the guide's keys) and
  later in what order. Nothing the scripts say is taken as true: a wrong hint costs time, never a wrong state.

## Why now

- E026's last kitchen run from boot (K1, e016's build, 4 workers) ended at 2,284.9 s with no kitchen: 8,099
  worker-seconds, of them 3,731.7 in the free tries' waits, 2,659.3 in room 28's reading and its queue, 1,579 in
  three tries the memory guard killed, 127.4 in room 33.
- E026's build then stopped twice at its own gates: Amendment 2 and 2b's checks, and G11 (the landmarks).
- E015's gate G2 (2026-11-04) stood against an engine-first design. Read again, it failed on its wording: it asked
  that trial t596 be kept, and the method kept an identical earlier trial. In the same data the method kept every
  step along the real route (D2: longest unkept run 0).

## Method

The search is described in `fmi-direct/src/main.rs`'s module text. In short:

- **Inputs.** Click sequences found breadth first at a state: the verb slots, the objects and actors the hit test
  answers, a grid of floor points. A click that leaves a sentence half made is clicked on from. At most 4 clicks.
- **A trial.** A fork, the click, the pointer parked, a run to the next stop where the bridge is idle and ego stands.
- **The game's own moves.** Each state is also run with no input (50, 200, 800 frames). Where the world moved by more
  than the room's flicker, the inputs are tried again from there.
- **Kept.** A state is kept for something new: a world atom; an interface or a thing on the screen in its room; a walk
  box or camera bucket in its room; an owned object in a new room; a variable's value; an atom new to its room; a new
  world. Seven queues in that order. Nothing is pruned for good.
- **The world's variables.** The guide's variables, less the interface's own, the clocks, and the uncapped counters.
- **Order.** Batches of states expanded side by side and committed in order, so a run is the same every time.

Reused from the system: the engine and its bridge (svm-e024, patches 0001-0010), `fmi-core`, `fmi-engine`, afford's
`offer_cfg` and `composing`, and `fmi guide map` for the guide's keys. Not used: the reader's readings, the planner,
`fmi-play`.

- Game, release, settings, seed: Monkey Island, DOS CD, E026's `env.sh` (`ON`, `MI`), seed 1. The root's movie starts
  with the skips event (text and cutscenes).
- Commands: `direct OUT --minutes M --workers W --guide GUIDE --rungs RUNGS`; `direct verify NODES ID OUT KEY=V`.
- Held out: the rungs file (`fmi-checks/rungs/rungs-part1.json`) is read only to report when a rung first holds.
  Run B, the hand routes and the Part I key are not read at all.

## Rules

- No input is written or picked by hand. No room, object, verb, variable or actor number in the code.
- A change is general or it is not made: it must be stated without naming the game, and it stays on for both games.
- A fix is judged by the game: does a run get further, or as far in less time. No gate is worded so that a method
  that works can fail it.
- Every rung claimed is replayed from boot in a fresh engine (`direct verify`).
- A run is the same twice (two runs of one binary give the same kept states in the same order), or the difference
  is reported.

## Measures

- The rungs P1-P14 of Part I: for each, the wall time at which a kept state first holds it, the clicks of its movie,
  and its replay from boot.
- Trials, kept states and expanded states per run; trials per second; trials per expanded state.
- The Fate of Atlantis demo's rungs C0-C9 with the same binary and no change.

## Rounds

A round is one general change, or a few of one kind, then one run: at most 30 minutes on the laptop (7 workers), or
at most 2 hours on the VM (30 cores) once a laptop run stops for compute and not for a missing mechanism. Each round
gets a line in the notebook entry: what changed, why, the run's rungs and times.

## Predictions (made after s3, see the disclosure)

| | Prediction |
|---|---|
| s4 (carried objects first), 30 min on the laptop | P1 within 6 min (90%). P2 (the money) held: 45%. P4's two owners held: 60%. |
| The furthest rung after 5 more rounds | central P5, range P3-P8 |
| The furthest rung after 15 more rounds | central P9, range P5-P14 |
| Part I's end within 15 more rounds | 35% |
| Part I's end within 30 more rounds | 60% |
| Where it first needs a new mechanism | the insult fights (P5-P6): random lines, a counter of wins, many short dialogs |
| The demo with the same binary, 30 min on the laptop | reaches C3: 70%; reaches C9: 30% |
| Two runs of one binary give the same kept states | 85% (a hash map's order somewhere is the risk) |

## Success threshold and stop rule

- **Yes:** a kept state holds Part I's end, replays from boot, and comes from a run that is the same twice. The demo's
  end with the same binary is reported beside it, reached or not.
- **Stop as "no":** three rounds in a row add no rung and bring no rung's time down by a fifth or more. Then the
  report says which mechanism the search lacks and whether the scripts could supply it.
- **Stop and ask:** a rung can be passed only by naming the game in the code.

## What this parks

E026's route (the reading's wait fix, the landmarks, the kitchen gates, Run 1) is not continued while E027 runs.
Its branches stay as they are: `e026` at 9a63b43, `e026-cost` at df6ea04, `e026-click` at e39925e.

## Addendum 1 (2026-11-11, 18:05 UTC): before the next runs on the VM

Written after vm28 and after the work on the laptop that followed it (the entry's sections from 16:10 UTC on), and
before any run with that tree on the VM. The rules and the stop rule above stand.

What changed since the runs that held P1-P12, in one line each (the entry has the measurements):

- a round is one piece worked on by several threads, the same with any number of them; no shares of first clicks;
- a page of the inventory is read from the slots' images (engine svm-e027, patch 0011), not guessed;
- a stop's engine comes from the nearest kept engine on its way; no boot from the whole movie;
- nodes.jsonl holds each stop's own events; a run writes its rung stops and its newest top stop as it goes;
- `--go` takes the exits the scripts name as ways, and a walk that cannot go on leaves the stop where it stands;
- `--shares`: the lines take turns (the newest every second turn, the next every fourth, and so on).

Runs planned, each at most 2 hours on the VM, launched and read with `experiments/E027-direct/run/vm.sh`:

1. **One run from boot**, `--lines gains --flag-lines --go --shares`. It is the run owed since vm11: the rungs so
   far were held by a chain of eleven runs.
2. **One run from vm28's stop 21276** (P12 held), same flags, for P10, P13 and P14.

Before either: the two checks on the laptop (object 420 from vm24's stop, the third trial from vm26's stop) pass
with the tree that is run, and two runs of the bench give the same digests.

Predictions (made now; the checks on the laptop are known for object 420 only: 13 stops with `--shares`, 77
without):

| | Prediction |
|---|---|
| The run from boot holds P6 (first trial) within 60 min | 70% |
| The run from boot holds P7 (second trial) within 120 min | 40% |
| The run from boot holds P9 within 120 min | 20% |
| Trials a stop, late in the run from vm28's stop (vm28: 3,994) | 1,200 to 2,000 |
| Stops expanded a minute at that depth with 28 workers (vm28: 21) | 60 to 150 |
| The run from vm28's stop holds P10 or P13 within 120 min | 35% |
| It holds P14 (Part I's end) within 120 min | 10% |
| Where the run from boot first stands still | the sword training's dialog (P5) or the fights: both took a mechanism each when first met, and neither has been met from boot with this order |
| Two runs of the bench give the same digests | 95% (five references gave the same digests with 1 and 4 threads, several times) |

What would count against the tree: the run from boot not holding P3 and P4 within 40 min (vm11 held them at 22
and 24 min with the old order): then `--shares` or the page reading costs the early game more than they give the
late game, and the two are measured apart.
