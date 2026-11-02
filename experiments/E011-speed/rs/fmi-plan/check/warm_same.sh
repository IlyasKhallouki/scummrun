#!/bin/bash
# G7: the warm task (taskcore warm) writes, for each request of a sequence, the files the old binary's serve wrote
# (task.sas, ops.jsonl byte for byte; stats.json but for times and memory), and its in-memory search returns Fast
# Downward's lazy h_FF plusone plan on that SAS file, byte for byte.
#
# usage: warm_same.sh TASKCORE TC REQS OUT REF [LIMIT_S]
#   REQS: "name<TAB>start<TAB>caps<TAB>goal" per line; REF: the old serve's output dir (REF/NAME/{task.sas,...})
set -u
TCB=$1; TC=$2; REQS=$3; OUT=$4; REF=$5; LIM=${6:-120}
HERE=$(cd "$(dirname "$0")" && pwd)
DW=${DOWNWARD_DIR:-/home/ilyask/projects/fast-mi/third_party/downward}/builds/release/bin/downward
FF_LAZY='let(hff,eval_modify_costs(ff(),cost_type=plusone),lazy_greedy([hff],preferred=[hff],reopen_closed=false))'
mkdir -p "$OUT"
{
  while IFS=$'\t' read -r name st cp gl; do
    mkdir -p "$OUT/$name"
    printf -- '--start-file\t%s\t--cost-caps\t%s\t--goal\t%s\t--start-check\tneeded\t--out\t%s\t--ops\t%s\t--stats\t%s\t--plan\t%s\n' \
      "$st" "$cp" "$gl" "$OUT/$name/task.sas" "$OUT/$name/ops.jsonl" "$OUT/$name/stats.json" "$OUT/$name/rust_plan"
  done < "$REQS"
} | "$TCB" warm --task "$TC" > "$OUT/replies.txt"
echo "warm rc $?"
cat "$OUT/replies.txt"
fail=0
while IFS=$'\t' read -r name st cp gl; do
  for f in task.sas ops.jsonl; do
    if cmp -s "$OUT/$name/$f" "$REF/$name/$f"; then echo "same  $name $f"; else echo "DIFF  $name $f"; fail=1; fi
  done
  python3 "$HERE/stats_norm.py" "$OUT/$name/stats.json" > "$OUT/$name/stats.norm"
  python3 "$HERE/stats_norm.py" "$REF/$name/stats.json" > "$OUT/$name/stats.ref.norm"
  if cmp -s "$OUT/$name/stats.norm" "$OUT/$name/stats.ref.norm"; then echo "same  $name stats.json (normalized)"; else echo "DIFF  $name stats.json"; diff "$OUT/$name/stats.norm" "$OUT/$name/stats.ref.norm" | head -10; fail=1; fi
  ( cd "$OUT/$name" && rm -f sas_plan && timeout "$LIM" "$DW" --search "$FF_LAZY" --internal-plan-file sas_plan < task.sas > fd.log 2>&1 )
  if [ -f "$OUT/$name/sas_plan" ] && cmp -s "$OUT/$name/sas_plan" "$OUT/$name/rust_plan"; then
    echo "same  $name plan: $(tr '\n' ' ' < "$OUT/$name/rust_plan")"
  else
    echo "DIFF  $name plan: FD $(tr '\n' ' ' < "$OUT/$name/sas_plan" 2>/dev/null) Rust $(tr '\n' ' ' < "$OUT/$name/rust_plan" 2>/dev/null)"; fail=1
  fi
done < "$REQS"
[ $fail = 0 ] && echo "ALL SAME" || echo "SOME DIFFER"
exit $fail
