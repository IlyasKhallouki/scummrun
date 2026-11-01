//! Fix round 1, F1c (fix-round1.md, switch input_settled): what the reading needs to start a sentence or a walk at the
//! room's input script, from the state the harness's own play leaves. All of it is read from the IR; nothing here
//! names a game's room, object, verb, variable or script (D1, plan.md's C2).
//!
//! * **The plain input script**: a global whose own code holds a doSentence with three variable operands and no
//!   yield. Today's sentence record is what it does.
//! * **The roles**: `v`, `a`, `b`, the plain script's three operands; the hover, a global with a loop with a wait,
//!   holding Local[L] = findObject(VAR_VIRT_MOUSE_X, VAR_VIRT_MOUSE_Y) and Var[a] = Local[L]; `last`, the variable of
//!   its test Local[L] != Var[last] that the hover then sets; `p`, the bare variable it tests where the true side writes
//!   Var[b] and the false side Var[a].
//! * **The guards**: the hover's tests of a variable and a constant before its findObject whose false side jumps past it.
//!
//! The runs are machine.rs's (`f1c_click`, `f1c_end`), the jobs builder.rs's (`Builder::settled_jobs`).

use crate::facts::Facts;
use crate::table::MOUSE_VARS;
use rustc_hash::FxHashMap;
use scumm_ir::{known, CmpOp, Expr, ExprId, Game, Stmt, UnitId, UnitKey, Yield};
use std::collections::{BTreeMap, BTreeSet};

/// VAR_VIRT_MOUSE_X and VAR_VIRT_MOUSE_Y (vars.cpp).
pub const VAR_VIRT_MOUSE_X: i32 = MOUSE_VARS[0];
pub const VAR_VIRT_MOUSE_Y: i32 = MOUSE_VARS[1];
/// runInputScript's area of a click on the room, and the left button's code (verbs.h:35-39, verbs.cpp:666).
pub const SCENE_CLICK_AREA: i32 = 2;
pub const LEFT_BUTTON: i32 = 1;
/// Where a constant a waiting slot carries sits in its list of locals: this plus the variable's number. No script has
/// a local so high (scumm_ir NUM_LOCALS is 25).
pub const SETTLED_LOCAL: i64 = 1000;
/// A job's pin that says its run starts at this input script (machine.rs `Reader::run_full`). No fact has this name,
/// so it never reaches the machine's oracle.
pub const SETTLED_PIN: &str = "input_settled";

/// One hover test before its findObject: fact, comparison, constant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Guard {
    pub key: String,
    pub op: CmpOp,
    pub value: i64,
}

impl Guard {
    pub fn holds(&self, v: i64) -> bool {
        match self.op {
            CmpOp::Eq => v == self.value,
            CmpOp::Ne => v != self.value,
            CmpOp::Lt => v < self.value,
            CmpOp::Le => v <= self.value,
            CmpOp::Gt => v > self.value,
            CmpOp::Ge => v >= self.value,
        }
    }
}

/// What F1c read from the scripts (the module's docstring).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettledRule {
    /// the plain input scripts' numbers
    pub plain: BTreeSet<i64>,
    pub v: i32,
    pub a: i32,
    pub b: i32,
    pub p: i32,
    pub last: i32,
    /// the hover and its one wait
    pub hover: UnitKey,
    pub wait: i64,
    pub guards: Vec<Guard>,
    /// the five variables each scope unit's own code reads, where it reads any
    pub reads: FxHashMap<UnitKey, Vec<i32>>,
    /// a local input script's number -> the room resources whose code installs it (a constant write to
    /// VAR_VERB_SCRIPT); a number a global or an object's code installs is in `anywhere`
    pub installed: BTreeMap<i64, BTreeSet<i32>>,
    pub anywhere: BTreeSet<i64>,
}

/// The operands of doSentence(Var, Var, Var) in a unit's own code, and no yield in it.
fn plain_operands(game: &Game, u: UnitId) -> Option<(i32, i32, i32)> {
    let ast = &game.ast;
    let ins = game.ins(u);
    if ins.iter().any(|x| x.yields.is_some()) {
        return None;
    }
    for x in ins {
        let Some(Stmt::Op { name: known::DO_SENTENCE, args, flags, .. }) = x.st else { continue };
        if ast.flags(flags).contains(&known::STOP) {
            continue;
        }
        if let [Expr::Var(v), Expr::Var(a), Expr::Var(b)] = ast.kids(args).iter().map(|&e| ast.expr(e)).collect::<Vec<_>>()[..] {
            return Some((v, a, b));
        }
    }
    None
}

/// An expression that is Var[n], or its truth value.
fn bare_var(game: &Game, e: ExprId) -> Option<i32> {
    match game.ast.expr(e) {
        Expr::Var(n) => Some(n),
        Expr::Truth(i) => bare_var(game, i),
        _ => None,
    }
}

/// `Var[n]`'s assignment by `=`, as (n, right side).
fn set_var(game: &Game, st: &Stmt) -> Option<(i32, ExprId)> {
    let Stmt::Assign(a) = *st else { return None };
    match (game.ast.expr(a.lhs), a.op) {
        (Expr::Var(n), scumm_ir::AssignOp::Set) => Some((n, a.rhs)),
        _ => None,
    }
}

/// `Local[L] = findObject(VAR_VIRT_MOUSE_X, VAR_VIRT_MOUSE_Y)`: L.
fn find_at_mouse(game: &Game, st: &Stmt) -> Option<i32> {
    let ast = &game.ast;
    let Stmt::Assign(a) = *st else { return None };
    let (Expr::Local(l), Expr::Getter { name: known::FIND_OBJECT, args }) = (ast.expr(a.lhs), ast.expr(a.rhs)) else { return None };
    is_mouse_args(game, ast.kids(args)).then_some(l)
}

/// Whether a getter's arguments are VAR_VIRT_MOUSE_X and VAR_VIRT_MOUSE_Y.
pub fn is_mouse_args(game: &Game, args: &[ExprId]) -> bool {
    matches!(args, [x, y] if game.ast.expr(*x) == Expr::Var(VAR_VIRT_MOUSE_X) && game.ast.expr(*y) == Expr::Var(VAR_VIRT_MOUSE_Y))
}

/// The hover's roles, if `u` is a hover for the operands (the module's docstring).
fn hover_roles(game: &Game, u: UnitId, ops: (i32, i32, i32)) -> Option<(i64, i32, i32, Vec<Guard>)> {
    let ast = &game.ast;
    let ins = game.ins(u);
    let (_, a, b) = ops;
    let lp = game.loops(u, true).iter().find(|lp| lp.body.iter().any(|&i| ins[i as usize].st.is_some_and(|s| find_at_mouse(game, &s).is_some())))?;
    let (f, l) = lp.body.iter().find_map(|&i| ins[i as usize].st.and_then(|s| find_at_mouse(game, &s)).map(|l| (i as usize, l)))?;
    if !ins.iter().any(|x| x.st.is_some_and(|s| matches!(set_var(game, &s), Some((n, r)) if n == a && ast.expr(r) == Expr::Local(l)))) {
        return None;
    }
    // its wait in the loop: after a breakHere or a delay, or at a WaitFor op
    let mut waits: BTreeSet<i64> = BTreeSet::new();
    for w in crate::scope::stop_points(game, u) {
        let Some(i) = game.at(u, w) else { continue };
        let after = i > 0 && lp.contains(i as i64 - 1) && ins[i as usize - 1].yields.is_some_and(|y| y != Yield::Wait);
        if after || (lp.contains(i as i64) && ins[i as usize].yields == Some(Yield::Wait)) {
            waits.insert(w as i64);
        }
    }
    let [wait] = waits.into_iter().collect::<Vec<_>>()[..] else { return None };
    let (mut last, mut p) = (None, None);
    let mut guards = Vec::new();
    for (j, x) in ins.iter().enumerate() {
        let Some(Stmt::Jump { cond: Some(c), .. }) = x.st else { continue };
        let target = x.target.map_or(ins.len(), |t| t as usize);
        if let Expr::Cmp { op, a: ca, b: cb } = ast.expr(c) {
            // a guard: its false side jumps past the findObject
            if j < f && target > f {
                if let (Expr::Var(n), Some(k)) = (ast.expr(ca), ast.constant(cb)) {
                    guards.push(Guard { key: format!("var:{n}"), op, value: k as i64 });
                }
            }
            if op == CmpOp::Ne {
                let var = match (ast.expr(ca), ast.expr(cb)) {
                    (Expr::Local(x), Expr::Var(n)) | (Expr::Var(n), Expr::Local(x)) if x == l => Some(n),
                    _ => None,
                };
                let sets = var.filter(|&n| ins[j + 1..target].iter().any(|y| y.st.is_some_and(|s| matches!(set_var(game, &s), Some((m, r)) if m == n && ast.expr(r) == Expr::Local(l)))));
                if j > f && last.is_none() && sets.is_some() && sets != Some(a) && sets != Some(b) {
                    last = sets;
                }
            }
        }
        if let Some(n) = bare_var(game, c) {
            let writes = |y: &scumm_ir::Ins, m: i32| y.st.is_some_and(|s| matches!(set_var(game, &s), Some((k, _)) if k == m));
            if j > f && p.is_none() && ins[j + 1..target].iter().any(|y| writes(y, b)) && ins.get(target).is_some_and(|y| writes(y, a)) {
                p = Some(n);
            }
        }
    }
    Some((wait, last?, p?, guards))
}

/// F1c's reading of the scripts; None where the game has no plain script or no hover (the switch then changes nothing).
pub fn settled_rule(game: &Game, units: &[UnitId], start: &Facts) -> Option<SettledRule> {
    let ast = &game.ast;
    let mut plain = BTreeSet::new();
    let mut ops = None;
    for u in game.ids() {
        let UnitKey::Global(n) = game.unit(u).key else { continue };
        if let Some(o) = plain_operands(game, u) {
            if ops.is_some_and(|x| x != o) {
                return None;
            }
            ops = Some(o);
            plain.insert(n as i64);
        }
    }
    let ops = ops?;
    let mut hover = None;
    for u in game.ids() {
        if matches!(game.unit(u).key, UnitKey::Global(_)) && !plain.contains(&match game.unit(u).key { UnitKey::Global(n) => n as i64, _ => 0 }) {
            if let Some(r) = hover_roles(game, u, ops) {
                hover = Some((game.unit(u).key, r));
                break;
            }
        }
    }
    let (hover, (wait, last, p, guards)) = hover?;
    let vars = [ops.0, ops.1, ops.2, p, last];
    let mut reads: FxHashMap<UnitKey, Vec<i32>> = FxHashMap::default();
    let mut installed: BTreeMap<i64, BTreeSet<i32>> = BTreeMap::new();
    let mut anywhere: BTreeSet<i64> = BTreeSet::new();
    // the start's value: a global's anywhere, a local's in the start's room
    let at_start = start.get0(&format!("var:{}", crate::builder::VAR_VERB_SCRIPT));
    if (at_start as i32) < game.first_local {
        anywhere.insert(at_start);
    } else if let Some(res) = game.resource(start.get0("room") as i32) {
        installed.entry(at_start).or_default().insert(res);
    }
    for &u in units {
        let key = game.unit(u).key;
        let mut seen: BTreeSet<i32> = BTreeSet::new();
        for x in game.ins(u) {
            let Some(st) = x.st else { continue };
            let target = match st {
                Stmt::Assign(a) => Some(a.lhs),
                _ => None,
            };
            ast.walk_stmt(&st, &mut |e| {
                if Some(e) != target {
                    if let Expr::Var(n) = ast.expr(e) {
                        if vars.contains(&n) {
                            seen.insert(n);
                        }
                    }
                }
                true
            });
            if let Some((crate::builder::VAR_VERB_SCRIPT, r)) = set_var(game, &st) {
                if let Some(c) = ast.constant(r) {
                    match key {
                        UnitKey::Local(res, _) | UnitKey::Entry(res) | UnitKey::Exit(res) => {
                            installed.entry(c as i64).or_default().insert(res);
                        }
                        _ => {
                            anywhere.insert(c as i64);
                        }
                    }
                }
            }
        }
        if !seen.is_empty() {
            reads.insert(key, seen.into_iter().collect());
        }
    }
    Some(SettledRule { plain, v: ops.0, a: ops.1, b: ops.2, p, last, hover, wait, guards, reads, installed, anywhere })
}

impl SettledRule {
    /// Whether input script `n` can be VAR_VERB_SCRIPT's value in room resource `res`: a global's number is; a local's
    /// needs that room's own local of the number, and a write that installs it from that room's code, a global or an
    /// object's.
    pub fn runs_in(&self, game: &Game, n: i64, res: Option<i32>) -> bool {
        if (n as i32) < game.first_local {
            return true;
        }
        let Some(res) = res else { return false };
        game.id(UnitKey::Local(res, n as i32)).is_some()
            && (self.anywhere.contains(&n) || self.installed.get(&n).is_some_and(|s| s.contains(&res)))
    }
}
