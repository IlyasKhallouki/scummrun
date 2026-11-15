#!/bin/bash
# E014 amendment (3): the explorer on the interactive demo with E013's build and engine and every switch on (E013's
# ALLON), the game's settings as the run's inputs. The same script runs on the laptop (checks) and on the VM (the run).
#   guide:  `fmi guide map --goal end=1 --start-boot`: the regression from the end sites, from the boot's facts
#   init:   `fmi explore init` from the boot into a new archive: stage play, goal end=1, the rooms from the files,
#           E009's switches, play when ready, commit as finished, the falsifier at STOP_JOBS (80) committed tries
#   run:    `fmi explore run --workers W --search fd --tail` under the guard at nice 10; it stops on E009's stop rules,
#           the goal or the falsifier, and at a goal stop verifies the goal entry from boot in a fresh lean engine. A
#           second `run` resumes from the archive's files. HEAVY_LOCK=none runs heavy scopes side by side (the VM).
#   verify: `fmi explore verify` of entry E (default every entry, one at a time) in a fresh lean engine, with E014's
#           mechanism switches and the game's settings; the count with differences or errors
#   stalls: `fmi explore stalls` (E013's table by cause) into D_OUT/stalls-NAME.{md,json}
# usage: ENV=vm/env.sh run.sh guide|init|run|verify [E]|stalls      (W, STOP_JOBS, HEAVY_LOCK, FLOOR_GB override)
set -u
source "${ENV:?an env file (vm/env.sh)}"
GUARD=$HOME/.cache/fast-mi-e007/guard.sh
W=${W:-2}
mkdir -p $D_OUT $D_RUN/tmp $(dirname $D_ARCH)
SW="$ALLON $GAME FMI_ENGINE_BIN=$E13_ENGINE_BIN FMI_MEMO= FMI_READING_CACHE=$D_RUN/rcache FMI_PLAN_CACHE=$D_RUN/plans FMI_POOL_DIR=$D_RUN/pool TMPDIR=$D_RUN/tmp E008_DISK_PATH=/var/tmp E008_FLOOR_GB=${FLOOR_GB:-5} E008_HEAVY_LOCK=${HEAVY_LOCK:-}"
case ${1:?guide, init, run, verify or stalls} in
  guide)
    env $SW nice -n 10 $FMI_BIN guide map --goal end=1 --start-boot --out $D_OUT/guide-$STAGE.json | tee $D_OUT/guide.log;;
  init)
    [ -e $D_ARCH ] && { echo "$D_ARCH exists"; exit 2; }
    env $SW nice -n 10 $FMI_BIN explore init $D_ARCH --stage $STAGE --goal end=1 --guide $D_OUT/guide-$STAGE.json \
      --allowed files --workers $W --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts \
      --stop-jobs ${STOP_JOBS:-80} --ready --commit-as-finished > $D_OUT/init.log 2>&1 || { cat $D_OUT/init.log; exit 1; }
    cat $D_OUT/init.log;;
  run)
    n=$(ls $D_OUT/run*.log 2>/dev/null | wc -l)
    log=$D_OUT/run$([ $n -gt 0 ] && echo -$n).log
    echo "[$(date +%T)] start, workers $W, load $(cut -d' ' -f1-3 /proc/loadavg), log $log"
    env $SW FMI_DRIVER_STATS=1 $GUARD $log $FMI_BIN explore run $D_ARCH --workers $W --search fd --tail
    echo "[$(date +%T)] $(tail -1 $log)";;
  verify)
    O=$D_OUT/verify; mkdir -p $O
    n=$(wc -l < $D_ARCH/entries.jsonl); bad=0
    if [ -n "${2:-}" ]; then list=$2; else list=$(seq 1 $((n - 1))); fi
    for i in $list; do
      env $E14M $GAME FMI_ENGINE_BIN=$E13_ENGINE_BIN nice -n 10 $FMI_BIN explore verify $D_ARCH --entry $i > $O/$i.out 2>&1 \
        || { bad=$((bad + 1)); echo "entry $i: error $(tail -1 $O/$i.out)"; continue; }
      grep -q '^differences: 0;' $O/$i.out || { bad=$((bad + 1)); echo "entry $i: $(tail -1 $O/$i.out)"; }
    done
    echo "[$(date +%T)] verify $D_ARCH: $(echo $list | wc -w) entries from boot, $bad with differences or errors";;
  stalls)
    name=$(basename $D_ARCH)
    env $SW $FMI_BIN explore stalls $D_ARCH --json $D_OUT/stalls-$name.json > $D_OUT/stalls-$name.md
    head -30 $D_OUT/stalls-$name.md;;
esac
