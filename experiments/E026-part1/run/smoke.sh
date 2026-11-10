#!/bin/bash
# E026 smoke (plan.md G8, G13; mi-checks-now's copy of E013-robust/integrate/smoke.sh): E012's driver smoke run on
# Monkey Island with env.sh's ON (MI's files named): `fmi explore init` with the kitchen check's settings (goal
# owner:566 = 1, E009's kitchen guide, E012's ten rooms, play when ready, commit as finished, the falsifier at N jobs),
# `fmi explore run --search fd --tail` with b workers under the guard and the idle:sleep lock, then every entry
# verified from boot with ON (`fmi explore verify`, one at a time) and stall-report's checks over the archive and its
# live stalls.json. The caller holds the heavy lock (mi-check.sh), so the driver's own is off (E008_HEAVY_LOCK=none).
# usage: flock /var/tmp/fast-mi/heavy.lock smoke.sh OUT N B
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
game_ok || exit 1
OUT=$1; N=$2; B=$3
A=$OUT/kitchen
mkdir -p $OUT/tmp
disk_ok $OUT || exit 1
[ -e $A ] && { echo "$A exists"; exit 2; }
SW="$ON $MI FMI_MEMO= FMI_READING_CACHE=$OUT/rcache FMI_PLAN_CACHE=$OUT/plans FMI_POOL_DIR=$OUT/pool TMPDIR=$OUT/tmp E008_DISK_PATH=$(cd $OUT && pwd) E008_FLOOR_GB=$FLOOR_GB E008_GUARD=$GUARD E008_HEAVY_LOCK=none"
echo "[$(date +%T)] start: $FMI_BIN, engine $FMI_ENGINE_BIN, b $B, N $N; load $(cut -d' ' -f1-3 /proc/loadavg)"
env $SW $FMI_BIN explore init $A --stage kitchen --goal owner:566=1 --guide $HOME/.cache/fast-mi-e009/guides/kitchen.json \
  --allowed 28,29,33,34,35,38,41,83,85,96 --workers $B --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts \
  --stop-jobs $N --ready --commit-as-finished > $OUT/init.log 2>&1 || { cat $OUT/init.log; exit 1; }
t0=$(date +%s.%N)
env $SW $GUARD $OUT/run.log $INHIBIT $FMI_BIN explore run $A --workers $B --search fd --tail
t1=$(date +%s.%N)
echo "[$(date +%T)] run: $(awk "BEGIN{print $t1 - $t0}") s; $(tail -1 $OUT/run.log)"
n=$(wc -l < $A/entries.jsonl); bad=0
for ((i = 1; i < n; i++)); do
  env $ON $MI nice -n $NICE $FMI_BIN explore verify $A --entry $i > $OUT/verify-$i.out 2>&1 || bad=$((bad + 1))
  grep -q '^differences: 0;' $OUT/verify-$i.out || bad=$((bad + 1))
done
echo "[$(date +%T)] verify: $((n - 1)) entries from boot (fmi explore verify), $bad with differences or errors"
if [ -e $A/stalls.json ]; then
  nice -n $NICE $CHK/stall-report $A --json $OUT/stall-report.json --md $OUT/stall-report.md --live $A/stalls.json > $OUT/stall-report.out 2>&1
  echo "[$(date +%T)] stall-report rc $?: $(grep -c '^- PASS' $OUT/stall-report.md) PASS, $(grep -c '^- FAIL' $OUT/stall-report.md) FAIL"
fi
echo "[$(date +%T)] readings at the bound: $(jq -r 'select((.reading.why // "") | startswith("bound")) | .job' $A/log.jsonl | wc -l)"
# the readings' bulk is not read after the verify: reading.json and the start rows stay
find $OUT \( -name 'reading.tc' -o -name 'reading.actions.jsonl' -o -name 'reading.lists.jsonl' -o -name 'reading.idx' \
  -o -name 'reading.reached' \) -delete
