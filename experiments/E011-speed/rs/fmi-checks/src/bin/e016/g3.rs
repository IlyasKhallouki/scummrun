//! `e016 g3` (plan.md, G3: A on an inventory object, the demo): see main.rs. Each step is realized by realize (its
//! candidates and forks recorded), and its accepted events are played in place on the chain's engine as the run
//! path's hook plays them (`realize::play`: scheduled, run to the last event's frame); the chain then runs to the next
//! idle stop. A second pass from the same start plays the steps through the hook itself (`executor::play_input` on a
//! branch with FMI_REALIZE=1), and its events and final digest are compared with the first pass's.

use crate::common::*;
use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::afford;
use fmi_explore::events::event_json;
use fmi_play::executor::{self, Br, Executor, PlayErr};
use fmi_play::realize::{self, Cfg, Step};
use fmi_play::replay::InputStep;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Instant;

/// The step as the window gives it to the hook: a sentence record's click step, a dialog line's line step.
pub fn input_of(s: &Step) -> InputStep {
    match *s {
        Step::Sentence { v, a, b } => InputStep { kind: "click", verb: Some(v as i64), a: Some(a as i64), b: b.map(|x| x as i64), key: None },
        Step::Dialog { v } => InputStep { kind: "line", verb: Some(v as i64), a: None, b: None, key: None },
    }
}

/// Runs `e` to the bridge's next idle stop within W frames: its status, or the end's reason.
pub fn to_idle(e: &mut Box<dyn Engine>) -> Res<Result<Status, String>> {
    let end = e.status().frame + afford::W;
    match e.run(&RunSpec { idle: true, max_frame: Some(end), ..Default::default() }) {
        Ok(st) => Ok(Ok(st)),
        Err(EngineError::Ended(st)) => Ok(Err(st.reason.clone())),
        Err(x) => Err(e2s(x)),
    }
}

/// One step realized from `e` (idle) and its events played in place on `e` (the hook's play): the step's row, and
/// the events (None on a miss).
pub fn realize_in_place(e: &mut Box<dyn Engine>, s: &Step, cfg: &Cfg) -> Res<(Value, Option<Vec<Event>>)> {
    let f0 = e.status().frame;
    match realize::realize(e, s, cfg).map_err(e2s)? {
        Ok(r) => {
            let row = json!({"step": s.text(), "ok": true, "from": f0, "approach": r.approach, "candidates": r.candidates,
                             "tries": r.tries, "seconds": r.seconds, "accepted_at": r.engine.status().frame,
                             "events": r.events.iter().map(event_json).collect::<Vec<_>>(),
                             "attempts": r.attempts.iter().map(attempt_json).collect::<Vec<_>>()});
            r.engine.quit();
            let last = r.events.iter().map(|x| x.frame).max().unwrap_or(f0);
            e.inputs(&r.events).map_err(e2s)?;
            match e.run(&RunSpec { frame: Some(last), ..Default::default() }) {
                Ok(_) => Ok((row, Some(r.events))),
                Err(EngineError::Ended(st)) => {
                    let mut row = row;
                    row["ended"] = json!(st.reason.clone());
                    Ok((row, None))
                }
                Err(x) => Err(e2s(x)),
            }
        }
        Err(m) => Ok((json!({"step": s.text(), "ok": false, "from": f0, "reason": m.reason, "approach": m.approach,
                             "candidates": m.candidates, "tries": m.tries, "seconds": m.seconds,
                             "attempts": m.attempts.iter().map(attempt_json).collect::<Vec<_>>()}), None)),
    }
}

pub fn run(argv: &[String]) -> Res<()> {
    let a = args(argv);
    let out = PathBuf::from(a.pos.first().ok_or("OUT")?);
    if !realize::on() {
        return Err(format!("{} is off", realize::SWITCH));
    }
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(a.s("movie")?).map_err(e2s)?).map_err(e2s)?;
    let n = a.num("clicks")? as usize;
    let movie: Vec<Event> = doc["movie"]
        .as_array()
        .ok_or("the file has no \"movie\"")?
        .iter()
        .take(n)
        .map(|c| Event { frame: c[0].as_u64().unwrap_or(0), what: Input::Click(c[1].as_i64().unwrap_or(0) as i32, c[2].as_i64().unwrap_or(0) as i32) })
        .collect();
    let frame = a.num("frame")? as u64;
    let steps: Vec<Step> = a.s("steps")?.split(',').map(|s| Step::parse(s).ok_or_else(|| format!("a step: {s}"))).collect::<Res<_>>()?;
    let local = a.num("local")? as i32;
    let ck = Key::class(a.num("class-obj")? as u64, a.num("class")? as u64);
    let obj = a.num_or("object", 0)? as u64;
    let run_on = a.num_or("run-on", 600)? as u64;
    let h = Host::from_env().map_err(e2s)?;
    let t0 = Instant::now();
    let mut e = boot(&h, &out.join("boot"), &movie, frame)?;
    if let Some(Some(b)) = afford::ready(&mut e, afford::W).map_err(e2s)? {
        e.quit();
        e = b;
    }
    let st0 = e.status().clone();
    let f0 = e.facts().map_err(e2s)?;
    let start = json!({"frame": st0.frame, "room": st0.room, "idle": st0.idle, "xstart": st0.xstart, "rng": st0.rng,
                       "owner": f0.val(Key::owner(obj)), "class": f0.val(ck)});
    // the second pass's start: a fork of the same stop
    let mut e2 = e.fork().map_err(e2s)?;
    let cfg = Cfg::default();
    e.segment(true).map_err(e2s)?;
    e.runs().map_err(e2s)?;
    let mut ran: Option<u64> = None;
    let note_runs = |e: &mut Box<dyn Engine>, ran: &mut Option<u64>| -> Res<()> {
        let room = e.status().room;
        let rs = e.runs().map_err(e2s)?.runs;
        if ran.is_none() && rs.iter().any(|r| r.where_ == WIO_LOCAL && r.number == local && r.room == room) {
            *ran = Some(e.status().frame);
        }
        Ok(())
    };
    let mut rows = Vec::new();
    let mut realized: Vec<Event> = Vec::new();
    let mut all_ok = true;
    for (i, s) in steps.iter().enumerate() {
        let (mut row, evs) = realize_in_place(&mut e, s, &cfg)?;
        note_runs(&mut e, &mut ran)?;
        let Some(evs) = evs else {
            rows.push(row);
            all_ok = false;
            break;
        };
        realized.extend(evs);
        if i + 1 < steps.len() {
            match to_idle(&mut e)? {
                Ok(st) => {
                    row["next_idle"] = json!({"frame": st.frame, "reason": st.reason, "room": st.room, "xstart": st.xstart});
                    note_runs(&mut e, &mut ran)?;
                    if st.reason != "idle" {
                        rows.push(row);
                        all_ok = false;
                        break;
                    }
                }
                Err(why) => {
                    row["next_idle"] = json!({"ended": why});
                    rows.push(row);
                    all_ok = false;
                    break;
                }
            }
        }
        rows.push(row);
    }
    let candidates: usize = rows.iter().map(|r| r["candidates"].as_u64().unwrap_or(0) as usize).sum();
    let mut v = json!({"start": start, "boot_s": t0.elapsed().as_secs_f64(), "steps": rows, "all_realized": all_ok,
                       "candidates": candidates});
    if all_ok {
        // the run on: frame by frame until the local has run and the class is cleared
        let last = realized.iter().map(|x| x.frame).max().unwrap_or(0);
        let mut cleared: Option<u64> = None;
        let (held, ended) = run_until(&mut e, run_on, &mut |e| {
            note_runs(e, &mut ran)?;
            if cleared.is_none() && e.facts().map_err(e2s)?.val(ck) == 0 {
                cleared = Some(e.status().frame);
            }
            Ok(ran.is_some() && cleared.is_some())
        })?;
        let fe = e.facts().map_err(e2s)?;
        v["run_on"] = json!({"last_input": last, "local_ran": ran, "class_cleared": cleared, "both": held, "ended": ended,
                             "stop": e.status().frame, "room": e.status().room, "owner": fe.val(Key::owner(obj)),
                             "class": fe.val(ck)});
        let pass = ran.is_some() && cleared.is_some() && candidates <= 15;
        let mut m = movie.clone();
        m.extend(realized.iter().cloned());
        let ver = verify(&h, &out.join("verify"), &m, &mut e)?;
        let verified = ver["differences"] == json!(0);
        v["verify"] = ver;
        // the second pass: the same steps through the run path's hook, on a branch of the start
        let mut hook_events: Vec<Event> = Vec::new();
        let mut hook_err = None;
        for (i, s) in steps.iter().enumerate() {
            let mut ex = Executor::new(false);
            let r = {
                let mut br = Br::new(&mut e2, None);
                executor::play_input(&mut ex, &mut br, &input_of(s), &s.text())
            };
            match r {
                Ok(()) => hook_events.extend(ex.movie.iter().cloned()),
                Err(PlayErr::Route(m)) => {
                    hook_err = Some(m);
                    break;
                }
                Err(PlayErr::Engine(x)) => return Err(e2s(x)),
            }
            if i + 1 < steps.len() {
                if let Err(why) = to_idle(&mut e2)? {
                    hook_err = Some(format!("ended: {why}"));
                    break;
                }
            }
        }
        let mut hook = json!({"events": hook_events.iter().map(event_json).collect::<Vec<_>>(), "error": hook_err,
                              "events_equal": hook_events == realized});
        if hook_err.is_none() {
            let target = e.status().frame;
            if e2.status().frame < target {
                e2.run(&RunSpec { frame: Some(target), ..Default::default() }).map_err(e2s)?;
            }
            let d1 = e.digest().map_err(e2s)?.digest;
            let d2 = e2.digest().map_err(e2s)?.digest;
            hook["frame"] = json!(e2.status().frame);
            hook["digest_equal"] = json!(d1 == d2 && e2.status().frame == target);
        }
        v["hook"] = hook;
        v["pass"] = json!(pass && verified);
    } else {
        v["pass"] = json!(false);
    }
    e.quit();
    e2.quit();
    write_events(&out.join("events.jsonl"), &realized)?;
    v["seconds"] = json!(t0.elapsed().as_secs_f64());
    std::fs::write(out.join("g3.json"), serde_json::to_string_pretty(&v).map_err(e2s)?).map_err(e2s)?;
    println!("{v}");
    Ok(())
}
