#!/bin/bash
# E012 afford's walk-box clicks measured first (plan.md, "Amendment 2026-11-03, component afford: the walk-box clicks"),
# on end's engine (patches 0001-0005): A2 again with FMI_BOXES off, then the try's form at each ENTRY with FMI_BOXES off
# and on, every kept entry verified from boot. Usage: boxes.sh OUT [ENTRY...]
# Env: as gates.sh; FMI_ENGINE_BIN must be a build with patch 0005.
set -u
out=$1; shift
entries=${*:-"0 2 4 49 67 89 203 204 205 209 222 224 225"}
mkdir -p "$out"
for x in "203 400" "205 600" "4 600"; do
  set -- $x
  FMI_BOXES=0 nice -n 10 "$AFF" m4 "$out/m4-$1" "$ARCH" "$1" --tested "$TESTED" --horizon "$2" \
    --probe "$PROBE/aff-$1/afford.tsv" > "$out/m4-$1.json" 2> "$out/m4-$1.err" || echo "m4 $1: rc $?"
done
for sw in 0 1; do
  for i in $entries; do
    FMI_BOXES=$sw nice -n 10 "$AFF" run "$out/b$sw-$i" "$ARCH" "$i" --tested "$TESTED" --verify > "$out/b$sw-$i.json" \
      2> "$out/b$sw-$i.err" || echo "run $sw $i: rc $?"
  done
done
