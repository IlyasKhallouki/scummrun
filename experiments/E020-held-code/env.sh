# E020's run: E019's set with the readings of held objects' code (FMI_READ_HELD=1).
source /var/tmp/fast-mi/wt-e016/experiments/E019-speed/env.sh
ALLON="$ALLON FMI_READ_HELD=1"
export ALLON
# amendment (1): one fresh reading of a scope at a time, shared by the tries that wait for it
ALLON="$ALLON FMI_READ_SHARE=1"
export ALLON
# amendment (3): iMUSE's sequencer by game time (patch 0008, the engine at svm-e020), and an entry whose return
# differs is dead while the run goes on
export E13_ENGINE_BIN=/var/tmp/fast-mi/svm-e020/build/scummvm/scummvm
ALLON="$ALLON FMI_IMUSE_TICKS=1 FMI_DIFFERS_DEAD=1"
export ALLON
# amendment (4): checkpoints compared without VAR_SOUNDRESULT; the in-process reading's RSS bound at 40 GB
ALLON="$ALLON FMI_SKIP_SOUNDRESULT=1 FMI_READ_MAX_RSS_GB=40"
export ALLON
