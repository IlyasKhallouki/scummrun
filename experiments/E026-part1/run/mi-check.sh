#!/bin/bash
# E026's Monkey Island checks (plan.md G8, G13; mi-checks-now's mi-check.sh), each with env.sh's ON and MI's files
# named, every verify from boot with the same env, the engine svm-e024, the build $E26T:
#   b3       b3.sh (heavy lock), then `fmi check tries` against E013's G-on (/var/tmp/fast-mi/e013-int/b3f/all-fast),
#            the bounded tries and those whose reading waited on the bound's thread, and verify-new of its new entries
#   kitchen  E012's goal chain (8 entries) verified from boot
#   smoke    smoke.sh with N = 4, b = 1 (heavy lock): its entries verified, stall-report's checks, readings at the bound
#   g3, g4   g3.sh (the demo) and g4.sh
# One line per check in OUT/summary.txt. The disk floor is 2 GB on OUT's disk.
# usage: mi-check.sh OUT b3|kitchen|smoke|g3|g4...
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
game_ok || exit 1
O=$1; shift
mkdir -p $O
disk_ok $O || exit 1
LOCK=/var/tmp/fast-mi/heavy.lock
log() { echo "[$(date +%T)] $*" | tee -a $O/summary.txt; }
log "build $E26T: fmi $(sha1sum $FMI_BIN | cut -c1-12), e016 $(sha1sum $CHK/e016 | cut -c1-12), stall-report $(sha1sum $CHK/stall-report | cut -c1-12); engine $FMI_ENGINE_BIN; E26: ${E26:-none}"
for st in "$@"; do
  case $st in
  b3)
    B=$O/b3f; mkdir -p $B
    t0=$(date +%s)
    $here/b3.sh $B > $B-all.out 2>&1
    log "b3 all: $(tail -3 $B-all.out | tr '\n' ' ') (script wall $(( $(date +%s) - t0 )) s)"
    find $B/all-fast/rcache \( -name 'reading.tc' -o -name 'reading.actions.jsonl' -o -name 'reading.lists.jsonl' -o -name 'reading.idx' -o -name 'reading.reached' \) -delete 2>/dev/null
    env $ALLOFF $FMI_BIN check tries /var/tmp/fast-mi/e013-int/b3f/all-fast $B/all-fast --json $B/vs-e013-all.json > $B/vs-e013-all.txt 2>&1
    log "b3 all vs E013's G-on all rc $?: $(tail -1 $B/vs-e013-all.txt)"
    log "b3 tries whose reading waited on the bound's thread: $(grep -l bound_wait $B/all-fast/jobs/*/result.json | wc -l); bounded: $(grep -l '"bound":' $B/all-fast/jobs/*/result.json | wc -l)"
    t0=$(date +%s)
    env $ON $MI nice -n $NICE $FMI_BIN check verify-new $B/all-fast --out $B/all-fast-verify > $B/all-verify.txt 2>&1
    log "b3 verify-new all rc $?: $(tail -1 $B/all-verify.txt) (wall $(( $(date +%s) - t0 )) s)";;
  kitchen)
    K=$O/kitchen; A=/var/tmp/fast-mi/e012-archives/kitchen; C=$K/archive
    rm -rf $C; mkdir -p $C
    cp -r $A/{stage.json,entries.jsonl,entries,log.jsonl,cells.json,readings.jsonl} $C/
    g=$(jq '.goal_entries[0]' $A/summary.json)
    chain=$(jq -rs --argjson g $g '. as $r | [$g | recurse(if ($r[.].parent // 0) != 0 then $r[.].parent else empty end)] | reverse | map(tostring) | join(" ")' $A/entries.jsonl)
    bad=0; n=0; t0=$(date +%s)
    for i in $chain; do
      n=$((n + 1))
      env $ON $MI nice -n $NICE $FMI_BIN explore verify $C --entry $i > $K/$i.out 2>&1 || { bad=$((bad + 1)); continue; }
      grep -q '^differences: 0;' $K/$i.out || bad=$((bad + 1))
    done
    log "kitchen: the goal entry $g's chain, $n entries ($chain) verified from boot, $bad with differences or errors; goal entry: $(tail -1 $K/$g.out) (wall $(( $(date +%s) - t0 )) s)";;
  smoke)
    S=$O/smoke; rm -rf $S
    t0=$(date +%s)
    flock $LOCK $here/smoke.sh $S 4 1 > $O/smoke.out 2>&1
    log "smoke: $(tail -5 $O/smoke.out | tr '\n' ' ') (script wall $(( $(date +%s) - t0 )) s)"
    jq -rc '"  smoke \(.job) scope \(.scope) grow \(.grow) new \(.new_entries | length) reading \(.reading.why)"' $S/kitchen/log.jsonl >> $O/summary.txt
    tail -5 $O/summary.txt;;
  g3)
    $here/g3.sh $O/g3 > $O/g3.out 2>&1
    log "g3 (demo): $(tail -2 $O/g3.out | tr '\n' ' ')";;
  g4)
    $here/g4.sh $O/g4 > $O/g4.out 2>&1
    log "g4: $(tail -2 $O/g4.out | tr '\n' ' ' | cut -c1-700)";;
  *) echo "$st: b3, kitchen, smoke, g3 or g4"; exit 2;;
  esac
done
