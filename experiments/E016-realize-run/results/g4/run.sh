#!/usr/bin/env bash
# E016 G4 (plan.md, "G4"): A on Monkey Island. Along E012's kitchen goal chain (archive
# /var/tmp/fast-mi/e012-archives/kitchen, read only: the files the tool reads are copied first, as a4/mi-check.sh's
# kitchen check copies its archive), each sentence, walk and dialog step the chain took re-realized with FMI_REALIZE=1:
# each chain entry's steps from the entry its job started at, and the whole chain from the boot's entry, then on to
# owner:566 = 1 and verified from boot. Build first:
#   cd experiments/E011-speed/rs && CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e016-a cargo build --release -p fmi-checks --bin e016
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
source "$here/../probe/env-mi.sh"
export FMI_REALIZE=1
A=/var/tmp/fast-mi/e012-archives/kitchen
OUT=${OUT:-/var/tmp/fast-mi/e016-a-out/g4}
C=$OUT/archive
rm -rf "$C"; mkdir -p "$C/jobs"
cp -r "$A"/{entries.jsonl,entries,summary.json} "$C"/
g=$(jq '.goal_entries[0]' "$A/summary.json")
# the goal entry and its ancestors (a4/mi-check.sh's rule), and their jobs' window records
chain=$(jq -rs --argjson g $g '. as $r | [$g | recurse(if ($r[.].parent // 0) != 0 then $r[.].parent else empty end)] | reverse | map(tostring) | join(" ")' "$A/entries.jsonl")
for i in $chain; do
  j=$(jq -r --argjson i $i 'select(.id == $i) | .job' "$A/entries.jsonl")
  for r in "$A/jobs/$j"/run/*/run.json; do
    d=$C/jobs/$j/run/$(basename "$(dirname "$r")"); mkdir -p "$d"; cp "$r" "$d/"
  done
done
echo "goal entry $g, chain $chain"
nice -n 10 "$E016_BIN" g4 "$OUT" "$C" "$g" --goal owner:566=1 --run-on 600 --mode jobs,chain
cp "$OUT/g4.json" "$here/g4.json"
cp "$OUT/chain-events.jsonl" "$here/chain-events.jsonl"
