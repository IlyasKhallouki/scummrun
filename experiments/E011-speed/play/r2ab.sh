#!/bin/bash
# E011 round 2, play: before and after, interleaved run by run so both meet the same load: test (a)'s windows through
# play-lean (r2bench.sh) with the round-1 binary on the round-1 engine and the new binary on the new engine.
# usage: r2ab.sh OUT ROUNDS TEST BEFORE_BIN BEFORE_ENGINE AFTER_BIN AFTER_ENGINE [ENV=V ...]
#   TEST: ready or strict; the ENV assignments apply to the "after" runs only
# Prints, per run and window: wall, the observed child frame, this process's CPU, the children's engines' CPU.
set -u
here=$(cd "$(dirname "$0")" && pwd)
OUT=$1; N=$2; T=$3; B0=$4; E0=$5; B1=$6; E1=$7; shift 7
mkdir -p $OUT
summ() { # LOG_ERR
  awk '/\[play\]   window [0-9]+: [0-9]+ observed child frames/ {obs=$4" x "$8}
       /\[play\]   window [0-9]+: CPU s:/ {cpu=$8" "$12" "$16; wall=$(NF-1)}
       /\[play\]   window [0-9]+: input code:/ {split($0,a,"children, "); split(a[2],b," "); ic=b[1]}
       /\[play\]   window [0-9]+: thread CPU s:/ {
         printf "  win %d: wall %s s, observed %s ms, input code %s ms/child, CPU this/parent/children %s\n", ++k, wall, obs, ic, cpu }' "$1"
}
for i in $(seq 1 $N); do
  for side in before after; do
    if [ $side = before ]; then B=$B0; E=$E0; envs=""; else B=$B1; E=$E1; envs="$*"; fi
    d=$OUT/$i-$side
    env $envs bash $here/r2bench.sh $B $E $d $T > $d.txt 2>&1
    echo "== round $i $side: $(grep -E '^(READY|STRICT)' $d/$T.log | cut -c1-90) | $(grep '^TIME' $d.txt) | $(grep '^===' $d.txt | cut -d' ' -f4-8)"
    summ $d/$T.err
  done
done
