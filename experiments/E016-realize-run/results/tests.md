# E016 A: the tests with the switches off

`CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e016-a cargo test --no-fail-fast` over the default members, in
`experiments/E011-speed/rs`, with `FMI_REALIZE` unset. "Before" is 4c40b3c (the plan's commit); "after" is f5ab28d.
- **The hook:** 8bcf853.
- **The inventory extension:** fa3bd2f.
- **Fix 1:** 072afe6.

| | passed | failed | ignored |
|---|---:|---:|---:|
| before | 241 | 0 | 9 |
| after | 248 | 0 | 9 |

- **The only binary whose count changed** is fmi-play's `tests/realize.rs`: 8 → 15. It sets `FMI_REALIZE=1` inside
  its own process. Every other test binary's count is the same as before (`diff` of the per-binary lists).
- **The 7 new tests** run on the toy engine:
  - an inventory object with a room object;
  - an inventory object with another inventory object;
  - the approach to b when a is in the inventory;
  - a bounded miss;
  - image slots first (amendment (2));
  - the hook plays the accepted events on the branch (a sentence and a dialog line);
  - the hook runs a branch that is not idle to its idle stop, and reports a miss as a route error with realize's
    reason.
- **`cargo check -p fmi-checks --bins`** builds every check binary (e015, e015chk and the new e016) with no warning.
- **With the switch off**, `play_input` and `Search::gate` take their old paths: the only new code on them is the
  `realize::on()` test.
- **The realize path names no profile read** (`grep-check.txt`). Its numeric literals are realize's limits (6, 60, 3,
  15, 8), the screen's half width, the engine's image verb type (1) and 0/1 counters.
