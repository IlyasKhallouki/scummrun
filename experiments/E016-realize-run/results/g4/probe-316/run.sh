#!/usr/bin/env bash
# E016 G4 run 3's walk_to(316) probe and amendment (5)'s runs probe: the chain-mode state (w316chain.jsonl: the chain
# run's inputs to 18316, the four accepted approach clicks, the bare click on 316 at 18372) and the chain's own state
# (../../probe's kitchen-chain.jsonl), each with the input log and the object scripts' slot runs.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
source "$here/../../probe/env-mi.sh"
OUT=${OUT:-/var/tmp/fast-mi/e016-a-out}
nice -n 10 "$E016_BIN" inlog "$OUT/w316chain-18371" --events "$here/w316chain.jsonl" --from 18371 --to 18380 > /dev/null
nice -n 10 "$E016_BIN" inlog "$OUT/r316-chain" --events "$here/w316chain.jsonl" --from 18371 --to 18560 --runs > /dev/null
nice -n 10 "$E016_BIN" inlog "$OUT/r316-orig" --events "$OUT/probe/kitchen-chain.jsonl" --from 18364 --to 18404 --runs > /dev/null
cp "$OUT/w316chain-18371/inlog.json" "$here/inlog.json"
cp "$OUT/r316-chain/inlog.json" "$here/runs-chain-18371.json"
cp "$OUT/r316-orig/inlog.json" "$here/runs-chain-own-18364.json"
