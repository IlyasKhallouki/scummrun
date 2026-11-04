#!/usr/bin/env bash
# E016 ladder: its validation (plan.md, "The outside ladder"), on 2026-11-04 from e016-b. Build: the ladder and its
# fixture builder; play8 (E014 amendment (8), read only) expected at C0; the fixture from E015's item check chain
# (carry-4: 35 clicks from boot, verified from boot by E015's check) replayed from boot with E015's item env, one entry
# per stop of the chain, expected at C6.
set -euo pipefail
cd /var/tmp/fast-mi/wt-e016-b/experiments/E011-speed/rs
CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e016-b nice -n 10 cargo build --release -p fmi-checks --bin ladder --bin ladder-fixture
B=/var/tmp/fast-mi/target-e016-b/release
R=/var/tmp/fast-mi/wt-e016-b/experiments/E016-realize-run/results/ladder
$B/ladder /var/tmp/fast-mi/e014-archives/play8 --json $R/play8.json > $R/play8.txt
# the fixture (the engine replays the chain; E015's environment, this worktree as FMI_ROOT)
( source /var/tmp/fast-mi/wt-e016-b/experiments/E015-engine-gates/results/item/env.sh
  export FMI_ROOT=/var/tmp/fast-mi/wt-e016-b
  nice -n 10 $B/ladder-fixture $R/fixture-carry \
    --chain /var/tmp/fast-mi/wt-e016-b/experiments/E015-engine-gates/results/item/check/carry-4-use-946.json \
    --frames 992,1043,1154,1344,1444,1459,1475,1910,2169,2980,2998,3044 )
$B/ladder $R/fixture-carry --json $R/fixture-carry.json > $R/fixture-carry.txt
