//! domains.py: the finite domain of each fact the planner's task can hold, read from the IR (`build`), and the
//! IR helpers reader.py borrows from it (`fold`, `decide`, `live`, the call-site reader `CallReader`).
//!
//! The rules and their ScummVM cites are domains.py's module docstring; this is the same code, step for step. Where
//! Python's result rests on an iteration order (the call-site fixpoint, `Counter.most_common`'s ties, the notes'
//! order) the order is Python's: units in ir.py's load order, call sites as met, reasons in first-count order.

use crate::domain::{Domain, Label};
use crate::facts::Facts;
use crate::py::{Counter, Py};
use rustc_hash::{FxHashMap, FxHashSet};
use scumm_ir::{known, ArithOp, AssignOp, Ast, Expr, ExprId, Game, RefKind, Span, Stmt, UnitId, UnitKey};
use scumm_machine::Switches;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_CUTSCENE: i64 = 5; // kMaxCutsceneNum, script.h:111
pub const HOOKS: std::ops::Range<i32> = 27..37; // VAR_SCROLL_SCRIPT..VAR_CUTSCENE_END_SCRIPT (vars.cpp:62-71)
pub const VAR_EGO: i32 = 1;
pub const VAR_ROOM: i32 = 4;
pub const VAR_OVERRIDE: i32 = 5;
pub const VAR_ROOM_RESOURCE: i32 = 22;
pub const VAR_WALKTO_OBJ: i32 = 38;
pub const VAR_ROOM_FLAG: i32 = 70;
pub const VAR_NEW_ROOM: i32 = 72;
/// Variables the engine writes outside the scripts (vars.cpp:36-109 indices): not facts.
pub const ENGINE: &[(i32, &str)] = &[
    (2, "VAR_CAMERA_POS_X: scumm.cpp:3379 every frame"),
    (3, "VAR_HAVE_MSG: scumm.cpp:3382 every frame"),
    (11, "VAR_TMR_1: a clock (facts.CLOCKS)"),
    (12, "VAR_TMR_2: a clock"),
    (13, "VAR_TMR_3: a clock"),
    (14, "VAR_MUSIC_TIMER: a clock"),
    (46, "VAR_TIMER: a clock"),
    (47, "VAR_TIMER_TOTAL: a clock"),
    (17, "VAR_CAMERA_MIN_X: room.cpp:203, the room's width"),
    (18, "VAR_CAMERA_MAX_X: room.cpp:205, the room's width"),
    (20, "VAR_VIRT_MOUSE_X: scumm.cpp:3385"),
    (21, "VAR_VIRT_MOUSE_Y: scumm.cpp:3386"),
    (23, "VAR_LAST_SOUND: sound.cpp:121-122, actor walk sounds every frame"),
    (25, "VAR_TALK_ACTOR: actor.cpp:3368, stopTalk on the message timer"),
    (26, "VAR_CAMERA_FAST_X: input.cpp:1283"),
    (37, "VAR_CHARINC: input.cpp:1150-1156"),
    (39, "VAR_DEBUGMODE: scumm.cpp:3391 every frame"),
    (44, "VAR_MOUSE_X: scumm.cpp:3387"),
    (45, "VAR_MOUSE_Y: scumm.cpp:3388"),
    (71, "VAR_GAME_LOADED: scumm.cpp:3415-3445"),
];
pub const ROOM_VARS: [i32; 3] = [VAR_ROOM, VAR_ROOM_RESOURCE, VAR_NEW_ROOM];
pub const BOOL_VARS: [i32; 2] = [VAR_OVERRIDE, VAR_ROOM_FLAG];
pub const BINARY_PREFIXES: [&str; 9] =
    ["bit:", "class:", "script:", "local:", "objscript:", "at:", "frozen:", "esc:", "verb:"];
pub const MAX_SET: usize = 4096;
pub const NUM_LOCALS: i32 = 25;
/// Engine variables the run's settings fix: VAR_DEBUGMODE is 0 with no debug flag (scumm.cpp:3389-3392).
pub const SETTINGS: [(i32, i64); 1] = [(39, 0)];

pub fn engine_why(n: i32) -> Option<&'static str> {
    ENGINE.iter().find(|e| e.0 == n).map(|e| e.1)
}

/// A set of ints folded from an expression, sorted and distinct (a frozenset).
pub type ISet = Vec<i64>;
/// Bound locals of a unit: Local[k] -> its values.
pub type Env = BTreeMap<i32, ISet>;
/// Variables with known values (the settings, or the exact domains).
pub type VarEnv = FxHashMap<i32, ISet>;

fn iset(mut v: Vec<i64>) -> ISet {
    v.sort_unstable();
    v.dedup();
    v
}

/// C division, truncating toward zero (script_v5.cpp:994).
fn div(a: i64, b: i64) -> i64 {
    let q = a.abs() / b.abs();
    if (a >= 0) == (b >= 0) {
        q
    } else {
        -q
    }
}

/// domains.py `fold`: the values of `e`, or None when it does not fold. `caps` counts MAX_SET hits
/// (domains.FOLD_CAPS).
pub fn fold(ast: &Ast, e: ExprId, env: Option<&Env>, var: Option<&VarEnv>, caps: &mut i64) -> Option<ISet> {
    match ast.expr(e) {
        Expr::Const(v) => Some(vec![v as i64]),
        Expr::Local(k) => env.and_then(|m| m.get(&k)).cloned(),
        Expr::Var(n) => var.and_then(|m| m.get(&n)).cloned(),
        Expr::Neg(x) => fold(ast, x, env, var, caps).map(|s| iset(s.into_iter().map(|v| -v).collect())),
        Expr::BinOp { op, a, b } => {
            let sa = fold(ast, a, env, var, caps);
            let sb = fold(ast, b, env, var, caps);
            let (sa, sb) = (sa?, sb?);
            if sa.len() * sb.len() > MAX_SET {
                *caps += 1;
                return None;
            }
            if op == ArithOp::Div {
                if sb.contains(&0) {
                    return None;
                }
                return Some(iset(sa.iter().flat_map(|&x| sb.iter().map(move |&y| div(x, y))).collect()));
            }
            let f = |x: i64, y: i64| match op {
                ArithOp::Add => x + y,
                ArithOp::Sub => x - y,
                ArithOp::Mul => x * y,
                ArithOp::Div => unreachable!(),
            };
            Some(iset(sa.iter().flat_map(|&x| sb.iter().map(move |&y| f(x, y))).collect()))
        }
        _ => None,
    }
}

/// domains.py `decide`: a jump condition's truth from the settings, or None.
pub fn decide(ast: &Ast, cond: ExprId, var: &VarEnv, caps: &mut i64) -> Option<bool> {
    let r: BTreeSet<bool> = match ast.expr(cond) {
        Expr::Cmp { op, a, b } => {
            let sa = fold(ast, a, None, Some(var), caps);
            let sb = fold(ast, b, None, Some(var), caps);
            let (sa, sb) = (sa?, sb?);
            let op = crate::domain::Cmp::of(op);
            sa.iter().flat_map(|&x| sb.iter().map(move |&y| op.eval(x, y))).collect()
        }
        Expr::Truth(x) | Expr::Not(x) => {
            let truth = matches!(ast.expr(cond), Expr::Truth(_));
            let s = fold(ast, x, None, Some(var), caps)?;
            s.iter().map(|&v| (v != 0) == truth).collect()
        }
        _ => return None,
    };
    (r.len() == 1).then(|| *r.iter().next().unwrap())
}

/// The settings as fold's variable map ({n: {v}}).
pub fn settings_env() -> VarEnv {
    SETTINGS.iter().map(|&(n, v)| (n, vec![v])).collect()
}

/// domains.py `live`: the instruction indices of unit `u` a run can reach, sorted.
pub fn live(game: &Game, u: UnitId, settings: &VarEnv, starts: Option<&[u32]>, caps: &mut i64) -> Vec<u32> {
    let unit = game.unit(u);
    let ins = game.ins(u);
    let n = ins.len() as u32;
    let mut todo: Vec<u32> = match starts {
        Some(s) => s.iter().copied().filter(|&i| i < n).collect(),
        None => match unit.key {
            UnitKey::Obj(_) => unit.entries.iter().map(|e| e.1).filter(|&i| i < n).collect(),
            _ => (0 < n).then_some(0).into_iter().collect(),
        },
    };
    let mut seen = vec![false; n as usize];
    while let Some(i) = todo.pop() {
        if i >= n || seen[i as usize] {
            continue;
        }
        seen[i as usize] = true;
        let x = &ins[i as usize];
        if let Some(Stmt::Jump { cond: Some(c), .. }) = x.st {
            if let Some(d) = decide(&game.ast, c, settings, caps) {
                if let Some(j) = if d { x.next } else { x.target } {
                    todo.push(j);
                }
                continue;
            }
        }
        todo.extend([x.next, x.target, x.esc].into_iter().flatten());
    }
    (0..n).filter(|&i| seen[i as usize]).collect()
}

/// The expression nodes of a statement (domains.py `_walk`, restricted to expressions: the checks only test their
/// kinds).
pub fn stmt_nodes(ast: &Ast, st: &Stmt) -> Vec<ExprId> {
    let mut out = Vec::new();
    ast.walk_stmt(st, &mut |e| {
        out.push(e);
        true
    });
    out
}

pub fn expr_nodes(ast: &Ast, e: ExprId) -> Vec<ExprId> {
    let mut out = Vec::new();
    ast.walk(e, &mut |x| {
        out.push(x);
        true
    });
    out
}

pub fn is_var(ast: &Ast, e: ExprId, n: i32) -> bool {
    ast.expr(e) == Expr::Var(n)
}

/// The locals a unit writes (domains.py `_Reader.written_locals`): the numbers, and whether an indexed local write
/// occurs ("indexed").
#[derive(Clone, Debug, Default)]
pub struct Written {
    pub locals: BTreeSet<i32>,
    pub indexed: bool,
}

/// domains.py `_Reader`: call sites and their arguments over the instructions a run can reach.
pub struct CallReader<'a> {
    pub game: &'a Game,
    /// the units read, in order
    pub units: Vec<UnitId>,
    pub keys: FxHashSet<UnitKey>,
    pub code: FxHashMap<UnitId, Vec<u32>>,
    pub first_local: i32,
    pub local_rooms: BTreeMap<i32, BTreeSet<i32>>,
}

impl<'a> CallReader<'a> {
    pub fn new(game: &'a Game, units: Vec<UnitId>, code: FxHashMap<UnitId, Vec<u32>>) -> Self {
        let keys: FxHashSet<UnitKey> = units.iter().map(|&u| game.unit(u).key).collect();
        let mut first_local = 1 << 16;
        let mut local_rooms: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();
        for &u in &units {
            if let UnitKey::Local(r, n) = game.unit(u).key {
                first_local = first_local.min(n);
                local_rooms.entry(n).or_default().insert(r);
            }
        }
        CallReader { game, units, keys, code, first_local, local_rooms }
    }

    /// The units a start of script `n` from `caller` runs (script.cpp:70-86).
    pub fn targets(&self, caller: UnitKey, n: i64) -> Vec<UnitKey> {
        let n32 = n as i32;
        if n < self.first_local as i64 {
            let k = UnitKey::Global(n32);
            return if self.keys.contains(&k) { vec![k] } else { vec![] };
        }
        match caller {
            UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => {
                let k = UnitKey::Local(r, n32);
                if self.keys.contains(&k) {
                    vec![k]
                } else {
                    vec![]
                }
            }
            _ => self
                .local_rooms
                .get(&n32)
                .map_or(Vec::new(), |rs| rs.iter().map(|&r| UnitKey::Local(r, n32)).collect()),
        }
    }

    pub fn written_locals(&self, u: UnitId) -> Written {
        let ast = &self.game.ast;
        let mut out = Written::default();
        let ins = self.game.ins(u);
        for &i in &self.code[&u] {
            let Some(st) = ins[i as usize].st else { continue };
            match st {
                Stmt::Assign(a) => {
                    if let Expr::Local(n) = ast.expr(a.lhs) {
                        out.locals.insert(n);
                    }
                }
                Stmt::Op { name, args, .. } if name == known::SET_VAR_RANGE => {
                    let a = ast.kids(args);
                    if let Some(&first) = a.first() {
                        if let Expr::Local(n) = ast.expr(first) {
                            let len = list_len(ast, a[2]);
                            out.locals.extend(n..n + len as i32);
                        }
                    }
                }
                _ => {}
            }
            for e in stmt_nodes(ast, &st) {
                if let Expr::Index { kind: RefKind::Local, .. } = ast.expr(e) {
                    out.indexed = true;
                }
            }
        }
        out
    }

    /// Units the engine starts itself: every constant written to a hook variable.
    pub fn hook_units(&self, counts: &mut Counter, notes: &mut Vec<(String, String)>, caps: &mut i64) -> FxHashSet<UnitKey> {
        let ast = &self.game.ast;
        let mut out = FxHashSet::default();
        for &u in &self.units {
            let key = self.game.unit(u).key;
            let ins = self.game.ins(u);
            for &i in &self.code[&u] {
                let Some(Stmt::Assign(a)) = ins[i as usize].st else { continue };
                let Expr::Var(n) = ast.expr(a.lhs) else { continue };
                if !HOOKS.contains(&n) {
                    continue;
                }
                let vals = if a.op == AssignOp::Set { fold(ast, a.rhs, None, None, caps) } else { None };
                if vals.is_none() {
                    counts.inc("hook variable writes that do not fold (assumed to start no other unit)");
                    notes.push(("hook write does not fold".into(), self.game.cite(u, i)));
                }
                for n in vals.unwrap_or_default() {
                    out.extend(self.targets(key, n));
                }
            }
        }
        out
    }

    /// The bound locals of every unit and the locals each unit writes (domains.py `_Reader.locals`).
    pub fn locals(
        &self,
        engine_started: &FxHashSet<UnitKey>,
        counts: &mut Counter,
        notes: &mut Vec<(String, String)>,
        caps: &mut i64,
    ) -> (FxHashMap<UnitKey, Env>, FxHashMap<UnitKey, Written>) {
        let ast = &self.game.ast;
        let mut order: Vec<UnitKey> = Vec::new();
        let mut sites: FxHashMap<UnitKey, Vec<(UnitKey, Span)>> = FxHashMap::default();
        for &u in &self.units {
            let key = self.game.unit(u).key;
            let ins = self.game.ins(u);
            for &i in &self.code[&u] {
                let Some(Stmt::Start { name, script, obj, args, .. }) = ins[i as usize].st else { continue };
                let callees: Vec<UnitKey> = if name == known::START_OBJECT {
                    let o = obj.and_then(|o| fold(ast, o, None, None, caps));
                    match o {
                        Some(o) if o.len() == 1 => {
                            let k = UnitKey::Obj(o[0] as i32);
                            if self.keys.contains(&k) {
                                vec![k]
                            } else {
                                vec![]
                            }
                        }
                        _ => {
                            counts.inc(
                                "starts of an object that does not fold (assumed to reach no unit whose locals are bound)",
                            );
                            notes.push(("startObject: object not a constant".into(), self.game.cite(u, i)));
                            continue;
                        }
                    }
                } else {
                    match ast.constant(script) {
                        Some(n) => self.targets(key, n as i64),
                        None => {
                            counts.inc(
                                "starts of a script number that does not fold (assumed to reach no unit whose locals are bound)",
                            );
                            notes.push(("start: number not a constant".into(), self.game.cite(u, i)));
                            continue;
                        }
                    }
                };
                for c in callees {
                    let e = sites.entry(c).or_default();
                    if e.is_empty() {
                        order.push(c);
                    }
                    e.push((key, args));
                }
            }
        }
        let mut env: FxHashMap<UnitKey, Env> = FxHashMap::default();
        for &u in &self.units {
            let key = self.game.unit(u).key;
            if matches!(key, UnitKey::Entry(_) | UnitKey::Exit(_)) {
                env.insert(key, (0..NUM_LOCALS).map(|k| (k, vec![0])).collect());
            }
        }
        let written: FxHashMap<UnitKey, Written> =
            self.units.iter().map(|&u| (self.game.unit(u).key, self.written_locals(u))).collect();
        let mut changed = true;
        while changed {
            changed = false;
            for callee in &order {
                let w = &written[callee];
                if engine_started.contains(callee) || w.indexed {
                    continue;
                }
                for k in 0..NUM_LOCALS {
                    if w.locals.contains(&k) {
                        continue;
                    }
                    let mut vals: Option<BTreeSet<i64>> = Some(BTreeSet::new());
                    for (caller, args) in &sites[callee] {
                        let a = ast.kids(*args);
                        let v = if (k as usize) < a.len() {
                            fold(ast, a[k as usize], env.get(caller), None, caps)
                        } else {
                            Some(vec![0])
                        };
                        match v {
                            None => {
                                vals = None;
                                break;
                            }
                            Some(v) => vals.as_mut().unwrap().extend(v),
                        }
                    }
                    if let Some(vals) = vals {
                        let vals: ISet = vals.into_iter().collect();
                        let e = env.entry(*callee).or_default();
                        if e.get(&k) != Some(&vals) {
                            e.insert(k, vals);
                            changed = true;
                        }
                    }
                }
            }
        }
        for key in engine_started {
            env.remove(key);
        }
        (env, written)
    }
}

/// The number of items of a List expression (`a[2].items`).
pub fn list_len(ast: &Ast, e: ExprId) -> usize {
    match ast.expr(e) {
        Expr::List(s) => s.len as usize,
        other => panic!("not a list: {other:?}"),
    }
}

pub fn list_items(ast: &Ast, e: ExprId) -> &[ExprId] {
    match ast.expr(e) {
        Expr::List(s) => ast.kids(s),
        other => panic!("not a list: {other:?}"),
    }
}

/// domains.py `Domains`.
#[derive(Clone, Debug)]
pub struct Domains {
    pub doms: BTreeMap<String, Domain>,
    pub why: BTreeMap<String, String>,
    pub lossy: BTreeMap<String, i64>,
    pub incomplete: BTreeMap<String, String>,
    pub counts: Counter,
    pub notes: Vec<(String, String)>,
    binary: Domain,
}

impl Domains {
    /// `Domains.get`: the domain of a fact key; None when it has no finite domain.
    pub fn get(&self, key: &str) -> Option<&Domain> {
        if BINARY_PREFIXES.iter().any(|p| key.starts_with(p)) {
            return Some(&self.binary);
        }
        self.doms.get(key)
    }

    /// golden.py's encoding of the dataclass.
    pub fn py(&self) -> Py {
        Py::obj(
            "Domains",
            vec![
                ("doms", Py::Dict(self.doms.iter().map(|(k, d)| (k.clone(), d.py())).collect())),
                ("why", Py::Dict(self.why.iter().map(|(k, v)| (k.clone(), Py::str(v))).collect())),
                ("lossy", Py::Dict(self.lossy.iter().map(|(k, v)| (k.clone(), Py::Int(*v))).collect())),
                ("incomplete", Py::Dict(self.incomplete.iter().map(|(k, v)| (k.clone(), Py::str(v))).collect())),
                ("counts", self.counts.py()),
                (
                    "notes",
                    Py::List(self.notes.iter().map(|(a, b)| Py::Tuple(vec![Py::str(a), Py::str(b)])).collect()),
                ),
            ],
        )
    }
}

/// `Counter.most_common(1)[0][0]` over a counter kept in first-count order.
fn most_common(c: &[(String, i64)]) -> &str {
    let mut best: Option<&(String, i64)> = None;
    for x in c {
        if best.is_none_or(|b| x.1 > b.1) {
            best = Some(x);
        }
    }
    &best.unwrap().0
}

fn bump(c: &mut Vec<(String, i64)>, k: String) {
    match c.iter_mut().find(|x| x.0 == k) {
        Some(x) => x.1 += 1,
        None => c.push((k, 1)),
    }
}

/// The room numbers of the game data (index.json's "rooms"), for domains.py's widened `room`.
pub fn meta_rooms(root: &std::path::Path) -> Vec<i32> {
    let p = root.join("data").join("scripts-dos-cd").join("index.json");
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).expect("index.json")).expect("index.json");
    let mut out: Vec<i32> = v["rooms"].as_object().expect("rooms").keys().map(|k| k.parse().expect("room")).collect();
    out.sort_unstable();
    out
}

/// domains.py `build(start, goals)` over every unit of the game, with the default settings.
pub fn build(game: &Game, rooms_meta: &[i32], start: &Facts, goals: &[(String, Vec<i64>)]) -> Domains {
    build_with(game, rooms_meta, start, goals, Switches::default())
}

/// `build` with fix round 1's switches (fix-round1.md; domains.py has none): with them off, `build`. F5
/// (draw_clears): a global script's drawObject clears objects of the room it runs in, so their states take 0 too (a
/// room's own code already adds its room's: `draw_rooms`); `objpos:O` of a movable object has its places' indices.
pub fn build_with(game: &Game, rooms_meta: &[i32], start: &Facts, goals: &[(String, Vec<i64>)], sw: Switches) -> Domains {
    let ast = &game.ast;
    let settings = settings_env();
    let mut caps = 0i64;
    let mut counts = Counter::default();
    let mut notes: Vec<(String, String)> = Vec::new();
    let units: Vec<UnitId> = game.ids().collect();
    let mut code: FxHashMap<UnitId, Vec<u32>> = FxHashMap::default();
    let mut total = 0i64;
    let mut anywhere = 0i64;
    let mut kept = 0i64;
    for &u in &units {
        let c = live(game, u, &settings, None, &mut caps);
        total += game.ins(u).len() as i64;
        anywhere += live(game, u, &VarEnv::default(), None, &mut caps).len() as i64;
        kept += c.len() as i64;
        code.insert(u, c);
    }
    counts.set("instructions", total);
    counts.set("instructions no start reaches, whatever the settings", total - anywhere);
    counts.set("instructions left out by the settings (only a decided jump leads there)", anywhere - kept);
    let rd = CallReader::new(game, units.clone(), code);
    let engine_started = rd.hook_units(&mut counts, &mut notes, &mut caps);
    let (env, written) = rd.locals(&engine_started, &mut counts, &mut notes, &mut caps);
    counts.set("units the engine starts with its own arguments (engine-bound locals)", engine_started.len() as i64);

    let mut consts: BTreeMap<i32, BTreeSet<i64>> = BTreeMap::new();
    let mut deltas: BTreeMap<i32, BTreeSet<i64>> = BTreeMap::new();
    let mut bad: BTreeMap<i32, Vec<(String, i64)>> = BTreeMap::new();
    let mut copies: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();
    let mut compared: BTreeMap<i32, BTreeSet<i64>> = BTreeMap::new();
    let mut cmp_vars: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();
    let mut reads: FxHashMap<i32, i64> = FxHashMap::default();
    let mut later: Vec<(scumm_ir::Sym, UnitId, u32)> = Vec::new();
    let mut indexed: Vec<(i32, String)> = Vec::new();

    let local_why = |e: ExprId, key: UnitKey| -> &'static str {
        let nodes = expr_nodes(ast, e);
        let ks: BTreeSet<i32> = nodes
            .iter()
            .filter_map(|&y| match ast.expr(y) {
                Expr::Local(n) => Some(n),
                _ => None,
            })
            .collect();
        if nodes.iter().any(|&y| matches!(ast.expr(y), Expr::Getter { .. } | Expr::Embed(_))) {
            return "a getter";
        }
        if nodes.iter().any(|&y| matches!(ast.expr(y), Expr::Var(_) | Expr::Index { .. })) {
            return "another variable";
        }
        if ks.iter().any(|k| written[&key].locals.contains(k)) {
            return "a local its unit writes";
        }
        if engine_started.contains(&key) && !ks.is_empty() {
            return "an engine-bound local";
        }
        if !ks.is_empty() {
            return "an unbound local";
        }
        "an expression that does not fold"
    };

    for &u in &units {
        let key = game.unit(u).key;
        let e = env.get(&key);
        let ins = game.ins(u);
        for &i in &rd.code[&u] {
            let Some(st) = ins[i as usize].st else { continue };
            let mut write = |n: i32, op: AssignOp, rhs: ExprId, caps: &mut i64, bad: &mut BTreeMap<i32, Vec<(String, i64)>>| {
                match op {
                    AssignOp::Mul | AssignOp::Div => {
                        let what = if op == AssignOp::Mul { "multiply" } else { "divide" };
                        bump(bad.entry(n).or_default(), format!("a {what} of itself"));
                    }
                    AssignOp::Add | AssignOp::Sub | AssignOp::Inc | AssignOp::Dec => match fold(ast, rhs, e, None, caps) {
                        None => bump(bad.entry(n).or_default(), format!("a step of {}", local_why(rhs, key))),
                        Some(s) => {
                            let up = matches!(op, AssignOp::Add | AssignOp::Inc);
                            deltas.entry(n).or_default().extend(s.iter().map(|&v| if up { v } else { -v }));
                        }
                    },
                    AssignOp::Set => {
                        if let Some(s) = fold(ast, rhs, e, None, caps) {
                            consts.entry(n).or_default().extend(s);
                            return;
                        }
                        if let Expr::BinOp { op: bop, a, b } = ast.expr(rhs) {
                            if matches!(bop, ArithOp::Add | ArithOp::Sub)
                                && (is_var(ast, a, n) || (bop == ArithOp::Add && is_var(ast, b, n)))
                            {
                                let other = if is_var(ast, a, n) { b } else { a };
                                if let Some(d) = fold(ast, other, e, None, caps) {
                                    deltas
                                        .entry(n)
                                        .or_default()
                                        .extend(d.iter().map(|&v| if bop == ArithOp::Add { v } else { -v }));
                                    return;
                                }
                            }
                        }
                        if let Expr::Var(m) = ast.expr(rhs) {
                            copies.entry(n).or_default().insert(m);
                            bump(bad.entry(n).or_default(), "a copy of another variable".into());
                            return;
                        }
                        bump(bad.entry(n).or_default(), format!("a value from {}", local_why(rhs, key)));
                    }
                }
            };
            if let Stmt::Jump { cond, .. } = st {
                if let Some(c) = cond {
                    // compare(c, key)
                    match ast.expr(c) {
                        Expr::Truth(x) | Expr::Not(x) if matches!(ast.expr(x), Expr::Var(_)) => {
                            let Expr::Var(n) = ast.expr(x) else { unreachable!() };
                            compared.entry(n).or_default().insert(0);
                        }
                        Expr::Cmp { a, b, .. } => {
                            for (x, y) in [(a, b), (b, a)] {
                                if let Expr::Var(xn) = ast.expr(x) {
                                    if let Some(s) = fold(ast, y, e, None, &mut caps) {
                                        compared.entry(xn).or_default().extend(s);
                                    } else if let Expr::Var(yn) = ast.expr(y) {
                                        cmp_vars.entry(xn).or_default().insert(yn);
                                    } else {
                                        counts.inc(format!("comparisons of a variable with {}", local_why(y, key)));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    for y in expr_nodes(ast, c) {
                        if let Expr::Index { kind: RefKind::Var, .. } = ast.expr(y) {
                            counts.inc("comparisons on an indexed variable (left to the reader)");
                        }
                    }
                }
                continue;
            }
            for y in stmt_nodes(ast, &st) {
                if let Expr::Embed(a) = ast.expr(y) {
                    if let Expr::Var(n) = ast.expr(a.lhs) {
                        bump(bad.entry(n).or_default(), "a value from a getter".into());
                    }
                }
            }
            if let Stmt::Assign(a) = st {
                match ast.expr(a.lhs) {
                    Expr::Var(n) => write(n, a.op, a.rhs, &mut caps, &mut bad),
                    Expr::Index { kind: RefKind::Var, base, offset } => match fold(ast, offset, e, None, &mut caps) {
                        None => indexed.push((base, game.cite(u, i))),
                        Some(s) => {
                            for v in s {
                                write(base + v as i32, a.op, a.rhs, &mut caps, &mut bad);
                            }
                        }
                    },
                    _ => {}
                }
                let lhs_var = match ast.expr(a.lhs) {
                    Expr::Var(n) => Some(n),
                    _ => None,
                };
                let self_step = match (lhs_var, ast.expr(a.rhs)) {
                    (Some(n), Expr::BinOp { a: x, b: y, .. }) => is_var(ast, x, n) || is_var(ast, y, n),
                    _ => false,
                };
                let mut nodes = expr_nodes(ast, a.rhs);
                if let Expr::Index { offset, .. } = ast.expr(a.lhs) {
                    nodes.extend(expr_nodes(ast, offset));
                }
                for y in nodes {
                    if let Expr::Var(n) = ast.expr(y) {
                        if !(self_step && Some(n) == lhs_var) {
                            *reads.entry(n).or_default() += 1;
                        }
                    }
                }
                continue;
            }
            for y in stmt_nodes(ast, &st) {
                if let Expr::Var(n) = ast.expr(y) {
                    *reads.entry(n).or_default() += 1;
                }
            }
            if let Stmt::Op { name, args, .. } = st {
                let a = ast.kids(args);
                if name == known::SET_VAR_RANGE && !a.is_empty() {
                    if let Expr::Var(first) = ast.expr(a[0]) {
                        for (k, &v) in list_items(ast, a[2]).iter().enumerate() {
                            write(first + k as i32, AssignOp::Set, v, &mut caps, &mut bad);
                        }
                    }
                } else if [
                    known::SET_OWNER_OF,
                    known::PICKUP_OBJECT,
                    known::SET_STATE,
                    known::DRAW_OBJECT,
                    known::LOAD_ROOM,
                    known::LOAD_ROOM_WITH_EGO,
                    known::ACTOR_FOLLOW_CAMERA,
                ]
                .contains(&name)
                {
                    later.push((name, u, i));
                }
            }
        }
    }
    for (k, vals) in goals {
        if let Some(n) = k.strip_prefix("var:") {
            compared.entry(n.parse().unwrap()).or_default().extend(vals.iter().copied());
        }
    }

    // variables: the plan's rule, then comparisons with variables whose domains are exact, until nothing changes
    let mut var_nums: BTreeSet<i32> = BTreeSet::new();
    var_nums.extend(consts.keys());
    var_nums.extend(deltas.keys());
    var_nums.extend(bad.keys());
    var_nums.extend(compared.keys());
    var_nums.extend(cmp_vars.keys());
    for (k, _) in start.iter() {
        if let Some(n) = k.strip_prefix("var:") {
            var_nums.insert(n.parse().unwrap());
        }
    }
    let special: BTreeSet<i32> = ROOM_VARS.iter().chain(BOOL_VARS.iter()).copied().chain([VAR_WALKTO_OBJ]).collect();
    let mut doms: BTreeMap<String, Domain> = BTreeMap::new();
    let mut why: BTreeMap<String, String> = BTreeMap::new();
    for &n in var_nums.difference(&special) {
        if let Some(w) = engine_why(n) {
            why.insert(format!("var:{n}"), format!("written by the engine: {w}"));
        } else if let Some(b) = bad.get(&n).filter(|b| !b.is_empty()) {
            why.insert(format!("var:{n}"), most_common(b).to_string());
        }
    }
    for (n, v) in SETTINGS {
        why.insert(format!("var:{n}"), format!("fixed by the settings (value {v}), not a fact"));
    }
    let empty = BTreeSet::new();
    let mut rounds = 0;
    let mut counters: BTreeSet<String>;
    loop {
        rounds += 1;
        let before = doms.clone();
        counters = BTreeSet::new();
        for &n in var_nums.difference(&special) {
            let key = format!("var:{n}");
            if why.contains_key(&key) {
                continue;
            }
            let mut cmp: BTreeSet<i64> = compared.get(&n).cloned().unwrap_or_default();
            for m in cmp_vars.get(&n).unwrap_or(&BTreeSet::new()) {
                if let Some(d) = doms.get(&format!("var:{m}")) {
                    if d.exact() {
                        cmp.extend(d.values.iter().filter_map(|l| l.int()));
                    }
                }
            }
            let dom = closure(
                start.get0(&key),
                consts.get(&n).unwrap_or(&empty),
                deltas.get(&n).unwrap_or(&empty),
                &cmp,
            );
            match dom {
                None => {
                    counters.insert(key.clone());
                    doms.remove(&key);
                }
                Some(d) => {
                    doms.insert(key, d);
                }
            }
        }
        if doms == before {
            break;
        }
    }
    for key in &counters {
        why.insert(key.clone(), "a counter never compared with a constant".into());
    }
    counts.set("rounds of the comparison fixpoint", rounds);
    let exact: VarEnv = doms
        .iter()
        .filter(|(k, d)| k.starts_with("var:") && d.exact())
        .map(|(k, d)| (k[4..].parse().unwrap(), d.values.iter().map(|l| l.int().unwrap()).collect()))
        .collect();
    let mut lossy: BTreeMap<String, i64> = BTreeMap::new();
    for &n in &var_nums {
        let key = format!("var:{n}");
        if let Some(d) = doms.get(&key) {
            if cmp_vars
                .get(&n)
                .is_some_and(|ms| ms.iter().any(|m| doms.get(&format!("var:{m}")).is_none_or(|x| !x.exact())))
            {
                counts.inc("finite variables compared with a variable whose domain is not exact");
            }
            let r = reads.get(&n).copied().unwrap_or(0);
            if !d.exact() && r != 0 {
                lossy.insert(key.clone(), r);
            }
            if d.values.contains(&Label::Below) {
                counts.inc("finite variables that need BELOW (the addition)");
            }
        } else if copies.get(&n).is_some_and(|c| !c.is_empty())
            && bad.get(&n).is_some_and(|b| b.len() == 1 && b[0].0 == "a copy of another variable")
            && copies[&n].iter().all(|m| doms.contains_key(&format!("var:{m}")))
        {
            counts.inc("variables left out only for copies of finite variables");
        }
    }

    // owners, states, rooms: the writes, with variables of exact domains folded
    let mut incomplete: BTreeMap<String, String> = BTreeMap::new();
    let mut owner_vals: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
    let mut state_vals: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
    let mut owner_ego: BTreeSet<i64> = BTreeSet::new();
    let mut draw_rooms: BTreeSet<Option<i32>> = BTreeSet::new();
    let mut room_vals: BTreeSet<i64> = BTreeSet::new();
    let mut room_wild: Vec<String> = Vec::new();
    let mut walkto: BTreeSet<i64> = BTreeSet::new();
    let ego = doms.get(&format!("var:{VAR_EGO}")).cloned();
    for &(name, u, i) in &later {
        let key = game.unit(u).key;
        let e = env.get(&key);
        let x = &game.ins(u)[i as usize];
        let Some(Stmt::Op { args, subops, .. }) = x.st else { unreachable!() };
        let a = ast.kids(args);
        let f = |i: usize, caps: &mut i64| if i < a.len() { fold(ast, a[i], e, Some(&exact), caps) } else { None };
        let cite = game.cite(u, i);
        if name == known::SET_OWNER_OF {
            let objs = f(0, &mut caps);
            let vals = f(1, &mut caps);
            let Some(objs) = objs else {
                counts.inc("owner writes whose object does not fold (target not known)");
                notes.push(("setOwnerOf: object does not fold".into(), cite));
                continue;
            };
            let Some(vals) = vals else {
                counts.inc("owner writes whose value does not fold (domain incomplete)");
                for o in objs {
                    incomplete.insert(format!("owner:{o}"), cite.clone());
                }
                continue;
            };
            for o in objs {
                owner_vals.entry(o).or_default().extend(vals.iter().copied());
            }
        } else if name == known::PICKUP_OBJECT {
            let Some(objs) = f(0, &mut caps) else {
                counts.inc("owner writes whose object does not fold (target not known)");
                notes.push(("pickupObject: object does not fold".into(), cite));
                continue;
            };
            for o in objs {
                owner_ego.insert(o);
                state_vals.entry(o).or_default().insert(1);
            }
        } else if name == known::SET_STATE || name == known::DRAW_OBJECT {
            let objs = f(0, &mut caps);
            let vals = if name == known::SET_STATE {
                f(1, &mut caps)
            } else {
                let img = ast.subs(subops).iter().find(|s| s.name == known::SET_IMAGE).copied();
                draw_rooms.insert(game.unit(u).room);
                if sw.draw_clears && game.unit(u).room.is_none() {
                    // F5: the rooms of the objects a global script draws (any room when the object does not fold)
                    match objs.as_ref() {
                        Some(os) => draw_rooms.extend(os.iter().map(|&o| game.object_room.get(&(o as i32)).copied())),
                        None => draw_rooms.extend(game.object_room.values().map(|&r| Some(r))),
                    }
                }
                match img {
                    Some(s) => fold(ast, ast.kids(s.args)[0], e, Some(&exact), &mut caps),
                    None => Some(vec![1]),
                }
            };
            let what = game.syms.str(name);
            let Some(objs) = objs else {
                counts.inc("state writes whose object does not fold (target not known)");
                notes.push((format!("{what}: object does not fold"), cite));
                continue;
            };
            let Some(vals) = vals else {
                counts.inc("state writes whose value does not fold (domain incomplete)");
                for o in objs {
                    incomplete.insert(format!("state:{o}"), cite.clone());
                }
                continue;
            };
            for o in objs {
                state_vals.entry(o).or_default().extend(vals.iter().copied());
            }
        } else if name == known::LOAD_ROOM || name == known::LOAD_ROOM_WITH_EGO {
            let r = f(if name == known::LOAD_ROOM { 0 } else { 1 }, &mut caps);
            match r {
                None => room_wild.push(cite),
                Some(r) => room_vals.extend(r),
            }
            if name == known::LOAD_ROOM_WITH_EGO {
                walkto.extend(f(0, &mut caps).unwrap_or_default());
            }
        } else {
            room_wild.push(cite);
        }
    }
    let rooms: BTreeSet<i64> = rooms_meta.iter().map(|&r| r as i64).collect();
    let mut pseudo: BTreeSet<i64> = BTreeSet::new();
    for u in game.ids() {
        for x in game.ins(u) {
            if let Some(Stmt::Op { name, args, .. }) = x.st {
                if name == known::PSEUDO_ROOM {
                    for &a in ast.kids(args).iter().skip(1) {
                        if let Some(v) = ast.constant(a) {
                            pseudo.insert((v | 0x80) as i64);
                        }
                    }
                }
            }
        }
    }
    let mut room_all: BTreeSet<i64> = room_vals.clone();
    room_all.insert(start.get0("room"));
    if !room_wild.is_empty() {
        room_all.extend(rooms.iter().copied());
        room_all.extend(pseudo.iter().copied());
        counts.set("room changes to a room that does not fold (room widened to every room)", room_wild.len() as i64);
        notes.extend(room_wild.iter().map(|c| ("room change does not fold".to_string(), c.clone())));
    }
    doms.insert("room".into(), Domain::of(room_all.iter().copied(), None, None));
    let cst = |n: i32| consts.get(&n).cloned().unwrap_or_default();
    for n in ROOM_VARS {
        let mut v = room_all.clone();
        v.insert(start.get0(&format!("var:{n}")));
        v.extend(cst(n));
        doms.insert(format!("var:{n}"), Domain::of(v, None, None));
    }
    for n in BOOL_VARS {
        let mut v: BTreeSet<i64> = [0, 1, start.get0(&format!("var:{n}"))].into_iter().collect();
        v.extend(cst(n));
        doms.insert(format!("var:{n}"), Domain::of(v, None, None));
    }
    let mut v: BTreeSet<i64> = [0, start.get0(&format!("var:{VAR_WALKTO_OBJ}"))].into_iter().collect();
    v.extend(walkto);
    doms.insert(format!("var:{VAR_WALKTO_OBJ}"), Domain::of(v, None, None));
    let ego_exact = ego.as_ref().is_some_and(|d| d.exact());
    if ego_exact {
        doms.insert("ego".into(), ego.clone().unwrap());
    } else if !owner_ego.is_empty() {
        counts.add("pickupObject owners: VAR_EGO has no exact domain", owner_ego.len() as i64);
    }
    let mut owners: BTreeSet<i64> = owner_vals.keys().copied().collect();
    owners.extend(owner_ego.iter().copied());
    owners.extend(start.iter().filter_map(|(k, _)| k.strip_prefix("owner:").map(|o| o.parse::<i64>().unwrap())));
    for o in owners {
        let mut vals = owner_vals.get(&o).cloned().unwrap_or_default();
        vals.insert(start.get0(&format!("owner:{o}")));
        if owner_ego.contains(&o) && ego_exact {
            vals.extend(ego.as_ref().unwrap().values.iter().filter_map(|l| l.int()));
        }
        doms.insert(format!("owner:{o}"), Domain::of(vals, None, None));
    }
    let obj_room = |o: i64| game.object_room.get(&(o as i32)).copied();
    let mut states: BTreeSet<i64> = state_vals.keys().copied().collect();
    states.extend(start.iter().filter_map(|(k, _)| k.strip_prefix("state:").map(|o| o.parse::<i64>().unwrap())));
    states.extend(game.object_room.iter().filter(|(_, r)| draw_rooms.contains(&Some(**r))).map(|(o, _)| *o as i64));
    for o in states {
        let mut vals = state_vals.get(&o).cloned().unwrap_or_default();
        vals.insert(start.get0(&format!("state:{o}")));
        if draw_rooms.contains(&obj_room(o)) {
            vals.insert(0);
        }
        doms.insert(format!("state:{o}"), Domain::of(vals, None, None));
    }
    doms.insert("cutscene".into(), Domain::of(0..=MAX_CUTSCENE, None, None));
    if sw.draw_clears {
        for (o, places) in &game.movable {
            let k = format!("objpos:{o}");
            let mut vals: BTreeSet<i64> = (0..places.len() as i64).collect();
            vals.insert(start.get0(&k));
            doms.insert(k, Domain::of(vals, None, None));
        }
    }

    if caps > 0 {
        counts.add("expressions past MAX_SET values (not worked out)", caps);
    }
    counts.set("indexed variable writes whose index does not fold (targets not worked out)", indexed.len() as i64);
    notes.extend(indexed.iter().map(|(b, c)| (format!("indexed write Var[{b} + ...]"), c.clone())));
    let near: BTreeSet<i32> =
        indexed.iter().flat_map(|(b, _)| *b..*b + 16).filter(|n| doms.contains_key(&format!("var:{n}"))).collect();
    counts.set("finite variables within 16 above the base of such a write (could be hit)", near.len() as i64);
    Domains { doms, why, lossy, incomplete, counts, notes, binary: Domain::binary() }
}

/// domains.py `_closure`: the start value closed under the writes, collapsed past the compared constants; None for
/// a counter never compared with a constant.
pub fn closure(s: i64, consts: &BTreeSet<i64>, deltas: &BTreeSet<i64>, compared: &BTreeSet<i64>) -> Option<Domain> {
    let mut seeds: BTreeSet<i64> = consts.clone();
    seeds.insert(s);
    let low = *seeds.iter().chain(compared.iter()).min().unwrap();
    if deltas.is_empty() {
        return Some(match compared.iter().max() {
            Some(&hi) => Domain::of(seeds.iter().copied(), Some(low), Some(hi)),
            None => Domain::of(seeds.iter().copied(), None, None),
        });
    }
    let high = *compared.iter().max()?;
    let mut dom_vals: BTreeSet<i64> = BTreeSet::new();
    let mut todo: Vec<i64> = seeds.iter().copied().collect();
    let mut seen: BTreeSet<Label> = BTreeSet::new();
    while let Some(v) = todo.pop() {
        let lab = if v < low {
            Label::Below
        } else if v > high {
            Label::Above
        } else {
            Label::Int(v)
        };
        if !seen.insert(lab) {
            continue;
        }
        match lab {
            Label::Above => {
                for &d in deltas {
                    todo.push(high + 1 + d);
                }
                for &d in deltas.iter().filter(|&&d| d < 0) {
                    todo.extend(high + d + 1..high + 1);
                }
            }
            Label::Below => {
                for &d in deltas {
                    todo.push(low - 1 + d);
                }
                for &d in deltas.iter().filter(|&&d| d > 0) {
                    todo.extend(low..low + d);
                }
            }
            Label::Int(_) => todo.extend(deltas.iter().map(|&d| v + d)),
        }
        dom_vals.insert(match lab {
            Label::Int(_) => v,
            Label::Above => high + 1,
            Label::Below => low - 1,
        });
    }
    Some(Domain::of(dom_vals, Some(low), Some(high)))
}
