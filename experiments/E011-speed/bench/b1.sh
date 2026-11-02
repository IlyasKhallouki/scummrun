#!/bin/bash
# E011 B1 (experiments/E011-speed/plan.md, "Benchmark"): the micro-benchmarks, before and after back to back on the
# same inputs, laptop, seed 1, nice 10, the heavy ones under the heavy lock and the guard. Outputs under B/b1.
#   engine    the engine component's bench.sh (E010's frame costs a-d, start, return by fork, the door chain from boot;
#             before: frame_costs.py and chain.py on svm-w3b; after: fmi-engine bench, lean and channel alone), and the
#             kitchen chain (E009's off check, entry 7, frame 3216) the same way;
#   readings  ring(38) j00091, ring(33) j00089, the refinement j00089 [103,194]: before, E009's binary through
#             read_rust (readio's ref_read.py); after, reader-scope --out-dir and fmi-reading's open, then the reading
#             cache (a miss, then a hit: the key and the open), and the refinement with the memo from j00000's;
#   planner   the fixed tasks (serve38, serve33: cold, the same request, another start, other caps, another goal):
#             before, plan_fd.plan_taskcore (plan_ref.py); after, fmi-plan (Gbfs and FdLamaFirstCosts, no store),
#             then from the plan store (hits); the old tail's check on the recorded replans of two readings;
#   windows   test (a)'s 4 windows, strict and ready, through fmi-engine on the lean build (play-lean); before is
#             B2's reference run (its windows.jsonl).
# usage: b1.sh [B] [engine|readings|planner|windows|all]
set -u
source $HOME/.cache/fast-mi-e011/env.sh
here=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$here/../../.." && pwd)
PY=$ROOT/.venv/bin/python
B=${1:-$HOME/.cache/fast-mi-e011/bench}; what=${2:-all}
O=$B/b1; mkdir -p $O
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
IN=$HOME/.cache/fast-mi-e011/profile-reader/inputs
export TMPDIR=/var/tmp/fast-mi/e011-tmp
TIMEFORMAT="%R %U %S"
ld() { cut -d' ' -f1 /proc/loadavg; }
timed() { # LABEL CMD...: one line "LABEL wall user sys load" (the time keyword, under the lock and the guard)
  local label=$1; shift
  local t; t=$( { time flock $LOCK $GUARD $O/$label.log "$@" > /dev/null 2>&1 ; } 2>&1 | tail -1 )
  local w; w=$(grep -o "GUARD: rc [0-9]* wall [0-9.]*" $O/$label.log | tail -1 | awk '{print $3, $5}')
  echo "$label rc/wall $w | time wall/user/sys $t | load $(ld)" | tee -a $O/b1.txt
}

if [ $what = engine ] || [ $what = all ]; then
  echo "== engine $(date +%T)"
  E011_ENGINE_OUT=$O/engine flock $LOCK bash $WT11/experiments/E011-speed/engine/bench.sh 2 > $O/engine-bench.log 2>&1
  tail -60 $O/engine-bench.log | grep -vE "^\s*$" | cut -c1-200
  K=$O/kitchen-arch; mkdir -p $K; [ -e $K/entries ] || ln -s $HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/new $K/entries
  for r in 1 2; do
    echo "-- kitchen chain, round $r: before (chain.py speed, svm-w3b)"
    FAST_MI_SCUMMVM_BIN=$HOME/.cache/fast-mi/svm-w3b/build/scummvm/scummvm flock $LOCK nice -n 10 $PY \
      $WT11/experiments/E011-speed/engine/chain.py speed $O/kspeed$r $K 7 3 2>&1 | tail -3
    echo "-- kitchen chain, round $r: after (fmi-engine bench, lean)"
    flock $LOCK nice -n 10 $FMI_ENGINE_TOOL bench $O/kafter$r $K 7 300 --lean 2>/dev/null
  done 2>&1 | tee $O/kitchen-chain.log | cut -c1-200
fi

if [ $what = readings ] || [ $what = all ]; then
  echo "== readings $(date +%T)"
  REF89="--fact-vars 103,194 --reach-also $IN/ref-j00089/refine-01.start.txt --reach-also $IN/ref-j00089/refine-02.start.txt"
  REF0="--fact-vars 103,194 --reach-also $IN/ref-j00000/refine-01.start.txt --reach-also $IN/ref-j00000/refine-02.start.txt"
  declare -A ROOMS=([r38]=33,38,83,85,96 [r33]=28,33,35,38 [ref89]=28,33,35,38)
  declare -A ROWS=([r38]=$IN/ring38-j00091.jsonl [r33]=$IN/ring33-j00089.jsonl [ref89]=$IN/ring33-j00089.jsonl)
  declare -A EXTRA=([r38]="" [r33]="" [ref89]="$REF89")
  for n in r38 r33 ref89; do
    rm -rf $O/rd-before-$n $O/rd-after-$n $O/rcache-$n
    timed rd-before-$n env READER_SCOPE_BIN=$READER_SCOPE_BIN $PY $WT11/experiments/E011-speed/rs/fmi-reading/ref/ref_read.py \
      --rooms ${ROOMS[$n]} --start-rows ${ROWS[$n]} --out $O/rd-before-$n ${EXTRA[$n]}
    $PY -c "import json; d=json.load(open('$O/rd-before-$n/reading.json')); print('  before $n: read_s', d.get('read_s'), 'reader wall/cpu', d['reader_scope'].get('read_wall_s'), d['reader_scope'].get('read_cpu_s'), 'peak', d['reader_scope'].get('peak_rss_mb'))" | tee -a $O/b1.txt
    timed rd-after-$n bash $WT11/experiments/E011-speed/rs/fmi-reading/ref/new_read.sh $FMI_READER_SCOPE ${ROOMS[$n]} \
      ${ROWS[$n]} $O/rd-after-$n ${EXTRA[$n]}
    $PY -c "import json; d=json.load(open('$O/rd-after-$n/rd-after-$n.json')); print('  after $n: read wall/cpu', d.get('read_wall_s'), d.get('read_cpu_s'), 'peak', d.get('peak_rss_mb'), 'files_s', d.get('files_s'))" | tee -a $O/b1.txt
    echo "  sha1 before/after: $(sha1sum < $O/rd-before-$n/reading.tc | cut -c1-8) $(sha1sum < $O/rd-after-$n/reading.tc | cut -c1-8)" | tee -a $O/b1.txt
    flock $LOCK nice -n 10 $FMI_READING_TOOL bench $O/rd-after-$n --every 47 --reps 5 2>&1 | tail -1 | cut -c1-300 | sed "s/^/  open $n: /" | tee -a $O/b1.txt
    for k in miss hit; do
      flock $LOCK nice -n 10 $FMI_READING_TOOL cache ${ROWS[$n]} ${ROOMS[$n]} owner:566=1 $O/rcache-$n --reader $FMI_READER_SCOPE \
        --threads 3 ${EXTRA[$n]} 2>&1 | tail -1 | cut -c1-300 | sed "s/^/  cache $k $n: /" | tee -a $O/b1.txt
    done
    rm -f $O/rd-before-$n/reading.tc $O/rd-before-$n/*.jsonl $O/rd-after-$n/reading.tc $O/rd-after-$n/*.jsonl
    rm -rf $O/rcache-$n
  done
  M=$O/memo; rm -rf $M
  bash $WT11/experiments/E011-speed/rs/fmi-reading/ref/new_read.sh $FMI_READER_SCOPE 28,33,35,38 $IN/ring33-boot.jsonl \
    $O/rd-memo-ref0 $REF0 --memo $M > /dev/null 2>&1
  timed rd-memo-ref89 bash $WT11/experiments/E011-speed/rs/fmi-reading/ref/new_read.sh $FMI_READER_SCOPE 28,33,35,38 \
    $IN/ring33-j00089.jsonl $O/rd-memo-ref89 $REF89 --memo $M
  $PY -c "import json; d=json.load(open('$O/rd-memo-ref89/rd-memo-ref89.json')); m=d.get('memo') or {}; print('  memo ref89 from j00000s store: read wall/cpu', d.get('read_wall_s'), d.get('read_cpu_s'), 'job_s', (d.get('pool') or {}).get('job_s'), 'taken', m.get('taken'), 'ran', m.get('ran_key_differs'))" | tee -a $O/b1.txt
  echo "  sha1 memo ref89: $(sha1sum < $O/rd-memo-ref89/reading.tc | cut -c1-8)" | tee -a $O/b1.txt
  rm -rf $M $O/rd-memo-ref0 $O/rd-memo-ref89/reading.tc $O/rd-memo-ref89/*.jsonl
fi

if [ $what = planner ] || [ $what = all ]; then
  echo "== planner $(date +%T)"
  C=$WT11/experiments/E011-speed/rs/fmi-plan/check
  R38=$HOME/.cache/fast-mi-e011/profile-planner/readings/r38; R33=$HOME/.cache/fast-mi-e011/plan/r33
  for n in 38 33; do
    eval R=\$R$n
    rm -rf $O/plan-before-$n; echo "-- r$n before (plan_ref.py) $(date +%T) load $(ld)"
    flock $LOCK nice -n 10 $PY $C/plan_ref.py $R $C/serve$n.tsv $O/plan-before-$n $TASKCORE_BIN 2>&1 | grep '^{' | cut -c1-260
    for s in gbfs fd; do
      echo "-- r$n after ($s, no store) $(date +%T) load $(ld)"
      FMI_PLAN_CACHE= flock $LOCK nice -n 10 $FMI_PLAN_TOOL seq --reading $R --reqs $C/serve$n.tsv --search $s 2>/dev/null | cut -c1-330
    done
    rm -rf $O/plans-$n
    for p in 1 2; do
      echo "-- r$n after (gbfs, the plan store, pass $p) load $(ld)"
      FMI_PLAN_CACHE=$O/plans-$n flock $LOCK nice -n 10 $FMI_PLAN_TOOL seq --reading $R --reqs $C/serve$n.tsv --search gbfs 2>/dev/null | cut -c1-200
    done
  done 2>&1 | tee $O/planner.log
  T=/var/tmp/fast-mi/e011-plan-scratch/tails
  for n in 6 7; do
    R=$(awk -v n=$n '$1==n{print $2}' $T/readings.tsv)
    echo "-- the old tail's check, reading $n ($R), 2 replans"
    FMI_PLAN_CACHE= nice -n 10 $FMI_PLAN_TOOL seq --reading $R --reqs $T/reqs-$n.tsv --tail-only 1 2>/dev/null | cut -c1-260
    FMI_PLAN_CACHE= nice -n 10 $FMI_PLAN_TOOL seq --reading $R --reqs $T/reqs-$n.tsv --tail-only 1 2>/dev/null | cut -c1-260
  done 2>&1 | tee -a $O/planner.log
fi

if [ $what = windows ] || [ $what = all ]; then
  echo "== windows $(date +%T)"
  flock $LOCK bash $here/g10play.sh $O/windows > $O/windows.log 2>&1
  grep -E "^(===|TIME|READY|STRICT|CHILDREN)|^window [0-9]: [0-9]+ children" $O/windows.log | cut -c1-200
fi
echo "== b1 done $(date +%T)"
