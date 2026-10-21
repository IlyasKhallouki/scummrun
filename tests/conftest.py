import contextlib
import os
from pathlib import Path

import pytest

from speedrun import paths

BRIDGE_MARKER = "speedrun-bridge"


def _skip_reason(report) -> str:
    longrepr = report.longrepr
    if isinstance(longrepr, tuple) and len(longrepr) == 3:
        return str(longrepr[2]).removeprefix("Skipped: ")
    return str(longrepr)


def pytest_terminal_summary(terminalreporter, exitstatus, config):
    skipped = terminalreporter.stats.get("skipped", [])
    if not skipped:
        return
    terminalreporter.write_sep("!", red=True, bold=True)
    terminalreporter.write_line(
        f"!!! {len(skipped)} TESTS SKIPPED — skipped tests are information, not passes !!!",
        red=True,
        bold=True,
    )
    for report in skipped:
        nodeid = getattr(report, "nodeid", None) or "<unknown test>"
        terminalreporter.write_line(f"  {nodeid}: {_skip_reason(report)}", red=True)
    terminalreporter.write_sep("!", red=True, bold=True)


@pytest.fixture(scope="session")
def game_ready() -> Path:
    if not (paths.CLASSIC_DIR / "MONKEY1.000").exists():
        pytest.skip(
            "GAME DATA MISSING: game/classic/MONKEY1.000 not found — "
            "run `uv run speedrun extract` (integration test skipped)"
        )
    return paths.CLASSIC_DIR


@pytest.fixture(scope="session")
def fd_ready() -> Path:
    """The Fast Downward release build directory (``--build`` argument)."""
    # Imported here so a broken planner module fails only the tests that use it.
    from speedrun.planner import FD_BUILD, fd_available

    if not fd_available():
        pytest.skip("FAST DOWNWARD BUILD MISSING: run scripts/build-downward.sh")
    return FD_BUILD


@pytest.fixture(scope="session")
def stock_engine(game_ready) -> Path:
    """The ScummVM binary, patched or not. For tests that run unpatched ScummVM."""
    if not paths.SCUMMVM_BIN.exists():
        pytest.skip(
            "SCUMMVM BUILD MISSING: build/scummvm/scummvm not found — "
            "run scripts/build-scummvm.sh"
        )
    return paths.SCUMMVM_BIN


@pytest.fixture(scope="session")
def dos_vga_ready() -> Path:
    """The DOS VGA floppy data directory (000.LFL + DISK01-04.LEC under game/)."""
    from speedrun import releases

    try:
        return releases.game_dir(releases.get("dos-vga"))
    except releases.ReleaseError as e:
        pytest.skip(f"DOS VGA GAME DATA MISSING: {e} (integration test skipped)")


@pytest.fixture(scope="session")
def engine_ready(game_ready) -> Path:
    """The ScummVM binary, which must carry the speedrun bridge patch."""
    if not paths.SCUMMVM_BIN.exists():
        pytest.skip(
            "SCUMMVM BUILD MISSING: build/scummvm/scummvm not found — "
            "run scripts/build-scummvm.sh"
        )
    # The bridge sources embed BRIDGE_MARKER as a string constant, so a patched
    # binary contains it. Searching the bytes avoids running ScummVM at all.
    if BRIDGE_MARKER.encode() not in paths.SCUMMVM_BIN.read_bytes():
        pytest.skip(
            f"SCUMMVM BUILD IS NOT PATCHED: build/scummvm/scummvm lacks the "
            f"{BRIDGE_MARKER!r} marker — rebuild with scripts/build-scummvm.sh"
        )
    return paths.SCUMMVM_BIN


def home_scummvm_dirs() -> list[Path]:
    """Where ScummVM would write in the user's home: config, data and cache.

    The XDG base dirs are honoured when set, and the ~/.config, ~/.local/share
    and ~/.cache defaults are always checked as well.
    """
    home = Path.home()
    dirs: list[Path] = []
    for var, default in (
        ("XDG_CONFIG_HOME", home / ".config"),
        ("XDG_DATA_HOME", home / ".local" / "share"),
        ("XDG_CACHE_HOME", home / ".cache"),
    ):
        value = os.environ.get(var)
        for base in ((Path(value),) if value else ()) + (default,):
            if base / "scummvm" not in dirs:
                dirs.append(base / "scummvm")
    return dirs


def _snapshot(root: Path):
    """None if absent, else every entry under root with its mtime."""
    if not root.exists():
        return None
    entries = [(".", root.stat().st_mtime_ns)]
    for p in sorted(root.rglob("*")):
        entries.append((str(p.relative_to(root)), p.lstat().st_mtime_ns))
    return entries


@contextlib.contextmanager
def guard_home_scummvm():
    """Fails (on exit) if ScummVM created or modified its dirs in the user's home.

    For fixtures wider than function scope, which run ScummVM before any
    function-scoped ``home_scummvm_guard`` takes its snapshot.
    """
    dirs = home_scummvm_dirs()
    before = {d: _snapshot(d) for d in dirs}
    yield dirs
    for d in dirs:
        after = _snapshot(d)
        if before[d] is None:
            assert after is None, f"ScummVM created {d} in the user's home"
        else:
            assert after == before[d], f"ScummVM modified {d} in the user's home"


@pytest.fixture
def home_scummvm_guard() -> list[Path]:
    """Fails the test if ScummVM created or modified its dirs in the user's home."""
    with guard_home_scummvm() as dirs:
        yield dirs


@pytest.fixture(scope="session")
def home_scummvm_guard_factory():
    """``guard_home_scummvm`` for fixtures of any scope, e.g. ``with home_scummvm_guard_factory(): ...``."""
    return guard_home_scummvm
