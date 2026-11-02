"""G7's tail gate: the 97 recorded replans (profile-planner out/tails.txt, from E006-E009's run records), each turned into
a planner request with the previous plan's steps as its tail, where the files it needs are still on disk: the new
plan's start.txt, caps.txt and bans.txt (its plans/plan-NNN directory) and the reading its stats.json names.

usage: tails_make.py TAILS_TXT OUT_DIR
Writes OUT_DIR/manifest.jsonl (one row per replan: whether it can be checked, why not, the recorded answer) and per
reading OUT_DIR/reqs-N.tsv (fmi-plan seq's lines, with the tail file) and OUT_DIR/readings.tsv (N<TAB>reading dir).
"""
import ast
import json
import re
import sys
from pathlib import Path

STRIP = re.compile(r"#\d+$")


def steps_of(p):
    s = p.get("steps")
    s = ast.literal_eval(s) if isinstance(s, str) else s
    return list(s or [])


def goal_text(goal):
    return ",".join(f"{k}={'|'.join(str(v) for v in (vs if isinstance(vs, list) else [vs]))}" for k, vs in goal.items())


def goal_of(run_file: Path, rdir: Path) -> str:
    """The request's goal: a job attempt's target (its log: "attempt aNN: target K=V"), else the reading's goal (a
    route plans to its reading's goal)."""
    if run_file.parent.parent.name == "run" and (run_file.parents[2] / "log").exists():
        att = run_file.parent.name
        for line in (run_file.parents[2] / "log").read_text().splitlines():
            m = re.search(rf"attempt {att}: target (\S+)", line)
            if m:
                return m[1]
    return goal_text(json.load(open(rdir / "reading.json"))["goal"])


def main():
    rows = [json.loads(line) for line in open(sys.argv[1]) if line.startswith("{")]
    out = Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    readings, man = {}, []
    seen = {}
    for r in rows:
        f = Path(r["f"])
        n = seen.get(str(f), 0)
        seen[str(f)] = n + 1
        d = json.load(open(f))
        run = d.get("run", d)
        ps = run.get("plans") or []
        pairs = [(a, b) for a, b in zip(ps, ps[1:]) if a.get("status") == "plan" and b.get("status") == "plan"]
        a, b = pairs[n]
        sa, sb = steps_of(a), steps_of(b)
        row = {"f": str(f), "i": a["plan"], "recorded_tail": r["tail"], "k": r["k"], "old": sa, "new": sb}
        pdir = f.parent / "plans" / f"plan-{b['plan']:03d}"
        st = pdir / "stats.json"
        if not st.exists():
            row["why_not"] = f"no {st}"
            man.append(row)
            continue
        task = Path(json.load(open(st))["task"])
        rdir = task.parent
        if not task.exists() or not (rdir / "reading.json").exists():
            row["why_not"] = f"reading not on disk: {task}"
            man.append(row)
            continue
        if not (pdir / "start.txt").exists():
            row["why_not"] = "no start.txt"
            man.append(row)
            continue
        bans = pdir / "bans.txt"
        if (b.get("bans") or 0) > 0 and not bans.exists():
            row["why_not"] = "bans not on disk"
            man.append(row)
            continue
        k = readings.setdefault(str(rdir), len(readings))
        name = f"r{len(man):03d}"
        tail = out / f"{name}.tail"
        tail.write_text("".join(s + "\n" for s in sa))
        caps = pdir / "caps.txt"
        goal = goal_of(f, rdir)
        line = "\t".join([name, str(pdir / "start.txt"), str(caps) if caps.exists() else "-", goal,
                          str(bans) if bans.exists() else "-", str(tail)])
        with open(out / f"reqs-{k}.tsv", "a") as fh:
            fh.write(line + "\n")
        row.update(name=name, reading=str(rdir), start=str(pdir / "start.txt"), caps=str(caps) if caps.exists() else None,
                   bans=str(bans) if bans.exists() else None, goal=goal)
        man.append(row)
    (out / "manifest.jsonl").write_text("".join(json.dumps(m) + "\n" for m in man))
    (out / "readings.tsv").write_text("".join(f"{k}\t{r}\n" for r, k in readings.items()))
    ok = [m for m in man if "name" in m]
    print(f"replans {len(man)}: checkable {len(ok)} on {len(readings)} readings "
          f"(recorded tails among them {sum(m['recorded_tail'] for m in ok)}); not checkable {len(man) - len(ok)}")
    why = {}
    for m in man:
        if "why_not" in m:
            w = m["why_not"].split(":")[0]
            why[w] = why.get(w, 0) + 1
    print("not checkable:", why)


if __name__ == "__main__":
    main()
