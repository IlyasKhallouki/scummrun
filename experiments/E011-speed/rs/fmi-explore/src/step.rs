//! step.py's parts a test can call alone, line for line: the targets from a reading's reached values and the guide
//! (`targets`, E009's `rotate`), the reuse rule (`reusable`, `reusable_detail`), E009's idle rule, the entries a
//! played attempt gives (`entries_from`), the start row, the result's records, and the cached readings' order.

use crate::archive::{fact_map, FactMap, NOT_INT};
use crate::files::{self, as_i64, Res};
use crate::pyjson::{self, canonical};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const SEED: i64 = 1;
/// the start maps them to pos:/escpos:
pub const SLOT_PREFIXES: [&str; 5] = ["at:", "script:", "local:", "objscript:", "esc:"];
/// and "room" (GUIDE.json's atoms)
pub const GUIDE_KINDS: [&str; 5] = ["var:", "bit:", "owner:", "state:", "class:"];
/// read.sh's, with wait_pass_cost
pub const SWITCHES: &str = "value_sets,draw_clears,actor_rooms,input_settled,wait_pass_cost";
/// walk's run.sh (E006's limits)
pub const REPLAN_LIMIT: u32 = 30;
pub const TIME_LIMIT: f64 = 86400.0;
pub const PLAN_TIME: u64 = 600;
pub const PLAN_MEMORY: &str = "3G";
pub const FORK_EACH: u32 = 7;
/// the first plan's status on which a cached reading is read fresh
pub const NOT_A_PLAN_RETRY: &str = "out of domain";
pub const REUSE_KEYS_FILE: &str = "reuse-keys.json";
pub const MADE_KINDS: [&str; 3] = ["reach", "refine", "reread"];
pub const KEEP_FILES: [&str; 3] = ["reading.json", "start.txt", "start.jsonl"];

pub fn guide_kind(k: &str) -> bool {
    k == "room" || GUIDE_KINDS.iter().any(|p| k.starts_with(p))
}

/// GUIDE.json's atoms as {(key, value or None for any): depth}.
pub fn guide_atoms(guide: &Value) -> HashMap<(String, Option<i64>), i64> {
    let mut out: HashMap<(String, Option<i64>), i64> = HashMap::new();
    for a in guide["atoms"].as_array().cloned().unwrap_or_default() {
        let key = (a[0].as_str().unwrap_or("").to_string(), as_i64(a.get(1)));
        let d = as_i64(a.get(2)).unwrap_or(0);
        let e = out.entry(key).or_insert(d);
        *e = (*e).min(d);
    }
    out
}

/// plan.md's rule: the least of the depths of [k, v] and [k, null]; None when neither is in R.
pub fn depth_of(atoms: &HashMap<(String, Option<i64>), i64>, k: &str, v: i64) -> Option<i64> {
    let a = atoms.get(&(k.to_string(), Some(v)));
    let b = atoms.get(&(k.to_string(), None));
    match (a, b) {
        (Some(x), Some(y)) => Some(*x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(*x),
        _ => None,
    }
}

/// reading.reached (key<TAB>v1<TAB>v2...) as {key: sorted int values}; non-int values left out, a key on several
/// lines gets the union. Keys in file order (Python's dict).
pub fn read_reached_text(text: &str) -> Vec<(String, Vec<i64>)> {
    let mut order: Vec<String> = Vec::new();
    let mut out: HashMap<String, BTreeSet<i64>> = HashMap::new();
    for ln in files::py_splitlines(text) {
        let parts: Vec<&str> = ln.trim_end_matches('\n').split('\t').collect();
        if parts.is_empty() || parts[0].is_empty() {
            continue;
        }
        if !out.contains_key(parts[0]) {
            order.push(parts[0].to_string());
        }
        let vals = out.entry(parts[0].to_string()).or_default();
        for x in &parts[1..] {
            if let Some(n) = py_int(x) {
                vals.insert(n);
            }
        }
    }
    order.into_iter().map(|k| { let v = out.remove(&k).unwrap(); (k, v.into_iter().collect()) }).collect()
}

/// Python's `int(x)` on a text (surrounding whitespace, a sign, underscores between digits).
pub fn py_int(x: &str) -> Option<i64> {
    let t = x.trim();
    let (neg, d) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    if d.is_empty() || d.starts_with('_') || d.ends_with('_') || d.contains("__") || !d.bytes().all(|c| c.is_ascii_digit() || c == b'_') {
        return None;
    }
    let n: i64 = d.replace('_', "").parse().ok()?;
    Some(if neg { -n } else { n })
}

pub fn read_reached(path: &Path) -> Res<Vec<(String, Vec<i64>)>> {
    Ok(read_reached_text(&files::read_text(path)?))
}

/// replay_part1.goal_text_of: {fact: [value]} as reader-scope's --goal (one value per fact, sorted by key).
pub fn goal_text(goal: &[(String, Vec<i64>)]) -> Res<String> {
    let mut g: Vec<&(String, Vec<i64>)> = goal.iter().collect();
    g.sort_by(|a, b| a.0.cmp(&b.0));
    let mut parts = Vec::new();
    for (k, vs) in g {
        let s: BTreeSet<i64> = vs.iter().copied().collect();
        if s.len() != 1 {
            return Err(format!("reader-scope's --goal takes one value per fact; {k} has {vs:?}"));
        }
        parts.push(format!("{k}={}", s.iter().next().unwrap()));
    }
    Ok(parts.join(","))
}

/// A target's value: an int, or tier 0's "K=V,K=V" for a goal of several keys.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TV {
    Int(i64),
    Text(String),
}

impl TV {
    pub fn json(&self) -> Value {
        match self {
            TV::Int(n) => json!(n),
            TV::Text(s) => json!(s),
        }
    }
    pub fn of(v: &Value) -> Option<TV> {
        match v {
            Value::String(s) => Some(TV::Text(s.clone())),
            x => as_i64(Some(x)).map(TV::Int),
        }
    }
    fn text(&self) -> String {
        match self {
            TV::Int(n) => n.to_string(),
            TV::Text(s) => s.clone(),
        }
    }
}

/// (k, v, tier, depth)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub k: String,
    pub v: TV,
    pub tier: i64,
    pub depth: i64,
}

impl Target {
    pub fn json(&self) -> Value {
        json!([self.k, self.v.json(), self.tier, self.depth])
    }
    pub fn text(&self) -> String {
        format!("{}={}", self.k, self.v.text())
    }
}

/// Tier 0's target: the goal's one atom, or ("goal", "K=V,K=V") for a goal of several keys.
pub fn goal_target(goal: &[(String, Vec<i64>)]) -> Res<(String, TV)> {
    if goal.len() == 1 {
        let (k, vs) = &goal[0];
        return Ok((k.clone(), TV::Int(*vs.first().ok_or("an empty goal value")?)));
    }
    Ok(("goal".into(), TV::Text(goal_text(goal)?)))
}

fn pair_key(k: &str, v: &Value) -> String {
    canonical(&json!([k, v]))
}

/// Step 3: [(k, v, tier, depth)], tier 0 first, then by (depth, key, value) within a tier (step.py `targets`).
#[allow(clippy::too_many_arguments)]
pub fn targets(reached: &[(String, Vec<i64>)], guide: &Value, entry_facts: &FactMap, root_facts: Option<&FactMap>,
               held: &[Value], tried: &[Value], goal: &[(String, Vec<i64>)], rotation: Option<&Value>) -> Res<Vec<Target>> {
    let atoms = guide_atoms(guide);
    let pair = |p: &Value| pair_key(p.get(0).and_then(|k| k.as_str()).unwrap_or(""), p.get(1).unwrap_or(&Value::Null));
    let held_set: HashSet<String> = held.iter().map(pair).collect();
    let tried_set: HashSet<String> = tried.iter().map(pair).collect();
    let reached_map: HashMap<&str, &Vec<i64>> = reached.iter().map(|(k, v)| (k.as_str(), v)).collect();
    let mut out = Vec::new();
    let mut first: Option<(String, TV)> = None;
    if !goal.is_empty() {
        let all_reached = goal.iter().all(|(k, vs)| vs.iter().any(|v| reached_map.get(k.as_str()).map(|r| r.contains(v)).unwrap_or(false)));
        let holds = goal.iter().all(|(k, vs)| vs.contains(&entry_facts.get(k).copied().unwrap_or(0)));
        if all_reached && !holds {
            let f = goal_target(goal)?;
            if !tried_set.contains(&pair_key(&f.0, &f.1.json())) {
                out.push(Target { k: f.0.clone(), v: f.1.clone(), tier: 0, depth: 0 });
            }
            first = Some(f);
        }
    }
    let mut rest = Vec::new();
    for (k, vals) in reached {
        if !guide_kind(k) {
            continue;
        }
        let now = entry_facts.get(k).copied().unwrap_or(0);
        for &v in vals {
            if v == now || first.as_ref().map(|f| f.0 == *k && f.1 == TV::Int(v)).unwrap_or(false)
                || tried_set.contains(&pair_key(k, &json!(v))) {
                continue;
            }
            let Some(d) = depth_of(&atoms, k, v) else { continue };
            let concrete = atoms.contains_key(&(k.clone(), Some(v)));
            if !concrete {
                if let Some(r) = root_facts {
                    if v == r.get(k).copied().unwrap_or(0) {
                        continue;
                    }
                }
            }
            let is_held = held_set.contains(&pair_key(k, &json!(v))) || (!concrete && held_set.contains(&pair_key(k, &Value::Null)));
            rest.push(Target { k: k.clone(), v: TV::Int(v), tier: if is_held { 2 } else { 1 }, depth: d });
        }
    }
    rest.sort_by(|a, b| (a.tier, a.depth, &a.k, &a.v).cmp(&(b.tier, b.depth, &b.k, &b.v)));
    out.extend(rest);
    match rotation {
        None => Ok(out),
        Some(r) => Ok(rotate(out, r)),
    }
}

/// E009 fix b: ``ts`` ordered by (tried in the cell, tried in all, depth, then E008's order (tier, depth, key, value)).
pub fn rotate(ts: Vec<Target>, rotation: &Value) -> Vec<Target> {
    let counts = |name: &str| -> HashMap<String, i64> {
        let mut m = HashMap::new();
        for r in rotation.get(name).and_then(|x| x.as_array()).cloned().unwrap_or_default() {
            m.insert(pair_key(r[0].as_str().unwrap_or(""), &r[1]), as_i64(r.get(2)).unwrap_or(0));
        }
        m
    };
    let (cell, every) = (counts("cell"), counts("all"));
    let mut ts = ts;
    ts.sort_by_key(|t| {
        let a = pair_key(&t.k, &t.v.json());
        let vk = match &t.v {
            TV::Int(n) => (0, *n, String::new()),
            TV::Text(s) => (1, 0, s.clone()),
        };
        (cell.get(&a).copied().unwrap_or(0), every.get(&a).copied().unwrap_or(0), t.depth, t.tier, t.k.clone(), vk)
    });
    ts
}

/// facts.py `diff`: {key: (old, new)}, absent 0.
pub fn diff(a: &FactMap, b: &FactMap) -> BTreeMap<String, (i64, i64)> {
    let mut out = BTreeMap::new();
    for (k, &x) in a {
        let y = b.get(k).copied().unwrap_or(0);
        if x != y {
            out.insert(k.clone(), (x, y));
        }
    }
    for (k, &y) in b {
        if !a.contains_key(k) && y != 0 {
            out.insert(k.clone(), (0, y));
        }
    }
    out
}

fn is_slot(k: &str) -> bool {
    SLOT_PREFIXES.iter().any(|p| k.starts_with(p))
}

/// Step 2's rule: (whether the entry's facts differ from the reading's start facts only on the reading's task facts
/// and on slot keys, the number of keys they differ on besides those).
pub fn reusable(start: &FactMap, task_keys: &HashSet<String>, entry: &FactMap) -> (bool, usize) {
    let d = diff(start, entry);
    let bad = d.keys().filter(|k| !task_keys.contains(*k) && !is_slot(k)).count();
    (bad == 0, bad)
}

pub fn reusable_detail(start: &FactMap, task_keys: &HashSet<String>, entry: &FactMap) -> Value {
    let d = diff(start, entry);
    let bad: Vec<&String> = d.keys().filter(|k| !task_keys.contains(*k) && !is_slot(k)).collect();
    json!({"differ": d.len(), "task": d.keys().filter(|k| task_keys.contains(*k)).count(),
           "slots": d.keys().filter(|k| !task_keys.contains(*k) && is_slot(k)).count(), "other": bad.len(),
           "first": bad.iter().take(12).map(|k| json!([k, d[*k].0, d[*k].1])).collect::<Vec<_>>()})
}

/// E009 fix a: a key is idle when its value comes back to a value it had before (the entry's value included).
/// {"returns": sorted idle keys, "first_return": {k: offset}, "changes": {k: n}} (dict orders as Python builds them).
pub fn idle_rule<'a, I>(start: &FactMap, frames: I) -> Value
where
    I: IntoIterator<Item = (i64, FactMap)>,
{
    let mut seen: HashMap<String, HashSet<i64>> = HashMap::new();
    let mut first: Vec<(String, i64)> = Vec::new();
    let mut first_set: HashSet<String> = HashSet::new();
    let mut changes: Vec<(String, i64)> = Vec::new();
    let mut ch_ix: HashMap<String, usize> = HashMap::new();
    let mut prev = start.clone();
    for (off, fa) in frames {
        // Python iterates set(fa) | set(prev): an order of its own; the counts and the first returns do not depend on
        // it within one frame, but the dicts' insertion order does: keys in sorted order here, and the
        // reference's outputs are compared as maps
        let mut keys: Vec<&String> = fa.keys().chain(prev.keys().filter(|k| !fa.contains_key(*k))).collect();
        keys.sort();
        for k in keys {
            let v = fa.get(k).copied().unwrap_or(0);
            if v == prev.get(k).copied().unwrap_or(0) {
                continue;
            }
            match ch_ix.get(k) {
                Some(&i) => changes[i].1 += 1,
                None => {
                    ch_ix.insert(k.clone(), changes.len());
                    changes.push((k.clone(), 1));
                }
            }
            let s = seen.entry(k.clone()).or_insert_with(|| HashSet::from([start.get(k).copied().unwrap_or(0)]));
            if s.contains(&v) && !first_set.contains(k) {
                first_set.insert(k.clone());
                first.push((k.clone(), off));
            }
            s.insert(v);
        }
        prev = fa;
    }
    let mut returns: Vec<&String> = first_set.iter().collect();
    returns.sort();
    let mut fr = Map::new();
    for (k, o) in &first {
        fr.insert(k.clone(), json!(o));
    }
    let mut chm = Map::new();
    for (k, n) in &changes {
        chm.insert(k.clone(), json!(n));
    }
    json!({"returns": returns, "first_return": Value::Object(fr), "changes": Value::Object(chm)})
}

/// Step 5: one entry per checkpoint with frame > ``entry_frame`` (frame order, one per frame). Every input must come
/// after ``entry_frame`` (premise S of Lemma C).
pub fn entries_from(entry_frame: i64, inputs: &[Value], checkpoints: &[Value]) -> Res<Vec<Value>> {
    let early: Vec<&Value> = inputs.iter().filter(|e| as_i64(e.get("frame")).unwrap_or(i64::MIN) <= entry_frame).collect();
    if let Some(e0) = early.first() {
        return Err(format!("ValueError: {} inputs at or before the entry's frame {entry_frame}: first {}", early.len(), files::py_repr(e0)));
    }
    let mut sorted: Vec<&Value> = checkpoints.iter().collect();
    sorted.sort_by_key(|c| as_i64(c.get(0)).unwrap_or(i64::MIN)); // stable, as Python's sorted
    let mut seen = HashSet::new();
    let mut cps: Vec<Value> = Vec::new();
    for c in sorted {
        let f = as_i64(c.get(0)).unwrap_or(i64::MIN);
        if f > entry_frame && seen.insert(f) {
            let extra = c.get(2).cloned().unwrap_or(json!({}));
            cps.push(json!([c[0], c[1], extra]));
        }
    }
    let mut out = Vec::new();
    for (i, c) in cps.iter().enumerate() {
        let f = as_i64(c.get(0)).unwrap();
        let tick = c[2].get("tick").cloned().unwrap_or(Value::Null);
        let events: Vec<Value> = inputs.iter().filter(|e| as_i64(e.get("frame")).unwrap_or(i64::MAX) <= f).cloned().collect();
        out.push(json!({"id": null, "parent": null, "frame": c[0], "tick": tick, "events": events,
                        "checkpoints": cps[..=i].to_vec(), "how": if i == cps.len() - 1 { "end" } else { "checkpoint" }}));
    }
    Ok(out)
}

pub fn first_plan_status(plans: &[Value]) -> Option<String> {
    plans.iter().find(|p| p.get("status").and_then(|s| s.as_str()) != Some("reach extension"))
        .map(|p| p.get("status").and_then(|s| s.as_str()).map(|s| s.to_string()).unwrap_or_else(|| "None".into()))
}

pub fn empty_result(job: &Value) -> Value {
    json!({"job": job.get("job"), "entry": job.get("entry").and_then(|e| e.get("id")), "ok": false, "error": null,
           "return": null, "reading": null, "targets": {"by_tier": {"0": 0, "1": 0, "2": 0}, "first": []},
           "probes": [], "attempt": null, "readings_made": [], "new": [], "tried": [], "exhausted": false,
           "cpu_s": {"return": 0.0, "read": 0.0, "plan": 0.0, "play": 0.0, "total": 0.0},
           "peak_rss_mb": {"python": 0, "children": 0}, "wall_s": 0.0})
}

/// The CLI's exit: 0 ok, 3 the return differs, 1 any other error.
pub fn exit_code(result: &Value) -> i32 {
    if files::truthy(result.get("ok")) {
        return 0;
    }
    if result.get("return").and_then(|r| r.get("same")) == Some(&Value::Bool(false)) {
        return 3;
    }
    1
}

/// The start-rows file a reading names as its start (``PATH seed N``), or None for start-part1.jsonl's starts.
pub fn start_rows_of(doc: &Value) -> Option<PathBuf> {
    let name = doc.get("start").and_then(|s| s.get("name")).and_then(|n| n.as_str()).unwrap_or("");
    let (head, _) = name.rsplit_once(" seed ")?;
    if head.starts_with('/') && head.ends_with(".jsonl") {
        Some(PathBuf::from(head))
    } else {
        None
    }
}

/// The job's readings of its scope whose files exist, newest first (reading.json's mtime, then the job's order).
pub fn cached_readings(job: &Value) -> Vec<PathBuf> {
    let mut scope: Vec<i64> = job["scope"].as_array().map(|a| a.iter().filter_map(|x| as_i64(Some(x))).collect()).unwrap_or_default();
    scope.sort();
    let mut out: Vec<(i128, usize, PathBuf)> = Vec::new();
    for (i, r) in job.get("readings").and_then(|r| r.as_array()).cloned().unwrap_or_default().iter().enumerate() {
        let p = PathBuf::from(r["path"].as_str().unwrap_or(""));
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let Ok(doc) = pyjson::loads(&text) else { continue };
        let mut rooms: Vec<i64> = doc.get("scope").and_then(|s| s.get("rooms")).and_then(|r| r.as_array())
            .map(|a| a.iter().filter_map(|x| as_i64(Some(x))).collect()).unwrap_or_default();
        rooms.sort();
        if rooms != scope {
            continue;
        }
        let packed = p.parent().unwrap_or(Path::new("")).join(doc.get("packed").and_then(|x| x.as_str()).unwrap_or("reading.tc"));
        if !packed.exists() || !packed.with_extension("reached").exists() {
            continue;
        }
        if let Some(rows) = start_rows_of(&doc) {
            if !rows.exists() {
                continue;
            }
        }
        let mt = std::fs::metadata(&p).and_then(|m| m.modified()).ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos() as i128).unwrap_or(0);
        out.push((-mt, i, p));
    }
    out.sort();
    out.into_iter().map(|x| x.2).collect()
}

/// The root's facts, for the hold rule of (k, any) atoms: the job's ``root_facts``, the root entry itself, or the
/// archive's root entry file; else None.
pub fn root_facts_of(job: &Value, job_dir: &Path) -> Option<FactMap> {
    if let Some(r) = job.get("root_facts").filter(|r| !r.is_null()) {
        return Some(fact_map(r));
    }
    if files::truthy(job["entry"].get("root")) {
        return Some(fact_map(&job["entry"]["checkpoint"][1]));
    }
    let jd = std::fs::canonicalize(job_dir).ok()?;
    let arch = jd.parent()?.parent()?;
    let text = std::fs::read_to_string(arch.join("entries.jsonl")).ok()?;
    for ln in files::py_splitlines(&text) {
        let e = pyjson::loads(ln).ok()?;
        if e.get("parent").map(|p| p.is_null()).unwrap_or(false) {
            let id = e.get("id")?;
            let doc = files::read_gz(&arch.join("entries").join(format!("{}.json.gz", files::py_str(Some(id))))).ok()?;
            return doc["checkpoints"].as_array()?.last().map(|c| fact_map(&c[1]));
        }
    }
    None
}

/// The harness's last reading when it is not the one it started on: its reading.json, its scope and its kind.
pub fn made_reading(h_packed: &Path, rd_packed: &Path) -> Option<Value> {
    if h_packed == rd_packed {
        return None;
    }
    let head = h_packed.parent().unwrap_or(Path::new("")).join("reading.json");
    let scope = std::fs::read_to_string(&head).ok().and_then(|t| pyjson::loads(&t).ok())
        .and_then(|d| d.get("scope").and_then(|s| s.get("rooms")).cloned()).unwrap_or(Value::Null);
    let name = h_packed.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let kind = MADE_KINDS.iter().find(|k| name.starts_with(&format!("{k}-"))).copied().unwrap_or("other");
    Some(json!({"path": head.to_string_lossy(), "scope": scope, "kind": kind}))
}

/// The readings the harness made in the attempt's folder and did not end on: their files go, but for reading.json
/// and the start files. Returns [[folder, bytes freed], ...].
pub fn drop_unused(adir: &Path, made: Option<&Value>) -> Vec<Value> {
    let keep = made.and_then(|m| m["path"].as_str()).map(|p| crate::archive::resolve(Path::new(p).parent().unwrap_or(Path::new(""))));
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(adir).map(|rd| rd.flatten().filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false)).map(|e| e.path()).collect()).unwrap_or_default();
    dirs.sort();
    let mut out = Vec::new();
    for d in dirs {
        let name = d.file_name().unwrap().to_string_lossy().into_owned();
        let kind = name.split('-').next().unwrap_or("");
        if !MADE_KINDS.contains(&kind) || keep.as_ref().map(|k| crate::archive::resolve(&d) == *k).unwrap_or(false) {
            continue;
        }
        let mut freed = 0u64;
        let mut todo = vec![d.clone()];
        while let Some(p) = todo.pop() {
            for e in std::fs::read_dir(&p).map(|rd| rd.flatten().collect::<Vec<_>>()).unwrap_or_default() {
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    todo.push(e.path());
                } else if !KEEP_FILES.contains(&e.file_name().to_string_lossy().as_ref()) {
                    if let Ok(m) = e.metadata() {
                        if std::fs::remove_file(e.path()).is_ok() {
                            freed += m.len();
                        }
                    }
                }
            }
        }
        out.push(json!([name, freed]));
    }
    out
}

fn pick(x: &Value, keys: &[&str]) -> Value {
    let mut m = Map::new();
    for k in keys {
        m.insert((*k).into(), x.get(*k).cloned().unwrap_or(Value::Null));
    }
    Value::Object(m)
}

pub fn attempt_record(t: &Target, res: &Value, seconds: f64) -> Value {
    // `_checkpoints`: the checkpoints, or their count (a try of fmi keeps them as packed facts)
    let cps = res.get("_checkpoints").map(|c| c.as_array().map(|a| a.len()).or(c.as_u64().map(|n| n as usize)).unwrap_or(0)).unwrap_or(0);
    let list = |k: &str| res.get(k).and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let mut out = json!({"target": [t.k, t.v.json()], "tier": t.tier, "depth": t.depth, "stop": res.get("stop"),
        "reached": res.get("_reached"), "goal_from_the_plan": res.get("goal_from_the_plan"), "frames": res.get("frames"),
        "frame0": res.get("frame0"), "frame_end": res.get("frame_end"), "qf0": res.get("qf0"), "qf_end": res.get("qf_end"),
        "qf": res.get("qf"), "replans": res.get("replans"),
        "refinements": list("refinements").iter().map(|x| pick(x, &["plan", "frame", "class", "added", "why_not", "seconds", "cpu_s"])).collect::<Vec<_>>(),
        "extensions": list("extensions").iter().map(|x| pick(x, &["plan", "frame", "n_values", "seconds", "cpu_s", "actions_before", "actions"])).collect::<Vec<_>>(),
        "window_summary": res.get("window_summary"), "seconds": pyjson::round(seconds, 1),
        "inputs": list("inputs").len(), "checkpoints": cps, "error": res.get("error")});
    if files::truthy(res.get("_ready")) {
        out["ready"] = json!(true);
        out["earliest"] = json!(false);
    }
    out
}

/// An attempt's CPU by part: ``read`` its reach extensions' and refinements' readers, ``play`` the window search's
/// engines and its own, ``plan`` the rest.
pub fn attempt_costs(res: &Value, att_cpu: f64) -> (f64, f64, f64) {
    let empty = json!({});
    let ws = files::or(res.get("window_summary"), &empty);
    let py = ws.get("python_cpu_s").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let list = |k: &str| res.get(k).and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let reads: f64 = list("extensions").iter().chain(list("refinements").iter()).map(|x| x.get("cpu_s").and_then(|c| c.as_f64()).unwrap_or(0.0)).sum();
    let eng: f64 = ws.get("engine_cpu_s").and_then(|e| e.as_object()).map(|m| m.values().filter_map(|v| v.as_f64()).sum()).unwrap_or(0.0);
    (reads, eng + py, (att_cpu - py - reads).max(0.0))
}

pub fn target_holds(t: &Target, goal: &[(String, Vec<i64>)], facts: &FactMap) -> bool {
    if t.tier == 0 {
        return goal.iter().all(|(k, vs)| vs.contains(&facts.get(k).copied().unwrap_or(0)));
    }
    match t.v {
        TV::Int(v) => facts.get(&t.k).copied().unwrap_or(0) == v,
        TV::Text(_) => false,
    }
}

/// The goal of job.json ({key: [values]}) as pairs.
pub fn goal_of(job: &Value) -> Vec<(String, Vec<i64>)> {
    job["goal"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_array().map(|a| a.iter().filter_map(|x| as_i64(Some(x))).collect()).unwrap_or_default())).collect()).unwrap_or_default()
}

/// A facts.py map as a JSON object, keys in the map's own order (callers sort where the reference does).
pub fn facts_json(f: &FactMap) -> Value {
    let mut keys: Vec<&String> = f.keys().collect();
    keys.sort();
    let mut m = Map::new();
    for k in keys {
        let v = f[k];
        m.insert(k.clone(), if v == NOT_INT { Value::Null } else { json!(v) });
    }
    Value::Object(m)
}

/// E009 fix a's rule on fmi-core's packed facts (the try's hot path: one diff per frame, no key text built but for
/// the keys that change). The same result as `idle_rule` (its maps in key order).
pub fn idle_rule_facts<I>(start: &fmi_core::Facts, frames: I) -> Value
where
    I: IntoIterator<Item = (i64, fmi_core::Facts)>,
{
    let mut seen: HashMap<fmi_core::Key, HashSet<i64>> = HashMap::new();
    let mut first: Vec<(fmi_core::Key, i64)> = Vec::new();
    let mut first_set: HashSet<fmi_core::Key> = HashSet::new();
    let mut changes: Vec<(fmi_core::Key, i64)> = Vec::new();
    let mut ch_ix: HashMap<fmi_core::Key, usize> = HashMap::new();
    let mut prev = start.clone();
    for (off, fa) in frames {
        for (k, _old, v) in prev.diff(&fa) {
            match ch_ix.get(&k) {
                Some(&i) => changes[i].1 += 1,
                None => {
                    ch_ix.insert(k, changes.len());
                    changes.push((k, 1));
                }
            }
            let s = seen.entry(k).or_insert_with(|| HashSet::from([start.get(k).unwrap_or(0)]));
            if s.contains(&v) && first_set.insert(k) {
                first.push((k, off));
            }
            s.insert(v);
        }
        prev = fa;
    }
    let mut returns: Vec<String> = first_set.iter().map(|k| k.text()).collect();
    returns.sort();
    let mut fr = Map::new();
    for (k, o) in &first {
        fr.insert(k.text(), json!(o));
    }
    let mut chm = Map::new();
    for (k, n) in &changes {
        chm.insert(k.text(), json!(n));
    }
    json!({"returns": returns, "first_return": Value::Object(fr), "changes": Value::Object(chm)})
}

/// facts.py `facts_of` as an ordered JSON object, keys in the order facts.py inserts them (the start row's file).
pub fn facts_of_ordered(s: &fmi_core::Obs) -> Value {
    use fmi_core::{CLOCKS, ENCD, EXCD, WIO_FLOBJECT, WIO_GLOBAL, WIO_INVENTORY, WIO_LOCAL, WIO_ROOM};
    let mut f = Map::new();
    for (n, &v) in s.vars.iter().enumerate() {
        if !CLOCKS.contains(&n) {
            f.insert(format!("var:{n}"), json!(v));
        }
    }
    for n in &s.bits_set {
        f.insert(format!("bit:{n}"), json!(1));
    }
    for (o, v) in &s.owners {
        f.insert(format!("owner:{o}"), json!(v));
    }
    for (o, v) in &s.states {
        f.insert(format!("state:{o}"), json!(v));
    }
    for (o, cs) in &s.classes {
        for c in cs {
            f.insert(format!("class:{o}:{c}"), json!(1));
        }
    }
    f.insert("room".into(), json!(s.room));
    if let Some(e) = s.ego {
        f.insert("ego".into(), json!(e));
    }
    for sl in &s.slots {
        if sl.where_ == WIO_GLOBAL {
            f.insert(format!("script:{}", sl.number), json!(1));
        } else if sl.where_ == WIO_LOCAL {
            f.insert(format!("local:{}", sl.number), json!(1));
        } else if [WIO_INVENTORY, WIO_ROOM, WIO_FLOBJECT].contains(&sl.where_) && sl.number != ENCD && sl.number != EXCD {
            f.insert(format!("objscript:{}", sl.number), json!(1));
        }
        let unit = crate::start::unit_name(sl.where_, sl.number);
        if let Some(u) = &unit {
            if sl.code >= 0 {
                f.insert(format!("at:{u}:{}", sl.code), json!(1));
            }
            if sl.frozen > 0 {
                f.insert(format!("frozen:{u}"), json!(1));
            }
        }
    }
    f.insert("cutscene".into(), json!(s.cutscene.depth));
    if let Some(top) = s.cutscene.levels.last().and_then(|l| l.override_.as_ref()) {
        if !top.stale && top.code >= 0 {
            if let Some(u) = crate::start::unit_name(top.where_, top.number) {
                f.insert(format!("esc:{u}:{}", top.code), json!(1));
            }
        }
    }
    for v in &s.verbs {
        f.insert(format!("verb:{}", v.id), json!(1));
    }
    Value::Object(f)
}

/// step.py `start_row`: the entry's start row in start-part1.jsonl's format (reader-scope --start-rows reads seed
/// and facts).
pub fn start_row(seed: i64, state: &fmi_core::Obs, status: &fmi_core::Status) -> Res<Value> {
    if seed != SEED {
        return Err(format!("ValueError: seed {seed}: E008 runs seed {SEED} only (plan.md)"));
    }
    let ego = status.ego.map(|(x, y)| json!([x, y])).unwrap_or(Value::Null);
    Ok(json!({"seed": seed, "frame": status.frame,
              "stop": {"stop": status.reason, "frame": status.frame, "tick": status.tick, "qf": status.qf, "room": status.room,
                       "idle": status.idle, "rng": status.rng, "up": status.up, "cs": status.cs, "ego": ego},
              "dump": {"frame": state.frame, "tick": state.tick, "room": state.room, "ego": state.ego,
                       "ego_pos": state.ego_pos.map(|(x, y)| json!([x, y])), "seed": state.seed, "boot_param": state.boot_param},
              "facts": facts_of_ordered(state)}))
}

/// entries_from's plan on frames alone: per entry (in frame order) the number of the attempt's checkpoints it holds
/// (frames > the entry's frame, one per frame, in order: a prefix), its events (the inputs up to its frame) and its
/// `how`; the same rules and the same error as entries_from.
pub fn entries_plan(entry_frame: i64, inputs: &[Value], frames: &[i64]) -> Res<Vec<(usize, Vec<Value>, &'static str)>> {
    let early: Vec<&Value> = inputs.iter().filter(|e| as_i64(e.get("frame")).unwrap_or(i64::MIN) <= entry_frame).collect();
    if let Some(e0) = early.first() {
        return Err(format!("ValueError: {} inputs at or before the entry's frame {entry_frame}: first {}", early.len(), files::py_repr(e0)));
    }
    let kept: Vec<i64> = frames.iter().copied().filter(|f| *f > entry_frame).collect();
    let n = kept.len();
    Ok(kept.iter().enumerate().map(|(i, &f)| {
        let events: Vec<Value> = inputs.iter().filter(|e| as_i64(e.get("frame")).unwrap_or(i64::MAX) <= f).cloned().collect();
        (i + 1, events, if i == n - 1 { "end" } else { "checkpoint" })
    }).collect())
}

/// target_holds on any facts map.
pub fn target_holds_in<F: crate::archive::FactLook + ?Sized>(t: &Target, goal: &[(String, Vec<i64>)], facts: &F) -> bool {
    let get = |k: &str| facts.look(k).unwrap_or(0);
    if t.tier == 0 {
        return goal.iter().all(|(k, vs)| vs.contains(&get(k)));
    }
    match t.v {
        TV::Int(v) => get(&t.k) == v,
        TV::Text(_) => false,
    }
}
