"""E011: two-entry archives for fmi-engine verify, one per new entry of a sequence's tries (verify_new.sh).

usage: python mkverify.py SEQ OUT

OUT/JOB-K/entries/0.json.gz is the root (SEQ/entries/0.json.gz); OUT/JOB-K/entries/1.json.gz is the try's new entry K
with its whole movie from boot (the job's entry's events, then the entry's own) and its checkpoints after the job
entry's last one (fmi-engine verify adds the root's).
"""

import gzip
import json
import sys
from pathlib import Path


def main() -> None:
    seq, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2])
    with gzip.open(seq / "entries" / "0.json.gz", "rt") as f:
        root = json.load(f)
    n = 0
    for d in sorted((seq / "jobs").iterdir()):
        rp = d / "result.json"
        if not rp.exists():
            continue
        res = json.loads(rp.read_text())
        job = json.loads((d / "job.json").read_text())
        ent = job["entry"]
        for k, nw in enumerate(res.get("new") or ()):
            with gzip.open(d / nw["file"], "rt") as f:
                e = json.loads(f.read())
            cps = ([] if ent.get("root") else [ent["checkpoint"]]) + e["checkpoints"]
            doc = {"id": 1, "parent": 0, "frame": e["frame"], "tick": e.get("tick"),
                   "events": ent["events"] + e["events"], "checkpoints": cps}
            a = out / f"{d.name}-{k}" / "entries"
            a.mkdir(parents=True, exist_ok=True)
            for i, x in ((0, root), (1, doc)):
                with gzip.open(a / f"{i}.json.gz", "wt") as f:
                    json.dump(x, f)
            n += 1
    print(f"{out}: {n} entries")


if __name__ == "__main__":
    main()
