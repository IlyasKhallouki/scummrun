# E014, amendment (2): the interactive demo. demo/env.sh's tools, build, engine and switches, pointed at this game.
# Source it as ENV for demo/run.sh (STAGE=play names the stage and the guide).
export D_ARCH=${D_ARCH:-/var/tmp/fast-mi/e014-archives/play}
export D_RUN=${D_RUN:-/var/tmp/fast-mi/e014-archives/play-run}
export D_OUT=${D_OUT:-/var/tmp/fast-mi/e014-play}
source /var/tmp/fast-mi/wt-e014/experiments/E014-fate-demo/demo/env.sh
# the game: its files (read only) and its extraction (scripts/dump-scripts.sh with the amendment's settings, never
# committed)
export E14_GAME_DIR=/var/tmp/fast-mi/games/fate-play-demo
export E14_SCRIPTS=/var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo
# the switches added on this game, one per break (results/assumptions.md)
E14P="FMI_EGO_VALUES=1 FMI_MERGE_DEDUP=1 FMI_READ_CUT=1"
E14ON="FMI_GAME_ID=atlantis FMI_GAME_DIR=$E14_GAME_DIR FMI_GAME_SCRIPTS=$E14_SCRIPTS SPEEDRUN_IDLE=handler SPEEDRUN_END_SITE=1 FMI_GOAL_END=1 FMI_READ_EVENTS=1 $E14P"
export E14ON E14P
export STAGE=play
