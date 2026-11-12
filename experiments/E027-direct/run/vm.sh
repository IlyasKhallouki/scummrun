#!/bin/bash
# E027 on the project's VM (fast-mi-1), from the laptop. The VM is started and stopped with scripts/cloud.py only
# (the ledger and the cap); this script does what a run needs once it is up, in the forms that worked on
# 2026-11-11: nothing waits on the laptop, a run goes on when the laptop's shell ends, and what a run found is on
# the laptop before the VM stops.
#
#   vm.sh ip IP                 note the VM's address (cloud.py start prints it)
#   vm.sh sync                  the search's source, the engine's patch, this folder and the kept stops, to the VM
#   vm.sh setup                 on the VM: E027's engine (engine.sh) and the search's binary
#   vm.sh run NAME MINUTES [FROM] [WATCH]
#                               a run, detached. FROM: "boot", a run's name on the VM (its top.jsonl and ways), or
#                               a results folder's name here that holds top.jsonl (results/NAME). WATCH: K=V,...
#   vm.sh stat NAME             the run's rungs, watched values and last progress line
#   vm.sh pull NAME             results/NAME here: rung-stops.jsonl, top.jsonl, edges.jsonl, summary.json, progress
#   vm.sh watch NAME            pull every 3 minutes until the run's summary is there (run it in the background)
#   vm.sh kill                  end the run's processes on the VM
#
# Settings (environment): WORKERS (28), SLICES (7: threads for one stop's round), AT_ONCE (4: stops at once),
# EXTRA (more flags for the search, e.g. --motion-once).
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); ROOT=$(cd "$HERE/../../.." && pwd)
IPF=$HOME/.cache/fast-mi-e026/vm/ip
SSH="ssh -i $HOME/.ssh/google_compute_engine -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=15"
VMROOT=/var/tmp/fast-mi/wt-direct; VMOUT=/var/tmp/fast-mi/direct; RES=$ROOT/experiments/E027-direct/results
host() { echo "ilyask@$(cat "$IPF")"; }
on() { timeout "${T:-120}" $SSH "$(host)" "$@"; }
cmd=${1:-}; shift || true
case $cmd in
  ip) echo "$1" > "$IPF"; cat "$IPF";;
  sync)
    cd "$ROOT"
    rsync -az --compress-choice=zstd -e "$SSH" --relative \
      experiments/E011-speed/rs/Cargo.toml experiments/E011-speed/rs/Cargo.lock \
      experiments/E011-speed/rs/fmi-direct experiments/E011-speed/rs/fmi-engine experiments/E011-speed/rs/fmi-core experiments/E011-speed/rs/fmi-explore \
      experiments/E011-speed/rs/fmi-checks/rungs patches experiments/E026-part1/run/env.sh experiments/E027-direct/run experiments/E027-direct/results \
      "$(host):$VMROOT/"
    echo synced;;
  setup)
    T=900 on "export PATH=\$HOME/.cargo/bin:\$PATH; set -e; $VMROOT/experiments/E027-direct/run/engine.sh; cd $VMROOT/experiments/E011-speed/rs && CARGO_TARGET_DIR=/var/tmp/fast-mi/target-direct7 cargo build --profile fast -p fmi-direct 2>&1 | tail -1; ls -la /var/tmp/fast-mi/target-direct7/fast/direct; df -h /var/tmp | tail -1";;
  run)
    name=$1; minutes=$2; from=${3:-boot}; watch=${4:-}
    case $from in
      boot) FROM="";;
      *) if on "test -f $VMOUT/$from/top.jsonl"; then FROM="--from-run $VMOUT/$from"; else FROM="--from-run $VMROOT/experiments/E027-direct/results/$from"; fi;;
    esac
    # the run's own script on the VM, started detached: it lives on when this shell and its ssh end
    on "mkdir -p $VMOUT/$name && cat > $VMOUT/$name-run.sh" <<RUN
#!/bin/bash
export PATH=\$HOME/.cargo/bin:\$PATH
cd $VMROOT
source experiments/E027-direct/run/env.sh
game_ok || true
env \$ON \$MI /var/tmp/fast-mi/target-direct7/fast/direct $VMOUT/$name --minutes $minutes --workers ${WORKERS:-28} \\
  --slices ${SLICES:-7} --batch 1 --at-once ${AT_ONCE:-4} --guide \$GUIDE27 --rungs \$RUNGS27 ${watch:+--watch $watch} \\
  --lines gains --flag-lines --shares --go ${EXTRA:-} $FROM > $VMOUT/$name/out.txt 2> $VMOUT/$name/err.txt
rm -rf $VMOUT/$name/eng
RUN
    T=30 on "chmod +x $VMOUT/$name-run.sh; nohup setsid $VMOUT/$name-run.sh > $VMOUT/$name/launch.txt 2>&1 < /dev/null & exit 0"
    echo "started $name: $minutes minutes, from $from";;
  stat)
    on "grep -E 'RUNG|WATCH|Too many|^node|panicked' $VMOUT/$1/err.txt | sort -u | cut -c1-120; grep -E '^\[ *[0-9.]+s\] kept' $VMOUT/$1/err.txt | tail -1 | cut -c1-330; ls $VMOUT/$1/summary.json 2>/dev/null; ps -eo comm | grep -cE '^direct$'";;
  pull)
    mkdir -p "$RES/$1"
    on "(grep -E 'RUNG|WATCH|^boot|^mentions|^guide|Too many|^node' $VMOUT/$1/err.txt | sort -u; grep -E '^\[ *[0-9.]+s\] kept' $VMOUT/$1/err.txt | awk 'NR%8==1' | cut -c1-330; grep -E '^\[ *[0-9.]+s\] kept' $VMOUT/$1/err.txt | tail -1 | cut -c1-330) > $VMOUT/$1/progress.txt" || true
    for f in rung-stops.jsonl top.jsonl edges.jsonl summary.json progress.txt; do
      timeout 120 rsync -az -e "$SSH" "$(host):$VMOUT/$1/$f" "$RES/$1/$f.new" 2>/dev/null && mv "$RES/$1/$f.new" "$RES/$1/$f" || true
    done
    ls -la "$RES/$1";;
  watch)
    until [ -f "$RES/$1/summary.json" ]; do "$0" pull "$1" > /dev/null 2>&1 || true; sleep 180; done; echo "run $1 ended; results in $RES/$1";;
  kill) on "pkill -x direct; pkill -x scummvm; sleep 1; ps -eo comm | grep -cE '^(direct|scummvm)$'" || true;;
  *) sed -n '2,20p' "$0"; exit 2;;
esac
