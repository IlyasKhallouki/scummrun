//! replay.Harness's planning loop with the window search (`run` with `window_search`, `_run_windows`), line for line,
//! in E009's and E010's settings: the taskcore planner served (`plan`, `_plan_taskcore`, `_planned`, `_same_input`),
//! cost caps, the reach extension (`_extend`), unobserved actor rooms, the strict window search, E007's refinement
//! loop (`_refine_turn`, `_swap`), and the record (`result`). F10's slot locals, the Python planner and the step
//! rules without the window search are not E009's settings and are refused.
//!
//! Its ports: the reading (fmi-core `ReadingSource`), the planner of each reading (fmi-core `PlannerHost`, kept warm
//! across attempts in `PlannerCache`), the window search (`WindowPlayer`), and the reader for the extension and the
//! refinement (`Rereads`, replay_part1's wiring).

use crate::events;
use crate::files;
use crate::nogood;
use crate::ports::WindowPlayer;
use crate::pyjson;
use crate::start::{self, Points};
use fmi_core::{
    facts_of, Checkpoint, Domains, EngineError, Facts, Key, Obs, PlanCtx, PlanError, PlanRequest, PlanResult, PlayOutcome,
    PlayState, Planner, PlannerHost, ReadingSource, Search, Val, K_ACTORROOM,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// E014 amendment (8)'s switch `FMI_GOAL_FACT=1` (assumptions row 23): before a plan, a target fact the task's reading
/// does not hold after a refinement stops the attempt (`goal_fact_stop`) without calling the planner, whose error it was
/// ("taskcore: --goal: no fact ..."). Unset: as before.
pub fn goal_fact_on() -> bool {
    std::env::var("FMI_GOAL_FACT").is_ok_and(|v| v == "1")
}

/// The first of the goal's facts that the reading's domains do not hold.
pub fn missing_goal_fact(goal: &[(Key, Vec<Val>)], doms: &Domains) -> Option<Key> {
    goal.iter().map(|g| g.0).find(|k| doms.get(*k).is_none())
}

/// The stop of an attempt whose refined task holds no fact of its target: a refinement's stop (model-blind in the
/// stall table), not the planner's error.
pub fn goal_fact_stop(k: Key) -> String {
    format!("refinement: the refined task holds no fact of the target: {}", k.text())
}

/// The harness's settings (step.py's attempt wiring).
#[derive(Clone, Debug)]
pub struct HarnessCfg {
    pub replan_limit: u32,
    pub time_limit_s: f64,
    pub plan_time_s: u64,
    /// E011's iterative deepening (plan.md, amendment "targets with no plan made cheap"): the planner's limit for the
    /// plans before the attempt's first plan; None: plan_time_s for every plan (the old behaviour)
    pub first_plan_time_s: Option<u64>,
    pub search: Search,
    pub cost_caps: bool,
    pub cost_dmax: Option<i64>,
    pub unobserved_actor_rooms: bool,
    pub reach_extend: bool,
    pub reach_extend_limit: usize,
    pub refine: bool,
    pub start_check: String,
    /// the plans' folder (adir/plans)
    pub workdir: PathBuf,
    /// the search's name in the plans' entries ("lama-first-costs" for FdLamaFirstCosts)
    pub search_name: String,
    /// E011's old tail first (the fast settings; off: the reference's planner calls): at a replan the planner is
    /// given the last plan's steps, and takes the shortest suffix that is a plan from the new start when one is
    pub tail: bool,
}

/// The reader for the reach extension and the refinement (replay_part1's reach_extender and refiner).
pub trait Rereads {
    /// The reading's scope read again from every start so far and this one (reader-scope --reach-also); the start as
    /// its file's text (`start::reach_start_text`).
    fn extend(&mut self, start: &str) -> Result<Arc<dyn ReadingSource>, String>;
    /// The scope read again with the variables `new` made facts too, from every start so far and this one.
    fn refine(&mut self, new: &[i64], start: &str) -> Result<Arc<dyn ReadingSource>, String>;
    /// CPU seconds of the readers so far (the extension's and refinement's records)
    fn cpu_s(&self) -> f64;
    /// E022 (FMI_REFINE_WIDEN): a further start for the next reads, read by none now (it seeds the fact
    /// variables' domains with its values)
    fn add_start(&mut self, _start: &str) -> Result<(), String> {
        Err("no further starts".into())
    }
    /// E022 (FMI_SLOT_LOCALS): the live slots of a further start, with their dumped locals, for the next reads (the
    /// reader builds their lists too, so a replan from that start finds its slots' lists)
    fn add_slot_locals(&mut self, _slots: Vec<Value>) {}
}

/// E022: `FMI_REFINE_WIDEN=N`, at most N more refinement passes when the last pass wrote values of a new fact
/// variable outside its domain (Lemma R's premise R0): each such value seeds the domain through a start that holds it,
/// and the scope is read again. Unset or 0: the try stops at R0, as before.
pub fn refine_widen() -> usize {
    std::env::var("FMI_REFINE_WIDEN").ok().and_then(|v| v.parse().ok()).unwrap_or(0)
}

/// E022: the live slots' dumped locals of `state` under `FMI_SLOT_LOCALS=1` (tryjob's provider), else None.
fn slot_locals_now(state: &fmi_core::Obs) -> Option<Vec<Value>> {
    if !std::env::var("FMI_SLOT_LOCALS").is_ok_and(|v| v == "1") {
        return None;
    }
    crate::tryjob::SLOT_LOCALS.get().and_then(|f| f(state))
}

/// E022 (`FMI_SLOT_LOCALS=1`; reader task.rs `replan_start_locals`, F10): each `args:<unit>` of a slot live at the
/// plan's start is the number of the reading's list (its `reading.lists.jsonl`) at the slot's wait point whose locals
/// and flags the slot's dumped locals match, the list with the most locals when several do; with no match the start
/// keeps its value. Without the switch, or without dumped locals, nothing changes. Returns the slots with no match
/// (`name@pos`).
fn args_of_slots(start_f: &mut fmi_core::Facts, doms: &Domains, state: &fmi_core::Obs, dir: &std::path::Path, counts: &mut BTreeMap<String, i64>) -> Vec<String> {
    let mut unmatched = Vec::new();
    if !std::env::var("FMI_SLOT_LOCALS").is_ok_and(|v| v == "1") {
        return unmatched;
    }
    let Some(slots) = crate::tryjob::SLOT_LOCALS.get().and_then(|f| f(state)) else { return unmatched };
    let lists: Vec<Value> = std::fs::read_to_string(dir.join("reading.lists.jsonl"))
        .map(|t| t.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
        .unwrap_or_default();
    let flag = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_bool());
    for (k, _) in &doms.0 {
        if k.kind() != fmi_core::K_ARGS {
            continue;
        }
        let text = k.text();
        let name = &text[5..];
        let pos = start_f.get(fmi_core::Key::parse(&format!("pos:{name}"))).unwrap_or(start::NOWHERE);
        if pos < 0 {
            continue;
        }
        let Some(slot) = slots.iter().find(|s| s["name"].as_str() == Some(name)) else { continue };
        let locals: Vec<i64> = slot["locals"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
        let sf = &slot["flags"];
        let best = lists
            .iter()
            .filter(|l| l["name"].as_str() == Some(name) && l["w"].as_i64() == Some(pos))
            .filter(|l| {
                let lf = &l["flags"];
                let fl_ok = ["resistant", "owned", "recursive"].iter().all(|f| match (flag(lf, f), flag(sf, f)) {
                    (Some(a), Some(b)) => a == b,
                    _ => true,
                });
                fl_ok && l["locals"].as_array().map(|ps| ps.iter().all(|p| {
                    let i = p[0].as_i64().unwrap_or(-1);
                    i >= 0 && locals.get(i as usize).copied().unwrap_or(0) == p[1].as_i64().unwrap_or(i64::MIN)
                })).unwrap_or(false)
            })
            .max_by_key(|l| l["locals"].as_array().map(|a| a.len()).unwrap_or(0));
        match best.and_then(|l| l["sig"].as_i64()) {
            Some(sig) => {
                start_f.set(*k, sig as fmi_core::Val);
                *counts.entry("args from the slot's dumped locals".into()).or_insert(0) += 1;
            }
            None => {
                *counts.entry("live slots whose dumped locals match no list".into()).or_insert(0) += 1;
                unmatched.push(format!("{name}@{pos}"));
            }
        }
    }
    unmatched
}

/// A reach start's text with `key` set to `v` (`key<TAB>value` lines).
fn start_with(text: &str, key: &str, v: i64) -> String {
    let mut out: Vec<String> = text.lines().filter(|l| l.split('\t').next() != Some(key)).map(String::from).collect();
    out.push(format!("{key}\t{v}"));
    out.join("\n") + "\n"
}

/// A reading's warm planner, locked by itself (E016 amendment (9)): one try's long search waits only the tries
/// that plan on the same reading, not every worker.
pub type SharedPlanner = Arc<Mutex<Box<dyn Planner>>>;

/// The warm planner of each reading (one per reading id), kept across plans and attempts.
pub struct PlannerCache {
    pub host: Arc<dyn PlannerHost>,
    kept: HashMap<[u8; 20], SharedPlanner>,
}

impl PlannerCache {
    pub fn new(host: Arc<dyn PlannerHost>) -> PlannerCache {
        PlannerCache { host, kept: HashMap::new() }
    }
    /// The reading's planner, made on first use; the cache's lock is held only for this, the planner's own lock
    /// for a plan.
    pub fn get(&mut self, rd: &dyn ReadingSource) -> Result<SharedPlanner, PlanError> {
        let id = rd.id();
        if !self.kept.contains_key(&id) {
            let p = self.host.planner(rd)?;
            self.kept.insert(id, Arc::new(Mutex::new(p)));
        }
        Ok(self.kept[&id].clone())
    }
    /// plan_fd.close_servers: the planners' memory, before a reader reads
    pub fn close(&mut self, id: &[u8; 20]) {
        self.kept.remove(id);
    }
    pub fn close_all(&mut self) {
        self.kept.clear();
    }
}

/// E016 amendment (9): `FMI_PLANNER_LOCK=reading`, a plan holds its reading's planner's lock and not the cache's, so
/// tries on other readings plan meanwhile. Unset: the cache's lock for the whole request, as before.
pub fn planner_lock_per_reading() -> bool {
    std::env::var("FMI_PLANNER_LOCK").is_ok_and(|v| v == "reading")
}

/// Whether the refinement keeps the old reading's planner (FMI_PLANNER_KEEP, on; 0 closes it as round 1 did).
fn planner_keep() -> bool {
    std::env::var("FMI_PLANNER_KEEP").map(|v| v != "0").unwrap_or(true)
        && crate::machine::mem_available_gb() >= crate::machine::Machine::from_env().mem_second_gb
}

/// The run stopped by an engine error (replay.Stop / ControlError): its reason.
fn engine_stop(e: &EngineError) -> String {
    match e {
        EngineError::Ended(s) => format!("engine: engine ended: {}", s.reason),
        e => format!("engine: {e}"),
    }
}

pub struct Harness<'a> {
    pub cfg: HarnessCfg,
    planners: &'a Mutex<PlannerCache>,
    player: Box<dyn WindowPlayer>,
    rereads: Option<&'a mut dyn Rereads>,
    pub reading: Arc<dyn ReadingSource>,
    pub b: Points,
    pub goal: Vec<(Key, Vec<Val>)>,
    pub ps: PlayState,
    pub cur: PlanResult,
    pub plans: Vec<Map<String, Value>>,
    pub replans: u32,
    pub extensions: Vec<Value>,
    /// E022 amendment (24) (`FMI_BAN_TRY=1`): label -> how often this try's plays refused it (any state)
    pub try_refused: HashMap<String, u32>,
    pub refinements: Vec<Value>,
    refined: Vec<(i64, usize)>,
    frame0: u64,
    tick0: u64,
    qf0: u64,
    t0: Instant,
    unobserved: Vec<&'static str>,
    predicted: Vec<&'static str>,
    progress: &'a dyn Fn(&str),
    /// the reading's domain keys in their text order (the start files'), and the reading's id
    order: Option<([u8; 20], start::TextOrder)>,
    /// E016 C (FMI_NOGOOD): the try's failure memory, the target's text, the current plan's exact state (its prefix key)
    /// and start, and the no-plan key's keys of the reading (its id)
    nogood: Option<nogood::Ctx>,
    ng_target: String,
    plan_at: Option<(String, Facts)>,
    ng_keys: Option<([u8; 20], Arc<HashSet<Key>>)>,
}

/// The stop reason of a plan request's status other than "plan" (the planner's, or the no-plan memory's).
fn status_stop(status: &str, limit: u64) -> String {
    match status {
        "time" => format!("planner timeout ({} s)", limit),
        "unsolvable" => "no plan (the planner proved none)".into(),
        "incomplete" => "no plan (the search ended without one)".into(),
        "memory" => "planner memory limit (3G)".into(),
        s => format!("planner {s}"),
    }
}

impl<'a> Harness<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(cfg: HarnessCfg, planners: &'a Mutex<PlannerCache>, player: Box<dyn WindowPlayer>,
               rereads: Option<&'a mut dyn Rereads>, reading: Arc<dyn ReadingSource>, goal: Vec<(Key, Vec<Val>)>,
               ps: PlayState, progress: &'a dyn Fn(&str)) -> Result<Harness<'a>, String> {
        if cfg.cost_caps && cfg.cost_dmax.unwrap_or(0) == 0 {
            return Err("cost caps: the taskcore planner and the reading's dmax (reading.json frame_costs)".into());
        }
        if (cfg.reach_extend || cfg.refine) && rereads.is_none() {
            return Err("the reach extension and the refinement read the scope again: they need the reader".into());
        }
        let head = crate::warm::head(&*reading);
        let b = Points::of(reading.domains(), head.get("live_locals").filter(|v| !v.is_null()).cloned());
        Ok(Harness {
            unobserved: start::unobserved(cfg.unobserved_actor_rooms),
            predicted: start::predicted(cfg.unobserved_actor_rooms),
            cfg,
            planners,
            player,
            rereads,
            reading,
            b,
            goal,
            ps,
            cur: PlanResult::default(),
            plans: Vec::new(),
            replans: 0,
            extensions: Vec::new(),
            try_refused: HashMap::new(),
            refinements: Vec::new(),
            refined: Vec::new(),
            frame0: 0,
            tick0: 0,
            qf0: 0,
            t0: Instant::now(),
            progress,
            order: None,
            nogood: None,
            ng_target: String::new(),
            plan_at: None,
            ng_keys: None,
        })
    }

    /// E016 C (FMI_NOGOOD): the try's failure memory for this attempt (`nogood`). Without it the harness plans with
    /// no bans and no memory, as before.
    pub fn set_nogood(&mut self, c: nogood::Ctx) {
        self.ng_target = nogood::target_text(&self.goal);
        self.nogood = Some(c);
    }

    /// E016 C: the no-plan key's keys for the current reading: its tested keys and the target's (None: every key).
    fn ng_keys(&mut self) -> Option<Arc<HashSet<Key>>> {
        let c = self.nogood.as_ref()?;
        let id = self.reading.id();
        if let Some((k, ks)) = &self.ng_keys {
            if *k == id {
                return Some(ks.clone());
            }
        }
        let tk = c.tested.as_ref().and_then(|f| f(&*self.reading))?;
        let mut ks: HashSet<Key> = tk.into_iter().collect();
        ks.extend(self.goal.iter().map(|g| g.0));
        let ks = Arc::new(ks);
        self.ng_keys = Some((id, ks.clone()));
        Some(ks)
    }

    /// E016 C: the no-plan key's keys (`ng_keys`), or every key of the start when the tested keys are not wired; None
    /// without the memory.
    fn ng_keys_or_all(&mut self, start_f: &Facts) -> Option<Arc<HashSet<Key>>> {
        self.nogood.as_ref()?;
        Some(self.ng_keys().unwrap_or_else(|| Arc::new(start_f.iter().map(|(k, _)| *k).collect())))
    }

    /// E016 C: a play that ended at an empty window (the step refused from where the plan stood) refutes the step at
    /// the plan's exact state: its ban (`nogood::ban_of`) is kept for the plan's (entry, prefix, target).
    fn note_refuted(&mut self) {
        let Some(c) = self.nogood.clone() else { return };
        let Some((prefix, start_f)) = self.plan_at.clone() else { return };
        let d = self.player.replan_cause_detail();
        let what = d.as_ref().and_then(|d| d.get("what")).and_then(|w| w.as_str());
        // E022 amendment (27) (FMI_BAN_TRY): a deviation counts its refuted record's label too. Try 7 planned "look at
        // the archway" and the sentence script's continuation after it, which no record of the engine's end matched;
        // each deviation kept 27 frames and the same plan came back, 13 times, to the replan limit
        if what == Some("deviation") && std::env::var("FMI_BAN_TRY").is_ok_and(|v| v == "1") {
            let rec = d.as_ref().and_then(|d| d.get("cause")).and_then(|c| c.get("record")).and_then(|r| r.as_str());
            if let Some(name) = rec {
                let label = match self.reading.by_name(name) {
                    Some(a) => self.reading.record(a).label,
                    None => nogood::label_of_name(name),
                };
                *self.try_refused.entry(label).or_insert(0) += 1;
            }
        }
        if what != Some("window empty") {
            return;
        }
        let n_plan = self.plans.len() as u64 - 1;
        let ws = self.player.windows();
        let Some(w) = ws.last().filter(|w| w.get("plan").and_then(|p| p.as_u64()) == Some(n_plan)) else { return };
        let Some(name) = w.get("input").and_then(|x| x.as_str()) else { return };
        let Some(i) = nogood::step_index(&self.cur.steps, name) else { return };
        let label = match self.reading.by_name(name) {
            Some(a) => self.reading.record(a).label,
            None => nogood::label_of_name(name),
        };
        let doms = self.reading.domains();
        let Some(ban) = nogood::ban_of(&label, &start_f, &self.cur.paths, i, &|k| doms.get(k).is_some()) else { return };
        let mut mem = c.mem.lock().unwrap();
        let r = nogood::Refuted { entry: mem.entry, prefix, target: self.ng_target.clone(), ban, step: name.to_string(),
                                  frame: w.get("start").and_then(|f| f.as_u64()).unwrap_or(0), plan: n_plan as u32,
                                  why: w.get("end_reason").and_then(|x| x.as_str()).unwrap_or("").to_string() };
        let j = r.json();
        // E022 amendment (24): the try's own count of the label's refusals, whatever the state. Amendment (27): a window
        // whose children formed no input (the interface refused every candidate: a held object's Use waits for B, a
        // sentence an object re-issues) counts twice, so the label is banned at once and the next plan goes round it;
        // a second such window on it would stop the attempt ("the same empty window 2 times")
        let unformed = w.get("end_reason").and_then(|x| x.as_str()).is_some_and(|r| r.contains("no input formed"));
        *self.try_refused.entry(label.clone()).or_insert(0) += if unformed { 2 } else { 1 };
        if mem.refute(r) {
            drop(mem);
            (self.progress)(&format!("refuted at plan {n_plan}'s state: {label} ({} keys)", j["point"].as_array().map(|a| a.len()).unwrap_or(0)));
            if let Some(e) = self.plans.last_mut() {
                e.insert("nogood_refuted".into(), j);
            }
        }
    }

    fn observe(&mut self) -> Result<(), EngineError> {
        let ctx = PlanCtx {
            reading: &*self.reading,
            doms: self.reading.domains(),
            unobserved: &self.unobserved,
            goal: &self.goal,
            result: &self.cur,
            unobserved_actor_rooms: self.cfg.unobserved_actor_rooms,
            time_limit_s: self.cfg.time_limit_s,
        };
        crate::machine::port(|| self.player.observe(&ctx, &mut self.ps))
    }

    fn goal_holds(&self) -> bool {
        start::holds(&self.goal, &self.ps.now, &self.unobserved).is_empty()
    }

    fn checkpoint(&mut self) {
        crate::machine::section("checkpoint", || self.checkpoint_())
    }

    fn checkpoint_(&mut self) {
        let frame = self.ps.engine.status().frame;
        if self.ps.checkpoints.last().map(|c| c.frame != frame).unwrap_or(true) {
            let st = self.ps.engine.status().clone();
            let mut actors: Vec<(i32, i32, i32)> = self.ps.state.actors.iter().map(|a| (a.id, a.x, a.y)).collect();
            actors.sort();
            self.ps.checkpoints.push(Checkpoint { frame, facts: self.ps.obs.clone(), tick: st.tick, rng: st.rng, actors });
        }
    }

    fn set_pred(&mut self, pred: Facts) {
        let kept: Vec<(Key, Val)> = pred.0.iter().copied().filter(|(k, _)| start::has_prefix(*k, &self.unobserved)).collect();
        self.ps.kept = Facts(kept);
        self.ps.pred = pred;
    }

    fn stop(&mut self, why: String) {
        self.ps.stop_reason = Some(why);
    }

    /// The planner on one request. The planners' lock is held for the request only (an extension plans again); with
    /// FMI_PLANNER_LOCK=reading (E016 amendment (9)) only the reading's planner's lock is, and the cache's lock only to
    /// find or build the planner.
    fn plan_req(&self, req: &PlanRequest) -> Result<PlanResult, PlanError> {
        crate::machine::port(|| {
            let mut g = self.planners.lock().unwrap();
            let p = g.get(&*self.reading)?;
            if planner_lock_per_reading() {
                drop(g);
            }
            let mut p = p.lock().unwrap_or_else(|e| e.into_inner());
            p.plan(req)
        })
    }

    /// Harness.plan → _plan_taskcore (the taskcore planner, served): a plan from the engine's state; false when the
    /// planner gives none (the stop reason is set).
    fn plan(&mut self, first: bool) -> Result<bool, EngineError> {
        let rd = self.reading.clone();
        let doms: &Domains = rd.domains();
        let (mut start_f, counts) = if first {
            crate::machine::section("task_start", || (start::task_start(&mut self.b, &self.ps.obs, doms), BTreeMap::new()))
        } else {
            let pr = start::prediction(&self.ps.now, &self.ps.pred);
            crate::machine::section("replan_start", || start::replan_start(&mut self.b, &self.ps.obs, &pr, doms))
        };
        if self.cfg.unobserved_actor_rooms {
            start::actor_rooms_start(&mut start_f, doms, &self.ps.pred, first);
        }
        let mut counts = counts;
        let unmatched = crate::machine::section("args_of_slots", || args_of_slots(&mut start_f, doms, &self.ps.state, rd.dir(), &mut counts));
        let n = self.plans.len();
        let frame = self.ps.engine.status().frame;
        // E016 C: the memory's bans for this exact state and target (none without the switch)
        let (ng_prefix, bans) = match &self.nogood {
            Some(c) => {
                let p = nogood::prefix_key(&self.player.movie(), frame);
                let mut b = c.mem.lock().unwrap().bans(&p, &self.ng_target);
                // E022 amendment (24) (`FMI_BAN_TRY=1`): a step refused twice in this try is banned in every state
                // for the rest of it. A refusal's ban holds for the movie's exact prefix, and a window that played a
                // few frames moves the next plan's start past it: the dark room's try planned the same refused
                // stairway step four times, a reach extension each time
                if std::env::var("FMI_BAN_TRY").is_ok_and(|v| v == "1") {
                    for (l, n) in &self.try_refused {
                        let ban = fmi_core::Ban { label: l.clone(), facts: vec![] };
                        if *n >= 2 && !b.contains(&ban) {
                            b.push(ban);
                        }
                    }
                }
                (Some(p), b)
            }
            None => (None, Vec::new()),
        };
        let mut entry = Map::new();
        entry.insert("plan".into(), json!(n));
        entry.insert("frame".into(), json!(frame));
        entry.insert("bans".into(), json!(bans.len()));
        entry.insert("start_counts".into(), json!(counts));
        entry.insert("reorders".into(), json!([]));
        entry.insert("planner".into(), json!("taskcore"));
        entry.insert("search".into(), json!(self.cfg.search_name));
        // _same_input: the planner's input (the start, the bans) is the previous plan's on the same frame
        let key = crate::machine::section("input_key", || start::input_key(frame, bans.len(), &start_f));
        entry.insert("input_key".into(), json!(key));
        self.plans.push(entry);
        if let Some(prev) = n.checked_sub(1).map(|i| &self.plans[i]) {
            if prev.get("input_key").and_then(|k| k.as_str()) == Some(key.as_str()) {
                let pn = prev["plan"].clone();
                let e = self.plans.last_mut().unwrap();
                e.insert("status".into(), json!("same input"));
                e.insert("steps".into(), json!([]));
                self.stop(format!("replan at frame {frame} with plan {pn}'s start and bans and no frame played since: the same plan again"));
                return Ok(false);
            }
        }
        // E022 amendment (8): a live slot whose dumped locals match no list of the reading: the reading extended from
        // this start, with the live slots' locals (FMI_SLOT_LOCALS), as for a start value it did not reach
        if !unmatched.is_empty() && self.cfg.reach_extend && self.rereads.is_some() {
            let values: Vec<String> = unmatched.iter().map(|s| format!("locals of {s}")).collect();
            return self.extend(first, start_f, (values.len() as i64, values));
        }
        let refined = self.refinements.iter().any(|t| t["added"].as_array().is_some_and(|a| !a.is_empty()));
        if goal_fact_on() && refined {
            if let Some(k) = missing_goal_fact(&self.goal, doms) {
                let e = self.plans.last_mut().unwrap();
                e.insert("status".into(), json!("no target fact"));
                e.insert("steps".into(), json!([]));
                self.stop(goal_fact_stop(k));
                return Ok(false);
            }
        }
        // E016 C: the no-plan memory answers a request it has seen on this reading
        if let Some(ks) = self.ng_keys_or_all(&start_f) {
            let rpath = self.reading.dir().join("reading.json").to_string_lossy().into_owned();
            let sk = nogood::start_key(&start_f, &ks);
            let c = self.nogood.clone().unwrap();
            let mut mem = c.mem.lock().unwrap();
            if let Some(st) = mem.no_plan(&rpath, &sk, &self.ng_target).map(|x| x.status.clone()) {
                mem.used_no_plan += 1;
                drop(mem);
                let e = self.plans.last_mut().unwrap();
                e.insert("status".into(), json!(st));
                e.insert("nogood".into(), json!("no plan (memory)"));
                e.insert("steps".into(), json!([]));
                (self.progress)(&format!("plan {n}: {st} (the no-plan memory)"));
                self.cur = PlanResult { status: st.clone(), ..Default::default() };
                self.stop(status_stop(&st, self.cfg.plan_time_s));
                return Ok(false);
            }
        }
        if !bans.is_empty() {
            let e = self.plans.last_mut().unwrap();
            e.insert("nogood_bans".into(), json!(bans.iter().map(|b| json!([b.label, b.facts.len()])).collect::<Vec<_>>()));
            self.nogood.as_ref().unwrap().mem.lock().unwrap().used_bans += 1;
        }
        (self.progress)(&format!("planning through taskcore (plan {n}, frame {frame}, {} bans)", bans.len()));
        let pdir = self.cfg.workdir.join(format!("plan-{n:03}"));
        let mut caps = BTreeMap::new();
        if self.cfg.cost_caps {
            caps = start::cost_caps_of(&self.ps.state, self.cfg.cost_dmax.unwrap_or(1));
            let _ = start::write_text(&pdir.join("caps.txt"), &start::caps_text(&caps));
            let big: BTreeMap<&String, &i64> = caps.iter().filter(|(_, v)| **v > 1).collect();
            self.plans.last_mut().unwrap().insert("cost_caps".into(), json!(big));
        }
        if self.order.as_ref().map(|o| o.0 != rd.id()).unwrap_or(true) {
            self.order = Some((rd.id(), crate::machine::section("text_order", || start::text_order_of(rd.id(), doms))));
        }
        crate::machine::section("start.txt", || {
            let _ = start::write_text(&pdir.join("start.txt"), &start::start_text_in(&self.order.as_ref().unwrap().1, doms, &start_f));
        });
        // the deepening's limit until the attempt has a plan, then the old one
        let found = self.plans.iter().any(|p| p.get("status").and_then(|x| x.as_str()) == Some("plan"));
        let limit = match self.cfg.first_plan_time_s {
            Some(l) if !found => {
                self.plans.last_mut().unwrap().insert("limit_s".into(), json!(l));
                l
            }
            _ => self.cfg.plan_time_s,
        };
        let req = PlanRequest {
            goal: self.goal.clone(),
            start: start_f.clone(),
            bans,
            caps: caps.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            search: self.cfg.search,
            start_check: self.cfg.start_check.clone(),
            // the last plan's steps (the planner checks their suffixes from the new start); none at the first plan
            tail: if self.cfg.tail && !first && self.cur.status == "plan" {
                Some(self.cur.steps.iter().map(|st| st.name.clone()).collect())
            } else {
                None
            },
            time_limit_s: limit,
        };
        // E022 amendment (7) (FMI_PLAN_FOCUS=1): the goal's families' actions first, the whole reading when they fail
        let focused = if crate::focus::on() {
            crate::machine::section("focus", || crate::focus::focus(&*self.reading, &req.start, &req.goal))
        } else {
            None
        };
        let planned = match focused {
            Some(f) => {
                let mut fr = req.clone();
                fr.bans.extend(f.bans.iter().cloned());
                fr.time_limit_s = limit.min(crate::focus::limit_s());
                let r = self.plan_req(&fr);
                let (st, secs) = match &r {
                    Ok(x) => (x.status.clone(), x.seconds),
                    Err(_) => ("error".to_string(), 0.0),
                };
                (self.progress)(&format!("plan {n}: focus on {} actions of {:?}: {st}", f.actions, f.families));
                self.plans.last_mut().unwrap().insert(
                    "focus".into(),
                    json!({"families": f.families, "actions": f.actions, "rounds": f.rounds, "status": st, "seconds": secs}),
                );
                match r {
                    Ok(x) if x.status == "plan" => Ok(x),
                    _ => self.plan_req(&req),
                }
            }
            None => self.plan_req(&req),
        };
        let res = match planned {
            Ok(r) => r,
            Err(e) => {
                let (msg, choice) = match &e {
                    PlanError::PathChoice(m) => (m.clone(), true),
                    PlanError::Taskcore(m) => (m.clone(), false),
                };
                let miss = if choice { None } else { start::reach_misses(&msg) };
                if let (Some(miss), true) = (miss, self.cfg.reach_extend) {
                    return self.extend(first, start_f, miss);
                }
                let e = self.plans.last_mut().unwrap();
                e.insert("status".into(), json!(if choice { "path choice" } else { "taskcore error" }));
                e.insert("error".into(), json!(msg));
                e.insert("steps".into(), json!([]));
                self.stop(format!("planner: {msg}"));
                return Ok(false);
            }
        };
        crate::machine::section("record_plan", || self.record_plan(&res, &caps));
        if res.status == "out of domain" {
            let e = self.plans.last_mut().unwrap();
            e.insert("misses".into(), json!(res.misses.iter().map(|(k, v)| json!([k.text(), start::val_json(*v)])).collect::<Vec<_>>()));
            e.insert("steps".into(), json!([]));
            let nm = res.n_misses;
            self.cur = res;
            self.stop(format!("the engine's state is outside the task's domains ({nm} facts)"));
            return Ok(false);
        }
        (self.progress)(&format!("plan {n}: {}, {} steps, {:.0} s", res.status, res.steps.len(), res.seconds));
        if res.status != "plan" {
            // E016 C: a no-plan the planner proved, asked with no bans, is the reading's
            if nogood::proves_none(&res.status) && req.bans.is_empty() {
                if let Some(ks) = self.ng_keys_or_all(&start_f) {
                    let head = crate::warm::head(&*self.reading);
                    let np = nogood::NoPlan {
                        reading: self.reading.dir().join("reading.json").to_string_lossy().into_owned(),
                        scope: head["scope"]["rooms"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default(),
                        start: nogood::start_key(&start_f, &ks),
                        target: self.ng_target.clone(),
                        status: res.status.clone(),
                    };
                    self.nogood.as_ref().unwrap().mem.lock().unwrap().note_no_plan(np);
                }
            }
            let why = status_stop(&res.status, limit);
            self.cur = res;
            self.stop(why);
            return Ok(false);
        }
        if let Some(p) = ng_prefix {
            self.plan_at = Some((p, start_f.clone()));
        }
        // the plan's slot positions and unobserved facts at its start
        let mut pred = Vec::new();
        for (k, d) in &doms.0 {
            if start::has_prefix(*k, &self.predicted) {
                pred.push((*k, d.label(start_f.get(*k).unwrap_or(0))));
            }
        }
        self.set_pred(Facts(pred));
        self.cur = res;
        Ok(true)
    }

    /// The plan's entry from the planner's result (Harness._plan_taskcore's entry.update).
    fn record_plan(&mut self, res: &PlanResult, caps: &BTreeMap<String, i64>) {
        let stats: Value = pyjson::loads(&res.stats).unwrap_or(Value::Null);
        let e = self.plans.last_mut().unwrap();
        e.insert("costs".into(), json!(res.paths.iter().map(|a| start::capped_cost(&a.name, a.cost, caps)).collect::<Vec<_>>()));
        e.insert("plan_cost".into(), json!(res.plan_cost));
        e.insert("status".into(), json!(res.status));
        e.insert("seconds".into(), json!(res.seconds));
        e.insert("steps".into(), json!(res.steps.iter().map(|s| s.name.clone()).collect::<Vec<_>>()));
        e.insert("operators".into(), stats.get("sas").and_then(|s| s.get("operators")).cloned().unwrap_or(Value::Null));
        e.insert("actions".into(), stats.get("actions").cloned().unwrap_or(Value::Null));
        e.insert("folds".into(), json!(res.folds));
        e.insert("predicted_goal".into(), json!(res.predicted_goal));
        e.insert("file_facts".into(), json!(res.file_facts.len()));
        e.insert("needed_facts".into(), json!(res.needed_facts.len()));
        e.insert("input_check_misses".into(),
                 stats.get("start_check").and_then(|s| s.get("facts_the_input_check_stops_on")).cloned().unwrap_or(Value::Null));
        let mut ch = Map::new();
        for (i, ps) in &res.choices {
            ch.insert(i.to_string(), json!(ps.iter().map(|p| p.name.clone()).collect::<Vec<_>>()));
        }
        e.insert("choices".into(), Value::Object(ch));
        e.insert("source".into(), json!(res.source));
        e.insert("cpu_s".into(), json!(res.cpu_s));
        // the no-plan proofs' record, when they ran (FMI_PLAN_DEAD)
        if let Some(d) = stats.get("dead") {
            e.insert("dead".into(), d.clone());
        }
    }

    /// Harness._extend: a replan start the --reach reading does not cover; the reading extended, the plan made again.
    fn extend(&mut self, first: bool, start_f: Facts, miss: (i64, Vec<String>)) -> Result<bool, EngineError> {
        let (n, values) = miss;
        let frame = self.ps.engine.status().frame;
        {
            let e = self.plans.last_mut().unwrap();
            e.insert("status".into(), json!("reach extension"));
            e.insert("reach_misses".into(), json!(n));
            e.insert("reach_values".into(), json!(values));
            e.insert("steps".into(), json!([]));
            e.remove("input_key");
        }
        // E022 amendment (23) (`FMI_REACH_TRY_LIMIT=N`): at most N reach extensions in a try. A try keeps each reading it
        // made (the dark room's 6 GB each, 35 GB by the fifth); past the limit it stops, and the next try starts from its
        // end in a process of its own
        if let Some(lim) = std::env::var("FMI_REACH_TRY_LIMIT").ok().and_then(|v| v.parse::<usize>().ok()) {
            if self.extensions.len() >= lim {
                self.stop(format!("reach extension: the try's limit of {lim} reached at frame {frame}"));
                return Ok(false);
            }
        }
        let tries: Vec<&Value> = self.extensions.iter()
            .filter(|x| x["frame"].as_u64() == Some(frame) && x["replans"].as_u64() == Some(self.replans as u64)).collect();
        let last_sub = tries.last().map(|l| {
            let lv: Vec<String> = l["values"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
            values.iter().all(|v| lv.contains(v))
        }).unwrap_or(false);
        if tries.len() >= self.cfg.reach_extend_limit || last_sub {
            self.stop(format!("reach extension: the start at frame {frame} holds {n} values the extended reading did not reach either: {}",
                              files::py_repr(&json!(values.iter().take(5).collect::<Vec<_>>()))));
            return Ok(false);
        }
        (self.progress)(&format!("reach extension at frame {frame}: {n} values not reached"));
        let actions_before = self.reading.n_actions();
        let rr = self.rereads.as_mut().unwrap();
        let (cpu0, wall0) = (rr.cpu_s(), Instant::now());
        let rs_ = start::reach_start_text_full(&start_f, self.reading.domains());
        if let Some(s) = slot_locals_now(&self.ps.state) {
            rr.add_slot_locals(s);
        }
        let new = match crate::machine::port(|| rr.extend(&rs_)) {
            Ok(r) => r,
            Err(e) => {
                self.stop(format!("error: reach extension: {e}"));
                return Ok(false);
            }
        };
        let cpu = rr.cpu_s() - cpu0;
        let plan = self.plans.last().unwrap()["plan"].clone();
        let rec = json!({"plan": plan, "frame": frame, "replan": !first, "replans": self.replans, "n_values": n,
                         "values": values, "seconds": pyjson::round(wall0.elapsed().as_secs_f64(), 2),
                         "cpu_s": pyjson::round(cpu, 2), "actions_before": actions_before, "actions": new.n_actions(),
                         "reading": new.dir().join("reading.json").to_string_lossy()});
        self.extensions.push(rec);
        self.swap(new);
        self.plan(first)
    }

    /// Harness._swap: the task of a reading made again (the packed file, the points, the tracked facts).
    fn swap(&mut self, new: Arc<dyn ReadingSource>) {
        let head = crate::warm::head(&*new);
        self.b = Points::of(new.domains(), head.get("live_locals").filter(|v| !v.is_null()).cloned());
        self.reading = new;
        let rd = self.reading.clone();
        self.ps.now = start::view(&mut self.b, rd.domains(), &self.ps.obs, &self.unobserved);
    }

    /// Harness._refine_turn: E007's loop after a window that ended a plan's play. False when the run stops.
    fn refine_turn(&mut self, stopped: bool) -> bool {
        crate::machine::section("refine_turn", || self.refine_turn_(stopped))
    }

    fn refine_turn_(&mut self, stopped: bool) -> bool {
        let d = self.player.replan_cause_detail().unwrap_or(Value::Null);
        let cause = d.get("cause").cloned().unwrap_or(Value::Null);
        let cause_obj = if cause.is_object() { cause.clone() } else { json!({}) };
        let mut new = start::refinable(Some(&cause_obj));
        if new.is_empty() && start::refine_checks_on() {
            // E022 amendment (27): a refuted check on a variable the reading holds no fact of
            let rec = cause_obj.get("record").and_then(|r| r.as_str()).and_then(|r| self.reading.by_name(r));
            if let Some(i) = rec {
                let line: Value = serde_json::from_str(self.reading.action_line(i)).unwrap_or(Value::Null);
                new = start::check_refinable(&cause_obj, &line);
            }
        }
        let short = if files::truthy(Some(&cause)) { Value::from(self.player.short(&cause)) } else { Value::Null };
        let what = d.get("what").filter(|w| files::truthy(Some(w))).cloned()
            .unwrap_or_else(|| self.player.replan_cause().map(Value::from).unwrap_or(Value::Null));
        let mut turn = json!({"plan": self.plans.len() as i64 - 1, "frame": self.ps.engine.status().frame,
                              "qf": self.ps.engine.status().qf, "what": what, "cause": short,
                              "class": cause_obj.get("class").cloned().unwrap_or(Value::Null),
                              "values": cause_obj.get("values").and_then(|v| v.as_array()).cloned().unwrap_or_default().iter()
                                  .map(|v| json!({"what": v.get("what"), "value": v.get("value"), "kind": v.get("kind")})).collect::<Vec<_>>(),
                              "added": []});
        let again: Vec<i64> = new.iter().copied().filter(|n| self.refined.iter().any(|(r, _)| r == n)).collect();
        if let Some(&a) = again.first() {
            let at = self.refined.iter().find(|(r, _)| *r == a).unwrap().1;
            turn["why_not"] = json!(format!("named again: var {a}, added at plan {at}"));
            let reason = format!("refinement: var {a} added at plan {at} is named again; {}", short.as_str().unwrap_or(""));
            let sr = if !stopped { reason } else { format!("{}; {reason}", self.ps.stop_reason.clone().unwrap_or("None".into())) };
            self.refinements.push(turn);
            self.stop(sr);
            return false;
        }
        if new.is_empty() {
            turn["why_not"] = json!(if !files::truthy(Some(&cause)) || !files::truthy(cause_obj.get("cause")) {
                "no cause".to_string()
            } else if cause_obj.get("class").and_then(|c| c.as_str()) != Some("split") {
                format!("class {}", files::py_str(cause_obj.get("class")))
            } else {
                "a split on no variable the reading holds no fact of".to_string()
            });
            self.refinements.push(turn);
            return true;
        }
        if stopped {
            turn["why_not"] = json!("the run had stopped");
            self.refinements.push(turn);
            return false;
        }
        // the replan's start, with the engine's values of the fact variables (the new reading's seeds, Lemma R (4))
        let rd = self.reading.clone();
        let pr = start::prediction(&self.ps.now, &self.ps.pred);
        let (mut st, _) = start::replan_start(&mut self.b, &self.ps.obs, &pr, rd.domains());
        let mut vars: Vec<i64> = self.refined.iter().map(|(r, _)| *r).collect();
        vars.extend(new.iter().copied());
        for n in &vars {
            st.set(Key::var(*n as u64), self.ps.obs.get(Key::var(*n as u64)).unwrap_or(0));
        }
        let mut ev = Map::new();
        for n in &new {
            ev.insert(n.to_string(), json!(self.ps.obs.get(Key::var(*n as u64)).unwrap_or(0)));
        }
        let head = crate::warm::head(&*self.reading);
        let folded = files::as_i64(head.get("folded")).unwrap_or(0);
        turn["added"] = json!(new);
        turn["engine_values"] = Value::Object(ev);
        turn["actions_before"] = json!(self.reading.n_actions());
        turn["records_before"] = json!(self.reading.n_actions() as i64 + folded);
        turn["operators_before"] = self.plans.last().and_then(|p| p.get("operators")).cloned().unwrap_or(Value::Null);
        (self.progress)(&format!("refinement at frame {}: var {:?} made facts", self.ps.engine.status().frame, new));
        let old_id = self.reading.id();
        // the old reading's planner: its memory, while the reader reads (plan_fd.close_servers). Kept (FMI_PLANNER_KEEP,
        // on) while MemAvailable holds the explorer's level for a second heavy job (E008_MEM_SECOND_GB): a planner is a
        // function of its reading, so the next request on that reading finds it warm instead of building it again
        if !planner_keep() {
            self.planners.lock().unwrap().close(&old_id);
        }
        let rr = self.rereads.as_mut().unwrap();
        let (cpu0, wall0) = (rr.cpu_s(), Instant::now());
        let rs_ = start::reach_start_text_full(&st, self.reading.domains());
        if let Some(s) = slot_locals_now(&self.ps.state) {
            rr.add_slot_locals(s);
        }
        let mut widened = 0;
        let (reading, doc, rep) = loop {
            let reading = match crate::machine::port(|| rr.refine(&new, &rs_)) {
                Ok(r) => r,
                Err(e) => {
                    self.refinements.push(turn);
                    self.stop(format!("error: refinement: {e}"));
                    return false;
                }
            };
            let doc = crate::warm::head(&*reading);
            let rep: Vec<Value> = doc.get("fact_vars_report").and_then(|r| r.as_array()).cloned().unwrap_or_default();
            // E022 (FMI_REFINE_WIDEN): the values written outside a new fact variable's domain seed it, and the scope
            // is read again (Lemma R's premise R0 then holds, or the try stops as before)
            let outside: Vec<(i64, Vec<i64>)> = new
                .iter()
                .filter_map(|n| {
                    let r = rep.iter().find(|x| x.get("var").and_then(|v| v.as_i64()) == Some(*n))?;
                    let vs: Vec<i64> = r.get("outside_after_last_pass")?.as_array()?.iter().filter_map(|v| v.as_i64()).collect();
                    (!vs.is_empty()).then_some((*n, vs))
                })
                .collect();
            if outside.is_empty() || widened >= refine_widen() {
                break (reading, doc, rep);
            }
            widened += 1;
            let mut ok = true;
            for (n, vs) in &outside {
                for v in vs {
                    if rr.add_start(&start_with(&rs_, &format!("var:{n}"), *v)).is_err() {
                        ok = false;
                    }
                }
            }
            if !ok {
                break (reading, doc, rep);
            }
            turn["widened"] = json!(widened);
        };
        let cpu = rr.cpu_s() - cpu0;
        let rep_of = |n: i64| rep.iter().find(|x| x.get("var").and_then(|v| v.as_i64()) == Some(n)).cloned().unwrap_or(json!({}));
        let rs = doc.get("reader_scope").cloned().unwrap_or(json!({}));
        turn["seconds"] = json!(pyjson::round(wall0.elapsed().as_secs_f64(), 2));
        turn["cpu_s"] = json!(pyjson::round(cpu, 2));
        turn["reader"] = json!({"read_wall_s": rs.get("read_wall_s"), "read_cpu_s": rs.get("read_cpu_s"), "peak_rss_mb": rs.get("peak_rss_mb")});
        turn["actions"] = json!(reading.n_actions());
        turn["records"] = doc.get("n_records").cloned().unwrap_or(Value::Null);
        turn["fact_vars"] = doc.get("fact_vars").cloned().unwrap_or(Value::Null);
        let mut fm = Map::new();
        for n in &new {
            let r = rep_of(*n);
            fm.insert(n.to_string(), json!({"what": r.get("what"), "domain": r.get("domain").and_then(|d| d.as_array()).map(|a| a.len()).unwrap_or(0),
                                            "outside_after_last_pass": r.get("outside_after_last_pass")}));
        }
        turn["facts"] = Value::Object(fm);
        turn["reading"] = json!(reading.dir().join("reading.json").to_string_lossy());
        let plan_n = turn["plan"].as_i64().unwrap_or(0) as usize;
        for n in &new {
            self.refined.push((*n, plan_n));
        }
        let outside: Vec<i64> = new.iter().copied().filter(|n| files::truthy(rep_of(*n).get("outside_after_last_pass"))).collect();
        let added: Vec<i64> = new.iter().copied().filter(|n| rep_of(*n).get("what").and_then(|w| w.as_str()) == Some("added")).collect();
        if !outside.is_empty() || added.len() < new.len() {
            let why = if !outside.is_empty() {
                format!("Lemma R's premise R0: the last pass wrote values of var {} outside the domain", files::py_repr(&json!(outside)))
            } else {
                let missing: Vec<i64> = new.iter().copied().filter(|n| !added.contains(n)).collect();
                format!("the reader did not make var {} a fact", files::py_repr(&json!(missing)))
            };
            turn["why_not"] = json!(why);
            self.refinements.push(turn);
            self.stop(format!("refinement: {why}"));
            return false;
        }
        let k = self.refinements.iter().filter(|t| t["added"].as_array().map(|a| !a.is_empty()).unwrap_or(false)).count() + 1;
        (self.progress)(&format!("refinement {k}: {} -> {} actions", turn["actions_before"], turn["actions"]));
        self.refinements.push(turn);
        self.swap(reading);
        true
    }

    fn play_ctx_play(&mut self) -> PlayOutcome {
        let n = self.plans.len() as u32 - 1;
        let ctx = PlanCtx {
            reading: &*self.reading,
            doms: self.reading.domains(),
            unobserved: &self.unobserved,
            goal: &self.goal,
            result: &self.cur,
            unobserved_actor_rooms: self.cfg.unobserved_actor_rooms,
            time_limit_s: self.cfg.time_limit_s,
        };
        let how = crate::machine::port(|| self.player.play_plan(&ctx, &mut self.ps, n));
        if let Some(notes) = self.player.plan_notes() {
            if let (Some(m), Some(e)) = (notes.as_object(), self.plans.last_mut()) {
                for (k, v) in m {
                    e.insert(k.clone(), v.clone());
                }
            }
        }
        how
    }

    /// Harness._run_windows: plan, play the plan's windows, replan or refine, until a stop. The record.
    pub fn run(&mut self) -> Value {
        self.t0 = Instant::now();
        let mut error: Option<String> = None;
        let mut causes: Vec<Value> = Vec::new();
        let r = (|| -> Result<(), EngineError> {
            self.observe()?;
            let st = self.ps.engine.status().clone();
            self.frame0 = st.frame;
            self.tick0 = st.tick;
            self.qf0 = st.qf;
            self.checkpoint();
            let mut planned = self.plan(true)?;
            loop {
                if self.goal_holds() {
                    let why = if self.player.goal_credited() { "goal".to_string() } else { "goal, not from the plan: it held at a plan's start".to_string() };
                    self.stop(why);
                    break;
                }
                if !planned && self.ps.stop_reason.is_some() {
                    break;
                }
                if self.t0.elapsed().as_secs_f64() > self.cfg.time_limit_s {
                    self.stop("time limit".into());
                    break;
                }
                let how = self.play_ctx_play();
                if self.nogood.is_some() {
                    self.note_refuted();
                }
                match how {
                    PlayOutcome::Goal => {
                        self.stop("goal".into());
                        break;
                    }
                    PlayOutcome::Stop => {
                        if self.cfg.refine {
                            self.refine_turn(true);
                        }
                        break;
                    }
                    PlayOutcome::Replan => {}
                }
                causes.push(json!({"plan": self.plans.len() as i64 - 1, "frame": self.ps.engine.status().frame,
                                   "cause": self.player.replan_cause()}));
                if self.cfg.refine && !self.refine_turn(false) {
                    break;
                }
                if self.replans >= self.cfg.replan_limit {
                    self.stop("replan limit".into());
                    break;
                }
                if self.t0.elapsed().as_secs_f64() > self.cfg.time_limit_s {
                    self.stop("time limit".into());
                    break;
                }
                self.replans += 1;
                planned = self.plan(false)?;
            }
            Ok(())
        })();
        if let Err(e) = r {
            self.stop(engine_stop(&e));
        }
        crate::machine::port(|| self.player.flush(&mut self.ps));
        match self.observe() {
            Ok(()) => self.checkpoint(),
            Err(e) => error = Some(format!("{}\nthe last observation failed: {e}", error.unwrap_or_default())),
        }
        // every checkpoint is a state the inputs from boot reach; verify reads them in frame order
        let mut cps = std::mem::take(&mut self.ps.checkpoints);
        cps.sort_by_key(|c| c.frame);
        cps.dedup_by_key(|c| c.frame);
        self.ps.checkpoints = cps;
        let mut out = self.result();
        let st = self.ps.engine.status().clone();
        out["goal_from_the_plan"] = json!(self.ps.stop_reason.as_deref() == Some("goal"));
        out["qf"] = json!(st.qf as i64 - self.qf0 as i64);
        out["qf0"] = json!(self.qf0);
        out["qf_end"] = json!(st.qf);
        out["replan_causes"] = json!(causes);
        out["windows"] = json!(crate::machine::port(|| self.player.windows()));
        out["window_summary"] = crate::machine::port(|| self.player.summary(&self.ps));
        if let Some(e) = error {
            out["error"] = json!(e);
        }
        out
    }

    /// Harness.result: the record of the run so far, from what the harness holds.
    pub fn result(&self) -> Value {
        crate::machine::section("result", || self.result_())
    }

    fn result_(&self) -> Value {
        let st = self.ps.engine.status();
        let mut out = json!({
            "stop": self.ps.stop_reason, "goal": self.goal_holds(), "replans": self.replans, "bans": [],
            "plans": self.plans.iter().map(|p| Value::Object(p.clone())).collect::<Vec<_>>(), "steps": [],
            "frames": st.frame as i64 - self.frame0 as i64, "ticks": st.tick as i64 - self.tick0 as i64,
            "frame0": self.frame0, "frame_end": st.frame, "seconds": self.t0.elapsed().as_secs_f64(),
            "inputs": self.player.movie().iter().map(events::event_json).collect::<Vec<_>>(),
        });
        if crate::mech::goal_end() {
            // E014 (FMI_GOAL_END): the end status's tick, for an end entry's checkpoint
            out["tick_end"] = json!(st.tick);
        }
        if self.cfg.reach_extend {
            out["extensions"] = json!(self.extensions);
        }
        if self.cfg.refine {
            out["refinements"] = json!(self.refinements);
        }
        out
    }

    /// The checkpoints as the archive's entries hold them: [frame, facts.py map, extra_of].
    pub fn checkpoints_json(&self) -> Vec<Value> {
        self.ps.checkpoints.iter().map(checkpoint_json).collect()
    }

    pub fn into_parts(self) -> (PlayState, Arc<dyn ReadingSource>) {
        (self.ps, self.reading)
    }

    /// `into_parts` with the window search's player, whose end the caller decides (`PlayerHost::done`).
    pub fn into_all(self) -> (PlayState, Arc<dyn ReadingSource>, Box<dyn WindowPlayer>) {
        (self.ps, self.reading, self.player)
    }
}

/// A checkpoint as the archive's files write it: [frame, facts (sorted by key text), {"tick", "rng", "actors"}].
pub fn checkpoint_json(c: &Checkpoint) -> Value {
    json!([c.frame, start::facts_json(&c.facts), {"tick": c.tick, "rng": c.rng,
           "actors": c.actors.iter().map(|(i, x, y)| json!([i, x, y])).collect::<Vec<_>>()}])
}

/// replay.extra_of from a dump and its status.
pub fn extra_of(state: &Obs, tick: u64, rng: u64) -> Value {
    let mut actors: Vec<(i32, i32, i32)> = state.actors.iter().map(|a| (a.id, a.x, a.y)).collect();
    actors.sort();
    json!({"tick": tick, "rng": rng, "actors": actors.iter().map(|(i, x, y)| json!([i, x, y])).collect::<Vec<_>>()})
}

/// The facts of a dump, as the harness's checkpoints keep them.
pub fn obs_facts(state: &Obs) -> Facts {
    facts_of(state)
}

#[allow(dead_code)]
fn _k(_: u8) -> u8 {
    K_ACTORROOM
}

#[cfg(test)]
mod widen_tests {
    use super::start_with;

    #[test]
    fn a_start_with_a_value_replaces_the_key_and_keeps_the_rest() {
        let t = "room\t49\nvar:165\t0\nbit:5\t1\n";
        let s = start_with(t, "var:165", 122);
        assert_eq!(s, "room\t49\nbit:5\t1\nvar:165\t122\n");
        assert_eq!(start_with("room\t49\n", "var:7", 3), "room\t49\nvar:7\t3\n");
    }
}
