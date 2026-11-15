#!/bin/bash
# E014 amendment (4), rule (2) on Monkey Island with both switches on (ALLON with A4: the bound 300 s, no background,
# grown scopes), on this amendment's build and E013's engine:
#   b3       B3's 15 tries (E013's integrate/b3.sh, settings fast, sequence all) against E013's G-on run of the same
#            (/var/tmp/fast-mi/e013-int/b3f/all-fast): `fmi check tries`; every new entry verified from boot (`fmi
#            check verify-new`, E014's mechanism switches on)
#   kitchen  E012's kitchen archive (read only, a copy of its files): every entry on its goal entry's chain verified
#            from boot (`fmi explore verify`, E014's mechanism switches and A4 on)
#   smoke    E012's smoke run (E013's integrate/smoke.sh: 4 jobs from the boot, the kitchen's settings, b = 1) with SET=all
#            (ALLON with A4), every entry verified from boot, the stall report over it
# usage: [A4_ENV=ENV] mi-check.sh OUT b3|kitchen|smoke...   (A4_ENV: another env file, as a6/env.sh; default a4/env.sh)
set -u
here=$(cd "$(dirname "$0")" && pwd)
export ENV=${A4_ENV:-$here/env.sh}
source $ENV
I=/var/tmp/fast-mi/wt-e014/experiments/E013-robust/integrate
O=$1; shift
mkdir -p $O
LOCK=/var/tmp/fast-mi/heavy.lock
log() { echo "[$(date +%T)] $*" | tee -a $O/summary.txt; }
for st in "$@"; do
  case $st in
  b3)
    B=$O/b3f; mkdir -p $B
    $I/b3.sh fast $B all > $B-all.out 2>&1
    log "b3 all: $(tail -3 $B-all.out | tr '\n' ' ')"
    find $B/all-fast/rcache \( -name 'reading.tc' -o -name 'reading.actions.jsonl' -o -name 'reading.lists.jsonl' -o -name 'reading.idx' -o -name 'reading.reached' \) -delete 2>/dev/null
    env $ALLOFF $FMI_BIN check tries /var/tmp/fast-mi/e013-int/b3f/all-fast $B/all-fast --json $B/vs-e013-all.json > $B/vs-e013-all.txt 2>&1
    log "b3 all vs E013's G-on all rc $?: $(tail -1 $B/vs-e013-all.txt)"
    grep -h '"bound\|bound_wait' $B/all-fast/jobs/*/result.json | head -3 > $B/bound-records.txt
    log "b3 tries whose reading waited on the bound's thread: $(grep -l bound_wait $B/all-fast/jobs/*/result.json | wc -l); bounded: $(grep -l '"bound":' $B/all-fast/jobs/*/result.json | wc -l)"
    env $E14M FMI_ENGINE_BIN=$E13_ENGINE_BIN nice -n 10 $FMI_BIN check verify-new $B/all-fast --out $B/all-fast-verify > $B/all-verify.txt 2>&1
    log "b3 verify-new all (E014's mechanisms on) rc $?: $(tail -1 $B/all-verify.txt)";;
  kitchen)
    K=$O/kitchen; A=/var/tmp/fast-mi/e012-archives/kitchen; C=$K/archive
    rm -rf $C; mkdir -p $C
    cp -r $A/{stage.json,entries.jsonl,entries,log.jsonl,cells.json,readings.jsonl} $C/
    g=$(jq '.goal_entries[0]' $A/summary.json)
    # the goal entry and its ancestors up to the root's child, in chain order
    chain=$(jq -rs --argjson g $g '. as $r | [$g | recurse(if ($r[.].parent // 0) != 0 then $r[.].parent else empty end)] | reverse | map(tostring) | join(" ")' $A/entries.jsonl)
    bad=0; n=0
    for i in $chain; do
      n=$((n + 1))
      env $E14M $A4 FMI_ENGINE_BIN=$E13_ENGINE_BIN nice -n 10 $FMI_BIN explore verify $C --entry $i > $K/$i.out 2>&1 || { bad=$((bad + 1)); continue; }
      grep -q '^differences: 0;' $K/$i.out || bad=$((bad + 1))
    done
    log "kitchen: the goal entry $g's chain, $n entries ($chain) verified from boot, $bad with differences or errors; goal entry: $(tail -1 $K/$g.out)";;
  smoke)
    S=$O/smoke; rm -rf $S
    flock $LOCK $I/smoke.sh $S 4 1 all > $O/smoke.out 2>&1
    log "smoke (ALLON with A4): $(tail -4 $O/smoke.out | tr '\n' ' ')"
    jq -rc '"  smoke \(.job) scope \(.scope) grow \(.grow) new \(.new_entries | length) reading \(.reading.why)"' $S/kitchen/log.jsonl >> $O/summary.txt
    tail -5 $O/summary.txt;;
  esac
done
