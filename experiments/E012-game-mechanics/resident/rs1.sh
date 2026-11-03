#!/bin/bash
# E012 resident, gate RS1 (E012 plan.md, "Amendment 2026-11-03, component resident"): G6's recorded inputs read by
# reader-scope (a child of `fmi read`, no memo, no --prep) and then in one fmi process with the switch
# FMI_READER_LIB=1, in RS1's order (ring(33) j00089, j00119, the refinement j00089 [103, 194] twice, ring(38) j00091).
# Each reading's five files have reader-scope's sha1, its reading.json is reader-scope's but for the measured fields
# (cmp_reading.py), and the files have G6's recorded sha1 where G6 has one. Each phase under the heavy lock and the
# guard; the big files go once summed.
# usage: rs1.sh OUT [ref] [inproc] [cmp]
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
OUT=$1; shift; parts=" ${*:-ref inproc cmp} "
mkdir -p $OUT
CMP=$here/../../E011-speed/rs/fmi-reading/ref/cmp_reading.py
R33=$READER_IN/ring33-j00089.jsonl; R119=$READER_IN/ring33-j00119.jsonl; R38=$READER_IN/ring38-j00091.jsonl
if [[ $parts == *" ref "* ]]; then
  D=$OUT/ref; rm -rf $D; mkdir -p $D
  { read_line r33-89 28,33,35,38 $R33 $D/r33-89
    read_line r33-119 28,33,35,38 $R119 $D/r33-119
    read_line ref89 28,33,35,38 $R33 $D/ref89 $REF89_ARGS
    read_line r38 33,38,83,85,96 $R38 $D/r38; } > $D/lines.txt
  echo "[$(date +%T)] ref: reader-scope, a child each"
  env -u FMI_READER_LIB -u FMI_MEMO -u FMI_READER_PREP $HEAVY $D/guard.log $FMI_BIN read $D/lines.txt
  grep '"name"' $D/guard.log > $D/read.jsonl; tail -1 $D/guard.log
  for t in r33-89 r33-119 ref89 r38; do sums_drop $D/$t; done
fi
if [[ $parts == *" inproc "* ]]; then
  D=$OUT/inproc; rm -rf $D; mkdir -p $D
  { read_line r33-89 28,33,35,38 $R33 $D/r33-89
    read_line r33-119 28,33,35,38 $R119 $D/r33-119
    read_line ref89 28,33,35,38 $R33 $D/ref89 $REF89_ARGS
    read_line ref89 28,33,35,38 $R33 $D/ref89-again $REF89_ARGS
    read_line r38 33,38,83,85,96 $R38 $D/r38; } > $D/lines.txt
  echo "[$(date +%T)] inproc: one fmi process, FMI_READER_LIB=1"
  env -u FMI_MEMO -u FMI_READER_PREP FMI_READER_LIB=1 $HEAVY $D/guard.log $FMI_BIN read $D/lines.txt
  grep '"name"' $D/guard.log > $D/read.jsonl; tail -1 $D/guard.log
  for t in r33-89 r33-119 ref89 ref89-again r38; do sums_drop $D/$t; done
fi
if [[ $parts == *" cmp "* ]]; then
  bad=0
  # G6's recorded sha1s: reading.tc and reading.reached (g6.sh), the line files (readio's reference)
  g6() { case $1 in
    r33-89) echo "a1ed4f5c 9b0c87b7 $(grep ' ref/r33/reading.actions.jsonl$' $READER_RIO/r33-files.sha1 | cut -c1-8) $(grep ' ref/r33/reading.lists.jsonl$' $READER_RIO/r33-files.sha1 | cut -c1-8)";;
    r33-119) echo "41eea348 9b0c87b7 - -";;
    ref89) echo "397fb6f1 ab30e2ef $(sha1sum < $READER_RIO/ref/ref89/reading.actions.jsonl | cut -c1-8) $(sha1sum < $READER_RIO/ref/ref89/reading.lists.jsonl | cut -c1-8)";;
    r38) echo "00dbb753 90b3d43a $(sha1sum < $READER_RIO/ref/r38/reading.actions.jsonl | cut -c1-8) $(sha1sum < $READER_RIO/ref/r38/reading.lists.jsonl | cut -c1-8)";;
  esac; }
  short() { awk '{printf "%s ", substr($1,1,8)}' $1/sums.txt | awk '{print $1, $2, $3, $4}'; }
  for p in r33-89:r33-89 r33-119:r33-119 ref89:ref89 ref89-again:ref89 r38:r38; do
    t=${p%%:*}; r=${p##*:}
    a=$OUT/ref/$r; b=$OUT/inproc/$t
    same=pass; cmp -s $a/sums.txt $b/sums.txt || { same=FAIL; bad=1; }
    want=$(g6 $r); got=$(short $b); g=pass
    for i in 1 2 3 4; do w=$(echo $want | cut -d' ' -f$i); x=$(echo $got | cut -d' ' -f$i); [ "$w" = - ] || [ "$w" = "$x" ] || { g=FAIL; bad=1; }; done
    $PY $CMP $a $b > $b/cmp.json; j=$([ $? = 0 ] && echo pass || { bad=1; echo FAIL; })
    echo "$t: five files vs reader-scope $same; tc/reached/actions/lists $got vs G6 ($want) $g; reading.json $j"
  done
  echo "RS1 $([ $bad = 0 ] && echo PASS || echo FAIL)"
fi
