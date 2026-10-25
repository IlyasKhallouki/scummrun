//! Fix round 1, F2 (fix-round1.md): what decides whether the engine's next turn of a slot does anything, from where
//! the slot stands between two frames. The engine gives every live, unfrozen slot its turn in runAllScripts before
//! checkExecVerbs reads the frame's input (scumm.cpp:3231-3232; script.cpp:968-987), so a slot whose wait is over
//! runs before the input of the same frame. The reader's planner rule (switch due_first) needs, per wait point:
//!
//! * [`WaitClass::Polled`]: a breakHere in a loop whose exit tests read only facts with keys the code fixes
//!   (`LoopKind::Facts`, no non-fact). The next turn leaves the loop exactly when its exit test holds; [`exits`]
//!   works that out from the facts' values, and [`poll_shape`] checks first that the code from the wait to the exit
//!   or the next yield is only jumps and constant writes (so the test is all the turn does while it stays).
//! * [`WaitClass::Plain`]: a breakHere in no loop that holds a yield. The next turn always runs on: its test is
//!   empty.
//! * [`WaitClass::Time`]: a WaitFor op, a delay (script_v5.cpp:972-987, script.cpp:1513-1530), or a breakHere in a
//!   loop whose exit tests read a non-fact (a timer, a position, a local, ...) or facts the code does not name.
//! * [`WaitClass::Ambient`]: a breakHere in a loop with no exit. The slot runs every frame.
//! * [`WaitClass::None`]: no place a slot stands between frames (after an opcode a nested call parked it, a start).
//!
//! Nothing here names a game's script, object, variable or room (D1, plan.md's C2).

use crate::analysis::{Loop, LoopKind, Read, WaitKind};
use crate::ast::*;
use crate::facts::Fact;
use crate::game::Game;
use crate::sym::known;

/// Where a slot stands between frames, by what decides its next turn (module docs).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WaitClass {
    /// the index of the loop in `Game::loops(u, cp)`
    Polled(u32),
    Plain,
    Time(&'static str),
    Ambient,
    None,
}

impl WaitClass {
    pub fn name(self) -> &'static str {
        match self {
            WaitClass::Polled(_) => "polled",
            WaitClass::Plain => "plain",
            WaitClass::Time(_) => "time",
            WaitClass::Ambient => "ambient",
            WaitClass::None => "none",
        }
    }
}

/// The class of the place `idx` of unit `u` (an instruction index; a slot that yielded at a breakHere or delay
/// stands at the instruction after it, one at a WaitFor op at the op itself), with switch const_polls `cp`.
pub fn wait_class(game: &Game, u: UnitId, idx: u32, cp: bool) -> WaitClass {
    let wp = game.wait_point(u, idx, cp);
    match wp.kind {
        WaitKind::Wait => WaitClass::Time("a WaitFor op"),
        WaitKind::Delay => WaitClass::Time("a delay"),
        WaitKind::Frame => match wp.loop_ {
            None => WaitClass::Plain,
            Some(li) => {
                let lp = &game.loops(u, cp)[li as usize];
                match lp.kind {
                    LoopKind::Ambient => WaitClass::Ambient,
                    LoopKind::Time => WaitClass::Time("a loop whose exit tests read only non-facts"),
                    LoopKind::Facts if !lp.non_facts.is_empty() => {
                        WaitClass::Time("a loop whose exit tests read a non-fact too")
                    }
                    LoopKind::Facts if !lp.reads.iter().all(|r| matches!(r, Read::Fact(_))) => {
                        WaitClass::Time("a loop whose exit tests read facts the code does not name")
                    }
                    LoopKind::Facts => WaitClass::Polled(li),
                }
            }
        },
        WaitKind::Parked | WaitKind::Start | WaitKind::Other => WaitClass::None,
    }
}

/// The facts a polled loop's exit tests read (every read of a `Polled` loop is a `Read::Fact`).
pub fn loop_facts(lp: &Loop) -> Vec<Fact> {
    lp.reads.iter().filter_map(|r| if let Read::Fact(k) = r { Some(*k) } else { None }).collect()
}

/// Why the turn from a polled wait is not a pure poll: the code from the wait (`idx`) to the loop's exit or the
/// next yield, on every path, must be jumps, writes of constants (or of anything to a local or a fact the tests do
/// not read), and an end of the script, and the only yield it reaches the one the slot waits after. `Ok` means
/// [`exits`] decides every valuation of the loop's facts.
pub fn poll_shape(game: &Game, u: UnitId, idx: u32, lp: &Loop) -> Result<(), &'static str> {
    let ins = game.ins(u);
    let ast = &game.ast;
    let reads = loop_facts(lp);
    let own = idx.checked_sub(1);
    let mut seen = vec![false; ins.len() + 1];
    let mut todo = vec![idx];
    while let Some(pc) = todo.pop() {
        if pc as usize >= ins.len() || !lp.contains(pc as i64) || seen[pc as usize] {
            continue;
        }
        seen[pc as usize] = true;
        let x = &ins[pc as usize];
        if x.yields.is_some() {
            if Some(pc) != own {
                return Err("another yield in the loop");
            }
            continue;
        }
        if x.ends == Some(Ends::Always) {
            continue;
        }
        match x.st {
            None => return Err("a line that did not parse"),
            Some(Stmt::Jump { cond: None, .. }) => todo.extend(x.target),
            Some(Stmt::Jump { cond: Some(c), .. }) => {
                cond_shape(ast, c)?;
                todo.extend(x.next);
                todo.extend(x.target);
            }
            Some(Stmt::Assign(a)) => {
                let hits = match ast.expr(a.lhs) {
                    Expr::Var(n) => reads.contains(&Fact::Var(n)),
                    Expr::Bit(n) => reads.contains(&Fact::Bit(n)),
                    Expr::Local(_) => false,
                    _ => return Err("a write whose target does not fold"),
                };
                if hits && (a.op != AssignOp::Set || ast.constant(a.rhs).is_none()) {
                    return Err("a write of a tested fact that is not a constant");
                }
                let mut embed = false;
                ast.walk(a.rhs, &mut |e| {
                    embed |= matches!(ast.expr(e), Expr::Embed(_));
                    !embed
                });
                if embed {
                    return Err("an opcode inside an expression");
                }
                todo.extend(x.next);
            }
            Some(Stmt::Start { .. }) | Some(Stmt::Op { .. }) => return Err("an opcode before the exit test"),
        }
    }
    Ok(())
}

fn cond_shape(ast: &Ast, c: ExprId) -> Result<(), &'static str> {
    let mut bad = None;
    ast.walk(c, &mut |e| {
        match ast.expr(e) {
            Expr::Local(_) => bad = Some("a local in the exit test"),
            Expr::Index { .. } => bad = Some("an indexed read in the exit test"),
            Expr::Embed(_) => bad = Some("an opcode inside the exit test"),
            Expr::Getter { name, .. }
                if !matches!(
                    name,
                    known::CLASS_OF_IS | known::GET_OBJECT_OWNER | known::GET_OBJECT_STATE | known::IS_SCRIPT_RUNNING
                ) =>
            {
                bad = Some("a getter exits() does not evaluate")
            }
            _ => {}
        }
        bad.is_none()
    });
    bad.map_or(Ok(()), Err)
}

/// From a polled wait at `idx` of unit `u` in loop `lp`, whether the next turn leaves the loop (or ends the script)
/// before it yields again, with the facts valued by `val` (the value of each fact the exit tests read). The rules
/// are model.py's: a comparison on int16 values (`i16`), o5's arithmetic, the getters' fact rules
/// (`_getter_on`). `Err` where a value is missing or the code is not a pure poll ([`poll_shape`]).
pub fn exits(game: &Game, u: UnitId, idx: u32, lp: &Loop, val: &dyn Fn(Fact) -> Option<i64>) -> Result<bool, &'static str> {
    let ins = game.ins(u);
    let own = idx.checked_sub(1);
    let mut env: Vec<(Fact, i64)> = Vec::new();
    let mut pc = idx;
    for _ in 0..=2 * ins.len() + 2 {
        if pc as usize >= ins.len() || !lp.contains(pc as i64) {
            return Ok(true);
        }
        let x = &ins[pc as usize];
        if x.yields.is_some() {
            return if Some(pc) == own { Ok(false) } else { Err("another yield in the loop") };
        }
        if x.ends == Some(Ends::Always) {
            return Ok(true);
        }
        let get = |k: Fact| env.iter().rev().find(|e| e.0 == k).map(|e| e.1).or_else(|| val(k));
        match x.st {
            None => return Err("a line that did not parse"),
            Some(Stmt::Jump { cond: None, .. }) => pc = x.target.ok_or("a jump with no target")?,
            Some(Stmt::Jump { cond: Some(c), .. }) => {
                // a jump goes to its target when its condition is false
                let held = eval(game, c, &get)? != 0;
                pc = if held { x.next.ok_or("no fallthrough")? } else { x.target.ok_or("a jump with no target")? };
            }
            Some(Stmt::Assign(a)) => {
                let key = match game.ast.expr(a.lhs) {
                    Expr::Var(n) => Some(Fact::Var(n)),
                    Expr::Bit(n) => Some(Fact::Bit(n)),
                    _ => None,
                };
                if let (Some(k), AssignOp::Set, Some(c)) = (key, a.op, game.ast.constant(a.rhs)) {
                    let c = if matches!(k, Fact::Bit(_)) { (c != 0) as i64 } else { c as i64 };
                    env.push((k, c));
                } else if let Some(k) = key {
                    if loop_facts(lp).contains(&k) {
                        return Err("a write of a tested fact that is not a constant");
                    }
                }
                pc = x.next.ok_or("no fallthrough")?;
            }
            Some(_) => return Err("an opcode before the exit test"),
        }
    }
    Err("no exit and no yield")
}

fn i16w(v: i64) -> i64 {
    v as i16 as i64
}

fn arith(op: ArithOp, a: i64, b: i64) -> Result<i64, &'static str> {
    let w = |v: i64| v as i32 as i64;
    Ok(match op {
        ArithOp::Add => w(a + b),
        ArithOp::Sub => w(a - b),
        ArithOp::Mul => w(a.wrapping_mul(b)),
        ArithOp::Div => {
            if b == 0 {
                return Err("a division by zero");
            }
            let q = a.abs() / b.abs();
            w(if (a >= 0) == (b >= 0) { q } else { -q })
        }
    })
}

/// An expression's value on facts (model.py `val`, `test`, `_getter_on` for the fact getters).
fn eval(game: &Game, e: ExprId, get: &dyn Fn(Fact) -> Option<i64>) -> Result<i64, &'static str> {
    let ast = &game.ast;
    let need = |k: Fact| get(k).ok_or("a fact with no value");
    Ok(match ast.expr(e) {
        Expr::Const(c) => c as i64,
        Expr::Var(n) => need(Fact::Var(n))?,
        Expr::Bit(n) => need(Fact::Bit(n))?,
        Expr::BinOp { op, a, b } => arith(op, eval(game, a, get)?, eval(game, b, get)?)?,
        Expr::Neg(a) => (-eval(game, a, get)?) as i32 as i64,
        Expr::Cmp { op, a, b } => {
            let (y, z) = (i16w(eval(game, a, get)?), i16w(eval(game, b, get)?));
            (match op {
                CmpOp::Eq => y == z,
                CmpOp::Ne => y != z,
                CmpOp::Lt => y < z,
                CmpOp::Le => y <= z,
                CmpOp::Gt => y > z,
                CmpOp::Ge => y >= z,
            }) as i64
        }
        Expr::Not(a) => (eval(game, a, get)? == 0) as i64,
        Expr::Truth(a) => (eval(game, a, get)? != 0) as i64,
        Expr::Getter { name, args } => {
            let args = ast.kids(args);
            let first = |k: usize| -> Result<i32, &'static str> {
                args.get(k).and_then(|&a| ast.constant(a)).ok_or("a getter argument that is not a constant")
            };
            match name {
                known::GET_OBJECT_OWNER => need(Fact::Owner(first(0)?))?,
                known::GET_OBJECT_STATE => need(Fact::State(first(0)?))?,
                known::IS_SCRIPT_RUNNING => match first(0)? {
                    0 => 0,
                    n => need(game.script_key(n))?,
                },
                known::CLASS_OF_IS => {
                    let obj = first(0)?;
                    let Some(Expr::List(items)) = args.get(1).map(|&a| ast.expr(a)) else {
                        return Err("classOfIs without a class list");
                    };
                    for &c in ast.kids(items) {
                        let n = ast.constant(c).ok_or("a class that is not a constant")?;
                        if n & 0x7F == 0 {
                            return Err("class 0");
                        }
                        let has = need(Fact::Class(obj, n & 0x7F))?;
                        if (has != 0) != (n & 0x80 != 0) {
                            return Ok(0);
                        }
                    }
                    1
                }
                _ => return Err("a getter exits() does not evaluate"),
            }
        }
        _ => return Err("an expression exits() does not evaluate"),
    })
}

/// The classes of every place a slot can stand between frames in the game (a WaitFor op, the instruction after a
/// breakHere or a delay), and of the polled ones whether they are pure polls: (class or reason, count), sorted.
pub fn survey(game: &Game, cp: bool) -> Vec<(String, usize)> {
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for u in game.ids() {
        let ins = game.ins(u);
        for idx in 0..=ins.len() as u32 {
            let c = wait_class(game, u, idx, cp);
            let name = match c {
                WaitClass::None => continue,
                WaitClass::Polled(li) => match poll_shape(game, u, idx, &game.loops(u, cp)[li as usize]) {
                    Ok(()) => "polled: a pure poll".to_string(),
                    Err(why) => format!("polled, not a pure poll: {why}"),
                },
                WaitClass::Time(why) => format!("time: {why}"),
                c => c.name().to_string(),
            };
            *counts.entry(name).or_default() += 1;
        }
    }
    counts.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int16_compare_and_division() {
        assert_eq!(i16w(0x8000), -0x8000);
        assert_eq!(arith(ArithOp::Div, -7, 2), Ok(-3));
        assert!(arith(ArithOp::Div, 1, 0).is_err());
    }
}
