#!/bin/bash
# E016 (plan.md "Monkey Island with every switch on", rule (2)): Monkey Island's checks since E013, b3, kitchen and
# smoke, as E014-fate-demo/a4/mi-check.sh runs them, with env.sh's set (play8's ALLON less scope growth, with E16) and
# E016's merged build (target-e016) in place of E014's: a4/mi-check.sh takes env.sh as its A4_ENV.
#   b3       B3's 15 tries against E013's G-on run of the same, every new entry verified from boot
#   kitchen  E012's kitchen archive (a copy): the goal entry's chain verified from boot
#   smoke    E012's smoke run (4 jobs from the boot, b = 1) in ALLON, every entry verified from boot, the stall report
# usage: mi-check.sh OUT b3|kitchen|smoke...   (default OUT /var/tmp/fast-mi/e016-mi-check, all three)
set -u
here=$(cd "$(dirname "$0")" && pwd)
O=${1:-/var/tmp/fast-mi/e016-mi-check}
shift || true
A4_ENV=$here/env.sh exec bash /var/tmp/fast-mi/wt-e014/experiments/E014-fate-demo/a4/mi-check.sh $O ${@:-b3 kitchen smoke}
