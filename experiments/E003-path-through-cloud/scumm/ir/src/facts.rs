//! facts.py's key scheme: the engine's state as a flat fact map, with typed keys.
//!
//! A [`Fact`] is a small `Copy` key that prints as facts.py's string (`var:5`, `class:17:32`,
//! `at:local:200:14`, `esc:script:152:82`, ...) and parses back from it. A [`FactMap`] maps keys to
//! values; an absent key is 0. [`facts_of`] turns the bridge's C7 state dump into a map, [`diff`] and
//! [`patch`] are facts.py's. facts.py's docstring has what each key means and the ScummVM cites.

use rustc_hash::FxHashMap;
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

// ScummVM constants (scumm.h:213-217, script.h:121-122, vars.cpp, scumm.h:1153)
pub const WIO_INVENTORY: i32 = 0;
pub const WIO_ROOM: i32 = 1;
pub const WIO_GLOBAL: i32 = 2;
pub const WIO_LOCAL: i32 = 3;
pub const WIO_FLOBJECT: i32 = 4;
/// kScriptNumENCD: the room's entry code
pub const ENCD: i32 = 10002;
/// kScriptNumEXCD: the room's exit code
pub const EXCD: i32 = 10001;
/// VAR_TMR_1-3, VAR_MUSIC_TIMER, VAR_TIMER, VAR_TIMER_TOTAL: the engine's clocks, not facts
pub const CLOCKS: [i32; 6] = [11, 12, 13, 14, 46, 47];
/// _resourceHeaderSize: a global script's PC starts after its resource header
pub const HEADER: i32 = 8;

#[inline]
pub fn is_clock(n: i32) -> bool {
    CLOCKS.contains(&n)
}

/// The unit an `at:`, `frozen:` or `esc:` key names: `script:N`, `local:N`, `obj:O`, `entry`, `exit`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Slot {
    Script(i32),
    Local(i32),
    Obj(i32),
    Entry,
    Exit,
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Slot::Script(n) => write!(f, "script:{n}"),
            Slot::Local(n) => write!(f, "local:{n}"),
            Slot::Obj(o) => write!(f, "obj:{o}"),
            Slot::Entry => f.write_str("entry"),
            Slot::Exit => f.write_str("exit"),
        }
    }
}

impl Slot {
    /// The unit kind as `family` splits `at:` and `esc:` keys: "script", "local", "obj", "entry", "exit".
    pub fn kind(self) -> &'static str {
        match self {
            Slot::Script(_) => "script",
            Slot::Local(_) => "local",
            Slot::Obj(_) => "obj",
            Slot::Entry => "entry",
            Slot::Exit => "exit",
        }
    }
}

/// facts.py `unit_of`: the unit of a slot (None for a `where` the engine does not use).
pub fn unit_of(where_: i32, number: i32) -> Option<Slot> {
    match where_ {
        WIO_GLOBAL => Some(Slot::Script(number)),
        WIO_LOCAL => Some(Slot::Local(number)),
        WIO_ROOM if number == ENCD => Some(Slot::Entry),
        WIO_ROOM if number == EXCD => Some(Slot::Exit),
        WIO_INVENTORY | WIO_ROOM | WIO_FLOBJECT => Some(Slot::Obj(number)),
        _ => None,
    }
}

/// A fact key.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Fact {
    /// `var:N`: global variable N (not a clock)
    Var(i32),
    /// `bit:N`
    Bit(i32),
    /// `owner:O`
    Owner(i32),
    /// `state:O`
    State(i32),
    /// `class:O:C`
    Class(i32, i32),
    /// `room`
    Room,
    /// `ego`
    Ego,
    /// `script:N`: a live global script
    Script(i32),
    /// `local:N`: a live room-local script
    Local(i32),
    /// `objscript:O`: a live object script
    ObjScript(i32),
    /// `at:<unit>:W`: a live slot of the unit resumes at W
    At(Slot, i32),
    /// `frozen:<unit>`
    Frozen(Slot),
    /// `cutscene`: the cutscene stack's depth
    Cutscene,
    /// `esc:<unit>:W`: the override armed at the top level
    Esc(Slot, i32),
    /// `verb:V`: verb V is visible
    Verb(i32),
    /// `objpos:O` (fix round 1, F5, switch draw_clears; not facts.py's): where object O is drawn in this room visit,
    /// as an index into `Game::places(O)` (0 is its CDHD place). o5_drawObject's setXY moves it
    /// (script_v5.cpp:1108-1114); loading the room puts it back (room.cpp:195, object.cpp:1145-1150)
    ObjPos(i32),
}

impl fmt::Display for Fact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Fact::Var(n) => write!(f, "var:{n}"),
            Fact::Bit(n) => write!(f, "bit:{n}"),
            Fact::Owner(o) => write!(f, "owner:{o}"),
            Fact::State(o) => write!(f, "state:{o}"),
            Fact::Class(o, c) => write!(f, "class:{o}:{c}"),
            Fact::Room => f.write_str("room"),
            Fact::Ego => f.write_str("ego"),
            Fact::Script(n) => write!(f, "script:{n}"),
            Fact::Local(n) => write!(f, "local:{n}"),
            Fact::ObjScript(o) => write!(f, "objscript:{o}"),
            Fact::At(u, w) => write!(f, "at:{u}:{w}"),
            Fact::Frozen(u) => write!(f, "frozen:{u}"),
            Fact::Cutscene => f.write_str("cutscene"),
            Fact::Esc(u, w) => write!(f, "esc:{u}:{w}"),
            Fact::Verb(v) => write!(f, "verb:{v}"),
            Fact::ObjPos(o) => write!(f, "objpos:{o}"),
        }
    }
}

/// A key that is not facts.py's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadKey(pub String);

impl fmt::Display for BadKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a fact key: {:?}", self.0)
    }
}

impl std::error::Error for BadKey {}

/// An int as Python prints it: no sign but '-', no leading zeros. A key with any other spelling is
/// refused, so a parsed key prints back to the same string.
fn int(s: &str) -> Option<i32> {
    let v: i32 = s.parse().ok()?;
    (v.to_string() == s).then_some(v)
}

fn slot_and_rest(s: &str) -> Option<(Slot, &str)> {
    let (kind, rest) = s.split_once(':').unwrap_or((s, ""));
    match kind {
        "entry" => Some((Slot::Entry, rest)),
        "exit" => Some((Slot::Exit, rest)),
        "script" | "local" | "obj" => {
            let (n, rest) = rest.split_once(':').unwrap_or((rest, ""));
            let n = int(n)?;
            let u = match kind {
                "script" => Slot::Script(n),
                "local" => Slot::Local(n),
                _ => Slot::Obj(n),
            };
            Some((u, rest))
        }
        _ => None,
    }
}

impl FromStr for Fact {
    type Err = BadKey;
    fn from_str(s: &str) -> Result<Self, BadKey> {
        let bad = || BadKey(s.to_string());
        let (kind, rest) = s.split_once(':').unwrap_or((s, ""));
        let one = |r: &str| int(r).ok_or_else(bad);
        let f = match kind {
            "room" if rest.is_empty() && !s.contains(':') => Fact::Room,
            "ego" if !s.contains(':') => Fact::Ego,
            "cutscene" if !s.contains(':') => Fact::Cutscene,
            "var" => Fact::Var(one(rest)?),
            "bit" => Fact::Bit(one(rest)?),
            "owner" => Fact::Owner(one(rest)?),
            "state" => Fact::State(one(rest)?),
            "class" => {
                let (o, c) = rest.split_once(':').ok_or_else(bad)?;
                Fact::Class(one(o)?, one(c)?)
            }
            "script" => Fact::Script(one(rest)?),
            "local" => Fact::Local(one(rest)?),
            "objscript" => Fact::ObjScript(one(rest)?),
            "verb" => Fact::Verb(one(rest)?),
            "objpos" => Fact::ObjPos(one(rest)?),
            "at" | "esc" => {
                let (u, w) = slot_and_rest(rest).ok_or_else(bad)?;
                let w = one(w)?;
                if kind == "at" {
                    Fact::At(u, w)
                } else {
                    Fact::Esc(u, w)
                }
            }
            "frozen" => match slot_and_rest(rest) {
                Some((u, "")) if !rest.ends_with(':') => Fact::Frozen(u),
                _ => return Err(bad()),
            },
            _ => return Err(bad()),
        };
        if f.to_string() != s {
            return Err(bad());
        }
        Ok(f)
    }
}

impl Fact {
    /// model.py `kind_of`: the part before the first ':' ("var", "at", "room", ...).
    pub fn kind(self) -> &'static str {
        match self {
            Fact::Var(_) => "var",
            Fact::Bit(_) => "bit",
            Fact::Owner(_) => "owner",
            Fact::State(_) => "state",
            Fact::Class(..) => "class",
            Fact::Room => "room",
            Fact::Ego => "ego",
            Fact::Script(_) => "script",
            Fact::Local(_) => "local",
            Fact::ObjScript(_) => "objscript",
            Fact::At(..) => "at",
            Fact::Frozen(_) => "frozen",
            Fact::Cutscene => "cutscene",
            Fact::Esc(..) => "esc",
            Fact::Verb(_) => "verb",
            Fact::ObjPos(_) => "objpos",
        }
    }

    /// model.py `family`: the kind, with `at:` and `esc:` split by unit kind ("at:script", "esc:local", ...).
    pub fn family(self) -> String {
        match self {
            Fact::At(u, _) => format!("at:{}", u.kind()),
            Fact::Esc(u, _) => format!("esc:{}", u.kind()),
            f => f.kind().to_string(),
        }
    }
}

/// A fact map: an absent key is 0.
pub type FactMap = FxHashMap<Fact, i32>;

fn num(v: &Value, what: &str) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("state dump: {what} is not an int: {v}")) as i32
}

fn key_int(k: &str, what: &str) -> i32 {
    int(k).unwrap_or_else(|| panic!("state dump: {what} key {k:?} is not an int"))
}

/// facts.py `facts_of`: the bridge's C7 state dump (`Branch.state()`) as facts.
pub fn facts_of(state: &Value) -> FactMap {
    let mut f = FactMap::default();
    for (n, v) in state["vars"].as_array().expect("vars").iter().enumerate() {
        if !is_clock(n as i32) {
            f.insert(Fact::Var(n as i32), num(v, "var"));
        }
    }
    for n in state["bits_set"].as_array().expect("bits_set") {
        f.insert(Fact::Bit(num(n, "bit")), 1);
    }
    for (o, v) in state["owners"].as_object().expect("owners") {
        f.insert(Fact::Owner(key_int(o, "owner")), num(v, "owner"));
    }
    for (o, v) in state["states"].as_object().expect("states") {
        f.insert(Fact::State(key_int(o, "state")), num(v, "state"));
    }
    for (o, classes) in state["classes"].as_object().expect("classes") {
        let o = key_int(o, "class");
        for c in classes.as_array().expect("class list") {
            f.insert(Fact::Class(o, num(c, "class")), 1);
        }
    }
    f.insert(Fact::Room, num(&state["room"], "room"));
    if let Some(ego) = state.get("ego").filter(|e| !e.is_null()) {
        f.insert(Fact::Ego, num(ego, "ego"));
    }
    for s in state["slots"].as_array().expect("slots") {
        // the dump lists live slots only
        let (w, n) = (num(&s["where"], "where"), num(&s["number"], "number"));
        match w {
            WIO_GLOBAL => {
                f.insert(Fact::Script(n), 1);
            }
            WIO_LOCAL => {
                f.insert(Fact::Local(n), 1);
            }
            WIO_INVENTORY | WIO_ROOM | WIO_FLOBJECT if n != ENCD && n != EXCD => {
                f.insert(Fact::ObjScript(n), 1);
            }
            _ => {}
        }
        if let Some(u) = unit_of(w, n) {
            let code = num(&s["code"], "code");
            if code >= 0 {
                f.insert(Fact::At(u, code), 1);
            }
            if num(&s["frozen"], "frozen") > 0 {
                f.insert(Fact::Frozen(u), 1);
            }
        }
    }
    let cutscene = &state["cutscene"];
    f.insert(Fact::Cutscene, num(&cutscene["depth"], "depth"));
    let levels = cutscene["levels"].as_array().expect("levels");
    let top = &levels.last().expect("levels run from 0 to the depth")["override"];
    if !top.is_null() && !top["stale"].as_bool().expect("stale") {
        let code = num(&top["code"], "code");
        if code >= 0 {
            if let Some(u) = unit_of(num(&top["where"], "where"), num(&top["number"], "number")) {
                f.insert(Fact::Esc(u, code), 1);
            }
        }
    }
    for v in state["verbs"].as_array().expect("verbs") {
        f.insert(Fact::Verb(num(&v["id"], "verb id")), 1);
    }
    f
}

/// facts.py `diff`: {key: (old, new)} for every key whose value differs from `a` to `b`; absent is 0.
pub fn diff(a: &FactMap, b: &FactMap) -> FxHashMap<Fact, (i32, i32)> {
    let mut out = FxHashMap::default();
    for (k, &va) in a {
        let vb = b.get(k).copied().unwrap_or(0);
        if va != vb {
            out.insert(*k, (va, vb));
        }
    }
    for (k, &vb) in b {
        if !a.contains_key(k) && vb != 0 {
            out.insert(*k, (0, vb));
        }
    }
    out
}

/// facts.py `patch`: `a` with a `diff` applied; a new value of 0 removes the key.
pub fn patch(a: &FactMap, d: &FxHashMap<Fact, (i32, i32)>) -> FactMap {
    let mut out = a.clone();
    for (k, &(_, new)) in d {
        if new != 0 {
            out.insert(*k, new);
        } else {
            out.remove(k);
        }
    }
    out
}
