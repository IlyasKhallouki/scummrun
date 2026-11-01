"""Tests of durations.py, the duration rules in game time (durations.md): each rule of the rule table on a hand-made
wait table, D4's closed form against the engine's countdown, and the bounds file read back."""

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import durations as DU  # noqa: E402

TABLE = """# wait-table: dmax 5, charinc Some(0)
scope\t28,41\t110
const\tvar:19\t5\t0
const\tvar:57\t46\t0
wait\tlocal:208\t28\t10\tbreakHere\tbreakHere: 1
wait\tlocal:208\t41\t10\tdelay\tdelay: 13
wait\tlocal:209\t41\t3\tWaitForMessage\tmessage: 25
wait\tscript:2\t-\t736\tWaitForActor\twait: 1
wait\tscript:3\t-\t5\tWaitForCamera\twait: 1
recursive\tlocal:208\t28\tsome cite
walk\tscript:2\t-\t736\tlocal:3\tlocal:4
"""


def table(tmp_path, text=TABLE):
    p = tmp_path / "w.tsv"
    p.write_text(text)
    return DU.WaitTable(p)


def countdown(n: int, deltas) -> tuple:
    """The engine's delay (scumm.cpp:3129-3132, script.cpp:1513-1542): frames until the slot runs again, and the
    quarter-frames those frames cost (4 * max(1, V) each, D1)."""
    left, frames, qf = n, 0, 0
    for v in deltas:
        frames += 1
        qf += 4 * max(1, v)
        left -= min(15, max(1, v))
        if left < 0:
            return frames, qf
    raise AssertionError("the delay did not end")


def test_constants(tmp_path):
    wt = table(tmp_path)
    assert wt.dmax == 5 and DU.delta_lo(wt, {}) == 5 and DU.talkstop_on(wt)
    wt2 = table(tmp_path, TABLE.replace("const\tvar:19\t5\t0", "const\tvar:19\t5\t1"))
    assert DU.delta_lo(wt2, {}) == 1  # written, not a fact
    assert DU.delta_lo(wt2, {"var:19": ["6", "3", "8"]}) == 3
    assert DU.delta_lo(wt2, {"var:19": ["below", "3"]}) == 1
    wt3 = table(tmp_path, TABLE.replace("const\tvar:57\t46\t0", "const\tvar:57\t0\t0"))
    assert not DU.talkstop_on(wt3)


def test_rules(tmp_path):
    wt = table(tmp_path)
    b = lambda name, w, cost, room: DU.event_bound(name, w, cost, wt, 5, True, room)  # noqa: E731
    assert b("local:208", 10, 1, "28") == (0, "recursive unit (D7)")
    assert b("local:208", 10, 13, "41")[0] == 4 * 5 * 13  # delay, one value of var 19
    assert b("local:209", 3, 25, "41")[0] == 20  # talk: 1 frame under the talk-stop key
    assert b("script:2", 736, 1, "-")[0] == 20 and "walk" in b("script:2", 736, 1, "-")[1]
    assert b("script:3", 5, 1, "-")[0] == 20
    assert b("script:3", 6, 1, "-") == (0, "no yield ends at W (a start not yet run): 0")
    assert DU.input_bound(2, 5)[0] == 40
    wild = table(tmp_path, TABLE + "wild\tscript:9\tcite\tvar:5\n")
    assert DU.event_bound("script:3", 5, 1, wild, 5, True, "-")[0] == 0


def test_delay_closed_form():
    # D4 against the countdown, with one value of var 19 and with values that change
    for n in [0, 1, 4, 5, 6, 59, 60, 1800]:
        for v in [1, 3, 5, 6, 15, 20]:
            frames, qf = countdown(n, [v] * 10000)
            dmax = min(15, max(1, v))
            assert frames == n // dmax + 1
            assert qf >= DU.delay_qf(n // dmax + 1, max(1, v), dmax) >= 4 * (n + 1)
        seq = [5, 6, 3, 15, 1, 7] * 2000
        frames, qf = countdown(n, seq)
        assert qf >= 4 * (n + 1) and frames >= n // 15 + 1
        assert qf >= DU.delay_qf(n // 15 + 1, 1, 15)


def test_bounds_file_round_trip(tmp_path):
    b = {"head": {"delta_lo": 5, "dmax": 5, "talkstop": 1, "qf_frame": 20},
         "actions": {("event(local:208@10#ab)", 13): ("event", "local:208", 10, 0, "recursive unit (D7)",
                                                     "28:0;41:260"),
                     ("sentence(2,315)", 2): ("input", "-", -1, 40, "input: 2 key(s) or click(s) (D5)", "-")},
         "points": {("local:208", "41", 10): [260, {"delay (D4)"}]},
         "caps": {("local:211", 33): (7780, "paused")}}
    DU.write(b, tmp_path / "b.tsv")
    r = DU.read(tmp_path / "b.tsv")
    assert r["head"]["qf_frame"] == 20
    assert r["actions"][("event(local:208@10#ab)", 13)][5] == {"28": 0, "41": 260}
    assert r["actions"][("sentence(2,315)", 2)][3] == 40
    assert r["points"][("local:208", "41", 10)] == (260, "delay (D4)")
    assert r["caps"][("local:211", 33)] == (7780, "paused")
