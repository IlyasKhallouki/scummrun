# Stalls: /var/tmp/fast-mi/e013-stall/r2-e12/on/kitchen

4 tries, wall 136.4 s, rows 136.4 s

## the run (4 tries, 136.4 s)

| class | seconds | share |
|---|---:|---:|
| productive | 128.3 | 94.1% |
| model-blind | 7.3 | 5.3% |
| no plan | 0.0 | 0.0% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 0.3 | 0.2% |
| model, no yield | 0.0 | 0.0% |
| wait | 0.2 | 0.2% |
| overhead | 0.4 | 0.3% |

rest: 0.28 s (0.21%), absolute 0.28 s (0.21%)

## the last 20 (4 tries, 136.4 s)

| class | seconds | share |
|---|---:|---:|
| productive | 128.3 | 94.1% |
| model-blind | 7.3 | 5.3% |
| no plan | 0.0 | 0.0% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 0.3 | 0.2% |
| model, no yield | 0.0 | 0.0% |
| wait | 0.2 | 0.2% |
| overhead | 0.4 | 0.3% |

rest: 0.28 s (0.21%), absolute 0.28 s (0.21%)

## Rows over the run (largest 40)

| phase | outcome | class | seconds | tries |
|---|---|---|---:|---:|
| read: refinement | added | productive | 67.7 | 1 |
| read | no cached reading | productive | 41.3 | 1 |
| plan | plan | productive | 16.6 | 1 |
| read: refinement | added | model-blind | 2.3 | 3 |
| read | differs on N keys | model-blind | 2.2 | 2 |
| read | no cached reading | model-blind | 1.8 | 1 |
| T |  | productive | 1.5 | 1 |
| afford | R0, dialog | productive | 0.9 | 3 |
| plan | plan | model-blind | 0.8 | 3 |
| rest |  | overhead | 0.3 | 4 |
| afford | R0, dialog | interface, no yield | 0.3 | 1 |
| play | ready: the first end point in the order of tries; deviation | productive | 0.2 | 1 |
| wait | static | wait | 0.2 | 4 |
| T |  | model-blind | 0.2 | 3 |
| return | same | overhead | 0.1 | 4 |
| play | a needed record refuted in the parent | model-blind | 0.0 | 3 |

## Tries by stop and class

| stop | try's class | tries | wall s |
|---|---|---:|---:|
| refinement: Lemma R's R0 | model-blind | 1 | 3.5 |
| refinement: Lemma R's R0 | productive | 3 | 132.9 |

## Shift

now: null
changes: []
cells the cell rule names: []

## Tries by stop and progress (a new atom of T or a new room)

| stop | progress | tries | wall s | productive s |
|---|---|---:|---:|---:|
| refinement: Lemma R's R0 | none | 1 | 3.5 | 0.0 |
| refinement: Lemma R's R0 | progress | 3 | 132.9 | 128.3 |

## The shift and the cell rule replayed over the commits

- the shift on after 0 of 4 commits; changes []
- the largest last-20 share of each non-productive class (the shift's threshold is 50%): model-blind 5.3% after commit 3 (j00003); interface, no yield 0.2% after commit 1 (j00001); wait 0.2% after commit 3 (j00003); overhead 0.3% after commit 3 (j00003)
- tries the cell rule would have made free (0 of 4; the cells as they are now; [commit, job, the try's class]): []
- cost: the table from the files 0.032 s (8.07 ms per try), its JSON 0.1 ms

## Checks

- PASS every try's rows sum to its wall: 4 tries, largest difference 0.00e0 s
- PASS the rows' total is the tries' wall: rows 136.400 s, wall 136.400 s
- PASS the rest row is at most 1% (signed and absolute): rest 0.282 s (0.206%), absolute 0.282 s (0.207%)
- PASS the new entries give back each line's new_t and new_rooms: 4 of 4 lines with E012's records; failing: []
- PASS the live stalls.json equals this report's table: 8750 bytes live, 8750 bytes offline
