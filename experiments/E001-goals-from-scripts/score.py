"""Score a set of facts an analysis pulled in against E001's frozen answer key.

Recall alone can be gamed: a list of every fact any player action touches finds
most of the key without any analysis. So every score also gives

* ``size``: how many facts the analysis hands over (travel and engine bookkeeping
  removed with the key's own K2/K3 rules, the analysis's starting fact removed);
* ``chance``: per Part, the hits a random pick of that many facts from the dump would get;
* the dump itself as a reference row (``dump()``).

Per-fact results are returned for Part I only (the dev set).
"""

import hashlib
import json
from pathlib import Path

from answer_key import classify
from speedrun.scriptindex import EGO, ScriptIndex
from speedrun.search import _query, guard_facts, persistent

HERE = Path(__file__).resolve().parent
PARTS = ("I", "II", "III", "IV")
KEY_SHA256 = "625462f6ff4839d0f5821a9eaf6acf6f80fb307aa7fc944fd71943049a270ad9"  # the frozen key (plan.md)


def load_key() -> list[dict]:
    data = (HERE / "key.json").read_bytes()
    if hashlib.sha256(data).hexdigest() != KEY_SHA256:
        raise SystemExit("key.json is not the frozen key; see plan.md")
    return json.loads(data)["key"]


def handed_over(facts, seed: tuple) -> set[tuple]:
    """The facts that count toward ``size``."""
    return {q for q in facts if q != seed and classify(q) is None}


def dump(ix: ScriptIndex, seed: tuple) -> set[tuple]:
    """Every persistent fact a player action writes, plus every fact its conditions read."""
    out = set()
    for key in ix.actions():
        for e in ix.effects(key):
            if e.kind in ("bit", "var", "owner", "state", "class"):
                out.add((e.kind, e.subject, e.value))
                if e.kind == "owner" and e.value != EGO:
                    out.add(("owner_not", e.subject, EGO))
            for c in persistent(guard_facts(e.guards)):
                q = _query(c)
                if q:
                    out.add(q)
    return handed_over(out, seed)


def score(found: set[tuple], seed: tuple, ref: set[tuple]) -> dict:
    """Recall per Part, size, and the chance hits of a same-size pick from ``ref`` (the dump)."""
    key = [f for f in load_key() if tuple(f["fact"]) != seed]
    given = handed_over(found, seed)
    parts = {}
    for p in PARTS:
        fs = [f for f in key if f["part"] == p]
        hits = sum(tuple(f["fact"]) in given for f in fs)
        in_ref = sum(tuple(f["fact"]) in ref for f in fs)
        parts[p] = {"facts": len(fs), "found": hits, "recall": round(hits / len(fs), 3),
                    "chance": round(min(len(given), len(ref)) / len(ref) * in_ref, 1)}
    held = [parts[p] for p in ("II", "III", "IV")]
    return {
        "size": len(given),
        "parts": parts,
        "held_out_pooled": {"facts": sum(p["facts"] for p in held), "found": sum(p["found"] for p in held)},
        "part1_facts": [{"fact": f["fact"], "means": f["means"], "move": f["move"],
                         "found": tuple(f["fact"]) in given} for f in key if f["part"] == "I"],
    }


def table(name: str, s: dict) -> str:
    h = s["held_out_pooled"]
    cells = "  ".join(f"{p} {s['parts'][p]['found']:3}/{s['parts'][p]['facts']:<3} (chance {s['parts'][p]['chance']:5.1f})"
                      for p in PARTS)
    return f"{name:10} size {s['size']:5}  {cells}  II-IV {h['found']}/{h['facts']}"
