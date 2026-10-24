"""E003 step 4, a measurement: how the reading and the task grow with the scope, from Part I's real start.

usage: python size_scaling.py graph                                   (the room graph and the scopes, printed)
       python size_scaling.py run --ks 0 1 2 3 --workers 16 --out DIR  (the supervisor: one child process per scope)
       python size_scaling.py child --k K --workers N --out DIR       (one scope; the supervisor starts it)
       python size_scaling.py report --jsonl F --log F                (the table and the fits, from the lines)
       options of run: --max-used-gb 100 (system memory in use, MemTotal - MemAvailable), --min-free-gb 0
       (MemAvailable floor; 3 on the laptop), --step-minutes 60

A measurement only: it imports reader.py, task.py, relevance.py and domains.py as they are and changes nothing.

Start: results/start-part1.jsonl, seed 1 (the first idle stop from boot), for the start and for the domains
(domains.build(start, goals)). Goal: results/end-part1.json ``goal_all``.

Scopes, from the IR alone: room R links to R' when code that belongs to R (its entry and exit code, its local
scripts, the scripts of the objects whose room is R) has loadRoom(R') or loadRoomWithEgo(obj, R') with a constant R'.
A pseudo room (>= 0x80) is mapped to its room resource (model.Game.pseudo); a link to R itself is dropped. Not links:
Resource.loadRoom (loads a resource, no room change), actorFollowCamera and putActorInRoom (no constant room).
Scope k is the rooms within k links of the start room; reader.Scope.of adds the global scripts they reach.

Per scope, in a child process: Game.load, domains.build, Scope.of, read_all(workers), build_task, relevance, prune,
write_sas, and the task file written to disk (head, state, tail, never the whole text in memory). The supervisor
samples memory every 2 s and stops the scope (kills its process group) when the system's memory in use passes
--max-used-gb, MemAvailable falls under --min-free-gb, or a step passes --step-minutes; the line then says where.
"""

import argparse
import collections
import hashlib
import json
import math
import os
import pickle
import resource
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
START = HERE / "results" / "start-part1.jsonl"
END = HERE / "results" / "end-part1.json"
SEED = 1
SOURCES = ("reader.py", "model.py", "task.py", "relevance.py", "domains.py", "ir.py", "actions.py", "facts.py")


# --- the scopes ----------------------------------------------------------------------------------------------------

def start_facts() -> dict:
    for line in START.read_text().splitlines():
        row = json.loads(line)
        if row["seed"] == SEED:
            return row["facts"]
    raise SystemExit(f"no seed {SEED} in {START}")


def goal() -> dict:
    g = json.loads(END.read_text())
    assert not g.get("goal_not"), "goal_not is not used here"
    return {k: {v} for k, v in g["goal_all"].items()}


def room_graph(game) -> tuple:
    """(links {room: set of rooms}, counts) from the IR (module docstring)."""
    from ir import Const, Op
    links, counts = collections.defaultdict(set), collections.Counter()
    for k, u in game.units.items():
        if k[0] in ("entry", "exit", "local"):
            room = k[1]
        elif k[0] == "obj":
            room = game.object_room.get(k[1])
        else:
            continue
        if room is None:
            counts["object units with no room"] += 1
            continue
        for x in u.ins:
            st = x.st
            if not (isinstance(st, Op) and st.name in ("loadRoom", "loadRoomWithEgo")):
                continue
            arg = st.args[0] if st.name == "loadRoom" else st.args[1]
            if not isinstance(arg, Const):
                counts[f"{st.name} with a room that is not a constant (no link)"] += 1
                continue
            t = arg.value
            if t == 0:
                counts[f"{st.name} to room 0 (no link)"] += 1
                continue
            if t in game.pseudo:
                counts["room changes to a pseudo room (mapped to its resource)"] += 1
                t = game.pseudo[t]
            if t == room:
                counts["room changes to the room itself (no link)"] += 1
                continue
            counts["room changes that are links"] += 1
            links[room].add(t)
    return links, counts


def scopes(game, start_room: int) -> dict:
    """{k: sorted rooms within k links of the start room}, for every k up to the last ring."""
    links, _ = room_graph(game)
    dist = {start_room: 0}
    ring = [start_room]
    while ring:
        nxt = []
        for r in ring:
            for t in sorted(links[r]):
                if t not in dist:
                    dist[t] = dist[r] + 1
                    nxt.append(t)
        ring = nxt
    return {k: sorted(r for r, d in dist.items() if d <= k) for k in range(max(dist.values()) + 1)}


def all_rooms(game) -> list:
    return sorted({k[1] for k in game.units if k[0] in ("entry", "exit", "local")} | set(game.object_room.values()))


def cmd_graph(a) -> None:
    import model as M
    import reader as R
    game = M.Game.load()
    start = start_facts()
    links, counts = room_graph(game)
    print(f"start: seed {SEED}, room {start['room']}")
    print("graph counts:", dict(counts))
    print(f"rooms with code or objects: {len(all_rooms(game))}; rooms with a link out: {len(links)}; "
          f"links: {sum(len(v) for v in links.values())}")
    sc = scopes(game, start["room"])
    for k, rooms in sc.items():
        s = R.Scope.of(game, rooms, start)
        kinds = collections.Counter(u[0] for u in s.units)
        print(f"k={k}: {len(rooms)} rooms {rooms}; units {dict(kinds)}")
    print("not reached:", sorted(set(all_rooms(game)) - set(sc[max(sc)])))


# --- one scope (the child) -----------------------------------------------------------------------------------------

def _status(path: Path, d: dict) -> None:
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(d, default=str))
    os.replace(tmp, path)


def _hwm_mb() -> float:
    for line in Path("/proc/self/status").read_text().splitlines():
        if line.startswith("VmHWM:"):
            return int(line.split()[1]) / 1024
    return float("nan")


def _reset_hwm() -> None:
    try:
        Path("/proc/self/clear_refs").write_text("5")
    except OSError:
        pass


def _cpu() -> tuple:
    s, c = resource.getrusage(resource.RUSAGE_SELF), resource.getrusage(resource.RUSAGE_CHILDREN)
    return s.ru_utime + s.ru_stime, c.ru_utime + c.ru_stime


def cmd_child(a) -> None:
    import domains as D
    import model as M
    import reader as R
    import relevance
    import task as T

    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    status_path = out / f"k{a.k}.status.json"
    res = {"k": a.k, "workers": a.workers, "pid": os.getpid(),
           "sources_sha1": {n: hashlib.sha1((HERE / n).read_bytes()).hexdigest()[:12] for n in SOURCES},
           "steps": {}, "progress": []}
    st = {"step": None, "t0": None, "res": res}

    def step(name):
        if st["step"] is not None:
            done = res["steps"][st["step"]]
            done["wall_s"] = round(time.monotonic() - st["t0"], 1)
            cs, cc = _cpu()
            done["cpu_self_s"] = round(cs - st["cpu"][0], 1)
            done["cpu_children_s"] = round(cc - st["cpu"][1], 1)
            done["parent_peak_rss_mb"] = round(_hwm_mb())
        if name is None:
            st["step"] = None
            _status(status_path, {"step": None, "res": res})
            return
        _reset_hwm()
        st.update(step=name, t0=time.monotonic(), cpu=_cpu())
        res["steps"][name] = {}
        _status(status_path, {"step": name, "step_started": time.time(), "res": res})

    def progress(what, jobs, n, secs):
        res["progress"].append({"what": what, "runs": jobs, "records": n, "s": round(secs, 1),
                                "t": round(time.time() - t_start)})
        _status(status_path, {"step": st["step"], "step_started": time.time() - (time.monotonic() - st["t0"]),
                              "res": res})
        print(f"[k={a.k}] {what}: {jobs} runs, {n} records, {secs:.0f} s", file=sys.stderr, flush=True)

    t_start = time.time()
    start = start_facts()
    gl = goal()
    step("setup")
    game = M.Game.load()
    rooms = scopes(game, start["room"])[a.k]
    doms = D.build(start, goals=gl)
    sc = R.Scope.of(game, rooms, start)
    kinds = collections.Counter(u[0] for u in sc.units)
    res.update(rooms=rooms, n_rooms=len(rooms), units=dict(kinds), n_units=len(sc.units),
               global_scripts=sorted(u[1] for u in sc.units if u[0] == "global"), n_global=kinds["global"],
               objects=len(sc.objects), sentence_verbs=sc.verbs, dialog_verbs=len(sc.dialog_verbs),
               walk_verb=sc.walk_verb, slot_facts=len(sc.universe),
               goal=sorted(gl), goal_static={k: sc.writes.has(k) for k in gl})
    rd = R.Reader(sc, doms, game)
    b = R.Builder(rd)
    b.progress = progress

    step("read")
    recs = b.read_all(workers=a.workers)
    b.progress = None
    ch = resource.getrusage(resource.RUSAGE_CHILDREN)
    res["largest_worker_peak_rss_mb"] = round(ch.ru_maxrss / 1024)  # Linux: the largest reaped child, in KB
    res["records"] = len(recs)
    res["records_by_kind"] = dict(sorted(collections.Counter(r.kind for r in recs).items()))
    res["reader_counts"] = {k: v for k, v in b.counts.items() if k.startswith(("pass ", "event rounds", "records"))}
    res["out_of_domain_keys"] = len(b.out_of_domain)
    gw = {}
    for k, vals in gl.items():
        on = [r for r in recs if k in r.effects]
        can = 0
        for r in on:
            e = r.effects[k]
            if (e[0] == "set" and e[1] in vals) or (e[0] == "guess" and set(e[1]) & vals) or \
                    (e[0] == "when" and e[2] in vals) or e[0] in ("copy", "add"):
                can += 1
        gw[k] = {"records_writing": len(on), "records_that_can_write_the_goal_value": can}
    res["goal_dynamic"] = gw

    step("build_task")
    task = R.build_task(b, recs, gl)
    res["task_actions"] = len(task.actions)
    res["task_facts"] = len(task.facts)
    res["task_folded"] = task.folded
    res["task_effects"] = sum(len(x.effects) for x in task.actions)
    del recs

    step("relevance")
    rel = relevance.relevant(task)
    res["relevance"] = dict(rel.counts)

    step("prune")
    small = R.prune(task, rel)
    res["pruned_actions"] = len(small.actions)
    res["pruned_facts"] = len(small.facts)
    res["pruned_effects"] = sum(len(x.effects) for x in small.actions)
    if not a.full_sas:
        del task

    step("write_sas")
    try:
        sas = T.write_sas(small)
        res["sas"] = dict(sas.counts)
    except (ValueError, T.OutOfDomain) as e:
        res["sas_error"] = f"{type(e).__name__}: {e}"[:2000]
        sas = None

    step("write_file")
    res["disk_free_before_write_bytes"] = shutil.disk_usage(out).free
    if sas is not None:
        path = out / f"k{a.k}.sas"
        try:
            state = sas.state(R.task_start(b, start, small))
            with open(path, "w") as f:
                f.write(sas.head)
                f.write(state)
                f.write(sas.tail)
            res["task_file_bytes"] = path.stat().st_size
        except (ValueError, T.OutOfDomain) as e:
            res["sas_error"] = f"{type(e).__name__}: {e}"[:2000]
        except OSError as e:  # a full disk: the size written so far is a lower bound
            res["file_error"] = f"{type(e).__name__}: {e}; {path.stat().st_size if path.exists() else 0} bytes written"
            res["disk_free_bytes"] = shutil.disk_usage(out).free
        finally:
            if not a.keep_file and path.exists():
                path.unlink()
    if a.full_sas:  # the task before relevance, for growth that does not depend on the goal (counts and size)
        del sas
        step("write_sas_full")
        try:
            full = T.write_sas(task)
            res["sas_full"] = dict(full.counts)
            step("write_file_full")
            path = out / f"k{a.k}.full.sas"
            try:
                state = full.state(R.task_start(b, start, task))
                with open(path, "w") as f:
                    f.write(full.head)
                    f.write(state)
                    f.write(full.tail)
                res["task_file_full_bytes"] = path.stat().st_size
            except OSError as e:
                res["file_full_error"] = (f"{type(e).__name__}: {e}; "
                                          f"{path.stat().st_size if path.exists() else 0} bytes written")
            finally:
                if path.exists():
                    path.unlink()
        except (ValueError, T.OutOfDomain) as e:
            res["sas_full_error"] = f"{type(e).__name__}: {e}"[:2000]
    step(None)
    res["finished"] = True
    res["wall_s"] = round(time.time() - t_start, 1)
    (out / f"k{a.k}.result.json").write_text(json.dumps(res, default=str))
    _status(status_path, {"step": None, "res": res})


# --- the supervisor ------------------------------------------------------------------------------------------------

def _meminfo() -> dict:
    d = {}
    for line in Path("/proc/meminfo").read_text().splitlines():
        k, v = line.split(":")
        d[k] = int(v.split()[0]) / (1024 * 1024)  # GB (GiB)
    return d


def _proc_mb(pid: int, field: str) -> float:
    try:
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith(field + ":"):
                return int(line.split()[1]) / 1024
    except OSError:
        pass
    return 0.0


def _children(pid: int) -> list:
    try:
        return [int(c) for c in Path(f"/proc/{pid}/task/{pid}/children").read_text().split()]
    except OSError:
        return []


def run_one(k: int, a) -> dict:
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    status_path = out / f"k{k}.status.json"
    for p in (status_path, out / f"k{k}.result.json"):
        if p.exists():
            p.unlink()
    cmd = [sys.executable, str(Path(__file__).resolve()), "child", "--k", str(k), "--workers", str(a.workers),
           "--out", str(out)] + (["--keep-file"] if a.keep_file else []) + (["--full-sas"] if a.full_sas else [])
    err = open(out / f"k{k}.stderr", "w")
    mem0 = _meminfo()
    base_used = mem0["MemTotal"] - mem0["MemAvailable"]
    t0 = time.time()
    proc = subprocess.Popen(cmd, stdout=err, stderr=err, start_new_session=True)
    peak_used, min_avail, peak_tree_rss, peak_worker_hwm, peak_parent_rss = base_used, mem0["MemAvailable"], 0.0, 0.0, 0.0
    stop = None
    last = {}
    while proc.poll() is None:
        time.sleep(2)
        m = _meminfo()
        used = m["MemTotal"] - m["MemAvailable"]
        peak_used, min_avail = max(peak_used, used), min(min_avail, m["MemAvailable"])
        kids = _children(proc.pid)
        parent_rss = _proc_mb(proc.pid, "VmRSS")
        peak_parent_rss = max(peak_parent_rss, parent_rss)
        peak_tree_rss = max(peak_tree_rss, parent_rss + sum(_proc_mb(c, "VmRSS") for c in kids))
        if kids:
            peak_worker_hwm = max(peak_worker_hwm, max(_proc_mb(c, "VmHWM") for c in kids))
        try:
            last = json.loads(status_path.read_text())
        except (OSError, ValueError):
            pass
        step_age = time.time() - last["step_started"] if last.get("step_started") else 0
        if used > a.max_used_gb:
            stop = f"memory in use {used:.1f} GB passed {a.max_used_gb} GB"
        elif m["MemAvailable"] < a.min_free_gb:
            stop = f"MemAvailable {m['MemAvailable']:.1f} GB fell under {a.min_free_gb} GB"
        elif step_age > a.step_minutes * 60:
            stop = f"step {last.get('step')} passed {a.step_minutes} minutes"
        if stop:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait()
            break
    err.close()
    res_path = out / f"k{k}.result.json"
    if res_path.exists():
        line = json.loads(res_path.read_text())
    else:
        line = dict(last.get("res") or {"k": k})
        line["finished"] = False
        line["stopped_at"] = last.get("step")
        line["stopped_why"] = stop or f"child exited with code {proc.returncode}"
        line["stderr_tail"] = (out / f"k{k}.stderr").read_text()[-3000:]
    line["supervisor"] = {"wall_s": round(time.time() - t0, 1), "exit_code": proc.returncode,
                          "mem_used_before_gb": round(base_used, 2), "peak_mem_used_gb": round(peak_used, 2),
                          "peak_mem_used_by_scope_gb": round(peak_used - base_used, 2),
                          "min_mem_available_gb": round(min_avail, 2),
                          "peak_tree_rss_gb": round(peak_tree_rss / 1024, 2),
                          "peak_parent_rss_gb": round(peak_parent_rss / 1024, 2),
                          "peak_worker_hwm_gb_sampled": round(peak_worker_hwm / 1024, 2),
                          "limits": {"max_used_gb": a.max_used_gb, "min_free_gb": a.min_free_gb,
                                     "step_minutes": a.step_minutes},
                          "host": os.uname().nodename, "cpus": os.cpu_count(),
                          "mem_total_gb": round(mem0["MemTotal"], 1)}
    return line


def cmd_run(a) -> None:
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    jsonl = out / "task-size-scaling.jsonl"
    for k in a.ks:
        print(f"scope k={k}: starting {time.strftime('%H:%M:%S')}", flush=True)
        line = run_one(k, a)
        with open(jsonl, "a") as f:
            f.write(json.dumps(line, default=str) + "\n")
        print(f"scope k={k}: finished={line.get('finished')} records={line.get('records')} "
              f"actions={line.get('task_actions')} ops={line.get('sas', {}).get('operators')} "
              f"file={line.get('task_file_bytes')} stopped={line.get('stopped_why')} "
              f"wall={line['supervisor']['wall_s']} s", flush=True)
        if not line.get("finished") and line.get("stopped_at") not in ("write_sas_full", "write_file_full"):
            print("stopped growing", flush=True)
            break


# --- the report ----------------------------------------------------------------------------------------------------

def _fit(xs, ys) -> dict | None:
    """Least squares of log y on log x: y = c * x^p, and of y on x: y = a + b x."""
    pts = [(x, y) for x, y in zip(xs, ys) if x and y and x > 0 and y > 0]
    if len(pts) < 2:
        return None
    lx, ly = [math.log(x) for x, _ in pts], [math.log(y) for _, y in pts]
    n = len(pts)
    mx, my = sum(lx) / n, sum(ly) / n
    sxx = sum((x - mx) ** 2 for x in lx)
    p = sum((x - mx) * (y - my) for x, y in zip(lx, ly)) / sxx if sxx else float("nan")
    c = math.exp(my - p * mx)
    xs2, ys2 = [x for x, _ in pts], [y for _, y in pts]
    mx2, my2 = sum(xs2) / n, sum(ys2) / n
    sxx2 = sum((x - mx2) ** 2 for x in xs2)
    b = sum((x - mx2) * (y - my2) for x, y in zip(xs2, ys2)) / sxx2 if sxx2 else float("nan")
    return {"power": p, "coef": c, "slope": b, "intercept": my2 - b * mx2, "points": n}


FULL_GAME = {"rooms": 83, "units": 1172}  # reader.Scope.of over every room with code or objects, from seed 1's start


def _g(r: dict, path: str):
    for part in path.split("."):
        r = r.get(part) if isinstance(r, dict) else None
    return r


COLUMNS = [  # (header, path in the line, scale)
    ("rooms", "n_rooms", 1), ("globals", "n_global", 1), ("units", "n_units", 1), ("objects", "objects", 1),
    ("records", "records", 1), ("actions", "task_actions", 1), ("kept", "relevance.actions kept", 1),
    ("ops", "sas.operators", 1), ("effects", "sas.effects", 1), ("MB", "task_file_bytes", 1e-6),
    ("ops full", "sas_full.operators", 1), ("effects full", "sas_full.effects", 1),
    ("MB full", "task_file_full_bytes", 1e-6),
    ("read s", "steps.read.wall_s", 1), ("read cpu s", "read_cpu", 1), ("build s", "steps.build_task.wall_s", 1),
    ("sas s", "steps.write_sas.wall_s", 1), ("parent GB", "parent_peak", 1e-3),
    ("worker GB", "largest_worker_peak_rss_mb", 1e-3), ("mem GB", "supervisor.peak_mem_used_by_scope_gb", 1),
]


def _derive(r: dict) -> dict:
    rd = _g(r, "steps.read") or {}
    r["read_cpu"] = (rd.get("cpu_self_s") or 0) + (rd.get("cpu_children_s") or 0) if rd.get("wall_s") else None
    r["parent_peak"] = max((v.get("parent_peak_rss_mb") or 0) for v in r.get("steps", {}).values()) \
        if r.get("steps") else None
    return r


def _fmt(v, scale) -> str:
    if v is None:
        return "-"
    v = v * scale
    if scale != 1 or isinstance(v, float):
        return f"{v:.1f}" if abs(v) < 1000 else f"{v:.0f}"
    return str(v)


def cmd_report(a) -> None:
    rows = [_derive(json.loads(line)) for line in Path(a.jsonl).read_text().splitlines() if line.strip()]
    L = []
    w = L.append
    w("E003 step 4, a measurement: how the reading and the task grow with the scope, from Part I's real start")
    w("")
    w("script: experiments/E003-path-through-cloud/size_scaling.py (new; reader.py, task.py, relevance.py and domains.py "
      "as they are)")
    shas = rows[0].get("sources_sha1", {})
    w("sources (sha1, 12 hex): " + ", ".join(f"{k} {v}" for k, v in shas.items()))
    w("")
    w(a.method.read_text().rstrip() if a.method else "")
    w("")
    w("== table (one row per scope; '-' = not reached) ==")
    heads = ["k"] + [h for h, _, _ in COLUMNS]
    table = [[str(r["k"])] + [_fmt(_g(r, path), sc) for _, path, sc in COLUMNS] for r in rows]
    widths = [max(len(x) for x in col) for col in zip(heads, *table)]
    w("  ".join(h.rjust(n) for h, n in zip(heads, widths)))
    for t in table:
        w("  ".join(x.rjust(n) for x, n in zip(t, widths)))
    w("")
    w("columns: rooms, global scripts and units in the scope (reader.Scope.of); objects the rooms define; records "
      "(paths read); actions (build_task); kept (relevance's kept actions); ops, effects, MB: the task file after "
      "relevance and prune (what the planner would get); ops full, effects full, MB full: the same task file before "
      "relevance (growth that does not depend on the goal); read s and read cpu s: read_all's wall and CPU (parent "
      "and workers); build s: build_task; sas s: write_sas of the pruned task; parent GB: the reading process's peak "
      "RSS over its steps (VmHWM); worker GB: the largest pool worker's peak RSS (RUSAGE_CHILDREN, copy-on-write "
      "pages from the parent included); mem GB: the machine's memory in use at its peak, minus before the scope "
      "(MemTotal - MemAvailable, sampled every 2 s)")
    w("")
    w("== per scope ==")
    for r in rows:
        w(f"k={r['k']}: {r.get('n_rooms')} rooms {r.get('rooms')}")
        w(f"  global scripts ({r.get('n_global')}): {r.get('global_scripts')}")
        w(f"  units {r.get('units')}; slot facts {r.get('slot_facts')}; dialog verbs {r.get('dialog_verbs')}")
        w(f"  records by kind: {r.get('records_by_kind')}; reader: {r.get('reader_counts')}; "
          f"keys written outside their domain: {r.get('out_of_domain_keys')}")
        w(f"  goal writable, static (scope_writes.has): {r.get('goal_static')}")
        w(f"  goal writable, in the records: {r.get('goal_dynamic')}")
        w(f"  task: {r.get('task_actions')} actions, {r.get('task_facts')} facts, {r.get('task_effects')} effects, "
          f"{r.get('task_folded')} records folded")
        w(f"  relevance: {r.get('relevance')}")
        w(f"  pruned: {r.get('pruned_actions')} actions, {r.get('pruned_facts')} facts, {r.get('pruned_effects')} "
          f"effects; file {r.get('task_file_bytes')} bytes")
        w(f"  task file (pruned): {r.get('sas')}")
        w(f"  task file (before relevance): {r.get('sas_full')}; {r.get('task_file_full_bytes')} bytes")
        for key in ("sas_error", "sas_full_error", "file_error", "file_full_error"):
            if r.get(key):
                w(f"  {key}: {r[key]}")
        w("  steps (wall s, CPU s of the process and of its workers, peak RSS MB of the process): " +
          "; ".join(f"{n} {v.get('wall_s')}/{v.get('cpu_self_s')}/{v.get('cpu_children_s')}/"
                    f"{v.get('parent_peak_rss_mb')}" for n, v in r.get("steps", {}).items()))
        w(f"  reading rounds: " + "; ".join(f"{p['what']} {p['runs']} runs {p['records']} records {p['s']} s"
                                            for p in r.get("progress", [])))
        sup = r.get("supervisor", {})
        w(f"  machine: {sup.get('host')}, {sup.get('cpus')} CPUs, {sup.get('mem_total_gb')} GB; workers "
          f"{r.get('workers')}; wall {sup.get('wall_s')} s; memory in use before {sup.get('mem_used_before_gb')} GB, "
          f"peak {sup.get('peak_mem_used_gb')} GB; peak tree RSS {sup.get('peak_tree_rss_gb')} GB (sum over "
          f"processes, shared pages counted once per process); sampled worker peak "
          f"{sup.get('peak_worker_hwm_gb_sampled')} GB")
        if not r.get("finished"):
            w(f"  STOPPED at step {r.get('stopped_at')}: {r.get('stopped_why')}")
        w("")
    done = [r for r in rows if r.get("finished") or r.get("records")]
    w("== growth from scope to scope ==")
    for r0, r1 in zip(done, done[1:]):
        dr, du = (r1["n_rooms"] - r0["n_rooms"]), (r1["n_units"] - r0["n_units"])
        parts = []
        for name, path in (("records", "records"), ("actions", "task_actions"), ("ops full", "sas_full.operators"),
                           ("MB full", "task_file_full_bytes"), ("ops", "sas.operators"), ("read cpu s", "read_cpu")):
            v0, v1 = _g(r0, path), _g(r1, path)
            if v0 and v1:
                parts.append(f"{name} x{v1 / v0:.1f} (+{(v1 - v0) / dr:.4g} per added room, "
                             f"+{(v1 - v0) / du:.4g} per added unit)")
        w(f"k={r0['k']} -> k={r1['k']} (+{dr} rooms, +{du} units): " + "; ".join(parts))
    w("")
    w("== fits: y = c * x^p by least squares on logs, x = units in the scope and x = rooms; extrapolated to the "
      f"full game ({FULL_GAME['units']} units, {FULL_GAME['rooms']} rooms) ==")
    for name, path in (("records", "records"), ("actions", "task_actions"), ("ops full", "sas_full.operators"),
                       ("effects full", "sas_full.effects"), ("MB full", "task_file_full_bytes"),
                       ("read cpu s", "read_cpu"), ("parent peak MB", "parent_peak")):
        for xname in ("units", "rooms"):
            pts = [(r[f"n_{xname}"], _g(r, path)) for r in done if _g(r, path)]
            f = _fit(*zip(*pts)) if len(pts) >= 2 else None
            if not f:
                continue
            ext = f["coef"] * FULL_GAME[xname] ** f["power"]
            ext_lin = f["intercept"] + f["slope"] * FULL_GAME[xname]
            scale = 1e-6 if path == "task_file_full_bytes" else 1
            w(f"  {name} on {xname}: p = {f['power']:.2f} over {f['points']} points "
              f"(x {', '.join(str(x) for x, _ in pts)}); full game {ext * scale:.3g} (power law), "
              f"{ext_lin * scale:.3g} (straight line)")
    w("")
    if a.notes:
        w(a.notes.read_text().rstrip())
    Path(a.log).write_text("\n".join(L) + "\n")
    print("\n".join(L))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("graph").set_defaults(fn=cmd_graph)
    p = sub.add_parser("child")
    p.add_argument("--k", type=int, required=True)
    p.add_argument("--workers", type=int, default=16)
    p.add_argument("--out", required=True)
    p.add_argument("--keep-file", action="store_true")
    p.add_argument("--full-sas", action="store_true")
    p.set_defaults(fn=cmd_child)
    p = sub.add_parser("run")
    p.add_argument("--ks", type=int, nargs="+", default=[0, 1, 2, 3])
    p.add_argument("--workers", type=int, default=16)
    p.add_argument("--out", required=True)
    p.add_argument("--max-used-gb", type=float, default=100.0)
    p.add_argument("--min-free-gb", type=float, default=0.0)
    p.add_argument("--step-minutes", type=float, default=60.0)
    p.add_argument("--keep-file", action="store_true")
    p.add_argument("--full-sas", action="store_true")
    p.set_defaults(fn=cmd_run)
    p = sub.add_parser("report")
    p.add_argument("--jsonl", required=True)
    p.add_argument("--log", required=True)
    p.add_argument("--method", type=Path, help="a text file with the method section")
    p.add_argument("--notes", type=Path, help="a text file with the findings, appended")
    p.set_defaults(fn=cmd_report)
    a = ap.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
