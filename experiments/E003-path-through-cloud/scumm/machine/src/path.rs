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

/// A frame's locals: Local[n] -> value, sorted by n. Shared by the frame's copies until one writes (a fork copies
/// every frame of the stack, and the frames below the running one do not change).
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Locals(Option<Rc<Vec<(i32, Val)>>>);

impl Locals {
    /// Locals from (n, value) pairs, sorted here.
    pub fn from_vec(mut v: Vec<(i32, Val)>) -> Locals {
        v.sort_by_key(|l| l.0);
        Locals((!v.is_empty()).then(|| Rc::new(v)))
    }

    pub fn as_slice(&self) -> &[(i32, Val)] {
        self.0.as_deref().map_or(&[], |v| v.as_slice())
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (i32, Val)> {
        self.as_slice().iter()
    }

    pub fn get(&self, n: i32) -> Option<Val> {
        self.as_slice().iter().find(|l| l.0 == n).map(|l| l.1)
    }

    pub fn set(&mut self, n: i32, v: Val) {
        let vec = Rc::make_mut(self.0.get_or_insert_with(|| Rc::new(Vec::new())));
        match vec.binary_search_by_key(&n, |l| l.0) {
            Ok(i) => vec[i].1 = v,
            Err(i) => vec.insert(i, (n, v)),
        }
    }
}

/// A running script's slot (model.py `Frame`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Frame {
    pub unit: UnitId,
    pub pc: Option<u32>,
    /// Local[n] -> value, sorted by n
    pub locals: Locals,
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
            locals: Locals::default(),
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
        self.locals.get(n)
    }

    pub fn set_local(&mut self, n: i32, v: Val) {
        self.locals.set(n, v)
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
    // --- dialog (row 4)
    /// ("input", area, verb, button): checkExecVerbs' click on a verb, runInputScript
    Input { area: i32, verb: i32, button: i32 },
    // --- esc (row 5)
    /// ("abort", armed esc: keys (string-sorted), VAR_CUTSCENEEXIT_KEY at pre): processInput's abortCutscene
    Abort { armed: Vec<Fact>, code: Val },
    /// ("override", unit name, W): the rewound slot's turn from its override point (W dumps as a str, as model.py
    /// splits it from the key)
    Override { name: Slot, w: i32 },
    /// ("key", VAR_CUTSCENEEXIT_KEY at pre): checkExecVerbs takes the key code
    Key(Val),
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

/// A join signature (model.py `_join`'s tuple): paths with one signature have the same stack (frames at the same
/// instruction with the same slot state, calls at the same step) and, with settle, the same kept slots' places, queue,
/// ambient loops run, slots that ran this round, turn owner, camera exits forked and `timed`. A round's start is
/// `("round", round, position key)`, the key a hash (model.py compares the hash itself).
///
/// The signature is kept as one vector of words (`Machine::join` writes each field in a fixed place, and a list with
/// its length first, so that equal words are equal signatures; frozensets go sorted) and a hash of them, made once:
/// equality tests the hash first, so the search for a path's waiting group mostly compares one number per group. A
/// call is named by its `cid` (its head and cite follow from it).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sig {
    pub h: u64,
    round: bool,
    words: Vec<u64>,
    /// what a hook adds to the signature (reader.py's `Reader._join`), encoded; empty without hooks. A hook that
    /// sets it also mixes it into `h`
    pub ext: Box<[u8]>,
}

impl Sig {
    pub fn from_words(round: bool, words: Vec<u64>) -> Sig {
        use std::hash::{Hash, Hasher};
        let mut st = rustc_hash::FxHasher::default();
        round.hash(&mut st);
        words.hash(&mut st);
        Sig { h: fmix(st.finish()), round, words, ext: Box::default() }
    }

    /// `("round", round, position key)`.
    pub fn round(round: u32, pos: u64) -> Sig {
        Sig::from_words(true, vec![round as u64, pos])
    }

    pub fn is_round(&self) -> bool {
        self.round
    }
}

impl std::hash::Hash for Sig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.h.hash(state);
    }
}

/// A unit name as a word.
#[inline]
pub fn slot_word(n: Slot) -> u64 {
    let (kind, v) = match n {
        Slot::Script(x) => (0u64, x),
        Slot::Local(x) => (1, x),
        Slot::Obj(x) => (2, x),
        Slot::Entry => (3, 0),
        Slot::Exit => (4, 0),
    };
    (kind << 32) | v as u32 as u64
}

/// An optional number as a word (None is all ones).
#[inline]
pub fn opt_word(x: Option<u32>) -> u64 {
    x.map_or(u64::MAX, |v| v as u64)
}

/// An optional bool as a word (None 2).
#[inline]
pub fn bool_word(x: Option<bool>) -> u64 {
    x.map_or(2, |b| b as u64)
}

/// `Frame.owned` as a word: False 0, True 1, an unknown 2 + its id (model.py compares `str(owned)`: for interned
/// values, the same test).
#[inline]
pub fn owned_word(o: Owned) -> u64 {
    match o {
        Owned::Bool(b) => b as u64,
        Owned::Unk(Val::Unk(id)) => 2 + id.0 as u64,
        Owned::Unk(Val::Int(_)) => unreachable!("an owned unknown"),
    }
}

/// `Frame.killed` as a word.
#[inline]
pub fn killed_word(k: Option<&'static str>) -> u64 {
    match k {
        None => 0,
        Some("stopped") => 1,
        Some("chained") => 2,
        Some("room") => 3,
        Some(other) => unreachable!("a frame killed as {other}"),
    }
}

/// `Path.owner` as a word.
#[inline]
pub fn owner_word(o: Option<Owner>) -> u64 {
    match o {
        None => u64::MAX,
        Some(Owner::Sentence) => u64::MAX - 1,
        Some(Owner::Slot(n)) => slot_word(n),
    }
}

/// `Frame.origin` as a word.
#[inline]
pub fn origin_word(o: &'static str) -> u64 {
    match o {
        "path" => 0,
        "pre" => 1,
        other => unreachable!("a frame origin {other}"),
    }
}

/// Two ints as one word.
#[inline]
pub fn pair_word(a: i32, b: i32) -> u64 {
    ((a as u32 as u64) << 32) | b as u32 as u64
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

impl crate::plist::FastEq for Assumption {
    #[inline]
    fn fast_eq(&self, o: &Self) -> bool {
        use crate::plist::s_eq;
        self.side == o.side && s_eq(&self.cite, &o.cite) && s_eq(&self.cond, &o.cond) && s_eq(&self.why, &o.why)
    }
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

/// A fact map of writes (model.py's `dict` of fact -> value). It keeps the sum of its entries' hashes as they are
/// written, so that a hash of the whole store (`_spins`, `_state_key`) costs nothing.
#[derive(Clone, Debug, Default)]
pub struct Store {
    map: FxHashMap<Fact, Val>,
    sum: u64,
}

impl Store {
    #[inline]
    pub fn get(&self, k: &Fact) -> Option<&Val> {
        self.map.get(k)
    }

    #[inline]
    pub fn contains_key(&self, k: &Fact) -> bool {
        self.map.contains_key(k)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// `store[k] = v`.
    #[inline]
    pub fn insert(&mut self, k: Fact, v: Val) {
        if let Some(old) = self.map.insert(k, v) {
            self.sum = self.sum.wrapping_sub(fx(&(k, old)));
        }
        self.sum = self.sum.wrapping_add(fx(&(k, v)));
    }

    /// Every (key, value), in no order.
    #[inline]
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, Fact, Val> {
        self.map.iter()
    }

    #[inline]
    pub fn keys(&self) -> std::collections::hash_map::Keys<'_, Fact, Val> {
        self.map.keys()
    }

    /// An order-free hash of the entries (the set of items, as model.py's `frozenset(writes.items())`).
    #[inline]
    pub fn hash_sum(&self) -> u64 {
        self.sum
    }

    /// `store.pop(k, None)` (a hook's: reader.py deletes writes).
    pub fn remove(&mut self, k: &Fact) -> Option<Val> {
        let old = self.map.remove(k)?;
        self.sum = self.sum.wrapping_sub(fx(&(*k, old)));
        Some(old)
    }
}

impl PartialEq for Store {
    /// model.py's dict equality (`g.writes == base.writes`).
    fn eq(&self, other: &Store) -> bool {
        self.sum == other.sum && self.map == other.map
    }
}

/// model.py `Path.rw`: owner -> (keys read before it wrote them, keys written) in this round's turns. One flat map of
/// (owner, key) -> read and written flags, and the owners `rw.setdefault` added (an owner may hold no key). Shared by
/// a path's forks until one adds to it: a read or write it already holds changes nothing.
#[derive(Clone, Debug, Default)]
pub struct Rw(Rc<RwInner>);

#[derive(Clone, Debug, Default)]
struct RwInner {
    owners: Vec<Owner>,
    map: FxHashMap<(Owner, Fact), u8>,
}

const RW_READ: u8 = 1;
const RW_WRITTEN: u8 = 2;

impl Rw {
    /// `rw.setdefault(owner, (set(), set()))`.
    #[inline]
    pub fn add_owner(&mut self, o: Owner) {
        if !self.0.owners.contains(&o) {
            Rc::make_mut(&mut self.0).owners.push(o);
        }
    }

    /// `r.add(key)` unless `key in w` (a key it holds already, read or written, changes nothing).
    #[inline]
    pub fn read(&mut self, o: Owner, k: Fact) {
        if !self.0.map.contains_key(&(o, k)) {
            Rc::make_mut(&mut self.0).map.insert((o, k), RW_READ);
        }
    }

    /// `w.add(key)`.
    #[inline]
    pub fn write(&mut self, o: Owner, k: Fact) {
        match self.0.map.get(&(o, k)) {
            Some(&f) if f & RW_WRITTEN != 0 => {}
            _ => *Rc::make_mut(&mut self.0).map.entry((o, k)).or_insert(0) |= RW_WRITTEN,
        }
    }

    pub fn clear(&mut self) {
        if !self.is_empty() {
            self.0 = Rc::default();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.owners.is_empty() && self.0.map.is_empty()
    }

    pub fn owners(&self) -> &[Owner] {
        &self.0.owners
    }

    pub fn has_owner(&self, o: Owner) -> bool {
        self.0.owners.contains(&o)
    }

    /// An owner's (reads, writes).
    pub fn sets(&self, o: Owner) -> (FxHashSet<Fact>, FxHashSet<Fact>) {
        let (mut r, mut w) = (FxHashSet::default(), FxHashSet::default());
        for (&(ow, k), &f) in &self.0.map {
            if ow == o {
                if f & RW_READ != 0 {
                    r.insert(k);
                }
                if f & RW_WRITTEN != 0 {
                    w.insert(k);
                }
            }
        }
        (r, w)
    }

    /// Every owner's (reads, writes), in `owners` order.
    pub fn all_sets(&self) -> Vec<(Owner, FxHashSet<Fact>, FxHashSet<Fact>)> {
        let mut out: Vec<(Owner, FxHashSet<Fact>, FxHashSet<Fact>)> =
            self.0.owners.iter().map(|&o| (o, FxHashSet::default(), FxHashSet::default())).collect();
        for (&(ow, k), &f) in &self.0.map {
            let e = out.iter_mut().find(|e| e.0 == ow).expect("an owner of rw");
            if f & RW_READ != 0 {
                e.1.insert(k);
            }
            if f & RW_WRITTEN != 0 {
                e.2.insert(k);
            }
        }
        out
    }

    /// model.py `_merge`'s union of each owner's sets.
    pub fn union(&mut self, other: &Rw) {
        if Rc::ptr_eq(&self.0, &other.0) {
            return;
        }
        for &o in &other.0.owners {
            self.add_owner(o);
        }
        for (&k, &f) in &other.0.map {
            match self.0.map.get(&k) {
                Some(&have) if have | f == have => {}
                _ => *Rc::make_mut(&mut self.0).map.entry(k).or_insert(0) |= f,
            }
        }
    }
}

/// model.py `Path.writer`: key -> the rule of the settle turn that last wrote it (model.py's rule strings are
/// literals). Shared by a path's forks until one changes it: a write by the rule it holds already changes nothing.
#[derive(Clone, Debug, Default)]
pub struct Writer(Rc<FxHashMap<Fact, &'static str>>);

impl Writer {
    /// `writer[k] = rule`.
    #[inline]
    pub fn insert(&mut self, k: Fact, rule: &'static str) {
        if self.0.get(&k) != Some(&rule) {
            Rc::make_mut(&mut self.0).insert(k, rule);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Fact, &&'static str)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// model.py `_merge`: `q.writer[k] = rule if q.writer.get(k, rule) == rule else "mixed"` for each of `other`'s.
    pub fn merge(&mut self, other: &Writer) {
        if Rc::ptr_eq(&self.0, &other.0) {
            return;
        }
        for (&k, &rule) in other.0.iter() {
            match self.0.get(&k) {
                Some(&have) if have == rule || have == "mixed" => {}
                Some(_) => {
                    Rc::make_mut(&mut self.0).insert(k, "mixed");
                }
                None => {
                    Rc::make_mut(&mut self.0).insert(k, rule);
                }
            }
        }
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
    /// the rule that slot runs by (model.py's rule strings are literals)
    pub owner_rule: &'static str,
    pub writer: Writer,
    /// owner -> (keys read before it wrote them, keys written), this round
    pub rw: Rw,
    /// (round, slot, slot, some keys, number of keys)
    pub conflicts: PList<(u32, Owner, Owner, Rc<[Fact]>, u32)>,
    pub ran_by: FxHashMap<Slot, &'static str>,
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
    /// what a hook adds to a path (reader.py's `SymPath` fields); None without hooks. Forks copy it
    pub ext: Option<Box<dyn PathExt>>,
}

/// The fields a hook adds to a path (reader.py's `SymPath` subclass of `model.Path`): cloned with the path.
pub trait PathExt: std::any::Any + std::fmt::Debug {
    fn clone_box(&self) -> Box<dyn PathExt>;
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl Clone for Box<dyn PathExt> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
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
