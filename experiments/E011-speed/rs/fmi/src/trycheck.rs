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
