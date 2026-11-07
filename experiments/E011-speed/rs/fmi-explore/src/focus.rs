//! E022 amendment (7) (`FMI_PLAN_FOCUS=1`): a plan on the actions the goal needs first, then on the whole reading.
//!
//! A reading holds every slot of its scope: in room 49 an ambient local has 11,272 events and a global 4,800, the
//! captain's conversation 898. LAMA, FD and the Rust GBFS all ran out of 900 s on the conversation's replan, where a
//! search on the conversation's own actions finds a 20-step plan in 0.1 s: the relaxed plan reaches the dialog's
//! variable through an event of any args, so the heuristic sees the goal near and the search wanders through the
//! other slots' events. This is partial grounding (Gnad et al., "Learning How to Ground a Plan", AAAI 2019): plan on a
//! subset of the actions first, on the whole task when the subset fails.
//!
//! **The subset.** Actions fall into families: an event's slot (`event(local:208@...` is `event:local:208`), any
//! other action its label with the parenthesized arguments dropped (`dialog_line(121)[17@49]` is
//! `dialog_line()[17@49]`). The families start as those with an action that writes a goal value. While the goal is not
//! reached in the relaxed sense (every effect adds, from the start's values, by the families' actions only), the
//! smallest family (fewest actions) that writes a value some family action's precondition reads and nothing reached
//! holds is added. The subset is the families' actions when the goal is reached and some family is left out; the
//! harness bans every other label (a ban with no facts cuts its whole action, bans.rs).
//!
//! **Sound.** A plan on the subset is a plan on the reading: its steps are the reading's actions, in a task with fewer.

use fmi_core::{Action, Ban, Domains, EffVal, Facts, Key, ReadingSource, Val, ABOVE, BELOW, NONE};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub fn on() -> bool {
    std::env::var("FMI_PLAN_FOCUS").is_ok_and(|v| v == "1")
}

/// The subset's time limit (`FMI_PLAN_FOCUS_S`, default 30 s); the whole reading's plan follows when it fails.
pub fn limit_s() -> u64 {
    std::env::var("FMI_PLAN_FOCUS_S").ok().and_then(|v| v.parse().ok()).unwrap_or(30)
}

/// An action name's label: the name less its "#rec" (tail.rs `label_of_name`'s last rule).
pub fn label_of(name: &str) -> &str {
    match name.rfind('#') {
        Some(i) if i + 1 < name.len() && name[i + 1..].bytes().all(|c| c.is_ascii_digit()) => &name[..i],
        _ => name,
    }
}

/// A label's family (the module docstring).
pub fn family(label: &str) -> String {
    if let Some(r) = label.strip_prefix("event(") {
        if let Some(i) = r.find('@') {
            return format!("event:{}", &r[..i]);
        }
    }
    let mut out = String::new();
    let mut depth = 0usize;
    for c in label.chars() {
        match c {
            '(' => {
                if depth == 0 {
                    out.push('(');
                }
                depth += 1;
            }
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    out.push(')');
                }
            }
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

struct Act {
    pre: Vec<(Key, Vec<Val>)>,
    /// each effect's values (an add: every value of its domain) and its conditions
    eff: Vec<(Key, Vec<Val>, Vec<(Key, Vec<Val>)>)>,
}

/// A reading's actions by family.
pub struct Index {
    acts: Vec<Act>,
    labels: Vec<String>,
    fams: Vec<String>,
    fam_of: Vec<u32>,
    by_fam: Vec<Vec<u32>>,
    writers: HashMap<(Key, Val), Vec<u32>>,
}

impl Index {
    pub fn of(actions: Vec<Action>, doms: &Domains) -> Index {
        let mut ix = Index { acts: vec![], labels: vec![], fams: vec![], fam_of: vec![], by_fam: vec![], writers: HashMap::new() };
        let mut fam_id: HashMap<String, u32> = HashMap::new();
        for a in actions {
            let label = label_of(&a.name).to_string();
            let f = family(&label);
            let n = fam_id.len() as u32;
            let fi = *fam_id.entry(f.clone()).or_insert(n);
            if fi == n {
                ix.fams.push(f);
                ix.by_fam.push(vec![]);
            }
            let ai = ix.acts.len() as u32;
            let eff: Vec<_> = a
                .effects
                .iter()
                .map(|e| {
                    let vs = match &e.v {
                        EffVal::Value(v) => vec![*v],
                        EffVal::Values(vs) => vs.clone(),
                        EffVal::Add(_) => doms.get(e.key).map(|d| d.values.clone()).unwrap_or_default(),
                    };
                    (e.key, vs, e.when.clone())
                })
                .collect();
            for (k, vs, _) in &eff {
                for v in vs {
                    let w = ix.writers.entry((*k, *v)).or_default();
                    if !w.contains(&fi) {
                        w.push(fi);
                    }
                }
            }
            ix.acts.push(Act { pre: a.pre, eff });
            ix.labels.push(label);
            ix.fam_of.push(fi);
            ix.by_fam[fi as usize].push(ai);
        }
        ix
    }
}

fn index(rd: &dyn ReadingSource) -> Arc<Index> {
    static C: Mutex<Vec<([u8; 20], Arc<Index>)>> = Mutex::new(Vec::new());
    let id = rd.id();
    if let Some((_, ix)) = C.lock().unwrap().iter().find(|(i, _)| *i == id) {
        return ix.clone();
    }
    let ix = Arc::new(Index::of((0..rd.n_actions()).map(|i| rd.action(i)).collect(), rd.domains()));
    let mut c = C.lock().unwrap();
    if c.len() >= 4 {
        c.remove(0);
    }
    c.push((id, ix.clone()));
    ix
}

type Reached = HashMap<Key, HashSet<Val>>;

fn holds(r: &Reached, k: &Key, vs: &[Val]) -> bool {
    r.get(k).is_some_and(|s| vs.iter().any(|v| s.contains(v)))
}

/// The values the families' actions reach from `seed`, every effect adding.
fn reach(ix: &Index, fams: &BTreeSet<u32>, seed: &Reached) -> Reached {
    let mut r = seed.clone();
    let acts: Vec<&Act> = fams.iter().flat_map(|&f| ix.by_fam[f as usize].iter().map(|&a| &ix.acts[a as usize])).collect();
    let mut done = vec![false; acts.len()];
    loop {
        let mut changed = false;
        for (i, a) in acts.iter().enumerate() {
            if done[i] || !a.pre.iter().all(|(k, vs)| holds(&r, k, vs)) {
                continue;
            }
            let mut all = true;
            for (k, vs, when) in &a.eff {
                if !when.iter().all(|(ck, cv)| holds(&r, ck, cv)) {
                    all = false;
                    continue;
                }
                let s = r.entry(*k).or_default();
                for v in vs {
                    changed |= s.insert(*v);
                }
            }
            // an action whose every effect applied adds nothing more
            done[i] = all;
        }
        if !changed {
            return r;
        }
    }
}

/// The subset a plan tries first (the module docstring).
pub struct Focus {
    pub families: Vec<String>,
    pub actions: usize,
    pub rounds: usize,
    pub bans: Vec<Ban>,
}

/// The subset of `ix` for `goal` from `start`, None when the goal needs every family or is not reached at all. The start
/// is the planner's (start.rs `start_text_in`): every fact of `doms`, 0 where `start` lacks it, labeled by its domain.
pub fn focus_in(ix: &Index, doms: &Domains, start: &Facts, goal: &[(Key, Vec<Val>)]) -> Option<Focus> {
    let mut seed: Reached = HashMap::new();
    for (k, d) in &doms.0 {
        let v = start.get(*k).unwrap_or(0);
        let l = if v == BELOW || v == ABOVE || v == NONE { v } else { d.label(v) };
        seed.entry(*k).or_default().insert(l);
    }
    let mut fams: BTreeSet<u32> =
        goal.iter().flat_map(|(k, vs)| vs.iter().filter_map(|v| ix.writers.get(&(*k, *v)))).flatten().copied().collect();
    if fams.is_empty() {
        return None;
    }
    for round in 0..ix.fams.len() {
        let r = reach(ix, &fams, &seed);
        if goal.iter().all(|(k, vs)| holds(&r, k, vs)) {
            if fams.len() == ix.fams.len() {
                return None;
            }
            let mut out: BTreeSet<&str> = BTreeSet::new();
            for (i, l) in ix.labels.iter().enumerate() {
                if !fams.contains(&ix.fam_of[i]) {
                    out.insert(l);
                }
            }
            let actions = fams.iter().map(|&f| ix.by_fam[f as usize].len()).sum();
            return Some(Focus {
                families: fams.iter().map(|&f| ix.fams[f as usize].clone()).collect(),
                actions,
                rounds: round,
                bans: out.into_iter().map(|l| Ban { label: l.to_string(), facts: vec![] }).collect(),
            });
        }
        // the families outside that write a value a family action reads and nothing reached holds
        let mut cand: HashMap<u32, usize> = HashMap::new();
        for &f in &fams {
            for &a in &ix.by_fam[f as usize] {
                for (k, vs) in &ix.acts[a as usize].pre {
                    if holds(&r, k, vs) {
                        continue;
                    }
                    for v in vs {
                        for &w in ix.writers.get(&(*k, *v)).into_iter().flatten() {
                            if !fams.contains(&w) {
                                *cand.entry(w).or_default() += 1;
                            }
                        }
                    }
                }
            }
        }
        let best = cand.iter().min_by_key(|(&w, &n)| (ix.by_fam[w as usize].len(), usize::MAX - n, ix.fams[w as usize].clone())).map(|(&w, _)| w)?;
        fams.insert(best);
    }
    None
}

/// `focus_in` on the reading's actions (indexed once per reading).
pub fn focus(rd: &dyn ReadingSource, start: &Facts, goal: &[(Key, Vec<Val>)]) -> Option<Focus> {
    let ix = index(rd);
    let f = focus_in(&ix, rd.domains(), start, goal);
    if std::env::var_os("FMI_PLAN_FOCUS_DEBUG").is_some() {
        let g: Vec<String> = goal.iter().map(|(k, vs)| format!("{}={:?}", k.text(), vs)).collect();
        let w: Vec<usize> = goal.iter().flat_map(|(k, vs)| vs.iter().map(move |v| (*k, *v))).map(|kv| ix.writers.get(&kv).map_or(0, |w| w.len())).collect();
        eprintln!(
            "focus: goal {:?}, writers {:?}, start {} facts, {} actions, {} families, focus {:?}",
            g,
            w,
            start.len(),
            ix.acts.len(),
            ix.fams.len(),
            f.as_ref().map(|f| (&f.families, f.actions))
        );
    }
    f
}
