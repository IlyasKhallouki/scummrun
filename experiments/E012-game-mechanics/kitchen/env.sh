# E012's kitchen rerun: integrate/env.sh with this worktree's build (E12T) and worktree (WT11); every other path is the
# integration's (round 2's tools, end's engine). Source it as ENV for kitchen/run.sh and kitchen/verify.sh.
export E12T=${E12T:-/var/tmp/fast-mi/target-e012-kitchen}
source /var/tmp/fast-mi/wt-e012-kitchen/experiments/E012-game-mechanics/integrate/env.sh
export WT11=/var/tmp/fast-mi/wt-e012-kitchen
# the rerun's files (plan.md "Runs"): the archive, and the run's caches beside it
export K_ARCH=/var/tmp/fast-mi/e012-archives/kitchen
export K_RUN=/var/tmp/fast-mi/e012-archives/run
export K_OUT=/var/tmp/fast-mi/e012-kitchen
