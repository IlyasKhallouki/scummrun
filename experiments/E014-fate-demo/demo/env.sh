# E014: E012's integrate/env.sh (round 2's tools, ON3, E12ON) with E014's build (E12T), worktree and engine, and the
# demo's settings. Source it as ENV for demo/run.sh and demo/verify.sh.
export E12T=${E12T:-/var/tmp/fast-mi/target-e014}
source /var/tmp/fast-mi/wt-e014/experiments/E012-game-mechanics/integrate/env.sh
export WT11=/var/tmp/fast-mi/wt-e014
# E14_FMI_BIN: another fmi binary (a scratch build); default the release build's
[ -n "${E14_FMI_BIN:-}" ] && export FMI_BIN=$E14_FMI_BIN
# the engine: patches 0001-0005 and 0006-e014 (experiments/E014-fate-demo/engine/build.sh)
export E14_ENGINE_BIN=/var/tmp/fast-mi/svm-e014/build/scummvm/scummvm
export FMI_ENGINE_BIN=$E14_ENGINE_BIN
# the game: its files (read only) and its extraction (scripts/dump-scripts.sh with E014's settings, never committed)
export E14_GAME_DIR=/var/tmp/fast-mi/games/fate-cd-demo
export E14_SCRIPTS=/var/tmp/fast-mi/e014-root/data/scripts-fate-demo
# every E014 switch: the game (engine target, files, extraction), the decision point without verb ids, the end's site,
# the end as the stage goal
E14ON="FMI_GAME_ID=atlantis FMI_GAME_DIR=$E14_GAME_DIR FMI_GAME_SCRIPTS=$E14_SCRIPTS SPEEDRUN_IDLE=handler SPEEDRUN_END_SITE=1 FMI_GOAL_END=1 FMI_READ_EVENTS=1"
export E14ON
# the run's files
export D_ARCH=${D_ARCH:-/var/tmp/fast-mi/e014-archives/demo}
export D_RUN=${D_RUN:-/var/tmp/fast-mi/e014-archives/run}
export D_OUT=${D_OUT:-/var/tmp/fast-mi/e014-demo}
