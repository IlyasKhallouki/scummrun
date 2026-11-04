# E015 G2: independent check

Checked on 2026-11-04 from branch e015-g2 at 67c9fec. I used a separate worktree (`wt-e015-g2-chk`), a separate build
(`CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015-g2-chk`) and a separate output folder (`/var/tmp/fast-mi/e015-out/g2-chk`).
Archive play8 was read only. At most 2 engines ran at once, under nice 10.

## Verdict: reproduced; FAIL holds as stated

Every number in the README recomputes, with one wrong sentence and one imprecise one (issues 1 and 2). Under the plan's
literal PASS rule G2 fails, because the trial t596 is not kept. The failure depends on which trial reached the dialog
first. It does not depend on whether E keeps the dialog: E keeps the dialog's first stop, which T does not. The kept
rate is mostly the same few states found again in each step (issue 4).

## What was re-run, and what matched

- **Seen set.** I booted all 628 entries from their movies. All 628 reach their frames, 0 differ from their stored
  checkpoints, and all 628 views (xstart, live units, offsets, reachable targets) equal the gate's.
- **G2-0.** j00077 from entry 184 gives 264 trials, 0 kept under T and `same: true`, in 24 s. t596 is seq 167 at frame
  1256, with `local:211` live at offset 578. My `trials.jsonl` equals the gate's line for line, apart from timings.
- **Sample.** 40 of 40 steps reproduce their trial count and T-kept count, and 0 are excluded.
- **Score.** My `score.json` equals the gate's, and `steps.tsv` is identical:
  - 16,026 trials;
  - kept under E: 97 (0.61%); under T: 21; under T∪U∪W: 97;
  - each kind alone: U 26, W 76, R 0, C 0;
  - sensitivity rule: 4.80%;
  - room 49: 37 of 9,281; room 42: 60 of 6,745;
  - per step: median 0.36%, maximum 4.82% (j00003); 13 steps keep 0, 14 keep 1, and the 12 room-42 steps keep 5 each.
- **An independent re-score.** I wrote a separate scorer that reuses none of the Rust code. It reads only
  `trials.jsonl`, `seen/*.json`, `summary.json`'s ambient units and `log.jsonl`, and implements the plan's sequential
  rule from the text. It gives the same counts, room splits, median and maximum, and the same two j00077 keeps:
  - `v1t596`: `live:49:local:211` and `at:49:local:211:578`;
  - `v12t-2t596`: `at:49:local:206:117`.
- **D2.** Both route files equal the gate's. Longest unkept run from entry 0: E 0, T∪U∪W 0, T 2. From 184: 0, 0, 1.
- **D1.** All 25 depth files equal the gate's, apart from timings. Room 63 is reached from 184 (depth 2) and 182
  (depth 3). 21 entries run out of kept stops after 37-864 trials; 7 of them keep nothing at depth 1. Entries 0 and 13
  stop at the trial limit (4,063 and 4,241). The whole D1 took 67 s of wall time.

## Did the procedure follow the plan?

- **Changes to plan.md.** The only change after b9fea7f is amendment (G2-1), which is appended. It was committed at
  14:23:41 and 14:24:07. The sample's re-runs finished at about 14:38, and `score.json` was written at 14:46. No PASS or
  FAIL rule, feature or prediction was edited.
- **Feature set.** It matches the plan's table:
  - **U:** K_SCRIPT, K_LOCAL and K_OBJSCRIPT. `facts_of` leaves out the room's entry and exit code (ENCD/EXCD).
  - **W:** K_AT keys at value 1, with entry and exit left out. An ambient unit's offsets are dropped, but its U atom is
    kept.
  - **R:** `offer_cfg(boxes=false, screen=true)`, giving object and actor hits.
  - **C:** `xstart.div_euclid(160)`.
  - **Ambient rule:** a no-input fork run for 1,600 frames; a unit is ambient if it takes 2 or more offset values.
  - No atom carries a frame, a counter or a trial index. The T-cell hash appears only in the sensitivity rule.
- **Selections.** I recomputed both from the archive:
  - **Sample:** 317 steps with trials, 111,626 trials and 39 kept. Every 8th in job-id order gives `sample.txt`
    exactly.
  - **D1:** 312 room-49 entries. Every 13th in id order is 0, 13, ..., 286 and 615 (24 entries), plus 184.
- **Seen set.** The scoring seen set is the root plus the entries in `log.jsonl` lines before the job's own line, which
  is the plan's "committed before the job's commit". Some of these entries were committed while the job was running, so
  the set can be slightly larger than the archive T that the job itself saw. That makes E stricter. It does not affect
  t596, because no archive entry holds `local:211` live (checked over all 628).
- **Hook.** The observer works on a fork. The calibration and all 40 sampled steps reproduce, so it changed no trial.

## Is t596's non-keep an artifact?

No:

- **Same atoms.** t596 (seq 167, frame 1256) and v1t596 (seq 15, frame 1258) share live units, reachable targets,
  xstart and all W offsets except `local:209`'s (133 against 55). `local:209` is ambient at entry 184.
- **Without the ambient rule.** t596 would still be unkept, because `at:49:local:209:133` is held by 207 archive
  entries.
- **The order is the run's.** The retrospective's own re-run (`research-scratch/gt/aff184/trials.tsv`, separate code)
  lists v1t596 at line 16 and t596 at line 168, with identical variable changes. So the retrospective's "the
  model-free step made the right click (t596)" overlooked that v1t596 reached the same dialog 152 trials earlier.
- **t596 alone.** No archive entry holds `live:49:local:211`. Had t596 been the step's first trial into the dialog, E
  would have kept it.

## Issues

1. **"All 41 recorded initial waits match" is wrong. 40 of 41 match.** In j00419, the recorded wait stopped once, at
   frame 9041. The stand-in wait stops 0 times (`summary.json` `wait.same: false` in both the gate's output and mine).
   The trial counts are unaffected, because amendment (G2-1) takes the offer points from the recorded stops.
2. **The README's D2 seen set is described only for entry 0.** "It holds only the entry and the route's kept stops" is
   true from entry 0. From entry 184, `seen_before` also holds entries 0-183.
3. **The criterion names a trial, not a state.** The FAIL is literal and correct. But the reason t596 is unkept is that
   v1t596, which E does keep, holds the same state. As the gate reports, j00077 keeps the `local:211` dialog under E (1
   of 264) and not under T. "Refuted as stated" is right. The stronger evidence against the hypothesis is D1 (2 of 25)
   and issue 4. Whether to restate the criterion is for the user to decide.
4. **The kept rate is mostly the same few states found again in each step.** This is an observation, not part of the
   gate.
   - Of the 76 trials kept under E but not under T, 14 room-49 keeps rest on `at:49:local:206:117`, which is novel
     again in every step. All 60 room-42 keeps rest on the same 11 atoms in every room-42 step.
   - This happens because the seen set is play8's archive, which T built: a stop kept only under E never enters it.
   - A rough estimate carries each step's E-novel atoms into the later sampled steps. It gives about 28 kept (0.17%),
     which is below the 0.5% bound. The estimate is not exact: it carries only the atoms that were novel, and it skips
     7 of every 8 steps.
   - The plan fixes the seen set as the archive, so the gate's 0.61% stands as specified.
5. **D1's chains are not saved.** D1 stores no list of events for the stops that reach room 63, so they can only be
   checked by rerunning the search, which matched. Entry 184's depth-2 result agrees with D2: from a boot of 184,
   ev5's last 2 inputs reach room 63 at frame 1405. Entry 182's depth-3 chain was not checked separately.
6. **Approximations in amendment (G2-1).** Neither changes the result, but both are worth knowing.
   - The waits after each trial use the stand-in domains. These are checked only through the trial and T-kept counts,
     and the amendment states that they are not exact against the recorded `observed` counts.
   - The U and W atoms of wait stops are not added to E's held set; those stops are judged under T only. This can only
     make E keep more, so it cannot explain t596.
