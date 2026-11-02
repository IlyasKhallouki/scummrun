#!/bin/bash
# readio: a reading by reader-scope --out-dir with E009's switches, the arguments read_rust (stream) gives E009's
# binary (ref_read.py) but for --packed/--golden.
# usage: new_read.sh BIN ROOMS START_ROWS OUT_DIR [extra reader-scope args: --fact-vars, --reach-also, --golden ...]
set -u
BIN=$1; ROOMS=$2; ROWS=$3; OUT=$4; shift 4
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
exec nice -n 10 "$BIN" --rooms "$ROOMS" --start-seed 1 --domain-goals none --goal owner:566=1 --name "$(basename "$OUT")" \
  --threads "${THREADS:-3}" --out "$OUT" --out-dir "$OUT" --elsewhere --live-locals --split-limit \
  --switches value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost --reach --frame-costs --start-rows "$ROWS" "$@"
