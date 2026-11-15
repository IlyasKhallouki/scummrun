//! E016 section C (FMI_NOGOOD=1): the failure memory. Its keys and bans (`nogood`), and the harness with it on a still
//! engine, a planner that plans one step unless it is banned, and a window search whose windows all end empty: the
//! refused step is banned at the plan's exact state in the same attempt and in a later try from the same entry, never
//! for another entry or another target; the no-plan memory answers a request it has seen on the same reading. Without
//! the memory the harness plans with no bans, as before.

use fmi_core::*;
use fmi_explore::harness::{Harness, HarnessCfg, PlannerCache};
use fmi_explore::nogood::{self, Memory, NoPlan, Refuted};
use fmi_explore::ports::WindowPlayer;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

static N: AtomicUsize = AtomicUsize::new(0);

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("nogood-{}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn k(t: &str) -> Key {
    Key::parse(t)
}

fn ev(frame: u64, x: i32) -> Event {
    Event { frame, what: Input::Click(x, 10) }
}

// --- the keys and the ban ------------------------------------------------------------------------------------------

#[test]
fn the_prefix_key_is_the_inputs_and_the_frame() {
    let a = nogood::prefix_key(&[ev(101, 5), ev(103, 6)], 110);
    assert_eq!(a, nogood::prefix_key(&[ev(101, 5), ev(103, 6)], 110));
    assert_ne!(a, nogood::prefix_key(&[ev(101, 5), ev(103, 6)], 111));
    assert_ne!(a, nogood::prefix_key(&[ev(101, 5), ev(104, 6)], 110));
    assert_ne!(a, nogood::prefix_key(&[ev(101, 5)], 110));
    assert_ne!(nogood::prefix_key(&[], 110), nogood::prefix_key(&[], 111));
}

#[test]
fn the_target_and_start_keys() {
    let t = nogood::target_text(&[(k("var:6"), vec![2, 1, 1]), (k("bit:3"), vec![1])]);
    assert_eq!(t, nogood::target_text(&[(k("bit:3"), vec![1]), (k("var:6"), vec![1, 2])]));
    assert_ne!(t, nogood::target_text(&[(k("var:6"), vec![1])]));
    let s1 = Facts::from_pairs(vec![(k("room"), 3), (k("var:5"), 0), (k("var:9"), 4)]);
    let s2 = Facts::from_pairs(vec![(k("room"), 3), (k("var:5"), 0), (k("var:9"), 7)]);
    let keys: HashSet<Key> = [k("room"), k("var:5")].into_iter().collect();
    // var:9 is not tested: the starts are the same to the no-plan memory
    assert_eq!(nogood::start_key(&s1, &keys), nogood::start_key(&s2, &keys));
    let keys9: HashSet<Key> = [k("room"), k("var:9")].into_iter().collect();
    assert_ne!(nogood::start_key(&s1, &keys9), nogood::start_key(&s2, &keys9));
}

#[test]
fn a_label_is_the_name_less_its_record() {
    assert_eq!(nogood::label_of_name("sentence(10,20)#54"), "sentence(10,20)");
    assert_eq!(nogood::label_of_name("sentence(10,20)"), "sentence(10,20)");
    assert_eq!(nogood::label_of_name("event(x@5#2)#17"), "event(x@5#2)");
    assert_eq!(nogood::label_of_name("event(x@5#)"), "event(x@5#)");
}

fn sorted(mut v: Vec<(Key, Val)>) -> Vec<(Key, Val)> {
    v.sort();
    v
}

fn act(name: &str, pre: &[(&str, &[Val])], eff: &[(&str, EffVal, &[(&str, &[Val])])]) -> Action {
    Action {
        name: name.into(),
        pre: pre.iter().map(|(a, vs)| (k(a), vs.to_vec())).collect(),
        effects: eff.iter().map(|(a, v, w)| Effect { key: k(a), v: v.clone(), when: w.iter().map(|(b, vs)| (k(b), vs.to_vec())).collect() }).collect(),
        what: 0,
        cost: 1,
    }
}

#[test]
fn the_ban_is_the_steps_precondition_at_the_predicted_state() {
    let start = Facts::from_pairs(vec![(k("room"), 3), (k("var:5"), 0), (k("var:6"), 0), (k("var:7"), 2)]);
    let paths = vec![
        act("walk(1)#0", &[("room", &[3])], &[("var:5", EffVal::Value(1), &[]), ("var:7", EffVal::Add(1), &[]),
                                             ("var:8", EffVal::Value(4), &[("var:6", &[1])])]),
        act("sentence(1,2)#3", &[("var:5", &[1]), ("var:7", &[2, 3]), ("var:8", &[0, 4])], &[]),
    ];
    let pred = nogood::predicted_before(&start, &paths, 1);
    assert_eq!(pred.get(&k("var:5")), Some(&1));
    // an added value is not told; an effect whose condition fails leaves the key
    assert_eq!(pred.get(&k("var:7")), None);
    assert_eq!(pred.get(&k("var:8")), None);
    assert_eq!(pred.get(&k("room")), Some(&3));
    let all = |_: Key| true;
    let b = nogood::ban_of("sentence(1,2)", &start, &paths, 1, &all).unwrap();
    assert_eq!(b.label, "sentence(1,2)");
    // the point in Key order
    assert_eq!(b.facts, sorted(vec![(k("room"), 3), (k("var:5"), 1)]));
    // only the reading's keys; none told: no ban
    let no_room = |x: Key| x != k("room");
    assert_eq!(nogood::ban_of("sentence(1,2)", &start, &paths, 1, &no_room).unwrap().facts, vec![(k("var:5"), 1)]);
    let none = |_: Key| false;
    assert!(nogood::ban_of("sentence(1,2)", &start, &paths, 1, &none).is_none());
    // the first step's ban is at the start
    assert_eq!(nogood::ban_of("walk(1)", &start, &paths, 0, &all).unwrap().facts, vec![(k("room"), 3)]);
}

fn refuted(entry: i64, prefix: &str, target: &str, label: &str) -> Refuted {
    Refuted { entry, prefix: prefix.into(), target: target.into(), ban: Ban { label: label.into(), facts: vec![(k("room"), 3)] },
              step: format!("{label}#0"), frame: 100, plan: 0, why: "200 frames".into() }
}

#[test]
fn the_memory_keeps_a_ban_with_its_entry_prefix_and_target() {
    let job = json!({"entry": {"id": 4}, "nogood": {"refuted": [refuted(4, "p", "t", "a").json(), refuted(5, "p", "t", "b").json()],
                                                     "no_plan": []}});
    let mut m = Memory::of_job(&job);
    assert_eq!(m.bans("p", "t").iter().map(|b| b.label.as_str()).collect::<Vec<_>>(), vec!["a"]);
    assert!(m.bans("q", "t").is_empty() && m.bans("p", "u").is_empty());
    // a repeat is not kept again; another entry's never
    assert!(!m.refute(refuted(4, "p", "t", "a")));
    assert!(!m.refute(refuted(5, "p", "t", "c")));
    assert!(m.refute(refuted(4, "p", "t", "c")));
    assert_eq!(m.bans("p", "t").len(), 2);
    let r = m.result_json();
    assert_eq!(r["refuted"].as_array().unwrap().len(), 1);
    assert_eq!(r["given"]["refuted"], 1);
    assert_eq!(Refuted::of_json(&r["refuted"][0]).unwrap(), refuted(4, "p", "t", "c"));
    // the no-plan memory
    let np = NoPlan { reading: "/r/reading.json".into(), scope: vec![3], start: "s".into(), target: "t".into(), status: "unsolvable".into() };
    assert!(m.note_no_plan(np.clone()));
    assert!(!m.note_no_plan(np.clone()));
    assert_eq!(m.no_plan("/r/reading.json", "s", "t").unwrap().status, "unsolvable");
    assert!(m.no_plan("/r2/reading.json", "s", "t").is_none() && m.no_plan("/r/reading.json", "s2", "t").is_none());
    assert_eq!(NoPlan::of_json(&np.json()).unwrap(), np);
}

#[test]
fn the_archives_job_file_gives_the_entrys_refutations_and_the_live_readings_no_plans() {
    let np = |r: &str| NoPlan { reading: r.into(), scope: vec![3], start: "s".into(), target: "t".into(), status: "unsolvable".into() }.json();
    let l1 = json!({"job": "j00001", "nogood": {"refuted": [refuted(4, "p", "t", "a").json(), refuted(6, "p", "t", "b").json()],
                                                "no_plan": [np("/old/reading.json")]}});
    let l2 = json!({"job": "j00002", "nogood": {"refuted": [refuted(4, "p", "t", "a").json()], "no_plan": [np("/new/reading.json")]}});
    let l3 = json!({"job": "j00003"});
    let live: HashSet<String> = ["/new/reading.json".to_string()].into_iter().collect();
    let j = nogood::job_json(&[&l1, &l2, &l3], 4, &live);
    assert_eq!(j["refuted"].as_array().unwrap().len(), 1);
    assert_eq!(j["refuted"][0]["label"], "a");
    assert_eq!(j["no_plan"].as_array().unwrap().len(), 1);
    assert_eq!(j["no_plan"][0]["reading"], "/new/reading.json");
}

// --- the harness with the memory ----------------------------------------------------------------------------------

/// An engine that stands still at one state (room 3, every var 0).
struct Still(Status);

impl Engine for Still {
    fn pid(&self) -> u32 {
        1
    }
    fn status(&self) -> &Status {
        &self.0
    }
    fn run(&mut self, _spec: &RunSpec) -> EResult<Status> {
        Ok(self.0.clone())
    }
    fn inputs(&mut self, _e: &[Event]) -> EResult<()> {
        Ok(())
    }
    fn obs(&mut self, _req: &ObsReq) -> EResult<Obs> {
        Ok(Obs { frame: self.0.frame, tick: self.0.tick, room: 3, vars: vec![0; 60], ..Default::default() })
    }
    fn facts(&mut self) -> EResult<Facts> {
        Ok(facts_of(&self.obs(&ObsReq::default())?))
    }
    fn holds(&mut self, _c: &[Cond]) -> EResult<bool> {
        Ok(false)
    }
    fn at(&mut self, _x: i32, _y: i32) -> EResult<At> {
        Ok(At::default())
    }
    fn set_rng(&mut self, _s: u32) -> EResult<()> {
        Ok(())
    }
    fn segment(&mut self, _on: bool) -> EResult<()> {
        Ok(())
    }
    fn runs(&mut self) -> EResult<RunsReply> {
        Ok(RunsReply::default())
    }
    fn explain(&mut self, _a: Option<&ExplainArm>) -> EResult<()> {
        Ok(())
    }
    fn explain_log(&mut self) -> EResult<ExplainReply> {
        Ok(ExplainReply::default())
    }
    fn fork(&mut self) -> EResult<Box<dyn Engine>> {
        Ok(Box::new(Still(self.0.clone())))
    }
    fn fork_each(&mut self, _n: u32) -> EResult<(Status, Vec<Box<dyn Engine>>)> {
        Err(EngineError::Bridge("no fork_each".into()))
    }
    fn state_json(&mut self, _r: &ObsReq) -> EResult<String> {
        Err(EngineError::Bridge("no json".into()))
    }
    fn cpu_s(&self) -> f64 {
        0.0
    }
    fn quit(self: Box<Self>) {}
    fn boxes(&mut self) -> EResult<Boxes> {
        Err(EngineError::Bridge("no boxes".into()))
    }
}

/// A reading of room 3 whose task facts are room, var:5 and var:6.
struct Rd {
    dir: PathBuf,
    head: String,
    doms: Domains,
    start: Facts,
}

fn reading(dir: &Path) -> Arc<dyn ReadingSource> {
    let d = |vs: &[Val]| Domain { values: vs.to_vec(), low: None, high: None };
    let mut doms = vec![(k("room"), d(&[3, 5])), (k("var:5"), d(&[0, 1])), (k("var:6"), d(&[0, 1]))];
    doms.sort_by_key(|p| p.0);
    std::fs::create_dir_all(dir).unwrap();
    Arc::new(Rd { dir: dir.to_path_buf(), head: json!({"scope": {"rooms": [3]}, "start": {"name": "s", "facts": {}}}).to_string(),
                  doms: Domains(doms), start: Facts::new() })
}

impl ReadingSource for Rd {
    fn id(&self) -> [u8; 20] {
        [3; 20]
    }
    fn dir(&self) -> &Path {
        &self.dir
    }
    fn head(&self) -> &str {
        &self.head
    }
    fn packed(&self) -> &[u8] {
        &[]
    }
    fn n_actions(&self) -> u32 {
        1
    }
    fn action_line(&self, _i: u32) -> &str {
        unimplemented!()
    }
    fn by_name(&self, _name: &str) -> Option<u32> {
        None
    }
    fn n_lists(&self) -> u32 {
        0
    }
    fn list_line(&self, _i: u32) -> &str {
        unimplemented!()
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
        unimplemented!()
    }
    fn record(&self, _i: u32) -> Record {
        unimplemented!()
    }
}

/// The planner: the one step `sentence(1,2)#0` (var:6 set) unless a ban names its label, then none ("unsolvable"); a
/// target on var:5 has no plan at all. Each request's bans are kept.
#[derive(Clone, Default)]
struct Planned {
    asked: Arc<Mutex<Vec<Vec<Ban>>>>,
}

impl Planner for Planned {
    fn plan(&mut self, req: &PlanRequest) -> Result<PlanResult, PlanError> {
        self.asked.lock().unwrap().push(req.bans.clone());
        if req.goal.iter().any(|g| g.0 == k("var:5")) || req.bans.iter().any(|b| b.label == "sentence(1,2)") {
            return Ok(PlanResult { status: "unsolvable".into(), ..Default::default() });
        }
        let a = act("sentence(1,2)#0", &[("room", &[3]), ("var:5", &[0])], &[("var:6", EffVal::Value(1), &[])]);
        Ok(PlanResult {
            status: "plan".into(),
            steps: vec![PlanStep { op: "a0".into(), action: 0, name: a.name.clone(), chosen: vec![], guessed: vec![] }],
            paths: vec![a.clone()],
            actions: vec![a],
            source: "search".into(),
            ..Default::default()
        })
    }
}

impl PlannerHost for Planned {
    fn planner(&self, _rd: &dyn ReadingSource) -> Result<Box<dyn Planner>, PlanError> {
        Ok(Box::new(self.clone()))
    }
}

/// A window search whose every window ends empty on the plan's first step; the engine does not move.
#[derive(Default)]
struct Empty {
    windows: Vec<Value>,
    detail: Option<Value>,
}

impl Player for Empty {
    fn observe(&mut self, _ctx: &PlanCtx<'_>, ps: &mut PlayState) -> EResult<()> {
        let o = ps.engine.obs(&ObsReq::default())?;
        ps.obs = facts_of(&o);
        ps.now = ps.obs.clone();
        ps.state = o;
        Ok(())
    }
    fn play_plan(&mut self, ctx: &PlanCtx<'_>, ps: &mut PlayState, n_plan: u32) -> PlayOutcome {
        let name = ctx.result.steps[0].name.clone();
        self.windows.push(json!({"plan": n_plan, "macro": 0, "input": name, "start": ps.engine.status().frame,
                                 "end_reason": "200 frames"}));
        self.detail = Some(json!({"what": "window empty", "cause": null}));
        PlayOutcome::Replan
    }
}

impl WindowPlayer for Empty {
    fn goal_credited(&self) -> bool {
        false
    }
    fn replan_cause(&self) -> Option<String> {
        Some("window empty: 200 frames".into())
    }
    fn replan_cause_detail(&self) -> Option<Value> {
        self.detail.clone()
    }
    fn short(&self, _cause: &Value) -> String {
        String::new()
    }
    fn plan_notes(&mut self) -> Option<Value> {
        None
    }
    fn movie(&self) -> Vec<Event> {
        vec![]
    }
    fn flush(&mut self, _ps: &mut PlayState) {}
    fn summary(&mut self, _ps: &PlayState) -> Value {
        json!({})
    }
    fn windows(&self) -> Vec<Value> {
        self.windows.clone()
    }
}

struct Ran {
    out: Value,
    asked: Vec<Vec<Ban>>,
}

/// One attempt for `goal` from the still engine at frame 100, with the memory `mem` or none.
fn attempt(dir: &Path, goal: &str, mem: Option<&nogood::Shared>) -> Ran {
    let rd = reading(&dir.join("rd"));
    let planned = Planned::default();
    let planners = Mutex::new(PlannerCache::new(Arc::new(planned.clone())));
    let cfg = HarnessCfg {
        replan_limit: 5, time_limit_s: 100.0, plan_time_s: 10, first_plan_time_s: None, search: Search::Gbfs, cost_caps: false,
        cost_dmax: None, unobserved_actor_rooms: false, reach_extend: false, reach_extend_limit: 0, refine: false,
        start_check: "needed".into(), workdir: dir.join(format!("plans-{}", N.fetch_add(1, Ordering::SeqCst))),
        search_name: "gbfs".into(), tail: false,
    };
    let ps = PlayState {
        engine: Box::new(Still(Status { frame: 100, tick: 600, reason: "idle".into(), ..Default::default() })),
        state: Obs::default(), obs: Facts::new(), now: Facts::new(), pred: Facts::new(), kept: Facts::new(),
        checkpoints: vec![], stop_reason: None, replan_cause: None, records: vec![], workdir: dir.to_path_buf(),
    };
    let say = |_: &str| {};
    let mut h = Harness::new(cfg, &planners, Box::new(Empty::default()), None, rd, vec![(k(goal), vec![1])], ps, &say).unwrap();
    if let Some(m) = mem {
        h.set_nogood(nogood::Ctx { mem: m.clone(), tested: None });
    }
    let out = h.run();
    let asked = planned.asked.lock().unwrap().clone();
    Ran { out, asked }
}

fn shared(job: &Value) -> nogood::Shared {
    Arc::new(Mutex::new(Memory::of_job(job)))
}

#[test]
fn without_the_memory_the_same_plan_again() {
    let d = tmp("off");
    let r = attempt(&d, "var:6", None);
    assert_eq!(r.asked, vec![Vec::<Ban>::new()]);
    assert!(r.out["stop"].as_str().unwrap().contains("the same plan again"), "{}", r.out["stop"]);
    assert!(r.out["plans"].as_array().unwrap().iter().all(|p| p["bans"] == 0 && p.get("nogood_bans").is_none()));
}

#[test]
fn a_refused_step_is_banned_at_the_plans_state_in_the_attempt_and_later_tries() {
    let d = tmp("refuted");
    let m = shared(&json!({"entry": {"id": 4}}));
    let r = attempt(&d, "var:6", Some(&m));
    // plan 0 with no ban; its window ends empty; the replan at the same state is asked with the step banned at the
    // plan's start (room 3, var:5 = 0), and the planner proves no plan
    let ban = Ban { label: "sentence(1,2)".into(), facts: sorted(vec![(k("room"), 3), (k("var:5"), 0)]) };
    assert_eq!(r.asked, vec![vec![], vec![ban.clone()]]);
    assert_eq!(r.out["stop"], json!("no plan (the planner proved none)"));
    let plans = r.out["plans"].as_array().unwrap();
    assert_eq!(plans[0]["nogood_refuted"]["label"], "sentence(1,2)");
    assert_eq!((plans[1]["bans"].clone(), plans[1]["nogood_bans"].clone()), (json!(1), json!([["sentence(1,2)", 2]])));
    let res = m.lock().unwrap().result_json();
    assert_eq!(res["refuted"].as_array().unwrap().len(), 1);
    // no no-plan record: the planner was asked with a ban
    assert!(res["no_plan"].as_array().unwrap().is_empty());
    // a later try from the same entry, the memory from its job.json: plan 0 is asked with the ban
    let job = json!({"entry": {"id": 4}, "nogood": {"refuted": res["refuted"].clone(), "no_plan": []}});
    let r2 = attempt(&d, "var:6", Some(&shared(&job)));
    assert_eq!(r2.asked, vec![vec![ban]]);
    // another target, or another entry: no ban
    let r3 = attempt(&d, "var:5", Some(&shared(&job)));
    assert_eq!(r3.asked, vec![Vec::<Ban>::new()]);
    let other = json!({"entry": {"id": 5}, "nogood": {"refuted": res["refuted"].clone(), "no_plan": []}});
    let r4 = attempt(&d, "var:6", Some(&shared(&other)));
    assert_eq!(r4.asked[0], Vec::<Ban>::new());
}

#[test]
fn the_no_plan_memory_answers_the_same_request_on_the_same_reading() {
    let d = tmp("noplan");
    let m = shared(&json!({"entry": {"id": 4}}));
    let r = attempt(&d, "var:5", Some(&m));
    assert_eq!(r.asked.len(), 1);
    assert_eq!(r.out["stop"], json!("no plan (the planner proved none)"));
    let res = m.lock().unwrap().result_json();
    let nps = res["no_plan"].as_array().unwrap();
    assert_eq!(nps.len(), 1);
    assert_eq!(nps[0]["status"], "unsolvable");
    assert!(nps[0]["reading"].as_str().unwrap().ends_with("rd/reading.json"));
    assert_eq!(nps[0]["scope"], json!([3]));
    // a later try (another entry: the no-plan memory is the reading's): answered without the planner
    let job = json!({"entry": {"id": 9}, "nogood": {"refuted": [], "no_plan": res["no_plan"].clone()}});
    let m2 = shared(&job);
    let r2 = attempt(&d, "var:5", Some(&m2));
    assert!(r2.asked.is_empty());
    assert_eq!(r2.out["stop"], json!("no plan (the planner proved none)"));
    assert_eq!(r2.out["plans"][0]["nogood"], json!("no plan (memory)"));
    assert_eq!(r2.out["plans"][0]["status"], json!("unsolvable"));
    assert_eq!(m2.lock().unwrap().used_no_plan, 1);
    // another target is asked
    let r3 = attempt(&d, "var:6", Some(&shared(&job)));
    assert_eq!(r3.asked.len(), 2);
}
