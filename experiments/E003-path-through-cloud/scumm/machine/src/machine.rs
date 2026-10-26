//! model.py `Machine`: runs units over a fact oracle, one path per way through unknown conditions.
//!
//! Every method here is the model.py method of the same name (without the leading underscore), in the
//! same order of reads, writes, forks and ends, with model.py's strings: the golden dumps compare them
//! byte for byte. model.py's module and method docstrings have the engine rules and ScummVM cites; the
//! comments here say only what the port does differently in form, and the Python quirks it keeps
//! (PORT-NOTES.md lists them).
//!
//! Ported so far: row 0 (feature own_entry, `run_entry`), row 1 (features dispatch and calls,
//! `run_sentence`; the calls' machinery is in `calls.rs`), row 2 (feature room_change: startScene's steps,
//! in `rooms.rs`), row 3 (feature settle: `run_wait` and the rounds, in `settle.rs`), row 4 (feature dialog:
//! `run_verb` and the input step, in `dialog.rs`) and row 5 (feature esc: `run_esc` and the abort, override and key
//! steps, in `esc.rs`). Every feature of model.py is ported.

mod calls;
mod dialog;
mod esc;
mod hooks;
mod rooms;
mod settle;

pub use dialog::{LEFT_CLICK, VERB_CLICK_AREA};
pub use hooks::Hooks;
pub use settle::ROUND;

use crate::enc;
use crate::heads::{effect, Effect};
use crate::oracle::{FactOracle, OVal, Prefix};
use crate::path::*;
use crate::trace::Tracer;
use crate::value::*;
use rustc_hash::{FxHashMap, FxHashSet};
use scumm_ir::analysis::{getter_kind, GetterKind};
use scumm_ir::json::J;
use scumm_ir::show::{head as ir_head, py_repr, show_expr};
use scumm_ir::{known, AssignSt, Expr, ExprId, Fact, Game, Reason, RefKind, Slot, Span, Stmt, Sym, UnitId, UnitKey};
use std::rc::Rc;

// --- engine constants (ScummVM; model.py cites each) ---------------------------------------------------------
pub const VAR_RESULT: i32 = 0;
pub const VAR_KEYPRESS: i32 = 0;
pub const VAR_EGO: i32 = 1;
pub const VAR_ROOM: i32 = 4;
pub const VAR_OVERRIDE: i32 = 5;
pub const VAR_CAMERA_MIN_X: i32 = 17;
pub const VAR_CAMERA_MAX_X: i32 = 18;
pub const VAR_ROOM_RESOURCE: i32 = 22;
pub const VAR_CUTSCENEEXIT_KEY: i32 = 24;
pub const VAR_TALK_ACTOR: i32 = 25;
pub const VAR_SCROLL_SCRIPT: i32 = 27;
pub const VAR_ENTRY_SCRIPT: i32 = 28;
pub const VAR_ENTRY_SCRIPT2: i32 = 29;
pub const VAR_EXIT_SCRIPT: i32 = 30;
pub const VAR_EXIT_SCRIPT2: i32 = 31;
pub const VAR_VERB_SCRIPT: i32 = 32;
pub const VAR_SENTENCE_SCRIPT: i32 = 33;
pub const VAR_INVENTORY_SCRIPT: i32 = 34;
pub const VAR_CUTSCENE_START_SCRIPT: i32 = 35;
pub const VAR_CUTSCENE_END_SCRIPT: i32 = 36;
pub const VAR_WALKTO_OBJ: i32 = 38;
pub const VAR_CURSORSTATE: i32 = 52;
pub const VAR_USERPUT: i32 = 53;
pub const VAR_ROOM_FLAG: i32 = 70;
pub const VAR_NEW_ROOM: i32 = 72;
pub const MAX_NESTING: usize = 15;
pub const OF_OWNER_ROOM: i32 = 0x0F;
pub const MESSAGE_BOX_ACTOR: i32 = 252;
pub const CLASS_UNTOUCHABLE: i32 = 32;
pub const NUM_ACTORS: i32 = 13;
pub const SCREEN_WIDTH: i32 = 320;
pub const PATH_LIMIT: usize = 256;
pub const STEP_LIMIT: u32 = 5000;
pub const ROUND_LIMIT: u32 = 400;
/// model.py `TIME_VARS`' keys: VAR_CAMERA_POS_X, VAR_HAVE_MSG, VAR_LAST_SOUND, VAR_TALK_ACTOR
pub const TIME_VARS: [i32; 4] = [2, 3, 23, 25];
/// path ends left out of the predictions (model.py `INFEASIBLE`)
pub const INFEASIBLE: [&str; 3] = ["spin", "nesting", "error"];

/// model.py `FEATURES`, as flags.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Features {
    pub own_entry: bool,
    pub dispatch: bool,
    pub calls: bool,
    pub room_change: bool,
    pub settle: bool,
    pub dialog: bool,
    pub esc: bool,
}

/// The features this port runs so far (PORT-NOTES.md).
pub const PORTED: &[&str] = &["own_entry", "dispatch", "calls", "room_change", "settle", "dialog", "esc"];

impl Features {
    /// The feature set of model.py's names, checked as `Machine.__init__` checks it, then against what is
    /// ported.
    pub fn parse(names: &[String]) -> Result<Features, String> {
        let mut f = Features::default();
        for n in names {
            match n.as_str() {
                "own_entry" => f.own_entry = true,
                "dispatch" => f.dispatch = true,
                "calls" => f.calls = true,
                "room_change" => f.room_change = true,
                "settle" => f.settle = true,
                "dialog" => f.dialog = true,
                "esc" => f.esc = true,
                _ => return Err(format!("unknown feature {n}")),
            }
        }
        if !f.own_entry {
            return Err("row 0 (own_entry) is the base every row builds on".into());
        }
        if f.dispatch != f.calls {
            return Err("dispatch and calls make sense only together (row 1)".into());
        }
        if f.room_change && !f.calls {
            return Err("room_change runs the exit and entry scripts nested: it needs calls".into());
        }
        if f.settle && !f.room_change {
            return Err("settle runs the rounds after rows 0-2 (row 3): it needs room_change".into());
        }
        if (f.dialog || f.esc) && !f.settle {
            return Err("dialog and esc need settle".into());
        }
        if let Some(n) = names.iter().find(|n| !PORTED.contains(&n.as_str())) {
            return Err(format!("feature {n} is not ported to Rust yet (PORT-NOTES.md)"));
        }
        Ok(f)
    }
}

/// model.py `SWITCHES`, as flags.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Switches {
    pub input_after_turns: bool,
    pub const_polls: bool,
    pub esc_key_at_pre: bool,
    /// fix round 1, F12 (fix-round1.md; Rust only, no model.py switch): the mouse variables as ranges, findObject
    /// and actorFromPos as value sets, tests on ranges and on value sets past MAX_VALUES decided where every value
    /// agrees, and a forked test on a variable or local that holds a range or a set narrowing it on each side
    pub value_sets: bool,
}

impl Switches {
    pub fn parse(names: &[String]) -> Result<Switches, String> {
        let mut s = Switches::default();
        for n in names {
            match n.as_str() {
                "input_after_turns" => s.input_after_turns = true,
                "const_polls" => s.const_polls = true,
                "esc_key_at_pre" => s.esc_key_at_pre = true,
                "value_sets" => s.value_sets = true,
                _ => return Err(format!("unknown switch {n}")),
            }
        }
        Ok(s)
    }
}

/// An instruction: its unit and index (model.py passes the `Ins` itself, `x`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct X {
    pub u: UnitId,
    pub i: u32,
}

/// model.py `Machine._step`'s write targets (`target`): a fact key, a local, or an unknown.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Target {
    Fact(Fact),
    Local(i32),
    Unk(Val),
}

/// model.py `Machine.where`: whereIsObject.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Where {
    Inventory,
    Room,
    Nowhere,
    Unk(Val),
}

/// A shared string from a str.
#[inline]
pub fn rc(s: &str) -> S {
    Rc::from(s)
}

/// model.py `slot_name`: a slot fact's unit name (objscript:O is obj:O).
pub fn slot_name(key: Fact) -> Slot {
    match key {
        Fact::Script(n) => Slot::Script(n),
        Fact::Local(n) => Slot::Local(n),
        Fact::ObjScript(o) => Slot::Obj(o),
        k => panic!("slot_name of {k}, not a slot fact"),
    }
}

/// Facts sorted as model.py sorts their strings.
pub fn sort_str(keys: &mut [Fact]) {
    keys.sort_by_cached_key(|k| k.to_string());
}

fn waits(name: Sym) -> (Reason, &'static str) {
    match name {
        known::WAIT_FOR_ACTOR => (Reason::Actor, "whether the actor is moving"),
        known::WAIT_FOR_MESSAGE => (Reason::Message, "whether a message shows"),
        known::WAIT_FOR_CAMERA => (Reason::Position, "whether the camera has reached its target"),
        known::WAIT_FOR_SENTENCE => (Reason::Sentence, "whether a sentence is queued or the sentence script runs"),
        _ => unreachable!("not a WaitFor op"),
    }
}

/// model.py `Machine`.
pub struct Machine<'g> {
    pub game: &'g Game,
    pub features: Features,
    pub switches: Switches,
    pub calls: bool,
    pub rooms: bool,
    pub settles: bool,
    pub dialogs: bool,
    pub escs: bool,
    pub path_limit: usize,
    pub step_limit: u32,
    pub round_limit: u32,
    /// golden.py's TraceMachine events, when a run is traced
    pub trace: Option<Tracer>,
    /// the current run's unknowns (handed to its `Run`)
    v: Vals,
    /// cites by instruction (global index), made once
    cites: Vec<Option<S>>,
    // the current run's state (model.py's `self._pre`, `self._key_routes`, merge counters)
    pre: Vec<Slot>,
    key_routes: Vec<(S, Vec<Assumption>)>,
    merges: u32,
    settle_merges: u32,
    value_merges: u32,
    /// the next `Call.cid`
    next_cid: u64,
    /// the run's unknowns of the oracle and of TIME_VARS, by key (with `v`, a run's)
    ocache: FxHashMap<(Fact, u8), Val>,
    /// a condition's printed text, by expression
    conds: FxHashMap<ExprId, S>,
    /// a subclass's overrides (hooks.rs); None: model.py's Machine itself
    pub hooks: Option<&'static dyn Hooks>,
    /// the hooking side's own state
    pub ext: Option<Box<dyn std::any::Any>>,
    /// F12: the objects findObject can give besides 0 (the reader's scope); None: the room's objects
    pub find_objects: Option<Rc<[i32]>>,
    /// F12: the tallest main screen the scripts set (`mouse_range`), worked out once
    screen_h: std::cell::Cell<Option<i32>>,
}

/// F12: the engine's mouse bounds for SCUMM v5 (input.cpp:414-434): the mouse is clipped to the screen,
/// _screenWidth x _screenHeight (scumm.cpp:395-396), VAR_VIRT_MOUSE_X adds the camera's left edge (at most a
/// room's width less the screen's), and VAR_VIRT_MOUSE_Y subtracts the main screen's top and is -1 off it
/// (its height set by resetScumm, initScreens(16, 144) for v5 (scumm.cpp:2053), then by the scripts' roomOps
/// SetScreen, script_v5.cpp:2392-2397, gfx.cpp:386).
pub const SCREEN_HEIGHT: i32 = 200;
/// F12: the main screen's height resetScumm sets for SCUMM v5 (initScreens(16, 144), scumm.cpp:2053).
pub const RESET_MAIN_HEIGHT: i32 = 144 - 16;

impl<'g> Machine<'g> {
    /// F12: the range of mouse variable `n` (VAR_VIRT_MOUSE_X/Y, VAR_MOUSE_X/Y), from the game: the widest room, the
    /// tallest main screen the scripts set (the screen's height where one is not a constant).
    pub fn mouse_range(&self, n: i32) -> (i32, i32) {
        let g = self.game;
        match n {
            44 => (0, SCREEN_WIDTH - 1),
            45 => (0, SCREEN_HEIGHT - 1),
            20 => (0, g.room_width.values().copied().max().unwrap_or(SCREEN_WIDTH).max(SCREEN_WIDTH) - 1),
            21 => {
                if let Some(h) = self.screen_h.get() {
                    return (-1, h - 1);
                }
                // resetScumm's main screen, then every roomOps SetScreen(top, bottom) of the scripts: the main screen
                // is bottom - top high
                let mut h = RESET_MAIN_HEIGHT;
                let mut seen = |args: Span| {
                    let a: Vec<Option<i32>> = g.ast.kids(args).iter().map(|&e| g.ast.constant(e)).collect();
                    h = match a.as_slice() {
                        [Some(t), Some(b)] => h.max(b - t),
                        _ => SCREEN_HEIGHT,
                    };
                };
                for x in g.ins.iter() {
                    if let Some(Stmt::Op { name, args, subops, .. }) = x.st {
                        if name == known::SET_SCREEN {
                            seen(args);
                        }
                        for sub in g.ast.subs(subops) {
                            if sub.name == known::SET_SCREEN {
                                seen(sub.args);
                            }
                        }
                    }
                }
                let h = h.clamp(1, SCREEN_HEIGHT);
                self.screen_h.set(Some(h));
                (-1, h - 1)
            }
            _ => panic!("var {n} is not a mouse variable"),
        }
    }

    /// F12: the value a mouse variable has at a click: its range (switch value_sets), else model.py's unknown.
    pub(crate) fn mouse_value(&mut self, n: i32, name: &str) -> Val {
        let what = format!("{name}: the mouse moved to the click");
        if self.switches.value_sets {
            let (lo, hi) = self.mouse_range(n);
            self.v.mk_range(Reason::Input, &what, lo, hi)
        } else {
            self.v.mk(Reason::Input, what)
        }
    }
}

impl<'g> Machine<'g> {
    /// `Machine(features, game, switches=...)` with model.py's limits.
    pub fn new(game: &'g Game, features: Features, switches: Switches) -> Machine<'g> {
        Machine {
            game,
            features,
            switches,
            calls: features.calls,
            rooms: features.room_change,
            settles: features.settle,
            dialogs: features.dialog,
            escs: features.esc,
            path_limit: PATH_LIMIT,
            step_limit: STEP_LIMIT,
            round_limit: ROUND_LIMIT,
            trace: None,
            v: Vals::default(),
            cites: vec![None; game.ins.len()],
            pre: Vec::new(),
            key_routes: Vec::new(),
            merges: 0,
            settle_merges: 0,
            value_merges: 0,
            next_cid: 0,
            ocache: FxHashMap::default(),
            conds: FxHashMap::default(),
            hooks: None,
            ext: None,
            find_objects: None,
            screen_h: Default::default(),
        }
        .with_wide()
    }

    /// The run's values follow switch value_sets (F12).
    fn with_wide(mut self) -> Self {
        self.v.wide = self.switches.value_sets;
        self
    }

    // --- helpers ----------------------------------------------------------------------------------------

    /// `x.cite()`, made once per instruction.
    pub fn cite(&mut self, x: X) -> S {
        let gi = (self.game.unit(x.u).ins_start + x.i) as usize;
        if let Some(c) = &self.cites[gi] {
            return c.clone();
        }
        let c: S = self.game.cite(x.u, x.i).into();
        self.cites[gi] = Some(c.clone());
        c
    }

    fn cite_of(&mut self, x: Option<X>) -> S {
        match x {
            Some(x) => self.cite(x),
            None => rc(""),
        }
    }

    fn show(&self, v: Val) -> String {
        self.v.show(v)
    }

    fn show_e(&self, e: ExprId) -> String {
        show_expr(&self.game.ast, &self.game.syms, e)
    }

    fn sym(&self, y: Sym) -> &'g str {
        self.game.syms.str(y)
    }

    fn unit_key(&self, u: UnitId) -> UnitKey {
        self.game.unit(u).key
    }

    /// The oracle's answer as a value (`FactOracle.get`).
    pub fn oget_base(&mut self, o: &FactOracle, k: Fact) -> Val {
        let (reason, tag) = match o.get(k) {
            OVal::Int(n) => return Val::Int(n),
            OVal::Held => (Reason::Held, 0),
            OVal::Timer => (Reason::Timer, 1),
        };
        if let Some(&v) = self.ocache.get(&(k, tag)) {
            return v;
        }
        let v = self.v.mk(reason, k.to_string());
        self.ocache.insert((k, tag), v);
        v
    }

    /// `Unknown(reason, name)` of a getter that reads no fact, made once per run.
    fn getter_unknown(&mut self, r: Reason, name: Sym) -> Val {
        let k = (Fact::Var(name.0 as i32), 4);
        if let Some(&v) = self.ocache.get(&k) {
            return v;
        }
        let v = self.v.mk(r, self.sym(name));
        self.ocache.insert(k, v);
        v
    }

    /// `Unknown(R_LOCAL, f"Local[{n}]")`, made once per run.
    pub(crate) fn unknown_local(&mut self, n: i32) -> Val {
        let k = (Fact::Var(n), 3);
        if let Some(&v) = self.ocache.get(&k) {
            return v;
        }
        let v = self.v.mk(Reason::Local, format!("Local[{n}]"));
        self.ocache.insert(k, v);
        v
    }

    /// A condition's printed text (`show(cond)`), made once.
    fn cond_text(&mut self, e: ExprId) -> S {
        if let Some(s) = self.conds.get(&e) {
            return s.clone();
        }
        let s = rc(&self.show_e(e));
        self.conds.insert(e, s.clone());
        s
    }

    /// The Run of an entry point that ran nothing, or of `_drive`: it takes the run's unknowns.
    fn finish_run(&mut self, mut r: Run) -> Run {
        r.vals = std::mem::take(&mut self.v);
        self.v.wide = self.switches.value_sets;
        self.ocache.clear();
        r
    }

    fn ev(&mut self, p: &mut Path, typ: &str, kw: Vec<(&'static str, J)>) {
        if let Some(t) = &mut self.trace {
            t.ev(p, typ, kw);
        }
    }

    // --- entry points -------------------------------------------------------------------------------------

    /// Row 0: object `obj`'s own entry for `verb` (model.py `run_entry`).
    pub fn run_entry(&mut self, o: &FactOracle, obj: i32, verb: i32, locals: Option<Vec<(i32, Val)>>) -> Run {
        let Some(idx) = self.game.entry(obj, verb) else {
            let why = if self.game.id(UnitKey::Obj(obj)).is_none() {
                "no code".to_string()
            } else {
                format!("no entry for verb {verb} or 255")
            };
            let r = Run { start: format!("object {obj}: {why}"), unit: Some(UnitKey::Obj(obj)), ..Default::default() };
            return self.finish_run(r);
        };
        let owner = self.oget(o, Fact::Owner(obj));
        let owned = match owner {
            Val::Int(n) => Owned::Bool(n != OF_OWNER_ROOM),
            Val::Unk(_) => Owned::Unk(owner),
        };
        // every v5 object slot runs without R (script_v5.cpp:2889, script.cpp:1432)
        self.run(o, UnitKey::Obj(obj), idx, locals, owned, Some(false))
    }

    /// Every path from instruction `pc` of `unit` (model.py `run`); `locals` None means unknown arguments.
    pub fn run(
        &mut self,
        o: &FactOracle,
        unit: UnitKey,
        pc: u32,
        locals: Option<Vec<(i32, Val)>>,
        owned: Owned,
        recursive: Option<bool>,
    ) -> Run {
        self.pre.clear();
        let g = self.game;
        let uid = g.id(unit).unwrap_or_else(|| panic!("no unit {unit}"));
        let mut f = Frame::new(uid, Some(pc), locals.is_some(), owned, recursive);
        if let Some(ls) = locals {
            f.locals = Locals::from_vec(ls);
        }
        f.sid = 1;
        let mut first = Path::new(vec![Entry::Frame(f)]);
        first.next_sid = 2;
        if self.calls {
            // the unit is the first frame of a stack, and its old wait points go (it runs from pc now)
            first.depth = 1;
            self.clear(&mut first, o, Prefix::At(Game::unit_name(unit)));
        }
        let u = g.unit(uid);
        let start = if pc < u.ins_len { g.cite(uid, pc) } else { format!("{}: past the end", g.syms.str(u.file)) };
        self.drive(o, first, start, Some(unit))
    }

    // --- the run ------------------------------------------------------------------------------------------

    /// model.py `_drive`: every path to its end. With calls on, a path at a join waits there; once no path
    /// can go on, the deepest waiting groups (in the order their signatures first waited) merge into one
    /// path each (`merge`) and go on.
    fn drive(&mut self, o: &FactOracle, mut first: Path, start: String, unit: Option<UnitKey>) -> Run {
        let g = self.game;
        if self.trace.is_some() {
            let top = enc::where_(g, &first);
            let u = unit.map_or(J::Null, enc::unit_key);
            self.ev(&mut first, "start", vec![("start", enc::s(&start)), ("unit", u), ("top", top)]);
        }
        let mut todo = vec![first];
        let mut done: Vec<Path> = Vec::new();
        // model.py's dict of signature -> waiting paths, in insertion order (a popped signature that waits
        // again goes last)
        let mut waiting: Vec<(Rc<Sig>, Vec<Path>)> = Vec::new();
        // a waiting group's place by its signature's hash (a second group of the same hash is found by a scan)
        let mut index: FxHashMap<u64, usize> = FxHashMap::default();
        let mut n_waiting = 0usize;
        let (mut limit, mut dead) = (0u32, 0usize);
        self.merges = 0;
        self.settle_merges = 0;
        self.value_merges = 0;
        self.key_routes.clear();
        self.v.caps.clear();
        loop {
            let Some(mut p) = todo.pop() else {
                if waiting.is_empty() {
                    break;
                }
                let deepest = waiting.iter().map(|w| w.1[0].stack.len()).max().unwrap();
                let mut rest = Vec::with_capacity(waiting.len());
                for (sig, group) in waiting.drain(..) {
                    if group[0].stack.len() == deepest {
                        n_waiting -= group.len();
                        let q = self.merge(o, group, sig);
                        todo.push(q);
                    } else {
                        rest.push((sig, group));
                    }
                }
                waiting = rest;
                index.clear();
                for (i, w) in waiting.iter().enumerate() {
                    index.entry(w.0.h).or_insert(i);
                }
                continue;
            };
            let mut parked: Option<Sig> = None;
            while p.end.is_none() {
                if p.steps - p.steps0 >= self.step_limit {
                    let at = format!("{} steps", self.step_limit);
                    self.halt(&mut p, "limit", rc(&at));
                    break;
                }
                if self.calls {
                    if p.resumed {
                        // just out of a merge at this join: its signature is the one it merged at (model.py works
                        // it out again, and gets the same: a merge changes nothing the signature holds)
                        let sig = p.joined.clone().expect("a merged path's join");
                        debug_assert!(self.join(&p, o).as_ref() == Some(&*sig));
                        if matches!(p.stack.last(), Some(Entry::Frame(_))) && self.spins(&mut p, &sig) {
                            break;
                        }
                    } else {
                        if let Some(sig) = self.join(&p, o) {
                            if p.joined.as_deref() != Some(&sig) {
                                parked = Some(sig);
                                break;
                            }
                            if matches!(p.stack.last(), Some(Entry::Frame(_))) && self.spins(&mut p, &sig) {
                                break;
                            }
                        }
                    }
                }
                p.resumed = false;
                let (n, at, routes) = (p.assumptions.len(), p.frame().and_then(|f| f.pc), self.key_routes.len());
                let mut more = self.advance(&mut p, o);
                if !more.is_empty() && self.trace.is_some() {
                    self.ev_fork(&mut p, &mut more);
                }
                if !more.is_empty() && done.len() - dead + todo.len() + n_waiting + 1 + more.len() > self.path_limit {
                    // a fork past the limit: neither side runs; the path ends where it forked
                    limit += more.len() as u32;
                    p.assumptions.truncate(n);
                    self.key_routes.truncate(routes);
                    p.end = Some("limit");
                    let at = at.map_or("None".to_string(), |a| a.to_string());
                    p.end_at = Some(rc(&format!("{} paths, at instruction {at}", self.path_limit)));
                } else {
                    todo.append(&mut more);
                }
            }
            if let Some(sig) = parked {
                // it waits at a join
                n_waiting += 1;
                let at = match index.get(&sig.h) {
                    Some(&i) if *waiting[i].0 == sig => Some(i),
                    Some(_) => waiting.iter().position(|w| *w.0 == sig),
                    None => None,
                };
                match at {
                    Some(i) => waiting[i].1.push(p),
                    None => {
                        index.entry(sig.h).or_insert(waiting.len());
                        waiting.push((Rc::new(sig), vec![p]));
                    }
                }
                continue;
            }
            let end = p.end.expect("an ended path");
            let infeasible = INFEASIBLE.contains(&end);
            if p.settle && !infeasible {
                self.close_settle(&mut p, o);
            }
            dead += infeasible as usize;
            done.push(p);
        }
        let r = Run {
            paths: done,
            start,
            limit,
            unit,
            merges: self.merges,
            value_caps: self.v.caps.clone(),
            settle_merges: self.settle_merges,
            pre_slots: self.pre.clone(),
            value_merges: self.value_merges,
            key_routes: std::mem::take(&mut self.key_routes),
            vals: Vals::default(),
        };
        self.finish_run(r)
    }

    /// golden.py's fork event (TraceMachine `_advance`).
    fn ev_fork(&mut self, p: &mut Path, more: &mut [Path]) {
        let g = self.game;
        let Some(t) = &mut self.trace else { return };
        t.tid(p);
        let kids: Vec<J> = more
            .iter_mut()
            .map(|q| {
                let tid = t.tid(q);
                J::Arr(vec![J::Int(tid as i64), q.assumptions.last().map_or(J::Null, enc::assumption)])
            })
            .collect();
        let assume = p.assumptions.last().map_or(J::Null, enc::assumption);
        let wh = enc::where_(g, p);
        t.ev(p, "fork", vec![("children", J::Arr(kids)), ("assume", assume), ("where", wh)]);
    }

    pub fn advance_base(&mut self, p: &mut Path, o: &FactOracle) -> Vec<Path> {
        match p.stack.last() {
            None => {
                self.halt(p, "end", rc("nothing left to run"));
                Vec::new()
            }
            Some(Entry::Call(_)) => self.call_step(p, o),
            Some(Entry::Frame(_)) => self.step(p, o),
        }
    }

    // --- reading ------------------------------------------------------------------------------------------

    /// The path's own write first, else the oracle's (model.py `read`).
    pub fn read_base(&mut self, p: &mut Path, k: Fact, o: &FactOracle) -> Val {
        let v = if let Some(&v) = p.writes.get(&k) {
            v
        } else if p.round != 0 && matches!(k, Fact::Var(n) if TIME_VARS.contains(&n)) {
            match self.ocache.get(&(k, 2)) {
                Some(&v) => v,
                None => {
                    let v = self.v.mk(Reason::Timer, format!("{k}: an engine variable that changes as frames pass"));
                    self.ocache.insert((k, 2), v);
                    v
                }
            }
        } else {
            self.oget(o, k)
        };
        if let Some(ow) = p.owner {
            p.rw.read(ow, k);
        }
        v
    }

    fn write(&mut self, p: &mut Path, k: Fact, v: Val) {
        p.writes.insert(k, v);
        if let Some(ow) = p.owner {
            p.rw.write(ow, k);
            p.writer.insert(k, p.owner_rule);
        }
    }

    fn local(&mut self, p: &Path, n: i32) -> Val {
        let f = p.frame().expect("a frame");
        if let Some(v) = f.local(n) {
            return v;
        }
        if f.args_known {
            Val::Int(0)
        } else {
            self.unknown_local(n)
        }
    }

    pub fn val_base(&mut self, p: &mut Path, o: &FactOracle, e: ExprId, x: Option<X>) -> Val {
        let g = self.game;
        match g.ast.expr(e) {
            Expr::Const(v) => Val::Int(v),
            Expr::Var(n) => self.read(p, Fact::Var(n), o),
            Expr::Bit(n) => self.read(p, Fact::Bit(n), o),
            Expr::Local(n) => self.local(p, n),
            Expr::Index { kind, base, offset } => {
                let ov = self.val(p, o, offset, x);
                match ov {
                    Val::Unk(_) => {
                        let w = format!("index of {} ({})", self.show_e(e), self.show(ov));
                        self.v.mk(Reason::Work, w)
                    }
                    Val::Int(k) => {
                        let n = key_num(base as i64 + k as i64);
                        match kind {
                            RefKind::Local => self.local(p, n),
                            RefKind::Var => self.read(p, Fact::Var(n), o),
                            RefKind::Bit => self.read(p, Fact::Bit(n), o),
                        }
                    }
                }
            }
            Expr::BinOp { op, a, b } => {
                let va = self.val(p, o, a, x);
                let vb = self.val(p, o, b, x);
                self.arith_at(p, op.into(), va, vb, x)
            }
            Expr::Neg(a) => match self.val(p, o, a, x) {
                Val::Int(v) => Val::Int(wrap32(-(v as i64))),
                u if self.v.wide && self.v.range(u).is_some() => self.v.arith(Op::Sub, Val::Int(0), u),
                u => u,
            },
            Expr::Getter { name, args } => self.getter(p, o, name, args, x),
            Expr::Embed(st) => {
                // an opcode inside an expression: it writes var 0, its value is var 0
                self.assign(p, o, st, x);
                self.val(p, o, st.lhs, x)
            }
            Expr::Str(_) | Expr::Concat(_) | Expr::List(_) => self.v.mk(Reason::Work, "text"),
            Expr::Flag(_) | Expr::Cmp { .. } | Expr::Not(_) | Expr::Truth(_) => {
                let w = self.show_e(e);
                self.v.mk(Reason::Work, w)
            }
        }
    }

    /// A printed condition (model.py `test`): comparisons cast to int16, the zero tests do not.
    pub fn test_base(&mut self, p: &mut Path, o: &FactOracle, c: ExprId, x: Option<X>) -> Dec {
        use scumm_ir::CmpOp::*;
        let g = self.game;
        match g.ast.expr(c) {
            Expr::Cmp { op, a, b } => {
                let va = self.val(p, o, a, x);
                let vb = self.val(p, o, b, x);
                self.v.decide(&[va, vb], |v| {
                    let (y, z) = (i16w(v[0]), i16w(v[1]));
                    match op {
                        Eq => y == z,
                        Ne => y != z,
                        Lt => y < z,
                        Le => y <= z,
                        Gt => y > z,
                        Ge => y >= z,
                    }
                })
            }
            Expr::Not(a) => {
                let v = self.val(p, o, a, x);
                self.v.decide(&[v], |v| v[0] == 0)
            }
            Expr::Truth(a) => {
                let v = self.val(p, o, a, x);
                self.v.decide(&[v], |v| v[0] != 0)
            }
            _ => {
                let w = format!("condition {}", py_repr(&g.ast, &g.syms, c));
                Dec::Unk(self.v.mk(Reason::Work, w))
            }
        }
    }

    /// The room resource a room number loads (model.py `Game.resource`, with its unknowns).
    fn resource(&mut self, room: Val) -> Val {
        match room {
            Val::Int(r) if r >= 0x80 => match self.game.pseudo.get(&r) {
                Some(&res) => Val::Int(res),
                None => self.v.mk(Reason::Work, format!("pseudo room {r} has no mapping")),
            },
            other => other,
        }
    }

    /// whereIsObject (model.py `where`).
    fn where_(&mut self, p: &mut Path, o: &FactOracle, obj: i32) -> Where {
        let owner = self.read(p, Fact::Owner(obj), o);
        match owner {
            Val::Unk(_) => Where::Unk(owner),
            Val::Int(ow) if ow != OF_OWNER_ROOM => {
                if ow != 0 {
                    Where::Inventory
                } else {
                    Where::Nowhere
                }
            }
            Val::Int(_) => {
                let room = self.read(p, Fact::Room, o);
                match self.resource(room) {
                    u @ Val::Unk(_) => Where::Unk(u),
                    Val::Int(r) => {
                        if self.game.object_room.get(&obj) == Some(&r) {
                            Where::Room
                        } else {
                            Where::Nowhere
                        }
                    }
                }
            }
        }
    }

    pub fn getter_base(&mut self, p: &mut Path, o: &FactOracle, name: Sym, args: Span, x: Option<X>) -> Val {
        let g = self.game;
        if name == known::FIND_INVENTORY && self.calls {
            return self.find_inventory(p, o, args, x);
        }
        match getter_kind(name) {
            Some(GetterKind::Fact) => {}
            Some(GetterKind::NonFact(r)) => {
                if self.switches.value_sets {
                    if let Some(v) = self.getter_set(p, o, name) {
                        return v;
                    }
                }
                return self.getter_unknown(r, name);
            }
            None => return self.getter_unknown(Reason::Work, name),
        }
        let mut vals = Vec::new();
        for &a in g.ast.kids(args) {
            if !matches!(g.ast.expr(a), Expr::List(_)) {
                vals.push(self.val(p, o, a, x));
            }
        }
        if let Some(&bad) = vals.iter().find(|&&v| self.v.is_plain(v)) {
            if self.v.range(bad).is_some() {
                // F12: a range is no fact getter's value: the getter on it is not known
                let what = format!("{} of {}", self.sym(name), self.v.unk(bad).unwrap().what);
                return self.v.mk(Reason::Work, what);
            }
            return bad;
        }
        if let Some(&s0) = vals.iter().find(|v| v.is_unknown()) {
            // value sets: the getter on each combination
            let s0 = self.v.unk(s0).unwrap().clone();
            let what = format!("{} of {}", self.sym(name), s0.what);
            let mut combos: Vec<Vec<i32>> = vec![Vec::new()];
            for &v in &vals {
                let vs = self.v.values_of(v).unwrap();
                combos = combos.iter().flat_map(|c| vs.iter().map(move |&y| [c.as_slice(), &[y]].concat())).collect();
                if combos.len() > MAX_VALUES {
                    self.v.cap("getter");
                    return self.v.mk(s0.reason, what);
                }
            }
            let mut out: Vec<Val> = Vec::new();
            for c in combos {
                let cv: Vec<Val> = c.into_iter().map(Val::Int).collect();
                let r = self.getter_on(p, o, name, args, &cv, x);
                if !out.contains(&r) {
                    out.push(r);
                }
            }
            // quirk kept apart: model.py takes the first unknown of a set (hash order); this takes the first
            // in combination order (PORT-NOTES.md)
            if let Some(&bad) = out.iter().find(|v| v.is_unknown()) {
                return bad;
            }
            let ints = out.iter().map(|v| v.int().unwrap()).collect();
            return self.v.one_of(ints, s0.reason, what);
        }
        self.getter_on(p, o, name, args, &vals, x)
    }

    /// F12: findObject's value, 0 or an object (the reader's scope, else the room's: object.cpp:557-597), and
    /// actorFromPos's, 0 or an actor (actor.cpp:2115-2147, _numActors); None: the room is not known.
    fn getter_set(&mut self, p: &mut Path, o: &FactOracle, name: Sym) -> Option<Val> {
        let g = self.game;
        let mut vals: Vec<i32> = vec![0];
        match name {
            known::FIND_OBJECT => match &self.find_objects {
                Some(objs) => vals.extend(objs.iter().copied()),
                None => {
                    let room = self.read(p, Fact::Room, o);
                    let Val::Int(res) = self.resource(room) else { return None };
                    let mut objs: Vec<i32> = g.object_room.iter().filter(|e| *e.1 == res).map(|e| *e.0).collect();
                    objs.sort_unstable();
                    vals.extend(objs);
                }
            },
            known::ACTOR_FROM_POS => vals.extend(1..NUM_ACTORS),
            _ => return None,
        }
        Some(self.v.mk_set(Reason::Position, self.sym(name), vals))
    }

    /// The objects whose owner fact is `owner` (model.py `_owned_by`), or the first owner fact, in key
    /// order, whose value is not known, with that value.
    pub fn owned_by_base(&mut self, p: &mut Path, o: &FactOracle, owner: i32) -> Result<Vec<i32>, (Fact, Val)> {
        let obj_of = |k: Fact| match k {
            Fact::Owner(n) => n,
            _ => unreachable!(),
        };
        if !o.tracks("owner") {
            let mut keys: Vec<Fact> = o.keys(Prefix::Owner).to_vec();
            keys.extend(p.writes.keys().copied().filter(|&k| Prefix::Owner.matches(k)));
            keys.sort_unstable();
            keys.dedup();
            sort_str(&mut keys);
            let mut owned = Vec::new();
            for k in keys {
                let v = self.read(p, k, o);
                match v {
                    Val::Unk(_) => return Err((k, v)),
                    Val::Int(n) => {
                        if n == owner {
                            owned.push(obj_of(k));
                        }
                    }
                }
            }
            return Ok(owned);
        }
        let mut written: Vec<Fact> = p.writes.keys().copied().filter(|&k| Prefix::Owner.matches(k)).collect();
        sort_str(&mut written);
        let mut objs: std::collections::BTreeSet<i32> =
            o.owned().get(&owner).map(|v| v.iter().copied().collect()).unwrap_or_default();
        for k in written {
            let v = self.read(p, k, o);
            match v {
                Val::Unk(_) => return Err((k, v)),
                Val::Int(n) => {
                    if n == owner {
                        objs.insert(obj_of(k));
                    } else {
                        objs.remove(&obj_of(k));
                    }
                }
            }
        }
        Ok(objs.into_iter().collect())
    }

    /// A fact getter on known argument values (model.py `_getter_on`).
    fn getter_on(&mut self, p: &mut Path, o: &FactOracle, name: Sym, args: Span, vals: &[Val], x: Option<X>) -> Val {
        let g = self.game;
        let int = |v: Val| v.int().expect("a known argument");
        match name {
            known::GET_OBJECT_OWNER => self.read(p, Fact::Owner(int(vals[0])), o),
            known::GET_OBJECT_STATE => self.read(p, Fact::State(int(vals[0])), o),
            known::IS_SCRIPT_RUNNING => {
                let n = int(vals[0]);
                if n == 0 {
                    Val::Int(0)
                } else {
                    self.read(p, g.script_key(n), o)
                }
            }
            known::CLASS_OF_IS => {
                let obj = int(vals[0]);
                let Expr::List(items) = g.ast.expr(g.ast.kids(args)[1]) else {
                    panic!("classOfIs without a class list")
                };
                let mut unknown: Option<Val> = None;
                for &c in g.ast.kids(items) {
                    let cv = self.val(p, o, c, x);
                    match cv {
                        Val::Unk(_) => unknown = Some(cv),
                        Val::Int(n) if n & 0x7F == 0 => unknown = Some(self.v.mk(Reason::Work, "class 0")),
                        Val::Int(n) => {
                            let has = self.read(p, Fact::Class(obj, n & 0x7F), o);
                            match has {
                                Val::Unk(_) => unknown = Some(has),
                                Val::Int(h) => {
                                    if (h != 0) != (n & 0x80 != 0) {
                                        return Val::Int(0); // one class that does not match decides
                                    }
                                }
                            }
                        }
                    }
                }
                unknown.unwrap_or(Val::Int(1))
            }
            known::GET_INVENTORY_COUNT => match self.owned_by(p, o, int(vals[0])) {
                Err((_, v)) => v,
                Ok(owned) => {
                    let c = self.cite_of(x);
                    self.note(p, c, "getInventoryCount", "counted from owner facts; the engine counts its inventory list");
                    Val::Int(owned.len() as i32)
                }
            },
            known::GET_VERB_ENTRY_POINT => {
                assert_eq!(vals.len(), 2, "getVerbEntryPoint takes two values");
                let (obj, verb) = (int(vals[0]), int(vals[1]));
                let w = self.where_(p, o, obj);
                if let Where::Unk(u) = w {
                    return u;
                }
                let idx = g.entry(obj, verb);
                match idx {
                    Some(idx) if w != Where::Nowhere => {
                        let base = *g.verb_offset.get(&obj).unwrap_or_else(|| panic!("no verb offset for {obj}"));
                        let u = g.id(UnitKey::Obj(obj)).unwrap();
                        Val::Int(base + g.ins(u)[idx as usize].offset)
                    }
                    _ => Val::Int(0),
                }
            }
            _ => self.v.mk(Reason::Work, self.sym(name)),
        }
    }

    // --- writing ------------------------------------------------------------------------------------------

    /// getResultPos resolves the target before the value (model.py `target`).
    fn target(&mut self, p: &mut Path, o: &FactOracle, lhs: ExprId, x: Option<X>) -> Target {
        let g = self.game;
        match g.ast.expr(lhs) {
            Expr::Var(n) => Target::Fact(Fact::Var(n)),
            Expr::Bit(n) => Target::Fact(Fact::Bit(n)),
            Expr::Local(n) => Target::Local(n),
            Expr::Index { kind, base, offset } => {
                let ov = self.val(p, o, offset, x);
                match ov {
                    Val::Unk(_) => {
                        let w = format!("index of {} ({})", self.show_e(lhs), self.show(ov));
                        Target::Unk(self.v.mk(Reason::Work, w))
                    }
                    Val::Int(k) => {
                        let n = key_num(base as i64 + k as i64);
                        match kind {
                            RefKind::Local => Target::Local(n),
                            RefKind::Var => Target::Fact(Fact::Var(n)),
                            RefKind::Bit => Target::Fact(Fact::Bit(n)),
                        }
                    }
                }
            }
            _ => {
                let w = format!("target {}", py_repr(&g.ast, &g.syms, lhs));
                Target::Unk(self.v.mk(Reason::Work, w))
            }
        }
    }

    fn put(&mut self, p: &mut Path, t: Target, v: Val, x: Option<X>, head: &str) {
        match t {
            Target::Unk(u) => {
                let c = self.cite_of(x);
                let w = self.show(u);
                p.unknown_writes.push((rc(head), c, w.into()));
            }
            Target::Local(n) => p.frame_mut().expect("a frame").set_local(n, v),
            Target::Fact(k @ Fact::Bit(_)) => {
                let w = match v {
                    Val::Int(n) => Val::Int((n != 0) as i32), // writeVar on a bit (script.cpp)
                    u => u,
                };
                self.write(p, k, w);
            }
            Target::Fact(k) => {
                self.write(p, k, v);
                if k == Fact::Var(VAR_EGO) {
                    // the dump's ego is _scummVars[VAR_EGO] (speedrun_state.cpp:474)
                    let w = p.writes.get(&k).copied().expect("just written");
                    self.write(p, Fact::Ego, w);
                }
            }
        }
    }

    /// The keys with a prefix the oracle or the path's store holds, deduplicated.
    fn prefixed(&self, p: &Path, o: &FactOracle, pre: Prefix) -> Vec<Fact> {
        let mut keys: Vec<Fact> = self.okeys(o, pre);
        keys.extend(p.writes.keys().copied().filter(|&k| pre.matches(k)));
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    /// Every fact with this prefix that is not 0 goes to 0 (model.py `clear`; FactOracle always lists,
    /// so its note never happens). The order of the keys changes nothing observable.
    pub fn clear_base(&mut self, p: &mut Path, o: &FactOracle, pre: Prefix) {
        for k in self.prefixed(p, o, pre) {
            if self.read(p, k, o) != Val::Int(0) {
                self.write(p, k, Val::Int(0));
            }
        }
    }

    /// A slot stops (model.py `stopped`).
    fn stopped(&mut self, p: &mut Path, o: &FactOracle, key: Fact) {
        self.write(p, key, Val::Int(0));
        let name = slot_name(key);
        self.clear(p, o, Prefix::At(name));
        self.clear(p, o, Prefix::Esc(name));
        if self.calls {
            self.set_freeze(p, o, name, Some((0, true)), "");
        }
        p.pop_slot(name);
    }

    fn assign(&mut self, p: &mut Path, o: &FactOracle, a: AssignSt, x: Option<X>) {
        let g = self.game;
        let t = self.target(p, o, a.lhs, x);
        let mut v = self.val(p, o, a.rhs, x);
        if let Some(op) = a.op.arith() {
            let old = match t {
                Target::Unk(u) => u,
                _ => self.val(p, o, a.lhs, x),
            };
            v = self.arith_at(p, op.into(), old, v, x);
        }
        // the statement's head names an unknown write only: made then
        let head = if let Target::Unk(_) = t { ir_head(&g.ast, &g.syms, &Stmt::Assign(a)) } else { String::new() };
        self.put(p, t, v, x, &head);
    }

    /// `_arith` in a path: with calls on, a division by a known 0 is the engine's error() (o5_divide and
    /// o5_expression, script_v5.cpp:993-994, 1176-1179): the path ends there.
    pub fn arith_at_base(&mut self, p: &mut Path, op: Op, a: Val, b: Val, x: Option<X>) -> Val {
        if op == Op::Div && b == Val::Int(0) && self.calls {
            let c = self.cite_of(x);
            self.error(p, "divide by zero", c, "an error() (script_v5.cpp:994, 1179)", true);
        }
        self.v.arith(op, a, b)
    }

    // --- row 0's starts: recorded, not run ------------------------------------------------------------------

    fn may_stop(&mut self, p: &mut Path, o: &FactOracle, key: Fact, head: &str) {
        let name = slot_name(key);
        for pre in [Prefix::At(name), Prefix::Esc(name)] {
            for k in self.prefixed(p, o, pre) {
                if self.read(p, k, o) != Val::Int(0) {
                    let u = self.v.mk(Reason::Work, format!("{name} restarted by {head}; the new instance is not run"));
                    self.write(p, k, u);
                }
            }
        }
    }

    fn started(&mut self, p: &mut Path, o: &FactOracle, key: Fact, recursive: Option<bool>, head: &str) {
        if recursive != Some(true) {
            self.may_stop(p, o, key, head);
        }
        self.write(p, key, Val::Int(1));
    }

    fn hook(&mut self, p: &mut Path, o: &FactOracle, var: i32, x: X, head: &str) {
        let n = self.read(p, Fact::Var(var), o);
        match n {
            Val::Unk(_) => {
                let c = self.cite(x);
                let w = format!("hook in var {var}: {}", self.show(n));
                p.unknown_writes.push((rc(head), c, w.into()));
            }
            Val::Int(0) => {}
            Val::Int(n) => {
                let key = self.game.script_key(n);
                self.started(p, o, key, Some(false), head);
                if self.is_self(p, key) {
                    let c = self.cite(x);
                    self.end(p, o, "end", rc(&format!("{c} (hook restarts the running script)")), false, None);
                }
            }
        }
    }

    // --- one step -----------------------------------------------------------------------------------------

    fn note(&mut self, p: &mut Path, cite: S, head: &str, what: &str) {
        p.notes.push((rc(head), cite, rc(what)));
    }

    /// An engine error() or warning() (model.py `_error`).
    pub fn error(&mut self, p: &mut Path, what: &str, cite: S, detail: &str, fatal: bool) {
        p.errors.push((rc(what), cite.clone(), rc(detail)));
        if fatal && p.end.is_none() {
            p.end = Some("error");
            p.end_at = Some(rc(&format!("{cite}: {what}")));
        }
        if self.trace.is_some() {
            let kw = vec![("what", enc::s(what)), ("cite", enc::s(&cite)), ("detail", enc::s(detail)), ("fatal", J::Bool(fatal))];
            self.ev(p, "error", kw);
        }
    }

    fn halt(&mut self, p: &mut Path, reason: &'static str, at: S) {
        p.end = Some(reason);
        p.end_at = Some(at.clone());
        if self.trace.is_some() {
            self.ev(p, "halt", vec![("reason", enc::s(reason)), ("at", enc::s(&at))]);
        }
    }

    fn is_self(&self, p: &Path, key: Fact) -> bool {
        let f = p.frame().expect("a frame");
        self.game.lifecycle_key(self.unit_key(f.unit)) == Some(key)
    }

    /// Row 0: end a path (model.py `_end`).
    fn end(&mut self, p: &mut Path, o: &FactOracle, reason: &'static str, at: S, lifecycle: bool, resume: Option<i32>) {
        p.end = Some(reason);
        p.end_at = Some(at.clone());
        if lifecycle && matches!(reason, "yield" | "end" | "room") {
            let unit = self.unit_key(p.frame().expect("a frame").unit);
            let (key, name) = (self.game.lifecycle_key(unit), Game::unit_name(unit));
            self.clear(p, o, Prefix::At(name));
            if reason == "yield" {
                if let Some(k) = key {
                    self.write(p, k, Val::Int(1));
                }
                match resume {
                    None => {
                        let w = format!("{at}: the resume offset is past the code; at: not predicted");
                        self.note(p, rc(""), "yield", &w);
                    }
                    Some(r) => self.write(p, Fact::At(name, r), Val::Int(1)),
                }
            } else {
                if let Some(k) = key {
                    self.write(p, k, Val::Int(0));
                }
                self.clear(p, o, Prefix::Esc(name));
            }
        }
        if self.trace.is_some() {
            let r = resume.map_or(J::Null, |r| J::Int(r as i64));
            self.ev(p, "end", vec![("reason", enc::s(reason)), ("at", enc::s(&at)), ("resume", r)]);
        }
    }

    /// The running frame's turn ends: row 0 ends the path, calls pops the frame (model.py `_done`).
    fn done(&mut self, p: &mut Path, o: &FactOracle, how: &'static str, at: S, resume: Option<i32>) {
        if self.calls {
            self.finish(p, o, how, at, resume);
        } else {
            self.end(p, o, how, at, true, resume);
        }
    }

    /// Runs one instruction of the running frame; returns the paths it forked off (model.py `_step`).
    fn step(&mut self, p: &mut Path, o: &FactOracle) -> Vec<Path> {
        let g = self.game;
        let (uid, pc, sid) = {
            let f = p.frame().expect("a frame");
            (f.unit, f.pc, f.sid)
        };
        let u = g.unit(uid);
        let idx = match pc {
            Some(i) if i < u.ins_len => i,
            _ => {
                let at = format!("{}: end of code", g.syms.str(u.file));
                self.done(p, o, "end", rc(&at), None);
                return Vec::new();
            }
        };
        let x = X { u: uid, i: idx };
        let ins = &g.ins(uid)[idx as usize];
        p.steps += 1;
        let Some(st) = ins.st else {
            let c = self.cite(x);
            p.unmodelled.push((rc("unparsed"), c.clone(), rc(g.text(ins))));
            self.end(p, o, "unmodelled", c, false, None);
            return Vec::new();
        };
        p.frame_mut().unwrap().pc = ins.next;
        let mut more = Vec::new();
        match st {
            Stmt::Jump { cond, .. } => return self.jump(p, o, x, cond, Some(idx)),
            Stmt::Assign(a) => self.assign(p, o, a, Some(x)),
            Stmt::Start { .. } => more = self.start(p, o, x, st),
            Stmt::Op { name, .. } => {
                let eff = effect(name);
                match eff {
                    Effect::Yield => {
                        // after breakHere or a delay the slot resumes at the next instruction (facts.py at:)
                        let resume = ins.next.filter(|&j| j < u.ins_len).map(|j| g.ins(uid)[j as usize].offset);
                        let c = self.cite(x);
                        self.done(p, o, "yield", c, resume);
                    }
                    Effect::Wait => more = self.wait(p, o, x, name),
                    Effect::Unmodelled(why) => {
                        let c = self.cite(x);
                        p.unmodelled.push((rc(self.sym(name)), c.clone(), rc(why)));
                        self.end(p, o, "unmodelled", c, false, None);
                    }
                    Effect::Write | Effect::End | Effect::Deferred => more = self.op(p, o, x, st),
                    Effect::None => {}
                }
                if eff == Effect::End {
                    let c = self.cite(x);
                    let calls = self.calls;
                    let goes = |q: &Path| if calls { q.running(sid) } else { q.end.is_none() };
                    if goes(p) {
                        self.done(p, o, "end", c.clone(), None);
                    }
                    for q in more.iter_mut() {
                        if goes(q) {
                            self.done(q, o, "end", c.clone(), None);
                        }
                    }
                }
            }
        }
        if self.calls {
            // an op that stopped its own slot ends its turn (stopScript and the like)
            self.finish_killed(p, o, sid, x);
            for q in more.iter_mut() {
                self.finish_killed(q, o, sid, x);
            }
        }
        more
    }

    /// `_step`'s last test (calls): the frame `sid`, still running, was killed by its own op.
    fn finish_killed(&mut self, q: &mut Path, o: &FactOracle, sid: u32, x: X) {
        if !q.running(sid) {
            return;
        }
        let Some(Entry::Frame(f)) = q.stack.last() else { unreachable!() };
        if let Some(how) = f.killed {
            let c = self.cite(x);
            self.finish(q, o, how, c, None);
        }
    }

    pub fn jump_base(&mut self, p: &mut Path, o: &FactOracle, x: X, cond: Option<ExprId>, idx: Option<u32>) -> Vec<Path> {
        let g = self.game;
        let ins = &g.ins(x.u)[x.i as usize];
        let Some(cond) = cond else {
            p.frame_mut().unwrap().pc = ins.target;
            return Vec::new();
        };
        let c = self.test(p, o, cond, Some(x));
        if let (Dec::Unk(u), Some(idx)) = (c, idx) {
            if p.round != 0 || p.timed {
                if let Some(q) = self.time_jump(p, x, cond, idx, u) {
                    return q;
                }
            }
        }
        match c {
            // o5 jumps when the condition is false
            Dec::Bool(b) => {
                p.frame_mut().unwrap().pc = if b { ins.next } else { ins.target };
                Vec::new()
            }
            Dec::Unk(u) => {
                let mut q = p.fork();
                let (cite, cond_s, why) = (self.cite(x), self.cond_text(cond), self.v.show_rc(u));
                p.assumptions.push(Assumption { cite: cite.clone(), cond: cond_s.clone(), side: Side::Bool(true), why: why.clone() });
                q.assumptions.push(Assumption { cite, cond: cond_s, side: Side::Bool(false), why });
                p.frame_mut().unwrap().pc = ins.next;
                q.frame_mut().unwrap().pc = ins.target;
                if self.switches.value_sets {
                    self.narrow(p, &mut q, cond, u);
                }
                vec![q]
            }
        }
    }

    /// F12: a fork on a test of a variable or local that holds a range or a value set (against a constant, or a
    /// zero test) leaves it on each side with the values that side allows, so a later test on it in the same path
    /// is decided (a comparison chain, a range split at its bound). A variable the path has not written is not
    /// narrowed (it is the oracle's, or the reader's fact, forked on read). Only the value the test decided on
    /// (`u`) is narrowed, and only model.py's own unknowns (class 0): not a hook's (a reader step counter, a
    /// conditional write or a copy), whose value the location does not hold as such.
    fn narrow(&mut self, yes: &mut Path, no: &mut Path, cond: ExprId, u: Val) {
        let g = self.game;
        use scumm_ir::CmpOp::*;
        let cmp = |op: scumm_ir::CmpOp, y: i32, z: i32| {
            let (y, z) = (i16w(y), i16w(z));
            match op {
                Eq => y == z,
                Ne => y != z,
                Lt => y < z,
                Le => y <= z,
                Gt => y > z,
                Ge => y >= z,
            }
        };
        // (the location, the test on its value)
        let (loc, test): (ExprId, Box<dyn Fn(i32) -> bool>) = match g.ast.expr(cond) {
            Expr::Cmp { op, a, b } => match (g.ast.constant(a), g.ast.constant(b)) {
                (None, Some(c)) => (a, Box::new(move |v| cmp(op, v, c))),
                (Some(c), None) => (b, Box::new(move |v| cmp(op, c, v))),
                _ => return,
            },
            Expr::Not(a) => (a, Box::new(|v| v == 0)),
            Expr::Truth(a) => (a, Box::new(|v| v != 0)),
            _ => return,
        };
        let get = |p: &Path| -> Option<Val> {
            match g.ast.expr(loc) {
                Expr::Var(n) => p.writes.get(&Fact::Var(n)).copied(),
                Expr::Local(n) => p.frame().and_then(|f| f.local(n)),
                _ => None,
            }
        };
        let Some(v) = get(yes) else { return };
        if v != u || self.v.unk(v).is_none_or(|x| x.class != 0) {
            return;
        }
        if self.v.range(v).is_none() && self.v.values_of(v).is_none_or(|vs| vs.len() < 2) {
            return;
        }
        for (side, q) in [(true, yes), (false, no)] {
            if get(q) != Some(v) {
                continue;
            }
            let Some(nv) = self.v.restrict(v, |x| test(x) == side) else { continue };
            match g.ast.expr(loc) {
                Expr::Var(n) => q.writes.insert(Fact::Var(n), nv),
                Expr::Local(n) => q.frame_mut().unwrap().set_local(n, nv),
                _ => {}
            }
        }
    }

    /// settle's `_jump` (a settle round or a runAllScripts turn, the condition not known): a time-only loop the
    /// slot waited in is taken to exit, once per loop and turn; a loop exit on the camera position forks once, and
    /// stays on the side that stayed. None: the usual fork.
    fn time_jump(&mut self, p: &mut Path, x: X, cond: ExprId, idx: u32, c: Val) -> Option<Vec<Path>> {
        let g = self.game;
        let cp = self.switches.const_polls;
        let ins = &g.ins(x.u)[x.i as usize];
        let fi = p.top_frame_index().expect("a frame");
        let (unit, has_loops) = {
            let f = p.frame_at(fi);
            (f.unit, !f.time_loops.is_empty())
        };
        let reason = self.v.unk(c).expect("an unknown").reason;
        if has_loops {
            if let Some((out, head)) = g.time_exit(unit, idx, false, cp) {
                let f = p.frame_at(fi);
                if f.time_loops.contains(&head) {
                    f.time_loops.retain(|&h| h != head);
                    f.pc = Some(out);
                    let cond_s = self.show_e(cond);
                    let why = rc(&format!("?({}: {}; {cond_s})", Reason::LoopExit.as_str(), reason.as_str()));
                    let cite = self.cite(x);
                    p.assumptions.push(Assumption { cite, cond: rc(&cond_s), side: Side::Exit, why });
                    return Some(Vec::new());
                }
            }
        }
        let (out, head) = g.time_exit(unit, idx, true, cp)?;
        let stay = if Some(out) == ins.next { ins.target } else { ins.next };
        let key = (Game::unit_name(self.unit_key(unit)), head);
        let cond_s = self.show_e(cond);
        let cite = self.cite(x);
        let cam = Reason::Camera.as_str();
        if p.camera_done.contains(&key) {
            let why = rc(&format!("?({cam}: stays, forked once; {cond_s})"));
            p.assumptions.push(Assumption { cite, cond: rc(&cond_s), side: Side::Stay, why });
            p.frame_at(fi).pc = stay;
            return Some(Vec::new());
        }
        // the fork comes before the camera exit is recorded: the side that leaves does not hold it
        let mut q = p.fork();
        p.camera_done.insert(key);
        let why = rc(&format!("?({cam}: {}; {cond_s})", reason.as_str()));
        let cond_r = rc(&cond_s);
        p.assumptions.push(Assumption { cite: cite.clone(), cond: cond_r.clone(), side: Side::Stay, why: why.clone() });
        q.assumptions.push(Assumption { cite, cond: cond_r, side: Side::Exit, why });
        p.frame_at(fi).pc = stay;
        q.frame_mut().unwrap().pc = Some(out);
        Some(vec![q])
    }

    /// o5_wait: blocked or not, a fork (model.py `_wait`); in a settle round or turn, a wait for an actor, a
    /// message or the camera blocks now, and WaitForSentence tests the queue and the sentence slot.
    pub fn wait_base(&mut self, p: &mut Path, o: &FactOracle, x: X, name: Sym) -> Vec<Path> {
        let g = self.game;
        let (reason, what) = waits(name);
        let ins = &g.ins(x.u)[x.i as usize];
        if p.round != 0 || p.timed {
            if name != known::WAIT_FOR_SENTENCE {
                let c = self.cite(x);
                self.done(p, o, "yield", c, Some(ins.offset));
                return Vec::new();
            }
            if let Dec::Bool(busy) = self.sentence_busy(p, o) {
                if busy {
                    let c = self.cite(x);
                    self.done(p, o, "yield", c, Some(ins.offset));
                }
                return Vec::new();
            }
        }
        let mut q = p.fork();
        let cond = rc(&format!("{} blocks", scumm_ir::py::strip(g.text(ins))));
        let why = rc(&format!("?({}: {what})", reason.as_str()));
        let cite = self.cite(x);
        p.assumptions.push(Assumption { cite: cite.clone(), cond: cond.clone(), side: Side::Bool(true), why: why.clone() });
        q.assumptions.push(Assumption { cite: cite.clone(), cond, side: Side::Bool(false), why });
        self.done(p, o, "yield", cite, Some(ins.offset));
        vec![q]
    }

    // --- starts ---------------------------------------------------------------------------------------------

    fn start(&mut self, p: &mut Path, o: &FactOracle, x: X, st: Stmt) -> Vec<Path> {
        let g = self.game;
        let Stmt::Start { name, script, obj, recursive, freeze, args } = st else { unreachable!() };
        let head = self.sym(name);
        if self.calls {
            // getWordVararg: the call site's arguments become the callee's locals, evaluated first
            let mut locals: Args = Vec::new();
            for (i, &a) in g.ast.kids(args).iter().enumerate() {
                let v = self.val(p, o, a, Some(x));
                locals.push((i as i32, v));
            }
            let c = self.cite(x);
            if name == known::START_OBJECT {
                let ob = self.val(p, o, obj.expect("startObject's object"), Some(x));
                let verb = self.val(p, o, script, Some(x));
                let (Val::Int(ob), Val::Int(verb)) = (ob, verb) else {
                    let w = format!("objscript of {} for verb {}", self.show(ob), self.show(verb));
                    p.unknown_writes.push((rc(head), c, w.into()));
                    return Vec::new();
                };
                self.push_call(p, rc(head), c, vec![Step::Object { obj: ob, verb, args: locals }], true);
                return Vec::new();
            }
            let n = self.val(p, o, script, Some(x));
            let Val::Int(n) = n else {
                let w = format!("script {}", self.show(n));
                p.unknown_writes.push((rc(head), c, w.into()));
                return Vec::new();
            };
            if name == known::CHAIN_SCRIPT {
                // the running slot dies first and _currentScript is 0xFF, then runScript(N) with that slot's
                // F and R (script_v5.cpp:843-850): N takes its place under the same caller
                let fi = p.top_frame_index().expect("a frame");
                let f = p.frame_at(fi);
                f.killed = Some("chained");
                let (unit, resistant, rec) = (f.unit, f.resistant, f.recursive);
                self.gone(p, o, unit);
                let step = Step::Script { n, resistant, recursive: rec, args: locals };
                self.push_call(p, rc(head), c, vec![step], false);
                return Vec::new();
            }
            let step = Step::Script { n, resistant: Some(freeze), recursive: Some(recursive), args: locals };
            self.push_call(p, rc(head), c, vec![step], true);
            return Vec::new();
        }
        if name == known::START_OBJECT {
            let ob = self.val(p, o, obj.expect("startObject's object"), Some(x));
            let verb = self.val(p, o, script, Some(x));
            let (Val::Int(ob), Val::Int(verb)) = (ob, verb) else {
                let c = self.cite(x);
                let w = format!("objscript of {} for verb {}", self.show(ob), self.show(verb));
                p.unknown_writes.push((rc(head), c, w.into()));
                return Vec::new();
            };
            // runObjectScript(O, verb, 0, 0), never with R: it stops O's scripts first, then starts a new
            // instance when O is in the room or inventory and has an entry for the verb
            let key = Fact::ObjScript(ob);
            match self.where_(p, o, ob) {
                Where::Unk(w) => {
                    self.may_stop(p, o, key, head);
                    self.write(p, key, w);
                }
                w if w != Where::Nowhere && g.entry(ob, verb).is_some() => self.started(p, o, key, Some(false), head),
                _ => self.stopped(p, o, key),
            }
            if self.is_self(p, key) {
                let c = self.cite(x);
                let at = format!("{c} (startObject stops the running object's scripts)");
                self.end(p, o, "end", rc(&at), false, None);
            }
            return Vec::new();
        }
        let n = self.val(p, o, script, Some(x));
        let n = match n {
            Val::Unk(_) => {
                let c = self.cite(x);
                let w = format!("script {}", self.show(n));
                p.unknown_writes.push((rc(head), c, w.into()));
                return Vec::new();
            }
            Val::Int(0) => return Vec::new(), // runScript(0) does nothing (script.cpp:62-63)
            Val::Int(n) => n,
        };
        let key = g.script_key(n);
        if name == known::CHAIN_SCRIPT {
            // the running slot dies first, then runScript with the slot's own R flag (script_v5.cpp:834-851)
            let rec = p.frame().unwrap().recursive;
            self.started(p, o, key, rec, head);
            let lifecycle = !self.is_self(p, key);
            let c = self.cite(x);
            self.end(p, o, "end", c, lifecycle, None);
            return Vec::new();
        }
        self.started(p, o, key, Some(recursive), head);
        if !recursive && self.is_self(p, key) {
            let c = self.cite(x);
            self.end(p, o, "end", rc(&format!("{c} (the start stops the running script)")), false, None);
        }
        Vec::new()
    }

    // --- op rules (model.py HEADS has the cites) -------------------------------------------------------------

    pub fn op_base(&mut self, p: &mut Path, o: &FactOracle, x: X, st: Stmt) -> Vec<Path> {
        let g = self.game;
        let Stmt::Op { name, args, subops, flags } = st else { unreachable!() };
        let args = g.ast.kids(args);
        let cite = self.cite(x);
        let int0 = Val::Int(0);
        match name {
            known::STOP_OBJECT_CODE => {} // the "end" effect ends the turn
            known::STOP_SCRIPT => {
                let n = self.val(p, o, args[0], Some(x));
                match n {
                    Val::Unk(_) => {
                        let w = format!("script {}", self.show(n));
                        p.unknown_writes.push((rc("stopScript"), cite, w.into()));
                    }
                    Val::Int(0) => self.done(p, o, "end", cite, None), // stopObjectCode (script_v5.cpp:3035-3036)
                    Val::Int(n) if self.calls => self.stop_script(p, o, n, &cite, "stopScript"),
                    Val::Int(n) => {
                        let key = g.script_key(n);
                        self.stopped(p, o, key);
                        if self.is_self(p, key) {
                            self.end(p, o, "end", cite, false, None);
                        }
                    }
                }
            }
            known::STOP_OBJECT_SCRIPT => {
                let ob = self.val(p, o, args[0], Some(x));
                let Val::Int(ob) = ob else {
                    let w = format!("objscript {}", self.show(ob));
                    p.unknown_writes.push((rc("stopObjectScript"), cite, w.into()));
                    return Vec::new();
                };
                if self.calls {
                    self.kill_object(p, o, ob, &cite, "stopObjectScript");
                    return Vec::new();
                }
                self.stopped(p, o, Fact::ObjScript(ob));
                if self.is_self(p, Fact::ObjScript(ob)) {
                    self.end(p, o, "end", cite, false, None);
                }
            }
            known::DO_SENTENCE => {
                if g.ast.flags(flags).contains(&known::STOP) {
                    // stops VAR_SENTENCE_SCRIPT (script_v5.cpp:1004-1008)
                    if p.settle {
                        // it also empties the sentence queue (_sentenceNum = 0, script_v5.cpp:1005)
                        p.queue.clear();
                    }
                    if p.settle && p.round == 0 {
                        // and clearClickedStatus wipes the frame's key or click (read by row 5's Esc key)
                        p.cleared = Some(true);
                    }
                    let n = self.read(p, Fact::Var(VAR_SENTENCE_SCRIPT), o);
                    match n {
                        Val::Unk(_) => {
                            let w = format!("sentence script {}", self.show(n));
                            p.unknown_writes.push((rc("doSentence"), cite, w.into()));
                        }
                        Val::Int(0) => {}
                        Val::Int(n) if self.calls => self.stop_script(p, o, n, &cite, "doSentence"),
                        Val::Int(n) => {
                            let key = g.script_key(n);
                            self.stopped(p, o, key);
                            if self.is_self(p, key) {
                                self.end(p, o, "end", cite, false, None);
                            }
                        }
                    }
                    return Vec::new();
                }
                if p.settle {
                    // doSentence (script.cpp:1145-1172) pushes (verb, A, B) with freeze count 0; v5 drops no
                    // duplicate. checkAndRunSentenceScript pops the top at the end of a later frame
                    let mut vals = Vec::with_capacity(args.len());
                    for &a in args {
                        vals.push(self.val(p, o, a, Some(x)));
                    }
                    match vals.as_slice() {
                        &[Val::Int(v), Val::Int(a), Val::Int(b)] => {
                            p.queue.push((v, a, b, 0));
                            self.note(p, cite, "doSentence", "sentence queued; settle runs it at a round's end");
                        }
                        _ => {
                            let strs: Vec<String> = vals.iter().map(|&v| self.show(v)).collect();
                            let refs: Vec<&str> = strs.iter().map(|s| s.as_str()).collect();
                            let w = format!("sentence {}", scumm_ir::py::repr_list(&refs));
                            p.unknown_writes.push((rc("doSentence"), cite.clone(), w.into()));
                            self.note(p, cite, "doSentence", "a sentence with an unknown verb or object is not queued");
                        }
                    }
                    return Vec::new();
                }
                self.note(
                    p,
                    cite,
                    "doSentence",
                    "sentence queued; VAR_SENTENCE_SCRIPT runs it at the end of a later frame (not run at this row)",
                );
            }
            known::SET_STATE => {
                let ob = self.val(p, o, args[0], Some(x));
                let s = self.val(p, o, args[1], Some(x));
                assert_eq!(args.len(), 2, "setState takes two values");
                let t = match ob {
                    Val::Int(n) => Target::Fact(Fact::State(n)),
                    Val::Unk(_) => {
                        let w = format!("object {}", self.show(ob));
                        Target::Unk(self.v.mk(Reason::Work, w))
                    }
                };
                self.put(p, t, s, Some(x), "setState");
            }
            known::SET_CLASS => {
                let ob = self.val(p, o, args[0], Some(x));
                let Expr::List(items) = g.ast.expr(args[1]) else { panic!("setClass without a class list") };
                for &c in g.ast.kids(items) {
                    let cv = self.val(p, o, c, Some(x));
                    match (ob, cv) {
                        (Val::Int(ob), Val::Int(cv)) => {
                            if cv == 0 {
                                // class 0 clears all class data (script_v5.cpp:679-681), classes 1-32
                                for k in 1..=32 {
                                    self.write(p, Fact::Class(ob, k), int0);
                                }
                            } else {
                                self.write(p, Fact::Class(ob, cv & 0x7F), Val::Int((cv & 0x80 != 0) as i32));
                            }
                        }
                        _ => {
                            let w = format!("class {} of {}", self.show(cv), self.show(ob));
                            p.unknown_writes.push((rc("setClass"), cite.clone(), w.into()));
                        }
                    }
                }
            }
            known::SET_OWNER_OF => return self.op_set_owner_of(p, o, x, args),
            known::PICKUP_OBJECT => {
                let ob = self.val(p, o, args[0], Some(x));
                match ob {
                    Val::Unk(_) => {
                        let w = format!("object {}", self.show(ob));
                        p.unknown_writes.push((rc("pickupObject"), cite, w.into()));
                    }
                    Val::Int(ob) => {
                        let ego = self.read(p, Fact::Var(VAR_EGO), o);
                        self.write(p, Fact::Owner(ob), ego);
                        self.write(p, Fact::Class(ob, CLASS_UNTOUCHABLE), Val::Int(1));
                        self.write(p, Fact::State(ob), Val::Int(1));
                    }
                }
                self.inventory_hook(p, o, x, 1, "pickupObject"); // runInventoryScript(1), script_v5.cpp:2033
            }
            known::PRINT => {
                // decodeParseString prints a Text sub-op in slot 3 for actor 252: a message box whose key
                // press goes to VAR_KEYPRESS, var 0
                if !g.ast.subs(subops).iter().any(|s| s.name == known::TEXT) {
                    return Vec::new();
                }
                let actor = self.val(p, o, args[0], Some(x));
                match actor {
                    Val::Unk(_) => {
                        let w = format!("actor {}: if {MESSAGE_BOX_ACTOR}, var 0 gets the key press", self.show(actor));
                        self.note(p, cite, "print", &w);
                    }
                    Val::Int(MESSAGE_BOX_ACTOR) => {
                        let u = self.v.mk(Reason::Key, "message box");
                        self.write(p, Fact::Var(VAR_KEYPRESS), u);
                    }
                    Val::Int(_) => {}
                }
            }
            known::SET_OBJECT_NAME => self.inventory_hook(p, o, x, 0, "setObjectName"),
            known::DRAW_OBJECT => {
                let ob = self.val(p, o, args[0], Some(x));
                let mut state = Val::Int(1);
                for s in g.ast.subs(subops) {
                    if s.name == known::SET_IMAGE {
                        state = self.val(p, o, g.ast.kids(s.args)[0], Some(x));
                    }
                }
                let Val::Int(ob) = ob else {
                    let w = format!("object {}", self.show(ob));
                    p.unknown_writes.push((rc("drawObject"), cite, w.into()));
                    return Vec::new();
                };
                let room = self.read(p, Fact::Room, o);
                match self.resource(room) {
                    res @ Val::Unk(_) => {
                        let u = self.v.mk(Reason::Work, format!("room {}", self.show(res)));
                        self.write(p, Fact::State(ob), u);
                    }
                    Val::Int(res) => {
                        if g.object_room.get(&ob) == Some(&res) {
                            // getObjectIndex finds only the room's objects
                            self.write(p, Fact::State(ob), state);
                            self.note(p, cite, "drawObject", "objects with the same position and size go to state 0 (not modelled)");
                        }
                    }
                }
            }
            known::VERB_OPS => {
                let v = self.val(p, o, args[0], Some(x));
                let Val::Int(v) = v else {
                    let w = format!("verb {}", self.show(v));
                    p.unknown_writes.push((rc("VerbOps"), cite, w.into()));
                    return Vec::new();
                };
                let key = Fact::Verb(v);
                let now = self.read(p, key, o);
                // a shown verb has a saveid-0 slot; a hidden one may have none
                let (mut shown, mut slot, mut changed) = (now, now == Val::Int(1), false);
                for s in g.ast.subs(subops) {
                    match s.name {
                        known::NEW => (shown, slot, changed) = (int0, true, true),
                        known::ON => {
                            shown = if slot {
                                Val::Int(1)
                            } else {
                                self.v.mk(Reason::Verb, format!("verb {v}: no shown slot, may have a hidden one"))
                            };
                            changed = true;
                        }
                        known::OFF | known::DIM | known::DELETE => {
                            (shown, changed) = (int0, true);
                            slot = slot && s.name != known::DELETE;
                        }
                        _ => {}
                    }
                }
                if changed && (shown.is_unknown() || shown != now) {
                    self.write(p, key, shown);
                }
            }
            known::SAVE_VERBS | known::RESTORE_VERBS => {
                let a = self.val(p, o, args[0], Some(x));
                let b = self.val(p, o, args[1], Some(x));
                let c = self.val(p, o, args[2], Some(x));
                assert_eq!(args.len(), 3, "saveVerbs and restoreVerbs take three values");
                let head = self.sym(name);
                let (Val::Int(a), Val::Int(b)) = (a, b) else {
                    let w = format!("verbs {}..{}", self.show(a), self.show(b));
                    p.unknown_writes.push((rc(head), cite, w.into()));
                    return Vec::new();
                };
                for v in a..=b {
                    if name == known::SAVE_VERBS {
                        if self.read(p, Fact::Verb(v), o) != int0 {
                            self.write(p, Fact::Verb(v), int0);
                        }
                    } else {
                        let u = self.v.mk(Reason::Verb, format!("verb {v} restored from save id {}", self.show(c)));
                        self.write(p, Fact::Verb(v), u);
                    }
                }
            }
            known::SET_VAR_RANGE => {
                // getResultPos, then setResult and _resultVarNumber++ per value: the next variable, bit or local
                let t = self.target(p, o, args[0], Some(x));
                let Expr::List(items) = g.ast.expr(args[2]) else { panic!("setVarRange without a value list") };
                for (k, &e) in g.ast.kids(items).iter().enumerate() {
                    let v = self.val(p, o, e, Some(x));
                    let k = k as i64;
                    let tk = match t {
                        Target::Unk(_) => t,
                        Target::Local(n) => Target::Local(key_num(n as i64 + k)),
                        Target::Fact(Fact::Var(n)) => Target::Fact(Fact::Var(key_num(n as i64 + k))),
                        Target::Fact(Fact::Bit(n)) => Target::Fact(Fact::Bit(key_num(n as i64 + k))),
                        Target::Fact(f) => unreachable!("setVarRange to {f}"),
                    };
                    self.put(p, tk, v, Some(x), "setVarRange");
                }
            }
            known::LOAD_ROOM => {
                let room = self.val(p, o, args[0], Some(x));
                if self.calls {
                    // o5_loadRoom: startScene(room, nullptr, 0) (script_v5.cpp:1849-1850)
                    let steps = self.scene_steps(room, None);
                    self.push_call(p, rc("loadRoom"), cite, steps, true);
                    return Vec::new();
                }
                self.room_change(p, o, room, x, "loadRoom");
            }
            known::LOAD_ROOM_WITH_EGO => {
                let obj = self.val(p, o, args[0], Some(x));
                let room = self.val(p, o, args[1], Some(x));
                if self.calls {
                    // o5_loadRoomWithEgo (script_v5.cpp:1857-1902): VAR_WALKTO_OBJ = obj, startScene(room, ego,
                    // obj), VAR_WALKTO_OBJ = 0 (room_change), then setCameraFollows(ego): a possible setCameraAt
                    // (the scroll hook) and the inventory hook (camera.cpp:62-86)
                    let mut steps = if self.rooms {
                        let mut s = vec![Step::Write(Fact::Var(VAR_WALKTO_OBJ), obj)];
                        s.extend(self.scene_steps(room, Some(obj)));
                        s.push(Step::Write(Fact::Var(VAR_WALKTO_OBJ), int0));
                        s
                    } else {
                        self.scene_steps(room, None)
                    };
                    steps.push(Step::ScrollNote);
                    steps.push(Step::Hook { var: VAR_INVENTORY_SCRIPT, args: vec![(0, int0)] });
                    self.push_call(p, rc("loadRoomWithEgo"), cite, steps, true);
                    return Vec::new();
                }
                self.room_change(p, o, room, x, "loadRoomWithEgo");
                self.hook(p, o, VAR_INVENTORY_SCRIPT, x, "loadRoomWithEgo");
                if p.end.is_none() {
                    self.note(p, cite, "loadRoomWithEgo", "the scroll hook may run (camera position, not a fact)");
                }
            }
            known::ACTOR_FOLLOW_CAMERA => {
                let a = self.val(p, o, args[0], Some(x));
                let mut q = p.fork();
                let what = format!("room of actor {}", self.show(a));
                let why = rc(&format!("?(actor state: {what})"));
                let cond = rc(&format!("actor {} in the current room", self.show_e(args[0])));
                p.assumptions.push(Assumption { cite: cite.clone(), cond: cond.clone(), side: Side::Bool(true), why: why.clone() });
                q.assumptions.push(Assumption { cite: cite.clone(), cond, side: Side::Bool(false), why });
                if self.calls {
                    // actorFollowCamera (camera.cpp:201-212): setCameraFollows (camera.cpp:62-86) changes the room
                    // when the actor is elsewhere and then always calls setCameraAt, so the scroll hook runs
                    // (camera.cpp:69-74, 52-55); it may call setCameraAt again (a position: a note), runs the
                    // inventory hook; a second inventory hook run when the followed actor changed is a note
                    let tail = [
                        Step::ScrollNote,
                        Step::Hook { var: VAR_INVENTORY_SCRIPT, args: vec![(0, int0)] },
                        Step::Note(rc("a second inventory hook run when the followed actor changed (camera.cpp:207-208) \
                                       is not run: camera._follows is not a fact")),
                    ];
                    self.push_call(p, rc("actorFollowCamera"), cite.clone(), tail.to_vec(), true);
                    let r = self.v.mk(Reason::Actor, what);
                    let mut steps = self.scene_steps(r, None);
                    steps.push(Step::Hook { var: VAR_SCROLL_SCRIPT, args: Vec::new() });
                    steps.extend(tail);
                    self.push_call(&mut q, rc("actorFollowCamera"), cite, steps, true);
                    return vec![q];
                }
                // setCameraFollows runs the inventory hook last, also after a room change (camera.cpp:61-86)
                self.hook(p, o, VAR_INVENTORY_SCRIPT, x, "actorFollowCamera");
                self.hook(&mut q, o, VAR_INVENTORY_SCRIPT, x, "actorFollowCamera");
                if q.end.is_none() {
                    let r = self.v.mk(Reason::Actor, what);
                    self.room_change(&mut q, o, r, x, "actorFollowCamera");
                }
                return vec![q];
            }
            known::SET_CAMERA_AT => {
                if self.calls {
                    // setCameraAt runs the scroll hook with no arguments (camera.cpp:52-55)
                    let steps = vec![Step::Hook { var: VAR_SCROLL_SCRIPT, args: Vec::new() }];
                    self.push_call(p, rc("setCameraAt"), cite, steps, true);
                    return Vec::new();
                }
                self.hook(p, o, VAR_SCROLL_SCRIPT, x, "setCameraAt");
            }
            known::FREEZE_SCRIPTS => {
                // o5_freezeScripts (script_v5.cpp:1252-1259): a flag not 0 freezes, 0 unfreezes (calls only; row 0
                // has no freeze counts)
                if self.calls {
                    let flag = self.val(p, o, args[0], Some(x));
                    match flag {
                        Val::Unk(_) => {
                            let w = format!("flag {}", self.show(flag));
                            p.unknown_writes.push((rc("freezeScripts"), cite, w.into()));
                        }
                        Val::Int(0) => self.unfreeze(p, o),
                        Val::Int(f) => self.freeze(p, o, f, x),
                    }
                }
            }
            known::CUTSCENE => {
                let c = self.read(p, Fact::Cutscene, o);
                let c = self.v.arith(Op::Add, c, Val::Int(1));
                self.write(p, Fact::Cutscene, c);
                self.clear(p, o, Prefix::EscAll); // the new level has no override (script.cpp:1641-1642)
                if self.calls {
                    // beginCutscene (script.cpp:1632-1648): the slot's cutsceneOverride + 1, the level keeps
                    // args[0], and the start hook runs with the cutscene's list while vm.cutSceneScriptIndex is
                    // this slot
                    let mut vals: Args = Vec::new();
                    if let Some(&a0) = args.first() {
                        if let Expr::List(items) = g.ast.expr(a0) {
                            for (i, &e) in g.ast.kids(items).iter().enumerate() {
                                let v = self.val(p, o, e, Some(x));
                                vals.push((i as i32, v));
                            }
                        }
                    }
                    let fi = p.top_frame_index().expect("a frame");
                    let f = p.frame_at(fi);
                    f.override_ += 1;
                    let sid = f.sid;
                    p.cut_data.push(vals.first().map_or(int0, |a| a.1));
                    let steps = vec![
                        Step::CutIndex(Some(sid)),
                        Step::Hook { var: VAR_CUTSCENE_START_SCRIPT, args: vals },
                        Step::CutIndex(None),
                    ];
                    self.push_call(p, rc("cutscene"), cite, steps, true);
                    return Vec::new();
                }
                self.hook(p, o, VAR_CUTSCENE_START_SCRIPT, x, "cutscene");
            }
            known::END_CUTSCENE => {
                let depth = self.read(p, Fact::Cutscene, o);
                // `armed` reads the esc: facts only with calls on (model.py's `self.calls and ...`). model.py
                // walks a Python set there (hash order) and stops at the first 1. With no read log (settle's
                // owner) the order changes nothing; in a settle turn the keys read go into the turn's read log,
                // and which ones rests on the set order: the port reads them in key order (PORT-NOTES.md, "Kept
                // apart"; orderdeps.py counts the cases)
                let mut armed = false;
                if self.calls {
                    let mut keys = self.prefixed(p, o, Prefix::EscAll);
                    if p.owner.is_some() {
                        sort_str(&mut keys);
                    }
                    for k in keys {
                        if self.read(p, k, o) == Val::Int(1) {
                            armed = true;
                            break;
                        }
                    }
                }
                self.write(p, Fact::Var(VAR_OVERRIDE), int0);
                let d = self.v.arith(Op::Sub, depth, Val::Int(1));
                self.write(p, Fact::Cutscene, d);
                self.clear(p, o, Prefix::EscAll);
                self.note(p, cite.clone(), "endCutscene", "an override armed on the level below may show again (not a fact)");
                if self.calls {
                    // endCutscene (script.cpp:1650-1687): the end hook gets the closed level's cutSceneData, known
                    // only for a level this path opened; a stack already at 0 is the engine's error()
                    let fi = p.top_frame_index().expect("a frame");
                    let f = p.frame_at(fi);
                    // script.cpp:1654-1664
                    f.override_ = 0.max(f.override_ - 1 - (armed && f.override_ > 1) as i32);
                    if depth == int0 {
                        self.error(p, "cutscene stack underflow", cite, "endCutscene's error() (script.cpp:1668-1681)", true);
                        return Vec::new();
                    }
                    let data = match p.cut_data.pop() {
                        Some(d) => d,
                        None => self.v.mk(Reason::CutData, "the data of a cutscene opened before the move"),
                    };
                    let steps = vec![Step::Hook { var: VAR_CUTSCENE_END_SCRIPT, args: vec![(0, data)] }];
                    self.push_call(p, rc("endCutscene"), cite, steps, true);
                    return Vec::new();
                }
                self.hook(p, o, VAR_CUTSCENE_END_SCRIPT, x, "endCutscene");
            }
            known::BEGIN_OVERRIDE => {
                self.write(p, Fact::Var(VAR_OVERRIDE), int0);
                self.clear(p, o, Prefix::EscAll); // one override per level: this one replaces it
                match g.ins(x.u)[x.i as usize].esc {
                    Some(e) => {
                        // Esc resumes at the jump after beginOverride (ir.py esc)
                        let unit = p.frame().unwrap().unit;
                        let off = g.ins(unit)[e as usize].offset;
                        let name = Game::unit_name(self.unit_key(unit));
                        self.write(p, Fact::Esc(name, off), Val::Int(1));
                    }
                    None => self.note(p, cite, "beginOverride", "no jump after it (ir.py); esc: not predicted"),
                }
            }
            known::END_OVERRIDE => {
                self.write(p, Fact::Var(VAR_OVERRIDE), int0);
                self.clear(p, o, Prefix::EscAll);
            }
            known::USERPUT_ON => self.write(p, Fact::Var(VAR_USERPUT), Val::Int(1)),
            known::USERPUT_OFF => self.write(p, Fact::Var(VAR_USERPUT), int0),
            known::CURSOR_SHOW => self.write(p, Fact::Var(VAR_CURSORSTATE), Val::Int(1)),
            known::CURSOR_HIDE => self.write(p, Fact::Var(VAR_CURSORSTATE), int0),
            known::USERPUT_SOFT_ON | known::USERPUT_SOFT_OFF | known::CURSOR_SOFT_ON | known::CURSOR_SOFT_OFF => {
                let var = if matches!(name, known::USERPUT_SOFT_ON | known::USERPUT_SOFT_OFF) {
                    VAR_USERPUT
                } else {
                    VAR_CURSORSTATE
                };
                let op = if matches!(name, known::USERPUT_SOFT_ON | known::CURSOR_SOFT_ON) { Op::Add } else { Op::Sub };
                let now = self.read(p, Fact::Var(var), o);
                let v = self.v.arith(op, now, Val::Int(1));
                self.write(p, Fact::Var(var), v);
            }
            known::ROOM_SCROLL => {
                let room = self.read(p, Fact::Room, o);
                let res = self.resource(room);
                let width = res.int().and_then(|r| g.room_width.get(&r).copied());
                for (var, &e) in [VAR_CAMERA_MIN_X, VAR_CAMERA_MAX_X].iter().zip(args) {
                    let mut v = self.val(p, o, e, Some(x));
                    if let Val::Int(n) = v {
                        let n = n.max(SCREEN_WIDTH / 2);
                        v = match width {
                            None => self.v.mk(Reason::Work, format!("width of room {}", self.show(res))),
                            Some(w) => Val::Int(n.min(w - SCREEN_WIDTH / 2)),
                        };
                    }
                    self.write(p, Fact::Var(*var), v);
                }
            }
            _ => unreachable!("no op rule for {}", self.sym(name)),
        }
        Vec::new()
    }

    /// The inventory hook: row 0 records the start, calls runs it with Local[0] = `arg`.
    fn inventory_hook(&mut self, p: &mut Path, o: &FactOracle, x: X, arg: i32, head: &str) {
        if self.calls {
            let c = self.cite(x);
            let steps = vec![Step::Hook { var: VAR_INVENTORY_SCRIPT, args: vec![(0, Val::Int(arg))] }];
            self.push_call(p, rc(head), c, steps, true);
        } else {
            self.hook(p, o, VAR_INVENTORY_SCRIPT, x, head);
        }
    }

    fn op_set_owner_of(&mut self, p: &mut Path, o: &FactOracle, x: X, args: &[ExprId]) -> Vec<Path> {
        let ob = self.val(p, o, args[0], Some(x));
        let owner = self.val(p, o, args[1], Some(x));
        assert_eq!(args.len(), 2, "setOwnerOf takes two values");
        let cite = self.cite(x);
        let Val::Int(ob) = ob else {
            let w = format!("owner of {}", self.show(ob));
            p.unknown_writes.push((rc("setOwnerOf"), cite, w.into()));
            self.inventory_hook(p, o, x, 0, "setOwnerOf");
            return Vec::new();
        };
        let key = Fact::ObjScript(ob);
        if self.calls {
            let mut more = Vec::new();
            if owner == Val::Int(0) {
                // clearOwnerOf stops the object's scripts (object.cpp:183-184)
                self.kill_object(p, o, ob, &cite, "setOwnerOf");
            } else if owner.is_unknown() {
                let ou = self.game.id(UnitKey::Obj(ob));
                if p.frames().any(|f| Some(f.unit) == ou) {
                    let mut q = p.fork();
                    let cond = rc(&format!("owner {} != 0", self.show_e(args[1])));
                    let why = rc(&self.show(owner));
                    p.assumptions.push(Assumption { cite: cite.clone(), cond: cond.clone(), side: Side::Bool(true), why: why.clone() });
                    q.assumptions.push(Assumption { cite: cite.clone(), cond, side: Side::Bool(false), why });
                    self.kill_object(&mut q, o, ob, &cite, "setOwnerOf");
                    q.writes.insert(Fact::Owner(ob), Val::Int(0));
                    self.inventory_hook(&mut q, o, x, 0, "setOwnerOf");
                    more.push(q);
                }
                if self.read(p, key, o) != Val::Int(0) {
                    self.write(p, key, owner);
                }
            }
            self.write(p, Fact::Owner(ob), owner);
            self.inventory_hook(p, o, x, 0, "setOwnerOf");
            return more;
        }
        if owner == Val::Int(0) {
            self.stopped(p, o, key); // clearOwnerOf stops the object's scripts (object.cpp:183-184)
        } else if owner.is_unknown() {
            self.write(p, key, owner);
        }
        self.write(p, Fact::Owner(ob), owner);
        self.hook(p, o, VAR_INVENTORY_SCRIPT, x, "setOwnerOf");
        if p.end.is_none() && self.is_self(p, key) {
            if owner == Val::Int(0) {
                let at = format!("{cite} (setOwnerOf 0 stops the running object)");
                self.end(p, o, "end", rc(&at), false, None);
            } else if owner.is_unknown() {
                let mut q = p.fork();
                let cond = rc(&format!("owner {} != 0", self.show_e(args[1])));
                let why = rc(&self.show(owner));
                p.assumptions.push(Assumption { cite: cite.clone(), cond: cond.clone(), side: Side::Bool(true), why: why.clone() });
                q.assumptions.push(Assumption { cite: cite.clone(), cond, side: Side::Bool(false), why });
                q.writes.insert(key, Val::Int(0));
                self.end(&mut q, o, "end", cite, false, None);
                return vec![q];
            }
        }
        Vec::new()
    }

    /// Row 0: a room change (model.py `_room_change`).
    fn room_change(&mut self, p: &mut Path, o: &FactOracle, room: Val, x: X, head: &str) {
        self.write(p, Fact::Room, room);
        let cite = self.cite(x);
        self.note(
            p,
            cite.clone(),
            head,
            "the room change's kills of other slots and its exit and entry scripts are not run (feature room_change)",
        );
        let f = p.frame().expect("a frame");
        let (key, owned) = (self.unit_key(f.unit), f.owned);
        match (key, owned) {
            (UnitKey::Entry(_) | UnitKey::Exit(_) | UnitKey::Local(..), _) | (UnitKey::Obj(_), Owned::Bool(false)) => {
                self.end(p, o, "room", cite, true, None)
            }
            (UnitKey::Obj(_), Owned::Unk(u)) => {
                let w = format!("slot kind unknown ({}); taken as surviving the room change", self.show(u));
                self.note(p, cite, head, &w);
            }
            _ => {}
        }
    }
}

/// The paths the predictions use (model.py `feasible`).
pub fn feasible(run: &Run) -> impl Iterator<Item = &Path> {
    run.paths.iter().filter(|p| !p.end.is_some_and(|e| INFEASIBLE.contains(&e)))
}

/// model.py `predict(run, pre)`: per fact some feasible path writes, its value on every such path (the
/// pre value where a path does not write it), whether all agree on a known value, and the values seen.
pub fn predict(run: &Run, o: &FactOracle) -> J {
    let v = &run.vals;
    let paths: Vec<&Path> = feasible(run).collect();
    let keys: FxHashSet<Fact> = paths.iter().flat_map(|p| p.writes.keys().copied()).collect();
    let mut out = std::collections::BTreeMap::new();
    for k in keys {
        let before = o.get(k);
        let mut known: Vec<i32> = Vec::new();
        let mut sets: std::collections::BTreeSet<i32> = Default::default();
        let mut n_unknown = 0;
        for p in &paths {
            let x = match p.writes.get(&k) {
                Some(&x) => Some(x),
                None => match before {
                    OVal::Int(n) => Some(Val::Int(n)),
                    _ => None, // the oracle's unknown: a plain one
                },
            };
            match x {
                Some(Val::Int(n)) => {
                    known.push(n);
                    sets.insert(n);
                }
                Some(Val::Unk(id)) => {
                    n_unknown += 1;
                    if let Some(vs) = &v.get(id).values {
                        sets.extend(vs.iter().copied());
                    }
                }
                None => n_unknown += 1,
            }
        }
        let certain = n_unknown == 0 && known.iter().all(|&n| n == known[0]);
        let pre = match before {
            OVal::Int(n) => J::Int(n as i64),
            OVal::Held => J::Str(format!("?(not held: {k})")),
            OVal::Timer => J::Str(format!("?(timer: {k})")),
        };
        let mut m = std::collections::BTreeMap::new();
        m.insert("certain".to_string(), J::Bool(certain));
        m.insert("values".to_string(), J::Arr(sets.into_iter().map(|n| J::Int(n as i64)).collect()));
        m.insert("unknown".to_string(), J::Int(n_unknown));
        m.insert("pre".to_string(), pre);
        out.insert(k.to_string(), J::Obj(m));
    }
    J::Obj(out)
}
