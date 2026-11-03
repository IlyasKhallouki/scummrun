#!/bin/bash
# E012 component try, gates G0 and G-on (plan.md, amendment "component try"): B3's 15 tries from round 2's job folders
# (E011 round 2's B3: the fast settings' r2-fast, the reference settings' r2-ref), one resident fmi per sequence, the
# job memo off and the reading cache's floor at 5 GB as round 2's B3 ran them, under heavy.lock and the guard.
#   off-fast  every E012 switch unset, round 2's switches (ON2), --search fd --tail     (G0, against r2-fast)
#   off-ref   every E012 switch unset, round 2's switches (ON2), --search fd            (G0, against r2-ref)
#   on-fast   every E012 switch on, ON3, --search fd --tail, end's engine (patches 0001-0005)   (G-on: verify-new)
#   g10-off   E011 G10 (test (a), E009's job ta from the root), the fast settings, every E012 switch unset (its CHECK line)
#   g10-on    the same with every E012 switch on, ON3, end's engine (verify-new)
# usage: gates.sh off-fast|off-ref|on-fast|g10-off|g10-on OUT      (TRY_FMI_BIN: the fmi binary, default this component's build); TRY_NO_CACHE=1: no reading cache; TRY_SUFFIX: a suffix of the output folder
set -u
source $HOME/.cache/fast-mi-e011/env.sh
source $HOME/.cache/fast-mi-e011/env-r2d.sh
FMI=${TRY_FMI_BIN:-/var/tmp/fast-mi/target-e012-try/release/fmi}
END_ENGINE=/var/tmp/fast-mi/svm-e012-end/build/scummvm/scummvm
E12ON="FMI_WAIT=1 FMI_TARGET_PER_KEY=1 FMI_DELAY_FORK=1 FMI_AFFORD=1 FMI_SELECT=progress SPEEDRUN_REPORT_RESTART=1 FMI_BOXES=1"
E12OFF="FMI_WAIT= FMI_TARGET_PER_KEY= FMI_DELAY_FORK= FMI_AFFORD= FMI_SELECT= SPEEDRUN_REPORT_RESTART= FMI_BOXES="
case $1 in
  off-fast) SRC=/var/tmp/fast-mi/e011-r2int/b3f/r2-fast; ARGS="--search fd --tail"; SW="$ON2 $E12OFF";;
  off-ref) SRC=/var/tmp/fast-mi/e011-r2int/b3/r2-ref; ARGS="--search fd"; SW="$ON2 $E12OFF";;
  on-fast) SRC=/var/tmp/fast-mi/e011-r2int/b3f/r2-fast; ARGS="--search fd --tail"; SW="$ON3 $E12ON FMI_ENGINE_BIN=$END_ENGINE";;
  g10-off) SRC=g10; ARGS="--search fd --tail"; SW="$ON2 $E12OFF";;
  g10-on) SRC=g10; ARGS="--search fd --tail"; SW="$ON3 $E12ON FMI_ENGINE_BIN=$END_ENGINE";;
  *) echo "usage: gates.sh off-fast|off-ref|on-fast|g10-off|g10-on OUT"; exit 2;;
esac
OUT=$2; R=$OUT/$1${TRY_SUFFIX:-}
# TRY_NO_CACHE=1: no reading cache (a run's refinements are made again whatever the disk's level, so that two runs
# compare by their code, not by the cache's floor)
RCACHE=$R/rcache; [ "${TRY_NO_CACHE:-0}" = 1 ] && RCACHE=
rm -rf $R; mkdir -p $R/jobs $R/tmp
jobs=()
if [ $SRC = g10 ]; then
  # E011 G10, test (a): E009's job "ta" from the root, in the fast settings (ready, the old tail first)
  ln -s $HOME/.cache/fast-mi-e011/bench/b0/entries $R/entries
  mkdir -p $R/jobs/ta
  jq -c --arg rd $R/readings/28-33-41-ta '.reading_dir = $rd | .ready = true' $HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/job.json > $R/jobs/ta/job.json
  jobs+=($R/jobs/ta)
else
  ln -s $(readlink -f $SRC/entries) $R/entries
  ln -s $(readlink -f $SRC/entries.jsonl) $R/entries.jsonl
  for d in $SRC/jobs/j000*; do
    j=$(basename $d); mkdir -p $R/jobs/$j
    sed "s#$SRC#$R#g" $d/job.json > $R/jobs/$j/job.json
    jobs+=($R/jobs/$j)
  done
fi
echo "{\"seq\": \"$1\", \"src\": \"$SRC\", \"bin\": \"$FMI\", \"bin_sha1\": \"$(sha1sum $FMI | cut -c1-12)\", \"cache\": \"$RCACHE\", \"args\": \"$ARGS\", \"env\": \"$SW\", \"start\": \"$(date -Is)\", \"load\": \"$(cut -d' ' -f1-3 /proc/loadavg)\"}" > $R/run.json
exec 9>/var/tmp/fast-mi/heavy.lock; flock 9
echo "{\"locked\": \"$(date -Is)\", \"bin_sha1\": \"$(sha1sum $FMI | cut -c1-12)\"}" >> $R/run.json
t0=$(date +%s.%N)
env $SW FMI_READING_CACHE=$RCACHE FMI_MEMO= FMI_PLAN_CACHE=$R/plans FMI_POOL_DIR=$R/pool TMPDIR=$R/tmp \
  E008_DISK_PATH=$HOME E008_FLOOR_GB=5 \
  $HOME/.cache/fast-mi-e007/guard.sh $R/fmi.log $FMI try "${jobs[@]}" $ARGS
rc=$?
t1=$(date +%s.%N)
flock -u 9; exec 9>&-
echo "{\"rc\": $rc, \"wall_s\": $(awk "BEGIN{print $t1 - $t0}"), \"end\": \"$(date -Is)\", \"load\": \"$(cut -d' ' -f1-3 /proc/loadavg)\"}" >> $R/run.json
tail -2 $R/fmi.log
rm -rf $R/rcache $R/tmp $R/pool
case $1 in
  g10-*) jq -c '.attempt | {stop, goal_from_the_plan, frame0, frame_end, frames, qf, replans, inputs}' $R/jobs/ta/result.json | tee $R/g10.txt
    echo "new $(jq '.new | length' $R/jobs/ta/result.json); round 2: goal True 2728 3216 488 11712 2 36, 8 new" | tee -a $R/g10.txt
    env $SW $FMI check verify-new $R --out $R-verify --threads 2 | tee $R/verify.txt;;
  off-*) $FMI check tries $SRC $R --json $R/g0.json | tee $R/g0.txt;;
  on-*) env $SW $FMI check verify-new $R --out $R-verify --threads 2 | tee $R/verify.txt;;
esac
