#!/bin/bash
# E013 component refuse, gates B1, B2 and B4's sample (plan.md B; amendment (3)): recorded tries run again, each from
# its own job.json, in one resident `fmi try` (E012's integrate/b3.sh's warm regime), with the recorded run's
# settings (round 2's fast settings: --search fd --tail; ON3 and every E012 switch on, E12ON), refuse's engine and
# build, and SW (default $E13RCHECK: FMI_REFUSE=1 with B4's check). The tries' folders, then:
#   refuse-check b1 SRC OUT/NAME OUT/NAME-b1          the windows against the recorded ones (B1)
#   refuse-check verify OUT/NAME OUT/NAME-verify      every new entry from boot (B2, rule (2))
# SRC kitchen: the E012 kitchen archive's 53 jobs (their readings are read where the archive keeps them, and a fresh
# reading goes to the run's own folder); b3: E012's B3 tries with everything on (integrate's e-fast sequence).
# The reading cache is off (its hard links would keep every reading's bulk) and each job's reading bulk is deleted
# after it (its windows and entries do not read it). The heavy lock is held for the
# run, the resident process under the guard at nice 10.
# usage: ENV=refuse/env.sh rerun.sh OUT kitchen|b3 [JOBS...]
set -u
source "${ENV:?an env file (refuse/env.sh)}"
OUT=$1; NAME=$2; shift 2
case $NAME in
  kitchen) SRC=/var/tmp/fast-mi/e012-archives/kitchen;;
  b3) SRC=/var/tmp/fast-mi/e012-int/b3f/e-fast;;
  *) echo "kitchen or b3"; exit 2;;
esac
SW=${SW:-$E13RCHECK}
R=$OUT/$NAME
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
jobs="$*"
[ -n "$jobs" ] || jobs=$(ls $SRC/jobs | grep '^j[0-9]')
mkdir -p $R/jobs $R/tmp $R/readings
ln -sfn $(readlink -f $SRC/entries) $R/entries
ln -sfn $(readlink -f $SRC/entries.jsonl) $R/entries.jsonl
for j in $jobs; do
  mkdir -p $R/jobs/$j
  if [ $NAME = kitchen ]; then
    # the readings stay where the archive keeps them; a fresh one goes to this run's folder
    sed "/\"reading_dir\"/s#$SRC#$R#" $SRC/jobs/$j/job.json > $R/jobs/$j/job.json
  else
    sed "s#$SRC#$R#g" $SRC/jobs/$j/job.json > $R/jobs/$j/job.json
  fi
done
exec 201>$LOCK; flock 201
echo "[$(date +%T)] heavy lock taken; $NAME: $(echo $jobs | wc -w) jobs; SW=$SW"
FIFO=$R/jobs.fifo; rm -f $FIFO; mkfifo $FIFO
env $ON3 $E12ON $SW FMI_ENGINE_BIN=$E13_ENGINE_BIN FMI_READING_CACHE= FMI_MEMO= FMI_PLAN_CACHE=$R/plans \
  FMI_POOL_DIR=$R/pool TMPDIR=$R/tmp E008_DISK_PATH=/var/tmp E008_FLOOR_GB=5 \
  $GUARD $R/fmi.log $FMI_BIN try @$FIFO --search fd --tail &
G=$!
exec 7>$FIFO
sleep 1
for j in $jobs; do
  d=$R/jobs/$j
  t0=$(date +%s.%N)
  echo "$d" >&7
  until grep -q "^done $d " $R/fmi.log 2>/dev/null; do
    kill -0 $G 2>/dev/null || { echo "the resident fmi died"; break 2; }
    sleep 0.2
  done
  t1=$(date +%s.%N)
  find $d $R/readings \( -name 'reading.tc' -o -name 'reading.actions.jsonl' -o -name 'reading.lists.jsonl' \
    -o -name 'reading.idx' -o -name 'reading.reached' \) -delete 2>/dev/null
  echo "[$(date +%T)] $j $(awk "BEGIN {printf \"%.1f\", $t1 - $t0}") s: $(grep "^done $d " $R/fmi.log | cut -c1-150)"
  for fs in / /home; do
    df -B1G --output=avail $fs | tail -1 | awk -v f=$fs '$1 < 5 { print "STOP: " f " under 5 GB"; exit 1 }' || break 2
  done
done
exec 7>&-
wait $G
tail -2 $R/fmi.log
flock -u 201; exec 201>&-
echo "[$(date +%T)] heavy lock released"
C=$E12T/checks/release/refuse-check
$C b1 $SRC $R $R-b1 | tail -20
env FMI_ENGINE_BIN=$E13_ENGINE_BIN nice -n 10 $C verify $R $R-verify | tail -2
