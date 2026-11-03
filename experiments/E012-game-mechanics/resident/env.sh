# E012 resident: E011's tools (env.sh), then this branch's binaries (build.sh's target).
source $HOME/.cache/fast-mi-e011/env.sh
T=${RESIDENT_TARGET:-/var/tmp/fast-mi/target-e012-resident}
export FMI_BIN=$T/release/fmi
export FMI_READER_SCOPE=$T/scumm/release/reader-scope
export READER_IN=$HOME/.cache/fast-mi-e011/profile-reader/inputs
export READER_RIO=$HOME/.cache/fast-mi-e011/readio
export KITCHEN_READINGS=/var/tmp/fast-mi/e011-archives/kitchen/readings
# each phase under the heavy lock and the guard; NOLOCK=1: the guard alone (a step under 1 GB, timing not taken)
if [ "${NOLOCK:-0}" = 1 ]; then export HEAVY="$HOME/.cache/fast-mi-e007/guard.sh"
else export HEAVY="flock /var/tmp/fast-mi/heavy.lock $HOME/.cache/fast-mi-e007/guard.sh"; fi
# reader-scope's arguments of E009's readings as fmi asks them (fmi-reading cache.rs ReadSpec::args):
# NAME ROOMS ROWS OUTDIR [more]
read_line() {
  echo "--rooms $2 --start-seed 1 --domain-goals none --goal owner:566=1 --name $1 --threads ${READ_THREADS:-3}" \
       "--out-dir $4 --elsewhere --live-locals --split-limit" \
       "--switches value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost --reach --frame-costs --start-rows $3" \
       "${@:5}"
}
REF89_ARGS="--reach-also $READER_IN/ref-j00089/refine-01.start.txt --reach-also $READER_IN/ref-j00089/refine-02.start.txt --fact-vars 103,194"
# the five files a reading is made of, sha1 per file, into DIR/sums.txt; then the big files go (DIR)
sums_drop() {
  ( cd $1 && sha1sum reading.tc reading.reached reading.actions.jsonl reading.lists.jsonl reading.idx > sums.txt ) &&
    rm -f $1/reading.tc $1/reading.actions.jsonl $1/reading.lists.jsonl $1/reading.idx
}
export PY=/home/ilyask/projects/fast-mi/.venv/bin/python
