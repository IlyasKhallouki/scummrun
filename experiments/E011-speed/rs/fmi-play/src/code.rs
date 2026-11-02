//! trace_explain.py as strict.py uses it: the engine's runs (`RunR`) and explain log (`Explained`, `Logged`), the
//! IR side over the scumm-ir crate (`Code`: units by the engine's slot, offsets, control flow, reachability, write,
//! start, yield and end sites), and Lemma X and B (`steps_of`, `taken`, `first_off`, `last_test`, `slice_test`,
//! `trace_back`, `getter_value`, `wait_values`, `cause_class`).

use crate::pyj::J;
use crate::val::Unit;
use fmi_core::{Key, Obs};
use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};
use scumm_ir::ast::{AssignOp, Ends, Expr, ExprId, Ins, RefKind, Stmt, UnitId, UnitKey, Yield};
use scumm_ir::Game;
use std::sync::{Arc, Mutex};

pub const LOCAL_BIT: u32 = 0x4000;
pub const BIT_BIT: u32 = 0x8000;
pub const SS_DEAD: i32 = 0;
pub const SS_PAUSED: i32 = 1;
pub const SS_RUNNING: i32 = 2;
const VAR_ROOM: i64 = 4;
const VAR_ROOM_RESOURCE: i64 = 22;
const ROOM_VARS: [i64; 6] = [VAR_ROOM, 17, 18, VAR_ROOM_RESOURCE, 70, 72];
const VAR_USERPUT: i64 = 53;
const VAR_CURSORSTATE: i64 = 52;
pub const CLOCKS: [i64; 6] = [11, 12, 13, 14, 46, 47];

/// A run's sequence number; a part j of a run that passed a wait (strict.PART_STEP) is seq + j / 1000.
pub type Seq = i64;
pub const SEQ_SCALE: i64 = 1000;
/// strict.Track's seq -1: matched before the branch could see
pub const SEQ_BEFORE: Seq = -SEQ_SCALE;

pub fn seq_j(s: Seq) -> J {
    if s % SEQ_SCALE == 0 {
        J::Int(s / SEQ_SCALE)
    } else {
        J::Float((s / SEQ_SCALE) as f64 + (s % SEQ_SCALE) as f64 * 0.001)
    }
}

/// trace_explain.Run: one slot run of the bridge's segment buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunR {
    pub frame: u64,
    pub after: bool,
    pub seq: Seq,
    pub depth: i32,
    pub slot: i32,
    pub where_: i32,
    pub number: i32,
    pub room: i32,
    pub code0: i64,
    pub code1: i64,
    pub status: i32,
    pub frozen: i32,
    pub delay: i32,
    pub ops: u64,
    pub unit: Option<Unit>,
}

impl RunR {
    pub fn of(r: &fmi_core::Run) -> RunR {
        RunR {
            frame: r.frame,
            after: r.after,
            seq: r.seq as i64 * SEQ_SCALE,
            depth: r.depth,
            slot: r.slot,
            where_: r.where_,
            number: r.number,
            room: r.room,
            code0: r.code0,
            code1: r.code1,
            status: r.status,
            frozen: r.frozen,
            delay: r.delay,
            ops: r.ops,
            unit: Unit::of(r.where_, r.number),
        }
    }
    /// a part (9.9) or a whole run
    pub fn is_part(&self) -> bool {
        self.seq % SEQ_SCALE != 0
    }
    pub fn whole(&self) -> Seq {
        self.seq / SEQ_SCALE * SEQ_SCALE
    }
    /// R_k: frame k's runs before its decision point and frame k-1's after it
    pub fn k(&self) -> u64 {
        if self.after {
            self.frame + 1
        } else {
            self.frame
        }
    }
}

/// One logged opcode (trace_explain.Op).
#[derive(Clone, Debug, PartialEq)]
pub struct OpL {
    pub code: i64,
    pub opcode: i64,
    pub reads: Vec<(u32, i32)>,
    pub writes: Vec<(u32, i32)>,
}

/// One explained run (trace_explain.Logged).
#[derive(Clone, Debug, PartialEq)]
pub struct Logged {
    pub seq: Seq,
    pub frame: u64,
    pub after: bool,
    pub slot: i32,
    pub where_: i32,
    pub number: i32,
    pub room: i32,
    pub code0: i64,
    pub ops: Vec<OpL>,
    pub code1: Option<i64>,
    pub status: Option<i32>,
    pub frozen: Option<i32>,
    pub delay: Option<i32>,
}

/// An explain entry with a part's seq (the bridge's, or strict.part_entries').
#[derive(Clone, Debug, PartialEq)]
pub enum Ent {
    B {
        seq: Seq,
        frame: u64,
        after: bool,
        slot: i32,
        where_: i32,
        number: i32,
        room: i32,
        code0: i64,
    },
    O {
        seq: Seq,
        code: i64,
        opcode: i64,
    },
    R {
        seq: Seq,
        var: u32,
        value: i32,
    },
    W {
        seq: Seq,
        var: u32,
        value: i32,
    },
    E {
        seq: Seq,
        code1: i64,
        status: i32,
        frozen: i32,
        delay: i32,
    },
}

impl Ent {
    pub fn of(e: &fmi_core::Explain) -> Ent {
        use fmi_core::Explain as X;
        match *e {
            X::Begin {
                seq,
                frame,
                after,
                slot,
                where_,
                number,
                room,
                code0,
            } => Ent::B {
                seq: seq as i64 * SEQ_SCALE,
                frame,
                after,
                slot,
                where_,
                number,
                room,
                code0,
            },
            X::Op { seq, code, opcode } => Ent::O {
                seq: seq as i64 * SEQ_SCALE,
                code,
                opcode: opcode as i64,
            },
            X::Read { seq, var, value } => Ent::R {
                seq: seq as i64 * SEQ_SCALE,
                var,
                value,
            },
            X::Write { seq, var, value } => Ent::W {
                seq: seq as i64 * SEQ_SCALE,
                var,
                value,
            },
            X::End {
                seq,
                code1,
                status,
                frozen,
                delay,
            } => Ent::E {
                seq: seq as i64 * SEQ_SCALE,
                code1,
                status,
                frozen,
                delay,
            },
        }
    }
    pub fn seq(&self) -> Seq {
        match *self {
            Ent::B { seq, .. } | Ent::O { seq, .. } | Ent::R { seq, .. } | Ent::W { seq, .. } | Ent::E { seq, .. } => {
                seq
            }
        }
    }
}

/// trace_explain.Explained: the explain log by run, in the order the runs began.
#[derive(Clone, Debug, Default)]
pub struct Explained {
    pub runs: IndexMap<Seq, Logged>,
}

impl Explained {
    pub fn add(&mut self, entries: &[Ent]) {
        for e in entries {
            match *e {
                Ent::B {
                    seq,
                    frame,
                    after,
                    slot,
                    where_,
                    number,
                    room,
                    code0,
                } => {
                    self.runs.insert(
                        seq,
                        Logged {
                            seq,
                            frame,
                            after,
                            slot,
                            where_,
                            number,
                            room,
                            code0,
                            ops: Vec::new(),
                            code1: None,
                            status: None,
                            frozen: None,
                            delay: None,
                        },
                    );
                }
                Ent::O { seq, code, opcode } => {
                    if let Some(r) = self.runs.get_mut(&seq) {
                        r.ops.push(OpL {
                            code,
                            opcode,
                            reads: vec![],
                            writes: vec![],
                        });
                    }
                }
                Ent::R { seq, var, value } | Ent::W { seq, var, value } => {
                    if let Some(r) = self.runs.get_mut(&seq) {
                        if r.ops.is_empty() {
                            r.ops.push(OpL {
                                code: -1,
                                opcode: -1,
                                reads: vec![],
                                writes: vec![],
                            });
                        }
                        let op = r.ops.last_mut().unwrap();
                        if matches!(e, Ent::R { .. }) {
                            op.reads.push((var, value));
                        } else {
                            op.writes.push((var, value));
                        }
                    }
                }
                Ent::E {
                    seq,
                    code1,
                    status,
                    frozen,
                    delay,
                } => {
                    if let Some(r) = self.runs.get_mut(&seq) {
                        r.code1 = Some(code1);
                        r.status = Some(status);
                        r.frozen = Some(frozen);
                        r.delay = Some(delay);
                    }
                }
            }
        }
    }
}

// --- the IR side ----------------------------------------------------------------------------------------------------

/// trace_explain.Code over the scumm-ir crate's game.
pub struct Code {
    pub game: Arc<Game>,
    /// the reading's facts (its domains' keys): a variable or bit outside them is not a fact of the reading
    pub facts: Option<Arc<FxHashSet<Key>>>,
    reach: Mutex<FxHashMap<(UnitId, Vec<u32>, bool), FxHashMap<u32, bool>>>,
}

pub type UKey = UnitId;

impl Code {
    pub fn new(game: Arc<Game>) -> Code {
        Code {
            game,
            facts: None,
            reach: Mutex::new(FxHashMap::default()),
        }
    }
    pub fn with_facts(&self, facts: Arc<FxHashSet<Key>>) -> Code {
        Code {
            game: self.game.clone(),
            facts: Some(facts),
            reach: Mutex::new(FxHashMap::default()),
        }
    }

    /// Code.key: the unit of a slot (where, number, room resource), if the game has it.
    pub fn key(&self, where_: i32, number: i32, room: i32) -> Option<UKey> {
        let k = match where_ {
            fmi_core::WIO_GLOBAL => UnitKey::Global(number),
            fmi_core::WIO_LOCAL => UnitKey::Local(room, number),
            fmi_core::WIO_ROOM if number == fmi_core::ENCD => UnitKey::Entry(room),
            fmi_core::WIO_ROOM if number == fmi_core::EXCD => UnitKey::Exit(room),
            fmi_core::WIO_INVENTORY | fmi_core::WIO_ROOM | fmi_core::WIO_FLOBJECT => UnitKey::Obj(number),
            _ => return None,
        };
        self.game.script.id(k)
    }

    /// Code.key_of_name
    pub fn key_of_name(&self, name: &str, room: i32) -> Option<UKey> {
        let (kind, n) = match name.split_once(':') {
            Some((k, n)) => (k, n.parse::<i32>().ok()),
            None => (name, None),
        };
        let k = match kind {
            "script" => UnitKey::Global(n?),
            "local" => UnitKey::Local(room, n?),
            "obj" => UnitKey::Obj(n?),
            "entry" => UnitKey::Entry(room),
            "exit" => UnitKey::Exit(room),
            _ => return None,
        };
        self.game.script.id(k)
    }

    #[inline]
    pub fn ins(&self, key: UKey) -> &[Ins] {
        self.game.script.ins(key)
    }
    #[inline]
    pub fn index(&self, key: UKey, code: i64) -> Option<u32> {
        self.game.script.at(key, code as i32)
    }
    pub fn name(&self, y: scumm_ir::Sym) -> &str {
        self.game.script.syms.str(y)
    }
    pub fn op_name<'a>(&'a self, st: &Stmt) -> Option<&'a str> {
        match st {
            Stmt::Op { name, .. } => Some(self.name(*name)),
            _ => None,
        }
    }
    pub fn head(&self, st: Option<&Stmt>) -> String {
        match st {
            Some(st) => scumm_ir::show::head(&self.game.script.ast, &self.game.script.syms, st),
            None => "?".into(),
        }
    }
    pub fn show(&self, e: ExprId) -> String {
        scumm_ir::show::show_expr(&self.game.script.ast, &self.game.script.syms, e)
    }
    pub fn expr(&self, e: ExprId) -> Expr {
        self.game.script.ast.expr(e)
    }
    /// ir.Jump with a condition
    pub fn is_cond_jump(x: &Ins) -> bool {
        matches!(x.st, Some(Stmt::Jump { cond: Some(_), .. }))
    }

    /// Code.succ: where control can go after instruction i (len: the script's end).
    pub fn succ(&self, key: UKey, i: u32, cross_yields: bool) -> Vec<u32> {
        let x = &self.ins(key)[i as usize];
        if matches!(x.yields, Some(Yield::Frame) | Some(Yield::Delay)) {
            return if cross_yields {
                x.next.into_iter().collect()
            } else {
                vec![]
            };
        }
        let mut out = Vec::new();
        if x.yields == Some(Yield::Wait) && cross_yields {
            out.push(i);
        }
        if matches!(x.st, Some(Stmt::Jump { cond: None, .. })) {
            return x.target.into_iter().collect();
        }
        if x.ends == Some(Ends::Always) {
            return vec![];
        }
        if let Some(n) = x.next {
            out.push(n);
        }
        if let Some(t) = x.target {
            out.push(t);
        }
        out
    }

    /// Code.reachable: some target is reachable from `start` (inclusive).
    pub fn reachable(&self, key: UKey, start: Option<u32>, targets: &[u32], cross_yields: bool) -> bool {
        let Some(start) = start else { return false };
        if targets.is_empty() {
            return false;
        }
        let mk = (key, targets.to_vec(), cross_yields);
        if let Some(m) = self.reach.lock().unwrap().get(&mk) {
            if let Some(&b) = m.get(&start) {
                return b;
            }
        }
        let n = self.ins(key).len() as u32;
        let mut seen = FxHashSet::default();
        seen.insert(start);
        let mut todo = vec![start];
        let mut found = false;
        while let Some(i) = todo.pop() {
            if targets.contains(&i) {
                found = true;
                break;
            }
            if i >= n {
                continue;
            }
            for j in self.succ(key, i, cross_yields) {
                if seen.insert(j) {
                    todo.push(j);
                }
            }
        }
        self.reach.lock().unwrap().entry(mk).or_default().insert(start, found);
        found
    }

    fn walk_any(&self, st: &Stmt, f: &mut impl FnMut(Expr) -> bool) -> bool {
        let ast = &self.game.script.ast;
        let mut hit = false;
        ast.walk_stmt(st, &mut |e| {
            if f(ast.expr(e)) {
                hit = true;
                return false;
            }
            true
        });
        hit
    }

    /// Code.write_sites: the instructions that can write `fact` (to `value` when constant there).
    pub fn write_sites(&self, key: UKey, fact: &str, value: Option<i64>, direct: bool) -> Vec<u32> {
        let ast = &self.game.script.ast;
        let (kind, rest) = fact.split_once(':').unwrap_or((fact, ""));
        let mut out = Vec::new();
        for (i, x) in self.ins(key).iter().enumerate() {
            let Some(st) = &x.st else { continue };
            let i = i as u32;
            match kind {
                "var" | "bit" => {
                    let n: i64 = rest.parse().unwrap_or(-1);
                    let want = |e: Expr| match e {
                        Expr::Var(v) => kind == "var" && v as i64 == n,
                        Expr::Bit(v) => kind == "bit" && v as i64 == n,
                        _ => false,
                    };
                    if self.walk_any(st, &mut |e| matches!(e, Expr::Embed(a) if want(ast.expr(a.lhs)))) {
                        out.push(i);
                    }
                    match st {
                        Stmt::Assign(a) => {
                            let lhs = ast.expr(a.lhs);
                            let idx_kind = matches!(lhs, Expr::Index { kind: k, .. } if (k == RefKind::Var && kind == "var") || (k == RefKind::Bit && kind == "bit"));
                            if want(lhs) || (!direct && idx_kind) {
                                let c = ast.constant(a.rhs);
                                if value.is_none()
                                    || a.op != AssignOp::Set
                                    || c.is_none()
                                    || c.map(|v| v as i64) == value
                                {
                                    out.push(i);
                                }
                            }
                        }
                        Stmt::Op { name, args, .. } => {
                            let nm = self.name(*name);
                            let a = ast.kids(*args);
                            if nm == "setVarRange" && !a.is_empty() {
                                let first = ast.expr(a[0]);
                                let base = match (first, kind) {
                                    (Expr::Var(v), "var") | (Expr::Bit(v), "bit") => Some(v as i64),
                                    _ => None,
                                };
                                if let Some(b0) = base {
                                    let items = a.get(2).map(|&e| match ast.expr(e) {
                                        Expr::List(s) => s.len as i64,
                                        _ => 0,
                                    });
                                    if b0 <= n && n < b0 + items.unwrap_or(0) {
                                        out.push(i);
                                    }
                                }
                            } else if kind == "var" && ROOM_VARS.contains(&n) && is_room_change(nm) {
                                out.push(i);
                            } else if kind == "var" && cursor_var(nm) == Some(n) {
                                out.push(i);
                            }
                        }
                        _ => {}
                    }
                }
                "state" | "owner" | "class" => {
                    let o: i64 = rest.split(':').next().unwrap_or("").parse().unwrap_or(-1);
                    let names: &[&str] = match kind {
                        "state" => &["setState", "drawObject", "pickupObject"],
                        "owner" => &["setOwnerOf", "pickupObject"],
                        _ => &["setClass", "pickupObject"],
                    };
                    if let Stmt::Op { name, args, .. } = st {
                        let a = ast.kids(*args);
                        if names.contains(&self.name(*name)) && !a.is_empty() {
                            let c = ast.constant(a[0]);
                            if (!direct && c.is_none()) || c.map(|v| v as i64) == Some(o) {
                                out.push(i);
                            }
                        }
                    }
                }
                "verb" => {
                    if let Stmt::Op { name, args, .. } = st {
                        let nm = self.name(*name);
                        let a = ast.kids(*args);
                        if ["VerbOps", "saveVerbs", "restoreVerbs"].contains(&nm) {
                            let v: i64 = rest.parse().unwrap_or(-1);
                            if a.is_empty()
                                || ast.constant(a[0]).is_none()
                                || ast.constant(a[0]).map(|c| c as i64) == Some(v)
                            {
                                out.push(i);
                            }
                        }
                    }
                }
                "room" => {
                    if let Stmt::Op { name, .. } = st {
                        if is_room_change(self.name(*name)) {
                            out.push(i);
                        }
                    }
                }
                "cutscene" => {
                    if let Stmt::Op { name, .. } = st {
                        if ["cutscene", "endCutscene"].contains(&self.name(*name)) {
                            out.push(i);
                        }
                    }
                }
                _ => {}
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Code.start_sites: the instructions that can start unit `target` (`direct`: a constant number or object only;
    /// else only those without).
    pub fn start_sites(&self, key: UKey, target: UnitKey, direct: bool) -> Vec<u32> {
        let ast = &self.game.script.ast;
        let mut out = Vec::new();
        for (i, x) in self.ins(key).iter().enumerate() {
            match &x.st {
                Some(Stmt::Start { name, script, obj, .. }) => {
                    let n = ast.constant(*script);
                    if self.name(*name) == "startObject" {
                        let oc = obj.and_then(|o| ast.constant(o));
                        let cons = oc.is_some();
                        if let UnitKey::Obj(t) = target {
                            if cons == direct && (!cons || oc == Some(t)) {
                                out.push(i as u32);
                            }
                        }
                    } else {
                        let last = match target {
                            UnitKey::Global(n) => Some(n),
                            UnitKey::Local(_, n) => Some(n),
                            _ => None,
                        };
                        if let Some(t) = last {
                            if n.is_some() == direct && (n.is_none() || n == Some(t)) {
                                out.push(i as u32);
                            }
                        }
                    }
                }
                // (a doSentence starts the sentence script, target ("sentence",), which no slot's unit is)
                _ => {}
            }
        }
        out
    }

    /// Code.yield_sites: the instructions after which a slot of `key` waits at offset `w`.
    pub fn yield_sites(&self, key: UKey, w: i64) -> Vec<u32> {
        let i = self.index(key, w);
        let mut out = Vec::new();
        for (j, x) in self.ins(key).iter().enumerate() {
            let j = j as u32;
            if matches!(x.yields, Some(Yield::Frame) | Some(Yield::Delay)) && x.next == i {
                out.push(j);
            } else if x.yields == Some(Yield::Wait) && Some(j) == i {
                out.push(j);
            } else if matches!(x.st, Some(Stmt::Start { .. })) && x.next == i {
                out.push(j);
            }
        }
        out
    }

    /// Code.end_sites: where a slot of `key` surely ends.
    pub fn end_sites(&self, key: UKey) -> Vec<u32> {
        let ins = self.ins(key);
        let mut out = vec![ins.len() as u32];
        out.extend(
            ins.iter()
                .enumerate()
                .filter(|(_, x)| matches!(x.ends, Some(Ends::Always) | Some(Ends::Room)))
                .map(|(j, _)| j as u32),
        );
        out.sort();
        out.dedup();
        out
    }
}

pub fn is_room_change(name: &str) -> bool {
    matches!(name, "loadRoom" | "loadRoomWithEgo" | "actorFollowCamera")
}

fn cursor_var(name: &str) -> Option<i64> {
    match name {
        "UserputOn" | "UserputOff" | "UserputSoftOn" | "UserputSoftOff" => Some(VAR_USERPUT),
        "CursorShow" | "CursorHide" | "CursorSoftOn" | "CursorSoftOff" => Some(VAR_CURSORSTATE),
        _ => None,
    }
}

// --- the first opcode off the path and the slice ---------------------------------------------------------------------

/// One executed opcode of a unit, placed in the IR (trace_explain.Step).
#[derive(Clone, Copy, Debug)]
pub struct StepX<'a> {
    pub run: usize,
    pub k: usize,
    pub i: Option<u32>,
    pub op: &'a OpL,
    pub lg: &'a Logged,
}

pub fn steps_of<'a>(code: &Code, key: UKey, runs: &[&'a Logged]) -> Vec<StepX<'a>> {
    let mut out = Vec::new();
    for (n, r) in runs.iter().enumerate() {
        for (k, op) in r.ops.iter().enumerate() {
            out.push(StepX {
                run: n,
                k,
                i: if op.code >= 0 { code.index(key, op.code) } else { None },
                op,
                lg: r,
            });
        }
    }
    out
}

/// trace_explain.taken: for the conditional jump at steps[n], (the side taken, the other side).
pub fn taken(code: &Code, key: UKey, steps: &[StepX], n: usize) -> Option<(Option<u32>, Option<u32>)> {
    let s = &steps[n];
    let x = &code.ins(key)[s.i? as usize];
    let nxt = steps.get(n + 1).filter(|t| t.run == s.run);
    let len = code.ins(key).len() as u32;
    match nxt {
        None => {
            if x.target == Some(len) {
                Some((x.target, x.next))
            } else {
                None
            }
        }
        Some(t) => {
            if t.i == x.target && t.i != x.next {
                Some((x.target, x.next))
            } else if t.i == x.next && t.i != x.target {
                Some((x.next, x.target))
            } else {
                None
            }
        }
    }
}

/// trace_explain.first_off (Lemma X)
pub fn first_off(code: &Code, key: UKey, steps: &[StepX], targets: &[u32], cross_yields: bool) -> Option<usize> {
    let ins = code.ins(key);
    for (n, s) in steps.iter().enumerate() {
        let Some(i) = s.i else { continue };
        let x = &ins[i as usize];
        if x.yields == Some(Yield::Wait) && targets.contains(&i) {
            let nxt = steps.get(n + 1).filter(|t| t.run == s.run);
            if nxt.is_some() {
                return Some(n);
            }
            continue;
        }
        if !Code::is_cond_jump(x) {
            continue;
        }
        let Some((went, other)) = taken(code, key, steps, n) else {
            continue;
        };
        if !code.reachable(key, went, targets, cross_yields) && code.reachable(key, other, targets, cross_yields) {
            return Some(n);
        }
    }
    None
}

/// trace_explain.last_test
pub fn last_test(code: &Code, key: UKey, steps: &[StepX]) -> Option<usize> {
    let ins = code.ins(key);
    (0..steps.len())
        .rev()
        .find(|&n| steps[n].i.is_some_and(|i| Code::is_cond_jump(&ins[i as usize])))
}

/// trace_explain.Value
#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    pub what: String,
    pub value: J,
    pub source: String,
    pub kind: String,
}

impl Value {
    pub fn new(what: impl Into<String>, value: J, source: impl Into<String>, kind: impl Into<String>) -> Value {
        Value {
            what: what.into(),
            value,
            source: source.into(),
            kind: kind.into(),
        }
    }
    pub fn j(&self) -> J {
        J::Obj(vec![
            ("what".into(), J::Str(self.what.clone())),
            ("value".into(), self.value.clone()),
            ("source".into(), J::Str(self.source.clone())),
            ("kind".into(), J::Str(self.kind.clone())),
        ])
    }
}

/// model.GETTERS' kind of a getter name ("unknown" when not a getter)
pub fn getter_kind(code: &Code, name: scumm_ir::Sym) -> String {
    match scumm_ir::analysis::getter_kind(name) {
        Some(scumm_ir::analysis::GetterKind::Fact) => "fact".into(),
        Some(scumm_ir::analysis::GetterKind::NonFact(r)) => r.as_str().into_owned(),
        None => {
            let _ = code;
            "unknown".into()
        }
    }
}

/// trace_explain.leaves: the variables, locals, bits, getters and indexes an expression (or statement) reads.
pub fn leaves_expr(code: &Code, e: ExprId) -> Vec<ExprId> {
    let ast = &code.game.script.ast;
    let mut out = Vec::new();
    ast.walk(e, &mut |y| {
        if matches!(
            ast.expr(y),
            Expr::Var(_) | Expr::Local(_) | Expr::Bit(_) | Expr::Getter { .. } | Expr::Index { .. }
        ) {
            out.push(y);
        }
        true
    });
    out
}

pub fn leaves_stmt(code: &Code, st: &Stmt) -> Vec<ExprId> {
    let ast = &code.game.script.ast;
    let mut out = Vec::new();
    ast.walk_stmt(st, &mut |y| {
        if matches!(
            ast.expr(y),
            Expr::Var(_) | Expr::Local(_) | Expr::Bit(_) | Expr::Getter { .. } | Expr::Index { .. }
        ) {
            out.push(y);
        }
        true
    });
    out
}

/// trace_explain.var_id
pub fn var_id(code: &Code, e: ExprId) -> Option<u32> {
    match code.expr(e) {
        Expr::Var(n) => Some(n as u32),
        Expr::Local(n) => Some(LOCAL_BIT | n as u32),
        Expr::Bit(n) => Some(BIT_BIT | n as u32),
        _ => None,
    }
}

pub fn show_var(n: u32) -> String {
    if n & BIT_BIT != 0 {
        format!("bit {}", n & 0x7FFF)
    } else if n & LOCAL_BIT != 0 {
        format!("local {}", n & 0xFFF)
    } else {
        format!("var {n}")
    }
}

/// trace_explain.slice_test (Lemma B)
pub fn slice_test(code: &Code, key: UKey, steps: &[StepX], n: usize, dump: Option<&Obs>) -> Vec<Value> {
    let ins = code.ins(key);
    let s = &steps[n];
    let x = &ins[s.i.unwrap() as usize];
    let reads = &s.op.reads;
    let leaves = match &x.st {
        Some(Stmt::Jump { cond: Some(c), .. }) => leaves_expr(code, *c),
        Some(Stmt::Jump { cond: None, .. }) => vec![],
        Some(Stmt::Assign(a)) => leaves_expr(code, a.rhs),
        Some(st) => leaves_stmt(code, st),
        None => vec![],
    };
    let mut out = Vec::new();
    for leaf in leaves {
        if let Expr::Getter { .. } = code.expr(leaf) {
            out.push(getter_value(code, leaf, dump));
            continue;
        }
        let Some(vid) = var_id(code, leaf) else {
            out.push(Value::new(code.show(leaf), J::Null, "", "unknown"));
            continue;
        };
        let val = reads.iter().find(|(v, _)| *v == vid).map(|p| p.1);
        out.push(trace_back(code, key, steps, n, vid, val, dump));
    }
    out
}

fn hex4(n: i32) -> String {
    format!("{:04X}", n)
}

/// trace_explain.trace_back
pub fn trace_back(
    code: &Code,
    key: UKey,
    steps: &[StepX],
    n: usize,
    vid: u32,
    val: Option<i32>,
    dump: Option<&Obs>,
) -> Value {
    let ins = code.ins(key);
    let ast = &code.game.script.ast;
    let what = show_var(vid);
    let vj = |v: Option<i32>| v.map(|x| J::Int(x as i64)).unwrap_or(J::Null);
    for m in (0..n).rev() {
        let s = &steps[m];
        let Some(i) = s.i else { continue };
        let Some(&(_, written)) = s.op.writes.iter().find(|(v, _)| *v == vid) else {
            continue;
        };
        let x = &ins[i as usize];
        let rhs = match &x.st {
            Some(Stmt::Assign(a)) => Some(a.rhs),
            _ => None,
        };
        let got = if val.is_some() { vj(val) } else { J::Int(written as i64) };
        if let Some(r) = rhs {
            if let Expr::Getter { name, .. } = ast.expr(r) {
                let g = getter_value(code, r, dump);
                return Value::new(
                    what,
                    got,
                    format!(
                        "{} at [{}] wrote {written}; the dump: {}",
                        code.name(name),
                        hex4(x.offset),
                        crate::pyj::Py::from_j(&g.value).str()
                    ),
                    getter_kind(code, name),
                );
            }
            let src = leaves_expr(code, r);
            let kinds: Vec<String> = src.iter().map(|&l| kind_of_leaf(code, l)).collect();
            let kind = kinds
                .iter()
                .find(|k| *k != "fact")
                .cloned()
                .unwrap_or_else(|| "fact".into());
            let names: Vec<String> = src.iter().map(|&l| code.show(l)).collect();
            let from = if names.is_empty() {
                "a constant".to_string()
            } else {
                names.join(", ")
            };
            return Value::new(
                what,
                got,
                format!("[{}] wrote {written} from {from}", hex4(x.offset)),
                kind,
            );
        }
        return Value::new(what, vj(val), format!("[{}] wrote {written}", hex4(x.offset)), "fact");
    }
    if vid & LOCAL_BIT != 0 && vid & BIT_BIT == 0 {
        let n_loc = (vid & 0xFFF) as i32;
        let mut rhs: Vec<String> = Vec::new();
        let mut gets: Vec<scumm_ir::Sym> = Vec::new();
        for x in ins {
            if let Some(Stmt::Assign(a)) = &x.st {
                if ast.expr(a.lhs) == Expr::Local(n_loc) {
                    let t = code.show(a.rhs);
                    if !rhs.contains(&t) {
                        rhs.push(t);
                    }
                    if let Expr::Getter { name, .. } = ast.expr(a.rhs) {
                        gets.push(name);
                    }
                }
            }
        }
        let kind = gets
            .iter()
            .map(|&g| getter_kind(code, g))
            .find(|k| k != "fact")
            .unwrap_or_else(|| {
                if rhs.is_empty() {
                    "unknown".into()
                } else {
                    "fact".into()
                }
            });
        return Value::new(
            what,
            vj(val),
            format!("set before these runs; its writers in the unit: {}", rhs.len()),
            kind,
        );
    }
    Value::new(what, vj(val), "", kind_of_var(code, vid))
}

/// trace_explain.kind_of_var
pub fn kind_of_var(code: &Code, vid: u32) -> String {
    if vid & BIT_BIT != 0 {
        return reading_kind(code, Key::bit((vid & 0x7FFF) as u64));
    }
    if vid & LOCAL_BIT != 0 {
        return "unknown".into();
    }
    if CLOCKS.contains(&(vid as i64)) {
        return "clock".into();
    }
    match code.game.nonfacts.get(&(vid as i32)) {
        Some(r) => r.as_str().into_owned(),
        None => reading_kind(code, Key::var(vid as u64)),
    }
}

/// trace_explain.reading_kind
pub fn reading_kind(code: &Code, key: Key) -> String {
    match &code.facts {
        None => "fact".into(),
        Some(f) if f.contains(&key) => "fact".into(),
        _ => "not a fact of the reading".into(),
    }
}

/// trace_explain.write_values: Lemma B on a write; on the test before it when its value reads nothing.
pub fn write_values(
    code: &Code,
    key: UKey,
    steps: &[StepX],
    n: usize,
    fact: &str,
    dump: Option<&Obs>,
) -> (Vec<Value>, Option<usize>) {
    let vals = slice_test(code, key, steps, n, dump);
    if !vals.is_empty() {
        return (vals, None);
    }
    let Some(m) = test_before_write(code, key, steps, n, fact) else {
        return (vec![], None);
    };
    (slice_test(code, key, steps, m, dump), Some(m))
}

/// trace_explain.test_before_write
pub fn test_before_write(code: &Code, key: UKey, steps: &[StepX], n: usize, fact: &str) -> Option<usize> {
    let i = steps[n].i;
    let targets: Vec<u32> = code
        .write_sites(key, fact, None, false)
        .into_iter()
        .filter(|&t| Some(t) != i)
        .collect();
    if targets.is_empty() {
        return None;
    }
    first_off(code, key, &steps[..n + 1], &targets, false)
}

/// trace_explain.kind_of_leaf
pub fn kind_of_leaf(code: &Code, leaf: ExprId) -> String {
    if let Expr::Getter { name, .. } = code.expr(leaf) {
        return getter_kind(code, name);
    }
    match var_id(code, leaf) {
        Some(v) => kind_of_var(code, v),
        None => "unknown".into(),
    }
}

/// trace_explain.getter_value on a getter node.
pub fn getter_value(code: &Code, g: ExprId, dump: Option<&Obs>) -> Value {
    let ast = &code.game.script.ast;
    let Expr::Getter { name, args } = ast.expr(g) else {
        return Value::new("?", J::Null, "", "unknown");
    };
    let a: Vec<Option<i64>> = ast
        .kids(args)
        .iter()
        .map(|&e| ast.constant(e).map(|c| c as i64))
        .collect();
    getter_value_args(code.name(name), &a, getter_kind(code, name), dump)
}

/// trace_explain.getter_value on a name and its constant arguments.
pub fn getter_value_args(name: &str, args: &[Option<i64>], kind: String, dump: Option<&Obs>) -> Value {
    let what = format!(
        "{name}({})",
        args.iter()
            .map(|a| a.map(|x| x.to_string()).unwrap_or_else(|| "?".into()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut val = J::Null;
    if let (Some(d), Some(Some(a0))) = (dump, args.first()) {
        let actor = d.actors.iter().find(|x| x.id as i64 == *a0);
        let field = match name {
            "getActorX" => Some(0),
            "getActorY" => Some(1),
            "getActorElevation" => Some(2),
            "getActorMoving" => Some(3),
            _ => None,
        };
        if let (Some(f), Some(a)) = (field, actor) {
            val = J::Int([a.x, a.y, a.elevation, a.moving][f] as i64);
        } else if name == "getObjectState" {
            val = d
                .states
                .iter()
                .find(|p| p.0 as i64 == *a0)
                .map(|p| J::Int(p.1 as i64))
                .unwrap_or(J::Null);
        } else if name == "getObjectOwner" {
            val = d
                .owners
                .iter()
                .find(|p| p.0 as i64 == *a0)
                .map(|p| J::Int(p.1 as i64))
                .unwrap_or(J::Null);
        } else if name == "classOfIs" {
            val = d
                .classes
                .iter()
                .find(|p| p.0 as i64 == *a0)
                .map(|p| J::Arr(p.1.iter().map(|&c| J::Int(c as i64)).collect()))
                .unwrap_or(J::Null);
        }
    }
    let src = if val.is_null() {
        "not in the dump"
    } else {
        "the state dump at the decision point before"
    };
    Value::new(what, val, src, kind)
}

/// trace_explain.wait_values: what a slot parked at instruction `x` waits on.
pub fn wait_values(code: &Code, x: &Ins, slot_delay: i64, dump: Option<&Obs>) -> Vec<Value> {
    let ast = &code.game.script.ast;
    let name = match &x.st {
        Some(Stmt::Op { name, .. }) => code.name(*name).to_string(),
        st => code.head(st.as_ref()),
    };
    match x.yields {
        Some(Yield::Delay) => {
            return vec![Value::new(
                "delay",
                J::Int(slot_delay),
                "the slot's delay at its last run (jiffies)",
                "time",
            )]
        }
        Some(Yield::Frame) => {
            return vec![Value::new(
                "frames to its next run",
                J::Int(1),
                "a breakHere yields one frame",
                "time",
            )]
        }
        _ => {}
    }
    match name.as_str() {
        "WaitForActor" => {
            let args: Vec<Option<i64>> = match &x.st {
                Some(Stmt::Op { args, .. }) => ast
                    .kids(*args)
                    .iter()
                    .map(|&e| ast.constant(e).map(|c| c as i64))
                    .collect(),
                _ => vec![],
            };
            vec![getter_value_args("getActorMoving", &args, "actor state".into(), dump)]
        }
        "WaitForCamera" => {
            let cam = dump
                .and_then(|d| d.vars.get(2))
                .map(|&v| J::Int(v as i64))
                .unwrap_or(J::Null);
            vec![Value::new(
                "var 2",
                cam,
                "VAR_CAMERA_POS_X at the decision point before; the camera's target is not in the dump",
                "position",
            )]
        }
        "WaitForMessage" => {
            let have = dump
                .and_then(|d| d.vars.get(3))
                .map(|&v| J::Int(v as i64))
                .unwrap_or(J::Null);
            vec![Value::new(
                "var 3",
                have,
                "VAR_HAVE_MSG at the decision point before: a message still shows",
                "time",
            )]
        }
        _ => vec![Value::new(name, J::Null, "a wait the dump does not show", "time")],
    }
}

/// trace_explain.cause_class
pub fn cause_class(values: &[Value]) -> &'static str {
    if values.iter().any(|v| v.kind != "fact") {
        "split"
    } else {
        "semantics"
    }
}

/// model.unit_name of a unit key: script:N, local:N, obj:O, entry, exit.
pub fn unit_name(k: UnitKey) -> String {
    match k {
        UnitKey::Global(n) => format!("script:{n}"),
        UnitKey::Local(_, n) => format!("local:{n}"),
        UnitKey::Obj(n) => format!("obj:{n}"),
        UnitKey::Entry(_) => "entry".into(),
        UnitKey::Exit(_) => "exit".into(),
    }
}
