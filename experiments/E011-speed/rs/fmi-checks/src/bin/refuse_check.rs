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
//!   refuse-check b3 ARCHIVE OUT [SENTENCE_ENTRY DIALOG_ENTRY]
//!       B3, the verdicts on fixtures at recorded entries (default 203, a sentence idle, and 205, a dialog, of the E011
//!       kitchen archive): a verb click (taken, its slot), a key no visible slot has (handled in the key area), a
//!       dialog line (taken), a scene click during the dialog (reported), a key while the gate is shut after the line
//!       (dropped: gate). Writes OUT/b3.jsonl.
//!   refuse-check b1 RECORDED RERUN OUT
//!       B1: the window records of RERUN (a folder of jobs run again with FMI_REFUSE=1) against RECORDED's windows of the
//!       same job, attempt, plan, window, start and start Q: every recorded try in a skip (after a refused try, before
//!       the change that ended the skip or the window's end) must have been refused with the refused try's message;
//!       no window B ended may be one the recorded search ended on an end point; the bound windows one by one.
//!       Writes OUT/b1.jsonl, b1-bound.jsonl and b1-summary.json.
//!   refuse-check verify RERUN OUT
//!       B2 and rule (2): every new entry of RERUN's jobs verified from boot in a fresh lean engine (the job's entry
//!       movie, then the entry's segment; the job's entry checkpoint, then the entry's). Writes OUT/verify.jsonl.
//!   refuse-check b4 RERUN...
//!       B4's sample: every affordance step's record in RERUN's results (run with FMI_REFUSE_CHECK=1): digests, waits
//!       shared and run, the pairs whose shared wait was run again and compared, those that differ; the trials and
//!       their seconds by verdict. Prints the totals.
//!   refuse-check digest ARCHIVE OUT IDX...
//!       B4's first, second and fourth parts at each entry: a fork run 200 frames on and asked the digest at every
//!       frame, against a replay from boot to the same frame and against the parent run there with no digest asked
//!       (equal digests, equal exact dumps); consecutive frames of that idle run differ; the cost per digest.
//!
//! ARCHIVE and RECORDED are read only (their jobs' job.json and run/*/run.json).

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
        Some("b3") => b3(&argv[1..]),
        Some("b1") => b1(&argv[1..]),
        Some("verify") => verify(&argv[1..]),
        Some("b4") => b4(&argv[1..]),
        Some("digest") => digest(&argv[1..]),
        _ => Err("usage: refuse-check m1 ARCHIVE OUT [JOB] | b3 ARCHIVE OUT [S D] | b1 RECORDED RERUN OUT | verify RERUN OUT | b4 RERUN... | digest ARCHIVE OUT IDX...".into()),
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

// --- B3: the verdicts on fixtures at recorded entries -----------------------------------------------------------------

/// An engine at an archive entry, booted from its movie with the input log on.
fn at_entry(host: &Host, arch: &Path, id: u64, out: &Path) -> Res<Box<dyn Engine>> {
    let e = fmi_engine::archive::chain_of(arch, id)?;
    let cfg = BootCfg { seed: 1, events: e.events, frame: Some(e.frame), dump_locals: false, lean: true, out_dir: out.to_path_buf() };
    let mut b: Box<dyn Engine> = Box::new(host.boot_bin(&cfg).map_err(e2s)?);
    b.input_log().map_err(e2s)?;
    Ok(b)
}

/// One fixture: on a fork of `e`, the inputs at the given frames after its stop, run two frames past the last; the
/// log's records.
fn fixture(e: &mut Box<dyn Engine>, evs: &[(u64, Input)]) -> Res<(Vec<InputRecord>, Status)> {
    let mut c = e.fork().map_err(e2s)?;
    let f0 = c.status().frame;
    let events: Vec<Event> = evs.iter().map(|(d, w)| Event { frame: f0 + d, what: w.clone() }).collect();
    c.inputs(&events).map_err(e2s)?;
    let last = events.iter().map(|x| x.frame).max().unwrap_or(f0);
    c.run(&RunSpec { frame: Some(last + 2), ..Default::default() }).map_err(e2s)?;
    let l = c.input_log().map_err(e2s)?;
    let st = c.status().clone();
    c.quit();
    Ok((l.records, st))
}

/// A printable key no visible verb slot has (the engine's key test: `_mouseAndKeyboardStat == key`).
fn free_key(o: &Obs) -> String {
    let keys: Vec<i32> = o.verbs.iter().map(|v| v.key).collect();
    (b'a'..=b'z').map(|c| c as char).find(|c| !keys.contains(&(*c as i32))).map(|c| c.to_string()).unwrap_or_else(|| "z".into())
}

fn b3(a: &[String]) -> Res<()> {
    let arch = PathBuf::from(a.first().ok_or("b3 ARCHIVE OUT [SENTENCE_ENTRY DIALOG_ENTRY]")?);
    let out = PathBuf::from(a.get(1).ok_or("b3 ARCHIVE OUT")?);
    let sent: u64 = a.get(2).map(|x| x.parse().map_err(e2s)).transpose()?.unwrap_or(203);
    let dial: u64 = a.get(3).map(|x| x.parse().map_err(e2s)).transpose()?.unwrap_or(205);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    std::env::set_var("SPEEDRUN_INPUT", "1");
    let host = Host::from_env().map_err(e2s)?;
    let mut rows = Vec::new();
    let mut pass = true;
    let row = |name: &str, entry: u64, st0: &Status, recs: &[InputRecord], want: &str, ok: bool, rows: &mut Vec<Value>| {
        let line = json!({"fixture": name, "entry": entry, "frame": st0.frame, "idle": st0.idle, "up": st0.up,
                          "records": recs.iter().map(record_j).collect::<Vec<_>>(), "want": want, "ok": ok});
        eprintln!("{name} at entry {entry}: {} (want {want}) {}", recs.iter().map(|r| r.verdict.text()).collect::<Vec<_>>().join(", "),
                  if ok { "ok" } else { "MISSED" });
        rows.push(line);
    };
    // the sentence entry: a verb click; a key no visible slot has
    let mut e = at_entry(&host, &arch, sent, &out.join("boot-s"))?;
    let st = e.status().clone();
    let o = e.obs(&ObsReq::default()).map_err(e2s)?;
    let v = o.verbs.first().ok_or("the sentence entry shows no verb")?;
    let (recs, _) = fixture(&mut e, &[(1, Input::Click(v.x + 2, v.y + 2))])?;
    let ok = recs.len() == 1 && recs[0].slot == v.slot && recs[0].verdict == Verdict::Taken;
    pass &= ok;
    row("a verb click", sent, &st, &recs, "taken, the slot clicked", ok, &mut rows);
    let k = free_key(&o);
    let (recs, _) = fixture(&mut e, &[(1, Input::Key(k.clone()))])?;
    // handled in the key area: no slot matched, the handler called (whatever it then did)
    let ok = recs.len() == 1 && !recs[0].key_match && recs[0].slot == 0 && !matches!(recs[0].verdict, Verdict::Dropped(_));
    pass &= ok;
    row(&format!("a key no visible slot has ({k})"), sent, &st, &recs, "handled in the key area", ok, &mut rows);
    e.quit();
    // the dialog entry: a line; a scene click; a key while the gate is shut (after the line)
    let mut e = at_entry(&host, &arch, dial, &out.join("boot-d"))?;
    let st = e.status().clone();
    let o = e.obs(&ObsReq::default()).map_err(e2s)?;
    let v = o.verbs.first().ok_or("the dialog entry shows no verb")?;
    let (recs, _) = fixture(&mut e, &[(1, Input::Click(v.x + 2, v.y + 2))])?;
    let ok = recs.len() == 1 && recs[0].verdict == Verdict::Taken;
    pass &= ok;
    row("a dialog line", dial, &st, &recs, "taken", ok, &mut rows);
    let top = st.screen.map(|s| s.0).unwrap_or(0);
    let (recs, _) = fixture(&mut e, &[(1, Input::Click(160, top + 60))])?;
    row("a scene click during the dialog", dial, &st, &recs, "ignored or taken (reported)", true, &mut rows);
    // the gate: the line clicked, then the first frame with the gate shut, a key there
    let mut c = e.fork().map_err(e2s)?;
    let f0 = c.status().frame;
    c.inputs(&[Event { frame: f0 + 1, what: Input::Click(v.x + 2, v.y + 2) }]).map_err(e2s)?;
    let mut shut = None;
    for _ in 0..600 {
        let f = c.status().frame + 1;
        c.run(&RunSpec { frame: Some(f), ..Default::default() }).map_err(e2s)?;
        if c.status().up <= 0 {
            shut = Some(f);
            break;
        }
    }
    let Some(fs) = shut else { return Err("no frame with the gate shut within 600 frames of the line".into()) };
    c.input_log().map_err(e2s)?;
    let (recs, _) = fixture(&mut c, &[(1, Input::Key(k.clone()))])?;
    let ok = recs.len() == 1 && recs[0].verdict == Verdict::Dropped("gate".into());
    pass &= ok;
    let stc = c.status().clone();
    row(&format!("a key while the gate is shut (frame {fs}, after the line)"), dial, &stc, &recs, "dropped: gate", ok, &mut rows);
    c.quit();
    e.quit();
    let text: String = rows.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(out.join("b3.jsonl"), text).map_err(e2s)?;
    println!("B3: {} fixtures, {}", rows.len(), if pass { "pass" } else { "FAIL" });
    if pass {
        Ok(())
    } else {
        Err("B3 failed".into())
    }
}

// --- B1: the window records of a rerun with FMI_REFUSE=1 against the recorded ones -----------------------------------

fn windows_of(dir: &Path) -> Vec<(String, String, Value)> {
    let mut out = Vec::new();
    for jd in jobs_of(dir, None).unwrap_or_default() {
        let j = jd.file_name().unwrap().to_string_lossy().to_string();
        for ad in attempts_of(&jd) {
            let a = ad.file_name().unwrap().to_string_lossy().to_string();
            if let Ok(run) = read_json(&ad.join("run.json")) {
                for w in run["windows"].as_array().cloned().unwrap_or_default() {
                    out.push((j.clone(), a.clone(), w));
                }
            }
        }
    }
    out
}

fn wkey(j: &str, a: &str, w: &Value) -> String {
    format!("{j}/{a}/{}/{}/{}/{}", w["plan"], w["macro"], w["start"], w["start_qf"])
}

fn rows_of(w: &Value) -> Vec<(u64, String, String)> {
    w["children"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| Some((r.get(0)?.as_u64()?, r.get(3)?.as_str()?.to_string(), r.get(7).and_then(|x| x.as_str()).unwrap_or("").to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn b1(a: &[String]) -> Res<()> {
    let rec_dir = PathBuf::from(a.first().ok_or("b1 RECORDED RERUN OUT")?);
    let rerun = PathBuf::from(a.get(1).ok_or("b1 RECORDED RERUN OUT")?);
    let out = PathBuf::from(a.get(2).ok_or("b1 RECORDED RERUN OUT")?);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let recorded: BTreeMap<String, Value> = windows_of(&rec_dir).into_iter().map(|(j, at, w)| (wkey(&j, &at, &w), w)).collect();
    let mut lines = Vec::new();
    let (mut matched, mut same_choice, mut b_windows, mut b_ended) = (0usize, 0usize, 0usize, 0usize);
    let (mut skipped_tries, mut skipped_same, mut skipped_other, mut ended_on_end_point) = (0usize, 0usize, 0usize, 0usize);
    let mut bound: Vec<Value> = Vec::new();
    for (j, at, w) in windows_of(&rerun) {
        let Some(r) = recorded.get(&wkey(&j, &at, &w)) else { continue };
        matched += 1;
        let ref_best = r.get("best").map(|b| (b["g"].clone(), b["first"].clone(), b["outcome"].clone()));
        let new_best = w.get("best").map(|b| (b["g"].clone(), b["first"].clone(), b["outcome"].clone()));
        if ref_best == new_best && r["end_reason"] == w["end_reason"] {
            same_choice += 1;
        }
        let Some(rf) = w.get("refuse") else { continue };
        let tries = rf["tries"].as_array().cloned().unwrap_or_default();
        if tries.is_empty() {
            continue;
        }
        b_windows += 1;
        let refs = rows_of(r);
        let news = rows_of(&w);
        let end_frame = w["end"].as_u64().unwrap_or(0).max(rf["end"]["bound"].as_u64().unwrap_or(0));
        let mut wv = Vec::new();
        for t in &tries {
            let Some(cause) = t["cause"].as_str() else { continue };
            if cause == "dropped: gate" {
                continue;
            }
            let g = t["g"].as_u64().unwrap_or(0);
            let to = t["change"]["frame"].as_u64().unwrap_or(end_frame);
            // the refused try's message, as the rerun recorded it, else as the recorded window did at that frame
            let why = news.iter().find(|x| x.0 == g).or_else(|| refs.iter().find(|x| x.0 == g)).map(|x| x.2.clone()).unwrap_or_default();
            for (rg, oc, rw) in refs.iter().filter(|x| x.0 > g && x.0 < to) {
                skipped_tries += 1;
                if oc == "interface" && *rw == why {
                    skipped_same += 1;
                } else {
                    skipped_other += 1;
                    wv.push(json!({"g": rg, "outcome": oc, "why": rw, "refused_why": why}));
                }
            }
        }
        let ended = w["end_reason"].as_str().is_some_and(|s| s.starts_with("refused input"));
        if ended {
            b_ended += 1;
            if r.get("best").is_some_and(|b| !b.is_null()) {
                ended_on_end_point += 1;
            }
        }
        let first_g = tries.first().and_then(|t| t["g"].as_u64()).unwrap_or(0);
        if bound_interface(r) {
            bound.push(json!({"job": j, "att": at, "plan": w["plan"], "macro": w["macro"], "input": w["input"],
                              "causes": tries.iter().map(|t| t["cause"].clone()).collect::<Vec<_>>(),
                              "skipped_frames": rf["skipped_frames"], "end_reason": w["end_reason"], "end": rf["end"],
                              "first_try": first_g, "ended_within_10": ended && rf["end"]["frame"].as_u64().is_some_and(|f| f <= first_g + 10),
                              "recorded_end": r["end"], "rerun_end": w["end"], "tries_recorded": refs.len(), "tries_rerun": news.len()}));
        }
        lines.push(json!({"job": j, "att": at, "plan": w["plan"], "macro": w["macro"], "start": w["start"], "input": w["input"],
                          "end_reason": w["end_reason"], "recorded_end_reason": r["end_reason"], "refuse": rf,
                          "violations": wv}));
    }
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(out.join("b1.jsonl"), text).map_err(e2s)?;
    let text: String = bound.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(out.join("b1-bound.jsonl"), text).map_err(e2s)?;
    let s = json!({"windows_matched": matched, "same_choice_and_end": same_choice, "windows_with_refused_tries": b_windows,
                   "ended_by_b": b_ended, "ended_by_b_where_recorded_chose_an_end_point": ended_on_end_point,
                   "recorded_tries_in_skips": skipped_tries, "refused_with_the_same_message": skipped_same,
                   "other": skipped_other, "bound_windows": bound.len(),
                   "bound_ended_within_10_frames": bound.iter().filter(|b| b["ended_within_10"] == json!(true)).count()});
    std::fs::write(out.join("b1-summary.json"), serde_json::to_string_pretty(&s).map_err(e2s)? + "\n").map_err(e2s)?;
    println!("{}", serde_json::to_string_pretty(&s).map_err(e2s)?);
    if skipped_other == 0 && ended_on_end_point == 0 {
        Ok(())
    } else {
        Err("B1 failed".into())
    }
}

// --- B2 (and rule (2)): every new entry of a rerun's jobs verified from boot -----------------------------------------

fn verify(a: &[String]) -> Res<()> {
    let rerun = PathBuf::from(a.first().ok_or("verify RERUN OUT")?);
    let out = PathBuf::from(a.get(1).ok_or("verify RERUN OUT")?);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let host = Host::from_env().map_err(e2s)?;
    let (mut n, mut bad) = (0usize, 0usize);
    let mut lines = Vec::new();
    for jd in jobs_of(&rerun, None)? {
        let jname = jd.file_name().unwrap().to_string_lossy().to_string();
        let Ok(job) = read_json(&jd.join("job.json")) else { continue };
        let base: Vec<Event> = job["entry"]["events"].as_array().cloned().unwrap_or_default().iter().filter_map(fmi_explore::events::event_of).collect();
        let eid = job["entry"]["id"].as_u64();
        let cp0 = job["entry"]["checkpoint"].to_string();
        let mut news: Vec<PathBuf> = std::fs::read_dir(jd.join("new")).map(|r| r.filter_map(|d| d.ok().map(|d| d.path())).collect()).unwrap_or_default();
        news.sort();
        for p in news {
            let text = fmi_engine::archive::gunzip(&p)?;
            let j = fmi_engine::json::parse(&text)?;
            let parent = j.get("parent").and_then(|x| x.int().ok()).map(|x| x as u64);
            if parent != eid {
                return Err(format!("{}: parent {parent:?} is not the job's entry {eid:?}", p.display()));
            }
            let mut evs = base.clone();
            for e in j.at("events")?.arr()? {
                evs.push(fmi_engine::archive::event_of(e)?);
            }
            let mut cps = vec![fmi_engine::archive::checkpoint_of_json(&fmi_engine::json::parse(&cp0)?)?];
            for c in j.at("checkpoints")?.arr()? {
                cps.push(fmi_engine::archive::checkpoint_of_json(c)?);
            }
            let cfg = BootCfg { seed: 1, events: vec![], frame: None, dump_locals: true, lean: true, out_dir: out.join("boot") };
            let v = fmi_engine::replay::verify(&host, &cfg, &evs, &cps, &[]).map_err(e2s)?;
            n += 1;
            if !v.same {
                bad += 1;
            }
            lines.push(json!({"job": jname, "new": p.file_name().unwrap().to_string_lossy(), "checkpoints": v.checkpoints,
                              "differ": v.differ.len(), "same": v.same, "seconds": v.seconds}));
        }
    }
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(out.join("verify.jsonl"), text).map_err(e2s)?;
    println!("verify: {n} new entries from boot, {bad} with differences");
    if bad == 0 {
        Ok(())
    } else {
        Err("verify: differences".into())
    }
}

// --- B4's sample: the affordance steps' shared waits, run again and compared (FMI_REFUSE_CHECK=1) --------------------

fn walk_refuse<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(m) => {
            if let Some(r) = m.get("refuse") {
                if r.get("pairs").is_some() && r.get("digests").is_some() {
                    out.push(v);
                }
            }
            for x in m.values() {
                walk_refuse(x, out);
            }
        }
        Value::Array(a) => {
            for x in a {
                walk_refuse(x, out);
            }
        }
        _ => {}
    }
}

fn b4(a: &[String]) -> Res<()> {
    let rerun = PathBuf::from(a.first().ok_or("b4 RERUN...")?);
    let mut tot: BTreeMap<&str, f64> = BTreeMap::new();
    let mut differ = Vec::new();
    let mut verdicts: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    let mut steps = 0usize;
    for dir in a {
        let rerun = PathBuf::from(dir);
        for jd in jobs_of(&rerun, None)? {
            let Ok(r) = read_json(&jd.join("result.json")) else { continue };
            let mut found = Vec::new();
            walk_refuse(&r, &mut found);
            for s in found {
                steps += 1;
                let rf = &s["refuse"];
                for k in ["digests", "shared_trial", "shared_twin", "waits_run", "ended_at_once", "pairs", "pairs_same", "twin_forks", "digest_s"] {
                    *tot.entry(k).or_default() += rf[k].as_f64().unwrap_or(0.0);
                }
                for p in rf["pairs_differ"].as_array().cloned().unwrap_or_default() {
                    differ.push(json!({"job": jd.file_name().unwrap().to_string_lossy(), "frame": s["offer_frame"], "pair": p}));
                }
                if let (Some(n), Some(t)) = (s["verdicts"].as_object(), s["verdict_s"].as_object()) {
                    for (k, v) in n {
                        let e = verdicts.entry(k.clone()).or_default();
                        e.0 += v.as_f64().unwrap_or(0.0);
                        e.1 += t.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
                    }
                }
            }
        }
    }
    let _ = rerun;
    let s = json!({"steps": steps, "totals": tot, "pairs_differ": differ,
                   "verdicts": verdicts.iter().map(|(k, v)| (k.clone(), json!({"trials": v.0, "seconds": v.1}))).collect::<serde_json::Map<_, _>>(),
                   "ms_per_digest": tot.get("digest_s").copied().unwrap_or(0.0) * 1e3 / tot.get("digests").copied().unwrap_or(0.0).max(1.0)});
    println!("{}", serde_json::to_string_pretty(&s).map_err(e2s)?);
    if differ.is_empty() {
        Ok(())
    } else {
        Err("B4: pairs with equal digests whose waits differ".into())
    }
}

// --- B4's first, second and fourth parts: the digest against a replay from boot, along an idle run, its cost ---------

fn digest(a: &[String]) -> Res<()> {
    let arch = PathBuf::from(a.first().ok_or("digest ARCHIVE OUT IDX...")?);
    let out = PathBuf::from(a.get(1).ok_or("digest ARCHIVE OUT IDX...")?);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let ids: Vec<u64> = a[2..].iter().map(|x| x.parse().map_err(e2s)).collect::<Res<_>>()?;
    let host = Host::from_env().map_err(e2s)?;
    let mut lines = Vec::new();
    let mut pass = true;
    for id in ids {
        let ent = fmi_engine::archive::chain_of(&arch, id)?;
        let boot = |frame: u64, tag: &str| -> Res<Box<dyn Engine>> {
            let cfg = BootCfg { seed: 1, events: ent.events.clone(), frame: Some(frame), dump_locals: false, lean: true, out_dir: out.join(tag) };
            Ok(Box::new(host.boot_bin(&cfg).map_err(e2s)?))
        };
        // a fork run 200 frames on, asked at every frame, against a replay from boot to the same frame
        let mut e = boot(ent.frame, "a")?;
        let mut f = e.fork().map_err(e2s)?;
        let mut ds = Vec::new();
        let t = std::time::Instant::now();
        for k in 1..=200 {
            f.run(&RunSpec { frame: Some(ent.frame + k), ..Default::default() }).map_err(e2s)?;
            ds.push(f.digest().map_err(e2s)?);
        }
        let per = t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        for _ in 0..100 {
            f.digest().map_err(e2s)?;
        }
        let ms = t.elapsed().as_secs_f64() * 1e3 / 100.0;
        let mut r = boot(ent.frame + 200, "r")?;
        let (dr, df) = (r.digest().map_err(e2s)?, f.digest().map_err(e2s)?);
        let same_replay = dr.digest == df.digest;
        // the parent with no digest asked on the way: the same state (the digest reads only)
        e.run(&RunSpec { frame: Some(ent.frame + 200), ..Default::default() }).map_err(e2s)?;
        let same_quiet = e.digest().map_err(e2s)?.digest == df.digest
            && e.obs(&ObsReq { exact: true, ..Default::default() }).map_err(e2s)? == f.obs(&ObsReq { exact: true, ..Default::default() }).map_err(e2s)?;
        let equal_next = ds.windows(2).filter(|w| w[0].digest == w[1].digest).count();
        let distinct: std::collections::HashSet<u64> = ds.iter().map(|d| d.digest).collect();
        let ok = same_replay && same_quiet && equal_next == 0;
        pass &= ok;
        let l = json!({"entry": id, "frame": ent.frame, "same_as_replay": same_replay, "same_as_quiet_fork": same_quiet,
                       "idle_frames": ds.len(), "distinct": distinct.len(), "equal_consecutive": equal_next,
                       "ms_per_digest": ms, "s_per_frame_with_digest": per / 200.0, "bytes": dr.bytes, "ok": ok});
        eprintln!("{l}");
        lines.push(l);
        for x in [e, f, r] {
            x.quit();
        }
    }
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(out.join("digest.jsonl"), text).map_err(e2s)?;
    println!("digest: {} entries, {}", lines.len(), if pass { "pass" } else { "FAIL" });
    if pass {
        Ok(())
    } else {
        Err("digest failed".into())
    }
}
