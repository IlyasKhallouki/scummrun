#!/bin/bash
# E013's kitchen rerun K1: the goal entry verified from boot (plan.md "Runs"), as E012's kitchen/verify.sh:
#   1. a fresh lean engine (E013's) through `fmi explore verify`, with E014's mechanism switches as the run had them,
#      and again with every switch unset;
#   2. svm-w3b through E008's `explore.py verify` (the frozen reference, E009's environment), on a copy of the
#      archive's files.
# usage: ENV=kitchen/env.sh verify.sh [ENTRY]   (default: the archive's first goal entry)
set -u
source "${ENV:?an env file (kitchen/env.sh)}"
E=${1:-}
arg=${E:+--entry $E}
echo "[$(date +%T)] fmi explore verify $K_ARCH $arg (E014's mechanisms on)"
env $E14M FMI_ENGINE_BIN=$E13_ENGINE_BIN nice -n 10 $FMI_BIN explore verify $K_ARCH $arg 2>&1 | tee $K_OUT/verify-fmi.out
echo "[$(date +%T)] fmi explore verify $K_ARCH $arg (every switch unset)"
env FMI_ENGINE_BIN=$E13_ENGINE_BIN nice -n 10 $FMI_BIN explore verify $K_ARCH $arg 2>&1 | tee $K_OUT/verify-fmi-off.out
C=$K_OUT/pyverify/kitchen
rm -rf $C && mkdir -p $C
cp -r $K_ARCH/{stage.json,entries.jsonl,entries,log.jsonl,cells.json,readings.jsonl} $C/
echo "[$(date +%T)] explore.py verify $C $arg (FAST_MI_SCUMMVM_BIN $FAST_MI_SCUMMVM_BIN)"
(cd $WT11/experiments/E008-go-explore && nice -n 10 $PY -u explore.py verify $C $arg) 2>&1 | tee $K_OUT/verify-py.out
