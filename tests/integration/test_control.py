"""The control socket (C9) and raw input (C8) on the DOS VGA floppy release."""

import pytest

from speedrun.control import Controller
from speedrun.engine import EngineConfig

LOGO_ESC = {"frame": 45, "key": "esc"}  # inside the logo-glitch window (docs/anypercent/findings.md)


def _cfg(tmp_path, game):
    return EngineConfig(out_dir=tmp_path / "run", game_path=game, release="dos-vga", timer_next="any",
                        start_frame=45, max_ticks=100000)


@pytest.mark.integration
def test_logo_glitch_and_dock_control(engine_ready, dos_vga_ready, tmp_path, home_scummvm_guard):
    with Controller(_cfg(tmp_path, dos_vga_ready)) as c:
        root = c.root
        root.inputs([LOGO_ESC])
        st = root.run(control=True, max_frame=400)
        assert st["stop"] == "control"
        assert st["room"] == 33 and st["v19"] == 5
        assert st["seg"] and st["tick0"] == 269
        assert st["qf"] == 4 * st["tick"]  # no room-change effect on the way


@pytest.mark.integration
def test_branch_matches_straight_replay(engine_ready, dos_vga_ready, tmp_path, home_scummvm_guard):
    click = {"frame": 60, "click": [250, 130]}
    with Controller(_cfg(tmp_path, dos_vga_ready)) as c:
        root = c.root
        root.inputs([LOGO_ESC])
        root.run(frame=55)
        other = c.fork(root)
        other.inputs([{"frame": 60, "click": [100, 128]}])
        other.run(frame=200)
        twin = c.fork(root)
        twin.inputs([click])
        twin.run(frame=200)
        branch_state = twin.state()
        twin.quit()
        other_state = other.state()
        other.quit()
        root.inputs([click])
        root.run(frame=200)
        straight = root.state()
    assert branch_state == straight
    assert other_state["ego_pos"] != straight["ego_pos"]


@pytest.mark.integration
def test_executor_dock_to_bar_and_movie_replays(engine_ready, dos_vga_ready, tmp_path, home_scummvm_guard):
    import json

    from speedrun.executor import Executor, Move
    from speedrun.probe import probe

    route = [
        Move("open bar door", verb=1, obj=437, until=[{"state": 437, "eq": 1}], max_frames=600),
        Move("enter bar", obj=437, until=[{"room": 28}], max_frames=300),
    ]
    with Controller(_cfg(tmp_path, dos_vga_ready)) as c:
        ex = Executor(c.root)
        ex._schedule([LOGO_ESC])
        ex.play_all(route)
        done = ex.results[-1].done_frame
    movie = tmp_path / "movie.jsonl"
    movie.write_text("".join(json.dumps(e) + "\n" for e in ex.movie))
    # The movie alone, replayed from boot, reaches the bar on the same frame.
    res = probe(EngineConfig(out_dir=tmp_path / "replay", game_path=dos_vga_ready, release="dos-vga",
                             timer_next="any", start_frame=45, raw_inputs=movie, goal=[{"room": 28}],
                             max_ticks=20000))
    goal = next(r for r in res.records if r["type"] == "goal")
    # The room changes after frame N's decision point: the executor's `until` sees it at
    # frame N + 1's decision point, and the goal record is stamped with frame N (C2).
    assert goal["frame"] + 1 == done


@pytest.fixture(scope="session")
def dos_cd_ready():
    from speedrun import releases

    try:
        return releases.game_dir(releases.get("dos-cd"))
    except releases.ReleaseError as e:
        pytest.skip(f"DOS CD GAME DATA MISSING: {e} (integration test skipped)")


@pytest.mark.integration
def test_cd_branch_matches_straight_replay_with_music(engine_ready, dos_cd_ready, tmp_path, home_scummvm_guard):
    """On DOS CD the music timer is driven by game ticks and track files are read from memory,
    so a branch forked while track 17 plays replays exactly like the straight run."""
    cfg = EngineConfig(out_dir=tmp_path / "run", game_path=dos_cd_ready, release="dos-cd", timer_next="any",
                       start_frame=50, max_ticks=10**6)
    esc = {"frame": 50, "key": "esc"}  # inside the CD logo-glitch window (frames 46-92)
    with Controller(cfg) as c:
        root = c.root
        root.run(frame=30)  # track 17 started on frame 3
        twin = c.fork(root)
        twin.inputs([esc])
        st = twin.run(control=True, max_frame=3000)
        branch_state = twin.state()
        twin.quit()
        root.inputs([esc])
        straight_stop = root.run(control=True, max_frame=3000)
        straight = root.state()
    assert st["v19"] == 5 and st["room"] == 38
    assert (st["frame"], st["qf"]) == (straight_stop["frame"], straight_stop["qf"])
    assert branch_state == straight
