#!/bin/bash
# E012 resident: several steps under one hold of the heavy lock (each still under the guard): STEP...
#   tests        the cargo tests (release profile, reusing build.sh's builds): scumm-reader's memo tests, fmi-reading's
#   rs1-inproc   rs1.sh OUT inproc, then rs1.sh OUT cmp
#   keys         kitchen.sh OUT j00057 keys
#   before N | disk | after N   kitchen.sh OUT SECOND PHASE N (SECOND from OUT/second, else j00057)
# usage: flock /var/tmp/fast-mi/heavy.lock heavy.sh OUT STEP [STEP...]   (steps with a number: "before:1")
here=$(cd "$(dirname "$0")" && pwd)
WT=$(cd $here/../../.. && pwd)
OUT=$1; shift
export NOLOCK=1
T=${RESIDENT_TARGET:-/var/tmp/fast-mi/target-e012-resident}
G=$HOME/.cache/fast-mi-e007/guard.sh
second() { cat $OUT/second 2>/dev/null || echo j00057; }
for s in "$@"; do
  step=${s%%:*}; n=${s#*:}; [ "$n" = "$s" ] && n=1
  echo "[$(date +%T)] step $s"
  case $step in
    tests)
      $G $OUT/tests-scumm.log bash -c "cd $WT/experiments/E003-path-through-cloud/scumm && CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_TARGET_DIR=$T/scumm cargo test --release -j 2 -p scumm-reader --lib memo::"
      grep -E "test result|FAILED|panicked|error" $OUT/tests-scumm.log | head
      $G $OUT/tests-rs.log bash -c "cd $WT/experiments/E011-speed/rs && CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_TARGET_DIR=$T cargo test --release -j 2 -p fmi -p fmi-reading"
      grep -E "test result|FAILED|panicked|error" $OUT/tests-rs.log | head ;;
    rs1-inproc) $here/rs1.sh $OUT/rs1 inproc cmp ;;
    keys) $here/kitchen.sh $OUT/kitchen j00057 keys ;;
    before|disk|after) $here/kitchen.sh $OUT/kitchen $(second) $step $n ;;
    *) echo "unknown step $s" ;;
  esac
done
