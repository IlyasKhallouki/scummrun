#!/usr/bin/env bash
# E013's engine (plan.md, amendment (2) (a)): ScummVM v2026.3.0 with patches 0001-0005, E014's 0006-e014 and E013's
# 0007 (the input log, SPEEDRUN_INPUT=1, and the state digest), built as E011's engine/build.sh builds (the slim
# configure line, plain make, its own folder). It never builds over svm-w3b and never resets a tree.
#
# usage: build.sh --tree DIR [-j N]       build DIR as it is
#        build.sh --fresh DIR [-j N]      make DIR (must not exist) from the tag and patches 0001-0005 and 0006-e014
#                                         (one commit each), apply 0007 on top, then build
#        build.sh --export --tree DIR     write patches/0007 from DIR's sources (git diff against its 0006-e014 commit)
#
# refuse's tree, /home/ilyask/.cache/fast-mi-e013/svm-e013-refuse, was made as a copy of E014's svm-e014 (cp -a: the
# tag, 0001-0005 committed, 0006-e014 applied), with 0006-e014 committed there and 0007 written in it.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
E11="$ROOT/experiments/E011-speed/engine/build.sh"
ORIGIN=/home/ilyask/.cache/fast-mi/svm-w3b/third_party/scummvm # read only: cloned, never touched
TAG=fed42f20 # v2026.3.0, svm-w3b's HEAD
TREE=
FRESH=0
EXPORT=0
JOBS=3
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
[[ -n "$TREE" ]] || { echo "--tree DIR or --fresh DIR" >&2; exit 2; }
case "$TREE" in *svm-w3b*|*svm-e012-end*|*svm-e014) echo "refusing to build in $TREE" >&2; exit 2 ;; esac
SRC="$TREE/third_party/scummvm"
P7="$ROOT/patches/0007-scumm-speedrun-input.patch"
if ((EXPORT)); then
    case "$(git -C "$SRC" log -1 --format=%s)" in "patch 0006-e014"*) ;; *) echo "$SRC: HEAD is not the 0006-e014 commit" >&2; exit 2 ;; esac
    git -C "$SRC" add -N engines/scumm/speedrun
    git -C "$SRC" diff HEAD > "$P7"
    echo "wrote $P7 ($(wc -l < "$P7") lines, sha1 $(sha1sum < "$P7" | cut -c1-8))"
    exit 0
fi
if ((FRESH)); then
    [[ ! -e "$TREE" ]] || { echo "$TREE exists; --fresh makes a new tree only" >&2; exit 2; }
    mkdir -p "$TREE/third_party" "$TREE/build/scummvm"
    git clone -q "$ORIGIN" "$SRC"
    git -C "$SRC" checkout -q "$TAG"
    for p in 0001-scumm-add-speedrun-bridge 0002-scumm-speedrun-lean 0003-scumm-speedrun-cuts 0004-scumm-speedrun-play \
             0005-scumm-speedrun-end 0006-e014-scumm-speedrun-handler; do
        git -C "$SRC" apply --whitespace=nowarn "$ROOT/patches/$p.patch"
        git -C "$SRC" add -A
        case $p in 0006-e014*) m="patch 0006-e014 (speedrun handler)" ;; *) m="patch ${p%%-*} (${p#*-})" ;; esac
        git -C "$SRC" -c user.name=build -c user.email=build@localhost commit -q -m "$m"
    done
    git -C "$SRC" apply --whitespace=nowarn "$P7"
fi
exec "$E11" --tree "$TREE" -j "$JOBS"
