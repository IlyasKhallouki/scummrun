#!/bin/bash
# E011 G7 on the integrated planner (experiments/E011-speed/plan.md, "Exactness gates"), the plan component's checks
# (rs/fmi-plan/check/) with the binaries built from e011; ring(33) steps under the heavy lock and the guard.
#   1. taskcore's command line (sas, decode, validate, serve on the request sequences) byte for byte the old binary's,
#      on ring(38) j00091 and ring(33) j00089;
#   2. fmi-plan's PlanResult equal to plan_fd.plan_taskcore's (the plan component's reference results) on serve38 and
#      serve33, with Search::Gbfs and Search::FdLamaFirstCosts, the plan cache off;
#   3. the plan cache on: the same sequence twice into an empty store, the second all hits, every result the same;
#   4. the Rust search against Fast Downward's lazy h_FF plusone search on ring(38) goals (fd_same.sh).
# Round 2: G7_SEARCHES (default "gbfs fd"; "lama": fmi-plan seq --search lama, the Rust lama-first-costs); FMI_PLAN_FAST
# passes through (r2-plan's fast forms), and with it 4. runs the watched h_FF (FD_SAME_HFF=watch); G7_LAMA=1: 5. the Rust
# lama-first-costs against Fast Downward's on serve38 and serve33 (route/lama_same.sh: graph, plan file, counts);
# G7_CHECK=1: 6. fmi-plan seq on serve38 and serve33 with the fast forms and every check mode (TASKCORE_CHECK_REDUCE,
# _BUILD, _HFF, _SUCC: the fast form against the reference's on every request; a difference panics), gbfs and lama.
# G7_STEPS: which of steps 1-4 run (default all).
# usage: g7.sh OUT
set -u
source $HOME/.cache/fast-mi-e011/env.sh
[ -n "${E011_ENV:-}" ] && source "$E011_ENV" # a second env file after env.sh (round 2: env-r2.sh, its binaries)
here=$(cd "$(dirname "$0")" && pwd)
PY=$here/../../../.venv/bin/python
C=$WT11/experiments/E011-speed/rs/fmi-plan/check
R38=$HOME/.cache/fast-mi-e011/profile-planner/readings/r38
R33=$HOME/.cache/fast-mi-e011/plan/r33
J=$HOME/.cache/fast-mi-e008/archives/kitchen/jobs
REF=/var/tmp/fast-mi/e011-plan-scratch
OUT=$1; mkdir -p $OUT
bad=0
heavy() { flock /var/tmp/fast-mi/heavy.lock "$@"; }
step() { [[ " ${G7_STEPS:-1 2 3 4} " == *" $1 "* ]]; }  # G7_STEPS: which of steps 1-4 run
check() { # LABEL LOG: the log's DIFF/MISSING lines fail
  if grep -qE "^(DIFF|MISSING)|different [1-9]" $2; then bad=1; echo "$1: FAIL"; grep -E "^(DIFF|MISSING)|different [1-9]" $2 | head -5
  else echo "$1: pass ($(grep -c '^same' $2) same)"; fi
}
p38=$J/j00091/run/a00/plans/plan-000; p33=$J/j00089/run/a00/plans/plan-000

if step 1; then
echo "[$(date +%T)] 1. taskcore CLI"
$C/cli_same.sh $TASKCORE_BIN $TASKCORE_E011 $R38/reading.tc bit:108=1 $p38/start.txt $p38/caps.txt $p38/fd/sas_plan \
  $HOME/.cache/fast-mi-e011/plan/names38.txt $OUT/cli38 $C/serve38.tsv > $OUT/cli38.log 2>&1
check "cli r38" $OUT/cli38.log
heavy $C/cli_same.sh $TASKCORE_BIN $TASKCORE_E011 $R33/reading.tc bit:108=1 $p33/start.txt $p33/caps.txt $p33/fd/sas_plan \
  $HOME/.cache/fast-mi-e011/plan/names33.txt $OUT/cli33 $C/serve33.tsv > $OUT/cli33.log 2>&1
check "cli r33" $OUT/cli33.log
rm -rf $OUT/cli33/*/*.sas $OUT/cli33/*/*/*.sas
fi

if step 2; then
echo "[$(date +%T)] 2. PlanResult = plan_fd.plan_taskcore"
for s in ${G7_SEARCHES:-gbfs fd}; do
  FMI_PLAN_CACHE= $FMI_PLAN_TOOL seq --reading $R38 --reqs $C/serve38.tsv --search $s --dump $OUT/seq38-$s \
    > $OUT/seq38-$s.jsonl 2> $OUT/seq38-$s.err
  $PY $C/compare.py $OUT/seq38-$s $REF/r5-ref38 > $OUT/cmp38-$s.log 2>&1; check "r38 $s" $OUT/cmp38-$s.log
  FMI_PLAN_CACHE= heavy $FMI_PLAN_TOOL seq --reading $R33 --reqs $C/serve33.tsv --search $s --dump $OUT/seq33-$s \
    > $OUT/seq33-$s.jsonl 2> $OUT/seq33-$s.err
  $PY $C/compare.py $OUT/seq33-$s $REF/r5-ref33 > $OUT/cmp33-$s.log 2>&1; check "r33 $s" $OUT/cmp33-$s.log
done
fi
if step 3; then
echo "[$(date +%T)] 3. the plan cache"
rm -rf $OUT/plans
for n in 1 2; do
  FMI_PLAN_CACHE=$OUT/plans $FMI_PLAN_TOOL seq --reading $R38 --reqs $C/serve38.tsv --search gbfs --dump $OUT/cache38-$n \
    > $OUT/cache38-$n.jsonl 2> $OUT/cache38-$n.err
  $PY $C/compare.py $OUT/cache38-$n $REF/r5-ref38 > $OUT/cmpc38-$n.log 2>&1; check "r38 cache pass $n" $OUT/cmpc38-$n.log
done
hits=$(grep -c '"source": "cache"' $OUT/cache38-2.jsonl); n=$(grep -c '"name"' $OUT/cache38-2.jsonl)
echo "r38 cache pass 2: $hits of $n from the store"; [ "$hits" = "$n" ] || bad=1
fi

if step 4; then
echo "[$(date +%T)] 4. Rust search = FD lazy h_FF plusone (ring(38))"
FD_SAME_HFF=$( [ "${FMI_PLAN_FAST:-0}" = 1 ] && echo watch ) $C/fd_same.sh $TASKCORE_E011 $R38/reading.tc $p38/start.txt $p38/caps.txt $OUT/fd38 60 bit:108=1 bit:109=1 bit:110=1 \
  bit:111=1 bit:113=1 bit:114=1 bit:115=1 bit:117=1 owner:566=1 > $OUT/fd38.log 2>&1
grep -E "^(same|DIFF|both)" $OUT/fd38.log; check "fd38" $OUT/fd38.log
rm -rf $OUT/fd38/*/task.sas
fi
if [ "${G7_LAMA:-0}" = 1 ]; then
  echo "[$(date +%T)] 5. the Rust lama-first-costs = Fast Downward's lama-first-costs (serve38, serve33)"
  for r in 38 33; do
    eval tc=\$R$r/reading.tc
    heavy $WT11/experiments/E011-speed/route/lama_same.sh $TASKCORE_E011 $LAMA_E011 $tc $C/serve$r.tsv $OUT/lama$r 600 \
      > $OUT/lama$r.log 2>&1
    tail -1 $OUT/lama$r.log; check "lama$r" $OUT/lama$r.log
  done
fi
if [ "${G7_CHECK:-0}" = 1 ]; then
  echo "[$(date +%T)] 6. the fast forms in check mode"
  for s in gbfs lama; do
    for r in 38 33; do
      eval rd=\$R$r
      FMI_PLAN_FAST=1 TASKCORE_CHECK_REDUCE=1 TASKCORE_CHECK_BUILD=1 TASKCORE_CHECK_HFF=1 TASKCORE_CHECK_SUCC=1 FMI_PLAN_CACHE= \
        heavy $FMI_PLAN_TOOL seq --reading $rd --reqs $C/serve$r.tsv --search $s --dump $OUT/chk$r-$s > $OUT/chk$r-$s.jsonl \
        2> $OUT/chk$r-$s.err; rc=$?
      $PY $C/compare.py $OUT/chk$r-$s $REF/r5-ref$r > $OUT/cmpchk$r-$s.log 2>&1
      [ $rc = 0 ] || { bad=1; echo "check r$r $s: rc $rc"; tail -3 $OUT/chk$r-$s.err; }
      check "check r$r $s ($(grep -c "^check: the two-level" $OUT/chk$r-$s.err) reduce, $(grep -c "^build: the one-pass" $OUT/chk$r-$s.err) build checks)" $OUT/cmpchk$r-$s.log
    done
  done
fi
echo "[$(date +%T)] G7 $([ $bad = 0 ] && echo PASS || echo FAIL)"
exit $bad
