//! Differential checks against the frozen reference's recorded outputs (G9's step part): step.py's `targets` (with
//! E009's rotation), `reusable`/`reusable_detail` and `idle_rule`, case by case from the fixtures that
//! experiments/E011-speed/explore/step_fixtures.py writes.

use crate::archive::fact_map;
use crate::files::{self, read_gz, read_json, Res};
use crate::pyjson;
use crate::step;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
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
                // the try's memo of the root's facts (E011 round 2): the same facts, the same targets
                let root_m = step::root_facts_of_memo(&job, jd);
                let ts_m = step::targets(&reached_cache[&rp], &guides[&gp], &facts, root_m.as_deref(), held, tried, &goal, rot)?;
                let root_ok = root.is_some() == c["root_facts"].as_bool().unwrap_or(false) && root.as_ref() == root_m.as_deref() && ts_m == ts;
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
                // the try's one diff (E011 round 2)
                let (ok1, det1) = step::reuse_of(&step::diff(&start, &facts), |k| keys.contains(k));
                (json!([ok, nb]) == c["out"] && det == c["detail"] && ok1 == ok && det1 == c["detail"], format!("{} vs {}", c["reading"], c["job_dir"]))
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
                let same = |r: &Value| r["returns"] == o["returns"] && same_map(&r["first_return"], &o["first_return"]) && same_map(&r["changes"], &o["changes"]);
                // the try's rule (E011 round 2): fed each frame's changes of the packed facts; and round 1's on them whole
                let t = Instant::now();
                let fs: Vec<fmi_core::Facts> = cps.iter().map(|x| crate::start::facts_of_json(&x[1])).collect();
                let mut rule = step::IdleRule::new();
                let mut ch = Vec::new();
                for (j, f) in fs.iter().enumerate().skip(1) {
                    ch.clear();
                    step::diff_into(&fs[j - 1], f, &mut ch);
                    rule.frame(cps[j][0].as_i64().unwrap_or(0) - f0, &ch);
                }
                let by_changes = rule.result();
                add_s(&mut rep.seconds, "idle_rule (changes)", t.elapsed().as_secs_f64());
                let whole = step::idle_rule_facts(&fs[0], fs[1..].iter().enumerate().map(|(j, f)| (cps[j + 1][0].as_i64().unwrap_or(0) - f0, f.clone())));
                (same(&r) && same(&by_changes) && same(&whole) && by_changes == whole, format!("{}", c["entry_file"]))
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

/// A recorded job of the reference replayed through `fmi try`'s code with every port a double (crate::doubles), in a
/// copy of its job dir under `out`: the try's decisions and records against the recorded ones.
pub fn replay_check(rec_dir: &Path, out: &Path, domains_dir: &Path) -> Res<(Vec<(String, bool, String)>, f64)> {
    use crate::doubles::*;
    use crate::driver::JobLog;
    use crate::harness::PlannerCache;
    use crate::tryjob::{run_job, Resident};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    let job = read_json(&rec_dir.join("job.json"))?;
    let want = read_json(&rec_dir.join("result.json"))?;
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out).map_err(|e| files::io_err(out, e))?;
    let mut j2 = job.clone();
    let rdname = Path::new(job["reading_dir"].as_str().unwrap_or("r")).file_name().unwrap().to_string_lossy().into_owned();
    j2["reading_dir"] = json!(out.join("readings").join(&rdname).to_string_lossy());
    std::fs::write(out.join("job.json"), pyjson::dumps(&j2, pyjson::Opts::INDENT1) + "\n").map_err(|e| files::io_err(out, e))?;
    // the attempts' runs, in order
    let mut runs = Vec::new();
    for k in 0..100 {
        let p = rec_dir.join("run").join(format!("a{k:02}")).join("run.json");
        if !p.exists() {
            break;
        }
        runs.push(read_json(&p)?);
    }
    let played = runs.last().cloned().unwrap_or(json!({}));
    let tl = Arc::new(timeline_of(rec_dir, &job, &played)?);
    let mut readings: Vec<PathBuf> = Vec::new();
    if let Some(p) = want["reading"]["path"].as_str() {
        readings.push(PathBuf::from(p));
    }
    for r in job["readings"].as_array().cloned().unwrap_or_default() {
        readings.push(PathBuf::from(r["path"].as_str().unwrap_or("")));
    }
    let run_dir = rec_dir.join("run").join(format!("a{:02}", runs.len().saturating_sub(1)));
    if let Ok(rd) = std::fs::read_dir(&run_dir) {
        for d in rd.flatten() {
            if d.path().join("reading.json").exists() {
                readings.push(d.path().join("reading.json"));
            }
        }
    }
    let fresh = if files::truthy(want["reading"].get("fresh")) { want["reading"]["path"].as_str().map(PathBuf::from) } else { None };
    let rr = Arc::new(RecordedReadings::with_dumps(domains_dir, &readings, fresh, run_dir.clone()));
    let plans: Vec<Value> = runs.iter().flat_map(|r| r["plans"].as_array().cloned().unwrap_or_default()).collect();
    let resident = Resident {
        returns: Mutex::new(Box::new(ScriptReturns(tl.clone()))),
        readings: rr.clone(),
        planners: Mutex::new(PlannerCache::new(Arc::new(ScriptPlanners { plans, next: Arc::new(Mutex::new(0)) }))),
        players: Arc::new(ScriptPlayers { run: played.clone(), tl: tl.clone() }),
        search: fmi_core::Search::FdLamaFirstCosts,
        search_name: "lama-first-costs".into(),
        forced_targets: None,
        tail: false,
    };
    let log = JobLog::create(&out.join("log"))?;
    let t = Instant::now();
    let got = run_job(out, &resident, &log, &AtomicBool::new(false));
    let secs = t.elapsed().as_secs_f64();
    let mut rows: Vec<(String, bool, String)> = Vec::new();
    let mut cmp = |what: &str, a: Value, b: Value| {
        let ok = a == b;
        rows.push((what.to_string(), ok, if ok { String::new() } else { format!("got {} want {}", pyjson::compact(&a), pyjson::compact(&b)) }));
    };
    let pick = |v: &Value, keys: &[&str]| -> Value {
        let mut m = serde_json::Map::new();
        for k in keys {
            m.insert((*k).into(), v.get(*k).cloned().unwrap_or(Value::Null));
        }
        Value::Object(m)
    };
    cmp("ok, error", json!([got["ok"], got["error"]]), json!([want["ok"], want["error"]]));
    cmp("return same", got["return"]["same"].clone(), want["return"]["same"].clone());
    cmp("reading", pick(&got["reading"], &["path", "fresh", "why", "scope", "records", "actions"]),
        pick(&want["reading"], &["path", "fresh", "why", "scope", "records", "actions"]));
    cmp("candidates", got["reading"]["candidates"].clone(), want["reading"]["candidates"].clone());
    cmp("targets", got["targets"].clone(), want["targets"].clone());
    let probe_keys = ["target", "tier", "depth", "status", "stop", "attempt"];
    cmp("probes", json!(got["probes"].as_array().cloned().unwrap_or_default().iter().map(|p| pick(p, &probe_keys)).collect::<Vec<_>>()),
        json!(want["probes"].as_array().cloned().unwrap_or_default().iter().map(|p| pick(p, &probe_keys)).collect::<Vec<_>>()));
    cmp("tried, exhausted", json!([got["tried"], got["exhausted"]]), json!([want["tried"], want["exhausted"]]));
    let att_keys = ["target", "tier", "depth", "stop", "reached", "goal_from_the_plan", "frames", "frame0", "frame_end", "qf0",
                    "qf_end", "qf", "replans", "inputs", "checkpoints"];
    cmp("attempt", pick(&got["attempt"], &att_keys), pick(&want["attempt"], &att_keys));
    let ref_keys = ["plan", "frame", "class", "added", "why_not"];
    let refs = |v: &Value| json!(v["attempt"]["refinements"].as_array().cloned().unwrap_or_default().iter().map(|t| pick(t, &ref_keys)).collect::<Vec<_>>());
    cmp("refinements", refs(&got), refs(&want));
    let ext_keys = ["plan", "frame", "n_values", "actions_before", "actions"];
    let exts = |v: &Value| json!(v["attempt"]["extensions"].as_array().cloned().unwrap_or_default().iter().map(|t| pick(t, &ext_keys)).collect::<Vec<_>>());
    cmp("extensions", exts(&got), exts(&want));
    let new_keys = ["file", "frame", "tick", "how"];
    let news = |v: &Value| json!(v["new"].as_array().cloned().unwrap_or_default().iter().map(|t| pick(t, &new_keys)).collect::<Vec<_>>());
    cmp("new", news(&got), news(&want));
    for n in got["new"].as_array().cloned().unwrap_or_default() {
        let f = n["file"].as_str().unwrap_or("");
        let a = read_gz(&out.join(f)).unwrap_or(Value::Null);
        let b = read_gz(&rec_dir.join(f)).unwrap_or(Value::Null);
        cmp(&format!("{f}"), a, b);
    }
    // the played attempt's record
    let k = runs.len().saturating_sub(1);
    let gr = read_json(&out.join("run").join(format!("a{k:02}")).join("run.json")).unwrap_or(Value::Null);
    let plan_keys = ["plan", "frame", "bans", "status", "steps", "reach_misses", "reach_values", "costs", "plan_cost"];
    let plans_of = |v: &Value| json!(v["plans"].as_array().cloned().unwrap_or_default().iter().map(|t| pick(t, &plan_keys)).collect::<Vec<_>>());
    cmp("run plans", plans_of(&gr), plans_of(&played));
    let rk = ["plan", "frame", "qf", "what", "cause", "class", "values", "added", "why_not"];
    let turns = |v: &Value| json!(v["refinements"].as_array().cloned().unwrap_or_default().iter().map(|t| pick(t, &rk)).collect::<Vec<_>>());
    cmp("run refinements", turns(&gr), turns(&played));
    cmp("run replan causes", gr["replan_causes"].clone(), played["replan_causes"].clone());
    cmp("run stop, replans, frames", json!([gr["stop"], gr["replans"], gr["frames"], gr["goal_from_the_plan"]]),
        json!([played["stop"], played["replans"], played["frames"], played["goal_from_the_plan"]]));
    cmp("run inputs", gr["inputs"].clone(), played["inputs"].clone());
    // the first plan's start file (its start does not depend on the window search)
    let a = std::fs::read_to_string(out.join("run").join(format!("a{k:02}")).join("plans").join("plan-000").join("start.txt")).unwrap_or_default();
    let b = std::fs::read_to_string(run_dir.join("plans").join("plan-000").join("start.txt")).unwrap_or_default();
    if !b.is_empty() && !a.is_empty() {
        cmp("plan-000 start.txt", json!(a == b), json!(true));
    }
    // the reads the try asked for (the reference's folders)
    let reads: Vec<String> = rr.reads.lock().unwrap().iter().map(|r| r.name.clone()).collect();
    rows.push(("reads".into(), true, format!("{reads:?}")));
    Ok((rows, secs))
}

/// A recorded archive's jobs ingested again, in its log's order, into a fresh archive with its stage (the jobs'
/// job.json, select.json, result.json and new entries copied; each log line's driver fields given as the extra): the
/// archive's logic on real results. Compared: entries.jsonl and cells.json byte for byte, log.jsonl line by line,
/// each job's job.json against job_doc of the archive before it (but "readings", which eviction changed during the
/// run), and readings.jsonl without the eviction's fields. Also the archive's own time per job (job_doc + select +
/// ingest), the driver's work on a fixed real input.
pub fn reingest_check(rec: &Path, out: &Path) -> Res<(Vec<(String, bool, String)>, Vec<f64>)> {
    use crate::archive::Archive;
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out.join("jobs")).map_err(|e| files::io_err(out, e))?;
    let stage = read_json(&rec.join("stage.json"))?;
    let root0 = read_gz(&rec.join("entries").join("0.json.gz"))?;
    let allowed: Vec<i64> = stage["allowed_rooms"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_i64()).collect();
    let mut a = Archive::create(out, stage["stage"].as_str().unwrap_or(""), &stage["goal"], Path::new(stage["guide"].as_str().unwrap_or("")),
                                &allowed, stage["workers"].as_i64().unwrap_or(2), stage.get("root").cloned(),
                                stage.get("e009").cloned(), stage.get("e010").cloned())?;
    let cp = root0["checkpoints"].as_array().cloned().unwrap_or_default();
    a.add(None, root0["frame"].as_i64().unwrap_or(0), root0["tick"].as_i64(), &[], &cp, None, "boot")?;
    a.write_cells()?;
    let (logs, _) = files::read_jsonl(&rec.join("log.jsonl"))?;
    let mut rows = Vec::new();
    let mut times = Vec::new();
    let mut cpus = Vec::new();
    let mut parts: Vec<(f64, f64, f64)> = Vec::new();
    let mut job_docs_bad = Vec::new();
    // with b workers the driver prepares job k after committing job k - b (one commit, then its fill): job.json is
    // compared with job_doc at that moment
    let workers = stage["workers"].as_u64().unwrap_or(2).max(1) as usize;
    let order: Vec<String> = logs.iter().filter(|x| crate::archive::committed(x)).map(|x| x["job"].as_str().unwrap_or("").to_string()).collect();
    let mut docs: std::collections::HashMap<String, Value> = Default::default();
    // a job whose entry is not in the archive yet was prepared later (its cell busy, or its entry not made yet): its
    // job.json is taken at the first fill after its entry exists
    let mut pending: Vec<usize> = Vec::new();
    let take_doc = |a: &Archive, k: usize, docs: &mut std::collections::HashMap<String, Value>, pending: &mut Vec<usize>| -> Res<()> {
        if let Some(j) = order.get(k) {
            let want = read_json(&rec.join("jobs").join(j).join("job.json"))?;
            let e = want["entry"]["id"].as_u64().unwrap_or(0) as usize;
            if e < a.rows.len() {
                docs.insert(j.clone(), a.job_doc(j, e)?);
            } else {
                pending.push(k);
            }
        }
        Ok(())
    };
    for k in 0..workers {
        take_doc(&a, k, &mut docs, &mut pending)?;
    }
    let mut done = 0;
    for x in &logs {
        let job = x["job"].as_str().unwrap_or("").to_string();
        if !crate::archive::committed(x) {
            a.log_event(x.clone())?;
            continue;
        }
        let src = rec.join("jobs").join(&job);
        let dst = out.join("jobs").join(&job);
        std::fs::create_dir_all(dst.join("new")).map_err(|e| files::io_err(&dst, e))?;
        for f in ["job.json", "select.json", "result.json"] {
            if src.join(f).exists() {
                std::fs::copy(src.join(f), dst.join(f)).map_err(|e| files::io_err(&src, e))?;
            }
        }
        if let Ok(rd) = std::fs::read_dir(src.join("new")) {
            for e in rd.flatten() {
                std::fs::copy(e.path(), dst.join("new").join(e.file_name())).map_err(|e2| files::io_err(&e.path(), e2))?;
            }
        }
        let want_job = read_json(&src.join("job.json"))?;
        let t = Instant::now();
        let c0 = crate::machine::thread_cpu_s();
        // the driver's work for this job: its job.json (from the archive before it) and the selection, then the ingest
        let e = want_job["entry"]["id"].as_u64().unwrap_or(0) as usize;
        let jd = a.job_doc(&job, e)?;
        let _ = pyjson::dumps(&jd, pyjson::Opts::INDENT1);
        let c1 = crate::machine::thread_cpu_s();
        let mut got_job = docs.remove(&job).unwrap_or(Value::Null);
        let _ = a.select(1);
        let _ = a.sel_key_json(e);
        let _ = a.n_selectable();
        let c2 = crate::machine::thread_cpu_s();
        let mut extra = serde_json::Map::new();
        for k in ["started", "ended", "driver_wall_s", "rc", "rusage", "guard"] {
            if let Some(v) = x.get(k) {
                extra.insert(k.into(), v.clone());
            }
        }
        let line = a.ingest(&dst, Some(&extra))?;
        times.push(t.elapsed().as_secs_f64());
        cpus.push(crate::machine::thread_cpu_s() - c0);
        parts.push((c1 - c0, c2 - c1, crate::machine::thread_cpu_s() - c2));
        done += 1;
        for k in std::mem::take(&mut pending) {
            take_doc(&a, k, &mut docs, &mut pending)?;
        }
        take_doc(&a, done + workers - 1, &mut docs, &mut pending)?;
        let mut wj = want_job.clone();
        for v in [&mut got_job, &mut wj] {
            if let Some(m) = v.as_object_mut() {
                m.remove("readings");
                // the archive's own path
                let rd = m.get("reading_dir").and_then(|r| r.as_str()).map(|r| Path::new(r).file_name().map(|n| n.to_string_lossy().into_owned()));
                if let Some(Some(n)) = rd {
                    m.insert("reading_dir".into(), json!(n));
                }
            }
        }
        if got_job != wj && job_docs_bad.len() < 5 {
            let diffk: Vec<String> = wj.as_object().map(|m| m.keys().filter(|k| got_job.get(*k) != wj.get(*k)).cloned().collect()).unwrap_or_default();
            job_docs_bad.push(format!("{job}: {diffk:?}"));
        }
        if &line != x {
            let diffk: Vec<String> = x.as_object().map(|m| m.keys().filter(|k| line.get(*k) != x.get(*k)).cloned().collect()).unwrap_or_default();
            rows.push((format!("log line {job}"), false, format!("{diffk:?}")));
        }
    }
    let same_file = |f: &str| -> (bool, String) {
        let a = std::fs::read(out.join(f)).unwrap_or_default();
        let b = std::fs::read(rec.join(f)).unwrap_or_default();
        (a == b, format!("{} vs {} bytes", a.len(), b.len()))
    };
    for f in ["entries.jsonl", "cells.json", "log.jsonl"] {
        let (ok, why) = same_file(f);
        rows.push((f.into(), ok, if ok { String::new() } else { why }));
    }
    rows.push(("job.json = job_doc at the fill (but readings)".into(), job_docs_bad.is_empty(), job_docs_bad.join("; ")));
    let strip = |p: &Path| -> Vec<Value> {
        files::read_jsonl(p).map(|r| r.0).unwrap_or_default().into_iter().map(|mut v| {
            if let Some(m) = v.as_object_mut() {
                m.remove("evicted");
                m.remove("bytes");
            }
            v
        }).collect()
    };
    let ra = strip(&out.join("readings.jsonl"));
    let rb = strip(&rec.join("readings.jsonl"));
    rows.push(("readings.jsonl (but evicted, bytes)".into(), ra == rb, format!("{} vs {} rows", ra.len(), rb.len())));
    for i in 0..a.rows.len() {
        let x = read_gz(&out.join("entries").join(format!("{i}.json.gz")))?;
        let y = read_gz(&rec.join("entries").join(format!("{i}.json.gz")))?;
        if x != y {
            rows.push((format!("entries/{i}.json.gz"), false, String::new()));
        }
    }
    rows.push((format!("entries/*.json.gz ({})", a.rows.len()), true, String::new()));
    let mut c = cpus.clone();
    c.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let n = parts.len().max(1) as f64;
    rows.push(("cpu".into(), true, format!("the archive's CPU per job: median {:.2} ms, mean {:.2} ms, max {:.2} ms; mean by part: job.json {:.2} ms (built and written as text), selection {:.2} ms, ingest {:.2} ms",
        c.get(c.len() / 2).copied().unwrap_or(0.0) * 1e3, c.iter().sum::<f64>() * 1e3 / c.len().max(1) as f64, c.last().copied().unwrap_or(0.0) * 1e3,
        parts.iter().map(|p| p.0).sum::<f64>() * 1e3 / n, parts.iter().map(|p| p.1).sum::<f64>() * 1e3 / n, parts.iter().map(|p| p.2).sum::<f64>() * 1e3 / n)));
    Ok((rows, times))
}
