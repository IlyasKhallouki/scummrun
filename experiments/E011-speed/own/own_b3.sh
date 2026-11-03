#!/bin/bash
# E011 round 2, item own: B3's warm regime (bench/b3.sh's warm(): one fmi process runs the 15 tries in log order, its
# kept entries, reading cache, memo and plan store kept) for two binaries, interleaved try by try so both meet the same
# load. Each try holds the heavy lock while it runs; each process runs under the guard at nice 10.
# usage: own_b3.sh ref|fast OUT NAME_A BIN_A NAME_B BIN_B [FIRST_JOB]
#   OUT/NAME_X: the sequence (prep_after.py's job folders from B0), its logs and stores; OUT/times.jsonl: per try wall
#   and CPU (tree_cpu.sh) and the load. B0: ~/.cache/fast-mi-e011/bench/b0.
set -u
source $HOME/.cache/fast-mi-e011/env.sh
here=$(cd "$(dirname "$0")" && pwd)
bench=$here/../bench
PY=$here/../../../.venv/bin/python
S=$1; OUT=$2; first=${7:-j00000}
B0=$HOME/.cache/fast-mi-e011/bench/b0
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
export TMPDIR=/var/tmp/fast-mi/e011-tmp E008_DISK_PATH=/var/tmp
case $S in ref) args="--search fd";; fast) args="--search gbfs --tail";; *) echo "settings: ref or fast"; exit 2;; esac
declare -A BIN FIFO PID FD
BIN[$3]=$4; BIN[$5]=$6
names="$3 $5"
mkdir -p $OUT
fd=7
for n in $names; do
  R=$OUT/$n
  [ -f $R/jobs/j00014/job.json ] || $PY $bench/prep_after.py $B0 $R --settings $S > /dev/null || exit 1
  mkdir -p $R/logs
  FIFO[$n]=$R/jobs.fifo; rm -f ${FIFO[$n]}; mkfifo ${FIFO[$n]}
  rm -rf $R/rcache $R/memo $R/plans $R/pool
  FMI_READING_CACHE=$R/rcache FMI_MEMO=$R/memo FMI_PLAN_CACHE=$R/plans FMI_POOL_DIR=$R/pool \
    $GUARD $R/fmi.log ${BIN[$n]} try @${FIFO[$n]} $args &
  eval "exec $fd>${FIFO[$n]}"; FD[$n]=$fd; fd=$((fd + 1))
  sleep 1
  PID[$n]=$(pgrep -f "try @${FIFO[$n]}" | while read p; do [ "$(cat /proc/$p/comm)" = fmi ] && echo $p; done | head -1)
  echo "warm $n: ${BIN[$n]} pid ${PID[$n]}"
done
now() { date +%s.%N; }
calc() { awk "BEGIN { printf \"%.3f\", $1 }"; }
for k in $(seq 0 14); do
  j=$(printf "j%05d" $k)
  [[ $j < $first ]] && continue
  for n in $names; do
    d=$OUT/$n/jobs/$j
    rm -rf $d/run $d/new $d/result.json $d/log
    exec 9>$LOCK; flock 9
    c0=$($bench/tree_cpu.sh ${PID[$n]}); t0=$(now)
    echo "$d" >&${FD[$n]}
    until grep -q "^done $d " $OUT/$n/fmi.log 2>/dev/null; do
      kill -0 ${PID[$n]} 2>/dev/null || { echo "warm $n died"; break; }
      sleep 0.05
    done
    t1=$(now); c1=$($bench/tree_cpu.sh ${PID[$n]})
    flock -u 9; exec 9>&-
    echo "{\"seq\": \"$n\", \"job\": \"$j\", \"wall_s\": $(calc "$t1 - $t0"), \"cpu_s\": $(calc "$c1 - $c0"), \"load\": $(cut -d' ' -f1 /proc/loadavg)}" >> $OUT/times.jsonl
    echo "[$(date +%T)] $n $j $(tail -1 $OUT/times.jsonl | cut -c1-100)"
  done
  df -B1G --output=avail / | tail -1 | awk '$1 < 2 { print "STOP: / under 2 GB"; exit 1 }' || break
done
for n in $names; do eval "exec ${FD[$n]}>&-"; done
wait
echo "[$(date +%T)] done"
