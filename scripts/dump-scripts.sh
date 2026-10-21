#!/usr/bin/env bash
# Decompile every script of The Secret of Monkey Island: the Mac release (SCUMM v5)
# or the DOS VGA floppy (SCUMM v4), picked by the files in GAME_DIR.
#
#   build/scummtr/            ScummTR source (pinned commit), provides scummrp
#   build/scummtr-build/      scummrp build (CMake, out of tree)
#   build/scummvm-tools/      descumm build (out of tree from third_party/scummvm-tools)
#   build/blocks/             raw blocks exported by `scummrp -o`
#   build/verb-blocks/        VERB sub-blocks sliced out of each OBCD (descumm input)
#   data/scripts/             decompiled scripts + index.json + INDEX.md
#
# Idempotent: tools are only built when missing; blocks and data/scripts are
# regenerated into temporary directories and swapped in.
#
# Env overrides: MAKE_JOBS (default 4), GAME_DIR (default game/classic).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MAKE_JOBS="${MAKE_JOBS:-4}"
GAME_DIR="${GAME_DIR:-game/classic}"
# The layout picks the release: MONKEY1.000 is the Mac release (SCUMM v5), MONKEY.000 the
# DOS CD release (SCUMM v5, same block format), 000.LFL plus
# DISKnn.LEC is the DOS VGA floppy (SCUMM v4). OUT and the block dirs default per release.
if [[ -f "$GAME_DIR/000.LFL" || -f "$GAME_DIR/000.lfl" ]]; then
  RELEASE="dos-vga"
  GAME_ID="monkey"               # scummrp id for 000.LFL / DISKnn.LEC (SCUMM v4)
  INDEX_FILE="$(ls "$GAME_DIR"/000.[Ll][Ff][Ll] | head -1)"
elif [[ -f "$GAME_DIR/MONKEY.000" ]]; then
  RELEASE="dos-cd"
  GAME_ID="monkeycd"             # scummrp id for MONKEY.000 / MONKEY.001 (SCUMM v5, XOR 0x69)
  INDEX_FILE="$GAME_DIR/MONKEY.000"
else
  RELEASE="mac"
  GAME_ID="monkeycdalt"          # scummrp id for MONKEY1.000 / MONKEY1.00x (SCUMM v5, XOR 0x69)
  INDEX_FILE="$GAME_DIR/MONKEY1.000"
fi

SCUMMTR_URL="https://github.com/dwatteau/scummtr.git"
SCUMMTR_REF="a50c6f4bf03566721b3f2c00df257a41a82e22ed"
SCUMMTR_SRC="build/scummtr"
SCUMMTR_BUILD="build/scummtr-build"
SCUMMRP="$SCUMMTR_BUILD/bin/scummrp"

TOOLS_SRC="third_party/scummvm-tools"
TOOLS_BUILD="build/scummvm-tools"
DESCUMM="$TOOLS_BUILD/descumm"

if [[ "$RELEASE" == "mac" ]]; then
  BLOCKS="build/blocks"
  VERB_BLOCKS="build/verb-blocks"
  OUT="${OUT:-data/scripts}"
else
  BLOCKS="build/blocks-$RELEASE"
  VERB_BLOCKS="build/verb-blocks-$RELEASE"
  OUT="${OUT:-data/scripts-$RELEASE}"
fi

log() { printf '==> %s\n' "$*" >&2; }

if [[ "$RELEASE" == "mac" ]]; then
  for f in MONKEY1.000 MONKEY1.001; do
    [[ -f "$GAME_DIR/$f" ]] || { echo "missing $GAME_DIR/$f" >&2; exit 1; }
  done
fi

# ---- scummrp (ScummTR) ------------------------------------------------------
if [[ ! -d "$SCUMMTR_SRC/.git" ]]; then
  log "cloning ScummTR into $SCUMMTR_SRC"
  git clone --quiet "$SCUMMTR_URL" "$SCUMMTR_SRC"
fi
if [[ "$(git -C "$SCUMMTR_SRC" rev-parse HEAD)" != "$SCUMMTR_REF" ]]; then
  log "checking out ScummTR $SCUMMTR_REF"
  git -C "$SCUMMTR_SRC" fetch --quiet origin
  git -C "$SCUMMTR_SRC" checkout --quiet --detach "$SCUMMTR_REF"
  rm -f "$SCUMMRP"
fi
if [[ ! -x "$SCUMMRP" ]]; then
  log "building scummrp"
  cmake -S "$SCUMMTR_SRC" -B "$SCUMMTR_BUILD" -DCMAKE_BUILD_TYPE=Release >/dev/null
  cmake --build "$SCUMMTR_BUILD" --target scummrp -j "$MAKE_JOBS" >/dev/null
fi

# ---- descumm (scummvm-tools, out-of-tree build; the submodule is never written) --
if [[ ! -x "$DESCUMM" ]]; then
  log "building descumm"
  mkdir -p "$TOOLS_BUILD"
  (
    cd "$TOOLS_BUILD"
    # configure writes config.h/config.mk/Makefile/config.log into the cwd and
    # sets srcdir to its own location, so this is a clean out-of-tree build.
    "$ROOT/$TOOLS_SRC/configure" --disable-wxwidgets --disable-boost \
      --disable-vorbis --disable-tremor --disable-mad --disable-flac \
      --disable-png --disable-freetype2 --disable-iconv --disable-zlib \
      >configure.out 2>&1 || { cat configure.out >&2; exit 1; }
    make -j "$MAKE_JOBS" descumm >/dev/null 2>build.log || { tail -40 build.log >&2; exit 1; }
  )
fi

# ---- export raw blocks ----------------------------------------------------------
log "exporting blocks with scummrp -g $GAME_ID"
rm -rf "$BLOCKS.tmp"
"$SCUMMRP" -q -g "$GAME_ID" -p "$GAME_DIR" -o -d "$BLOCKS.tmp"
rm -rf "$BLOCKS"
mv "$BLOCKS.tmp" "$BLOCKS"

# ---- tool provenance for the index -------------------------------------------
TOOL_INFO="build/dump-scripts-tools.json"
cat >"$TOOL_INFO" <<EOF
{
 "scummrp": {"project": "ScummTR", "url": "$SCUMMTR_URL",
             "commit": "$(git -C "$SCUMMTR_SRC" rev-parse HEAD)",
             "describe": "$(git -C "$SCUMMTR_SRC" describe --tags --always)",
             "game_id": "$GAME_ID"},
 "descumm": {"project": "scummvm-tools",
             "commit": "$(git -C "$TOOLS_SRC" rev-parse HEAD)",
             "describe": "$(git -C "$TOOLS_SRC" describe --tags --always)"}
}
EOF

# ---- decompile + index ------------------------------------------------------------
log "decompiling into $OUT"
if [[ "$RELEASE" == "dos-vga" ]]; then
  uv run --quiet --no-project --python 3.12 --script scripts/dump_scripts_v4.py \
    --index-file "$INDEX_FILE" \
    --blocks "$BLOCKS" \
    --descumm "$DESCUMM" \
    --out "$OUT" \
    --jobs "$MAKE_JOBS" \
    --tool-info "$TOOL_INFO"
  log "done: $OUT/INDEX.md"
  exit 0
fi
uv run --quiet --no-project --python 3.12 --script scripts/dump_scripts.py \
  --index-file "$INDEX_FILE" \
  --blocks "$BLOCKS" \
  --verb-dir "$VERB_BLOCKS" \
  --descumm "$DESCUMM" \
  --out "$OUT" \
  --jobs "$MAKE_JOBS" \
  --tool-info "$TOOL_INFO"
log "done: $OUT/INDEX.md"
