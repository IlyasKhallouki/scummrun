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
//!   probe state OUT F [--events FILE] [--locals]
//!       (E014 amendment (8)) the movie from boot, run to frame F: the stop's status, camera, verbs (slot, id, mode,
//!       key, x, y), room objects (id, state, parent, box, walk point), actors, slots and walk boxes, as one JSON line
//!   probe watch OUT F N [--events FILE] [--after FILE] [--units W:N,..] [--vars V,..] [--actors A,..] [--locals]
//!       (E014 amendment (8)) the movie from boot to frame F, the events of --after scheduled there (absolute frames),
//!       then N frames one at a time: per frame the slot runs, the explain log's begins, opcodes, reads, writes and
//!       ends of the units W:N (where, number), the vars and actors asked, the slots (with --locals their locals and
//!       cutscene overrides) and the cutscene levels; a line only when it differs from the last

use fmi_core::*;
use fmi_engine::{Host, HostCfg};
use serde_json::{json, Value};
use std::path::PathBuf;

fn boot_cfg(out: &PathBuf, events: Vec<Event>, frame: Option<u64>) -> BootCfg {
    // --locals (state, watch): the slots dumped with their locals, flags and cutscene overrides
    let locals = std::env::args().any(|x| x == "--locals");
    BootCfg { seed: 1, events, frame, dump_locals: locals, lean: true, out_dir: out.clone() }
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
        Some("state") => {
            // the movie played from boot to frame F, then the stop's verbs, room objects, actors, slots and walk boxes
            let out = PathBuf::from(&a[1]);
            let f: u64 = a[2].parse().expect("F");
            let events = opt("--events").map(|p| read_events(&p)).unwrap_or_default();
            let mut e = host.spawn(&boot_cfg(&out, events, Some(f))).expect("spawn");
            e.run(&RunSpec { frame: Some(f), ..Default::default() }).expect("the run to F");
            let s = e.status().clone();
            let o = e.obs(&ObsReq::default()).expect("obs");
            let boxes = e
                .boxes()
                .ok()
                .map(|b| b.boxes.iter().map(|w| json!([w.ul, w.ur, w.lr, w.ll, w.flags])).collect::<Vec<_>>());
            println!(
                "{}",
                json!({
                    "status": status_json(&s), "cam": s.cam, "xstart": s.xstart, "room_w": s.room_w, "walkbox": s.walkbox,
                    "verbs": o.verbs.iter().filter(|v| v.id != 0)
                        .map(|v| json!([v.slot, v.id, v.mode, v.key, v.x, v.y])).collect::<Vec<_>>(),
                    "objects": o.room_objects.as_ref().map(|r| r.iter()
                        .map(|x| json!([x.id, x.state, x.parent, x.x, x.y, x.w, x.h, x.walk])).collect::<Vec<_>>()),
                    "actors": o.actors.iter().map(|x| json!([x.id, x.x, x.y, x.elevation, x.moving])).collect::<Vec<_>>(),
                    "slots": o.slots.iter()
                        .map(|x| json!([x.slot, x.where_, x.number, x.status, x.code, x.frozen, x.delay])).collect::<Vec<_>>(),
                    "cutscene": o.cutscene.depth, "boxes": boxes, "ego": o.ego, "ego_pos": o.ego_pos,
                })
            );
        }
        Some("watch") => {
            // the movie played from boot to frame F, then N frames one at a time: per frame the slot runs (every unit,
            // code0 -> code1), the explain log's ops, reads and writes of the units --units W:N,.. (default none), the
            // vars --vars and the actors --actors; a line only when it differs from the last
            let out = PathBuf::from(&a[1]);
            let f: u64 = a[2].parse().expect("F");
            let n: u64 = a[3].parse().expect("N");
            let events = opt("--events").map(|p| read_events(&p)).unwrap_or_default();
            let list = |k: &str| -> Vec<i64> {
                opt(k)
                    .map(|v| v.split(',').filter(|x| !x.is_empty()).map(|x| x.parse().unwrap()).collect())
                    .unwrap_or_default()
            };
            let vars = list("--vars");
            let actors = list("--actors");
            let units: Vec<(i32, i32)> = opt("--units")
                .map(|v| {
                    v.split(',')
                        .filter(|x| !x.is_empty())
                        .map(|x| {
                            let (w, n) = x.split_once(':').expect("W:N");
                            (w.parse().unwrap(), n.parse().unwrap())
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut e = host.spawn(&boot_cfg(&out, events, Some(f))).expect("spawn");
            e.run(&RunSpec { frame: Some(f), ..Default::default() }).expect("the run to F");
            // --after FILE: events whose frames count from F (the "inputs" command's frames after now)
            if let Some(p) = opt("--after") {
                e.inputs(&read_events(&p)).expect("the inputs after F");
            }
            if !units.is_empty() {
                e.explain(Some(&ExplainArm { from: f, to: f + n + 1, units: units.clone() })).expect("explain");
            }
            let mut last_line = String::new();
            for _ in 0..n {
                let r = match e.step(1, true, !units.is_empty()) {
                    Ok(r) => r,
                    Err(err) => {
                        println!("{}", json!({"frame": e.status().frame, "error": err.to_string()}));
                        break;
                    }
                };
                let s = r.status;
                let o = e.obs(&ObsReq::default()).expect("obs");
                let runs: Vec<Value> = r
                    .runs
                    .map(|rr| {
                        rr.runs.iter().map(|x| json!([x.where_, x.number, x.code0, x.code1, x.status, x.after])).collect()
                    })
                    .unwrap_or_default();
                let mut ops: Vec<Value> = Vec::new();
                for en in r.explain.map(|x| x.entries).unwrap_or_default() {
                    match en {
                        Explain::Begin { where_, number, code0, frame, after, slot, .. } => {
                            ops.push(json!(["begin", where_, number, code0, frame, after, slot]))
                        }
                        Explain::Op { code, opcode, .. } => ops.push(json!(["op", code, opcode])),
                        Explain::Read { var, value, .. } => ops.push(json!(["r", var, value])),
                        Explain::Write { var, value, .. } => ops.push(json!(["w", var, value])),
                        Explain::End { code1, status, .. } => ops.push(json!(["end", code1, status])),
                    }
                }
                let vs: Vec<Value> = vars.iter().map(|&v| json!([v, o.vars.get(v as usize).copied()])).collect();
                let acs: Vec<Value> = o
                    .actors
                    .iter()
                    .filter(|x| actors.contains(&(x.id as i64)))
                    .map(|x| json!([x.id, x.x, x.y, x.moving]))
                    .collect();
                let sl: Vec<Value> = o
                    .slots
                    .iter()
                    .map(|x| json!([x.where_, x.number, x.code, x.status, x.cutscene_override, x.locals, x.resistant, x.recursive]))
                    .collect();
                let lv: Vec<Value> = o.cutscene.levels.iter().map(|l| json!([l.level, l.data])).collect();
                let body = json!({"runs": runs, "ops": ops, "vars": vs, "actors": acs, "cs": o.cutscene.depth, "slots": sl, "levels": lv,
                                  "up": s.up, "idle": s.idle, "walkbox": s.walkbox});
                let line = body.to_string();
                if line != last_line {
                    let mut b = body;
                    b["frame"] = json!(s.frame);
                    println!("{b}");
                    last_line = line;
                }
                if s.ended {
                    println!("{}", json!({"ended": s.reason}));
                    break;
                }
            }
        }
        _ => {
            eprintln!(
                "usage: probe boot OUT [--max-frame N] | probe trace OUT FRAMES [--every K] [--events FILE] | probe state \
                 OUT F [--events FILE] | probe watch OUT F N [--events FILE] [--units W:N,..] [--vars ..] [--actors ..]"
            );
            std::process::exit(2);
        }
    }
}
