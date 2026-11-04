#!/bin/bash
# E013 integration: every entry of an archive verified from boot, one at a time through `fmi explore verify` in a fresh
# lean engine (E013's), with the switches given (G0: the E012 kitchen archive with SPEEDRUN_INPUT off and on; K1: the
# rerun's archive with E014's mechanism switches). A read-only archive is verified in a copy of its files (verify
# writes its verify-N.json and folder beside the entries). Prints the count with differences or errors.
# usage: ENV=integrate/env.sh verify-all.sh ARCH OUT [copy] [SWITCH=VALUE ...]
set -u
source "${ENV:?an env file (integrate/env.sh)}"
A=$1; O=$2; shift 2
mkdir -p $O
if [ "${1:-}" = copy ]; then
  shift
  C=$O/archive; rm -rf $C; mkdir -p $C
  cp -r $A/{stage.json,entries.jsonl,entries,log.jsonl,cells.json,readings.jsonl} $C/
  A=$C
fi
n=$(wc -l < $A/entries.jsonl); bad=0
echo "[$(date +%T)] verify $A: $((n - 1)) entries; switches: $*"
for ((i = 1; i < n; i++)); do
  env FMI_ENGINE_BIN=$E13_ENGINE_BIN "$@" nice -n 10 $FMI_BIN explore verify $A --entry $i > $O/$i.out 2>&1 || { bad=$((bad + 1)); echo "entry $i: error $(tail -1 $O/$i.out)"; continue; }
  grep -q '^differences: 0;' $O/$i.out || { bad=$((bad + 1)); echo "entry $i: $(tail -1 $O/$i.out)"; }
done
echo "[$(date +%T)] verify: $((n - 1)) entries from boot (fmi explore verify), $bad with differences or errors"
