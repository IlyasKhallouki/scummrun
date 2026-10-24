"""Build E001's answer key: the goal facts of each Part, from the Any% route.

usage: uv run python experiments/E001-goals-from-scripts/answer_key.py

The route's moves say what each one waits for (``until``). Those facts are the
route's goals, but they also hold travel, engine bookkeeping and the steps of
counters. The rules below remove those, the same way for every Part, and name no
object, script, room or variable of the game:

* K1 source: every persistent fact in the route moves' ``until`` (kinds from
  ``search.PERSISTENT``, no list of vars), credited to the first Part where it is a goal.
* K2 travel: rooms, actor rooms, ``VAR_ROOM``, and vars that hold a place:
  a var compared with ``VAR_ROOM`` or passed as a room to ``loadRoom``/``loadRoomWithEgo``,
  and a var whose every read is a coordinate (``putActor``, ``walkActorTo``, ``Pos``,
  or a line with an actor, object, mouse or camera position).
* K3 engine bookkeeping: the vars the engine owns (``ENGINE_VARS``, from ScummVM's
  ``scumm/vars.cpp`` for v5), the vars written directly by the scripts the boot script
  (global 1) installs as engine hooks, and the vars an input script copies from the
  click arguments (``Var[n] = Local[0..2]``).
* K4 steps: inside one Part, a bit or var keeps only the value the route needs last
  (cited at the first move that needs it). An earlier value is kept too when the game
  tests for exactly that value: some script assigns it as a constant, some condition
  compares with it, and (for a var) no script changes the var by arithmetic.

Writes ``key.json`` (the frozen key) and ``key.md`` (the same, readable) next to this file.
"""

import collections
import hashlib
import json
import re
from functools import cache
from pathlib import Path

from speedrun import paths, route
from speedrun.scriptindex import EGO, ScriptIndex
from speedrun.search import PERSISTENT, _query

HERE = Path(__file__).resolve().parent
SCRIPTS = paths.ROOT / "data" / "scripts-dos-cd"

PARTS = {"S1": "I", "S2": "I", "S3": "I", "S4": "II", "S5": "III", "S6": "III", "S7": "IV"}

# ScummEngine::setupScummVars() with its version >= 4 and >= 5 blocks, plus the
# ScummEngine_v5::setupScummVars() override (third_party/scummvm/engines/scumm/vars.cpp).
ENGINE_VARS = frozenset(
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, *range(11, 27), *range(28, 39), 40, *range(42, 50)]  # all versions
    + [27, 39, 50, 51, 52, 53]  # version >= 4
    + [56, 57, 59, *range(64, 73)]  # version >= 5
    + [9, 54, 60])  # ScummEngine_v5
VAR_ROOM = 4
HOOKS = ("VAR_ENTRY_SCRIPT", "VAR_ENTRY_SCRIPT2", "VAR_EXIT_SCRIPT", "VAR_EXIT_SCRIPT2", "VAR_VERB_SCRIPT",
         "VAR_SENTENCE_SCRIPT", "VAR_INVENTORY_SCRIPT", "VAR_CUTSCENE_START_SCRIPT", "VAR_CUTSCENE_END_SCRIPT")

_LINE = re.compile(r"^\[[0-9A-F]{4}\] \(..\)\s*(.*)$")
_LHS = re.compile(r"^(?:\}\s*)?(?:Exprmode\s+)?((?:Var|Bit|Local)\[(?:[^\[\]]|\[[^\]]*\])*\])\s*(=|\+=|-=|\*=|/=|\+\+|--)")
_ACCESS = re.compile(r"(Var|Bit)\[(\d+)(?: \+ (\d+))?\]")
_COND = re.compile(r"^(?:\}\s*else\s+)?(?:if|unless|while)\s*\(")
_COORD_CALL = re.compile(r"\b(?:putActor|walkActorTo)\(|\bPos\(|\bSetXY\(")
_COORD_READ = re.compile(r"getActor[XY]\(|getObject[XY]\(|VAR_(?:VIRT_)?MOUSE_[XY]\b|VAR_CAMERA_POS_X\b")
_ROOM_ARG = re.compile(r"\bloadRoom\(([^,)]*)\)|\bloadRoomWithEgo\([^,]*,([^,)]*)")


def _statements(path: Path):
    for line in path.read_text(errors="replace").splitlines():
        m = _LINE.match(line)
        if m:
            yield m[1].strip()


def _var(text: str) -> int | None:
    m = re.fullmatch(r"\s*Var\[(\d+)\]\s*", text)
    return int(m[1]) if m else None


@cache
def script_facts(scripts: Path = SCRIPTS) -> dict:
    """What the scripts do with each bit and var, read once over every file."""
    reads = collections.defaultdict(list)  # (kind, n) -> statements that read it
    assigned = collections.defaultdict(set)  # (kind, n) -> constants assigned to it
    arithmetic = set()  # vars changed with ++, --, +=, ...
    tested = collections.defaultdict(set)  # ("var", n) -> constants a condition compares it with
    room_vars = set()
    prev = ""
    for path in sorted(scripts.rglob("*.txt")):
        for st in _statements(path):
            rest = st
            m = _LHS.match(st)
            if m:
                lhs, op = m[1], m[2]
                rest = lhs[lhs.index("[") + 1:-1] + " " + st[m.end():]
                a = _ACCESS.fullmatch(lhs)
                if a:
                    kind, n = a[1].lower(), int(a[2]) + int(a[3] or 0)
                    if op == "=":
                        c = re.match(r"\s*(-?\d+);$", st[m.end():])
                        if c:
                            assigned[(kind, n)].add(int(c[1]))
                    elif kind == "var":
                        arithmetic.add(n)
            for kind, base, off in _ACCESS.findall(rest):
                reads[(kind.lower(), int(base) + int(off or 0))].append((st, prev))
            if _COND.match(st):
                for n, v in re.findall(r"Var\[(\d+)\] (?:==|!=) (-?\d+)\b", st):
                    tested[("var", int(n))].add(int(v))
                for v, n in re.findall(r"\b(-?\d+) (?:==|!=) Var\[(\d+)\]", st):
                    tested[("var", int(n))].add(int(v))
            for n in re.findall(r"Var\[(\d+)\] (?:==|!=) VAR_ROOM|VAR_ROOM (?:==|!=) Var\[(\d+)\]", st):
                room_vars.add(int(n[0] or n[1]))
            for args in _ROOM_ARG.findall(st):
                for a in args:
                    if _var(a) is not None:
                        room_vars.add(_var(a))
            prev = st
    return {"reads": reads, "assigned": assigned, "arithmetic": arithmetic, "tested": tested,
            "room_vars": room_vars}


def _coordinate_use(st: str, prev: str) -> bool:
    if _COORD_CALL.search(st) or _COORD_READ.search(st):
        return True
    return "VAR_RESULT" in st and re.search(r"VAR_RESULT = getActor[XY]\(|VAR_RESULT = getObject[XY]\(", prev) is not None


def position_vars(scripts: Path = SCRIPTS) -> set[int]:
    """Vars read at least once, and only ever read as a coordinate."""
    reads = script_facts(scripts)["reads"]
    return {n for (kind, n), rs in reads.items()
            if kind == "var" and rs and all(_coordinate_use(st, prev) for st, prev in rs)}


def _script_file(n: int, room_dir: Path | None, scripts: Path) -> Path | None:
    if n < 200:
        return scripts / "global" / f"script-{n:03d}.txt"
    return room_dir / f"local-{n}.txt" if room_dir else None


@cache
def engine_hook_vars(scripts: Path = SCRIPTS) -> dict[int, str]:
    """Vars the engine's hook scripts write directly, and vars input scripts copy from their arguments."""
    out = {}
    hooks = {}
    for st in _statements(scripts / "global" / "script-001.txt"):
        m = re.fullmatch(r"(VAR_[A-Z0-9_]+) = (\d+);", st)
        if m and m[1] in HOOKS:
            hooks[int(m[2])] = m[1]
    for n, slot in sorted(hooks.items()):
        for st in _statements(scripts / "global" / f"script-{n:03d}.txt"):
            m = _LHS.match(st)
            if m and _var(m[1]) is not None:
                out.setdefault(_var(m[1]), f"written by {slot} script {n}")
    for path in sorted(scripts.rglob("*.txt")):  # every script ever installed as the input script
        for st in _statements(path):
            m = re.fullmatch(r"VAR_VERB_SCRIPT = (\d+);", st)
            f = m and _script_file(int(m[1]), path.parent if path.parent.name.startswith("room-") else None, scripts)
            if f and f.exists():
                for s in _statements(f):
                    e = re.fullmatch(r"Var\[(\d+)\] = Local\[[012]\];", s)
                    if e:
                        out.setdefault(int(e[1]), f"input script {m[1]} copies a click argument")
    return out


def classify(q: tuple, scripts: Path = SCRIPTS) -> str | None:
    """K2 or K3 for a fact that is travel or engine bookkeeping, else None."""
    kind, n, _ = q
    if kind in ("room", "actor_room"):
        return "K2 travel"
    if kind != "var":
        return None
    if n == VAR_ROOM or n in script_facts(scripts)["room_vars"] or n in position_vars(scripts):
        return "K2 travel"
    if n in ENGINE_VARS or n in engine_hook_vars(scripts):
        return "K3 engine bookkeeping"
    return None


def tested_value(kind: str, n: int, v: int, scripts: Path = SCRIPTS) -> bool:
    """K4's exception: the game assigns ``v`` as a constant and tests for it."""
    sf = script_facts(scripts)
    if v not in sf["assigned"][(kind, n)]:
        return False
    if kind == "bit":
        return any(_COND.match(st) for st, _ in sf["reads"][("bit", n)])
    return n not in sf["arithmetic"] and v in sf["tested"][("var", n)]


def route_facts() -> tuple[list[dict], collections.Counter]:
    """K1: every persistent fact in the route moves' ``until``, in route order, and a count of
    the conditions that are not one fact (``{var, in: [...]}``, ``{not: room}``) or not persistent."""
    raw = {}
    for seg, fn in route.SEGMENTS.items():
        for i, m in enumerate(json.loads((route.ROUTE_DIR / fn).read_text())["moves"], 1):
            raw[f"{seg}#{i}"] = m
    out, other = [], collections.Counter()
    for mv in route.full_route():
        label = mv.name.split(" ")[0]
        for c in mv.until:
            inner = c.get("not", c)
            k = next(iter(inner))
            if k not in PERSISTENT:
                other[f"not persistent: {k}"] += 1
                continue
            q = ("room", inner["room"], None) if k == "room" and "not" not in c else _query(c)
            if q is None:
                other[f"not one fact: {json.dumps(c)}"] += 1
                continue
            out.append({"fact": list(q), "move": label, "part": PARTS[label.split("#")[0]],
                        "move_name": raw[label]["name"]})
    return out, other


def describe(q: tuple, ix: ScriptIndex) -> str:
    kind, n, v = q
    name = ix.object_name.get(n) if kind in ("owner", "owner_not", "state", "class") else None
    obj = f"{name} ({n})" if name else f"object {n}"
    if kind == "owner":
        return f"Guybrush has the {obj}" if v == EGO else f"the {obj} belongs to actor {v}"
    if kind == "owner_not":
        return f"Guybrush no longer has the {obj}"
    if kind == "state":
        return f"the {obj} is in state {v}"
    if kind == "class":
        return f"the {obj} gets class {v}" if v > 0 else f"the {obj} loses class {-v}"
    if kind == "bit":
        return f"Bit[{n}] = {v}"
    if kind == "var":
        return f"Var[{n}] = {v}"
    if kind == "script":
        return f"global script {n} is running"
    if kind == "room":
        return f"in room {n}"
    if kind == "actor_room":
        return f"actor {n} in room {v}"
    return f"{kind} {n} = {v}"


def build() -> dict:
    ix = ScriptIndex(SCRIPTS)
    facts, other = route_facts()
    dropped = collections.defaultdict(dict)  # rule -> fact -> first entry

    survivors = []
    for f in facts:
        q = tuple(f["fact"])
        rule = classify(q)
        if rule:
            dropped[rule].setdefault(q, f)
        else:
            survivors.append(f)

    last = {}  # K4: (part, kind, n) -> the value the route needs last in that Part
    for f in survivors:
        kind, n, v = f["fact"]
        if kind in ("bit", "var"):
            last[(f["part"], kind, n)] = v

    key, credited = [], set()
    for f in survivors:
        q = tuple(f["fact"])
        kind, n, v = q
        if kind in ("bit", "var") and v != last[(f["part"], kind, n)] and not tested_value(kind, n, v):
            dropped["K4 earlier step"].setdefault(q, f)
            continue
        if q in credited:  # K1: only the first Part where it is a goal
            continue
        credited.add(q)
        key.append(dict(f, means=describe(q, ix)))
    for rule in dropped:  # a fact kept somewhere is not listed as dropped
        dropped[rule] = {q: f for q, f in dropped[rule].items() if q not in credited}

    unique = {tuple(f["fact"]) for f in facts}
    return {
        "rules": "K1-K4, experiments/E001-goals-from-scripts/answer_key.py",
        "route_facts": len(unique),
        "outside_k1": dict(sorted(other.items())),
        "dropped": {k: [dict(f, means=describe(q, ix)) for q, f in v.items()] for k, v in sorted(dropped.items())},
        "parts": {p: sum(f["part"] == p for f in key) for p in ("I", "II", "III", "IV")},
        "key": key,
    }


def write_md(data: dict, digest: str) -> str:
    out = ["# E001 answer key", "",
           f"Built by `answer_key.py` (rules K1-K4). `key.json` sha256 `{digest}`.", "",
           f"{data['route_facts']} distinct route facts, {len(data['key'])} kept: "
           + ", ".join(f"Part {p} {n}" for p, n in data["parts"].items()) + ".", ""]
    for part in ("I", "II", "III", "IV"):
        out += [f"## Part {part}", "", "| # | Fact | Means | Route move |", "|---|---|---|---|"]
        rows = [f for f in data["key"] if f["part"] == part]
        for i, f in enumerate(rows, 1):
            out.append(f"| {i} | `{tuple(f['fact'])}` | {f['means']} | {f['move']} {f['move_name']} |")
        out.append("")
    out += ["## Dropped", ""]
    for rule, fs in data["dropped"].items():
        out += [f"### {rule} ({len(fs)})", ""]
        out += [f"- Part {f['part']}, `{tuple(f['fact'])}` {f['means']} ({f['move']} {f['move_name']})" for f in fs]
        out.append("")
    out += ["## Route conditions outside K1", "",
            "Conditions in the route's `until` that are not a persistent fact, or not one fact.", ""]
    out += [f"- {k}: {n}" for k, n in data["outside_k1"].items()]
    out.append("")
    return "\n".join(out)


def main() -> None:
    data = build()
    text = json.dumps(data, indent=1) + "\n"
    (HERE / "key.json").write_text(text)
    digest = hashlib.sha256(text.encode()).hexdigest()
    (HERE / "key.md").write_text(write_md(data, digest))
    print(f"{data['route_facts']} route facts -> {len(data['key'])} kept {data['parts']}")
    print({k: len(v) for k, v in data["dropped"].items()})
    print("key.json sha256", digest)


if __name__ == "__main__":
    main()
