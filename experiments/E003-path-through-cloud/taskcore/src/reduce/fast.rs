//! The reductions' loop from a start, faster, with the same result (E011 round 2, plan component): the task, the counts
//! and the replaced pairs are `reduce::finish`'s and `reduce::run`'s, word for word (TASKCORE_CHECK_REDUCE compares them
//! on every request of the warm task). What changes is only how much work finds that result:
//!
//! * **(f) on groups no pass touched** (`red_f_fast`). After an (f) pass, no survivor dominates a later survivor of its
//!   group: each was checked against every earlier kept one, and every earlier survivor was kept. A later (f) pass
//!   looks at an exact-signature group whose members are all, word for word and cost for cost, survivors of the last
//!   (f) pass of this loop, in the same relative order: the members sort as they did (cost, set count, size, then
//!   position, and positions keep their order), so each member's earlier members are some of its earlier survivors
//!   then, none of which dominates it. The pass drops nothing there and is left out for that group. Two survivors never
//!   have the same words and cost (one would have dropped the other), so a member's survivor is found by its words.
//!   Members that left the group since (dropped by another pass) change nothing: fewer earlier members, still none
//!   dominating.
//! * **(f)'s dominance test, filtered** (`red_f_fast`): the same first dominator in the same keep order, with cheaper
//!   tests ahead of the walk that can only say "not a dominator": a fact of k that i lacks (a 64-bit mask, as before),
//!   and, when k and i have the same facts (each once), a value of i that k lacks on that fact (a 512-bit mask of
//!   (fact, value) pairs: dominance then needs each pair of i among k's).

use super::*;

/// The survivors of this loop's last (f) pass: their words and costs, found by a hash of both (`red_f_fast`).
#[derive(Default)]
pub struct Memo {
    words: Vec<u32>,
    off: Vec<usize>,
    cost: Vec<u32>,
    by_hash: FxMap<u64, u32>,
    valid: bool,
    /// groups left out, and groups checked, by the last pass
    pub skipped: u64,
    pub checked: u64,
}

fn content_hash(w: &[u32], cost: u32) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ cost as u64;
    for &x in w {
        h = (h.rotate_left(5) ^ x as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    h ^ (h >> 31)
}

impl Memo {
    /// The survivor of the last pass with the words and cost of `t`'s action `i`: its number then.
    fn survivor(&self, t: &Task, i: usize) -> Option<u32> {
        let w = t.acts.view(i).w;
        let cost = t.acts.meta[i].cost;
        let j = *self.by_hash.get(&content_hash(w, cost))?;
        let ju = j as usize;
        if self.cost[ju] == cost && &self.words[self.off[ju]..self.off[ju + 1]] == w {
            Some(j)
        } else {
            None
        }
    }

    fn take(&mut self, t: &Task) {
        self.words.clear();
        self.words.extend_from_slice(&t.acts.words);
        self.off.clear();
        self.off.extend_from_slice(&t.acts.off);
        self.cost.clear();
        self.cost.extend(t.acts.meta.iter().map(|m| m.cost));
        self.by_hash.clear();
        self.by_hash.reserve(t.acts.len());
        for i in 0..t.acts.len() {
            // a hash shared by two survivors keeps the first: the other is then never found, so its group is checked
            self.by_hash.entry(content_hash(t.acts.view(i).w, t.acts.meta[i].cost)).or_insert(i as u32);
        }
        self.valid = true;
    }
}

const VB_WORDS: usize = 8;

/// A member's precondition as (f)'s tests read it.
struct Member {
    span: (usize, usize),
    count: usize,
    size: u64,
    bits: u64,
    /// the exact fact list's number in the group, and whether a fact has two sets
    facts: u32,
    dup: bool,
    vb: [u64; VB_WORDS],
}

#[inline]
fn pair_bit(f: u32, v: u32) -> usize {
    let h = (f as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (v as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    ((h ^ (h >> 29)) % (64 * VB_WORDS as u64)) as usize
}

/// `red_f` with the same result (module docstring): the same groups, keep orders, first dominators, pairs and counts.
pub fn red_f_fast(t: Task, c: &mut Counts, replaced: &mut Vec<(u32, u32)>, memo: &mut Memo) -> (Task, bool) {
    let n = t.acts.len();
    // the whole task the last pass's survivors, word for word and cost for cost, in order: every group is left out
    if memo.valid && memo.off == t.acts.off && memo.words == t.acts.words && memo.cost.iter().zip(&t.acts.meta).all(|(&c, m)| c == m.cost) {
        memo.skipped = u64::MAX;
        return (t, false);
    }
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut by_hash: FxMap<u64, Vec<usize>> = FxMap::default();
    let mut scratch = SigScratch::default();
    for i in 0..n {
        let h = sig_hash(t.acts.view(i), &mut scratch);
        by_hash.entry(h).or_default().push(i);
    }
    let mut drop = vec![false; n];
    let (mut dup, mut dom) = (0u64, 0u64);
    let mut groups: Vec<Vec<usize>> = by_hash.into_values().filter(|g| g.len() > 1).collect();
    groups.sort();
    let mut arena: Vec<u32> = Vec::new();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let (mut skipped, mut checked) = (0u64, 0u64);
    let mut parena: Vec<u32> = Vec::new();
    let mut members: Vec<Member> = Vec::new();
    let mut tmp: Vec<(u32, &[u32])> = Vec::new();
    let mut fl: Vec<u32> = Vec::new();
    let mut fact_lists: FxMap<Vec<u32>, u32> = FxMap::default();
    let mut slot: FxMap<usize, usize> = FxMap::default();
    for hg in groups {
        arena.clear();
        spans.clear();
        for &i in &hg {
            let a = arena.len();
            sig_into(t.acts.view(i), &mut scratch, &mut arena);
            spans.push((a, arena.len()));
        }
        let sigs = |y: usize| &arena[spans[y].0..spans[y].1];
        let mut done = vec![false; hg.len()];
        for x in 0..hg.len() {
            if done[x] {
                continue;
            }
            let g: Vec<usize> = (x..hg.len()).filter(|&y| !done[y] && sigs(y) == sigs(x)).collect();
            for &y in &g {
                done[y] = true;
            }
            if g.len() < 2 {
                continue;
            }
            let mut ix: Vec<usize> = g.iter().map(|&y| hg[y]).collect();
            // every member a survivor of the last pass, in the same order: nothing to drop
            if memo.valid {
                let mut last: Option<u32> = None;
                let clean = ix.iter().all(|&i| match memo.survivor(&t, i) {
                    Some(j) if last.is_none_or(|l| j > l) => {
                        last = Some(j);
                        true
                    }
                    _ => false,
                });
                if clean {
                    skipped += 1;
                    continue;
                }
            }
            checked += 1;
            // each member's precondition sorted (by fact, then values), in one arena, with its filters
            parena.clear();
            members.clear();
            fact_lists.clear();
            slot.clear();
            for &i in &ix {
                tmp.clear();
                tmp.extend(t.acts.view(i).pre());
                tmp.sort();
                let a = parena.len();
                let mut size = 0u64;
                let mut bits = 0u64;
                let mut vb = [0u64; VB_WORDS];
                fl.clear();
                for &(f, s) in &tmp {
                    parena.push(f);
                    parena.push(s.len() as u32);
                    parena.extend_from_slice(s);
                    size += s.len() as u64;
                    bits |= 1 << (f % 64);
                    fl.push(f);
                    for &v in s {
                        let b = pair_bit(f, v);
                        vb[b / 64] |= 1 << (b % 64);
                    }
                }
                let dupf = fl.windows(2).any(|w| w[0] == w[1]);
                let next = fact_lists.len() as u32;
                let facts = *fact_lists.entry(fl.clone()).or_insert(next);
                slot.insert(i, members.len());
                members.push(Member { span: (a, parena.len()), count: tmp.len(), size, bits, facts, dup: dupf, vb });
            }
            let mem = |i: usize| &members[slot[&i]];
            ix.sort_by_key(|&i| (t.acts.meta[i].cost, mem(i).count, std::cmp::Reverse(mem(i).size), i));
            let mut keep: Vec<usize> = Vec::new();
            for &i in &ix {
                let mi = mem(i);
                let pi = &parena[mi.span.0..mi.span.1];
                let by = keep.iter().copied().find(|&k| {
                    let mk = mem(k);
                    if t.acts.meta[k].cost > t.acts.meta[i].cost || mk.bits & !mi.bits != 0 {
                        return false;
                    }
                    if mk.facts == mi.facts && !mk.dup && !mi.dup && (0..VB_WORDS).any(|w| mi.vb[w] & !mk.vb[w] != 0) {
                        return false;
                    }
                    pre_within(&parena[mk.span.0..mk.span.1], pi)
                });
                match by {
                    Some(k) => {
                        drop[i] = true;
                        let mk = mem(k);
                        if parena[mk.span.0..mk.span.1] == *pi && t.acts.meta[k].cost == t.acts.meta[i].cost {
                            dup += 1;
                        } else {
                            dom += 1;
                        }
                        pairs.push((i, k));
                    }
                    None => keep.push(i),
                }
            }
        }
    }
    memo.skipped = skipped;
    memo.checked = checked;
    for &(i, k) in &pairs {
        let to = t.acts.members(k)[0];
        replaced.extend(t.acts.members(i).iter().map(|&m| (m, to)));
    }
    let r = rebuild(t, |_, i, _| if drop[i] { Edit::Drop } else { Edit::Same });
    memo.take(&r.0);
    add(c, "(f) duplicate actions (same precondition, effects and cost), left out", dup);
    add(c, "(f) dominated actions (same effects, a weaker precondition at no more cost elsewhere), left out", dom);
    r
}

/// `reduce::finish` with the fast passes (module docstring); `memo` starts empty for each loop.
pub fn finish_fast(t: Task, en: Enabled, start: usize, c: &mut Counts, replaced: &mut Vec<(u32, u32)>, changed: bool, memo: &mut Memo) -> Task {
    let mut t = t;
    let mut rounds = 1u64;
    let mut changed = changed;
    let mut first = true;
    loop {
        if !first {
            rounds += 1;
            t = normalize(t, c);
            changed = false;
        }
        let mut step = |t: Task, on: bool, f: &mut dyn FnMut(Task) -> (Task, bool)| -> Task {
            if !on {
                return t;
            }
            let (nt, ch) = f(t);
            changed |= ch;
            nt
        };
        if !first {
            t = step(t, en.a, &mut |t| prof("a", t, |t| red_a(t, c)));
            t = step(t, en.b, &mut |t| prof("b", t, |t| red_b(t, c)));
            t = step(t, en.c, &mut |t| prof("c", t, |t| red_c(t, c)));
            t = step(t, en.d, &mut |t| prof("d fast", t, |t| red_d_fast(t, start, c)));
        }
        t = step(t, en.e, &mut |t| prof("e fast", t, |t| red_e_fast(t, c)));
        t = step(t, en.f, &mut |t| prof("f fast", t, |t| red_f_fast(t, c, replaced, memo)));
        first = false;
        if !changed {
            break;
        }
    }
    *c.entry("rounds of the reduction loop".into()).or_insert(0) += rounds;
    t
}

/// `reduce::run` with the fast passes.
pub fn run_fast(t: Task, en: Enabled, start: usize, c: &mut Counts, replaced: &mut Vec<(u32, u32)>, memo: &mut Memo) -> Result<Task, String> {
    let (t, changed) = prefix(t, en, c)?;
    let (t, ch) = if en.d { prof("d fast", t, |t| red_d_fast(t, start, c)) } else { (t, false) };
    Ok(finish_fast(t, en, start, c, replaced, changed | ch, memo))
}

/// The first difference between two tasks (None: the same strings, facts, goal, starts, actions word for word, metas,
/// members and input count).
pub fn task_diff(a: &Task, b: &Task) -> Option<String> {
    if a.strs.list != b.strs.list {
        return Some("strings".into());
    }
    if a.facts.key != b.facts.key || a.facts.dom_off != b.facts.dom_off || a.facts.dom != b.facts.dom {
        return Some("facts".into());
    }
    if a.goal != b.goal {
        return Some(format!("goal {:?} / {:?}", a.goal, b.goal));
    }
    if a.starts != b.starts {
        return Some("starts".into());
    }
    if a.n_orig != b.n_orig {
        return Some("n_orig".into());
    }
    if a.acts.len() != b.acts.len() {
        return Some(format!("{} / {} actions", a.acts.len(), b.acts.len()));
    }
    for i in 0..a.acts.len() {
        if a.acts.view(i).w != b.acts.view(i).w {
            return Some(format!("action {} ({}): words", i, a.name(i)));
        }
        let (m, n) = (&a.acts.meta[i], &b.acts.meta[i]);
        if (m.rec, m.label, m.kind, m.end, m.cont, m.cost) != (n.rec, n.label, n.kind, n.end, n.cont, n.cost) {
            return Some(format!("action {} ({}): meta", i, a.name(i)));
        }
        if a.acts.members(i) != b.acts.members(i) {
            return Some(format!("action {} ({}): members", i, a.name(i)));
        }
    }
    if a.acts.words != b.acts.words || a.acts.off != b.acts.off || a.acts.mem != b.acts.mem || a.acts.mem_off != b.acts.mem_off {
        return Some("the arrays".into());
    }
    None
}

/// What (d) finds from a start (`red_d_core`'s analysis, the same numbers): the new domains and goal, how values move,
/// which actions apply and which effects fire, and whether anything changes.
struct DFind {
    facts: Facts,
    goal: Vec<(u32, Vec<u32>)>,
    base: Vec<usize>,
    reached: Vec<bool>,
    remap: Vec<u32>,
    applied: Vec<bool>,
    fired: Vec<bool>,
    eoff: Vec<usize>,
    changed: bool,
}

/// `red_d_core`'s reachability, domains and counts (each count the same), without building the task. The sweep visits
/// only the actions with something left to do (not applied, or an effect not fired): the others change nothing in a
/// visit, so the rounds and what they reach are the same.
fn d_find(t: &Task, st: &[u32], c: &mut Counts) -> DFind {
    let nf = t.facts.len();
    let base: Vec<usize> = (0..nf as u32).map(|f| t.facts.base(f)).collect();
    let mut reached = vec![false; t.facts.total_values()];
    for f in 0..nf {
        if st[f] != NONE {
            reached[base[f] + st[f] as usize] = true;
        }
    }
    let n = t.acts.len();
    let mut eoff = vec![0usize; n + 1];
    for i in 0..n {
        eoff[i + 1] = eoff[i] + t.acts.view(i).neff() as usize;
    }
    let mut applied = vec![false; n];
    let mut fired = vec![false; eoff[n]];
    let mut rounds = 0u64;
    let mut pending: Vec<u32> = (0..n as u32).collect();
    let mut next: Vec<u32> = Vec::with_capacity(n);
    loop {
        rounds += 1;
        let mut new_value = false;
        next.clear();
        for &iu in &pending {
            let i = iu as usize;
            let v = t.acts.view(i);
            if !applied[i] {
                if v.pre().all(|(f, s)| s.iter().any(|&x| reached[base[f as usize] + x as usize])) {
                    applied[i] = true;
                } else {
                    next.push(iu);
                    continue;
                }
            }
            let mut left = false;
            for (j, e) in v.effs().enumerate() {
                if fired[eoff[i] + j] {
                    continue;
                }
                if e.when().all(|(f, s)| s.iter().any(|&x| reached[base[f as usize] + x as usize])) {
                    fired[eoff[i] + j] = true;
                    if e.kind != K_BAD {
                        for &x in e.vals {
                            let p = base[e.fact as usize] + x as usize;
                            if !reached[p] {
                                reached[p] = true;
                                new_value = true;
                            }
                        }
                    }
                } else {
                    left = true;
                }
            }
            if left {
                next.push(iu);
            }
        }
        std::mem::swap(&mut pending, &mut next);
        if !new_value {
            break;
        }
    }
    add(c, "(d) reachability rounds", rounds);
    let mut goal_ok = true;
    let mut keep = reached.clone();
    for (f, vals) in &t.goal {
        if !vals.iter().any(|&x| reached[base[*f as usize] + x as usize]) {
            goal_ok = false;
            for x in 0..t.facts.size(*f) {
                keep[base[*f as usize] + x as usize] = true;
            }
        }
    }
    if !goal_ok {
        add(c, "(d) the goal cannot be reached ignoring deletes (unsolvable)", 1);
    }
    for f in 0..nf {
        let r = base[f]..base[f] + t.facts.size(f as u32) as usize;
        if !keep[r.clone()].iter().any(|&b| b) {
            for p in r {
                keep[p] = true;
            }
        }
    }
    // facts an action writes twice, kept whole (`red_d_core`): only facts whose domain shrinks can have a set that
    // becomes whole, so with none the loop over the actions finds nothing
    let kept_n: Vec<usize> = (0..nf).map(|f| keep[base[f]..base[f] + t.facts.size(f as u32) as usize].iter().filter(|&&b| b).count()).collect();
    let shrinks: Vec<bool> = (0..nf).map(|f| kept_n[f] < t.facts.size(f as u32) as usize).collect();
    let becomes_whole = |f: u32, s: &[u32]| {
        if !shrinks[f as usize] {
            return false;
        }
        let (size, kn) = (t.facts.size(f) as usize, kept_n[f as usize]);
        kn < size && s.len() < size && s.iter().filter(|&&x| keep[base[f as usize] + x as usize]).count() == kn
    };
    let mut protect: Vec<u32> = Vec::new();
    if shrinks.iter().any(|&b| b) {
        let mut cand: Vec<u32> = Vec::new();
        for i in 0..n {
            if !applied[i] {
                continue;
            }
            let v = t.acts.view(i);
            cand.clear();
            cand.extend(v.pre().filter(|(f, s)| becomes_whole(*f, s)).map(|(f, _)| f));
            for e in v.effs() {
                cand.extend(e.when().filter(|(f, s)| *f == e.fact && becomes_whole(*f, s)).map(|(f, _)| f));
            }
            if cand.is_empty() {
                continue;
            }
            cand.sort_unstable();
            cand.dedup();
            protect.extend(cand.iter().copied().filter(|&k| writes_twice(v, k)));
        }
    }
    protect.sort_unstable();
    protect.dedup();
    for &k in &protect {
        for p in base[k as usize]..base[k as usize] + t.facts.size(k) as usize {
            keep[p] = true;
        }
    }
    add(c, "(d) facts an action writes twice, kept whole (a set on them would hold for every value)", protect.len() as u64);
    let mut remap = vec![NONE; keep.len()];
    let mut facts = Facts { key: t.facts.key.clone(), dom_off: vec![0], dom: Vec::new() };
    let mut dropped_values = 0u64;
    for f in 0..nf {
        let mut k = 0u32;
        for x in 0..t.facts.size(f as u32) {
            let p = base[f] + x as usize;
            if keep[p] {
                remap[p] = k;
                k += 1;
                facts.dom.push(t.facts.dom(f as u32)[x as usize]);
            } else {
                dropped_values += 1;
            }
        }
        facts.dom_off.push(facts.dom.len() as u32);
    }
    // the counts the output would give: actions not applied, effects not fired, sets with a value not reached
    let (mut dropped_actions, mut dropped_effects, mut cut_sets) = (0u64, 0u64, 0u64);
    let cut = |f: u32, s: &[u32]| s.iter().any(|&x| !reached[base[f as usize] + x as usize]);
    for i in 0..n {
        if !applied[i] {
            dropped_actions += 1;
            continue;
        }
        let v = t.acts.view(i);
        cut_sets += v.pre().filter(|(f, s)| cut(*f, s)).count() as u64;
        for (j, e) in v.effs().enumerate() {
            if !fired[eoff[i] + j] {
                dropped_effects += 1;
                continue;
            }
            cut_sets += e.when().filter(|(f, s)| cut(*f, s)).count() as u64;
        }
    }
    let goal: Vec<(u32, Vec<u32>)> = t
        .goal
        .iter()
        .map(|(f, vals)| (*f, vals.iter().filter_map(|&x| Some(remap[base[*f as usize] + x as usize]).filter(|&r| r != NONE)).collect()))
        .collect();
    add(c, "(d) actions whose precondition cannot hold, left out", dropped_actions);
    add(c, "(d) effects whose conditions cannot hold, left out", dropped_effects);
    add(c, "(d) values never reached, left out of the domains", dropped_values);
    add(c, "(d) precondition and condition sets cut to reached values", cut_sets);
    let changed = dropped_actions + dropped_effects + dropped_values + cut_sets > 0;
    DFind { facts, goal, base, reached, remap, applied, fired, eoff, changed }
}

impl DFind {
    fn remap_start(&self, vals: &[u32]) -> Vec<u32> {
        vals.iter().enumerate().map(|(f, &x)| if x == NONE { NONE } else { self.remap[self.base[f] + x as usize] }).collect()
    }

    /// The actions `red_d_core` builds, written word for word as `Actions::push` writes them.
    fn actions(&self, t: &Task) -> Actions {
        let (base, reached, remap) = (&self.base, &self.reached, &self.remap);
        let n = t.acts.len();
        let mut acts = Actions::default();
        acts.words.reserve(t.acts.words.len());
        acts.off.reserve(n);
        acts.meta.reserve(n);
        let w = &mut acts.words;
        for i in 0..n {
            if !self.applied[i] {
                continue;
            }
            let v = t.acts.view(i);
            let np = v.pre().count();
            w.push(np as u32);
            for (f, s) in v.pre() {
                w.push(f);
                let at = w.len();
                w.push(0);
                let b = base[f as usize];
                for &x in s {
                    if reached[b + x as usize] {
                        w.push(remap[b + x as usize]);
                    }
                }
                w[at] = (w.len() - at - 1) as u32;
            }
            let at_n = w.len();
            w.push(0);
            let mut ne = 0u32;
            for (j, e) in v.effs().enumerate() {
                if !self.fired[self.eoff[i] + j] {
                    continue;
                }
                ne += 1;
                w.push(e.fact);
                w.push(e.kind);
                if e.kind == K_BAD {
                    w.push(e.vals.len() as u32);
                    w.extend_from_slice(e.vals);
                } else {
                    let at = w.len();
                    w.push(0);
                    let b = base[e.fact as usize];
                    for &x in e.vals {
                        let r = remap[b + x as usize];
                        if r != NONE {
                            w.push(r);
                        }
                    }
                    w[at] = (w.len() - at - 1) as u32;
                }
                w.push(e.nwhen());
                for (f, s) in e.when() {
                    w.push(f);
                    let at = w.len();
                    w.push(0);
                    let b = base[f as usize];
                    for &x in s {
                        if reached[b + x as usize] {
                            w.push(remap[b + x as usize]);
                        }
                    }
                    w[at] = (w.len() - at - 1) as u32;
                }
            }
            w[at_n] = ne;
            acts.off.push(w.len());
            acts.meta.push(t.acts.meta[i]);
            acts.mem.extend_from_slice(t.acts.members(i));
            acts.mem_off.push(acts.mem.len());
        }
        acts
    }
}

/// `red_d` with the same result (`d_find`, then the actions written in place of built; a (d) that changes nothing
/// gives back its task as it is: no action, effect, value or set is left out, so every word is the same).
pub fn red_d_fast(t: Task, start: usize, c: &mut Counts) -> (Task, bool) {
    let st = t.starts[start].1.clone();
    let d = d_find(&t, &st, c);
    if !d.changed {
        let mut t = t;
        t.starts = t.starts.iter().map(|(name, vals)| (*name, d.remap_start(vals))).collect();
        return (t, false);
    }
    let acts = d.actions(&t);
    let starts = t.starts.iter().map(|(name, vals)| (*name, d.remap_start(vals))).collect();
    (Task { strs: t.strs, facts: d.facts, goal: d.goal, starts, acts, n_orig: t.n_orig }, true)
}

/// `red_d_from` with the same result.
pub fn red_d_from_fast(t: &Task, push: Option<(u32, Vec<u32>)>, start: usize, c: &mut Counts) -> (Task, bool) {
    let st: &[u32] = match &push {
        Some((_, vals)) => vals,
        None => &t.starts[start].1,
    };
    let d = d_find(t, st, c);
    let mut starts: Vec<(u32, Vec<u32>)> = t.starts.iter().map(|(name, vals)| (*name, d.remap_start(vals))).collect();
    if let Some((name, vals)) = push {
        starts.push((name, d.remap_start(&vals)));
    }
    let acts = if d.changed { d.actions(t) } else { t.acts.clone() };
    (Task { strs: t.strs.clone(), facts: d.facts, goal: d.goal, starts, acts, n_orig: t.n_orig }, d.changed)
}

/// (e)'s needed values (`needed_src` on an `Actions`), the same sweep visit for visit, with each effect's facts and
/// value ranges read from a table instead of decoded from the words on every visit, and each effect's rule (i) and
/// rule (ii) loops run once: after a loop has run, every value it sets is needed for good, so running it again sets
/// nothing (no value, no count, no clock: the rest of the sweep sees the same state).
#[allow(unused_assignments)]
pub fn needed_fast(acts: &Actions, facts: &Facts, goal: &[(u32, Vec<u32>)]) -> Needed {
    let nf = facts.len();
    let base: Vec<usize> = (0..nf as u32).map(|f| facts.base(f)).collect();
    let mut need = vec![false; facts.total_values()];
    let mut ncount = vec![0u32; nf];
    let n = acts.len();
    let mut kept = vec![false; n];
    #[allow(unused_assignments)]
    let mut ch = false;
    let mut clock: u64 = 1;
    let mut fact_clock = vec![0u64; nf];
    #[allow(unused_assignments)]
    macro_rules! set {
        ($f:expr, $x:expr) => {{
            let p = base[$f as usize] + $x as usize;
            if !need[p] {
                need[p] = true;
                ncount[$f as usize] += 1;
                ch = true;
                clock += 1;
                fact_clock[$f as usize] = clock;
            }
        }};
    }
    for (f, vals) in goal {
        for &x in vals {
            set!(*f, x);
        }
    }
    // the tables: per action its written facts (sorted, unique), its precondition's facts, its effects
    let w = &acts.words;
    let mut wf_off = vec![0usize; n + 1];
    let mut wf: Vec<u32> = Vec::new();
    let mut pf_off = vec![0usize; n + 1];
    let mut pf: Vec<u32> = Vec::new();
    // per effect: fact, kind, values (start, len) and conditions (position of the count word) in `w`
    let mut e_off = vec![0usize; n + 1];
    let mut e_fact: Vec<u32> = Vec::new();
    let mut e_kind: Vec<u32> = Vec::new();
    let mut e_vals: Vec<(u32, u32)> = Vec::new();
    let mut e_when: Vec<u32> = Vec::new();
    // per action its precondition sets as (fact, start, len)
    let mut ps_off = vec![0usize; n + 1];
    let mut ps: Vec<(u32, u32, u32)> = Vec::new();
    let mut fs: Vec<u32> = Vec::new();
    for i in 0..n {
        let mut p = acts.off[i];
        let npre = w[p] as usize;
        p += 1;
        for _ in 0..npre {
            let (f, k) = (w[p], w[p + 1] as usize);
            pf.push(f);
            ps.push((f, (p + 2) as u32, k as u32));
            p += 2 + k;
        }
        pf_off[i + 1] = pf.len();
        ps_off[i + 1] = ps.len();
        let neff = w[p] as usize;
        p += 1;
        fs.clear();
        for _ in 0..neff {
            let (f, kind, k) = (w[p], w[p + 1], w[p + 2] as usize);
            e_fact.push(f);
            e_kind.push(kind);
            e_vals.push(((p + 3) as u32, k as u32));
            p += 3 + k;
            e_when.push(p as u32);
            let nw = w[p] as usize;
            p += 1;
            for _ in 0..nw {
                p += 2 + w[p + 1] as usize;
            }
            if kind != K_BAD {
                fs.push(f);
            }
        }
        e_off[i + 1] = e_fact.len();
        fs.sort_unstable();
        fs.dedup();
        wf.extend_from_slice(&fs);
        wf_off[i + 1] = wf.len();
    }
    let ne = e_fact.len();
    let mut done_i = vec![false; ne];
    let mut done_ii = vec![false; ne];
    let mut seen = vec![0u64; n];
    let mut rounds = 0u64;
    let vals_of = |e: usize| &w[e_vals[e].0 as usize..(e_vals[e].0 + e_vals[e].1) as usize];
    loop {
        rounds += 1;
        ch = false;
        for i in 0..n {
            let prev = seen[i];
            if prev != 0 && wf[wf_off[i]..wf_off[i + 1]].iter().all(|&f| fact_clock[f as usize] <= prev) {
                continue;
            }
            seen[i] = clock;
            let mut every = prev == 0;
            if !kept[i] {
                let hit = (e_off[i]..e_off[i + 1]).any(|e| {
                    let f = e_fact[e] as usize;
                    e_kind[e] != K_BAD && (every || fact_clock[f] > prev) && vals_of(e).iter().any(|&x| need[base[f] + x as usize])
                });
                if !hit {
                    continue;
                }
                kept[i] = true;
                ch = true;
                every = true;
                for &(f, s, k) in &ps[ps_off[i]..ps_off[i + 1]] {
                    for &x in &w[s as usize..(s + k) as usize] {
                        set!(f, x);
                    }
                }
            }
            let prefs = &pf[pf_off[i]..pf_off[i + 1]];
            for e in e_off[i]..e_off[i + 1] {
                let wp = e_when[e] as usize;
                let nwhen = w[wp] as usize;
                if e_kind[e] == K_BAD || nwhen == 0 {
                    continue;
                }
                let k = e_fact[e] as usize;
                if !every && fact_clock[k] <= prev {
                    continue;
                }
                if ncount[k] == 0 {
                    continue;
                }
                let vals = vals_of(e);
                let hits = vals.iter().filter(|&&x| need[base[k] + x as usize]).count();
                let case_i = hits > 0;
                let case_ii = if e_kind[e] == K_GUESS && vals.len() > 1 { true } else { ncount[k] as usize > hits };
                let (do_i, do_ii) = (case_i && !done_i[e], case_ii && !done_ii[e]);
                if !do_i && !do_ii {
                    continue;
                }
                let mut p = wp + 1;
                for _ in 0..nwhen {
                    let (cf, cn) = (w[p], w[p + 1] as usize);
                    let cs = &w[p + 2..p + 2 + cn];
                    p += 2 + cn;
                    if prefs.contains(&cf) {
                        continue;
                    }
                    if do_i {
                        for &x in cs {
                            set!(cf, x);
                        }
                    }
                    if do_ii {
                        for x in 0..facts.size(cf) {
                            if !contains(cs, x) {
                                set!(cf, x);
                            }
                        }
                    }
                }
                done_i[e] |= do_i;
                done_ii[e] |= do_ii;
            }
        }
        if !ch {
            break;
        }
    }
    Needed { ncount, kept, rounds }
}

/// `red_e` with `needed_fast` (the same needed values, kept actions and rounds, so the same task and counts).
pub fn red_e_fast(t: Task, c: &mut Counts) -> (Task, bool) {
    let nf = t.facts.len();
    let t0 = std::time::Instant::now();
    let Needed { ncount, kept, rounds, .. } = needed_fast(&t.acts, &t.facts, &t.goal);
    if std::env::var_os("TASKCORE_PROFILE").is_some() {
        eprintln!("profile e fast: needed {:.3} s, {} rounds", t0.elapsed().as_secs_f64(), rounds);
    }
    let whole = (0..nf).filter(|&f| ncount[f] > 0 && ncount[f] == t.facts.size(f as u32)).count();
    let needed: u64 = ncount.iter().map(|&x| x as u64).sum();
    let (mut da, mut de) = (0u64, 0u64);
    let r = rebuild(t, |_, i, v| {
        if !kept[i] {
            da += 1;
            return Edit::Drop;
        }
        let k = v.effs().filter(|e| e.kind != K_BAD && ncount[e.fact as usize] == 0).count();
        if k == 0 {
            return Edit::Same;
        }
        de += k as u64;
        Edit::Words(v.words_keeping(|_, e| e.kind == K_BAD || ncount[e.fact as usize] > 0))
    });
    add(c, "(e) actions that write no needed value, left out", da);
    add(c, "(e) effects on facts with no needed value, left out", de);
    c.insert("(e) last pass: needed values".into(), needed);
    c.insert("(e) last pass: facts needed whole".into(), whole as u64);
    c.insert("(e) last pass: facts with a needed value".into(), ncount.iter().filter(|&&x| x > 0).count() as u64);
    add(c, "(e) relevance rounds", rounds);
    r
}
