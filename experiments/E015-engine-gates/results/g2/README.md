# E015 G2: novelty over engine-visible state, results

Run on 2026-11-04 on the laptop, per plan.md's G2 section and amendment (G2-1). Archive play8 (read only). Numbers
only; rooms, objects, scripts and units by number.

## Verdict: FAIL as stated

- **t596 is not kept under E.** Its stop is the one the plan names: frame 1256, `local:211` live, `local:211` waiting
  at offset 578. Every atom it holds was already held:
  - **Earlier in the same step.** Trial `v1t596` (seq 15, a verb slot then object 596) stops at frame 1258 in the same
    dialog, `local:211` at 578. It is kept under E for exactly the two atoms `live:49:local:211` and
    `at:49:local:211:578`.
  - **t596 itself** comes at seq 167, because afford's order puts bare targets after the verb-then-target pairs. By
    then nothing in it is new.
- **The kept rate is inside the range.** 97 of 16,026 re-scored trials are kept, 0.61%. The range is [0.5%, 20%].
- **Under the plan's PASS rule** (t596 kept, and the rate in range), G2 fails, so report §9's hypothesis is refuted as
  stated (decision rule 3).
- **What the data show besides.** E does keep local 211's dialog from j00077: one trial of 264, the first to reach it.
  T kept none.

## Calibration

- **G2-0, j00077 from entry 184.** Met at the first full run after the re-runner's fix:
  - 264 trials (180 + 84) and 0 kept under T;
  - t596 at frame 1256 with `local:211` live;
  - 20 s.
- **Fixes before G2-0 was met.** The reading is not on the laptop, so the first re-run, with the wait's domains taken
  from all of T, waited differently. It made 10 wait stops where the run had 0 and 2,860 trials. The second fix took
  the offer points from the recorded wait and gave domains only to T's own keys, the verbs and the goal. This is
  amendment (G2-1).
- **The sample.** Every 8th of the 317 steps with trials, from j00003 (40 steps): **40 of 40 reproduce** their trial
  count and their T-kept count, and 0 are excluded. All 41 recorded initial waits match the re-run's stop frames.
  See `calibration.tsv`.
- **The seen set.** 628 entries were booted from their movies; 0 differ from their stored checkpoints. That took
  185 s of boots in all.

## Numbers (j00077 + 40 sampled steps, 16,026 trials)

| | kept | rate |
|---|---|---|
| T (as the run judged) | 21 | 0.13% |
| E = T ∪ U ∪ W ∪ R ∪ C | 97 | 0.61% |
| T ∪ U ∪ W | 97 | 0.61% |
| U alone | 26 | 0.16% |
| W alone | 76 | 0.47% |
| R alone | 0 | 0 |
| C alone | 0 | 0 |
| sensitivity rule (first offset per unit per cell) | 769 | 4.80% |

- **Attribution** (of the 97 kept under E, those with a novel atom of each kind): T 21, U 26, W 76, R 0, C 0.
- **By room.**
  - Room 49: 9,281 trials, 37 kept under E (0.40%), 21 under T.
  - Room 42: 6,745 trials, 60 kept under E (0.89%), 0 under T.
- **Per step.** Median rate 0.36%, highest 4.8% (j00003, 21 of 436, all through T).
  - 13 steps keep 0, and 14 keep 1.
  - Each of the 12 room-42 steps keeps 5.
- **j00077 alone:** 2 of 264.
  - `v1t596`: `live:49:local:211` and `at:49:local:211:578`.
  - `v12t-2t596`: `at:49:local:206:117`.
- **Ambient units at entry 184.** `local:209`, `script:151`, `script:75` and `script:86`. The job's recorded idle keys
  name `local:209`, `script:75` and `script:86`.
- **No kept trial had shared a wait** (E013's digest dedupe): 0 of 97.

## D1: IW(1) under E from 25 entries

- **The entries.** Every 13th room-49 entry (0, 13, ..., 286, 615), plus 184.
- **The limits.** Depth 5, 4,000 trials, 120 s. Per entry in `depth/ID.json`.
- **Room 63 within the limits:** 2 of 25.
  - Entry 184: `local:211` kept at depth 1, room 63 at depth 2.
  - Entry 182: `local:211` kept at depth 2, room 63 at depth 3.
- **The other 23 did not reach room 63.**
  - 21 stopped with no kept stop left, after 37-864 trials.
  - 0 and 13 hit the trial limit (4,063 and 4,241 trials) at depth 3, with 18 and 8 nodes.
- **The 120 s limit never bound.** The longest search took 22 s.
- **What stopped them.** The seen set (every earlier entry of play8) already holds the camera buckets and the
  reachable targets. A walk click's stop is new under no kind, so the search never scrolls left.

## D2: ground truth's route A under E, T∪U∪W and T

From entry 0, ev5's 5 inputs. Each cell is kept or not, with the novel atom count:

| input | stop | room | xstart | E | T∪U∪W | T |
|---|---|---|---|---|---|---|
| choice (993) | 1032 | 49 | 256 | kept 27 | kept 27 | kept 21 |
| floor (1040) | 1061 | 49 | 168 | kept 2 | kept 1 | kept 1 |
| floor (1101) | 1137 | 49 | 0 | kept 4 | kept 2 | not |
| object 596 (1201) | 1315 | 49 | 0 | kept 2 | kept 2 | not |
| choice (1320) | 1405 | 63 | 88 | kept 16 | kept 10 | kept 4 |

- **The longest unkept run before room 63:** E 0, T∪U∪W 0, T 2.
- **From entry 184, ev5's last 2 inputs.** The click at (30, 25) also opens local 211's dialog. Room 63 at 1405.
  Longest unkept run: E 0, T∪U∪W 0, T 1.
- **D2's seen set is small.** It holds only the entry and the route's kept stops, so it is much easier than D1's.

## Predictions scored

| prediction | outcome |
|---|---|
| calibration at the first try: 80% | missed: met after two re-runner fixes, within the 45 minutes |
| ≥36 of 40 sampled steps reproduce: 75% | hit: 40 of 40 |
| t596 kept under E: 85% | **missed**: not kept (v1t596, 152 trials earlier, held the same atoms) |
| kept through U at least, given kept: 90% | not scored (t596 not kept); v1t596 was kept through U and W |
| kept rate 1-8%, median 3% | missed: 0.61% |
| kept rate in [0.5%, 20%]: 75% | hit: 0.61% |
| j00077 alone 2-15 of 264 | hit: 2 |
| U or W keep the most | hit: W alone 76, U alone 26, R 0, C 0 |
| G2 passes: 65% | **missed: fails** |
| D1 width 1 (room 63 reached) in ≥80% of entries: 60% | missed: 2 of 25 |
| D1 depth 4-5 from xstart-256 entries, 2 from left-edge entries | 256 entries: never reached; 184: 2 (hit); 182: 3 |
| D2 longest unkept: E 0; T∪U∪W 1-2; T 3 | E 0 hit; T∪U∪W 0 missed; T 2 missed |
| sensitivity rule keeps 1.5-3× | missed: 7.9× (769 vs 97) |

## Reproduction

```
git checkout e015-g2   # worktree /var/tmp/fast-mi/wt-e015-g2
cd experiments/E011-speed/rs
CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015-g2 cargo build --release -p fmi-checks --bin e015-g2
mkdir -p /var/tmp/fast-mi/e015-out/g2
cp experiments/E015-engine-gates/results/g2/scripts/env.sh experiments/E015-engine-gates/results/g2/sample.txt /var/tmp/fast-mi/e015-out/g2/
source /var/tmp/fast-mi/e015-out/g2/env.sh      # play8's ALLON and the game (E014 a8/run-env.sh)
$E15 seen  $ARCH $OUT --ids 0-627               # the seen set's boots
$E15 rerun $ARCH j00077 $OUT                    # G2-0
results/g2/scripts/run-sample.sh                # the 40 sampled steps, 2 hosts
$E15 score $ARCH $OUT j00077 $(cat $OUT/sample.txt)
$E15 route $ARCH $OUT --events /var/tmp/fast-mi/research-scratch/gt/ev5.jsonl --entry 0 --tjob j00077
$E15 route $ARCH $OUT --events /var/tmp/fast-mi/research-scratch/gt/ev5.jsonl --entry 184 --last 2 --tjob j00077
results/g2/scripts/run-d1.sh                    # D1, 2 hosts (--room 63 --unit local:211)
```

Files: `calibration.tsv` (re-run against the record), `steps.tsv` and `score.json` (kept per step, per kind, per room,
the kept trials' novel atoms, t596's row), `depth/` (D1), `route/` (D2).
