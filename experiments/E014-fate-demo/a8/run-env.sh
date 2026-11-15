# E014 amendment (8): the run's environment on the laptop and the VM: a8/env.sh (target-e014e, E013's engine, ALLON
# with the bound 300 s, A4_BG readings in the background (default 2), grown scopes, FMI_READ_WIDEN, FMI_STAG_CELLS,
# FMI_ARGS_LATER, FMI_READ_SOUND, FMI_OFFER_SCREEN, FMI_GOAL_FACT), the run's wall limit (FMI_STOP_WALL, 6600 s; the
# falsifier's bound out of reach) and the game's settings, which are the run's inputs and not mechanisms. Source it as
# ENV for vm/run.sh.
export A4_BG=${A4_BG:-2}
source /var/tmp/fast-mi/wt-e014/experiments/E014-fate-demo/a8/env.sh
export FMI_STOP_WALL=${FMI_STOP_WALL:-6600}
export STOP_JOBS=${STOP_JOBS:-100000}
# the game: its files (read only) and its extraction (scripts/dump-scripts.sh with amendment (2)'s settings, never
# committed)
export E14_GAME_DIR=/var/tmp/fast-mi/games/fate-play-demo
export E14_SCRIPTS=/var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo
GAME="FMI_GAME_ID=atlantis FMI_GAME_DIR=$E14_GAME_DIR FMI_GAME_SCRIPTS=$E14_SCRIPTS"
export GAME
# the run's files (the same paths on the laptop and the VM)
export D_ARCH=${D_ARCH:-/var/tmp/fast-mi/e014-archives/play8}
export D_RUN=${D_RUN:-/var/tmp/fast-mi/e014-archives/play8-run}
export D_OUT=${D_OUT:-/var/tmp/fast-mi/e014-play8}
export STAGE=${STAGE:-play}
