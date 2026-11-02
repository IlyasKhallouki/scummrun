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
use crate::ports::WindowPlayer;
use crate::pyjson;
use crate::start::{self, Points};
use fmi_core::{
    facts_of, Checkpoint, Domains, EngineError, Facts, Key, Obs, PlanCtx, PlanError, PlanRequest, PlanResult, PlayOutcome,
    PlayState, Planner, PlannerHost, ReadingSource, Search, Val, K_ACTORROOM,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The harness's settings (step.py's attempt wiring).
#[derive(Clone, Debug)]
pub struct HarnessCfg {
    pub replan_limit: u32,
    pub time_limit_s: f64,
    pub plan_time_s: u64,
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
    /// The reading's scope read again from every start so far and this one (reader-scope --reach-also).
    fn extend(&mut self, start: &[(String, i64)]) -> Result<Arc<dyn ReadingSource>, String>;
    /// The scope read again with the variables `new` made facts too, from every start so far and this one.
    fn refine(&mut self, new: &[i64], start: &[(String, i64)]) -> Result<Arc<dyn ReadingSource>, String>;
    /// CPU seconds of the readers so far (the extension's and refinement's records)
    fn cpu_s(&self) -> f64;
}

/// The warm planner of each reading (one per reading id), kept across plans and attempts.
pub struct PlannerCache {
    pub host: Arc<dyn PlannerHost>,
    kept: HashMap<[u8; 20], Box<dyn Planner>>,
}

impl PlannerCache {
    pub fn new(host: Arc<dyn PlannerHost>) -> PlannerCache {
        PlannerCache { host, kept: HashMap::new() }
    }
    pub fn get(&mut self, rd: &dyn ReadingSource) -> Result<&mut Box<dyn Planner>, PlanError> {
        let id = rd.id();
        if !self.kept.contains_key(&id) {
            let p = self.host.planner(rd)?;
            self.kept.insert(id, p);
        }
        Ok(self.kept.get_mut(&id).unwrap())
    }
    /// plan_fd.close_servers: the planners' memory, before a reader reads
    pub fn close(&mut self, id: &[u8; 20]) {
        self.kept.remove(id);
    }
    pub fn close_all(&mut self) {
        self.kept.clear();
    }
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
    order: Option<([u8; 20], Vec<(std::rc::Rc<str>, Key)>)>,
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
            refinements: Vec::new(),
            refined: Vec::new(),
            frame0: 0,
            tick0: 0,
            qf0: 0,
            t0: Instant::now(),
            progress,
            order: None,
        })
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
        let n = self.plans.len();
        let frame = self.ps.engine.status().frame;
        let mut entry = Map::new();
        entry.insert("plan".into(), json!(n));
        entry.insert("frame".into(), json!(frame));
        entry.insert("bans".into(), json!(0));
        entry.insert("start_counts".into(), json!(counts));
        entry.insert("reorders".into(), json!([]));
        entry.insert("planner".into(), json!("taskcore"));
        entry.insert("search".into(), json!(self.cfg.search_name));
        // _same_input: the planner's input (the start, the bans) is the previous plan's on the same frame
        let key = crate::machine::section("input_key", || start::input_key(frame, 0, &start_f));
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
        (self.progress)(&format!("planning through taskcore (plan {n}, frame {frame}, 0 bans)"));
        let pdir = self.cfg.workdir.join(format!("plan-{n:03}"));
        let mut caps = BTreeMap::new();
        if self.cfg.cost_caps {
            caps = start::cost_caps_of(&self.ps.state, self.cfg.cost_dmax.unwrap_or(1));
            let _ = start::write_text(&pdir.join("caps.txt"), &start::caps_text(&caps));
            let big: BTreeMap<&String, &i64> = caps.iter().filter(|(_, v)| **v > 1).collect();
            self.plans.last_mut().unwrap().insert("cost_caps".into(), json!(big));
        }
        if self.order.as_ref().map(|o| o.0 != rd.id()).unwrap_or(true) {
            self.order = Some((rd.id(), crate::machine::section("text_order", || start::text_order(doms))));
        }
        crate::machine::section("start.txt", || {
            let _ = start::write_text(&pdir.join("start.txt"), &start::start_text_in(&self.order.as_ref().unwrap().1, doms, &start_f));
        });
        let req = PlanRequest {
            goal: self.goal.clone(),
            start: start_f.clone(),
            bans: vec![],
            caps: caps.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            search: self.cfg.search,
            start_check: self.cfg.start_check.clone(),
            // the last plan's steps (the planner checks their suffixes from the new start); none at the first plan
            tail: if self.cfg.tail && !first && self.cur.status == "plan" {
                Some(self.cur.steps.iter().map(|st| st.name.clone()).collect())
            } else {
                None
            },
            time_limit_s: self.cfg.plan_time_s,
        };
        // the planners' lock is held for the request only (an extension below plans again)
        let planned = {
            let mut g = self.planners.lock().unwrap();
            crate::machine::port(|| g.get(&*self.reading).and_then(|p| p.plan(&req)))
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
            let why = match res.status.as_str() {
                "time" => format!("planner timeout ({} s)", self.cfg.plan_time_s),
                "unsolvable" => "no plan (the planner proved none)".into(),
                "incomplete" => "no plan (the search ended without one)".into(),
                "memory" => "planner memory limit (3G)".into(),
                s => format!("planner {s}"),
            };
            self.cur = res;
            self.stop(why);
            return Ok(false);
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
        let rs_ = start::reach_start(&start_f);
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
        let new = start::refinable(Some(&cause_obj));
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
        self.planners.lock().unwrap().close(&old_id); // the old reading's planner: its memory, while the reader reads
        let rr = self.rereads.as_mut().unwrap();
        let (cpu0, wall0) = (rr.cpu_s(), Instant::now());
        let rs_ = start::reach_start(&st);
        let reading = match crate::machine::port(|| rr.refine(&new, &rs_)) {
            Ok(r) => r,
            Err(e) => {
                self.refinements.push(turn);
                self.stop(format!("error: refinement: {e}"));
                return false;
            }
        };
        let cpu = rr.cpu_s() - cpu0;
        let doc = crate::warm::head(&*reading);
        let rep: Vec<Value> = doc.get("fact_vars_report").and_then(|r| r.as_array()).cloned().unwrap_or_default();
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
