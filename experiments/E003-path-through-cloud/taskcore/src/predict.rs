//! A plan's paths and predicted states (E011, plan.md "plan", item 4): plan_fd.py's `predict`, `apply_step`,
//! `either`, `start_state` and the cut to the needed facts, and replay.py's `cut` and `ban_pieces`, rule for rule, on
//! fmi-core's types (`Action`, `Domains`, `Key`, `Val`).
//!
//! Python's values map to `Val` as fmi-core says (an int, `BELOW`, `ABOVE`, `NONE` for None), plus plan_fd's two
//! markers: `UNKNOWN` ("<unknown>": a predicted value the plan does not give) and `OUTSIDE` ("<outside the domain>": a
//! start value its domain lacks). A Python set of values is a `Vec<Val>` sorted ascending (the domain's order).

use fmi_core::{Action, Domain, Domains, EffVal, Effect, Facts, Key, Val, ABOVE, BELOW, NONE};
use std::collections::HashMap;

/// plan_fd.UNKNOWN
pub const UNKNOWN: Val = i64::MIN + 2;
/// plan_fd.OUTSIDE
pub const OUTSIDE: Val = i64::MIN + 3;

/// A predicted state: plan_fd's dict, key -> value (`NONE` where the dict holds None; a key it lacks is absent).
pub type State = HashMap<Key, Val>;

/// One decoded step as `predict` reads it (decode_ops's dict): the reduced action's name, its members (input
/// actions), its operator, its guessed values with their conditions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub op: String,
    pub name: String,
    pub members: Vec<u32>,
    pub chosen: Vec<(Key, Val)>,
    pub guessed: Vec<(Key, Val, Vec<(Key, Vec<Val>)>)>,
}

/// plan_fd.PathChoice and the other errors `predict` raises (a ValueError for a file that is not the task's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PredictError {
    PathChoice(String),
    Other(String),
}

fn get(state: &State, k: Key) -> Val {
    state.get(&k).copied().unwrap_or(NONE)
}

/// plan_fd `_label`: an int through the domain's `label` (None where it lacks it); a label as it is.
pub fn label(d: &Domain, v: Val) -> Val {
    if v == BELOW || v == ABOVE || v == UNKNOWN || v == OUTSIDE || v == NONE {
        v
    } else {
        d.label(v)
    }
}

/// domains.py `Domain.add`: the values `v + d` can have (`v` a label of the domain).
pub fn dom_add(dom: &Domain, v: Val, d: i64) -> Result<Vec<Val>, PredictError> {
    if v == ABOVE {
        if d >= 0 {
            return Ok(vec![ABOVE]);
        }
        let high = dom.high.ok_or_else(|| PredictError::Other("Domain.add: ABOVE with no high bound".into()))?;
        return Ok(dom.values.iter().copied().filter(|&x| x == ABOVE || (x != BELOW && x > high + d)).collect());
    }
    if v == BELOW {
        if d <= 0 {
            return Ok(vec![BELOW]);
        }
        let low = dom.low.ok_or_else(|| PredictError::Other("Domain.add: BELOW with no low bound".into()))?;
        return Ok(dom.values.iter().copied().filter(|&x| x == BELOW || (x != ABOVE && x < low + d)).collect());
    }
    let lab = dom.label(v + d);
    Ok(if lab == NONE { Vec::new() } else { vec![lab] })
}

fn whole(dom: Option<&Domain>, vs: &[Val]) -> bool {
    match dom {
        Some(d) => d.values.iter().all(|x| vs.contains(x)),
        None => false,
    }
}

/// plan_fd `_meets`: Some(true), Some(false), or None (unknown).
pub fn meets(facts: &Domains, k: Key, vs: &[Val], state: &State) -> Option<bool> {
    let v = get(state, k);
    if vs.contains(&v) || whole(facts.get(k), vs) {
        return Some(true);
    }
    if v == UNKNOWN {
        None
    } else {
        Some(false)
    }
}

/// plan_fd `_holds`: Some(true); None (may hold) where the only sets not met read an unknown value on a fact the file
/// does not hold (`held`; None: every fact counts as held); else Some(false).
pub fn holds(facts: &Domains, pre: &[(Key, Vec<Val>)], state: &State, held: Option<&std::collections::HashSet<Key>>) -> Option<bool> {
    let mut out = Some(true);
    for (k, vs) in pre {
        let m = meets(facts, *k, vs, state);
        if m.is_none() && held.is_some() && !held.unwrap().contains(k) {
            out = None;
        } else if m != Some(true) {
            return Some(false);
        }
    }
    out
}

/// plan_fd `_guess`
fn guess(key: Key, vals: &[Val], guessed: &[(Key, Val, Vec<(Key, Vec<Val>)>)], state: &State) -> Val {
    let mut got: Vec<Val> = Vec::new();
    for (k, v, w) in guessed {
        if *k == key && vals.contains(v) && w.iter().all(|(c, s)| s.contains(&get(state, *c))) && !got.contains(v) {
            got.push(*v);
        }
    }
    if got.len() == 1 {
        got[0]
    } else {
        UNKNOWN
    }
}

fn dom_of(facts: &Domains, k: Key) -> Result<&Domain, PredictError> {
    facts.get(k).ok_or_else(|| PredictError::Other(format!("KeyError: {}", k.text())))
}

/// plan_fd `apply_step`: `state` after action `a` with the plan's guessed values.
pub fn apply_step(facts: &Domains, a: &Action, guessed: &[(Key, Val, Vec<(Key, Vec<Val>)>)], state: &State) -> Result<State, PredictError> {
    let mut new = state.clone();
    for e in &a.effects {
        let met: Vec<Option<bool>> = e.when.iter().map(|(c, s)| meets(facts, *c, s, state)).collect();
        if met.contains(&Some(false)) {
            continue;
        }
        if met.contains(&None) {
            new.insert(e.key, UNKNOWN);
            continue;
        }
        let d = dom_of(facts, e.key)?;
        let vals: Vec<Val> = match &e.v {
            EffVal::Value(v) => {
                new.insert(e.key, label(d, *v));
                continue;
            }
            EffVal::Values(vs) => {
                let mut out: Vec<Val> = Vec::new();
                for &v in vs {
                    let l = label(d, v);
                    if !out.contains(&l) {
                        out.push(l);
                    }
                }
                out
            }
            EffVal::Add(step) => {
                let cur = get(state, e.key);
                if cur == OUTSIDE {
                    continue;
                }
                if cur == UNKNOWN || cur == NONE {
                    Vec::new()
                } else {
                    dom_add(d, cur, *step)?
                }
            }
        };
        let v = match vals.len() {
            1 => vals[0],
            0 => UNKNOWN,
            _ => guess(e.key, &vals, guessed, state),
        };
        new.insert(e.key, v);
    }
    Ok(new)
}

/// plan_fd `start_state`: the start (a facts.py map, absent is 0) as the task's values; OUTSIDE where a domain lacks
/// it.
pub fn start_state(facts: &Domains, start: &Facts) -> State {
    let mut out = State::with_capacity(facts.0.len());
    for (k, d) in &facts.0 {
        let lab = label(d, start.val(*k));
        out.insert(*k, if lab == NONE { OUTSIDE } else { lab });
    }
    out
}

fn union(a: &mut Vec<Val>, b: &[Val]) {
    for &x in b {
        if !a.contains(&x) {
            a.push(x);
        }
    }
    a.sort_unstable();
}

/// plan_fd `either`: a stand-in for a step that is several paths (every fact their preconditions name, with the
/// union of their sets, keys in their text order; all their effects, each once, in order; the first path's record
/// and cost).
pub fn either(name: &str, paths: &[Action]) -> Action {
    let mut keys: Vec<Key> = Vec::new();
    for p in paths {
        for (k, _) in &p.pre {
            if !keys.contains(k) {
                keys.push(*k);
            }
        }
    }
    let mut texts: Vec<(String, Key)> = keys.into_iter().map(|k| (k.text(), k)).collect();
    texts.sort();
    let pre = texts
        .into_iter()
        .map(|(_, k)| {
            let mut s: Vec<Val> = Vec::new();
            for p in paths {
                if let Some((_, vs)) = p.pre.iter().find(|(k2, _)| *k2 == k) {
                    union(&mut s, vs);
                }
            }
            (k, s)
        })
        .collect();
    let mut effects: Vec<Effect> = Vec::new();
    for p in paths {
        for e in &p.effects {
            if !effects.contains(e) {
                effects.push(e.clone());
            }
        }
    }
    Action { name: name.to_string(), pre, effects, what: paths[0].what, cost: paths[0].cost }
}

/// The action with its effects cut to the facts (e) needs (plan_taskcore's `cut`).
pub fn cut_to(a: &Action, needed: &std::collections::HashSet<Key>) -> Action {
    Action { effects: a.effects.iter().filter(|e| needed.contains(&e.key)).cloned().collect(), ..a.clone() }
}

fn py_list(names: &[&str]) -> String {
    let v: Vec<String> = names.iter().map(|n| format!("'{}'", n)).collect();
    format!("[{}]", v.join(", "))
}

/// plan_fd `predict` with `choices` (engine_choice, as the harness runs it): each step's path (or `either` of the
/// paths left to the engine), the predicted state after the last step, and the choices (step index -> its paths).
/// `pieces(m)`: input action m's paths under the bans; `label(m)`: its record's label.
pub fn predict(
    facts: &Domains,
    pieces: &mut dyn FnMut(u32) -> Vec<Action>,
    label_of: &dyn Fn(u32) -> String,
    steps: &[Step],
    start: &Facts,
    held: &std::collections::HashSet<Key>,
    engine_choice: bool,
) -> Result<(Vec<Action>, State, Vec<(u32, Vec<Action>)>), PredictError> {
    let mut state = start_state(facts, start);
    let mut out = Vec::with_capacity(steps.len());
    let mut choices = Vec::new();
    for (i, s) in steps.iter().enumerate() {
        let mut members: Vec<u32> = Vec::new();
        for &m in &s.members {
            if !members.contains(&m) {
                members.push(m);
            }
        }
        let mut cands: Vec<Action> = Vec::new();
        for &m in &members {
            cands.extend(pieces(m));
        }
        if s.members.len() == 1 {
            let lab = label_of(s.members[0]);
            if cands.iter().any(|p| s.name != p.name.split("~ban:").next().unwrap_or("") && s.name != lab) {
                let names: Vec<&str> = cands.iter().take(2).map(|p| p.name.as_str()).collect();
                return Err(PredictError::Other(format!(
                    "step {}: the packed file's action {} is {}, the task's {}: the file is not this task's",
                    i + 1,
                    s.members[0],
                    s.name,
                    py_list(&names)
                )));
            }
        }
        let h: Vec<Option<bool>> = cands.iter().map(|p| holds(facts, &p.pre, &state, Some(held))).collect();
        let sure: Vec<usize> = (0..cands.len()).filter(|&j| h[j] == Some(true)).collect();
        let maybe: Vec<usize> = (0..cands.len()).filter(|&j| h[j].is_none()).collect();
        let ok: Vec<usize> = sure.iter().chain(maybe.iter()).copied().collect();
        if ok.len() > 1 && sure.len() <= 1 && engine_choice {
            let paths: Vec<Action> = ok.iter().map(|&j| cands[j].clone()).collect();
            let afters: Vec<State> =
                paths.iter().map(|p| apply_step(facts, p, &s.guessed, &state)).collect::<Result<_, _>>()?;
            let mut next = State::with_capacity(afters[0].len());
            for (k, v) in &afters[0] {
                let agree = afters[1..].iter().all(|x| x.get(k) == Some(v));
                next.insert(*k, if agree { *v } else { UNKNOWN });
            }
            state = next;
            out.push(either(&s.name, &paths));
            choices.push((i as u32, paths));
            continue;
        }
        if ok.len() != 1 {
            let mut msg = format!(
                "step {} ({}, {}): {} of its {} paths hold in the plan's predicted state",
                i + 1,
                s.name,
                s.op,
                sure.len(),
                cands.len()
            );
            if !maybe.is_empty() {
                msg.push_str(&format!(
                    " and {} may hold (an unknown value on a fact the file does not hold)",
                    maybe.len()
                ));
            }
            if !ok.is_empty() {
                let names: Vec<&str> = ok.iter().take(4).map(|&j| cands[j].name.as_str()).collect();
                msg.push_str(&format!(": {}", py_list(&names)));
            }
            return Err(PredictError::PathChoice(msg));
        }
        let p = cands.swap_remove(ok[0]);
        state = apply_step(facts, &p, &s.guessed, &state)?;
        out.push(p);
    }
    Ok((out, state, choices))
}

/// replay.py `cut`: action `a` with the ban's point cut out of its precondition: disjoint pieces, piece i holding the
/// first i keys at the banned values and key i at any other value; `[a]` when the precondition misses the point.
pub fn ban_cut(a: &Action, point: &[(Key, Val)], facts: &Domains) -> Vec<Action> {
    let cur: Vec<Vec<Val>> = point
        .iter()
        .map(|(k, _)| match a.pre.iter().find(|(k2, _)| k2 == k) {
            Some((_, vs)) => vs.clone(),
            None => facts.get(*k).map(|d| d.values.clone()).unwrap_or_default(),
        })
        .collect();
    if point.iter().zip(&cur).any(|((_, v), s)| !s.contains(v)) {
        return vec![a.clone()];
    }
    let mut out = Vec::new();
    for (i, (k, v)) in point.iter().enumerate() {
        let rest: Vec<Val> = cur[i].iter().copied().filter(|x| x != v).collect();
        if rest.is_empty() {
            continue;
        }
        let mut pre = a.pre.clone();
        let put = |pre: &mut Vec<(Key, Vec<Val>)>, k: Key, vs: Vec<Val>| match pre.iter_mut().find(|(k2, _)| *k2 == k) {
            Some(e) => e.1 = vs,
            None => pre.push((k, vs)),
        };
        for (k2, v2) in &point[..i] {
            put(&mut pre, *k2, vec![*v2]);
        }
        put(&mut pre, *k, rest);
        out.push(Action { name: format!("{}~ban:{}", a.name, k.text()), pre, ..a.clone() });
    }
    out
}

/// replay.py `ban_pieces` for one action: its pieces under every ban of its label (`ban_cut`, ban after ban), a ban
/// applied only when the task has every key it names.
pub fn pieces_of(a: &Action, label: &str, bans: &[fmi_core::Ban], facts: &Domains) -> Vec<Action> {
    let mut pieces = vec![a.clone()];
    for ban in bans.iter().filter(|b| b.label == label) {
        if ban.facts.iter().all(|(k, _)| facts.get(*k).is_some()) {
            pieces = pieces.iter().flat_map(|q| ban_cut(q, &ban.facts, facts)).collect();
        }
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dom(vals: &[Val]) -> Domain {
        Domain { values: vals.to_vec(), low: None, high: None }
    }

    #[test]
    fn apply_and_guess() {
        let k = Key::var(1);
        let g = Key::var(2);
        let facts = Domains(vec![(k, dom(&[0, 1, 2])), (g, dom(&[0, 1]))]);
        let mut start = Facts::new();
        start.set(g, 1);
        let s = start_state(&facts, &start);
        assert_eq!(s[&k], 0);
        let a = Action {
            name: "x".into(),
            pre: vec![(g, vec![1])],
            effects: vec![Effect { key: k, v: EffVal::Values(vec![1, 2]), when: vec![] }],
            what: 0,
            cost: 1,
        };
        assert_eq!(holds(&facts, &a.pre, &s, None), Some(true));
        let n = apply_step(&facts, &a, &[(k, 2, vec![])], &s).unwrap();
        assert_eq!(n[&k], 2);
        let n = apply_step(&facts, &a, &[], &s).unwrap();
        assert_eq!(n[&k], UNKNOWN);
    }

    #[test]
    fn cut_pieces() {
        let k = Key::var(1);
        let r = Key::parse("room");
        let facts = Domains(vec![(k, dom(&[0, 1, 2])), (r, dom(&[28, 33]))]);
        let a = Action { name: "e#1".into(), pre: vec![(k, vec![0, 1])], effects: vec![], what: 0, cost: 1 };
        let p = ban_cut(&a, &[(k, 1), (r, 33)], &facts);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].pre, vec![(k, vec![0])]);
        assert_eq!(p[1].pre, vec![(k, vec![1]), (r, vec![28])]);
        assert_eq!(p[1].name, "e#1~ban:room");
    }
}
