#!/usr/bin/env bash
# E016 G3 (plan.md, "G3"): A on an inventory object in the demo. From the item check's movie (E015
# results/item/check/carry-4-use-946.json: its first 30 clicks reach room 42 holding object 946, idle at frame 2980),
# realize s3:490, then s8:946:491; pass when room 42's local 212 runs and class 12 on object 491 is cleared within 15
# candidates, and the chain verifies from boot. Build first:
#   cd experiments/E011-speed/rs && CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e016-a cargo build --release -p fmi-checks --bin e016
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
source "$here/env-demo.sh"
OUT=${OUT:-/var/tmp/fast-mi/e016-a-out/g3}
MOVIE=$here/../../../E015-engine-gates/results/item/check/carry-4-use-946.json
nice -n 10 "$E016_BIN" g3 "$OUT" --movie "$MOVIE" --clicks 30 --frame 2980 --steps s3:490,s8:946:491 \
  --local 212 --class-obj 491 --class 12 --object 946 --run-on 600 > /dev/null
cp "$OUT/g3.json" "$here/g3.json"
cp "$OUT/events.jsonl" "$here/events.jsonl"
jq -c '{pass, candidates, all_realized, run_on, verify: .verify.differences, hook: {events_equal: .hook.events_equal, digest_equal: .hook.digest_equal, error: .hook.error}}' "$here/g3.json"
