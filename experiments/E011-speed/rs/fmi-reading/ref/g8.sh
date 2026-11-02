#!/bin/bash
# readio's G8 (experiments/E011-speed/plan.md) on one reading: fmi-reading (on NEW, reader-scope --out-dir's) against
# the Python reference (on REF, read_rust's files; the same bytes by G6): action(i) and record(i) every EVERY-th
# action, by_name on every action's name and on names no action has, E009's K; then the index built from the files
# against reader-scope's, and the measures.
# usage: g8.sh FMI_READING_BIN REF_DIR NEW_DIR EVERY OUT_DIR
set -u
B=$1; REF=$2; NEW=$3; EVERY=$4; OUT=$5
R=$(cd "$(dirname "$0")" && pwd)
PY=$(cd "$R/../../../../.." && pwd)/.venv/bin/python
mkdir -p "$OUT"
name=$(basename "$NEW")
ok=1
$B dump "$NEW" --every "$EVERY" --out "$OUT/$name.rust.jsonl"
$PY "$R/dump_reading.py" dump "$REF" --every "$EVERY" --out "$OUT/$name.py.jsonl"
if cmp -s "$OUT/$name.rust.jsonl" "$OUT/$name.py.jsonl"; then
  echo "dump: same, $(wc -l < "$OUT/$name.rust.jsonl") actions, sha1 $(sha1sum < "$OUT/$name.rust.jsonl" | cut -c1-12)"
else
  echo "dump: DIFFERENT"; ok=0
fi
$B names "$NEW" --out "$OUT/$name.names.jsonl"
$PY "$R/dump_reading.py" names "$REF" "$OUT/$name.names.jsonl" || ok=0
$B reuse-keys "$NEW" > "$OUT/$name.reuse.json"
$PY "$R/dump_reading.py" reuse "$REF" "$OUT/$name.reuse.json" || ok=0
$B idx "$NEW"
$B bench "$NEW" --every "$EVERY" --reps 5
echo "G8 $name: $([ $ok = 1 ] && echo pass || echo FAIL)"
