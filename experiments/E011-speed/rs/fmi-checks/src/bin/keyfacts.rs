//! keyfacts: E001's Part I key facts held by an archive's entries (experiments/E026-part1/plan.md I3), a port of
//! `~/.cache/fast-mi-e026/mi-history/keyfacts.py`. Evaluation only, outside the system: it reads the key and each
//! archive's `entries.jsonl`, nothing else.
//!
//!   keyfacts ARCHIVE... [--key KEY_JSON] [--json OUT]
//!
//! KEY_JSON: E001's key.json (experiments/E001-goals-from-scripts/answer_key.py writes it; never committed, as it
//! quotes the game), default the repository's. Its facts of part "I", numbered from 1 in the key's order, are compared
//! on each entry's `held` list: a fact absent from `held` is unknown. [kind, n, v]: bit, var, state and owner are
//! `kind:n` = v; owner_not `owner:n` != v; class `class:n:|v|` = 1 when v > 0, else 0; any other kind is never held.
//!
//! Per archive, as keyfacts.py prints it: the entries, the rooms seen, and the facts held by some entry with the frame
//! and id of the earliest entry holding each (the first in the file on a tie). Then the most facts one entry's chain
//! holds (a fact held by the entry or an ancestor), with that entry (least id on a tie) and its chain. `--json OUT`:
//! the same, numbers only (no key text).

use fmi_explore::files;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

type Res<T> = Result<T, String>;

/// keyfacts.py `fact_key`: the held key, the value and whether it must equal (else differ).
fn fact_key(f: &Value) -> Option<(String, i64, bool)> {
    let (kind, n, v) = (f.get(0)?.as_str()?, f.get(1)?.as_i64()?, f.get(2)?.as_i64()?);
    match kind {
        "bit" | "var" | "state" | "owner" => Some((format!("{kind}:{n}"), v, true)),
        "owner_not" => Some((format!("owner:{n}"), v, false)),
        "class" => Some((format!("class:{n}:{}", v.abs()), if v > 0 { 1 } else { 0 }, true)),
        _ => None,
    }
}

/// The Part I facts of a key.json, in order.
fn part1(key: &Value) -> Res<Vec<Value>> {
    let k = key.get("key").and_then(|k| k.as_array()).ok_or("no \"key\" list")?;
    Ok(k.iter().filter(|x| x.get("part").and_then(|p| p.as_str()) == Some("I")).map(|x| x["fact"].clone()).collect())
}

/// The 1-based indices of the facts an entry's `held` holds.
fn held_of(row: &Value, facts: &[Option<(String, i64, bool)>]) -> (BTreeSet<usize>, Option<i64>) {
    let mut held: BTreeMap<&str, i64> = BTreeMap::new();
    for p in row.get("held").and_then(|h| h.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
        if let (Some(k), Some(v)) = (p.get(0).and_then(|k| k.as_str()), p.get(1).and_then(|v| v.as_i64())) {
            held.insert(k, v);
        }
    }
    let mut out = BTreeSet::new();
    for (i, f) in facts.iter().enumerate() {
        let Some((name, v, eq)) = f else { continue };
        if let Some(h) = held.get(name.as_str()) {
            if (h == v) == *eq {
                out.insert(i + 1);
            }
        }
    }
    (out, held.get("room").copied())
}

fn chain_of(id: i64, parent: &BTreeMap<i64, Option<i64>>) -> Vec<i64> {
    let mut c = vec![id];
    let mut at = id;
    while let Some(Some(p)) = parent.get(&at) {
        if c.contains(p) {
            break;
        }
        c.push(*p);
        at = *p;
    }
    c.reverse();
    c
}

/// One archive: the text (keyfacts.py's two lines, then the best chain's) and the JSON.
fn archive(arch: &str, facts: &[Value]) -> Res<(String, Value)> {
    let keys: Vec<Option<(String, i64, bool)>> = facts.iter().map(fact_key).collect();
    let (rows, _) = files::read_jsonl(&Path::new(arch).join("entries.jsonl"))?;
    let mut rooms = BTreeSet::new();
    let mut best: BTreeMap<usize, (i64, i64)> = BTreeMap::new();
    let mut own: BTreeMap<i64, BTreeSet<usize>> = BTreeMap::new();
    let mut parent: BTreeMap<i64, Option<i64>> = BTreeMap::new();
    for e in &rows {
        let (held, room) = held_of(e, &keys);
        if let Some(r) = room {
            rooms.insert(r);
        }
        let (frame, id) = (e["frame"].as_i64().unwrap_or(0), e["id"].as_i64().unwrap_or(-1));
        for &i in &held {
            if best.get(&i).map_or(true, |b| frame < b.0) {
                best.insert(i, (frame, id));
            }
        }
        own.insert(id, held);
        parent.insert(id, e.get("parent").and_then(|p| p.as_i64()));
    }
    // per entry the facts its chain holds (a parent is committed before its child: ids ascending)
    let mut on_chain: BTreeMap<i64, BTreeSet<usize>> = BTreeMap::new();
    let mut top: Option<(usize, i64)> = None;
    for (id, s) in &own {
        let mut c = s.clone();
        if let Some(Some(p)) = parent.get(id) {
            match on_chain.get(p) {
                Some(ps) => c.extend(ps),
                None => chain_of(*p, &parent).iter().for_each(|a| c.extend(own.get(a).into_iter().flatten())),
            }
        }
        if top.map_or(true, |t| c.len() > t.0) {
            top = Some((c.len(), *id));
        }
        on_chain.insert(*id, c);
    }
    let rooms_txt = rooms.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ");
    let held_txt = best.iter().map(|(i, (f, e))| format!("#{i}: {f}@{e}")).collect::<Vec<_>>().join(", ");
    let mut text = format!("{arch}: entries {}; rooms [{rooms_txt}]\n  Part I key facts held (index: frame, entry): {held_txt}\n", rows.len());
    let top_json = match top {
        Some((n, id)) => {
            let ix: Vec<usize> = on_chain[&id].iter().copied().collect();
            text += &format!("  the most on one chain: {n} of {} (entry {id}): {}\n", facts.len(),
                             ix.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(", "));
            json!({"facts": n, "entry": id, "indices": ix, "chain": chain_of(id, &parent)})
        }
        None => json!(null),
    };
    let mut held = Map::new();
    for (i, (f, e)) in &best {
        held.insert(i.to_string(), json!({"fact": facts[i - 1], "frame": f, "entry": e}));
    }
    Ok((text, json!({"archive": arch, "entries": rows.len(), "rooms": rooms, "facts": facts.len(), "held": held, "best_chain": top_json})))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut archs = Vec::new();
    let (mut key, mut out): (Option<PathBuf>, Option<PathBuf>) = (None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--key" => key = it.next().map(PathBuf::from),
            "--json" => out = it.next().map(PathBuf::from),
            _ => archs.push(a.clone()),
        }
    }
    if archs.is_empty() {
        eprintln!("usage: keyfacts ARCHIVE... [--key KEY_JSON] [--json OUT]");
        std::process::exit(2);
    }
    let key = key.unwrap_or_else(|| scumm_ir::game::default_root().join("experiments/E001-goals-from-scripts/key.json"));
    let r = files::read_json(&key)
        .and_then(|k| part1(&k))
        .map_err(|e| format!("--key {}: {e} (E001's key.json: answer_key.py writes it)", key.display()))
        .and_then(|facts| {
            let mut docs = Vec::new();
            for a in &archs {
                let (text, doc) = archive(a, &facts)?;
                print!("{text}");
                docs.push(doc);
            }
            match &out {
                Some(p) => files::write_json(p, &json!({"key": key.display().to_string(), "archives": docs}), Some(1)),
                None => Ok(()),
            }
        });
    if let Err(e) = r {
        eprintln!("keyfacts: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facts_as_keyfacts_py_reads_them() {
        assert_eq!(fact_key(&json!(["bit", 72, 1])), Some(("bit:72".into(), 1, true)));
        assert_eq!(fact_key(&json!(["owner_not", 5, 2])), Some(("owner:5".into(), 2, false)));
        assert_eq!(fact_key(&json!(["class", 9, -6])), Some(("class:9:6".into(), 0, true)));
        assert_eq!(fact_key(&json!(["class", 9, 6])), Some(("class:9:6".into(), 1, true)));
        assert_eq!(fact_key(&json!(["script", 9, 1])), None);
    }

    #[test]
    fn earliest_entry_per_fact_and_the_best_chain() {
        let d = std::env::temp_dir().join(format!("keyfacts-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let rows = [json!({"id": 0, "parent": null, "frame": 10, "held": [["room", 33], ["bit:1", 0]]}),
                    json!({"id": 1, "parent": 0, "frame": 30, "held": [["room", 41], ["bit:1", 1], ["owner:7", 3]]}),
                    json!({"id": 2, "parent": 0, "frame": 20, "held": [["room", 35], ["bit:1", 1]]}),
                    json!({"id": 3, "parent": 1, "frame": 40, "held": [["room", 35], ["owner:7", 1], ["var:4", 2]]})];
        std::fs::write(d.join("entries.jsonl"), rows.iter().map(|r| format!("{r}\n")).collect::<String>()).unwrap();
        let facts = vec![json!(["bit", 1, 1]), json!(["owner_not", 7, 1]), json!(["var", 4, 2]), json!(["state", 8, 1])];
        let (text, doc) = archive(&d.display().to_string(), &facts).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].ends_with(": entries 4; rooms [33, 35, 41]"));
        // #1 earliest at entry 2 (frame 20); #2 only at entry 1; #3 at entry 3
        assert_eq!(lines[1], "  Part I key facts held (index: frame, entry): #1: 20@2, #2: 30@1, #3: 40@3");
        // entry 3's chain (0, 1, 3) holds #1 and #2 (at 1) and #3
        assert_eq!(lines[2], "  the most on one chain: 3 of 4 (entry 3): #1, #2, #3");
        assert_eq!(doc["best_chain"]["chain"], json!([0, 1, 3]));
        assert_eq!(doc["held"]["1"], json!({"fact": ["bit", 1, 1], "frame": 20, "entry": 2}));
        let _ = std::fs::remove_dir_all(&d);
    }
}
