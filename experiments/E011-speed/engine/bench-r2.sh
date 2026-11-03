#!/usr/bin/env bash
# E011 round 2's engine bench (plan.md B1's engine part, on patch 0003's cuts), before and after back to back on the
# same load, seed 1, nice 10:
#   before: round 1's build (svm-e011, patches 0001 + 0002) with the four lean switches;
#   after:  round 2's build (svm-e011-r2, + patch 0003) with the four and both cuts (SPEEDRUN_CUTS=1).
# Each round, for each side in turn:
#   1. fmi-engine bench on the door archive's entry 5 (E010's frame costs a-d, start, return by fork, the chain from
#      boot), as round 1's bench.sh runs it;
#   2. fmi-engine-frames under perf stat (perf-engine.sh): from the root (frame 2728) 0 and 3,000 frames in 50-frame
#      commands and in observed Steps; the difference is the frames' own user cycles and instructions;
#   3. the door chain from boot (fmi-engine verify, entry 5: start, 3,129 frames, 6 checkpoints) under perf stat,
#      three times.
# usage: bench-r2.sh [ROUNDS]   outputs under ~/.cache/fast-mi-e011/engine-r2/bench-r2/; bench_r2.py summarizes them
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
O=${E011_ENGINE_OUT:-$HOME/.cache/fast-mi-e011/engine-r2}/bench-r2
FMI=${FMI_ENGINE_TOOL:-/var/tmp/fast-mi/target-e011-r2-engine/release/fmi-engine}
FRAMES=${FMI_FRAMES_TOOL:-$(dirname "$FMI")/fmi-engine-frames}
OLD=${E011_ENGINE_BIN_R1:-/var/tmp/fast-mi/svm-e011/build/scummvm/scummvm}
NEW=${E011_ENGINE_BIN:-/var/tmp/fast-mi/svm-e011-r2/build/scummvm/scummvm}
DOOR=$HOME/.cache/fast-mi-e009/archives/door
ROUNDS=${1:-3}
rm -rf "$O"; mkdir -p "$O"
side() { # ROUND NAME BIN ENV...
    local r=$1 name=$2 bin=$3; shift 3
    local d=$O/r$r/$name env=() e
    for e in "$@"; do env+=(--env "$e"); done
    mkdir -p "$d"
    echo "-- r$r $name: bench (load $(cut -d' ' -f1 /proc/loadavg))"
    FMI_ENGINE_BIN=$bin nice -n 10 "$FMI" bench "$d/bench" "$DOOR" 5 300 --lean "${env[@]}" > "$d/bench.json" \
        2> "$d/bench.err"
    local m n
    for m in cmd50 step; do
        for n in 0 3000; do
            PERF_REAL=$bin PERF_MODE=stat PERF_OUT=$d/frames-$m-$n FMI_ENGINE_BIN=$HERE/perf-engine.sh nice -n 10 \
                "$FRAMES" "$d/frames-$m-$n/run" "$m" "$n" --lean "${env[@]}" > "$d/frames-$m-$n.json" 2> /dev/null
        done
    done
    for n in 1 2 3; do
        PERF_REAL=$bin PERF_MODE=stat PERF_OUT=$d/door-$n FMI_ENGINE_BIN=$HERE/perf-engine.sh nice -n 10 \
            "$FMI" verify "$d/door-$n/run" "$DOOR" 5 --lean "${env[@]}" > "$d/door-$n.json" 2> /dev/null
        rm -rf "$d/door-$n/run"
    done
    rm -rf "$d"/frames-*/run "$d/bench"/chain* "$d/bench/boot"
}
for r in $(seq 1 "$ROUNDS"); do
    echo "== round $r $(date +%H:%M:%S) load $(cut -d' ' -f1 /proc/loadavg)"
    side "$r" before "$OLD"
    side "$r" after "$NEW" SPEEDRUN_CUTS=1
done
echo "== done $(date +%H:%M:%S) load $(cut -d' ' -f1 /proc/loadavg)"
"$HERE/../../../.venv/bin/python" "$HERE/bench_r2.py" "$O"
