"""pairs_cmp.py TASKCORE A.tc B.tc [--start NAME] [--json OUT]: the check of reach_pairs (reach-pairs.md, "Checks", 3). A is read with
--reach alone, B with reach_pairs too, from the same start. Compares the actions of both files (`taskcore dump --fold off
--reduce none`, as canonical texts), the fact values (d) reaches in each (`--reduce d`), and the reader's own reached
values (NAME.reached). Every action of B must be one of A's; the actions of A that B lacks are listed by job label, with
whether B has any action of that label (a job held on a pair writes none)."""
import json, subprocess, sys, tempfile
from collections import Counter
from pathlib import Path


def dump(tc, path, how, start):
    with tempfile.TemporaryDirectory(dir=Path(path).parent) as d:
        out = f"{d}/x.jsonl"
        subprocess.run([tc, "dump", "--task", path, "--start", start, "--fold", "off", "--reduce", how, "--out", out, "--stats", out + ".stats"], check=True, stdout=subprocess.DEVNULL)
        facts, acts = {}, Counter()
        for line in open(out):
            x = json.loads(line)
            if "fact" in x:
                facts[x["fact"]] = tuple(sorted(x["values"]))
            else:
                pre = tuple(sorted((k, tuple(sorted(v))) for k, v in x["pre"]))
                eff = tuple(sorted((k, kind, tuple(sorted(v)), tuple(sorted((a, tuple(sorted(b))) for a, b in w))) for k, kind, v, w in x["eff"]))
                acts[(x["label"], x["kind"], x["end"], x["cont"], x["cost"], pre, eff)] += 1
        return facts, acts


def reached(path):
    p = Path(path).with_suffix(".reached")
    if not p.exists():
        return None
    return {l.split("\t")[0]: tuple(sorted(l.rstrip("\n").split("\t")[1:])) for l in p.read_text().splitlines() if l}


def main():
    tc, a, b = sys.argv[1:4]
    out = sys.argv[sys.argv.index("--json") + 1] if "--json" in sys.argv else None
    start = sys.argv[sys.argv.index("--start") + 1] if "--start" in sys.argv else "start"
    _, A = dump(tc, a, "none", start)
    _, B = dump(tc, b, "none", start)
    fa, _ = dump(tc, a, "d", start)
    fb, _ = dump(tc, b, "d", start)
    extra = B - A
    lost = A - B
    labels_b = Counter(k[0].split("#")[0] for k in B.elements())
    fam = Counter()
    held = Counter()
    for k, n in lost.items():
        lab = k[0].split("#")[0]
        fam[lab] += n
        if lab not in labels_b:
            held[lab] += n
    ra, rb = reached(a), reached(b)
    d_diff = sorted(k for k in set(fa) | set(fb) if fa.get(k) != fb.get(k))
    r_diff = sorted(k for k in set(ra or {}) | set(rb or {}) if (ra or {}).get(k) != (rb or {}).get(k)) if ra is not None and rb is not None else None
    res = {
        "actions_a": sum(A.values()),
        "actions_b": sum(B.values()),
        "b_not_in_a": sum(extra.values()),
        "a_not_in_b": sum(lost.values()),
        "a_not_in_b_of_labels_b_lacks": sum(held.values()),
        "labels_b_lacks": len(held),
        "families": dict(fam.most_common(25)),
        "d_values_differ": d_diff[:50],
        "n_d_values_differ": len(d_diff),
        "reader_reached_differ": None if r_diff is None else r_diff[:50],
        "n_reader_reached_differ": None if r_diff is None else len(r_diff),
    }
    if extra:
        res["b_not_in_a_examples"] = [str(k)[:300] for k in list(extra)[:5]]
    if r_diff:
        res["reader_reached_examples"] = {k: [(ra or {}).get(k), (rb or {}).get(k)] for k in r_diff[:10]}
    if d_diff:
        res["d_examples"] = {k: [fa.get(k), fb.get(k)] for k in d_diff[:10]}
    text = json.dumps(res, indent=1)
    print(text)
    if out:
        Path(out).write_text(text + "\n")


main()
