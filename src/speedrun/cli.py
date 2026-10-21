"""`speedrun` command-line entry point.

The pipeline is ``dump-objects`` -> ``plan`` -> ``compile`` -> ``run``/``demo``;
``measure`` replays a plan on many seeds and ``optimize`` searches for the
time-optimal plan (Phase 8). ``extract-check``, ``extract-merge`` and
``pddl-diff`` validate, merge and compare the extracted model (Phase 7,
``speedrun.citations``, ``speedrun.extract``, ``speedrun.pddl_diff``).
Each later command brings its inputs up to date first: ``compile`` dumps the
objects if ``out/objects.json`` is missing and re-plans if the ``.sas_plan`` is
older than the model or incomplete, and ``run``/``demo`` recompile if the
``.jsonl`` is older than any of its inputs. An output whose inputs were edited
while it was being built is back-dated, so the next command rebuilds it.

**Objective.** ``run``, ``demo`` and ``measure`` replay the *time* plan,
``out/plans/<segment>.time.jsonl`` (written by ``speedrun optimize``), when it
is fresh: newer than ``domain.pddl``, ``problem.pddl``, ``segment.toml``,
``steps.toml``, ``measured-costs.json`` and ``objects.json``, with
``measured-costs.json`` measured under the engine's current timing pins
(``speedrun.engine.timing_settings``, e.g. ``talkspeed``). Otherwise they use
the action-count plan, recompiled as above. ``--objective actions|time``
overrides the choice, and every command prints the objective it used.

Every handler reads ``paths.*`` at call time, and calls ``run_engine`` and
``run_planner`` through this module's globals, so tests can monkeypatch both
the directories and the ScummVM/Fast Downward boundaries.
"""

import argparse
import contextlib
import dataclasses
import itertools
import json
import os
import shutil
import signal
import sys
import threading
from collections.abc import Iterable
from datetime import UTC, datetime
from pathlib import Path

from speedrun import extract, gamedata, measure, paths, pddl_diff, probe, releases
from speedrun.citations import check_fragments, filter_fragments, parse_fragment
from speedrun.compiler import CompileError, ObjectIndex, compile_plan, load_steps, write_jsonl
from speedrun.costing import CostingError, CostTable, write_timed
from speedrun.engine import EngineConfig, EngineResult, bridge_version, run_engine, timing_settings
from speedrun.planner import Plan, PlannerError, parse_plan, plan_is_complete, run_planner
from speedrun.segments import Segment, SegmentError, load_segment
from speedrun.stats import plan_stats
from speedrun.trace import Trace, TraceError, format_error, format_table, format_ticks, load_trace, summary

DEFAULT_SEGMENT = "part1"

DUMP_MAX_TICKS = 60000
DUMP_TIMEOUT_S = 600
# Wall-clock budgets: generous, since a demo plays the whole segment in real time.
RUN_TIMEOUT_S = {"run": 1800, "demo": 3600}

NOT_MEASURED = "(NOT MEASURED: boot param)"

_BANNER = "!" * 78


class CommandError(Exception):
    """A command failed in an expected way; ``main`` prints the message and exits 1."""


def _warn(*lines: str) -> None:
    """A loud, framed warning on stderr."""
    sys.stdout.flush()  # keep the warning in place when stdout is a pipe
    print(_BANNER, *(f"!!! {line}" for line in lines), _BANNER, sep="\n", file=sys.stderr)


def _positive_int(text: str) -> int:
    value = int(text)
    if value <= 0:
        raise argparse.ArgumentTypeError(f"must be a positive integer, got {value}")
    return value


def _non_negative_int(text: str) -> int:
    value = int(text)
    if value < 0:
        raise argparse.ArgumentTypeError(f"must be a non-negative integer, got {value}")
    return value


def _fraction(text: str) -> float:
    try:
        value = float(text)
    except ValueError:
        raise argparse.ArgumentTypeError(f"must be a number in [0, 1), got {text!r}") from None
    if not 0 <= value < 1:
        raise argparse.ArgumentTypeError(f"must be in [0, 1), got {value}")
    return value


def _seed_spec(text: str) -> list[int]:
    try:
        return measure.parse_seeds(text)
    except ValueError as e:
        raise argparse.ArgumentTypeError(str(e)) from e


def default_jobs() -> int:
    """Parallel engine runs: every CPU but two, and at least one."""
    return max(1, (os.cpu_count() or 1) - 2)


OBJECTIVES = ("actions", "time")


# --- parser ------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="speedrun",
        description="Automated glitchless speedrunner for The Secret of Monkey Island.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    sub = parser.add_subparsers(dest="command", metavar="<command>")

    sub.add_parser(
        "extract",
        help="detect game data layout and extract classic MI1 files into game/classic",
    )

    p = sub.add_parser(
        "dump-objects",
        help="dump all rooms' objects and verbs from the engine to out/objects.json",
    )
    p.add_argument(
        "--max-ticks", type=_positive_int, metavar="N",
        help=f"engine tick cap for the dump (default {DUMP_MAX_TICKS})",
    )  # fmt: skip

    p = sub.add_parser("plan", help="run Fast Downward on the segment's PDDL model")
    p.add_argument("segment", help="segment name, e.g. part1")
    p.add_argument(
        "--objective", choices=OBJECTIVES, default="actions",
        help="actions: the model as written (default); time: a timed copy costed from measured-costs.json",
    )  # fmt: skip

    p = sub.add_parser("compile", help="compile the planner output to the player's JSONL")
    p.add_argument(
        "segment", nargs="?", default=DEFAULT_SEGMENT, help=f"segment name (default: {DEFAULT_SEGMENT})"
    )

    for mode, text in (
        ("run", "replay the segment headless and print ticks"),
        ("demo", "replay the segment in a visible window, in real time from the segment start, and print ticks"),
    ):
        p = sub.add_parser(mode, help=text)
        p.add_argument("segment", help="segment name, e.g. part1")
        p.add_argument("--seed", type=_non_negative_int, default=1, metavar="N",
                       help="ScummVM random seed (default 1)")  # fmt: skip
        p.add_argument("--max-ticks", type=_positive_int, metavar="N", help="engine tick cap, counted from boot")
        p.add_argument("--replan", action="store_true", help="re-run the planner and recompile first")
        p.add_argument("--release", choices=sorted(releases.RELEASES), default=releases.DEFAULT_RELEASE,
                       help=f"game release (default {releases.DEFAULT_RELEASE}); the plan's object ids must exist in it")
        p.add_argument(
            "--boot-param", type=int, metavar="N",
            help="pass ScummVM --boot-param=N (forces debug mode: the run is NOT a valid measured run)",
        )  # fmt: skip
        p.add_argument(
            "--objective", choices=OBJECTIVES,
            help="replay the action-count or the time plan (default: time if its plan is fresh, else actions)",
        )  # fmt: skip
        if mode == "demo":  # run is fast from boot to end already
            p.add_argument(
                "--fast-boot", action=argparse.BooleanOptionalAction, default=True,
                help="fast-forward the boot (logo, credits, opening) to the segment start, then play in "
                     "real time; ticks are unaffected (default: on)",
            )  # fmt: skip

    p = sub.add_parser("probe", help="replay a raw input movie (Any%%) on a release and show where the time goes")
    p.add_argument("inputs", type=Path, nargs="?", help="raw input schedule, JSONL (bridge contract C8)")
    p.add_argument("--release", choices=sorted(releases.RELEASES), default="dos-vga", help="game release (default dos-vga)")
    p.add_argument("--seed", type=_non_negative_int, default=1, metavar="N", help="ScummVM random seed (default 1)")
    p.add_argument("--max-ticks", type=_positive_int, default=20000, metavar="N", help="tick cap from boot (default 20000)")
    p.add_argument("--start-frame", type=_positive_int, metavar="N",
                   help="segment start frame (default: the first esc input, i.e. the logo skip)")
    p.add_argument("--goal", metavar="JSON", help="C3 goal conditions, e.g. '[{\"room\": 28}]'")
    p.add_argument("--no-skips", action="store_true", help="no automatic text and cutscene skips")
    p.add_argument("--frame-log", action="store_true", help="also write frames.jsonl")
    p.add_argument("--set", action="append", default=[], metavar="KEY=VALUE",
                   help="extra ScummVM game setting, e.g. music_driver=adlib (repeatable)")

    p = sub.add_parser("measure", help="replay a plan headless on many seeds and report per-action ticks")
    p.add_argument("segment", help="segment name, e.g. part1")
    p.add_argument("--seeds", type=_seed_spec, default=measure.parse_seeds(measure.DEFAULT_SEEDS), metavar="SPEC",
                   help=f"seeds, e.g. 1-30 or 1,2,5 (default {measure.DEFAULT_SEEDS})")  # fmt: skip
    p.add_argument("--jobs", type=_positive_int, metavar="N", help="parallel runs (default: CPUs - 2)")
    p.add_argument("--plan", type=Path, metavar="FILE",
                   help="a compiled .jsonl, or a Fast Downward plan to compile (default: the segment's plan)")  # fmt: skip
    p.add_argument("--no-skips", action="store_true", help="turn text and cutscene skipping off")
    p.add_argument("--objective", choices=OBJECTIVES, help="which segment plan to measure (default as for run)")

    p = sub.add_parser("optimize", help="search for the plan with the lowest mean ticks over many seeds")
    p.add_argument("segment", help="segment name, e.g. part1")
    for flag, default, text in (
        ("--explore-seeds", "1-5", "seeds that measure each optimistic plan"),
        ("--select-seeds", "1-30", "seeds that pick the winner among the candidates"),
        ("--report-seeds", "31-60", "held-out seeds for the winner and runner-up"),
    ):
        p.add_argument(flag, type=_seed_spec, default=measure.parse_seeds(default), metavar="SPEC",
                       help=f"{text} (default {default})")  # fmt: skip
    p.add_argument("--candidates", type=_non_negative_int, default=20, metavar="K",
                   help="plans from sampled cost vectors (default 20)")  # fmt: skip
    p.add_argument("--jobs", type=_positive_int, metavar="N", help="parallel runs (default: CPUs - 2)")
    p.add_argument("--max-iterations", type=_positive_int, default=40, metavar="N",
                   help="cap on the optimistic measure-and-replan loop (default 40)")  # fmt: skip
    p.add_argument("--keyed", action=argparse.BooleanOptionalAction, default=True,
                   help="cost actions by ego's position before them, not by one pooled mean "
                        "(default: keyed)")  # fmt: skip
    p.add_argument("--key-rooms", choices=("flagged", "all"), default="flagged",
                   help="keyed: track the position only in rooms whose actions vary with it (flagged, default), "
                        "or everywhere (all: slower planning)")  # fmt: skip
    p.add_argument("--perturb", type=_fraction, default=0.1, metavar="P",
                   help="also plan K candidates from mean costs each multiplied by U[1-P, 1+P] (default 0.1; "
                        "0: none)")  # fmt: skip
    p.add_argument("--reuse", action=argparse.BooleanOptionalAction, default=True,
                   help="pool earlier out/measure and out/optimize summaries of the same bridge, skips and "
                        "engine pins before the first iteration (default: reuse)")  # fmt: skip

    p = sub.add_parser("extract-check", help="validate the extraction fragments out/extract/<segment>/*.pddl")
    p.add_argument("segment", help="segment name, e.g. part1")

    p = sub.add_parser(
        "extract-merge",
        help="merge the accepted fragments into out/extract/<segment>/domain.pddl and problem.pddl",
    )
    p.add_argument("segment", help="segment name, e.g. part1")
    p.add_argument("--state", type=Path, metavar="FILE",
                   help="segment-start state dump for :init (C7 state-start.json; default: the newest "
                        "out/runs/*/state-start.json without a boot param)")  # fmt: skip

    p = sub.add_parser("pddl-diff", help="compare the hand-written model with the extracted one")
    p.add_argument("segment", help="segment name, e.g. part1")
    p.add_argument("--out", type=Path, metavar="FILE", help="report path (default: docs/extraction-diff.md)")

    # The top-level help lists every command with its options, not just the names.
    usages = [
        " ".join(sp.format_usage().removeprefix("usage: ").replace(" [-h]", "").split())
        for sp in sub.choices.values()
    ]
    parser.epilog = "commands and options:\n" + "\n".join(f"  {u}" for u in usages)
    return parser


# --- shared helpers ----------------------------------------------------------


def _utc_stamp() -> str:
    return datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")


def new_dir(parent: Path, name: str) -> Path:
    """Create ``parent/name``, with a ``-N`` suffix if that name is taken."""
    parent.mkdir(parents=True, exist_ok=True)
    for n in itertools.count():
        path = parent / (name if n == 0 else f"{name}-{n}")
        try:
            path.mkdir()
        except FileExistsError:
            continue
        return path
    raise AssertionError("unreachable")


def _new_run_dir(mode: str) -> Path:
    """``RUNS_DIR/<UTC>-<mode>``, with a ``-N`` suffix if that name is taken."""
    return new_dir(paths.RUNS_DIR, f"{_utc_stamp()}-{mode}")


def _launch(cfg) -> EngineResult:
    try:
        result = run_engine(cfg)
    except FileNotFoundError as e:  # no ScummVM build
        raise CommandError(str(e)) from e
    if result.timed_out:
        _warn(
            f"The engine timed out after {cfg.timeout_s:g}s of wall time and was killed.",
            f"log: {result.log_path}",
        )
    return result


def _load_run_trace(run_dir: Path) -> Trace:
    try:
        return load_trace(run_dir / "trace.jsonl")
    except TraceError as e:
        raise CommandError(f"{e}\nrun dir: {run_dir}\nengine log: {run_dir / 'stdout.log'}") from e


def _segment(name: str) -> Segment:
    try:
        return load_segment(name, base=paths.PDDL_DIR)
    except SegmentError as e:
        raise CommandError(str(e)) from e


def _objects_path() -> Path:
    return paths.OUT_DIR / "objects.json"


def _sas_plan_path(name: str) -> Path:
    return paths.PLANS_DIR / f"{name}.sas_plan"


def _jsonl_path(name: str) -> Path:
    return paths.PLANS_DIR / f"{name}.jsonl"


def time_sas_plan_path(name: str) -> Path:
    """The time-optimal plan chosen by ``speedrun optimize``."""
    return paths.PLANS_DIR / f"{name}.time.sas_plan"


def time_jsonl_path(name: str) -> Path:
    return paths.PLANS_DIR / f"{name}.time.jsonl"


def costs_path(seg: Segment) -> Path:
    """The pooled measured cost table (derived data, committed)."""
    return seg.dir / "measured-costs.json"


def time_plan_inputs(seg: Segment) -> list[Path]:
    """What the time plan depends on: the model, its steps, the measured costs and the object dump."""
    return [*_plan_inputs(seg), seg.steps, costs_path(seg), _objects_path()]


def _engine_mismatch(table_path: Path) -> str | None:
    """Why the measured costs do not fit the engine's current timing pins or bridge, or None."""
    try:
        data = json.loads(table_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        return f"cannot read {table_path}: {e}"
    measured = data.get("engine") if isinstance(data, dict) else None
    current = timing_settings()
    if measured != current:
        return f"{table_path.name} was measured with engine settings {measured}, but the engine now pins {current}"
    # A rebuilt bridge can change timing and bumps its marker when it does, so the
    # costs must come from the bridge that is built now. An unreadable binary
    # (None) cannot be checked here; the run itself then fails to start.
    bridges = data.get("bridges") or []
    built = bridge_version()
    if bridges and built is not None and bridges != [built]:
        return f"{table_path.name} was measured with {', '.join(bridges)}, but the built bridge is {built}"
    return None


def time_plan_staleness(name: str, seg: Segment) -> str | None:
    """Why the time plan cannot be used (out of date, or measured under other engine pins), or None."""
    return _staleness(time_jsonl_path(name), time_plan_inputs(seg)) or _engine_mismatch(costs_path(seg))


def _plan_inputs(seg: Segment) -> list[Path]:
    return [seg.domain, seg.problem, seg.dir / "segment.toml"]


def _stale(target: Path, inputs: Iterable[Path]) -> bool:
    """True if ``target`` is missing, or any input is missing or newer than it."""
    return _staleness(target, inputs) is not None


def _staleness(target: Path, inputs: Iterable[Path]) -> str | None:
    """Why ``target`` is out of date (it or an input is missing, or an input is newer), or None."""
    if not target.is_file():
        return f"no {target}"
    built = target.stat().st_mtime_ns
    for p in inputs:
        if not p.is_file():
            return f"no {p}"
        if p.stat().st_mtime_ns > built:
            return f"{p.name} is newer than {target.name}"
    return None


def _mtime(path: Path) -> int | None:
    try:
        return path.stat().st_mtime_ns
    except FileNotFoundError:
        return None


def _newest(paths: Iterable[Path]) -> int:
    """The newest mtime (ns) among ``paths`` that exist; 0 if none do."""
    return max((t for t in map(_mtime, paths) if t is not None), default=0)


def _outdate_if_edited(output: Path, inputs: Iterable[Path], before: int) -> None:
    """Back-date ``output`` if an input changed while it was being built.

    ``before`` is the newest input mtime, recorded before the build read its
    inputs. An input edited during the build is newer than ``before`` but older
    than the output just written, so ``_stale`` would take the output as up to
    date. Setting the output's mtime to ``before`` puts it behind the edit, so
    the next command rebuilds it.
    """
    changed = [p for p in inputs if (t := _mtime(p)) is None or t > before]
    if not changed:
        return
    _warn(
        f"{', '.join(p.name for p in changed)} changed while {output.name} was being built.",
        f"{output} may predate that edit, so it is marked out of date and will be rebuilt next time.",
    )
    os.utime(output, ns=(output.stat().st_atime_ns, before))


# --- dump-objects ------------------------------------------------------------


class _BadDump(Exception):
    """An engine object dump that must not replace ``OUT_DIR/objects.json``."""


def _check_dump(path: Path) -> tuple[int, int]:
    """Validate an engine object dump (C6) and return its (rooms, verbs) counts.

    Raises ``_BadDump`` naming the problem. ``dump_objects`` checks the run
    dir's dump with this before it replaces ``OUT_DIR/objects.json``, so a bad
    dump never clobbers a good one.
    """
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        raise _BadDump(f"cannot read it as JSON: {e}") from e
    if not isinstance(data, dict):
        raise _BadDump(f"expected a JSON object, got {type(data).__name__}")
    rooms, verbs = data.get("rooms"), data.get("verbs")
    if not isinstance(rooms, list) or not rooms:
        raise _BadDump(f"'rooms' must be a non-empty list, got {rooms!r:.60}")
    if not isinstance(verbs, list):
        raise _BadDump(f"'verbs' must be a list, got {verbs!r:.60}")
    try:
        ObjectIndex.from_dict(data)  # what `compile` will need from it
    except CompileError as e:
        raise _BadDump(str(e)) from e
    return len(rooms), len(verbs)


def dump_objects(max_ticks: int | None = None) -> Path:
    """Run the engine's object/verb dump and copy it to ``OUT_DIR/objects.json``."""
    run_dir = _new_run_dir("dump")
    print(f"dumping objects and verbs (run dir {run_dir}) ...")
    cfg = EngineConfig(
        out_dir=run_dir,
        dump_objects=True,
        fast=True,
        max_ticks=max_ticks or DUMP_MAX_TICKS,
        timeout_s=DUMP_TIMEOUT_S,
    )
    _launch(cfg)
    trace = _load_run_trace(run_dir)
    dumped = run_dir / "objects.json"
    reason = trace.end.get("reason") if trace.end else None
    if reason != "dump_done" or not dumped.is_file():
        lines = [f"object dump failed: end reason {reason or 'unknown (no end record)'}"]
        if reason == "dump_done":
            lines.append(f"the bridge reported dump_done but wrote no {dumped}")
        lines += [format_error(e) for e in trace.errors]
        lines += [f"run dir: {run_dir}", f"engine log: {run_dir / 'stdout.log'}"]
        raise CommandError("\n".join(lines))

    dest = _objects_path()
    try:
        rooms, verbs = _check_dump(dumped)
    except _BadDump as e:
        raise CommandError(
            f"{dumped} is not a valid object dump: {e}\n"
            f"{dest} was left unchanged\nrun dir: {run_dir}\nengine log: {run_dir / 'stdout.log'}"
        ) from e
    dest.parent.mkdir(parents=True, exist_ok=True)
    # copyfile (not copy2) so the copy is newer than any jsonl compiled from an older dump;
    # write-then-rename so a reader never sees half a file.
    tmp = dest.with_name(dest.name + ".tmp")
    shutil.copyfile(dumped, tmp)
    tmp.replace(dest)
    print(f"wrote {dest}: {rooms} rooms, {verbs} verbs")
    return dest


def _cmd_dump_objects(args: argparse.Namespace) -> int:
    dump_objects(args.max_ticks)
    return 0


# --- plan --------------------------------------------------------------------


def _print_plan(plan: Plan, plan_file: Path) -> None:
    print(f"plan ({plan_file}):")
    width = len(str(len(plan.actions)))
    for i, action in enumerate(plan.actions, 1):
        print(f"  {i:>{width}}. {' '.join(action)}")
    st = plan_stats(plan)
    print(f"cost={st.cost}, actions={st.actions}, transitions={st.transitions}")


def plan_segment(name: str, seg: Segment) -> Plan:
    """Run Fast Downward on the segment, write ``PLANS_DIR/<name>.sas_plan`` and print the plan."""
    for path in (seg.domain, seg.problem):
        if not path.is_file():
            raise CommandError(f"segment {name!r}: {path} not found")
    plan_file = _sas_plan_path(name)
    inputs = _plan_inputs(seg)
    before = _newest(inputs)
    print(f"planning {name} with Fast Downward ...")
    try:
        plan = run_planner(seg.domain, seg.problem, plan_file)
    except PlannerError as e:
        raise CommandError(f"{e}\nplanner log: {e.log_path}") from e
    _outdate_if_edited(plan_file, inputs, before)
    _print_plan(plan, plan_file)
    return plan


def plan_segment_timed(name: str, seg: Segment) -> Plan:
    """Plan a timed copy of the model, costed from ``measured-costs.json``, under ``PLANS_DIR/<name>-timed/``.

    This is the plan that is optimal for the surrogate (mean-cost) model. It does
    not replace the time plan, which ``speedrun optimize`` picks by measurement.
    """
    table_path = costs_path(seg)
    if not table_path.is_file():
        raise CommandError(f"no {table_path}: measure the costs first with `speedrun optimize {name}`")
    mismatch = _engine_mismatch(table_path)
    if mismatch:
        _warn(f"{mismatch}:", "the timed model's costs do not match today's engine (re-run speedrun optimize).")
    try:
        table = CostTable.load(table_path)
        out = paths.PLANS_DIR / f"{name}-timed"
        domain, problem = write_timed(seg.domain, seg.problem, table.means(), out)
    except (CostingError, OSError) as e:
        raise CommandError(f"cannot build the timed model: {e}") from e
    plan_file = out / f"{name}.sas_plan"
    print(f"planning {name} with Fast Downward on the timed model {out} ...")
    try:
        plan = run_planner(domain, problem, plan_file)
    except PlannerError as e:
        raise CommandError(f"{e}\nplanner log: {e.log_path}") from e
    _print_plan(plan, plan_file)
    print(f"objective: time (costs: mean ticks from {table_path}, {len(table.actions())} measured actions; "
          "unmeasured actions cost 1)")  # fmt: skip
    return plan


def _cmd_plan(args: argparse.Namespace) -> int:
    seg = _segment(args.segment)
    if getattr(args, "objective", "actions") == "time":
        plan_segment_timed(args.segment, seg)
    else:
        plan_segment(args.segment, seg)
    return 0


# --- compile -----------------------------------------------------------------


def _current_plan(name: str, seg: Segment, replan: bool) -> Plan:
    plan_file = _sas_plan_path(name)
    if replan or _stale(plan_file, _plan_inputs(seg)):
        return plan_segment(name, seg)
    try:
        text = plan_file.read_text(encoding="utf-8")
    except (OSError, ValueError) as e:
        raise CommandError(f"cannot read plan {plan_file}: {e}; re-plan with `speedrun plan {name}`") from e
    if not plan_is_complete(text):
        print(f"plan {plan_file} is incomplete (no '; cost = N' trailer): re-planning")
        return plan_segment(name, seg)
    try:
        plan = parse_plan(text)
    except ValueError as e:
        raise CommandError(f"cannot read plan {plan_file}: {e}; re-plan with `speedrun plan {name}`") from e
    print(f"plan {plan_file} is up to date ({len(plan.actions)} actions, cost {plan.cost})")
    return plan


def compile_segment(name: str, seg: Segment, replan: bool = False) -> Path:
    """Bring objects.json and the plan up to date, then write ``PLANS_DIR/<name>.jsonl``."""
    objects_path, plan_file = _objects_path(), _sas_plan_path(name)
    sources = [*_plan_inputs(seg), seg.steps]
    # Recorded before planning, so an edit made while Fast Downward runs still counts.
    sources_before = _newest(sources)
    if not objects_path.is_file():
        print(f"{objects_path} is missing: dumping objects first")
        dump_objects()
    plan = _current_plan(name, seg, replan)
    if not seg.steps.is_file():
        raise CommandError(f"segment {name!r}: {seg.steps} not found")
    # The dump and the plan may have just been written by this build; from here on they must not change.
    before = max(sources_before, _newest([objects_path, plan_file]))
    try:
        templates = load_steps(seg.steps)
    except CompileError as e:  # names the steps file
        raise CommandError(f"cannot load steps: {e}") from e
    try:
        objects = ObjectIndex.from_dump(objects_path)
    except (OSError, ValueError, CompileError) as e:  # unreadable, not JSON, or not an object dump
        raise CommandError(f"cannot read {objects_path}: {e}; re-dump with `speedrun dump-objects`") from e
    try:
        steps = compile_plan(plan, templates, objects)
    except CompileError as e:
        raise CommandError(f"compile failed: {e}") from e
    out = _jsonl_path(name)
    write_jsonl(steps, out)
    _outdate_if_edited(out, [*sources, objects_path, plan_file], before)
    print(f"wrote {out}: {len(steps)} steps")
    return out


def _cmd_compile(args: argparse.Namespace) -> int:
    compile_segment(args.segment, _segment(args.segment))
    return 0


# --- run / demo --------------------------------------------------------------


def _ensure_compiled(name: str, seg: Segment, replan: bool) -> Path:
    jsonl = _jsonl_path(name)
    inputs = [*_plan_inputs(seg), seg.steps, _objects_path(), _sas_plan_path(name)]
    if replan or _stale(jsonl, inputs):
        return compile_segment(name, seg, replan=replan)
    print(f"compiled plan {jsonl} is up to date")
    return jsonl


def select_plan(name: str, seg: Segment, objective: str | None, replan: bool = False) -> tuple[str, Path]:
    """The objective and the compiled plan to replay; prints which and why.

    ``objective`` None picks the time plan when it is fresh (see the module doc),
    else the action-count plan, which is recompiled if stale. ``replan`` re-plans
    the action-count model, so it never picks the time plan.
    """
    time_jsonl = time_jsonl_path(name)
    why_stale = time_plan_staleness(name, seg)
    if objective == "time":
        if replan:
            raise CommandError(
                "--replan re-plans the action-count model; the time plan comes from "
                f"`speedrun optimize {name}` (drop --replan, or use --objective actions)"
            )
        if why_stale:
            raise CommandError(f"no fresh time plan ({why_stale}); run `speedrun optimize {name}`")
        print(f"objective: time (requested; plan {time_jsonl} from speedrun optimize)")
        return "time", time_jsonl
    if objective is None and not replan and why_stale is None:
        print(f"objective: time (plan {time_jsonl} from speedrun optimize is fresh)")
        return "time", time_jsonl
    if objective == "actions":
        reason = "requested"
    elif replan:
        reason = "--replan re-plans the action-count model"
    else:
        reason = f"no fresh time plan: {why_stale}"
    jsonl = _ensure_compiled(name, seg, replan)
    print(f"objective: actions ({reason}; plan {jsonl})")
    return "actions", jsonl


def _engine_supports(field_name: str) -> bool:
    return dataclasses.is_dataclass(EngineConfig) and any(
        f.name == field_name for f in dataclasses.fields(EngineConfig)
    )


def _boot_param_warning(value: int) -> None:
    _warn(
        f"--boot-param {value}: boot params force ScummVM debug mode, and MI1's boot script",
        "then takes debug starts that set game state directly (docs/plan.md C1).",
        "This run is NOT a valid measured run (rules/glitchless.md).",
    )


def _check_audio_pump(trace: Trace) -> None:
    pump = trace.boot.get("audio_pump") if trace.boot else None
    if pump is not True:
        state = "has no boot record" if trace.boot is None else f"has audio_pump={json.dumps(pump)}"
        _warn(
            f"AUDIO PUMP OFF: the trace {state}. The bridge did not pump the null",
            "mixer on game ticks, so sound and music timing followed wall-clock time.",
            "These ticks are not comparable with other runs.",
        )


def _print_randomized_vars(seg: Segment, run_dir: Path) -> None:
    if not seg.randomized_vars:
        return
    state_path = run_dir / "state-start.json"
    try:
        state = json.loads(state_path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        print(f"randomized vars: {state_path} was not written (the segment never started)")
        return
    except (OSError, ValueError) as e:
        print(f"randomized vars: cannot read {state_path}: {e}")
        return
    values = state.get("vars") if isinstance(state, dict) else None
    print(f"randomized vars ({state_path.name}):")
    for entry in seg.randomized_vars:
        n = entry["var"]
        value = values[n] if isinstance(values, list) and 0 <= n < len(values) else "?"
        print(f"  var {n} = {value}  ({entry['cite']})")


def run_segment(name: str, mode: str, args: argparse.Namespace) -> int:
    """Shared by ``run`` (headless, fast) and ``demo`` (windowed, real time).

    The demo fast-forwards the boot to the segment start unless ``--no-fast-boot``
    (``EngineConfig.fast_boot``); only wall-clock pacing changes, so ticks match ``run``.
    """
    seg = _segment(name)
    # ScummVM treats boot param 0 as "no boot param": not passed, not a debug start, a measured run.
    boot_param = args.boot_param or None
    if boot_param is not None:
        _boot_param_warning(boot_param)
    objective, jsonl = select_plan(name, seg, getattr(args, "objective", None), args.replan)

    run_dir = _new_run_dir(mode)
    release = releases.get(getattr(args, "release", releases.DEFAULT_RELEASE))
    config = {
        "out_dir": run_dir,
        "plan": jsonl,
        "start": seg.start,
        "goal": seg.goal,
        "seed": args.seed,
        "max_ticks": args.max_ticks,
        "boot_param": boot_param,
        "headless": mode == "run",
        "fast": mode == "run",
        "fast_boot": getattr(args, "fast_boot", False),  # demo only
        "timeout_s": RUN_TIMEOUT_S[mode],
    }
    if release.name != releases.DEFAULT_RELEASE:  # the default is EngineConfig's own
        try:
            config.update(release=release.name, game_path=releases.game_dir(release))
        except releases.ReleaseError as e:
            raise CommandError(str(e)) from e
    if seg.interrupts:
        config["interrupts"] = seg.interrupts
    if seg.inventory is not None:
        if _engine_supports("inventory"):
            config["inventory"] = {k: v for k, v in seg.inventory.items() if k != "cite"}
        else:
            _warn(
                "EngineConfig has no 'inventory' field, so SPEEDRUN_INVENTORY is not set:",
                "plan steps that click inventory slots will fail with click_target_missing.",
            )
    print(f"{mode} {name}: release {release.name}, seed {args.seed}, objective {objective}, plan {jsonl}, run dir {run_dir}")
    _launch(EngineConfig(**config))

    trace = _load_run_trace(run_dir)
    print(format_table(trace))
    print()
    lines = summary(trace).splitlines()
    if boot_param is not None:
        lines[0] += f" {NOT_MEASURED}"
    print("\n".join(lines))
    _check_audio_pump(trace)
    _print_randomized_vars(seg, run_dir)
    print(f"run dir: {run_dir}")
    if boot_param is not None:
        _boot_param_warning(boot_param)
    return 0 if trace.reached_goal else 1


def _cmd_run(args: argparse.Namespace) -> int:
    return run_segment(args.segment, "run", args)


def _cmd_demo(args: argparse.Namespace) -> int:
    return run_segment(args.segment, "demo", args)


# --- measure -------------------------------------------------------------------


def _ensure_objects() -> Path:
    objects_path = _objects_path()
    if not objects_path.is_file():
        print(f"{objects_path} is missing: dumping objects first")
        dump_objects()
    return objects_path


def compile_plan_file(plan_file: Path, seg: Segment, out: Path) -> Path:
    """Compile a Fast Downward plan file through the segment's steps.toml into ``out`` (a jsonl)."""
    try:
        plan = parse_plan(plan_file.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        raise CommandError(f"cannot read plan {plan_file}: {e}") from e
    objects_path = _ensure_objects()
    try:
        steps = compile_plan(plan, load_steps(seg.steps), ObjectIndex.from_dump(objects_path))
    except (OSError, ValueError, CompileError) as e:
        raise CommandError(f"cannot compile {plan_file}: {e}") from e
    write_jsonl(steps, out)
    return out


def _print_progress(result: measure.RunResult) -> None:
    if result.ok:
        print(f"  seed {result.seed}: {result.total_ticks} ticks ({format_ticks(result.total_ticks)})", flush=True)
    else:
        failure = result.failure or {}
        where = f" at {failure['action']!r}" if failure.get("action") else ""
        print(f"  seed {result.seed}: FAILED ({failure.get('reason')}{where})", flush=True)


def run_measure(name: str, seg: Segment, plan: Path, seeds: list[int], out_dir: Path, jobs: int,
                skips: bool = True, positions=None) -> dict:  # fmt: skip
    """``measure.measure_plan`` with progress lines, mapping its errors to ``CommandError``.

    ``positions`` (``speedrun.positions.Positions``) adds each instance's position context.
    """
    print(f"measuring {plan} on seeds {measure.format_seeds(seeds)} ({len(seeds)} runs, {jobs} at a time, "
          f"skips {'on' if skips else 'off'}) in {out_dir} ...", flush=True)  # fmt: skip
    try:
        return measure.measure_plan(plan, seg, seeds, out_dir, jobs=jobs, skips=skips, progress=_print_progress,
                                    positions=positions)  # fmt: skip
    except FileNotFoundError as e:  # no ScummVM build
        raise CommandError(str(e)) from e
    except measure.MeasureError as e:
        raise CommandError(str(e)) from e


def _positions_or_none(seg: Segment):
    """The segment's position anchors for the instances' ``context`` field, or None (with a note)."""
    from speedrun.positions import PositionError, load_positions

    try:
        return load_positions(seg.domain, seg.problem, seg.steps, _objects_path())
    except PositionError as e:
        print(f"note: no position contexts ({e})")
        return None


def _cmd_measure(args: argparse.Namespace) -> int:
    name = args.segment
    seg = _segment(name)
    jobs = args.jobs or default_jobs()
    if args.plan is not None:
        plan = args.plan
        if not plan.is_file():
            raise CommandError(f"no plan file {plan}")
    else:
        _, plan = select_plan(name, seg, args.objective)
    out_dir = new_dir(paths.OUT_DIR / "measure", _utc_stamp())
    if plan.suffix != ".jsonl":
        plan = compile_plan_file(plan, seg, out_dir / "plan.jsonl")
    summary = run_measure(name, seg, plan, args.seeds, out_dir, jobs, skips=not args.no_skips,
                          positions=_positions_or_none(seg))  # fmt: skip
    print()
    print(measure.format_summary(summary))
    print(f"summary: {out_dir / 'summary.json'}")
    total = summary["total"]
    if total["n_fail"]:
        seeds = ", ".join(str(f["seed"]) for f in summary["failures"])
        _warn(f"{total['n_fail']} of {total['n_ok'] + total['n_fail']} seeds failed: {seeds}",
              f"summary: {out_dir / 'summary.json'}")  # fmt: skip
        return 1
    return 0


# --- optimize ------------------------------------------------------------------


def _cmd_optimize(args: argparse.Namespace) -> int:
    from speedrun import optimize  # the optimiser imports this module

    name = args.segment
    seg = _segment(name)
    settings = optimize.Settings(
        explore_seeds=args.explore_seeds,
        select_seeds=args.select_seeds,
        report_seeds=args.report_seeds,
        candidates=args.candidates,
        jobs=args.jobs or default_jobs(),
        max_iterations=args.max_iterations,
        keyed=args.keyed,
        key_rooms=args.key_rooms,
        perturb=args.perturb,
        reuse=args.reuse,
    )
    for path in (seg.domain, seg.problem, seg.steps):
        if not path.is_file():
            raise CommandError(f"segment {name!r}: {path} not found")
    try:
        return optimize.optimize(name, seg, _ensure_objects(), settings)
    except optimize.OptimizeError as e:
        raise CommandError(str(e)) from e


# --- extract -----------------------------------------------------------------


def _cmd_extract(args: argparse.Namespace) -> int:
    # Read paths at call time (not via extract()'s defaults) so tests can monkeypatch them.
    game_dir, dest = paths.GAME_DIR, paths.CLASSIC_DIR
    det = gamedata.detect_layout(game_dir)
    print(f"layout: {det.layout.name} ({det.path or 'nothing found under ' + str(game_dir)})")

    if det.layout is gamedata.Layout.SE_PAK:
        print(
            "ScummVM reads Monkey1.pak directly, so nothing was extracted. "
            "Script dumping (descumm) needs the classic MONKEY1.000/.001 files, "
            "which v1 cannot get from the Special Edition pak."
        )
        return 0

    try:
        written = gamedata.extract(game_dir, dest)
    except (gamedata.GameDataMissing, gamedata.ExtractionError) as e:
        print(f"speedrun extract: {e}", file=sys.stderr)
        return 1

    if not written:
        print(f"nothing to write: the classic files already live in {dest}")
    for path in written:
        print(f"wrote {path} ({path.stat().st_size} bytes)")
        if path.name == gamedata.INDEX_NAME:
            digest = gamedata.md5(path)
            label = gamedata.EXPECTED_INDEX_MD5.get(digest)
            if label:
                print(f"  md5 {digest}: {label}")
            else:
                print(f"  md5 {digest}")
                print(
                    f"speedrun extract: warning: unknown {gamedata.INDEX_NAME} md5 {digest}; "
                    f"expected one of {', '.join(gamedata.EXPECTED_INDEX_MD5)} "
                    "(continuing, but this variant is untested)",
                    file=sys.stderr,
                )
    return 0


# --- extraction checks, merge and diff (Phase 7) --------------------------------


def _extract_dir(name: str) -> Path:
    """Where extraction fragments, and the model merged from them, live."""
    return paths.OUT_DIR / "extract" / name


def _scripts_dir() -> Path:
    return paths.DATA_DIR / "scripts"


def _fragments(name: str) -> dict[str, str]:
    directory = _extract_dir(name)
    try:
        frags = extract.load_fragments(directory)
    except (OSError, UnicodeDecodeError) as e:
        raise CommandError(f"cannot read the fragments in {directory}: {e}") from e
    if not frags:
        raise CommandError(f"no extraction fragments in {directory} (<group>.pddl files, format: speedrun/citations.py)")
    return frags


def _cmd_extract_check(args: argparse.Namespace) -> int:
    directory = _extract_dir(args.segment)
    frags = _fragments(args.segment)
    issues = check_fragments(frags, _scripts_dir())
    for issue in issues:
        print(issue)
    actions = sum(len(parse_fragment(text).actions) for text in frags.values())
    print(f"{directory}: {len(frags)} fragment(s), {actions} action(s), {len(issues)} issue(s)")
    return 1 if issues else 0


def _latest_state_dump() -> Path:
    """The newest ``RUNS_DIR/*/state-start.json`` of a run without a boot param (a real segment start)."""
    for path in sorted(paths.RUNS_DIR.glob("*/state-start.json"), key=lambda p: p.parent.name, reverse=True):
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if isinstance(data, dict) and not data.get("boot_param"):
            return path
    raise CommandError(
        f"no segment-start state dump (state-start.json without a boot param) under {paths.RUNS_DIR}: "
        "run `speedrun run <segment>` first, or pass --state FILE"
    )


def _cmd_extract_merge(args: argparse.Namespace) -> int:
    seg = _segment(args.segment)
    directory = _extract_dir(args.segment)
    frags = _fragments(args.segment)
    accepted, rejected = filter_fragments(frags, _scripts_dir())
    rejected_path = directory / "rejected.json"
    entries = [
        {"fragment": src, "action": name, "issues": [{"code": i.code, "line": i.line, "message": i.message}
                                                     for i in issues]}
        for src, name, issues in rejected
    ]  # fmt: skip
    rejected_path.write_text(json.dumps(entries, indent=2) + "\n", encoding="utf-8")
    for src, name, issues in rejected:
        print(f"rejected {src}: {name}")
        for issue in issues:
            print(f"  line {issue.line}: {issue.message} [{issue.code}]")
    kept = sum(len(parse_fragment(text).actions) for text in accepted.values())
    if not kept:
        raise CommandError(f"every extracted action was rejected; see {rejected_path}")

    state_path = args.state or _latest_state_dump()
    print(f"state dump: {state_path}")
    try:
        domain, problem = extract.merge(accepted, state_path, seg.goal, name=f"{seg.name}-extracted")
    except extract.ExtractError as e:
        raise CommandError(str(e)) from e
    domain_path, problem_path = directory / "domain.pddl", directory / "problem.pddl"
    domain_path.write_text(domain, encoding="utf-8")
    problem_path.write_text(problem, encoding="utf-8")
    print(f"wrote {domain_path} ({kept} actions), {problem_path} and {rejected_path} ({len(rejected)} rejected)")
    return 0


def _diff_index() -> ObjectIndex:
    """The engine's object dump if there is one, else the script dump's index."""
    objects_path = _objects_path()
    index_path = _scripts_dir() / "index.json"
    try:
        if objects_path.is_file():
            return ObjectIndex.from_dump(objects_path)
        if index_path.is_file():
            return pddl_diff.index_from_script_dump(index_path)
    except (OSError, ValueError, KeyError, CompileError) as e:
        raise CommandError(f"cannot read the object index: {e}") from e
    raise CommandError(f"no object index: neither {objects_path} (`speedrun dump-objects`) "
                       f"nor {index_path} (scripts/dump-scripts.sh)")  # fmt: skip


def _relative(path: Path) -> Path:
    """``path`` relative to the repository, for text that gets committed; absolute if it lies outside."""
    try:
        return path.relative_to(paths.ROOT)
    except ValueError:
        return path


def _cmd_pddl_diff(args: argparse.Namespace) -> int:
    seg = _segment(args.segment)
    frags = _fragments(args.segment)
    accepted, rejected = filter_fragments(frags, _scripts_dir())
    index = _diff_index()
    try:
        report = pddl_diff.diff(seg.domain, seg.problem, load_steps(seg.steps), index, accepted)
    except (OSError, ValueError, CompileError, extract.ExtractError) as e:
        raise CommandError(f"cannot compare the models: {e}") from e
    notes = [
        (
            f"Hand model: `{seg.domain.name}`, `{seg.problem.name}`, `{seg.steps.name}` of segment `{seg.name}`. "
            f"Extracted model: the accepted fragments in `{_relative(_extract_dir(args.segment))}`."
        )
    ]
    if rejected:
        notes.append(f"{len(rejected)} extracted action(s) rejected by the validator are not compared "
                     f"(`speedrun extract-check {args.segment}`).")  # fmt: skip
    out = args.out or paths.DOCS_DIR / "extraction-diff.md"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(report.to_markdown(title=f"Extraction diff: {seg.name}", notes=notes), encoding="utf-8")
    print(f"wrote {out}: {len(report.matched)} matched, {len(report.hand_only)} hand-only and "
          f"{len(report.extracted_only)} extracted-only signatures")  # fmt: skip
    return 0


def _cmd_probe(args: argparse.Namespace) -> int:
    rel = releases.get(args.release)
    try:
        game = releases.game_dir(rel)
    except releases.ReleaseError as e:
        raise CommandError(str(e)) from e
    start = args.start_frame
    if start is None and args.inputs:
        firsts = [json.loads(line) for line in args.inputs.read_text().splitlines() if line.strip()]
        start = next((e["frame"] for e in firsts if e.get("key") == "esc"), None)
    settings = {}
    for item in args.set:
        key, sep, value = item.partition("=")
        if not sep or not key:
            raise CommandError(f"--set {item!r}: expected KEY=VALUE")
        settings[key] = value
    goal = json.loads(args.goal) if args.goal else []
    out = _new_run_dir("probe")
    cfg = EngineConfig(
        out_dir=out, game_path=game, release=rel.name, seed=args.seed, max_ticks=args.max_ticks,
        raw_inputs=args.inputs, timer_next="any", start_frame=start, goal=goal,
        skip_text=not args.no_skips, skip_cutscenes=not args.no_skips, frame_log=args.frame_log,
        game_settings=settings,
    )  # fmt: skip
    res = probe.probe(cfg)
    print(probe.report(res))
    print(f"run dir: {out}")
    return 0 if res.end.get("reason") in ("goal", "max_ticks") else 1


COMMANDS = {
    "extract": _cmd_extract,
    "probe": _cmd_probe,
    "dump-objects": _cmd_dump_objects,
    "plan": _cmd_plan,
    "compile": _cmd_compile,
    "run": _cmd_run,
    "demo": _cmd_demo,
    "measure": _cmd_measure,
    "optimize": _cmd_optimize,
    "extract-check": _cmd_extract_check,
    "extract-merge": _cmd_extract_merge,
    "pddl-diff": _cmd_pddl_diff,
}


_EXIT_SIGNALS = tuple(getattr(signal, name) for name in ("SIGTERM", "SIGHUP") if hasattr(signal, name))


def _exit_on_signal(signum: int, frame) -> None:
    # One shot: a second signal (e.g. SIGHUP, then SIGTERM) must not cut the cleanup short.
    for sig in _EXIT_SIGNALS:
        signal.signal(sig, signal.SIG_IGN)
    sys.exit(128 + signum)


@contextlib.contextmanager
def _signals_exit():
    """Turn SIGTERM and SIGHUP into ``SystemExit(128 + signum)`` for the duration.

    By default those signals kill the process outright, skipping the cleanup in
    ``run_planner`` and ``run_engine`` (``except BaseException``): Fast Downward
    runs in its own session, so its process group would be orphaned, and
    ScummVM would keep running. As an exception, the cleanup runs. A signal that
    is already ignored (SIGHUP under nohup) stays ignored. The previous handlers
    are restored on exit.
    """
    if threading.current_thread() is not threading.main_thread():
        yield  # only the main thread may set handlers
        return
    previous = {}
    for sig in _EXIT_SIGNALS:
        handler = signal.getsignal(sig)
        if handler != signal.SIG_IGN:
            previous[sig] = handler
            signal.signal(sig, _exit_on_signal)
    try:
        yield
    finally:
        for sig, handler in previous.items():
            # None: a handler not installed from Python; the default is the best approximation.
            signal.signal(sig, signal.SIG_DFL if handler is None else handler)


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.command is None:
        parser.print_help()
        return 2
    with _signals_exit():
        try:
            return COMMANDS[args.command](args)
        except CommandError as e:
            print(f"speedrun {args.command}: {e}", file=sys.stderr)
            return 1


if __name__ == "__main__":
    raise SystemExit(main())
