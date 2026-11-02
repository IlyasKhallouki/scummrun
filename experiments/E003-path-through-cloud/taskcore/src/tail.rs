//! The old tail first at a replan (E011, plan.md "plan", item 6): the previous plan's remaining steps, checked as a plan
//! of the new task from the new start; the planner plans only when they fail.
//!
//! **The check** is taskcore `validate`'s rule on the input task under the request's bans (the task the harness plays:
//! the reading's actions, cut by the bans as replay.py `ban_pieces` cuts them), in plan_fd's terms (predict.rs), from
//! `start_state(start)`: each step is a path whose precondition holds for certain (`holds` with every fact held: a set
//! read on an unknown value fails): the piece of its own input action (`label#rec`, less "~ban:...") that holds when
//! there is one (the pieces of one action are disjoint), else one of the other paths of its label (the name less
//! "fold:", "+N", "~ban:..." and "#rec": a refinement renumbers records, a fold or a ban renames a path, and the
//! planner can take another record of the same label), each tried in the reading's order until the rest of the tail
//! checks (a depth-first walk, at most 10,000 step visits);
//! its effects apply with no guessed value (a write of several values, or under a condition on an unknown value,
//! makes the fact unknown); after the last step every goal set holds for certain. Otherwise the tail is refused, with
//! the first reason.
//!
//! **Which steps.** The steps the caller gives as remaining can start with steps the engine already ran on its own
//! (a replan after a window that ran ahead), or steps a shorter suffix does not need: `check_suffixes` takes the
//! shortest suffix that checks.
//!
//! **Sound.** Each accepted step's path applies in the state the steps before it lead to, whatever values the guessed
//! writes take (no set reads an unknown value), and the goal holds in the end state whatever those values are. So the
//! tail is a plan of the input task from the start, a sequence of its paths, as a searched plan is (where paths of one
//! label overlap, they differ only on replay checks, which the harness reads when it plays a step, as for any plan).
//! It need not be the plan the search would return; where the search returned the old tail (82 of 97 recorded
//! replans, by names with the records' numbers left out), it is checked to be the same steps.

use crate::predict::{self, State, UNKNOWN};
use fmi_core::{Action, Domains, Facts, Key, Val, NONE};

/// The label a plan step's name stands for (module docstring).
pub fn label_of_name(name: &str) -> &str {
    let mut s = name;
    if let Some(r) = s.strip_prefix("fold:") {
        s = r;
        if let Some(i) = s.rfind('+') {
            if s[i + 1..].bytes().all(|c| c.is_ascii_digit()) && i + 1 < s.len() {
                s = &s[..i];
            }
        }
    }
    if let Some(i) = s.find("~ban:") {
        s = &s[..i];
    }
    if let Some(i) = s.rfind('#') {
        if i + 1 < s.len() && s[i + 1..].bytes().all(|c| c.is_ascii_digit()) {
            s = &s[..i];
        }
    }
    s
}

/// An accepted tail: how many of the given steps it skips, each step's path, and the state after the last.
pub struct Accepted {
    pub skipped: usize,
    pub paths: Vec<Action>,
    pub end: State,
}

/// The shortest suffix of `names` that is a plan from `start` (`check` on `names[k..]` for k = n, n - 1, ..., 0): the
/// steps the caller counts as remaining may include some the engine has already run on its own, or steps a shorter
/// suffix does not need. Every suffix accepted is a plan (`check`); the shortest is the cheapest (every action costs at
/// least 1).
pub fn check_suffixes(
    facts: &Domains,
    names: &[String],
    start: &Facts,
    goal: &[(Key, Vec<Val>)],
    cands: &mut dyn FnMut(&str) -> Result<Vec<Action>, String>,
) -> Result<Accepted, String> {
    let mut first = None;
    for k in (0..=names.len()).rev() {
        match check(facts, &names[k..], start, goal, cands) {
            Ok(mut a) => {
                a.skipped = k;
                return Ok(a);
            }
            Err(e) => {
                if std::env::var_os("TASKCORE_TAIL_DEBUG").is_some() {
                    eprintln!("tail: from step {}: {}", k + 1, e);
                }
                if first.is_none() {
                    first = Some(e);
                }
            }
        }
    }
    Err(first.unwrap_or_else(|| "no tail".into()))
}

/// Check `names` from `start` to `goal` (module docstring). `cands(label)`: every path of every input action of that
/// label, under the request's bans.
pub fn check(
    facts: &Domains,
    names: &[String],
    start: &Facts,
    goal: &[(Key, Vec<Val>)],
    cands: &mut dyn FnMut(&str) -> Result<Vec<Action>, String>,
) -> Result<Accepted, String> {
    // each step's candidates, in order: its own input action's pieces, then the other paths of its label
    let mut steps: Vec<(Vec<Action>, Vec<Action>)> = Vec::with_capacity(names.len());
    for n in names {
        let all = cands(label_of_name(n))?;
        let base = n.strip_prefix("fold:").map_or(n.as_str(), |_| "");
        let base = base.split("~ban:").next().unwrap_or("");
        let (own, other): (Vec<Action>, Vec<Action>) =
            all.into_iter().partition(|p| !base.is_empty() && p.name.split("~ban:").next() == Some(base));
        steps.push((own, other));
    }
    let state = predict::start_state(facts, start);
    let mut paths = Vec::with_capacity(names.len());
    let mut budget = 10_000u32;
    let mut why = String::new();
    match walk(facts, names, goal, &steps, 0, &state, &mut paths, &mut budget, &mut why)? {
        Some(end) => Ok(Accepted { skipped: 0, paths, end }),
        None => Err(why),
    }
}

/// The steps from `i` on, from `state` (module docstring): a step's own input action when one of its pieces holds,
/// else each of its label's other paths that holds, in the reading's order, the first that leads to the goal; at most
/// `budget` step visits in all.
#[allow(clippy::too_many_arguments)]
fn walk(
    facts: &Domains,
    names: &[String],
    goal: &[(Key, Vec<Val>)],
    steps: &[(Vec<Action>, Vec<Action>)],
    i: usize,
    state: &State,
    paths: &mut Vec<Action>,
    budget: &mut u32,
    why: &mut String,
) -> Result<Option<State>, String> {
    if i == steps.len() {
        for (k, vs) in goal {
            let d = facts.get(*k).ok_or(format!("the goal's fact {} is not the task's", k.text()))?;
            let v = state.get(k).copied().unwrap_or(NONE);
            let labs: Vec<Val> = vs.iter().map(|&x| predict::label(d, x)).collect();
            if v == UNKNOWN || !labs.contains(&v) {
                if why.is_empty() {
                    *why = format!("the goal {} does not hold after the tail", k.text());
                }
                return Ok(None);
            }
        }
        return Ok(Some(state.clone()));
    }
    if *budget == 0 {
        return Ok(None);
    }
    *budget -= 1;
    let (own, other) = &steps[i];
    let holds = |p: &Action| predict::holds(facts, &p.pre, state, None) == Some(true);
    let own_hold: Vec<&Action> = own.iter().filter(|p| holds(p)).collect();
    // the pieces of one input action are disjoint: at most one holds
    let tries: Vec<&Action> = if own_hold.len() == 1 { own_hold } else { other.iter().filter(|p| holds(p)).collect() };
    if tries.is_empty() && why.is_empty() {
        *why = format!("step {} ({}): none of its label's {} paths holds", i + 1, names[i], own.len() + other.len());
        if std::env::var_os("TASKCORE_TAIL_DEBUG").is_some() {
            for p in own.iter().chain(other).take(3) {
                let miss: Vec<String> = p
                    .pre
                    .iter()
                    .filter(|(k, vs)| predict::meets(facts, *k, vs, state) != Some(true))
                    .map(|(k, vs)| format!("{}={} not in {:?}", k.text(), state.get(k).copied().unwrap_or(NONE), vs))
                    .collect();
                eprintln!("  {}: {:?}", p.name, miss);
            }
        }
    }
    for p in tries {
        let next = predict::apply_step(facts, p, &[], state).map_err(|e| format!("step {}: {:?}", i + 1, e))?;
        paths.push(p.clone());
        if let Some(end) = walk(facts, names, goal, steps, i + 1, &next, paths, budget, why)? {
            return Ok(Some(end));
        }
        paths.pop();
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(label_of_name("event(script:2@736#da2874b63864)#510"), "event(script:2@736#da2874b63864)");
        assert_eq!(label_of_name("event(script:2@736#da2874b63864)"), "event(script:2@736#da2874b63864)");
        assert_eq!(label_of_name("sentence(10,489)#56~ban:room"), "sentence(10,489)");
        assert_eq!(label_of_name("fold:esc(local:202@801#1ae62637c73)#1359+2"), "esc(local:202@801#1ae62637c73)");
        assert_eq!(label_of_name("walk_to(426)"), "walk_to(426)");
    }
}
