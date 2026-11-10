#!/bin/bash
# E026 probe (plan.md G2, G3, R3; mi-checks-now's probe.sh): one B3 try, cold and alone, with env.sh's ON and MI's files
# named, then OVERRIDES (VAR= assignments after ON: later ones win in env), to measure one reading or one switch. A
# measurement, no change to the system. One line per probe in OUT/summary.txt; the try's bulk is removed after.
# usage: probe.sh OUT NAME JOB ["OVERRIDES"]
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
game_ok || exit 1
OUT=$1; NAME=$2; J=$3; OVR=${4:-}
SRC=/var/tmp/fast-mi/e011-r2int/b3f/r2-fast
P=$OUT/p/$NAME; rm -rf $P; mkdir -p $P/jobs/$J $P/tmp
disk_ok $OUT || exit 1
ln -s $(readlink -f $SRC/entries) $P/entries
ln -s $(readlink -f $SRC/entries.jsonl) $P/entries.jsonl
sed "s#$SRC#$P#g" $SRC/jobs/$J/job.json > $P/jobs/$J/job.json
echo "$OVR" > $P/overrides.txt
t0=$(date +%s.%N)
env $ON $MI $OVR FMI_READING_CACHE=$P/rcache FMI_MEMO= FMI_PLAN_CACHE=$P/plans FMI_POOL_DIR=$P/pool TMPDIR=$P/tmp E008_DISK_PATH=$OUT E008_FLOOR_GB=$FLOOR_GB \
  timeout 900 nice -n $NICE $FMI_BIN try $P/jobs/$J --search fd --tail > $P/try.log 2>&1
rc=$?
w=$(awk "BEGIN{printf \"%.1f\", $(date +%s.%N) - $t0}")
r=$(jq -c '{why: .reading.why, read_s: .reading.read_s, read_wall_s: .reading.read_wall_s, read_cpu_s: .reading.read_cpu_s, records: .reading.records, bound: .reading.bound.waited_s, rss: .reading.peak_rss_mb, stop: .attempt.stop, wall_s}' $P/jobs/$J/result.json 2>/dev/null | cut -c1-400)
echo "$NAME $J rc $rc wall $w s (overrides: ${OVR:-none}): $r" | tee -a $OUT/summary.txt
rm -rf $P/pool $P/tmp $P/readings $P/rcache $P/jobs/$J/run
