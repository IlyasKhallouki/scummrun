//! E012's try (experiments/E012-game-mechanics/plan.md, "How a try runs"; component `try`): the switches of the
//! mechanisms the try runs, the ports it reads them through where their crate is not this one's (T from the reading,
//! the wait), the try's novelty, the entries it commits besides its attempt's (the wait's stops, the affordance step's
//! kept outcomes, the delay fork's candidates), the dead moves, and the affordance step's triggers. tryjob.rs runs
//! them in the plan's order; with every switch off none of this runs and the try is E011 round 2's.

use crate::afford::{self, AffordCfg, Novelty};
use crate::archive::{FactLook, NewEntry, SharedCps};
use crate::events;
use crate::files::{self, Res};
use crate::progress::{Novel, Seen, TAtom};
use crate::start;
use fmi_core::{is_game_end, Checkpoint, EResult, Engine, Facts, Key, Obs, ObsReq, ReadingSource, Val};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn env_is(name: &str, v: &str) -> bool {
    std::env::var(name).is_ok_and(|x| x == v)
}

/// The E012 switches the try reads, each its component's (all off by default): `FMI_WAIT=1` (wait),
/// `FMI_TARGET_PER_KEY=1` (targets), `FMI_DELAY_FORK=1` (delay), `FMI_AFFORD=1` (afford), `FMI_SELECT=progress`
/// (archive). end's switches (`SPEEDRUN_REPORT_RESTART`, `FMI_BOXES`) are read where they act, in fmi-engine and afford.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Switches {
    pub wait: bool,
    pub per_key: bool,
    pub delay: bool,
    pub afford: bool,
    pub select: bool,
}

impl Switches {
    pub fn from_env() -> Switches {
        Switches {
            wait: env_is("FMI_WAIT", "1"),
            per_key: crate::step::per_key(),
            delay: env_is("FMI_DELAY_FORK", "1"),
            afford: afford::on(),
            select: crate::progress::on(),
        }
    }
    /// Some switch is on: the try reads T.
    pub fn any(&self) -> bool {
        self.wait || self.per_key || self.delay || self.afford || self.select
    }
    pub fn json(&self) -> Value {
        json!({"wait": self.wait, "per_key": self.per_key, "delay": self.delay, "afford": self.afford, "select": self.select})
    }
}

/// The reading's tested atoms T as the try passes them on (component `tested`, `Tested::of`, flattened: each atom a
/// key and its value set), with the record of their counts.
#[derive(Clone, Debug, Default)]
pub struct TestedT {
    pub flat: Vec<(Key, Vec<Val>)>,
    pub record: Value,
    keys: Vec<Key>,
    set: HashSet<Key>,
}

impl TestedT {
    pub fn new(flat: Vec<(Key, Vec<Val>)>, record: Value) -> TestedT {
        let set: HashSet<Key> = flat.iter().map(|(k, _)| *k).collect();
        let keys: Vec<Key> = flat.iter().map(|(k, _)| *k).collect::<BTreeSet<Key>>().into_iter().collect();
        TestedT { flat, record, keys, set }
    }
    /// T's keys, sorted (afford's tested keys, the delay fork's digest keys).
    pub fn keys(&self) -> &[Key] {
        &self.keys
    }
    /// Whether an action of the reading tests the key (targets' T tier).
    pub fn has(&self, k: Key) -> bool {
        self.set.contains(&k)
    }
}

/// T of a reading (provider: `tested`, in fmi-reading, which this crate does not depend on; fmi wires it).
pub trait TestedPort: Send + Sync {
    fn tested(&self, rd: &dyn ReadingSource) -> Result<TestedT, String>;
}

/// One stop of a wait: its frame and checkpoint (the state the entry's movie reaches at that frame with no new input).
#[derive(Clone, Debug)]
pub struct Stop {
    pub frame: u64,
    pub checkpoint: Checkpoint,
    pub room: i32,
}

/// A wait's outcome as the try reads it (component `wait`'s WaitOut).
#[derive(Clone, Debug, Default)]
pub struct Waited {
    pub frame0: u64,
    pub stops: Vec<Stop>,
    /// "horizon", "static", "ended: R" or "watchdog"
    pub end: String,
    /// the engine's end reason and frame when the run ended in the wait
    pub ended: Option<(String, u64)>,
    pub is_static: bool,
    /// the last due frame read (absolute; frame0 when no slot was paused)
    pub due: u64,
    /// E009's idle record from the wait's first frames, when asked
    pub idle: Option<Value>,
    /// the wait's record (result.json "e012"."wait")
    pub record: Value,
    pub cpu_s: f64,
    pub seconds: f64,
}

/// The wait (provider: `wait`): from the engine's stop with no input, the stops on T's atoms; `idle_frames` > 0 also
/// gives E009's idle record from the same frames.
pub trait WaitPort: Send + Sync {
    fn wait(&self, root: &mut Box<dyn Engine>, atoms: &[(Key, Vec<Val>)], rd: &dyn ReadingSource, idle_frames: u64)
            -> Result<Waited, String>;
    /// The last due frame of the dump's paused slots (fmi-core's `due`), None when no slot is paused: the wait's
    /// horizon at another frame than the wait's (the affordance step's).
    fn last_due(&self, o: &Obs) -> Option<u64>;
}

/// The ports of E012's mechanisms that live outside this crate (None: not wired; a switch that needs one fails the
/// try with an error saying so).
#[derive(Clone, Default)]
pub struct Mech {
    pub tested: Option<Arc<dyn TestedPort>>,
    pub wait: Option<Arc<dyn WaitPort>>,
}

/// The try's novelty (plan.md 1 "Kept", 4, 6): an atom of T or a room is novel for a state when no live archive entry
/// holds it (the archive's, job.json "novelty"; without it, the try's entry alone) and no entry this try keeps holds it
/// (IW(1) within the try).
pub struct TryNovel {
    base: Novel,
    /// "archive" (job.json "novelty") or "entry" (none in job.json: the entry's own state)
    pub source: &'static str,
    kept: Mutex<HashSet<String>>,
}

impl TryNovel {
    pub fn new(job: &Value, t: &[TAtom], entry: &Facts) -> TryNovel {
        match Novel::of_job(job, Some(t.to_vec())) {
            Some(base) => TryNovel { base, source: "archive", kept: Mutex::new(HashSet::new()) },
            None => TryNovel { base: Novel::new(seen_of(entry, t), t), source: "entry", kept: Mutex::new(HashSet::new()) },
        }
    }
    /// The novel atoms' texts (T's order, then `room=R`) that no entry this try keeps holds.
    pub fn novel(&self, f: &Facts) -> Vec<String> {
        let kept = self.kept.lock().unwrap();
        self.base.novel(f).into_iter().filter(|a| !kept.contains(a)).collect()
    }
    /// An entry is kept that holds these atoms: they are novel no more in this try.
    pub fn keep(&self, atoms: &[String]) {
        self.kept.lock().unwrap().extend(atoms.iter().cloned());
    }
    /// An entry is kept (the attempt's): what it holds is novel no more in this try.
    pub fn keep_facts(&self, f: &Facts) {
        let n = self.novel(f);
        self.keep(&n);
    }
    /// The delay fork's novelty of an end state (fmi-play's `NovelOf`): this novelty as it stands now.
    pub fn snapshot(&self) -> Arc<dyn Fn(&Facts) -> Option<bool> + Send + Sync> {
        let base = self.base.clone();
        let kept = self.kept.lock().unwrap().clone();
        Arc::new(move |f: &Facts| Some(base.novel(f).iter().any(|a| !kept.contains(a))))
    }
    pub fn open(&self) -> usize {
        self.base.open.len()
    }
}

impl Novelty for TryNovel {
    fn novel(&self, f: &Facts) -> Vec<String> {
        TryNovel::novel(self, f)
    }
}

/// What the entry alone holds, as the archive's "novelty" gives what its live entries hold: its room, and each T key's
/// value (an absent key is 0).
fn seen_of(entry: &Facts, t: &[TAtom]) -> Seen {
    let mut s = Seen::default();
    if let Some(r) = FactLook::look(entry, "room") {
        s.rooms.insert(r);
    }
    for a in t {
        if !s.vals.contains_key(&a.key) {
            s.vals.insert(a.key.clone(), vec![FactLook::look(entry, &a.key).unwrap_or(0)]);
        }
    }
    s
}

/// Why the affordance step runs (plan.md 4, "Triggers").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Lemma R's R0 stop
    R0,
    /// the same empty window twice
    EmptyWindow,
    /// no target of the try plans
    NoPlan,
}

impl Trigger {
    pub fn text(&self) -> &'static str {
        match self {
            Trigger::R0 => "R0",
            Trigger::EmptyWindow => "empty window",
            Trigger::NoPlan => "no plan",
        }
    }
}

/// The trigger a played attempt's stop gives: Lemma R's R0 (the harness's refinement turn) or the window search's
/// "the same empty window N times".
pub fn trigger_of_stop(stop: Option<&str>) -> Option<Trigger> {
    let s = stop?;
    if s.contains("Lemma R's premise R0") {
        Some(Trigger::R0)
    } else if s.starts_with("the same empty window") {
        Some(Trigger::EmptyWindow)
    } else {
        None
    }
}

/// The game's end a harness stop names ("engine: engine ended: R" with R one of fmi-core's GAME_ENDS).
pub fn game_end_of_stop(stop: Option<&str>) -> Option<String> {
    let r = stop?.strip_prefix("engine: engine ended: ")?;
    let r = r.split(['\n', ';']).next().unwrap_or("").trim();
    is_game_end(r).then(|| r.to_string())
}

/// A dead move (plan.md 5): its run ended in the game's own end; it has no entry. result.json "dead".
pub fn dead_json(reason: &str, frame: u64, events: &[Value], how: &str) -> Value {
    json!({"reason": reason, "frame": frame, "events": events, "how": how})
}

/// An entry the try commits besides its attempt's: its events after the try's entry (absolute frames) and its
/// checkpoints (frame order, the last at its frame), with the novel atoms it was kept for.
#[derive(Clone, Debug)]
pub struct Extra {
    pub how: &'static str,
    pub events: Vec<Value>,
    pub cps: Vec<Checkpoint>,
    pub novel: Vec<String>,
}

impl Extra {
    pub fn frame(&self) -> u64 {
        self.cps.last().map(|c| c.frame).unwrap_or(0)
    }
}

/// The extra entries written as the try's new entries `first`.. (JOB_DIR/new/K.json.gz), each kept for the ingest;
/// their result.json "new" items.
pub fn write_extras(new_dir: &Path, first: usize, parent: i64, entry_frame: u64, extras: Vec<Extra>) -> Res<Vec<Value>> {
    std::fs::create_dir_all(new_dir).map_err(|e| files::io_err(new_dir, e))?;
    let mut out = Vec::new();
    for (j, x) in extras.into_iter().enumerate() {
        let frame = x.frame();
        if x.cps.is_empty() || frame <= entry_frame || x.cps.iter().any(|c| c.frame <= entry_frame) {
            return Err(format!("ValueError: a {} entry at frame {frame}: not a segment after frame {entry_frame}", x.how));
        }
        if let Some(e) = x.events.iter().find(|e| files::as_i64(e.get("frame")).map(|f| f <= entry_frame as i64 || f > frame as i64).unwrap_or(true)) {
            return Err(format!("ValueError: a {} entry at frame {frame}: event {} outside ({entry_frame}, {frame}]", x.how, files::py_repr(e)));
        }
        let i = first + j;
        let tick = x.cps.last().unwrap().tick as i64;
        let n = x.cps.len();
        let ne = NewEntry { parent, frame: frame as i64, tick: Some(tick), events: x.events, shared: Arc::new(SharedCps::new(x.cps)), n };
        let p = new_dir.join(format!("{i}.json.gz"));
        ne.write(&p, None)?;
        crate::archive::stash_shared(&p, ne);
        out.push(json!({"file": format!("new/{i}.json.gz"), "frame": frame, "tick": tick, "how": x.how, "novel": x.novel}));
    }
    Ok(out)
}

/// A checkpoint as the archive's files write it ([frame, facts, {"tick", "rng", "actors"}]), read back.
pub fn checkpoint_of_json(v: &Value) -> Option<Checkpoint> {
    let frame = v.get(0)?.as_u64()?;
    let facts = start::facts_of_json(v.get(1)?);
    let ex = v.get(2)?;
    let actors = ex
        .get("actors")?
        .as_array()?
        .iter()
        .map(|a| Some((a.get(0)?.as_i64()? as i32, a.get(1)?.as_i64()? as i32, a.get(2)?.as_i64()? as i32)))
        .collect::<Option<Vec<_>>>()?;
    Some(Checkpoint { frame, facts, tick: ex.get("tick")?.as_u64()?, rng: ex.get("rng")?.as_u64()?, actors })
}

/// The delay fork's candidates in an attempt's window records (`"delay"`'s `"candidates"`, component delay): each
/// its inputs after the try's entry (the attempt's movie up to the window, then the child's) and its end checkpoint,
/// with the window's index and the candidate's k.
pub fn delay_candidates(windows: &[Value]) -> Vec<(usize, Value, Vec<Value>, Checkpoint)> {
    let mut out = Vec::new();
    for (w, rec) in windows.iter().enumerate() {
        let Some(cs) = rec.get("delay").and_then(|d| d.get("candidates")).and_then(|c| c.as_array()) else { continue };
        for c in cs {
            let (Some(inputs), Some(cp)) = (c.get("inputs").and_then(|i| i.as_array()), c.get("checkpoint").and_then(checkpoint_of_json)) else {
                continue;
            };
            out.push((w, c.get("k").cloned().unwrap_or(Value::Null), inputs.clone(), cp));
        }
    }
    out
}

/// The delay fork's record of an attempt: its windows' `"delay"` fields summed (the window summary's own `"delay"`
/// is kept as the play gives it).
pub fn delay_record(windows: &[Value]) -> Value {
    let mut triggered = 0;
    let mut chosen: Vec<Value> = Vec::new();
    let (mut refuted, mut cands, mut outcomes) = (0, 0, 0);
    for rec in windows {
        let Some(d) = rec.get("delay") else { continue };
        if d.get("k_max").is_some() || d.get("draws").is_some() {
            triggered += 1;
            chosen.push(d.get("chosen").cloned().unwrap_or(Value::Null));
            if files::truthy(d.get("refuted")) {
                refuted += 1;
            }
            cands += d.get("candidates").and_then(|c| c.as_array()).map(|c| c.len()).unwrap_or(0);
            outcomes += d.get("outcomes").and_then(|c| c.as_array()).map(|c| c.len()).unwrap_or(0);
        }
    }
    json!({"windows": windows.len(), "triggered": triggered, "chosen": chosen, "refuted": refuted, "outcomes": outcomes,
           "candidates": cands})
}

/// The affordance step's outcome in the try: its record, the entries it keeps, its dead moves, the inputs it offered.
#[derive(Debug, Default)]
pub struct Afforded {
    pub record: Value,
    pub kept: Vec<Extra>,
    pub dead: Vec<Value>,
    pub offered: usize,
    pub cpu_s: f64,
}

/// The affordance step (plan.md 4) on the engine `e` stands at: made ready (a fork run with no input to the first idle
/// stop, when `e` is not idle), the interface's inputs offered there, each tried in a fork (afford's `try_all`), the
/// outcomes kept by the try's novelty. `prefix_events`, `prefix_cps`: the movie and checkpoints from the try's entry to
/// `e`'s frame (the kept entries continue them). `dead_moves`: job.json's dead moves of the entry, whose inputs are not
/// tried again. `last_due`: the wait's horizon at the offer's frame (its last due frame); `then`: the wait's stops
/// after a trial's idle stop.
#[allow(clippy::too_many_arguments)]
pub fn afford_step(e: &mut Box<dyn Engine>, why: Trigger, t: &TestedT, nov: &TryNovel, last_due: Option<&dyn Fn(&Obs) -> Option<u64>>,
                   then: Option<afford::Then<'_>>, dead_moves: &[Value], prefix_events: &[Value], prefix_cps: &[Checkpoint])
                   -> EResult<Afforded> {
    let t0 = Instant::now();
    let frame = e.status().frame;
    let mut out = Afforded::default();
    let ready = afford::ready(e, afford::W)?;
    let mut fork = match ready {
        None => None,
        Some(Some(b)) => Some(b),
        Some(None) => {
            out.record = json!({"why": why.text(), "frame": frame, "ready": false, "seconds": t0.elapsed().as_secs_f64()});
            return Ok(out);
        }
    };
    let res = (|| -> EResult<(afford::AffordOut, u64, u64, usize)> {
        let eng: &mut Box<dyn Engine> = match fork.as_mut() {
            Some(b) => b,
            None => e,
        };
        let o = eng.obs(&ObsReq::default())?;
        let f0 = eng.status().frame;
        let horizon = afford::W + last_due.and_then(|d| d(&o)).map(|d| d.saturating_sub(f0)).unwrap_or(0);
        let offer = afford::offer(eng, &o)?;
        let all = afford::inputs_of(&offer);
        let dead: HashSet<String> = dead_moves.iter().filter_map(|d| d.get("events")).map(|ev| ev.to_string()).collect();
        let mut skipped = 0;
        let inputs: Vec<afford::Inputs> = all
            .into_iter()
            .filter(|inp| {
                if dead.is_empty() {
                    return true;
                }
                let mut evs: Vec<Value> = prefix_events.to_vec();
                evs.extend(inp.events(f0).iter().map(events::event_json));
                let d = dead.contains(&Value::Array(evs).to_string());
                skipped += d as usize;
                !d
            })
            .collect();
        let cfg = AffordCfg { horizon, ..Default::default() };
        let r = afford::try_all(eng, &offer, &inputs, t.keys(), nov, &cfg, then)?;
        Ok((r, f0, horizon, skipped))
    })();
    if let Some(b) = fork.take() {
        out.cpu_s += b.cpu_s();
        b.quit();
    }
    let (r, f0, horizon, skipped) = res?;
    out.offered = r.trials.iter().filter(|t| t.inputs.clicks.len() < 3).count();
    for k in &r.kept {
        nov.keep(&k.novel);
        let mut evs = prefix_events.to_vec();
        evs.extend(k.events_json());
        let mut cps: Vec<Checkpoint> = prefix_cps.iter().filter(|c| c.frame < k.checkpoint.frame).cloned().collect();
        cps.push(k.checkpoint.clone());
        out.kept.push(Extra { how: "afford", events: evs, cps, novel: k.novel.clone() });
    }
    for tr in r.trials.iter().filter(|t| t.ended && is_game_end(&t.reason)) {
        let mut evs = prefix_events.to_vec();
        evs.extend(tr.events.iter().map(events::event_json));
        out.dead.push(dead_json(&tr.reason, tr.frame, &evs, "afford"));
    }
    out.cpu_s += r.engine_cpu_s;
    let mut rec = r.json();
    rec["why"] = json!(why.text());
    rec["frame"] = json!(frame);
    rec["offer_frame"] = json!(f0);
    rec["ready"] = json!(true);
    rec["horizon"] = json!(horizon);
    rec["skipped_dead"] = json!(skipped);
    rec["dead"] = json!(out.dead.len());
    rec["step_seconds"] = json!(t0.elapsed().as_secs_f64());
    out.record = rec;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmi_core::K_ROOM;

    fn facts(pairs: &[(&str, i64)]) -> Facts {
        Facts::from_pairs(pairs.iter().map(|(k, v)| (Key::parse(k), *v)).collect())
    }

    #[test]
    fn triggers_and_ends() {
        assert_eq!(trigger_of_stop(Some("refinement: Lemma R's premise R0: the last pass wrote values of var [194] outside the domain")), Some(Trigger::R0));
        assert_eq!(trigger_of_stop(Some("the same empty window 2 times on s1: x")), Some(Trigger::EmptyWindow));
        assert_eq!(trigger_of_stop(Some("goal")), None);
        assert_eq!(trigger_of_stop(None), None);
        assert_eq!(game_end_of_stop(Some("engine: engine ended: quit")), Some("quit".into()));
        assert_eq!(game_end_of_stop(Some("engine: engine ended: restart")), Some("restart".into()));
        assert_eq!(game_end_of_stop(Some("engine: engine ended: crash")), None);
        assert_eq!(game_end_of_stop(Some("planner timeout (120 s)")), None);
    }

    #[test]
    fn novelty_from_the_entry_and_iw1_within_the_try() {
        let t = vec![TAtom::value("state:316", 1), TAtom::value("var:103", 382)];
        let entry = facts(&[("room", 28), ("state:316", 0), ("var:103", 0)]);
        let n = TryNovel::new(&json!({}), &t, &entry);
        assert_eq!(n.source, "entry");
        assert!(n.novel(&entry).is_empty());
        let door = facts(&[("room", 28), ("state:316", 1)]);
        assert_eq!(n.novel(&door), vec!["state:316=1".to_string()]);
        let other_room = facts(&[("room", 41), ("state:316", 1)]);
        assert_eq!(n.novel(&other_room), vec!["state:316=1".to_string(), "room=41".to_string()]);
        let snap = n.snapshot();
        n.keep(&n.novel(&door));
        assert!(n.novel(&door).is_empty());
        assert_eq!(n.novel(&other_room), vec!["room=41".to_string()]);
        // the snapshot is the novelty when it was taken
        assert_eq!(snap(&door), Some(true));
        let snap2 = n.snapshot();
        assert_eq!(snap2(&door), Some(false));
        assert_eq!(snap2(&other_room), Some(true));
        let _ = K_ROOM;
    }

    #[test]
    fn novelty_from_the_archive() {
        let t = vec![TAtom::value("state:316", 1)];
        let job = json!({"novelty": {"t": [], "rooms": [28, 33], "seen": {"state:316": [0, 1]}}});
        let n = TryNovel::new(&job, &t, &facts(&[("room", 28)]));
        assert_eq!(n.source, "archive");
        assert!(n.novel(&facts(&[("room", 33), ("state:316", 1)])).is_empty());
        assert_eq!(n.novel(&facts(&[("room", 41)])), vec!["room=41".to_string()]);
    }

    #[test]
    fn checkpoints_round_trip() {
        let c = Checkpoint { frame: 3000, facts: facts(&[("room", 28), ("var:103", 382)]), tick: 20611, rng: 3182679328, actors: vec![(1, 2, 3)] };
        let v = crate::harness::checkpoint_json(&c);
        assert_eq!(checkpoint_of_json(&v), Some(c));
    }

    #[test]
    fn delay_candidates_from_windows() {
        let cp = json!([3010, {"room": 28}, {"tick": 5, "rng": 7, "actors": []}]);
        let ws = vec![
            json!({"delay": {"rng": {}}}),
            json!({"delay": {"k_max": 7, "chosen": 1, "candidates": [{"k": 0, "inputs": [{"frame": 3001, "click": [1, 2]}], "checkpoint": cp}]}}),
            json!({}),
        ];
        let c = delay_candidates(&ws);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, 1);
        assert_eq!(c[0].3.frame, 3010);
        let r = delay_record(&ws);
        assert_eq!(r["triggered"], json!(1));
        assert_eq!(r["candidates"], json!(1));
    }

    #[test]
    fn extras_are_segments_after_the_entry() {
        let d = std::env::temp_dir().join(format!("fmi-mech-{}", std::process::id()));
        let cp = |f: u64| Checkpoint { frame: f, facts: facts(&[("room", 28)]), tick: f * 6, rng: 1, actors: vec![] };
        let ok = Extra { how: "wait", events: vec![], cps: vec![cp(110)], novel: vec!["room=28".into()] };
        let items = write_extras(&d, 3, 7, 100, vec![ok]).unwrap();
        assert_eq!(items[0]["file"], json!("new/3.json.gz"));
        assert_eq!(items[0]["frame"], json!(110));
        let doc = files::read_gz(&d.join("3.json.gz")).unwrap();
        assert_eq!(doc["parent"], json!(7));
        assert_eq!(doc["checkpoints"].as_array().unwrap().len(), 1);
        let early = Extra { how: "afford", events: vec![json!({"frame": 100, "click": [1, 1]})], cps: vec![cp(120)], novel: vec![] };
        assert!(write_extras(&d, 0, 7, 100, vec![early]).is_err());
        let before = Extra { how: "delay", events: vec![], cps: vec![cp(90)], novel: vec![] };
        assert!(write_extras(&d, 0, 7, 100, vec![before]).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// The wait component's `wait` (fmi_explore::wait) as the try reads it: its stops and end, its horizon's last due frame,
/// E009's idle record from its first frames (with the wait's seconds and CPU when the record has none), its record.
pub struct RealWait;

impl WaitPort for RealWait {
    fn wait(&self, root: &mut Box<dyn Engine>, atoms: &[(Key, Vec<Val>)], rd: &dyn ReadingSource, idle_frames: u64)
            -> Result<Waited, String> {
        let cfg = crate::wait::WaitCfg { idle_frames, ..Default::default() };
        let w = crate::wait::wait(root, atoms, rd, &cfg)?;
        let ended = match &w.end {
            crate::wait::WaitEnd::Ended(r) => Some((r.clone(), w.frame0 + w.frames)),
            _ => None,
        };
        let idle = w.idle.clone().map(|mut v| {
            if v.get("seconds").is_none() {
                v["seconds"] = json!(crate::pyjson::round(w.seconds, 1));
            }
            if v.get("cpu_s").is_none() {
                v["cpu_s"] = json!(crate::pyjson::round(w.cpu_s, 1));
            }
            v
        });
        Ok(Waited {
            frame0: w.frame0,
            stops: w.stops.iter().map(|s| Stop { frame: s.frame, checkpoint: s.checkpoint.clone(), room: s.room }).collect(),
            end: w.end.text(),
            ended,
            is_static: w.is_static(),
            due: w.horizon,
            idle,
            record: w.json(atoms),
            cpu_s: w.cpu_s,
            seconds: w.seconds,
        })
    }
    fn last_due(&self, o: &Obs) -> Option<u64> {
        crate::wait::dues_of(o).iter().map(|d| d.1).max()
    }
}
