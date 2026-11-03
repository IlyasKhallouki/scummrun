# Stalls: /var/tmp/fast-mi/e014-pscratch/p6

7 tries, wall 114.7 s, rows 114.7 s

## the run (7 tries, 114.7 s)

| class | seconds | share |
|---|---:|---:|
| productive | 4.4 | 3.9% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.1 | 0.1% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 39.9 | 34.8% |
| model, no yield | 69.6 | 60.7% |
| wait | 0.6 | 0.5% |
| overhead | 0.2 | 0.1% |

rest: 0.10 s (0.09%), absolute 0.20 s (0.17%)

## the last 20 (7 tries, 114.7 s)

| class | seconds | share |
|---|---:|---:|
| productive | 4.4 | 3.9% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.1 | 0.1% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 39.9 | 34.8% |
| model, no yield | 69.6 | 60.7% |
| wait | 0.6 | 0.5% |
| overhead | 0.2 | 0.1% |

rest: 0.10 s (0.09%), absolute 0.20 s (0.17%)

## Rows over the run (largest 40)

| phase | outcome | class | seconds | tries |
|---|---|---|---:|---:|
| afford | no plan, sentence | interface, no yield | 39.9 | 4 |
| read | no cached reading | model, no yield | 34.8 | 1 |
| read | differs on N keys | model, no yield | 34.7 | 1 |
| afford | no plan, sentence | productive | 4.4 | 1 |
| wait | static | wait | 0.4 | 5 |
| wait | horizon | wait | 0.2 | 2 |
| rest |  | overhead | 0.1 | 7 |
| plan | plan | model, no yield | 0.1 | 2 |
| read | reused (e009 keys) | no plan | 0.1 | 2 |
| return | same | overhead | 0.1 | 7 |
| T |  | model, no yield | 0.0 | 2 |
| T |  | no plan | 0.0 | 5 |
| play | ready: the first end point in the order of tries; goal | model, no yield | 0.0 | 2 |
| read | reused | no plan | 0.0 | 3 |

## Tries by stop and class

| stop | try's class | tries | wall s |
|---|---|---:|---:|
| goal | model, no yield | 2 | 69.9 |
| none played: no target | no plan | 4 | 40.3 |
| none played: no target | productive | 1 | 4.5 |

## Shift

now: "model, no yield"
changes: [[0,"j00000","on","model, no yield",0.994315]]
cells the cell rule names: []

## Tries by stop and progress (a new atom of T or a new room)

| stop | progress | tries | wall s | productive s |
|---|---|---:|---:|---:|
| goal | none | 2 | 69.9 | 0.0 |
| none played: no target | none | 4 | 40.3 | 0.0 |
| none played: no target | progress | 1 | 4.5 | 4.4 |

## The shift and the cell rule replayed over the commits

- the shift on after 7 of 7 commits; changes [[0,"j00000","on","model, no yield",0.994315]]
- the largest last-20 share of each non-productive class (the shift's threshold is 50%): no plan 0.1% after commit 1 (j00001); interface, no yield 34.8% after commit 6 (j00006); model, no yield 99.4% after commit 0 (j00000); wait 0.5% after commit 6 (j00006); overhead 0.3% after commit 0 (j00000)
- tries the cell rule would have made free (0 of 7; the cells as they are now; [commit, job, the try's class]): []
- cost: the table from the files 0.009 s (1.25 ms per try), its JSON 0.2 ms

## Checks

- PASS every try's rows sum to its wall: 7 tries, largest difference 0.00e0 s
- PASS the rows' total is the tries' wall: rows 114.700 s, wall 114.700 s
- PASS the rest row is at most 1% (signed and absolute): rest 0.102 s (0.089%), absolute 0.197 s (0.172%)
- PASS the new entries give back each line's new_t and new_rooms: 7 of 7 lines with E012's records; failing: []
