//! reader.py's machine-dependent half (decisions.md D7): `Reader`, the subclass of model.Machine, with `SymOracle`
//! and `SymPath`. [`Reader`] fills [`builder::Machine`](crate::builder::Machine), so the Builder reads with the
//! Rust machine (scumm-machine) and no Python. Each hook below is the Reader method of the same name in reader.py,
//! in its order of reads, writes, forks and counts; the machine's own methods are the port's
//! (machine/PORT-NOTES.md). PORT-NOTES.md says how Python's mechanisms map:
//!
//! * `Fork` (an exception) is a panic with a [`ForkOn`] payload, thrown by `resume_unwind` (no panic hook runs)
//!   and caught in `advance` (`Reader._advance`), which reruns the step from its snapshot once per value;
//! * `SymPath`'s fields are [`SymX`], the path's `ext`, cloned with the path; the Reader's state is [`St`], the
//!   machine's `ext`; `SymOracle` is the `oget` and `okeys` hooks;
//! * the reader's own keys in the machine's store: `saved:V:I` is `Fact::Class(SAVED + V, I)`, the clear mark
//!   `at:<unit>:*` (`esc:<unit>:*`) is `Fact::At(unit, MARK)` (`Fact::Esc`); a run's precondition keys are
//!   [`PKey`] (facts, and `args:<unit>`);
//! * `Cond` and `Copy` are unknowns of class 1 and 2 (`Unknown::class`); the reader's reasons are
//!   `Reason::Other`;
//! * the runs `run_click_verb` and `run_event` start with a call whose one step is a marked note that the `eng`
//!   hook runs as `_eng_input` (model.py's, without its switch) and `_eng_resume_at`.

use crate::builder::{Ctx, Machine as RunMachine, RdCounts, RunOut, DROP_ENDS};
use crate::domain::{Domain, Label, Labels};
use crate::py::Py;
use crate::record::{Assumption as RAssumption, Event, Kept, PathOut, Pins, Pre, UClass, Unknown as RUnknown, Val as RVal, What};
use crate::scope::{self, Scope};
use crate::table::{FactTable, Kind};
use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use scumm_ir::{known, CmpOp, Expr, ExprId, Fact, FactMap, Game, Reason, Slot, Span, Stmt, Sym, UnitId, UnitKey, Yield};
use scumm_machine::machine::{Hooks, VAR_INVENTORY_SCRIPT, VAR_OVERRIDE, VAR_ROOM, VAR_ROOM_RESOURCE, X};
use scumm_machine::oracle::{FactOracle, Prefix};
use scumm_machine::path::{fmix, Assumption, Call, Entry, Owned, Path, PathExt, Run, Side, Sig, Step, S};
use scumm_machine::value::{i16w, Dec, Op, Unknown, Val};
use scumm_machine::{Features, Machine, Switches};
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::Hasher;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

/// reader.py `PATH_LIMIT`: paths per run.
pub const PATH_LIMIT: usize = 1024;
const STEP_BIG: i32 = 1 << 20;
pub const R_FACT: Reason = Reason::Other("fact");
pub const R_NONFACT: Reason = Reason::Other("not a fact");
pub const R_COLLAPSED: Reason = Reason::Other("collapsed value");
pub const R_STEP: Reason = Reason::Other("step");
pub const R_COND: Reason = Reason::Other("conditional write");
pub const R_COPY: Reason = Reason::Other("copy");
/// `Unknown::class` of reader.py's `Cond` and `Copy`
pub const COND: u8 = 1;
pub const COPY: u8 = 2;
/// the `w` of a clear mark (`at:<unit>:*`)
const MARK: i32 = i32::MIN;
/// `saved:V:I` is `Fact::Class(SAVED + V, I)`
const SAVED: i32 = i32::MIN;
/// the steps of the calls `run_click_verb` and `run_event` start with (model.py's `("input", ...)`, reader.py's
/// `("resume_at", ev)`)
const INPUT_STEP: &str = "\u{1}reader: _eng_input";
const RESUME_STEP: &str = "\u{1}reader: _eng_resume_at";
const VERB_CLICK_AREA: i32 = 1; // model.py, verbs.h:35
const LEFT_CLICK: i32 = 1; // model.py, verbs.cpp:666
const VAR_VERB_SCRIPT: i32 = 32;
const VAR_USERPUT: i32 = 53;
const VAR_CURSORSTATE: i32 = 52;
const ESC_KEY: [&str; 3] = ["runInputScript", "input.cpp:1427-1428", "the Esc key code: checkExecVerbs and the input script, not run"];
/// reader.py `MEMO_GETTERS`
const MEMO_GETTERS: [(&str, &[&str]); 10] = [
    ("getActorRoom", &["putactor", "loadroom"]),
    ("getActorX", &["actor", "loadroom"]),
    ("getActorY", &["actor", "loadroom"]),
    ("getActorWalkBox", &["actor", "loadroom"]),
    ("getActorElevation", &["actor", "loadroom"]),
    ("getDist", &["actor", "loadroom", "drawobject"]),
    ("getActorCostume", &["actor", "loadroom", "animate"]),
    ("getActorFacing", &["actor", "loadroom", "animate"]),
    ("getActorMoving", &["actor", "loadroom", "animate"]),
    ("getActorWidth", &["actor", "loadroom"]),
];

fn memo_subs(getter: &str) -> Option<&'static [&'static str]> {
    MEMO_GETTERS.iter().find(|g| g.0 == getter).map(|g| g.1)
}

// --- keys ---------------------------------------------------------------------------------------------------------

/// A key of a path's assumptions (`SymPath.pre`): a fact, or a slot's `args:<unit>` (the reader's own).
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum PKey {
    F(Fact),
    Args(Slot),
}

fn saved_fact(v: i32, id: i32) -> Fact {
    Fact::Class(SAVED.wrapping_add(v), id)
}

fn saved_of(f: Fact) -> Option<(i32, i32)> {
    match f {
        Fact::Class(o, i) if o < SAVED / 2 => Some((o.wrapping_sub(SAVED), i)),
        _ => None,
    }
}

/// The key's facts.py (or reader.py) string.
pub fn key_str(f: Fact) -> String {
    match f {
        Fact::At(s, MARK) => format!("at:{s}:*"),
        Fact::Esc(s, MARK) => format!("esc:{s}:*"),
        _ => match saved_of(f) {
            Some((v, i)) => format!("saved:{v}:{i}"),
            None => f.to_string(),
        },
    }
}

/// model.py `family`.
fn family(f: Fact) -> String {
    if saved_of(f).is_some() {
        "saved".into()
    } else {
        f.family()
    }
}

fn slot_of(name: &str) -> Slot {
    match format!("frozen:{name}").parse::<Fact>() {
        Ok(Fact::Frozen(s)) => s,
        _ => panic!("not a unit name: {name}"),
    }
}

fn pkey_of(s: &str) -> PKey {
    if let Some(name) = s.strip_prefix("args:") {
        return PKey::Args(slot_of(name));
    }
    if let Some(rest) = s.strip_prefix("saved:") {
        let (v, i) = rest.split_once(':').expect("saved:V:I");
        return PKey::F(saved_fact(v.parse().unwrap(), i.parse().unwrap()));
    }
    PKey::F(s.parse().unwrap_or_else(|_| panic!("a precondition key the machine half cannot hold: {s}")))
}

fn pkey_str(k: PKey) -> String {
    match k {
        PKey::F(f) => key_str(f),
        PKey::Args(s) => format!("args:{s}"),
    }
}

/// A Python reason string as the machine's `Reason`.
fn reason_of(s: &str) -> Reason {
    use Reason::*;
    let all = [
        Random, Position, Actor, Timer, Local, Held, Work, Sound, String, Inventory, Verb, Message, Sentence, Key, Freeze,
        CutData, Cache, Branch, LoopExit, WaitOver, Settle, Camera, Input, Counter, R_FACT, R_NONFACT, R_COLLAPSED,
        R_STEP, R_COND, R_COPY,
    ];
    if let Some(r) = all.iter().find(|r| r.as_str() == s) {
        return *r;
    }
    match s.strip_prefix("var ").and_then(|n| n.parse().ok()) {
        Some(n) => Var(n),
        None => panic!("a reason the machine does not know: {s}"),
    }
}

// --- the Reader's state and the SymPath's fields --------------------------------------------------------------------

/// How the reader reads a fact (`FactTable.kind`, `domain`, `start`), worked out once per pass.
struct Info {
    key: Rc<str>,
    kind: Kind,
    dom: Option<Domain>,
    start: i64,
}

/// A comparison of `Reader.test`'s kept tests.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum COp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl COp {
    fn of(op: CmpOp) -> COp {
        match op {
            CmpOp::Eq => COp::Eq,
            CmpOp::Ne => COp::Ne,
            CmpOp::Lt => COp::Lt,
            CmpOp::Le => COp::Le,
            CmpOp::Gt => COp::Gt,
            CmpOp::Ge => COp::Ge,
        }
    }
    /// reader.py `FLIP`
    fn flip(self) -> COp {
        match self {
            COp::Lt => COp::Gt,
            COp::Le => COp::Ge,
            COp::Gt => COp::Lt,
            COp::Ge => COp::Le,
            o => o,
        }
    }
    fn negate(self) -> COp {
        match self {
            COp::Eq => COp::Ne,
            COp::Ne => COp::Eq,
            COp::Lt => COp::Ge,
            COp::Le => COp::Gt,
            COp::Gt => COp::Le,
            COp::Ge => COp::Lt,
        }
    }
}

/// `_memo_ident`'s mode: "i16" (a comparison) or "full" (a zero test).
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum Mode {
    I16,
    Full,
}

/// (reason, what, mode) of a non-fact value one frame keeps.
type Ident = (Reason, Rc<str>, Mode);
/// ((op, constant), side)
type Test = (COp, i64, bool);

/// A slot a path left waiting (`SymPath.kept`'s tuple).
#[derive(Clone, Debug)]
struct KeptS {
    unit: UnitId,
    w: i32,
    locals: Vec<(i32, Val)>,
    known: bool,
    rec: Option<bool>,
    res: Option<bool>,
    owned: Owned,
    ov: i32,
    cd: Vec<Val>,
}

/// reader.py `SymPath`'s fields (the path's `ext`). The maps are shared by forks until one changes them.
#[derive(Clone, Debug, Default)]
pub struct SymX {
    pre: Rc<BTreeMap<PKey, Labels>>,
    kept: Rc<BTreeMap<Slot, KeptS>>,
    /// unit name -> (freezes not yet undone, `frozen:` before: None for "unwritten")
    fz: Rc<BTreeMap<Slot, (i32, Option<Val>)>>,
    nf: Rc<BTreeMap<Ident, Vec<Test>>>,
    resume_wait: Option<(UnitId, i32)>,
}

impl PathExt for SymX {
    fn clone_box(&self) -> Box<dyn PathExt> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The payload of reader.py's `Fork`: the fact a step read with more than one value left, and (fix round 1, F12)
/// the value sets to fork into where a comparison alone reads it (None: one path per value).
pub struct ForkOn(pub Fact, pub Option<Vec<Labels>>);

/// The Reader's state (the machine's `ext`).
#[derive(Default)]
struct St {
    table: Option<FactTable>,
    scope: Option<Scope>,
    info: FxHashMap<Fact, Rc<Info>>,
    /// the universe's slot keys by prefix, in its order (`SymOracle.keys`)
    uni: FxHashMap<Prefix, Vec<Fact>>,
    uni_set: FxHashSet<Fact>,
    /// `{game.resource(r) for r in scope.rooms}`
    scope_rooms: Vec<Option<i32>>,
    hook_writes: FxHashMap<i32, Rc<[Fact]>>,
    // the run
    pins: FxHashMap<Fact, Val>,
    /// the pins' keys the universe lacks, string-sorted
    pin_extra: Vec<Fact>,
    seed: Rc<BTreeMap<PKey, Labels>>,
    event: Option<Event>,
    verb: i32,
    first_fork: Option<String>,
    rd: RdCounts,
    // inside a step
    lazy: u32,
    step_key: Option<Fact>,
    pending: Option<(Ident, (COp, i64))>,
    view: Option<Rc<BTreeMap<PKey, Labels>>>,
    /// F12: the instruction the step being run runs (None: an engine step)
    cur: Option<X>,
}

fn st<'a>(m: &'a Machine<'_>) -> &'a St {
    m.ext.as_deref().expect("the Reader's state").downcast_ref::<St>().unwrap()
}

fn st_mut<'a>(m: &'a mut Machine<'_>) -> &'a mut St {
    m.ext.as_deref_mut().expect("the Reader's state").downcast_mut::<St>().unwrap()
}

fn sx(p: &Path) -> &SymX {
    p.ext.as_deref().expect("a SymPath").as_any().downcast_ref::<SymX>().unwrap()
}

fn sx_mut(p: &mut Path) -> &mut SymX {
    p.ext.as_deref_mut().expect("a SymPath").as_any_mut().downcast_mut::<SymX>().unwrap()
}

fn count(m: &mut Machine<'_>, what: &str) {
    st_mut(m).rd.counts.inc(what);
}

fn lazily<R>(m: &mut Machine<'_>, f: impl FnOnce(&mut Machine<'_>) -> R) -> R {
    st_mut(m).lazy += 1;
    let r = f(m);
    st_mut(m).lazy -= 1;
    r
}

fn fork(k: Fact) -> ! {
    panic::resume_unwind(Box::new(ForkOn(k, None)))
}

/// F12: a read of fact `k`, whose values left are `cands`, that only a comparison with a constant (or a zero test)
/// of the step's jump reads: the value sets the comparison separates. None where the step uses the value otherwise
/// (a copy, an object or script number, an index, arithmetic: one path per value).
fn compare_sets(m: &mut Machine<'_>, k: Fact, cands: &[Label]) -> Option<Vec<Labels>> {
    let x = st(m).cur?;
    let g = m.game;
    let Some(Stmt::Jump { cond: Some(c), .. }) = g.ins(x.u)[x.i as usize].st else { return None };
    let ast = &g.ast;
    let reads = |e: ExprId| -> bool {
        match ast.expr(e) {
            Expr::Var(n) => k == Fact::Var(n) || (n == VAR_ROOM && k == Fact::Room),
            Expr::Bit(n) => k == Fact::Bit(n),
            Expr::Getter { name, args } => {
                let o = ast.kids(args).first().and_then(|&a| ast.constant(a));
                match (name, o) {
                    (known::GET_OBJECT_STATE, Some(o)) => k == Fact::State(o),
                    (known::GET_OBJECT_OWNER, Some(o)) => k == Fact::Owner(o),
                    _ => false,
                }
            }
            _ => false,
        }
    };
    let cmp = |op: CmpOp, y: i64, z: i64| {
        let (y, z) = (i16w(y as i32), i16w(z as i32));
        match op {
            CmpOp::Eq => y == z,
            CmpOp::Ne => y != z,
            CmpOp::Lt => y < z,
            CmpOp::Le => y <= z,
            CmpOp::Gt => y > z,
            CmpOp::Ge => y >= z,
        }
    };
    let test: Box<dyn Fn(i64) -> bool> = match ast.expr(c) {
        Expr::Cmp { op, a, b } => match (reads(a), ast.constant(b), reads(b), ast.constant(a)) {
            (true, Some(c), _, _) => Box::new(move |v| cmp(op, v, c as i64)),
            (_, _, true, Some(c)) => Box::new(move |v| cmp(op, c as i64, v)),
            _ => return None,
        },
        Expr::Not(a) if reads(a) => Box::new(|v| v == 0),
        Expr::Truth(a) if reads(a) => Box::new(|v| v != 0),
        _ => return None,
    };
    let i = info(m, k);
    let d = i.dom.as_ref()?;
    let val = |l: Label| match l {
        Label::Int(n) => n,
        Label::Above => d.high.unwrap() + 1,
        Label::Below => d.low.unwrap() - 1,
    };
    let (yes, no): (Labels, Labels) = cands.iter().partition(|&&l| test(val(l)));
    Some([yes, no].into_iter().filter(|s| !s.is_empty()).collect())
}

/// F12 at a fork on read: a comparison alone reads the fact. One value set left: its value set, no fork (the test
/// decides on it); more: a fork into them.
fn read_by_sets(m: &mut Machine<'_>, k: Fact, cands: &[Label]) -> Option<Val> {
    if !st(m).table.as_ref().unwrap().value_sets {
        return None;
    }
    let Some(sets) = compare_sets(m, k, cands) else {
        let at = match st(m).cur.and_then(|x| m.game.ins(x.u)[x.i as usize].st) {
            None => "an engine step",
            Some(Stmt::Jump { .. }) => "a jump's other test",
            Some(Stmt::Assign(_)) => "an assignment",
            Some(Stmt::Start { .. }) => "a script start",
            Some(Stmt::Op { .. }) => "an op",
        };
        count(m, &format!("F12: fact reads forked per value: {at}"));
        return None;
    };
    if sets.len() == 1 {
        count(m, "F12: fact reads a comparison alone reads, decided on their value set (no fork)");
        let i = info(m, k);
        return Some(lazy_of(m, &i, Some(cands)));
    }
    count(m, "F12: forks on read into value sets");
    panic::resume_unwind(Box::new(ForkOn(k, Some(sets))))
}

fn info(m: &mut Machine<'_>, k: Fact) -> Rc<Info> {
    let s = st_mut(m);
    if let Some(i) = s.info.get(&k) {
        return i.clone();
    }
    let key = key_str(k);
    let table = s.table.as_ref().unwrap();
    let kind = table.kind(&key, s.scope.as_ref().unwrap());
    let dom = table.domain(&key).map(|d| d.into_owned());
    let start = table.start.get0(&key);
    let i = Rc::new(Info { key: key.into(), kind, dom, start });
    s.info.insert(k, i.clone());
    i
}

fn mk_unknown(m: &mut Machine<'_>, reason: Reason, what: &str, values: Option<Vec<i32>>, class: u8) -> Val {
    let values = values.map(|mut v| {
        v.sort_unstable();
        v.dedup();
        v.into_boxed_slice()
    });
    m.vals().intern(Unknown { reason, what: what.into(), values, class })
}

/// `FactTable.value(key, label)`.
fn value(m: &mut Machine<'_>, i: &Info, l: Label) -> Val {
    match l {
        Label::Int(n) => Val::Int(n as i32),
        _ => {
            let d = i.dom.as_ref().unwrap();
            let rep = if l == Label::Above { d.high.unwrap() + 1 } else { d.low.unwrap() - 1 };
            mk_unknown(m, R_COLLAPSED, &i.key, Some(vec![rep as i32]), 0)
        }
    }
}

/// `FactTable.lazy(key, labels)`.
fn lazy_of(m: &mut Machine<'_>, i: &Info, labels: Option<&[Label]>) -> Val {
    let d = i.dom.as_ref().unwrap();
    let labels = labels.unwrap_or(&d.values);
    let vals: Vec<i32> = labels
        .iter()
        .map(|&l| match l {
            Label::Int(n) => n as i32,
            Label::Above => (d.high.unwrap() + 1) as i32,
            Label::Below => (d.low.unwrap() - 1) as i32,
        })
        .collect();
    mk_unknown(m, R_FACT, &i.key, Some(vals), 0)
}

/// reader.py `STEP0`.
fn step0(m: &mut Machine<'_>) -> Val {
    mk_unknown(m, R_STEP, "", Some(vec![0, STEP_BIG]), 0)
}

fn is_step(m: &Machine<'_>, v: Val) -> bool {
    matches!(m.vals_ref().unk(v), Some(u) if u.reason == R_STEP)
}

fn step_of(m: &Machine<'_>, v: Val) -> i64 {
    m.vals_ref().unk(v).unwrap().values.as_ref().unwrap()[0] as i64
}

/// `Reader._stepped`.
fn stepped(m: &mut Machine<'_>, k: Fact, base: Label, d: i64) -> Val {
    if let Label::Int(n) = base {
        return Val::Int((n + d) as i32);
    }
    count(m, "steps on a collapsed value (a guess)");
    let i = info(m, k);
    let vals = i.dom.as_ref().unwrap().add(base, d);
    if vals.len() > 1 {
        lazy_of(m, &i, Some(&vals))
    } else {
        value(m, &i, vals[0])
    }
}

/// A plain unknown's (reason, what).
fn plain(m: &Machine<'_>, v: Val) -> Option<(Reason, String)> {
    match m.vals_ref().unk(v) {
        Some(u) if u.values.is_none() => Some((u.reason, u.what.to_string())),
        _ => None,
    }
}

fn class_of(m: &Machine<'_>, v: Val) -> Option<u8> {
    m.vals_ref().unk(v).map(|u| u.class)
}

fn call_at(p: &Path, ci: usize) -> (S, S) {
    match &p.stack[ci] {
        Entry::Call(c) => (c.head.clone(), c.cite.clone()),
        Entry::Frame(_) => panic!("stack entry {ci} is a frame"),
    }
}

/// `("sym", c)` in `Path.freeze` (a freeze `Reader._freeze` recorded; no count).
fn sym_count(c: i32) -> (i32, bool) {
    (i32::MIN.wrapping_add(c), false)
}

fn is_sym_count(n: Option<(i32, bool)>) -> bool {
    matches!(n, Some((c, false)) if c < i32::MIN / 2)
}

/// reader.py `_consistent`.
fn consistent(tests: &[Test]) -> bool {
    let (mut lo, mut hi) = (-(1i64 << 40), 1i64 << 40);
    let mut eq = BTreeSet::new();
    let mut ne = BTreeSet::new();
    for &(op, k, side) in tests {
        let op = if side { op } else { op.negate() };
        match op {
            COp::Eq => {
                eq.insert(k);
            }
            COp::Ne => {
                ne.insert(k);
            }
            COp::Lt => hi = hi.min(k - 1),
            COp::Le => hi = hi.min(k),
            COp::Gt => lo = lo.max(k + 1),
            COp::Ge => lo = lo.max(k),
        }
    }
    if let Some(&e) = eq.iter().next() {
        return eq.len() == 1 && lo <= e && e <= hi && !ne.contains(&e);
    }
    hi - lo + 1 > ne.iter().filter(|&&k| lo <= k && k <= hi).count() as i64
}

/// `Reader._memo_ident`.
fn memo_ident(m: &Machine<'_>, v: Val, mode: Mode) -> Option<Ident> {
    if m.vals_ref().range(v).is_some() {
        return None; // F12: a range is narrowed on the path, not kept as a test
    }
    let u = m.vals_ref().unk(v)?;
    if u.values.is_some() {
        return None;
    }
    if u.reason == R_NONFACT {
        if let Some(n) = u.what.strip_prefix("var:") {
            let n: i32 = n.parse().unwrap();
            if !st(m).table.as_ref().unwrap().engine.contains(&n) {
                return Some((u.reason, Rc::from(&*u.what), mode));
            }
        }
    }
    if matches!(u.reason, Reason::Actor | Reason::Position)
        && u.what.contains('(')
        && memo_subs(u.what.split('(').next().unwrap()).is_some()
    {
        return Some((u.reason, Rc::from(&*u.what), mode));
    }
    None
}

/// `Reader._op_at`: the name of the op the path's next step runs.
fn op_at(m: &Machine<'_>, p: &Path) -> Option<&'static str> {
    let Some(Entry::Frame(f)) = p.stack.last() else { return None };
    let g = m.game;
    if f.unit == scumm_machine::path::NONE_UNIT {
        return None;
    }
    let pc = f.pc?;
    if pc >= g.unit(f.unit).ins_len {
        return None;
    }
    match g.ins(f.unit)[pc as usize].st {
        Some(Stmt::Op { name, .. }) | Some(Stmt::Start { name, .. }) => Some(static_str(g.syms.str(name))),
        _ => None,
    }
}

/// An op name as a static string (the names are few; leaked once each).
fn static_str(s: &str) -> &'static str {
    thread_local! {
        static NAMES: std::cell::RefCell<FxHashMap<String, &'static str>> = Default::default();
    }
    NAMES.with(|n| {
        let mut n = n.borrow_mut();
        if let Some(&v) = n.get(s) {
            return v;
        }
        let v: &'static str = Box::leak(s.to_string().into_boxed_str());
        n.insert(s.to_string(), v);
        v
    })
}

/// `Reader._forget`.
fn forget(m: &mut Machine<'_>, q: &mut Path, op: Option<&str>, var_writes: bool) {
    let name = op.unwrap_or("").to_lowercase();
    let idents: Vec<Ident> = sx(q).nf.keys().cloned().collect();
    for ident in idents {
        let what = &*ident.1;
        let getter = what.split('(').next().unwrap();
        let hit = memo_subs(getter).is_some_and(|subs| !name.is_empty() && subs.iter().any(|s| name.contains(s)))
            || (var_writes && what.starts_with("var:"));
        if hit {
            Rc::make_mut(&mut sx_mut(q).nf).remove(&ident);
            count(m, "non-fact tests forgotten after an op that may change them");
        }
    }
}

/// `Reader.read` (the hook's body; the hook adds F9's not-known actor rooms, `actor_room_value`).
fn read_fact(m: &mut Machine<'_>, p: &mut Path, k: Fact, o: &FactOracle) -> Val {
    let stepping = st(m).step_key == Some(k);
    if let Some(&v) = p.writes.get(&k) {
        if !stepping && is_step(m, v) {
            // a counter the path stepped is read: its value is needed now
            let base = sx(p).pre.get(&PKey::F(k)).filter(|b| b.len() == 1).map(|b| b[0]);
            match base {
                Some(b) => {
                    let d = step_of(m, v);
                    return stepped(m, k, b, d);
                }
                None => fork(k),
            }
        }
        return v;
    }
    match k {
        Fact::At(s @ (Slot::Entry | Slot::Exit), _) => {
            if let Some(&v) = p.writes.get(&Fact::At(s, MARK)) {
                return v;
            }
        }
        Fact::Esc(s @ (Slot::Entry | Slot::Exit), _) => {
            if let Some(&v) = p.writes.get(&Fact::Esc(s, MARK)) {
                return v;
            }
        }
        _ => {}
    }
    if let Some(s) = sx(p).pre.get(&PKey::F(k)) {
        if s.len() == 1 {
            let l = s[0];
            let i = info(m, k);
            return value(m, &i, l);
        }
        if stepping {
            return step0(m);
        }
        if st(m).lazy > 0 {
            let s = s.clone();
            let i = info(m, k);
            return lazy_of(m, &i, Some(&s));
        }
        let s = s.clone();
        if let Some(v) = read_by_sets(m, k, &s) {
            return v;
        }
        fork(k);
    }
    if let Some(&v) = st(m).pins.get(&k) {
        return v;
    }
    if k == Fact::Var(VAR_ROOM) {
        // the engine writes it with the room (room.cpp:149, 155): the same fact
        return m.read(p, Fact::Room, o);
    }
    if k == Fact::Var(VAR_ROOM_RESOURCE) {
        let r = m.read(p, Fact::Room, o);
        return m.h_resource(r);
    }
    let i = info(m, k);
    match i.kind {
        Kind::Const => Val::Int(i.start as i32),
        Kind::Fact => {
            let d = i.dom.as_ref().unwrap();
            if d.values.len() == 1 {
                return value(m, &i, d.values[0]);
            }
            if stepping {
                return step0(m);
            }
            if st(m).lazy > 0 {
                return lazy_of(m, &i, None);
            }
            let all = d.values.clone();
            if let Some(v) = read_by_sets(m, k, &all) {
                return v;
            }
            fork(k)
        }
        Kind::NonFact => {
            if let (true, Fact::Var(n @ (20 | 21 | 44 | 45))) = (st(m).table.as_ref().unwrap().value_sets, k) {
                // F12: the mouse is a range (scumm-machine `mouse_range`), in the scope's rooms
                let rooms: Vec<i32> = st(m).scope_rooms.iter().flatten().copied().collect();
                let (lo, hi) = m.mouse_range(n, Some(&rooms));
                return m.vals().mk_range(R_NONFACT, &i.key, lo, hi);
            }
            m.vals().mk(R_NONFACT, &*i.key)
        }
    }
}

/// F9 (switch actor_rooms): an actor's room the reading does not know (crate::table::NOT_KNOWN, a lazy set holding it, or
/// the room of an actor whose room is no fact) reads as getActorRoom's non-fact, named as `Reader.getter` names it
/// with the switch off, so the frame's non-fact tests are remembered as before (`memo_ident`).
fn actor_room_value(m: &mut Machine<'_>, a: i32, v: Val) -> Val {
    let not_known = match v {
        Val::Int(n) => n as i64 == crate::table::NOT_KNOWN,
        Val::Unk(_) => {
            let u = m.vals_ref().unk(v).unwrap();
            match &u.values {
                Some(vs) => vs.iter().any(|&n| n as i64 == crate::table::NOT_KNOWN),
                None => u.reason == R_NONFACT && *u.what == *format!("actorroom:{a}"),
            }
        }
    };
    if not_known {
        m.vals().mk(Reason::Actor, format!("getActorRoom({a})"))
    } else {
        v
    }
}

// --- the hooks: reader.py's Reader methods ---------------------------------------------------------------------------

/// reader.py `Reader`'s overrides of model.Machine.
pub struct ReaderHooks;

static HOOKS: ReaderHooks = ReaderHooks;

#[allow(clippy::too_many_arguments)]
impl Hooks for ReaderHooks {
    /// `Reader._advance`: the step runs on a snapshot; a read of a fact with several values left (`Fork`) runs it
    /// again from the snapshot once per value.
    fn advance(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle) -> Vec<Path> {
        if p.ext.is_none() {
            // `Reader._drive`'s `_to_sym`: the first path of a run_sentence (whose call is no join)
            p.ext = Some(sym_path(st(m)));
        }
        let snap = p.clone();
        let op = if sx(p).nf.is_empty() { None } else { op_at(m, p) };
        st_mut(m).cur = match p.stack.last() {
            Some(Entry::Frame(f)) if f.unit != scumm_machine::path::NONE_UNIT => {
                f.pc.filter(|&pc| pc < m.game.unit(f.unit).ins_len).map(|pc| X { u: f.unit, i: pc })
            }
            _ => None,
        };
        let unknown = p.unknown_writes.len();
        let res = panic::catch_unwind(AssertUnwindSafe(|| m.advance_base(p, o)));
        st_mut(m).cur = None; // a read outside a step is not the step's
        let mut more = match res {
            Ok(more) => more,
            Err(e) => {
                let (key, sets) = match e.downcast::<ForkOn>() {
                    Ok(f) => (f.0, f.1),
                    Err(e) => panic::resume_unwind(e),
                };
                // the `finally` blocks of the methods the Fork went through
                let s = st_mut(m);
                s.lazy = 0;
                s.step_key = None;
                s.pending = None;
                s.view = None;
                return on_fork(m, p, snap, key, sets);
            }
        };
        let now = p.unknown_writes.len();
        if op.is_some() || now != unknown {
            if !sx(p).nf.is_empty() {
                forget(m, p, op, now != unknown);
            }
            for q in more.iter_mut() {
                if q.ext.is_some() && !sx(q).nf.is_empty() {
                    let w = q.unknown_writes.len() != unknown;
                    forget(m, q, op, w);
                }
            }
        }
        more
    }

    fn read(&self, m: &mut Machine<'_>, p: &mut Path, k: Fact, o: &FactOracle) -> Val {
        let v = read_fact(m, p, k, o);
        match k {
            Fact::ActorRoom(a) => actor_room_value(m, a, v),
            _ => v,
        }
    }

    /// Fix round 1: a read that does not fork (the machine's `read_soft`): the fact's values as a set, lazily.
    fn read_soft(&self, m: &mut Machine<'_>, p: &mut Path, k: Fact, o: &FactOracle) -> Val {
        lazily(m, |m| m.read(p, k, o))
    }

    /// `SymOracle.get`.
    fn oget(&self, m: &mut Machine<'_>, _o: &FactOracle, k: Fact) -> Val {
        if let Some(&v) = st(m).pins.get(&k) {
            return v;
        }
        let view = st(m).view.clone();
        if let Some(s) = view.as_ref().and_then(|v| v.get(&PKey::F(k))) {
            if s.len() == 1 {
                let i = info(m, k);
                return value(m, &i, s[0]);
            }
        }
        let i = info(m, k);
        match i.kind {
            Kind::Const => Val::Int(i.start as i32),
            Kind::Fact => {
                let labels = view.as_ref().filter(|v| !v.is_empty()).and_then(|v| v.get(&PKey::F(k))).cloned();
                lazy_of(m, &i, labels.as_deref())
            }
            Kind::NonFact => m.vals().mk(R_NONFACT, &*i.key),
        }
    }

    /// `SymOracle.keys`: the universe's keys, then the pins' the universe lacks.
    fn okeys(&self, m: &Machine<'_>, _o: &FactOracle, pre: Prefix) -> Vec<Fact> {
        let s = st(m);
        let mut out = s.uni.get(&pre).cloned().unwrap_or_default();
        out.extend(s.pin_extra.iter().copied().filter(|&k| pre.matches(k)));
        out
    }

    /// `Reader.val`: a negated non-fact gets a name of its own.
    fn val(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, e: ExprId, x: Option<X>) -> Val {
        let v = m.val_base(p, o, e, x);
        if matches!(m.game.ast.expr(e), Expr::Neg(_)) && m.vals_ref().range(v).is_none() {
            if let Some((r, what)) = plain(m, v) {
                return m.vals().mk(r, format!("-({what})"));
            }
        }
        v
    }

    /// `Reader.test`: a test on a non-fact value the path already tested in this frame.
    fn test(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, c: ExprId, x: Option<X>) -> Dec {
        st_mut(m).pending = None;
        let (out, pred, mode) = match m.game.ast.expr(c) {
            Expr::Cmp { op, a, b } => {
                let va = m.val(p, o, a, x);
                let vb = m.val(p, o, b, x);
                let out = m.vals().decide(&[va, vb], |v| {
                    let (y, z) = (i16w(v[0]), i16w(v[1]));
                    match op {
                        CmpOp::Eq => y == z,
                        CmpOp::Ne => y != z,
                        CmpOp::Lt => y < z,
                        CmpOp::Le => y <= z,
                        CmpOp::Gt => y > z,
                        CmpOp::Ge => y >= z,
                    }
                });
                let pred = match (out, va, vb) {
                    (Dec::Unk(u), _, Val::Int(b)) if u == va => Some((COp::of(op), i16w(b) as i64)),
                    (Dec::Unk(u), Val::Int(a), _) if u == vb => Some((COp::of(op).flip(), i16w(a) as i64)),
                    _ => None,
                };
                (out, pred, Mode::I16)
            }
            Expr::Not(a) => {
                let v = m.val(p, o, a, x);
                let out = m.vals().decide(&[v], |y| y[0] == 0);
                (out, (out == Dec::Unk(v)).then_some((COp::Eq, 0)), Mode::Full)
            }
            Expr::Truth(a) => {
                let v = m.val(p, o, a, x);
                let out = m.vals().decide(&[v], |y| y[0] != 0);
                (out, (out == Dec::Unk(v)).then_some((COp::Ne, 0)), Mode::Full)
            }
            _ => return m.test_base(p, o, c, x),
        };
        let Dec::Unk(u) = out else { return out };
        let Some(pred) = pred else { return out };
        let Some(ident) = memo_ident(m, u, mode) else { return out };
        let have: Vec<Test> = sx(p).nf.get(&ident).cloned().unwrap_or_default();
        let sides: Vec<bool> = [true, false]
            .into_iter()
            .filter(|&s| {
                let mut t = have.clone();
                t.push((pred.0, pred.1, s));
                consistent(&t)
            })
            .collect();
        if sides.len() == 1 {
            count(m, "non-fact tests decided by the same test earlier in the frame");
            return Dec::Bool(sides[0]);
        }
        st_mut(m).pending = Some((ident, pred));
        out
    }

    /// `Reader.getter`: a MEMO_GETTERS getter whose arguments are known names them.
    fn getter(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, name: Sym, args: Span, x: Option<X>) -> Val {
        let v = m.getter_base(p, o, name, args, x);
        let g = m.game;
        let nm = g.syms.str(name);
        if memo_subs(nm).is_none() {
            return v;
        }
        let Some((r, what)) = plain(m, v) else { return v };
        if what != nm {
            return v;
        }
        let mut vals = Vec::new();
        for &a in g.ast.kids(args) {
            if !matches!(g.ast.expr(a), Expr::List(_)) {
                vals.push(m.val(p, o, a, x));
            }
        }
        if vals.iter().all(|v| v.int().is_some()) {
            let parts: Vec<String> = vals.iter().map(|v| v.int().unwrap().to_string()).collect();
            return m.vals().mk(r, format!("{nm}({})", parts.join(", ")));
        }
        v
    }

    /// `Reader._owned_by`: the inventory is not enumerated.
    fn owned_by(&self, m: &mut Machine<'_>, _p: &mut Path, _o: &FactOracle, owner: i32) -> Result<Vec<i32>, (Fact, Val)> {
        count(m, "inventory reads left unknown (getInventoryCount, findInventory)");
        let u = m.vals().mk(Reason::Inventory, "the inventory is not enumerated by the reader");
        Err((Fact::Owner(owner), u))
    }

    /// model.py `_find_inventory` with `Reader._owned_by`'s `(None, unknown)`: the unknown.
    fn find_inventory(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, args: Span, x: Option<X>) -> Val {
        let ks = m.game.ast.kids(args);
        assert_eq!(ks.len(), 2, "findInventory takes two values");
        let (a0, a1) = (ks[0], ks[1]);
        let owner = m.val(p, o, a0, x);
        let n = m.val(p, o, a1, x);
        let (Val::Int(ow), Val::Int(_)) = (owner, n) else {
            let w = format!("findInventory({},{})", m.vals_ref().show(owner), m.vals_ref().show(n));
            return m.vals().mk(Reason::Inventory, w);
        };
        match m.owned_by(p, o, ow) {
            Err((_, v)) => v,
            Ok(_) => unreachable!("the reader's _owned_by never lists"),
        }
    }

    /// `Reader.clear`: one mark for a prefix with two colons (`at:entry:`, `esc:exit:`), else lazily.
    fn clear(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, pre: Prefix) {
        let mark = match pre {
            Prefix::At(s @ (Slot::Entry | Slot::Exit)) => Some(Fact::At(s, MARK)),
            Prefix::Esc(s @ (Slot::Entry | Slot::Exit)) => Some(Fact::Esc(s, MARK)),
            _ => None,
        };
        match mark {
            Some(mk) => {
                let keys: Vec<Fact> = p.writes.keys().copied().filter(|&k| pre.matches(k)).collect();
                for k in keys {
                    p.writes.remove(&k);
                }
                m.h_write(p, mk, Val::Int(0));
            }
            None => lazily(m, |m| m.clear_base(p, o, pre)),
        }
    }

    /// `Reader._arith_at`: a value computed from a non-fact gets a name of its own.
    fn arith_at(&self, m: &mut Machine<'_>, p: &mut Path, op: Op, a: Val, b: Val, x: Option<X>) -> Val {
        let out = m.arith_at_base(p, op, a, b, x);
        if m.vals_ref().range(out).is_some() {
            return out; // F12: a range keeps its bounds
        }
        if let Some((r, what)) = plain(m, out) {
            let ops = match op {
                Op::Add => "+",
                Op::Sub => "-",
                Op::Mul => "*",
                Op::Div => "/",
            };
            return m.vals().mk(r, format!("({what}) {ops} ..."));
        }
        out
    }

    /// `Reader._jump`: a fork on a non-fact test `test` keeps records the side each path took.
    fn jump(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, x: X, cond: Option<ExprId>, idx: Option<u32>) -> Vec<Path> {
        st_mut(m).pending = None;
        let mut more = m.jump_base(p, o, x, cond, idx);
        let pend = st_mut(m).pending.take();
        if !more.is_empty() {
            count(m, "forks on non-fact tests");
        }
        if let Some((ident, (op, k))) = pend {
            if more.len() == 1 && more[0].ext.is_some() {
                Rc::make_mut(&mut sx_mut(p).nf).entry(ident.clone()).or_default().push((op, k, true));
                Rc::make_mut(&mut sx_mut(&mut more[0]).nf).entry(ident).or_default().push((op, k, false));
            }
        }
        more
    }

    /// `Reader._wait`: the wait an event resumes at falls through; any other blocks (the slot yields there).
    fn wait(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, x: X, name: Sym) -> Vec<Path> {
        let g = m.game;
        let unit = p.frame().expect("a frame").unit;
        let ins = &g.ins(x.u)[x.i as usize];
        if sx(p).resume_wait == Some((unit, ins.offset)) {
            sx_mut(p).resume_wait = None;
            let (reason, what) = Machine::h_waits(name);
            let cond = format!("{} blocks", scumm_ir::py::strip(g.text(ins)));
            let why = format!("?({}: {what})", reason.as_str());
            let cite = m.cite(x);
            p.assumptions.push(Assumption { cite, cond: cond.into(), side: Side::Bool(false), why: why.into() });
            return Vec::new();
        }
        count(m, "WaitFor ops that yield (their later part is an event)");
        let c = m.cite(x);
        m.h_done(p, o, "yield", c, Some(ins.offset));
        Vec::new()
    }

    /// The ops reader.py wraps or overrides: STEP_OPS, saveVerbs, restoreVerbs, VerbOps (lazily).
    fn op(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, x: X, st: Stmt) -> Vec<Path> {
        let Stmt::Op { name, args, .. } = st else { unreachable!() };
        match name {
            known::CUTSCENE | known::END_CUTSCENE => step_op(m, p, o, x, st, Fact::Cutscene, name == known::END_CUTSCENE),
            known::USERPUT_SOFT_ON | known::USERPUT_SOFT_OFF => step_op(m, p, o, x, st, Fact::Var(VAR_USERPUT), false),
            known::CURSOR_SOFT_ON | known::CURSOR_SOFT_OFF => step_op(m, p, o, x, st, Fact::Var(VAR_CURSORSTATE), false),
            known::SAVE_VERBS => save_verbs(m, p, o, x, st, args),
            known::RESTORE_VERBS => restore_verbs(m, p, o, x, st, args),
            known::VERB_OPS => lazily(m, |m| m.op_base(p, o, x, st)),
            _ => m.op_base(p, o, x, st),
        }
    }

    /// `Reader._join`: the machine's signature with the assumed values, the steps, the kept slots and the freezes.
    fn join(&self, m: &Machine<'_>, p: &Path, o: &FactOracle) -> Option<Sig> {
        let mut sig = m.join_base(p, o)?;
        let x = sx(p);
        let mut b: Vec<u8> = Vec::with_capacity(256);
        b.extend_from_slice(&(x.pre.len() as u32).to_le_bytes());
        for (k, labels) in x.pre.iter() {
            enc_pkey(&mut b, *k);
            b.extend_from_slice(&(labels.len() as u32).to_le_bytes());
            for l in labels {
                enc_label(&mut b, *l);
            }
        }
        // reader.py walks every write for a step; only the counters STEP_OPS step ever hold one (a read of a
        // step anywhere else resolves it or forks), so these three are the same walk
        let steps: Vec<(Fact, i64)> = [Fact::Var(VAR_CURSORSTATE), Fact::Var(VAR_USERPUT), Fact::Cutscene]
            .into_iter()
            .filter_map(|k| p.writes.get(&k).filter(|&&v| is_step(m, v)).map(|&v| (k, step_of(m, v))))
            .collect();
        b.extend_from_slice(&(steps.len() as u32).to_le_bytes());
        for (k, d) in steps {
            enc_fact(&mut b, k);
            b.extend_from_slice(&d.to_le_bytes());
        }
        b.extend_from_slice(&(x.kept.len() as u32).to_le_bytes());
        for (name, k) in x.kept.iter() {
            enc_slot(&mut b, *name);
            b.extend_from_slice(&k.unit.0.to_le_bytes());
            b.extend_from_slice(&k.w.to_le_bytes());
            b.push(k.known as u8);
        }
        b.extend_from_slice(&(x.fz.len() as u32).to_le_bytes());
        for (name, (c, before)) in x.fz.iter() {
            enc_slot(&mut b, *name);
            b.extend_from_slice(&c.to_le_bytes());
            match before {
                None => b.push(0),
                Some(v) => {
                    b.push(1);
                    enc_val(&mut b, *v);
                }
            }
        }
        let mut h = FxHasher::default();
        h.write(&b);
        sig.h = fmix(sig.h ^ fmix(h.finish() ^ 0x6a09_e667_f3bc_c908));
        sig.ext = b.into_boxed_slice();
        Some(sig)
    }

    /// `Reader._merge`: the machine's merge with the assumed values as the oracle's view; the non-fact tests every
    /// merged path took the same way stay; the kept slots' locals are joined.
    fn merge(&self, m: &mut Machine<'_>, o: &FactOracle, group: Vec<Path>, sig: Rc<Sig>) -> Path {
        if group.len() == 1 {
            return m.merge_base(o, group, sig);
        }
        let g0 = &group[0];
        let x0 = sx(g0);
        let frames0: Vec<&[(i32, Val)]> = g0.frames().map(|f| f.locals.as_slice()).collect();
        let same = group[1..].iter().all(|g| {
            g.writes == g0.writes && {
                let fl: Vec<&[(i32, Val)]> = g.frames().map(|f| f.locals.as_slice()).collect();
                fl == frames0
            }
        });
        let mut nf: BTreeMap<Ident, Vec<Test>> = BTreeMap::new();
        if same {
            for (ident, tests) in x0.nf.iter() {
                let common: Vec<Test> = tests
                    .iter()
                    .copied()
                    .filter(|t| group[1..].iter().all(|g| sx(g).nf.get(ident).is_some_and(|ts| ts.contains(t))))
                    .collect();
                if !common.is_empty() {
                    nf.insert(ident.clone(), common);
                }
            }
        }
        let forgot = nf != *x0.nf;
        let mut kept = (*x0.kept).clone();
        let names: Vec<Slot> = kept.keys().copied().collect();
        for name in names {
            let known = kept[&name].known;
            let others: Vec<Vec<(i32, Val)>> = group.iter().map(|g| sx(g).kept[&name].locals.clone()).collect();
            let mut ns: Vec<i32> = others.iter().flat_map(|l| l.iter().map(|e| e.0)).collect();
            ns.sort_unstable();
            ns.dedup();
            let mut joined = Vec::with_capacity(ns.len());
            for n in ns {
                let mut vals = Vec::with_capacity(others.len());
                for l in &others {
                    let v = match l.iter().find(|e| e.0 == n) {
                        Some(e) => e.1,
                        None if known => Val::Int(0),
                        None => m.vals().mk(Reason::Local, format!("Local[{n}]")),
                    };
                    vals.push(v);
                }
                joined.push((n, m.vals().join_values(&vals)));
            }
            kept.get_mut(&name).unwrap().locals = joined;
        }
        let view = x0.pre.clone();
        st_mut(m).view = Some(view);
        let mut q = m.merge_base(o, group, sig);
        st_mut(m).view = None;
        if forgot {
            count(m, "non-fact tests forgotten where merged paths differ");
        }
        let xq = sx_mut(&mut q);
        xq.nf = Rc::new(nf);
        xq.kept = Rc::new(kept);
        q
    }

    /// The call steps reader.py overrides or wraps (`_eng_scene`; LAZY_METHODS' `_kill_room`, `_eng_scene_begin`,
    /// `_eng_scene_load`, `_eng_scene_ego`, `_eng_scroll_note`) and its two of its own (`_eng_input`,
    /// `_eng_resume_at`).
    fn eng(&self, m: &mut Machine<'_>, p: &mut Path, ci: usize, step: &Step, o: &FactOracle) -> Vec<Path> {
        match step {
            Step::Note(s) if &**s == INPUT_STEP => eng_input(m, p, ci, o),
            Step::Note(s) if &**s == RESUME_STEP => eng_resume_at(m, p, ci, o),
            Step::Scene { room, .. } => {
                // `Reader._eng_scene`: a room change to a room outside the scope ends the path there
                let res = match *room {
                    Val::Int(r) => m.game.resource(r),
                    Val::Unk(_) => None,
                };
                if res.is_none() || !st(m).scope_rooms.contains(&res) {
                    let (_, cite) = call_at(p, ci);
                    let at = format!("{cite}: room {}", m.vals_ref().show(*room));
                    m.h_halt(p, "left the scope", at.into());
                    return Vec::new();
                }
                m.eng_base(p, ci, step, o)
            }
            Step::SceneBegin(_) | Step::SceneLoad(_) | Step::SceneEgo(_) | Step::SceneKill | Step::ScrollNote => {
                lazily(m, |m| m.eng_base(p, ci, step, o))
            }
            _ => m.eng_base(p, ci, step, o),
        }
    }

    /// `Reader._eng_script`: the inventory hook is not run; its writes are guessed.
    fn eng_script(
        &self,
        m: &mut Machine<'_>,
        p: &mut Path,
        ci: usize,
        o: &FactOracle,
        n: i32,
        resistant: Option<bool>,
        recursive: Option<bool>,
        args: &[(i32, Val)],
    ) {
        if n != 0 && m.game.id(UnitKey::Global(n)).is_some() {
            let hook = lazily(m, |m| m.read(p, Fact::Var(VAR_INVENTORY_SCRIPT), o));
            if hook == Val::Int(n) {
                let (head, cite) = call_at(p, ci);
                if recursive != Some(true) {
                    m.h_stop_script(p, o, n, &cite, &head);
                }
                count(m, &format!("inventory hook runs summarized (writes guessed): {head}"));
                let keys = hook_writes(m, n);
                let u = m.vals().mk(Reason::Inventory, format!("written by the inventory hook {n} (not run)"));
                for &k in keys.iter() {
                    m.h_write(p, k, u);
                }
                m.h_note(p, cite, "inventory hook", &format!("script {n} not run: its writes are guessed"));
                return;
            }
        }
        m.eng_script_base(p, ci, o, n, resistant, recursive, args)
    }

    /// `Reader._finish`: a slot left waiting is kept with its locals (`SymPath.kept`).
    fn finish(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, how: &'static str, at: S, resume: Option<i32>) {
        let f = match p.stack.last() {
            Some(Entry::Frame(f)) => f.clone(),
            _ => panic!("finish: the top is not a frame"),
        };
        m.finish_base(p, o, how, at, resume);
        let g = m.game;
        let name = Game::unit_name(g.unit(f.unit).key);
        if how == "yield" || how == "parked" {
            let w = if how == "yield" {
                resume
            } else {
                f.pc.filter(|&pc| pc < g.unit(f.unit).ins_len).map(|pc| g.ins(f.unit)[pc as usize].offset)
            };
            if let Some(w) = w {
                let ov = f.override_;
                let cd: Vec<Val> = if ov != 0 && p.cut_data.len() >= ov as usize {
                    p.cut_data[p.cut_data.len() - ov as usize..].to_vec()
                } else {
                    Vec::new()
                };
                let ov = if cd.len() == ov as usize { ov } else { 0 };
                let k = KeptS {
                    unit: f.unit,
                    w,
                    locals: f.locals.as_slice().to_vec(),
                    known: f.args_known,
                    rec: f.recursive,
                    res: f.resistant,
                    owned: f.owned,
                    ov,
                    cd,
                };
                Rc::make_mut(&mut sx_mut(p).kept).insert(name, k);
            }
        } else if sx(p).kept.contains_key(&name) {
            Rc::make_mut(&mut sx_mut(p).kept).remove(&name);
        }
    }

    /// `Reader._count`: a freeze this path recorded has no count; else lazily.
    fn count(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, name: Slot) -> Option<(i32, bool)> {
        if let Some(&n) = p.freeze.get(&name) {
            if is_sym_count(n) {
                return None;
            }
        }
        lazily(m, |m| m.count_base(p, o, name))
    }

    fn set_freeze(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, name: Slot, n: Option<(i32, bool)>, why: &str) {
        lazily(m, |m| m.set_freeze_base(p, o, name, n, why))
    }

    /// `Reader._freeze`: the machine's rule, read lazily; a slot whose liveness is not known gets its freeze
    /// recorded, and `frozen:` where it is live.
    fn freeze(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, flag: i32, x: X) {
        let before: FxHashMap<Slot, Val> =
            p.writes.iter().filter_map(|(k, v)| if let Fact::Frozen(s) = k { Some((*s, *v)) } else { None }).collect();
        lazily(m, |m| m.freeze_base(p, o, flag, x));
        let mut names: Vec<Slot> = p.freeze.iter().filter(|(_, n)| n.is_none()).map(|(s, _)| *s).collect();
        names.sort_unstable();
        for name in names {
            let k = Fact::Frozen(name);
            if !matches!(p.writes.get(&k), Some(Val::Unk(_))) {
                continue;
            }
            let b = before.get(&name).copied();
            let c = {
                let fz = Rc::make_mut(&mut sx_mut(p).fz);
                let e = fz.entry(name).or_insert((0, b));
                e.0 += 1;
                e.0
            };
            p.freeze.insert(name, Some(sym_count(c)));
            let live = match name {
                Slot::Script(n) => Some(Fact::Script(n)),
                Slot::Local(n) => Some(Fact::Local(n)),
                Slot::Obj(ob) => Some(Fact::ObjScript(ob)),
                Slot::Entry | Slot::Exit => None,
            };
            if let Some(live) = live {
                if !matches!(p.writes.get(&live), Some(Val::Int(_))) {
                    let v = mk_unknown(m, R_COND, &live.to_string(), Some(vec![1]), COND);
                    p.writes.insert(k, v);
                    count(m, "freezes of a slot whose liveness is not known (frozen: where live)");
                }
            }
        }
    }

    /// `Reader._unfreeze`: a freeze this path recorded is undone by as many unfreezes; a slot frozen before the
    /// path is taken as frozen once.
    fn unfreeze(&self, m: &mut Machine<'_>, p: &mut Path, o: &FactOracle) {
        st_mut(m).lazy += 1;
        let mut saved: FxHashMap<Slot, Option<(i32, bool)>> = FxHashMap::default();
        let in_freeze: Vec<Slot> = p.freeze.keys().copied().collect();
        for name in in_freeze {
            if sx(p).fz.contains_key(&name) {
                let v = p.freeze.remove(&name).unwrap();
                saved.insert(name, v);
            }
        }
        m.unfreeze_base(p, o);
        st_mut(m).lazy -= 1;
        let names: Vec<Slot> = sx(p).fz.keys().copied().collect();
        for name in names {
            let (left, before) = {
                let fz = Rc::make_mut(&mut sx_mut(p).fz);
                let e = fz.get_mut(&name).unwrap();
                e.0 -= 1;
                (e.0, e.1)
            };
            if left > 0 {
                p.freeze.insert(name, saved.get(&name).copied().unwrap_or(None));
                continue;
            }
            let k = Fact::Frozen(name);
            match before {
                None => {
                    p.writes.remove(&k);
                }
                Some(v) => p.writes.insert(k, v),
            }
            p.freeze.remove(&name);
            Rc::make_mut(&mut sx_mut(p).fz).remove(&name);
            count(m, "freezes undone in the same path (frozen: restored)");
        }
        let slots = lazily(m, |m| m.h_live_slots(p, o));
        for (name, _, _) in slots {
            let k = Fact::Frozen(name);
            if sx(p).fz.contains_key(&name) || p.freeze.contains_key(&name) || p.writes.contains_key(&k) {
                continue;
            }
            p.writes.insert(k, Val::Int(0));
            count(m, "unfreezes of a slot frozen before the path (count taken as 1)");
        }
    }
}

/// A SymPath's fields for a run's first path (`_to_sym(first, pre)`).
fn sym_path(s: &St) -> Box<dyn PathExt> {
    Box::new(SymX { pre: s.seed.clone(), ..Default::default() })
}

/// `Reader._go`'s first path: the call, a SymPath.
fn first_path(m: &mut Machine<'_>, call: Call) -> Path {
    let mut p = Path::new(vec![Entry::Call(call)]);
    p.ext = Some(sym_path(st(m)));
    p
}

/// `Reader._advance`'s `except Fork`; with F12's value sets (`sets`), one path per set instead of per value.
fn on_fork(m: &mut Machine<'_>, p: &mut Path, snap: Path, key: Fact, sets: Option<Vec<Labels>>) -> Vec<Path> {
    let i = info(m, key);
    let dom = i.dom.as_ref().unwrap_or_else(|| panic!("a fork on {} with no domain", i.key));
    let vals: Labels = match sx(p).pre.get(&PKey::F(key)) {
        Some(s) => dom.values.iter().copied().filter(|v| s.contains(v)).collect(),
        None => dom.values.clone(),
    };
    let groups: Vec<Labels> = match sets {
        Some(sets) => sets,
        None => vals.iter().map(|&v| vec![v]).collect(),
    };
    let fam = family(key);
    let s = st_mut(m);
    if s.first_fork.is_none() {
        s.first_fork = Some(i.key.to_string());
    }
    s.rd.forks.inc(&fam);
    s.rd.fork_keys.inc(&*i.key);
    s.rd.counts.inc("forks on read");
    s.rd.counts.add("paths from forks on read", groups.len() as i64);
    if vals.is_empty() {
        // no value of the fact is left: the path cannot happen
        s.rd.counts.inc(&format!("paths with no value left for a fact ({fam})"));
        *p = snap;
        m.h_halt(p, "no value", (*i.key).into());
        return Vec::new();
    }
    let mut forks = Vec::with_capacity(groups.len());
    for g in groups.iter().skip(1) {
        let mut q = snap.clone();
        Rc::make_mut(&mut sx_mut(&mut q).pre).insert(PKey::F(key), g.clone());
        forks.push(q);
    }
    let mut first = snap;
    Rc::make_mut(&mut sx_mut(&mut first).pre).insert(PKey::F(key), groups[0].clone());
    *p = first;
    forks
}

/// reader.py `_make_step`: the op with its counter kept as a step until something reads it.
fn step_op(m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, x: X, st: Stmt, key: Fact, end: bool) -> Vec<Path> {
    {
        let s = st_mut(m);
        s.step_key = Some(key);
        if end {
            // its test of an armed override reads every esc: key: lazily
            s.lazy += 1;
        }
    }
    let out = m.op_base(p, o, x, st);
    {
        let s = st_mut(m);
        s.step_key = None;
        if end {
            s.lazy -= 1;
        }
    }
    if end {
        if let Some(&v) = p.writes.get(&key) {
            if is_step(m, v) {
                // endCutscene with no cutscene open is the engine's error() (script.cpp:1668-1681): the depth
                // before it, the start's plus the steps before, is at least 1, a precondition
                let d = step_of(m, v) + 1;
                let i = info(m, key);
                let dom = i.dom.as_ref().unwrap();
                let have: Labels = sx(p).pre.get(&PKey::F(key)).cloned().unwrap_or_else(|| dom.values.clone());
                let ok: Labels =
                    have.into_iter().filter(|&u| u == Label::Above || matches!(u, Label::Int(n) if n + d >= 1)).collect();
                if ok.is_empty() {
                    let c = m.cite(x);
                    m.error(p, "cutscene stack underflow", c, "endCutscene's error() (script.cpp:1668-1681)", true);
                } else if ok != dom.values {
                    Rc::make_mut(&mut sx_mut(p).pre).insert(PKey::F(key), ok);
                }
            }
        }
    }
    out
}

/// `Reader._op_saveVerbs`: whether each verb was shown is kept as `saved:<verb>:<id>`.
fn save_verbs(m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, x: X, st: Stmt, args: Span) -> Vec<Path> {
    let ks = m.game.ast.kids(args);
    let vals: Vec<Val> = ks.iter().map(|&e| m.val(p, o, e, Some(x))).collect();
    let [a, b, sid] = vals[..] else { panic!("saveVerbs takes three values") };
    let (Val::Int(a), Val::Int(b), Val::Int(sid)) = (a, b, sid) else {
        return lazily(m, |m| m.op_base(p, o, x, st));
    };
    for v in a..=b {
        let now = lazily(m, |m| m.read(p, Fact::Verb(v), o));
        let s = match now {
            Val::Int(_) => now,
            _ if class_of(m, now) == Some(COPY) => now,
            _ => mk_unknown(m, R_COPY, &format!("verb:{v}"), Some(vec![0, 1]), COPY),
        };
        m.h_write(p, saved_fact(v, sid), s);
        m.h_write(p, Fact::Verb(v), Val::Int(0));
    }
    Vec::new()
}

/// `Reader._op_restoreVerbs`: a saved verb that was shown is shown again (`saved:<verb>:<id>`).
fn restore_verbs(m: &mut Machine<'_>, p: &mut Path, o: &FactOracle, x: X, st: Stmt, args: Span) -> Vec<Path> {
    let ks = m.game.ast.kids(args);
    let vals: Vec<Val> = ks.iter().map(|&e| m.val(p, o, e, Some(x))).collect();
    let [a, b, sid] = vals[..] else { panic!("restoreVerbs takes three values") };
    let (Val::Int(a), Val::Int(b), Val::Int(sid)) = (a, b, sid) else {
        return m.op_base(p, o, x, st);
    };
    for v in a..=b {
        let key = saved_fact(v, sid);
        let s = lazily(m, |m| m.read(p, key, o));
        match s {
            Val::Int(n) => {
                if n != 0 {
                    m.h_write(p, Fact::Verb(v), Val::Int(1));
                }
            }
            _ if class_of(m, s) == Some(COPY) => m.h_write(p, Fact::Verb(v), s),
            _ => {
                let c = mk_unknown(m, R_COND, &key_str(key), Some(vec![1]), COND);
                m.h_write(p, Fact::Verb(v), c);
            }
        }
        m.h_write(p, key, Val::Int(0));
    }
    Vec::new()
}

/// model.py `_eng_input` (its switch input_after_turns off): the click on a verb runs the input script.
fn eng_input(m: &mut Machine<'_>, p: &mut Path, ci: usize, o: &FactOracle) -> Vec<Path> {
    let verb = st(m).verb;
    for k in [Fact::Var(VAR_VERB_SCRIPT), Fact::Var(VAR_USERPUT), Fact::Verb(verb)] {
        // the drift's reads (`Path.input_drift`, which the reader does not read): a fact may fork here
        m.read(p, k, o);
    }
    let (_, cite) = call_at(p, ci);
    let n = m.oget(o, Fact::Var(VAR_VERB_SCRIPT));
    match n {
        Val::Unk(_) => {
            let w = format!("input script {}", m.vals_ref().show(n));
            p.unknown_writes.push(("runInputScript".into(), cite, w.into()));
        }
        Val::Int(0) => m.h_note(p, cite, "runInputScript", "VAR_VERB_SCRIPT is 0: no input script runs"),
        Val::Int(n) => {
            let args = vec![(0, Val::Int(VERB_CLICK_AREA)), (1, Val::Int(verb)), (2, Val::Int(LEFT_CLICK))];
            let step = Step::Script { n, resistant: Some(false), recursive: Some(false), args };
            let cid = m.h_cid();
            p.stack.push(Entry::Call(Call {
                head: "runInputScript".into(),
                cite: "script.cpp:1478-1511".into(),
                steps: vec![step].into(),
                caller: None,
                active: false,
                i: 0,
                cid,
            }));
        }
    }
    Vec::new()
}

/// `Reader._eng_resume_at`: runAllScripts gives the event's slot its turn from its wait point, with its locals.
fn eng_resume_at(m: &mut Machine<'_>, p: &mut Path, ci: usize, o: &FactOracle) -> Vec<Path> {
    let ev = st(m).event.clone().expect("the run's event");
    let g = m.game;
    let uid = g.id(ev.unit).expect("the event's unit");
    let idx = g.at(uid, ev.w as i32).expect("the event's wait point");
    let key = g.lifecycle_key(ev.unit);
    let name = slot_of(&ev.name);
    if ev.abort {
        m.h_write(p, Fact::Var(VAR_OVERRIDE), Val::Int(1));
        m.clear(p, o, Prefix::Esc(name));
        m.set_freeze(p, o, name, Some((0, true)), "");
        p.unknown_writes.push((ESC_KEY[0].into(), ESC_KEY[1].into(), ESC_KEY[2].into()));
        count(m, "Esc keys whose input-script run is left out (ESC_KEY)");
    }
    if !ev.abort && idx > 0 {
        let prev = &g.ins(uid)[idx as usize - 1];
        let delay = matches!(prev.st, Some(Stmt::Op { name: known::DELAY | known::DELAY_VARIABLE, .. }));
        if prev.next == Some(idx) && delay {
            let c = m.cite(X { u: uid, i: idx - 1 });
            let cond = format!("{}'s delay is over", ev.name);
            let why = format!("?({}: {}'s delay count)", Reason::Timer.as_str(), ev.name);
            p.assumptions.push(Assumption { cite: c, cond: cond.into(), side: Side::Bool(true), why: why.into() });
        }
    }
    m.clear(p, o, Prefix::At(name));
    if g.ins(uid)[idx as usize].yields == Some(Yield::Wait) {
        sx_mut(p).resume_wait = Some((uid, ev.w as i32));
    }
    let locals: Vec<(i32, Val)> = ev.locals.iter().map(|(n, v)| (*n as i32, mval(m, v))).collect();
    let owned = match &ev.owned {
        Py::Bool(b) => Owned::Bool(*b),
        other => Owned::Unk(mval(m, &RVal::from_py(other))),
    };
    let rec = opt_bool(&ev.recursive);
    let res = opt_bool(&ev.resistant);
    m.h_push_frame(p, ci, uid, idx, &locals, owned, rec, res, key, ev.args_known, false);
    let ov = ev.override_;
    if ov != 0 && ev.cut_data.len() as i64 == ov && p.end.is_none() {
        // the cutscenes this slot began in an earlier turn, and their data
        let top = p.stack.len() - 1;
        p.frame_at(top).override_ = (if ev.abort { ov - 1 } else { ov }) as i32;
        p.cut_data = ev.cut_data.iter().map(|&c| Val::Int(c as i32)).collect();
        count(m, "events resuming a slot with its own open cutscenes (cutscene data carried)");
    }
    Vec::new()
}

fn opt_bool(p: &Py) -> Option<bool> {
    match p {
        Py::Bool(b) => Some(*b),
        Py::None => None,
        other => panic!("a slot flag that is not a bool or None: {}", other.text()),
    }
}

/// `Reader._hook_writes(n)` as facts.
fn hook_writes(m: &mut Machine<'_>, n: i32) -> Rc<[Fact]> {
    if let Some(v) = st(m).hook_writes.get(&n) {
        return v.clone();
    }
    let keys: Rc<[Fact]> = scope::hook_writes(m.game, n)
        .iter()
        .map(|k| k.parse::<Fact>().unwrap_or_else(|_| panic!("a hook write the machine cannot hold: {k}")))
        .collect();
    st_mut(m).hook_writes.insert(n, keys.clone());
    keys
}

// --- the join signature's extension, encoded -------------------------------------------------------------------------

fn enc_slot(b: &mut Vec<u8>, s: Slot) {
    match s {
        Slot::Script(n) => {
            b.push(0);
            b.extend_from_slice(&n.to_le_bytes());
        }
        Slot::Local(n) => {
            b.push(1);
            b.extend_from_slice(&n.to_le_bytes());
        }
        Slot::Obj(n) => {
            b.push(2);
            b.extend_from_slice(&n.to_le_bytes());
        }
        Slot::Entry => b.push(3),
        Slot::Exit => b.push(4),
    }
}

fn enc_fact(b: &mut Vec<u8>, f: Fact) {
    let mut one = |tag: u8, n: i32| {
        b.push(tag);
        b.extend_from_slice(&n.to_le_bytes());
    };
    match f {
        Fact::Var(n) => one(0, n),
        Fact::Bit(n) => one(1, n),
        Fact::Owner(n) => one(2, n),
        Fact::State(n) => one(3, n),
        Fact::Class(o, c) => {
            one(4, o);
            b.extend_from_slice(&c.to_le_bytes());
        }
        Fact::Room => b.push(5),
        Fact::Ego => b.push(6),
        Fact::Script(n) => one(7, n),
        Fact::Local(n) => one(8, n),
        Fact::ObjScript(n) => one(9, n),
        Fact::At(s, w) => {
            b.push(10);
            enc_slot(b, s);
            b.extend_from_slice(&w.to_le_bytes());
        }
        Fact::Frozen(s) => {
            b.push(11);
            enc_slot(b, s);
        }
        Fact::Cutscene => b.push(12),
        Fact::Esc(s, w) => {
            b.push(13);
            enc_slot(b, s);
            b.extend_from_slice(&w.to_le_bytes());
        }
        Fact::Verb(n) => one(14, n),
        Fact::ObjPos(n) => one(15, n),
        Fact::ActorRoom(n) => one(16, n),
    }
}

fn enc_pkey(b: &mut Vec<u8>, k: PKey) {
    match k {
        PKey::F(f) => {
            b.push(0);
            enc_fact(b, f);
        }
        PKey::Args(s) => {
            b.push(1);
            enc_slot(b, s);
        }
    }
}

fn enc_label(b: &mut Vec<u8>, l: Label) {
    match l {
        Label::Below => b.push(0),
        Label::Int(n) => {
            b.push(1);
            b.extend_from_slice(&n.to_le_bytes());
        }
        Label::Above => b.push(2),
    }
}

fn enc_val(b: &mut Vec<u8>, v: Val) {
    match v {
        Val::Int(n) => {
            b.push(0);
            b.extend_from_slice(&n.to_le_bytes());
        }
        Val::Unk(id) => {
            b.push(1);
            b.extend_from_slice(&id.0.to_le_bytes());
        }
    }
}

// --- values between the two halves ---------------------------------------------------------------------------------

/// A Builder value (pins, an event's locals) in the machine's run.
fn mval(m: &mut Machine<'_>, v: &RVal) -> Val {
    match v {
        RVal::Int(n) => Val::Int(i32::try_from(*n).unwrap_or_else(|_| panic!("a value past int32: {n}"))),
        RVal::Unk(u) => {
            let class = match u.class {
                UClass::Unknown => 0,
                UClass::Cond => COND,
                UClass::Copy => COPY,
            };
            let values = u.values.as_ref().map(|vs| vs.iter().map(|&x| x as i32).collect());
            mk_unknown(m, reason_of(&u.reason), &u.what, values, class)
        }
    }
}

/// A machine value as the Builder reads it.
fn rval(run: &Run, v: Val) -> RVal {
    match v {
        Val::Int(n) => RVal::Int(n as i64),
        Val::Unk(id) => {
            let u = run.vals.get(id);
            RVal::Unk(RUnknown {
                class: match u.class {
                    0 => UClass::Unknown,
                    COND => UClass::Cond,
                    COPY => UClass::Copy,
                    c => panic!("an unknown of class {c}"),
                },
                reason: u.reason.as_str().into_owned(),
                what: u.what.to_string(),
                values: u.values.as_ref().map(|vs| vs.iter().map(|&x| x as i64).collect()),
            })
        }
    }
}

fn py_opt_bool(b: Option<bool>) -> Py {
    b.map_or(Py::None, Py::Bool)
}

fn tri_py(t: &(S, S, S)) -> Py {
    Py::Tuple(vec![Py::str(&*t.0), Py::str(&*t.1), Py::str(&*t.2)])
}

/// `Builder.run`'s and `Builder.record`'s reads of a path (reader_port_oracle.py's `_path_dict`).
fn path_out(game: &Game, run: &Run, p: &Path) -> PathOut {
    let end = p.end.map(|e| e.to_string());
    let sym = p.ext.is_some();
    if !sym || end.as_deref().is_some_and(|e| DROP_ENDS.contains(&e)) {
        return PathOut { end, sym, ..Default::default() };
    }
    let x = sx(p);
    let pre: Pre = x.pre.iter().map(|(k, v)| (pkey_str(*k), v.clone())).collect();
    let writes: BTreeMap<String, RVal> = p.writes.iter().map(|(&k, &v)| (key_str(k), rval(run, v))).collect();
    let kept: BTreeMap<String, Kept> = x
        .kept
        .iter()
        .map(|(name, k)| {
            let kept = Kept {
                unit: game.unit(k.unit).key,
                w: k.w as i64,
                locals: k.locals.iter().map(|(n, v)| (*n as i64, rval(run, *v))).collect(),
                known: k.known,
                recursive: py_opt_bool(k.rec),
                resistant: py_opt_bool(k.res),
                owned: match k.owned {
                    Owned::Bool(b) => Py::Bool(b),
                    Owned::Unk(v) => rval(run, v).py(),
                },
                override_: k.ov as i64,
                cut_data: k.cd.iter().map(|&v| rval(run, v)).collect(),
            };
            (name.to_string(), kept)
        })
        .collect();
    let assumptions = p
        .assumptions
        .iter()
        .map(|a| RAssumption {
            cite: a.cite.to_string(),
            cond: a.cond.to_string(),
            side: match a.side {
                Side::Bool(b) => Py::Bool(b),
                Side::Exit => Py::str("exit"),
                Side::Stay => Py::str("stay"),
            },
            why: a.why.to_string(),
        })
        .collect();
    let n = p.notes.len();
    let notes = p.notes.iter().skip(n.saturating_sub(5)).map(tri_py).collect();
    PathOut {
        end,
        sym,
        pre,
        writes,
        kept,
        unknown_writes: p.unknown_writes.iter().map(tri_py).collect(),
        assumptions,
        notes,
    }
}

// --- the Reader -----------------------------------------------------------------------------------------------------

/// reader.py `Reader`: model.Machine with the features of rows 0-2 (own_entry, dispatch, calls, room_change; no
/// settle, D5) and reader.py's overrides, one run at a time.
pub struct Reader<'g> {
    pub m: Machine<'g>,
    o: FactOracle,
}

impl<'g> Reader<'g> {
    pub fn new(game: &'g Game) -> Reader<'g> {
        let features = Features { own_entry: true, dispatch: true, calls: true, room_change: true, ..Default::default() };
        let mut m = Machine::new(game, features, Switches::default());
        m.path_limit = PATH_LIMIT;
        m.hooks = Some(&HOOKS);
        m.ext = Some(Box::new(St::default()));
        Reader { m, o: FactOracle::new(FactMap::default(), None) }
    }

    /// The table and scope of the Builder: copied at the first run and again when a pass widened a domain.
    fn sync(&mut self, cx: &Ctx) {
        let game = self.m.game;
        let s = st_mut(&mut self.m);
        if s.scope.is_none() {
            let scope = cx.scope.clone();
            s.uni.clear();
            s.uni_set.clear();
            for k in &scope.universe {
                let f: Fact = k.parse().unwrap_or_else(|_| panic!("a universe key the machine cannot hold: {k}"));
                s.uni_set.insert(f);
                for pre in prefixes(f) {
                    s.uni.entry(pre).or_default().push(f);
                }
            }
            s.scope_rooms = scope.rooms.iter().map(|&r| game.resource(r)).collect();
            s.scope = Some(scope);
        }
        if s.table.as_ref().is_none_or(|t| t.extra != cx.table.extra) {
            s.table = Some(cx.table.clone());
            s.info.clear();
        }
        // fix round 1's switches, as the reading's scope has them (Scope::of_with); value_sets follows the reading's
        // table instead (run_full), with the machine's wide sets and findObject's objects
        self.m.switches.draw_clears = cx.scope.switches.draw_clears;
        self.m.switches.actor_rooms = cx.scope.switches.actor_rooms;
    }
}

/// The oracle prefixes (model.py's `keys` callers) a key lists under.
fn prefixes(f: Fact) -> Vec<Prefix> {
    match f {
        Fact::At(s, _) => vec![Prefix::At(s)],
        Fact::Esc(s, _) => vec![Prefix::Esc(s), Prefix::EscAll],
        Fact::Owner(_) => vec![Prefix::Owner],
        Fact::Script(_) => vec![Prefix::Script],
        Fact::Local(_) => vec![Prefix::Local],
        Fact::ObjScript(_) => vec![Prefix::ObjScript],
        _ => vec![],
    }
}

impl Reader<'_> {
    /// One run (`Reader.run_sentence_sym`, `run_click_verb`, `run_event`): the machine's Run, the first fact it
    /// forked on and the Reader's counters' increments.
    pub fn run_full(&mut self, cx: &Ctx, pins: &Pins, pre: &Pre, what: &What) -> (Run, Option<String>, RdCounts) {
        self.sync(cx);
        // fix round 1, F12: the machine's switch value_sets follows the reading's (findObject over the scope's objects)
        if self.m.switches.value_sets != cx.table.value_sets {
            self.m.switches.value_sets = cx.table.value_sets;
            self.m.vals().wide = cx.table.value_sets;
            self.m.find_objects = cx.table.value_sets.then(|| cx.scope.objects.iter().map(|&o| o as i32).collect());
        }
        let m = &mut self.m;
        let mut mpins = FxHashMap::default();
        let mut extra: Vec<(String, Fact)> = Vec::new();
        for (k, v) in pins {
            // only facts reach the machine (an args: pin is the Builder's)
            let Ok(f) = k.parse::<Fact>() else { continue };
            let v = mval(m, v);
            mpins.insert(f, v);
            if !st(m).uni_set.contains(&f) {
                extra.push((k.clone(), f));
            }
        }
        extra.sort();
        let seed: BTreeMap<PKey, Labels> = pre.iter().map(|(k, v)| (pkey_of(k), v.clone())).collect();
        {
            let s = st_mut(m);
            s.pins = mpins;
            s.pin_extra = extra.into_iter().map(|e| e.1).collect();
            s.seed = Rc::new(seed);
            s.first_fork = None;
            s.rd = RdCounts::default();
            s.lazy = 0;
            s.step_key = None;
            s.pending = None;
            s.view = None;
            s.event = None;
        }
        let o = &self.o;
        let run = match what {
            What::Sentence { verb, a, b } => m.run_sentence(o, *verb as i32, *a as i32, b.map(|b| b as i32)),
            What::Verb(v) => {
                st_mut(m).verb = *v as i32;
                let cid = m.h_cid();
                let call = Call {
                    head: "click".into(),
                    cite: "scumm.cpp:3175-3255".into(),
                    steps: vec![Step::Note(INPUT_STEP.into())].into(),
                    caller: None,
                    active: false,
                    i: 0,
                    cid,
                };
                let first = first_path(m, call);
                m.h_drive(o, first, format!("click on verb {v}"), None)
            }
            What::Event(ev) => {
                st_mut(m).event = Some(ev.clone());
                let cid = m.h_cid();
                let call = Call {
                    head: "runAllScripts".into(),
                    cite: "script.cpp:968-987".into(),
                    steps: vec![Step::Note(RESUME_STEP.into())].into(),
                    caller: None,
                    active: false,
                    i: 0,
                    cid,
                };
                // its call's head is model.py's ROUND, so the first path waits at a round's join as it starts
                let first = first_path(m, call);
                m.h_drive(o, first, ev.label(), Some(ev.unit))
            }
        };
        let s = st_mut(m);
        (run, s.first_fork.take(), std::mem::take(&mut s.rd))
    }

    /// A path of `run` whole, as reader_run_dump.py writes it.
    pub fn path_dump(&self, run: &Run, p: &Path) -> Py {
        let game = self.m.game;
        let tri_list = |l: &scumm_machine::plist::PList<(S, S, S)>| Py::List(l.iter().map(tri_py).collect());
        let o = path_out(game, run, &Path { notes: Default::default(), end: None, ..p.clone() });
        let mut f = vec![
            ("end", p.end.map_or(Py::None, Py::str)),
            ("end_at", p.end_at.as_ref().map_or(Py::None, |s| Py::str(&**s))),
            ("sym", Py::Bool(p.ext.is_some())),
            ("steps", Py::Int(p.steps as i64)),
            ("writes", Py::Dict(p.writes.iter().map(|(&k, &v)| (key_str(k), rval(run, v).py())).collect())),
            ("assumptions", Py::List(o.assumptions.iter().map(|a| a.py()).collect())),
            ("notes", tri_list(&p.notes)),
            ("unknown_writes", tri_list(&p.unknown_writes)),
            ("errors", tri_list(&p.errors)),
            ("unmodelled", tri_list(&p.unmodelled)),
            ("turns", Py::List(p.turns.iter().map(|t| Py::Tuple(vec![Py::str(t.0.to_string()), Py::str(t.1)])).collect())),
        ];
        if p.ext.is_some() {
            f.push(("pre", crate::record::pre_py(&o.pre)));
            f.push(("kept", Py::Dict(o.kept.iter().map(|(k, v)| (k.clone(), v.py())).collect())));
        }
        Py::dict(f)
    }
}

impl RunMachine for Reader<'_> {
    /// `Reader.run_sentence_sym`, `run_click_verb`, `run_event`.
    fn run(&mut self, cx: &Ctx, _label: &str, pins: &Pins, pre: &Pre, what: &What) -> RunOut {
        let (run, first_fork, rd) = self.run_full(cx, pins, pre, what);
        let game = self.m.game;
        // the switch split_limit: the Builder splits a run the path limit cut and reads none of its paths
        let paths = if cx.table.split_limit && run.limit != 0 {
            Vec::new()
        } else {
            run.paths.iter().map(|p| path_out(game, &run, p)).collect()
        };
        RunOut { paths, limit: run.limit as i64, value_caps: run.value_caps.iter().map(|c| c.1 as i64).sum(), first_fork, rd }
    }
}

/// A captured run (reader_port_oracle.py's line `c`, the `i`-th) made again by the Rust machine half and written
/// whole, as reader_run_dump.py writes the Python's.
pub fn dump_capture_run(rd: &mut Reader, cx: &Ctx, i: usize, c: &Py) -> Py {
    let pins = crate::record::pins_from_py(c.get("pins"));
    let pre = crate::record::pre_from_py(c.get("pre"));
    let w = c.get("what").items();
    let what = match w[0].as_str() {
        "run_sentence_sym" => What::Sentence { verb: w[1].int(), a: w[2].int(), b: (!w[3].is_none()).then(|| w[3].int()) },
        "run_click_verb" => What::Verb(w[1].int()),
        "run_event" => What::Event(Event::from_py(&w[1])),
        k => panic!("not a run: {k}"),
    };
    let (run, first_fork, rdc) = rd.run_full(cx, &pins, &pre, &what);
    let nz = |c: &crate::py::Counter| Py::Dict(c.0.iter().filter(|(_, &v)| v != 0).map(|(k, v)| (k.clone(), Py::Int(*v))).collect());
    Py::dict(vec![
        ("run", Py::Int(i as i64)),
        ("label", c.get("label").clone()),
        ("pass", c.get("pass").clone()),
        ("limit", Py::Int(run.limit as i64)),
        ("value_caps", Py::Int(run.value_caps.iter().map(|c| c.1 as i64).sum())),
        ("first_fork", first_fork.map_or(Py::None, Py::str)),
        ("rd", Py::dict(vec![("counts", nz(&rdc.counts)), ("forks", nz(&rdc.forks)), ("fork_keys", nz(&rdc.fork_keys))])),
        ("paths", Py::List(run.paths.iter().map(|p| rd.path_dump(&run, p)).collect())),
    ])
}
