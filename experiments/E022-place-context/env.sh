# E022's run: E021's set with a place's context (FMI_PLACE_CTX=1)
source /var/tmp/fast-mi/wt-e016/experiments/E021-quiz/env.sh
ALLON="$ALLON FMI_PLACE_CTX=1"
export ALLON
# amendment (2): a refinement whose last pass wrote values outside a new fact variable's domain seeds them and reads
# again, at most 3 times
ALLON="$ALLON FMI_REFINE_WIDEN=3"
export ALLON
# amendment (6): a fresh reading and its re-reads take the live slots' locals (F10)
ALLON="$ALLON FMI_SLOT_LOCALS=1"
# amendment (7): a plan on the goal's families of actions first (partial grounding), the whole reading after
ALLON="$ALLON FMI_PLAN_FOCUS=1"
# amendment (9): the scope rooms' input vars are facts of a fresh reading
ALLON="$ALLON FMI_READ_INPUT_FACTS=1"
export ALLON
