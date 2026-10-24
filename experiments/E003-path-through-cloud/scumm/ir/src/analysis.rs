//! What model.Game reads from the code once, before any Machine runs: control-flow successors, join
//! points, the loops that hold a yield and what their exit tests read, wait points, time exits, and the
//! variables whose every write is unknown (model.py `Game.successors`, `join_points`, `loops`,
//! `_test_reads`, `wait_point`, `time_exits`, `time_loops_at`, `const_poll_loops`, `script_writes`,
//! `non_fact_vars`). Every rule and order is model.py's; its docstrings have the reasons.
//!
//! Python computes these lazily per unit; here [`crate::Game::load`] computes them for every unit and
//! both settings of switch const_polls, into flat tables (an [`Analyses`] per setting).

use crate::ast::*;
use crate::facts::{is_clock, Fact};
use crate::read::Script;
use crate::sym::{known, Sym};
use rustc_hash::FxHashMap;
use std::fmt;

/// Why a value is not a fact (model.py's `R_*` reasons, "counter", and `var N` from non_fact_vars).
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Reason {
    Random,
    Position,
    Actor,
    Timer,
    Local,
    Held,
    Work,
    Sound,
    String,
    Inventory,
    Verb,
    Message,
    Sentence,
    Key,
    Freeze,
    CutData,
    Cache,
    Branch,
    LoopExit,
    WaitOver,
    Settle,
    Camera,
    Input,
    /// a counter the loop itself steps (`x op= constant`)
    Counter,
    /// a variable whose value comes from variable N, itself not a fact
    Var(i32),
}

impl Reason {
    /// model.py's string for the reason.
    pub fn as_str(self) -> std::borrow::Cow<'static, str> {
        use std::borrow::Cow::Borrowed;
        Borrowed(match self {
            Reason::Random => "random draw",
            Reason::Position => "position",
            Reason::Actor => "actor state",
            Reason::Timer => "timer",
            Reason::Local => "local",
            Reason::Held => "not held",
            Reason::Work => "cannot work out",
            Reason::Sound => "sound",
            Reason::String => "string",
            Reason::Inventory => "inventory order",
            Reason::Verb => "verb slot",
            Reason::Message => "message state",
            Reason::Sentence => "sentence queue",
            Reason::Key => "key press",
            Reason::Freeze => "freeze count",
            Reason::CutData => "cutscene data",
            Reason::Cache => "resource cache",
            Reason::Branch => "branch",
            Reason::LoopExit => "loop exit by time",
            Reason::WaitOver => "wait over by time",
            Reason::Settle => "not settled",
            Reason::Camera => "camera position",
            Reason::Input => "mouse",
            Reason::Counter => "counter",
            Reason::Var(n) => return std::borrow::Cow::Owned(format!("var {n}")),
        })
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

/// A getter's class (model.py `GETTERS`): evaluated from facts, or unknown for a reason.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GetterKind {
    Fact,
    NonFact(Reason),
}

const S5: &str = "script_v5.cpp";

/// model.py `GETTERS`: (name, kind, cite), in its order.
pub fn getters() -> Vec<(Sym, &'static str, GetterKind, String)> {
    use GetterKind::*;
    use Reason::*;
    let text = "a text code inside a string (descumm), never a value".to_string();
    let mut out = vec![
        (known::CLASS_OF_IS, Fact, format!("{S5}:1490-1517; object.cpp:229-250")),
        (known::GET_OBJECT_OWNER, Fact, format!("{S5}:1428-1431")),
        (known::GET_OBJECT_STATE, Fact, format!("{S5}:1433-1436")),
        (known::IS_SCRIPT_RUNNING, Fact, format!("{S5}:1447-1479; script.cpp:1557-1564")),
        (known::GET_INVENTORY_COUNT, Fact, format!("{S5}:1423-1426; object.cpp:87-96")),
        (known::GET_VERB_ENTRY_POINT, Fact, format!("{S5}:1481-1488; script.cpp:167-235")),
        (known::GET_RANDOM_NR, NonFact(Random), format!("{S5}:1438-1445")),
        (known::GET_ACTOR_X, NonFact(Position), format!("{S5}:1347-1357")),
        (known::GET_ACTOR_Y, NonFact(Position), format!("{S5}:1359-1369")),
        (known::GET_ACTOR_WALK_BOX, NonFact(Position), format!("{S5}:1333-1338")),
        (known::GET_ACTOR_ELEVATION, NonFact(Position), format!("{S5}:1268-1273")),
        (known::GET_DIST, NonFact(Position), format!("{S5}:1406-1421")),
        (known::FIND_OBJECT, NonFact(Position), format!("{S5}:1211-1250")),
        (known::ACTOR_FROM_POS, NonFact(Position), format!("{S5}:416-422")),
        (known::GET_ACTOR_ROOM, NonFact(Actor), format!("{S5}:1289-1322")),
        (known::GET_ACTOR_COSTUME, NonFact(Actor), format!("{S5}:1261-1266")),
        (known::GET_ACTOR_FACING, NonFact(Actor), format!("{S5}:1275-1280")),
        (known::GET_ACTOR_MOVING, NonFact(Actor), format!("{S5}:1282-1287")),
        (known::GET_ACTOR_WIDTH, NonFact(Actor), format!("{S5}:1340-1345")),
        (known::IS_SOUND_RUNNING, NonFact(Sound), format!("{S5}:2866-2873")),
        (known::GET_STRING_CHAR, NonFact(String), format!("{S5}:3041-3123")),
        (
            known::FIND_INVENTORY,
            NonFact(Inventory),
            format!("{S5}:1204-1209; object.cpp:77-85 (the inventory list's order is not a fact)"),
        ),
    ];
    for name in [
        known::GET_INT,
        known::GET_NAME,
        known::GET_STRING,
        known::GET_VERB,
        known::KEEP_TEXT,
        known::NEWLINE,
        known::UNKNOWN8,
        known::WAIT,
    ] {
        out.push((name, NonFact(Work), text.clone()));
    }
    out.into_iter()
        .map(|(y, k, c)| (y, crate::sym::known::ALL[y.0 as usize], k, c))
        .collect()
}

/// The class of getter `name` (None: not in model.py's GETTERS).
#[inline]
pub fn getter_kind(name: Sym) -> Option<GetterKind> {
    use GetterKind::*;
    use Reason::*;
    Some(match name {
        known::CLASS_OF_IS
        | known::GET_OBJECT_OWNER
        | known::GET_OBJECT_STATE
        | known::IS_SCRIPT_RUNNING
        | known::GET_INVENTORY_COUNT
        | known::GET_VERB_ENTRY_POINT => Fact,
        known::GET_RANDOM_NR => NonFact(Random),
        known::GET_ACTOR_X
        | known::GET_ACTOR_Y
        | known::GET_ACTOR_WALK_BOX
        | known::GET_ACTOR_ELEVATION
        | known::GET_DIST
        | known::FIND_OBJECT
        | known::ACTOR_FROM_POS => NonFact(Position),
        known::GET_ACTOR_ROOM
        | known::GET_ACTOR_COSTUME
        | known::GET_ACTOR_FACING
        | known::GET_ACTOR_MOVING
        | known::GET_ACTOR_WIDTH => NonFact(Actor),
        known::IS_SOUND_RUNNING => NonFact(Sound),
        known::GET_STRING_CHAR => NonFact(String),
        known::FIND_INVENTORY => NonFact(Inventory),
        known::GET_INT
        | known::GET_NAME
        | known::GET_STRING
        | known::GET_VERB
        | known::KEEP_TEXT
        | known::NEWLINE
        | known::UNKNOWN8
        | known::WAIT => NonFact(Work),
        _ => return None,
    })
}

/// VAR_CAMERA_POS_X (vars.cpp:39)
pub const VAR_CAMERA_POS_X: i32 = 2;
/// model.py `TIME_VARS`: engine variables the engine changes as frames pass, outside opcodes.
pub const TIME_VARS: [i32; 4] = [2, 3, 23, 25];
/// model.py `HOOK_VARS`: VAR_SCROLL_SCRIPT..VAR_CUTSCENE_END_SCRIPT, the script numbers the engine starts.
pub const HOOK_VARS: std::ops::Range<i32> = 27..37;

/// What an exit test reads: a fact key, or a prefix when the code does not fix the key
/// (model.py's strings ending in ":").
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Read {
    Fact(Fact),
    /// `var:`, `bit:`, `owner:`, `state:`, `class:`, `script:`, `local:`: any key of that kind
    Any(&'static str),
    /// `class:O:`: any class of object O
    ClassOf(i32),
}

impl fmt::Display for Read {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Read::Fact(k) => write!(f, "{k}"),
            Read::Any(kind) => write!(f, "{kind}:"),
            Read::ClassOf(o) => write!(f, "class:{o}:"),
        }
    }
}

/// A loop's class by what its exit tests read (model.py `Loop.kind`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum LoopKind {
    /// no exit at all
    Ambient,
    /// every exit test reads only non-facts
    Time,
    /// some exit test reads a fact
    Facts,
}

impl LoopKind {
    pub fn as_str(self) -> &'static str {
        match self {
            LoopKind::Ambient => "ambient",
            LoopKind::Time => "time",
            LoopKind::Facts => "facts",
        }
    }
}

/// A loop of a unit that holds a yield (model.py `Loop`): the natural loop of a backward jump.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Loop {
    pub head: u32,
    /// sorted instruction indices
    pub body: Vec<u32>,
    /// (jump index, its successor outside the body), by jump index
    pub exits: Vec<(u32, u32)>,
    pub kind: LoopKind,
    /// what the exit tests read, sorted by their strings
    pub reads: Vec<Read>,
    /// the reasons of the non-facts they read, sorted by their strings
    pub non_facts: Vec<Reason>,
}

impl Loop {
    #[inline]
    pub fn contains(&self, i: i64) -> bool {
        i >= 0 && self.body.binary_search(&(i as u32)).is_ok()
    }
    /// A time loop whose exit tests read the camera position (settle forks on it once).
    #[inline]
    pub fn camera(&self) -> bool {
        self.non_facts.contains(&Reason::Camera)
    }
}

/// Where a live slot stands, as settle classes it (model.py `WaitPoint.kind`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum WaitKind {
    Wait,
    Delay,
    Frame,
    Parked,
    Start,
    Other,
}

impl WaitKind {
    pub fn as_str(self) -> &'static str {
        match self {
            WaitKind::Wait => "wait",
            WaitKind::Delay => "delay",
            WaitKind::Frame => "frame",
            WaitKind::Parked => "parked",
            WaitKind::Start => "start",
            WaitKind::Other => "other",
        }
    }
}

/// model.py `WaitPoint`: the kind, the WaitFor op or the opcode a parked slot ran (`known::EMPTY`: none),
/// and the innermost loop that holds the yield, as an index into the unit's loops.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct WaitPoint {
    pub kind: WaitKind,
    pub op: Sym,
    pub loop_: Option<u32>,
}

/// One script write to a variable (model.py `script_writes`): a value, or `x op= v` as `x op v`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Write {
    Expr(ExprId),
    Arith(ArithOp, ExprId, ExprId),
}

/// The analyses of every unit under one setting of switch const_polls.
#[derive(Clone, Debug, Default)]
pub struct Analyses {
    /// every unit's loops, unit after unit, each unit's in model.py's order (by backward jump)
    pub loops: Vec<Loop>,
    /// per unit: its loops, `loops[start .. start + len]`
    pub unit_loops: Vec<Span>,
    /// per point (`Script::point`): the wait point of a slot standing there
    pub wait_points: Vec<WaitPoint>,
    /// per instruction (global index), camera off and on: (successor outside, head) of a time-loop exit
    pub time_exits: [Vec<Option<(u32, u32)>>; 2],
    /// per point: the heads of the time-only loops (not the camera's) that hold the slot's yield
    pub time_loops_at: Vec<Span>,
    pub time_loop_heads: Vec<u32>,
}

impl Script {
    /// The index of (unit, instruction index 0..=len) in per-point tables: each unit has len + 1 points.
    #[inline]
    pub fn point(&self, u: UnitId, idx: u32) -> usize {
        (self.unit(u).ins_start + u.0 + idx) as usize
    }

    /// The number of points of every unit.
    pub fn points(&self) -> usize {
        self.ins.len() + self.units.len()
    }

    /// model.py `Game.successors`: where control can go from instruction `i` (the end not listed).
    pub fn successors(&self, u: UnitId, i: u32) -> Succ {
        let ins = self.ins(u);
        let x = &ins[i as usize];
        let mut out = Succ::default();
        match x.st {
            Some(Stmt::Jump { cond: None, .. }) => out.push(x.target),
            Some(Stmt::Jump { .. }) => {
                out.push(x.next);
                out.push(x.target);
            }
            _ if x.ends == Some(Ends::Always) => {}
            _ => out.push(x.next),
        }
        if x.yields == Some(Yield::Wait) {
            out.push(Some(i));
        }
        out.keep_below(ins.len() as u32);
        out
    }
}

/// Up to three successors.
#[derive(Copy, Clone, Default, Debug)]
pub struct Succ {
    a: [u32; 3],
    n: u8,
}

impl Succ {
    fn push(&mut self, j: Option<u32>) {
        if let Some(j) = j {
            self.a[self.n as usize] = j;
            self.n += 1;
        } else {
            // keep the place so keep_below drops it: None is never listed
            self.a[self.n as usize] = u32::MAX;
            self.n += 1;
        }
    }
    fn keep_below(&mut self, n: u32) {
        let mut k = 0;
        for i in 0..self.n as usize {
            if self.a[i] < n {
                self.a[k] = self.a[i];
                k += 1;
            }
        }
        self.n = k as u8;
    }
}

impl std::ops::Deref for Succ {
    type Target = [u32];
    fn deref(&self) -> &[u32] {
        &self.a[..self.n as usize]
    }
}

/// What the analyses need besides the IR: first_local (script_key) and the non-fact variables.
pub(crate) struct Ctx<'a> {
    pub sc: &'a Script,
    pub first_local: i32,
    pub nonfacts: &'a FxHashMap<i32, Reason>,
}

fn add<T: PartialEq>(v: &mut Vec<T>, x: T) {
    if !v.contains(&x) {
        v.push(x);
    }
}

/// model.py `Game._test_reads` for one loop: the facts and non-fact reasons its exit tests read.
struct Reads<'a, 'b> {
    cx: &'b Ctx<'a>,
    ins: &'a [Ins],
    body: &'b [u32],
    cp: bool,
    facts: Vec<Read>,
    non: Vec<Reason>,
    seen: Vec<(RefKind, i32)>,
}

impl<'a, 'b> Reads<'a, 'b> {
    /// The assignments in the body that write `kind` `n` (top level and embedded).
    fn writes(&self, kind: RefKind, n: i32) -> Vec<AssignSt> {
        let ast = &self.cx.sc.ast;
        let want = match kind {
            RefKind::Var => Expr::Var(n),
            RefKind::Bit => Expr::Bit(n),
            RefKind::Local => Expr::Local(n),
        };
        let mut out = Vec::new();
        for &k in self.body {
            let Some(st) = self.ins[k as usize].st else { continue };
            if let Stmt::Assign(a) = st {
                if ast.expr(a.lhs) == want {
                    out.push(a);
                }
            }
            ast.walk_stmt(&st, &mut |e| {
                if let Expr::Embed(a) = ast.expr(e) {
                    if ast.expr(a.lhs) == want {
                        out.push(a);
                    }
                }
                true
            });
        }
        out
    }

    fn refer(&mut self, kind: RefKind, n: i32) {
        if self.seen.contains(&(kind, n)) {
            return;
        }
        let mut writes = self.writes(kind, n);
        let ast = &self.cx.sc.ast;
        if !writes.is_empty()
            && self.cp
            && kind != RefKind::Local
            && writes.iter().all(|w| w.op == AssignOp::Set && matches!(ast.expr(w.rhs), Expr::Const(_)))
        {
            writes.clear(); // switch const_polls: read as if the body did not write it
        }
        if !writes.is_empty() {
            for w in writes {
                if w.op != AssignOp::Set && matches!(ast.expr(w.rhs), Expr::Const(_)) {
                    add(&mut self.non, Reason::Counter);
                } else {
                    self.seen.push((kind, n));
                    self.walk(w.rhs);
                    self.seen.pop();
                    if w.op != AssignOp::Set {
                        add(&mut self.non, Reason::Counter);
                    }
                }
            }
            return;
        }
        match kind {
            RefKind::Local => add(&mut self.non, Reason::Local),
            RefKind::Var if is_clock(n) => add(&mut self.non, Reason::Timer),
            RefKind::Var if n == VAR_CAMERA_POS_X => add(&mut self.non, Reason::Camera),
            RefKind::Var if TIME_VARS.contains(&n) => add(&mut self.non, Reason::Timer),
            RefKind::Var if self.cx.nonfacts.contains_key(&n) => add(&mut self.non, self.cx.nonfacts[&n]),
            RefKind::Var => add(&mut self.facts, Read::Fact(Fact::Var(n))),
            RefKind::Bit => add(&mut self.facts, Read::Fact(Fact::Bit(n))),
        }
    }

    fn walk(&mut self, e: ExprId) {
        let ast = &self.cx.sc.ast;
        match ast.expr(e) {
            Expr::Var(n) => self.refer(RefKind::Var, n),
            Expr::Bit(n) => self.refer(RefKind::Bit, n),
            Expr::Local(n) => self.refer(RefKind::Local, n),
            Expr::Index { kind, offset, .. } => {
                match kind {
                    RefKind::Local => add(&mut self.non, Reason::Local),
                    k => add(&mut self.facts, Read::Any(k.as_str())),
                }
                self.walk(offset);
            }
            Expr::BinOp { a, b, .. } | Expr::Cmp { a, b, .. } => {
                self.walk(a);
                self.walk(b);
            }
            Expr::Neg(x) | Expr::Not(x) | Expr::Truth(x) => self.walk(x),
            Expr::Embed(a) => self.walk(a.rhs),
            Expr::Getter { name, args } => {
                match getter_kind(name).unwrap_or(GetterKind::NonFact(Reason::Work)) {
                    GetterKind::NonFact(r) => add(&mut self.non, r),
                    GetterKind::Fact => self.fact_getter(name, ast.kids(args)),
                }
            }
            _ => {}
        }
    }

    fn fact_getter(&mut self, name: Sym, args: &[ExprId]) {
        let ast = &self.cx.sc.ast;
        let o = args.first().and_then(|&a| ast.constant(a));
        let f = &mut self.facts;
        match name {
            known::GET_OBJECT_OWNER => add(f, o.map_or(Read::Any("owner"), |o| Read::Fact(Fact::Owner(o)))),
            known::GET_OBJECT_STATE => add(f, o.map_or(Read::Any("state"), |o| Read::Fact(Fact::State(o)))),
            known::CLASS_OF_IS => {
                let cs: Vec<Option<i32>> = match args.get(1).map(|&a| ast.expr(a)) {
                    Some(Expr::List(items)) => ast.kids(items).iter().map(|&c| ast.constant(c)).collect(),
                    _ => vec![None],
                };
                for c in cs {
                    add(f, match (o, c) {
                        (Some(o), Some(c)) => Read::Fact(Fact::Class(o, c & 0x7F)),
                        (Some(o), None) => Read::ClassOf(o),
                        (None, _) => Read::Any("class"),
                    });
                }
            }
            known::IS_SCRIPT_RUNNING => match o {
                Some(o) if o >= self.cx.first_local => add(f, Read::Fact(Fact::Local(o))),
                Some(o) => add(f, Read::Fact(Fact::Script(o))),
                None => {
                    add(f, Read::Any("script"));
                    add(f, Read::Any("local"));
                }
            },
            known::GET_INVENTORY_COUNT => add(f, Read::Any("owner")),
            known::GET_VERB_ENTRY_POINT => {
                add(f, o.map_or(Read::Any("owner"), |o| Read::Fact(Fact::Owner(o))));
                add(f, Read::Fact(Fact::Room));
            }
            _ => unreachable!("a fact getter model.py's _test_reads does not name"),
        }
    }
}

fn sort_by_str<T: fmt::Display>(v: &mut [T]) {
    v.sort_by_cached_key(|x| x.to_string());
}

/// model.py `Game.loops` for one unit: one loop per backward jump whose natural loop holds a yield.
pub(crate) fn unit_loops(cx: &Ctx, u: UnitId, cp: bool) -> Vec<Loop> {
    let sc = cx.sc;
    let ins = sc.ins(u);
    let n = ins.len();
    // predecessors, as a CSR table
    let mut count = vec![0u32; n + 1];
    let succs: Vec<Succ> = (0..n as u32).map(|i| sc.successors(u, i)).collect();
    for s in &succs {
        for &j in s.iter() {
            count[j as usize + 1] += 1;
        }
    }
    for i in 0..n {
        count[i + 1] += count[i];
    }
    let mut fill = count.clone();
    let mut preds = vec![0u32; count[n] as usize];
    for (i, s) in succs.iter().enumerate() {
        for &j in s.iter() {
            preds[fill[j as usize] as usize] = i as u32;
            fill[j as usize] += 1;
        }
    }
    let mut out = Vec::new();
    let mut in_body = vec![false; n];
    for i in 0..n {
        let x = &ins[i];
        let (Some(Stmt::Jump { .. }), Some(head)) = (x.st, x.target) else { continue };
        if head as usize > i {
            continue;
        }
        let mut body = vec![head, i as u32];
        in_body[head as usize] = true;
        in_body[i] = true;
        let mut todo = vec![i as u32];
        while let Some(q) = todo.pop() {
            for &p in &preds[count[q as usize] as usize..count[q as usize + 1] as usize] {
                if !in_body[p as usize] {
                    in_body[p as usize] = true;
                    body.push(p);
                    todo.push(p);
                }
            }
        }
        body.sort_unstable();
        body.dedup();
        let yields = body.iter().any(|&k| ins[k as usize].yields.is_some());
        if !yields {
            for &k in &body {
                in_body[k as usize] = false;
            }
            continue;
        }
        let mut exits = Vec::new();
        for &k in &body {
            let y = &ins[k as usize];
            if let Some(Stmt::Jump { cond: Some(_), .. }) = y.st {
                let outside: Vec<Option<u32>> = [y.next, y.target]
                    .into_iter()
                    .filter(|j| match j {
                        None => true,
                        Some(j) => *j as usize >= n || !in_body[*j as usize],
                    })
                    .collect();
                if outside.len() == 1 {
                    exits.push((k, outside[0].unwrap_or(n as u32)));
                }
            }
        }
        let mut r = Reads { cx, ins, body: &body, cp, facts: Vec::new(), non: Vec::new(), seen: Vec::new() };
        for &(k, _) in &exits {
            if let Some(Stmt::Jump { cond: Some(c), .. }) = ins[k as usize].st {
                r.walk(c);
            }
        }
        let (mut reads, mut non) = (r.facts, r.non);
        sort_by_str(&mut reads);
        sort_by_str(&mut non);
        let kind = if exits.is_empty() {
            LoopKind::Ambient
        } else if !reads.is_empty() {
            LoopKind::Facts
        } else {
            LoopKind::Time
        };
        for &k in &body {
            in_body[k as usize] = false;
        }
        out.push(Loop { head, body, exits, kind, reads, non_facts: non });
    }
    out
}

/// model.py `Game.wait_point`, given the unit's loops.
pub(crate) fn wait_point(sc: &Script, u: UnitId, idx: u32, loops: &[Loop]) -> WaitPoint {
    let ins = sc.ins(u);
    let n = ins.len() as u32;
    let x = (idx < n).then(|| &ins[idx as usize]);
    let prev = (idx > 0 && idx <= n && ins[idx as usize - 1].next == Some(idx)).then(|| &ins[idx as usize - 1]);
    let innermost = |i: u32| -> Option<u32> {
        let mut best: Option<(usize, u32)> = None;
        for (k, lp) in loops.iter().enumerate() {
            if lp.contains(i as i64) && best.is_none_or(|(len, _)| lp.body.len() < len) {
                best = Some((lp.body.len(), k as u32));
            }
        }
        best.map(|b| b.1)
    };
    if let Some(x) = x.filter(|x| x.yields == Some(Yield::Wait)) {
        return WaitPoint { kind: WaitKind::Wait, op: x.st.and_then(|s| s.name()).unwrap(), loop_: innermost(idx) };
    }
    if let Some(p) = prev {
        match p.yields {
            Some(Yield::Frame) => return WaitPoint { kind: WaitKind::Frame, op: known::EMPTY, loop_: innermost(idx - 1) },
            Some(Yield::Delay) => return WaitPoint { kind: WaitKind::Delay, op: known::EMPTY, loop_: innermost(idx - 1) },
            _ => {}
        }
        if let Some(name) = p.st.and_then(|s| s.name()) {
            return WaitPoint { kind: WaitKind::Parked, op: name, loop_: None };
        }
    }
    if idx == 0 || sc.unit(u).entries.iter().any(|e| e.1 == idx) {
        return WaitPoint { kind: WaitKind::Start, op: known::EMPTY, loop_: None };
    }
    WaitPoint { kind: WaitKind::Other, op: known::EMPTY, loop_: None }
}

/// The analyses of every unit under one const_polls setting.
pub(crate) fn analyse(cx: &Ctx, cp: bool) -> Analyses {
    let sc = cx.sc;
    let mut a = Analyses {
        loops: Vec::new(),
        unit_loops: Vec::with_capacity(sc.units.len()),
        wait_points: Vec::with_capacity(sc.points()),
        time_exits: [vec![None; sc.ins.len()], vec![None; sc.ins.len()]],
        time_loops_at: Vec::with_capacity(sc.points()),
        time_loop_heads: Vec::new(),
    };
    for u in sc.ids() {
        let loops = unit_loops(cx, u, cp);
        let start = sc.unit(u).ins_start as usize;
        // time exits: inner loops last, so they win (a stable sort by size, largest first)
        let mut order: Vec<usize> = (0..loops.len()).collect();
        order.sort_by_key(|&k| std::cmp::Reverse(loops[k].body.len()));
        for (camera, table) in a.time_exits.iter_mut().enumerate() {
            for &k in &order {
                let lp = &loops[k];
                if lp.kind == LoopKind::Time && lp.camera() == (camera == 1) {
                    for &(j, succ) in &lp.exits {
                        table[start + j as usize] = Some((succ, lp.head));
                    }
                }
            }
        }
        for idx in 0..=sc.unit(u).ins_len {
            let wp = wait_point(sc, u, idx, &loops);
            let y = if wp.kind == WaitKind::Wait { idx as i64 } else { idx as i64 - 1 };
            let mut heads: Vec<u32> = loops
                .iter()
                .filter(|lp| lp.kind == LoopKind::Time && !lp.camera() && lp.contains(y))
                .map(|lp| lp.head)
                .collect();
            heads.sort_unstable();
            let s = a.time_loop_heads.len() as u32;
            a.time_loop_heads.extend_from_slice(&heads);
            a.time_loops_at.push(Span { start: s, len: heads.len() as u32 });
            a.wait_points.push(wp);
        }
        let s = a.loops.len() as u32;
        a.unit_loops.push(Span { start: s, len: loops.len() as u32 });
        a.loops.extend(loops);
    }
    a
}

/// model.py `script_writes`: every value the scripts write to each variable, by variable in the order
/// first written: an assignment's value (`x op= v` as `x op v`), an embedded opcode's value, and
/// setVarRange's constants. A write to an indexed variable is left out.
pub fn script_writes(sc: &Script) -> Vec<(i32, Vec<Write>)> {
    let ast = &sc.ast;
    let mut out: Vec<(i32, Vec<Write>)> = Vec::new();
    let mut at: FxHashMap<i32, usize> = FxHashMap::default();
    let mut put = |n: i32, w: Write| {
        let k = *at.entry(n).or_insert_with(|| {
            out.push((n, Vec::new()));
            out.len() - 1
        });
        out[k].1.push(w);
    };
    for x in &sc.ins {
        let Some(st) = x.st else { continue };
        ast.walk_stmt(&st, &mut |e| {
            if let Expr::Embed(a) = ast.expr(e) {
                if let Expr::Var(n) = ast.expr(a.lhs) {
                    put(n, Write::Expr(a.rhs));
                }
            }
            true
        });
        match st {
            Stmt::Assign(a) => {
                if let Expr::Var(n) = ast.expr(a.lhs) {
                    put(n, match a.op.arith() {
                        None => Write::Expr(a.rhs),
                        Some(op) => Write::Arith(op, a.lhs, a.rhs),
                    });
                }
            }
            Stmt::Op { name: known::SET_VAR_RANGE, args, .. } => {
                let args = ast.kids(args);
                if let Expr::Var(n) = ast.expr(args[0]) {
                    let Expr::List(items) = ast.expr(args[2]) else { panic!("setVarRange without a list") };
                    for (k, &e) in ast.kids(items).iter().enumerate() {
                        put(n + k as i32, Write::Expr(e));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// model.py `non_fact_vars`: variables whose every script write is unknown by the machine's rules (an
/// unknown getter, a clock, or a variable already in the set), the least fixpoint; {var: one reason,
/// the most common, the first seen on a tie}.
pub fn non_fact_vars(sc: &Script, writes: &[(i32, Vec<Write>)]) -> FxHashMap<i32, Reason> {
    let ast = &sc.ast;
    let mut out: FxHashMap<i32, Reason> = FxHashMap::default();
    let why = |out: &FxHashMap<i32, Reason>, w: &Write| -> Option<Reason> {
        let mut found = None;
        let mut f = |e: ExprId| {
            let r = match ast.expr(e) {
                Expr::Getter { name, .. } => match getter_kind(name) {
                    Some(GetterKind::NonFact(r)) => Some(r),
                    _ => None,
                },
                Expr::Var(n) if is_clock(n) => Some(Reason::Timer),
                Expr::Var(n) if out.contains_key(&n) => Some(Reason::Var(n)),
                _ => None,
            };
            found = r;
            r.is_none()
        };
        match *w {
            Write::Expr(e) => {
                ast.walk(e, &mut f);
            }
            Write::Arith(_, a, b) => {
                let _ = ast.walk(a, &mut f) && ast.walk(b, &mut f);
            }
        }
        found
    };
    let mut changed = true;
    while changed {
        changed = false;
        for (n, values) in writes {
            if out.contains_key(n) {
                continue;
            }
            let reasons: Vec<Option<Reason>> = values.iter().map(|w| why(&out, w)).collect();
            if reasons.iter().all(|r| r.is_some()) {
                // Counter.most_common(1): the highest count, the first seen on a tie
                let mut counts: Vec<(Reason, usize)> = Vec::new();
                for r in reasons.into_iter().flatten() {
                    match counts.iter_mut().find(|c| c.0 == r) {
                        Some(c) => c.1 += 1,
                        None => counts.push((r, 1)),
                    }
                }
                let best = counts.iter().fold(None::<(Reason, usize)>, |b, &c| match b {
                    Some(b) if b.1 >= c.1 => Some(b),
                    _ => Some(c),
                });
                out.insert(*n, best.unwrap().0);
                changed = true;
            }
        }
    }
    out
}
