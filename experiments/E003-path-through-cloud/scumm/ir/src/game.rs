//! model.py `Game`: the units, plus what the engine reads from the game data that the IR does not carry
//! (each object's room and parent, the offset of its verb block, each room's width, the pseudo-room
//! map), the F flags each script's slots can have, and every static analysis, computed once.

use crate::analysis::{self, Analyses, Ctx, Loop, Reason, WaitPoint, Write};
use crate::ast::*;
use crate::facts::{Fact, Slot};
use crate::places::{self, ObjBox};
use crate::read::{self, by_int_key, Script};
use crate::sym::known;
use rustc_hash::FxHashMap;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

/// The F (freeze resistant) flags a script's slots can be started with (model.py's set of bools).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Resist {
    pub without_f: bool,
    pub with_f: bool,
}

impl Resist {
    fn from_bits(b: u8) -> Self {
        Resist { without_f: b & 1 != 0, with_f: b & 2 != 0 }
    }
    /// The flags as a sorted list (Python's `sorted(set)`).
    pub fn sorted(self) -> Vec<bool> {
        [(self.without_f, false), (self.with_f, true)].into_iter().filter(|p| p.0).map(|p| p.1).collect()
    }
}

/// A start whose script number is not a constant: (cite, its F flag, or None for a chainScript).
pub type WildStart = (String, Option<bool>);

/// The fields of model.py's `Game` besides the units, as its constructor takes them.
#[derive(Clone, Debug, Default)]
pub struct GameData {
    pub first_local: i32,
    pub local_rooms: BTreeMap<i32, Vec<i32>>,
    pub object_room: FxHashMap<i32, i32>,
    pub object_parent: FxHashMap<i32, i32>,
    pub verb_offset: FxHashMap<i32, i32>,
    pub room_width: FxHashMap<i32, i32>,
    pub pseudo: FxHashMap<i32, i32>,
    pub pseudo_conflicts: Vec<(String, String)>,
    pub resistance: FxHashMap<UnitKey, Resist>,
    pub wild_starts: Vec<(String, Option<bool>)>,
    pub object_box: FxHashMap<i32, ObjBox>,
    pub movable: FxHashMap<i32, Vec<(i32, i32)>>,
}

/// model.py `Game`, with every analysis computed for both settings of switch const_polls.
#[derive(Clone, Debug)]
pub struct Game {
    pub script: Script,
    /// local scripts are numbered from _numGlobalScripts (script.cpp:70, 78)
    pub first_local: i32,
    /// local script number -> the rooms whose resource has it, sorted
    pub local_rooms: BTreeMap<i32, Vec<i32>>,
    /// object -> the room resource it is defined in
    pub object_room: FxHashMap<i32, i32>,
    pub object_parent: FxHashMap<i32, i32>,
    /// object -> offset of its VERB block in its OBCD (getVerbEntrypoint's verboffs)
    pub verb_offset: FxHashMap<i32, i32>,
    /// room -> width (RMHD)
    pub room_width: FxHashMap<i32, i32>,
    /// pseudo room (>= 0x80) -> room resource
    pub pseudo: FxHashMap<i32, i32>,
    /// (cite, why) of PseudoRoom mappings not kept
    pub pseudo_conflicts: Vec<(String, String)>,
    /// unit -> F flags it is started with (a key with no flags is kept, as model.py's defaultdict does)
    pub resistance: FxHashMap<UnitKey, Resist>,
    /// (cite, F flag or None for a chainScript) of starts whose script number is not a constant
    pub wild_starts: Vec<(String, Option<bool>)>,
    /// model.py `script_writes`, by variable in first-write order
    pub script_writes: Vec<(i32, Vec<Write>)>,
    /// model.py `Game.nonfacts`
    pub nonfacts: FxHashMap<i32, Reason>,
    /// [const_polls off, const_polls on]
    pub analyses: [Analyses; 2],
    /// per point (`Script::point`): whether a jump goes there (model.py `join_points`)
    pub joins: Vec<bool>,
    /// (unit, head) of every loop switch const_polls reclassifies
    pub const_poll_loops: Vec<(UnitId, u32)>,
    /// fix round 1, F5 (switch draw_clears; not model.py's): object -> its CDHD box (places.rs)
    pub object_box: FxHashMap<i32, ObjBox>,
    /// F5: room resource -> its objects with a box, sorted
    pub room_objects: FxHashMap<i32, Vec<i32>>,
    /// F5: object -> its places, when a drawObject with setXY can move it (`places::movable`); `objpos:O` indexes it
    pub movable: FxHashMap<i32, Vec<(i32, i32)>>,
}

impl std::ops::Deref for Game {
    type Target = Script;
    fn deref(&self) -> &Script {
        &self.script
    }
}

/// The repository root: `FAST_MI_ROOT`, else four levels above this crate (scumm/ir in
/// experiments/E003-path-through-cloud).
pub fn default_root() -> PathBuf {
    match std::env::var_os("FAST_MI_ROOT") {
        Some(r) => PathBuf::from(r),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..").canonicalize().expect("repository root"),
    }
}

impl Game {
    /// model.py `Game.load`: the units of data/scripts-dos-cd under `root`, with the raw blocks under
    /// `root`, and every analysis.
    pub fn load(root: &Path) -> io::Result<Game> {
        let scripts = root.join("data").join("scripts-dos-cd");
        let meta: Value = serde_json::from_slice(&std::fs::read(scripts.join("index.json"))?)?;
        let script = read::load_with(&scripts, Some(root), &meta)?;
        Game::from_script(script, root, &meta)
    }

    /// The rest of model.py `Game.load`, over units already read.
    pub fn from_script(script: Script, root: &Path, meta: &Value) -> io::Result<Game> {
        let mut local_rooms: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
        for u in &script.units {
            if let UnitKey::Local(r, n) = u.key {
                local_rooms.entry(n).or_default().push(r);
            }
        }
        for rs in local_rooms.values_mut() {
            rs.sort_unstable();
            rs.dedup();
        }
        let mut room_width = FxHashMap::default();
        for (r, room) in meta["rooms"].as_object().expect("rooms") {
            let path = root.join(room["lflf"].as_str().expect("lflf")).join("ROOM").join("RMHD");
            if path.exists() {
                // after the 8-byte block header, RMHD holds width, height, objects (room.cpp:343)
                let data = std::fs::read(path)?;
                room_width.insert(r.parse().expect("room"), crate::py::from_le(crate::py::slice(&data, 8, 10), false) as i32);
            }
        }
        let (pseudo, pseudo_conflicts) = pseudo_rooms(&script);
        let mut object_room = FxHashMap::default();
        let mut object_parent = FxHashMap::default();
        let mut verb_offset = FxHashMap::default();
        for (o, ob) in by_int_key(&meta["objects"]) {
            object_room.insert(o, ob["room"].as_i64().expect("object room") as i32);
            let parent = match ob.get("parent") {
                None => 0,
                Some(p) => p.as_i64().expect("object parent is an int") as i32,
            };
            object_parent.insert(o, parent);
            if let Some(v) = ob.get("verb_block_offset_in_obcd").and_then(|v| v.as_i64()) {
                verb_offset.insert(o, v as i32);
            }
        }
        let first_local = local_rooms.keys().next().copied().unwrap_or(1 << 16);
        let object_box = places::read_boxes(root, meta)?;
        let movable = places::movable(&script, &object_room, &object_box, &places::room_objects(&object_room, &object_box));
        let data = GameData {
            first_local,
            local_rooms,
            object_room,
            object_parent,
            verb_offset,
            room_width,
            pseudo,
            pseudo_conflicts,
            object_box,
            movable,
            ..Default::default()
        };
        let mut g = Game::new(script, data);
        (g.resistance, g.wild_starts) = g.compute_resistance();
        Ok(g)
    }

    /// model.py's `Game(...)` constructor: the units and the data as given (a test builds them by
    /// hand, as test_model.py's `game`); resistance is taken as given, as Python's constructor does
    /// (only `Game.load` works it out, `compute_resistance`). Every analysis is computed here, so add
    /// every unit to the script first.
    pub fn new(script: Script, data: GameData) -> Game {
        let script_writes = analysis::script_writes(&script);
        let nonfacts = analysis::non_fact_vars(&script, &script_writes);
        let room_objects = places::room_objects(&data.object_room, &data.object_box);
        let mut g = Game {
            script,
            first_local: data.first_local,
            local_rooms: data.local_rooms,
            object_room: data.object_room,
            object_parent: data.object_parent,
            verb_offset: data.verb_offset,
            room_width: data.room_width,
            pseudo: data.pseudo,
            pseudo_conflicts: data.pseudo_conflicts,
            resistance: data.resistance,
            wild_starts: data.wild_starts,
            script_writes,
            nonfacts,
            analyses: Default::default(),
            joins: Vec::new(),
            const_poll_loops: Vec::new(),
            room_objects,
            object_box: data.object_box,
            movable: data.movable,
        };
        let mut joins = vec![false; g.points()];
        for u in g.ids() {
            for x in g.ins(u) {
                if let Some(t) = x.target {
                    joins[g.point(u, t)] = true;
                }
            }
        }
        g.joins = joins;
        let cx = Ctx { sc: &g.script, first_local: g.first_local, nonfacts: &g.nonfacts };
        let analyses = [analysis::analyse(&cx, false), analysis::analyse(&cx, true)];
        g.analyses = analyses;
        g.const_poll_loops = g.compute_const_poll_loops();
        g
    }

    /// model.py `Game._resistance` (its docstring has the engine rules and cites): the F flags of each
    /// script's slots, and the starts whose script number is not a constant.
    pub fn compute_resistance(&self) -> (FxHashMap<UnitKey, Resist>, Vec<WildStart>) {
        let ast = &self.ast;
        let mut flags: FxHashMap<UnitKey, u8> = FxHashMap::default();
        let mut wild = Vec::new();
        let mut chains: Vec<(Option<UnitKey>, UnitKey)> = Vec::new();
        for u in self.ids() {
            let key = self.unit(u).key;
            let room = match key {
                UnitKey::Local(r, _) | UnitKey::Entry(r) | UnitKey::Exit(r) => Some(r),
                _ => None,
            };
            let src = matches!(key, UnitKey::Global(_) | UnitKey::Local(..)).then_some(key);
            for (i, x) in self.ins(u).iter().enumerate() {
                match x.st {
                    Some(Stmt::Start { name, script, freeze, .. })
                        if name == known::START_SCRIPT || name == known::CHAIN_SCRIPT =>
                    {
                        match ast.constant(script) {
                            None => wild.push((self.cite(u, i as u32), (name == known::START_SCRIPT).then_some(freeze))),
                            Some(n) if name == known::START_SCRIPT => {
                                for dst in self.started(n, room) {
                                    *flags.entry(dst).or_insert(0) |= if freeze { 2 } else { 1 };
                                }
                            }
                            Some(n) => chains.extend(self.started(n, room).into_iter().map(|dst| (src, dst))),
                        }
                    }
                    Some(Stmt::Assign(a)) => {
                        if let (Expr::Var(v), Some(n)) = (ast.expr(a.lhs), ast.constant(a.rhs)) {
                            if analysis::HOOK_VARS.contains(&v) && n != 0 {
                                for dst in self.started(n, None) {
                                    *flags.entry(dst).or_insert(0) |= 1;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut changed = true;
        while changed {
            changed = false;
            for &(src, dst) in &chains {
                let add = match src {
                    Some(s) => {
                        let f = *flags.entry(s).or_insert(0);
                        if f == 0 {
                            1
                        } else {
                            f
                        }
                    }
                    None => 1,
                };
                let d = flags.entry(dst).or_insert(0);
                if add & !*d != 0 {
                    *d |= add;
                    changed = true;
                }
            }
        }
        (flags.into_iter().map(|(k, b)| (k, Resist::from_bits(b))).collect(), wild)
    }

    /// model.py `Game._started`: the units a start of script `n` from code of `room` (None: global or
    /// object code, or a hook) can run.
    pub fn started(&self, n: i32, room: Option<i32>) -> Vec<UnitKey> {
        if n < self.first_local {
            return vec![UnitKey::Global(n)];
        }
        match room {
            Some(r) => vec![UnitKey::Local(r, n)],
            None => self.local_rooms.get(&n).map_or(Vec::new(), |rs| rs.iter().map(|&r| UnitKey::Local(r, n)).collect()),
        }
    }

    /// model.py `Game.resistant`: the F flag of a live slot of `script:N` / `local:N` the path did not
    /// start, `res` the room resource in use. Some when every constant start agrees (a script nothing
    /// starts has none), None when they differ or the room is not known.
    pub fn resistant(&self, key: Fact, res: Option<i32>) -> Option<bool> {
        let unit = match key {
            Fact::Script(n) => UnitKey::Global(n),
            Fact::Local(n) => UnitKey::Local(res?, n),
            _ => return Some(false),
        };
        let r = self.resistance.get(&unit).copied().unwrap_or_default();
        match (r.without_f, r.with_f) {
            (true, true) => None,
            (_, with_f) => Some(with_f),
        }
    }

    /// model.py `Game.resource`: the room resource a room number loads; a pseudo room maps through
    /// _resourceMapper (None: no mapping).
    pub fn resource(&self, room: i32) -> Option<i32> {
        if room >= 0x80 {
            self.pseudo.get(&room).copied()
        } else {
            Some(room)
        }
    }

    /// model.py `Game.script_key`: a script number's fact.
    pub fn script_key(&self, n: i32) -> Fact {
        if n >= self.first_local {
            Fact::Local(n)
        } else {
            Fact::Script(n)
        }
    }

    /// model.py `Game.entry`: the instruction object `obj` runs for `verb` (its event, else its 0xFF one).
    pub fn entry(&self, obj: i32, verb: i32) -> Option<u32> {
        let u = self.unit(self.id(UnitKey::Obj(obj))?);
        u.entry_of(verb).or_else(|| u.entry_of(read::DEFAULT_VERB))
    }

    /// model.py `unit_name`: the unit as `at:` and `esc:` keys write it.
    pub fn unit_name(key: UnitKey) -> Slot {
        match key {
            UnitKey::Global(n) => Slot::Script(n),
            UnitKey::Local(_, n) => Slot::Local(n),
            UnitKey::Obj(o) => Slot::Obj(o),
            UnitKey::Entry(_) => Slot::Entry,
            UnitKey::Exit(_) => Slot::Exit,
        }
    }

    /// model.py `lifecycle_key`: the fact of the unit's own slot (entry and exit code have none).
    pub fn lifecycle_key(&self, key: UnitKey) -> Option<Fact> {
        match key {
            UnitKey::Global(n) => Some(Fact::Script(n)),
            UnitKey::Local(_, n) => Some(self.script_key(n)),
            UnitKey::Obj(o) => Some(Fact::ObjScript(o)),
            _ => None,
        }
    }

    /// model.py `Game.join_points`: the instructions of a unit a jump goes to (the end included).
    pub fn join_points(&self, u: UnitId) -> Vec<u32> {
        (0..=self.unit(u).ins_len).filter(|&i| self.joins[self.point(u, i)]).collect()
    }

    #[inline]
    fn an(&self, const_polls: bool) -> &Analyses {
        &self.analyses[const_polls as usize]
    }

    /// model.py `Game.loops`.
    pub fn loops(&self, u: UnitId, const_polls: bool) -> &[Loop] {
        let a = self.an(const_polls);
        &a.loops[a.unit_loops[u.0 as usize].range()]
    }

    /// model.py `Game.wait_point` (its loop an index into `loops(u, const_polls)`).
    pub fn wait_point(&self, u: UnitId, idx: u32, const_polls: bool) -> WaitPoint {
        self.an(const_polls).wait_points[self.point(u, idx)]
    }

    /// One entry of model.py `Game.time_exits`: for jump `j`, (its successor outside the loop, the head).
    pub fn time_exit(&self, u: UnitId, j: u32, camera: bool, const_polls: bool) -> Option<(u32, u32)> {
        self.an(const_polls).time_exits[camera as usize][(self.unit(u).ins_start + j) as usize]
    }

    /// model.py `Game.time_exits`: every (jump, successor, head), by jump.
    pub fn time_exits(&self, u: UnitId, camera: bool, const_polls: bool) -> Vec<(u32, u32, u32)> {
        let t = &self.an(const_polls).time_exits[camera as usize][self.unit(u).range()];
        t.iter().enumerate().filter_map(|(j, e)| e.map(|(s, h)| (j as u32, s, h))).collect()
    }

    /// model.py `Game.time_loops_at`.
    pub fn time_loops_at(&self, u: UnitId, idx: u32, const_polls: bool) -> &[u32] {
        let a = self.an(const_polls);
        &a.time_loop_heads[a.time_loops_at[self.point(u, idx)].range()]
    }

    /// model.py `Game.const_poll_loops`.
    fn compute_const_poll_loops(&self) -> Vec<(UnitId, u32)> {
        let mut out = Vec::new();
        for u in self.ids() {
            // the same backward jumps in the same order, so the two lists pair up
            for (off, on) in self.loops(u, false).iter().zip(self.loops(u, true)) {
                if on.kind != off.kind {
                    out.push((u, on.head));
                }
            }
        }
        out
    }
}

/// The pseudo-room map o5_pseudoRoom builds, read from the scripts (model.py `Game.load`): descumm
/// prints each mapped room as j & 0x7F (descumm.cpp:1045-1060); the engine maps j (>= 0x80) to the
/// resource (script_v5.cpp:2085-2092).
fn pseudo_rooms(sc: &Script) -> (FxHashMap<i32, i32>, Vec<(String, String)>) {
    let mut pseudo: FxHashMap<i32, i32> = FxHashMap::default();
    let mut conflicts = Vec::new();
    for u in sc.ids() {
        for (i, x) in sc.ins(u).iter().enumerate() {
            let Some(Stmt::Op { name: known::PSEUDO_ROOM, args, .. }) = x.st else { continue };
            let vals: Vec<Option<i32>> = sc.ast.kids(args).iter().map(|&a| sc.ast.constant(a)).collect();
            let (res, rooms) = vals.split_first().expect("PseudoRoom with no arguments");
            for &j in rooms {
                match (res, j) {
                    (None, _) | (_, None) => conflicts.push((sc.cite(u, i as u32), "not a constant".to_string())),
                    (Some(res), Some(j)) => {
                        let room = j | 0x80;
                        match pseudo.get(&room) {
                            Some(&old) if old != *res => {
                                conflicts.push((sc.cite(u, i as u32), format!("room {room}: {old} and {res}")))
                            }
                            _ => {
                                pseudo.insert(room, *res);
                            }
                        }
                    }
                }
            }
        }
    }
    (pseudo, conflicts)
}
