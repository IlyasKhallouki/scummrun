//! The old tail first at a replan (E011, plan.md "plan", item 6): the previous plan's remaining steps, checked as a plan
//! of the new task from the new start; the planner plans only when they fail.
//!
//! **The check** is taskcore `validate`'s rule on the input task under the request's bans (the task the harness plays:
//! the reading's actions, cut by the bans as replay.py `ban_pieces` cuts them), in plan_fd's terms (predict.rs), from
//! `start_state(start)`: for each step, its candidates are the pieces of its own input action when the reading has an
//! action of that name (`label#rec`, less "~ban:..."), else every path of its label (the name less "fold:", "+N",
//! "~ban:..." and "#rec": a refinement renumbers records, and a fold or a ban renames a path); of those whose
//! precondition holds for certain (`holds` with every fact held: a set read on an unknown value fails) exactly one must
//! hold, and it is the step;
//! its effects apply with no guessed value (a write of several values, or under a condition on an unknown value,
//! makes the fact unknown); after the last step every goal set holds for certain. Otherwise the tail is refused, with
//! the first reason.
//!
//! **Which steps.** The steps the caller gives as remaining can start with steps the engine already ran on its own
//! (a replan after a window that ran ahead), or steps a shorter suffix does not need: `check_suffixes` takes the
//! shortest suffix that checks.
//!
//! **Sound.** Each accepted step's path applies in the state the steps before it lead to, whatever values the guessed
//! writes take (no set reads an unknown value), and in that state no other path of its label applies; the goal holds
//! in the end state whatever those values are. So the tail is a plan of the input task from the start: the plan the
//! harness plays is a sequence of these paths. It need not be the plan the search would return; where the search
//! would return it (82 of 97 recorded replans returned the old tail), the paths are the same.

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
    let mut state = predict::start_state(facts, start);
    let mut paths = Vec::with_capacity(names.len());
    for (i, n) in names.iter().enumerate() {
        let label = label_of_name(n);
        let all = cands(label)?;
        // the step's own input action when this reading has it (the same record: its pieces are disjoint); else the
        // label's paths (a refinement renumbered the records), of which exactly one must hold
        let base = n.strip_prefix("fold:").map_or(n.as_str(), |_| "");
        let base = base.split("~ban:").next().unwrap_or("");
        let own: Vec<Action> =
            all.iter().filter(|p| !base.is_empty() && p.name.split("~ban:").next() == Some(base)).cloned().collect();
        let cs = if own.is_empty() { all } else { own };
        let hold: Vec<usize> =
            (0..cs.len()).filter(|&j| predict::holds(facts, &cs[j].pre, &state, None) == Some(true)).collect();
        if hold.len() != 1 {
            if std::env::var_os("TASKCORE_TAIL_DEBUG").is_some() && i == 0 {
                for p in cs.iter().take(3) {
                    let miss: Vec<String> = p
                        .pre
                        .iter()
                        .filter(|(k, vs)| predict::meets(facts, *k, vs, &state) != Some(true))
                        .map(|(k, vs)| format!("{}={} not in {:?}", k.text(), state.get(k).copied().unwrap_or(NONE), vs))
                        .collect();
                    eprintln!("  {}: {:?}", p.name, miss);
                }
            }
            return Err(format!("step {} ({}): {} of the {} paths of its label hold", i + 1, n, hold.len(), cs.len()));
        }
        let p = cs[hold[0]].clone();
        state = predict::apply_step(facts, &p, &[], &state).map_err(|e| format!("step {}: {:?}", i + 1, e))?;
        paths.push(p);
    }
    for (k, vs) in goal {
        let d = facts.get(*k).ok_or(format!("the goal's fact {} is not the task's", k.text()))?;
        let v = state.get(k).copied().unwrap_or(NONE);
        let labs: Vec<Val> = vs.iter().map(|&x| predict::label(d, x)).collect();
        if v == UNKNOWN || !labs.contains(&v) {
            return Err(format!("the goal {} does not hold after the tail", k.text()));
        }
    }
    Ok(Accepted { skipped: 0, paths, end: state })
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
