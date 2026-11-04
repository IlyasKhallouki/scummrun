# E014 amendment (4): the run's environment on the laptop and the VM: a4/env.sh (target-e014b, E013's engine, ALLON
# with A4: the bound 300 s, A4_BG readings in the background (default 2), grown scopes) and the game's settings, which
# are the run's inputs and not mechanisms. Source it as ENV for vm/run.sh.
export A4_BG=${A4_BG:-2}
source /var/tmp/fast-mi/wt-e014/experiments/E014-fate-demo/a4/env.sh
# the game: its files (read only) and its extraction (scripts/dump-scripts.sh with amendment (2)'s settings, never
# committed)
export E14_GAME_DIR=/var/tmp/fast-mi/games/fate-play-demo
export E14_SCRIPTS=/var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo
GAME="FMI_GAME_ID=atlantis FMI_GAME_DIR=$E14_GAME_DIR FMI_GAME_SCRIPTS=$E14_SCRIPTS"
export GAME
# the run's files (the same paths on the laptop and the VM)
export D_ARCH=${D_ARCH:-/var/tmp/fast-mi/e014-archives/play-vm4}
export D_RUN=${D_RUN:-/var/tmp/fast-mi/e014-archives/play-vm4-run}
export D_OUT=${D_OUT:-/var/tmp/fast-mi/e014-play-vm4}
export STAGE=${STAGE:-play}
