"""Probe: the cells of each room's object-box arrangement (the distinct non-empty sets of CDHD boxes that hold a
point, object.cpp:561-593 tests x_pos <= x < x_pos + width, same for y). Boxes are on the 8-pixel grid, so a cell
is a set of grid squares. Counts only. Run from the E003 folder."""
import collections
import json
import pathlib

ROOT = pathlib.Path("../..")
meta = json.loads((ROOT / "data/scripts-dos-cd/index.json").read_text())
rooms = collections.defaultdict(list)
for o, v in meta["objects"].items():
    p = ROOT / v["block"]
    if not p.is_file():
        continue
    b = p.read_bytes()
    i = b.find(b"CDHD")
    if i < 0:
        continue
    d = b[i + 8:i + 8 + 13]
    x, y, w, h = d[2], d[3], d[4], d[5]
    if w and h:
        rooms[v["room"]].append((int(o), x, y, w, h))
tot_obj = tot_cells = 0
multi = 0
shadowed = 0
per = {}
for r, objs in rooms.items():
    cells = collections.Counter()
    for gx in range(0, 256):
        for gy in range(0, 64):
            s = frozenset(o for o, x, y, w, h in objs if x <= gx < x + w and y <= gy < y + h)
            if s:
                cells[s] += 1
    per[r] = (len(objs), len(cells), sum(1 for s in cells if len(s) > 1))
    tot_obj += len(objs)
    tot_cells += len(cells)
    multi += per[r][2]
print("rooms", len(rooms), "objects with a box", tot_obj, "cells", tot_cells, "cells under 2+ boxes", multi)
for r in (28, 41, 33):
    print("room", r, "objects, cells, cells under 2+ boxes:", per.get(r))
print("max cells in a room:", max(per.items(), key=lambda kv: kv[1][1]))
