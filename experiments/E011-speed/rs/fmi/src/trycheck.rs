//! E012 component try's gates (experiments/E012-game-mechanics/plan.md, amendment "component try"):
//!
//!   fmi check tries A_SEQ B_SEQ [--json OUT]
//!       G0: each try of B_SEQ (B_SEQ/jobs/J) against A_SEQ's (A_SEQ/jobs/J), as E011's cmp_tries.py compares them
//!       (the attempt's stop, target, frames, replans, inputs and refinements, every new entry's frame, tick, how,
//!       events and checkpoints' facts), and also the targets, probes, tried and exhausted. Exit 0 when every try
//!       is the same.
//!   fmi check verify-new SEQ --out DIR [--threads N]
//!       G-on: every new entry of SEQ's tries verified from boot in a fresh lean engine (fmi-engine's replay::verify):
//!       the movie from boot (the try's entry's events, then the new entry's), against the root's checkpoints
//!       (SEQ/entries/0.json.gz), the try's entry's checkpoint and the new entry's checkpoints. Exit 0 when none
//!       differs.
//!   fmi check try-parts SEQ... [--json OUT]
//!       E012 integration's measurement: each try's wall by part (E011's report.py rules, with T, the wait and the
//!       affordance step as parts of their own), the sums and the medians.

use fmi_core::{BootCfg, Checkpoint, Event, Key};
use fmi_explore::files::{self, read_gz, read_json};
use fmi_explore::mech::checkpoint_of_json;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn job_dirs(seq: &Path) -> Vec<PathBuf> {
    let mut ds: Vec<PathBuf> = std::fs::read_dir(seq.join("jobs"))
        .map(|r| r.flatten().map(|d| d.path()).filter(|p| p.join("result.json").exists()).collect())
        .unwrap_or_default();
    ds.sort();
    ds
}

fn stop_class(s: &Value) -> String {
    s.as_str().unwrap_or("").split(':').next().unwrap_or("").to_string()
}

/// A try's new entries as cmp_tries.py reads them: frame, tick, how, events, and each checkpoint's [frame, facts].
fn entries(d: &Path, res: &Value) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for n in res["new"].as_array().cloned().unwrap_or_default() {
        let e = read_gz(&d.join(n["file"].as_str().unwrap_or("")))?;
        let cps: Vec<Value> = e["checkpoints"].as_array().cloned().unwrap_or_default().iter().map(|c| json!([c[0], c[1]])).collect();
        out.push(json!({"frame": e["frame"], "tick": e.get("tick"), "how": n.get("how"), "events": e["events"], "checkpoints": cps}));
    }
    Ok(out)
}

fn pick(v: &Value, keys: &[&str]) -> Value {
    let mut m = serde_json::Map::new();
    for k in keys {
        m.insert((*k).into(), v.get(*k).cloned().unwrap_or(Value::Null));
    }
    Value::Object(m)
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 80 {
        format!("{}...", &s[..s.char_indices().nth(80).map(|x| x.0).unwrap_or(s.len())])
    } else {
        s
    }
}

/// One try of B against A's.
fn compare(a_dir: &Path, b_dir: &Path) -> Result<Value, String> {
    let (a, b) = (read_json(&a_dir.join("result.json"))?, read_json(&b_dir.join("result.json"))?);
    let empty = json!({});
    let (aa, ba) = (files::or(a.get("attempt"), &empty), files::or(b.get("attempt"), &empty));
    let mut diff: Vec<Value> = Vec::new();
    for k in ["stop", "target", "frames", "replans", "frame0", "frame_end", "qf", "inputs"] {
        if aa.get(k) != ba.get(k) {
            diff.push(json!([k, short(aa.get(k).unwrap_or(&Value::Null)), short(ba.get(k).unwrap_or(&Value::Null))]));
        }
    }
    let added = |x: &Value| -> Value {
        json!(x["refinements"].as_array().cloned().unwrap_or_default().iter().map(|r| r.get("added").cloned().unwrap_or(Value::Null)).collect::<Vec<_>>())
    };
    if added(aa) != added(ba) {
        diff.push(json!(["refinements", added(aa), added(ba)]));
    }
    let probe = ["target", "tier", "depth", "status", "stop", "attempt"];
    let probes = |x: &Value| json!(x["probes"].as_array().cloned().unwrap_or_default().iter().map(|p| pick(p, &probe)).collect::<Vec<_>>());
    for (k, va, vb) in [
        ("targets", a["targets"].clone(), b["targets"].clone()),
        ("probes", probes(&a), probes(&b)),
        ("tried", a["tried"].clone(), b["tried"].clone()),
        ("exhausted", a["exhausted"].clone(), b["exhausted"].clone()),
        ("ok", a["ok"].clone(), b["ok"].clone()),
    ] {
        if va != vb {
            diff.push(json!([k, short(&va), short(&vb)]));
        }
    }
    let (ea, eb) = (entries(a_dir, &a)?, entries(b_dir, &b)?);
    if ea.len() != eb.len() {
        diff.push(json!(["new entries", ea.len(), eb.len()]));
    }
    for (i, (x, y)) in ea.iter().zip(eb.iter()).enumerate() {
        for k in ["frame", "tick", "how", "events", "checkpoints"] {
            if x[k] != y[k] {
                diff.push(json!([format!("new {i} {k}"), short(&x[k]), short(&y[k])]));
            }
        }
    }
    let class = [stop_class(&aa["stop"]), stop_class(&ba["stop"])];
    Ok(json!({"job": b["job"], "class": class, "same_class": class[0] == class[1],
              "new": eb.iter().map(|e| e["frame"].clone()).collect::<Vec<_>>(), "differ": diff}))
}

pub fn cmd_tries(a: &Path, b: &Path, out: Option<&Path>) -> Result<bool, String> {
    let mut rows = Vec::new();
    for d in job_dirs(b) {
        let name = d.file_name().unwrap().to_string_lossy().to_string();
        let r = compare(&a.join("jobs").join(&name), &d)?;
        let same = r["differ"].as_array().map(|x| x.is_empty()).unwrap_or(false);
        println!("{name}: class {} ({} A's), new {}, {}", r["class"][1], if r["same_class"] == json!(true) { "=" } else { "!=" },
                 r["new"], if same { "same as A".to_string() } else { format!("differs: {}", short(&r["differ"])) });
        rows.push(r);
    }
    let n_same = rows.iter().filter(|r| r["differ"].as_array().map(|x| x.is_empty()).unwrap_or(false)).count();
    let n_class = rows.iter().filter(|r| r["same_class"] == json!(true)).count();
    println!("G0 {}: {} tries, stop class = A's in {n_class}, everything = A's in {n_same}", b.display(), rows.len());
    if let Some(o) = out {
        files::write_json(o, &json!(rows), Some(1))?;
    }
    Ok(!rows.is_empty() && n_same == rows.len())
}

fn cps_of(v: &Value) -> Result<Vec<Checkpoint>, String> {
    v.as_array().cloned().unwrap_or_default().iter().map(|c| checkpoint_of_json(c).ok_or_else(|| format!("a checkpoint: {}", short(c)))).collect()
}

fn events_of(v: &Value) -> Vec<Event> {
    v.as_array().cloned().unwrap_or_default().iter().filter_map(fmi_explore::events::event_of).collect()
}

/// One entry to verify: its name, its movie from boot and its checkpoints in frame order.
struct ToVerify {
    name: String,
    how: Value,
    events: Vec<Event>,
    cps: Vec<Checkpoint>,
}

pub fn cmd_verify_new(seq: &Path, out: &Path, threads: usize) -> Result<bool, String> {
    let root = read_gz(&seq.join("entries").join("0.json.gz"))?;
    let root_cps = cps_of(&root["checkpoints"])?;
    let mut todo: Vec<ToVerify> = Vec::new();
    for d in job_dirs(seq) {
        let res = read_json(&d.join("result.json"))?;
        let job = read_json(&d.join("job.json"))?;
        let ent = &job["entry"];
        let mut base: Vec<Checkpoint> = root_cps.clone();
        if !files::truthy(ent.get("root")) {
            base.push(checkpoint_of_json(&ent["checkpoint"]).ok_or("the try's entry's checkpoint")?);
        }
        for (k, n) in res["new"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            let e = read_gz(&d.join(n["file"].as_str().unwrap_or("")))?;
            let mut events = events_of(&ent["events"]);
            events.extend(events_of(&e["events"]));
            let mut cps = base.clone();
            cps.extend(cps_of(&e["checkpoints"])?);
            cps.sort_by_key(|c| c.frame);
            cps.dedup_by_key(|c| c.frame);
            todo.push(ToVerify { name: format!("{}-{k}", d.file_name().unwrap().to_string_lossy()), how: n.get("how").cloned().unwrap_or(Value::Null), events, cps });
        }
    }
    std::fs::create_dir_all(out).map_err(|e| files::io_err(out, e))?;
    let host = fmi_engine::Host::from_env().map_err(|e| format!("the engine host: {e}"))?;
    let next = Mutex::new(0usize);
    let rows: Mutex<Vec<(usize, Value)>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| loop {
                let i = {
                    let mut g = next.lock().unwrap();
                    let i = *g;
                    *g += 1;
                    i
                };
                let Some(t) = todo.get(i) else { break };
                let run = out.join(format!("run-{}", t.name));
                let base = BootCfg { seed: 1, events: vec![], frame: None, dump_locals: true, lean: true, out_dir: run.clone() };
                let r = fmi_engine::replay::verify(&host, &base, &t.events, &t.cps, &[Key::state(316), Key::owner(566)]);
                let _ = std::fs::remove_dir_all(&run);
                let row = match r {
                    Ok(v) => json!({"entry": t.name, "how": t.how, "checkpoints": v.checkpoints, "inputs": v.inputs, "differ": v.differ.len(),
                                    "first": v.differ.first().map(|d| format!("{d:?}")), "same": v.same, "seconds": v.seconds}),
                    Err(e) => json!({"entry": t.name, "how": t.how, "error": e.to_string(), "same": false}),
                };
                rows.lock().unwrap().push((i, row));
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort_by_key(|r| r.0);
    let rows: Vec<Value> = rows.into_iter().map(|r| r.1).collect();
    let bad: Vec<&Value> = rows.iter().filter(|r| r["same"] != json!(true)).collect();
    for r in &bad {
        println!("{}: DIFFERS {}", r["entry"], short(r));
    }
    let mut by_how: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for r in &rows {
        let e = by_how.entry(r["how"].as_str().unwrap_or("?").to_string()).or_default();
        e.0 += 1;
        e.1 += (r["same"] == json!(true)) as usize;
    }
    files::write_json(&out.join("verify.json"), &json!(rows), Some(1))?;
    println!("verify-new {}: {} new entries from boot, {} with differences; by how {}", seq.display(), rows.len(), bad.len(),
             by_how.iter().map(|(k, (n, s))| format!("{k} {s}/{n}")).collect::<Vec<_>>().join(", "));
    Ok(bad.is_empty())
}

fn f64_of(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

/// `fmi check try-report SEQ...`: per sequence and try, the wall and CPU by part and E012's counts, as a Markdown table,
/// and the sequence's sums (the try component's measurements).
pub fn cmd_report(seqs: &[PathBuf]) -> Result<(), String> {
    for seq in seqs {
        println!("\n### {}\n", seq.display());
        println!("| try | entry | wall s | return | read | idle+wait | plan | play | afford | stop | new (how) | wait stops/kept | delay trig/cand/kept | afford trials/kept | dead |");
        println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---|---|---|---|---|---|");
        let mut sums = [0.0f64; 7];
        let mut counts = [0usize; 9];
        for d in job_dirs(seq) {
            let r = read_json(&d.join("result.json"))?;
            let c = &r["cpu_s"];
            let parts = [f64_of(&r["wall_s"]), f64_of(&c["return"]), f64_of(&c["read"]), f64_of(&c["idle"]) + f64_of(&c["wait"]), f64_of(&c["plan"]), f64_of(&c["play"]),
                         f64_of(&c["afford"])];
            for (s, p) in sums.iter_mut().zip(parts.iter()) {
                *s += p;
            }
            let mut hows: std::collections::BTreeMap<String, usize> = Default::default();
            for n in r["new"].as_array().cloned().unwrap_or_default() {
                *hows.entry(n["how"].as_str().unwrap_or("?").to_string()).or_default() += 1;
            }
            let e = &r["e012"];
            let w = &e["wait"];
            let (ws, wk) = (w["stops"].as_array().map(|a| a.len()).unwrap_or(0), w["kept"].as_u64().unwrap_or(0) as usize);
            let dl = &e["delay"];
            let (dt, dc, dk) = (dl["triggered"].as_u64().unwrap_or(0) as usize, dl["candidates"].as_u64().unwrap_or(0) as usize,
                                dl["kept"].as_u64().unwrap_or(0) as usize);
            let afs = e["afford"].as_array().cloned().unwrap_or_default();
            let at: usize = afs.iter().map(|a| a["trials"].as_u64().unwrap_or(0) as usize).sum();
            let ak: usize = afs.iter().map(|a| a["kept"].as_u64().unwrap_or(0) as usize).sum();
            let dead = r["dead"].as_array().map(|a| a.len()).unwrap_or(0);
            for (s, p) in counts.iter_mut().zip([ws, wk, dt, dc, dk, afs.len(), at, ak, dead]) {
                *s += p;
            }
            let stop = stop_class(&r["attempt"]["stop"]);
            println!("| {} | {} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {} | {} {} | {ws}/{wk} | {dt}/{dc}/{dk} | {at}/{ak} | {dead} |",
                     d.file_name().unwrap().to_string_lossy(), r["entry"], parts[0], parts[1], parts[2], parts[3], parts[4], parts[5], parts[6],
                     if stop.is_empty() { "(none played)".into() } else { stop }, r["new"].as_array().map(|a| a.len()).unwrap_or(0),
                     json!(hows));
        }
        println!("| sum | | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | | | {}/{} | {}/{}/{} | {} steps, {}/{} | {} |",
                 sums[0], sums[1], sums[2], sums[3], sums[4], sums[5], sums[6], counts[0], counts[1], counts[2], counts[3], counts[4],
                 counts[5], counts[6], counts[7], counts[8]);
    }
    Ok(())
}

/// The wall of one try by part, by E011's report.py rules (B5), with E012's steps as parts of their own:
/// return `return.boot_s`; read the fresh reading's `reading.wall_s` (a reused one's `load_s`) plus the refinements' and
/// extensions' `seconds`; T `e012.t.seconds`; idle `idle.seconds`, or with the wait on `e012.wait.seconds` (the wait
/// replaces the idle measurement); play the window records' `wall_s` (run/a*/run.json); plan the probes' `seconds` less
/// play and the refinements'; afford the affordance steps' `step_seconds`; rest the try's `wall_s` less the parts.
pub const PARTS: [&str; 8] = ["return", "read", "T", "idle/wait", "plan", "play", "afford", "rest"];

fn wall_parts(d: &Path, r: &Value) -> [f64; 9] {
    let a = &r["attempt"];
    let rd = &r["reading"];
    let sub: f64 = ["refinements", "extensions"]
        .iter()
        .flat_map(|k| a[*k].as_array().cloned().unwrap_or_default())
        .map(|x| f64_of(&x["seconds"]))
        .sum();
    let mut play = 0.0;
    if let Ok(rs) = std::fs::read_dir(d.join("run")) {
        let mut ps: Vec<PathBuf> = rs.flatten().map(|e| e.path().join("run.json")).filter(|p| p.exists()).collect();
        ps.sort();
        for p in ps {
            if let Ok(v) = read_json(&p) {
                play += v["windows"].as_array().map(|w| w.iter().map(|x| f64_of(&x["wall_s"])).sum::<f64>()).unwrap_or(0.0);
            }
        }
    }
    let probes: f64 = r["probes"].as_array().map(|p| p.iter().map(|x| f64_of(&x["seconds"])).sum()).unwrap_or(0.0);
    let read = if rd["fresh"].as_bool().unwrap_or(false) { f64_of(&rd["wall_s"]) } else { f64_of(&rd["load_s"]) } + sub;
    let e = &r["e012"];
    let idle = if e["wait"].is_object() { f64_of(&e["wait"]["seconds"]) } else { f64_of(&r["idle"]["seconds"]) };
    let afford: f64 = e["afford"].as_array().map(|a| a.iter().map(|x| f64_of(&x["step_seconds"])).sum()).unwrap_or(0.0);
    let t = f64_of(&e["t"]["seconds"]);
    let plan = (probes - play - sub).max(0.0);
    let ret = f64_of(&r["return"]["boot_s"]);
    let wall = f64_of(&r["wall_s"]);
    let rest = (wall - (ret + read + t + idle + plan + play + afford)).max(0.0);
    [ret, read, t, idle, plan, play, afford, rest, wall]
}

fn median_of(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    match s.len() {
        0 => f64::NAN,
        n if n % 2 == 1 => s[n / 2],
        n => (s[n / 2 - 1] + s[n / 2]) / 2.0,
    }
}

/// `fmi check try-parts SEQ... [--json OUT]`: per sequence, each try's wall by part (`wall_parts`), then the sums and
/// the medians over the tries, the ring(33) tries (a 4-room scope) and the ring(38) tries (5 rooms).
pub fn cmd_parts(seqs: &[PathBuf], json_out: Option<&Path>) -> Result<(), String> {
    let mut all = serde_json::Map::new();
    for seq in seqs {
        println!("\n### {}\n", seq.display());
        println!("| try | entry | scope | wall s | {} |", PARTS.join(" | "));
        println!("|---|---|---:|---:|{}", "---:|".repeat(PARTS.len()));
        let mut rows: Vec<(usize, [f64; 9])> = Vec::new();
        let mut js = Vec::new();
        for d in job_dirs(seq) {
            let r = read_json(&d.join("result.json"))?;
            let p = wall_parts(&d, &r);
            let scope = r["reading"]["scope"].as_array().map(|a| a.len()).unwrap_or(0);
            println!("| {} | {} | {} | {:.2} | {} |", d.file_name().unwrap().to_string_lossy(), r["entry"], scope, p[8],
                     p[..8].iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(" | "));
            let mut m = serde_json::Map::new();
            m.insert("job".into(), json!(d.file_name().unwrap().to_string_lossy()));
            m.insert("scope".into(), json!(scope));
            m.insert("wall".into(), json!(p[8]));
            for (k, v) in PARTS.iter().zip(p.iter()) {
                m.insert((*k).into(), json!(v));
            }
            js.push(Value::Object(m));
            rows.push((scope, p));
        }
        let col = |i: usize, f: &dyn Fn(usize) -> bool| -> Vec<f64> { rows.iter().filter(|r| f(r.0)).map(|r| r.1[i]).collect() };
        let sum: Vec<f64> = (0..9).map(|i| col(i, &|_| true).iter().sum()).collect();
        println!("| sum | | | {:.2} | {} |", sum[8], sum[..8].iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(" | "));
        let mut stats = serde_json::Map::new();
        for (name, f) in [("all", &(|_| true) as &dyn Fn(usize) -> bool), ("ring33", &|s| s == 4), ("ring38", &|s| s == 5)] {
            let med: Vec<f64> = (0..9).map(|i| median_of(&col(i, f))).collect();
            let n = col(8, f).len();
            println!("| median, {name} ({n}) | | | {:.2} | {} |", med[8], med[..8].iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(" | "));
            let mut m = serde_json::Map::new();
            m.insert("n".into(), json!(n));
            m.insert("wall".into(), json!(med[8]));
            for (k, v) in PARTS.iter().zip(med.iter()) {
                m.insert((*k).into(), json!(v));
            }
            stats.insert(format!("median_{name}"), Value::Object(m));
        }
        let w = col(8, &|_| true);
        let mean = if w.is_empty() { f64::NAN } else { sum[8] / w.len() as f64 };
        let max = w.iter().cloned().fold(f64::NAN, f64::max);
        println!("\nwall: sum {:.1} s, mean {:.2} s, max {:.1} s", sum[8], mean, max);
        let mut sm = serde_json::Map::new();
        sm.insert("wall".into(), json!(sum[8]));
        for (k, v) in PARTS.iter().zip(sum.iter()) {
            sm.insert((*k).into(), json!(v));
        }
        stats.insert("sum".into(), Value::Object(sm));
        stats.insert("mean".into(), json!(mean));
        stats.insert("max".into(), json!(max));
        stats.insert("tries".into(), Value::Array(js));
        all.insert(seq.display().to_string(), Value::Object(stats));
    }
    if let Some(p) = json_out {
        files::write_json(p, &Value::Object(all), Some(1))?;
    }
    Ok(())
}
