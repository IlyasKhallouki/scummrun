# E012 try: B3's 15 tries, every E012 switch off and on, no reading cache (fmi check try-report)

CPU seconds by part, wall seconds per try. Laptop, seed 1, nice 10, load 3.0-3.2, one resident fmi per sequence under the heavy lock. Binary 73ff54e0eefe (e012-try 73477f2). Off: ON2 and every E012 switch unset. On: ON3, every E012 switch on, end's engine.

### off-fast-nc

| try | entry | wall s | return | read | idle+wait | plan | play | afford | stop | new (how) | wait stops/kept | delay trig/cand/kept | afford trials/kept | dead |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|---|---|---|---|---|
| j00000 | 0 | 120.1 | 0.0 | 172.5 | 0.0 | 14.6 | 0.0 | 0.0 | refinement | 4 {"checkpoint":3,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00001 | 4 | 3.9 | 0.0 | 2.5 | 0.1 | 0.3 | 0.0 | 0.0 | refinement | 1 {"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00002 | 3 | 4.5 | 0.0 | 4.0 | 0.1 | 0.3 | 0.0 | 0.0 | refinement | 2 {"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00003 | 2 | 5.1 | 0.0 | 4.8 | 0.0 | 0.3 | 0.0 | 0.0 | refinement | 2 {"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00004 | 6 | 2.0 | 0.0 | 4.2 | 0.1 | 0.2 | 0.0 | 0.0 | refinement | 2 {"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00005 | 3 | 1.8 | 0.0 | 4.1 | 0.0 | 0.1 | 0.0 | 0.0 | refinement | 2 {"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00006 | 1 | 30.6 | 0.0 | 55.6 | 0.1 | 8.3 | 0.0 | 0.0 | refinement | 4 {"checkpoint":3,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00007 | 15 | 2.0 | 0.0 | 4.4 | 0.0 | 0.2 | 0.0 | 0.0 | refinement | 2 {"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00008 | 14 | 27.3 | 0.0 | 55.5 | 0.1 | 5.5 | 0.0 | 0.0 | refinement | 4 {"checkpoint":3,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00009 | 1 | 27.5 | 0.0 | 57.1 | 0.0 | 5.1 | 0.0 | 0.0 | refinement | 4 {"checkpoint":3,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00010 | 21 | 2.1 | 0.0 | 4.5 | 0.0 | 0.2 | 0.0 | 0.0 | refinement | 2 {"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00011 | 20 | 28.5 | 0.0 | 58.9 | 0.0 | 5.0 | 0.0 | 0.0 | refinement | 4 {"checkpoint":3,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00012 | 14 | 21.6 | 0.0 | 57.6 | 0.0 | 0.1 | 0.0 | 0.0 | refinement | 4 {"checkpoint":3,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00013 | 31 | 2.1 | 0.0 | 4.4 | 0.0 | 0.2 | 0.0 | 0.0 | refinement | 2 {"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| j00014 | 30 | 28.3 | 0.0 | 57.8 | 0.0 | 5.1 | 0.0 | 0.0 | refinement | 4 {"checkpoint":3,"end":1} | 0/0 | 0/0/0 | 0/0 | 0 |
| sum | | 307.4 | 0.0 | 547.9 | 0.5 | 45.5 | 0.0 | 0.0 | | | 0/0 | 0/0/0 | 0 steps, 0/0 | 0 |

### on-fast-nc

| try | entry | wall s | return | read | idle+wait | plan | play | afford | stop | new (how) | wait stops/kept | delay trig/cand/kept | afford trials/kept | dead |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|---|---|---|---|---|
| j00000 | 0 | 124.0 | 0.0 | 173.0 | 0.0 | 14.9 | 0.0 | 0.1 | refinement | 5 {"afford":1,"checkpoint":3,"end":1} | 0/0 | 0/0/0 | 4/1 | 0 |
| j00001 | 4 | 4.2 | 0.0 | 2.8 | 0.1 | 0.3 | 0.0 | 0.1 | refinement | 2 {"afford":1,"end":1} | 0/0 | 0/0/0 | 4/1 | 0 |
| j00002 | 3 | 5.2 | 0.0 | 4.6 | 0.1 | 0.3 | 0.0 | 0.1 | refinement | 5 {"afford":1,"checkpoint":1,"end":1,"wait":2} | 2/2 | 0/0/0 | 4/1 | 0 |
| j00003 | 2 | 5.2 | 0.0 | 4.4 | 0.1 | 0.3 | 0.0 | 0.1 | refinement | 3 {"afford":1,"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 4/1 | 0 |
| j00004 | 6 | 2.4 | 0.0 | 4.5 | 0.1 | 0.2 | 0.0 | 0.1 | refinement | 5 {"afford":1,"checkpoint":1,"end":1,"wait":2} | 2/2 | 0/0/0 | 4/1 | 0 |
| j00005 | 3 | 2.5 | 0.0 | 4.5 | 0.1 | 0.2 | 0.0 | 0.1 | refinement | 5 {"afford":1,"checkpoint":1,"end":1,"wait":2} | 2/2 | 0/0/0 | 4/1 | 0 |
| j00006 | 1 | 32.9 | 0.0 | 56.3 | 0.1 | 8.4 | 0.0 | 0.1 | refinement | 7 {"afford":1,"checkpoint":3,"end":1,"wait":2} | 2/2 | 0/0/0 | 4/1 | 0 |
| j00007 | 15 | 2.2 | 0.0 | 4.1 | 0.1 | 0.2 | 0.0 | 0.1 | refinement | 3 {"afford":1,"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 4/1 | 0 |
| j00008 | 14 | 28.9 | 0.0 | 57.1 | 0.1 | 5.9 | 0.0 | 0.1 | refinement | 7 {"afford":1,"checkpoint":3,"end":1,"wait":2} | 2/2 | 0/0/0 | 4/1 | 0 |
| j00009 | 1 | 27.6 | 0.0 | 55.1 | 0.1 | 5.4 | 0.0 | 0.1 | refinement | 7 {"afford":1,"checkpoint":3,"end":1,"wait":2} | 2/2 | 0/0/0 | 4/1 | 0 |
| j00010 | 21 | 2.8 | 0.0 | 5.1 | 0.1 | 0.2 | 0.0 | 0.1 | refinement | 3 {"afford":1,"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 4/1 | 0 |
| j00011 | 20 | 28.2 | 0.0 | 57.5 | 0.1 | 5.2 | 0.0 | 0.1 | refinement | 7 {"afford":1,"checkpoint":3,"end":1,"wait":2} | 2/2 | 0/0/0 | 4/1 | 0 |
| j00012 | 14 | 22.3 | 0.0 | 57.4 | 0.1 | 0.1 | 0.0 | 0.1 | refinement | 7 {"afford":1,"checkpoint":3,"end":1,"wait":2} | 2/2 | 0/0/0 | 4/1 | 0 |
| j00013 | 31 | 2.5 | 0.0 | 4.4 | 0.1 | 0.2 | 0.0 | 0.1 | refinement | 3 {"afford":1,"checkpoint":1,"end":1} | 0/0 | 0/0/0 | 4/1 | 0 |
| j00014 | 30 | 29.9 | 0.0 | 58.5 | 0.1 | 6.3 | 0.0 | 0.2 | refinement | 7 {"afford":1,"checkpoint":3,"end":1,"wait":2} | 2/2 | 0/0/0 | 4/1 | 0 |
| sum | | 320.8 | 0.0 | 549.3 | 1.4 | 48.1 | 0.0 | 1.6 | | | 18/18 | 0/0/0 | 15 steps, 60/15 | 0 |
