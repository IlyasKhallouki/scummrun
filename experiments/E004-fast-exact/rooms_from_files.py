"""E004: could Part I's scope be found from the game files alone? The rooms reachable from the start by the room
changes the scripts make, against the Part I scope (the rooms of results/groundtruth-part1.jsonl's moves).

usage: rooms_from_files.py [--seed 1] [--out FILE] [--no-start-globals]

A room's code is its entry and exit code, its local scripts and the verb code of the objects defined in it, plus every
global script that code starts by a constant number, transitively; the globals the start runs or installs as hooks
(the hook variables' start values, the scripts live at the start) count as code of every room, unless
--no-start-globals. A room change is a
`loadRoom(R)` or `loadRoomWithEgo(O, R)` with a constant room (a pseudo room through the scripts' PseudoRoom map, as
model.Game.room_resource reads it), or with a variable: every constant some script assigns to that variable, or with
a local that is the script's argument: that argument of each start by a constant number with a constant there; and an
`actorFollowCamera`, which goes to the followed actor's room: every constant room a `putActorInRoom` of the same unit
puts an actor in. From the start's room, every room a reached room's code changes to is reached. Room changes by any
other expression are counted, not followed. Only room numbers are written.
"""

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
E003 = HERE.parents[0] / "E003-path-through-cloud"
sys.path.insert(0, str(E003))

import model  # noqa: E402
from ir import Assign, Const, Local, Op, Start, Var  # noqa: E402

HOOKS = (27, 28, 29, 30, 31, 32, 33, 34, 35, 36)  # VAR_SCROLL_SCRIPT .. VAR_CUTSCENE_END_SCRIPT (vars.cpp:62-71)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--out")
    ap.add_argument("--no-start-globals", action="store_true")
    a = ap.parse_args()
    g = model.Game.load()
    starts = [json.loads(l) for l in (E003 / "results" / "start-part1.jsonl").read_text().splitlines() if l.strip()]
    start = next(s for s in starts if s["seed"] == a.seed)["facts"]
    res = lambda r: g.pseudo.get(r, r)  # noqa: E731
    # every constant each variable is assigned anywhere (a room change by a variable reaches each)
    var_consts = {}
    for u in g.units.values():
        for x in u.ins:
            st = x.st
            if isinstance(st, Assign) and st.op == "=" and isinstance(st.lhs, Var) and isinstance(st.rhs, Const):
                var_consts.setdefault(st.lhs.n, set()).add(st.rhs.value)

    # the globals that change the room to one of their arguments: global -> the argument's local numbers
    arg_rooms = {}
    for k, u in g.units.items():
        for x in u.ins:
            st = x.st
            if k[0] == "global" and isinstance(st, Op) and st.name in ("loadRoom", "loadRoomWithEgo"):
                arg = st.args[0] if st.name == "loadRoom" else st.args[1]
                if isinstance(arg, Local):
                    arg_rooms.setdefault(k[1], set()).add(arg.n)

    def code(u):
        """(constant global starts, constant room changes, room changes that are not constants) of one unit."""
        gl, rooms, wild = set(), set(), 0
        follows = any(isinstance(x.st, Op) and x.st.name == "actorFollowCamera" for x in u.ins)
        for x in u.ins:
            st = x.st
            if isinstance(st, Start) and st.name != "startObject" and isinstance(st.script, Const):
                if 0 < st.script.value < g.first_local:
                    gl.add(st.script.value)
                for i in arg_rooms.get(st.script.value, ()):
                    a = st.args[i] if i < len(st.args) else None
                    if isinstance(a, Const) and a.value > 0:
                        rooms.add(res(a.value))
            if follows and isinstance(st, Op) and st.name == "putActorInRoom" and isinstance(st.args[1], Const):
                if st.args[1].value > 0:
                    rooms.add(res(st.args[1].value))
            if isinstance(st, Op) and st.name in ("loadRoom", "loadRoomWithEgo"):
                arg = st.args[0] if st.name == "loadRoom" else st.args[1]
                if isinstance(arg, Const):
                    rooms.add(res(arg.value))
                elif isinstance(arg, Var) and arg.n in var_consts:
                    rooms |= {res(v) for v in var_consts[arg.n] if v > 0}
                elif not isinstance(arg, Local):
                    wild += 1
        return gl, rooms, wild

    per = {k: code(u) for k, u in g.units.items()}

    def closure(keys):
        """The room changes of `keys`' code and of every global it starts, transitively."""
        todo, seen, rooms, wild = list(keys), set(), set(), 0
        while todo:
            k = todo.pop()
            if k in seen or k not in per:
                continue
            seen.add(k)
            gl, rs, w = per[k]
            rooms |= rs
            wild += w
            todo += [("global", n) for n in gl]
        return rooms, wild, seen

    everywhere = [("global", start.get(f"var:{h}", 0)) for h in HOOKS if start.get(f"var:{h}", 0)]
    everywhere += [("global", int(k.split(":")[1])) for k, v in start.items() if k.startswith("script:") and v == 1]
    common, common_wild, _ = closure([] if a.no_start_globals else everywhere)

    def room_code(r):
        ks = [k for k in g.units if (k[0] in ("entry", "exit") and k[1] == r) or (k[0] == "local" and k[1] == r)]
        ks += [("obj", o) for o, rr in g.object_room.items() if rr == r and ("obj", o) in g.units]
        return ks

    r0 = res(start["room"])
    reached, todo, edges, wild = {r0}, [r0], {}, {}
    while todo:
        r = todo.pop()
        rs, w, _ = closure(room_code(r))
        rs = (rs | common) - {0}
        edges[r], wild[r] = sorted(rs), w
        for x in rs:
            if x not in reached:
                reached.add(x)
                todo.append(x)
    rows = [json.loads(l) for l in (E003 / "results" / "groundtruth-part1.jsonl").read_text().splitlines() if l.strip()]
    scope = sorted({res(r[k]) for r in rows for k in ("room_pre", "room_done", "room_post") if r.get(k)} - {0})
    out = {"seed": a.seed, "start_globals": not a.no_start_globals, "start_room": r0, "reached": sorted(reached), "n_reached": len(reached),
           "part1_scope": scope, "n_part1_scope": len(scope),
           "scope_reached": sorted(set(scope) & reached), "scope_not_reached": sorted(set(scope) - reached),
           "reached_not_in_scope": sorted(reached - set(scope)),
           "room_changes_not_constant_in_reached_code": sum(wild[r] for r in reached) + common_wild,
           "edges": {str(k): v for k, v in sorted(edges.items())}}
    text = json.dumps(out, indent=1) + "\n"
    if a.out:
        Path(a.out).write_text(text)
    print(json.dumps({k: v for k, v in out.items() if k != "edges"}))


if __name__ == "__main__":
    main()
