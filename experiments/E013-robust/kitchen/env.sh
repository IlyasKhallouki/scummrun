# E013's kitchen rerun K1 (plan.md "Runs", amendment (2) (b) and (c)): integrate/env.sh (e013's build and engine, the
# switch sets) and the run's files. Source it as ENV for kitchen/run.sh, verify.sh and verify-all.sh.
source /var/tmp/fast-mi/wt-e013/experiments/E013-robust/integrate/env.sh
export K_ARCH=${K_ARCH:-/var/tmp/fast-mi/e013-archives/kitchen}
export K_RUN=${K_RUN:-/var/tmp/fast-mi/e013-archives/run}
export K_OUT=${K_OUT:-/var/tmp/fast-mi/e013-kitchen}
