#!/bin/bash
# E011 round 2, play (results/README.md "What binds", item 4): test (a)'s windows on the lean build through
# fmi-engine, with a given play-lean binary and engine, back to back. Runs play-lean (fmi-checks) on a copy of the
# play fixtures (the reading stays where the capture put it), at nice 10.
# usage: r2bench.sh PLAY_LEAN ENGINE OUT [recs|children|strict|ready ...]
#   env: FX (the fixture copy, default /var/tmp/fast-mi/e011-r2-play/fx/a); any FMI_* switch is passed through
set -u
source $HOME/.cache/fast-mi-e011/env.sh
[ -n "${E011_ENV:-}" ] && source "$E011_ENV" # a second env file after env.sh (round 2: env-r2.sh, its binaries)
B=$1; ENG=$2; OUT=$3; shift 3
F=${FX:-/var/tmp/fast-mi/e011-r2-play/fx/a}
R="$HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/run/a00/windows.jsonl $HOME/.cache/fast-mi-e008-check/step-a/jobs/ta/run/a00/windows.jsonl $HOME/.cache/fast-mi-e008/step-tests/jobs/ta/run/a00/windows.jsonl"
tests=${*:-recs children strict ready}
mkdir -p $OUT
for t in $tests; do
  echo "=== $t $(date +%T) load $(cut -d' ' -f1-3 /proc/loadavg) MemAvailable $(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo) MB"
  TIMEFORMAT="TIME $t: wall %R s, user %U s, sys %S s"
  extra=""; [ $t = strict ] && extra=$R
  time FMI_ENGINE_BIN=$ENG nice -n 10 $B $t $F $extra > $OUT/$t.log 2> $OUT/$t.err
  echo "rc $?"
  grep -E "^(READY|STRICT|CHILDREN|RECS|window [0-9]:|plan [0-9])|differ|pass|FAIL|hit cache" $OUT/$t.log | cut -c1-220 | tail -12
done
