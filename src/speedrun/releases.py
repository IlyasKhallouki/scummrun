"""Game releases the pipeline can drive, and where each one's data lives.

MI1 shipped in several releases whose scripts, object ids and timing differ, so
every run, script dump and measurement belongs to exactly one release:

* ``mac``: the Macintosh release (SCUMM v5, ``MONKEY1.000``/``.001``), which the
  glitchless Part I route was built on. ScummVM runs it at 240 quarter-frames
  per second, so a tick (jiffy) is 1/60 s.
* ``dos-vga``: the DOS VGA floppy release (SCUMM v4, ``000.LFL`` plus
  ``DISK01.LEC``-``DISK04.LEC``), ScummVM md5 ``15e03ffbfeddb9c2aebc13dcb2a4a8f4``.
  ScummVM paces it with the original PIT divisor, 1193182/5041 quarter-frames
  per second (``scumm.cpp`` ``setTimerAndShakeFrequency()``), so a tick lasts
  4 * 5041 / 1193182 s, about 1/59.17 s.
* ``dos-cd``: the DOS VGA CD-ROM release (SCUMM v5, ``MONKEY.000``/``.001`` from
  the CD's ENGLISH directory), ScummVM md5 ``2d1e891fe52df707c30185e52c50cd92``,
  with its CD audio tracks as ``trackN.flac`` files beside the data. ScummVM paces
  it at 1193182/4096 x 3433/4167 = 239.992 quarter-frames per second: a tick is
  1/59.998 s. This is the release of the Any% record.

Tick counts from different releases are never compared or pooled; wall-clock
times are, through ``seconds()``.
"""

import os
from dataclasses import dataclass
from pathlib import Path

from speedrun import paths

PIT_BASE_HZ = 1193182.0
PIT_V2_4_DIVISOR = 5041.0
# DOS v5: the orchestrator timer's divisor and sub-timer step/threshold (scumm.h).
PIT_V5_6_ORCHESTRATOR_DIVISOR = 4096.0
PIT_V5_6_SUBTIMER_INC = 3433.0
PIT_V5_SUBTIMER_THRESH = 4167.0


@dataclass(frozen=True)
class Release:
    name: str
    target: str  # the ScummVM target section in the per-run ini
    gameid: str
    platform: str  # ScummVM platform code
    version: int  # SCUMM version
    timer_hz: float  # quarter-frames per second; a tick is 4 / timer_hz seconds
    scripts_dir: Path  # decompiled scripts (scripts/dump-scripts.sh)
    index_names: tuple[str, ...]  # files that identify the release's data directory

    def seconds(self, ticks: float) -> float:
        """Wall-clock seconds for ``ticks`` jiffies of this release."""
        return ticks * 4.0 / self.timer_hz


RELEASES: dict[str, Release] = {
    "mac": Release(
        name="mac",
        target="monkey-mac",
        gameid="monkey",
        platform="macintosh",
        version=5,
        timer_hz=240.0,
        scripts_dir=paths.DATA_DIR / "scripts",
        index_names=("MONKEY1.000", "MONKEY1.001"),
    ),
    "dos-vga": Release(
        name="dos-vga",
        target="monkey-vga",
        gameid="monkey",
        platform="pc",
        version=4,
        timer_hz=PIT_BASE_HZ / PIT_V2_4_DIVISOR,
        scripts_dir=paths.DATA_DIR / "scripts-dos-vga",
        index_names=("000.LFL", "DISK01.LEC"),
    ),
    "dos-cd": Release(
        name="dos-cd",
        target="monkey-cd",
        gameid="monkey",
        platform="pc",
        version=5,
        timer_hz=PIT_BASE_HZ / PIT_V5_6_ORCHESTRATOR_DIVISOR * PIT_V5_6_SUBTIMER_INC / PIT_V5_SUBTIMER_THRESH,
        scripts_dir=paths.DATA_DIR / "scripts-dos-cd",
        index_names=("MONKEY.000", "MONKEY.001", "track2.flac"),
    ),
}

DEFAULT_RELEASE = "mac"


class ReleaseError(Exception):
    pass


def get(name: str) -> Release:
    try:
        return RELEASES[name]
    except KeyError:
        raise ReleaseError(f"unknown release {name!r}; known: {', '.join(sorted(RELEASES))}") from None


def game_dir(release: Release, root: Path | None = None) -> Path:
    """The directory holding ``release``'s data files.

    The Mac release is read from the extraction output (``paths.CLASSIC_DIR``,
    see ``speedrun extract``). The DOS floppy files need no extraction, so the
    first directory under ``game/`` (sorted walk) that holds all of
    ``index_names`` is used as is.
    """
    if release.name == "mac":
        return paths.CLASSIC_DIR
    root = paths.GAME_DIR if root is None else Path(root)
    wanted = {n.lower() for n in release.index_names}
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        if wanted <= {f.lower() for f in filenames}:
            return Path(dirpath)
    raise ReleaseError(f"no {release.name} data under {root}: expected {', '.join(release.index_names)} in one directory")
