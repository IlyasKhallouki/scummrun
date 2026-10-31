"""Relaxed (delete-free) reachability over a packed file (taskcore/README.md, version 1) from one of its starts: which
values a plan could reach ignoring deletes, whether the goal is among them, and what blocks chosen actions.

usage: python reach_packed.py FILE.tc START [--label SUBSTRING ...] [--writes KEY=VALUE ...]

Each round streams every action: one whose precondition sets each hold a reached value adds the values of its effects
whose conditions do too (a guess adds each of its values, an add each value the domain's add table gives from a reached
value). It stops when a round adds nothing. Then, per --label, the actions whose label holds the substring: how many,
how many apply, and the precondition sets that hold no reached value, by count; per --writes, the actions with an
effect that writes that value, by label, and what blocks them. Memory is the reached set and one action at a time.
"""

import array
import collections
import mmap
import sys

MAGIC, VERSION, END_MAGIC = 0x524F4354, 1, 0x444E4554
NONE = 0xFFFFFFFF


class Packed:
    def __init__(self, path):
        self.f = open(path, "rb")
        self.mm = mmap.mmap(self.f.fileno(), 0, access=mmap.ACCESS_READ)
        w = self.w = memoryview(self.mm).cast("I")
        if w[0] != MAGIC or w[1] != VERSION:
            raise ValueError(f"{path}: not a taskcore file of version {VERSION}")
        body, self.n, o_str, o_facts, o_adds, o_goal, self.o_starts, end = w[len(w) - 8:]
        if end != END_MAGIC:
            raise ValueError(f"{path}: bad footer")
        ns = w[o_str]
        pos = (o_str + 1 + ns) * 4
        self.s = []
        for ln in w[o_str + 1:o_str + 1 + ns]:
            self.s.append(self.mm[pos:pos + ln].decode())
            pos += ln
        p = o_facts
        self.nf = w[p]
        p += 1
        self.keys, self.doms = [], []
        for _ in range(self.nf):
            k, size = w[p], w[p + 1]
            self.keys.append(self.s[k])
            self.doms.append(list(w[p + 2:p + 2 + size]))
            p += 2 + size
        self.adds = {}
        p = o_adds
        na = w[p]
        p += 1
        for _ in range(na):
            fid, d, size = w[p], w[p + 1], w[p + 2]
            p += 3
            m = {}
            for u in self.doms[fid]:
                cnt = w[p]
                m[u] = list(w[p + 1:p + 1 + cnt])
                p += 1 + cnt
            self.adds[(fid, d)] = m
        p = o_goal
        self.goal = []
        for _ in range(w[p]):
            fid, m = w[p + 1], w[p + 2]
            self.goal.append((fid, set(w[p + 3:p + 3 + m])))
            p += 2 + m
        self.offs = array.array("Q")
        q = body
        for _ in range(self.n):
            self.offs.append(q)
            q += 1 + w[q]

    def start(self, name):
        w, p = self.w, self.o_starts
        for _ in range(w[p]):
            if self.s[w[p + 1]] == name:
                return {(fid, v) for fid, v in enumerate(w[p + 2:p + 2 + self.nf]) if v != NONE}
            p += 2 + self.nf
        raise KeyError(f"no start {name}")

    def sets(self, q, k):
        w, out = self.w, []
        for _ in range(k):
            fid, m = w[q], w[q + 1]
            out.append((fid, w[q + 2:q + 2 + m]))
            q += 2 + m
        return out, q

    def action(self, i):
        """(label, precondition sets, effects [(fid, values or None for an add, d, condition sets)])."""
        w = self.w
        q = self.offs[i] + 1
        label = self.s[w[q + 1]]
        pre, q = self.sets(q + 7, w[q + 6])
        ne = w[q]
        q += 1
        effs = []
        for _ in range(ne):
            op, fid = w[q], w[q + 1]
            d = None
            if op == 0:
                vals = [w[q + 2]]
                q += 3
            elif op == 1:
                m = w[q + 2]
                vals = list(w[q + 3:q + 3 + m])
                q += 3 + m
            else:
                vals, d = None, w[q + 2]
                q += 3
            when, q = self.sets(q + 1, w[q])
            effs.append((fid, vals, d, when))
        return label, pre, effs


def reach(pk, reached):
    meets = lambda sl: all(any((fid, v) in reached for v in vs) for fid, vs in sl)  # noqa: E731
    rnd = 0
    while True:
        rnd += 1
        new = 0
        for i in range(pk.n):
            _, pre, effs = pk.action(i)
            if not meets(pre):
                continue
            for fid, vals, d, when in effs:
                if not meets(when):
                    continue
                if vals is None:
                    tab = pk.adds[(fid, d)]
                    vals = [x for (g, u) in list(reached) if g == fid for x in tab.get(u, [])]
                for v in vals:
                    if (fid, v) not in reached:
                        reached.add((fid, v))
                        new += 1
        print(f"round {rnd}: {new} new values, {len(reached)} reached", flush=True)
        if new == 0:
            return


def main():
    path, start = sys.argv[1], sys.argv[2]
    labels, writes, mode = [], [], None
    for a in sys.argv[3:]:
        if a in ("--label", "--writes"):
            mode = a
        elif mode == "--label":
            labels.append(a)
        elif mode == "--writes":
            writes.append(a.split("=", 1))
    pk = Packed(path)
    reached = pk.start(start)
    reach(pk, reached)
    s, keys = pk.s, pk.keys
    for fid, vs in pk.goal:
        ok = any((fid, v) in reached for v in vs)
        print(f"goal {keys[fid]} in {sorted(s[v] for v in vs)}: {'reached' if ok else 'not reached'}")

    def missing(pre):
        return [(keys[fid], tuple(s[v] for v in vs)) for fid, vs in pre if not any((fid, v) in reached for v in vs)]
    for sub in labels:
        hist, app, tot = collections.Counter(), 0, 0
        for i in range(pk.n):
            label, pre, _ = pk.action(i)
            if sub in label:
                tot += 1
                miss = missing(pre)
                app += not miss
                hist.update(miss)
        print(f"label {sub}: {tot} actions, {app} apply; blocking sets: {hist.most_common(10)}")
    for key, val in writes:
        hits, app, hist = collections.Counter(), collections.Counter(), collections.Counter()
        for i in range(pk.n):
            label, pre, effs = pk.action(i)
            if any(keys[fid] == key and vals and any(s[v] == val for v in vals) for fid, vals, _, _ in effs):
                base = label.split("#")[0]
                hits[base] += 1
                miss = missing(pre)
                if not miss:
                    app[base] += 1
                hist.update(miss)
        print(f"writes {key}={val}: {sum(hits.values())} actions ({hits.most_common(6)}), {sum(app.values())} apply; "
              f"blocking sets: {hist.most_common(6)}")


if __name__ == "__main__":
    main()
