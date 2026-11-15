# E019's run: E018's set (place targets, reveal, rarity...) with the measured speed fixes: repeated offer points and
# repeated tries skipped, no offers at the wait's stops.
source /var/tmp/fast-mi/wt-e016/experiments/E018-place-targets/env.sh
ALLON="$ALLON FMI_FREE_SAME=1 FMI_SKIP_SAME=1 FMI_FREE_NO_STOPS=1"
export ALLON
ALLON="$ALLON FMI_REALIZE_HINTS=1"
export ALLON
