"""E003 step 2: the Part I end, the key's last three Part I goals as facts. A disclosed human input.

usage: uv run python experiments/E003-path-through-cloud/end_part1.py   (writes results/end-part1.json and .log)

plan.md step 2: the Part I end is the key's goals #97-99, the facts written when Part I hands over to
Part II. The key (``experiments/E001-goals-from-scripts/key.json``) was built from the human route, so
this is a human input, used only to tune on Part I; it breaks E001's rule C3, and plan.md C2 lists it as
a disclosed exception. The held-out run (step 6) does not use it: its goal is the game end (end.py).
Nothing the system runs imports this file; the system reads only ``results/end-part1.json``, which
holds the goal (``goal_all``, ``goal_not``), the key's entry numbers and the key's sha256, and no route
data: the route moves, their names and E001's object names (made through ScriptIndex) go to the log
only.

Which entries: the last three of the key's entries whose Part is "I", in key order; the log checks that
they are #97-99. Each key fact ``[kind, n, v]`` (E001 answer_key.py ``describe``) maps to one facts.py
key: ``owner`` -> ``owner:n`` = v (v the owner: an actor number, or 0x0F for the room); ``state`` ->
``state:n`` = v; ``bit`` -> ``bit:n`` = v; ``var`` -> ``var:n`` = v; ``script`` -> ``script:n`` = 1;
``class`` -> ``class:n:|v|`` = 1 when v > 0, 0 when v < 0 (the object gains or loses class |v|);
``owner_not`` -> ``owner:n`` != v. The goal holds when every fact holds.

Checks, in the log: the three entries and their keys; the actor they name against the start's ``ego``
(``start-part1.jsonl``, so "Guybrush has" is ``owner:n`` == the start's VAR_EGO); their values at each
seed's start (they must not hold yet); how many of the key's 99 Part I goals already hold at the start
(the boot script's debug-only branch, which the start excludes, would write 31 of them, plan.md step 2);
and the lines of ``groundtruth-part1.log`` where run B's engine reached these facts at its Part I end.
"""

import hashlib
import json
import os
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
KEY = ROOT / "experiments" / "E001-goals-from-scripts" / "key.json"
START = HERE / "results" / "start-part1.jsonl"
GROUNDTRUTH_LOG = HERE / "results" / "groundtruth-part1.log"
OUT = HERE / "results" / "end-part1.json"
LOG = HERE / "results" / "end-part1.log"
PART = "I"
LAST = 3  # plan.md step 2: "the key's last three Part I goals (#97-99)"


def as_fact(fact: list) -> tuple[str, str, int]:
    """A key fact ``[kind, n, v]`` as (facts.py key, "==" or "!=", value)."""
    kind, n, v = fact
    if kind in ("owner", "state", "bit", "var"):
        return f"{kind}:{n}", "==", v
    if kind == "script":
        return f"script:{n}", "==", 1
    if kind == "class":
        return f"class:{n}:{abs(v)}", "==", int(v > 0)
    if kind == "owner_not":
        return f"owner:{n}", "!=", v
    raise ValueError(f"key fact kind {kind!r} has no facts.py key")


def holds(facts: dict[str, int], key: str, op: str, value: int) -> bool:
    got = facts.get(key, 0)
    return got == value if op == "==" else got != value


def part_end(key: list[dict], part: str = PART, last: int = LAST) -> list[dict]:
    """The last ``last`` entries of ``part``, with their entry numbers (1-based) and facts.py keys."""
    picked = [(n, e) for n, e in enumerate(key, 1) if e["part"] == part][-last:]
    out = []
    for n, e in picked:
        k, op, v = as_fact(e["fact"])
        out.append({"key_entry": n, "key_fact": e["fact"], "fact": k, "op": op, "value": v, "move": e["move"],
                    "move_name": e["move_name"], "means": e["means"]})
    return out


def main() -> None:
    raw = KEY.read_bytes()
    data = json.loads(raw)
    goals = part_end(data["key"])
    doc = {"source": str(KEY.relative_to(ROOT)), "key_sha256": hashlib.sha256(raw).hexdigest(),
           "disclosed": "human input from the Any% route via E001's key; tuning on Part I only; breaks E001 rule C3 "
                        "(plan.md step 2, C2)",
           "goal_all": {g["fact"]: g["value"] for g in goals if g["op"] == "=="},
           "goal_not": {g["fact"]: g["value"] for g in goals if g["op"] == "!="},
           "key_entries": [g["key_entry"] for g in goals]}
    log = [f"key: {doc['source']} sha256 {doc['key_sha256'][:16]}..., {len(data['key'])} entries, "
           f"Part {PART}: {sum(e['part'] == PART for e in data['key'])}",
           f"Part {PART} end = the last {LAST} Part {PART} entries: #{[g['key_entry'] for g in goals]} "
           f"({'#97-99 as plan.md says' if [g['key_entry'] for g in goals] == [97, 98, 99] else 'NOT #97-99'})"]
    for g in goals:
        log.append(f"  #{g['key_entry']} {g['key_fact']} -> {g['fact']} {g['op']} {g['value']}  "
                   f"({g['means']}; route move {g['move']}: {g['move_name']})")
    if START.exists():
        starts = [json.loads(l) for l in START.read_text().splitlines() if l.strip()]
        egos = sorted({s["facts"].get("ego") for s in starts})
        owners = sorted({g["value"] for g in goals if g["fact"].startswith("owner:")})
        log.append(f"the owner value(s) {owners} against the start's ego {egos}: "
                   f"{'the same actor' if owners == egos else 'DIFFER'}")
        for s in starts:
            vals = {g["fact"]: s["facts"].get(g["fact"], 0) for g in goals}
            held = [g["fact"] for g in goals if holds(s["facts"], g["fact"], g["op"], g["value"])]
            part = [(n, as_fact(e["fact"])) for n, e in enumerate(data["key"], 1) if e["part"] == PART]
            already = [n for n, p in part if holds(s["facts"], *p)]
            log.append(f"  seed {s['seed']} start (frame {s['stop']['frame']}): {vals}; end facts holding: {held}; "
                       f"Part {PART} key goals already holding: {len(already)} of {len(part)} #{already}")
        for n in already:
            e = data["key"][n - 1]
            log.append(f"    #{n} {e['fact']} ({e['means']}; route move {e['move']})")
    else:
        log.append(f"{START.name} missing: start values not checked")
    if GROUNDTRUTH_LOG.exists():
        lines = [l for l in GROUNDTRUTH_LOG.read_text().splitlines()
                 if any(l.startswith(f"key #{g['key_entry']} ") for g in goals)]
        log.append(f"run B's engine at its Part {PART} end ({GROUNDTRUTH_LOG.name}, an instrument):")
        log += [f"  {l}" for l in lines] or ["  no such lines"]
    for path, text in ((OUT, json.dumps(doc, indent=1) + "\n"), (LOG, "\n".join(log) + "\n")):
        tmp = path.with_name(path.name + ".tmp")
        tmp.write_text(text)
        os.replace(tmp, path)
    print("\n".join(log))


if __name__ == "__main__":
    main()
