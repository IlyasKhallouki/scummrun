#!/bin/bash
# E011 B2 and G10 (experiments/E011-speed/plan.md): test (a) end to end, one try from the root on rooms 28, 33, 41 to
# owner:566 = 1, each under the heavy lock and the guard at nice 10:
#   before  E009's step test (a) on svm-w3b (E009's off_a.py: the try, then its goal entry verified from boot);
#   after   fmi try on the same job (E009's off check's job.json, its own reading folder), cold (one process, no
#           stores): fd-strict (FdLamaFirstCosts, strict windows), fd-ready (play when ready), gbfs-strict,
#           gbfs-fast (Gbfs, --tail, ready); each try's goal entry verified from boot in a fresh lean engine.
# The CHECK line is off_a.py's: rc stop goal_from_the_plan frame0 frame_end frames qf replans inputs children new.
# usage: b2.sh [B] [before|after|all]
set -u
source $HOME/.cache/fast-mi-e011/env.sh
here=$(cd "$(dirname "$0")" && pwd)
PY=$here/../../../.venv/bin/python
B=${1:-$HOME/.cache/fast-mi-e011/bench}; what=${2:-all}
O=$B/b2; mkdir -p $O
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
JOB=$HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/job.json
export TMPDIR=/var/tmp/fast-mi/e011-tmp
check_line() { # JOB_DIR: off_a.py's CHECK (a) line from result.json
  $PY - "$1" <<'EOF'
import json, sys
r = json.load(open(sys.argv[1] + "/result.json")); a = r.get("attempt") or {}
ws = a.get("window_summary") or {}
rc = 0 if r.get("ok") else 1
print("CHECK (a):", rc, a.get("stop"), a.get("goal_from_the_plan"), a.get("frame0"), a.get("frame_end"), a.get("frames"),
      a.get("qf"), a.get("replans"), a.get("inputs"), ws.get("children"), len(r.get("new") or ()), "wall", r.get("wall_s"))
EOF
}
if [ $what = before ] || [ $what = all ]; then
  rm -rf $O/before; mkdir -p $O/before
  echo "[$(date +%T)] before: off_a.py on svm-w3b"
  TIMEFORMAT="%R %U %S"
  { time flock $LOCK env CHECK_OUT=$O/before $GUARD $O/before.log $PY -u $HOME/.cache/fast-mi-e009/scripts/off_a.py ; } 2> $O/before.time
  grep -E "^CHECK|^\[a\]|GUARD" $O/before.log; echo "time wall/user/sys (lock wait in wall) $(tail -1 $O/before.time)"
fi
if [ $what = after ] || [ $what = all ]; then
  for v in fd-strict fd-ready gbfs-strict gbfs-fast; do
    d=$O/$v/jobs/ta; rm -rf $O/$v; mkdir -p $d
    ln -s $B/b0/entries $O/$v/entries  # the root (E009's, the off check's root state) for the verify
    $PY - "$JOB" "$d/job.json" "$O/$v/readings/28-33-41-ta" "$v" <<'EOF'
import json, sys
j = json.load(open(sys.argv[1])); j["reading_dir"] = sys.argv[3]
if sys.argv[4].endswith("ready") or sys.argv[4].endswith("fast"):
    j["ready"] = True
open(sys.argv[2], "w").write(json.dumps(j) + "\n")
EOF
    case $v in fd-*) a="--search fd";; gbfs-strict) a="--search gbfs";; gbfs-fast) a="--search gbfs --tail";; esac
    echo "[$(date +%T)] after: $v ($a)"
    TIMEFORMAT="%R %U %S"
    { time flock $LOCK env FMI_PLAN_CACHE= FMI_POOL_DIR=$O/$v/pool $GUARD $O/$v.log $FMI_BIN try $d $a ; } 2> $O/$v.time
    check_line $d; echo "  time wall/user/sys $(tail -1 $O/$v.time); $(grep GUARD $O/$v.log)"
    bash $here/verify_new.sh $O/$v $O/$v/verify > /dev/null 2>&1
    echo "  verify from boot (lean): $(tail -1 $O/$v/verify/verify.txt | cut -c1-200)"
    rm -rf $O/$v/pool
  done
fi
