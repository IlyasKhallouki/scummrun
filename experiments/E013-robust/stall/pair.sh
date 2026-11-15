#!/bin/bash
# E013 component stall, rule (2) (plan.md, stall's amendment (2)): E012's driver smoke run on Monkey Island
# (integrate/smoke.sh's settings: the kitchen's stage, goal, guide and rooms, E009's switches, play when ready, commit
# as finished, the falsifier at N jobs) with b = 1, ON3 and every E012 switch on, in switch set SET, twice on one
# build: without stall's switches ("off") and with FMI_STALLS=1 FMI_FREE_SHIFT=1 ("on"). Then:
#   - the two runs' tries are the same (`fmi check tries`), and so are their job files, archive rows, cells and log
#     lines, timing fields and the runs' own folder names aside;
#   - every entry of each run verifies from boot (`fmi explore verify`, one at a time, 0 differences);
#   - the "on" run's stalls.json is stall-report's table over the same files (C3's check on a real run), and stall-report's
#     C1 checks hold over each run.
# SET: e12 (E012's switches, E012's engine) or e14 (E012's and E014's mechanism switches of amendment (2) (b), E014's
# engine). Under the guard; the caller holds the heavy lock across the whole run (fmi with the reader resident is above
# 1 GB), so the driver's own lock per heavy job is off (E008_HEAVY_LOCK=none).
# usage: FMI_BIN=... STALL_REPORT=... flock /var/tmp/fast-mi/heavy.lock pair.sh SET OUT [N]
set -u
here=$(cd "$(dirname "$0")" && pwd)
WT=$(cd $here/../../.. && pwd)
BIN=${FMI_BIN:?the fmi binary}
SR=${STALL_REPORT:?the stall-report binary}
SET=${1:?e12 or e14}; OUT=${2:?the output folder}; N=${3:-4}
# E014's tools (fmi-plan, fmi-reading, the reader's binaries) beside this fmi, as E014's demo/env.sh takes them
export E12T=${E12T:-/var/tmp/fast-mi/target-e014}
source $WT/experiments/E012-game-mechanics/integrate/env.sh
FMI_BIN=$BIN
E14P="FMI_EGO_VALUES=1 FMI_MERGE_DEDUP=1 FMI_READ_CUT=1 FMI_STAG_SWEEP=1 FMI_VERBS_VAR_ON=1 FMI_NOVEL_VERBS=1 FMI_NOVEL_GUIDE=1"
case $SET in
  e12) ENG=$E12_ENGINE_BIN; SETSW="";;
  e14) ENG=/var/tmp/fast-mi/svm-e014/build/scummvm/scummvm
       SETSW="SPEEDRUN_IDLE=handler SPEEDRUN_END_SITE=1 FMI_GOAL_END=1 FMI_READ_EVENTS=1 $E14P";;
  *) echo "SET: e12 or e14"; exit 2;;
esac
GUARD=$HOME/.cache/fast-mi-e007/guard.sh
[ -e $OUT ] && { echo "$OUT exists"; exit 2; }
mkdir -p $OUT
run() {  # run NAME STALL_SWITCHES
  local O=$OUT/$1 A=$OUT/$1/kitchen
  mkdir -p $O/tmp
  local SW="$ON3 $E12ON $SETSW $2 FMI_ENGINE_BIN=$ENG FMI_MEMO= FMI_READING_CACHE=$O/rcache FMI_PLAN_CACHE=$O/plans FMI_POOL_DIR=$O/pool TMPDIR=$O/tmp E008_DISK_PATH=/var/tmp E008_FLOOR_GB=5 E008_HEAVY_LOCK=none"
  echo "[$(date +%T)] $1: start ($SET, '$2'), load $(cut -d' ' -f1-3 /proc/loadavg)"
  env $SW $FMI_BIN explore init $A --stage kitchen --goal owner:566=1 --guide $HOME/.cache/fast-mi-e009/guides/kitchen.json \
    --allowed 28,29,33,34,35,38,41,83,85,96 --workers 1 --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts \
    --stop-jobs $N --ready --commit-as-finished > $O/init.log 2>&1 || { cat $O/init.log; exit 1; }
  local t0=$(date +%s.%N)
  env $SW $GUARD $O/run.log $FMI_BIN explore run $A --workers 1 --search fd --tail
  local t1=$(date +%s.%N)
  echo "[$(date +%T)] $1: run $(awk "BEGIN{print $t1 - $t0}") s; $(tail -1 $O/run.log)"
  local n=$(wc -l < $A/entries.jsonl) bad=0
  for ((i = 1; i < n; i++)); do
    env $SETSW FMI_ENGINE_BIN=$ENG nice -n 10 $FMI_BIN explore verify $A --entry $i > $O/verify-$i.out 2>&1 || bad=$((bad + 1))
    grep -q '^differences: 0;' $O/verify-$i.out || bad=$((bad + 1))
  done
  echo "[$(date +%T)] $1: verify: $((n - 1)) entries from boot, $bad with differences or errors"
  nice -n 10 $SR $A --json $O/stall-report.json --md $O/stall-report.md $([ -e $A/stalls.json ] && echo --live $A/stalls.json) > $O/stall-report.out 2>&1
  echo "[$(date +%T)] $1: stall-report rc $?: $(grep -c '^- PASS' $O/stall-report.md) PASS, $(grep -c '^- FAIL' $O/stall-report.md) FAIL; stalls.json $([ -e $A/stalls.json ] && echo written || echo none)"
  rm -rf $O/rcache $O/plans $O/pool $O/tmp
}
run off ""
run on "FMI_STALLS=1 FMI_FREE_SHIFT=1"
# the two runs against each other
A=$OUT/off/kitchen; B=$OUT/on/kitchen
env $SETSW $FMI_BIN check tries $A $B --json $OUT/tries.json > $OUT/tries.out 2>&1; rc=$?
echo "[$(date +%T)] fmi check tries off on: rc $rc; $(tail -1 $OUT/tries.out)"
# the files, each run's own folder written as OUT and the timing fields dropped
norm() { sed -e "s#$OUT/off#RUN#g" -e "s#$OUT/on#RUN#g"; }
LOGKEYS='{job, entry, cell, chosen, commit_index, outcome, error, rc, n, key, selectable, scope, targets, tried, job_tried, exhausted, deferred, dead, dead_entries, goal_entries, new_entries, new_cells, new_r, new_reps, new_rooms, new_t, t_new, t_size}'
diffs=0
for f in entries.jsonl cells.json; do
  cmp -s <(norm < $A/$f) <(norm < $B/$f) || { echo "differs: $f"; diffs=$((diffs + 1)); }
done
cmp -s <(jq -c "$LOGKEYS" $A/log.jsonl | norm) <(jq -c "$LOGKEYS" $B/log.jsonl | norm) || { echo "differs: log.jsonl"; diffs=$((diffs + 1)); }
for j in $(ls $A/jobs) ; do
  cmp -s <(norm < $A/jobs/$j/job.json) <(norm < $B/jobs/$j/job.json) || { echo "differs: jobs/$j/job.json"; diffs=$((diffs + 1)); }
done
[ "$(ls $A/jobs)" == "$(ls $B/jobs)" ] || { echo "differs: the jobs"; diffs=$((diffs + 1)); }
echo "[$(date +%T)] files: $(ls $A/jobs | wc -l) jobs, $(wc -l < $A/entries.jsonl) entries, $(grep -c . $A/log.jsonl) log lines; $diffs differ"
# the readings' large files, once compared (the archives' rows, results and records stay)
find $OUT/off/kitchen $OUT/on/kitchen -type f -size +5M -delete
