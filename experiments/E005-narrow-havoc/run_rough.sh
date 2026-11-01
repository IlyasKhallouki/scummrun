#!/usr/bin/env bash
# E005's readings on the laptop (plan.md, "Machine"): nice 10, 3 threads, each under /var/tmp/fast-mi/heavy.lock, and
# stopped when MemAvailable falls under 3 GB. Each run is detached: OUT/TAG/ holds the report, the packed file, the
# trace without its job-start lines, RSS every 10 s and the exit code (steps.txt).
#
#   run_rough.sh part1 N TAG       Part I (reader-scope --part1, goal results/end-part1.json), rough at N
#   run_rough.sh bar N TAG         rooms 33 and 28, goal state:316=1 (E004's check scope)
#   run_rough.sh kitchen N TAG     the kitchen (reader-port read kitchen, its own goal and start)
#   run_rough.sh status
#
# N 0 is the full reading (no --step-limit). Switches: run D's, havoc_cut, havoc_starts, reach_pairs, and E005's
# havoc_rooms and havoc_args (SWITCHES overrides), with --reach --elsewhere --live-locals --split-limit, seed 1.
# reader-scope's own time cap is set out of the way: a run ends when it is done or when memory runs short.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
E003="$ROOT/experiments/E003-path-through-cloud"
OUT="${OUT:-$HOME/.cache/fast-mi-e005}"
THREADS="${THREADS:-3}"
SW="${SWITCHES:-value_sets,draw_clears,actor_rooms,grow_limit,havoc_cut,havoc_starts,reach_pairs,havoc_rooms,havoc_args}"
RS="$E003/scumm/target/release/reader-scope"
RP="$E003/scumm/target/release/reader-port"
LOCK=/var/tmp/fast-mi/heavy.lock
# the machine's facts: 23 GB in all; a run stops when MemAvailable falls under 3 GB
MIN_FREE_KB=$((3 * 1024 * 1024))

say() { echo "[$(date +%H:%M:%S)] $*"; }

# one reading, detached, under the lock: $1 the directory, then the command
detached() {
    local dir="$1"; shift
    mkdir -p "$dir"
    rm -f "$dir/steps.txt" "$dir/rss.tsv"
    git -C "$ROOT" log -1 --format=%h > "$dir/commit.txt"
    printf '%q ' "$@" > "$dir/cmd.txt"
    setsid nohup flock "$LOCK" bash -c '
        dir="$1"; min="$2"; shift 2
        cd "'"$E003"'"
        echo "lock taken $(date +%H:%M:%S)" >> "$dir/steps.txt"
        SECONDS=0
        READER_TRACE=1 nice -n 10 "$@" > "$dir/stdout.log" 2> >(grep --line-buffered -v "^start " > "$dir/trace.log") &
        pid=$!
        while kill -0 $pid 2>/dev/null; do
            avail=$(awk "/MemAvailable/{print \$2}" /proc/meminfo)
            echo "$SECONDS $(awk "/VmRSS/{print \$2}" /proc/$pid/status 2>/dev/null) $avail $(cut -d" " -f1 /proc/loadavg) $(awk "{print \$14+\$15}" /proc/$pid/stat 2>/dev/null)" >> "$dir/rss.tsv"
            if [ "$avail" -lt "$min" ]; then
                echo "stopped: MemAvailable $avail kB under $min kB at $SECONDS s" >> "$dir/steps.txt"
                kill -TERM $pid 2>/dev/null || true
                break
            fi
            sleep 10
        done
        wait $pid; rc=$?
        echo "rc $rc $SECONDS s" >> "$dir/steps.txt"
    ' _ "$dir" "$MIN_FREE_KB" "$@" > "$dir/nohup.out" 2>&1 < /dev/null &
    say "queued in $dir"
}

limit() { [[ "$1" == 0 ]] && echo "" || echo "--step-limit $1"; }

common() {
    echo --threads "$THREADS" --reach --elsewhere --live-locals --split-limit --switches "$SW" \
        --cap-min 100000 --min-free-gb 3 --max-rss-gb 20 --hash
}

case "${1:-}" in
    part1)
        n="$2"; tag="$3"
        # shellcheck disable=SC2046
        detached "$OUT/$tag" "$RS" --part1 --name part1 --out "$OUT/$tag" $(common) $(limit "$n")
        ;;
    bar)
        n="$2"; tag="$3"
        # shellcheck disable=SC2046
        detached "$OUT/$tag" "$RS" --rooms 33,28 --name bar --goal state:316=1 --out "$OUT/$tag" $(common) $(limit "$n")
        ;;
    kitchen)
        n="$2"; tag="$3"
        sw="$SW,split_limit,reach"
        [[ "$n" != 0 ]] && sw="$sw,step_limit=$n"
        detached "$OUT/$tag" "$RP" read kitchen --threads "$THREADS" --switches "$sw" --live-locals --elsewhere \
            --packed "$OUT/$tag/kitchen.tc" --no-keep
        ;;
    status)
        for d in "$OUT"/*/; do
            [ -f "$d/steps.txt" ] || continue
            echo "$d: $(tr '\n' ' ' < "$d/steps.txt") | $(tail -1 "$d/rss.tsv" 2>/dev/null)"
        done
        free -g | head -2
        ;;
    *) sed -n '2,15p' "${BASH_SOURCE[0]}"; exit 2 ;;
esac
