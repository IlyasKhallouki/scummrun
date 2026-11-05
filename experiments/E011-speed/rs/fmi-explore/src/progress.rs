//! E012 component `archive` (experiments/E012-game-mechanics/plan.md, sections 5 and 6, and the amendment "component
//! `archive`"): the archive's tested atoms T, novelty over T and the rooms, the dead and exhausted marks, the cells over
//! T and the order by real progress, behind `FMI_SELECT=progress`. The archive's hooks are in `archive.rs`; this module
//! holds what they use and what a try needs (`TAtom`, `t_atoms`, `Novel`).
//!
//! Everything here is ordering and bookkeeping: no state is pruned, every live entry stays selectable (E009's
//! argument), and a dead entry is one whose run ended in the game's own end (component `end`).

use crate::archive::{FactLook, NOT_INT};
use fmi_core::{Domain, Domains, Key, Val, ABOVE, BELOW};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The switch's variable and value (plan.md, the component table).
pub const SELECT_ENV: &str = "FMI_SELECT";
pub const PROGRESS: &str = "progress";

/// `FMI_SELECT=progress`.
pub fn on() -> bool {
    std::env::var(SELECT_ENV).map(|v| v == PROGRESS).unwrap_or(false)
}

/// The script-control kinds a cell leaves out: the slot, the position, the arguments and the freeze (plan.md 6).
pub const CONTROL_PREFIXES: [&str; 9] =
    ["script:", "local:", "objscript:", "pos:", "escpos:", "at:", "esc:", "args:", "frozen:"];

/// Whether a key's text is a script-control key (`CONTROL_PREFIXES`).
pub fn control_key(k: &str) -> bool {
    CONTROL_PREFIXES.iter().any(|p| k.starts_with(p))
}

/// A tested atom as the archive holds it (the amendment): the key, the set's ints, and the domain's bounds where the
/// set held `below` (lo) or `above` (hi).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TAtom {
    pub key: String,
    pub vals: Vec<i64>,
    pub lo: Option<i64>,
    pub hi: Option<i64>,
}

impl TAtom {
    /// The atom of a reading's (key, value set) under the key's domain: `fmi_reading::tested::atom_holds(dom, vs, v)`
    /// for every engine value v. A set's int that the domain would not label as itself is left out (it can never
    /// hold); `below` and `above` hold only where the domain has the bound.
    pub fn of(k: Key, vs: &[Val], dom: Option<&Domain>) -> TAtom {
        let mut vals: Vec<i64> = vs
            .iter()
            .copied()
            .filter(|&v| v != BELOW && v != ABOVE && v != fmi_core::NONE)
            .filter(|&v| dom.map(|d| d.label(v) == v).unwrap_or(true))
            .collect();
        vals.sort_unstable();
        vals.dedup();
        let lo = if vs.contains(&BELOW) { dom.and_then(|d| d.low) } else { None };
        let hi = if vs.contains(&ABOVE) { dom.and_then(|d| d.high) } else { None };
        TAtom { key: k.text(), vals, lo, hi }
    }

    /// An atom of one value (the goal's).
    pub fn value(key: &str, v: i64) -> TAtom {
        TAtom { key: key.to_string(), vals: vec![v], lo: None, hi: None }
    }

    /// Whether the atom holds at value `v` (facts.py's value: an absent key is 0).
    pub fn holds(&self, v: i64) -> bool {
        v != NOT_INT
            && (self.vals.binary_search(&v).is_ok()
                || self.lo.map(|l| v < l).unwrap_or(false)
                || self.hi.map(|h| v > h).unwrap_or(false))
    }

    /// Whether it holds in a state's facts.
    pub fn holds_in<F: FactLook + ?Sized>(&self, f: &F) -> bool {
        self.holds(f.look(&self.key).unwrap_or(0))
    }

    /// `k=a|b|<lo|>hi`
    pub fn text(&self) -> String {
        let mut parts: Vec<String> = self.vals.iter().map(|v| v.to_string()).collect();
        if let Some(l) = self.lo {
            parts.push(format!("<{l}"));
        }
        if let Some(h) = self.hi {
            parts.push(format!(">{h}"));
        }
        format!("{}={}", self.key, parts.join("|"))
    }

    /// `[key, [ints], lo, hi]`
    pub fn json(&self) -> Value {
        json!([self.key, self.vals, self.lo, self.hi])
    }

    pub fn of_json(v: &Value) -> Option<TAtom> {
        let a = v.as_array()?;
        let key = a.first()?.as_str()?.to_string();
        let mut vals: Vec<i64> = a.get(1)?.as_array()?.iter().map(|x| x.as_i64()).collect::<Option<Vec<i64>>>()?;
        vals.sort_unstable();
        vals.dedup();
        Some(TAtom { key, vals, lo: a.get(2).and_then(|x| x.as_i64()), hi: a.get(3).and_then(|x| x.as_i64()) })
    }

    pub fn control(&self) -> bool {
        control_key(&self.key)
    }
}

/// A reading's T as the archive's atoms: `t_atoms(&Tested::of(&rd).flat(), rd.domains())`, in the given order, each
/// atom once.
pub fn t_atoms(flat: &[(Key, Vec<Val>)], doms: &Domains) -> Vec<TAtom> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(flat.len());
    for (k, vs) in flat {
        let a = TAtom::of(*k, vs, doms.get(*k));
        if seen.insert(a.clone()) {
            out.push(a);
        }
    }
    out
}

/// The atoms of a JSON list (result.json "tested", a log line's "t_new", job.json's "novelty"."t").
pub fn atoms_of_json(v: Option<&Value>) -> Vec<TAtom> {
    v.and_then(|x| x.as_array()).map(|a| a.iter().filter_map(TAtom::of_json).collect()).unwrap_or_default()
}

pub fn atoms_json(t: &[TAtom]) -> Value {
    Value::Array(t.iter().map(|a| a.json()).collect())
}

/// The goal's atoms ({key: [values]}): one atom per key with the goal's values.
pub fn goal_atoms(goal: &[(String, Vec<i64>)]) -> Vec<TAtom> {
    goal.iter()
        .map(|(k, vs)| {
            let mut vals = vs.clone();
            vals.sort_unstable();
            vals.dedup();
            TAtom { key: k.clone(), vals, lo: None, hi: None }
        })
        .collect()
}

// --- the entries' last facts, kept compact ---------------------------------------------------------------------------

/// Every row's last facts, as the root's facts and each other row's differences from them (an absent key is 0, as
/// facts.py reads a map). The archive keys every row again from here when T grows, with no file read.
#[derive(Default, Debug)]
pub struct FactStore {
    ids: HashMap<String, u32>,
    pub names: Vec<String>,
    /// per key id: the root's value (0 when absent)
    root: Vec<i64>,
    /// per row: (key id, value) where the value is not the root's, ascending
    rows: Vec<Vec<(u32, i64)>>,
    /// the key ids of `owner:O` keys, with O
    owners: Vec<(u32, i64)>,
}

impl FactStore {
    fn intern(&mut self, k: &str) -> u32 {
        if let Some(&i) = self.ids.get(k) {
            return i;
        }
        let i = self.names.len() as u32;
        self.names.push(k.to_string());
        self.ids.insert(k.to_string(), i);
        self.root.push(0);
        if let Some(o) = k.strip_prefix("owner:").and_then(|o| o.parse::<i64>().ok()) {
            self.owners.push((i, o));
        }
        i
    }

    /// Row i's facts as pairs (`put_each`).
    pub fn put(&mut self, i: usize, pairs: &[(String, i64)]) {
        self.put_each(i, &|f: &mut dyn FnMut(&str, i64)| {
            for (k, v) in pairs {
                f(k, *v);
            }
        });
    }

    /// Row i's facts, given as every (key, value) pair through `each` (rows come in id order, row 0 the root; a row
    /// given again replaces its facts, the root's only before any other row).
    pub fn put_each(&mut self, i: usize, each: &dyn Fn(&mut dyn FnMut(&str, i64))) {
        if i == 0 {
            self.root.iter_mut().for_each(|v| *v = 0);
            let mut got: Vec<(u32, i64)> = Vec::new();
            each(&mut |k, v| got.push((self.intern(k), v)));
            for (id, v) in got {
                self.root[id as usize] = v;
            }
            if self.rows.is_empty() {
                self.rows.push(Vec::new());
            } else {
                self.rows[0].clear();
            }
            return;
        }
        let mut seen = vec![false; self.names.len()];
        let mut diff: Vec<(u32, i64)> = Vec::new();
        each(&mut |k, v| {
            let id = self.intern(k);
            if (id as usize) < seen.len() {
                seen[id as usize] = true;
            }
            if v != self.root[id as usize] {
                diff.push((id, v));
            }
        });
        for (id, s) in seen.iter().enumerate() {
            if !s && self.root[id] != 0 {
                diff.push((id as u32, 0));
            }
        }
        diff.sort_unstable();
        diff.dedup_by_key(|p| p.0);
        while self.rows.len() <= i {
            self.rows.push(Vec::new());
        }
        self.rows[i] = diff;
    }

    pub fn has(&self, i: usize) -> bool {
        i < self.rows.len()
    }

    /// Row i's value of key `k` (0 when absent).
    pub fn get(&self, i: usize, k: &str) -> i64 {
        match self.ids.get(k) {
            None => 0,
            Some(&id) => self.get_id(i, id),
        }
    }

    fn get_id(&self, i: usize, id: u32) -> i64 {
        match self.rows[i].binary_search_by_key(&id, |p| p.0) {
            Ok(j) => self.rows[i][j].1,
            Err(_) => self.root[id as usize],
        }
    }

    pub fn view(&self, i: usize) -> RowFacts<'_> {
        RowFacts { st: self, i }
    }

    /// Per key, the values the given rows hold (absent: 0), keys sorted.
    pub fn seen(&self, rows: &[usize]) -> BTreeMap<String, Vec<i64>> {
        let mut vals: Vec<BTreeSet<i64>> = vec![BTreeSet::new(); self.names.len()];
        let mut count: Vec<usize> = vec![0; self.names.len()];
        for &i in rows {
            for &(id, v) in &self.rows[i] {
                vals[id as usize].insert(v);
                count[id as usize] += 1;
            }
        }
        let mut out = BTreeMap::new();
        for (id, name) in self.names.iter().enumerate() {
            let mut s = std::mem::take(&mut vals[id]);
            if count[id] < rows.len() {
                s.insert(self.root[id]);
            }
            if !s.is_empty() {
                out.insert(name.clone(), s.into_iter().collect());
            }
        }
        out
    }

    /// The differences kept, in all (the store's size).
    pub fn cells(&self) -> usize {
        self.rows.iter().map(|r| r.len()).sum::<usize>() + self.root.len()
    }
}

/// One row of a `FactStore` as a facts map.
pub struct RowFacts<'a> {
    st: &'a FactStore,
    i: usize,
}

impl FactLook for RowFacts<'_> {
    fn look(&self, k: &str) -> Option<i64> {
        self.st.ids.get(k).map(|&id| self.st.get_id(self.i, id))
    }
    fn owners(&self) -> Vec<(i64, i64)> {
        self.st.owners.iter().map(|&(id, o)| (o, self.st.get_id(self.i, id))).collect()
    }
}

// --- the archive's progress state ------------------------------------------------------------------------------------

/// What the archive keeps for the order of plan.md 6 (the amendment): T, the rows' last facts, the atoms each row
/// holds, the least live holder of each atom, the live rooms and the exhausted marks.
#[derive(Default, Debug)]
pub struct Progress {
    /// T, in the order its atoms came
    pub t: Vec<TAtom>,
    t_ix: HashMap<TAtom, u32>,
    /// per atom: its text, and whether its key is a script-control key
    texts: Vec<String>,
    control: Vec<bool>,
    /// T's keys, each with its atoms' ids, in the order the keys came
    keys: Vec<(String, Vec<u32>)>,
    key_ix: HashMap<String, usize>,
    pub store: FactStore,
    /// per row: the ids of T's atoms its last facts hold, ascending
    pub held: Vec<Vec<u32>>,
    /// atom id -> (frame, id) of its least live holder
    pub first: HashMap<u32, (i64, usize)>,
    /// room -> its live entries
    pub rooms: BTreeMap<i64, usize>,
    /// E016 amendment (10) (FMI_NOVEL_PLACE=1): place text (`place_text`) -> its live entries
    pub places: BTreeMap<String, usize>,
    /// entry -> |T| when it was marked exhausted
    pub exhausted: HashMap<usize, usize>,
    /// row -> the distinct rooms of the entries from the root to it
    pub chain_rooms: HashMap<usize, std::rc::Rc<BTreeSet<i64>>>,
}

impl Progress {
    pub fn new(goal: &[(String, Vec<i64>)]) -> Progress {
        let mut p = Progress::default();
        p.merge(&goal_atoms(goal));
        p
    }

    /// Adds the atoms T does not have; returns them (in order).
    pub fn merge(&mut self, atoms: &[TAtom]) -> Vec<TAtom> {
        let mut new = Vec::new();
        for a in atoms {
            if !self.t_ix.contains_key(a) {
                let id = self.t.len() as u32;
                self.t_ix.insert(a.clone(), id);
                self.texts.push(a.text());
                self.control.push(a.control());
                match self.key_ix.get(&a.key) {
                    Some(&k) => self.keys[k].1.push(id),
                    None => {
                        self.key_ix.insert(a.key.clone(), self.keys.len());
                        self.keys.push((a.key.clone(), vec![id]));
                    }
                }
                self.t.push(a.clone());
                new.push(a.clone());
            }
        }
        new
    }

    pub fn atom_id(&self, a: &TAtom) -> Option<u32> {
        self.t_ix.get(a).copied()
    }

    /// The ids of T's atoms a state holds, ascending.
    pub fn held_of<F: FactLook + ?Sized>(&self, f: &F) -> Vec<u32> {
        let mut out = Vec::new();
        for (k, ids) in &self.keys {
            let v = f.look(k).unwrap_or(0);
            out.extend(ids.iter().copied().filter(|&j| self.t[j as usize].holds(v)));
        }
        out.sort_unstable();
        out
    }

    /// An atom's text.
    pub fn text(&self, j: u32) -> &str {
        &self.texts[j as usize]
    }

    /// The texts of the atoms a cell keys on (the control keys' left out), sorted.
    pub fn cell_atoms<F: FactLook + ?Sized>(&self, f: &F) -> Vec<String> {
        let mut out: Vec<&str> =
            self.held_of(f).into_iter().filter(|&j| !self.control[j as usize]).map(|j| self.texts[j as usize].as_str()).collect();
        out.sort_unstable();
        out.into_iter().map(|t| t.to_string()).collect()
    }

    /// Forget every row's derived state (a rekey makes it again).
    pub fn clear_rows(&mut self) {
        self.held.clear();
        self.first.clear();
        self.rooms.clear();
        self.places.clear();
    }

    /// Row i (placed in id order): its held atoms from the store, and, when live, its share of the least holders and
    /// the rooms.
    pub fn place(&mut self, i: usize, frame: i64, room: Option<i64>, live: bool) {
        let h = self.held_of(&self.store.view(i));
        if live {
            for &a in &h {
                match self.first.get(&a) {
                    Some(&o) if o <= (frame, i) => {}
                    _ => {
                        self.first.insert(a, (frame, i));
                    }
                }
            }
            if let Some(r) = room {
                *self.rooms.entry(r).or_insert(0) += 1;
                if place_on() {
                    let p = place_text(r, &self.store.view(i));
                    *self.places.entry(p).or_insert(0) += 1;
                }
            }
        }
        while self.held.len() <= i {
            self.held.push(Vec::new());
        }
        self.held[i] = h;
    }

    /// Per row, the atoms whose least live holder it is.
    pub fn novel_counts(&self) -> HashMap<usize, i64> {
        let mut out = HashMap::new();
        for &(_, i) in self.first.values() {
            *out.entry(i).or_insert(0) += 1;
        }
        out
    }

    /// Whether entry i is exhausted under the current T.
    pub fn exhausted_now(&self, i: usize) -> bool {
        self.exhausted.get(&i).map(|&n| n >= self.t.len()).unwrap_or(false)
    }
}

// --- novelty for a try -------------------------------------------------------------------------------------------------

/// What the archive's live entries hold (job.json "novelty"): their rooms and, per key, the values their last facts
/// hold.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    pub rooms: BTreeSet<i64>,
    pub vals: HashMap<String, Vec<i64>>,
    /// E016 amendment (10): the live entries' places (None: job.json has none, the switch was off)
    pub places: Option<BTreeSet<String>>,
    /// E018 (FMI_TARGET_PLACE=1): per place, the values its live entries hold, as `vals` for the whole archive (None:
    /// the switch was off)
    pub by_place: Option<HashMap<String, HashMap<String, Vec<i64>>>>,
}

/// E016 amendment (10) (`FMI_NOVEL_PLACE=1`): a state is novel too when its place, the room with the objects ego
/// owns (the cell key's first two fields), is one no live entry holds. Carrying an object into a room seen without it
/// is a pair of facts that single atoms never make novel.
pub const PLACE_ENV: &str = "FMI_NOVEL_PLACE";

pub fn place_on() -> bool {
    std::env::var(PLACE_ENV).is_ok_and(|v| v == "1")
}

/// The place of a state in room `room`: `place=R:o1,o2` with the objects ego owns ascending, none when ego is 0 or
/// unknown (as `Archive::cell_key`).
pub fn place_text<F: FactLook + ?Sized>(room: i64, f: &F) -> String {
    let mut owned: Vec<i64> = match f.look("ego") {
        Some(ego) if ego != 0 => f.owners().into_iter().filter(|&(_, v)| v == ego).map(|(o, _)| o).collect(),
        _ => Vec::new(),
    };
    owned.sort_unstable();
    owned.dedup();
    format!("place={room}:{}{}", owned.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(","), crate::ctx::suffix(room, f))
}

impl Seen {
    pub fn of_json(v: &Value) -> Seen {
        let rooms = v.get("rooms").and_then(|r| r.as_array()).map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
        let vals = v
            .get("seen")
            .and_then(|s| s.as_object())
            .map(|m| {
                m.iter()
                    .map(|(k, vs)| {
                        let mut v: Vec<i64> = vs.as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
                        v.sort_unstable();
                        (k.clone(), v)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let places = v.get("places").and_then(|p| p.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).map(String::from).collect());
        let vals_of = |m: &Map<String, Value>| -> HashMap<String, Vec<i64>> {
            m.iter()
                .map(|(k, vs)| {
                    let mut v: Vec<i64> = vs.as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
                    v.sort_unstable();
                    (k.clone(), v)
                })
                .collect()
        };
        let by_place = v.get("by_place").and_then(|b| b.as_object())
            .map(|m| m.iter().filter_map(|(p, s)| Some((p.clone(), vals_of(s.as_object()?)))).collect());
        Seen { rooms, vals, places, by_place }
    }

    /// Whether some live entry holds the atom (a key no entry has is 0 in every one).
    pub fn holds(&self, a: &TAtom) -> bool {
        match self.vals.get(&a.key) {
            Some(vs) => vs.iter().any(|&v| a.holds(v)),
            None => a.holds(0),
        }
    }
}

/// The archive's novelty for a try (IW(1) at the archive's level, plan.md 1 "Kept" and 4): the atoms of T and the
/// room a state holds that no live archive entry holds.
#[derive(Clone, Debug)]
pub struct Novel {
    pub seen: Seen,
    /// T's atoms no live entry holds (the only ones that can be novel)
    pub open: Vec<TAtom>,
    /// T's size
    pub t: usize,
    /// T itself (E018's novelty per place tests every atom against the place's values)
    t_atoms: Vec<TAtom>,
}

impl Novel {
    pub fn new(seen: Seen, t: &[TAtom]) -> Novel {
        let open = t.iter().filter(|a| !seen.holds(a)).cloned().collect();
        let t_atoms = if seen.by_place.is_some() { t.to_vec() } else { Vec::new() };
        Novel { seen, open, t: t.len(), t_atoms }
    }

    /// From job.json's "novelty" and the try's T (None: the archive's T that job.json gives). None when job.json has
    /// no "novelty" (the switch is off).
    pub fn of_job(job: &Value, t: Option<Vec<TAtom>>) -> Option<Novel> {
        let nv = job.get("novelty")?;
        let t = t.unwrap_or_else(|| atoms_of_json(nv.get("t")));
        Some(Novel::new(Seen::of_json(nv), &t))
    }

    /// The novel atoms' texts (T's order), then `room=R` when the room is novel, then `place=R:..` when the place is
    /// (amendment (10)).
    pub fn novel<F: FactLook + ?Sized>(&self, f: &F) -> Vec<String> {
        let mut out: Vec<String> = match self.place_vals(f) {
            // E018: the atoms of T no live entry at the state's place holds (a place with no entry is novel below)
            Some(Some(vals)) => self.t_atoms.iter().filter(|a| a.holds_in(f) && !held_in(vals, a)).map(|a| a.text()).collect(),
            Some(None) => Vec::new(),
            None => self.open.iter().filter(|a| a.holds_in(f)).map(|a| a.text()).collect(),
        };
        if let Some(r) = f.look("room") {
            if r != NOT_INT && !self.seen.rooms.contains(&r) {
                out.push(format!("room={r}"));
            }
        }
        if let Some(p) = self.novel_place(f) {
            out.push(p);
        }
        out
    }

    pub fn is_novel<F: FactLook + ?Sized>(&self, f: &F) -> bool {
        if let Some(r) = f.look("room") {
            if r != NOT_INT && !self.seen.rooms.contains(&r) {
                return true;
            }
        }
        if self.novel_place(f).is_some() {
            return true;
        }
        match self.place_vals(f) {
            Some(Some(vals)) => self.t_atoms.iter().any(|a| a.holds_in(f) && !held_in(vals, a)),
            Some(None) => false,
            None => self.open.iter().any(|a| a.holds_in(f)),
        }
    }

    /// E018: with the places' values in job.json, those of the state's place (Some(None): a place with no live entry,
    /// or no room); None without them.
    fn place_vals<F: FactLook + ?Sized>(&self, f: &F) -> Option<Option<&HashMap<String, Vec<i64>>>> {
        let bp = self.seen.by_place.as_ref()?;
        let Some(r) = f.look("room").filter(|&r| r != NOT_INT) else { return Some(None) };
        Some(bp.get(&place_text(r, f)))
    }

    /// The state's place when the live entries' places are known and none is it.
    fn novel_place<F: FactLook + ?Sized>(&self, f: &F) -> Option<String> {
        let places = self.seen.places.as_ref()?;
        let r = f.look("room").filter(|&r| r != NOT_INT)?;
        let p = place_text(r, f);
        (!places.contains(&p)).then_some(p)
    }
}

/// Whether some entry whose values are `vals` holds the atom (`Seen::holds` over one place's values).
fn held_in(vals: &HashMap<String, Vec<i64>>, a: &TAtom) -> bool {
    match vals.get(&a.key) {
        Some(vs) => vs.iter().any(|&v| a.holds(v)),
        None => a.holds(0),
    }
}

/// E018: job.json's "novelty"."by_place": per place of the live rows (`place_text`), the values they hold, as
/// "seen" gives them for the whole archive.
pub fn by_place_json(p: &Progress, live: &[usize], room_of: impl Fn(usize) -> Option<i64>) -> Value {
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for &i in live {
        if let Some(r) = room_of(i) {
            groups.entry(place_text(r, &p.store.view(i))).or_default().push(i);
        }
    }
    let mut out = Map::new();
    for (pl, rows) in groups {
        let mut seen = Map::new();
        for (k, vs) in p.store.seen(&rows) {
            if vs != [0] {
                seen.insert(k, json!(vs));
            }
        }
        out.insert(pl, Value::Object(seen));
    }
    Value::Object(out)
}

/// job.json's "novelty" from the progress state and the live rows. A key whose live values are only 0 is left out:
/// `Seen` reads an absent key as 0 in every entry, the same.
pub fn novelty_json(p: &Progress, live: &[usize]) -> Value {
    let mut seen = Map::new();
    for (k, vs) in p.store.seen(live) {
        if vs != [0] {
            seen.insert(k, json!(vs));
        }
    }
    let mut nv = json!({"t": atoms_json(&p.t), "rooms": p.rooms.keys().collect::<Vec<_>>(), "seen": Value::Object(seen)});
    if place_on() {
        nv["places"] = json!(p.places.keys().collect::<Vec<_>>());
    }
    nv
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::FactMap;

    #[test]
    fn atom_of_a_set_is_atom_holds_through_the_label() {
        let d = Domain { values: vec![BELOW, 0, 1, 2, ABOVE], low: Some(0), high: Some(2) };
        let k = Key::parse("var:5");
        for vs in [vec![ABOVE], vec![BELOW, 0], vec![1, 2], vec![BELOW, 1, ABOVE], vec![0], vec![7]] {
            let a = TAtom::of(k, &vs, Some(&d));
            for v in -3..6 {
                let want = vs.contains(&d.label(v));
                assert_eq!(a.holds(v), want, "{vs:?} at {v}");
            }
            assert!(!a.holds(NOT_INT));
        }
        let none = TAtom::of(k, &[BELOW, 3], None);
        assert_eq!(none, TAtom { key: "var:5".into(), vals: vec![3], lo: None, hi: None });
        assert!(none.holds(3) && !none.holds(-100));
        let a = TAtom::of(k, &[BELOW, 1, ABOVE], Some(&d));
        assert_eq!(a.text(), "var:5=1|<0|>2");
        assert_eq!(TAtom::of_json(&a.json()), Some(a.clone()));
        assert_eq!(a.json(), json!(["var:5", [1], 0, 2]));
    }

    #[test]
    fn control_keys() {
        for k in ["script:3", "local:211", "objscript:316", "pos:local:211", "escpos:script:1", "at:local:208:12",
                  "esc:local:1:4", "args:script:14", "frozen:local:211"] {
            assert!(control_key(k), "{k}");
        }
        for k in ["var:103", "state:316", "owner:566", "room", "bit:7", "class:316:6", "cutscene", "verb:120", "ego"] {
            assert!(!control_key(k), "{k}");
        }
    }

    fn fm(p: &[(&str, i64)]) -> FactMap {
        p.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn pairs(f: &FactMap) -> Vec<(String, i64)> {
        let mut v: Vec<(String, i64)> = f.iter().map(|(k, v)| (k.clone(), *v)).collect();
        v.sort();
        v
    }

    #[test]
    fn the_store_reads_back_every_row() {
        let mut st = FactStore::default();
        let r = fm(&[("room", 33), ("ego", 1), ("owner:500", 1), ("var:5", 7)]);
        let a = fm(&[("room", 28), ("ego", 1), ("owner:500", 1), ("var:9", 4)]);
        let b = fm(&[("room", 33), ("ego", 1), ("owner:500", 1), ("var:5", 7), ("owner:566", 1)]);
        st.put(0, &pairs(&r));
        st.put(1, &pairs(&a));
        st.put(2, &pairs(&b));
        for (i, f) in [(0, &r), (1, &a), (2, &b)] {
            for k in ["room", "ego", "owner:500", "owner:566", "var:5", "var:9", "var:77"] {
                assert_eq!(st.get(i, k), f.get(k).copied().unwrap_or(0), "row {i} {k}");
            }
            let mut o = st.view(i).owners();
            o.retain(|p| p.1 != 0);
            o.sort();
            let mut want = f.owners();
            want.sort();
            assert_eq!(o, want);
        }
        let s = st.seen(&[0, 1]);
        assert_eq!(s["room"], vec![28, 33]);
        assert_eq!(s["var:5"], vec![0, 7]);
        assert_eq!(s["var:9"], vec![0, 4]);
        assert_eq!(s["owner:566"], vec![0]);
        assert_eq!(st.seen(&[2])["owner:566"], vec![1]);
    }

    #[test]
    fn novelty_is_against_the_live_entries_values() {
        let seen = Seen {
            rooms: [33, 28].into_iter().collect(),
            vals: [("var:5".to_string(), vec![0, 7]), ("state:316".to_string(), vec![0])].into_iter().collect(),
            places: None,
            by_place: None,
        };
        let t = vec![TAtom::value("var:5", 7), TAtom::value("var:5", 3), TAtom::value("state:316", 1),
                     TAtom { key: "var:9".into(), vals: vec![], lo: None, hi: Some(2) }, TAtom::value("var:9", 0)];
        let n = Novel::new(seen, &t);
        assert_eq!(n.open.len(), 3);
        let f = fm(&[("room", 41), ("var:5", 3), ("state:316", 1), ("var:9", 5)]);
        assert_eq!(n.novel(&f), vec!["var:5=3", "state:316=1", "var:9=>2", "room=41"]);
        assert!(n.is_novel(&f));
        let g = fm(&[("room", 28), ("var:5", 7)]);
        assert!(n.novel(&g).is_empty() && !n.is_novel(&g));
        let job = json!({"novelty": {"t": atoms_json(&t), "rooms": [28, 33], "seen": {"var:5": [0, 7], "state:316": [0]}}});
        let m = Novel::of_job(&job, None).unwrap();
        assert_eq!(m.novel(&f), n.novel(&f));
        assert!(Novel::of_job(&json!({}), None).is_none());
        let facts = fmi_core::Facts::from_pairs(vec![(Key::parse("room"), 41), (Key::parse("var:5"), 3)]);
        assert_eq!(m.novel(&facts), vec!["var:5=3", "room=41"]);
    }

    /// Amendment (10): carrying an owned object into a seen room is novel once its place is unseen, and only with
    /// the live entries' places in job.json.
    #[test]
    fn a_seen_room_with_a_new_owned_set_is_a_novel_place() {
        let t = vec![TAtom::value("owner:946", 1)];
        let job = |places: Option<Value>| {
            let mut nv = json!({"t": atoms_json(&t), "rooms": [49, 63, 69], "seen": {"owner:946": [0, 1]}});
            if let Some(p) = places {
                nv["places"] = p;
            }
            json!({"novelty": nv})
        };
        let carried = fm(&[("room", 63), ("ego", 1), ("owner:946", 1), ("owner:20", 1), ("owner:937", 2)]);
        assert_eq!(place_text(63, &carried), "place=63:20,946");
        let off = Novel::of_job(&job(None), None).unwrap();
        assert!(off.novel(&carried).is_empty() && !off.is_novel(&carried));
        let on = Novel::of_job(&job(Some(json!(["place=49:20", "place=63:20", "place=69:20,946"]))), None).unwrap();
        assert_eq!(on.novel(&carried), vec!["place=63:20,946"]);
        assert!(on.is_novel(&carried));
        let back = fm(&[("room", 69), ("ego", 1), ("owner:946", 1), ("owner:20", 1)]);
        assert!(on.novel(&back).is_empty() && !on.is_novel(&back));
        let no_ego = fm(&[("room", 63), ("ego", 0), ("owner:946", 1)]);
        assert_eq!(place_text(63, &no_ego), "place=63:");
    }

    /// E018: with the places' values, an atom is novel at a place where no live entry holds it, though entries
    /// elsewhere do.
    #[test]
    fn novelty_by_place_judges_atoms_at_the_states_place() {
        let t = vec![TAtom::value("var:416", 1), TAtom::value("owner:946", 1)];
        let nv = json!({"t": atoms_json(&t), "rooms": [49], "seen": {"var:416": [0, 1], "owner:946": [0, 1]},
                        "places": ["place=49:20", "place=49:20,946"],
                        "by_place": {"place=49:20": {"var:416": [0, 1]}, "place=49:20,946": {"owner:946": [1]}}});
        let n = Novel::of_job(&json!({"novelty": nv}), None).unwrap();
        let with = fm(&[("room", 49), ("ego", 1), ("owner:20", 1), ("owner:946", 1), ("var:416", 1)]);
        assert_eq!(n.novel(&with), vec!["var:416=1"]);
        assert!(n.is_novel(&with));
        let without = fm(&[("room", 49), ("ego", 1), ("owner:20", 1), ("var:416", 1)]);
        assert!(n.novel(&without).is_empty() && !n.is_novel(&without));
        let plain = Novel::of_job(&json!({"novelty": {"t": atoms_json(&t), "rooms": [49], "seen": {"var:416": [0, 1], "owner:946": [0, 1]}}}), None).unwrap();
        assert!(!plain.is_novel(&with));
    }
}
