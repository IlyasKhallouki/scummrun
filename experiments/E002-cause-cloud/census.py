"""E002 step 1: which SCUMM v5 opcodes MI1's scripts use, and which of them ScriptIndex reads.

usage: uv run python experiments/E002-cause-cloud/census.py

Every decompiled line is ``[OFFS] (OP) statement``. OP is the opcode byte, so each line maps
to the engine function that runs it through ScummVM's v5 table (``OPCODE(0xNN, o5_x)`` in
``script_v5.cpp``). The census stops if a byte has no entry: then the list of what the scripts
can do would not be complete.

For each o5 function it records how many lines use it, the statement forms descumm prints
(the head name, plus the sub-operations inside ``[...]`` for ActorOps and VerbOps), and how
many of its lines match one of ``ScriptIndex``'s effect patterns. Writes ``results/census.json``.
"""

import collections
import json
import re
from pathlib import Path

from speedrun import paths
from speedrun.scriptindex import _EFFECTS, _LINE

HERE = Path(__file__).resolve().parent
SCRIPTS = paths.ROOT / "data" / "scripts-dos-cd"
ENGINE = paths.ROOT / "third_party" / "scummvm" / "engines" / "scumm"
_OPCODE = re.compile(r"^\s*OPCODE\(0x([0-9a-f]{2}), (o5_\w+)\);")
_HEAD = re.compile(r"^\}?\s*(?:else\s+)?(?:if\s*\(\s*!?)?([A-Za-z_.]+)")
_SUBOP = re.compile(r"(\w+)\(")


def opcode_table() -> dict[int, tuple[str, int]]:
    """Byte -> (o5 function, source line). The first entry per byte is the v5 table; a later
    entry for the same byte sits behind a ``_game`` guard (Sega CD) and does not apply here.
    Bytes whose entry is commented out (``//	OPCODE(...)``, v4 leftovers) are not valid v5."""
    table, dropped = {}, set()
    lines = (ENGINE / "script_v5.cpp").read_text().splitlines()
    for n, line in enumerate(lines, 1):
        m = _OPCODE.match(line)
        if m and int(m[1], 16) not in table:
            table[int(m[1], 16)] = (m[2], n)
        elif m := _OPCODE.match(line.lstrip("/")):
            dropped.add(int(m[1], 16))
    if len(table) + len(dropped - set(table)) != 256:
        raise SystemExit(f"the v5 opcode table covers {len(table)} + {len(dropped)} bytes, not 256")
    return table


def form(st: str) -> tuple[str, list[str]]:
    """The statement's head name and, for a bracketed op list, the sub-operation names."""
    m = _HEAD.match(st)
    head = m[1] if m else st.split(" ")[0]
    subs = []
    if head in ("ActorOps", "VerbOps") and "[" in st:
        subs = _SUBOP.findall(st[st.index("[") :])
    return head, subs


def census() -> dict:
    table = opcode_table()
    by_fn = collections.defaultdict(lambda: {"bytes": set(), "lines": 0, "read": 0, "files": set(),
                                             "forms": collections.Counter(), "subops": collections.Counter(),
                                             "example": None})
    total = 0
    for path in sorted(SCRIPTS.rglob("*.txt")):
        for raw in path.read_text().splitlines():
            m = _LINE.match(raw)
            if not m or m[2] == "**":
                continue
            op = int(m[2], 16)
            if op not in table:
                raise SystemExit(f"{path}: opcode byte {m[2]} has no v5 table entry")
            st = m[4].strip()
            if st.startswith("Exprmode "):  # as ScriptIndex reads it (_parse_lines)
                st = st[len("Exprmode "):]
            fn = by_fn[table[op][0]]
            fn["bytes"].add(op)
            fn["lines"] += 1
            fn["read"] += any(rx.search(st) for _, rx in _EFFECTS)
            fn["files"].add(str(path.relative_to(SCRIPTS)))
            head, subs = form(st)
            fn["forms"][head] += 1
            fn["subops"].update(subs)
            fn["example"] = fn["example"] or st[:120]
            total += 1
    used = {name for name, _ in table.values()}
    return {
        "lines": total,
        "table_functions": len(used),
        "functions": {
            name: {"source_line": next(n for nm, n in table.values() if nm == name),
                   "bytes": [f"{b:02X}" for b in sorted(d["bytes"])], "lines": d["lines"],
                   "read_by_scriptindex": d["read"], "files": len(d["files"]),
                   "forms": dict(d["forms"].most_common()), "subops": dict(d["subops"].most_common()),
                   "example": d["example"]}
            for name, d in sorted(by_fn.items(), key=lambda kv: -kv[1]["lines"])
        },
        "unused": sorted(used - set(by_fn)),
    }


def main() -> None:
    c = census()
    out = HERE / "results"
    out.mkdir(exist_ok=True)
    (out / "census.json").write_text(json.dumps(c, indent=1) + "\n")
    fns = c["functions"]
    print(f"{c['lines']} lines, every opcode byte maps; MI1 uses {len(fns)} of the table's "
          f"{c['table_functions']} o5 functions")
    for name, d in fns.items():
        print(f"  {name:28} {d['lines']:6} lines  {d['read_by_scriptindex']:6} read  {d['files']:4} files  "
              f"{', '.join(list(d['forms'])[:4])}")
    print("not used:", ", ".join(c["unused"]))


if __name__ == "__main__":
    main()
