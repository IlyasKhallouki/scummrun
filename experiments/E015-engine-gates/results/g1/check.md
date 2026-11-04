# E015 G1: independent check

Checked 2026-11-04 from origin/e015-g1 at f2a39f8, in a separate worktree (detached) and a separate build
(`CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015-g1-chk`, release). Outputs in `/var/tmp/fast-mi/e015-out/g1-chk/`.
Same engine (`svm-e013`), the plan's environment, seed 1, one engine host at a time under `nice -n 10`.

**Result: reproduced; the PASS holds.**

## The plan was followed

- `plan.md` is unchanged between the plan's commit b9fea7f and f2a39f8 (`git diff b9fea7f f2a39f8 -- plan.md` is
  empty). The gate's procedure, pass criteria, limits (6, 60, 3, 15) and predictions are the committed ones.
- realize.rs has one commit (6ae5341), so no fix was committed. The code and result commits were all made within 5
  minutes (13:48-13:53), so git alone cannot show that the code came before the run. The record is consistent with "no
  fix", but git does not prove it.
- The routine follows the spec: afford's `offer_cfg`; the approach orders floor points by distance to the object's walk
  point and accepts (Walk touch and xstart toward) or a hit point; candidates (i) bare click, (ii) slot then object,
  (iii) followed by b or by up to 15 slots; up to 3 hit points; accepted on the Sentence touch (v, a, b); the dialog
  slot is found by its dump id and accepted on "taken". The Sentence touch's fields are (a = verb, b = object A,
  old = object B) in fmi-core's `TouchKind`, and the routine compares (a, b, old) with (v, a, b).
- One small extension of the text: the approach also misses with "approach" when no floor point is accepted for a
  click (not only after 6 clicks). This is what (b) hit. It is a natural reading of the spec and changes no gate
  criterion.

## The numbers recompute

| run | committed | this check |
|---|---|---|
| gate inputs | 993 (4,154); 1043 (5,125); 1069 (21,131); 1086 (31,23); 1217 (4,154) | identical (`events.jsonl` byte-equal, two runs) |
| gate steps | d120: 1 cand, 1 fork; s10:596: 2 approach, 1 cand, 3 forks; d120: 1 cand, 1 fork | identical |
| approach seen | w7 walk, xstart 256->136; w22 walk + hit, 136->0 | identical |
| Sentence touch | (10,596,0) | identical |
| room 63 | frame 1289 | 1289 |
| replay from boot | 3,850 facts, 0 differ; room 63/63; digest, tick 5240, rng equal | identical (digest 14423852849303604489) |
| (a) entry 184 | 0 approach, 1 cand, 1 fork, (10,596,0) | identical; also replayed from boot here (`--verify`, not done in the gate): 9 inputs to frame 1156, 3,860 facts, 0 differ, room and digest equal |
| (b) entry 0, s11:593 | miss "approach", 0 cand, 8 forks, all walk=0, xstart 256 | identical |
| E014 probe trace (target-e014e) | room 63 at 1289, tick 5240 | identical (lines equal except `seconds`) |

The records (`realize.json`) equal the committed `gate.json`, `a.json` and `b.json` field by field once the wall
times are left out. The wall times were higher here, under the load of other agents. The gate run took 0.40 s and
0.43 s (0.18 s committed). The sentence with its approach took 15.8 and 21.2 ms (10.3 committed), each dialog verb
2.3-3.7 ms (1.7), (a) 2.9 ms (2.1) and (b) 58.5 ms (29). The scoring does not change: the 0.1-3 s prediction is still
missed by 5-50x.

Predictions: 7 of 9 hit, 2 missed (wall per sentence; room 63's frame 1289 against 1,350-1,600), as scored.

## Room 63 needs all three steps (added test, not in the plan)

| steps realized from entry 0, then run on 600 frames | room 63 |
|---|---|
| d120, s10:596, d120 (the gate) | at 1289 |
| d120, s10:596 (no second dialog choice) | not reached (room 49 at 1687) |
| d120 only | not reached (room 49 at 1594) |
| d120, d120 (no sentence) | the second d120 misses with "no slot": verb 120 is not offered at the idle at 1042 |

So room 63 comes from the second dialog choice. That choice is offered only after the sentence (10,596). The PASS
criterion "room 63 loads after the dialog choice" holds causally, not only in time.

## No profile or verb id

- No caller of `fmi_play::realize` exists outside the measurement binary `e015` and the tests (grep over the
  workspace). Nothing on the run path calls it.
- realize.rs names none of DOS_CD, selected_verb, wait_walk_to, DIALOG_FIRST/LAST or the executor. Its numeric
  literals are the limits, the screen's half width (`afford::SCREEN_W / 2`), the switch's "1", 0/1 counters, and tuple
  indices. The numbers 120, 10, 596 and 593 appear only as the binary's arguments.
- The bare click's verb 10 came from the engine's Sentence touch. Nothing names it.

## Tests (switch off)

`cargo test --release -p fmi-play -p fmi-explore` with FMI_REALIZE unset, in the check build:
- fmi-explore: 47 unit tests, then afford 12, archive 28, archive_progress 15, bench_start 1, free 6, free_explore 5,
  grow 3, guide 11, stall 5, step 26, targets_per_key 11;
- fmi-play: 22 unit tests (1 ignored), realize 8, realize_off 1.

There were 0 failures, and the counts match tests.md. No existing test file changed since b9fea7f. In source, only
afford.rs changed (the `object_grid` refactor), along with fmi-play's lib.rs line and the new realize.rs.

## Issues (none changes the verdict)

1. **"No fix" cannot be shown from git**, because code and results were committed together after the run (see above).
   The two identical reruns show that the committed code gives the committed result.
2. **Only the bare-click candidate was exercised on the demo.** The gate, (a) and (b) never reached candidates (ii)
   or (iii) on the real engine. The first floor point was accepted at each approach click, so the ordering was never
   tested against a rejection either. The demo's evidence for the routine is 3 accepted forks on one route.
3. **(b) does not test a hidden object.** Its 8 forks all failed to walk because the opening dialog is up at entry 0,
   as the write-up says. Whether the routine misses within bounds for an object that is truly unreachable from a
   free-walking state is untested.
4. **The dialog steps are given as verb ids by the gate's arguments.** This is allowed by the plan: a dialog choice is
   the planned value of `var:165`, and a v5 slot's id is its verb. But the realized chain depends on the planner
   naming 120 twice. The second 120 is the one a plan for `var:165 = 120` would give, and the first is play8's choice.
5. The plan's common section names `CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015`, while the gate used
   `target-e015-g1`. This is cosmetic.
