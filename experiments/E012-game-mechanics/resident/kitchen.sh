#!/bin/bash
# E012 resident, gate RS2 and the measure (E012 plan.md, "Amendment 2026-11-03, component resident"): ring(33) from
# the kitchen check's entry 0 (j00000's start row) and then from a second entry, each phase one `fmi read` under the
# heavy lock and the guard, the big files summed and dropped after each phase.
#   keys        in fmi's process, the memo's key over the units' order (FMI_READER_UNITS_SET=0) with a disk folder
#               for the key texts: entry 0, 191 (j00057), 189 (j00059); prints which keys' units line differs
#   before N    round 2's way (B3's settings): reader-scope as a child, --prep, no memo: entry 0, then the second
#   disk        reader-scope as a child, --prep, round 1's disk memo (FMI_MEMO, a new folder): entry 0, the second
#   after N     the switch FMI_READER_LIB=1: entry 0, the second, the second again (every run taken: the floor)
#   cmp         the second entry's (and entry 0's) files: every phase's against `before 1`'s, by sha1
# usage: kitchen.sh OUT SECOND PHASE [N]    (SECOND: j00057 or j00059)
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
OUT=$1; SECOND=$2; PHASE=$3; N=${4:-1}
mkdir -p $OUT
row() { echo $KITCHEN_READINGS/28-33-35-38-$1/start.jsonl; }
k() { read_line k-$1 28,33,35,38 $(row $1) $2; }  # NAME-tag ENTRYJOB OUTDIR
run() { # D ENV...: `fmi read` of D/lines.txt under the heavy lock and the guard, with the environment given
  local D=$1; shift
  echo "[$(date +%T)] $PHASE $N: $*"
  env -u FMI_READER_LIB -u FMI_MEMO -u FMI_READER_PREP -u FMI_READER_UNITS_SET -u FMI_READER_MEMO_GB "$@" \
    $HEAVY $D/guard.log $FMI_BIN read $D/lines.txt
  grep '"name"' $D/guard.log > $D/read.jsonl; tail -1 $D/guard.log
  for d in $D/*/; do [ -f $d/reading.json ] && sums_drop $d; done
  $PY -c "
import json,sys
for l in open('$D/read.jsonl'):
    r=json.loads(l)
    if 'wall_s' not in r: continue
    m=r.get('memo') or {}
    print('  %-10s wall %6.2f s cpu %6.2f s  taken %s ran %s (key differs %s)  rss peak seen %s MB, process %s MB, child max %s MB  mem store %s MB  %s' % (
      r['name'], r['wall_s'], r['cpu_s'], m.get('taken'), (m.get('ran_no_stored_run') or 0) + (m.get('ran_key_differs') or 0) + (m.get('ran_list_differs') or 0) + (m.get('ran_reached_differs') or 0),
      m.get('ran_key_differs'), r.get('reading_rss_peak_seen_mb'), r.get('process_hwm_mb'), r.get('children_maxrss_mb'), r.get('memo_in_memory_mb'), r.get('error') or ''))
"
}
case $PHASE in
  keys)
    D=$OUT/keys; rm -rf $D; mkdir -p $D
    { k j00000 $D/k0; k j00057 $D/k57; k j00059 $D/k59; } > $D/lines.txt
    run $D FMI_READER_LIB=1 FMI_READER_UNITS_SET=0 FMI_MEMO=$D/memo READER_MEMO_FLOOR_GB=5
    $PY - $D <<'PYEOF'
import json, sys, os
D = sys.argv[1]
keys = {}
for l in open(f"{D}/read.jsonl"):
    r = json.loads(l)
    if "memo" in r and r["memo"]:
        keys[r["name"]] = r["memo"]["dir"]
lines = {n: dict(l.split(" ", 1) for l in open(os.path.join(d, "inputs.txt")).read().splitlines() if " " in l) for n, d in keys.items()}
base = lines.get("k-j00000")
for n, t in lines.items():
    diff = sorted(k for k in set(t) | set(base) if t.get(k) != base.get(k))
    print(f"  {n}: key {os.path.basename(keys[n])[:8]}; lines that differ from entry 0's: {diff}")
PYEOF
    ;;
  before)
    D=$OUT/before-$N; rm -rf $D; mkdir -p $D
    { k j00000 $D/k0; k $SECOND $D/k2; } > $D/lines.txt
    run $D FMI_READER_PREP=1 ;;
  disk)
    D=$OUT/disk-$N; rm -rf $D; mkdir -p $D
    { k j00000 $D/k0; k $SECOND $D/k2; } > $D/lines.txt
    run $D FMI_READER_PREP=1 FMI_MEMO=$D/memo READER_MEMO_FLOOR_GB=5
    rm -rf $D/memo ;;
  after)
    D=$OUT/after-$N; rm -rf $D; mkdir -p $D
    { k j00000 $D/k0; k $SECOND $D/k2; k $SECOND $D/k2-again; } > $D/lines.txt
    run $D FMI_READER_LIB=1 ;;
  cmp)
    bad=0
    ref=$OUT/before-1
    for D in $OUT/before-* $OUT/disk-* $OUT/after-*; do
      [ -d $D ] || continue
      for t in k0 k2 k2-again; do
        [ -f $D/$t/sums.txt ] || continue
        r=$ref/${t%-again}
        if cmp -s $r/sums.txt $D/$t/sums.txt; then s=pass; else s=FAIL; bad=1; fi
        $PY $here/../../E011-speed/rs/fmi-reading/ref/cmp_reading.py $r $D/$t > $D/$t/cmp.json; j=$([ $? = 0 ] && echo pass || { bad=1; echo FAIL; })
        echo "$(basename $D)/$t: five files vs before-1/${t%-again} $s, reading.json $j"
      done
    done
    echo "RS2 files $([ $bad = 0 ] && echo PASS || echo FAIL)" ;;
  *) echo "phase: keys | before N | disk | after N | cmp"; exit 2 ;;
esac
