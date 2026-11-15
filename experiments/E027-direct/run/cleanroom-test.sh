#!/bin/bash
# cleanroom-test.sh: the README's steps in a fresh clone of main, with nothing from this machine but the game files
# (the folders below are this laptop's: change the clone source and the game folders for another machine).
set -euo pipefail
R=/var/tmp/fast-mi/repro
rm -rf $R; mkdir -p $R; cd $R
t0=$(date +%s); step() { echo "== $(( $(date +%s) - t0 ))s  $*"; }

step "1. clone (submodules from GitHub)"
git clone -q --recurse-submodules --shallow-submodules /home/ilyask/projects/scummrun scummrun
cd scummrun
git log --oneline -1
mkdir -p game/dos-cd game/fate-play-demo
cp /home/ilyask/projects/scummrun/game/dos-cd/MONKEY.00? /home/ilyask/projects/scummrun/game/dos-cd/track*.flac game/dos-cd/
cp /var/tmp/fast-mi/games/fate-play-demo/PLAYFATE.00? game/fate-play-demo/
md5sum game/dos-cd/MONKEY.00? game/fate-play-demo/PLAYFATE.000

step "2. decompile the scripts"
GAME_DIR=game/dos-cd scripts/dump-scripts.sh > $R/dump-mi.log 2>&1 || { tail -20 $R/dump-mi.log; exit 1; }
GAME_DIR=game/fate-play-demo SCUMMRP_GAME=atlantis INDEX_NAME=PLAYFATE.000 RELEASE=fate-play-demo DUMP_ALLOW_PROBLEMS=1 scripts/dump-scripts.sh > $R/dump-demo.log 2>&1 || { tail -20 $R/dump-demo.log; exit 1; }
tail -2 $R/dump-mi.log; tail -2 $R/dump-demo.log
ls data

step "3a. the engine"
nice -n 5 experiments/E027-direct/run/engine.sh > $R/engine.log 2>&1 || { tail -20 $R/engine.log; exit 1; }
tail -1 $R/engine.log
step "3b. the search"
(cd experiments/E011-speed/rs && nice -n 5 cargo build -q --profile fast -p fmi-direct) > $R/cargo.log 2>&1 || { tail -20 $R/cargo.log; exit 1; }
ls -la experiments/E011-speed/rs/target/fast/direct

step "4. the gate"
nice -n 5 experiments/E027-direct/run/gate.sh || true

step "5. replay the ending"
source experiments/E027-direct/run/env.sh && mkdir -p out/verify-full5
env $ON $MI experiments/E011-speed/rs/target/fast/direct verify \
  experiments/E027-direct/results/full5/end-stop.jsonl 7895 out/verify-full5 bit:565=1 2>&1 | tail -1
step "done"
