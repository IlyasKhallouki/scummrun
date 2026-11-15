# E017 (plan.md "Method"): E016's final set (E016-realize-run/run/env.sh) with one change, FMI_PLAN_TIME=300 and
# FMI_PLAN_DEEPEN=60 (the later assignments win in `env`), on play9's archive resumed as play10.
export D_ARCH=${D_ARCH:-/var/tmp/fast-mi/e017-archives/play10}
export D_RUN=${D_RUN:-/var/tmp/fast-mi/e017-archives/play10-run}
export D_OUT=${D_OUT:-/var/tmp/fast-mi/e017-play10}
source /var/tmp/fast-mi/wt-e016/experiments/E016-realize-run/run/env.sh
ALLON="$ALLON FMI_PLAN_TIME=300 FMI_PLAN_DEEPEN=60"
export ALLON
