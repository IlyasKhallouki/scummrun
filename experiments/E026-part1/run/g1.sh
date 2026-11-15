#!/bin/bash
# E026 G1 (plan.md Gates): the instruments I2, I3 and I5 against their references, read only on the archives.
#   ladder   --rungs rungs-demo.json on play9: its JSON byte-identical to the built-in ladder's (BASE_LADDER: fmi-checks
#            ladder built at 530ca2c, before the rungs moved); rungs-part1.json --score on E012's kitchen: P1 at entry
#            150, frame 18,416
#   keyfacts on E012's kitchen: #1, #8, #15, #16, #19, #20, #21, #45, #51, as keyfacts.py's (mi-history/keyfacts.out)
#   project  stall-report --project on E012's kitchen (the line 14 x T_K1)
# usage: [BASE_LADDER=bin] g1.sh OUT
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
O=$1; mkdir -p $O
disk_ok $O || exit 1
BASE_LADDER=${BASE_LADDER:-$O26/stage1/base/ladder}
P9=/var/tmp/fast-mi/e016-archives/play9; K=/var/tmp/fast-mi/e012-archives/kitchen
log() { echo "[$(date +%T)] $*" | tee -a $O/g1.txt; }
ok=1
nice -n $NICE $BASE_LADDER $P9 --json $O/play9-builtin.json > $O/play9-builtin.txt
nice -n $NICE $CHK/ladder $P9 --rungs $RUNGS/rungs-demo.json --json $O/play9-rungs.json > $O/play9-rungs.txt
if cmp -s $O/play9-builtin.json $O/play9-rungs.json; then r=PASS; else r=FAIL; ok=0; fi
log "$r ladder --rungs rungs-demo.json on play9: JSON sha1 $(sha1sum < $O/play9-rungs.json | cut -c1-12), built-in $(sha1sum < $O/play9-builtin.json | cut -c1-12); $(cat $O/play9-rungs.txt)"
nice -n $NICE $CHK/ladder $K --rungs $RUNGS/rungs-part1.json --score --json $O/kitchen-part1.json > $O/kitchen-part1.txt
p1=$(jq -c '.rungs[] | select(.rung == "P1") | [.entry, .frame]' $O/kitchen-part1.json)
if [ "$p1" = "[150,18416]" ]; then r=PASS; else r=FAIL; ok=0; fi
log "$r rungs-part1.json on E012's kitchen: P1 at [entry, frame] $p1; $(cat $O/kitchen-part1.txt)"
nice -n $NICE $CHK/keyfacts $K --key $KEY --json $O/keyfacts.json > $O/keyfacts.txt
kf=$(jq -r '.archives[0].held | keys | map(tonumber) | sort | map(tostring) | join(",")' $O/keyfacts.json)
if [ "$kf" = "1,8,15,16,19,20,21,45,51" ]; then r=PASS; else r=FAIL; ok=0; fi
log "$r keyfacts on E012's kitchen: #$kf; $(sed -n 2,3p $O/keyfacts.txt | tr -s ' ' | tr '\n' ';')"
nice -n $NICE $CHK/stall-report $K --project --json $O/kitchen-stalls.json --md $O/kitchen-stalls.md > /dev/null
log "stall-report --project on E012's kitchen: $(jq -r .projection.line $O/kitchen-stalls.json)"
log "G1 $([ $ok = 1 ] && echo PASS || echo FAIL)"
