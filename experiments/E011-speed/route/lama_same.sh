#!/bin/bash
# E011 route, G7's lama clause: the Rust lama-first-costs (taskcore lama.rs, `lama search`) against Fast Downward's
# lama-first-costs (plan_fd.LAMA_FIRST_COSTS, its search binary called directly) on SAS files taskcore writes: the
# landmark graph (graph_cmp.py), the plan file byte for byte, and FD's expanded / evaluated / generated counts.
#
# usage: lama_same.sh TASKCORE LAMA TC REQS OUT LIMIT_S
#   REQS: one request per line, name<TAB>start.txt<TAB>caps.txt<TAB>goal (fmi-plan check/serve*.tsv)
set -u
TCB=$1; LAMA=$2; TC=$3; REQS=$4; OUT=$5; LIM=$6
here=$(cd "$(dirname "$0")" && pwd)
DW=${DOWNWARD_DIR:-/home/ilyask/projects/fast-mi/third_party/downward}/builds/release/bin/downward
SEARCH='let(hlm,eval_modify_costs(landmark_sum(lm_factory=lm_reasonable_orders_hps(lm_rhw(),verbosity=debug),pref=false),cost_type=plusone),let(hff,eval_modify_costs(ff(),cost_type=plusone),lazy_greedy([hff,hlm],preferred=[hff,hlm],reopen_closed=false)))'
mkdir -p "$OUT"
n=0; same=0; diff=0
while IFS=$'\t' read -r name start caps goal; do
  [ -z "$name" ] && continue
  d=$OUT/$name; mkdir -p "$d"; rm -f "$d"/rust_plan
  "$TCB" sas --task "$TC" --start-file "$start" --cost-caps "$caps" --goal "$goal" --start-check needed \
    --out "$d/task.sas" --stats "$d/stats.json" 2> "$d/sas.err" || { echo "$name: taskcore sas: $(head -c 200 "$d/sas.err")"; continue; }
  n=$((n+1))
  if [ ! -f "$d/fd.log" ] || [ "${FRESH_FD:-0}" = 1 ]; then
    rm -f "$d/sas_plan"
    ( cd "$d" && TIMEFORMAT="%R %U %S"; { time timeout "$LIM" "$DW" --search "$SEARCH" --internal-plan-file sas_plan < task.sas > fd.log 2>&1; } 2> fd.time )
  fi
  "$LAMA" graph --sas "$d/task.sas" > "$d/rust.dump" 2> "$d/graph.err"
  g=$(python3 "$here/graph_cmp.py" "$d/fd.log" "$d/rust.dump")
  TIMEFORMAT="%R %U %S"; { time timeout "$LIM" "$LAMA" search --sas "$d/task.sas" --plan "$d/rust_plan" > "$d/rust.json" 2> "$d/rust.err"; } 2> "$d/rust.time"
  fdc=$(grep -E "^\[t=.*\] (Expanded|Evaluated|Generated) [0-9]+ state" "$d/fd.log" | sed -E 's/.*\] (\w+) ([0-9]+) .*/\2/' | tr '\n' ' ')
  rc=$(python3 -c "import json;d=json.load(open('$d/rust.json'));print(d['expanded'],d['evaluated'],d['generated'],'')" 2>/dev/null)
  if [ -f "$d/sas_plan" ] && [ -f "$d/rust_plan" ] && cmp -s "$d/sas_plan" "$d/rust_plan" && [ "$fdc" = "$rc" ] && [[ $g == same* ]]; then
    same=$((same+1)); echo "same  $name: $(tail -1 "$d/sas_plan") e/e/g $rc; graph $g; fd $(cut -d' ' -f1 "$d/fd.time") s, rust $(cut -d' ' -f1 "$d/rust.time") s"
  elif [ ! -f "$d/sas_plan" ] && [ ! -f "$d/rust_plan" ] && grep -q "Search stopped without finding a solution\|Completely explored" "$d/fd.log" && grep -q '"unsolvable"' "$d/rust.json" && [ "$fdc" = "$rc" ] && [[ $g == same* ]]; then
    same=$((same+1)); echo "same  $name: no plan (FD and Rust unsolvable), e/e/g $rc; graph $g"
  else
    diff=$((diff+1)); echo "DIFF  $name: graph $g; FD '$fdc' $(tr '\n' ' ' < "$d/sas_plan" 2>/dev/null | cut -c1-200); Rust '$rc' $(tr '\n' ' ' < "$d/rust_plan" 2>/dev/null | cut -c1-200)"
  fi
  rm -f "$d/task.sas"
done < "$REQS"
echo "requests $n: same $same, different $diff"
[ $diff = 0 ]
