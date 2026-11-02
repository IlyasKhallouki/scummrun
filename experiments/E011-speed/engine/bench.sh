#!/usr/bin/env bash
# E011 engine bench (plan.md B1's engine part), before and after back to back on the same load, seed 1, nice 10:
#   before: E010's frame_costs.py (the window search's frame costs a-d through the JSON protocol) and the profile's
#           chain.py speed (start, boot to 2728, the door chain to 3129), both on svm-w3b;
#   after:  fmi-engine bench (the same categories through the binary channel, start, return by fork of a kept entry,
#           the door chain from boot), on the E011 build with the lean switches, and without them (channel only).
# usage: bench.sh [ROUNDS]   outputs under ~/.cache/fast-mi-e011/engine/bench/
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WT="$(cd "$HERE/../../.." && pwd)"
O=${E011_ENGINE_OUT:-$HOME/.cache/fast-mi-e011/engine}/bench
PY=$WT/.venv/bin/python
W3B=$HOME/.cache/fast-mi/svm-w3b/build/scummvm/scummvm
FMI=$WT/experiments/E011-speed/rs/fmi-engine/target/release/fmi-engine
DOOR=$HOME/.cache/fast-mi-e009/archives/door
FC=/var/tmp/fast-mi/wt-e010/experiments/E010-play-when-ready/measure/frame_costs.py
ROUNDS=${1:-2}
mkdir -p "$O"
for r in $(seq 1 "$ROUNDS"); do
    echo "== round $r $(date +%H:%M:%S) load $(cut -d' ' -f1 /proc/loadavg)"
    rm -rf "$O/fc$r" "$O/sp$r" "$O/after$r" "$O/chan$r"
    echo "-- before: frame_costs.py on svm-w3b"
    FAST_MI_SCUMMVM_BIN=$W3B nice -n 10 $PY "$FC" "$O/fc$r" 300 2>&1 | tail -7
    echo "-- before: chain.py speed on svm-w3b"
    FAST_MI_SCUMMVM_BIN=$W3B nice -n 10 $PY "$HERE/chain.py" speed "$O/sp$r" "$DOOR" 5 3 2>&1 | tail -3
    echo "-- after: fmi-engine bench, lean"
    nice -n 10 "$FMI" bench "$O/after$r" "$DOOR" 5 300 --lean 2>/dev/null
    echo "-- after: fmi-engine bench, the channel alone (lean off)"
    nice -n 10 "$FMI" bench "$O/chan$r" "$DOOR" 5 300 2>/dev/null
done
echo "== done $(date +%H:%M:%S) load $(cut -d' ' -f1 /proc/loadavg)"
