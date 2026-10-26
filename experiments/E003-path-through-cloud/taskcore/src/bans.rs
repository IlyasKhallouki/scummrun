//! Two inputs for the replay harness (replay.py), each read only when its option is given: a start as a file
//! (`--start-file`), and bans (`--bans`), applied to the input task before the start check, the fold and the
//! reductions, so that everything after them sees the banned task as its input.
//!
//! **The start file.** One line per fact: `key<TAB>value`, the value's text as the packed file writes values (the
//! domain's label, `str(v)`), or `key<TAB>` alone for a value the fact's domain lacks (the start check then stops on
//! it where the file names the fact, as task.py raises OutOfDomain). Every fact of the packed file needs a line;
//! keys the file lacks are ignored (export_records.py `write_start` writes one).
//!
//! **Bans.** One ban per line: `label<TAB>key<TAB>value<TAB>key<TAB>value...`, in the order the harness made them
//! (export_records.py `write_bans`). A ban means what replay.py's `Ban` means (`apply_bans` and `cut`, rule for
//! rule): for every action of the ban's label (the record's label; the packed file's label), in the file's order,
//! and for every ban of that label in order, the banned point is cut out of the action's precondition. `cut`: with
//! `cur[k]` the action's precondition set on `k` (its fact's whole domain when it has none), an action whose sets
//! miss the point (some value not in `cur[k]`) is left as it is; otherwise it becomes the disjoint pieces `i` =
//! 0, 1, ...: piece `i` holds keys `0..i` at their banned values and key `i` at any other value of `cur[i]` (none
//! when that is empty), its other precondition sets and its effects as the action's. A ban that names a key the
//! file lacks is not applied at all (replay.py: `all(k in task.facts ...)`). That is replay.py's rule only when the
//! file's facts are the task's: export_records writes build_task's facts, which are every key its actions name;
//! a task.Task can hold facts no action names (export_task writes them with `every_fact`). So a ban key the file
//! lacks but the start file names (write_start writes every fact of the caller's task) is an error. Each piece
//! keeps the action's name, label and members, so a plan reads back to the input actions.
//!
//! One difference that cannot reach the file: task.py's precondition sets may hold values their domain lacks (they
//! are cut to the domain when the file is written); here they were cut on reading. A piece whose set would hold
//! only such values is dropped here and written by task.py as an action no value meets, which it drops too.

use crate::model::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// The index of value text `v` in fact `f`'s domain, NONE when the domain lacks it.
fn value_index(t: &Task, f: u32, v: &str) -> u32 {
    match t.strs.find(v) {
        Some(s) => t.facts.dom(f).iter().position(|&x| x == s).map(|p| p as u32).unwrap_or(NONE),
        None => NONE,
    }
}

fn read_lines(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))
}

/// The start in a start file (module docstring): per fact its value index, NONE where the domain lacks it; and the
/// keys the file names.
pub fn read_start(t: &Task, path: &Path) -> Result<(Vec<u32>, HashSet<String>), String> {
    let text = read_lines(path)?;
    let mut by: HashMap<&str, &str> = HashMap::new();
    for (n, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let (k, v) = line.split_once('\t').ok_or(format!("{}:{}: no tab", path.display(), n + 1))?;
        if by.insert(k, v).is_some() {
            return Err(format!("{}:{}: {} given twice", path.display(), n + 1, k));
        }
    }
    let mut out = Vec::with_capacity(t.facts.len());
    for f in 0..t.facts.len() as u32 {
        let v = by.get(t.key(f)).ok_or(format!("{}: no value for fact {}", path.display(), t.key(f)))?;
        out.push(if v.is_empty() { NONE } else { value_index(t, f, v) });
    }
    Ok((out, by.keys().map(|k| k.to_string()).collect()))
}

pub struct Ban {
    pub label: String,
    pub point: Vec<(String, String)>,
}

/// The bans in a bans file (module docstring).
pub fn read_bans(path: &Path) -> Result<Vec<Ban>, String> {
    let text = read_lines(path)?;
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() % 2 != 1 {
            return Err(format!("{}:{}: a key without its value", path.display(), n + 1));
        }
        let point = parts[1..].chunks(2).map(|c| (c[0].to_string(), c[1].to_string())).collect();
        out.push(Ban { label: parts[0].to_string(), point });
    }
    Ok(out)
}

#[derive(Default, Debug, Clone)]
pub struct BanCounts {
    pub bans: u64,          // bans in the file
    pub not_applied: u64,   // bans that name a key the task lacks (replay.py does not apply them)
    pub actions_hit: u64,   // actions of a banned label whose precondition held the point (cut)
    pub pieces: u64,        // the pieces they became
    pub actions_after: u64, // actions after the bans
}

fn set_of<'a>(pre: &'a [OSet], f: u32) -> Option<&'a Vec<u32>> {
    pre.iter().find(|s| s.fact == f).map(|s| &s.vals)
}

fn put(pre: &mut Vec<OSet>, f: u32, vals: Vec<u32>) {
    match pre.iter_mut().find(|s| s.fact == f) {
        Some(s) => s.vals = vals,
        None => pre.push(OSet { fact: f, vals }),
    }
}

/// replay.py `cut`: the action with the point cut out of its precondition (module docstring); None when its
/// precondition misses the point (the action stays as it is).
fn cut(t: &Task, a: &OAct, point: &[(u32, u32)]) -> Option<Vec<OAct>> {
    let cur: Vec<Vec<u32>> =
        point.iter().map(|&(f, _)| set_of(&a.pre, f).cloned().unwrap_or_else(|| (0..t.facts.size(f)).collect())).collect();
    if point.iter().zip(&cur).any(|(&(_, v), s)| !contains(s, v)) {
        return None;
    }
    let mut out = Vec::new();
    for (i, &(f, v)) in point.iter().enumerate() {
        let rest: Vec<u32> = cur[i].iter().copied().filter(|&x| x != v).collect();
        if rest.is_empty() {
            continue;
        }
        let mut pre = a.pre.clone();
        for &(f2, v2) in &point[..i] {
            put(&mut pre, f2, vec![v2]);
        }
        put(&mut pre, f, rest);
        out.push(OAct { pre, eff: a.eff.clone() });
    }
    Some(out)
}

/// The task with every ban applied to every action of its label (module docstring). `named`: the start file's keys.
pub fn apply(t: Task, bans: &[Ban], named: Option<&HashSet<String>>, c: &mut BanCounts) -> Result<Task, String> {
    let b = banned(&t, bans, named, c)?;
    let mut acts = Actions::default();
    for i in 0..b.n() {
        acts.copy_src(&b, i);
    }
    drop(b);
    Ok(Task { acts, ..t })
}

/// The banned task as a view over the input's actions (`apply` without the copy, for serve.rs): the actions of a
/// banned label replaced by their pieces, in place; every other action is the input's own.
pub struct Banned<'a> {
    pub input: &'a Actions,
    pub pieces: Actions,
    /// per action of the banned task: the input's action `i` (`i`), or piece `p` (`p | PIECE`)
    pub order: Vec<u32>,
}

const PIECE: u32 = 1 << 31;

impl<'a> Banned<'a> {
    fn at(&self, i: usize) -> (&Actions, usize) {
        let o = self.order[i];
        if o & PIECE != 0 {
            (&self.pieces, (o & !PIECE) as usize)
        } else {
            (self.input, o as usize)
        }
    }
}

impl<'a> Src for Banned<'a> {
    fn n(&self) -> usize {
        self.order.len()
    }
    fn view(&self, i: usize) -> View<'_> {
        let (a, j) = self.at(i);
        a.view(j)
    }
    fn meta(&self, i: usize) -> Meta {
        let (a, j) = self.at(i);
        a.meta[j]
    }
    fn members(&self, i: usize) -> &[u32] {
        let (a, j) = self.at(i);
        a.members(j)
    }
}

/// The bans of the module docstring on `t`, as a view (`Banned`); the counts in `c`.
pub fn banned<'a>(t: &'a Task, bans: &[Ban], named: Option<&HashSet<String>>, c: &mut BanCounts) -> Result<Banned<'a>, String> {
    let key_of: HashMap<&str, u32> = (0..t.facts.len() as u32).map(|f| (t.key(f), f)).collect();
    let mut by: FxMap<u32, Vec<Vec<(u32, u32)>>> = FxMap::default();
    for ban in bans {
        c.bans += 1;
        let point: Option<Vec<(u32, u32)>> =
            ban.point.iter().map(|(k, v)| key_of.get(k.as_str()).map(|&f| (f, value_index(t, f, v)))).collect();
        let Some(point) = point else {
            if let Some((k, _)) = ban.point.iter().find(|(k, _)| !key_of.contains_key(k.as_str()) && named.map_or(false, |n| n.contains(k))) {
                return Err(format!("a ban on {} names fact {}, which the start file names and the packed file lacks: \
                                    the file's facts are not the task's", ban.label, k));
            }
            c.not_applied += 1;
            continue;
        };
        if let Some(label) = t.strs.find(&ban.label) {
            by.entry(label).or_default().push(point);
        }
    }
    if t.acts.len() as u64 >= PIECE as u64 {
        return Err(format!("{} actions: more than the banned view holds", t.acts.len()));
    }
    let mut pieces_out = Actions::default();
    let mut order = Vec::with_capacity(t.acts.len());
    for i in 0..t.acts.len() {
        let Some(points) = by.get(&t.acts.meta[i].label) else {
            order.push(i as u32);
            continue;
        };
        let mut pieces = vec![t.acts.get(i)];
        let mut hit = false;
        for point in points {
            let mut next = Vec::with_capacity(pieces.len());
            for q in pieces {
                match cut(t, &q, point) {
                    Some(ps) => {
                        hit = true;
                        next.extend(ps);
                    }
                    None => next.push(q),
                }
            }
            pieces = next;
        }
        if hit {
            c.actions_hit += 1;
            c.pieces += pieces.len() as u64;
        }
        for p in &pieces {
            if pieces_out.len() as u64 >= PIECE as u64 {
                return Err("too many pieces for the banned view".into());
            }
            order.push(pieces_out.len() as u32 | PIECE);
            pieces_out.push(t.acts.meta[i], t.acts.members(i), p);
        }
    }
    c.actions_after = order.len() as u64;
    Ok(Banned { input: &t.acts, pieces: pieces_out, order })
}
