"""The reference planner: plan_fd.plan_taskcore with serve=True and lama-first-costs (E009's settings) on a request
sequence, each result as canonical JSON in the form fmi-plan's `result_json` writes (the differential test G7).

usage: plan_ref.py READING_DIR REQS OUT_DIR [TASKCORE_BIN]
  REQS: `name<TAB>start.txt<TAB>caps.txt<TAB>goal[<TAB>bans.txt]` per line ("-" for none), as fmi-plan seq reads them
"""
import json
import os
import sys
import time
from pathlib import Path

E003 = Path(__file__).resolve().parents[4] / "E003-path-through-cloud"
sys.path.insert(0, str(E003))
sys.path.insert(0, str(E003.parents[1] / "src"))
import packed_reading as PR  # noqa: E402
import plan_fd  # noqa: E402
import replay  # noqa: E402


def value(t):
    try:
        return int(t)
    except ValueError:
        return t


def start_of(text, facts):
    out = {}
    for line in text.splitlines():
        if not line:
            continue
        k, v = line.split("\t", 1)
        if v == "":
            d, x = facts[k], 999_999_999
            while d.label(x) is not None:
                x -= 1
            out[k] = x
        else:
            out[k] = value(v)
    return out


def vals(vs):
    return sorted(str(v) for v in vs)


def action(a):
    effs = []
    for e in a.effects:
        if e.values is not None:
            kind, payload = "guess", vals(e.values)
        elif e.add is not None:
            kind, payload = "add", str(e.add)
        else:
            kind, payload = "set", str(e.value)
        effs.append([e.key, kind, payload, [[c, vals(s)] for c, s in e.when]])
    return {"name": a.name, "pre": sorted([k, vals(v)] for k, v in a.pre.items()), "effects": effs, "cost": a.cost}


def canon(res):
    return {"status": res.status, "plan_cost": res.plan_cost, "predicted_goal": res.predicted_goal,
            "file_facts": list(res.file_facts), "needed_facts": list(res.needed_facts), "folds": list(res.folds),
            "steps": [[s.op, s.action, s.name, [[k, str(v)] for k, v in s.chosen],
                       [[k, str(v), [[c, vals(x)] for c, x in w]] for k, v, w in s.guessed]] for s in res.steps],
            "paths": [action(a) for a in res.paths], "actions": [action(a) for a in res.actions],
            "choices": {str(i): [action(a) for a in ps] for i, ps in res.choices.items()},
            "misses": [[k, str(v)] for k, v in res.misses], "n_misses": res.n_misses}


def main():
    rdir, reqs, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]).resolve()
    binary = sys.argv[4] if len(sys.argv) > 4 else os.environ["TASKCORE_BIN"]
    out.mkdir(parents=True, exist_ok=True)
    rd = PR.load_reading(rdir / "reading.json")
    task = rd.task
    for line in reqs.read_text().splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        f = line.split("\t")
        name, st, caps, goal_text = f[:4]
        bans = []
        if len(f) > 4 and f[4] not in ("", "-"):
            for b in Path(f[4]).read_text().splitlines():
                p = b.split("\t")
                bans.append(replay.Ban(p[0], tuple((p[i], value(p[i + 1])) for i in range(1, len(p), 2))))
        goal = {}
        for part in goal_text.split(","):
            k, vs = part.split("=")
            goal[k] = {value(v) for v in vs.split("|")}
        start = start_of(Path(st).read_text(), task.facts)
        args = ["--goal", goal_text] + (["--cost-caps", caps] if caps not in ("", "-") else [])
        t0 = time.monotonic()
        c0 = time.process_time()
        res = plan_fd.plan_taskcore(rd.packed, task.facts, goal, task.ban_pieces(bans), start, bans,
                                    search="lama-first-costs", workdir=out / f"work-{name}", binary=binary, args=args,
                                    serve=True)
        (out / f"{name}.json").write_text(json.dumps(canon(res)) + "\n")
        print(json.dumps({"name": name, "status": res.status, "steps": len(res.steps), "wall_s": round(time.monotonic() - t0, 2),
                          "py_cpu_s": round(time.process_time() - c0, 2), "write_s": round(res.write_s, 2),
                          "fd_s": round(res.seconds, 2), "decode_s": round(res.decode_s, 2)}), flush=True)
    plan_fd.close_servers()


if __name__ == "__main__":
    main()
