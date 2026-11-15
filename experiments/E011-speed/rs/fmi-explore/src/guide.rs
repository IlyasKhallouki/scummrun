//! guide.py, line for line, on scumm-ir's `Game` (E003's IR in Rust, byte for byte ir.py's): the static write map of
//! the game's code regressed from a goal, and the room rings. The file it writes (format `e008-guide-1`) is guide.py's
//! bytes but for `stats.seconds`.
//!
//! * Atoms are (key, value), value None meaning any value; keys `var:N`, `bit:N`, `owner:O`, `state:O`, `class:O:C`,
//!   `room`.
//! * Writers W(k, v): the units with a direct write site of k (trace_explain.Code.write_sites' rule on the indexed
//!   instructions); for `state:O` also every unit of O's room with a drawObject; for `room = R` the units whose code
//!   loads R (rooms_from_files.py's rules); `room` with any value: also every other room change.
//! * Needs N(u, i): the atoms the conditions of the conditional jumps from which site i is reachable read (registers
//!   followed back to their writes), and how u starts (its room for room code, its room and (owner:O, any) for an
//!   object's verbs, a global's start sites' needs as a least fixpoint).
//! * Regression breadth first from the goal's atoms. `wide` adds writes through an index or to a computed object and
//!   computed starts.
//!
//! Python's dicts and sets are kept where their order reaches the output (the regression's frontiers, the atoms'
//! order); everywhere else a set is a set.

use crate::files::{self, Res};
use crate::pyjson;
use scumm_ir::{AssignOp, Expr, ExprId, Game, Ins, JumpForm, RefKind, Stmt, UnitKey, Yield, Ends};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

pub const FORMAT: &str = "e008-guide-1";
/// VAR_SCROLL_SCRIPT .. VAR_CUTSCENE_END_SCRIPT (vars.cpp:62-71)
pub const HOOKS: [i32; 10] = [27, 28, 29, 30, 31, 32, 33, 34, 35, 36];
/// classOfIs / setClass: a class operand with bit 0x80 means "has" / "set"
pub const CLASS_SET: i32 = 0x80;
const VAR_RESULT: i32 = 0;
const VAR_ROOM: i32 = 4;
const VAR_ROOM_RESOURCE: i32 = 22;
const VAR_NEW_ROOM: i32 = 72;
const VAR_CURSORSTATE: i32 = 52;
const VAR_USERPUT: i32 = 53;
/// trace_explain.ROOM_VARS
const ROOM_VARS: [i32; 6] = [VAR_ROOM, 17, 18, VAR_ROOM_RESOURCE, 70, 72];
const ROOM_HOLDERS: [i32; 3] = [VAR_ROOM, VAR_ROOM_RESOURCE, VAR_NEW_ROOM];
/// facts.CLOCKS
const CLOCKS: [i32; 6] = [11, 12, 13, 14, 46, 47];
const ROOM_CHANGE: [&str; 3] = ["loadRoom", "loadRoomWithEgo", "actorFollowCamera"];

fn room_set(n: i32) -> bool {
    ROOM_VARS.contains(&n) && !ROOM_HOLDERS.contains(&n)
}

fn cursor_var(op: &str) -> Option<i32> {
    match op {
        "UserputOn" | "UserputOff" | "UserputSoftOn" | "UserputSoftOff" => Some(VAR_USERPUT),
        "CursorShow" | "CursorHide" | "CursorSoftOn" | "CursorSoftOff" => Some(VAR_CURSORSTATE),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum OKind {
    State,
    Owner,
    Class,
}

impl OKind {
    /// the ops that write an object's state, owner and class
    fn write_ops(self) -> &'static [&'static str] {
        match self {
            OKind::State => &["setState", "drawObject", "pickupObject"],
            OKind::Owner => &["setOwnerOf", "pickupObject"],
            OKind::Class => &["setClass", "pickupObject"],
        }
    }
}

const OBJ_OPS: [&str; 5] = ["drawObject", "pickupObject", "setClass", "setOwnerOf", "setState"];

/// An atom's key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AK {
    Var(i32),
    Bit(i32),
    Owner(i32),
    State(i32),
    Class(i32, i32),
    Room,
}

impl AK {
    pub fn text(self) -> String {
        match self {
            AK::Var(n) => format!("var:{n}"),
            AK::Bit(n) => format!("bit:{n}"),
            AK::Owner(o) => format!("owner:{o}"),
            AK::State(o) => format!("state:{o}"),
            AK::Class(o, c) => format!("class:{o}:{c}"),
            AK::Room => "room".into(),
        }
    }
    pub fn parse(t: &str) -> Option<AK> {
        if t == "room" {
            return Some(AK::Room);
        }
        let (p, rest) = t.split_once(':')?;
        Some(match p {
            "var" => AK::Var(rest.parse().ok()?),
            "bit" => AK::Bit(rest.parse().ok()?),
            "owner" => AK::Owner(rest.parse().ok()?),
            "state" => AK::State(rest.parse().ok()?),
            "class" => {
                let (o, c) = rest.split_once(':')?;
                AK::Class(o.parse().ok()?, c.parse().ok()?)
            }
            _ => return None,
        })
    }
}

pub type Atom = (AK, Option<i32>);

/// guide.py `atom_order`: (key, str(value))
fn atom_order(a: &Atom) -> (String, String) {
    (a.0.text(), a.1.map(|v| v.to_string()).unwrap_or_else(|| "None".into()))
}

fn atom_str(a: &Atom) -> String {
    format!("{}={}", a.0.text(), a.1.map(|v| v.to_string()).unwrap_or_else(|| "null".into()))
}

pub fn unit_name(k: UnitKey) -> String {
    match k {
        UnitKey::Local(r, n) => format!("local:{r}:{n}"),
        k => format!("{}:{}", k.kind(), k.numbers().0[0]),
    }
}

/// A register: a local, or VAR_RESULT
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Reg {
    Local(i32),
    Result,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum IdxKey {
    Var(i32),
    Bit(i32),
    Obj(i32),
}

/// The game with the tables guide.py reads (shared by the direct and the wide map).
pub struct Code<'g> {
    pub g: &'g Game,
    /// unit index by key, keys sorted
    pub sorted: Vec<UnitKey>,
    nonfacts: HashSet<i32>,
    /// per unit, each instruction's predecessors along succ across yields (len(ins) + 1 lists)
    preds: Vec<Vec<Vec<u32>>>,
}

impl<'g> Code<'g> {
    pub fn new(g: &'g Game) -> Code<'g> {
        let mut sorted: Vec<UnitKey> = g.script.units.iter().map(|u| u.key).collect();
        sorted.sort();
        let mut c = Code { g, sorted, nonfacts: g.nonfacts.keys().copied().collect(), preds: Vec::new() };
        c.preds = (0..g.script.units.len()).map(|u| c.make_preds(u)).collect();
        c
    }
    fn uid(&self, k: UnitKey) -> Option<usize> {
        self.g.script.by_key.get(&k).map(|u| u.0 as usize)
    }
    fn ins(&self, u: usize) -> &'g [Ins] {
        &self.g.script.ins[self.g.script.units[u].range()]
    }
    fn name(&self, s: scumm_ir::Sym) -> &'g str {
        self.g.script.syms.str(s)
    }
    fn e(&self, x: ExprId) -> Expr {
        self.g.script.ast.expr(x)
    }
    fn konst(&self, x: ExprId) -> Option<i32> {
        self.g.script.ast.constant(x)
    }
    fn kids(&self, s: scumm_ir::Span) -> &'g [ExprId] {
        self.g.script.ast.kids(s)
    }

    /// trace_explain.Code.succ(key, i, cross_yields=True)
    fn succ(&self, ins: &[Ins], i: usize) -> Vec<u32> {
        let x = &ins[i];
        if matches!(x.yields, Some(Yield::Frame) | Some(Yield::Delay)) {
            return x.next.into_iter().collect();
        }
        let mut out = Vec::new();
        if x.yields == Some(Yield::Wait) {
            out.push(i as u32);
        }
        if let Some(Stmt::Jump { cond: None, .. }) = x.st {
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

    /// Each instruction's predecessors along succ across yields (guide.py `preds`)
    fn make_preds(&self, u: usize) -> Vec<Vec<u32>> {
        let ins = self.ins(u);
        let n = ins.len();
        let mut p = vec![Vec::new(); n + 1];
        for i in 0..n {
            for j in self.succ(ins, i) {
                if (j as usize) <= n {
                    p[j as usize].push(i as u32);
                }
            }
        }
        p
    }

    /// The instructions j with Code.reachable(key, j, {i}, cross_yields=True): i and its ancestors, ascending.
    fn reaching(&self, u: usize, i: u32) -> Vec<u32> {
        let p = &self.preds[u];
        let mut seen = vec![false; p.len()];
        seen[i as usize] = true;
        let mut todo = vec![i];
        while let Some(m) = todo.pop() {
            for &j in &p[m as usize] {
                if !seen[j as usize] {
                    seen[j as usize] = true;
                    todo.push(j);
                }
            }
        }
        (0..p.len() as u32).filter(|&j| seen[j as usize]).collect()
    }

    /// Calls f on every expression node of the statement, ir.py `_walk`'s order.
    fn walk(&self, st: &Stmt, f: &mut impl FnMut(ExprId) -> bool) {
        self.g.script.ast.walk_stmt(st, f);
    }

    fn is_reg(&self, x: ExprId) -> Option<Reg> {
        match self.e(x) {
            Expr::Local(n) => Some(Reg::Local(n)),
            Expr::Var(n) if n == VAR_RESULT => Some(Reg::Result),
            _ => None,
        }
    }

    fn expr_is(&self, x: ExprId, r: Reg) -> bool {
        matches!((self.e(x), r), (Expr::Local(n), Reg::Local(m)) if n == m) || matches!((self.e(x), r), (Expr::Var(VAR_RESULT), Reg::Result))
    }

    /// The value expression register r gets at instruction m: (op, expression), or None.
    fn writes_reg(&self, u: usize, m: u32, r: Reg) -> Option<(AssignOp, ExprId)> {
        let st = self.ins(u).get(m as usize)?.st?;
        if let Stmt::Assign(a) = st {
            if self.expr_is(a.lhs, r) {
                return Some((a.op, a.rhs));
            }
        }
        if r == Reg::Result {
            let mut last = None;
            self.walk(&st, &mut |y| {
                if let Expr::Embed(a) = self.e(y) {
                    if self.expr_is(a.lhs, r) {
                        last = Some((a.op, a.rhs));
                    }
                }
                true
            });
            return last;
        }
        None
    }

    /// The writes of register r in unit u that reach instruction j with no other write of r between.
    fn defs(&self, u: usize, j: u32, r: Reg) -> Vec<(u32, AssignOp, ExprId)> {
        let p = &self.preds[u];
        let mut seen = HashSet::new();
        let mut todo: Vec<u32> = p[j as usize].clone();
        let mut out = Vec::new();
        while let Some(m) = todo.pop() {
            if !seen.insert(m) {
                continue;
            }
            if let Some((op, e)) = self.writes_reg(u, m, r) {
                out.push((m, op, e));
                continue;
            }
            todo.extend(p[m as usize].iter().copied());
        }
        out.sort_by_key(|t| t.0);
        out
    }
}

/// The static write map (guide.py `Map`).
pub struct WMap<'g, 'c> {
    pub c: &'c mut Code<'g>,
    pub wide: bool,
    start: HashMap<String, i64>,
    unit_globals: Vec<BTreeSet<i32>>,
    /// per unit: room -> sites, rooms in insertion order
    unit_rooms: Vec<Vec<(i32, BTreeSet<u32>)>>,
    unit_changes: Vec<BTreeSet<u32>>,
    room_by: HashMap<Option<i32>, Vec<usize>>,
    by: HashMap<IdxKey, BTreeMap<UnitKey, BTreeSet<u32>>>,
    wild_var: Vec<(usize, u32, Option<i32>)>,
    wild_bit: Vec<(usize, u32, Option<i32>)>,
    wild_obj: Vec<(usize, u32, String)>,
    draws: HashMap<usize, BTreeSet<u32>>,
    writers_memo: HashMap<Atom, Vec<(UnitKey, BTreeSet<u32>)>>,
    tests_memo: HashMap<(usize, u32), Rc<HashSet<Atom>>>,
    conds_memo: HashMap<(usize, u32), Rc<HashSet<Atom>>>,
    pub start_needs: Vec<HashSet<Atom>>,
}

impl<'g, 'c> WMap<'g, 'c> {
    pub fn new(c: &'c mut Code<'g>, start: HashMap<String, i64>, wide: bool) -> WMap<'g, 'c> {
        let n = c.g.script.units.len();
        let mut m = WMap {
            c,
            wide,
            start,
            unit_globals: vec![BTreeSet::new(); n],
            unit_rooms: vec![Vec::new(); n],
            unit_changes: vec![BTreeSet::new(); n],
            room_by: HashMap::new(),
            by: HashMap::new(),
            wild_var: Vec::new(),
            wild_bit: Vec::new(),
            wild_obj: Vec::new(),
            draws: HashMap::new(),
            writers_memo: HashMap::new(),
            tests_memo: HashMap::new(),
            conds_memo: HashMap::new(),
            start_needs: Vec::new(),
        };
        let t = std::time::Instant::now();
        m.room_units();
        let t1 = t.elapsed().as_secs_f64();
        m.index();
        let t2 = t.elapsed().as_secs_f64();
        m.starts();
        let t3 = t.elapsed().as_secs_f64();
        if std::env::var("FMI_GUIDE_PROFILE").is_ok() {
            eprintln!("map wide={wide}: room_units {t1:.3} s, index {:.3} s, starts {:.3} s", t2 - t1, t3 - t2);
        }
        m
    }

    fn res(&self, r: i32) -> i32 {
        self.c.g.pseudo.get(&r).copied().unwrap_or(r)
    }

    fn op_name(&self, st: &Stmt) -> Option<&'g str> {
        match st {
            Stmt::Op { name, .. } => Some(self.c.name(*name)),
            _ => None,
        }
    }

    fn op_args(&self, st: &Stmt) -> &'g [ExprId] {
        match st {
            Stmt::Op { args, .. } => self.c.kids(*args),
            _ => &[],
        }
    }

    // --- rooms (rooms_from_files.py's rules, per unit)

    fn room_units(&mut self) {
        let g = self.c.g;
        let mut var_consts: HashMap<i32, BTreeSet<i32>> = HashMap::new();
        let mut arg_rooms: HashMap<i32, BTreeSet<i32>> = HashMap::new();
        for (u, unit) in g.script.units.iter().enumerate() {
            for x in self.c.ins(u) {
                let Some(st) = x.st else { continue };
                if let Stmt::Assign(a) = st {
                    if a.op == AssignOp::Set {
                        if let (Expr::Var(n), Some(v)) = (self.c.e(a.lhs), self.c.konst(a.rhs)) {
                            var_consts.entry(n).or_default().insert(v);
                        }
                    }
                }
                if let UnitKey::Global(gn) = unit.key {
                    if let Some(name @ ("loadRoom" | "loadRoomWithEgo")) = self.op_name(&st) {
                        let args = self.op_args(&st);
                        let arg = if name == "loadRoom" { args.first() } else { args.get(1) };
                        if let Some(&a) = arg {
                            if let Expr::Local(l) = self.c.e(a) {
                                arg_rooms.entry(gn).or_default().insert(l);
                            }
                        }
                    }
                }
            }
        }
        for u in 0..g.script.units.len() {
            let ins = self.c.ins(u);
            let mut gl = BTreeSet::new();
            let mut rooms: Vec<(i32, BTreeSet<u32>)> = Vec::new();
            let mut changes = BTreeSet::new();
            let add = |rooms: &mut Vec<(i32, BTreeSet<u32>)>, r: i32, sites: &[u32]| {
                if let Some(x) = rooms.iter_mut().find(|x| x.0 == r) {
                    x.1.extend(sites.iter().copied());
                } else {
                    rooms.push((r, sites.iter().copied().collect()));
                }
            };
            let follow: Vec<u32> = ins.iter().enumerate()
                .filter(|(_, x)| x.st.as_ref().and_then(|s| self.op_name(s)) == Some("actorFollowCamera"))
                .map(|(i, _)| i as u32).collect();
            for (i, x) in ins.iter().enumerate() {
                let Some(st) = x.st else { continue };
                let i = i as u32;
                if let Stmt::Start { name, script, args, .. } = st {
                    if self.c.name(name) != "startObject" {
                        if let Some(sv) = self.c.konst(script) {
                            if 0 < sv && sv < g.first_local {
                                gl.insert(sv);
                            }
                            if let Some(js) = arg_rooms.get(&sv) {
                                let a = self.c.kids(args);
                                for &j in js {
                                    if let Some(v) = a.get(j as usize).and_then(|&e| self.c.konst(e)) {
                                        if v > 0 {
                                            let r = self.res(v);
                                            add(&mut rooms, r, &[i]);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                let name = self.op_name(&st);
                if !follow.is_empty() && name == Some("putActorInRoom") {
                    if let Some(v) = self.op_args(&st).get(1).and_then(|&e| self.c.konst(e)) {
                        if v > 0 {
                            let r = self.res(v);
                            add(&mut rooms, r, &follow);
                        }
                    }
                }
                if let Some(nm) = name {
                    if ROOM_CHANGE.contains(&nm) {
                        changes.insert(i);
                    }
                    if nm == "loadRoom" || nm == "loadRoomWithEgo" {
                        let args = self.op_args(&st);
                        let arg = if nm == "loadRoom" { args.first() } else { args.get(1) };
                        if let Some(&a) = arg {
                            match self.c.e(a) {
                                Expr::Const(v) => {
                                    let r = self.res(v);
                                    add(&mut rooms, r, &[i]);
                                }
                                Expr::Var(n) if var_consts.contains_key(&n) => {
                                    for &v in &var_consts[&n] {
                                        if v > 0 {
                                            let r = self.res(v);
                                            add(&mut rooms, r, &[i]);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            for (_, s) in &rooms {
                changes.extend(s.iter().copied());
            }
            self.unit_globals[u] = gl;
            self.unit_rooms[u] = rooms;
            self.unit_changes[u] = changes;
        }
        let sorted = self.c.sorted.clone();
        for k in sorted {
            let u = self.c.uid(k).unwrap();
            for (r, _) in &self.unit_rooms[u] {
                self.room_by.entry(Some(*r)).or_default().push(u);
            }
            if !self.unit_changes[u].is_empty() {
                self.room_by.entry(None).or_default().push(u);
            }
        }
    }

    /// Room r's own code: its entry, exit and local scripts and the verb code of its objects.
    fn room_code(&self, r: i32) -> Vec<usize> {
        let g = self.c.g;
        let mut ks: Vec<usize> = g.script.units.iter().enumerate()
            .filter(|(_, u)| matches!(u.key, UnitKey::Entry(x) | UnitKey::Exit(x) | UnitKey::Local(x, _) if x == r))
            .map(|(i, _)| i).collect();
        for (&o, &rr) in g.object_room.iter() {
            if rr == r {
                if let Some(u) = self.c.uid(UnitKey::Obj(o)) {
                    ks.push(u);
                }
            }
        }
        ks
    }

    fn closure(&self, keys: &[usize]) -> BTreeSet<i32> {
        let mut todo: Vec<usize> = keys.to_vec();
        let mut seen = HashSet::new();
        let mut rooms = BTreeSet::new();
        while let Some(u) = todo.pop() {
            if !seen.insert(u) {
                continue;
            }
            rooms.extend(self.unit_rooms[u].iter().map(|x| x.0));
            for &n in &self.unit_globals[u] {
                if let Some(v) = self.c.uid(UnitKey::Global(n)) {
                    todo.push(v);
                }
            }
        }
        rooms
    }

    fn sget(&self, k: &str) -> i64 {
        self.start.get(k).copied().unwrap_or(0)
    }

    /// The globals the start runs or installs as hooks (as unit keys; a key no unit has is left out of the closure).
    fn start_globals(&self) -> Vec<UnitKey> {
        let mut out: Vec<UnitKey> = HOOKS.iter().map(|h| self.sget(&format!("var:{h}"))).filter(|&v| v != 0)
            .map(|v| UnitKey::Global(v as i32)).collect();
        let mut sc: Vec<(&String, &i64)> = self.start.iter().filter(|(k, v)| k.starts_with("script:") && **v == 1).collect();
        sc.sort();
        for (k, _) in sc {
            if let Some(n) = k.split(':').nth(1).and_then(|n| n.parse::<i32>().ok()) {
                out.push(UnitKey::Global(n));
            }
        }
        out
    }

    pub fn rooms(&self) -> Value {
        let sg: Vec<usize> = self.start_globals().into_iter().filter_map(|k| self.c.uid(k)).collect();
        let common = self.closure(&sg);
        let r0 = self.res(self.sget("room") as i32);
        let mut reached = BTreeSet::from([r0]);
        let mut todo = vec![r0];
        let mut edges: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
        while let Some(r) = todo.pop() {
            let mut rs = self.closure(&self.room_code(r));
            rs.extend(common.iter().copied());
            rs.remove(&0);
            edges.insert(r, rs.iter().copied().collect());
            for &x in &rs {
                if reached.insert(x) {
                    todo.push(x);
                }
            }
        }
        let mut em = serde_json::Map::new();
        for (r, v) in &edges {
            em.insert(r.to_string(), json!(v));
        }
        json!({"start": r0, "edges": Value::Object(em), "reached": reached.into_iter().collect::<Vec<_>>()})
    }

    // --- writers

    fn index(&mut self) {
        let g = self.c.g;
        let mut by: HashMap<IdxKey, BTreeMap<UnitKey, BTreeSet<u32>>> = HashMap::new();
        for (u, unit) in g.script.units.iter().enumerate() {
            let k = unit.key;
            let mut draws = BTreeSet::new();
            for (i, x) in self.c.ins(u).iter().enumerate() {
                let Some(st) = x.st else { continue };
                let i = i as u32;
                let mut put = |ix: IdxKey| {
                    by.entry(ix).or_default().entry(k).or_default().insert(i);
                };
                let c = &*self.c;
                c.walk(&st, &mut |y| {
                    if let Expr::Embed(a) = c.e(y) {
                        match c.e(a.lhs) {
                            Expr::Var(n) => put(IdxKey::Var(n)),
                            Expr::Bit(n) => put(IdxKey::Bit(n)),
                            _ => {}
                        }
                    }
                    true
                });
                match st {
                    Stmt::Assign(a) => match c.e(a.lhs) {
                        Expr::Var(n) => put(IdxKey::Var(n)),
                        Expr::Bit(n) => put(IdxKey::Bit(n)),
                        Expr::Index { kind, .. } if kind != RefKind::Local => {
                            let v = if a.op == AssignOp::Set { c.konst(a.rhs) } else { None };
                            if kind == RefKind::Var {
                                self.wild_var.push((u, i, v));
                            } else {
                                self.wild_bit.push((u, i, v));
                            }
                        }
                        _ => {}
                    },
                    Stmt::Op { name, args, .. } => {
                        let nm = c.name(name);
                        let args = c.kids(args);
                        if nm == "setVarRange" {
                            if let Some(&a0) = args.first() {
                                let items = args.get(2).map(|&l| match c.e(l) {
                                    Expr::List(s) => c.kids(s).len(),
                                    _ => 0,
                                }).unwrap_or(0) as i32;
                                match c.e(a0) {
                                    Expr::Var(n0) => (n0..n0 + items).for_each(|n| put(IdxKey::Var(n))),
                                    Expr::Bit(n0) => (n0..n0 + items).for_each(|n| put(IdxKey::Bit(n))),
                                    _ => {}
                                }
                            }
                        }
                        if ROOM_CHANGE.contains(&nm) {
                            for n in ROOM_VARS {
                                put(IdxKey::Var(n));
                            }
                        }
                        if let Some(v) = cursor_var(nm) {
                            put(IdxKey::Var(v));
                        }
                        if OBJ_OPS.contains(&nm) && !args.is_empty() {
                            match c.konst(args[0]) {
                                Some(o) => put(IdxKey::Obj(o)),
                                None => self.wild_obj.push((u, i, nm.to_string())),
                            }
                        }
                        if nm == "drawObject" {
                            draws.insert(i);
                        }
                    }
                    _ => {}
                }
            }
            if !draws.is_empty() {
                self.draws.insert(u, draws);
            }
        }
        self.by = by;
    }

    /// The sites Code.write_sites(direct=False) adds to the direct ones.
    fn wide_sites(&self, key: AK, value: Option<i32>) -> Vec<(usize, u32)> {
        let kv = |w: &Vec<(usize, u32, Option<i32>)>| -> Vec<(usize, u32)> {
            w.iter().filter(|(_, _, v)| value.is_none() || v.is_none() || *v == value).map(|(k, i, _)| (*k, *i)).collect()
        };
        match key {
            AK::Var(_) => kv(&self.wild_var),
            AK::Bit(_) => kv(&self.wild_bit),
            AK::State(_) | AK::Owner(_) | AK::Class(..) => {
                let ok = match key {
                    AK::State(_) => OKind::State,
                    AK::Owner(_) => OKind::Owner,
                    _ => OKind::Class,
                };
                self.wild_obj.iter().filter(|(_, _, op)| ok.write_ops().contains(&op.as_str())).map(|(k, i, _)| (*k, *i)).collect()
            }
            AK::Room => vec![],
        }
    }

    fn candidates(&self, key: AK) -> Option<&BTreeMap<UnitKey, BTreeSet<u32>>> {
        let ix = match key {
            AK::Var(n) => IdxKey::Var(n),
            AK::Bit(n) => IdxKey::Bit(n),
            AK::State(o) | AK::Owner(o) | AK::Class(o, _) => IdxKey::Obj(o),
            AK::Room => return None,
        };
        self.by.get(&ix)
    }

    fn direct_sites(&self, u: usize, key: AK, value: Option<i32>, cands: &BTreeSet<u32>) -> BTreeSet<u32> {
        let c = &*self.c;
        let ins = c.ins(u);
        let mut out = BTreeSet::new();
        match key {
            AK::Var(n) | AK::Bit(n) => {
                let is_var = matches!(key, AK::Var(_));
                let want = |x: ExprId| match c.e(x) {
                    Expr::Var(m) => is_var && m == n,
                    Expr::Bit(m) => !is_var && m == n,
                    _ => false,
                };
                for &i in cands {
                    let st = ins[i as usize].st.unwrap();
                    let mut emb = false;
                    c.walk(&st, &mut |y| {
                        if let Expr::Embed(a) = c.e(y) {
                            if want(a.lhs) {
                                emb = true;
                                return false;
                            }
                        }
                        true
                    });
                    if emb {
                        out.insert(i);
                        continue;
                    }
                    match st {
                        Stmt::Assign(a) => {
                            if want(a.lhs) && (value.is_none() || a.op != AssignOp::Set || c.konst(a.rhs).is_none() || c.konst(a.rhs) == value) {
                                out.insert(i);
                            }
                        }
                        Stmt::Op { name, args, .. } => {
                            let nm = c.name(name);
                            let args = c.kids(args);
                            let a0_kind_ok = args.first().map(|&a| matches!((c.e(a), is_var), (Expr::Var(_), true) | (Expr::Bit(_), false))).unwrap_or(false);
                            if nm == "setVarRange" && a0_kind_ok {
                                let n0 = match c.e(args[0]) {
                                    Expr::Var(m) | Expr::Bit(m) => m,
                                    _ => 0,
                                };
                                let items = args.get(2).map(|&l| match c.e(l) {
                                    Expr::List(s) => c.kids(s).len(),
                                    _ => 0,
                                }).unwrap_or(0) as i32;
                                if n0 <= n && n < n0 + items {
                                    out.insert(i);
                                }
                            } else if nm != "setVarRange"
                                && ((is_var && ROOM_VARS.contains(&n) && ROOM_CHANGE.contains(&nm)) || (is_var && cursor_var(nm) == Some(n)))
                            {
                                out.insert(i);
                            }
                        }
                        _ => {}
                    }
                }
            }
            AK::State(o) | AK::Owner(o) | AK::Class(o, _) => {
                let ok = match key {
                    AK::State(_) => OKind::State,
                    AK::Owner(_) => OKind::Owner,
                    _ => OKind::Class,
                };
                for &i in cands {
                    if let Some(Stmt::Op { name, args, .. }) = ins[i as usize].st {
                        let args = c.kids(args);
                        if ok.write_ops().contains(&c.name(name)) && !args.is_empty() && c.konst(args[0]) == Some(o) {
                            out.insert(i);
                        }
                    }
                }
            }
            AK::Room => {}
        }
        out
    }

    /// [(unit, sites)] of the atom, units in sorted order.
    pub fn writers(&mut self, a: Atom) -> Vec<(UnitKey, BTreeSet<u32>)> {
        if let Some(w) = self.writers_memo.get(&a) {
            return w.clone();
        }
        let (key, value) = a;
        let mut out: BTreeMap<UnitKey, BTreeSet<u32>> = BTreeMap::new();
        let units = &self.c.g.script.units;
        if key == AK::Room {
            for &u in self.room_by.get(&value).map(|v| v.as_slice()).unwrap_or(&[]) {
                let s = match value {
                    None => self.unit_changes[u].clone(),
                    Some(r) => self.unit_rooms[u].iter().find(|x| x.0 == r).map(|x| x.1.clone()).unwrap_or_default(),
                };
                out.insert(units[u].key, s);
            }
        } else {
            if let Some(c) = self.candidates(key) {
                for (k, cands) in c {
                    let u = self.c.uid(*k).unwrap();
                    let s = self.direct_sites(u, key, value, cands);
                    if !s.is_empty() {
                        out.entry(*k).or_default().extend(s);
                    }
                }
            }
            if self.wide {
                for (u, i) in self.wide_sites(key, value) {
                    out.entry(units[u].key).or_default().insert(i);
                }
            }
            if let AK::State(o) = key {
                if let Some(&room) = self.c.g.object_room.get(&o) {
                    for u in self.room_code(room) {
                        if let Some(d) = self.draws.get(&u) {
                            out.entry(units[u].key).or_default().extend(d.iter().copied());
                        }
                    }
                }
            }
        }
        let res: Vec<(UnitKey, BTreeSet<u32>)> = out.into_iter().collect();
        self.writers_memo.insert(a, res.clone());
        res
    }

    // --- needs: the tests on the way

    fn reads(&mut self, u: usize, j: u32, e: ExprId, value: Option<i32>, out: &mut HashSet<Atom>, stack: &mut Vec<(usize, u32, Reg)>) {
        let x = self.c.e(e);
        match x {
            Expr::Var(n) if n != VAR_RESULT => {
                if ROOM_HOLDERS.contains(&n) {
                    out.insert((AK::Room, value.map(|v| self.res(v))));
                } else if !room_set(n) && !CLOCKS.contains(&n) && !self.c.nonfacts.contains(&n) {
                    out.insert((AK::Var(n), value));
                }
            }
            Expr::Bit(n) => {
                out.insert((AK::Bit(n), value));
            }
            Expr::Local(_) | Expr::Var(_) => {
                let r = self.c.is_reg(e).unwrap();
                if stack.contains(&(u, j, r)) {
                    return;
                }
                stack.push((u, j, r));
                for (m, op, rhs) in self.c.defs(u, j, r) {
                    if op == AssignOp::Set {
                        self.reads(u, m, rhs, value, out, stack);
                    } else {
                        self.reads(u, m, rhs, None, out, stack);
                        self.reads(u, m, e, None, out, stack);
                    }
                }
                stack.pop();
            }
            Expr::Embed(a) => self.reads(u, j, a.rhs, value, out, stack),
            Expr::Getter { name, args } => {
                let nm = self.c.name(name);
                let args = self.c.kids(args);
                let o = args.first().and_then(|&a| self.c.konst(a));
                if let Some(o) = o {
                    match nm {
                        "getObjectState" => {
                            out.insert((AK::State(o), value));
                        }
                        "getObjectOwner" => {
                            out.insert((AK::Owner(o), value));
                        }
                        "classOfIs" if args.len() > 1 => {
                            let items: Vec<ExprId> = match self.c.e(args[1]) {
                                Expr::List(s) => self.c.kids(s).to_vec(),
                                _ => vec![args[1]],
                            };
                            for it in items {
                                if let Some(cv) = self.c.konst(it) {
                                    out.insert((AK::Class(o, cv & !CLASS_SET), Some(if cv & CLASS_SET != 0 { 1 } else { 0 })));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                for &a in args {
                    self.reads(u, j, a, None, out, stack);
                }
            }
            Expr::Cmp { a, b, .. } => {
                if let Some(v) = self.c.konst(b) {
                    self.reads(u, j, a, Some(v), out, stack);
                } else if let Some(v) = self.c.konst(a) {
                    self.reads(u, j, b, Some(v), out, stack);
                } else {
                    self.reads(u, j, a, None, out, stack);
                    self.reads(u, j, b, None, out, stack);
                }
            }
            Expr::Not(y) | Expr::Truth(y) | Expr::Neg(y) => self.reads(u, j, y, None, out, stack),
            Expr::BinOp { a, b, .. } => {
                self.reads(u, j, a, None, out, stack);
                self.reads(u, j, b, None, out, stack);
            }
            Expr::Index { offset, .. } => self.reads(u, j, offset, None, out, stack),
            Expr::List(s) | Expr::Concat(s) => {
                for &y in self.c.kids(s) {
                    self.reads(u, j, y, None, out, stack);
                }
            }
            Expr::Const(_) | Expr::Str(_) | Expr::Flag(_) => {}
        }
    }

    fn cond_atoms(&mut self, u: usize, j: u32) -> Rc<HashSet<Atom>> {
        if let Some(s) = self.conds_memo.get(&(u, j)) {
            return s.clone();
        }
        let mut out = HashSet::new();
        if let Some(Stmt::Jump { cond: Some(c), .. }) = self.c.ins(u)[j as usize].st {
            self.reads(u, j, c, None, &mut out, &mut Vec::new());
        }
        let out = Rc::new(out);
        self.conds_memo.insert((u, j), out.clone());
        out
    }

    /// Need (a): every fact the conditions of the conditional jumps from which site i is reachable read.
    pub fn tests(&mut self, u: usize, i: u32) -> Rc<HashSet<Atom>> {
        if let Some(s) = self.tests_memo.get(&(u, i)) {
            return s.clone();
        }
        let n = self.c.ins(u).len() as u32;
        let mut out = HashSet::new();
        for j in self.c.reaching(u, i) {
            if j < n {
                if let Some(Stmt::Jump { cond: Some(_), .. }) = self.c.ins(u)[j as usize].st {
                    let a = self.cond_atoms(u, j);
                    out.extend(a.iter().copied());
                }
            }
        }
        let out = Rc::new(out);
        self.tests_memo.insert((u, i), out.clone());
        out
    }

    // --- needs: how a unit starts

    fn starts(&mut self) {
        let g = self.c.g;
        let mut hooks: BTreeSet<i32> = HOOKS.iter().map(|h| self.sget(&format!("var:{h}")) as i32).filter(|&s| s != 0).collect();
        for u in 0..g.script.units.len() {
            for x in self.c.ins(u) {
                if let Some(Stmt::Assign(a)) = x.st {
                    if let (Expr::Var(n), Some(v)) = (self.c.e(a.lhs), self.c.konst(a.rhs)) {
                        if HOOKS.contains(&n) && v != 0 {
                            hooks.insert(v);
                        }
                    }
                }
            }
        }
        let mut free: HashSet<UnitKey> = hooks.iter().map(|&n| UnitKey::Global(n)).collect();
        free.extend(self.start_globals());
        let mut starters: HashMap<UnitKey, Vec<(usize, u32)>> = HashMap::new();
        let (mut computed, mut computed_obj) = (Vec::new(), Vec::new());
        for u in 0..g.script.units.len() {
            for (i, x) in self.c.ins(u).iter().enumerate() {
                let Some(Stmt::Start { name, script, obj, .. }) = x.st else { continue };
                if self.c.name(name) == "startObject" {
                    if obj.and_then(|o| self.c.konst(o)).is_none() {
                        computed_obj.push((u, i as u32));
                    }
                } else if let Some(n) = self.c.konst(script) {
                    if 0 < n && n < g.first_local && self.c.uid(UnitKey::Global(n)).is_some() {
                        starters.entry(UnitKey::Global(n)).or_default().push((u, i as u32));
                    }
                } else {
                    computed.push((u, i as u32));
                }
            }
        }
        let n = g.script.units.len();
        let mut sn: Vec<HashSet<Atom>> = vec![HashSet::new(); n];
        for (u, unit) in g.script.units.iter().enumerate() {
            sn[u] = match unit.key {
                UnitKey::Entry(r) | UnitKey::Exit(r) | UnitKey::Local(r, _) => HashSet::from([(AK::Room, Some(r))]),
                UnitKey::Obj(o) => {
                    let mut s = HashSet::from([(AK::Owner(o), None)]);
                    if let Some(&r) = g.object_room.get(&o) {
                        s.insert((AK::Room, Some(r)));
                    }
                    s
                }
                UnitKey::Global(_) => HashSet::new(),
            };
        }
        let mut edges: Vec<(usize, Vec<(usize, u32)>)> = Vec::new();
        for k in self.c.sorted.clone() {
            let u = self.c.uid(k).unwrap();
            match k {
                UnitKey::Global(_) if !free.contains(&k) => {
                    let mut e = starters.get(&k).cloned().unwrap_or_default();
                    if self.wide {
                        e.extend(computed.iter().copied());
                    }
                    edges.push((u, e));
                }
                UnitKey::Obj(_) if self.wide => edges.push((u, computed_obj.clone())),
                _ => {}
            }
        }
        let mut changed = true;
        while changed {
            changed = false;
            for (k, es) in &edges {
                let mut new = sn[*k].clone();
                for &(v, i) in es {
                    new.extend(self.tests(v, i).iter().copied());
                    new.extend(sn[v].iter().copied());
                }
                if new.len() != sn[*k].len() {
                    sn[*k] = new;
                    changed = true;
                }
            }
        }
        self.start_needs = sn;
    }

    pub fn needs(&mut self, u: usize, i: u32) -> HashSet<Atom> {
        let mut s: HashSet<Atom> = (*self.tests(u, i)).clone();
        s.extend(self.start_needs[u].iter().copied());
        s
    }
}

/// E014 (plan.md (d)): the goal key of the game's own end, `end` = 1: a script's restart or quit (systemOps sub-ops 1
/// and 3). It is no fact of the scripts, so it has no atom; its "writers" are the end sites (`end_sites`).
pub const END_KEY: &str = "end";

impl<'g, 'c> WMap<'g, 'c> {
    /// E014: the end sites, (unit, instruction): every `systemOps(1)` (restart) and `systemOps(3)` (quit) statement.
    pub fn end_sites(&self) -> Vec<(usize, u32)> {
        let mut out = Vec::new();
        for u in 0..self.c.g.script.units.len() {
            for (i, x) in self.c.ins(u).iter().enumerate() {
                if let Some(Stmt::Op { name, args, .. }) = x.st {
                    if name == scumm_ir::known::SYSTEM_OPS {
                        if let Some(&a0) = self.c.kids(args).first() {
                            if matches!(self.c.konst(a0), Some(1) | Some(3)) {
                                out.push((u, i as u32));
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

/// Breadth first from the goal's atoms: {atom: depth}. E014: the goal key `end` (END_KEY) starts from the end sites'
/// needs at depth 1, as a fact's write sites' needs are its writers' (the regression goes on from them).
pub fn regress(m: &mut WMap, goal: &[(String, Vec<i64>)]) -> Res<HashMap<Atom, i64>> {
    let mut depth: HashMap<Atom, i64> = HashMap::new();
    let mut end_needs: Vec<Atom> = Vec::new();
    for (k, vs) in goal {
        if k == END_KEY {
            for (u, i) in m.end_sites() {
                let t = m.tests(u, i);
                end_needs.extend(t.iter().chain(m.start_needs[u].iter()).copied());
            }
            continue;
        }
        let ak = AK::parse(k).ok_or(format!("goal key {k}"))?;
        for v in vs {
            depth.insert((ak, Some(*v as i32)), 0);
        }
    }
    for a in end_needs {
        depth.entry(a).or_insert(1);
    }
    // a breadth-first level's depth does not depend on the order within the level (guide.py sorts each frontier,
    // which reaches only `parent`, not written): the frontier is sorted once per level, the needs are not
    // a write site's needs are the same whichever atom it writes: once a site is expanded every need has a depth,
    // so a site met again (at the same level or later) adds nothing and is skipped
    let mut frontier: Vec<Atom> = depth.iter().filter(|(_, &v)| v == 0).map(|(a, _)| *a).collect();
    frontier.sort_by_cached_key(atom_order);
    let mut d = 0;
    let mut expanded: HashSet<(usize, u32)> = HashSet::new();
    // E014: with only the end goal there is no depth-0 atom; the regression starts at depth 1 from the end's needs
    if frontier.is_empty() && !depth.is_empty() {
        frontier = depth.keys().copied().collect();
        frontier.sort_by_cached_key(atom_order);
        d = 1;
    }
    while !frontier.is_empty() {
        let mut nxt = Vec::new();
        for a in &frontier {
            for (k, sites) in m.writers(*a) {
                let u = m.c.uid(k).unwrap();
                for i in sites {
                    if !expanded.insert((u, i)) {
                        continue;
                    }
                    let t = m.tests(u, i);
                    for b in t.iter().chain(m.start_needs[u].iter()) {
                        if let std::collections::hash_map::Entry::Vacant(e) = depth.entry(*b) {
                            e.insert(d + 1);
                            nxt.push(*b);
                        }
                    }
                }
            }
        }
        if d == 0 {
            // E014: the end's needs, preset at depth 1, join the first level a fact goal's regression reaches
            let have: HashSet<Atom> = nxt.iter().copied().collect();
            nxt.extend(depth.iter().filter(|(a, &v)| v == 1 && !have.contains(*a)).map(|(a, _)| *a).collect::<Vec<_>>());
        }
        nxt.sort_by_cached_key(atom_order);
        frontier = nxt;
        d += 1;
    }
    Ok(depth)
}

/// The repository root the game data is read from (scumm-ir's rule: FAST_MI_ROOT, else the root of the tree the
/// crate was built in).
pub fn default_root() -> std::path::PathBuf {
    scumm_ir::default_root()
}

/// model.Game.load() (scumm-ir's port, byte for byte ir.py's).
pub fn load_game(root: &Path) -> Res<Game> {
    Game::load(root).map_err(|e| format!("the game under {}: {e}", root.display()))
}

/// guide.py `start_row`: the seed's facts from E003's results/start-part1.jsonl.
pub fn start_row(e003: &Path, seed: i64) -> Res<HashMap<String, i64>> {
    let p = e003.join("results").join("start-part1.jsonl");
    let text = files::read_text(&p)?;
    for ln in files::py_splitlines(&text) {
        if ln.trim().is_empty() {
            continue;
        }
        let r = pyjson::loads(ln).map_err(|e| format!("{}: {e}", p.display()))?;
        if r["seed"].as_i64() == Some(seed) {
            return Ok(r["facts"].as_object().map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_i64()?))).collect()).unwrap_or_default());
        }
    }
    Err(format!("{}: no row for seed {seed}", p.display()))
}

/// guide.py `parse_goal`: "K=V[,K=V]" or "part1" as {key: [values]}, sorted.
pub fn parse_goal(text: &str, e003: &Path) -> Res<Vec<(String, Vec<i64>)>> {
    let pairs: Vec<(String, String)> = if text == "part1" {
        let doc = files::read_json(&e003.join("results").join("end-part1.json"))?;
        doc["goal_all"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), files::py_str(Some(v)))).collect()).unwrap_or_default()
    } else {
        text.split(',').filter(|p| !p.trim().is_empty()).map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (k.to_string(), v.to_string())
        }).collect()
    };
    let mut goal: Vec<(String, Vec<i64>)> = Vec::new();
    for (k, v) in pairs {
        let v: i64 = crate::step::py_int(&v).ok_or(format!("goal {text}: {v} is not an int"))?;
        let k = k.trim().to_string();
        match goal.iter_mut().find(|x| x.0 == k) {
            Some(x) => {
                if !x.1.contains(&v) {
                    x.1.push(v);
                }
            }
            None => goal.push((k, vec![v])),
        }
    }
    for x in goal.iter_mut() {
        x.1.sort();
    }
    goal.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(goal)
}

/// The GUIDE dict of format e008-guide-1 for `goal`; `seconds` is the stats' time.
pub fn build(g: &Game, goal: &[(String, Vec<i64>)], wide: bool, start: &HashMap<String, i64>, seconds: &mut dyn FnMut() -> f64) -> Res<Value> {
    // guide.py builds both maps and regresses on both (the other one for stats.wide): two threads, the game shared
    let other = std::thread::scope(|sc| {
        let h = sc.spawn(|| -> Res<HashMap<Atom, i64>> {
            let mut code2 = Code::new(g);
            let mut m2 = WMap::new(&mut code2, start.clone(), !wide);
            regress(&mut m2, goal)
        });
        let mut code = Code::new(g);
        let out = (|| -> Res<_> {
            let mut m_out = WMap::new(&mut code, start.clone(), wide);
            let depth = regress(&mut m_out, goal)?;
            let mut atoms: Vec<Atom> = depth.keys().copied().collect();
            atoms.sort_by_cached_key(|a| (depth[a], atom_order(a)));
            let mut writers = serde_json::Map::new();
            let mut no_writer = Vec::new();
            let mut units: HashSet<UnitKey> = HashSet::new();
            for a in &atoms {
                let ws = m_out.writers(*a);
                if !ws.is_empty() {
                    let mut names: Vec<String> = ws.iter().map(|(k, _)| unit_name(*k)).collect();
                    names.sort();
                    writers.insert(atom_str(a), json!(names));
                    units.extend(ws.iter().map(|(k, _)| *k));
                } else {
                    no_writer.push(json!([a.0.text(), a.1]));
                }
            }
            Ok((m_out.rooms(), depth, writers, no_writer, units, atoms))
        })();
        (out, h.join().unwrap_or_else(|_| Err("the other map's thread panicked".into())))
    });
    let (out_rooms, depth, writers, no_writer, units, atoms) = other.0?;
    let wd = other.1?;
    if std::env::var("FMI_GUIDE_PROFILE").is_ok() {
        eprintln!("both maps and their regressions: {:.3} s", seconds());
    }
    let (wide_depth, _) = if wide { (&depth, ()) } else { (&wd, ()) };
    let mut keys = serde_json::Map::new();
    for a in &atoms {
        let t = a.0.text();
        if !keys.contains_key(&t) {
            keys.insert(t, json!(depth[a]));
        }
    }
    let mut hist: BTreeMap<i64, i64> = BTreeMap::new();
    for d in depth.values() {
        *hist.entry(*d).or_insert(0) += 1;
    }
    let mut hm = serde_json::Map::new();
    for (d, n) in &hist {
        hm.insert(d.to_string(), json!(n));
    }
    let mut gm = serde_json::Map::new();
    for (k, vs) in goal {
        gm.insert(k.clone(), json!(vs));
    }
    let wide_keys: HashSet<AK> = wide_depth.keys().map(|a| a.0).collect();
    Ok(json!({
        "format": FORMAT,
        "goal": Value::Object(gm),
        "map": if wide { "wide" } else { "direct" },
        "rooms": out_rooms,
        "atoms": atoms.iter().map(|a| json!([a.0.text(), a.1, depth[a]])).collect::<Vec<_>>(),
        "keys": Value::Object(keys),
        "writers": Value::Object(writers),
        "stats": {"atoms": atoms.len(), "keys": atoms.iter().map(|a| a.0).collect::<HashSet<_>>().len(),
                  "depth_hist": Value::Object(hm), "units": units.len(), "seconds": pyjson::round(seconds(), 3),
                  "no_writer": no_writer,
                  "wide": {"atoms": wide_depth.len(), "keys": wide_keys.len()}},
    }))
}


/// guide.py `dumps`: one line per top-level field, compact values.
pub fn dumps(guide: &Value) -> String {
    let items: Vec<String> = guide
        .as_object()
        .map(|m| m.iter().map(|(k, v)| format!("{}: {}", pyjson::str_text(k), pyjson::compact(v))).collect())
        .unwrap_or_default();
    format!("{{{}}}\n", items.join(",\n "))
}

/// guide.py `stats_text`.
pub fn stats_text(guide: &Value, key: Option<&Path>) -> Res<String> {
    let s = &guide["stats"];
    let maxd = s["depth_hist"].as_object().map(|m| m.keys().filter_map(|k| k.parse::<i64>().ok()).max().unwrap_or(0)).unwrap_or(0);
    let mut lines = vec![
        format!("goal {}  map {}", pyjson::dumps_default(&guide["goal"]), guide["map"].as_str().unwrap_or("")),
        format!("|R| {} atoms over {} keys, depth at most {maxd}, writer units {}, {} s", s["atoms"], s["keys"], s["units"],
                files::py_str(s.get("seconds"))),
        format!("depth histogram {}", s["depth_hist"].as_object().map(|m| m.iter().map(|(d, n)| format!("{d}:{n}")).collect::<Vec<_>>().join(" ")).unwrap_or_default()),
        format!("wide map: |R| {} atoms over {} keys", s["wide"]["atoms"], s["wide"]["keys"]),
        format!("atoms with no writer: {}", s["no_writer"].as_array().map(|a| a.len()).unwrap_or(0)),
    ];
    for (k, vs) in guide["goal"].as_object().cloned().unwrap_or_default() {
        for v in vs.as_array().cloned().unwrap_or_default() {
            let a = format!("{k}={}", pyjson::dumps_default(&v));
            let ws = guide["writers"].get(&a).and_then(|w| w.as_array()).map(|w| w.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
            lines.push(format!("writers of {a}: {}", if ws.is_empty() { "none".to_string() } else { ws }));
        }
    }
    let got: Vec<String> = guide["atoms"].as_array().cloned().unwrap_or_default().iter()
        .filter(|a| a[0] == "room").map(|a| format!("{}@{}", files::py_str(a.get(1)), a[2])).collect();
    lines.push(format!("room atoms (value@depth): {}{}", got.iter().take(40).cloned().collect::<Vec<_>>().join(" "), if got.len() > 40 { " ..." } else { "" }));
    lines.push(format!("rooms: start {}, reached {}", guide["rooms"]["start"], guide["rooms"]["reached"].as_array().map(|a| a.len()).unwrap_or(0)));
    if let Some(k) = key {
        let r = recall(guide, k)?;
        lines.push(format!("key (evaluation only): {} Part I facts, key in R {}, (key, value) or (key, null) in R {}", r.0, r.1, r.2));
    }
    Ok(lines.join("\n") + "\n")
}

/// guide.py `recall` (evaluation only).
pub fn recall(guide: &Value, key_path: &Path) -> Res<(usize, usize, usize)> {
    let doc = files::read_json(key_path)?;
    let entries: Vec<Value> = doc["key"].as_array().cloned().unwrap_or_default().into_iter().filter(|e| e.get("part").and_then(|p| p.as_str()) == Some("I")).collect();
    let mut d: HashSet<(String, Option<i64>)> = HashSet::new();
    for a in guide["atoms"].as_array().cloned().unwrap_or_default() {
        d.insert((a[0].as_str().unwrap_or("").to_string(), a[1].as_i64()));
    }
    let (mut in_key, mut in_value) = (0, 0);
    for e in &entries {
        let f = &e["fact"];
        let kind = f[0].as_str().unwrap_or("");
        let n = files::py_str(f.get(1));
        let v = f[2].as_i64().unwrap_or(0);
        let (k, v, neg) = match kind {
            "class" => (format!("class:{n}:{}", v.abs()), if v > 0 { 1 } else { 0 }, false),
            "owner_not" => (format!("owner:{n}"), v, true),
            _ => (format!("{kind}:{n}"), v, false),
        };
        if guide["keys"].get(&k).is_some() {
            in_key += 1;
        }
        let ok = if neg {
            d.contains(&(k.clone(), None)) || d.iter().any(|(kk, vv)| *kk == k && vv.is_some() && *vv != Some(v))
        } else {
            d.contains(&(k.clone(), Some(v))) || d.contains(&(k.clone(), None))
        };
        in_value += ok as usize;
    }
    Ok((entries.len(), in_key, in_value))
}

#[allow(dead_code)]
fn unused(_: JumpForm) {}

// --- the reader's rules ---------------------------------------------------------------------------------------------

/// guide.py `load`: a guide file of format e008-guide-1.
pub fn load(path: &Path) -> Res<Value> {
    let g = files::read_json(path)?;
    if g.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        return Err(format!("ValueError: {}: format {}, not {FORMAT}", path.display(), files::py_repr(g.get("format").unwrap_or(&Value::Null))));
    }
    Ok(g)
}

/// guide.py `depth`: the least depth of [key, value] and [key, null]; None when neither is in R.
pub fn depth(guide: &Value, key: &str, value: Option<i64>) -> Option<i64> {
    let mut best: Option<i64> = None;
    for a in guide["atoms"].as_array().into_iter().flatten() {
        if a[0].as_str() != Some(key) {
            continue;
        }
        let v = a[1].as_i64();
        if v == value || v.is_none() {
            let d = a[2].as_i64()?;
            best = Some(best.map_or(d, |b| b.min(d)));
        }
    }
    best
}

/// guide.py `ring`: sorted({room} | edges[room]) & allowed (every room when allowed is None).
pub fn ring(guide: &Value, room: i64, allowed: Option<&[i64]>) -> Vec<i64> {
    let mut rs: BTreeSet<i64> = guide["rooms"]["edges"].get(room.to_string()).and_then(|e| e.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
    rs.insert(room);
    match allowed {
        Some(al) => rs.into_iter().filter(|r| al.contains(r)).collect(),
        None => rs.into_iter().collect(),
    }
}

impl<'g, 'c> WMap<'g, 'c> {
    /// The unit index of a key (for the tests).
    pub fn unit_of(&self, k: UnitKey) -> Option<usize> {
        self.c.uid(k)
    }
    /// The atoms a conditional jump's condition reads (for the tests).
    pub fn cond_atoms_of(&mut self, u: usize, j: u32) -> HashSet<Atom> {
        (*self.cond_atoms(u, j)).clone()
    }
}
