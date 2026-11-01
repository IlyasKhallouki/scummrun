"""E004 plan step A4, window-search.md section 9.7, Lemma F's test: children the bridge forks ahead with one ``run``
``fork_each`` command against children forked one frame at a time.

usage (from this folder, with ../../.venv/bin/python, under nice):
       fork_each_check.py --run RUN.json --frame F [--n N] --bin SCUMMVM [--out DIR]

An instrument: nothing the system runs imports it. A fresh engine from boot plays a window search run's inputs up to
frame ``--frame`` (a harness boot's run, replay_part1 ``run``); there it forks a reference branch, then makes N children
with one ``fork_each``; the reference runs one frame at a time and forks at each. Each pair's full state dumps
(SPEEDRUN_DUMP_LOCALS and SPEEDRUN_DUMP_EXACT) are compared at the fork, and again after both get the same click on the
next frame and run 30 frames. It names no game number; the frame is its argument.
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

import replay as RP  # noqa: E402
import replay_part1 as RP1  # noqa: E402
from speedrun import paths  # noqa: E402
from speedrun.control import Controller  # noqa: E402


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", type=Path, required=True)
    ap.add_argument("--frame", type=int, required=True)
    ap.add_argument("--n", type=int, default=7)
    ap.add_argument("--bin", type=Path, required=True)
    ap.add_argument("--out", type=Path, default=paths.OUT_DIR / "E004-fork-each-check")
    a = ap.parse_args()
    paths.SCUMMVM_BIN = a.bin
    doc = json.loads(a.run.read_text())
    if doc.get("starter"):
        raise SystemExit("a run from another start than the harness's boot: not handled")
    ins = [e for e in doc["run"]["inputs"] if e["frame"] <= a.frame]
    a.out.mkdir(parents=True, exist_ok=True)
    cfg = dataclasses.replace(RP1.config(doc["seed"], a.out, RP.write_events(a.out / "movie.jsonl", ins)),
                              dump_locals=True, dump_exact=True)
    rows, bad = [], 0
    with Controller(cfg, timeout_s=600) as c:
        p = c.root
        p.run(frame=a.frame)
        p.segment(True)
        p.runs()
        q = c.fork(p)
        t = time.perf_counter()
        st, kids = c.fork_each(p, a.n)
        t_each = time.perf_counter() - t
        t = time.perf_counter()
        ref = []
        for f in range(a.frame + 1, a.frame + a.n + 1):
            q.run(frame=f)
            ref.append(c.fork(q))
        t_one = time.perf_counter() - t
        for k, r in zip(kids, ref):
            f0 = k.frame
            d0 = sorted(x for x in set(k.state()) | set(r.state()) if k.state().get(x) != r.state().get(x))
            for b in (k, r):
                b.inputs([{"frame": b.frame + 1, "click": [200, 100]}])
                b.run(frame=b.frame + 30)
            sk, sr = k.state(), r.state()
            d1 = sorted(x for x in set(sk) | set(sr) if sk.get(x) != sr.get(x))
            rows.append((f0, r.frame - 30, d0, d1))
            bad += bool(d0) + bool(d1)
            k.quit()
            r.quit()
        q.quit()
    print(f"fork_each_check: {a.run.name}, seed {doc['seed']}, from frame {a.frame}: {a.n} children of one fork_each "
          f"(the parent stopped '{st['stop']}' at {st['frame']}, {len(st.get('forks', []))} forks listed) against {a.n} "
          f"forked one frame at a time; full dumps (locals, exact) compared")
    for f0, f1, d0, d1 in rows:
        print(f"frame {f0} (reference {f1}): top-level keys that differ at the fork {d0}, 30 frames after the same click "
              f"{d1}")
    print(f"pairs with a difference: {bad}; wall: fork_each {t_each * 1000:.1f} ms, one frame and one fork at a time "
          f"{t_one * 1000:.1f} ms")


if __name__ == "__main__":
    main()
