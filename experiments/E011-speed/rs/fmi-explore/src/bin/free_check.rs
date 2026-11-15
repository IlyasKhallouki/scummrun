//! E013 component `free`'s checks (experiments/E013-robust/plan.md, free's amendment, "Gates as run"):
//!
//! * `free_check same A B`: D2. Two archives of the same driver run (E012's smoke, b = 1) made by two builds: the same
//!   jobs, and for each its job.json, select.json and result.json, then entries.jsonl, every entry's file, cells.json
//!   and log.jsonl, the same but for the timing fields (keys named for seconds, CPU, memory, wall clock or a process)
//!   and the paths under each run's own folder (written as <RUN>), and the bytes of a probe's dropped folders (their
//!   reports hold timings). Prints each difference; exit 1 when any.
//! * `free_check report A`: D3. Per free try: its entry and frame, offer points, trials (by verdict when the trials
//!   carry one), waits run and reused, delays, kept by level, dead moves, its wall and the free step's seconds; per run
//!   the kinds and the free entries. JSON lines on stdout after a table.

use fmi_explore::files::{read_gz, read_json, read_jsonl};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Keys whose values are timings, CPU, memory, wall-clock times or process ids: not compared.
const TIMING: [&str; 16] = ["seconds", "time", "started", "ended", "rusage", "guard", "pid", "rc", "cpu_s", "peak_rss_mb",
                             "phases_s", "ms_per_trial", "cpu_ms_per_trial", "explore_s", "parts", "maxrss_mb"];

fn timing_key(k: &str) -> bool {
    TIMING.contains(&k) || k.ends_with("_s") || k.ends_with("_seconds") || k.ends_with("_mb") || k.starts_with("cpu")
}

/// The value with its timing keys left out and the run's folder written as <RUN> in every string. A probe's
/// "dropped" ([[folder, bytes freed]]) keeps its folders only: the bytes count the dropped readings' reports and logs
/// (reader-scope.err, refine-NN.json, reading.json, cache.json), whose text holds their timings, loads and memory.
fn norm(v: &Value, run: &str) -> Value {
    match v {
        Value::Object(m) => {
            let mut o = Map::new();
            for (k, x) in m {
                if k == "dropped" {
                    let names: Vec<Value> = x.as_array().map(|a| a.iter().map(|d| d.get(0).cloned().unwrap_or(Value::Null)).collect()).unwrap_or_default();
                    o.insert(k.clone(), Value::Array(names));
                } else if !timing_key(k) {
                    o.insert(k.clone(), norm(x, run));
                }
            }
            Value::Object(o)
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| norm(x, run)).collect()),
        Value::String(s) if !run.is_empty() && s.contains(run) => Value::String(s.replace(run, "<RUN>")),
        x => x.clone(),
    }
}

/// The first differences between two values, as paths (at most `left` of them).
fn diff(a: &Value, b: &Value, path: &str, out: &mut Vec<String>, left: usize) {
    if out.len() >= left {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for k in keys {
                match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) => diff(p, q, &format!("{path}.{k}"), out, left),
                    (p, q) => out.push(format!("{path}.{k}: {} | {}", short(p), short(q))),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                diff(p, q, &format!("{path}[{i}]"), out, left);
            }
        }
        _ if a == b => {}
        _ => out.push(format!("{path}: {} | {}", short(Some(a)), short(Some(b)))),
    }
}

fn short(v: Option<&Value>) -> String {
    let s = v.map(|v| v.to_string()).unwrap_or("(absent)".into());
    if s.len() > 160 {
        format!("{}...", &s[..160])
    } else {
        s
    }
}

/// A run's folder: the archive's parent (smoke.sh's OUT, which holds the reading cache, the pool and the plans).
fn run_dir(a: &Path) -> String {
    let a = std::fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    a.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

fn jobs(a: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(a.join("jobs"))
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with('j')).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn load(p: &Path, gz: bool) -> Value {
    if gz {
        read_gz(p).unwrap_or(Value::Null)
    } else if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
        read_jsonl(p).map(|(v, _)| Value::Array(v)).unwrap_or(Value::Null)
    } else {
        read_json(p).unwrap_or(Value::Null)
    }
}

fn same(a: &Path, b: &Path) -> i32 {
    let (ra, rb) = (run_dir(a), run_dir(b));
    let mut bad = 0;
    let mut files: Vec<(PathBuf, bool)> = Vec::new();
    let (ja, jb) = (jobs(a), jobs(b));
    if ja != jb {
        println!("jobs: {ja:?} | {jb:?}");
        bad += 1;
    }
    for j in &ja {
        for f in ["job.json", "select.json", "result.json"] {
            files.push((PathBuf::from("jobs").join(j).join(f), false));
        }
    }
    files.push((PathBuf::from("entries.jsonl"), false));
    let n = load(&a.join("entries.jsonl"), false).as_array().map(|x| x.len()).unwrap_or(0);
    for i in 0..n {
        files.push((PathBuf::from("entries").join(format!("{i}.json.gz")), true));
    }
    files.push((PathBuf::from("cells.json"), false));
    files.push((PathBuf::from("log.jsonl"), false));
    let mut compared = 0;
    for (f, gz) in &files {
        let (x, y) = (norm(&load(&a.join(f), *gz), &ra), norm(&load(&b.join(f), *gz), &rb));
        compared += 1;
        if x.is_null() && y.is_null() {
            println!("{}: in neither", f.display());
            bad += 1;
            continue;
        }
        let mut d = Vec::new();
        diff(&x, &y, "", &mut d, 12);
        if !d.is_empty() {
            bad += 1;
            println!("{}: {} differences", f.display(), d.len());
            for l in d {
                println!("  {l}");
            }
        }
    }
    println!("compared {compared} files ({} jobs, {n} entries): {bad} differ", ja.len());
    if bad == 0 {
        0
    } else {
        1
    }
}

fn report(a: &Path) -> i32 {
    let rows = load(&a.join("entries.jsonl"), false);
    let rows = rows.as_array().cloned().unwrap_or_default();
    let mut lines = Vec::new();
    let (mut n_free, mut n_model, mut n_none, mut kept_free) = (0, 0, 0, 0);
    println!("| job | entry | frame | offers | trials | verdicts | waits run / reused | delays (counted, more, trials, kept) | kept 1 / delay / 2 | dead | free s | wall s |");
    println!("|---|---:|---:|---:|---:|---|---|---|---|---:|---:|---:|");
    for j in jobs(a) {
        let res = load(&a.join("jobs").join(&j).join("result.json"), false);
        match res.get("kind").and_then(|k| k.as_str()) {
            Some("free") => n_free += 1,
            Some(_) => n_model += 1,
            None => n_none += 1,
        }
        let Some(f) = res.get("free") else { continue };
        let e = res["entry"].as_u64().unwrap_or(0) as usize;
        let frame = rows.get(e).and_then(|r| r.get("frame")).cloned().unwrap_or(Value::Null);
        let k = &f["kept"];
        let d = &f["delays"];
        let verdicts = f.get("verdicts").map(|v| v.to_string()).unwrap_or("-".into());
        println!("| {j} | {e} | {frame} | {} | {} | {verdicts} | {} / {} | {}, {}, {}, {} | {} / {} / {} | {} | {:.1} | {} |",
                 f["offers"], f["trials"], f["waits_run"], f["waits_reused"], d["counted"], d["delayed_inputs"], d["trials"],
                 d["kept"], k.get("1").unwrap_or(&json!(0)), k.get("delay").unwrap_or(&json!(0)), k.get("2").unwrap_or(&json!(0)),
                 f["dead"], f["seconds"].as_f64().unwrap_or(0.0), res["wall_s"]);
        kept_free += f["kept_total"].as_u64().unwrap_or(0);
        lines.push(json!({"job": j, "entry": e, "frame": frame, "offers": f["offers"], "points": f["n_points"], "trials": f["trials"],
                          "verdicts": f.get("verdicts"), "waits_run": f["waits_run"], "waits_reused": f["waits_reused"],
                          "delays": d, "kept": k, "kept_total": f["kept_total"], "dead": f["dead"],
                          "free_s": f["seconds"], "wall_s": res["wall_s"], "new": res["new"].as_array().map(|x| x.len())}));
    }
    let free_rows = rows.iter().filter(|r| r.get("how").and_then(|h| h.as_str()) == Some("free")).count();
    println!();
    println!("tries: {n_free} free, {n_model} model, {n_none} with no kind; entries kept by free tries {kept_free}, rows how free {free_rows}");
    for l in lines {
        println!("{l}");
    }
    0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rc = match args.first().map(|s| s.as_str()) {
        Some("same") if args.len() == 3 => same(Path::new(&args[1]), Path::new(&args[2])),
        Some("report") if args.len() == 2 => report(Path::new(&args[1])),
        _ => {
            eprintln!("usage: free_check same ARCHIVE_A ARCHIVE_B | free_check report ARCHIVE");
            2
        }
    };
    std::process::exit(rc);
}
