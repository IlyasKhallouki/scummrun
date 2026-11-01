#!/usr/bin/env bash
# E004's rough reading (rough-reading.md) on the cloud VM: the Part I reading with every job read to N steps, its
# plan, and the check scopes against their full readings. From the rough worktree:
#
#   python3 scripts/cloud.py up --job rough
#   python3 scripts/cloud.py sh --job rough -- bash experiments/E004-fast-exact/vm_rough.sh build
#   python3 scripts/cloud.py sh --job rough -- bash experiments/E004-fast-exact/vm_rough.sh read N TAG [SWITCHES]
#   python3 scripts/cloud.py sh --job rough -- bash experiments/E004-fast-exact/vm_rough.sh plan TAG
#   python3 scripts/cloud.py sh --job rough -- bash experiments/E004-fast-exact/vm_rough.sh check N TAG [SWITCHES]
#   python3 scripts/cloud.py sh --job rough -- bash experiments/E004-fast-exact/vm_rough.sh status
#   python3 scripts/cloud.py down --job rough rough-out
#
# Steps:
#   build    the Rust reader and taskcore (the VM's earlier setup has Rust, uv, the Python environment and Fast
#            Downward; vm_part1.sh setup makes them)
#   read     reader-scope --part1 --reach --elsewhere --live-locals --split-limit --switches SWITCHES --step-limit N
#            (N 0: no --step-limit), seed 1, READER_TRACE=1, detached: $OUT/TAG/ holds the report, the packed file,
#            the trace without its job-start lines, RSS every 10 s and the exit code (steps.txt)
#   plan     rough_plan.py on $OUT/TAG/part1.tc with --key (evaluation only): $OUT/TAG/plan/
#   check    the check scopes (two rooms, two others, k = 1, k = 2) read with --goal state:316=1 at N and without a
#            step limit, and rough_contains.py on each pair: $OUT/TAG/
#   status   the runs' exit codes and their last RSS lines
#
# Settings: OUT=$HOME/fast-mi/rough-out; THREADS=all CPUs; CAP_MIN=120; SWITCHES (default run D's and havoc_cut,
# havoc_starts). The check scopes are the five check scopes but the kitchen, by reader-scope's options.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
E003="$ROOT/experiments/E003-path-through-cloud"
OUT="${OUT:-$ROOT/rough-out}"
THREADS="${THREADS:-$(nproc)}"
CAP_MIN="${CAP_MIN:-120}"
DEFAULT_SW=value_sets,draw_clears,actor_rooms,grow_limit,havoc_cut,havoc_starts
PY="$ROOT/.venv/bin/python"
RS="$E003/scumm/target/release/reader-scope"
TC="$E003/taskcore/target/release/taskcore"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

say() { echo "[$(date -u +%H:%M:%S)] $*"; }

build() {
    (cd "$E003/scumm" && cargo build --release -p scumm-reader --bin reader-scope --bin reader-port)
    (cd "$E003/taskcore" && cargo build --release)
    ls -la "$RS" "$TC" "$ROOT/third_party/downward/builds/release/bin/downward"
    git -C "$ROOT" log -1 --format=%h 2>/dev/null || true
    nproc; free -g; df -h "$ROOT"
}

# one reading, detached: $1 the directory, then reader-scope's options
detached() {
    local dir="$1"; shift
    mkdir -p "$dir"
    rm -f "$dir/steps.txt"
    setsid nohup bash -c '
        dir="$1"; shift
        cd "'"$E003"'"
        SECONDS=0
        READER_TRACE=1 "$@" 2> >(grep --line-buffered -v "^start " > "$dir/trace.log") &
        pid=$!
        while kill -0 $pid 2>/dev/null; do
            echo "$SECONDS $(awk "/VmRSS/{print \$2}" /proc/$pid/status 2>/dev/null) $(awk "/MemAvailable/{print \$2}" /proc/meminfo) $(cut -d" " -f1 /proc/loadavg)" >> "$dir/rss.tsv"
            sleep 10
        done
        wait $pid; rc=$?
        echo "rc $rc $SECONDS s" >> "$dir/steps.txt"
    ' _ "$dir" "$@" > "$dir/nohup.out" 2>&1 < /dev/null &
    say "started in $dir"
}

limit() { [[ "$1" == 0 ]] && echo "" || echo "--step-limit $1"; }

read_part1() {
    local n="$1" tag="$2" sw="${3:-$DEFAULT_SW}"
    # shellcheck disable=SC2046
    detached "$OUT/$tag" "$RS" --part1 --name part1 --threads "$THREADS" --out "$OUT/$tag" --reach --elsewhere \
        --live-locals --split-limit --switches "$sw" $(limit "$n") --cap-min "$CAP_MIN" --min-free-gb 8 \
        --max-rss-gb 200 --max-files-gb 40 --hash
}

plan() {
    local tag="$1"
    (cd "$ROOT" && "$PY" -I "$HERE/rough_plan.py" "$OUT/$tag/part1.tc" "$OUT/$tag/plan" --taskcore "$TC" --key \
        --memory 64G --time 1800) 2>&1 | tee "$OUT/$tag/plan.log"
}

check() {
    local n="$1" tag="$2" sw="${3:-$DEFAULT_SW}"
    mkdir -p "$OUT/$tag"
    for s in "bar:--rooms 33,28" "jail:--rooms 31,33" "k1:--k 1" "k2:--k 2"; do
        local name="${s%%:*}" opt="${s#*:}"
        for side in full rough; do
            local l=""
            [[ "$side" == rough ]] && l="$(limit "$n")"
            # shellcheck disable=SC2086
            (cd "$E003" && SECONDS=0 && READER_TRACE=1 "$RS" $opt --name "$name" --threads "$THREADS" \
                --out "$OUT/$tag/$side" --goal state:316=1 --hash --reach --elsewhere --live-locals --split-limit \
                --switches "$sw" $l --cap-min "$CAP_MIN" --min-free-gb 8 2> >(grep -v "^start " > "$OUT/$tag/$side-$name.trace") \
                && echo "$side $name rc 0 ${SECONDS}s" >> "$OUT/$tag/steps.txt" || echo "$side $name rc $? ${SECONDS}s" >> "$OUT/$tag/steps.txt")
        done
        python3 "$HERE/rough_contains.py" "$OUT/$tag/full/$name.reached" "$OUT/$tag/rough/$name.reached" --json \
            > "$OUT/$tag/contains-$name.json" || true
    done
    cat "$OUT/$tag/steps.txt" "$OUT/$tag"/contains-*.json
}

status() {
    for d in "$OUT"/*/; do
        echo "$d: $(cat "$d/steps.txt" 2>/dev/null | tr '\n' ' ') $(tail -1 "$d/rss.tsv" 2>/dev/null)"
    done
    free -g | head -2
}

case "${1:-}" in
    build) build ;;
    read) shift; read_part1 "$@" ;;
    plan) shift; plan "$@" ;;
    check) shift; check "$@" ;;
    status) status ;;
    *) sed -n '2,26p' "${BASH_SOURCE[0]}"; exit 2 ;;
esac
