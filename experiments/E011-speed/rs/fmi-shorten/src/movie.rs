//! E024 Part 2, item 9 (plan.md, "Verify"): a version of the route, its step log, and its check from boot.
//!
//! * **The files** (`write`): OUT/NAME-NNN.jsonl (the movie: the boot's events and every placed input) and
//!   OUT/NAME-NNN.steps.jsonl (a first line of settings, the goal's end reason with them; a line per step: its inputs,
//!   its completion's frame, tick and the stage's digest there, its scan; `after_end` when its completion is after the
//!   route's end, which no digest reaches: found on its own fork, it was later than the end the next steps reached; a
//!   last line with the end). NNN is the next free number: an earlier version is never overwritten. A version is
//!   written as unverified-NNN and becomes route-NNN (`promote`) only once verify passes (the review of stage A: a
//!   route that failed verify looked like a version).
//! * **The stage's digests** (`replay`): the route from boot as the stage runs a movie (its events to BOOT_FRAME at the
//!   boot, the later ones scheduled as the run reaches them), the digest at each step's completion, and the end.
//! * **Verify** (`verify`): the version read back from its files, booted fresh twice: by the bridge's launch (the
//!   whole movie as the boot's input file, run to the first control, then to each completion for its digest, then to
//!   the end) and standalone (no control channel: the engine runs the movie to its end by itself, the end read from
//!   its trace). Both must end at the stage's site and tick, the launch with the stage's digest at every completion,
//!   and the stage's end must be the goal's (its reason, with the site). A movie without its step log fails. A step
//!   whose completion has no digest is reported unchecked (`unchecked`: after the end, or no digest taken), not passed
//!   over silently (the review of stage B).

use crate::chain::{self, Sched};
use crate::verdict::End;
use crate::{e2s, Res};
use fmi_core::*;
use fmi_explore::events::{event_json, event_of};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The step log's name for a route file.
pub fn steps_path(route: &Path) -> PathBuf {
    route.with_extension("steps.jsonl")
}

/// Writes the next free `name`-NNN.jsonl in `dir` and its step log (a step completed after `end`'s frame marked
/// `after_end`); returns the movie's path.
pub fn write(dir: &Path, name: &str, route: &[Event], meta: &Value, steps: &[Value], end: &Value) -> Res<PathBuf> {
    std::fs::create_dir_all(dir).map_err(e2s)?;
    for n in 1..1000 {
        let p = dir.join(format!("{name}-{n:03}.jsonl"));
        // never overwritten: the route and its log are made new or not at all
        let Ok(mut f) = std::fs::OpenOptions::new().write(true).create_new(true).open(&p) else { continue };
        let mut g = std::fs::OpenOptions::new().write(true).create_new(true).open(steps_path(&p)).map_err(|e| format!("{}: {e}", steps_path(&p).display()))?;
        for ev in route {
            writeln!(f, "{}", event_json(ev)).map_err(e2s)?;
        }
        writeln!(g, "{}", json!({"meta": meta})).map_err(e2s)?;
        for s in steps {
            match (s["completion"]["frame"].as_u64(), end["frame"].as_u64()) {
                (Some(c), Some(f)) if c > f => {
                    let mut s = s.clone();
                    s["after_end"] = json!(true);
                    writeln!(g, "{s}").map_err(e2s)?;
                }
                _ => writeln!(g, "{s}").map_err(e2s)?,
            }
        }
        writeln!(g, "{}", json!({"end": end})).map_err(e2s)?;
        return Ok(p);
    }
    Err(format!("{}: no free route number", dir.display()))
}

/// `p` (a version's movie, with its step log) renamed to the next free `name`-NNN.jsonl of its folder, never
/// overwriting one; returns the new path.
pub fn promote(p: &Path, name: &str) -> Res<PathBuf> {
    let dir = p.parent().ok_or(format!("{}: no folder", p.display()))?;
    for n in 1..1000 {
        let q = dir.join(format!("{name}-{n:03}.jsonl"));
        // a link fails where the name is taken: never overwritten
        if std::fs::hard_link(p, &q).is_err() {
            continue;
        }
        if let Err(e) = std::fs::hard_link(steps_path(p), steps_path(&q)) {
            let _ = std::fs::remove_file(&q);
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                continue;
            }
            return Err(format!("{}: {e}", steps_path(&q).display()));
        }
        std::fs::remove_file(p).map_err(e2s)?;
        std::fs::remove_file(steps_path(p)).map_err(e2s)?;
        return Ok(q);
    }
    Err(format!("{}: no free {name} number", dir.display()))
}

/// A route file's events.
pub fn read_route(p: &Path) -> Res<Vec<Event>> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).ok().and_then(|v| event_of(&v)).ok_or(format!("{}: not an event: {l}", p.display())))
        .collect()
}

/// What a boot of a route came to: the digest at each frame asked, and the end.
#[derive(Clone, Debug, Default)]
pub struct Boot {
    pub digests: BTreeMap<u64, u64>,
    pub end: Option<End>,
    /// frames asked that the run had passed at its first stop
    pub passed: Vec<u64>,
}

impl Boot {
    pub fn json(&self) -> Value {
        json!({"digests": self.digests.iter().map(|(f, d)| json!([f, format!("{d:016x}")])).collect::<Vec<_>>(),
               "end": self.end.as_ref().map(|e| json!({"frame": e.frame, "tick": e.tick, "reason": e.reason})),
               "passed": self.passed})
    }
}

/// `e` run to each of `frames` for its digest there, then on (at most to `max_frame`) to the game's end.
fn digests_and_end(mut e: Box<dyn Engine>, mut sched: Option<Sched>, frames: &[u64], max_frame: u64) -> Res<Boot> {
    let mut b = Boot::default();
    for &f in frames {
        if f <= e.status().frame {
            if f == e.status().frame {
                b.digests.insert(f, e.digest().map_err(e2s)?.digest);
            } else {
                b.passed.push(f);
            }
            continue;
        }
        if let Some(s) = sched.as_mut() {
            s.upto(&mut e, f)?;
        }
        match e.run(&RunSpec { frame: Some(f), ..Default::default() }) {
            Ok(_) => {
                b.digests.insert(f, e.digest().map_err(e2s)?.digest);
            }
            Err(EngineError::Ended(st)) => {
                b.end = Some(End::of(&st));
                return Ok(b);
            }
            Err(x) => return Err(e2s(x)),
        }
    }
    if let Some(s) = sched.as_mut() {
        s.upto(&mut e, u64::MAX)?;
    }
    if e.status().frame < max_frame {
        match e.run(&RunSpec { max_frame: Some(max_frame), ..Default::default() }) {
            Ok(_) => e.quit(),
            Err(EngineError::Ended(st)) => b.end = Some(End::of(&st)),
            Err(x) => return Err(e2s(x)),
        }
    } else {
        e.quit();
    }
    Ok(b)
}

/// The stage's replay of `route` (module docstring).
pub fn replay(host: &dyn EngineHost, route: &[Event], seed: u32, frames: &[u64], max_frame: u64, out: &Path) -> Res<Boot> {
    let (e, sched) = chain::boot(host, route, seed, out)?;
    digests_and_end(e, Some(sched), frames, max_frame)
}

/// The bridge's launch of `route`: every event in the boot's input file, the run to the first control stop.
pub fn launch(host: &dyn EngineHost, route: &[Event], seed: u32, frames: &[u64], max_frame: u64, out: &Path) -> Res<Boot> {
    let cfg = BootCfg { seed, events: route.to_vec(), frame: None, dump_locals: false, lean: true, out_dir: out.to_path_buf() };
    match host.boot(&cfg) {
        Ok(e) => digests_and_end(e, None, frames, max_frame),
        Err(EngineError::Ended(st)) => Ok(Boot { end: Some(End::of(&st)), ..Default::default() }),
        Err(x) => Err(e2s(x)),
    }
}

/// A standalone launch: the engine runs `route` from boot by itself (no control channel), at most to `max_tick`; its
/// end as its trace has it (None: no end).
pub type Standalone<'a> = &'a dyn Fn(&[Event], u32, u64, &Path) -> Res<Option<End>>;

/// Item 9 on the route file `route_path` and its step log (module docstring).
pub fn verify(host: &dyn EngineHost, standalone: Standalone<'_>, route_path: &Path, out: &Path) -> Res<Value> {
    let route = read_route(route_path)?;
    let sp = steps_path(route_path);
    let Ok(text) = std::fs::read_to_string(&sp) else {
        return Ok(json!({"ok": false, "why": format!("no step log: {}", sp.display())}));
    };
    let mut meta = Value::Null;
    let mut want: BTreeMap<u64, u64> = BTreeMap::new();
    // the steps whose completion has no digest: (k, frame)
    let mut bare: Vec<(Value, u64)> = Vec::new();
    let mut end: Option<End> = None;
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(l).map_err(|e| format!("{}: {e}", sp.display()))?;
        if let Some(m) = v.get("meta") {
            meta = m.clone();
        } else if let Some(e) = v.get("end") {
            end = Some(End {
                tick: e["tick"].as_u64().unwrap_or(0),
                frame: e["frame"].as_u64().unwrap_or(0),
                reason: e["reason"].as_str().unwrap_or("").to_string(),
            });
        } else if let Some(f) = v["completion"]["frame"].as_u64() {
            match v["digest"].as_str() {
                Some(d) => {
                    want.insert(f, u64::from_str_radix(d, 16).map_err(e2s)?);
                }
                None => bare.push((v["k"].clone(), f)),
            }
        }
    }
    let Some(end) = end.filter(|e| !e.reason.is_empty()) else {
        return Ok(json!({"ok": false, "why": "the step log has no end"}));
    };
    // the end must be the goal's: the game's end with its reason and site (the review of stage A)
    let goal = meta["goal_end"].as_str().unwrap_or("");
    if end.reason != goal {
        return Ok(json!({"ok": false, "why": format!("the step log's end {:?} is not the goal's {goal:?}", end.reason)}));
    }
    // unchecked: a completion after the end (the game ended first), or before it with no digest taken; the goal's own
    // at the end is the end's check
    let unchecked: Vec<Value> = bare
        .iter()
        .filter(|(_, f)| *f != end.frame)
        .map(|(k, f)| json!({"k": k, "frame": f, "why": if *f > end.frame { "after the end" } else { "no digest" }}))
        .collect();
    let seed = meta["seed"].as_u64().unwrap_or(1) as u32;
    let frames: Vec<u64> = want.keys().copied().collect();
    let max_frame = end.frame + 1000;
    let a = launch(host, &route, seed, &frames, max_frame, &out.join("launch"))?;
    let b = standalone(&route, seed, end.tick + 60_000, &out.join("standalone"))?;
    let mut miss = Vec::new();
    for (f, d) in &want {
        match a.digests.get(f) {
            Some(x) if x == d => {}
            got => miss.push(json!({"frame": f, "want": format!("{d:016x}"), "got": got.map(|x| format!("{x:016x}"))})),
        }
    }
    let same = |x: &Option<End>| x.as_ref() == Some(&end);
    let ok = miss.is_empty() && same(&a.end) && same(&b);
    Ok(json!({"ok": ok, "route": route_path.display().to_string(), "events": route.len(),
              "end": {"frame": end.frame, "tick": end.tick, "reason": end.reason}, "unchecked": unchecked,
              "launch": {"same_end": same(&a.end), "digests": want.len(), "digest_miss": miss, "boot": a.json()},
              "standalone": {"same_end": same(&b), "end": b.as_ref().map(|e| json!({"frame": e.frame, "tick": e.tick, "reason": e.reason}))}}))
}
