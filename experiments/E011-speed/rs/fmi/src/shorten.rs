//! E024: `fmi shorten label --movie M --archive A [--seed S] [--no-twins] [--no-draws] [--check] [--tail N] OUT`
//! (fmi-shorten, experiments/E024-shorten/plan.md Part 2, items 1-3). The movie replayed from boot with the engine host
//! of this process's environment: OUT/steps.jsonl (the steps, each input's anchor, timing and identity, keep(k), the
//! twin's draw keys and the readings' draws that mark it) and OUT/labels.json (the summary); with --check, G2's
//! re-placement in OUT/g2.json. The archive folder is read only (its readings are linked into OUT/readings).
//!
//! `fmi shorten run --movie M --archive A --goal end --seed S [--skips] [--workers N] [--chunk C] [--tail N]
//! [--no-twins] [--no-ego] [--no-fallbacks] [--rounds R] [--wall SECONDS] OUT` (items 4-9): the movie's steps (as
//! `label`, with the twins' draw keys unless --no-twins), then the forward pass, each step found again in order from
//! boot (fmi-shorten pass.rs; --no-ego: a step's goal without where ego stands), a step the scan does not complete by
//! the fallbacks (refind.rs, unless --no-fallbacks), with --skips under text and cutscene skips from boot (skips.rs);
//! a pass that reaches the goal (the movie's end reason, with its site, where the stage's replay of the route ends
//! too) is written as OUT/unverified-NNN.jsonl with its step log (each step's completion, tick and digest from that
//! replay), verified from boot (movie.rs), and renamed OUT/route-NNN.jsonl only if that passes; a pass that stops is
//! written as OUT/stopped-NNN.jsonl with its log. With --rounds R (R > 0) the reductions (reduce.rs) from the verified
//! route, at most R rounds and --wall seconds (7200) from the start, each kept drop a new verified version.
//! OUT/run-NNN.json (and stdout) gets the summary. Exit 0 only with a verified route; 2 when the run went through
//! without one (stopped, or verify failed); 1 on an error.

use fmi_explore::events::event_of;
use fmi_core::{EngineHost, Event};
use fmi_engine::{Host, HostCfg};
use fmi_shorten::chain::{self, Held, Opts};
use fmi_shorten::refind::{self, RCfg};
use fmi_shorten::scan::{Cfg, Cost};
use fmi_shorten::verdict::{End, Goal};
use fmi_shorten::{draws, label, movie, pass, reduce, skips};
use std::collections::{BTreeMap, BTreeSet};
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

/// What a version of a pass needs of the run: the engines, its settings, its folders, the movie's steps.
struct Stage<'a> {
    host: &'a Host,
    hcfg: &'a HostCfg,
    out: &'a Path,
    eng: &'a Path,
    seed: u32,
    goal_end: &'a str,
    meta: Value,
    origs: &'a BTreeMap<usize, pass::Orig>,
}

impl Stage<'_> {
    /// A step of the pass as its step log has it.
    fn row(&self, s: &pass::StepRun, digests: &BTreeMap<u64, u64>) -> Value {
        let o = self.origs.get(&s.k);
        let f = s.found.as_ref();
        let ins = s.plan.as_ref().map(|p| p.inputs.iter().map(|i| json!({"anchor": i.anchor.json(), "timing": i.timing.name(), "gap": i.gap})).collect::<Vec<_>>());
        json!({"k": s.k, "rooms": o.map(|o| o.rooms.clone()), "how": s.how.map(|h| h.name()), "inputs": ins,
               "skips_off": s.plan.as_ref().map(|p| p.skips_off), "skipped": o.map(|o| o.skipped.clone()),
               "orig": o.map(|o| json!({"from": o.from, "frame": o.frame, "tick": o.tick, "len": o.len()})),
               "from": s.from, "t": f.map(|f| f.t), "events": f.map(|f| f.events.iter().map(fmi_explore::events::event_json).collect::<Vec<_>>()),
               "completion": f.map(|f| json!({"frame": f.frame, "tick": f.tick})),
               "digest": f.and_then(|f| digests.get(&f.frame)).map(|d| format!("{d:016x}")),
               "scan": s.scan.json(), "relaxed": s.relaxed.as_ref().map(|r| r.json()), "fallbacks": s.fallbacks})
    }

    /// A version of the pass `p` (item 9): the stage's digests at each completion and its end from its own replay of
    /// the route (in frame order: a step may complete before the one before it); a pass that reaches the goal (the
    /// movie's end reason, with its site, where the stage's replay ends too) written as OUT/unverified-NNN.jsonl with
    /// its step log, verified from boot and renamed OUT/route-NNN.jsonl only if that passes; else OUT/stopped-NNN.jsonl.
    /// The verified end (None: no verified route), the files' path, the record.
    fn version(&self, p: &pass::Pass, dropped: &BTreeSet<usize>) -> Result<(Option<End>, PathBuf, Value), String> {
        let end_frame = p.end.as_ref().map(|f| f.frame);
        let mut comps: Vec<u64> = p.steps.iter().filter_map(|s| s.found.as_ref().map(|f| f.frame)).filter(|&f| Some(f) != end_frame).collect();
        comps.sort_unstable();
        comps.dedup();
        let last = comps.iter().copied().chain(end_frame).max().unwrap_or(0);
        // an error here leaves the pass's files written (no end: the stopped route)
        let (stage, stage_err) = match movie::replay(self.host, &p.route, self.seed, &comps, last + 1000, &self.eng.join("stage")) {
            Ok(b) => (b, None),
            Err(x) => (movie::Boot::default(), Some(x)),
        };
        let rows: Vec<Value> = p.steps.iter().map(|s| self.row(s, &stage.digests)).collect();
        let mut meta = self.meta.clone();
        meta["dropped"] = json!(dropped);
        let end_json = |e: &End| json!({"frame": e.frame, "tick": e.tick, "reason": e.reason});
        let rec = json!({"stage": {"end": stage.end.as_ref().map(end_json), "digests": stage.digests.len(), "error": stage_err}});
        match (&p.end, &stage.end) {
            // the stage's own replay ends where the pass did, at the goal's site (the review of stage A)
            (Some(f), Some(e)) if e.frame == f.frame && e.tick == f.tick && e.reason == self.goal_end => {
                let draft = movie::write(self.out, "unverified", &p.route, &meta, &rows, &end_json(e))?;
                let alone = |r: &[Event], s: u32, mt: u64, d: &Path| standalone(self.hcfg, r, s, mt, d);
                let mut v = movie::verify(self.host, &alone, &draft, &self.eng.join("verify")).unwrap_or_else(|x| json!({"ok": false, "error": x}));
                let ok = v["ok"] == true;
                let path = if ok { movie::promote(&draft, "route")? } else { draft };
                eprintln!("{}: {} (end frame {} tick {})", if ok { "route" } else { "unverified" }, path.display(), e.frame, e.tick);
                v["stage"] = rec["stage"].clone();
                Ok((ok.then(|| e.clone()), path, v))
            }
            _ => {
                let end = stage.end.as_ref().map(end_json).unwrap_or(Value::Null);
                let p2 = movie::write(self.out, "stopped", &p.route, &meta, &rows, &end)?;
                Ok((None, p2, rec))
            }
        }
    }
}

/// The pass's candidates' costs (G6), every scan's merged.
fn cost_of(p: &pass::Pass) -> Cost {
    let mut cost = Cost::default();
    for s in &p.steps {
        cost.merge(&s.scan.cost);
        if let Some(r) = &s.relaxed {
            cost.merge(&r.cost);
        }
    }
    cost
}

/// The wall time the stage may take (plan.md's stop rule: 2 hours).
const WALL_S: u64 = 7200;

fn run_in(a: &super::Args) -> Result<bool, String> {
    let t0 = std::time::Instant::now();
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
    let rounds = num("rounds", 0)? as usize;
    let wall = num("wall", WALL_S)?;
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
    let ego = !a.flag("no-ego");
    let (specs, origs) = pass::specs(&ch, &l, ego, &root)?;
    // the goal: the game's end with the movie's reason (its site with it)
    let Some(Goal::End(goal_end)) = specs.last().map(|s| s.goal.clone()) else {
        return Err("the movie does not end the game: no goal".into());
    };
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
    // item 6's settings: the input handlers of the movie's sentences and of its dialog lines (label.rs)
    let dialog: BTreeSet<i32> = l.anchors.iter().zip(&l.ids).filter(|(an, _)| matches!(an, label::Anchor::Dialog { .. })).filter_map(|(_, id)| id.as_ref().map(|x| x.handler)).collect();
    let rc = RCfg { scan: scan_cfg, skips: a.flag("skips"), leaves: refind::LEAVES, sentence: label::sentence_handlers(&ch), dialog };
    // the forward pass
    let pcfg = pass::PassCfg { seed, scan: scan_cfg, out: eng.join("pass"), w: fmi_explore::afford::W, refind: (!a.flag("no-fallbacks")).then_some(rc) };
    let p = pass::forward(&host, &boot, &specs, &origs, &pcfg)?;
    eprintln!("pass: {:.1} s", p.wall_s);
    let by_k: BTreeMap<usize, pass::Orig> = specs.iter().zip(&origs).map(|(s, o)| (s.k, o.clone())).collect();
    let meta = json!({"movie": movie_path, "archive": archive, "goal": goal, "goal_end": goal_end, "seed": seed, "skips": a.flag("skips"), "ego": ego,
                      "env": env, "boot": boot.iter().map(fmi_explore::events::event_json).collect::<Vec<_>>(),
                      "workers": scan_cfg.workers, "chunk": scan_cfg.chunk, "w": pcfg.w});
    let st = Stage { host: &host, hcfg: &hcfg, out: &out, eng: &eng, seed, goal_end: &goal_end, meta, origs: &by_k };
    let (v, path, vrep) = st.version(&p, &BTreeSet::new())?;
    let hows: BTreeMap<&str, usize> = p.steps.iter().filter_map(|s| s.how).fold(BTreeMap::new(), |mut m, h| {
        *m.entry(h.name()).or_default() += 1;
        m
    });
    let mut sum = json!({"movie": movie_path, "seed": seed, "skips": skips_report, "label": {"inputs": ch.met.len(), "steps": specs.len(),
                         "seconds": label_s, "end": ch.end.as_ref().map(|s| json!({"frame": s.frame, "tick": s.tick, "reason": s.reason}))},
                         "pass": {"wall_s": p.wall_s, "steps": p.steps.len(), "how": hows, "stopped": p.stopped, "events": p.route.len(),
                                  "end": p.end.as_ref().map(|f| json!({"frame": f.frame, "tick": f.tick})),
                                  "version": path.display().to_string(), "verify": vrep},
                         "cost": cost_of(&p).json()});
    let mut verified = v.is_some();
    // item 7: the reductions, from the verified forward pass
    if let (Some(end), true) = (v.clone(), rounds > 0) {
        let make = |d: &BTreeSet<usize>| pass::specs_without(&ch, &l, ego, &root, d);
        let ver = |p: &pass::Pass, d: &BTreeSet<usize>| st.version(p, d);
        let cx = reduce::Ctx { host: &host, boot: &boot, specs: &make, cfg: &pcfg, out: &eng, version: &ver };
        let best = reduce::Best { pass: p, end, path: path.clone(), dropped: BTreeSet::new() };
        let deadline = t0 + std::time::Duration::from_secs(wall);
        let (best, recs) = reduce::rounds(best, &by_k, rounds, deadline, &cx)?;
        sum["rounds"] = json!(recs);
        sum["dropped"] = json!(best.dropped);
        sum["route"] = json!(best.path.display().to_string());
        sum["end"] = json!({"frame": best.end.frame, "tick": best.end.tick, "reason": best.end.reason});
    } else if let Some(end) = v {
        sum["route"] = json!(path.display().to_string());
        sum["end"] = json!({"frame": end.frame, "tick": end.tick, "reason": end.reason});
    } else {
        verified = false;
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
    Ok(verified)
}

pub fn cmd_run(argv: &[String]) -> i32 {
    let opts = ["movie", "archive", "goal", "seed", "workers", "chunk", "tail", "rounds", "wall"];
    match args(argv, &opts, &["skips", "no-twins", "no-ego", "no-fallbacks"], 1).and_then(|a| run_in(&a)) {
        Ok(true) => 0,
        Ok(false) => {
            eprintln!("fmi shorten run: no verified route");
            2
        }
        Err(e) => {
            eprintln!("fmi shorten run: {e}");
            1
        }
    }
}

/// `fmi shorten verify ROUTE [--ends] OUT` (item 9): ROUTE checked from its files as `run` checks a version it wrote
/// (the bridge's settings from its step log's first line); with --ends, a diagnostic only: the movie's end by the
/// bridge's launch and standalone, with no step log and no verdict. Exit 0 when the check passes (with --ends: both
/// launches end, at the same frame, tick and reason), 2 when it does not, 1 on an error.
pub fn cmd_verify(argv: &[String]) -> i32 {
    let r = (|| -> Result<bool, String> {
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
            json!({"launch": e(&b.end), "standalone": e(&s), "same": b.end.is_some() && b.end == s})
        } else {
            movie::verify(&host, &|r: &[Event], sd: u32, mt: u64, d: &Path| standalone(&hcfg, r, sd, mt, d), &route, &out)?
        };
        println!("{v}");
        Ok(v["ok"] == true || v["same"] == true)
    })();
    match r {
        Ok(true) => 0,
        Ok(false) => 2,
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

