//! The reading as the window search reads it, over the reading port (`fmi_core::ReadingSource`):
//!
//! * the records (reader.Record as packed_reading.PackedTask._action builds it: `Rec`) and the task's actions, decoded
//!   on demand and kept;
//! * packed_reading.Points: the wait and Esc points of each unit, the ELSEWHERE facts, the live locals;
//! * the built lists (packed_reading.load_lists);
//! * window.source_index (the actions by their input) and strict.Reading's event index and B1 tuples (`B1`);
//! * window.FastView and strict.ArgsView.

use crate::pyj::{self, J};
use crate::val::*;
use fmi_core::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::{Arc, Mutex, OnceLock};

/// reader.Record.checks' entry: (cite, condition, side, why).
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub cite: String,
    pub cond: String,
    /// True, False, or None (the third item is neither)
    pub side: Option<bool>,
    pub why: String,
    pub j: J,
}

impl Check {
    pub fn of(j: &J) -> Check {
        let a = j.arr();
        Check {
            cite: a.first().and_then(J::str).unwrap_or("").to_string(),
            cond: a.get(1).and_then(J::str).unwrap_or("").to_string(),
            side: match a.get(2) {
                Some(J::Bool(b)) => Some(*b),
                _ => None,
            },
            why: a.get(3).and_then(J::str).unwrap_or("").to_string(),
            j: j.clone(),
        }
    }
}

/// reader.Record as the harness reads it (PackedTask._action's `what`).
#[derive(Clone, Debug)]
pub struct Rec {
    pub i: u32,
    pub kind: String,
    pub label: String,
    pub pre: Vec<(Key, Vec<Val>)>,
    pub effects: Vec<(Key, RecEff)>,
    pub checks: Vec<Check>,
    /// the source as its JSON list
    pub src: Vec<J>,
    /// the source's JSON text (the key of source_index)
    pub src_text: String,
    pub engine_pre: Vec<Key>,
}

impl Rec {
    pub fn eff(&self, k: Key) -> Option<&RecEff> {
        self.effects.iter().find(|e| e.0 == k).map(|e| &e.1)
    }
    pub fn src_int(&self, i: usize) -> Option<i64> {
        self.src.get(i).and_then(J::int)
    }
    pub fn src_str(&self, i: usize) -> Option<&str> {
        self.src.get(i).and_then(J::str)
    }
}

/// strict.Reading.rec's effect: ("set", v), ("guess", values) or (kind,).
#[derive(Clone, Debug, PartialEq)]
pub enum B1Eff {
    Set(Val),
    Guess(Arc<[Val]>),
    Other(&'static str),
}

impl B1Eff {
    pub fn kind(&self) -> &'static str {
        match self {
            B1Eff::Set(_) => "set",
            B1Eff::Guess(_) => "guess",
            B1Eff::Other(k) => k,
        }
    }
}

/// strict.Reading.rec(i): (index, unit, wait point, pre, effects, checks), the unobserved facts left out.
#[derive(Clone, Debug)]
pub struct B1 {
    pub idx: u32,
    pub unit: Option<Unit>,
    pub wp: Option<i64>,
    pub pre: Vec<(Key, Vec<Val>)>,
    pub eff: Vec<(Key, B1Eff)>,
    pub checks: Arc<Vec<Check>>,
}

impl B1 {
    pub fn eff(&self, k: Key) -> Option<&B1Eff> {
        self.eff.iter().find(|e| e.0 == k).map(|e| &e.1)
    }
    pub fn pre(&self, k: Key) -> Option<&Vec<Val>> {
        self.pre.iter().find(|e| e.0 == k).map(|e| &e.1)
    }
}

/// A local of a built list: a number, or unknown with its value set (None: any value).
#[derive(Clone, Debug, PartialEq)]
pub enum Loc {
    Num(i64),
    Unknown(Option<Vec<i64>>),
}

/// A slot flag of a built list: true, false, null, or unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    T,
    F,
    Null,
    Unknown,
}

fn flag_of(j: Option<&J>) -> Flag {
    match j {
        Some(J::Bool(true)) => Flag::T,
        Some(J::Bool(false)) => Flag::F,
        Some(J::Null) | None => Flag::Null,
        _ => Flag::Unknown,
    }
}

/// One built list (packed_reading.load_lists' entry).
#[derive(Clone, Debug)]
pub struct List {
    pub sig: i64,
    pub locals: Vec<(i64, Loc)>,
    pub override_: i64,
    pub cut_data: Vec<i64>,
    /// (recursive, resistant, owned), when the line has flags
    pub flags: Option<(Flag, Flag, Flag)>,
    pub flags_j: Option<J>,
}

impl List {
    pub fn local(&self, n: i64) -> Loc {
        self.locals
            .iter()
            .find(|l| l.0 == n)
            .map(|l| l.1.clone())
            .unwrap_or(Loc::Num(0))
    }
}

/// What the window search derives from a reading once (kept across plays of the same reading).
pub struct ReadingData {
    pub id: [u8; 20],
    pub doc: J,
    /// table.points: (K_AT or K_ESC, unit) -> wait points (sorted)
    pub points: FxHashMap<(u8, Unit), Vec<Val>>,
    pub elsewhere: FxHashSet<Key>,
    pub lists: FxHashMap<(Unit, i64), Vec<List>>,
    /// reading.json live_locals' slots, when the reading has them
    pub live_locals: Option<FxHashMap<String, Option<Vec<i64>>>>,
    recs: Mutex<FxHashMap<u32, Arc<Rec>>>,
    acts: Mutex<FxHashMap<u32, Arc<Action>>>,
    b1s: Mutex<FxHashMap<u32, Arc<B1>>>,
    b1_unobserved: Prefixes,
    by_source: OnceLock<FxHashMap<String, Vec<u32>>>,
    events: OnceLock<(FxHashMap<(Unit, i64), Vec<u32>>, FxHashMap<Unit, Vec<u32>>)>,
    pub frame_costs_dmax: Option<i64>,
    /// the reading's facts (trace_explain Code.facts)
    pub facts: Arc<FxHashSet<Key>>,
}

/// The reading as the window search reads it: the port and the data derived from it.
pub struct PlayReading<'r> {
    pub src: &'r dyn ReadingSource,
    pub doms: &'r Domains,
    pub d: Arc<ReadingData>,
}

impl<'r> std::ops::Deref for PlayReading<'r> {
    type Target = ReadingData;
    fn deref(&self) -> &ReadingData {
        &self.d
    }
}

impl ReadingData {
    /// `unobserved_actor_rooms`: strict.Reading leaves out saved: and, with it, actorroom:.
    pub fn new(src: &dyn ReadingSource, unobserved_actor_rooms: bool) -> ReadingData {
        let doc = pyj::parse(src.head()).unwrap_or(J::Null);
        let doms = src.domains();
        let mut points = FxHashMap::default();
        let mut elsewhere = FxHashSet::default();
        for (k, d) in &doms.0 {
            let at = match k.kind() {
                K_POS => K_AT,
                K_ESCPOS => K_ESC,
                _ => continue,
            };
            let ws: Vec<Val> = d
                .values
                .iter()
                .copied()
                .filter(|&v| is_int(v) && v != NOWHERE && v != ELSEWHERE)
                .collect();
            points.insert((at, Unit::of_key(*k)), ws);
            if d.values.contains(&ELSEWHERE) {
                elsewhere.insert(*k);
            }
        }
        let mut lists: FxHashMap<(Unit, i64), Vec<List>> = FxHashMap::default();
        for i in 0..src.n_lists() {
            let ln = src.list_line(i);
            if ln.trim().is_empty() {
                continue;
            }
            let Ok(d) = pyj::parse(ln) else { continue };
            let Some(u) = d.s("name").and_then(Unit::parse) else {
                continue;
            };
            let w = d.i("w").unwrap_or(0);
            let locals = d
                .a("locals")
                .iter()
                .map(|p| {
                    let a = p.arr();
                    let n = a.first().and_then(J::int).unwrap_or(0);
                    let v = match a.get(1) {
                        Some(J::Obj(_)) => {
                            let o = a.get(1).unwrap();
                            Loc::Unknown(
                                o.get("values")
                                    .filter(|x| !x.is_null())
                                    .map(|x| x.arr().iter().filter_map(J::int).collect()),
                            )
                        }
                        Some(x) => Loc::Num(x.int().unwrap_or(0)),
                        None => Loc::Num(0),
                    };
                    (n, v)
                })
                .collect();
            let flags_j = d.get("flags").cloned();
            let flags = flags_j.as_ref().map(|f| {
                (
                    flag_of(f.get("recursive")),
                    flag_of(f.get("resistant")),
                    flag_of(f.get("owned")),
                )
            });
            lists.entry((u, w)).or_default().push(List {
                sig: d.i("sig").unwrap_or(0),
                locals,
                override_: d.i("override").unwrap_or(0),
                cut_data: d.a("cut_data").iter().filter_map(J::int).collect(),
                flags,
                flags_j,
            });
        }
        let live_locals = doc.get("live_locals").and_then(|l| l.get("slots")).map(|s| {
            s.obj()
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        if v.is_null() {
                            None
                        } else {
                            Some(v.arr().iter().filter_map(J::int).collect())
                        },
                    )
                })
                .collect()
        });
        let frame_costs_dmax = doc.get("frame_costs").and_then(|f| f.i("dmax"));
        let mut un: Vec<&'static str> = vec!["saved:"];
        if unobserved_actor_rooms {
            un.push("actorroom:");
        }
        ReadingData {
            id: src.id(),
            facts: Arc::new(doms.0.iter().map(|d| d.0).collect()),
            doc,
            points,
            elsewhere,
            lists,
            live_locals,
            recs: Mutex::new(FxHashMap::default()),
            acts: Mutex::new(FxHashMap::default()),
            b1s: Mutex::new(FxHashMap::default()),
            b1_unobserved: Prefixes::of(&un),
            by_source: OnceLock::new(),
            events: OnceLock::new(),
            frame_costs_dmax,
        }
    }
}

impl<'r> PlayReading<'r> {
    pub fn new(src: &'r dyn ReadingSource, d: Arc<ReadingData>) -> PlayReading<'r> {
        PlayReading {
            src,
            doms: src.domains(),
            d,
        }
    }

    pub fn dom(&self, k: Key) -> Option<&'r Domain> {
        self.doms.get(k)
    }

    /// Points.read_locals: the locals a slot's lists are keyed on (None: every local).
    pub fn read_locals(&self, name: &str, room: i32) -> Option<Vec<i64>> {
        let ll = self.live_locals.as_ref()?;
        let key = if name.starts_with("script:") || name.starts_with("obj:") {
            name.to_string()
        } else {
            format!("{name}@{room}")
        };
        ll.get(&key).cloned().flatten()
    }

    pub fn live(&self) -> bool {
        self.live_locals.as_ref().is_some_and(|l| !l.is_empty()) || self.doc.get("live_locals").is_some_and(J::truthy)
    }

    /// The reading's action i (PackedTask.actions[i]).
    pub fn action(&self, i: u32) -> Arc<Action> {
        if let Some(a) = self.acts.lock().unwrap().get(&i) {
            return a.clone();
        }
        let a = Arc::new(self.src.action(i));
        self.acts.lock().unwrap().insert(i, a.clone());
        a
    }

    /// The record of action i (its `what`).
    pub fn rec(&self, i: u32) -> Arc<Rec> {
        if let Some(r) = self.recs.lock().unwrap().get(&i) {
            return r.clone();
        }
        let r = self.src.record(i);
        let src_j = pyj::parse(&r.source).unwrap_or(J::Arr(vec![]));
        let rec = Arc::new(Rec {
            i,
            kind: r.kind,
            label: r.label,
            pre: r.pre,
            effects: r.effects,
            checks: r
                .checks
                .iter()
                .map(|c| Check::of(&pyj::parse(c).unwrap_or(J::Null)))
                .collect(),
            src_text: pyj::dumps(&src_j),
            src: src_j.arr().to_vec(),
            engine_pre: r.engine_pre,
        });
        self.recs.lock().unwrap().insert(i, rec.clone());
        rec
    }

    /// strict.Reading.rec(i)
    pub fn b1(&self, i: u32) -> Arc<B1> {
        if let Some(r) = self.b1s.lock().unwrap().get(&i) {
            return r.clone();
        }
        let a = self.action(i);
        let w = self.rec(i);
        let (unit, wp) = if w.src_str(0) == Some("event") {
            (w.src_str(1).and_then(Unit::parse), w.src_int(2))
        } else {
            (None, None)
        };
        let un = &self.b1_unobserved;
        let eff = w
            .effects
            .iter()
            .filter(|(k, _)| !un.has(*k))
            .map(|(k, e)| {
                (
                    *k,
                    match e {
                        RecEff::Set(v) => B1Eff::Set(*v),
                        RecEff::Guess(vs) => B1Eff::Guess(vs.clone().into()),
                        RecEff::Add(_) => B1Eff::Other("add"),
                        RecEff::When(..) => B1Eff::Other("when"),
                        RecEff::Copy(_) => B1Eff::Other("copy"),
                    },
                )
            })
            .collect();
        let pre = a.pre.iter().filter(|(k, _)| !un.has(*k)).cloned().collect();
        let b = Arc::new(B1 {
            idx: i,
            unit,
            wp,
            pre,
            eff,
            checks: Arc::new(w.checks.clone()),
        });
        self.b1s.lock().unwrap().insert(i, b.clone());
        b
    }

    /// window.source_index: the reading's actions by their input (the action line's source, as JSON text).
    pub fn by_source(&self) -> &FxHashMap<String, Vec<u32>> {
        self.by_source.get_or_init(|| {
            let mut out: FxHashMap<String, Vec<u32>> = FxHashMap::default();
            for i in 0..self.src.n_actions() {
                if let Some((n, src)) = source_of_line(self.src.action_line(i)) {
                    if let Ok(j) = pyj::parse(src) {
                        out.entry(pyj::dumps(&j)).or_default().push(n);
                    }
                }
            }
            out
        })
    }

    /// strict.Reading's `events` {(unit, w): [action]} and `by_unit` {unit: [action]}.
    pub fn events(&self) -> &(FxHashMap<(Unit, i64), Vec<u32>>, FxHashMap<Unit, Vec<u32>>) {
        self.events.get_or_init(|| {
            let mut ev: FxHashMap<(Unit, i64), Vec<u32>> = FxHashMap::default();
            let mut bu: FxHashMap<Unit, Vec<u32>> = FxHashMap::default();
            for i in 0..self.src.n_actions() {
                let Some((n, src)) = source_of_line(self.src.action_line(i)) else {
                    continue;
                };
                if !src.starts_with("[\"event\"") {
                    continue;
                }
                let Ok(j) = pyj::parse(src) else { continue };
                let a = j.arr();
                if a.len() > 4 && a[4].truthy() {
                    continue;
                }
                let (Some(u), Some(w)) = (
                    a.get(1).and_then(J::str).and_then(Unit::parse),
                    a.get(2).and_then(J::int),
                ) else {
                    continue;
                };
                ev.entry((u, w)).or_default().push(n);
                bu.entry(u).or_default().push(n);
            }
            (ev, bu)
        })
    }
}

/// window._SOURCE on an action line: (its "i", its "source" list's text), the line's head as the reference writes it.
pub fn source_of_line(ln: &str) -> Option<(u32, &str)> {
    let rest = ln.strip_prefix("{\"i\": ")?;
    let d = rest.find(|c: char| !c.is_ascii_digit())?;
    let i: u32 = rest[..d].parse().ok()?;
    let rest = rest[d..].strip_prefix(", \"rec\": ")?;
    let rest = rest.strip_prefix('-').unwrap_or(rest);
    let d = rest.find(|c: char| !c.is_ascii_digit())?;
    if d == 0 {
        return None;
    }
    let rest = rest[d..].strip_prefix(", \"label\": \"")?;
    // the label: (?:[^"\\]|\\.)*
    let b = rest.as_bytes();
    let mut k = 0;
    while k < b.len() && b[k] != b'"' {
        if b[k] == b'\\' {
            k += 1;
        }
        k += 1;
    }
    if k >= b.len() {
        return None;
    }
    let rest = rest[k + 1..].strip_prefix(", \"source\": ")?;
    if !rest.starts_with('[') {
        return None;
    }
    let e = rest.find(']')?;
    Some((i, &rest[..=e]))
}

// --- window.FastView ---------------------------------------------------------------------------------------------

struct PosK<'r> {
    k: Key,
    dom: &'r Domain,
    at: u8,
    unit: Unit,
    /// (the at:/esc: key of each wait point, the point), sorted; None when the scope has no points for the unit
    cands: Option<Vec<(Key, Val)>>,
    ws: &'r [Val],
}

/// window.FastView: replay.view for fixed domains, on their keys only.
pub struct FastView<'r> {
    plain: Vec<(Key, &'r Domain)>,
    pos: Vec<PosK<'r>>,
    elsewhere: &'r FxHashSet<Key>,
}

static EMPTY: [Val; 0] = [];

impl<'r> FastView<'r> {
    pub fn new(
        rd: &'r PlayReading<'_>,
        keys: impl IntoIterator<Item = (Key, &'r Domain)>,
        unobserved: &Prefixes,
    ) -> FastView<'r> {
        let mut plain = Vec::new();
        let mut pos = Vec::new();
        for (k, dom) in keys {
            if unobserved.has(k) {
                continue;
            }
            let at = match k.kind() {
                K_POS => K_AT,
                K_ESCPOS => K_ESC,
                _ => {
                    plain.push((k, dom));
                    continue;
                }
            };
            let unit = Unit::of_key(k);
            let ws = rd.points.get(&(at, unit));
            let cands = ws.map(|ws| {
                ws.iter()
                    .map(|&w| (Key::pack(at, unit.0, unit.1, w as u64), w))
                    .collect()
            });
            // (ws are sorted ascending: Python's sorted(ws))
            pos.push(PosK {
                k,
                dom,
                at,
                unit,
                cands,
                ws: ws.map(|v| v.as_slice()).unwrap_or(&EMPTY),
            });
        }
        // the plain keys' order does not matter (a view is read by key)
        FastView {
            plain,
            pos,
            elsewhere: &rd.elsewhere,
        }
    }

    pub fn n_keys(&self) -> usize {
        self.plain.len() + self.pos.len()
    }

    pub fn view(&self, obs: &Facts) -> View {
        let mut out = View::with_capacity(self.n_keys() + 8);
        for &(k, dom) in &self.plain {
            out.setv(k, dom.label(obs.val(k)));
        }
        for p in &self.pos {
            let mut v = NOWHERE;
            if let Some(c) = &p.cands {
                if let Some(&(_, w)) = c.iter().find(|(key, _)| obs.get(*key) == Some(1)) {
                    v = w;
                }
            }
            if v == NOWHERE && self.elsewhere.contains(&p.k) {
                // a slot at a wait point none of the scope's points
                let lo = Key::pack(p.at, p.unit.0, p.unit.1, 0);
                let hi = Key::pack(p.at, p.unit.0, p.unit.1, (1 << 24) - 1);
                let st = obs.0.partition_point(|x| x.0 < lo);
                if obs.0[st..]
                    .iter()
                    .take_while(|x| x.0 <= hi)
                    .any(|x| x.1 == 1 && !p.ws.contains(&(x.0.b() as Val)))
                {
                    v = ELSEWHERE;
                }
            }
            out.setv(p.k, p.dom.label(v));
        }
        out
    }
}

// --- replay.dumped_slots, live_match, list_matches, match_list ---------------------------------------------------

/// One live slot of a dump with locals (replay.dumped_slots' entry).
#[derive(Clone, Debug, PartialEq)]
pub struct DSlot {
    pub name: String,
    pub unit: Unit,
    pub room: i32,
    pub w: i64,
    pub esc: Option<i64>,
    pub locals: Vec<i32>,
    pub override_: i64,
    pub cut_data: Vec<i64>,
    pub owned: bool,
    /// (resistant, recursive, owned) when the dump has the flags
    pub flags: Option<(bool, bool, bool)>,
}

pub const WIO_INVENTORY_: i32 = 0;
pub const OF_OWNER_ROOM: i32 = 0x0F;

/// replay.dumped_slots: the live slots by unit, and the skipped ones; None when the dump has no locals.
#[allow(clippy::type_complexity)]
pub fn dumped_slots(s: &Obs) -> Option<(FxHashMap<Unit, DSlot>, Vec<(String, String)>)> {
    if s.slots.iter().any(|sl| sl.locals.is_none()) {
        return None;
    }
    let depth = s.cutscene.depth;
    let levels = &s.cutscene.levels;
    let top = levels.last().and_then(|l| l.override_.as_ref());
    let esc_unit = top
        .filter(|t| !t.stale && t.code >= 0)
        .and_then(|t| Unit::of(t.where_, t.number));
    let mut names: FxHashMap<Option<Unit>, usize> = FxHashMap::default();
    for sl in &s.slots {
        *names.entry(Unit::of(sl.where_, sl.number)).or_default() += 1;
    }
    let mut out = FxHashMap::default();
    let mut skipped = Vec::new();
    for sl in &s.slots {
        let name = Unit::of(sl.where_, sl.number);
        let Some(u) = name.filter(|_| sl.code >= 0) else {
            skipped.push(("no unit".into(), format!("{} {}", sl.where_, sl.number)));
            continue;
        };
        if names[&name] > 1 {
            skipped.push(("two slots".into(), u.text()));
            continue;
        }
        let ov = sl.cutscene_override;
        let cut: Vec<i64> = if 0 < ov && ov <= depth {
            let lo = (depth - ov + 1).max(0) as usize;
            let hi = ((depth + 1) as usize).min(levels.len());
            levels
                .get(lo..hi)
                .unwrap_or(&[])
                .iter()
                .map(|l| l.data.unwrap_or(0) as i64)
                .collect()
        } else {
            vec![]
        };
        let obj = if u.0 == U_OBJ { Some(u.1 as u32) } else { None };
        let owner = obj
            .and_then(|o| s.owners.iter().find(|p| p.0 == o).map(|p| p.1))
            .unwrap_or(0);
        let flags = match (sl.resistant, sl.recursive) {
            (Some(r), Some(c)) => Some((r, c, obj.is_some() && sl.where_ == WIO_INVENTORY_)),
            _ => None,
        };
        out.insert(
            u,
            DSlot {
                name: u.text(),
                unit: u,
                room: s.room,
                w: sl.code,
                esc: if esc_unit == Some(u) { top.map(|t| t.code) } else { None },
                locals: sl.locals.clone().unwrap_or_default(),
                override_: if cut.len() as i64 == ov as i64 { ov as i64 } else { 0 },
                cut_data: cut,
                owned: obj.is_some() && owner != OF_OWNER_ROOM,
                flags,
            },
        );
    }
    Some((out, skipped))
}

/// replay.live_match: None off --live-locals; else (the locals the unit reads, the dump's flags). Err: a slot with no
/// flags under the switch.
pub type Live = Option<(Option<Vec<i64>>, (bool, bool, bool))>;

pub fn live_match(rd: &PlayReading, slot: &DSlot) -> Result<Live, String> {
    if rd.live_locals.as_ref().is_none_or(|l| l.is_empty()) {
        return Ok(None);
    }
    let Some(f) = slot.flags else {
        return Err(format!(
            "F10 under --live-locals: the dump gives slot {} no flags (resistant, recursive), and a list's number covers \
             them; dump the slots with a build whose SPEEDRUN_DUMP_LOCALS lists them",
            slot.name
        ));
    };
    Ok(Some((rd.read_locals(&slot.name, slot.room), f)))
}

/// replay.list_matches
pub fn list_matches(lst: &List, slot: &DSlot, live: &Live) -> Result<bool, String> {
    if lst.override_ != slot.override_ || lst.cut_data != slot.cut_data {
        return Ok(false);
    }
    let mut reads: Option<&Vec<i64>> = None;
    if let Some((r, (resistant, recursive, owned))) = live {
        reads = r.as_ref();
        let Some(lf) = lst.flags else {
            return Err("F10 under --live-locals: the reading's lists file has no slot flags (an older packed_reading.py made it)".into());
        };
        let want = (
            if *recursive { Flag::T } else { Flag::Null },
            if *resistant { Flag::T } else { Flag::F },
            if *owned { Flag::T } else { Flag::F },
        );
        if lf != want {
            return Ok(false);
        }
    }
    if lst
        .locals
        .iter()
        .any(|(n, _)| !(0 <= *n && (*n as usize) < slot.locals.len()))
    {
        return Ok(false);
    }
    for (n, &x) in slot.locals.iter().enumerate() {
        if let Some(r) = reads {
            if !r.contains(&(n as i64)) {
                continue;
            }
        }
        match lst.local(n as i64) {
            Loc::Unknown(Some(vs)) => {
                if !vs.contains(&(x as i64)) {
                    return Ok(false);
                }
            }
            Loc::Unknown(None) => {}
            Loc::Num(v) => {
                if v != x as i64 {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// replay.match_list: the list with the most locals known as numbers, the first among equals.
pub fn match_list<'a>(rd: &'a PlayReading, slot: &DSlot, w: i64, live: &Live) -> Result<Option<&'a List>, String> {
    let mut best: Option<(usize, &List)> = None;
    for lst in rd.lists.get(&(slot.unit, w)).map(|v| v.as_slice()).unwrap_or(&[]) {
        if list_matches(lst, slot, live)? {
            let known = lst.locals.iter().filter(|(_, v)| matches!(v, Loc::Num(_))).count();
            if best.is_none_or(|b| known > b.0) {
                best = Some((known, lst));
            }
        }
    }
    Ok(best.map(|b| b.1))
}

// --- strict.ArgsView -----------------------------------------------------------------------------------------------

type ArgsKey = (Unit, i64, Vec<i32>, i64, Vec<i64>, Option<(bool, bool, bool)>);

/// strict.ArgsView: `args:U` from the dumped locals, the built lists at U's wait point they match ({0} when none
/// does or U has no live slot at a point).
pub struct ArgsView<'a> {
    rd: &'a PlayReading<'a>,
    pub units: Vec<Unit>,
    cache: Mutex<FxHashMap<ArgsKey, Arc<[Val]>>>,
}

impl<'a> ArgsView<'a> {
    pub fn new(rd: &'a PlayReading<'a>, mut units: Vec<Unit>) -> ArgsView<'a> {
        units.sort_by_cached_key(|u| u.text());
        ArgsView {
            rd,
            units,
            cache: Mutex::new(FxHashMap::default()),
        }
    }
    pub fn apply(&self, dump: &Obs, view: &mut View) -> Result<(), String> {
        if self.units.is_empty() {
            return Ok(());
        }
        let ds = dumped_slots(dump);
        let zero: Arc<[Val]> = Arc::from(vec![0i64]);
        for &u in &self.units {
            let k = u.args();
            let pos = view.val(u.pos());
            let slot = ds.as_ref().and_then(|d| d.0.get(&u));
            let Some(slot) = slot.filter(|_| is_int(pos) && pos >= 0) else {
                view.set(k, VV::Set(zero.clone()));
                continue;
            };
            let key: ArgsKey = (
                u,
                pos,
                slot.locals.clone(),
                slot.override_,
                slot.cut_data.clone(),
                slot.flags,
            );
            let got = self.cache.lock().unwrap().get(&key).cloned();
            let sigs = match got {
                Some(s) => s,
                None => {
                    let live = live_match(self.rd, slot)?;
                    let mut sigs: Vec<Val> = Vec::new();
                    for lst in self.rd.lists.get(&(u, pos)).map(|v| v.as_slice()).unwrap_or(&[]) {
                        if list_matches(lst, slot, &live)? {
                            sigs.push(lst.sig);
                        }
                    }
                    sigs.sort();
                    sigs.dedup();
                    if sigs.is_empty() {
                        sigs.push(0);
                    }
                    let s: Arc<[Val]> = Arc::from(sigs);
                    self.cache.lock().unwrap().insert(key, s.clone());
                    s
                }
            };
            view.set(k, VV::Set(sigs));
        }
        Ok(())
    }
}
