import pytest

from speedrun import engine, releases
from speedrun.engine import EngineConfig


def test_dos_vga_tick_is_the_pit_jiffy():
    rel = releases.get("dos-vga")
    # 4 quarter-frames of 5041/1193182 s each (scumm.cpp setTimerAndShakeFrequency()).
    assert rel.seconds(1) == pytest.approx(4 * 5041 / 1193182)
    assert 59.1 < 1 / rel.seconds(1) < 59.2


def test_mac_tick_is_a_sixtieth():
    assert releases.get("mac").seconds(60) == pytest.approx(1.0)


def test_unknown_release_is_an_error():
    with pytest.raises(releases.ReleaseError, match="unknown release"):
        releases.get("amiga")


def test_game_dir_finds_the_floppy_files(tmp_path):
    d = tmp_path / "some" / "MONKEY"
    d.mkdir(parents=True)
    for name in ("000.LFL", "disk01.lec", "DISK02.LEC"):
        (d / name).write_bytes(b"")
    assert releases.game_dir(releases.get("dos-vga"), tmp_path) == d


def test_game_dir_reports_missing_floppy_files(tmp_path):
    (tmp_path / "000.LFL").write_bytes(b"")
    with pytest.raises(releases.ReleaseError, match="DISK01.LEC"):
        releases.game_dir(releases.get("dos-vga"), tmp_path)


def test_ini_and_argv_follow_the_release(tmp_path, monkeypatch):
    monkeypatch.setattr(engine.paths, "OUT_DIR", tmp_path / "out")
    cfg = EngineConfig(out_dir=tmp_path / "run", game_path=tmp_path, release="dos-vga",
                       game_settings={"music_driver": "adlib", "talkspeed": "200"})
    text = engine._ini_text(cfg)
    assert "[monkey-vga]" in text and "[monkey-mac]" not in text
    assert "platform=pc" in text
    assert "music_driver=adlib" in text
    assert "talkspeed=200" in text and "talkspeed=255" not in text
    assert engine.build_argv(cfg)[-1] == "monkey-vga"


def test_anypercent_env(tmp_path):
    inputs = tmp_path / "in.jsonl"
    cfg = EngineConfig(out_dir=tmp_path, raw_inputs=inputs, timer_next="any", start_frame=45, frame_log=True)
    env = engine.build_env(cfg, base_env={})
    assert env["SPEEDRUN_INPUTS"] == str(inputs.resolve())
    assert env["SPEEDRUN_TIMER_NEXT"] == "any"
    assert env["SPEEDRUN_START_FRAME"] == "45"
    assert env["SPEEDRUN_FRAME_LOG"] == "1"


def test_glitchless_env_has_no_anypercent_keys(tmp_path):
    env = engine.build_env(EngineConfig(out_dir=tmp_path), base_env={})
    assert not {"SPEEDRUN_INPUTS", "SPEEDRUN_TIMER_NEXT", "SPEEDRUN_START_FRAME", "SPEEDRUN_FRAME_LOG"} & set(env)
