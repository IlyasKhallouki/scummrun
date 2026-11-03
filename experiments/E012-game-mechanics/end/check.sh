#!/usr/bin/env bash
# E012 component end (plan.md "Components", row end, and the amendment "component end"): patch 0005's gates, on the
# laptop at nice 10, seed 1. Outputs under /var/tmp/fast-mi/e012-end/ (dumps hold game text: they stay there).
#
# usage: check.sh off|on|g1off|g1on|hooks|boxes|unreach|tests
#   off      E011's engine gates G2, G3 and G4 (engine/check-r2.sh) on patch 0005's build, the switch off
#   on       the same with SPEEDRUN_REPORT_RESTART=1 in every engine's environment (scummvm-report.sh)
#   g1off    E011's G1 (fork_exact.py, 7 load children) on patch 0005's build, the switch off, under the heavy lock
#   g1on     the same, the switch on
#   hooks    fmi-engine-end hooks at the E011 kitchen check's entries 0 (room 33, frame 2728) and 203 (room 28, 3393)
#   boxes    fmi-engine-end boxes at one entry in each of the six rooms the kitchen check entered
#   unreach  fmi-engine-end unreach over every recorded archive: no recorded chain reaches a hook
#   tests    cargo test of fmi-engine, the engine tests included (--include-ignored)
# Environment: END_ENGINE_BIN (patch 0005's build), END_TARGET (the cargo target with fmi-engine and fmi-engine-end).
set -uo pipefail
source "$HOME/.cache/fast-mi-e011/env.sh" # first: it sets WT and PY to E009's, which the lines below replace
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WT="$(cd "$HERE/../../.." && pwd)"
O=${END_OUT:-/var/tmp/fast-mi/e012-end}
BIN=${END_ENGINE_BIN:-/var/tmp/fast-mi/svm-e012-end/build/scummvm/scummvm}
T=${END_TARGET:-/var/tmp/fast-mi/target-e012-end}
C11=$WT/experiments/E011-speed/engine/check-r2.sh
END=$T/release/fmi-engine-end
KITCHEN=/var/tmp/fast-mi/e011-archives/kitchen # the E011 kitchen check's archive (read only)
mkdir -p "$O"
say() { echo "[$(date +%H:%M:%S)] $*"; }
export FMI_ENGINE_TOOL=$T/release/fmi-engine FMI_ENGINE_BIN=$BIN END_ENGINE_BIN=$BIN

e011() { # MODE GATE...: E011's check-r2.sh gates, with the switch off (the build) or on (the wrapper)
    local mode=$1 rc=0; shift
    local eb=$BIN; [ "$mode" = on ] && eb=$HERE/scummvm-report.sh
    mkdir -p "$O/$mode"
    for g in "$@"; do
        E011_ENGINE_BIN=$eb E011_ENGINE_OUT=$O/$mode "$C11" "$g" > "$O/$mode/$g.log" 2>&1
        local r=$?
        say "$mode $g exit $r"; tail -4 "$O/$mode/$g.log"
        [ $r = 0 ] || rc=1
    done
    return $rc
}

g1() { # MODE: G1 under the heavy lock (its 7 load children take the machine's CPUs)
    flock /var/tmp/fast-mi/heavy.lock "$HOME/.cache/fast-mi-e007/guard.sh" "$O/g1$1.guard.log" \
        bash -c "END_OUT=$O $0 _g1 $1"
    local r=$?; tail -2 "$O/g1$1.guard.log"; return $r
}

archives() { # every recorded archive with real movies (fake-step and test archives left out)
    local k=$O/e009-off-kitchen
    mkdir -p "$k"; [ -e "$k/entries" ] || ln -s "$HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/new" "$k/entries"
    echo "$HOME/.cache/fast-mi-e008/archives/kitchen $HOME/.cache/fast-mi-e008/archives/smoke"
    echo "$HOME/.cache/fast-mi-e008/archives/door $HOME/.cache/fast-mi-e008-check/archives/kitchen"
    echo "$HOME/.cache/fast-mi-e008-check/archives/smoke $HOME/.cache/fast-mi-e008-check/archives/door"
    echo "$HOME/.cache/fast-mi-e009/archives/door $HOME/.cache/fast-mi-e009/laptop-verify/door $k"
    echo "$KITCHEN /var/tmp/fast-mi/e011-bench-data/b0"
    for s in a-fast a-ref b-fast b-ref; do echo "$HOME/.cache/fast-mi-e011/bench/b3/$s"; done
    for s in b3/r1-ref b3/r2-ref b3f/r1-fast b3f/r2-fast b3o/r2off-ref; do echo "/var/tmp/fast-mi/e011-r2int/$s"; done
}

case "${1:-}" in
    off|on) e011 "$1" g2 g3 g4 ;;
    g1off) g1 off ;;
    g1on) g1 on ;;
    _g1) e011 "$2" g1 ;;
    hooks)
        rc=0
        for i in 0 203; do
            nice -n 10 "$END" hooks "$O/hooks-$i" "$KITCHEN" $i --lean > "$O/hooks-$i.log" 2>&1 || rc=1
            cat "$O/hooks-$i.log"
        done
        say "hooks exit $rc"; exit $rc ;;
    boxes)
        nice -n 10 "$END" boxes "$O/boxes" "$KITCHEN" 0 15 49 67 89 203 --lean 2>&1 | tee "$O/boxes.log"
        r=${PIPESTATUS[0]}; say "boxes exit $r"; exit $r ;;
    unreach)
        rm -rf "$O/unreach"
        "$HOME/.cache/fast-mi-e007/guard.sh" "$O/unreach.log" "$END" unreach "$O/unreach" $(archives) \
            --lean --threads 2
        r=$?; cat "$O/unreach.log"; say "unreach exit $r"; exit $r ;;
    tests)
        cd "$WT/experiments/E011-speed/rs" && CARGO_TARGET_DIR=$T nice -n 10 cargo test --release -j 2 -p fmi-engine \
            -- --include-ignored 2>&1 | grep -E "^(test |test result|running)" ;;
    *) sed -n '2,15p' "$0"; exit 2 ;;
esac
