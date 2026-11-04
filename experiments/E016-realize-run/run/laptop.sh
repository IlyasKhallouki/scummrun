#!/bin/bash
# E016 play9 (plan.md "The run: play9"): the demo from boot, seed 1, on the laptop, b = 4, with env.sh's set (play8's
# ALLON less scope growth, with fix A and C-F: E16) and 1 reading in the background, the wall limit 10,800 s, in the new
# archive play9. E014's vm/run.sh makes the guide's copy, the archive (init), every entry verified from boot (verify)
# and the stall table (stalls); the run itself is vm/run.sh's run with the ladder's watcher beside it (watch.sh: the
# ladder every 5 minutes, a clean stop after 40 minutes of wall with no new rung, through FMI_STOP_FILE) and the disk
# rules of plan.md F: at least MIN_FREE_GB (15) free on the run's disk at its start, and once, at a "disk floor" stop,
# the archive and its run files moved to the disk with the most room (OTHER), a link left at the old paths, and the
# run resumed there. Then the ladder over the whole archive (ladder: D_OUT/ladder-final.{json,txt}).
# usage: laptop.sh [init|run|verify|stalls|ladder]...   (default all five in order; run resumes from the archive's files)
set -u
here=$(cd "$(dirname "$0")" && pwd)
export ENV=$here/env.sh A4_BG=${A4_BG:-1} W=${W:-4}
export FMI_STOP_WALL=${FMI_STOP_WALL:-10800} STOP_JOBS=${STOP_JOBS:-100000}
export D_ARCH=/var/tmp/fast-mi/e016-archives/play9 D_RUN=/var/tmp/fast-mi/e016-archives/play9-run
export D_OUT=/var/tmp/fast-mi/e016-play9
MIN_FREE_GB=${MIN_FREE_GB:-15}
OTHER=${OTHER:-$HOME/.cache/fast-mi-e016}
R=$here/../../E014-fate-demo/vm/run.sh
STOPF=$D_OUT/STOP
source $ENV
mkdir -p $D_OUT
[ -e $D_OUT/guide-play.json ] || cp /var/tmp/fast-mi/e014-play-vm4/guide-play.json $D_OUT/
free_gb() { df -P -BG "$1" | awk 'NR == 2 { sub("G", "", $4); print $4 }'; }
# the run's disk: where the archive's real folder is (a link after a relocation)
disk() { local d; d=$(readlink -f $D_ARCH 2>/dev/null || echo $D_ARCH); while [ ! -e $d ]; do d=$(dirname $d); done; echo $d; }
run_once() {
  local dp=$1 n log
  n=$(ls $D_OUT/run*.log 2>/dev/null | wc -l)
  log=$D_OUT/run$([ $n -gt 0 ] && echo -$n).log
  rm -f $STOPF
  echo "[$(date +%T)] start, workers $W, disk $dp ($(free_gb $dp) GB free), load $(cut -d' ' -f1-3 /proc/loadavg), log $log"
  $here/watch.sh $D_ARCH $D_OUT $STOPF &
  local wpid=$!
  env $ALLON $GAME FMI_ENGINE_BIN=$E13_ENGINE_BIN FMI_MEMO= FMI_READING_CACHE=$D_RUN/rcache FMI_PLAN_CACHE=$D_RUN/plans \
    FMI_POOL_DIR=$D_RUN/pool TMPDIR=$D_RUN/tmp E008_DISK_PATH=$dp E008_FLOOR_GB=${FLOOR_GB:-5} E008_HEAVY_LOCK=${HEAVY_LOCK:-} \
    FMI_STOP_FILE=$STOPF FMI_DRIVER_STATS=1 $HOME/.cache/fast-mi-e007/guard.sh $log $FMI_BIN explore run $D_ARCH --workers $W --search fd --tail
  kill $wpid 2>/dev/null; wait $wpid 2>/dev/null
  echo "[$(date +%T)] $(tail -1 $log)"
}
for st in ${@:-init run verify stalls ladder}; do
  case $st in
    init)
      f=$(free_gb /var/tmp)
      [ "$f" -ge $MIN_FREE_GB ] || { echo "init: /var/tmp has $f GB free, under $MIN_FREE_GB GB (plan.md F)"; exit 1; }
      bash $R init || { echo "init failed"; exit 1; };;
    run)
      mkdir -p $D_RUN/tmp
      dp=$(disk); f=$(free_gb $dp)
      [ -e $D_ARCH/summary.json ] || [ "$f" -ge $MIN_FREE_GB ] || { echo "run: $dp has $f GB free, under $MIN_FREE_GB GB (plan.md F)"; exit 1; }
      run_once $dp
      # plan.md F: once, at the disk floor, the archive and its run files go to the disk with the most room
      if [ "$(jq -r .stop $D_ARCH/summary.json 2>/dev/null)" = "disk floor" ] && [ ! -L $D_ARCH ]; then
        mkdir -p $OTHER
        a=$(free_gb $(disk)); b=$(free_gb $OTHER)
        need=$(( $(du -s -BG $D_ARCH $D_RUN | awk '{ sub("G", "", $1); s += $1 } END { print s }') + ${FLOOR_GB:-5} + 1 ))
        if [ "$b" -gt "$a" ] && [ "$b" -ge "$need" ]; then
          echo "[$(date +%T)] disk floor: moving the archive and its run files to $OTHER ($b GB free, $need GB needed)"
          for d in $D_ARCH $D_RUN; do
            mv $d $OTHER/ && ln -s $OTHER/$(basename $d) $d || { echo "relocation of $d failed"; exit 1; }
          done
          rm $D_ARCH/summary.json
          run_once $OTHER
        else
          echo "[$(date +%T)] disk floor: no disk with more room ($OTHER: $b GB free, $need GB needed); the run stays stopped"
        fi
      fi;;
    verify|stalls)
      bash $R $st || { echo "$st failed"; exit 1; };;
    ladder)
      $LADDER $D_ARCH --json $D_OUT/ladder-final.json | tee $D_OUT/ladder-final.txt;;
    *) echo "$st: init, run, verify, stalls or ladder"; exit 2;;
  esac
done
