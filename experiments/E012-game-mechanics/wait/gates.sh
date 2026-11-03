#!/bin/bash
# E012 wait's gates W1-W4 (plan.md, the `wait` row and the amendment "the `wait` component"): `fmi check wait` from the
# kitchen archive of E011's check (read only), each entry rebuilt from boot from its movie.
#   room28   entries 203, 205, 225 (W2's) with every check, T from the kept reading 28-33-41-j00065 (copied to DATA)
#   room28b  the other room-28 entries, the same reading (W1, W4 and the differential checks on more states)
#   e4       entry 4 (room 38, W3), T from a fresh reading of its job's scope 33, 38, 83, 85, 96 from its state
#   w4       W4's timing alone: every entry above, the wait 10 times with no check, under the heavy lock (the reading
#            of e4 made by the e4 group)
# usage: ENV=env.sh gates.sh OUT [room28|room28b|e4]...   (ENV: FMI_BIN = this branch's fmi, FMI_ENGINE_BIN = round 2's
# engine, FMI_READER_SCOPE; ON3 the switches the try runs with). Each check runs under the guard at nice 10; its record
# is OUT/GROUP.json, its log OUT/GROUP.log.
set -u
source "${ENV:?an env file}"
OUT=$1; shift
ARCH=/var/tmp/fast-mi/e011-archives/kitchen
DATA=${WAIT_DATA:-/var/tmp/fast-mi/e012-wait} # WAIT_DATA: another data folder (the integration's)
GUARD=$HOME/.cache/fast-mi-e007/guard.sh
mkdir -p $OUT
R28=$DATA/readings/28-33-41-j00065
if [ ! -f $R28/reading.json ]; then
  mkdir -p $R28
  for f in reading.json reading.tc reading.actions.jsonl reading.lists.jsonl reading.reached reading.idx start.jsonl; do
    cp --reflink=auto $ARCH/readings/28-33-41-j00065/$f $R28/
  done
fi
checks="--verify --view-check --due-check --idle-check --repeat 5"
for g in "$@"; do
  echo "$(date +%T) $g: load $(cut -d' ' -f1-3 /proc/loadavg)" >> $OUT/load.txt
  case $g in
    room28) $GUARD $OUT/$g.log env $ON3 $FMI_BIN check wait $ARCH --entries 203,205,225 --reading $R28/reading.json \
              --out $DATA/run/$g $checks > /dev/null;;
    room28b) $GUARD $OUT/$g.log env $ON3 $FMI_BIN check wait $ARCH --entries 204,206,207,208,209,220,221,222,223,224 \
              --reading $R28/reading.json --out $DATA/run/$g $checks > /dev/null;;
    e4) rm -rf $DATA/readings/33-38-83-85-96-e4
        $GUARD $OUT/$g.log env $ON3 $FMI_BIN check wait $ARCH --entries 4 --read-scope 33,38,83,85,96 \
              --read-dir $DATA/readings/33-38-83-85-96-e4 --out $DATA/run/$g $checks > /dev/null;;
    w4) (flock 9; echo "$(date +%T) w4 has the lock: load $(cut -d' ' -f1-3 /proc/loadavg)" >> $OUT/load.txt
        $GUARD $OUT/$g-28.log env $ON3 $FMI_BIN check wait $ARCH --entries 203,205,225,204,206,207,208,209,220,221,222,223,224 \
              --reading $R28/reading.json --out $DATA/run/$g-28 --repeat 10 > /dev/null
        $GUARD $OUT/$g-38.log env $ON3 $FMI_BIN check wait $ARCH --entries 4 \
              --reading $DATA/readings/33-38-83-85-96-e4/reading.json --out $DATA/run/$g-38 --repeat 10 > /dev/null) 9>/var/tmp/fast-mi/heavy.lock
        cp $DATA/run/$g-28/wait-check.json $OUT/$g-28.json; cp $DATA/run/$g-38/wait-check.json $OUT/$g-38.json;;
    *) echo "unknown group $g"; exit 2;;
  esac
  [ $g = w4 ] || cp $DATA/run/$g/wait-check.json $OUT/$g.json
  echo "$(date +%T) $g done: load $(cut -d' ' -f1-3 /proc/loadavg)" >> $OUT/load.txt
done
