//! E024 Part 2, item 8 (plan.md, "Skips from boot"): the bridge's text and cutscene skips from the boot on.
//!
//! The segment starts at frame 1 (SPEEDRUN_START_FRAME) with Skips{text, cutscenes} at frame 1. SPEEDRUN_TIMER_NEXT,
//! the var 19 the bridge requires whenever the player has control, is var 19 at the first control of a boot without
//! skips: measured from the game, not written in. The var-19 rule: when the start fails the bridge's check (its
//! "timer_next" error) or the first control under skips shows another var 19, the start moves to the first frame of a
//! boot without skips whose var 19 is that value, the skips with it. Any other end is no reason to move it: it fails.

use crate::{e2s, Res};
use fmi_core::*;
use serde_json::{json, Value};
use std::path::Path;

/// A host whose engines get these bridge settings on top of its own.
pub type MakeHost<'a> = &'a dyn Fn(&[(String, String)]) -> Box<dyn EngineHost>;

/// The settings of a run with skips from boot.
#[derive(Clone, Debug)]
pub struct Setup {
    /// the bridge's settings (SPEEDRUN_START_FRAME, SPEEDRUN_TIMER_NEXT)
    pub env: Vec<(String, String)>,
    /// the segment's start frame, where the Skips event goes
    pub start: u64,
    pub timer_next: i32,
    pub report: Value,
}

/// The first control stop of a boot with `events` (the bridge's first idle stop), or its end.
fn first_control(host: &dyn EngineHost, events: &[Event], seed: u32, out: &Path) -> Res<Result<Status, Status>> {
    let _ = std::fs::remove_dir_all(out);
    let cfg = BootCfg { seed, events: events.to_vec(), frame: None, dump_locals: false, lean: true, out_dir: out.to_path_buf() };
    match host.boot(&cfg) {
        Ok(e) => {
            let st = e.status().clone();
            e.quit();
            Ok(Ok(st))
        }
        Err(EngineError::Ended(st)) => Ok(Err(*st)),
        Err(x) => Err(e2s(x)),
    }
}

/// The bridge's error code in a boot's trace (the last error record's), if any.
fn trace_error(out: &Path) -> Option<String> {
    let t = std::fs::read_to_string(out.join("trace.jsonl")).ok()?;
    t.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["type"] == "error")
        .last()
        .and_then(|v| v["code"].as_str().map(String::from))
}

fn skips_at(f: u64) -> Vec<Event> {
    vec![Event { frame: f, what: Input::Skips { text: true, cutscenes: true, spare: None } }]
}

fn kv(k: &str, v: impl ToString) -> (String, String) {
    (k.to_string(), v.to_string())
}

fn stop_json(r: &Result<Status, Status>) -> Value {
    match r {
        Ok(st) => json!({"frame": st.frame, "tick": st.tick, "v19": st.v19, "room": st.room, "idle": st.idle}),
        Err(st) => json!({"ended": st.reason, "frame": st.frame, "tick": st.tick}),
    }
}

/// Item 8 (module docstring).
pub fn setup(make: MakeHost<'_>, seed: u32, out: &Path) -> Res<Setup> {
    let a = first_control(&*make(&[kv("SPEEDRUN_START_FRAME", 1)]), &[], seed, &out.join("a"))?;
    let v = match &a {
        Ok(st) => st.v19,
        Err(st) => return Err(format!("the boot without skips ended at frame {}: {}", st.frame, st.reason)),
    };
    let env = vec![kv("SPEEDRUN_START_FRAME", 1), kv("SPEEDRUN_TIMER_NEXT", v)];
    let b = first_control(&*make(&env), &skips_at(1), seed, &out.join("b"))?;
    let mut report = json!({"timer_next": v, "a": stop_json(&a), "b": stop_json(&b)});
    let failed = match &b {
        Ok(st) => st.v19 != v,
        Err(_) => trace_error(&out.join("b")).as_deref() == Some("timer_next"),
    };
    if !failed {
        if let Err(st) = &b {
            return Err(format!("the boot with skips from frame 1 ended at frame {}: {}", st.frame, st.reason));
        }
        report["start"] = json!(1);
        return Ok(Setup { env, start: 1, timer_next: v, report });
    }
    // the var-19 rule: the first frame of a boot without skips whose var 19 is v
    let host = make(&[]);
    let cfg = BootCfg { seed, events: vec![], frame: Some(crate::chain::BOOT_FRAME), dump_locals: false, lean: true, out_dir: out.join("v19") };
    let mut e = host.boot(&cfg).map_err(e2s)?;
    let mut f1 = None;
    while e.status().idle.is_none() {
        let st = e.run(&RunSpec { frame: Some(e.status().frame + 1), ..Default::default() }).map_err(e2s)?;
        if st.v19 == v {
            f1 = Some(st.frame);
            break;
        }
    }
    e.quit();
    let f1 = f1.ok_or("the var-19 rule: no frame before the first control has its var 19")?;
    let env = vec![kv("SPEEDRUN_START_FRAME", f1), kv("SPEEDRUN_TIMER_NEXT", v)];
    let c = first_control(&*make(&env), &skips_at(f1), seed, &out.join("c"))?;
    report["start"] = json!(f1);
    report["c"] = stop_json(&c);
    match c {
        Ok(st) if st.v19 == v => Ok(Setup { env, start: f1, timer_next: v, report }),
        _ => Err(format!("the var-19 rule: the start at frame {f1} fails too: {report}")),
    }
}
