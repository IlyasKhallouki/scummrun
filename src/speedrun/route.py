"""The Any% route files (``routes/anypercent/S*.json``) as executor moves.

Each segment file lists moves in play order, with the fields of
``executor.Move`` plus documentation (``kind``, ``glitch``, ``cite``,
``note``). S1 and S2 are one stretch of play: S2's moves slot into S1 in the
order ``routes/anypercent/README.md`` ("S1 and S2: the merged order") gives,
by 1-based position in each file.
"""

import json
from pathlib import Path

from speedrun import paths
from speedrun.executor import Move

ROUTE_DIR = paths.ROOT / "routes" / "anypercent"

SEGMENTS = {
    "S1": "S1-trials.json",
    "S2": "S2-sword.json",
    "S3": "S3-crew-slide.json",
    "S4": "S4-part2.json",
    "S5": "S5-part3-island.json",
    "S6": "S6-catacombs-ghostship.json",
    "S7": "S7-part4.json",
}

# (segment, first, last), 1-based and inclusive.
MERGED_S1_S2 = [
    ("S1", 1, 8), ("S2", 1, 4), ("S1", 9, 51), ("S2", 5, 5), ("S1", 52, 53), ("S2", 6, 7),
    ("S1", 54, 55), ("S2", 8, 8), ("S1", 56, 83), ("S2", 9, 28), ("S1", 84, 96), ("S2", 29, 59),
]

_DOC_FIELDS = {"room", "kind", "glitch", "cite", "note"}
_MOVE_FIELDS = {"name", "key", "verb", "obj", "obj2", "point", "choose", "after", "until", "max_frames",
                "obj_from_var", "skips", "delay", "choose_by", "press", "auto_skips", "optional", "gate", "approach_y",
                "rclick", "via", "approach", "reclick", "reclick_sweep", "approach_hold", "shmoove", "no_early"}


class RouteFileError(Exception):
    pass


def to_move(raw: dict, label: str) -> Move:
    unknown = set(raw) - _DOC_FIELDS - _MOVE_FIELDS
    if unknown:
        raise RouteFileError(f"{label}: unknown fields {sorted(unknown)}")
    kw = {k: v for k, v in raw.items() if k in _MOVE_FIELDS and v is not None}
    if "point" in kw:
        kw["point"] = tuple(kw["point"])
    if "via" in kw:
        kw["via"] = tuple(kw["via"])
    kw["name"] = f"{label} {raw['name']}"
    return Move(**kw)


def load_segment(name: str, route_dir: Path = ROUTE_DIR) -> list[Move]:
    """A segment's moves, each named ``<segment>#<1-based position> <name>``."""
    data = json.loads((route_dir / SEGMENTS[name]).read_text())
    return [to_move(m, f"{name}#{i}") for i, m in enumerate(data["moves"], 1)]


def merged_s1_s2(route_dir: Path = ROUTE_DIR) -> list[Move]:
    segs = {s: load_segment(s, route_dir) for s in ("S1", "S2")}
    moves = [m for seg, a, b in MERGED_S1_S2 for m in segs[seg][a - 1:b]]
    if len(moves) != sum(len(v) for v in segs.values()):
        raise RouteFileError(f"the S1/S2 interleave covers {len(moves)} moves, the files hold "
                             f"{sum(len(v) for v in segs.values())}")
    return moves


def full_route(route_dir: Path = ROUTE_DIR) -> list[Move]:
    moves = merged_s1_s2(route_dir)
    for seg in ("S3", "S4", "S5", "S6", "S7"):
        moves += load_segment(seg, route_dir)
    return moves
