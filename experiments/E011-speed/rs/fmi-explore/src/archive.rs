//! explore.py's `Archive`, line for line (E008, with E009's and E010's switches): the archive's files and what follows
//! from them. The files are explore.py's, byte for byte (its JSON writers, `pyjson`); the archive's logic reads only
//! the files and the jobs' results, so it is deterministic given the results (plan.md, the explorer's exactness).
//!
//! What is kept in memory beyond explore.py: each entry's events, last checkpoint and last facts, read once from its
//! file (explore.py reads the file again on every use). The files are written by this process only, so the copy is
//! the file.

use crate::files::{self, as_i64, or, py_str, read_gz, read_json, read_jsonl, truthy, Res};
use crate::progress::{self, Progress, TAtom};
use crate::pyjson::{self, canonical};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const SEED: i64 = 1;
pub const GUIDE_FORMAT: &str = "e008-guide-1";
/// the guide's keys a cell's key holds (plan.md "Cells")
pub const KEY_KINDS: [&str; 5] = ["var:", "bit:", "owner:", "state:", "class:"];
/// a scope of this many rooms or more runs under the heavy lock
pub const HEAVY_ROOMS: usize = 5;
pub const GIB: f64 = (1u64 << 30) as f64;
/// log.jsonl outcomes that commit a job; "guard-killed" does not
pub const COMMITTED: [&str; 3] = ["ok", "error", "return differs"];
/// job.json "threads" (the reader) and "workers" (the window search)
pub const THREADS: i64 = 3;
pub const WINDOW_WORKERS: i64 = 3;
pub const START_FILES: [&str; 2] = ["start.txt", "start.jsonl"];

pub type FactMap = HashMap<String, i64>;

/// A fact map from a checkpoint's facts object (values are facts.py ints).
pub fn fact_map(v: &Value) -> FactMap {
    let mut out = FactMap::new();
    if let Some(m) = v.as_object() {
        out.reserve(m.len());
        for (k, x) in m {
            out.insert(k.clone(), as_i64(Some(x)).unwrap_or(NOT_INT));
        }
    }
    out
}

/// a fact value that is not an int (never in facts.py's maps): equal to no int
pub const NOT_INT: i64 = i64::MIN + 7;

/// A facts.py map as the archive reads it: a parsed map, or a checkpoint's JSON object read in place (no key copied).
pub trait FactLook {
    fn look(&self, k: &str) -> Option<i64>;
    /// (object, value) of every owner:O key
    fn owners(&self) -> Vec<(i64, i64)>;
}

/// E012: a facts map's every (key, value) pair, for the progress store (the maps the archive adds rows from).
pub trait FactPairs {
    fn pairs(&self) -> Vec<(String, i64)>;
    /// every pair through `f`, with no key text made where the map holds one
    fn each(&self, f: &mut dyn FnMut(&str, i64)) {
        for (k, v) in self.pairs() {
            f(&k, v);
        }
    }
}

impl FactLook for FactMap {
    fn look(&self, k: &str) -> Option<i64> {
        self.get(k).copied()
    }
    fn owners(&self) -> Vec<(i64, i64)> {
        self.iter().filter_map(|(k, v)| Some((k.strip_prefix("owner:")?.parse().ok()?, *v))).collect()
    }
}

impl FactPairs for FactMap {
    fn pairs(&self) -> Vec<(String, i64)> {
        self.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }
}

impl FactLook for Map<String, Value> {
    fn look(&self, k: &str) -> Option<i64> {
        self.get(k).map(|x| as_i64(Some(x)).unwrap_or(NOT_INT))
    }
    fn owners(&self) -> Vec<(i64, i64)> {
        self.iter().filter_map(|(k, v)| Some((k.strip_prefix("owner:")?.parse().ok()?, as_i64(Some(v)).unwrap_or(NOT_INT)))).collect()
    }
}

impl FactPairs for Map<String, Value> {
    fn pairs(&self) -> Vec<(String, i64)> {
        self.iter().map(|(k, x)| (k.clone(), as_i64(Some(x)).unwrap_or(NOT_INT))).collect()
    }
    fn each(&self, f: &mut dyn FnMut(&str, i64)) {
        for (k, x) in self {
            f(k, as_i64(Some(x)).unwrap_or(NOT_INT));
        }
    }
}

impl FactLook for fmi_core::Facts {
    fn look(&self, k: &str) -> Option<i64> {
        self.get(fmi_core::Key::parse(k)).map(|v| if v == fmi_core::NONE || v == fmi_core::BELOW || v == fmi_core::ABOVE { NOT_INT } else { v })
    }
    fn owners(&self) -> Vec<(i64, i64)> {
        self.0.iter().filter(|(k, _)| k.kind() == fmi_core::K_OWNER).map(|(k, v)| (k.a() as i64, *v)).collect()
    }
}

impl FactPairs for fmi_core::Facts {
    fn pairs(&self) -> Vec<(String, i64)> {
        self.0
            .iter()
            .map(|(k, v)| {
                let mut s = String::with_capacity(24);
                write_key(&mut s, *k);
                (s, if *v == fmi_core::NONE || *v == fmi_core::BELOW || *v == fmi_core::ABOVE { NOT_INT } else { *v })
            })
            .collect()
    }
    fn each(&self, f: &mut dyn FnMut(&str, i64)) {
        let mut s = String::with_capacity(32);
        for (k, v) in &self.0 {
            s.clear();
            write_key(&mut s, *k);
            f(&s, if *v == fmi_core::NONE || *v == fmi_core::BELOW || *v == fmi_core::ABOVE { NOT_INT } else { *v });
        }
    }
}

/// A key's text written in place (fmi-core's `Key::text` without its allocation, for the packed kinds).
pub fn write_key(s: &mut String, k: fmi_core::Key) {
    use fmi_core::*;
    let mut b = itoa::Buffer::new();
    let unit = |s: &mut String, u: u8, a: u64| match u {
        U_SCRIPT => {
            s.push_str("script:");
            s.push_str(itoa::Buffer::new().format(a));
        }
        U_LOCAL => {
            s.push_str("local:");
            s.push_str(itoa::Buffer::new().format(a));
        }
        U_OBJ => {
            s.push_str("obj:");
            s.push_str(itoa::Buffer::new().format(a));
        }
        U_ENTRY => s.push_str("entry"),
        U_EXIT => s.push_str("exit"),
        _ => s.push('?'),
    };
    let (a, bb) = (k.a(), k.b());
    match k.kind() {
        K_VAR | K_BIT | K_OWNER | K_STATE | K_SCRIPT | K_LOCAL | K_OBJSCRIPT | K_VERB | K_ACTORROOM | K_OBJPOS => {
            s.push_str(match k.kind() {
                K_VAR => "var:",
                K_BIT => "bit:",
                K_OWNER => "owner:",
                K_STATE => "state:",
                K_SCRIPT => "script:",
                K_LOCAL => "local:",
                K_OBJSCRIPT => "objscript:",
                K_VERB => "verb:",
                K_ACTORROOM => "actorroom:",
                _ => "objpos:",
            });
            s.push_str(b.format(a));
        }
        K_CLASS | K_SAVED => {
            s.push_str(if k.kind() == K_CLASS { "class:" } else { "saved:" });
            s.push_str(b.format(a));
            s.push(':');
            s.push_str(itoa::Buffer::new().format(bb));
        }
        K_ROOM => s.push_str("room"),
        K_EGO => s.push_str("ego"),
        K_CUTSCENE => s.push_str("cutscene"),
        K_GOAL => s.push_str("goal"),
        K_AT | K_ESC => {
            s.push_str(if k.kind() == K_AT { "at:" } else { "esc:" });
            unit(s, k.unit(), a);
            s.push(':');
            s.push_str(b.format(bb));
        }
        K_FROZEN | K_POS | K_ESCPOS | K_ARGS => {
            s.push_str(match k.kind() {
                K_FROZEN => "frozen:",
                K_POS => "pos:",
                K_ESCPOS => "escpos:",
                _ => "args:",
            });
            unit(s, k.unit(), a);
        }
        _ => s.push_str(&k.text()),
    }
}

/// A checkpoint's compact text [frame,{facts, in key order},{"tick","rng","actors"}] (the entry files' JSON).
pub fn checkpoint_text(c: &fmi_core::Checkpoint) -> String {
    let mut s = String::with_capacity(16 * c.facts.len() + 64);
    s.push('[');
    s.push_str(itoa::Buffer::new().format(c.frame));
    s.push_str(",{");
    for (i, (k, v)) in c.facts.0.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let mut kt = String::with_capacity(24);
        write_key(&mut kt, *k);
        crate::pyjson::write_str(&mut s, &kt);
        s.push(':');
        match *v {
            fmi_core::BELOW => s.push_str("\"below\""),
            fmi_core::ABOVE => s.push_str("\"above\""),
            fmi_core::NONE => s.push_str("null"),
            n => s.push_str(itoa::Buffer::new().format(n)),
        }
    }
    s.push_str("},{\"tick\":");
    s.push_str(itoa::Buffer::new().format(c.tick));
    s.push_str(",\"rng\":");
    s.push_str(itoa::Buffer::new().format(c.rng));
    s.push_str(",\"actors\":[");
    for (i, (id, x, y)) in c.actors.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("[{id},{x},{y}]"));
    }
    s.push_str("]}]");
    s
}

/// An attempt's checkpoints, shared by the entries it gives (each entry holds a prefix), with their texts and those
/// texts compressed once: an entry file is gzip members joined (its head, the checkpoints' members, the tail), which
/// gzip.open reads as one text, so the try's files and the archive's share every checkpoint's compression.
#[derive(Debug)]
pub struct SharedCps {
    pub cps: Vec<fmi_core::Checkpoint>,
    pub texts: Vec<String>,
    /// member j: the text of checkpoint j, after a comma for j > 0 (its place in every entry's list)
    pub members: Vec<Vec<u8>>,
}

impl SharedCps {
    pub fn new(cps: Vec<fmi_core::Checkpoint>) -> SharedCps {
        let both: Vec<(String, Vec<u8>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = cps.iter().enumerate().map(|(j, c)| sc.spawn(move || {
                let t = checkpoint_text(c);
                let m = if j == 0 { files::gz_member(&t) } else { files::gz_member(&format!(",{t}")) };
                (t, m)
            })).collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let (texts, members) = both.into_iter().unzip();
        SharedCps { cps, texts, members }
    }
    /// The checkpoint as a JSON value (its text parsed), for an entry's last checkpoint.
    pub fn value(&self, j: usize) -> Value {
        crate::pyjson::loads(&self.texts[j]).unwrap_or(Value::Null)
    }
}

/// A new entry a try of this process wrote: its file's fields but the checkpoints, which are the first `n` of `shared`.
pub struct NewEntry {
    pub parent: i64,
    pub frame: i64,
    pub tick: Option<i64>,
    pub events: Vec<Value>,
    pub shared: std::sync::Arc<SharedCps>,
    pub n: usize,
}

impl NewEntry {
    /// The head of the entry file's text, up to the checkpoints' list.
    fn head_text(&self, id: Option<usize>) -> String {
        let mut text = String::with_capacity(256);
        text.push_str(&format!("{{\"id\":{},\"parent\":{},\"frame\":{},\"tick\":{},\"events\":",
                               id.map(|i| i.to_string()).unwrap_or("null".into()), self.parent, self.frame,
                               self.tick.map(|t| t.to_string()).unwrap_or("null".into())));
        crate::pyjson::write_compact_slice(&mut text, &self.events);
        text.push_str(",\"checkpoints\":[");
        text
    }

    /// The entry file as gzip members (its head with this id, the shared checkpoints' members, the tail), written.
    pub fn write(&self, p: &Path, id: Option<usize>) -> Res<()> {
        let head = files::gz_member(&self.head_text(id));
        let tail = files::gz_member("]}");
        let mut ms: Vec<&[u8]> = Vec::with_capacity(self.n + 2);
        ms.push(&head);
        for m in &self.shared.members[..self.n] {
            ms.push(m);
        }
        ms.push(&tail);
        files::write_gz_members(p, &ms)
    }

    /// The entry file's compact text with this id (null: the try's new/K.json.gz).
    pub fn text(&self, id: Option<usize>) -> String {
        let total: usize = self.shared.texts[..self.n].iter().map(|t| t.len() + 1).sum();
        let mut text = String::with_capacity(total + 256);
        text.push_str(&format!("{{\"id\":{},\"parent\":{},\"frame\":{},\"tick\":{},\"events\":",
                               id.map(|i| i.to_string()).unwrap_or("null".into()), self.parent, self.frame,
                               self.tick.map(|t| t.to_string()).unwrap_or("null".into())));
        crate::pyjson::write_compact_slice(&mut text, &self.events);
        text.push_str(",\"checkpoints\":[");
        for (j, t) in self.shared.texts[..self.n].iter().enumerate() {
            if j > 0 {
                text.push(',');
            }
            text.push_str(t);
        }
        text.push_str("]}");
        text
    }
}

/// What the ingest gets for a new entry: the file's document, or a try's entry from the stash.
pub enum NewDoc {
    Doc(Value),
    Shared(NewEntry),
}

fn obj_of(v: &Value) -> &Map<String, Value> {
    static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
    v.as_object().unwrap_or_else(|| EMPTY.get_or_init(Map::new))
}

fn lk<F: FactLook + ?Sized>(f: &F, k: &str) -> i64 {
    f.look(k).unwrap_or(0)
}

fn get0(f: &FactMap, k: &str) -> i64 {
    f.get(k).copied().unwrap_or(0)
}

/// (tick or +inf, id): explore.py `_order`.
pub type Order = (i128, usize);

fn tick_ord(t: Option<i64>) -> i128 {
    t.map(|x| x as i128).unwrap_or(i128::MAX)
}

/// The guide's atoms (explore.py `guide_atoms`): key -> (its concrete values, whether (k, any) is in R).
#[derive(Clone, Debug, Default)]
pub struct Atom {
    pub values: HashSet<i64>,
    pub any: bool,
}

pub fn guide_atoms(guide: &Value) -> Res<BTreeMap<String, Atom>> {
    let mut out: BTreeMap<String, Atom> = BTreeMap::new();
    for a in guide["atoms"].as_array().ok_or("guide: no atoms")? {
        let k = a[0].as_str().ok_or("guide: an atom's key")?.to_string();
        let e = out.entry(k).or_default();
        match &a[1] {
            Value::Null => e.any = true,
            v => {
                e.values.insert(as_i64(Some(v)).ok_or("guide: an atom's value")?);
            }
        }
    }
    Ok(out)
}

pub fn load_guide(path: &Path) -> Res<Value> {
    let g = read_json(path)?;
    if g.get("format").and_then(|f| f.as_str()) != Some(GUIDE_FORMAT) {
        return Err(format!(
            "{}: format {}, not {GUIDE_FORMAT}",
            path.display(),
            files::py_repr(g.get("format").unwrap_or(&Value::Null))
        ));
    }
    Ok(g)
}

/// The guide's rule: sorted({R} | edges[R]) cut to the allowed rooms.
pub fn ring(edges: &HashMap<i64, Vec<i64>>, room: i64, allowed: &BTreeSet<i64>) -> Vec<i64> {
    let mut s: BTreeSet<i64> = edges.get(&room).map(|v| v.iter().copied().collect()).unwrap_or_default();
    s.insert(room);
    s.into_iter().filter(|r| allowed.contains(r)).collect()
}

/// `--goal` "K=V[,K=V]" as {key: [value]} (explore.py `parse_goal`, without "part1").
pub fn parse_goal(text: &str) -> Res<Value> {
    let mut out: BTreeMap<String, BTreeSet<i64>> = BTreeMap::new();
    for part in text.split(',') {
        let (k, v) = part.split_once('=').ok_or(format!("--goal {text}: K=V[,K=V]"))?;
        let v: i64 = v.trim().parse().map_err(|_| format!("--goal {text}: {v} is not an int"))?;
        out.entry(k.trim().to_string()).or_default().insert(v);
    }
    let mut m = Map::new();
    for (k, vs) in out {
        m.insert(k, Value::Array(vs.into_iter().map(Value::from).collect()));
    }
    Ok(Value::Object(m))
}

/// A cell (cells.json's value).
#[derive(Clone, Debug)]
pub struct Cell {
    pub key: Option<Value>,
    pub rep: usize,
    pub entries: Vec<usize>,
    pub n: i64,
    pub exhausted: Vec<usize>,
    /// str(entry) -> [[k, v], ...], in insertion order
    pub tried: Vec<(String, Value)>,
    /// E011's deepening (step::deepen): str(entry) -> [[k, v, n], ...], the targets deferred n times; empty without it
    pub deferred: Vec<(String, Value)>,
    /// E012 (FMI_SELECT=progress): the committed tries from the cell's entries; None with the switch off
    pub chosen: Option<i64>,
    /// E013 (FMI_FREE, component free): the free tries' counts (free_n, free_chosen), counted apart from the model's
    /// (`n`, `chosen` count the model's tries only); None with the switch off
    pub free: Option<(i64, i64)>,
}

impl Cell {
    fn to_json(&self) -> Value {
        let mut tried = Map::new();
        for (k, v) in &self.tried {
            tried.insert(k.clone(), v.clone());
        }
        let mut out = json!({"key": self.key.clone().unwrap_or(Value::Null), "rep": self.rep, "entries": self.entries,
               "n": self.n, "exhausted": self.exhausted, "tried": Value::Object(tried)});
        if !self.deferred.is_empty() {
            let mut d = Map::new();
            for (k, v) in &self.deferred {
                d.insert(k.clone(), v.clone());
            }
            out["deferred"] = Value::Object(d);
        }
        if let Some(c) = self.chosen {
            out["chosen"] = Value::from(c);
        }
        if let Some((n, c)) = self.free {
            out["free"] = json!([n, c]);
        }
        out
    }
    pub fn tried_of(&self, e: &str) -> Option<&Value> {
        self.tried.iter().find(|(k, _)| k == e).map(|(_, v)| v)
    }
    fn set_tried(&mut self, e: String, v: Value) {
        if let Some(x) = self.tried.iter_mut().find(|(k, _)| *k == e) {
            x.1 = v;
        } else {
            self.tried.push((e, v));
        }
    }
    pub fn deferred_of(&self, e: &str) -> Option<&Value> {
        self.deferred.iter().find(|(k, _)| k == e).map(|(_, v)| v)
    }
    fn set_deferred(&mut self, e: String, v: Value) {
        let empty = v.as_array().map(|a| a.is_empty()).unwrap_or(true);
        self.deferred.retain(|(k, _)| *k != e);
        if !empty {
            self.deferred.push((e, v));
        }
    }
}

/// An entries.jsonl row.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: usize,
    pub parent: Option<usize>,
    pub frame: i64,
    pub tick: Option<i64>,
    pub room: Option<i64>,
    pub cell: String,
    pub g: i64,
    pub held: Vec<(String, i64)>,
    pub r_path: usize,
    pub job: Option<String>,
    pub how: String,
    pub goal: bool,
    pub seg_held: Vec<(String, i64)>,
    /// E012: the game's own end that the entry's run ended in (a dead entry is never selected); None for every
    /// entry with the switch off
    pub dead: Option<String>,
}

fn pairs_json(p: &[(String, i64)]) -> Value {
    Value::Array(p.iter().map(|(k, v)| json!([k, v])).collect())
}

fn pairs_of(v: &Value) -> Vec<(String, i64)> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| Some((p.get(0)?.as_str()?.to_string(), as_i64(p.get(1)).unwrap_or(NOT_INT))))
                .collect()
        })
        .unwrap_or_default()
}

impl Row {
    pub fn to_json(&self) -> Value {
        let mut v = json!({"id": self.id, "parent": self.parent, "frame": self.frame, "tick": self.tick, "room": self.room,
               "cell": self.cell, "g": self.g, "held": pairs_json(&self.held), "r_path": self.r_path,
               "job": self.job, "how": self.how, "goal": self.goal, "seg_held": pairs_json(&self.seg_held)});
        if let Some(d) = &self.dead {
            v["dead"] = Value::from(d.clone());
        }
        v
    }
    pub fn of_json(v: &Value) -> Res<Row> {
        Ok(Row {
            id: v["id"].as_u64().ok_or("entries.jsonl: a row's id")? as usize,
            parent: v["parent"].as_u64().map(|x| x as usize),
            frame: as_i64(v.get("frame")).ok_or("entries.jsonl: a row's frame")?,
            tick: as_i64(v.get("tick")),
            room: as_i64(v.get("room")),
            cell: v["cell"].as_str().unwrap_or_default().to_string(),
            g: as_i64(v.get("g")).unwrap_or(0),
            held: pairs_of(&v["held"]),
            r_path: v["r_path"].as_u64().unwrap_or(0) as usize,
            job: v["job"].as_str().map(|s| s.to_string()),
            how: v["how"].as_str().unwrap_or("checkpoint").to_string(),
            goal: v["goal"].as_bool().unwrap_or(false),
            seg_held: pairs_of(&v["seg_held"]),
            dead: v.get("dead").and_then(|d| d.as_str()).map(|d| d.to_string()),
        })
    }
}

/// What the archive keeps of an entry's file.
#[derive(Debug)]
pub struct EntryInfo {
    pub events: Value,
    last_cp: std::cell::OnceCell<Value>,
    /// a try's entry: its last checkpoint's text, parsed when first asked for
    last_text: Option<(std::sync::Arc<SharedCps>, usize)>,
}

impl EntryInfo {
    pub fn of(events: Value, last_cp: Value) -> EntryInfo {
        let c = std::cell::OnceCell::new();
        let _ = c.set(last_cp);
        EntryInfo { events, last_cp: c, last_text: None }
    }
    /// The entry's last checkpoint, [frame, facts, extra].
    pub fn last_cp(&self) -> &Value {
        self.last_cp.get_or_init(|| match &self.last_text {
            Some((sh, j)) => sh.value(*j),
            None => Value::Null,
        })
    }
}

/// A reading of readings.jsonl.
pub type ReadingRow = Map<String, Value>;

/// What cells.json's text of a cell is made from (an equal shape is an equal text): E012's fields, then E013's free
/// counts.
type CellShape = ((bool, usize, usize, i64, usize, usize, usize, i64), Option<(i64, i64)>);

pub struct Archive {
    pub root: PathBuf,
    pub stage: Value,
    pub goal: Vec<(String, Vec<i64>)>,
    pub goal_json: Value,
    pub guide: Value,
    pub atoms: BTreeMap<String, Atom>,
    pub cell_keys: Vec<String>,
    pub edges: HashMap<i64, Vec<i64>>,
    pub allowed: BTreeSet<i64>,
    pub rows: Vec<Row>,
    /// cell id -> its cell, in insertion order
    pub cells: Vec<(String, Cell)>,
    cell_ix: HashMap<String, usize>,
    /// (k, v) -> the (tick, id) of its least holder
    pub held_by: HashMap<(String, i64), Order>,
    pub root_facts: Option<FactMap>,
    pub readings: Vec<ReadingRow>,
    pub logs: Vec<Value>,
    /// job -> (entry id, cell)
    pub busy: BTreeMap<String, (usize, String)>,
    chain_r: RefCell<HashMap<usize, Rc<HashSet<(String, i64)>>>>,
    /// E016 amendment (11) (FMI_SELECT_PLACE=1): per place, the model's and the free tries' (n, chosen), made from
    /// the log once per its length
    place_counts: RefCell<Option<(usize, [HashMap<String, (i64, i64)>; 2])>>,
    /// E018 (FMI_PLACE_RARITY=1): per place, how many rows hold each held pair, made once per row count
    place_pairs: RefCell<Option<(usize, HashMap<String, HashMap<(String, i64), usize>>)>>,
    /// E022: per live row, the guide atoms it is the first live row at its place (with its context) to hold, by rows
    place_firsts: RefCell<Option<(usize, HashMap<usize, i64>)>>,
    /// E019 (FMI_SKIP_SAME=1): job.json fingerprint -> the first job with it, made from the job dirs on first use
    fingerprints: RefCell<Option<HashMap<String, String>>>,
    pub e009: Map<String, Value>,
    pub e010: Map<String, Value>,
    pub idle_frames: i64,
    /// I: the keys measured idle at some entry, less the goal's (fix a)
    pub idle: BTreeSet<String>,
    pub idle_measured: BTreeSet<i64>,
    pub last_place: (bool, bool),
    entries: RefCell<HashMap<usize, Rc<EntryInfo>>>,
    /// cells.json's text per cell (json.dumps of the cell) and the cell's shape it was made from: a cell's lists only
    /// grow and its n changes, so an equal shape is an equal cell (rekey clears it)
    cell_text: RefCell<HashMap<String, (CellShape, String)>>,
    /// during an ingest: the entries.jsonl rows written in one append (one sync) after the entries' files
    pending_rows: Option<Vec<Value>>,
    /// during an ingest: the entries' files (path, text), compressed and written together before the rows
    pending_files: Option<Vec<(PathBuf, String)>>,
    /// E012 (FMI_SELECT=progress, or an archive made with it): T, the rows' facts and the marks; None with the switch
    /// off
    pub progress: Option<Progress>,
    /// during an ingest: the dead mark of the entry being added
    next_dead: Option<String>,
    /// E012: row -> the distinct rooms of the entries from the root to it
    chain_rooms: RefCell<HashMap<usize, Rc<BTreeSet<i64>>>>,
    /// E013 (component free): the free tries' counts are kept (FMI_FREE set, or a committed line of the log has a
    /// kind); off, every cell and file is E012's
    pub free_on: bool,
    /// E014 amendment (4) (b) (FMI_SCOPE_GROW, or a committed line of the log has a "grow"): room -> the rooms grown
    /// for it (grow.rs); None: every scope is E009's ring
    pub grown: Option<BTreeMap<i64, BTreeSet<i64>>>,
}

fn goal_pairs(v: &Value) -> Vec<(String, Vec<i64>)> {
    v.as_object()
        .map(|m| {
            m.iter()
                .map(|(k, vs)| {
                    (k.clone(), vs.as_array().map(|a| a.iter().filter_map(|x| as_i64(Some(x))).collect()).unwrap_or_default())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The new entries a try of this process wrote (JOB_DIR/new/K.json.gz -> its document): ingest takes them from here
/// instead of reading the file back, once it has seen the file is there.
fn new_cache() -> &'static std::sync::Mutex<HashMap<PathBuf, NewDoc>> {
    static C: std::sync::OnceLock<std::sync::Mutex<HashMap<PathBuf, NewDoc>>> = std::sync::OnceLock::new();
    C.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// A try's new entry, written to `path`, kept for the ingest.
pub fn stash_new(path: &Path, doc: Value) {
    new_cache().lock().unwrap().insert(resolve(path), NewDoc::Doc(doc));
}

/// A try's new entry, its checkpoints shared with the attempt's other entries, kept for the ingest.
pub fn stash_shared(path: &Path, e: NewEntry) {
    new_cache().lock().unwrap().insert(resolve(path), NewDoc::Shared(e));
}

pub fn committed(x: &Value) -> bool {
    x.get("outcome").and_then(|o| o.as_str()).map(|o| COMMITTED.contains(&o)).unwrap_or(false)
}

impl Archive {
    fn new(root: &Path) -> Res<Archive> {
        let stage = read_json(&root.join("stage.json"))?;
        let goal_json = {
            // {k: list(v)}
            let mut m = Map::new();
            if let Some(g) = stage["goal"].as_object() {
                for (k, v) in g {
                    m.insert(k.clone(), v.clone());
                }
            }
            Value::Object(m)
        };
        let guide = load_guide(Path::new(stage["guide"].as_str().ok_or("stage.json: guide")?))?;
        let atoms = guide_atoms(&guide)?;
        let mut cell_keys: Vec<String> = guide["keys"]
            .as_object()
            .map(|m| m.keys().filter(|k| KEY_KINDS.iter().any(|p| k.starts_with(p))).cloned().collect())
            .unwrap_or_default();
        cell_keys.sort();
        let mut edges = HashMap::new();
        if let Some(m) = guide["rooms"]["edges"].as_object() {
            for (r, v) in m {
                let r: i64 = r.parse().map_err(|_| format!("guide: room {r}"))?;
                edges.insert(r, v.as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default());
            }
        }
        let allowed = stage["allowed_rooms"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
        let e009 = stage.get("e009").and_then(|v| v.as_object()).cloned().unwrap_or_default();
        let e010 = stage.get("e010").and_then(|v| v.as_object()).cloned().unwrap_or_default();
        let idle_frames = as_i64(e009.get("idle_frames")).unwrap_or(0);
        let made_progress = stage.get("e012").and_then(|e| e.get("select")).and_then(|s| s.as_str()) == Some(progress::PROGRESS);
        let prog = if progress::on() || made_progress { Some(Progress::new(&goal_pairs(&goal_json))) } else { None };
        Ok(Archive {
            root: root.to_path_buf(),
            goal: goal_pairs(&goal_json),
            goal_json,
            stage,
            guide,
            atoms,
            cell_keys,
            edges,
            allowed,
            rows: Vec::new(),
            cells: Vec::new(),
            cell_ix: HashMap::new(),
            held_by: HashMap::new(),
            root_facts: None,
            readings: Vec::new(),
            logs: Vec::new(),
            busy: BTreeMap::new(),
            chain_r: RefCell::new(HashMap::new()),
            e009,
            e010,
            idle_frames,
            idle: BTreeSet::new(),
            idle_measured: BTreeSet::new(),
            last_place: (false, false),
            entries: RefCell::new(HashMap::new()),
            cell_text: RefCell::new(HashMap::new()),
            pending_rows: None,
            pending_files: None,
            progress: prog,
            next_dead: None,
            chain_rooms: RefCell::new(HashMap::new()),
            place_counts: RefCell::new(None),
            place_pairs: RefCell::new(None),
            place_firsts: RefCell::new(None),
            fingerprints: RefCell::new(None),
            free_on: crate::free::rho0().is_some(),
            grown: if crate::grow::on() { Some(BTreeMap::new()) } else { None },
        })
    }

    // --- making and opening

    /// explore.py `Archive.create`.
    #[allow(clippy::too_many_arguments)]
    pub fn create(root: &Path, stage: &str, goal: &Value, guide: &Path, allowed: &[i64], workers: i64,
                  root_info: Option<Value>, e009: Option<Value>, e010: Option<Value>) -> Res<Archive> {
        if root.join("stage.json").exists() {
            return Err(format!("{}: an archive is already there", root.display()));
        }
        load_guide(guide)?;
        for d in ["entries", "jobs", "readings"] {
            fs::create_dir_all(root.join(d)).map_err(|e| files::io_err(&root.join(d), e))?;
        }
        let mut goal_sorted = Map::new();
        if let Some(g) = goal.as_object() {
            for (k, v) in g {
                let mut vs: Vec<i64> = v.as_array().map(|a| a.iter().filter_map(|x| as_i64(Some(x))).collect()).unwrap_or_default();
                vs.sort();
                goal_sorted.insert(k.clone(), json!(vs));
            }
        }
        let mut allowed: Vec<i64> = allowed.to_vec();
        allowed.sort();
        let guide_abs = fs::canonicalize(guide).map_err(|e| files::io_err(guide, e))?;
        let mut doc = json!({"stage": stage, "seed": SEED, "goal": Value::Object(goal_sorted),
                             "guide": guide_abs.to_string_lossy(), "allowed_rooms": allowed, "workers": workers,
                             "created": files::strftime("%Y-%m-%dT%H:%M:%S"),
                             "root": root_info.unwrap_or_else(|| json!({}))});
        if let Some(e) = e009.filter(|e| truthy(Some(e))) {
            doc["e009"] = e;
        }
        if let Some(e) = e010.filter(|e| truthy(Some(e))) {
            doc["e010"] = e;
        }
        if progress::on() {
            doc["e012"] = json!({"select": progress::PROGRESS});
        }
        files::write_json(&root.join("stage.json"), &doc, Some(1))?;
        for f in ["entries.jsonl", "log.jsonl", "readings.jsonl"] {
            let p = root.join(f);
            fs::OpenOptions::new().create(true).append(true).open(&p).map_err(|e| files::io_err(&p, e))?;
        }
        Archive::open(root)
    }

    /// explore.py `Archive.open`.
    pub fn open(root: &Path) -> Res<Archive> {
        let mut a = Archive::new(root)?;
        a.load()?;
        Ok(a)
    }

    fn load(&mut self) -> Res<()> {
        let r = self.root.clone();
        let (logs, clean) = read_jsonl(&r.join("log.jsonl"))?;
        self.logs = logs;
        self.free_on = self.free_on || self.logs.iter().any(|x| committed(x) && x.get("kind").is_some());
        // E014 amendment (4) (b): the rooms grown, replayed from the committed lines in commit order
        if self.grown.is_none() && self.logs.iter().any(|x| committed(x) && x.get("grow").is_some()) {
            self.grown = Some(BTreeMap::new());
        }
        if let Some(g) = self.grown.as_mut() {
            for x in self.logs.iter().filter(|x| committed(x)) {
                if let (Some(r), Some(add)) = (x["grow"]["room"].as_i64(), x["grow"]["add"].as_array()) {
                    g.entry(r).or_default().extend(add.iter().filter_map(|a| a.as_i64()));
                }
            }
        }
        if !clean {
            files::rewrite_jsonl(&r.join("log.jsonl"), &self.logs)?;
        }
        let committed_jobs: HashSet<String> = self.committed_jobs();
        if let Some(p) = self.progress.as_mut() {
            // E012: T as the committed tries left it (the goal's atoms, then each line's new atoms)
            for x in &self.logs {
                if committed(x) && x["outcome"].as_str() != Some("return differs") {
                    p.merge(&progress::atoms_of_json(x.get("t_new")));
                }
            }
        }
        let (rows, clean) = read_jsonl(&r.join("entries.jsonl"))?;
        let mut keep: Vec<Value> = Vec::new();
        for row in &rows {
            // an ingest cut short: its rows (always the last ones) and their files go
            let job = row.get("job").and_then(|j| j.as_str());
            if row.get("job").map(|j| j.is_null()).unwrap_or(true) || job.map(|j| committed_jobs.contains(j)).unwrap_or(false) {
                if row["id"].as_u64() != Some(keep.len() as u64) {
                    return Err(format!("{}/entries.jsonl: entry {} at line {}", r.display(), row["id"], keep.len()));
                }
                keep.push(row.clone());
            }
        }
        if keep.len() != rows.len() || !clean {
            for row in &rows[keep.len().min(rows.len())..] {
                let _ = fs::remove_file(r.join("entries").join(format!("{}.json.gz", row["id"])));
            }
            files::rewrite_jsonl(&r.join("entries.jsonl"), &keep)?;
        }
        if let Ok(rd) = fs::read_dir(r.join("entries")) {
            // files written before their row
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if !name.ends_with(".json.gz") {
                    continue;
                }
                if let Ok(n) = name.split('.').next().unwrap_or("").parse::<usize>() {
                    if n >= keep.len() {
                        let _ = fs::remove_file(e.path());
                    }
                }
            }
        }
        let old_cells = if r.join("cells.json").exists() { read_json(&r.join("cells.json"))? } else { json!({}) };
        for row in &keep {
            let row = Row::of_json(row)?;
            if row.parent.is_none() {
                self.root_facts = Some(self.facts(0)?);
            }
            let key = old_cells.get(&row.cell).and_then(|c| c.get("key")).filter(|k| !k.is_null()).cloned();
            self.place(row, key);
        }
        for x in &self.logs {
            if !committed(x) {
                continue;
            }
            let Some(cell) = x.get("cell").and_then(|c| c.as_str()) else { continue };
            let Some(&ci) = self.cell_ix.get(cell) else { continue };
            let c = &mut self.cells[ci].1;
            c.n = as_i64(x.get("n")).unwrap_or(0);
            if x.get("tried").map(|t| !t.is_null()).unwrap_or(false) {
                c.set_tried(py_str(x.get("entry")), x["tried"].clone());
            }
            if let Some(d) = x.get("deferred").filter(|d| !d.is_null()) {
                c.set_deferred(py_str(x.get("entry")), d.clone());
            }
            if truthy(x.get("exhausted")) {
                let e = x["entry"].as_u64().unwrap_or(0) as usize;
                if !c.exhausted.contains(&e) {
                    c.exhausted.push(e);
                }
            }
        }
        for ci in 0..self.cells.len() {
            // (E012: the rows are keyed again below)
            if self.cells[ci].1.key.is_none() && self.progress.is_none() {
                let e0 = self.cells[ci].1.entries[0];
                let f = self.facts(e0)?;
                self.cells[ci].1.key = Some(self.cell_key(&f));
            }
        }
        let (readings, clean) = read_jsonl(&r.join("readings.jsonl"))?;
        let n0 = readings.len();
        self.readings = readings
            .into_iter()
            .filter(|x| {
                x.get("job").map(|j| j.is_null()).unwrap_or(true)
                    || x["job"].as_str().map(|j| committed_jobs.contains(j)).unwrap_or(false)
            })
            .filter_map(|x| x.as_object().cloned())
            .collect();
        if self.readings.len() != n0 || !clean {
            self.rewrite_readings()?;
        }
        if self.idle_frames != 0 {
            let goal_keys: HashSet<&str> = self.goal.iter().map(|(k, _)| k.as_str()).collect();
            for x in &self.logs {
                if committed(x) && truthy(x.get("idle")) {
                    let idle = &x["idle"];
                    if let Some(ks) = idle.get("keys").and_then(|k| k.as_array()) {
                        for k in ks.iter().filter_map(|k| k.as_str()) {
                            if !goal_keys.contains(k) {
                                self.idle.insert(k.to_string());
                            }
                        }
                    }
                    if let Some(e) = as_i64(idle.get("entry")) {
                        self.idle_measured.insert(e);
                    }
                }
            }
        }
        if self.idle_frames != 0 || self.progress.is_some() {
            self.rekey()?;
        }
        Ok(())
    }

    fn rewrite_readings(&self) -> Res<()> {
        let rows: Vec<Value> = self.readings.iter().map(|x| Value::Object(x.clone())).collect();
        files::rewrite_jsonl(&self.root.join("readings.jsonl"), &rows)
    }

    // --- entries

    /// The entry's file (explore.py `entry`).
    pub fn entry(&self, i: usize) -> Res<Value> {
        read_gz(&self.root.join("entries").join(format!("{i}.json.gz")))
    }

    /// The entry's events, last checkpoint and last facts, read once.
    pub fn info(&self, i: usize) -> Res<Rc<EntryInfo>> {
        if let Some(x) = self.entries.borrow().get(&i) {
            return Ok(x.clone());
        }
        let doc = self.entry(i)?;
        let last_cp = doc["checkpoints"].as_array().and_then(|a| a.last()).cloned().ok_or(format!("entry {i}: no checkpoint"))?;
        let info = Rc::new(EntryInfo::of(doc["events"].clone(), last_cp));
        self.entries.borrow_mut().insert(i, info.clone());
        Ok(info)
    }

    pub fn facts(&self, i: usize) -> Res<FactMap> {
        Ok(fact_map(&self.info(i)?.last_cp()[1]))
    }

    fn goal_keys(&self) -> HashSet<&str> {
        self.goal.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// explore.py `cell_key`.
    pub fn cell_key<F: FactLook + ?Sized>(&self, facts: &F) -> Value {
        let ego = facts.look("ego");
        let mut owned: Vec<i64> = Vec::new();
        if let Some(ego) = ego {
            owned = facts.owners().into_iter().filter(|(_, v)| *v == ego).map(|(o, _)| o).collect();
        }
        owned.sort();
        if let Some(p) = &self.progress {
            // E012: room, owned objects, the atoms of T held but the script-control keys' (the amendment)
            owned.retain(|_| ego != Some(0));
            return json!([facts.look("room"), owned, p.cell_atoms(facts)]);
        }
        let kv: Vec<Value> =
            self.cell_keys.iter().filter(|k| !self.idle.contains(*k)).map(|k| json!([k, lk(facts, k)])).collect();
        json!([facts.look("room"), owned, kv])
    }

    pub fn cell_id(key: &Value) -> String {
        sha1_smol::Sha1::from(canonical(key).as_bytes()).digest().to_string()
    }

    /// The R atoms ``facts`` holds, as (k, v) pairs (module docstring).
    pub fn held<F: FactLook + ?Sized>(&self, facts: &F) -> Vec<(String, i64)> {
        let mut out = Vec::new();
        for (k, a) in &self.atoms {
            if self.idle.contains(k) {
                continue;
            }
            let v = lk(facts, k);
            let rv = match &self.root_facts {
                Some(r) => get0(r, k),
                None => v,
            };
            if a.values.contains(&v) || (a.any && v != rv) {
                out.push((k.clone(), v));
            }
        }
        out.sort();
        out
    }

    /// #g: the goal keys whose value is not the goal's.
    pub fn g<F: FactLook + ?Sized>(&self, facts: &F) -> i64 {
        self.goal.iter().filter(|(k, vs)| !vs.contains(&lk(facts, k))).count() as i64
    }

    pub fn chain(&self, i: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut cur = Some(i);
        while let Some(j) = cur {
            out.push(j);
            cur = self.rows[j].parent;
        }
        out.reverse();
        out
    }

    pub fn chain_r(&self, i: Option<usize>) -> Rc<HashSet<(String, i64)>> {
        let Some(i) = i else { return Rc::new(HashSet::new()) };
        if let Some(s) = self.chain_r.borrow().get(&i) {
            return s.clone();
        }
        let mut s = HashSet::new();
        for j in self.chain(i) {
            s.extend(self.rows[j].seg_held.iter().cloned());
        }
        let s = Rc::new(s);
        self.chain_r.borrow_mut().insert(i, s.clone());
        s
    }

    /// The concatenation of the chain's segments from the root: the events of the entry's movie from boot.
    pub fn movie(&self, i: usize) -> Res<Vec<Value>> {
        let mut out = Vec::new();
        for j in self.chain(i) {
            if let Some(a) = self.info(j)?.events.as_array() {
                out.extend(a.iter().cloned());
            }
        }
        Ok(out)
    }

    pub fn checkpoints(&self, i: usize) -> Res<Vec<Value>> {
        let mut out = Vec::new();
        for j in self.chain(i) {
            if let Some(a) = self.entry(j)?["checkpoints"].as_array() {
                out.extend(a.iter().cloned());
            }
        }
        Ok(out)
    }

    /// A new entry (plan.md "Entries"): its file, then its row. Returns its id.
    #[allow(clippy::too_many_arguments)]
    pub fn add(&mut self, parent: Option<usize>, frame: i64, tick: Option<i64>, events: &[Value], checkpoints: &[Value],
               job: Option<&str>, how: &str) -> Res<usize> {
        let i = self.rows.len();
        let cps: &[Value] = checkpoints;
        if cps.is_empty() || as_i64(cps.last().unwrap().get(0)) != Some(frame) {
            return Err(format!("ValueError: entry {i}: the last checkpoint is not at the entry's frame {frame}"));
        }
        for w in cps.windows(2) {
            if as_i64(w[1].get(0)) < as_i64(w[0].get(0)) {
                return Err(format!("ValueError: entry {i}: checkpoints out of frame order"));
            }
        }
        match parent {
            None => {
                if i != 0 || !events.is_empty() {
                    return Err("ValueError: only entry 0, the root, has no parent, and it has no events".into());
                }
            }
            Some(p) => {
                if p >= i {
                    return Err(format!("ValueError: entry {i}: parent {p} is not in the archive"));
                }
                let f0 = self.rows[p].frame;
                if events.iter().any(|e| {
                    let f = as_i64(e.get("frame")).unwrap_or(i64::MIN);
                    !(f0 < f && f <= frame)
                }) {
                    return Err(format!("ValueError: entry {i}: an event outside its segment ({f0}, {frame}]"));
                }
                if cps.iter().any(|c| as_i64(c.get(0)).unwrap_or(i64::MIN) <= f0) {
                    return Err(format!("ValueError: entry {i}: a checkpoint at or before the parent's frame {f0}"));
                }
            }
        }
        let ta = std::time::Instant::now();
        let last = obj_of(&cps.last().unwrap()[1]);
        if parent.is_none() {
            self.root_facts = Some(fact_map(&cps.last().unwrap()[1]));
        }
        if let Some(p) = self.progress.as_mut() {
            p.store.put_each(i, &|f: &mut dyn FnMut(&str, i64)| last.each(f));
        }
        let key = self.cell_key(last);
        let mut seg: BTreeSet<(String, i64)> = BTreeSet::new();
        for c in cps {
            seg.extend(self.held(obj_of(&c[1])));
        }
        let g = self.g(last);
        let r_path = {
            let cr = self.chain_r(parent);
            let mut u: HashSet<&(String, i64)> = cr.iter().collect();
            u.extend(seg.iter());
            u.len()
        };
        let row = Row {
            id: i,
            parent,
            frame,
            tick,
            room: last.look("room"),
            cell: Self::cell_id(&key),
            g,
            held: self.held(last),
            r_path,
            job: job.map(|s| s.to_string()),
            how: how.to_string(),
            goal: g == 0,
            seg_held: seg.into_iter().collect(),
            dead: self.next_dead.take(),
        };
        let t1 = ta.elapsed().as_secs_f64();
        // write_gz's text of {"id", "parent", "frame", "tick", "events", "checkpoints"}, from the slices (no copy)
        let mut text = String::with_capacity(1 << 16);
        text.push_str(&format!("{{\"id\":{i},\"parent\":{},\"frame\":{frame},\"tick\":{},\"events\":",
                               parent.map(|p| p.to_string()).unwrap_or("null".into()), tick.map(|t| t.to_string()).unwrap_or("null".into())));
        crate::pyjson::write_compact_slice(&mut text, events);
        text.push_str(",\"checkpoints\":");
        crate::pyjson::write_compact_slice(&mut text, cps);
        text.push('}');
        let t2 = ta.elapsed().as_secs_f64();
        let path = self.root.join("entries").join(format!("{i}.json.gz"));
        match self.pending_files.as_mut() {
            Some(p) => p.push((path, std::mem::take(&mut text))),
            None => files::write_gz_text(&path, &text)?,
        }
        if std::env::var("FMI_ARCHIVE_PROFILE").is_ok() {
            eprintln!("add {i}: {} cps, {} bytes; keys and atoms {:.2} ms, text {:.2} ms, gz {:.2} ms", cps.len(), text.len(),
                      t1 * 1e3, (t2 - t1) * 1e3, (ta.elapsed().as_secs_f64() - t2) * 1e3);
        }
        match self.pending_rows.as_mut() {
            Some(p) => p.push(row.to_json()),
            None => files::append_jsonl(&self.root.join("entries.jsonl"), &[row.to_json()])?,
        }
        let info = Rc::new(EntryInfo::of(Value::Array(events.to_vec()), cps.last().unwrap().clone()));
        self.entries.borrow_mut().insert(i, info);
        self.last_place = self.place(row, Some(key));
        self.progress_place(i);
        Ok(i)
    }

    /// `add` for a try's entry of this process: the checkpoints read in place and their texts reused.
    pub fn add_shared(&mut self, parent: Option<usize>, ne: &NewEntry, job: Option<&str>, how: &str) -> Res<usize> {
        let i = self.rows.len();
        let cps = &ne.shared.cps[..ne.n];
        let frame = ne.frame;
        if cps.is_empty() || cps.last().unwrap().frame as i64 != frame {
            return Err(format!("ValueError: entry {i}: the last checkpoint is not at the entry's frame {frame}"));
        }
        if cps.windows(2).any(|w| w[1].frame < w[0].frame) {
            return Err(format!("ValueError: entry {i}: checkpoints out of frame order"));
        }
        let p = parent.ok_or("ValueError: a try's entry has a parent")?;
        if p >= i {
            return Err(format!("ValueError: entry {i}: parent {p} is not in the archive"));
        }
        let f0 = self.rows[p].frame;
        if ne.events.iter().any(|e| {
            let f = as_i64(e.get("frame")).unwrap_or(i64::MIN);
            !(f0 < f && f <= frame)
        }) {
            return Err(format!("ValueError: entry {i}: an event outside its segment ({f0}, {frame}]"));
        }
        if cps.iter().any(|c| c.frame as i64 <= f0) {
            return Err(format!("ValueError: entry {i}: a checkpoint at or before the parent's frame {f0}"));
        }
        let last = &cps.last().unwrap().facts;
        if let Some(p) = self.progress.as_mut() {
            p.store.put_each(i, &|f: &mut dyn FnMut(&str, i64)| last.each(f));
        }
        let key = self.cell_key(last);
        let mut seg: BTreeSet<(String, i64)> = BTreeSet::new();
        for c in cps {
            seg.extend(self.held(&c.facts));
        }
        let g = self.g(last);
        let r_path = {
            let cr = self.chain_r(parent);
            let mut u: HashSet<&(String, i64)> = cr.iter().collect();
            u.extend(seg.iter());
            u.len()
        };
        let row = Row {
            id: i,
            parent,
            frame,
            tick: ne.tick,
            room: last.look("room"),
            cell: Self::cell_id(&key),
            g,
            held: self.held(last),
            r_path,
            job: job.map(|s| s.to_string()),
            how: how.to_string(),
            goal: g == 0,
            seg_held: seg.into_iter().collect(),
            dead: self.next_dead.take(),
        };
        // the file from the attempt's compressed checkpoints (no text made, nothing compressed but its head)
        ne.write(&self.root.join("entries").join(format!("{i}.json.gz")), Some(i))?;
        match self.pending_rows.as_mut() {
            Some(pr) => pr.push(row.to_json()),
            None => files::append_jsonl(&self.root.join("entries.jsonl"), &[row.to_json()])?,
        }
        let info = Rc::new(EntryInfo { events: Value::Array(ne.events.clone()), last_cp: std::cell::OnceCell::new(),
                                       last_text: Some((ne.shared.clone(), ne.n - 1)) });
        self.entries.borrow_mut().insert(i, info);
        self.last_place = self.place(row, Some(key));
        self.progress_place(i);
        Ok(i)
    }

    /// The row in memory: its cell and the least holders. (new cell, new representative)
    fn place(&mut self, row: Row, key: Option<Value>) -> (bool, bool) {
        let i = row.id;
        let c = row.cell.clone();
        let held = row.held.clone();
        self.rows.push(row);
        let new_cell = !self.cell_ix.contains_key(&c);
        if new_cell {
            self.cell_ix.insert(c.clone(), self.cells.len());
            let chosen = self.progress.as_ref().map(|_| 0);
            let free = self.free_on.then_some((0, 0));
            self.cells.push((c.clone(), Cell { key, rep: i, entries: vec![], n: 0, exhausted: vec![], tried: vec![], deferred: vec![], chosen, free }));
        }
        let ci = self.cell_ix[&c];
        self.cells[ci].1.entries.push(i);
        let rep = self.cells[ci].1.rep;
        let new_rep = !new_cell && self.order(i) < self.order(rep);
        if new_rep {
            self.cells[ci].1.rep = i;
        }
        let oi = self.order(i);
        for p in held {
            match self.held_by.get(&p) {
                Some(o) if *o <= oi => {}
                _ => {
                    self.held_by.insert(p, oi);
                }
            }
        }
        (new_cell, new_rep)
    }

    pub fn order(&self, i: usize) -> Order {
        (tick_ord(self.rows[i].tick), i)
    }

    pub fn cell(&self, id: &str) -> &Cell {
        &self.cells[self.cell_ix[id]].1
    }

    pub fn cell_mut(&mut self, id: &str) -> &mut Cell {
        let i = self.cell_ix[id];
        &mut self.cells[i].1
    }

    pub fn has_cell(&self, id: &str) -> bool {
        self.cell_ix.contains_key(id)
    }

    // --- E009 fix a: the idle set, and the rows keyed again when it grows

    /// A job's measurement at ``entry``: its idle keys join I (less the goal's keys); when I grows every row is keyed
    /// again. Returns the keys new to I.
    pub fn merge_idle(&mut self, entry: i64, keys: &[String]) -> Res<Vec<String>> {
        self.idle_measured.insert(entry);
        let goal = self.goal_keys();
        let new: BTreeSet<String> =
            keys.iter().filter(|k| !goal.contains(k.as_str()) && !self.idle.contains(*k)).cloned().collect();
        let new: Vec<String> = new.into_iter().collect();
        if !new.is_empty() {
            self.idle.extend(new.iter().cloned());
            self.rekey()?;
        }
        Ok(new)
    }

    /// Every row's cell, held atoms, segment atoms and #r computed again from its entry file under the current I.
    pub fn rekey(&mut self) -> Res<()> {
        let rows = std::mem::take(&mut self.rows);
        self.cells.clear();
        self.cell_ix.clear();
        self.cell_text.borrow_mut().clear();
        self.held_by.clear();
        self.chain_r.borrow_mut().clear();
        if let Some(p) = self.progress.as_mut() {
            p.clear_rows();
        }
        for row in rows {
            let e = self.entry(row.id)?;
            let cps = e["checkpoints"].as_array().cloned().unwrap_or_default();
            let last = obj_of(&cps.last().ok_or("rekey: an entry with no checkpoint")?[1]);
            if row.parent.is_none() {
                self.root_facts = Some(fact_map(&cps.last().unwrap()[1]));
            }
            let facts = last;
            if let Some(p) = self.progress.as_mut() {
                p.store.put_each(row.id, &|f: &mut dyn FnMut(&str, i64)| facts.each(f));
            }
            let key = self.cell_key(facts);
            let mut seg: BTreeSet<(String, i64)> = BTreeSet::new();
            for c in &cps {
                seg.extend(self.held(obj_of(&c[1])));
            }
            let mut new = row.clone();
            new.cell = Self::cell_id(&key);
            new.held = self.held(facts);
            new.seg_held = seg.iter().cloned().collect();
            let cr = self.chain_r(row.parent);
            let mut u: HashSet<&(String, i64)> = cr.iter().collect();
            u.extend(seg.iter());
            new.r_path = u.len();
            let id = new.id;
            self.place(new, Some(key));
            self.progress_place(id);
        }
        self.recount();
        Ok(())
    }

    // --- E012 (FMI_SELECT=progress): T, novelty, marks, the order (progress.rs, the plan's amendment)

    /// Row i's T atoms, and, when it is live, its share of the least holders and the rooms (its facts are in the
    /// store already).
    fn progress_place(&mut self, i: usize) {
        let (frame, room, live) = (self.rows[i].frame, self.rows[i].room, self.rows[i].dead.is_none());
        // E022: the row's state against its parent's, for the keys a room's context leaves out (ctx.rs)
        if let (true, Some(p), Some(par)) = (crate::ctx::on(), self.progress.as_ref(), self.rows[i].parent) {
            crate::ctx::note_edge(&p.store.view(par), &p.store.view(i));
        }
        if let Some(p) = self.progress.as_mut() {
            p.place(i, frame, room, live);
        }
    }

    /// Every row keyed again from the store (T grew): no file is read. The counts are counted again from the log.
    pub fn rekey_t(&mut self) {
        if self.progress.is_none() {
            return;
        }
        let rows = std::mem::take(&mut self.rows);
        self.cells.clear();
        self.cell_ix.clear();
        self.cell_text.borrow_mut().clear();
        self.held_by.clear();
        self.progress.as_mut().unwrap().clear_rows();
        for mut row in rows {
            let key = self.cell_key(&self.progress.as_ref().unwrap().store.view(row.id));
            row.cell = Self::cell_id(&key);
            let id = row.id;
            self.place(row, Some(key));
            self.progress_place(id);
        }
        self.recount();
    }

    /// The live rows (every row but the dead ones).
    pub fn live_rows(&self) -> Vec<usize> {
        self.rows.iter().filter(|r| r.dead.is_none()).map(|r| r.id).collect()
    }

    /// The distinct rooms of the entries from the root to row i.
    pub fn chain_rooms(&self, i: usize) -> Rc<BTreeSet<i64>> {
        if let Some(s) = self.chain_rooms.borrow().get(&i) {
            return s.clone();
        }
        let mut s = match self.rows[i].parent {
            Some(p) => (*self.chain_rooms(p)).clone(),
            None => BTreeSet::new(),
        };
        if let Some(r) = self.rows[i].room {
            s.insert(r);
        }
        let s = Rc::new(s);
        self.chain_rooms.borrow_mut().insert(i, s.clone());
        s
    }

    /// An entry's key within its cell: (-rooms on its chain, -novel atoms held, frame, id).
    fn entry_key_p(&self, i: usize, nov: &HashMap<usize, i64>) -> (i64, i64, i64, usize) {
        (-(self.chain_rooms(i).len() as i64), -nov.get(&i).copied().unwrap_or(0), self.rows[i].frame, i)
    }

    /// A cell's candidate under the switch: its least live entry, not exhausted under the current T, in an allowed
    /// room, by `entry_key_p`.
    fn candidate_p(&self, cell: &str, nov: &HashMap<usize, i64>) -> Option<usize> {
        let p = self.progress.as_ref()?;
        let c = self.cell(cell);
        c.entries
            .iter()
            .copied()
            .filter(|&i| {
                let r = &self.rows[i];
                r.dead.is_none() && !p.exhausted_now(i) && r.room.map(|x| self.allowed.contains(&x)).unwrap_or(false)
            })
            .min_by_key(|&i| self.entry_key_p(i, nov))
    }

    /// The selection key under the switch: (n, chosen, -rooms on the chain, -novel atoms held, frame, id).
    pub fn sel_key_p(&self, i: usize, nov: &HashMap<usize, i64>) -> (i64, i64, i64, i64, i64, usize) {
        let c = self.cell(&self.rows[i].cell);
        let (a, b, f, id) = self.entry_key_p(i, nov);
        (c.n, c.chosen.unwrap_or(0), a, b, f, id)
    }

    /// Per entry, the atoms of T whose least live holder it is.
    pub fn novel_counts(&self) -> HashMap<usize, i64> {
        self.progress.as_ref().map(|p| p.novel_counts()).unwrap_or_default()
    }

    /// The dead moves the log gives for entry i (job.json "dead").
    pub fn dead_moves(&self, i: usize) -> Vec<Value> {
        let mut out = Vec::new();
        for x in &self.logs {
            if committed(x) && x.get("entry").and_then(|e| e.as_u64()) == Some(i as u64) {
                if let Some(d) = x.get("dead").and_then(|d| d.as_array()) {
                    out.extend(d.iter().cloned());
                }
            }
        }
        out
    }

    /// The atoms of T and the rooms that a try's new live entries hold and no live entry of a lower id holds, per
    /// committed line (in the log's order): what `new_t` and `new_rooms` are, computed from the rows (S2: a log made
    /// without the switch). Lines with no new entry give nothing.
    pub fn novelty_from_rows(&self) -> Vec<(usize, Vec<String>, Vec<i64>)> {
        let Some(p) = &self.progress else { return Vec::new() };
        let mut first: HashMap<u32, usize> = HashMap::new();
        let mut room_first: HashMap<i64, usize> = HashMap::new();
        for r in self.rows.iter().filter(|r| r.dead.is_none()) {
            for &a in &p.held[r.id] {
                first.entry(a).or_insert(r.id);
            }
            if let Some(room) = r.room {
                room_first.entry(room).or_insert(r.id);
            }
        }
        let mut out = Vec::new();
        for (k, x) in self.logs.iter().enumerate() {
            if !committed(x) || x["outcome"].as_str() == Some("return differs") {
                continue;
            }
            let ids: BTreeSet<usize> = x
                .get("new_entries")
                .and_then(|n| n.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_u64().map(|v| v as usize)).filter(|&i| i < self.rows.len()).collect())
                .unwrap_or_default();
            let mut t: Vec<String> = first.iter().filter(|(_, i)| ids.contains(i)).map(|(a, _)| p.text(*a).to_string()).collect();
            t.sort();
            let mut rooms: Vec<i64> = room_first.iter().filter(|(_, i)| ids.contains(i)).map(|(r, _)| *r).collect();
            rooms.sort();
            out.push((k, t, rooms));
        }
        out
    }

    /// S2: `new_t` and `new_rooms` written into the log lines in memory (not the file) where a line has none, from
    /// `novelty_from_rows`, and the counts counted again (on cells made afresh).
    pub fn backfill_novelty(&mut self) -> usize {
        let fill = self.novelty_from_rows();
        let mut n = 0;
        for (k, t, rooms) in fill {
            let x = &mut self.logs[k];
            if x.get("new_t").is_none() && x.get("new_rooms").is_none() {
                x["new_t"] = json!(t);
                x["new_rooms"] = json!(rooms);
                n += 1;
            }
        }
        // the cells made again, then counted from the log (recount adds to a cell's counts)
        self.rekey_t();
        n
    }

    /// n, tried and exhausted of the current cells from the committed log lines, in order (E012: n and chosen from
    /// `new_t` and `new_rooms`, and the exhausted marks with |T| at each).
    pub fn recount(&mut self) {
        let logs = std::mem::take(&mut self.logs);
        let prog = self.progress.is_some();
        if let Some(p) = self.progress.as_mut() {
            p.exhausted.clear();
        }
        for x in &logs {
            if !committed(x) {
                continue;
            }
            let Some(e) = x.get("entry").and_then(|e| e.as_u64()).map(|e| e as usize) else { continue };
            if e >= self.rows.len() {
                continue;
            }
            if prog && x["outcome"].as_str() != Some("return differs") && truthy(x.get("exhausted")) {
                let p = self.progress.as_mut().unwrap();
                let at = as_i64(x.get("t_size")).map(|n| n as usize).unwrap_or(p.t.len());
                p.exhausted.insert(e, at);
            }
            if differs_dead_on() && x["outcome"].as_str() == Some("return differs") && self.rows[e].dead.is_none()
                && !sound_result_only(x.get("return"))
            {
                self.rows[e].dead = Some("return differs".into());
            }
            let cid = self.rows[e].cell.clone();
            let c = self.cell_mut(&cid);
            if x["outcome"].as_str() != Some("return differs") {
                if crate::free::line_is_free(x) {
                    // E013: a free try counts in the cell's free counts only
                    let gave = |k: &str| x.get(k).and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false);
                    let f = c.free.get_or_insert((0, 0));
                    f.0 = if gave("new_t") || gave("new_rooms") { 0 } else { f.0 + 1 };
                    f.1 += 1;
                } else if prog {
                    let gave = |k: &str| x.get(k).and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false);
                    c.n = if gave("new_t") || gave("new_rooms") { 0 } else { c.n + 1 };
                    c.chosen = Some(c.chosen.unwrap_or(0) + 1);
                } else {
                    c.n = if truthy(x.get("new_cells")) || truthy(x.get("new_reps")) { 0 } else { c.n + 1 };
                }
            }
            if x.get("tried").map(|t| !t.is_null()).unwrap_or(false) {
                c.set_tried(e.to_string(), x["tried"].clone());
            }
            if let Some(d) = x.get("deferred").filter(|d| !d.is_null()) {
                c.set_deferred(e.to_string(), d.clone());
            }
            if truthy(x.get("exhausted")) && !c.exhausted.contains(&e) {
                c.exhausted.push(e);
            }
        }
        self.logs = logs;
    }

    /// The entry a cell offers for selection (E008: its representative unless exhausted; fix a: its least (tick, id)
    /// entry that is not exhausted).
    pub fn candidate(&self, cell: &str) -> Option<usize> {
        if self.progress.is_some() {
            return self.candidate_p(cell, &self.novel_counts());
        }
        let c = self.cell(cell);
        if self.idle_frames == 0 {
            return if c.exhausted.contains(&c.rep) { None } else { Some(c.rep) };
        }
        let mut es = c.entries.clone();
        es.sort_by_key(|&i| self.order(i));
        es.into_iter().find(|i| !c.exhausted.contains(i))
    }

    /// Fix b: {"cell": [[k, v, n], ...], "all": [...]}.
    pub fn rotation(&self, i: usize) -> Value {
        let cell = &self.rows[i].cell;
        let mut cc: HashMap<String, (Value, i64)> = HashMap::new();
        let mut aa: HashMap<String, (Value, i64)> = HashMap::new();
        for x in &self.logs {
            if !committed(x) {
                continue;
            }
            let Some(e) = x.get("entry").and_then(|e| e.as_u64()).map(|e| e as usize) else { continue };
            if e >= self.rows.len() {
                continue;
            }
            let here = &self.rows[e].cell == cell;
            if let Some(ts) = x.get("job_tried").and_then(|t| t.as_array()) {
                for t in ts {
                    let k = json!([t.get(0).cloned().unwrap_or(Value::Null), t.get(1).cloned().unwrap_or(Value::Null)]);
                    let ck = canonical(&k);
                    aa.entry(ck.clone()).or_insert((k.clone(), 0)).1 += 1;
                    if here {
                        cc.entry(ck).or_insert((k, 0)).1 += 1;
                    }
                }
            }
        }
        fn rows(d: HashMap<String, (Value, i64)>) -> Value {
            let mut v: Vec<(String, (Value, i64))> = d.into_iter().collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Array(v.into_iter().map(|(_, (k, n))| json!([k[0], k[1], n])).collect())
        }
        json!({"cell": rows(cc), "all": rows(aa)})
    }

    /// Fix c: the floor plus twice the largest reading registered.
    pub fn evict_level_gb(&self, floor_gb: f64) -> f64 {
        let big = self.readings.iter().map(|x| as_i64(x.get("bytes")).unwrap_or(0)).max().unwrap_or(0);
        floor_gb + 2.0 * big as f64 / GIB
    }

    // --- cells and the order

    pub fn cells_json(&self) -> Value {
        let mut m = Map::new();
        for (id, c) in &self.cells {
            m.insert(id.clone(), c.to_json());
        }
        Value::Object(m)
    }

    pub fn representative(&self, cell: &str) -> usize {
        self.cell(cell).rep
    }

    pub fn favoured(&self) -> HashSet<usize> {
        self.held_by.values().map(|&(_, i)| i).collect()
    }

    /// (#g, not favoured, n, -#r, tick, id)
    pub fn sel_key(&self, i: usize, fav: &HashSet<usize>) -> (i64, i64, i64, i64, i128, usize) {
        let row = &self.rows[i];
        (row.g, if fav.contains(&i) { 0 } else { 1 }, self.cell(&row.cell).n, -(row.r_path as i64), tick_ord(row.tick), i)
    }

    pub fn sel_key_json(&self, i: usize) -> Value {
        if self.progress.is_some() {
            let k = self.sel_key_p(i, &self.novel_counts());
            return json!([k.0, k.1, k.2, k.3, k.4, k.5]);
        }
        let k = self.sel_key(i, &self.favoured());
        let t = if k.4 == i128::MAX { Value::from(f64::INFINITY) } else { Value::from(k.4 as i64) };
        json!([k.0, k.1, k.2, k.3, t, k.5])
    }

    pub fn selectable(&self, cell: &str) -> bool {
        if self.progress.is_some() {
            return self.candidate(cell).is_some();
        }
        match self.candidate(cell) {
            Some(i) => self.rows[i].room.map(|r| self.allowed.contains(&r)).unwrap_or(false),
            None => false,
        }
    }

    pub fn n_selectable(&self) -> usize {
        if self.progress.is_some() {
            let nov = self.novel_counts();
            return self.cells.iter().filter(|(c, _)| self.candidate_p(c, &nov).is_some()).count();
        }
        self.cells.iter().filter(|(c, _)| self.selectable(c)).count()
    }

    /// The first ``b`` candidates in the order, each of another cell.
    pub fn select(&self, b: usize) -> Vec<usize> {
        let busy_ids: HashSet<usize> = self.busy.values().map(|(e, _)| *e).collect();
        let mut busy_cells: HashSet<&str> = self.busy.values().map(|(_, c)| c.as_str()).collect();
        for e in &busy_ids {
            busy_cells.insert(self.rows[*e].cell.as_str());
        }
        if self.progress.is_some() {
            // E012: cells by (n, chosen), then their candidates' keys (plan.md 6)
            let nov = self.novel_counts();
            if frontier_on() {
                // E016 F: the frontier's entries first
                return self.select_front(b, false, &busy_ids, &busy_cells, &nov, |i| { let k = self.sel_key_p(i, &nov); (self.place_owned_key(i), self.place_key(i, false), self.place_new_key(i, &nov, false), k.0, self.rarity_key(i), k) });
            }
            let mut cands: Vec<((i64, (i64, i64), i64, i64, i64, (i64, i64, i64, i64, i64, usize)), usize)> = self
                .cells
                .iter()
                .filter(|(cid, _)| !busy_cells.contains(cid.as_str()))
                .filter_map(|(cid, _)| self.candidate_p(cid, &nov))
                .filter(|i| !busy_ids.contains(i))
                .map(|i| { let k = self.sel_key_p(i, &nov); ((self.place_owned_key(i), self.place_key(i, false), self.place_new_key(i, &nov, false), k.0, self.rarity_key(i), k), i) })
                .collect();
            cands.sort();
            return cands.into_iter().take(b).map(|(_, i)| i).collect();
        }
        let fav = self.favoured();
        let mut cands: Vec<usize> = self
            .cells
            .iter()
            .filter(|(cid, c)| !busy_cells.contains(cid.as_str()) && !busy_ids.contains(&c.rep) && self.selectable(cid))
            .filter_map(|(cid, _)| self.candidate(cid))
            .filter(|i| !busy_ids.contains(i))
            .collect();
        cands.sort_by_key(|&i| self.sel_key(i, &fav));
        cands.truncate(b);
        cands
    }

    /// E013 (component free): a free job's selection key: E012's under the switch with the cell's free counts in place of
    /// the model's: (free_n, free_chosen, -rooms on the chain, -novel atoms held, frame, id).
    pub fn sel_key_free(&self, i: usize, nov: &HashMap<usize, i64>) -> (i64, i64, i64, i64, i64, usize) {
        let c = self.cell(&self.rows[i].cell);
        let (a, b, f, id) = self.entry_key_p(i, nov);
        let (n, ch) = c.free.unwrap_or((0, 0));
        (n, ch, a, b, f, id)
    }

    /// E013 (component free): the first ``b`` candidates for free jobs, each of another cell: E012's candidates (a cell's
    /// candidate does not depend on its counts) in the free order (`sel_key_free`). Without the progress order, the
    /// model's selection.
    pub fn select_free(&self, b: usize) -> Vec<usize> {
        if self.progress.is_none() {
            return self.select(b);
        }
        let busy_ids: HashSet<usize> = self.busy.values().map(|(e, _)| *e).collect();
        let mut busy_cells: HashSet<&str> = self.busy.values().map(|(_, c)| c.as_str()).collect();
        for e in &busy_ids {
            busy_cells.insert(self.rows[*e].cell.as_str());
        }
        let nov = self.novel_counts();
        if frontier_on() {
            // E016 F: the frontier's entries first, whichever kind the next job is
            return self.select_front(b, true, &busy_ids, &busy_cells, &nov, |i| { let k = self.sel_key_free(i, &nov); (self.place_owned_key(i), self.place_key(i, true), self.place_new_key(i, &nov, true), k.0, self.rarity_key(i), k) });
        }
        let mut cands: Vec<((i64, (i64, i64), i64, i64, i64, (i64, i64, i64, i64, i64, usize)), usize)> = self
            .cells
            .iter()
            .filter(|(cid, _)| !busy_cells.contains(cid.as_str()))
            .filter_map(|(cid, _)| self.candidate_p(cid, &nov))
            .filter(|i| !busy_ids.contains(i))
            .map(|i| { let k = self.sel_key_free(i, &nov); ((self.place_owned_key(i), self.place_key(i, true), self.place_new_key(i, &nov, true), k.0, self.rarity_key(i), k), i) })
            .collect();
        cands.sort();
        cands.into_iter().take(b).map(|(_, i)| i).collect()
    }

    /// E016 amendment (11): the place of entry i, its cell key's room and owned objects (None: a cell without a key).
    pub fn place_of_row(&self, i: usize) -> Option<String> {
        let k = self.cell(&self.rows[i].cell).key.as_ref()?;
        // E022 (FMI_PLACE_CTX=1): the place's context from the row's facts (ctx.rs)
        let ctx = match (crate::ctx::on(), self.progress.as_ref(), k.get(0).and_then(|r| r.as_i64())) {
            (true, Some(p), Some(room)) => crate::ctx::suffix(room, &p.store.view(i)),
            _ => String::new(),
        };
        Some(format!("{}|{}{ctx}", k.get(0)?, k.get(1)?))
    }

    /// E016 amendment (11) (`FMI_SELECT_PLACE=1`): the place's (n, chosen) for the model's tries (free false) or the
    /// free tries, counted as a cell's (`recount`): n is the tries from the place since one gave a new atom of T or a
    /// room, chosen every try. A place's order comes before its cells': a place whose tries keep making new cells but
    /// nothing new in T (a conversation) cannot hold every worker. (0, 0) with the switch off.
    pub fn place_key(&self, i: usize, free: bool) -> (i64, i64) {
        if !select_place_on() {
            return (0, 0);
        }
        let Some(pl) = self.place_of_row(i) else { return (0, 0) };
        let mut cache = self.place_counts.borrow_mut();
        if cache.as_ref().map(|c| c.0) != Some(self.logs.len()) {
            let mut counts: [HashMap<String, (i64, i64)>; 2] = [HashMap::new(), HashMap::new()];
            // E022 (FMI_PLACE_CTX=1): a try that adds an atom no earlier row at its new entry's place held (the
            // place with its context) gave something too, as a new atom of T or a room does archive-wide
            let ctx = crate::ctx::on();
            let mut place_held: HashMap<String, HashSet<(String, i64)>> = HashMap::new();
            if ctx {
                if let Some(pl0) = self.place_of_row(0) {
                    place_held.entry(pl0).or_default().extend(self.rows[0].held.iter().cloned());
                }
            }
            for x in &self.logs {
                if !committed(x) || x["outcome"].as_str() == Some("return differs") {
                    continue;
                }
                let Some(e) = x.get("entry").and_then(|e| e.as_u64()).map(|e| e as usize) else { continue };
                if e >= self.rows.len() {
                    continue;
                }
                let Some(q) = self.place_of_row(e) else { continue };
                let gave = |k: &str| x.get(k).and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false);
                let mut new_at_place = false;
                if ctx {
                    for id in x.get("new_entries").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|v| v.as_u64()) {
                        let id = id as usize;
                        let Some(p) = self.rows.get(id).and_then(|_| self.place_of_row(id)) else { continue };
                        let set = place_held.entry(p).or_default();
                        for a in &self.rows[id].held {
                            if set.insert(a.clone()) {
                                new_at_place = true;
                            }
                        }
                    }
                }
                let c = counts[crate::free::line_is_free(x) as usize].entry(q).or_insert((0, 0));
                c.0 = if gave("new_t") || gave("new_rooms") || new_at_place { 0 } else { c.0 + 1 };
                c.1 += 1;
            }
            *cache = Some((self.logs.len(), counts));
        }
        cache.as_ref().unwrap().1[free as usize].get(&pl).copied().unwrap_or((0, 0))
    }

    /// E022 (FMI_PLACE_CTX=1): ahead of the cell's counts, minus the atoms entry i is the first at its place to hold
    /// (`Progress::novel_counts` under the switch), while its cell has had fewer than 3 tries of the kind (model, or
    /// free): an entry that first reached something at a place is tried a few times before the place's other fresh
    /// cells. 0 otherwise.
    pub fn place_new_key(&self, i: usize, _nov: &HashMap<usize, i64>, free: bool) -> i64 {
        if !crate::ctx::on() {
            return 0;
        }
        let c = self.cell(&self.rows[i].cell);
        let n = if free { c.free.map(|f| f.1).unwrap_or(0) } else { c.chosen.unwrap_or(0) };
        if n >= 3 {
            return 0;
        }
        let mut cache = self.place_firsts.borrow_mut();
        if cache.as_ref().map(|c| c.0) != Some(self.rows.len()) {
            // the guide's atoms (Row.held, as job.json's "held"), by place in id order: the first live holder of each
            let mut seen: HashMap<String, HashSet<(String, i64)>> = HashMap::new();
            let mut firsts: HashMap<usize, i64> = HashMap::new();
            for (j, r) in self.rows.iter().enumerate() {
                if r.dead.is_some() {
                    continue;
                }
                let Some(p) = self.place_of_row(j) else { continue };
                let set = seen.entry(p).or_default();
                for a in &r.held {
                    if set.insert(a.clone()) {
                        *firsts.entry(j).or_insert(0) += 1;
                    }
                }
            }
            *cache = Some((self.rows.len(), firsts));
        }
        -cache.as_ref().unwrap().1.get(&i).copied().unwrap_or(0)
    }

    /// E018 (FMI_TARGET_PLACE=1 with FMI_SELECT_PLACE=1): the place's objects owned, negated, ahead of its counts:
    /// places holding more objects first (Go-Explore's keys held, here the inventory). 0 otherwise.
    pub fn place_owned_key(&self, i: usize) -> i64 {
        if !(select_place_on() && target_place_on()) {
            return 0;
        }
        let k = self.cell(&self.rows[i].cell).key.as_ref();
        -(k.and_then(|k| k.get(1)).and_then(|o| o.as_array()).map(|a| a.len()).unwrap_or(0) as i64)
    }

    /// E019: a job.json's fingerprint, its text less the job's name and reading folder (which name the job): two jobs
    /// with one fingerprint are the same try, which is deterministic (the archive's verification rests on that).
    pub fn job_fingerprint(doc: &Value) -> String {
        let mut d = doc.clone();
        if let Some(m) = d.as_object_mut() {
            m.remove("job");
            m.remove("reading_dir");
        }
        sha1_smol::Sha1::from(canonical(&d).as_bytes()).digest().to_string()
    }

    /// E019 (`FMI_SKIP_SAME=1`): the committed job whose job.json has the fingerprint of the job in `d`, if one has a
    /// result; else None, and `d`'s fingerprint is noted for later jobs.
    pub fn same_job(&self, d: &Path) -> Res<Option<String>> {
        let doc = read_json(&d.join("job.json"))?;
        let fp = Self::job_fingerprint(&doc);
        let mut cache = self.fingerprints.borrow_mut();
        if cache.is_none() {
            let mut m = HashMap::new();
            for name in self.committed_jobs() {
                if let Ok(j) = read_json(&self.root.join("jobs").join(&name).join("job.json")) {
                    m.entry(Self::job_fingerprint(&j)).or_insert(name);
                }
            }
            *cache = Some(m);
        }
        let m = cache.as_mut().unwrap();
        let name = d.file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
        match m.get(&fp) {
            Some(prev) if *prev != name && self.root.join("jobs").join(prev).join("result.json").exists()
                && self.committed_jobs().contains(prev) => Ok(Some(prev.clone())),
            Some(_) => Ok(None),
            None => {
                m.insert(fp, name);
                Ok(None)
            }
        }
    }

    /// E019: the result of a job that repeats `prev` exactly: `prev`'s result with this job's name, no new entries (they
    /// are in the archive already), no time spent, and "same_as".
    pub fn write_same_result(&self, d: &Path, prev: &str) -> Res<()> {
        let mut r = read_json(&self.root.join("jobs").join(prev).join("result.json"))?;
        let name = d.file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
        r["job"] = json!(name);
        r["new"] = json!([]);
        r["same_as"] = json!(prev);
        r["wall_s"] = json!(0.0);
        if let Some(c) = r.get_mut("cpu_s").and_then(|c| c.as_object_mut()) {
            for v in c.values_mut() {
                *v = json!(0.0);
            }
        }
        files::write_json(&d.join("result.json"), &r, Some(1))
    }

    /// E018 (`FMI_PLACE_RARITY=1`): entry i's rarity at its place, negated (so rarer first): the sum over its held pairs
    /// of 1 / the number of rows at the place holding the pair, times 1,000 (count-based novelty within a place, as
    /// Go-Explore weights cells by their attributes' counts). 0 with the switch off.
    pub fn rarity_key(&self, i: usize) -> i64 {
        if !std::env::var("FMI_PLACE_RARITY").is_ok_and(|v| v == "1") {
            return 0;
        }
        let Some(pl) = self.place_of_row(i) else { return 0 };
        let mut cache = self.place_pairs.borrow_mut();
        if cache.as_ref().map(|c| c.0) != Some(self.rows.len()) {
            let mut m: HashMap<String, HashMap<(String, i64), usize>> = HashMap::new();
            for r in &self.rows {
                if let Some(q) = self.place_of_row(r.id) {
                    let e = m.entry(q).or_default();
                    for p in &r.held {
                        *e.entry(p.clone()).or_insert(0) += 1;
                    }
                }
            }
            *cache = Some((self.rows.len(), m));
        }
        let counts = &cache.as_ref().unwrap().1;
        let Some(c) = counts.get(&pl) else { return 0 };
        let score: f64 = self.rows[i].held.iter().map(|p| 1.0 / *c.get(p).unwrap_or(&1) as f64).sum();
        -((score * 1000.0) as i64)
    }

    /// E016 section F (`FMI_FRONTIER=1`, plan.md "F. Configuration and selection"): the frontier, every live entry that
    /// is the first entry (least id) in its room and that no job has had as its entry yet, committed or running. Such
    /// an entry is in a room no earlier entry held, and it gets the next free worker once.
    /// Under `FMI_FRONTIER_ATOMS=1` (E022 amendment (10)) also each entry that is the least live holder in the whole
    /// archive of an atom of T (`Progress::first_counts`) and has had no try of the next job's kind (`free`), running or
    /// committed: a new atom comes first as a new room does, once for a model try and once for a free one.
    pub fn frontier(&self, free: bool) -> HashSet<usize> {
        let mut first: HashMap<i64, usize> = HashMap::new();
        for r in self.rows.iter().filter(|r| r.dead.is_none()) {
            if let Some(room) = r.room {
                first.entry(room).or_insert(r.id);
            }
        }
        let busy: HashSet<usize> = self.busy.values().map(|(e, _)| *e).collect();
        let entry_of = |x: &Value| x.get("entry").and_then(|e| e.as_u64()).map(|e| e as usize);
        let mut chosen = busy.clone();
        chosen.extend(self.logs.iter().filter_map(entry_of));
        let mut out: HashSet<usize> = first.into_values().filter(|i| !chosen.contains(i)).collect();
        if let (Some(p), true) = (&self.progress, frontier_atoms_on()) {
            let mut tried = busy;
            tried.extend(self.logs.iter().filter(|x| crate::free::line_is_free(x) == free).filter_map(entry_of));
            out.extend(p.first_counts().into_iter().filter(|&(i, n)| n > 0 && !tried.contains(&i)).map(|(i, _)| i));
        }
        out
    }

    /// E016 F: the selection with the frontier first. Each idle cell gives its candidate as before, or its least
    /// frontier entry that a candidate may be (live, not exhausted, in an allowed room); the frontier's entries come
    /// first, then the rest, each part in the order `key` gives. Under `FMI_FRONTIER_ATOMS=1` the frontier's entries
    /// come in id order (the order they were found), so a stream of new atoms cannot starve an older one.
    fn select_front<K: Ord>(&self, b: usize, free: bool, busy_ids: &HashSet<usize>, busy_cells: &HashSet<&str>,
                            nov: &HashMap<usize, i64>, key: impl Fn(usize) -> K) -> Vec<usize> {
        let front = self.frontier(free);
        let p = self.progress.as_ref();
        let may = |i: usize| {
            let r = &self.rows[i];
            r.dead.is_none() && !p.is_some_and(|p| p.exhausted_now(i)) && r.room.map(|x| self.allowed.contains(&x)).unwrap_or(false)
        };
        let mut cands: Vec<((bool, usize, K), usize)> = self
            .cells
            .iter()
            .filter(|(cid, _)| !busy_cells.contains(cid.as_str()))
            .filter_map(|(cid, c)| {
                let f = c.entries.iter().copied().filter(|&i| front.contains(&i) && may(i) && !busy_ids.contains(&i)).min();
                f.or_else(|| self.candidate_p(cid, nov))
            })
            .filter(|i| !busy_ids.contains(i))
            .map(|i| {
                let f = front.contains(&i);
                ((!f, if f && frontier_atoms_on() { i } else { 0 }, key(i)), i)
            })
            .collect();
        cands.sort_by(|a, b| a.0.cmp(&b.0));
        cands.into_iter().take(b).map(|(_, i)| i).collect()
    }

    /// Why entry `i` is or is not selected (the model's selection with the progress order and the frontier, as
    /// `select` makes it): whether it is on the frontier and may be a candidate, its cell's candidate, its key and its
    /// rank among the candidates. For reports; it selects nothing.
    pub fn explain_selection(&self, i: usize) -> Value {
        let nov = self.novel_counts();
        let front = self.frontier(false);
        let p = self.progress.as_ref();
        let r = &self.rows[i];
        let key = |i: usize| {
            let k = self.sel_key_p(i, &nov);
            (self.place_owned_key(i), self.place_key(i, false), self.place_new_key(i, &nov, false), k.0, self.rarity_key(i), k)
        };
        let may = |i: usize| {
            let r = &self.rows[i];
            r.dead.is_none() && !p.is_some_and(|p| p.exhausted_now(i)) && r.room.map(|x| self.allowed.contains(&x)).unwrap_or(false)
        };
        let mut cands: Vec<((bool, usize, _), usize)> = self
            .cells
            .iter()
            .filter_map(|(cid, c)| {
                let f = c.entries.iter().copied().filter(|&i| front.contains(&i) && may(i)).min();
                f.or_else(|| self.candidate_p(cid, &nov))
            })
            .map(|i| {
                let f = front.contains(&i);
                ((!f, if f && frontier_atoms_on() { i } else { 0 }, key(i)), i)
            })
            .collect();
        cands.sort_by(|a, b| a.0.cmp(&b.0));
        let rank = cands.iter().position(|(_, j)| *j == i);
        let cell_cand = cands.iter().find(|(_, j)| self.rows[*j].cell == r.cell).map(|(_, j)| *j);
        json!({
            "entry": i, "cell": r.cell, "room": r.room, "frame": r.frame,
            "dead": r.dead.is_some(), "exhausted": p.is_some_and(|p| p.exhausted_now(i)),
            "frontier": front.contains(&i), "novel": nov.get(&i).copied().unwrap_or(0),
            "cell_entries": self.cell(&r.cell).entries.len(), "cell_candidate": cell_cand,
            "key": format!("{:?}", key(i)), "rank": rank, "candidates": cands.len(),
            "first": cands.first().map(|(k, j)| json!({"entry": j, "key": format!("{:?}", k)})),
        })
    }

    pub fn goal_entries(&self) -> Vec<usize> {
        self.rows.iter().filter(|r| r.goal && r.dead.is_none()).map(|r| r.id).collect()
    }

    /// After the last committed job that added a pair no earlier entry held (outside I), as many committed jobs as
    /// there were selectable cells right after it, none adding one.
    /// E012's stagnation (FMI_SELECT=progress): a full sweep of the selectable cells with no new atom of T and no new
    /// room since the last try that gave one.
    pub fn stagnant(&self) -> bool {
        let done: Vec<&Value> = self.logs.iter().filter(|x| committed(x)).collect();
        if self.progress.is_some() {
            // E012: over new atoms of T and new rooms
            let gave = |x: &Value, k: &str| x.get(k).and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false);
            let last = done.iter().rposition(|x| gave(x, "new_t") || gave(x, "new_rooms"));
            let frontier = match last {
                // E014 (FMI_STAG_SWEEP=1): no try has given a new atom of T or a new room yet: the sweep is of the
                // cells selectable now (the last committed try's count), not of the root's cell alone
                None if stag_sweep() => done.last().and_then(|x| as_i64(x.get("selectable"))).unwrap_or(1).max(1),
                None => 1,
                Some(l) => as_i64(done[l].get("selectable")).unwrap_or(1).max(1),
            };
            let since = done.len() as i64 - last.map(|l| l as i64 + 1).unwrap_or(0);
            return since >= frontier && (!stag_cells() || self.swept(&done[last.map(|l| l + 1).unwrap_or(0)..]));
        }
        let last = done
            .iter()
            .enumerate()
            .filter(|(_, x)| {
                x.get("new_r")
                    .and_then(|r| r.as_array())
                    .map(|ps| ps.iter().any(|p| p.get(0).and_then(|k| k.as_str()).map(|k| !self.idle.contains(k)).unwrap_or(true)))
                    .unwrap_or(false)
            })
            .map(|(k, _)| k)
            .max();
        let frontier = match last {
            None => 1,
            Some(l) => as_i64(done[l].get("selectable")).unwrap_or(1).max(1),
        };
        let since = done.len() as i64 - last.map(|l| l as i64 + 1).unwrap_or(0);
        since >= frontier && (!stag_cells() || self.swept(&done[last.map(|l| l + 1).unwrap_or(0)..]))
    }

    /// E014 amendment (7) (`FMI_STAG_CELLS=1`): whether every cell selectable now has had a try among the committed
    /// lines `since` (those after the last try that gave progress). A try is a committed line of either kind, errors
    /// included and "return differs" excluded (as `recount` counts them); its cell is its entry's cell now.
    pub fn swept(&self, since: &[&Value]) -> bool {
        self.untried_selectable(since).is_empty()
    }

    /// The cells selectable now (the selection's own test) with no try among the committed lines `since`, in the cells'
    /// order.
    pub fn untried_selectable(&self, since: &[&Value]) -> Vec<String> {
        let tried: HashSet<&str> = since
            .iter()
            .filter(|x| x["outcome"].as_str() != Some("return differs"))
            .filter_map(|x| x.get("entry").and_then(|e| e.as_u64()).map(|e| e as usize))
            .filter(|&e| e < self.rows.len())
            .map(|e| self.rows[e].cell.as_str())
            .collect();
        if self.progress.is_some() {
            let nov = self.novel_counts();
            return self
                .cells
                .iter()
                .filter(|(c, _)| !tried.contains(c.as_str()) && self.candidate_p(c, &nov).is_some())
                .map(|(c, _)| c.clone())
                .collect();
        }
        self.cells.iter().filter(|(c, _)| !tried.contains(c.as_str()) && self.selectable(c)).map(|(c, _)| c.clone()).collect()
    }

    /// {(entry, error): count} over the committed jobs that failed.
    pub fn failures(&self) -> HashMap<(i64, String), i64> {
        let mut out = HashMap::new();
        for x in &self.logs {
            if x.get("outcome").and_then(|o| o.as_str()) == Some("error") {
                let k = (as_i64(x.get("entry")).unwrap_or(-1), canonical(x.get("error").unwrap_or(&Value::Null)));
                *out.entry(k).or_insert(0) += 1;
            }
        }
        out
    }

    pub fn failures_of(&self, entry: i64, error: &Value) -> i64 {
        self.failures().get(&(entry, canonical(error))).copied().unwrap_or(0)
    }

    // --- readings

    /// The live readings of ``scope``, newest first.
    pub fn readings_for(&self, scope: &[i64]) -> Vec<&ReadingRow> {
        let mut s = scope.to_vec();
        s.sort();
        let sv = json!(s);
        self.readings
            .iter()
            .rev()
            .filter(|x| {
                x.get("scope") == Some(&sv)
                    && !truthy(x.get("evicted"))
                    && (!read_held_on() || truthy(x.get("held")))
                    && x.get("path").and_then(|p| p.as_str()).map(|p| Path::new(p).exists()).unwrap_or(false)
            })
            .collect()
    }

    pub fn register_reading(&mut self, scope: &[i64], path: &str, start_entry: i64, job: Option<&str>, kind: &str,
                            used: Option<i64>) -> Res<()> {
        let p = Path::new(path);
        let mut s = scope.to_vec();
        s.sort();
        let parent = p.parent().unwrap_or(Path::new(""));
        let bytes = if parent.exists() { files::dir_bytes(parent) } else { 0 };
        let mut row = json!({"scope": s, "path": path, "start_entry": start_entry, "job": job, "bytes": bytes,
                             "last_used": used, "kind": kind});
        if read_held_on() {
            row["held"] = json!(true);
        }
        files::append_jsonl(&self.root.join("readings.jsonl"), std::slice::from_ref(&row))?;
        self.readings.push(row.as_object().unwrap().clone());
        Ok(())
    }

    pub fn touch_reading(&mut self, path: &str, used: i64) -> Res<()> {
        for x in self.readings.iter_mut() {
            if x.get("path").and_then(|p| p.as_str()) == Some(path) {
                x.insert("last_used".into(), Value::from(used));
            }
        }
        self.rewrite_readings()
    }

    /// Deletes readings least recently used first, never one in ``in_use``, while ``free_fn()`` is under the evict
    /// level. A reading's files go; its start files stay. Returns the paths evicted.
    pub fn evict(&mut self, free_fn: &mut dyn FnMut() -> f64, in_use: &HashSet<String>, level_gb: f64) -> Res<Vec<String>> {
        let level = level_gb * GIB;
        let mut order: Vec<usize> = (0..self.readings.len())
            .filter(|&k| {
                !truthy(self.readings[k].get("evicted"))
                    && !in_use.contains(self.readings[k].get("path").and_then(|p| p.as_str()).unwrap_or(""))
            })
            .collect();
        order.sort_by_key(|&k| (as_i64(self.readings[k].get("last_used")).unwrap_or(-1), k));
        let mut out = Vec::new();
        let root = fs::canonicalize(&self.root).unwrap_or(self.root.clone());
        for k in order {
            if free_fn() >= level {
                break;
            }
            let path = self.readings[k].get("path").and_then(|p| p.as_str()).unwrap_or("").to_string();
            let d = resolve(Path::new(&path).parent().unwrap_or(Path::new("")));
            let live: Vec<PathBuf> = self
                .readings
                .iter()
                .enumerate()
                .filter(|(j, y)| *j != k && !truthy(y.get("evicted")))
                .map(|(_, y)| resolve(Path::new(y.get("path").and_then(|p| p.as_str()).unwrap_or("")).parent().unwrap_or(Path::new(""))))
                .collect();
            if d.starts_with(&root) && d != root && d != root.join("readings") && d != root.join("jobs") {
                let keep: Vec<PathBuf> = live.into_iter().filter(|q| *q == d || q.starts_with(&d)).collect();
                evict_dir(&d, &keep);
            }
            self.readings[k].insert("evicted".into(), Value::Bool(true));
            out.push(path);
        }
        if !out.is_empty() {
            self.rewrite_readings()?;
        }
        Ok(out)
    }

    // --- jobs

    pub fn job_names(&self) -> Vec<String> {
        let mut out: Vec<String> = fs::read_dir(self.root.join("jobs"))
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| n.len() > 1 && n.starts_with('j') && n[1..].bytes().all(|c| c.is_ascii_digit()))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }

    pub fn next_job(&self) -> String {
        let names = self.job_names();
        let n = names.last().map(|l| l[1..].parse::<u64>().unwrap_or(0) + 1).unwrap_or(0);
        format!("j{n:05}")
    }

    pub fn committed_jobs(&self) -> HashSet<String> {
        self.logs.iter().filter(|x| committed(x)).filter_map(|x| x["job"].as_str().map(|s| s.to_string())).collect()
    }

    /// Job dirs with a job.json and no committed log line, in start order.
    pub fn unfinished_jobs(&self) -> Vec<String> {
        let done = self.committed_jobs();
        self.job_names()
            .into_iter()
            .filter(|n| !done.contains(n) && self.root.join("jobs").join(n).join("job.json").exists())
            .collect()
    }

    /// The scope of a try from an entry in `room` (None: no room): E009's ring, or with E014's grown scopes (amendment
    /// (4) (b)) the room and the rooms grown for it, cut to the ring, or with E016's room scopes the room alone.
    pub fn scope_of_room(&self, room: Option<i64>) -> Vec<i64> {
        let r = room.unwrap_or(i64::MIN);
        let rg = ring(&self.edges, r, &self.allowed);
        match &self.grown {
            Some(g) => crate::grow::scope(r, &rg, g.get(&r)),
            None if crate::grow::room_only() => crate::grow::scope(r, &rg, None),
            None => rg,
        }
    }

    /// E014 amendment (4) (a): the registered readings of other scopes than `scope` that hold `room`, newest first
    /// (a bounded try's stand-in when its scope has none).
    pub fn readings_holding(&self, room: i64, scope: &[i64]) -> Vec<&ReadingRow> {
        let mut s = scope.to_vec();
        s.sort();
        let sv = json!(s);
        self.readings
            .iter()
            .rev()
            .filter(|x| {
                x.get("scope") != Some(&sv)
                    && x.get("scope").and_then(|v| v.as_array()).map(|a| a.iter().any(|r| r.as_i64() == Some(room))).unwrap_or(false)
                    && !truthy(x.get("evicted"))
                    && (!read_held_on() || truthy(x.get("held")))
                    && x.get("path").and_then(|p| p.as_str()).map(|p| Path::new(p).exists()).unwrap_or(false)
            })
            .collect()
    }

    /// job.json (plan.md "Interfaces").
    pub fn job_doc(&self, job: &str, i: usize) -> Res<Value> {
        let row = &self.rows[i];
        let cell = self.cell(&row.cell);
        let scope = self.scope_of_room(row.room);
        let rooms = scope.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("-");
        let mut held: Vec<&(String, i64)> = if target_place_on() {
            // E018 (FMI_TARGET_PLACE=1): the atoms held at the entry's place only, so a fact reached elsewhere stays a
            // target where it was not (a conversation's facts, reached without an object, with the object in hand)
            let pl = self.place_of_row(i);
            let mut s: BTreeSet<&(String, i64)> = BTreeSet::new();
            for r in &self.rows {
                if self.place_of_row(r.id) == pl {
                    s.extend(r.held.iter());
                }
            }
            s.into_iter().collect()
        } else {
            self.held_by.keys().collect()
        };
        held.sort();
        let readings: Vec<Value> = self
            .readings_for(&scope)
            .into_iter()
            .map(|x| json!({"path": x.get("path"), "start_entry": x.get("start_entry")}))
            .collect();
        let env_int = |k: &str, d: i64| std::env::var(k).ok().filter(|v| !v.is_empty()).and_then(|v| v.parse().ok()).unwrap_or(d);
        let info = self.info(i)?;
        let mut doc = json!({
            "job": job, "seed": SEED, "stage": self.stage["stage"], "goal": self.goal_json, "guide": self.stage["guide"],
            "entry": {"id": i, "root": row.parent.is_none(), "frame": row.frame, "events": self.movie(i)?,
                      "checkpoint": info.last_cp()},
            "scope": scope, "tried": cell.tried_of(&i.to_string()).cloned().unwrap_or_else(|| json!([])),
            "held": held.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
            "readings": readings,
            "reading_dir": self.root.join("readings").join(format!("{rooms}-{job}")).to_string_lossy(),
            "threads": env_int("E008_THREADS", THREADS), "workers": env_int("E008_WINDOW_WORKERS", WINDOW_WORKERS),
        });
        let m = doc.as_object_mut().unwrap();
        for (k, v) in self.job_e009(i).as_object().unwrap() {
            m.insert(k.clone(), v.clone());
        }
        if truthy(self.e010.get("ready")) {
            m.insert("ready".into(), Value::Bool(true));
        }
        if crate::step::deepen().is_some() {
            m.insert("deferred".into(), cell.deferred_of(&i.to_string()).cloned().unwrap_or_else(|| json!([])));
        }
        if let Some(p) = &self.progress {
            let live = self.live_rows();
            let mut nv = progress::novelty_json(p, &live);
            if target_place_on() {
                // E018: novelty per place, the targets' partition on the keeping side
                nv["by_place"] = progress::by_place_json(p, &live, |r| self.rows[r].room);
            }
            m.insert("novelty".into(), nv);
            m.insert("dead".into(), Value::Array(self.dead_moves(i)));
        }
        if crate::nogood::on() {
            // E016 C: the entry's refutations, and the no-plan records of the scope's newest reading (a record of a
            // reading that a newer one of its scope replaced, or that was evicted, is dropped)
            let live: HashSet<String> =
                self.readings_for(&scope).first().and_then(|x| x.get("path")).and_then(|p| p.as_str()).map(|p| p.to_string()).into_iter().collect();
            let lines: Vec<&Value> = self.logs.iter().filter(|x| committed(x)).collect();
            m.insert("nogood".into(), crate::nogood::job_json(&lines, i as i64, &live));
        }
        // E014 amendment (4): the whole ring for the try's scope need (b), the readings that may stand in (a)
        if self.grown.is_some() {
            m.insert("ring".into(), json!(ring(&self.edges, row.room.unwrap_or(i64::MIN), &self.allowed)));
        }
        if crate::bound::on() {
            if let Some(room) = row.room {
                let rs: Vec<Value> = self.readings_holding(room, &scope).into_iter()
                    .map(|x| json!({"path": x.get("path"), "start_entry": x.get("start_entry"), "scope": x.get("scope")})).collect();
                m.insert("readings_room".into(), Value::Array(rs));
            }
        }
        Ok(doc)
    }

    /// job.json's E009 switches for entry ``i``; {} with every switch off.
    pub fn job_e009(&self, i: usize) -> Value {
        let mut out = Map::new();
        if self.idle_frames != 0 {
            out.insert("idle_frames".into(), Value::from(self.idle_frames));
            out.insert("idle_measured".into(), Value::Bool(self.idle_measured.contains(&(i as i64))));
        }
        if truthy(self.e009.get("rotate")) {
            out.insert("rotation".into(), self.rotation(i));
        }
        if truthy(self.e009.get("reuse_nonfacts")) {
            out.insert("reuse".into(), Value::from("e009"));
        }
        Value::Object(out)
    }

    /// E013 (component free): `prepare_job` with the job's kind: job.json "kind", select.json "kind", "why" and "rho",
    /// and a free job's key in the free order. None: `prepare_job`, E012's files.
    pub fn prepare_job_as(&self, i: usize, kind: Option<&crate::free::Kind>) -> Res<PathBuf> {
        let Some(k) = kind else { return self.prepare_job(i) };
        let job = self.next_job();
        let d = self.root.join("jobs").join(&job);
        fs::create_dir_all(&d).map_err(|e| files::io_err(&d, e))?;
        let key = if k.free && self.progress.is_some() {
            let x = self.sel_key_free(i, &self.novel_counts());
            json!([x.0, x.1, x.2, x.3, x.4, x.5])
        } else {
            self.sel_key_json(i)
        };
        let mut sel = json!({"key": key, "cell": self.rows[i].cell, "selectable": self.n_selectable(), "time": files::now_s(),
                             "kind": k.text(), "why": k.why, "rho": k.rho});
        if frontier_on() {
            sel["frontier"] = json!(self.frontier(k.free).contains(&i));
        }
        if select_place_on() {
            sel["place"] = json!(self.place_of_row(i));
            sel["place_key"] = json!(self.place_key(i, k.free));
        }
        files::write_json(&d.join("select.json"), &sel, Some(1))?;
        let mut doc = self.job_doc(&job, i)?;
        doc["kind"] = json!(k.text());
        if k.free && crate::free::t_archive() {
            // E016 D: the registered readings that may stand in for the wait, as the try never reads
            let rows = |v: Vec<&ReadingRow>| -> Value {
                Value::Array(v.into_iter().map(|x| json!({"path": x.get("path"), "start_entry": x.get("start_entry"), "scope": x.get("scope")})).collect())
            };
            if doc.get("readings_room").is_none() {
                let room = self.rows[i].room;
                doc["readings_room"] = rows(room.map(|r| self.readings_holding(r, &self.scope_of_room(Some(r)))).unwrap_or_default());
            }
            doc["readings_any"] = rows(self.readings_any(crate::free::READINGS_ANY));
        }
        files::write_json(&d.join("job.json"), &doc, Some(1))?;
        Ok(d)
    }

    /// E016 D: the live registered readings of any scope, newest first, at most `n`.
    pub fn readings_any(&self, n: usize) -> Vec<&ReadingRow> {
        self.readings
            .iter()
            .rev()
            .filter(|x| !truthy(x.get("evicted")) && x.get("path").and_then(|p| p.as_str()).map(|p| Path::new(p).exists()).unwrap_or(false))
            .take(n)
            .collect()
    }

    /// jobs/JOB/job.json for candidate ``i``, and select.json.
    pub fn prepare_job(&self, i: usize) -> Res<PathBuf> {
        let job = self.next_job();
        let d = self.root.join("jobs").join(&job);
        fs::create_dir_all(&d).map_err(|e| files::io_err(&d, e))?;
        let mut sel = json!({"key": self.sel_key_json(i), "cell": self.rows[i].cell, "selectable": self.n_selectable(),
                             "time": files::now_s()});
        if frontier_on() {
            // E016 F: whether the entry was the frontier's
            sel["frontier"] = json!(self.frontier(false).contains(&i));
        }
        files::write_json(&d.join("select.json"), &sel, Some(1))?;
        files::write_json(&d.join("job.json"), &self.job_doc(&job, i)?, Some(1))?;
        Ok(d)
    }

    /// Commits a finished job: its new entries, cells.json, readings.jsonl, then its log.jsonl line. Returns it.
    pub fn ingest(&mut self, job_dir: &Path, extra: Option<&Map<String, Value>>) -> Res<Value> {
        let job = read_json(&job_dir.join("job.json"))?;
        let res = read_json(&job_dir.join("result.json"))?;
        let name = job["job"].as_str().unwrap_or("").to_string();
        let e = job["entry"]["id"].as_u64().ok_or("job.json: entry id")? as usize;
        if self.committed_jobs().contains(&name) {
            return Err(format!("ValueError: {name} is committed already"));
        }
        let empty = json!({});
        let ret = or(res.get("return"), &empty).clone();
        let mut idle_line = Value::Null;
        if self.idle_frames != 0 && truthy(res.get("idle")) && truthy(ret.get("same")) {
            let returns: Vec<String> = res["idle"]
                .get("returns")
                .and_then(|r| r.as_array())
                .map(|a| a.iter().filter_map(|k| k.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let new_idle = self.merge_idle(e as i64, &returns)?;
            let goal = self.goal_keys();
            idle_line = json!({"entry": e, "keys": returns.iter().filter(|k| !goal.contains(k.as_str())).collect::<Vec<_>>(),
                               "new": new_idle, "size": self.idle.len(), "frames": res["idle"].get("frames"),
                               "seconds": res["idle"].get("seconds")});
        }
        // E012: the try's T joins the archive's before anything is placed (the rows are keyed again when T grows)
        let mut t_new: Vec<TAtom> = Vec::new();
        if self.progress.is_some() && ret.get("same") != Some(&Value::Bool(false)) {
            t_new = self.progress.as_mut().unwrap().merge(&progress::atoms_of_json(res.get("tested")));
            if !t_new.is_empty() {
                self.rekey_t();
            }
        }
        let cell = self.rows[e].cell.clone();
        let sel = if job_dir.join("select.json").exists() { read_json(&job_dir.join("select.json"))? } else { json!({}) };
        let mut line = Map::new();
        line.insert("job".into(), json!(name));
        line.insert("entry".into(), json!(e));
        line.insert("cell".into(), json!(cell));
        line.insert("key".into(), sel.get("key").cloned().unwrap_or(Value::Null));
        line.insert("scope".into(), job["scope"].clone());
        let ok = truthy(res.get("ok"));
        let mut error = if !ok { res.get("error").cloned().unwrap_or(Value::Null) } else { Value::Null };
        let mut new_files: Vec<(Value, NewDoc)> = Vec::new();
        let outcome;
        if ret.get("same") == Some(&Value::Bool(false)) {
            outcome = "return differs";
        } else {
            let mut oc = if ok { "ok" } else { "error" };
            match self.check_new(&res, &name, e, job_dir) {
                Ok(nf) => new_files = nf,
                Err(msg) => {
                    oc = "error";
                    error = Value::from(format!("ingest: {msg}"));
                    new_files.clear();
                }
            }
            outcome = oc;
        }
        let prof = std::env::var("FMI_ARCHIVE_PROFILE").is_ok();
        let tp = std::time::Instant::now();
        let cp0 = crate::machine::thread_cpu_s();
        let before: HashSet<(String, i64)> = self.held_by.keys().cloned().collect();
        let (before_t, before_rooms): (HashSet<u32>, BTreeSet<i64>) = match &self.progress {
            Some(p) => (p.first.keys().copied().collect(), p.rooms.keys().copied().collect()),
            None => (HashSet::new(), BTreeSet::new()),
        };
        let (mut ids, mut new_cells, mut new_reps) = (Vec::new(), 0i64, 0i64);
        let mut hows: Vec<String> = Vec::new();
        self.pending_rows = Some(Vec::new());
        self.pending_files = Some(Vec::new());
        for (n, doc) in &new_files {
            let how = n.get("how").and_then(|h| h.as_str()).filter(|h| !h.is_empty()).unwrap_or("checkpoint").to_string();
            if self.progress.is_some() {
                self.next_dead =
                    n.get("dead").filter(|d| !d.is_null()).map(|d| d.as_str().map(|s| s.to_string()).unwrap_or_else(|| d.to_string()));
            }
            let id = match doc {
                NewDoc::Doc(doc) => self.add(
                    Some(e),
                    as_i64(doc.get("frame")).unwrap_or(0),
                    as_i64(doc.get("tick")),
                    doc["events"].as_array().map(|a| a.as_slice()).unwrap_or(&[]),
                    doc["checkpoints"].as_array().map(|a| a.as_slice()).unwrap_or(&[]),
                    Some(&name),
                    &how,
                )?,
                NewDoc::Shared(ne) => self.add_shared(Some(e), ne, Some(&name), &how)?,
            };
            ids.push(id);
            hows.push(how.clone());
            new_cells += self.last_place.0 as i64;
            new_reps += self.last_place.1 as i64;
        }
        // the entries' files first (compressed in parallel), then their rows
        if let Some(fs) = self.pending_files.take() {
            let errs: Vec<String> = std::thread::scope(|sc| {
                let hs: Vec<_> = fs.iter().map(|(p, t)| sc.spawn(move || files::write_gz_text(p, t))).collect();
                hs.into_iter().filter_map(|h| h.join().ok().and_then(|r| r.err())).collect()
            });
            if let Some(e) = errs.into_iter().next() {
                self.pending_rows = None;
                return Err(e);
            }
        }
        if let Some(rows) = self.pending_rows.take() {
            if !rows.is_empty() {
                files::append_jsonl(&self.root.join("entries.jsonl"), &rows)?;
            }
        }
        let t_add = tp.elapsed().as_secs_f64();
        let mut new_r: Vec<(String, i64)> = self.held_by.keys().filter(|p| !before.contains(*p)).cloned().collect();
        new_r.sort();
        // E012: the atoms of T and the rooms the new live entries hold and no earlier live entry held
        let (mut new_t, mut new_rooms, mut dead_entries): (Vec<String>, Vec<i64>, Vec<usize>) = (Vec::new(), Vec::new(), Vec::new());
        if let Some(p) = &self.progress {
            let mut ts: BTreeSet<u32> = BTreeSet::new();
            let mut rs: BTreeSet<i64> = BTreeSet::new();
            for &i in &ids {
                if self.rows[i].dead.is_some() {
                    dead_entries.push(i);
                    continue;
                }
                ts.extend(p.held[i].iter().copied().filter(|a| !before_t.contains(a)));
                if let Some(r) = self.rows[i].room.filter(|r| !before_rooms.contains(r)) {
                    rs.insert(r);
                }
            }
            new_t = ts.into_iter().map(|a| p.text(a).to_string()).collect();
            new_t.sort();
            new_rooms = rs.into_iter().collect();
        }
        let gave = !new_t.is_empty() || !new_rooms.is_empty();
        if outcome == "return differs" && differs_dead_on() && self.rows[e].dead.is_none() && !sound_result_only(res.get("return")) {
            self.rows[e].dead = Some("return differs".into());
        }
        if outcome != "return differs" {
            let mut tried: HashMap<String, Value> = HashMap::new();
            if let Some(t) = self.cell(&cell).tried_of(&e.to_string()).and_then(|t| t.as_array()) {
                for x in t {
                    tried.insert(canonical(x), x.clone());
                }
            }
            if let Some(t) = res.get("tried").and_then(|t| t.as_array()) {
                for x in t {
                    tried.insert(canonical(x), x.clone());
                }
            }
            let mut tv: Vec<(String, Value)> = tried.into_iter().collect();
            tv.sort_by(|a, b| a.0.cmp(&b.0));
            let exhausted = truthy(res.get("exhausted"));
            // the deepening's deferred targets: the old ones not tried now, then this try's (with their new counts)
            let deferred = if crate::step::deepen().is_some() {
                let key = |x: &Value| canonical(&json!([x.get(0).cloned().unwrap_or(Value::Null), x.get(1).cloned().unwrap_or(Value::Null)]));
                let tried_now: HashSet<String> = tv.iter().map(|(k, _)| k.clone()).collect();
                let new: Vec<Value> = res.get("deferred").and_then(|d| d.as_array()).cloned().unwrap_or_default();
                let new_keys: HashSet<String> = new.iter().map(key).collect();
                let mut d: Vec<Value> = self.cell(&cell).deferred_of(&e.to_string()).and_then(|d| d.as_array()).cloned().unwrap_or_default();
                d.retain(|x| !tried_now.contains(&key(x)) && !new_keys.contains(&key(x)));
                d.extend(new);
                Some(Value::Array(d))
            } else {
                None
            };
            let prog = self.progress.is_some();
            let c = self.cell_mut(&cell);
            if crate::free::job_is_free(&job) {
                // E013: a free try counts in the cell's free counts only
                let f = c.free.get_or_insert((0, 0));
                f.0 = if gave { 0 } else { f.0 + 1 };
                f.1 += 1;
            } else if prog {
                c.n = if gave { 0 } else { c.n + 1 };
                c.chosen = Some(c.chosen.unwrap_or(0) + 1);
            } else {
                c.n = if new_cells != 0 || new_reps != 0 { 0 } else { c.n + 1 };
            }
            c.set_tried(e.to_string(), Value::Array(tv.into_iter().map(|(_, v)| v).collect()));
            if let Some(d) = deferred {
                c.set_deferred(e.to_string(), d);
            }
            if exhausted && !c.exhausted.contains(&e) {
                c.exhausted.push(e);
            }
            if let Some(p) = self.progress.as_mut().filter(|_| exhausted) {
                let n = p.t.len();
                p.exhausted.insert(e, n);
            }
        }
        let used = self.committed_jobs().len() as i64;
        let rd = or(res.get("reading"), &empty).clone();
        if truthy(rd.get("path")) && outcome != "return differs" {
            let path = rd["path"].as_str().unwrap_or("").to_string();
            if truthy(rd.get("fresh")) {
                let scope = scope_of(or(rd.get("scope"), &job["scope"]));
                self.register_reading(&scope, &path, e as i64, Some(&name), "fresh", Some(used))?;
            } else {
                self.touch_reading(&path, used)?;
            }
        }
        if outcome != "return differs" {
            if let Some(ms) = res.get("readings_made").filter(|m| truthy(Some(m))).and_then(|m| m.as_array()) {
                for m in ms {
                    let scope = scope_of(or(m.get("scope"), &job["scope"]));
                    let kind = m.get("kind").and_then(|k| k.as_str()).unwrap_or("made").to_string();
                    let path = py_str(m.get("path"));
                    self.register_reading(&scope, &path, e as i64, Some(&name), &kind, Some(used))?;
                }
            }
        }
        let t_read = tp.elapsed().as_secs_f64();
        self.write_cells()?;
        let t_cells = tp.elapsed().as_secs_f64();
        let att = or(res.get("attempt"), &empty).clone();
        let n_now = self.cell(&cell).n;
        let tried_now = self.cell(&cell).tried_of(&e.to_string()).cloned().unwrap_or(Value::Null);
        let goal_entries: Vec<usize> = ids.iter().copied().filter(|&i| self.rows[i].goal).collect();
        line.insert("outcome".into(), json!(outcome));
        line.insert("error".into(), error);
        line.insert("new_entries".into(), json!(ids));
        line.insert("new_cells".into(), json!(new_cells));
        line.insert("new_reps".into(), json!(new_reps));
        line.insert("new_r".into(), pairs_json(&new_r));
        line.insert("n".into(), json!(n_now));
        line.insert("tried".into(), tried_now);
        if crate::step::deepen().is_some() {
            line.insert("deferred".into(), self.cell(&cell).deferred_of(&e.to_string()).cloned().unwrap_or_else(|| json!([])));
        }
        line.insert("exhausted".into(), json!(truthy(res.get("exhausted"))));
        line.insert("selectable".into(), json!(self.n_selectable()));
        line.insert("goal_entries".into(), json!(goal_entries));
        line.insert("return".into(), ret.clone());
        let mut rdl = Map::new();
        for k in ["path", "fresh", "why", "scope", "records", "actions", "read_wall_s", "read_cpu_s", "peak_rss_mb", "files_s"] {
            rdl.insert(k.into(), rd.get(k).cloned().unwrap_or(Value::Null));
        }
        line.insert("reading".into(), Value::Object(rdl));
        line.insert("targets".into(), or(res.get("targets"), &empty).get("by_tier").cloned().unwrap_or(Value::Null));
        line.insert("probes".into(), json!(or(res.get("probes"), &json!([])).as_array().map(|a| a.len()).unwrap_or(0)));
        if truthy(Some(&att)) {
            let mut a = Map::new();
            for k in ["target", "stop", "reached", "goal_from_the_plan", "frames", "qf0", "qf_end", "replans",
                      "refinements", "extensions", "seconds"] {
                a.insert(k.into(), att.get(k).cloned().unwrap_or(Value::Null));
            }
            line.insert("attempt".into(), Value::Object(a));
        } else {
            line.insert("attempt".into(), Value::Null);
        }
        for k in ["cpu_s", "peak_rss_mb", "wall_s"] {
            line.insert(k.into(), res.get(k).cloned().unwrap_or(Value::Null));
        }
        if !self.e009.is_empty() {
            line.insert("job_tried".into(), or(res.get("tried"), &json!([])).clone());
            line.insert("idle".into(), idle_line);
        }
        if !self.e010.is_empty() {
            line.insert("commit_index".into(), json!(used));
        }
        if let Some(p) = &self.progress {
            line.insert("t_new".into(), progress::atoms_json(&t_new));
            line.insert("t_size".into(), json!(p.t.len()));
            line.insert("new_t".into(), json!(new_t));
            line.insert("new_rooms".into(), json!(new_rooms));
            line.insert("chosen".into(), json!(self.cell(&cell).chosen));
            line.insert("dead_entries".into(), json!(dead_entries));
            let dead = if outcome != "return differs" { res.get("dead").filter(|d| d.is_array()).cloned() } else { None };
            line.insert("dead".into(), dead.unwrap_or_else(|| json!([])));
        }
        // E016 C (FMI_NOGOOD): what the try added to the failure memory (job_doc gives it to later tries)
        if let Some(ng) = res.get("nogood").filter(|_| outcome != "return differs") {
            line.insert("nogood".into(), ng.clone());
        }
        if let Some(k) = job.get("kind") {
            // E013 (FMI_FREE): the job's kind, and the cell's free counts after it
            line.insert("kind".into(), k.clone());
            line.insert("free".into(), json!(self.cell(&cell).free));
        }
        // E014 amendment (4) (b): the scope of the entry's room grows by the rooms the try needed and the rooms its
        // model-free entries entered (grow.rs), checked against the ring and the scope as they stand at this commit
        if self.grown.is_some() && outcome != "return differs" {
            if let Some(r0) = self.rows[e].room {
                let rg = ring(&self.edges, r0, &self.allowed);
                let sc = self.scope_of_room(Some(r0));
                let left: Vec<(String, Option<i64>)> = ids.iter().zip(&hows).map(|(&i, h)| (h.clone(), self.rows[i].room)).collect();
                let add = crate::grow::growth(res.get("scope_need"), &left, &sc, &rg);
                if !add.is_empty() {
                    self.grown.as_mut().unwrap().entry(r0).or_default().extend(add.iter().map(|x| x.0));
                    line.insert("grow".into(), json!({"room": r0, "add": add.iter().map(|x| x.0).collect::<Vec<_>>(),
                                                       "why": add.iter().map(|x| x.1.clone()).collect::<Vec<_>>(),
                                                       "scope": self.scope_of_room(Some(r0))}));
                }
            }
        }
        if let Some(x) = extra {
            for (k, v) in x {
                line.insert(k.clone(), v.clone());
            }
        }
        let line = Value::Object(line);
        let t_line = tp.elapsed().as_secs_f64();
        files::append_jsonl(&self.root.join("log.jsonl"), std::slice::from_ref(&line))?;
        self.logs.push(line.clone());
        if prof {
            let t = tp.elapsed().as_secs_f64();
            eprintln!("ingest {name}: {} entries; add {:.2} ms, readings {:.2}, cells {:.2}, line {:.2}, log append {:.2} ms; CPU {:.2} ms",
                      ids.len(), t_add * 1e3, (t_read - t_add) * 1e3, (t_cells - t_read) * 1e3, (t_line - t_cells) * 1e3, (t - t_line) * 1e3,
                      (crate::machine::thread_cpu_s() - cp0) * 1e3);
        }
        Ok(line)
    }

    /// ingest's checks of the result and its new entries (explore.py's try block): the entry files, or the error's
    /// text as Python gives it (`TypeName: message`).
    fn check_new(&self, res: &Value, name: &str, e: usize, job_dir: &Path) -> Result<Vec<(Value, NewDoc)>, String> {
        if res.get("job").and_then(|j| j.as_str()) != Some(name) || res.get("entry").and_then(|x| x.as_u64()) != Some(e as u64) {
            return Err(format!("ValueError: result.json names job {} entry {}", py_str(res.get("job")), py_str(res.get("entry"))));
        }
        let mut out = Vec::new();
        let empty = json!([]);
        for n in or(res.get("new"), &empty).as_array().cloned().unwrap_or_default() {
            let file = n.get("file").ok_or("KeyError: 'file'")?;
            let fname = py_str(Some(file));
            let p = job_dir.join(&fname);
            let doc = match std::fs::File::open(&p) {
                Err(err) => return Err(format!("{}: {}", files::python_os_error_class(&err), files::python_os_error(&p, err))),
                Ok(_) => match new_cache().lock().unwrap().remove(&resolve(&p)) {
                    Some(d) => d,
                    None => NewDoc::Doc(read_gz(&p).map_err(|m| format!("JSONDecodeError: {m}"))?),
                },
            };
            let f0 = self.rows[e].frame;
            let bad = match &doc {
                NewDoc::Doc(doc) => {
                    if doc.get("parent").and_then(|x| x.as_u64()) != Some(e as u64) {
                        return Err(format!("ValueError: {fname}: parent {}, not the job's entry {e}", py_str(doc.get("parent"))));
                    }
                    let frame = as_i64(doc.get("frame")).ok_or("KeyError: 'frame'")?;
                    let cps = doc.get("checkpoints").ok_or("KeyError: 'checkpoints'")?.as_array().cloned().unwrap_or_default();
                    let events = doc.get("events").ok_or("KeyError: 'events'")?.as_array().cloned().unwrap_or_default();
                    cps.is_empty()
                        || as_i64(cps.last().unwrap().get(0)) != Some(frame)
                        || frame <= f0
                        || events.iter().any(|ev| {
                            let f = as_i64(ev.get("frame")).unwrap_or(i64::MIN);
                            !(f0 < f && f <= frame)
                        })
                }
                NewDoc::Shared(ne) => {
                    if ne.parent != e as i64 {
                        return Err(format!("ValueError: {fname}: parent {}, not the job's entry {e}", ne.parent));
                    }
                    ne.n == 0
                        || ne.shared.cps[ne.n - 1].frame as i64 != ne.frame
                        || ne.frame <= f0
                        || ne.events.iter().any(|ev| {
                            let f = as_i64(ev.get("frame")).unwrap_or(i64::MIN);
                            !(f0 < f && f <= ne.frame)
                        })
                }
            };
            if bad {
                return Err(format!("ValueError: {fname}: not a segment after frame {f0}"));
            }
            out.push((n, doc));
        }
        Ok(out)
    }

    /// A log.jsonl line that commits nothing (a guard's kill).
    pub fn log_event(&mut self, line: Value) -> Res<()> {
        files::append_jsonl(&self.root.join("log.jsonl"), std::slice::from_ref(&line))?;
        self.logs.push(line);
        Ok(())
    }

    /// cells.json (`json.dumps(cellmap, indent=None) + "\n"`), from each cell's text, made again for a changed cell only.
    pub fn write_cells(&self) -> Res<()> {
        let mut cache = self.cell_text.borrow_mut();
        let mut out = String::with_capacity(1 << 16);
        out.push('{');
        for (i, (id, c)) in self.cells.iter().enumerate() {
            let shape = ((c.key.is_some(), c.rep, c.entries.len(), c.n, c.exhausted.len(), c.tried.len(),
                          c.tried.iter().map(|(_, v)| v.as_array().map(|a| a.len()).unwrap_or(0)).sum::<usize>(),
                          c.chosen.unwrap_or(-1)), c.free);
            let fresh = match cache.get(id) {
                Some((sh, _)) => *sh != shape,
                None => true,
            };
            if fresh {
                cache.insert(id.clone(), (shape, crate::pyjson::dumps_default(&c.to_json())));
            }
            if i > 0 {
                out.push_str(", ");
            }
            crate::pyjson::write_str(&mut out, id);
            out.push_str(": ");
            out.push_str(&cache[id].1);
        }
        out.push_str("}\n");
        files::write_text_atomic(&self.root.join("cells.json"), &out)
    }
}

fn scope_of(v: &Value) -> Vec<i64> {
    v.as_array().map(|a| a.iter().filter_map(|x| as_i64(Some(x))).collect()).unwrap_or_default()
}

/// `Path.resolve()`, for paths that may not exist (the existing prefix resolved).
pub fn resolve(p: &Path) -> PathBuf {
    if let Ok(r) = fs::canonicalize(p) {
        return r;
    }
    let mut parts = Vec::new();
    let mut cur = p.to_path_buf();
    loop {
        if let Ok(r) = fs::canonicalize(&cur) {
            let mut out = r;
            for x in parts.iter().rev() {
                out.push(x);
            }
            return out;
        }
        match (cur.file_name().map(|s| s.to_os_string()), cur.parent().map(|x| x.to_path_buf())) {
            (Some(n), Some(par)) => {
                parts.push(n);
                cur = par;
            }
            _ => return p.to_path_buf(),
        }
    }
}

/// A reading's files under ``d``, except its start files and the dirs of other live readings; nothing when another
/// live reading shares ``d``.
pub fn evict_dir(d: &Path, keep: &[PathBuf]) {
    if keep.iter().any(|q| q == d) {
        return;
    }
    // os.walk(topdown=False): children first
    fn walk(p: &Path, keep: &[PathBuf]) {
        let pp = resolve(p);
        let mut subdirs = Vec::new();
        let mut files_ = Vec::new();
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    subdirs.push(e.path());
                } else {
                    files_.push(e);
                }
            }
        }
        for s in subdirs {
            walk(&s, keep);
        }
        if keep.iter().any(|q| *q == pp || pp.starts_with(q)) {
            return;
        }
        for f in files_ {
            let n = f.file_name().to_string_lossy().into_owned();
            if !START_FILES.contains(&n.as_str()) {
                let _ = fs::remove_file(f.path());
            }
        }
        let _ = fs::remove_dir(p);
    }
    walk(d, keep);
}

/// The archive's stage name, for messages.
pub fn pyjson_text(v: &Value) -> String {
    pyjson::compact(v)
}

/// E014's switch `FMI_STAG_SWEEP=1`: before any try has given a new atom of T or a new room, E012's stagnation sweep
/// is of the cells selectable at the last committed try, not of one cell (a game whose first readings test few atoms,
/// e.g. a dialog at the start, stopped after one try). Unset: one cell, as before.
fn stag_sweep() -> bool {
    std::env::var("FMI_STAG_SWEEP").is_ok_and(|v| v == "1")
}

/// E014 amendment (7)'s switch `FMI_STAG_CELLS=1`: stagnation fires only once every cell selectable now has had a
/// committed try since the last try that gave a new atom of T or a new room (a new pair, without the progress order),
/// besides the count. The count alone let the selection's order by frame leave late cells untried at the stop. Unset:
/// the count alone, as before.
pub fn stag_cells() -> bool {
    std::env::var("FMI_STAG_CELLS").is_ok_and(|v| v == "1")
}

/// E016 section F's switch `FMI_FRONTIER=1` (experiments/E016-realize-run/plan.md, "F. Configuration and selection"):
/// under the progress order, an entry in a room that no earlier entry held goes ahead of the selection's order for
/// the next free worker, once (`Archive::frontier`). Unset: the order as before.
/// E022 amendment (10): the frontier also holds the untried least live holders of atoms of T.
pub fn frontier_atoms_on() -> bool {
    std::env::var("FMI_FRONTIER_ATOMS").is_ok_and(|v| v == "1")
}

pub fn frontier_on() -> bool {
    std::env::var("FMI_FRONTIER").is_ok_and(|v| v == "1")
}

/// E019: a try whose job.json repeats a committed one's is not run (`Archive::same_job`).
pub fn skip_same_on() -> bool {
    std::env::var("FMI_SKIP_SAME").is_ok_and(|v| v == "1")
}

/// E020: an entry whose return differed from its checkpoint is dead (never selected again) and the run goes on
/// (`FMI_DIFFERS_DEAD=1`); unset, the driver stops at the first return that differs.
pub fn differs_dead_on() -> bool {
    std::env::var("FMI_DIFFERS_DEAD").is_ok_and(|v| v == "1")
}

/// E020 amendment (4): a return that differed on VAR_SOUNDRESULT (var 56) alone, which `FMI_SKIP_SOUNDRESULT=1` leaves
/// out of the comparison: its entry is not dead.
fn sound_result_only(ret: Option<&Value>) -> bool {
    let only = |d: &serde_json::Map<String, Value>| {
        d.len() == 2
            && d.get("facts").and_then(|f| f.as_i64()) == Some(1)
            && d.get("first").and_then(|f| f.as_array()).is_some_and(|a| a.len() == 1 && a[0][0] == "var:56")
    };
    std::env::var("FMI_SKIP_SOUNDRESULT").is_ok_and(|v| v == "1")
        && ret.and_then(|r| r.get("differ")).and_then(|d| d.as_object()).is_some_and(only)
}

/// E019: readings read the code of the objects actors hold (the reader's `FMI_READ_HELD`), so only the readings
/// registered with it are offered (`readings_for`, `readings_holding`) and a reading serves the inventory it was read
/// with (tryjob's reading choice).
pub fn read_held_on() -> bool {
    std::env::var("FMI_READ_HELD").is_ok_and(|v| v == "1")
}

/// E018: a try's targets leave out the atoms held at its entry's place, not in the whole archive (`job_doc`).
pub fn target_place_on() -> bool {
    std::env::var("FMI_TARGET_PLACE").is_ok_and(|v| v == "1")
}

/// E016 amendment (11): the selection orders places before cells (`Archive::place_key`).
pub fn select_place_on() -> bool {
    std::env::var("FMI_SELECT_PLACE").is_ok_and(|v| v == "1")
}
