//! ladder: E016's outside scorer (experiments/E016-realize-run/plan.md, "The outside ladder"). It reads an archive's
//! `entries.jsonl` and its entries' checkpoints (`entries/N.json.gz`) and nothing else of the system: no log, cells,
//! stalls, summary or result. It is the experiment's instrument, outside the system, so it names the demo's numbers.
//!
//!   ladder ARCHIVE [--json OUT] [--cache FILE] [--t0 EPOCH_S]
//!
//! Every checkpoint of every entry's file is a state its movie reaches from boot. A rung is reached by the first entry
//! (least id: entries are added in commit order) whose file holds a checkpoint where the rung's facts hold:
//!
//!   C0  the opening's dialog answered: room = 49 and var:32 = 4 (the input script back from the dialog's, 17)
//!   C1  room = 63
//!   C2  room = 69
//!   C3  owner:946 = ego (the `ego` fact)
//!   C4  room = 49 and owner:946 = ego
//!   C5  room = 42 and owner:946 = ego
//!   C6  class 12 cleared on object 491: class:491:12 = 0 (a set class is 1, a cleared one absent) in a full state
//!       (one with a room; an end checkpoint holds the end's facts only)
//!   C7  room = 82
//!   C8  room = 48
//!   C9  object 582's restart at 0x0067: end = 1, end:kind = 1 (restart), end:number = 582, end:offset = 0x67, and
//!       end:where an object's slot (0 inventory, 1 room, 4 floating; patch 0006-e014's site)
//!
//! Per rung: the entry, its try (the row's `job`), the entry's `how` and whether that is a planned step (`checkpoint`,
//! `end` without end facts: the plan's play in a model try) or a model-free one (`free`: a free try; `wait`, `afford`,
//! `delay`), the checkpoint's frame, and the wall time of the first reach: the entry file's modification time (written
//! once, at the commit) less t0 (`--t0`, else the root's file's time, the archive's start). "Furthest" is the highest
//! rung reached; "in order" the highest with every rung below it reached too.
//!
//! `--cache FILE`: each entry's rungs kept by id and file time, so a watcher's run reads only the new files.
//! Output: the JSON (to `--json OUT`, else stdout) and one line of text on stdout (last).

use fmi_explore::files;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Res<T> = Result<T, String>;

const N_RUNGS: usize = 10;

/// The rungs' facts as text (the JSON's "what").
const WHAT: [&str; N_RUNGS] = [
    "room=49 and var:32=4",
    "room=63",
    "room=69",
    "owner:946=ego",
    "room=49 and owner:946=ego",
    "room=42 and owner:946=ego",
    "class:491:12=0 (a full state)",
    "room=82",
    "room=48",
    "end=1, end:kind=1, end:number=582, end:offset=0x67, end:where in {0,1,4}",
];

/// A checkpoint's facts (a facts.py map): the value of a key, absent being None.
fn get(f: &Map<String, Value>, k: &str) -> Option<i64> {
    f.get(k).and_then(|v| v.as_i64())
}

/// The rungs a checkpoint's facts hold.
pub fn rungs_of(f: &Map<String, Value>) -> [bool; N_RUNGS] {
    let room = get(f, "room");
    let ego = get(f, "ego");
    let holds946 = ego.is_some() && get(f, "owner:946") == ego;
    let end_site = get(f, "end") == Some(1)
        && get(f, "end:kind") == Some(1)
        && get(f, "end:number") == Some(582)
        && get(f, "end:offset") == Some(0x67)
        && matches!(get(f, "end:where"), Some(0) | Some(1) | Some(4));
    [
        room == Some(49) && get(f, "var:32") == Some(4),
        room == Some(63),
        room == Some(69),
        holds946,
        room == Some(49) && holds946,
        room == Some(42) && holds946,
        room.is_some() && get(f, "class:491:12").unwrap_or(0) == 0,
        room == Some(82),
        room == Some(48),
        end_site,
    ]
}

/// One entry's rungs: per rung the first checkpoint's frame that holds it.
#[derive(Clone, Debug, Default)]
struct Hits {
    mtime: f64,
    frames: [Option<i64>; N_RUNGS],
    /// the last checkpoint holds end facts (a game-end entry)
    end: bool,
}

fn mtime_s(p: &Path) -> Option<f64> {
    let m = std::fs::metadata(p).ok()?.modified().ok()?;
    m.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs_f64())
}

fn hits_of(file: &Path) -> Res<Hits> {
    let doc = files::read_gz(file)?;
    let mut h = Hits { mtime: mtime_s(file).unwrap_or(0.0), ..Default::default() };
    let cps = doc.get("checkpoints").and_then(|c| c.as_array()).ok_or(format!("{}: no checkpoints", file.display()))?;
    for cp in cps {
        let frame = cp.get(0).and_then(|x| x.as_i64()).unwrap_or(-1);
        let Some(f) = cp.get(1).and_then(|x| x.as_object()) else { continue };
        for (k, held) in rungs_of(f).into_iter().enumerate() {
            if held && h.frames[k].is_none() {
                h.frames[k] = Some(frame);
            }
        }
    }
    h.end = cps.last().and_then(|c| c.get(1)).and_then(|f| f.get("end")).is_some();
    Ok(h)
}

fn hits_json(h: &Hits) -> Value {
    json!({"mtime": h.mtime, "frames": h.frames.to_vec(), "end": h.end})
}

fn hits_of_json(v: &Value) -> Option<Hits> {
    let fr = v.get("frames")?.as_array()?;
    if fr.len() != N_RUNGS {
        return None;
    }
    let mut frames = [None; N_RUNGS];
    for (k, x) in fr.iter().enumerate() {
        frames[k] = x.as_i64();
    }
    Some(Hits { mtime: v.get("mtime")?.as_f64()?, frames, end: v.get("end")?.as_bool()? })
}

/// Whether an entry's `how` is a planned step (the play of a model try's plan) or a model-free one.
fn step_of(how: &str, end: bool) -> &'static str {
    match how {
        "boot" => "boot",
        "checkpoint" => "planned",
        "end" if !end => "planned",
        "end" => "game end (its step is not in entries.jsonl)",
        "free" | "wait" | "afford" | "delay" => "model-free",
        _ => "unknown",
    }
}

/// UTC "YYYY-MM-DDTHH:MM:SSZ" of an epoch second.
fn iso(t: f64) -> String {
    let s = t.floor() as i64;
    let (days, rem) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    // civil from days (H. Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(arch) = args.get(1).filter(|a| !a.starts_with("--")).map(PathBuf::from) else {
        eprintln!("usage: ladder ARCHIVE [--json OUT] [--cache FILE] [--t0 EPOCH_S]");
        std::process::exit(2);
    };
    match run(&arch, &args) {
        Ok(line) => println!("{line}"),
        Err(e) => {
            eprintln!("ladder: {e}");
            std::process::exit(1);
        }
    }
}

fn run(arch: &Path, args: &[String]) -> Res<String> {
    let (rows, _) = files::read_jsonl(&arch.join("entries.jsonl"))?;
    let cache_p = arg(args, "--cache").map(PathBuf::from);
    let mut cache: BTreeMap<i64, Hits> = BTreeMap::new();
    if let Some(p) = cache_p.as_ref().filter(|p| p.exists()) {
        let v = files::read_json(p)?;
        if v.get("archive").and_then(|a| a.as_str()) == Some(&*arch.to_string_lossy()) {
            for (k, h) in v.get("entries").and_then(|e| e.as_object()).into_iter().flatten() {
                if let (Ok(id), Some(h)) = (k.parse::<i64>(), hits_of_json(h)) {
                    cache.insert(id, h);
                }
            }
        }
    }
    let mut read = 0usize;
    let mut first: [Option<(i64, i64)>; N_RUNGS] = [None; N_RUNGS];
    let mut by_id: BTreeMap<i64, (&Value, Hits)> = BTreeMap::new();
    for row in &rows {
        let Some(id) = row.get("id").and_then(|i| i.as_i64()) else { continue };
        let file = arch.join("entries").join(format!("{id}.json.gz"));
        let mt = mtime_s(&file).unwrap_or(0.0);
        let h = match cache.get(&id).filter(|h| h.mtime == mt) {
            Some(h) => h.clone(),
            None => {
                read += 1;
                hits_of(&file)?
            }
        };
        cache.insert(id, h.clone());
        by_id.insert(id, (row, h));
    }
    for (id, (_, h)) in &by_id {
        for k in 0..N_RUNGS {
            if first[k].is_none() {
                if let Some(f) = h.frames[k] {
                    first[k] = Some((*id, f));
                }
            }
        }
    }
    let t0 = match arg(args, "--t0") {
        Some(t) => (t.parse::<f64>().map_err(|e| format!("--t0: {e}"))?, "--t0"),
        None => (by_id.get(&0).map(|(_, h)| h.mtime).unwrap_or(0.0), "the root's file"),
    };
    let mut rungs = Vec::new();
    let mut furthest: Option<usize> = None;
    let mut in_order: Option<usize> = None;
    for k in 0..N_RUNGS {
        let mut r = json!({"rung": format!("C{k}"), "what": WHAT[k], "reached": first[k].is_some()});
        if let Some((id, frame)) = first[k] {
            let (row, h) = &by_id[&id];
            let how = row.get("how").and_then(|x| x.as_str()).unwrap_or("");
            r["entry"] = json!(id);
            r["job"] = row.get("job").cloned().unwrap_or(Value::Null);
            r["how"] = json!(how);
            r["step"] = json!(step_of(how, h.end));
            r["frame"] = json!(frame);
            r["room"] = row.get("room").cloned().unwrap_or(Value::Null);
            r["wall_s"] = json!(((h.mtime - t0.0) * 10.0).round() / 10.0);
            r["time"] = json!(iso(h.mtime));
            furthest = Some(k);
            if k == 0 || in_order == Some(k - 1) {
                in_order = Some(k);
            }
        }
        rungs.push(r);
    }
    let name = |k: Option<usize>| k.map(|k| format!("C{k}")).unwrap_or_else(|| "none".into());
    let out = json!({"archive": arch.to_string_lossy(), "entries": by_id.len(), "files_read": read,
                     "t0": t0.0, "t0_from": t0.1, "t0_time": iso(t0.0), "furthest": name(furthest),
                     "furthest_in_order": name(in_order), "rungs": rungs});
    match arg(args, "--json") {
        Some(p) => files::write_json(Path::new(p), &out, Some(1))?,
        None => println!("{}", serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?),
    }
    if let Some(p) = cache_p {
        let mut m = Map::new();
        for (id, h) in &cache {
            m.insert(id.to_string(), hits_json(h));
        }
        files::write_json(&p, &json!({"archive": arch.to_string_lossy(), "entries": Value::Object(m)}), None)?;
    }
    // the line: the furthest rung, then each rung reached: entry, try, step, minutes of wall
    let mut parts = Vec::new();
    for (k, r) in rungs.iter().enumerate() {
        if r["reached"] == json!(true) {
            parts.push(format!("C{k} e{} {} {} {:.1} min", r["entry"], r["job"].as_str().unwrap_or("-"),
                               if r["step"] == "planned" { "planned" } else if r["step"] == "model-free" { "model-free" } else { r["step"].as_str().unwrap_or("") },
                               r["wall_s"].as_f64().unwrap_or(0.0) / 60.0));
        }
    }
    Ok(format!("ladder {}: furthest {} (in order {}); {} entries; {}", arch.display(), name(furthest), name(in_order), by_id.len(),
               if parts.is_empty() { "no rung reached".to_string() } else { parts.join("; ") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn reached(v: Value) -> Vec<usize> {
        rungs_of(&f(v)).iter().enumerate().filter(|(_, h)| **h).map(|(k, _)| k).collect()
    }

    #[test]
    fn the_rungs_facts() {
        // the root: the opening's dialog up (var:32 17), class 12 set on 491
        assert!(reached(json!({"room": 49, "ego": 1, "var:32": 17, "owner:946": 15, "class:491:12": 1})).is_empty());
        assert_eq!(reached(json!({"room": 49, "ego": 1, "var:32": 4, "owner:946": 15, "class:491:12": 1})), vec![0]);
        assert_eq!(reached(json!({"room": 69, "ego": 1, "var:32": 4, "owner:946": 1, "class:491:12": 1})), vec![2, 3]);
        assert_eq!(reached(json!({"room": 49, "ego": 1, "var:32": 4, "owner:946": 1, "class:491:12": 1})), vec![0, 3, 4]);
        assert_eq!(reached(json!({"room": 42, "ego": 1, "var:32": 4, "owner:946": 1, "class:491:12": 1})), vec![3, 5]);
        // class 12 cleared (absent), 946 consumed
        assert_eq!(reached(json!({"room": 42, "ego": 1, "var:32": 4, "owner:946": 0})), vec![6]);
        assert_eq!(reached(json!({"room": 82, "ego": 1, "class:491:12": 1})), vec![7]);
        assert_eq!(reached(json!({"room": 48, "ego": 1, "class:491:12": 1})), vec![8]);
        // owner 946 with no ego fact is not the ego's
        assert!(reached(json!({"room": 63, "owner:946": 1, "class:491:12": 1})) == vec![1]);
    }

    #[test]
    fn c9_is_object_582s_restart_at_0x67_and_an_end_checkpoint_is_no_full_state() {
        let end = |w: i64, n: i64, off: i64, kind: i64| {
            json!({"end": 1, "end:kind": kind, "end:where": w, "end:number": n, "end:room": 48, "end:offset": off})
        };
        assert_eq!(reached(end(1, 582, 0x67, 1)), vec![9]);
        assert_eq!(reached(end(0, 582, 0x67, 1)), vec![9]);
        // a global's restart (the death screen's slot kind), another object, another offset, a quit: not C9; and an end
        // checkpoint holds no room, so it is not C6 though class:491:12 is absent from it
        assert!(reached(end(2, 57, 0xD0E, 1)).is_empty());
        assert!(reached(end(1, 581, 0x67, 1)).is_empty());
        assert!(reached(end(1, 582, 0x68, 1)).is_empty());
        assert!(reached(end(1, 582, 0x67, 3)).is_empty());
    }

    #[test]
    fn planned_or_model_free() {
        assert_eq!(step_of("checkpoint", false), "planned");
        assert_eq!(step_of("end", false), "planned");
        assert!(step_of("end", true).starts_with("game end"));
        for h in ["free", "wait", "afford", "delay"] {
            assert_eq!(step_of(h, false), "model-free");
        }
        assert_eq!(step_of("boot", false), "boot");
    }

    #[test]
    fn iso_times() {
        assert_eq!(iso(0.0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(1_793_190_000.5), "2026-10-28T12:20:00Z");
        assert_eq!(iso(951_782_400.0), "2000-02-29T00:00:00Z");
    }
}
