#!/usr/bin/env bash
# E015 G2 step 3: re-run the sampled steps, 2 engine hosts at a time, nice 10
source /var/tmp/fast-mi/e015-out/g2/env.sh
cd $OUT
cat sample.txt | xargs -P 2 -I{} bash -c "test -f jobs/{}/summary.json || nice -n 10 $E15 rerun $ARCH {} $OUT >> rerun.log 2>> rerun.err"
