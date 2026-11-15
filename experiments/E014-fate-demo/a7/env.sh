# E014 amendment (7): a6/env.sh (ALLON with A4 and FMI_READ_WIDEN) with this amendment's build (target-e014d: e014 with
# FMI_STAG_CELLS and the bridge's refused inputs as trials) and its switch added to ALLON and A4. Source it as ENV
# (a4/mi-check.sh takes it as A4_ENV).
#   A7 / A7OFF   FMI_STAG_CELLS=1 / unset
export E13T=${E13T:-/var/tmp/fast-mi/target-e014d}
source /var/tmp/fast-mi/wt-e014/experiments/E014-fate-demo/a6/env.sh
A7="FMI_STAG_CELLS=1"
A7OFF="FMI_STAG_CELLS="
ALLON="$ALLON $A7"
ALLOFF="$ALLOFF $A7OFF"
A4="$A4 $A7"
export A4 A7 A7OFF ALLON ALLOFF
