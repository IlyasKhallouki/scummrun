#!/bin/bash
# high6 (--arrivals) test: from test-high5b's stop 2581 (room 85, var:282 = 3 just won, its line's news), its ways and
# toggles, 7 threads, 30 min. Prediction: P8 within 30 min (high5 from the same stop: its room-61 stop never expanded
# in 690 s). Stop rule: P8 or 30 min.
cd /var/tmp/fast-mi/wt-direct && source experiments/E027-direct/run/env.sh; export ON MI GUIDE27 RUNGS27; for kv in $ON $MI; do export "$kv"; done
D=/var/tmp/fast-mi/direct/test-high6; mkdir -p $D
T=/var/tmp/fast-mi/direct/test-high5b
exec systemd-inhibit --what=idle:sleep --why="scummrun test" nice -n 5 /var/tmp/fast-mi/direct/bench/bin/high6 $D --minutes 30 --stall 60 --workers 7 --slices 7 --batch 1 --at-once 1 --guide $GUIDE27 --rungs $RUNGS27 --watch room=44,room=61,var:282=3,var:282=4 --until bit:565=1 --lines gains --flag-lines --shares --go --motion-once --count-lines --stay-lines --born-first --walk-born --runs-on --counts-world --ends --said-flags --high-lines --arrivals --from $T/rung-stops.jsonl 2581 --news var:282 --edges $T/edges.jsonl --toggles $T/toggles.txt > $D/out.txt 2> $D/err.txt < /dev/null
