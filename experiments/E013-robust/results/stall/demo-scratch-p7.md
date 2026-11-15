# Stalls: /var/tmp/fast-mi/e014-pscratch/p7

3 tries, wall 63.2 s, rows 63.2 s

## the run (3 tries, 63.2 s)

| class | seconds | share |
|---|---:|---:|
| productive | 46.8 | 74.1% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.1 | 0.1% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 16.1 | 25.5% |
| model, no yield | 0.0 | 0.0% |
| wait | 0.2 | 0.3% |
| overhead | 0.0 | 0.0% |

rest: -0.02 s (-0.04%), absolute 0.06 s (0.10%)

## the last 20 (3 tries, 63.2 s)

| class | seconds | share |
|---|---:|---:|
| productive | 46.8 | 74.1% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.1 | 0.1% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 16.1 | 25.5% |
| model, no yield | 0.0 | 0.0% |
| wait | 0.2 | 0.3% |
| overhead | 0.0 | 0.0% |

rest: -0.02 s (-0.04%), absolute 0.06 s (0.10%)

## Rows over the run (largest 40)

| phase | outcome | class | seconds | tries |
|---|---|---|---:|---:|
| read | no cached reading | productive | 30.3 | 1 |
| afford | no plan, sentence | productive | 16.4 | 1 |
| afford | no plan, sentence | interface, no yield | 16.1 | 1 |
| wait | horizon | wait | 0.1 | 1 |
| plan | plan | productive | 0.1 | 1 |
| wait | static | wait | 0.1 | 1 |
| wait | horizon | productive | 0.1 | 1 |
| read | reused (e009 keys) | no plan | 0.1 | 2 |
| return | same | overhead | 0.0 | 3 |
| T |  | no plan | 0.0 | 2 |
| T |  | productive | 0.0 | 1 |
| play | ready: the first end point in the order of tries; goal | productive | 0.0 | 1 |
| rest |  | overhead | -0.0 | 3 |

## Tries by stop and class

| stop | try's class | tries | wall s |
|---|---|---:|---:|
| goal | productive | 1 | 30.5 |
| none played: no target | no plan | 1 | 16.2 |
| none played: no target | productive | 1 | 16.5 |

## Shift

now: null
changes: []
cells the cell rule names: []

## Tries by stop and progress (a new atom of T or a new room)

| stop | progress | tries | wall s | productive s |
|---|---|---:|---:|---:|
| goal | progress | 1 | 30.5 | 30.4 |
| none played: no target | none | 1 | 16.2 | 0.0 |
| none played: no target | progress | 1 | 16.5 | 16.4 |

## The shift and the cell rule replayed over the commits

- the shift on after 0 of 3 commits; changes []
- the largest last-20 share of each non-productive class (the shift's threshold is 50%): no plan 0.1% after commit 1 (j00001); interface, no yield 25.5% after commit 2 (j00002); wait 0.3% after commit 2 (j00002); overhead 0.1% after commit 1 (j00001)
- tries the cell rule would have made free (0 of 3; the cells as they are now; [commit, job, the try's class]): []
- cost: the table from the files 0.050 s (16.81 ms per try), its JSON 0.2 ms

## Checks

- PASS every try's rows sum to its wall: 3 tries, largest difference 0.00e0 s
- PASS the rows' total is the tries' wall: rows 63.200 s, wall 63.200 s
- PASS the rest row is at most 1% (signed and absolute): rest -0.025 s (-0.039%), absolute 0.063 s (0.100%)
- PASS the new entries give back each line's new_t and new_rooms: 3 of 3 lines with E012's records; failing: []
