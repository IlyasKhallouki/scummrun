#!/bin/bash
# tree_cpu.sh PID: the CPU seconds of PID's live process tree: utime + stime + cutime + cstime of every live process
# whose ancestors include PID (PID too). A try's CPU in a resident process is this before and after it. The stat lines
# are read by cat, which goes on past a process that ends meanwhile (awk reading /proc itself can stop there).
cat /proc/[0-9]*/stat 2>/dev/null | awk -v root="$1" -v hz="$(getconf CLK_TCK)" '
{
  i = index($0, ") "); if (i == 0) next
  split(substr($0, i + 2), f, " "); pid = substr($0, 1, index($0, " ") - 1)
  pp[pid] = f[2]; cpu[pid] = f[12] + f[13] + f[14] + f[15]
}
END {
  s = 0
  for (p in pp) { q = p; d = 0; while (q != root && (q in pp) && d < 200) { q = pp[q]; d++ } if (q == root) s += cpu[p] }
  printf "%.2f\n", s / hz
}'
