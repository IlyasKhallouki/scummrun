//! The switch ghost_slots (experiments/E004-fast-exact/b1-refine.md, section 2, Lemma G): for each unit some code of
//! the scope starts with the recursive flag, whether the instances older than its newest are invisible to the task.
//! The reading follows a unit's newest instance (a recursive start writes the unit's slot facts with the new start);
//! an older instance is a ghost. Its runs are stutters of the task when (a) the code it can run writes no fact of the
//! table but the unit's own slot facts, and (b) no instruction of the scope reads the unit's liveness. The records do
//! not change; the verdicts go into the reading's head and the report. Off by default; with it off nothing here runs.

use crate::domains::fold;
use crate::havoc;
use crate::record::Cut;
use crate::scope::Scope;
use crate::domain::Label;
use crate::table::FactTable;
use scumm_ir::{known, Expr, Game, Stmt, UnitKey};
use std::collections::{BTreeMap, BTreeSet};

/// The units a recursive start can name, and whether a start site's number did not fold.
fn recursive_targets(game: &Game, scope: &Scope) -> (BTreeSet<UnitKey>, usize) {
    let ast = &game.ast;
    let mut caps = 0i64;
    let res_of = |k: UnitKey| match k {
        UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
        UnitKey::Obj(o) => game.object_room.get(&o).and_then(|&r| game.resource(r)),
        UnitKey::Global(_) => None,
    };
    let all_res: Vec<i32> = scope.rooms.iter().filter_map(|&r| game.resource(r)).collect();
    let mut out = BTreeSet::new();
    let mut unknown = 0usize;
    let add = |n: i32, res: Option<i32>, out: &mut BTreeSet<UnitKey>| {
        if n <= 0 {
            return;
        }
        if n < game.first_local {
            if game.id(UnitKey::Global(n)).is_some() {
                out.insert(UnitKey::Global(n));
            }
        } else {
            let rooms: Vec<i32> = match res {
                Some(r) => vec![r],
                None => all_res.clone(),
            };
            for r in rooms {
                if game.id(UnitKey::Local(r, n)).is_some() {
                    out.insert(UnitKey::Local(r, n));
                }
            }
        }
    };
    for &u in &scope.units {
        let key = game.unit(u).key;
        for x in game.ins(u) {
            let Some(st) = x.st else { continue };
            if let Stmt::Start { name, script, obj: None, recursive: true, .. } = st {
                if name != known::START_SCRIPT {
                    continue;
                }
                match fold(ast, script, None, None, &mut caps) {
                    Some(ns) => {
                        for n in ns {
                            add(n as i32, res_of(key), &mut out);
                        }
                    }
                    None => unknown += 1,
                }
            }
        }
    }
    if unknown > 0 {
        // a recursive start whose number does not fold: every script unit of the scope may be its target
        for &u in &scope.units {
            let k = game.unit(u).key;
            if matches!(k, UnitKey::Global(_) | UnitKey::Local(..)) {
                out.insert(k);
            }
        }
    }
    (out, unknown)
}

/// The values of variable `n` when its domain in the table is a finite set of numbers (a variable's value stays in
/// its domain: the domain is its start value closed under the scope's writes, and the reader widens it when a write
/// falls outside), else None.
fn var_values(table: &FactTable, n: i32) -> Option<Vec<i64>> {
    let d = table.domain(&format!("var:{n}"))?;
    if d.low.is_some() || d.high.is_some() {
        return None;
    }
    d.values.iter().map(|l| if let Label::Int(v) = l { Some(*v) } else { None }).collect()
}

/// The script numbers whose liveness some instruction of the scope reads (`isScriptRunning`), with the cites of the
/// arguments that do not fold (a constant expression, or a variable with a finite domain).
fn liveness_reads(game: &Game, scope: &Scope, table: &FactTable) -> (BTreeSet<i64>, Vec<String>) {
    let ast = &game.ast;
    let mut caps = 0i64;
    let mut nums = BTreeSet::new();
    let mut unknown = Vec::new();
    for &u in &scope.units {
        for (i, x) in game.ins(u).iter().enumerate() {
            let Some(st) = x.st else { continue };
            ast.walk_stmt(&st, &mut |e| {
                if let Expr::Getter { name, args } = ast.expr(e) {
                    if name == known::IS_SCRIPT_RUNNING {
                        let a = ast.kids(args).first().copied();
                        let got = a.and_then(|a| {
                            fold(ast, a, None, None, &mut caps).or_else(|| match ast.expr(a) {
                                Expr::Var(n) => var_values(table, n),
                                _ => None,
                            })
                        });
                        match got {
                            Some(ns) => nums.extend(ns),
                            None => unknown.push(game.cite(u, i as u32)),
                        }
                    }
                }
                true
            });
        }
    }
    (nums, unknown)
}

/// The verdict per unit a recursive start can name, by its slot name: "invisible", or the conditions that failed.
pub fn verdicts(game: &Game, scope: &Scope, table: &FactTable) -> BTreeMap<String, String> {
    let (targets, unknown_starts) = recursive_targets(game, scope);
    let (reads, reads_unknown) = liveness_reads(game, scope, table);
    let mut out = BTreeMap::new();
    for k in targets {
        let name = Game::unit_name(k).to_string();
        let number = match k {
            UnitKey::Global(n) | UnitKey::Local(_, n) => n as i64,
            _ => continue,
        };
        let room = match k {
            UnitKey::Local(r, _) => Some(r),
            _ => None,
        };
        let cut = Cut { frames: vec![(k, Some(0))], steps: vec![] };
        let w = havoc::writes(game, scope, table, &cut, room);
        let mut why: Vec<String> = Vec::new();
        if !w.facts.is_empty() {
            let some: Vec<&String> = w.facts.keys().take(4).collect();
            why.push(format!("its code writes {} facts of the table ({})", w.facts.len(), some.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
        }
        let others: Vec<&String> = w.slots.iter().filter(|s| **s != name).collect();
        if !others.is_empty() {
            why.push(format!("its code touches {} other slots ({})", others.len(), others.iter().take(4).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
        }
        if w.frozen_all {
            why.push("its code freezes scripts".into());
        }
        if w.esc_all {
            why.push("its code opens or closes a cutscene or an override".into());
        }
        if reads.contains(&number) {
            why.push("its liveness is read (isScriptRunning)".into());
        } else if !reads_unknown.is_empty() {
            why.push(format!("an isScriptRunning argument does not fold ({})", reads_unknown.join(", ")));
        }
        out.insert(name, if why.is_empty() { "invisible".to_string() } else { why.join("; ") });
    }
    if unknown_starts > 0 {
        out.insert("(recursive starts whose number does not fold)".into(), unknown_starts.to_string());
    }
    out
}
