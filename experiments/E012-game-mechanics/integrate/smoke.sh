#!/bin/bash
# E012 integration: the driver with every switch on, a few tries (not the kitchen rerun): `fmi explore init` with the
# kitchen check's settings (E009's switches, play when ready, commit as finished, b = 2) and E009's falsifier set to
# N jobs, then `fmi explore run` in the fast settings with ON3 and every E012 switch on (end's engine). It runs the
# paths no gate runs end to end: job.json's "novelty" and "dead" from the archive (FMI_SELECT=progress) read by the
# try, result.json's "tested", "dead" and the extra entries ingested by the archive, the cells over T, and the
# driver's verify. Then every entry is verified from boot (fmi explore verify, one at a time). Under the heavy lock
# and the guard.
# usage: ENV=integrate/env.sh smoke.sh OUT [N]
set -u
source "${ENV:?an env file (integrate/env.sh)}"
OUT=$1; N=${2:-4}
A=$OUT/kitchen
GUARD=$HOME/.cache/fast-mi-e007/guard.sh
mkdir -p $OUT/tmp
[ -e $A ] && { echo "$A exists"; exit 2; }
SW="$ON3 $E12ON FMI_ENGINE_BIN=$E12_ENGINE_BIN FMI_MEMO= FMI_READING_CACHE=$OUT/rcache FMI_PLAN_CACHE=$OUT/plans FMI_POOL_DIR=$OUT/pool TMPDIR=$OUT/tmp E008_DISK_PATH=/var/tmp E008_FLOOR_GB=5"
exec 201>/var/tmp/fast-mi/heavy.lock; flock 201
echo "[$(date +%T)] heavy lock taken, load $(cut -d' ' -f1-3 /proc/loadavg)"
env $SW $FMI_BIN explore init $A --stage kitchen --goal owner:566=1 --guide $HOME/.cache/fast-mi-e009/guides/kitchen.json \
  --allowed 28,29,33,34,35,38,41,83,85,96 --workers 2 --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts \
  --stop-jobs $N --ready --commit-as-finished > $OUT/init.log 2>&1 || { cat $OUT/init.log; exit 1; }
t0=$(date +%s.%N)
env $SW $GUARD $OUT/run.log $FMI_BIN explore run $A --workers 2 --search fd --tail
t1=$(date +%s.%N)
echo "[$(date +%T)] run: $(awk "BEGIN{print $t1 - $t0}") s; $(tail -1 $OUT/run.log)"
flock -u 201; exec 201>&-
n=$(wc -l < $A/entries.jsonl); bad=0
for ((i = 1; i < n; i++)); do
  env FMI_ENGINE_BIN=$E12_ENGINE_BIN nice -n 10 $FMI_BIN explore verify $A --entry $i > $OUT/verify-$i.out 2>&1 || bad=$((bad + 1))
  grep -q '^differences: 0;' $OUT/verify-$i.out || bad=$((bad + 1))
done
echo "[$(date +%T)] verify: $((n - 1)) entries from boot (fmi explore verify), $bad with differences or errors"
