#!/usr/bin/env bash
# E015 item gate: the commands as run on 2026-11-04 (laptop, one engine host, nice -n 10). Outputs in $OUT; the
# summaries and small logs were copied here. Build: e015-item at ba98b46 (tool) / 844e1ae (plan amendments).
set -euo pipefail
cd /var/tmp/fast-mi/wt-e015-item/experiments/E011-speed/rs
CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015-item nice -n 10 cargo build --release -p fmi-checks --bin e015
source /var/tmp/fast-mi/wt-e015-item/experiments/E015-engine-gates/results/item/env.sh
mkdir -p $OUT && cd $OUT
# steps 1-4: exit A, room 63 -> room 69, owner:946 = ego, the chain from boot
nice -n 10 $E015 item $OUT --movie /var/tmp/fast-mi/research-scratch/gt/ev5.jsonl --room-a 63 --room-b 69 \
  --object 946 --steps 1234 > chain.json 2> chain.err
# step 5, static: start sites of local 212 and clears of class 12 on object 491
$E015 item $OUT --grep $FMI_GAME_SCRIPTS --local 212 --class-obj 491 --class 12 > grep.json
# steps 5-6: the room-42 sweep from 10 play8 entries (amendment (1)), and room 75 -> room 49
nice -n 10 $E015 item $OUT --archive /var/tmp/fast-mi/e014-archives/play8 \
  --entries 302,324,341,358,456,556,573,595,356,296 --room 42 --local 212 --class-obj 491 --class 12 \
  --room-back 75 --room-home 49 --steps 56 > sweep.json 2> sweep.err
# detector control (not gating): the same detectors on room 42's locals 213 and 202, entry 302, depth 1
for L in 213 202; do
  nice -n 10 $E015 item /var/tmp/fast-mi/e015-out/item-control/l$L --archive /var/tmp/fast-mi/e014-archives/play8 \
    --entries 302 --room 42 --local $L --class-obj 491 --class 32 --room-back 75 --room-home 49 --steps 5 --depth 1
done
