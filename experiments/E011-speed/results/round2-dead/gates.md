# The gates on the merged build (2026-11-03)

plan.md, amendment "after round 2: the no-plan proofs merged into round 2". Branch e011-r2 with e011-deadtarget and
e011 merged, built into `/var/tmp/fast-mi/target-e011-r2` (`~/.cache/fast-mi-e011/env-r2d.sh`). "Everything on" (ON3)
is round 2's ON2 with `FMI_PLAN_DEAD=1` and `FMI_PLAN_DEEPEN=120`; "everything off" (OFF3) is OFF2 with both off.
Laptop, seed 1, nice 10, each heavy step under the heavy lock and the guard. Logs: `/var/tmp/fast-mi/e011-r2dead`.

| gate | everything off | everything on |
|---|---|---|
| cargo tests (taskcore, fmi-core, fmi-reading, fmi-plan, fmi-explore, fmi) | pass (taskcore 19, among them dead.rs's random tasks; the workspace's 80) | |
| G7: taskcore's command line on r38 and r33 | 25 + 28 outputs the old binary's bytes | the same |
| G7: PlanResult = plan_fd's on serve38 and serve33 (gbfs, fd, lama) | 6 + 7 each, same | 6 + 7 each, same, with the proofs on |
| G7: the plan cache, the Rust search = FD's h_FF search (r38) | pass; 9 goals same | pass; 9 goals same (watched h_FF) |
| G7: the Rust lama-first-costs = Fast Downward's (serve38, serve33) | 6 + 7 same | |
| G7: the fast forms in every check mode, gbfs and lama | | 6 + 7 each, same, with the proofs on |
| G9 (fake steps against explore.py, the guide, step, harness, replays, reingests) | pass, 18 scenarios × 2 modes, 0 differing | pass with ON2 and the proofs on (the deepening is gate 5's) |
| G10: test (a), `fmi try`, cold | FD's binary: strict and ready print `CHECK (a): 0 goal True 2728 3216 488 11712 2 36` (501 and 321 children), 8 + 8 entries from boot, 0 differences | the Rust lama-first-costs: strict, ready and ready with the old tail print the same line (501, 321, 321 children), 8 + 8 + 8 entries from boot, 0 differences |
| G11: the 15 tries against B0 | reference settings: stop class and everything = B0's in 15; 43 new entries from boot, 0 differences | reference settings: the same, 43 entries, 0 differences; fast settings: see README.md |
| dead-target gate 1: recorded requests replayed | | 41 requests (103 records); 40 with a recorded plan return it (same steps in all records); no proof claims a request with a plan; bit:118 = 1 (check-j00015) proven dead by h^2 |
| dead-target gate 2: each found plan's states inside the h^2 rows | | 53 of 53 |
| dead-target gate 3: the fast h^2 = the plain fixpoint | | 53 of 53 tasks (the random tasks: cargo tests) |
| dead-target gate 4: serve38 and serve33 with the proofs on and off | | the same bytes (6 + 7 results) |
| dead-target gate 5: deepening on fake steps (deepen_check.py) | | pass |

The replay of gate 1 ran with everything on, so Fast Downward's groups were served by the Rust lama-first-costs: the
same 40 recorded plans, step for step, as Fast Downward's search found them in the recorded runs.
