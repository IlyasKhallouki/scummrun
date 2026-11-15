#!/bin/bash
# E013 gate A1 for patch 0007 (plan.md A1, amendment (2) (a)): E011's engine gates G2, G3 and G4 (engine/check-r2.sh)
# on the build with patches 0001-0007, SPEEDRUN_INPUT off and on (engine/scummvm-input.sh), and G1 (fork_exact.py, 7
# load children) with it on. Logs a line per gate to OUT/summary.txt.
# usage: ENV=refuse/env.sh a1.sh OUT [g1]
set -u
source "${ENV:?an env file (refuse/env.sh)}"
here=$(cd "$(dirname "$0")" && pwd)
X=$WT11/experiments/E011-speed; O=$1; mkdir -p $O; S=$O/summary.txt
log() { echo "[$(date +%T)] $*" | tee -a $S; }
export FMI_ENGINE_TOOL=$E12T/release/fmi-engine
for mode in off on; do
  eb=$E13_ENGINE_BIN; [ $mode = on ] && eb=$here/../engine/scummvm-input.sh
  mkdir -p $O/engine-$mode
  for g in g2 g3 g4; do
    END_ENGINE_BIN=$E13_ENGINE_BIN E011_ENGINE_BIN=$eb E011_ENGINE_OUT=$O/engine-$mode nice -n 10 $X/engine/check-r2.sh $g > $O/engine-$mode/$g.log 2>&1
    log "A1 $mode $g exit $?: $(tail -3 $O/engine-$mode/$g.log | tr '\n' ' ' | cut -c1-400)"
  done
done
if [ "${2:-}" = g1 ]; then
  flock /var/tmp/fast-mi/heavy.lock $HOME/.cache/fast-mi-e007/guard.sh $O/engine-on/g1.guard.log env E011_ENGINE_BIN=$here/../engine/scummvm-input.sh E011_ENGINE_OUT=$O/engine-on $X/engine/check-r2.sh g1
  log "A1 on g1: $(grep -v GUARD $O/engine-on/g1.guard.log | tail -4 | tr '\n' ' ' | cut -c1-400) $(grep GUARD $O/engine-on/g1.guard.log)"
fi
