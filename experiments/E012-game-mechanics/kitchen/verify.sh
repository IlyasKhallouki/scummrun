#!/bin/bash
# E012's kitchen rerun: the goal entry verified from boot twice (plan.md "Runs"):
#   1. a fresh lean engine through `fmi explore verify` (end's engine), on the archive;
#   2. svm-w3b through E008's `explore.py verify` (the reference, E009's environment), on a copy of the archive's
#      files (explore.py writes its verify-ID.json where fmi's is).
# usage: ENV=kitchen/env.sh verify.sh [ENTRY]   (default: the archive's first goal entry)
set -u
source "${ENV:?an env file (kitchen/env.sh)}"
E=${1:-}
arg=${E:+--entry $E}
echo "[$(date +%T)] fmi explore verify $K_ARCH $arg"
env FMI_ENGINE_BIN=$E12_ENGINE_BIN nice -n 10 $FMI_BIN explore verify $K_ARCH $arg 2>&1 | tee $K_OUT/verify-fmi.out
C=$K_OUT/pyverify/kitchen
rm -rf $C && mkdir -p $C
cp -r $K_ARCH/{stage.json,entries.jsonl,entries,log.jsonl,cells.json,readings.jsonl} $C/
echo "[$(date +%T)] explore.py verify $C $arg (FAST_MI_SCUMMVM_BIN $FAST_MI_SCUMMVM_BIN)"
(cd $WT11/experiments/E008-go-explore && nice -n 10 $PY -u explore.py verify $C $arg) 2>&1 | tee $K_OUT/verify-py.out
