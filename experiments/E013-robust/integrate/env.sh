# E013 integration: E012's integrate/env.sh (round 2's tools, ON3, E12ON) with e013's build (E13T, integrate/build.sh)
# and worktree, and E013's engine (integrate/engine.sh: patches 0001-0005, 0006-e014 and 0007). Source it as ENV.
#   E14M / E14MOFF   E014's mechanism switches of amendment (2) (b) on / unset (the game's settings stay unset: MI)
#   E13ON / E13OFF   E013's switches on / unset (refuse's check and dedupe switches unset with them)
#   ALLON            amendment (2) (b)'s one switch set: ON3, E12ON, E14M and E13ON
#   ALLOFF           every E012, E013 and E014 switch unset, with round 2's switches on (ON3), as E012's G0 ran
export E13T=${E13T:-/var/tmp/fast-mi/target-e013}
export E12T=$E13T
source /var/tmp/fast-mi/wt-e013/experiments/E012-game-mechanics/integrate/env.sh
export WT11=/var/tmp/fast-mi/wt-e013
export E13_ENGINE_BIN=${E13_ENGINE_BIN:-/var/tmp/fast-mi/svm-e013/build/scummvm/scummvm}
export FMI_ENGINE_BIN=$E13_ENGINE_BIN
E14P="FMI_EGO_VALUES=1 FMI_MERGE_DEDUP=1 FMI_READ_CUT=1 FMI_STAG_SWEEP=1 FMI_VERBS_VAR_ON=1 FMI_NOVEL_VERBS=1 FMI_NOVEL_GUIDE=1"
E14M="SPEEDRUN_IDLE=handler SPEEDRUN_END_SITE=1 FMI_GOAL_END=1 FMI_READ_EVENTS=1 $E14P"
E14MOFF="SPEEDRUN_IDLE= SPEEDRUN_END_SITE= FMI_GOAL_END= FMI_READ_EVENTS= FMI_EGO_VALUES= FMI_MERGE_DEDUP= FMI_READ_CUT= FMI_STAG_SWEEP= FMI_VERBS_VAR_ON= FMI_NOVEL_VERBS= FMI_NOVEL_GUIDE= FMI_GAME_ID= FMI_GAME_DIR= FMI_GAME_SCRIPTS="
E13ON="SPEEDRUN_INPUT=1 FMI_REFUSE=1 FMI_STALLS=1 FMI_FREE=0.25 FMI_FREE_SHIFT=1"
E13OFF="SPEEDRUN_INPUT= FMI_REFUSE= FMI_REFUSE_CHECK= FMI_REFUSE_DEDUPE= FMI_STALLS= FMI_FREE= FMI_FREE_SHIFT="
ALLON="$ON3 $E12ON $E14M $E13ON"
ALLOFF="$ON3 $E12OFF $E14MOFF $E13OFF"
export E14P E14M E14MOFF E13ON E13OFF ALLON ALLOFF
# E012's scripts (integrate/gates.sh's stages, b3.sh, kitchen/verify*.sh) take the engine from E12_ENGINE_BIN: E013's
export E12_ENGINE_BIN=$E13_ENGINE_BIN
