//! Exact reductions, each switchable and counted, run in a loop until none changes the task.
//!
//! The task's meaning (task.py): a state gives each fact one value of its domain; an action applies when every
//! precondition set holds its fact's value; its effects read the state before it (an effect fires when every
//! condition set holds); a set effect writes its value, a guessed write writes one of its values, the planner's
//! choice (a variant, or a pick variable); every action costs at least 1. A plan is a sequence of actions (with
//! the guessed values it chose) from the start to a state where every goal set holds; its cost is the sum of its
//! actions' costs. The pick and goal operators of the encoding are not actions. Every claim below is about this
//! cost, and about the end state meeting every goal set. The file's own optimal cost is this one in the model
//! encoding (the default, sas.rs); task.py's encoding differs in two ways the reductions can expose: with metric
//! 0 Fast Downward counts pick and goal operators as 1 each, and a goal set is a latch.
//!
//! "All plans survive" below means: the reduced task has the same plans as the input, as sequences of actions
//! with their guessed values (an action reduced only in what it writes to facts nothing reads is the same action).
//! "Optimal plans survive" means: every optimal plan of the input is a plan of the reduced task, so the optimal
//! cost is the same; plans that are not optimal may be lost, and the soundness note says which.
//!
//! Before the reductions, `normalize` drops precondition and condition sets that hold for every value of their
//! fact (`task._expand` leaves them out too): no state changes.

use crate::model::*;
use std::collections::BTreeMap;

pub type Counts = BTreeMap<String, u64>;

#[derive(Clone, Copy, Default, Debug)]
pub struct Enabled {
    pub a: bool,
    pub b: bool,
    pub c: bool,
    pub d: bool,
    pub e: bool,
    pub f: bool,
}

impl Enabled {
    pub fn parse(s: &str) -> Result<Enabled, String> {
        let mut en = Enabled::default();
        if s == "none" || s.is_empty() {
            return Ok(en);
        }
        if s == "all" {
            return Ok(Enabled { a: true, b: true, c: true, d: true, e: true, f: true });
        }
        for p in s.split(',') {
            match p.trim() {
                "a" => en.a = true,
                "b" => en.b = true,
                "c" => en.c = true,
                "d" => en.d = true,
                "e" => en.e = true,
                "f" => en.f = true,
                x => return Err(format!("unknown reduction {:?} (a-f, all, none)", x)),
            }
        }
        Ok(en)
    }
    pub fn any(&self) -> bool {
        self.a || self.b || self.c || self.d || self.e || self.f
    }
}

fn add(c: &mut Counts, k: &str, n: u64) {
    if n > 0 {
        *c.entry(k.to_string()).or_insert(0) += n;
    }
}

enum Edit {
    Same,
    Drop,
    New(OAct),
}

/// The task with each action kept, dropped or replaced by `f`; true when something changed.
fn rebuild(t: Task, mut f: impl FnMut(&Task, usize, View) -> Edit) -> (Task, bool) {
    let mut acts = Actions::default();
    let mut changed = false;
    for i in 0..t.acts.len() {
        match f(&t, i, t.acts.view(i)) {
            Edit::Same => acts.copy_from(&t.acts, i),
            Edit::Drop => changed = true,
            Edit::New(a) => {
                changed = true;
                acts.push(t.acts.meta[i], t.acts.members(i), &a);
            }
        }
    }
    (Task { acts, ..t }, changed)
}

fn is_full(t: &Task, f: u32, s: &[u32]) -> bool {
    s.len() as u32 == t.facts.size(f)
}

/// Sets that hold for every value, left out (module docstring).
pub fn normalize(t: Task, c: &mut Counts) -> Task {
    let mut n = 0u64;
    let (t, _) = rebuild(t, |t, _, v| {
        let full_pre = v.pre().filter(|(f, s)| is_full(t, *f, s)).count();
        let full_when: usize = v.effs().map(|e| e.when().filter(|(f, s)| is_full(t, *f, s)).count()).sum();
        if full_pre + full_when == 0 {
            return Edit::Same;
        }
        n += (full_pre + full_when) as u64;
        let mut a = v.to_owned();
        a.pre.retain(|s| !is_full(t, s.fact, &s.vals));
        for e in a.eff.iter_mut() {
            e.when.retain(|s| !is_full(t, s.fact, &s.vals));
        }
        Edit::New(a)
    });
    add(c, "normalize: sets that hold for every value, left out", n);
    t
}

/// (a) Effects that cannot change the state.
///
/// * a write of the value the action's own precondition already fixes (`pre[k] = {v}`, the effect writes `v`), or
///   that its own condition fixes (`when k = {v}`); a guessed write with one value counts as a write of that value;
/// * an effect whose condition cannot hold where the action applies: an empty condition set, or one disjoint from
///   the precondition's set on the same fact;
/// * a condition the precondition implies (`pre[c]` within the condition's set) is left out of the effect.
///
/// **Soundness: all plans survive.** Where the action applies, the dropped write leaves its fact as it was, the
/// dropped effect never fires, and the dropped condition always holds: the action changes every state exactly as
/// before. Same transitions, same costs.
pub fn red_a(t: Task, c: &mut Counts) -> (Task, bool) {
    let (mut noop, mut dead, mut implied) = (0u64, 0u64, 0u64);
    let r = rebuild(t, |_, _, v| {
        let pre: Vec<(u32, &[u32])> = v.pre().collect();
        let pget = |f: u32| pre.iter().find(|p| p.0 == f).map(|p| p.1);
        let mut out: Option<OAct> = None;
        for (j, e) in v.effs().enumerate() {
            if e.kind == K_BAD {
                continue;
            }
            let mut is_dead = false;
            let mut drop_conds = Vec::new();
            for (ci, (cf, cs)) in e.when().enumerate() {
                match pget(cf) {
                    _ if cs.is_empty() => is_dead = true,
                    Some(ps) if !intersects(ps, cs) => is_dead = true,
                    Some(ps) if is_subset(ps, cs) => drop_conds.push(ci),
                    _ => {}
                }
            }
            let single = e.vals.len() == 1;
            let is_noop = !is_dead
                && single
                && (pget(e.fact).map_or(false, |ps| ps.len() == 1 && ps[0] == e.vals[0])
                    || e.when().enumerate().any(|(ci, (cf, cs))| {
                        cf == e.fact && !drop_conds.contains(&ci) && cs.len() == 1 && cs[0] == e.vals[0]
                    }));
            if !is_dead && !is_noop && drop_conds.is_empty() {
                continue;
            }
            let a = out.get_or_insert_with(|| v.to_owned());
            // mark: effect j is replaced below (positions are those of v, the edits go to a copy)
            if is_dead {
                dead += 1;
                a.eff[j].kind = u32::MAX; // to drop
            } else if is_noop {
                noop += 1;
                a.eff[j].kind = u32::MAX;
            } else {
                implied += drop_conds.len() as u64;
                let w = std::mem::take(&mut a.eff[j].when);
                a.eff[j].when = w.into_iter().enumerate().filter(|(ci, _)| !drop_conds.contains(ci)).map(|(_, s)| s).collect();
            }
        }
        match out {
            None => Edit::Same,
            Some(mut a) => {
                a.eff.retain(|e| e.kind != u32::MAX);
                Edit::New(a)
            }
        }
    });
    add(c, "(a) writes of a value the action already fixes, left out", noop);
    add(c, "(a) effects whose condition cannot hold where the action applies, left out", dead);
    add(c, "(a) conditions the precondition implies, left out", implied);
    r
}

/// (b) Projection: a fact that no precondition, no effect condition and no goal reads is dropped, with every
/// effect on it. Repeated until every fact left is read (dropping effects drops their conditions).
///
/// **Soundness: all plans survive.** Whether an action applies, which of its effects fire and whether the goal holds
/// depend only on facts that are read; the dropped facts' values never matter. Each plan of the input is a plan of
/// the projection and back (same actions, same guesses on the facts kept). An action left with no effect is a null
/// action, for (c).
pub fn red_b(t: Task, c: &mut Counts) -> (Task, bool) {
    let nf = t.facts.len();
    let mut t = t;
    let mut changed = false;
    loop {
        let mut read = vec![false; nf];
        for (f, _) in &t.goal {
            read[*f as usize] = true;
        }
        for i in 0..t.acts.len() {
            let v = t.acts.view(i);
            for (f, _) in v.pre() {
                read[f as usize] = true;
            }
            for e in v.effs() {
                for (f, _) in e.when() {
                    read[f as usize] = true;
                }
            }
        }
        let mut dropped = 0u64;
        let (nt, ch) = rebuild(t, |_, _, v| {
            let n = v.effs().filter(|e| !read[e.fact as usize] && e.kind != K_BAD).count();
            if n == 0 {
                return Edit::Same;
            }
            dropped += n as u64;
            let mut a = v.to_owned();
            a.eff.retain(|e| read[e.fact as usize] || e.kind == K_BAD);
            Edit::New(a)
        });
        t = nt;
        add(c, "(b) effects on facts nothing reads, left out", dropped);
        if !ch {
            break;
        }
        changed = true;
    }
    (t, changed)
}

/// (c) Null actions: an action with no effect left changes no state.
///
/// **Soundness: optimal plans survive.** A plan that uses a null action is still a plan without it, and costs less
/// (every action costs at least 1), so no optimal plan uses one. Plans that use one are lost; each has a cheaper
/// plan that survives.
pub fn red_c(t: Task, c: &mut Counts) -> (Task, bool) {
    let mut n = 0u64;
    let r = rebuild(t, |_, _, v| {
        if v.neff() == 0 {
            n += 1;
            Edit::Drop
        } else {
            Edit::Same
        }
    });
    add(c, "(c) null actions (no effect), left out", n);
    r
}

/// (d) Forward reachability from the start, deletes ignored: the values each fact can ever take (the start's, then
/// every value an effect can write once its action can apply and its conditions can hold; a guessed write reaches
/// every value it lists). An action whose precondition cannot hold (some set holds no reached value, an empty set
/// among them) is dropped; an effect whose conditions cannot hold is dropped; values never reached leave the
/// domains, the precondition and condition sets, and the goal (a goal no reached value meets is kept whole: the
/// task is then unsolvable, and the planner says so).
///
/// **Soundness: all plans survive.** Ignoring deletes only adds states, so every state a plan can reach gives each
/// fact a reached value, and every action a plan applies can apply in the relaxation. The reachable states, the
/// actions that apply in them and what they do are unchanged. The result depends on the start: a replan from a
/// state outside the reached values needs the task reduced again.
///
/// This needs a start value in the domain of every fact the task reads or writes. A start with a value outside
/// a fact's domain (NONE) reaches nothing on that fact: an action that needs it would be dropped (the task looks
/// unsolvable), or, if some effect writes the fact, its domain would shrink to the written values and a
/// precondition that holds for all of them would be left out, although it does not hold at the start. task.py
/// raises OutOfDomain on such a start (build the task again with that value); taskcore stops the same way before
/// the fold and the reductions (sas::start_misses), so (d) never sees such a fact in use.
pub fn red_d(t: Task, start: usize, c: &mut Counts) -> (Task, bool) {
    let nf = t.facts.len();
    let base: Vec<usize> = (0..nf as u32).map(|f| t.facts.base(f)).collect();
    let st = t.starts[start].1.clone();
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
    loop {
        rounds += 1;
        let mut new_value = false;
        for i in 0..n {
            let v = t.acts.view(i);
            if !applied[i] {
                if v.pre().all(|(f, s)| s.iter().any(|&x| reached[base[f as usize] + x as usize])) {
                    applied[i] = true;
                } else {
                    continue;
                }
            }
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
                }
            }
        }
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
                // no value reached: a fact with no start value in its domain (NONE) that nothing writes. The start
                // check (sas::start_misses, before the fold) has stopped the run if the input's file names such a
                // fact, so no variant reads or writes it; its domain is left as it is
                keep[p] = true;
            }
        }
    }
    // new domains
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
    let rm = |f: u32, s: &[u32]| -> Vec<u32> {
        s.iter().filter(|&&x| reached[base[f as usize] + x as usize]).map(|&x| remap[base[f as usize] + x as usize]).collect()
    };
    let rm_keep = |f: u32, s: &[u32]| -> Vec<u32> {
        s.iter().filter_map(|&x| {
            let r = remap[base[f as usize] + x as usize];
            if r == NONE { None } else { Some(r) }
        }).collect()
    };
    let (mut dropped_actions, mut dropped_effects, mut cut_sets) = (0u64, 0u64, 0u64);
    let mut acts = Actions::default();
    for i in 0..n {
        if !applied[i] {
            dropped_actions += 1;
            continue;
        }
        let v = t.acts.view(i);
        let mut a = OAct::default();
        for (f, s) in v.pre() {
            let ns = rm(f, s);
            if ns.len() != s.len() {
                cut_sets += 1;
            }
            a.pre.push(OSet { fact: f, vals: ns });
        }
        for (j, e) in v.effs().enumerate() {
            if !fired[eoff[i] + j] {
                dropped_effects += 1;
                continue;
            }
            let mut when = Vec::new();
            for (f, s) in e.when() {
                let ns = rm(f, s);
                if ns.len() != s.len() {
                    cut_sets += 1;
                }
                when.push(OSet { fact: f, vals: ns });
            }
            let vals = if e.kind == K_BAD { e.vals.to_vec() } else { rm_keep(e.fact, e.vals) };
            a.eff.push(OEff { fact: e.fact, kind: e.kind, vals, when });
        }
        acts.push(t.acts.meta[i], t.acts.members(i), &a);
    }
    let goal: Vec<(u32, Vec<u32>)> = t.goal.iter().map(|(f, vals)| (*f, rm_keep(*f, vals))).collect();
    let starts: Vec<(u32, Vec<u32>)> = t
        .starts
        .iter()
        .map(|(name, vals)| {
            (*name, vals.iter().enumerate().map(|(f, &x)| if x == NONE { NONE } else { remap[base[f] + x as usize] }).collect())
        })
        .collect();
    add(c, "(d) actions whose precondition cannot hold, left out", dropped_actions);
    add(c, "(d) effects whose conditions cannot hold, left out", dropped_effects);
    add(c, "(d) values never reached, left out of the domains", dropped_values);
    add(c, "(d) precondition and condition sets cut to reached values", cut_sets);
    let changed = dropped_actions + dropped_effects + dropped_values + cut_sets > 0;
    (Task { strs: t.strs, facts, goal, starts, acts, n_orig: t.n_orig }, changed)
}

/// (e) Backward relevance by (fact, value). Needed values N, from the goal back:
///
/// * the goal's values are needed;
/// * an action is kept when one of its effects can write a needed value (a guessed write: any of its values);
/// * a kept action's precondition values are needed;
/// * for a kept action's effect on fact k with values W, and each of its conditions (c, C) on a fact c that is not
///   in the action's precondition: (i) if W meets N(k), C is needed (where the effect wrote a needed value, it must
///   fire again); (ii) if the effect could fire and destroy a needed value (k has a needed value outside W; for a
///   guess of more than one value, k has any needed value), the values outside C are needed too (where it did not
///   fire, it must not fire). A condition on a precondition fact needs nothing more: that fact's value is needed
///   whole by the precondition.
///
/// Then the actions not kept are dropped, and in kept actions the effects on facts with no needed value.
///
/// **Soundness: optimal plans survive.** Take any plan and keep only its kept actions, with the same guessed
/// values. Invariant, after each step: every fact whose value in the plan is a needed value has the same value in
/// the kept plan. It holds at the start. A dropped action writes only values that are not needed, so the facts it
/// changes have no needed value afterwards. A kept action applies in both (its precondition values are needed, so
/// they are the same in both); each of its effects on a fact k with a needed value fires in the kept plan where it
/// fired in the plan if it wrote a needed value (rule i: its conditions read the same values there), and does not
/// fire where it did not if that could destroy a needed value (rule ii); so k ends with the same value wherever the
/// plan leaves k at a needed value. At the end the goal's values are needed, so the kept plan reaches the goal. It
/// fires a guess only where the plan did (where it matters), with the same value. So every plan has a sub-plan
/// that survives, costing no more; an optimal plan has no action to drop (dropping one would cost less), so it
/// survives whole. Plans with dropped actions are lost. task.py's relevance.py is the same rule with every
/// condition fact needed whole; this one is finer and keeps no more.
#[allow(unused_assignments)]
pub fn red_e(t: Task, c: &mut Counts) -> (Task, bool) {
    let nf = t.facts.len();
    let base: Vec<usize> = (0..nf as u32).map(|f| t.facts.base(f)).collect();
    let mut need = vec![false; t.facts.total_values()];
    let mut ncount = vec![0u32; nf];
    let n = t.acts.len();
    let mut kept = vec![false; n];
    let mut ch = false;
    macro_rules! set {
        ($f:expr, $x:expr) => {{
            let p = base[$f as usize] + $x as usize;
            if !need[p] {
                need[p] = true;
                ncount[$f as usize] += 1;
                ch = true;
            }
        }};
    }
    for (f, vals) in &t.goal {
        for &x in vals {
            set!(*f, x);
        }
    }
    let mut rounds = 0u64;
    let mut prefs: Vec<u32> = Vec::new();
    loop {
        rounds += 1;
        ch = false;
        for i in 0..n {
            let v = t.acts.view(i);
            if !kept[i] {
                let hit = v.effs().any(|e| e.kind != K_BAD && e.vals.iter().any(|&x| need[base[e.fact as usize] + x as usize]));
                if !hit {
                    continue;
                }
                kept[i] = true;
                ch = true;
                for (f, s) in v.pre() {
                    for &x in s {
                        set!(f, x);
                    }
                }
            }
            prefs.clear();
            prefs.extend(v.pre().map(|(f, _)| f));
            for e in v.effs() {
                if e.kind == K_BAD || e.nwhen() == 0 {
                    continue;
                }
                let k = e.fact as usize;
                if ncount[k] == 0 {
                    continue;
                }
                let hits = e.vals.iter().filter(|&&x| need[base[k] + x as usize]).count();
                let case_i = hits > 0;
                let case_ii = if e.kind == K_GUESS && e.vals.len() > 1 { true } else { ncount[k] as usize > hits };
                if !case_i && !case_ii {
                    continue;
                }
                for (cf, cs) in e.when() {
                    if prefs.contains(&cf) {
                        continue;
                    }
                    if case_i {
                        for &x in cs {
                            set!(cf, x);
                        }
                    }
                    if case_ii {
                        for x in 0..t.facts.size(cf) {
                            if !contains(cs, x) {
                                set!(cf, x);
                            }
                        }
                    }
                }
            }
        }
        if !ch {
            break;
        }
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
        let mut a = v.to_owned();
        a.eff.retain(|e| e.kind == K_BAD || ncount[e.fact as usize] > 0);
        Edit::New(a)
    });
    add(c, "(e) actions that write no needed value, left out", da);
    add(c, "(e) effects on facts with no needed value, left out", de);
    c.insert("(e) last pass: needed values".into(), needed);
    c.insert("(e) last pass: facts needed whole".into(), whole as u64);
    c.insert("(e) last pass: facts with a needed value".into(), ncount.iter().filter(|&&x| x > 0).count() as u64);
    add(c, "(e) relevance rounds", rounds);
    r
}

/// (f) Duplicates and dominance among actions with the same effects (the same set of effects, conditions and
/// guessed values included). Action A dominates action B when every precondition set of A contains B's set on
/// the same fact (so A applies wherever B does) and A costs no more. B is dropped. Identical actions (each
/// dominates the other) keep the first.
///
/// **Soundness: the optimal cost survives, and an optimal plan.** Where B applies, A applies and changes the
/// state the same way (same effects, read in the same state, same guess choices), at no more cost. Replacing B by A
/// in a plan gives a plan of no more cost. If A costs less, no optimal plan uses B, so every optimal plan survives.
/// If they cost the same, optimal plans that use B are lost, but each has a twin with A in its place, of the same
/// cost, that survives. Plans that use B are lost.
pub fn red_f(t: Task, c: &mut Counts, replaced: &mut Vec<(u32, u32)>) -> (Task, bool) {
    let n = t.acts.len();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let sig = |i: usize| -> Vec<u32> {
        let v = t.acts.view(i);
        let mut effs: Vec<Vec<u32>> = v
            .effs()
            .map(|e| {
                let mut w: Vec<(u32, &[u32])> = e.when().collect();
                w.sort();
                let mut x = vec![e.fact, e.kind, e.vals.len() as u32];
                x.extend_from_slice(e.vals);
                x.push(w.len() as u32);
                for (f, s) in w {
                    x.push(f);
                    x.push(s.len() as u32);
                    x.extend_from_slice(s);
                }
                x
            })
            .collect();
        effs.sort();
        effs.dedup();
        effs.concat()
    };
    let mut by_hash: FxMap<u64, Vec<usize>> = FxMap::default();
    for i in 0..n {
        by_hash.entry(hash_words(&sig(i))).or_default().push(i);
    }
    let mut drop = vec![false; n];
    let (mut dup, mut dom) = (0u64, 0u64);
    let mut groups: Vec<Vec<usize>> = by_hash.into_values().filter(|g| g.len() > 1).collect();
    groups.sort();
    for hg in groups {
        // exact groups inside the hash group
        let sigs: Vec<Vec<u32>> = hg.iter().map(|&i| sig(i)).collect();
        let mut done = vec![false; hg.len()];
        for x in 0..hg.len() {
            if done[x] {
                continue;
            }
            let g: Vec<usize> = (x..hg.len()).filter(|&y| !done[y] && sigs[y] == sigs[x]).collect();
            for &y in &g {
                done[y] = true;
            }
            if g.len() < 2 {
                continue;
            }
            let mut ix: Vec<usize> = g.iter().map(|&y| hg[y]).collect();
            let pres: FxMap<usize, Vec<(u32, Vec<u32>)>> = ix
                .iter()
                .map(|&i| {
                    let mut p: Vec<(u32, Vec<u32>)> = t.acts.view(i).pre().map(|(f, s)| (f, s.to_vec())).collect();
                    p.sort();
                    (i, p)
                })
                .collect();
            let size = |i: usize| -> u64 { pres[&i].iter().map(|(_, s)| s.len() as u64).sum() };
            ix.sort_by_key(|&i| (t.acts.meta[i].cost, pres[&i].len(), std::cmp::Reverse(size(i)), i));
            let mut keep: Vec<usize> = Vec::new();
            for &i in &ix {
                let pi = &pres[&i];
                let by = keep.iter().copied().find(|&k| {
                    t.acts.meta[k].cost <= t.acts.meta[i].cost
                        && pres[&k].iter().all(|(f, sk)| pi.iter().find(|(g, _)| g == f).map_or(false, |(_, si)| is_subset(si, sk)))
                });
                match by {
                    Some(k) => {
                        drop[i] = true;
                        if pres[&k] == *pi && t.acts.meta[k].cost == t.acts.meta[i].cost {
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
    // the replaced pairs as members (input actions), for the record: every member of the dropped action (a folded
    // action has one per path) points at the first member of the action that replaced it
    for &(i, k) in &pairs {
        let to = t.acts.members(k)[0];
        replaced.extend(t.acts.members(i).iter().map(|&m| (m, to)));
    }
    let r = rebuild(t, |_, i, _| if drop[i] { Edit::Drop } else { Edit::Same });
    add(c, "(f) duplicate actions (same precondition, effects and cost), left out", dup);
    add(c, "(f) dominated actions (same effects, a weaker precondition at no more cost elsewhere), left out", dom);
    r
}

/// The reductions in a loop until none changes the task. `start` is the start (d) reaches from.
pub fn run(t: Task, en: Enabled, start: usize, c: &mut Counts, replaced: &mut Vec<(u32, u32)>) -> Result<Task, String> {
    for i in 0..t.acts.len() {
        if t.acts.view(i).effs().any(|e| e.kind == K_BAD) {
            return Err(format!("{}: an effect writes a value its domain lacks (the writer raises; reductions refuse)", t.name(i)));
        }
    }
    let mut t = t;
    let mut rounds = 0u64;
    loop {
        rounds += 1;
        t = normalize(t, c);
        let mut changed = false;
        let mut step = |t: Task, on: bool, f: &mut dyn FnMut(Task) -> (Task, bool)| -> Task {
            if !on {
                return t;
            }
            let (nt, ch) = f(t);
            changed |= ch;
            nt
        };
        t = step(t, en.a, &mut |t| red_a(t, c));
        t = step(t, en.b, &mut |t| red_b(t, c));
        t = step(t, en.c, &mut |t| red_c(t, c));
        t = step(t, en.d, &mut |t| red_d(t, start, c));
        t = step(t, en.e, &mut |t| red_e(t, c));
        t = step(t, en.f, &mut |t| red_f(t, c, replaced));
        if !changed {
            break;
        }
    }
    *c.entry("rounds of the reduction loop".into()).or_insert(0) += rounds;
    Ok(t)
}
