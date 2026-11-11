# E027: the settings of the direct search's runs and checks. E026's settings (the engine's switches, each game's
# files), with E027's engine: svm-e027 is svm-e024 with patch 0011 (SPEEDRUN_VERB_IMAGES: an image verb's name is
# its image's hash). The search sets that switch itself; with an engine that lacks the patch a page of inventory
# images is not seen.
#   FMI_ENGINE_E027=PATH   another build of the engine
ROOT27=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$ROOT27/experiments/E026-part1/run/env.sh"
export FMI_ENGINE_BIN=${FMI_ENGINE_E027:-/var/tmp/fast-mi/svm-e027/build/scummvm/scummvm}
GUIDE27=$HOME/.cache/fast-mi-e026/size-cost/guide-part1.json
RUNGS27=$ROOT27/experiments/E011-speed/rs/fmi-checks/rungs/rungs-part1.json
