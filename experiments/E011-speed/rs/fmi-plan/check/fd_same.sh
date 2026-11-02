#!/bin/bash
# G7: the Rust search (taskcore search) against Fast Downward's lazy_greedy([hff], preferred=[hff],
# reopen_closed=false) with plusone costs, the search binary called directly, on SAS files taskcore writes: the plan
# files byte for byte, and FD's expanded / evaluated / generated counts.
#
# usage: fd_same.sh TASKCORE TC START CAPS OUT LIMIT_S TARGET...    (one SAS file per target, K=V)
set -u
TCB=$1; TC=$2; START=$3; CAPS=$4; OUT=$5; LIM=$6; shift 6
DW=${DOWNWARD_DIR:-/home/ilyask/projects/fast-mi/third_party/downward}/builds/release/bin/downward
FF_LAZY='let(hff,eval_modify_costs(ff(),cost_type=plusone),lazy_greedy([hff],preferred=[hff],reopen_closed=false))'
mkdir -p "$OUT"
n=0; same=0; both_out=0; diff=0
for g in "$@"; do
  d=$OUT/$(echo "$g" | tr ':=|,' '____'); mkdir -p "$d"; rm -f "$d"/sas_plan "$d"/rust_plan
  "$TCB" sas --task "$TC" --start-file "$START" --cost-caps "$CAPS" --goal "$g" --start-check needed \
    --out "$d/task.sas" --stats "$d/stats.json" 2> "$d/sas.err"
  if [ $? != 0 ]; then echo "$g: taskcore sas: $(head -c 200 "$d/sas.err")"; continue; fi
  n=$((n+1))
  ( cd "$d" && timeout "$LIM" "$DW" --search "$FF_LAZY" --internal-plan-file sas_plan < task.sas > fd.log 2>&1 ); fdrc=$?
  timeout "$LIM" "$TCB" search --sas "$d/task.sas" --plan "$d/rust_plan" > "$d/rust.json" 2> "$d/rust.err"; rrc=$?
  fdc=$(grep -E "^\[t=.*\] (Expanded|Evaluated|Generated) [0-9]+ state" "$d/fd.log" | sed -E 's/.*\] (\w+) ([0-9]+) .*/\2/' | tr '\n' ' ')
  rc=$(python3 -c "import json;d=json.load(open('$d/rust.json'));print(d['expanded'],d['evaluated'],d['generated'],'')" 2>/dev/null)
  if [ -f "$d/sas_plan" ] && [ -f "$d/rust_plan" ] && cmp -s "$d/sas_plan" "$d/rust_plan" && [ "$fdc" = "$rc" ]; then
    same=$((same+1)); echo "same  $g: $(head -c -1 <<< "$(tail -1 "$d/sas_plan")") expanded/evaluated/generated $rc"
  elif [ ! -f "$d/sas_plan" ] && [ ! -f "$d/rust_plan" ]; then
    fdst=$(grep -E "Completely explored|Search stopped without|no solution|Time limit" "$d/fd.log" | head -1)
    rst=$(python3 -c "import json;print(json.load(open('$d/rust.json'))['status'])" 2>/dev/null)
    if [ "$fdrc" = 11 ] && [ "$rst" = unsolvable ] && [ "$fdc" = "$rc" ]; then
      same=$((same+1)); echo "same  $g: no plan (FD exit 11, Rust unsolvable), counts $rc"
    else
      both_out=$((both_out+1)); echo "nopl  $g: FD rc $fdrc ($fdst) counts '$fdc'; Rust rc $rrc status '$rst' counts '$rc'"
    fi
  else
    diff=$((diff+1)); echo "DIFF  $g: FD rc $fdrc '$fdc' $(tr '\n' ' ' < "$d/sas_plan" 2>/dev/null); Rust rc $rrc '$rc' $(tr '\n' ' ' < "$d/rust_plan" 2>/dev/null)"
  fi
done
echo "tasks $n: same $same, both without a plan in ${LIM}s $both_out, different $diff"
[ $diff = 0 ]
