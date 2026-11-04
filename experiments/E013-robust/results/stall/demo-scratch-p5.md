# Stalls: /var/tmp/fast-mi/e014-pscratch/p5

6 tries, wall 109.9 s, rows 109.9 s

## the run (6 tries, 109.9 s)

| class | seconds | share |
|---|---:|---:|
| productive | 4.5 | 4.1% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.1 | 0.1% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 40.1 | 36.5% |
| model, no yield | 64.8 | 59.0% |
| wait | 0.5 | 0.5% |
| overhead | -0.1 | -0.1% |

rest: -0.11 s (-0.10%), absolute 0.15 s (0.14%)

## the last 20 (6 tries, 109.9 s)

| class | seconds | share |
|---|---:|---:|
| productive | 4.5 | 4.1% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.1 | 0.1% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 40.1 | 36.5% |
| model, no yield | 64.8 | 59.0% |
| wait | 0.5 | 0.5% |
| overhead | -0.1 | -0.1% |

rest: -0.11 s (-0.10%), absolute 0.15 s (0.14%)

## Rows over the run (largest 40)

| phase | outcome | class | seconds | tries |
|---|---|---|---:|---:|
| afford | no plan, sentence | interface, no yield | 40.1 | 3 |
| read | differs on N keys | model, no yield | 33.8 | 1 |
| read | no cached reading | model, no yield | 30.9 | 1 |
| afford | no plan, sentence | productive | 4.5 | 1 |
| wait | static | wait | 0.3 | 4 |
| wait | horizon | wait | 0.2 | 2 |
| plan | plan | model, no yield | 0.1 | 2 |
| read | reused (e009 keys) | no plan | 0.1 | 2 |
| return | same | overhead | 0.0 | 6 |
| T |  | no plan | 0.0 | 4 |
| T |  | model, no yield | 0.0 | 2 |
| play | ready: the first end point in the order of tries; goal | model, no yield | 0.0 | 2 |
| read | reused | no plan | 0.0 | 2 |
| rest |  | overhead | -0.1 | 6 |

## Tries by stop and class

| stop | try's class | tries | wall s |
|---|---|---:|---:|
| goal | model, no yield | 2 | 65.0 |
| none played: no target | no plan | 3 | 40.3 |
| none played: no target | productive | 1 | 4.6 |

## Shift

now: "model, no yield"
changes: [[0,"j00000","on","model, no yield",0.996794]]
cells the cell rule names: []

## Tries by stop and progress (a new atom of T or a new room)

| stop | progress | tries | wall s | productive s |
|---|---|---:|---:|---:|
| goal | none | 2 | 65.0 | 0.0 |
| none played: no target | none | 3 | 40.3 | 0.0 |
| none played: no target | progress | 1 | 4.6 | 4.5 |

## The shift and the cell rule replayed over the commits

- the shift on after 6 of 6 commits; changes [[0,"j00000","on","model, no yield",0.996794]]
- the largest last-20 share of each non-productive class (the shift's threshold is 50%): no plan 0.1% after commit 1 (j00001); interface, no yield 36.9% after commit 1 (j00001); model, no yield 99.7% after commit 0 (j00000); wait 0.5% after commit 5 (j00005); overhead 0.1% after commit 0 (j00000)
- tries the cell rule would have made free (0 of 6; the cells as they are now; [commit, job, the try's class]): []
- cost: the table from the files 0.010 s (1.72 ms per try), its JSON 0.3 ms

## Checks

- PASS every try's rows sum to its wall: 6 tries, largest difference 0.00e0 s
- PASS the rows' total is the tries' wall: rows 109.900 s, wall 109.900 s
- PASS the rest row is at most 1% (signed and absolute): rest -0.112 s (-0.102%), absolute 0.154 s (0.140%)
- PASS the new entries give back each line's new_t and new_rooms: 6 of 6 lines with E012's records; failing: []
