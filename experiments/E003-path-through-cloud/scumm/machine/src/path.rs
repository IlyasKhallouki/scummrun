//! Paths (model.py `Frame`, `Call`, `Path`, `Run`).
//!
//! Every field model.py's dataclasses have is here, with its Python meaning, so the golden dump can
//! print each one (`enc.rs`). Strings that paths share (cites, statement texts, notes) are `Rc<str>`:
//! a fork copies the lists as model.py's `Path.fork` does, but not the strings in them.

use crate::plist::PList;
use crate::value::Val;
use rustc_hash::{FxHashMap, FxHashSet};
use scumm_ir::{Fact, Slot, UnitId};
use std::rc::Rc;

/// A shared string.
pub type S = Rc<str>;

/// The unit of check.py's "no effect at this row" path, `Frame(("none",), 0)`.
pub const NONE_UNIT: UnitId = UnitId(u32::MAX);

/// A Python value that is True, False or an unknown (`Frame.owned`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Owned {
    Bool(bool),
    Unk(Val),
}

/// A running script's slot (model.py `Frame`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Frame {
    pub unit: UnitId,
    pub pc: Option<u32>,
    /// Local[n] -> value, sorted by n
    pub locals: Vec<(i32, Val)>,
    /// False: a local the path has not written is unknown
    pub args_known: bool,
    pub owned: Owned,
    /// the slot's R flag; None: not known
    pub recursive: Option<bool>,
    /// the slot's F flag; None: not known
    pub resistant: Option<bool>,
    pub sid: u32,
    pub freeze: i32,
    /// why the slot died while its turn was not over: stopped, chained, room
    pub killed: Option<&'static str>,
    pub override_: i32,
    pub nested: bool,
    /// "path" or "pre" (settle)
    pub origin: &'static str,
    /// settle: heads of the time-only loops it waited in before this turn
    pub time_loops: Vec<u32>,
}

impl Frame {
    /// `Frame(unit, pc, locals, args_known, owned, recursive)` with the other fields at their defaults.
    pub fn new(unit: UnitId, pc: Option<u32>, args_known: bool, owned: Owned, recursive: Option<bool>) -> Frame {
        Frame {
            unit,
            pc,
            locals: Vec::new(),
            args_known,
            owned,
            recursive,
            resistant: Some(false),
            sid: 0,
            freeze: 0,
            killed: None,
            override_: 0,
            nested: true,
            origin: "path",
            time_loops: Vec::new(),
        }
    }

    pub fn local(&self, n: i32) -> Option<Val> {
        self.locals.iter().find(|l| l.0 == n).map(|l| l.1)
    }

    pub fn set_local(&mut self, n: i32, v: Val) {
        match self.locals.binary_search_by_key(&n, |l| l.0) {
            Ok(i) => self.locals[i].1 = v,
            Err(i) => self.locals.insert(i, (n, v)),
        }
    }
}

/// A step's arguments for a script's locals (model.py's `{i: value}` dicts), by local number, sorted.
pub type Args = Vec<(i32, Val)>;

/// One step of a `Call` (model.py's step tuples, by their first element; each runs model.py's
/// `_eng_<name>`). The calls feature builds them; row 0 has none. Encoded as model.py's tuples (`enc.rs`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Step {
    /// ("write", key, value)
    Write(Fact, Val),
    /// ("note", what)
    Note(S),
    /// ("resume", slot fact): runAllScripts' turn of the sentence script's live slot
    Resume(Fact),
    /// ("sentence", verb, A, B): checkAndRunSentenceScript; B None for a walk_to
    Sentence { verb: i32, a: i32, b: Option<i32> },
    /// ("script", N, F flag, R flag, args): runScript
    Script { n: i32, resistant: Option<bool>, recursive: Option<bool>, args: Args },
    /// ("object", O, verb, args): runObjectScript
    Object { obj: i32, verb: i32, args: Args },
    /// ("hook", var, args): runScript(VAR(var), 0, 0, args)
    Hook { var: i32, args: Args },
    /// ("room1", room): a room change without feature room_change
    Room1(Val),
    /// ("scroll_note",)
    ScrollNote,
    /// ("cut_index", sid or None)
    CutIndex(Option<u32>),
    // --- room_change (row 2): startScene's steps
    /// ("scene", room, ego object or None): startScene inside an opcode
    Scene { room: Val, ego: Option<Val> },
    /// ("scene_begin", room): stopTalk, the running slot's end, VAR_NEW_ROOM
    SceneBegin(Val),
    /// ("room_code", "exit" or "entry"): the room's EXCD or ENCD, nested (`entry` true for ENCD)
    RoomCode { entry: bool },
    /// ("scene_kill",): killScriptsAndResources
    SceneKill,
    /// ("scene_load", room): the room, its variables and the camera bounds
    SceneLoad(Val),
    /// ("scene_ego", object): the ego's object must be in the new room
    SceneEgo(Val),
    // --- settle (row 3)
    /// ("turn", unit name): one slot's turn in runAllScripts
    Turn(Slot),
    /// ("sentences",): checkAndRunSentenceScript on the sentence queue at a round's end
    Sentences,
}

/// Engine code between nested script runs (model.py `Call`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Call {
    pub head: S,
    pub cite: S,
    /// shared by forks, as model.py's tuple is
    pub steps: Rc<[Step]>,
    pub caller: Option<u32>,
    pub active: bool,
    pub i: u32,
    /// The call's identity: model.py names a call in a join signature by its steps tuple's `id()`, which
    /// forks share and no other call has in the run (the machine keeps the tuples alive). A number given
    /// once per Call made does the same. Not dumped.
    pub cid: u64,
}

/// One stack entry of a join signature (model.py `_join`): a frame's slot state, or a call's place.
/// A call's head and cite are left out: its `cid` fixes them.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum SigItem {
    F {
        sid: u32,
        unit: UnitId,
        pc: Option<u32>,
        killed: Option<&'static str>,
        freeze: i32,
        override_: i32,
        /// model.py compares `str(owned)`: for interned values, the same test
        owned: Owned,
        recursive: Option<bool>,
        resistant: Option<bool>,
        args_known: bool,
        time_loops: Vec<u32>,
    },
    C { cid: u64, i: u32, active: bool, caller: Option<u32> },
}

/// The settle part of a join signature (model.py `_join`: the round, the kept slots' places, the queue, the
/// ambient loops run, the slots that ran in this round, the turn's owner, the camera exits forked, `timed`). The
/// frozensets are kept sorted, so that equal sets compare equal.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SettleSig {
    pub round: u32,
    /// `_slots_sig(p, full=False)`: (name, unit, pc, origin) per kept slot, in the dict's order
    pub slots: Vec<(Slot, UnitId, Option<u32>, &'static str)>,
    pub queue: Vec<(i32, i32, i32, i32)>,
    pub ambient_done: Vec<(Slot, u32)>,
    pub round_ran: Vec<Slot>,
    pub owner: Option<Owner>,
    pub camera_done: Vec<(Slot, u32)>,
    pub timed: bool,
}

/// A join signature (model.py `_join`'s tuple): paths with one signature have the same stack. A round's start
/// (settle) is `("round", round, position key)`: `round` holds the round and the position key, a hash (model.py
/// compares the hash itself). `h` is a hash of the rest, made once (`Sig::new`): equality tests it first, so the
/// search for a path's waiting group mostly compares one number per group.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sig {
    pub h: u64,
    pub round: Option<(u32, u64)>,
    pub cut_index: Option<u32>,
    pub cut_len: u32,
    pub settle: Option<Box<SettleSig>>,
    pub items: Vec<SigItem>,
}

impl Sig {
    pub fn new(cut_index: Option<u32>, cut_len: u32, settle: Option<Box<SettleSig>>, items: Vec<SigItem>) -> Sig {
        use std::hash::{Hash, Hasher};
        let mut st = rustc_hash::FxHasher::default();
        cut_index.hash(&mut st);
        cut_len.hash(&mut st);
        settle.hash(&mut st);
        items.hash(&mut st);
        Sig { h: fmix(st.finish()), round: None, cut_index, cut_len, settle, items }
    }

    /// `("round", round, position key)`.
    pub fn round(round: u32, pos: u64) -> Sig {
        Sig { h: fmix(fx(&(round, pos)) ^ 0x2545_f491_4f6c_dd1d), round: Some((round, pos)), cut_index: None, cut_len: 0, settle: None, items: Vec::new() }
    }

    pub fn is_round(&self) -> bool {
        self.round.is_some()
    }
}

impl std::hash::Hash for Sig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.h.hash(state);
    }
}

/// A stack entry: a frame or a call.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Entry {
    Frame(Frame),
    Call(Call),
}

/// The side an assumption took: True or False at an unknown condition, "exit" or "stay" at a loop exit
/// settle decides by time.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Side {
    Bool(bool),
    Exit,
    Stay,
}

/// (cite, condition, side taken, why unknown)
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Assumption {
    pub cite: S,
    pub cond: S,
    pub side: Side,
    pub why: S,
}

/// (statement kind or head, cite, text): model.py's `unmodelled`, `notes`, `unknown_writes`, `errors`.
pub type Tri = (S, S, S);

/// Who a settle turn's reads and writes belong to: a slot, or the sentence queue ("sentence").
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Owner {
    Slot(Slot),
    Sentence,
}

/// A 64-bit finalizer (murmur3 fmix64): spreads a hash so that sums of entry hashes do not cancel.
#[inline]
pub fn fmix(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^ (h >> 33)
}

/// A hash of one value, spread (`fmix`), for order-free sums.
#[inline]
pub fn fx<T: std::hash::Hash>(t: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = rustc_hash::FxHasher::default();
    t.hash(&mut h);
    fmix(h.finish())
}

/// A fact map of writes (model.py's `dict` of fact -> value). It reads as the map (`Deref`) and keeps the sum
/// of its entries' hashes as they are written, so that a hash of the whole store (`_spins`) costs nothing.
#[derive(Clone, Debug, Default)]
pub struct Store {
    map: FxHashMap<Fact, Val>,
    sum: u64,
}

impl Store {
    /// `store[k] = v`.
    #[inline]
    pub fn insert(&mut self, k: Fact, v: Val) {
        if let Some(old) = self.map.insert(k, v) {
            self.sum = self.sum.wrapping_sub(fx(&(k, old)));
        }
        self.sum = self.sum.wrapping_add(fx(&(k, v)));
    }

    /// An order-free hash of the entries (the set of items, as model.py's `frozenset(writes.items())`).
    #[inline]
    pub fn hash_sum(&self) -> u64 {
        self.sum
    }
}

impl PartialEq for Store {
    /// model.py's dict equality (`g.writes == base.writes`).
    fn eq(&self, other: &Store) -> bool {
        self.sum == other.sum && self.map == other.map
    }
}

impl std::ops::Deref for Store {
    type Target = FxHashMap<Fact, Val>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

/// model.py `Path.history`: (state hash, writes) at each round's start, a persistent list (newest first) that
/// forks share: model.py copies the list (not the dicts) at every fork.
#[derive(Clone, Debug, Default)]
pub struct History(Option<Rc<HNode>>);

#[derive(Debug)]
pub struct HNode {
    pub h: u64,
    pub writes: Rc<Store>,
    prev: Option<Rc<HNode>>,
    len: u32,
}

impl History {
    pub fn len(&self) -> u32 {
        self.0.as_ref().map_or(0, |n| n.len)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// `history.append((h, writes))`.
    pub fn push(&mut self, h: u64, writes: Rc<Store>) {
        let len = self.len() + 1;
        self.0 = Some(Rc::new(HNode { h, writes, prev: self.0.take(), len }));
    }

    /// `history[-1] = (h, writes)`.
    pub fn set_last(&mut self, h: u64, writes: Rc<Store>) {
        let last = self.0.take().expect("a history entry to replace");
        self.0 = Some(Rc::new(HNode { h, writes, prev: last.prev.clone(), len: last.len }));
    }

    /// The first index whose hash is `h` (model.py `next(i for i, (h, _) in enumerate(history) if h == state)`).
    pub fn find(&self, h: u64) -> Option<usize> {
        let mut out = None;
        let mut cur = self.0.as_deref();
        while let Some(n) = cur {
            if n.h == h {
                out = Some(n.len as usize - 1);
            }
            cur = n.prev.as_deref();
        }
        out
    }

    /// The writes of entries `i..`, oldest first.
    pub fn writes_from(&self, i: usize) -> Vec<Rc<Store>> {
        let mut out = Vec::new();
        let mut cur = self.0.as_deref();
        while let Some(n) = cur {
            if (n.len as usize) <= i {
                break;
            }
            out.push(n.writes.clone());
            cur = n.prev.as_deref();
        }
        out.reverse();
        out
    }
}

/// model.py `Path`. The trace id `tid` is golden.py's (a TraceMachine bookkeeping field); a fork has none.
/// The lists that only grow are `PList`s: a fork shares their frozen part (model.py copies them).
#[derive(Clone, Debug, Default)]
pub struct Path {
    pub stack: Vec<Entry>,
    pub writes: Store,
    pub assumptions: PList<Assumption>,
    pub unmodelled: PList<Tri>,
    pub notes: PList<Tri>,
    pub unknown_writes: PList<Tri>,
    pub end: Option<&'static str>,
    pub end_at: Option<S>,
    pub steps: u32,
    /// unit name -> (count, exact) of slots off the stack whose count the path set; None: not known
    pub freeze: FxHashMap<Slot, Option<(i32, bool)>>,
    pub cut_index: Option<u32>,
    pub cut_data: Vec<Val>,
    pub turns: PList<(Slot, &'static str)>,
    pub errors: PList<Tri>,
    pub depth: u32,
    pub reentries: u32,
    pub next_sid: u32,
    pub last: Option<Box<Frame>>,
    pub merged: u32,
    /// the join it was last merged at (calls; not dumped)
    pub joined: Option<Rc<Sig>>,
    /// states met at jump targets in the running frame's turn (calls; not dumped)
    pub seen: FxHashSet<u64>,
    pub resumed: bool,
    // --- settle
    pub settle: bool,
    pub round: u32,
    pub first_end: Option<&'static str>,
    /// the writes when the immediate part ended (shared by forks: never changed)
    pub immediate: Option<Rc<Store>>,
    pub after: FxHashSet<Fact>,
    /// unit name -> the Frame of a live slot off the stack, in the order they became live
    pub slots: Vec<(Slot, Frame)>,
    /// (verb, A, B, freeze count), last on top
    pub queue: Vec<(i32, i32, i32, i32)>,
    /// the writes when this round began (shared with `history`: never changed)
    pub round_start: Rc<Store>,
    /// (state hash, writes) at each round's start (dumped as its length)
    pub history: History,
    pub round_ran: FxHashSet<Slot>,
    pub ambient_done: FxHashSet<(Slot, u32)>,
    pub owner: Option<Owner>,
    pub owner_rule: S,
    pub writer: FxHashMap<Fact, S>,
    /// owner -> (keys read before it wrote them, keys written), this round
    pub rw: FxHashMap<Owner, (FxHashSet<Fact>, FxHashSet<Fact>)>,
    /// (round, slot, slot, some keys, number of keys)
    pub conflicts: PList<(u32, Owner, Owner, Rc<[Fact]>, u32)>,
    pub ran_by: FxHashMap<Slot, S>,
    pub not_run: FxHashMap<Slot, S>,
    /// (unit name, origin, why)
    pub skips: FxHashSet<(Slot, S, S)>,
    pub sentences: u32,
    pub steps0: u32,
    pub timed: bool,
    pub camera_done: FxHashSet<(Slot, u32)>,
    pub ambient_pass: FxHashSet<Slot>,
    pub ambient_vals: FxHashMap<Fact, Vec<Val>>,
    pub ambient_slots: FxHashSet<Slot>,
    pub multi: FxHashSet<Slot>,
    pub closed: bool,
    /// a hash of the kept slots, locals included, when this round began (not dumped)
    pub round_slots: u64,
    // --- dialog
    /// (key, pre value, value in the store)
    pub input_drift: Vec<(Fact, Val, Val)>,
    // --- esc
    /// False, True, or None when merged paths disagree
    pub cleared: Option<bool>,
    // --- golden.py's trace bookkeeping
    pub tid: Option<u32>,
}

impl Path {
    /// `Path(stack)` with model.py's defaults.
    pub fn new(stack: Vec<Entry>) -> Path {
        Path { stack, next_sid: 1, cleared: Some(false), ..Default::default() }
    }

    /// The running (top) frame's index in the stack, if any.
    #[inline]
    pub fn top_frame_index(&self) -> Option<usize> {
        self.stack.iter().rposition(|e| matches!(e, Entry::Frame(_)))
    }

    /// model.py `Path.frame`: the running (top) frame; once the stack holds none, the first frame.
    pub fn frame(&self) -> Option<&Frame> {
        match self.top_frame_index() {
            Some(i) => match &self.stack[i] {
                Entry::Frame(f) => Some(f),
                Entry::Call(_) => unreachable!(),
            },
            None => self.last.as_deref(),
        }
    }

    pub fn frame_mut(&mut self) -> Option<&mut Frame> {
        match self.top_frame_index() {
            Some(i) => match &mut self.stack[i] {
                Entry::Frame(f) => Some(f),
                Entry::Call(_) => unreachable!(),
            },
            None => self.last.as_deref_mut(),
        }
    }

    /// model.py `Path.fork`: a copy; the copy has no trace id (dataclasses.replace makes a new object).
    pub fn fork(&self) -> Path {
        let mut q = self.clone();
        q.tid = None;
        q
    }

    /// The frames on the stack, bottom first (model.py `Machine._frames`).
    pub fn frames(&self) -> impl Iterator<Item = &Frame> {
        self.stack.iter().filter_map(|e| match e {
            Entry::Frame(f) => Some(f),
            Entry::Call(_) => None,
        })
    }

    pub fn frames_mut(&mut self) -> impl Iterator<Item = &mut Frame> {
        self.stack.iter_mut().filter_map(|e| match e {
            Entry::Frame(f) => Some(f),
            Entry::Call(_) => None,
        })
    }

    /// The stack index of the frame `sid` (model.py `_frame_of`; a sid of 0 or None finds none).
    pub fn frame_of(&self, sid: Option<u32>) -> Option<usize> {
        let sid = sid.filter(|&s| s != 0)?;
        self.stack.iter().position(|e| matches!(e, Entry::Frame(f) if f.sid == sid))
    }

    /// The frame at stack index `i`.
    pub fn frame_at(&mut self, i: usize) -> &mut Frame {
        match &mut self.stack[i] {
            Entry::Frame(f) => f,
            Entry::Call(_) => panic!("stack entry {i} is a call"),
        }
    }

    /// The call at stack index `i`.
    pub fn call_at(&mut self, i: usize) -> &mut Call {
        match &mut self.stack[i] {
            Entry::Call(c) => c,
            Entry::Frame(_) => panic!("stack entry {i} is a frame"),
        }
    }

    /// `p.slots.get(name)`.
    pub fn slot(&self, name: Slot) -> Option<&Frame> {
        self.slots.iter().find(|s| s.0 == name).map(|s| &s.1)
    }

    /// `p.slots[name] = f`: in its place when the name is there, else last (a dict's order).
    pub fn set_slot(&mut self, name: Slot, f: Frame) {
        match self.slots.iter_mut().find(|s| s.0 == name) {
            Some(s) => s.1 = f,
            None => self.slots.push((name, f)),
        }
    }

    /// `p.slots.pop(name, None)`.
    pub fn pop_slot(&mut self, name: Slot) {
        if let Some(i) = self.slots.iter().position(|s| s.0 == name) {
            self.slots.remove(i);
        }
    }

    /// model.py `Machine._running`: whether the frame `sid` is still the running one (the top, the path
    /// not ended).
    pub fn running(&self, sid: u32) -> bool {
        self.end.is_none() && matches!(self.stack.last(), Some(Entry::Frame(f)) if f.sid == sid)
    }
}

/// model.py `Run`.
#[derive(Debug, Default)]
pub struct Run {
    pub paths: Vec<Path>,
    /// the first instruction's cite, or why nothing ran
    pub start: String,
    pub limit: u32,
    /// the unit's key, as model.py's tuple (None: no unit)
    pub unit: Option<scumm_ir::UnitKey>,
    pub merges: u32,
    /// VALUE_CAPS of the run, in first-count order
    pub value_caps: Vec<(&'static str, u64)>,
    pub settle_merges: u32,
    pub pre_slots: Vec<Slot>,
    pub value_merges: u32,
    /// (where the key went, the assumptions the path held then)
    pub key_routes: Vec<(S, Vec<Assumption>)>,
    /// the run's unknowns, which the paths' values index
    pub vals: crate::value::Vals,
}
