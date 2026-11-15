#!/bin/bash
# E014 amendment (8): the run on the laptop if the VM cannot start: a8/run-env.sh (ALLON with the bound 300 s, grown
# scopes, FMI_READ_WIDEN, FMI_STAG_CELLS, amendment (8)'s switches, the wall limit, the game's settings) with b = 2 and
# 1 reading in the background, in the new
# archive play8; then every entry verified from boot (vm/run.sh verify) and the stall table (vm/run.sh stalls).
# usage: laptop.sh [init|run|verify|stalls]...   (default: all four in order; run resumes from the archive's files)
set -u
here=$(cd "$(dirname "$0")" && pwd)
export ENV=$here/run-env.sh A4_BG=1 W=2
export D_ARCH=/var/tmp/fast-mi/e014-archives/play8 D_RUN=/var/tmp/fast-mi/e014-archives/play8-run
export D_OUT=/var/tmp/fast-mi/e014-play8
R=$here/../vm/run.sh
mkdir -p $D_OUT
[ -e $D_OUT/guide-play.json ] || cp /var/tmp/fast-mi/e014-play-vm4/guide-play.json $D_OUT/
for st in ${@:-init run verify stalls}; do
  bash $R $st || { echo "$st failed"; exit 1; }
done
