"""Plan step A0: fork exactness under load. The lemma and what the test sees: fork-exact.md.

usage (from this folder, with ../../.venv/bin/python, under nice):
       fork_exact.py run --bin BIN [--fork-frame 235] [--frames 3000] [--load 7] [--chunk 100] [--results]
       fork_exact.py check-dump --new BIN [--old BIN] [--frames 3000] [--results]

**The inputs.** Run B's recorded inputs from boot (groundtruth.MOVIE, replay_kitchen.prefix_events), with run B's
settings (replay_kitchen.cfg_of) and seed 1. The kitchen start is ``--fork-frame``, the frame the kitchen run starts
from (results/replay-kitchen-taskcore-files-all.json's frame0). The span is the ``--frames`` frames after it.

**run.** Every engine is the build ``--bin`` with SPEEDRUN_DUMP_LOCALS and SPEEDRUN_DUMP_EXACT on.

1. *The reference F.* A fresh engine boots, runs to the fork frame and then one frame at a time to its end. At every
   stop it keeps the stop status and the state dump.
2. *The parent P.* A second engine boots and runs to the fork frame. It forks the measured child C and ``--load``
   load children there.
3. *The load.* One thread per load child runs it in ``--chunk``-frame run commands. At each chunk's end it compares
   the child's status and dump with F's at that frame. A child that reaches the span's end quits, and the next fork
   of P takes its place.
4. *The measured walk.* C and P step one frame at a time in lockstep through the span. At every frame both are
   compared with F, status and dump. At every frame P also forks once, as the window search's parent does (plan A4).
   That fork becomes a load child if one is needed, and quits at once otherwise.
5. At the end, P's trace.jsonl is compared with F's. Children write no trace.

A comparison is of the JSON texts, key order included. The span's events come from F's stops and trace: room changes,
CD track starts and frames with a cutscene open. Fork times are measured around Controller.fork. Child frames per
second come from the load children's run commands, dumps excluded. Every 50 frames the walk samples the load average
and MemAvailable, and it stops if MemAvailable falls under 3 GB.

**check-dump** (lemma D). Three engines play the same inputs from boot in lockstep, one frame at a time, to the span's
end:

- ``old``: ``--old``, the shared fr1-main build by default;
- ``new-off``: ``--new`` with every switch off;
- ``new-on``: ``--new`` with SPEEDRUN_DUMP_EXACT only.

At every frame the three stop statuses must be equal. new-off's dump must equal old's, and so must new-on's once its
``exact`` object is taken out. At the end the three trace.jsonl files are compared too.

Results: results/fork-exact.json and .log, results/fork-exact-dump-check.json and .log. They hold numbers and JSON
paths only. The dumps, which hold names from the game, stay under out/.
"""

import argparse
import dataclasses
import hashlib
import json
import os
import queue
import statistics
import sys
import threading
import time
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

from speedrun import paths  # noqa: E402
from speedrun.control import Controller  # noqa: E402

RESULTS = HERE / "results"
OUT = paths.OUT_DIR / "E003-fork-exact"
OLD_BIN = Path("/var/tmp/fast-mi/svm-fr1-main/build/scummvm/scummvm")
MEM_FLOOR_KB = 3 * 1024 * 1024


def say(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr, flush=True)


def text(d: dict) -> str:
    return json.dumps(d, separators=(",", ":"))


def sha(s: str) -> str:
    return hashlib.sha1(s.encode()).hexdigest()


def first_diff(a, b, path="") -> str:
    """Where two JSON values first differ: a path, and the two values if they are numbers (no game text)."""
    def show(v):
        return str(v) if isinstance(v, (int, float, bool)) or v is None else f"<{type(v).__name__}>"
    if type(a) is not type(b):
        return f"{path}: {show(a)} != {show(b)}"
    if isinstance(a, dict):
        if list(a) != list(b):
            return f"{path}: keys differ ({len(a)} against {len(b)})"
        for k in a:
            if a[k] != b[k]:
                return first_diff(a[k], b[k], f"{path}.{k}")
    if isinstance(a, list):
        if len(a) != len(b):
            return f"{path}: length {len(a)} != {len(b)}"
        for i, (x, y) in enumerate(zip(a, b)):
            if x != y:
                return first_diff(x, y, f"{path}[{i}]")
    return f"{path}: {show(a)} != {show(b)}"


def mem_available_kb() -> int:
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemAvailable:"):
            return int(line.split()[1])
    return 0


def loadavg() -> float:
    return float(Path("/proc/loadavg").read_text().split()[0])


def inputs_file(out: Path) -> Path:
    import replay_kitchen
    events = replay_kitchen.prefix_events()
    path = out / "inputs.jsonl"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(e) + "\n" for e in sorted(events, key=lambda e: e["frame"])))
    return path


def config(out_dir: Path, inputs: Path, **switches):
    import replay_kitchen
    return dataclasses.replace(replay_kitchen.cfg_of(out_dir, inputs, seed=1), **switches)


def quantiles(xs: list) -> dict:
    if not xs:
        return {}
    s = sorted(xs)
    return {"n": len(s), "median": round(statistics.median(s), 3), "p90": round(s[int(0.9 * (len(s) - 1))], 3),
            "min": round(s[0], 3), "max": round(s[-1], 3)}


class Reference:
    """F's stop status and dump at every frame, as hashes and compressed texts."""

    def __init__(self):
        self.frames = {}

    def put(self, frame: int, status: dict, state: dict) -> None:
        st, sd = text(status), text(state)
        self.frames[frame] = (sha(st), sha(sd), zlib.compress(st.encode(), 1), zlib.compress(sd.encode(), 1))

    def check(self, frame: int, status: dict | None, state: dict) -> str | None:
        """None if equal to F at ``frame``, else where they first differ. ``status`` None: the dump only."""
        h_st, h_sd, z_st, z_sd = self.frames[frame]
        if status is not None and sha(text(status)) != h_st:
            return "status " + first_diff(json.loads(zlib.decompress(z_st)), status)
        if sha(text(state)) != h_sd:
            return "dump " + first_diff(json.loads(zlib.decompress(z_sd)), state)
        return None


def cmd_run(a) -> dict:
    out = a.out
    binary = Path(a.bin)
    paths.SCUMMVM_BIN = binary  # engine.build_argv reads it at call time
    inputs = inputs_file(out)
    n0, end = a.fork_frame, a.fork_frame + a.frames
    sw = {"dump_locals": True, "dump_exact": True}
    doc = {"bin": str(binary), "bin_sha1": hashlib.sha1(binary.read_bytes()).hexdigest()[:12], "seed": 1,
           "fork_frame": n0, "frames": a.frames, "end_frame": end, "load_children": a.load, "chunk": a.chunk,
           "cpus": os.cpu_count()}

    # 1. the reference
    say(f"reference: boot, run to frame {n0}, then one frame at a time to {end}")
    ref = Reference()
    t0 = time.monotonic()
    rooms, cutscene_frames, steps = [], 0, []
    with Controller(config(out / "fresh", inputs, **sw), timeout_s=600) as c:
        st = c.root.run(frame=n0)
        ref.put(n0, st, c.root.state())
        room = st["room"]
        for f in range(n0 + 1, end + 1):
            t = time.perf_counter()
            st = c.root.run(frame=f)
            steps.append((time.perf_counter() - t) * 1000)
            if "end" in st:
                raise SystemExit(f"the reference ended at frame {f}: {st['end'].get('reason')}")
            ref.put(f, st, c.root.state())
            if st["room"] != room:
                rooms.append(f)
                room = st["room"]
            cutscene_frames += st["cs"] > 0
    trace_f = (out / "fresh" / "trace.jsonl").read_bytes()
    cd = [r for r in map(json.loads, trace_f.decode().splitlines()) if r["type"] == "cd_track" and n0 < r["frame"] <= end]
    doc["reference"] = {"seconds": round(time.monotonic() - t0, 1), "step_ms": quantiles(steps),
                        "room_changes": len(rooms), "first_room_change": rooms[0] if rooms else None,
                        "cutscene_frames": cutscene_frames, "cd_track_starts": len(cd),
                        "cd_tracks_playing": sum(1 for r in cd if r["playing"]),
                        "first_cd_track_start": cd[0]["frame"] if cd else None}
    say(f"reference: {doc['reference']}")

    # 2. the parent and the children at the fork frame
    lock = threading.Lock()
    diffs, counts = [], {"compared": 0, "differ": 0}
    load_counts = {"compared": 0, "differ": 0, "children": 0, "chunks": 0}
    fork_ms, fps, loads, mems = [], [], [], []

    def note(who: str, frame: int, where: str | None, tally: dict) -> None:
        with lock:
            tally["compared"] += 1
            if where is not None:
                tally["differ"] += 1
                if len(diffs) < a.keep:
                    diffs.append({"who": who, "frame": frame, "where": where})

    ctl = Controller(config(out / "parent", inputs, **sw), timeout_s=600)
    ctl.start()
    P = ctl.root
    P.run(frame=n0)

    def fork():
        t = time.perf_counter()
        ch = ctl.fork(P)
        fork_ms.append((time.perf_counter() - t) * 1000)
        return ch

    C = fork()
    note("C", n0, ref.check(n0, None, C.state()), counts)
    pool: queue.Queue = queue.Queue()
    for _ in range(a.load):
        pool.put(fork())
    note("P", n0, ref.check(n0, None, P.state()), counts)
    wanted = threading.Semaphore(0)  # load slots waiting for a child
    stop = threading.Event()

    def load_worker(k: int) -> None:
        while not stop.is_set():
            try:
                ch = pool.get(timeout=0.2)
            except queue.Empty:
                continue
            with lock:
                load_counts["children"] += 1
            frame = ch.status["frame"]
            try:
                while frame < end and not stop.is_set():
                    target = min(frame + a.chunk, end)
                    t = time.perf_counter()
                    st = ch.run(frame=target)
                    dt = time.perf_counter() - t
                    if "end" in st:
                        note(f"L{k}", target, f"ended: {st['end'].get('reason')}", load_counts)
                        break
                    if target - frame >= a.chunk // 2:
                        with lock:
                            fps.append((target - frame) / dt)
                    frame = target
                    with lock:
                        load_counts["chunks"] += 1
                    note(f"L{k}", frame, ref.check(frame, st, ch.state()), load_counts)
            finally:
                ch.quit()
            wanted.release()

    workers = [threading.Thread(target=load_worker, args=(k,), daemon=True) for k in range(a.load)]
    for w in workers:
        w.start()

    # 4. the measured walk
    say(f"walk: C and P from {n0} to {end}, {a.load} load children")
    t0 = time.monotonic()
    walk_ms, stopped = [], None
    for f in range(n0 + 1, end + 1):
        t = time.perf_counter()
        for b in (C, P):
            b._send({"cmd": "run", "frame": f})
        sts = [b._await_stop() for b in (C, P)]
        walk_ms.append((time.perf_counter() - t) * 1000)
        for who, b, st in (("C", C, sts[0]), ("P", P, sts[1])):
            if "end" in st:
                note(who, f, f"ended: {st['end'].get('reason')}", counts)
                stopped = f"{who} ended at frame {f}"
                break
            note(who, f, ref.check(f, st, b.state()), counts)
        if stopped:
            break
        ch = fork()  # the window search's per-frame fork
        if f < end and wanted.acquire(blocking=False):
            pool.put(ch)
            while f < end and wanted.acquire(blocking=False):
                pool.put(fork())
        else:
            ch.quit()
        if f % 50 == 0:
            loads.append(loadavg())
            mems.append(mem_available_kb())
            if mems[-1] < MEM_FLOOR_KB:
                stopped = f"MemAvailable {mems[-1] // 1024} MB at frame {f}"
                break
            if f % 500 == 0:
                say(f"walk: frame {f}, compared {counts['compared']} (+{load_counts['compared']} load), "
                    f"differ {counts['differ']} (+{load_counts['differ']}), load {loads[-1]}")
    walk_s = time.monotonic() - t0
    stop.set()
    for w in workers:
        w.join(timeout=60)
    while not pool.empty():
        pool.get().quit()
    C.quit()
    ctl.close()
    trace_p = (out / "parent" / "trace.jsonl").read_bytes()
    doc["walk"] = {"seconds": round(walk_s, 1), "stopped": stopped, "frame_ms": quantiles(walk_ms),
                   "C_and_P": counts, "load": load_counts, "differences": diffs,
                   "trace_sha1": {"F": hashlib.sha1(trace_f).hexdigest()[:12],
                                  "P": hashlib.sha1(trace_p).hexdigest()[:12]},
                   "traces_equal": trace_f == trace_p}
    doc["fork_ms"] = quantiles(fork_ms)
    doc["child_fps"] = quantiles(fps)
    doc["loadavg"] = quantiles(loads)
    doc["mem_available_mb"] = quantiles([m / 1024 for m in mems])
    return doc


def run_log(doc: dict) -> list:
    r, w = doc["reference"], doc["walk"]
    lines = [
        f"A0: fork exactness under load. Build {doc['bin']} (sha1 {doc['bin_sha1']}), seed {doc['seed']}, run B's "
        f"inputs from boot; fork at frame {doc['fork_frame']}, {doc['frames']} frames to frame {doc['end_frame']}; "
        f"dumps with SPEEDRUN_DUMP_LOCALS and SPEEDRUN_DUMP_EXACT; {doc['load_children']} load children; "
        f"{doc['cpus']} CPUs",
        f"span: {r['room_changes']} room changes (first at frame {r['first_room_change']}), {r['cutscene_frames']} "
        f"frames with a cutscene open, {r['cd_track_starts']} CD track starts ({r['cd_tracks_playing']} playing, "
        f"first at frame {r['first_cd_track_start']})",
        f"reference F: {r['seconds']} s; per-frame run command {r['step_ms']} ms",
        f"walk: {w['seconds']} s; per frame (C and P run) {w['frame_ms']} ms; stopped early: {w['stopped']}",
        f"C and P against F: {w['C_and_P']['compared']} frame comparisons (status and dump), "
        f"{w['C_and_P']['differ']} differ",
        f"load children against F: {w['load']['children']} children, {w['load']['chunks']} chunks, "
        f"{w['load']['compared']} comparisons, {w['load']['differ']} differ",
        f"P's trace.jsonl equals F's: {w['traces_equal']} ({w['trace_sha1']})",
        f"fork ms: {doc['fork_ms']}",
        f"child frames per second (load children, run commands only): {doc['child_fps']}",
        f"load average (1 min) during the walk: {doc['loadavg']}; MemAvailable MB: {doc['mem_available_mb']}",
    ]
    for d in w["differences"]:
        lines.append(f"  differs: {d['who']} at frame {d['frame']}: {d['where']}")
    return lines


def strip_exact(state: dict) -> dict:
    return {k: v for k, v in state.items() if k != "exact"}


def cmd_check_dump(a) -> dict:
    out = a.out / "dump-check"
    inputs = inputs_file(out)
    end = a.fork_frame + a.frames
    old, new = Path(a.old), Path(a.new)
    engines = {}
    t0 = time.monotonic()
    doc = {"old": str(old), "old_sha1": hashlib.sha1(old.read_bytes()).hexdigest()[:12], "new": str(new),
           "new_sha1": hashlib.sha1(new.read_bytes()).hexdigest()[:12], "end_frame": end}
    n = status_diff = off_diff = on_diff = exact_missing = 0
    diffs = []
    try:
        for tag, binary, exact in (("old", old, False), ("new-off", new, False), ("new-on", new, True)):
            paths.SCUMMVM_BIN = binary
            c = Controller(config(out / tag, inputs, dump_exact=exact), timeout_s=600)
            c.start()
            engines[tag] = c
        first = {t: c.root.status.get("frame") for t, c in engines.items()}
        if len(set(first.values())) != 1:
            raise SystemExit(f"first stops differ: {first}")
        frame = first["old"]
        doc["first_frame"] = frame
        while frame < end:
            frame += 1
            st = {t: c.root.run(frame=frame) for t, c in engines.items()}
            if any("end" in s for s in st.values()):
                doc["ended"] = {t: s.get("end", {}).get("reason") for t, s in st.items()}
                break
            n += 1
            if not (st["old"] == st["new-off"] == st["new-on"]):
                status_diff += 1
                if len(diffs) < a.keep:
                    diffs.append({"frame": frame, "what": "status", "where": first_diff(st["old"], st["new-on"])})
            s_old, s_off, s_on = (engines[t].root.state() for t in ("old", "new-off", "new-on"))
            if text(s_old) != text(s_off):
                off_diff += 1
                if len(diffs) < a.keep:
                    diffs.append({"frame": frame, "what": "switch off", "where": first_diff(s_old, s_off)})
            if text(strip_exact(s_on)) != text(s_old):
                on_diff += 1
                if len(diffs) < a.keep:
                    diffs.append({"frame": frame, "what": "switch on", "where": first_diff(s_old, strip_exact(s_on))})
            exact_missing += "exact" not in s_on or "exact" in s_off or "exact" in s_old
            if n % 1000 == 0:
                say(f"dump check: frame {frame}, differences {status_diff}/{off_diff}/{on_diff}")
    finally:
        for c in engines.values():
            c.close()
    traces = {t: hashlib.sha1((out / t / "trace.jsonl").read_bytes()).hexdigest()[:12] for t in engines}
    doc.update(frames=n, to_frame=frame, status_differ=status_diff, switch_off_differ=off_diff,
               switch_on_differ=on_diff, exact_object_misplaced=exact_missing, trace_sha1=traces,
               traces_equal=len(set(traces.values())) == 1, differences=diffs,
               seconds=round(time.monotonic() - t0, 1))
    return doc


def dump_log(doc: dict) -> list:
    lines = [f"A0, lemma D: SPEEDRUN_DUMP_EXACT changes nothing else. Old build {doc['old']} (sha1 {doc['old_sha1']}), "
             f"new build {doc['new']} (sha1 {doc['new_sha1']}); run B's inputs from boot, seed 1, every frame from "
             f"{doc['first_frame']} to {doc['to_frame']}; every other switch off",
             f"{doc['frames']} frames compared; stop status differs on {doc['status_differ']}; the new build's dump "
             f"with the switch off differs from the old build's on {doc['switch_off_differ']}; with the switch on, "
             f"without its exact object, on {doc['switch_on_differ']}; frames where the exact object is missing or "
             f"misplaced {doc['exact_object_misplaced']}; trace.jsonl equal: {doc['traces_equal']} "
             f"({doc['trace_sha1']}); {doc['seconds']} s"]
    for d in doc["differences"]:
        lines.append(f"  differs at frame {d['frame']} ({d['what']}): {d['where']}")
    return lines


def write(stem: str, doc: dict, lines: list, results: bool) -> None:
    if results:
        RESULTS.mkdir(exist_ok=True)
        (RESULTS / f"{stem}.json").write_text(json.dumps(doc, indent=1) + "\n")
        (RESULTS / f"{stem}.log").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("run", "check-dump"):
        p = sub.add_parser(name)
        p.add_argument("--fork-frame", type=int, default=235)
        p.add_argument("--frames", type=int, default=3000)
        p.add_argument("--keep", type=int, default=5, help="differences kept in the report")
        p.add_argument("--out", type=Path, default=OUT)
        p.add_argument("--results", action="store_true")
        p.add_argument("--tag", default="", help="the results' name: results/fork-exact[-dump-check]-TAG")
        if name == "run":
            p.add_argument("--bin", required=True)
            p.add_argument("--load", type=int, default=7)
            p.add_argument("--chunk", type=int, default=100)
        else:
            p.add_argument("--new", required=True)
            p.add_argument("--old", default=str(OLD_BIN))
    a = ap.parse_args()
    suffix = f"-{a.tag}" if a.tag else ""
    if a.cmd == "run":
        doc = cmd_run(a)
        write("fork-exact" + suffix, doc, run_log(doc), a.results)
    else:
        doc = cmd_check_dump(a)
        write("fork-exact-dump-check" + suffix, doc, dump_log(doc), a.results)


if __name__ == "__main__":
    main()
