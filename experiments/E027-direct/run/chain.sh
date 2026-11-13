#!/bin/bash
# chain.sh OUT [chain.py's options]: chain.py with env.sh's settings (E027, Monkey Island)
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$HERE/env.sh"
ulimit -n 65536 2>/dev/null || true
export ON MI GUIDE27 RUNGS27
exec python3 "$HERE/chain.py" "$@"
