# E012 `targets`: one try end to end with the switch off and on (2026-11-03)

Not a gate of plan.md: a check that a value list goes through the harness, the planner and the window search, and
that the entries it makes verify from boot. Scripts: `targets/e2e-setup.sh`, `targets/e2e-run.sh`.

- **The try.** A copy of the kitchen check's j00030 (entry 93, frame 3192, scope (29, 35)) on its reading, j00028's
  refine-01 with var 103 a fact, made again in full (31,807 records, the log's count). Branch e012-targets' `fmi`
  (release), round 2's engine build, ON3 (round 2's switches, the no-plan proofs, deepening at 120 s), `--search fd
  --tail`, laptop at load 7-15. Every new entry is verified from boot in a fresh lean engine (`fmi-engine verify` on
  a two-entry archive, as E011's verify_new.sh makes it).

| | switch off | switch on |
|---|---|---|
| targets (tier 0 / 1 / 2) | 62 (0 / 56 / 6) | 35 (0 / 33 / 2) |
| first target | var:103 = 369 | var:103 at any of 28 values (367, 369-380, 382, 441-453, 464) |
| the planner | 2 plans, a plan of cost 1 each | 3 plans (the last the old tail), cost 1: walk_to(382) |
| the play's stop | goal, 8 frames (3192 to 3200) | "the same empty window 2 times on walk_to(382)", 7,203 frames (3192 to 10395) |
| new entries (frames) | 3 (3195, 3199, 3200) | 3 (3195, 6795, 10395) |
| verified from boot | 3 of 3, 0 differences | 3 of 3, 0 differences |
| try wall, peak RSS | 5.2 s, 384 MB | 12.5 s, 382 MB |

- The switch-off try is the kitchen check's j00030 again: the same target, stop, frames and new entries' frames
  (the check: 11.3 s on round 1's binaries).
- With the switch on the planner took 382 (a walk to object 382), at the same plan cost as 369 (1): among the
  values of equal cost the search's own order chose. The play cannot make that input run ("no run of the click's units starts script:2"), the same failure the kitchen
  check met when it tried var:103 = 382 alone (j00051 and j00052, stopped after 3,618-3,619 frames). The list is then
  tried from this entry, so later tries from entry 93 go to other targets; var:103's other values stay targets from
  the other entries.
- So a value list hands the choice of value to the planner's costs. A value whose input the play cannot make, and
  which the planner picks among the cheapest, takes the key's attempt from that entry. The same empty window twice is one of
  section 4's triggers of the affordance step, which is where E012 acts on it.
