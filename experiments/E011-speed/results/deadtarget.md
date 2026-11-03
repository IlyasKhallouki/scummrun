# Targets with no plan made cheap: results (2026-11-03)

plan.md, amendment "targets with no plan made cheap". Branch e011-deadtarget. Laptop, seed 1, nice 10, at load 3-15
(the kitchen check, the integrator's round 2 and these runs together). Tools: `experiments/E011-speed/deadtarget/`
(requests.py, gate.sh, report.py, deepen_check.py), `taskcore dead`, `fmi-plan seq --dead on|--proof-only 1`.

## The switches

| switch | what | off (default) |
|---|---|---|
| `FMI_PLAN_DEAD=1` | h^1 then h^2 on the request's SAS task before the search; a goal outside either fixpoint is "unsolvable", source "proof" | the old planner |
| `FMI_PLAN_DEAD_S` | h^2's watchdog (60 s) | |
| `FMI_PLAN_DEEPEN=L0` | a target deferred n times is planned with L0 · 2^n until its attempt has a plan; a timeout defers it per entry, never "tried" | the old limit for every plan |

## Gates

| gate | result |
|---|---|
| 1. recorded requests with a plan, rebuilt, proofs on | 41 distinct requests (103 records: the check 25, B0 16, B3 60, E008 2) and E008's serve38 (6) and serve33 (7); 319 recorded plans had their reading evicted or gone. No proof claims no plan on any request with a plan; each of the 40 requests with a plan returns the recorded plan (the step names in order) of every record (101); serve38 and serve33 give the same bytes with the proofs on and off |
| 2. a found plan's states inside the h^2 rows | 53 of 53 found plans |
| 3. the fast h^2 equals the plain one | 53 of 53 tasks (10 k to 155 k operators); 20,000 random tasks (with every reachable state's pairs inside, and no claim on a solvable one) |
| 4. both switches off | G9: 18 scenarios × 2 modes, 0 differing; fmi-plan's dumps byte for byte (gate 1's serve); cargo tests of taskcore, fmi-plan, fmi-explore pass |
| 5. deepening on fake steps (deepen_check.py) | pass: limits L0 · 2^n; deferred targets last; a 15 s target found at 20 s after one deferral; the one with no plan deferred twice and never tried; no entry with deferred targets exhausted; a killed and resumed run writes the same log lines |

## The check's bit:118 = 1 requests

Tries 15 (entry 36, frame 2771) and 16 (entry 26, frame 2770) give the same start file, caps and goal: one task, 154,661
operators, 7,164 facts, on the refinement reading j00000/refine-01.

| | request wall |
|---|---:|
| before, 600 s limit (the check) | 607 s, 612 s |
| before, 180 s limit (the check, resumed) | 191 s, 187 s |
| after, proofs on (proof-only replay) | 18.4 s: reductions and build 13.9 s, h^1 0.3 s, h^2 4.5 s |
| after, proofs on (the gate's replay, check mode) | 19.1 s |

What proves it: h^2. The goal fact bit:118 = 1 itself is outside the pair fixpoint; h^1 reaches it. Each of the 8,712
effects that write it needs, with its operator's preconditions and its conditions, a pair that h^2 shows no reachable
state holds (`taskcore dead --explain`, the first such pair of each, values as SAS indices):

| pair outside h^2 | writers |
|---|---:|
| bit:386 = 0 with var:252 = 1 | 2,952 |
| bit:387 = 0 with var:253 = 1 | 2,016 |
| bit:387 = 1 with var:252 = 1 | 936 |
| bit:388 = 0 with var:254 = 1 | 864 |
| bit:387 = 1 with var:257 = 1 | 648 |
| bit:390 = 0 with var:256 = 1 | 432 |
| bit:388 = 1 with var:252 = 1 | 288 |
| bit:388 = 1 with var:257 = 1, bit:391 = 0 with var:257 = 1 | 216 each |
| bit:388 = 1 with var:253 = 1, bit:390 = 1 with var:252 = 1 | 72 each |

On ring(38) the same two kinds cover all 216 writers (bit:388 = 0 with var:254 = 1, bit:390 = 0 with var:256 = 1).

## How often the proofs prove a target dead

| set | targets | h^1 | h^2 |
|---|---:|---:|---:|
| every target at entry 36 (try 15's 89) | 89 | 0 | 1 (bit:118 = 1) |
| every target at entry 26 (try 16's 89) | 89 | 0 | 1 (bit:118 = 1) |
| the check's rebuildable requests | 24 distinct | 0 | 1 (bit:118 = 1) |
| the profile's ring(38) survey's 30 s time-outs (bit:118 = 1, var:53 = -1, var:52 = -1) | 3 | 0 | 2 (var:52 = -1 open) |

Whether the other targets have plans is known only where a try found one.

## The proofs' cost

h^2 stops as soon as the goal is in its rows (rows only grow, so nothing can be proven after that).

| requests | n | proofs, median | max | reductions and build, median |
|---|---:|---:|---:|---:|
| ring(33) with a plan | 29 | 0.38 s | 1.4 s | 13.8 s |
| ring(38) with a plan | 24 | 0.011 s | 0.019 s | 0.23 s |
| bit:118 (dead) | 1 | 4.5 s | | 13.9 s |

The whole h^2 fixpoint (the checks): 2.6-7.3 s on ring(33), 0.02-0.07 s on ring(38).

## The first limit L0

Fast Downward's search seconds of the 26 found plans with FD's search on the warm path (fmi-plan, gate 1):

1.4, 1.8, 1.9, 2.0, 2.3, 2.5, 2.6, 2.8, 2.8, 3.5, 13.4, 15.1, 37.3, 39.2, 39.6, 39.8, 46.1, 53.0, 88.9, 95.8, 96.3,
108.3, 110.2, 111.6, 114.5, 119.8

Median 37.3 s, p90 111.6 s, max 119.8 s. L0 = 120 s (`FMI_PLAN_DEEPEN=120`). The nine above 80 s are the check's
requests on the refinement reading (tries 15, 16, 18), planned at this load.

## Time per dead target, before and after

- Proven dead (bit:118): 18-19 s, against 187-191 s with the 180 s limit and 607-612 s with 600 s.
- Dead and not proven: L0 + the reductions in its first round (about 134 s at L0 = 120), then 2 L0, 4 L0, ... in
  later tries, each only once the entry has no other target left. Before: the old limit once, then never again.

## Notes

- A try that only defers targets makes no new reached values, so it counts toward E008's stagnation stop; the stop
  rules are unchanged.
- The run uses the proofs with `FMI_PLAN_DEAD=1` and the deepening with `FMI_PLAN_DEEPEN=120`; both are off by default.
