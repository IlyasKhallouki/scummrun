"""G6's file part (experiments/E011-speed/plan.md): a reading reader-scope wrote with --out-dir (NEW) against the
reference reading read_rust made with E009's binary (REF, ref_read.py).

usage: python cmp_reading.py REF_DIR NEW_DIR

* reading.tc, reading.actions.jsonl, reading.lists.jsonl, reading.reached: the same sha1.
* reading.json: NEW's text is json.dumps(indent=1) of its own value (Python's formatting, byte for byte), and with
  the fields that are measured or name the run's own files (MEASURED) taken from REF, it is REF's text byte for byte.

Prints a JSON report; exit 0 when every check holds.
"""

import hashlib
import json
import sys
from pathlib import Path

FILES = ("reading.tc", "reading.actions.jsonl", "reading.lists.jsonl", "reading.reached")
# read_rust's measured fields (the reader's wall, CPU and peak memory, the process's wall) and the provenance of the
# golden route (the golden file and the packed file read_rust deletes after)
MEASURED = (("read_s",), ("reader_scope", "read_wall_s"), ("reader_scope", "read_cpu_s"),
            ("reader_scope", "peak_rss_mb"), ("from_golden",))


def sha1(p: Path) -> str:
    h = hashlib.sha1()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def main() -> None:
    ref, new = Path(sys.argv[1]), Path(sys.argv[2])
    out = {"files": {}, "json": {}}
    ok = True
    for f in FILES:
        a, b = ref / f, new / f
        if not a.exists() and not b.exists():
            continue
        sa, sb = sha1(a) if a.exists() else None, sha1(b) if b.exists() else None
        out["files"][f] = {"ref": sa, "new": sb, "same": sa == sb}
        ok &= sa == sb
    ta, tb = (ref / "reading.json").read_text(), (new / "reading.json").read_text()
    da, db = json.loads(ta), json.loads(tb)
    canonical = json.dumps(db, indent=1) + "\n" == tb
    taken = []
    for path in MEASURED:
        x, y = da, db
        for k in path[:-1]:
            x, y = x.get(k, {}), y.get(k, {})
        if path[-1] in x or path[-1] in y:
            taken.append({"field": ".".join(path), "ref": x.get(path[-1]), "new": y.get(path[-1])})
            if path[-1] in x:
                y[path[-1]] = x[path[-1]]
            else:
                y.pop(path[-1], None)
    same = json.dumps(db, indent=1) + "\n" == ta
    keys_a, keys_b = list(da), list(db)
    out["json"] = {"new_is_python_formatting": canonical, "same_but_measured": same, "measured_taken_from_ref": taken,
                   "ref_sha1": hashlib.sha1(ta.encode()).hexdigest(), "new_sha1": hashlib.sha1(tb.encode()).hexdigest()}
    if not same:
        out["json"]["key_order_same"] = keys_a == keys_b
        out["json"]["differ"] = sorted(k for k in set(da) | set(db) if da.get(k) != db.get(k))[:20]
    ok &= canonical and same
    out["ok"] = ok
    print(json.dumps(out, indent=1))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
