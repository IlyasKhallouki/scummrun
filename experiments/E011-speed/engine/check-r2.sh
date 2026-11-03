#!/usr/bin/env bash
# E011 round 2's engine gates (CUTS.md; plan.md G1-G5 on patch 0003's build, and verify from boot of B3's new
# entries), on the laptop at nice 10, seed 1. Outputs under ~/.cache/fast-mi-e011/engine-r2/ (dumps hold game text:
# they stay there).
#
# usage: check-r2.sh g2|g3|g4|g1|g5|b3|all
#   g2  the door chain (E009 door archive, entry 5, frame 3129): 37 exact dumps of svm-w3b (chain.py), then of the new
#       build through the Python reference with: every switch off; each of the six alone; the four (SPEEDRUN_LEAN);
#       the four and the mixer; the four and pixels; all six; and every switch off with the pixel-free dump (the
#       reference for screens_np). A run without pixels is compared whole with svm-w3b's (chain.py compare); a run
#       with pixels without the screens digest (compare_np.py drop) and on its screens_np against the pixel-free
#       reference (compare_np.py np), whose other fields are compared with svm-w3b's too. Then fmi-engine (binary
#       channel, all six, joystick_num=-1) the same way, with G4 at every dump.
#   g3  the kitchen chain (E009 off check, jobs/ta/new entry 7, frame 3216): 40 dumps, the same runs
#   g4  fmi-engine walk: 400 frames from 2728 on the kitchen movie, one Step each, against State, Obs and JSON, all six
#   g1  E003's fork_exact.py run on the new build with all six (3,000 frames from 235, 7 load children; pixel-free
#       exact dumps)
#   g5  E009's step test (a) through the Python reference on the new build with all six (off_a.py)
#   b3  every new entry of B3's four sequences (~/.cache/fast-mi-e011/bench/b3/{a,b}-{fast,ref}) verified from boot in
#       a fresh engine of the new build with all six (fmi-engine verify, on bench/mkverify.py's two-entry archives)
# Environment: E011_ENGINE_BIN (default the round-2 build), FMI_ENGINE_TOOL (fmi-engine built from this branch).
set -uo pipefail
source "$HOME/.cache/fast-mi-e009/env.sh" # first: it sets WT and PY to E009's, which the lines below replace
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WT="$(cd "$HERE/../../.." && pwd)"
O=${E011_ENGINE_OUT:-$HOME/.cache/fast-mi-e011/engine-r2}
PY=$WT/.venv/bin/python
W3B=$HOME/.cache/fast-mi/svm-w3b/build/scummvm/scummvm
NEW=${E011_ENGINE_BIN:-/var/tmp/fast-mi/svm-e011-r2/build/scummvm/scummvm}
export E011_ENGINE_BIN=$NEW
LEANW=$HERE/svm-lean.sh
FMI=${FMI_ENGINE_TOOL:-/var/tmp/fast-mi/target-e011-r2-engine/release/fmi-engine}
DOOR=$HOME/.cache/fast-mi-e009/archives/door
KITCHEN=$O/kitchen-arch # entries -> E009's off check job's new entries
B3=$HOME/.cache/fast-mi-e011/bench/b3
mkdir -p "$O" "$KITCHEN"
[ -e "$KITCHEN/entries" ] || ln -s "$HOME/.cache/fast-mi-e009/off/step-a/jobs/ta/new" "$KITCHEN/entries"
say() { echo "[$(date +%H:%M:%S)] $*"; }
zero() { case "$1" in *", 0 differ") return 0 ;; *) return 1 ;; esac; }

chain() { # NAME ARCH IDX
    local name=$1 arch=$2 idx=$3 d=$O/$1 bad=0 c
    rm -rf "$d"; mkdir -p "$d"
    say "$name: svm-w3b"
    FAST_MI_SCUMMVM_BIN=$W3B nice -n 10 $PY "$HERE/chain.py" dumps "$d/base" "$arch" "$idx" > "$d/base.out" 2>&1 \
        || { cat "$d/base.out"; return 1; }
    tail -1 "$d/base.out"
    for sw in none nopixdump splash present events audio mixer pixels all all,mixer all,pixels all6; do
        local r=$d/${sw//,/+}
        FAST_MI_SCUMMVM_BIN=$LEANW LEAN=$sw nice -n 10 $PY "$HERE/chain.py" dumps "$r" "$arch" "$idx" \
            > "$r.out" 2>&1 || { cat "$r.out"; bad=1; continue; }
        case "$sw" in
            *pixels*|all6)
                c=$($PY "$HERE/compare_np.py" drop "$d/base/dumps.jsonl" "$r/dumps.jsonl" | tail -1)
                local c2; c2=$($PY "$HERE/compare_np.py" np "$d/nopixdump/dumps.jsonl" "$r/dumps.jsonl" | tail -1)
                say "$name lean=$sw: $(tail -1 "$r.out" | cut -c1-100) | but pixels vs svm-w3b: $c | screens_np: $c2"
                zero "$c" && zero "$c2" || bad=1 ;;
            nopixdump)
                c=$($PY "$HERE/compare_np.py" drop "$d/base/dumps.jsonl" "$r/dumps.jsonl" | tail -1)
                say "$name lean=$sw: $(tail -1 "$r.out" | cut -c1-100) | but the digest vs svm-w3b: $c"
                zero "$c" || bad=1 ;;
            *)
                c=$($PY "$HERE/chain.py" compare "$d/base/dumps.jsonl" "$r/dumps.jsonl" | tail -1)
                say "$name lean=$sw: $(tail -1 "$r.out" | cut -c1-100) | $c"
                zero "$c" || bad=1 ;;
        esac
    done
    FMI_ENGINE_BIN=$NEW nice -n 10 "$FMI" dumps "$d/rust" "$arch" "$idx" --lean --env SPEEDRUN_DUMP_EXACT=1 \
        --env SPEEDRUN_CUTS=1 > "$d/rust.out" 2> "$d/rust.err" || bad=1
    c=$($PY "$HERE/compare_np.py" drop "$d/base/dumps.jsonl" "$d/rust/dumps.jsonl" | tail -1)
    local c3; c3=$($PY "$HERE/compare_np.py" np "$d/nopixdump/dumps.jsonl" "$d/rust/dumps.jsonl" | tail -1)
    say "$name fmi-engine (bin, all six): $(cut -c1-160 "$d/rust.out") | but pixels vs svm-w3b: $c | screens_np: $c3"
    head -5 "$d/rust.err"
    zero "$c" && zero "$c3" || bad=1
    return $bad
}

g2() { chain g2-door "$DOOR" 5; }
g3() { chain g3-kitchen "$KITCHEN" 7; }

g4() {
    rm -rf "$O/g4"; mkdir -p "$O/g4"
    say "g4: walk 400 frames from 2728 (kitchen movie), all six"
    FMI_ENGINE_BIN=$NEW nice -n 10 "$FMI" walk "$O/g4" "$KITCHEN" 7 2728 400 --lean --env SPEEDRUN_DUMP_EXACT=1 \
        --env SPEEDRUN_CUTS=1 2>&1 | tail -12
    return "${PIPESTATUS[0]}"
}

g1() {
    rm -rf "$O/g1"; mkdir -p "$O/g1"
    say "g1: fork_exact.py run on the new build, all six"
    (cd "$WT/experiments/E003-path-through-cloud" && LEAN=all6 nice -n 10 $PY fork_exact.py run --bin "$LEANW" \
        --out "$O/g1" > "$O/g1/run.out" 2> "$O/g1/run.err")
    local rc=$?
    tail -25 "$O/g1/run.out"
    return $rc
}

g5() {
    rm -rf "$O/g5"; mkdir -p "$O/g5"
    say "g5: step test (a) on the new build, all six"
    CHECK_OUT=$O/g5/step-a FAST_MI_SCUMMVM_BIN=$LEANW LEAN=all6 nice -n 10 $PY -u \
        "$HOME/.cache/fast-mi-e009/scripts/off_a.py" > "$O/g5/off-a.log" 2>&1
    local rc=$?
    grep -e '^CHECK' -e '^\[a\]' -e 'window search' "$O/g5/off-a.log"
    return $rc
}

b3() {
    local bad=0 n=0
    rm -rf "$O/b3"; mkdir -p "$O/b3"
    for s in a-fast a-ref b-fast b-ref; do
        local out=$O/b3/$s
        $PY "$WT/experiments/E011-speed/bench/mkverify.py" "$B3/$s" "$out/arch" || return 1
        local k=0 kb=0
        for a in "$out"/arch/*/; do
            local name; name=$(basename "$a")
            local r; r=$(FMI_ENGINE_BIN=$NEW nice -n 10 "$FMI" verify "$out/run-$name" "$a" 1 --lean \
                --env SPEEDRUN_CUTS=1 2> "$out/$name.err")
            local rc=$? d
            rm -rf "$out/run-$name"
            d=$(echo "$r" | grep -o '"differ": [0-9]*' | awk '{print $2}')
            k=$((k + 1))
            if [ $rc != 0 ] || [ "${d:-1}" != 0 ]; then kb=$((kb + 1)); echo "$s $name: DIFFERS rc $rc $r"; fi
            echo "$name $r" >> "$out/verify.txt"
        done
        say "b3 $s: $k new entries verified from boot (all six), $kb with differences"
        n=$((n + k)); bad=$((bad + kb))
    done
    say "b3: $n entries, $bad with differences"
    [ $bad = 0 ]
}

case "${1:-}" in
    g1|g2|g3|g4|g5|b3) "$1"; rc=$?; say "$1 exit $rc"; exit $rc ;;
    all) for g in g2 g3 g4 b3 g1 g5; do $g; say "$g exit $?"; done ;;
    *) sed -n '2,24p' "$0"; exit 2 ;;
esac
