"""E005: the havoc records of a reading, from READER_TRACE's havoc lines (havoc.rs `havoc_record`, par.rs `trace_havoc`).

usage: havoc_summary.py TRACE [--units N]

Each line: "havoc LABEL: U units, E effects, V events; at ...; pending [...]; every unit: [...]" and, with havoc_rooms or
havoc_args on, "; rooms: ...; starts named K, not [...]". Prints JSON: how many havoc records, how many name every unit
(the closure's reasons), the closure sizes, the room changes and the rooms they name, the starts havoc_args named, and
the jobs with the most havoc records. `--units N` also counts the records whose closure has at least N units. No game
text is read or written: labels are job names and unit numbers.
"""

import collections
import json
import re
import statistics
import sys

LINE = re.compile(r"^havoc (?P<label>.*?): (?P<units>\d+) units, (?P<eff>\d+) effects, (?P<ev>\d+) events; at (?P<at>.*?); "
                  r"pending (?P<pending>.*?); every unit: (?P<why>\[.*?\])(?:; rooms: (?P<rooms>.*?); starts named "
                  r"(?P<named>\d+), not (?P<not>\[.*\]))?$")


def main() -> None:
    path = sys.argv[1]
    big = int(sys.argv[sys.argv.index("--units") + 1]) if "--units" in sys.argv else None
    units, eff, ev = [], [], []
    every, rooms_every, rooms_none, rooms_n, named, unnamed = 0, 0, 0, [], 0, 0
    why = collections.Counter()
    room_why = collections.Counter()
    jobs = collections.Counter()
    n_big = 0
    bad = 0
    with open(path, errors="replace") as f:
        for line in f:
            if not line.startswith("havoc "):
                continue
            m = LINE.match(line.rstrip("\n"))
            if not m:
                bad += 1
                continue
            u = int(m["units"])
            units.append(u)
            eff.append(int(m["eff"]))
            ev.append(int(m["ev"]))
            if big is not None and u >= big:
                n_big += 1
            if m["why"] != "[]":
                every += 1
                why[m["why"]] += 1
            jobs[m["label"].split("#")[0]] += 1
            r = m["rooms"]
            if r is not None:
                named += int(m["named"])
                unnamed += 0 if m["not"] == "[]" else 1
                if r == "none":
                    rooms_none += 1
                elif r.startswith("every scope room"):
                    rooms_every += 1
                    room_why[r[len("every scope room "):][:160]] += 1
                else:
                    rooms_n.append(len([x for x in r.strip("{}").split(",") if x.strip()]))
    q = lambda xs, p: sorted(xs)[min(len(xs) - 1, int(p * len(xs)))] if xs else None
    out = {
        "havoc_records": len(units),
        "naming_every_unit": every,
        "closure_units": {"min": min(units, default=None), "median": q(units, 0.5), "p90": q(units, 0.9),
                          "max": max(units, default=None)},
        "effects": {"median": q(eff, 0.5), "max": max(eff, default=None)},
        "events": {"median": q(ev, 0.5), "max": max(ev, default=None), "sum": sum(ev)},
        "room_changes": {"none": rooms_none, "every_scope_room": rooms_every, "named_rooms": len(rooms_n),
                         "rooms_named_median": statistics.median(rooms_n) if rooms_n else None,
                         "rooms_named_max": max(rooms_n, default=None)},
        "starts_named_by_havoc_args": named,
        "records_with_a_start_not_resolved": unnamed,
        "every_unit_reasons": dict(why.most_common(5)),
        "every_room_reasons": dict(room_why.most_common(5)),
        "jobs": dict(jobs.most_common(10)),
        "lines_not_parsed": bad,
    }
    if big is not None:
        out[f"closures_with_at_least_{big}_units"] = n_big
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
