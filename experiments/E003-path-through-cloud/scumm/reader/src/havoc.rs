//! The switch havoc_cut (experiments/E004-fast-exact/havoc-cut.md has the rule and its proof): a path the step limit
//! ended becomes one havoc record, in place of stopping the reading. The record holds the path's precondition and its
//! writes so far, then any value, or one of the constants its code writes, or no change, for each fact the code the
//! path can still run may write; each slot that code can start, stop or reach may wait at any of its wait points with
//! unknown locals, or end. Off by default; with it off nothing here runs.

use crate::builder::Builder;
use crate::domain::{Label, Labels};
use crate::domains::{self, fold, list_items, stmt_nodes};
use crate::record::{Cut, CutStep, Eff, Event, PathOut, Pre, Record, What};
use crate::scope::{self, Scope, Writes, CLASS_UNTOUCHABLE};
use crate::table::{FactTable, Kind};
use crate::Py;
use rustc_hash::FxHashMap;
use scumm_ir::{known, AssignOp, Expr, ExprId, Game, RefKind, Stmt, Sym, UnitId, UnitKey};
use std::collections::{BTreeMap, BTreeSet};

/// The hook variables (vars.cpp:62-71): the scroll script, the room's entry and exit hooks, the input and sentence
/// scripts, the inventory script, the cutscene hooks.
const VAR_SCROLL_SCRIPT: i32 = 27;
const ROOM_HOOKS: [i32; 4] = [28, 29, 30, 31];
const VAR_SENTENCE_SCRIPT: i32 = 33;
const VAR_INVENTORY_SCRIPT: i32 = 34;
const VAR_CUTSCENE_START_SCRIPT: i32 = 35;
const VAR_CUTSCENE_END_SCRIPT: i32 = 36;

/// The fact kinds whose every writing construct [`sites`] reads, as `scope_writes` does.
const MODELED: [&str; 7] = ["var:", "bit:", "state:", "owner:", "class:", "verb:", "saved:"];

/// The code a cut path can still run (havoc-cut.md, "The write set").
#[derive(Debug, Default)]
pub struct Closure {
    /// each unit, with the instructions its code is read from
    pub starts: BTreeMap<UnitId, BTreeSet<u32>>,
    /// a reachable instruction can change the room
    pub room_change: bool,
    /// a start the closure could not work out: every unit of the scope
    pub every_unit: bool,
    /// the slots a reachable stopScript or stopObjectScript names
    pub stops: BTreeSet<UnitKey>,
    /// a stop whose number does not fold: every slot
    pub stops_all: bool,
    /// the names of the ops in the reachable code
    pub ops: BTreeSet<Sym>,
    /// the engine's own writes among the pending steps
    pub engine_writes: BTreeSet<String>,
    /// the hook variables a reachable instruction runs
    pub hooks: BTreeSet<i32>,
}

impl Closure {
    pub fn units(&self) -> Vec<UnitId> {
        self.starts.keys().copied().collect()
    }
    fn has_op(&self, names: &[Sym]) -> bool {
        names.iter().any(|n| self.ops.contains(n))
    }
}

/// The instructions a whole unit is read from: an object's verb entries, else its first instruction.
fn entries(game: &Game, u: UnitId) -> Vec<u32> {
    let unit = game.unit(u);
    match unit.key {
        UnitKey::Obj(_) => unit.entries.iter().map(|e| e.1).collect(),
        _ => vec![0],
    }
}

/// The room resource a unit's code runs in, when it is room code.
fn unit_res(k: UnitKey) -> Option<i32> {
    match k {
        UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
        _ => None,
    }
}

/// The code a cut path can still run: the frames' code from where each stands, every unit a start in it names, the
/// hooks it runs, and the room code of the scope when it can change the room (havoc-cut.md). `room` is the room
/// resource the path stands in, when its room is known: while no instruction of the closure can change the room, a
/// local script number names that room's script (`eng_script` reads the room when it starts it).
pub fn closure(game: &Game, scope: &Scope, table: &FactTable, cut: &Cut, room: Option<i32>) -> Closure {
    let ast = &game.ast;
    let settings = domains::settings_env();
    let mut caps = 0i64;
    let mut c = Closure::default();
    let all_res: BTreeSet<i32> = scope.rooms.iter().filter_map(|&r| game.resource(r)).collect();
    // the local script numbers started, with the room resource of the code that started them (None: any room)
    let mut locals: BTreeSet<(i32, Option<i32>)> = BTreeSet::new();
    let mut todo: Vec<(UnitId, Vec<u32>)> = Vec::new();
    let add_script = |n: i32, res: Option<i32>, todo: &mut Vec<(UnitId, Vec<u32>)>, locals: &mut BTreeSet<(i32, Option<i32>)>| {
        if n == 0 {
            return;
        }
        if n < game.first_local {
            if let Some(u) = game.id(UnitKey::Global(n)) {
                todo.push((u, entries(game, u)));
            }
        } else {
            locals.insert((n, res));
        }
    };
    for &(k, pc) in &cut.frames {
        if let Some(u) = game.id(k) {
            todo.push((u, vec![pc.unwrap_or(0)]));
        }
    }
    for s in &cut.steps {
        match s {
            CutStep::Script(n) => add_script(*n, room, &mut todo, &mut locals),
            CutStep::Object(o) => {
                if let Some(u) = game.id(UnitKey::Obj(*o)) {
                    todo.push((u, entries(game, u)));
                }
            }
            CutStep::Hook(v) => {
                c.hooks.insert(*v);
            }
            CutStep::Write(k) => {
                c.engine_writes.insert(k.clone());
            }
            CutStep::Room => c.room_change = true,
            CutStep::Other(_) => c.every_unit = true,
        }
    }
    let mut room_added = false;
    let mut hooks_done: BTreeSet<i32> = BTreeSet::new();
    let mut every_done = false;
    loop {
        while let Some((u, starts)) = todo.pop() {
            let have = c.starts.entry(u).or_default();
            let new: Vec<u32> = starts.into_iter().filter(|s| !have.contains(s)).collect();
            if new.is_empty() {
                continue;
            }
            have.extend(new.iter().copied());
            let key = game.unit(u).key;
            let res = if c.room_change { None } else { unit_res(key).or(room) };
            let ins = game.ins(u);
            for i in domains::live(game, u, &settings, Some(&new), &mut caps) {
                let Some(st) = ins[i as usize].st else { continue };
                match st {
                    Stmt::Start { name, script, obj, .. } => {
                        if name == known::START_OBJECT {
                            match obj.and_then(|o| fold(ast, o, None, None, &mut caps)) {
                                Some(os) => {
                                    for o in os {
                                        if let Some(v) = game.id(UnitKey::Obj(o as i32)) {
                                            todo.push((v, entries(game, v)));
                                        }
                                    }
                                }
                                None => c.every_unit = true,
                            }
                        } else {
                            match fold(ast, script, None, None, &mut caps) {
                                Some(ns) => {
                                    for n in ns {
                                        add_script(n as i32, res, &mut todo, &mut locals);
                                    }
                                }
                                None => c.every_unit = true,
                            }
                        }
                    }
                    Stmt::Op { name, args, .. } => {
                        c.ops.insert(name);
                        let a = ast.kids(args);
                        match name {
                            known::LOAD_ROOM | known::LOAD_ROOM_WITH_EGO | known::ACTOR_FOLLOW_CAMERA => {
                                c.room_change = true;
                                c.hooks.extend([VAR_SCROLL_SCRIPT, VAR_INVENTORY_SCRIPT]);
                            }
                            known::SET_CAMERA_AT => {
                                c.hooks.insert(VAR_SCROLL_SCRIPT);
                            }
                            known::CUTSCENE => {
                                c.hooks.insert(VAR_CUTSCENE_START_SCRIPT);
                            }
                            known::END_CUTSCENE => {
                                c.hooks.insert(VAR_CUTSCENE_END_SCRIPT);
                            }
                            known::SET_OWNER_OF | known::PICKUP_OBJECT => {
                                c.hooks.insert(VAR_INVENTORY_SCRIPT);
                            }
                            known::DO_SENTENCE => {
                                c.hooks.insert(VAR_SENTENCE_SCRIPT);
                            }
                            known::SYSTEM_OPS => c.every_unit = true,
                            known::STOP_SCRIPT | known::STOP_OBJECT_SCRIPT => {
                                match a.first().and_then(|&e| fold(ast, e, None, None, &mut caps)) {
                                    Some(ns) => {
                                        for n in ns {
                                            let n = n as i32;
                                            if name == known::STOP_OBJECT_SCRIPT {
                                                c.stops.insert(UnitKey::Obj(n));
                                            } else if n == 0 {
                                                // stopScript(0) stops the running slot, which is in the closure
                                            } else if n < game.first_local {
                                                c.stops.insert(UnitKey::Global(n));
                                            } else {
                                                for &r in &all_res {
                                                    c.stops.insert(UnitKey::Local(r, n));
                                                }
                                            }
                                        }
                                    }
                                    None => c.stops_all = true,
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut more = false;
        if c.every_unit && !every_done {
            every_done = true;
            c.room_change = true;
            c.stops_all = true;
            for &u in &scope.units {
                todo.push((u, entries(game, u)));
            }
            more = true;
        }
        if c.room_change && !room_added {
            room_added = true;
            for &r in &all_res {
                for k in [UnitKey::Entry(r), UnitKey::Exit(r)] {
                    if let Some(u) = game.id(k) {
                        todo.push((u, entries(game, u)));
                    }
                }
            }
            c.hooks.extend(ROOM_HOOKS);
            more = true;
        }
        // local scripts: in the room of the code that started them, or with a room change in any scope room
        for &(n, res) in &locals {
            let rs: Vec<i32> = match res {
                Some(r) if !c.room_change => vec![r],
                _ => all_res.iter().copied().collect(),
            };
            for r in rs {
                if let Some(u) = game.id(UnitKey::Local(r, n)) {
                    if !c.starts.contains_key(&u) {
                        todo.push((u, entries(game, u)));
                        more = true;
                    }
                }
            }
        }
        // each hook runs the scripts its variable can name: its domain, else every unit
        let hooks: Vec<i32> = c.hooks.iter().copied().filter(|h| !hooks_done.contains(h)).collect();
        for h in hooks {
            hooks_done.insert(h);
            match table.domain(&format!("var:{h}")) {
                Some(d) => {
                    for v in d.values.iter().filter_map(|l| l.int()) {
                        add_script(v as i32, room, &mut todo, &mut locals);
                    }
                }
                None => c.every_unit = true,
            }
            more = true;
        }
        if !more && todo.is_empty() {
            break;
        }
    }
    c
}

/// Where a write site can land: one fact, every fact with a prefix, a window of 16 variables after a base (an index
/// that does not fold), or a class of every object.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Exact(String),
    Prefix(String),
    Window(i32),
    Class(i64),
}

impl Target {
    fn hits(&self, key: &str) -> bool {
        match self {
            Target::Exact(k) => k == key,
            Target::Prefix(p) => key.starts_with(p.as_str()),
            Target::Window(b) => key.strip_prefix("var:").and_then(|n| n.parse::<i32>().ok()).is_some_and(|n| *b <= n && n < b + 16),
            Target::Class(c) => key.starts_with("class:") && key.rsplit(':').next().and_then(|x| x.parse::<i64>().ok()) == Some(*c),
        }
    }
}

/// One write site: where it lands, and the constants it writes (None: not worked out).
type Site = (Target, Option<BTreeSet<i64>>);

/// The write sites of the code: every construct `scope_writes` reads a target from, with the value it writes when
/// that is a constant (an assignment, setVarRange, setState, setOwnerOf, setClass, drawObject), else None.
/// drawObject draws and clears only objects of the room it runs in (o5_drawObject returns when getObjectIndex finds
/// no local object, script_v5.cpp:1101-1130): the room of room code, else `room`, the path's, while the closure
/// cannot change the room; else every scope room.
fn sites(game: &Game, scope: &Scope, c: &Closure, room: Option<i32>) -> Vec<Site> {
    let ast = &game.ast;
    let settings = domains::settings_env();
    let mut caps = 0i64;
    let mut out: Vec<Site> = Vec::new();
    let consts = |e: ExprId, caps: &mut i64| fold(ast, e, None, None, caps).map(|v| v.into_iter().collect::<BTreeSet<i64>>());
    fn lhs(ast: &scumm_ir::Ast, e: ExprId, caps: &mut i64) -> Vec<Target> {
        match ast.expr(e) {
            Expr::Var(n) => vec![Target::Exact(format!("var:{n}"))],
            Expr::Bit(n) => vec![Target::Exact(format!("bit:{n}"))],
            Expr::Index { kind: k @ (RefKind::Var | RefKind::Bit), base, offset } => match fold(ast, offset, None, None, caps) {
                Some(s) => s.into_iter().map(|v| Target::Exact(format!("{}:{}", k.as_str(), base as i64 + v))).collect(),
                None if k == RefKind::Var => vec![Target::Window(base)],
                None => vec![Target::Prefix("bit:".into())],
            },
            _ => vec![],
        }
    }
    for (&u, starts) in &c.starts {
        let starts: Vec<u32> = starts.iter().copied().collect();
        let room_of_unit = game.unit(u).room;
        let global = matches!(game.unit(u).key, UnitKey::Global(_));
        let ins = game.ins(u);
        for i in domains::live(game, u, &settings, Some(&starts), &mut caps) {
            let Some(st) = ins[i as usize].st else { continue };
            for y in stmt_nodes(ast, &st) {
                if let Expr::Embed(a) = ast.expr(y) {
                    for t in lhs(ast, a.lhs, &mut caps) {
                        out.push((t, None));
                    }
                }
            }
            match st {
                Stmt::Assign(a) => {
                    let v = if a.op == AssignOp::Set { consts(a.rhs, &mut caps) } else { None };
                    for t in lhs(ast, a.lhs, &mut caps) {
                        out.push((t, v.clone()));
                    }
                }
                Stmt::Op { name, args, .. } => {
                    let a = ast.kids(args);
                    let objs = |e: ExprId, caps: &mut i64| fold(ast, e, None, None, caps);
                    if name == known::SET_VAR_RANGE {
                        let items = list_items(ast, a[2]);
                        match ast.expr(a[0]) {
                            Expr::Var(n) | Expr::Bit(n) => {
                                let kind = if matches!(ast.expr(a[0]), Expr::Var(_)) { "var" } else { "bit" };
                                for (k, &e) in items.iter().enumerate() {
                                    out.push((Target::Exact(format!("{kind}:{}", n + k as i32)), consts(e, &mut caps)));
                                }
                            }
                            Expr::Local(_) => {}
                            _ => {
                                for t in lhs(ast, a[0], &mut caps) {
                                    out.push((t, None));
                                }
                                // an index that folds names its first variable; the next ones follow it
                                out.push((Target::Prefix("var:".into()), None));
                                out.push((Target::Prefix("bit:".into()), None));
                            }
                        }
                    } else if name == known::SET_CLASS {
                        let cls: Vec<Option<Vec<i64>>> = list_items(ast, a[1]).iter().map(|&e| fold(ast, e, None, None, &mut caps)).collect();
                        let os = objs(a[0], &mut caps);
                        let known_cls = cls.iter().all(|x| x.is_some());
                        let flat: Vec<i64> = cls.iter().flatten().flatten().copied().collect();
                        let wild = !known_cls || flat.iter().any(|v| v & 0x7F == 0);
                        let val = |v: i64| -> BTreeSet<i64> { [((v & 0x80) != 0) as i64].into_iter().collect() };
                        match os {
                            Some(os) => {
                                for o in os {
                                    if wild {
                                        out.push((Target::Prefix(format!("class:{o}:")), None));
                                    } else {
                                        for &v in &flat {
                                            out.push((Target::Exact(format!("class:{o}:{}", v & 0x7F)), Some(val(v))));
                                        }
                                    }
                                }
                            }
                            None if wild => out.push((Target::Prefix("class:".into()), None)),
                            None => {
                                for &v in &flat {
                                    out.push((Target::Class(v & 0x7F), Some(val(v))));
                                }
                            }
                        }
                    } else if [known::SET_STATE, known::DRAW_OBJECT, known::PICKUP_OBJECT, known::SET_OWNER_OF].contains(&name) {
                        let os = objs(a[0], &mut caps);
                        let one = || Some([1i64].into_iter().collect::<BTreeSet<i64>>());
                        let (kinds, v): (&[&str], Option<BTreeSet<i64>>) = if name == known::SET_STATE {
                            (&["state"], a.get(1).and_then(|&e| consts(e, &mut caps)))
                        } else if name == known::SET_OWNER_OF {
                            (&["owner"], a.get(1).and_then(|&e| consts(e, &mut caps)))
                        } else if name == known::PICKUP_OBJECT {
                            (&["owner", "state"], None)
                        } else {
                            (&["state"], one())
                        };
                        // drawObject: the objects of the room it runs in
                        let draw_rooms: Vec<i32> = match (if global { None } else { room_of_unit }, room) {
                            _ if name != known::DRAW_OBJECT => vec![],
                            (Some(r), _) => vec![r],
                            (None, Some(r)) if !c.room_change => vec![r],
                            _ => scope.rooms.clone(),
                        };
                        let in_rooms = |o: &i32| game.object_room.get(o).is_some_and(|r| draw_rooms.contains(r));
                        match &os {
                            Some(os) => {
                                for o in os {
                                    for k in kinds {
                                        out.push((Target::Exact(format!("{k}:{o}")), v.clone()));
                                    }
                                    if name == known::PICKUP_OBJECT {
                                        out.push((Target::Exact(format!("class:{o}:{CLASS_UNTOUCHABLE}")), None));
                                    }
                                }
                            }
                            None if name == known::DRAW_OBJECT => {
                                for o in game.object_room.keys().filter(|o| in_rooms(o)) {
                                    out.push((Target::Exact(format!("state:{o}")), v.clone()));
                                }
                            }
                            None => {
                                for k in kinds {
                                    out.push((Target::Prefix(format!("{k}:")), v.clone()));
                                }
                                if name == known::PICKUP_OBJECT {
                                    out.push((Target::Class(CLASS_UNTOUCHABLE), None));
                                }
                            }
                        }
                        if name == known::DRAW_OBJECT {
                            // it clears objects drawn at the same place (script_v5.cpp:1122-1126): state 0
                            let zero: BTreeSet<i64> = [0i64].into_iter().collect();
                            for o in game.object_room.keys().filter(|o| in_rooms(o)) {
                                out.push((Target::Exact(format!("state:{o}")), Some(zero.clone())));
                            }
                        }
                    } else if [known::VERB_OPS, known::SAVE_VERBS, known::RESTORE_VERBS].contains(&name) {
                        out.push((Target::Prefix("verb:".into()), None));
                        if name != known::VERB_OPS {
                            out.push((Target::Prefix("saved:".into()), None));
                        }
                    } else if let Some(n) = scope::cursor_op_var(game.syms.str(name)) {
                        out.push((Target::Exact(format!("var:{n}")), None));
                    } else if name == known::PRINT || name == known::PRINT_EGO {
                        out.push((Target::Exact(format!("var:{}", scope::VAR_KEYPRESS)), None));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// What a havoc record writes, before it is put on the path's own record: facts with their values (None: any value of
/// the domain), the slots it may start, stop or reach, and the units whose events it names.
#[derive(Debug, Default)]
pub struct HavocWrites {
    pub facts: BTreeMap<String, Option<BTreeSet<i64>>>,
    pub slots: BTreeSet<String>,
    pub event_units: BTreeSet<UnitKey>,
    /// the units whose slots may start or end (their lifecycle facts)
    pub slot_units: BTreeSet<UnitKey>,
    pub frozen_all: bool,
    pub esc_all: bool,
    pub closure_units: usize,
}

/// The facts and slots of the havoc record of a path cut at `cut` (havoc-cut.md, "The write set", "Values", "Slots").
pub fn writes(game: &Game, scope: &Scope, table: &FactTable, cut: &Cut, room: Option<i32>) -> HavocWrites {
    let c = closure(game, scope, table, cut, room);
    let units = c.units();
    let starts: FxHashMap<UnitId, Vec<u32>> = c.starts.iter().map(|(&u, s)| (u, s.iter().copied().collect())).collect();
    let w: Writes = scope::scope_writes_code(game, &units, Some(&starts));
    let mut fix = Writes::default();
    // F5's objpos: of the scope's rooms' movable objects comes from loading a room: only with a room change
    let fix_rooms: Vec<i32> = if c.room_change { scope.rooms.clone() } else { vec![] };
    scope::fix1_writes(game, &units, &fix_rooms, &scope.start, scope.switches, &mut fix);
    let mut engine: BTreeSet<String> = c.engine_writes.clone();
    let v = |n: i32| format!("var:{n}");
    engine.insert(v(scope::VAR_RESULT));
    if c.room_change {
        engine.insert("room".into());
        for n in [domains::VAR_ROOM, domains::VAR_ROOM_RESOURCE, domains::VAR_NEW_ROOM, domains::VAR_ROOM_FLAG] {
            engine.insert(v(n));
        }
        engine.insert(v(scope::VAR_CAMERA_MIN_X));
        engine.insert(v(scope::VAR_CAMERA_MAX_X));
        engine.insert(v(domains::VAR_WALKTO_OBJ));
    }
    if c.has_op(&[known::CUTSCENE, known::END_CUTSCENE]) {
        engine.insert("cutscene".into());
    }
    if c.has_op(&[known::CUTSCENE, known::END_CUTSCENE, known::BEGIN_OVERRIDE, known::END_OVERRIDE]) {
        engine.insert(v(domains::VAR_OVERRIDE));
    }
    if c.has_op(&[known::PRINT, known::PRINT_EGO, known::ACTOR_OPS]) || c.room_change {
        engine.insert(v(scope::VAR_TALK_ACTOR));
        engine.insert(v(scope::VAR_KEYPRESS));
    }
    let mut exact: FxHashMap<String, Vec<Option<BTreeSet<i64>>>> = FxHashMap::default();
    let mut wide: Vec<Site> = Vec::new();
    let room_no = room.and_then(|r| scope.rooms.iter().copied().find(|&x| game.resource(x) == Some(r)));
    for (t, v) in sites(game, scope, &c, room_no) {
        match t {
            Target::Exact(k) => exact.entry(k).or_default().push(v),
            t => wide.push((t, v)),
        }
    }
    let mut out = HavocWrites { closure_units: units.len(), ..Default::default() };
    let skip = |k: &str| {
        ["pos:", "escpos:", "args:", "at:", "esc:", "frozen:", "script:", "local:", "objscript:"].iter().any(|p| k.starts_with(p))
    };
    let keys: BTreeSet<&String> = table.doms.doms.keys().chain(table.extra.keys()).collect();
    for k in keys {
        if skip(k) || table.kind(k, scope) != Kind::Fact {
            continue;
        }
        let in_engine = engine.contains(k.as_str()) || fix.has(k);
        let mut hit: Vec<&Option<BTreeSet<i64>>> = wide.iter().filter(|s| s.0.hits(k)).map(|s| &s.1).collect();
        if let Some(v) = exact.get(k.as_str()) {
            hit.extend(v.iter());
        }
        // the kinds `sites` reads every writer of (each construct scope_writes reads them from): written where a site
        // hits; any other kind where scope_writes finds a writer
        let modeled = MODELED.iter().any(|p| k.starts_with(p));
        if !in_engine && (if modeled { hit.is_empty() } else { !w.has(k) }) {
            continue;
        }
        let vals = if in_engine || hit.is_empty() || hit.iter().any(|s| s.is_none()) {
            None
        } else {
            Some(hit.iter().flat_map(|s| s.as_ref().unwrap().iter().copied()).collect())
        };
        out.facts.insert(k.clone(), vals);
    }
    // ego is var:VAR_EGO's mirror (scumm-machine `put`)
    if let Some(e) = out.facts.get(&v(domains::VAR_EGO)).cloned() {
        if table.kind("ego", scope) == Kind::Fact {
            out.facts.insert("ego".into(), e);
        }
    }
    // the slots: the closure's units, the stopped ones, and with a room change every slot of room code
    let all_names = || table.points.values().flat_map(|m| m.keys().cloned()).collect::<BTreeSet<String>>();
    for &u in &units {
        let k = game.unit(u).key;
        out.slots.insert(Game::unit_name(k).to_string());
        out.event_units.insert(k);
        out.slot_units.insert(k);
    }
    for &k in &c.stops {
        out.slots.insert(Game::unit_name(k).to_string());
        out.slot_units.insert(k);
    }
    if c.stops_all {
        out.slots.extend(all_names());
        out.slot_units.extend(scope.units.iter().map(|&u| game.unit(u).key));
    }
    if c.room_change {
        for &u in &scope.units {
            let k = game.unit(u).key;
            let room_code = match k {
                UnitKey::Local(..) | UnitKey::Entry(_) | UnitKey::Exit(_) => true,
                UnitKey::Obj(o) => game.object_room.contains_key(&o),
                UnitKey::Global(_) => false,
            };
            if room_code {
                out.slots.insert(Game::unit_name(k).to_string());
                out.slot_units.insert(k);
            }
        }
    }
    out.frozen_all = c.has_op(&[known::FREEZE_SCRIPTS]);
    out.esc_all = c.has_op(&[known::CUTSCENE, known::END_CUTSCENE, known::BEGIN_OVERRIDE, known::END_OVERRIDE]);
    out
}

/// One fact's havoc effect: any value of its domain (`vals` None), or one of `vals` or no change, where no change
/// is the path's own write, its precondition's set, or any value of the domain, as cases on the current value.
/// None when the effect changes nothing.
fn effect(table: &FactTable, key: &str, vals: &Option<BTreeSet<i64>>, own: Option<&Eff>, pre: &Pre) -> Option<Eff> {
    let dom = table.domain(key)?;
    if dom.values.len() < 2 {
        return None;
    }
    let all = || Some(Eff::Guess(dom.values.clone()));
    let Some(vals) = vals else { return all() };
    let mut labs: BTreeSet<Label> = BTreeSet::new();
    for &v in vals {
        match table.label(key, v) {
            Some(l) => {
                labs.insert(l);
            }
            // a constant the domain lacks: any value, as a guess of the reader takes it
            None => return all(),
        }
    }
    let with = |u: Label| -> Labels {
        let mut s = labs.clone();
        s.insert(u);
        s.into_iter().collect()
    };
    let one = |s: Labels| if s.len() == 1 { Eff::Set(s[0]) } else { Eff::Guess(s) };
    match own {
        Some(Eff::Set(u)) => return Some(one(with(*u))),
        Some(_) => return all(),
        None => {}
    }
    let current: Labels = match pre.get(key) {
        Some(s) => s.clone(),
        None => dom.values.clone(),
    };
    if current.len() == 1 {
        let s = with(current[0]);
        return (s != current).then(|| one(s));
    }
    if current.iter().all(|u| labs.len() == 1 && labs.contains(u)) {
        return None;
    }
    let cases = current.iter().map(|&u| (vec![(key.to_string(), vec![u])], with(u))).collect();
    Some(Eff::Cases(cases))
}

impl Builder<'_> {
    /// The havoc record of a path the step limit ended (havoc-cut.md): the path's own record, or its precondition
    /// alone, then the havoc effects and events, ending "havoc".
    pub(crate) fn havoc_record(&mut self, kind: &str, label: &str, p: &PathOut, what: &What) -> Record {
        let cut = p.cut.clone().expect("a cut path");
        let base = PathOut { cut: None, ..p.clone() };
        let mut rec = match self.record(kind, label, &base, what) {
            Some(r) => r,
            None => {
                let mut pre = Pre::new();
                for (k, s) in &p.pre {
                    if let Some(dom) = self.table.domain(k) {
                        if *s != dom.values || (k.starts_with("args:") && self.table.reach.is_some()) {
                            pre.insert(k.clone(), s.clone());
                        }
                    }
                }
                let (pre, _) = self.positions(pre, Vec::new());
                Record {
                    kind: kind.into(),
                    label: label.into(),
                    pre,
                    effects: vec![],
                    checks: vec![],
                    end: None,
                    notes: vec![],
                    next_events: vec![],
                    source: what.clone(),
                    either: vec![],
                    engine_pre: vec![],
                    unknown: vec![],
                }
            }
        };
        // the room the path stands in: its own write, else its precondition's one value
        let room = match p.writes.get("room") {
            Some(crate::record::Val::Int(r)) => Some(*r),
            Some(_) => None,
            None => p.pre.get("room").filter(|s| s.len() == 1).and_then(|s| s[0].int()),
        };
        let res = room.and_then(|r| self.game.resource(r as i32));
        let hw = writes(self.game, &self.scope, &self.table, &cut, res);
        let mut eff: BTreeMap<String, Eff> = BTreeMap::new();
        for (k, vals) in &hw.facts {
            let own = rec.effects.iter().find(|e| &e.0 == k).map(|e| &e.1);
            if let Some(e) = effect(&self.table, k, vals, own, &rec.pre) {
                eff.insert(k.clone(), e);
            }
        }
        // the slots: any wait point or none; the list unknown (0); frozen or not; any Esc point; live or not
        let mut slots: BTreeSet<String> = hw.slots.clone();
        if hw.frozen_all || hw.esc_all {
            slots.extend(self.table.points.values().flat_map(|m| m.keys().cloned()));
        }
        for name in &slots {
            for k in [format!("pos:{name}"), format!("escpos:{name}"), format!("frozen:{name}")] {
                if k.starts_with("escpos:") && !hw.esc_all && !hw.slots.contains(name) {
                    continue;
                }
                if k.starts_with("frozen:") && (self.table.kind(&k, &self.scope) != Kind::Fact || (!hw.frozen_all && !hw.slots.contains(name))) {
                    continue;
                }
                if k.starts_with("pos:") && !hw.slots.contains(name) {
                    continue;
                }
                if let Some(e) = effect(&self.table, &k, &None, None, &rec.pre) {
                    eff.insert(k, e);
                }
            }
            if hw.slots.contains(name) {
                let a = format!("args:{name}");
                if rec.pre.get(&a).is_none_or(|s| s != &[Label::Int(0)]) {
                    eff.insert(a, Eff::Set(Label::Int(0)));
                }
            }
        }
        for &k in &hw.slot_units {
            if let Some(f) = self.game.lifecycle_key(k) {
                let f = f.to_string();
                if self.table.kind(&f, &self.scope) == Kind::Fact {
                    if let Some(e) = effect(&self.table, &f, &None, None, &rec.pre) {
                        eff.insert(f, e);
                    }
                }
            }
        }
        let n_eff = eff.len();
        for e in rec.effects.iter_mut() {
            if let Some(h) = eff.remove(&e.0) {
                e.1 = h;
            }
        }
        rec.effects.extend(eff);
        // the events: each wait point of each unit the code can run, with unknown locals (and its Esc points)
        let mut seen: BTreeSet<(String, UnitKey, i64, i64, bool)> =
            rec.next_events.iter().map(|e| (e.name.clone(), e.unit, e.w, e.sig, e.abort)).collect();
        let mut n_ev = 0usize;
        for &k in &hw.event_units {
            let Some(u) = self.game.id(k) else { continue };
            let name = Game::unit_name(k).to_string();
            let res = match k {
                UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
                UnitKey::Obj(o) => self.game.object_room.get(&o).and_then(|&r| self.game.resource(r)),
                UnitKey::Global(_) => None,
            };
            let resistant = match self.game.lifecycle_key(k) {
                Some(f) => self.game.resistant(f, res),
                None => Some(false),
            };
            let owned = match k {
                UnitKey::Obj(_) => Py::None,
                _ => Py::Bool(false),
            };
            let mut ws: Vec<(i64, bool)> = scope::stop_points(self.game, u)
                .into_iter()
                .map(|w| w as i64)
                .filter(|&w| self.table.is_point("at", &name, w))
                .map(|w| (w, false))
                .collect();
            if hw.esc_all {
                if let Some(es) = self.table.points.get("esc").and_then(|m| m.get(&name)) {
                    ws.extend(es.iter().map(|&w| (w, true)));
                }
            }
            for (w, abort) in ws {
                if !seen.insert((name.clone(), k, w, 0, abort)) {
                    continue;
                }
                n_ev += 1;
                rec.next_events.push(Event {
                    name: name.clone(),
                    unit: k,
                    w,
                    sig: 0,
                    locals: vec![],
                    args_known: false,
                    recursive: Py::None,
                    resistant: Py::opt_bool(resistant),
                    owned: owned.clone(),
                    abort,
                    override_: 0,
                    cut_data: vec![],
                });
            }
        }
        rec.end = Some("havoc".into());
        self.counts.inc(format!("havoc_cut: paths the step limit ended, made havoc records: {kind}"));
        self.counts.add("havoc_cut: havoc effects", n_eff as i64);
        self.counts.add("havoc_cut: havoc events (unknown locals)", n_ev as i64);
        crate::par::trace_havoc(label, hw.closure_units, n_eff, n_ev, &cut);
        rec
    }
}
