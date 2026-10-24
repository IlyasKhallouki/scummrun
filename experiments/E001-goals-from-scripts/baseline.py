"""E001 step 1: score the quick goal regression against the frozen answer key.

usage: uv run python experiments/E001-goals-from-scripts/baseline.py

The regression is the autonomy probe's ``regress3.py``, unchanged: start from the
win fact (Bit[565], picked by hand), ask which player actions write it (object verb
handlers only, ``ScriptIndex.writers``), add the facts their ``if`` conditions read,
the items they use and the rooms they happen in, and repeat until nothing new comes in.

Scored by ``score.py``, next to the dump reference (every fact any player action touches).
Writes ``results/step1-baseline.json``.
"""

import collections
import json
import re
from pathlib import Path

from score import dump, score, table
from speedrun import paths
from speedrun.scriptindex import EGO, ScriptIndex
from speedrun.search import _query, guard_facts, persistent

HERE = Path(__file__).resolve().parent
SEED = ("bit", 565, 1)


def regress(ix: ScriptIndex) -> tuple[dict, list]:
    seen = {}
    stalls = []
    agenda = collections.deque([(SEED, 0)])
    while agenda:
        q, d = agenda.popleft()
        if q in seen:
            continue
        seen[q] = d
        if q[0] == "owner_not":
            ws = ix.writers("owner", q[1], None, exclude=(EGO,))
        elif q[0] == "room":
            ws = ix.writers("room", q[1], None)
        else:
            ws = ix.writers(*q)
        if not ws:
            stalls.append(q)
            continue
        for (_, obj, verb), eff in ws:
            needs = [_query(c) for c in persistent(guard_facts(eff.guards))]
            needs = [n for n in needs if n is not None and n[0] != "var" or (n and n[1] != 4)]

            def is_item(o):
                return o in ix.items or bool(ix.writers("owner", o, EGO))
            if verb in (7, 4, 80) and is_item(obj):
                needs.append(("owner", obj, EGO))
            r = ix.object_room.get(obj)
            if r and not is_item(obj) and q[0] != "room":
                needs.append(("room", r, None))
            for o in (int(x) for x in re.findall(r"(?:Local\[0\]|OBJ2) == (\d+)", " ".join(eff.guards))):
                if is_item(o):
                    needs.append(("owner", o, EGO))
            for n in needs:
                if n and n not in seen:
                    agenda.append((n, d + 1))
    return seen, stalls


def main() -> None:
    ix = ScriptIndex(paths.ROOT / "data" / "scripts-dos-cd")
    seen, stalls = regress(ix)
    ref = dump(ix, SEED)
    base = score(set(seen), SEED, ref)
    ref_score = score(ref, SEED, ref)
    result = {"seed": list(SEED), "pulled_in": len(seen), "max_depth": max(seen.values()), "stalls": len(stalls),
              "baseline": base, "dump": {k: v for k, v in ref_score.items() if k != "part1_facts"}}
    out = HERE / "results"
    out.mkdir(exist_ok=True)
    (out / "step1-baseline.json").write_text(json.dumps(result, indent=1) + "\n")
    print(f"pulled in {len(seen)} facts (depth {result['max_depth']}), {len(stalls)} with no player-action writer")
    print(table("baseline", base))
    print(table("dump", ref_score))


if __name__ == "__main__":
    main()
