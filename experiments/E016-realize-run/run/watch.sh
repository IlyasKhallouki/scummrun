#!/bin/bash
# E016's ladder watcher (plan.md "The outside ladder" and "Stop rule"). During a run it runs the ladder (fmi-checks
# ladder: entries.jsonl and the entries' checkpoints only) every INTERVAL s (300), appends one line to OUT/ladder.log
# and keeps the latest JSON in OUT/ladder.json. After WAIT s (2400: 40 minutes) of wall with no new rung (no rung
# reached for the first time since the last one, or since the watcher started), it asks the run to stop cleanly: it
# writes the run's stop file (the driver's FMI_STOP_FILE: no try starts, the running tries finish and are committed,
# the stop is "stop file"). It ends when the run's summary.json appears, or once it has asked.
# usage: [INTERVAL=s] [WAIT=s] [LADDER=bin] watch.sh ARCHIVE OUT STOP_FILE   (LADDER: env.sh's)
set -u
A=$1; O=$2; F=$3
INTERVAL=${INTERVAL:-300}; WAIT=${WAIT:-2400}
L=${LADDER:?the ladder binary (env.sh LADDER)}
mkdir -p $O
log=$O/ladder.log
t_new=$(date +%s); seen=""
echo "[$(date +%T)] watching $A every $INTERVAL s; a stop is asked after $WAIT s with no new rung ($F)" >> $log
while :; do
  if [ -e $A/summary.json ]; then
    echo "[$(date +%T)] the run stopped: $(jq -c '{stop, detail}' $A/summary.json)" >> $log
    break
  fi
  now=$(date +%s)
  if [ -e $A/entries.jsonl ]; then
    if line=$($L $A --json $O/ladder.json --cache $O/ladder.cache.json 2>> $O/ladder.err); then
      reached=$(jq -r '[.rungs[] | select(.reached) | .rung] | join(",")' $O/ladder.json)
      [ "$reached" != "$seen" ] && { t_new=$now; seen=$reached; }
    else
      line="ladder failed: $(tail -1 $O/ladder.err)"
    fi
    echo "[$(date +%T)] $line; $(( (now - t_new) / 60 )) min since a new rung" >> $log
  fi
  if [ $((now - t_new)) -ge $WAIT ]; then
    echo "no new rung for $((WAIT / 60)) min of wall (reached: ${seen:-none}); asked by watch.sh at $(date +%T)" > $F
    echo "[$(date +%T)] stop asked: $(cat $F)" >> $log
    break
  fi
  sleep $INTERVAL
done
