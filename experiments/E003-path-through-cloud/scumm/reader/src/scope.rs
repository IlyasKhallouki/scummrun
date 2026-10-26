//! reader.py's scope: what the scope's code can write (`Writes`, `scope_writes`, `entry_writes`), the verbs read
//! from the scripts (`made_verbs`, `walk_verbs`), a unit's wait points (`stop_points`), `Scope.of`, and the inventory
//! hook's summary (`Reader._hook_writes`), which needs the IR only.

use crate::domains::{self, fold, list_items, stmt_nodes, CallReader, Env, VarEnv};
use crate::facts::{unit_name, unit_py, unit_text, Facts};
use crate::py::{Counter, Py};
use rustc_hash::FxHashMap;
use scumm_ir::{known, Expr, ExprId, Game, RefKind, Stmt, UnitId, UnitKey, Yield};
use scumm_machine::Switches;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

pub const CLASS_UNTOUCHABLE: i64 = 32; // kObjectClassUntouchable, object.h:54
pub const NUM_ACTORS: i64 = 13; // _numActors for v5 (scumm.cpp:1726-1739)
pub const OF_OWNER_ROOM: i64 = 0x0F; // scumm.cpp:1745
pub const VAR_KEYPRESS: i32 = 0;
pub const VAR_CAMERA_MIN_X: i32 = 17;
pub const VAR_CAMERA_MAX_X: i32 = 18;
pub const VAR_TALK_ACTOR: i32 = 25;
pub const VAR_RESULT: i32 = 0;
pub const VAR_USERPUT: i32 = 53;
pub const VAR_CURSORSTATE: i32 = 52;

/// The variable each cursor op writes (reader.CURSOR_OPS, o5_cursorCommand, script_v5.cpp:917-938).
pub fn cursor_op_var(name: &str) -> Option<i32> {
    match name {
        "UserputOn" | "UserputOff" | "UserputSoftOn" | "UserputSoftOff" => Some(VAR_USERPUT),
        "CursorShow" | "CursorHide" | "CursorSoftOn" | "CursorSoftOff" => Some(VAR_CURSORSTATE),
        _ => None,
    }
}

/// reader.py `Writes`: the facts some code of the scope can write.
#[derive(Clone, Debug, Default)]
pub struct Writes {
    pub exact: BTreeSet<String>,
    pub wild: BTreeSet<String>,
    pub var_windows: Vec<(i32, String)>,
    pub classes: BTreeSet<i64>,
    pub counts: Counter,
}

impl Writes {
    pub fn has(&self, key: &str) -> bool {
        if self.exact.contains(key) {
            return true;
        }
        if self.wild.iter().any(|w| key.starts_with(w.as_str())) {
            return true;
        }
        if key.starts_with("class:") {
            let c: i64 = key.rsplit(':').next().unwrap().parse().unwrap();
            if self.classes.contains(&c) {
                return true;
            }
        }
        if let Some(n) = key.strip_prefix("var:") {
            let n: i32 = n.parse().unwrap();
            return self.var_windows.iter().any(|(b, _)| *b <= n && n < b + 16);
        }
        false
    }

    pub fn py(&self) -> Py {
        Py::dict(vec![
            ("exact", Py::Set(self.exact.iter().map(Py::str).collect())),
            ("wild", Py::Set(self.wild.iter().map(Py::str).collect())),
            (
                "var_windows",
                Py::List(self.var_windows.iter().map(|(b, c)| Py::Tuple(vec![Py::Int(*b as i64), Py::str(c)])).collect()),
            ),
            ("classes", Py::Set(self.classes.iter().map(|&c| Py::Int(c)).collect())),
            ("counts", self.counts.py()),
        ])
    }
}

/// The engine's own writes inside opcodes the scope runs (scope_writes' fixed part).
fn engine_writes() -> [String; 13] {
    [
        "room".into(),
        "cutscene".into(),
        "ego".into(),
        format!("var:{}", domains::VAR_ROOM),
        format!("var:{}", domains::VAR_ROOM_RESOURCE),
        format!("var:{}", domains::VAR_NEW_ROOM),
        format!("var:{}", domains::VAR_ROOM_FLAG),
        format!("var:{}", domains::VAR_WALKTO_OBJ),
        format!("var:{}", domains::VAR_OVERRIDE),
        format!("var:{VAR_CAMERA_MIN_X}"),
        format!("var:{VAR_CAMERA_MAX_X}"),
        format!("var:{VAR_TALK_ACTOR}"),
        format!("var:{VAR_RESULT}"),
    ]
}

/// reader.py `scope_writes`: the `Writes` of `units` (in order), their locals bound from their call sites; `starts`
/// reads a unit only from those instruction indices.
pub fn scope_writes(game: &Game, units: &[UnitId], starts: Option<&FxHashMap<UnitId, Vec<u32>>>) -> Writes {
    let ast = &game.ast;
    let settings = domains::settings_env();
    let mut caps = 0i64;
    let mut out = Writes::default();
    let code: FxHashMap<UnitId, Vec<u32>> = units
        .iter()
        .map(|&u| (u, domains::live(game, u, &settings, starts.and_then(|s| s.get(&u)).map(|v| v.as_slice()), &mut caps)))
        .collect();
    let rd = CallReader::new(game, units.to_vec(), code);
    let mut notes = Vec::new();
    let started = rd.hook_units(&mut out.counts, &mut notes, &mut caps);
    let (env, _) = rd.locals(&started, &mut out.counts, &mut notes, &mut caps);
    let objs = |e: ExprId, env: Option<&Env>, caps: &mut i64| fold(ast, e, env, None, caps);

    fn target(game: &Game, out: &mut Writes, e: ExprId, env: Option<&Env>, kind: &str, caps: &mut i64) {
        let ast = &game.ast;
        match ast.expr(e) {
            Expr::Var(n) => {
                out.exact.insert(format!("var:{n}"));
            }
            Expr::Bit(n) => {
                out.exact.insert(format!("bit:{n}"));
            }
            Expr::Index { kind: k @ (RefKind::Var | RefKind::Bit), base, offset } => match fold(ast, offset, env, None, caps) {
                None => {
                    if k == RefKind::Var {
                        out.var_windows.push((base, kind.to_string()));
                    } else {
                        out.wild.insert("bit:".into());
                    }
                    out.counts.inc(format!("indexed {} writes whose index does not fold", k.as_str()));
                }
                Some(s) => {
                    for v in s {
                        out.exact.insert(format!("{}:{}", k.as_str(), base as i64 + v));
                    }
                }
            },
            _ => {}
        }
    }

    for &u in units {
        let key = game.unit(u).key;
        let e = env.get(&key);
        let ins = game.ins(u);
        for &i in &rd.code[&u] {
            let Some(st) = ins[i as usize].st else { continue };
            let cite = game.cite(u, i);
            for y in stmt_nodes(ast, &st) {
                if let Expr::Embed(a) = ast.expr(y) {
                    target(game, &mut out, a.lhs, e, &cite, &mut caps);
                }
            }
            match st {
                Stmt::Assign(a) => target(game, &mut out, a.lhs, e, &cite, &mut caps),
                Stmt::Op { name, args, .. } => {
                    let a = ast.kids(args);
                    let sname = game.syms.str(name);
                    if name == known::SET_VAR_RANGE {
                        let first = a[0];
                        match ast.expr(first) {
                            Expr::Var(n) | Expr::Bit(n) => {
                                let kind = if matches!(ast.expr(first), Expr::Var(_)) { "var" } else { "bit" };
                                for k in 0..list_items(ast, a[2]).len() as i32 {
                                    out.exact.insert(format!("{kind}:{}", n + k));
                                }
                            }
                            Expr::Local(_) => {}
                            _ => target(game, &mut out, first, e, &cite, &mut caps),
                        }
                    } else if [known::SET_STATE, known::DRAW_OBJECT, known::PICKUP_OBJECT, known::SET_OWNER_OF, known::SET_CLASS]
                        .contains(&name)
                    {
                        let os = objs(a[0], e, &mut caps);
                        if name == known::SET_CLASS {
                            let cls: Vec<Option<Vec<i64>>> =
                                list_items(ast, a[1]).iter().map(|&c| fold(ast, c, e, None, &mut caps)).collect();
                            let cs: Option<BTreeSet<i64>> = if cls.iter().any(|c| c.is_none()) {
                                None
                            } else {
                                Some(cls.iter().flat_map(|c| c.as_ref().unwrap().iter().map(|v| v & 0x7F)).collect())
                            };
                            let wild = cs.as_ref().is_none_or(|cs| cs.contains(&0));
                            match os {
                                None => {
                                    out.counts.inc("setClass on an object that does not fold");
                                    if wild {
                                        out.wild.insert("class:".into());
                                    } else {
                                        out.classes.extend(cs.unwrap());
                                    }
                                }
                                Some(os) => {
                                    for o in os {
                                        if wild {
                                            out.wild.insert(format!("class:{o}:"));
                                        } else {
                                            for c in cs.as_ref().unwrap() {
                                                out.exact.insert(format!("class:{o}:{c}"));
                                            }
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                        let kinds: &[&str] = if name == known::SET_OWNER_OF {
                            &["owner"]
                        } else if name == known::PICKUP_OBJECT {
                            &["owner", "state"]
                        } else {
                            &["state"]
                        };
                        match os {
                            None => {
                                out.counts.inc(format!("{sname} on an object that does not fold"));
                                for k in kinds {
                                    out.wild.insert(format!("{k}:"));
                                }
                                if name == known::PICKUP_OBJECT {
                                    out.classes.insert(CLASS_UNTOUCHABLE);
                                }
                            }
                            Some(os) => {
                                for o in os {
                                    for k in kinds {
                                        out.exact.insert(format!("{k}:{o}"));
                                    }
                                    if name == known::PICKUP_OBJECT {
                                        out.exact.insert(format!("class:{o}:{CLASS_UNTOUCHABLE}"));
                                    }
                                }
                            }
                        }
                        if name == known::DRAW_OBJECT {
                            // it clears objects drawn at the same place (script_v5.cpp:1122-1126)
                            if let Some(r) = game.unit(u).room {
                                for (o, rr) in &game.object_room {
                                    if *rr == r {
                                        out.exact.insert(format!("state:{o}"));
                                    }
                                }
                            }
                        }
                    } else if [known::VERB_OPS, known::SAVE_VERBS, known::RESTORE_VERBS].contains(&name) {
                        let vs = objs(a[0], e, &mut caps);
                        match vs {
                            Some(vs) if name == known::VERB_OPS => {
                                for v in vs {
                                    out.exact.insert(format!("verb:{v}"));
                                }
                            }
                            _ => {
                                out.wild.insert("verb:".into());
                            }
                        }
                    } else if let Some(n) = cursor_op_var(sname) {
                        out.exact.insert(format!("var:{n}"));
                    } else if name == known::PRINT || name == known::PRINT_EGO {
                        out.exact.insert(format!("var:{VAR_KEYPRESS}"));
                    }
                }
                _ => {}
            }
        }
    }
    out.exact.extend(engine_writes());
    out.wild.insert("esc:".into());
    out.wild.insert("frozen:".into());
    for &u in units {
        match game.unit(u).key {
            UnitKey::Global(n) => {
                out.exact.insert(format!("script:{n}"));
            }
            UnitKey::Local(_, n) => {
                out.exact.insert(format!("local:{n}"));
            }
            UnitKey::Obj(o) => {
                out.exact.insert(format!("objscript:{o}"));
            }
            _ => {}
        }
        out.wild.insert(format!("at:{}:", unit_name(game.unit(u).key)));
    }
    out
}

/// The engine's part of `scope_writes` (`scope_writes(game, {}).exact`).
pub fn engine_exact() -> BTreeSet<String> {
    engine_writes().into_iter().collect()
}

static ENTRY_WRITES: Mutex<BTreeMap<i32, Vec<String>>> = Mutex::new(BTreeMap::new());

/// reader.py `entry_writes`: the facts the `verb` entries of every object of the game write by a constant number.
/// Cached per verb (one game per process, as reader.py's cache keys on id(game)).
pub fn entry_writes(game: &Game, verb: i32) -> Vec<String> {
    if let Some(v) = ENTRY_WRITES.lock().unwrap().get(&verb) {
        return v.clone();
    }
    let engine = engine_exact();
    let mut keys = BTreeSet::new();
    for u in game.ids() {
        let unit = game.unit(u);
        if let (UnitKey::Obj(_), Some(idx)) = (unit.key, unit.entry_of(verb)) {
            let starts: FxHashMap<UnitId, Vec<u32>> = [(u, vec![idx])].into_iter().collect();
            let w = scope_writes(game, &[u], Some(&starts));
            for x in w.exact.difference(&engine) {
                if ["var:", "bit:", "verb:", "owner:", "state:", "class:"].iter().any(|p| x.starts_with(p)) {
                    keys.insert(x.clone());
                }
            }
        }
    }
    let out: Vec<String> = keys.into_iter().collect();
    ENTRY_WRITES.lock().unwrap().insert(verb, out.clone());
    out
}

/// Reader._hook_writes: what the inventory hook `n` (a global script) can write by a constant number, the 16
/// variables after an indexed write's base, and what the object entries it starts write.
pub fn hook_writes(game: &Game, n: i32) -> Vec<String> {
    let u = game.id(UnitKey::Global(n)).expect("the hook's unit");
    let w = scope_writes(game, &[u], None);
    let engine = engine_exact();
    let mut keys: BTreeSet<String> = w
        .exact
        .difference(&engine)
        .filter(|k| ["var:", "bit:", "verb:"].iter().any(|p| k.starts_with(p)))
        .cloned()
        .collect();
    for (b, _) in &w.var_windows {
        for i in 0..16 {
            keys.insert(format!("var:{}", b + i));
        }
    }
    for x in game.ins(u) {
        if let Some(Stmt::Start { name, script, .. }) = x.st {
            if name == known::START_OBJECT {
                if let Some(v) = game.ast.constant(script) {
                    keys.extend(entry_writes(game, v));
                }
            }
        }
    }
    keys.into_iter().collect()
}

/// reader.py `made_verbs`: {verb: the VerbOps sub-op names any script gives it} for every verb some script creates
/// with New (a constant number).
pub fn made_verbs(game: &Game) -> BTreeMap<i32, BTreeSet<String>> {
    let ast = &game.ast;
    let mut made: BTreeMap<i32, BTreeSet<String>> = BTreeMap::new();
    for x in &game.ins {
        if let Some(Stmt::Op { name: known::VERB_OPS, args, subops, .. }) = x.st {
            if let Some(v) = ast.constant(ast.kids(args)[0]) {
                made.entry(v).or_default().extend(ast.subs(subops).iter().map(|s| game.syms.str(s.name).to_string()));
            }
        }
    }
    made.retain(|_, n| n.contains("New"));
    made
}

/// The verbs some object's event table names.
pub fn event_verbs(game: &Game) -> BTreeMap<i32, i64> {
    let mut evs: BTreeMap<i32, i64> = BTreeMap::new();
    for u in &game.units {
        if let UnitKey::Obj(_) = u.key {
            for &(v, _) in &u.entries {
                *evs.entry(v).or_default() += 1;
            }
        }
    }
    evs
}

/// reader.py `walk_verbs`: verbs some script creates with New that no script turns On and some event table names.
pub fn walk_verbs(game: &Game) -> Vec<i32> {
    let evs = event_verbs(game);
    made_verbs(game).into_iter().filter(|(v, n)| !n.contains("On") && evs.contains_key(v)).map(|(v, _)| v).collect()
}

/// reader.py `stop_points`: the offsets where a slot of unit `u` can wait.
pub fn stop_points(game: &Game, u: UnitId) -> Vec<i32> {
    let ins = game.ins(u);
    let mut out = BTreeSet::new();
    for x in ins {
        if x.yields == Some(Yield::Wait) {
            out.insert(x.offset);
        }
        if let Some(nx) = x.next {
            if (nx as usize) < ins.len() && !matches!(x.st, Some(Stmt::Assign(_)) | Some(Stmt::Jump { .. })) {
                out.insert(ins[nx as usize].offset);
            }
        }
    }
    out.into_iter().collect()
}

/// model.py `lifecycle_key`: the fact of the unit's own slot (entry and exit code have none).
pub fn lifecycle_key(game: &Game, k: UnitKey) -> Option<String> {
    game.lifecycle_key(k).map(|f| f.to_string())
}

/// reader.py `Scope`.
#[derive(Clone, Debug)]
pub struct Scope {
    pub rooms: Vec<i32>,
    /// the scope's units in reader.py's dict order: the rooms' units in load order, then the global scripts in the
    /// order the walk reached them
    pub units: Vec<UnitId>,
    pub unit_keys: rustc_hash::FxHashSet<UnitKey>,
    pub objects: Vec<i64>,
    pub verbs: Vec<i64>,
    pub dialog_verbs: Vec<i64>,
    pub walk_verb: Option<i64>,
    pub start: Facts,
    pub writes: Writes,
    pub universe: Vec<String>,
    pub counts: Counter,
    /// fix round 1's switches the reading runs with (machine.rs `Switches`; reader.py has none): the Reader's machine
    /// takes them, and `of_with` adds the writes they make
    pub switches: Switches,
    /// fix round 1, F1 (switch input_script): the input scripts, mouse loops, sentence-building variables, slot
    /// verbs and cells (inputs.rs); None with the switch off
    pub inputs: Option<crate::inputs::InputScope>,
}

impl Scope {
    /// `Scope.of(game, rooms, start)`.
    pub fn of(game: &Game, rooms: &[i32], start: &Facts) -> Scope {
        Scope::of_with(game, rooms, start, Switches::default())
    }

    /// `Scope.of` with fix round 1's switches (fix-round1.md): with them off, `of`.
    pub fn of_with(game: &Game, rooms: &[i32], start: &Facts, switches: Switches) -> Scope {
        let ast = &game.ast;
        let mut rooms: Vec<i32> = rooms.to_vec();
        rooms.sort_unstable();
        let in_rooms = |r: i32| rooms.contains(&r);
        let walks = walk_verbs(game);
        let walk_verb = (walks.len() == 1).then(|| walks[0] as i64);
        let mut units: Vec<UnitId> = Vec::new();
        for u in game.ids() {
            let k = game.unit(u).key;
            let inside = match k {
                UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => in_rooms(r),
                UnitKey::Obj(o) => game.object_room.get(&o).is_some_and(|&r| in_rooms(r)),
                UnitKey::Global(_) => false,
            };
            if inside {
                units.push(u);
            }
        }
        let mut hooks: BTreeSet<i64> = (domains::HOOKS).map(|n| start.get0(&format!("var:{n}"))).collect();
        hooks.remove(&0);
        let mut todo: Vec<UnitKey> = hooks
            .iter()
            .filter(|&&n| n < game.first_local as i64 && game.id(UnitKey::Global(n as i32)).is_some())
            .map(|&n| UnitKey::Global(n as i32))
            .collect();
        for (k, v) in start.iter() {
            if let Some(n) = k.strip_prefix("script:") {
                if v == 1 {
                    todo.push(UnitKey::Global(n.parse().unwrap()));
                }
            }
        }
        todo.extend(units.iter().map(|&u| game.unit(u).key));
        let mut seen: rustc_hash::FxHashSet<UnitKey> = Default::default();
        let mut counts = Counter::default();
        while let Some(k) = todo.pop() {
            let Some(u) = game.id(k) else { continue };
            if !seen.insert(k) {
                continue;
            }
            if let UnitKey::Global(_) = k {
                if !units.contains(&u) {
                    units.push(u);
                }
            }
            for x in game.ins(u) {
                match x.st {
                    Some(Stmt::Start { name, script, .. }) if name != known::START_OBJECT => match ast.constant(script) {
                        Some(n) => {
                            if 0 < n && n < game.first_local {
                                todo.push(UnitKey::Global(n));
                            }
                        }
                        None => counts.inc("starts with a script number that does not fold (callee not in the scope)"),
                    },
                    Some(Stmt::Assign(a)) => {
                        if let Expr::Var(v) = ast.expr(a.lhs) {
                            if domains::HOOKS.contains(&v) {
                                if let Some(n) = ast.constant(a.rhs) {
                                    if 0 < n && n < game.first_local {
                                        todo.push(UnitKey::Global(n));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        counts.set("walk verb candidates (walk_verbs; one is taken only when there is one)", walks.len() as i64);
        let mut objects: Vec<i64> =
            game.object_room.iter().filter(|(_, &r)| in_rooms(r)).map(|(&o, _)| o as i64).collect();
        objects.sort_unstable();
        let evs = event_verbs(game);
        let made = made_verbs(game);
        let verbs: Vec<i64> = made
            .iter()
            .filter(|(v, n)| n.contains("Text") && n.contains("On") && evs.contains_key(v) && Some(**v as i64) != walk_verb)
            .map(|(&v, _)| v as i64)
            .collect();
        let mut made_here: BTreeSet<i32> = BTreeSet::new();
        for &u in &units {
            for x in game.ins(u) {
                if let Some(Stmt::Op { name: known::VERB_OPS, args, subops, .. }) = x.st {
                    if let Some(v) = ast.constant(ast.kids(args)[0]) {
                        if ast.subs(subops).iter().any(|s| s.name == known::NEW) {
                            made_here.insert(v);
                        }
                    }
                }
            }
        }
        let dialog_verbs: Vec<i64> = made
            .iter()
            .filter(|(v, n)| {
                !evs.contains_key(v) && Some(**v as i64) != walk_verb && !n.contains("SetToObject") && made_here.contains(v)
            })
            .map(|(&v, _)| v as i64)
            .collect();
        let mut writes = scope_writes(game, &units, None);
        fix1_writes(game, &units, &rooms, start, switches, &mut writes);
        let inputs = switches.input_script.then(|| {
            let shown: Vec<i64> = verbs.iter().chain(dialog_verbs.iter()).copied().collect();
            crate::inputs::writes(&mut writes);
            if switches.sentence_queue {
                crate::inputs::queue_writes(&mut writes);
            }
            crate::inputs::input_scope(game, &units, &rooms, start, &shown, switches.draw_clears)
        });
        let mut uni: BTreeSet<String> = BTreeSet::new();
        for &u in &units {
            let k = game.unit(u).key;
            let name = unit_name(k);
            for w in stop_points(game, u) {
                uni.insert(format!("at:{name}:{w}"));
            }
            let ins = game.ins(u);
            for x in ins {
                if let Some(e) = x.esc {
                    if (e as usize) < ins.len() {
                        uni.insert(format!("esc:{name}:{}", ins[e as usize].offset));
                    }
                }
            }
            if let Some(lk) = lifecycle_key(game, k) {
                uni.insert(lk);
            }
        }
        for (k, v) in start.iter() {
            if ["at:", "esc:", "script:", "local:", "objscript:"].iter().any(|p| k.starts_with(p)) && v == 1 {
                uni.insert(k.clone());
            }
        }
        let unit_keys = units.iter().map(|&u| game.unit(u).key).collect();
        Scope {
            rooms,
            units,
            unit_keys,
            objects,
            verbs,
            dialog_verbs,
            walk_verb,
            start: start.clone(),
            writes,
            universe: uni.into_iter().collect(),
            counts,
            switches,
            inputs,
        }
    }

    /// `Scope.other_b`: B = "other": the lowest object of A's room with no number among `consts`.
    pub fn other_b(&self, game: &Game, a: i64, consts: &BTreeSet<i64>) -> Option<i64> {
        let room = game.object_room.get(&(a as i32)).copied();
        self.objects.iter().copied().find(|&o| {
            game.object_room.get(&(o as i32)).copied() == room && o != a && !consts.contains(&o)
        })
    }

    pub fn has_unit(&self, k: UnitKey) -> bool {
        self.unit_keys.contains(&k)
    }

    /// The oracle's encoding of a scope (reader_port_oracle.py `scope_dict`).
    pub fn py(&self, game: &Game) -> Py {
        let mut keys: Vec<UnitKey> = self.units.iter().map(|&u| game.unit(u).key).collect();
        keys.sort_by_key(|&k| unit_text(k));
        Py::dict(vec![
            ("rooms", Py::List(self.rooms.iter().map(|&r| Py::Int(r as i64)).collect())),
            ("units", Py::List(keys.into_iter().map(unit_py).collect())),
            ("objects", Py::List(self.objects.iter().map(|&o| Py::Int(o)).collect())),
            ("verbs", Py::List(self.verbs.iter().map(|&o| Py::Int(o)).collect())),
            ("dialog_verbs", Py::List(self.dialog_verbs.iter().map(|&o| Py::Int(o)).collect())),
            ("walk_verb", Py::opt_int(self.walk_verb)),
            ("universe", Py::strs(&self.universe)),
            ("counts", self.counts.py()),
            ("writes", self.writes.py()),
        ])
    }
}

/// Fix round 1's writes (fix-round1.md), added to `scope_writes`' with each switch on.
///
/// F5, draw_clears: loading a scope room puts its movable objects back (`objpos:O` = 0, room.cpp:195), and a
/// drawObject with setXY moves one, so `objpos:O` of every movable object of the scope's rooms. scope_writes already
/// has the clearing's `state:` writes of a room's own code (every object of its room); a global script's drawObject
/// can clear any object of the room it runs in, a scope room: every object of the scope's rooms.
///
/// F9, actor_rooms: `actorroom:A` of each actor a write of `actor_room_writes` can reach. An actor operand is worked
/// out here when it is a constant or a variable the scope does not write (its start value); any other reaches every
/// actor. The FactTable works the operands out again with the reading's domains (`table::actor_room_domains`).
pub fn fix1_writes(game: &Game, units: &[UnitId], rooms: &[i32], start: &Facts, sw: Switches, out: &mut Writes) {
    if sw.draw_clears {
        for &r in rooms {
            for &o in game.room_objects.get(&r).map(|v| v.as_slice()).unwrap_or(&[]) {
                if game.movable.contains_key(&o) {
                    out.exact.insert(format!("objpos:{o}"));
                }
            }
        }
        let global_draws = units.iter().any(|&u| {
            matches!(game.unit(u).key, UnitKey::Global(_))
                && game.ins(u).iter().any(|x| matches!(x.st, Some(Stmt::Op { name: known::DRAW_OBJECT, .. })))
        });
        if global_draws {
            for (o, r) in &game.object_room {
                if rooms.contains(r) {
                    out.exact.insert(format!("state:{o}"));
                }
            }
        }
    }
    if sw.actor_rooms {
        let var = |n: i32| -> Option<BTreeSet<i64>> {
            let k = format!("var:{n}");
            (!out.has(&k)).then(|| [start.get0(&k)].into_iter().collect())
        };
        let mut actors: BTreeSet<i64> = BTreeSet::new();
        for (who, _) in actor_room_writes(game, units, &var) {
            actors.extend(who.unwrap_or_else(|| (0..NUM_ACTORS).collect()));
        }
        for a in actors {
            out.exact.insert(format!("actorroom:{a}"));
        }
    }
}

/// F9 (switch actor_rooms): every write of an actor's room (Actor::_room, a byte) in `units`: o5_putActorInRoom's
/// (script_v5.cpp:2160-2196) and o5_loadRoomWithEgo's, the ego's (1857-1866); no other v5 opcode changes it
/// (actorOps' Init is initActor(0), which keeps the room: actor.cpp:160-192). Each as (the actors it can write, None:
/// any; the rooms it can write, as bytes, None: not worked out). An operand is worked out when it is a constant or
/// a variable whose values `var` gives (VAR_EGO for loadRoomWithEgo's actor). An actor number that is no actor is
/// left out: the engine stops with derefActor's error() (actor.cpp:2281-2293).
pub fn actor_room_writes(
    game: &Game,
    units: &[UnitId],
    var: &dyn Fn(i32) -> Option<BTreeSet<i64>>,
) -> Vec<(Option<BTreeSet<i64>>, Option<BTreeSet<i64>>)> {
    let ast = &game.ast;
    let vals = |e: ExprId| -> Option<BTreeSet<i64>> {
        match ast.expr(e) {
            Expr::Const(c) => Some([c as i64].into_iter().collect()),
            Expr::Var(n) => var(n),
            _ => None,
        }
    };
    let mut out = Vec::new();
    for &u in units {
        for x in game.ins(u) {
            let Some(Stmt::Op { name, args, .. }) = x.st else { continue };
            let a = ast.kids(args);
            let who = if name == known::PUT_ACTOR_IN_ROOM {
                vals(a[0])
            } else if name == known::LOAD_ROOM_WITH_EGO {
                var(domains::VAR_EGO)
            } else {
                continue;
            };
            let who = who.map(|s| s.into_iter().filter(|a| (0..NUM_ACTORS).contains(a)).collect());
            let rooms = vals(a[1]).map(|s| s.into_iter().map(|r| r & 0xFF).collect());
            out.push((who, rooms));
        }
    }
    out
}

/// i16 wrap (model.py `i16`).
pub fn i16w(v: i64) -> i64 {
    ((v + 0x8000) & 0xFFFF) - 0x8000
}

/// reader.py `_live_bound`: domains.live from the unit's start, with the locals `env` bound too (a jump whose
/// condition the settings and those locals decide goes one way, compared as int16 as the machine compares).
pub fn live_bound(game: &Game, u: UnitId, env: &Env) -> Vec<u32> {
    let ast = &game.ast;
    let settings: VarEnv = domains::settings_env();
    let ins = game.ins(u);
    let n = ins.len() as u32;
    let mut caps = 0i64;
    let mut seen = vec![false; n as usize];
    let mut todo: Vec<u32> = vec![0];
    while let Some(i) = todo.pop() {
        if i >= n || seen[i as usize] {
            continue;
        }
        seen[i as usize] = true;
        let x = &ins[i as usize];
        if let Some(Stmt::Jump { cond: Some(c), .. }) = x.st {
            let d: Option<bool> = match ast.expr(c) {
                Expr::Cmp { op, a, b } => {
                    let sa = fold(ast, a, Some(env), Some(&settings), &mut caps);
                    let sb = fold(ast, b, Some(env), Some(&settings), &mut caps);
                    match (sa, sb) {
                        (Some(sa), Some(sb)) => {
                            let op = crate::domain::Cmp::of(op);
                            let r: BTreeSet<bool> =
                                sa.iter().flat_map(|&y| sb.iter().map(move |&z| op.eval(i16w(y), i16w(z)))).collect();
                            (r.len() == 1).then(|| *r.iter().next().unwrap())
                        }
                        _ => None,
                    }
                }
                Expr::Truth(a) | Expr::Not(a) => {
                    let truth = matches!(ast.expr(c), Expr::Truth(_));
                    fold(ast, a, Some(env), Some(&settings), &mut caps).and_then(|s| {
                        let r: BTreeSet<bool> = s.iter().map(|&v| (v != 0) == truth).collect();
                        (r.len() == 1).then(|| *r.iter().next().unwrap())
                    })
                }
                _ => None,
            };
            if let Some(d) = d {
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
