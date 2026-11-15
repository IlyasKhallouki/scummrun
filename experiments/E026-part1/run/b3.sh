#!/bin/bash
# E026 b3 (plan.md G8, G13; mi-checks-now's copy of E013-robust/integrate/b3.sh): B3's 15 tries (E011's B0 jobs, round
# 2's warm regime: one resident fmi runs them in log order, the job memo off) on Monkey Island with env.sh's ON (MI's
# files named), the engine svm-e024, `--search fd --tail` (fast). The heavy lock is held for the whole run; the
# process runs under the guard at nice $NICE with the idle:sleep lock. Per try a line in OUT/times.jsonl; G12's watcher
# (no python in the fmi tree) beside it. Tries B3_K (default 0-14; a subset is a test of the script, not b3).
# usage: b3.sh OUT
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
game_ok || exit 1
OUT=$1
mkdir -p $OUT; OUTDISK=$(cd $OUT && pwd)
disk_ok $OUTDISK || exit 1
SRC=/var/tmp/fast-mi/e011-r2int/b3f/r2-fast; ARGS="--search fd --tail"
LOCK=/var/tmp/fast-mi/heavy.lock
export E008_DISK_PATH=$OUTDISK E008_FLOOR_GB=$FLOOR_GB
now() { date +%s.%N; }
calc() { awk "BEGIN { printf \"%.3f\", $1 }"; }
exec 201>$LOCK; flock 201
echo "[$(date +%T)] heavy lock taken, load $(cut -d' ' -f1-3 /proc/loadavg)"
touch $OUT/g12.flag; : > $OUT/g12.roots; $BENCH/g12watch.sh $OUT/g12.flag $OUT/g12.log $OUT/g12.roots &
watch=$!
R=$OUT/all-fast
rm -rf $R; mkdir -p $R/jobs $R/logs $R/tmp
ln -s $(readlink -f $SRC/entries) $R/entries
ln -s $(readlink -f $SRC/entries.jsonl) $R/entries.jsonl
for d in $SRC/jobs/j000*; do
  j=$(basename $d); mkdir -p $R/jobs/$j
  sed "s#$SRC#$R#g" $d/job.json > $R/jobs/$j/job.json
done
FIFO=$R/jobs.fifo; rm -f $FIFO; mkfifo $FIFO
env $ON $MI FMI_READING_CACHE=$R/rcache FMI_MEMO= FMI_PLAN_CACHE=$R/plans FMI_POOL_DIR=$R/pool TMPDIR=$R/tmp \
  $GUARD $R/fmi.log $INHIBIT $FMI_BIN try @$FIFO $ARGS &
GPID=$!
exec 7>$FIFO
sleep 1
# the warm process: by its name and /proc cmdline (no pgrep -f)
PID=$(for p in /proc/[0-9]*; do [ "$(cat $p/comm 2>/dev/null)" = fmi ] && tr "\0" " " < $p/cmdline 2>/dev/null | grep -q "^$FMI_BIN try @$FIFO " && echo ${p#/proc/}; done | head -1)
[ -n "$PID" ] || { echo "warm: no process"; exit 1; }
echo $PID >> $OUT/g12.roots
echo "warm all-fast: $FMI_BIN pid $PID ($ARGS; $(echo $ON $MI | wc -w) settings)"
echo "{\"regime\": \"all-fast\", \"bin\": \"$FMI_BIN\", \"bin_sha1\": \"$(sha1sum $FMI_BIN | cut -c1-12)\", \"args\": \"$ARGS\", \"env\": \"$ON $MI\", \"engine\": \"$FMI_ENGINE_BIN\"}" >> $OUT/seqs.jsonl
for k in ${B3_K:-$(seq 0 14)}; do
  j=$(printf "j%05d" $k); d=$R/jobs/$j
  c0=$($BENCH/tree_cpu.sh $PID); t0=$(now)
  echo "$d" >&7
  until grep -q "^done $d " $R/fmi.log 2>/dev/null; do
    kill -0 $PID 2>/dev/null || { echo "warm all-fast died"; break; }
    sleep 0.05
  done
  t1=$(now); c1=$($BENCH/tree_cpu.sh $PID)
  echo "{\"regime\": \"all-fast\", \"job\": \"$j\", \"wall_s\": $(calc "$t1 - $t0"), \"cpu_s\": $(calc "$c1 - $c0"), \"load\": $(cut -d' ' -f1 /proc/loadavg), \"mem_avail_kb\": $(awk '/MemAvailable/{print $2}' /proc/meminfo), \"rss_kb\": $(awk '/VmRSS/{print $2}' /proc/$PID/status 2>/dev/null || echo 0)}" >> $OUT/times.jsonl
  echo "[$(date +%T)] all-fast $j $(tail -1 $OUT/times.jsonl | cut -c1-160)"
  disk_ok $OUTDISK || break
done
exec 7>&-
wait $GPID
sleep 2
tail -1 $R/fmi.log
rm -f $OUT/g12.flag; wait $watch
tail -1 $OUT/g12.log
flock -u 201; exec 201>&-
echo "[$(date +%T)] heavy lock released; done"
