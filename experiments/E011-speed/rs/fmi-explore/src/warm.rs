//! What the one process keeps from try to try besides the ports: each reading's reading.json parsed, its reached
//! values, and the guides (a guide file is read again only when it changed).

use crate::files;
use crate::pyjson;
use fmi_core::ReadingSource;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

type Reached = Arc<Vec<(String, Vec<i64>)>>;

/// a reading's key here: its folder and its packed file's sha1 (two folders can hold the same packed file and differ
/// in reading.json, a fresh reading of the same scope from another entry)
type RKey = (PathBuf, [u8; 20]);

fn heads() -> &'static Mutex<HashMap<RKey, Arc<Value>>> {
    static C: OnceLock<Mutex<HashMap<RKey, Arc<Value>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reacheds() -> &'static Mutex<HashMap<RKey, Reached>> {
    static C: OnceLock<Mutex<HashMap<RKey, Reached>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

fn guides() -> &'static Mutex<HashMap<PathBuf, (Option<SystemTime>, Arc<Value>)>> {
    static C: OnceLock<Mutex<HashMap<PathBuf, (Option<SystemTime>, Arc<Value>)>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// at most this many readings kept (about 1 MB each parsed): a reading not used for a while is read again
const KEEP: usize = 64;

fn bound<K: Clone + std::hash::Hash + Eq, V>(h: &mut HashMap<K, V>) {
    if h.len() >= KEEP {
        if let Some(k) = h.keys().next().cloned() {
            h.remove(&k);
        }
    }
}

/// A reading's reading.json, parsed once per reading (a reading's files never change once made).
pub fn head(rd: &dyn ReadingSource) -> Arc<Value> {
    let id = (rd.dir().to_path_buf(), rd.id());
    if let Some(v) = heads().lock().unwrap().get(&id) {
        return v.clone();
    }
    let v = Arc::new(pyjson::loads(rd.head()).unwrap_or(Value::Null));
    let mut h = heads().lock().unwrap();
    bound(&mut h);
    h.insert(id, v.clone());
    v
}

/// A reading's reached values (step.read_reached), parsed once per reading.
pub fn reached(rd: &dyn ReadingSource) -> Reached {
    let id = (rd.dir().to_path_buf(), rd.id());
    if let Some(v) = reacheds().lock().unwrap().get(&id) {
        return v.clone();
    }
    let v = Arc::new(crate::step::read_reached_text(rd.reached()));
    let mut h = reacheds().lock().unwrap();
    bound(&mut h);
    h.insert(id, v.clone());
    v
}

/// A guide file, read again only when its modification time changed.
pub fn guide(p: &Path) -> files::Res<Arc<Value>> {
    let mt = std::fs::metadata(p).and_then(|m| m.modified()).ok();
    if let Some((t, v)) = guides().lock().unwrap().get(p) {
        if *t == mt && mt.is_some() {
            return Ok(v.clone());
        }
    }
    let v = Arc::new(files::read_json(p)?);
    guides().lock().unwrap().insert(p.to_path_buf(), (mt, v.clone()));
    Ok(v)
}
