# Stalls: /var/tmp/fast-mi/e014-archives/play4

37 tries, wall 9452.8 s, rows 9452.8 s

## the run (37 tries, 9452.8 s)

| class | seconds | share |
|---|---:|---:|
| productive | 431.3 | 4.6% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.1 | 0.0% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 3357.9 | 35.5% |
| model, no yield | 0.0 | 0.0% |
| wait | 26.6 | 0.3% |
| overhead | 236.9 | 2.5% |
| reading bound | 5400.0 | 57.1% |

rest: 236.36 s (2.50%), absolute 236.65 s (2.50%)

## the last 20 (20 tries, 5626.2 s)

| class | seconds | share |
|---|---:|---:|
| productive | 0.0 | 0.0% |
| model-blind | 0.0 | 0.0% |
| no plan | 0.0 | 0.0% |
| refused input | 0.0 | 0.0% |
| effect mismatch | 0.0 | 0.0% |
| interface, no yield | 2071.6 | 36.8% |
| model, no yield | 0.0 | 0.0% |
| wait | 18.7 | 0.3% |
| overhead | 236.0 | 4.2% |
| reading bound | 3300.0 | 58.7% |

rest: 235.62 s (4.19%), absolute 235.70 s (4.19%)

## Rows over the run (largest 40)

| phase | outcome | class | seconds | tries |
|---|---|---|---:|---:|
| read | bound | reading bound | 5400.0 | 18 |
| free | entry, level 1: taken | interface, no yield | 2532.1 | 30 |
| free | delays | interface, no yield | 554.2 | 30 |
| rest |  | overhead | 236.4 | 37 |
| free | stop, level 1: taken | interface, no yield | 210.3 | 3 |
| free | kept, level 2: taken | productive | 143.9 | 3 |
| free | entry, level 1: taken | productive | 111.3 | 3 |
| read | no cached reading | productive | 86.5 | 2 |
| free | delays | productive | 66.1 | 3 |
| free | entry, level 1: other | interface, no yield | 38.1 | 30 |
| wait | static | wait | 24.8 | 32 |
| afford | no plan: taken | interface, no yield | 18.8 | 1 |
| afford | no plan: taken | productive | 18.8 | 1 |
| free | stop, level 1: other | interface, no yield | 3.4 | 3 |
| free | kept, level 2: other | productive | 2.1 | 3 |
| free | entry, level 1: other | productive | 2.0 | 3 |
| wait | horizon | wait | 1.8 | 4 |
| return | same | overhead | 0.5 | 37 |
| T |  | interface, no yield | 0.5 | 31 |
| free | entry, level 1: none | interface, no yield | 0.2 | 11 |
| afford | no plan: other | interface, no yield | 0.2 | 1 |
| afford | no plan: other | productive | 0.2 | 1 |
| wait | horizon | productive | 0.2 | 1 |
| plan | plan | productive | 0.1 | 1 |
| read | reused (e009 keys) | no plan | 0.1 | 2 |
| free | kept, level 2: none | productive | 0.0 | 3 |
| free | entry, level 1: none | productive | 0.0 | 2 |
| free | stop, level 1: none | interface, no yield | 0.0 | 2 |
| free | kept, level 2: dropped: gate | refused input | 0.0 | 1 |
| T |  | productive | 0.0 | 4 |
| afford | no plan: none | productive | 0.0 | 1 |
| free | entry, level 1: ignored | refused input | 0.0 | 2 |
| afford | no plan: none | interface, no yield | 0.0 | 1 |
| T |  | no plan | 0.0 | 2 |
| free | other | interface, no yield | 0.0 | 30 |
| free | other | productive | 0.0 | 3 |
| play | ready: the first end point in the order of tries; goal | productive | 0.0 | 1 |
| read | T only: differs on N keys | interface, no yield | 0.0 | 13 |
| read | reused (e009 keys) | interface, no yield | 0.0 | 2 |
| read | stand-in | productive | 0.0 | 2 |

## Tries by stop and class

| stop | try's class | tries | wall s |
|---|---|---:|---:|
| error | overhead | 1 | 534.4 |
| free | interface, no yield | 15 | 1899.0 |
| free | productive | 1 | 114.0 |
| goal | productive | 1 | 35.9 |
| none played: no target | no plan | 1 | 19.2 |
| none played: no target | productive | 1 | 19.2 |
| reading bound | productive | 2 | 863.5 |
| reading bound | reading bound | 15 | 5967.6 |

## Shift

now: "reading bound"
changes: [[4,"j00004","on","reading bound",0.502849]]
cells the cell rule names: []
