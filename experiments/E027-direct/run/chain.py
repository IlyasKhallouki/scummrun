"""chain.py OUT [--hours H] [--stall M] [--workers W] [--bin BIN] [--from NODES ID] [--edges F] [--toggles F]

E027: the direct search from the boot to the game's end in legs, with no hand between them. A leg is one run of the
search (`direct`) with `--stall M`: it ends when its highest level was not raised for M minutes. The next leg begins
at the stop the leg ended with as its top (OUT/legNN/top.jsonl: the newest stop that began a line at its highest
level), with the ways from room to room and the flags seen unset again that the legs before it saw. The chain ends
when the evaluation's end value is held (--until, the rungs' end: bit:565 = 1 in Monkey Island), after H hours, or
after three legs in a row that won nothing over the stop they began at (no object owned that was not before).

The rule is the one the chain of 2026-11-12/13 followed by hand (a run that stalls is followed by a run from its top
stop), without the hand. Each leg's stops replay from the boot, as every run's do.

Settings: experiments/E027-direct/run/env.sh, through chain.sh (the engine, the game, the guide and the rungs).
"""
import json, os, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
FLAGS = ["--lines", "gains", "--flag-lines", "--shares", "--go", "--motion-once", "--count-lines", "--stay-lines",
         "--born-first", "--walk-born", "--runs-on", "--counts-world", "--ends", "--said-flags"]
# what is read to report (evaluation only: rooms entered and values held, first by which stop and when)
WATCH = ("room=20,room=12,room=69,room=39,room=70,room=75,room=83,room=59,owner:901=1,owner:902=1,owner:903=1,"
         "owner:761=1,owner:823=1,owner:733=1")
UNTIL = "bit:565=1"


def opt(a, name, default=None, n=1):
    if name not in a:
        return default
    i = a.index(name)
    return a[i + 1] if n == 1 else a[i + 1:i + 1 + n]


def env():
    # the settings env.sh sets (chain.sh sources it and exports ON, MI, GUIDE27 and RUNGS27): ON and MI are lists of
    # K=V for the engine and the search
    e = dict(os.environ)
    for kv in (e.get("ON", "") + " " + e.get("MI", "")).split():
        if "=" in kv:
            k, v = kv.split("=", 1)
            e[k] = v
    return e, e["GUIDE27"], e["RUNGS27"]


def top_of(leg: Path):
    p = leg / "top.jsonl"
    if not p.exists():
        return None
    line = p.read_text()
    i = line.find(',"events"')
    d = json.loads(line[:i] + "}")
    return d["id"], d.get("ever", []), d["room"], d["frame"]


def main():
    a = sys.argv[1:]
    out = Path(a[0])
    out.mkdir(parents=True, exist_ok=True)
    hours = float(opt(a, "--hours", "12"))
    stall = opt(a, "--stall", "45")
    workers = opt(a, "--workers", "7")
    binary = opt(a, "--bin", "/var/tmp/fast-mi/direct/bench/bin/chain1")
    start = opt(a, "--from", None, 2)
    edges = opt(a, "--edges")
    toggles = opt(a, "--toggles")
    e, guide, rungs = env()
    t0 = time.time()
    log = open(out / "chain.txt", "a")

    def say(s):
        line = f"{time.strftime('%Y-%m-%d %H:%M:%S', time.gmtime())} UTC  {s}"
        print(line, flush=True)
        log.write(line + "\n")
        log.flush()

    say(f"chain: {binary}, {workers} threads, stall {stall} min, {hours} h at most, from {'the boot' if not start else start}")
    idle, n, ever0 = 0, 0, None
    while True:
        left = hours * 60 - (time.time() - t0) / 60
        if left < 5:
            say("the hours are spent: the chain ends")
            return 1
        leg = out / f"leg{n:02d}"
        leg.mkdir(exist_ok=True)
        cmd = [binary, str(leg), "--minutes", f"{left:.1f}", "--stall", stall, "--workers", workers, "--slices", workers,
               "--batch", "1", "--at-once", "1", "--guide", guide, "--rungs", rungs, "--watch", WATCH, "--until", UNTIL] + FLAGS
        if start:
            cmd += ["--from", start[0], start[1]]
        if edges:
            cmd += ["--edges", edges]
        if toggles:
            cmd += ["--toggles", toggles]
        say(f"leg {n:02d}: begins" + (f" at {start[0]} stop {start[1]}" if start else " at the boot"))
        with open(leg / "out.txt", "w") as fo, open(leg / "err.txt", "w") as fe:
            rc = subprocess.run(cmd, env=e, stdout=fo, stderr=fe, stdin=subprocess.DEVNULL).returncode
        err = (leg / "err.txt").read_text(errors="replace")
        for l in err.splitlines():
            if "WATCH" in l or "UNTIL" in l or "STALL" in l or "RUNG" in l:
                say(f"leg {n:02d}: {l.strip()}")
        if "UNTIL" in err:
            say(f"leg {n:02d}: the end value is held: the chain ends ({(time.time() - t0) / 3600:.2f} h)")
            return 0
        if rc != 0:
            say(f"leg {n:02d}: the search failed (exit {rc}): the chain ends")
            return 2
        top = top_of(leg)
        if top is None:
            say(f"leg {n:02d}: no top stop: the chain ends")
            return 3
        tid, ever, room, frame = top
        say(f"leg {n:02d}: top stop {tid}, room {room}, frame {frame}, {len(ever)} objects ever owned")
        # (won nothing over its start: the objects ever owned at its top stop are those at the start)
        if ever0 is not None and len(ever) <= len(ever0):
            idle += 1
            if idle >= 3:
                say("three legs in a row won nothing: the chain ends")
                return 4
        else:
            idle = 0
        ever0 = ever
        # the next leg: from the top stop, with the ways and the flags seen unset that this leg knew
        start = [str(leg / "top.jsonl"), str(tid)]
        edges = str(leg / "edges.jsonl")
        tg = leg / "toggles.txt"
        if tg.exists():
            merged = out / f"toggles-{n:02d}.txt"
            ks = set(tg.read_text().split())
            if toggles and Path(toggles).exists():
                ks |= set(Path(toggles).read_text().split())
            merged.write_text("".join(k + "\n" for k in sorted(ks)))
            toggles = str(merged)
        for x in leg.glob("eng"):
            subprocess.run(["rm", "-rf", str(x)])
        n += 1


if __name__ == "__main__":
    sys.exit(main())
