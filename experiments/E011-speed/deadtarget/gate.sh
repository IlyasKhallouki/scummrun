#!/bin/bash
# Gate 1-3 of plan.md's amendment "targets with no plan made cheap": every rebuildable recorded request (requests.py's
# groups) replayed through fmi-plan with the proofs on and FMI_PLAN_DEAD_CHECK (a found plan's states inside the h^2
# rows; the fast h^2 equal to the plain one); and E008's serve sequences with the proofs on and off, compared byte for
# byte. The report is report.py's.
#
# usage: [SERVE="38 33"] gate.sh FMI_PLAN GATE_DIR SERVE_DIR [GROUP ...]
#   GATE_DIR: requests.py's output; SERVE_DIR: fmi-plan/check (serve38.tsv, serve33.tsv); SERVE: which sequences
#   (the ring(33) ones need the heavy lock, as the ring(33) groups do)
set -u
BIN=$1; G=$2; SV=$3; shift 3
OUT=$G/out; mkdir -p "$OUT"
groups=("$@")
[ ${#groups[@]} = 0 ] && groups=($(python3 -c "import json,sys; g=json.load(open('$G/groups.json')); print(' '.join(sorted(g, key=lambda k: g[k]['reading'].endswith(('refine-01','28-33-35-38-j00000','r33')))))"))
for g in "${groups[@]}"; do
  rd=$(python3 -c "import json; print(json.load(open('$G/groups.json'))['$g']['reading'])")
  s=$(python3 -c "import json; print(json.load(open('$G/groups.json'))['$g']['search'])")
  FMI_PLAN_DEAD_CHECK=1 "$BIN" seq --reading "$rd" --reqs "$G/groups/$g.tsv" --search "$s" --cache off --dead on --time 600 \
    --dump "$OUT/$g" > "$OUT/$g.log" 2> "$OUT/$g.err"
  echo "$g rc $?"
done
for r in ${SERVE-38 33}; do
  rd=$([ $r = 38 ] && echo $HOME/.cache/fast-mi-e011/profile-planner/readings/r38 || echo $HOME/.cache/fast-mi-e011/plan/r33)
  for d in on off; do
    FMI_PLAN_DEAD_CHECK=1 "$BIN" seq --reading "$rd" --reqs "$SV/serve$r.tsv" --search fd --cache off --dead $d --time 600 \
      --dump "$OUT/serve$r-$d" > "$OUT/serve$r-$d.log" 2> "$OUT/serve$r-$d.err"
    echo "serve$r-$d rc $?"
  done
done
