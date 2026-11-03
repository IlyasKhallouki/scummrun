#!/bin/bash
# E011 B0 (experiments/E011-speed/plan.md, "Benchmark"): E009's 15 kitchen tries again with the Python reference, in
# log order, each under the heavy lock and the guard at nice 10. The sequence goes to SEQ (default
# ~/.cache/fast-mi-e011/bench/b0); each try's output is SEQ/jobs/JOB, its log SEQ/logs/JOB.log.
# usage: b0.sh [SEQ] [FIRST_JOB]
set -u
source $HOME/.cache/fast-mi-e009/env.sh
here=$(cd "$(dirname "$0")" && pwd)
py=$here/../../../.venv/bin/python
seq=${1:-$HOME/.cache/fast-mi-e011/bench/b0}
first=${2:-j00000}
mkdir -p $seq/jobs $seq/logs
for n in $(seq 0 14); do
  job=$(printf "j%05d" $n)
  [[ $job < $first ]] && continue
  if [ -f $seq/jobs/$job/result.json ]; then echo "$job: done already"; continue; fi
  free=$(df --output=avail -B1G $HOME | tail -1)
  if [ "$free" -lt 8 ]; then echo "STOP: /home has $free GB free"; exit 1; fi
  echo "[$(date +%T)] $job"
  flock /var/tmp/fast-mi/heavy.lock $HOME/.cache/fast-mi-e007/guard.sh $seq/logs/$job.log \
    $py -u $here/before.py $seq/jobs/$job
  grep -E "^(CHECK|NOTE|  )" $seq/logs/$job.log | grep -v "^  *\[" ; tail -1 $seq/logs/$job.log
done
