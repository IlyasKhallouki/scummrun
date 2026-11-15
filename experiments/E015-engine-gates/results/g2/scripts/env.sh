# E015 G2: play8's environment (E014 a8/run-env.sh: ALLON and the game's settings) for the re-runner
source /var/tmp/fast-mi/wt-e014/experiments/E014-fate-demo/a8/run-env.sh >/dev/null 2>&1
for kv in $ALLON $GAME; do export "$kv"; done
export FMI_ENGINE_BIN=/var/tmp/fast-mi/svm-e013/build/scummvm/scummvm
export E15=/var/tmp/fast-mi/target-e015-g2/release/e015-g2
export ARCH=/var/tmp/fast-mi/e014-archives/play8
export OUT=/var/tmp/fast-mi/e015-out/g2
