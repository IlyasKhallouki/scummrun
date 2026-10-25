"""Probe: objects that share a box (x, y, w, h from CDHD, object.cpp:1145-1150) in their room, and drawObject sites
whose object shares its box. Run from the E003 folder. Prints counts only."""
import collections
import json
import pathlib
import sys

sys.path.insert(0, ".")
import ir
from ir import Const, Op

ROOT = pathlib.Path("../..")
meta = json.loads((ROOT / "data/scripts-dos-cd/index.json").read_text())
boxes = {}
for o, v in meta["objects"].items():
    p = ROOT / v["block"]
    if not p.is_file():
        continue
    b = p.read_bytes()
    i = b.find(b"CDHD")
    if i < 0:
        continue
    d = b[i + 8:i + 8 + 13]
    obj = int.from_bytes(d[0:2], "little")
    x, y, w, h = d[2] * 8, d[3] * 8, d[4] * 8, d[5] * 8
    boxes[int(o)] = (v["room"], x, y, w, h)
print("objects with a box:", len(boxes))
groups = collections.defaultdict(list)
for o, (r, x, y, w, h) in boxes.items():
    groups[(r, x, y, w, h)].append(o)
shared = {k: v for k, v in groups.items() if len(v) > 1}
print("boxes shared by 2+ objects:", len(shared), "objects in them:", sum(len(v) for v in shared.values()),
      "rooms:", len({k[0] for k in shared}))
sharing = {o for v in shared.values() for o in v}

units = ir.load()
sites = sites_shared = setxy = nonconst = 0
objs_drawn_shared = set()
for key, u in units.items():
    for x in u.ins:
        st = x.st
        if isinstance(st, Op) and st.name == "drawObject":
            sites += 1
            if any(s.name == "setXY" for s in st.subops):
                setxy += 1
            a = st.args[0] if st.args else None
            if not isinstance(a, Const):
                nonconst += 1
                continue
            if a.value in sharing:
                sites_shared += 1
                objs_drawn_shared.add(a.value)
print("drawObject sites:", sites, "with setXY:", setxy, "object not a constant:", nonconst,
      "on an object that shares its box:", sites_shared, "objects:", len(objs_drawn_shared))

# positions an object can have: its CDHD place plus each constant setXY target (o5_drawObject, script_v5.cpp:1108-1114)
places = {o: {(x, y)} for o, (r, x, y, w, h) in boxes.items()}
draws = []
for key, u in units.items():
    for x in u.ins:
        st = x.st
        if isinstance(st, Op) and st.name == "drawObject" and st.args and isinstance(st.args[0], Const):
            o = st.args[0].value
            if o not in boxes:
                continue
            tgt = None
            for s in st.subops:
                if s.name == "setXY" and all(isinstance(t, Const) for t in s.args):
                    tgt = (s.args[0].value * 8, s.args[1].value * 8)
                    places[o].add(tgt)
            draws.append((o, tgt))
byroom = collections.defaultdict(list)
for o, (r, x, y, w, h) in boxes.items():
    byroom[(r, w, h)].append(o)
static_clear = may_clear = 0
pairs = set()
for o, tgt in draws:
    r, x, y, w, h = boxes[o]
    at = {tgt} if tgt else places[o]
    others = [p for p in byroom[(r, w, h)] if p != o and places[p] & at]
    if others:
        may_clear += 1
        pairs |= {(o, p) for p in others}
        if all(len(places[p]) == 1 for p in others) and len(at) == 1:
            static_clear += 1
moved = sum(1 for o in places if len(places[o]) > 1)
print("objects with more than one place:", moved)
print("drawObject sites (constant object) that may clear another object:", may_clear, "of", len(draws),
      "decided from constants alone:", static_clear, "object pairs:", len(pairs))
