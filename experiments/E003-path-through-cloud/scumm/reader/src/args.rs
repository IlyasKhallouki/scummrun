//! The switch havoc_args (experiments/E005-narrow-havoc/havoc-args.md has the rule and its proof): a start in a havoc
//! closure whose number is a local, or a variable indexed by one, names the scripts its number's values name, from a
//! static analysis of the game's code: each unit's locals by a forward analysis of its control flow over sets and
//! ranges, started from what its launches pass. Off by default; with it off nothing here runs.

use crate::domains::{self, VarEnv};
use crate::scope::{Scope, NUM_ACTORS};
use crate::table::{FactTable, Kind};
use rustc_hash::{FxHashMap, FxHashSet};
use scumm_ir::{known, ArithOp, AssignOp, CmpOp, Expr, ExprId, Game, RefKind, Stmt, UnitId, UnitKey};
use std::collections::{BTreeMap, BTreeSet};

/// The most numbers a set holds before it becomes a range.
pub const MAX_SET: usize = 64;
/// The bounds that stand for minus and plus infinity: far from i64's own, so sums and products saturate in range.
pub const LO: i64 = -(1 << 40);
pub const HI: i64 = 1 << 40;
/// A unit's locals (script.h NUM_SCRIPT_LOCAL).
pub const NUM_LOCALS: usize = domains::NUM_LOCALS as usize;
/// Rounds at an instruction before a growing bound is widened to infinity, and the narrowing rounds after.
const WIDEN_AFTER: u32 = 8;
const NARROW_ROUNDS: u32 = 2;
/// The largest products a set operation computes pairwise (domains.rs MAX_SET).
const PAIRS: usize = 4096;
/// A variable index past this reaches the local range (`writeVar`'s 0x4000 bit).
const LOCAL_BIT: i64 = 0x4000;
/// Hook variables (vars.cpp:62-71): the engine starts the script they hold.
const HOOKS: std::ops::Range<i32> = domains::HOOKS;
/// The boot script (scumm.cpp runBootscript).
const BOOT: i32 = 1;

/// The values a local or an expression can have: a set, a range (bounds LO and HI are unbounded), or anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum V {
    Set(BTreeSet<i64>),
    Range(i64, i64),
    Top,
}

fn clamp(x: i64) -> i64 {
    x.clamp(LO, HI)
}

impl V {
    pub fn c(v: i64) -> V {
        V::Set([v].into_iter().collect())
    }
    pub fn set(s: BTreeSet<i64>) -> V {
        if s.len() > MAX_SET {
            V::range(*s.first().unwrap(), *s.last().unwrap())
        } else {
            V::Set(s)
        }
    }
    /// A range, as a set when it is small, as Top when it is unbounded both ways.
    pub fn range(lo: i64, hi: i64) -> V {
        let (lo, hi) = (clamp(lo), clamp(hi));
        if lo <= LO && hi >= HI {
            V::Top
        } else if hi >= lo && hi - lo < MAX_SET as i64 {
            V::Set((lo..=hi).collect())
        } else {
            V::Range(lo, hi)
        }
    }
    /// The lowest and highest value (a set is never empty).
    pub fn bounds(&self) -> (i64, i64) {
        match self {
            V::Set(s) => (*s.first().unwrap(), *s.last().unwrap()),
            V::Range(lo, hi) => (*lo, *hi),
            V::Top => (LO, HI),
        }
    }
    pub fn join(&self, o: &V) -> V {
        match (self, o) {
            (V::Top, _) | (_, V::Top) => V::Top,
            (V::Set(a), V::Set(b)) => V::set(a.union(b).copied().collect()),
            _ => {
                let (a, b) = (self.bounds(), o.bounds());
                V::range(a.0.min(b.0), a.1.max(b.1))
            }
        }
    }
    /// Whether every value of `self` is one of `o`'s.
    pub fn within(&self, o: &V) -> bool {
        match (self, o) {
            (_, V::Top) => true,
            (V::Top, _) => false,
            (V::Set(a), V::Set(b)) => a.is_subset(b),
            (_, V::Range(lo, hi)) => {
                let (a, b) = self.bounds();
                *lo <= a && b <= *hi
            }
            (V::Range(..), V::Set(_)) => false,
        }
    }
    /// `self` joined with `new`, where a bound that moves goes to infinity.
    pub fn widen(&self, new: &V) -> V {
        if new.within(self) {
            return self.clone();
        }
        let ((a, b), (c, d)) = (self.bounds(), new.bounds());
        V::range(if c < a { LO } else { a }, if d > b { HI } else { b })
    }
    /// The numbers, when there are at most `max`.
    pub fn values(&self, max: usize) -> Option<BTreeSet<i64>> {
        match self {
            V::Set(s) if s.len() <= max => Some(s.clone()),
            V::Range(lo, hi) if *lo > LO && *hi < HI && ((hi - lo) as usize) < max => Some((*lo..=*hi).collect()),
            _ => None,
        }
    }
    /// `a op b` (script_v5.cpp's arithmetic; a division by a set that holds 0 is anything).
    pub fn arith(op: ArithOp, a: &V, b: &V) -> V {
        let f = |x: i64, y: i64| -> Option<i64> {
            Some(clamp(match op {
                ArithOp::Add => x.saturating_add(y),
                ArithOp::Sub => x.saturating_sub(y),
                ArithOp::Mul => x.saturating_mul(y),
                ArithOp::Div => {
                    if y == 0 {
                        return None;
                    }
                    // C division truncates toward zero (domains.rs `div`)
                    let q = x.abs() / y.abs();
                    if (x >= 0) == (y >= 0) {
                        q
                    } else {
                        -q
                    }
                }
            }))
        };
        if let (V::Set(x), V::Set(y)) = (a, b) {
            if x.len() * y.len() <= PAIRS {
                let mut out = BTreeSet::new();
                for &p in x {
                    for &q in y {
                        match f(p, q) {
                            Some(v) => {
                                out.insert(v);
                            }
                            None => return V::Top,
                        }
                    }
                }
                return V::set(out);
            }
        }
        if matches!(a, V::Top) || matches!(b, V::Top) {
            return V::Top;
        }
        let ((p, q), (r, s)) = (a.bounds(), b.bounds());
        if op == ArithOp::Div && r <= 0 && 0 <= s {
            return V::Top;
        }
        let c: Vec<i64> = [(p, r), (p, s), (q, r), (q, s)].iter().filter_map(|&(x, y)| f(x, y)).collect();
        V::range(*c.iter().min().unwrap(), *c.iter().max().unwrap())
    }
    /// The values `x` of `self` for which `x op c` holds; None when there is none.
    pub fn refine(&self, op: CmpOp, c: i64) -> Option<V> {
        if let V::Set(s) = self {
            let out: BTreeSet<i64> = s.iter().copied().filter(|&x| cmp(op, x, c)).collect();
            return (!out.is_empty()).then_some(V::Set(out));
        }
        let (lo, hi) = self.bounds();
        let (lo, hi) = match op {
            CmpOp::Eq => (lo.max(c), hi.min(c)),
            CmpOp::Ne if lo == c => (lo + 1, hi),
            CmpOp::Ne if hi == c => (lo, hi - 1),
            CmpOp::Ne => return Some(self.clone()),
            CmpOp::Lt => (lo, hi.min(c - 1)),
            CmpOp::Le => (lo, hi.min(c)),
            CmpOp::Gt => (lo.max(c + 1), hi),
            CmpOp::Ge => (lo.max(c), hi),
        };
        (lo <= hi).then(|| V::range(lo, hi))
    }
}

fn cmp(op: CmpOp, x: i64, y: i64) -> bool {
    match op {
        CmpOp::Eq => x == y,
        CmpOp::Ne => x != y,
        CmpOp::Lt => x < y,
        CmpOp::Le => x <= y,
        CmpOp::Gt => x > y,
        CmpOp::Ge => x >= y,
    }
}

/// `!(x op c)` as `x op' c`.
fn negate(op: CmpOp) -> CmpOp {
    match op {
        CmpOp::Eq => CmpOp::Ne,
        CmpOp::Ne => CmpOp::Eq,
        CmpOp::Lt => CmpOp::Ge,
        CmpOp::Le => CmpOp::Gt,
        CmpOp::Gt => CmpOp::Le,
        CmpOp::Ge => CmpOp::Lt,
    }
}

/// `c op x` as `x op' c`.
fn mirror(op: CmpOp) -> CmpOp {
    match op {
        CmpOp::Lt => CmpOp::Gt,
        CmpOp::Le => CmpOp::Ge,
        CmpOp::Gt => CmpOp::Lt,
        CmpOp::Ge => CmpOp::Le,
        o => o,
    }
}

/// A unit's locals (its arguments, at its start).
pub type Locals = Vec<V>;

/// What the analysis holds at an instruction: the unit's locals, and the variables a condition narrowed since the
/// last instruction that could run other code or yield (only assignments and jumps keep them: nothing else runs in
/// between, and the engine changes its own variables only between frames).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub locals: Locals,
    pub vars: BTreeMap<i32, V>,
}

impl State {
    fn of(locals: Locals) -> State {
        State { locals, vars: BTreeMap::new() }
    }
    fn top() -> State {
        State::of(all_top())
    }
    fn join(&self, o: &State) -> State {
        let vars = self.vars.iter().filter_map(|(n, v)| o.vars.get(n).map(|w| (*n, v.join(w)))).collect();
        State { locals: join_locals(&self.locals, &o.locals), vars }
    }
    fn widen(&self, new: &State) -> State {
        let vars = self.vars.iter().filter_map(|(n, v)| new.vars.get(n).map(|w| (*n, v.widen(w)))).collect();
        State { locals: self.locals.iter().zip(&new.locals).map(|(o, x)| o.widen(x)).collect(), vars }
    }
}

fn all_top() -> Locals {
    vec![V::Top; NUM_LOCALS]
}

fn join_locals(a: &Locals, b: &Locals) -> Locals {
    a.iter().zip(b).map(|(x, y)| x.join(y)).collect()
}

/// A variable's values by the domain convention (havoc-cut.md, "Values"; havoc.rs `var_values`): a fact's domain of
/// plain values, a variable no code of the scope writes its setting or its start value; anything otherwise. The
/// mouse's screen position is a range: the engine clips it to the screen (F12, scumm-machine `mouse_range`,
/// input.cpp:414-434).
pub fn var_vals(scope: &Scope, table: &FactTable, n: i32) -> V {
    if n == VAR_MOUSE_X {
        return V::range(0, scumm_machine::machine::SCREEN_WIDTH as i64 - 1);
    }
    if n == VAR_MOUSE_Y {
        return V::range(0, scumm_machine::machine::SCREEN_HEIGHT as i64 - 1);
    }
    let key = format!("var:{n}");
    match table.kind(&key, scope) {
        Kind::Fact => match table.domain(&key) {
            Some(d) => match d.values.iter().map(|l| l.int()).collect::<Option<BTreeSet<i64>>>() {
                Some(s) if !s.is_empty() => V::set(s),
                _ => V::Top,
            },
            None => V::Top,
        },
        Kind::Const => V::c(domains::SETTINGS.iter().find(|s| s.0 == n).map_or_else(|| table.start.get0(&key), |s| s.1)),
        Kind::NonFact => V::Top,
    }
}

/// VAR_MOUSE_X and VAR_MOUSE_Y (vars.cpp:79-80).
const VAR_MOUSE_X: i32 = 44;
const VAR_MOUSE_Y: i32 = 45;

/// The instructions whose locals the analysis keeps: starts, and the ops whose operands havoc_rooms reads.
fn is_site(st: &Stmt) -> bool {
    match st {
        Stmt::Start { .. } => true,
        Stmt::Op { name, .. } => {
            [known::LOAD_ROOM, known::LOAD_ROOM_WITH_EGO, known::ACTOR_FOLLOW_CAMERA, known::PUT_ACTOR_IN_ROOM].contains(name)
        }
        _ => false,
    }
}

/// The analysis of the game's code (havoc-args.md, steps 1 to 4).
pub struct HavocArgs {
    /// the state at each start and each room op of a unit some launch reaches
    sites: FxHashMap<(UnitId, u32), State>,
    /// the variables' values the analysis read (by the domain convention), each once
    vars: FxHashMap<i32, V>,
    /// units no launch reaches (their code never runs)
    pub unreached: usize,
    /// computed starts whose values are not known: they may start any unit
    pub wild: Vec<(UnitId, u32)>,
    /// the rounds the fixpoint took
    pub rounds: u32,
}

/// What evaluating an expression needs: the variables' values.
struct Env<'a> {
    game: &'a Game,
    scope: &'a Scope,
    table: &'a FactTable,
    vars: std::cell::RefCell<FxHashMap<i32, V>>,
    settings: VarEnv,
}

impl Env<'_> {
    fn var(&self, n: i32) -> V {
        if let Some(v) = self.vars.borrow().get(&n) {
            return v.clone();
        }
        let v = var_vals(self.scope, self.table, n);
        self.vars.borrow_mut().insert(n, v.clone());
        v
    }

    /// The values of `e` in state `s`.
    fn eval(&self, e: ExprId, s: &State) -> V {
        let ast = &self.game.ast;
        match ast.expr(e) {
            Expr::Const(v) => V::c(v as i64),
            Expr::Local(k) => s.locals.get(k as usize).cloned().unwrap_or(V::Top),
            Expr::Var(n) => s.vars.get(&n).cloned().unwrap_or_else(|| self.var(n)),
            Expr::Bit(_) => V::range(0, 1),
            Expr::Index { kind, base, offset } => {
                let Some(xs) = self.eval(offset, s).values(MAX_SET) else { return V::Top };
                let mut out: Option<V> = None;
                for x in xs {
                    let n = base as i64 + x;
                    let v = match kind {
                        RefKind::Bit => V::range(0, 1),
                        // an index past the variables reads a bit or a local (readVar's 0x8000 and 0x4000 bits)
                        RefKind::Var if !(0..LOCAL_BIT).contains(&n) => V::Top,
                        RefKind::Var => s.vars.get(&(n as i32)).cloned().unwrap_or_else(|| self.var(n as i32)),
                        RefKind::Local => usize::try_from(n).ok().and_then(|k| s.locals.get(k)).cloned().unwrap_or(V::Top),
                    };
                    out = Some(match out {
                        Some(o) => o.join(&v),
                        None => v,
                    });
                }
                out.unwrap_or(V::Top)
            }
            Expr::BinOp { op, a, b } => V::arith(op, &self.eval(a, s), &self.eval(b, s)),
            Expr::Neg(x) => V::arith(ArithOp::Sub, &V::c(0), &self.eval(x, s)),
            Expr::Getter { name, args } => {
                if name == known::FIND_OBJECT {
                    // an object number (uint16) or 0 (object.cpp findObject)
                    V::range(0, 0xFFFF)
                } else if name == known::ACTOR_FROM_POS {
                    // an actor number or 0 (actor.cpp getActorFromPos)
                    V::range(0, NUM_ACTORS - 1)
                } else if name == known::GET_RANDOM_NR {
                    // 0 to its operand (o5_getRandomNr, RandomSource::getRandomNumber)
                    match ast.kids(args).first().map(|&a| self.eval(a, s)) {
                        Some(v) if !matches!(v, V::Top) && v.bounds().1 < HI => V::range(0, v.bounds().1.max(0)),
                        _ => V::Top,
                    }
                } else {
                    V::Top
                }
            }
            Expr::Embed(a) if a.op == AssignOp::Set && matches!(ast.expr(a.lhs), Expr::Var(0)) => self.eval(a.rhs, s),
            Expr::Cmp { .. } | Expr::Not(_) | Expr::Truth(_) => V::range(0, 1),
            _ => V::Top,
        }
    }

    /// Whether a write to `target` may land on a local: a local, or an indexed variable or bit whose index may leave
    /// the variables (writeVar's 0x4000 bit).
    fn may_hit_locals(&self, target: ExprId, s: &State) -> bool {
        match self.game.ast.expr(target) {
            Expr::Index { kind: RefKind::Local, .. } => true,
            Expr::Index { base, offset, .. } => {
                let (lo, hi) = self.eval(offset, s).bounds();
                !(base as i64 + lo >= 0 && base as i64 + hi < LOCAL_BIT)
            }
            _ => false,
        }
    }

    /// The state after instruction `st` (its writes, in the order the engine makes them).
    fn transfer(&self, st: Option<Stmt>, s: &State) -> State {
        let ast = &self.game.ast;
        let Some(st) = st else { return State::top() };
        let mut out = s.clone();
        // an opcode run inside an expression writes its target first, and may run anything
        for y in domains::stmt_nodes(ast, &st) {
            if let Expr::Embed(a) = ast.expr(y) {
                out.vars.clear();
                match ast.expr(a.lhs) {
                    Expr::Local(k) => {
                        if let Some(v) = out.locals.get_mut(k as usize) {
                            *v = V::Top;
                        }
                    }
                    _ if self.may_hit_locals(a.lhs, s) => return State::top(),
                    _ => {}
                }
            }
        }
        match st {
            Stmt::Assign(a) => {
                let rhs = self.eval(a.rhs, &out);
                match ast.expr(a.lhs) {
                    Expr::Local(k) if (k as usize) < NUM_LOCALS => {
                        let k = k as usize;
                        out.locals[k] = match a.op.arith() {
                            None => rhs,
                            Some(op) => V::arith(op, &out.locals[k], &rhs),
                        };
                    }
                    Expr::Local(_) => return State::top(),
                    Expr::Var(n) => {
                        let v = match a.op.arith() {
                            None => rhs,
                            Some(op) => V::arith(op, &self.eval(a.lhs, &out), &rhs),
                        };
                        out.vars.insert(n, v);
                    }
                    _ if self.may_hit_locals(a.lhs, &out) => return State::top(),
                    // an indexed variable: the narrowed variables it may hit are no longer known
                    Expr::Index { kind: RefKind::Var, .. } => out.vars.clear(),
                    _ => {}
                }
            }
            Stmt::Jump { .. } => {}
            Stmt::Op { name, args, .. } => {
                // an op may run other code or yield: no narrowed variable holds after it
                out.vars.clear();
                if name == known::SET_VAR_RANGE {
                    let a = ast.kids(args);
                    match a.first().map(|&f| ast.expr(f)) {
                        Some(Expr::Local(n)) => {
                            for (j, &item) in domains::list_items(ast, a[2]).iter().enumerate() {
                                let v = self.eval(item, s);
                                match out.locals.get_mut(n as usize + j) {
                                    Some(x) => *x = v,
                                    None => return State::top(),
                                }
                            }
                        }
                        Some(_) if self.may_hit_locals(a[0], &out) => return State::top(),
                        // setVarRange on variables writes len of them from its base, its own operand
                        _ => {}
                    }
                }
            }
            Stmt::Start { .. } => out.vars.clear(),
        }
        out
    }

    /// The state on each edge out of instruction `i`: (successor, state), the condition narrowing a jump's.
    fn edges(&self, u: UnitId, i: u32, s: &State) -> Vec<(u32, State)> {
        let ins = &self.game.ins(u)[i as usize];
        let out = self.transfer(ins.st, s);
        let mut v = Vec::new();
        let mut caps = 0i64;
        match ins.st {
            Some(Stmt::Jump { cond: Some(c), .. }) => {
                let decided = domains::decide(&self.game.ast, c, &self.settings, &mut caps);
                // the fallthrough holds the condition, the jump its negation
                if decided != Some(false) {
                    if let (Some(n), Some(st)) = (ins.next, self.narrow(c, true, &out)) {
                        v.push((n, st));
                    }
                }
                if decided != Some(true) {
                    if let (Some(t), Some(st)) = (ins.target, self.narrow(c, false, &out)) {
                        v.push((t, st));
                    }
                }
            }
            _ => {
                for j in [ins.next, ins.target].into_iter().flatten() {
                    v.push((j, out.clone()));
                }
            }
        }
        if let Some(e) = ins.esc {
            // Esc resumes there in a later frame: no narrowed variable holds
            let mut o = out;
            o.vars.clear();
            v.push((e, o));
        }
        v
    }

    /// The state where condition `c` is `truth`; None when no value can make it so.
    fn narrow(&self, c: ExprId, truth: bool, s: &State) -> Option<State> {
        let ast = &self.game.ast;
        let one = |e: ExprId| -> Option<i64> {
            match self.eval(e, s).values(1) {
                Some(v) if v.len() == 1 => v.into_iter().next(),
                _ => None,
            }
        };
        // what can be narrowed: a local, or a variable
        enum W {
            L(usize),
            G(i32),
        }
        let what = |e: ExprId| match ast.expr(e) {
            Expr::Local(k) if (k as usize) < NUM_LOCALS => Some(W::L(k as usize)),
            Expr::Var(n) => Some(W::G(n)),
            _ => None,
        };
        let (w, op, val) = match ast.expr(c) {
            Expr::Not(x) => return self.narrow(x, !truth, s),
            Expr::Truth(x) => match what(x) {
                Some(w) => (w, if truth { CmpOp::Ne } else { CmpOp::Eq }, 0),
                None => return Some(s.clone()),
            },
            Expr::Local(_) | Expr::Var(_) => (what(c).unwrap(), if truth { CmpOp::Ne } else { CmpOp::Eq }, 0),
            Expr::Cmp { op, a, b } => {
                let op = if truth { op } else { negate(op) };
                match (what(a), what(b)) {
                    (Some(w), _) => match one(b) {
                        Some(v) => (w, op, v),
                        None => return Some(s.clone()),
                    },
                    (_, Some(w)) => match one(a) {
                        Some(v) => (w, mirror(op), v),
                        None => return Some(s.clone()),
                    },
                    _ => return Some(s.clone()),
                }
            }
            _ => return Some(s.clone()),
        };
        let mut out = s.clone();
        match w {
            W::L(k) => out.locals[k] = out.locals[k].refine(op, val)?,
            W::G(n) => {
                let cur = s.vars.get(&n).cloned().unwrap_or_else(|| self.var(n));
                out.vars.insert(n, cur.refine(op, val)?);
            }
        }
        Some(out)
    }

    /// The state at each instruction of unit `u` a run from `entry` reaches (None: not reached).
    fn unit(&self, u: UnitId, entry: &Locals) -> Vec<Option<State>> {
        let game = self.game;
        let unit = game.unit(u);
        let n = game.ins(u).len();
        let entry = State::of(entry.clone());
        let mut at: Vec<Option<State>> = vec![None; n];
        let starts: Vec<u32> = match unit.key {
            UnitKey::Obj(_) => unit.entries.iter().map(|e| e.1).filter(|&i| (i as usize) < n).collect(),
            _ => (n > 0).then_some(0).into_iter().collect(),
        };
        let mut visits = vec![0u32; n];
        let mut todo: Vec<u32> = Vec::new();
        for &i in &starts {
            at[i as usize] = Some(match &at[i as usize] {
                Some(s) => s.join(&entry),
                None => entry.clone(),
            });
            todo.push(i);
        }
        while let Some(i) = todo.pop() {
            let s = at[i as usize].clone().unwrap();
            for (j, out) in self.edges(u, i, &s) {
                if j as usize >= n {
                    continue;
                }
                let new = match &at[j as usize] {
                    None => out,
                    Some(old) => {
                        visits[j as usize] += 1;
                        let joined = old.join(&out);
                        if joined == *old {
                            continue;
                        }
                        if visits[j as usize] > WIDEN_AFTER {
                            old.widen(&joined)
                        } else {
                            joined
                        }
                    }
                };
                at[j as usize] = Some(new);
                todo.push(j);
            }
        }
        // narrowing: each round recomputes every instruction's state from its predecessors' (a descending sequence
        // from a fixpoint, so each round still over-approximates)
        for _ in 0..NARROW_ROUNDS {
            let mut next: Vec<Option<State>> = vec![None; n];
            for &i in &starts {
                next[i as usize] = Some(entry.clone());
            }
            for i in 0..n {
                let Some(s) = &at[i] else { continue };
                for (j, out) in self.edges(u, i as u32, s) {
                    if j as usize >= n {
                        continue;
                    }
                    next[j as usize] = Some(match &next[j as usize] {
                        Some(o) => o.join(&out),
                        None => out,
                    });
                }
            }
            at = next;
        }
        at
    }
}

/// The units a start of script `n` from unit `caller` runs (`Game::started`: room code's own room, else any room).
fn callees(game: &Game, caller: UnitKey, n: i64) -> Vec<UnitId> {
    if n <= 0 || n > i32::MAX as i64 {
        return Vec::new();
    }
    let room = match caller {
        UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
        _ => None,
    };
    game.started(n as i32, room).into_iter().filter_map(|k| game.id(k)).collect()
}

impl HavocArgs {
    /// The analysis of every unit of the game, with the variables of `scope`'s table (havoc-args.md, steps 1 to 4).
    pub fn of(game: &Game, scope: &Scope, table: &FactTable) -> HavocArgs {
        let env = Env { game, scope, table, vars: Default::default(), settings: domains::settings_env() };
        let ast = &game.ast;
        let ids: Vec<UnitId> = game.ids().collect();
        // the entries the engine gives: room code none (0), hooks' scripts, the boot and objects' code unknown
        let mut entry: FxHashMap<UnitId, Locals> = FxHashMap::default();
        let mut fixed: FxHashSet<UnitId> = FxHashSet::default();
        for &u in &ids {
            match game.unit(u).key {
                UnitKey::Entry(_) | UnitKey::Exit(_) => {
                    entry.insert(u, vec![V::c(0); NUM_LOCALS]);
                }
                UnitKey::Obj(_) => {
                    entry.insert(u, all_top());
                    fixed.insert(u);
                }
                UnitKey::Global(n) if n == BOOT => {
                    entry.insert(u, all_top());
                    fixed.insert(u);
                }
                _ => {}
            }
        }
        let mut hook_wild = false;
        for h in HOOKS {
            match env.var(h).values(4096) {
                Some(ns) => {
                    for n in ns {
                        for c in callees(game, UnitKey::Global(0), n) {
                            entry.insert(c, all_top());
                            fixed.insert(c);
                        }
                    }
                }
                None => hook_wild = true,
            }
        }
        if hook_wild {
            for &u in &ids {
                entry.insert(u, all_top());
                fixed.insert(u);
            }
        }
        // the fixpoint over launches: a unit is read again when its entry grows
        let mut sites: FxHashMap<(UnitId, u32), State> = FxHashMap::default();
        let mut wild: BTreeSet<(UnitId, u32)> = BTreeSet::new();
        let mut wild_args: Option<Locals> = None;
        let mut todo: BTreeSet<UnitId> = entry.keys().copied().collect();
        let mut rounds = 0u32;
        while !todo.is_empty() {
            rounds += 1;
            let batch: Vec<UnitId> = std::mem::take(&mut todo).into_iter().collect();
            let mut grown: BTreeMap<UnitId, Locals> = BTreeMap::new();
            let mut new_wild: Option<Locals> = None;
            for u in batch {
                let Some(e) = entry.get(&u).cloned() else { continue };
                let at = env.unit(u, &e);
                let key = game.unit(u).key;
                let ins = game.ins(u);
                for (i, s) in at.iter().enumerate() {
                    let Some(s) = s else { continue };
                    let Some(st) = ins[i].st else { continue };
                    if !is_site(&st) {
                        continue;
                    }
                    sites.insert((u, i as u32), s.clone());
                    let Stmt::Start { name, script, args, .. } = st else { continue };
                    if name == known::START_OBJECT {
                        continue;
                    }
                    let passed: Locals = (0..NUM_LOCALS)
                        .map(|k| ast.kids(args).get(k).map_or(V::c(0), |&a| env.eval(a, s)))
                        .collect();
                    match env.eval(script, s).values(4096) {
                        Some(ns) => {
                            for n in ns {
                                for c in callees(game, key, n) {
                                    let g = grown.entry(c).or_insert_with(|| passed.clone());
                                    *g = join_locals(g, &passed);
                                }
                            }
                        }
                        None => {
                            wild.insert((u, i as u32));
                            new_wild = Some(match new_wild {
                                Some(w) => join_locals(&w, &passed),
                                None => passed,
                            });
                        }
                    }
                }
            }
            if let Some(w) = new_wild {
                let w = match &wild_args {
                    Some(o) => join_locals(o, &w),
                    None => w,
                };
                if wild_args.as_ref() != Some(&w) {
                    // a start whose number is not known may start any unit with these arguments
                    for &u in &ids {
                        let g = grown.entry(u).or_insert_with(|| w.clone());
                        *g = join_locals(g, &w);
                    }
                    wild_args = Some(w);
                }
            }
            for (c, passed) in grown {
                if fixed.contains(&c) {
                    continue;
                }
                let new = match entry.get(&c) {
                    Some(old) => join_locals(old, &passed),
                    None => passed,
                };
                if entry.get(&c) != Some(&new) {
                    entry.insert(c, new);
                    todo.insert(c);
                }
            }
        }
        let unreached = ids.iter().filter(|u| !entry.contains_key(u)).count();
        let vars = env.vars.into_inner();
        HavocArgs { sites, vars, unreached, wild: wild.into_iter().collect(), rounds }
    }

    /// The locals at instruction `i` of unit `u` (a start or a room op); None when no launch reaches it.
    pub fn locals_at(&self, u: UnitId, i: u32) -> Option<&Locals> {
        self.sites.get(&(u, i)).map(|s| &s.locals)
    }

    /// The values of expression `e` at instruction `i` of unit `u`, when there are at most `max`; None when not known
    /// or not reached.
    pub fn values_at(&self, game: &Game, scope: &Scope, table: &FactTable, u: UnitId, i: u32, e: ExprId, max: usize) -> Option<BTreeSet<i64>> {
        let s = self.sites.get(&(u, i))?;
        let env = Env { game, scope, table, vars: std::cell::RefCell::new(self.vars.clone()), settings: domains::settings_env() };
        env.eval(e, s).values(max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[i64]) -> V {
        V::Set(v.iter().copied().collect())
    }

    #[test]
    fn a_set_past_its_size_is_a_range_and_a_small_range_a_set() {
        assert_eq!(V::set((0..=64).collect()), V::Range(0, 64));
        assert_eq!(V::range(3, 5), s(&[3, 4, 5]));
        assert_eq!(V::range(LO, HI), V::Top);
        assert_eq!(s(&[1, 2]).join(&s(&[5])), s(&[1, 2, 5]));
        assert_eq!(V::Range(0, 100).join(&s(&[200])), V::Range(0, 200));
    }

    #[test]
    fn arithmetic_on_sets_and_ranges() {
        assert_eq!(V::arith(ArithOp::Add, &s(&[1, 2]), &s(&[10])), s(&[11, 12]));
        assert_eq!(V::arith(ArithOp::Div, &s(&[-7, 7]), &s(&[2])), s(&[-3, 3]));
        assert_eq!(V::arith(ArithOp::Div, &s(&[7]), &s(&[0, 1])), V::Top);
        assert_eq!(V::arith(ArithOp::Add, &V::Range(0, 100), &s(&[396])), V::Range(396, 496));
        assert_eq!(V::arith(ArithOp::Mul, &V::Range(-2, 100), &V::Range(1, 3)), V::Range(-6, 300));
        assert_eq!(V::arith(ArithOp::Add, &V::Top, &s(&[1])), V::Top);
    }

    #[test]
    fn a_condition_narrows_on_each_side() {
        let x = V::Range(0, 0xFFFF);
        assert_eq!(x.refine(CmpOp::Ne, 0), Some(V::Range(1, 0xFFFF)));
        assert_eq!(x.refine(CmpOp::Ne, 0).unwrap().refine(negate(CmpOp::Gt), 12), Some(V::range(1, 12)));
        assert_eq!(x.refine(CmpOp::Lt, 0), None);
        assert_eq!(V::Top.refine(CmpOp::Ge, 5), Some(V::Range(5, HI)));
        assert_eq!(s(&[58, 200]).refine(CmpOp::Ne, 0), Some(s(&[58, 200])));
        assert_eq!(s(&[0]).refine(CmpOp::Ne, 0), None);
    }

    #[test]
    fn widening_sends_a_moving_bound_to_infinity() {
        let old = V::Range(0, 100);
        assert_eq!(old.widen(&V::Range(0, 101)), V::Range(0, HI));
        assert_eq!(old.widen(&V::Range(-1, 50)), V::Range(LO, 100));
        assert_eq!(old.widen(&s(&[5])), old);
        assert_eq!(V::Range(0, HI).refine(CmpOp::Le, 7), Some(V::range(0, 7)));
    }
}
