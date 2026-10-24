"""check.py's helpers for scoring moves on many cores: each move's CPU time and peak RSS, the per-move times file,
and the resume folder that keeps each finished move's record.

A record is kept with pickle, not JSON: the log is built from the records in memory, and a JSON round trip turns
tuples into lists. The record's JSON line (``json.dumps(out, default=str)``, as check.py writes it) is kept beside
it and checked on load, so a record whose pickle round trip changed what check.py would write is never used.

check.py imports this module before the model, so ``AT_IMPORT`` hashes the sources as Python is about to read them.
"""

import hashlib
import json
import os
import pickle
import re
import resource
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
# What a move's record depends on besides the features, the switches and the ground truth file (``run_key``): the
# sources (groundtruth.py builds the rows from the ground truth file; the executor's profile gives check.py's walk
# verb), the game's decompiled scripts and raw blocks (``ir.load``, ``model.Game.load``: ir.SCRIPTS, and the block
# paths of its index.json, all under build/blocks-dos-cd), and the Python that runs them (``sys.version``).
SOURCES = tuple(HERE / n for n in ("check.py", "model.py", "actions.py", "facts.py", "ir.py", "groundtruth.py")) \
    + (ROOT / "src" / "speedrun" / "executor.py",)
TREES = (ROOT / "data" / "scripts-dos-cd", ROOT / "build" / "blocks-dos-cd")
TIMES_HEAD = "label\tkind\tseconds\tpeak_rss_mb\tpaths"


def sources_digest() -> str:
    """One hash of every file in ``SOURCES``, by name and content."""
    h = hashlib.sha256()
    for p in SOURCES:
        h.update(f"{p.name}\n".encode())
        h.update(hashlib.sha256(p.read_bytes()).digest())
    return h.hexdigest()


def trees_digest() -> str:
    """One hash of every file under ``TREES``, by its path in the tree and its content (a rename changes it too)."""
    h = hashlib.sha256()
    for root in TREES:
        h.update(f"{root.name}\n".encode())
        for p in sorted(root.rglob("*")):
            if p.is_file():
                h.update(f"{p.relative_to(root).as_posix()}\n".encode())
                h.update(hashlib.sha256(p.read_bytes()).digest())
    return h.hexdigest()


AT_IMPORT = sources_digest()  # the sources when check.py started, before it imported the model


def run_key(features, switches, gt_digest: str, sources: str | None = None) -> str:
    """The hash a kept record must carry: the features, the switches, the ground truth file's hash, the Python
    version, ``sources`` (a ``sources_digest()`` taken earlier, as ``AT_IMPORT``; else read now) and the game's files
    (``trees_digest``). Any change to one of them makes every kept record stale."""
    return hashlib.sha256(
        f"features={','.join(sorted(features))}\nswitches={','.join(sorted(switches))}\ngroundtruth={gt_digest}\n"
        f"python={sys.version}\nsources={sources or sources_digest()}\ngame={trees_digest()}\n".encode()).hexdigest()


def _reset_peak() -> None:
    """Reset this process's peak RSS (VmHWM), so the next read is the peak of what ran since (Linux 4.0+)."""
    try:
        Path("/proc/self/clear_refs").write_text("5")
    except OSError:
        pass


def _peak_mb() -> float:
    try:
        for line in Path("/proc/self/status").read_text().splitlines():
            if line.startswith("VmHWM:"):
                return int(line.split()[1]) / 1024
    except OSError:
        pass
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024


def measure(fn, *args):
    """(fn(*args), CPU seconds of this process while it ran, peak RSS in MB of this process while it ran)."""
    _reset_peak()
    t0 = time.process_time()
    out = fn(*args)
    return out, time.process_time() - t0, _peak_mb()


class Store:
    """The resume folder: one file per finished move, named by its label and the run's key (``run_key``)."""

    def __init__(self, folder: Path, key: str):
        self.folder, self.key = Path(folder), key
        self.folder.mkdir(parents=True, exist_ok=True)
        self.loaded = 0

    def path(self, label: str) -> Path:
        return self.folder / f"{re.sub(r'[^A-Za-z0-9#_-]', '_', label)}.{self.key[:16]}.pkl"

    def get(self, label: str, head: str):
        """(record, seconds, peak RSS) kept for ``label`` under this key, or None. ``head``: what the record must have
        been scored for (check.py's ``head_line``: the move's status, kind and action). A file cut short (a VM taken
        away while it wrote), one for another key, label or head, or one whose JSON line no longer matches is not
        used."""
        p = self.path(label)
        if not p.exists():
            return None
        try:
            d = pickle.loads(p.read_bytes())
            if d["key"] != self.key or d["label"] != label or d["head"] != head \
                    or json.dumps(d["out"], default=str) != d["json"]:
                return None
        except Exception:
            return None
        self.loaded += 1
        return d["out"], d["seconds"], d["rss_mb"]

    def put(self, label: str, head: str, out: dict, line: str, seconds: float, rss_mb: float) -> None:
        p = self.path(label)
        tmp = p.with_name(p.name + f".{os.getpid()}.tmp")
        tmp.write_bytes(pickle.dumps({"key": self.key, "label": label, "head": head, "out": out, "json": line,
                                      "seconds": seconds, "rss_mb": rss_mb}, protocol=pickle.HIGHEST_PROTOCOL))
        os.replace(tmp, p)


def read_times(paths) -> dict:
    """{label: seconds} from the first times file of ``paths`` that exists, else {}."""
    for p in paths:
        p = Path(p)
        if p.exists():
            out = {}
            for line in p.read_text().splitlines()[1:]:
                f = line.split("\t")
                if len(f) >= 3:
                    out[f[0]] = float(f[2])
            return out
    return {}


def write_times(path: Path, rows: list) -> None:
    """``rows``: (label, kind, seconds, peak RSS in MB, paths), in move order."""
    Path(path).write_text(TIMES_HEAD + "\n" + "".join(
        f"{lab}\t{kind}\t{sec:.3f}\t{rss:.1f}\t{paths}\n" for lab, kind, sec, rss, paths in rows))
