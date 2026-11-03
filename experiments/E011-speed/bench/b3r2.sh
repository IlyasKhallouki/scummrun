#!/bin/bash
# E011 round 2's B3 and B4 (experiments/E011-speed/plan.md, "Benchmark"; Results, round 2): B3's warm regime (one fmi
# process runs the 15 tries in log order and keeps its kept entries, warm planners, reading cache, memo and plan store,
# all empty at the start) for round 1's binaries and round 2's, interleaved try by try so that before and after meet
# the same load (B4's rule, with round 1's binaries as the "before"). One settings group per run:
#   ref   strict windows, FdLamaFirstCosts (--search fd); G11's reference clause;
#   fast  play when ready and the old tail first (--tail); round 1's fast search was Gbfs, round 2's is the Rust
#         lama-first-costs (--search fd with FMI_PLAN_LAMA=rust, in ON2).
# The sequences, one resident process each:
#   r1     round 1's fmi, reader-scope and engine (env-r2.sh's R1_*);
#   r2     round 2's binaries with every round-2 switch on (ON2);
#   r2off  round 2's binaries with every round-2 switch off (OFF2: round 1's behaviour).
# Each try holds the heavy lock while it runs; each process runs under the guard at nice 10. Per try a line in
# OUT/times.jsonl: wall, CPU of the resident process's tree (tree_cpu.sh before and after), the load. G12's watcher
# samples /proc every second for the whole run.
# usage: E011_ENV=$HOME/.cache/fast-mi-e011/env-r2.sh b3r2.sh ref|fast OUT SEQ... [-- FIRST_JOB]
#   B3_MEMO=0: no job memo store (FMI_MEMO unset) in every sequence; E008_FLOOR_GB (the reading cache's disk floor) passes
#   through
set -u
source $HOME/.cache/fast-mi-e011/env.sh
[ -n "${E011_ENV:-}" ] && source "$E011_ENV" # a second env file after env.sh (round 2: env-r2.sh, its binaries)
here=$(cd "$(dirname "$0")" && pwd)
PY=$here/../../../.venv/bin/python
S=$1; OUT=$2; shift 2
seqs=""; first=j00000
while (($#)); do
  if [ "$1" = -- ]; then first=$2; break; fi
  seqs="$seqs $1"; shift
done
B0=$HOME/.cache/fast-mi-e011/bench/b0
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
export TMPDIR=/var/tmp/fast-mi/e011-tmp E008_DISK_PATH=/var/tmp
case $S in ref|fast) ;; *) echo "settings: ref or fast"; exit 2;; esac
args_of() { # SEQ
  case $S-$1 in
    ref-*) echo "--search fd";;
    fast-r1) echo "--search gbfs --tail";;
    fast-*) echo "--search fd --tail";;
  esac
}
env_of() { # SEQ
  case $1 in
    r1) echo "FMI_READER_SCOPE=$R1_READER_SCOPE FMI_ENGINE_BIN=$R1_ENGINE_BIN $OFF2";;
    r2) echo "$ON2";;
    r2off) echo "$OFF2";;
  esac
}
bin_of() { case $1 in r1) echo $R1_FMI_BIN;; *) echo $FMI_BIN;; esac; }
now() { date +%s.%N; }
calc() { awk "BEGIN { printf \"%.3f\", $1 }"; }
mkdir -p $OUT
touch $OUT/g12.flag; : > $OUT/g12.roots; $here/g12watch.sh $OUT/g12.flag $OUT/g12.log $OUT/g12.roots &
watch=$!
declare -A FIFO PID FD GPID
fd=7
for q in $seqs; do
  n=$q-$S; R=$OUT/$n
  [ -f $R/jobs/j00014/job.json ] || $PY $here/prep_after.py $B0 $R --settings $S > /dev/null || exit 1
  mkdir -p $R/logs
  FIFO[$q]=$R/jobs.fifo; rm -f ${FIFO[$q]}; mkfifo ${FIFO[$q]}
  rm -rf $R/rcache $R/memo $R/plans $R/pool
  memo=$R/memo; [ "${B3_MEMO:-1}" = 0 ] && memo=
  env $(env_of $q) FMI_READING_CACHE=$R/rcache FMI_MEMO=$memo FMI_PLAN_CACHE=$R/plans FMI_POOL_DIR=$R/pool \
    $GUARD $R/fmi.log $(bin_of $q) try @${FIFO[$q]} $(args_of $q) &
  GPID[$q]=$!
  eval "exec $fd>${FIFO[$q]}"; FD[$q]=$fd; fd=$((fd + 1))
  sleep 1
  PID[$q]=$(pgrep -f "^$(bin_of $q) try @${FIFO[$q]}" | head -1)
  [ -n "${PID[$q]}" ] || { echo "warm $n: no process"; exit 1; }
  echo ${PID[$q]} >> $OUT/g12.roots
  echo "warm $n: $(bin_of $q) pid ${PID[$q]} ($(args_of $q); $(env_of $q | wc -w) switch settings)"
  echo "{\"regime\": \"$n\", \"bin\": \"$(bin_of $q)\", \"args\": \"$(args_of $q)\", \"env\": \"$(env_of $q)\"}" >> $OUT/seqs.jsonl
done
for k in $(seq 0 14); do
  j=$(printf "j%05d" $k)
  [[ $j < $first ]] && continue
  for q in $seqs; do
    n=$q-$S; d=$OUT/$n/jobs/$j
    rm -rf $d/run $d/new $d/result.json $d/log
    exec 9>$LOCK; flock 9
    c0=$($here/tree_cpu.sh ${PID[$q]}); t0=$(now)
    echo "$d" >&${FD[$q]}
    until grep -q "^done $d " $OUT/$n/fmi.log 2>/dev/null; do
      kill -0 ${PID[$q]} 2>/dev/null || { echo "warm $n died"; break; }
      sleep 0.05
    done
    t1=$(now); c1=$($here/tree_cpu.sh ${PID[$q]})
    flock -u 9; exec 9>&-
    echo "{\"regime\": \"$n\", \"job\": \"$j\", \"wall_s\": $(calc "$t1 - $t0"), \"cpu_s\": $(calc "$c1 - $c0"), \"load\": $(cut -d' ' -f1 /proc/loadavg), \"mem_avail_kb\": $(awk '/MemAvailable/{print $2}' /proc/meminfo)}" >> $OUT/times.jsonl
    echo "[$(date +%T)] $n $j $(tail -1 $OUT/times.jsonl | cut -c1-140)"
  done
  for fs in / /home; do
    df -B1G --output=avail $fs | tail -1 | awk -v f=$fs '$1 < 5 { print "STOP: " f " under 5 GB"; exit 1 }' || break 2
  done
done
for q in $seqs; do eval "exec ${FD[$q]}>&-"; done
for q in $seqs; do wait ${GPID[$q]}; done
sleep 2
for q in $seqs; do tail -1 $OUT/$q-$S/fmi.log; done
rm -f $OUT/g12.flag; wait $watch
tail -1 $OUT/g12.log
echo "[$(date +%T)] done"
