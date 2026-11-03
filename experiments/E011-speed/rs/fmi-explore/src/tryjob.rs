//! step.py's `run_job`, line for line, on the ports: one try of the explorer (return, idle measurement, the reading,
//! the targets, the attempts through the harness, the entries reached), writing JOB_DIR/result.json and
//! JOB_DIR/new/K.json.gz in the reference's formats. The ports stay warm across tries in `Resident` (one process).

use crate::driver::{JobLog, Step};
use crate::files::{self, read_json, Res};
use crate::harness::{self, Harness, HarnessCfg, PlannerCache};
use crate::machine;
use crate::ports::{EntryRef, PlayerHost, Readings, Returns, ReadReq, WindowCfg};
use crate::pyjson;
use crate::start;
use crate::step::{self, Target, TV};
use crate::wiring::Wiring;
use crate::events;
use fmi_core::{facts_of, Checkpoint, Engine, Facts, Key, Obs, ObsReq, PlayState, ReadingSource, RunSpec, Search, Status, Val};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// What stays warm from try to try in the one process: the kept entries, the readings, the planners, the window
/// search's host.
pub struct Resident {
    pub returns: Mutex<Box<dyn Returns>>,
    pub readings: Arc<dyn Readings>,
    pub planners: Mutex<PlannerCache>,
    pub players: Arc<dyn PlayerHost>,
    pub search: Search,
    /// the plans' entries' search name ("lama-first-costs" for FD, "gbfs" for the Rust search)
    pub search_name: String,
    /// `--targets K=V,...`: the targets forced, in order (the benchmark's B0/B3)
    pub forced_targets: Option<Vec<(String, i64)>>,
    /// E011's old tail first at a replan (the fast settings)
    pub tail: bool,
}

/// The try as the driver's in-process step.
pub struct TryStep(pub Arc<Resident>);

impl Step for TryStep {
    fn run(&self, job_dir: &Path, log: &JobLog, cancel: &AtomicBool) -> i32 {
        let r = run_job(job_dir, &self.0, log, cancel);
        if cancel.load(Ordering::SeqCst) {
            // the guard's kill: no result, so the driver classifies the job as guard-killed
            let _ = std::fs::remove_file(job_dir.join("result.json"));
            return 143;
        }
        step::exit_code(&r)
    }
}

struct ReturnDiffers(String);

/// FMI_TRY_PROFILE: the try's own time by section (wall less the ports), on stderr.
struct Prof {
    on: bool,
    t: Instant,
    p: f64,
    rows: Vec<(String, f64, f64)>,
}

impl Prof {
    fn new() -> Prof {
        Prof { on: std::env::var("FMI_TRY_PROFILE").is_ok(), t: Instant::now(), p: machine::port_s(), rows: vec![] }
    }
    fn mark(&mut self, what: &str) {
        if self.on {
            let (w, p) = (self.t.elapsed().as_secs_f64(), machine::port_s());
            self.rows.push((what.into(), w, p - self.p));
            self.t = Instant::now();
            self.p = p;
        }
    }
    fn print(&self) {
        let secs = machine::sections_take();
        if self.on {
            for (w, t, p) in &self.rows {
                eprintln!("try profile: {w}: {:.2} ms, of which ports {:.2} ms", t * 1e3, p * 1e3);
            }
            for (w, t, n) in secs {
                eprintln!("try profile:   section {w}: {:.2} ms in {n} calls", t * 1e3);
            }
        }
    }
}

fn cpu_now() -> f64 {
    machine::thread_cpu_s()
}

/// replay.compare_checkpoint: the differences between a checkpoint and the engine's state and status.
pub fn compare_checkpoint(want: &Value, state: &Obs, status: &Status) -> Map<String, Value> {
    let mut out = Map::new();
    let wf = start::facts_of_json(&want[1]);
    let got = facts_of(state);
    let mut d: Vec<(String, Val, Val)> = wf.diff(&got).into_iter().map(|(k, a, b)| (k.text(), a, b)).collect();
    if !d.is_empty() {
        d.sort_by(|a, b| a.0.cmp(&b.0));
        out.insert("facts".into(), json!(d.len()));
        out.insert("first".into(), json!(d.iter().take(8).map(|(k, a, b)| json!([k, [start::val_json(*a), start::val_json(*b)]])).collect::<Vec<_>>()));
    }
    if let Some(extra) = want.get(2).and_then(|e| e.as_object()) {
        let got = harness::extra_of(state, status.tick, status.rng);
        for (k, v) in extra {
            let g = got.get(k).cloned().unwrap_or(Value::Null);
            if &g != v {
                out.insert(k.clone(), json!({"want": v, "got": g}));
            }
        }
    }
    out
}

/// start.py's checks of a boot from the root (replay_part1.boot's row["checks"]), those the bridge's state shows. The
/// stop's own checks (an idle stop of kind sentence) hold for a boot run to the idle stop; a return that is a fork of
/// a kept root, or a boot run to the root's frame, stops otherwise, and its state is held to the root's checkpoint
/// (facts, tick, random state, actors) instead.
fn boot_checks(st: &Status, obs: &Obs) -> Vec<String> {
    let mut out = Vec::new();
    if st.reason == "idle" && st.idle.as_deref() != Some("sentence") {
        out.push(format!("idle kind {}, not sentence", files::py_repr(&json!(st.idle))));
    }
    if obs.seed != Some(1) {
        out.push(format!("dump seed {}", obs.seed.map(|s| s.to_string()).unwrap_or("None".into())));
    }
    if obs.boot_param != 0 {
        out.push(format!("dump boot_param {}", obs.boot_param));
    }
    let dbg = obs.vars.get(39).copied().unwrap_or(0);
    if dbg != 0 {
        out.push(format!("VAR_DEBUGMODE is {dbg}"));
    }
    out
}

/// Step 1: the engine at the entry, and the return's record.
fn boot_entry(r: &Resident, entry: &Value, out_dir: &Path) -> Result<(Box<dyn Engine>, Value), String> {
    let t = Instant::now();
    let root = files::truthy(entry.get("root"));
    let evs: Vec<fmi_core::Event> = entry["events"].as_array().cloned().unwrap_or_default().iter().filter_map(events::event_of).collect();
    if root && !evs.is_empty() {
        return Err("ValueError: a root entry has no events".into());
    }
    let er = EntryRef { id: files::as_i64(entry.get("id")).unwrap_or(-1), root, frame: entry["frame"].as_u64().unwrap_or(0), events: evs };
    let mut b = machine::port(|| r.returns.lock().unwrap().return_to(&er, out_dir)).map_err(|e| format!("EngineError: {e}"))?;
    let obs = machine::port(|| b.obs(&ObsReq::default())).map_err(|e| format!("EngineError: {e}"))?;
    let st = b.status().clone();
    let mut d = compare_checkpoint(&entry["checkpoint"], &obs, &st);
    if st.frame != er.frame {
        d.insert("frame".into(), json!({"want": er.frame, "got": st.frame}));
    }
    if root {
        let checks = boot_checks(&st, &obs);
        if !checks.is_empty() {
            d.insert("boot_checks".into(), json!(checks));
        }
    }
    let ret = json!({"same": d.is_empty(), "differ": Value::Object(d), "boot_s": pyjson::round(t.elapsed().as_secs_f64(), 2), "frame": st.frame});
    Ok((b, ret))
}

/// E009 fix a: a fork of the returned engine run `frames` frames with no input, the facts taken at each.
fn measure_idle(root: &mut Box<dyn Engine>, frames: u64) -> Result<Value, String> {
    let t = Instant::now();
    let mut b = machine::port(|| root.fork()).map_err(|e| format!("EngineError: {e}"))?;
    let f0 = b.status().frame;
    let start_f = machine::port(|| b.facts()).map_err(|e| format!("EngineError: {e}"))?;
    let mut seq = Vec::with_capacity(frames as usize);
    let mut err = None;
    for i in 1..=frames {
        if let Err(e) = machine::port(|| b.run(&RunSpec { frame: Some(f0 + i), ..Default::default() })) {
            err = Some(e);
            break;
        }
        match machine::port(|| b.facts()) {
            Ok(f) => seq.push((i as i64, f)),
            Err(e) => {
                err = Some(e);
                break;
            }
        }
    }
    let frame_end = b.status().frame;
    let cpu = b.cpu_s();
    b.quit();
    if let Some(e) = err {
        return Err(format!("EngineError: {e}"));
    }
    let mut out = step::idle_rule_facts(&start_f, seq);
    out["frame_end"] = json!(frame_end);
    out["cpu_s"] = json!(pyjson::round(cpu, 1));
    out["frames"] = json!(frames);
    out["frame0"] = json!(f0);
    out["seconds"] = json!(pyjson::round(t.elapsed().as_secs_f64(), 1));
    Ok(out)
}

fn doc_of(rd: &dyn ReadingSource) -> Arc<Value> {
    crate::warm::head(rd)
}

fn reading_path(rd: &dyn ReadingSource) -> PathBuf {
    rd.dir().join("reading.json")
}

/// step.py `reading_record`.
fn reading_record(rd: &dyn ReadingSource, fresh: bool, why: &str, wall: f64, load_s: f64) -> Value {
    let doc = doc_of(rd);
    let empty = json!({});
    let rs = files::or(doc.get("reader_scope"), &empty).clone();
    let read_s = doc.get("read_s").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let mut out = json!({"path": reading_path(rd).to_string_lossy(), "fresh": fresh, "why": why, "scope": doc["scope"]["rooms"],
        "records": doc.get("n_records"), "actions": doc.get("n_actions"),
        "read_wall_s": if fresh { rs.get("read_wall_s").cloned().unwrap_or(Value::Null) } else { json!(0.0) },
        "read_cpu_s": if fresh { rs.get("read_cpu_s").cloned().unwrap_or(Value::Null) } else { json!(0.0) },
        "peak_rss_mb": if fresh { rs.get("peak_rss_mb").cloned().unwrap_or(Value::Null) } else { json!(0) },
        "files_s": if fresh { json!(pyjson::round(wall - read_s, 1)) } else { json!(0.0) }});
    out["read_s"] = if fresh { doc.get("read_s").cloned().unwrap_or(Value::Null) } else { json!(0.0) };
    out["wall_s"] = json!(pyjson::round(wall, 1));
    out["load_s"] = json!(pyjson::round(load_s, 2));
    out["fact_vars"] = files::or(doc.get("fact_vars"), &json!([])).clone();
    out["start"] = doc["start"]["name"].clone();
    out
}

fn task_keys(rd: &dyn ReadingSource) -> HashSet<String> {
    rd.domains().0.iter().map(|(k, _)| k.text()).collect()
}

/// Step 2 on the cached readings (step.py `choose_reading`): (the reading or None, why, the candidates checked).
fn choose_reading(r: &Resident, job: &Value, facts: &crate::archive::FactMap) -> Result<(Option<Arc<dyn ReadingSource>>, String, Vec<Value>), String> {
    let cands = step::cached_readings(job);
    if cands.is_empty() {
        return Ok((None, "no cached reading".into(), vec![]));
    }
    let mut checked = Vec::new();
    for p in cands {
        let rd = machine::port(|| r.readings.open(&p))?;
        let doc = doc_of(&*rd);
        let sf = crate::archive::fact_map(&doc["start"]["facts"]);
        let keys = task_keys(&*rd);
        let (mut ok, _) = step::reusable(&sf, &keys, facts);
        let mut row = json!({"path": p.to_string_lossy(), "reusable": ok});
        for (k, v) in step::reusable_detail(&sf, &keys, facts).as_object().unwrap() {
            row[k] = v.clone();
        }
        if !ok && job.get("reuse").and_then(|x| x.as_str()) == Some("e009") {
            let extra = machine::port(|| r.readings.reuse_keys(&*rd))?;
            let mut k2 = keys.clone();
            k2.extend(extra.iter().cloned());
            ok = step::reusable(&sf, &k2, facts).0;
            row["reusable"] = json!(ok);
            row["reuse_keys"] = json!(extra.len());
            row["e009"] = step::reusable_detail(&sf, &k2, facts);
        }
        let e009 = row.get("e009").is_some();
        checked.push(row);
        if ok {
            return Ok((Some(rd), if e009 { "reused (e009 keys)".into() } else { "reused".into() }, checked));
        }
    }
    let first = checked[0].get("e009").cloned().unwrap_or_else(|| checked[0].clone());
    Ok((None, format!("differs on {} keys", first["other"]), checked))
}

/// Step 2's fresh reading from the returned engine's state (step.py `read_fresh`): (reading, wall s, load s).
fn read_fresh(r: &Resident, job: &Value, state: &Obs, status: &Status, threads: u32, log: &JobLog, cancel: &AtomicBool)
              -> Result<(Arc<dyn ReadingSource>, f64, f64), String> {
    let rdir = PathBuf::from(job["reading_dir"].as_str().unwrap_or(""));
    if !rdir.is_absolute() || rdir.to_string_lossy().chars().any(|c| c.is_whitespace()) {
        return Err(format!("ValueError: reading_dir {}: an absolute path with no spaces (the reach extender's start-rows rule)", rdir.display()));
    }
    std::fs::create_dir_all(&rdir).map_err(|e| files::io_err(&rdir, e))?;
    let rows = rdir.join("start.jsonl");
    std::fs::write(&rows, step::start_row_text(step::SEED, state, status)? + "\n").map_err(|e| files::io_err(&rows, e))?;
    let name = rdir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut scope: Vec<i64> = job["scope"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
    scope.sort();
    log.say(&format!("reading {:?} fresh from frame {} into {}", scope, status.frame, rdir.display()));
    let goal = step::goal_of(job);
    let t = Instant::now();
    let req = ReadReq {
        rooms: scope,
        seed: step::SEED,
        goal_text: step::goal_text(&goal)?,
        name,
        out: rdir.clone(),
        threads,
        slot_locals: vec![],
        domain_goals: "none".into(),
        elsewhere: true,
        live_locals: true,
        split_limit: true,
        switches: step::SWITCHES.into(),
        reach: true,
        frame_costs: true,
        reach_also: vec![],
        start_rows: Some(rows),
        fact_vars: vec![],
        stream: true,
    };
    let path = machine::port(|| r.readings.read(&req, cancel))?;
    let wall = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let rd = machine::port(|| r.readings.open(&path))?;
    let load_s = t.elapsed().as_secs_f64();
    let _ = std::fs::remove_file(rdir.join(format!("{}.golden.jsonl.gz", rdir.file_name().unwrap().to_string_lossy())));
    Ok((rd, wall, load_s))
}

/// One attempt's outcome (step.py `attempt`'s return).
struct Attempt {
    res: Value,
    packed: PathBuf,
    /// the harness's checkpoints, in frame order, one per frame
    checkpoints: Vec<Checkpoint>,
    inputs: Vec<Value>,
    secs: f64,
    cpu: f64,
}

/// Step 4 for one target (step.py `attempt`): a fork of the returned engine and the harness with step.py's wiring.
#[allow(clippy::too_many_arguments)]
fn attempt(r: &Resident, rd: &Arc<dyn ReadingSource>, root: &mut Box<dyn Engine>, tg: &Target, goal: &[(String, Vec<i64>)],
           adir: &Path, threads: u32, workers: u32, ready: bool, first_limit: Option<u64>, log: &JobLog,
           cancel: &Arc<AtomicBool>) -> Result<Attempt, String> {
    let (hgoal, text): (Vec<(Key, Vec<Val>)>, String) = if tg.tier == 0 {
        (goal.iter().map(|(k, vs)| (Key::parse(k), vs.clone())).collect(), step::goal_text(goal)?)
    } else {
        let v = match &tg.v {
            TV::Int(n) => *n,
            TV::Text(_) => return Err("a tier 1 or 2 target with a text value".into()),
        };
        (vec![(Key::parse(&tg.k), vec![v])], tg.text())
    };
    let _ = text;
    let doc = doc_of(&**rd);
    let frame_costs = files::or(doc.get("frame_costs"), &json!({})).clone();
    std::fs::create_dir_all(adir).map_err(|e| files::io_err(adir, e))?;
    let (cpu0, t0) = (cpu_now(), Instant::now());
    let branch = machine::port(|| root.fork()).map_err(|e| format!("EngineError: {e}"))?;
    let cfg = HarnessCfg {
        replan_limit: step::REPLAN_LIMIT,
        time_limit_s: step::TIME_LIMIT,
        plan_time_s: step::plan_time(),
        first_plan_time_s: first_limit,
        search: r.search,
        cost_caps: true,
        cost_dmax: frame_costs.get("dmax").and_then(|d| d.as_i64()),
        unobserved_actor_rooms: true,
        reach_extend: true,
        reach_extend_limit: 3,
        refine: true,
        start_check: "needed".into(),
        workdir: adir.join("plans"),
        search_name: r.search_name.clone(),
        tail: r.tail,
    };
    let mut wiring = Wiring::new(r.readings.clone(), &**rd, adir, threads, cancel.clone())?;
    let player = machine::port(|| r.players.player(&WindowCfg { workers, fork_each: step::FORK_EACH, ready, frame_costs: frame_costs.clone() }));
    let ps = PlayState {
        engine: branch,
        state: Obs::default(),
        obs: Facts::new(),
        now: Facts::new(),
        pred: Facts::new(),
        kept: Facts::new(),
        checkpoints: Vec::<Checkpoint>::new(),
        stop_reason: None,
        replan_cause: None,
        records: Vec::new(),
        workdir: adir.to_path_buf(),
    };
    let say = |m: &str| log.say(m);
    log.say(&format!("attempt {}: target {} (tier {}) from frame {}", adir.file_name().unwrap().to_string_lossy(), tg.text(), tg.tier, root.status().frame));
    let mut h = Harness::new(cfg, &r.planners, player, Some(&mut wiring), rd.clone(), hgoal, ps, &say)?;
    let ta = Instant::now();
    let pa = machine::port_s();
    let res = h.run();
    let tb = Instant::now();
    let pb = machine::port_s();
    let checkpoints = std::mem::take(&mut h.ps.checkpoints);
    let inputs = res["inputs"].as_array().cloned().unwrap_or_default();
    let (ps, last) = h.into_parts();
    machine::port(|| ps.engine.quit());
    let packed = last.dir().join(crate::warm::head(&*last).get("packed").and_then(|p| p.as_str()).unwrap_or("reading.tc"));
    let tc = Instant::now();
    let _ = std::fs::write(adir.join("run.json"), pyjson::dumps_default(&res) + "\n");
    if std::env::var("FMI_TRY_PROFILE").is_ok() {
        eprintln!("try profile:   harness run {:.2} ms (ports {:.2} ms), checkpoints {:.2} ms, run.json {:.2} ms",
                  (tb - ta).as_secs_f64() * 1e3, (pb - pa) * 1e3, (tc - tb).as_secs_f64() * 1e3, tc.elapsed().as_secs_f64() * 1e3);
    }
    // the readings the harness made: their golden lines go
    if let Ok(rd) = std::fs::read_dir(adir) {
        for d in rd.flatten() {
            if let Ok(inner) = std::fs::read_dir(d.path()) {
                for f in inner.flatten() {
                    if f.file_name().to_string_lossy().ends_with(".golden.jsonl.gz") {
                        let _ = std::fs::remove_file(f.path());
                    }
                }
            }
        }
    }
    Ok(Attempt { res, packed, checkpoints, inputs, secs: t0.elapsed().as_secs_f64(), cpu: cpu_now() - cpu0 + wiring.cpu_s })
}

/// The targets, or the ones forced (`--targets`), as step.targets gives them.
fn targets_of(r: &Resident, rd: &dyn ReadingSource, job: &Value, job_dir: &Path, facts: &crate::archive::FactMap,
              goal: &[(String, Vec<i64>)]) -> Res<Vec<Target>> {
    let guide = crate::warm::guide(Path::new(job["guide"].as_str().unwrap_or("")))?;
    let reached = crate::warm::reached(rd);
    let empty = vec![];
    let held = job.get("held").and_then(|h| h.as_array()).unwrap_or(&empty);
    let tried = job.get("tried").and_then(|h| h.as_array()).unwrap_or(&empty);
    let rot = job.get("rotation").filter(|x| !x.is_null());
    let ts = step::targets(&reached, &guide, facts, step::root_facts_of(job, job_dir).as_ref(), held, tried, goal, rot)?;
    // job.json's "forced_targets" [[K, V], ...] (the benchmark's tries, several in one process), else `--targets`
    let from_job: Option<Vec<(String, i64)>> = job.get("forced_targets").and_then(|f| f.as_array()).map(|a| {
        a.iter().filter_map(|p| Some((p.get(0)?.as_str()?.to_string(), p.get(1)?.as_i64()?))).collect()
    });
    if let Some(forced) = from_job.as_ref().or(r.forced_targets.as_ref()) {
        // the benchmark's: the log's targets in its order, each with its tier and depth as targets gives them
        let mut out = Vec::new();
        for (k, v) in forced {
            let t = ts.iter().find(|t| t.k == *k && t.v == TV::Int(*v)).cloned()
                .unwrap_or(Target { k: k.clone(), v: TV::Int(*v), tier: if goal.len() == 1 && goal[0].0 == *k { 0 } else { 1 }, depth: 0 });
            out.push(t);
        }
        return Ok(out);
    }
    Ok(ts)
}

/// `step.py run JOB_DIR`: the result as written to JOB_DIR/result.json.
pub fn run_job(job_dir: &Path, r: &Resident, log: &JobLog, cancel: &AtomicBool) -> Value {
    let job_dir = crate::archive::resolve(job_dir);
    let (t_job, cpu_job) = (Instant::now(), cpu_now());
    let port0 = machine::port_s();
    let job = match read_json(&job_dir.join("job.json")) {
        Ok(j) => j,
        Err(e) => {
            let mut out = step::empty_result(&json!({}));
            out["error"] = json!(format!("FileNotFoundError: {e}"));
            let _ = files::write_json(&job_dir.join("result.json"), &out, Some(1));
            return out;
        }
    };
    let mut out = step::empty_result(&job);
    // step.py's cpu dict, in its order ("idle" after the four when measured)
    let mut cpu: Vec<(&str, f64)> = vec![("return", 0.0), ("read", 0.0), ("plan", 0.0), ("play", 0.0)];
    let cancel_arc = Arc::new(AtomicBool::new(cancel.load(Ordering::SeqCst)));
    let mut root_engine: Option<Box<dyn Engine>> = None;
    let mut prof = Prof::new();
    let res: Result<(), Result<ReturnDiffers, String>> = (|| {
        if job.get("seed").and_then(|s| s.as_i64()) != Some(step::SEED) {
            return Err(Err(format!("ValueError: seed {}: E008 runs seed {} only (plan.md)", files::py_str(job.get("seed")), step::SEED)));
        }
        let entry = &job["entry"];
        let goal = step::goal_of(&job);
        let threads = files::as_i64(job.get("threads")).filter(|&t| t != 0).unwrap_or(3) as u32;
        let workers = files::as_i64(job.get("workers")).filter(|&t| t != 0).unwrap_or(3) as u32;
        let run_dir = job_dir.join("run");
        std::fs::create_dir_all(&run_dir).map_err(|e| Err(files::io_err(&run_dir, e)))?;
        // 1. return
        log.say(&format!("job {}: returning to entry {} at frame {}", files::py_str(job.get("job")), files::py_str(entry.get("id")), entry["frame"]));
        let c0 = cpu_now();
        let (root, ret) = boot_entry(r, entry, &run_dir.join("boot")).map_err(Err)?;
        let root = root_engine.insert(root);
        cpu_set(&mut cpu, "return", cpu_now() - c0 + root.cpu_s());
        out["return"] = ret.clone();
        prof.mark("return");
        if !ret["same"].as_bool().unwrap_or(false) {
            return Err(Ok(ReturnDiffers(format!("the return to entry {} differs from its checkpoint: {}",
                                                files::py_str(entry.get("id")), files::py_repr(&ret["differ"])))));
        }
        let state = machine::port(|| root.obs(&ObsReq::default())).map_err(|e| Err(format!("EngineError: {e}")))?;
        let status = root.status().clone();
        let facts_v = step::facts_of_ordered(&state);
        let facts = crate::archive::fact_map(&facts_v);
        let idle_frames = files::as_i64(job.get("idle_frames")).unwrap_or(0);
        if idle_frames != 0 && !files::truthy(job.get("idle_measured")) {
            log.say(&format!("idle: {idle_frames} frames with no input from frame {}", status.frame));
            let c0 = cpu_now();
            let idle = measure_idle(root, idle_frames as u64).map_err(Err)?;
            cpu_set(&mut cpu, "idle", cpu_now() - c0 + idle["cpu_s"].as_f64().unwrap_or(0.0));
            log.say(&format!("idle keys: {} ({}), {} s", files::py_repr(&json!(idle["returns"].as_array().unwrap().iter().take(12).collect::<Vec<_>>())),
                             idle["returns"].as_array().unwrap().len(), idle["seconds"]));
            out["idle"] = idle;
        }
        prof.mark("facts and idle");
        // 2. reading
        let c0 = cpu_now();
        let t = Instant::now();
        let (rd0, why, checked) = choose_reading(r, &job, &facts).map_err(Err)?;
        let load_s = t.elapsed().as_secs_f64();
        let mut rd: Arc<dyn ReadingSource> = match rd0 {
            Some(rd) => {
                out["reading"] = reading_record(&*rd, false, &why, 0.0, load_s);
                rd
            }
            None => {
                let (rd, wall, load_s) = read_fresh(r, &job, &state, &status, threads, log, &cancel_arc).map_err(Err)?;
                out["reading"] = reading_record(&*rd, true, &why, wall, load_s);
                rd
            }
        };
        out["reading"]["candidates"] = json!(checked);
        cpu_add(&mut cpu, "read", cpu_now() - c0);
        log.say(&format!("reading {}: {} ({why})", out["reading"]["path"].as_str().unwrap_or(""),
                         if files::truthy(out["reading"].get("fresh")) { "fresh" } else { "reused" }));
        prof.mark("reading");
        // 3. targets
        let ts = targets_of(r, &*rd, &job, &job_dir, &facts, &goal).map_err(Err)?;
        let by = |n: i64| ts.iter().filter(|x| x.tier == n).count();
        out["targets"] = json!({"by_tier": {"0": by(0), "1": by(1), "2": by(2)},
                                "first": ts.iter().take(20).map(|x| x.json()).collect::<Vec<_>>(), "n": ts.len()});
        log.say(&format!("{} targets: {}", ts.len(), pyjson::compact(&out["targets"]["by_tier"])));
        prof.mark("targets");
        // 4. attempts
        let ready = files::truthy(job.get("ready"));
        let mut played: Option<(Target, Attempt)> = None;
        let mut fresh_again = false;
        // E011's deepening (step::deepen): the deferred targets last, each with its doubled limit
        let deepen = step::deepen();
        let deferred_in = if deepen.is_some() { step::deferred_of(&job) } else { Default::default() };
        let mut queue: std::collections::VecDeque<(Target, u32)> = match deepen {
            Some(_) => step::deepen_order(ts, &deferred_in).into_iter().collect(),
            None => ts.into_iter().map(|t| (t, 0)).collect(),
        };
        let mut n_att = 0;
        while let Some((tg, n_def)) = queue.pop_front() {
            if cancel.load(Ordering::SeqCst) {
                return Err(Err("the guard's kill".into()));
            }
            let adir = run_dir.join(format!("a{n_att:02}"));
            let first_limit = deepen.map(|l0| step::deepen_limit(l0, n_def));
            let att = attempt(r, &rd, root, &tg, &goal, &adir, threads, workers, ready, first_limit, log, &cancel_arc).map_err(Err)?;
            prof.mark("attempt");
            n_att += 1;
            let status_ = step::first_plan_status(att.res["plans"].as_array().map(|a| a.as_slice()).unwrap_or(&[]));
            let rd_packed = rd.dir().join(doc_of(&*rd).get("packed").and_then(|p| p.as_str()).unwrap_or("reading.tc"));
            let made = step::made_reading(&att.packed, &rd_packed);
            if let Some(m) = &made {
                out["readings_made"].as_array_mut().unwrap().push(m.clone());
            }
            let dropped = step::drop_unused(&adir, made.as_ref());
            let (rr, pl, pn) = step::attempt_costs(&att.res, att.cpu);
            cpu_add(&mut cpu, "read", rr);
            cpu_add(&mut cpu, "play", pl);
            cpu_add(&mut cpu, "plan", pn);
            out["probes"].as_array_mut().unwrap().push(json!({"target": [tg.k, tg.v.json()], "tier": tg.tier, "depth": tg.depth,
                "status": status_, "stop": att.res.get("stop"), "seconds": pyjson::round(att.secs, 1),
                "attempt": format!("a{:02}", n_att - 1), "reading": reading_path(&*rd).to_string_lossy(), "dropped": dropped}));
            if let Some(l) = first_limit {
                let pr = out["probes"].as_array_mut().unwrap().last_mut().unwrap();
                pr["limit_s"] = json!(l);
                pr["deferred_before"] = json!(n_def);
            }
            if status_.as_deref() == Some("plan") {
                played = Some((tg, att));
                break;
            }
            if deepen.is_some() && status_.as_deref() == Some("time") {
                // the first plan ran into its limit: deferred, not tried (later tries retry it with twice the limit)
                if out.get("deferred").is_none() {
                    out["deferred"] = json!([]);
                }
                out["deferred"].as_array_mut().unwrap().push(json!([tg.k, tg.v.json(), n_def + 1]));
                log.say(&format!("deferred {} ({} s limit, deferred {} times now)", tg.text(), first_limit.unwrap_or(0), n_def + 1));
                continue;
            }
            out["tried"].as_array_mut().unwrap().push(json!([tg.k, tg.v.json()]));
            if status_.as_deref() == Some(step::NOT_A_PLAN_RETRY) && !files::truthy(out["reading"].get("fresh")) && !fresh_again {
                // the cached reading does not cover the entry: the scope read from the entry's state, the same target
                fresh_again = true;
                let c0 = cpu_now();
                let old = out["reading"].clone();
                let (nrd, wall, load_s) = read_fresh(r, &job, &state, &status, threads, log, &cancel_arc).map_err(Err)?;
                rd = nrd;
                out["reading"] = reading_record(&*rd, true, step::NOT_A_PLAN_RETRY, wall, load_s);
                out["reading"]["replaced"] = json!({"path": old["path"], "why": old["why"]});
                out["reading"]["candidates"] = json!(checked);
                cpu_add(&mut cpu, "read", cpu_now() - c0);
                out["tried"].as_array_mut().unwrap().pop();
                queue.push_front((tg, n_def));
            }
        }
        prof.mark("probes");
        if let Some((tg, mut att)) = played {
            out["tried"].as_array_mut().unwrap().push(json!([tg.k, tg.v.json()]));
            let entry_frame = entry["frame"].as_i64().unwrap_or(0);
            let frames: Vec<i64> = att.checkpoints.iter().map(|c| c.frame as i64).collect();
            let plan = step::entries_plan(entry_frame, &att.inputs, &frames).map_err(Err)?;
            let reached = match att.checkpoints.last() {
                Some(c) => step::target_holds_in(&tg, &goal, &c.facts),
                None => step::target_holds(&tg, &goal, &facts),
            };
            att.res["_checkpoints"] = json!(att.checkpoints.len());
            att.res["_reached"] = json!(reached);
            att.res["_ready"] = json!(ready);
            out["attempt"] = step::attempt_record(&tg, &att.res, att.secs);
            // the entries share the attempt's checkpoints (each a prefix): each checkpoint's text is made once, and the
            // files are written together; the ingest of this process takes them from the stash
            let kept: Vec<Checkpoint> = att.checkpoints.into_iter().filter(|c| c.frame as i64 > entry_frame).collect();
            let shared = Arc::new(crate::archive::SharedCps::new(kept));
            let parent = files::as_i64(entry.get("id")).unwrap_or(-1);
            let new_dir = job_dir.join("new");
            std::fs::create_dir_all(&new_dir).map_err(|e| Err(files::io_err(&new_dir, e)))?;
            let ents: Vec<crate::archive::NewEntry> = plan.iter().map(|(n, evs, _)| crate::archive::NewEntry {
                parent, frame: shared.cps[n - 1].frame as i64, tick: Some(shared.cps[n - 1].tick as i64), events: evs.clone(),
                shared: shared.clone(), n: *n }).collect();
            let errs: Vec<String> = std::thread::scope(|sc| {
                let hs: Vec<_> = ents.iter().enumerate().map(|(i, e)| {
                    let p = new_dir.join(format!("{i}.json.gz"));
                    sc.spawn(move || e.write(&p, None))
                }).collect();
                hs.into_iter().filter_map(|h| h.join().ok().and_then(|r| r.err())).collect()
            });
            if let Some(e) = errs.into_iter().next() {
                return Err(Err(e));
            }
            for (i, (e, (_, _, how))) in ents.into_iter().zip(plan.iter()).enumerate() {
                out["new"].as_array_mut().unwrap().push(json!({"file": format!("new/{i}.json.gz"), "frame": e.frame, "tick": e.tick, "how": how}));
                crate::archive::stash_shared(&new_dir.join(format!("{i}.json.gz")), e);
            }
            log.say(&format!("played {}: stop {}, {} frames, {} entries", tg.text(), files::py_repr(att.res.get("stop").unwrap_or(&Value::Null)),
                             files::py_str(att.res.get("frames")), out["new"].as_array().unwrap().len()));
        }
        prof.mark("new entries");
        // with deepening, an entry whose try deferred a target is not exhausted: the target comes back
        let deferred_now = out.get("deferred").and_then(|d| d.as_array()).map(|d| !d.is_empty()).unwrap_or(false);
        out["exhausted"] = json!(played_none(&out) && !deferred_now);
        out["ok"] = json!(true);
        Ok(())
    })();
    match res {
        Ok(()) => {}
        Err(Ok(ReturnDiffers(m))) => out["error"] = json!(m),
        Err(Err(m)) => {
            out["error"] = json!(m);
            log.line(&format!("error: {m}"));
        }
    }
    if let Some(root) = root_engine {
        // the returned engine is kept for later returns to the same entry (the pool decides)
        let id = files::as_i64(job["entry"].get("id")).unwrap_or(-1);
        machine::port(|| r.returns.lock().unwrap().keep(id, root));
    }
    let mut cpu_v = Map::new();
    let mut total = 0.0;
    for (k, v) in &cpu {
        let x = pyjson::round(*v, 1);
        total += x;
        cpu_v.insert((*k).to_string(), json!(x));
    }
    cpu_v.insert("total".into(), json!(pyjson::round(total, 1)));
    cpu_v.insert("rusage_total".into(), json!(pyjson::round(cpu_now() - cpu_job, 1)));
    out["cpu_s"] = Value::Object(cpu_v);
    let (p, c) = machine::peaks_mb();
    out["peak_rss_mb"] = json!({"python": p, "children": c});
    out["wall_s"] = json!(pyjson::round(t_job.elapsed().as_secs_f64(), 1));
    // fmi's own account (not a reference field; the archive reads none of it): the try's wall in the ports and outside
    let wall = t_job.elapsed().as_secs_f64();
    let ports = machine::port_s() - port0;
    out["explore_s"] = json!({"wall": wall, "ports": ports, "own": wall - ports});
    let _ = files::write_json(&job_dir.join("result.json"), &out, Some(1));
    prof.mark("result");
    prof.print();
    out
}

fn cpu_set(cpu: &mut Vec<(&str, f64)>, k: &'static str, v: f64) {
    match cpu.iter_mut().find(|x| x.0 == k) {
        Some(x) => x.1 = v,
        None => cpu.push((k, v)),
    }
}

fn cpu_add(cpu: &mut [(&str, f64)], k: &str, v: f64) {
    if let Some(x) = cpu.iter_mut().find(|x| x.0 == k) {
        x.1 += v;
    }
}

fn played_none(out: &Value) -> bool {
    out["attempt"].is_null()
}

#[allow(dead_code)]
fn unused(_: &dyn Readings) {}
