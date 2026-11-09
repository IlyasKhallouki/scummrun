#!/bin/bash
# make.sh ROUTE OUT_DIR: a video of a verified route. The route is verified from boot as `fmi shorten verify` does;
# its standalone launch runs through engine-wrap.sh, which turns on the bridge's SPEEDRUN_VIDEO capture there only;
# render.py draws the clicks, the skip keys and a real-time timer (jiffies / 59.998) and aligns the audio.
set -euo pipefail
ROUTE=$(realpath "$1"); OUT=$(realpath -m "$2"); HERE=$(dirname "$(realpath "$0")")
FMI=${FMI:-/home/ilyask/.cache/fast-mi-e024/bin-584e092/fmi}
mkdir -p "$OUT"; rm -rf "$OUT/verify" "$OUT"/cap.*
cp "$ROUTE" "${ROUTE%.jsonl}.steps.jsonl" "$OUT/"
source /var/tmp/fast-mi/wt-e016/experiments/E024-shorten/env.sh
env $ALLON $GAME FMI_ENGINE_BIN="$HERE/engine-wrap.sh" FMI_CAPTURE="$OUT/cap" \
  "$FMI" shorten verify "$OUT/$(basename "$ROUTE")" "$OUT/verify" > "$OUT/verify.json"
uv run --no-project --with numpy --with pillow python -I "$HERE/render.py" "$OUT" "$OUT/video.mp4"
rm -f "$OUT/cap.rgb"
