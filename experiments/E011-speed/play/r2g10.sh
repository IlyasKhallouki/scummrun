#!/bin/bash
# E011 round 2, play: G10's FD clauses (plan.md G10; bench/b2.sh's fd-strict and fd-ready) with a given fmi, engine and
# fmi-engine tool: fmi try on test (a)'s job (E009's off check's job.json, its own reading folder), cold, under the
# heavy lock and the guard at nice 10, then the goal entry verified from boot in a fresh lean engine (verify_new.sh
# with the same engine and tool). The CHECK line is off_a.py's (bench/b2.sh's check_line).
# usage: r2g10.sh OUT FMI ENGINE FMI_ENGINE_TOOL [fd-strict|fd-ready ...]   (env: FMI_* switches pass through)
set -u
source $HOME/.cache/fast-mi-e011/env.sh
here=$(cd "$(dirname "$0")" && pwd)
bench=$here/../bench
PY=$here/../../../.venv/bin/python
O=$1; export FMI_BIN=$2 FMI_ENGINE_BIN=$3 FMI_ENGINE_TOOL=$4; shift 4
vs=${*:-fd-strict fd-ready}
mkdir -p $O
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
JOB=$HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/job.json
B=$HOME/.cache/fast-mi-e011/bench
export TMPDIR=/var/tmp/fast-mi/e011-tmp
for v in $vs; do
  d=$O/$v/jobs/ta; rm -rf $O/$v; mkdir -p $d
  ln -s $B/b0/entries $O/$v/entries
  $PY - "$JOB" "$d/job.json" "$O/$v/readings/28-33-41-ta" "$v" <<'EOF'
import json, sys
j = json.load(open(sys.argv[1])); j["reading_dir"] = sys.argv[3]
if sys.argv[4].endswith("ready"):
    j["ready"] = True
open(sys.argv[2], "w").write(json.dumps(j) + "\n")
EOF
  echo "[$(date +%T)] $v (load $(cut -d' ' -f1-3 /proc/loadavg))"
  TIMEFORMAT="%R %U %S"
  { time flock $LOCK env FMI_PLAN_CACHE= FMI_POOL_DIR=$O/$v/pool $GUARD $O/$v.log $FMI_BIN try $d --search fd ; } 2> $O/$v.time
  $PY - "$d" <<'EOF'
import json, sys
r = json.load(open(sys.argv[1] + "/result.json")); a = r.get("attempt") or {}
ws = a.get("window_summary") or {}
rc = 0 if r.get("ok") else 1
print("  CHECK (a):", rc, a.get("stop"), a.get("goal_from_the_plan"), a.get("frame0"), a.get("frame_end"), a.get("frames"),
      a.get("qf"), a.get("replans"), a.get("inputs"), ws.get("children"), len(r.get("new") or ()), "wall", r.get("wall_s"))
EOF
  echo "  time wall/user/sys (lock wait in wall) $(tail -1 $O/$v.time); $(grep GUARD $O/$v.log)"
  # every new entry from boot in a fresh lean engine with this run's engine and tool (verify_new.sh's loop)
  V=$O/$v/verify; mkdir -p $V
  $PY $bench/mkverify.py $O/$v $V/arch > /dev/null
  for a in $V/arch/*/; do
    nice -n 10 $FMI_ENGINE_TOOL verify $V/run-$(basename $a) $a 1 --lean 2> $V/$(basename $a).err | sed "s#^#$(basename $a) #" >> $V/verify.txt
    rm -rf $V/run-$(basename $a)
  done
  echo "  verify from boot (lean): $(wc -l < $V/verify.txt) entries, $(grep -c '"differ": 0' $V/verify.txt) with 0 differences; last: $(tail -1 $V/verify.txt | cut -c1-160)"
  rm -rf $O/$v/pool
done
