#!/usr/bin/env bash
# E011's engine build (plan.md, component "engine"): ScummVM v2026.3.0 with patches 0001 and 0002, the profile's slim
# configure line (profile-engine.md section 5), plain make, in its own folder on / . It never builds over svm-w3b and
# never resets a tree: an existing tree is built as it is.
#
# usage: build.sh [--tree DIR] [-j N]     build DIR (default /var/tmp/fast-mi/svm-e011) as it is
#        build.sh --fresh DIR [-j N]      make DIR (must not exist) from the tag and patches 0001 and 0002, then build
#        build.sh --export [--tree DIR]   write patches/0002 from DIR's sources (git diff against tag + 0001)
#
# DIR holds third_party/scummvm (a git tree: the tag, then a commit with 0001) and build/scummvm. The binary is
# DIR/build/scummvm/scummvm.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
PATCHES="$ROOT/patches"
P1="$PATCHES/0001-scumm-add-speedrun-bridge.patch"
P2="$PATCHES/0002-scumm-speedrun-lean.patch"
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
    git -C "$SRC" diff HEAD > "$P2"
    echo "wrote $P2 ($(wc -l < "$P2") lines, sha1 $(sha1sum < "$P2" | cut -c1-8))"
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
