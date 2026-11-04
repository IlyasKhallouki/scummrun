# E014 amendment (4): E013's integrate/env.sh with this amendment's build (target-e014b: e014 with (a) and (b)) and
# switches, E013's engine (svm-e013) unchanged. Source it as ENV.
#   A4 / A4OFF   the amendment's switches on (the bound A4_BOUND s, default 300; A4_BG readings in the background,
#                default 0; grown scopes) / unset
#   ALLON        E013's one switch set (ON3, E12ON, E14M, E13ON) with A4 added; ALLOFF with A4OFF added
export E13T=${E13T:-/var/tmp/fast-mi/target-e014b}
source /var/tmp/fast-mi/wt-e014/experiments/E013-robust/integrate/env.sh
A4="FMI_READ_BOUND=${A4_BOUND:-300} FMI_READ_BOUND_BG=${A4_BG:-0} FMI_SCOPE_GROW=1"
A4OFF="FMI_READ_BOUND= FMI_READ_BOUND_BG= FMI_SCOPE_GROW="
ALLON="$ALLON $A4"
ALLOFF="$ALLOFF $A4OFF"
export A4 A4OFF ALLON ALLOFF
