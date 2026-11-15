# Stalls: /var/tmp/fast-mi/e014-archives/play7

80 tries, wall 3429.2 s, rows 3429.2 s

## the run (80 tries, 3429.2 s)

| class | seconds | share |
|---|---:|---:|
| productive | 463.2 | 13.5% |
| model-blind | 482.1 | 14.1% |
| no plan | 343.1 | 10.0% |
| refused input | 17.4 | 0.5% |
| effect mismatch | 1316.6 | 38.4% |
| interface, no yield | 798.3 | 23.3% |
| model, no yield | 0.0 | 0.0% |
| wait | 7.8 | 0.2% |
| overhead | 0.7 | 0.0% |
| reading bound | 0.0 | 0.0% |

rest: 0.39 s (0.01%), absolute 2.95 s (0.09%)

## the last 20 (20 tries, 1759.3 s)

| class | seconds | share |
|---|---:|---:|
| productive | 221.6 | 12.6% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.0 | 0.0% |
| refused input | 0.1 | 0.0% |
| effect mismatch | 1313.9 | 74.7% |
| interface, no yield | 221.8 | 12.6% |
| model, no yield | 0.0 | 0.0% |
| wait | 1.9 | 0.1% |
| overhead | 0.1 | 0.0% |
| reading bound | 0.0 | 0.0% |

rest: 0.09 s (0.01%), absolute 0.55 s (0.03%)

## Rows over the run (largest 40)

| phase | outcome | class | seconds | tries |
|---|---|---|---:|---:|
| plan | plan | effect mismatch | 1131.8 | 11 |
| read | no cached reading | model-blind | 359.9 | 11 |
| free | entry, level 1: taken | interface, no yield | 266.9 | 26 |
| read: refinement | added | no plan | 222.3 | 8 |
| read | no cached reading | interface, no yield | 162.1 | 6 |
| plan | plan | productive | 151.9 | 4 |
| read: refinement | added | effect mismatch | 117.8 | 9 |
| free | delays | interface, no yield | 115.3 | 26 |
| free | stop, level 1: taken | interface, no yield | 98.1 | 7 |
| afford | empty window: taken | interface, no yield | 93.8 | 10 |
| read | differs on N keys | no plan | 84.1 | 5 |
| read | no cached reading | productive | 82.7 | 2 |
| afford | empty window: taken | productive | 56.7 | 5 |
| free | kept, level 2: taken | productive | 53.4 | 3 |
| read: extension | replan | productive | 39.4 | 1 |
| read | differs on N keys | model-blind | 37.9 | 8 |
| afford | no plan: taken | interface, no yield | 36.2 | 4 |
| play | N frames; goal, not from the plan+spent | effect mismatch | 35.8 | 9 |
| free | entry, level 1: taken | productive | 34.8 | 3 |
| read | no cached reading | no plan | 32.6 | 1 |
| read: refinement | added | model-blind | 31.2 | 9 |
| read: refinement | added | productive | 27.7 | 2 |
| read: extension | no replan | effect mismatch | 17.8 | 2 |
| read: extension | no replan | model-blind | 17.6 | 4 |
| read: extension | replan | model-blind | 14.6 | 16 |
| free | kept, level 2: taken | interface, no yield | 12.7 | 1 |
| read: extension | replan | refused input | 12.7 | 1 |
| play | refused input: no input formed; interface | model-blind | 10.7 | 12 |
| plan | unsolvable | effect mismatch | 6.4 | 2 |
| wait | static | wait | 6.3 | 64 |
| read | differs on N keys | effect mismatch | 5.8 | 6 |
| afford | R0: taken | productive | 5.4 | 3 |
| afford | R0: taken | interface, no yield | 5.3 | 2 |
| play | N frames; goal, not from the plan+spent | model-blind | 5.2 | 4 |
| read | differs on N keys | productive | 4.5 | 1 |
| free | entry, level 1: other | interface, no yield | 3.6 | 26 |
| plan | plan | model-blind | 3.3 | 21 |
| afford | no plan: taken | productive | 2.7 | 1 |
| play | refused input: no input formed; interface | refused input | 2.6 | 2 |
| plan | plan | no plan | 1.8 | 8 |

## Tries by stop and class

| stop | try's class | tries | wall s |
|---|---|---:|---:|
| free | interface, no yield | 26 | 663.6 |
| free | productive | 3 | 90.2 |
| goal, not from the plan | productive | 2 | 273.6 |
| no plan (the planner proved none) | no plan | 7 | 308.6 |
| none played: no plan | no plan | 1 | 6.7 |
| none played: reach extension | model-blind | 3 | 36.8 |
| none played: reach extension | productive | 1 | 17.0 |
| planner error | no plan | 1 | 33.2 |
| reach extension | model-blind | 16 | 315.0 |
| refinement: Lemma R's R0 | model-blind | 2 | 65.3 |
| refinement: Lemma R's R0 | productive | 3 | 96.9 |
| the same empty window | effect mismatch | 9 | 1182.9 |
| the same empty window | productive | 5 | 325.4 |
| the same empty window | refused input | 1 | 14.0 |

## Shift

now: "effect mismatch"
changes: [[20,"j00024","on","no plan",0.553222],[33,"j00023","off","no plan",0.299644],[46,"j00049","on","interface, no yield",0.524334],[72,"j00060","off","interface, no yield",0.297065],[73,"j00066","on","effect mismatch",0.506472]]
cells the cell rule names: []
