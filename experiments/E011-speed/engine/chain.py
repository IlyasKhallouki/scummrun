"""(Copied unchanged but for this note from ~/.cache/fast-mi-e011/profile-engine/scripts/chain.py, the E011 engine
profile's script: the reference side of G2 and G3.)

E011 profile: the door chain from boot (seed 1, entry ENTRY's inputs) on whatever FAST_MI_SCUMMVM_BIN runs.
  chain.py dumps OUT ARCHIVE ENTRY   full state dumps with the exact object (SPEEDRUN_DUMP_EXACT) every 100 frames, at
                                     the archive's entry frames and at the end -> OUT/dumps.jsonl, plus timings
  chain.py speed OUT ARCHIVE ENTRY [REPS]  exec -> first stop, boot to 2728 in one command, 2728 -> entry frame in one
                                     command, a state dump; REPS times
  chain.py compare A/dumps.jsonl B/dumps.jsonl   frames whose dumps differ, and the first differing fields"""
import dataclasses, gzip, json, sys, time
from pathlib import Path
WT = Path("/var/tmp/fast-mi/wt-e009")
sys.path.insert(0, str(WT / "experiments/E003-path-through-cloud")); sys.path.insert(0, str(WT / "src"))


def flat(d, pre=""):
    if isinstance(d, dict):
        for k, v in d.items():
            yield from flat(v, f"{pre}{k}.")
    elif isinstance(d, list) and len(d) < 64:
        for i, v in enumerate(d):
            yield from flat(v, f"{pre}{i}.")
    else:
        yield pre.rstrip("."), d


def compare(a, b):
    A = [json.loads(l) for l in open(a)]; B = [json.loads(l) for l in open(b)]
    bad = 0
    for x, y in zip(A, B):
        if x["frame"] != y["frame"]:
            print("frame sets differ", x["frame"], y["frame"]); return 1
        if x["state"] != y["state"]:
            fx, fy = dict(flat(x["state"])), dict(flat(y["state"]))
            keys = sorted(k for k in set(fx) | set(fy) if fx.get(k) != fy.get(k))
            print(f"frame {x['frame']}: {len(keys)} fields differ: " + "; ".join(f"{k}: {fx.get(k)!r} vs {fy.get(k)!r}" for k in keys[:8]))
            bad += 1
    print(f"{len(A)} vs {len(B)} dumps compared, {bad} differ")
    return 1 if bad or len(A) != len(B) else 0


def main():
    mode = sys.argv[1]
    if mode == "compare":
        sys.exit(compare(sys.argv[2], sys.argv[3]))
    import replay
    import replay_part1 as P
    from speedrun.control import Controller
    out, arch, idx = Path(sys.argv[2]), Path(sys.argv[3]), int(sys.argv[4])
    entry = json.load(gzip.open(arch / "entries" / f"{idx}.json.gz"))
    events = [e for e in entry["events"] if e["frame"] <= entry["frame"]]
    out.mkdir(parents=True, exist_ok=True)
    if mode == "dumps":
        frames = set(range(100, entry["frame"], 100))
        for i in range(idx + 1):
            frames.add(json.load(gzip.open(arch / "entries" / f"{i}.json.gz"))["frame"])
        frames = sorted(f for f in frames if f <= entry["frame"])
        cfg = dataclasses.replace(P.config(1, out / "run", replay.write_events(out / "prefix.jsonl", events), dump_locals=True),
                                  dump_exact=True)
        t0 = time.monotonic()
        ctl = Controller(cfg, timeout_s=600); ctl.start()
        t1 = time.monotonic(); trun = 0.0
        with open(out / "dumps.jsonl", "w") as fh:
            for f in frames:
                t = time.monotonic(); st = ctl.root.run(frame=f); trun += time.monotonic() - t
                if st.get("frame") != f:
                    print("stopped at", st); break
                fh.write(json.dumps({"frame": f, "status": {k: v for k, v in st.items() if k != "stop"}, "state": ctl.root.state()}) + "\n")
        ctl.close()
        print(json.dumps({"dumps": len(frames), "start_s": round(t1 - t0, 3), "run_s": round(trun, 3), "frames": frames[-1],
                          "ms_per_frame": round(1000 * trun / frames[-1], 4)}))
    elif mode == "speed":
        reps = int(sys.argv[5]) if len(sys.argv) > 5 else 3
        for r in range(reps):
            d = out / f"s{r}"; d.mkdir(parents=True, exist_ok=True)
            cfg = P.config(1, d, replay.write_events(d / "prefix.jsonl", events), dump_locals=True)
            t0 = time.monotonic()
            ctl = Controller(cfg, timeout_s=600); ctl.start()
            t1 = time.monotonic()
            st1 = ctl.root.run(frame=2728); t2 = time.monotonic()
            st2 = ctl.root.run(frame=entry["frame"]); t3 = time.monotonic()
            s = ctl.root.state(); t4 = time.monotonic()
            ctl.close(); t5 = time.monotonic()
            print(json.dumps(dict(rep=r, start_s=round(t1 - t0, 4), boot_to_2728_s=round(t2 - t1, 4),
                                  boot_ms_per_frame=round(1000 * (t2 - t1) / (st1["frame"] - 1), 4),
                                  door_frames=st2["frame"] - st1["frame"], door_s=round(t3 - t2, 4),
                                  door_ms_per_frame=round(1000 * (t3 - t2) / (st2["frame"] - st1["frame"]), 4),
                                  state_s=round(t4 - t3, 4), close_s=round(t5 - t4, 4), total_s=round(t5 - t0, 4),
                                  owner566=s.get("owners", {}).get("566"))), flush=True)


if __name__ == "__main__":
    main()
