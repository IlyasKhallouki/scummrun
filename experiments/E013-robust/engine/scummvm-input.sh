#!/bin/bash
# E013 (patch 0007): the engine with the input log on. E011's gate scripts (engine/check-r2.sh, through svm-lean.sh
# and fmi-engine) set the engine's environment themselves; pointed at this file (E011_ENGINE_BIN), every engine they
# start also has SPEEDRUN_INPUT=1. E13_ENGINE_BIN: the build (default refuse's).
export SPEEDRUN_INPUT=1
exec "${E13_ENGINE_BIN:-/home/ilyask/.cache/fast-mi-e013/svm-e013-refuse/build/scummvm/scummvm}" "$@"
