#!/bin/bash
# E011 route, G11 with the Rust lama-first-costs: B3's warm sequence (bench/b3.sh's b-fast regime: one fmi process
# runs E009's 15 kitchen tries in log order, keeping its kept entries, warm planners, reading cache, memo and plan
# store, empty at the start) with fmi-plan serving FdLamaFirstCosts by taskcore's lama.rs (FMI_PLAN_LAMA=rust).
# Settings: fast (play when ready, --tail, --search fd) or strict (strict windows, --search fd). Each try runs under the
# heavy lock and the guard at nice 10; per try one line in SEQ/times.jsonl (wall, CPU of the process tree). Then each
# try against B0's (bench/cmp_tries.py) and every new entry verified from boot (bench/verify_new.sh).
# usage: b3lama.sh FMI_BIN OUT fast|strict
set -u
source $HOME/.cache/fast-mi-e011/env.sh
here=$(cd "$(dirname "$0")" && pwd)
bench=$here/../bench
PY=$here/../../../.venv/bin/python
FMI=$1; O=$2; mode=$3
B=$HOME/.cache/fast-mi-e011/bench
S=$O/b-lama-$mode; LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
case $mode in fast) settings=fast; args="--search fd --tail";; strict) settings=ref; args="--search fd";; *) exit 2;; esac
rm -rf $S; mkdir -p $S/logs
$PY $bench/prep_after.py $B/b0 $S --settings $settings || exit 1
export E008_DISK_PATH=/var/tmp TMPDIR=/var/tmp/fast-mi/e011-tmp
now() { date +%s.%N; }
calc() { awk "BEGIN { printf \"%.2f\", $1 }"; }
FIFO=$S/jobs.fifo; rm -f $FIFO; mkfifo $FIFO
FMI_PLAN_LAMA=rust FMI_READING_CACHE=$S/rcache FMI_MEMO=$S/memo FMI_PLAN_CACHE=$S/plans FMI_POOL_DIR=$S/pool \
  $GUARD $S/fmi.log $FMI try @$FIFO $args &
gpid=$!
exec 8>$FIFO
sleep 1
pid=$(pgrep -f "try @$FIFO" | while read p; do [ "$(cat /proc/$p/comm)" = fmi ] && echo $p; done | head -1)
echo "[$(date +%T)] warm b-lama-$mode: fmi pid $pid ($args, FMI_PLAN_LAMA=rust)"
for n in $(seq 0 14); do
  j=$(printf "j%05d" $n); d=$S/jobs/$j
  rm -rf $d/run $d/new $d/result.json $d/log
  exec 9>$LOCK; flock 9
  c0=$($bench/tree_cpu.sh $pid); t0=$(now)
  echo "$d" >&8
  until grep -q "^done $d " $S/fmi.log 2>/dev/null; do
    kill -0 $pid 2>/dev/null || { echo "warm b-lama died"; break; }
    sleep 0.05
  done
  t1=$(now); c1=$($bench/tree_cpu.sh $pid)
  flock -u 9; exec 9>&-
  echo "{\"regime\": \"b-lama-$mode\", \"job\": \"$j\", \"wall_s\": $(calc "$t1 - $t0"), \"cpu_s\": $(calc "$c1 - $c0"), \"load\": $(cut -d' ' -f1 /proc/loadavg)}" >> $S/times.jsonl
  echo "  $j $(tail -1 $S/times.jsonl | cut -c1-120)"
done
exec 8>&-
wait $gpid
tail -1 $S/fmi.log
$PY $bench/cmp_tries.py $B/b0 $S --json $S/cmp.json | tail -20
bash $bench/verify_new.sh $S $S/verify 2>&1 | tail -3
rm -rf $S/pool $S/rcache $S/memo
