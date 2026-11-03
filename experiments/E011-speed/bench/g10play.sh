#!/bin/bash
# E011 G10's play part (experiments/E011-speed/plan.md, "play": "tests 2-4 ... again on the lean build through
# fmi-engine at integration"), and B1's test (a) windows after: fmi-checks' play-lean on the capture of E009's step test
# (a): records, every child (test 2), strict (test 3, every window record against the four recorded Python runs) and
# ready (test 4). At nice 10.
# usage: g10play.sh OUT
set -u
source $HOME/.cache/fast-mi-e011/env.sh
[ -n "${E011_ENV:-}" ] && source "$E011_ENV" # a second env file after env.sh (round 2: env-r2.sh, its binaries)
B=/var/tmp/fast-mi/target-e011-checks/release/play-lean
F=$HOME/.cache/fast-mi-e011/fixtures/play/a
R="$HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/run/a00/windows.jsonl $HOME/.cache/fast-mi-e008-check/step-a/jobs/ta/run/a00/windows.jsonl $HOME/.cache/fast-mi-e008/step-tests/jobs/ta/run/a00/windows.jsonl"
OUT=$1; mkdir -p $OUT
cd $OUT
for t in recs children strict ready; do
  echo "=== $t $(date +%T) load $(cut -d' ' -f1-3 /proc/loadavg)"
  TIMEFORMAT="TIME $t: wall %R s, user %U s, sys %S s"
  if [ $t = strict ]; then time nice -n 10 $B $t $F $R > $OUT/$t.log 2>&1; else time nice -n 10 $B $t $F > $OUT/$t.log 2>&1; fi
  echo "rc $?"; grep -E "^(READY|STRICT|CHILDREN|RECS|window [0-9]:)|differ|pass|FAIL" $OUT/$t.log | cut -c1-200 | tail -8
done
