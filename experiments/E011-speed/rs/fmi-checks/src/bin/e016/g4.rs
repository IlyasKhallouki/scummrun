//! `e016 g4` (plan.md, G4: A on Monkey Island): see main.rs.
//!
//! **The chain's steps.** The goal entry's chain is read from the archive (entries.jsonl, each entry's events, its
//! job's windows in `jobs/<job>/run/*/run.json`). A window whose chosen try's inputs (`best` or `chosen`: first to
//! issue) are among a chain entry's events is a planned step: `sentence(v,a[,b])`, `walk_to(a)` or `dialog_line(v)`,
//! starting at the try's fork frame g. A chain entry's events no window covers are the affordance trial that made it,
//! starting at the frame before its first click. Each step's record in the chain is read from the original chain
//! replayed from boot with patch 0007's input log: a planned sentence or walk_to step's Sentence touch on its object;
//! a trial's last Sentence touch, else its last taken verb click (area 1, a dialog line: its value is the verb). A
//! trial with neither is kept as clicks.
//!
//! **Re-realized.** A step realized from an engine (`step_from`): run to the step's start frame when the engine is
//! before it, then to the bridge's idle stop; realize the step (FMI_REALIZE=1) with the chain's own numbers (a walk
//! step's verb is the chain's Sentence touch's) and play the accepted events in place as the hook does (g3.rs). A miss
//! is reported with realize's reason, and the step's own clicks are played instead, shifted by the frames the engine
//! stands past the step's start (so the steps after it can still be tried); a kept trial is played the same way.
//! - `jobs`: each chain entry's steps from the entry its job started at (the archive's state, rebuilt from boot).
//! - `chain`: every step from the boot's entry in order; then the run on to the goal (up to R frames), and the movie
//!   replayed from boot and compared (every fact, the room, the digest).
//!
//! Pass: every step realized (the chain's Sentence touch, or the dialog verb taken) in both, no clicks played for a
//! miss, and the re-realized chain reaches the goal, verified from boot.

use crate::common::*;
use crate::g3::{realize_in_place, to_idle};
use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::events::{event_json, event_of};
use fmi_play::realize::{Cfg, Step};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// One step the chain took.
#[derive(Clone, Debug)]
struct ChainStep {
    /// the chain entry whose events hold it
    entry: u64,
    /// "window" (a planned step) or "afford" (the trial that made an entry)
    how: &'static str,
    /// the window's input record name ("" for a trial)
    name: String,
    /// the frame the step starts at: the chosen try's fork frame, or the frame before the trial's first click
    start: u64,
    /// its clicks and keys in the chain
    events: Vec<Event>,
    /// what the chain's input log shows for it, and the step realize takes (None: kept as clicks)
    seen: Value,
    step: Option<Step>,
}

fn read_jsonl(p: &Path) -> Res<Vec<Value>> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    text.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).map_err(e2s)).collect()
}

fn entry_events(arch: &Path, id: u64) -> Res<Vec<Event>> {
    let v = fmi_explore::files::read_gz(&arch.join("entries").join(format!("{id}.json.gz")))?;
    Ok(v["events"].as_array().cloned().unwrap_or_default().iter().filter_map(event_of).collect())
}

/// The windows of a job's attempts, in order: (input name, the chosen try's g, first, issue).
fn windows(arch: &Path, job: &str) -> Res<Vec<(String, u64, u64, u64)>> {
    let dir = arch.join("jobs").join(job).join("run");
    let mut atts: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd.filter_map(|x| x.ok().map(|x| x.path())).collect(),
        Err(_) => return Ok(vec![]),
    };
    atts.sort();
    let mut out = Vec::new();
    for a in atts {
        let p = a.join("run.json");
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let v: Value = serde_json::from_str(&text).map_err(e2s)?;
        for w in v["windows"].as_array().cloned().unwrap_or_default() {
            let b = if w["best"].is_object() { &w["best"] } else { &w["chosen"] };
            if let (Some(g), Some(f), Some(i)) = (b["g"].as_u64(), b["first"].as_u64(), b["issue"].as_u64()) {
                out.push((w["input"].as_str().unwrap_or("").to_string(), g, f, i));
            }
        }
    }
    Ok(out)
}

/// A window's input record name as (kind, numbers): "sentence(2,444)#60" -> ("sentence", [2, 444]).
fn name_parts(name: &str) -> (String, Vec<i32>) {
    let base = name.split('#').next().unwrap_or("");
    let (kind, rest) = base.split_once('(').unwrap_or((base, ""));
    let inner = rest.split(')').next().unwrap_or("");
    (kind.to_string(), inner.split(',').filter_map(|x| x.trim().parse().ok()).collect())
}

fn sentence_touches(recs: &[InputRecord]) -> Vec<(u64, i32, i32, i32)> {
    recs.iter()
        .flat_map(|r| r.writes.iter().filter(|t| t.kind == TouchKind::Sentence).map(move |t| (r.frame, t.a, t.b, t.old)))
        .collect()
}

/// The step realize takes for a chain step, from the chain's records of it.
fn step_of(cs: &ChainStep, recs: &[InputRecord]) -> (Value, Option<Step>) {
    let touches = sentence_touches(recs);
    let tj = |t: &(u64, i32, i32, i32)| json!([t.0, t.1, t.2, t.3]);
    if cs.how == "window" {
        let (kind, n) = name_parts(&cs.name);
        match kind.as_str() {
            "sentence" | "walk_to" => {
                let (a, b) = if kind == "walk_to" { (n.first().copied(), None) } else { (n.get(1).copied(), n.get(2).copied()) };
                let Some(a) = a else { return (json!({"error": "no object in the name"}), None) };
                let mine: Vec<&(u64, i32, i32, i32)> = touches.iter().filter(|t| t.2 == a && (kind == "walk_to" || t.3 == b.unwrap_or(0))).collect();
                let Some(t) = mine.last() else {
                    return (json!({"touches": touches.iter().map(tj).collect::<Vec<_>>(), "error": "no Sentence touch on the object"}), None);
                };
                let v = if kind == "walk_to" { t.1 } else { n[0] };
                (json!({"touch": tj(t), "touches": touches.iter().map(tj).collect::<Vec<_>>()}), Some(Step::Sentence { v, a, b }))
            }
            "dialog_line" => {
                let v = n.first().copied().unwrap_or(0);
                let taken = recs.iter().any(|r| r.verdict == Verdict::Taken && r.area == 1 && r.value == v);
                (json!({"dialog": v, "taken": taken}), Some(Step::Dialog { v }))
            }
            _ => (json!({"error": format!("a window of kind {kind}")}), None),
        }
    } else if let Some(t) = touches.last() {
        (json!({"touch": tj(t), "touches": touches.iter().map(tj).collect::<Vec<_>>()}),
         Some(Step::Sentence { v: t.1, a: t.2, b: (t.3 != 0).then_some(t.3) }))
    } else if let Some(r) = recs.iter().rev().find(|r| r.verdict == Verdict::Taken && r.area == 1) {
        (json!({"dialog": r.value, "slot": r.slot, "frame": r.frame}), Some(Step::Dialog { v: r.value }))
    } else {
        (json!({"records": recs.iter().map(record_json).collect::<Vec<_>>(), "kept": true}), None)
    }
}

/// The chain's steps and each one's records, from the original chain replayed from boot; and whether the original
/// reaches the goal at the goal entry's frame.
fn chain_steps(h: &Host, out: &Path, arch: &Path, chain: &[Value], goal: (Key, Val)) -> Res<(Vec<ChainStep>, Vec<Event>, Value)> {
    let mut steps: Vec<ChainStep> = Vec::new();
    let mut movie: Vec<Event> = Vec::new();
    for w in chain.windows(2) {
        let (p, e) = (&w[0], &w[1]);
        let id = e["id"].as_u64().ok_or("an entry without an id")?;
        let (pf, ef) = (p["frame"].as_u64().unwrap_or(0), e["frame"].as_u64().unwrap_or(0));
        let evs: Vec<Event> = entry_events(arch, id)?.into_iter().filter(|x| x.frame > pf && x.frame <= ef).collect();
        movie.extend(evs.iter().cloned());
        let mut covered = vec![false; evs.len()];
        for (name, g, first, issue) in windows(arch, e["job"].as_str().unwrap_or(""))? {
            let idx: Vec<usize> = (0..evs.len()).filter(|&i| !covered[i] && evs[i].frame >= first && evs[i].frame <= issue).collect();
            if idx.is_empty() || issue > ef {
                continue;
            }
            for &i in &idx {
                covered[i] = true;
            }
            steps.push(ChainStep { entry: id, how: "window", name, start: g, events: idx.iter().map(|&i| evs[i].clone()).collect(), seen: Value::Null, step: None });
        }
        let rest: Vec<Event> = (0..evs.len()).filter(|&i| !covered[i]).map(|i| evs[i].clone()).collect();
        if let Some(f) = rest.first().map(|x| x.frame) {
            steps.push(ChainStep { entry: id, how: "afford", name: String::new(), start: f - 1, events: rest, seen: Value::Null, step: None });
        }
    }
    steps.sort_by_key(|s| s.events.first().map(|x| x.frame).unwrap_or(0));
    // the original chain from boot with the input log: each step's records
    let f0 = chain[0]["frame"].as_u64().unwrap_or(0);
    let mut e = boot(h, &out.join("original"), &movie, f0)?;
    let later: Vec<Event> = movie.iter().filter(|x| x.frame > f0).cloned().collect();
    e.inputs(&later).map_err(e2s)?;
    e.input_log().map_err(e2s)?;
    for s in steps.iter_mut() {
        let (lo, hi) = (s.events.first().unwrap().frame, s.events.last().unwrap().frame);
        if e.status().frame < hi + 1 {
            e.run(&RunSpec { frame: Some(hi + 1), ..Default::default() }).map_err(e2s)?;
        }
        let recs: Vec<InputRecord> = e.input_log().map_err(e2s)?.records.into_iter().filter(|r| r.frame >= lo && r.frame <= hi).collect();
        let (seen, step) = step_of(s, &recs);
        s.seen = seen;
        s.step = step;
    }
    let gf = chain.last().unwrap()["frame"].as_u64().unwrap_or(0);
    if e.status().frame < gf {
        e.run(&RunSpec { frame: Some(gf), ..Default::default() }).map_err(e2s)?;
    }
    let got = e.facts().map_err(e2s)?.val(goal.0);
    let orig = json!({"frame": e.status().frame, "goal_value": got, "goal": got == goal.1, "inputs": movie.len()});
    e.quit();
    Ok((steps, movie, orig))
}

/// A step from `e`: to its start frame when `e` is before it, then to the idle stop; realized and played in place, or
/// on a miss (and for a kept trial) its own clicks played shifted. The row, and the events played.
fn step_from(e: &mut Box<dyn Engine>, s: &ChainStep, cfg: &Cfg) -> Res<(Value, Vec<Event>, bool)> {
    let mut row = json!({"entry": s.entry, "how": s.how, "name": s.name, "start": s.start, "chain_seen": s.seen,
                         "chain_events": s.events.iter().map(event_json).collect::<Vec<_>>()});
    if e.status().frame < s.start {
        match e.run(&RunSpec { frame: Some(s.start), ..Default::default() }) {
            Ok(_) => {}
            Err(EngineError::Ended(st)) => {
                row["ended"] = json!(st.reason.clone());
                return Ok((row, vec![], false));
            }
            Err(x) => return Err(e2s(x)),
        }
    }
    if e.status().idle.is_none() {
        match to_idle(e)? {
            Ok(st) if st.reason == "idle" => {}
            Ok(st) => row["no_idle"] = json!(st.frame),
            Err(why) => {
                row["ended"] = json!(why);
                return Ok((row, vec![], false));
            }
        }
    }
    row["from"] = json!(e.status().frame);
    if let Some(step) = &s.step {
        let (r, evs) = realize_in_place(e, step, cfg)?;
        let ok = r["ok"] == json!(true) && evs.is_some();
        row["realized"] = r;
        if let Some(evs) = evs {
            return Ok((row, evs, ok));
        }
        if row.get("ended").is_some() || row["realized"].get("ended").is_some() {
            return Ok((row, vec![], false));
        }
    }
    // the step's own clicks, shifted by the frames `e` stands past the step's start
    let shift = e.status().frame.saturating_sub(s.start);
    let evs: Vec<Event> = s.events.iter().map(|x| Event { frame: x.frame + shift, what: x.what.clone() }).collect();
    let last = evs.iter().map(|x| x.frame).max().unwrap_or(e.status().frame);
    e.inputs(&evs).map_err(e2s)?;
    row["played_chain_clicks"] = json!({"shift": shift});
    match e.run(&RunSpec { frame: Some(last), ..Default::default() }) {
        Ok(_) => {}
        Err(EngineError::Ended(st)) => row["ended"] = json!(st.reason.clone()),
        Err(x) => return Err(e2s(x)),
    }
    Ok((row, evs, false))
}

fn summary(rows: &[Value]) -> Value {
    let realized = rows.iter().filter(|r| r["realized"]["ok"] == json!(true)).count();
    let tried = rows.iter().filter(|r| r.get("realized").is_some()).count();
    let mut misses: BTreeMap<String, usize> = BTreeMap::new();
    for r in rows.iter().filter(|r| r["realized"]["ok"] == json!(false)) {
        *misses.entry(r["realized"]["reason"].as_str().unwrap_or("?").to_string()).or_default() += 1;
    }
    let cand: u64 = rows.iter().map(|r| r["realized"]["candidates"].as_u64().unwrap_or(0)).sum();
    let appr: u64 = rows.iter().map(|r| r["realized"]["approach"].as_u64().unwrap_or(0)).sum();
    let forks: u64 = rows.iter().map(|r| r["realized"]["tries"].as_u64().unwrap_or(0)).sum();
    let secs: f64 = rows.iter().map(|r| r["realized"]["seconds"].as_f64().unwrap_or(0.0)).sum();
    json!({"steps": rows.len(), "tried": tried, "realized": realized, "misses": misses,
           "kept": rows.iter().filter(|r| r.get("realized").is_none()).count(),
           "candidates": cand, "approach": appr, "forks": forks, "realize_s": secs})
}

pub fn run(argv: &[String]) -> Res<()> {
    let a = args(argv);
    if a.pos.len() < 3 {
        return Err("OUT ARCHIVE GOAL_ENTRY --goal KEY=V".into());
    }
    if !fmi_play::realize::on() {
        return Err(format!("{} is off", fmi_play::realize::SWITCH));
    }
    let out = PathBuf::from(&a.pos[0]);
    let arch = PathBuf::from(&a.pos[1]);
    let gid: u64 = a.pos[2].parse().map_err(e2s)?;
    let (gk, gv) = a.s("goal")?.split_once('=').ok_or("--goal KEY=V")?;
    let goal = (Key::parse(gk), gv.parse::<Val>().map_err(e2s)?);
    let run_on = a.num_or("run-on", 600)? as u64;
    let modes = a.opts.get("mode").cloned().unwrap_or_else(|| "jobs,chain".into());
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let t0 = Instant::now();
    let rows = read_jsonl(&arch.join("entries.jsonl"))?;
    let by_id: BTreeMap<u64, Value> = rows.into_iter().filter_map(|r| r["id"].as_u64().map(|i| (i, r))).collect();
    let mut chain = vec![by_id.get(&gid).ok_or("no goal entry")?.clone()];
    while let Some(p) = chain.last().unwrap()["parent"].as_u64() {
        chain.push(by_id.get(&p).ok_or("a missing parent")?.clone());
    }
    chain.reverse();
    let h = Host::from_env().map_err(e2s)?;
    let (steps, movie, orig) = chain_steps(&h, &out, &arch, &chain, goal)?;
    let cfg = Cfg::default();
    let mut v = json!({"chain": chain.iter().map(|c| c["id"].clone()).collect::<Vec<_>>(), "goal": a.s("goal")?,
                       "original": orig, "steps": steps.len()});
    let mut pass = true;
    if modes.contains("jobs") {
        // each chain entry's steps from the entry its job started at
        let mut rows = Vec::new();
        for w in chain.windows(2) {
            let (p, e) = (&w[0], &w[1]);
            let id = e["id"].as_u64().unwrap();
            let mine: Vec<&ChainStep> = steps.iter().filter(|s| s.entry == id).collect();
            if mine.is_empty() {
                continue;
            }
            let pf = p["frame"].as_u64().unwrap_or(0);
            let m: Vec<Event> = movie.iter().filter(|x| x.frame <= pf).cloned().collect();
            let mut eng = boot(&h, &out.join(format!("jobs-{id}")), &m, pf)?;
            for s in mine {
                let (row, _, _) = step_from(&mut eng, s, &cfg)?;
                rows.push(row);
            }
            eng.quit();
        }
        let sm = summary(&rows);
        pass &= sm["realized"] == sm["steps"];
        v["jobs"] = json!({"summary": sm, "rows": rows});
    }
    if modes.contains("chain") {
        let f0 = chain[0]["frame"].as_u64().unwrap_or(0);
        let m0: Vec<Event> = movie.iter().filter(|x| x.frame <= f0).cloned().collect();
        let mut eng = boot(&h, &out.join("chain"), &m0, f0)?;
        let mut played: Vec<Event> = m0.clone();
        let mut rows = Vec::new();
        let mut ended = false;
        for s in &steps {
            let (row, evs, _) = step_from(&mut eng, s, &cfg)?;
            ended = row.get("ended").is_some();
            played.extend(evs);
            rows.push(row);
            if ended {
                break;
            }
        }
        let sm = summary(&rows);
        let mut ch = json!({"summary": sm});
        if !ended {
            let mut first: Option<u64> = None;
            let (_, end) = run_until(&mut eng, run_on, &mut |e| {
                let ok = e.facts().map_err(e2s)?.val(goal.0) == goal.1;
                if ok && first.is_none() {
                    first = Some(e.status().frame);
                }
                Ok(ok)
            })?;
            ch["goal"] = json!({"reached": first, "ended": end, "stop": eng.status().frame, "room": eng.status().room});
            let ver = verify(&h, &out.join("chain-verify"), &played, &mut eng)?;
            pass &= first.is_some() && ver["differences"] == json!(0) && sm["realized"] == sm["steps"];
            ch["verify"] = ver;
        } else {
            pass = false;
        }
        write_events(&out.join("chain-events.jsonl"), &played)?;
        ch["rows"] = json!(rows);
        v["chain_run"] = ch;
        eng.quit();
    }
    v["pass"] = json!(pass);
    v["seconds"] = json!(t0.elapsed().as_secs_f64());
    std::fs::write(out.join("g4.json"), serde_json::to_string_pretty(&v).map_err(e2s)?).map_err(e2s)?;
    let brief = json!({"pass": pass, "original": v["original"], "steps": v["steps"], "jobs": v["jobs"]["summary"],
                       "chain": v["chain_run"]["summary"], "goal": v["chain_run"]["goal"],
                       "verify": v["chain_run"]["verify"]["differences"], "seconds": v["seconds"]});
    println!("{brief}");
    Ok(())
}
