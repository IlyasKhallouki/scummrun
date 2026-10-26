"""Fix round 1, F10: the bridge dumps each live slot's locals. The check that the new build changes nothing but the
dump.

usage (from this folder, with ../../.venv/bin/python, under nice):
       bridge_locals.py check --new BIN [--old BIN] [--runs room33,bar-door,kitchen] [--margin 300] [--out DIR]
       bridge_locals.py replans --new BIN [--golden FILE] [--out DIR]   (the kitchen run's replans, below)

**The switch.** SPEEDRUN_DUMP_LOCALS (EngineConfig.dump_locals) makes the bridge's state dump list, per live slot,
``locals``: its NUM_SCRIPT_LOCAL locals (vm.localvar, script.h:76, 136), and per cutscene level ``data``: its
cutSceneData (script.h:131; beginCutscene stores it, endCutscene hands it to the end hook, script.cpp:1640, 1658).
Nothing else in the bridge reads the switch. Off, the dump is as before.

**The check.** Three engines play the same recorded inputs from boot in lockstep, one frame at a time: the old build
(``--old``, the shared build/scummvm by default), the new build with the switch off, and the new build with it on.
At every frame, every engine's stop status and state dump are compared: the old build's and the new build's with the
switch off must be equal (the JSON text, key order included); the switch-on dump must equal them once its ``locals``
and ``data`` fields are taken out. At the end the three trace.jsonl files are compared too. The runs:

* ``room33``: results/replay-part1-room33-seed1.json's inputs (seed 1, plan.md's settings, replay_part1.config);
* ``bar-door``: results/replay-part1-bar-door-seed1.json's inputs (seed 1, rooms 33 and 28);
* ``kitchen``: run B's inputs to frame 235 and the kitchen run's inputs (results/replay-kitchen-taskcore-files-all.json,
  replay_kitchen.cfg_of). It replays recorded inputs, so it is a check of the bridge, not a system run.

**The kitchen's replans** (``replans``): the kitchen run (results/replay-kitchen-taskcore-files-all.json) replanned 9
times, with 5 to 9 live slots at args 0 each time. Its recorded inputs are played again on the new build with the
switch on, to each replan's frame, and each live slot whose unit has an ``args:`` fact in the reading (golden.py's
kitchen file, the reading that run planned on) is matched against the reading's built lists at its wait point
(replay.match_list). It counts the slots a list matches and the ones a replan would read again; nothing is planned.

Each run goes on ``--margin`` frames past the run's last frame. The report counts the frames, the live slots seen, the
slots with a nonzero local and the cutscene levels with data, and keeps the first differences.
"""

import argparse
import collections
import dataclasses
import hashlib
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

from speedrun import paths  # noqa: E402
from speedrun.control import Controller  # noqa: E402

RESULTS = HERE / "results"
NEW_FIELDS = {"slots": "locals", "levels": "data"}  # the fields the switch adds: per slot, per cutscene level


def say(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr, flush=True)


def strip(state: dict) -> dict:
    """The state dump without the fields the switch adds."""
    s = dict(state)
    s["slots"] = [{k: v for k, v in sl.items() if k != "locals"} for sl in state["slots"]]
    cs = dict(state["cutscene"])
    cs["levels"] = [{k: v for k, v in lv.items() if k != "data"} for lv in cs["levels"]]
    s["cutscene"] = cs
    return s


def text(d: dict) -> str:
    return json.dumps(d, separators=(",", ":"))


def first_diff(a, b, path="") -> str:
    """Where two JSON values first differ (a path and the two values, shortened)."""
    if type(a) is not type(b):
        return f"{path}: {str(a)[:80]} != {str(b)[:80]}"
    if isinstance(a, dict):
        if list(a) != list(b):
            return f"{path}: keys {list(a)[:12]} != {list(b)[:12]}"
        for k in a:
            if a[k] != b[k]:
                return first_diff(a[k], b[k], f"{path}.{k}")
    if isinstance(a, list):
        if len(a) != len(b):
            return f"{path}: length {len(a)} != {len(b)}"
        for i, (x, y) in enumerate(zip(a, b)):
            if x != y:
                return first_diff(x, y, f"{path}[{i}]")
    return f"{path}: {str(a)[:80]} != {str(b)[:80]}"


def runs_of(names: list, out: Path) -> list:
    """(name, config function of (out_dir, inputs file), the input events from boot, the last frame of the run)."""
    import replay_kitchen
    import replay_part1
    rows = []
    for name in names:
        if name in ("room33", "bar-door"):
            doc = json.loads((RESULTS / f"replay-part1-{name}-seed1.json").read_text())
            events = doc["run"]["inputs"]
            last = max([doc["run"]["frame_end"]] + [s["end"] or 0 for s in doc["run"]["steps"]])
            rows.append((name, lambda o, i: replay_part1.config(1, o, i), events, last))
        elif name == "kitchen":
            doc = json.loads((RESULTS / "replay-kitchen-taskcore-files-all.json").read_text())
            prefix = [e for e in replay_kitchen.prefix_events() if e["frame"] <= doc["run"]["frame0"]]
            rows.append((name, replay_kitchen.cfg_of, prefix + doc["run"]["inputs"], doc["run"]["frame_end"]))
        else:
            raise SystemExit(f"unknown run {name!r}: room33, bar-door or kitchen")
    return rows


def write_inputs(path: Path, events: list) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(e) + "\n" for e in sorted(events, key=lambda e: e["frame"])))
    return path


def check_run(name: str, cfg_of, events: list, last: int, old: Path, new: Path, out: Path, keep: int = 5) -> dict:
    """The three engines in lockstep from boot to ``last`` (module docstring)."""
    inputs = write_inputs(out / name / "inputs.jsonl", events)
    engines = {}
    t0 = time.monotonic()
    row = {"run": name, "inputs": len(events), "last_frame": last, "old": str(old), "new": str(new)}
    try:
        for tag, binary, dump in (("old", old, False), ("new-off", new, False), ("new-on", new, True)):
            cfg = dataclasses.replace(cfg_of(out / name / tag, inputs), dump_locals=dump)
            paths.SCUMMVM_BIN = Path(binary)  # engine.build_argv reads it at call time
            c = Controller(cfg, timeout_s=600)
            c.start()
            engines[tag] = c
        first = {t: c.root.status.get("frame") for t, c in engines.items()}
        row["first_stop"] = first
        if len(set(first.values())) != 1:
            row["error"] = f"first stops differ: {first}"
            return row
        frame = first["old"]
        n = status_diff = off_diff = on_diff = 0
        diffs = []
        counts = collections.Counter()
        while frame < last:
            frame += 1
            st = {t: c.root.run(frame=frame) for t, c in engines.items()}
            if any("end" in s for s in st.values()):
                row["ended"] = {t: s.get("end") for t, s in st.items()}
                break
            n += 1
            if not (st["old"] == st["new-off"] == st["new-on"]):
                status_diff += 1
                if len(diffs) < keep:
                    diffs.append({"frame": frame, "what": "status", "where": first_diff(st["old"], st["new-on"])})
            s_old, s_off, s_on = (engines[t].root.state() for t in ("old", "new-off", "new-on"))
            if text(s_old) != text(s_off):
                off_diff += 1
                if len(diffs) < keep:
                    diffs.append({"frame": frame, "what": "switch off", "where": first_diff(s_old, s_off)})
            bare = strip(s_on)
            if text(bare) != text(s_old):
                on_diff += 1
                if len(diffs) < keep:
                    diffs.append({"frame": frame, "what": "switch on", "where": first_diff(s_old, bare)})
            missing = sum(1 for sl in s_on["slots"] if len(sl.get("locals", [])) != 25) + \
                sum(1 for lv in s_on["cutscene"]["levels"] if "data" not in lv)
            counts["slot or level without the new field"] += missing
            counts["live slots seen"] += len(s_on["slots"])
            counts["live slots with a nonzero local"] += sum(1 for sl in s_on["slots"] if any(sl["locals"]))
            counts["cutscene levels seen"] += len(s_on["cutscene"]["levels"])
            counts["cutscene levels with nonzero data"] += sum(1 for lv in s_on["cutscene"]["levels"] if lv["data"])
            if n % 2000 == 0:
                say(f"{name}: frame {frame}, {n} frames compared, differences {status_diff}/{off_diff}/{on_diff}")
        row.update(frames=n, to_frame=frame, status_differ=status_diff, switch_off_differ=off_diff,
                   switch_on_differ=on_diff, first_differences=diffs, counts=dict(counts))
    finally:
        for c in engines.values():
            c.close()
    traces = {}
    for tag in ("old", "new-off", "new-on"):
        p = out / name / tag / "trace.jsonl"
        traces[tag] = hashlib.sha1(p.read_bytes()).hexdigest() if p.exists() else None
    row["trace_sha1"] = traces
    row["traces_equal"] = len(set(traces.values())) == 1 and None not in traces.values()
    row["seconds"] = round(time.monotonic() - t0, 1)
    return row


def cmd_check(a) -> dict:
    old = Path(a.old) if a.old else paths.BUILD_DIR / "scummvm" / "scummvm"
    new = Path(a.new)
    rows = []
    for name, cfg_of, events, last in runs_of(a.runs.split(","), a.out):
        say(f"{name}: {len(events)} input events, boot to frame {last + a.margin}")
        row = check_run(name, cfg_of, events, last + a.margin, old, new, a.out)
        say(f"{name}: {row.get('frames')} frames; status differ {row.get('status_differ')}, switch off differ "
            f"{row.get('switch_off_differ')}, switch on differ {row.get('switch_on_differ')}; traces equal "
            f"{row.get('traces_equal')}; {row.get('seconds')} s")
        rows.append(row)
    doc = {"old": str(old), "old_sha1": hashlib.sha1(old.read_bytes()).hexdigest()[:12], "new": str(new),
           "new_sha1": hashlib.sha1(new.read_bytes()).hexdigest()[:12], "margin": a.margin, "runs": rows}
    if a.results:
        RESULTS.mkdir(exist_ok=True)
        (RESULTS / "bridge-locals-check.json").write_text(json.dumps(doc, indent=1) + "\n")
        (RESULTS / "bridge-locals-check.log").write_text("\n".join(log_lines(doc)) + "\n")
    print("\n".join(log_lines(doc)))
    return doc


def log_lines(doc: dict) -> list:
    lines = [f"F10: the bridge's locals dump, old build {doc['old']} (sha1 {doc['old_sha1']}) against the new build "
             f"{doc['new']} (sha1 {doc['new_sha1']}), switch off and on, every frame from boot to {doc['margin']} "
             f"frames past each run"]
    for r in doc["runs"]:
        if "error" in r:
            lines.append(f"{r['run']}: error {r['error']}")
            continue
        c = r["counts"]
        lines.append(f"{r['run']}: {r['inputs']} input events, {r['frames']} frames compared (to frame "
                     f"{r['to_frame']}); stop status differs on {r['status_differ']} frames; the dump with the switch "
                     f"off differs from the old build's on {r['switch_off_differ']} frames; with the switch on, "
                     f"without its new fields, on {r['switch_on_differ']}; trace.jsonl equal: {r['traces_equal']}; "
                     f"{r['seconds']} s")
        lines.append(f"  live slots seen {c.get('live slots seen', 0):,}, with a nonzero local "
                     f"{c.get('live slots with a nonzero local', 0):,}; cutscene levels seen "
                     f"{c.get('cutscene levels seen', 0):,}, with nonzero data "
                     f"{c.get('cutscene levels with nonzero data', 0):,}; slots or levels without the new field "
                     f"{c.get('slot or level without the new field', 0)}")
        for d in r["first_differences"]:
            lines.append(f"  differs at frame {d['frame']} ({d['what']}): {d['where']}")
    return lines


def cmd_replans(a) -> dict:
    """The kitchen run's replans against the kitchen reading's lists (module docstring)."""
    import packed_reading as PR
    import replay
    import replay_kitchen
    t0 = time.monotonic()
    lists, args_keys, n = {}, set(), 0
    for ln in PR._golden_lines(a.golden):
        if ln["kind"] != "record":
            continue
        n += 1
        rec = ln["record"]["$Record"]
        args_keys |= {k for k in list(rec["pre"]) + list(rec["effects"]) if k.startswith("args:")}
        if rec.get("next_events"):
            for x in PR.lists_of(PR._dec(rec["next_events"]), PR._dec(rec["effects"])):
                lst = lists.setdefault((x["name"], x["w"]), [])
                if not any(y["sig"] == x["sig"] for y in lst):
                    lst.append({"sig": x["sig"], "locals": {i: v for i, v in x["locals"]}, "override": x["override"],
                                "cut_data": x["cut_data"]})
    say(f"{a.golden}: {n} records, {sum(map(len, lists.values()))} lists, {len(args_keys)} args: facts")
    doc = json.loads((RESULTS / "replay-kitchen-taskcore-files-all.json").read_text())
    run = doc["run"]
    prefix = [e for e in replay_kitchen.prefix_events() if e["frame"] <= run["frame0"]]
    inputs = write_inputs(a.out / "kitchen-replans" / "inputs.jsonl", prefix + run["inputs"])
    paths.SCUMMVM_BIN = Path(a.new)
    cfg = dataclasses.replace(replay_kitchen.cfg_of(a.out / "kitchen-replans" / "run", inputs), dump_locals=True)
    rows = []
    with Controller(cfg, timeout_s=600) as c:
        for p in run["plans"][1:]:
            st = c.root.run(frame=p["frame"])
            if st.get("frame") != p["frame"]:
                raise SystemExit(f"stopped at {st}, not frame {p['frame']}")
            dump = replay.dumped_slots(c.root.state())
            live, matched, unmatched = [], [], []
            for name, slot in sorted(dump["slots"].items()):
                if f"args:{name}" not in args_keys:
                    continue
                live.append(name)
                lst = replay.match_list(lists, slot, slot["w"])
                (matched if lst is not None else unmatched).append(f"{name}@{slot['w']}")
            rows.append({"plan": p["plan"], "frame": p["frame"],
                         "run_args0": p["start_counts"].get("live slots with unknown locals (args 0)", 0),
                         "live": len(live), "matched": matched, "unmatched": unmatched,
                         "skipped": dump["skipped"]})
    out = {"golden": str(a.golden), "records": n, "lists": sum(map(len, lists.values())), "new": a.new,
           "new_sha1": hashlib.sha1(Path(a.new).read_bytes()).hexdigest()[:12], "replans": rows,
           "seconds": round(time.monotonic() - t0, 1)}
    lines = [f"F10: the kitchen run's {len(rows)} replans (results/replay-kitchen-taskcore-files-all.json) on the new "
             f"build (sha1 {out['new_sha1']}) with the locals dump, the live slots against the kitchen reading's "
             f"{out['lists']:,} built lists ({n:,} records); {out['seconds']} s"]
    for r in rows:
        lines.append(f"plan {r['plan']} at frame {r['frame']}: the run had {r['run_args0']} live slots at args 0; "
                     f"{r['live']} live slots with an args: fact, {len(r['matched'])} match a built list, "
                     f"{len(r['unmatched'])} would be read again {r['unmatched']}"
                     + (f"; left out {r['skipped']}" if r["skipped"] else ""))
    tot = (sum(r["run_args0"] for r in rows), sum(r["live"] for r in rows), sum(len(r["matched"]) for r in rows),
           sum(len(r["unmatched"]) for r in rows))
    lines.append(f"in all: {tot[0]} slots at args 0 in the run; {tot[1]} live slots, {tot[2]} matched, {tot[3]} to read "
                 f"again")
    if a.results:
        (RESULTS / "bridge-locals-kitchen-replans.json").write_text(json.dumps(out, indent=1) + "\n")
        (RESULTS / "bridge-locals-kitchen-replans.log").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check")
    c.add_argument("--new", required=True)
    c.add_argument("--old")
    c.add_argument("--runs", default="room33,bar-door,kitchen")
    c.add_argument("--margin", type=int, default=300)
    c.add_argument("--out", type=Path, default=paths.OUT_DIR / "E003-bridge-locals")
    c.add_argument("--results", action="store_true", help="write results/bridge-locals-check.json and .log")
    r = sub.add_parser("replans")
    r.add_argument("--new", required=True)
    r.add_argument("--golden", type=Path, default=paths.OUT_DIR / "golden" / "reader" / "kitchen.jsonl.gz")
    r.add_argument("--out", type=Path, default=paths.OUT_DIR / "E003-bridge-locals")
    r.add_argument("--results", action="store_true",
                   help="write results/bridge-locals-kitchen-replans.json and .log")
    a = ap.parse_args()
    if a.cmd == "check":
        cmd_check(a)
    else:
        cmd_replans(a)


if __name__ == "__main__":
    main()
