//! `fmi read FILE` (E012's component `resident`, its gate and its measure): readings one after another through fmi's
//! readings port (wire_real.rs `RealReadings`): reader-scope as a child process, or, with the switch FMI_READER_LIB=1,
//! this process with the game and the job memo's store resident. FILE has one reading per line, reader-scope's
//! arguments split at spaces (no quoting); empty lines and lines that start with # are left out. The port adds its
//! own arguments as for a try (FMI_MEMO's --memo, FMI_READER_PREP's --prep, the switch's). One JSON line per
//! reading: its wall and CPU seconds in this call (this process's and its children's), the report's parts, the
//! memo's counts and the memory.

use crate::wire_real::RealReadings;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn status_mb(field: &str) -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines().find(|l| l.starts_with(field)).and_then(|l| l.split_whitespace().nth(1)).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0) / 1024
}

pub fn cmd_read(argv: &[String]) -> i32 {
    let Some(file) = argv.first() else {
        eprintln!("usage: fmi read FILE");
        return 2;
    };
    let reader = PathBuf::from(std::env::var("FMI_READER_SCOPE").unwrap_or_default());
    let rr = match RealReadings::new(reader, fmi_explore::guide::default_root()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("fmi read: {e}");
            return 2;
        }
    };
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("fmi read {file}: {e}");
            return 2;
        }
    };
    println!(
        "{}",
        json!({"allocator": fmi_reading::inproc::allocator(), "inproc": rr.inproc.is_some(),
               "memo": rr.memo.as_ref().map(|m| m.display().to_string()), "reader": rr.reader.display().to_string()})
    );
    let cancel = AtomicBool::new(false);
    let mut bad = 0;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let args: Vec<String> = line.split_whitespace().map(String::from).collect();
        let get = |f: &str| args.iter().position(|x| x == f).and_then(|i| args.get(i + 1).cloned());
        let name = get("--name").unwrap_or_default();
        let dir = PathBuf::from(get("--out-dir").or_else(|| get("--out")).unwrap_or_else(|| ".".into()));
        let report = PathBuf::from(get("--out").or_else(|| get("--out-dir")).unwrap_or_else(|| ".".into())).join(format!("{name}.json"));
        let (s0, c0) = fmi_reading::inproc::rusage();
        let t = Instant::now();
        let r = rr.read_args(args, &dir, &cancel);
        let wall = t.elapsed().as_secs_f64();
        let (s1, c1) = fmi_reading::inproc::rusage();
        let rep: Value = std::fs::read_to_string(&report).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let r2 = |x: f64| (x * 100.0).round() / 100.0;
        let m = &rep["memo"];
        let out = json!({
            "name": name,
            "ok": r.is_ok(),
            "error": r.as_ref().err(),
            "args": r.as_ref().ok(),
            "wall_s": r2(wall),
            "cpu_s": r2((s1 - s0) + (c1 - c0)),
            "self_cpu_s": r2(s1 - s0),
            "child_cpu_s": r2(c1 - c0),
            "status": rep["status"],
            "records": rep["records"],
            "read_wall_s": rep["read_wall_s"],
            "read_cpu_s": rep["read_cpu_s"],
            "setup_s": rep["setup_s"],
            "total_s": rep["total_s"],
            "peak_rss_mb": rep["peak_rss_mb"],
            "reading_rss_peak_seen_mb": rep["rss_peak_seen"]["mb"],
            "process_rss_mb": status_mb("VmRSS:"),
            "process_hwm_mb": status_mb("VmHWM:"),
            "children_maxrss_mb": fmi_reading::inproc::children_maxrss_mb(),
            "memo": if m.is_null() { Value::Null } else { json!({
                "dir": m["dir"], "files_opened": m["files_opened"], "runs_opened": m["runs_opened"], "taken": m["taken"],
                "ran_no_stored_run": m["ran_no_stored_run"], "ran_key_differs": m["ran_key_differs"],
                "ran_list_differs": m["ran_list_differs"], "ran_reached_differs": m["ran_reached_differs"],
                "stored": m["stored"], "stored_mb": m["stored_mb"], "lookup_s": m["lookup_s"], "store_s": m["store_s"]}) },
            "memo_in_memory_mb": rr.inproc.as_ref().map(|ip| ip.memo_bytes() >> 20),
            "pool_job_s": rep["pool"]["job_s"],
            "pool_jobs": rep["pool"]["jobs"],
            "merge_and_stream_s": rep["pool"]["merge_and_stream_s"],
            "sink_thread_busy_s": rep["sink_thread_busy_s"],
            "reading_thread_cpu_s": rep["reading_thread_cpu_s"],
            "sink_thread_cpu_s": rep["sink_thread_cpu_s"],
            "files": rep["files"],
            "packed_finish_s": rep["packed"]["finish_s"],
            "load": rep["load_at_end"],
        });
        println!("{out}");
        if r.is_err() {
            bad += 1;
        }
    }
    (bad > 0) as i32
}
