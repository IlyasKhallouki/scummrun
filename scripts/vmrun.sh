#!/bin/bash
# vmrun.sh: run one heavy job on the project VM (fast-mi-1) at the laptop's own absolute paths, so a script written
# for the laptop runs there unchanged, with at most VM_SLOTS slots taken at once (default 6; a job takes -w of them).
#
# usage: scripts/vmrun.sh [-w N] [-u PATH]... [-b PATH]... -- CMD [ARG]...
#   -u PATH  copy PATH (absolute: a file or a folder) to the same path on the VM first, adding and updating, never
#            deleting there; a worktree (a folder holding .git) is mirrored instead, without target/ and .git
#   -b PATH  copy PATH from the VM back to the laptop after the job (also when it fails or is stopped), adding and
#            updating, never deleting on the laptop
#   -w N     slots the job takes (default 1): a try with W workers or a cargo build takes more
# The command runs in the caller's working folder (which must exist on the VM), in bash, with cargo on PATH; the
# caller's environment does not travel, so variables go in the command (env A=1 ./x.sh) or in the scripts it runs.
# Stopping vmrun (TERM, INT) stops the job's process group on the VM. Exit code: the command's.
# The VM's address is read from $VMRUN_IP or ~/.cache/fast-mi-e026/vm/ip; start the VM with scripts/cloud.py.
set -u
W=1; UP=(); BACK=()
while (($#)); do
    case "$1" in
        -w) W=$2; shift 2 ;;
        -u) UP+=("$2"); shift 2 ;;
        -b) BACK+=("$2"); shift 2 ;;
        --) shift; break ;;
        *) echo "vmrun: unknown argument $1 (the command goes after --)" >&2; exit 2 ;;
    esac
done
(($#)) || { echo "vmrun: no command" >&2; exit 2; }
IP=${VMRUN_IP:-$(cat ~/.cache/fast-mi-e026/vm/ip 2>/dev/null)}
[ -n "$IP" ] || { echo "vmrun: no VM address (~/.cache/fast-mi-e026/vm/ip)" >&2; exit 2; }
O=(-i "$HOME/.ssh/google_compute_engine" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
   -o LogLevel=ERROR -o ServerAliveInterval=30 -o ConnectTimeout=30 -o ConnectionAttempts=5)
SSH=(ssh "${O[@]}" "ilyask@$IP")
RS=(rsync -az --compress-choice=zstd --info=stats0 -e "ssh ${O[*]}")

for p in "${UP[@]}"; do
    [[ $p == /* ]] || { echo "vmrun: -u $p is not absolute" >&2; exit 2; }
    "${SSH[@]}" "mkdir -p '$(dirname "$p")'" || exit 255
    if [ -d "$p" ] && [ -e "$p/.git" ]; then
        "${RS[@]}" --delete --exclude=target/ --exclude=.git "$p/" "ilyask@$IP:$p/" || exit 255
    elif [ -d "$p" ]; then
        "${RS[@]}" "$p/" "ilyask@$IP:$p/" || exit 255
    else
        "${RS[@]}" "$p" "ilyask@$IP:$p" || exit 255
    fi
done

ID=$(date +%s%N)-$$
back() {
    local p
    for p in "${BACK[@]}"; do
        mkdir -p "$(dirname "$p")"
        if "${SSH[@]}" "test -d '$p'"; then "${RS[@]}" "ilyask@$IP:$p/" "$p/"; else "${RS[@]}" "ilyask@$IP:$p" "$p"; fi
    done
}
stop() {
    "${SSH[@]}" "p=\$(cat ~/vmjobs/$ID.pid 2>/dev/null) && kill -TERM -\$p 2>/dev/null; sleep 2; kill -KILL -\$p 2>/dev/null; true"
    back
    exit 143
}
trap stop TERM INT HUP

CMD=$(printf '%q ' "$@")
SLOTS=${VM_SLOTS:-6}
# the slots are lock files held by this job's shell on the VM: the kernel frees them when it ends, however it ends;
# a job takes all W or none, so two jobs never wait on each other's half; jobs take their slots in turn
"${SSH[@]}" bash -s <<EOF &
mkdir -p ~/vmslots ~/vmjobs
# the turnstile: a job holds it until it has all its slots, so a job that needs many is not starved by single ones
exec {turn}>~/vmslots/turn
/usr/bin/flock \$turn
got=()
while :; do
    got=()
    for i in \$(seq 0 \$(($SLOTS - 1))); do
        exec {fd}>~/vmslots/\$i
        if /usr/bin/flock -n \$fd; then got+=(\$fd); else exec {fd}>&-; fi
        [ \${#got[@]} -ge $W ] && break
    done
    [ \${#got[@]} -ge $W ] && break
    for fd in "\${got[@]}"; do exec {fd}>&-; done
    sleep 3
done
exec {turn}>&-
cd $(printf '%q' "$PWD") || exit 2
export PATH=\$HOME/.cargo/bin:\$HOME/.local/bin:\$PATH
setsid bash -c $(printf '%q' "$CMD") < /dev/null &
pid=\$!
echo \$pid > ~/vmjobs/$ID.pid
wait \$pid; rc=\$?
rm -f ~/vmjobs/$ID.pid
exit \$rc
EOF
job=$!
wait $job; rc=$?
back
exit $rc
