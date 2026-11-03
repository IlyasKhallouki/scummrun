#!/bin/bash
# E012 integration: release builds of branch e012 into E12T (default /var/tmp/fast-mi/target-e012), debug info off:
#   rs/ workspace (fmi, fmi-engine, fmi-reading, fmi-plan)    -> $E12T/release
#   fmi-checks (play-lean, play-wire, afford-check, delay-check, ...) -> $E12T/checks/release
#   the scumm workspace's reader-scope, reader-port, reader-seq      -> $E12T/scumm/release
# taskcore is unchanged since E011 round 2 (its binaries are used as they are). The engine is end's build
# (svm-e012-end, patches 0001-0005). Run under the heavy lock and the guard. usage: build.sh [all|fmi|checks|reader]
set -e
here=$(cd "$(dirname "$0")" && pwd)
WT=$(cd $here/../../.. && pwd)
T=${E12T:-/var/tmp/fast-mi/target-e012}
export CARGO_PROFILE_RELEASE_DEBUG=0
what=${1:-all}
if [ $what = all ] || [ $what = fmi ]; then
  cd $WT/experiments/E011-speed/rs && CARGO_TARGET_DIR=$T nice -n 10 cargo build --release -j 2 2>&1 | tail -20
fi
if [ $what = all ] || [ $what = checks ]; then
  cd $WT/experiments/E011-speed/rs && CARGO_TARGET_DIR=$T/checks nice -n 10 cargo build --release -j 2 -p fmi-checks 2>&1 | tail -20
fi
if [ $what = all ] || [ $what = reader ]; then
  cd $WT/experiments/E003-path-through-cloud/scumm && CARGO_TARGET_DIR=$T/scumm nice -n 10 cargo build --release -j 2 -p scumm-reader --bin reader-scope --bin reader-port --bin reader-seq 2>&1 | tail -20
fi
echo BUILD-DONE
