"""E004 plan step B1: trace validation with the engine's explanation (trace-validation.md).

usage (from this folder, with ../../.venv/bin/python, under nice):
       trace_check.py check-off --new BIN [--old BIN] [--frames N] [--results]

An instrument: nothing the system runs imports it. It names no game number; the moves, frames and units it reports come
from the runs it reads.

**check-off** (trace-validation.md, Lemma O): three engines play run B's recorded inputs on seed 1 from boot in
lockstep, one frame at a time: the shared build (``--old``), the new build with the bridge's ``segment`` and
``explain`` off, and the new build with ``segment`` on from the first stop and ``explain`` armed for every run of every
frame (each frame's runs and log are read and cleared). At every frame the stop status and the full state dump (with
SPEEDRUN_DUMP_LOCALS and SPEEDRUN_DUMP_EXACT) must be equal across the three; at the end the three trace.jsonl files.
It also counts the runs and log entries the third engine gave, the buffers' overflow counts and Lemma S's premise
(``violations``: opcodes whose ``_currentScript`` was not their run's slot).
"""

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import fork_exact as FX  # noqa: E402
from speedrun import paths  # noqa: E402
from speedrun.control import Controller  # noqa: E402

RESULTS = HERE / "results"
OUT = paths.OUT_DIR / "E004-trace"
SHARED_BIN = Path("/var/tmp/fast-mi/svm-window/build/scummvm/scummvm")


def say(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr, flush=True)


def sha12(path: Path) -> str:
    return hashlib.sha1(Path(path).read_bytes()).hexdigest()[:12]


# --- Lemma O: the controls observe only ------------------------------------------------------------------------------

def cmd_check_off(a) -> dict:
    out = a.out / "check-off"
    inputs = FX.inputs_file(out)
    old, new = Path(a.old), Path(a.new)
    doc = {"old": str(old), "old_sha1": sha12(old), "new": str(new), "new_sha1": sha12(new)}
    engines = {}
    t0 = time.monotonic()
    n = status_diff = off_diff = on_diff = 0
    runs = entries = dropped_runs = dropped_log = violations = 0
    diffs = []
    try:
        for tag, binary in (("old", old), ("new-off", new), ("new-on", new)):
            paths.SCUMMVM_BIN = binary
            c = Controller(FX.config(out / tag, inputs, dump_locals=True, dump_exact=True), timeout_s=600)
            c.start()
            engines[tag] = c
        first = {t: c.root.status.get("frame") for t, c in engines.items()}
        if len(set(first.values())) != 1:
            raise SystemExit(f"first stops differ: {first}")
        frame = first["old"]
        on = engines["new-on"].root
        on.segment(True)
        on.explain(frame + 1, frame + a.frames)
        doc["first_frame"] = frame
        end = frame + a.frames
        while frame < end:
            frame += 1
            st = {t: c.root.run(frame=frame) for t, c in engines.items()}
            if any("end" in s for s in st.values()):
                doc["ended"] = {t: s.get("end", {}).get("reason") for t, s in st.items()}
                break
            n += 1
            r = on.runs()
            lg = on.explain_log()
            runs += len(r["runs"])
            entries += len(lg["explain"])
            dropped_runs += r["dropped"]
            dropped_log += lg["dropped"]
            violations = max(violations, r["violations"], lg["violations"])
            if not (st["old"] == st["new-off"] == st["new-on"]):
                status_diff += 1
                if len(diffs) < a.keep:
                    diffs.append({"frame": frame, "what": "status", "where": FX.first_diff(st["old"], st["new-on"])})
            s_old, s_off, s_on = (engines[t].root.state() for t in ("old", "new-off", "new-on"))
            if FX.text(s_old) != FX.text(s_off):
                off_diff += 1
                if len(diffs) < a.keep:
                    diffs.append({"frame": frame, "what": "controls off", "where": FX.first_diff(s_old, s_off)})
            if FX.text(s_on) != FX.text(s_old):
                on_diff += 1
                if len(diffs) < a.keep:
                    diffs.append({"frame": frame, "what": "controls on", "where": FX.first_diff(s_old, s_on)})
            if n % 500 == 0:
                say(f"check-off: frame {frame}, differences {status_diff}/{off_diff}/{on_diff}, runs {runs}, "
                    f"log entries {entries}")
    finally:
        for c in engines.values():
            c.close()
    traces = {t: sha12(out / t / "trace.jsonl") for t in engines}
    doc.update(frames=n, to_frame=frame, status_differ=status_diff, off_differ=off_diff, on_differ=on_diff,
               trace_sha1=traces, traces_equal=len(set(traces.values())) == 1, runs=runs, log_entries=entries,
               runs_dropped=dropped_runs, log_dropped=dropped_log, violations=violations, differences=diffs,
               seconds=round(time.monotonic() - t0, 1))
    return doc


def check_off_log(doc: dict) -> list:
    lines = [f"B1, Lemma O: the bridge's segment and explain observe only. Shared build {doc['old']} (sha1 "
             f"{doc['old_sha1']}), new build {doc['new']} (sha1 {doc['new_sha1']}); run B's inputs from boot on seed 1, "
             f"every frame from {doc['first_frame']} to {doc['to_frame']}; SPEEDRUN_DUMP_LOCALS and SPEEDRUN_DUMP_EXACT "
             f"on in all three",
             f"{doc['frames']} frames compared; stop status differs on {doc['status_differ']}; the full dump differs, "
             f"new build with the controls off against the shared build, on {doc['off_differ']}; with segment on and "
             f"explain on every run of every frame, on {doc['on_differ']}; trace.jsonl equal: {doc['traces_equal']} "
             f"({doc['trace_sha1']})",
             f"the third engine gave {doc['runs']} slot runs ({doc['runs_dropped']} dropped) and {doc['log_entries']} "
             f"explain entries ({doc['log_dropped']} dropped); opcodes off their run's slot (Lemma S's premise): "
             f"{doc['violations']}; {doc['seconds']} s"]
    for d in doc["differences"]:
        lines.append(f"  differs at frame {d['frame']} ({d['what']}): {d['where']}")
    return lines


# --- run B on seed 1: the ground truth and the model check -------------------------------------------------------

ANY_SEED = 836557856  # plan.md: the Any% seed, never used here
ROW5 = "own_entry,dispatch,calls,room_change,settle,dialog,esc"
FR1 = "actor_rooms,const_polls,draw_clears,input_after_turns,value_sets"  # fix round 1's five, as fr1-merge's row 5


def _truth_at(path: Path) -> None:
    """Point groundtruth.py's file at ``path`` (its ``load`` default too, which Python binds at definition)."""
    import groundtruth
    groundtruth.OUT = path
    groundtruth.load.__defaults__ = (path,)


def cmd_runb_truth(a) -> None:
    """groundtruth.py's ground truth of run B's Part I moves (the same moves, boundaries and dumps), with run B's
    recorded inputs played on ``--seed`` in place of run B's own seed."""
    import groundtruth
    if a.seed == ANY_SEED:
        raise SystemExit("the Any% seed is not used (plan.md)")
    a.out.mkdir(parents=True, exist_ok=True)
    paths.SCUMMVM_BIN = Path(a.bin)
    _truth_at(a.out / f"groundtruth-part1-seed{a.seed}.jsonl")
    groundtruth.SEED = a.seed
    groundtruth.ENGINE_DIR = a.out / "engine"
    sys.argv = [sys.argv[0]]
    groundtruth.main()


def cmd_runb_check(a) -> None:
    """check.py's model check (check_rust.py: the Rust machine) on ``--truth``, row 5 with fix round 1's switches."""
    _truth_at(Path(a.truth).resolve())
    import check_rust
    sys.argv = [str(HERE / "check_rust.py"), "--out", str(a.out), "--bin", str(a.machine), "--features", ROW5,
                "--switches", a.switches, "--jobs", str(a.jobs)]
    check_rust.main()


def write(stem: str, doc: dict, lines: list, results: bool) -> None:
    if results:
        RESULTS.mkdir(exist_ok=True)
        (RESULTS / f"{stem}.json").write_text(json.dumps(doc, indent=1) + "\n")
        (RESULTS / f"{stem}.log").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("check-off")
    p.add_argument("--new", required=True)
    p.add_argument("--old", default=str(SHARED_BIN))
    p.add_argument("--frames", type=int, default=3234)
    p.add_argument("--keep", type=int, default=5)
    p.add_argument("--out", type=Path, default=OUT)
    p.add_argument("--results", action="store_true")
    p = sub.add_parser("runb-truth")
    p.add_argument("--seed", type=int, default=1)
    p.add_argument("--bin", required=True)
    p.add_argument("--out", type=Path, required=True)
    p = sub.add_parser("runb-check")
    p.add_argument("--truth", type=Path, required=True)
    p.add_argument("--machine", type=Path, required=True)
    p.add_argument("--switches", default=FR1)
    p.add_argument("--jobs", type=int, default=3)
    p.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    if a.cmd == "runb-truth":
        cmd_runb_truth(a)
    elif a.cmd == "runb-check":
        cmd_runb_check(a)
    elif a.cmd == "check-off":
        doc = cmd_check_off(a)
        write("trace-check-off", doc, check_off_log(doc), a.results)


if __name__ == "__main__":
    main()
