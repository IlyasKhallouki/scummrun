"""The exactness check of the reader's switch --reach (reach.md): the task (d) alone makes from a full reading against
the task (d) alone makes from a reading with the switch, as canonical texts, and the fact values reached in each.

    python reach_check.py TASKCORE FULL.tc REACH.tc [--start start] [--reached REACH.reached] [--show N] [--json OUT]

It runs `taskcore dump --fold off --reduce d` on each file, so the sets and effects compared are those (d) leaves. Then it
applies the closure with event naming over what (d) keeps of the full file, with each guess cut to the domains of the
file with the switch: what is left between that and the file with the switch is a job read otherwise (reach.md).
"""

import argparse
import json
import re
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path


def dump(taskcore, path, start, out, how=("--fold", "off", "--reduce", "d")):
    subprocess.run([taskcore, "dump", "--task", path, "--start", start, *how, "--out", out, "--stats", out + ".stats"], check=True, stdout=subprocess.DEVNULL)
    stats = json.loads(Path(out + ".stats").read_text())
    facts, acts = {}, []
    start = {}
    with open(out) as f:
        for line in f:
            x = json.loads(line)
            if "fact" in x:
                facts[x["fact"]] = set(x["values"])
                start[x["fact"]] = x["start"]
            else:
                pre = tuple(sorted((k, tuple(sorted(v))) for k, v in x["pre"]))
                eff = tuple(sorted((k, kind, tuple(sorted(v)), tuple(sorted((a, tuple(sorted(b))) for a, b in w))) for k, kind, v, w in x["eff"]))
                acts.append((x["label"], x["kind"], x["end"], x["cont"], x["cost"], pre, eff))
    stats["start_values"] = start
    return facts, acts, stats


def norm(c):
    """With the switch a job's and a record's set on args:X is always written; the full reading leaves it out where the
    lists known when the job was made are none (reach.md, "Lists"). So {0} on args:X, and a write of 0, are left out on
    both sides, and a record left with no effect is a no-op."""
    pre = tuple(x for x in c[5] if not (x[0].startswith("args:") and x[1] == ("0",)))
    eff = tuple(e for e in c[6] if not (e[0].startswith("args:") and e[2] == ("0",)))
    return c[:5] + (pre, eff)


EVENT = re.compile(r"^(event|esc)\((.+)@(-?\d+)(?:#([0-9a-f]+))?\)")


def cut_guesses(c, domain):
    """An action with each guess cut to the values of `domain` (the domains of the reading with the switch)."""
    eff = []
    for k, kind, vals, when in c[6]:
        if kind == 1 and k in domain:
            vals = tuple(v for v in vals if v in domain[k])
        eff.append((k, kind, vals, when))
    return c[:6] + (tuple(sorted(eff)),)


def closure_named(acts, start):
    """(d) with event naming (reach.md, "Event naming"): the closure over `acts`, where an event's action applies only
    once a record that applies, or the start, names the event: its unit, wait point and list. Returns the indices."""
    reached = {k: ({v} if v is not None else set()) for k, v in start.items()}
    named = set()
    for k, v in start.items():
        if k.startswith("pos:") and v not in (None, "-1", "-2"):
            named.add(("event", k[4:], v, "0"))
    # (kind, unit, wait, list) of each action's event; None for an input
    want = []
    for c in acts:
        m = EVENT.match(c[0])
        if m:
            want.append((m.group(1), m.group(2), m.group(3), str(int(m.group(4), 16)) if m.group(4) else "0"))
        else:
            want.append(None)
    meets = lambda sets: all(any(x in reached.get(k, ()) for x in s) for k, s in sets)
    applied = [False] * len(acts)
    fired = [[False] * len(c[6]) for c in acts]
    changed = True
    rounds = 0
    while changed:
        changed = False
        rounds += 1
        for i, c in enumerate(acts):
            if not applied[i]:
                w = want[i]
                if w is not None and w not in named:
                    continue
                if not meets(c[5]):
                    continue
                applied[i] = True
                changed = True
                pre = dict(c[5])
                sets = {e[0]: e[2][0] for e in c[6] if e[1] == 0 and not e[3] and len(e[2]) == 1}
                for tok in filter(None, c[3].split(";")):
                    kind = "event"
                    if tok.startswith("esc:") or tok.endswith("!"):
                        kind = "esc"
                    name, _, wait = tok.rstrip("!").removeprefix("esc:").rpartition("@")
                    a = "args:" + name
                    lists = {sets[a]} if a in sets else set(pre.get(a, ("0",)))
                    for s in lists | {"0"}:
                        named.add((kind, name, wait, s))
            for j, (k, _, vals, when) in enumerate(c[6]):
                if not fired[i][j] and meets(when):
                    fired[i][j] = True
                    r = reached.setdefault(k, set())
                    for v in vals:
                        if v not in r:
                            r.add(v)
                            changed = True
    return [i for i, x in enumerate(applied) if x], reached


def pairs_written(acts, start_pairs):
    """Lemma 2: the (unit, pos, args) a kept record writes or keeps, and the start's."""
    out = set(start_pairs)
    for c in acts:
        pre = dict(c[5])
        sets = {e[0]: e[2][0] for e in c[6] if e[1] == 0 and not e[3] and len(e[2]) == 1}
        for key in set(sets) | set(pre):
            if not key.startswith("args:"):
                continue
            name = key[5:]
            pk = "pos:" + name
            if pk not in sets and key not in sets:
                continue  # the record writes neither: it only reads the pair
            ws = [sets[pk]] if pk in sets else list(pre.get(pk, ()))
            ss = [sets[key]] if key in sets else list(pre.get(key, ()))
            for w in ws:
                for s in ss:
                    out.add((name, w, s))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("taskcore")
    ap.add_argument("full")
    ap.add_argument("reach")
    ap.add_argument("--start", default="start")
    ap.add_argument("--reached")
    ap.add_argument("--show", type=int, default=5)
    ap.add_argument("--json")
    a = ap.parse_args()
    with tempfile.TemporaryDirectory(prefix="reach-check-", dir=Path(a.reach).parent) as d:
        ff, af, sf = dump(a.taskcore, a.full, a.start, d + "/f.jsonl")
        rf, ar, sr = dump(a.taskcore, a.reach, a.start, d + "/r.jsonl")
        # the task taskcore makes with every reduction, from each: the same up to names, or how it differs
        # (no fold: the fold before (d) sees the full reading's domains, and writes the same effect as more cases)
        tf, atf, _ = dump(a.taskcore, a.full, a.start, d + "/tf.jsonl", ("--fold", "off"))
        tr, atr, _ = dump(a.taskcore, a.reach, a.start, d + "/tr.jsonl", ("--fold", "off"))
    out = {"full_actions": sf["input_actions"], "reach_actions": sr["input_actions"], "d_keeps_of_full": len(af), "d_keeps_of_reach": len(ar)}
    ctf, ctr = Counter(map(norm, atf)), Counter(map(norm, atr))
    out["task"] = {
        "actions_full": len(atf),
        "actions_reach": len(atr),
        "only_full": sum((ctf - ctr).values()),
        "only_reach": sum((ctr - ctf).values()),
        "labels_only_full": Counter(c[0].split("#")[0] for c in (ctf - ctr).elements()).most_common(12),
        "labels_only_reach": Counter(c[0].split("#")[0] for c in (ctr - ctf).elements()).most_common(12),
        "facts_differ": {k: (sorted(tf[k] - tr[k]), sorted(tr[k] - tf[k])) for k in set(tf) & set(tr) if tf[k] != tr[k]},
        "facts_only_full": sum(1 for k in tf if k not in tr),
        "facts_only_reach": sum(1 for k in tr if k not in tf),
    }
    kf = Counter(n for n in map(norm, af) if n[6])
    kr = Counter(n for n in map(norm, ar) if n[6])
    only_full, only_reach = kf - kr, kr - kf
    out["kept_by_d_not_written"] = sum(only_full.values())
    out["written_not_kept_by_d"] = sum(only_reach.values())
    # the values reached: every fact in both files (a fact only one file has is its start value there, or it is listed)
    values_only_full = {k: sorted(v - rf.get(k, set())) for k, v in ff.items() if k in rf and v - rf[k]}
    values_only_reach = {k: sorted(v - ff.get(k, set())) for k, v in rf.items() if k in ff and v - ff[k]}
    out["values_full"] = sum(len(v) for v in ff.values())
    out["values_reach"] = sum(len(v) for v in rf.values())
    out["facts_only_in_full"] = {k: sorted(v) for k, v in ff.items() if k not in rf and len(v) > 1}
    out["facts_only_in_reach"] = {k: sorted(v) for k, v in rf.items() if k not in ff and len(v) > 1}
    out["n_facts_only_in_full_one_value"] = sum(1 for k, v in ff.items() if k not in rf and len(v) == 1)
    out["values_only_full"] = values_only_full
    out["values_only_reach"] = values_only_reach
    if a.reached:
        fv = {}
        for line in Path(a.reached).read_text().splitlines():
            parts = line.split("\t")
            fv[parts[0]] = {x for x in parts[1:] if x}
        # a list SIG_LIMIT folded is still among the reader's values with its number; the file has it as 0
        out["reader_values_differ_from_d"] = {k: (sorted(fv.get(k, set()) - v), sorted(v - fv.get(k, set()))) for k, v in rf.items() if k in fv and (v - fv[k] or (fv[k] - v and not k.startswith("args:")))}
    # the closure with event naming over what (d) keeps of the full reading, each guess cut to the domains of the reading
    # with the switch: what --reach should write (Theorem 1, and reach.md's "Domains")
    with tempfile.TemporaryDirectory(prefix="reach-check-", dir=Path(a.reach).parent) as d:
        dom_r, _, _ = dump(a.taskcore, a.reach, a.start, d + "/dr.jsonl", ("--fold", "off", "--reduce", "none"))
    af_cut = [cut_guesses(c, dom_r) for c in af]
    keep, named_reached = closure_named(af_cut, sf["start_values"])
    kn = Counter(n for n in (norm(af_cut[i]) for i in keep) if n[6])
    out["guesses_cut"] = sum(1 for x, y in zip(af, af_cut) if x != y)
    out["d_named_keeps_of_full"] = len(keep)
    out["named_only_full"] = sum((kn - kr).values())
    out["named_only_reach"] = sum((kr - kn).values())
    out["named_only_full_labels"] = Counter(c[0].split("#")[0] for c in (kn - kr).elements()).most_common(12)
    out["named_only_reach_labels"] = Counter(c[0].split("#")[0] for c in (kr - kn).elements()).most_common(12)
    out["named_values_only_full"] = {k: sorted(v - rf.get(k, set())) for k, v in named_reached.items() if k in rf and v - rf[k]}
    out["named_values_only_reach"] = {k: sorted(v - named_reached.get(k, set())) for k, v in rf.items() if k in named_reached and v - named_reached[k]}
    pw = pairs_written(af, ())
    same_label = {c[0] for c in only_reach}
    cls, other = Counter(), []
    for c in only_full.elements():
        pre = dict(c[5])
        unnamed = False
        for k, s in pre.items():
            if k.startswith("args:") and s != ("0",):
                name = k[5:]
                pos = pre.get("pos:" + name, ())
                if len(pos) == 1 and all((name, pos[0], x) not in pw for x in s):
                    unnamed = True
        if c[0] in same_label:
            cls["a job read otherwise (split or folded in one reading only)"] += 1
        elif unnamed:
            cls["an event with a known list no kept record names (lemma 2)"] += 1
        else:
            cls["other"] += 1
            other.append(c)
    out["kept_by_d_not_written_classes"] = dict(cls)
    out["only_full_labels"] = Counter(c[0].split("#")[0] for c in only_full.elements()).most_common(30)
    out["only_reach_labels"] = Counter(c[0].split("#")[0] for c in only_reach.elements()).most_common(30)
    brief = {k: (v if not isinstance(v, dict) else {x: (y if len(str(y)) < 160 else f"{len(y)} items") for x, y in list(v.items())[:30]}) for k, v in out.items()}
    print(json.dumps(brief, indent=1))
    for name, bag in (("other", other), ("only reach", list(only_reach.elements())), ("only full", list(only_full.elements()))):
        for c in list(bag)[: a.show]:
            print(name, ":", str(c)[:1500])
    if a.json:
        Path(a.json).write_text(json.dumps(out, indent=1))


if __name__ == "__main__":
    sys.exit(main())
