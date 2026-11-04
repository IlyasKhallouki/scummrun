# Stalls: /var/tmp/fast-mi/e014-archives/play8

502 tries, wall 52117.3 s, rows 52117.3 s

## the run (502 tries, 52117.3 s)

| class | seconds | share |
|---|---:|---:|
| productive | 829.5 | 1.6% |
| model-blind | 997.8 | 1.9% |
| no plan | 312.7 | 0.6% |
| refused input | 2366.1 | 4.5% |
| effect mismatch | 158.6 | 0.3% |
| interface, no yield | 10297.6 | 19.8% |
| model, no yield | 1020.3 | 2.0% |
| wait | 62.4 | 0.1% |
| overhead | 2172.3 | 4.2% |
| reading bound | 33900.0 | 65.0% |

rest: 2167.81 s (4.16%), absolute 2175.92 s (4.17%)

## the last 20 (20 tries, 2178.1 s)

| class | seconds | share |
|---|---:|---:|
| productive | 29.2 | 1.3% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.0 | 0.0% |
| refused input | 741.4 | 34.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 243.4 | 11.2% |
| model, no yield | 0.0 | 0.0% |
| wait | 1.3 | 0.1% |
| overhead | 862.9 | 39.6% |
| reading bound | 300.0 | 13.8% |

rest: 862.83 s (39.61%), absolute 862.97 s (39.62%)

## Rows over the run (largest 40)

| phase | outcome | class | seconds | tries |
|---|---|---|---:|---:|
| read | bound | reading bound | 33900.0 | 177 |
| free | entry, level 1: taken | interface, no yield | 4319.8 | 309 |
| free | delays | interface, no yield | 2401.5 | 309 |
| rest |  | overhead | 2167.8 | 502 |
| free | stop, level 1: taken | interface, no yield | 1979.8 | 113 |
| plan | unsolvable | refused input | 1064.3 | 16 |
| afford | empty window: taken | interface, no yield | 924.4 | 64 |
| plan | plan | model-blind | 568.2 | 5 |
| read | differs on N keys | refused input | 533.8 | 21 |
| plan | plan | refused input | 519.5 | 43 |
| read: refinement | added | model, no yield | 396.1 | 15 |
| plan | plan | model, no yield | 308.8 | 60 |
| plan | plan | productive | 305.0 | 7 |
| plan | reach extension | model-blind | 298.6 | 3 |
| read | no cached reading | interface, no yield | 277.2 | 11 |
| read | no cached reading | model, no yield | 250.3 | 9 |
| read: refinement | added | no plan | 187.8 | 13 |
| free | kept, level 2: taken | productive | 148.3 | 8 |
| afford | no plan: taken | interface, no yield | 132.2 | 10 |
| free | entry, level 1: other | interface, no yield | 114.1 | 309 |
| read | differs on N keys | no plan | 113.0 | 10 |
| play | refused input: no input formed; interface | refused input | 105.7 | 43 |
| afford | empty window: taken | productive | 104.6 | 7 |
| read | differs on N keys | model-blind | 70.6 | 4 |
| play | N frames; spent | effect mismatch | 67.6 | 15 |
| read | no cached reading | refused input | 67.5 | 4 |
| free | entry, level 1: taken | productive | 65.3 | 8 |
| read: extension | no replan | refused input | 61.7 | 2 |
| free | kept, level 2: taken | interface, no yield | 60.0 | 4 |
| read | no cached reading | productive | 55.7 | 1 |
| free | stop, level 1: other | interface, no yield | 52.2 | 113 |
| play | refused input: no input formed; interface | effect mismatch | 50.4 | 11 |
| read: extension | replan | productive | 43.9 | 1 |
| wait | static | wait | 39.8 | 303 |
| read: refinement | added | productive | 38.8 | 4 |
| free | delays | productive | 37.3 | 8 |
| read | no cached reading | model-blind | 36.7 | 1 |
| read | no cached reading | effect mismatch | 33.6 | 1 |
| read | differs on N keys | model, no yield | 24.0 | 8 |
| wait | horizon | wait | 22.7 | 180 |

## Tries by stop and class

| stop | try's class | tries | wall s |
|---|---|---:|---:|
| error | interface, no yield | 3 | 403.8 |
| error | overhead | 13 | 1757.1 |
| free | interface, no yield | 192 | 5995.0 |
| free | productive | 8 | 267.3 |
| free | reading bound | 35 | 11575.4 |
| goal | model, no yield | 43 | 715.8 |
| goal | productive | 3 | 4.9 |
| goal, not from the plan | model, no yield | 17 | 311.4 |
| goal, not from the plan | productive | 3 | 447.4 |
| no plan (the planner proved none) | no plan | 15 | 311.8 |
| none played: no plan | no plan | 3 | 30.3 |
| none played: no target | no plan | 4 | 65.2 |
| none played: reach extension | model-blind | 3 | 344.9 |
| reading bound | productive | 2 | 679.4 |
| reading bound | reading bound | 80 | 24919.0 |
| refinement | model-blind | 2 | 569.4 |
| refinement: Lemma R's R0 | model-blind | 1 | 44.7 |
| refinement: Lemma R's R0 | productive | 2 | 89.9 |
| replan limit | effect mismatch | 2 | 1.9 |
| the same empty window | effect mismatch | 26 | 614.9 |
| the same empty window | productive | 7 | 393.7 |
| the same empty window | refused input | 38 | 2574.1 |

## Shift

now: null
changes: [[25,"j00025","on","no plan",0.519197],[30,"j00034","off","no plan",0.237667],[33,"j00019","on","interface, no yield",0.504506],[68,"j00072","on","model-blind",0.613211],[83,"j00074","off","model-blind",0.275215],[87,"j00089","on","refused input",0.500116],[99,"j00094","off","refused input",0.256707],[103,"j00102","on","model, no yield",0.585915],[119,"j00115","on","interface, no yield",0.631024],[138,"j00138","on","overhead",0.746857],[145,"j00147","on","reading bound",0.581041],[228,"j00223","on","interface, no yield",0.945927],[246,"j00255","on","reading bound",0.693762],[386,"j00381","on","interface, no yield",0.994358],[465,"j00424","on","reading bound",0.693081],[490,"j00497","off","reading bound",0.235738]]
cells the cell rule names: []
