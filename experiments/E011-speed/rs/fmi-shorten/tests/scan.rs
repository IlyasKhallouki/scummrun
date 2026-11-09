//! G3 (plan.md, "Gates"), fmi-shorten's scan, pass and verify on a fake engine: the earliest completion when t=5 fails,
//! t=6 works and t=7 fails; pruning at best-1 never loses an earlier end, whatever the workers' order, nor (the bound
//! by (tick, frame, t)) one at a later frame with a lower tick; ties go to the earliest t; a candidate's engine error
//! and the goal reached while a candidate waits for an inner input; a fallback trial's engine error (an offer trial's,
//! a dialog line's) counted as its outcome; the dialog search keeping the earlier of two lines at one frame; a
//! reduction is kept only for an earlier verified end (verdict::earlier; reduce.rs: a step the game does not need
//! dropped and kept once its end verifies, a drop that loses the end not kept, nor one whose version does not verify; a
//! test's engine error fails that test alone, the rounds going on; an input's drop whose step's plan changed stale); a
//! forward pass of two steps; a route verified from its files, a movie without its step log failing, and a step
//! completed after the end recorded and reported unchecked.

use fmi_core::*;
use fmi_shorten::label::{Anchor, HitId, Identity, Timing};
use fmi_shorten::scan::{self, Cfg, In, StepSpec};
use fmi_shorten::verdict::{self, End, Goal};
use fmi_shorten::reduce::{self, Best, Cand, Drop};
use fmi_shorten::refind::{self, How, RCfg};
use fmi_shorten::{movie, pass};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

const DOOR: i32 = 51;
const V_DONE: usize = 40;
const V_END: usize = 41;
/// A window, taken whenever nothing is pending, setting V_WIN `WIN_DELAY` frames later: a step the game does not need.
const WINDOW: i32 = 52;
const V_WIN: usize = 42;
const WIN_DELAY: u64 = 20;
/// A dialog line of the fake: a verb slot at (x, y), 20 by 8, taken whenever nothing is pending, setting V_DONE
/// `LINE_DELAY` frames later; with `kill` its click ends the engine's process (a crash or a timeout: the run fails with
/// an io error), with `fast` it makes every frame after it one tick long.
#[derive(Clone, Copy)]
struct Line {
    slot: i32,
    x: i32,
    y: i32,
    kill: bool,
    fast: bool,
}
const LINE_DELAY: u64 = 3;
/// The input handler the fake's lines are chosen under (VAR_VERB_SCRIPT)
const DIALOG_HANDLER: i32 = 7;

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
    lines: Arc<Vec<Line>>,
    /// a line's `kill` was clicked: the process is gone
    dead: bool,
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
        lines: Arc::new(Vec::new()),
        dead: false,
    }
}

fn record(frame: u64, what: &Input, taken: bool) -> InputRecord {
    InputRecord {
        frame, input: vec![what.clone()], stat: 0, verdict: if taken { Verdict::Taken } else { Verdict::Ignored },
        area: 2, value: 0, mode: 1, slot: 0, zone: 0, key_match: false, handler: 4, writes: vec![], reads: vec![],
        ops: vec![], ops_dropped: 0, touches_dropped: 0,
    }
}

impl Fake {
    fn hit(&self, x: i32, y: i32) -> At {
        if let Some(l) = self.lines.iter().find(|l| (l.x..l.x + 20).contains(&x) && (l.y..l.y + 8).contains(&y)) {
            return At { x, y, zone: 0, main: None, verb_slot: l.slot };
        }
        let obj = if (100..140).contains(&x) && (50..90).contains(&y) {
            DOOR
        } else if (200..240).contains(&x) && (50..90).contains(&y) {
            WINDOW
        } else {
            0
        };
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
                let at = self.hit(x, y);
                if let Some(l) = self.lines.iter().find(|l| l.slot == at.verb_slot).copied() {
                    if l.kill {
                        self.dead = true;
                        return Ok(());
                    }
                    let taken = self.due.is_none();
                    if taken {
                        self.due = Some((f + LINE_DELAY, V_DONE));
                        if l.fast {
                            self.pace = 1;
                        }
                    }
                    self.log.push(record(f, &e.what, taken));
                    continue;
                }
                let on = at.main.map_or(0, |m| m.2);
                let on_window = on == WINDOW && self.due.is_none();
                if on_window {
                    self.due = Some((f + WIN_DELAY, V_WIN));
                }
                let taken = on == DOOR && self.due.is_none() && self.rule.contains_key(&f);
                if taken {
                    let var = if self.vars[V_DONE] == 1 { V_END } else { V_DONE };
                    self.due = Some((f + self.rule[&f], var));
                    if self.fast.contains(&f) {
                        self.pace = 1;
                    }
                }
                self.log.push(record(f, &e.what, taken || on_window));
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
            if self.dead {
                return Err(EngineError::Io(std::io::Error::other("the engine's process is gone")));
            }
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
            verbs: self.lines.iter().map(|l| Verb { slot: l.slot, id: 100 + l.slot, x: l.x, y: l.y, ..Default::default() }).collect(),
            room_objects: Some(vec![
                RoomObject { id: DOOR, x: 100, y: 50, w: 40, h: 40, ..Default::default() },
                RoomObject { id: WINDOW, x: 200, y: 50, w: 40, h: 40, ..Default::default() },
            ]),
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

fn rcfg(workers: usize) -> RCfg {
    let dialog = [DIALOG_HANDLER].into_iter().collect();
    RCfg { scan: Cfg { workers, chunk: 4 }, skips: false, leaves: 10, sentence: Default::default(), dialog }
}

/// The fake at its idle stop at frame 2 with `lines` offered, chosen under DIALOG_HANDLER; the door taken at 3 (V_DONE
/// at 6).
fn with_lines(lines: Vec<Line>) -> Box<dyn Engine> {
    let mut f = fake(&[(3, 3)]);
    f.lines = Arc::new(lines);
    f.vars[fmi_explore::afford::VAR_VERB_SCRIPT] = DIALOG_HANDLER;
    let mut e: Box<dyn Engine> = Box::new(f);
    e.run(&RunSpec { frame: Some(2), ..Default::default() }).unwrap();
    e
}

#[test]
fn a_fallback_trials_engine_error_is_its_outcome_not_the_passes() {
    let kill = Line { slot: 1, x: 10, y: 150, kill: true, fast: false };
    let line = Line { slot: 2, x: 10, y: 160, kill: false, fast: false };
    for workers in [1, 3] {
        // the offer at 2: the line that kills the engine alone, then with the door and with the window: three errors;
        // the door alone completes at 6, the window alone does not
        let mut e = with_lines(vec![kill]);
        let (r, reps) = refind::refind(&mut e, &spec(), 50, 0, 200, &rcfg(workers)).unwrap();
        let r = r.expect("found by an offer trial");
        assert_eq!((r.how, r.found.t, r.found.frame), (How::Offer, 2, 6), "workers {workers}");
        let o = reps.iter().find(|v| v["how"] == "offer").unwrap();
        assert_eq!((o["inputs"].as_u64(), o["error"].as_u64(), o["run"].as_u64(), o["done"].as_u64()), (Some(5), Some(3), Some(2), Some(1)), "{o}");
        assert!(o["errors"][0][1].as_str().is_some_and(|m| m.contains("process is gone")), "{o}");
        // the dialog lines (the offer's trials cut to the original step's length, 1 frame: none completes): the line
        // that kills the engine is a leaf, counted; the other completes at 6
        let mut e = with_lines(vec![kill, line]);
        let (r, reps) = refind::refind(&mut e, &spec(), 1, 0, 200, &rcfg(workers)).unwrap();
        let r = r.expect("found by the dialog lines");
        assert_eq!((r.how, r.found.frame, r.found.events.len()), (How::Dialogs, 6, 1), "workers {workers}");
        let d = reps.iter().find(|v| v["how"] == "dialogs").unwrap();
        assert_eq!((d["lines"].as_u64(), d["leaves"].as_u64(), d["error"].as_u64()), (Some(2), Some(2), Some(1)), "{d}");
        assert_eq!(reps.iter().find(|v| v["how"] == "offer").unwrap()["done"], 0);
    }
}

#[test]
fn the_dialog_search_keeps_the_earlier_tick_at_one_frame() {
    // two lines, both V_DONE at frame 6; the second (searched first) makes the frames after it one tick long: tick 21
    // against the first's 36. The offer's trials cut to 1 frame complete none
    let slow = Line { slot: 1, x: 10, y: 150, kill: false, fast: false };
    let fast = Line { slot: 2, x: 10, y: 160, kill: false, fast: true };
    for workers in [1, 3] {
        let mut e = with_lines(vec![slow, fast]);
        let (r, _) = refind::refind(&mut e, &spec(), 1, 0, 200, &rcfg(workers)).unwrap();
        let r = r.expect("found by the dialog lines");
        assert_eq!((r.how, r.found.frame, r.found.tick), (How::Dialogs, 6, 21), "workers {workers}");
        assert_eq!(r.found.events.iter().map(|e| e.what.clone()).collect::<Vec<_>>(), vec![Input::Click(fast.x + 2, fast.y + 2)]);
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
    let cfg = pass::PassCfg { seed: 1, scan: Cfg { workers: 2, chunk: 4 }, out: dir.join("e"), w: 50, refind: None };
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
    let meta = serde_json::json!({"seed": 1, "goal_end": "restart@1:2:3:0004"});
    let r = movie::write(&dir, "unverified", &p.route, &meta, &rows, &endv).unwrap();
    // never overwritten: the next is 002
    let r2 = movie::write(&dir, "unverified", &p.route, &meta, &rows, &endv).unwrap();
    assert!(r.ends_with("unverified-001.jsonl") && r2.ends_with("unverified-002.jsonl"));
    let alone = |route: &[Event], seed: u32, _max: u64, d: &Path| -> Result<Option<End>, String> {
        Ok(movie::replay(&host, route, seed, &[], 100, d)?.end)
    };
    let v = movie::verify(&host, &alone, &r, &dir.join("v")).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    // verified: a route of its own name, with its step log, never over another
    std::fs::write(dir.join("route-001.jsonl"), "").unwrap();
    let rr = movie::promote(&r, "route").unwrap();
    assert!(rr.ends_with("route-002.jsonl") && movie::steps_path(&rr).exists() && !r.exists() && !movie::steps_path(&r).exists());
    assert_eq!(movie::verify(&host, &alone, &rr, &dir.join("vr")).unwrap()["ok"], true);
    // an end that is not the goal's fails
    let other = serde_json::json!({"seed": 1, "goal_end": "restart@9:9:9:0009"});
    let r4 = movie::write(&dir, "unverified", &p.route, &other, &rows, &endv).unwrap();
    assert_eq!(movie::verify(&host, &alone, &r4, &dir.join("v4")).unwrap()["ok"], false);
    // a movie without its step log fails
    std::fs::remove_file(movie::steps_path(&r2)).unwrap();
    let v = movie::verify(&host, &alone, &r2, &dir.join("v2")).unwrap();
    assert_eq!(v["ok"], false);
    // a digest that differs fails
    let bad = vec![serde_json::json!({"k": 0, "completion": {"frame": 10, "tick": 60}, "digest": "0000000000000001"})];
    let r3 = movie::write(&dir, "unverified", &p.route, &meta, &bad, &endv).unwrap();
    assert_eq!(movie::verify(&host, &alone, &r3, &dir.join("v3")).unwrap()["ok"], false);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The drops' game: the door taken at any frame from 3 on, three frames to V_DONE and to the end.
fn drop_host() -> FakeHost {
    FakeHost((3..400).map(|f| (f, 3)).collect())
}

/// The drops' steps: step 0, the window (V_WIN 20 frames later), a step the game does not need; step 1, the door
/// (V_DONE); step 2, the goal: the door again ends the game. And the movie's steps.
fn drop_steps() -> (Vec<StepSpec>, Vec<pass::Orig>) {
    let win = {
        let want = Identity { hit: HitId::Object(WINDOW, 0), verdict: "taken".into(), area: 2, value: 0, mode: 1, handler: 4, sentences: vec![], walks: vec![] };
        In { anchor: Anchor::Object { id: WINDOW, actor: 0 }, timing: Timing::Idle, gap: 0, want, orig: Input::Click(0, 0) }
    };
    let all = vec![
        StepSpec { k: 0, inputs: vec![win], goal: Goal::Keep(verdict::conds(&[(Key::var(V_WIN as u64), 1)], &|_| None).unwrap()), relaxed: None },
        StepSpec { k: 1, ..spec() },
        StepSpec { k: 2, inputs: vec![door_click()], goal: Goal::End("restart@1:2:3:0004".into()), relaxed: None },
    ];
    let orig = |from, frame| pass::Orig { from, frame, tick: frame * 6, rooms: vec![1], inputs: 1, skipped: vec![] };
    (all, vec![orig(2, 40), orig(40, 60), orig(60, 80)])
}

#[test]
fn a_drop_is_kept_only_for_an_earlier_verified_end() {
    let host = drop_host();
    let (all, origs) = drop_steps();
    let dir = std::env::temp_dir().join(format!("fmi-shorten-g3r-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rc = RCfg { scan: Cfg { workers: 2, chunk: 4 }, skips: false, leaves: 10, sentence: Default::default(), dialog: Default::default() };
    let cfg = pass::PassCfg { seed: 1, scan: Cfg { workers: 2, chunk: 4 }, out: dir.join("e"), w: 50, refind: Some(rc) };
    let p = pass::forward(&host, &[], &all, &origs, &cfg).unwrap();
    let f = p.end.clone().expect("the end");
    // the window at 3, V_WIN at 23; the door at 4 is refused (V_WIN pending), taken at 24, V_DONE at 27; the door at
    // 28, the end at 31
    assert_eq!((f.frame, f.tick), (31, 186), "{f:?} {:?}", p.route);
    let goal = "restart@1:2:3:0004";
    let best = Best { pass: p.clone(), end: End { tick: f.tick, frame: f.frame, reason: goal.into() }, path: dir.join("route-001.jsonl"), dropped: Default::default() };
    let specs = |d: &std::collections::BTreeSet<usize>| -> Result<(Vec<StepSpec>, Vec<pass::Orig>), String> {
        Ok(all.iter().cloned().zip(origs.iter().cloned()).filter(|(s, _)| !d.contains(&s.k)).unzip())
    };
    // a version: the stage's replay from boot, verified unless `refuse`
    let version = |refuse: bool| {
        let host = &host;
        let dir = dir.clone();
        move |p: &pass::Pass, _d: &std::collections::BTreeSet<usize>| -> Result<(Option<End>, std::path::PathBuf, serde_json::Value), String> {
            let b = movie::replay(host, &p.route, 1, &[], 1000, &dir.join("stage"))?;
            Ok((b.end.filter(|_| !refuse), dir.join("route-002.jsonl"), serde_json::json!({})))
        }
    };
    let by_k: std::collections::BTreeMap<usize, pass::Orig> = all.iter().zip(&origs).map(|(s, o)| (s.k, o.clone())).collect();
    let cands = reduce::candidates(&best, &by_k);
    assert!(cands.iter().any(|c| c.what == Drop::Steps(vec![0])) && cands.iter().any(|c| c.what == Drop::Steps(vec![1])));
    let ok = version(false);
    let cx = reduce::Ctx { host: &host, boot: &[], specs: &specs, cfg: &cfg, out: &dir, version: &ok };
    // the window dropped: the door at 3, V_DONE at 6, the door at 7, the end at 10; kept, verified
    let c0 = Cand { what: Drop::Steps(vec![0]), kind: "step", waste: 0 };
    let (nb, rec) = reduce::attempt(&c0, &best, &cx).unwrap();
    let nb = nb.expect("kept");
    assert_eq!((nb.end.frame, nb.end.tick, rec["result"].as_str()), (10, 60, Some("kept")));
    // the door's first click dropped: the second sets V_DONE, the game does not end; the cheap replay says so
    let c1 = Cand { what: Drop::Steps(vec![1]), kind: "step", waste: 0 };
    let (nb1, rec) = reduce::attempt(&c1, &best, &cx).unwrap();
    assert!(nb1.is_none() && rec["result"] == "cheap replay failed", "{rec}");
    // earlier, but its version not verified: not kept
    let no = version(true);
    let cx2 = reduce::Ctx { version: &no, ..cx };
    let (nb2, rec) = reduce::attempt(&c0, &best, &cx2).unwrap();
    assert!(nb2.is_none() && rec["result"] == "not verified", "{rec}");
    // the rounds: the window's drop kept in round 1, round 2 gains nothing and stops
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let (b, recs) = reduce::rounds(best, &by_k, 3, deadline, &cx);
    assert_eq!((b.end.tick, b.dropped.iter().copied().collect::<Vec<_>>()), (60, vec![0]));
    assert_eq!(recs.len(), 2);
    assert_eq!(recs[0]["kept"], 1);
    assert_eq!(recs[1]["gain_ticks"], 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A host whose boot number `fail` (from 0) fails as an engine that does not answer.
struct Flaky {
    inner: FakeHost,
    boots: std::sync::atomic::AtomicUsize,
    fail: usize,
}

impl EngineHost for Flaky {
    fn boot(&self, cfg: &BootCfg) -> EResult<Box<dyn Engine>> {
        if self.boots.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == self.fail {
            return Err(EngineError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "the engine did not answer")));
        }
        self.inner.boot(cfg)
    }
}

#[test]
fn a_reduction_tests_engine_error_fails_that_test_alone() {
    let host = drop_host();
    let (all, origs) = drop_steps();
    let dir = std::env::temp_dir().join(format!("fmi-shorten-g3e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rc = RCfg { scan: Cfg { workers: 2, chunk: 4 }, skips: false, leaves: 10, sentence: Default::default(), dialog: Default::default() };
    let cfg = pass::PassCfg { seed: 1, scan: Cfg { workers: 2, chunk: 4 }, out: dir.join("e"), w: 50, refind: Some(rc) };
    let p = pass::forward(&host, &[], &all, &origs, &cfg).unwrap();
    let f = p.end.clone().expect("the end");
    let best = Best { pass: p, end: End { tick: f.tick, frame: f.frame, reason: "restart@1:2:3:0004".into() }, path: dir.join("route-001.jsonl"), dropped: Default::default() };
    let specs = |d: &std::collections::BTreeSet<usize>| -> Result<(Vec<StepSpec>, Vec<pass::Orig>), String> {
        Ok(all.iter().cloned().zip(origs.iter().cloned()).filter(|(s, _)| !d.contains(&s.k)).unzip())
    };
    let version = |p: &pass::Pass, _d: &std::collections::BTreeSet<usize>| -> Result<(Option<End>, std::path::PathBuf, serde_json::Value), String> {
        Ok((movie::replay(&host, &p.route, 1, &[], 1000, &dir.join("stage"))?.end, dir.join("route-002.jsonl"), serde_json::json!({})))
    };
    let by_k: BTreeMap<usize, pass::Orig> = all.iter().zip(&origs).map(|(s, o)| (s.k, o.clone())).collect();
    // round 1: the window's drop (boots 0 and 1: the cheap replay's and the pass's) kept; the door's drop, its boot
    // failing: an error, recorded, and the round goes on; round 2: the door's drop again, no gain, the rounds stop
    let flaky = Flaky { inner: drop_host(), boots: 0.into(), fail: 2 };
    let cx = reduce::Ctx { host: &flaky, boot: &[], specs: &specs, cfg: &cfg, out: &dir, version: &version };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let (b, recs) = reduce::rounds(best, &by_k, 3, deadline, &cx);
    assert_eq!((b.end.tick, b.dropped.iter().copied().collect::<Vec<_>>()), (60, vec![0]));
    assert_eq!((recs.len(), &recs[0]["kept"], &recs[0]["errors"], &recs[0]["tried"]), (2, &1.into(), &1.into(), &2.into()), "{}", recs[0]);
    let t = &recs[0]["tests"][1];
    assert_eq!((&t["drop"]["steps"], t["result"].as_str()), (&serde_json::json!([1]), Some("error")), "{t}");
    assert!(t["error"].as_str().is_some_and(|m| m.contains("did not answer")), "{t}");
    assert_eq!(recs[1]["errors"], 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_input_drop_whose_plan_changed_is_stale() {
    let (all, origs) = drop_steps();
    let (w, d) = (all[0].inputs[0].clone(), door_click());
    let plan = |inputs: Vec<In>| pass::Plan { inputs, skips_off: false, how: How::Scan };
    let plans = |inputs: Vec<In>| -> BTreeMap<usize, pass::Plan> { [(1, plan(inputs))].into_iter().collect() };
    let input = |i| Cand { what: Drop::Input { k: 1, i }, kind: "input", waste: 0 };
    let start = plans(vec![w.clone(), d.clone()]);
    // the plan unchanged: the index names the same input
    assert!(!reduce::stale(&input(1), &start, &start));
    // the step placed anew by other inputs (the door alone; the two the other way round): index 1 names another input
    for now in [plans(vec![d.clone()]), plans(vec![d.clone(), w.clone()])] {
        assert!(reduce::stale(&input(1), &start, &now));
        assert!(!reduce::stale(&Cand { what: Drop::Steps(vec![1]), kind: "step", waste: 0 }, &start, &now));
    }
    // a test of an input the best's plan does not hold: stale, no test of the route unchanged
    let host = drop_host();
    let dir = std::env::temp_dir().join(format!("fmi-shorten-g3s-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rc = RCfg { scan: Cfg { workers: 2, chunk: 4 }, skips: false, leaves: 10, sentence: Default::default(), dialog: Default::default() };
    let cfg = pass::PassCfg { seed: 1, scan: Cfg { workers: 2, chunk: 4 }, out: dir.join("e"), w: 50, refind: Some(rc) };
    let p = pass::forward(&host, &[], &all, &origs, &cfg).unwrap();
    let f = p.end.clone().expect("the end");
    let best = Best { pass: p, end: End { tick: f.tick, frame: f.frame, reason: "restart@1:2:3:0004".into() }, path: dir.join("route-001.jsonl"), dropped: Default::default() };
    let specs = |d: &std::collections::BTreeSet<usize>| -> Result<(Vec<StepSpec>, Vec<pass::Orig>), String> {
        Ok(all.iter().cloned().zip(origs.iter().cloned()).filter(|(s, _)| !d.contains(&s.k)).unzip())
    };
    let version = |_p: &pass::Pass, _d: &std::collections::BTreeSet<usize>| -> Result<(Option<End>, std::path::PathBuf, serde_json::Value), String> {
        Err("no version expected".into())
    };
    let cx = reduce::Ctx { host: &host, boot: &[], specs: &specs, cfg: &cfg, out: &dir, version: &version };
    assert_eq!(best.plans()[&1].inputs.len(), 1);
    let (nb, rec) = reduce::attempt(&input(1), &best, &cx).unwrap();
    assert!(nb.is_none() && rec["result"] == "stale", "{rec}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_step_completed_after_the_end_is_recorded_and_reported_unchecked() {
    // the forward pass's route (a_forward_pass_and_its_route_verified): the end at frame 16; a step whose completion,
    // found on its own fork, is at 20 has no digest: its step log line says so and verify reports it, still ok
    let rule: Vec<(u64, u64)> = [(7, 3), (30, 3)].into_iter().chain((12..40).map(|f| (f, 4))).collect();
    let host = FakeHost(rule);
    let specs = vec![spec(), StepSpec { k: 1, inputs: vec![door_click()], goal: Goal::End("restart@1:2:3:0004".into()), relaxed: None }];
    let origs = vec![
        pass::Orig { from: 2, frame: 40, tick: 240, rooms: vec![1], inputs: 1, skipped: vec![] },
        pass::Orig { from: 30, frame: 60, tick: 360, rooms: vec![1], inputs: 1, skipped: vec![] },
    ];
    let dir = std::env::temp_dir().join(format!("fmi-shorten-g3a-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = pass::PassCfg { seed: 1, scan: Cfg { workers: 2, chunk: 4 }, out: dir.join("e"), w: 50, refind: None };
    let p = pass::forward(&host, &[], &specs, &origs, &cfg).unwrap();
    let stage = movie::replay(&host, &p.route, 1, &[10, 20], 100, &dir.join("stage")).unwrap();
    assert_eq!((stage.end.as_ref().map(|e| e.frame), stage.digests.len()), (Some(16), 1));
    let rows = vec![
        serde_json::json!({"k": 0, "completion": {"frame": 10, "tick": 60}, "digest": format!("{:016x}", stage.digests[&10])}),
        serde_json::json!({"k": 5, "completion": {"frame": 20, "tick": 120}, "digest": null}),
        serde_json::json!({"k": 1, "completion": {"frame": 16, "tick": 96}, "digest": null}),
    ];
    let endv = serde_json::json!({"frame": 16, "tick": 96, "reason": "restart@1:2:3:0004"});
    let meta = serde_json::json!({"seed": 1, "goal_end": "restart@1:2:3:0004"});
    let r = movie::write(&dir, "unverified", &p.route, &meta, &rows, &endv).unwrap();
    let log: Vec<serde_json::Value> = std::fs::read_to_string(movie::steps_path(&r)).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let after: Vec<bool> = log[1..4].iter().map(|v| v["after_end"] == true).collect();
    assert_eq!(after, vec![false, true, false]);
    let alone = |route: &[Event], seed: u32, _max: u64, d: &Path| -> Result<Option<End>, String> {
        Ok(movie::replay(&host, route, seed, &[], 100, d)?.end)
    };
    let v = movie::verify(&host, &alone, &r, &dir.join("v")).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    assert_eq!(v["unchecked"], serde_json::json!([{"k": 5, "frame": 20, "why": "after the end"}]), "{v}");
    let _ = std::fs::remove_dir_all(&dir);
}
