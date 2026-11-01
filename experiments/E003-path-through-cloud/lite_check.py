"""E004 plan step A4, window-search.md section 9.7, Lemma D's test: the bridge's lite state dump (``state`` with
``objects``) against the full dump, frame by frame.

usage (from this folder, with ../../.venv/bin/python, under nice):
       lite_check.py --run RUN.json --reading DIR/reading.json --from F --to G --bin SCUMMVM [--out DIR]

An instrument: nothing the system runs imports it. A fresh engine from boot (SPEEDRUN_DUMP_LOCALS on) plays a window
search run's inputs (a harness boot's run, replay_part1 ``run``) and stops at every frame from ``--from`` to ``--to``.
At each, the full dump and the lite dump for the objects the reading's facts name are read, and compared: facts.py's
facts_of on the keys the lite dump has (every fact but the owners, states and classes of other objects), the dumped
slots (replay.dumped_slots), and the fields replay.eval_check reads (vars, actors, room). It names no game number; the
frames are its arguments.
"""

import argparse
import dataclasses
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import facts as F  # noqa: E402
import packed_reading as PR  # noqa: E402
import replay as RP  # noqa: E402
import replay_part1 as RP1  # noqa: E402
from speedrun import paths  # noqa: E402
from speedrun.control import Controller  # noqa: E402


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", type=Path, required=True)
    ap.add_argument("--reading", type=Path, required=True)
    ap.add_argument("--from", dest="lo", type=int, required=True)
    ap.add_argument("--to", dest="hi", type=int, required=True)
    ap.add_argument("--bin", type=Path, required=True)
    ap.add_argument("--out", type=Path, default=paths.OUT_DIR / "E004-lite-check")
    a = ap.parse_args()
    paths.SCUMMVM_BIN = a.bin
    doc = json.loads(a.run.read_text())
    if doc.get("starter"):
        raise SystemExit("a run from another start than the harness's boot: not handled")
    rd = PR.load_reading(a.reading)
    objects = sorted({int(k.split(":")[1]) for k in rd.task.facts if k.startswith(("owner:", "state:", "class:"))})
    a.out.mkdir(parents=True, exist_ok=True)
    cfg = dataclasses.replace(RP1.config(doc["seed"], a.out, RP.write_events(a.out / "movie.jsonl",
                                                                             doc["run"]["inputs"])), dump_locals=True)
    differ, frames, t_full, t_lite, n_full, n_lite = [], 0, 0.0, 0.0, 0, 0
    with Controller(cfg, timeout_s=600) as c:
        b = c.root
        b.run(frame=a.lo)
        for f in range(a.lo, a.hi + 1):
            if f > b.frame:
                b.run(frame=f)
            t = time.perf_counter()
            full = b.state()
            t_full += time.perf_counter() - t
            t = time.perf_counter()
            lite = b.state_lite(objects)
            t_lite += time.perf_counter() - t
            n_full += len(json.dumps(full))
            n_lite += len(json.dumps(lite))
            ff, fl = F.facts_of(full), F.facts_of(lite)
            other = {f"owner:{o}" for o in full["owners"]} | {f"state:{o}" for o in full["states"]}
            keep = lambda k: not (k.split(":")[0] in ("owner", "state", "class") and  # noqa: E731
                                  int(k.split(":")[1]) not in objects)
            d = sorted(k for k in set(ff) | set(fl) if keep(k) and ff.get(k) != fl.get(k))
            if RP.dumped_slots(full) != RP.dumped_slots(lite):
                d.append("dumped slots")
            for k in ("vars", "actors", "room", "verbs", "cutscene", "slots", "bits_set"):
                if full.get(k) != lite.get(k):
                    d.append(k)
            if d:
                differ.append((f, d[:6]))
            frames += 1
            del other
    print(f"lite_check: {a.run.name}, seed {doc['seed']}, frames {a.lo}-{a.hi}: the lite dump for the reading's "
          f"{len(objects)} objects against the full dump at each of {frames} frames")
    print(f"frames with a difference: {len(differ)}; first {differ[:5]}")
    print(f"mean bytes: full {n_full // max(1, frames)}, lite {n_lite // max(1, frames)}; mean round trip and parse: "
          f"full {t_full / max(1, frames) * 1000:.2f} ms, lite {t_lite / max(1, frames) * 1000:.2f} ms")


if __name__ == "__main__":
    main()
