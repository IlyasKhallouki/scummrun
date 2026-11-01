# 2026-11-01: E003, the kitchen from boot with --reach

Question: with every reader switch that passed (input_settled with --reach, frame costs), does the whole chain play the kitchen
from boot to owner:566 = ego? What happens at a replan start the --reach reading did not reach? How do the frames compare with
run B and with the earlier kitchen runs? Numbers in `experiments/E003-path-through-cloud/results/kitchen-boot-reach.log`, the
per-run logs `replay-part1-kitchen-boot.log` and `replay-part1-k1-boot.log`, and `runb-reference-meat-seed1.json`.

Answer: yes, in 2,216 frames, with 0 differences from boot. It needed two harness switches besides the reach extension: the
actors' rooms as unobserved facts, and the Executor's stepping stones on forks. No replan start left the reached values, so the
extension never ran. Run B's inputs on seed 1 take 523 frames from the same room. 2,052 of the 2,216 frames are the harness's
waits.

## The readings

Seed 1, from results/start-part1.jsonl, through `replay_part1.py read-rust`, which now passes reader-scope's `--switches`,
`--reach` and `--frame-costs` and keeps the reached values next to reading.tc. Switches: value_sets, draw_clears, actor_rooms,
input_settled, split_limit, reach, frame costs, with --elsewhere and --live-locals.

| scope | records | actions | read CPU s | peak MB |
|---|---|---|---|---|
| kitchen, rooms 33, 28, 41 | 41,877 | 40,773 | 106.9 | 582 |
| k = 1, rooms 28, 33, 35, 38 | 235,579 | 234,401 | 407.5 | 2,154 |

k = 1 with --reach alone was 51,726 records. input_settled multiplies it by 4.6 there, against 1.47 on the kitchen from s17.

## Three things the first runs showed

1. **The actors' rooms.** The first start was refused: taskcore answered that the start holds actorroom:1 = 0 and
   actorroom:3 = 0, which the reading did not reach. The state dump has no actor room (only the current room's visible
   actors), so the harness wrote 0 for every actorroom: fact, while the reader starts an actor's room the start does not give
   as not known (-1). Without --reach nothing checks this: a start value inside the domain is planned from.
   Switch `--unobserved-actor-rooms`: these facts are unobserved, as args: and saved: are; not matched, not waited for, the
   plan's values kept; not known at the first plan. The first start is then accepted, 44,605 operators.
2. **The start check through serve** raised on any error reply: a local `import re` in the same function hid the module's.
   Fixed.
3. **The door off screen.** From boot, ego enters the bar at its left side, and door 316 is at room x 592. The Executor's
   floor clicks toward it stop at screen x 303, since x 307 to 319 at the door's walk height are other objects, and the camera
   does not pan with ego at 303. The harness gives the Executor no fork, so no stepping stone is tried. The run without them:
   30 interface failures in a row, the replan limit, 1,057 frames, no goal. Switch `--approach-stones`: the Executor's stepping
   stones are tried on forks of the run's engine, each trial with no fork of its own, and only the stone kept is played. One
   stone brought the door on screen at frame 3533, camera at 288.

## The run

| | value |
|---|---|
| stop | goal, verified from boot: 0 differences at 14 checkpoints, goal at the end |
| frames | 2,216 (2,728 to 4,944) |
| plans, replans, bans | 3, 2, 0 |
| reach extensions | 0 |
| inputs | 39 |
| run wall | 432.5 s, of which planning 336.7 s and playing 60.4 s |

Plan 0 has 27 steps and predicts 343 frames. The two replans took 62.4 s of taskcore wall time (23.9 CPU s) and 80.6 s of search.
Each was built again in the server, because the cost caps change at each replan. Plan 0's search took 168.7 s.

Where the frames go:

| what | frames |
|---|---|
| walking and clicking up to each input | 128 |
| writes that held after an input | 30 |
| waits at the input limit for predicted writes that did not come | 1,200 |
| the wait for walk_to(428)'s precondition, room 33, with ego already in room 28 | 660 |
| retry waits | 192 |
| door clicks that did nothing | 6 |
| dialogue | 0 |

Door 316 took 4 clicks. 2 did nothing ("the record's path never started"), and the fourth took ego into the kitchen 6 frames
later. The harness then waited 594 more frames to the limit for the writes walk_to(316)[202] predicts (the cook's slots frozen),
which did not come.

Predicted against played, per plan: 343 against 731, 27 against 1,473, 7 against 12.

## Against run B and the earlier runs

| run | from | frames |
|---|---|---|
| this run | the idle stop at frame 2728, room 33 | 2,216 |
| run B's inputs on seed 1 | room 33 at frame 51 | 523 |
| run B's inputs on seed 1 | S1#7 pre, frame 235 | 339 |
| costs branch, unit cost | S1#7 pre | 13,141 |
| costs branch, frame costs and caps | S1#7 pre | 10,101 |

Run B's recorded inputs on seed 1 reach owner:566 = ego at frame 574, the ground truth's frame for S1#10. This run is 4.2 times
run B from the same room. Its walking and clicking, 128 frames, is under run B's total. The difference is in the waits: 1,860
frames of waits for predictions that did not hold, and 192 of retry waits.

## --reach and replans

Wired as reach.md planned it. On taskcore's refusal (`sas` exit 7, `serve` error, both naming the values) the harness writes the
start as the engine's facts.py map with its args: numbers, and reader-scope reads the scope again from the first start and every
start so far (`--reach-also`, now given once per start). The harness then takes that reading and plans again. A start the extended
reading still does not cover, or an extension that adds no value, stops the run. The extension's wall and CPU time are recorded per
extension.

In this run no replan start held a value outside the first reading's reached values: 0 extensions in 36 plans (34 on the
kitchen, 2 on the pilot). So its cost here is 0. The tests cover the refusal off and on, through sas and serve, and the reader
with three starts.

## The k = 1 pilot

Rooms 28, 33, 35, 38, goal state:316 = 1, the same switches, from boot through serve. Goal in 1,111 frames, 1 replan, 0 bans,
0 extensions, 0 differences from boot at 5 checkpoints. Of the 1,111 frames, 600 are a wait at the limit for var:102 = 1 after
door 428, which did not come. state:316 = 1 then held at frame 3839, during the retry waits of walk_to(428), before the plan's
own steps on door 316. The plan's search took 100.6 s and 62.6 s on 97,547 operators. taskcore took 152.9 s and 69.9 s (39.4 and
26.7 CPU s); the second request kept the server's task. The start check through serve took 275 s in all.

## Off: unchanged

Every new switch is off by default: read-rust's `--switches`, `--reach`, `--frame-costs`, and run's `--search`, `--cost-caps`,
`--reach-extend`, `--unobserved-actor-rooms`, `--approach-stones`. Without them the reader's command, the harness and the Executor
are as before. The harness tests: 121 passed. The unit tests: 925 passed. reader-scope's only change is that it reads
`--reach-also` more than once.
