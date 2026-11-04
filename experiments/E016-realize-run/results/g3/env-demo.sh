# E016 A: the demo's settings for e016 (E014 a8/run-env.sh's game; E013's engine; E014's mechanism switches the bridge
# reads; the input log; the walk boxes; FMI_REALIZE=1)
export FMI_ENGINE_BIN=/var/tmp/fast-mi/svm-e013/build/scummvm/scummvm
export FMI_ROOT=${FMI_ROOT:-/var/tmp/fast-mi/wt-e016-a}
export FMI_GAME_ID=atlantis FMI_GAME_DIR=/var/tmp/fast-mi/games/fate-play-demo
export FMI_GAME_SCRIPTS=/var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo
export SPEEDRUN_IDLE=handler SPEEDRUN_END_SITE=1 FMI_BOXES=1 FMI_OFFER_SCREEN=1 SPEEDRUN_INPUT=1
export FMI_REALIZE=1
export E016_BIN=${E016_BIN:-/var/tmp/fast-mi/target-e016-a/release/e016}
