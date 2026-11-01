"""E004, rough-reading.md check 3: the values a rough reading reaches hold the full reading's.

usage: rough_contains.py FULL.reached ROUGH.reached [--json]

Each file is a reading's NAME.reached (key<TAB>value<TAB>...). For every fact of the full reading but the `args:`
facts (an event a havoc record names has the list 0, which covers every list), the values the full reading reaches
and the rough one does not. A fact the rough file lacks counts with all its values. Prints the counts and the first
differences; exit code 1 when any fact differs.
"""

import json
import sys


def read(path: str) -> dict:
    out = {}
    for line in open(path):
        parts = line.rstrip("\n").split("\t")
        if parts and parts[0]:
            out[parts[0]] = set(parts[1:])
    return out


def main() -> None:
    full, rough = read(sys.argv[1]), read(sys.argv[2])
    missing = {k: sorted(v - rough.get(k, set())) for k, v in full.items()
               if not k.startswith("args:") and v - rough.get(k, set())}
    out = {"facts_full": len(full), "facts_rough": len(rough),
           "facts_compared": sum(not k.startswith("args:") for k in full),
           "facts_with_values_rough_lacks": len(missing),
           "first": dict(list(sorted(missing.items()))[:20]),
           "values_rough_adds": sum(len(rough[k] - full.get(k, set())) for k in rough if not k.startswith("args:"))}
    print(json.dumps(out) if "--json" in sys.argv else json.dumps(out, indent=1))
    sys.exit(1 if missing else 0)


if __name__ == "__main__":
    main()
