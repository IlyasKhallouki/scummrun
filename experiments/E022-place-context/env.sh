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
# amendment (10): the frontier also holds the untried least live holders of atoms of T
ALLON="$ALLON FMI_FRONTIER_ATOMS=1"
export ALLON
# amendment (11): a room change to a room outside the scope is an action that ends there
ALLON="$ALLON FMI_READ_EXITS=1"
# amendment (12): an object with a parent is placed only with its parent in the state findObject needs
ALLON="$ALLON FMI_READ_PARENTS=1"
# amendment (13): a child whose run holds the goal is a goal child though the plan's goal records did not match
ALLON="$ALLON FMI_GOAL_UNPLANNED=1"
export ALLON
# amendment (14): a start of a global script that enumerates the inventory is summarized as the inventory hook's
ALLON="$ALLON FMI_READ_INVENTORY_SUMMARY=1"
export ALLON
# amendment (15): the atom frontier counts the current driver's tries only (a changed model tries its frontier again)
ALLON="$ALLON FMI_FRONTIER_FRESH=1"
export ALLON
# amendment (17): ahead of the place keys, the cells tried least, the most advanced (guide atoms set) first
ALLON="$ALLON FMI_SELECT_ADVANCED=1"
export ALLON
# amendment (17): a continuing run with an unchanged model leaves the fresh reset off (every restart put hundreds of
# untried first holders back ahead of the rest)
ALLON="$ALLON FMI_FRONTIER_FRESH=0"
export ALLON
# amendment (19): object code's local starts go to the object's own room in the F flags
ALLON="$ALLON FMI_READ_OBJ_ROOM=1"
export ALLON
