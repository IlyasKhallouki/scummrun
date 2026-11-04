//! E015's G2 (experiments/E015-engine-gates/plan.md, "G2: novelty over engine-visible state"): the measurement tool.
//! Outside the system, like `probe` and `afford-check`; the archive is read only. Numbers only, no game text.
//!
//!   e015-g2 rerun ARCHIVE JOB OUT
//!       re-runs the job's free step as the try ran it (fmi_explore::free::explore from its entry, with its job.json,
//!       its T = result.json's "tested" merged with job.json's archive T, the try's wait first, its delays and the one
//!       more level), with FMI_NOVEL_ENGINE=1's per-stop observer: OUT/jobs/JOB/trials.jsonl (one line per trial: its
//!       name, frames, room, the run's T verdict, the stop's live units, wait offsets, reachable targets, xstart and
//!       cell over T) and OUT/jobs/JOB/summary.json (the counts against the job's record, the ambient units).
//!       The reading is not on this machine: the wait reads T's domains from T's own atoms (`StandIn`).
//!   e015-g2 seen ARCHIVE OUT --ids A-B
//!       the seen set's engine part: each entry booted from its movie, one offer at its frame: OUT/seen/ID.json
//!       (xstart, reachable targets; live units and offsets from its stored checkpoint).
//!   e015-g2 score ARCHIVE OUT JOB...
//!       the sequential keeping under E, under each kind alone, under T∪U∪W and under the sensitivity rule;
//!       OUT/score.json and OUT/steps.tsv.
//!   e015-g2 depth ARCHIVE OUT --entries I,J,.. --tjob JOB [--max-depth 5] [--max-trials 4000] [--max-s 120]
//!       D1: IW(1) breadth first under E over afford's inputs from each entry: OUT/depth/ENTRY.json
//!   e015-g2 route ARCHIVE OUT --events FILE --entry I [--last N] --tjob JOB
//!       D2: the route's inputs from the entry, each stop marked kept or not under E, T∪U∪W and T.

use fmi_core::*;
use fmi_engine::Host;
use fmi_explore::afford::{self, AffordCfg, Novelty, Trial};
use fmi_explore::enovel::{self, View};
use fmi_explore::files::{self, read_gz, read_json};
use fmi_explore::mech::{RealWait, TestedT, TryNovel, WaitPort};
use fmi_explore::progress::{self, TAtom};
use fmi_explore::{events, free, mech};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

type Res<T> = Result<T, String>;

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn opt<'a>(argv: &'a [String], k: &str) -> Option<&'a str> {
    argv.iter().position(|a| a == k).and_then(|i| argv.get(i + 1)).map(|s| s.as_str())
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::env::set_var(enovel::SWITCH, "1");
    let r = match argv.first().map(|s| s.as_str()) {
        Some("rerun") => rerun_cmd(&argv[1..]),
        Some("seen") => seen_cmd(&argv[1..]),
        Some("score") => score_cmd(&argv[1..]),
        Some("depth") => depth_cmd(&argv[1..]),
        Some("route") => route_cmd(&argv[1..]),
        _ => Err("usage: e015-g2 rerun|seen|score|depth|route ...".into()),
    };
    if let Err(e) = r {
        eprintln!("e015-g2: {e}");
        std::process::exit(1);
    }
}

// --- the archive ---------------------------------------------------------------------------------------------------

fn entry(arch: &Path, i: u64) -> Res<Value> {
    read_gz(&arch.join("entries").join(format!("{i}.json.gz")))
}

/// The entry's movie from boot (its chain's segments) and its frame.
fn movie(arch: &Path, idx: u64) -> Res<(Vec<Value>, u64)> {
    let e = entry(arch, idx)?;
    let frame = e["frame"].as_u64().ok_or("entry without a frame")?;
    let mut chain = vec![e];
    while let Some(p) = chain.last().unwrap().get("parent").and_then(|p| p.as_u64()) {
        chain.push(entry(arch, p)?);
    }
    chain.reverse();
    let evs: Vec<Value> = chain
        .iter()
        .flat_map(|c| c["events"].as_array().cloned().unwrap_or_default())
        .filter(|v| v["frame"].as_u64().is_some_and(|f| f <= frame))
        .collect();
    Ok((evs, frame))
}

/// The entry's last checkpoint ([frame, facts, extra]).
fn last_checkpoint(arch: &Path, idx: u64) -> Res<Value> {
    let e = entry(arch, idx)?;
    e["checkpoints"].as_array().and_then(|c| c.last()).cloned().ok_or_else(|| format!("entry {idx}: no checkpoint"))
}

fn facts_of_cp(cp: &Value) -> Res<Facts> {
    let mut pairs = Vec::new();
    for (k, v) in cp[1].as_object().ok_or("checkpoint facts")? {
        if let Some(n) = v.as_i64() {
            pairs.push((Key::parse(k), n));
        }
    }
    Ok(Facts::from_pairs(pairs))
}

fn boot(h: &Host, out: &Path, evs: &[Value], frame: u64) -> Res<Box<dyn Engine>> {
    let events: Vec<Event> = evs.iter().filter_map(events::event_of).collect();
    if events.len() != evs.len() {
        return Err("an event the bridge cannot read".into());
    }
    let cfg = BootCfg { seed: 1, events, frame: Some(frame), dump_locals: false, lean: true, out_dir: out.join("boot") };
    Ok(Box::new(h.boot_bin(&cfg).map_err(e2s)?))
}

/// The log's commit order: each job's line index, and the entries each line committed.
fn commits(arch: &Path) -> Res<(HashMap<String, usize>, Vec<Vec<u64>>)> {
    let text = std::fs::read_to_string(arch.join("log.jsonl")).map_err(e2s)?;
    let (mut at, mut news) = (HashMap::new(), Vec::new());
    for (i, l) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let v: Value = serde_json::from_str(l).map_err(e2s)?;
        at.insert(v["job"].as_str().unwrap_or("").to_string(), i);
        news.push(v["new_entries"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default());
    }
    Ok((at, news))
}

/// The entries committed before the job's own line, the root first.
fn seen_ids(arch: &Path, job: &str) -> Res<Vec<u64>> {
    let (at, news) = commits(arch)?;
    let n = *at.get(job).ok_or_else(|| format!("{job}: not in log.jsonl"))?;
    let mut ids = vec![0u64];
    for l in &news[..n] {
        ids.extend(l.iter().copied());
    }
    Ok(ids)
}

// --- T without the reading -----------------------------------------------------------------------------------------

/// T's flat (key, set) from its atoms: the ints, `below` where the atom has the domain's low bound, `above` where it
/// has the high one.
fn flat_of(atoms: &[TAtom]) -> Vec<(Key, Vec<Val>)> {
    atoms
        .iter()
        .map(|a| {
            let mut vs: Vec<Val> = a.vals.clone();
            if a.lo.is_some() {
                vs.push(BELOW);
            }
            if a.hi.is_some() {
                vs.push(ABOVE);
            }
            (Key::parse(&a.key), vs)
        })
        .collect()
}

/// T's keys' domains as T's atoms show them: each key's ints, and the bounds its atoms carry.
fn doms_of(atoms: &[TAtom]) -> Domains {
    let mut m: BTreeMap<Key, Domain> = BTreeMap::new();
    for a in atoms {
        let d = m.entry(Key::parse(&a.key)).or_insert_with(|| Domain { values: vec![], low: None, high: None });
        d.values.extend(a.vals.iter().copied());
        if a.lo.is_some() {
            d.low = a.lo;
        }
        if a.hi.is_some() {
            d.high = a.hi;
        }
    }
    Domains(
        m.into_iter()
            .map(|(k, mut d)| {
                d.values.sort_unstable();
                d.values.dedup();
                (k, d)
            })
            .collect(),
    )
}

/// The reading the wait is given: T's domains from T's atoms, no live locals. Nothing else of it is read.
struct StandIn {
    doms: Domains,
    start: Facts,
}

impl ReadingSource for StandIn {
    fn id(&self) -> [u8; 20] {
        [0; 20]
    }
    fn dir(&self) -> &Path {
        Path::new("/nonexistent")
    }
    fn head(&self) -> &str {
        "{}"
    }
    fn packed(&self) -> &[u8] {
        &[]
    }
    fn n_actions(&self) -> u32 {
        0
    }
    fn action_line(&self, _i: u32) -> &str {
        ""
    }
    fn by_name(&self, _name: &str) -> Option<u32> {
        None
    }
    fn n_lists(&self) -> u32 {
        0
    }
    fn list_line(&self, _i: u32) -> &str {
        ""
    }
    fn reached(&self) -> &str {
        ""
    }
    fn domains(&self) -> &Domains {
        &self.doms
    }
    fn goal(&self) -> &[(Key, Vec<Val>)] {
        &[]
    }
    fn start(&self) -> &Facts {
        &self.start
    }
    fn action(&self, _i: u32) -> Action {
        unimplemented!("the stand-in reading has no actions")
    }
    fn record(&self, _i: u32) -> Record {
        unimplemented!("the stand-in reading has no records")
    }
}

/// A state's cell over T: its room and the atoms of T that hold in it (a hash, as text).
fn cell_of(atoms: &[TAtom], f: &Facts) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut eat = |x: u64| {
        h ^= x;
        h = h.wrapping_mul(0x100000001b3);
    };
    eat(f.get(Key::pack(K_ROOM, 0, 0, 0)).unwrap_or(0) as u64);
    for (i, a) in atoms.iter().enumerate() {
        if a.holds_in(f) {
            eat(i as u64 + 1);
        }
    }
    format!("{h:016x}")
}

fn view_json(v: &View) -> Value {
    json!({"frame": v.frame, "room": v.room, "xstart": v.xstart, "live": v.live,
           "at": v.at.iter().map(|(u, o)| json!([u, o])).collect::<Vec<_>>(), "reach": v.reach})
}

fn view_of_json(v: &Value) -> View {
    let s = |x: &Value| x.as_array().map(|a| a.iter().filter_map(|y| y.as_str().map(String::from)).collect()).unwrap_or_default();
    View {
        frame: v["frame"].as_u64().unwrap_or(0),
        room: v["room"].as_i64().unwrap_or(0) as i32,
        xstart: v["xstart"].as_i64().unwrap_or(0) as i32,
        live: s(&v["live"]),
        at: v["at"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| Some((p[0].as_str()?.to_string(), p[1].as_u64()?))).collect())
            .unwrap_or_default(),
        reach: s(&v["reach"]),
    }
}

/// The job's T as the free step used it: result.json's "tested" (the reading's T with E014's extra atoms) and that
/// merged with job.json's archive T.
fn job_t(job: &Value, res: &Value) -> Res<(Vec<TAtom>, Vec<TAtom>, Vec<TAtom>)> {
    let atoms = progress::atoms_of_json(res.get("tested"));
    if atoms.is_empty() {
        return Err("result.json has no tested atoms".into());
    }
    let arch = progress::atoms_of_json(job.get("novelty").and_then(|n| n.get("t")));
    let have: HashSet<TAtom> = atoms.iter().cloned().collect();
    let mut merged = atoms.clone();
    merged.extend(arch.iter().filter(|a| !have.contains(*a)).cloned());
    Ok((atoms, arch, merged))
}

// --- rerun ---------------------------------------------------------------------------------------------------------

fn rerun_cmd(a: &[String]) -> Res<()> {
    if a.len() < 3 {
        return Err("rerun ARCHIVE JOB OUT".into());
    }
    let (arch, job_name, out) = (PathBuf::from(&a[0]), a[1].clone(), PathBuf::from(&a[2]));
    let t0 = Instant::now();
    let jd = arch.join("jobs").join(&job_name);
    let job = read_json(&jd.join("job.json"))?;
    let res = read_json(&jd.join("result.json"))?;
    let od = out.join("jobs").join(&job_name);
    std::fs::create_dir_all(&od).map_err(e2s)?;
    let h = Host::from_env().map_err(e2s)?;
    let ent = &job["entry"];
    let evs = ent["events"].as_array().cloned().unwrap_or_default();
    let frame = ent["frame"].as_u64().ok_or("entry frame")?;
    let mut root = boot(&h, &od, &evs, frame)?;
    let o = root.obs(&ObsReq::default()).map_err(e2s)?;
    let differ = fmi_explore::tryjob::compare_checkpoint(&ent["checkpoint"], &o, root.status());
    if !differ.is_empty() || root.status().frame != frame {
        root.quit();
        return Err(format!("{job_name}: the return differs from the entry's checkpoint: {}", Value::Object(differ)));
    }
    let entry_facts = facts_of(&o);
    let (atoms, arch_t, merged) = job_t(&job, &res)?;
    let t = TestedT::new(flat_of(&atoms), Value::Null);
    let keys = free::merged_keys(&t, &arch_t);
    let nov = TryNovel::new(&job, &merged, &entry_facts);
    // the reading's domains are not here: T's own keys (the reading's atoms, before E014's extra ones), the verbs and
    // the goal's key are given domains from T's atoms; E014's other extra atoms have none, as in the run's waits
    // (result.json's wait "observed" counts; the closest rule found, not exact)
    let n_own = res["e012"]["t"]["atoms"].as_u64().unwrap_or(0).saturating_sub(res["e012"]["t"]["e014_extra"].as_u64().unwrap_or(0)) as usize;
    let own: HashSet<String> = atoms.iter().take(n_own).map(|a| a.key.clone()).collect();
    let dom_atoms: Vec<TAtom> =
        atoms.iter().filter(|a| own.contains(&a.key) || a.key.starts_with("verb:") || a.key == "end").cloned().collect();
    let rd = StandIn { doms: doms_of(&dom_atoms), start: entry_facts.clone() };
    // the try's wait, as the try runs it before the free step (its novel stops kept: novel no more in the try)
    let idle_frames = if job["idle_frames"].as_u64().unwrap_or(0) != 0 && !files::truthy(job.get("idle_measured")) {
        job["idle_frames"].as_u64().unwrap()
    } else {
        0
    };
    let w = RealWait.wait(&mut root, &t.flat, &rd, idle_frames)?;
    // the offer points after the entry: the recorded wait's stops (frames of the entry's no-input run), exact
    let rec_wait0 = res.get("e012").and_then(|e| e.get("wait")).cloned().unwrap_or(Value::Null);
    let rec_frames: Vec<u64> = rec_wait0["stops"].as_array().map(|a| a.iter().filter_map(|s| s["frame"].as_u64()).collect()).unwrap_or_default();
    let mut stops: Vec<mech::Stop> = Vec::new();
    {
        let mut c = root.fork().map_err(e2s)?;
        for &fr in &rec_frames {
            let st = c.run(&RunSpec { frame: Some(fr), ..Default::default() }).map_err(e2s)?;
            if st.frame != fr {
                c.quit();
                return Err(format!("{job_name}: the wait's stop at frame {fr} is not reached ({})", st.reason));
            }
            let o = c.obs(&ObsReq::default()).map_err(e2s)?;
            stops.push(mech::Stop { frame: fr, checkpoint: afford::checkpoint_of(&st, &o), room: o.room });
        }
        c.quit();
    }
    let mut wait_kept = 0;
    for s in &stops {
        let n = nov.novel(&s.checkpoint.facts);
        if !n.is_empty() {
            nov.keep(&n);
            wait_kept += 1;
        }
    }
    let rec_wait = res.get("e012").and_then(|e| e.get("wait")).cloned().unwrap_or(Value::Null);
    let rec_stops: Vec<u64> = rec_wait["stops"].as_array().map(|a| a.iter().filter_map(|s| s["frame"].as_u64()).collect()).unwrap_or_default();
    let my_stops: Vec<u64> = w.stops.iter().map(|s| s.frame).collect();
    // the ambient units at the entry
    let (offsets, ambient) = enovel::ambient_units(&mut root, enovel::AMBIENT_FRAMES).map_err(e2s)?;
    let rec_idle_units: BTreeSet<String> = res
        .get("idle")
        .and_then(|i| i.get("returns"))
        .and_then(|r| r.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|k| k.as_str())
                .filter_map(|k| k.strip_prefix("at:"))
                .filter_map(|k| k.rsplit_once(':').map(|x| x.0.to_string()))
                .collect()
        })
        .unwrap_or_default();
    // the observer
    let recs: Rc<RefCell<Vec<Value>>> = Rc::new(RefCell::new(Vec::new()));
    let (recs2, merged2) = (recs.clone(), merged.clone());
    let verbose = std::env::var("E015_VERBOSE").is_ok_and(|v| v == "1");
    let mut tlast = Instant::now();
    enovel::set_observer(Some(Box::new(move |tr: &Trial, b: Option<&mut Box<dyn Engine>>| {
        let n = recs2.borrow().len();
        if verbose {
            eprintln!("trial {n} {} frame {} {:.3}s", tr.inputs.name(), tr.frame, tlast.elapsed().as_secs_f64());
            tlast = Instant::now();
        }
        let mut r = json!({"seq": n, "name": tr.inputs.name(), "events": tr.events.iter().map(events::event_json).collect::<Vec<_>>(),
                           "frame": tr.frame, "reason": tr.reason, "ended": tr.ended, "idle": tr.idle, "room": tr.room,
                           "t_novel": tr.novel, "t_kept": tr.kept, "verdict": tr.verdict, "shared": tr.shared});
        if let Some(b) = b {
            match enovel::view_at(b) {
                Ok((v, f)) => {
                    r["view"] = view_json(&v);
                    r["cell"] = json!(cell_of(&merged2, &f));
                }
                Err(e) => r["view_error"] = json!(e.to_string()),
            }
        }
        recs2.borrow_mut().push(r);
    })));
    let cancel = AtomicBool::new(false);
    let dead_moves: Vec<Value> = job.get("dead").and_then(|d| d.as_array()).cloned().unwrap_or_default();
    let wait_port: Arc<dyn WaitPort> = Arc::new(RealWait);
    let cx = free::Ctx { t: &t, keys: &keys, nov: &nov, wait: wait_port, rd: &rd, dead_moves: &dead_moves, cancel: &cancel };
    let fr = free::explore(&mut root, &stops, &cx);
    enovel::set_observer(None);
    root.quit();
    let f = fr?;
    let recs = recs.borrow();
    let mut lines = String::new();
    for r in recs.iter() {
        lines.push_str(&r.to_string());
        lines.push('\n');
    }
    std::fs::write(od.join("trials.jsonl"), lines).map_err(e2s)?;
    let rf = &res["free"];
    let trials = f.record["trials"].as_u64().unwrap_or(0);
    let kept = f.kept.len() as u64;
    let (trials_rec, kept_rec) = (rf["trials"].as_u64().unwrap_or(0), rf["kept_total"].as_u64().unwrap_or(0));
    let summary = json!({
        "job": job_name, "kind": res["kind"], "entry": ent["id"], "entry_frame": frame, "room": o.room,
        "trials": trials, "trials_recorded": trials_rec, "kept": kept, "kept_recorded": kept_rec,
        "observed": recs.len(), "same": trials == trials_rec && kept == kept_rec,
        "kept_by_level": f.record["kept"], "kept_by_level_recorded": rf["kept"],
        "delays": f.record["delays"], "delays_recorded": rf["delays"],
        "wait": {"standin_stops": my_stops, "stops_recorded": rec_stops, "same": my_stops == rec_stops, "kept": wait_kept, "standin_observed": w.record["observed"], "observed_recorded": rec_wait["observed"],
                 "end": w.end, "end_recorded": rec_wait["end"]},
        "ambient": ambient, "units_offsets": offsets.iter().map(|(u, o)| (u.clone(), json!(o.len()))).collect::<serde_json::Map<_, _>>(),
        "idle_units_recorded": rec_idle_units,
        "merged": progress::atoms_json(&merged), "t_atoms": atoms.len(),
        "seconds": t0.elapsed().as_secs_f64(),
    });
    files::write_json(&od.join("summary.json"), &summary, Some(1))?;
    println!("{}", json!({"job": summary["job"], "trials": trials, "trials_recorded": trials_rec, "kept": kept,
                          "kept_recorded": kept_rec, "same": summary["same"], "wait_same": summary["wait"]["same"],
                          "seconds": summary["seconds"]}));
    Ok(())
}

// --- seen ----------------------------------------------------------------------------------------------------------

fn seen_cmd(a: &[String]) -> Res<()> {
    if a.len() < 2 {
        return Err("seen ARCHIVE OUT --ids A-B".into());
    }
    let (arch, out) = (PathBuf::from(&a[0]), PathBuf::from(&a[1]));
    let ids = opt(a, "--ids").ok_or("--ids A-B")?;
    let (lo, hi) = ids.split_once('-').ok_or("--ids A-B")?;
    let (lo, hi): (u64, u64) = (lo.parse().map_err(e2s)?, hi.parse().map_err(e2s)?);
    let step: u64 = opt(a, "--step").map(|s| s.parse().unwrap_or(1)).unwrap_or(1);
    let sd = out.join("seen");
    std::fs::create_dir_all(&sd).map_err(e2s)?;
    let h = Host::from_env().map_err(e2s)?;
    let mut done = 0;
    let mut i = lo;
    while i <= hi {
        let p = sd.join(format!("{i}.json"));
        if p.exists() || !arch.join("entries").join(format!("{i}.json.gz")).exists() {
            i += step;
            continue;
        }
        let t = Instant::now();
        let (evs, frame) = movie(&arch, i)?;
        let cp = last_checkpoint(&arch, i)?;
        let cf = facts_of_cp(&cp)?;
        let tmp = out.join(format!("seen-boot-{}", std::process::id()));
        let mut e = boot(&h, &tmp, &evs, frame)?;
        let r = (|| -> Res<Value> {
            let o = e.obs(&ObsReq::default()).map_err(e2s)?;
            let st = e.status().clone();
            let differ = cf.diff(&facts_of(&o)).len();
            let of = afford::offer_cfg(&mut e, &o, false, true).map_err(e2s)?;
            let mut reach: Vec<String> = of
                .targets
                .iter()
                .filter_map(|p| match p.hit {
                    afford::Hit::Object(id) => Some(format!("obj:{id}")),
                    afford::Hit::Actor(id) => Some(format!("actor:{id}")),
                    _ => None,
                })
                .collect();
            reach.sort();
            reach.dedup();
            let v = enovel::view_of(&cf, frame, st.xstart, reach);
            Ok(json!({"id": i, "frame": frame, "booted_frame": st.frame, "differ": differ, "idle": st.idle, "view": view_json(&v),
                      "seconds": t.elapsed().as_secs_f64()}))
        })();
        e.quit();
        let v = r?;
        files::write_json(&p, &v, None)?;
        done += 1;
        i += step;
    }
    println!("{}", json!({"seen": done}));
    Ok(())
}

// --- scoring -------------------------------------------------------------------------------------------------------

/// A seen state's atoms by kind (U, W unfiltered, R, C), its cell-atoms under a job's T for the sensitivity rule.
struct SeenEntry {
    view: View,
    facts: Facts,
}

fn load_seen(arch: &Path, out: &Path, id: u64, cache: &mut HashMap<u64, Rc<SeenEntry>>) -> Res<Rc<SeenEntry>> {
    if let Some(s) = cache.get(&id) {
        return Ok(s.clone());
    }
    let v = read_json(&out.join("seen").join(format!("{id}.json")))?;
    let cp = last_checkpoint(arch, id)?;
    let s = Rc::new(SeenEntry { view: view_of_json(&v["view"]), facts: facts_of_cp(&cp)? });
    cache.insert(id, s.clone());
    Ok(s)
}

const KINDS: [&str; 4] = ["U", "W", "R", "C"];

/// One feature set's sequential keeping over a step's trials: a trial is kept when the run kept it under T (with
/// `t_in`) or it holds an atom of the set's kinds that no seen state and no trial kept before it holds.
struct Keeper {
    kinds: Vec<&'static str>,
    t_in: bool,
    held: HashSet<String>,
    kept: usize,
}

impl Keeper {
    fn new(kinds: &[&'static str], t_in: bool, seen: &HashSet<String>) -> Keeper {
        Keeper { kinds: kinds.to_vec(), t_in, held: seen.clone(), kept: 0 }
    }
    /// Whether the trial is kept, and its novel atoms.
    fn offer(&mut self, t_kept: bool, by: &BTreeMap<&'static str, Vec<String>>) -> (bool, Vec<String>) {
        let mine: Vec<&String> = self.kinds.iter().flat_map(|k| by.get(k).into_iter().flatten()).collect();
        let novel: Vec<String> = mine.iter().filter(|a| !self.held.contains(**a)).map(|a| (*a).clone()).collect();
        let k = (self.t_in && t_kept) || !novel.is_empty();
        if k {
            self.kept += 1;
            self.held.extend(mine.into_iter().cloned());
        }
        (k, novel)
    }
}

fn score_cmd(a: &[String]) -> Res<()> {
    if a.len() < 3 {
        return Err("score ARCHIVE OUT JOB...".into());
    }
    let (arch, out) = (PathBuf::from(&a[0]), PathBuf::from(&a[1]));
    let jobs: Vec<String> = a[2..].iter().filter(|x| !x.starts_with("--")).cloned().collect();
    let mut cache: HashMap<u64, Rc<SeenEntry>> = HashMap::new();
    let mut steps = Vec::new();
    let mut tsv = String::from("job\tkind\tentry\troom\ttrials\tt_kept\te_kept\tu\tw\tr\tc\ttuw\tsens\tsame\n");
    let (mut n_all, mut e_all, mut t_all, mut sens_all, mut tuw_all) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut alone_all: BTreeMap<&str, usize> = BTreeMap::new();
    let mut attrib_all: BTreeMap<&str, usize> = BTreeMap::new();
    let mut by_room: BTreeMap<i64, (usize, usize, usize)> = BTreeMap::new();
    let mut excluded = Vec::new();
    let mut t596 = Value::Null;
    for job in &jobs {
        let jd = out.join("jobs").join(job);
        let sum = read_json(&jd.join("summary.json"))?;
        if sum["same"] != json!(true) {
            excluded.push(json!({"job": job, "trials": sum["trials"], "trials_recorded": sum["trials_recorded"],
                                 "kept": sum["kept"], "kept_recorded": sum["kept_recorded"]}));
            continue;
        }
        let merged = progress::atoms_of_json(sum.get("merged"));
        let ambient: BTreeSet<String> = sum["ambient"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        // the seen set: every entry committed before the job
        let ids = seen_ids(&arch, job)?;
        let mut seen: HashSet<String> = HashSet::new();
        let mut seen_s: HashSet<String> = HashSet::new();
        let none = BTreeSet::new();
        for &i in &ids {
            let s = load_seen(&arch, &out, i, &mut cache)?;
            for (_, v) in enovel::atoms(&s.view, &none) {
                seen.extend(v);
            }
            seen_s.extend(enovel::cell_atoms(&s.view, &cell_of(&merged, &s.facts)));
        }
        let mut e = Keeper::new(&KINDS, true, &seen);
        let mut tuw = Keeper::new(&["U", "W"], true, &seen);
        let mut alone: Vec<Keeper> = KINDS.iter().map(|k| Keeper::new(&[*k], false, &seen)).collect();
        let mut sens_seen = seen.clone();
        sens_seen.extend(seen_s);
        let mut sens = Keeper::new(&["U", "S", "R", "C"], true, &sens_seen);
        let (mut n, mut tk) = (0usize, 0usize);
        let mut attrib: BTreeMap<&str, usize> = BTreeMap::new();
        let mut kept_rows = Vec::new();
        let text = std::fs::read_to_string(jd.join("trials.jsonl")).map_err(e2s)?;
        for l in text.lines().filter(|l| !l.is_empty()) {
            let r: Value = serde_json::from_str(l).map_err(e2s)?;
            n += 1;
            let t_kept = r["t_kept"].as_u64().unwrap_or(0) > 0;
            tk += t_kept as usize;
            let mut by: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
            if let Some(v) = r.get("view") {
                let v = view_of_json(v);
                by = enovel::atoms(&v, &ambient);
                by.insert("S", enovel::cell_atoms(&v, r["cell"].as_str().unwrap_or("")));
            }
            let room = r["room"].as_i64().unwrap_or(0);
            let (ek, enov) = e.offer(t_kept, &by);
            tuw.offer(t_kept, &by);
            sens.offer(t_kept, &by);
            for k in alone.iter_mut() {
                k.offer(false, &by);
            }
            let ent = by_room.entry(room).or_default();
            ent.0 += 1;
            ent.1 += ek as usize;
            ent.2 += t_kept as usize;
            if ek {
                let mut kinds: Vec<&str> = Vec::new();
                for k in KINDS {
                    let pre = match k {
                        "U" => "live:",
                        "W" => "at:",
                        "R" => "reach:",
                        _ => "cam:",
                    };
                    if enov.iter().any(|x| x.starts_with(pre)) {
                        kinds.push(k);
                        *attrib.entry(k).or_default() += 1;
                    }
                }
                if t_kept {
                    *attrib.entry("T").or_default() += 1;
                }
                kept_rows.push(json!({"seq": r["seq"], "name": r["name"], "frame": r["frame"], "room": room, "t": t_kept,
                                      "kinds": kinds, "novel": enov, "shared": r["shared"]}));
            }
            if job == "j00077" && r["name"] == json!("t596") && t596.is_null() {
                let live211 = r["view"]["live"].as_array().is_some_and(|a| a.iter().any(|x| x == "local:211"));
                t596 = json!({"seq": r["seq"], "frame": r["frame"], "kept_e": ek, "kept_t": t_kept, "novel": enov,
                              "local_211_live": live211, "at": r["view"]["at"]});
            }
        }
        n_all += n;
        e_all += e.kept;
        t_all += tk;
        sens_all += sens.kept;
        tuw_all += tuw.kept;
        for (k, kp) in KINDS.iter().zip(alone.iter()) {
            *alone_all.entry(k).or_default() += kp.kept;
        }
        for (k, c) in &attrib {
            *attrib_all.entry(k).or_default() += c;
        }
        tsv.push_str(&format!(
            "{job}\t{}\t{}\t{}\t{n}\t{tk}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t1\n",
            sum["kind"].as_str().unwrap_or(""),
            sum["entry"],
            sum["room"],
            e.kept,
            alone[0].kept,
            alone[1].kept,
            alone[2].kept,
            alone[3].kept,
            tuw.kept,
            sens.kept
        ));
        steps.push(json!({"job": job, "entry": sum["entry"], "room": sum["room"], "trials": n, "t_kept": tk, "e_kept": e.kept,
                          "alone": KINDS.iter().zip(alone.iter()).map(|(k, x)| (k.to_string(), json!(x.kept))).collect::<serde_json::Map<_, _>>(),
                          "tuw_kept": tuw.kept, "sens_kept": sens.kept, "attrib": attrib, "seen_entries": ids.len(),
                          "ambient": ambient.len(), "kept_rows": kept_rows}));
    }
    let rate = |k: usize| if n_all > 0 { k as f64 / n_all as f64 } else { 0.0 };
    let score = json!({
        "jobs": jobs.len(), "scored": steps.len(), "excluded": excluded, "trials": n_all,
        "t_kept": t_all, "e_kept": e_all, "e_rate": rate(e_all), "t_rate": rate(t_all),
        "tuw_kept": tuw_all, "tuw_rate": rate(tuw_all), "sens_kept": sens_all, "sens_rate": rate(sens_all),
        "alone": alone_all, "attrib": attrib_all,
        "by_room": by_room.iter().map(|(r, (n, e, t))| (r.to_string(), json!({"trials": n, "e_kept": e, "t_kept": t, "e_rate": *e as f64 / (*n).max(1) as f64}))).collect::<serde_json::Map<_, _>>(),
        "t596": t596, "steps": steps,
    });
    files::write_json(&out.join("score.json"), &score, Some(1))?;
    std::fs::write(out.join("steps.tsv"), tsv).map_err(e2s)?;
    println!("{}", json!({"scored": score["scored"], "excluded": score["excluded"].as_array().map(|x| x.len()), "trials": n_all,
                          "e_kept": e_all, "e_rate": score["e_rate"], "t_kept": t_all, "t596": score["t596"],
                          "alone": score["alone"], "sens_rate": score["sens_rate"], "tuw_rate": score["tuw_rate"]}));
    Ok(())
}

// --- D1 and D2 -----------------------------------------------------------------------------------------------------

/// T's novelty for D1 and D2: an atom of T (or the room) that holds in the state and in no seen state.
struct TSeen {
    atoms: Vec<TAtom>,
    held: Vec<bool>,
    rooms: HashSet<i64>,
}

impl TSeen {
    fn new(atoms: Vec<TAtom>) -> TSeen {
        let n = atoms.len();
        TSeen { atoms, held: vec![false; n], rooms: HashSet::new() }
    }
    fn novel(&self, f: &Facts) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.atoms.len()).filter(|&i| !self.held[i] && self.atoms[i].holds_in(f)).collect();
        let room = f.get(Key::pack(K_ROOM, 0, 0, 0)).unwrap_or(0);
        if !self.rooms.contains(&room) {
            v.push(usize::MAX);
        }
        v
    }
    fn add(&mut self, f: &Facts) {
        for i in 0..self.atoms.len() {
            if !self.held[i] && self.atoms[i].holds_in(f) {
                self.held[i] = true;
            }
        }
        self.rooms.insert(f.get(Key::pack(K_ROOM, 0, 0, 0)).unwrap_or(0));
    }
}

/// A feature set for D1/D2: T's part (with `t`) and the engine kinds.
struct Feat {
    t: bool,
    kinds: Vec<&'static str>,
    ts: TSeen,
    held: HashSet<String>,
}

impl Feat {
    fn new(t: bool, kinds: &[&'static str], atoms: &[TAtom], seen: &[(View, Facts)]) -> Feat {
        let mut f = Feat { t, kinds: kinds.to_vec(), ts: TSeen::new(atoms.to_vec()), held: HashSet::new() };
        let none = BTreeSet::new();
        for (v, fa) in seen {
            f.ts.add(fa);
            let by = enovel::atoms(v, &none);
            for k in &f.kinds {
                f.held.extend(by.get(k).cloned().unwrap_or_default());
            }
        }
        f
    }
    /// (kept, the number of novel atoms) of a stop; a kept stop's atoms are held after.
    fn offer(&mut self, v: &View, fa: &Facts, ambient: &BTreeSet<String>) -> (bool, usize) {
        let by = enovel::atoms(v, ambient);
        let mine: Vec<String> = self.kinds.iter().flat_map(|k| by.get(k).cloned().unwrap_or_default()).collect();
        let nov_e = mine.iter().filter(|a| !self.held.contains(*a)).count();
        let nov_t = if self.t { self.ts.novel(fa).len() } else { 0 };
        let k = nov_e + nov_t > 0;
        if k {
            self.held.extend(mine);
            if self.t {
                self.ts.add(fa);
            }
        }
        (k, nov_e + nov_t)
    }
}

/// The seen states of the archive before an entry (ids below it), with their views (cached by `seen`) and facts.
fn seen_before(arch: &Path, out: &Path, id: u64) -> Res<Vec<(View, Facts)>> {
    let mut cache = HashMap::new();
    let mut v = Vec::new();
    for i in 0..id {
        if !arch.join("entries").join(format!("{i}.json.gz")).exists() {
            continue;
        }
        let s = load_seen(arch, out, i, &mut cache)?;
        v.push((s.view.clone(), s.facts.clone()));
    }
    Ok(v)
}

/// T for D1 and D2: a job's merged T (its summary from `rerun`).
fn tjob_atoms(out: &Path, job: &str) -> Res<Vec<TAtom>> {
    let s = read_json(&out.join("jobs").join(job).join("summary.json"))?;
    Ok(progress::atoms_of_json(s.get("merged")))
}

struct NoNovelty;
impl Novelty for NoNovelty {
    fn novel(&self, _f: &Facts) -> Vec<String> {
        Vec::new()
    }
}

/// A trial's stop as the observer saw it.
#[derive(Clone)]
struct Seen1 {
    events: Vec<Event>,
    frame: u64,
    ended: bool,
    room: i32,
    view: Option<(View, Facts)>,
}

fn depth_cmd(a: &[String]) -> Res<()> {
    if a.len() < 2 {
        return Err("depth ARCHIVE OUT --entries I,J --tjob JOB".into());
    }
    let (arch, out) = (PathBuf::from(&a[0]), PathBuf::from(&a[1]));
    let ids: Vec<u64> = opt(a, "--entries").ok_or("--entries")?.split(',').filter_map(|x| x.parse().ok()).collect();
    let tjob = opt(a, "--tjob").ok_or("--tjob JOB")?;
    let max_depth: usize = opt(a, "--max-depth").map(|x| x.parse().unwrap_or(5)).unwrap_or(5);
    let max_trials: usize = opt(a, "--max-trials").map(|x| x.parse().unwrap_or(4000)).unwrap_or(4000);
    let max_s: f64 = opt(a, "--max-s").map(|x| x.parse().unwrap_or(120.0)).unwrap_or(120.0);
    let target_room: i32 = opt(a, "--room").map(|x| x.parse().unwrap_or(0)).unwrap_or(0);
    let target_unit = opt(a, "--unit").unwrap_or("").to_string();
    let atoms = tjob_atoms(&out, tjob)?;
    let dd = out.join("depth");
    std::fs::create_dir_all(&dd).map_err(e2s)?;
    let h = Host::from_env().map_err(e2s)?;
    for id in ids {
        let t0 = Instant::now();
        let (evs, frame) = movie(&arch, id)?;
        let mut root = boot(&h, &dd.join(format!("b{id}")), &evs, frame)?;
        let (_, ambient) = enovel::ambient_units(&mut root, enovel::AMBIENT_FRAMES).map_err(e2s)?;
        let seen = seen_before(&arch, &out, id)?;
        let mut feat = Feat::new(true, &KINDS, &atoms, &seen);
        // the entry itself is in the archive: seen
        {
            let (v, f) = enovel::view_at(&mut root).map_err(e2s)?;
            feat.offer(&v, &f, &BTreeSet::new());
        }
        // a node: the clicks from the entry and the frame of its stop
        let mut frontier: Vec<(Vec<Event>, u64)> = vec![(vec![], frame)];
        let (mut trials, mut kept_total) = (0usize, 0usize);
        let mut first_unit: Option<usize> = None;
        let mut first_room: Option<usize> = None;
        let mut per_depth = Vec::new();
        let mut why_stop = "depth";
        'outer: for depth in 1..=max_depth {
            let mut next: Vec<(Vec<Event>, u64)> = Vec::new();
            let (mut d_trials, mut d_kept) = (0usize, 0usize);
            for (node, node_frame) in &frontier {
                if trials >= max_trials {
                    why_stop = "trials";
                    break 'outer;
                }
                if t0.elapsed().as_secs_f64() >= max_s {
                    why_stop = "seconds";
                    break 'outer;
                }
                // the engine at the node: the root, the node's clicks, run to its stop
                let mut b = root.fork().map_err(e2s)?;
                if !node.is_empty() {
                    b.inputs(node).map_err(e2s)?;
                    match b.run(&RunSpec { frame: Some(*node_frame), ..Default::default() }) {
                        Ok(st) if st.frame == *node_frame => {}
                        Ok(_) => {
                            b.quit();
                            continue;
                        }
                        Err(_) => continue,
                    }
                }
                let ready = match afford::ready(&mut b, afford::W) {
                    Ok(x) => x,
                    Err(_) => {
                        b.quit();
                        continue;
                    }
                };
                let mut rb = match ready {
                    None => None,
                    Some(Some(x)) => Some(x),
                    Some(None) => {
                        b.quit();
                        continue;
                    }
                };
                let eng: &mut Box<dyn Engine> = match rb.as_mut() {
                    Some(x) => x,
                    None => &mut b,
                };
                let o = eng.obs(&ObsReq::default()).map_err(e2s)?;
                let of = afford::offer(eng, &o).map_err(e2s)?;
                let inputs = afford::inputs_of(&of);
                let got: Rc<RefCell<Vec<Seen1>>> = Rc::new(RefCell::new(Vec::new()));
                let g2 = got.clone();
                enovel::set_observer(Some(Box::new(move |tr: &Trial, b: Option<&mut Box<dyn Engine>>| {
                    let view = b.and_then(|b| enovel::view_at(b).ok());
                    g2.borrow_mut().push(Seen1 { events: tr.events.clone(), frame: tr.frame, ended: tr.ended, room: tr.room, view });
                })));
                let cfg = AffordCfg::default();
                let r = afford::try_all(eng, &of, &inputs, &[], &NoNovelty, &cfg, None);
                enovel::set_observer(None);
                if let Some(x) = rb.take() {
                    x.quit();
                }
                b.quit();
                r.map_err(e2s)?;
                for s in got.borrow().iter() {
                    trials += 1;
                    d_trials += 1;
                    if s.room == target_room && first_room.is_none() {
                        first_room = Some(depth);
                    }
                    let Some((v, f)) = &s.view else { continue };
                    if s.ended {
                        continue;
                    }
                    let (k, _) = feat.offer(v, f, &ambient);
                    if k {
                        kept_total += 1;
                        d_kept += 1;
                        if first_unit.is_none() && v.live.iter().any(|u| *u == target_unit) {
                            first_unit = Some(depth);
                        }
                        let mut evs = node.clone();
                        evs.extend(s.events.iter().cloned());
                        next.push((evs, s.frame));
                    }
                }
            }
            per_depth.push(json!({"depth": depth, "nodes": frontier.len(), "trials": d_trials, "kept": d_kept}));
            if first_room.is_some() {
                why_stop = "room";
                break;
            }
            if next.is_empty() {
                why_stop = "no kept stop";
                break;
            }
            frontier = next;
        }
        root.quit();
        let rec = json!({"entry": id, "frame": frame, "trials": trials, "kept": kept_total, "per_depth": per_depth,
                         "first_unit_depth": first_unit, "first_room_depth": first_room, "stop": why_stop,
                         "ambient": ambient, "seconds": t0.elapsed().as_secs_f64()});
        files::write_json(&dd.join(format!("{id}.json")), &rec, Some(1))?;
        println!("{}", json!({"entry": id, "trials": trials, "kept": kept_total, "unit_depth": first_unit, "room_depth": first_room,
                              "stop": why_stop, "seconds": rec["seconds"]}));
    }
    Ok(())
}

fn route_cmd(a: &[String]) -> Res<()> {
    if a.len() < 2 {
        return Err("route ARCHIVE OUT --events FILE --entry I [--last N] --tjob JOB".into());
    }
    let (arch, out) = (PathBuf::from(&a[0]), PathBuf::from(&a[1]));
    let id: u64 = opt(a, "--entry").ok_or("--entry")?.parse().map_err(e2s)?;
    let last: usize = opt(a, "--last").map(|x| x.parse().unwrap_or(0)).unwrap_or(0);
    let tjob = opt(a, "--tjob").ok_or("--tjob JOB")?;
    let atoms = tjob_atoms(&out, tjob)?;
    let text = std::fs::read_to_string(opt(a, "--events").ok_or("--events FILE")?).map_err(e2s)?;
    let mut route: Vec<Event> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| events::event_of(&v))
        .collect();
    if last > 0 && last < route.len() {
        route = route[route.len() - last..].to_vec();
    }
    let rd = out.join("route");
    std::fs::create_dir_all(&rd).map_err(e2s)?;
    let h = Host::from_env().map_err(e2s)?;
    let (evs, frame) = movie(&arch, id)?;
    let mut root = boot(&h, &rd.join(format!("b{id}")), &evs, frame)?;
    let (_, ambient) = enovel::ambient_units(&mut root, enovel::AMBIENT_FRAMES).map_err(e2s)?;
    let seen = seen_before(&arch, &out, id)?;
    let sets: [(&str, bool, Vec<&'static str>); 3] = [("E", true, KINDS.to_vec()), ("TUW", true, vec!["U", "W"]), ("T", true, vec![])];
    let mut feats: Vec<Feat> = sets.iter().map(|(_, t, k)| Feat::new(*t, k, &atoms, &seen)).collect();
    {
        let (v, f) = enovel::view_at(&mut root).map_err(e2s)?;
        for ft in feats.iter_mut() {
            ft.offer(&v, &f, &BTreeSet::new());
        }
    }
    let mut rows = Vec::new();
    for i in 0..route.len() {
        let mut b = root.fork().map_err(e2s)?;
        b.inputs(&route[..=i]).map_err(e2s)?;
        let f1 = route[i].frame + 1;
        b.run(&RunSpec { frame: Some(f1), ..Default::default() }).map_err(e2s)?;
        let st = match b.run(&RunSpec { idle: true, max_frame: Some(f1 + afford::W), ..Default::default() }) {
            Ok(s) => s,
            Err(e) => {
                rows.push(json!({"input": i, "error": e.to_string()}));
                break;
            }
        };
        let (v, f) = enovel::view_at(&mut b).map_err(e2s)?;
        b.quit();
        let mut row = json!({"input": i, "click_frame": route[i].frame, "stop": st.frame, "reason": st.reason, "room": v.room,
                             "xstart": v.xstart, "live": v.live});
        for ((name, _, _), ft) in sets.iter().zip(feats.iter_mut()) {
            let (k, n) = ft.offer(&v, &f, &ambient);
            row[*name] = json!({"kept": k, "novel": n});
        }
        rows.push(row);
    }
    root.quit();
    let longest = |name: &str| -> usize {
        let (mut best, mut cur) = (0, 0);
        for r in &rows {
            if r[name]["kept"] == json!(false) {
                cur += 1;
                best = best.max(cur);
            } else {
                cur = 0;
            }
        }
        best
    };
    let rec = json!({"entry": id, "inputs": route.len(), "rows": rows, "ambient": ambient,
                     "longest_unkept": {"E": longest("E"), "TUW": longest("TUW"), "T": longest("T")}});
    files::write_json(&rd.join(format!("route-{id}-{}.json", route.len())), &rec, Some(1))?;
    println!("{}", rec["longest_unkept"]);
    Ok(())
}
