#!/bin/bash
# E027's engine: svm-e024's tree with patch 0011 (the images' names), built beside it as svm-e027. Idempotent: a tree
# that holds the patch already is only built again. On the laptop and on the VM alike (both hold svm-e024).
#   engine.sh [FROM=/var/tmp/fast-mi/svm-e024] [TO=/var/tmp/fast-mi/svm-e027] [JOBS=nproc]
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); ROOT=$(cd "$HERE/../../.." && pwd)
FROM=${1:-/var/tmp/fast-mi/svm-e024}; TO=${2:-/var/tmp/fast-mi/svm-e027}; JOBS=${3:-$(nproc)}
[ -x "$FROM/build/scummvm/scummvm" ] || { echo "no engine at $FROM"; exit 2; }
[ -d "$TO" ] || cp -a "$FROM" "$TO"
cd "$TO/third_party/scummvm"
if ! grep -q SPEEDRUN_VERB_IMAGES engines/scumm/speedrun/speedrun_bridge.cpp; then
  patch -p1 < "$ROOT/patches/0011-scumm-speedrun-verb-images.patch"
fi
make -C "$TO/build/scummvm" -j"$JOBS" 2>&1 | tail -2
ls -la "$TO/build/scummvm/scummvm"
