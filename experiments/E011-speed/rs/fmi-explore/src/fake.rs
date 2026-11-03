//! fake_step.py, line for line: a stand-in for a try in the explorer's tests. It reads job.json and writes
//! result.json and new entries in the reference's formats, with no engine, reader or planner; what it does depends on
//! job.json and the guide only, so a run of the explorer gives the same archive however its jobs are timed.
//!
//! env: FAKE_GOAL_AFTER (default 4); FAKE_SLEEP_S (default 0; even job numbers sleep it, odd ones a third of it);
//! FAKE_FAIL (an entry id whose jobs fail with one error); FAKE_DIFFER (an entry id whose return differs);
//! FAKE_GUARD_ONCE (a marker file: the first job to find it absent creates it and ends as the guard's kill does);
//! FAKE_NO_PROGRESS (a key: the target is that key at the entry's value plus 2); FAKE_IDLE_KEY (E009: a key the end
//! entry raises by 1 in every job).
//!
//! E011's deepening (FMI_PLAN_DEEPEN, step::deepen; not fake_step.py's): FAKE_SLOW "k=v:s,..." gives a target's
//! planning seconds ("inf": no plan). The candidates come in step::deepen_order with job.json's "deferred"; one whose
//! seconds pass its limit L0 * 2^n is deferred (probe status "time"), and the first other one plays as before.

use crate::driver::{JobLog, Step};
use crate::files::{self, as_i64, read_json, Res};
use crate::pyjson;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

pub struct FakeStep;

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

fn write_json(path: &Path, v: &Value) -> Res<()> {
    // fake_step.py's write_json: indent=1, atomic
    files::write_json(path, v, Some(1))
}

fn get0(f: &Map<String, Value>, k: &str) -> i64 {
    as_i64(f.get(k)).unwrap_or(0)
}

impl Step for FakeStep {
    fn run(&self, job_dir: &Path, log: &JobLog, _cancel: &AtomicBool) -> i32 {
        match fake(job_dir, log) {
            Ok(rc) => rc,
            Err(e) => {
                log.line(&format!("fake step: {e}"));
                1
            }
        }
    }
}

/// fake_step.py `main` on JOB_DIR; the exit code (0, or 143 for the guard's kill).
pub fn fake(d: &Path, log: &JobLog) -> Res<i32> {
    let t0 = Instant::now();
    let job = read_json(&d.join("job.json"))?;
    let name = job["job"].as_str().unwrap_or("j0");
    let n: i64 = name[1..].parse().unwrap_or(0);
    if let Some(marker) = env("FAKE_GUARD_ONCE") {
        if !Path::new(&marker).exists() {
            fs::write(&marker, format!("{name}\n")).map_err(|e| files::io_err(Path::new(&marker), e))?;
            log.line("GUARD: MemAvailable 1000 kB, killing group 1");
            return Ok(143);
        }
    }
    let sleep: f64 = env("FAKE_SLEEP_S").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    std::thread::sleep(Duration::from_secs_f64(if n % 2 == 0 { sleep } else { sleep / 3.0 }));
    let e = &job["entry"];
    let cp = e["checkpoint"].as_array().ok_or("job.json: entry checkpoint")?;
    let frame = as_i64(cp.first()).ok_or("checkpoint frame")?;
    let facts = cp.get(1).and_then(|f| f.as_object()).cloned().unwrap_or_default();
    let extra = cp.get(2).cloned().unwrap_or(json!({}));
    let eid = e["id"].clone();
    let mut out = json!({"job": name, "entry": eid, "ok": true, "error": null,
                         "return": {"same": true, "differ": {}, "boot_s": 0.0}, "reading": null,
                         "targets": {"by_tier": {"0": 0, "1": 0, "2": 0}, "first": []}, "probes": [], "attempt": null,
                         "readings_made": [], "new": [], "tried": [], "exhausted": false,
                         "cpu_s": {"return": 0.0, "read": 0.0, "plan": 0.0, "play": 0.0, "total": 0.0},
                         "peak_rss_mb": {"python": 10.0, "children": 0.0}, "wall_s": 0.0});
    let eid_s = files::py_str(Some(&eid));
    if env("FAKE_DIFFER").as_deref() == Some(eid_s.as_str()) {
        out["return"] = json!({"same": false, "differ": {"facts": 1, "first": [["var:990", [0, 1]]]}, "boot_s": 0.0});
        out["ok"] = json!(false);
        out["error"] = json!("the return differs");
        write_json(&d.join("result.json"), &out)?;
        return Ok(0);
    }
    if env("FAKE_FAIL").as_deref() == Some(eid_s.as_str()) {
        out["ok"] = json!(false);
        out["error"] = json!("fake failure");
        write_json(&d.join("result.json"), &out)?;
        return Ok(0);
    }
    let readings = job["readings"].as_array().cloned().unwrap_or_default();
    if !readings.is_empty() {
        out["reading"] = json!({"path": readings[0]["path"], "fresh": false, "why": null, "scope": job["scope"],
                                "records": 10, "actions": 5, "read_wall_s": 0.0, "read_cpu_s": 0.0,
                                "peak_rss_mb": 0.0, "files_s": 0.0});
    } else {
        let rd = Path::new(job["reading_dir"].as_str().unwrap_or(""));
        fs::create_dir_all(rd).map_err(|e| files::io_err(rd, e))?;
        let doc = json!({"fake": true, "scope": job["scope"]});
        fs::write(rd.join("reading.json"), pyjson::dumps_default(&doc) + "\n").map_err(|e| files::io_err(rd, e))?;
        out["reading"] = json!({"path": rd.join("reading.json").to_string_lossy(), "fresh": true,
                                "why": "no cached reading", "scope": job["scope"], "records": 100, "actions": 50,
                                "read_wall_s": 1.0, "read_cpu_s": 2.0, "peak_rss_mb": 50.0, "files_s": 0.5});
    }
    let guide = read_json(Path::new(job["guide"].as_str().unwrap_or("")))?;
    let pair = |p: &Value| (p.get(0).cloned().unwrap_or(Value::Null), p.get(1).cloned().unwrap_or(Value::Null));
    let key = |p: &(Value, Value)| pyjson::compact(&json!([p.0, p.1]));
    let held: HashSet<String> = job["held"].as_array().map(|a| a.iter().map(|p| key(&pair(p))).collect()).unwrap_or_default();
    let tried: HashSet<String> = job["tried"].as_array().map(|a| a.iter().map(|p| key(&pair(p))).collect()).unwrap_or_default();
    let goal = job["goal"].as_object().cloned().unwrap_or_default();
    let scope: Vec<i64> = job["scope"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
    // (tier, depth, key, value)
    let mut cands: Vec<(i64, i64, String, i64)> = Vec::new();
    for a in guide["atoms"].as_array().cloned().unwrap_or_default() {
        let k = a[0].as_str().unwrap_or("").to_string();
        let depth = as_i64(a.get(2)).unwrap_or(0);
        if goal.contains_key(&k) || (k == "room" && !a[1].as_i64().map(|v| scope.contains(&v)).unwrap_or(false)) {
            continue;
        }
        let v = if a[1].is_null() { get0(&facts, &k) + 1 } else { as_i64(a.get(1)).unwrap_or(0) };
        let kv = key(&(json!(k), json!(v)));
        if get0(&facts, &k) == v || tried.contains(&kv) {
            continue;
        }
        cands.push((if held.contains(&kv) { 2 } else { 1 }, depth, k, v));
    }
    cands.sort();
    if let Some(nop) = env("FAKE_NO_PROGRESS") {
        let v = get0(&facts, &nop) + 2;
        cands = vec![(1, 0, nop, v)];
    }
    out["targets"]["by_tier"] = json!({"0": 0, "1": cands.iter().filter(|c| c.0 == 1).count(),
                                       "2": cands.iter().filter(|c| c.0 == 2).count()});
    out["targets"]["first"] = Value::Array(cands.iter().take(5).map(|(t, dp, k, v)| json!([k, v, t, dp])).collect());
    if cands.is_empty() {
        out["exhausted"] = json!(true);
        write_json(&d.join("result.json"), &out)?;
        return Ok(0);
    }
    let mut chosen = cands[0].clone();
    if let Some(l0) = crate::step::deepen() {
        // the deepening's order and deferrals (module docstring)
        let slow: std::collections::HashMap<String, f64> = env("FAKE_SLOW")
            .map(|s| {
                s.split(',')
                    .filter_map(|p| {
                        let (kv, t) = p.rsplit_once(':')?;
                        Some((kv.to_string(), if t == "inf" { f64::INFINITY } else { t.parse().ok()? }))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let deferred_in = crate::step::deferred_of(&job);
        let ts: Vec<crate::step::Target> = cands
            .iter()
            .map(|(t, dp, k, v)| crate::step::Target { k: k.clone(), v: crate::step::TV::Int(*v), tier: *t, depth: *dp })
            .collect();
        let mut probes = Vec::new();
        let mut deferred = Vec::new();
        let mut play = None;
        for (tg, n) in crate::step::deepen_order(ts, &deferred_in) {
            let v = match tg.v {
                crate::step::TV::Int(v) => v,
                _ => continue,
            };
            let limit = crate::step::deepen_limit(l0, n);
            let secs = slow.get(&format!("{}={}", tg.k, v)).copied().unwrap_or(0.0);
            if secs > limit as f64 {
                probes.push(json!({"target": [tg.k, v], "status": "time", "seconds": limit, "limit_s": limit, "deferred_before": n}));
                deferred.push(json!([tg.k, v, n + 1]));
                continue;
            }
            probes.push(json!({"target": [tg.k, v], "status": "plan", "seconds": 0.0, "limit_s": limit, "deferred_before": n}));
            play = Some((tg.tier, tg.depth, tg.k.clone(), v));
            break;
        }
        out["probes"] = Value::Array(probes);
        out["deferred"] = Value::Array(deferred.clone());
        match play {
            Some(c) => chosen = c,
            None => {
                out["exhausted"] = json!(deferred.is_empty());
                write_json(&d.join("result.json"), &out)?;
                log.line(&format!("{name}: entry {eid_s} deferred {} targets, none played", deferred.len()));
                return Ok(0);
            }
        }
    } else {
        out["probes"] = json!([{"target": [chosen.2, chosen.3], "status": "plan", "seconds": 0.0}]);
    }
    let (tier, depth, k, v) = chosen;
    out["tried"] = json!([[k, v]]);
    let mut f1 = facts.clone();
    f1.insert("var:990".into(), json!(get0(&facts, "var:990") + 1));
    let mut f2 = f1.clone();
    f2.insert(k.clone(), json!(v));
    if let Some(idle_key) = env("FAKE_IDLE_KEY") {
        f2.insert(idle_key.clone(), json!(get0(&facts, &idle_key) + 1));
        if files::truthy(job.get("idle_frames")) && !files::truthy(job.get("idle_measured")) {
            let mut r: Vec<String> = goal.keys().cloned().collect();
            r.push(idle_key.clone());
            r.sort();
            r.dedup();
            let mut fr = Map::new();
            fr.insert(idle_key.clone(), json!(2));
            let mut ch = Map::new();
            ch.insert(idle_key.clone(), json!(9));
            out["idle"] = json!({"returns": r, "first_return": fr, "changes": ch, "frames": job["idle_frames"],
                                 "seconds": 0.0, "cpu_s": 0.0});
        }
    }
    let goal_after: i64 = env("FAKE_GOAL_AFTER").and_then(|s| s.parse().ok()).unwrap_or(4);
    if n >= goal_after - 1 {
        for (g, vals) in &goal {
            f2.insert(g.clone(), vals[0].clone());
        }
    }
    let ev1 = json!({"frame": frame + 3, "click": [10, 10]});
    let ev2 = json!({"frame": frame + 7, "key": "p"});
    let tick = as_i64(extra.get("tick")).unwrap_or(0);
    let rng = extra.get("rng").cloned().unwrap_or(Value::Null);
    let x1 = json!({"tick": tick + 3, "rng": rng, "actors": []});
    let x2 = json!({"tick": tick + 7, "rng": rng, "actors": []});
    let cp1 = json!([frame + 3, Value::Object(f1), x1]);
    let cp2 = json!([frame + 7, Value::Object(f2), x2]);
    let n0 = json!({"id": null, "parent": eid, "frame": frame + 3, "tick": x1["tick"], "events": [ev1], "checkpoints": [cp1]});
    let n1 = json!({"id": null, "parent": eid, "frame": frame + 7, "tick": x2["tick"], "events": [ev1, ev2],
                    "checkpoints": [cp1, cp2]});
    for (k, n) in [n0, n1].into_iter().enumerate() {
        let p = d.join("new").join(format!("{k}.json.gz"));
        files::write_gz_default(&p, &n)?;
        crate::archive::stash_new(&p, n);
    }
    out["new"] = json!([{"file": "new/0.json.gz", "frame": frame + 3, "tick": x1["tick"], "how": "checkpoint"},
                        {"file": "new/1.json.gz", "frame": frame + 7, "tick": x2["tick"], "how": "end"}]);
    out["attempt"] = json!({"target": [k, v], "stop": "goal", "reached": true, "goal_from_the_plan": true, "frames": 7,
                            "qf0": 0, "qf_end": 168, "replans": 0, "refinements": 0, "extensions": 0,
                            "window_summary": null, "seconds": 0.0});
    out["wall_s"] = json!(pyjson::round(t0.elapsed().as_secs_f64(), 3));
    write_json(&d.join("result.json"), &out)?;
    log.line(&format!("{name}: entry {eid_s} target {k}={v} (tier {tier}, depth {depth})"));
    Ok(0)
}
