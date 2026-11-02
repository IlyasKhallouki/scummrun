#!/bin/bash
# E011 G7's tail part on the integrated planner (experiments/E011-speed/plan.md): fmi-plan's old-tail check
# (`seq --tail-only 1`) on the plan component's requests made from the recorded replans whose readings are on disk
# (tails_make.py's, /var/tmp/fast-mi/e011-plan-scratch/tails), reported against the recorded replans and every accepted
# tail validated by the old taskcore binary. Under the heavy lock (the k = 2 readings).
# usage: g7tail.sh OUT
set -u
source $HOME/.cache/fast-mi-e011/env.sh
C=$WT11/experiments/E011-speed/rs/fmi-plan/check
T=/var/tmp/fast-mi/e011-plan-scratch/tails
OUT=$1; rm -rf $OUT; mkdir -p $OUT
cp $T/manifest.jsonl $OUT/
for n in $(cut -f1 $T/readings.tsv); do
  R=$(awk -v n=$n '$1==n{print $2}' $T/readings.tsv)
  [ -f $R/reading.json ] || { echo "reading $n: $R gone"; continue; }
  FMI_PLAN_CACHE= flock /var/tmp/fast-mi/heavy.lock nice -n 10 $FMI_PLAN_TOOL seq --reading $R --reqs $T/reqs-$n.tsv \
    --tail-only 1 > $OUT/out-$n.jsonl 2> $OUT/err-$n.txt
  echo "reading $n rc $?"
done
python3 $C/tails_report.py $OUT | tail -3
python3 $C/tails_validate.py $OUT $TASKCORE_BIN | tail -1
