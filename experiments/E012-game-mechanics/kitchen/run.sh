#!/bin/bash
# E012's kitchen rerun (plan.md "Runs"; amendment "the kitchen rerun, as run"):
#   init: `fmi explore init` from the boot into a new archive with the kitchen check's settings: owner:566 = 1, the
#         k = 2 scope's 10 rooms, b = 2, E009's switches, play when ready, commit as finished, E009's copy of E008's
#         guides, the falsifier at 80 tries; FMI_SELECT=progress, so stage.json keeps the selection by progress.
#   run:  `fmi explore run` in round 2's fast settings (play when ready, the old tail first, the Rust
#         lama-first-costs: --search fd --tail), the no-plan proofs and deepening on (ON3), every E012 switch on
#         (E12ON), end's engine. It stops on E009's stop rules or the falsifier; at a goal stop the driver verifies the
#         goal entry from boot in a fresh lean engine. A second `run` resumes from the archive's files.
# The run is under the guard at nice 10; the driver takes the heavy lock itself for each job of a heavy scope
# (driver.rs HeavyLock), so this script must not hold it.
# usage: ENV=kitchen/env.sh run.sh init|run
set -u
source "${ENV:?an env file (kitchen/env.sh)}"
GUARD=$HOME/.cache/fast-mi-e007/guard.sh
mkdir -p $K_OUT $K_RUN/tmp $(dirname $K_ARCH)
SW="$ON3 $E12ON FMI_ENGINE_BIN=$E12_ENGINE_BIN FMI_MEMO= FMI_READING_CACHE=$K_RUN/rcache FMI_PLAN_CACHE=$K_RUN/plans FMI_POOL_DIR=$K_RUN/pool TMPDIR=$K_RUN/tmp E008_DISK_PATH=/var/tmp E008_FLOOR_GB=5"
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
