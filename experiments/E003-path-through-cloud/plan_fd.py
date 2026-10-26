"""E003 step 4: run Fast Downward's lama-first on a task, read the plan and the landmarks (plan step 4).

usage: from plan_fd import plan, plan_taskcore
       result = plan(sas, start, time_limit=600)    # sas: task.SasTask; start: a facts.py map
       result.status, result.steps, result.landmarks, result.orderings
       result = plan_taskcore(packed, facts, goal, pieces, start, bans, workdir=DIR)   # through taskcore (below)

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

**Through taskcore** (``plan_taskcore``, the replay harness's planner, replay.py): the task is the packed file
(export_records.py, the reading's records, build_task's task) and taskcore (taskcore/README.md) writes the SAS file:
the start as a file (``--start-file``), the harness's bans (``--bans``, replay.py's ``Ban``: taskcore cuts them out of
the input task before anything else), its fold and reductions for that start, and the operator table (``--ops``).
The start check is on the facts reduction (e) needs (``--start-check needed``, taskcore/README.md): a start value
outside a domain stops the plan only on a fact that can matter to the goal, as the python planner's pruned task held
only such facts; ``start_check="input"`` checks every fact the input's file names (task.py on build_task's task).
Then lama-first (the same search string and limits as ``plan``) or, with ``search="anytime"``, anytime LAMA
(``seq-sat-lama-2011``) for ``time_limit`` seconds, its last (best) plan. The plan reads back through the operator
table, as ``parse_plan`` reads task.py's (picks folded into the steps that read them, goal operators left out), to
the reduced actions and the input actions they stand for. A folded step stands for several paths; the step is the
path whose precondition holds in the plan's predicted state at that step (``predict``: from the start, each step's
path with the plan's guessed values; a guess on a fact the file does not hold has no value in the plan, so it is
unknown, and a start value outside its domain is outside), and a step where not exactly one path holds or may hold
raises ``PathChoice``. A path's precondition may hold where it reads an unknown value on a fact the reduced file
does not hold: of a kept action's precondition the reductions drop only sets that hold for every value the fact can
reach from the start ((d), then normalize), so the unknown value is in the set. A folded step whose paths differ only
on such a fact has two paths that may hold: ``plan_taskcore`` leaves it to the engine (``choices``: at most one path
holds and the others may; the harness takes, when it plays the step, the path whose precondition holds in the
engine's state, replay.py), the plan's state after it keeps what all its paths agree on and is UNKNOWN elsewhere;
``predict`` without ``choices`` raises there. An unknown value on a fact the file holds fails (the planner
gave it a value; not knowing it is a fault here), and so does a value outside the domain, unless the set is the
whole domain. ``pieces`` gives each input action's paths under the bans (replay.py ``ban_pieces``: the same cut,
in Python), so the chosen path is an action of the unreduced banned task. Each step's action is returned with its
effects cut to the facts (e) needs (the operator table's ``needed``): the python planner's pruned task held the
effects on the facts relevance.py needs, and on the kitchen (e) alone gives that task exactly
(results/taskcore-kitchen.log). A start outside a domain on a needed fact: taskcore stops as task.py raises
OutOfDomain; status "out of domain". Any other taskcore error raises ``TaskcoreError``.
"""

import dataclasses
import json
import os
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
# DOWNWARD_DIR or FAST_MI_DOWNWARD: a Fast Downward checkout other than this one's submodule (a worktree whose
# submodules are not checked out uses a built one, read only)
DOWNWARD = Path(os.environ.get("DOWNWARD_DIR") or os.environ.get("FAST_MI_DOWNWARD")
                or ROOT / "third_party" / "downward")
DRIVER = DOWNWARD / "fast-downward.py"
BUILD = "release"
TASKCORE = Path(os.environ.get("TASKCORE_BIN") or HERE / "taskcore" / "target" / "release" / "taskcore")
ANYTIME = "seq-sat-lama-2011"  # driver/aliases.py: anytime LAMA

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


# --- through taskcore ----------------------------------------------------------------------------------------------

UNKNOWN = "<unknown>"  # a predicted value the plan does not give: a guess the reduced task does not hold
OUTSIDE = "<outside the domain>"  # a start value its fact's domain lacks: no set holds it but the whole domain


class PathChoice(ValueError):
    """A step of a taskcore plan where not exactly one of its paths holds or may hold in the plan's predicted
    state."""


class TaskcoreError(RuntimeError):
    """taskcore stopped with an error other than a start outside a domain (its message; ``cmd``: the command)."""

    def __init__(self, message: str, cmd: str = ""):
        super().__init__(message)
        self.cmd = cmd


@dataclass
class TaskcoreResult(Result):
    actions: list = field(default_factory=list)  # per step: its path (task.Action), effects on the file's facts only
    paths: list = field(default_factory=list)  # per step: its path as it is in the unreduced banned task
    write_s: float = 0.0  # taskcore sas: load, bans, start check, fold, reductions, the file and the operator table
    decode_s: float = 0.0  # the operator table read, the plan read back, the paths chosen
    operators: int = 0
    stats: dict = field(default_factory=dict)  # taskcore's
    misses: list = field(default_factory=list)  # out of domain: (key, start value), the first ten taskcore names
    n_misses: int = 0
    file_facts: list = field(default_factory=list)  # the task facts the file holds
    needed_facts: list = field(default_factory=list)  # the facts (e) needs (the operator table's head)
    folds: list = field(default_factory=list)  # per step: the reduced action's name when it stands for several paths
    predicted_goal: bool | None = None  # the goal holds in the predicted state after the last step
    choices: dict = field(default_factory=dict)  # step index -> its paths (effects cut as ``actions``), left to the
    # engine (module docstring); that step's entry in ``actions`` and ``paths`` is ``either`` of them


def _conds(w) -> tuple:
    return tuple((k, frozenset(_value(v) for v in vs)) for k, vs in w)


def read_ops(path) -> tuple:
    """taskcore's operator table (sas.rs ``write_ops``): (head, op number -> row, reduced action -> row)."""
    head, ops, acts = None, {}, {}
    with open(path) as f:
        for line in f:
            d = json.loads(line)
            if head is None:
                head = d
            elif "op" in d:
                ops[d["op"]] = d
            else:
                acts[d["action"]] = d
    return head, ops, acts


def decode_ops(text: str, ops: dict, acts: dict) -> list:
    """The plan's steps through the operator table, ``parse_plan``'s rules: goal operators left out, each pick folded
    into the steps that read it (a pick variable starts at its first value). Per step a dict: op, the reduced action
    and its name, its members (the input actions), chosen and guessed as task.Op holds them."""
    first = {}
    for n in sorted(ops):
        if "pick" in ops[n]:
            first.setdefault(ops[n]["pick"][0], _value(ops[n]["pick"][1]))
    steps, picks = [], {}
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("("):
            continue
        d = ops[int(line.strip("()").split()[0][1:])]
        if d.get("goal"):
            continue
        if "pick" in d:
            picks[d["pick"][0]] = _value(d["pick"][1])
            continue
        a = acts[d["action"]]
        guessed = tuple((k, _value(v), _conds(w)) for k, v, w in d["guessed"])
        for k, vals, w in a["picked"]:
            vals = [_value(v) for v in vals]
            u = picks.get(k, first.get(k))
            guessed += ((k, u if u in vals else vals[0], _conds(w)),)
        steps.append({"op": f"a{d['op']}", "action": d["action"], "name": a["name"], "members": a["members"],
                      "chosen": tuple((k, _value(v)) for k, v in d["chosen"]), "guessed": guessed})
    return steps


def _label(d, v):
    return d.label(int(v)) if isinstance(v, (int, bool)) else v


def _meets(facts: dict, k, vs, state: dict):
    """Whether ``state`` meets the set ``vs`` on ``k`` (task.py: the set cut to the domain; a set that holds for every
    value holds whatever the value, an unknown one or one outside the domain too): True, False, or None (unknown)."""
    v = state.get(k)
    if v in vs or (k in facts and set(facts[k].values) <= set(vs)):
        return True
    return None if v == UNKNOWN else False


def _holds(facts: dict, pre: dict, state: dict, held=None):
    """Whether ``state`` meets the precondition ``pre``: True; None (may hold) where the only sets not met read an
    unknown value on facts the file does not hold (``held``: the file's facts; None: every fact counts as held);
    else False (module docstring)."""
    out = True
    for k, vs in pre.items():
        m = _meets(facts, k, vs, state)
        if m is None and held is not None and k not in held:
            out = None
        elif not m:
            return False
    return out


def _guess(key, vals, guessed, state):
    """The value the plan guessed for ``key`` among ``vals``: the step's guesses on ``key`` whose conditions hold
    in ``state``; UNKNOWN unless exactly one value."""
    got = {v for k, v, w in guessed if k == key and v in vals and all(state.get(c) in s for c, s in w)}
    return next(iter(got)) if len(got) == 1 else UNKNOWN


def apply_step(facts: dict, a, guessed, state: dict) -> dict:
    """``state`` after action ``a`` (task.py's meaning: conditions read the state before; a guess of several values,
    or a step whose result is several values, is the plan's guessed value or UNKNOWN; an effect that fires unless a
    condition on an UNKNOWN value fails makes its fact UNKNOWN)."""
    new = dict(state)
    for e in a.effects:
        met = [_meets(facts, c, s, state) for c, s in e.when]
        if False in met:
            continue
        if None in met:
            new[e.key] = UNKNOWN
            continue
        d = facts[e.key]
        if e.values is None and e.add is None:
            new[e.key] = _label(d, e.value)
            continue
        if e.values is not None:
            vals = {_label(d, v) for v in e.values}
        else:
            cur = state.get(e.key)
            if cur == OUTSIDE:
                continue  # task.py's add is one write per value of the domain: none fires outside it
            vals = d.add(cur, e.add) if cur not in (UNKNOWN, None) else frozenset()
        new[e.key] = next(iter(vals)) if len(vals) == 1 else _guess(e.key, vals, guessed, state) if vals else UNKNOWN
    return new


def start_state(facts: dict, start: dict) -> dict:
    """``start`` (a facts.py map, absent is 0) as the task's values; OUTSIDE where a domain lacks it."""
    out = {}
    for k, d in facts.items():
        lab = _label(d, start.get(k, 0))
        out[k] = OUTSIDE if lab is None else lab
    return out


def either(name: str, paths: list):
    """A stand-in for a step that is several paths, until the engine chooses: every fact their preconditions name,
    with the union of their sets there, and all their effects (for the harness's order, which reads only which facts a
    step reads and writes: the sets are never checked, the engine chooses the path), the first path's record."""
    from task import Action
    keys = set().union(*(set(p.pre) for p in paths))
    pre = {k: set().union(*(set(p.pre.get(k, ())) for p in paths)) for k in sorted(keys)}
    effects = tuple(dict.fromkeys(e for p in paths for e in p.effects))
    return Action(name, pre=pre, effects=effects, what=paths[0].what, cost=paths[0].cost)


def predict(facts: dict, pieces: list, steps: list, start: dict, held=None, choices=None) -> tuple:
    """Each step's path: of the paths its members stand for (``pieces[m]`` for input action m), the one whose
    precondition holds or may hold in the plan's predicted state at that step (module docstring; ``held``: the
    file's facts, see ``_holds``); PathChoice when not exactly one does. With ``choices`` (a dict), a step where at
    most one path holds and the others may hold is left to the engine instead: ``choices[i]`` its paths, its entry
    an ``either`` of them, the state after it what they agree on (UNKNOWN elsewhere). Returns (the paths, the
    predicted state after the last step)."""
    held = set(held) if held is not None else None
    state, out = start_state(facts, start), []
    for i, s in enumerate(steps):
        cands = [p for m in dict.fromkeys(s["members"]) for p in pieces[m]]
        if len(s["members"]) == 1 and any(s["name"] not in (p.name.split("~ban:")[0], getattr(p.what, "label", None))
                                          for p in cands):
            raise ValueError(f"step {i + 1}: the packed file's action {s['members'][0]} is {s['name']}, the task's "
                             f"{[p.name for p in cands][:2]}: the file is not this task's")
        h = [(p, _holds(facts, p.pre, state, held)) for p in cands]
        sure, maybe = [p for p, x in h if x is True], [p for p, x in h if x is None]
        ok = sure + maybe
        if len(ok) > 1 and len(sure) <= 1 and choices is not None:
            choices[i] = ok
            afters = [apply_step(facts, p, s["guessed"], state) for p in ok]
            state = {k: (v if all(x.get(k) == v for x in afters[1:]) else UNKNOWN) for k, v in afters[0].items()}
            out.append(either(s["name"], ok))
            continue
        if len(ok) != 1:
            raise PathChoice(f"step {i + 1} ({s['name']}, {s['op']}): {len(sure)} of its {len(cands)} paths hold in "
                             f"the plan's predicted state" + (f" and {len(maybe)} may hold (an unknown value on a fact "
                                                              f"the file does not hold)" if maybe else "")
                             + (f": {[p.name for p in ok][:4]}" if ok else ""))
        out.append(ok[0])
        state = apply_step(facts, ok[0], s["guessed"], state)
    return out, state


def plan_taskcore(packed, facts: dict, goal: dict, pieces: list, start: dict, bans=(), *, time_limit: int = 600,
                  memory_limit: str = "32G", search: str = "lama-first", workdir, binary=TASKCORE, args=(),
                  start_check: str = "needed", keep: bool = False, engine_choice: bool = True) -> TaskcoreResult:
    """A plan through taskcore (module docstring). ``facts``: the packed file's facts (key -> Domain), ``goal`` its
    goal, ``pieces`` per input action its paths under ``bans`` (replay.ban_pieces), ``start`` a facts.py map (with
    the task's pos:/escpos:/args: keys), ``bans`` replay.Ban or (label, ((key, value), ...)). ``args``: further
    taskcore options (the default: fold strict, reductions (a)-(f), the model encoding). ``start_check``: taskcore's
    ``--start-check`` (needed: on the facts (e) needs; input: on every fact the input's file names). ``keep``: keep
    the SAS file and the operator table in ``workdir``. ``engine_choice``: a step whose paths differ only on facts
    the file does not hold is left to the engine (``choices``) instead of raising PathChoice."""
    from export_records import write_bans, write_start
    work = Path(workdir)
    work.mkdir(parents=True, exist_ok=True)
    sas_file, ops_file, stats_file = work / "task.sas", work / "ops.jsonl", work / "stats.json"
    cmd = [str(binary), "sas", "--task", str(packed), "--start-file", str(write_start(work / "start.txt", facts, start)),
           "--out", str(sas_file), "--stats", str(stats_file), "--ops", str(ops_file), "--start-check",
           start_check] + [str(a) for a in args]
    if bans:
        cmd += ["--bans", str(write_bans(work / "bans.txt", bans))]
    t0 = time.monotonic()
    p = subprocess.run(cmd, capture_output=True, text=True)
    write_s = time.monotonic() - t0
    if p.returncode != 0:
        m = re.search(r"(\d+) facts out of their domains: \[(.*)\]", p.stderr)
        if not m:
            raise TaskcoreError(p.stderr.strip() or f"exit {p.returncode}", " ".join(cmd))
        keys = [json.loads(f'"{k}"') for k in re.findall(r'"((?:[^"\\]|\\.)*)"', m[2])]
        return TaskcoreResult("out of domain", -1, write_s=write_s, misses=[(k, start.get(k, 0)) for k in keys],
                              n_misses=int(m[1]), workdir=str(work))
    stats = json.loads(stats_file.read_text())
    fd = work / "fd"
    for old in fd.glob("sas_plan*"):
        old.unlink()
    if search == "lama-first":
        code, log, text, seconds, _ = run(sas_file.read_text(), time_limit, memory_limit, workdir=str(fd))
        status = STATUS.get(code, "error")
    elif search == "anytime":
        code, log, _, seconds, _ = run(sas_file.read_text(), time_limit, memory_limit, alias=ANYTIME, workdir=str(fd))
        found = sorted(fd.glob("sas_plan.*"), key=lambda q: int(q.suffix[1:]))
        text = found[-1].read_text() if found else ""
        status = "plan" if found else STATUS.get(code, "error")
    else:
        raise ValueError(f"search {search!r}: lama-first or anytime")
    res = TaskcoreResult(status, code, seconds=seconds, workdir=str(work), log=log, write_s=write_s,
                         operators=stats["sas"]["operators"], stats=stats)
    res.landmarks, res.orderings = parse_landmarks(log)  # lama-first's dump (anytime LAMA's alias has none)
    if status == "plan":
        t1 = time.monotonic()
        head, ops, acts = read_ops(ops_file)
        res.file_facts, res.needed_facts = head["facts"], head["needed"]
        dec = decode_ops(text, ops, acts)
        choices = {}
        paths, end = predict(facts, pieces, dec, start, head["facts"], choices=choices if engine_choice else None)
        needed = set(head["needed"])

        def cut(a):
            return dataclasses.replace(a, effects=tuple(e for e in a.effects if e.key in needed))
        res.paths = paths
        res.actions = [cut(a) for a in paths]
        res.choices = {i: [cut(p) for p in ps] for i, ps in choices.items()}
        for i, ps in res.choices.items():
            res.actions[i] = either(dec[i]["name"], ps)
        res.steps = [Step(s["op"], i, a.name, s["chosen"], s["guessed"]) for i, (s, a) in enumerate(zip(dec, paths))]
        res.folds = [s["name"] if len(s["members"]) > 1 else None for s in dec]
        res.predicted_goal = all(end.get(k) in {_label(facts[k], v) for v in vs} for k, vs in goal.items())
        res.decode_s = time.monotonic() - t1
    if not keep:
        for q in (sas_file, ops_file, fd / "task.sas"):
            if q.exists():
                q.unlink()
    return res


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
