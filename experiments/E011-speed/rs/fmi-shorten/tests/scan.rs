//! G3 (plan.md, "Gates"), fmi-shorten's scan, pass and verify on a fake engine: the earliest completion when t=5
//! fails, t=6 works and t=7 fails; pruning at best-1 never loses an earlier end, whatever the workers' order, nor (the
//! bound by (tick, frame, t)) one at a later frame with a lower tick; ties go to the earliest t; a candidate's engine
//! error and the goal reached while a candidate waits for an inner input; a reduction is kept only for an earlier
//! verified end (verdict::earlier, stage B's rule); a forward pass of two steps; a route verified from its files, and a
//! movie without its step log failing.

use fmi_core::*;
use fmi_shorten::label::{Anchor, HitId, Identity, Timing};
use fmi_shorten::scan::{self, Cfg, In, StepSpec};
use fmi_shorten::verdict::{self, End, Goal};
use fmi_shorten::{movie, pass};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

const DOOR: i32 = 51;
const V_DONE: usize = 40;
const V_END: usize = 41;

/// The fake game: a door on the screen. A click on it at frame f is taken when the rule has f, and sets V_DONE (the
/// step's progress) the rule's delay later; any other click is ignored. A taken click on the door while V_DONE is set
/// ends the game the rule's delay later ("restart"). The bridge's idle holds when nothing is pending.
#[derive(Clone)]
struct Fake {
    st: Status,
    vars: Vec<i32>,
    pending: Vec<Event>,
    /// (frame, the var it sets)
    due: Option<(u64, usize)>,
    rule: Arc<BTreeMap<u64, u64>>,
    log: Vec<InputRecord>,
    /// frames whose click fails the engine (its process gone)
    crash: Arc<Vec<u64>>,
    /// frames whose taken click makes every frame after it one tick long (a skip), and the ticks a frame takes
    fast: Arc<Vec<u64>>,
    pace: u64,
}

fn fake(rule: &[(u64, u64)]) -> Fake {
    Fake {
        st: Status { frame: 1, tick: 6, room: 1, idle: Some("sentence".into()), ..Default::default() },
        vars: vec![0; 64],
        pending: Vec::new(),
        due: None,
        rule: Arc::new(rule.iter().copied().collect()),
        log: Vec::new(),
        crash: Arc::new(Vec::new()),
        fast: Arc::new(Vec::new()),
        pace: 6,
    }
}

impl Fake {
    fn hit(&self, x: i32, y: i32) -> At {
        let obj = if (100..140).contains(&x) && (50..90).contains(&y) { DOOR } else { 0 };
        At { x, y, zone: 0, main: Some((x, y, obj, 0)), verb_slot: 0 }
    }

    fn step(&mut self) -> Result<(), Box<Status>> {
        self.st.frame += 1;
        self.st.tick += self.pace;
        let f = self.st.frame;
        let now: Vec<Event> = self.pending.iter().filter(|e| e.frame == f).cloned().collect();
        self.pending.retain(|e| e.frame != f);
        for e in now {
            if let Input::Click(x, y) = e.what {
                let on_door = self.hit(x, y).main.is_some_and(|m| m.2 == DOOR);
                let taken = on_door && self.due.is_none() && self.rule.contains_key(&f);
                if taken {
                    let var = if self.vars[V_DONE] == 1 { V_END } else { V_DONE };
                    self.due = Some((f + self.rule[&f], var));
                    if self.fast.contains(&f) {
                        self.pace = 1;
                    }
                }
                self.log.push(InputRecord {
                    frame: f, input: vec![e.what.clone()], stat: 0, verdict: if taken { Verdict::Taken } else { Verdict::Ignored },
                    area: 2, value: 0, mode: 1, slot: 0, zone: 0, key_match: false, handler: 4, writes: vec![], reads: vec![],
                    ops: vec![], ops_dropped: 0, touches_dropped: 0,
                });
            }
        }
        if let Some((at, var)) = self.due {
            if at <= f {
                self.due = None;
                if var == V_END {
                    let mut st = self.st.clone();
                    st.ended = true;
                    st.reason = "restart@1:2:3:0004".into();
                    return Err(Box::new(st));
                }
                self.vars[var] = 1;
            }
        }
        self.st.idle = if self.due.is_some() { None } else { Some("sentence".into()) };
        Ok(())
    }

    fn holds1(&self, c: &Cond) -> bool {
        let v = match c.kind {
            CondKind::Var => self.vars[c.subject as usize] == c.value,
            CondKind::Room => self.st.room == c.subject,
            _ => false,
        };
        v != c.negate
    }
}

impl Engine for Fake {
    fn pid(&self) -> u32 {
        1
    }
    fn status(&self) -> &Status {
        &self.st
    }
    fn run(&mut self, spec: &RunSpec) -> EResult<Status> {
        if spec.frame.is_none() && spec.max_frame.is_none() && spec.until.is_empty() && !spec.idle {
            return Err(EngineError::Bridge("run needs a stop".into()));
        }
        loop {
            self.step().map_err(EngineError::Ended)?;
            let f = self.st.frame;
            let reason = if spec.frame.is_some_and(|x| f >= x) {
                "frame"
            } else if !spec.until.is_empty() && spec.until.iter().all(|c| self.holds1(c)) {
                "until"
            } else if spec.idle && self.st.idle.is_some() {
                "idle"
            } else if spec.max_frame.is_some_and(|x| f >= x) {
                "max_frame"
            } else {
                continue;
            };
            self.st.reason = reason.into();
            return Ok(self.st.clone());
        }
    }
    fn inputs(&mut self, events: &[Event]) -> EResult<()> {
        if events.iter().any(|e| self.crash.contains(&e.frame)) {
            return Err(EngineError::Bridge("engine closed the connection".into()));
        }
        self.pending.extend_from_slice(events);
        Ok(())
    }
    fn obs(&mut self, _req: &ObsReq) -> EResult<Obs> {
        Ok(Obs {
            frame: self.st.frame,
            room: self.st.room,
            ego: Some(1),
            vars: self.vars.clone(),
            room_objects: Some(vec![RoomObject { id: DOOR, x: 100, y: 50, w: 40, h: 40, ..Default::default() }]),
            ..Default::default()
        })
    }
    fn facts(&mut self) -> EResult<Facts> {
        Ok(facts_of(&self.obs(&ObsReq::default())?))
    }
    fn holds(&mut self, c: &[Cond]) -> EResult<bool> {
        Ok(c.iter().all(|c| self.holds1(c)))
    }
    fn at(&mut self, x: i32, y: i32) -> EResult<At> {
        Ok(self.hit(x, y))
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
        Ok(Box::new(self.clone()))
    }
    fn fork_each(&mut self, n: u32) -> EResult<(Status, Vec<Box<dyn Engine>>)> {
        let mut kids: Vec<Box<dyn Engine>> = Vec::new();
        for _ in 0..n {
            self.step().map_err(EngineError::Ended)?;
            kids.push(Box::new(self.clone()));
        }
        self.st.reason = "fork_each".into();
        Ok((self.st.clone(), kids))
    }
    fn state_json(&mut self, _r: &ObsReq) -> EResult<String> {
        Err(EngineError::Bridge("no json".into()))
    }
    fn cpu_s(&self) -> f64 {
        0.0
    }
    fn quit(self: Box<Self>) {}
    fn input_log(&mut self) -> EResult<InputLog> {
        Ok(InputLog { records: std::mem::take(&mut self.log), dropped: 0, frame: self.st.frame })
    }
    fn digest(&mut self) -> EResult<Digest> {
        let d = self.vars.iter().enumerate().fold(self.st.frame, |a, (i, v)| a.wrapping_mul(31).wrapping_add((i as u64) ^ (*v as u64)));
        Ok(Digest { frame: self.st.frame, digest: d, bytes: 0 })
    }
}

struct FakeHost(Vec<(u64, u64)>);

impl EngineHost for FakeHost {
    fn boot(&self, cfg: &BootCfg) -> EResult<Box<dyn Engine>> {
        let mut t = fake(&self.0);
        t.pending = cfg.events.clone();
        match cfg.frame {
            Some(f) => {
                t.pending.retain(|e| e.frame <= f);
                t.run(&RunSpec { frame: Some(f), ..Default::default() })?;
            }
            None => {
                t.run(&RunSpec { idle: true, max_frame: Some(100_000), ..Default::default() })?;
            }
        }
        Ok(Box::new(t))
    }
}

fn door_click() -> In {
    let want = Identity { hit: HitId::Object(DOOR, 0), verdict: "taken".into(), area: 2, value: 0, mode: 1, handler: 4, sentences: vec![], walks: vec![] };
    In { anchor: Anchor::Object { id: DOOR, actor: 0 }, timing: Timing::Idle, gap: 0, want, orig: Input::Click(0, 0) }
}

fn done_goal() -> Goal {
    Goal::Keep(verdict::conds(&[(Key::var(V_DONE as u64), 1)], &|_| None).unwrap())
}

fn spec() -> StepSpec {
    StepSpec { k: 0, inputs: vec![door_click()], goal: done_goal(), relaxed: None }
}

/// The scan of the fake game with `rule` from a boot stopped at frame 2, completions up to frame 200.
fn scan_with(rule: &[(u64, u64)], cfg: Cfg) -> scan::Scan {
    let mut e = FakeHost(rule.to_vec()).boot(&BootCfg { seed: 1, frame: Some(2), ..Default::default() }).unwrap();
    scan::scan(&mut e, &spec(), 200, &cfg).unwrap()
}

#[test]
fn the_earliest_completion_when_t5_fails_t6_works_t7_fails() {
    // the click of t is at t + 1: taken at 7 (t = 6) and from 40 on (completing later), refused at 6 and 8
    let rule: Vec<(u64, u64)> = std::iter::once((7, 3)).chain((40..60).map(|f| (f, 3))).collect();
    for workers in [1, 3, 8] {
        let s = scan_with(&rule, Cfg { workers, chunk: 4 });
        let f = s.found.expect("found");
        assert_eq!((f.t, f.frame), (6, 10), "workers {workers}");
        assert_eq!(f.events.iter().map(|e| e.frame).collect::<Vec<_>>(), vec![7]);
        // t = 2..=5 and 7.. refused at the click (the producer stops at the best less one)
        assert!(s.counts.refused >= 5, "{:?}", s.counts);
        assert_eq!(s.counts.done, 1);
        // G6's refused bucket holds the candidates that clicked; one with no point never clicks
        assert_eq!(s.cost.refused.0, s.counts.refused + s.counts.other);
        assert_eq!(s.cost.no_point.0, s.counts.no_point);
    }
}

#[test]
fn pruning_at_best_less_one_never_loses_an_earlier_end() {
    // t = 6 completes at 47, t = 14 at 20: the later t is the earlier end, and found whatever the order
    let rule = [(7, 40), (15, 5)];
    for (workers, chunk) in [(1, 1), (2, 3), (8, 16)] {
        let f = scan_with(&rule, Cfg { workers, chunk }).found.expect("found");
        assert_eq!((f.t, f.frame), (14, 20), "workers {workers} chunk {chunk}");
    }
    // ties go to the earliest t: t = 9 and t = 11 both complete at 25
    let rule = [(10, 15), (12, 13)];
    for (workers, chunk) in [(1, 1), (4, 2), (8, 16)] {
        let f = scan_with(&rule, Cfg { workers, chunk }).found.expect("found");
        assert_eq!((f.t, f.frame), (9, 25), "workers {workers} chunk {chunk}");
    }
    // frames of different lengths: t = 6 completes at frame 17, tick 102; t = 8's click makes the frames after it one
    // tick long, and completes at frame 39 with tick 84: the earlier end by the objective, though at a later frame
    for (workers, chunk) in [(1, 1), (2, 3), (8, 16)] {
        let mut f = fake(&[(7, 10), (9, 30)]);
        f.fast = Arc::new(vec![9]);
        let mut e: Box<dyn Engine> = Box::new(f);
        e.run(&RunSpec { frame: Some(2), ..Default::default() }).unwrap();
        let f = scan::scan(&mut e, &spec(), 200, &Cfg { workers, chunk }).unwrap().found.expect("found");
        assert_eq!((f.t, f.frame, f.tick), (8, 39, 84), "workers {workers} chunk {chunk}");
    }
    // nothing within the limit: none, every t tried
    let s = scan_with(&[(150, 100)], Cfg { workers: 4, chunk: 8 });
    assert!(s.found.is_none());
    assert_eq!(s.counts.late, 1);
}

#[test]
fn a_candidates_engine_error_is_its_outcome_not_the_scans() {
    // the click at 5 (t = 4) fails its engine: counted with its message; t = 6 is still found
    let mut f = fake(&[(7, 3)]);
    f.crash = Arc::new(vec![5]);
    let mut e: Box<dyn Engine> = Box::new(f);
    e.run(&RunSpec { frame: Some(2), ..Default::default() }).unwrap();
    for workers in [1, 4] {
        let s = scan::scan(&mut e, &spec(), 200, &Cfg { workers, chunk: 4 }).unwrap();
        assert_eq!(s.found.as_ref().map(|f| (f.t, f.frame)), Some((6, 10)), "workers {workers}");
        assert_eq!(s.counts.error, 1);
        assert_eq!(s.errors.iter().map(|x| x.0).collect::<Vec<_>>(), vec![4]);
    }
}

#[test]
fn the_goal_reached_while_waiting_for_an_inner_input_is_its_completion() {
    // V_DONE already set: the door at 7 ends the game at 11, before the step's second input (by either timing class)
    // is placed; the goal's end with its reason is the completion, its inputs those placed
    for timing in [Timing::Idle, Timing::Next] {
        let mut f = fake(&[(7, 4)]);
        f.vars[V_DONE] = 1;
        let mut e: Box<dyn Engine> = Box::new(f);
        e.run(&RunSpec { frame: Some(2), ..Default::default() }).unwrap();
        let second = In { timing, gap: 6, ..door_click() };
        let goal = StepSpec { k: 0, inputs: vec![door_click(), second.clone()], goal: Goal::End("restart@1:2:3:0004".into()), relaxed: None };
        let s = scan::scan(&mut e, &goal, 200, &Cfg { workers: 2, chunk: 4 }).unwrap();
        let found = s.found.expect("found");
        assert_eq!((found.t, found.frame, found.events[0].frame), (6, 11, 7), "{timing:?}");
        // idle: no stop before the end, the second input never placed; next: tried at each frame on a fork of its own,
        // the last in the end's own frame
        assert_eq!(found.events.len(), if timing == Timing::Idle { 1 } else { 2 });
        // another end reason: an end, counted
        let other = StepSpec { goal: Goal::End("restart@9:9:9:0009".into()), ..goal };
        let s = scan::scan(&mut e, &other, 200, &Cfg { workers: 2, chunk: 4 }).unwrap();
        assert!(s.found.is_none());
        assert!(s.counts.ended >= 1 && s.counts.error == 0, "{:?}", s.counts);
    }
}

#[test]
fn a_reduction_is_kept_only_for_an_earlier_verified_end() {
    let best = End { tick: 1200, frame: 200, reason: "restart@1:2:3:0004".into() };
    let at = |tick, frame, r: &str| End { tick, frame, reason: r.into() };
    assert!(verdict::earlier(Some(&at(1100, 190, "restart@1:2:3:0004")), &best));
    // not verified, the same end, a later one, another site
    assert!(!verdict::earlier(None, &best));
    assert!(!verdict::earlier(Some(&best.clone()), &best));
    assert!(!verdict::earlier(Some(&at(1300, 190, "restart@1:2:3:0004")), &best));
    assert!(!verdict::earlier(Some(&at(1100, 190, "restart@9:2:3:0004")), &best));
    // by tick first, then frame
    assert!(verdict::earlier(Some(&at(1200, 199, "restart@1:2:3:0004")), &best));
}

#[test]
fn a_forward_pass_and_its_route_verified() {
    // step 0: the door at frame 7 or 30 (V_DONE three frames later); step 1, the goal: the door again, from frame
    // 12 on, ends the game four frames later
    let rule: Vec<(u64, u64)> = [(7, 3), (30, 3)].into_iter().chain((12..40).map(|f| (f, 4))).collect();
    let host = FakeHost(rule.clone());
    let specs = vec![
        spec(),
        StepSpec { k: 1, inputs: vec![door_click()], goal: Goal::End("restart@1:2:3:0004".into()), relaxed: None },
    ];
    let origs = vec![
        pass::Orig { from: 2, frame: 40, tick: 240, rooms: vec![1], inputs: 1, skipped: vec![] },
        pass::Orig { from: 30, frame: 60, tick: 360, rooms: vec![1], inputs: 1, skipped: vec![] },
    ];
    let dir = std::env::temp_dir().join(format!("fmi-shorten-g3-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = pass::PassCfg { seed: 1, scan: Cfg { workers: 2, chunk: 4 }, out: dir.join("e"), w: 50 };
    let p = pass::forward(&host, &[], &specs, &origs, &cfg).unwrap();
    assert!(p.stopped.is_none());
    let end = p.end.clone().expect("the end");
    // step 0 at its first window (click 7, V_DONE at 10); step 1 from 7 on, the door taken at 12
    assert_eq!(p.steps[0].scan.found.as_ref().map(|f| (f.t, f.frame)), Some((6, 10)));
    assert_eq!(p.route.iter().map(|e| e.frame).collect::<Vec<_>>(), vec![7, 12]);
    assert_eq!((end.frame, end.tick), (16, 96));
    // the stage's replay, the files, the check from boot (the standalone launch: the fake from boot)
    let comps = [10];
    let stage = movie::replay(&host, &p.route, 1, &comps, 100, &dir.join("stage")).unwrap();
    assert_eq!(stage.end.as_ref().map(|e| (e.frame, e.tick)), Some((16, 96)));
    let rows = vec![serde_json::json!({"k": 0, "completion": {"frame": 10, "tick": 60}, "digest": format!("{:016x}", stage.digests[&10])})];
    let endv = serde_json::json!({"frame": 16, "tick": 96, "reason": "restart@1:2:3:0004"});
    let r = movie::write(&dir, "route", &p.route, &serde_json::json!({"seed": 1}), &rows, &endv).unwrap();
    // never overwritten: the next is 002
    let r2 = movie::write(&dir, "route", &p.route, &serde_json::json!({"seed": 1}), &rows, &endv).unwrap();
    assert!(r.ends_with("route-001.jsonl") && r2.ends_with("route-002.jsonl"));
    let alone = |route: &[Event], seed: u32, _max: u64, d: &Path| -> Result<Option<End>, String> {
        Ok(movie::replay(&host, route, seed, &[], 100, d)?.end)
    };
    let v = movie::verify(&host, &alone, &r, &dir.join("v")).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    // a movie without its step log fails
    std::fs::remove_file(movie::steps_path(&r2)).unwrap();
    let v = movie::verify(&host, &alone, &r2, &dir.join("v2")).unwrap();
    assert_eq!(v["ok"], false);
    // a digest that differs fails
    let bad = vec![serde_json::json!({"k": 0, "completion": {"frame": 10, "tick": 60}, "digest": "0000000000000001"})];
    let r3 = movie::write(&dir, "route", &p.route, &serde_json::json!({"seed": 1}), &bad, &endv).unwrap();
    assert_eq!(movie::verify(&host, &alone, &r3, &dir.join("v3")).unwrap()["ok"], false);
    let _ = std::fs::remove_dir_all(&dir);
}
