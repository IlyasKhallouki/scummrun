//! The fork fold: plan step 1's "other branches become conditional effects", done on the task.
//!
//! The reader makes one action per path: a handler that forks on a fact gives one action per branch, each with the
//! branch's values in its precondition. The fold joins the paths of one input (one label) that end the same way
//! and leave the same continuation (the events the path leaves: unit, wait point, Esc or not), into one action:
//!
//! * group: same label, record kind, path end, continuation and cost. `Strict` (the default) folds only paths that
//!   end in a yield: a yield names its site, so these are paths that meet again (step 1's rule). `Every` also folds
//!   paths that end the handler (end, stopped, chained, room, parked) with the same end kind; the record keeps the
//!   end's kind, not its site, so this joins an early return with the last one, which step 1 keeps apart (a
//!   switch, off by default). Actions with a precondition no value meets are left alone.
//! * split each group, in order, greedily into sub-groups of pairwise disjoint paths (two paths are disjoint when
//!   some fact in both preconditions has disjoint sets). Paths that overlap differ only in tests on non-facts
//!   (replay checks) and cannot be told apart by conditions on facts.
//! * a sub-group of two or more becomes one action: a fact with the same set on every path stays a precondition;
//!   on a fact where the sets differ the union is the precondition (left out when it is the whole domain), and each
//!   path's own set becomes a condition of that path's effects. An effect every path has stays as it is; every other
//!   effect gets its path's conditions (intersected with its own; one that becomes empty never fires and is left
//!   out).
//! * exactness: the paths are disjoint boxes inside the box of the unions; they cover it exactly when their
//!   volumes add up to its volume. Only then is the sub-group folded; a sub-group with a hole (a state of the union
//!   box where no path applies) stays one action per path, counted.
//! * the writer's PRE_LIMIT: a folded action whose precondition sets alone give more than PRE_LIMIT variants would
//!   stop the writer (task.py raises on such an action); that sub-group stays one action per path, counted.
//! * a path that writes a fact twice (reduce::double_writes) needs its set on that fact: where the union on it is
//!   the whole domain (left out), an effect every path has would lose it and the writer would raise; that
//!   sub-group stays one action per path, counted. build_task writes one effect per fact, so this never happens
//!   on its tasks.
//!
//! **Soundness (exact).** In a state where the folded action applies, the state is in the union box, so (cover) in
//! exactly one path's box (disjoint): that path applies, no other does. Conversely where a path applies, the folded
//! action applies (its sets are unions). In that state the conditions of that path hold (they are its own sets)
//! and every other path's conditions fail (the state is outside its box, on a fact where the sets differ, and that
//! set is in the other path's conditions), so the folded action makes exactly that path's writes. Same transitions,
//! same costs: every plan, optimal or not, maps to a plan of the same cost both ways. `check` tests it on sampled
//! states (corners and random points of each path's box, and random points of the union box).

use crate::model::*;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FoldMode {
    Off,
    Strict,
    Every,
}

#[derive(Default, Debug, Clone)]
pub struct FoldCounts {
    pub groups: u64,         // groups with two or more paths
    pub grouped: u64,        // paths in them
    pub subgroups: u64,      // sub-groups of two or more pairwise disjoint paths
    pub folded_paths: u64,   // paths that became part of a folded action
    pub folded_actions: u64, // folded actions made
    pub holes: u64,          // sub-groups left unfolded: their paths do not cover the union box
    pub overflow: u64,       // sub-groups left unfolded: the volume does not fit in 128 bits
    pub pre_limit: u64,      // sub-groups left unfolded: the folded precondition passes the writer's PRE_LIMIT
    pub dead_effects: u64,   // conditional effects whose conditions cannot hold together (left out)
    pub double_writes: u64,  // sub-groups left unfolded: a union of the whole domain on a fact a path writes twice
}

fn boxes(t: &Task, i: usize) -> Vec<(u32, Vec<u32>)> {
    let mut b: Vec<(u32, Vec<u32>)> = t
        .acts
        .view(i)
        .pre()
        .filter(|(f, s)| (s.len() as u32) < t.facts.size(*f))
        .map(|(f, s)| (f, s.to_vec()))
        .collect();
    b.sort();
    b
}

fn disjoint(p: &[(u32, Vec<u32>)], q: &[(u32, Vec<u32>)]) -> bool {
    let (mut i, mut j) = (0, 0);
    while i < p.len() && j < q.len() {
        if p[i].0 == q[j].0 {
            if !intersects(&p[i].1, &q[j].1) {
                return true;
            }
            i += 1;
            j += 1;
        } else if p[i].0 < q[j].0 {
            i += 1;
        } else {
            j += 1;
        }
    }
    false
}

fn canon(e: &OEff) -> OEff {
    let mut c = e.clone();
    c.when.sort();
    c
}

fn get<'a>(b: &'a [(u32, Vec<u32>)], f: u32) -> Option<&'a Vec<u32>> {
    b.binary_search_by_key(&f, |x| x.0).ok().map(|i| &b[i].1)
}

/// The folded action of sub-group `ix` (module docstring), or None when the paths do not cover the union box. With
/// `build` false it only decides (and returns an empty action): the fold builds each folded action when it writes it,
/// so it never holds them all.
fn fold_one(t: &Task, ix: &[usize], bx: &[Vec<(u32, Vec<u32>)>], c: &mut FoldCounts, build: bool) -> Option<OAct> {
    let mut keys: Vec<u32> = bx.iter().flat_map(|b| b.iter().map(|x| x.0)).collect();
    keys.sort_unstable();
    keys.dedup();
    let mut pre = Vec::new();
    let mut differ = Vec::new();
    for &k in &keys {
        let sets: Vec<Option<&Vec<u32>>> = bx.iter().map(|b| get(b, k)).collect();
        if sets.iter().all(|s| *s == sets[0]) {
            if let Some(s) = sets[0] {
                pre.push(OSet { fact: k, vals: s.clone() });
            }
            continue;
        }
        let size = t.facts.size(k);
        let mut u: Vec<u32> = Vec::new();
        for s in &sets {
            match s {
                None => {
                    u = (0..size).collect();
                    break;
                }
                Some(s) => u = union(&u, s),
            }
        }
        if (u.len() as u32) < size {
            pre.push(OSet { fact: k, vals: u.clone() });
        } else if ix.iter().any(|&i| crate::reduce::double_writes(t.acts.view(i)).contains(&k)) {
            // the union holds every value and is left out, so an effect every path has loses the path's set on k,
            // which a path that writes k twice needs (reduce::double_writes): left unfolded
            c.double_writes += 1;
            return None;
        }
        differ.push((k, u.len() as u128));
    }
    // the writer's PRE_LIMIT (module docstring); `pre` holds no set of the whole domain
    let mut pre_product: u128 = 1;
    for p in &pre {
        pre_product = pre_product.saturating_mul(p.vals.len() as u128);
    }
    if pre_product > crate::sas::PRE_LIMIT {
        c.pre_limit += 1;
        return None;
    }
    // cover: the disjoint path boxes fill the union box exactly when their volumes add up to its volume
    let mut bound: u128 = 1;
    for &(_, n) in &differ {
        match bound.checked_mul(n) {
            Some(v) => bound = v,
            None => {
                c.overflow += 1;
                return None;
            }
        }
    }
    let mut total: u128 = 0;
    for b in bx {
        let mut vol: u128 = 1;
        for &(k, _) in &differ {
            let n = get(b, k).map(|s| s.len() as u128).unwrap_or(t.facts.size(k) as u128);
            vol = vol.checked_mul(n)?;
        }
        total = total.checked_add(vol)?;
    }
    if total != bound {
        c.holes += 1;
        return None;
    }
    if !build {
        return Some(OAct::default());
    }
    let acts: Vec<OAct> = ix.iter().map(|&i| t.acts.get(i)).collect();
    let sets: Vec<HashSet<OEff>> = acts.iter().map(|a| a.eff.iter().map(canon).collect()).collect();
    let shared: HashSet<OEff> = sets[0].iter().filter(|e| sets[1..].iter().all(|s| s.contains(*e))).cloned().collect();
    let mut eff = Vec::new();
    let mut done: HashSet<OEff> = HashSet::new();
    for e in &acts[0].eff {
        let ce = canon(e);
        if shared.contains(&ce) && done.insert(ce) {
            eff.push(e.clone());
        }
    }
    for (a, b) in acts.iter().zip(bx) {
        let cond: Vec<OSet> = differ.iter().filter_map(|&(k, _)| get(b, k).map(|s| OSet { fact: k, vals: s.clone() })).collect();
        let mut mine: HashSet<OEff> = HashSet::new();
        for e in &a.eff {
            let ce = canon(e);
            if shared.contains(&ce) || !mine.insert(ce) {
                continue;
            }
            let mut when: Vec<OSet> = Vec::with_capacity(e.when.len() + cond.len());
            for s in e.when.iter().chain(cond.iter()) {
                if let Some(w) = when.iter_mut().find(|w| w.fact == s.fact) {
                    w.vals = intersect(&w.vals, &s.vals);
                } else {
                    when.push(s.clone());
                }
            }
            if when.iter().any(|w| w.vals.is_empty()) {
                c.dead_effects += 1;
                continue;
            }
            eff.push(OEff { fact: e.fact, kind: e.kind, vals: e.vals.clone(), when });
        }
    }
    Some(OAct { pre, eff })
}

/// The fold (module docstring). Returns the new task and, per folded action, (its index, its paths' indices in
/// `t`).
pub fn fold(t: Task, mode: FoldMode) -> (Task, FoldCounts, Vec<(usize, Vec<usize>)>) {
    let mut c = FoldCounts::default();
    if mode == FoldMode::Off {
        return (t, c, Vec::new());
    }
    let n = t.acts.len();
    let yield_s = t.strs.list.iter().position(|s| s == "yield").map(|p| p as u32);
    let mut gid: FxMap<(u32, u32, u32, u32, u32), usize> = FxMap::default();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for i in 0..n {
        let m = t.acts.meta[i];
        if mode == FoldMode::Strict && Some(m.end) != yield_s {
            continue;
        }
        let v = t.acts.view(i);
        if v.pre().any(|(_, s)| s.is_empty()) || v.effs().any(|e| e.kind == K_BAD) {
            continue;
        }
        let g = *gid.entry((m.label, m.kind, m.end, m.cont, m.cost)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(i);
    }
    // the plan: for each action, the sub-group it leads (folded at its place) or joins (left out)
    let mut lead: FxMap<usize, Vec<usize>> = FxMap::default();
    let mut joined = vec![false; n];
    for g in &groups {
        if g.len() < 2 {
            continue;
        }
        c.groups += 1;
        c.grouped += g.len() as u64;
        let bx: Vec<Vec<(u32, Vec<u32>)>> = g.iter().map(|&i| boxes(&t, i)).collect();
        let mut subs: Vec<Vec<usize>> = Vec::new(); // positions in g
        for p in 0..g.len() {
            match subs.iter_mut().find(|sg| sg.iter().all(|&q| disjoint(&bx[p], &bx[q]))) {
                Some(sg) => sg.push(p),
                None => subs.push(vec![p]),
            }
        }
        for sg in subs {
            if sg.len() < 2 {
                continue;
            }
            c.subgroups += 1;
            let ix: Vec<usize> = sg.iter().map(|&p| g[p]).collect();
            let sb: Vec<Vec<(u32, Vec<u32>)>> = sg.iter().map(|&p| bx[p].clone()).collect();
            if fold_one(&t, &ix, &sb, &mut c, false).is_some() {
                c.folded_actions += 1;
                c.folded_paths += ix.len() as u64;
                for &i in &ix[1..] {
                    joined[i] = true;
                }
                lead.insert(ix[0], ix);
            }
        }
    }
    let mut acts = Actions::default();
    let mut map = Vec::new();
    for i in 0..n {
        if joined[i] {
            continue;
        }
        if let Some(ix) = lead.remove(&i) {
            let sb: Vec<Vec<(u32, Vec<u32>)>> = ix.iter().map(|&j| boxes(&t, j)).collect();
            let mut scratch = FoldCounts::default();
            let a = fold_one(&t, &ix, &sb, &mut scratch, true).expect("decided above");
            c.dead_effects += scratch.dead_effects;
            let members: Vec<u32> = ix.iter().flat_map(|&j| t.acts.members(j).iter().copied()).collect();
            map.push((acts.len(), ix));
            acts.push(t.acts.meta[i], &members, &a);
        } else {
            acts.copy_from(&t.acts, i);
        }
    }
    (Task { acts, ..t }, c, map)
}

#[derive(Default, Debug, Clone)]
pub struct CheckCounts {
    pub folds: u64,
    pub paths: u64,
    pub states: u64,
    pub folded_does_not_apply: u64,
    pub another_path_applies: u64,
    pub writes_differ: u64,
    pub conflicting_writes: u64,
    pub union_box_states: u64,
    pub union_box_not_one_path: u64,
}

impl CheckCounts {
    pub fn mismatches(&self) -> u64 {
        self.folded_does_not_apply + self.another_path_applies + self.writes_differ + self.conflicting_writes + self.union_box_not_one_path
    }
}

fn applies(a: &OAct, s: &FxMap<u32, u32>) -> bool {
    a.pre.iter().all(|p| contains(&p.vals, s[&p.fact]))
}

/// The writes of `a` in state `s`: fact -> the values it can take (a guess: any of its values), writes of the value
/// the fact already has left out; Err when two effects that fire give different results.
fn writes(a: &OAct, s: &FxMap<u32, u32>) -> Result<Vec<(u32, Vec<u32>)>, ()> {
    let mut out: Vec<(u32, Vec<u32>)> = Vec::new();
    for e in &a.eff {
        if !e.when.iter().all(|w| contains(&w.vals, s[&w.fact])) {
            continue;
        }
        match out.iter().find(|(f, _)| *f == e.fact) {
            Some((_, v)) if *v != e.vals => return Err(()),
            Some(_) => {}
            None => out.push((e.fact, e.vals.clone())),
        }
    }
    out.retain(|(f, v)| !(v.len() == 1 && v[0] == s[f]));
    out.sort();
    Ok(out)
}

/// The fold check (module docstring): `orig` the task before the fold, `folded` after, `map` from `fold`.
pub fn check(orig: &Task, folded: &Task, map: &[(usize, Vec<usize>)], per_path: usize, seed: u64) -> CheckCounts {
    let mut c = CheckCounts::default();
    let mut rng = Rng(seed);
    for (fi, ix) in map {
        c.folds += 1;
        let fa = folded.acts.get(*fi);
        let ms: Vec<OAct> = ix.iter().map(|&i| orig.acts.get(i)).collect();
        let mut involved: Vec<u32> = Vec::new();
        for a in ms.iter().chain(std::iter::once(&fa)) {
            involved.extend(a.pre.iter().map(|p| p.fact));
            for e in &a.eff {
                involved.push(e.fact);
                involved.extend(e.when.iter().map(|w| w.fact));
            }
        }
        involved.sort_unstable();
        involved.dedup();
        let random_state = |rng: &mut Rng| -> FxMap<u32, u32> {
            involved.iter().map(|&f| (f, rng.below(orig.facts.size(f)))).collect()
        };
        for (m, a) in ms.iter().enumerate() {
            c.paths += 1;
            for k in 0..per_path.max(2) {
                let mut s = random_state(&mut rng);
                for p in &a.pre {
                    let v = match k {
                        0 => p.vals[0],
                        1 => *p.vals.last().unwrap(),
                        _ => p.vals[rng.below(p.vals.len() as u32) as usize],
                    };
                    s.insert(p.fact, v);
                }
                c.states += 1;
                if !applies(&fa, &s) {
                    c.folded_does_not_apply += 1;
                } else if ms.iter().enumerate().any(|(j, b)| j != m && applies(b, &s)) {
                    c.another_path_applies += 1;
                } else {
                    match (writes(&fa, &s), writes(a, &s)) {
                        (Ok(x), Ok(y)) => {
                            if x != y {
                                c.writes_differ += 1;
                            }
                        }
                        _ => c.conflicting_writes += 1,
                    }
                }
            }
        }
        for _ in 0..per_path.max(2) * 2 {
            let mut s = random_state(&mut rng);
            for p in &fa.pre {
                s.insert(p.fact, p.vals[rng.below(p.vals.len() as u32) as usize]);
            }
            c.union_box_states += 1;
            if ms.iter().filter(|b| applies(b, &s)).count() != 1 {
                c.union_box_not_one_path += 1;
            }
        }
    }
    c
}
