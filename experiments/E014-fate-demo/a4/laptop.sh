#!/bin/bash
# E014 amendment (5): the run on the laptop, as the VM could not start: amendment (4)'s run (a4/run-env.sh: ALLON with
# the bound 300 s and grown scopes, the game's settings) with b = 2 and 1 reading in the background, in the new archive
# play4; then every entry verified from boot (vm/run.sh verify) and the stall table (vm/run.sh stalls).
# usage: laptop.sh [init|run|verify|stalls]...   (default: all four in order; run resumes from the archive's files)
set -u
here=$(cd "$(dirname "$0")" && pwd)
export ENV=$here/run-env.sh A4_BG=1 W=2
export D_ARCH=/var/tmp/fast-mi/e014-archives/play4 D_RUN=/var/tmp/fast-mi/e014-archives/play4-run
export D_OUT=/var/tmp/fast-mi/e014-play4
R=$here/../vm/run.sh
mkdir -p $D_OUT
[ -e $D_OUT/guide-play.json ] || cp /var/tmp/fast-mi/e014-play-vm4/guide-play.json $D_OUT/
for st in ${@:-init run verify stalls}; do
  bash $R $st || { echo "$st failed"; exit 1; }
done
