//! step.py's `run_job`, line for line, on the ports: one try of the explorer (return, idle measurement, the reading,
//! the targets, the attempts through the harness, the entries reached), writing JOB_DIR/result.json and
//! JOB_DIR/new/K.json.gz in the reference's formats. The ports stay warm across tries in `Resident` (one process).

use crate::driver::{JobLog, Step};
use crate::files::{self, read_json, Res};
use crate::harness::{self, Harness, HarnessCfg, PlannerCache};
use crate::machine;
use crate::ports::{EntryRef, PlayExtras, PlayerHost, Readings, Returns, ReadReq, WindowCfg};
use crate::mech::{self, Extra, Switches, TestedT, TryNovel, Trigger};
use crate::progress;
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
    /// E012's ports outside this crate (T, the wait); unused with every E012 switch off
    pub mech: mech::Mech,
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
            let own: f64 = self.rows.iter().map(|r| r.1 - r.2).sum();
            eprintln!("try profile: own {:.2} ms", own * 1e3);
            for (w, t, n) in secs {
                eprintln!("try profile:   section {w}: {:.2} ms in {n} calls", t * 1e3);
            }
        }
    }
}

fn cpu_now() -> f64 {
    machine::thread_cpu_s()
}

/// The checkpoint's facts (a facts.py map, JSON) equal the state's, shown without parsing the checkpoint's keys: every
/// key of `got` is in the checkpoint with its value as an int, or is absent from it and 0, and the checkpoint has no
/// other key. Then `facts_of_json(want).diff(got)` is empty. False when that is not shown (the caller diffs them).
pub fn same_facts(want: &Value, got: &Facts) -> bool {
    let Some(m) = want.as_object() else { return false };
    let mut t = String::with_capacity(48);
    let mut matched = 0;
    for &(k, v) in got.iter() {
        t.clear();
        crate::archive::write_key(&mut t, k);
        match m.get(t.as_str()) {
            Some(x) => {
                if x.as_i64() != Some(v) {
                    return false;
                }
                matched += 1;
            }
            None if v != 0 => return false,
            None => {}
        }
    }
    matched == m.len()
}

/// E014 (FMI_GAME_ID): the engines run another game than MI's DOS CD release. Its engine may set its own boot param
/// (ScummVM's copy-protection skip for some game ids, scumm.cpp); the host still passes 0, so the boot checks accept
/// what the engine holds.
pub fn other_game() -> bool {
    std::env::var("FMI_GAME_ID").is_ok_and(|v| !v.is_empty())
}

/// replay.compare_checkpoint: the differences between a checkpoint and the engine's state and status.
pub fn compare_checkpoint(want: &Value, state: &Obs, status: &Status) -> Map<String, Value> {
    let mut out = Map::new();
    let got = facts_of(state);
    // equal facts shown in one pass (FMI_TRY_MEMO), else the diff as round 1 made it
    let mut d: Vec<(String, Val, Val)> = if switch("FMI_TRY_MEMO") && same_facts(&want[1], &got) {
        Vec::new()
    } else {
        let wf = start::facts_of_json(&want[1]);
        wf.diff(&got).into_iter().map(|(k, a, b)| (k.text(), a, b)).collect()
    };
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
    if obs.boot_param != 0 && !other_game() {
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

/// An E011 switch of this crate: on unless its variable is "0" (off: round 1's code).
fn switch(name: &str) -> bool {
    std::env::var(name).map(|v| v != "0").unwrap_or(true)
}

/// The next idle frame (`frame`, one after this stop) and the facts' changes to it, appended to `out`. With the
/// feature `step_changes` (fmi-core's `Engine::step_changes`, IFACE e011-r2-own in COORD.md) the engine gives them in
/// one call, from Step's delta. Without it, today's calls: a run and the facts (the engine port's), and their diff
/// from the facts of the frame before, kept in `prev` (this try's own work).
#[cfg(not(feature = "step_changes"))]
fn idle_frame(b: &mut Box<dyn Engine>, frame: u64, prev: &mut Option<Facts>, out: &mut Vec<(Key, Val, Val)>) -> fmi_core::EResult<Status> {
    if prev.is_none() {
        *prev = Some(machine::port(|| b.facts())?);
    }
    let st = machine::port(|| b.run(&RunSpec { frame: Some(frame), ..Default::default() }))?;
    let f = machine::port(|| b.facts())?;
    step::diff_into(prev.as_ref().unwrap(), &f, out);
    *prev = Some(f);
    Ok(st)
}

#[cfg(feature = "step_changes")]
fn idle_frame(b: &mut Box<dyn Engine>, frame: u64, _prev: &mut Option<Facts>, out: &mut Vec<(Key, Val, Val)>) -> fmi_core::EResult<Status> {
    let n = frame.checked_sub(b.status().frame).filter(|&n| n >= 1).unwrap_or(1) as u32;
    let (st, ch) = machine::port(|| b.step_changes(n))?;
    out.extend_from_slice(&ch);
    Ok(st)
}

/// E009 fix a, evaluated as the frames come (FMI_IDLE_CHANGES, on by default): a fork of the returned engine run
/// `frames` frames with no input, each frame's changes given to the idle rule; no frame's whole facts are kept. The
/// same frames and the same rule as `measure_idle_whole`: a run to a frame with no other stop condition stops at that
/// frame or ends (the bridge's Run), so each frame here is the one the run to `f0 + i` stops at, and is checked so.
fn measure_idle(root: &mut Box<dyn Engine>, frames: u64) -> Result<Value, String> {
    if !switch("FMI_IDLE_CHANGES") {
        return measure_idle_whole(root, frames);
    }
    let t = Instant::now();
    let mut b = machine::port(|| root.fork()).map_err(|e| format!("EngineError: {e}"))?;
    let f0 = b.status().frame;
    let mut rule = step::IdleRule::new();
    let mut prev: Option<Facts> = None;
    let mut ch: Vec<(Key, Val, Val)> = Vec::with_capacity(64);
    let mut err = None;
    for i in 1..=frames {
        ch.clear();
        match idle_frame(&mut b, f0 + i, &mut prev, &mut ch) {
            Ok(st) if st.frame == f0 + i => rule.frame(i as i64, &ch),
            Ok(st) => {
                err = Some(format!("the idle run stopped at frame {} ({}), not {}", st.frame, st.reason, f0 + i));
                break;
            }
            Err(e) => {
                err = Some(format!("EngineError: {e}"));
                break;
            }
        }
    }
    drop(prev);
    let frame_end = b.status().frame;
    let cpu = b.cpu_s();
    machine::port(|| b.quit());
    if let Some(e) = err {
        return Err(e);
    }
    let mut out = rule.result();
    out["frame_end"] = json!(frame_end);
    out["cpu_s"] = json!(pyjson::round(cpu, 1));
    out["frames"] = json!(frames);
    out["frame0"] = json!(f0);
    out["seconds"] = json!(pyjson::round(t.elapsed().as_secs_f64(), 1));
    Ok(out)
}

/// E009 fix a as round 1 ran it: a fork of the returned engine run `frames` frames with no input, the facts taken at
/// each, the rule run over them all at the end.
fn measure_idle_whole(root: &mut Box<dyn Engine>, frames: u64) -> Result<Value, String> {
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

/// What the reading choice reads of a reading, made once per reading in the process (FMI_TRY_MEMO; a reading's files
/// never change once made): its start facts as texts and, when they map onto fmi-core's packed facts one to one
/// (every key its key's own text, every value an int), packed; and its task facts' keys, as texts.
struct ReadingMemo {
    start: crate::archive::FactMap,
    packed: Option<Facts>,
    keys: HashSet<String>,
}

fn reading_memo(rd: &dyn ReadingSource) -> Arc<ReadingMemo> {
    // keyed by the packed file's sha1 and the reading.json warm.rs gives (one parse per text), which the memo holds
    type Memo = Mutex<std::collections::HashMap<([u8; 20], usize), (Arc<Value>, Arc<ReadingMemo>)>>;
    static C: std::sync::OnceLock<Memo> = std::sync::OnceLock::new();
    let doc = doc_of(rd);
    let id = (rd.id(), Arc::as_ptr(&doc) as usize);
    if let Some((_, m)) = C.get_or_init(Default::default).lock().unwrap().get(&id) {
        return m.clone();
    }
    let sj = &doc["start"]["facts"];
    let m = Arc::new(ReadingMemo { start: crate::archive::fact_map(sj), packed: step::packed_facts(sj), keys: task_keys(rd) });
    let mut c = C.get_or_init(Default::default).lock().unwrap();
    if c.len() >= 64 {
        if let Some(k) = c.keys().next().cloned() {
            c.remove(&k);
        }
    }
    c.insert(id, (doc.clone(), m.clone()));
    m
}

/// The entry's facts as the try reads them: round 1's text map, or fmi-core's packed facts (FMI_TRY_MEMO), read by
/// key text alike (`step::PackedLook`).
pub enum EntryFacts {
    Text(crate::archive::FactMap),
    Packed(Facts),
}

impl crate::archive::FactLook for EntryFacts {
    fn look(&self, k: &str) -> Option<i64> {
        match self {
            EntryFacts::Text(m) => m.look(k),
            EntryFacts::Packed(f) => step::PackedLook(f).look(k),
        }
    }
    fn owners(&self) -> Vec<(i64, i64)> {
        match self {
            EntryFacts::Text(m) => m.owners(),
            EntryFacts::Packed(f) => step::PackedLook(f).owners(),
        }
    }
}

impl EntryFacts {
    fn text_map(&self) -> std::borrow::Cow<'_, crate::archive::FactMap> {
        match self {
            EntryFacts::Text(m) => std::borrow::Cow::Borrowed(m),
            EntryFacts::Packed(f) => std::borrow::Cow::Owned(f.iter().map(|(k, v)| (k.text(), *v)).collect()),
        }
    }
}

/// The reading choice's outcome: the reading or None, why, the candidates checked.
type Choice = (Option<Arc<dyn ReadingSource>>, String, Vec<Value>);

/// Step 2 on the cached readings (step.py `choose_reading`): (the reading or None, why, the candidates checked).
fn choose_reading(r: &Resident, job: &Value, facts: &EntryFacts) -> Result<Choice, String> {
    if !switch("FMI_TRY_MEMO") {
        return choose_reading_r1(r, job, &facts.text_map());
    }
    let cands = step::cached_readings_with(job, true);
    if cands.is_empty() {
        return Ok((None, "no cached reading".into(), vec![]));
    }
    let mut checked = Vec::new();
    for p in cands {
        let rd = machine::port(|| r.readings.open(&p))?;
        let m = reading_memo(&*rd);
        // one diff of the start facts and the entry's, read for the task keys and for them with E009's; on the packed
        // facts when both are (the same differences, their keys' texts made for them alone)
        let d = match (&m.packed, facts) {
            (Some(sp), EntryFacts::Packed(ep)) => step::diff_packed(sp, ep),
            _ => step::diff(&m.start, &facts.text_map()),
        };
        let (mut ok, det) = step::reuse_of(&d, |k| m.keys.contains(k));
        let mut row = json!({"path": p.to_string_lossy(), "reusable": ok});
        for (k, v) in det.as_object().unwrap() {
            row[k] = v.clone();
        }
        if !ok && job.get("reuse").and_then(|x| x.as_str()) == Some("e009") {
            let extra = machine::port(|| r.readings.reuse_keys(&*rd))?;
            let (ok2, det2) = step::reuse_of(&d, |k| m.keys.contains(k) || extra.contains(k));
            ok = ok2;
            row["reusable"] = json!(ok);
            row["reuse_keys"] = json!(extra.len());
            row["e009"] = det2;
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

/// The reading choice as round 1 made it (FMI_TRY_MEMO=0).
fn choose_reading_r1(r: &Resident, job: &Value, facts: &crate::archive::FactMap) -> Result<Choice, String> {
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
    /// the attempt's engine at its end, kept when asked (E012's affordance step runs from it), else quit
    engine: Option<Box<dyn Engine>>,
}

/// Step 4 for one target (step.py `attempt`): a fork of the returned engine and the harness with step.py's wiring.
#[allow(clippy::too_many_arguments)]
fn attempt(r: &Resident, rd: &Arc<dyn ReadingSource>, root: &mut Box<dyn Engine>, tg: &Target, goal: &[(String, Vec<i64>)],
           adir: &Path, threads: u32, workers: u32, ready: bool, first_limit: Option<u64>, log: &JobLog,
           cancel: &Arc<AtomicBool>, x: &PlayExtras, keep: bool) -> Result<Attempt, String> {
    // the harness's goal (step::target_goal: E012's one target per key gives the key a value list); tier 0's goal has
    // reader-scope's form
    if tg.tier == 0 {
        step::goal_text(goal)?;
    }
    let hgoal: Vec<(Key, Vec<Val>)> = step::target_goal(tg, goal)?;
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
    let player = machine::port(|| r.players.player_with(&WindowCfg { workers, fork_each: step::FORK_EACH, ready, frame_costs: frame_costs.clone() }, x));
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
    let (ps, last, player) = h.into_all();
    let engine = if keep {
        Some(ps.engine)
    } else {
        machine::port(|| ps.engine.quit());
        None
    };
    // the window search's player: its reading data and game go off this thread (FMI_PLAYER_DONE), or here (round 1)
    if switch("FMI_PLAYER_DONE") {
        r.players.done(player);
    } else {
        drop(player);
    }
    let packed = last.dir().join(crate::warm::head(&*last).get("packed").and_then(|p| p.as_str()).unwrap_or("reading.tc"));
    let tc = Instant::now();
    let _ = std::fs::write(adir.join("run.json"), pyjson::dumps_default(&res) + "\n");
    if std::env::var("FMI_TRY_PROFILE").is_ok() {
        eprintln!("try profile:   harness run {:.2} ms (ports {:.2} ms), its end (the player's) {:.2} ms, run.json {:.2} ms",
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
    Ok(Attempt { res, packed, checkpoints, inputs, secs: t0.elapsed().as_secs_f64(), cpu: cpu_now() - cpu0 + wiring.cpu_s, engine })
}

/// A guide's atoms (step::guide_atoms), kept for the last guide seen (warm.rs keeps the guide while its file is the
/// same; the guide is held here with its atoms, so the same allocation is the same guide).
fn guide_atoms(g: &Arc<Value>) -> Arc<step::Atoms> {
    type Memo = Mutex<Option<(Arc<Value>, Arc<step::Atoms>)>>;
    static C: std::sync::OnceLock<Memo> = std::sync::OnceLock::new();
    let mut c = C.get_or_init(Default::default).lock().unwrap();
    if let Some((k, a)) = c.as_ref() {
        if Arc::ptr_eq(k, g) {
            return a.clone();
        }
    }
    let a = Arc::new(step::guide_atoms(g));
    *c = Some((g.clone(), a.clone()));
    a
}

/// The targets, or the ones forced (`--targets`), as step.targets gives them.
fn targets_of(r: &Resident, rd: &dyn ReadingSource, job: &Value, job_dir: &Path, facts: &EntryFacts,
              goal: &[(String, Vec<i64>)], t: Option<&TestedT>) -> Res<Vec<Target>> {
    let guide = crate::warm::guide(Path::new(job["guide"].as_str().unwrap_or("")))?;
    let reached = crate::warm::reached(rd);
    let empty = vec![];
    let held = job.get("held").and_then(|h| h.as_array()).unwrap_or(&empty);
    let tried = job.get("tried").and_then(|h| h.as_array()).unwrap_or(&empty);
    let rot = job.get("rotation").filter(|x| !x.is_null());
    let ts = if step::per_key() {
        // E012's one target per key (plan.md section 3), with the T tier: the keys an action of the try's reading tests
        let has = t.map(|t| move |k: &str| t.has(Key::parse(k)));
        step::targets_per_key(&reached, &guide_atoms(&guide), facts, step::root_facts_of_memo(job, job_dir).as_deref(), held, tried, goal,
                              rot, has.as_ref().map(|h| h as step::TestedKey))?
    } else if switch("FMI_TRY_MEMO") {
        step::targets_with(&reached, &guide_atoms(&guide), facts, step::root_facts_of_memo(job, job_dir).as_deref(), held, tried, goal, rot)?
    } else {
        step::targets(&reached, &guide, facts, step::root_facts_of(job, job_dir).as_ref(), held, tried, goal, rot)?
    };
    // job.json's "forced_targets" [[K, V], ...] (the benchmark's tries, several in one process), else `--targets`
    let from_job: Option<Vec<(String, i64)>> = job.get("forced_targets").and_then(|f| f.as_array()).map(|a| {
        a.iter().filter_map(|p| Some((p.get(0)?.as_str()?.to_string(), p.get(1)?.as_i64()?))).collect()
    });
    if let Some(forced) = from_job.as_ref().or(r.forced_targets.as_ref()) {
        // the benchmark's: the log's targets in its order, each with its tier and depth as targets gives them
        let mut out = Vec::new();
        for (k, v) in forced {
            // (a value list of E012's one target per key gives a forced value of it its tier and depth)
            let t = ts.iter().find(|t| t.k == *k && t.v.ints().contains(v))
                .map(|t| Target { k: k.clone(), v: TV::Int(*v), tier: t.tier, depth: t.depth })
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
    // E012's mechanisms in the try (plan.md, "How a try runs"); every switch off: none runs, the try is round 2's
    let sw = Switches::from_env();
    let mut e12 = Map::new();
    // E013 (FMI_FREE, component free): a free try plans nothing; its offers follow the wait (free.rs)
    let free_try = crate::free::job_is_free(&job);
    let mut free_rec: Option<Value> = None;
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
        let facts = if switch("FMI_TRY_MEMO") {
            EntryFacts::Packed(facts_of(&state))
        } else {
            EntryFacts::Text(crate::archive::fact_map(&step::facts_of_ordered(&state)))
        };
        let idle_frames = files::as_i64(job.get("idle_frames")).unwrap_or(0);
        // E012 (FMI_WAIT): the wait, after the reading, replaces the idle measurement and feeds E009's idle rule from
        // its own first frames (plan.md 1, "In the try")
        let idle_due = idle_frames != 0 && !files::truthy(job.get("idle_measured"));
        if idle_due && !sw.wait {
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
            // E013: a free try needs a reading for T only: the newest cached reading of the scope when none is reusable
            None if free_try && !checked.is_empty() => {
                let p = PathBuf::from(checked[0]["path"].as_str().unwrap_or(""));
                let rd = machine::port(|| r.readings.open(&p)).map_err(Err)?;
                out["reading"] = reading_record(&*rd, false, &format!("T only: {why}"), 0.0, t.elapsed().as_secs_f64());
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
        // E012, 2: T (the reading's tested atoms) and the try's novelty over T and the rooms
        let entry_frame_u = status.frame;
        let entry_facts = if sw.any() || free_try { facts_of(&state) } else { Facts::new() };
        let mut extras: Vec<Extra> = Vec::new();
        let mut dead: Vec<Value> = Vec::new();
        let mut free_keys: Option<TestedT> = None;
        let (tt, nov): (Option<TestedT>, Option<TryNovel>) = if sw.any() || free_try {
            let t0 = Instant::now();
            let port = r.mech.tested.as_ref().ok_or_else(|| Err("E012: the E012 switches read T, and no tested port is wired".to_string()))?;
            let t = port.tested(&*rd).map_err(|e| Err(format!("E012 tested: {e}")))?;
            // E014 (FMI_NOVEL_VERBS, FMI_NOVEL_GUIDE): T with the interface's and the guide's atoms
            let t = if ["FMI_NOVEL_VERBS", "FMI_NOVEL_GUIDE"].iter().any(|k| std::env::var(k).is_ok_and(|v| v == "1")) {
                let mut flat = t.flat.clone();
                let n = mech::novelty_extra(&mut flat, &job, &doc_of(&*rd)["scope"], &entry_facts);
                let mut rec = t.record.clone();
                rec["e014_extra"] = json!(n);
                TestedT::new(flat, rec)
            } else {
                t
            };
            let atoms = progress::t_atoms(&t.flat, rd.domains());
            if sw.select {
                out["tested"] = progress::atoms_json(&atoms);
            }
            let nov = if free_try {
                // E013: a free try's novelty is over its T merged with the archive's (job.json "novelty"), each atom once
                let arch = progress::atoms_of_json(job.get("novelty").and_then(|n| n.get("t")));
                let mut merged = atoms.clone();
                let have: HashSet<progress::TAtom> = atoms.iter().cloned().collect();
                merged.extend(arch.iter().filter(|a| !have.contains(*a)).cloned());
                free_keys = Some(crate::free::merged_keys(&t, &arch));
                TryNovel::new(&job, &merged, &entry_facts)
            } else {
                TryNovel::new(&job, &atoms, &entry_facts)
            };
            let mut rec = t.record.clone();
            rec["atoms"] = json!(atoms.len());
            rec["keys"] = json!(t.keys().len());
            rec["novelty"] = json!(nov.source);
            rec["open"] = json!(nov.open());
            rec["seconds"] = json!(t0.elapsed().as_secs_f64());
            e12.insert("t".into(), rec);
            (Some(t), Some(nov))
        } else {
            (None, None)
        };
        // E012, 3 (FMI_WAIT): time passes; the stops become candidate entries, kept by novelty
        let mut waited: Option<mech::Waited> = None;
        if sw.wait || free_try {
            let (t, nov) = (tt.as_ref().unwrap(), nov.as_ref().unwrap());
            let port = r.mech.wait.as_ref().ok_or_else(|| Err("E012: FMI_WAIT=1, and no wait port is wired".to_string()))?;
            let c0 = cpu_now();
            let w = port.wait(root, &t.flat, &*rd, if idle_due { idle_frames as u64 } else { 0 }).map_err(|e| Err(format!("E012 wait: {e}")))?;
            cpu_set(&mut cpu, "wait", cpu_now() - c0 + w.cpu_s);
            if idle_due {
                if let Some(idle) = &w.idle {
                    out["idle"] = idle.clone();
                }
            }
            let mut kept = 0;
            for s in &w.stops {
                let n = nov.novel(&s.checkpoint.facts);
                if !n.is_empty() {
                    nov.keep(&n);
                    kept += 1;
                    extras.push(Extra { how: "wait", events: vec![], cps: vec![s.checkpoint.clone()], novel: n });
                }
            }
            if let Some((reason, frame)) = &w.ended {
                if fmi_core::is_game_end(reason) {
                    dead.push(mech::dead_json(reason, *frame, &[], "wait"));
                    // E014 (FMI_GOAL_END): the wait that ends the game through a script is also the goal's entry
                    if let Some(x) = w.ended_at.and_then(|(f, t)| mech::end_extra(reason, f, t, vec![], &[])) {
                        extras.push(x);
                    }
                }
            }
            let mut rec = w.record.clone();
            rec["kept"] = json!(kept);
            log.say(&format!("wait: {} stops, {} kept, end {}, {:.2} s", w.stops.len(), kept, w.end, w.seconds));
            e12.insert("wait".into(), rec);
            waited = Some(w);
            prof.mark("wait");
        }
        // E013 (component free): the free try's offers at the entry and the wait's stops, delays, one more level
        if free_try {
            let (t, nv) = (tt.as_ref().unwrap(), nov.as_ref().unwrap());
            let wait = r.mech.wait.clone().ok_or_else(|| Err("E013 free: no wait port is wired".to_string()))?;
            let dead_moves: Vec<Value> = job.get("dead").and_then(|d| d.as_array()).cloned().unwrap_or_default();
            let stops: Vec<mech::Stop> = waited.as_ref().map(|w| w.stops.clone()).unwrap_or_default();
            let cx = crate::free::Ctx { t, keys: free_keys.as_ref().unwrap_or(t), nov: nv, wait, rd: &*rd, dead_moves: &dead_moves,
                                        cancel };
            let c0 = cpu_now();
            let f = machine::port(|| crate::free::explore(root, &stops, &cx)).map_err(Err)?;
            cpu_set(&mut cpu, "free", cpu_now() - c0 + f.cpu_s);
            log.say(&format!("free: {} offers, {} trials, {} kept, {} dead, {:.2} s", f.record["offers"], f.record["trials"],
                             f.kept.len(), f.dead.len(), f.record["seconds"].as_f64().unwrap_or(0.0)));
            extras.extend(f.kept);
            dead.extend(f.dead);
            free_rec = Some(f.record);
            prof.mark("free");
        }
        // 3. targets (none for a free try)
        let ts = if free_try { Vec::new() } else { targets_of(r, &*rd, &job, &job_dir, &facts, &goal, tt.as_ref()).map_err(Err)? };
        let by = |n: i64| ts.iter().filter(|x| x.tier == n).count();
        out["targets"] = json!({"by_tier": {"0": by(0), "1": by(1), "2": by(2)},
                                "first": ts.iter().take(20).map(|x| x.json()).collect::<Vec<_>>(), "n": ts.len()});
        log.say(&format!("{} targets: {}", ts.len(), pyjson::compact(&out["targets"]["by_tier"])));
        prof.mark("targets");
        // 4. attempts
        let ready = files::truthy(job.get("ready"));
        let mut played: Option<(Target, Attempt, Option<usize>)> = None;
        let mut fresh_again = false;
        // E011's deepening (step::deepen): the deferred targets last, each with its doubled limit
        let deepen = step::deepen();
        let deferred_in = if deepen.is_some() { step::deferred_of(&job) } else { Default::default() };
        // each target with its deferral count and, for E012's goal again from a wait's stop, the stop's index
        let mut queue: std::collections::VecDeque<(Target, u32, Option<usize>)> = match deepen {
            Some(_) => step::deepen_order(ts, &deferred_in).into_iter().map(|(t, n)| (t, n, None)).collect(),
            None => ts.into_iter().map(|t| (t, 0, None)).collect(),
        };
        // E012 (FMI_DELAY_FORK): the window search's digest keys and novelty, from the try
        let extras_of = |nov: Option<&TryNovel>| -> PlayExtras {
            match (sw.delay, tt.as_ref(), nov) {
                (true, Some(t), Some(n)) => PlayExtras { tested_keys: Some(Arc::new(t.keys().to_vec())), novel_of: Some(n.snapshot()) },
                _ => PlayExtras::default(),
            }
        };
        let keep_engine = sw.afford;
        let mut goal_again = false;
        let mut goal_again_rec: Vec<Value> = Vec::new();
        let mut n_att = 0;
        while let Some((tg, n_def, from_stop)) = queue.pop_front() {
            if cancel.load(Ordering::SeqCst) {
                return Err(Err("the guard's kill".into()));
            }
            let adir = run_dir.join(format!("a{n_att:02}"));
            let first_limit = deepen.map(|l0| step::deepen_limit(l0, n_def));
            let x = extras_of(nov.as_ref());
            let att = match from_stop {
                None => attempt(r, &rd, root, &tg, &goal, &adir, threads, workers, ready, first_limit, log, &cancel_arc, &x, keep_engine).map_err(Err)?,
                Some(si) => {
                    // the goal again from a wait's stop: the entry's state run on with no input to the stop's frame
                    // (a fork of the entry; the stop's checkpoint is held to it)
                    let stop = &waited.as_ref().unwrap().stops[si];
                    let mut b = engine_at(root, &stop.checkpoint).map_err(Err)?;
                    let a = attempt(r, &rd, &mut b, &tg, &goal, &adir, threads, workers, ready, first_limit, log, &cancel_arc, &x, keep_engine);
                    machine::port(|| b.quit());
                    a.map_err(Err)?
                }
            };
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
            if let Some(si) = from_stop {
                let f = waited.as_ref().unwrap().stops[si].frame;
                out["probes"].as_array_mut().unwrap().last_mut().unwrap()["from_stop"] = json!(f);
                goal_again_rec.push(json!({"stop": f, "status": status_, "stop_reason": att.res.get("stop"), "seconds": pyjson::round(att.secs, 1)}));
            }
            if status_.as_deref() == Some("plan") {
                played = Some((tg, att, from_stop));
                break;
            }
            if let Some(e) = att.engine {
                machine::port(|| e.quit());
            }
            // E012 (FMI_WAIT): the goal proven dead or deferred from the entry is requested again from each of the
            // wait's stops, on the same reading, before the other targets (plan.md 1, "The planner plans only up to
            // where the player must wait")
            if from_stop.is_some() {
                continue;
            }
            let goal_dead = matches!(status_.as_deref(), Some("unsolvable") | Some("incomplete"))
                || (deepen.is_some() && status_.as_deref() == Some("time"));
            if tg.tier == 0 && goal_dead && !goal_again {
                if let Some(w) = waited.as_ref().filter(|w| !w.stops.is_empty()) {
                    goal_again = true;
                    log.say(&format!("the goal again from the wait's {} stops", w.stops.len()));
                    for si in (0..w.stops.len()).rev() {
                        queue.push_front((tg.clone(), n_def, Some(si)));
                    }
                }
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
                queue.push_front((tg, n_def, None));
            }
        }
        prof.mark("probes");
        if goal_again {
            e12.insert("goal_again".into(), json!(goal_again_rec));
        }
        let mut afford_recs: Vec<Value> = Vec::new();
        let mut afford_offered: Option<usize> = None;
        let parent = files::as_i64(entry.get("id")).unwrap_or(-1);
        if let Some((tg, mut att, played_from)) = played {
            // the goal again from a stop is the entry's goal target, tried once in its records
            let rec = json!([tg.k, tg.v.json()]);
            if played_from.is_none() || !out["tried"].as_array().unwrap().contains(&rec) {
                out["tried"].as_array_mut().unwrap().push(rec);
            }
            let entry_frame = entry["frame"].as_i64().unwrap_or(0);
            let frames: Vec<i64> = att.checkpoints.iter().map(|c| c.frame as i64).collect();
            let plan = step::entries_plan(entry_frame, &att.inputs, &frames).map_err(Err)?;
            let reached = match att.checkpoints.last() {
                Some(c) => step::target_holds_in(&tg, &goal, &c.facts),
                None => step::target_holds_in(&tg, &goal, &facts),
            };
            att.res["_checkpoints"] = json!(att.checkpoints.len());
            att.res["_reached"] = json!(reached);
            att.res["_ready"] = json!(ready);
            out["attempt"] = step::attempt_record(&tg, &att.res, att.secs);
            let att_engine = att.engine.take();
            let att_stop: Option<String> = att.res.get("stop").and_then(|s| s.as_str()).map(|s| s.to_string());
            // the entries share the attempt's checkpoints (each a prefix): each checkpoint's text is made once, and the
            // files are written together; the ingest of this process takes them from the stash
            let kept: Vec<Checkpoint> = att.checkpoints.into_iter().filter(|c| c.frame as i64 > entry_frame).collect();
            // E012: the attempt's entries are kept, so what they hold is novel no more in this try; the affordance
            // step's entries continue the attempt's movie and checkpoints
            if let Some(n) = nov.as_ref() {
                for c in &kept {
                    n.keep_facts(&c.facts);
                }
            }
            let prefix_cps: Vec<Checkpoint> = if att_engine.is_some() { kept.clone() } else { Vec::new() };
            let last_cp = kept.last().map(|c| c.frame as i64).unwrap_or(entry_frame);
            let shared = Arc::new(crate::archive::SharedCps::new(kept));
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
            // E012, 5 (FMI_DELAY_FORK): the delay fork's candidates, kept by the try's novelty
            if sw.delay {
                let ws = att.res["windows"].as_array().cloned().unwrap_or_default();
                let nv = nov.as_ref().unwrap();
                let mut rec = mech::delay_record(&ws);
                let mut n_kept = 0;
                for (_, _, inputs, cp) in mech::delay_candidates(&ws) {
                    let n = nv.novel(&cp.facts);
                    if n.is_empty() {
                        continue;
                    }
                    nv.keep(&n);
                    n_kept += 1;
                    extras.push(Extra { how: "delay", events: inputs, cps: vec![cp], novel: n });
                }
                rec["kept"] = json!(n_kept);
                rec["summary"] = att.res["window_summary"].get("delay").cloned().unwrap_or(Value::Null);
                e12.insert("delay".into(), rec);
            }
            // E012, 5 (component end): a move whose run ends in the game's own end is dead; it has no entry
            if sw.any() {
                if let Some(reason) = mech::game_end_of_stop(att_stop.as_deref()) {
                    let evs: Vec<Value> = att.inputs.iter().filter(|e| files::as_i64(e.get("frame")).unwrap_or(0) > last_cp).cloned().collect();
                    dead.push(mech::dead_json(&reason, att.res["frame_end"].as_u64().unwrap_or(0), &evs, "attempt"));
                    // E014 (FMI_GOAL_END): an attempt that ends the game through a script is also the goal's entry,
                    // its inputs from the try's entry on (the harness's end status: frame_end, tick_end)
                    if let (Some(f), Some(t)) = (att.res["frame_end"].as_u64(), att.res["tick_end"].as_u64()) {
                        let all: Vec<Value> = att.inputs.iter().filter(|e| files::as_i64(e.get("frame")).map(|x| x > entry_frame_u as i64 && x <= f as i64).unwrap_or(false)).cloned().collect();
                        if let Some(x) = mech::end_extra(&reason, f, t, all, &[]) {
                            extras.push(x);
                        }
                    }
                }
            }
            // E012, 6 (FMI_AFFORD): the interface's own inputs where the model could not act (R0, the same empty window)
            if let Some(mut e) = att_engine {
                if let (Some(trig), Some(t), Some(nv)) = (mech::trigger_of_stop(att_stop.as_deref()), tt.as_ref(), nov.as_ref()) {
                    let f = e.status().frame as i64;
                    let prefix: Vec<Value> = att.inputs.iter().filter(|x| files::as_i64(x.get("frame")).unwrap_or(i64::MAX) <= f).cloned().collect();
                    let c0 = cpu_now();
                    let a = run_afford(r, &sw, &mut e, trig, t, nv, &*rd, &job, &prefix, &prefix_cps, log);
                    machine::port(|| e.quit());
                    let a = a.map_err(Err)?;
                    cpu_set(&mut cpu, "afford", cpu_now() - c0 + a.cpu_s);
                    afford_recs.push(a.record);
                    extras.extend(a.kept);
                    dead.extend(a.dead);
                } else {
                    machine::port(|| e.quit());
                }
            }
        }
        // E012, 6 (FMI_AFFORD): no target of the try plans: the interface's own inputs at the entry
        if sw.afford && !free_try && played_none(&out) {
            let (t, nv) = (tt.as_ref().unwrap(), nov.as_ref().unwrap());
            let c0 = cpu_now();
            let a = run_afford(r, &sw, root, Trigger::NoPlan, t, nv, &*rd, &job, &[], &[], log).map_err(Err)?;
            cpu_set(&mut cpu, "afford", cpu_now() - c0 + a.cpu_s);
            afford_offered = Some(a.offered);
            afford_recs.push(a.record);
            extras.extend(a.kept);
            dead.extend(a.dead);
        }
        prof.mark("new entries");
        // E012, 7: the commit: the candidate entries kept by novelty (after the attempt's), and the dead moves
        let n_extra = extras.len();
        if sw.any() || free_try {
            let mut by_how = Map::new();
            for x in &extras {
                let n = by_how.get(x.how).and_then(|v| v.as_u64()).unwrap_or(0);
                by_how.insert(x.how.into(), json!(n + 1));
            }
            let first = out["new"].as_array().unwrap().len();
            let items = mech::write_extras(&job_dir.join("new"), first, parent, entry_frame_u, extras).map_err(Err)?;
            out["new"].as_array_mut().unwrap().extend(items);
            if !afford_recs.is_empty() {
                e12.insert("afford".into(), json!(afford_recs));
            }
            e12.insert("kept".into(), Value::Object(by_how));
            e12.insert("dead".into(), json!(dead.len()));
            out["dead"] = json!(dead);
            if n_extra > 0 || !dead.is_empty() {
                log.say(&format!("E012: {n_extra} more entries ({}), {} dead moves", pyjson::compact(&e12["kept"]), dead.len()));
            }
        }
        // with deepening, an entry whose try deferred a target is not exhausted: the target comes back
        let deferred_now = out.get("deferred").and_then(|d| d.as_array()).map(|d| !d.is_empty()).unwrap_or(false);
        // E012 (plan.md 5): exhausted also needs the entry static for waiting, offering no input, and no entry kept
        let e12_exhausted = !sw.any()
            || (n_extra == 0 && (!sw.wait || waited.as_ref().map(|w| w.is_static).unwrap_or(false))
                && (!sw.afford || afford_offered == Some(0)));
        // (E013: a free try tries no target, so it never marks its entry exhausted)
        out["exhausted"] = json!(!free_try && played_none(&out) && !deferred_now && e12_exhausted);
        out["ok"] = json!(true);
        Ok(())
    })();
    if sw.any() || free_try {
        e12.insert("switches".into(), sw.json());
        out["e012"] = Value::Object(e12);
    }
    // E013 (FMI_FREE): the job's kind, and a free try's record
    if let Some(k) = job.get("kind") {
        out["kind"] = k.clone();
    }
    if let Some(f) = free_rec {
        out["free"] = f;
    }
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

/// E012: an engine at a wait's stop: a fork of the entry's engine run with no input to the stop's frame, held to the
/// stop's checkpoint (facts, tick, random state, actors), so the goal is requested again from that state.
fn engine_at(root: &mut Box<dyn Engine>, cp: &Checkpoint) -> Result<Box<dyn Engine>, String> {
    let mut b = machine::port(|| root.fork()).map_err(|e| format!("EngineError: {e}"))?;
    let st = machine::port(|| b.run(&RunSpec { frame: Some(cp.frame), ..Default::default() }));
    let differ = match st {
        Ok(st) if st.frame == cp.frame => {
            let obs = machine::port(|| b.obs(&ObsReq::default())).map_err(|e| format!("EngineError: {e}"));
            match obs {
                Ok(o) => {
                    let d = compare_checkpoint(&harness::checkpoint_json(cp), &o, b.status());
                    (!d.is_empty()).then(|| files::py_repr(&Value::Object(d)))
                }
                Err(e) => Some(e),
            }
        }
        Ok(st) => Some(format!("the run stopped at frame {} ({})", st.frame, st.reason)),
        Err(e) => Some(format!("EngineError: {e}")),
    };
    if let Some(d) = differ {
        machine::port(|| b.quit());
        return Err(format!("E012: the wait's stop at frame {} is not reached again from the entry: {d}", cp.frame));
    }
    Ok(b)
}

/// E012's affordance step in the try (mech::afford_step) with the wait's horizon and stops when the wait is on, and
/// job.json's dead moves of the entry left out.
#[allow(clippy::too_many_arguments)]
fn run_afford(r: &Resident, sw: &Switches, e: &mut Box<dyn Engine>, why: Trigger, t: &TestedT, nv: &TryNovel, rd: &dyn ReadingSource,
              job: &Value, prefix: &[Value], prefix_cps: &[Checkpoint], log: &JobLog) -> Result<mech::Afforded, String> {
    let dead_moves: Vec<Value> = job.get("dead").and_then(|d| d.as_array()).cloned().unwrap_or_default();
    let wait = if sw.wait { r.mech.wait.clone() } else { None };
    let then_f = wait.as_ref().map(|w| {
        move |b: &mut Box<dyn Engine>| -> Result<Vec<Checkpoint>, String> { Ok(w.wait(b, &t.flat, rd, 0)?.stops.into_iter().map(|s| s.checkpoint).collect()) }
    });
    let due_f = wait.as_ref().map(|w| move |o: &Obs| w.last_due(o));
    let then: Option<crate::afford::Then<'_>> = then_f.as_ref().map(|f| f as crate::afford::Then<'_>);
    let due: Option<&dyn Fn(&Obs) -> Option<u64>> = due_f.as_ref().map(|f| f as &dyn Fn(&Obs) -> Option<u64>);
    let a = machine::port(|| mech::afford_step(e, why, t, nv, due, then, &dead_moves, prefix, prefix_cps))
        .map_err(|e| format!("EngineError: afford: {e}"))?;
    log.say(&format!("afford ({}): {} trials, {} kept, {} dead, {:.2} s", why.text(), a.record["trials"], a.kept.len(), a.dead.len(),
                     a.record["step_seconds"].as_f64().unwrap_or(0.0)));
    Ok(a)
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
