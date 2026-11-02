"""G8 (experiments/E011-speed/plan.md): the Python reference's side of fmi-reading's checks.

usage: python dump_reading.py dump DIR [--every N] --out FILE   PackedTask._action(i) as canonical JSON lines, every
                                                                Nth action (fmi-reading dump writes the same)
       python dump_reading.py names DIR RUST_NAMES              strict.Reading's by_name against fmi-reading names'
                                                                answers, line by line
       python dump_reading.py reuse DIR RUST_KEYS               step.reuse_keys_of on the reading's scope and start
                                                                against fmi-reading reuse-keys' answer

The reading is loaded with packed_reading.load_reading. Exit 0 when the check holds (dump always 0).
"""

import argparse
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
sys.path.insert(0, str(ROOT / "experiments" / "E003-path-through-cloud"))
sys.path.insert(0, str(ROOT / "experiments" / "E008-go-explore"))
sys.path.insert(0, str(ROOT / "src"))

import packed_reading as PR  # noqa: E402


def vals(s) -> list:
    """A set of domain values in the canonical order: "below", the ints ascending, "above"."""
    return sorted(s, key=lambda v: (0, 0) if v == "below" else (2, 0) if v == "above" else (1, v))


def sets(items) -> list:
    return [[k, vals(v)] for k, v in items]


def canonical(task, i: int) -> dict:
    a = task._action(i)
    r = a.what
    effects = []
    for e in a.effects:
        if e.values is not None:
            kind, p = "values", vals(e.values)
        elif e.add is not None:
            kind, p = "add", e.add
        else:
            kind, p = "value", e.value
        effects.append([e.key, kind, p, sets(e.when)])
    reffects = []
    for k, e in r.effects.items():
        if e[0] == "guess":
            reffects.append([k, "guess", vals(e[1])])
        else:
            reffects.append([k, *e])
    return {"i": i, "name": a.name, "cost": a.cost, "what": i, "pre": sets(a.pre.items()), "effects": effects,
            "record": {"kind": r.kind, "label": r.label, "pre": sets(r.pre.items()), "effects": reffects,
                       "checks": [json.dumps(list(c)) for c in r.checks], "end": r.end,
                       "source": json.dumps(list(r.source)), "engine_pre": sorted(r.engine_pre)}}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=("dump", "names", "reuse"))
    ap.add_argument("dir", type=Path)
    ap.add_argument("rust", type=Path, nargs="?")
    ap.add_argument("--every", type=int, default=1)
    ap.add_argument("--out", type=Path)
    a = ap.parse_args()
    rd = PR.load_reading(a.dir / "reading.json")
    if a.cmd == "dump":
        t = time.monotonic()
        with open(a.out, "w") as f:
            for i in range(0, len(rd.task.actions), a.every):
                f.write(json.dumps(canonical(rd.task, i)) + "\n")
        print(json.dumps({"actions": len(rd.task.actions), "dumped": len(range(0, len(rd.task.actions), a.every)),
                          "s": round(time.monotonic() - t, 1)}))
        return
    if a.cmd == "names":
        t = time.monotonic()
        by_name = {x.name: i for i, x in enumerate(rd.task.actions)}  # strict.Reading's by_name
        setup = time.monotonic() - t
        n = bad = 0
        first = []
        for ln in a.rust.read_text().splitlines():
            name, got = json.loads(ln)
            n += 1
            want = by_name.get(name)
            if want != got:
                bad += 1
                if len(first) < 5:
                    first.append([name, want, got])
        print(json.dumps({"names": n, "differ": bad, "first": first, "python_by_name_s": round(setup, 1)}))
        sys.exit(0 if n > 0 and bad == 0 else 1)
    import step
    t = time.monotonic()
    k = step.reuse_keys_of(rd.doc["scope"]["rooms"], rd.doc["start"]["facts"])
    s = time.monotonic() - t
    got = json.loads(a.rust.read_text())
    same = {f: k[f] == got[f] for f in ("keys", "named", "guards")}
    print(json.dumps({"keys": len(k["keys"]), "same": same, "python_s": round(s, 1),
                      "rust_s": got.get("keys_of_s"), "rust_game_load_s": got.get("game_load_s"),
                      "only_python": sorted(set(k["keys"]) - set(got["keys"]))[:10],
                      "only_rust": sorted(set(got["keys"]) - set(k["keys"]))[:10]}))
    sys.exit(0 if all(same.values()) else 1)


if __name__ == "__main__":
    main()
