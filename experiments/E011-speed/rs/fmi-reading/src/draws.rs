//! E012 component `tested`, its draws (experiments/E012-game-mechanics/plan.md, "Method" 2, and the amendment
//! "component `tested`"): each action's checks on a random draw, with the draw's range and the share of its values
//! that gives the check's side.
//!
//! * [`draws`]: one [`Draw`] per check whose reason is a random draw. A check's text names the draw but not its
//!   constants (`?(random draw: (getRandomNr) + ...)`, the reader's `arith_at`), so range and share need the scripts:
//!   [`DrawGame`] holds the game, loaded once per process. Without it, range and share are None.
//!
//! **Range and share** ([`DrawGame::site`]). The check's cite gives the jump. Each `Local[n]` or `Var[n]` its test
//! reads is followed back to the one assignment that reaches it in the unit: a local through yields (no other script
//! writes a slot's locals), a variable only through jumps and assignments to other targets (any other op may write it
//! or run a script that does). The search fails at the unit's start, an object's verb entry, an Esc point, an op that
//! writes the target in another way, or two reaching assignments. The test must then depend on exactly one
//! `getRandomNr(n)` whose `n` resolves to a constant the same way: `range` is (0, n) (`o5_getRandomNr`,
//! `common/random.cpp:62`). A getter of facts on the draw (classOfIs, getObjectState) keeps the range and leaves the
//! share unknown. `share` is the fraction of the n + 1 values for which the test takes the check's side, each value
//! run through the resolved arithmetic as the reader's machine does it (`arith_int`: int32; `test_base`: comparisons
//! at int16). The resolved operand printed the reader's way (`getRandomNr`, `(x) op ...`) must equal the check's own
//! text, or range and share are None.

use fmi_core::ReadingSource;
use scumm_ir::{known, ArithOp, AssignOp, CmpOp, Expr, ExprId, Game, Ins, RefKind, Stmt, UnitId};
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Mutex;

/// The reason text of a check on a random draw (`Reason::Random`, scumm-ir analysis.rs).
pub const DRAW_WHY: &str = "?(random draw";

/// Whether a check's reason is a random draw.
pub fn is_draw(why: &str) -> bool {
    why.starts_with(DRAW_WHY) && matches!(why.as_bytes().get(DRAW_WHY.len()), Some(b':') | Some(b')'))
}

/// A check's reason (`?(reason: what)` or `?(reason)`), or None for a check that is not on a non-fact.
pub fn reason_of(why: &str) -> Option<&str> {
    let r = why.strip_prefix("?(")?;
    let end = r.find([':', ')']).unwrap_or(r.len());
    Some(&r[..end])
}

/// One check of an action on a random draw (module docstring).
#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    /// the check's index in the action's checks
    pub check: usize,
    pub cite: String,
    pub cond: String,
    /// the side the check takes: the test's truth (None: a time loop's "exit" or "stay")
    pub side: Option<bool>,
    pub why: String,
    /// the cite of the `getRandomNr` the test depends on, when resolved
    pub site: Option<String>,
    /// the draw's values (0, n), when resolved
    pub range: Option<(i32, i32)>,
    /// the share of the draw's values that give `side`, when the test is worked out from the draw
    pub share: Option<f64>,
}

/// A check's parts: (cite, cond, side, why) from its JSON text `[cite, cond, side, why]`.
pub fn check_parts(text: &str) -> Option<(String, String, Option<bool>, String)> {
    let v: Value = serde_json::from_str(text).ok()?;
    let a = v.as_array()?;
    let s = |i: usize| a.get(i).and_then(Value::as_str).map(String::from);
    Some((s(0)?, s(1)?, a.get(2).and_then(Value::as_bool), s(3)?))
}

/// A check's reason text (its last element), borrowed from the check's text when it holds no escape.
pub(crate) fn check_why(text: &str) -> Option<std::borrow::Cow<'_, str>> {
    use std::borrow::Cow;
    let t = text.trim_end();
    let body = t.strip_suffix(']')?.trim_end().strip_suffix('"')?;
    let q = body.rfind('"')?;
    let w = &body[q + 1..];
    if !w.contains('\\') && body[..q].ends_with([' ', ',']) {
        return Some(Cow::Borrowed(w));
    }
    check_parts(text).map(|p| Cow::Owned(p.3))
}

/// The draw checks of action `i`; range and share from `game` when given.
pub fn draws(rd: &dyn ReadingSource, i: u32, game: Option<&DrawGame>) -> Vec<Draw> {
    let ln = crate::line::parse(rd.action_line(i)).unwrap_or_else(|e| panic!("action {i}: its line: {e}"));
    let mut out = Vec::new();
    for (j, ch) in ln.checks.iter().enumerate() {
        let Some((cite, cond, side, why)) = check_parts(ch) else { continue };
        if !is_draw(&why) {
            continue;
        }
        let mut d = Draw { check: j, cite, cond, side, why, site: None, range: None, share: None };
        if let Some(g) = game {
            if let Ok(s) = g.site(&d.cite) {
                if format!("{DRAW_WHY}: {})", s.what) == d.why {
                    d.site = Some(s.site.clone());
                    d.range = Some((0, s.n));
                    d.share = match (side, s.trues) {
                        (Some(b), Some(t)) => {
                            let k = if b { t } else { s.n as u64 + 1 - t };
                            Some(k as f64 / (s.n as f64 + 1.0))
                        }
                        _ => None,
                    };
                }
            }
        }
        out.push(d);
    }
    out
}

/// A draw check's site worked out from the scripts ([`DrawGame::site`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    /// the `getRandomNr`'s cite
    pub site: String,
    /// its operand: the draw's values are 0..=n
    pub n: i32,
    /// the test's operand that holds the draw, printed as the reader prints its unknown (constants elided)
    pub what: String,
    /// the number of the n + 1 values for which the test is true (None: a test the arithmetic does not decide, e.g.
    /// a getter's)
    pub trues: Option<u64>,
}

/// The game, for the draws' range and share (module docstring). Sites are worked out once each.
pub struct DrawGame {
    game: Game,
    files: HashMap<String, UnitId>,
    preds: Mutex<HashMap<UnitId, Vec<Vec<u32>>>>,
    sites: Mutex<HashMap<String, Result<Site, String>>>,
}

/// A value resolved from the scripts: a constant, the one draw, arithmetic on them, or a getter's value on them.
#[derive(Clone, Debug, PartialEq, Eq)]
enum S {
    C(i32),
    D,
    Bin(ArithOp, Box<S>, Box<S>),
    Neg(Box<S>),
    /// a getter (classOfIs, getObjectState, ...) of the draw: its value is a fact's, not worked out here; the
    /// reader passes its operand's unknown through (the operand that holds the draw)
    Opaque(Box<S>),
}

impl S {
    fn has_draw(&self) -> bool {
        match self {
            S::C(_) => false,
            S::D => true,
            S::Bin(_, a, b) => a.has_draw() || b.has_draw(),
            S::Neg(a) | S::Opaque(a) => a.has_draw(),
        }
    }

    /// `arith_int` (scumm machine value.rs): int32 arithmetic; None for a division by zero or a getter's value.
    fn eval(&self, d: i32) -> Option<i32> {
        Some(match self {
            S::C(v) => *v,
            S::D => d,
            S::Opaque(_) => return None,
            S::Neg(a) => (-(a.eval(d)? as i64)) as i32,
            S::Bin(op, a, b) => {
                let (x, y) = (a.eval(d)? as i64, b.eval(d)? as i64);
                match op {
                    ArithOp::Add => (x + y) as i32,
                    ArithOp::Sub => (x - y) as i32,
                    ArithOp::Mul => (x * y) as i32,
                    ArithOp::Div => {
                        if y == 0 {
                            return None;
                        }
                        let q = x.abs() / y.abs();
                        (if (x >= 0) == (y >= 0) { q } else { -q }) as i32
                    }
                }
            }
        })
    }

    /// The reader's name for the unknown this value is (`getRandomNr`; `arith_at`: `(x) op ...`; a negation and a
    /// getter keep their operand's), None when it holds no draw or the draw on both sides of an operator.
    fn what(&self) -> Option<String> {
        match self {
            S::C(_) => None,
            S::D => Some("getRandomNr".into()),
            S::Neg(a) | S::Opaque(a) => a.what(),
            S::Bin(op, a, b) => {
                let x = match (a.has_draw(), b.has_draw()) {
                    (true, false) => a.what()?,
                    (false, true) => b.what()?,
                    _ => return None,
                };
                let o = match op {
                    ArithOp::Add => "+",
                    ArithOp::Sub => "-",
                    ArithOp::Mul => "*",
                    ArithOp::Div => "/",
                };
                Some(format!("({x}) {o} ..."))
            }
        }
    }
}

/// A test resolved from the scripts.
enum T {
    Cmp(CmpOp, S, S),
    Not(S),
    Truth(S),
}

impl T {
    /// The test's truth for the draw's value `d` (`test_base`: comparisons at int16, the zero tests on the value);
    /// None where the value is not worked out.
    fn holds(&self, d: i32) -> Option<bool> {
        let i16w = |v: i32| v as i16 as i32;
        Some(match self {
            T::Cmp(op, a, b) => {
                let (y, z) = (i16w(a.eval(d)?), i16w(b.eval(d)?));
                match op {
                    CmpOp::Eq => y == z,
                    CmpOp::Ne => y != z,
                    CmpOp::Lt => y < z,
                    CmpOp::Le => y <= z,
                    CmpOp::Gt => y > z,
                    CmpOp::Ge => y >= z,
                }
            }
            T::Not(a) => a.eval(d)? == 0,
            T::Truth(a) => a.eval(d)? != 0,
        })
    }
}

/// The one draw a resolution may depend on: (unit, instruction, the getter's expression) and its operand.
type DrawAt = (UnitId, u32, ExprId, i32);

/// How deep a resolution follows assignments (each step goes back to an earlier assignment; a guard, never reached in
/// the scripts measured).
const MAX_DEPTH: u32 = 64;

impl DrawGame {
    /// The game under the repository root `root` (scumm_ir::Game::load).
    pub fn load(root: &Path) -> io::Result<DrawGame> {
        Ok(DrawGame::of(Game::load(root)?))
    }

    pub fn of(game: Game) -> DrawGame {
        let mut files = HashMap::new();
        for u in game.ids() {
            files.insert(game.syms.str(game.unit(u).file).to_string(), u);
        }
        DrawGame { game, files, preds: Mutex::new(HashMap::new()), sites: Mutex::new(HashMap::new()) }
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    /// The instruction a cite names (`file:line [XXXX]`).
    pub fn at(&self, cite: &str) -> Option<(UnitId, u32)> {
        let (file, rest) = cite.rsplit_once(':')?;
        let (line, off) = rest.split_once(" [")?;
        let line: u32 = line.parse().ok()?;
        let off = i32::from_str_radix(off.strip_suffix(']')?, 16).ok()?;
        let u = *self.files.get(file)?;
        let i = self.game.ins(u).iter().position(|x| x.offset == off && x.line == line)?;
        Some((u, i as u32))
    }

    /// The draw site of a check's cite, worked out once (module docstring); Err says why it is not.
    pub fn site(&self, cite: &str) -> Result<Site, String> {
        if let Some(s) = self.sites.lock().unwrap().get(cite) {
            return s.clone();
        }
        let s = self.work_out(cite);
        self.sites.lock().unwrap().insert(cite.to_string(), s.clone());
        s
    }

    fn work_out(&self, cite: &str) -> Result<Site, String> {
        let (u, i) = self.at(cite).ok_or("no instruction at the cite")?;
        let g = &self.game;
        let x = &g.ins(u)[i as usize];
        let Some(Stmt::Jump { cond: Some(c), .. }) = x.st else { return Err("not a conditional jump".into()) };
        let mut r = Resolver { dg: self, u, draw: None };
        let t = match g.ast.expr(c) {
            Expr::Cmp { op, a, b } => T::Cmp(op, r.sym(a, i, 0)?, r.sym(b, i, 0)?),
            Expr::Not(a) => T::Not(r.sym(a, i, 0)?),
            Expr::Truth(a) => T::Truth(r.sym(a, i, 0)?),
            // a getter as the test (the IR's jump on classOfIs and its kind): its value's truth
            Expr::Getter { .. } => T::Truth(r.sym(c, i, 0)?),
            _ => return Err("a test of another form".into()),
        };
        let (du, dj, _, n) = r.draw.ok_or("the test does not depend on a draw")?;
        let held = match &t {
            T::Cmp(_, a, b) => match (a.has_draw(), b.has_draw()) {
                (true, false) => a,
                (false, true) => b,
                _ => return Err("the draw on both sides".into()),
            },
            T::Not(a) | T::Truth(a) => a,
        };
        let what = held.what().ok_or("the draw on both sides of an operator")?;
        let mut trues = Some(0u64);
        for d in 0..=n {
            match t.holds(d) {
                Some(b) => trues = trues.map(|k| k + b as u64),
                None => {
                    trues = None;
                    break;
                }
            }
        }
        Ok(Site { site: g.cite(du, dj), n, what, trues })
    }

    /// The instructions that go to each instruction of `u` (by falling through or jumping), memoized.
    fn preds_of(&self, u: UnitId) -> Vec<Vec<u32>> {
        if let Some(p) = self.preds.lock().unwrap().get(&u) {
            return p.clone();
        }
        let ins = self.game.ins(u);
        let mut p = vec![Vec::new(); ins.len()];
        for (j, x) in ins.iter().enumerate() {
            for t in [x.next, x.target].into_iter().flatten() {
                if (t as usize) < ins.len() && !p[t as usize].contains(&(j as u32)) {
                    p[t as usize].push(j as u32);
                }
            }
        }
        self.preds.lock().unwrap().insert(u, p.clone());
        p
    }
}

/// What an instruction does to a target (a local or a variable).
enum W {
    /// nothing: the search goes on before it
    None,
    /// assigns it this value
    Def(ExprId),
    /// writes it some other way, or may (the search fails)
    Kill,
}

struct Resolver<'a> {
    dg: &'a DrawGame,
    u: UnitId,
    draw: Option<DrawAt>,
}

impl Resolver<'_> {
    /// The value of `e` read at instruction `i` (before it runs).
    fn sym(&mut self, e: ExprId, i: u32, depth: u32) -> Result<S, String> {
        if depth > MAX_DEPTH {
            return Err("too deep".into());
        }
        let g = &self.dg.game;
        let ast = &g.ast;
        match ast.expr(e) {
            Expr::Const(v) => Ok(S::C(v)),
            t @ (Expr::Local(_) | Expr::Var(_)) => {
                // a read of a target the same statement also writes inside an expression is not followed
                let x = &g.ins(self.u)[i as usize];
                if let Some(st) = x.st {
                    let mut written = false;
                    ast.walk_stmt(&st, &mut |y| {
                        if let Expr::Embed(a) = ast.expr(y) {
                            if ast.expr(a.lhs) == t {
                                written = true;
                                return false;
                            }
                        }
                        true
                    });
                    if written {
                        return Err("read where its statement writes it".into());
                    }
                }
                let (j, rhs) = self.reaching(i, t)?;
                self.sym(rhs, j, depth + 1)
            }
            Expr::BinOp { op, a, b } => {
                Ok(S::Bin(op, Box::new(self.sym(a, i, depth + 1)?), Box::new(self.sym(b, i, depth + 1)?)))
            }
            Expr::Neg(a) => Ok(S::Neg(Box::new(self.sym(a, i, depth + 1)?))),
            Expr::Embed(a) if a.op == AssignOp::Set => self.sym(a.rhs, i, depth + 1),
            Expr::Getter { name, args } if name == known::GET_RANDOM_NR => {
                let &[arg] = ast.kids(args) else { return Err("getRandomNr with other than one operand".into()) };
                let s = self.sym(arg, i, depth + 1)?;
                if s.has_draw() {
                    return Err("a draw's operand from a draw".into());
                }
                let n = s.eval(0).ok_or("a division by zero")?;
                if n < 0 {
                    return Err("a negative operand".into());
                }
                let at = (self.u, i, e, n);
                match self.draw {
                    None => self.draw = Some(at),
                    Some(d) if d == at => {}
                    Some(_) => return Err("two draws".into()),
                }
                Ok(S::D)
            }
            Expr::Getter { args, .. } => {
                // a getter of facts on the draw: exactly one operand holds it, every other is worked out
                let mut held = None;
                for &a in ast.kids(args) {
                    if matches!(ast.expr(a), Expr::List(_) | Expr::Str(_) | Expr::Concat(_)) {
                        continue;
                    }
                    let s = self.sym(a, i, depth + 1)?;
                    if s.has_draw() {
                        if held.is_some() {
                            return Err("a getter with the draw in two operands".into());
                        }
                        held = Some(s);
                    }
                }
                Ok(S::Opaque(Box::new(held.ok_or("a getter's value, not on the draw")?)))
            }
            _ => Err("an operand of another form".into()),
        }
    }

    /// What instruction `x` does to target `t`.
    fn write_of(&self, x: &Ins, t: Expr) -> W {
        let ast = &self.dg.game.ast;
        let Some(st) = x.st else { return W::Kill };
        let is_var = matches!(t, Expr::Var(_));
        let mut embed: Option<ExprId> = None;
        let mut kill = false;
        let mut n_embeds = 0;
        ast.walk_stmt(&st, &mut |y| {
            if let Expr::Embed(a) = ast.expr(y) {
                let l = ast.expr(a.lhs);
                if l == t {
                    n_embeds += 1;
                    if a.op == AssignOp::Set {
                        embed = Some(a.rhs);
                    } else {
                        kill = true;
                    }
                } else if matches!(l, Expr::Index { kind: RefKind::Local | RefKind::Var, .. }) {
                    kill = true;
                }
            }
            true
        });
        if n_embeds > 1 {
            kill = true;
        }
        match st {
            Stmt::Assign(a) => {
                let l = ast.expr(a.lhs);
                if l == t {
                    return if a.op == AssignOp::Set { W::Def(a.rhs) } else { W::Kill };
                }
                if matches!(l, Expr::Index { kind: RefKind::Local | RefKind::Var, .. }) {
                    return W::Kill;
                }
            }
            Stmt::Jump { .. } => {}
            Stmt::Start { .. } | Stmt::Op { .. } => {
                if is_var {
                    return W::Kill;
                }
            }
        }
        if kill {
            W::Kill
        } else if let Some(r) = embed {
            W::Def(r)
        } else {
            W::None
        }
    }

    /// The one assignment to `t` that reaches instruction `i` in the unit: (its instruction, its value).
    fn reaching(&self, i: u32, t: Expr) -> Result<(u32, ExprId), String> {
        let g = &self.dg.game;
        let unit = g.unit(self.u);
        let ins = g.ins(self.u);
        let preds = self.dg.preds_of(self.u);
        let open = |k: u32| k == 0 || unit.entries.iter().any(|e| e.1 == k) || ins.iter().any(|x| x.esc == Some(k));
        let mut seen = vec![false; ins.len()];
        let mut stack = vec![i];
        let mut defs: Vec<(u32, ExprId)> = Vec::new();
        while let Some(k) = stack.pop() {
            if open(k) {
                return Err("reached from the unit's start, an entry or an Esc point".into());
            }
            for &j in &preds[k as usize] {
                if seen[j as usize] {
                    continue;
                }
                seen[j as usize] = true;
                match self.write_of(&ins[j as usize], t) {
                    W::None => stack.push(j),
                    W::Def(r) => {
                        if !defs.contains(&(j, r)) {
                            defs.push((j, r));
                        }
                    }
                    W::Kill => return Err("written another way on a path".into()),
                }
            }
        }
        match defs.as_slice() {
            [d] => Ok(*d),
            [] => Err("no assignment reaches it".into()),
            _ => Err("two assignments reach it".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scumm_ir::{GameData, Script, UnitKey};

    fn game(units: &[(UnitKey, &str, &str)]) -> DrawGame {
        let mut sc = Script::new();
        for (k, file, text) in units {
            sc.add_unit(*k, None, file, "", text, None, None);
        }
        DrawGame::of(Game::new(sc, GameData { first_local: 200, ..Default::default() }))
    }

    #[test]
    fn reasons() {
        assert!(is_draw("?(random draw: getRandomNr)"));
        assert!(is_draw("?(random draw)"));
        assert!(!is_draw("?(random drawing: x)"));
        assert!(!is_draw("?(timer: local:207's delay count)"));
        assert_eq!(reason_of("?(not a fact: var:103)"), Some("not a fact"));
        assert_eq!(reason_of("?(cannot work out)"), Some("cannot work out"));
        assert_eq!(reason_of("3"), None);
        let ch = r#"["room-000-x/local-210.txt:12 [0016]", "classOfIs(Local[0],[134])", true, "?(random draw: ((getRandomNr) * ...) + ...)"]"#;
        assert_eq!(check_why(ch).as_deref(), Some("?(random draw: ((getRandomNr) * ...) + ...)"));
        let p = check_parts(ch).unwrap();
        assert_eq!((p.0.as_str(), p.2), ("room-000-x/local-210.txt:12 [0016]", Some(true)));
        let esc = r#"["a.txt:1 [0000]", "x \"q\"", "exit", "?(loop exit by time: x)"]"#;
        assert_eq!(check_why(esc).as_deref(), Some("?(loop exit by time: x)"));
        assert_eq!(check_parts(esc).unwrap().2, None);
    }

    /// A draw tested on the straight code after it, through a jump chain (an else-if on the draw), and the operand
    /// from an earlier constant assignment.
    #[test]
    fn shares() {
        let text = "[0000] (1A) Var[100] = 3;\n\
                    [0005] (AC) Exprmode Var[100] = (<VAR_RESULT = getRandomNr(Var[100])> + 1);\n\
                    [0013] (48) if (Var[100] == 1) /*0020*/ {\n\
                    [001A] (1A)   Var[7] = 1;\n\
                    [001F] (18) } else /*0030*/ {\n\
                    [0020] (48)   if (Var[100] <= 3) /*0030*/ {\n\
                    [0027] (1A)     Var[7] = 2;\n\
                    [002C] (**)   }\n\
                    [002C] (**) }\n\
                    [002C] (A0) stopObjectCode();\n";
        let g = game(&[(UnitKey::Global(50), "g/s50.txt", text)]);
        let s = g.site("g/s50.txt:3 [0013]").unwrap();
        assert_eq!(
            (s.site.as_str(), s.n, s.what.as_str(), s.trues),
            ("g/s50.txt:2 [0005]", 3, "(getRandomNr) + ...", Some(1))
        );
        // reached only by the jump from [0013]: Var[100] is still the draw plus 1, 3 of 4 values pass
        let s = g.site("g/s50.txt:6 [0020]").unwrap();
        assert_eq!((s.n, s.trues), (3, Some(3)));
    }

    /// A local through a yield and a loop back to its assignment; a variable is not followed through a yield.
    #[test]
    fn locals_and_vars() {
        let text = "[0000] (16) Local[1] = getRandomNr(5);\n\
                    [0004] (16) VAR_RESULT = getRandomNr(1);\n\
                    [0008] (80) breakHere();\n\
                    [0009] (48) if (Local[1] == 1) /*0013*/ {\n\
                    [0010] (18)   goto 0000;\n\
                    [0013] (**) }\n\
                    [0013] (48) if (Local[1] == 5) /*001D*/ {\n\
                    [001A] (1A)   Var[7] = 1;\n\
                    [001D] (**) }\n\
                    [001D] (A8) if (VAR_RESULT) /*0026*/ {\n\
                    [0022] (1A)   Var[7] = 2;\n\
                    [0026] (**) }\n\
                    [0026] (A0) stopObjectCode();\n";
        let g = game(&[(UnitKey::Global(51), "g/s51.txt", text)]);
        let s = g.site("g/s51.txt:7 [0013]").unwrap();
        assert_eq!((s.n, s.what.as_str(), s.trues), (5, "getRandomNr", Some(1)));
        assert_eq!(s.site, "g/s51.txt:1 [0000]");
        assert!(g.site("g/s51.txt:10 [001D]").is_err(), "VAR_RESULT is not followed through breakHere");
    }

    /// The reader's printing of the operand, int16 comparisons, a getter's test (range only), and a local the
    /// unit's start reaches (an argument: not resolved).
    #[test]
    fn forms() {
        let text = "[0000] (96) Local[1] = getRandomNr(10);\n\
                    [0005] (AC) Exprmode Local[0] = (330 + (Local[1] * 3));\n\
                    [0014] (9D) if (classOfIs(Local[0],[134])) /*0020*/ {\n\
                    [001D] (1A)   Var[7] = 1;\n\
                    [0020] (**) }\n\
                    [0020] (48) if (Local[0] > 345) /*002A*/ {\n\
                    [0027] (1A)   Var[7] = 2;\n\
                    [002A] (**) }\n\
                    [002A] (48) if (Local[2] == 1) /*0034*/ {\n\
                    [0031] (1A)   Var[7] = 3;\n\
                    [0034] (**) }\n\
                    [0034] (A0) stopObjectCode();\n";
        let g = game(&[(UnitKey::Global(52), "g/s52.txt", text)]);
        let s = g.site("g/s52.txt:3 [0014]").unwrap();
        assert_eq!((s.n, s.what.as_str(), s.trues), (10, "((getRandomNr) * ...) + ...", None));
        // 330 + 3d > 345 for d = 6..10
        let s = g.site("g/s52.txt:6 [0020]").unwrap();
        assert_eq!(s.trues, Some(5));
        assert!(g.site("g/s52.txt:9 [002A]").is_err());
        assert!(g.site("g/s52.txt:99 [002A]").is_err(), "a cite with another line");
    }

    /// A getter's value on the draw, assigned then tested: the range, no share; a counter stepped in a loop and a
    /// value set on two paths are not one assignment; a draw whose operand comes through a yield is not resolved.
    #[test]
    fn not_resolved() {
        let text = "[0000] (AC) Exprmode Local[1] = (454 + <VAR_RESULT = getRandomNr(9)>);\n\
                    [000D] (8F) VAR_RESULT = getObjectState(Local[1]);\n\
                    [0012] (48) if (VAR_RESULT == 0) /*001C*/ {\n\
                    [0019] (1A)   Var[7] = 1;\n\
                    [001C] (**) }\n\
                    [001C] (1A) Local[2] = 0;\n\
                    [0021] (46) Local[2]++;\n\
                    [0024] (80) breakHere();\n\
                    [0025] (44) unless (Local[2] > Local[1]) goto 0021;\n\
                    [002C] (AC) Exprmode Local[3] = (<VAR_RESULT = getRandomNr(5)> + 445);\n\
                    [0039] (44) if (Local[3] > 448) /*0045*/ {\n\
                    [0040] (5A)   Local[3] += 1;\n\
                    [0045] (**) }\n\
                    [0045] (04) if (Local[3] <= 448) /*0050*/ {\n\
                    [004C] (1A)   Var[7] = 2;\n\
                    [0050] (**) }\n\
                    [0050] (AE) WaitForCamera();\n\
                    [0052] (96) Local[4] = getRandomNr(Var[164]);\n\
                    [0057] (48) if (Local[4] == 2) /*0061*/ {\n\
                    [005E] (1A)   Var[7] = 3;\n\
                    [0061] (**) }\n\
                    [0061] (A0) stopObjectCode();\n";
        let g = game(&[(UnitKey::Global(53), "g/s53.txt", text)]);
        let s = g.site("g/s53.txt:3 [0012]").unwrap();
        assert_eq!((s.n, s.what.as_str(), s.trues), (9, "(getRandomNr) + ...", None));
        assert_eq!(s.site, "g/s53.txt:1 [0000]");
        assert!(g.site("g/s53.txt:9 [0025]").is_err(), "a counter");
        assert!(g.site("g/s53.txt:14 [0045]").is_err(), "two values");
        assert!(g.site("g/s53.txt:19 [0057]").is_err(), "Var[164] through WaitForCamera");
        // Local[3] in 445..450 is over 448 for 2 of the 6 values
        let s = g.site("g/s53.txt:11 [0039]").unwrap();
        assert_eq!((s.n, s.trues), (5, Some(2)));
    }
}
