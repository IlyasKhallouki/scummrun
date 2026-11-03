#!/bin/bash
# E012 integration: the gates on branch e012's build (integrate/build.sh) and end's engine (patches 0001-0005), with
# every E012 switch off and on. Each stage logs a line to OUT/summary.txt; heavy steps run under the heavy lock and the
# guard at nice 10. Outputs that hold game text (dumps, readings) stay under OUT.
# usage: gates.sh OUT STAGE...
#   tests      the cargo tests: fmi-core, fmi-reading (T1 on the kitchen reading j00065), fmi-plan, fmi-explore (S1,
#              afford, targets, wait), fmi, fmi-play's lib (R3, R2 at stride 1; R2's table), fmi-engine with the engine
#              tests (end's hooks), scumm-reader's memo tests (resident)
#   engine     E011 G2, G3, G4 and G5 (engine/check-r2.sh) with SPEEDRUN_REPORT_RESTART off and on (end/scummvm-report.sh)
#   g1off|g1on E011 G1 (fork_exact.py, 7 load children) with the switch off or on, under the heavy lock
#   g6         E011 G6 on e012's reader-scope: the kitchen golden; --prep as fmi runs it, memo off and both memo pairs;
#              then resident's RS1 (G6's readings in one fmi process with FMI_READER_LIB=1)
#   g7         E011 G7 off (the plan's reference forms) and on (the fast forms in every check mode, the proofs on)
#   g8         E011 G8 (fmi-reading's consumer)
#   g9         E011 G9 with everything off (OFF3, E12OFF) and on (ON3, E12ON)
#   g10        E011 G10: play's fixture tests (play-lean) off and on; test (a) with `fmi try`: the Rust lama-first-costs
#              (strict, ready, fast) with every E012 switch off and on, Fast Downward's binary (strict, ready) off
#   end        end's hooks and boxes at the kitchen check's entries (fmi-engine-end)
#   wait       wait's W1-W3 (room 28's entries, entry 4) and W4 (under the heavy lock), everything on
#   delay      delay's R1 and R3 on the engine (delay-check, the fork forced at p = 1/2, 1/4, 1/21 and with no K)
#   targets    targets' T2 (targets_t2 on the 19 readings made again)
#   afford     afford's A1-A3 (afford/gates.sh) and the walk boxes (afford/boxes.sh) on end's engine
set -u
here=$(cd "$(dirname "$0")" && pwd)
export ENV=$here/env.sh E011_ENV=$here/env.sh
source $ENV
WT=$WT11; X=$WT/experiments/E011-speed; E12=$WT/experiments/E012-game-mechanics
O=$1; shift; mkdir -p $O
S=$O/summary.txt
LOCK=/var/tmp/fast-mi/heavy.lock; GUARD=$HOME/.cache/fast-mi-e007/guard.sh
export TMPDIR=/var/tmp/fast-mi/e012-int/tmp; mkdir -p $TMPDIR
log() { echo "[$(date +%T)] $*" | tee -a $S; }
RS=$X/rs
ARCH=/var/tmp/fast-mi/e011-archives/kitchen
for st in "$@"; do
  case $st in
  tests)
    d=$O/tests; mkdir -p $d
    log "tests: cargo test (release, debug info off) under the heavy lock"
    flock $LOCK $GUARD $d/rs.log bash -c "cd $RS && CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_TARGET_DIR=$E12T FMI_T1_READING=$ARCH/readings/28-33-41-j00065 cargo test --release -j 2 -p fmi-core -p fmi-reading -p fmi-plan -p fmi-explore -p fmi"
    log "tests rs: $(grep -E '^test result' $d/rs.log | awk '{p+=$4; f+=$6; i+=$8} END {print p" passed, "f" failed, "i" ignored"}'); $(grep -c FAILED $d/rs.log) FAILED lines; $(tail -1 $d/rs.log)"
    flock $LOCK $GUARD $d/play.log bash -c "cd $RS && CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_TARGET_DIR=$E12T cargo test --release -j 2 -p fmi-play --lib && CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_TARGET_DIR=$E12T cargo test --release -j 2 -p fmi-play --lib -- --ignored r2_table --nocapture"
    log "tests fmi-play: $(grep -E '^test result' $d/play.log | tr '\n' ' '); $(grep -E '^R2 stride' $d/play.log | grep -c pass) of 10 R2 strides pass; $(tail -1 $d/play.log)"
    flock $LOCK $GUARD $d/engine.log bash -c "cd $RS && FMI_ENGINE_BIN=$E12_ENGINE_BIN CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_TARGET_DIR=$E12T cargo test --release -j 2 -p fmi-engine -- --include-ignored"
    log "tests fmi-engine: $(grep -E '^test result' $d/engine.log | tr '\n' ' '); $(tail -1 $d/engine.log)"
    flock $LOCK $GUARD $d/scumm.log bash -c "cd $WT/experiments/E003-path-through-cloud/scumm && CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_TARGET_DIR=$E12T/scumm cargo test --release -j 2 -p scumm-reader --lib memo::"
    log "tests scumm-reader memo: $(grep -E '^test result' $d/scumm.log | tr '\n' ' '); $(tail -1 $d/scumm.log)";;
  engine)
    for mode in off on; do
      eb=$E12_ENGINE_BIN; [ $mode = on ] && eb=$E12/end/scummvm-report.sh
      mkdir -p $O/engine-$mode
      for g in g2 g3 g4 g5; do
        END_ENGINE_BIN=$E12_ENGINE_BIN E011_ENGINE_BIN=$eb E011_ENGINE_OUT=$O/engine-$mode nice -n 10 $X/engine/check-r2.sh $g > $O/engine-$mode/$g.log 2>&1
        log "engine $mode $g exit $?: $(tail -3 $O/engine-$mode/$g.log | tr '\n' ' ' | cut -c1-300)"
      done
    done;;
  g1off|g1on)
    mode=${st#g1}; eb=$E12_ENGINE_BIN; [ $mode = on ] && eb=$E12/end/scummvm-report.sh
    mkdir -p $O/engine-$mode
    flock $LOCK $GUARD $O/engine-$mode/g1.guard.log env END_ENGINE_BIN=$E12_ENGINE_BIN E011_ENGINE_BIN=$eb E011_ENGINE_OUT=$O/engine-$mode $X/engine/check-r2.sh g1
    log "engine $mode g1: $(grep -v GUARD $O/engine-$mode/g1.guard.log | tail -4 | tr '\n' ' ' | cut -c1-400) $(grep GUARD $O/engine-$mode/g1.guard.log)";;
  g6)
    d=$O/g6; mkdir -p $d
    READER_MEMO_FLOOR_GB=5 G6_PARTS="kitchen" $X/bench/g6.sh $d/kitchen > $d/kitchen.log 2>&1
    log "G6 kitchen golden exit $?: $(tail -1 $d/kitchen.log)"
    READER_MEMO_FLOOR_GB=5 G6_PARTS="off memo89 memo33" G6_ARGS=--prep G6_NOGOLDEN=1 $X/bench/g6.sh $d/prep3 > $d/prep3.log 2>&1
    log "G6 --prep (fmi's form), memo off and both pairs, exit $?: $(grep -E 'pass|FAIL' $d/prep3.log | cut -c1-90 | tr '\n' '|')"
    RESIDENT_TARGET=$E12T $E12/resident/rs1.sh $d/rs1 > $d/rs1.log 2>&1
    log "RS1 exit $?: $(tail -6 $d/rs1.log | tr '\n' '|')";;
  g7)
    d=$O/g7; mkdir -p $d
    FMI_PLAN_FAST=0 FMI_PLAN_DEAD=0 G7_SEARCHES="gbfs fd lama" G7_LAMA=1 $X/bench/g7.sh $d/off > $d/off.log 2>&1
    log "G7 off exit $?: $(tail -1 $d/off.log)"
    FMI_PLAN_FAST=1 FMI_PLAN_DEAD=1 G7_SEARCHES="gbfs fd lama" G7_CHECK=1 $X/bench/g7.sh $d/on > $d/on.log 2>&1
    log "G7 on exit $?: $(tail -1 $d/on.log)";;
  g8)
    d=$O/g8; mkdir -p $d
    $X/bench/g8.sh $d/run > $d/run.log 2>&1
    log "G8 exit $?: $(tail -4 $d/run.log | tr '\n' ' ')";;
  g9)
    d=$O/g9; mkdir -p $d
    env $OFF3 $E12OFF $X/bench/g9.sh $d/off > $d/off.log 2>&1
    log "G9 off exit $?: $(grep -E 'pass|FAIL' $d/off.log | tr '\n' ' ' | cut -c1-400)"
    env $ON3 $E12ON $X/bench/g9.sh $d/on > $d/on.log 2>&1
    log "G9 on exit $?: $(grep -E 'pass|FAIL' $d/on.log | tr '\n' ' ' | cut -c1-400)";;
  g10)
    d=$O/g10; mkdir -p $d
    env $ON3 $E12OFF $X/bench/g10play.sh $d/play-off > $d/play-off.log 2>&1
    log "G10 play fixtures off exit $?: $(grep -E '^(READY|STRICT|CHILDREN|RECS)|rc ' $d/play-off.log | tr '\n' '|' | cut -c1-400)"
    env $ON3 $E12ON $X/bench/g10play.sh $d/play-on > $d/play-on.log 2>&1
    log "G10 play fixtures on exit $?: $(grep -E '^(READY|STRICT|CHILDREN|RECS)|rc ' $d/play-on.log | tr '\n' '|' | cut -c1-400)"
    env $ON3 $E12OFF $X/route/g10.sh $FMI_BIN $d/lama-off lama-strict lama-ready lama-fast > $d/lama-off.log 2>&1
    log "G10 lama, E012 off, exit $?: $(grep -E 'CHECK|verify' $d/lama-off.log | cut -c1-160 | tr '\n' '|')"
    env $ON3 $E12ON $X/route/g10.sh $FMI_BIN $d/lama-on lama-strict lama-ready lama-fast > $d/lama-on.log 2>&1
    log "G10 lama, E012 on, exit $?: $(grep -E 'CHECK|verify' $d/lama-on.log | cut -c1-160 | tr '\n' '|')"
    env $OFF3 $E12OFF $X/play/r2g10.sh $d/fd-off $FMI_BIN $FMI_ENGINE_BIN $FMI_ENGINE_TOOL fd-strict fd-ready > $d/fd-off.log 2>&1
    log "G10 FD's binary, everything off, exit $?: $(grep -E 'CHECK|verify' $d/fd-off.log | cut -c1-160 | tr '\n' '|')";;
  end)
    d=$O/end; mkdir -p $d
    END_OUT=$d END_TARGET=$E12T END_ENGINE_BIN=$E12_ENGINE_BIN $E12/end/check.sh hooks > $d/hooks.out 2>&1
    log "end hooks exit $?: $(grep -iE 'pass|fail|exit' $d/hooks.out | tail -4 | tr '\n' '|' | cut -c1-400)"
    END_OUT=$d END_TARGET=$E12T END_ENGINE_BIN=$E12_ENGINE_BIN $E12/end/check.sh boxes > $d/boxes.out 2>&1
    log "end boxes exit $?: $(tail -3 $d/boxes.out | tr '\n' '|' | cut -c1-400)";;
  wait)
    d=$O/wait; mkdir -p $d
    WENV=$d/env.sh
    { echo "source $ENV"; echo "export FMI_ENGINE_BIN=$E12_ENGINE_BIN"; echo "export ON3=\"\$ON3 \$E12ON\""; } > $WENV
    ENV=$WENV WAIT_DATA=$d/data $E12/wait/gates.sh $d/out room28 room28b e4 w4 > $d/gates.log 2>&1
    log "wait exit $?: $(tail -2 $d/out/load.txt | tr '\n' '|')";;
  delay)
    d=$O/delay; rm -rf $d/run $d/fx-a; mkdir -p $d/fx-a $d/run
    # a copy of test (a)'s capture: the check works in its folder (rust/)
    for f in children.jsonl plans.jsonl stops.jsonl windows.jsonl; do cp $HOME/.cache/fast-mi-e011/fixtures/play/a/$f $d/fx-a/; done
    env $ON3 FMI_ENGINE_BIN=$E12_ENGINE_BIN nice -n 10 $E12T/checks/release/delay-check $d/fx-a $d/run 1/2 1/4 1/21 0 > $d/delay-check.txt 2>&1
    log "delay R1/R3 exit $?: $(grep -E '^DELAY' $d/delay-check.txt | cut -c1-200 | tr '\n' '|')";;
  targets)
    d=$O/targets; mkdir -p $d
    nice -n 10 $E12T/release/targets_t2 $ARCH /var/tmp/fast-mi/e012-targets-t2/readings.tsv $d/t2.jsonl > $d/t2.md 2> $d/t2.err
    log "targets T2 exit $?: $(grep -iE 'pass|fail' $d/t2.md | head -6 | tr '\n' '|' | cut -c1-400)";;
  afford)
    d=$O/afford; mkdir -p $d
    source /var/tmp/fast-mi/scratchpad/afford/env.sh # ARCH, TESTED, PROBE (afford's inputs, read only)
    source $ENV
    export AFF=$E12T/checks/release/afford-check FMI_ENGINE_BIN=$E12_ENGINE_BIN FMI_ENGINE_CUTS=1 SPEEDRUN_REPORT_RESTART=1
    $E12/afford/gates.sh $d/gates > $d/gates.log 2>&1
    log "afford gates exit $?: $(cat $d/gates.log | tr '\n' '|' | cut -c1-300)"
    $E12/afford/boxes.sh $d/boxes > $d/boxes.log 2>&1
    log "afford boxes exit $?: $(cat $d/boxes.log | tr '\n' '|' | cut -c1-300)";;
  *) log "unknown stage $st";;
  esac
done
