//! `fmi check wait`: the wait's gates (experiments/E012-game-mechanics/plan.md, the `wait` row, and the amendment
//! "the `wait` component"). From entries of an archive, read only (its entries.jsonl and entry files; nothing there is
//! written), each rebuilt from boot from its movie:
//!
//! * W1: each stop of the wait is verified from boot, as an entry would be (the entry's movie, its chain's checkpoints
//!   and the stop's), 0 differences;
//! * W2: the frames of the stops and the due frames read at the entry (`fmi_core::due`), and the due model checked on
//!   the engine: each paused slot is still paused at its due frame - 1 with the delay the rule gives, and has run at
//!   its due frame;
//! * W3: whether the wait is static;
//! * W4: the wait's seconds, run `repeat` times;
//! * the differential checks: T's values from the changes against `start::view` of the whole facts at every frame,
//!   and the wait's idle record against E009's rule on the whole facts of each frame (`step::idle_rule_facts`).
//!
//! T comes from a reading: as section 1 defines it, from the reading's records (`atoms_of`), until `tested` lands.

use crate::events;
use crate::files::{self, Res};
use crate::ports::{ReadReq, Readings};
use crate::pyjson;
use crate::start;
use crate::step;
use crate::tryjob::compare_checkpoint;
use crate::wait::{self, WaitCfg};
use fmi_core::{BootCfg, Engine, EngineHost, Key, ObsReq, ReadingSource, RunSpec, Val, SS_PAUSED};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

pub struct WaitCheckArgs {
    pub archive: PathBuf,
    pub entries: Vec<usize>,
    /// T's reading (its reading.json); else `read_scope`
    pub reading: Option<PathBuf>,
    /// a fresh reading of these rooms from the first entry's state, into `read_dir`
    pub read_scope: Option<Vec<i64>>,
    pub read_dir: Option<PathBuf>,
    pub out: PathBuf,
    pub verify: bool,
    pub view_check: bool,
    pub due_check: bool,
    pub idle_check: bool,
    pub repeat: usize,
}

/// The archive, read only: its rows' parents and frames, the entry files, the stage's goal.
pub struct ArchiveRo {
    pub root: PathBuf,
    pub parent: Vec<Option<usize>>,
    pub frame: Vec<u64>,
    pub room: Vec<i64>,
    pub goal: Vec<(String, Vec<i64>)>,
}

impl ArchiveRo {
    pub fn open(root: &Path) -> Res<ArchiveRo> {
        let text = std::fs::read_to_string(root.join("entries.jsonl")).map_err(|e| files::io_err(&root.join("entries.jsonl"), e))?;
        let (mut parent, mut frame, mut room) = (Vec::new(), Vec::new(), Vec::new());
        for (n, line) in text.lines().enumerate() {
            let r: Value = serde_json::from_str(line).map_err(|e| format!("entries.jsonl line {}: {e}", n + 1))?;
            if r["id"].as_u64() != Some(n as u64) {
                return Err(format!("entries.jsonl line {}: id {}", n + 1, r["id"]));
            }
            parent.push(r["parent"].as_u64().map(|p| p as usize));
            frame.push(r["frame"].as_u64().unwrap_or(0));
            room.push(r["room"].as_i64().unwrap_or(0));
        }
        let stage = files::read_json(&root.join("stage.json"))?;
        let goal = step::goal_of(&stage);
        Ok(ArchiveRo { root: root.to_path_buf(), parent, frame, room, goal })
    }

    pub fn chain(&self, i: usize) -> Vec<usize> {
        let mut out = vec![i];
        while let Some(p) = self.parent[*out.last().unwrap()] {
            out.push(p);
        }
        out.reverse();
        out
    }

    fn entry(&self, i: usize) -> Res<Value> {
        files::read_gz(&self.root.join("entries").join(format!("{i}.json.gz")))
    }

    /// (the movie from boot, the chain's checkpoints)
    pub fn movie_cps(&self, i: usize) -> Res<(Vec<Value>, Vec<Value>)> {
        let (mut ev, mut cps) = (Vec::new(), Vec::new());
        for j in self.chain(i) {
            let e = self.entry(j)?;
            ev.extend(e["events"].as_array().cloned().unwrap_or_default());
            cps.extend(e["checkpoints"].as_array().cloned().unwrap_or_default());
        }
        Ok((ev, cps))
    }
}

/// The unit an event's record runs on (its source's second field: script:N, local:N, obj:N, entry, exit), and the keys
/// of its own unit: the slot (script:N, local:N, objscript:N), its position (pos:, at:), arguments (args:), freeze
/// (frozen:) and Esc point (escpos:, esc:).
fn own_unit(unit: &str, k: &str) -> bool {
    let slot = match unit.split_once(':') {
        Some(("obj", n)) => format!("objscript:{n}"),
        Some(_) => unit.to_string(),
        None => String::new(),
    };
    (!slot.is_empty() && k == slot)
        || [format!("pos:{unit}"), format!("args:{unit}"), format!("frozen:{unit}"), format!("escpos:{unit}")].iter().any(|x| x == k)
        || k.starts_with(&format!("at:{unit}:"))
        || k.starts_with(&format!("esc:{unit}:"))
}

/// T as section 1 defines it, from the reading's records, by `tested`'s rule (its amendment): for each action, the
/// keys of its line's pre_keys that its precondition holds, with the precondition's value set, less an event's keys on
/// its own unit; plus the goal's atoms. Each atom's values sorted, each atom once, in (key, values) order. With the
/// counts T1 names: keys tested by inputs, by other units' events, in all.
pub fn atoms_of(rd: &dyn ReadingSource) -> Res<(Vec<(Key, Vec<Val>)>, Value)> {
    let mut atoms: BTreeSet<(Key, Vec<Val>)> = BTreeSet::new();
    let (mut ik, mut ek) = (BTreeSet::new(), BTreeSet::new());
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let mut not_held = 0u64;
    for i in 0..rd.n_actions() {
        let line: Value = serde_json::from_str(rd.action_line(i)).map_err(|e| format!("action line {i}: {e}"))?;
        let src = &line["source"];
        let kind = src.get(0).and_then(|s| s.as_str()).unwrap_or("?").to_string();
        let unit = if kind == "event" { src.get(1).and_then(|s| s.as_str()).map(|s| s.to_string()) } else { None };
        *kinds.entry(kind).or_insert(0) += 1;
        let pre = rd.action(i).pre;
        for k in line["pre_keys"].as_array().into_iter().flatten().filter_map(|k| k.as_str()) {
            if unit.as_deref().is_some_and(|u| own_unit(u, k)) {
                continue;
            }
            let key = Key::parse(k);
            let Some((_, vs)) = pre.iter().find(|p| p.0 == key) else {
                not_held += 1;
                continue;
            };
            if unit.is_some() { ek.insert(key) } else { ik.insert(key) };
            let mut vs = vs.clone();
            vs.sort_unstable();
            vs.dedup();
            atoms.insert((key, vs));
        }
    }
    for (k, vs) in rd.goal() {
        let mut vs = vs.clone();
        vs.sort_unstable();
        vs.dedup();
        atoms.insert((*k, vs));
    }
    let atoms: Vec<(Key, Vec<Val>)> = atoms.into_iter().collect();
    let keys: BTreeSet<Key> = atoms.iter().map(|a| a.0).collect();
    let counts = json!({"actions": kinds, "input_keys": ik.len(), "event_keys": ek.len(), "keys": keys.len(),
        "pre_keys_not_held": not_held, "atoms": atoms.len(), "observed_keys": wait::observed_keys(&atoms).len()});
    Ok((atoms, counts))
}

fn eng<E: std::fmt::Display>(e: E) -> String {
    format!("EngineError: {e}")
}

/// An engine at the archive's entry, booted from its movie, and whether it equals the entry's checkpoint.
fn boot_entry(host: &Arc<dyn EngineHost>, ar: &ArchiveRo, i: usize, out: &Path) -> Res<(Box<dyn Engine>, Vec<Value>, Vec<Value>, Value)> {
    let (movie, cps) = ar.movie_cps(i)?;
    let evs: Vec<fmi_core::Event> = movie.iter().filter_map(events::event_of).collect();
    let t = Instant::now();
    let mut b = host
        .boot(&BootCfg { seed: 1, events: evs, frame: Some(ar.frame[i]), dump_locals: false, lean: true, out_dir: out.to_path_buf() })
        .map_err(eng)?;
    let o = b.obs(&ObsReq::default()).map_err(eng)?;
    let st = b.status().clone();
    let mut d = compare_checkpoint(cps.last().unwrap_or(&Value::Null), &o, &st);
    if st.frame != ar.frame[i] {
        d.insert("frame".into(), json!({"want": ar.frame[i], "got": st.frame}));
    }
    let ret = json!({"same": d.is_empty(), "differ": Value::Object(d), "boot_s": pyjson::round(t.elapsed().as_secs_f64(), 2)});
    Ok((b, movie, cps, ret))
}

/// The due model on the engine: for each paused slot at the entry, a fork run to its due frame - 1 shows the slot
/// still in the same wait with delay D - (k - 1) * delta (k = due - frame0); run one frame more, it has run (no longer
/// paused at the same place with a lower delay).
fn due_check(root: &mut Box<dyn Engine>) -> Res<Vec<Value>> {
    let o0 = root.obs(&ObsReq::default()).map_err(eng)?;
    let f0 = o0.frame;
    let Some(d) = fmi_core::delay_step(&o0) else { return Ok(vec![]) };
    let mut out = Vec::new();
    for s in o0.slots.iter().filter(|s| s.status == SS_PAUSED) {
        let Some(f) = fmi_core::due(&o0, s.slot) else { continue };
        let k = (f - f0) as i64;
        let mut b = root.fork().map_err(eng)?;
        let mut row = json!({"slot": s.slot, "unit": start::unit_name(s.where_, s.number), "delay": s.delay, "due": k});
        let r: Res<()> = (|| {
            if k > 1 {
                b.run(&RunSpec { frame: Some(f - 1), ..Default::default() }).map_err(eng)?;
            }
            let a = b.obs(&ObsReq::default()).map_err(eng)?;
            let sa = a.slots.iter().find(|x| x.slot == s.slot);
            let want = i64::from(s.delay) - (k - 1) * d;
            let before = sa.is_some_and(|x| x.status == SS_PAUSED && x.number == s.number && x.where_ == s.where_ && x.offs == s.offs
                && i64::from(x.delay) == want);
            b.run(&RunSpec { frame: Some(f), ..Default::default() }).map_err(eng)?;
            let c = b.obs(&ObsReq::default()).map_err(eng)?;
            let sc = c.slots.iter().find(|x| x.slot == s.slot);
            let ran = !sc.is_some_and(|x| x.status == SS_PAUSED && x.number == s.number && x.offs == s.offs && i64::from(x.delay) <= want);
            row["paused_before"] = json!(before);
            row["ran_at_due"] = json!(ran);
            row["v19_same"] = json!(a.vars.get(19) == o0.vars.get(19));
            Ok(())
        })();
        if let Err(e) = r {
            row["error"] = json!(e);
        }
        b.quit();
        out.push(row);
    }
    Ok(out)
}

/// E009's idle rule on the whole facts of each frame (round 1's form, `step::idle_rule_facts`), from a fork.
fn idle_whole(root: &mut Box<dyn Engine>, frames: u64) -> Res<Value> {
    let mut b = root.fork().map_err(eng)?;
    let f0 = b.status().frame;
    let start_f = b.facts().map_err(eng)?;
    let mut seq = Vec::with_capacity(frames as usize);
    for i in 1..=frames {
        b.run(&RunSpec { frame: Some(f0 + i), ..Default::default() }).map_err(eng)?;
        seq.push((i as i64, b.facts().map_err(eng)?));
    }
    b.quit();
    Ok(step::idle_rule_facts(&start_f, seq))
}

/// replay.verify on a fresh lean engine: the movie's events from boot, each checkpoint's frame in turn (the boot
/// stops at the first one), each compared (`compare_checkpoint`). The differences, as replay.verify lists them.
/// (`HostVerifier::replay_verify` runs to the first checkpoint's frame after booting to it, which the bridge refuses.)
pub fn verify_from_boot(host: &Arc<dyn EngineHost>, movie: &[Value], cps: &[Value], out_dir: &Path) -> Res<Vec<Value>> {
    let evs: Vec<fmi_core::Event> = movie.iter().filter_map(events::event_of).collect();
    let first = cps.first().and_then(|c| c[0].as_u64());
    // the boot replays the events up to its frame (the host's prefix); the rest are scheduled at that stop
    let mut b = host
        .boot(&BootCfg { seed: 1, events: evs.clone(), frame: first, dump_locals: false, lean: true, out_dir: out_dir.to_path_buf() })
        .map_err(|e| format!("verify: {e}"))?;
    if let Some(f) = first {
        let mut rest: Vec<fmi_core::Event> = evs.into_iter().filter(|e| e.frame > f).collect();
        rest.sort_by_key(|e| e.frame);
        if !rest.is_empty() {
            b.inputs(&rest).map_err(|e| format!("verify: {e}"))?;
        }
    }
    let mut differ = Vec::new();
    for cp in cps {
        let frame = cp[0].as_u64().unwrap_or(0);
        if b.status().frame < frame {
            if let Err(e) = b.run(&RunSpec { frame: Some(frame), ..Default::default() }) {
                differ.push(json!({"frame": frame, "error": format!("stopped at {e}")}));
                break;
            }
        }
        let st = b.status().clone();
        if st.frame != frame {
            differ.push(json!({"frame": frame, "error": format!("stopped at {}", st.frame)}));
            break;
        }
        let s = b.obs(&ObsReq::default()).map_err(|e| format!("verify: {e}"))?;
        let d = compare_checkpoint(cp, &s, &st);
        if !d.is_empty() {
            let mut m = serde_json::Map::new();
            m.insert("frame".into(), json!(frame));
            m.extend(d);
            differ.push(Value::Object(m));
        }
    }
    b.quit();
    Ok(differ)
}

/// The fresh reading of `scope` from the engine's state (tryjob's read_fresh with E009's settings).
fn read_fresh(readings: &Arc<dyn Readings>, b: &mut Box<dyn Engine>, scope: &[i64], dir: &Path, goal: &[(String, Vec<i64>)])
              -> Res<Arc<dyn ReadingSource>> {
    std::fs::create_dir_all(dir).map_err(|e| files::io_err(dir, e))?;
    let state = b.obs(&ObsReq::default()).map_err(eng)?;
    let status = b.status().clone();
    let rows = dir.join("start.jsonl");
    std::fs::write(&rows, step::start_row_text(step::SEED, &state, &status)? + "\n").map_err(|e| files::io_err(&rows, e))?;
    let mut rooms = scope.to_vec();
    rooms.sort();
    let req = ReadReq {
        rooms,
        seed: step::SEED,
        goal_text: step::goal_text(goal)?,
        name: dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        out: dir.to_path_buf(),
        threads: 3,
        slot_locals: vec![],
        domain_goals: "none".into(),
        elsewhere: true,
        live_locals: true,
        split_limit: true,
        switches: step::SWITCHES.into(),
        reach: true,
        frame_costs: true,
        reach_also: vec![],
        start_rows: Some(rows),
        fact_vars: vec![],
        stream: true,
    };
    let p = readings.read(&req, &AtomicBool::new(false))?;
    readings.open(&p)
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s.get(s.len() / 2).copied().unwrap_or(0.0)
}

/// The check: one record per entry, and the reading's T counts.
pub fn wait_check(host: Arc<dyn EngineHost>, readings: Arc<dyn Readings>, a: &WaitCheckArgs) -> Res<Value> {
    let ar = ArchiveRo::open(&a.archive)?;
    std::fs::create_dir_all(&a.out).map_err(|e| files::io_err(&a.out, e))?;
    let mut rd: Option<Arc<dyn ReadingSource>> = match &a.reading {
        Some(p) => Some(readings.open(p)?),
        None => None,
    };
    let mut t_atoms: Option<(Vec<(Key, Vec<Val>)>, Value)> = None;
    let mut rows = Vec::new();
    for &i in &a.entries {
        let edir = a.out.join(format!("e{i}"));
        let (mut root, movie, cps, ret) = boot_entry(&host, &ar, i, &edir.join("boot"))?;
        let mut row = json!({"entry": i, "frame": ar.frame[i], "room": ar.room[i], "return": ret});
        if rd.is_none() {
            let (Some(scope), Some(dir)) = (&a.read_scope, &a.read_dir) else {
                return Err("T's reading: --reading or --read-scope with --read-dir".into());
            };
            let t = Instant::now();
            rd = Some(read_fresh(&readings, &mut root, scope, dir, &ar.goal)?);
            row["read_s"] = json!(pyjson::round(t.elapsed().as_secs_f64(), 1));
        }
        let rd_ = rd.clone().unwrap();
        if t_atoms.is_none() {
            let t = Instant::now();
            let (atoms, mut counts) = atoms_of(&*rd_)?;
            counts["seconds"] = json!(pyjson::round(t.elapsed().as_secs_f64(), 2));
            counts["reading"] = json!(rd_.dir().join("reading.json").to_string_lossy());
            t_atoms = Some((atoms, counts));
        }
        let atoms = &t_atoms.as_ref().unwrap().0;
        if a.due_check {
            row["due_check"] = json!(due_check(&mut root)?);
        }
        // the wait with the checks on (the differential view, the idle rule), then `repeat` times as the try runs it
        let cfg = WaitCfg { idle_frames: if a.idle_check { 1600 } else { 0 }, view_check: a.view_check, ..Default::default() };
        let w = wait::wait(&mut root, atoms, &*rd_, &cfg)?;
        let mut rec = w.json(atoms);
        rec["view_misses_first"] = json!(w.view_misses.iter().take(8).map(|(f, k, x, y)| json!([f, k, start::val_json(*x), start::val_json(*y)])).collect::<Vec<_>>());
        let o0 = root.obs(&ObsReq::default()).map_err(eng)?;
        rec["dues0_units"] = json!(w.dues0.iter().map(|(sl, f)| {
            let s = o0.slots.iter().find(|s| s.slot == *sl).unwrap();
            json!([start::unit_name(s.where_, s.number), s.delay, f - w.frame0])
        }).collect::<Vec<_>>());
        if a.idle_check {
            let want = idle_whole(&mut root, 1600)?;
            let got = w.idle.clone().unwrap_or(Value::Null);
            let same = ["returns", "first_return", "changes"].iter().all(|k| want[*k] == got[*k]);
            rec["idle_same"] = json!(same);
            rec["idle_returns"] = json!(got["returns"].as_array().map(|x| x.len()));
            if !same {
                rec["idle_want"] = want;
                rec["idle_got"] = got;
            }
        }
        let mut secs = vec![];
        for _ in 0..a.repeat {
            let w2 = wait::wait(&mut root, atoms, &*rd_, &WaitCfg::default())?;
            let same = w2.stops.iter().map(|s| (s.frame, s.atoms.clone())).eq(w.stops.iter().map(|s| (s.frame, s.atoms.clone())))
                && w2.end == w.end && w2.frames == w.frames;
            if !same {
                return Err(format!("entry {i}: a repeated wait differs from the first ({} stops, {}, against {} stops, {})",
                                   w2.stops.len(), w2.end.text(), w.stops.len(), w.end.text()));
            }
            secs.push(w2.seconds);
        }
        // the floor under the wait's frames: the same frames from a fork with nothing done between them
        let mut floor = vec![];
        for _ in 0..a.repeat {
            let t = Instant::now();
            let mut b = root.fork().map_err(eng)?;
            let tf = Instant::now();
            b.wait(w.frames, &mut |_, _| false).map_err(eng)?;
            let frames_s = tf.elapsed().as_secs_f64();
            b.quit();
            floor.push((t.elapsed().as_secs_f64(), frames_s));
        }
        if !floor.is_empty() {
            let all: Vec<f64> = floor.iter().map(|x| x.0).collect();
            let fr: Vec<f64> = floor.iter().map(|x| x.1).collect();
            rec["floor"] = json!({"n": floor.len(), "median_s": pyjson::round(median(&all), 3), "frames_median_s": pyjson::round(median(&fr), 3),
                                  "us_per_frame": pyjson::round(median(&fr) / w.frames.max(1) as f64 * 1e6, 1)});
        }
        if !secs.is_empty() {
            rec["repeat"] = json!({"n": secs.len(), "all_s": secs.iter().map(|x| pyjson::round(*x, 3)).collect::<Vec<_>>(),
                "median_s": pyjson::round(median(&secs), 3),
                "max_s": pyjson::round(secs.iter().cloned().fold(0.0, f64::max), 3), "min_s": pyjson::round(secs.iter().cloned().fold(f64::MAX, f64::min), 3)});
        }
        if a.verify {
            let mut vs = Vec::new();
            for (n, s) in w.stops.iter().enumerate() {
                let cp = pyjson::loads(&crate::archive::checkpoint_text(&s.checkpoint)).map_err(|e| e.to_string())?;
                let mut all = cps.clone();
                all.push(cp);
                let t = Instant::now();
                let d = verify_from_boot(&host, &movie, &all, &edir.join(format!("verify-{n}")))?;
                vs.push(json!({"stop": n, "frame": s.frame, "checkpoints": all.len(), "same": d.is_empty(),
                               "differ": d.len(), "first": d.first(), "seconds": pyjson::round(t.elapsed().as_secs_f64(), 2)}));
            }
            rec["verify"] = json!(vs);
        }
        row["wait"] = rec;
        root.quit();
        let _ = std::fs::remove_dir_all(&edir);
        rows.push(row);
    }
    let (_, counts) = t_atoms.unwrap_or_default();
    let out = json!({"tested": counts, "entries": rows});
    files::write_json(&a.out.join("wait-check.json"), &out, Some(1))?;
    Ok(out)
}
