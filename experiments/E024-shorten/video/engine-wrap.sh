#!/bin/bash
# The engine, plus SPEEDRUN_VIDEO and the frame log in the standalone launch only (no control channel)
if [ -z "${SPEEDRUN_CONTROL:-}" ] && [ -n "${FMI_CAPTURE:-}" ]; then
  unset SPEEDRUN_CUTS SPEEDRUN_COUNT_MIXER SPEEDRUN_NO_PIXELS
  export SPEEDRUN_VIDEO="$FMI_CAPTURE" SPEEDRUN_FRAME_LOG=1
  env | grep '^SPEEDRUN_' | sort > "$FMI_CAPTURE.env"
fi
exec "${FMI_CAPTURE_ENGINE:-/var/tmp/fast-mi/svm-e024/build/scummvm/scummvm}" "$@"
