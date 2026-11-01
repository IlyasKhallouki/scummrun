"""Tests of E008's explorer (explore.py; plan.md "Entries", "Cells", "Selection", "Verify", "Parallelism and the
machine", "Interfaces", "Stop rule").

usage: uv run pytest experiments/E008-go-explore/test_explore.py              (unit tests, no engine)
       E008_ENGINE=1 uv run pytest experiments/E008-go-explore/test_explore.py  (also the engine test)

* unit: a synthetic archive (hand-made facts and a GUIDE fixture): cell keys, held atoms, representatives,
  favoured entries, each criterion of the order breaking a tie, n, exhaustion, busy cells, allowed rooms, movies and
  chain checkpoints, job.json, the readings index and its eviction, stagnation, the rollback of a cut ingest.
* engine (E008_ENGINE=1): init at the boot (frame 2728), two entries from E007's kitchen movie as a chain, verify
  (0 differences, owner:566 = 1) and report (488 frames, 11,712 qf from the root).

The fixtures name MI1's numbers (tests may, C2); explore.py does not.
"""

import gzip
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import explore as X  # noqa: E402

PY = sys.executable
ALLOWED = [28, 33, 35, 38, 41]
GUIDE = {
    "format": "e008-guide-1", "goal": {"owner:566": [1]}, "map": "direct",
    "rooms": {"start": 33, "edges": {"33": [28, 35, 38], "28": [33, 41], "41": [28], "35": [33], "38": [33]},
              "reached": [28, 33, 35, 38, 41]},
    "atoms": [["owner:566", 1, 0], ["room", 41, 1], ["state:316", 1, 2], ["var:103", None, 2], ["bit:7", 1, 3],
              ["room", 28, 3]],
    "keys": {"owner:566": 0, "room": 1, "state:316": 2, "var:103": 2, "bit:7": 3},
    "writers": {"owner:566=1": ["obj:566"]}, "stats": {}}
ROOT_FACTS = {"room": 33, "ego": 1, "owner:500": 1, "var:5": 7, "at:script:1:10": 1}


def cp(frame, facts, tick):
    return [frame, facts, {"tick": tick, "rng": 1, "actors": []}]


def make_archive(tmp: Path, guide: dict = GUIDE, allowed=ALLOWED, goal=None) -> X.Archive:
    tmp.mkdir(parents=True, exist_ok=True)
    gp = tmp / "GUIDE.json"
    gp.write_text(json.dumps(guide))
    a = X.Archive.create(tmp / "arch", "test", goal or {"owner:566": [1]}, gp, allowed, 2,
                         {"frame": 100, "tick": 0, "qf": 1000, "room": 33})
    a.add(None, 100, 0, [], [cp(100, dict(ROOT_FACTS), 0)], None, "boot")
    a.write_cells()
    return a


def child(a: X.Archive, parent: int, df: int, tick: int, job=None, **facts) -> int:
    """An entry ``df`` frames after ``parent`` whose facts are the parent's with ``facts`` over them."""
    f0 = a.rows[parent]["frame"]
    base = dict(a.facts(parent))
    base.update({k.replace("__", ":"): v for k, v in facts.items()})
    return a.add(parent, f0 + df, tick, [{"frame": f0 + df, "click": [1, 1]}], [cp(f0 + df, base, tick)], job)


def job_with(a: X.Archive, entry: int, news: list, tried=(), exhausted=False, ok=True, error=None,
             same=True, reading=None) -> Path:
    """A job dir for ``entry`` (prepare_job) and its result: ``news`` [(df, tick, facts over the entry's)]."""
    d = a.prepare_job(entry)
    job = json.loads((d / "job.json").read_text())
    f0, base = a.rows[entry]["frame"], dict(a.facts(entry))
    new = []
    for k, (df, tick, over) in enumerate(news):
        f = dict(base, **over)
        doc = {"id": None, "parent": entry, "frame": f0 + df, "tick": tick,
               "events": [{"frame": f0 + df, "click": [2, 2]}], "checkpoints": [cp(f0 + df, f, tick)]}
        (d / "new").mkdir(exist_ok=True)
        with gzip.open(d / "new" / f"{k}.json.gz", "wt") as fh:
            json.dump(doc, fh)
        new.append({"file": f"new/{k}.json.gz", "frame": f0 + df, "tick": tick, "how": "checkpoint"})
    res = {"job": job["job"], "entry": entry, "ok": ok, "error": error,
           "return": {"same": same, "differ": {}, "boot_s": 0.0}, "reading": reading,
           "targets": {"by_tier": {"0": 0, "1": 1, "2": 0}, "first": []}, "probes": [], "attempt": None,
           "readings_made": [], "new": new, "tried": [list(t) for t in tried], "exhausted": exhausted,
           "cpu_s": {"total": 1.0}, "peak_rss_mb": {"python": 1.0, "children": 0.0}, "wall_s": 0.1}
    (d / "result.json").write_text(json.dumps(res))
    return d


# --- unit ---------------------------------------------------------------------------------------------------------

def test_cell_key_is_room_owned_objects_and_the_guides_keys(tmp_path):
    a = make_archive(tmp_path)
    k0 = a.cell_key(ROOT_FACTS)
    assert k0 == [33, [500], [["bit:7", 0], ["owner:566", 0], ["state:316", 0], ["var:103", 0]]]
    # a fact that is no guide key, a slot and a position leave the cell as it is
    assert a.cell_key(dict(ROOT_FACTS, **{"var:5": 9, "at:script:1:12": 1, "local:200": 1})) == k0
    assert a.cell_key(dict(ROOT_FACTS, **{"owner:566": 1}))[1] == [500, 566]
    assert a.cell_key(dict(ROOT_FACTS, **{"owner:566": 3}))[1] == [500]  # owned by another actor than the ego
    assert a.cell_key(dict(ROOT_FACTS, room=41))[0] == 41
    assert a.cell_id(k0) == a.rows[0]["cell"] and len(a.rows[0]["cell"]) == 40


def test_held_atoms_values_and_any(tmp_path):
    a = make_archive(tmp_path)
    assert a.rows[0]["held"] == []
    assert a.held(dict(ROOT_FACTS, **{"var:103": 5, "state:316": 1, "room": 41})) == \
        [["room", 41], ["state:316", 1], ["var:103", 5]]
    assert a.held(dict(ROOT_FACTS, **{"var:103": 0, "state:316": 2})) == []  # the root's value; a value not in R
    assert a.g(ROOT_FACTS) == 1 and a.g(dict(ROOT_FACTS, **{"owner:566": 1})) == 0


def test_representative_is_the_least_tick_then_id(tmp_path):
    a = make_archive(tmp_path)
    x = child(a, 0, 5, 50, var__5=1)  # var:5 is no guide key: the root's cell
    assert a.rows[x]["cell"] == a.rows[0]["cell"] and a.representative(a.rows[0]["cell"]) == 0
    b1 = child(a, 0, 5, 50, state__316=2)
    b2 = child(a, 0, 6, 30, state__316=2)
    b3 = child(a, 0, 7, 30, state__316=2)
    c = a.rows[b1]["cell"]
    assert a.rows[b2]["cell"] == c == a.rows[b3]["cell"]
    assert a.representative(c) == b2 and a.cells()[c]["entries"] == [b1, b2, b3]


def test_favoured_is_the_least_holder_of_some_atom(tmp_path):
    a = make_archive(tmp_path)
    late = child(a, 0, 5, 40, state__316=1)
    early = child(a, 0, 6, 20, state__316=1, bit__7=1)
    other = child(a, 0, 7, 10, state__316=2)
    fav = a.favoured()
    assert early in fav and late not in fav and other not in fav and 0 not in fav


def _sel(a, b=2):
    return a.select(b)


def test_order_g_first(tmp_path):
    a = make_archive(tmp_path)
    a.busy["j"] = (0, a.rows[0]["cell"])
    gx = child(a, 0, 5, 100, owner__566=1)  # #g 0, favoured, late
    y = child(a, 0, 6, 1, room=41)  # #g 1, favoured, early
    assert _sel(a) == [gx, y]


def test_order_favoured_second(tmp_path):
    a = make_archive(tmp_path)
    a.busy["j"] = (0, a.rows[0]["cell"])
    f = child(a, 0, 5, 50, bit__7=1)
    u = child(a, 0, 6, 10, room=35)
    assert a.rows[f]["g"] == a.rows[u]["g"] and _sel(a) == [f, u]


def test_order_n_third(tmp_path):
    a = make_archive(tmp_path)
    a.busy["j"] = (0, a.rows[0]["cell"])
    h = child(a, 0, 5, 5, bit__7=1)
    a.busy["k"] = (h, a.rows[h]["cell"])
    p = child(a, h, 5, 10, room=35)  # #r 1, early: better on the later criteria
    q = child(a, 0, 6, 20, room=38)
    a.cells()[a.rows[p]["cell"]]["n"] = 1
    assert _sel(a) == [q, p]


def test_order_r_fourth(tmp_path):
    a = make_archive(tmp_path)
    a.busy["j"] = (0, a.rows[0]["cell"])
    h = child(a, 0, 5, 5, bit__7=1)
    a.busy["k"] = (h, a.rows[h]["cell"])
    p = child(a, h, 5, 30, room=35)  # #r 1, late
    q = child(a, 0, 6, 20, room=38)  # #r 0
    assert a.rows[p]["r_path"] == 1 and a.rows[q]["r_path"] == 0
    assert _sel(a) == [p, q]


def test_order_tick_then_id(tmp_path):
    a = make_archive(tmp_path)
    a.busy["j"] = (0, a.rows[0]["cell"])
    late = child(a, 0, 5, 30, room=35)
    early = child(a, 0, 6, 20, room=38)
    assert _sel(a) == [early, late]
    b = make_archive(tmp_path / "b")
    b.busy["j"] = (0, b.rows[0]["cell"])
    first = child(b, 0, 5, 20, room=35)
    second = child(b, 0, 6, 20, room=38)
    assert _sel(b) == [first, second]


def test_busy_cells_and_allowed_rooms(tmp_path):
    a = make_archive(tmp_path)
    out = child(a, 0, 5, 1, room=96)  # not an allowed room
    s1 = child(a, 0, 6, 10, room=35)
    s2 = child(a, 0, 7, 20, room=35, var__5=3)  # s1's cell, not its representative
    assert out not in a.select(10)
    a.busy["j"] = (s2, a.rows[s2]["cell"])  # a job on any entry of a cell makes the cell busy
    assert a.select(10) == [0]
    a.busy.clear()
    assert a.select(10) == [0, s1]


def test_n_goes_to_zero_on_a_new_cell_or_representative_else_up(tmp_path):
    a = make_archive(tmp_path)
    c0 = a.rows[0]["cell"]
    line = a.ingest(job_with(a, 0, [(5, 10, {"var:5": 1})]))  # the root's cell, not its representative
    assert line["new_cells"] == 0 and line["new_reps"] == 0 and a.cells()[c0]["n"] == 1
    a.ingest(job_with(a, 0, [(5, 11, {"var:5": 2})]))
    assert a.cells()[c0]["n"] == 2
    line = a.ingest(job_with(a, 0, [(5, 12, {"state:316": 2})]))  # a new cell
    assert line["new_cells"] == 1 and a.cells()[c0]["n"] == 0
    c1 = a.rows[line["new_entries"][0]]["cell"]
    a.ingest(job_with(a, 0, [(5, 13, {"var:5": 3})]))
    assert a.cells()[c0]["n"] == 1
    line = a.ingest(job_with(a, 0, [(5, 2, {"state:316": 2, "var:5": 9})]))  # a new representative of c1
    assert line["new_reps"] == 1 and a.cells()[c0]["n"] == 0 and a.representative(c1) == line["new_entries"][0]
    # the files give the same state
    b = X.Archive.open(a.root)
    assert b.cells() == a.cells() and b.rows == a.rows


def test_tried_and_exhausted_per_representative(tmp_path):
    a = make_archive(tmp_path)
    a.busy["j"] = (0, a.rows[0]["cell"])
    x = child(a, 0, 5, 50, room=35)
    c = a.rows[x]["cell"]
    a.busy.clear()
    a.ingest(job_with(a, x, [], tried=[["room", 41]]))
    assert a.cells()[c]["tried"] == {str(x): [["room", 41]]}
    d = a.prepare_job(x)
    assert json.loads((d / "job.json").read_text())["tried"] == [["room", 41]]
    shutil.rmtree(d)
    a.ingest(job_with(a, x, [], tried=[["bit:7", 1]], exhausted=True))
    assert a.cells()[c]["tried"][str(x)] == [["bit:7", 1], ["room", 41]] and a.cells()[c]["exhausted"] == [x]
    assert x not in a.select(10)
    y = child(a, 0, 6, 10, room=35, var__5=1)  # an earlier state of the same cell: its new representative
    assert a.representative(c) == y and y in a.select(10)
    b = X.Archive.open(a.root)
    assert b.cells()[c]["exhausted"] == [x] and b.cells()[c]["tried"][str(x)] == [["bit:7", 1], ["room", 41]]


def test_movie_and_chain_checkpoints(tmp_path):
    a = make_archive(tmp_path)
    f1 = dict(ROOT_FACTS, room=28)
    e1 = a.add(0, 120, 5, [{"frame": 105, "click": [1, 2]}, {"frame": 118, "key": "p"}],
               [cp(110, ROOT_FACTS, 3), cp(120, f1, 5)], "j", "end")
    e2 = a.add(e1, 140, 9, [{"frame": 130, "click": [3, 4]}], [cp(140, dict(f1, room=41), 9)], "j", "end")
    assert a.movie(e2) == [{"frame": 105, "click": [1, 2]}, {"frame": 118, "key": "p"}, {"frame": 130, "click": [3, 4]}]
    assert [c[0] for c in a.checkpoints(e2)] == [100, 110, 120, 140]
    assert a.chain(e2) == [0, e1, e2] and a.rows[e2]["r_path"] == 2  # room 28 at e1, room 41 at e2
    with pytest.raises(ValueError):  # an input at the parent's frame (the bridge refuses it: premise S)
        a.add(e2, 150, 10, [{"frame": 140, "click": [0, 0]}], [cp(150, f1, 10)])
    with pytest.raises(ValueError):
        a.add(e2, 150, 10, [], [cp(149, f1, 10)])


def test_job_json(tmp_path):
    a = make_archive(tmp_path)
    e1 = child(a, 0, 20, 5, bit__7=1)
    rd = tmp_path / "arch" / "readings" / "r1"
    rd.mkdir(parents=True)
    (rd / "reading.json").write_text("{}")
    rd2 = tmp_path / "arch" / "readings" / "r2"
    rd2.mkdir()
    (rd2 / "reading.json").write_text("{}")
    a.register_reading([33, 28, 35, 38], rd / "reading.json", 0, None, used=0)
    a.register_reading([28, 33, 35, 38], rd2 / "reading.json", e1, None, used=1)
    a.register_reading([28, 33], rd2 / "reading.json", e1, None, used=1)
    d = a.prepare_job(e1)
    job = json.loads((d / "job.json").read_text())
    assert d.name == "j00000" and sorted(job) == sorted(["job", "seed", "stage", "goal", "guide", "entry", "scope",
                                                         "tried", "held", "readings", "reading_dir", "threads",
                                                         "workers"])
    assert job["job"] == "j00000" and job["seed"] == 1 and job["stage"] == "test" and job["goal"] == {"owner:566": [1]}
    assert job["guide"] == str((tmp_path / "GUIDE.json").resolve())
    assert job["entry"] == {"id": e1, "root": False, "frame": 120, "events": a.movie(e1),
                            "checkpoint": a.checkpoints(e1)[-1]}
    assert job["scope"] == [28, 33, 35, 38] and job["tried"] == [] and job["held"] == [["bit:7", 1]]
    assert job["readings"] == [{"path": str(rd2 / "reading.json"), "start_entry": e1},
                               {"path": str(rd / "reading.json"), "start_entry": 0}]
    assert job["reading_dir"] == str(a.root / "readings" / "28-33-35-38-j00000")
    assert job["threads"] == 3 and job["workers"] == 3
    assert a.next_job() == "j00001" and a.unfinished_jobs() == ["j00000"]
    root_job = json.loads((a.prepare_job(0) / "job.json").read_text())
    assert root_job["entry"]["root"] is True and root_job["entry"]["events"] == []


def test_readings_evict_least_recently_used_and_the_floor(tmp_path):
    a = make_archive(tmp_path)
    paths = []
    for k, used in enumerate((3, 1, 2, 4)):
        d = a.root / "readings" / f"r{k}"
        d.mkdir(parents=True)
        (d / "reading.json").write_text("{}")
        (d / "big.bin").write_bytes(b"x" * 1000)
        (d / "start.txt").write_text("room\t33\n")
        paths.append(d / "reading.json")
        a.register_reading([28, 33], d / "reading.json", 0, None, used=used)
    assert [x["path"] for x in a.readings_for([33, 28])] == [str(p) for p in reversed(paths)]
    gone_bytes = []

    def free():  # a fake disk: 10 GB + what the evictions freed; the level is 10 GB + 1,500 bytes
        return 10 * X.GIB + sum(gone_bytes)

    def free_now():
        gone_bytes[:] = [1000 * sum(1 for p in paths if not p.exists())]
        return free()
    out = a.evict(free_now, in_use={str(paths[1])}, level_gb=10 + 1500 / X.GIB)
    # r1 (used 1) is in use, so r2 (used 2) then r0 (used 3) go; then the space is enough
    assert out == [str(paths[2]), str(paths[0])]
    assert not paths[2].exists() and (paths[2].parent / "start.txt").exists() and paths[1].exists()
    assert [x["path"] for x in a.readings_for([28, 33])] == [str(paths[3]), str(paths[1])]
    b = X.Archive.open(a.root)
    assert [x["path"] for x in b.readings_for([28, 33])] == [str(paths[3]), str(paths[1])]


def test_stagnation_after_one_sweep_of_the_frontier(tmp_path):
    a = make_archive(tmp_path)
    assert not a.stagnant()
    a.logs = [{"job": "j0", "outcome": "ok", "new_r": [], "selectable": 4}]
    assert a.stagnant()  # nothing new since the start, whose frontier is the root's cell
    a.logs = [{"job": "j0", "outcome": "ok", "new_r": [["room", 41]], "selectable": 3},
              {"job": "j1", "outcome": "ok", "new_r": [], "selectable": 5},
              {"job": "j2", "outcome": "guard-killed"},
              {"job": "j2", "outcome": "error", "new_r": [], "selectable": 5}]
    assert not a.stagnant()
    a.logs.append({"job": "j3", "outcome": "ok", "new_r": [], "selectable": 5})
    assert a.stagnant()


def test_a_cut_ingest_is_rolled_back_and_the_job_is_unfinished(tmp_path):
    a = make_archive(tmp_path)
    d = job_with(a, 0, [(5, 10, {"state:316": 1})])
    child(a, 0, 5, 10, job=d.name, state__316=1)  # the ingest's first entry, then the driver died
    assert len(X.Archive.open(a.root).rows) == 1
    b = X.Archive.open(a.root)
    assert not (a.root / "entries" / "1.json.gz").exists() and b.unfinished_jobs() == [d.name]
    line = b.ingest(d)
    assert line["new_entries"] == [1] and line["new_r"] == [["state:316", 1]] and b.unfinished_jobs() == []


def test_ingest_refuses_a_segment_that_is_not_after_the_entry(tmp_path):
    a = make_archive(tmp_path)
    d = job_with(a, 0, [(0, 10, {"state:316": 1})])  # at the entry's own frame
    line = a.ingest(d)
    assert line["outcome"] == "error" and line["error"].startswith("ingest:") and line["new_entries"] == []


def test_readings_registered_from_results(tmp_path):
    a = make_archive(tmp_path)
    rd = a.root / "readings" / "28-33-35-38-j00000"
    rd.mkdir(parents=True)
    (rd / "reading.json").write_text("{}")
    a.ingest(job_with(a, 0, [], reading={"path": str(rd / "reading.json"), "fresh": True,
                                         "scope": [28, 33, 35, 38]}))
    assert [x["start_entry"] for x in a.readings_for([28, 33, 35, 38])] == [0]
    a.ingest(job_with(a, 0, [], reading={"path": str(rd / "reading.json"), "fresh": False,
                                         "scope": [28, 33, 35, 38]}))
    assert a.readings[0]["last_used"] == 1


# --- engine -------------------------------------------------------------------------------------------------------

KITCHEN = Path.home() / ".cache/fast-mi-e007/results" / \
    "replay-part1-kitchen-off-seed1-serve-lama-first-costs-caps-reachx-uar-stones-window-strict-cache-ahead7-lite-" \
    "split-refine.json"


@pytest.mark.skipif(os.environ.get("E008_ENGINE") != "1", reason="the engine test: E008_ENGINE=1")
def test_engine_kitchen_chain_verifies_from_the_root():
    svm = Path.home() / ".cache/fast-mi/svm-w3b/build/scummvm/scummvm"
    if svm.exists():
        os.environ.setdefault("FAST_MI_SCUMMVM_BIN", str(svm))
    out = Path.home() / ".cache/fast-mi-e008-explore/test-engine"
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    gp = out / "GUIDE.json"
    gp.write_text(json.dumps(GUIDE))
    arch = out / "arch"
    p = subprocess.run([PY, "-u", str(HERE / "explore.py"), "init", str(arch), "--stage", "kitchen", "--goal",
                        "owner:566=1", "--guide", str(gp), "--allowed", "k2"], capture_output=True, text=True,
                       timeout=600, env=dict(os.environ))
    assert p.returncode == 0, p.stderr[-3000:]
    a = X.Archive.open(arch)
    assert a.rows[0]["frame"] == 2728 and a.stage["root"]["qf"] == 65284 and a.rows[0]["room"] == 33
    assert a.stage["allowed_rooms"] == [28, 29, 33, 34, 35, 38, 41, 83, 85, 96]
    X._engine_path()
    import replay
    import replay_part1
    from facts import facts_of
    inputs = json.loads(KITCHEN.read_text())["run"]["inputs"]

    def at(frame, events):
        ctl, b = replay.boot(lambda o, i: replay_part1.config(1, o, i), events, frame, out / f"boot-{frame}")
        try:
            s = b.state()
            return [frame, facts_of(s), replay.extra_of(s, b.status)], b.status.get("tick")
        finally:
            ctl.close()
    first = [e for e in inputs if e["frame"] <= 3000]
    cp1, t1 = at(3000, first)
    cp2, t2 = at(3216, inputs)
    e1 = a.add(0, 3000, t1, first, [cp1], None, "checkpoint")
    e2 = a.add(e1, 3216, t2, [e for e in inputs if e["frame"] > 3000], [cp2], None, "end")
    assert a.movie(e2) == inputs and a.goal_entries() == [e2]
    p = subprocess.run([PY, "-u", str(HERE / "explore.py"), "verify", str(arch)], capture_output=True, text=True,
                       timeout=600)
    assert p.returncode == 0, p.stderr[-3000:]
    v = json.loads((arch / f"verify-{e2}.json").read_text())
    assert v["same"] and v["verify"]["differ"] == [] and v["verify"]["final"] == {"owner:566": 1} and v["goal"]
    assert v["frames"] == 488 and v["qf"] == 11712 and abs(v["seconds"] - 48.80) < 0.01 and v["inputs"] == 36
    p = subprocess.run([PY, "-u", str(HERE / "explore.py"), "report", str(arch)], capture_output=True, text=True,
                       timeout=120)
    assert p.returncode == 0, p.stderr[-3000:]
    assert "488 frames, 11712 qf" in p.stdout
    rep = json.loads((arch / "report.json").read_text())
    assert rep["verified"]["frames"] == 488 and rep["verified"]["qf"] == 11712 and rep["goal_chain"]["chain"] == \
        [0, e1, e2]
