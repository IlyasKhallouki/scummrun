//! Gates R1 and R3 of E012's delay fork on the engine (experiments/E012-game-mechanics/plan.md, amendment "component
//! delay"): test (a)'s recorded plans played in ready mode with the fork on every input window (the test setting
//! `Search.delay_force_p`, which no run sets), from the capture's start, for each p given.
//!
//! * R1: the attempt's inputs and checkpoints, and every candidate (its inputs and end checkpoint), verified from boot
//!   in a fresh engine (the caller's `verify`, fmi-engine's replay.verify): 0 differences.
//! * R3 on the engine: each fork's record chooses the least k whose try matched (success or the goal), or none and
//!   refuted when no try with k < K matched.
//! * Measured: per window the tries, their outcomes by k, the chosen k, the candidates, the random calls per parent
//!   frame, the wall; written to OUT/delay-<p>.jsonl.
//!
//! The recorded plans hold while the route is the capture's: once a fork changes it (a refuted window, a later k),
//! the next recorded plan starts elsewhere and the run stops there (no planner here).
//!
//! usage: delay-check FIXTURES OUT P... (P as 1/2, 1/21 or 0.25)

use crate::delay;
use crate::fixture::{self, Fx};
use crate::pyj::{self, J};
use crate::reading::PlayReading;
use crate::window::{Config, Search};
use fmi_core::*;
use std::path::Path;
use std::time::Instant;

/// Verify from boot: (inputs, checkpoints) -> (same, a description).
pub type Verify<'a> = &'a dyn Fn(&[Event], &[Checkpoint]) -> Result<(bool, String), String>;

fn parse_p(s: &str) -> Option<f64> {
    match s.split_once('/') {
        Some((a, b)) => Some(a.trim().parse::<f64>().ok()? / b.trim().parse::<f64>().ok()?),
        None => s.parse().ok(),
    }
}

/// One p: the plans played with the fork forced; returns (all gates hold, the summary line).
fn one(fx: &Fx, p: f64, tag: &str, out: &Path, verify: Verify) -> (bool, J) {
    let t_all = Instant::now();
    let reading = &fx.reading;
    let mut search = Search::new(Config::e010_ready(6)).unwrap();
    search.delay_on = true;
    search.delay_force_p = Some(p);
    search.progress = Some(Box::new(|m: &str| eprintln!("[delay] {m}")));
    let frame0 = fx.plans[0].i("frame").unwrap() as u64;
    let mut root = fixture::boot(fx, &J::Arr(vec![]), frame0, &format!("delay-root-{tag}"));
    let branch = root.fork().unwrap();
    let mut ps = PlayState {
        engine: branch,
        state: Obs::default(),
        obs: Facts::new(),
        now: Facts::new(),
        pred: Facts::new(),
        kept: Facts::new(),
        checkpoints: vec![],
        stop_reason: None,
        replan_cause: None,
        records: vec![],
        workdir: fx.dir.join("rust"),
    };
    let goal = fixture::conds_of(fx.plans[0].get("goal").unwrap());
    let un = ["args:", "saved:", "actorroom:"];
    let mut outs: Vec<String> = Vec::new();
    let mut left = String::new();
    for pj in &fx.plans {
        let n = pj.i("n_plan").unwrap() as u32;
        let res = fixture::plan_result(pj);
        let ctx = PlanCtx {
            reading,
            doms: reading.domains(),
            unobserved: &un,
            goal: &goal,
            result: &res,
            unobserved_actor_rooms: true,
            time_limit_s: 1e9,
        };
        let f0 = ps.engine.status().frame;
        if f0 != pj.i("frame").unwrap() as u64 {
            left = format!(
                "plan {n}: the engine is at {f0}, the capture's plan starts at {} (the route left the capture's)",
                pj.i("frame").unwrap()
            );
            break;
        }
        crate::window::set_pred(&mut ps, &fixture::view_of(pj.get("pred").unwrap()), &fixture::unobserved());
        {
            let data = search.reading_data(reading);
            let rd = PlayReading::new(reading, data);
            search.observe(&rd, &fixture::unobserved(), &mut ps).unwrap();
        }
        let o = search.play_plan(&ctx, &mut ps, n);
        let o = match o {
            PlayOutcome::Goal => "goal",
            PlayOutcome::Replan => "replan",
            PlayOutcome::Stop => "stop",
        };
        outs.push(o.to_string());
        if o != "replan" {
            break;
        }
    }
    let play_s = t_all.elapsed().as_secs_f64();
    let mut ok = true;
    // R3 on the engine: each fork's record against its own outcomes
    let mut rows: Vec<J> = Vec::new();
    let mut r3_all = true;
    for (wi, w) in search.windows.iter().enumerate() {
        let Some(d) = w.get("delay") else {
            println!("window {wi}: no delay record");
            ok = false;
            continue;
        };
        let rng = d.get("rng").cloned().unwrap_or(J::Null);
        if d.get("k_max").is_none() {
            println!(
                "window {wi}: {} (no input or not ready); rng {}",
                w.s("input").unwrap_or("events only"),
                pyj::dumps(&rng)
            );
            rows.push(crate::jobj! {"window" => wi, "input" => w.get("input").cloned().unwrap_or(J::Null), "rng" => rng});
            continue;
        }
        let kk = d.i("k_max");
        let outcomes = d.a("outcomes");
        let least = outcomes
            .iter()
            .filter(|o| matches!(o.arr()[3].str(), Some("success") | Some("goal")))
            .filter_map(|o| o.arr()[0].int())
            .min();
        let chosen = d.i("chosen");
        let refuted = d.get("refuted") == Some(&J::Bool(true));
        let r3 = chosen == least;
        let mut by: Vec<(String, i64)> = Vec::new();
        for o in outcomes {
            let s = o.arr()[3].str().unwrap_or("").to_string();
            match by.iter_mut().find(|x| x.0 == s) {
                Some(x) => x.1 += 1,
                None => by.push((s, 1)),
            }
        }
        println!(
            "window {wi}: {} at {}: K {:?}, first {:?}, chosen k {:?} (least matching {:?}), refuted {refuted}, {} tries {:?}, {} candidates; end: {}; {} children, wall {} s; R3 {}",
            w.s("input").unwrap_or(""),
            w.i("start").unwrap_or(0),
            kk,
            d.i("first"),
            chosen,
            least,
            outcomes.len(),
            by,
            d.a("candidates").len(),
            w.s("end_reason").unwrap_or(""),
            w.i("forks").unwrap_or(0),
            pyj::py_float(match w.get("wall_s") {
                Some(J::Float(f)) => *f,
                _ => 0.0,
            }),
            if r3 { "pass" } else { "FAIL" }
        );
        println!("   rng per parent frame: {}", pyj::dumps(&rng));
        ok &= r3;
        r3_all &= r3;
        rows.push(crate::jobj! {
            "window" => wi, "input" => w.get("input").cloned().unwrap_or(J::Null), "start" => w.get("start").cloned().unwrap_or(J::Null),
            "end" => w.get("end").cloned().unwrap_or(J::Null), "end_reason" => w.get("end_reason").cloned().unwrap_or(J::Null),
            "forks" => w.get("forks").cloned().unwrap_or(J::Null), "wall_s" => w.get("wall_s").cloned().unwrap_or(J::Null),
            "outcomes_all" => w.get("outcomes").cloned().unwrap_or(J::Null),
            "delay" => J::Obj(d.obj().iter().filter(|(k, _)| k != "candidates").cloned().collect()),
            "candidates" => d.a("candidates").len(), "r3" => r3,
        });
    }
    // R1: the attempt and every candidate, from boot
    let mut cps = ps.checkpoints.clone();
    cps.sort_by_key(|c| c.frame);
    cps.dedup_by_key(|c| c.frame);
    let t_v = Instant::now();
    let mut verified = 0usize;
    let mut differ = 0usize;
    let mut lines: Vec<String> = Vec::new();
    match verify(&search.movie, &cps) {
        Ok((same, txt)) => {
            verified += 1;
            differ += !same as usize;
            lines.push(format!(
                "attempt: {} inputs, {} checkpoints: {}",
                search.movie.len(),
                cps.len(),
                txt
            ));
        }
        Err(e) => {
            differ += 1;
            lines.push(format!("attempt: verify error {e}"));
        }
    }
    for c in &search.delay_candidates {
        match verify(&c.inputs, std::slice::from_ref(&c.checkpoint)) {
            Ok((same, txt)) => {
                verified += 1;
                differ += !same as usize;
                lines.push(format!(
                    "candidate plan {} window {} k {} ({}, frame {}, {} inputs): {}",
                    c.plan,
                    c.window,
                    c.k,
                    c.outcome,
                    c.checkpoint.frame,
                    c.inputs.len(),
                    txt
                ));
            }
            Err(e) => {
                differ += 1;
                lines.push(format!("candidate k {}: verify error {e}", c.k));
            }
        }
    }
    for l in &lines {
        println!("   R1 {l}");
    }
    let r1 = differ == 0;
    ok &= r1;
    let summary = crate::jobj! {
        "p" => J::Float(p), "k_max" => delay::k_max(p), "plans" => J::Arr(outs.iter().map(|s| J::Str(s.clone())).collect()),
        "left" => left.clone(), "frame_end" => ps.engine.status().frame, "inputs" => search.movie.len(),
        "checkpoints" => cps.len(), "candidates" => search.delay_candidates.len(),
        "verified" => verified, "differ" => differ, "r1" => r1, "ok" => ok,
        "play_s" => J::Float((play_s * 100.0).round() / 100.0),
        "verify_s" => J::Float((t_v.elapsed().as_secs_f64() * 100.0).round() / 100.0),
        "summary" => search.summary().get("delay").cloned().unwrap_or(J::Null),
        "windows" => J::Arr(rows),
    };
    let _ = std::fs::write(out.join(format!("delay-{tag}.jsonl")), pyj::dumps(&summary) + "\n");
    println!(
        "DELAY p = {p:.4} (K {:?}): plans {:?}{}; {} windows, {} candidates; R1 {} ({verified} verified from boot, {differ} differ); R3 {}; play {play_s:.1} s, verify {:.1} s",
        delay::k_max(p),
        outs,
        if left.is_empty() { String::new() } else { format!("; {left}") },
        search.windows.len(),
        search.delay_candidates.len(),
        if r1 { "pass" } else { "FAIL" },
        if r3_all { "pass" } else { "FAIL" },
        t_v.elapsed().as_secs_f64()
    );
    ps.engine.quit();
    if let Some(r) = search.root.take() {
        r.quit();
    }
    root.quit();
    (ok, summary)
}

pub fn run(a: &[String], host: Box<dyn EngineHost>, lean: bool, verify: Verify) -> bool {
    if a.len() < 4 {
        eprintln!("usage: delay-check FIXTURES OUT P...");
        return false;
    }
    let fx = fixture::load(Path::new(&a[1]), host, lean);
    let out = Path::new(&a[2]);
    let _ = std::fs::create_dir_all(out);
    let mut ok = true;
    for s in &a[3..] {
        let Some(p) = parse_p(s) else {
            eprintln!("not a p: {s}");
            return false;
        };
        let tag = s.replace('/', "-");
        let (o, _) = one(&fx, p, &tag, out, verify);
        ok &= o;
    }
    println!("DELAY-CHECK: {}", if ok { "PASS" } else { "FAIL" });
    ok
}
