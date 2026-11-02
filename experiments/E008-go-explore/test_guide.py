"""E008: tests of guide.py, from the game files alone (no engine, no reading).

usage: cd experiments/E008-go-explore && ../../.venv/bin/python -m pytest -q test_guide.py
"""

import json
import os
import random
import re
import subprocess
import sys
import time
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import guide as G  # noqa: E402

PY = sys.executable
K2 = [28, 29, 33, 34, 35, 38, 41, 83, 85, 96]
SECONDS = re.compile(r'"seconds":[0-9.]+')


def run_map(tmp: Path, name: str, goal: str, *extra, seed: str = "0") -> tuple:
    out = tmp / f"{name}.json"
    env = dict(os.environ, PYTHONHASHSEED=seed)
    t = time.time()
    subprocess.run([PY, str(HERE / "guide.py"), "map", "--goal", goal, "--out", str(out), *extra], check=True,
                   cwd=HERE, env=env, stdout=subprocess.DEVNULL)
    return out, time.time() - t


@pytest.fixture(scope="module")
def game():
    return G.M.Game.load()


@pytest.fixture(scope="module")
def door(tmp_path_factory):
    """The door's guide from two CLI runs under different hash seeds: (path, path)."""
    tmp = tmp_path_factory.mktemp("door")
    a, _ = run_map(tmp, "a", "state:316=1", seed="1")
    b, _ = run_map(tmp, "b", "state:316=1", seed="2")
    return a, b


@pytest.fixture(scope="module")
def door_map(game):
    return G.Map(game, G.start_row(1), wide=False)


# --- 1. the door ------------------------------------------------------------------------------------------------------

def test_door_writers_and_depths(door):
    g = G.load(door[0])
    assert g["format"] == "e008-guide-1" and g["map"] == "direct"
    assert g["goal"] == {"state:316": [1]}
    w = g["writers"]["state:316=1"]
    assert "local:28:205" in w and "local:28:209" in w
    assert ["state:316", 1, 0] in g["atoms"]
    assert ["room", 28, 1] in g["atoms"]
    assert G.depth(g, "room", 28) == 1 and G.depth(g, "state:316", 1) == 0
    s = g["stats"]
    assert s["atoms"] == len(g["atoms"]) and s["keys"] == len(g["keys"])
    assert sum(s["depth_hist"].values()) == s["atoms"]
    assert s["wide"]["atoms"] >= s["atoms"]
    # atoms sorted by (depth, key, str(value)); keys hold each key's least depth
    order = [(d, k, str(v)) for k, v, d in g["atoms"]]
    assert order == sorted(order)
    least = {}
    for k, v, d in g["atoms"]:
        least.setdefault(k, d)
    assert g["keys"] == least


def test_door_depth_is_one_more_than_its_least_source(door, door_map):
    """Recompute from the map: every atom of depth d > 0 is a need of a writer site of an atom of depth d - 1, and of
    none of a smaller depth; every need of a writer site of an atom in R is in R."""
    g = G.load(door[0])
    depth = {(k, v): d for k, v, d in g["atoms"]}
    best = {}
    for (k, v), d in depth.items():
        for u, sites in door_map.writers(k, v):
            for i in sites:
                for b in door_map.needs(u, i):
                    best[b] = min(best.get(b, d + 1), d + 1)
    assert set(best) <= set(depth)
    for a, d in depth.items():
        if d > 0:
            assert best[a] == d, (a, d, best.get(a))


def test_door_same_bytes(door):
    a, b = (SECONDS.sub('"seconds":0', p.read_text()) for p in door)
    assert a == b
    assert json.loads(a)["stats"]["seconds"] == 0


def test_door_writers_name_units(door, game):
    g = G.load(door[0])
    names = {G.unit_name(k) for k in game.units}
    for atom, units in g["writers"].items():
        assert units == sorted(units) and set(units) <= names, atom
    no = {G.atom_str(k, v) for k, v in g["stats"]["no_writer"]}
    assert not no & set(g["writers"])
    assert no | set(g["writers"]) == {G.atom_str(k, v) for k, v, _ in g["atoms"]}


# --- 2. the kitchen ---------------------------------------------------------------------------------------------------

def test_kitchen_room_41(tmp_path, capsys):
    out, _ = run_map(tmp_path, "kitchen", "owner:566=1")
    g = G.load(out)
    d = G.depth(g, "room", 41)
    assert d is not None and ["room", 41, d] in g["atoms"]
    assert ["owner:566", 1, 0] in g["atoms"]
    with capsys.disabled():
        print(f"\n  kitchen: room = 41 at depth {d} (plan.md predicts 1); |R| {g['stats']['atoms']}")


# --- 3. rings and rooms -----------------------------------------------------------------------------------------------

def test_rings_and_rooms_from_files(door, tmp_path):
    g = G.load(door[0])
    assert G.ring(g, 33, K2) == [28, 33, 35, 38]
    assert G.ring(g, 28, K2) == [28, 33, 41]
    ref = tmp_path / "rooms.json"
    subprocess.run([PY, str(HERE.parent / "E004-fast-exact" / "rooms_from_files.py"), "--out", str(ref)], check=True,
                   cwd=HERE, stdout=subprocess.DEVNULL)
    r = json.loads(ref.read_text())
    assert g["rooms"]["edges"] == r["edges"]
    assert g["rooms"]["reached"] == r["reached"]
    assert g["rooms"]["start"] == r["start_room"] == 33
    assert len(g["rooms"]["reached"]) == 80


# --- 4. Part I --------------------------------------------------------------------------------------------------------

def test_part1(tmp_path, capsys):
    out, secs = run_map(tmp_path, "part1", "part1")
    assert secs < 300
    g = G.load(out)
    assert g["stats"]["atoms"] > 0
    for o in (901, 902, 903):
        assert [f"owner:{o}", 1, 0] in g["atoms"]
    with capsys.disabled():
        print(f"\n  part1: |R| {g['stats']['atoms']}, {secs:.1f} s")


# --- 5. the reader's rules on a hand-made guide -----------------------------------------------------------------------

HAND = {"format": "e008-guide-1", "goal": {"state:1": [1]}, "map": "direct",
        "rooms": {"start": 5, "edges": {"5": [6, 7], "6": [5], "7": []}, "reached": [5, 6, 7]},
        "atoms": [["state:1", 1, 0], ["room", 5, 1], ["var:2", None, 2], ["var:2", 3, 1], ["bit:4", 0, 3]],
        "keys": {"state:1": 0, "room": 1, "var:2": 1, "bit:4": 3},
        "writers": {"state:1=1": ["local:5:200"]},
        "stats": {"atoms": 5, "keys": 4, "depth_hist": {"0": 1, "1": 2, "2": 1, "3": 1}, "units": 1, "seconds": 0.0,
                  "no_writer": [["room", 5]], "wide": {"atoms": 5, "keys": 4}}}


def test_load_depth_ring_rules(tmp_path):
    p = tmp_path / "g.json"
    p.write_text(json.dumps(HAND))
    g = G.load(p)
    assert G.depth(g, "state:1", 1) == 0
    assert G.depth(g, "state:1", 0) is None
    assert G.depth(g, "var:2", 3) == 1  # [k, v] at 1 beats [k, null] at 2
    assert G.depth(g, "var:2", 9) == 2  # only [k, null]
    assert G.depth(g, "bit:4", 0) == 3
    assert G.depth(g, "bit:4", 1) is None
    assert G.depth(g, "var:9", 0) is None
    assert G.ring(g, 5) == [5, 6, 7]
    assert G.ring(g, 5, [5, 7, 9]) == [5, 7]
    assert G.ring(g, 7, [5, 6]) == []
    assert G.ring(g, 6, [5, 6]) == [5, 6]
    assert G.ring(g, 8) == [8]  # a room with no edges is its own ring
    bad = tmp_path / "bad.json"
    bad.write_text(json.dumps(dict(HAND, format="other")))
    with pytest.raises(ValueError):
        G.load(bad)


def test_parse_goal():
    assert G.parse_goal("state:316=1") == {"state:316": [1]}
    assert G.parse_goal("owner:566=1,state:316=1,state:316=0") == {"owner:566": [1], "state:316": [0, 1]}
    assert G.parse_goal("part1") == {"owner:901": [1], "owner:902": [1], "owner:903": [1]}


# --- the map's parts against trace_explain.Code, which they index -----------------------------------------------------

def test_direct_sites_are_write_sites(door, door_map, game):
    """The indexed direct sites equal Code.write_sites(direct=True) on every candidate unit, and a sample of other
    units has no write site, for the door's atoms of depth at most 2 and a sample of the rest."""
    g = G.load(door[0])
    rng = random.Random(1)
    atoms = [(k, v) for k, v, d in g["atoms"] if d <= 2 and k != "room"]
    rest = [(k, v) for k, v, d in g["atoms"] if d > 2 and k != "room"]
    atoms += rng.sample(rest, 40)
    units = sorted(game.units)
    for k, v in atoms:
        cand = door_map.candidates(k)
        for u in cand:
            assert door_map.direct_sites(u, k, v) == door_map.code.write_sites(u, k, v, direct=True), (u, k, v)
        for u in rng.sample([u for u in units if u not in cand], 15):
            assert not door_map.code.write_sites(u, k, v, direct=True), (u, k, v)


def test_wide_sites_are_write_sites(game):
    """The wide map's sites equal Code.write_sites(direct=False) on the candidates and on the units it adds."""
    m = G.Map(game, G.start_row(1), wide=True)
    for k, v in [("var:196", 3), ("var:101", None), ("bit:20", None), ("bit:20", 1), ("state:316", 1),
                 ("owner:566", 1), ("class:420:6", 0)]:
        got = {u: s for u, s in m.writers(k, v)}
        units = set(m.candidates(k)) | {u for u, _ in m.wide_sites(k, v)}
        for u in sorted(units):
            ref = m.code.write_sites(u, k, v, direct=False)
            extra = m.draws.get(u, frozenset()) if k.startswith("state:") else frozenset()
            assert got.get(u, frozenset()) - extra == ref - extra, (u, k, v)


def test_reaching_is_code_reachable(door_map, game):
    for key, samples in [(("local", 28, 205), 16), (("local", 28, 209), 15), (("entry", 28), 10), (("global", 56), 2)]:
        n = len(game.units[key].ins)
        for i in range(0, n, max(1, n // samples)):
            got = door_map.reaching(key, i)
            ref = {j for j in range(n) if door_map.code.reachable(key, j, frozenset({i}), cross_yields=True)}
            assert got - {n} == ref, (key, i)


def test_registers_followed_to_their_writes(door_map, game):
    """local 209's test of VAR_RESULT reads a state through a getter whose object is a local argument: no atom; the
    room's own start need remains."""
    u = game.units[("local", 28, 209)]
    js = [j for j, x in enumerate(u.ins) if isinstance(x.st, G.ir.Jump) and x.st.cond is not None]
    assert js
    assert all(("var:0", 1) not in door_map.cond_atoms(("local", 28, 209), j) for j in js)
    assert door_map.start_needs[("local", 28, 209)] == frozenset({("room", 28)})


def test_stats_cli(door):
    out = subprocess.run([PY, str(HERE / "guide.py"), "stats", str(door[0]), "--key",
                          str(HERE.parent / "E001-goals-from-scripts" / "key.json")],
                         check=True, cwd=HERE, capture_output=True, text=True).stdout
    assert "|R|" in out and "depth histogram" in out and "wide map" in out
    m = re.search(r"key \(evaluation only\): (\d+) Part I facts, key in R (\d+), .* in R (\d+)", out)
    assert m and int(m.group(1)) == 99 and int(m.group(3)) <= int(m.group(2))
