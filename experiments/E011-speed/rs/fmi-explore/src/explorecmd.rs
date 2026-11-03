//! explore.py's commands that need the engine, on fmi-core's `EngineHost` (and the explore component's `Returns`):
//! `init` (the root booted as the harness boots it, its checkpoint as entry 0), `verify` (the goal entry's movie from
//! boot in a fresh engine against every checkpoint of its chain, then the run clock), and the scopes of `--allowed kN`
//! (size_scaling.scopes, from the game files).

use crate::archive::{self, Archive, SEED};
use crate::driver::Verifier;
use crate::events;
use crate::files::{self, Res};
use crate::harness;
use crate::ports::{EntryRef, Returns};
use crate::pyjson;
use crate::step;
use crate::tryjob::compare_checkpoint;
use fmi_core::{facts_of, BootCfg, EngineHost, ObsReq, RunSpec};
use scumm_ir::{Expr, Game, Stmt, UnitKey};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// releases.get("dos-cd").timer_hz: quarter-frames per second (PIT_BASE_HZ / 4096 * 3433 / 4167, in that order).
pub fn timer_hz() -> f64 {
    1193182.0 / 4096.0 * 3433.0 / 4167.0
}

/// size_scaling.room_graph's links: {room: rooms its code loads by a constant}.
pub fn room_links(g: &Game) -> BTreeMap<i32, BTreeSet<i32>> {
    let mut links: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();
    for u in &g.script.units {
        let room = match u.key {
            UnitKey::Entry(r) | UnitKey::Exit(r) | UnitKey::Local(r, _) => Some(r),
            UnitKey::Obj(o) => g.object_room.get(&o).copied(),
            UnitKey::Global(_) => continue,
        };
        let Some(room) = room else { continue };
        for x in &g.script.ins[u.range()] {
            let Some(Stmt::Op { name, args, .. }) = x.st else { continue };
            let nm = g.script.syms.str(name);
            if nm != "loadRoom" && nm != "loadRoomWithEgo" {
                continue;
            }
            let a = g.script.ast.kids(args);
            let arg = if nm == "loadRoom" { a.first() } else { a.get(1) };
            let Some(Expr::Const(mut t)) = arg.map(|&e| g.script.ast.expr(e)) else { continue };
            if t == 0 {
                continue;
            }
            if let Some(&p) = g.pseudo.get(&t) {
                t = p;
            }
            if t == room {
                continue;
            }
            links.entry(room).or_default().insert(t);
        }
    }
    links
}

/// size_scaling.scopes: {k: sorted rooms within k links of the start room}.
pub fn scopes(g: &Game, start_room: i32) -> BTreeMap<usize, Vec<i64>> {
    let links = room_links(g);
    let mut dist: BTreeMap<i32, usize> = BTreeMap::from([(start_room, 0)]);
    let mut ring = vec![start_room];
    while !ring.is_empty() {
        let mut nxt = Vec::new();
        for r in &ring {
            let dr = dist[r];
            for &t in links.get(r).map(|s| s.iter().collect::<Vec<_>>()).unwrap_or_default() {
                if let std::collections::btree_map::Entry::Vacant(e) = dist.entry(t) {
                    e.insert(dr + 1);
                    nxt.push(t);
                }
            }
        }
        ring = nxt;
    }
    let maxd = dist.values().copied().max().unwrap_or(0);
    (0..=maxd).map(|k| (k, dist.iter().filter(|(_, d)| **d <= k).map(|(r, _)| *r as i64).collect())).collect()
}

/// explore.py `parse_allowed`: kN, files (the guide's rooms.reached) or R,R,...
pub fn parse_allowed(text: &str, guide: &Value, start_room: i64, game: Option<&Game>) -> Res<Vec<i64>> {
    if let Some(n) = text.strip_prefix('k').and_then(|n| n.parse::<usize>().ok()) {
        let g = game.ok_or("--allowed kN needs the game files")?;
        let sc = scopes(g, start_room as i32);
        return sc.get(&n).cloned().ok_or(format!("--allowed {text}: the scopes from room {start_room} end at k = {}",
                                                 sc.keys().max().copied().unwrap_or(0)));
    }
    if text == "files" {
        let mut v: Vec<i64> = guide["rooms"]["reached"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_i64()).collect();
        v.sort();
        return Ok(v);
    }
    let s: BTreeSet<i64> = text.split(',').filter(|x| !x.trim().is_empty()).filter_map(|x| x.trim().parse().ok()).collect();
    Ok(s.into_iter().collect())
}

/// explore.py `boot_root`: the root's checkpoint and stage.json's root fields.
pub fn boot_root(r: &mut dyn Returns, out_dir: &Path) -> Res<(Value, Value)> {
    let t = Instant::now();
    let mut b = r.return_to(&EntryRef { id: 0, root: true, frame: 0, events: vec![] }, out_dir).map_err(|e| format!("the boot: {e}"))?;
    let s = b.obs(&ObsReq::default()).map_err(|e| format!("the boot: {e}"))?;
    let st = b.status().clone();
    let facts = step::facts_of_ordered(&s);
    let checks: Vec<String> = {
        let mut c = Vec::new();
        // a pool's return is a fork of the kept boot, whose stop reads "forked" with the idle stop's other fields
        if st.reason != "idle" && !(st.reason == "forked" && st.idle.as_deref() == Some("sentence")) {
            c.push(format!("stop {}, not idle", files::py_repr(&json!(st.reason))));
        }
        if st.idle.as_deref() != Some("sentence") {
            c.push(format!("idle kind {}, not sentence", files::py_repr(&json!(st.idle))));
        }
        if s.seed != Some(SEED) {
            c.push(format!("dump seed {}", s.seed.map(|x| x.to_string()).unwrap_or("None".into())));
        }
        if s.boot_param != 0 {
            c.push(format!("dump boot_param {}", s.boot_param));
        }
        c
    };
    if !checks.is_empty() {
        b.quit();
        return Err(format!("the boot fails start.py's checks: {}", files::py_repr(&json!(checks))));
    }
    let cp = json!([st.frame, facts, harness::extra_of(&s, st.tick, st.rng)]);
    let ego = st.ego.map(|(x, y)| json!([x, y])).unwrap_or(Value::Null);
    let info = json!({"frame": st.frame, "tick": st.tick, "qf": st.qf, "room": facts.get("room"),
                      "boot_s": pyjson::round(t.elapsed().as_secs_f64(), 1),
                      "stop": {"stop": st.reason, "frame": st.frame, "tick": st.tick, "room": st.room, "idle": st.idle,
                               "rng": st.rng, "up": st.up, "cs": st.cs, "ego": ego}});
    r.keep(0, b);
    Ok((cp, info))
}

/// The switches of `init` (stage.json "e009" and "e010").
#[derive(Clone, Debug, Default)]
pub struct InitArgs {
    pub archive: String,
    pub stage: String,
    pub goal: String,
    pub guide: String,
    pub allowed: String,
    pub workers: i64,
    pub idle_frames: i64,
    pub rotate: bool,
    pub evict_by_size: bool,
    pub reuse_nonfacts: bool,
    pub stop_jobs: i64,
    pub ready: bool,
    pub commit_as_finished: bool,
}

/// explore.py `cmd_init` (with E010's `--ready` and `--commit-as-finished`).
pub fn init(a: &InitArgs, r: &mut dyn Returns, game: Option<&Game>) -> Res<Archive> {
    let root = Path::new(&a.archive);
    if root.join("stage.json").exists() {
        return Err(format!("{}: an archive is already there", root.display()));
    }
    let guide = archive::load_guide(Path::new(&a.guide))?;
    let goal = if a.goal == "part1" {
        let e003 = crate::guide::default_root().join("experiments").join("E003-path-through-cloud");
        let doc = files::read_json(&e003.join("results").join("end-part1.json"))?;
        if files::truthy(doc.get("goal_not")) {
            return Err("end-part1.json has goal_not values; a stage goal holds values to reach only".into());
        }
        let mut m = Map::new();
        let mut kv: Vec<(String, Value)> = doc["goal_all"].as_object().cloned().unwrap_or_default().into_iter().collect();
        kv.sort_by(|x, y| x.0.cmp(&y.0));
        for (k, v) in kv {
            m.insert(k, json!([files::as_i64(Some(&v))]));
        }
        Value::Object(m)
    } else {
        archive::parse_goal(&a.goal)?
    };
    std::fs::create_dir_all(root).map_err(|e| files::io_err(root, e))?;
    files::say(&format!("{}: booting the root", root.display()));
    let (cp, info) = boot_root(r, &root.join("root"))?;
    let allowed = parse_allowed(&a.allowed, &guide, files::as_i64(info.get("room")).unwrap_or(0), game)?;
    let mut e009 = Map::new();
    for (k, v) in [("idle_frames", json!(a.idle_frames)), ("rotate", json!(a.rotate)), ("evict_by_size", json!(a.evict_by_size)),
                   ("reuse_nonfacts", json!(a.reuse_nonfacts)), ("stop_jobs", json!(a.stop_jobs))] {
        if files::truthy(Some(&v)) {
            e009.insert(k.into(), v);
        }
    }
    let mut e010 = Map::new();
    for (k, v) in [("ready", a.ready), ("commit_finished", a.commit_as_finished)] {
        if v {
            e010.insert(k.into(), json!(true));
        }
    }
    let mut ar = Archive::create(root, &a.stage, &goal, Path::new(&a.guide), &allowed, a.workers, Some(info.clone()),
                                 Some(Value::Object(e009.clone())), Some(Value::Object(e010.clone())))?;
    ar.add(None, files::as_i64(cp.get(0)).unwrap_or(0), files::as_i64(info.get("tick")), &[], std::slice::from_ref(&cp), None, "boot")?;
    ar.write_cells()?;
    files::say(&format!("{}: root at frame {} (room {}, qf {}); allowed rooms {:?}{}{}", root.display(), cp[0], info["room"], info["qf"],
                        allowed, if e009.is_empty() { String::new() } else { format!("; E009 {}", files::py_repr(&Value::Object(e009))) },
                        if e010.is_empty() { String::new() } else { format!("; E010 {}", files::py_repr(&Value::Object(e010))) }));
    Ok(ar)
}

/// explore.py `Archive.verify` on an engine host: the movie from boot against every checkpoint of the chain, then
/// the run clock at the entry's frame. Writes ARCHIVE/verify-ID.json.
pub struct HostVerifier {
    pub host: Arc<dyn EngineHost>,
    pub lean: bool,
}

impl HostVerifier {
    /// replay.verify: the movie's events from boot in a fresh engine; each checkpoint's frame, then the comparison.
    pub fn replay_verify(&self, movie: &[Value], cps: &[Value], goal_keys: &[String], out_dir: &Path) -> Res<Value> {
        let evs: Vec<fmi_core::Event> = movie.iter().filter_map(events::event_of).collect();
        let mut out = json!({"checkpoints": cps.len(), "differ": [], "inputs": movie.iter().filter(|e| events::is_input(e)).count()});
        let first = cps.first().and_then(|c| c[0].as_u64());
        // every host's boot replays only the events up to its frame; the rest are scheduled at that stop, and the
        // first checkpoint is compared where the boot stopped (the bridge refuses a run to the current frame)
        let mut b = self.host.boot(&BootCfg { seed: 1, events: evs.clone(), frame: first, dump_locals: false, lean: self.lean, out_dir: out_dir.to_path_buf() })
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
            let run = if b.status().frame < frame { b.run(&RunSpec { frame: Some(frame), ..Default::default() }) } else { Ok(b.status().clone()) };
            let st = match run {
                Ok(s) => s,
                Err(e) => {
                    differ.push(json!({"frame": frame, "error": format!("stopped at {e}")}));
                    break;
                }
            };
            if st.frame != frame {
                differ.push(json!({"frame": frame, "error": format!("stopped at {}", st.frame)}));
                break;
            }
            let s = b.obs(&ObsReq::default()).map_err(|e| format!("verify: {e}"))?;
            let d = compare_checkpoint(cp, &s, &st);
            if !d.is_empty() {
                let mut m = Map::new();
                m.insert("frame".into(), json!(frame));
                for (k, v) in d {
                    m.insert(k, v);
                }
                differ.push(Value::Object(m));
            }
        }
        if !cps.is_empty() {
            let s = b.obs(&ObsReq::default()).map_err(|e| format!("verify: {e}"))?;
            let f = facts_of(&s);
            let mut fin = Map::new();
            for k in goal_keys {
                fin.insert(k.clone(), json!(f.get(fmi_core::Key::parse(k)).unwrap_or(0)));
            }
            out["final"] = Value::Object(fin);
        }
        b.quit();
        out["same"] = json!(differ.is_empty());
        out["differ"] = json!(differ);
        Ok(out)
    }
}

impl Verifier for HostVerifier {
    fn verify(&self, a: &Archive, i: usize) -> Res<Value> {
        let out_dir = a.root.join(format!("verify-{i}"));
        let movie = a.movie(i)?;
        let cps = a.checkpoints(i)?;
        let mut goal_keys: Vec<String> = a.goal.iter().map(|(k, _)| k.clone()).collect();
        goal_keys.sort();
        let t = Instant::now();
        let mut ver = self.replay_verify(&movie, &cps, &goal_keys, &out_dir)?;
        ver["seconds"] = json!(pyjson::round(t.elapsed().as_secs_f64(), 1));
        let frame = a.rows[i].frame as u64;
        let evs: Vec<fmi_core::Event> = movie.iter().filter_map(events::event_of).filter(|e| e.frame <= frame).collect();
        let b = self.host.boot(&BootCfg { seed: 1, events: evs, frame: Some(frame), dump_locals: false, lean: self.lean,
                                          out_dir: out_dir.join("clock") }).map_err(|e| format!("verify clock: {e}"))?;
        let (qf, tick) = (b.status().qf, b.status().tick);
        b.quit();
        let root = a.stage.get("root").cloned().unwrap_or(json!({}));
        let dq = files::as_i64(root.get("qf")).map(|r| qf as i64 - r);
        let chain = a.chain(i);
        let fin = ver.get("final").cloned().unwrap_or(json!({}));
        let goal = a.goal.iter().all(|(k, vs)| fin.get(k).and_then(|v| v.as_i64()).map(|v| vs.contains(&v)).unwrap_or(false));
        let out = json!({"entry": i, "chain": chain, "segments": chain.len() - 1, "frame": frame,
                         "frames": frame as i64 - a.rows[0].frame, "qf": dq, "qf_end": qf, "qf_root": root.get("qf"),
                         "seconds": dq.map(|d| d as f64 / timer_hz()), "tick_end": tick, "inputs": ver.get("inputs"),
                         "goal": goal, "same": ver["same"], "verify": ver});
        files::write_json(&a.root.join(format!("verify-{i}.json")), &out, Some(1))?;
        Ok(out)
    }
}

/// E013 `fmi explore stalls ARCHIVE [--json OUT]` (plan.md C, stall.rs): the stall table over the archive's files at
/// any time, read only; `--json` writes it as the driver writes stalls.json.
pub fn stalls(root: &Path, json_out: Option<&Path>) -> Res<()> {
    let s = crate::stall::Stalls::of_archive(root)?;
    print!("{}", crate::stall::report_text(&s));
    if let Some(p) = json_out {
        files::write_json(p, &s.json(), Some(1))?;
    }
    Ok(())
}
