"""E003: replanning without reloading, checked on recorded replans (taskcore/README.md, "Replanning without reloading").

usage: serve_check.py compare NAME PACKED OUT_DIR REQS.json --old MAIN_BIN [--new BIN] [--skip-old] [--skip-new-sas]
                      [--args TASKCORE_OPTION ...]
       serve_check.py starts PACKED OUT_DIR [OVERLAY.txt ...]
       serve_check.py log OUT_DIR/summary.json ...

``compare``: per request of REQS.json (``[{"start": a start file, "bans": a bans file or null, "check": "needed" or
"input"}, ...]``, a replay run's replans), main's ``taskcore sas`` (before, ``--old``), the new binary's ``sas``, and
one ``taskcore serve`` process that takes every request in turn (after). Each writes the SAS file, the operator table,
the kept list and the stats; the check is that they are the same bytes (the stats but for times, memory and paths),
or that both stop with the same message. Per request: wall seconds, CPU seconds and peak memory of each (a ``sas``
process's from wait4; the server's from its reply, which resets its peak before each request). ``--args`` go to every
binary (``--goal`` needs the new one: ``--skip-old``). It pauses while the machine has under 3.5 GB available.
Writes OUT_DIR/summary.json.

``starts``: start files for a packed file: its own start (start-0.txt), and that start with the facts of each
OVERLAY start file that the packed file has and its domains hold (start-1.txt, ...): other states to replan from.

``log``: the summaries as the table in results/serve-check.log.
"""
import argparse
import struct
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

VOLATILE = ("seconds", "rss", "peak_rss")  # key prefixes: times and memory
PATHS = ("task", "start")  # keys: paths


def strip(x):
    if isinstance(x, dict):
        return {k: strip(v) for k, v in x.items() if not k.startswith(VOLATILE) and k not in PATHS}
    return x


def sha(p: Path) -> str:
    return hashlib.sha1(p.read_bytes()).hexdigest()[:16] if p.exists() else "missing"


def sas_cmd(binary, packed, req, out: Path, extra):
    cmd = [binary, "sas", "--task", packed, "--start-file", req["start"], "--out", out / "task.sas", "--stats",
           out / "stats.json", "--ops", out / "ops.jsonl", "--kept", out / "kept.json", "--start-check",
           req.get("check", "needed")] + list(extra)
    if req.get("bans"):
        cmd += ["--bans", req["bans"]]
    return [str(c) for c in cmd]


def mem_guard(gb=3.5):
    """Pause while the machine's available memory is under ``gb`` (the shared laptop's rule: 3 GB, plus a margin)."""
    while True:
        avail = int(next(l for l in open("/proc/meminfo") if l.startswith("MemAvailable")).split()[1]) / 2**20
        if avail >= gb:
            return
        print(f"memory guard: {avail:.1f} GB available, waiting", file=sys.stderr, flush=True)
        time.sleep(20)


def run_sas(binary, packed, req, out: Path, extra):
    mem_guard()
    out.mkdir(parents=True, exist_ok=True)
    for f in out.iterdir():
        f.unlink()
    t = time.monotonic()
    p = subprocess.Popen(sas_cmd(binary, packed, req, out, extra), stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                         text=True)
    _, status, ru = os.wait4(p.pid, 0)
    secs = time.monotonic() - t
    code = os.waitstatus_to_exitcode(status)
    stderr = p.stderr.read()
    p.returncode = code
    err = None if code == 0 else stderr.strip().replace("taskcore: ", "", 1)
    return {"seconds": round(secs, 3), "cpu_seconds": round(ru.ru_utime + ru.ru_stime, 3),
            "max_rss_mb": round(ru.ru_maxrss / 1024, 1), "error": err}


class Server:
    def __init__(self, binary, packed, extra):
        cmd = [str(binary), "serve", "--task", str(packed)] + [str(a) for a in extra]
        self.t0 = time.monotonic()
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        line = self.p.stdout.readline()
        assert line.startswith("ready\t"), line
        self.hello = json.loads(line.split("\t", 1)[1])
        self.ready_s = time.monotonic() - self.t0

    def request(self, req, out: Path):
        out.mkdir(parents=True, exist_ok=True)
        for f in out.iterdir():
            f.unlink()
        mem_guard()
        args = sas_cmd("x", "x", req, out, ())[4:]  # drop binary, sas, --task, packed
        t = time.monotonic()
        self.p.stdin.write("\t".join(args) + "\n")
        self.p.stdin.flush()
        line = self.p.stdout.readline().rstrip("\n")
        secs = time.monotonic() - t
        kind, rest = line.split("\t", 1)
        if kind == "ok":
            d = json.loads(rest)
            return {"seconds": round(secs, 3), "reply": d, "error": None}
        return {"seconds": round(secs, 3), "error": rest}

    def close(self):
        self.p.stdin.close()
        _, status, ru = os.wait4(self.p.pid, 0)
        self.p.returncode = os.waitstatus_to_exitcode(status)
        return round(ru.ru_maxrss / 1024, 1)


def same(a: Path, b: Path):
    out = {}
    for f in ("task.sas", "ops.jsonl", "kept.json"):
        out[f] = (a / f).exists() and (b / f).exists() and (a / f).read_bytes() == (b / f).read_bytes()
    sa, sb = a / "stats.json", b / "stats.json"
    if sa.exists() and sb.exists():
        out["stats"] = strip(json.loads(sa.read_text())) == strip(json.loads(sb.read_text()))
    return out


def tables(path):
    data = Path(path).read_bytes()
    w = lambda i: struct.unpack_from("<I", data, 4 * i)[0]  # noqa: E731
    n = len(data) // 4
    body, nact, o_str, o_facts, o_adds, o_goal, o_starts, _ = struct.unpack_from("<8I", data, len(data) - 32)
    nstr = w(o_str)
    lens = [w(o_str + 1 + i) for i in range(nstr)]
    p = (o_str + 1 + nstr) * 4
    strs = []
    for l in lens:
        strs.append(data[p:p + l].decode())
        p += l
    q = o_facts
    nf = w(q); q += 1
    facts = []
    for _ in range(nf):
        key, size = w(q), w(q + 1); q += 2
        vals = [strs[w(q + j)] for j in range(size)]; q += size
        facts.append((strs[key], vals))
    q = o_starts
    ns = w(q); q += 1
    starts = []
    for _ in range(ns):
        name = strs[w(q)]; q += 1
        vals = []
        for f in range(nf):
            x = w(q + f)
            vals.append(None if x == 0xFFFFFFFF else strs[x])
        q += nf
        starts.append((name, vals))
    return facts, starts



def starts(argv):
    packed, out = argv[0], Path(argv[1])
    out.mkdir(parents=True, exist_ok=True)
    facts, starts = tables(packed)
    base = {k: v for (k, _), v in zip(facts, starts[0][1])}
    dom = {k: set(vs) for k, vs in facts}
    (out / "start-0.txt").write_text("".join(f"{k}\t{'' if v is None else v}\n" for k, v in base.items()))
    for i, ov in enumerate(argv[2:], 1):
        st = dict(base)
        moved = 0
        for line in Path(ov).read_text().splitlines():
            k, v = line.split("\t")
            if k in st and v in dom[k] and st[k] != v:
                st[k] = v
                moved += 1
        (out / f"start-{i}.txt").write_text("".join(f"{k}\t{'' if v is None else v}\n" for k, v in st.items()))
        print(f"start-{i}: {moved} facts from {ov}")



def compare(argv):
    ap = argparse.ArgumentParser(prog="serve_check.py compare")
    ap.add_argument("name")
    ap.add_argument("packed")
    ap.add_argument("out", type=Path)
    ap.add_argument("reqs", type=Path)
    ap.add_argument("--old", required=False, default=None, help="main's taskcore (before)")
    ap.add_argument("--new", default=str(Path(__file__).resolve().parent / "taskcore" / "target" / "release" / "taskcore"))
    ap.add_argument("--skip-old", action="store_true")
    ap.add_argument("--skip-new-sas", action="store_true")
    ap.add_argument("--args", nargs=argparse.REMAINDER, default=[])
    a = ap.parse_args(argv)
    if not a.skip_old and not a.old:
        ap.error("--old MAIN_BIN (main's taskcore), or --skip-old")
    reqs = json.loads(a.reqs.read_text())
    rows = []
    mem_guard()
    srv = Server(a.new, a.packed, a.args)
    print(f"{a.name}: server ready in {srv.ready_s:.1f} s, {srv.hello}", flush=True)
    for i, req in enumerate(reqs):
        row = {"i": i, "req": req}
        d_srv = a.out / f"r{i:02d}" / "serve"
        row["serve"] = srv.request(req, d_srv)
        if not a.skip_new_sas:
            d_new = a.out / f"r{i:02d}" / "new"
            row["new"] = run_sas(a.new, a.packed, req, d_new, a.args)
            row["new_vs_serve"] = same(d_new, d_srv)
        if not a.skip_old:
            d_old = a.out / f"r{i:02d}" / "old"
            row["old"] = run_sas(a.old, a.packed, req, d_old, a.args)
            row["old_vs_serve"] = same(d_old, d_srv)
            if not a.skip_new_sas:
                row["old_vs_new"] = same(d_old, d_new)
            row["errors_same"] = row["old"]["error"] == row["serve"]["error"]
        row["sas_sha"] = sha(d_srv / "task.sas")
        print(json.dumps({k: row[k] for k in row if k != "req"}), flush=True)
        rows.append(row)
    rss_end = srv.close()
    summary = {"name": a.name, "packed": a.packed, "args": a.args, "server_ready_s": round(srv.ready_s, 2),
               "hello": srv.hello, "server_max_rss_mb": rss_end, "rows": rows}
    (a.out / "summary.json").write_text(json.dumps(summary, indent=1) + "\n")



def log(paths):
    out = []
    for p in paths:
        d = json.loads(Path(p).read_text())
        out.append(f"== {d['name']}: {d['packed']} {' '.join(d['args'])}")
        out.append(f"server: load {d['hello']['seconds_load']:.1f} s, {d['hello']['rss'] / 2**20:.0f} MB resident after "
                   f"the load, {d['server_max_rss_mb']:.0f} MB peak over the session")
        out.append("req  bans  kept   serve s / cpu s / peak MB   sas(new) s / cpu s / MB   sas(main) s / cpu s / MB   "
                   "same bytes")
        for r in d["rows"]:
            s, rp = r["serve"], r["serve"].get("reply") or {}
            sv = (f"{s['seconds']:7.2f} {rp.get('cpu_seconds', float('nan')):7.2f} {rp.get('peak_rss', 0) / 2**20:6.0f}"
                  if not s["error"] else f"error: {s['error'][:40]}")
            nw = r.get("new")
            nw = f"{nw['seconds']:7.2f} {nw['cpu_seconds']:7.2f} {nw['max_rss_mb']:6.0f}" if nw else "       -"
            od = r.get("old")
            od = f"{od['seconds']:7.2f} {od['cpu_seconds']:7.2f} {od['max_rss_mb']:6.0f}" if od else "       -"
            checks = [all(r[k].values()) for k in ("old_vs_serve", "new_vs_serve", "old_vs_new") if k in r]
            same = "yes" if all(checks) and (r.get("errors_same", True)) else "NO"
            out.append(f"{r['i']:3d}  {'yes' if r['req'].get('bans') else 'no':4s}  {rp.get('kept', '-'):5s}  {sv}   {nw}   "
                       f"{od}   {same}")
        out.append("")
    print("\n".join(out))


def main():
    cmd, argv = sys.argv[1], sys.argv[2:]
    if cmd == "compare":
        compare(argv)
    elif cmd == "starts":
        starts(argv)
    elif cmd == "log":
        log(argv)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
