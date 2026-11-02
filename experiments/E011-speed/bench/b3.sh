#!/bin/bash
# E011 B3 and B4 (experiments/E011-speed/plan.md, "Benchmark"): the 15 tries after, in four sequences, interleaved
# try by try with the Python reference's (B4), so that every regime meets the same load:
#   b4      the reference (bench/before.py, E009's code on svm-w3b), from B0's entries and readings;
#   a-ref   cold: one fmi try process per try, strict windows, FdLamaFirstCosts; no reading cache, memo or plan store;
#   a-fast  cold, the fast settings: Gbfs, the old tail first (--tail), play when ready;
#   b-ref   warm: one fmi process runs the 15 tries in log order (the job folders through a FIFO), keeping its kept
#           entries, warm planners, and its reading cache, memo and plan store (empty at the start); strict, FD;
#   b-fast  warm, the fast settings.
# Every step runs under the heavy lock (the warm processes hold it while a try runs) and the guard at nice 10. Per step
# a line in SEQ/times.jsonl: wall, and CPU seconds of the step's process tree (a cold step: the time keyword's user and
# sys; a warm try: tree_cpu.sh of the resident process before and after). G12's watcher samples /proc every second.
# usage: b3.sh [B] [FIRST_JOB]      (B: ~/.cache/fast-mi-e011/bench; B0 in B/b0, outputs B/b3 and B/b4)
set -u
source $HOME/.cache/fast-mi-e011/env.sh
here=$(cd "$(dirname "$0")" && pwd)
PY=$here/../../../.venv/bin/python
B=${1:-$HOME/.cache/fast-mi-e011/bench}
first=${2:-j00000}
S=$B/b3; LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
mkdir -p $S $B/b4/jobs $B/b4/logs
for n in entries entries.jsonl; do [ -e $B/b4/$n ] || ln -s $B/b0/$n $B/b4/$n; done
for r in a-ref a-fast b-ref b-fast; do
  [ -f $S/$r/jobs/j00014/job.json ] || $PY $here/prep_after.py $B/b0 $S/$r --settings ${r#*-} || exit 1
  mkdir -p $S/$r/logs
done
export E008_DISK_PATH=/var/tmp TMPDIR=/var/tmp/fast-mi/e011-tmp
args_of() { case $1 in *-ref) echo "--search fd";; *-fast) echo "--search gbfs --tail";; esac; }
now() { date +%s.%N; }
calc() { awk "BEGIN { printf \"%.2f\", $1 }"; }
put() { echo "$1" >> $S/times.jsonl; }
load() { cut -d' ' -f1 /proc/loadavg; }
gwall() { grep -o "GUARD: rc [0-9]* wall [0-9.]*" $1 | tail -1 | awk '{print $5}'; }  # the command's wall, lock wait excluded

# G12: the watcher, for the whole run
touch $S/g12.flag; $here/g12watch.sh $S/g12.flag $S/g12.log &
watch=$!

# the warm processes: fmi try @FIFO, each under the guard, with its own stores
declare -A FIFO PID GPID
for r in b-ref b-fast; do
  FIFO[$r]=$S/$r/jobs.fifo; rm -f ${FIFO[$r]}; mkfifo ${FIFO[$r]}
  rm -rf $S/$r/rcache $S/$r/memo $S/$r/plans $S/$r/pool
  FMI_READING_CACHE=$S/$r/rcache FMI_MEMO=$S/$r/memo FMI_PLAN_CACHE=$S/$r/plans FMI_POOL_DIR=$S/$r/pool \
    $GUARD $S/$r/fmi.log $FMI_BIN try @${FIFO[$r]} $(args_of $r) &
  GPID[$r]=$!
  if [ $r = b-ref ]; then exec 7>${FIFO[$r]}; else exec 8>${FIFO[$r]}; fi
  sleep 1
  PID[$r]=$(pgrep -f "try @${FIFO[$r]}" | while read p; do [ "$(cat /proc/$p/comm)" = fmi ] && echo $p; done | head -1)
  echo "warm $r: fmi pid ${PID[$r]}"
done

cold() { # REGIME JOB
  local r=$1 j=$2 d=$S/$1/jobs/$2 t0 t1 tm
  rm -rf $d/run $d/new $d/result.json $d/log $S/$r/readings/*-$j
  TIMEFORMAT="%R %U %S"
  t0=$(now)
  { time flock $LOCK env FMI_PLAN_CACHE= FMI_POOL_DIR=$S/$r/pool-$j $GUARD $S/$r/logs/$j.log \
      $FMI_BIN try $d $(args_of $r) ; } 2> $S/$r/logs/$j.time
  t1=$(now); tm=($(tail -1 $S/$r/logs/$j.time))
  put "{\"regime\": \"$r\", \"job\": \"$j\", \"wall_s\": $(gwall $S/$r/logs/$j.log), \"cpu_s\": $(calc "${tm[1]} + ${tm[2]}"), \"load\": $(load)}"
  rm -rf $S/$r/pool-$j
}

warm() { # REGIME JOB
  local r=$1 j=$2 d=$S/$1/jobs/$2 c0 c1 t0 t1 fd
  rm -rf $d/run $d/new $d/result.json $d/log
  fd=7; [ $r = b-fast ] && fd=8
  exec 9>$LOCK; flock 9
  c0=$($here/tree_cpu.sh ${PID[$r]}); t0=$(now)
  echo "$d" >&$fd
  until grep -q "^done $d " $S/$r/fmi.log 2>/dev/null; do
    kill -0 ${PID[$r]} 2>/dev/null || { echo "warm $r died"; break; }
    sleep 0.05
  done
  t1=$(now); c1=$($here/tree_cpu.sh ${PID[$r]})
  flock -u 9; exec 9>&-
  put "{\"regime\": \"$r\", \"job\": \"$j\", \"wall_s\": $(calc "$t1 - $t0"), \"cpu_s\": $(calc "$c1 - $c0"), \"load\": $(load)}"
}

before() { # JOB
  local j=$1 t0 t1 tm
  rm -rf $B/b4/jobs/$j $B/b4/readings/*-$j
  TIMEFORMAT="%R %U %S"
  t0=$(now)
  { time flock $LOCK $GUARD $B/b4/logs/$j.log $PY -u $here/before.py $B/b4/jobs/$j --seq $B/b0 ; } 2> $B/b4/logs/$j.time
  t1=$(now); tm=($(tail -1 $B/b4/logs/$j.time))
  put "{\"regime\": \"b4\", \"job\": \"$j\", \"wall_s\": $(gwall $B/b4/logs/$j.log), \"cpu_s\": $(calc "${tm[1]} + ${tm[2]}"), \"load\": $(load)}"
}

for n in $(seq 0 14); do
  j=$(printf "j%05d" $n)
  [[ $j < $first ]] && continue
  echo "[$(date +%T)] $j load $(load)"
  before $j;      echo "  b4     $(tail -1 $S/times.jsonl | cut -c1-120)"; grep -h "^CHECK" $B/b4/logs/$j.log | cut -c1-160
  cold a-ref $j;  echo "  a-ref  $(tail -1 $S/times.jsonl | cut -c1-120)"
  cold a-fast $j; echo "  a-fast $(tail -1 $S/times.jsonl | cut -c1-120)"
  warm b-ref $j;  echo "  b-ref  $(tail -1 $S/times.jsonl | cut -c1-120)"
  warm b-fast $j; echo "  b-fast $(tail -1 $S/times.jsonl | cut -c1-120)"
  df -B1G --output=avail /home | tail -1 | awk '$1 < 8 { print "STOP: /home under 8 GB"; exit 1 }' || break
done
exec 7>&- 8>&-
wait ${GPID[b-ref]} ${GPID[b-fast]}
tail -1 $S/b-ref/fmi.log; tail -1 $S/b-fast/fmi.log
sleep 2
rm -f $S/g12.flag; wait $watch
tail -1 $S/g12.log
echo "[$(date +%T)] B3/B4 done"
