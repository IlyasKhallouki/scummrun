#!/usr/bin/env bash
# E016 play9 on the cloud VM (amendment (7)), as E014's vm/vm.sh: the laptop's files at the laptop's paths, nothing built
# on the VM. The run is laptop.sh's (init, run with the ladder's watcher, stalls, ladder) with the VM's workers and
# readings in the background (play8's b = 10 and 2 by default) and no heavy lock; then every entry verified from boot,
# P at a time (verify_all).
#
# Laptop (the main checkout's scripts/cloud.py keeps the ledger):
#   python3 scripts/cloud.py start --type n2-custom-30-122880 --hours H --standard
#   python3 scripts/cloud.py sh --job e016 -- true     (the VM's lock for this job; puts the ssh key on it)
#   bash vm.sh push                                    copy the files (list below)
#   bash vm.sh vm setup                                the binaries on the copied loader, checks, disk and memory
#   bash vm.sh vm start [W] [BG]                       laptop.sh init run stalls ladder, detached ($L/run.log)
#   bash vm.sh vm status | follow | pull
#   python3 scripts/cloud.py release --job e016; python3 scripts/cloud.py stop; python3 scripts/cloud.py status
#
# What goes to the VM: the worktrees wt-e016 (the run's scripts), wt-e014 and wt-e013 (the environment chain) with their
# links resolved (no .git, out, build, game, data, third_party, .venv, targets); E016's build (target-e016: release,
# checks/release, scumm/release); E013's engine (svm-e013); Fast Downward's search binary; every library they load; the
# demo's files and extraction; the guide; the environment files of E009 and E011; E007's guard.
set -euo pipefail

NAME_VM=fast-mi-1 ZONE=us-central1-a PROJECT=lumind-mvp
WT=/var/tmp/fast-mi/wt-e016
WT14=/var/tmp/fast-mi/wt-e014
WT13=/var/tmp/fast-mi/wt-e013
HERE=$WT/experiments/E016-realize-run/run
T=/var/tmp/fast-mi/target-e016
SVM=/var/tmp/fast-mi/svm-e013/build/scummvm/scummvm
FDB=/home/ilyask/projects/fast-mi/third_party/downward/builds/release/bin/downward
GAMED=/var/tmp/fast-mi/games/fate-play-demo
XR=/var/tmp/fast-mi/e014-root
GUIDE=/var/tmp/fast-mi/e014-play-vm4/guide-play.json
ARCHS=/var/tmp/fast-mi/e016-archives
OUT=/var/tmp/fast-mi/e016-play9
L=$OUT/vm-logs
LIB=/opt/fmi-e016-lib
SSHOPT="-i $HOME/.ssh/google_compute_engine -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ServerAliveInterval=30 -o LogLevel=ERROR"
U=$(whoami)

say() { echo "[$(date -u +%H:%M:%S)] $*"; }
ip() { gcloud compute instances describe $NAME_VM --zone=$ZONE --project=$PROJECT \
         --format='get(networkInterfaces[0].accessConfigs[0].natIP)'; }
put() { rsync -az -e "ssh $SSHOPT" "$@"; }
bins() {
    for d in $T/release $T/checks/release $T/scumm/release; do
        find $d -maxdepth 1 -type f -perm -u+x ! -name '*.laptop' | sort
    done
    echo $SVM
    echo $FDB
}

push() {
    local IP; IP=$(ip); local V=$U@$IP
    say "dirs on $IP"
    ssh $SSHOPT $V "sudo mkdir -p /var/tmp/fast-mi $LIB && sudo chown $U /var/tmp/fast-mi $LIB && \
        mkdir -p $WT $WT14 $WT13 $T/release $T/checks/release $T/scumm/release $(dirname $SVM) $(dirname $FDB) $GAMED \
        $XR/data $XR/build $(dirname $GUIDE) $ARCHS $OUT $L $HOME/.cache/fast-mi-e007 $HOME/.cache/fast-mi-e009 \
        $HOME/.cache/fast-mi-e011"
    say "worktrees (links resolved)"
    for w in $WT $WT14 $WT13; do
        put -L --exclude=/.git --exclude=/out --exclude=/build --exclude=/game --exclude=/data --exclude=/third_party \
            --exclude=/.venv --exclude=target/ --exclude=__pycache__ --exclude=.pytest_cache $w/ $V:$w/
    done
    say "binaries"
    local b
    for b in $(bins); do put $b $V:$b; done
    say "their libraries"
    local list; list=$(mktemp)
    for b in $(bins); do ldd $b | awk '/=> \//{print $3}'; done | sort -u > $list
    echo /lib64/ld-linux-x86-64.so.2 >> $list
    # sdl2-compat loads SDL3 at run time (dlopen), so ldd does not list it
    ldconfig -p | awk '/libSDL3\.so\.0 /{print $NF; exit}' >> $list
    say "$(wc -l < $list) libraries"
    rsync -aL --no-relative --files-from=$list -e "ssh $SSHOPT" / $V:$LIB/
    rm -f $list
    say "the demo's files and extraction, the guide"
    put $GAMED/ $V:$GAMED/
    put $XR/data/scripts-fate-play-demo $V:$XR/data/
    put $XR/build/blocks-fate-play-demo $XR/build/verb-blocks-fate-play-demo $V:$XR/build/
    put $GUIDE $V:$GUIDE
    say "environment, guard"
    put $HOME/.cache/fast-mi-e007/guard.sh $HOME/.cache/fast-mi-e007/waitmem.sh $V:$HOME/.cache/fast-mi-e007/
    put $HOME/.cache/fast-mi-e009/env.sh $V:$HOME/.cache/fast-mi-e009/
    put $HOME/.cache/fast-mi-e011/env.sh $HOME/.cache/fast-mi-e011/env-r2.sh $HOME/.cache/fast-mi-e011/env-r2d.sh \
        $V:$HOME/.cache/fast-mi-e011/
    say "pushed"
}

# --- on the VM -------------------------------------------------------------------------------------------------------

setup() {
    say "packages"
    sudo apt-get install -yq patchelf binutils jq >/dev/null
    local b a c t bad=0
    t=$(mktemp -d)
    for b in $(bins); do
        # a binary not yet on the copied loader is the laptop's (a fresh push): it replaces the kept copy
        [ "$(patchelf --print-interpreter $b)" = $LIB/ld-linux-x86-64.so.2 ] || cp $b $b.laptop
        cp $b.laptop $b
        patchelf --set-interpreter $LIB/ld-linux-x86-64.so.2 --force-rpath --set-rpath $LIB $b
        objcopy -O binary --only-section=.text $b.laptop $t/a; objcopy -O binary --only-section=.text $b $t/c
        a=$(sha1sum < $t/a | cut -c1-12); c=$(sha1sum < $t/c | cut -c1-12)
        [ "$a" = "$c" ] || { echo "$b: the code section differs ($a, $c)"; bad=1; }
        [ "$(ldd $b | grep -c 'not found' || true)" = 0 ] || { echo "$b: missing libraries"; bad=1; }
    done
    rm -rf $t
    for f in $LIB/*.so*; do
        if readelf -d $f 2>/dev/null | grep -q RUNPATH; then patchelf --set-rpath $LIB $f; fi
    done
    say "$(bins | wc -l) binaries on $LIB's loader; code sections equal: $([ $bad = 0 ] && echo yes || echo NO)"
    [ $bad = 0 ] || exit 1
    SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy $SVM --version | head -1
    $T/release/fmi 2>&1 | head -1
    echo "downward rc $($FDB --help >/dev/null 2>&1; echo $?)"
    nproc; free -g | head -2; df -h /var/tmp /home
}

run_bg() {  # W BG: laptop.sh's init, run, stalls, ladder; then every entry from boot
    cd $HERE
    HEAVY_LOCK=none W=$1 A4_BG=$2 bash laptop.sh init run stalls ladder || true
    say "run done"
    verify_all
}

resume_bg() {  # W BG: the archive run on from its files (amendment (9)), then stalls, ladder, every entry from boot
    cd $HERE
    rm -f $ARCHS/play9/summary.json
    HEAVY_LOCK=none MIN_FREE_GB=${MIN_FREE_GB:-10} W=$1 A4_BG=$2 bash laptop.sh run stalls ladder || true
    say "run done"
    verify_all
}

resume() {  # [W] [BG], detached
    mkdir -p $L
    setsid nohup bash $HERE/vm.sh resume_bg ${1:-10} ${2:-2} >> $L/run.log 2>&1 < /dev/null &
    say "resumed with ${1:-10} workers, ${2:-2} readings in the background (pid $!)"
}

start() {  # [W] [BG], detached from the ssh session
    mkdir -p $L
    setsid nohup bash $HERE/vm.sh run_bg ${1:-10} ${2:-2} > $L/run.log 2>&1 < /dev/null &
    say "started with ${1:-10} workers, ${2:-2} readings in the background (pid $!)"
}

verify_all() {  # P: every entry of the run's archive from boot, P at a time; the count with differences or errors
    cd $HERE
    source $HERE/env.sh
    local O=$OUT/verify n
    mkdir -p $O
    n=$(wc -l < $D_ARCH/entries.jsonl)
    seq 1 $((n - 1)) | xargs -P ${1:-24} -I{} bash -c "env $E14M $GAME FMI_ENGINE_BIN=$E13_ENGINE_BIN nice -n 10 \
        $FMI_BIN explore verify $D_ARCH --entry {} > $O/{}.out 2>&1; grep -q '^differences: 0;' $O/{}.out || echo 'entry {}: '\$(tail -1 $O/{}.out)" \
        > $L/verify-bad.txt || true
    say "verify $D_ARCH: $((n - 1)) entries from boot, $(grep -c . $L/verify-bad.txt || true) with differences or errors" | tee $L/verify.log
}

status() {
    tail -3 $L/run.log 2>/dev/null || true
    tail -2 $OUT/run.log 2>/dev/null || true
    tail -1 $OUT/ladder.log 2>/dev/null || true
    uptime; free -g | sed -n 2p; df -h /var/tmp | tail -1
}

# --- on the laptop ---------------------------------------------------------------------------------------------------

pull() {  # the run's archive back without readings, and the VM's outputs
    local IP; IP=$(ip)
    mkdir -p $ARCHS/play9 $OUT
    rsync -a -e "ssh $SSHOPT" --exclude=/readings --exclude='/jobs/*/run' --exclude='/verify-*/' $U@$IP:$ARCHS/play9/ $ARCHS/play9/
    rsync -a -e "ssh $SSHOPT" $U@$IP:$OUT/ $OUT/
}

follow() {  # the ladder and the archive's log every 120 s; the archive without readings every 30 min
    mkdir -p $ARCHS/play9 $OUT
    local k=0 IP
    while true; do
        IP=$(ip) || true
        rsync -a -e "ssh $SSHOPT" $U@$IP:$ARCHS/play9/log.jsonl $U@$IP:$ARCHS/play9/entries.jsonl $ARCHS/play9/ 2>/dev/null || true
        rsync -a -e "ssh $SSHOPT" --exclude=/verify $U@$IP:$OUT/ $OUT/ 2>/dev/null || true
        k=$((k + 1))
        if [ $((k % 15)) = 0 ]; then
            rsync -a -e "ssh $SSHOPT" --exclude=/readings --exclude='/jobs/*/run' --exclude='/verify-*/' \
                $U@$IP:$ARCHS/play9/ $ARCHS/play9/ 2>/dev/null || true
        fi
        sleep 120
    done
}

vm() {  # run a subcommand of this script on the VM
    local IP; IP=$(ip)
    ssh $SSHOPT $U@$IP "bash $HERE/vm.sh $*"
}

cmd=${1:-}; shift || true
case "$cmd" in
    push|pull|follow|vm|setup|start|run_bg|resume|resume_bg|verify_all|status)
        $cmd "$@" ;;
    *) sed -n '2,20p' "$0"; exit 2 ;;
esac
