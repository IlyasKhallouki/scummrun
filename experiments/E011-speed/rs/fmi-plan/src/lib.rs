//! fmi-plan: E011's planner port (experiments/E011-speed/plan.md, component `plan`): `fmi_core::PlannerHost` and
//! `Planner` over a `ReadingSource`, in place of plan_fd.py's `plan_taskcore` with `serve=True`.
//!
//! One `WarmPlanner` per reading holds taskcore's warm task (taskcore/src/warm.rs: the packed file loaded once, the
//! work that reads no start kept per (bans, goal, caps)). A request (`Planner::plan`):
//!
//! 1. **the plan cache** (taskcore cache.rs, F4): the request's key, looked up in memory, then in the store on disk;
//!    a hit is the stored result (`source` "cache");
//! 2. **the old tail** (taskcore tail.rs), when the request gives one: the previous plan's remaining steps, checked as a
//!    plan of the input task from this start; accepted, they are the plan (`source` "tail");
//! 3. **the warm request** to the reduced task (`sas`'s, byte for byte), the SAS task built in memory
//!    (search.rs `from_task`), and the search: `Gbfs`, the Rust search (Fast Downward's lazy greedy h_FF with plusone
//!    costs, the same plans), or `FdLamaFirstCosts`, Fast Downward's own search binary on the SAS text, as E009 runs
//!    it (the differential tests' reference);
//! 4. every plan checked from the start on the SAS task (search.rs `check_plan`), read back (decode.rs: the plan's
//!    rows only), its paths chosen and its states predicted (predict.rs: plan_fd's `predict` with the engine's
//!    choices), the actions cut to the facts (e) needs: plan_fd's `TaskcoreResult` as fmi-core's `PlanResult`.
//!
//! Errors are plan_fd's: a start outside a needed domain is the status "out of domain" with its misses; a step whose
//! paths do not choose one is `PlanError::PathChoice`; anything taskcore stops on is `PlanError::Taskcore` with
//! "taskcore: " and its message (the reached check's message is what `reach_misses` reads).
//!
//! Only E009's taskcore settings are run (strict fold, every reduction, PRODUCT_LIMIT 64, the model encoding, no pick
//! windows): the port has no other.

#[cfg(feature = "file-reading")]
pub mod double;
pub mod json;
pub mod raw;

use fmi_core::{
    val_text, Action, Ban, Domains, Facts, Key, PlanError, PlanRequest, PlanResult, PlanStep, Planner, PlannerHost,
    ReadingSource, Search, Val, NONE,
};
use raw::{value_of, Raw};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use taskcore::{cache, decode, predict, sas, search, tail, warm};

/// plan_fd.LAMA_FIRST_COSTS: lama-first's greedy search with costs, the landmark dump on (E009's search).
pub const LAMA_FIRST_COSTS: &str = "let(hlm,eval_modify_costs(landmark_sum(lm_factory=lm_reasonable_orders_hps(lm_rhw(),verbosity=debug),pref=false),cost_type=plusone),let(hff,eval_modify_costs(ff(),cost_type=plusone),lazy_greedy([hff,hlm],preferred=[hff,hlm],reopen_closed=false)))";

/// Makes the warm planners (`PlannerHost`).
#[derive(Clone, Debug)]
pub struct Host {
    /// the plan cache (in memory, and in `cache_dir`); false: every request plans (the old behaviour)
    pub cache: bool,
    /// the plan store on disk (F4); None: in memory only
    pub cache_dir: Option<PathBuf>,
    /// how many (bans, goal, caps) each planner keeps warm
    pub keep: usize,
    /// Fast Downward's search binary (`Search::FdLamaFirstCosts`)
    pub downward: PathBuf,
    /// where Fast Downward's files go
    pub work: PathBuf,
}

impl Host {
    /// The host E011 runs: the plan cache on (FMI_PLAN_NO_CACHE set: off), its store `~/.cache/fast-mi-e011/plans`
    /// (FMI_PLAN_CACHE, "" for none), Fast Downward from DOWNWARD_DIR.
    pub fn from_env() -> Host {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let cache_dir = match std::env::var("FMI_PLAN_CACHE") {
            Ok(s) if s.is_empty() => None,
            Ok(s) => Some(PathBuf::from(s)),
            Err(_) => Some(PathBuf::from(&home).join(".cache/fast-mi-e011/plans")),
        };
        let dw = std::env::var("DOWNWARD_DIR").unwrap_or_else(|_| "/home/ilyask/projects/fast-mi/third_party/downward".into());
        Host {
            cache: std::env::var("FMI_PLAN_NO_CACHE").is_err(),
            cache_dir,
            keep: 4,
            downward: PathBuf::from(dw).join("builds/release/bin/downward"),
            work: std::env::temp_dir().join("fmi-plan"),
        }
    }
}

impl PlannerHost for Host {
    fn planner(&self, reading: &dyn ReadingSource) -> Result<Box<dyn Planner>, PlanError> {
        Ok(Box::new(WarmPlanner::new(self, reading)?))
    }
}

/// Seconds of one request's parts (the last request's, for the bench).
#[derive(Clone, Debug, Default)]
pub struct Timing {
    pub key: f64,
    pub tail: f64,
    pub reduce: f64,
    pub build: f64,
    pub relax: f64,
    pub search: f64,
    pub check: f64,
    pub decode: f64,
    pub predict: f64,
    pub store: f64,
    pub total: f64,
    pub cpu: f64,
    pub expanded: u64,
    pub evaluated: u64,
    pub operators: usize,
    pub unary: usize,
    pub kept: String,
}

/// Cost caps as taskcore reads them: (label prefix, frames).
type Caps = Vec<(String, u32)>;

/// One reading's planner (module docstring).
pub struct WarmPlanner {
    host: Host,
    warm: warm::Warm,
    id: [u8; 20],
    domains: Domains,
    raw: Raw,
    by_label: Option<HashMap<u32, Vec<u32>>>,
    memo: HashMap<[u8; 32], PlanResult>,
    lowered_memo: HashMap<Caps, Vec<(u32, u32)>>,
    /// the reading's facts sorted by their key texts (the start file's order): (text, index in `domains`)
    sorted_facts: Vec<(String, usize)>,
    n_fd: u64,
    pub last: Timing,
}

fn tc_err(e: String) -> PlanError {
    PlanError::Taskcore(format!("taskcore: {}", e))
}

/// CPU seconds of this process (utime + stime), all threads.
pub fn cpu_s() -> f64 {
    taskcore::cli::cpu_s()
}

/// The text of a start value as export_records `_label_text` writes it: "" where the domain lacks it.
fn label_text(d: &fmi_core::Domain, v: Val) -> String {
    let lab = predict::label(d, v);
    if lab == NONE || !d.values.contains(&lab) {
        String::new()
    } else {
        val_text(lab)
    }
}

/// taskcore's `--goal` text of a goal.
pub fn goal_text(goal: &[(Key, Vec<Val>)]) -> String {
    goal.iter()
        .map(|(k, vs)| format!("{}={}", k.text(), vs.iter().map(|&v| val_text(v)).collect::<Vec<_>>().join("|")))
        .collect::<Vec<_>>()
        .join(",")
}

/// "N facts out of their domains: [..]" (taskcore's message): N and the keys it names.
fn out_of_domain(msg: &str) -> Option<(u64, Vec<String>)> {
    let i = msg.find(" facts out of their domains: [")?;
    let n: u64 = msg[..i].rsplit(|c: char| !c.is_ascii_digit()).next()?.parse().ok()?;
    let rest = &msg[i + " facts out of their domains: [".len()..];
    let mut keys = Vec::new();
    let b = rest.as_bytes();
    let mut p = 0;
    while p < b.len() && b[p] != b']' {
        if b[p] == b'"' {
            p += 1;
            let mut s = String::new();
            while p < b.len() && b[p] != b'"' {
                if b[p] == b'\\' && p + 1 < b.len() {
                    p += 1;
                }
                s.push(b[p] as char);
                p += 1;
            }
            keys.push(s);
        }
        p += 1;
    }
    Some((n, keys))
}

impl WarmPlanner {
    pub fn new(host: &Host, reading: &dyn ReadingSource) -> Result<WarmPlanner, PlanError> {
        let head = fmi_plan_head(reading.head()).map_err(tc_err)?;
        let reached = if reading.reached().is_empty() { None } else { Some(reading.reached().to_string()) };
        let cfg = warm::Config { keep: host.keep, ..Default::default() };
        let path = reading.dir().join(&head);
        let w = warm::Warm::new(reading.packed(), &path.display().to_string(), reached, cfg).map_err(tc_err)?;
        let t = w.task();
        let raw = Raw::new(&path, reading.packed(), &t.strs.list, &t.facts.key).map_err(tc_err)?;
        let domains = reading.domains().clone();
        let mut sorted_facts: Vec<(String, usize)> = domains.0.iter().enumerate().map(|(i, (k, _))| (k.text(), i)).collect();
        sorted_facts.sort();
        Ok(WarmPlanner {
            host: host.clone(),
            id: reading.id(),
            sorted_facts,
            domains,
            raw,
            warm: w,
            by_label: None,
            memo: HashMap::new(),
            lowered_memo: HashMap::new(),
            n_fd: 0,
            last: Timing::default(),
        })
    }

    /// The start file's lines of a start (export_records `write_start`), sorted by key.
    pub fn start_lines(&self, start: &Facts) -> Vec<(String, String)> {
        self.sorted_facts
            .iter()
            .map(|(t, i)| {
                let (k, d) = &self.domains.0[*i];
                (t.clone(), label_text(d, start.val(*k)))
            })
            .collect()
    }

    fn bans_of(bans: &[Ban]) -> Vec<taskcore::bans::Ban> {
        bans.iter()
            .map(|b| taskcore::bans::Ban {
                label: b.label.clone(),
                point: b.facts.iter().map(|(k, v)| (k.text(), val_text(*v))).collect(),
            })
            .collect()
    }

    fn caps_of(caps: &[(String, i64)]) -> Vec<(String, u32)> {
        caps.iter().map(|(p, c)| (p.clone(), (*c).clamp(1, u32::MAX as i64) as u32)).collect()
    }

    fn lowered(&mut self, caps: &[(String, u32)]) -> Vec<(u32, u32)> {
        if let Some(l) = self.lowered_memo.get(caps) {
            return l.clone();
        }
        let l = self.warm.lowered_by(caps);
        self.lowered_memo.insert(caps.to_vec(), l.clone());
        l
    }

    fn label_of(&self, m: u32) -> String {
        let t = self.warm.task();
        t.strs.get(t.acts.meta[m as usize].label).to_string()
    }

    /// Every path of every input action of `label`, under `bans` (the tail check's candidates).
    fn paths_of_label(&mut self, label: &str, bans: &[Ban]) -> Result<Vec<Action>, String> {
        if self.by_label.is_none() {
            let t = self.warm.task();
            let mut by: HashMap<u32, Vec<u32>> = HashMap::new();
            for (i, m) in t.acts.meta.iter().enumerate() {
                by.entry(m.label).or_default().push(i as u32);
            }
            self.by_label = Some(by);
        }
        let Some(id) = self.warm.task().strs.find(label) else { return Ok(Vec::new()) };
        let ids = self.by_label.as_ref().unwrap().get(&id).cloned().unwrap_or_default();
        let mut out = Vec::new();
        for m in ids {
            let a = self.raw.action(m)?;
            out.extend(predict::pieces_of(&a, label, bans, &self.domains));
        }
        Ok(out)
    }

    fn key(&mut self, req: &PlanRequest, lines: &[(String, String)], lowered: &[(u32, u32)]) -> [u8; 32] {
        let start: Vec<String> = lines.iter().map(|(k, v)| format!("{}\t{}", k, v)).collect();
        cache::key(&cache::KeyIn {
            reading: self.id,
            goal: &req.goal,
            start: &start,
            bans: &req.bans,
            caps: lowered,
            search: req.search,
            start_check: &req.start_check,
        })
    }

    /// The request's plan (module docstring), with `self.last` set to its parts' seconds.
    pub fn plan_req(&mut self, req: &PlanRequest) -> Result<PlanResult, PlanError> {
        let t0 = Instant::now();
        let cpu0 = cpu_s();
        self.last = Timing::default();
        if req.start_check != "needed" && req.start_check != "input" {
            return Err(tc_err(format!("--start-check {}: input or needed", req.start_check)));
        }
        let lines = self.start_lines(&req.start);
        let caps = Self::caps_of(&req.caps);
        let lowered = self.lowered(&caps);
        let key = self.key(req, &lines, &lowered);
        self.last.key = t0.elapsed().as_secs_f64();
        let use_cache = self.host.cache;
        let hit = if !use_cache { None } else { self.memo.get(&key).cloned() }.or_else(|| {
            if !use_cache {
                return None;
            }
            let r = cache::load(self.host.cache_dir.as_deref()?, &key)?;
            self.memo.insert(key, r.clone());
            Some(r)
        });
        if let Some(mut r) = hit {
            r.source = "cache".into();
            r.seconds = t0.elapsed().as_secs_f64();
            r.cpu_s = cpu_s() - cpu0;
            self.last.total = r.seconds;
            self.last.kept = "cache".into();
            return Ok(r);
        }
        let bans = Self::bans_of(&req.bans);
        let goal = goal_text(&req.goal);
        if let Some(names) = &req.tail {
            let t1 = Instant::now();
            let r = self.try_tail(req, names, &lines, &bans, &goal);
            self.last.tail = t1.elapsed().as_secs_f64();
            if let Some(mut r) = r? {
                r.seconds = t0.elapsed().as_secs_f64();
                r.cpu_s = cpu_s() - cpu0;
                self.last.total = r.seconds;
                self.last.kept = "tail".into();
                return Ok(r);
            }
        }
        let mut r = self.search(req, lines, bans, goal, caps)?;
        r.seconds = t0.elapsed().as_secs_f64();
        r.cpu_s = cpu_s() - cpu0;
        if use_cache && matches!(r.status.as_str(), "plan" | "unsolvable" | "incomplete" | "out of domain") {
            let t1 = Instant::now();
            self.memo.insert(key, r.clone());
            if let Some(dir) = &self.host.cache_dir {
                cache::store(dir, &key, &r).map_err(tc_err)?;
            }
            self.last.store = t1.elapsed().as_secs_f64();
        }
        self.last.total = t0.elapsed().as_secs_f64();
        self.last.cpu = cpu_s() - cpu0;
        Ok(r)
    }

    /// The old tail alone (the check of item 6): Some(the result) when the request's tail holds, None when it does
    /// not (no search).
    pub fn tail_only(&mut self, req: &PlanRequest) -> Result<Option<PlanResult>, PlanError> {
        let t0 = Instant::now();
        self.last = Timing::default();
        let lines = self.start_lines(&req.start);
        let bans = Self::bans_of(&req.bans);
        let goal = goal_text(&req.goal);
        let names = req.tail.clone().unwrap_or_default();
        let r = self.try_tail(req, &names, &lines, &bans, &goal);
        self.last.tail = t0.elapsed().as_secs_f64();
        self.last.total = self.last.tail;
        r
    }

    /// The old tail (taskcore tail.rs): Some(the result) when it holds, None when the planner must plan.
    fn try_tail(
        &mut self,
        req: &PlanRequest,
        names: &[String],
        lines: &[(String, String)],
        bans: &[taskcore::bans::Ban],
        goal: &str,
    ) -> Result<Option<PlanResult>, PlanError> {
        // the start the reading does not cover is the planner's answer (the reach extension), not the tail's
        if let Err(e) = self.warm.reached_error(lines) {
            return Err(tc_err(e));
        }
        let domains = self.domains.clone();
        let fbans = req.bans.clone();
        let acc = {
            // each label's paths once per check (the suffixes ask for the same labels again)
            let mut seen: HashMap<String, Vec<Action>> = HashMap::new();
            let mut cands = |label: &str| -> Result<Vec<Action>, String> {
                if let Some(v) = seen.get(label) {
                    return Ok(v.clone());
                }
                let v = self.paths_of_label(label, &fbans)?;
                seen.insert(label.to_string(), v.clone());
                Ok(v)
            };
            tail::check_suffixes(&domains, names, &req.start, &req.goal, &mut cands)
        };
        let Ok(acc) = acc else { return Ok(None) };
        let needed = self.warm.needed_for(bans, Some(goal)).map_err(tc_err)?;
        let nset: HashSet<Key> = needed.iter().map(|k| Key::parse(k)).collect();
        let n = acc.paths.len();
        let skipped = acc.skipped;
        let steps = acc
            .paths
            .iter()
            .enumerate()
            .map(|(i, p)| PlanStep { op: "tail".into(), action: i as u32, name: p.name.clone(), chosen: vec![], guessed: vec![] })
            .collect();
        Ok(Some(PlanResult {
            status: "plan".into(),
            steps,
            actions: acc.paths.iter().map(|a| predict::cut_to(a, &nset)).collect(),
            paths: acc.paths,
            choices: vec![],
            folds: vec![None; n],
            predicted_goal: true,
            plan_cost: None,
            file_facts: vec![],
            needed_facts: needed.iter().map(|k| Key::parse(k)).collect(),
            misses: vec![],
            n_misses: 0,
            stats: format!("{{\"tail\": true, \"skipped\": {}}}", skipped),
            source: "tail".into(),
            seconds: 0.0,
            cpu_s: 0.0,
        }))
    }

    fn search(
        &mut self,
        req: &PlanRequest,
        lines: Vec<(String, String)>,
        bans: Vec<taskcore::bans::Ban>,
        goal: String,
        caps: Vec<(String, u32)>,
    ) -> Result<PlanResult, PlanError> {
        let t1 = Instant::now();
        let wreq = warm::Request {
            goal: Some(goal),
            start: lines,
            bans,
            caps: Some(caps),
            needed_check: req.start_check == "needed",
            start_name: "request".into(),
        };
        let red = match self.warm.request(&wreq) {
            Ok(r) => r,
            Err(e) => {
                if let Some((n, keys)) = out_of_domain(&e) {
                    return Ok(PlanResult {
                        status: "out of domain".into(),
                        misses: keys.iter().map(|k| (Key::parse(k), req.start.val(Key::parse(k)))).collect(),
                        n_misses: n,
                        source: "search".into(),
                        ..Default::default()
                    });
                }
                return Err(tc_err(e));
            }
        };
        self.last.reduce = t1.elapsed().as_secs_f64();
        self.last.kept = red.kept.to_string();
        let t2 = Instant::now();
        let cfg = self.warm.cfg;
        let t = &red.task;
        let mut x = sas::Expander::new(t, cfg.limit);
        x.model = cfg.model;
        let start_vals = &t.starts[red.start].1;
        let (plan, st, of) = search::build(&mut x, start_vals).map_err(tc_err)?;
        self.last.build = t2.elapsed().as_secs_f64();
        self.last.operators = st.n_ops();
        let deadline = Some(Instant::now() + Duration::from_secs(req.time_limit_s.max(1)));
        let (status, ops, plan_cost) = match req.search {
            Search::Gbfs => {
                let o = search::lazy_greedy(&st, deadline);
                self.last.relax = o.relax_s;
                self.last.search = o.search_s;
                self.last.expanded = o.expanded;
                self.last.evaluated = o.evaluated;
                self.last.unary = o.unary;
                let cost = if o.status == "plan" { Some(o.cost) } else { None };
                (o.status, o.plan, cost)
            }
            Search::FdLamaFirstCosts => {
                let ts = Instant::now();
                let r = self.fd(&mut x, &plan, start_vals, req.time_limit_s)?;
                self.last.search = ts.elapsed().as_secs_f64();
                r
            }
        };
        let mut stats = red.stats;
        let mut c = plan.counts.clone();
        c.bytes = 0;
        stats.raw("sas", &taskcore::cli::sas_json(&c));
        stats.s("search", match req.search {
            Search::Gbfs => "gbfs",
            Search::FdLamaFirstCosts => "lama-first-costs",
        });
        stats.s("kept", red.kept);
        let nf = t.facts.len() as u32;
        let file_facts: Vec<Key> = plan.vars.iter().filter(|&&v| v < nf).map(|&v| Key::parse(t.key(v))).collect();
        let needed_facts: Vec<Key> = red.needed.iter().map(|k| Key::parse(k)).collect();
        let mut res = PlanResult {
            status: status.clone(),
            plan_cost,
            file_facts,
            needed_facts,
            source: "search".into(),
            ..Default::default()
        };
        if status != "plan" {
            // plan_fd reads the operator table's facts only for a plan
            res.file_facts.clear();
            res.needed_facts.clear();
            res.stats = stats.done();
            return Ok(res);
        }
        // every returned plan is a plan of the SAS task from its start
        let tc = Instant::now();
        search::check_plan(&st, &ops).map_err(|e| tc_err(format!("the plan does not check: {}", e)))?;
        self.last.check = tc.elapsed().as_secs_f64();
        // read back: the plan's rows only
        let td = Instant::now();
        let mut xd = sas::Expander::new(t, cfg.limit);
        xd.model = cfg.model;
        let dec = decode::decode(&mut xd, &of, &ops).map_err(tc_err)?;
        let txt = |f: u32, v: u32| value_of(decode::val_text(t, f, v));
        let val = |f: u32, v: u32| -> Result<Val, PlanError> {
            txt(f, v).ok_or_else(|| tc_err(format!("{}: a value {:?}", t.key(f), decode::val_text(t, f, v))))
        };
        let mut psteps = Vec::with_capacity(dec.len());
        let mut out_steps = Vec::with_capacity(dec.len());
        for d in &dec {
            let chosen: Vec<(Key, Val)> =
                d.chosen.iter().map(|&(f, v)| Ok((Key::parse(t.key(f)), val(f, v)?))).collect::<Result<_, PlanError>>()?;
            let mut guessed = Vec::with_capacity(d.guessed.len());
            let mut gtext = Vec::with_capacity(d.guessed.len());
            for (f, v, c) in &d.guessed {
                let conds = c
                    .iter()
                    .map(|(cf, vs)| {
                        let mut vv: Vec<Val> = vs.iter().map(|&x| val(*cf, x)).collect::<Result<_, _>>()?;
                        vv.sort_unstable();
                        Ok((Key::parse(t.key(*cf)), vv))
                    })
                    .collect::<Result<Vec<_>, PlanError>>()?;
                guessed.push((Key::parse(t.key(*f)), val(*f, *v)?, conds));
                gtext.push((Key::parse(t.key(*f)), val(*f, *v)?, decode::conds_json(t, c)));
            }
            psteps.push(predict::Step { op: format!("a{}", d.op), name: d.name.clone(), members: d.members.clone(), chosen: chosen.clone(), guessed });
            out_steps.push((format!("a{}", d.op), chosen, gtext));
        }
        self.last.decode = td.elapsed().as_secs_f64();
        // the paths and the predicted states (plan_fd.predict with the engine's choices)
        let tp = Instant::now();
        let held: HashSet<Key> = res.file_facts.iter().copied().collect();
        let domains = self.domains.clone();
        let fbans = req.bans.clone();
        let labels: HashMap<u32, String> =
            dec.iter().flat_map(|d| d.members.iter().map(|&m| (m, self.label_of(m)))).collect();
        let raw = &self.raw;
        let mut err = None;
        let mut pieces = |m: u32| -> Vec<Action> {
            match raw.action(m) {
                Ok(a) => predict::pieces_of(&a, &labels[&m], &fbans, &domains),
                Err(e) => {
                    err = Some(e);
                    Vec::new()
                }
            }
        };
        let label_of = |m: u32| labels.get(&m).cloned().unwrap_or_default();
        let pr = predict::predict(&domains, &mut pieces, &label_of, &psteps, &req.start, &held, true);
        if let Some(e) = err {
            return Err(tc_err(e));
        }
        let (paths, end, choices) = match pr {
            Ok(x) => x,
            Err(predict::PredictError::PathChoice(m)) => return Err(PlanError::PathChoice(m)),
            Err(predict::PredictError::Other(m)) => return Err(tc_err(format!("predict: {}", m))),
        };
        let nset: HashSet<Key> = res.needed_facts.iter().copied().collect();
        let mut actions: Vec<Action> = paths.iter().map(|a| predict::cut_to(a, &nset)).collect();
        let choices: Vec<(u32, Vec<Action>)> =
            choices.into_iter().map(|(i, ps)| (i, ps.iter().map(|a| predict::cut_to(a, &nset)).collect())).collect();
        for (i, ps) in &choices {
            actions[*i as usize] = predict::either(&dec[*i as usize].name, ps);
        }
        res.steps = out_steps
            .into_iter()
            .zip(&paths)
            .enumerate()
            .map(|(i, ((op, chosen, guessed), a))| PlanStep { op, action: i as u32, name: a.name.clone(), chosen, guessed })
            .collect();
        res.folds = dec.iter().map(|d| if d.members.len() > 1 { Some(d.name.clone()) } else { None }).collect();
        res.predicted_goal = req.goal.iter().all(|(k, vs)| {
            let v = end.get(k).copied().unwrap_or(NONE);
            match domains.get(*k) {
                Some(d) => vs.iter().any(|&x| predict::label(d, x) == v),
                None => false,
            }
        });
        res.paths = paths;
        res.actions = actions;
        res.choices = choices;
        self.last.predict = tp.elapsed().as_secs_f64();
        stats.f("seconds_decode", self.last.decode).f("seconds_predict", self.last.predict);
        res.stats = stats.done();
        Ok(res)
    }

    /// `Search::FdLamaFirstCosts`: the SAS text (sas.rs pass2) and Fast Downward's search binary on it, the plan file
    /// read back: (status, operators, the plan's cost as FD reports it).
    fn fd(&mut self, x: &mut sas::Expander, plan: &sas::Plan, start: &[u32], limit_s: u64) -> Result<(String, Vec<u32>, Option<i64>), PlanError> {
        self.n_fd += 1;
        let dir = self.host.work.join(format!("fd-{}-{}", std::process::id(), self.n_fd));
        std::fs::create_dir_all(&dir).map_err(|e| tc_err(format!("{}: {}", dir.display(), e)))?;
        let sas_path = dir.join("task.sas");
        let f = std::fs::File::create(&sas_path).map_err(|e| tc_err(e.to_string()))?;
        sas::pass2(x, plan, start, std::io::BufWriter::with_capacity(4 << 20, f)).map_err(tc_err)?;
        let r = run_fd(&self.host.downward, &dir, &sas_path, limit_s);
        let _ = std::fs::remove_dir_all(&dir);
        r
    }
}

/// Fast Downward's search binary in `dir` on `sas` (stdin), `LAMA_FIRST_COSTS`, killed past `limit_s`.
pub fn run_fd(downward: &Path, dir: &Path, sas: &Path, limit_s: u64) -> Result<(String, Vec<u32>, Option<i64>), PlanError> {
    let io = |e: std::io::Error| tc_err(e.to_string());
    let log_path = dir.join("fd.log");
    let mut child = std::process::Command::new(downward)
        .arg("--search")
        .arg(LAMA_FIRST_COSTS)
        .arg("--internal-plan-file")
        .arg(dir.join("sas_plan"))
        .current_dir(dir)
        .stdin(std::fs::File::open(sas).map_err(io)?)
        .stdout(std::fs::File::create(&log_path).map_err(io)?)
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| tc_err(format!("{}: {}", downward.display(), e)))?;
    let end = Instant::now() + Duration::from_secs(limit_s.max(1));
    let code = loop {
        if let Some(s) = child.try_wait().map_err(io)? {
            break s.code();
        }
        if Instant::now() > end {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(("time".into(), vec![], None));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let status = match code {
        Some(0) => "plan",
        Some(11) => "unsolvable",
        Some(12) => "incomplete",
        Some(22) => "memory",
        Some(23) => "time",
        Some(34) => "unsupported",
        _ => "error",
    };
    let log = std::fs::read_to_string(&log_path).unwrap_or_default();
    let cost = log.lines().rev().find_map(|l| l.split("Plan cost: ").nth(1)).and_then(|c| c.trim().parse().ok());
    if status != "plan" {
        return Ok((status.into(), vec![], None));
    }
    let text = std::fs::read_to_string(dir.join("sas_plan")).map_err(io)?;
    let ops = search::read_plan(&text).map_err(tc_err)?;
    Ok((status.into(), ops, cost))
}

/// reading.json's "packed" file name.
fn fmi_plan_head(head: &str) -> Result<String, String> {
    let doc = json::parse(head)?;
    Ok(doc.get("packed").and_then(json::Json::as_str).ok_or("reading.json: no \"packed\"")?.to_string())
}

impl Planner for WarmPlanner {
    fn plan(&mut self, req: &PlanRequest) -> Result<PlanResult, PlanError> {
        self.plan_req(req)
    }
}
