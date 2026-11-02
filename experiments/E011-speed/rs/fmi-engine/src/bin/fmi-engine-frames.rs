//! fmi-engine-frames: one fixed frame workload on one engine, for `perf stat` on the engine (round 2's engine cuts,
//! experiments/E011-speed/engine/CUTS.md; plan.md B1's frame costs on fixed frames).
//!
//!   fmi-engine-frames OUT MODE N [--lean] [--env K=V]...
//!       boots the root (seed 1, no movie, its first idle stop: frame 2728), then plays N frames from there with no
//!       input, all in one engine:
//!         cmd50  in 50-frame Run commands (a frame inside a command);
//!         bare   one Run command per frame;
//!         step   one Step per frame with runs, explain entries and the F2 block, then facts() from the block (the
//!                window search's observed frame, as `fmi-engine bench` (a) runs it).
//!       N = 0 is the boot alone: `perf stat` of N frames less N = 0 is the frames' own count.
//!       Prints one JSON line: mode, n, the first and last frame, wall ms, engine CPU ms (its main thread), driver
//!       CPU ms.

use fmi_core::*;
use fmi_engine::{Host, HostCfg};
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut lean = false;
    let mut env = Vec::new();
    let mut pos = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--lean" => lean = true,
            "--env" => {
                i += 1;
                let (k, v) = args[i].split_once('=').expect("--env K=V");
                env.push((k.to_string(), v.to_string()));
            }
            a => pos.push(a.to_string()),
        }
        i += 1;
    }
    if pos.len() != 3 {
        eprintln!("usage: fmi-engine-frames OUT cmd50|bare|step N [--lean] [--env K=V]...");
        std::process::exit(2);
    }
    let out = PathBuf::from(&pos[0]);
    let mode = pos[1].clone();
    let n: u64 = pos[2].parse().expect("N");
    let mut cfg = HostCfg::from_env();
    cfg.extra_env.extend(env);
    let h = Host::new(cfg).expect("host");
    let boot = BootCfg {
        seed: 1,
        events: Vec::new(),
        frame: None,
        dump_locals: true,
        lean,
        out_dir: out.join("root"),
    };
    let mut e = h.spawn(&boot).expect("spawn");
    e.run(&RunSpec {
        idle: true,
        max_frame: Some(200_000),
        ..Default::default()
    })
    .expect("idle stop");
    let f0 = e.status().frame;
    if mode == "step" {
        e.segment(true).expect("segment");
        e.runs().expect("runs");
        e.explain(Some(&ExplainArm {
            from: f0 + 1,
            to: f0 + 1_000_000,
            units: vec![],
        }))
        .expect("explain");
        e.explain_log().expect("explain_log");
    }
    let pid = e.pid();
    let c0 = fmi_engine::sys::main_thread_cpu_ns(pid);
    let s0 = fmi_engine::sys::self_cpu_s();
    let t = Instant::now();
    match mode.as_str() {
        "cmd50" => {
            let mut left = n;
            while left > 0 {
                let k = left.min(50);
                let f = e.status().frame + k;
                e.run(&RunSpec {
                    frame: Some(f),
                    ..Default::default()
                })
                .expect("run");
                left -= k;
            }
        }
        "bare" => {
            for _ in 0..n {
                let f = e.status().frame + 1;
                e.run(&RunSpec {
                    frame: Some(f),
                    ..Default::default()
                })
                .expect("run");
            }
        }
        "step" => {
            for _ in 0..n {
                e.step(1, true, true).expect("step");
                e.facts().expect("facts");
            }
        }
        m => {
            eprintln!("unknown mode {m}");
            std::process::exit(2);
        }
    }
    let wall = t.elapsed().as_secs_f64() * 1e3;
    let cpu = (fmi_engine::sys::main_thread_cpu_ns(pid) - c0) as f64 / 1e6;
    let own = (fmi_engine::sys::self_cpu_s() - s0) * 1e3;
    let f1 = e.status().frame;
    Box::new(e).quit();
    println!(
        "{{\"mode\": \"{mode}\", \"n\": {n}, \"frame0\": {f0}, \"frame1\": {f1}, \"wall_ms\": {wall:.3}, \"engine_cpu_ms\": {cpu:.3}, \"driver_cpu_ms\": {own:.3}}}"
    );
}
