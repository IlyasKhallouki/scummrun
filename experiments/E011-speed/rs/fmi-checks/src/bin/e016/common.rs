//! What the subcommands share: arguments, the boot, movies, the input log's records and attempts as JSON, and the
//! replay from boot.
#![allow(dead_code)]

use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::events::{event_json, event_of};
use fmi_play::realize::Attempt;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

pub type Res<T> = Result<T, String>;

pub fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Positional arguments and `--name value` options (a bare `--flag` before another option or at the end is "1").
pub struct Args {
    pub pos: Vec<String>,
    pub opts: BTreeMap<String, String>,
}

pub fn args(argv: &[String]) -> Args {
    let (mut pos, mut opts) = (Vec::new(), BTreeMap::new());
    let mut i = 0;
    while i < argv.len() {
        if let Some(n) = argv[i].strip_prefix("--") {
            match argv.get(i + 1) {
                Some(v) if !v.starts_with("--") => {
                    opts.insert(n.to_string(), v.clone());
                    i += 2;
                }
                _ => {
                    opts.insert(n.to_string(), "1".into());
                    i += 1;
                }
            }
        } else {
            pos.push(argv[i].clone());
            i += 1;
        }
    }
    Args { pos, opts }
}

impl Args {
    pub fn s(&self, k: &str) -> Res<&str> {
        self.opts.get(k).map(String::as_str).ok_or(format!("--{k}"))
    }
    pub fn num(&self, k: &str) -> Res<i64> {
        self.s(k)?.parse().map_err(|e| format!("--{k}: {e}"))
    }
    pub fn num_or(&self, k: &str, d: i64) -> Res<i64> {
        if self.opts.contains_key(k) {
            self.num(k)
        } else {
            Ok(d)
        }
    }
    pub fn flag(&self, k: &str) -> bool {
        self.opts.contains_key(k)
    }
}

/// A fresh lean engine from boot with `events` scheduled, run to `frame`, seed 1.
pub fn boot(h: &Host, out: &Path, events: &[Event], frame: u64) -> Res<Box<dyn Engine>> {
    let cfg = BootCfg { seed: 1, events: events.to_vec(), frame: Some(frame), dump_locals: false, lean: true, out_dir: out.to_path_buf() };
    Ok(Box::new(h.boot_bin(&cfg).map_err(e2s)?))
}

/// A movie file: one event per line, as SPEEDRUN_INPUTS writes them.
pub fn read_events(p: &str) -> Res<Vec<Event>> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).map_err(e2s).and_then(|v| event_of(&v).ok_or(format!("event: {l}"))))
        .collect()
}

pub fn write_events(p: &Path, evs: &[Event]) -> Res<()> {
    let mut s = String::new();
    for e in evs {
        s.push_str(&event_json(e).to_string());
        s.push('\n');
    }
    std::fs::write(p, s).map_err(e2s)
}

pub fn input_json(i: &Input) -> Value {
    match i {
        Input::Click(x, y) => json!({"click": [x, y]}),
        Input::RClick(x, y) => json!({"rclick": [x, y]}),
        Input::Move(x, y) => json!({"move": [x, y]}),
        Input::Key(k) => json!({"key": k}),
        Input::Skips { .. } => json!({"skips": true}),
    }
}

/// A touch as [kind, a, b, old, now].
pub fn touch_json(t: &Touch) -> Value {
    json!([format!("{:?}", t.kind), t.a, t.b, t.old, t.now])
}

/// One input log record: the path's facts, the writes in full and the reads counted by kind.
pub fn record_json(r: &InputRecord) -> Value {
    let mut reads: BTreeMap<String, usize> = BTreeMap::new();
    for t in &r.reads {
        *reads.entry(format!("{:?}", t.kind)).or_default() += 1;
    }
    json!({"frame": r.frame, "input": r.input.iter().map(input_json).collect::<Vec<_>>(), "stat": r.stat,
           "verdict": r.verdict.text(), "area": r.area, "value": r.value, "mode": r.mode, "slot": r.slot, "zone": r.zone,
           "key_match": r.key_match, "handler": r.handler, "writes": r.writes.iter().map(touch_json).collect::<Vec<_>>(),
           "reads": reads, "ops_dropped": r.ops_dropped, "touches_dropped": r.touches_dropped})
}

pub fn attempt_json(a: &Attempt) -> Value {
    json!({"phase": a.phase, "name": a.name, "accepted": a.accepted, "seen": a.seen, "frame": a.frame,
           "events": a.events.iter().map(event_json).collect::<Vec<_>>(), "seconds": a.seconds})
}

/// A fresh engine from boot with `movie`, run to `frame`; compared with `e` (stopped at `frame`) on every fact, the
/// room and the state digest.
pub fn verify(h: &Host, out: &Path, movie: &[Event], e: &mut Box<dyn Engine>) -> Res<Value> {
    let t = Instant::now();
    let frame = e.status().frame;
    let want_facts = e.facts().map_err(e2s)?;
    let want_room = e.status().room;
    let want = e.digest().map_err(e2s)?;
    let mut fresh = boot(h, out, movie, frame)?;
    let got_facts = fresh.facts().map_err(e2s)?;
    let got = fresh.digest().map_err(e2s)?;
    let st = fresh.status().clone();
    fresh.quit();
    let d = want_facts.diff(&got_facts);
    let first: Vec<Value> = d.iter().take(8).map(|(k, x, y)| json!([k.text(), x, y])).collect();
    let n = d.len() + usize::from(st.room != want_room) + usize::from(got.digest != want.digest) + usize::from(st.frame != frame);
    Ok(json!({"frame": frame, "got_frame": st.frame, "inputs": movie.iter().filter(|x| !matches!(x.what, Input::Skips { .. })).count(),
              "facts": want_facts.0.len(), "facts_differ": d.len(), "first": first, "room": [want_room, st.room],
              "digest": [format!("{:016x}", want.digest), format!("{:016x}", got.digest)],
              "differences": n, "seconds": t.elapsed().as_secs_f64()}))
}

/// The run on from the stop, one frame at a time, until `stop` holds or `frames` pass; the frame it held at (None:
/// it did not), and the end's reason when the run ended.
pub fn run_until(e: &mut Box<dyn Engine>, frames: u64, stop: &mut dyn FnMut(&mut Box<dyn Engine>) -> Res<bool>) -> Res<(Option<u64>, Option<String>)> {
    let end = e.status().frame + frames;
    while e.status().frame < end {
        let to = e.status().frame + 1;
        match e.run(&RunSpec { frame: Some(to), ..Default::default() }) {
            Ok(_) => {}
            Err(EngineError::Ended(st)) => return Ok((None, Some(st.reason.clone()))),
            Err(x) => return Err(e2s(x)),
        }
        if stop(e)? {
            return Ok((Some(e.status().frame), None));
        }
    }
    Ok((None, None))
}
