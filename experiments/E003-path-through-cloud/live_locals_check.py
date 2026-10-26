"""E003: a reading made with the Rust reader's switch --live-locals against the same scope read without it.

usage: python live_locals_check.py OFF.golden.jsonl.gz ON.golden.jsonl.gz

Both files are golden.py's reading format (reader-scope --golden, reader-port read --out, or a golden file). Every
list of locals the OFF reading built, with the slot flags its event carries, is mapped to the ON list of its read
locals with the same flags (unit name, the locals the ON head lists for its unit, override, cutscene data, resistant,
owned, and recursive as "true or not": scumm/reader/PORT-NOTES.md, "Live locals"). The map must be a function and
lose no list. The OFF records with their lists renumbered by it (the `args:` writes and preconditions, the event
labels, the next events and the source event) must all be ON records. An ON record that is no renumbered OFF record
is put down to a flag set the OFF reading did not run: its list is the image of an OFF list that carried two flag
sets, or a list the OFF reading never built (a later wait point of such a variant); anything else is "unexplained".
Prints counts and labels only, no game text. The records are kept as digests, so a reading of 10^6 records fits.
"""

import collections
import gzip
import hashlib
import json
import re
import sys

LABEL = re.compile(r"^(event|esc)\((.*)@(-?\d+)#([0-9a-f]+)\)$")
COUNTS = ["runs: event", "runs: skip_cutscene", "event rounds", "records",
          "lists of locals past SIG_LIMIT (taken as unknown)",
          "lists of locals with locals their unit never reads (left out: live locals)"]


def lines(path):
    with gzip.open(path, "rt") as f:
        for ln in f:
            yield json.loads(ln)


def head_and_builder(path):
    head = builder = None
    for d in lines(path):
        if d["kind"] == "reading":
            head = d["reading"]
        elif d["kind"] == "builder":
            builder = d["builder"]
    return head, builder


def records(path):
    for d in lines(path):
        if d["kind"] == "record":
            yield d["record"]["$Record"]


def source_event(r):
    s = r["source"]
    if isinstance(s, dict) and s.get("$tuple", [None])[0] == "event":
        return s["$tuple"][1]["$Event"]
    return None


def events(r):
    for e in r["next_events"]:
        yield e["$Event"]
    e = source_event(r)
    if e is not None:
        yield e


def slot_key(ev):
    """The head's name of an event's unit (`script:N`, `obj:N`, `local:N@R`, `entry@R`, `exit@R`)."""
    u = ev["unit"]["$tuple"]
    if u[0] == "global":
        return f"script:{u[1]}"
    if u[0] == "obj":
        return f"obj:{u[1]}"
    if u[0] == "local":
        return f"local:{u[2]}@{u[1]}"
    return f"{u[0]}@{u[1]}"


def locals_of(ev):
    loc = ev["locals"]
    return loc.get("$map", []) if isinstance(loc, dict) else loc


def recursive(ev):
    return True if ev["recursive"] is True else None


def flags(ev):
    return json.dumps([recursive(ev), ev["resistant"], ev["owned"]])


def main(off_path, on_path):
    _, off_b = head_and_builder(off_path)
    on_head, on_b = head_and_builder(on_path)
    slots = on_head["live_locals"]["slots"]
    missing = collections.Counter()

    def read_locals(ev):
        k = slot_key(ev)
        if k not in slots:
            missing[ev["name"]] += 1
            return locals_of(ev)
        reads = slots[k]
        return locals_of(ev) if reads is None else [kv for kv in locals_of(ev) if kv[0] in set(reads)]

    def key(ev, locs):
        return json.dumps([ev["name"], locs, ev["override"], ev["cut_data"], recursive(ev), ev["resistant"],
                           ev["owned"]], sort_keys=True)

    off_keys = collections.defaultdict(set)
    for r in records(off_path):
        for e in events(r):
            if e["sig"]:
                off_keys[e["sig"]].add(key(e, read_locals(e)))
    on_sigs = collections.defaultdict(set)
    for r in records(on_path):
        for e in events(r):
            if e["sig"]:
                on_sigs[key(e, locals_of(e))].add(e["sig"])
    sigmap, lost = {}, 0
    for s, ks in off_keys.items():
        for k in ks:
            if k in on_sigs:
                sigmap[(s, json.dumps(json.loads(k)[4:]))] = next(iter(on_sigs[k]))
            else:
                lost += 1
    print(f"lists: OFF {len(off_keys)}, ON {len(on_sigs)}; OFF lists carrying two flag sets "
          f"{sum(1 for ks in off_keys.values() if len(ks) > 1)}; ON keys with two numbers "
          f"{sum(1 for ss in on_sigs.values() if len(ss) > 1)}; OFF (list, flags) with no ON list {lost}; "
          f"OFF (list, flags) mapped {len(sigmap)} onto {len(set(sigmap.values()))} ON lists")

    def m(v, fl):
        return 0 if v == 0 else sigmap.get((v, fl), ("unmapped", v))

    def renum_event(e):
        e = dict(e)
        e["sig"] = m(e["sig"], flags(e)) if e["sig"] else 0
        e["recursive"] = recursive(e)
        locs = read_locals(e)
        e["locals"] = {"$map": locs} if locs else {}
        return e

    def renum(r):
        r = dict(r)
        nfl = {e["$Event"]["name"]: flags(e["$Event"]) for e in r["next_events"]}
        src = source_event(r)
        r["effects"] = {k: ({"$tuple": ["set", m(v["$tuple"][1], nfl.get(k[5:]))]}
                            if k.startswith("args:") and v["$tuple"][0] == "set" else v)
                        for k, v in r["effects"].items()}
        pre = {}
        for k, v in r["pre"].items():
            if k.startswith("args:"):
                fl = flags(src) if src is not None and src["name"] == k[5:] else None
                v = {"$set": sorted({m(x, fl) for x in v["$set"]}, key=str)}
            pre[k] = v
        r["pre"] = pre
        g = LABEL.match(r["label"])
        if g:
            t = m(int(g[4], 16), flags(src))
            r["label"] = f"{g[1]}({g[2]}@{g[3]}#{t:x})" if isinstance(t, int) else r["label"] + "?unmapped"
        r["next_events"] = [{"$Event": renum_event(e["$Event"])} for e in r["next_events"]]
        if src is not None:
            r["source"] = {"$tuple": ["event", {"$Event": renum_event(src)}]}
        return r

    info = {}

    def digest(r):
        t = json.dumps(r, sort_keys=True)
        h = hashlib.sha1(t.encode()).digest()
        if h not in info:
            info[h] = (r["kind"], re.sub(r"#[0-9a-f]+", "#.", r["label"]), "unmapped" in t)
        return h

    off_c = collections.Counter(digest(renum(r)) for r in records(off_path))
    on_c = collections.Counter(digest(r) for r in records(on_path))
    a, b = set(off_c), set(on_c)
    print(f"records: OFF {sum(off_c.values())}, ON {sum(on_c.values())}; OFF renumbered distinct {len(a)}, ON distinct "
          f"{len(b)}; OFF renumbered not in ON {len(a - b)}, ON not in OFF renumbered {len(b - a)}; unmapped lists "
          f"used {sum(1 for x in a if info[x][2])}; events of units the ON head lacks {sum(missing.values())}")
    for name, s in (("OFF only", a - b), ("ON only", b - a)):
        if s:
            print(f"  {name}: kinds {dict(collections.Counter(info[x][0] for x in s))}, labels "
                  f"{collections.Counter(info[x][1] for x in s).most_common(8)}")
    print(f"OFF records that became copies of another once renumbered: {sum(c - 1 for c in off_c.values() if c > 1)}")
    for k in COUNTS:
        print(f"  {k}: OFF {off_b['counts'].get(k)}, ON {on_b['counts'].get(k)}")
    multi = {s for s, ks in off_keys.items() if len(ks) > 1}
    img_multi = {t for (s, _), t in sigmap.items() if s in multi}
    img_all = set(sigmap.values())
    why, units = collections.Counter(), collections.Counter()
    for r in records(on_path):
        if hashlib.sha1(json.dumps(r, sort_keys=True).encode()).digest() in a:
            continue
        e = source_event(r)
        if e is None:
            why["not an event"] += 1
            continue
        units[f"{tuple(e['unit']['$tuple'])} R={e['resistant']}"] += 1
        if e["sig"] in img_multi:
            why["list of an OFF list with two flag sets"] += 1
        elif e["sig"] and e["sig"] not in img_all:
            why["a list OFF never built (a later wait point of such a variant)"] += 1
        else:
            why["unexplained"] += 1
    print(f"ON-only records by cause: {dict(why)}; by unit: {dict(units)}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
