//! explore.py's `Archive`, line for line (E008, with E009's and E010's switches): the archive's files and what follows
//! from them. The files are explore.py's, byte for byte (its JSON writers, `pyjson`); the archive's logic reads only
//! the files and the jobs' results, so it is deterministic given the results (plan.md, the explorer's exactness).
//!
//! What is kept in memory beyond explore.py: each entry's events, last checkpoint and last facts, read once from its
//! file (explore.py reads the file again on every use). The files are written by this process only, so the copy is
//! the file.

use crate::files::{self, as_i64, or, py_str, read_gz, read_json, read_jsonl, truthy, Res};
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
}

impl Cell {
    fn to_json(&self) -> Value {
        let mut tried = Map::new();
        for (k, v) in &self.tried {
            tried.insert(k.clone(), v.clone());
        }
        json!({"key": self.key.clone().unwrap_or(Value::Null), "rep": self.rep, "entries": self.entries,
               "n": self.n, "exhausted": self.exhausted, "tried": Value::Object(tried)})
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
        json!({"id": self.id, "parent": self.parent, "frame": self.frame, "tick": self.tick, "room": self.room,
               "cell": self.cell, "g": self.g, "held": pairs_json(&self.held), "r_path": self.r_path,
               "job": self.job, "how": self.how, "goal": self.goal, "seg_held": pairs_json(&self.seg_held)})
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
        })
    }
}

/// What the archive keeps of an entry's file.
#[derive(Debug)]
pub struct EntryInfo {
    pub events: Value,
    pub last_cp: Value,
    pub facts: FactMap,
}

/// A reading of readings.jsonl.
pub type ReadingRow = Map<String, Value>;

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
    pub e009: Map<String, Value>,
    pub e010: Map<String, Value>,
    pub idle_frames: i64,
    /// I: the keys measured idle at some entry, less the goal's (fix a)
    pub idle: BTreeSet<String>,
    pub idle_measured: BTreeSet<i64>,
    pub last_place: (bool, bool),
    entries: RefCell<HashMap<usize, Rc<EntryInfo>>>,
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
        if !clean {
            files::rewrite_jsonl(&r.join("log.jsonl"), &self.logs)?;
        }
        let committed_jobs: HashSet<String> = self.committed_jobs();
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
            if truthy(x.get("exhausted")) {
                let e = x["entry"].as_u64().unwrap_or(0) as usize;
                if !c.exhausted.contains(&e) {
                    c.exhausted.push(e);
                }
            }
        }
        for ci in 0..self.cells.len() {
            if self.cells[ci].1.key.is_none() {
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
        let facts = fact_map(&last_cp[1]);
        let info = Rc::new(EntryInfo { events: doc["events"].clone(), last_cp, facts });
        self.entries.borrow_mut().insert(i, info.clone());
        Ok(info)
    }

    pub fn facts(&self, i: usize) -> Res<FactMap> {
        Ok(self.info(i)?.facts.clone())
    }

    fn goal_keys(&self) -> HashSet<&str> {
        self.goal.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// explore.py `cell_key`.
    pub fn cell_key(&self, facts: &FactMap) -> Value {
        let ego = facts.get("ego");
        let mut owned: Vec<i64> = Vec::new();
        if let Some(&ego) = ego {
            for (k, &v) in facts {
                if let Some(o) = k.strip_prefix("owner:") {
                    if v == ego {
                        if let Ok(o) = o.parse::<i64>() {
                            owned.push(o);
                        }
                    }
                }
            }
        }
        owned.sort();
        let kv: Vec<Value> =
            self.cell_keys.iter().filter(|k| !self.idle.contains(*k)).map(|k| json!([k, get0(facts, k)])).collect();
        json!([facts.get("room"), owned, kv])
    }

    pub fn cell_id(key: &Value) -> String {
        sha1_smol::Sha1::from(canonical(key).as_bytes()).digest().to_string()
    }

    /// The R atoms ``facts`` holds, as (k, v) pairs (module docstring).
    pub fn held(&self, facts: &FactMap) -> Vec<(String, i64)> {
        let root = self.root_facts.as_ref().unwrap_or(facts);
        let mut out = Vec::new();
        for (k, a) in &self.atoms {
            if self.idle.contains(k) {
                continue;
            }
            let v = get0(facts, k);
            if a.values.contains(&v) || (a.any && v != get0(root, k)) {
                out.push((k.clone(), v));
            }
        }
        out.sort();
        out
    }

    /// #g: the goal keys whose value is not the goal's.
    pub fn g(&self, facts: &FactMap) -> i64 {
        self.goal.iter().filter(|(k, vs)| !vs.contains(&get0(facts, k))).count() as i64
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
        let cps: Vec<Value> = checkpoints.to_vec();
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
        let facts = fact_map(&cps.last().unwrap()[1]);
        if parent.is_none() {
            self.root_facts = Some(facts.clone());
        }
        let key = self.cell_key(&facts);
        let mut seg: BTreeSet<(String, i64)> = BTreeSet::new();
        for c in &cps {
            seg.extend(self.held(&fact_map(&c[1])));
        }
        let g = self.g(&facts);
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
            room: facts.get("room").copied(),
            cell: Self::cell_id(&key),
            g,
            held: self.held(&facts),
            r_path,
            job: job.map(|s| s.to_string()),
            how: how.to_string(),
            goal: g == 0,
            seg_held: seg.into_iter().collect(),
        };
        let doc = json!({"id": i, "parent": parent, "frame": frame, "tick": tick, "events": events, "checkpoints": cps});
        files::write_gz(&self.root.join("entries").join(format!("{i}.json.gz")), &doc)?;
        files::append_jsonl(&self.root.join("entries.jsonl"), &[row.to_json()])?;
        let info = Rc::new(EntryInfo { events: doc["events"].clone(), last_cp: cps.last().unwrap().clone(), facts });
        self.entries.borrow_mut().insert(i, info);
        self.last_place = self.place(row, Some(key));
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
            self.cells.push((c.clone(), Cell { key, rep: i, entries: vec![], n: 0, exhausted: vec![], tried: vec![] }));
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
        self.held_by.clear();
        self.chain_r.borrow_mut().clear();
        for row in rows {
            let e = self.entry(row.id)?;
            let cps = e["checkpoints"].as_array().cloned().unwrap_or_default();
            let facts = fact_map(&cps.last().ok_or("rekey: an entry with no checkpoint")?[1]);
            if row.parent.is_none() {
                self.root_facts = Some(facts.clone());
            }
            let key = self.cell_key(&facts);
            let mut seg: BTreeSet<(String, i64)> = BTreeSet::new();
            for c in &cps {
                seg.extend(self.held(&fact_map(&c[1])));
            }
            let mut new = row.clone();
            new.cell = Self::cell_id(&key);
            new.held = self.held(&facts);
            new.seg_held = seg.iter().cloned().collect();
            let cr = self.chain_r(row.parent);
            let mut u: HashSet<&(String, i64)> = cr.iter().collect();
            u.extend(seg.iter());
            new.r_path = u.len();
            self.place(new, Some(key));
        }
        self.recount();
        Ok(())
    }

    /// n, tried and exhausted of the current cells from the committed log lines, in order.
    fn recount(&mut self) {
        let logs = std::mem::take(&mut self.logs);
        for x in &logs {
            if !committed(x) {
                continue;
            }
            let Some(e) = x.get("entry").and_then(|e| e.as_u64()).map(|e| e as usize) else { continue };
            if e >= self.rows.len() {
                continue;
            }
            let cid = self.rows[e].cell.clone();
            let c = self.cell_mut(&cid);
            if x["outcome"].as_str() != Some("return differs") {
                c.n = if truthy(x.get("new_cells")) || truthy(x.get("new_reps")) { 0 } else { c.n + 1 };
            }
            if x.get("tried").map(|t| !t.is_null()).unwrap_or(false) {
                c.set_tried(e.to_string(), x["tried"].clone());
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
        let k = self.sel_key(i, &self.favoured());
        let t = if k.4 == i128::MAX { Value::from(f64::INFINITY) } else { Value::from(k.4 as i64) };
        json!([k.0, k.1, k.2, k.3, t, k.5])
    }

    pub fn selectable(&self, cell: &str) -> bool {
        match self.candidate(cell) {
            Some(i) => self.rows[i].room.map(|r| self.allowed.contains(&r)).unwrap_or(false),
            None => false,
        }
    }

    pub fn n_selectable(&self) -> usize {
        self.cells.iter().filter(|(c, _)| self.selectable(c)).count()
    }

    /// The first ``b`` candidates in the order, each of another cell.
    pub fn select(&self, b: usize) -> Vec<usize> {
        let busy_ids: HashSet<usize> = self.busy.values().map(|(e, _)| *e).collect();
        let mut busy_cells: HashSet<&str> = self.busy.values().map(|(_, c)| c.as_str()).collect();
        for e in &busy_ids {
            busy_cells.insert(self.rows[*e].cell.as_str());
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

    pub fn goal_entries(&self) -> Vec<usize> {
        self.rows.iter().filter(|r| r.goal).map(|r| r.id).collect()
    }

    /// After the last committed job that added a pair no earlier entry held (outside I), as many committed jobs as
    /// there were selectable cells right after it, none adding one.
    pub fn stagnant(&self) -> bool {
        let done: Vec<&Value> = self.logs.iter().filter(|x| committed(x)).collect();
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
        since >= frontier
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
        let row = json!({"scope": s, "path": path, "start_entry": start_entry, "job": job, "bytes": bytes,
                         "last_used": used, "kind": kind});
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

    /// job.json (plan.md "Interfaces").
    pub fn job_doc(&self, job: &str, i: usize) -> Res<Value> {
        let row = &self.rows[i];
        let cell = self.cell(&row.cell);
        let scope = ring(&self.edges, row.room.unwrap_or(i64::MIN), &self.allowed);
        let rooms = scope.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("-");
        let mut held: Vec<&(String, i64)> = self.held_by.keys().collect();
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
                      "checkpoint": info.last_cp},
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

    /// jobs/JOB/job.json for candidate ``i``, and select.json.
    pub fn prepare_job(&self, i: usize) -> Res<PathBuf> {
        let job = self.next_job();
        let d = self.root.join("jobs").join(&job);
        fs::create_dir_all(&d).map_err(|e| files::io_err(&d, e))?;
        let sel = json!({"key": self.sel_key_json(i), "cell": self.rows[i].cell, "selectable": self.n_selectable(),
                         "time": files::now_s()});
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
        let mut new_files: Vec<(Value, Value)> = Vec::new();
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
        let before: HashSet<(String, i64)> = self.held_by.keys().cloned().collect();
        let (mut ids, mut new_cells, mut new_reps) = (Vec::new(), 0i64, 0i64);
        for (n, doc) in &new_files {
            let how = n.get("how").and_then(|h| h.as_str()).filter(|h| !h.is_empty()).unwrap_or("checkpoint").to_string();
            let id = self.add(
                Some(e),
                as_i64(doc.get("frame")).unwrap_or(0),
                as_i64(doc.get("tick")),
                doc["events"].as_array().map(|a| a.as_slice()).unwrap_or(&[]),
                doc["checkpoints"].as_array().map(|a| a.as_slice()).unwrap_or(&[]),
                Some(&name),
                &how,
            )?;
            ids.push(id);
            new_cells += self.last_place.0 as i64;
            new_reps += self.last_place.1 as i64;
        }
        let mut new_r: Vec<(String, i64)> = self.held_by.keys().filter(|p| !before.contains(*p)).cloned().collect();
        new_r.sort();
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
            let c = self.cell_mut(&cell);
            c.n = if new_cells != 0 || new_reps != 0 { 0 } else { c.n + 1 };
            c.set_tried(e.to_string(), Value::Array(tv.into_iter().map(|(_, v)| v).collect()));
            if exhausted && !c.exhausted.contains(&e) {
                c.exhausted.push(e);
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
        self.write_cells()?;
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
        if let Some(x) = extra {
            for (k, v) in x {
                line.insert(k.clone(), v.clone());
            }
        }
        let line = Value::Object(line);
        files::append_jsonl(&self.root.join("log.jsonl"), std::slice::from_ref(&line))?;
        self.logs.push(line.clone());
        Ok(line)
    }

    /// ingest's checks of the result and its new entries (explore.py's try block): the entry files, or the error's
    /// text as Python gives it (`TypeName: message`).
    fn check_new(&self, res: &Value, name: &str, e: usize, job_dir: &Path) -> Result<Vec<(Value, Value)>, String> {
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
                Ok(_) => read_gz(&p).map_err(|m| format!("JSONDecodeError: {m}"))?,
            };
            if doc.get("parent").and_then(|x| x.as_u64()) != Some(e as u64) {
                return Err(format!("ValueError: {fname}: parent {}, not the job's entry {e}", py_str(doc.get("parent"))));
            }
            let f0 = self.rows[e].frame;
            let frame = as_i64(doc.get("frame")).ok_or("KeyError: 'frame'")?;
            let cps = doc.get("checkpoints").ok_or("KeyError: 'checkpoints'")?.as_array().cloned().unwrap_or_default();
            let events = doc.get("events").ok_or("KeyError: 'events'")?.as_array().cloned().unwrap_or_default();
            let bad = cps.is_empty()
                || as_i64(cps.last().unwrap().get(0)) != Some(frame)
                || frame <= f0
                || events.iter().any(|ev| {
                    let f = as_i64(ev.get("frame")).unwrap_or(i64::MIN);
                    !(f0 < f && f <= frame)
                });
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

    pub fn write_cells(&self) -> Res<()> {
        files::write_json(&self.root.join("cells.json"), &self.cells_json(), None)
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
