"""Canonical filesystem locations. Every other module imports paths from here."""

import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

GAME_DIR = ROOT / "game"
CLASSIC_DIR = GAME_DIR / "classic"

BUILD_DIR = ROOT / "build"
# FAST_MI_SCUMMVM_BIN: another ScummVM build than the shared one (a bridge change tried in its own build folder
# before the shared build is switched over); unset or empty, the shared build.
SCUMMVM_BIN = Path(os.environ.get("FAST_MI_SCUMMVM_BIN") or BUILD_DIR / "scummvm" / "scummvm")

OUT_DIR = ROOT / "out"
PLANS_DIR = OUT_DIR / "plans"
RUNS_DIR = OUT_DIR / "runs"

PDDL_DIR = ROOT / "pddl"
DOCS_DIR = ROOT / "docs"
DOWNWARD_DIR = ROOT / "third_party" / "downward"
SCRIPTS_DIR = ROOT / "scripts"
DATA_DIR = ROOT / "data"
