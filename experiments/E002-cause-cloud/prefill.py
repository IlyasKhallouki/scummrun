"""E002 step 2 input: the engine calls each v5 o5 function makes, found by text search.

usage: uv run python experiments/E002-cause-cloud/prefill.py   (after census.py)

For every o5 function in the v5 table, used by MI1 or not (``results/census.json``), it cuts the
function's body out of ``script_v5.cpp`` and lists the lines that call one of the engine's state
primitives below.
This is a starting point for the opcode table, not the table: a primitive reached through a
helper in another file is not seen here, and reviewers check every row against the source.
Writes ``results/prefill.json``.
"""

import json
import re
from pathlib import Path

from census import ENGINE, HERE

PRIMITIVES = {
    "var": r"\bsetResult\(|\bwriteVar\(",
    "owner": r"\bsetOwnerOf\(|\bputOwner\(|\baddObjectToInventory\(",
    "state": r"\bputState\(",
    "class": r"\bputClass\(",
    "room": r"\bstartScene\(",
    "actor": r"\bputActor\(|->_room\b|\bstartWalkActor\(|\bstartWalk\(",
    "boxes": r"\bsetBoxFlags\(|\bcreateBoxMatrix\(|\bsetBoxScale\(",
    "starts": r"\brunScript\(|\brunObjectScript\(|_sentence\[|\bdoSentence\(",
    "stops": r"\bstopScript\(|\bstopObjectScript\(|\bfreezeScripts\(",
    "input": r"_userPut\b|_cursor\.state\b|\b_userState\b",
    "names": r"\bsetObjectName\(|\bloadPtrToResource\(|\bsetStringVars\(",
    "verbs": r"_verbs\[|\bkillVerb\(|\bredrawVerbs\(|\bsaveVerbs\(|\brestoreVerbs\(",
    "random": r"\bgetRandomNumber|_rnd\.",
    "cutscene": r"\bbeginCutscene\(|\bendCutscene\(|\bbeginOverride\(|\bendOverride\(",
    "blocks": r"\bbreakHere\(|\.delay\b|\bdelay\b.*=|_haveMsg\b|\bactorTalk\(|\bVAR_TIMER\b",
}


def bodies() -> dict[str, tuple[int, list[str]]]:
    """o5 function -> (first line number, body lines), from its definition to the closing ``}``."""
    lines = (ENGINE / "script_v5.cpp").read_text().splitlines()
    out, name = {}, None
    for n, line in enumerate(lines, 1):
        m = re.match(r"^void ScummEngine_v5::(o5_\w+)\(\) \{", line)
        if m:
            name, start, body = m[1], n, []
        elif name and line == "}":
            out[name] = (start, body)
            name = None
        elif name:
            body.append((n, line))
    return out


def main() -> None:
    census = json.loads((HERE / "results" / "census.json").read_text())
    src = bodies()
    rows = {}
    unused = {name: {"lines": 0, "forms": {}, "subops": {}, "example": None} for name in census["unused"]}
    for name, c in {**census["functions"], **unused}.items():
        start, body = src[name]
        hits = {}
        for kind, rx in PRIMITIVES.items():
            found = [n for n, line in body if re.search(rx, line) and not line.strip().startswith("//")]
            if found:
                hits[kind] = found
        rows[name] = {"source": f"script_v5.cpp:{start}", "body_lines": len(body), "primitives": hits,
                      "lines_in_mi1": c["lines"], "forms": c["forms"], "subops": c["subops"],
                      "example": c["example"]}
    (HERE / "results" / "prefill.json").write_text(json.dumps(rows, indent=1) + "\n")
    for name, r in rows.items():
        print(f"{name:28} {r['source']:22} {', '.join(r['primitives']) or '-'}")


if __name__ == "__main__":
    main()
