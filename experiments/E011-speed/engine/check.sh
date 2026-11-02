#!/usr/bin/env bash
# E011 engine gates (plan.md "Exactness gates" G1-G5), on the laptop at nice 10, seed 1. Outputs under
# ~/.cache/fast-mi-e011/engine/ (dumps hold game text: they stay there).
#
# usage: check.sh g2|g3|g4|g1|g5|all
#   g2  the door chain (E009 door archive, entry 5, frame 3129): 37 exact dumps of svm-w3b (chain.py), then of the
#       E011 build through the Python reference with the lean switches off, each alone and all four, and through
#       fmi-engine (binary channel, all four, joystick_num=-1); each compared with svm-w3b's; G4 at every Rust dump
#   g3  the kitchen chain (E009 off check, jobs/ta/new entry 7, frame 3216): 40 dumps, the same runs
#   g4  fmi-engine walk: 400 frames from 2728 on the kitchen movie, one Step each, against State, Obs and JSON
#   g1  E003's fork_exact.py run on the lean build (3,000 frames from 235, 7 load children)
#   g5  E009's step test (a) through the Python reference on the lean build (off_a.py)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WT="$(cd "$HERE/../../.." && pwd)"
O=${E011_ENGINE_OUT:-$HOME/.cache/fast-mi-e011/engine}
PY=$WT/.venv/bin/python
W3B=$HOME/.cache/fast-mi/svm-w3b/build/scummvm/scummvm
LEANW=$HERE/svm-lean.sh
FMI=$WT/experiments/E011-speed/rs/fmi-engine/target/release/fmi-engine
DOOR=$HOME/.cache/fast-mi-e009/archives/door
KITCHEN=$O/kitchen-arch # entries -> E009's off check job's new entries
mkdir -p "$O" "$KITCHEN"
[ -e "$KITCHEN/entries" ] || ln -s "$HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/new" "$KITCHEN/entries"
source "$HOME/.cache/fast-mi-e009/env.sh"
say() { echo "[$(date +%H:%M:%S)] $*"; }

chain() { # NAME ARCH IDX: the reference's dumps and the build's, compared
    local name=$1 arch=$2 idx=$3 d=$O/$1 bad=0
    rm -rf "$d"; mkdir -p "$d"
    say "$name: svm-w3b"
    FAST_MI_SCUMMVM_BIN=$W3B nice -n 10 $PY "$HERE/chain.py" dumps "$d/base" "$arch" "$idx" > "$d/base.out" 2>&1 \
        || { cat "$d/base.out"; return 1; }
    tail -1 "$d/base.out"
    for sw in none splash present events audio all; do
        FAST_MI_SCUMMVM_BIN=$LEANW LEAN=$sw nice -n 10 $PY "$HERE/chain.py" dumps "$d/$sw" "$arch" "$idx" \
            > "$d/$sw.out" 2>&1 || { cat "$d/$sw.out"; bad=1; continue; }
        local c; c=$($PY "$HERE/chain.py" compare "$d/base/dumps.jsonl" "$d/$sw/dumps.jsonl" | tail -1)
        say "$name lean=$sw: $(tail -1 "$d/$sw.out" | cut -c1-120) | $c"
        case "$c" in *", 0 differ") ;; *) bad=1 ;; esac
    done
    nice -n 10 "$FMI" dumps "$d/rust" "$arch" "$idx" --lean --env SPEEDRUN_DUMP_EXACT=1 > "$d/rust.out" 2> "$d/rust.err" \
        || bad=1
    local c; c=$($PY "$HERE/chain.py" compare "$d/base/dumps.jsonl" "$d/rust/dumps.jsonl" | tail -1)
    say "$name fmi-engine (bin, lean): $(cat "$d/rust.out") | $c"
    head -5 "$d/rust.err"
    case "$c" in *", 0 differ") ;; *) bad=1 ;; esac
    return $bad
}

g2() { chain g2-door "$DOOR" 5; }
g3() { chain g3-kitchen "$KITCHEN" 7; }

g4() {
    rm -rf "$O/g4"; mkdir -p "$O/g4"
    say "g4: walk 400 frames from 2728 (kitchen movie)"
    nice -n 10 "$FMI" walk "$O/g4" "$KITCHEN" 7 2728 400 --lean --env SPEEDRUN_DUMP_EXACT=1 2>&1 | tail -12
    return "${PIPESTATUS[0]}"
}

g1() {
    rm -rf "$O/g1"; mkdir -p "$O/g1"
    say "g1: fork_exact.py run on the lean build"
    (cd "$WT/experiments/E003-path-through-cloud" && LEAN=all nice -n 10 $PY fork_exact.py run --bin "$LEANW" \
        --out "$O/g1" > "$O/g1/run.out" 2> "$O/g1/run.err")
    local rc=$?
    tail -25 "$O/g1/run.out"
    return $rc
}

g5() {
    rm -rf "$O/g5"; mkdir -p "$O/g5"
    say "g5: step test (a) on the lean build"
    CHECK_OUT=$O/g5/step-a FAST_MI_SCUMMVM_BIN=$LEANW LEAN=all nice -n 10 $PY -u \
        "$HOME/.cache/fast-mi-e009/scripts/off_a.py" > "$O/g5/off-a.log" 2>&1
    local rc=$?
    grep -e '^CHECK' -e '^\[a\]' -e 'window search' "$O/g5/off-a.log"
    return $rc
}

case "${1:-}" in
    g1|g2|g3|g4|g5) "$1"; rc=$?; say "$1 exit $rc"; exit $rc ;;
    all) for g in g2 g3 g4 g1 g5; do $g; say "$g exit $?"; done ;;
    *) sed -n '2,15p' "$0"; exit 2 ;;
esac
