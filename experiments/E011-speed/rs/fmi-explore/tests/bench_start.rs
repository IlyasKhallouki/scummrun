//! Timings of the harness's per-plan work on a recorded state (run with --release --nocapture; skipped without the
//! recorded files).

use fmi_explore::start;
use fmi_explore::machine::thread_cpu_s as cpu;

#[test]
fn per_plan_costs() {
    let home = std::env::var("HOME").unwrap_or_default();
    let job = std::path::PathBuf::from(format!("{home}/.cache/fast-mi-e009/off/step-a/jobs/ta/job.json"));
    let Ok(j) = fmi_explore::files::read_json(&job) else { return };
    let obs = start::facts_of_json(&j["entry"]["checkpoint"][1]);
    let n = 200;
    let t = cpu();
    for _ in 0..n {
        let _ = start::input_key(2728, 0, &obs);
    }
    eprintln!("input_key: {:.3} ms a call ({} keys)", (cpu() - t) * 1e3 / n as f64, obs.len());
    let t = cpu();
    for _ in 0..n {
        let _ = start::reach_start(&obs);
    }
    eprintln!("reach_start: {:.3} ms a call", (cpu() - t) * 1e3 / n as f64);
    let t = cpu();
    for _ in 0..n {
        let mut s = String::new();
        for (k, v) in &obs.0 {
            s.push_str(&start::key_text(*k));
            s.push_str(&fmi_core::val_text(*v));
        }
    }
    eprintln!("texts: {:.3} ms a call", (cpu() - t) * 1e3 / n as f64);
    let t = cpu();
    let big = "x".repeat(100_000);
    for _ in 0..n {
        let _ = sha1_smol::Sha1::from(big.as_bytes()).digest().to_string();
    }
    eprintln!("sha1 of 100 KB: {:.3} ms", (cpu() - t) * 1e3 / n as f64);
}
