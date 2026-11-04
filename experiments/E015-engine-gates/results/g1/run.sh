#!/usr/bin/env bash
# E015 G1 (plan.md, "G1"): the commands of the gate and of its reported runs (a)-(c). Build first:
#   cd experiments/E011-speed/rs && CARGO_TARGET_DIR=/var/tmp/fast-mi/target-e015-g1 cargo build --release -p fmi-checks --bin e015
# Usage: run.sh gate|a|b|check  (outputs under /var/tmp/fast-mi/e015-out/g1/<name>)
set -euo pipefail
export FMI_ENGINE_BIN=/var/tmp/fast-mi/svm-e013/build/scummvm/scummvm
export FMI_ROOT=${FMI_ROOT:-/var/tmp/fast-mi/wt-e015-g1}
# the game's settings (E014 a8/run-env.sh) and patch 0007's input log
export FMI_GAME_ID=atlantis FMI_GAME_DIR=/var/tmp/fast-mi/games/fate-play-demo
export FMI_GAME_SCRIPTS=/var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo
export SPEEDRUN_IDLE=handler SPEEDRUN_END_SITE=1 FMI_BOXES=1 FMI_OFFER_SCREEN=1 SPEEDRUN_INPUT=1
export FMI_REALIZE=1
BIN=${E015_BIN:-/var/tmp/fast-mi/target-e015-g1/release/e015}
ARCH=/var/tmp/fast-mi/e014-archives/play8
OUT=/var/tmp/fast-mi/e015-out/g1
case "${1:-gate}" in
  # the gate: from entry 0, the opening's dialog verb 120, sentence(10,596) with its approach, dialog verb 120; run
  # on until room 63 or 600 frames; the chain replayed from boot
  gate) nice -n 10 "$BIN" realize "$OUT/gate" "$ARCH" 0 --steps d120,s10:596,d120 --room 63 --run-on 600 --verify ;;
  # (a) sentence(10,596) from entry 184 (596 on the screen)
  a) nice -n 10 "$BIN" realize "$OUT/a" "$ARCH" 184 --steps s10:596 ;;
  # (b) sentence(11,593) from entry 0 (593 cannot be clicked on this path): expected a bounded miss
  b) nice -n 10 "$BIN" realize "$OUT/b" "$ARCH" 0 --steps s11:593 ;;
  # the check with another build: E014's probe replays the gate's events from boot (the room per frame)
  check) nice -n 10 /var/tmp/fast-mi/target-e014e/checks/release/probe trace "$OUT/probe-trace" 1300 --events "$OUT/gate/events.jsonl" ;;
  *) echo "usage: run.sh gate|a|b|check" >&2; exit 2 ;;
esac
