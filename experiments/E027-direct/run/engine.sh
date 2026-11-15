#!/bin/bash
# engine.sh [OUT] [JOBS]: builds the search's engine from the repository alone: ScummVM v2026.3.0 (the pinned submodule
# third_party/scummvm) with every patch in patches/ (0001-0013, "the bridge"), configured with the options of the runs'
# engine (E011's slim build: the SCUMM engine only, FLAC on for the CD audio, the optional libraries off).
#   OUT   default build/svm-direct: OUT/src is the patched source, OUT/build the build, OUT/build/scummvm the engine
#   JOBS  default nproc
# Idempotent: an OUT/src whose patch stamp matches patches/ is only built again. env.sh points FMI_ENGINE_BIN here.
# The runs of 2026-11 used an engine built from a tree with these patches plus a trace hook in no patch file, which no
# run of the search uses; the gate (gate.sh) shows this build gives the same results.
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); ROOT=$(cd "$HERE/../../.." && pwd)
OUT=${1:-$ROOT/build/svm-direct}; JOBS=${2:-$(nproc)}
TAG=v2026.3.0
SUB=$ROOT/third_party/scummvm
CONFIGURE_FLAGS=(--disable-all-engines --enable-engine=scumm --disable-detection-full --enable-optimizations
    --disable-gtk --disable-fluidsynth --disable-fluidlite --disable-libcurl --disable-cloud --disable-sdlnet
    --disable-mt32emu --disable-timidity --disable-seq-midi --disable-alsa --disable-sndio --disable-vorbis
    --disable-ogg --disable-mad --disable-faad --disable-jpeg --disable-png --disable-gif --disable-theoradec
    --disable-vpx --disable-freetype2 --disable-fribidi --disable-readline --disable-libunity --disable-discord
    --disable-tts --disable-system-dialogs --disable-opengl-game --disable-enet --disable-sonivox --disable-bink
    --disable-lua)

# 1. the source: the submodule at the tag, copied to OUT/src
if [[ ! -e "$SUB/.git" ]]; then
  echo "==> fetching the ScummVM submodule"
  git -C "$ROOT" submodule update --init --depth 1 third_party/scummvm
fi
if ! git -C "$SUB" rev-parse -q --verify "refs/tags/$TAG^{commit}" >/dev/null; then
  git -C "$SUB" fetch -q --depth 1 origin tag "$TAG"
fi
STAMP=$(cat "$ROOT"/patches/*.patch | sha256sum | cut -d' ' -f1)
if [[ ! -f "$OUT/src/.patches-stamp" || "$(<"$OUT/src/.patches-stamp")" != "$STAMP" ]]; then
  echo "==> ScummVM $TAG into $OUT/src, with patches/"
  rm -rf "$OUT/src"; mkdir -p "$OUT"
  git clone -q --no-checkout "$SUB" "$OUT/src"
  git -C "$OUT/src" checkout -q "$(git -C "$SUB" rev-parse "refs/tags/$TAG^{commit}")"
  for p in $(LC_ALL=C ls "$ROOT"/patches/*.patch); do
    echo "    $(basename "$p")"
    git -C "$OUT/src" apply --whitespace=nowarn "$p"
  done
  echo "$STAMP" > "$OUT/src/.patches-stamp"
  rm -rf "$OUT/build"
fi

# 2. configure (once) and build
mkdir -p "$OUT/build"
if [[ ! -f "$OUT/build/config.mk" ]]; then
  echo "==> configure"
  (cd "$OUT/build" && ../src/configure "${CONFIGURE_FLAGS[@]}") > "$OUT/build/configure.out" 2>&1 \
    || { tail -20 "$OUT/build/configure.out"; exit 1; }
  grep -q '^USE_FLAC = 1' "$OUT/build/config.mk" \
    || { echo "configure found no FLAC library (libFLAC): the CD audio, and with it the game's timing, would differ" >&2; exit 1; }
fi
echo "==> make -j$JOBS"
make -C "$OUT/build" -j"$JOBS" > "$OUT/build/make.out" 2>&1 || { tail -30 "$OUT/build/make.out"; exit 1; }
ls -la "$OUT/build/scummvm"
