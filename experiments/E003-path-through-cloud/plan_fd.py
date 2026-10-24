"""E003 step 4: run Fast Downward's lama-first on a task, read the plan and the landmarks (plan step 4).

usage: from plan_fd import plan
       result = plan(sas, start, time_limit=600)    # sas: task.SasTask; start: a facts.py map
       result.status, result.steps, result.landmarks, result.orderings

**The planner.** Fast Downward (``third_party/downward``, release 26.6.0, built with ``./build.py release``),
its search component on our file: the driver runs search only when the one input file looks like translator
output (driver/arguments.py:205-217). The search is lama-first (driver/aliases.py:127-132): lazy greedy best
first search with the FF heuristic and the landmark sum heuristic over ``lm_reasonable_orders_hps(lm_rhw())``,
preferred operators from both, unit costs. It is deterministic. The search string is the alias's own, with
one change: ``verbosity=debug`` on the landmark factory, which makes it dump the landmark graph it built
(landmark_factory.cc:139-141, landmarks/util.cc:82-147); that changes what is logged, not the search
(test_task.py runs both and compares the plans). Each call gets ``time_limit`` seconds (600 per replan, plan
step 4) and ``memory_limit`` (32 GB: plan step 4 stops when the task cannot be built in it), and a fresh
directory of its own, since FD writes its plan and its log to files (kept when the caller names it, else
removed: the log and plan are in the Result).

**Result.** ``status`` from the exit code (docs/exit-codes.md): "plan" (0), "unsolvable" (11), "incomplete"
(12: the search ended without a plan), "memory" (22), "time" (23), "unsupported" (34), else "error"; the raw
code is kept. ``steps``: the plan's operators, each with its action, the precondition values its variant
took and the values it guessed (task.Op, with the pick operators folded in), the goal and pick operators
left out. ``landmarks``: each
node of the dump, its facts (key, value), whether it is disjunctive (``|``) or conjunctive (``&``), true at
the start (``bold``) and true in the goal (``filled``), and ``aux`` when it holds a pick or goal variable of
the encoding rather than a game fact; ``orderings``: (from, to, kind), kind one of "nec",
"gn", "n", "r". The facts are the task's, read back from the value names (task.atom).
"""

import json
import re
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

from task import GOAL_KEY, PICK, SasTask

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
DOWNWARD = ROOT / "third_party" / "downward"
DRIVER = DOWNWARD / "fast-downward.py"
BUILD = "release"

# driver/aliases.py:127-132, the "lama-first" alias, as one string
LAMA_FIRST = ("let(hlm,eval_modify_costs(landmark_sum(lm_factory=lm_reasonable_orders_hps(lm_rhw()),pref=false),"
              "cost_type=one),let(hff,eval_modify_costs(ff(),cost_type=one),lazy_greedy([hff,hlm],"
              "preferred=[hff,hlm],cost_type=one,reopen_closed=false)))")
# the same search, with the landmark factory's debug output on (module docstring)
LAMA_FIRST_LANDMARKS = LAMA_FIRST.replace("lm_reasonable_orders_hps(lm_rhw())",
                                          "lm_reasonable_orders_hps(lm_rhw(),verbosity=debug)")
STATUS = {0: "plan", 11: "unsolvable", 12: "incomplete", 22: "memory", 23: "time", 34: "unsupported"}

_NODE = re.compile(r'^\s*lm(\d+) \[label="(.*)"(.*)\];$')
_EDGE = re.compile(r'^\s*lm(\d+) -> lm(\d+) \[label="(\w+)"')
_ATOM = re.compile(r"^(?:Atom|NegatedAtom) \w+\((.*)=([^=()]*)\)$")


@dataclass
class Step:
    op: str  # the operator's name, a<N>
    action: int  # index into the task's actions
    name: str
    chosen: tuple  # ((key, value), ...): the precondition values of this variant
    guessed: tuple  # ((key, value, when), ...): values the reader could not work out, guessed here


@dataclass
class Landmark:
    id: int
    facts: list  # [(key, value)]
    disjunctive: bool
    conjunctive: bool
    initial: bool  # true at the start
    goal: bool  # true in the goal
    aux: bool = False  # holds a pick or goal variable of the encoding (task.py), not a game fact


@dataclass
class Result:
    status: str
    exit_code: int
    steps: list = field(default_factory=list)
    landmarks: list = field(default_factory=list)
    orderings: list = field(default_factory=list)  # (from id, to id, kind)
    seconds: float = 0.0
    workdir: str = ""
    log: str = ""


def _value(v: str):
    return int(v) if re.fullmatch(r"-?\d+", v) else v


def parse_landmarks(log: str) -> tuple[list, list]:
    """The landmark graph dumped between "Dumping landmark graph:" and "Landmark graph end." (landmarks/util.cc)."""
    nodes, edges, inside = [], [], False
    for line in log.splitlines():
        if "Dumping landmark graph" in line:
            inside, nodes, edges = True, [], []  # the last dump is the final graph
            continue
        if "Landmark graph end." in line:
            inside = False
            continue
        if not inside:
            continue
        if m := _NODE.match(line):
            label, style = m[2], m[3]
            disj, conj = " | " in label, " & " in label
            facts = []
            for part in re.split(r" \| | & ", label):
                a = _ATOM.match(part)
                facts.append((a[1], _value(a[2])) if a else (part, None))
            aux = any(isinstance(k, str) and (k == GOAL_KEY or k.startswith(PICK)) for k, _ in facts)
            nodes.append(Landmark(int(m[1]), facts, disj, conj, "bold" in style, "filled" in style, aux))
        elif m := _EDGE.match(line):
            edges.append((int(m[1]), int(m[2]), m[3]))
    return nodes, edges


def parse_plan(text: str, sas: SasTask) -> list:
    """The plan's steps: goal and pick operators left out, each pick folded into the steps that read it
    (task.py: a pick variable starts at its first value)."""
    steps, picks = [], {}
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("("):
            continue
        op = sas.op(line)
        if op.goal:
            continue
        if op.pick is not None:
            picks[op.pick[0]] = op.pick[1]
            continue
        a = sas.task.actions[op.action]
        guessed = op.guessed
        for key, vals, when in op.picked:
            u = picks.get(key, sas.facts[PICK + key].values[0])
            guessed += ((key, u if u in vals else vals[0], when),)
        steps.append(Step(op.name, op.action, a.name, op.chosen, guessed))
    return steps


def run(text: str, time_limit: int = 600, memory_limit: str = "32G", search: str = LAMA_FIRST_LANDMARKS,
        alias: str | None = None, workdir: str | None = None) -> tuple[int, str, str, float, str]:
    """Run the search component on a translator-output ``text``: (exit code, log, plan text, seconds, directory).
    With no ``workdir`` it runs in a temporary directory that is removed afterwards (directory "")."""
    if workdir is None:
        with tempfile.TemporaryDirectory(prefix="fd-") as tmp:
            return run(text, time_limit, memory_limit, search, alias, tmp)[:4] + ("",)
    work = Path(workdir)
    work.mkdir(parents=True, exist_ok=True)
    sas_file, plan_file = work / "task.sas", work / "sas_plan"
    sas_file.write_text(text)
    cmd = [sys.executable, str(DRIVER), "--build", BUILD, "--plan-file", str(plan_file),
           "--search-time-limit", f"{time_limit}s", "--search-memory-limit", memory_limit]
    cmd += ["--alias", alias, str(sas_file)] if alias else [str(sas_file), "--search", search]
    t0 = time.monotonic()
    p = subprocess.run(cmd, cwd=work, capture_output=True, text=True)
    seconds = time.monotonic() - t0
    log = p.stdout + p.stderr
    (work / "fd.log").write_text(log)
    return p.returncode, log, plan_file.read_text() if plan_file.exists() else "", seconds, str(work)


def plan(sas: SasTask, start: dict, time_limit: int = 600, memory_limit: str = "32G",
         workdir: str | None = None) -> Result:
    """lama-first on ``sas`` from ``start`` (module docstring)."""
    code, log, text, seconds, work = run(sas.text(start), time_limit, memory_limit, workdir=workdir)
    if workdir is not None:  # the operator table next to task.sas, for a replay in another process
        (Path(workdir) / "ops.json").write_text(json.dumps(sas.op_table()))
    nodes, edges = parse_landmarks(log)
    steps = parse_plan(text, sas) if code == 0 else []
    return Result(STATUS.get(code, "error"), code, steps, nodes, edges, seconds, work, log)


def main() -> None:
    """The build check: the build's commit and tools, and lama-first on a hand-made task (results/task-build.log)."""
    from domains import BINARY, Domain
    from task import Action, Effect, Task, write_sas

    def out(cmd, cwd=None):
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True).stdout.strip()
    lines = ["E003 step 4: Fast Downward build and check", "",
             f"commit: {out(['git', 'rev-parse', 'HEAD'], DOWNWARD)} ({out(['git', 'describe', '--tags'], DOWNWARD)})",
             "build: cd third_party/downward && ./build.py release -j8   (CMAKE_BUILD_TYPE=Release, builds/release)",
             f"tools: {out(['cmake', '--version']).splitlines()[0]}; {out(['g++', '--version']).splitlines()[0]}",
             f"binary: {(DOWNWARD / 'builds' / BUILD / 'bin' / 'downward').exists()}", ""]
    task = Task({"door": BINARY, "room": Domain.of([1, 2])},
                [Action("open door", pre={"room": {1}}, effects=(Effect("door", 1),)),
                 Action("walk through", pre={"door": {1}, "room": {1}}, effects=(Effect("room", 2),))],
                {"room": {2}})
    sas = write_sas(task)
    text = sas.text({"room": 1})
    code_a, _, plan_a, sec_a, _ = run(text, 60, alias="lama-first")
    res = plan(sas, {"room": 1}, time_limit=60)
    lines += ["hand-made task (task.py): door 0/1, room 1/2; open the door in room 1, walk through it; goal room 2",
              f"  --alias lama-first: exit {code_a}, {sec_a:.2f} s, plan {plan_a.splitlines()}",
              f"  lama-first with the landmark dump: {res.status} (exit {res.exit_code}), {res.seconds:.2f} s, "
              f"plan {[s.name for s in res.steps]}, same plan as the alias: {plan_a == run(text, 60)[2]}",
              f"  landmarks ({len(res.landmarks)}):"]
    lines += [f"    lm{lm.id} {lm.facts}{' initial' if lm.initial else ''}{' goal' if lm.goal else ''}" for lm in res.landmarks]
    lines += [f"  orderings: {res.orderings}", "", "the file:", text]
    (HERE / "results" / "task-build.log").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
