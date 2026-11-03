#!/bin/bash
# E011 route, G10 with the Rust lama-first-costs (experiments/E011-speed/plan.md; bench/b2.sh's "after" part with the
# new search): test (a), one try from the root on rooms 28, 33, 41 to owner:566 = 1, with `fmi try --search fd` and
# FMI_PLAN_LAMA=rust (fmi-plan serves FdLamaFirstCosts by taskcore's lama.rs), cold (one process, no stores), each under
# the heavy lock and the guard at nice 10: lama-strict (strict windows), lama-ready (play when ready), lama-fast (ready
# and the old tail first, --tail). Each try's CHECK line (off_a.py's) and its new entries verified from boot in a fresh
# lean engine (bench/verify_new.sh).
# usage: g10.sh FMI_BIN OUT [VARIANT...]
set -u
source $HOME/.cache/fast-mi-e011/env.sh
here=$(cd "$(dirname "$0")" && pwd)
bench=$here/../bench
PY=$here/../../../.venv/bin/python
FMI=$1; O=$2; shift 2
vars=${*:-lama-strict lama-ready lama-fast}
B=$HOME/.cache/fast-mi-e011/bench
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
JOB=$HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/job.json
mkdir -p $O
check_line() { # JOB_DIR: off_a.py's CHECK (a) line from result.json
  $PY - "$1" <<'PYEOF'
import json, sys
r = json.load(open(sys.argv[1] + "/result.json")); a = r.get("attempt") or {}
ws = a.get("window_summary") or {}
rc = 0 if r.get("ok") else 1
print("CHECK (a):", rc, a.get("stop"), a.get("goal_from_the_plan"), a.get("frame0"), a.get("frame_end"), a.get("frames"),
      a.get("qf"), a.get("replans"), a.get("inputs"), ws.get("children"), len(r.get("new") or ()), "wall", r.get("wall_s"))
PYEOF
}
for v in $vars; do
  d=$O/$v/jobs/ta; rm -rf $O/$v; mkdir -p $d
  ln -s $B/b0/entries $O/$v/entries
  $PY - "$JOB" "$d/job.json" "$O/$v/readings/28-33-41-ta" "$v" <<'PYEOF'
import json, sys
j = json.load(open(sys.argv[1])); j["reading_dir"] = sys.argv[3]
if sys.argv[4].endswith("ready") or sys.argv[4].endswith("fast"):
    j["ready"] = True
open(sys.argv[2], "w").write(json.dumps(j) + "\n")
PYEOF
  case $v in *-fast) a="--search fd --tail";; *) a="--search fd";; esac
  echo "[$(date +%T)] $v ($a, FMI_PLAN_LAMA=rust)"
  TIMEFORMAT="%R %U %S"
  { time flock $LOCK env FMI_PLAN_LAMA=rust FMI_PLAN_CACHE= FMI_POOL_DIR=$O/$v/pool $GUARD $O/$v.log $FMI try $d $a ; } 2> $O/$v.time
  check_line $d; echo "  time wall/user/sys $(tail -1 $O/$v.time); $(grep GUARD $O/$v.log)"
  bash $bench/verify_new.sh $O/$v $O/$v/verify > /dev/null 2>&1
  echo "  verify from boot (lean): $(tail -1 $O/$v/verify/verify.txt | cut -c1-200)"
  rm -rf $O/$v/pool
done
