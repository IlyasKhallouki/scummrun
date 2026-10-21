# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Decompile every SCUMM v4 script block of MI1 (DOS VGA floppy) and build an index.

Driven by scripts/dump-scripts.sh when the game directory holds the floppy layout
(000.LFL + DISKnn.LEC). Inputs:
  * the unencrypted index file 000.LFL, parsed here for RN (room names) and 0S
    (the global script directory);
  * the per-block dump made by `scummrp -g monkey -o` under --blocks
    (DISK_nnnn/LE/LF_rrrr/{SC_nnnn, RO/{EN, EX, LS_nnn, OC_nnnn}});
  * the descumm binary from scummvm-tools, run with -4.

v4 blocks have 6-byte small headers (LE32 size + 2-char tag). descumm accepts every
script block as dumped, including OC (object code), whose verb table it prints as
"Events:". The output tree mirrors data/scripts/ (the Mac v5 dump):
global/script-NNN.txt, room-NNN-name/{entry,exit,local-NNN,obj-NNNN-name}.txt.

Everything is written to <out>.tmp and swapped into <out> at the end.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import struct
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

DESCUMM_FLAGS = ["-4", "-o"]

OFFSET_BASES = {
    "global": "byte offset from the first opcode, i.e. after the 6-byte SC header "
    "(ScummVM script PC = offset + 6)",
    "entry": "byte offset from the first opcode, i.e. after the 6-byte EN header",
    "exit": "byte offset from the first opcode, i.e. after the 6-byte EX header",
    "local": "byte offset from the first opcode, i.e. after the 7-byte LS header "
    "(6-byte header + 1-byte script number)",
    "verb": "byte offset from the start of the OC block (header included), the same base "
    "as the Events table and ScummVM's verb script PC",
}

HARD_ERROR_RE = re.compile(rb"ERROR|\?\?(?:Var|Bit|Local)\?\?|Unknown\?\?|\bUnknown[0-9A-F]{2}\(\)")
STRING_LIT_RE = re.compile(rb'"[^"\n]*"')


def slug(raw: str, maxlen: int = 40) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", raw.encode("ascii", "ignore").decode().lower()).strip("-")
    return s[:maxlen].rstrip("-") or "unnamed"


def small_blocks(data: bytes):
    p = 0
    while p + 6 <= len(data):
        size, tag = struct.unpack("<I2s", data[p : p + 6])
        if size < 6 or p + size > len(data):
            raise ValueError(f"bad block {tag!r} size {size} at {p}")
        yield tag.decode("latin-1"), p, size
        p += size


def parse_index(path: Path) -> tuple[dict[int, str], dict[int, int]]:
    data = path.read_bytes()
    blocks = {tag: data[off + 6 : off + size] for tag, off, size in small_blocks(data)}
    names: dict[int, str] = {}
    rn = blocks["RN"]
    q = 0
    while rn[q] != 0:
        names[rn[q]] = bytes(c ^ 0xFF for c in rn[q + 1 : q + 10]).split(b"\0")[0].decode("latin-1")
        q += 10
    body = blocks["0S"]
    n = struct.unpack("<H", body[:2])[0]
    dscr = {i: body[2 + 5 * i] for i in range(n) if body[2 + 5 * i]}
    return names, dscr


def object_name(oc: bytes) -> str:
    return oc[oc[18] :].split(b"\0")[0].rstrip(b"@").decode("latin-1")


def descumm(binary: str, block: Path) -> tuple[int, bytes, bytes]:
    proc = subprocess.run([binary, *DESCUMM_FLAGS, str(block)], capture_output=True)
    return proc.returncode, proc.stdout, proc.stderr


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--index-file", type=Path, required=True)
    ap.add_argument("--blocks", type=Path, required=True)
    ap.add_argument("--descumm", required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--tool-info", type=Path)
    args = ap.parse_args()

    names, dscr = parse_index(args.index_file)
    lfs = sorted(args.blocks.glob("DISK_*/LE/LF_*"), key=lambda p: int(p.name.split("_")[1]))
    if not lfs:
        raise SystemExit(f"no LF directories under {args.blocks}")

    jobs: list[dict] = []
    rooms: dict[int, dict] = {}
    objects: dict[int, dict] = {}
    globals_: dict[int, dict] = {}
    problems: list[str] = []

    for lf in lfs:
        room = int(lf.name.split("_")[1])
        rname = names.get(room, "")
        if not rname:
            problems.append(f"LF {room} has no RN entry")
        rdir = f"room-{room:03d}-{slug(rname)}"
        rinfo = rooms[room] = {"name": rname, "dir": rdir, "disk": lf.parent.parent.name,
                               "entry": None, "exit": None, "local_scripts": {}, "objects": {},
                               "global_scripts_stored_here": []}
        for sc in sorted(lf.glob("SC_*")):
            num = int(sc.name.split("_")[1])
            out = f"global/script-{num:03d}.txt"
            if num in globals_:
                problems.append(f"global script {num} dumped twice")
            globals_[num] = {"file": out, "room": room, "room_name": rname}
            rinfo["global_scripts_stored_here"].append(num)
            jobs.append({"kind": "global", "room": room, "number": num, "block": sc, "out": out})
        ro = lf / "RO"
        for kind, tag in (("entry", "EN"), ("exit", "EX")):
            blk = ro / tag
            if blk.exists():
                out = f"{rdir}/{kind}.txt"
                rinfo[kind] = {"file": out}
                jobs.append({"kind": kind, "room": room, "number": None, "block": blk, "out": out})
        for ls in sorted(ro.glob("LS_*")):
            num = int(ls.name.split("_")[1])
            out = f"{rdir}/local-{num:03d}.txt"
            rinfo["local_scripts"][str(num)] = {"file": out}
            jobs.append({"kind": "local", "room": room, "number": num, "block": ls, "out": out})
        for oc in sorted(ro.glob("OC_*")):
            num = int(oc.name.split("_")[1])
            data = oc.read_bytes()
            if struct.unpack("<H", data[6:8])[0] != num:
                problems.append(f"{oc}: id {struct.unpack('<H', data[6:8])[0]} != file name id {num}")
            oname = object_name(data)
            out = f"{rdir}/obj-{num:04d}-{slug(oname)}.txt"
            verbs = []
            p = 19
            while data[p] != 0:
                verbs.append({"verb": data[p], "offset": f"{struct.unpack('<H', data[p + 1 : p + 3])[0]:04X}"})
                p += 3
            if num in objects:
                problems.append(f"object {num} appears in rooms {objects[num]['room']} and {room}")
            objects[num] = {"name": oname, "room": room, "room_name": rname, "file": out,
                            "parent": data[12], "verbs": verbs}
            rinfo["objects"][str(num)] = {"name": oname, "file": out}
            jobs.append({"kind": "verb", "room": room, "number": num, "block": oc, "out": out})

    for num, room in sorted(dscr.items()):
        g = globals_.get(num)
        if g is None:
            problems.append(f"0S lists global script {num} in room {room} but it was not dumped")
        elif g["room"] != room:
            problems.append(f"global script {num}: 0S says room {room}, dump has LF {g['room']}")

    with ThreadPoolExecutor(max_workers=args.jobs) as ex:
        results = list(ex.map(lambda j: descumm(args.descumm, j["block"]), jobs))

    tmp = args.out.with_name(args.out.name + ".tmp")
    shutil.rmtree(tmp, ignore_errors=True)
    failures = []
    for job, (rc, out, err) in zip(jobs, results, strict=True):
        errors = []
        if rc != 0:
            errors.append(f"descumm exit code {rc}")
        for line in STRING_LIT_RE.sub(b'""', out).splitlines():
            if HARD_ERROR_RE.search(line):
                errors.append(line.decode("latin-1").strip())
        if errors:
            failures.append({"file": job["out"], "errors": errors[:5]})
        header = [
            f"# {job['kind']} script" + (f" {job['number']}" if job["number"] is not None else "")
            + f", room {job['room']} ({rooms[job['room']]['name']})",
            f"# decompiled with: descumm {' '.join(DESCUMM_FLAGS)} {job['block'].name}",
            f"# offsets: {OFFSET_BASES[job['kind']]}",
            "",
        ]
        dest = tmp / job["out"]
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes("\n".join(header).encode() + b"\n" + out + (b"\n# stderr:\n" + err if err else b""))

    tools = json.loads(args.tool_info.read_text()) if args.tool_info else {}
    tools.setdefault("descumm", {})["flags"] = " ".join(DESCUMM_FLAGS)
    index = {"game": "monkey / VGA (DOS floppy, SCUMM v4)", "tools": tools, "offset_bases": OFFSET_BASES,
             "rooms": {str(k): v for k, v in sorted(rooms.items())},
             "objects": {str(k): v for k, v in sorted(objects.items())},
             "globals": {str(k): v for k, v in sorted(globals_.items())},
             "problems": problems, "failures": failures, "descumm_runs": len(jobs)}
    (tmp / "index.json").write_text(json.dumps(index, indent=1) + "\n")
    lines = ["# MI1 DOS VGA floppy (SCUMM v4) script index", "",
             f"{len(jobs)} blocks decompiled; {len(failures)} with descumm errors; {len(problems)} index problems.", ""]
    lines += [f"- problem: {p}" for p in problems]
    lines += [f"- failure: `{f['file']}`: {'; '.join(f['errors'])}" for f in failures]
    lines += ["", "| room | name | disk | locals | objects | globals stored here |", "|---|---|---|---|---|---|"]
    for r, info in sorted(rooms.items()):
        lines.append(f"| {r} | {info['name']} | {info['disk']} | {len(info['local_scripts'])} | "
                     f"{len(info['objects'])} | {', '.join(map(str, info['global_scripts_stored_here'])) or '-'} |")
    (tmp / "INDEX.md").write_text("\n".join(lines) + "\n")

    shutil.rmtree(args.out, ignore_errors=True)
    tmp.rename(args.out)
    print(f"{len(jobs)} blocks, {len(failures)} failures, {len(problems)} problems -> {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
