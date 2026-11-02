"""A taskcore stats.json (or serve reply) without what changes from run to run: times, memory, CPU.

usage: stats_norm.py FILE  -> the normalized JSON on stdout (keys sorted)
"""
import json
import sys

VOLATILE = ("seconds", "rss", "peak_rss", "cpu_seconds")


def norm(x):
    if isinstance(x, dict):
        return {k: norm(v) for k, v in x.items() if not k.startswith(VOLATILE)}
    if isinstance(x, list):
        return [norm(v) for v in x]
    return x


print(json.dumps(norm(json.load(open(sys.argv[1]))), sort_keys=True, indent=1))
