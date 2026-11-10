//! ladder: E016's outside scorer (experiments/E016-realize-run/plan.md, "The outside ladder"), with E026's rungs from a
//! file (experiments/E026-part1/plan.md I2). It reads an archive's `entries.jsonl` and its entries' checkpoints
//! (`entries/N.json.gz`) and nothing else of the system: no log, cells, stalls, summary or result. It is the
//! experiment's instrument, outside the system: the games' numbers are in the rungs files (`fmi-checks/rungs/`), never
//! read by the system.
//!
//!   ladder ARCHIVE --rungs FILE [--json OUT] [--cache FILE] [--t0 EPOCH_S] [--score]
//!
//! A rung (FILE's "rungs", in order) is a set of facts that all hold ("all"), an optional group of which at least k
//! hold ("least": k, "of"), and an optional full-state flag ("full": the checkpoint holds a room; an end checkpoint
//! holds the end's facts only). A fact is [KEY, OP, VALUE]: OP one of = != >= <= > < and in; VALUE a number, a list
//! (in), or another key (both present). A key absent from a checkpoint is unknown, except a bit or a class in a full
//! state, which is 0 (facts.py lists set bits and classes only). `rungs-demo.json` holds the demo's C0-C9 (E016's,
//! once built in here); `rungs-part1.json` Monkey Island Part I's P0-P14 (E026).
//!
//! Every checkpoint of every entry's file is a state its movie reaches from boot. A rung is reached by the first entry
//! (least id: entries are added in commit order) whose file holds a checkpoint where the rung's facts hold.
//!
//! Per rung: the entry, its try (the row's `job`), the entry's `how` and whether that is a planned step (`checkpoint`,
//! `end` without end facts: the plan's play in a model try) or a model-free one (`free`: a free try; `wait`, `afford`,
//! `delay`), the checkpoint's frame, and the wall time of the first reach: the entry file's modification time (written
//! once, at the commit) less t0 (`--t0`, else the root's file's time, the archive's start). "Furthest" is the highest
//! rung reached; "in order" the highest with every rung below it reached too.
//!
//! `--score` (E026 I2): each rung scored on its own, its first entry's chain (root to entry) added; and the score, the
//! most rungs one entry's chain holds (a rung held at a checkpoint of the entry's file or an ancestor's), with that
//! entry (least id on a tie) and its chain, so order does not matter. Which entries verify from boot is the caller's.
//!
//! `--cache FILE`: each entry's rungs kept by id and file time, so a watcher's run reads only the new files (a cache of
//! other rungs is not used).
//! Output: the JSON (to `--json OUT`, else stdout) and one line of text on stdout (last).

use fmi_explore::files;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Res<T> = Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    Eq,
    Ne,
    Ge,
    Le,
    Gt,
    Lt,
    In,
}

/// A fact's right side: a number, a set (in), or another key's value.
#[derive(Clone, Debug, PartialEq)]
enum Rhs {
    N(i64),
    Set(Vec<i64>),
    Key(String),
}

#[derive(Clone, Debug, PartialEq)]
struct Fact {
    key: String,
    op: Op,
    rhs: Rhs,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rung {
    name: String,
    what: String,
    full: bool,
    all: Vec<Fact>,
    least: usize,
    of: Vec<Fact>,
}

fn fact_of(v: &Value) -> Res<Fact> {
    let a = v.as_array().filter(|a| a.len() == 3).ok_or(format!("a fact is [KEY, OP, VALUE]: {v}"))?;
    let key = a[0].as_str().ok_or(format!("a fact's key: {v}"))?.to_string();
    let op = match a[1].as_str().unwrap_or("") {
        "=" => Op::Eq,
        "!=" => Op::Ne,
        ">=" => Op::Ge,
        "<=" => Op::Le,
        ">" => Op::Gt,
        "<" => Op::Lt,
        "in" => Op::In,
        _ => return Err(format!("a fact's op (= != >= <= > < in): {v}")),
    };
    let rhs = match (&a[2], op) {
        (Value::Array(xs), Op::In) => Rhs::Set(xs.iter().map(|x| x.as_i64().ok_or(format!("in: a list of numbers: {v}"))).collect::<Res<_>>()?),
        (_, Op::In) => return Err(format!("in: a list of numbers: {v}")),
        (Value::String(k), _) => Rhs::Key(k.clone()),
        (x, _) => Rhs::N(x.as_i64().ok_or(format!("a fact's value (a number or a key): {v}"))?),
    };
    Ok(Fact { key, op, rhs })
}

fn facts_of(v: Option<&Value>) -> Res<Vec<Fact>> {
    v.and_then(|x| x.as_array()).map(|a| a.iter().map(fact_of).collect()).unwrap_or(Ok(Vec::new()))
}

/// The rungs of a rungs file's JSON.
pub fn rungs_of_json(doc: &Value) -> Res<Vec<Rung>> {
    let rs = doc.get("rungs").and_then(|r| r.as_array()).ok_or("no \"rungs\" list")?;
    let mut out = Vec::new();
    for r in rs {
        let name = r.get("rung").and_then(|x| x.as_str()).ok_or(format!("a rung's name (\"rung\"): {r}"))?.to_string();
        let of = facts_of(r.get("of"))?;
        let least = r.get("least").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        if least > of.len() {
            return Err(format!("{name}: at least {least} of {} facts", of.len()));
        }
        out.push(Rung {
            what: r.get("what").and_then(|x| x.as_str()).unwrap_or(&name).to_string(),
            full: r.get("full").and_then(|x| x.as_bool()).unwrap_or(false),
            all: facts_of(r.get("all"))?,
            least,
            of,
            name,
        });
    }
    if out.is_empty() {
        return Err("no rungs".into());
    }
    Ok(out)
}

/// A key's value in a checkpoint's facts (a facts.py map): present, or 0 for a bit or a class absent from a full state.
fn value(f: &Map<String, Value>, k: &str, full: bool) -> Option<i64> {
    match f.get(k).and_then(|v| v.as_i64()) {
        Some(v) => Some(v),
        None if full && (k.starts_with("bit:") || k.starts_with("class:")) => Some(0),
        None => None,
    }
}

fn holds(x: &Fact, f: &Map<String, Value>, full: bool) -> bool {
    let Some(a) = value(f, &x.key, full) else { return false };
    let b = match &x.rhs {
        Rhs::Set(s) => return x.op == Op::In && s.contains(&a),
        Rhs::N(n) => *n,
        Rhs::Key(k) => match value(f, k, full) {
            Some(b) => b,
            None => return false,
        },
    };
    match x.op {
        Op::Eq => a == b,
        Op::Ne => a != b,
        Op::Ge => a >= b,
        Op::Le => a <= b,
        Op::Gt => a > b,
        Op::Lt => a < b,
        Op::In => false,
    }
}

/// The rungs a checkpoint's facts hold.
pub fn rungs_held(rungs: &[Rung], f: &Map<String, Value>) -> Vec<bool> {
    let full = f.contains_key("room");
    rungs
        .iter()
        .map(|r| {
            (full || !r.full)
                && r.all.iter().all(|x| holds(x, f, full))
                && r.of.iter().filter(|x| holds(x, f, full)).count() >= r.least
        })
        .collect()
}

/// One entry's rungs: per rung the first checkpoint's frame that holds it.
#[derive(Clone, Debug, Default)]
struct Hits {
    mtime: f64,
    frames: Vec<Option<i64>>,
    /// the last checkpoint holds end facts (a game-end entry)
    end: bool,
}

fn mtime_s(p: &Path) -> Option<f64> {
    let m = std::fs::metadata(p).ok()?.modified().ok()?;
    m.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs_f64())
}

fn hits_of(file: &Path, rungs: &[Rung]) -> Res<Hits> {
    let doc = files::read_gz(file)?;
    let mut h = Hits { mtime: mtime_s(file).unwrap_or(0.0), frames: vec![None; rungs.len()], ..Default::default() };
    let cps = doc.get("checkpoints").and_then(|c| c.as_array()).ok_or(format!("{}: no checkpoints", file.display()))?;
    for cp in cps {
        let frame = cp.get(0).and_then(|x| x.as_i64()).unwrap_or(-1);
        let Some(f) = cp.get(1).and_then(|x| x.as_object()) else { continue };
        for (k, held) in rungs_held(rungs, f).into_iter().enumerate() {
            if held && h.frames[k].is_none() {
                h.frames[k] = Some(frame);
            }
        }
    }
    h.end = cps.last().and_then(|c| c.get(1)).and_then(|f| f.get("end")).is_some();
    Ok(h)
}

fn hits_json(h: &Hits) -> Value {
    json!({"mtime": h.mtime, "frames": h.frames, "end": h.end})
}

fn hits_of_json(v: &Value, n: usize) -> Option<Hits> {
    let fr = v.get("frames")?.as_array()?;
    if fr.len() != n {
        return None;
    }
    Some(Hits { mtime: v.get("mtime")?.as_f64()?, frames: fr.iter().map(|x| x.as_i64()).collect(), end: v.get("end")?.as_bool()? })
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
    let (Some(arch), Some(rf)) = (args.get(1).filter(|a| !a.starts_with("--")).map(PathBuf::from), arg(&args, "--rungs")) else {
        eprintln!("usage: ladder ARCHIVE --rungs FILE [--json OUT] [--cache FILE] [--t0 EPOCH_S] [--score]");
        std::process::exit(2);
    };
    match files::read_json(Path::new(rf)).and_then(|d| rungs_of_json(&d)).map_err(|e| format!("--rungs {rf}: {e}")).and_then(|r| run(&arch, &r, &args)) {
        Ok(line) => println!("{line}"),
        Err(e) => {
            eprintln!("ladder: {e}");
            std::process::exit(1);
        }
    }
}

/// The ids from the root to `id` by the rows' parents.
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

fn run(arch: &Path, rungs: &[Rung], args: &[String]) -> Res<String> {
    let n = rungs.len();
    let (rows, _) = files::read_jsonl(&arch.join("entries.jsonl"))?;
    let cache_p = arg(args, "--cache").map(PathBuf::from);
    let sig: Vec<Value> = rungs.iter().map(|r| json!([r.name, r.what])).collect();
    let mut cache: BTreeMap<i64, Hits> = BTreeMap::new();
    if let Some(p) = cache_p.as_ref().filter(|p| p.exists()) {
        let v = files::read_json(p)?;
        if v.get("archive").and_then(|a| a.as_str()) == Some(&*arch.to_string_lossy()) && v.get("rungs") == Some(&json!(sig)) {
            for (k, h) in v.get("entries").and_then(|e| e.as_object()).into_iter().flatten() {
                if let (Ok(id), Some(h)) = (k.parse::<i64>(), hits_of_json(h, n)) {
                    cache.insert(id, h);
                }
            }
        }
    }
    let mut read = 0usize;
    let mut first: Vec<Option<(i64, i64)>> = vec![None; n];
    let mut by_id: BTreeMap<i64, (&Value, Hits)> = BTreeMap::new();
    for row in &rows {
        let Some(id) = row.get("id").and_then(|i| i.as_i64()) else { continue };
        let file = arch.join("entries").join(format!("{id}.json.gz"));
        let mt = mtime_s(&file).unwrap_or(0.0);
        let h = match cache.get(&id).filter(|h| h.mtime == mt) {
            Some(h) => h.clone(),
            None => {
                read += 1;
                hits_of(&file, rungs)?
            }
        };
        cache.insert(id, h.clone());
        by_id.insert(id, (row, h));
    }
    for (id, (_, h)) in &by_id {
        for k in 0..n {
            if first[k].is_none() {
                if let Some(f) = h.frames[k] {
                    first[k] = Some((*id, f));
                }
            }
        }
    }
    let score = args.iter().any(|a| a == "--score");
    let parent: BTreeMap<i64, Option<i64>> = by_id.iter().map(|(id, (row, _))| (*id, row.get("parent").and_then(|p| p.as_i64()))).collect();
    let t0 = match arg(args, "--t0") {
        Some(t) => (t.parse::<f64>().map_err(|e| format!("--t0: {e}"))?, "--t0"),
        None => (by_id.get(&0).map(|(_, h)| h.mtime).unwrap_or(0.0), "the root's file"),
    };
    let mut out_rungs = Vec::new();
    let mut furthest: Option<usize> = None;
    let mut in_order: Option<usize> = None;
    for (k, rung) in rungs.iter().enumerate() {
        let mut r = json!({"rung": rung.name, "what": rung.what, "reached": first[k].is_some()});
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
            if score {
                r["chain"] = json!(chain_of(id, &parent));
            }
            furthest = Some(k);
            if k == 0 || in_order == Some(k - 1) {
                in_order = Some(k);
            }
        }
        out_rungs.push(r);
    }
    let name = |k: Option<usize>| k.map(|k| rungs[k].name.clone()).unwrap_or_else(|| "none".into());
    let mut out = json!({"archive": arch.to_string_lossy(), "entries": by_id.len(), "files_read": read,
                         "t0": t0.0, "t0_from": t0.1, "t0_time": iso(t0.0), "furthest": name(furthest),
                         "furthest_in_order": name(in_order), "rungs": out_rungs});
    // the score: per entry the rungs its chain holds (a parent is committed before its child: ids ascending)
    let mut best: Option<(usize, i64)> = None;
    if score {
        let mut held: BTreeMap<i64, Vec<bool>> = BTreeMap::new();
        for (id, (_, h)) in &by_id {
            let mut v: Vec<bool> = h.frames.iter().map(|f| f.is_some()).collect();
            if let Some(Some(p)) = parent.get(id) {
                let ps = held.get(p).cloned().unwrap_or_else(|| {
                    let mut s = vec![false; n];
                    for a in chain_of(*p, &parent) {
                        for (k, f) in by_id.get(&a).map(|x| x.1.frames.clone()).unwrap_or_default().into_iter().enumerate() {
                            s[k] |= f.is_some();
                        }
                    }
                    s
                });
                for k in 0..n {
                    v[k] |= ps[k];
                }
            }
            let c = v.iter().filter(|x| **x).count();
            if best.map_or(true, |b| c > b.0) {
                best = Some((c, *id));
            }
            held.insert(*id, v);
        }
        out["score"] = match best {
            Some((c, id)) => json!({"rungs": c, "of": n, "entry": id,
                                    "held": rungs.iter().zip(&held[&id]).filter(|(_, h)| **h).map(|(r, _)| r.name.clone()).collect::<Vec<_>>(),
                                    "chain": chain_of(id, &parent)}),
            None => json!({"rungs": 0, "of": n, "entry": null, "held": [], "chain": []}),
        };
    }
    match arg(args, "--json") {
        Some(p) => files::write_json(Path::new(p), &out, Some(1))?,
        None => println!("{}", serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?),
    }
    if let Some(p) = cache_p {
        let mut m = Map::new();
        for (id, h) in &cache {
            m.insert(id.to_string(), hits_json(h));
        }
        files::write_json(&p, &json!({"archive": arch.to_string_lossy(), "rungs": sig, "entries": Value::Object(m)}), None)?;
    }
    // the line: the furthest rung, then each rung reached: entry, try, step, minutes of wall
    let mut parts = Vec::new();
    for r in &out["rungs"].as_array().cloned().unwrap_or_default() {
        if r["reached"] == json!(true) {
            parts.push(format!("{} e{} {} {} {:.1} min", r["rung"].as_str().unwrap_or(""), r["entry"], r["job"].as_str().unwrap_or("-"),
                               if r["step"] == "planned" { "planned" } else if r["step"] == "model-free" { "model-free" } else { r["step"].as_str().unwrap_or("") },
                               r["wall_s"].as_f64().unwrap_or(0.0) / 60.0));
        }
    }
    let tail = match best {
        Some((c, id)) if score => format!("; score {c} of {n} rungs (entry {id}'s chain)"),
        _ if score => format!("; score 0 of {n} rungs"),
        _ => String::new(),
    };
    Ok(format!("ladder {}: furthest {} (in order {}); {} entries; {}{tail}", arch.display(), name(furthest), name(in_order), by_id.len(),
               if parts.is_empty() { "no rung reached".to_string() } else { parts.join("; ") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Vec<Rung> {
        rungs_of_json(&serde_json::from_str(include_str!("../../rungs/rungs-demo.json")).unwrap()).unwrap()
    }

    fn part1() -> Vec<Rung> {
        rungs_of_json(&serde_json::from_str(include_str!("../../rungs/rungs-part1.json")).unwrap()).unwrap()
    }

    fn f(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn reached(rungs: &[Rung], v: Value) -> Vec<usize> {
        rungs_held(rungs, &f(v)).iter().enumerate().filter(|(_, h)| **h).map(|(k, _)| k).collect()
    }

    #[test]
    fn the_demo_file_is_the_built_in_c0_to_c9() {
        let d = demo();
        let names: Vec<&str> = d.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["C0", "C1", "C2", "C3", "C4", "C5", "C6", "C7", "C8", "C9"]);
        // E016's WHAT, word for word (the JSON's "what")
        assert_eq!(d[0].what, "room=49 and var:32=4");
        assert_eq!(d[6].what, "class:491:12=0 (a full state)");
        assert_eq!(d[9].what, "end=1, end:kind=1, end:number=582, end:offset=0x67, end:where in {0,1,4}");
        assert!(d[6].full && d.iter().filter(|r| r.full).count() == 1);
    }

    #[test]
    fn the_rungs_facts() {
        let d = demo();
        // the root: the opening's dialog up (var:32 17), class 12 set on 491
        assert!(reached(&d, json!({"room": 49, "ego": 1, "var:32": 17, "owner:946": 15, "class:491:12": 1})).is_empty());
        assert_eq!(reached(&d, json!({"room": 49, "ego": 1, "var:32": 4, "owner:946": 15, "class:491:12": 1})), vec![0]);
        assert_eq!(reached(&d, json!({"room": 69, "ego": 1, "var:32": 4, "owner:946": 1, "class:491:12": 1})), vec![2, 3]);
        assert_eq!(reached(&d, json!({"room": 49, "ego": 1, "var:32": 4, "owner:946": 1, "class:491:12": 1})), vec![0, 3, 4]);
        assert_eq!(reached(&d, json!({"room": 42, "ego": 1, "var:32": 4, "owner:946": 1, "class:491:12": 1})), vec![3, 5]);
        // class 12 cleared (absent), 946 consumed
        assert_eq!(reached(&d, json!({"room": 42, "ego": 1, "var:32": 4, "owner:946": 0})), vec![6]);
        assert_eq!(reached(&d, json!({"room": 82, "ego": 1, "class:491:12": 1})), vec![7]);
        assert_eq!(reached(&d, json!({"room": 48, "ego": 1, "class:491:12": 1})), vec![8]);
        // owner 946 with no ego fact is not the ego's
        assert!(reached(&d, json!({"room": 63, "owner:946": 1, "class:491:12": 1})) == vec![1]);
    }

    #[test]
    fn c9_is_object_582s_restart_at_0x67_and_an_end_checkpoint_is_no_full_state() {
        let d = demo();
        let end = |w: i64, n: i64, off: i64, kind: i64| {
            json!({"end": 1, "end:kind": kind, "end:where": w, "end:number": n, "end:room": 48, "end:offset": off})
        };
        assert_eq!(reached(&d, end(1, 582, 0x67, 1)), vec![9]);
        assert_eq!(reached(&d, end(0, 582, 0x67, 1)), vec![9]);
        // a global's restart (the death screen's slot kind), another object, another offset, a quit: not C9; and an end
        // checkpoint holds no room, so it is not C6 though class:491:12 is absent from it
        assert!(reached(&d, end(2, 57, 0xD0E, 1)).is_empty());
        assert!(reached(&d, end(1, 581, 0x67, 1)).is_empty());
        assert!(reached(&d, end(1, 582, 0x68, 1)).is_empty());
        assert!(reached(&d, end(1, 582, 0x67, 3)).is_empty());
        // end:where absent: not C9
        assert!(reached(&d, json!({"end": 1, "end:kind": 1, "end:number": 582, "end:offset": 0x67})).is_empty());
    }

    #[test]
    fn part1_rungs_counters_groups_and_the_root() {
        let p = part1();
        assert_eq!(p.len(), 15);
        assert_eq!(p[0].name, "P0");
        assert_eq!(p[14].name, "P14");
        // P0 holds at any full state, never at an end checkpoint
        assert_eq!(reached(&p, json!({"room": 33, "var:196": 0})), vec![0]);
        assert!(reached(&p, json!({"end": 1, "end:kind": 1})).is_empty());
        // the kitchen; the trials counted by var:196 >= k
        assert_eq!(reached(&p, json!({"room": 41, "owner:566": 1})), vec![0, 1]);
        assert_eq!(reached(&p, json!({"room": 41, "var:196": 2})), vec![0, 6, 7]);
        // the crew, at least k of three bits in any order; a cleared bit is absent (0)
        assert_eq!(reached(&p, json!({"room": 41, "bit:88": 1})), vec![0, 11]);
        assert_eq!(reached(&p, json!({"room": 41, "bit:76": 1, "bit:89": 1})), vec![0, 11, 12]);
        assert_eq!(reached(&p, json!({"room": 41, "bit:76": 1, "bit:89": 1, "bit:88": 1})), vec![0, 11, 12, 13]);
        // the store needs its three facts at once; the end its three owners
        assert_eq!(reached(&p, json!({"room": 30, "owner:396": 1, "owner:388": 1})), vec![0]);
        assert_eq!(reached(&p, json!({"room": 30, "owner:396": 1, "owner:388": 1, "bit:98": 1})), vec![0, 4]);
        assert_eq!(reached(&p, json!({"room": 83, "owner:901": 1, "owner:902": 1, "owner:903": 1})), vec![0, 14]);
    }

    #[test]
    fn absent_keys_and_ops() {
        let r = rungs_of_json(&json!({"rungs": [
            {"rung": "a", "all": [["bit:5", "=", 0]]},
            {"rung": "b", "all": [["var:3", "!=", 2]]},
            {"rung": "c", "all": [["var:3", "<", 2], ["var:4", ">", 1], ["var:4", "<=", 7]]},
            {"rung": "d", "all": [["owner:7", "=", "var:9"]]},
            {"rung": "e", "all": [["var:3", "in", [1, 9]]]}]})).unwrap();
        // a bit absent is 0 in a full state, unknown elsewhere; a var absent is unknown
        assert_eq!(reached(&r, json!({"room": 1, "var:3": 1, "var:4": 7})), vec![0, 1, 2, 4]);
        assert!(reached(&r, json!({"end": 1})).is_empty());
        assert_eq!(reached(&r, json!({"room": 1, "owner:7": 4, "var:9": 4, "var:3": 2})), vec![0, 3]);
        assert_eq!(reached(&r, json!({"room": 1, "owner:7": 4})), vec![0]);
        assert!(rungs_of_json(&json!({"rungs": [{"rung": "x", "all": [["var:1", "~", 1]]}]})).is_err());
        assert!(rungs_of_json(&json!({"rungs": [{"rung": "x", "least": 2, "of": [["var:1", "=", 1]]}]})).is_err());
        assert!(rungs_of_json(&json!({"rungs": [{"rung": "x", "all": [["var:1", "in", 1]]}]})).is_err());
    }

    #[test]
    fn the_score_is_the_most_rungs_on_one_chain_and_the_json_is_unchanged_without_it() {
        let d = std::env::temp_dir().join(format!("ladder-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("entries")).unwrap();
        // 0 the root; 1 and 2 its children, 3 a child of 1: the kitchen at 1, the map at 3, the money at 2
        let rows = [(0, None, vec![json!([10, {"room": 33}])]),
                    (1, Some(0), vec![json!([20, {"room": 41, "owner:566": 1}])]),
                    (2, Some(0), vec![json!([30, {"room": 35, "bit:72": 1}])]),
                    (3, Some(1), vec![json!([40, {"room": 35, "owner:566": 0, "bit:65": 1}])])];
        let mut text = String::new();
        for (id, parent, cps) in &rows {
            files::write_gz(&d.join("entries").join(format!("{id}.json.gz")), &json!({"id": id, "checkpoints": cps})).unwrap();
            text += &format!("{}\n", json!({"id": id, "parent": parent, "job": "j", "how": if *id == 0 { "boot" } else { "checkpoint" }, "room": 1}));
        }
        std::fs::write(d.join("entries.jsonl"), text).unwrap();
        let p = part1();
        let plain = run(&d, &p, &["ladder".into(), "--json".into(), d.join("a.json").display().to_string()]).unwrap();
        assert!(plain.contains("furthest P3 (in order P3)") && !plain.contains("score"));
        let a = files::read_json(&d.join("a.json")).unwrap();
        assert!(a.get("score").is_none() && a["rungs"][1].get("chain").is_none());
        let line = run(&d, &p, &["ladder".into(), "--score".into(), "--json".into(), d.join("b.json").display().to_string()]).unwrap();
        let b = files::read_json(&d.join("b.json")).unwrap();
        // entry 3's chain holds P0, P1 (at 1) and P3; entry 2's P0 and P2
        assert_eq!(b["score"], json!({"rungs": 3, "of": 15, "entry": 3, "held": ["P0", "P1", "P3"], "chain": [0, 1, 3]}));
        assert_eq!(b["rungs"][3]["chain"], json!([0, 1, 3]));
        assert_eq!(b["rungs"][2]["chain"], json!([0, 2]));
        assert!(line.ends_with("; score 3 of 15 rungs (entry 3's chain)"));
        // the same keys in the same order otherwise
        let mut b2 = b.clone();
        b2.as_object_mut().unwrap().remove("score");
        for r in b2["rungs"].as_array_mut().unwrap() {
            r.as_object_mut().unwrap().remove("chain");
        }
        assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b2).unwrap());
        let _ = std::fs::remove_dir_all(&d);
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
