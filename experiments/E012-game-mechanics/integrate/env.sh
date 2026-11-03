# E012 integration: E011 round 2's tools with the no-plan proofs (env-r2d.sh: ON3/OFF3, taskcore, round 2's own fmi in
# R2_FMI_BIN), then branch e012's binaries (integrate/build.sh's target) and end's engine (patches 0001-0005).
# E12ON / E12OFF: every E012 switch on or unset. Source it as E011_ENV for E011's gate scripts.
source $HOME/.cache/fast-mi-e011/env.sh
source $HOME/.cache/fast-mi-e011/env-r2d.sh
E12T=${E12T:-/var/tmp/fast-mi/target-e012}
export WT11=/var/tmp/fast-mi/wt-e012
export FMI_BIN=$E12T/release/fmi
export FMI_ENGINE_TOOL=$E12T/release/fmi-engine
export FMI_READING_TOOL=$E12T/release/fmi-reading
export FMI_PLAN_TOOL=$E12T/release/fmi-plan
export FMI_PLAY_LEAN=$E12T/checks/release/play-lean
export FMI_PLAY_WIRE=$E12T/checks/release/play-wire
export FMI_READER_SCOPE=$E12T/scumm/release/reader-scope
export READER_PORT_E011=$E12T/scumm/release/reader-port
export READER_SEQ_E011=$E12T/scumm/release/reader-seq
# round 2's merged build (the no-plan proofs), the "before" of E012's measurement
export R2D_FMI_BIN=/var/tmp/fast-mi/target-e011-r2/release/fmi
export R2D_READER_SCOPE=/var/tmp/fast-mi/target-e011-r2/scumm/release/reader-scope
export R2_ENGINE_BIN=/var/tmp/fast-mi/svm-e011-r2int/build/scummvm/scummvm
export E12_ENGINE_BIN=/var/tmp/fast-mi/svm-e012-end/build/scummvm/scummvm
export FMI_ENGINE_BIN=$E12_ENGINE_BIN
E12ON="FMI_WAIT=1 FMI_TARGET_PER_KEY=1 FMI_DELAY_FORK=1 FMI_AFFORD=1 FMI_SELECT=progress SPEEDRUN_REPORT_RESTART=1 FMI_BOXES=1 FMI_READER_LIB=1"
E12OFF="FMI_WAIT= FMI_TARGET_PER_KEY= FMI_DELAY_FORK= FMI_AFFORD= FMI_SELECT= SPEEDRUN_REPORT_RESTART= FMI_BOXES= FMI_READER_LIB="
export E12ON E12OFF E12T
