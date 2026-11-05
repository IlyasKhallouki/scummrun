# E022's run: E021's set with a place's context (FMI_PLACE_CTX=1)
source /var/tmp/fast-mi/wt-e016/experiments/E021-quiz/env.sh
ALLON="$ALLON FMI_PLACE_CTX=1"
export ALLON
# amendment (2): a refinement whose last pass wrote values outside a new fact variable's domain seeds them and reads
# again, at most 3 times
ALLON="$ALLON FMI_REFINE_WIDEN=3"
export ALLON
