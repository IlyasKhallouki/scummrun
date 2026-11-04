#!/bin/bash
# E013 integration: E012's driver smoke run (integrate/smoke.sh's settings: `fmi explore init` with the kitchen check's
# settings, goal owner:566 = 1, E009's kitchen guide, E012's ten rooms, E009's switches, play when ready, commit as
# finished, the falsifier at N jobs; `fmi explore run --search fd --tail`), b workers, in switch set SET, then every
# entry verified from boot (`fmi explore verify`, one at a time, with the run's engine and E014's mechanism switches
# when SET has them), and with FMI_STALLS=1 stall-report's checks over the archive and its live stalls.json (C3).
#   SET=e12   ON3 and E12ON, every E013 and E014 switch unset (G0's driver run, against E012's build: BIN and ENG)
#   SET=all   amendment (2) (b)'s one switch set (ALLON)                                      (G-on's driver run)
# The caller holds the heavy lock across the run (fmi with the reader resident is above 1 GB), so the driver's own lock
# per heavy job is off (E008_HEAVY_LOCK=none). BIN, ENG: another fmi and engine (default e013's and E013's).
# usage: ENV=integrate/env.sh [BIN=...] [ENG=...] flock /var/tmp/fast-mi/heavy.lock smoke.sh OUT N B SET
set -u
source "${ENV:?an env file (integrate/env.sh)}"
OUT=$1; N=$2; B=$3; SET=$4
BIN=${BIN:-$FMI_BIN}; ENG=${ENG:-$E13_ENGINE_BIN}
A=$OUT/kitchen
GUARD=$HOME/.cache/fast-mi-e007/guard.sh
case $SET in
  e12) SETSW="$ON3 $E12ON $E14MOFF $E13OFF"; MECH="";;
  all) SETSW="$ALLON"; MECH="$E14M";;
  *) echo "SET=$SET: e12 or all"; exit 2;;
esac
mkdir -p $OUT/tmp
[ -e $A ] && { echo "$A exists"; exit 2; }
SW="$SETSW FMI_ENGINE_BIN=$ENG FMI_MEMO= FMI_READING_CACHE=$OUT/rcache FMI_PLAN_CACHE=$OUT/plans FMI_POOL_DIR=$OUT/pool TMPDIR=$OUT/tmp E008_DISK_PATH=/var/tmp E008_FLOOR_GB=5 E008_HEAVY_LOCK=none"
echo "[$(date +%T)] start: $BIN, set $SET, engine $ENG, b $B, N $N; load $(cut -d' ' -f1-3 /proc/loadavg)"
env $SW $BIN explore init $A --stage kitchen --goal owner:566=1 --guide $HOME/.cache/fast-mi-e009/guides/kitchen.json \
  --allowed 28,29,33,34,35,38,41,83,85,96 --workers $B --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts \
  --stop-jobs $N --ready --commit-as-finished > $OUT/init.log 2>&1 || { cat $OUT/init.log; exit 1; }
t0=$(date +%s.%N)
env $SW $GUARD $OUT/run.log $BIN explore run $A --workers $B --search fd --tail
t1=$(date +%s.%N)
echo "[$(date +%T)] run: $(awk "BEGIN{print $t1 - $t0}") s; $(tail -1 $OUT/run.log)"
n=$(wc -l < $A/entries.jsonl); bad=0
for ((i = 1; i < n; i++)); do
  env $MECH FMI_ENGINE_BIN=$ENG nice -n 10 $BIN explore verify $A --entry $i > $OUT/verify-$i.out 2>&1 || bad=$((bad + 1))
  grep -q '^differences: 0;' $OUT/verify-$i.out || bad=$((bad + 1))
done
echo "[$(date +%T)] verify: $((n - 1)) entries from boot (fmi explore verify), $bad with differences or errors"
if [ -e $A/stalls.json ]; then
  nice -n 10 $E13T/checks/release/stall-report $A --json $OUT/stall-report.json --md $OUT/stall-report.md --live $A/stalls.json > $OUT/stall-report.out 2>&1
  echo "[$(date +%T)] stall-report rc $?: $(grep -c '^- PASS' $OUT/stall-report.md) PASS, $(grep -c '^- FAIL' $OUT/stall-report.md) FAIL"
fi
# the readings' bulk is not read after the verify: reading.json and the start rows stay
find $OUT \( -name 'reading.tc' -o -name 'reading.actions.jsonl' -o -name 'reading.lists.jsonl' -o -name 'reading.idx' \
  -o -name 'reading.reached' \) -delete
