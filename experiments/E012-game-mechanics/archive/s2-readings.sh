#!/bin/bash
# E012 archive, gate S2: T of the kitchen's readings. The 19 readings the E011 kitchen check's committed tries used
# are made again from their start files (round 1's reader-scope with the arguments the try and the harness gave it,
# as E012 targets' T2 made them: experiments/E012-game-mechanics/targets/t2-readings.sh on e012-targets), one at a
# time; each reading's T goes to T.jsonl (archive-s2 tested) and its files are deleted. The one reading still in the
# archive (28-33-41-j00065) is read through links. The archive itself is only read.
# usage: s2-readings.sh OUT_DIR   (OUT_DIR on /home: a reading is up to ~0.6 GB while it lives)
set -u
A=/var/tmp/fast-mi/e011-archives/kitchen
R=/var/tmp/fast-mi/target-e011-scumm/release/reader-scope
S2=${S2_BIN:-/var/tmp/fast-mi/target-e012-archive/debug/archive-s2}
O=$1
export FAST_MI_ROOT=${FAST_MI_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
mkdir -p $O/readings
T=$O/T.jsonl
read_one() { # id name rooms start_rows fact_vars reach_also...
  local id=$1 name=$2 rooms=$3 rows=$4 fv=$5; shift 5
  if grep -q "\"name\":\"$id\"" $T 2>/dev/null; then echo "$id: kept"; return; fi
  local d=$O/readings/$id
  rm -rf $d; mkdir -p $d
  local args=(--rooms $rooms --start-seed 1 --domain-goals none --goal owner:566=1 --name $name --threads 2 --out-dir $d
              --elsewhere --live-locals --split-limit --switches value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost --reach)
  for f in "$@"; do args+=(--reach-also $f); done
  args+=(--frame-costs --start-rows $rows)
  [ -n "$fv" ] && args+=(--fact-vars $fv)
  local s=$(date +%s)
  $R "${args[@]}" 2> $d/reader-scope.err || { echo "$id: reader-scope failed"; tail -3 $d/reader-scope.err; return 1; }
  echo "$id: read in $(( $(date +%s) - s )) s, $(jq -c '[.n_records, .n_actions]' $d/reading.json)"
  if [ "$(jq '.n_actions' $d/reading.json)" != "0" ]; then
    $S2 tested "$id=$d" >> $T || { echo "$id: tested failed"; return 1; }
  fi
  find $d -type f ! -name reading.json ! -name reader-scope.err -delete
}
read_one F1 28-33-35-38-j00000 28,33,35,38 $A/readings/28-33-35-38-j00000/start.jsonl ""
read_one F2 33-38-83-85-96-j00001 33,38,83,85,96 $A/readings/33-38-83-85-96-j00001/start.jsonl ""
read_one F3 33-38-83-85-96-j00002 33,38,83,85,96 $A/readings/33-38-83-85-96-j00002/start.jsonl ""
read_one F4 33-38-83-85-96-j00003 33,38,83,85,96 $A/readings/33-38-83-85-96-j00003/start.jsonl ""
read_one F5 29-33-34-35-83-j00017 29,33,34,35,83 $A/readings/29-33-34-35-83-j00017/start.jsonl ""
read_one F6 29-33-34-35-83-j00019 29,33,34,35,83 $A/readings/29-33-34-35-83-j00019/start.jsonl ""
read_one F7 29-35-j00020 29,35 $A/readings/29-35-j00020/start.jsonl ""
read_one F8 34-35-j00025 34,35 $A/readings/34-35-j00025/start.jsonl ""
read_one F9 28-33-35-38-j00057 28,33,35,38 $A/readings/28-33-35-38-j00057/start.jsonl ""
read_one F10 28-33-35-38-j00059 28,33,35,38 $A/readings/28-33-35-38-j00059/start.jsonl ""
read_one F11 29-33-34-35-83-j00060 29,33,34,35,83 $A/readings/29-33-34-35-83-j00060/start.jsonl ""
read_one F12 28-33-41-j00061 28,33,41 $A/readings/28-33-41-j00061/start.jsonl ""
read_one F13 28-33-41-j00062 28,33,41 $A/readings/28-33-41-j00062/start.jsonl ""
read_one F14 28-33-41-j00063 28,33,41 $A/readings/28-33-41-j00063/start.jsonl ""
read_one D1 refine-01 28,33,35,38 $A/readings/28-33-35-38-j00000/start.jsonl 103 $A/jobs/j00000/run/a00/refine-01/start.txt
read_one D2 refine-01 29,33,34,35,83 $A/readings/29-33-34-35-83-j00019/start.jsonl 103 $A/jobs/j00019/run/a00/refine-01/start.txt
read_one D3 reach-01 29,35 $A/readings/29-35-j00020/start.jsonl "" $A/jobs/j00022/run/a36/reach-01/start.txt
read_one D4 reach-02 29,35 $A/readings/29-35-j00020/start.jsonl "" $A/jobs/j00022/run/a36/reach-01/start.txt $A/jobs/j00023/run/a36/reach-02/start.txt
read_one D5 refine-01 29,35 $A/readings/29-35-j00020/start.jsonl 103 $A/jobs/j00022/run/a36/reach-01/start.txt $A/jobs/j00023/run/a36/reach-02/start.txt $A/jobs/j00028/run/a00/refine-01/start.txt
# the reading still in the archive, through links (its index is there and fits: nothing is written)
if ! grep -q '"name":"K65"' $T 2>/dev/null; then
  L=$O/readings/K65; mkdir -p $L
  for f in $A/readings/28-33-41-j00065/*; do ln -sf $f $L/; done
  $S2 tested "K65=$L" >> $T
fi
