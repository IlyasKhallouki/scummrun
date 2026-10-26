"""Tests of ``taskcore serve`` (taskcore/src/serve.rs): replanning without reloading writes what ``taskcore sas`` writes.

usage: python test_taskcore_serve.py [N] [seed]    (N random tasks, default 200; the summary on stdout)
       pytest test_taskcore_serve.py               (a fixed case, and the same with N = 40)
       TASKCORE_BIN=<binary>                       (default taskcore/target/release/taskcore)

For each random task (test_taskcore's, with labels as test_replay_taskcore gives them) and each configuration below,
one server takes a sequence of requests, as a replay run gives them: new starts with no bans (the kept task is used),
bans (built), the same bans from another start (kept), other bans and none again (built again), the file's own start
by name, a start with a value outside a domain under both start checks, and another goal. Each request's files (the
SAS file, the operator table, the kept list) must be ``taskcore sas``'s with the same options, byte for byte, and its
stats the same but for times, memory and paths; or both stop, with the same message.
"""

import collections
import json
import random
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import test_replay_taskcore as RT  # noqa: E402
import test_taskcore as TC  # noqa: E402
from export_records import write_bans, write_start  # noqa: E402

BIN = TC.BIN
CONFIGS = [
    ("default", []),
    ("fold every", ["--fold", "every"]),
    ("no (d)", ["--reduce", "a,b,c,e,f"]),
    ("reduce none", ["--reduce", "none"]),
    ("fold off", ["--fold", "off"]),
    ("PRODUCT_LIMIT 1, task encoding", ["--product-limit", "1", "--encoding", "task"]),
]
VOLATILE = ("seconds", "rss", "peak_rss")  # key prefixes: times and memory
PATHS = ("task", "start")  # keys: paths


def strip(x):
    """Stats without times, memory and paths."""
    if isinstance(x, dict):
        return {k: strip(v) for k, v in x.items() if not k.startswith(VOLATILE) and k not in PATHS}
    return x


def outputs(d: Path) -> dict:
    out = {f: (d / f).read_bytes() if (d / f).exists() else None for f in ("r.sas", "r.ops", "r.kept")}
    out["stats"] = strip(json.loads((d / "r.json").read_text())) if (d / "r.json").exists() else None
    return out


def clear(d: Path) -> None:
    d.mkdir(parents=True, exist_ok=True)
    for f in d.iterdir():
        f.unlink()


def rand_start(rng, task, outside_p=0.0) -> dict:
    start = {}
    for k, d in task.facts.items():
        start[k] = rng.choice([v for v in range(-1, 7) if d.label(v) is not None])
    exact = [k for k, d in task.facts.items() if d.exact]
    if exact and rng.random() < outside_p:
        start[rng.choice(exact)] = 9
    return start


def requests(rng, task, start, work: Path) -> list:
    """The request sequence of the module docstring: per request its options (paths in ``work``)."""
    def sf(i, st):
        return ["--start-file", str(write_start(work / f"start{i}.txt", task.facts, st))]

    def bf(i, bans):
        return ["--bans", str(write_bans(work / f"bans{i}.txt", bans))]
    b1, b2 = RT.rand_bans(rng, task), RT.rand_bans(rng, task)
    s1, s2 = rand_start(rng, task), rand_start(rng, task)
    out = [sf(0, start), sf(1, s1), sf(2, start) + bf(2, b1), sf(3, s2) + bf(3, b1), sf(4, s1) + bf(4, b2),
           sf(5, s2), ["--start", "s"], ["--start", "s"] + bf(7, b2)]
    for i, mode in ((8, "needed"), (9, "input")):
        out.append(sf(i, rand_start(rng, task, outside_p=1.0)) + ["--start-check", mode] + (bf(i, b1) if i == 9 else []))
    k = rng.choice(list(task.facts))
    v = rng.choice(list(task.facts[k].values))
    out.append(sf(10, s1) + ["--goal", f"{k}={v}"])
    out.append(sf(11, s2))  # the server's goal again
    return out


def check_task(task, start, work: Path, counts: collections.Counter, fails: list, rng) -> None:
    RT.export(work, task, start)
    reqs = requests(rng, task, start, work)
    for name, args in CONFIGS:
        srv = subprocess.Popen([str(BIN), "serve", "--task", str(work / "t.tc")] + args, stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        hello = srv.stdout.readline()
        if not hello.startswith("ready\t"):
            fails.append((name, "serve did not start", hello, srv.stderr.read()))
            continue
        has_e = "--reduce" not in args or "e" in args[args.index("--reduce") + 1].split(",")
        for i, req in enumerate(reqs):
            if "--start-check" not in req and has_e:
                req = req + ["--start-check", "needed"]  # the harness's
            files = ["--out", "r.sas", "--stats", "r.json", "--ops", "r.ops", "--kept", "r.kept"]
            a_dir, b_dir = work / "sas", work / "serve"
            clear(a_dir)
            clear(b_dir)
            p = subprocess.run([str(BIN), "sas", "--task", str(work / "t.tc")] + args + req +
                               [x if j % 2 == 0 else str(a_dir / x) for j, x in enumerate(files)],
                               capture_output=True, text=True)
            srv.stdin.write("\t".join(req + [x if j % 2 == 0 else str(b_dir / x) for j, x in enumerate(files)]) + "\n")
            srv.stdin.flush()
            line = srv.stdout.readline().rstrip("\n")
            if not line:
                fails.append((name, i, "serve stopped", srv.stderr.read()))
                break
            kind, rest = line.split("\t", 1)
            if p.returncode != 0:
                want = p.stderr.strip().replace("taskcore: ", "", 1)
                if kind != "error" or rest != want:
                    fails.append((name, i, "errors differ", want, line))
                else:
                    counts["the same error"] += 1
                continue
            if kind != "ok":
                fails.append((name, i, "serve stopped where sas did not", rest))
                continue
            a, b = outputs(a_dir), outputs(b_dir)
            if a != b:
                fails.append((name, i, "outputs differ", [k for k in a if a[k] != b[k]], req))
                continue
            counts["the same files"] += 1
            counts[f"{json.loads(rest)['kept']}"] += 1
        srv.stdin.close()
        srv.wait()
    counts["tasks"] += 1


def run_many(n: int, seed: int = 1):
    rng = random.Random(seed)
    counts, fails = collections.Counter(), []
    with tempfile.TemporaryDirectory(prefix="taskcore-serve-test-") as tmp:
        for _ in range(n):
            task, start = TC.rand_task(rng)
            check_task(RT.labelled(task), start, Path(tmp), counts, fails, rng)
    return counts, fails


def test_the_harness_plans_alike_through_the_server(tmp_path):
    """test_replay_taskcore's harness case (a walk banned, another way to open the door, then no plan) with
    ``taskcore_serve`` on: the same plans and stop as through a taskcore process per plan, one server for all of
    them, and the same files in each plan's folder."""
    import plan_fd
    import replay as RP
    from export_records import export_task, record_naming
    runs = {}
    for serve in (False, True):
        b, (r, other, flip), task = RT._harness_task()
        export_task(tmp_path / "k.tc", task, {}, meta=lambda a: (a.what.kind, a.what.end, ""), naming=record_naming)
        h = RP.Harness(b, task, task, RT._fake_branch(), {"state:316": {1}}, packed=tmp_path / "k.tc",
                       workdir=tmp_path / f"plans-{serve}", taskcore_serve=serve, keep_files=True)
        h.observe()
        names = [[s.name for s in h.plan(first=True)]]
        h.bans.append(RP.ban_of(r, h.belief()))
        names.append([s.name for s in h.plan(first=False)])
        h.bans.append(RP.Ban("sentence(2,316)", (("room", 28),)))
        names.append([s.name for s in h.plan(first=False)])
        runs[serve] = (names, h.stop_reason, [p["status"] for p in h.plans], [p.get("serve") for p in h.plans])
        if serve:
            assert len(plan_fd._SERVER) == 1 and plan_fd._SERVER[0].key[1] == str(tmp_path / "k.tc")
    assert runs[True][:3] == runs[False][:3]
    assert runs[False][0] == [["walk_to(316)#0"], ["sentence(1,7)#2", "sentence(2,316)#1"], []]
    assert [x["kept"] for x in runs[True][3]] == ["built", "built", "built"]  # each plan brought a new ban
    for n in range(3):
        for f in ("task.sas", "ops.jsonl"):
            a, b = tmp_path / "plans-False" / f"plan-{n:03d}" / f, tmp_path / "plans-True" / f"plan-{n:03d}" / f
            assert a.read_bytes() == b.read_bytes(), (n, f)
    plan_fd.close_servers()
    assert not plan_fd._SERVER


def test_a_server_error_stops_the_harness_with_its_reason(tmp_path):
    import plan_fd
    import replay as RP
    b, _, task = RT._harness_task()
    h = RP.Harness(b, task, task, RT._fake_branch(), {"state:316": {1}}, packed=tmp_path / "missing.tc",
                   workdir=tmp_path / "plans", taskcore_serve=True)
    h.observe()
    assert h.plan(first=True) == [] and h.plans[0]["status"] == "taskcore error"
    assert h.stop_reason.startswith("planner: ") and "missing.tc" in h.stop_reason
    plan_fd.close_servers()


def test_random_tasks():
    counts, fails = run_many(40, seed=7)
    assert not fails, fails[:3]
    assert counts["the same files"] > 0 and counts["kept"] > 0 and counts["built"] > 0


if __name__ == "__main__":
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    counts, fails = run_many(n, seed)
    for k, v in sorted(counts.items()):
        print(f"{v:8d}  {k}")
    print(f"{len(fails)} failures")
    for f in fails[:10]:
        print(repr(f)[:2000])
    sys.exit(1 if fails else 0)
