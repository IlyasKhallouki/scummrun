"""The exactness check of the reader's switch --reach (reach.md): the actions of a reading with the switch against the
actions step (d) keeps of the full reading, as canonical texts, and the fact values (d) reaches from each.

    python reach_check.py FULL.tc REACH.tc [--start start] [--reached REACH.reached] [--show N] [--json OUT]

Reads taskcore's packed files directly. (d) here is reduce.rs `red_d_core`'s closure on the actions as packed.rs loads
them, and taskcore's own count of what it keeps is the check on this copy (--taskcore BIN).
"""

import argparse
import json
import struct
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

MAGIC, END_MAGIC, NONE = 0x524F4354, 0x444E4554, 0xFFFFFFFF


class Packed:
    def __init__(self, path):
        data = Path(path).read_bytes()
        w = struct.unpack(f"<{len(data) // 4}I", data[: len(data) // 4 * 4])
        assert w[0] == MAGIC and w[-1] == END_MAGIC, path
        body, nact, o_str, o_facts, o_adds, o_goal, o_starts = w[-8:-1]
        nstr = w[o_str]
        lens = w[o_str + 1 : o_str + 1 + nstr]
        p = (o_str + 1 + nstr) * 4
        self.strs = []
        for n in lens:
            self.strs.append(data[p : p + n].decode())
            p += n
        q = o_facts
        nf = w[q]
        q += 1
        self.keys, self.doms = [], []
        for _ in range(nf):
            self.keys.append(self.strs[w[q]])
            n = w[q + 1]
            self.doms.append(list(w[q + 2 : q + 2 + n]))
            q += 2 + n
        self.index = [{v: i for i, v in enumerate(d)} for d in self.doms]
        q = o_adds
        nadd = w[q]
        q += 1
        self.adds = {}
        for _ in range(nadd):
            fact, d, n = w[q], w[q + 1], w[q + 2]
            q += 3
            tbl = []
            for _ in range(n):
                m = w[q]
                tbl.append(list(w[q + 1 : q + 1 + m]))
                q += 1 + m
            self.adds[(fact, d)] = tbl
        q = o_goal
        ng = w[q]
        q += 1
        self.goal = []
        for _ in range(ng):
            f, n = w[q], w[q + 1]
            self.goal.append((f, list(w[q + 2 : q + 2 + n])))
            q += 2 + n
        q = o_starts
        ns = w[q]
        q += 1
        self.starts = {}
        for _ in range(ns):
            self.starts[self.strs[w[q]]] = list(w[q + 1 : q + 1 + nf])
            q += 1 + nf
        self.acts = []
        q = body
        for _ in range(nact):
            n = w[q]
            b = w[q + 1 : q + 1 + n]
            q += 1 + n
            self.acts.append(self._action(b))

    def _sets(self, b, c, n):
        out = []
        for _ in range(n):
            f, m = b[c], b[c + 1]
            out.append((f, tuple(b[c + 2 : c + 2 + m])))
            c += 2 + m
        return out, c

    def _action(self, b):
        rec, label, kind, end, cont, cost = b[:6]
        pre, c = self._sets(b, 7, b[6])
        neff = b[c]
        c += 1
        effs = []
        for _ in range(neff):
            op, fact = b[c], b[c + 1]
            c += 2
            if op == 0:
                pay = (b[c],)
                c += 1
            elif op == 1:
                n = b[c]
                pay = tuple(b[c + 1 : c + 1 + n])
                c += 1 + n
            else:
                pay = (b[c],)
                c += 1
            when, c = self._sets(b, c + 1, b[c])
            effs.append((op, fact, pay, when))
        return (rec, label, kind, end, cont, cost, pre, effs)

    def text_sets(self, sets):
        return tuple(sorted((self.keys[f], tuple(sorted(self.strs[v] for v in vs))) for f, vs in sets))

    def canon(self, a):
        rec, label, kind, end, cont, cost, pre, effs = a
        es = []
        for op, fact, pay, when in effs:
            if op == 2:
                p = ("add", struct.unpack("<i", struct.pack("<I", pay[0]))[0])
            else:
                p = tuple(sorted(self.strs[v] for v in pay))
            es.append((self.keys[fact], op, p, self.text_sets(when)))
        return (self.strs[label], self.strs[kind], self.strs[end], self.strs[cont], cost, self.text_sets(pre), tuple(sorted(es)))

    def expanded(self, a):
        """The action as packed.rs loads it: precondition sets and effects as (when sets, fact, values or None)."""
        _, _, _, _, _, _, pre, effs = a
        ix = self.index
        cut = lambda f, vs: tuple(sorted(ix[f][v] for v in vs if v in ix[f]))
        p = [(f, cut(f, vs)) for f, vs in pre]
        out = []
        for op, fact, pay, when in effs:
            w = [(f, cut(f, vs)) for f, vs in when]
            if op == 0 or op == 1:
                if all(v in ix[fact] for v in pay):
                    out.append((w, fact, tuple(ix[fact][v] for v in pay)))
            else:
                size = len(self.doms[fact])
                own = None
                for f, s in reversed(w):
                    if f == fact and len(s) < size:
                        own = s
                        break
                rest = [x for x in w if x[0] != fact]
                tbl = self.adds[(fact, pay[0])]
                for u in range(size):
                    if own is not None and u not in own:
                        continue
                    nxt = sorted(ix[fact][v] for v in tbl[u] if v in ix[fact])
                    if size == 1 and nxt == [u]:
                        continue
                    if nxt:
                        out.append((rest + [(fact, (u,))], fact, tuple(nxt)))
        return p, out


def closure(pk, start_name):
    """(d): the reached (fact, value index) pairs, and the actions that apply."""
    start = pk.starts[start_name]
    reached = [set() for _ in pk.keys]
    for f, v in enumerate(start):
        if v != NONE and v in pk.index[f]:
            reached[f].add(pk.index[f][v])
    exp = [pk.expanded(a) for a in pk.acts]
    applied = [False] * len(exp)
    fired = [[False] * len(e[1]) for e in exp]
    changed = True
    while changed:
        changed = False
        for i, (pre, effs) in enumerate(exp):
            if not applied[i]:
                if all(any(x in reached[f] for x in s) for f, s in pre):
                    applied[i] = True
                else:
                    continue
            for j, (w, fact, vals) in enumerate(effs):
                if fired[i][j]:
                    continue
                if all(any(x in reached[f] for x in s) for f, s in w):
                    fired[i][j] = True
                    for v in vals:
                        if v not in reached[fact]:
                            reached[fact].add(v)
                            changed = True
    return reached, applied


def texts(pk, reached):
    return {pk.keys[f]: {pk.strs[pk.doms[f][x]] for x in r} for f, r in enumerate(reached)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("full")
    ap.add_argument("reach")
    ap.add_argument("--start", default="start")
    ap.add_argument("--reached")
    ap.add_argument("--show", type=int, default=5)
    ap.add_argument("--json")
    ap.add_argument("--taskcore")
    a = ap.parse_args()
    f, r = Packed(a.full), Packed(a.reach)
    rf, af = closure(f, a.start)
    rr, ar = closure(r, a.start)
    out = {"full_actions": len(f.acts), "reach_actions": len(r.acts)}
    out["d_keeps_of_full"] = sum(af)
    out["d_keeps_of_reach"] = sum(ar)
    if a.taskcore:
        with tempfile.TemporaryDirectory() as d:
            for name, path, mine in (("full", a.full, sum(af)), ("reach", a.reach, sum(ar))):
                st = Path(d) / "s.json"
                subprocess.run([a.taskcore, "sas", "--task", path, "--start", a.start, "--out", str(Path(d) / "t.sas"), "--fold", "off", "--reduce", "d", "--stats", str(st)], check=True, stdout=subprocess.DEVNULL)
                n = json.loads(st.read_text())["actions_after_reduce"]
                out[f"taskcore_d_keeps_of_{name}"] = n
                assert n == mine, f"{name}: taskcore's (d) keeps {n}, this copy {mine}"
    # args:X with the single value 0 in the reading with the switch: no kept record names a list of X, so the fact is
    # constant and a precondition or a write of it says nothing. The full reading's domain may hold lists only unkept
    # records named. Both sides leave it out, and a record left with no effect is a no-op on either side.
    const = {k for k, d in zip(r.keys, r.doms) if k.startswith("args:") and [r.strs[v] for v in d] == ["0"]}
    def norm(c):
        pre = tuple(x for x in c[5] if not (x[0] in const and x[1] == ("0",)))
        eff = tuple(e for e in c[6] if not (e[0] in const and e[2] == ("0",)))
        return c[:5] + (pre, eff)
    kf = Counter(n for n in (norm(f.canon(x)) for x, k in zip(f.acts, af) if k) if n[6])
    kr = Counter(n for n in (norm(r.canon(x)) for x, k in zip(r.acts, ar) if k) if n[6])
    out["args_facts_constant_in_reach"] = len(const)
    only_full = kf - kr
    only_reach = kr - kf
    out["kept_by_d_not_written"] = sum(only_full.values())
    out["written_not_kept_by_d"] = sum(only_reach.values())
    tf, tr = texts(f, rf), texts(r, rr)
    # a fact the reading with the switch has no record on is only its start value in the full file
    st_f = {f.keys[i]: (f.strs[v] if v != NONE else None) for i, v in enumerate(f.starts[a.start])}
    only_f = {}
    facts_absent = 0
    for k in tf:
        d = tf[k] - tr.get(k, set())
        if k not in tr:
            facts_absent += 1
            d -= {st_f[k]}
        if d:
            only_f[k] = sorted(d)
    out["facts_only_in_full_file"] = facts_absent
    only_r = {k: sorted(tr[k] - tf.get(k, set())) for k in tr if tr[k] - tf.get(k, set())}
    out["values_reached_full"] = sum(len(v) for v in tf.values())
    out["values_reached_reach"] = sum(len(v) for v in tr.values())
    out["values_only_full"] = only_f
    out["values_only_reach"] = only_r
    if a.reached:
        file_vals = {}
        for line in Path(a.reached).read_text().splitlines():
            parts = line.split("\t")
            file_vals[parts[0]] = set(x for x in parts[1:] if x)
        # a list SIG_LIMIT folded is in the reader's values with its number, and 0 in the file
        bad = {k: (sorted(file_vals.get(k, set()) - v), sorted(v - file_vals.get(k, set()))) for k, v in tr.items() if v - file_vals.get(k, set()) or (file_vals.get(k, set()) - v and not k.startswith("args:"))}
        out["reached_file_differs_from_d"] = bad
    # Lemma 2's class: the record needs (pos:X = w, args:X = s), and no kept record of the full reading writes that pair
    pairs = set()
    for x, k in zip(f.acts, af):
        if not k:
            continue
        c = f.canon(x)
        pre = dict(c[5])
        sets = {}
        for key, op, p, when in c[6]:
            if op == 0:
                sets[key] = p[0]
        for key in set(sets) | set(pre):
            if key.startswith("args:"):
                name = key[5:]
                w = sets.get("pos:" + name) or (pre.get("pos:" + name, ("",))[0] if len(pre.get("pos:" + name, ())) == 1 else None)
                s_ = sets.get(key) or (pre[key][0] if len(pre.get(key, ())) == 1 else None)
                if w is not None and s_ is not None:
                    pairs.add((name, w, s_))
    for k in ("start",):
        st = f.starts[a.start]
        for i, key in enumerate(f.keys):
            if key.startswith("args:") and st[i] != NONE:
                name = key[5:]
                if "pos:" + name in f.keys:
                    pairs.add((name, st_f.get("pos:" + name), f.strs[st[i]]))
    cls = Counter()
    other = []
    for c in only_full.elements():
        pre = dict(c[5])
        ar = [k for k in pre if k.startswith("args:") and pre[k] != ("0",)]
        unnamed = False
        for k in ar:
            name = k[5:]
            pos = pre.get("pos:" + name, ())
            if len(pos) == 1 and (name, pos[0], pre[k][0]) not in pairs:
                unnamed = True
        if unnamed:
            cls["lemma 2: a (pos, args) pair no kept record writes"] += 1
        else:
            cls["other"] += 1
            other.append(c)
    out["kept_by_d_not_written_classes"] = dict(cls)
    wf = Counter(c[0] for c in only_full.elements())
    wr = Counter(c[0] for c in only_reach.elements())
    out["only_full_by_label"] = wf.most_common(20)
    out["only_reach_by_label"] = wr.most_common(20)
    brief = {k: (v if not isinstance(v, dict) else {x: (y if len(str(y)) < 200 else f"{len(y)} items") for x, y in list(v.items())[:40]}) for k, v in out.items()}
    print(json.dumps(brief, indent=1))
    for name, bag in (("kept by (d), not written, not in lemma 2's class", other), ("written, not kept by (d)", only_reach)):
        for c in list(bag)[: a.show]:
            print(name, ":", c)
    if a.json:
        Path(a.json).write_text(json.dumps(out, indent=1))


if __name__ == "__main__":
    sys.exit(main())
