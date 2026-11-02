#!/bin/bash
# E011 G8 on the integrated consumer (experiments/E011-speed/plan.md, "Exactness gates"): readio's g8.sh with the
# fmi-reading tool built in the rs/ workspace (serde_json's features as in the fmi binary), on the three readings.
# The files are reader-scope's (byte for byte the reference's, G6): ring(38) and the refinement from readio's runs,
# ring(33) the plan component's copy (its index made at the first open). ring(33) under the heavy lock.
# usage: g8.sh OUT
set -u
source $HOME/.cache/fast-mi-e011/env.sh
G8=$WT11/experiments/E011-speed/rs/fmi-reading/ref/g8.sh
RIO=$HOME/.cache/fast-mi-e011/readio
OUT=$1; mkdir -p $OUT
bad=0
$G8 $FMI_READING_TOOL $RIO/ref/r38 $RIO/new/r38 1 $OUT > $OUT/r38.log 2>&1; tail -1 $OUT/r38.log
grep -q "pass$" $OUT/r38.log || bad=1
$G8 $FMI_READING_TOOL $RIO/ref/ref89 $RIO/new/ref89 1 $OUT > $OUT/ref89.log 2>&1; tail -1 $OUT/ref89.log
grep -q "pass$" $OUT/ref89.log || bad=1
R33=$HOME/.cache/fast-mi-e011/plan/r33
flock /var/tmp/fast-mi/heavy.lock $G8 $FMI_READING_TOOL $R33 $R33 47 $OUT > $OUT/r33.log 2>&1; tail -1 $OUT/r33.log
grep -q "pass$" $OUT/r33.log || bad=1
echo "G8 $([ $bad = 0 ] && echo PASS || echo FAIL)"
exit $bad
