# Stalls: /var/tmp/fast-mi/e012-archives/kitchen

53 tries, wall 1658.1 s, rows 1658.1 s

## the run (53 tries, 1658.1 s)

| class | seconds | share |
|---|---:|---:|
| productive | 738.0 | 44.5% |
| model-blind | 376.4 | 22.7% |
| no plan | 0.7 | 0.0% |
| refused input | 109.8 | 6.6% |
| effect mismatch | 10.9 | 0.7% |
| interface, no yield | 362.9 | 21.9% |
| model, no yield | 45.5 | 2.7% |
| wait | 6.0 | 0.4% |
| overhead | 7.9 | 0.5% |

rest: 6.77 s (0.41%), absolute 10.10 s (0.61%)

## the last 20 (20 tries, 772.6 s)

| class | seconds | share |
|---|---:|---:|
| productive | 231.2 | 29.9% |
| model-blind | 282.5 | 36.6% |
| no plan | 0.0 | 0.0% |
| refused input | 21.1 | 2.7% |
| effect mismatch | 0.6 | 0.1% |
| interface, no yield | 204.8 | 26.5% |
| model, no yield | 24.9 | 3.2% |
| wait | 2.7 | 0.3% |
| overhead | 4.9 | 0.6% |

rest: 4.41 s (0.57%), absolute 6.73 s (0.87%)

## Rows over the run (largest 40)

| phase | outcome | class | seconds | tries |
|---|---|---|---:|---:|
| afford | empty window, sentence | productive | 360.6 | 5 |
| afford | no plan, sentence | interface, no yield | 179.0 | 3 |
| afford | empty window, sentence | interface, no yield | 174.9 | 5 |
| read: refinement | added | productive | 160.2 | 5 |
| plan | time | model-blind | 121.4 | 1 |
| plan | plan | productive | 83.0 | 9 |
| read | no cached reading | productive | 75.8 | 4 |
| plan | unsolvable | model-blind | 52.7 | 8 |
| read: refinement | added | model-blind | 52.5 | 21 |
| read | no cached reading | model-blind | 34.6 | 4 |
| read: extension | no replan | model-blind | 32.1 | 5 |
| read | differs on N keys | model-blind | 30.9 | 7 |
| read | differs on N keys | productive | 27.8 | 2 |
| play | N frames; interface | refused input | 24.9 | 7 |
| plan | plan | refused input | 23.1 | 7 |
| plan | plan | model-blind | 21.9 | 22 |
| read | no cached reading | refused input | 21.7 | 2 |
| read: refinement | added | refused input | 19.5 | 3 |
| read: refinement | added | model, no yield | 15.4 | 2 |
| read | differs on N keys | refused input | 15.3 | 3 |
| read | differs on N keys | model, no yield | 12.7 | 2 |
| plan | reach extension | model-blind | 12.5 | 4 |
| plan | plan | model, no yield | 12.2 | 6 |
| afford | R0, dialog | interface, no yield | 8.2 | 16 |
| T |  | productive | 8.1 | 9 |
| read | differs on N keys | effect mismatch | 7.7 | 3 |
| rest |  | overhead | 6.8 | 53 |
| play | N frames; interface | productive | 5.8 | 1 |
| read: extension | replan | model-blind | 5.5 | 1 |
| afford | R0, dialog | productive | 4.9 | 8 |
| play | N frames; spent | model-blind | 4.9 | 1 |
| read: extension | replan | productive | 4.9 | 1 |
| T |  | model-blind | 3.5 | 26 |
| read | reused (e009 keys) | model-blind | 3.4 | 11 |
| wait | horizon | wait | 3.3 | 20 |
| play | N frames; goal, not from the plan | model, no yield | 3.2 | 1 |
| wait | static | wait | 2.7 | 32 |
| plan | plan | effect mismatch | 2.6 | 4 |
| read | reused (e009 keys) | refused input | 2.6 | 2 |
| T |  | refused input | 2.4 | 7 |

## Tries by stop and class

| stop | try's class | tries | wall s |
|---|---|---:|---:|
| goal | model, no yield | 2 | 22.4 |
| goal | productive | 4 | 68.3 |
| goal, not from the plan | model, no yield | 4 | 24.1 |
| none played: no target | no plan | 1 | 1.6 |
| none played: reach extension | model-blind | 3 | 210.8 |
| none played: reach extension | productive | 1 | 8.2 |
| reach extension | model-blind | 1 | 31.7 |
| refinement: Lemma R's R0 | model-blind | 16 | 242.6 |
| refinement: Lemma R's R0 | productive | 8 | 343.0 |
| the same empty window | effect mismatch | 2 | 3.3 |
| the same empty window | productive | 6 | 456.8 |
| the same empty window | refused input | 5 | 245.3 |

## Shift

now: null
changes: []
cells the cell rule names: ["0b3cc652ec6b6b15dfd2c1299f96bf32b6a66b1c","220b725496f739a740e6838c3449aab8cc9ba0f2"]

## Tries by stop and progress (a new atom of T or a new room)

| stop | progress | tries | wall s | productive s |
|---|---|---:|---:|---:|
| goal | none | 2 | 22.4 | 0.0 |
| goal | progress | 4 | 68.3 | 67.3 |
| goal, not from the plan | none | 4 | 24.1 | 0.0 |
| none played: no target | none | 1 | 1.6 | 0.0 |
| none played: reach extension | none | 3 | 210.8 | 0.0 |
| none played: reach extension | progress | 1 | 8.2 | 0.1 |
| reach extension | none | 1 | 31.7 | 0.0 |
| refinement: Lemma R's R0 | none | 16 | 242.6 | 0.0 |
| refinement: Lemma R's R0 | progress | 8 | 343.0 | 263.7 |
| the same empty window | none | 7 | 248.6 | 0.0 |
| the same empty window | progress | 6 | 456.8 | 406.9 |

## The shift and the cell rule replayed over the commits

- the shift on after 0 of 53 commits; changes []
- the largest last-20 share of each non-productive class (the shift's threshold is 50%): model-blind 36.6% after commit 52 (j00052); no plan 0.2% after commit 26 (j00027); refused input 15.4% after commit 35 (j00036); effect mismatch 2.6% after commit 21 (j00022); interface, no yield 41.6% after commit 42 (j00040); model, no yield 8.2% after commit 41 (j00042); wait 0.8% after commit 14 (j00015); overhead 0.7% after commit 44 (j00044)
- tries the cell rule would have made free (0 of 53; the cells as they are now; [commit, job, the try's class]): []
- cost: the table from the files 0.624 s (11.77 ms per try), its JSON 2.4 ms

## Checks

- PASS every try's rows sum to its wall: 53 tries, largest difference 0.00e0 s
- PASS the rows' total is the tries' wall: rows 1658.100 s, wall 1658.100 s
- PASS each try's rows sum to its wall in report.json: 53 of 53 tries; report 53 tries; 
- PASS the rows' total is report.json's: rows 1658.100 s, report 1658.100 s
- PASS the rest row is at most 1% (signed and absolute): rest 6.774 s (0.409%), absolute 10.103 s (0.609%)
- PASS the new entries give back each line's new_t and new_rooms: 53 of 53 lines with E012's records; failing: []
