"""E004 plan step C2: the duration rules in game time (durations.md), applied to a reading's actions.

usage (from experiments/E003-path-through-cloud, with ../../.venv/bin/python):
  ../E004-fast-exact/durations.py bounds --task R/reading.tc --wait-table W.tsv [--caps F | --start-pos]
                                         --out B.tsv [--taskcore BIN]

The wait table is the reader's `wait-table --reading R/reading.json` (its `wait`, `recursive`, `wild`, `walk` and
`const` lines). The task's actions come from taskcore's `dump` of the input task (no fold, no reduction), so every
packed action is listed with its label, kind and frame cost (a reading made with `--frame-costs`).

The bounds file, tab-separated, is what taskcore's `--timed` and trace_check.py's game-time wait check read:

  # key value             delta_lo, dmax, talkstop (1: the talk-stop key can be on), qf_frame (4 * delta_lo)
  action LABEL COST CLASS UNIT W LB RULE ROOMS
                          per (label, frame cost): CLASS `event` (UNIT@W, LB from its entry) or `input` (LB from
                          the previous input, or the start); LB in quarter-frames, the least over the rooms whose
                          unit of that name has W; ROOMS `room:lb;...` per room where the name is a room's unit
                          (`local:N`: the current room's, durations.md "The rule table"), `-` otherwise; RULE names
                          the lemma (the least LB's)
  point UNIT ROOM W LB RULES
                          per event wait point of a unit: the least LB of its actions (the check's bound there)
  cap UNIT W LB KIND      D8: a slot waiting at W at the start; KIND `paused` (its next event's bound from the
                          start is LB) or `other` (the least of LB and the event's own bound); D7's units get 0

`--caps F`: the harness's cost caps (replay.py ``cost_caps_of``: ``event(NAME@W<TAB>frames``), turned into game
time by D8. `--start-pos`: no dump at hand; every slot the start holds at a wait point gets D8's one frame, a bound for
any slot (D2). Without either, no cap is written (a start where no slot waits).

An instrument and a writer of an input file: nothing else reads it, and with taskcore's --timed off nothing changes.
"""

import argparse
import collections
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
E003 = HERE.parent / "E003-path-through-cloud"
TASKCORE = E003 / "taskcore" / "target" / "release" / "taskcore"

INPUT_KINDS = {"sentence", "walk_to", "dialog_line", "skip_cutscene"}
EVENT_RE = re.compile(r"event\(([^@()]+)@(-?\d+)")


class WaitTable:
    """The reader's wait-table file (wait_table.rs's module docstring)."""

    def __init__(self, path: Path):
        self.dmax, self.charinc = None, None
        self.points = collections.defaultdict(list)  # (name, W) -> [(room, kinds, rule)]
        self.recursive, self.wild, self.walks, self.const = {}, [], set(), {}  # recursive: (name, room) -> cite
        self.rooms = None
        for line in Path(path).read_text().splitlines():
            if line.startswith("#"):
                m = re.search(r"dmax (\d+), charinc (\S+)", line)
                if m:
                    self.dmax = int(m[1])
                    self.charinc = None if m[2] == "None" else int(re.sub(r"\D", "", m[2]))
                continue
            f = line.split("\t")
            if f[0] == "wait":
                self.points[(f[1], int(f[3]))].append((f[2], f[4].split(",") if f[4] else [], f[5]))
            elif f[0] == "recursive":
                self.recursive.setdefault((f[1], f[2]), f[3])
            elif f[0] == "wild":
                self.wild.append(f[1:])
            elif f[0] == "walk":
                self.walks.add((f[1], int(f[3])))
            elif f[0] == "const":
                self.const[f[1]] = (int(f[2]), f[3] == "1")
            elif f[0] == "scope":
                self.rooms = [int(r) for r in f[1].split(",")]
        assert self.dmax is not None, f"{path}: no dmax line"


def delta_lo(wt: WaitTable, facts: dict) -> int:
    """D1's δ_lo: max(1, var 19 at the start) when the scope does not write it; else the least value of its domain
    in the task when it is a fact with integer values only; else 1."""
    start, written = wt.const.get("var:19", (0, True))
    if not written:
        return max(1, start)
    vals = facts.get("var:19")
    if vals and all(re.fullmatch(r"-?\d+", v) for v in vals):
        return max(1, min(int(v) for v in vals))
    return 1


def talkstop_on(wt: WaitTable) -> bool:
    """D6 (a): the talk-stop key can be on during a wait (VAR_TALKSTOP_KEY not 0 at the start, or written)."""
    start, written = wt.const.get("var:57", (1, True))
    return start != 0 or written


def delay_qf(frames: int, dl: int, dmax: int) -> int:
    """D4 from costs.rs's frame bound F = floor(n / dmax) + 1: 4 * max(δ_lo * F, dmax * (F - 1) + 1)."""
    return 4 * max(dl * frames, dmax * (frames - 1) + 1)


def event_bound(name: str, w: int, cost: int, wt: WaitTable, dl: int, talkstop: bool, room: str) -> tuple:
    """(lb in quarter-frames, rule) for an event of the unit `name` of `room` (`-` for a unit of no room) resuming at
    `w` with costs.rs's frame cost `cost` (durations.md, the rule table)."""
    if wt.wild:
        return 0, "recursive: a wild recursive start in the scope (D7)"
    if (name, room) in wt.recursive:
        return 0, "recursive unit (D7)"
    rows = [r for r in wt.points.get((name, w), []) if r[0] == room]
    if not rows:
        return 0, "no yield ends at W (a start not yet run): 0"
    kinds = {k for _, ks, _ in rows for k in ks}
    if "WaitForMessage" in kinds:
        why = "the talk-stop key on" if talkstop else "the camera not shown fixed"
        return 4 * dl, f"talk wait: 1 frame, {why} (D6)"
    if kinds & {"delay", "delayVariable"}:
        return delay_qf(cost, dl, wt.dmax), "delay (D4)"
    if "WaitForActor" in kinds and (name, w) in wt.walks:
        return 4 * dl * cost, "walk wait: 1 frame, D9 not established (D2)"
    rule = "/".join(sorted(kinds)) or "wait"
    return 4 * dl * cost, f"{rule}: {cost} frame(s) (D2, D3)"


def input_bound(cost: int, dl: int) -> tuple:
    return 4 * dl * cost, f"input: {cost} key(s) or click(s), one a frame (D5)"


def dump_task(task: Path, binary: Path) -> tuple:
    """taskcore dump of the input task: (facts: key -> values, actions: [dict])."""
    with tempfile.TemporaryDirectory(prefix="durations-") as tmp:
        out = Path(tmp) / "task.dump"
        r = subprocess.run([str(binary), "dump", "--task", str(task), "--fold", "off", "--reduce", "none",
                            "--encoding", "task", "--out", str(out)], capture_output=True, text=True)
        if r.returncode != 0:
            raise RuntimeError(r.stderr)
        facts, acts = {}, []
        with open(out) as f:
            for line in f:
                d = json.loads(line)
                if "fact" in d:
                    facts[d["fact"]] = (d["values"], d.get("start"))
                else:
                    acts.append(d)
    return facts, acts


def bounds(task: Path, wt: WaitTable, binary: Path = TASKCORE, caps=None, start_pos: bool = False) -> dict:
    facts, acts = dump_task(task, binary)
    dl = delta_lo(wt, {k: v for k, (v, _) in facts.items()})
    ts = talkstop_on(wt)
    out = {"head": {"delta_lo": dl, "dmax": wt.dmax, "talkstop": int(ts), "qf_frame": 4 * dl}, "actions": {},
           "points": {}, "caps": {}, "rules": collections.Counter()}
    for a in acts:
        key = (a["label"], a["cost"])
        if key in out["actions"]:
            continue
        m = EVENT_RE.match(a["label"])
        if a["kind"] in INPUT_KINDS:
            lb, rule = input_bound(a["cost"], dl)
            row = ("input", "-", -1, lb, rule, "-")
        elif m:
            name, w = m[1], int(m[2])
            rooms = sorted({r for r, _, _ in wt.points.get((name, w), [])} | {r for n, r in wt.recursive if n == name})
            per = {r: event_bound(name, w, a["cost"], wt, dl, ts, r) for r in rooms} or \
                {"-": event_bound(name, w, a["cost"], wt, dl, ts, "-")}
            lb, rule = min(per.values())
            by_room = ";".join(f"{r}:{v[0]}" for r, v in per.items()) if any(r != "-" for r in per) else "-"
            row = ("event", name, w, lb, rule, by_room)
            for r, (lbr, ruler) in per.items():
                p = out["points"].setdefault((name, r, w), [lbr, set()])
                p[0] = min(p[0], lbr)
                p[1].add(ruler.split(":")[0])
        else:
            raise ValueError(f"an action that is neither an input nor an event: {a['label']} ({a['kind']})")
        out["actions"][key] = row
        out["rules"][rule.split(":")[0] + (" (input)" if row[0] == "input" else "")] += 1
    if caps is not None:
        for name, w, frames in caps:
            out["caps"][(name, w)] = (delay_qf(frames, dl, wt.dmax), "paused") if frames > 1 else (4 * dl, "other")
    elif start_pos:
        for k, (vals, start) in facts.items():
            if k.startswith("pos:") and start not in (None, "-1"):
                out["caps"][(k[4:], int(start))] = (4 * dl, "other")
    return out


def read_caps(path: Path) -> list:
    """The harness's caps file (replay.py ``cost_caps_of``): ``event(NAME@W<TAB>frames`` per line."""
    out = []
    for line in Path(path).read_text().splitlines():
        if not line.strip():
            continue
        k, v = line.split("\t")
        m = EVENT_RE.match(k)
        out.append((m[1], int(m[2]), int(v)))
    return out


def write(b: dict, path: Path) -> None:
    with open(path, "w") as f:
        for k, v in b["head"].items():
            f.write(f"# {k} {v}\n")
        for (label, cost), (cls, unit, w, lb, rule, rooms) in sorted(b["actions"].items()):
            f.write(f"action\t{label}\t{cost}\t{cls}\t{unit}\t{w}\t{lb}\t{rule}\t{rooms}\n")
        for (unit, room, w), (lb, rules) in sorted(b["points"].items()):
            f.write(f"point\t{unit}\t{room}\t{w}\t{lb}\t{','.join(sorted(rules))}\n")
        for (unit, w), (lb, kind) in sorted(b["caps"].items()):
            f.write(f"cap\t{unit}\t{w}\t{lb}\t{kind}\n")


def read(path: Path) -> dict:
    """A bounds file back: head, actions by (label, cost), points by (unit, room, W), caps by (unit, W)."""
    b = {"head": {}, "actions": {}, "points": {}, "caps": {}}
    for line in Path(path).read_text().splitlines():
        if line.startswith("# "):
            k, v = line[2:].split(" ", 1)
            b["head"][k] = int(v)
            continue
        f = line.split("\t")
        if f[0] == "action":
            rooms = {} if f[8] == "-" else {r: int(v) for r, v in (x.split(":") for x in f[8].split(";"))}
            b["actions"][(f[1], int(f[2]))] = (f[3], f[4], int(f[5]), int(f[6]), f[7], rooms)
        elif f[0] == "point":
            b["points"][(f[1], f[2], int(f[3]))] = (int(f[4]), f[5])
        elif f[0] == "cap":
            b["caps"][(f[1], int(f[2]))] = (int(f[3]), f[4])
    return b


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("bounds")
    p.add_argument("--task", type=Path, required=True)
    p.add_argument("--wait-table", type=Path, required=True)
    p.add_argument("--caps", type=Path)
    p.add_argument("--start-pos", action="store_true")
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--taskcore", type=Path, default=TASKCORE)
    a = ap.parse_args()
    wt = WaitTable(a.wait_table)
    b = bounds(a.task, wt, a.taskcore, read_caps(a.caps) if a.caps else None, a.start_pos)
    write(b, a.out)
    print(f"{a.out}: {len(b['actions'])} (label, cost) pairs, {len(b['points'])} wait points, {len(b['caps'])} caps; "
          f"delta_lo {b['head']['delta_lo']}, dmax {b['head']['dmax']}, talk-stop key on {bool(b['head']['talkstop'])}")
    for r, n in b["rules"].most_common():
        print(f"  {n:6d}  {r}")


if __name__ == "__main__":
    sys.exit(main())
