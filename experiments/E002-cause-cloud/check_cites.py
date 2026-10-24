"""E002: check that every source citation in the engine tables points at real lines.

usage: uv run python experiments/E002-cause-cloud/check_cites.py

The engine tables (``results/engine-opcodes.json``, ``results/engine-triggers.json``) were written
by model agents, and their claims rest on the ScummVM source lines they cite. This finds every
``file.ext:N`` or ``file.ext:N-M`` in them and checks that the file exists under ``third_party/``
(the path as written, else ``engines/scumm/``, else the one file with that name under
``third_party/scummvm`` or ``third_party/scummvm-tools``) and that the lines are inside it.
It does not check that a line says what the row claims; it checks that the citation can be read.
"""

import collections
import json
import re
from pathlib import Path

from census import HERE
from speedrun import paths

THIRD = paths.ROOT / "third_party"
SCUMMVM = THIRD / "scummvm"
_CITE = re.compile(r"((?:[\w.-]+/)*[\w.-]+\.(?:cpp|h)):(\d+)(?:-(\d+))?")


def lines_of(name: str, cache: dict) -> int | None:
    """Line count of the cited file, or None if no such file exists."""
    if "_by_name" not in cache:
        cache["_by_name"] = collections.defaultdict(list)
        for root in (SCUMMVM, THIRD / "scummvm-tools"):
            for p in root.rglob("*"):
                if p.suffix in (".cpp", ".h"):
                    cache["_by_name"][p.name].append(p)
    if name not in cache:
        named = cache["_by_name"].get(Path(name).name, [])
        path = next((p for p in (paths.ROOT / name, THIRD / name, SCUMMVM / name, SCUMMVM / "engines" / "scumm" / name)
                     if p.is_file()), named[0] if len(named) == 1 else None)
        cache[name] = len(path.read_text(errors="replace").splitlines()) if path else None
    return cache[name]


def strings(x):
    if isinstance(x, str):
        yield x
    elif isinstance(x, dict):
        for v in x.values():
            yield from strings(v)
    elif isinstance(x, list):
        for v in x:
            yield from strings(v)


def main() -> None:
    cache: dict = {}
    for name in ("engine-opcodes.json", "engine-triggers.json"):
        data = json.loads((HERE / "results" / name).read_text())
        tally, bad = collections.Counter(), []
        for s in strings(data):
            for f, a, b in _CITE.findall(s):
                n = lines_of(f, cache)
                last = int(b or a)
                if n is None:
                    tally["file not found"] += 1
                    bad.append(f"{f}:{a}")
                elif int(a) < 1 or last > n or last < int(a):
                    tally["line out of range"] += 1
                    bad.append(f"{f}:{a}{'-' + b if b else ''} (file has {n})")
                else:
                    tally["ok"] += 1
        total = sum(tally.values())
        print(f"{name}: {total} citations, {tally['ok']} resolve, {tally['file not found']} file not found, "
              f"{tally['line out of range']} out of range")
        for x in sorted(set(bad))[:15]:
            print("   ", x)


if __name__ == "__main__":
    main()
