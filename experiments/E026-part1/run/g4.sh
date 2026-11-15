#!/usr/bin/env bash
# E026 G4 on Monkey Island (plan.md G6, G13; mi-checks-now's copy of E016-realize-run/results/g4/run.sh): A along E012's
# kitchen chain, per job and along the chain, with env.sh's ON and MI's files named (MI), the e016 tool of the build;
# at nice $NICE. Threshold (E016 plan.md:114): 14 of 14 along the chain, the goal owner:566 = 1 reached.
# usage: g4.sh [OUT]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
game_ok
A=/var/tmp/fast-mi/e012-archives/kitchen
OUT=${1:-$O26/g4}
C=$OUT/archive
rm -rf "$C"; mkdir -p "$C/jobs"
disk_ok $OUT
cp -r "$A"/{entries.jsonl,entries,summary.json} "$C"/
g=$(jq '.goal_entries[0]' "$A/summary.json")
chain=$(jq -rs --argjson g $g '. as $r | [$g | recurse(if ($r[.].parent // 0) != 0 then $r[.].parent else empty end)] | reverse | map(tostring) | join(" ")' "$A/entries.jsonl")
for i in $chain; do
  j=$(jq -r --argjson i $i 'select(.id == $i) | .job' "$A/entries.jsonl")
  for r in "$A/jobs/$j"/run/*/run.json; do
    d=$C/jobs/$j/run/$(basename "$(dirname "$r")"); mkdir -p "$d"; cp "$r" "$d/"
  done
done
echo "goal entry $g, chain $chain"
t0=$(date +%s.%N)
env $ON $MI nice -n $NICE "$CHK/e016" g4 "$OUT" "$C" "$g" --goal owner:566=1 --run-on 600 --mode jobs,chain
echo "wall $(awk "BEGIN{print $(date +%s.%N) - $t0}") s"
