#!/bin/bash
# E012's kitchen rerun: every entry of the archive verified from boot (the exactness rule: every new entry verifies
# from boot with 0 differences), one at a time through `fmi explore verify` in a fresh lean engine (end's), as the
# integration's smoke run did. Prints the count with differences or errors.
# usage: ENV=kitchen/env.sh verify-all.sh
set -u
source "${ENV:?an env file (kitchen/env.sh)}"
O=$K_OUT/verify-all; mkdir -p $O
n=$(wc -l < $K_ARCH/entries.jsonl); bad=0
for ((i = 1; i < n; i++)); do
  env FMI_ENGINE_BIN=$E12_ENGINE_BIN nice -n 10 $FMI_BIN explore verify $K_ARCH --entry $i > $O/$i.out 2>&1 || { bad=$((bad + 1)); echo "entry $i: error"; continue; }
  grep -q '^differences: 0;' $O/$i.out || { bad=$((bad + 1)); echo "entry $i: $(tail -1 $O/$i.out)"; }
done
echo "[$(date +%T)] verify: $((n - 1)) entries from boot (fmi explore verify), $bad with differences or errors"
