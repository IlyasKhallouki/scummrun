#!/usr/bin/env bash
# E011's engine build (plan.md, component "engine"): ScummVM v2026.3.0 with patches 0001 and 0002 (round 1) and 0003
# (round 2, the cuts: CUTS.md), the profile's slim configure line (profile-engine.md section 5), plain make, in its own
# folder on / . It never builds over svm-w3b and never resets a tree: an existing tree is built as it is.
#
# usage: build.sh [--tree DIR] [-j N]     build DIR (default /var/tmp/fast-mi/svm-e011) as it is
#        build.sh --fresh DIR [-j N]      make DIR (must not exist) from the tag and patches 0001, 0002 and 0003, then
#                                         build
#        build.sh --export [--tree DIR]   write the tree's top patch from DIR's sources: patches/0003 (git diff against
#                                         tag + 0001 + 0002) in a tree whose HEAD is the 0002 commit, else patches/0002
#                                         (git diff against tag + 0001), as round 1's tree svm-e011 has it
#
# DIR holds third_party/scummvm (a git tree: the tag, a commit with 0001, in round 2's trees a commit with 0002) and
# build/scummvm. The binary is DIR/build/scummvm/scummvm. Round 2's build: --tree /var/tmp/fast-mi/svm-e011-r2 (a copy
# of svm-e011's tree, 0002 committed, 0003 applied; its binary is FMI_ENGINE_BIN's with the cuts).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
PATCHES="$ROOT/patches"
P1="$PATCHES/0001-scumm-add-speedrun-bridge.patch"
P2="$PATCHES/0002-scumm-speedrun-lean.patch"
P3="$PATCHES/0003-scumm-speedrun-cuts.patch"
ORIGIN=/home/ilyask/.cache/fast-mi/svm-w3b/third_party/scummvm # read only: cloned, never touched
TAG=fed42f20 # v2026.3.0, svm-w3b's HEAD
TREE=/var/tmp/fast-mi/svm-e011
FRESH=0
EXPORT=0
JOBS=3

CONFIGURE_FLAGS=(--disable-all-engines --enable-engine=scumm --disable-detection-full --enable-optimizations
    --disable-gtk --disable-fluidsynth --disable-fluidlite --disable-libcurl --disable-cloud --disable-sdlnet
    --disable-mt32emu --disable-timidity --disable-seq-midi --disable-alsa --disable-sndio --disable-vorbis
    --disable-ogg --disable-mad --disable-faad --disable-jpeg --disable-png --disable-gif --disable-theoradec
    --disable-vpx --disable-freetype2 --disable-fribidi --disable-readline --disable-libunity --disable-discord
    --disable-tts --disable-system-dialogs --disable-opengl-game --disable-enet --disable-sonivox --disable-bink
    --disable-lua)

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
case "$TREE" in
    *svm-w3b*) echo "refusing to build in svm-w3b" >&2; exit 2 ;;
esac
SRC="$TREE/third_party/scummvm"
BUILD="$TREE/build/scummvm"

if ((EXPORT)); then
    # new files must be in the index (git add -N) to show in the diff, as for 0001
    git -C "$SRC" add -N engines/scumm/speedrun
    TOP="$P2"
    case "$(git -C "$SRC" log -1 --format=%s)" in "patch 0002"*) TOP="$P3" ;; esac
    git -C "$SRC" diff HEAD > "$TOP"
    echo "wrote $TOP ($(wc -l < "$TOP") lines, sha1 $(sha1sum < "$TOP" | cut -c1-8))"
    exit 0
fi

if ((FRESH)); then
    [[ ! -e "$TREE" ]] || { echo "$TREE exists; --fresh makes a new tree only" >&2; exit 2; }
    mkdir -p "$TREE/third_party" "$BUILD"
    git clone -q "$ORIGIN" "$SRC"
    git -C "$SRC" checkout -q "$TAG"
    git -C "$SRC" apply --whitespace=nowarn "$P1"
    git -C "$SRC" add -A
    git -C "$SRC" -c user.name=build -c user.email=build@localhost commit -q -m "patch 0001 (speedrun bridge)"
    git -C "$SRC" apply --whitespace=nowarn "$P2"
    if [[ -f "$P3" ]]; then
        git -C "$SRC" add -A
        git -C "$SRC" -c user.name=build -c user.email=build@localhost commit -q -m "patch 0002 (speedrun lean)"
        git -C "$SRC" apply --whitespace=nowarn "$P3"
    fi
fi

[[ -d "$SRC" ]] || { echo "no source tree at $SRC" >&2; exit 2; }
mkdir -p "$BUILD"
stamp="${CONFIGURE_FLAGS[*]}"
if [[ ! -f "$BUILD/config.mk" || ! -f "$BUILD/.config-stamp" || "$(<"$BUILD/.config-stamp")" != "$stamp" ]]; then
    echo "==> configure (slim)"
    (cd "$BUILD" && ../../third_party/scummvm/configure "${CONFIGURE_FLAGS[@]}") > "$BUILD/configure.out" 2>&1
    printf '%s\n' "$stamp" > "$BUILD/.config-stamp"
fi
echo "==> make -j$JOBS in $BUILD"
nice -n 10 make -C "$BUILD" -j"$JOBS" > "$BUILD/make.out" 2>&1 || { tail -30 "$BUILD/make.out"; exit 1; }
tail -2 "$BUILD/make.out"
ls -la "$BUILD/scummvm"
