"""E003, D7: check.py's model check, with the Rust machine (scumm/machine) in place of model.py's Machine.

usage (from this folder, under nice):
       ../../.venv/bin/python check_rust.py --out DIR [check.py's own options: --features ... --switches ... --jobs N]
                                            [--verify] [--canonical] [--no-build] [--bin PATH]

It runs check.py's own ``main``, unchanged, with three of its module globals replaced in this process (and so in the
workers it forks):

* ``check.run_model``, the machine's entry point for one move: it sends the action and the oracle's fact map and
  families to ``machine-serve`` (scumm/machine/src/bin/machine_serve.rs; one Rust process per scoring process, started
  on first use) and decodes the Run it gets back. The Rust sends golden.py's ``run`` line and one ``path`` line per
  path (FORMAT.txt), with one change for speed: a list of plain tuples goes by columns (``$tuples``; a column of
  strings as a table with each string once and the indices in base64, ``$strs``), and the decoding builds it with
  zip. The decoded ``model.Run`` has every field,
  and each ``model.Path`` every field but the four FORMAT.txt leaves out (``seen``, ``round_slots``, ``joined``,
  ``history``), which keep their defaults: check.py reads none of them (FORMAT.txt's CHECK_READS lists what it
  reads). Dicts come back in key order, as golden.py writes them, except ``Path.slots``, which keeps the machine's
  order. ``model.predict`` then runs in Python on the decoded Run, as check.py calls it.
* ``check.OUT`` (results/) becomes DIR, so every file check.py writes goes there and results/ is never touched:
  check-part1[-rowN[-switches]].jsonl and .log, growth.md (check.py replaces its row in DIR/growth.md: copy
  results/growth.md into DIR first to get the full table), and with ``--jobs`` above 1 the .times.tsv.
* ``check.score_move`` is wrapped, only to time each move.

Everything else is check.py's own code: the moves and their actions, usability, the scoring, the predictions, the
summary and the files. So the files match results/ byte for byte (the log's first line, which holds the run's time,
apart) when the Rust machine matches model.py.

``--verify``: the server also sends golden.py's canonical lines, and every decoded Run is encoded back with golden.py
and compared with them; it stops at the first difference (slower; the decoding's own check). ``--canonical``: the
server sends the canonical lines instead of the columns (slower; for comparison). ``--no-build``: skip
``cargo build --release -p scumm-machine --bin machine-serve`` (run in scumm/ otherwise; quick when nothing changed).
``--bin PATH``: another machine-serve binary (the tests pass cargo's own), not built here. ``--resume`` is refused: check.py's resume folder keys records by the
Python sources only, so it would mix records of both machines.

Besides check.py's files it writes DIR/<stem>.rust-times.tsv (stem: ``check.times_stem``): per counted move with an
action, the Rust machine's seconds, the encoding's, the bytes sent, the decoding's seconds, the whole move's
(``score_move``'s) wall and CPU seconds in Python, the paths, and the peak RSS of the Python process and of its Rust
server so far; and DIR/<stem>.rust-summary.json: the run's wall time, the CPU of every process of the run and of the
Rust servers among them, the peak RSS of each, and the slowest moves. One line on stderr says the same.
"""

import argparse
import array
import base64
import gc
import hashlib
import json
import multiprocessing.util
import os
import resource
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCUMM = HERE / "scumm"
BIN = SCUMM / "target" / "release" / "machine-serve"

# the Python sources the Rust machine reproduces byte for byte (scumm/machine/PORT-NOTES.md)
FROZEN = {"model.py": "cc630446bdbe", "ir.py": "c6ccf01059f1", "facts.py": "c06fc3489018", "actions.py": "ddbd1aaa2207",
          "check.py": "fc1c3a935c4e", "reader.py": "94d495853f07"}


def frozen() -> None:
    got = {n: hashlib.sha1((HERE / n).read_bytes()).hexdigest()[:12] for n in FROZEN}
    bad = {n: h for n, h in got.items() if h != FROZEN[n]}
    if bad:
        raise SystemExit(f"check_rust: sources changed since the Rust machine was checked against them: {bad} "
                         f"(expected {FROZEN}); stopping")


frozen()  # before check.py and model.py are imported

import check  # noqa: E402  (imports parallel first, which hashes the sources as imported)
import golden  # noqa: E402
import groundtruth  # noqa: E402
import model  # noqa: E402

_ARGS = {"verify": False, "canonical": False, "dir": None, "bin": BIN}
_WIDTH = {2: "H", 4: "I"}  # the array type codes of the wire's index widths
assert all(array.array(c).itemsize == w for w, c in _WIDTH.items())
_SERVERS = {}  # os.getpid() -> Server: a forked worker starts its own
_CUR = {}  # the move being scored in this process: its label and the Rust part's numbers


def _hook(d: dict):
    """json.loads' object_hook: golden.py's canonical encoding back into Python values (FORMAT.txt), and the wire's
    lists of tuples by columns (machine_serve.rs ``compact``). The innermost objects come first, so the values inside
    a $tuple, $set, $map, $tuples or dataclass are decoded already."""
    if len(d) == 1:
        k = next(iter(d))
        if k[:1] == "$":
            v = d[k]
            if k == "$tuples":  # a list of tuples, sent by columns
                return list(zip(*v))
            if k == "$strs":  # a column of strings: [table, width, base64 of the indices as little-endian u16 or u32]
                a = array.array(_WIDTH[v[1]], base64.b64decode(v[2]))
                if sys.byteorder == "big":
                    a.byteswap()
                return list(map(v[0].__getitem__, a))
            if k == "$tuple":
                return tuple(v)
            if k == "$set":
                return set(v)
            if k == "$map":
                return {kk: vv for kk, vv in v}
            if k == "$Unknown":
                vals = v["values"]
                return model.Unknown(v["reason"], v["what"], None if vals is None else frozenset(vals))
            if k == "$Frame":
                return model.Frame(**v)
            if k == "$Call":
                return model.Call(**v)
            raise SystemExit(f"check_rust: no decoding for {k}")
    return d


def decode(body: bytes, canon: bytes = b"") -> model.Run:
    """The Run from the Rust's lines: one ``run`` line, then one ``path`` line per path in ``run.paths`` order.
    ``canon``: the same lines in golden.py's canonical form, when ``--verify`` asked the server for them. The cyclic
    garbage collector is off while it builds: the millions of new tuples would start it over and over, and the
    decoded objects hold no cycles."""
    was = gc.isenabled()
    gc.disable()
    try:
        return _decode(body, canon)
    finally:
        if was:
            gc.enable()


def _decode(body: bytes, canon: bytes) -> model.Run:
    texts = body.split(b"\n")
    if texts[-1] != b"":
        raise SystemExit("check_rust: the Rust's lines do not end in a newline")
    texts.pop()
    head = json.loads(texts[0], object_hook=_hook)
    if head["kind"] != "run":
        raise SystemExit(f"check_rust: the first line is {head['kind']}, not run")
    rd = head["run"]
    paths = []
    for k, t in enumerate(texts[1:]):
        d = json.loads(t, object_hook=_hook)
        if d["kind"] != "path" or d["index"] != k:
            raise SystemExit(f"check_rust: line {k + 1} is not path {k}")
        f = d["path"]
        f.pop("history_len")
        f.pop("tid")
        slots = f.pop("slots")
        p = model.Path(**f)
        p.slots = {n: fr for n, fr in slots}
        paths.append(p)
    if len(paths) != rd["n_paths"]:
        raise SystemExit(f"check_rust: {len(paths)} path lines, n_paths {rd['n_paths']}")
    run = model.Run(paths, rd["start"], limit=rd["limit"], unit=rd["unit"], merges=rd["merges"],
                    value_caps=rd["value_caps"], settle_merges=rd["settle_merges"], pre_slots=rd["pre_slots"],
                    value_merges=rd["value_merges"], key_routes=rd["key_routes"])
    if _ARGS["verify"]:
        verify(run, canon.split(b"\n")[:-1])
    return run


def verify(run: model.Run, texts: list) -> None:
    """golden.py's encoding of the decoded Run gives back the Rust's canonical lines, byte for byte."""
    if len(texts) != len(run.paths) + 1:
        raise SystemExit(f"check_rust: {len(texts)} canonical lines for {len(run.paths)} paths")
    if golden.line({"kind": "run", "run": golden.run_dict(run)}).encode() != texts[0]:
        raise SystemExit("check_rust: the run line does not decode back")
    for k, p in enumerate(run.paths):
        sent = json.loads(texts[k + 1])["path"]
        back = golden.path_dict(p)
        back["history_len"], back["tid"] = sent["history_len"], sent["tid"]
        if golden.line({"kind": "path", "index": k, "path": back}).encode() != texts[k + 1]:
            raise SystemExit(f"check_rust: path {k} does not decode back to its line")


class Server:
    """One machine-serve process, for one feature set and switch set."""

    def __init__(self, features: frozenset, switches: frozenset, machine: model.Machine):
        cmd = [str(_ARGS["bin"]), "--features", ",".join(sorted(features))]
        if switches:
            cmd += ["--switches", ",".join(sorted(switches))]
        self.key = (features, switches)
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        hello = json.loads(self.p.stdout.readline() or b"null")
        want = {"features": sorted(features), "switches": sorted(switches), "path_limit": machine.path_limit,
                "step_limit": machine.step_limit, "round_limit": machine.round_limit}
        if hello != want:
            raise SystemExit(f"check_rust: machine-serve is not this Machine: {hello} != {want}")
        # a forked worker exits through multiprocessing's finalizers, not atexit: close and wait for the server there,
        # so its CPU time reaches the parent's RUSAGE_CHILDREN through the worker
        multiprocessing.util.Finalize(None, self.close, exitpriority=0)

    def close(self) -> None:
        if self.p.returncode is None:
            self.p.stdin.close()
            self.p.wait()

    def run(self, action, oracle) -> tuple[dict, bytes, bytes]:
        req = {"action": {"kind": action.kind, "verb": action.verb, "a": action.a, "b": action.b, "walk": action.walk},
               "pre": oracle.facts, "kinds": None if oracle.kinds is None else sorted(oracle.kinds),
               "canonical": _ARGS["canonical"], "verify": _ARGS["verify"]}
        self.p.stdin.write(json.dumps(req, separators=(",", ":")).encode() + b"\n")
        self.p.stdin.flush()
        line = self.p.stdout.readline()
        if not line:
            raise SystemExit(f"check_rust: machine-serve stopped (exit {self.p.wait()})")
        head = json.loads(line)
        body = self.p.stdout.read(head["bytes"])
        canon = self.p.stdout.read(head["canonical_bytes"])
        if len(body) != head["bytes"] or len(canon) != head["canonical_bytes"]:
            raise SystemExit("check_rust: machine-serve's answer is cut short")
        return head, body, canon


def run_model(machine: model.Machine, action, oracle) -> model.Run:
    """check.py's ``run_model`` (the same arguments, the same Run), on the Rust machine."""
    if type(oracle) is not model.FactOracle:
        raise SystemExit(f"check_rust: the Rust machine takes a FactOracle, not {type(oracle).__name__}")
    srv = _SERVERS.get(os.getpid())
    if srv is None or srv.key != (machine.features, machine.switches):
        srv = _SERVERS[os.getpid()] = Server(machine.features, machine.switches, machine)
    head, body, canon = srv.run(action, oracle)
    t = time.perf_counter()
    run = decode(body, canon)
    _CUR.update(rust=head, decode_s=time.perf_counter() - t, server=srv.p.pid)
    return run


_real_score_move = check.score_move


def score_move(row, action, machine, nonfacts, kinds) -> dict:
    """check.py's ``score_move``, timed; the numbers go to a file of this process in DIR."""
    _CUR.clear()
    t, c = time.perf_counter(), time.process_time()
    out = _real_score_move(row, action, machine, nonfacts, kinds)
    wall, cpu = time.perf_counter() - t, time.process_time() - c
    if "rust" in _CUR:
        h = _CUR["rust"]
        rec = {"label": check.label(row), "kind": check.kind_name(action), "machine_s": h["machine_s"],
               "encode_s": h["encode_s"], "bytes": h["bytes"], "decode_s": _CUR["decode_s"], "move_wall_s": wall,
               "move_cpu_s": cpu, "paths": h["paths"], "py_rss_mb": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
               / 1024, "rust_rss_mb": h["peak_rss_kb"] / 1024, "server": _CUR["server"], "rust_cpu_s": h["cpu_s"]}
        with open(_ARGS["dir"] / f".rust-times.{os.getpid()}.jsonl", "a") as fh:
            fh.write(json.dumps(rec) + "\n")
    return out


def summary(folder: Path, stem: str, wall: float, order: list) -> None:
    """Gathers the processes' files into DIR/<stem>.rust-times.tsv (move order) and DIR/<stem>.rust-summary.json."""
    recs = {}
    for f in sorted(folder.glob(".rust-times.*.jsonl")):
        for line in f.read_text().splitlines():
            r = json.loads(line)
            recs[r["label"]] = r
        f.unlink()
    rows = [recs[lab] for lab in order if lab in recs]
    cols = ["label", "kind", "machine_s", "encode_s", "bytes", "decode_s", "move_wall_s", "move_cpu_s", "paths",
            "py_rss_mb", "rust_rss_mb"]
    fmt = {"machine_s": "{:.4f}", "encode_s": "{:.4f}", "decode_s": "{:.4f}", "move_wall_s": "{:.4f}",
           "move_cpu_s": "{:.4f}", "py_rss_mb": "{:.1f}", "rust_rss_mb": "{:.1f}"}
    (folder / f"{stem}.rust-times.tsv").write_text("\t".join(cols) + "\n" + "".join(
        "\t".join(fmt.get(c, "{}").format(r[c]) for c in cols) + "\n" for r in rows))
    servers = {}
    for r in rows:  # a server reports its CPU so far: its last move's number is its total
        servers[r["server"]] = max(servers.get(r["server"], 0.0), r["rust_cpu_s"])
    # every process of the run: this one, the forked workers and every server (each waited for by its parent)
    me, kids = resource.getrusage(resource.RUSAGE_SELF), resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu = me.ru_utime + me.ru_stime + kids.ru_utime + kids.ru_stime
    rust_cpu = sum(servers.values())
    py_cpu = cpu - rust_cpu
    slow = sorted(rows, key=lambda r: -r["move_wall_s"])[:5]
    s = {"stem": stem, "moves_run": len(rows), "wall_s": round(wall, 3), "cpu_s": round(cpu, 3),
         "python_cpu_s": round(py_cpu, 3),
         "rust_cpu_s": round(rust_cpu, 3), "rust_machine_s": round(sum(r["machine_s"] for r in rows), 3),
         "rust_encode_s": round(sum(r["encode_s"] for r in rows), 3),
         "decode_s": round(sum(r["decode_s"] for r in rows), 3), "bytes": sum(r["bytes"] for r in rows),
         "servers": len(servers), "python_peak_rss_mb": round(max([r["py_rss_mb"] for r in rows] +
                                                                  [me.ru_maxrss / 1024]), 1),
         "children_peak_rss_mb": round(kids.ru_maxrss / 1024, 1),
         "rust_peak_rss_mb": round(max((r["rust_rss_mb"] for r in rows), default=0), 1),
         "slowest": [{k: r[k] for k in ("label", "machine_s", "encode_s", "decode_s", "move_wall_s", "bytes")}
                     for r in slow]}
    (folder / f"{stem}.rust-summary.json").write_text(json.dumps(s, indent=1) + "\n")
    print(f"check_rust {stem}: {len(rows)} moves on Rust; wall {wall:.1f} s; CPU {cpu:.1f} s: Python {py_cpu:.1f} s, Rust "
          f"{rust_cpu:.1f} s (machine {s['rust_machine_s']:.1f} s, encoding {s['rust_encode_s']:.1f} s); decoding "
          f"{s['decode_s']:.1f} s of {s['bytes'] / 1e6:.0f} MB; peak RSS Python {s['python_peak_rss_mb']:.0f} MB, "
          f"Rust {s['rust_peak_rss_mb']:.0f} MB; slowest "
          + ", ".join(f"{r['label']} {r['move_wall_s']:.2f} s (Rust {r['machine_s']:.2f} s)" for r in slow[:3]),
          file=sys.stderr)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0], add_help=False)
    ap.add_argument("--out", type=Path, required=True, help="the folder check.py writes into (in place of results/)")
    ap.add_argument("--verify", action="store_true", help="encode each decoded Run back and compare (slower)")
    ap.add_argument("--canonical", action="store_true", help="have the Rust send golden.py's lines unchanged")
    ap.add_argument("--no-build", action="store_true", help="do not run cargo build --release for machine-serve")
    ap.add_argument("--bin", type=Path, default=BIN, help="the machine-serve binary (implies --no-build)")
    a, rest = ap.parse_known_args()
    if any(x == "--resume" or x.startswith("--resume=") for x in rest):
        raise SystemExit("check_rust: --resume is refused (its records are keyed by the Python sources only)")
    out = a.out.resolve()
    if out == check.OUT.resolve():
        raise SystemExit("check_rust: --out is results/ itself; give another folder")
    out.mkdir(parents=True, exist_ok=True)
    if not a.no_build and a.bin == BIN:
        subprocess.run(["cargo", "build", "--release", "-q", "-p", "scumm-machine", "--bin", "machine-serve"],
                       cwd=SCUMM, check=True)
    if not a.bin.exists():
        raise SystemExit(f"check_rust: {a.bin} is missing (cargo build --release -p scumm-machine in scumm/)")
    _ARGS.update(verify=a.verify, canonical=a.canonical, dir=out, bin=a.bin.resolve())
    for f in out.glob(".rust-times.*.jsonl"):
        f.unlink()
    check.OUT = out
    check.run_model = run_model
    check.score_move = score_move
    sys.argv = [str(HERE / "check.py")] + rest
    t = time.perf_counter()
    try:
        check.main()
    finally:
        for srv in _SERVERS.values():
            srv.close()
    ns = check.argparse.ArgumentParser(add_help=False)
    ns.add_argument("--features", default="own_entry")
    ns.add_argument("--switches", default="")
    b, _ = ns.parse_known_args(rest)
    features = frozenset(f.strip() for f in b.features.split(",") if f.strip())
    switches = frozenset(f.strip() for f in b.switches.split(",") if f.strip())
    order = [check.label(r) for r in groundtruth.load()]
    summary(out, check.times_stem(features, switches), time.perf_counter() - t, order)


if __name__ == "__main__":
    main()
