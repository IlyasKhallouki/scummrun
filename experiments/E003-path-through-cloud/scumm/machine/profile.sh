#!/bin/bash
# Where the Rust machine spends its time on a run's moves: a poor man's sampling profiler (no perf on the laptop).
# It builds machine-run with line tables into target/prof, runs it under gdb SAMPLES times (ptrace_scope 1 lets gdb
# trace its own children only), interrupts it after a random delay while it repeats every move REPEAT times, and
# tallies the machine thread's stacks: the leaf function, the first frame in this crate, and our functions inclusive.
#
# usage (from scumm/machine): ./profile.sh RUN DIR [REPEAT] [SAMPLES]
#   DIR/machine/RUN/jobs.json must exist (a machine-run folder); the move files there are overwritten without
#   check.py's line, so use a throwaway copy of the folder.
set -eu
RUN=$1; DIR=$2; REPEAT=${3:-3000}; N=${4:-80}
HERE=$(cd "$(dirname "$0")" && pwd)
cd "$HERE/.."
CARGO_TARGET_DIR=target/prof CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
  cargo build --release -p scumm-machine --bin machine-run >/dev/null 2>&1
BIN=$PWD/target/prof/release/machine-run
OUT=$(mktemp)
for i in $(seq "$N"); do
  d=$(python3 -c "import random; print(round(1.2 + random.random() * 6, 2))")
  timeout -s INT "$d" gdb -batch -ex run -ex "thread apply all bt 30" -ex kill \
    --args "$BIN" "$RUN" --out "$DIR" --repeat "$REPEAT" --no-prep --no-check --no-trace --jobs 1 2>/dev/null \
    | grep -E "^#" | sed -E 's/^#([0-9]+) +(0x[0-9a-f]+ in )?/\1 /' >> "$OUT" || true
  echo "----" >> "$OUT"
done
python3 - "$OUT" <<'EOF'
import collections, re, sys
selfc, incl, ours, n = collections.Counter(), collections.Counter(), collections.Counter(), 0
name = lambda f: re.sub(r"<.*", "", f.split(" (")[0].split(" at ")[0]).strip()
for sample in open(sys.argv[1]).read().split("----"):
    threads, cur = [], []
    for line in sample.strip().splitlines():
        m = re.match(r"(\d+) (.*)", line)
        if not m:
            continue
        if m[1] == "0" and cur:
            threads.append(cur)
            cur = []
        cur.append(m[2])
    if cur:
        threads.append(cur)
    t = next((t for t in threads if any("run_model" in f for f in t)), None)
    if t is None:
        continue
    n += 1
    selfc[name(t[0])] += 1
    first = next((f for f in t if "machine/src" in f), None)
    if first:
        ours[name(first) + " @" + first.rsplit("machine/src/", 1)[1]] += 1
    for f in {name(f) for f in t if "machine/src" in f}:
        incl[f] += 1
print(n, "samples")
print("leaf:", selfc.most_common(12))
print("first frame in this crate:", ours.most_common(15))
print("inclusive:", incl.most_common(25))
EOF
rm -f "$OUT"
