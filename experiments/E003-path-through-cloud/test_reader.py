"""Tests of reader.py, and the kitchen demo (plan step 4's machinery end to end, on a small part of Part I).

usage: uv run python experiments/E003-path-through-cloud/test_reader.py   (the demo, results/reader-kitchen.log, then
       the tests on the same reading)
       uv run pytest experiments/E003-path-through-cloud/test_reader.py   (the tests alone; the reading takes ~20 min)
       READER_PICKLE=<file> ...   (an instrument for iterating: the reading is loaded from that file when it exists,
       else read and saved there; never under the repo)

An instrument: the fixtures name MI1's numbers (tests may, C2), reader.py does not. The demo's start is the
engine's state before run B's move S1#7 (groundtruth.py's ``pre``), its scope the bar (room 28) and the kitchen
(room 41) with the global scripts they reach, and its goal the hunk of meat (object 566) owned by the ego
(VAR_EGO's value at the start). The walk verb is the reader's own (``reader.walk_verbs``, from the scripts); a test
checks it against the executor's interface profile.

What the scripts say the engine does (read by hand, the check): Open door 316 while local 211 runs starts local
214 (obj-0316-door.txt [0018]-[0027]), which starts local 212 (local-214.txt [004B]), which after its delays
starts local 216 (local-212.txt [0020]), which opens 316 for the cook (local-216.txt [001B], global script 25);
Walk to 316 while it is open starts local 218 (obj-0316-door.txt [003F]-[004B]), which loads room 41
(local-218.txt [0017]); Pick up 566 tests owner 15 and picks it up (obj-0566-hunk-of-meat.txt [0035]-[0041]).
But local 211 itself, once its delay is over, starts local 216 (local-211.txt [001E]-[0039]), with no click.
And in the bar a click does not reach the sentence script directly: the bar's input script (VAR_VERB_SCRIPT = 202)
hands a click on 316 to local 203 (local-202.txt [001C]-[0023]), which runs the door's entry itself
(local-203.txt [0002]-[0053]). The reader's sentences start in the sentence script (plan step 1), so they do not see
that; ``test_variant_against_the_engine`` measures what that changes on S1#7.
"""

import collections
import functools
import os
import pickle
import sys
import time
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "src"))

import domains as D  # noqa: E402
import groundtruth  # noqa: E402
import model as M  # noqa: E402
import plan_fd  # noqa: E402
import reader as R  # noqa: E402
import relevance  # noqa: E402
import task as T  # noqa: E402
from speedrun.executor import PROFILES  # noqa: E402

ROOMS = (28, 41)  # the bar and the kitchen
MOVE = "S1#7"  # the start: the engine's state before run B's "provoke the cook" move
MEAT, DOOR = 566, 316
LOG = HERE / "results" / "reader-kitchen.log"
MODEL_SHA = __import__("hashlib").sha1((HERE / "model.py").read_bytes()).hexdigest()[:12]  # the model.py this process read
READER_SHA = __import__("hashlib").sha1((HERE / "reader.py").read_bytes()).hexdigest()[:12]
WORKERS = 7  # processes for the reading (the machine has 8 cores)
SENTENCE_WAIT = 736  # the sentence script's first WaitForActor (global/script-002.txt [02E0]); a replan fixture


@functools.cache
def row() -> dict:
    return next(r for r in groundtruth.load() if r["name"].startswith(MOVE + " "))


def start() -> dict:
    return row()["pre"]


@functools.cache
def built():
    """The demo's reading: (builder, records, seconds), once per process (from READER_PICKLE when it is there)."""
    cache = os.environ.get("READER_PICKLE")
    if cache and Path(cache).exists():
        b, recs, secs = pickle.loads(Path(cache).read_bytes())
        return b, recs, dict(secs, loaded=cache)
    pre = start()
    t0 = time.monotonic()
    doms = D.build(pre)
    t1 = time.monotonic()
    game = M.Game.load()
    sc = R.Scope.of(game, ROOMS, pre)
    rd = R.Reader(sc, doms, game)
    b = R.Builder(rd)
    b.progress = lambda what, jobs, n, secs: print(f"[reading] {what}: {jobs} runs, {n} records, {secs:.0f} s; "
                                                   f"slowest so far {b.slow.most_common(2)}", file=sys.stderr, flush=True)
    recs = b.read_all(workers=WORKERS)
    b.progress = None
    out = b, recs, {"domains": t1 - t0, "read": time.monotonic() - t1}
    if cache:
        Path(cache).write_bytes(pickle.dumps(out))
    return out


def goal() -> dict:
    return {f"owner:{MEAT}": {start()["var:1"]}}


@functools.cache
def full_task():
    b, recs, _ = built()
    return R.build_task(b, recs, goal())


def replan_observed() -> dict:
    """A replan fixture: S1#7's state with the sentence script waiting at its first WaitForActor, as a sentence leaves
    it, and its locals unknown (the bridge dumps none)."""
    obs = dict(start())
    obs["script:2"] = 1
    obs[f"at:script:2:{SENTENCE_WAIT}"] = 1
    return obs


@functools.cache
def pruned(records: str):
    """The task from the demo's records (``records`` "all", or "no 211" without local 211's own event at its start
    point), relevance and the pruned task (reader.prune)."""
    b, recs, _ = built()
    kept = [r for r in recs if not (records == "no 211" and r.label == "event(local:211@33)")]
    task = R.build_task(b, kept, goal())
    rel = relevance.relevant(task)
    small = R.prune(task, rel)
    small.folded = task.folded
    return small, rel


@functools.cache
def plan_of(variant: str = "all", time_limit: int = 600):
    """``variant`` "all", "no 211" (``pruned``) or "replan" (all, from ``replan_observed``): the pruned task,
    relevance, the file's counts and size, lama-first's result and the start used."""
    b, recs, _ = built()
    small, rel = pruned("no 211" if variant == "no 211" else "all")
    st = R.replan_start(b, replan_observed(), {}, small)[0] if variant == "replan" else R.task_start(b, start(), small)
    cache = os.environ.get("READER_PICKLE")
    saved = Path(f"{cache}.plan-{variant.replace(' ', '')}-{time_limit}.pkl") if cache else None
    if saved is not None and saved.exists():  # the instrument's cache: the same reading gives the same task
        info, res = pickle.loads(saved.read_bytes())
        return small, rel, info, res, st
    sas = T.write_sas(small)
    res = plan_fd.plan(sas, st, time_limit=time_limit)
    info = dict(sas.counts, MB=len(sas.text(st)) / 1e6)  # the file's size; the text itself is not kept (memory)
    del sas
    if saved is not None:
        saved.write_bytes(pickle.dumps((info, res)))
    return small, rel, info, res, st


def by_label(recs, label):
    return [r for r in recs if r.label == label]


def replay(task, steps, start_map) -> tuple:
    """Play plan ``steps`` (plan_fd Steps) over the task's own actions, from ``start_map`` (reader.task_start): each
    step's precondition must hold; its effects apply from the state before it (task.py's semantics: a ``when`` holds
    in the state before; a guessed value is the one the step chose). Returns (reached the goal, the first step that
    could not run or None, the steps that used a guessed value, the state after)."""
    state = {k: d.label(start_map.get(k, 0)) for k, d in task.facts.items()}
    used = []
    for i, s in enumerate(steps):
        a = task.actions[s.action]
        if any(state.get(k) not in set(v) for k, v in a.pre.items()):
            return False, i, used, state
        chosen = {k: v for k, v, _ in s.guessed}
        new = dict(state)
        for e in a.effects:
            if any(state.get(k) not in set(v) for k, v in e.when):
                continue
            dom = task.facts[e.key]
            if e.values is not None:
                if e.key not in chosen:
                    return False, i, used, state
                new[e.key] = chosen[e.key]
                used.append((i, e.key, chosen[e.key]))
            elif e.add is not None:
                vals = dom.add(state[e.key], e.add)
                if len(vals) == 1:
                    new[e.key] = next(iter(vals))
                elif e.key in chosen:
                    new[e.key] = chosen[e.key]
                    used.append((i, e.key, chosen[e.key]))
                else:
                    return False, i, used, state
            else:
                new[e.key] = dom.label(e.value) if isinstance(e.value, int) else e.value
        state = new
    return all(state.get(k) in {task.facts[k].label(v) if isinstance(v, int) else v for v in vals}
               for k, vals in task.goal.items()), None, used, state


def needed_steps(task, steps, start_map) -> list:
    """The plan without the steps the goal does not need: a step, or else a pair of steps (an input and the event that
    continues it), is dropped while the rest still reaches the goal (``replay``), until none can be."""
    keep = list(steps)
    while True:
        n = len(keep)
        drops = [(i,) for i in range(n)] + [(i, j) for i in range(n) for j in range(i + 1, n)]
        for d in drops:
            trial = [s for k, s in enumerate(keep) if k not in d]
            if replay(task, trial, start_map)[0]:
                keep = trial
                break
        else:
            return keep


def relaxed_reach(task, start_map) -> tuple:
    """Delete-free reachability over the task's actions from ``start_map``: (the values each fact can reach, the
    actions usable). A guessed effect reaches every value it lists."""
    def lab(dom, v):
        return dom.label(v) if isinstance(v, int) else v
    have = collections.defaultdict(set)
    for k, d in task.facts.items():
        have[k].add(lab(d, start_map.get(k, 0)))
    used, changed = set(), True
    while changed:
        changed = False
        for i, a in enumerate(task.actions):
            if i in used or not all(have[k] & {lab(task.facts[k], v) for v in s} for k, s in a.pre.items()):
                continue
            used.add(i)
            changed = True
            for e in a.effects:
                if not all(have[k] & set(s) for k, s in e.when):
                    continue
                d = task.facts[e.key]
                if e.values is not None:
                    new = {lab(d, v) for v in e.values}
                elif e.add is not None:
                    new = set().union(*(d.add(u, e.add) for u in list(have[e.key])))
                else:
                    new = {lab(d, e.value)}
                have[e.key] |= new
    return have, used


def engine_vs_prefix(task, steps, start_map) -> dict:
    """S1#7's engine diff at the click's done frame (groundtruth ``done_diff``: the human's Open 316, until state:316
    is 1) against the task state after the plan's steps up to the first that makes state:316 1. Keys the task tracks;
    at: keys as pos: (reader.positions)."""
    st0 = {k: d.label(start_map.get(k, 0)) for k, d in task.facts.items()}
    upto = next((i + 1 for i in range(len(steps)) if replay(task, steps[:i + 1], start_map)[3].get(f"state:{DOOR}") == 1),
                len(steps))
    _, _, _, after = replay(task, steps[:upto], start_map)
    predicted = {k: v for k, v in after.items() if st0.get(k) != v}
    engine = {}
    for k, (was, now) in row()["done_diff"].items():
        if k.startswith("at:") and now == 1:
            name, w = k[3:].rsplit(":", 1)
            engine[f"pos:{name}"] = int(w)
        elif not k.startswith(("at:", "esc:")):
            engine[k] = now
    tracked = {k: v for k, v in engine.items() if k in task.facts}
    hit = {k for k, v in predicted.items() if k in tracked and tracked[k] == v}
    return {"steps": upto, "predicted": predicted, "engine": tracked, "untracked": sorted(set(engine) - set(tracked)),
            "hit": sorted(hit), "precision": len(hit) / max(1, len(predicted)), "recall": len(hit) / max(1, len(tracked)),
            "missed": sorted(set(tracked) - hit), "wrong": sorted(set(predicted) - hit)}


# --- the reader's rules ------------------------------------------------------------------------------------------------

def test_machine_matches():
    """reader.py's wrappers and overrides still match model.Machine (finding 8: checked at import, here too)."""
    assert R.check_machine() == []
    for name in R.LAZY_METHODS:
        assert R.Reader.__dict__[name].__name__ == name


def test_scope_and_walk_verb():
    """The walk verb comes from the scripts (C2), and it is the one the executor's profile names."""
    b, recs, _ = built()
    sc = b.scope
    assert R.walk_verbs(b.game) == [PROFILES["dos-cd"].walk_to] and sc.walk_verb == PROFILES["dos-cd"].walk_to
    assert ("local", 28, 211) in sc.units and ("global", 2) in sc.units  # the sentence script is reached
    assert DOOR in sc.objects and MEAT in sc.objects
    assert sc.walk_verb not in sc.verbs and 2 in sc.verbs and 9 in sc.verbs


def test_constants_are_no_preconditions():
    """A fact nothing in the scope writes is read from the start: no fork, no precondition (class 15 of the door,
    which the sentence script's class chain tests, global/script-002.txt [033D])."""
    b, recs, _ = built()
    assert b.table.kind(f"class:{DOOR}:15") == "const"
    assert not any(f"class:{DOOR}:15" in r.pre for r in recs)


def test_fork_on_read_gives_preconditions():
    """The sentence script tests Bit[5 + VAR_EGO] (global/script-002.txt [02AD]): the walk differs by it."""
    b, recs, _ = built()
    opens = by_label(recs, f"sentence(2,{DOOR})")
    assert opens
    assert {r.pre.get("bit:6") for r in opens} >= {frozenset({0}), frozenset({1})}


def test_sentence_leaves_its_later_part_as_an_event():
    """Open 316: the sentence script walks and yields at its WaitForActor; its locals become ``args:script:2``, the
    event with those locals goes on, and a later one runs the door's Open entry, which forks on local 211."""
    b, recs, _ = built()
    walk = [r for r in by_label(recs, f"sentence(2,{DOOR})") if r.pre.get("bit:6") == frozenset({0})]
    assert walk and all("args:script:2" in r.effects for r in walk)
    sig = walk[0].effects["args:script:2"][1]
    assert [r for r in recs if r.kind == "event" and r.label.startswith("event(script:2@")
            and r.label.endswith(f"#{sig:x})") and r.pre.get("args:script:2") == frozenset({sig})]
    opened = [r for r in recs if r.kind == "event" and r.label.startswith("event(script:2@") and "local:211" in r.pre]
    with_211 = [r for r in opened if r.pre["local:211"] == frozenset({1})]
    without = [r for r in opened if r.pre["local:211"] == frozenset({0})]
    # local 211 running: local 214 starts; not running: script 25 opens the door
    assert any(r.effects.get("local:214") == ("set", 1) for r in with_211)
    assert any(r.effects.get(f"state:{DOOR}") == ("set", 1) for r in without)


def test_sentence_held_or_frozen():
    """checkAndRunSentenceScript holds a sentence only while the sentence slot is live with freeze count 0
    (script.cpp:1186-1191): sentences have a second variant with the slot live and frozen (finding 9)."""
    b, recs, _ = built()
    frozen = [r for r in recs if r.kind in ("sentence", "walk_to") and r.label.endswith("~frozen")]
    assert frozen and all(r.pre.get("script:2") == frozenset({1}) and r.pre.get("frozen:script:2") == frozenset({1})
                          for r in frozen)
    plain = [r for r in recs if r.kind in ("sentence", "walk_to") and not r.label.endswith("~frozen")]
    assert all(r.pre.get("script:2") == frozenset({0}) for r in plain)


def test_local_211_event_opens_the_door():
    b, recs, _ = built()
    evs = [r for r in recs if r.label == "event(local:211@33)"]
    assert any(r.effects.get(f"state:{DOOR}") == ("set", 1) and r.effects.get("local:211") == ("set", 0)
               for r in evs)
    # its condition on time is a replay check, not a precondition
    assert all(any("delay is over" in c[1] for c in r.checks) for r in evs)


def test_one_frame_reads_an_actor_room_once():
    """Finding 3: local 216 reads getActorRoom(6) (local-216.txt [0004]); when it is 28 it starts local 217, whose
    getDist test may restart 216 in the same frame, and the new 216 reads the same room. No record may take the two
    reads different ways: a record that starts 217 took [0004] as "== 28", and none also opened the door through the
    "!= 28" side of a second read."""
    b, recs, _ = built()
    evs = [r for r in recs if r.label == "event(local:211@33)"]
    for r in evs:
        sides = {c[2] for c in r.checks if c[0].endswith("local-216.txt:10 [0004]")}
        assert len(sides) <= 1 and ("local-216.txt:10 [0004]", "VAR_RESULT != 28") not in \
            {(c[0].rsplit("/", 1)[-1], c[1]) for c in r.either}
        if r.effects.get("local:217") == ("set", 1):
            assert sides == {False} and r.effects.get(f"state:{DOOR}") is None
    assert b.rd.counts["non-fact tests decided by the same test earlier in the frame"] > 0


def test_walk_into_the_kitchen_and_pick_up():
    b, recs, _ = built()
    assert any(r.effects.get("room") == ("set", 41) for r in recs if r.label.startswith("event(local:218@"))
    ego = start()["var:1"]
    assert any(r.effects.get(f"owner:{MEAT}") == ("set", ego) for r in recs if r.kind == "event")


def test_give_reaches_the_receivers_entry():
    """Finding 6: Give starts B's verb-80 entry with A (global/script-002.txt [009A]); B ranges over the scope's
    objects with that entry (322 and 339 in the bar)."""
    b, recs, _ = built()
    give = {r.source[3] for r in recs if r.kind == "sentence" and r.source[1] == 4 and r.source[3]}
    assert {322, 339} <= give


def test_unknown_targets_are_counted():
    """Finding 2: every write whose target does not fold is counted by statement and site and listed on its record
    for replay (not guessed). The inventory hook is summarized wherever it is started, so script 9's startObject on
    findInventory's result is not run."""
    b, recs, _ = built()
    left = sum(len(r.unknown) for r in recs)
    assert 0 < left <= sum(v for k, v in b.counts.items() if k.startswith("unknown-target writes left out"))
    assert sum(b.unknown_writes.values()) >= left and sum(b.unknown_sites.values()) == sum(b.unknown_writes.values())
    assert not any(k.startswith("unknown target") for k in b.guesses)
    assert not any("script-009" in cite for _, cite in b.unknown_sites)
    assert any(k.startswith("inventory hook runs summarized") for k in b.rd.counts)


def test_esc_lists_the_key_it_does_not_run():
    """Finding 10: after abortCutscene's slot runs, the Esc key goes on to checkExecVerbs and the input script
    (input.cpp:1427-1428); the reader does not run that (reader.ESC_KEY) and lists it on every Esc record."""
    b, recs, _ = built()
    esc = [r for r in recs if r.kind == "skip_cutscene"]
    assert esc and all(R.ESC_KEY in r.unknown for r in esc)


def test_no_record_from_a_dropped_end():
    b, recs, _ = built()
    assert all(r.end not in R.DROP_ENDS for r in recs)


def test_workers_do_not_change_the_records():
    """The same inputs on one process and on four give the same records in the same order."""
    game = M.Game.load()
    b0, _, _ = built()
    jobs = list(b0.inputs())[:24]
    out = []
    for workers in (1, 4):
        b = R.Builder(R.Reader(b0.scope, b0.table.doms, game))
        out.append([(r.label, sorted(map(str, r.pre.items())), sorted(map(str, r.effects.items())))
                    for r in b._run_jobs(jobs, workers)])
    assert out[0] == out[1] and out[0]


def test_replan_start_copies_args_only_where_the_slot_is():
    """Finding 1: at a replan the slot dump has no locals; ``replan_start`` takes ``args:`` from the plan's prediction
    when the slot is where the plan put it, else 0."""
    b, _, _ = built()
    st, counts = R.replan_start(b, replan_observed(), {"pos:script:2": SENTENCE_WAIT, "args:script:2": 77})
    assert st["pos:script:2"] == SENTENCE_WAIT and st["args:script:2"] == 77
    st, counts = R.replan_start(b, replan_observed(), {"pos:script:2": 915, "args:script:2": 77})
    assert st["args:script:2"] == 0 and counts["live slots with unknown locals (args 0)"] == 1


def test_replan_with_unknown_locals_can_reach_the_goal():
    """Finding 1: from S1#7 with the sentence script waiting at its WaitForActor and its locals unknown (args 0), the
    goal is reachable ignoring deletes: the args-0 events let the sentence slot end."""
    b, _, _ = built()
    task = full_task()
    st = R.replan_start(b, replan_observed(), {}, task)[0]
    have, used = relaxed_reach(task, st)
    del task
    full_task.cache_clear()  # memory: the plans below build their own tasks
    assert 0 in have["script:2"] and have[f"owner:{MEAT}"] & {start()["var:1"]}


def test_kitchen_plan():
    task, rel, sas, res, st = plan_of("all")
    assert res.status == "plan"
    ok, bad, used, _ = replay(task, res.steps, st)
    assert ok and bad is None and used == []
    names = [s.name for s in res.steps]
    assert any(n.startswith(f"sentence(9,{MEAT})") for n in names)


def test_kitchen_plan_without_211s_own_timer():
    """With local 211's event left out (a disclosed variant: its delay never runs out within the plan), the plan
    has to provoke the cook: Open 316 first."""
    task, rel, sas, res, st = plan_of("no 211")
    assert res.status == "plan"
    ok, bad, used, _ = replay(task, res.steps, st)
    assert ok and bad is None and used == []
    names = [s.name for s in res.steps]
    assert any(n.startswith(f"sentence(2,{DOOR})") for n in names)


def test_variant_against_the_engine():
    """Finding 4, a measurement: the variant plan's steps up to the door opening, against what the engine changed in
    S1#7 (the human's Open 316), over the facts the pruned task tracks. What should match does: local 214 starts,
    waiting at 15, and the door opens. The engine also ran local 203 (the bar's input script's handler), which the
    reader's sentence, started in the sentence script, does not: the known gap (a plan question)."""
    task, rel, sas, res, st = plan_of("no 211")
    cmp = engine_vs_prefix(task, res.steps, st)
    for k in ("local:214", "pos:local:214", f"state:{DOOR}"):
        assert k in cmp["hit"], (k, cmp)
    assert "local:203" in cmp["engine"] and "local:203" not in cmp["predicted"]


def test_replan_plan():
    """Finding 1 with 2: a plan from the replan fixture, and whether it rests on a guessed value."""
    task, rel, sas, res, st = plan_of("replan")
    assert res.status == "plan"
    ok, bad, used, _ = replay(task, res.steps, st)
    assert ok and bad is None and used == []


# --- the demo ----------------------------------------------------------------------------------------------------------

def _plan_lines(res, task) -> list:
    out = []
    for i, s in enumerate(res.steps, 1):
        r = task.actions[s.action].what
        pre = {k: sorted(map(str, v)) for k, v in sorted(r.pre.items())}
        eff = {k: (e[1] if e[0] == "set" else f"{e[0]} {sorted(map(str, e[1])) if e[0] == 'guess' else e[1]}")
               for k, e in sorted(r.effects.items()) if not k.startswith(("at:", "esc:", "frozen:"))}
        out.append(f"{i:3}. {s.name}  [{r.kind}, path end {r.end}]")
        out.append(f"       pre: {pre}")
        if s.chosen:
            out.append(f"       variant: {list(s.chosen)}")
        if s.guessed:
            out.append(f"       guessed: {[(k, str(v)) for k, v, _ in s.guessed]}")
        out.append(f"       effects (slot positions left out): {eff}")
        if r.checks:
            out.append(f"       replay checks: {[(c[0], c[1], c[2]) for c in r.checks]}")
        if r.either:
            out.append(f"       met on both sides (not checked): {r.either}")
        if r.unknown:
            out.append(f"       writes with unknown targets left out: {r.unknown}")
    return out


def _median(xs):
    xs = sorted(xs)
    return xs[len(xs) // 2] if xs else 0


def main() -> None:
    b, recs, secs = built()
    import hashlib
    lines = ["E003 step 4, second half: the reader (reader.py) on the kitchen branch", "",
             f"model.py read: sha1 {MODEL_SHA}, reader.py sha1 {READER_SHA}" +
             (f" (reading loaded from {secs['loaded']})" if "loaded" in secs else ""), "",
             f"start: the engine's state before run B's move {MOVE} (groundtruth.py pre); scope rooms {ROOMS} and "
             f"the global scripts they reach; goal owner:{MEAT} = VAR_EGO's value {start()['var:1']}; walk verb "
             f"{b.scope.walk_verb} (reader.walk_verbs, from the scripts; the executor profile's is "
             f"{PROFILES['dos-cd'].walk_to})", ""]
    if "read" in secs:
        lines += [f"time: domains.build {secs['domains']:.1f} s, reading {secs['read']:.1f} s", ""]
    sc = b.scope
    lines += [f"scope: {len(sc.units)} units ({sum(1 for k in sc.units if k[0] == 'global')} global), "
              f"{len(sc.objects)} objects, sentence verbs {sc.verbs}, dialog verbs {sc.dialog_verbs}, "
              f"{len(sc.universe)} slot facts listed", ""]
    kinds = collections.Counter(r.kind for r in recs)
    runs = {k[6:]: v for k, v in b.counts.items() if k.startswith("runs: ")}
    lines.append("actions (one per path) by kind, and the runs they come from:")
    for k in sorted(set(kinds) | set(runs)):
        lines.append(f"  {k:14} {kinds.get(k, 0):6} actions from {runs.get(k, 0):5} runs   ({b.seconds.get(k, 0):.1f} s)")
    lines.append(f"  {'total':14} {len(recs):6}")
    pres = [len(r.pre) for r in recs]
    eng = [len(r.engine_pre) for r in recs]
    scr = [len(r.pre) - len(r.engine_pre) for r in recs]
    effs = [len(r.effects) for r in recs]
    lines += ["", f"preconditions per action: median {_median(pres)}, max {max(pres)}, "
              f"{sum(1 for n in pres if n == 0)} with none",
              f"  of them the input's engine conditions (actions.usable, the event's slot and wait point): median "
              f"{_median(eng)}, max {max(eng)}",
              f"  of them what the scripts tested (forks on read): median {_median(scr)}, max {max(scr)}, "
              f"{sum(1 for n in scr if n == 0)} actions with none",
              f"effects per action: median {_median(effs)}, max {max(effs)}", ""]
    for kind in sorted(kinds):
        ks = [len(r.pre) - len(r.engine_pre) for r in recs if r.kind == kind]
        lines.append(f"  {kind:14} script-tested preconditions: median {_median(ks)}, max {max(ks)}")
    fam = collections.Counter(R.M.family(k) for r in recs for k in r.pre)
    lines.append("")
    lines.append("precondition facts by family (actions x facts): " + ", ".join(f"{k} {v}" for k, v in fam.most_common()))
    lines.append("forks on read by family: " + ", ".join(f"{k} {v}" for k, v in b.rd.forks.most_common()))
    lines += ["", f"conditions that could not be facts (replay checks), by reason "
              f"(total {sum(b.nonfact_checks.values())}):"]
    lines += [f"  {v:6}  {k}" for k, v in b.nonfact_checks.most_common()]
    either = sum(len(r.either) for r in recs)
    lines += [f"conditions met on both sides of one record (listed apart, not checked): {either} on "
              f"{sum(1 for r in recs if r.either)} records"]
    lines += ["", "guessed effects (a value the reader cannot work out; replan on a mismatch), by reason:"]
    lines += [f"  {v:6}  {k}" for k, v in b.guesses.most_common()]
    lines.append(f"records with a guessed effect: {sum(1 for r in recs if any(e[0] == 'guess' for e in r.effects.values()))}"
                 f" of {len(recs)}")
    lines += ["", f"writes whose target does not fold (model.py unknown_writes), by statement (total "
              f"{sum(b.unknown_writes.values())}, counted per record; left out of the task and listed on the record "
              f"for replay):"]
    lines += [f"  {v:6}  {k}" for k, v in b.unknown_writes.most_common()]
    lines.append("  by site, the 12 largest:")
    lines += [f"  {v:6}  {k[0]} at {k[1]}" for k, v in b.unknown_sites.most_common(12)]
    lines += ["", "path ends (all runs):"]
    lines += [f"  {v:6}  {k}: {e}" for (k, e), v in sorted(b.ends.items(), key=lambda kv: (-kv[1], str(kv[0])))]
    lines += ["", "counts:"]
    lines += [f"  {v:8}  {k}" for k, v in sorted(b.counts.items())]
    lines += [f"  {v:8}  {k}" for k, v in sorted(b.rd.counts.items())]
    lines += [f"  {v:8}  {k}" for k, v in sorted(b.table.counts.items())]
    lines += [f"  {v:8}  {k}" for k, v in sorted(sc.counts.items())]
    lines += [f"  {v:8}  {k}" for k, v in sorted(sc.writes.counts.items())]
    lines += ["", "path ends that are no action, by what was run (limit, left the scope, ...), the 25 largest:"]
    lines += [f"  {v:6}  {k}: {e}" for (k, e), v in sorted(b.drops.items(), key=lambda kv: (-kv[1], str(kv[0])))[:25]]
    chain = ("sentence(2,316)", "walk_to(316)", "sentence(9,566)", "event(local:211)", "event(local:212)",
             "event(local:214)", "event(local:216)", "event(local:218)", "event(script:2)", "event(script:25)",
             "event(script:26)")
    on_chain = {k: v for k, v in b.drops.items() if k[0] in chain}
    lines.append(f"  on the demo's chain ({', '.join(chain)}): " +
                 (", ".join(f"{k[0]} {k[1]} {v}" for k, v in sorted(on_chain.items())) or "none"))
    lines += ["", "values written outside their domain (domains.py lacks them; the task leaves such writes out):"]
    lines += [f"  {k}: {sorted(v)}" for k, v in sorted(b.out_of_domain.items())] or ["  none"]
    task = full_task()
    st = R.replan_start(b, replan_observed(), {}, task)[0]
    have, used = relaxed_reach(task, st)
    del task
    full_task.cache_clear()  # memory: the plans below build their own tasks
    lines += ["", f"replan fixture (S1#7 with the sentence script waiting at {SENTENCE_WAIT}, args 0): ignoring "
              f"deletes the goal is reachable: {bool(have[f'owner:{MEAT}'] & {start()['var:1']})}, "
              f"script:2 can reach {sorted(map(str, have['script:2']))}, {len(used)} actions usable"]
    for title, variant in (("the plan", "all"),
                           ("variant: local 211's own event left out (its delay never runs out within the plan)",
                            "no 211"),
                           (f"replan: from the replan fixture (sentence script at {SENTENCE_WAIT}, args 0)",
                            "replan")):
        task, rel, sas, res, st = plan_of(variant)
        lines += ["", f"=== {title} ===",
                  f"records folded into an action of the same input with the same precondition and effects: "
                  f"{task.folded}",
                  f"task: {len(task.actions)} actions, {len(task.facts)} facts; relevance keeps {rel.counts['actions kept']}"
                  f" actions, {rel.counts['facts needed']} facts needed ({rel.counts['facts needed whole']} whole)",
                  f"after pruning effects on facts nothing needs: {sum(len(a.effects) for a in task.actions)} effects",
                  f"SAS: {sas['variables']} variables, {sas['operators']} operators, "
                  f"{sas['effects']} effects, {sas['pick variables']} pick variables, {sas['MB']:.1f} MB",
                  f"lama-first: {res.status} (exit {res.exit_code}) in {res.seconds:.1f} s, {len(res.steps)} steps, "
                  f"{len(res.landmarks)} landmarks ({sum(1 for lm in res.landmarks if lm.aux)} of the encoding)"]
        if res.status != "plan":
            continue
        lines += _plan_lines(res, task)
        ok, bad, used, _ = replay(task, res.steps, st)
        lines.append(f"replay over the task's own actions: goal reached {ok}, first step that cannot run "
                     f"{bad}, guessed values used {[(i + 1, k, str(v)) for i, k, v in used]}")
        need = needed_steps(task, res.steps, st) if ok else res.steps
        lines.append(f"steps the goal needs ({len(need)} of {len(res.steps)}): {[s.name for s in need]}")
        if variant == "no 211":
            cmp = engine_vs_prefix(task, res.steps, st)
            lines += [f"against the engine (S1#7 done_diff, the human's Open 316 until state:316 = 1), the plan's first "
                      f"{cmp['steps']} steps: precision {cmp['precision']:.2f}, recall {cmp['recall']:.2f} over the "
                      f"{len(cmp['engine'])} changed facts the task tracks",
                      f"  matched: {cmp['hit']}",
                      f"  the engine changed, not predicted: {cmp['missed']}",
                      f"  predicted, not what the engine shows: {cmp['wrong']}",
                      f"  changed facts the task does not track: {len(cmp['untracked'])}"]
    LOG.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
    # the tests, on this process's reading (pytest imports this module by its name)
    sys.modules["test_reader"] = sys.modules["__main__"]
    sys.exit(pytest.main([__file__, "-q", "-p", "no:cacheprovider"]))
