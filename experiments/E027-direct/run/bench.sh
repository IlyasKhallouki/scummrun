#!/bin/bash
# E027: the search's cost and behaviour on fixed stops, for holding two builds against each other.
# Each reference is a stop an earlier run kept (results/). The search is run from it until a fixed number of stops
# is expanded (--stops), one stop at a time, so the trials, the kept stops and their order do not depend on the
# machine: the digest of the kept stops says whether a change altered what the search does, the seconds what it
# costs. Monkey Island only; the rungs file is read to report, as in every run.
#   bench.sh BIN OUT [WORKERS=4] [STOPS=7] [REFS="p7top cake crew meathook boot"]
set -e
BIN=$1; OUT=$2; W=${3:-4}; N=${4:-7}; REFS=${5:-"p7top cake crew meathook boot"}
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); ROOT=$(cd "$HERE/../../.." && pwd); RES=$ROOT/experiments/E027-direct/results
source "$ROOT/experiments/E026-part1/run/env.sh"
ulimit -n 65536 2>/dev/null || true
cd "$ROOT"
for ref in $REFS; do
  case $ref in
    p7top)    FROM="--from $RES/vm24/vm24-top.jsonl 10139 --edges $RES/vm24/edges-vm24.jsonl --news owner:640,owner:641,owner:642,owner:643";;
    cake)     FROM="--from $RES/vm26/stop-1184.jsonl 1184 --edges $RES/vm26/edges.jsonl --news class:420:3";;
    crew)     FROM="--from $RES/vm27/rung-stops.jsonl 1022 --edges $RES/vm28/edges.jsonl --news bit:304,owner:907";;
    meathook) FROM="--from $RES/vm28/stops.jsonl 21276 --edges $RES/vm28/edges.jsonl --news bit:88";;
    boot)     FROM="";;
    *) echo "unknown reference $ref"; exit 2;;
  esac
  O=$OUT/$ref; rm -rf "$O"; mkdir -p "$O"
  env $ON $MI "$BIN" "$O" --minutes 60 --stops $N --workers $W --slices $W --batch 1 --at-once 1 \
    --guide $HOME/.cache/fast-mi-e026/size-cost/guide-part1.json --rungs experiments/E011-speed/rs/fmi-checks/rungs/rungs-part1.json \
    --lines gains --flag-lines --go $FROM > "$O/out.txt" 2> "$O/err.txt" || { echo "$ref: the run failed"; tail -3 "$O/err.txt"; continue; }
  rm -rf "$O/eng"
  python3 - "$ref" "$O" <<'PY'
import json, sys, hashlib
ref, O = sys.argv[1], sys.argv[2]
s = json.load(open(f"{O}/summary.json"))
h = hashlib.sha1(); n = 0
for l in open(f"{O}/nodes.jsonl"):
    i = l.find(',"events"')
    d = json.loads(l[:i] + "}") if i > 0 else json.loads(l)
    h.update(repr((d["id"], d["parent"], d["how"], d["room"], d["level"], d["class"], d["frame"])).encode()); n += 1
t = s["trial_seconds"]; b = s.get("batch_seconds", {})
print(f'{ref:9s} stops {s["expanded"]:3d} trials {s["trials"]:7d} kept {n:5d} digest {h.hexdigest()[:10]} | wall {s["seconds"]:6.1f} s'
      f' explore {b.get("explore", 0):6.1f} commit {b.get("commit", 0):5.1f} engines {b.get("engines", 0):4.1f}'
      f' | trial s: fork {t["fork"]:6.1f} run {t["run"]:6.1f} read {t["read"]:5.1f} inputs {t["inputs"]:5.1f} quit {t["quit"]:4.1f}')
PY
done
