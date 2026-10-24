"""Tests for check.py's parallel path (``--jobs``, ``score_parallel``) and its resume folder (``--resume``), over the
real game and ground truth: a handful of moves per row 0-2, scored in forked workers and one after another in this
process (``main``'s loop), must give equal records.

usage: ../../.venv/bin/python -m pytest test_parallel.py -q
"""

import json
import os
import shutil
import sys
from pathlib import Path

import pytest

import check
import groundtruth
import ir
import model
import parallel
import speedrun.executor

ROWS = {0: "own_entry", 1: "own_entry,dispatch,calls", 2: "own_entry,dispatch,calls,room_change"}
SLOW = {"S1#83", "S2#21", "S2#29"}  # row 2's three slowest moves (10-40 s each); left out to keep the tests short


@pytest.fixture(scope="module")
def data():
    rows = groundtruth.load()
    return rows, check.held_families(rows), check.run_b_inputs(rows)


def fresh_game() -> model.Game:
    """The game loaded again, its caches empty (``Game.load`` keeps one per process), so forked workers start cold."""
    return model.Game.load.__func__.__wrapped__(model.Game)


def pick(rows, game, per_kind=2):
    """A handful of rows: the first ``per_kind`` counted moves of each action kind, and two excluded rows."""
    seen, out = {}, []
    for row in rows:
        if check.label(row) in SLOW:
            continue
        status, action, _ = check.classify(row, game)
        k = check.kind_name(action) if status == "counted" else "excluded"
        if seen.get(k, 0) < per_kind:
            seen[k] = seen.get(k, 0) + 1
            out.append(row)
    return out


def setup(features: str, data, game=None):
    rows, kinds, inputs = data
    machine = model.Machine(frozenset(features.split(",")), game=game or fresh_game())
    return machine, model.non_fact_vars(machine.game), kinds, inputs, pick(rows, model.Game.load())


def serial(rows, machine, nonfacts, kinds, inputs) -> list[dict]:
    """``main``'s loop for ``--jobs 1`` without ``--resume`` (check.py), copied: the records one after another."""
    moves = []
    for row in rows:
        status, action, reason = check.classify(row, machine.game)
        m = {"i": row["i"], "label": check.label(row), "name": row["name"], "status": status, "reason": reason,
             "kind": check.kind_name(action), "action": action.as_dict() if action else None,
             "run_b_inputs": inputs[row["i"]]}
        if status == "counted":
            m |= check.score_move(row, action, machine, nonfacts, kinds)
        moves.append(m)
    return moves


def lines(moves):
    return [json.dumps(m, default=str) for m in moves]


@pytest.mark.parametrize("row", sorted(ROWS))
def test_parallel_records_equal_serial(row, data):
    # forked first, from a game no move has run on; then one after another, on another fresh game
    machine, nonfacts, kinds, inputs, rows = setup(ROWS[row], data)
    assert len({check.kind_name(check.classify(r, model.Game.load())[1]) for r in rows}) >= 4
    tp = {}
    par = check.score_parallel(rows, machine, nonfacts, kinds, inputs, 3, times=tp)
    machine, nonfacts, kinds, inputs, _ = setup(ROWS[row], data)
    ser = serial(rows, machine, nonfacts, kinds, inputs)
    assert [m["label"] for m in par] == [check.label(r) for r in rows]  # row order, whatever order they ran in
    assert par == ser
    assert lines(par) == lines(ser)
    counted = [m["label"] for m in ser if m["status"] == "counted"]
    assert sorted(tp) == sorted(counted)
    by = {m["label"]: m for m in ser}
    assert all(tp[k][0] == by[k]["kind"] and tp[k][3] == by[k]["paths"] for k in counted)  # kind and paths


def test_submit_order_changes_nothing(data):
    machine, nonfacts, kinds, inputs, rows = setup(ROWS[1], data)
    ser = serial(rows, machine, nonfacts, kinds, inputs)
    # the last rows first, as a times file would put them
    prior = {check.label(r): float(i) for i, r in enumerate(rows)}
    par = check.score_parallel(rows, machine, nonfacts, kinds, inputs, 2, prior=prior)
    assert lines(par) == lines(ser)


def test_resume_with_a_partial_folder(data, tmp_path, monkeypatch):
    machine, nonfacts, kinds, inputs, rows = setup(ROWS[2], data)
    fresh = serial(rows, machine, nonfacts, kinds, inputs)
    key = parallel.run_key(machine.features, machine.switches, check.gt_digest())
    half = rows[:len(rows) // 2]
    first = check.score_parallel(half, machine, nonfacts, kinds, inputs, 2, store=parallel.Store(tmp_path, key))
    kept = [m["label"] for m in first if m["status"] == "counted"]
    assert kept and len(list(tmp_path.glob("*.pkl"))) == len(kept)

    # the moves already kept must be loaded, not scored: scoring one of them fails (in the workers too, by fork)
    score = check.score_move

    def guarded(row, *a):
        assert check.label(row) not in kept, f"{check.label(row)} scored again"
        return score(row, *a)

    monkeypatch.setattr(check, "score_move", guarded)
    store = parallel.Store(tmp_path, key)
    times = {}
    par = check.score_parallel(rows, machine, nonfacts, kinds, inputs, 2, store=store, times=times)
    assert store.loaded == len(kept)
    assert lines(par) == lines(fresh)
    assert par == fresh
    counted = [m["label"] for m in fresh if m["status"] == "counted"]
    assert len(list(tmp_path.glob("*.pkl"))) == len(counted) and sorted(times) == sorted(counted)

    # --jobs 1 --resume loads them all
    store = parallel.Store(tmp_path, key)
    again = check.score_parallel(rows, machine, nonfacts, kinds, inputs, 1, store=store)
    assert store.loaded == len(counted) and lines(again) == lines(fresh)


def test_a_stale_record_is_not_reused(data, tmp_path):
    machine, nonfacts, kinds, inputs, rows = setup(ROWS[1], data)
    fresh = serial(rows, machine, nonfacts, kinds, inputs)
    digest = check.gt_digest()
    key = parallel.run_key(machine.features, machine.switches, digest)
    m = next(m for m in fresh if m["status"] == "counted")
    lab, head = m["label"], check.head_line(m)
    bogus = {"accepted": "stale"}
    # a record kept under another key: other features, other switches, another ground truth
    for other in (parallel.run_key(machine.features | {"room_change"}, machine.switches, digest),
                  parallel.run_key(machine.features, {"const_polls"}, digest),
                  parallel.run_key(machine.features, machine.switches, "0" * 12)):
        assert other != key
        parallel.Store(tmp_path, other).put(lab, head, bogus, json.dumps(bogus), 1.0, 1.0)
    s = parallel.Store(tmp_path, key)
    p = s.path(lab)
    # one kept under this key for another action (the executor's walk verb changed, say)
    other_head = check.head_line(m | {"action": {**(m["action"] or {}), "verb": -1}})
    assert other_head != head
    s.put(lab, other_head, bogus, json.dumps(bogus), 1.0, 1.0)
    assert parallel.Store(tmp_path, key).get(lab, head) is None
    # a file under this key's name that holds another key, and one cut short
    forged = parallel.Store(tmp_path, "f" * 64)
    forged.put(lab, head, bogus, json.dumps(bogus), 1.0, 1.0)
    shutil.copy(forged.path(lab), p)
    assert parallel.Store(tmp_path, key).get(lab, head) is None
    s.put(lab, head, bogus, json.dumps(bogus), 1.0, 1.0)
    assert parallel.Store(tmp_path, key).get(lab, head) is not None  # the same record, whole, is used
    p.write_bytes(p.read_bytes()[:40])
    assert parallel.Store(tmp_path, key).get(lab, head) is None
    store = parallel.Store(tmp_path, key)
    assert lines(check.score_parallel(rows, machine, nonfacts, kinds, inputs, 2, store=store)) == lines(fresh)
    assert store.loaded == 0


def test_the_inputs_hashed_are_those_read():
    # the trees are the ones ir.load and Game.load read: ir.SCRIPTS, and every raw block path in its index.json
    assert parallel.TREES[0] == ir.SCRIPTS
    meta = json.loads((ir.SCRIPTS / "index.json").read_text())
    blocks = [g["block"] for g in meta["global_scripts"].values()] + [r["lflf"] for r in meta["rooms"].values()]
    assert blocks and all((ir.ROOT / b).resolve().is_relative_to(parallel.TREES[1]) for b in blocks)
    assert Path(speedrun.executor.__file__).resolve() in parallel.SOURCES
    assert {p.name for p in parallel.SOURCES} >= {"check.py", "model.py", "actions.py", "facts.py", "ir.py",
                                                    "groundtruth.py"}
    assert parallel.AT_IMPORT == parallel.sources_digest()


def test_the_key_follows_every_input(tmp_path, monkeypatch):
    src, tree = tmp_path / "src", tmp_path / "tree"
    src.mkdir()
    (tree / "room-1").mkdir(parents=True)
    sources = []
    for p in parallel.SOURCES:
        shutil.copy(p, src / p.name)
        sources.append(src / p.name)
    (tree / "index.json").write_text("{}")
    (tree / "room-1" / "local-200.txt").write_text("[0000] (00) stopObjectCode()\n")
    monkeypatch.setattr(parallel, "SOURCES", tuple(sources))
    monkeypatch.setattr(parallel, "TREES", (tree,))
    key = parallel.run_key({"own_entry"}, set(), "abc")
    assert parallel.run_key({"own_entry"}, set(), "abc") == key
    assert parallel.run_key({"own_entry"}, set(), "abc", parallel.sources_digest()) == key
    for p in sources + [tree / "index.json", tree / "room-1" / "local-200.txt"]:
        before = p.read_bytes()
        p.write_bytes(before + b"\n")
        assert parallel.run_key({"own_entry"}, set(), "abc") != key, p.name
        p.write_bytes(before)
    assert parallel.run_key({"own_entry"}, set(), "abc") == key
    os.rename(tree / "room-1" / "local-200.txt", tree / "room-1" / "local-201.txt")  # a rename
    assert parallel.run_key({"own_entry"}, set(), "abc") != key
    os.rename(tree / "room-1" / "local-201.txt", tree / "room-1" / "local-200.txt")
    (tree / "room-1" / "extra.txt").write_text("")  # a new file
    assert parallel.run_key({"own_entry"}, set(), "abc") != key
    (tree / "room-1" / "extra.txt").unlink()
    assert parallel.run_key({"own_entry"}, set(), "abc") == key
    assert parallel.run_key({"own_entry"}, set(), "abc", "0" * 64) != key  # the sources as imported
    assert parallel.run_key({"own_entry", "dispatch"}, set(), "abc") != key
    assert parallel.run_key({"own_entry"}, {"const_polls"}, "abc") != key
    assert parallel.run_key({"own_entry"}, set(), "abd") != key
    monkeypatch.setattr(sys, "version", sys.version + " other")
    assert parallel.run_key({"own_entry"}, set(), "abc") != key


def test_a_change_while_loading_stops_the_run(data, tmp_path, monkeypatch):
    # main takes the key before the game and the ground truth are read; score_many takes it again after
    machine, nonfacts, kinds, inputs, rows = setup(ROWS[0], data, model.Game.load())
    monkeypatch.setattr(check, "OUT", tmp_path / "out")
    with pytest.raises(SystemExit, match="changed while they were read"):
        check.score_many(rows, machine, nonfacts, kinds, inputs, machine.features, machine.switches, 2,
                         tmp_path / "store", "0" * 64, 0.0)
    assert not list((tmp_path).rglob("*.pkl")) and not (tmp_path / "out").exists()


def test_times_file(tmp_path):
    rows = [("S1#1", "sentence", 1.23456, 100.04, 3), ("S1#2", "wait", 0.5, 99.0, 1)]
    parallel.write_times(tmp_path / "t.tsv", rows)
    text = (tmp_path / "t.tsv").read_text().splitlines()
    assert text == [parallel.TIMES_HEAD, "S1#1\tsentence\t1.235\t100.0\t3", "S1#2\twait\t0.500\t99.0\t1"]
    assert parallel.read_times([tmp_path / "none.tsv", tmp_path / "t.tsv"]) == {"S1#1": 1.235, "S1#2": 0.5}
