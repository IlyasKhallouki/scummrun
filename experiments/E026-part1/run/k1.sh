#!/bin/bash
# E026 K1a/K1b (plan.md G9, G10, G13; mi-checks-now's copy of E016-realize-run/run/k1.sh): Monkey Island's kitchen by
# search from boot with env.sh's ON (MI's files named), W workers (3: G9), a 60-minute wall (FMI_STOP_WALL), E012's
# init settings; the run starts only on an idle laptop (env.sh idle_ok), under the guard (the run's and the driver's,
# E008_GUARD) and the idle:sleep lock, with watch.sh beside it (Part I's ladder and key facts, MemAvailable and the
# driver's RSS every INTERVAL s, the first differing return; no stop asked: WAIT=0). verify: the goal chain from boot
# with ON; stalls: stall-report with --project (G10's 14 x T_K1); prune: the readings' bulk and the engines' folders.
# The disk floor is 2 GB on the output disk. Outputs under K1_OUT (default $O26/k1a).
# usage: [W=3] [K1_OUT=DIR] [K1_WALL=s] k1.sh [init|run|verify|stalls|prune]...
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
game_ok || exit 1
K1_OUT=${K1_OUT:-$O26/k1a}
K_ARCH=$K1_OUT/archive
K_RUN=$K1_OUT/run
K_OUT=$K1_OUT/out
W=${W:-3}
mkdir -p $K_OUT $K_RUN/tmp
disk_ok $K1_OUT || exit 1
SW="$ON $MI FMI_MEMO= FMI_READING_CACHE=$K_RUN/rcache FMI_PLAN_CACHE=$K_RUN/plans FMI_POOL_DIR=$K_RUN/pool TMPDIR=$K_RUN/tmp E008_DISK_PATH=$(cd $K1_OUT && pwd) E008_FLOOR_GB=$FLOOR_GB E008_GUARD=$GUARD FMI_STOP_WALL=${K1_WALL:-3600}"
for st in ${@:-init run verify stalls prune}; do
  case $st in
    init)
      [ -e $K_ARCH ] && { echo "$K_ARCH exists"; exit 2; }
      env $SW nice -n $NICE $FMI_BIN explore init $K_ARCH --stage kitchen --goal owner:566=1 \
        --guide $HOME/.cache/fast-mi-e009/guides/kitchen.json --allowed 28,29,33,34,35,38,41,83,85,96 --workers $W \
        --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts --stop-jobs 100000 --ready --commit-as-finished \
        > $K_OUT/init.log 2>&1 || { cat $K_OUT/init.log; exit 1; }
      cat $K_OUT/init.log;;
    run)
      idle_ok || exit 1
      log=$K_OUT/run.log
      echo "[$(date +%T)] start, workers $W, load $(cut -d' ' -f1-3 /proc/loadavg), MemAvailable $(awk '/MemAvailable/{print $2}' /proc/meminfo) kB, log $log"
      WAIT=0 RUNGS_FILE=$RUNGS/rungs-part1.json $here/watch.sh $K_ARCH $K_OUT/watch $K_OUT/STOP &
      wpid=$!
      t0=$(date +%s)
      env $SW FMI_DRIVER_STATS=1 FMI_STOP_FILE=$K_OUT/STOP $GUARD $log $INHIBIT $FMI_BIN explore run $K_ARCH --workers $W --search fd --tail
      # the watcher's last line comes when summary.json appears; a run killed by the guard writes none
      for i in $(seq 30); do kill -0 $wpid 2>/dev/null || break; sleep 1; done
      kill $wpid 2>/dev/null; wait $wpid 2>/dev/null
      echo "[$(date +%T)] $(tail -1 $log) (script wall $(( $(date +%s) - t0 )) s)"
      jq -c '{stop, detail, jobs, entries, goal_entries}' $K_ARCH/summary.json;;
    verify)
      g=$(jq '.goal_entries[0] // empty' $K_ARCH/summary.json)
      [ -n "$g" ] || { echo "verify: no goal entry ($(jq -r .stop $K_ARCH/summary.json))"; exit 1; }
      chain=$(jq -rs --argjson g $g '. as $r | [$g | recurse(if ($r[.].parent // 0) != 0 then $r[.].parent else empty end)] | reverse | map(tostring) | join(" ")' $K_ARCH/entries.jsonl)
      bad=0; n=0; mkdir -p $K_OUT/verify
      for i in $chain; do
        n=$((n + 1))
        env $ON $MI nice -n $NICE $FMI_BIN explore verify $K_ARCH --entry $i > $K_OUT/verify/$i.out 2>&1 || { bad=$((bad + 1)); continue; }
        grep -q '^differences: 0;' $K_OUT/verify/$i.out || bad=$((bad + 1))
      done
      t=$(jq -r 'select(.id == '$g') | .job' $K_ARCH/entries.jsonl)
      echo "[$(date +%T)] K1: goal entry $g (job $t), its chain of $n entries verified from boot, $bad with differences or errors; $(tail -1 $K_OUT/verify/$g.out)"
      jq -c 'select(.id == '$g') | {id, job, frame, tick, room}' $K_ARCH/entries.jsonl;;
    stalls)
      nice -n $NICE $CHK/stall-report $K_ARCH --project --json $K_OUT/stall-report.json --md $K_OUT/stall-report.md > $K_OUT/stall-report.out 2>&1
      echo "[$(date +%T)] stall-report rc $?: $(grep -c '^- PASS' $K_OUT/stall-report.md) PASS, $(grep -c '^- FAIL' $K_OUT/stall-report.md) FAIL; $(jq -r .projection.line $K_OUT/stall-report.json)"
      echo "[$(date +%T)] tries $(grep -c . $K_ARCH/log.jsonl), mean try $(jq -s 'map(.wall_s // 0) | add / length | . * 10 | round / 10' $K_ARCH/log.jsonl) s, readings at the bound $(jq -r 'select((.reading.why // "") | startswith("bound")) | .job' $K_ARCH/log.jsonl | wc -l), driver peak $(jq -s 'map(.rusage.maxrss_mb // 0) | max' $K_ARCH/log.jsonl) MB";;
    prune)
      # the readings' bulk is not read after the verify (reading.json and the start rows stay); the engines' folders go
      find $K1_OUT \( -name 'reading.tc' -o -name 'reading.actions.jsonl' -o -name 'reading.lists.jsonl' -o -name 'reading.idx' \
        -o -name 'reading.reached' \) -delete
      rm -rf $K_RUN/pool $K_RUN/tmp
      echo "[$(date +%T)] pruned: $(du -sh $K1_OUT | cut -f1)";;
    *) echo "$st: init, run, verify, stalls or prune"; exit 2;;
  esac
done
