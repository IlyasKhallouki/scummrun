#!/usr/bin/env bash
# E015 G2 D1: IW(1) under E from 25 entries, 2 hosts
source /var/tmp/fast-mi/e015-out/g2/env.sh
cd $OUT
A=0,26,52,78,104,130,156,182,195,221,247,273,615
B=13,39,65,91,117,143,169,184,208,234,260,286
nice -n 10 $E15 depth $ARCH $OUT --entries $A --tjob j00077 --room 63 --unit local:211 > d1-a.log 2> d1-a.err &
nice -n 10 $E15 depth $ARCH $OUT --entries $B --tjob j00077 --room 63 --unit local:211 > d1-b.log 2> d1-b.err &
wait
