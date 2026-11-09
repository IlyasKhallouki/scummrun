//! `e024 g0` (plan.md, "Gates", G0): see main.rs.

use fmi_core::*;
use fmi_engine::Host;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Res<T> = Result<T, String>;

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// `--name value` options after the positional OUT.
fn opt(argv: &[String], k: &str) -> Option<String> {
    argv.iter().position(|a| a == &format!("--{k}")).and_then(|i| argv.get(i + 1).cloned())
}

/// What a boot came to: its first control, or its end.
struct Boot {
    out: Value,
    v19: Option<i32>,
    digest: Option<u64>,
}

/// The bridge's settings for the next boot: each variable set in this process's environment (the host passes it on,
/// host.rs) or removed.
fn settings(start_frame: Option<u64>, timer_next: Option<i32>) {
    match start_frame {
        Some(f) => std::env::set_var("SPEEDRUN_START_FRAME", f.to_string()),
        None => std::env::remove_var("SPEEDRUN_START_FRAME"),
    }
    match timer_next {
        Some(v) => std::env::set_var("SPEEDRUN_TIMER_NEXT", v.to_string()),
        None => std::env::remove_var("SPEEDRUN_TIMER_NEXT"),
    }
}

/// The trace's skip records by kind, the segment start's record, and the end's and an error's.
fn trace_of(dir: &Path) -> Value {
    let mut skips: BTreeMap<String, usize> = BTreeMap::new();
    let mut seg = Value::Null;
    let (mut end, mut err) = (Value::Null, Value::Null);
    if let Ok(t) = std::fs::read_to_string(dir.join("trace.jsonl")) {
        for l in t.lines() {
            let Ok(v) = serde_json::from_str::<Value>(l) else { continue };
            match v["type"].as_str() {
                Some("skip") => *skips.entry(v["kind"].as_str().unwrap_or("?").to_string()).or_default() += 1,
                Some("segment_start") => seg = json!({"frame": v["frame"], "tick": v["tick"], "var19": v["var19"]}),
                Some("end") => end = json!({"frame": v["frame"], "reason": v["reason"]}),
                Some("error") => err = json!({"frame": v["frame"], "code": v["code"]}),
                _ => {}
            }
        }
    }
    json!({"skips": skips, "segment_start": seg, "end": end, "error": err})
}

/// One boot with `events` to its first control (at most `max_frame`).
fn boot(dir: &Path, events: &[Event], max_frame: u64, seed: u32) -> Res<Boot> {
    let _ = std::fs::remove_dir_all(dir);
    let h = Host::from_env().map_err(e2s)?;
    let cfg = BootCfg { seed, events: events.to_vec(), frame: None, dump_locals: false, lean: true, out_dir: dir.to_path_buf() };
    // an end at the first stop (a check that fails at frame 1) is the boot's end too
    let r = h.spawn(&cfg).and_then(|mut e| e.run(&RunSpec { idle: true, max_frame: Some(max_frame), ..Default::default() }).map(|st| (e, st)));
    let b = match r {
        Ok((mut e, st)) => {
            let d = e.digest().map_err(e2s)?;
            let b = Boot {
                out: json!({"stop": st.reason, "frame": st.frame, "tick": st.tick, "seconds": st.tick as f64 / 60.0,
                            "room": st.room, "v19": st.v19, "idle": st.idle, "seg": st.seg, "tick0": st.tick0,
                            "digest": format!("{:016x}", d.digest)}),
                v19: Some(st.v19),
                digest: Some(d.digest),
            };
            Box::new(e).quit();
            b
        }
        Err(EngineError::Ended(st)) => Boot {
            out: json!({"ended": st.reason, "frame": st.frame, "tick": st.tick, "room": st.room, "v19": st.v19}),
            v19: None,
            digest: None,
        },
        Err(x) => return Err(e2s(x)),
    };
    let mut out = b.out;
    out["trace"] = trace_of(dir);
    Ok(Boot { out, ..b })
}

/// The first frame of the boot with no skips whose stop has var 19 = `v`, before its first control.
fn first_v19(dir: &Path, v: i32, max_frame: u64, seed: u32) -> Res<Option<u64>> {
    let _ = std::fs::remove_dir_all(dir);
    let h = Host::from_env().map_err(e2s)?;
    let cfg = BootCfg { seed, events: vec![], frame: None, dump_locals: false, lean: true, out_dir: dir.to_path_buf() };
    let mut e = h.spawn(&cfg).map_err(e2s)?;
    let mut found = None;
    while e.status().frame < max_frame {
        let to = e.status().frame + 1;
        let st = e.run(&RunSpec { frame: Some(to), ..Default::default() }).map_err(e2s)?;
        if st.v19 == v {
            found = Some(st.frame);
            break;
        }
        if st.idle.is_some() {
            break;
        }
    }
    Box::new(e).quit();
    Ok(found)
}

fn skips_at(f: u64) -> Vec<Event> {
    vec![Event { frame: f, what: Input::Skips { text: true, cutscenes: true, spare: None } }]
}

pub fn run(argv: &[String]) -> Res<()> {
    let out = PathBuf::from(argv.first().ok_or("OUT")?);
    std::fs::create_dir_all(&out).map_err(e2s)?;
    let seed: u32 = opt(argv, "seed").map(|s| s.parse().map_err(e2s)).transpose()?.unwrap_or(1);
    let maxf: u64 = opt(argv, "max-frame").map(|s| s.parse().map_err(e2s)).transpose()?.unwrap_or(20_000);
    settings(None, None);
    let std_ = boot(&out.join("std"), &[], maxf, seed)?;
    eprintln!("std {}", std_.out);
    settings(Some(1), None);
    let a = boot(&out.join("a"), &[], maxf, seed)?;
    eprintln!("a {}", a.out);
    let b = boot(&out.join("b"), &skips_at(1), maxf, seed)?;
    eprintln!("b {}", b.out);
    let c = json!({"v19_a": a.v19, "v19_b": b.v19, "equal": a.v19.is_some() && a.v19 == b.v19});
    let mut doc = json!({"seed": seed, "std": std_.out, "a": a.out, "b": b.out, "c": c,
                         "a_digest_is_std": a.digest.is_some() && a.digest == std_.digest});
    // the var-19 rule: TIMER_NEXT is (a)'s var 19, measured, not written in
    if let Some(v) = a.v19 {
        settings(Some(1), Some(v));
        let d = boot(&out.join("d"), &skips_at(1), maxf, seed)?;
        eprintln!("d {}", d.out);
        let failed = d.v19.is_none();
        doc["d"] = d.out;
        doc["timer_next"] = json!(v);
        // the host passes TIMER_NEXT on: any other value fails the check at the segment start
        settings(Some(1), Some(v + 1));
        let x = boot(&out.join("other"), &skips_at(1), maxf, seed)?;
        doc["other_value_ends"] = x.out;
        if failed {
            settings(None, None);
            let f1 = first_v19(&out.join("v19scan"), v, maxf, seed)?;
            doc["first_v19_frame"] = json!(f1);
            if let Some(f1) = f1 {
                settings(Some(f1), Some(v));
                let e = boot(&out.join("e"), &skips_at(f1), maxf, seed)?;
                eprintln!("e {}", e.out);
                doc["e"] = e.out;
                doc["e_v19_equal"] = json!(e.v19 == Some(v));
            }
        }
    }
    settings(None, None);
    std::fs::write(out.join("g0.json"), serde_json::to_string_pretty(&doc).map_err(e2s)?).map_err(e2s)?;
    println!("{doc}");
    Ok(())
}
