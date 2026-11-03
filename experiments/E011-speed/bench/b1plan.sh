#!/bin/bash
# E011 round 2, B1's planner: the plan component's ring(33) request sequence (serve33, 7 requests on j00089's reading:
# cold, the same request, another start, other caps, another goal, ...) through fmi-plan seq with the plan store off,
# for round 1's fmi-plan (Gbfs; FdLamaFirstCosts by Fast Downward's binary) and round 2's (the Rust lama-first-costs
# with the fast forms, round 2's fast settings; Gbfs with the fast forms), interleaved round by round; each sequence
# under the heavy lock at nice 10. Each request's line (wall, CPU, the reductions, the build, the relaxed task, the
# search) goes to OUT/VARIANT-ROUND.jsonl.
# usage: E011_ENV=$HOME/.cache/fast-mi-e011/env-r2.sh b1plan.sh OUT [ROUNDS]
set -u
source $HOME/.cache/fast-mi-e011/env.sh
[ -n "${E011_ENV:-}" ] && source "$E011_ENV" # a second env file after env.sh (round 2: env-r2.sh, its binaries)
OUT=$1; N=${2:-1}; mkdir -p $OUT
R33=$HOME/.cache/fast-mi-e011/plan/r33
C=$WT11/experiments/E011-speed/rs/fmi-plan/check
R1_PLAN=$(dirname $R1_FMI_BIN)/fmi-plan
export TMPDIR=/var/tmp/fast-mi/e011-tmp
for k in $(seq 1 $N); do
  for v in r1-gbfs r1-fd r2-lama r2-gbfs; do
    case $v in
      r1-gbfs) e=""; b=$R1_PLAN; s=gbfs;;
      r1-fd) e=""; b=$R1_PLAN; s=fd;;
      r2-lama) e="FMI_PLAN_FAST=1"; b=$FMI_PLAN_TOOL; s=lama;;
      r2-gbfs) e="FMI_PLAN_FAST=1"; b=$FMI_PLAN_TOOL; s=gbfs;;
    esac
    flock /var/tmp/fast-mi/heavy.lock env $e FMI_PLAN_CACHE= nice -n 10 $b seq --reading $R33 --reqs $C/serve33.tsv \
      --search $s > $OUT/$v-$k.jsonl 2> $OUT/$v-$k.err
    echo "$v round $k (load $(cut -d' ' -f1 /proc/loadavg)): $(jq -r '"\(.name) \(.status) \(.total_s|tostring|.[0:6]) s"' $OUT/$v-$k.jsonl | tr '\n' ';')"
  done
done
