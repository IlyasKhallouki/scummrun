"""Replay a raw input movie (bridge contract C8) and summarise where the time goes.

``speedrun probe`` is the Any% experiment loop: one headless run of a release
with a ``SPEEDRUN_INPUTS`` schedule, reported as the rooms it passed through,
the frames at which the player had control, the var 19 (``VAR_TIMER_NEXT``)
values, and wall-clock time from every wait the engine made (the records'
``qf`` quarter-frames, which also count room-change effects) at the release's
timer rate (``speedrun.releases``). Times count from the segment start: the
logo-skip frame for Any% (``start_frame``).
"""

import json
from dataclasses import dataclass, field
from pathlib import Path

from speedrun import releases
from speedrun.engine import EngineConfig, run_engine


@dataclass
class ProbeResult:
    release: str
    out_dir: Path
    records: list[dict]
    end: dict
    tick0: int | None
    goal_tick: int | None
    qf0: int | None = None  # quarter-frames at the segment start: the RTA clock (C2)
    goal_qf: int | None = None
    rooms: list[dict] = field(default_factory=list)
    controls: list[dict] = field(default_factory=list)

    def seconds(self, ticks: float) -> float:
        return releases.get(self.release).seconds(ticks)

    def rta(self, qf: int) -> float:
        """Wall-clock seconds from the segment start to a record's ``qf``."""
        return (qf - (self.qf0 or 0)) * releases.get(self.release).seconds(1) / 4

    @property
    def ticks(self) -> int | None:
        """Ticks from the segment start to the goal (None without both)."""
        if self.tick0 is None or self.goal_tick is None:
            return None
        return self.goal_tick - self.tick0


def read_records(path: Path) -> list[dict]:
    return [json.loads(line) for line in Path(path).read_text().splitlines() if line.strip()]


def probe(cfg: EngineConfig) -> ProbeResult:
    run_engine(cfg)
    records = read_records(cfg.out_dir / "trace.jsonl")
    start = next((r for r in records if r["type"] == "segment_start"), None)
    goal = next((r for r in records if r["type"] == "goal"), None)
    tick0 = start["tick0"] if start else None
    return ProbeResult(
        release=cfg.release,
        out_dir=cfg.out_dir,
        records=records,
        end=records[-1] if records else {},
        tick0=tick0,
        goal_tick=(tick0 + goal["ticks_from_start"]) if goal and tick0 is not None else None,
        qf0=start.get("qf") if start else None,
        goal_qf=goal.get("qf") if goal else None,
        rooms=[r for r in records if r["type"] == "room"],
        controls=[r for r in records if r["type"] == "control"],
    )


def report(res: ProbeResult) -> str:
    rel = releases.get(res.release)
    t0 = res.tick0 or 0
    lines = [f"release {rel.name}: tick = {rel.seconds(1) * 1000:.3f} ms; segment start at tick {res.tick0}"]
    events = sorted(res.rooms + res.controls + [r for r in res.records if r["type"] in ("input", "skip", "error")],
                    key=lambda r: (r["tick"], r["frame"]))
    for r in events:
        dt = res.rta(r["qf"]) if "qf" in r else rel.seconds(r["tick"] - t0)
        if r["type"] == "room":
            what = f"room {r['from']} -> {r['room']} (var19 {r['var19']})"
        elif r["type"] == "control":
            what = f"control ({r['kind']}) in room {r['room']} at {r.get('ego_pos')}"
        elif r["type"] == "input":
            what = "input " + ", ".join(f"{k} {r[k]}" for k in ("key", "click", "rclick", "move") if k in r)
        elif r["type"] == "skip":
            what = f"skip {r['kind']} in room {r['room']}"
        else:
            what = f"error {r.get('code')}: {r.get('message')}"
        lines.append(f"  f{r['frame']:>6} t{r['tick']:>7} {dt:>9.3f}s  {what}")
    end = res.end
    lines.append(f"end: {end.get('reason')} at tick {end.get('tick')} frame {end.get('frame')} room {end.get('room')}")
    if res.ticks is not None:
        rta = f"{res.rta(res.goal_qf):.3f} s RTA" if res.goal_qf is not None else f"{rel.seconds(res.ticks):.3f} s"
        lines.append(f"goal: {res.ticks} ticks, {rta}")
    return "\n".join(lines)
