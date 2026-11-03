//! delay-check: E012's delay fork on the engine (gates R1 and R3, experiments/E012-game-mechanics/plan.md, amendment
//! "component delay"): test (a)'s plans in ready mode with the fork on every input window, every candidate and the
//! attempt verified from boot through fmi-engine's replay.verify on the lean build.
//!
//!   delay-check FIXTURES OUT P...

use fmi_core::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let host = fmi_engine::Host::from_env().expect("fmi-engine's host");
    let vhost = fmi_engine::Host::from_env().expect("fmi-engine's host");
    let out = std::path::PathBuf::from(a.get(2).cloned().unwrap_or_default()).join("verify");
    let base = BootCfg {
        seed: 1,
        events: vec![],
        frame: None,
        dump_locals: true,
        lean: true,
        out_dir: out,
    };
    let verify = |events: &[Event], cps: &[Checkpoint]| -> Result<(bool, String), String> {
        let v = fmi_engine::replay::verify(&vhost, &base, events, cps, &[]).map_err(|e| e.to_string())?;
        let txt = if v.same {
            format!("0 differences ({} checkpoints, {:.2} s)", v.checkpoints, v.seconds)
        } else {
            format!("{} DIFFER: {:?}", v.differ.len(), v.differ.iter().take(3).collect::<Vec<_>>())
        };
        Ok((v.same, txt))
    };
    let ok = fmi_play::delay_check::run(&a, Box::new(host), true, &verify);
    std::process::exit(if ok { 0 } else { 1 });
}
