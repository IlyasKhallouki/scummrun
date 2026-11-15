#!/bin/bash
# gate.sh [BIN] [OUT]: the gate. The five Monkey Island references (bench.sh: 2 threads, 7 stops expanded from each) and
# the Fate of Atlantis demo's first 25 stops, each run one stop at a time so that the result does not depend on the
# machine, compared with the digests of November 2026 (the laptop's and the VM's: results/full5/gate-vm.txt). Prints
# PASS when all six match: the search, the engine, the game's files and the scripts' dumps then behave as in the runs.
#   BIN  the search (default: cargo's build, experiments/E011-speed/rs/target/fast/direct)
#   OUT  default out/gate
# Needs: the engine (engine.sh), Monkey Island's DOS CD files in game/dos-cd, the demo's in game/fate-play-demo, and
# both scripts' dumps (data/scripts-dos-cd, data/scripts-fate-play-demo). About 5 to 10 minutes.
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); ROOT=$(cd "$HERE/../../.." && pwd)
BIN=$(realpath "${1:-$ROOT/experiments/E011-speed/rs/target/fast/direct}"); OUT=${2:-$ROOT/out/gate}
mkdir -p "$OUT"; OUT=$(realpath "$OUT")
[ -x "$BIN" ] || { echo "no search at $BIN (build it: cd experiments/E011-speed/rs && cargo build --profile fast -p fmi-direct)" >&2; exit 2; }

bash "$HERE/bench.sh" "$BIN" "$OUT/refs" 2 7 > "$OUT/refs.log" 2>&1 || { cat "$OUT/refs.log"; exit 1; }

source "$HERE/env.sh"
ulimit -n 65536 2>/dev/null || true
O=$OUT/demo; rm -rf "$O"; mkdir -p "$O"
env $ON $DEMO "$BIN" "$O" --minutes 14 --stops 25 --workers 2 --slices 2 --batch 1 --at-once 1 \
  --lines gains --flag-lines --go --shares > "$O/out.txt" 2> "$O/err.txt" || { tail -5 "$O/err.txt"; exit 1; }
rm -rf "$O/eng"

python3 - "$OUT" <<'PY'
import hashlib, json, re, sys
OUT = sys.argv[1]
# the digests of the runs of 2026-11 (trials, stops kept, digest of the kept stops)
EXPECT = {"p7top": (7337, 134, "aa25d4c5a4"), "cake": (10800, 154, "c738a50ae4"), "crew": (5329, 306, "b9ce9c2930"),
          "meathook": (10799, 56, "22e11af161"), "boot": (2219, 52, "644e9e311c"), "demo": (3163, 225, "d2549dd5c3")}
got = {}
for l in open(f"{OUT}/refs.log"):
    m = re.match(r"(\w+)\s+stops\s+\d+ trials\s+(\d+) kept\s+(\d+) digest (\w+)", l)
    if m:
        got[m.group(1)] = (int(m.group(2)), int(m.group(3)), m.group(4))
s = json.load(open(f"{OUT}/demo/summary.json")); h = hashlib.sha1(); n = 0
for l in open(f"{OUT}/demo/nodes.jsonl"):
    i = l.find(',"events"'); d = json.loads(l[:i] + "}") if i > 0 else json.loads(l)
    if d["id"] == 0:
        continue
    h.update(repr((d["id"], d["parent"], d["how"], d["room"], d["level"], d["class"], d["frame"])).encode()); n += 1
got["demo"] = (s["trials"], n, h.hexdigest()[:10])
ok = True
for k, e in EXPECT.items():
    g = got.get(k)
    same = g == e
    ok &= same
    print(f"{k:9s} trials {g[0] if g else '-':>6} kept {g[1] if g else '-':>4} digest {g[2] if g else '-':10s}  "
          f"{'same' if same else 'DIFFERENT (expected %s trials, %s kept, %s)' % e}")
print("PASS" if ok else "FAIL")
sys.exit(0 if ok else 1)
PY
