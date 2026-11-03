# E012 `delay`, gates R1-R3 and G0 for its switch (2026-11-03)

Branch e012-delay. Method: plan.md section 2 and the amendments "component delay" (1) and (2).

- **Code:**
  - `fmi-play/src/delay.rs`: switch FMI_DELAY_FORK, off by default. The bound K(p), the draws of a window and its
    p, the fork's rules (`Fork::take`, `Fork::refute`), the outcome digest, the candidates, the random calls per
    frame (xorshift32 stepped from one Status `rng` to the next), R2's χ².
  - `fmi-play/src/window.rs`: the fork in the ready search (`Ready.delay`, `resolve_tries`, `Ready::delay_refute`,
    resolve_final's fallback), the end dump of a try that is not an end point, the record's `"delay"`, the summary's
    `"delay"`, the hooks `draws_of`, `novel_of`, `tested_keys` and the test setting `delay_force_p`.
  - `fmi-core`: `Draw` (the amendment's interface).
  - Tests: `delay.rs` (K, the random steps, p, the rules, χ², R2), `delay_fixture.rs` (R3).
  - The engine check: `fmi-play/src/delay_check.rs`, binary `fmi-checks` `delay-check`. G0's tries:
    `delay/b3.sh`.
- **Setup:** laptop, seed 1, nice 10, at load 5-8 (other E012 components building and running their gates). Engine:
  round 2's `svm-e011-r2int` (patches 0001-0004, lean, read only), with round 2's play switches on
  (FMI_ENGINE_CUTS, FMI_PLAY_BATCH, FMI_HIT_CACHE, FMI_STONE_KEEP).
- **Rows:** `delay-check.jsonl` and `delay-check.txt` (the engine runs), `r2.txt`.

## Gates

| gate | clause | result |
|---|---|---|
| R1 | entries verify from boot | test (a)'s plans in ready mode with the fork forced on every input window, p = 1/2, 1/4, 1/21 (K = 7, 17, 95) and with no K: the 4 attempts (27, 27, 28 and 36 inputs; 3, 3, 5 and 8 checkpoints) and their 1, 1, 12 and 13 candidate entries, 31 runs in all, verified from boot in a fresh lean engine (fmi-engine's `replay::verify`): 0 differences |
| R2 | xorshift32 outputs mod n, n = 1..233, from 3182679328, uniform by χ² at 1% | 100,000 successive outputs at each call stride 1-10: the family of the 232 non-trivial n passes at 1% (Bonferroni, p ≥ 4.3·10⁻⁵) at every stride; least p 1.3·10⁻⁴ (stride 5, n = 162). Per n at 1%: 1-9 of 232 below (2.3 expected if the n were independent; they share the samples) |
| R3 | the record's k equals the least matching k on a fixture of forced branches | the window search's own resolution of tries over forced tables: every outcome of k = 0..5 for K = 7 and K = 2 (1,458 tables); 2,000 random tables (gated and issuable frames, waits, 1-3 tries in flight ending in any order, duplicates, deviations, tries past K), 967 with a match and 1,033 without; 1,000 with no K (amendment 2: the first end point). On the engine, the 11 forks' records each chose the least k whose try matched |

Checks of the fixture: with the deviation taken at once (E010's rule) both table tests fail, and with the bound one
frame too wide the random tables fail.

## G0 for the switch

- **Test (a)'s play fixtures** (E011's play tests 3 and 4 on the lean engine, `play-lean`):
  - strict, FMI_DELAY_FORK unset: the same children as the capture in the 4 windows, the 3 plans with 0 differences
    (outcome, checkpoints, inputs, engine frame, prediction), every window record equal field for field to a recorded
    reference run;
  - ready, unset and set: the same 4 children as the capture, the 3 plans with 0 differences. Set, no window is
    triggered (test (a)'s windows check no draw); the records gain `"delay"` with the calls per frame only.
- **The fork forced with no K** (amendment 2): the route is E010's, the goal at 3216 with 36 inputs, as the capture.
- **B3's 15 tries** in round 2's fast settings: below.

## Measured

- **Calls per frame** (test (a), the parent's frames): room 33 (2728-2799), 1 per frame in its 24 frames; room 28, 2 per
  frame in 250 and 4 in 46 of the 296 frames from 2798, and 1 in the frame at 3204. M1 measured 1 in room 33 and 2-6
  in room 28.
- **The forced forks on test (a)** (per window: tries with a k, their outcomes, the choice):

  | p (K) | window 1, sentence(2,428) | window 2, walk_to(428) | window 3, walk_to(316)[202] | the run |
  |---|---|---|---|---|
  | 1/2 (7) | 1 try, success at k = 0 | 7 tries (1 refuted, 6 spent): refuted, window empty | | replan at 2783, off the capture's route |
  | 1/4 (17) | 1 try, success at k = 0 | 17 tries (1 refuted, 16 spent): refuted, window empty | | replan at 2793, off the route |
  | 1/21 (95) | 1 try, success at k = 0 | 95 tries (2 refuted, 18 spent, 75 deviations): refuted, the first deviation (k = 20, E010's choice) taken, 9 candidates | 95 tries, all spent: refuted, window empty, 3 candidates | replan at 2894, off the route |
  | none | 1 try, success at k = 0 | 21 tries, the deviation at k = 20 | 294 tries, the deviation after 293 failures | E010's route: the goal at 3216 (window 4: the goal at k = 0) |

  Window 2 is a deviation window in ready mode: E010 takes the deviation at k = 20, after 18 spent and 2 refuted
  tries. The prediction (k = 0 in windows 1, 2 and 4) was wrong for it. Window 3's chosen try in E010 comes after
  293 failed ones, so every K tried refutes it, as predicted.
- **Cost of the fork:** one full dump per try that is not an end point. Window 3 with K = 95: 96 children in 1.08 s
  (11 ms per try with its end dump); with no K, 296 children in 3.56 s.
