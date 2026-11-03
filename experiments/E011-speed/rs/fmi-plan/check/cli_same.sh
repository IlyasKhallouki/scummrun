#!/bin/bash
# G7, part 1: the new taskcore's command line writes the old binary's files, byte for byte (stats.json but for
# times, memory and CPU), on one fixed task: sas, decode, validate, and serve on a request sequence.
#
# usage: cli_same.sh OLD NEW TC GOAL START CAPS PLAN NAMES OUT [SERVE_REQS]
#   TC: reading.tc; GOAL: K=V; START, CAPS: plan-000's start.txt and caps.txt; PLAN: a sas_plan of that task;
#   NAMES: input action names (one per line) for validate; SERVE_REQS: a file of serve requests, one per line,
#   "name<TAB>start<TAB>caps<TAB>goal" (the profile's sequences)
set -u
OLD=$1; NEW=$2; TC=$3; GOAL=$4; START=$5; CAPS=$6; PLAN=$7; NAMES=$8; OUT=$9; REQS=${10:-}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"
fail=0
same() {  # same LABEL A B
  if cmp -s "$2" "$3"; then echo "same  $1"; else echo "DIFF  $1 ($2 $3)"; fail=1; fi
}
same_stats() {
  python3 "$HERE/stats_norm.py" "$2" > "$2.norm"; python3 "$HERE/stats_norm.py" "$3" > "$3.norm"
  same "$1 (normalized)" "$2.norm" "$3.norm"
}
for who in old new; do
  B=$OLD; [ $who = new ] && B=$NEW
  d=$OUT/$who; mkdir -p "$d"
  "$B" sas --task "$TC" --start-file "$START" --cost-caps "$CAPS" --goal "$GOAL" \
     --start-check needed --out "$d/task.sas" --ops "$d/ops.jsonl" --stats "$d/stats.json" --kept "$d/kept.json"
  echo "rc $?"
  "$B" decode --task "$TC" --start-file "$START" --cost-caps "$CAPS" --goal "$GOAL" --start-check needed \
     --plan "$PLAN" --out "$d/steps.json"; echo "decode rc $?"
  "$B" validate --task "$TC" --start-file "$START" --cost-caps "$CAPS" --goal "$GOAL" --start-check needed \
     --names "$NAMES" --out "$d/validate.json"; echo "validate rc $?"
done
same "task.sas" "$OUT/old/task.sas" "$OUT/new/task.sas"
same "ops.jsonl" "$OUT/old/ops.jsonl" "$OUT/new/ops.jsonl"
same "kept.json" "$OUT/old/kept.json" "$OUT/new/kept.json"
same_stats "stats.json" "$OUT/old/stats.json" "$OUT/new/stats.json"
same "decode steps.json" "$OUT/old/steps.json" "$OUT/new/steps.json"
same "validate.json" "$OUT/old/validate.json" "$OUT/new/validate.json"
if [ -n "$REQS" ]; then
  for who in old new; do
    B=$OLD; [ $who = new ] && B=$NEW
    d=$OUT/serve-$who; mkdir -p "$d"
    {
      while IFS=$'\t' read -r name st cp gl; do
        mkdir -p "$d/$name"
        printf -- '--start-file\t%s\t--cost-caps\t%s\t--goal\t%s\t--start-check\tneeded\t--out\t%s\t--ops\t%s\t--stats\t%s\n' \
          "$st" "$cp" "$gl" "$d/$name/task.sas" "$d/$name/ops.jsonl" "$d/$name/stats.json"
      done < "$REQS"
    } | "$B" serve --task "$TC" > "$d/replies.txt"
    echo "serve $who rc $?: $(cut -f1 "$d/replies.txt" | tr '\n' ' ')"
  done
  while IFS=$'\t' read -r name st cp gl; do
    same "serve $name task.sas" "$OUT/serve-old/$name/task.sas" "$OUT/serve-new/$name/task.sas"
    same "serve $name ops.jsonl" "$OUT/serve-old/$name/ops.jsonl" "$OUT/serve-new/$name/ops.jsonl"
    same_stats "serve $name stats.json" "$OUT/serve-old/$name/stats.json" "$OUT/serve-new/$name/stats.json"
  done < "$REQS"
  cut -f1 "$OUT/serve-old/replies.txt" > "$OUT/serve-old/kinds"; cut -f1 "$OUT/serve-new/replies.txt" > "$OUT/serve-new/kinds"
  same "serve reply kinds" "$OUT/serve-old/kinds" "$OUT/serve-new/kinds"
fi
[ $fail = 0 ] && echo "ALL SAME" || echo "SOME DIFFER"
exit $fail
