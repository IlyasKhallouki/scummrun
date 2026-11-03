//! refuse-check: E013's component `refuse` (experiments/E013-robust/plan.md, method B; amendment (2) (a)), its first
//! step M1 and its gates. Counts, frames and numbers only; no game text.
//!
//!   refuse-check m1 ARCHIVE OUT [JOB]
//!       M1: every window of the archive's attempts that ran to its frame bound with every try ended at the interface
//!       (the window record's end_reason "<W> frames" and outcomes {"interface": n}). For each: the parent rebuilt from
//!       boot at the window's start (the job's entry movie and the attempt's inputs up to it; checked against the
//!       record's start and start_qf), forked at the first try's frame, the first try played again (the executor's
//!       input code as a window child plays it) on an engine with the input log (SPEEDRUN_INPUT=1), and the log read:
//!       per input its verdict, the handler's arguments, the verb slot, the reads and writes by kind; the parent's gate
//!       at the frame of each input (a no-input fork run there). Writes OUT/m1.jsonl and OUT/m1-summary.json.
//!
//! ARCHIVE is read only (its jobs' job.json and run/*/run.json).

use fmi_core::*;
use fmi_engine::Host;
use fmi_play::executor::{self, Br, Executor, PlayErr};
use fmi_play::replay::InputStep;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Res<T> = Result<T, String>;

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn read_json(p: &Path) -> Res<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?).map_err(e2s)
}

/// The input step of a window's input record name: "sentence(V,A[,B])#n" and "walk_to(...)" are clicks, as
/// replay::step_input makes them from the record (verb, object A, object B).
pub fn input_of(name: &str) -> Option<InputStep> {
    let base = name.split('#').next()?;
    let (kind, rest) = base.split_once('(')?;
    let inner = rest.split(')').next()?;
    let nums: Vec<i64> = inner.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    match kind {
        "sentence" => Some(InputStep { kind: "click", verb: nums.first().copied(), a: nums.get(1).copied(), b: nums.get(2).copied(), key: None }),
        "dialog_line" => Some(InputStep { kind: "line", verb: nums.first().copied(), a: None, b: None, key: None }),
        _ => None,
    }
}

fn input_j(i: &Input) -> Value {
    match i {
        Input::Click(x, y) => json!({"click": [x, y]}),
        Input::RClick(x, y) => json!({"rclick": [x, y]}),
        Input::Move(x, y) => json!({"move": [x, y]}),
        Input::Key(k) => json!({"key": k}),
        Input::Skips { .. } => json!({"skips": true}),
    }
}

fn kinds(ts: &[Touch]) -> Value {
    let mut m: BTreeMap<&str, usize> = BTreeMap::new();
    for t in ts {
        *m.entry(t.kind.name()).or_default() += 1;
    }
    json!(m)
}

/// One input record as M1 reports it.
pub fn record_j(r: &InputRecord) -> Value {
    json!({"frame": r.frame, "input": r.input.iter().map(input_j).collect::<Vec<_>>(), "verdict": r.verdict.text(),
           "stat": r.stat, "area": r.area, "value": r.value, "mode": r.mode, "slot": r.slot, "zone": r.zone,
           "key_match": r.key_match, "handler": r.handler, "writes": r.writes.len(), "reads": r.reads.len(),
           "write_kinds": kinds(&r.writes), "read_kinds": kinds(&r.reads), "ops": r.ops.len(),
           "dropped": [r.ops_dropped, r.touches_dropped]})
}

/// The parent's engine at a window's start, rebuilt from boot: the job's entry movie and the attempt's inputs up to the
/// start frame.
pub fn parent_at(host: &Host, job: &Value, run: &Value, start: u64, out: &Path) -> Res<Box<dyn Engine>> {
    let mut evs: Vec<Event> =
        job["entry"]["events"].as_array().ok_or("job.json: entry events")?.iter().filter_map(fmi_explore::events::event_of).collect();
    for v in run["inputs"].as_array().cloned().unwrap_or_default() {
        if let Some(e) = fmi_explore::events::event_of(&v) {
            if e.frame <= start {
                evs.push(e);
            }
        }
    }
    let cfg = BootCfg { seed: 1, events: evs, frame: Some(start), dump_locals: false, lean: true, out_dir: out.to_path_buf() };
    Ok(Box::new(host.boot_bin(&cfg).map_err(e2s)?))
}

/// A window that ran to its frame bound with every try ended at the interface.
pub fn bound_interface(w: &Value) -> bool {
    let er = w["end_reason"].as_str().unwrap_or("");
    let oc = w["outcomes"].as_object();
    er.ends_with(" frames")
        && oc.is_some_and(|o| !o.is_empty() && o.iter().all(|(k, _)| k == "interface"))
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let r = match argv.first().map(|s| s.as_str()) {
        Some("m1") => m1(&argv[1..]),
        _ => Err("usage: refuse-check m1 ARCHIVE OUT [JOB]".into()),
    };
    if let Err(e) = r {
        eprintln!("refuse-check: {e}");
        std::process::exit(1);
    }
}

fn jobs_of(arch: &Path, only: Option<&str>) -> Res<Vec<PathBuf>> {
    let mut jobs: Vec<PathBuf> =
        std::fs::read_dir(arch.join("jobs")).map_err(e2s)?.filter_map(|d| d.ok().map(|d| d.path())).collect();
    jobs.sort();
    Ok(jobs.into_iter().filter(|j| only.is_none_or(|o| j.file_name().is_some_and(|n| n == o))).collect())
}

fn attempts_of(jd: &Path) -> Vec<PathBuf> {
    let mut a: Vec<PathBuf> =
        std::fs::read_dir(jd.join("run")).map(|r| r.filter_map(|d| d.ok().map(|d| d.path())).collect()).unwrap_or_default();
    a.sort();
    a
}

fn m1(a: &[String]) -> Res<()> {
    let arch = PathBuf::from(a.first().ok_or("m1 ARCHIVE OUT [JOB]")?);
    let out = PathBuf::from(a.get(1).ok_or("m1 ARCHIVE OUT [JOB]")?);
    let only = a.get(2).map(|s| s.as_str());
    std::fs::create_dir_all(&out).map_err(e2s)?;
    std::env::set_var("SPEEDRUN_INPUT", "1");
    let host = Host::from_env().map_err(e2s)?;
    let mut lines = Vec::new();
    let mut summary: BTreeMap<String, usize> = BTreeMap::new();
    for jd in jobs_of(&arch, only)? {
        let jname = jd.file_name().unwrap().to_string_lossy().to_string();
        let Ok(job) = read_json(&jd.join("job.json")) else { continue };
        for ad in attempts_of(&jd) {
            let Ok(run) = read_json(&ad.join("run.json")) else { continue };
            for w in run["windows"].as_array().cloned().unwrap_or_default() {
                if !bound_interface(&w) {
                    continue;
                }
                let line = match m1_window(&host, &out, &jname, &ad, &job, &run, &w) {
                    Ok(v) => v,
                    Err(e) => json!({"job": jname, "plan": w["plan"], "macro": w["macro"], "input": w["input"], "error": e}),
                };
                let key = format!(
                    "{} | last input: {}",
                    line["route"].as_str().map(|s| s.split(": ").skip(2).collect::<Vec<_>>().join(": ")).unwrap_or_else(|| "error".into()),
                    line["last"]["verdict"].as_str().unwrap_or(if line["records"].as_array().is_some_and(|r| r.is_empty()) { "no record" } else { "-" })
                );
                *summary.entry(key).or_default() += 1;
                eprintln!("{} plan {} macro {}: {}", jname, w["plan"], w["macro"], line["route"].as_str().unwrap_or("error"));
                lines.push(line);
            }
        }
    }
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(out.join("m1.jsonl"), text).map_err(e2s)?;
    let s = json!({"windows": lines.len(), "by_route_and_last_verdict": summary,
                   "same_as_recorded": lines.iter().filter(|l| l["same_as_recorded"] == json!(true)).count(),
                   "parent_gate_open_at_last_input": lines.iter().filter(|l| l["last"]["parent_gate_open"] == json!(true)).count(),
                   "with_inputs": lines.iter().filter(|l| l["records"].as_array().is_some_and(|r| !r.is_empty())).count()});
    std::fs::write(out.join("m1-summary.json"), serde_json::to_string_pretty(&s).map_err(e2s)? + "\n").map_err(e2s)?;
    println!("{}", serde_json::to_string_pretty(&s).map_err(e2s)?);
    Ok(())
}

fn m1_window(host: &Host, out: &Path, jname: &str, ad: &Path, job: &Value, run: &Value, w: &Value) -> Res<Value> {
    let name = w["input"].as_str().ok_or("input")?;
    let inp = input_of(name).ok_or_else(|| format!("input {name}: not a sentence or a line"))?;
    let start = w["start"].as_u64().ok_or("start")?;
    let start_qf = w["start_qf"].as_u64().ok_or("start_qf")?;
    let child0 = w["children"].get(0).cloned().unwrap_or(Value::Null);
    let g = child0.get(0).and_then(|x| x.as_u64()).unwrap_or(start);
    let mut parent = parent_at(host, job, run, start, &out.join("boot"))?;
    let at_start = json!({"frame": parent.status().frame, "qf": parent.status().qf,
                          "ok": parent.status().frame == start && parent.status().qf == start_qf});
    if g > start {
        parent.run(&RunSpec { frame: Some(g), ..Default::default() }).map_err(e2s)?;
    }
    let mut cb = parent.fork().map_err(e2s)?;
    cb.input_log().map_err(e2s)?; // the records the parent's movie left
    let mut ex = Executor::new(true);
    let res = {
        let mut br = Br::new(&mut cb, None);
        br.watch_first = true;
        br.fuse = true;
        executor::play_input(&mut ex, &mut br, &inp, name)
    };
    let route = match &res {
        Ok(()) => "ok".to_string(),
        Err(PlayErr::Route(m)) => format!("route: {m}"),
        Err(PlayErr::Engine(e)) => format!("engine: {e}"),
    };
    let reached = cb.status().frame;
    let (up, cs) = (cb.status().up, cb.status().cs);
    let log = cb.input_log().map_err(e2s)?;
    cb.quit();
    // the parent's gate at each input's frame (a no-input fork)
    let mut gates: BTreeMap<u64, (i32, i32)> = BTreeMap::new();
    {
        let mut p = parent.fork().map_err(e2s)?;
        for r in &log.records {
            if r.frame > p.status().frame {
                if p.run(&RunSpec { frame: Some(r.frame), ..Default::default() }).is_err() {
                    break;
                }
            }
            if p.status().frame == r.frame {
                gates.insert(r.frame, (p.status().up, p.status().cs));
            }
        }
        p.quit();
    }
    parent.quit();
    let recs: Vec<Value> = log
        .records
        .iter()
        .map(|r| {
            let mut v = record_j(r);
            if let Some((u, c)) = gates.get(&r.frame) {
                v["parent_gate"] = json!([u, c]);
                v["parent_gate_open"] = json!(*u > 0);
            }
            v
        })
        .collect();
    let last = recs.last().cloned().unwrap_or(Value::Null);
    // the executor wanted the plan's verb: did the last input reach the handler with it (as a verb click's value)?
    let wanted = inp.verb;
    let last_rec = log.records.last();
    let reached_with_verb = last_rec.map(|r| !matches!(r.verdict, Verdict::Dropped(_)) && Some(r.value as i64) == wanted);
    let recorded_why = child0.get(7).and_then(|x| x.as_str()).unwrap_or("");
    let same = route.strip_prefix("route: ").is_some_and(|m| fmi_play::pyj::cut(m, 80) == recorded_why);
    Ok(json!({
        "job": jname, "att": ad.file_name().unwrap().to_string_lossy(), "plan": w["plan"], "macro": w["macro"], "input": name,
        "start": start, "at_start": at_start, "g": g, "route": route, "reached": reached, "child_gate": [up, cs],
        "inputs": ex.movie.iter().filter(|e| executor::is_input(e)).map(|e| json!([e.frame, input_j(&e.what)])).collect::<Vec<_>>(),
        "records": recs, "log_dropped": log.dropped, "last": last, "last_reached_handler_with_the_verb": reached_with_verb,
        "recorded_first_child": child0, "same_as_recorded": same,
    }))
}
