# Stalls: /var/tmp/fast-mi/e013-stall/r2-e14/on/kitchen

4 tries, wall 338.2 s, rows 338.2 s

## the run (4 tries, 338.2 s)

| class | seconds | share |
|---|---:|---:|
| productive | 337.8 | 99.9% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.2 | 0.1% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 0.0 | 0.0% |
| model, no yield | 0.0 | 0.0% |
| wait | 0.2 | 0.1% |
| overhead | -0.1 | -0.0% |

rest: -0.24 s (-0.07%), absolute 0.28 s (0.08%)

## the last 20 (4 tries, 338.2 s)

| class | seconds | share |
|---|---:|---:|
| productive | 337.8 | 99.9% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.2 | 0.1% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 0.0 | 0.0% |
| model, no yield | 0.0 | 0.0% |
| wait | 0.2 | 0.1% |
| overhead | -0.1 | -0.0% |

rest: -0.24 s (-0.07%), absolute 0.28 s (0.08%)

## Rows over the run (largest 40)

| phase | outcome | class | seconds | tries |
|---|---|---|---:|---:|
| plan | unsolvable | productive | 207.9 | 1 |
| read: refinement | added | productive | 43.2 | 1 |
| read | no cached reading | productive | 41.7 | 2 |
| plan | plan | productive | 25.6 | 3 |
| play | N frames; interface | productive | 9.4 | 1 |
| afford | no plan, sentence | productive | 3.9 | 1 |
| afford | empty window, sentence | productive | 3.7 | 1 |
| T |  | productive | 1.9 | 3 |
| wait | static | wait | 0.2 | 4 |
| read | reused (e009 keys) | productive | 0.2 | 1 |
| read | no cached reading | no plan | 0.2 | 1 |
| return | same | overhead | 0.2 | 4 |
| play | ready: the first end point in the order of tries; deviation | productive | 0.1 | 1 |
| play | the input's precondition closed in the parent after its window opened; success | productive | 0.1 | 1 |
| play | a needed record refuted in the parent; interface+refuted | productive | 0.0 | 1 |
| play | the input's precondition closed in the parent after its window opened; spent | productive | 0.0 | 1 |
| play | ready: the first end point in the order of tries; goal | productive | 0.0 | 1 |
| play | ready: the first end point in the order of tries; success | productive | 0.0 | 1 |
| T |  | no plan | 0.0 | 1 |
| rest |  | overhead | -0.2 | 4 |

## Tries by stop and class

| stop | try's class | tries | wall s |
|---|---|---:|---:|
| goal | productive | 1 | 2.5 |
| no plan (the planner proved none) | productive | 1 | 0.9 |
| none played: no target | productive | 1 | 4.2 |
| the same empty window | productive | 1 | 330.6 |

## Shift

now: null
changes: []
cells the cell rule names: []

## Tries by stop and progress (a new atom of T or a new room)

| stop | progress | tries | wall s | productive s |
|---|---|---:|---:|---:|
| goal | progress | 1 | 2.5 | 2.4 |
| no plan (the planner proved none) | progress | 1 | 0.9 | 0.8 |
| none played: no target | progress | 1 | 4.2 | 3.9 |
| the same empty window | progress | 1 | 330.6 | 330.7 |

## The shift and the cell rule replayed over the commits

- the shift on after 0 of 4 commits; changes []
- the largest last-20 share of each non-productive class (the shift's threshold is 50%): no plan 0.1% after commit 2 (j00002); wait 0.1% after commit 3 (j00003)
- tries the cell rule would have made free (0 of 4; the cells as they are now; [commit, job, the try's class]): []
- cost: the table from the files 0.148 s (37.09 ms per try), its JSON 0.2 ms

## Checks

- PASS every try's rows sum to its wall: 4 tries, largest difference 0.00e0 s
- PASS the rows' total is the tries' wall: rows 338.200 s, wall 338.200 s
- PASS the rest row is at most 1% (signed and absolute): rest -0.242 s (-0.072%), absolute 0.283 s (0.084%)
- PASS the new entries give back each line's new_t and new_rooms: 4 of 4 lines with E012's records; failing: []
- PASS the live stalls.json equals this report's table: 12748 bytes live, 12748 bytes offline
