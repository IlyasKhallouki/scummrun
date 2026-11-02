#!/bin/bash
# E011 G9 on the integrated binary (experiments/E011-speed/plan.md, "Exactness gates"): the explore component's checks
# with fmi built in the rs/ workspace (every port linked in):
#   driver: 18 fake-step scenarios against explore.py with fake_step.py, in process and a process per job;
#   guide: guide.py's bytes for door, kitchen, part1 and the wide kitchen;
#   step: step.py's targets, reusable and idle_rule on the recorded cases; harness: the start rules;
#   replay: recorded tries through run_job and the harness, every port a double; reingest: recorded archives.
# usage: g9.sh OUT
set -u
source $HOME/.cache/fast-mi-e011/env.sh
here=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$here/../../.." && pwd)
PY=$ROOT/.venv/bin/python
X=$WT11/experiments/E011-speed/explore
F=$HOME/.cache/fast-mi-e011/explore/fixtures
OUT=$1; mkdir -p $OUT
bad=0
ok() { if [ $1 = 0 ]; then echo "$2: pass"; else echo "$2: FAIL (rc $1)"; bad=1; fi; }
cd $WT11
$PY $X/g9_driver.py --fmi $FMI_BIN --out $OUT/driver > $OUT/driver.log 2>&1; ok $? "driver"; tail -1 $OUT/driver.log
$PY $X/g9_guide.py --fmi $FMI_BIN --out $OUT/guide > $OUT/guide.log 2>&1; ok $? "guide"; tail -3 $OUT/guide.log
$FMI_BIN check step $F/step/step-check.jsonl > $OUT/step.log 2>&1; ok $? "step"; tail -1 $OUT/step.log
$FMI_BIN check harness $F/harness/harness-check.jsonl > $OUT/harness.log 2>&1; ok $? "harness"; tail -1 $OUT/harness.log
for j in $HOME/.cache/fast-mi-e009/off/step-a/jobs/ta $HOME/.cache/fast-mi-e008/step-tests/jobs/ta \
         $HOME/.cache/fast-mi-e008/step-tests/jobs/tb; do
  n=$(echo $j | awk -F/ '{print $(NF-3)"-"$NF}')
  rm -rf $OUT/replay-$n
  $FMI_BIN check replay $j --out $OUT/replay-$n > $OUT/replay-$n.log 2>&1; ok $? "replay $n"; tail -1 $OUT/replay-$n.log
done
for a in $HOME/.cache/fast-mi-e008/archives/door $HOME/.cache/fast-mi-e008/archives/smoke \
         $HOME/.cache/fast-mi-e008/archives/kitchen $HOME/.cache/fast-mi-e009/archives/door; do
  n=$(echo $a | awk -F/ '{print $(NF-2)"-"$NF}')
  rm -rf $OUT/reingest-$n
  $FMI_BIN check reingest $a --out $OUT/reingest-$n > $OUT/reingest-$n.log 2>&1; ok $? "reingest $n"
  tail -1 $OUT/reingest-$n.log; rm -rf $OUT/reingest-$n
done
echo "G9 $([ $bad = 0 ] && echo PASS || echo FAIL)"
exit $bad
