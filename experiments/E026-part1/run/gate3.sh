#!/bin/bash
# E026 G3 (plan.md C1, the reader switch wait_queued), with env.sh's ON and FMI_READ_SWITCHES set to QUEUED (E022
# env.sh:51's list with wait_queued in place of wait_both; E26 held it when G3 ran, 2026-11-10 02:38-02:52):
#  (a) b3's j00006 (scope [28,33,35,38]) cold and alone (probe.sh); gate: its reading <= 90 s, not bounded;
#  (b) the demo's room-48 job (E022's entry 28025, ~/.cache/fast-mi-e022/r48-data/job-28025.json, its reading folder
#      moved here) under wait_both (E022 env.sh:51's list) and under wait_queued, each a cold `fmi try`; gate: the
#      attempt's events and the kept entries the same both ways, and the reading's count of the waits that also passed
#      on a path holding a sentence >= 1. If the demo differs, wait_queued is wrong: stop.
# A measurement, no change to the system. Lines in OUT/summary.txt.
# usage: gate3.sh OUT [a|b]...
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
game_ok || exit 1
OUT=$1; shift
mkdir -p $OUT
disk_ok $OUT || exit 1
quiet_ok || exit 1
calc() { awk "BEGIN { printf \"%.1f\", $1 }"; }
JOB=$HOME/.cache/fast-mi-e022/r48-data/job-28025.json
BOTH="FMI_READ_SWITCHES=value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost,wait_both"
QUEUED="FMI_READ_SWITCHES=value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost,wait_queued"
for part in ${@:-a b}; do
  case $part in
    a)
      $here/probe.sh $OUT a-queued j00006 "$QUEUED"
      tail -1 $OUT/summary.txt | sed 's/^/a: /';;
    b)
      for name in both queued; do
        ovr=$QUEUED; [ $name = both ] && ovr=$BOTH
        P=$OUT/b-$name; rm -rf $P; mkdir -p $P/jobs/j09202 $P/tmp
        jq --arg d $P/readings/48-j09202 '.reading_dir = $d' $JOB > $P/jobs/j09202/job.json
        t0=$(date +%s.%N)
        env $ON $DEMO $ovr FMI_READING_CACHE=$P/rcache FMI_MEMO= FMI_PLAN_CACHE=$P/plans FMI_POOL_DIR=$P/pool TMPDIR=$P/tmp \
          E008_DISK_PATH=$OUT E008_FLOOR_GB=$FLOOR_GB timeout 1800 nice -n $NICE $FMI_BIN try $P/jobs/j09202 --search fd --tail > $P/try.log 2>&1
        rc=$?
        R=$P/jobs/j09202/result.json
        # what the gate compares: each attempt's inputs, stop and end (run/aNN/run.json, in order), and each kept
        # entry's frame, tick, how and events (new/N.json.gz)
        for a in $P/jobs/j09202/run/a*/run.json; do
          [ -e $a ] && jq -c '{a: input_filename | split("/")[-2], stop, frame_end, qf_end, inputs}' $a
        done | sed "s#$P#P#g" > $P/attempts.json
        jq -c '.new[]? | {file, frame, tick, how}' $R | while read -r n; do
          echo "$n $(zcat $P/jobs/j09202/$(jq -r .file <<< "$n") | jq -c .events)"
        done > $P/kept.json
        q=$(jq -c '.wait_queued // {}' $P/readings/48-j09202/48-j09202.json 2>/dev/null)
        echo "b $name: rc $rc wall $(calc "$(date +%s.%N) - $t0") s: $(jq -c '{why: .reading.why, read_wall_s: .reading.read_wall_s, records: .reading.records, bound: .reading.bound, stop: .attempt.stop, new: (.new | length), wall_s}' $R 2>/dev/null); wait_queued counts $q" | tee -a $OUT/summary.txt
        rm -rf $P/pool $P/tmp $P/rcache $P/jobs/j09202/run
      done
      a=$(cmp -s $OUT/b-both/attempts.json $OUT/b-queued/attempts.json && echo same || echo DIFFER)
      k=$(cmp -s $OUT/b-both/kept.json $OUT/b-queued/kept.json && echo same || echo DIFFER)
      echo "b: attempts $a ($(grep -c . $OUT/b-queued/attempts.json)), kept entries $k ($(grep -c . $OUT/b-queued/kept.json))" | tee -a $OUT/summary.txt;;
  esac
done
