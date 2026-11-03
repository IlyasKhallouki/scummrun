#!/bin/bash
# E014's run on the demo (plan.md (e)):
#   guide: `fmi guide map --goal end=1 --start-boot`: the regression from the demo's end sites, from its boot's facts
#   init:  `fmi explore init` from the boot into a new archive: stage demo, goal end=1, the rooms from the files, b = 2,
#          E009's switches, play when ready, commit as finished, the falsifier at 80 tries (STOP_JOBS)
#   run:   `fmi explore run` in round 2's fast settings (--search fd --tail), ON3, every E012 switch (E12ON) and every
#          E014 switch (E14ON); it stops on E009's stop rules, the goal or the falsifier; at a goal stop the driver
#          verifies the goal entry from boot in a fresh lean engine. A second `run` resumes from the archive's files.
# The run is under the guard at nice 10; the driver takes the heavy lock itself for each job of a heavy scope.
# usage: ENV=demo/env.sh run.sh guide|init|run      (D_ARCH, D_RUN, D_OUT, STOP_JOBS override the defaults; STAGE,
#        default demo, names the stage and the guide: play/env.sh sets it for the interactive demo)
set -u
source "${ENV:?an env file (demo/env.sh)}"
STAGE=${STAGE:-demo}
GUARD=$HOME/.cache/fast-mi-e007/guard.sh
mkdir -p $D_OUT $D_RUN/tmp $(dirname $D_ARCH)
SW="$ON3 $E12ON $E14ON FMI_ENGINE_BIN=$E14_ENGINE_BIN FMI_MEMO= FMI_READING_CACHE=$D_RUN/rcache FMI_PLAN_CACHE=$D_RUN/plans FMI_POOL_DIR=$D_RUN/pool TMPDIR=$D_RUN/tmp E008_DISK_PATH=/var/tmp E008_FLOOR_GB=5"
case ${1:?guide, init or run} in
  guide)
    env $SW nice -n 10 $FMI_BIN guide map --goal end=1 --start-boot --out $D_OUT/guide-$STAGE.json | tee $D_OUT/guide.log;;
  init)
    [ -e $D_ARCH ] && { echo "$D_ARCH exists"; exit 2; }
    env $SW nice -n 10 $FMI_BIN explore init $D_ARCH --stage $STAGE --goal end=1 --guide $D_OUT/guide-$STAGE.json \
      --allowed files --workers 2 --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts \
      --stop-jobs ${STOP_JOBS:-80} --ready --commit-as-finished > $D_OUT/init.log 2>&1 || { cat $D_OUT/init.log; exit 1; }
    cat $D_OUT/init.log;;
  run)
    n=$(ls $D_OUT/run*.log 2>/dev/null | wc -l)
    log=$D_OUT/run$([ $n -gt 0 ] && echo -$n).log
    echo "[$(date +%T)] start, load $(cut -d' ' -f1-3 /proc/loadavg), log $log"
    env $SW FMI_DRIVER_STATS=1 $GUARD $log $FMI_BIN explore run $D_ARCH --workers 2 --search fd --tail
    echo "[$(date +%T)] $(tail -1 $log)";;
esac
