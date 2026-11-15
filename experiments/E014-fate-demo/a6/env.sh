# E014 amendment (6): a4/env.sh (ALLON with A4) with this amendment's build (target-e014c: e014 with FMI_READ_WIDEN)
# and its switch added to ALLON and A4. Source it as ENV (a4/mi-check.sh takes it as A4_ENV).
#   A6 / A6OFF   FMI_READ_WIDEN=1 / unset
export E13T=${E13T:-/var/tmp/fast-mi/target-e014c}
source /var/tmp/fast-mi/wt-e014/experiments/E014-fate-demo/a4/env.sh
A6="FMI_READ_WIDEN=1"
A6OFF="FMI_READ_WIDEN="
ALLON="$ALLON $A6"
ALLOFF="$ALLOFF $A6OFF"
A4="$A4 $A6"
export A4 A6 A6OFF ALLON ALLOFF
