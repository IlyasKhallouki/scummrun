# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Decompile every SCUMM v5 script block of MI1 (Mac) and build an index.

Driven by scripts/dump-scripts.sh. Inputs:
  * the XOR-0x69 index file (MONKEY1.000), parsed here for RNAM/DROO/DSCR,
    because scummrp does not export the index-file blocks;
  * the per-block dump made by `scummrp -o` (ScummTR) under --blocks;
  * the descumm binary from scummvm-tools.

scummrp exports each OBCD as one opaque file (CDHD + VERB + OBNA), while
descumm only accepts a bare VERB block, so this script slices the VERB
sub-block out of each OBCD (and reads the object name from OBNA).

Everything is written to <out>.tmp and swapped into <out> at the end.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import struct
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path

DESCUMM_FLAGS = ["-5", "-o"]

OFFSET_BASES = {
    "global": "byte offset from the first opcode, i.e. after the 8-byte SCRP header "
    "(ScummVM script PC = offset + 8)",
    "entry": "byte offset from the first opcode, i.e. after the 8-byte ENCD header",
    "exit": "byte offset from the first opcode, i.e. after the 8-byte EXCD header",
    "local": "byte offset from the first opcode, i.e. after the 9-byte LSCR header "
    "(8-byte header + 1-byte script number)",
    "verb": "byte offset from the start of the VERB block (header included), the same base "
    "as the Events table (ScummVM script PC = offset + verb_block_offset_in_obcd)",
}

# Lines descumm emits when it cannot decode something. Matched after string
# literals are blanked out, so game text cannot trigger them.
HARD_ERROR_RE = re.compile(
    rb"ERROR|\?\?(?:Var|Bit|Local)\?\?|Unknown\?\?|\bUnknown[0-9A-F]{2}\(\)"
)
SOFT_WARN_RE = re.compile(
    rb"\bunknown\d+\(|UnknownCursorCommand|SetBoxUnknown|UnknownWait|StringFuncUnknown"
)
STRING_LIT_RE = re.compile(rb'"[^"\n]*"')
LINE_OFFS_RE = re.compile(rb"^\[([0-9A-F]{4})\] \((..)\) ")
VERB_DEF_RE = re.compile(rb'VerbOps\((\d+),\[New\(\),Text\("([^"]*)"\)')


# ---------------------------------------------------------------- helpers


def slug(raw: str, maxlen: int = 40) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", raw.encode("ascii", "ignore").decode().lower()).strip("-")
    return s[:maxlen].rstrip("-") or "unnamed"


def ascii_safe(raw: bytes) -> str:
    return "".join(chr(b) if 0x20 <= b < 0x7F else f"\\x{b:02x}" for b in raw)


def iter_blocks(data: bytes, start: int = 0, end: int | None = None):
    """Yield (tag, offset, size) for consecutive big-endian SCUMM v5 blocks."""
    end = len(data) if end is None else end
    p = start
    while p + 8 <= end:
        tag = data[p : p + 4].decode("latin-1")
        size = struct.unpack(">I", data[p + 4 : p + 8])[0]
        if size < 8 or p + size > end:
            raise ValueError(f"bad block {tag!r} size {size} at {p}")
        yield tag, p, size
        p += size


# ---------------------------------------------------------------- index file


@dataclass
class GameIndex:
    room_names: dict[int, bytes]
    droo_rooms: set[int]
    dscr: dict[int, int]  # global script number -> room


def parse_index(path: Path) -> GameIndex:
    data = bytes(b ^ 0x69 for b in path.read_bytes())
    blocks = {tag: data[off + 8 : off + size] for tag, off, size in iter_blocks(data)}

    names: dict[int, bytes] = {}
    rnam = blocks["RNAM"]
    q = 0
    while rnam[q] != 0:
        room = rnam[q]
        names[room] = bytes(c ^ 0xFF for c in rnam[q + 1 : q + 10]).split(b"\0")[0]
        q += 10

    def directory(body: bytes) -> tuple[bytes, tuple[int, ...]]:
        n = struct.unpack("<H", body[:2])[0]
        return body[2 : 2 + n], struct.unpack(f"<{n}I", body[2 + n : 2 + 5 * n])

    droo_disk, _ = directory(blocks["DROO"])
    dscr_room, _ = directory(blocks["DSCR"])
    return GameIndex(
        room_names=names,
        droo_rooms={i for i, disk in enumerate(droo_disk) if disk},
        dscr={i: r for i, r in enumerate(dscr_room) if r},
    )


# ---------------------------------------------------------------- jobs


@dataclass
class Job:
    kind: str  # global | entry | exit | local | verb
    room: int
    number: int | None  # script number or object id
    block: Path  # file handed to descumm
    source: Path  # block as dumped by scummrp
    out_rel: str
    meta: dict = field(default_factory=dict)
    stdout: bytes = b""
    stderr: bytes = b""
    rc: int | None = None
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)


def parent_objects(data_file: Path) -> dict[int, tuple[int, int]]:
    """Per object with a parent, (the parent's object, the parent state), from the data file (XOR 0x69 like the index
    file). A CDHD's parent is a 1-based index into its room's objects in the order of the ROOM block's OBCD blocks
    (ScummVM's loadRoomObjects fills _objs in that order); the parent state is 1 when the flags byte is 0x80, else the
    flags' low four bits (resetRoomObject). findObject touches a child only while its parent is in that state."""
    data = bytes(b ^ 0x69 for b in data_file.read_bytes())
    out: dict[int, tuple[int, int]] = {}
    for tag, p, size in iter_blocks(data, 0):
        if tag != "LECF":
            continue
        for t2, q, _ in iter_blocks(data, p + 8, p + size):
            if t2 != "LOFF":
                continue
            for i in range(data[q + 8]):
                off = struct.unpack("<I", data[q + 10 + i * 5 : q + 14 + i * 5])[0]
                if data[off : off + 4] != b"ROOM":
                    continue
                order = []
                for t3, r, s3 in iter_blocks(data, off + 8, off + struct.unpack(">I", data[off + 4 : off + 8])[0]):
                    if t3 != "OBCD":
                        continue
                    for t4, c, _ in iter_blocks(data, r + 8, r + s3):
                        if t4 == "CDHD":
                            order.append((struct.unpack("<H", data[c + 8 : c + 10])[0], data[c + 15], data[c + 14]))
                for obj, parent, flags in order:
                    if 0 < parent <= len(order):
                        out[obj] = (order[parent - 1][0], 1 if flags == 0x80 else flags & 0xF)
            break
    return out


def add_parents(objects: dict, data_file: Path, problems: list[str]) -> None:
    """Each object with a parent gets "parent_object" and "parent_state" (`parent_objects`), when the data file
    reads."""
    try:
        parents = parent_objects(data_file)
    except (OSError, ValueError, struct.error) as e:
        problems.append(f"{data_file}: the objects' parents not read ({e})")
        return
    for obj, (po, ps) in parents.items():
        if obj in objects:
            objects[obj]["parent_object"] = po
            objects[obj]["parent_state"] = ps


def parse_obcd(path: Path) -> dict:
    data = path.read_bytes()
    if data[:4] != b"OBCD":
        raise ValueError(f"{path}: not an OBCD block")
    subs = {tag: (off, size) for tag, off, size in iter_blocks(data, 8)}
    cdhd_off, _ = subs["CDHD"]
    obj_id = struct.unpack("<H", data[cdhd_off + 8 : cdhd_off + 10])[0]
    parent = data[cdhd_off + 8 + 7]
    verb_off, verb_size = subs["VERB"]
    verb = data[verb_off : verb_off + verb_size]
    entries = []
    p = 8
    seen: set[int] = set()
    fallback_seen = False
    while verb[p] != 0:
        code = verb[p]
        offs = struct.unpack("<H", verb[p + 1 : p + 3])[0]
        # ScummVM picks the first entry whose code matches or is 0xFF.
        shadowed = fallback_seen or code in seen
        entries.append({"verb": code, "offset": offs, "shadowed": shadowed})
        seen.add(code)
        fallback_seen |= code == 0xFF
        p += 3
    name_raw = b""
    if "OBNA" in subs:
        o, s = subs["OBNA"]
        name_raw = data[o + 8 : o + s].split(b"\0")[0].rstrip(b"@")
    return {
        "id": obj_id,
        "parent": parent,
        "verb_block": verb,
        "verb_block_offset_in_obcd": verb_off,
        "verb_entries": entries,
        "name_raw": name_raw,
    }


def collect_jobs(blocks_root: Path, verb_dir: Path, index: GameIndex, problems: list[str]):
    lflfs = sorted(blocks_root.glob("DISK_*/LECF/LFLF_*"))
    if not lflfs:
        raise SystemExit(f"no LFLF directories under {blocks_root}")
    jobs: list[Job] = []
    rooms: dict[int, dict] = {}
    objects: dict[int, dict] = {}
    globals_: dict[int, dict] = {}

    for lflf in lflfs:
        room = int(lflf.name.split("_")[1])
        name_raw = index.room_names.get(room)
        if name_raw is None:
            problems.append(f"LFLF {room} has no RNAM entry")
            name_raw = b""
        if room not in index.droo_rooms:
            problems.append(f"LFLF {room} is not listed in DROO")
        rname = name_raw.decode("latin-1")
        rdir = f"room-{room:03d}-{slug(rname)}"
        rinfo = rooms[room] = {
            "name": rname,
            "dir": rdir,
            "lflf": str(lflf),
            "entry": None,
            "exit": None,
            "local_scripts": {},
            "objects": {},
            "global_scripts_stored_here": [],
        }

        for scrp in sorted(lflf.glob("SCRP_*")):
            num = int(scrp.name.split("_")[1])
            if num in globals_:
                problems.append(f"global script {num} dumped twice")
            out = f"global/script-{num:03d}.txt"
            globals_[num] = {"file": out, "room": room, "room_name": rname, "block": str(scrp)}
            rinfo["global_scripts_stored_here"].append(num)
            jobs.append(Job("global", room, num, scrp, scrp, out))

        roomdir = lflf / "ROOM"
        for kind, tag in (("entry", "ENCD"), ("exit", "EXCD")):
            blk = roomdir / tag
            if blk.exists():
                out = f"{rdir}/{kind}.txt"
                rinfo[kind] = {"file": out}
                jobs.append(Job(kind, room, None, blk, blk, out))
            else:
                problems.append(f"room {room} has no {tag}")

        for lscr in sorted(roomdir.glob("LSCR_*")):
            num = int(lscr.name.split("_")[1])
            out = f"{rdir}/local-{num:03d}.txt"
            rinfo["local_scripts"][str(num)] = {"file": out}
            jobs.append(Job("local", room, num, lscr, lscr, out))

        for obcd in sorted(roomdir.glob("OBCD_*")):
            num = int(obcd.name.split("_")[1])
            ob = parse_obcd(obcd)
            if ob["id"] != num:
                problems.append(f"{obcd}: CDHD id {ob['id']} != file name id {num}")
            if num in objects:
                problems.append(f"object {num} appears in rooms {objects[num]['room']} and {room}")
            oname = ob["name_raw"].decode("latin-1")
            out = f"{rdir}/obj-{num:04d}-{slug(oname)}.txt"
            vb = verb_dir / f"room-{room:03d}" / f"VERB_{num:04d}"
            vb.parent.mkdir(parents=True, exist_ok=True)
            vb.write_bytes(ob["verb_block"])
            entries = ob["verb_entries"]
            objects[num] = {
                "name": oname,
                "name_hex": ob["name_raw"].hex(),
                "room": room,
                "room_name": rname,
                "file": out,
                "parent": ob["parent"],
                "block": str(obcd),
                "verb_block": str(vb),
                "verb_block_offset_in_obcd": ob["verb_block_offset_in_obcd"],
                "verbs": [
                    {"verb": e["verb"], "offset": f"{e['offset']:04X}", "shadowed": e["shadowed"]}
                    for e in entries
                ],
            }
            rinfo["objects"][str(num)] = {"name": oname, "file": out}
            jobs.append(Job("verb", room, num, vb, obcd, out, meta={"entries": entries, "object": ob}))

    # Completeness against the index directory of global scripts.
    for num, room in sorted(index.dscr.items()):
        g = globals_.get(num)
        if g is None:
            problems.append(f"DSCR lists global script {num} in room {room} but it was not dumped")
        elif g["room"] != room:
            problems.append(f"global script {num}: DSCR says room {room}, dump has LFLF {g['room']}")
    for num in globals_:
        if num not in index.dscr:
            problems.append(f"global script {num} dumped but absent from DSCR")
    for room in sorted(set(index.room_names) - set(rooms)):
        problems.append(f"RNAM room {room} has no LFLF in the dump")

    return jobs, rooms, objects, globals_


# ---------------------------------------------------------------- descumm


def run_descumm(descumm: str, job: Job) -> Job:
    if job.kind == "verb" and not job.meta["entries"]:
        job.rc = None  # nothing to decompile: empty verb table
        return job
    proc = subprocess.run([descumm, *DESCUMM_FLAGS, str(job.block)], capture_output=True)
    job.rc, job.stdout, job.stderr = proc.returncode, proc.stdout, proc.stderr
    return job


def check(job: Job) -> None:
    if job.kind == "verb" and not job.meta["entries"]:
        return
    if job.rc != 0:
        job.errors.append(f"descumm exit code {job.rc}")
    if job.stderr.strip():
        job.errors.append("stderr: " + job.stderr.decode("latin-1").strip()[:300])
    lines = job.stdout.split(b"\n")
    if lines and lines[-1] == b"":
        lines.pop()
    if not lines or lines[-1] != b"END":
        job.errors.append("output does not end with END")
    for ln in lines:
        bare = STRING_LIT_RE.sub(b'""', ln)
        if HARD_ERROR_RE.search(bare):
            job.errors.append("decode error: " + ln.decode("latin-1")[:200])
        elif SOFT_WARN_RE.search(ln):
            job.warnings.append("unknown code: " + ln.decode("latin-1")[:200])
    if job.kind == "verb":
        line_offs = {int(m.group(1), 16) for ln in lines if (m := LINE_OFFS_RE.match(ln))}
        entries = job.meta["entries"]
        first = min(e["offset"] for e in entries)
        if first > 255:
            job.errors.append("smallest verb offset > 0xFF: descumm starts decoding at 0xFF")
        table_end = 8 + 3 * len(entries) + 1
        if first != table_end:
            # descumm decodes from the smallest entry offset; bytes in between are skipped.
            job.errors.append(f"code starts at {first:04X} but verb table ends at {table_end:04X}")
        for e in entries:
            if e["offset"] not in line_offs:
                job.errors.append(
                    f"verb {e['verb']} entry offset {e['offset']:04X} is not a decoded line start"
                )


# ---------------------------------------------------------------- output


def verb_label(code: int, legend: dict[int, dict]) -> str:
    if code == 0xFF:
        return "default/any verb"
    v = legend.get(code)
    return v["name"] if v else "unnamed"


def render(job: Job, rooms: dict, objects: dict, legend: dict[int, dict], title: str | None = None) -> bytes:
    room = rooms[job.room]
    hdr = [
        f"# {title}" if title else "# The Secret of Monkey Island (Mac, SCUMM v5)",
        f"# decompiled with: descumm {' '.join(DESCUMM_FLAGS)} {job.block.name}",
    ]
    if job.kind == "global":
        hdr.append(f"# global script {job.number} (stored in LFLF of room {job.room} '{room['name']}')")
    elif job.kind in ("entry", "exit"):
        hdr.append(f"# room {job.room} '{room['name']}' {job.kind} script ({job.source.name})")
    elif job.kind == "local":
        hdr.append(f"# room {job.room} '{room['name']}' local script {job.number}")
    else:
        ob = objects[job.number]
        hdr.append(
            f"# object {job.number} \"{ascii_safe(bytes.fromhex(ob['name_hex']))}\" "
            f"in room {job.room} '{room['name']}' (parent object: {ob['parent']})"
        )
        hdr.append(f"# VERB block at +0x{ob['verb_block_offset_in_obcd']:X} within OBCD")
        if job.meta["entries"]:
            hdr.append("# verb entry points (ScummVM uses the first entry matching the verb or 0xFF):")
            for e in job.meta["entries"]:
                sh = "  [shadowed by an earlier entry]" if e["shadowed"] else ""
                hdr.append(
                    f"#   verb {e['verb']:3d} (0x{e['verb']:02X}) {verb_label(e['verb'], legend)!s:<16}"
                    f" @ {e['offset']:04X}{sh}"
                )
        else:
            hdr.append("# verb table is empty: object has no verb scripts")
    hdr.append(f"# source block: {job.source}")
    hdr.append(f"# offsets: [XXXX] = {OFFSET_BASES[job.kind]}")
    status = "error" if job.errors else ("warning" if job.warnings else "ok")
    hdr.append(f"# status: {status}")
    for e in job.errors:
        hdr.append(f"#   error: {e}")
    hdr.append("#")
    head = ("\n".join(hdr) + "\n").encode("ascii")

    if job.kind != "verb" or not job.meta["entries"]:
        return head + job.stdout

    # Insert '#' marker lines before each verb entry point; descumm lines stay verbatim.
    markers: dict[int, list[str]] = {}
    for e in job.meta["entries"]:
        sh = " [shadowed by an earlier entry]" if e["shadowed"] else ""
        markers.setdefault(e["offset"], []).append(
            f"# ---- verb {e['verb']} ({verb_label(e['verb'], legend)}) entry @ {e['offset']:04X}{sh} ----"
        )
    out: list[bytes] = []
    for ln in job.stdout.split(b"\n"):
        m = LINE_OFFS_RE.match(ln)
        if m and m.group(2) != b"**":
            offs = int(m.group(1), 16)
            for mk in markers.pop(offs, []):
                out.append(mk.encode("ascii"))
        out.append(ln)
    return head + b"\n".join(out)


def build_legend(jobs: list[Job]) -> dict[int, dict]:
    legend: dict[int, dict] = {}
    for job in sorted((j for j in jobs if j.kind == "global"), key=lambda j: j.number):
        for ln in job.stdout.split(b"\n"):
            m = VERB_DEF_RE.search(ln)
            if not m:
                continue
            vid = int(m.group(1))
            if vid in legend:
                continue
            om = LINE_OFFS_RE.match(ln)
            legend[vid] = {
                "name": m.group(2).decode("latin-1"),
                "defined_in": job.out_rel,
                "offset": om.group(1).decode() if om else None,
            }
    return legend


def md_escape(s: str) -> str:
    return s.replace("|", "\\|")


def write_index_md(path: Path, idx: dict, title: str | None = None) -> None:
    c = idx["counts"]
    L = [
        f"# {title}: decompiled scripts index" if title else "# MI1 (Mac) decompiled scripts index",
        "",
        "Generated by `scripts/dump-scripts.sh`. Do not edit by hand.",
        "",
        "## Tools",
        "",
    ]
    for name, t in idx["tools"].items():
        L.append(f"- **{name}**: " + ", ".join(f"{k}=`{v}`" for k, v in t.items()))
    L += ["", "## Counts", ""]
    for k, v in c.items():
        L.append(f"- {k}: {v}")
    L += ["", "## Offsets (citation key)", "",
          "Every descumm code line starts with `[XXXX] (OP)`: `XXXX` is the hex byte offset of the "
          "instruction and `OP` its opcode. Cite scripts as *file + offset*; line numbers shift "
          "with the `#` header. Lines starting with `#` are added by this pipeline, everything "
          "else is verbatim descumm output. Offset base per script kind:", ""]
    for k, v in OFFSET_BASES.items():
        L.append(f"- **{k}**: {v}")
    L += ["", "## Status", ""]
    if idx["problems"]:
        L.append("Index/dump consistency problems:")
        L += [f"- {p}" for p in idx["problems"]]
    else:
        L.append("Dump is consistent with the index file (RNAM/DROO/DSCR): no problems.")
    L.append("")
    if idx["failures"]:
        L.append("descumm failures:")
        L += [f"- `{f['file']}`: {'; '.join(f['errors'])}" for f in idx["failures"]]
    else:
        L.append("descumm failures: none.")
    L.append("")
    if idx["warnings"]:
        L.append("Files with descumm warnings (undecoded string escape codes and similar, not fatal):")
        L += [f"- `{w['file']}`: {len(w['warnings'])} line(s)" for w in idx["warnings"]]
    L += ["", "## Verb legend", "",
          "From `VerbOps(N,[New(),Text(...)])` in global scripts (first definition wins). "
          "0xFF in an object's verb table is the fallback entry.", "",
          "| verb | name | defined in |", "|---:|---|---|"]
    for vid, v in sorted(idx["verbs"].items(), key=lambda kv: int(kv[0])):
        L.append(f"| {vid} | {md_escape(v['name'])} | `{v['defined_in']}` @ {v['offset']} |")
    L += ["", "## Rooms", "",
          "| room | name | dir | local scripts | objects | global scripts stored here |",
          "|---:|---|---|---|---:|---|"]
    for rid, r in sorted(idx["rooms"].items(), key=lambda kv: int(kv[0])):
        locs = ", ".join(sorted(r["local_scripts"], key=int)) or "-"
        gl = ", ".join(str(g) for g in r["global_scripts_stored_here"]) or "-"
        L.append(f"| {rid} | {md_escape(r['name'])} | `{r['dir']}/` | {locs} | {len(r['objects'])} | {gl} |")
    L += ["", "## Global scripts", "", "| script | stored in room | file |", "|---:|---|---|"]
    for sid, g in sorted(idx["global_scripts"].items(), key=lambda kv: int(kv[0])):
        L.append(f"| {sid} | {g['room']} ({md_escape(g['room_name'])}) | `{g['file']}` |")
    L += ["", "## Objects", "", "| object | name | room | verbs | file |", "|---:|---|---|---|---|"]
    for oid, o in sorted(idx["objects"].items(), key=lambda kv: int(kv[0])):
        verbs = ", ".join(
            (idx["verbs"].get(str(v["verb"]), {}).get("name") or ("fallback" if v["verb"] == 255 else str(v["verb"])))
            + ("(shadowed)" if v["shadowed"] else "")
            for v in o["verbs"]
        ) or "-"
        L.append(
            f"| {oid} | {md_escape(ascii_safe(bytes.fromhex(o['name_hex'])))} | {o['room']} ({md_escape(o['room_name'])}) "
            f"| {md_escape(verbs)} | `{o['file']}` |"
        )
    path.write_text("\n".join(L) + "\n", encoding="utf-8")


def parents_into(argv: list[str]) -> int:
    """`dump_scripts.py --parents-into INDEX_JSON --data-file F`: `add_parents` on an index.json made before."""
    ap = argparse.ArgumentParser()
    ap.add_argument("--parents-into", type=Path, required=True)
    ap.add_argument("--data-file", type=Path, required=True)
    a = ap.parse_args(argv)
    idx = json.loads(a.parents_into.read_text())
    objects = {int(k): v for k, v in idx["objects"].items()}
    problems: list[str] = []
    add_parents(objects, a.data_file, problems)
    idx["objects"] = {str(k): v for k, v in sorted(objects.items())}
    a.parents_into.write_text(json.dumps(idx, indent=1) + "\n")
    n = sum(1 for o in objects.values() if "parent_object" in o)
    print(f"{a.parents_into}: {n} objects with a parent object; problems {problems}")
    return 1 if problems else 0


def main() -> int:
    if "--parents-into" in sys.argv:
        return parents_into(sys.argv[1:])
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--index-file", type=Path, required=True)
    ap.add_argument("--blocks", type=Path, required=True)
    ap.add_argument("--verb-dir", type=Path, required=True)
    ap.add_argument("--descumm", required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--tool-info", type=Path, help="JSON file with tool versions to embed")
    # E014: another game's title for the headers and the index (unset: MI's text, as before)
    ap.add_argument("--title", help="the game's title in each file's header and in the index")
    args = ap.parse_args()

    index = parse_index(args.index_file)
    problems: list[str] = []
    if args.verb_dir.exists():
        shutil.rmtree(args.verb_dir)
    jobs, rooms, objects, globals_ = collect_jobs(args.blocks, args.verb_dir, index, problems)
    # E022 amendment (12): the objects' parents as objects, with the state they need (the data file next to the index)
    data_file = args.index_file.with_suffix(".001")
    if data_file.exists():
        add_parents(objects, data_file, problems)

    with ThreadPoolExecutor(max_workers=args.jobs) as ex:
        jobs = list(ex.map(lambda j: run_descumm(args.descumm, j), jobs))
    for j in jobs:
        check(j)
    legend = build_legend(jobs)

    tmp = args.out.with_name(args.out.name + ".tmp")
    if tmp.exists():
        shutil.rmtree(tmp)
    tmp.mkdir(parents=True)
    for j in jobs:
        dest = tmp / j.out_rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(render(j, rooms, objects, legend, args.title))

    failures = [{"file": j.out_rel, "errors": j.errors} for j in jobs if j.errors]
    warnings = [{"file": j.out_rel, "warnings": j.warnings} for j in jobs if j.warnings]
    status = {j.out_rel: ("error" if j.errors else "warning" if j.warnings else "ok") for j in jobs}
    for g in globals_.values():
        g["status"] = status[g["file"]]
    for o in objects.values():
        o["status"] = "no-verbs" if not o["verbs"] else status[o["file"]]
    for r in rooms.values():
        for ref in [r["entry"], r["exit"], *r["local_scripts"].values()]:
            if ref:
                ref["status"] = status[ref["file"]]
        for oid, ref in r["objects"].items():
            ref["status"] = objects[int(oid)]["status"]

    counts = {
        "rooms": len(rooms),
        "global_scripts": len(globals_),
        "entry_scripts": sum(1 for j in jobs if j.kind == "entry"),
        "exit_scripts": sum(1 for j in jobs if j.kind == "exit"),
        "local_scripts": sum(1 for j in jobs if j.kind == "local"),
        "objects": len(objects),
        "objects_with_verb_scripts": sum(1 for o in objects.values() if o["verbs"]),
        "verb_entry_points": sum(len(o["verbs"]) for o in objects.values()),
        "descumm_runs": sum(1 for j in jobs if j.rc is not None),
        "files_ok": sum(1 for s in status.values() if s == "ok"),
        "files_with_warnings": len(warnings),
        "files_with_errors": len(failures),
    }
    tools = json.loads(args.tool_info.read_text()) if args.tool_info else {}
    tools.setdefault("descumm", {})["flags"] = " ".join(DESCUMM_FLAGS)
    game = {"title": "The Secret of Monkey Island", "platform": "Mac", "scumm_version": 5,
            "index_file": str(args.index_file)}
    if args.title:
        game = {"title": args.title, "scumm_version": 5, "index_file": str(args.index_file)}
    idx = {
        "game": game,
        "tools": tools,
        "offset_bases": OFFSET_BASES,
        "counts": counts,
        "problems": problems,
        "failures": failures,
        "warnings": warnings,
        "verbs": {str(k): v for k, v in sorted(legend.items())},
        "rooms": {str(k): v for k, v in sorted(rooms.items())},
        "global_scripts": {str(k): v for k, v in sorted(globals_.items())},
        "objects": {str(k): v for k, v in sorted(objects.items())},
    }
    (tmp / "index.json").write_text(json.dumps(idx, indent=1) + "\n", encoding="utf-8")
    write_index_md(tmp / "INDEX.md", idx, args.title)

    old = args.out.with_name(args.out.name + ".old")
    if old.exists():
        shutil.rmtree(old)
    if args.out.exists():
        os.replace(args.out, old)
    os.replace(tmp, args.out)
    if old.exists():
        shutil.rmtree(old)

    print(json.dumps(counts, indent=1))
    for p in problems:
        print(f"PROBLEM: {p}", file=sys.stderr)
    for f in failures:
        print(f"FAILED: {f['file']}: {'; '.join(f['errors'])}", file=sys.stderr)
    return 1 if (failures or problems) else 0


if __name__ == "__main__":
    sys.exit(main())
