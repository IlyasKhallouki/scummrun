"""E011 round 2: the summary of bench-r2.sh's outputs (before: round 1's lean build; after: patch 0003 with both cuts).

usage: bench_r2.py OUT     (OUT = ~/.cache/fast-mi-e011/engine-r2/bench-r2)

Prints a markdown table: for each measure the median over the rounds before and after, and after/before. Cycles and
instructions are user space (perf stat, cycles:u and instructions:u); per-frame counts are fmi-engine-frames' 3,000
frames less its 0 frames, so the boot drops out. Also writes OUT/summary.json."""
import json
import re
import statistics
import sys
from pathlib import Path

NUM = re.compile(r"([0-9.]+) (ms|us) ([A-Za-z ]+?)(?:,|$| \()")


def stat(d: Path) -> dict:
    """the one perf stat -x, file under d: cycles, instructions (counts) and task-clock (ms)"""
    (f,) = list(d.glob("stat-*.txt"))
    out = {}
    for line in f.read_text().splitlines():
        p = line.split(",")
        if len(p) < 3 or not p[0] or p[0].startswith("<"):
            continue
        ev = p[2].split(":")[0]
        out[ev] = float(p[0])
    return out


def bench_vals(j: dict) -> dict:
    """fmi-engine bench's text values as numbers: {measure: {"wall": ms, "engine CPU": ms, ...}}"""
    out = {}
    for k, v in j.items():
        vals = {}
        for num, unit, what in NUM.findall(v):
            x = float(num) / (1000 if unit == "us" else 1)
            vals[what.strip()] = x
        m = re.match(r"([0-9.]+) ms wall \(median", v)
        if m:
            vals["wall"] = float(m.group(1))
        m = re.match(r"([0-9.]+) ms$", v)
        if m:
            vals["engine CPU"] = float(m.group(1))
        m = re.match(r"([0-9.]+) ms wall median", v)
        if m:
            vals["wall"] = float(m.group(1))
        out[k] = vals
    return out


def side(d: Path) -> dict:
    r = {}
    b = bench_vals(json.loads((d / "bench.json").read_text()))
    pick = [
        ("bench: frame in 50-frame commands, engine CPU (ms)", "c frame in 50-frame commands", "engine CPU"),
        ("bench: frame in 50-frame commands, wall (ms)", "c frame in 50-frame commands", "wall"),
        ("bench: observed parent frame (Step + facts), wall (ms)", "a parent frame, observed (Step + facts)", "wall"),
        ("bench: observed parent frame, engine CPU (ms)", "a parent frame, observed (Step + facts)", "engine CPU"),
        ("bench: observed parent frame, driver CPU (ms)", "a parent frame, observed (Step + facts)", "driver CPU"),
        ("bench: bare frame (one command), engine CPU (ms)", "c bare frame", "engine CPU"),
        ("bench: start to the first stop, wall (ms)", "start to the first stop", "wall"),
        ("bench: start to the first stop, engine CPU (ms)", "start to the first stop", "engine CPU"),
        ("bench: fork + quit, parent CPU (ms)", "d fork + quit", "parent CPU"),
        ("bench: fork + quit, wall (ms)", "d fork + quit", "wall"),
        ("bench: return by fork of a kept entry, wall (ms, median)", "return by fork of a kept entry", "wall"),
        ("bench: door chain from boot, wall (ms, median of 3)", "entry 5 (frame 3129) replayed from boot", "wall"),
        ("bench: door chain from boot, engine CPU (ms)", "chain from boot, engine CPU", "engine CPU"),
    ]
    for name, k, what in pick:
        v = b.get(k, {}).get(what)
        if v is not None:
            r[name] = v
    for m in ("cmd50", "step"):
        s0, s1 = stat(d / f"frames-{m}-0"), stat(d / f"frames-{m}-3000")
        j1 = json.loads((d / f"frames-{m}-3000.json").read_text())
        n = j1["n"]
        label = "frame in 50-frame commands" if m == "cmd50" else "observed frame (Step + facts)"
        r[f"frames: {label}, user cycles (k)"] = (s1["cycles"] - s0["cycles"]) / n / 1e3
        r[f"frames: {label}, user instructions (k)"] = (s1["instructions"] - s0["instructions"]) / n / 1e3
        r[f"frames: {label}, engine CPU (us)"] = j1["engine_cpu_ms"] / n * 1e3
        r[f"frames: {label}, wall (us)"] = j1["wall_ms"] / n * 1e3
        if m == "step":
            r[f"frames: {label}, driver CPU (us)"] = j1["driver_cpu_ms"] / n * 1e3
        if m == "cmd50":
            r["frames: boot to the root (2728 frames and start), user instructions (M)"] = s0["instructions"] / 1e6
    door = []
    for k in (1, 2, 3):
        s = stat(d / f"door-{k}")
        v = json.loads((d / f"door-{k}.json").read_text())
        assert v.get("differ") == 0, v
        door.append((s, v))
    r["door chain from boot (verify): user instructions (M)"] = statistics.median(s["instructions"] for s, _ in door) / 1e6
    r["door chain from boot (verify): user cycles (M)"] = statistics.median(s["cycles"] for s, _ in door) / 1e6
    r["door chain from boot (verify): engine task-clock (ms)"] = statistics.median(s["task-clock"] for s, _ in door)
    r["door chain from boot (verify): wall incl. start and checks (ms)"] = statistics.median(
        v["seconds"] for _, v in door) * 1e3
    return r


def main():
    out = Path(sys.argv[1])
    rounds = sorted(p for p in out.glob("r*") if p.is_dir())
    data = {"before": [], "after": []}
    for rd in rounds:
        for s in data:
            if (rd / s / "bench.json").exists():
                data[s].append(side(rd / s))
    keys = list(data["after"][0].keys())
    rows, summary = [], {}
    for k in keys:
        b = [x[k] for x in data["before"] if k in x]
        a = [x[k] for x in data["after"] if k in x]
        mb, ma = statistics.median(b), statistics.median(a)
        summary[k] = {"before": b, "after": a, "before_median": mb, "after_median": ma}
        rows.append(f"| {k} | {mb:.4g} | {ma:.4g} | {ma / mb:.2f} |")
    print(f"rounds: {len(rounds)}")
    print("| measure | before (median) | after (median) | after/before |")
    print("|---|---:|---:|---:|")
    print("\n".join(rows))
    (out / "summary.json").write_text(json.dumps(summary, indent=1) + "\n")


if __name__ == "__main__":
    main()
