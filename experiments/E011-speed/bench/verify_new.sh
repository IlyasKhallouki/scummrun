#!/bin/bash
# E011 G11's second part (experiments/E011-speed/plan.md): every new entry of a sequence's tries verified from boot in a
# fresh lean engine (fmi-engine verify: replay.verify's port, the movie from boot against every checkpoint: the
# root's, the try's entry's, the new entry's). Each entry becomes a two-entry archive (the root and the entry with its
# whole movie) made by mkverify.py. Run after the benchmark.
# usage: verify_new.sh SEQ OUT
set -u
source $HOME/.cache/fast-mi-e011/env.sh
[ -n "${E011_ENV:-}" ] && source "$E011_ENV" # a second env file after env.sh (round 2: env-r2.sh, its binaries)
here=$(cd "$(dirname "$0")" && pwd)
PY=$here/../../../.venv/bin/python
SEQ=$1; OUT=$2; mkdir -p $OUT
$PY $here/mkverify.py $SEQ $OUT/arch || exit 1
n=0; bad=0
for a in $OUT/arch/*/; do
  name=$(basename $a)
  r=$(nice -n 10 $FMI_ENGINE_TOOL verify $OUT/run-$name $a 1 --lean 2> $OUT/$name.err)
  rc=$?; n=$((n + 1)); rm -rf $OUT/run-$name
  d=$(echo "$r" | grep -o '"differ": [0-9]*' | awk '{print $2}')
  if [ $rc != 0 ] || [ "${d:-1}" != 0 ]; then bad=$((bad + 1)); echo "$name: DIFFERS rc $rc $r"; head -3 $OUT/$name.err; fi
  echo "$name $r" >> $OUT/verify.txt
done
echo "verify $(basename $SEQ): $n new entries from boot, $bad with differences"
[ $bad = 0 ]
