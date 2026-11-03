#!/bin/bash
# E012 integration: B3's 15 tries (E011's B0 jobs, round 2's warm regime: one resident fmi per sequence runs the 15
# tries in log order, the job memo off as round 2's B3 ran them) for round 2 and branch e012, interleaved try by try so
# that they meet the same load (E011's b3r2.sh, with E012's sequences and no Python: the job folders are round 2's own
# B3 folders' job.json with the paths changed, as try/gates.sh makes them). One settings group per run:
#   ref   strict windows, FdLamaFirstCosts (--search fd)
#   fast  play when ready and the old tail first, the Rust lama-first-costs (--search fd --tail; FMI_PLAN_LAMA=rust)
# The sequences:
#   d     round 2 with the no-plan proofs (R2D_FMI_BIN, its reader-scope, round 2's engine), everything on (ON3)
#   eoff  e012 with round 2's switches on (ON3) and every E012 switch unset, end's engine (patches 0001-0005)  (G0)
#   e     e012 with everything on: ON3 and every E012 switch (E12ON), end's engine                            (G-on)
# The heavy lock is held for the whole run (E011's amendment "the heavy lock held across the fast sequence"); each
# process runs under the guard at nice 10. Per try a line in OUT/times.jsonl: wall, CPU of the resident process's tree,
# the load. G12's watcher samples /proc every second, rooted at the sequences' resident processes.
# usage: ENV=integrate/env.sh b3.sh ref|fast OUT SEQ...
set -u
source "${ENV:?an env file (integrate/env.sh)}"
here=$(cd "$(dirname "$0")" && pwd)
X=$WT11/experiments/E011-speed/bench
S=$1; OUT=$2; shift 2
seqs="$*"
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
export E008_DISK_PATH=/var/tmp E008_FLOOR_GB=5
case $S in
  ref) SRC=/var/tmp/fast-mi/e011-r2int/b3/r2-ref; ARGS="--search fd";;
  fast) SRC=/var/tmp/fast-mi/e011-r2int/b3f/r2-fast; ARGS="--search fd --tail";;
  *) echo "settings: ref or fast"; exit 2;;
esac
env_of() { # SEQ
  case $1 in
    d) echo "$ON3 $E12OFF FMI_READER_SCOPE=$R2D_READER_SCOPE FMI_ENGINE_BIN=$R2_ENGINE_BIN";;
    eoff) echo "$ON3 $E12OFF FMI_ENGINE_BIN=$E12_ENGINE_BIN";;
    e) echo "$ON3 $E12ON FMI_ENGINE_BIN=$E12_ENGINE_BIN";;
  esac
}
bin_of() { case $1 in d) echo $R2D_FMI_BIN;; *) echo $FMI_BIN;; esac; }
now() { date +%s.%N; }
calc() { awk "BEGIN { printf \"%.3f\", $1 }"; }
mkdir -p $OUT
exec 201>$LOCK; flock 201
echo "[$(date +%T)] heavy lock taken, load $(cut -d' ' -f1-3 /proc/loadavg)"
touch $OUT/g12.flag; : > $OUT/g12.roots; $X/g12watch.sh $OUT/g12.flag $OUT/g12.log $OUT/g12.roots &
watch=$!
declare -A FIFO PID FD GPID
fd=7
for q in $seqs; do
  n=$q-$S; R=$OUT/$n
  rm -rf $R; mkdir -p $R/jobs $R/logs $R/tmp
  ln -s $(readlink -f $SRC/entries) $R/entries
  ln -s $(readlink -f $SRC/entries.jsonl) $R/entries.jsonl
  for d in $SRC/jobs/j000*; do
    j=$(basename $d); mkdir -p $R/jobs/$j
    sed "s#$SRC#$R#g" $d/job.json > $R/jobs/$j/job.json
  done
  FIFO[$q]=$R/jobs.fifo; rm -f ${FIFO[$q]}; mkfifo ${FIFO[$q]}
  env $(env_of $q) FMI_READING_CACHE=$R/rcache FMI_MEMO= FMI_PLAN_CACHE=$R/plans FMI_POOL_DIR=$R/pool TMPDIR=$R/tmp \
    $GUARD $R/fmi.log $(bin_of $q) try @${FIFO[$q]} $ARGS &
  GPID[$q]=$!
  eval "exec $fd>${FIFO[$q]}"; FD[$q]=$fd; fd=$((fd + 1))
  sleep 1
  PID[$q]=$(pgrep -f "^$(bin_of $q) try @${FIFO[$q]}" | head -1)
  [ -n "${PID[$q]}" ] || { echo "warm $n: no process"; exit 1; }
  echo ${PID[$q]} >> $OUT/g12.roots
  echo "warm $n: $(bin_of $q) pid ${PID[$q]} ($ARGS; $(env_of $q | wc -w) settings)"
  echo "{\"regime\": \"$n\", \"bin\": \"$(bin_of $q)\", \"bin_sha1\": \"$(sha1sum $(bin_of $q) | cut -c1-12)\", \"args\": \"$ARGS\", \"env\": \"$(env_of $q)\"}" >> $OUT/seqs.jsonl
done
for k in $(seq 0 14); do
  j=$(printf "j%05d" $k)
  for q in $seqs; do
    n=$q-$S; d=$OUT/$n/jobs/$j
    c0=$($X/tree_cpu.sh ${PID[$q]}); t0=$(now)
    echo "$d" >&${FD[$q]}
    until grep -q "^done $d " $OUT/$n/fmi.log 2>/dev/null; do
      kill -0 ${PID[$q]} 2>/dev/null || { echo "warm $n died"; break; }
      sleep 0.05
    done
    t1=$(now); c1=$($X/tree_cpu.sh ${PID[$q]})
    echo "{\"regime\": \"$n\", \"job\": \"$j\", \"wall_s\": $(calc "$t1 - $t0"), \"cpu_s\": $(calc "$c1 - $c0"), \"load\": $(cut -d' ' -f1 /proc/loadavg), \"mem_avail_kb\": $(awk '/MemAvailable/{print $2}' /proc/meminfo), \"rss_kb\": $(awk '/VmRSS/{print $2}' /proc/${PID[$q]}/status 2>/dev/null || echo 0)}" >> $OUT/times.jsonl
    echo "[$(date +%T)] $n $j $(tail -1 $OUT/times.jsonl | cut -c1-160)"
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
flock -u 201; exec 201>&-
echo "[$(date +%T)] heavy lock released; done"
