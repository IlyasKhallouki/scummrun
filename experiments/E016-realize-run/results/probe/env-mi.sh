# E016 A: Monkey Island's settings for e016 (E013's engine, E014's mechanism switches the bridge reads, the input log,
# the walk boxes). FMI_ROOT is this worktree, whose game/ links the repository's game files (never committed).
export FMI_ENGINE_BIN=/var/tmp/fast-mi/svm-e013/build/scummvm/scummvm
export FMI_ROOT=${FMI_ROOT:-/var/tmp/fast-mi/wt-e016-a}
export SPEEDRUN_INPUT=1 SPEEDRUN_IDLE=handler SPEEDRUN_END_SITE=1 SPEEDRUN_REPORT_RESTART=1 FMI_BOXES=1 FMI_OFFER_SCREEN=1
unset FMI_GAME_ID FMI_GAME_DIR FMI_GAME_SCRIPTS
export E016_BIN=${E016_BIN:-/var/tmp/fast-mi/target-e016-a/release/e016}
