#!/bin/bash
# E012 resident: release builds of fmi (rs/ workspace) and reader-scope (scumm workspace) from this worktree, into
# RESIDENT_TARGET (default /var/tmp/fast-mi/target-e012-resident), debug info off. Run under the heavy lock and the
# guard. usage: build.sh [all|fmi|reader]
set -e
here=$(cd "$(dirname "$0")" && pwd)
WT=$(cd $here/../../.. && pwd)
T=${RESIDENT_TARGET:-/var/tmp/fast-mi/target-e012-resident}
export CARGO_PROFILE_RELEASE_DEBUG=0
what=${1:-all}
if [ $what = all ] || [ $what = fmi ]; then
  cd $WT/experiments/E011-speed/rs && CARGO_TARGET_DIR=$T cargo build --release -j 2 -p fmi 2>&1 | tail -30
fi
if [ $what = all ] || [ $what = reader ]; then
  cd $WT/experiments/E003-path-through-cloud/scumm && CARGO_TARGET_DIR=$T/scumm cargo build --release -j 2 -p scumm-reader --bin reader-scope 2>&1 | tail -30
fi
echo BUILD-DONE
