"""The exactness check of the reader's switch --reach (reach.md): the task (d) alone makes from a full reading against
the task (d) alone makes from a reading with the switch, as canonical texts, and the fact values reached in each.

    python reach_check.py TASKCORE FULL.tc REACH.tc [--start start] [--reached REACH.reached] [--show N] [--json OUT]

It runs `taskcore dump --fold off --reduce d` on each file, so the sets and effects compared are those (d) leaves.
"""

import argparse
import json
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path


def dump(taskcore, path, start, out, how=("--fold", "off", "--reduce", "d")):
    subprocess.run([taskcore, "dump", "--task", path, "--start", start, *how, "--out", out, "--stats", out + ".stats"], check=True, stdout=subprocess.DEVNULL)
    stats = json.loads(Path(out + ".stats").read_text())
    facts, acts = {}, []
    with open(out) as f:
        for line in f:
            x = json.loads(line)
            if "fact" in x:
                facts[x["fact"]] = set(x["values"])
            else:
                pre = tuple(sorted((k, tuple(sorted(v))) for k, v in x["pre"]))
                eff = tuple(sorted((k, kind, tuple(sorted(v)), tuple(sorted((a, tuple(sorted(b))) for a, b in w))) for k, kind, v, w in x["eff"]))
                acts.append((x["label"], x["kind"], x["end"], x["cont"], x["cost"], pre, eff))
    return facts, acts, stats


def norm(c):
    """With the switch a job's and a record's set on args:X is always written; the full reading leaves it out where the
    lists known when the job was made are none (reach.md, "Lists"). So {0} on args:X, and a write of 0, are left out on
    both sides, and a record left with no effect is a no-op."""
    pre = tuple(x for x in c[5] if not (x[0].startswith("args:") and x[1] == ("0",)))
    eff = tuple(e for e in c[6] if not (e[0].startswith("args:") and e[2] == ("0",)))
    return c[:5] + (pre, eff)


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
