#!/usr/bin/env bash
# E016 amendments (3) and (4): E015's G1 gate rerun with the fixed approach (E015 results/g1/run.sh "gate", with this
# branch's e015 and outputs here): from play8's entry 0, d120, s10:596, d120; on to room 63; verified from boot. Build:
#   cd experiments/E011-speed/rs && CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e016-a cargo build --release -p fmi-checks --bin e015
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
source "$here/../g3/env-demo.sh"
BIN=${E015_BIN:-/var/tmp/fast-mi/target-e016-a/release/e015}
OUT=${OUT:-/var/tmp/fast-mi/e016-a-out/g1}
nice -n 10 "$BIN" realize "$OUT" /var/tmp/fast-mi/e014-archives/play8 0 --steps d120,s10:596,d120 --room 63 --run-on 600 --verify > /dev/null
cp "$OUT/realize.json" "$here/realize.json"
cp "$OUT/events.jsonl" "$here/events.jsonl"
