"""Probe: which units install an input script (VAR_VERB_SCRIPT = const), and what room-local input scripts test.

Run from the E003 folder: ../../.venv/bin/python <this file>
Prints counts only (no game text)."""
import collections
import sys

sys.path.insert(0, ".")
import ir
from ir import Assign, Bit, Cmp, Const, Getter, Index, Jump, Local, Not, Op, Start, Truth, Var

VAR_VERB_SCRIPT = 32
VAR_SENTENCE_SCRIPT = 33
units = ir.load()
local_rooms = collections.defaultdict(set)
for k in units:
    if k[0] == "local":
        local_rooms[k[2]].add(k[1])
first_local = min(local_rooms)
print("first_local", first_local, "units", len(units))

installs = collections.defaultdict(set)  # value -> units
for key, u in units.items():
    for x in u.ins:
        st = x.st
        if isinstance(st, Assign) and st.lhs == Var(VAR_VERB_SCRIPT) and st.op == "=" and isinstance(st.rhs, Const):
            installs[st.rhs.value].add(key)
glob = sorted(v for v in installs if 0 < v < first_local)
loc = sorted(v for v in installs if v >= first_local)
print("global input scripts installed:", glob)
print("local numbers installed:", loc)

# room-local input scripts per room: an install from the room's own code (entry/exit/local of room r) reaches
# local N of room r; an install from global/object code reaches local N of whichever room is in use
per_room = collections.defaultdict(set)
from_global = collections.defaultdict(set)
for v in loc:
    for key in installs[v]:
        if key[0] in ("entry", "exit", "local"):
            if ("local", key[1], v) in units:
                per_room[key[1]].add(v)
            else:
                print("  local", v, "installed by", key, "but room lacks it")
        else:
            from_global[v].add(key)
print("rooms whose own code installs a local input script:", len(per_room), dict(sorted(per_room.items())))
print("local numbers installed from global/object code:", {v: sorted(map(str, ks)) for v, ks in from_global.items()})
for v, ks in from_global.items():
    print("   rooms with local", v, ":", sorted(local_rooms.get(v, ())))
rooms = sorted({k[1] for k in units if k[0] == "entry"})
print("rooms:", len(rooms))


def reads(e, out):
    if isinstance(e, (Var, Bit, Local)):
        out.add(e)
    elif isinstance(e, Index):
        out.add(e)
        reads(e.offset, out)
    elif isinstance(e, Cmp):
        reads(e.a, out); reads(e.b, out)
    elif isinstance(e, (Not, Truth)):
        reads(e.x, out)
    elif isinstance(e, Getter):
        out.add(("getter", e.name))
        for a in e.args:
            reads(a, out)
    elif hasattr(e, "a") and hasattr(e, "b"):
        reads(e.a, out); reads(e.b, out)
    elif hasattr(e, "items"):
        for a in e.items:
            reads(a, out)


# what each room-local input script does: chains to a global input script? compares a var with object consts?
objs_by_room = collections.defaultdict(set)
for k, u in units.items():
    if k[0] == "obj":
        objs_by_room[u.room].add(k[1])
print()
tot_obj = 0
for r in sorted(per_room):
    for v in sorted(per_room[r]):
        u = units[("local", r, v)]
        chains, stops, sentences, starts, cmps_obj, cmp_vars, getters = [], 0, 0, [], set(), set(), set()
        for x in u.ins:
            st = x.st
            if isinstance(st, Start):
                if st.name == "chainScript":
                    chains.append(st.script.value if isinstance(st.script, Const) else "?")
                else:
                    starts.append((st.name, st.script.value if isinstance(st.script, Const) else "?"))
            if isinstance(st, Op) and st.name == "doSentence":
                sentences += 1
            if isinstance(st, Jump) and st.cond is not None:
                c = st.cond
                if isinstance(c, Cmp):
                    for l, rr in ((c.a, c.b), (c.b, c.a)):
                        if isinstance(rr, Const) and rr.value in objs_by_room[r] and isinstance(l, (Var, Local)):
                            cmps_obj.add((l, rr.value))
                s = set()
                reads(c, s)
                cmp_vars |= {e for e in s if isinstance(e, (Var, Bit))}
                getters |= {e[1] for e in s if isinstance(e, tuple)}
            if isinstance(st, Assign) and isinstance(st.rhs, Getter):
                getters.add(st.rhs.name)
        tot_obj += len({o for _, o in cmps_obj})
        print(f"room {r} local {v}: ins {len(u.ins)} chain->{chains} starts {starts[:8]} doSentence {sentences} "
              f"obj-tests {sorted((str(l), o) for l, o in cmps_obj)} getters {sorted(getters)} "
              f"tested-vars {sorted(str(e) for e in cmp_vars)[:12]}")
print("objects named in an object-constant test by a room-local input script:", tot_obj)

# chains of local scripts the room-local input script hands off to (one level)
print()
gl = set(glob)
for r in sorted(per_room):
    for v in sorted(per_room[r]):
        u = units[("local", r, v)]
        for x in u.ins:
            st = x.st
            if isinstance(st, Start) and st.name == "chainScript" and isinstance(st.script, Const) \
                    and st.script.value >= first_local:
                w = units.get(("local", r, st.script.value))
                if w is None:
                    continue
                ch = [y.st.script.value for y in w.ins if isinstance(y.st, Start) and y.st.name == "chainScript"
                      and isinstance(y.st.script, Const)]
                ends = sum(1 for y in w.ins if y.ends == "always")
                print(f"  room {r}: local {v} chains to local {st.script.value}: its chains {ch}, ends {ends}")
