#!/bin/bash
# E013 component free's gates as run on the branch (plan.md, free's amendment, "Gates as run", and amendment (2) (b)):
#   D2: E012's smoke run (4 jobs from the boot, b = 1) with FMI_FREE unset, on this build and on E012's merged build
#       (target-e012), one after the other: free_check same (job files, entries, cells, log; timing and paths aside)
#   D1: the smoke run with FMI_FREE=0.25 (b = 1, 4 jobs: j00003 free) and FMI_FREE=1 (b = 2, 4 jobs, every job free),
#       each with E012's switches (SET=e12) and with the one switch set of amendment (2) (SET=all, E014's engine);
#       every entry verified from boot (smoke.sh's verify), 0 differences
#   D3: free_check report of each D1 archive
# Each run takes the heavy lock for itself. usage: gates.sh OUT [d2|d1|all]
set -u
here=$(cd "$(dirname "$0")" && pwd)
OUT=${1:?an output folder}; what=${2:-all}
T=${E13T:-/var/tmp/fast-mi/target-e013-free}
mkdir -p $OUT
run() {  # name bin set N B rho
  local name=$1 bin=$2 set=$3 n=$4 b=$5 rho=${6:-}
  [ -e $OUT/$name/kitchen ] && { echo "[$(date +%T)] $name: there already"; return; }
  FMI_BIN=$bin SET=$set flock /var/tmp/fast-mi/heavy.lock $here/smoke.sh $OUT/$name $n $b $rho > $OUT/$name.out 2>&1
  cat $OUT/$name.out
  # the readings' bulk (about 350 MB each, the archive's and the attempts' kept ones) is not read after the run and its
  # verify: reading.json and the start rows stay
  find $OUT/$name \( -name 'reading.tc' -o -name 'reading.actions.jsonl' -o -name 'reading.lists.jsonl' \
    -o -name 'reading.idx' -o -name 'reading.reached' \) -delete
}
if [ $what = all ] || [ $what = d2 ]; then
  run d2-free $T/release/fmi e12 4 1
  run d2-e012 /var/tmp/fast-mi/target-e012/release/fmi e12 4 1
  echo "== D2: free_check same"
  $T/release/free_check same $OUT/d2-free/kitchen $OUT/d2-e012/kitchen | tee $OUT/d2-same.txt
fi
if [ $what = all ] || [ $what = d1 ]; then
  for set in e12 all; do
    run d1-$set-q $T/release/fmi $set 4 1 0.25
    run d1-$set-1 $T/release/fmi $set 4 2 1
  done
  for x in d1-e12-q d1-e12-1 d1-all-q d1-all-1; do
    echo "== D3: $x"
    $T/release/free_check report $OUT/$x/kitchen | tee $OUT/$x-report.txt
  done
fi
