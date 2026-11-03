//! fmi-core: the types and ports every E011 crate shares (experiments/E011-speed/plan.md, Appendix A).
//!
//! Copied byte for byte from plan.md into `experiments/E011-speed/rs/fmi-core/`; only the integrator changes it
//! (a change is proposed in /home/ilyask/.cache/fast-mi-e011/COORD.md as an "IFACE" entry first). No dependency.
//!
//! * `Key`, `Val`, `Facts`: facts.py's `{key: int}` maps with integer keys (absent is 0).
//! * `facts_of`: facts.py `facts_of` on the typed state dump `Obs` (the C7 dump less its text).
//! * `Domain`: domains.py `Domain` (labels BELOW/ABOVE as sentinel values).
//! * the engine port (`Engine`, `EngineHost`), the reading port (`ReadingSource`), the planner port (`Planner`) and the
//!   play port (`Player`): what one component calls in another. Each port has one provider (plan.md, "Components").

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

// --- values ------------------------------------------------------------------------------------------------------

/// A fact's value: an engine int, or a domain label below.
pub type Val = i64;
/// domains.py BELOW ("below"): every value under the domain's `low`.
pub const BELOW: Val = i64::MIN;
/// domains.py ABOVE ("above"): every value over the domain's `high`.
pub const ABOVE: Val = i64::MAX;
/// Python's None where a value is not known or the domain does not hold it (`Domain.label` -> None).
pub const NONE: Val = i64::MIN + 1;

/// A value as Python prints it in the reference's files: "below", "above", "None" or the int.
pub fn val_text(v: Val) -> String {
    match v {
        BELOW => "below".into(),
        ABOVE => "above".into(),
        NONE => "None".into(),
        n => n.to_string(),
    }
}

// --- keys ----------------------------------------------------------------------------------------------------------

/// A fact key, packed: kind (bits 56-63), unit (48-55), a (24-47), b (0-23). `Key::parse(t).text() == t` for every
/// key text; a text no packed form holds exactly (another prefix, a number over 2^24 - 1, a non-canonical number) is
/// interned instead (kind `K_OTHER`, a = its index in this process's table), so keys of that kind are equal only
/// within one process. The order of `Key` is not Python's string order: sort by `text()` where a file must match.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Key(pub u64);

pub const K_VAR: u8 = 1; // var:N
pub const K_BIT: u8 = 2; // bit:N
pub const K_OWNER: u8 = 3; // owner:O
pub const K_STATE: u8 = 4; // state:O
pub const K_CLASS: u8 = 5; // class:O:C
pub const K_ROOM: u8 = 6; // room
pub const K_EGO: u8 = 7; // ego
pub const K_SCRIPT: u8 = 8; // script:N
pub const K_LOCAL: u8 = 9; // local:N
pub const K_OBJSCRIPT: u8 = 10; // objscript:O
pub const K_AT: u8 = 11; // at:<unit>:W
pub const K_FROZEN: u8 = 12; // frozen:<unit>
pub const K_CUTSCENE: u8 = 13; // cutscene
pub const K_ESC: u8 = 14; // esc:<unit>:W
pub const K_VERB: u8 = 15; // verb:V
pub const K_POS: u8 = 16; // pos:<unit>
pub const K_ESCPOS: u8 = 17; // escpos:<unit>
pub const K_ARGS: u8 = 18; // args:<unit>
pub const K_SAVED: u8 = 19; // saved:A:B
pub const K_ACTORROOM: u8 = 20; // actorroom:A
pub const K_OBJPOS: u8 = 21; // objpos:O
pub const K_GOAL: u8 = 22; // goal (task.py GOAL_KEY)
pub const K_OTHER: u8 = 255; // interned text

/// A slot's unit (facts.py `unit_of`): script:N, local:N, obj:O, entry, exit.
pub const U_NONE: u8 = 0;
pub const U_SCRIPT: u8 = 1;
pub const U_LOCAL: u8 = 2;
pub const U_OBJ: u8 = 3;
pub const U_ENTRY: u8 = 4;
pub const U_EXIT: u8 = 5;

const MAX24: u64 = (1 << 24) - 1;

type Interned = Mutex<(Vec<String>, HashMap<String, u32>)>;

fn intern() -> &'static Interned {
    static T: OnceLock<Interned> = OnceLock::new();
    T.get_or_init(|| Mutex::new((Vec::new(), HashMap::new())))
}

fn num(s: &str) -> Option<u64> {
    // canonical decimal only ("0", "17"; not "017", "+1", "-1"), so that text() gives the same string back
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse::<u64>().ok().filter(|&n| n <= MAX24)
}

fn unit(s: &str) -> Option<(u8, u64)> {
    match s {
        "entry" => Some((U_ENTRY, 0)),
        "exit" => Some((U_EXIT, 0)),
        _ => {
            let (k, n) = s.split_once(':')?;
            let u = match k {
                "script" => U_SCRIPT,
                "local" => U_LOCAL,
                "obj" => U_OBJ,
                _ => return None,
            };
            Some((u, num(n)?))
        }
    }
}

fn unit_text(u: u8, a: u64) -> String {
    match u {
        U_SCRIPT => format!("script:{a}"),
        U_LOCAL => format!("local:{a}"),
        U_OBJ => format!("obj:{a}"),
        U_ENTRY => "entry".into(),
        U_EXIT => "exit".into(),
        _ => "?".into(),
    }
}

impl Key {
    pub const fn pack(kind: u8, unit: u8, a: u64, b: u64) -> Key {
        Key(((kind as u64) << 56) | ((unit as u64) << 48) | ((a & MAX24) << 24) | (b & MAX24))
    }
    pub fn kind(self) -> u8 {
        (self.0 >> 56) as u8
    }
    pub fn unit(self) -> u8 {
        (self.0 >> 48) as u8
    }
    pub fn a(self) -> u64 {
        (self.0 >> 24) & MAX24
    }
    pub fn b(self) -> u64 {
        self.0 & MAX24
    }
    pub fn var(n: u64) -> Key {
        Key::pack(K_VAR, 0, n, 0)
    }
    pub fn bit(n: u64) -> Key {
        Key::pack(K_BIT, 0, n, 0)
    }
    pub fn owner(o: u64) -> Key {
        Key::pack(K_OWNER, 0, o, 0)
    }
    pub fn state(o: u64) -> Key {
        Key::pack(K_STATE, 0, o, 0)
    }
    pub fn class(o: u64, c: u64) -> Key {
        Key::pack(K_CLASS, 0, o, c)
    }
    pub fn verb(v: u64) -> Key {
        Key::pack(K_VERB, 0, v, 0)
    }

    /// The key of a text (facts.py, the reader's and the task's keys).
    pub fn parse(t: &str) -> Key {
        if let Some(k) = Key::packed(t) {
            if k.text() == t {
                return k;
            }
        }
        let mut g = intern().lock().unwrap();
        if let Some(&i) = g.1.get(t) {
            return Key::pack(K_OTHER, 0, i as u64, 0);
        }
        let i = g.0.len() as u32;
        g.0.push(t.to_string());
        g.1.insert(t.to_string(), i);
        Key::pack(K_OTHER, 0, i as u64, 0)
    }

    fn packed(t: &str) -> Option<Key> {
        match t {
            "room" => return Some(Key::pack(K_ROOM, 0, 0, 0)),
            "ego" => return Some(Key::pack(K_EGO, 0, 0, 0)),
            "cutscene" => return Some(Key::pack(K_CUTSCENE, 0, 0, 0)),
            "goal" => return Some(Key::pack(K_GOAL, 0, 0, 0)),
            _ => {}
        }
        let (p, rest) = t.split_once(':')?;
        let one = |k: u8| num(rest).map(|n| Key::pack(k, 0, n, 0));
        let two = |k: u8| {
            let (a, b) = rest.split_once(':')?;
            Some(Key::pack(k, 0, num(a)?, num(b)?))
        };
        let of_unit = |k: u8| unit(rest).map(|(u, a)| Key::pack(k, u, a, 0));
        let unit_w = |k: u8| {
            let (u, w) = rest.rsplit_once(':')?;
            let (u, a) = unit(u)?;
            Some(Key::pack(k, u, a, num(w)?))
        };
        match p {
            "var" => one(K_VAR),
            "bit" => one(K_BIT),
            "owner" => one(K_OWNER),
            "state" => one(K_STATE),
            "class" => two(K_CLASS),
            "script" => one(K_SCRIPT),
            "local" => one(K_LOCAL),
            "objscript" => one(K_OBJSCRIPT),
            "at" => unit_w(K_AT),
            "frozen" => of_unit(K_FROZEN),
            "esc" => unit_w(K_ESC),
            "verb" => one(K_VERB),
            "pos" => of_unit(K_POS),
            "escpos" => of_unit(K_ESCPOS),
            "args" => of_unit(K_ARGS),
            "saved" => two(K_SAVED),
            "actorroom" => one(K_ACTORROOM),
            "objpos" => one(K_OBJPOS),
            _ => None,
        }
    }

    /// The key's text, as facts.py and the reader write it.
    pub fn text(self) -> String {
        let (a, b) = (self.a(), self.b());
        match self.kind() {
            K_VAR => format!("var:{a}"),
            K_BIT => format!("bit:{a}"),
            K_OWNER => format!("owner:{a}"),
            K_STATE => format!("state:{a}"),
            K_CLASS => format!("class:{a}:{b}"),
            K_ROOM => "room".into(),
            K_EGO => "ego".into(),
            K_SCRIPT => format!("script:{a}"),
            K_LOCAL => format!("local:{a}"),
            K_OBJSCRIPT => format!("objscript:{a}"),
            K_AT => format!("at:{}:{b}", unit_text(self.unit(), a)),
            K_FROZEN => format!("frozen:{}", unit_text(self.unit(), a)),
            K_CUTSCENE => "cutscene".into(),
            K_ESC => format!("esc:{}:{b}", unit_text(self.unit(), a)),
            K_VERB => format!("verb:{a}"),
            K_POS => format!("pos:{}", unit_text(self.unit(), a)),
            K_ESCPOS => format!("escpos:{}", unit_text(self.unit(), a)),
            K_ARGS => format!("args:{}", unit_text(self.unit(), a)),
            K_SAVED => format!("saved:{a}:{b}"),
            K_ACTORROOM => format!("actorroom:{a}"),
            K_OBJPOS => format!("objpos:{a}"),
            K_GOAL => "goal".into(),
            K_OTHER => intern()
                .lock()
                .unwrap()
                .0
                .get(a as usize)
                .cloned()
                .unwrap_or_else(|| "?".into()),
            _ => "?".into(),
        }
    }

    /// `text().starts_with(prefix)`, as the reference's prefix tests (UNOBSERVED, PREDICTED, SLOT_PREFIXES).
    pub fn has_prefix(self, prefix: &str) -> bool {
        self.text().starts_with(prefix)
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

// --- facts ---------------------------------------------------------------------------------------------------------

/// facts.py's `{key: int}`: pairs sorted by `Key`, each key once; a key that is absent is 0 (facts.py).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Facts(pub Vec<(Key, Val)>);

impl Facts {
    pub fn new() -> Facts {
        Facts(Vec::new())
    }
    /// From pairs in any order; a later pair for the same key wins (a Python dict built in that order).
    pub fn from_pairs(v: Vec<(Key, Val)>) -> Facts {
        let mut idx: Vec<usize> = (0..v.len()).collect();
        idx.sort_by_key(|&i| (v[i].0, std::cmp::Reverse(i)));
        idx.dedup_by_key(|i| v[*i].0);
        Facts(idx.into_iter().map(|i| v[i]).collect())
    }
    /// The value as `dict.get(k)`: None when absent.
    pub fn get(&self, k: Key) -> Option<Val> {
        self.0.binary_search_by_key(&k, |p| p.0).ok().map(|i| self.0[i].1)
    }
    /// The value as facts.py reads a map: absent is 0.
    pub fn val(&self, k: Key) -> Val {
        self.get(k).unwrap_or(0)
    }
    pub fn set(&mut self, k: Key, v: Val) {
        match self.0.binary_search_by_key(&k, |p| p.0) {
            Ok(i) => self.0[i].1 = v,
            Err(i) => self.0.insert(i, (k, v)),
        }
    }
    pub fn remove(&mut self, k: Key) {
        if let Ok(i) = self.0.binary_search_by_key(&k, |p| p.0) {
            self.0.remove(i);
        }
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &(Key, Val)> {
        self.0.iter()
    }
    /// facts.py `diff`: (key, old, new) for every key whose value differs, absent being 0; in `Key` order.
    pub fn diff(&self, other: &Facts) -> Vec<(Key, Val, Val)> {
        let (a, b) = (&self.0, &other.0);
        let (mut i, mut j, mut out) = (0, 0, Vec::new());
        while i < a.len() || j < b.len() {
            let (k, x, y) = if j >= b.len() || (i < a.len() && a[i].0 < b[j].0) {
                i += 1;
                (a[i - 1].0, a[i - 1].1, 0)
            } else if i >= a.len() || b[j].0 < a[i].0 {
                j += 1;
                (b[j - 1].0, 0, b[j - 1].1)
            } else {
                i += 1;
                j += 1;
                (a[i - 1].0, a[i - 1].1, b[j - 1].1)
            };
            if x != y {
                out.push((k, x, y));
            }
        }
        out
    }
    /// facts.py `patch`: the diff applied; a new value of 0 removes the key.
    pub fn patch(&mut self, d: &[(Key, Val, Val)]) {
        for &(k, _, new) in d {
            if new != 0 {
                self.set(k, new)
            } else {
                self.remove(k)
            }
        }
    }
    /// The pairs in Python's string order of the keys (for files the reference writes sorted).
    pub fn text_sorted(&self) -> Vec<(String, Val)> {
        let mut v: Vec<(String, Val)> = self.0.iter().map(|&(k, x)| (k.text(), x)).collect();
        v.sort();
        v
    }
}

// --- the engine's state (C7 less its text) -------------------------------------------------------------------------

/// The bridge's C7 state dump (speedrun_state.cpp `stateJson`), field for field and in its order, less the names
/// (game text): what the JSON dump and the binary dump both decode to, so `Obs` equality is the binary dump's check.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Obs {
    pub tick: u64,
    pub frame: u64,
    pub room: i32,
    pub ego: Option<i32>,
    pub ego_pos: Option<(i32, i32)>,
    pub seed: Option<i64>,
    pub boot_param: i32,
    pub vars: Vec<i32>,
    pub bits_set: Vec<u32>,
    /// absent in a lite dump
    pub inventory: Option<Vec<i32>>,
    /// (object, owner) in the dump's order; a lite dump holds only the asked objects
    pub owners: Vec<(u32, i32)>,
    pub states: Vec<(u32, i32)>,
    /// (object, its classes 1..32 ascending), objects with a class only
    pub classes: Vec<(u32, Vec<u32>)>,
    /// absent in a lite dump
    pub room_objects: Option<Vec<RoomObject>>,
    pub verbs: Vec<Verb>,
    pub slots: Vec<Slot>,
    pub cutscene: Cutscene,
    pub actors: Vec<ActorPos>,
    /// SPEEDRUN_DUMP_EXACT's object (the profile's exactness checks), when asked
    pub exact: Option<Exact>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct RoomObject {
    pub id: i32,
    pub state: i32,
    pub parent: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub walk: (i32, i32),
    /// the name (game text): only with `ObsReq::names`
    pub name: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Verb {
    pub slot: i32,
    pub id: i32,
    pub mode: i32,
    pub saveid: i32,
    pub kind: i32, // the dump's "type"
    pub key: i32,
    pub x: i32,
    pub y: i32,
    /// the name (game text): only with `ObsReq::names`
    pub name: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Slot {
    pub slot: i32,
    pub number: i32,
    pub where_: i32, // the dump's "where"
    pub status: i32,
    pub offs: u64,
    pub code: i64,
    pub frozen: i32,
    pub delay: i32,
    pub cutscene_override: i32,
    /// with dump_locals (replay_part1.config): the slot's locals, its freeze resistance and recursion
    pub locals: Option<Vec<i32>>,
    pub resistant: Option<bool>,
    pub recursive: Option<bool>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Cutscene {
    pub depth: i32,
    pub levels: Vec<CutLevel>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct CutLevel {
    pub level: i32,
    pub data: Option<i32>,
    pub override_: Option<Override>, // the dump's "override" (null when none)
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Override {
    pub slot: i32,
    pub number: i32,
    pub where_: i32,
    pub status: i32,
    pub ptr: u64,
    pub code: i64,
    pub stale: bool,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct ActorPos {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub elevation: i32,
    pub moving: i32,
}

/// The exact object (speedrun_state.cpp exactJson): every field as the dump has it, in its order; the text-valued
/// fields kept as their text ("audio_owed" is "%.17g", "screens" a hex digest).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Exact {
    pub fields: Vec<(String, ExactVal)>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum ExactVal {
    Int(i64),
    Bool(bool),
    Text(String),
    List(Vec<ExactVal>),
    Map(Vec<(String, ExactVal)>),
    Null,
}

/// What a dump holds (the JSON "state" command's options, and the binary twin's flags).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ObsReq {
    /// Some: a lite dump (`{"cmd": "state", "objects": [...]}`): these objects' owners, states and classes only
    pub objects: Option<Vec<u32>>,
    pub names: bool,
    pub exact: bool,
}

// --- facts_of ------------------------------------------------------------------------------------------------------

pub const WIO_INVENTORY: i32 = 0;
pub const WIO_ROOM: i32 = 1;
pub const WIO_GLOBAL: i32 = 2;
pub const WIO_LOCAL: i32 = 3;
pub const WIO_FLOBJECT: i32 = 4;
pub const ENCD: i32 = 10002;
pub const EXCD: i32 = 10001;
pub const CLOCKS: [usize; 6] = [11, 12, 13, 14, 46, 47];

/// facts.py `unit_of`: (unit, number) of a slot, or None.
pub fn unit_of(where_: i32, number: i32) -> Option<(u8, u64)> {
    match where_ {
        WIO_GLOBAL => Some((U_SCRIPT, number as u64)),
        WIO_LOCAL => Some((U_LOCAL, number as u64)),
        WIO_ROOM if number == ENCD => Some((U_ENTRY, 0)),
        WIO_ROOM if number == EXCD => Some((U_EXIT, 0)),
        WIO_INVENTORY | WIO_ROOM | WIO_FLOBJECT => Some((U_OBJ, number as u64)),
        _ => None,
    }
}

/// facts.py `facts_of`, on the typed dump.
pub fn facts_of(s: &Obs) -> Facts {
    let mut f: Vec<(Key, Val)> = Vec::with_capacity(s.vars.len() + 4096);
    for (n, &v) in s.vars.iter().enumerate() {
        if !CLOCKS.contains(&n) {
            f.push((Key::var(n as u64), v as Val));
        }
    }
    f.extend(s.bits_set.iter().map(|&n| (Key::bit(n as u64), 1)));
    f.extend(s.owners.iter().map(|&(o, v)| (Key::owner(o as u64), v as Val)));
    f.extend(s.states.iter().map(|&(o, v)| (Key::state(o as u64), v as Val)));
    for (o, cs) in &s.classes {
        f.extend(cs.iter().map(|&c| (Key::class(*o as u64, c as u64), 1)));
    }
    f.push((Key::pack(K_ROOM, 0, 0, 0), s.room as Val));
    if let Some(e) = s.ego {
        f.push((Key::pack(K_EGO, 0, 0, 0), e as Val));
    }
    for sl in &s.slots {
        let room_code = sl.number == ENCD || sl.number == EXCD;
        match sl.where_ {
            WIO_GLOBAL => f.push((Key::pack(K_SCRIPT, 0, sl.number as u64, 0), 1)),
            WIO_LOCAL => f.push((Key::pack(K_LOCAL, 0, sl.number as u64, 0), 1)),
            WIO_INVENTORY | WIO_ROOM | WIO_FLOBJECT if !room_code => {
                f.push((Key::pack(K_OBJSCRIPT, 0, sl.number as u64, 0), 1))
            }
            _ => {}
        }
        if let Some((u, a)) = unit_of(sl.where_, sl.number) {
            if sl.code >= 0 {
                f.push((Key::pack(K_AT, u, a, sl.code as u64), 1));
            }
            if sl.frozen > 0 {
                f.push((Key::pack(K_FROZEN, u, a, 0), 1));
            }
        }
    }
    f.push((Key::pack(K_CUTSCENE, 0, 0, 0), s.cutscene.depth as Val));
    if let Some(top) = s.cutscene.levels.last().and_then(|l| l.override_.as_ref()) {
        if !top.stale && top.code >= 0 {
            if let Some((u, a)) = unit_of(top.where_, top.number) {
                f.push((Key::pack(K_ESC, u, a, top.code as u64), 1));
            }
        }
    }
    f.extend(s.verbs.iter().map(|v| (Key::verb(v.id as u64), 1)));
    Facts::from_pairs(f)
}

// --- domains -------------------------------------------------------------------------------------------------------

/// domains.py `Domain`: (BELOW?, ints ascending..., ABOVE?), with `low`/`high` set when BELOW/ABOVE are in it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Domain {
    pub values: Vec<Val>,
    pub low: Option<i64>,
    pub high: Option<i64>,
}

impl Domain {
    /// `Domain.label`: the value an engine value has here; NONE when the domain does not hold it.
    pub fn label(&self, n: i64) -> Val {
        if let Some(h) = self.high {
            if n > h {
                return ABOVE;
            }
        }
        if let Some(l) = self.low {
            if n < l {
                return BELOW;
            }
        }
        if self.values.binary_search(&n).is_ok() {
            n
        } else {
            NONE
        }
    }
}

/// key -> domain (domains.Domains.doms with the binary prefixes, as the harness's `doms`); sorted by key.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Domains(pub Vec<(Key, Domain)>);

impl Domains {
    pub fn get(&self, k: Key) -> Option<&Domain> {
        self.0.binary_search_by_key(&k, |p| p.0).ok().map(|i| &self.0[i].1)
    }
}

// --- engine commands -----------------------------------------------------------------------------------------------

/// One C8 input event (speedrun_inputs.cpp); `frame` as the "inputs" command takes it (frames after now).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Event {
    pub frame: u64,
    pub what: Input,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Input {
    Click(i32, i32),
    RClick(i32, i32),
    Move(i32, i32),
    /// "esc", "space", "enter" or one printable character
    Key(String),
    Skips {
        text: bool,
        cutscenes: bool,
        spare: Option<Vec<i32>>,
    },
}

/// One C3 condition as the bridge holds it (SpeedrunCondition).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cond {
    pub kind: CondKind,
    pub subject: i32,
    pub op: CondOp,
    pub value: i32,
    pub values: Vec<i32>,
    pub negate: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CondKind {
    Var,
    Bit,
    Room,
    Owner,
    State,
    Has,
    ActorRoom,
    ActorX,
    ActorY,
    Class,
    ActorMoving,
    Script,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CondOp {
    Eq,
    Le,
    Ge,
    In,
}

/// The "run" command's stops (speedrun_control.cpp); the first that holds ends the run.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RunSpec {
    /// run to the decision point of this frame
    pub frame: Option<u64>,
    pub max_frame: Option<u64>,
    pub until: Vec<Cond>,
    pub until_any: Vec<Vec<Cond>>,
    pub control: bool,
    pub idle: bool,
    pub menu: bool,
}

/// The stop (or end) reply of a run (speedrun_control.cpp `status`), field for field.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Status {
    /// the stop reason ("frame", "max_frame", "control", "until", "idle", "menu", "until_any:<i>", "fork_each"),
    /// or the end reason when `ended`
    pub reason: String,
    pub ended: bool,
    pub frame: u64,
    pub tick: u64,
    pub qf: u64,
    pub qf0: u64,
    pub room: i32,
    pub up: i32,
    pub cur: i32,
    pub cs: i32,
    pub v19: i32,
    pub seg: bool,
    pub tick0: u64,
    pub rng: u64,
    /// "sentence" or "dialog" when the bridge's idle rule holds
    pub idle: Option<String>,
    pub blk: Option<String>,
    pub ego: Option<(i32, i32)>,
    pub mv: Option<i32>,
    pub walkbox: Option<i32>, // the dump's "box"
    pub cam: i32,
    pub xstart: i32,
    pub room_w: i32,
    /// fork_each: (frame, pid) of each child
    pub forks: Vec<(u64, u32)>,
    /// the screen layout at the stop: initScreens' b and h and `_forceBannerVirtScreen` (the hit test's screens,
    /// play/HITCACHE.md); reported by the binary channel with patch 0004, None otherwise (the JSON protocol, an end)
    pub screen: Option<(i32, i32, bool)>,
}

/// The "at" reply: what a click at screen (x, y) would hit now.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct At {
    pub x: i32,
    pub y: i32,
    pub zone: i32,
    /// in the main virtual screen: (vx, vy, obj, actor)
    pub main: Option<(i32, i32, i32, i32)>,
    pub verb_slot: i32,
}

/// One slot run (speedrun_explain.cpp, the "runs" reply's rows), field for field.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Run {
    pub frame: u64,
    pub after: bool,
    pub seq: u64,
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
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RunsReply {
    pub runs: Vec<Run>,
    pub dropped: u64,
    pub violations: u64,
    pub frame: u64,
}

/// One "explain" entry (speedrun_explain.cpp), field for field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Explain {
    Begin {
        seq: u64,
        frame: u64,
        after: bool,
        slot: i32,
        where_: i32,
        number: i32,
        room: i32,
        code0: i64,
    },
    Op {
        seq: u64,
        code: i64,
        opcode: u8,
    },
    Read {
        seq: u64,
        var: u32,
        value: i32,
    },
    Write {
        seq: u64,
        var: u32,
        value: i32,
    },
    End {
        seq: u64,
        code1: i64,
        status: i32,
        frozen: i32,
        delay: i32,
    },
}

/// The "explain" command's arming: frames from..=to, the units [(where, number)] (empty: every unit).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ExplainArm {
    pub from: u64,
    pub to: u64,
    pub units: Vec<(i32, i32)>,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ExplainReply {
    pub entries: Vec<Explain>,
    pub dropped: u64,
    pub violations: u64,
}

#[derive(Debug)]
pub enum EngineError {
    /// the engine replied with an error (the bridge's "error" text)
    Bridge(String),
    /// the engine ended (its "end" reply) where a stop was expected
    Ended(Box<Status>),
    Io(std::io::Error),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Bridge(e) => write!(f, "bridge: {e}"),
            EngineError::Ended(s) => write!(f, "engine ended: {}", s.reason),
            EngineError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for EngineError {}

pub type EResult<T> = Result<T, EngineError>;

/// One engine process, stopped at a decision point (the bridge's control socket, one connection per process).
/// Provider: the `engine` component (fmi-engine, the binary channel). Every method is one command and its reply.
pub trait Engine: Send {
    fn pid(&self) -> u32;
    /// the status of the last stop (the hello's frame and tick before the first run)
    fn status(&self) -> &Status;
    fn run(&mut self, spec: &RunSpec) -> EResult<Status>;
    /// schedule input; frames are frames after now (the "inputs" command)
    fn inputs(&mut self, events: &[Event]) -> EResult<()>;
    fn obs(&mut self, req: &ObsReq) -> EResult<Obs>;
    /// `facts_of(obs(full))` without building the dump the facts do not read (the hot path)
    fn facts(&mut self) -> EResult<Facts>;
    fn holds(&mut self, conds: &[Cond]) -> EResult<bool>;
    fn at(&mut self, x: i32, y: i32) -> EResult<At>;
    fn set_rng(&mut self, state: u32) -> EResult<()>;
    fn segment(&mut self, on: bool) -> EResult<()>;
    fn runs(&mut self) -> EResult<RunsReply>;
    /// Some(arm) arms the explain log; None disarms it
    fn explain(&mut self, arm: Option<&ExplainArm>) -> EResult<()>;
    fn explain_log(&mut self) -> EResult<ExplainReply>;
    /// an exact copy of this process (fork), stopped where this one is
    fn fork(&mut self) -> EResult<Box<dyn Engine>>;
    /// the "run" command's fork_each: a child at each of the next `n` decision points, each stopped there; the
    /// parent stops after the last one ("fork_each")
    fn fork_each(&mut self, n: u32) -> EResult<(Status, Vec<Box<dyn Engine>>)>;
    /// the C7 dump's JSON text (the differential tests against the Python reference only)
    fn state_json(&mut self, req: &ObsReq) -> EResult<String>;
    /// CPU seconds of this process so far (utime + stime)
    fn cpu_s(&self) -> f64;
    fn quit(self: Box<Self>);
    /// An observed frame in one command: run to the decision point of `frame`, then what `runs`, `explain_log` and
    /// `obs(req)` give at that stop (the window search's observed child frame). The default makes the four calls.
    fn step_observe(&mut self, frame: u64, req: &ObsReq) -> EResult<(Status, RunsReply, ExplainReply, Obs)> {
        let st = self.run(&RunSpec {
            frame: Some(frame),
            ..Default::default()
        })?;
        let runs = self.runs()?;
        let ents = self.explain_log()?;
        let o = self.obs(req)?;
        Ok((st, runs, ents, o))
    }
    /// A run and the dump at its stop in one command (the input code's run when it reads the state next). The
    /// default makes the two calls.
    fn run_obs(&mut self, spec: &RunSpec, req: &ObsReq) -> EResult<(Status, Obs)> {
        let st = self.run(spec)?;
        let o = self.obs(req)?;
        Ok((st, o))
    }
    /// A run and the room's dump at its stop in one command: the full dump cut by `room_cut` (what the input code
    /// reads). The default makes the two calls and cuts.
    fn run_room(&mut self, spec: &RunSpec) -> EResult<(Status, Obs)> {
        let (st, mut o) = self.run_obs(spec, &ObsReq::default())?;
        room_cut(&mut o);
        Ok((st, o))
    }
    /// `at` at each point, in order, at the current stop (the hit tests read and change nothing). The default makes
    /// one call per point.
    fn at_many(&mut self, pts: &[(i32, i32)]) -> EResult<Vec<At>> {
        pts.iter().map(|&(x, y)| self.at(x, y)).collect()
    }
    /// Runs `frames` frames with no new input (a run to the frame `frames` after this stop) and returns the stop's
    /// status and the facts' changes from this stop to it: (key, old, new) for every key whose value differs, absent
    /// 0, in `Key` order (`Facts::diff` of the facts before and after). Default: today's calls; a provider that
    /// follows Step's delta gives them in one round trip and with no whole facts map (IFACE e011-r2-own).
    fn step_changes(&mut self, frames: u32) -> EResult<(Status, Vec<(Key, Val, Val)>)> {
        let before = self.facts()?;
        let to = self.status().frame + u64::from(frames);
        let st = self.run(&RunSpec { frame: Some(to), ..Default::default() })?;
        let after = self.facts()?;
        Ok((st, before.diff(&after)))
    }
    /// The wait move's run (E012, plan.md section 1): frames with no new input from this stop, one at a time. After
    /// each, `each` gets its status and the facts' changes from the frame before (as `step_changes(1)` gives them)
    /// and returns true to stop there. At most `frames` frames. Returns the last status and the frames run. Default:
    /// one `step_changes(1)` per frame; a provider may run the same frames in fewer round trips.
    fn wait(&mut self, frames: u64, each: &mut dyn FnMut(&Status, &[(Key, Val, Val)]) -> bool) -> EResult<(Status, u64)> {
        let mut st = self.status().clone();
        let mut n = 0;
        while n < frames {
            let (s, ch) = self.step_changes(1)?;
            n += 1;
            st = s;
            if each(&st, &ch) {
                break;
            }
        }
        Ok((st, n))
    }
}

/// The room's dump made from a full dump: the owners, states and classes of the objects the room's object list
/// names only (in the full dump's object order), and no exact object; every other field as it is.
pub fn room_cut(o: &mut Obs) {
    let mut ids: Vec<u32> = o
        .room_objects
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|r| r.id as u32)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let keep = |x: &u32| ids.binary_search(x).is_ok();
    o.owners.retain(|p| keep(&p.0));
    o.states.retain(|p| keep(&p.0));
    o.classes.retain(|p| keep(&p.0));
    o.exact = None;
}

/// How an engine starts (replay_part1.config): seed 1, the input movie from boot, the bridge's settings.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct BootCfg {
    pub seed: u32,
    /// the movie: events from boot with absolute frames (an entry's "events")
    pub events: Vec<Event>,
    /// run to the decision point of this frame before returning (None: the first idle stop, as replay_part1.boot)
    pub frame: Option<u64>,
    pub dump_locals: bool,
    /// the engine's exact fast path (patch 0002's switches); false runs it as svm-w3b does
    pub lean: bool,
    pub out_dir: PathBuf,
}

/// Starts engines. Provider: the `engine` component.
pub trait EngineHost: Send + Sync {
    fn boot(&self, cfg: &BootCfg) -> EResult<Box<dyn Engine>>;
}

// --- the reading ---------------------------------------------------------------------------------------------------

/// A reading's files (packed_reading.py's reading: reading.json, reading.tc, reading.actions.jsonl,
/// reading.lists.jsonl, reading.reached) and E011's index. Provider: the `readio` component (fmi-reading).
pub trait ReadingSource: Send + Sync {
    /// sha1 of reading.tc: a reading's identity in every cache key
    fn id(&self) -> [u8; 20];
    fn dir(&self) -> &Path;
    /// reading.json's text
    fn head(&self) -> &str;
    /// reading.tc's bytes (mapped)
    fn packed(&self) -> &[u8];
    fn n_actions(&self) -> u32;
    /// reading.actions.jsonl's line i, without its newline (byte-identical to packed_reading's)
    fn action_line(&self, i: u32) -> &str;
    /// strict.Reading's `by_name`: the last action with this name
    fn by_name(&self, name: &str) -> Option<u32>;
    fn n_lists(&self) -> u32;
    /// reading.lists.jsonl's line i, without its newline
    fn list_line(&self, i: u32) -> &str;
    /// reading.reached's text
    fn reached(&self) -> &str;
    /// the reading's task facts and their domains (the packed file's facts, as PackedTask.facts)
    fn domains(&self) -> &Domains;
    /// the packed file's goal
    fn goal(&self) -> &[(Key, Vec<Val>)];
    /// the start facts it was read from (reading.json "start", as a facts.py map)
    fn start(&self) -> &Facts;
    /// PackedTask._action(i): name "label#rec" (or the label), pre, effects and cost; `what` = i
    fn action(&self, i: u32) -> Action;
    /// the reader.Record PackedTask._action(i) builds (the action's `what`)
    fn record(&self, i: u32) -> Record;
}

/// reader.Record's effect on one key (packed_reading.effects_of): ("set", v), ("guess", values), ("add", d),
/// ("copy", key), ("when", key, v).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RecEff {
    Set(Val),
    Guess(Vec<Val>),
    Add(i64),
    Copy(Key),
    When(Key, Val),
}

/// reader.Record as packed_reading.PackedTask._action builds it from the packed action and its line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Record {
    pub kind: String,
    pub label: String,
    /// the precondition on the line's "pre_keys" that the packed precondition holds
    pub pre: Vec<(Key, Vec<Val>)>,
    pub effects: Vec<(Key, RecEff)>,
    /// the line's "checks", each as its JSON text
    pub checks: Vec<String>,
    pub end: String,
    /// the line's "source" as its JSON text
    pub source: String,
    pub engine_pre: Vec<Key>,
}

// --- the planner ---------------------------------------------------------------------------------------------------

/// task.py `Effect`: a known value, the key plus a constant, or one of several values (a guess).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EffVal {
    Value(Val),
    Add(i64),
    Values(Vec<Val>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Effect {
    pub key: Key,
    pub v: EffVal,
    /// ((key, values), ...): all hold before the action
    pub when: Vec<(Key, Vec<Val>)>,
}

/// task.py `Action` as the planner's paths give it; `what` is the reading's action index (its record).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Action {
    pub name: String,
    pub pre: Vec<(Key, Vec<Val>)>,
    pub effects: Vec<Effect>,
    pub what: u32,
    pub cost: i64,
}

/// replay.Ban: a record's label and the (key, value) pairs it is banned under.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ban {
    pub label: String,
    pub facts: Vec<(Key, Val)>,
}

/// plan_fd.Step: one step of a plan.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlanStep {
    pub op: String,
    pub action: u32,
    pub name: String,
    pub chosen: Vec<(Key, Val)>,
    /// (key, value, when): values the reader could not work out, guessed here
    pub guessed: Vec<(Key, Val, String)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Search {
    /// Fast Downward's lama-first-costs on taskcore's SAS file (plan_fd.SEARCHES), as E009 runs it
    FdLamaFirstCosts,
    /// the Rust search: lazy greedy best-first, h_FF with preferred operators, cost + 1 (the profile's prototype)
    Gbfs,
}

/// One plan request (plan_fd.plan_taskcore's inputs).
#[derive(Clone, Debug)]
pub struct PlanRequest {
    /// the goal (taskcore --goal): key -> values
    pub goal: Vec<(Key, Vec<Val>)>,
    /// the start (a facts.py map with the task's pos:/escpos:/args: keys)
    pub start: Facts,
    pub bans: Vec<Ban>,
    /// the cost caps (replay.cost_caps_of): action name -> cap
    pub caps: Vec<(String, i64)>,
    pub search: Search,
    /// taskcore --start-check: "needed" or "input"
    pub start_check: String,
    /// the previous plan's remaining steps' names: used when they still form a plan from this start (plan.md)
    pub tail: Option<Vec<String>>,
    pub time_limit_s: u64,
}

/// plan_fd.TaskcoreResult, the fields the harness reads.
#[derive(Clone, Debug, Default)]
pub struct PlanResult {
    /// "plan", "unsolvable", "incomplete", "time", "memory", "error" or "out of domain"
    pub status: String,
    pub steps: Vec<PlanStep>,
    /// per step its path of the banned task (res.paths) and that path cut to the needed facts (res.actions)
    pub paths: Vec<Action>,
    pub actions: Vec<Action>,
    /// step index -> its paths left to the engine (res.choices), cut to the needed facts
    pub choices: Vec<(u32, Vec<Action>)>,
    /// per step its fold's name when it folds more than one member (res.folds)
    pub folds: Vec<Option<String>>,
    pub predicted_goal: bool,
    pub plan_cost: Option<i64>,
    pub file_facts: Vec<Key>,
    pub needed_facts: Vec<Key>,
    /// out of domain: (key, start value) of the facts out of their domains, and their count
    pub misses: Vec<(Key, Val)>,
    pub n_misses: u64,
    /// taskcore's stats.json text
    pub stats: String,
    /// where the plan came from: "search", "cache", "tail"
    pub source: String,
    pub seconds: f64,
    pub cpu_s: f64,
}

#[derive(Debug)]
pub enum PlanError {
    /// plan_fd.PathChoice: a step whose paths differ on facts the file holds
    PathChoice(String),
    /// plan_fd.TaskcoreError (with its message, which `reach_misses` reads)
    Taskcore(String),
}

/// The planner of one reading, kept warm across requests. Provider: the `plan` component (taskcore's library).
pub trait Planner: Send {
    fn plan(&mut self, req: &PlanRequest) -> Result<PlanResult, PlanError>;
}

/// Makes the warm planner of a reading (one per reading id, kept by the caller).
pub trait PlannerHost: Send + Sync {
    fn planner(&self, reading: &dyn ReadingSource) -> Result<Box<dyn Planner>, PlanError>;
}

// --- play ----------------------------------------------------------------------------------------------------------

/// The harness's state the window search reads and writes (replay.Harness's fields that window.py uses).
pub struct PlayState {
    /// h.branch: the engine the plan plays on
    pub engine: Box<dyn Engine>,
    /// h.state, h.obs, h.now (view of obs), h.pred, h.kept
    pub state: Obs,
    pub obs: Facts,
    pub now: Facts,
    pub pred: Facts,
    pub kept: Facts,
    /// h.checkpoints
    pub checkpoints: Vec<Checkpoint>,
    pub stop_reason: Option<String>,
    /// h.replan_cause (window.py's deviation cause), JSON text
    pub replan_cause: Option<String>,
    /// h.plans' entries and the window records, JSON lines the reference writes
    pub records: Vec<String>,
    pub workdir: PathBuf,
}

/// replay.Harness.checkpoint's tuple: (frame, facts, extra_of(state, status)), for `verify`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Checkpoint {
    pub frame: u64,
    pub facts: Facts,
    pub tick: u64,
    pub rng: u64,
    /// sorted [id, x, y] of the dump's actors
    pub actors: Vec<(i32, i32, i32)>,
}

/// What the plan plays against (the rest of the harness's fields window.py reads).
pub struct PlanCtx<'a> {
    pub reading: &'a dyn ReadingSource,
    pub doms: &'a Domains,
    /// h.unobserved: key prefixes the dump does not hold
    pub unobserved: &'a [&'a str],
    pub goal: &'a [(Key, Vec<Val>)],
    pub result: &'a PlanResult,
    pub unobserved_actor_rooms: bool,
    pub time_limit_s: f64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlayOutcome {
    Goal,
    Replan,
    Stop,
}

/// Plays one plan through its windows (window.Search.play_plan), and observes for the harness (Harness.observe).
/// Provider: the `play` component (fmi-play).
pub trait Player: Send {
    fn observe(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState) -> EResult<()>;
    fn play_plan(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState, n_plan: u32) -> PlayOutcome;
}

// --- E012: time passing (plan.md section 1, the wait component) ---------------------------------------------------

/// A slot's status while it waits out a delay (script.h ssPaused); a frozen slot has 0x80 set on top of it.
pub const SS_PAUSED: i32 = 1;
/// VAR_TIMER_NEXT (var 19 in SCUMM v5): the ticks the engine's next frame lasts.
pub const VAR_TIMER_NEXT: usize = 19;
/// The most a paused slot's delay goes down in one frame (scumm.cpp scummLoop: `if (delta > 15) delta = 15;`).
pub const DELAY_STEP_MAX: i64 = 15;

/// The ticks the next frame takes off every paused slot's delay: the stop's var 19 (the main loop's delta is
/// VAR_TIMER_NEXT, at least 1), at most DELAY_STEP_MAX (scummLoop's cap before decreaseScriptDelay). None when the
/// dump has no var 19.
pub fn delay_step(o: &Obs) -> Option<i64> {
    let v = *o.vars.get(VAR_TIMER_NEXT)? as i64;
    Some(v.clamp(1, DELAY_STEP_MAX))
}

/// The frame in which the dump's slot `slot` runs again, by the engine's own rule (script.cpp decreaseScriptDelay):
/// each frame lowers the delay of every slot whose status is exactly SS_PAUSED (not a frozen one) by the frame's
/// delta, and the slot runs in the frame its delay goes below 0, before that frame's decision point (v5:
/// runAllScripts before checkExecVerbs). So the stop's frame + floor(delay / delta) + 1. None for a slot not in the
/// dump or not paused. Exact while var 19 keeps its value.
pub fn due(o: &Obs, slot: i32) -> Option<u64> {
    let s = o.slots.iter().find(|s| s.slot == slot)?;
    if s.status != SS_PAUSED {
        return None;
    }
    let d = delay_step(o)?;
    Some(o.frame + (i64::from(s.delay.max(0)) / d) as u64 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        for t in [
            "var:100",
            "bit:108",
            "owner:566",
            "state:316",
            "class:12:3",
            "room",
            "ego",
            "script:2",
            "local:202",
            "objscript:489",
            "at:local:202:21",
            "at:script:2:7",
            "at:entry:0",
            "at:obj:428:105",
            "frozen:script:2",
            "frozen:entry",
            "cutscene",
            "esc:local:200:33",
            "verb:2",
            "pos:script:21",
            "escpos:exit",
            "args:obj:12",
            "saved:1:2",
            "actorroom:3",
            "objpos:44",
            "goal",
            "pick:var:100",
            "due:x",
            "var:007",
            "var:-1",
        ] {
            assert_eq!(Key::parse(t).text(), t, "{t}");
        }
        assert_eq!(Key::parse("var:007").kind(), K_OTHER);
        assert_eq!(Key::parse("at:local:202:21"), Key::pack(K_AT, U_LOCAL, 202, 21));
    }

    #[test]
    fn diff_and_patch() {
        let a = Facts::from_pairs(vec![(Key::var(1), 3), (Key::bit(2), 1), (Key::var(1), 4)]);
        assert_eq!(a.get(Key::var(1)), Some(4));
        let b = Facts::from_pairs(vec![(Key::var(1), 4), (Key::var(5), 0), (Key::verb(9), 1)]);
        let d = a.diff(&b);
        assert_eq!(d, vec![(Key::bit(2), 1, 0), (Key::verb(9), 0, 1)]);
        let mut c = a.clone();
        c.patch(&d);
        assert!(c.diff(&b).is_empty());
    }

    #[test]
    fn facts_of_slots() {
        let s = Obs {
            vars: vec![7; 50],
            slots: vec![Slot {
                where_: WIO_LOCAL,
                number: 202,
                code: 21,
                frozen: 1,
                ..Default::default()
            }],
            cutscene: Cutscene {
                depth: 1,
                levels: vec![CutLevel {
                    level: 0,
                    data: None,
                    override_: Some(Override {
                        where_: WIO_ROOM,
                        number: ENCD,
                        code: 4,
                        ..Default::default()
                    }),
                }],
            },
            ..Default::default()
        };
        let f = facts_of(&s);
        assert_eq!(f.get(Key::var(11)), None);
        assert_eq!(f.get(Key::parse("local:202")), Some(1));
        assert_eq!(f.get(Key::parse("at:local:202:21")), Some(1));
        assert_eq!(f.get(Key::parse("frozen:local:202")), Some(1));
        assert_eq!(f.get(Key::parse("esc:entry:4")), Some(1));
        assert_eq!(f.get(Key::parse("cutscene")), Some(1));
    }
}
