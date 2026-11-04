#!/bin/bash
# E016 K1 (plan.md "Monkey Island with every switch on"): Monkey Island's kitchen by search under env.sh's set (play8's
# ALLON less scope growth, with E16), on the laptop, b = 4, a 60-minute wall (FMI_STOP_WALL=3600). As E012's kitchen
# rerun (E012-game-mechanics/kitchen/run.sh), with that run's init settings (owner:566 = 1, E009's kitchen guide, the
# k = 2 scope's 10 rooms, E009's switches, play when ready, commit as finished) and its run settings (--search fd
# --tail, under the guard at nice 10; the driver takes the heavy lock per heavy job itself); no falsifier bound (the
# wall decides). At a goal stop the driver verifies the goal entry from boot.
#   init:   `fmi explore init` into a new archive
#   run:    `fmi explore run`; a second run resumes from the archive's files
#   verify: the goal entry and its ancestors verified from boot one at a time (`fmi explore verify`, E014's mechanism
#           switches and A4 on), as a4/mi-check.sh's kitchen check
# usage: k1.sh [init|run|verify]...   (default all three in order)
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
K_ARCH=/var/tmp/fast-mi/e016-archives/k1
K_RUN=/var/tmp/fast-mi/e016-archives/k1-run
K_OUT=/var/tmp/fast-mi/e016-k1
W=${W:-4}
GUARD=$HOME/.cache/fast-mi-e007/guard.sh
mkdir -p $K_OUT $K_RUN/tmp $(dirname $K_ARCH)
SW="$ALLON FMI_ENGINE_BIN=$E13_ENGINE_BIN FMI_MEMO= FMI_READING_CACHE=$K_RUN/rcache FMI_PLAN_CACHE=$K_RUN/plans FMI_POOL_DIR=$K_RUN/pool TMPDIR=$K_RUN/tmp E008_DISK_PATH=/var/tmp E008_FLOOR_GB=5 FMI_STOP_WALL=${K1_WALL:-3600}"
for st in ${@:-init run verify}; do
  case $st in
    init)
      [ -e $K_ARCH ] && { echo "$K_ARCH exists"; exit 2; }
      env $SW nice -n 10 $FMI_BIN explore init $K_ARCH --stage kitchen --goal owner:566=1 \
        --guide $HOME/.cache/fast-mi-e009/guides/kitchen.json --allowed 28,29,33,34,35,38,41,83,85,96 --workers $W \
        --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts --stop-jobs 100000 --ready --commit-as-finished \
        > $K_OUT/init.log 2>&1 || { cat $K_OUT/init.log; exit 1; }
      cat $K_OUT/init.log;;
    run)
      n=$(ls $K_OUT/run*.log 2>/dev/null | wc -l)
      log=$K_OUT/run$([ $n -gt 0 ] && echo -$n).log
      echo "[$(date +%T)] start, workers $W, load $(cut -d' ' -f1-3 /proc/loadavg), log $log"
      env $SW FMI_DRIVER_STATS=1 $GUARD $log $FMI_BIN explore run $K_ARCH --workers $W --search fd --tail
      echo "[$(date +%T)] $(tail -1 $log)"
      jq -c '{stop, detail, jobs, entries, goal_entries, verify}' $K_ARCH/summary.json;;
    verify)
      g=$(jq '.goal_entries[0] // empty' $K_ARCH/summary.json)
      [ -n "$g" ] || { echo "verify: no goal entry ($(jq -r .stop $K_ARCH/summary.json))"; exit 1; }
      chain=$(jq -rs --argjson g $g '. as $r | [$g | recurse(if ($r[.].parent // 0) != 0 then $r[.].parent else empty end)] | reverse | map(tostring) | join(" ")' $K_ARCH/entries.jsonl)
      bad=0; n=0; mkdir -p $K_OUT/verify
      for i in $chain; do
        n=$((n + 1))
        env $E14M $A4 FMI_ENGINE_BIN=$E13_ENGINE_BIN nice -n 10 $FMI_BIN explore verify $K_ARCH --entry $i > $K_OUT/verify/$i.out 2>&1 || { bad=$((bad + 1)); continue; }
        grep -q '^differences: 0;' $K_OUT/verify/$i.out || bad=$((bad + 1))
      done
      t=$(jq -r 'select(.id == '$g') | .job' $K_ARCH/entries.jsonl)
      echo "[$(date +%T)] K1: goal entry $g (job $t), its chain of $n entries verified from boot, $bad with differences or errors; $(tail -1 $K_OUT/verify/$g.out)";;
    *) echo "$st: init, run or verify"; exit 2;;
  esac
done
