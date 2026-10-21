import json
import textwrap

import pytest

from speedrun import paths
from speedrun.scriptindex import EGO, ScriptIndex
from speedrun.search import persistent

DOS_CD = paths.DATA_DIR / "scripts-dos-cd"


def _write(tmp_path, files: dict[str, str], meta: dict) -> ScriptIndex:
    for name, text in files.items():
        p = tmp_path / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(textwrap.dedent(text))
    (tmp_path / "index.json").write_text(json.dumps(meta))
    return ScriptIndex(tmp_path)


TOY_META = {
    "objects": {"10": {"name": "door", "room": 1}, "11": {"name": "key", "room": 1}},
    "global_scripts": {"25": {"file": "global/script-025.txt"}},
    "rooms": {"1": {"objects": {"10": {"name": "door", "file": "room-001/obj-0010-door.txt"},
                                "11": {"name": "key", "file": "room-001/obj-0011-key.txt"}},
                    "local_scripts": {}},
              "2": {"entry": {"file": "room-002/entry.txt"}, "objects": {}, "local_scripts": {}}},
}

TOY = {
    "room-001/obj-0010-door.txt": """\
        # ---- verb 2 (Open) entry @ 0010 ----
        [0010] (0A) startScript(25,[]);
        [0013] (00) stopObjectCode();
        # ---- verb 11 (Walk to) entry @ 0014 ----
        [0014] (0F) VAR_RESULT = getObjectState(10);
        [0019] (48) if (VAR_RESULT == 1) /*0025*/ {
        [0020] (24)   loadRoomWithEgo(10,2,-1,-1);
        [0025] (**) }
        [0025] (00) stopObjectCode();
        """,
    "room-001/obj-0011-key.txt": """\
        # ---- verb 9 (Pick up) entry @ 0010 ----
        [0010] (25) pickupObject(11,1);
        [0013] (1A) Bit[83 + 2] = 1;
        [0018] (00) stopObjectCode();
        """,
    "global/script-025.txt": """\
        [0000] (28) if (!Local[0]) /*000A*/ {
        [0005] (9A)   Local[0] = VAR_ME;
        [000A] (**) }
        [000A] (87) setState(Local[0],1);
        [000E] (A0) stopObjectCode();
        """,
    "room-002/entry.txt": """\
        [0000] (1A) VAR_VERB_SCRIPT = 14;
        [0005] (A0) stopObjectCode();
        """,
}


def test_global_called_without_arguments_sees_var_me(tmp_path):
    ix = _write(tmp_path, TOY, TOY_META)
    assert [k for k, _ in ix.writers("state", 10, 1)] == [("obj", 10, 2)]


def test_pickup_and_summed_bit_index(tmp_path):
    ix = _write(tmp_path, TOY, TOY_META)
    assert [k for k, _ in ix.writers("owner", 11, EGO)] == [("obj", 11, 9)]
    assert [k for k, _ in ix.writers("bit", 85, 1)] == [("obj", 11, 9)]


def test_room_load_runs_the_entry_script_and_names_engine_vars(tmp_path):
    ix = _write(tmp_path, TOY, TOY_META)
    (key, eff), = ix.writers("var", 32, 14)  # VAR_VERB_SCRIPT, written by room 2's entry
    assert key == ("obj", 10, 11)
    assert "getObjectState(10) == 1" in eff.guards  # VAR_RESULT resolved to what it read


def test_exit_graph(tmp_path):
    ix = _write(tmp_path, TOY, TOY_META)
    assert ix.exits() == {1: [(10, 2, 1)]}


def test_persistent_drops_transient_vars_and_positions():
    conds = [{"room": 30}, {"var": 194, "eq": 0}, {"actor_x": 1, "ge": 3}, {"var": 221, "eq": 1},
             {"not": {"class": 420, "set": 6}}, {"actor_room": 11, "eq": 0}]
    assert persistent(conds) == [{"room": 30}, {"var": 221, "eq": 1}, {"not": {"class": 420, "set": 6}},
                                 {"actor_room": 11, "eq": 0}]


@pytest.mark.skipif(not (DOS_CD / "index.json").exists(), reason="DOS CD scripts not dumped")
def test_dos_cd_writers_found_in_the_engine_runs():
    ix = ScriptIndex(DOS_CD)
    # Otis gives the cake through the jail's local 203 (verb 80 = given an object).
    assert ("obj", 405, 80) in [k for k, _ in ix.writers("owner", 420, EGO)]
    # The idol trial bit is written by global 71 as Bit[83 + Local[0]].
    assert ("obj", 578, 9) in [k for k, _ in ix.writers("bit", 85, 1)]
    # Low Street: the clock archway is the plain way to High Street.
    assert (451, 34, 0) in ix.exits()[35]
    # The forest's screens are pseudo-rooms named by VAR_ROOM guards.
    assert any(nxt == 215 for _, nxt, _ in ix.exits()[218])
