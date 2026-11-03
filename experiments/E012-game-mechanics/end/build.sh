#!/usr/bin/env bash
# E012 component end: the engine with patches 0001-0005, built as E011's engine/build.sh builds (the slim configure
# line, plain make, its own folder on /). It never builds over svm-w3b and never resets a tree.
#
# usage: build.sh [--tree DIR] [-j N]     build DIR (default /var/tmp/fast-mi/svm-e012-end) as it is
#        build.sh --fresh DIR [-j N]      make DIR (must not exist) from the tag and patches 0001-0004 (one commit
#                                         each), apply 0005 on top, then build
#        build.sh --export [--tree DIR]   write patches/0005 from DIR's sources (git diff against its 0004 commit)
#
# The binary is DIR/build/scummvm/scummvm. The component's build was made from a copy of E011 round 2's tree
# (svm-e011-r2int: the tag, 0001, 0002 and 0003 committed, 0004 applied), with 0004 committed and 0005 applied.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
E11="$ROOT/experiments/E011-speed/engine/build.sh"
ORIGIN=/home/ilyask/.cache/fast-mi/svm-w3b/third_party/scummvm # read only: cloned, never touched
TAG=fed42f20 # v2026.3.0, svm-w3b's HEAD
TREE=/var/tmp/fast-mi/svm-e012-end
FRESH=0
EXPORT=0
JOBS=1
while (($#)); do
    case "$1" in
        --tree) TREE="$2"; shift ;;
        --fresh) TREE="$2"; FRESH=1; shift ;;
        --export) EXPORT=1 ;;
        -j) JOBS="$2"; shift ;;
        *) echo "unknown argument $1" >&2; exit 2 ;;
    esac
    shift
done
case "$TREE" in *svm-w3b*) echo "refusing to build in svm-w3b" >&2; exit 2 ;; esac
SRC="$TREE/third_party/scummvm"
P5="$ROOT/patches/0005-scumm-speedrun-end.patch"
if ((EXPORT)); then
    case "$(git -C "$SRC" log -1 --format=%s)" in "patch 0004"*) ;; *) echo "$SRC: HEAD is not the 0004 commit" >&2; exit 2 ;; esac
    git -C "$SRC" diff HEAD > "$P5"
    echo "wrote $P5 ($(wc -l < "$P5") lines, sha1 $(sha1sum < "$P5" | cut -c1-8))"
    exit 0
fi
if ((FRESH)); then
    [[ ! -e "$TREE" ]] || { echo "$TREE exists; --fresh makes a new tree only" >&2; exit 2; }
    mkdir -p "$TREE/third_party" "$TREE/build/scummvm"
    git clone -q "$ORIGIN" "$SRC"
    git -C "$SRC" checkout -q "$TAG"
    for p in 0001-scumm-add-speedrun-bridge 0002-scumm-speedrun-lean 0003-scumm-speedrun-cuts 0004-scumm-speedrun-play; do
        git -C "$SRC" apply --whitespace=nowarn "$ROOT/patches/$p.patch"
        git -C "$SRC" add -A
        git -C "$SRC" -c user.name=build -c user.email=build@localhost commit -q -m "patch ${p%%-*} (${p#*-})"
    done
    git -C "$SRC" apply --whitespace=nowarn "$P5"
fi
exec "$E11" --tree "$TREE" -j "$JOBS"
