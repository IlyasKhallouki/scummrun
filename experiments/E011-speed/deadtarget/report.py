"""The gate's report (plan.md, amendment "targets with no plan made cheap", gates 1-3 and the first limit L0).

usage: python3 report.py GATE_DIR > report.json   (a summary on stderr)

For each request of requests.py's manifest: the replay's status, source, proof and seconds (gate.sh's logs and
dumps) against its records. Gate 1 fails on a request with a recorded plan that a proof claims dead, or whose replayed
plan (the step names in order) is not the recorded one. Gate 2 and 3 fail on any "dead check" error (fmi-plan's
FMI_PLAN_DEAD_CHECK). E008's serve sequences: the dumps with the proofs on and off are the same bytes. L0: the p90 of
the found plans' search seconds with Fast Downward's search (the run's), rounded up to 10 s.

A test tool: it reads GATE_DIR only.
"""

import json
import math
import sys
from pathlib import Path


def lines(p: Path):
    return [json.loads(x) for x in p.read_text().splitlines() if x.startswith("{")] if p.exists() else []


def pct(xs, q):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, max(0, math.ceil(q * len(xs)) - 1))] if xs else None


def main():
    g = Path(sys.argv[1])
    out = g / "out"
    man = [json.loads(x) for x in (g / "manifest.jsonl").read_text().splitlines()]
    rows, bad = [], []
    by_name = {}
    for grp in json.loads((g / "groups.json").read_text()):
        for x in lines(out / f"{grp}.log"):
            if "name" in x:
                by_name[x["name"]] = x
        err = out / f"{grp}.err"
        if err.exists():
            for ln in err.read_text().splitlines():
                if "dead check" in ln and "inside h^2, and h^2 equals" not in ln:
                    bad.append(f"{grp}: {ln}")
    for r in man:
        x = by_name.get(r["name"])
        recs = r["records"]
        plan_recs = [c for c in recs if c["status"] == "plan"]
        row = {"name": r["name"], "group": r["group"], "goal": r["goal"], "records": len(recs),
               "recorded": sorted({c["status"] for c in recs}), "from": [c["from"] for c in recs]}
        if x is None:
            row["replay"] = None
            bad.append(f"{r['name']}: not replayed")
            rows.append(row)
            continue
        row.update({k: x.get(k) for k in ("status", "source", "proof", "dead_s", "search_s", "reduce_s", "total_s",
                                           "operators", "steps", "error")})
        dump = out / r["group"] / f"{r['name']}.json"
        names = [s[2] for s in json.loads(dump.read_text())["steps"]] if dump.exists() else None
        if plan_recs:
            if x["status"] != "plan":
                bad.append(f"{r['name']}: recorded plan, replay {x['status']} ({x['source']}, proof {x['proof']})")
            ops = {c["operators"] for c in plan_recs if c.get("operators") is not None}
            row["operators_recorded"] = sorted(ops)
            if ops and x.get("operators") not in ops:
                bad.append(f"{r['name']}: operators {x.get('operators')} vs recorded {sorted(ops)} (another reading?)")
            same = [names == c["steps"] for c in plan_recs]
            row["same_plan"] = all(same)
            if not all(same):
                bad.append(f"{r['name']}: the plan differs from {sum(1 for s in same if not s)} of {len(same)} records")
        if x["source"] == "proof" and plan_recs:
            bad.append(f"{r['name']}: a proof claims no plan for a recorded plan")
        rows.append(row)
    serve = {}
    for r in (38, 33):
        on, off = out / f"serve{r}-on", out / f"serve{r}-off"
        if on.exists() and off.exists():
            diff = [p.name for p in sorted(on.glob("*.json")) if p.read_bytes() != (off / p.name).read_bytes()]
            st = {x["name"]: (x["status"], x["proof"], x["dead_s"]) for x in lines(out / f"serve{r}-on.log") if "name" in x}
            serve[f"serve{r}"] = {"n": len(list(on.glob("*.json"))), "differ": diff, "on": st}
            bad += [f"serve{r}: {d} differs with the proofs on" for d in diff]
            bad += [f"serve{r}: {n} claimed dead" for n, (s, p, _) in st.items() if p in ("h1", "h2")]
    found = [r for r in rows if r.get("status") == "plan" and r.get("source") == "search"]
    fd_found = [r["search_s"] for r in found if r["group"].endswith("-fd")]
    l0 = math.ceil(pct(fd_found, 0.9) / 10) * 10 if fd_found else None
    summary = {
        "requests": len(rows), "records": sum(r["records"] for r in rows),
        "with_recorded_plan": sum(1 for r in rows if "plan" in r["recorded"]),
        "claimed_dead": [(r["name"], r["goal"], r.get("proof"), r.get("dead_s"), r.get("total_s")) for r in rows
                         if r.get("source") == "proof"],
        "same_plan": sum(1 for r in rows if r.get("same_plan")),
        "fd_found_search_s": {"n": len(fd_found), "median": pct(fd_found, 0.5), "p90": pct(fd_found, 0.9),
                              "p99": pct(fd_found, 0.99), "max": max(fd_found) if fd_found else None},
        "L0": l0,
        "dead_s_ring33": sorted(r["dead_s"] for r in rows if r.get("dead_s") and (r.get("operators") or 0) > 50000),
        "dead_s_ring38": sorted(r["dead_s"] for r in rows if r.get("dead_s") and 0 < (r.get("operators") or 0) <= 50000),
        "serve": serve, "bad": bad, "pass": not bad,
    }
    json.dump({"summary": summary, "rows": rows}, sys.stdout, indent=1)
    print(json.dumps(summary, indent=1), file=sys.stderr)


if __name__ == "__main__":
    main()
