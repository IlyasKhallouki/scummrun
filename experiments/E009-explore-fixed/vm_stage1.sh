#!/usr/bin/env bash
# E009 stage 1 on the cloud VM (plan.md, amendment of 2026-11-02): the laptop's files at the laptop's paths, so every
# path in the code, the environment and the archives is the one the laptop uses.
#
# Laptop (the main checkout's scripts/cloud.py keeps the ledger):
#   python3 scripts/cloud.py start --type TYPE --hours H
#   python3 scripts/cloud.py sh --job e009 -- true          (the VM's lock for this job; puts the ssh key on it)
#   bash vm_stage1.sh push                                  copy the files (list below)
#   bash vm_stage1.sh vm setup                              patch the engine's loader path, check, disk and memory
#   bash vm_stage1.sh vm offcheck                           the off check again on the VM (log under $E/logs)
#   bash vm_stage1.sh vm start NAME GOAL STOP_JOBS          one stage in the background (log $E/logs/NAME.log)
#   bash vm_stage1.sh pull NAME                             the archive back, without readings and run folders
#   bash vm_stage1.sh follow NAME...                        copy each archive's log.jsonl back every 120 s
#
# What goes to the VM: the worktree with its links resolved (no .git, out, build, game, third_party), build's
# blocks-dos-cd and verb-blocks-dos-cd, game/dos-cd, the main checkout's .venv and src (the venv's .pth), its Python
# (uv's cpython 3.12), Fast Downward's driver and release build, taskcore, reader-scope, svm-w3b's ScummVM binary and
# every library it loads (ldd, with the laptop's loader), E007's guard.sh and waitmem.sh, E009's guides and scripts.
# The VM's C library is older than the one ScummVM was built against, so setup copies the binary and sets its ELF
# interpreter and library path to the copied loader and libraries (patchelf); setup checks that the code section is
# byte for byte the laptop's.
set -euo pipefail

NAME_VM=fast-mi-1 ZONE=us-central1-a PROJECT=lumind-mvp
WT=/var/tmp/fast-mi/wt-e009
MAIN=/home/ilyask/projects/fast-mi
E=$HOME/.cache/fast-mi-e009
SVM=$HOME/.cache/fast-mi/svm-w3b/build/scummvm/scummvm
LIB=/opt/svm-w3b-lib
FD=$MAIN/third_party/downward
TC=$HOME/.cache/fast-mi-walk/target/release/taskcore
RS=$HOME/.cache/fast-mi-e008/target-scumm/release/reader-scope
PYDIR=$HOME/.local/share/uv/python
SSHOPT="-i $HOME/.ssh/google_compute_engine -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ServerAliveInterval=30 -o LogLevel=ERROR"
U=$(whoami)

say() { echo "[$(date -u +%H:%M:%S)] $*"; }
ip() { gcloud compute instances describe $NAME_VM --zone=$ZONE --project=$PROJECT \
         --format='get(networkInterfaces[0].accessConfigs[0].natIP)'; }
put() { rsync -az -e "ssh $SSHOPT" "$@"; }

push() {
    local IP; IP=$(ip); local V=$U@$IP
    say "dirs on $IP"
    ssh $SSHOPT $V "sudo mkdir -p /var/tmp/fast-mi $LIB && sudo chown $U /var/tmp/fast-mi $LIB && \
        mkdir -p $(dirname $SVM) $(dirname $TC) $(dirname $RS) $HOME/.cache/fast-mi-e007 $E/logs $PYDIR $FD/builds \
        $MAIN $WT/build $WT/game $WT/out"
    say "worktree (links resolved)"
    put -L --exclude=/.git --exclude=/out --exclude=/build --exclude=/game --exclude=/third_party --exclude=/.venv \
        --exclude=__pycache__ --exclude=.pytest_cache $WT/ $V:$WT/
    put -L $WT/build/blocks-dos-cd $WT/build/verb-blocks-dos-cd $V:$WT/build/
    put -L $WT/game/dos-cd $V:$WT/game/
    ssh $SSHOPT $V "ln -sfn $MAIN/.venv $WT/.venv"
    say "Python"
    put $MAIN/.venv $MAIN/src $V:$MAIN/
    put $(readlink -f $PYDIR/cpython-3.12-linux-x86_64-gnu) $PYDIR/cpython-3.12-linux-x86_64-gnu $V:$PYDIR/
    say "Fast Downward, taskcore, reader-scope"
    put $FD/fast-downward.py $FD/driver $V:$FD/
    put $FD/builds/release $V:$FD/builds/
    put $TC $V:$TC
    put $RS $V:$RS
    say "ScummVM and its libraries"
    put $SVM $V:$SVM.laptop
    local list; list=$(mktemp)
    ldd $SVM | awk '/=> \//{print $3}' | sort -u > $list
    grep -q ld-linux $list || { echo "no loader in ldd's list"; exit 1; }
    say "$(wc -l < $list) libraries"
    rsync -aL --no-relative --files-from=$list -e "ssh $SSHOPT" / $V:$LIB/
    rm -f $list
    say "guard, guides, scripts"
    put $HOME/.cache/fast-mi-e007/guard.sh $HOME/.cache/fast-mi-e007/waitmem.sh $V:$HOME/.cache/fast-mi-e007/
    put $E/guides $E/scripts $E/env.sh $V:$E/
    say "pushed"
}

# --- on the VM -------------------------------------------------------------------------------------------------------

vm_env() {
    source $E/env.sh
    # The VM's machine settings (plan.md amendment of 2026-11-02), read by explore.py's machine()
    export E008_FLOOR_GB=${E008_FLOOR_GB:-8} E008_MEM_SECOND_GB=${E008_MEM_SECOND_GB:-7}
}

setup() {
    say "packages"
    sudo apt-get install -yq patchelf binutils >/dev/null
    cp $SVM.laptop $SVM
    patchelf --set-interpreter $LIB/ld-linux-x86-64.so.2 --force-rpath --set-rpath $LIB $SVM
    local a b t; t=$(mktemp -d)
    objcopy -O binary --only-section=.text $SVM.laptop $t/a; objcopy -O binary --only-section=.text $SVM $t/b
    a=$(sha1sum < $t/a | cut -c1-12); b=$(sha1sum < $t/b | cut -c1-12); rm -rf $t
    say ".text laptop $a, patched $b"
    [ "$a" = "$b" ] || { echo "the code section differs"; exit 1; }
    say "loader: $(readelf -l $SVM | grep -o 'interpreter: [^]]*')"
    say "missing libraries: $(ldd $SVM | grep -c 'not found' || true)"
    $SVM --version | head -2
    $WT/.venv/bin/python -c 'import sys; print(sys.version)'
    $TC --help >/dev/null 2>&1; echo "taskcore rc $?"
    $FD/builds/release/bin/downward --help >/dev/null 2>&1; echo "downward rc $?"
    nproc; free -g | head -2; df -h /home /var/tmp /dev/shm
}

offcheck() {
    vm_env
    export CHECK_OUT=$E/off/step-a-vm
    cd $WT/experiments/E008-go-explore
    { $PY -u $E/scripts/off_a.py; echo "off_a exit $?"
      cd $WT && E008_ENGINE=1 $PY -m pytest -q -rA experiments/E008-go-explore/test_e009.py -k engine
      echo "idle engine test exit $?"; } > $E/logs/off-a-vm.log 2>&1
    grep -e '^CHECK' -e 'exit' $E/logs/off-a-vm.log
}

stage() {  # NAME GOAL STOP_JOBS: as $E/stage1.sh, with the stage's own heavy lock
    vm_env
    local name=$1 goal=$2 stop=$3
    export E008_HEAVY_LOCK=/var/tmp/fast-mi/heavy-$name.lock
    mkdir -p $E/archives
    cd $WT/experiments/E008-go-explore
    $PY explore.py init $E/archives/$name --stage $name --goal $goal --guide $E/guides/$name.json --allowed k2 \
        --idle-frames 1600 --rotate --evict-by-size --reuse-nonfacts --stop-jobs $stop || exit 1
    nice -n 10 $PY -u explore.py run $E/archives/$name --workers 2
    echo "run exit $?"
    $PY explore.py verify $E/archives/$name; echo "verify exit $?"
    $PY explore.py report $E/archives/$name > $E/logs/$name.report; echo "report exit $?"
}

start() {  # NAME GOAL STOP_JOBS, detached from the ssh session
    mkdir -p $E/logs
    setsid nohup bash $WT/experiments/E009-explore-fixed/vm_stage1.sh stage "$@" > $E/logs/$1.log 2>&1 < /dev/null &
    say "started $1 (pid $!)"
}

pull() {  # NAME
    local IP; IP=$(ip)
    mkdir -p $E/archives/$1
    rsync -a -e "ssh $SSHOPT" --exclude=/readings --exclude='/jobs/*/run' $U@$IP:$E/archives/$1/ $E/archives/$1/
    rsync -a -e "ssh $SSHOPT" $U@$IP:$E/logs/ $E/logs/vm/
}

follow() {  # NAME...: the PI's session reads each archive's log.jsonl on the laptop
    while true; do
        local IP; IP=$(ip) || true
        for n in "$@"; do
            mkdir -p $E/archives/$n
            rsync -a -e "ssh $SSHOPT" $U@$IP:$E/archives/$n/log.jsonl $U@$IP:$E/archives/$n/summary.json \
                $E/archives/$n/ 2>/dev/null || true
        done
        sleep 120
    done
}

vm() {  # run a subcommand of this script on the VM
    local IP; IP=$(ip)
    ssh $SSHOPT $U@$IP "bash $WT/experiments/E009-explore-fixed/vm_stage1.sh $*"
}

cmd=${1:-}; shift || true
case "$cmd" in
    push|pull|follow|vm|setup|offcheck|stage|start) $cmd "$@" ;;
    *) sed -n '2,20p' "$0"; exit 2 ;;
esac
