#!/bin/bash
# E026 copy of mi-checks-now's guard.sh (~/.cache/fast-mi-e007/guard.sh at nice ${NICE:-5}): run a command at nice NICE
# in its own process group; kill the group if MemAvailable falls under 3 GB. The last line of LOG: the command's rc and
# wall, whether it was killed (low 1), and the least MemAvailable seen (sampled every second).
# usage: guard.sh LOG cmd...
log=$1; shift
s=$(date +%s.%N)
setsid nice -n ${NICE:-5} "$@" > "$log" 2>&1 &
pid=$!
low=0; minavail=99999999
while kill -0 $pid 2>/dev/null; do
  a=$(awk '/MemAvailable/{print $2}' /proc/meminfo)
  [ "$a" -lt "$minavail" ] && minavail=$a
  if [ "$a" -lt 3145728 ]; then echo "GUARD: MemAvailable ${a} kB, killing group $pid" >> "$log"; kill -- -$pid; low=1; break; fi
  sleep 1
done
wait $pid; rc=$?
e=$(date +%s.%N)
echo "GUARD: rc $rc wall $(awk "BEGIN { printf \"%.1f\", $e - $s }") s low $low min MemAvailable ${minavail} kB" >> "$log"
