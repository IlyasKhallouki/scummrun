//! E024: `fmi shorten label --movie M --archive A [--seed S] [--no-twins] [--no-draws] [--check] [--tail N] OUT`
//! (fmi-shorten, experiments/E024-shorten/plan.md Part 2, items 1-3). The movie replayed from boot with the engine host
//! of this process's environment: OUT/steps.jsonl (the steps, each input's anchor, timing and identity, keep(k), the
//! twin's draw keys and the readings' draws that mark it) and OUT/labels.json (the summary); with --check, G2's
//! re-placement in OUT/g2.json. The archive folder is read only (its readings are linked into OUT/readings).
//!
//! `fmi shorten run --movie M --archive A --goal end --seed S [--skips] [--workers N] [--chunk C] [--tail N]
//! [--no-twins] [--no-ego] [--rounds 0] OUT` (items 4, 5, 8, 9; --rounds above 0 is refused: the reductions, item 7,
//! are stage B): the movie's steps (as `label`, with the twins' draw keys unless
//! --no-twins), then the forward pass, each step found again in order from boot (fmi-shorten pass.rs; --no-ego: a
//! step's goal without where ego stands), with --skips under text and cutscene skips from boot (skips.rs); a pass that
//! reaches the game's end is written as OUT/route-NNN.jsonl with its step log OUT/route-NNN.steps.jsonl (each step's
//! completion, tick and digest from the stage's replay of the route) and verified from boot (movie.rs); a pass that
//! stops is written as OUT/stopped-NNN.jsonl with its log. OUT/run-NNN.json (and stdout) gets the summary.

use fmi_explore::events::event_of;
use fmi_core::{EngineHost, Event};
use fmi_engine::{Host, HostCfg};
use fmi_shorten::chain::{self, Held, Opts};
use fmi_shorten::scan::{Cfg, Cost};
use fmi_shorten::verdict::End;
use fmi_shorten::{draws, label, movie, pass, skips};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

/// A subcommand's arguments: `with_value` its options with a value, `flags` its flags, at most `npos` positionals.
/// Any other option is an error, as is a flag given a value or an option none (the review of stage A: the plan's
/// `--rounds 3`, an unknown option, made OUT "3").
fn args(argv: &[String], with_value: &[&str], flags: &[&str], npos: usize) -> Result<super::Args, String> {
    let a = super::Args::parse(argv, with_value);
    for (k, v) in &a.opts {
        match (with_value.contains(&k.as_str()), flags.contains(&k.as_str()), v) {
            (true, _, Some(_)) | (_, true, None) => {}
            (true, _, None) => return Err(format!("--{k}: no value")),
            (_, true, Some(v)) => return Err(format!("--{k}={v}: a flag takes no value")),
            _ => return Err(format!("--{k}: no such option")),
        }
    }
    if a.pos.len() > npos {
        return Err(format!("{} arguments ({}), {npos} at most", a.pos.len(), a.pos.join(" ")));
    }
    Ok(a)
}

fn read_movie(p: &Path) -> Result<Vec<fmi_core::Event>, String> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).ok().and_then(|v| event_of(&v)).ok_or(format!("{}: not an event: {l}", p.display())))
        .collect()
}

fn label_in(a: &super::Args) -> Result<(), String> {
    let out = PathBuf::from(a.pos.first().ok_or("OUT")?);
    let movie = read_movie(Path::new(a.get("movie").ok_or("--movie")?))?;
    let mut held = Held::load(Path::new(a.get("archive").ok_or("--archive")?))?;
    let num = |k: &str, d: u64| a.get(k).map(|v| v.parse::<u64>().map_err(|e| format!("--{k}: {e}"))).unwrap_or(Ok(d));
    let o = Opts { seed: num("seed", 1)? as u32, out: out.join("replay"), twins: !a.flag("no-twins"), tail: num("tail", 3000)? };
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let host = fmi_engine::Host::from_env().map_err(|e| format!("the engine host: {e}"))?;
    let t0 = std::time::Instant::now();
    let ch = chain::replay(&host, &movie, &mut held, &o)?;
    let replay_s = t0.elapsed().as_secs_f64();
    let l = label::labels(&ch);
    let rds = if a.flag("no-draws") {
        Vec::new()
    } else {
        let game = fmi_reading::draws::DrawGame::load(&fmi_explore::guide::default_root()).ok();
        draws::load(Path::new(a.get("archive").unwrap_or_default()), &out.join("readings"), game.as_ref())?
    };
    let mut f = std::fs::File::create(out.join("steps.jsonl")).map_err(|e| e.to_string())?;
    let mut marked = Vec::new();
    for (s, mut v) in ch.steps.iter().zip(label::steps_json(&ch, &l)) {
        let m = draws::mark(s, &rds);
        if !m.is_empty() {
            marked.push(s.k);
        }
        v["reader_draws"] = m.into();
        writeln!(f, "{v}").map_err(|e| e.to_string())?;
    }
    let mut sum = label::summary(&ch, &l);
    sum["replay_s"] = replay_s.into();
    sum["reader_draw_steps"] = marked.into();
    sum["readings"] = draws::summary(&rds);
    if a.flag("check") {
        let t1 = std::time::Instant::now();
        let mut g2 = label::check(&host, &movie, &ch, &l, &Opts { out: out.join("check"), ..o.clone() })?;
        g2["seconds"] = t1.elapsed().as_secs_f64().into();
        std::fs::write(out.join("g2.json"), serde_json::to_string_pretty(&g2).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        for k in ["inputs", "placed", "same_hit", "same_identity", "moved", "steps_checked", "keep_at_completion", "keep_before_completion", "same_end"] {
            sum["g2"][k] = g2[k].clone();
        }
    }
    std::fs::write(out.join("labels.json"), serde_json::to_string_pretty(&sum).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("{sum}");
    Ok(())
}

pub fn cmd_label(argv: &[String]) -> i32 {
    match args(argv, &["movie", "archive", "seed", "tail"], &["no-twins", "no-draws", "check"], 1).and_then(|a| label_in(&a)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fmi shorten label: {e}");
            1
        }
    }
}

/// `base` with these bridge settings on top of its own (a setting it holds replaced).
fn with_env(base: &HostCfg, env: &[(String, String)]) -> HostCfg {
    let mut c = base.clone();
    c.extra_env.retain(|(k, _)| !env.iter().any(|e| e.0 == *k));
    c.extra_env.extend(env.iter().cloned());
    c
}

/// The standalone launch (movie.rs): the end record of the engine's trace.
fn standalone(cfg: &HostCfg, route: &[Event], seed: u32, max_ticks: u64, out: &Path) -> Result<Option<End>, String> {
    let _ = std::fs::remove_dir_all(out);
    let host = Host::new(cfg.clone()).map_err(|e| e.to_string())?;
    let boot = fmi_core::BootCfg { seed, events: route.to_vec(), frame: None, dump_locals: false, lean: true, out_dir: out.to_path_buf() };
    host.standalone(&boot, max_ticks, std::time::Duration::from_secs(1800)).map_err(|e| e.to_string())?;
    let t = std::fs::read_to_string(out.join("trace.jsonl")).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(t.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|v| v["type"] == "end").last().map(|v| End {
        tick: v["tick"].as_u64().unwrap_or(0),
        frame: v["frame"].as_u64().unwrap_or(0),
        reason: v["reason"].as_str().unwrap_or("").to_string(),
    }))
}

fn write_json(p: &Path, v: &Value) -> Result<(), String> {
    std::fs::write(p, serde_json::to_string_pretty(v).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", p.display()))
}

fn run_in(a: &super::Args) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    if a.get("rounds").is_some_and(|r| r != "0") {
        return Err("--rounds: the reductions (plan.md item 7) are stage B, not built: no round can run".into());
    }
    let out = PathBuf::from(a.pos.first().ok_or("OUT")?);
    let movie_path = a.get("movie").ok_or("--movie")?;
    let movie = read_movie(Path::new(movie_path))?;
    let goal = a.get("goal").unwrap_or("end");
    if goal != "end" {
        return Err(format!("--goal {goal}: only the game's end (end) is a goal here"));
    }
    let archive = a.get("archive").ok_or("--archive")?;
    let mut held = Held::load(Path::new(archive))?;
    let num = |k: &str, d: u64| a.get(k).map(|v| v.parse::<u64>().map_err(|e| format!("--{k}: {e}"))).unwrap_or(Ok(d));
    let seed = num("seed", 1)? as u32;
    let scan_cfg = Cfg { workers: num("workers", 8)? as usize, chunk: num("chunk", 16)? as u32 };
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let eng = out.join("engines");
    let base = HostCfg::from_env();
    let host0 = Host::new(base.clone()).map_err(|e| format!("the engine host: {e}"))?;
    // items 1-3: the movie's steps as it was run
    let o = Opts { seed, out: eng.join("replay"), twins: !a.flag("no-twins"), tail: num("tail", 3000)? };
    let ch = chain::replay(&host0, &movie, &mut held, &o)?;
    let l = label::labels(&ch);
    let root = |k: fmi_core::Key| held.root.get(&k.text()).copied().unwrap_or(0);
    let (specs, origs) = pass::specs(&ch, &l, !a.flag("no-ego"), &root)?;
    let label_s = t0.elapsed().as_secs_f64();
    eprintln!("labels: {} inputs, {} steps ({label_s:.1} s)", ch.met.len(), specs.len());
    // item 8: skips from boot
    let (host, boot, skips_report, env) = if a.flag("skips") {
        let make = |env: &[(String, String)]| -> Box<dyn EngineHost> { Box::new(Host::new(with_env(&base, env)).expect("host")) };
        let sk = skips::setup(&make, seed, &eng.join("skips"))?;
        eprintln!("skips: {}", sk.report);
        let h = Host::new(with_env(&base, &sk.env)).map_err(|e| e.to_string())?;
        (h, pass::boot_events(Some(sk.start)), sk.report, sk.env)
    } else {
        (host0.clone(), Vec::new(), Value::Null, Vec::new())
    };
    let hcfg = with_env(&base, &env);
    // the forward pass
    let pcfg = pass::PassCfg { seed, scan: scan_cfg, out: eng.join("pass"), w: fmi_explore::afford::W };
    let p = pass::forward(&host, &boot, &specs, &origs, &pcfg)?;
    eprintln!("pass: {:.1} s", p.wall_s);
    let mut cost = Cost::default();
    for s in &p.steps {
        cost.merge(&s.scan.cost);
        if let Some(r) = &s.relaxed {
            cost.merge(&r.cost);
        }
    }
    let found = |s: &pass::StepRun| s.scan.found.clone().or_else(|| s.relaxed.as_ref().and_then(|r| r.found.clone()));
    // the stage's digests at each completion, and its end, from its own replay of the route
    // (in frame order: a step may complete before the one before it)
    let end_frame = p.end.as_ref().map(|f| f.frame);
    let mut comps: Vec<u64> = p.steps.iter().filter_map(|s| found(s).map(|f| f.frame)).filter(|&f| Some(f) != end_frame).collect();
    comps.sort_unstable();
    comps.dedup();
    let last = comps.iter().copied().chain(end_frame).max().unwrap_or(0);
    // an error here leaves the pass's files written (no end: the stopped route)
    let (stage, stage_err) = match movie::replay(&host, &p.route, seed, &comps, last + 1000, &eng.join("stage")) {
        Ok(b) => (b, None),
        Err(x) => (movie::Boot::default(), Some(x)),
    };
    let rows: Vec<Value> = p
        .steps
        .iter()
        .zip(&origs)
        .zip(&specs)
        .map(|((s, o), sp)| {
            let f = found(s);
            let f = f.as_ref();
            json!({"k": s.k, "rooms": o.rooms,
                   "inputs": sp.inputs.iter().map(|i| json!({"anchor": i.anchor.json(), "timing": i.timing.name(), "gap": i.gap})).collect::<Vec<_>>(),
                   "skipped": o.skipped,
                   "orig": {"from": o.from, "frame": o.frame, "tick": o.tick, "len": o.len()},
                   "t": f.map(|f| f.t), "events": f.map(|f| f.events.iter().map(fmi_explore::events::event_json).collect::<Vec<_>>()),
                   "completion": f.map(|f| json!({"frame": f.frame, "tick": f.tick})),
                   "digest": f.and_then(|f| stage.digests.get(&f.frame)).map(|d| format!("{d:016x}")),
                   "scan": s.scan.json(), "relaxed": s.relaxed.as_ref().map(|r| r.json())})
        })
        .collect();
    let meta = json!({"movie": movie_path, "archive": archive, "goal": goal, "seed": seed, "skips": a.flag("skips"), "ego": !a.flag("no-ego"),
                      "env": env, "boot": boot.iter().map(fmi_explore::events::event_json).collect::<Vec<_>>(),
                      "workers": scan_cfg.workers, "chunk": scan_cfg.chunk, "w": pcfg.w});
    let end_json = |e: &End| json!({"frame": e.frame, "tick": e.tick, "reason": e.reason});
    let mut sum = json!({"movie": movie_path, "seed": seed, "skips": skips_report, "label": {"inputs": ch.met.len(), "steps": specs.len(),
                         "seconds": label_s, "end": ch.end.as_ref().map(|s| json!({"frame": s.frame, "tick": s.tick, "reason": s.reason}))},
                         "pass": {"wall_s": p.wall_s, "steps": p.steps.len(), "stopped": p.stopped, "events": p.route.len(),
                                  "end": p.end.as_ref().map(|f| json!({"frame": f.frame, "tick": f.tick}))},
                         "stage": {"end": stage.end.as_ref().map(end_json), "digests": stage.digests.len(), "error": stage_err},
                         "cost": cost.json()});
    match (&p.end, &stage.end) {
        (Some(f), Some(e)) if e.frame == f.frame && e.tick == f.tick => {
            let route = movie::write(&out, "route", &p.route, &meta, &rows, &end_json(e))?;
            eprintln!("route: {}", route.display());
            let v = movie::verify(&host, &|r: &[Event], s: u32, mt: u64, d: &Path| standalone(&hcfg, r, s, mt, d), &route, &eng.join("verify"))?;
            sum["route"] = json!(route.display().to_string());
            sum["verify"] = v;
        }
        _ => {
            let end = stage.end.as_ref().map(end_json).unwrap_or(Value::Null);
            let p2 = movie::write(&out, "stopped", &p.route, &meta, &rows, &end)?;
            sum["stopped_route"] = json!(p2.display().to_string());
        }
    }
    sum["wall_s"] = json!(t0.elapsed().as_secs_f64());
    for n in 1..1000 {
        let f = out.join(format!("run-{n:03}.json"));
        if !f.exists() {
            write_json(&f, &sum)?;
            break;
        }
    }
    println!("{sum}");
    Ok(())
}

pub fn cmd_run(argv: &[String]) -> i32 {
    let opts = ["movie", "archive", "goal", "seed", "workers", "chunk", "tail", "rounds"];
    match args(argv, &opts, &["skips", "no-twins", "no-ego"], 1).and_then(|a| run_in(&a)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fmi shorten run: {e}");
            1
        }
    }
}

/// `fmi shorten verify ROUTE [--ends] OUT` (item 9): ROUTE checked from its files as `run` checks a version it wrote
/// (the bridge's settings from its step log's first line); with --ends, a diagnostic only: the movie's end by the
/// bridge's launch and standalone, with no step log and no verdict.
pub fn cmd_verify(argv: &[String]) -> i32 {
    let r = (|| -> Result<(), String> {
        let a = args(argv, &[], &["ends"], 2)?;
        let route = PathBuf::from(a.pos.first().ok_or("ROUTE")?);
        let out = PathBuf::from(a.pos.get(1).ok_or("OUT")?);
        let meta: Value = std::fs::read_to_string(movie::steps_path(&route))
            .ok()
            .and_then(|t| t.lines().next().and_then(|l| serde_json::from_str::<Value>(l).ok()))
            .map(|v| v["meta"].clone())
            .unwrap_or(Value::Null);
        let env: Vec<(String, String)> = meta["env"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| Some((p[0].as_str()?.to_string(), p[1].as_str()?.to_string()))).collect())
            .unwrap_or_default();
        let hcfg = with_env(&HostCfg::from_env(), &env);
        let host = Host::new(hcfg.clone()).map_err(|e| e.to_string())?;
        let v = if a.flag("ends") {
            let ev = movie::read_route(&route)?;
            let b = movie::launch(&host, &ev, 1, &[], 1_000_000, &out.join("launch"))?;
            let s = standalone(&hcfg, &ev, 1, fmi_engine::host::MAX_TICKS, &out.join("standalone"))?;
            let e = |x: &Option<End>| x.as_ref().map(|e| json!({"frame": e.frame, "tick": e.tick, "reason": e.reason}));
            json!({"launch": e(&b.end), "standalone": e(&s), "same": b.end == s})
        } else {
            movie::verify(&host, &|r: &[Event], sd: u32, mt: u64, d: &Path| standalone(&hcfg, r, sd, mt, d), &route, &out)?
        };
        println!("{v}");
        Ok(())
    })();
    match r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fmi shorten verify: {e}");
            1
        }
    }
}

/// `fmi shorten trace --movie M [--seed S] [--from F] [--to T] [--env K=V,...] OUT`: a diagnostic, the movie from boot
/// and, frame by frame in [F, T], its stop: frame, tick, room, the camera's xstart, the idle kind, ego's position, var
/// 19 (one JSON line each on stdout).
pub fn cmd_trace(argv: &[String]) -> i32 {
    let r = (|| -> Result<(), String> {
        let a = args(argv, &["movie", "seed", "from", "to", "env"], &[], 1)?;
        let out = PathBuf::from(a.pos.first().ok_or("OUT")?);
        let movie = read_movie(Path::new(a.get("movie").ok_or("--movie")?))?;
        let num = |k: &str, d: u64| a.get(k).map(|v| v.parse::<u64>().map_err(|e| format!("--{k}: {e}"))).unwrap_or(Ok(d));
        let env: Vec<(String, String)> = a.get("env").unwrap_or("").split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let host = Host::new(with_env(&HostCfg::from_env(), &env)).map_err(|e| e.to_string())?;
        let (from, to) = (num("from", chain::BOOT_FRAME)?, num("to", 0)?);
        let (mut e, mut sched) = chain::boot(&host, &movie, num("seed", 1)? as u32, &out)?;
        while e.status().frame < to {
            let f = e.status().frame + 1;
            sched.upto(&mut e, f)?;
            let st = match e.run(&fmi_core::RunSpec { frame: Some(f), ..Default::default() }) {
                Ok(st) => st,
                Err(fmi_core::EngineError::Ended(st)) => {
                    println!("{}", json!({"frame": st.frame, "tick": st.tick, "end": st.reason}));
                    return Ok(());
                }
                Err(x) => return Err(x.to_string()),
            };
            if f >= from {
                println!("{}", json!({"frame": st.frame, "tick": st.tick, "room": st.room, "xstart": st.xstart, "cam": st.cam,
                                      "idle": st.idle, "ego": st.ego, "mv": st.mv, "v19": st.v19, "screen": st.screen}));
            }
        }
        e.quit();
        Ok(())
    })();
    match r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fmi shorten trace: {e}");
            1
        }
    }
}
