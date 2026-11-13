"""keep.py OUT BIN: keeps the chain in OUT going until the game's end value is held.

When a chain (run/chain.py) ends for any reason other than the end value (its hours spent, a failed search, no top
stop, three idle legs), a new chain starts from the furthest stop the last chains reached (the newest leg with a
top.jsonl), with that leg's ways and the merged toggles, in full1/contN. Same rule as the legs, no hand. Up to 6
continuations. Log: full1/keep.txt.
"""
import json, os, subprocess, time
from pathlib import Path

import sys
D = Path(sys.argv[1])
CHAIN = "/var/tmp/fast-mi/wt-direct/experiments/E027-direct/run/chain.sh"
BIN = sys.argv[2]
log = open(D / "keep.txt", "a")


def say(s):
    log.write(f"{time.strftime('%Y-%m-%d %H:%M:%S', time.gmtime())} UTC  {s}\n")
    log.flush()


def chain_alive():
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            if open(f"/proc/{p}/comm").read().strip() != "python3":
                continue
            c = open(f"/proc/{p}/cmdline", "rb").read()
            if b"chain.py" in c and str(D).encode() in c:
                return True
        except Exception:
            pass
    return False


def chains():
    # the chain dirs in order: full1 itself, then cont1, cont2, ...
    out = [D]
    i = 1
    while (D / f"cont{i}").exists():
        out.append(D / f"cont{i}")
        i += 1
    return out


def ended_at_end():
    return any("the end value is held" in (c / "chain.txt").read_text(errors="replace")
               for c in chains() if (c / "chain.txt").exists())


def furthest():
    # the newest leg (over all chains) that wrote a top stop
    legs = []
    for c in chains():
        legs += sorted(c.glob("leg[0-9][0-9]"))
    for leg in reversed(legs):
        if (leg / "top.jsonl").exists():
            return leg
    return None


def toggles():
    ks = set()
    for c in chains():
        for f in list(c.glob("toggles-*.txt")) + list(c.glob("leg*/toggles.txt")):
            ks |= set(f.read_text().split())
    if not ks:
        return None
    p = D / "toggles-keep.txt"
    p.write_text("".join(k + "\n" for k in sorted(ks)))
    return p


say(f"keep: watching {D}")
for n in range(1, 7):
    while chain_alive():
        time.sleep(60)
    if ended_at_end():
        say("the end value is held: done")
        break
    leg = furthest()
    out = D / f"cont{n}"
    out.mkdir()
    cmd = ["systemd-inhibit", "--what=idle:sleep", "--why=scummrun full run", "nice", "-n", "5", CHAIN, str(out),
           "--hours", "14", "--stall", "60", "--workers", "7", "--bin", BIN]
    if leg is not None:
        line = open(leg / "top.jsonl").read()
        tid = json.loads(line[:line.find(',"events"')] + "}")["id"]
        cmd += ["--from", str(leg / "top.jsonl"), str(tid)]
        if (leg / "edges.jsonl").exists():
            cmd += ["--edges", str(leg / "edges.jsonl")]
    tg = toggles()
    if tg:
        cmd += ["--toggles", str(tg)]
    say(f"chain ended without the end value; cont{n} begins: " + " ".join(cmd[6:]))
    with open(out / "run.log", "w") as fo:
        subprocess.Popen(cmd, stdout=fo, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, start_new_session=True)
    time.sleep(120)
else:
    say("six continuations used: keep stops")
