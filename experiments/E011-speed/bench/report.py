"""E011 B5 (experiments/E011-speed/plan.md, "Benchmark"): the 15 tries' times by part, per regime, from the tries'
result.json files and B3's times.jsonl.

usage: python report.py B OUT_DIR

Regimes: b0 (the reference during round 1), b4 (the reference interleaved with B3), and B3's a-ref, a-fast, b-ref,
b-fast. For each try, wall and CPU seconds by part, from the try's own result.json, the same rule for Python and Rust:

* return: wall ``return.boot_s``; CPU ``cpu_s.return``;
* idle: wall ``idle.seconds``; CPU ``cpu_s.idle``;
* read: wall, the fresh reading's ``reading.wall_s`` (a reused one's ``load_s``) plus the refinements' and reach
  extensions' ``seconds``; CPU, Python's ``cpu_s.read`` (the reader, the reading's Python side and its loading, the
  refinements' readers), Rust's ``reading.read_cpu_s`` of a fresh reading plus ``cpu_s.read`` (the refinements');
* play: wall, the window records' ``wall_s``; CPU ``cpu_s.play``;
* plan: wall, the attempts' ``seconds`` less play and the refinements' and extensions'; CPU ``cpu_s.plan``;
* the try: wall ``wall_s``; CPU from times.jsonl (the step's process tree) where measured, else ``rusage_total``;
* rest: the try less the five parts.

Medians over the 15 tries, ring(33) (7 tries: 4 rooms) and ring(38) (8 tries: 5 rooms), and the means. Writes
OUT_DIR/tries.md and OUT_DIR/tries.json.
"""

import json
import statistics as st
import sys
from pathlib import Path

PARTS = ("return", "idle", "read", "plan", "play", "rest")
REGIMES = ("b0", "b4", "a-ref", "a-fast", "b-ref", "b-fast")


def load_jsonl(p: Path) -> list:
    return [json.loads(x) for x in p.read_text().splitlines() if x.strip()] if p.exists() else []


def parts(d: Path, cpu_tree: float | None) -> dict | None:
    rp = d / "result.json"
    if not rp.exists():
        return None
    r = json.loads(rp.read_text())
    rust = "explore_s" in r
    a = r.get("attempt") or {}
    cpu = r.get("cpu_s") or {}
    rd = r.get("reading") or {}
    sub = list(a.get("refinements") or ()) + list(a.get("extensions") or ())
    sub_w = sum(x.get("seconds") or 0.0 for x in sub)
    win = []
    for w in sorted(d.glob("run/a*/windows.jsonl")):
        win += load_jsonl(w)
    play_w = sum(x.get("wall_s") or 0.0 for x in win)
    att_w = sum(p.get("seconds") or 0.0 for p in r.get("probes") or ())
    read_w = (rd.get("wall_s") or 0.0 if rd.get("fresh") else rd.get("load_s") or 0.0) + sub_w
    read_c = ((rd.get("read_cpu_s") or 0.0) if rust and rd.get("fresh") else 0.0) + (cpu.get("read") or 0.0)
    out = {
        "return": [(r.get("return") or {}).get("boot_s") or 0.0, cpu.get("return") or 0.0],
        "idle": [(r.get("idle") or {}).get("seconds") or 0.0, cpu.get("idle") or 0.0],
        "read": [read_w, read_c],
        "play": [play_w, cpu.get("play") or 0.0],
        "plan": [max(0.0, att_w - play_w - sub_w), cpu.get("plan") or 0.0],
    }
    tw = r.get("wall_s") or 0.0
    tc = cpu_tree if cpu_tree is not None else cpu.get("rusage_total") or 0.0
    out["rest"] = [max(0.0, tw - sum(v[0] for v in out.values())), max(0.0, tc - sum(v[1] for v in out.values()))]
    out["try"] = [tw, tc]
    out["ok"] = r.get("ok")
    out["stop"] = (a.get("stop") or r.get("error") or "")[:60]
    out["fresh"] = rd.get("fresh")
    out["rust"] = rust
    out["scope"] = len(rd.get("scope") or ())
    return out


def med(xs):
    return st.median(xs) if xs else float("nan")


def main() -> None:
    b, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    times = {(t["regime"], t["job"]): t for t in load_jsonl(b / "b3" / "times.jsonl")}
    jobs = [f"j{n:05d}" for n in range(15)]
    seqs = {"b0": b / "b0", "b4": b / "b4", **{r: b / "b3" / r for r in REGIMES[2:]}}
    data = {}
    for reg, seq in seqs.items():
        for j in jobs:
            t = times.get((reg, j))
            p = parts(seq / "jobs" / j, t["cpu_s"] if t else None)
            if p is not None:
                if t:
                    p["load"] = t.get("load")
                data[(reg, j)] = p
    lines = ["# E011 B5: the 15 tries by part", "", "Wall / CPU seconds. ring(33): 4-room scope; ring(38): 5-room.", ""]
    summary = {}
    for reg in REGIMES:
        rows = [(j, data[(reg, j)]) for j in jobs if (reg, j) in data]
        if not rows:
            continue
        lines += [f"## {reg}", "", "| try | scope | " + " | ".join(PARTS) + " | try | stop |",
                  "|---|---|" + "---:|" * (len(PARTS) + 1) + "---|"]
        for j, p in rows:
            cells = " | ".join(f"{p[k][0]:.2f} / {p[k][1]:.2f}" for k in PARTS)
            lines.append(f"| {j} | {p['scope']} | {cells} | {p['try'][0]:.2f} / {p['try'][1]:.2f} | {p['stop']} |")
        s = {}
        for name, sel in (("all", lambda p: True), ("ring33", lambda p: p["scope"] == 4),
                          ("ring38", lambda p: p["scope"] == 5)):
            ps = [p for _, p in rows if sel(p)]
            s[name] = {k: {"wall_median": med([p[k][0] for p in ps]), "cpu_median": med([p[k][1] for p in ps]),
                           "wall_max": max([p[k][0] for p in ps], default=float("nan")),
                           "wall_mean": st.mean([p[k][0] for p in ps]) if ps else float("nan"),
                           "cpu_mean": st.mean([p[k][1] for p in ps]) if ps else float("nan")}
                       for k in PARTS + ("try",)}
            s[name]["n"] = len(ps)
        summary[reg] = s
        lines += ["", "| over | n | try wall median | try CPU median | try wall mean | try CPU mean | try wall max | "
                  + " | ".join(f"{k} wall med" for k in PARTS) + " |", "|---|" + "---:|" * (6 + len(PARTS))]
        for name in ("all", "ring33", "ring38"):
            x = s[name]
            lines.append(f"| {name} | {x['n']} | {x['try']['wall_median']:.2f} | {x['try']['cpu_median']:.2f} | "
                         f"{x['try']['wall_mean']:.2f} | {x['try']['cpu_mean']:.2f} | {x['try']['wall_max']:.2f} | "
                         + " | ".join(f"{x[k]['wall_median']:.2f}" for k in PARTS) + " |")
        lines.append("")
    (out / "tries.md").write_text("\n".join(lines) + "\n")
    (out / "tries.json").write_text(json.dumps({"tries": {f"{r}/{j}": p for (r, j), p in data.items()},
                                                "summary": summary}, indent=1) + "\n")
    for reg, s in summary.items():
        print(f"{reg}: try wall median {s['all']['try']['wall_median']:.2f} s (ring33 {s['ring33']['try']['wall_median']:.2f}, "
              f"ring38 {s['ring38']['try']['wall_median']:.2f}), mean {s['all']['try']['wall_mean']:.2f}; CPU median "
              f"{s['all']['try']['cpu_median']:.2f}")


if __name__ == "__main__":
    main()
