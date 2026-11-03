"""The recorded plan requests that can be rebuilt (plan.md, amendment "targets with no plan made cheap", gate 1).

usage: python3 requests.py OUT_DIR SOURCE=ROOT[,MAP_FROM=MAP_TO] ...

Each SOURCE is an archive (ROOT/jobs/J/{job.json,result.json,run/aNN/run.json,run/aNN/plans/plan-NNN/*.txt}). For
every plan a try's planner searched (source "search"), the request is its plan dir's start.txt and caps.txt, the
attempt's target as the goal, the try's search, and the reading the plan was made on: the try's reading, or the last
refinement's or reach extension's made before that plan. A reading is usable when its folder holds reading.tc; MAP
rewrites a path prefix first (another run's copy of the same reading). Identical requests (reading, start, caps, goal,
search) are kept once, with every record they have.

Writes OUT_DIR/groups/G.tsv (fmi-plan seq's request lines, one file per reading and search), OUT_DIR/groups.json
({G: {"reading", "search", "n"}}) and OUT_DIR/manifest.jsonl (one line per request: name, group, the records' status,
steps, seconds, operators, and where they come from). Requests whose reading is gone are counted in skipped.json.

A test tool: it reads the archives and writes only under OUT_DIR.
"""

import hashlib
import json
import sys
from pathlib import Path


def sha(p: Path) -> str:
    return hashlib.sha1(p.read_bytes()).hexdigest()


def goal_of(target) -> str:
    k, v = target[0], target[1]
    return v if k == "goal" else f"{k}={v}"


def main():
    out = Path(sys.argv[1])
    (out / "groups").mkdir(parents=True, exist_ok=True)
    reqs = {}  # key -> {"name", "group", "records": [...]}
    groups = {}
    skipped = {}
    for spec in sys.argv[2:]:
        src, rest = spec.split("=", 1)
        root, *maps = rest.split(",")
        mp = [m.split("=", 1) for m in maps]

        def resolve(path: str):
            d = str(Path(path).parent)
            for a, b in mp:
                if d.startswith(a):
                    d = b + d[len(a):]
            return d if (Path(d) / "reading.tc").exists() else None

        for jd in sorted((Path(root) / "jobs").iterdir()):
            rp = jd / "result.json"
            if not rp.exists():
                continue
            res = json.loads(rp.read_text())
            if not res.get("reading") or not res["reading"].get("path"):
                continue
            probes = res.get("probes") or []
            for ad in sorted((jd / "run").glob("a[0-9][0-9]")):
                runp = ad / "run.json"
                if not runp.exists():
                    continue
                run = json.loads(runp.read_text())
                k = int(ad.name[1:])
                if k >= len(probes):
                    continue
                target = probes[k]["target"]
                made = sorted([(x["plan"], x["reading"]) for x in run.get("refinements", []) + run.get("extensions", [])
                               if x.get("reading")], key=lambda x: x[0])
                for p in run.get("plans", []):
                    # the Python reference's plans have no "source": each was a search
                    if p.get("source", "search") != "search" or p.get("status") in (None, "reach extension", "same input"):
                        continue
                    if p.get("bans"):
                        skipped["bans"] = skipped.get("bans", 0) + 1
                        continue
                    i = p["plan"]
                    path = res["reading"]["path"]
                    for mi, mpth in made:
                        if mi < i:
                            path = mpth
                    rd = resolve(path)
                    pd = ad / "plans" / f"plan-{i:03d}"
                    if rd is None or not (pd / "start.txt").exists():
                        why = "reading gone" if rd is None else "no start.txt"
                        skipped[why] = skipped.get(why, 0) + 1
                        continue
                    search = "fd" if p.get("search") == "lama-first-costs" else "gbfs"
                    goal = goal_of(target)
                    caps = pd / "caps.txt"
                    key = (rd, sha(pd / "start.txt"), sha(caps) if caps.exists() else "-", goal, search)
                    g = f"{hashlib.sha1(rd.encode()).hexdigest()[:8]}-{search}"
                    groups.setdefault(g, {"reading": rd, "search": search, "n": 0})
                    if key not in reqs:
                        groups[g]["n"] += 1
                        name = f"{src}-{jd.name}-{ad.name}-p{i}"
                        reqs[key] = {"name": name, "group": g, "start": str(pd / "start.txt"),
                                     "caps": str(caps) if caps.exists() else "-", "goal": goal, "records": []}
                    reqs[key]["records"].append({"from": f"{src}/{jd.name}/{ad.name}/plan-{i:03d}", "status": p["status"],
                                                 "steps": p.get("steps"), "seconds": p.get("seconds"),
                                                 "operators": p.get("operators")})
    for g in groups:
        lines = [f"{r['name']}\t{r['start']}\t{r['caps']}\t{r['goal']}" for r in reqs.values() if r["group"] == g]
        (out / "groups" / f"{g}.tsv").write_text("\n".join(lines) + "\n")
    (out / "groups.json").write_text(json.dumps(groups, indent=1) + "\n")
    with open(out / "manifest.jsonl", "w") as f:
        for r in reqs.values():
            f.write(json.dumps(r) + "\n")
    (out / "skipped.json").write_text(json.dumps(skipped, indent=1) + "\n")
    n_rec = sum(len(r["records"]) for r in reqs.values())
    print(f"{len(reqs)} requests ({n_rec} records) in {len(groups)} groups; skipped {skipped}")
    for g, x in sorted(groups.items()):
        print(g, x["search"], x["n"], x["reading"])


if __name__ == "__main__":
    main()
