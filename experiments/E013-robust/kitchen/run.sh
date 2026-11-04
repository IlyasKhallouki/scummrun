#!/bin/bash
# E013's kitchen rerun K1 (plan.md "Runs"; amendment (2) (b), (c)): E012's kitchen/run.sh with e013's build and engine
# and amendment (2) (b)'s one switch set (ALLON: ON3, every E012 switch, E014's mechanisms SPEEDRUN_IDLE=handler,
# SPEEDRUN_END_SITE=1, FMI_GOAL_END=1, FMI_READ_EVENTS=1 and E14P, and E013's SPEEDRUN_INPUT=1 FMI_REFUSE=1
# FMI_STALLS=1 FMI_FREE=0.25 FMI_FREE_SHIFT=1).
#   init: `fmi explore init` from the boot into a new archive: owner:566 = 1, E009's kitchen guide, the k = 2 scope's 10
#         rooms, b = 2, E009's switches, play when ready, commit as finished, the falsifier at 80 tries.
#   run:  `fmi explore run --workers 2 --search fd --tail`. It stops on E009's stop rules or the falsifier; at a goal
#         stop the driver verifies the goal entry from boot in a fresh lean engine. A second `run` resumes.
# Under the guard at nice 10; the driver takes the heavy lock itself for each job of a heavy scope (driver.rs
# HeavyLock), so this script must not hold it.
# usage: ENV=kitchen/env.sh run.sh init|run
set -u
source "${ENV:?an env file (kitchen/env.sh)}"
GUARD=$HOME/.cache/fast-mi-e007/guard.sh
mkdir -p $K_OUT $K_RUN/tmp $(dirname $K_ARCH)
SW="$ALLON FMI_ENGINE_BIN=$E13_ENGINE_BIN FMI_MEMO= FMI_READING_CACHE=$K_RUN/rcache FMI_PLAN_CACHE=$K_RUN/plans FMI_POOL_DIR=$K_RUN/pool TMPDIR=$K_RUN/tmp E008_DISK_PATH=/var/tmp E008_FLOOR_GB=5"
case ${1:?init or run} in
  init)
    [ -e $K_ARCH ] && { echo "$K_ARCH exists"; exit 2; }
    env $SW nice -n 10 $FMI_BIN explore init $K_ARCH --stage kitchen --goal owner:566=1 \
      --guide $HOME/.cache/fast-mi-e009/guides/kitchen.json --allowed 28,29,33,34,35,38,41,83,85,96 --workers 2 \
      --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts --stop-jobs 80 --ready --commit-as-finished \
      > $K_OUT/init.log 2>&1 || { cat $K_OUT/init.log; exit 1; }
    cat $K_OUT/init.log;;
  run)
    n=$(ls $K_OUT/run*.log 2>/dev/null | wc -l)
    log=$K_OUT/run$([ $n -gt 0 ] && echo -$n).log
    echo "[$(date +%T)] start, load $(cut -d' ' -f1-3 /proc/loadavg), log $log"
    env $SW FMI_DRIVER_STATS=1 $GUARD $log $FMI_BIN explore run $K_ARCH --workers 2 --search fd --tail
    echo "[$(date +%T)] $(tail -1 $log)";;
esac
