#!/bin/bash
# FAST_MI_SCUMMVM_BIN wrapper for the Python reference (G1, G2, G3, G5): runs the E011 build with the lean switches set
# in the engine's own environment (speedrun.engine.build_env drops the caller's SPEEDRUN_* variables, not these).
#   LEAN=all (default): SPEEDRUN_LEAN=1; LEAN=none: every switch off; or a comma list of splash,present,events,audio
#   (SPEEDRUN_NO_SPLASH, SPEEDRUN_NO_PRESENT, SPEEDRUN_NO_EVENTS, SPEEDRUN_SILENT_AUDIO), each alone or together.
#   E011_ENGINE_BIN: the build (default /var/tmp/fast-mi/svm-e011/build/scummvm/scummvm).
BIN=${E011_ENGINE_BIN:-/var/tmp/fast-mi/svm-e011/build/scummvm/scummvm}
case "${LEAN:-all}" in
  all) export SPEEDRUN_LEAN=1 ;;
  none) ;;
  *) IFS=, read -ra SW <<< "$LEAN"
     for s in "${SW[@]}"; do
       case "$s" in
         splash) export SPEEDRUN_NO_SPLASH=1 ;;
         present) export SPEEDRUN_NO_PRESENT=1 ;;
         events) export SPEEDRUN_NO_EVENTS=1 ;;
         audio) export SPEEDRUN_SILENT_AUDIO=1 ;;
         *) echo "svm-lean.sh: unknown switch $s" >&2; exit 2 ;;
       esac
     done ;;
esac
exec "$BIN" "$@"
