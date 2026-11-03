#!/bin/bash
# E013 gate A3 on the handler rule (plan.md A3, amendment (2) (a)): fmi-checks' a3 reads the old rule (SPEEDRUN_IDLE
# unset) and the handler rule (SPEEDRUN_IDLE=handler) on the same states, in two lean engines (E013's), with every other
# switch unset:
#   chain    E012's goal chain (the kitchen archive's entry 150), every frame from the root to the goal
#   e012     every checkpoint (entry) of E012's kitchen archive
#   e011     every checkpoint (entry) of E011's kitchen archive
#   offers   E012's affordance offers (the kitchen archive's tries' e012.afford), and the gate on those that kept an
#            outcome
# Archives are read only. usage: ENV=integrate/env.sh a3.sh OUT [chain|e012|e011|offers]...
set -u
source "${ENV:?an env file (integrate/env.sh)}"
O=$1; shift; mkdir -p $O/tmp
A3=$E13T/checks/release/a3
K12=/var/tmp/fast-mi/e012-archives/kitchen
K11=/var/tmp/fast-mi/e011-archives/kitchen
run() { env -u SPEEDRUN_IDLE $E14MOFF $E13OFF FMI_ENGINE_BIN=$E13_ENGINE_BIN TMPDIR=$O/tmp nice -n 10 $A3 "$@"; }
for st in "$@"; do
  case $st in
    chain) run chain $K12 150 $O/chain.jsonl > $O/chain.out 2>&1; echo "[$(date +%T)] chain rc $?: $(tail -1 $O/chain.out | cut -c1-600)";;
    e012) run entries $K12 $O/e012.jsonl > $O/e012.out 2>&1; echo "[$(date +%T)] e012 rc $?: $(tail -1 $O/e012.out | cut -c1-600)";;
    e011) run entries $K11 $O/e011.jsonl > $O/e011.out 2>&1; echo "[$(date +%T)] e011 rc $?: $(tail -1 $O/e011.out | cut -c1-600)";;
    offers) run offers $K12 $O/offers.jsonl > $O/offers.out 2>&1; echo "[$(date +%T)] offers rc $?: $(tail -1 $O/offers.out | cut -c1-600)";;
    *) echo "unknown stage $st";;
  esac
done
