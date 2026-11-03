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
#
# E014 (experiments/E014-fate-demo): another SCUMM v5 game by explicit settings. Unset, the release is picked by MI's
# file names as before and every output is byte for byte the same.
#   SCUMMRP_GAME  scummrp's game id (`scummrp -L` lists its table), e.g. atlantis; picks the v5 path below
#   RELEASE       the output folders' name: build/blocks-RELEASE, build/verb-blocks-RELEASE, data/scripts-RELEASE
#   INDEX_NAME    the index file's name in GAME_DIR, when it differs from the name in scummrp's table (a demo's): a
#                 folder of links, build/scummrp-in-RELEASE, gives scummrp the table's names for the game's files
#   GAME_TITLE    the title in the index and in each file's header (default: MI's text, as before)
#   OUT_ROOT      where build/ and data/ outputs go (default: the repository); GAME_DIR is relative to it
#   TOOLS_ROOT    where scummrp, descumm and their sources are (default: the repository)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS_ROOT="${TOOLS_ROOT:-$ROOT}"
OUT_ROOT="${OUT_ROOT:-$ROOT}"
mkdir -p "$OUT_ROOT/build" "$OUT_ROOT/data"
cd "$OUT_ROOT"

MAKE_JOBS="${MAKE_JOBS:-4}"
GAME_DIR="${GAME_DIR:-game/classic}"
# The layout picks the release: MONKEY1.000 is the Mac release (SCUMM v5), MONKEY.000 the
# DOS CD release (SCUMM v5, same block format), 000.LFL plus
# DISKnn.LEC is the DOS VGA floppy (SCUMM v4). OUT and the block dirs default per release.
if [[ -n "${SCUMMRP_GAME:-}" ]]; then
  : "${RELEASE:?RELEASE names the output folders when SCUMMRP_GAME is set}"
  GAME_ID="$SCUMMRP_GAME"
  INDEX_FILE="$GAME_DIR/${INDEX_NAME:?INDEX_NAME is the index file in GAME_DIR}"
  [[ -f "$INDEX_FILE" ]] || { echo "missing $INDEX_FILE" >&2; exit 1; }
elif [[ -f "$GAME_DIR/000.LFL" || -f "$GAME_DIR/000.lfl" ]]; then
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
SCUMMTR_SRC="$TOOLS_ROOT/build/scummtr"
SCUMMTR_BUILD="$TOOLS_ROOT/build/scummtr-build"
SCUMMRP="$SCUMMTR_BUILD/bin/scummrp"

TOOLS_SRC="$TOOLS_ROOT/third_party/scummvm-tools"
TOOLS_BUILD="$TOOLS_ROOT/build/scummvm-tools"
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
    "$TOOLS_SRC/configure" --disable-wxwidgets --disable-boost \
      --disable-vorbis --disable-tremor --disable-mad --disable-flac \
      --disable-png --disable-freetype2 --disable-iconv --disable-zlib \
      >configure.out 2>&1 || { cat configure.out >&2; exit 1; }
    make -j "$MAKE_JOBS" descumm >/dev/null 2>build.log || { tail -40 build.log >&2; exit 1; }
  )
fi

# ---- export raw blocks ----------------------------------------------------------
log "exporting blocks with scummrp -g $GAME_ID"
rm -rf "$BLOCKS.tmp"
SCUMMRP_DIR="$GAME_DIR"
if [[ -n "${SCUMMRP_GAME:-}" ]]; then
  # scummrp opens the files by its table's names (e.g. ATLANTIS.000, ATLANTIS.001): link the game's files under them
  want="$("$SCUMMRP" -L | awk -F'|' -v g="$GAME_ID" '{gsub(/ /, "", $1); gsub(/ /, "", $3)} $1 == g {print $3}')"
  [[ -n "$want" ]] || { echo "scummrp has no game $GAME_ID" >&2; exit 1; }
  if [[ "$want" != "$INDEX_NAME" ]]; then
    SCUMMRP_DIR="build/scummrp-in-$RELEASE"
    rm -rf "$SCUMMRP_DIR" && mkdir -p "$SCUMMRP_DIR"
    have_stem="${INDEX_NAME%.*}"; want_stem="${want%.*}"
    for f in "$GAME_DIR/$have_stem".*; do
      ln -s "$(cd "$(dirname "$f")" && pwd)/$(basename "$f")" "$SCUMMRP_DIR/$want_stem.${f##*.}"
    done
  fi
fi
"$SCUMMRP" -q -g "$GAME_ID" -p "$SCUMMRP_DIR" -o -d "$BLOCKS.tmp"
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
  uv run --quiet --no-project --python 3.12 --script "$ROOT/scripts/dump_scripts_v4.py" \
    --index-file "$INDEX_FILE" \
    --blocks "$BLOCKS" \
    --descumm "$DESCUMM" \
    --out "$OUT" \
    --jobs "$MAKE_JOBS" \
    --tool-info "$TOOL_INFO"
  log "done: $OUT/INDEX.md"
  exit 0
fi
uv run --quiet --no-project --python 3.12 --script "$ROOT/scripts/dump_scripts.py" \
  ${GAME_TITLE:+--title "$GAME_TITLE"} \
  --index-file "$INDEX_FILE" \
  --blocks "$BLOCKS" \
  --verb-dir "$VERB_BLOCKS" \
  --descumm "$DESCUMM" \
  --out "$OUT" \
  --jobs "$MAKE_JOBS" \
  --tool-info "$TOOL_INFO"
log "done: $OUT/INDEX.md"
