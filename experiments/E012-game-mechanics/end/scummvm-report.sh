#!/bin/bash
# E012 component end: patch 0005's build with its switch on. E011's gate scripts (engine/check-r2.sh, through
# svm-lean.sh and fmi-engine) set the engine's environment themselves; pointed at this file (E011_ENGINE_BIN), every
# engine they start also has SPEEDRUN_REPORT_RESTART=1. END_ENGINE_BIN: the build (default the end component's).
export SPEEDRUN_REPORT_RESTART=1
exec "${END_ENGINE_BIN:-/var/tmp/fast-mi/svm-e012-end/build/scummvm/scummvm}" "$@"
