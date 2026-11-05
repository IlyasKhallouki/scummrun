# E020's run: E019's set with the readings of held objects' code (FMI_READ_HELD=1).
source /var/tmp/fast-mi/wt-e016/experiments/E019-speed/env.sh
ALLON="$ALLON FMI_READ_HELD=1"
export ALLON
# amendment (1): one fresh reading of a scope at a time, shared by the tries that wait for it
ALLON="$ALLON FMI_READ_SHARE=1"
export ALLON
