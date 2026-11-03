#!/bin/bash
# E011 route: the Rust lama-first-costs against the Rust Gbfs (and Fast Downward's lama-first-costs) on the same
# requests through fmi-plan (the request path a run takes: the warm request, the in-memory task, the search, the
# check, the decode), interleaved: gbfs, lama, gbfs, lama, ... ROUNDS times, one fmi-plan process each, the plan cache
# off. One JSON line per request and round in OUT/runs.jsonl (fmi-plan's seconds, CPU, expanded, evaluated).
# usage: bench_plan.sh FMI_PLAN READING REQS OUT ROUNDS [SEARCHES]
set -u
FP=$1; RD=$2; REQS=$3; OUT=$4; ROUNDS=$5; SEARCHES=${6:-gbfs lama}
mkdir -p $OUT
for r in $(seq 1 $ROUNDS); do
  for s in $SEARCHES; do
    FMI_PLAN_CACHE= nice -n 10 $FP seq --reading $RD --reqs $REQS --search $s --cache off > $OUT/$s-$r.jsonl 2> $OUT/$s-$r.err
    python3 -c "
import json, sys
for l in open('$OUT/$s-$r.jsonl'):
    d = json.loads(l)
    if 'name' in d:
        d['search'], d['round'] = '$s', $r
        print(json.dumps(d))
" >> $OUT/runs.jsonl
  done
done
python3 - $OUT/runs.jsonl <<'PYEOF'
import json, sys, collections
runs = [json.loads(l) for l in open(sys.argv[1])]
by = collections.defaultdict(list)
for d in runs:
    by[(d['name'], d['search'])].append(d)
names = list(dict.fromkeys(d['name'] for d in runs))
searches = list(dict.fromkeys(d['search'] for d in runs))
print('request', *[f'{s} cpu (min) eval' for s in searches], 'lama/gbfs', sep='\t')
for n in names:
    row, cpu = [n], {}
    for s in searches:
        xs = by[(n, s)]
        c = min(x['cpu_s'] for x in xs); cpu[s] = c
        row.append(f"{c:.2f} {xs[0]['evaluated']} {xs[0]['status']}")
    if 'gbfs' in cpu and 'lama' in cpu and cpu['gbfs'] > 0:
        row.append(f"{cpu['lama'] / cpu['gbfs']:.2f}")
    print(*row, sep='\t')
PYEOF
