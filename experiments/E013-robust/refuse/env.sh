# E013 component refuse's gates: E012's integrate/env.sh (round 2's tools, ON3, E12ON) with refuse's build (E12T),
# worktree and engine (patches 0001-0005, 0006-e014 and 0007). Source it as ENV for refuse/rerun.sh.
export E12T=${E12T:-/var/tmp/fast-mi/target-e013-refuse}
source /var/tmp/fast-mi/wt-e013-refuse/experiments/E012-game-mechanics/integrate/env.sh
export WT11=/var/tmp/fast-mi/wt-e013-refuse
# the engine with patch 0007 (experiments/E013-robust/engine/build.sh)
export E13_ENGINE_BIN=${E13_ENGINE_BIN:-/home/ilyask/.cache/fast-mi-e013/svm-e013-refuse/build/scummvm/scummvm}
export FMI_ENGINE_BIN=$E13_ENGINE_BIN
# the reader binaries: the scumm workspace is E014's head (merged into e013 unchanged); its build
export FMI_READER_SCOPE=/var/tmp/fast-mi/target-e014/scumm/release/reader-scope
export READER_PORT_E011=/var/tmp/fast-mi/target-e014/scumm/release/reader-port
export READER_SEQ_E011=/var/tmp/fast-mi/target-e014/scumm/release/reader-seq
# E013's refuse switch (FMI_REFUSE=1 passes SPEEDRUN_INPUT=1 to every engine); E13RCHECK adds gate B4's check
E13R="FMI_REFUSE=1 SPEEDRUN_INPUT=1"
E13ROFF="FMI_REFUSE= SPEEDRUN_INPUT= FMI_REFUSE_CHECK= FMI_REFUSE_DEDUPE="
E13RCHECK="$E13R FMI_REFUSE_CHECK=1"
export E13R E13ROFF E13RCHECK
