//! `e015 realize` (G1): see main.rs.

use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::afford;
use fmi_explore::events::{event_json, event_of};
use fmi_play::realize::{self, Attempt, Cfg, Step};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Instant;

type Res<T> = Result<T, String>;

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn opt<'a>(a: &'a [String], k: &str) -> Option<&'a str> {
    a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).map(|s| s.as_str())
}

/// An archive's entry file, read only.
fn entry(arch: &Path, i: u64) -> Res<Value> {
    fmi_explore::files::read_gz(&arch.join("entries").join(format!("{i}.json.gz")))
}

/// The entry's movie from boot (its chain's events up to its frame) and its frame.
fn movie(arch: &Path, idx: u64) -> Res<(Vec<Event>, u64)> {
    let e = entry(arch, idx)?;
    let frame = e["frame"].as_u64().ok_or("entry without a frame")?;
    let mut chain = vec![e];
    while let Some(p) = chain.last().unwrap().get("parent").and_then(|p| p.as_u64()) {
        chain.push(entry(arch, p)?);
    }
    chain.reverse();
    let evs: Vec<Event> = chain
        .iter()
        .flat_map(|c| c["events"].as_array().cloned().unwrap_or_default())
        .filter(|v| v["frame"].as_u64().is_some_and(|f| f <= frame))
        .filter_map(|v| event_of(&v))
        .collect();
    Ok((evs, frame))
}

fn attempt_json(a: &Attempt) -> Value {
    json!({"phase": a.phase, "name": a.name, "accepted": a.accepted, "seen": a.seen, "frame": a.frame,
           "events": a.events.iter().map(event_json).collect::<Vec<_>>(), "seconds": a.seconds})
}

fn boot(h: &Host, out: &Path, events: &[Event], frame: u64) -> Res<Box<dyn Engine>> {
    let cfg = BootCfg { seed: 1, events: events.to_vec(), frame: Some(frame), dump_locals: false, lean: true, out_dir: out.to_path_buf() };
    Ok(Box::new(h.boot_bin(&cfg).map_err(e2s)?))
}

pub fn run(a: &[String]) -> Res<()> {
    let pos: Vec<&String> = {
        let mut v = Vec::new();
        let mut i = 0;
        while i < a.len() {
            if a[i].starts_with("--") {
                if ["--steps", "--room", "--run-on"].contains(&a[i].as_str()) {
                    i += 1;
                }
            } else {
                v.push(&a[i]);
            }
            i += 1;
        }
        v
    };
    if pos.len() < 3 {
        return Err("OUT ARCHIVE IDX".into());
    }
    let out = PathBuf::from(pos[0]);
    let arch = PathBuf::from(pos[1]);
    let idx: u64 = pos[2].parse().map_err(e2s)?;
    let steps: Vec<Step> = opt(a, "--steps")
        .ok_or("--steps S1,S2,..")?
        .split(',')
        .map(|s| Step::parse(s).ok_or_else(|| format!("a step: {s}")))
        .collect::<Res<_>>()?;
    let room: Option<i32> = opt(a, "--room").map(|r| r.parse().map_err(e2s)).transpose()?;
    let run_on: u64 = opt(a, "--run-on").map(|r| r.parse().map_err(e2s)).transpose()?.unwrap_or(600);
    let verify = a.iter().any(|x| x == "--verify");
    if !realize::on() {
        return Err(format!("{} is off", realize::SWITCH));
    }
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let h = Host::from_env().map_err(e2s)?;
    let (evs, frame) = movie(&arch, idx)?;
    let t0 = Instant::now();
    let mut e = boot(&h, &out.join("boot"), &evs, frame)?;
    let boot_s = t0.elapsed().as_secs_f64();
    // the interface is read where the bridge is idle (a no-input fork to the first idle stop when the entry is not)
    let mut cur: Box<dyn Engine> = match afford::ready(&mut e, afford::W).map_err(e2s)? {
        None => e,
        Some(Some(b)) => {
            e.quit();
            b
        }
        Some(None) => return Err(format!("entry {idx}: no idle stop within {} frames", afford::W)),
    };
    let start = json!({"frame": cur.status().frame, "idle": cur.status().idle, "room": cur.status().room,
                       "xstart": cur.status().xstart});
    let cfg = Cfg::default();
    let mut realized: Vec<Event> = Vec::new();
    let mut rows = Vec::new();
    let mut all_ok = true;
    for (i, s) in steps.iter().enumerate() {
        let f_before = cur.status().frame;
        let r = realize::realize(&mut cur, s, &cfg).map_err(e2s)?;
        match r {
            Ok(ok) => {
                let mut row = json!({"step": s.text(), "ok": true, "from": f_before, "approach": ok.approach,
                                     "candidates": ok.candidates, "tries": ok.tries, "seconds": ok.seconds,
                                     "events": ok.events.iter().map(event_json).collect::<Vec<_>>(),
                                     "accepted_at": ok.engine.status().frame,
                                     "attempts": ok.attempts.iter().map(attempt_json).collect::<Vec<_>>()});
                realized.extend(ok.events.iter().cloned());
                let old = std::mem::replace(&mut cur, ok.engine);
                old.quit();
                if i + 1 < steps.len() {
                    // the next step starts where the bridge is idle again
                    let end = cur.status().frame + afford::W;
                    let st = cur.run(&RunSpec { idle: true, max_frame: Some(end), ..Default::default() }).map_err(e2s)?;
                    row["next_idle"] = json!({"frame": st.frame, "reason": st.reason, "idle": st.idle, "room": st.room,
                                              "xstart": st.xstart});
                    if st.reason != "idle" {
                        rows.push(row);
                        all_ok = false;
                        break;
                    }
                }
                rows.push(row);
            }
            Err(m) => {
                rows.push(json!({"step": s.text(), "ok": false, "from": f_before, "reason": m.reason, "approach": m.approach,
                                 "candidates": m.candidates, "tries": m.tries, "seconds": m.seconds,
                                 "attempts": m.attempts.iter().map(attempt_json).collect::<Vec<_>>()}));
                all_ok = false;
                break;
            }
        }
    }
    let mut summary = json!({"entry": idx, "entry_frame": frame, "boot_s": boot_s, "start": start,
                             "steps": rows, "all_realized": all_ok});
    if let (true, Some(r)) = (all_ok, room) {
        // run on, one frame at a time, until the room is r or run_on frames pass
        let f0 = cur.status().frame;
        let mut reached = None;
        let mut ended = None;
        while cur.status().frame < f0 + run_on {
            let to = cur.status().frame + 1;
            match cur.run(&RunSpec { frame: Some(to), ..Default::default() }) {
                Ok(st) => {
                    if st.room == r {
                        reached = Some(st.frame);
                        break;
                    }
                }
                Err(EngineError::Ended(st)) => {
                    ended = Some(st.reason.clone());
                    break;
                }
                Err(x) => return Err(e2s(x)),
            }
        }
        summary["run_on"] = json!({"from": f0, "room": r, "reached": reached, "ended": ended,
                                   "stop": cur.status().frame, "room_at_stop": cur.status().room});
    }
    let last = cur.status().frame;
    summary["last_frame"] = json!(last);
    if verify && all_ok {
        let t = Instant::now();
        let mut m = evs.clone();
        m.extend(realized.iter().cloned());
        let want_facts = facts_of(&cur.obs(&ObsReq::default()).map_err(e2s)?);
        let want_room = cur.status().room;
        let want_digest = cur.digest().map_err(e2s)?;
        let mut fresh = boot(&h, &out.join("verify"), &m, last)?;
        let got_facts = facts_of(&fresh.obs(&ObsReq::default()).map_err(e2s)?);
        let got_room = fresh.status().room;
        let got_digest = fresh.digest().map_err(e2s)?;
        let d = want_facts.diff(&got_facts);
        let first: Vec<Value> = d.iter().take(8).map(|(k, x, y)| json!([k.text(), x, y])).collect();
        let differences = d.len() + (want_room != got_room) as usize + (want_digest.digest != got_digest.digest) as usize;
        summary["verify"] = json!({"frame": last, "inputs": m.len(), "facts": want_facts.0.len(), "facts_differ": d.len(),
                                   "first": first, "room": [want_room, got_room], "digest": [want_digest.digest, got_digest.digest],
                                   "tick": [cur.status().tick, fresh.status().tick], "rng": [cur.status().rng, fresh.status().rng],
                                   "differences": differences, "seconds": t.elapsed().as_secs_f64()});
        fresh.quit();
    }
    cur.quit();
    let mut lines = String::new();
    for ev in &realized {
        lines.push_str(&event_json(ev).to_string());
        lines.push('\n');
    }
    std::fs::write(out.join("events.jsonl"), lines).map_err(e2s)?;
    summary["seconds"] = json!(t0.elapsed().as_secs_f64());
    std::fs::write(out.join("realize.json"), serde_json::to_string_pretty(&summary).map_err(e2s)?).map_err(e2s)?;
    println!("{summary}");
    Ok(())
}
