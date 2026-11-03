#!/bin/bash
# E012 targets: one end-to-end try with FMI_TARGET_PER_KEY=1 on a copy of the kitchen check's try j00030 (entry 93,
# room 29 scope (29, 35)), on its reading (j00028's refine-01, var 103 a fact) made again in full.
set -eu
A=/var/tmp/fast-mi/e011-archives/kitchen
E=/var/tmp/fast-mi/e012-targets-t2/e2e
R=/var/tmp/fast-mi/target-e011-scumm/release/reader-scope
export FAST_MI_ROOT=${FAST_MI_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
D=$E/reading/refine-01
if [ ! -f $D/reading.json ]; then
  mkdir -p $D
  $R --rooms 29,35 --start-seed 1 --domain-goals none --goal owner:566=1 --name refine-01 --threads 2 --out-dir $D \
     --elsewhere --live-locals --split-limit --switches value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost --reach \
     --reach-also $A/jobs/j00022/run/a36/reach-01/start.txt --reach-also $A/jobs/j00023/run/a36/reach-02/start.txt \
     --reach-also $A/jobs/j00028/run/a00/refine-01/start.txt --frame-costs --start-rows $A/readings/29-35-j00020/start.jsonl \
     --fact-vars 103 2> $D/reader-scope.err
fi
jq -c '[.n_records, .n_actions]' $D/reading.json
# the archive's root entry (root_facts_of reads entries.jsonl and the root's file next to the jobs)
mkdir -p $E/arch/entries $E/arch/jobs
head -1 $A/entries.jsonl > $E/arch/entries.jsonl
cp $A/entries/0.json.gz $E/arch/entries/0.json.gz
for v in off on; do
  J=$E/arch/jobs/j00030-$v
  rm -rf $J; mkdir -p $J
  jq --arg p $D/reading.json --arg rd $E/arch/readings/29-35-$v '.readings = [{"path": $p, "start_entry": 92}] | .reading_dir = $rd' \
     $A/jobs/j00030/job.json > $J/job.json
done
ls $E/arch/jobs
