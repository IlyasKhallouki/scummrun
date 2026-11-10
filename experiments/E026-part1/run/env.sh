# E026 (plan.md Part 1, I1): the settings of E026's checks and runs, for both games, written out: it sources nothing.
# ALLON is E024's ALLON on e016 resolved, the last value of each switch (~/.cache/fast-mi-e026/e016-survey/allon.txt;
# the env of mi-checks-now's G0); E26 holds E026's switches, each added with its gate (plan.md Parts 2-4), on for both
# games; the engine is svm-e024; each game's files are named (FMI_GAME_*), so no run relies on host.rs's default game.
#   ON          the env under test, $ALLON $E26: every run and every verify from boot of a check uses it
#   ALLOFF      E024's ALLOFF (`fmi check tries`' env, as mi-check.sh ran it)
#   MI, DEMO    each game's FMI_GAME_ID, FMI_GAME_DIR and FMI_GAME_SCRIPTS
#   E26T        the build: release/fmi, fmi-checks' binaries (ladder, keyfacts, stall-report, e016) and
#               scumm/release/reader-scope; a check may name a frozen copy (E26T=DIR in that layout)
#   RUNGS, KEY  the ladder's rungs files (fmi-checks/rungs); E001's key.json (never committed)
#   O26         the outputs (~/.cache/fast-mi-e026/build/<stage>/); NICE, GUARD, FLOOR_GB (2 GB on the output disk)
# Source it from the scripts beside it (b3.sh, smoke.sh, k1.sh, g3.sh, g4.sh, mi-check.sh, probe.sh, watch.sh).
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
RUN26=$ROOT/experiments/E026-part1/run
# E011 round 2 (ON3)
ALLON="FMI_ENGINE_CUTS=1 FMI_PLAY_BATCH=1 FMI_HIT_CACHE=1 FMI_STONE_KEEP=1 FMI_PLAN_FAST=1 FMI_PLAN_LAMA=rust"
ALLON="$ALLON FMI_READER_PREP=1 FMI_IDLE_CHANGES=1 FMI_PLAYER_DONE=1 FMI_TRY_MEMO=1 FMI_PLANNER_KEEP=1 FMI_PLAY_KEEP=1"
ALLON="$ALLON FMI_PLAN_DEAD=1 FMI_PLAN_DEEPEN=60"
# E012 (E12ON); E014's mechanisms (E14M); E013 (E13ON)
ALLON="$ALLON FMI_WAIT=1 FMI_TARGET_PER_KEY=1 FMI_DELAY_FORK=1 FMI_AFFORD=1 FMI_SELECT=progress SPEEDRUN_REPORT_RESTART=1"
ALLON="$ALLON FMI_BOXES=1 FMI_READER_LIB=1"
ALLON="$ALLON SPEEDRUN_IDLE=handler SPEEDRUN_END_SITE=1 FMI_GOAL_END=1 FMI_READ_EVENTS=1 FMI_EGO_VALUES=1 FMI_MERGE_DEDUP=1"
ALLON="$ALLON FMI_READ_CUT=1 FMI_STAG_SWEEP=1 FMI_VERBS_VAR_ON=1 FMI_NOVEL_VERBS=1 FMI_NOVEL_GUIDE=1"
ALLON="$ALLON SPEEDRUN_INPUT=1 FMI_REFUSE=1 FMI_STALLS=1 FMI_FREE=0.25 FMI_FREE_SHIFT=1"
# E014 A4-A8, scope growth by room (E016 amendment (8))
ALLON="$ALLON FMI_READ_BOUND=300 FMI_READ_BOUND_BG=0 FMI_READ_WIDEN=1 FMI_STAG_CELLS=1 FMI_ARGS_LATER=1 FMI_READ_SOUND=1"
ALLON="$ALLON FMI_OFFER_SCREEN=1 FMI_GOAL_FACT=1 FMI_SCOPE_GROW=room"
# E016 (E16), with E017's plan limits (FMI_PLAN_TIME 300, FMI_PLAN_DEEPEN 60 above)
ALLON="$ALLON FMI_REALIZE=1 FMI_NOGOOD=1 FMI_FREE_T=archive FMI_GOAL_SITE=verb FMI_FRONTIER=1 FMI_NOVEL_PLACE=1"
ALLON="$ALLON FMI_SELECT_PLACE=1 FMI_PLAN_TIME=300 FMI_REACH_ZEROS=1 FMI_PLANNER_LOCK=reading"
# E018-E021
ALLON="$ALLON FMI_TARGET_PLACE=1 FMI_REVEAL=1 FMI_PLACE_RARITY=1 FMI_FREE_SAME=1 FMI_SKIP_SAME=1 FMI_FREE_NO_STOPS=1"
ALLON="$ALLON FMI_REALIZE_HINTS=1 FMI_READ_HELD=1 FMI_READ_SHARE=1 FMI_IMUSE_TICKS=1 FMI_DIFFERS_DEAD=1"
ALLON="$ALLON FMI_SKIP_SOUNDRESULT=1 FMI_READ_MAX_RSS_GB=40 FMI_TEMP_SAVES=1"
# E022, amendments (1)-(28)
ALLON="$ALLON FMI_PLACE_CTX=1 FMI_REFINE_WIDEN=3 FMI_SLOT_LOCALS=1 FMI_PLAN_FOCUS=1 FMI_READ_INPUT_FACTS=1"
ALLON="$ALLON FMI_FRONTIER_ATOMS=1 FMI_READ_EXITS=1 FMI_READ_PARENTS=1 FMI_GOAL_UNPLANNED=1 FMI_READ_INVENTORY_SUMMARY=1"
ALLON="$ALLON FMI_FRONTIER_FRESH=0 FMI_SELECT_ADVANCED=1 FMI_READ_OBJ_ROOM=1 FMI_SELECT_CONTINUE=1 FMI_READ_MERGE=1"
ALLON="$ALLON FMI_READ_B_ARGS=1 FMI_READ_INV_B=1 FMI_READ_RENAME=1 FMI_BAN_TRY=1 FMI_READ_TRIM=1 FMI_REFINE_CHECKS=1"
ALLON="$ALLON FMI_REALIZE_QUEUED=1 FMI_READ_B_SWAP=1 FMI_READ_QUEUE=1 FMI_READ_STRING_SUMMARY=1"
ALLON="$ALLON FMI_READ_SWITCHES=value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost,wait_both"
ALLON="$ALLON FMI_SELECT_FURTHEST=1"
# E024: the scripts' files in each engine's memory (patch 0010)
ALLON="$ALLON FMI_MEM_FILES=1"
# E026's switches (plan.md Parts 2-4), each added with its gate
E26=""
ON="$ALLON $E26"
ALLOFF="FMI_ENGINE_CUTS=1 FMI_PLAY_BATCH=1 FMI_HIT_CACHE=1 FMI_STONE_KEEP=1 FMI_PLAN_FAST=1 FMI_PLAN_LAMA=rust"
ALLOFF="$ALLOFF FMI_READER_PREP=1 FMI_IDLE_CHANGES=1 FMI_PLAYER_DONE=1 FMI_TRY_MEMO=1 FMI_PLANNER_KEEP=1 FMI_PLAY_KEEP=1"
ALLOFF="$ALLOFF FMI_PLAN_DEAD=1 FMI_PLAN_DEEPEN=120 FMI_WAIT= FMI_TARGET_PER_KEY= FMI_DELAY_FORK= FMI_AFFORD= FMI_SELECT="
ALLOFF="$ALLOFF SPEEDRUN_REPORT_RESTART= FMI_BOXES= FMI_READER_LIB= SPEEDRUN_IDLE= SPEEDRUN_END_SITE= FMI_GOAL_END="
ALLOFF="$ALLOFF FMI_READ_EVENTS= FMI_EGO_VALUES= FMI_MERGE_DEDUP= FMI_READ_CUT= FMI_STAG_SWEEP= FMI_VERBS_VAR_ON="
ALLOFF="$ALLOFF FMI_NOVEL_VERBS= FMI_NOVEL_GUIDE= FMI_GAME_ID= FMI_GAME_DIR= FMI_GAME_SCRIPTS= SPEEDRUN_INPUT= FMI_REFUSE="
ALLOFF="$ALLOFF FMI_REFUSE_CHECK= FMI_REFUSE_DEDUPE= FMI_STALLS= FMI_FREE= FMI_FREE_SHIFT= FMI_READ_BOUND= FMI_READ_BOUND_BG="
ALLOFF="$ALLOFF FMI_SCOPE_GROW= FMI_READ_WIDEN= FMI_STAG_CELLS= FMI_ARGS_LATER= FMI_READ_SOUND= FMI_OFFER_SCREEN="
ALLOFF="$ALLOFF FMI_GOAL_FACT= FMI_REALIZE= FMI_NOGOOD= FMI_FREE_T= FMI_GOAL_SITE= FMI_FRONTIER= FMI_NOVEL_PLACE="
ALLOFF="$ALLOFF FMI_SELECT_PLACE= FMI_PLAN_TIME= FMI_REACH_ZEROS= FMI_PLANNER_LOCK="
export ALLON E26 ON ALLOFF ROOT RUN26
# the games: Monkey Island's DOS CD files are the worktree's game link (never committed), the demo's fixed folders
MI="FMI_GAME_ID=monkey FMI_GAME_DIR=$ROOT/game/dos-cd FMI_GAME_SCRIPTS=$ROOT/data/scripts-dos-cd"
DEMO="FMI_GAME_ID=atlantis FMI_GAME_DIR=/var/tmp/fast-mi/games/fate-play-demo FMI_GAME_SCRIPTS=/var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo"
export MI DEMO
# the engine (patches 0001-0010) and the build
export FMI_ENGINE_BIN=/var/tmp/fast-mi/svm-e024/build/scummvm/scummvm
export E26T=${E26T:-/var/tmp/fast-mi/target-e026}
export FMI_BIN=$E26T/release/fmi CHK=$E26T/release
# reader-scope of the same tree (named by every wired fmi; FMI_READER_LIB=1 reads in fmi itself); the older builds the
# driver records in stage.json (plan_env), at E024's values
export FMI_READER_SCOPE=$E26T/scumm/release/reader-scope
export DOWNWARD_DIR=/home/ilyask/projects/fast-mi/third_party/downward
export FAST_MI_SCUMMVM_BIN=$HOME/.cache/fast-mi/svm-w3b/build/scummvm/scummvm
export TASKCORE_BIN=$HOME/.cache/fast-mi-walk/target/release/taskcore
export READER_SCOPE_BIN=$HOME/.cache/fast-mi-e008/target-scumm/release/reader-scope
export RUNGS=$ROOT/experiments/E011-speed/rs/fmi-checks/rungs
export KEY=${KEY:-/home/ilyask/projects/scummrun/experiments/E001-goals-from-scripts/key.json}
export BENCH=$ROOT/experiments/E011-speed/bench
export O26=${O26:-$HOME/.cache/fast-mi-e026/build}
export TMPDIR=/var/tmp/fast-mi/e026-tmp
mkdir -p $TMPDIR
export NICE=${NICE:-5} GUARD=$RUN26/guard.sh FLOOR_GB=${FLOOR_GB:-2}
# each game's files are there (a missing link voided a check: E016 plan.md:464-468), the binaries are built
game_ok() {
  local d
  for d in $ROOT/game/dos-cd $ROOT/data/scripts-dos-cd /var/tmp/fast-mi/games/fate-play-demo \
    /var/tmp/fast-mi/e014-root/data/scripts-fate-play-demo; do
    [ -d "$d" ] || { echo "env.sh: missing $d" >&2; return 1; }
  done
  [ -e $ROOT/game/dos-cd/MONKEY.000 ] || { echo "env.sh: no MONKEY.000 in $ROOT/game/dos-cd" >&2; return 1; }
  for d in $FMI_ENGINE_BIN $FMI_BIN $FMI_READER_SCOPE; do
    [ -x "$d" ] || { echo "env.sh: missing $d" >&2; return 1; }
  done
}
# GB free on DIR's disk, and the floor: under FLOOR_GB on the output disk a check stops
free_gb() { df -P -BG "$1" | awk 'NR == 2 { sub("G", "", $4); print $4 }'; }
disk_ok() {
  local f; f=$(free_gb "$1")
  [ "$f" -ge "$FLOOR_GB" ] || { echo "env.sh: $1's disk has $f GB free, under $FLOOR_GB GB" >&2; return 1; }
}
# a timed gate or a run starts on an idle laptop (plan.md C5, G9): MemAvailable >= 10 GB, load < 2
idle_ok() {
  local m l; m=$(awk '/MemAvailable/ { print $2 }' /proc/meminfo); l=$(cut -d' ' -f1 /proc/loadavg)
  awk -v m=$m -v l=$l 'BEGIN { exit !(m >= 10 * 1048576 && l < 2) }' || {
    echo "env.sh: not idle: MemAvailable $((m / 1024)) MB, load $l (needs >= 10 GB and < 2)" >&2; return 1; }
}
# the laptop stays awake through a long check or a run (hypridle suspends otherwise)
INHIBIT="systemd-inhibit --what=idle:sleep --who=E026 --why=check --mode=block"
export INHIBIT
