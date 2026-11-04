# E014, amendment (3): the interactive demo on E013's build and engine with every switch on. E013's integrate/env.sh
# (target-e013, svm-e013 with patches 0001-0005, 0006-e014 and 0007; ALLON: ON3, E12ON, E014's mechanisms and E013's
# switches), and the game's settings, which are the run's inputs and not mechanisms. Source it as ENV for vm/run.sh.
source /var/tmp/fast-mi/wt-e013/experiments/E013-robust/integrate/env.sh
# the game: its files (read only) and its extraction (scripts/dump-scripts.sh with amendment (2)'s settings, never
# committed)
export E14_GAME_DIR=/var/tmp/fast-mi/games/fate-play-demo
export E14_SCRIPTS=/var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo
GAME="FMI_GAME_ID=atlantis FMI_GAME_DIR=$E14_GAME_DIR FMI_GAME_SCRIPTS=$E14_SCRIPTS"
export GAME
# the run's files (the same paths on the laptop and the VM)
export D_ARCH=${D_ARCH:-/var/tmp/fast-mi/e014-archives/play-vm}
export D_RUN=${D_RUN:-/var/tmp/fast-mi/e014-archives/play-vm-run}
export D_OUT=${D_OUT:-/var/tmp/fast-mi/e014-play-vm}
export STAGE=${STAGE:-play}
