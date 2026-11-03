#!/bin/bash
# E011 G6 on the integrated reader (experiments/E011-speed/plan.md, "Exactness gates"): reader-scope built from e011
# (readio's --out-dir, readcore's core, --memo), each reading under the heavy lock and the guard at nice 10.
#   * the kitchen golden: reader-port read kitchen --packed, sha1 279b7a24, 0 differences against golden.py;
#   * ring(38) j00091, ring(33) j00089, the refinement j00089 [103,194], memo off: the packed file, the golden body and
#     builder line and the reached file have E009's binary's sha1 (readcore's runs), the four reading files
#     reading_from_golden's (readio's reference: cmp_reading.py where its files are kept, else their sha1);
#   * memo on: the refinement j00089 twice from an empty store (stores, then takes), the fresh ring(33) j00089 into a
#     second store, then j00119 from it: each the same sha1s.
# usage: g6.sh OUT
#   G6_ARGS: more reader-scope switches (round 2: --prep); G6_THREADS: the readings' threads (3); G6_NOGOLDEN=1: no
#   --golden (the files fmi reads, as fmi asks them; the golden body and builder line are then not compared)
#   G6_PARTS: which checks (default "kitchen off memo89 memo33": the golden; memo off; the refinement pair; the fresh
#   ring(33) pair)
set -u
source $HOME/.cache/fast-mi-e011/env.sh
[ -n "${E011_ENV:-}" ] && source "$E011_ENV" # a second env file after env.sh (round 2: env-r2.sh, its binaries)
here=$(cd "$(dirname "$0")" && pwd)
PY=$here/../../../.venv/bin/python
BIN=$FMI_READER_SCOPE
IN=$HOME/.cache/fast-mi-e011/profile-reader/inputs
ROWS38=$HOME/.cache/fast-mi-e011/profile-planner/readings/r38/start.jsonl  # = IN/ring38-j00091.jsonl, the path readio's reference names
ROWS33=$HOME/.cache/fast-mi-e011/profile-planner/readings/r33/start.jsonl  # = IN/ring33-j00089.jsonl
RIO=$HOME/.cache/fast-mi-e011/readio
CMP=$WT11/experiments/E011-speed/rs/fmi-reading/ref/cmp_reading.py
OUT=$1; mkdir -p $OUT
XARGS=${G6_ARGS:-}; THREADS=${G6_THREADS:-3}; NOGOLD=${G6_NOGOLDEN:-0}
M=$OUT/memo-a; M2=$OUT/memo-b; rm -rf $M $M2
REF89="--fact-vars 103,194 --reach-also $IN/ref-j00089/refine-01.start.txt --reach-also $IN/ref-j00089/refine-02.start.txt"
bad=0
read1() { # TAG ROOMS ROWS WANT [reader-scope args]: one reading, its sums against WANT (packed body builder reached)
  local tag=$1 rooms=$2 rows=$3 want=$4; shift 4
  local d=$OUT/$tag; rm -rf $d; mkdir -p $d
  local gold="--golden $d/$tag.golden.jsonl.gz"
  [ $NOGOLD = 1 ] && { gold=""; want="$(echo $want | awk '{print $1, "-", "-", $4}')"; }
  TIMEFORMAT="%R %U %S"
  { time flock /var/tmp/fast-mi/heavy.lock $HOME/.cache/fast-mi-e007/guard.sh $d/guard.log \
    $BIN --rooms $rooms --start-seed 1 --domain-goals none --goal owner:566=1 \
    --name $tag --threads $THREADS --out $d --out-dir $d $gold --elsewhere --live-locals \
    --split-limit --switches value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost --reach --frame-costs \
    --start-rows $rows "$@" $XARGS ; } 2> $d/time
  local p b t r
  p=$(sha1sum < $d/reading.tc | cut -c1-8); r=$(sha1sum < $d/reading.reached | cut -c1-8)
  if [ $NOGOLD = 1 ]; then b=-; t=-; else
    b=$(zcat $d/$tag.golden.jsonl.gz | sed '1d;$d' | sha1sum | cut -c1-8)
    t=$(zcat $d/$tag.golden.jsonl.gz | tail -1 | sha1sum | cut -c1-8)
  fi
  local got="$p $b $t $r" ok=pass
  [ "$got" = "$want" ] || { ok=FAIL; bad=1; }
  local memo; memo=$($PY -c "import json,sys; r=json.load(open('$d/$tag.json')); m=r.get('memo'); print('peak %s MB, ' % r.get('peak_rss_mb') + ('memo taken %s stored %s' % (m.get('taken'), m.get('stored')) if m else 'memo off'))")
  echo "$tag: $ok packed/body/builder/reached $got (want $want); wall/user/sys $(cat $d/time); $memo"
  tail -1 $d/guard.log
}
files() { # TAG REFDIR|SHA1FILE: the four files against readio's reference
  local d=$OUT/$1
  if [ -d "$2" ]; then
    $PY $CMP $2 $d > $d/cmp.json; local rc=$?
    [ $rc = 0 ] || bad=1
    echo "$1 files vs reading_from_golden (cmp_reading.py): $([ $rc = 0 ] && echo pass || echo FAIL)"
  else
    local ok=pass
    for f in reading.tc reading.actions.jsonl reading.lists.jsonl reading.reached; do
      w=$(grep " ref/r33/$f\$" $2 | cut -c1-40); g=$(sha1sum < $d/$f | cut -c1-40)
      [ "$w" = "$g" ] || { ok=FAIL; bad=1; echo "  $f $g want $w"; }
    done
    echo "$1 files vs reading_from_golden's sha1 ($2): $ok"
  fi
}
drop() { rm -f $OUT/$1/reading.tc $OUT/$1/*.golden.jsonl.gz $OUT/$1/reading.actions.jsonl $OUT/$1/reading.lists.jsonl \
           $OUT/$1/reading.idx; }
part() { [[ " ${G6_PARTS:-kitchen off memo89 memo33} " == *" $1 "* ]]; }  # G6_PARTS: the checks to run

if part kitchen; then
echo "[$(date +%T)] kitchen golden (reader-port)"
d=$OUT/kitchen; rm -rf $d; mkdir -p $d
TIMEFORMAT="%R %U %S"
{ time flock /var/tmp/fast-mi/heavy.lock $HOME/.cache/fast-mi-e007/guard.sh $d/guard.log \
  $READER_PORT_E011 read kitchen --packed $d/kitchen.tc ; } 2> $d/time
s=$(sha1sum < $d/kitchen.tc | cut -c1-8)
grep -iE "compared|differ" $d/guard.log | tail -3
[ "$s" = 279b7a24 ] || bad=1
echo "kitchen packed $s (want 279b7a24): $([ "$s" = 279b7a24 ] && echo pass || echo FAIL); wall/user/sys $(cat $d/time)"
fi

if part off; then
echo "[$(date +%T)] memo off"
read1 r38 33,38,83,85,96 $ROWS38 "00dbb753 f8caa72f fb177a89 90b3d43a"
files r38 $RIO/ref/r38; drop r38
read1 r33 28,33,35,38 $IN/ring33-j00089.jsonl "a1ed4f5c d4dbc9fe 59faac2f 9b0c87b7"
files r33 $RIO/r33-files.sha1; drop r33
read1 ref89 28,33,35,38 $ROWS33 "397fb6f1 95f02638 494c1368 ab30e2ef" $REF89
files ref89 $RIO/ref/ref89; drop ref89
fi

echo "[$(date +%T)] memo on"
if part memo89; then
read1 ref89-m1 28,33,35,38 $IN/ring33-j00089.jsonl "397fb6f1 95f02638 494c1368 ab30e2ef" $REF89 --memo $M; drop ref89-m1
read1 ref89-m2 28,33,35,38 $IN/ring33-j00089.jsonl "397fb6f1 95f02638 494c1368 ab30e2ef" $REF89 --memo $M; drop ref89-m2
fi
if part memo33; then
read1 r33-m1 28,33,35,38 $IN/ring33-j00089.jsonl "a1ed4f5c d4dbc9fe 59faac2f 9b0c87b7" --memo $M2; drop r33-m1
read1 r33-119-m2 28,33,35,38 $IN/ring33-j00119.jsonl "41eea348 dc7364c8 59faac2f 9b0c87b7" --memo $M2; drop r33-119-m2
fi
rm -rf $M $M2
echo "[$(date +%T)] G6 $([ $bad = 0 ] && echo PASS || echo FAIL)"
exit $bad
