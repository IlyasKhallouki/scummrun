# G1 tests: the switch off leaves every existing test unchanged

`CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015-g1 cargo test -p fmi-play -p fmi-explore` in
`experiments/E011-speed/rs`, FMI_REALIZE unset. "Before" is b9fea7f (the plan's commit), "after" is 6ae5341
(realize.rs, its tests, afford's `object_grid`).

| test binary | before | after |
|---|---|---|
| fmi-explore unit (src/lib.rs) | 47 passed | 47 passed |
| fmi-explore tests/afford.rs | 12 | 12 |
| tests/archive.rs | 28 | 28 |
| tests/archive_progress.rs | 15 | 15 |
| tests/bench_start.rs | 1 | 1 |
| tests/free.rs | 6 | 6 |
| tests/free_explore.rs | 5 | 5 |
| tests/grow.rs | 3 | 3 |
| tests/guide.rs | 11 | 11 |
| tests/stall.rs | 5 | 5 |
| tests/step.rs | 26 | 26 |
| tests/targets_per_key.rs | 11 | 11 |
| fmi-play unit (src/lib.rs) | 22 passed, 1 ignored | 22 passed, 1 ignored |
| fmi-play tests/realize.rs (new) | - | 8 passed |
| fmi-play tests/realize_off.rs (new) | - | 1 passed |

0 failures before and after. The toy-engine tests (tests/realize.rs): a bare click gives the sentence; only a verb slot
then the object gives it (3 candidates, 7 forks); a target off the screen comes on after 2 floor clicks; a target that
never comes on screen misses with "approach"; no candidate gives the sentence ("no sentence" after 4 candidates, and
after 2 with the limit set to 2); a dialog verb (taken; "not taken"; "no slot"); a two-object sentence; step texts.
tests/realize_off.rs: with FMI_REALIZE unset the routine refuses before it touches the engine.
