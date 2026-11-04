#!/bin/bash
# E013 integration's engine: a fresh tree /var/tmp/fast-mi/svm-e013 from the tag and the committed patches 0001-0005,
# 0006-e014 and 0007 (engine/build.sh --fresh: one commit per patch, 0007 applied on top; no tree is reset), built with
# E011's slim configure line. Run under the heavy lock and the guard. usage: engine.sh [DIR]
set -e
here=$(cd "$(dirname "$0")" && pwd)
exec nice -n 10 $here/../engine/build.sh --fresh ${1:-/var/tmp/fast-mi/svm-e013} -j 3
