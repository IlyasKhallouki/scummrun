//! A plan read back to the reduced task's actions (E011, plan.md "plan", item 4): plan_fd.py `decode_ops`'s steps,
//! from the plan's operators only, with no operator table written or read.
//!
//! `decode_ops` reads the table `sas --ops` writes (sas.rs `write_ops`) and, per plan operator: leaves a goal operator
//! out; takes a pick operator as the value of its pick variable; and for an operator of an action gives the action,
//! its name and members, the variant's precondition values (`chosen`), and its guessed values: the variant's guesses
//! with their live conditions, then each guess the action writes through a pick variable (`picked`), with the pick
//! variable's value (its first value until a pick operator sets it) when that is one of the guess's values, else the
//! guess's first value. A pick variable keeps its value from step to step.
//!
//! Here the operator's action and variant number come from `search::from_task`'s table (`OpOf`), and the action is
//! expanded again (sas.rs `Expander::expand`, the writer's own expansion) to that variant: the same rows `write_ops`
//! writes for it, for the plan's few actions only.

use crate::model::*;
use crate::sas::{Expander, Variant};
use crate::search::OpOf;
use std::collections::HashMap;

/// A guessed value with its conditions: (fact, value, [(fact, values)]).
pub type DGuess = (u32, u32, Vec<(u32, Vec<u32>)>);

/// One step: `decode_ops`'s dict, with facts and values as the reduced task's ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DStep {
    pub op: u32,
    pub action: usize,
    pub name: String,
    pub members: Vec<u32>,
    pub chosen: Vec<(u32, u32)>,
    /// (fact, value, its conditions (fact, values))
    pub guessed: Vec<DGuess>,
}

/// The variant `k` of action `ai`: (chosen, guessed with conditions); and the action's picked guesses.
fn variant(x: &mut Expander, ai: usize, k: u32) -> Result<(Vec<(u32, u32)>, Vec<DGuess>, crate::sas::Picked), String> {
    let mut n = 0u32;
    let mut got = None;
    let picked = x.expand(ai, &mut |v: &Variant| {
        if n == k {
            let guessed = v
                .guessed
                .iter()
                .zip(v.guess_conds)
                .map(|(&(f, u), &(ls, le))| {
                    let c = v.conds[ls..le].iter().map(|&(cf, off, len)| (cf, v.arena[off..off + len].to_vec())).collect();
                    (f, u, c)
                })
                .collect();
            got = Some((v.chosen.to_vec(), guessed));
        }
        n += 1;
        Ok(())
    })?;
    let (c, g) = got.ok_or(format!("action {} has no variant {}", ai, k))?;
    Ok((c, g, picked))
}

/// The plan's steps (module docstring). `x` is an expander of the task `of` was built from, set as it was.
pub fn decode(x: &mut Expander, of: &[OpOf], plan: &[u32]) -> Result<Vec<DStep>, String> {
    let t = x.t;
    let mut picks: HashMap<u32, u32> = HashMap::new();
    let mut steps = Vec::new();
    for &o in plan {
        match *of.get(o as usize).ok_or(format!("no operator a{}", o))? {
            OpOf::Goal => continue,
            OpOf::Pick(f, u) => {
                picks.insert(f, u);
            }
            OpOf::Action(ai, k) => {
                let (chosen, mut guessed, picked) = variant(x, ai as usize, k)?;
                for (key, vals, when) in picked {
                    // a pick variable starts at its first value (the first pick operator's)
                    let u = *picks.get(&key).unwrap_or(&0);
                    guessed.push((key, if vals.contains(&u) { u } else { vals[0] }, when));
                }
                steps.push(DStep {
                    op: o,
                    action: ai as usize,
                    name: t.name(ai as usize),
                    members: t.acts.members(ai as usize).to_vec(),
                    chosen,
                    guessed,
                });
            }
        }
    }
    Ok(steps)
}

/// A value's text (the domain's label as the files write it).
pub fn val_text(t: &Task, f: u32, v: u32) -> &str {
    t.strs.get(t.facts.dom(f)[v as usize])
}

/// A guess's conditions as `write_ops` writes them: `[["key", ["v", ...]], ...]`.
pub fn conds_json(t: &Task, c: &[(u32, Vec<u32>)]) -> String {
    let parts: Vec<String> = c
        .iter()
        .map(|(f, vs)| {
            let v: Vec<String> = vs.iter().map(|&x| crate::cli::esc(val_text(t, *f, x))).collect();
            format!("[{}, [{}]]", crate::cli::esc(t.key(*f)), v.join(", "))
        })
        .collect();
    format!("[{}]", parts.join(", "))
}

/// The steps as JSON lines (the test against plan_fd.decode_ops): op, action, name, members, chosen [[key, value]],
/// guessed [[key, value, conditions]]; values as their texts, each condition's values sorted by text.
pub fn steps_json(t: &Task, steps: &[DStep]) -> String {
    let esc = crate::cli::esc;
    let mut out = String::new();
    for s in steps {
        let ch: Vec<String> =
            s.chosen.iter().map(|&(f, v)| format!("[{}, {}]", esc(t.key(f)), esc(val_text(t, f, v)))).collect();
        let gu: Vec<String> = s
            .guessed
            .iter()
            .map(|(f, v, c)| {
                let cs: Vec<String> = c
                    .iter()
                    .map(|(cf, vs)| {
                        let mut v: Vec<&str> = vs.iter().map(|&x| val_text(t, *cf, x)).collect();
                        v.sort_unstable();
                        let v: Vec<String> = v.into_iter().map(esc).collect();
                        format!("[{}, [{}]]", esc(t.key(*cf)), v.join(", "))
                    })
                    .collect();
                format!("[{}, {}, [{}]]", esc(t.key(*f)), esc(val_text(t, *f, *v)), cs.join(", "))
            })
            .collect();
        let mem: Vec<String> = s.members.iter().map(|m| m.to_string()).collect();
        out.push_str(&format!(
            "{{\"op\": \"a{}\", \"action\": {}, \"name\": {}, \"members\": [{}], \"chosen\": [{}], \"guessed\": [{}]}}\n",
            s.op,
            s.action,
            esc(&s.name),
            mem.join(", "),
            ch.join(", "),
            gu.join(", ")
        ));
    }
    out
}
