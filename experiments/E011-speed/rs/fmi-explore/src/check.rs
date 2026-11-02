//! Differential checks against the frozen reference's recorded outputs (G9's step part): step.py's `targets` (with
//! E009's rotation), `reusable`/`reusable_detail` and `idle_rule`, case by case from the fixtures that
//! experiments/E011-speed/explore/step_fixtures.py writes.

use crate::archive::fact_map;
use crate::files::{self, read_gz, read_json, Res};
use crate::pyjson;
use crate::step;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

pub struct CheckReport {
    pub cases: usize,
    pub by_kind: Vec<(String, usize, usize)>,
    pub first_bad: Vec<String>,
    /// seconds spent in the Rust functions alone, by kind
    pub seconds: Vec<(String, f64)>,
}

fn bump(v: &mut Vec<(String, usize, usize)>, k: &str, bad: bool) {
    if let Some(x) = v.iter_mut().find(|x| x.0 == k) {
        x.1 += 1;
        x.2 += bad as usize;
    } else {
        v.push((k.to_string(), 1, bad as usize));
    }
}

fn add_s(v: &mut Vec<(String, f64)>, k: &str, s: f64) {
    if let Some(x) = v.iter_mut().find(|x| x.0 == k) {
        x.1 += s;
    } else {
        v.push((k.to_string(), s));
    }
}

/// Maps compared as maps (Python's dict order of `idle_rule`'s maps follows its set order, which varies by run).
fn same_map(a: &Value, b: &Value) -> bool {
    match (a.as_object(), b.as_object()) {
        (Some(x), Some(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k) == Some(v)),
        _ => a == b,
    }
}

pub fn step_check(path: &Path) -> Res<CheckReport> {
    let text = files::read_text(path)?;
    let mut rep = CheckReport { cases: 0, by_kind: Vec::new(), first_bad: Vec::new(), seconds: Vec::new() };
    let mut guides: std::collections::HashMap<String, Value> = Default::default();
    let mut reached_cache: std::collections::HashMap<String, Vec<(String, Vec<i64>)>> = Default::default();
    for (n, ln) in files::py_splitlines(&text).into_iter().enumerate() {
        if ln.trim().is_empty() {
            continue;
        }
        let c = pyjson::loads(ln).map_err(|e| format!("{}: line {}: {e}", path.display(), n + 1))?;
        let kind = c["kind"].as_str().unwrap_or("").to_string();
        rep.cases += 1;
        let (ok, what) = match kind.as_str() {
            "targets" => {
                let jd = Path::new(c["job_dir"].as_str().unwrap_or(""));
                let job = read_json(&jd.join("job.json"))?;
                let gp = job["guide"].as_str().unwrap_or("").to_string();
                if !guides.contains_key(&gp) {
                    guides.insert(gp.clone(), read_json(Path::new(&gp))?);
                }
                let rp = c["reached"].as_str().unwrap_or("").to_string();
                if !reached_cache.contains_key(&rp) {
                    reached_cache.insert(rp.clone(), step::read_reached(Path::new(&rp))?);
                }
                let t = Instant::now();
                let facts = fact_map(&job["entry"]["checkpoint"][1]);
                let root = step::root_facts_of(&job, jd);
                let goal = step::goal_of(&job);
                let empty = vec![];
                let held = job.get("held").and_then(|h| h.as_array()).unwrap_or(&empty);
                let tried = job.get("tried").and_then(|h| h.as_array()).unwrap_or(&empty);
                let rot = c.get("rotation").filter(|r| !r.is_null());
                let ts = step::targets(&reached_cache[&rp], &guides[&gp], &facts, root.as_ref(), held, tried, &goal, rot)?;
                add_s(&mut rep.seconds, &kind, t.elapsed().as_secs_f64());
                let got = Value::Array(ts.iter().map(|x| x.json()).collect());
                let root_ok = root.is_some() == c["root_facts"].as_bool().unwrap_or(false);
                (got == c["out"] && root_ok, format!("{} on {}: {} targets, reference {}", jd.display(), rp,
                                                      ts.len(), c["out"].as_array().map(|a| a.len()).unwrap_or(0)))
            }
            "reusable" => {
                let rd = read_json(Path::new(c["reading"].as_str().unwrap_or("")))?;
                let keys: HashSet<String> = read_json(Path::new(c["task_keys"].as_str().unwrap_or("")))?
                    .as_array().map(|a| a.iter().filter_map(|k| k.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
                let job = read_json(&Path::new(c["job_dir"].as_str().unwrap_or("")).join("job.json"))?;
                let t = Instant::now();
                let start = fact_map(&rd["start"]["facts"]);
                let facts = fact_map(&job["entry"]["checkpoint"][1]);
                let (ok, nb) = step::reusable(&start, &keys, &facts);
                let det = step::reusable_detail(&start, &keys, &facts);
                add_s(&mut rep.seconds, &kind, t.elapsed().as_secs_f64());
                (json!([ok, nb]) == c["out"] && det == c["detail"], format!("{} vs {}", c["reading"], c["job_dir"]))
            }
            "idle_rule" => {
                let e = read_gz(Path::new(c["entry_file"].as_str().unwrap_or("")))?;
                let cps = e["checkpoints"].as_array().cloned().unwrap_or_default();
                let t = Instant::now();
                let f0 = cps[0][0].as_i64().unwrap_or(0);
                let start = fact_map(&cps[0][1]);
                let frames: Vec<(i64, crate::archive::FactMap)> = cps[1..].iter().map(|x| (x[0].as_i64().unwrap_or(0) - f0, fact_map(&x[1]))).collect();
                let r = step::idle_rule(&start, frames);
                add_s(&mut rep.seconds, &kind, t.elapsed().as_secs_f64());
                let o = &c["out"];
                (r["returns"] == o["returns"] && same_map(&r["first_return"], &o["first_return"]) && same_map(&r["changes"], &o["changes"]),
                 format!("{}", c["entry_file"]))
            }
            k => (false, format!("unknown kind {k}")),
        };
        bump(&mut rep.by_kind, &kind, !ok);
        if !ok && rep.first_bad.len() < 10 {
            rep.first_bad.push(format!("line {}: {kind}: {what}", n + 1));
        }
    }
    Ok(rep)
}

/// A reading's domains as the fixtures write them (key -> [values, low, high]; "below"/"above" as text).
pub fn domains_of_fixture(v: &Value) -> crate::files::Res<fmi_core::Domains> {
    let mut out = Vec::new();
    for (k, d) in v.as_object().ok_or("domains: not an object")? {
        let mut values: Vec<fmi_core::Val> = d[0].as_array().cloned().unwrap_or_default().iter().map(|x| match x {
            Value::String(s) if s == "below" => fmi_core::BELOW,
            Value::String(s) if s == "above" => fmi_core::ABOVE,
            x => x.as_i64().unwrap_or(fmi_core::NONE),
        }).collect();
        values.sort();
        out.push((fmi_core::Key::parse(k), fmi_core::Domain { values, low: d[1].as_i64(), high: d[2].as_i64() }));
    }
    out.sort_by_key(|p| p.0);
    Ok(fmi_core::Domains(out))
}

fn counts_json(c: &std::collections::BTreeMap<String, i64>) -> Value {
    let mut m = serde_json::Map::new();
    for (k, v) in c {
        m.insert(k.clone(), json!(v));
    }
    Value::Object(m)
}

/// The harness's start rules against the reference's outputs (experiments/E011-speed/explore/harness_fixtures.py).
pub fn harness_check(path: &Path) -> Res<CheckReport> {
    use crate::start;
    let text = files::read_text(path)?;
    let mut rep = CheckReport { cases: 0, by_kind: Vec::new(), first_bad: Vec::new(), seconds: Vec::new() };
    let mut doms_cache: std::collections::HashMap<String, fmi_core::Domains> = Default::default();
    let mut live_cache: std::collections::HashMap<String, Option<Value>> = Default::default();
    for (n, ln) in files::py_splitlines(&text).into_iter().enumerate() {
        if ln.trim().is_empty() {
            continue;
        }
        let c = pyjson::loads(ln).map_err(|e| format!("{}: line {}: {e}", path.display(), n + 1))?;
        let kind = c["kind"].as_str().unwrap_or("").to_string();
        rep.cases += 1;
        let df = c["domains"].as_str().unwrap_or("").to_string();
        if !doms_cache.contains_key(&df) {
            let d = read_json(Path::new(&df))?;
            doms_cache.insert(df.clone(), domains_of_fixture(&d["domains"])?);
            live_cache.insert(df.clone(), d.get("live_locals").filter(|x| !x.is_null()).cloned());
        }
        let rp = df.clone();
        let doms = &doms_cache[&df];
        let job = read_json(&Path::new(c["job_dir"].as_str().unwrap_or("")).join("job.json"))?;
        let obs = start::facts_of_json(&job["entry"]["checkpoint"][1]);
        let frame = c["frame"].as_u64().unwrap_or(0);
        let unob = start::unobserved(true);
        let t = Instant::now();
        let mut b = start::Points::of(doms, live_cache[&rp].clone());
        let (ok, what) = match kind.as_str() {
            "first" => {
                let mut st = start::task_start(&mut b, &obs, doms);
                start::actor_rooms_start(&mut st, doms, &fmi_core::Facts::new(), true);
                let now = start::view(&mut b, doms, &obs, &unob);
                let text = start::start_text(doms, &st);
                let key = start::input_key(frame, 0, &st);
                let view = start::facts_json(&now);
                let okv = same_map(&view, &c["view"]);
                let okc = same_map(&counts_json(&b.counts), &c["counts"]);
                (text == c["start_text"].as_str().unwrap_or("") && key == c["input_key"].as_str().unwrap_or("") && okv && okc,
                 format!("{} at {frame}: text {}, key {}, view {okv}, counts {okc}", c["job_dir"], text == c["start_text"].as_str().unwrap_or(""),
                         key == c["input_key"].as_str().unwrap_or("")))
            }
            "replan" => {
                let pred = start::facts_of_json(&c["pred"]);
                let now = start::view(&mut b, doms, &obs, &unob);
                let (mut st, counts) = start::replan_start(&mut b, &obs, &start::prediction(&now, &pred), doms);
                start::actor_rooms_start(&mut st, doms, &pred, false);
                let text = start::start_text(doms, &st);
                let key = start::input_key(frame, 0, &st);
                let okc = same_map(&counts_json(&counts), &c["counts"]);
                (text == c["start_text"].as_str().unwrap_or("") && key == c["input_key"].as_str().unwrap_or("") && okc,
                 format!("{} at {frame}: text {}, key {}, counts {okc}", c["job_dir"], text == c["start_text"].as_str().unwrap_or(""),
                         key == c["input_key"].as_str().unwrap_or("")))
            }
            k => (false, format!("unknown kind {k}")),
        };
        add_s(&mut rep.seconds, &kind, t.elapsed().as_secs_f64());
        bump(&mut rep.by_kind, &kind, !ok);
        if !ok && rep.first_bad.len() < 10 {
            rep.first_bad.push(format!("line {}: {kind}: {what}", n + 1));
        }
    }
    Ok(rep)
}
