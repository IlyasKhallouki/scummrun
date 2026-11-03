#!/bin/bash
# E011 round 2: play's fixture tests (plan.md "play", tests 2-4; bench/g10play.sh's runs) on patch 0003's build with
# all six switches, through fmi-engine's binary channel: the capture of E009's step test (a) replayed as records, every
# child from its fork frame (test 2), strict windows against the recorded Python runs (test 3) and ready (test 4). Every
# child's observed frames run with the count-only mixer and no pixels, so this checks the window search's own reads.
# usage: play-r2.sh OUT    (PLAY_LEAN: fmi-checks' play-lean built from this branch; E011_ENGINE_BIN: the build)
set -u
source $HOME/.cache/fast-mi-e011/env.sh
[ -n "${E011_ENV:-}" ] && source "$E011_ENV" # a second env file after env.sh (round 2: env-r2.sh, its binaries)
export FMI_ENGINE_BIN=${E011_ENGINE_BIN:-/var/tmp/fast-mi/svm-e011-r2/build/scummvm/scummvm}
export FMI_ENGINE_CUTS=1
B=${PLAY_LEAN:-/var/tmp/fast-mi/target-e011-r2-engine/release/play-lean}
F=$HOME/.cache/fast-mi-e011/fixtures/play/a
R="$HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/run/a00/windows.jsonl $HOME/.cache/fast-mi-e008-check/step-a/jobs/ta/run/a00/windows.jsonl $HOME/.cache/fast-mi-e008/step-tests/jobs/ta/run/a00/windows.jsonl"
OUT=$1; mkdir -p $OUT
cd $OUT
bad=0
for t in recs children strict ready; do
  echo "=== $t $(date +%T) load $(cut -d' ' -f1-3 /proc/loadavg)"
  TIMEFORMAT="TIME $t: wall %R s, user %U s, sys %S s"
  if [ $t = strict ]; then time nice -n 10 $B $t $F $R > $OUT/$t.log 2>&1; else time nice -n 10 $B $t $F > $OUT/$t.log 2>&1; fi
  rc=$?; echo "rc $rc"; [ $rc = 0 ] || bad=1
  grep -E "^(READY|STRICT|CHILDREN|RECS|window [0-9]:)|differ|pass|FAIL" $OUT/$t.log | cut -c1-200 | tail -8
done
exit $bad
