#!/bin/bash
# E012 targets: the end-to-end try on the copy of j00030 with FMI_TARGET_PER_KEY off or on (round 2's settings with
# the no-plan proofs and deepening, ON3; the fast search), then every new entry verified from boot in a fresh lean
# engine (fmi-engine verify on a two-entry archive: the root and the entry with its whole movie, as E011's
# verify_new.sh makes them).
# usage: run.sh off|on
set -u
source $HOME/.cache/fast-mi-e011/env-r2d.sh
T=/var/tmp/fast-mi/target-e012-targets/release
export FMI_BIN=$T/fmi
export TMPDIR=/var/tmp/fast-mi/e012-targets-t2/tmp
mkdir -p $TMPDIR
v=$1
E=/var/tmp/fast-mi/e012-targets-t2/e2e
J=$E/arch/jobs/j00030-$v
rm -rf $J/run $J/new $J/result.json $J/log
s=$(date +%s.%N)
env $ON3 FMI_TARGET_PER_KEY=$([ $v = on ] && echo 1 || echo 0) $FMI_BIN try $J --search fd --tail
rc=$?; echo "try rc $rc wall $(awk -v a=$s -v b=$(date +%s.%N) "BEGIN{printf \"%.1f\", b - a}") s"
jq -c '{targets: .targets.by_tier, n: .targets.n, first: .targets.first[0:3], probes: [.probes[] | {target, status, stop, seconds}], attempt: (.attempt | {target, stop, reached, frames, frame0, frame_end}), new: [.new[] | .frame], error}' $J/result.json
# every new entry from boot
V=$E/verify-$v; rm -rf $V; mkdir -p $V
n=0; bad=0
for f in $J/new/*.json.gz; do
  k=$(basename $f .json.gz)
  a=$V/arch-$k/entries; mkdir -p $a
  cp $E/arch/entries/0.json.gz $a/0.json.gz
  zcat $f | jq -c --slurpfile job $J/job.json '{id: 1, parent: 0, frame: .frame, tick: .tick,
      events: ($job[0].entry.events + .events), checkpoints: ([$job[0].entry.checkpoint] + .checkpoints)}' | gzip > $a/1.json.gz
  r=$(nice -n 10 $FMI_ENGINE_TOOL verify $V/run-$k $V/arch-$k 1 --lean 2> $V/$k.err)
  rc=$?; n=$((n + 1)); rm -rf $V/run-$k
  d=$(echo "$r" | grep -o '"differ": [0-9]*' | awk '{print $2}')
  if [ $rc != 0 ] || [ "${d:-1}" != 0 ]; then bad=$((bad + 1)); echo "entry $k: DIFFERS rc $rc $r"; head -3 $V/$k.err; fi
  echo "$k $r" >> $V/verify.txt
done
echo "verify $v: $n new entries from boot, $bad with differences"
