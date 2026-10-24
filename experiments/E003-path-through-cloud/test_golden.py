"""Tests of golden.py: the canonical encoding, the compare's diff, and that the tracer changes no result.

usage: ../../.venv/bin/python -m pytest test_golden.py   (a few seconds)
"""

import dataclasses
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import golden  # noqa: E402
import model  # noqa: E402


def test_types_are_marked():
    assert golden.dumps((1, [2, 3])) == '{"$tuple":[1,[2,3]]}'
    assert golden.dumps({3, 1, 2}) == '{"$set":[1,2,3]}'
    assert golden.dumps(frozenset({"b", "a"})) == '{"$set":["a","b"]}'
    assert golden.dumps([True, 1, None]) == "[true,1,null]"
    assert golden.dumps({"b": 1, "a": (2,)}) == '{"a":{"$tuple":[2]},"b":1}'


def test_order_rule():
    # ints numerically, strs by code point, a mix by the canonical text of each element
    assert golden.dumps({10, 2, 1}) == '{"$set":[1,2,10]}'
    assert golden.dumps({"above", 3, 1}) == '{"$set":["above",1,3]}'
    assert golden.dumps({(1, "x"), (0, "y")}) == '{"$set":[{"$tuple":[0,"y"]},{"$tuple":[1,"x"]}]}'


def test_maps_and_empty_dicts():
    assert golden.dumps({10: "a", 2: "b"}) == '{"$map":[[2,"b"],[10,"a"]]}'
    assert golden.dumps({("global", 2): {False}}) == '{"$map":[[{"$tuple":["global",2]},{"$set":[false]}]]}'
    assert golden.dumps({}) == "{}"
    try:
        golden.dumps({"$x": 1})
    except ValueError:
        pass
    else:
        raise AssertionError("a $ key must be refused")


def test_dataclasses_and_unknowns():
    u = model.Unknown("timer", "var:11", frozenset({3, 1}))
    assert golden.dumps(u) == '{"$Unknown":{"reason":"timer","values":{"$set":[1,3]},"what":"var:11"}}'
    f = model.Frame(("global", 2), 5, {1: u})
    d = json.loads(golden.dumps(f))["$Frame"]
    assert d["unit"] == {"$tuple": ["global", 2]} and d["locals"]["$map"][0][0] == 1
    assert set(d) == {x.name for x in dataclasses.fields(model.Frame)}


def test_diff_is_typed():
    assert golden.diff({"a": [1, True]}, {"a": [1, True]}) is None
    assert golden.diff({"a": [1, True]}, {"a": [1, 1]}) == ("a[1]", True, 1)
    assert golden.diff([1, 2], [1, 2, 3]) == (".length", 2, 3)
    assert golden.diff({"a": 1}, {"b": 1})[0] == "a"


def _dump(machine_cls, label: str, features) -> list:
    import check
    import groundtruth
    rows = groundtruth.load()
    kinds = check.held_families(rows)
    r = next(x for x in rows if x["name"].split(" ")[0] == label)
    m = machine_cls(features)
    _, action, _ = check.classify(r, m.game)
    run = check.run_model(m, action, model.FactOracle(r["pre"], kinds))
    out = [golden.line({"run": golden.run_dict(run)})]
    for p in run.paths:
        d = golden.path_dict(p)
        d.pop("tid")
        out.append(golden.line(d))
    return out


def test_tracer_changes_nothing():
    for label, row in (("S1#4", 1), ("S1#33", 2), ("S1#2", 5), ("S2#14", 4)):
        feats = golden.ROW_FEATURES[row]
        assert _dump(model.Machine, label, feats) == _dump(golden.TraceMachine, label, feats), label
