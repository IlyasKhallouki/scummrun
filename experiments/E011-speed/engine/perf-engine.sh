#!/bin/bash
# FMI_ENGINE_BIN wrapper for round 2's bench (bench-r2.sh): runs the engine build PERF_REAL under the profile's perf
# (~/.cache/fast-mi-e011/profile-engine/tools, user space only) and writes PERF_OUT/stat-<ns>.txt (PERF_MODE=stat:
# cycles:u, instructions:u, task-clock) or PERF_OUT/perf-<ns>.data (PERF_MODE=record: cycles:u every 10,000, no call
# graph). Forked children are counted with their parent (perf's inherit).
T=$HOME/.cache/fast-mi-e011/profile-engine/tools/root
REAL=${PERF_REAL:?PERF_REAL: the engine build}
export LD_LIBRARY_PATH=$T/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
mkdir -p "$PERF_OUT"
n=$(date +%s%N)
case "${PERF_MODE:-stat}" in
  stat) exec "$T/usr/bin/perf" stat -x, -e cycles:u,instructions:u,task-clock -o "$PERF_OUT/stat-$n.txt" -- "$REAL" "$@" ;;
  record) exec "$T/usr/bin/perf" record -q -e cycles:u -c 10000 -o "$PERF_OUT/perf-$n.data" -- "$REAL" "$@" ;;
  *) echo "perf-engine.sh: PERF_MODE stat or record" >&2; exit 2 ;;
esac
