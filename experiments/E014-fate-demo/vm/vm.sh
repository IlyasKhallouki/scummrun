#!/usr/bin/env bash
# E014 amendment (3) on the cloud VM, as E009's vm_stage1.sh: the laptop's files at the laptop's paths, so every path in
# the code, the environment and the archives is the one the laptop uses. Nothing is built on the VM.
#
# Laptop (the main checkout's scripts/cloud.py keeps the ledger):
#   python3 scripts/cloud.py start --type n2-custom-30-122880 --hours H --standard
#   python3 scripts/cloud.py sh --job e014 -- true     (the VM's lock for this job; puts the ssh key on it)
#   bash vm.sh push                                    copy the files (list below)
#   bash vm.sh vm setup                                the binaries on the copied loader, checks, disk and memory
#   bash vm.sh vm check                                the check before the run (below), log $L/check.log
#   bash vm.sh pull-check                              the VM's check archive back, against the laptop's
#   bash vm.sh vm start W                              init and run the explorer with W workers, detached ($L/run.log)
#   bash vm.sh follow                                  copy log.jsonl back every 120 s, the archive every 30 min
#   bash vm.sh pull                                    the archive back without its readings, and the VM's logs
#   python3 scripts/cloud.py release --job e014; python3 scripts/cloud.py stop; python3 scripts/cloud.py status
#
# What goes to the VM's private disk (deleted with it), never into git: the worktrees wt-e013 (the binaries' compiled
# repository root) and wt-e014 with their links resolved (no .git, out, build, game, data, third_party, .venv); E013's
# build (target-e013: release, checks/release, scumm/release executables); E013's engine (svm-e013, patches 0001-0005,
# 0006-e014, 0007); Fast Downward's search binary; every library they load (ldd, with the laptop's loader); the demo's
# files and extraction; the guide; the environment files of E009 and E011; E007's guard.
# The VM's C library is older than the laptop's, so setup copies each binary aside (NAME.laptop), sets its ELF
# interpreter and library path to the copied loader and libraries (patchelf) and checks that its code section is byte
# for byte the laptop's.
#
# The check before the run (`vm check`): the laptop's 2-try archive with the run's settings (b = 1,
# /var/tmp/fast-mi/e014-archives/vm-check2) has every entry verified from boot on the VM; then the same 2 tries are run
# on the VM from a fresh init at the same paths, and `pull-check` brings them back as vm-check2-vm and compares them
# with the laptop's (fmi check tries; entries.jsonl, cells.json, every job.json, log.jsonl's keys: timing aside).
set -euo pipefail

NAME_VM=fast-mi-1 ZONE=us-central1-a PROJECT=lumind-mvp
WT=/var/tmp/fast-mi/wt-e014
WT13=/var/tmp/fast-mi/wt-e013
HERE=$WT/experiments/E014-fate-demo/vm
T=/var/tmp/fast-mi/target-e013
SVM=/var/tmp/fast-mi/svm-e013/build/scummvm/scummvm
FDB=/home/ilyask/projects/fast-mi/third_party/downward/builds/release/bin/downward
GAMED=/var/tmp/fast-mi/games/fate-play-demo
XR=/var/tmp/fast-mi/e014-root
ARCHS=/var/tmp/fast-mi/e014-archives
OUT=/var/tmp/fast-mi/e014-play-vm
L=$OUT/logs
CK=/var/tmp/fast-mi/e014-vm-check2
LIB=/opt/fmi-e014-lib
SSHOPT="-i $HOME/.ssh/google_compute_engine -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ServerAliveInterval=30 -o LogLevel=ERROR"
U=$(whoami)

say() { echo "[$(date -u +%H:%M:%S)] $*"; }
ip() { gcloud compute instances describe $NAME_VM --zone=$ZONE --project=$PROJECT \
         --format='get(networkInterfaces[0].accessConfigs[0].natIP)'; }
put() { rsync -az -e "ssh $SSHOPT" "$@"; }
# the ELF executables pushed: E013's build, the engine, Fast Downward's search
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
        mkdir -p $WT $WT13 $T/release $T/checks/release $T/scumm/release $(dirname $SVM) $(dirname $FDB) $GAMED \
        $XR/data $XR/build $ARCHS $OUT $L $CK $HOME/.cache/fast-mi-e007 $HOME/.cache/fast-mi-e009 $HOME/.cache/fast-mi-e011"
    say "worktrees (links resolved)"
    for w in $WT $WT13; do
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
    say "$(wc -l < $list) libraries"
    rsync -aL --no-relative --files-from=$list -e "ssh $SSHOPT" / $V:$LIB/
    rm -f $list
    say "the demo's files and extraction, the guide, the laptop's check archive"
    put $GAMED/ $V:$GAMED/
    put $XR/data/scripts-fate-play-demo $V:$XR/data/
    put $XR/build/blocks-fate-play-demo $XR/build/verb-blocks-fate-play-demo $V:$XR/build/
    put $OUT/guide-play.json $V:$OUT/
    put --exclude=/readings --exclude='/jobs/*/run' $ARCHS/vm-check2 $V:$ARCHS/
    put $CK/guide-play.json $V:$CK/
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
        [ -e $b.laptop ] || cp $b $b.laptop
        cp $b.laptop $b
        patchelf --set-interpreter $LIB/ld-linux-x86-64.so.2 --force-rpath --set-rpath $LIB $b
        objcopy -O binary --only-section=.text $b.laptop $t/a; objcopy -O binary --only-section=.text $b $t/c
        a=$(sha1sum < $t/a | cut -c1-12); c=$(sha1sum < $t/c | cut -c1-12)
        [ "$a" = "$c" ] || { echo "$b: the code section differs ($a, $c)"; bad=1; }
        [ "$(ldd $b | grep -c 'not found' || true)" = 0 ] || { echo "$b: missing libraries"; bad=1; }
    done
    rm -rf $t
    # a library with its own RUNPATH looks for its own needs there only
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

check() {
    cd $HERE
    export ENV=$HERE/env.sh D_ARCH=$ARCHS/vm-check2 D_RUN=$ARCHS/vm-check2-run D_OUT=$CK HEAVY_LOCK=none
    {
        say "verify the laptop's check archive from boot on the VM"
        bash run.sh verify
        say "the same 2 tries on the VM (b = 1), at the laptop's paths"
        rm -rf $ARCHS/vm-check2-laptop $D_RUN $CK/verify $CK/run*.log $CK/init.log
        mv $D_ARCH $ARCHS/vm-check2-laptop
        W=1 STOP_JOBS=2 bash run.sh init
        W=1 bash run.sh run
        bash run.sh verify
        say "check done"
    } > $L/check.log 2>&1
    tail -6 $L/check.log
}

run_bg() {  # W: init and run, then verify every entry, then the stall table
    cd $HERE
    export ENV=$HERE/env.sh HEAVY_LOCK=none W=$1
    bash run.sh init
    bash run.sh run
    say "run done"
    bash run.sh stalls || true
    say "stalls done"
}

start() {  # W, detached from the ssh session
    mkdir -p $L
    setsid nohup bash $HERE/vm.sh run_bg "$@" > $L/run.log 2>&1 < /dev/null &
    say "started with $1 workers (pid $!)"
}

resume() {  # W: run the archive on from its files
    cd $HERE
    export ENV=$HERE/env.sh HEAVY_LOCK=none W=$1
    bash run.sh run
    say "run done"
    bash run.sh stalls || true
}

restart() {  # W, detached
    setsid nohup bash $HERE/vm.sh resume "$@" >> $L/run.log 2>&1 < /dev/null &
    say "resumed with $1 workers (pid $!)"
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
    uptime; free -g | sed -n 2p; df -h /var/tmp | tail -1
}

# --- on the laptop ---------------------------------------------------------------------------------------------------

pull_check() {  # the VM's 2 tries back as vm-check2-vm, against the laptop's vm-check2 (the same paths in both)
    local IP; IP=$(ip)
    rm -rf $ARCHS/vm-check2-vm
    mkdir -p $ARCHS/vm-check2-vm
    rsync -a -e "ssh $SSHOPT" --exclude=/readings --exclude='/jobs/*/run' --exclude='/verify-*/' $U@$IP:$ARCHS/vm-check2/ $ARCHS/vm-check2-vm/
    rsync -a -e "ssh $SSHOPT" $U@$IP:$L/ $OUT/logs-vm/
    source $HERE/env.sh
    local A=$ARCHS/vm-check2 B=$ARCHS/vm-check2-vm diffs=0 f j
    env $E14M $GAME $FMI_BIN check tries $A $B --json $OUT/check-tries.json > $OUT/check-tries.out 2>&1 || true
    say "fmi check tries laptop VM: $(tail -1 $OUT/check-tries.out)"
    local LOGKEYS='{job, entry, cell, chosen, commit_index, outcome, error, rc, n, key, selectable, scope, targets, tried, job_tried, exhausted, deferred, dead, dead_entries, goal_entries, new_entries, new_cells, new_r, new_reps, new_rooms, new_t, t_new, t_size, kind}'
    for f in entries.jsonl cells.json; do
        cmp -s $A/$f $B/$f || { echo "differs: $f"; diffs=$((diffs + 1)); }
    done
    cmp -s <(jq -c "$LOGKEYS" $A/log.jsonl) <(jq -c "$LOGKEYS" $B/log.jsonl) || { echo "differs: log.jsonl"; diffs=$((diffs + 1)); }
    for j in $(ls $A/jobs); do
        cmp -s $A/jobs/$j/job.json $B/jobs/$j/job.json || { echo "differs: jobs/$j/job.json"; diffs=$((diffs + 1)); }
    done
    [ "$(ls $A/jobs)" == "$(ls $B/jobs)" ] || { echo "differs: the jobs"; diffs=$((diffs + 1)); }
    say "files: $(ls $A/jobs | wc -l) jobs, $(wc -l < $A/entries.jsonl) entries, $(grep -c . $A/log.jsonl) log lines; $diffs differ"
}

pull() {  # the run's archive back without readings, and the VM's logs and outputs
    local IP; IP=$(ip)
    source $HERE/env.sh
    mkdir -p $D_ARCH
    rsync -a -e "ssh $SSHOPT" --exclude=/readings --exclude='/jobs/*/run' --exclude='/verify-*/' $U@$IP:$D_ARCH/ $D_ARCH/
    rsync -a -e "ssh $SSHOPT" --exclude=/verify $U@$IP:$OUT/ $OUT/vm/
}

follow() {  # the PI's session reads the archive's log.jsonl on the laptop; the archive without readings every 30 min
    source $HERE/env.sh
    mkdir -p $D_ARCH
    local k=0
    while true; do
        local IP; IP=$(ip) || true
        rsync -a -e "ssh $SSHOPT" $U@$IP:$D_ARCH/log.jsonl $D_ARCH/ 2>/dev/null || true
        rsync -a -e "ssh $SSHOPT" $U@$IP:$D_ARCH/stalls.json $U@$IP:$D_ARCH/summary.json $D_ARCH/ 2>/dev/null || true
        k=$((k + 1))
        if [ $((k % 15)) = 0 ]; then
            rsync -a -e "ssh $SSHOPT" --exclude=/readings --exclude='/jobs/*/run' --exclude='/verify-*/' \
                $U@$IP:$D_ARCH/ $D_ARCH/ 2>/dev/null || true
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
    push|pull|pull-check|follow|vm|setup|check|start|run_bg|resume|restart|verify_all|status)
        $(echo $cmd | tr - _) "$@" ;;
    *) sed -n '2,30p' "$0"; exit 2 ;;
esac
