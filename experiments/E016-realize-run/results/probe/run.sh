#!/usr/bin/env bash
# E016 amendment (1)'s probe: how a walk_to step's run shows in patch 0007's input log, on E012's kitchen chain
# (archive /var/tmp/fast-mi/e012-archives/kitchen, read only). Build first:
#   cd experiments/E011-speed/rs && CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e016-a cargo build --release -p fmi-checks --bin e016
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
source "$here/env-mi.sh"
A=/var/tmp/fast-mi/e012-archives/kitchen
OUT=${OUT:-/var/tmp/fast-mi/e016-a-out/probe}
mkdir -p "$OUT"
# the goal entry's chain, its events from boot in chain order (entry 0 is the boot, no events)
for i in 0 44 55 85 123 135 142 146 150; do zcat "$A/entries/$i.json.gz" | jq -c '.events[]'; done > "$OUT/kitchen-chain.jsonl"
# walk_to(444) at 10161 and walk_to(367) at 10163; walk_to(316)'s window (18318-18365) and sentence(9,566) (18405-18406)
nice -n 10 "$E016_BIN" inlog "$OUT/walk-10160" --events "$OUT/kitchen-chain.jsonl" --from 10160 --to 10200 > /dev/null
nice -n 10 "$E016_BIN" inlog "$OUT/walk-18300" --events "$OUT/kitchen-chain.jsonl" --from 18300 --to 18420 > /dev/null
cp "$OUT/walk-10160/inlog.json" "$here/walk-10160.json"
cp "$OUT/walk-18300/inlog.json" "$here/walk-18300.json"
