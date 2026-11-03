#!/bin/bash
# g12watch.sh FLAG LOG [ROOTS] (G12, experiments/E011-speed/plan.md): while FLAG exists, every second, the processes whose
# ancestors include an fmi process; any python among them is a violation (logged with its pid and command line).
# The last line of LOG: samples, samples with an fmi process, the largest fmi tree, python processes seen.
# ROOTS: a file of pids, one per line (read every second): only those processes are roots, not every process named
# fmi (round 2: the benchmark's resident processes, while another fmi run shares the laptop).
flag=$1; log=$2; roots=${3:-}
n=0; with=0; maxt=0; py=0
while [ -e "$flag" ]; do
  rl=""; [ -n "$roots" ] && rl=" $(tr '\n' ' ' < "$roots" 2>/dev/null) "
  r=$(ps -eo pid=,ppid=,comm= | awk -v rl="$rl" '
    { pp[$1] = $2; cm[$1] = $3; if ((rl == "" && $3 == "fmi") || (rl != "" && index(rl, " " $1 " ") > 0)) root[$1] = 1 }
    END {
      nr = 0; for (r in root) nr++
      t = 0; py = 0
      for (p in pp) { q = p; d = 0; while ((q in pp) && d < 200) { if (q in root) { t++; if (cm[p] ~ /python/) { py++; bad = bad " " p } break } q = pp[q]; d++ } }
      print nr, t, py, bad
    }')
  set -- $r
  n=$((n + 1)); [ "$1" -gt 0 ] && with=$((with + 1)); [ "$2" -gt $maxt ] && maxt=$2
  if [ "$3" -gt 0 ]; then
    py=$((py + $3)); shift 3
    for p in "$@"; do echo "$(date +%T) PYTHON in an fmi tree: $p $(tr '\0' ' ' < /proc/$p/cmdline 2>/dev/null)" >> "$log"; done
  fi
  sleep 1
done
echo "G12: $n samples, $with with an fmi process, largest fmi tree $maxt processes, python processes in an fmi tree $py" >> "$log"
