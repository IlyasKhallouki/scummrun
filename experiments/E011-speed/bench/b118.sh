#!/bin/bash
# E011, the no-plan proofs merged into round 2 (plan.md, amendment "after round 2: the no-plan proofs merged into round
# 2"): the kitchen check's two bit:118 = 1 requests end to end. `fmi try` on copies of the check's tries 15 (entry 36,
# frame 2771) and 16 (entry 26, frame 2770) with the target forced, cold (one process per try, no reading cache, memo
# or plan store), the check's limit FMI_PLAN_TIME=180, in three forms interleaved try by try, each try under the heavy
# lock and the guard at nice 10:
#   r1  round 1's fmi, reader-scope and engine (env-r2.sh's R1_*), --search fd (Fast Downward's binary): the check's
#       settings;
#   r2  round 2's own fmi (R2_FMI_BIN) with ON2, --search fd --tail;
#   d   the merged build (FMI_BIN) with everything on (ON3), --search fd --tail.
# The readings: the check's refinement j00000/refine-01 was evicted from its archive; B0's j00000 refine-01 has the
# same start file (sha1) and the same records and actions, so the job copies name it (big files hardlinked, small ones
# copied, so nothing is written into B0's folders).
# usage: E011_ENV=$HOME/.cache/fast-mi-e011/env-r2d.sh b118.sh JOBS_DIR OUT [SEQ...]
#   JOBS_DIR holds j00015/job.json and j00016/job.json (the check's job files)
set -u
source $HOME/.cache/fast-mi-e011/env.sh
[ -n "${E011_ENV:-}" ] && source "$E011_ENV"
here=$(cd "$(dirname "$0")" && pwd)
PY=$here/../../../.venv/bin/python
JOBS=$1; OUT=$2; shift 2
seqs=${*:-r1 r2 d}
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
B0=$HOME/.cache/fast-mi-e011/bench/b0
export TMPDIR=/var/tmp/fast-mi/e011-tmp E008_DISK_PATH=/var/tmp
mkdir -p $OUT/readings
link_reading() { # SRC DST: the big files hardlinked, the rest copied
  mkdir -p $2
  for f in $1/*; do
    case $(basename $f) in
      reading.tc|reading.actions.jsonl|reading.lists.jsonl|reading.reached|reading.idx) ln -f $f $2/;;
      *) cp -p $f $2/;;
    esac
  done
}
R1=$OUT/readings/refine-j00000; R0=$OUT/readings/28-33-35-38-j00000
[ -f $R1/reading.tc ] || link_reading $B0/jobs/j00000/run/a00/refine-01 $R1
[ -f $R0/reading.tc ] || link_reading $B0/readings/28-33-35-38-j00000 $R0
args_of() { case $1 in r1) echo "--search fd";; *) echo "--search fd --tail";; esac; }
env_of() {
  case $1 in
    r1) echo "FMI_READER_SCOPE=$R1_READER_SCOPE FMI_ENGINE_BIN=$R1_ENGINE_BIN $OFF2";;
    r2) echo "$ON2";;
    d) echo "$ON3";;
  esac
}
bin_of() { case $1 in r1) echo $R1_FMI_BIN;; r2) echo $R2_FMI_BIN;; *) echo $FMI_BIN;; esac; }
now() { date +%s.%N; }
for j in j00015 j00016; do
  for q in $seqs; do
    d=$OUT/$q/$j; rm -rf $d; mkdir -p $d
    $PY - $JOBS/$j/job.json $d/job.json $R1 $R0 $d/reading <<'PYEOF'
import json, sys
j = json.load(open(sys.argv[1]))
j["readings"] = [{"path": sys.argv[3] + "/reading.json", "start_entry": 0},
                 {"path": sys.argv[4] + "/reading.json", "start_entry": 0}]
j["reading_dir"] = sys.argv[5]
open(sys.argv[2], "w").write(json.dumps(j) + "\n")
PYEOF
    exec 9>$LOCK; flock 9
    t0=$(now)
    env $(env_of $q) FMI_PLAN_TIME=180 FMI_PLAN_CACHE= FMI_POOL_DIR=$d/pool FMI_READING_CACHE= FMI_MEMO= \
      $GUARD $d/fmi.log $(bin_of $q) try $d --targets bit:118=1 $(args_of $q)
    t1=$(now)
    flock -u 9; exec 9>&-
    rm -rf $d/pool
    $PY - $d $q $j $t0 $t1 >> $OUT/times.jsonl <<'PYEOF'
import json, sys
d, q, j, t0, t1 = sys.argv[1:6]
r = json.load(open(d + "/result.json"))
a = r.get("attempt") or {}
pr = (r.get("probes") or [{}])[0]
print(json.dumps({"seq": q, "job": j, "wall_s": round(float(t1) - float(t0), 2), "try_wall_s": r.get("wall_s"),
                  "reading": (r.get("reading") or {}).get("why"), "status": pr.get("status"), "stop": a.get("stop") or pr.get("stop"),
                  "attempt_s": a.get("seconds", pr.get("seconds")), "load": open("/proc/loadavg").read().split()[0]}))
PYEOF
    echo "[$(date +%T)] $(tail -1 $OUT/times.jsonl)"
  done
done
