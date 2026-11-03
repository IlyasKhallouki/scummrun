//! E014 (experiments/E014-fate-demo): what the bridge sees from the boot of a game, frame by frame. A measurement
//! tool, not part of a run: the engine host from the environment (`HostCfg::from_env`, E014's FMI_GAME_ID and
//! FMI_GAME_DIR), seed 1, the lean switches.
//!
//!   probe boot OUT [--max-frame N]
//!       the boot to the first idle stop (replay_part1.boot), or to frame N: the stop's status as JSON
//!   probe trace OUT FRAMES [--every K] [--events FILE]
//!       the boot, then FRAMES frames from frame 0, one Step of K frames at a time (default 1); one JSON line per
//!       change of (room, userput, cutscene, idle kind, blocker, end) and per change of the visible verb slots (ids,
//!       modes, keys; no text), and a last line with the counts. --events: a movie (one event per line, as
//!       SPEEDRUN_INPUTS) played from boot.

use fmi_core::*;
use fmi_engine::{Host, HostCfg};
use serde_json::{json, Value};
use std::path::PathBuf;

fn boot_cfg(out: &PathBuf, events: Vec<Event>, frame: Option<u64>) -> BootCfg {
    BootCfg { seed: 1, events, frame, dump_locals: false, lean: true, out_dir: out.clone() }
}

fn status_json(s: &Status) -> Value {
    json!({"reason": s.reason, "ended": s.ended, "frame": s.frame, "tick": s.tick, "room": s.room, "up": s.up,
           "cs": s.cs, "idle": s.idle, "blk": s.blk, "ego": s.ego, "rng": s.rng, "screen": s.screen})
}

fn visible(o: &Obs) -> Vec<Value> {
    o.verbs
        .iter()
        .filter(|v| v.id != 0 && v.saveid == 0 && v.mode == 1)
        .map(|v| json!([v.slot, v.id, v.key, v.kind, v.x, v.y]))
        .collect()
}

fn read_events(p: &str) -> Vec<Event> {
    let text = std::fs::read_to_string(p).expect("--events");
    let mut out = Vec::new();
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(l).expect("an event line");
        let frame = v["frame"].as_u64().expect("frame");
        let what = if let Some(c) = v.get("click") {
            Input::Click(c[0].as_i64().unwrap() as i32, c[1].as_i64().unwrap() as i32)
        } else if let Some(k) = v.get("key") {
            Input::Key(k.as_str().unwrap().to_string())
        } else {
            panic!("an event this probe does not play: {l}")
        };
        out.push(Event { frame, what });
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let opt = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let host = Host::new(HostCfg::from_env()).expect("the host");
    match a.first().map(String::as_str) {
        Some("boot") => {
            let out = PathBuf::from(&a[1]);
            let max: u64 = opt("--max-frame").map(|v| v.parse().unwrap()).unwrap_or(fmi_engine::host::MAX_FRAME);
            let mut e = host.spawn(&boot_cfg(&out, Vec::new(), None)).expect("spawn");
            let t = std::time::Instant::now();
            let st = e.run(&RunSpec { idle: true, max_frame: Some(max), ..Default::default() });
            let s = match st {
                Ok(s) => status_json(&s),
                Err(err) => json!({"error": err.to_string()}),
            };
            println!("{}", json!({"boot": s, "seconds": t.elapsed().as_secs_f64()}));
        }
        Some("trace") => {
            let out = PathBuf::from(&a[1]);
            let frames: u64 = a[2].parse().expect("FRAMES");
            let every: u32 = opt("--every").map(|v| v.parse().unwrap()).unwrap_or(1);
            let events = opt("--events").map(|p| read_events(&p)).unwrap_or_default();
            let mut e = host.spawn(&boot_cfg(&out, events, None)).expect("spawn");
            let mut last: Option<Value> = None;
            let mut last_verbs: Option<Vec<Value>> = None;
            let (mut idle_frames, mut lines) = (0u64, 0u64);
            let start = e.status().frame;
            let t = std::time::Instant::now();
            while e.status().frame < start + frames {
                let r = e.step(every, false, false);
                let s = match r {
                    Ok(o) => o.status,
                    Err(err) => {
                        println!("{}", json!({"frame": e.status().frame, "error": err.to_string()}));
                        break;
                    }
                };
                if s.idle.is_some() {
                    idle_frames += every as u64;
                }
                let key = json!({"room": s.room, "up": s.up, "cs": s.cs, "idle": s.idle, "blk": s.blk, "ended": s.ended,
                                 "reason": if s.ended { Some(s.reason.clone()) } else { None }});
                let obs = e.obs(&ObsReq::default()).ok();
                let verbs = obs.as_ref().map(visible).unwrap_or_default();
                if last.as_ref() != Some(&key) || last_verbs.as_ref() != Some(&verbs) {
                    let mut line = key.clone();
                    line["frame"] = json!(s.frame);
                    line["tick"] = json!(s.tick);
                    if last_verbs.as_ref() != Some(&verbs) {
                        line["verbs"] = json!(verbs);
                    }
                    if let Some(o) = &obs {
                        line["vs"] = json!(o.vars.get(32).copied()); // VAR_VERB_SCRIPT (vars.cpp, v5)
                    }
                    println!("{line}");
                    lines += 1;
                    last = Some(key);
                    last_verbs = Some(verbs);
                }
                if s.ended {
                    break;
                }
            }
            println!("{}", json!({"done": e.status().frame, "idle_frames": idle_frames, "lines": lines,
                                  "seconds": t.elapsed().as_secs_f64()}));
        }
        _ => {
            eprintln!("usage: probe boot OUT [--max-frame N] | probe trace OUT FRAMES [--every K] [--events FILE]");
            std::process::exit(2);
        }
    }
}
