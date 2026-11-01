"""experiments/E004-fast-exact/pairs_check.py on two small dumps (reach-pairs.md): a room change whose write of the verb
script may not fire leaves the old value in the new room; one whose write must fire replaces it."""
import json
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "experiments/E004-fast-exact/pairs_check.py"


def dump(tmp_path, when):
    lines = [
        {"fact": "var:32", "values": ["201", "202"], "start": "202"},
        {"fact": "room", "values": ["28", "85"], "start": "28"},
        {"fact": "bit:5", "values": ["0", "1"], "start": "0"},
        {"label": "flip", "pre": [], "eff": [["bit:5", 0, ["1"], []]]},
        # the room change: room 85, and the verb script 201 under `when`
        {"label": "go", "pre": [["room", ["28"]]], "eff": [["room", 0, ["85"], []], ["var:32", 0, ["201"], when]]},
        {"label": "dialog_line(120)[202@85]", "pre": [["var:32", ["202"]], ["room", ["85"]]], "eff": [["bit:5", 0, ["0"], []]]},
    ]
    p = tmp_path / "t.jsonl"
    p.write_text("".join(json.dumps(x) + "\n" for x in lines))
    out = subprocess.run([sys.executable, str(SCRIPT), str(p), "--show", "202@85"], capture_output=True, text=True, check=True)
    return out.stdout


def test_an_effect_that_may_not_fire_leaves_the_old_value(tmp_path):
    out = dump(tmp_path, [["bit:5", ["1"]]])
    assert "(var:32 = 202, room = 85): reached" in out
    assert "conditional effects on var:32 (kept actions), conditions on another fact too: 1" in out


def test_an_effect_that_must_fire_replaces_it(tmp_path):
    out = dump(tmp_path, [])
    assert "(var:32 = 202, room = 85): not reached" in out
    assert "1 dialog_line(120)[202@85]" in out
