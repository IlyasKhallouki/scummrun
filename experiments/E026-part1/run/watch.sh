#!/bin/bash
# E026's watcher (plan.md I4, "Stop rule"), outside the system: the system never reads any of this. During a run of
# ARCHIVE, every INTERVAL s (300): the ladder over RUNGS_FILE with its score (fmi-checks ladder --score) and E001's
# Part I key facts (fmi-checks keyfacts; KEYFACTS=0: not), one line to OUT/watch.log and the latest JSONs in OUT/.
# Every MEM_EVERY s (10): MemAvailable, and the driver's RSS (the `fmi explore run ARCHIVE` process, by its name and
# /proc cmdline: VmRSS, VmHWM) and its process tree's; each line gives the least MemAvailable and the largest RSS since
# the last one. The first differing return (a log line with return.same = false; FMI_DIFFERS_DEAD=1 only marks its
# entries dead) is reported once, and with DIFFER_STOP=1 (default) a stop is asked at once, until it is explained.
# After WAIT s (3600) with no new rung and no new key fact a stop is asked (WAIT=0: never). A stop is asked through
# STOP_FILE (the driver's FMI_STOP_FILE: no try starts, the running tries finish). It ends when ARCHIVE/summary.json
# appears, after a last line.
# usage: [INTERVAL=s] [WAIT=s] [MEM_EVERY=s] [RUNGS_FILE=...] [KEYFACTS=0|1] [DIFFER_STOP=0|1] watch.sh ARCHIVE OUT STOP_FILE
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
A=$1; O=$2; F=$3
INTERVAL=${INTERVAL:-300}; WAIT=${WAIT:-3600}; MEM_EVERY=${MEM_EVERY:-10}
RF=${RUNGS_FILE:-$RUNGS/rungs-part1.json}
mkdir -p $O
log=$O/watch.log
t_new=$(date +%s); seen=""; asked=0; differ=""
echo "[$(date +%T)] watching $A every $INTERVAL s (memory every $MEM_EVERY s), rungs $RF; a stop is asked$( [ $WAIT -gt 0 ] && echo " after $WAIT s with no new rung and no new key fact" || echo " never for want of progress")$( [ ${DIFFER_STOP:-1} = 1 ] && echo ", and at the first differing return") ($F)" >> $log
# the driver: an fmi process whose command line runs `explore run` on this archive
driver() {
  local p
  for p in /proc/[0-9]*; do
    [ "$(cat $p/comm 2>/dev/null)" = fmi ] || continue
    tr "\0" " " < $p/cmdline 2>/dev/null | grep -qF " explore run $A " && { echo ${p#/proc/}; return; }
  done
}
# kB of RSS of PID's process tree
tree_rss() {
  ps -eo pid=,ppid=,rss= | awk -v root=$1 '{ pp[$1] = $2; rss[$1] = $3 } END {
    s = 0; for (p in pp) { q = p; d = 0; while (q != root && (q in pp) && d < 200) { q = pp[q]; d++ } if (q == root) s += rss[p] }
    print s }'
}
ask() {
  [ $asked = 1 ] && return
  echo "$1; asked by watch.sh at $(date +%T)" > $F
  asked=1
  echo "[$(date +%T)] stop asked: $1" >> $log
}
minavail=99999999; maxrss=0; hwm=0; maxtree=0; next=0; lastp=""
while :; do
  done_=0; [ -e $A/summary.json ] && done_=1
  a=$(awk '/MemAvailable/{print $2}' /proc/meminfo); [ $a -lt $minavail ] && minavail=$a
  p=$(driver)
  if [ -n "$p" ]; then
    lastp=$p
    r=$(awk '/VmRSS/{print $2}' /proc/$p/status 2>/dev/null || echo 0); [ "${r:-0}" -gt $maxrss ] && maxrss=$r
    h=$(awk '/VmHWM/{print $2}' /proc/$p/status 2>/dev/null || echo 0); [ "${h:-0}" -gt $hwm ] && hwm=$h
    t=$(tree_rss $p); [ "${t:-0}" -gt $maxtree ] && maxtree=$t
  fi
  now=$(date +%s)
  if [ $now -ge $next ] || [ $done_ = 1 ]; then
    next=$((now + INTERVAL))
    line="no entries yet"
    if [ -e $A/entries.jsonl ]; then
      if l=$($CHK/ladder $A --rungs $RF --score --json $O/ladder.json --cache $O/ladder.cache.json 2>> $O/ladder.err); then
        line="${l#ladder $A: }"
        held=$(jq -r '[.rungs[] | select(.reached) | .rung] | join(",")' $O/ladder.json)
      else
        line="ladder failed: $(tail -1 $O/ladder.err)"; held=$seen
      fi
      if [ ${KEYFACTS:-1} = 1 ]; then
        if $CHK/keyfacts $A --key $KEY --json $O/keyfacts.json > $O/keyfacts.txt 2>> $O/keyfacts.err; then
          kf=$(jq -r '.archives[0].held | keys | map(tonumber) | sort | map(tostring) | join(",")' $O/keyfacts.json)
          line="$line; key facts $(jq '.archives[0].held | length' $O/keyfacts.json) held, the most on one chain $(jq -r '.archives[0].best_chain | "\(.facts) (entry \(.entry))"' $O/keyfacts.json)"
        else
          kf=""; line="$line; keyfacts failed: $(tail -1 $O/keyfacts.err)"
        fi
        held="$held|$kf"
      fi
      [ "$held" != "$seen" ] && { t_new=$now; seen=$held; }
      if [ -z "$differ" ] && [ -e $A/log.jsonl ]; then
        differ=$(jq -c 'select(.return.same == false) | {job, entry, return}' $A/log.jsonl 2>/dev/null | head -1 | cut -c1-600)
        if [ -n "$differ" ]; then
          echo "[$(date +%T)] FIRST DIFFERING RETURN: $differ" >> $log
          [ ${DIFFER_STOP:-1} = 1 ] && ask "the first differing return: $(echo $differ | cut -c1-200)"
        fi
      fi
    fi
    echo "[$(date +%T)] $line; MemAvailable min $((minavail / 1024)) MB; driver ${lastp:-none} RSS max $((maxrss / 1024)) MB (VmHWM $((hwm / 1024)) MB), its tree max $((maxtree / 1024)) MB; $(( (now - t_new) / 60 )) min since a new rung or key fact" >> $log
    minavail=99999999; maxrss=0; maxtree=0
    if [ $WAIT -gt 0 ] && [ $((now - t_new)) -ge $WAIT ]; then
      ask "no new rung and no new key fact for $((WAIT / 60)) min of wall (held: ${seen:-none})"
    fi
  fi
  if [ $done_ = 1 ]; then
    echo "[$(date +%T)] the run stopped: $(jq -c '{stop, detail}' $A/summary.json)" >> $log
    break
  fi
  sleep $MEM_EVERY
done
