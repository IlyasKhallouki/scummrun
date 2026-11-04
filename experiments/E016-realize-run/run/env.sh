# E016 (plan.md "Monkey Island with every switch on", "The run: play9"): E014's a8/env.sh (play8's ALLON, E013's engine)
# with E016's merged build (target-e016: e016 with fix A and C-F; its fmi-checks in target-e016/checks, as E014's
# builds), scope growth off and E016's switches; then the demo's settings and the run's paths, as a8/run-env.sh. Source
# it as ENV (E014's vm/run.sh, a4/mi-check.sh as A4_ENV) or from run/laptop.sh, run/k1.sh, run/mi-check.sh.
#   E16 / E16OFF   FMI_REALIZE=1 FMI_NOGOOD=1 FMI_FREE_T=archive FMI_GOAL_SITE=verb FMI_FRONTIER=1 / unset
#   ALLON          play8's ALLON with FMI_SCOPE_GROW=room (plan F: a room's reading is that room; amendment (8)), with E16
#   ALLOFF         play8's ALLOFF with E16OFF
#   A4             E014's A4 (a4/mi-check.sh's set: the bound, background readings, A6-A8) with FMI_SCOPE_GROW=room, with E16
#   LADDER         the outside ladder (fmi-checks ladder) of the merged build
#   GAME, D_*      the demo's settings (the run's inputs, not mechanisms) and the run's files (play9 by default)
export E13T=${E13T:-/var/tmp/fast-mi/target-e016}
source /var/tmp/fast-mi/wt-e014/experiments/E014-fate-demo/a8/env.sh
nogrow() {
  local out="" w
  for w in $1; do
    case $w in FMI_SCOPE_GROW=*) ;; *) out="$out $w";; esac
  done
  echo "$out"
}
# FMI_PLAN_DEEPEN=10 (amendment (9)): the first plan limit 10 s, doubled per deferral, where play8 had 120 s
# FMI_NOVEL_PLACE=1 (amendment (10)): a seen room with a new set of owned objects is novel
# FMI_SELECT_PLACE=1 (amendment (11)): places by their own tries before cells
# FMI_PLAN_TIME=30 (amendment (12)): every plan of a try at most 30 s, where it was 600 s after the first
# FMI_REACH_ZEROS=1 (amendment (13)): reach and refine starts carry the domains' cleared keys at 0
E16="FMI_REALIZE=1 FMI_NOGOOD=1 FMI_FREE_T=archive FMI_GOAL_SITE=verb FMI_FRONTIER=1 FMI_PLAN_DEEPEN=10 FMI_NOVEL_PLACE=1 FMI_SELECT_PLACE=1 FMI_PLAN_TIME=30 FMI_REACH_ZEROS=1"
E16OFF="FMI_REALIZE= FMI_NOGOOD= FMI_FREE_T= FMI_GOAL_SITE= FMI_FRONTIER= FMI_NOVEL_PLACE= FMI_SELECT_PLACE= FMI_PLAN_TIME= FMI_REACH_ZEROS="
ALLON="$(nogrow "$ALLON") FMI_SCOPE_GROW=room $E16"
ALLOFF="$ALLOFF $E16OFF"
A4="$(nogrow "$A4") FMI_SCOPE_GROW=room $E16"
export A4 E16 E16OFF ALLON ALLOFF
export LADDER=${LADDER:-$E13T/checks/release/ladder}
# the demo: its files (read only) and its extraction (never committed)
export E14_GAME_DIR=/var/tmp/fast-mi/games/fate-play-demo
export E14_SCRIPTS=/var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo
GAME="FMI_GAME_ID=atlantis FMI_GAME_DIR=$E14_GAME_DIR FMI_GAME_SCRIPTS=$E14_SCRIPTS"
export GAME
# the run's files
export D_ARCH=${D_ARCH:-/var/tmp/fast-mi/e016-archives/play9}
export D_RUN=${D_RUN:-/var/tmp/fast-mi/e016-archives/play9-run}
export D_OUT=${D_OUT:-/var/tmp/fast-mi/e016-play9}
export STAGE=${STAGE:-play}
# the demo's files are read from fixed paths: a missing one fails here, not as a 600 s connect timeout
for d in "$E14_GAME_DIR" "$E14_SCRIPTS"; do
  [ -d "$d" ] || { echo "env.sh: missing $d" >&2; return 1 2>/dev/null || exit 1; }
done
# Monkey Island's files are a link in each worktree (never committed): mi_game checks it before its checks
mi_game() {
  local d; d=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)/game/dos-cd
  [ -d "$d" ] || { echo "env.sh: missing $d (link game to the main checkout's game)" >&2; return 1; }
}
