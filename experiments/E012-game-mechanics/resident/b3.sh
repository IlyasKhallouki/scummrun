#!/bin/bash
# E012 resident, end to end: B3's 15 tries in E011 round 2's fast settings (play when ready, the old tail first, the
# Rust lama-first-costs, every round-2 switch on), warm in one resident fmi, each try under the heavy lock; then each
# try against B0's (E011's cmp_tries.py). After delay's b3.sh.
#   b3.sh OUT off|on|on-system|on-mimalloc   on: FMI_READER_LIB=1 (the reading in fmi's process, the job memo
#                                resident, the allocator by phase); on-system, on-mimalloc: FMI_READER_ALLOC=system or
#                                mimalloc throughout (the plan's amendments "the allocator" and "the allocator by phase")
# The binaries: this branch's build (RESIDENT_TARGET, default /var/tmp/fast-mi/target-e012-resident); the engine:
# round 2's (patches 0001-0004, read only).
set -u
SEQ=$2; OUT=$1/$SEQ
here=$(cd "$(dirname "$0")" && pwd)
PY=$here/../../../.venv/bin/python
T=${RESIDENT_TARGET:-/var/tmp/fast-mi/target-e012-resident}
B0=$HOME/.cache/fast-mi-e011/bench/b0
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
source $HOME/.cache/fast-mi-e009/env.sh
export FMI_ENGINE_BIN=/var/tmp/fast-mi/svm-e011-r2int/build/scummvm/scummvm
export FMI_READER_SCOPE=$T/scumm/release/reader-scope
export TMPDIR=$1/tmp E008_DISK_PATH=$1 E008_FLOOR_GB=5
mkdir -p $TMPDIR
ON2="FMI_ENGINE_CUTS=1 FMI_PLAY_BATCH=1 FMI_HIT_CACHE=1 FMI_STONE_KEEP=1 FMI_PLAN_FAST=1 FMI_PLAN_LAMA=rust FMI_READER_PREP=1 FMI_IDLE_CHANGES=1 FMI_PLAYER_DONE=1 FMI_TRY_MEMO=1 FMI_PLANNER_KEEP=1 FMI_PLAY_KEEP=1"
case $SEQ in off) SW="";; on) SW="FMI_READER_LIB=1";; on-system) SW="FMI_READER_LIB=1 FMI_READER_ALLOC=system";;
  on-mimalloc) SW="FMI_READER_LIB=1 FMI_READER_ALLOC=mimalloc";;
  *) echo "off, on, on-system or on-mimalloc"; exit 2;; esac
[ -f $OUT/jobs/j00014/job.json ] || $PY $here/../../E011-speed/bench/prep_after.py $B0 $OUT --settings fast > /dev/null || exit 1
rm -rf $OUT/rcache $OUT/plans $OUT/pool
FIFO=$OUT/jobs.fifo; rm -f $FIFO; mkfifo $FIFO
env -u FMI_MEMO -u FMI_READER_ALLOC $ON2 $SW FMI_READING_CACHE=$OUT/rcache FMI_PLAN_CACHE=$OUT/plans FMI_POOL_DIR=$OUT/pool \
  $GUARD $OUT/fmi.log $T/release/fmi try @$FIFO --search fd --tail &
GP=$!
exec 7>$FIFO
sleep 1
PID=$(pgrep -f "^$T/release/fmi try @$FIFO" | head -1)
[ -n "$PID" ] || { echo "no fmi process"; exit 1; }
echo "[$(date +%T)] $SEQ: fmi pid $PID ($SW)"
for k in $(seq 0 14); do
  j=$(printf "j%05d" $k); d=$OUT/jobs/$j
  rm -rf $d/run $d/new $d/result.json $d/log
  exec 9>$LOCK; flock 9
  t0=$(date +%s.%N)
  echo "$d" >&7
  until grep -q "^done $d " $OUT/fmi.log 2>/dev/null; do
    kill -0 $PID 2>/dev/null || { echo "fmi died"; break; }
    sleep 0.05
  done
  t1=$(date +%s.%N)
  flock -u 9; exec 9>&-
  echo "[$(date +%T)] $SEQ $j $(awk "BEGIN { printf \"%.1f\", $t1 - $t0 }") s, load $(cut -d' ' -f1 /proc/loadavg), fmi RSS $(awk '/VmRSS/{print int($2/1024)}' /proc/$PID/status 2>/dev/null) MB"
  for fs in / /home; do
    df -B1G --output=avail $fs | tail -1 | awk -v f=$fs '$1 < 5 { print "STOP: " f " under 5 GB"; exit 1 }' || break 2
  done
done
exec 7>&-
wait $GP
tail -2 $OUT/fmi.log
$PY $here/../../E011-speed/bench/cmp_tries.py $B0 $OUT --json $OUT/cmp.json | tail -16
