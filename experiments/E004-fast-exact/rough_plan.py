"""E004, the rough reading (rough-reading.md): plan a reading's packed file with taskcore and lama-first, and read the
plan back as its order of rooms and goal facts.

usage: rough_plan.py PACKED OUT [--taskcore BIN] [--time S] [--memory M] [--key] [--goal K=V,...]

1. ``taskcore sas`` from the packed file's start (``--start start``, every reduction, the fold; the reading's
   ``NAME.reached`` next to it) writes OUT/task.sas and the operator table;
2. Fast Downward's lama-first (plan_fd.LAMA_FIRST: unit costs) on it;
3. ``taskcore decode`` reads the plan back to the input actions each step stands for, and ``taskcore dump --fold off
   --reduce none`` gives those actions whole (precondition, effects, end);
4. the plan is played on the input actions from the start: at each step the member whose precondition holds (a fact
   the reduced file does not hold is unknown, and a set on an unknown fact may hold), its effects with the plan's
   guessed values (a guess the plan does not give leaves its fact unknown). After each step: the room, and the goal
   facts that first hold.

A step is a havoc step when its record ends "havoc" (havoc-cut.md), else exact. OUT/summary.json has the counts, the
order and the times; each tool's peak memory is its own (wait4).

``--key`` (evaluation only; nothing here feeds the reading or the planner): the key's Part I goals
(experiments/E001-goals-from-scripts/key.json through end_part1.as_fact), each with the ground truth's move that
reaches it (results/groundtruth-part1.jsonl, by the move's name), against the step of the plan where each first holds.
Only fact keys, values and move numbers are written, no text of the game or the key.
"""

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
E003 = ROOT / "experiments" / "E003-path-through-cloud"
sys.path.insert(0, str(E003))

TASKCORE = E003 / "taskcore" / "target" / "release" / "taskcore"
DOWNWARD = Path(os.environ.get("DOWNWARD_DIR") or ROOT / "third_party" / "downward")
# driver/aliases.py:127-132, the "lama-first" alias, as one string (plan_fd.LAMA_FIRST)
LAMA_FIRST = ("let(hlm,eval_modify_costs(landmark_sum(lm_factory=lm_reasonable_orders_hps(lm_rhw()),pref=false),"
              "cost_type=one),let(hff,eval_modify_costs(ff(),cost_type=one),lazy_greedy([hff,hlm],"
              "preferred=[hff,hlm],cost_type=one,reopen_closed=false)))")
STATUS = {0: "plan", 11: "unsolvable", 12: "incomplete", 22: "memory", 23: "time", 34: "unsupported"}
K_SET, K_GUESS, K_BAD = 0, 1, 2


def timed(cmd, log: Path, cwd=None) -> dict:
    """Run ``cmd`` (stdout and stderr to ``log``): exit code, wall s, CPU s and peak RSS (MB) of the process."""
    t0 = time.monotonic()
    with open(log, "w") as f:
        p = subprocess.Popen([str(c) for c in cmd], stdout=f, stderr=subprocess.STDOUT, cwd=cwd)
        _, status, ru = os.wait4(p.pid, 0)
    return {"code": os.waitstatus_to_exitcode(status), "wall_s": round(time.monotonic() - t0, 2),
            "cpu_s": round(ru.ru_utime + ru.ru_stime, 2), "peak_rss_mb": ru.ru_maxrss // 1024}


def holds(state: dict, key: str, vals) -> bool | None:
    """Whether ``key``'s value is in ``vals``: True, False, or None when the value is unknown."""
    if key not in state or state[key] is None:
        return None
    return state[key] in vals


def member(state: dict, acts: dict, members: list):
    """The member whose precondition holds in ``state`` (an unknown fact may hold), and how many may hold."""
    ok = [m for m in members if all(holds(state, k, vs) is not False for k, vs in acts[m]["pre"])]
    return (ok[0] if ok else None), len(ok)


def apply(state: dict, act: dict, guessed: dict) -> dict:
    """The state after ``act``: each effect whose conditions hold fires (on the state before), a guess with the
    plan's value or unknown; an effect with a condition on an unknown fact leaves its fact unknown."""
    out = dict(state)
    for key, kind, vals, when in act["eff"]:
        c = [holds(state, k, vs) for k, vs in when]
        if any(x is False for x in c):
            continue
        if kind == K_SET:
            v = vals[0]
        elif kind == K_GUESS:
            v = guessed.get(key)
            if v is not None and v not in vals:
                v = None
        else:
            v = None
        out[key] = v if all(x is True for x in c) else (v if out.get(key) == v else None)
    return out


def key_goals():
    """The key's Part I goals: {"#n": (key, op, value, route move)} (end_part1.as_fact)."""
    import end_part1
    key = json.loads(end_part1.KEY.read_text())["key"]
    return {f"#{n}": (*end_part1.as_fact(e["fact"]), e["move"]) for n, e in enumerate(key, 1) if e["part"] == end_part1.PART}


def ground_truth_rank() -> dict:
    """Each ground truth move's place in results/groundtruth-part1.jsonl, by its name's first word (S1#4, ...); a move
    an entry stands for (its ``via``) takes that entry's place."""
    rows = [json.loads(l) for l in (E003 / "results" / "groundtruth-part1.jsonl").read_text().splitlines() if l.strip()]
    rank = {}
    for r in rows:
        for v in r.get("via") or []:
            if isinstance(v, str) and v.split() and "#" in v.split()[0]:
                rank.setdefault(v.split()[0], r["i"])
        rank[r["name"].split()[0]] = r["i"]
    return rank


def goal_holds(state: dict, key: str, op: str, v: int) -> bool:
    got = state.get(key)
    if got is None:
        return False
    return (got == str(v)) if op == "==" else (got != str(v))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("packed")
    ap.add_argument("out")
    ap.add_argument("--taskcore", default=str(TASKCORE))
    ap.add_argument("--time", type=int, default=1800)
    ap.add_argument("--memory", default="8G")
    ap.add_argument("--key", action="store_true")
    ap.add_argument("--goal", help="the goal facts to follow (K=V,...); default results/end-part1.json's")
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    tc, packed = a.taskcore, a.packed
    summary = {"packed": packed}
    base = ["--task", packed, "--start", "start"]
    summary["sas"] = timed([tc, "sas", *base, "--out", out / "task.sas", "--ops", out / "ops.jsonl", "--stats",
                            out / "stats.json"], out / "sas.log")
    if summary["sas"]["code"] != 0:
        summary["status"] = f"taskcore sas exit {summary['sas']['code']}"
        (out / "summary.json").write_text(json.dumps(summary, indent=1) + "\n")
        print(json.dumps(summary))
        return
    stats = json.loads((out / "stats.json").read_text())
    summary["operators"] = stats.get("sas", {}).get("operators")
    fd = out / "fd"
    fd.mkdir(exist_ok=True)
    for old in fd.glob("sas_plan*"):
        old.unlink()
    summary["fd"] = timed([sys.executable, DOWNWARD / "fast-downward.py", "--build", "release", "--plan-file",
                           fd / "sas_plan", "--search-time-limit", f"{a.time}s", "--search-memory-limit", a.memory,
                           out / "task.sas", "--search", LAMA_FIRST], fd / "fd.log", cwd=fd)
    code = summary["fd"]["code"]
    summary["status"] = STATUS.get(code, f"error {code}")
    if code != 0:
        (out / "summary.json").write_text(json.dumps(summary, indent=1) + "\n")
        print(json.dumps(summary))
        return
    summary["decode"] = timed([tc, "decode", *base, "--plan", fd / "sas_plan", "--out", out / "steps.json"],
                              out / "decode.log")
    steps = json.loads((out / "steps.json").read_text())["steps"]
    want = {m for s in steps for m in s["members"]}
    summary["dump"] = timed([tc, "dump", *base, "--fold", "off", "--reduce", "none", "--out", out / "dump.jsonl"],
                            out / "dump.log")
    state, acts, i = {}, {}, 0
    with open(out / "dump.jsonl") as f:
        for line in f:
            d = json.loads(line)
            if "fact" in d:
                state[d["fact"]] = d["start"]
                continue
            if i in want:
                acts[i] = d
            i += 1
    (out / "dump.jsonl").unlink()  # the input task whole; only the plan's members are kept (members.json)
    (out / "members.json").write_text(json.dumps({str(k): v for k, v in acts.items()}) + "\n")
    goal = (dict(kv.split("=") for kv in a.goal.split(",")) if a.goal
            else json.loads((E003 / "results" / "end-part1.json").read_text())["goal_all"])
    kg = key_goals() if a.key else {}
    start_kg = {n for n, (k, op, v, _) in kg.items() if goal_holds(state, k, op, v)}
    order, first_goal, first_kg = [], {}, {}
    havoc = exact = ambiguous = 0
    for j, s in enumerate(steps):
        m, n_ok = member(state, acts, s["members"])
        if m is None:
            m, n_ok = s["members"][0], 0
        ambiguous += n_ok > 1
        act = acts[m]
        is_havoc = act["end"] == "havoc"
        havoc += is_havoc
        exact += not is_havoc
        state = apply(state, act, {k: v for k, v in s["guessed"]})
        new_goal = [k for k, v in goal.items() if k not in first_goal and state.get(k) == str(v)]
        for k in new_goal:
            first_goal[k] = j
        new_kg = [n for n, (k, op, v, _) in kg.items() if n not in start_kg and n not in first_kg and goal_holds(state, k, op, v)]
        for n in new_kg:
            first_kg[n] = j
        order.append({"step": j, "action": act["label"], "rec": act["rec"], "kind": act["kind"], "end": act["end"],
                      "havoc": is_havoc, "members_holding": n_ok, "room": state.get("room"), "goal_facts": new_goal,
                      "key_goals": new_kg})
    summary.update({"plan_steps": len(steps), "havoc_steps": havoc, "exact_steps": exact,
                    "steps_with_several_members_holding": ambiguous,
                    "goal_first_step": first_goal, "goal_reached": all(k in first_goal for k in goal),
                    "rooms": [o["room"] for o in order]})
    if a.key:
        rank = ground_truth_rank()
        gt = {n: rank.get(mv) for n, (_, _, _, mv) in kg.items()}
        passed = sorted(first_kg, key=lambda n: (first_kg[n], gt.get(n) if gt.get(n) is not None else 1e9))
        # pairs of passed goals the plan orders as the ground truth does (ties in either left out)
        same = diff = 0
        for x in range(len(passed)):
            for y in range(x + 1, len(passed)):
                p, q = passed[x], passed[y]
                if first_kg[p] == first_kg[q] or gt[p] is None or gt[q] is None or gt[p] == gt[q]:
                    continue
                same += (first_kg[p] < first_kg[q]) == (gt[p] < gt[q])
                diff += (first_kg[p] < first_kg[q]) != (gt[p] < gt[q])
        summary["key"] = {"goals": len(kg), "holding_at_start": sorted(start_kg, key=lambda n: int(n[1:])),
                          "passed": len(first_kg),
                          "order": [{"goal": n, "fact": kg[n][0], "plan_step": first_kg[n], "ground_truth_move": kg[n][3],
                                     "ground_truth_i": gt[n]} for n in passed],
                          "pairs_same_order": same, "pairs_other_order": diff,
                          "not_passed": sorted(set(kg) - set(first_kg) - start_kg, key=lambda n: int(n[1:]))}
    (out / "order.json").write_text(json.dumps(order, indent=1) + "\n")
    (out / "summary.json").write_text(json.dumps(summary, indent=1) + "\n")
    print(json.dumps({k: v for k, v in summary.items() if k not in ("rooms", "key")}))


if __name__ == "__main__":
    main()
