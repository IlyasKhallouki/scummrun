# 2026-11-01: E003 pick windows in taskcore (plan step C1)

Question: does moving each free pick to a window just before the operator that reads it keep the optimum, and does A* then
prove the optimum where it ran out before? Proof: `experiments/E003-path-through-cloud/taskcore/README.md`, "Pick windows".
Numbers: `experiments/E003-path-through-cloud/results/pick-windows.log`.

Answer: yes at unit cost. A* with landmark cost partitioning proves 6 on the bar door in 15 expansions and 6 on k = 1 in 18,
where it had not closed f = 5. The kitchen stays at 7. The F1c kitchen, left at 6 or 7, is 7: A* closes f = 6 after 4,945
expansions. With frame costs, A* proves the bar door's optimum, 12 frames. On the kitchen it reaches a lower bound of 33 frames
against a plan of 373 frames in 600 CPU s.

## The rewrite

A guess written through a pick variable gives a pick operator per value: no precondition, cost 0, never set back. From every
state every assignment of the pick variables is reachable at cost 0, so A* expands that plateau before it closes an f-layer.

With `--pick-windows on` (taskcore `sas`, `decode`, `serve`; off by default) the free picks go. An operator whose effects
read pick variables opens a window: `begin` (its precondition, lock 0 to j), one pick per value of each variable it reads,
then the operator itself, which sets those variables back to 0 and lock to 0. Every other operator and the goal need
lock = 0.

The proof uses six properties of taskcore's files, which hold for any game: preconditions and the goal name only facts, only
pick operators write pick variables, pick variables appear only in effect conditions, every value has a free pick, and picks
start at 0. Two theorems map plans both ways, with the same steps outside the picks, in the same order, and the same cost. So
the optimum is the same, and `decode` reads the same steps. The writer checks three of the properties on every variant.

The file with the switch on is the prototype's (`lockpicks.py`) line for line on the bar door, but for names.

## Unit costs

| scope | operators off | on | A* off | A* on |
|---|---|---|---|---|
| bar door | 5,523 | 9,478 | f = 5 not closed in 300 CPU s, 2.5 GB | 6, 15 expanded, 2.1 s, 53 MB |
| k = 1 | 53,372 | 74,806 | f = 5 not closed in 300 CPU s, 1.2 GB | 6, 18 expanded, 11.8 s, 243 MB |
| kitchen | 17,672 | 22,842 | 7, 8 expanded | 7, 8 expanded |
| kitchen, F1c | 43,323 | 130,558 | f = 6 not closed in 1,800 CPU s (F1c's gate) | 7, 4,953 expanded, 26.7 s, 251 MB |

The F1c kitchen's optimal plan is the 7-step plan the greedy search found there. The reading was made again on this branch
and has F1c's packed sha1.

None of these optimal plans opens a window. The goals need no guessed value, and on the bar door no consumer applies in the
first 20,000 states of a breadth-first search. So the plateau was made of picks these plans never read.

## Frame costs

| scope | operators on | upper bound | A* on |
|---|---|---|---|
| bar door | 9,478 | 12 frames, 7 steps | optimum 12, 4,770 expanded, 2.7 s, 53 MB |
| kitchen from s17 | 22,842 | 373, 8 steps | lower bound 33 at 491 CPU s, 818,049 expanded; stopped at 600 CPU s, 402 MB |
| kitchen, replan p0 | 22,993 | 377, 11 steps | lower bound 27 at 438 CPU s, 673,405 expanded; stopped at 600 CPU s, 475 MB |

With the switch off, A* stays at its first f-layer: 12 on the kitchen from s17 after 600 CPU s (1.5 GB), 5 on the bar door until the
memory guard stopped it at 429 s wall.

The kitchen's upper bounds hold one event of 361 frames, the cook's timer. Each frame of lower bound costs 1.5 to 1.7 times the
expansions of the one before. Projected at that rate, explicit A* does not reach 373. A bound that close needs a heuristic that
sees waiting (plan step C3).

## Checks

- Random tasks (test_taskcore's), 300 from seed 1 in four configurations: Fast Downward's optimum off and on equals the task's
  own in every case. 816 optimal plans each way were mapped to the other file and run there at the same cost, and decode reads
  the same steps from both. With 60 tasks from seed 3, 54 plans each way pass through a window.
- On the packed files: the optimal plans with the switch on, mapped back and run on the file with it off, at the same cost and
  with the same steps; random operator sequences on the bar door both ways.
- Switch off: `serve_check.py` on 31 recorded requests (kitchen, kitchen with bans, bar door, its control, k = 1 to the door
  and to Part I): ksr's taskcore and this branch's `sas` and `serve` write the same bytes. The reader is unchanged: the kitchen's
packed sha1 is 279b7a24 with every switch off. 41 tests pass.
