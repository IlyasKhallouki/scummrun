#!/bin/bash
# E012 afford's gates A1-A3 (plan.md, "Components"; the amendment "component afford"), on the kitchen archive of E011
# (read only) and round 2's engine. Usage: gates.sh OUT [ENTRY...]
#   A2: M4's form (fixed horizon, no third click) name for name against M4's probe files: 203 (400 frames), 205 and 4
#       (600 frames).
#   A1: the try's form at each ENTRY (default: the probe's entries), every kept entry verified from boot.
#   A3: the try's form at 203, five times on the same engine: the wall per trial.
# Env: AFF (afford-check), ARCH (the archive), TESTED (T's keys, one per line), PROBE (M4's probe outputs),
# FMI_ENGINE_BIN, FMI_ENGINE_CUTS.
set -u
out=$1; shift
entries=${*:-"0 2 4 49 67 89 203 204 205 209 222 224 225"}
mkdir -p "$out"
for x in "203 400" "205 600" "4 600"; do
  set -- $x
  nice -n 10 "$AFF" m4 "$out/m4-$1" "$ARCH" "$1" --tested "$TESTED" --horizon "$2" --probe "$PROBE/aff-$1/afford.tsv" \
    > "$out/m4-$1.json" 2> "$out/m4-$1.err" || echo "m4 $1: rc $?"
done
for i in $entries; do
  nice -n 10 "$AFF" run "$out/run-$i" "$ARCH" "$i" --tested "$TESTED" --verify > "$out/run-$i.json" 2> "$out/run-$i.err" \
    || echo "run $i: rc $?"
done
uptime > "$out/a3.load"
nice -n 10 "$AFF" run "$out/a3-203" "$ARCH" 203 --tested "$TESTED" --repeat 5 > "$out/a3-203.json" 2> "$out/a3-203.err" \
  || echo "a3: rc $?"
uptime >> "$out/a3.load"
