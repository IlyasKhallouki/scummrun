#!/bin/bash
# The engine, plus SPEEDRUN_VIDEO and the frame log in the standalone launch only (no control channel)
if [ -z "${SPEEDRUN_CONTROL:-}" ] && [ -n "${FMI_CAPTURE:-}" ]; then
  unset SPEEDRUN_CUTS SPEEDRUN_COUNT_MIXER SPEEDRUN_NO_PIXELS
  # FMI_CAPTURE_SOUND=1: the lean switches but the silent sound chip (SPEEDRUN_LEAN=1 sets all four; no game state
  # reads what the chip puts out, and the launch's digests are checked all the same)
  if [ -n "${FMI_CAPTURE_SOUND:-}" ] && [ "${SPEEDRUN_LEAN:-}" = 1 ]; then
    unset SPEEDRUN_LEAN
    export SPEEDRUN_NO_SPLASH=1 SPEEDRUN_NO_PRESENT=1 SPEEDRUN_NO_EVENTS=1
  fi
  export SPEEDRUN_VIDEO="$FMI_CAPTURE" SPEEDRUN_FRAME_LOG=1
  env | grep '^SPEEDRUN_' | sort > "$FMI_CAPTURE.env"
fi
exec "${FMI_CAPTURE_ENGINE:-/var/tmp/fast-mi/svm-e024/build/scummvm/scummvm}" "$@"
