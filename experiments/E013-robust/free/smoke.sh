#!/bin/bash
# E013 component free: E012's driver smoke run (experiments/E012-game-mechanics/integrate/smoke.sh) with the worker
# count, FMI_FREE and the switch set given: `fmi explore init` with the kitchen check's settings and the falsifier at N
# jobs, `fmi explore run` in the fast settings, then every entry verified from boot (`fmi explore verify`, one at a
# time, with the run's engine and switches). Monkey Island: goal owner:566 = 1, E009's kitchen guide, E012's ten rooms.
#   SET=e12 (default): ON3 and every E012 switch (E12ON), end's engine (patches 0001-0005): free's amendment's runs.
#   SET=all: amendment (2) (b)'s one switch set for both games, as far as this branch has it: ON3, E12ON and E014's
#            mechanism switches (SPEEDRUN_IDLE=handler, SPEEDRUN_END_SITE=1, FMI_GOAL_END=1, FMI_READ_EVENTS=1 and
#            play/env.sh's E14P), E014's engine (patches 0001-0005 and 0006-e014). The game's settings stay unset.
# The run is under the guard; the caller holds the heavy lock across the whole run (fmi with the reader resident is
# above 1 GB), so the driver's own lock per heavy job is off (E008_HEAVY_LOCK=none).
# usage: FMI_BIN=... [SET=e12|all] flock /var/tmp/fast-mi/heavy.lock smoke.sh OUT N WORKERS [RHO]
#        (RHO empty: FMI_FREE unset)
set -u
here=$(cd "$(dirname "$0")" && pwd)
WT=$(cd $here/../../.. && pwd)
BIN=${FMI_BIN:?the fmi binary}
source $WT/experiments/E012-game-mechanics/integrate/env.sh
FMI_BIN=$BIN
export WT11=$WT
OUT=$1; N=$2; B=$3; RHO=${4:-}
SET=${SET:-e12}
A=$OUT/kitchen
GUARD=$HOME/.cache/fast-mi-e007/guard.sh
E14_ENGINE_BIN=/var/tmp/fast-mi/svm-e014/build/scummvm/scummvm
E14P="FMI_EGO_VALUES=1 FMI_MERGE_DEDUP=1 FMI_READ_CUT=1 FMI_STAG_SWEEP=1 FMI_VERBS_VAR_ON=1 FMI_NOVEL_VERBS=1 FMI_NOVEL_GUIDE=1"
case $SET in
  e12) ENG=$E12_ENGINE_BIN; MECH="";;
  all) ENG=$E14_ENGINE_BIN; MECH="SPEEDRUN_IDLE=handler SPEEDRUN_END_SITE=1 FMI_GOAL_END=1 FMI_READ_EVENTS=1 $E14P";;
  *) echo "SET=$SET: e12 or all"; exit 2;;
esac
mkdir -p $OUT/tmp
[ -e $A ] && { echo "$A exists"; exit 2; }
SW="$ON3 $E12ON $MECH FMI_ENGINE_BIN=$ENG FMI_MEMO= FMI_READING_CACHE=$OUT/rcache FMI_PLAN_CACHE=$OUT/plans FMI_POOL_DIR=$OUT/pool TMPDIR=$OUT/tmp E008_DISK_PATH=/var/tmp E008_FLOOR_GB=5 E008_HEAVY_LOCK=none FMI_FREE=$RHO"
echo "[$(date +%T)] start: $FMI_BIN, set $SET, engine $ENG, b $B, N $N, FMI_FREE '$RHO'; load $(cut -d' ' -f1-3 /proc/loadavg)"
env $SW $FMI_BIN explore init $A --stage kitchen --goal owner:566=1 --guide $HOME/.cache/fast-mi-e009/guides/kitchen.json \
  --allowed 28,29,33,34,35,38,41,83,85,96 --workers $B --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts \
  --stop-jobs $N --ready --commit-as-finished > $OUT/init.log 2>&1 || { cat $OUT/init.log; exit 1; }
t0=$(date +%s.%N)
env $SW $GUARD $OUT/run.log $FMI_BIN explore run $A --workers $B --search fd --tail
t1=$(date +%s.%N)
echo "[$(date +%T)] run: $(awk "BEGIN{print $t1 - $t0}") s; $(tail -1 $OUT/run.log)"
n=$(wc -l < $A/entries.jsonl); bad=0
for ((i = 1; i < n; i++)); do
  env $MECH FMI_ENGINE_BIN=$ENG nice -n 10 $FMI_BIN explore verify $A --entry $i > $OUT/verify-$i.out 2>&1 || bad=$((bad + 1))
  grep -q '^differences: 0;' $OUT/verify-$i.out || bad=$((bad + 1))
done
echo "[$(date +%T)] verify: $((n - 1)) entries from boot (fmi explore verify), $bad with differences or errors"
