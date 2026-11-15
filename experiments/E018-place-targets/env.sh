# E018: E017's set (plan limits 300 s and 60 s on E016's final set) with FMI_TARGET_PLACE=1, on play10's archive.
source /var/tmp/fast-mi/wt-e016/experiments/E017-c5/env.sh
ALLON="$ALLON FMI_TARGET_PLACE=1"
export ALLON
# added at 10:15 (plan, "Reveal"): an outcome that brings new objects or actors into the hit test is kept (3 a step)
ALLON="$ALLON FMI_REVEAL=1"
export ALLON
# added at 10:45: within a place, rarer states first (FMI_PLACE_RARITY)
ALLON="$ALLON FMI_PLACE_RARITY=1"
export ALLON
