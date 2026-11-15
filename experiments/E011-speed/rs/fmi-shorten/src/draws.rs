//! E024 Part 2, item 3's cross-check (plan.md, "Draw keys"): the draws the reader found, against the twin fork's.
//!
//! The archive's readings (its `readings/` folder) are read through fmi-reading: in each, the actions with a check
//! on a random draw (fmi-reading draws.rs; range and share from the game's scripts when they load) and the keys their
//! effects write. A step is marked when a reading of one of its rooms has such an action whose effects write a key
//! the step changed or one of its twin's draw keys. Each reading is opened through a folder of links (`view`), so an
//! index the reader makes again goes there, never into the archive.

use crate::chain::Step;
use crate::{e2s, Res};
use fmi_core::{Key, ReadingSource};
use fmi_reading::draws::{self, Draw, DrawGame};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;

/// One action of a reading with checks on a random draw.
pub struct DrawAction {
    pub action: u32,
    /// the keys its effects write
    pub keys: Vec<Key>,
    pub draws: Vec<Draw>,
}

/// A reading's draw actions.
pub struct ReadingDraws {
    pub name: String,
    pub rooms: Vec<i64>,
    pub n_actions: u32,
    pub actions: Vec<DrawAction>,
}

/// The readings of the archive folder `archive`, each linked into `view` and read there.
pub fn load(archive: &Path, view: &Path, game: Option<&DrawGame>) -> Res<Vec<ReadingDraws>> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(archive.join("readings")) else { return Ok(out) };
    let mut dirs: Vec<_> = rd.flatten().map(|d| d.path()).filter(|p| p.join("reading.json").exists()).collect();
    dirs.sort();
    for d in dirs {
        let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let v = view.join(&name);
        std::fs::create_dir_all(&v).map_err(e2s)?;
        for f in std::fs::read_dir(&d).map_err(e2s)?.flatten() {
            let to = v.join(f.file_name());
            if !to.exists() {
                std::os::unix::fs::symlink(f.path(), &to).map_err(e2s)?;
            }
        }
        // the index: a copy, so that one made again replaces the copy (the reader writes a temporary file and renames)
        let idx = v.join("reading.idx");
        if idx.is_symlink() {
            std::fs::remove_file(&idx).map_err(e2s)?;
            std::fs::copy(d.join("reading.idx"), &idx).map_err(e2s)?;
        }
        let r = fmi_reading::Reading::open(&v).map_err(|e| format!("{}: {e}", v.display()))?;
        let head: Value = serde_json::from_str(r.head()).map_err(e2s)?;
        let rooms = head["rooms"].as_array().map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
        let mut actions = Vec::new();
        for i in 0..r.n_actions() {
            if !r.action_line(i).contains(draws::DRAW_WHY) {
                continue;
            }
            let ds = draws::draws(&r, i, game);
            if ds.is_empty() {
                continue;
            }
            let keys = r.action(i).effects.iter().map(|e| e.key).collect();
            actions.push(DrawAction { action: i, keys, draws: ds });
        }
        out.push(ReadingDraws { name, rooms, n_actions: r.n_actions(), actions });
    }
    Ok(out)
}

/// The readings' draw actions that mark step `s` (module docstring), as JSON: per reading, the actions and the keys
/// they meet, and the draws' ranges and shares.
pub fn mark(s: &Step, rds: &[ReadingDraws]) -> Vec<Value> {
    let mut want: BTreeSet<Key> = s.changed.iter().copied().collect();
    if let Some(t) = &s.twin {
        want.extend(t.draw.iter().copied());
    }
    let mut out = Vec::new();
    for r in rds.iter().filter(|r| r.rooms.iter().any(|x| s.rooms.contains(x))) {
        let mut met: BTreeSet<Key> = BTreeSet::new();
        let mut ranges: BTreeSet<(i32, i32)> = BTreeSet::new();
        let mut n = 0;
        for a in &r.actions {
            let hit: Vec<Key> = a.keys.iter().copied().filter(|k| want.contains(k)).collect();
            if hit.is_empty() {
                continue;
            }
            n += 1;
            met.extend(hit);
            ranges.extend(a.draws.iter().filter_map(|d| d.range));
        }
        if n > 0 {
            out.push(json!({"reading": r.name, "actions": n, "keys": met.iter().map(|k| k.text()).collect::<Vec<_>>(),
                            "ranges": ranges.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>()}));
        }
    }
    out
}

/// The readings' summary: per reading its rooms, its actions and those with draws.
pub fn summary(rds: &[ReadingDraws]) -> Value {
    Value::Array(
        rds.iter()
            .map(|r| {
                let keys: BTreeSet<Key> = r.actions.iter().flat_map(|a| a.keys.iter().copied()).collect();
                json!({"reading": r.name, "rooms": r.rooms, "actions": r.n_actions, "draw_actions": r.actions.len(),
                       "draw_keys": keys.len()})
            })
            .collect(),
    )
}
