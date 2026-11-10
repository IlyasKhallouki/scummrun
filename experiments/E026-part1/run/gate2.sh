#!/bin/bash
# E026 G2 (plan.md C2, a stopped reading stops), with FMI_BIN (the build under test) and BASE (the build before C2):
#  (a) K1's room-28 try j00001 (scope [28], entry 15, frame 13,600; mi-checks-now's K1 archive) with ON (wait_both) and
#      FMI_READ_BOUND=30, in a warm `fmi try @FIFO` that stays up after the try: the try's wall, and from its end every
#      0.5 s for HOLD s the process's reading threads (reader-*, the bounded reading's) and its CPU (utime + stime);
#      gate: the try ends <= bound + 10 s and its reading threads are gone by then;
#  (b) b3's j00006 (scope [28,33,35,38]) cold and alone with FMI_READ_SWITCHES unset (not bounded): its reading's files,
#      kept, compared file by file across the two builds; gate: byte-identical.
# A measurement, no change to the system. One line per part and build in OUT/summary.txt.
# usage: [BASE=FMI] [BASE_SCOPE=READER_SCOPE] [HOLD=s] gate2.sh OUT [a|b]...
set -u
here=$(cd "$(dirname "$0")" && pwd)
source $here/env.sh
game_ok || exit 1
OUT=$1; shift
BASE=${BASE:-$O26/stage2/base/fmi}
BASE_SCOPE=${BASE_SCOPE:-$O26/stage2/base/reader-scope}
HOLD=${HOLD:-60}
K1A=$HOME/.cache/fast-mi-e026/mi-checks-now/k1/archive
B3S=/var/tmp/fast-mi/e011-r2int/b3f/r2-fast
mkdir -p $OUT
disk_ok $OUT || exit 1
idle_ok || exit 1
calc() { awk "BEGIN { printf \"%.1f\", $1 }"; }
# the process's threads named reader-* or "bounded reading", and its CPU s (fields 14 + 15 of /proc/PID/stat)
threads() { grep -lE '^(reader-|bounded reading)' /proc/$1/task/*/comm 2>/dev/null | wc -l; }
cpu() { awk -v t=$(getconf CLK_TCK) '{ printf "%.2f", ($14 + $15) / t }' /proc/$1/stat 2>/dev/null; }
for part in ${@:-a b}; do
  for name in base c2; do
    bin=$FMI_BIN; scope=$FMI_READER_SCOPE
    [ $name = base ] && { bin=$BASE; scope=$BASE_SCOPE; }
    case $part in
      a)
        P=$OUT/a-$name; rm -rf $P; mkdir -p $P/jobs/j00001 $P/tmp
        ln -s $K1A/entries $P/entries; ln -s $K1A/entries.jsonl $P/entries.jsonl
        sed "s#$K1A#$P#g" $K1A/jobs/j00001/job.json > $P/jobs/j00001/job.json
        FIFO=$P/jobs.fifo; mkfifo $FIFO
        env $ON $MI FMI_READER_SCOPE=$scope FMI_READ_BOUND=30 FMI_READING_CACHE=$P/rcache FMI_MEMO= FMI_PLAN_CACHE=$P/plans FMI_POOL_DIR=$P/pool \
          TMPDIR=$P/tmp E008_DISK_PATH=$OUT E008_FLOOR_GB=$FLOOR_GB nice -n $NICE $bin try @$FIFO --search fd --tail > $P/fmi.log 2>&1 &
        gpid=$!
        exec 7>$FIFO
        sleep 1
        # the warm process by its name and /proc cmdline (no pgrep -f)
        PID=$(for p in /proc/[0-9]*; do [ "$(cat $p/comm 2>/dev/null)" = fmi ] && tr "\0" " " < $p/cmdline 2>/dev/null | grep -q "^$bin try @$FIFO " && echo ${p#/proc/}; done | head -1)
        [ -n "$PID" ] || { echo "g2 a $name: no process"; exit 1; }
        d=$P/jobs/j00001; t0=$(date +%s.%N)
        echo "$d" >&7
        until grep -q "^done $d " $P/fmi.log 2>/dev/null; do kill -0 $PID 2>/dev/null || break; sleep 0.1; done
        t1=$(date +%s.%N); c1=$(cpu $PID)
        : > $P/threads.tsv; gone=""
        while :; do
          t=$(calc "$(date +%s.%N) - $t1"); n=$(threads $PID)
          echo -e "$t\t$n\t$(cpu $PID)" >> $P/threads.tsv
          [ -z "$gone" ] && [ "$n" -eq 0 ] && gone=$t
          awk -v t=$t -v h=$HOLD 'BEGIN { exit !(t >= h) }' && break
          sleep 0.5
        done
        c2=$(cpu $PID)
        exec 7>&-
        wait $gpid
        w=$(jq .wall_s $d/result.json); b=$(jq -c .reading.bound $d/result.json)
        echo "a $name ($(sha1sum $bin | cut -c1-12)): try wall $w s (script $(calc "$t1 - $t0") s), bound $b; reading threads at the try's end $(sed -n 1p $P/threads.tsv | cut -f2), gone after ${gone:-more than $HOLD} s; CPU in the $HOLD s after the try $(calc "$c2 - $c1") s" | tee -a $OUT/summary.txt
        rm -rf $P/pool $P/tmp $P/rcache;;
      b)
        P=$OUT/b-$name; rm -rf $P; mkdir -p $P/jobs/j00006 $P/tmp
        ln -s $(readlink -f $B3S/entries) $P/entries; ln -s $(readlink -f $B3S/entries.jsonl) $P/entries.jsonl
        sed "s#$B3S#$P#g" $B3S/jobs/j00006/job.json > $P/jobs/j00006/job.json
        t0=$(date +%s.%N)
        env $ON $MI FMI_READER_SCOPE=$scope FMI_READ_SWITCHES= FMI_READING_CACHE=$P/rcache FMI_MEMO= FMI_PLAN_CACHE=$P/plans FMI_POOL_DIR=$P/pool \
          TMPDIR=$P/tmp E008_DISK_PATH=$OUT E008_FLOOR_GB=$FLOOR_GB timeout 900 nice -n $NICE $bin try $P/jobs/j00006 --search fd --tail > $P/try.log 2>&1
        rc=$?
        r=$(jq -c '{why: .reading.why, read_wall_s: .reading.read_wall_s, records: .reading.records, bound: .reading.bound, rss: .reading.peak_rss_mb, wall_s}' $P/jobs/j00006/result.json 2>/dev/null)
        echo "b $name ($(sha1sum $bin | cut -c1-12)): rc $rc wall $(calc "$(date +%s.%N) - $t0") s: $r" | tee -a $OUT/summary.txt
        (cd $P/readings && find . -type f | sort | xargs sha1sum) > $P/readings.sha1
        rm -rf $P/pool $P/tmp $P/rcache $P/jobs/j00006/run;;
    esac
  done
  if [ $part = b ]; then
    diff <(cut -c1-40,42- $OUT/b-base/readings.sha1) <(cut -c1-40,42- $OUT/b-c2/readings.sha1) > $OUT/b-readings.diff
    echo "b: $(grep -c . $OUT/b-c2/readings.sha1) reading files, $(grep -c '^>' $OUT/b-readings.diff) differ across the builds" | tee -a $OUT/summary.txt
  fi
done
