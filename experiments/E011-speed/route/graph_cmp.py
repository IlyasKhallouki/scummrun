"""E011 route: the Rust landmark graph (`lama graph`) against Fast Downward's debug dump (fd.log, verbosity=debug):
the nodes in order with their labels and styles, byte for byte, and the orderings as a set (Fast Downward prints a
node's orderings in its pointer map's order).

usage: python graph_cmp.py FD_LOG RUST_DUMP   prints "same N nodes M orderings" or the differences; exit 1 on one"""
import re
import sys


def parse(text):
    nodes, edges = [], set()
    for line in text.splitlines():
        m = re.match(r'^\s*lm(\d+) \[label="(.*)"(.*)\];$', line)
        if m:
            nodes.append((int(m.group(1)), m.group(2), m.group(3)))
            continue
        m = re.match(r'^\s*lm(\d+) -> lm(\d+) \[label=(.*)\];$', line)
        if m:
            edges.add((int(m.group(1)), int(m.group(2)), m.group(3)))
    return nodes, edges


fd = open(sys.argv[1]).read()
i = fd.rfind("digraph G {")
if i < 0:
    print("no graph in the log")
    sys.exit(1)
fn, fe = parse(fd[i:fd.find("\n}", i)])
rn, re_ = parse(open(sys.argv[2]).read())
if fn == rn and fe == re_:
    print(f"same {len(fn)} nodes {len(fe)} orderings")
    sys.exit(0)
print(f"DIFF nodes fd {len(fn)} rust {len(rn)}, orderings fd {len(fe)} rust {len(re_)}")
for a, b in zip(fn, rn):
    if a != b:
        print("  node", a, "| rust", b)
        break
print("  fd only", sorted(fe - re_)[:10], "rust only", sorted(re_ - fe)[:10])
sys.exit(1)
