#!/bin/bash
# E013 integration: release builds of branch e013 into E13T (default /var/tmp/fast-mi/target-e013), debug info off, as
# E012's integrate/build.sh lays them out:
#   rs/ workspace (fmi, fmi-engine, fmi-reading, fmi-plan, free_check, targets_t2) -> $E13T/release
#   fmi-checks (play-lean, play-wire, afford-check, refuse-check, stall-report, ...)  -> $E13T/checks/release
#   the scumm workspace's reader-scope, reader-port, reader-seq                         -> $E13T/scumm/release
# The engine is engine.sh's (patches 0001-0005, 0006-e014 and 0007, in /var/tmp/fast-mi/svm-e013). Run under the heavy
# lock and the guard. usage: build.sh [all|fmi|checks|reader]
set -e
here=$(cd "$(dirname "$0")" && pwd)
WT=$(cd $here/../../.. && pwd)
T=${E13T:-/var/tmp/fast-mi/target-e013}
export CARGO_PROFILE_RELEASE_DEBUG=0
what=${1:-all}
if [ $what = all ] || [ $what = fmi ]; then
  cd $WT/experiments/E011-speed/rs && CARGO_TARGET_DIR=$T nice -n 10 cargo build --release -j 3 2>&1 | tail -20
fi
if [ $what = all ] || [ $what = checks ]; then
  cd $WT/experiments/E011-speed/rs && CARGO_TARGET_DIR=$T/checks nice -n 10 cargo build --release -j 3 -p fmi-checks 2>&1 | tail -20
fi
if [ $what = all ] || [ $what = reader ]; then
  cd $WT/experiments/E003-path-through-cloud/scumm && CARGO_TARGET_DIR=$T/scumm nice -n 10 cargo build --release -j 3 -p scumm-reader --bin reader-scope --bin reader-port --bin reader-seq 2>&1 | tail -20
fi
echo BUILD-DONE
