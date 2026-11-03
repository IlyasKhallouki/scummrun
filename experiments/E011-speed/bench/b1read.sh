#!/bin/bash
# E011 round 2, B1's readings: one fresh ring(33) reading (j00089's start rows, G6's switches, 3 threads, the files fmi
# reads, no golden, no memo) by round 1's reader-scope, round 2's with --prep off and round 2's with --prep on,
# interleaved run by run so all three meet the same load; each under the heavy lock and the guard at nice 10. Prints
# the wall, the reader's CPU and peak memory from its report, and checks the packed and reached files' sha1.
# usage: E011_ENV=$HOME/.cache/fast-mi-e011/env-r2.sh b1read.sh OUT [ROUNDS]
set -u
source $HOME/.cache/fast-mi-e011/env.sh
[ -n "${E011_ENV:-}" ] && source "$E011_ENV" # a second env file after env.sh (round 2: env-r2.sh, its binaries)
OUT=$1; N=${2:-2}; mkdir -p $OUT
IN=$HOME/.cache/fast-mi-e011/profile-reader/inputs
for k in $(seq 1 $N); do
  for v in r1 r2 r2prep; do
    case $v in r1) bin=$R1_READER_SCOPE; x="";; r2) bin=$FMI_READER_SCOPE; x="";; r2prep) bin=$FMI_READER_SCOPE; x=--prep;; esac
    d=$OUT/$v-$k; rm -rf $d; mkdir -p $d
    flock /var/tmp/fast-mi/heavy.lock $HOME/.cache/fast-mi-e007/guard.sh $d/guard.log \
      $bin --rooms 28,33,35,38 --start-seed 1 --domain-goals none --goal owner:566=1 --name r33 --threads 3 --out $d \
      --out-dir $d --elsewhere --live-locals --split-limit \
      --switches value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost --reach --frame-costs \
      --start-rows $IN/ring33-j00089.jsonl $x
    p=$(sha1sum < $d/reading.tc | cut -c1-8); r=$(sha1sum < $d/reading.reached | cut -c1-8)
    echo "$v round $k: $(jq -c '{read_wall_s, read_cpu_s, peak_rss_mb}' $d/r33.json) packed $p reached $r; $(grep -o 'wall [0-9.]* s' $d/guard.log); load $(cut -d' ' -f1 /proc/loadavg)"
    rm -f $d/reading.tc $d/reading.actions.jsonl $d/reading.lists.jsonl $d/reading.idx
  done
done
