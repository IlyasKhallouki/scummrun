#!/usr/bin/env bash
# E003 steps 4 and 5 on the cloud VM: the Part I attempt from seed 1 on the part1 scope (reader-scope --part1: the
# rooms of the recorded Part I moves, 31 rooms), with the Rust reader's ELSEWHERE switch on
# (scumm/reader/PORT-NOTES.md). Run each step in the VM's repo copy, in this order:
#
#   python3 scripts/cloud.py up --job partI          (from the partI worktree: its code, the patch, the game files)
#   python3 scripts/cloud.py sh --job partI -- bash experiments/E003-path-through-cloud/vm_part1.sh setup
#   python3 scripts/cloud.py sh --job partI -- bash experiments/E003-path-through-cloud/vm_part1.sh read
#   python3 scripts/cloud.py sh --job partI -- bash experiments/E003-path-through-cloud/vm_part1.sh run
#   python3 scripts/cloud.py down --job partI out/partI/results
#
# Steps:
#   setup    system packages, Rust and uv when missing, the Python environment; ScummVM and Fast Downward cloned
#            at the repository's pins when missing (ScummVM's tag with patches/*.patch applied once, on the fresh
#            clone); then the builds: the Rust reader and taskcore, Fast Downward (./build.py release), ScummVM
#            (scripts/build-scummvm.sh --no-reset). Ends with the free disk, memory and /dev/shm.
#   read     reader-scope $SCOPE --elsewhere [--live-locals] [--split-limit] on $THREADS threads: the packed file and
#            golden.py's records streamed into $READ, then packed_reading.py from-golden on $THREADS processes:
#            $READ/reading.json, reading.tc, reading.actions.jsonl. The reader's report and log go to $RES.
#   check    packed_reading.py check on the reading (optional: about 3.5 ms of CPU per action, so hours at Part I's
#            scale).
#   run      replay_part1.py run 1 on the reading: boot, the start check, the harness (taskcore and lama-first per
#            plan, the engine, bans and replans), verify from boot. The record and its log go to $RES.
#   control  run 1 --control: the same run on the control's reading, which replay_part1.py makes next to the
#            reading the first time.
#
# Settings (environment, with their defaults): SCOPE="--part1"; NAME=part1 (the reading's name, which the run's
# records carry: replay-part1-NAME-seed1.json); THREADS=all CPUs; READ=/dev/shm/partI (the reading
# lives in RAM: the packed file is about 1.8 KB a record and the reader's last rewrite of it needs twice that for a
# moment, more than the VM's 30 GB disk holds after the builds); RES=out/partI/results; CAP_MIN=240 (the reader's
# own time cap); MAX_RSS_GB=200; MIN_FREE_GB=8 (MemAvailable); MAX_FILES_GB=100 (the packed and golden files
# together); REPLANS=500;
# TIME_LIMIT=10800 (the run's seconds); PLAN_TIME=600 (plan.md step 4); PLAN_MEMORY=64G (Fast Downward per plan);
# DEADLINE (UTC epoch seconds, optional: the harness's time limit ends there at the latest); LIVE_LOCALS=0 (1: the
# reader's switch --live-locals, scumm/reader/PORT-NOTES.md "Live locals": events keyed only on the locals their unit
# reads; reading.json and the run's record then say so); SPLIT_LIMIT=0 (1: the reader's switch --split-limit,
# scumm/reader/PORT-NOTES.md "The path limit": a run past the path limit is split until no part passes it, and the
# reading stops on a run it cannot split).
# Only seed 1: reader-scope reads results/start-part1.jsonl's seed 1, and a reading serves the seed it was read from.
#
# C2: this file names no game number. The reading and the results stay on the VM's private disk and RAM until
# `down`, which copies only $RES (JSON records and logs; replay_part1.py's records hold no game text).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
SCOPE="${SCOPE:---part1}"
NAME="${NAME:-part1}"
THREADS="${THREADS:-$(nproc)}"
READ="${READ:-/dev/shm/partI}"
RES="${RES:-$ROOT/out/partI/results}"
CAP_MIN="${CAP_MIN:-240}"
MAX_RSS_GB="${MAX_RSS_GB:-200}"
MAX_FILES_GB="${MAX_FILES_GB:-100}"
MIN_FREE_GB="${MIN_FREE_GB:-8}"
REPLANS="${REPLANS:-500}"
TIME_LIMIT="${TIME_LIMIT:-10800}"
PLAN_TIME="${PLAN_TIME:-600}"
PLAN_MEMORY="${PLAN_MEMORY:-64G}"
LIVE_LOCALS="${LIVE_LOCALS:-0}"
SPLIT_LIMIT="${SPLIT_LIMIT:-0}"
PY="$ROOT/.venv/bin/python"
SCUMMVM_TAG=v2026.3.0  # scripts/build-scummvm.sh's pin
DOWNWARD_PIN=7ea2755269ff9398279b30f5c848e32ba6047c9c  # the submodule's commit (release 26.6.0)
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

say() { echo "[$(date -u +%H:%M:%S)] $*"; }

setup() {
    say "system packages"
    sudo apt-get update -q
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -yq build-essential cmake pkg-config git curl rsync \
        python3 libsdl2-dev zlib1g-dev
    if ! command -v cargo >/dev/null; then
        say "Rust (rustup, minimal profile)"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    fi
    if ! command -v uv >/dev/null; then
        say "uv"
        curl -LsSf https://astral.sh/uv/install.sh | sh
    fi
    say "the Python environment"
    (cd "$ROOT" && uv sync)
    if [[ ! -e "$ROOT/third_party/scummvm/.git" ]]; then
        say "ScummVM $SCUMMVM_TAG, with the bridge patch"
        mkdir -p "$ROOT/third_party"
        git clone -q --depth 1 --branch "$SCUMMVM_TAG" https://github.com/scummvm/scummvm.git \
            "$ROOT/third_party/scummvm"
        for p in "$ROOT"/patches/*.patch; do git -C "$ROOT/third_party/scummvm" apply --whitespace=nowarn "$p"; done
    fi
    if [[ ! -e "$ROOT/third_party/downward/build.py" ]]; then
        say "Fast Downward $DOWNWARD_PIN"
        mkdir -p "$ROOT/third_party/downward"
        git -C "$ROOT/third_party/downward" init -q
        git -C "$ROOT/third_party/downward" fetch -q --depth 1 https://github.com/aibasel/downward.git "$DOWNWARD_PIN"
        git -C "$ROOT/third_party/downward" checkout -q FETCH_HEAD
    fi
    say "the Rust reader and taskcore"
    (cd "$HERE/scumm" && cargo build --release -p scumm-reader --bin reader-scope)
    (cd "$HERE/taskcore" && cargo build --release)
    say "Fast Downward"
    (cd "$ROOT/third_party/downward" && python3 build.py release -j "$THREADS")
    say "ScummVM (--no-reset: the tree as uploaded)"
    "$ROOT/scripts/build-scummvm.sh" --no-reset -j "$THREADS"
    df -h "$ROOT" /dev/shm
    free -g
}

read_scope() {
    mkdir -p "$READ" "$RES"
    local live=()
    [[ "$LIVE_LOCALS" == 1 ]] && live=(--live-locals)
    [[ "$SPLIT_LIMIT" == 1 ]] && live+=(--split-limit)
    say "reader-scope $SCOPE --elsewhere ${live[*]}, $THREADS threads, into $READ"
    (cd "$HERE" && scumm/target/release/reader-scope $SCOPE --elsewhere "${live[@]}" --threads "$THREADS" --name "$NAME" \
        --out "$READ" --packed "$READ/reading.tc" --golden "$READ/reading.golden.jsonl.gz" --cap-min "$CAP_MIN" \
        --max-rss-gb "$MAX_RSS_GB" --max-files-gb "$MAX_FILES_GB" --min-free-gb "$MIN_FREE_GB") 2>&1 | tee "$RES/reader-scope.log"
    cp "$READ/$NAME.json" "$RES/reader-scope-$NAME.json"
    say "packed_reading.py from-golden"
    (cd "$HERE" && "$PY" packed_reading.py from-golden "$READ/reading.golden.jsonl.gz" "$READ/reading.tc" \
        --start-seed 1 --name reading --out "$READ" --workers "$THREADS") 2>&1 | tee "$RES/from-golden.log"
    df -h "$ROOT" /dev/shm
}

check() {
    (cd "$HERE" && "$PY" packed_reading.py check "$READ/reading.json") > "$RES/reading-check.json"
}

run() {
    mkdir -p "$RES"
    (cd "$HERE" && "$PY" replay_part1.py run 1 --reading "$READ/reading.json" --results "$RES" \
        --replan-limit "$REPLANS" --time-limit "$TIME_LIMIT" --plan-time "$PLAN_TIME" --plan-memory "$PLAN_MEMORY" \
        ${DEADLINE:+--deadline "$DEADLINE"}) 2>&1 | tee "$RES/run-seed1.log"
}

control() {
    mkdir -p "$RES"
    (cd "$HERE" && "$PY" replay_part1.py run 1 --reading "$READ/reading.json" --control --results "$RES" \
        --replan-limit "$REPLANS" --time-limit "$TIME_LIMIT" --plan-time "$PLAN_TIME" --plan-memory "$PLAN_MEMORY" \
        ${DEADLINE:+--deadline "$DEADLINE"}) 2>&1 | tee "$RES/run-seed1-control.log"
}

case "${1:-}" in
    setup) setup ;;
    read) read_scope ;;
    check) check ;;
    run) run ;;
    control) control ;;
    *) sed -n '2,39p' "${BASH_SOURCE[0]}"; exit 2 ;;
esac
