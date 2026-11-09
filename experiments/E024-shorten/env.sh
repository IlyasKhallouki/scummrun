# E024's settings: E022's run (the engine settings of the input movie's run) with the scripts' files kept in each
# engine's memory (patch 0010, FMI_MEM_FILES=1), so a scan's forks of one boot never share the save folder's file
source /var/tmp/fast-mi/wt-e016/experiments/E022-place-context/env.sh
export E13_ENGINE_BIN=/var/tmp/fast-mi/svm-e024/build/scummvm/scummvm
ALLON="$ALLON FMI_MEM_FILES=1"
export ALLON
