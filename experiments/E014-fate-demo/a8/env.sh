# E014 amendment (8): a7/env.sh (ALLON with A4, FMI_READ_WIDEN and FMI_STAG_CELLS) with this amendment's build
# (target-e014e: e014 with FMI_ARGS_LATER, FMI_READ_SOUND, FMI_OFFER_SCREEN, FMI_GOAL_FACT and FMI_STOP_WALL) and its
# switches added to ALLON and A4. Source it as ENV (a4/mi-check.sh takes it as A4_ENV).
#   A8 / A8OFF   FMI_ARGS_LATER=1 FMI_READ_SOUND=1 FMI_OFFER_SCREEN=1 FMI_GOAL_FACT=1 / unset
export E13T=${E13T:-/var/tmp/fast-mi/target-e014e}
source /var/tmp/fast-mi/wt-e014/experiments/E014-fate-demo/a7/env.sh
A8="FMI_ARGS_LATER=1 FMI_READ_SOUND=1 FMI_OFFER_SCREEN=1 FMI_GOAL_FACT=1"
A8OFF="FMI_ARGS_LATER= FMI_READ_SOUND= FMI_OFFER_SCREEN= FMI_GOAL_FACT="
ALLON="$ALLON $A8"
ALLOFF="$ALLOFF $A8OFF"
A4="$A4 $A8"
export A4 A8 A8OFF ALLON ALLOFF
