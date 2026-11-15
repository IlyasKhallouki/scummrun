# E021's run: E020's set with the scripts' temporary saves kept in the engine's memory (patch 0009, FMI_TEMP_SAVES=1)
source /var/tmp/fast-mi/wt-e016/experiments/E020-held-code/env.sh
export E13_ENGINE_BIN=/var/tmp/fast-mi/svm-e021/build/scummvm/scummvm
ALLON="$ALLON FMI_TEMP_SAVES=1"
export ALLON
