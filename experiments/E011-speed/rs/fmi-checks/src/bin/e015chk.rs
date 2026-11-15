//! e015chk: the independent check of E015's item gate (results/item/check.md). It reuses the gate's own search
//! (`e015/item.rs`) and chains searches hop by hop: from a movie's last stop, a breadth-first search to a goal, the
//! goal's clicks appended to the movie, the next hop from there; at the end the whole movie is replayed from boot.
//! Every game number is an argument.
//!
//!   e015chk OUT --movie EVENTS --frame F --object O --class-obj C --class K --hops H1,H2,..
//!     F: the movie's last stop (an idle stop); each hop is KIND:ARG:DEPTH:TRIALS:FULL with KIND one of
//!       room   a stop in room ARG
//!       dialog a stop at an idle other than the sentence idle (ARG unused)
//!       ran    a run of the current room's local ARG anywhere in the trial
//!       vroom  a stop in room ARG, by a breadth-first search over the verb slots alone whose verb id is at least FULL
//!              (a dialog's choices; FULL is that lowest id here)
//!       clear  class:C:ARG absent at the stop where it was set at the hop's start
//!       state  state:ARG nonzero at the stop where it was 0 at the hop's start
//!       own    owner:O = the dump's ego at the stop (ARG unused)
//!       verb   verb:ARG on at the stop where it was off at the hop's start (a dialog's choices)
//!     FULL 1: afford's inputs in full with the third click (as a try runs them); 0: as the gate's steps 2 and 3
//!     Each stop records owner:O, class:C:K and the local's run.

#[path = "e015/item.rs"]
#[allow(dead_code)]
mod item;

use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::afford;
use fmi_explore::events::event_of;
use serde_json::{json, Value};
use std::path::PathBuf;

type Res<T> = Result<T, String>;

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn opt<'a>(argv: &'a [String], k: &str) -> Res<&'a str> {
    let i = argv.iter().position(|a| a == &format!("--{k}")).ok_or(format!("--{k}"))?;
    argv.get(i + 1).map(String::as_str).ok_or(format!("--{k} needs a value"))
}

fn num(argv: &[String], k: &str) -> Res<i64> {
    opt(argv, k)?.parse().map_err(|e| format!("--{k}: {e}"))
}

fn clicks(m: &[Event]) -> Value {
    json!(m.iter().map(|e| match e.what { Input::Click(x, y) => json!([e.frame, x, y]), _ => json!(null) }).collect::<Vec<_>>())
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&argv) {
        eprintln!("e015chk: {e}");
        std::process::exit(1);
    }
}

fn run(argv: &[String]) -> Res<()> {
    let out = PathBuf::from(argv.first().ok_or("OUT")?);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let h = Host::from_env().map_err(e2s)?;
    let text = std::fs::read_to_string(opt(argv, "movie")?).map_err(e2s)?;
    let mut movie: Vec<Event> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).map_err(e2s).and_then(|v| event_of(&v).ok_or(format!("event: {l}"))))
        .collect::<Res<_>>()?;
    let mut frame = num(argv, "frame")? as u64;
    let obj = num(argv, "object")? as u64;
    let ck = Key::class(num(argv, "class-obj")? as u64, num(argv, "class")? as u64);
    let ok = Key::owner(obj);
    let ego = Key::pack(K_EGO, 0, 0, 0);
    let mut res = Vec::new();
    let mut last: Option<item::Goal> = None;
    for (k, hop) in opt(argv, "hops")?.split(',').enumerate() {
        let p: Vec<&str> = hop.split(':').collect();
        if p.len() != 5 {
            return Err(format!("hop {hop}: KIND:ARG:DEPTH:TRIALS:FULL"));
        }
        let (kind, arg) = (p[0].to_string(), p[1].parse::<i32>().map_err(e2s)?);
        let cfg0 = BootCfg { seed: 1, events: movie.clone(), frame: Some(frame), dump_locals: false, lean: true, out_dir: out.join(format!("boot{k}")) };
        let mut e: Box<dyn Engine> = Box::new(h.boot_bin(&cfg0).map_err(e2s)?);
        if let Some(Some(b)) = afford::ready(&mut e, afford::W).map_err(e2s)? {
            e.quit();
            e = b;
        }
        let st0 = e.status().clone();
        let cfg = item::Cfg {
            depth: p[2].parse().map_err(e2s)?,
            trials: p[3].parse().map_err(e2s)?,
            horizon: afford::W,
            full: p[4] == "1",
            room: st0.room,
            finish_node: false,
        };
        let room0 = st0.room;
        let cobj = num(argv, "class-obj")? as u64;
        let class0 = e.facts().map_err(e2s)?.val(Key::class(cobj, arg.max(0) as u64));
        let state0 = e.facts().map_err(e2s)?.val(Key::state(arg.max(0) as u64));
        let verb0 = e.facts().map_err(e2s)?.val(Key::verb(arg.max(0) as u64));
        let mut det = |s: &item::Stop| {
            let ran = s.runs.iter().any(|r| r.where_ == WIO_LOCAL && r.number == arg && r.room == s.st.room);
            let goal = match kind.as_str() {
                "room" => s.st.room == arg,
                "dialog" => s.st.reason == "idle" && s.st.room == room0 && s.st.idle.as_deref().is_some_and(|i| i != "sentence"),
                "ran" => ran,
                "clear" => class0 != 0 && s.facts.val(Key::class(cobj, arg.max(0) as u64)) == 0,
                "state" => state0 == 0 && s.facts.val(Key::state(arg.max(0) as u64)) != 0,
                "own" => s.facts.val(ego) != 0 && s.facts.val(ok) == s.facts.val(ego),
                "verb" => verb0 == 0 && s.facts.val(Key::verb(arg.max(0) as u64)) != 0,
                _ => false,
            };
            item::Seen { goal, extra: json!({"owner": s.facts.val(ok), "ego": s.facts.val(ego), "class": s.facts.val(ck), "ran": ran}) }
        };
        let o = if kind == "vroom" { vbfs(&mut e, &cfg, arg, p[4].parse().map_err(e2s)?, &mut det)? } else { item::search(&mut e, &cfg, &mut det)? };
        e.quit();
        {
            use std::io::Write;
            let mut f = std::fs::File::create(out.join(format!("hop{k}.trials.jsonl"))).map_err(e2s)?;
            for t in &o.trials {
                writeln!(f, "{t}").map_err(e2s)?;
            }
        }
        let cnt = |key: &str, v: Value| o.trials.iter().filter(|t| t["seen"][key] == v).count();
        let row = json!({"hop": hop, "start": {"frame": st0.frame, "room": st0.room, "idle": st0.idle, "rng": st0.rng},
                         "trials": o.trials.len(), "stopped": o.stopped, "goals": o.goals, "seconds": o.seconds,
                         "stops_ran": cnt("ran", json!(true)), "stops_class_0": cnt("class", json!(0)),
                         "goal": o.goal.as_ref().map(|g| json!({"trial": g.trial, "names": g.names, "frame": g.frame, "room": g.room,
                                                                  "idle": g.idle, "owner": g.facts.val(ok), "class": g.facts.val(ck),
                                                                  "path": clicks(&g.path)}))});
        eprintln!("hop {k}: {row}");
        res.push(row);
        let Some(g) = o.goal else {
            break;
        };
        movie.extend(g.path.iter().cloned());
        frame = g.frame;
        last = Some(g);
    }
    let mut v = json!({"hops": res});
    if let Some(g) = &last {
        v["verify"] = item::verify(&h, &out.join("verify"), &movie, g.frame, &g.facts, g.room, g.digest)?;
        v["movie"] = clicks(&movie);
    }
    std::fs::write(out.join("summary.json"), serde_json::to_string_pretty(&v).map_err(e2s)?).map_err(e2s)?;
    println!("{v}");
    Ok(())
}

/// Breadth-first over the verb slots alone (each clicked at the frame after the stop, run to the next idle or W), to
/// the first stop in room `goal`; only idle stops in the start's room are expanded.
fn vbfs(start: &mut Box<dyn Engine>, cfg: &item::Cfg, goal: i32, vmin: i32, det: &mut dyn FnMut(&item::Stop) -> item::Seen) -> Res<item::Out> {
    let t0 = std::time::Instant::now();
    let mut out = item::Out { trials: vec![], nodes: vec![], goal: None, goals: 0, stopped: String::new(), seconds: 0.0 };
    let mut level: Vec<(Vec<Event>, Vec<String>, u64)> = vec![(vec![], vec![], start.status().frame)];
    'd: for depth in 0..cfg.depth {
        let mut next = Vec::new();
        for (path, names, fr) in level {
            let mut ne = start.fork().map_err(e2s)?;
            if !path.is_empty() {
                ne.inputs(&path).map_err(e2s)?;
                ne.run(&RunSpec { frame: Some(fr), ..Default::default() }).map_err(e2s)?;
            }
            ne.input_log().map_err(e2s)?;
            let o = ne.obs(&ObsReq::default()).map_err(e2s)?;
            let of = afford::offer(&mut ne, &o).map_err(e2s)?;
            let f0 = ne.status().frame;
            let end = f0 + cfg.horizon;
            for inp in afford::inputs_of(&of).into_iter().filter(|i| i.clicks.len() == 1 && matches!(i.clicks[0].hit, afford::Hit::Verb { id, .. } if id >= vmin)) {
                if out.trials.len() >= cfg.trials {
                    out.stopped = "trials".into();
                    ne.quit();
                    break 'd;
                }
                let mut c = ne.fork().map_err(e2s)?;
                c.segment(true).map_err(e2s)?;
                c.runs().map_err(e2s)?;
                let evs = inp.events(f0);
                c.inputs(&evs).map_err(e2s)?;
                let last = evs.last().map(|x| x.frame).unwrap_or(f0);
                let mut rec = json!({"i": out.trials.len(), "depth": depth + 1, "inputs": inp.name()});
                let st = match c.run(&RunSpec { frame: Some(last + 1), ..Default::default() }) {
                    Ok(_) => match c.run(&RunSpec { idle: true, max_frame: Some(end), ..Default::default() }) {
                        Ok(s) => Some(s),
                        Err(EngineError::Ended(_)) => None,
                        Err(x) => return Err(x.to_string()),
                    },
                    Err(EngineError::Ended(_)) => None,
                    Err(x) => return Err(x.to_string()),
                };
                let Some(st) = st else {
                    rec["ended"] = json!(true);
                    out.trials.push(rec);
                    c.quit();
                    continue;
                };
                let facts = c.facts().map_err(e2s)?;
                let digest = c.digest().map_err(e2s)?.digest;
                let runs = c.runs().map_err(e2s)?.runs;
                let log = c.input_log().map_err(e2s)?;
                let seen = det(&item::Stop { st: &st, facts: &facts, runs: &runs, log: &log });
                let mut p2 = path.clone();
                p2.extend(evs.iter().cloned());
                let mut n2 = names.clone();
                n2.push(inp.name());
                rec["room"] = json!(st.room);
                rec["frame"] = json!(st.frame);
                rec["reason"] = json!(st.reason);
                rec["seen"] = seen.extra;
                rec["path"] = clicks(&p2);
                c.quit();
                if st.room == goal {
                    rec["goal"] = json!(true);
                    out.goals += 1;
                    out.trials.push(rec);
                    out.goal = Some(item::Goal { trial: out.trials.len() - 1, path: p2, names: n2, frame: st.frame, facts, digest,
                                                 room: st.room, idle: st.idle.clone(), depth: depth + 1 });
                    out.stopped = "goal".into();
                    ne.quit();
                    break 'd;
                }
                out.trials.push(rec);
                if st.reason == "idle" && st.room == start.status().room {
                    next.push((p2, n2, st.frame));
                }
            }
            ne.quit();
        }
        level = next;
    }
    if out.stopped.is_empty() {
        out.stopped = "depth".into();
    }
    out.seconds = t0.elapsed().as_secs_f64();
    Ok(out)
}
