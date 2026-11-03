#!/bin/bash
# E011 round 2, item own (the try's own time): one fixed try run N times in one fmi process with FMI_TRY_PROFILE
# (play when ready, Gbfs with the old tail), under the heavy lock and the guard. The jobs (own_prep.py):
#   p    B0's j00004, ring(38): entry 6 at frame 2852, B0's j00002 reading reused (E009's keys), target bit:110 = 1,
#        1,600 idle frames;
#   q12  B0's j00012, ring(33): entry 14 at frame 2768, B0's j00000 refined reading reused, target bit:115 = 1, no idle;
#   q6   B0's j00006, ring(33): entry 1, the same reading, target bit:111 = 1, 1,600 idle frames.
# usage: own_try.sh FMI_BIN TAG JOB [N] [ENV=VALUE...]     (data: ~/.cache/fast-mi-e011/own)
set -u
source $HOME/.cache/fast-mi-e011/env.sh
FMI=$1; TAG=$2; JOB=$3; N=${4:-7}; shift 4 2>/dev/null || shift $#
D=$HOME/.cache/fast-mi-e011/own; R=$D/runs/$TAG
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
rm -rf $R; mkdir -p $R/jobs/$JOB
cp $D/jobs/$JOB/job.json $R/jobs/$JOB/
# the archive's root for the targets' hold rule (R/jobs/JOB -> R/entries.jsonl, as B3's sequences)
ln -s $D/entries $R/entries; ln -s $D/entries.jsonl $R/entries.jsonl
export TMPDIR=/var/tmp/fast-mi/e011-tmp FMI_TRY_PROFILE=1
# p peaks at about 0.15 GB (fmi) and 0.2 GB (engines): under the guard alone; the ring(33) jobs (1.3 GB) under the lock
case $JOB in p) L=;; *) L="flock $LOCK";; esac
$L env "$@" FMI_READING_CACHE=$R/rcache FMI_MEMO=$R/memo FMI_PLAN_CACHE=$R/plans FMI_POOL_DIR=$R/pool \
  $GUARD $R/fmi.log $FMI try $R/jobs/$JOB --repeat $N --search gbfs --tail
grep -E 'own time|GUARD' $R/fmi.log
rm -rf $R/pool $R/rcache $R/memo
