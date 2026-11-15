#!/usr/bin/env bash
# E026 G3 on the demo (plan.md G6, G13; mi-checks-now's copy of E016-realize-run/results/g3/run.sh): A on an inventory
# object along E015's item chain, with env.sh's ON and the demo's files (DEMO), the e016 tool of the build; at nice
# $NICE. Threshold: at most 15 candidates.
# usage: g3.sh [OUT]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
game_ok
OUT=${1:-$O26/g3}
mkdir -p $OUT
disk_ok $OUT
MOVIE=$ROOT/experiments/E015-engine-gates/results/item/check/carry-4-use-946.json
t0=$(date +%s.%N)
env $ON $DEMO nice -n $NICE "$CHK/e016" g3 "$OUT" --movie "$MOVIE" --clicks 30 --frame 2980 --steps s3:490,s8:946:491 \
  --local 212 --class-obj 491 --class 12 --object 946 --run-on 600 > /dev/null
echo "wall $(awk "BEGIN{print $(date +%s.%N) - $t0}") s"
jq -c '{pass, candidates, all_realized, run_on, verify: .verify.differences, hook: {events_equal: .hook.events_equal, digest_equal: .hook.digest_equal, error: .hook.error}}' "$OUT/g3.json"
