//! E013 component `free`: the free try's offers (`free::explore`) on a toy engine. The offer points (the entry, then
//! the wait's stops), the delays on an input that consumed more random draws than the no-input twin (the outcome
//! found only at a later frame), one more level of offers from each outcome kept, IW(1) within the try, the dead moves,
//! the kept entries' movies (each replays from the entry to its checkpoints), the guard's kill, and the record that
//! stall reads (result.json "free": "points" with "where", "level", "seconds", "record"; "delays"."seconds").

use fmi_core::*;
use fmi_explore::afford;
use fmi_explore::free;
use fmi_explore::mech::{Stop, TestedT, TryNovel, WaitPort, Waited};
use fmi_explore::progress::TAtom;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// A toy game behind the Engine port. Verbs "open" (id 2) and "push" (id 3). Room 3: a door (object 100; opening it
/// changes the room to 7 six frames later) and a lever (102; pushing it draws twice from the random source and sets
/// bit 11 when the push lands on a frame divisible by 5; clicking it alone quits the game). Room 7: a chest (104;
/// opening it sets bit 12). The random source steps once every frame (a timer's draw).
#[derive(Clone)]
struct Toy {
    st: Status,
    pending: Vec<Event>,
    bits: Vec<u32>,
    busy: u64,
    verb: i32,
    room_at: Option<(u64, i32)>,
    end_at: Option<u64>,
    /// the end's reason ("quit", or with E014's site "quit@where:number:room:offset")
    end_reason: &'static str,
}

fn toy(frame: u64) -> Toy {
    let mut t = Toy {
        st: Status { frame, room: 3, reason: "frame".into(), xstart: 0, screen: Some((0, 144, false)), rng: 12345, ..Default::default() },
        pending: vec![],
        bits: vec![],
        busy: 0,
        verb: 0,
        room_at: None,
        end_at: None,
        end_reason: "quit",
    };
    t.st.idle = t.idle();
    t
}

fn rect_hit(x: i32, y: i32, r: (i32, i32, i32, i32)) -> bool {
    r.0 <= x && x < r.0 + r.2 && r.1 <= y && y < r.1 + r.3
}

impl Toy {
    fn idle(&self) -> Option<String> {
        (self.busy == 0).then(|| "sentence".to_string())
    }
    fn verbs(&self) -> Vec<Verb> {
        let v = |slot, id, x, y| Verb { slot, id, mode: 1, x, y, ..Default::default() };
        vec![v(1, 2, 10, 150), v(2, 3, 60, 150)]
    }
    fn objects(&self) -> Vec<RoomObject> {
        let o = |id, x, y, w, h| RoomObject { id, x, y, w, h, ..Default::default() };
        if self.st.room == 7 {
            vec![o(104, 100, 40, 20, 20)]
        } else {
            vec![o(100, 50, 40, 20, 20), o(102, 200, 40, 10, 10)]
        }
    }
    fn at_(&self, x: i32, y: i32) -> At {
        let mut a = At { x, y, zone: 0, ..Default::default() };
        if y >= 144 {
            a.zone = 2;
            a.verb_slot = self.verbs().iter().find(|v| rect_hit(x, y, (v.x, v.y, 40, 8))).map(|v| v.slot).unwrap_or(0);
        } else {
            let obj = self.objects().iter().find(|o| rect_hit(x, y, (o.x, o.y, o.w, o.h))).map(|o| o.id).unwrap_or(0);
            a.main = Some((x, y, obj, 0));
        }
        a
    }
    fn click(&mut self, x: i32, y: i32) {
        let a = self.at_(x, y);
        let f = self.st.frame;
        if a.verb_slot > 0 {
            self.verb = self.verbs().iter().find(|v| v.slot == a.verb_slot).unwrap().id;
            self.busy = 1;
            return;
        }
        let Some((_, _, obj, _)) = a.main else { return };
        match (self.verb, obj) {
            (2, 100) => {
                self.room_at = Some((f + 6, 7));
                self.busy = 8;
            }
            (3, 102) => {
                self.st.rng = free::rng_step(free::rng_step(self.st.rng as u32)) as u64;
                if f % 5 == 0 {
                    self.bits.push(11);
                }
                self.busy = 3;
            }
            (0, 102) => {
                self.end_at = Some(f + 2);
                self.busy = 5;
            }
            (2, 104) => {
                self.bits.push(12);
                self.busy = 3;
            }
            (_, 0) => self.busy = 4,
            _ => self.busy = 2,
        }
        self.verb = 0;
    }
    fn step(&mut self) -> EResult<()> {
        self.st.frame += 1;
        let f = self.st.frame;
        self.st.tick += 6;
        self.st.rng = free::rng_step(self.st.rng as u32) as u64;
        self.busy = self.busy.saturating_sub(1);
        if self.end_at == Some(f) {
            return Err(EngineError::Ended(Box::new(Status { reason: self.end_reason.into(), ended: true, frame: f, tick: self.st.tick, ..Default::default() })));
        }
        if let Some((at, room)) = self.room_at {
            if at == f {
                self.st.room = room;
                self.room_at = None;
            }
        }
        let now: Vec<Event> = self.pending.iter().filter(|e| e.frame == f).cloned().collect();
        self.pending.retain(|e| e.frame != f);
        for e in now {
            if let Input::Click(x, y) = e.what {
                self.click(x, y);
            }
        }
        self.st.idle = self.idle();
        Ok(())
    }
}

impl Engine for Toy {
    fn pid(&self) -> u32 {
        1
    }
    fn status(&self) -> &Status {
        &self.st
    }
    fn run(&mut self, spec: &RunSpec) -> EResult<Status> {
        if let Some(f) = spec.frame {
            if f <= self.st.frame {
                return Err(EngineError::Bridge(format!("frame {f} is not after the current frame {}", self.st.frame)));
            }
        }
        loop {
            self.step()?;
            let reason = if spec.frame == Some(self.st.frame) {
                "frame"
            } else if spec.idle && self.st.idle.is_some() {
                "idle"
            } else if spec.max_frame == Some(self.st.frame) {
                "max_frame"
            } else {
                continue;
            };
            self.st.reason = reason.into();
            return Ok(self.st.clone());
        }
    }
    fn inputs(&mut self, events: &[Event]) -> EResult<()> {
        self.pending.extend_from_slice(events);
        Ok(())
    }
    fn obs(&mut self, _req: &ObsReq) -> EResult<Obs> {
        let mut bits = self.bits.clone();
        bits.sort_unstable();
        bits.dedup();
        Ok(Obs {
            tick: self.st.tick,
            frame: self.st.frame,
            room: self.st.room,
            vars: vec![0; 60],
            bits_set: bits,
            room_objects: Some(self.objects()),
            verbs: self.verbs(),
            ..Default::default()
        })
    }
    fn facts(&mut self) -> EResult<Facts> {
        Ok(facts_of(&self.obs(&ObsReq::default())?))
    }
    fn holds(&mut self, _c: &[Cond]) -> EResult<bool> {
        Ok(false)
    }
    fn at(&mut self, x: i32, y: i32) -> EResult<At> {
        Ok(self.at_(x, y))
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

/// The wait after each trial: no more stops (counted).
struct NoStops(AtomicUsize);

impl WaitPort for NoStops {
    fn wait(&self, _root: &mut Box<dyn Engine>, _atoms: &[(Key, Vec<Val>)], _rd: &dyn ReadingSource, _idle: u64) -> Result<Waited, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Waited::default())
    }
    fn last_due(&self, _o: &Obs) -> Option<u64> {
        None
    }
}

/// The free step reads no reading itself (the wait port does).
struct NoReading;

impl ReadingSource for NoReading {
    fn id(&self) -> [u8; 20] {
        unimplemented!()
    }
    fn dir(&self) -> &Path {
        unimplemented!()
    }
    fn head(&self) -> &str {
        unimplemented!()
    }
    fn packed(&self) -> &[u8] {
        unimplemented!()
    }
    fn n_actions(&self) -> u32 {
        unimplemented!()
    }
    fn action_line(&self, _i: u32) -> &str {
        unimplemented!()
    }
    fn by_name(&self, _name: &str) -> Option<u32> {
        unimplemented!()
    }
    fn n_lists(&self) -> u32 {
        unimplemented!()
    }
    fn list_line(&self, _i: u32) -> &str {
        unimplemented!()
    }
    fn reached(&self) -> &str {
        unimplemented!()
    }
    fn domains(&self) -> &Domains {
        unimplemented!()
    }
    fn goal(&self) -> &[(Key, Vec<Val>)] {
        unimplemented!()
    }
    fn start(&self) -> &Facts {
        unimplemented!()
    }
    fn action(&self, _i: u32) -> Action {
        unimplemented!()
    }
    fn record(&self, _i: u32) -> Record {
        unimplemented!()
    }
}

const F0: u64 = 100;

fn boxed(t: Toy) -> Box<dyn Engine> {
    Box::new(t)
}

/// T: bits 9, 11 and 12 set (and the rooms, by themselves).
fn tested() -> TestedT {
    TestedT::new(vec![(Key::bit(9), vec![1]), (Key::bit(11), vec![1]), (Key::bit(12), vec![1])], Value::Null)
}

/// The engine run with no input from the entry to `frame`, and its checkpoint there: a wait's stop.
fn stop_at(frame: u64) -> Stop {
    let mut b = boxed(toy(F0));
    let st = b.run(&RunSpec { frame: Some(frame), ..Default::default() }).unwrap();
    let o = b.obs(&ObsReq::default()).unwrap();
    Stop { frame, checkpoint: afford::checkpoint_of(&st, &o), room: st.room }
}

struct Ran {
    out: free::FreeOut,
    waits: usize,
}

/// The switches are the process's environment: one test at a time, FMI_GOAL_END and E016's FMI_GOAL_SITE unset unless
/// a test sets them.
static ENV: Mutex<()> = Mutex::new(());

fn env(goal_end: bool) -> MutexGuard<'static, ()> {
    env_site(goal_end, false)
}

/// `env` with E016's FMI_GOAL_SITE=verb set (`site_verb`) or unset.
fn env_site(goal_end: bool, site_verb: bool) -> MutexGuard<'static, ()> {
    let g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    if goal_end {
        std::env::set_var("FMI_GOAL_END", "1");
    } else {
        std::env::remove_var("FMI_GOAL_END");
    }
    if site_verb {
        std::env::set_var("FMI_GOAL_SITE", "verb");
    } else {
        std::env::remove_var("FMI_GOAL_SITE");
    }
    g
}

fn explore(stops: &[Stop], cancel: bool) -> Result<Ran, String> {
    explore_with(stops, cancel, "quit")
}

fn explore_with(stops: &[Stop], cancel: bool, end_reason: &'static str) -> Result<Ran, String> {
    let t = tested();
    let atoms: Vec<TAtom> = t.flat.iter().map(|(k, vs)| TAtom::of(*k, vs, None)).collect();
    let mut root = boxed(Toy { end_reason, ..toy(F0) });
    let entry = root.facts().unwrap();
    let nov = TryNovel::new(&json!({}), &atoms, &entry);
    let wait = Arc::new(NoStops(AtomicUsize::new(0)));
    let stop = AtomicBool::new(cancel);
    let cx = free::Ctx { t: &t, keys: &t, nov: &nov, wait: wait.clone(), rd: &NoReading, dead_moves: &[], cancel: &stop };
    let out = free::explore(&mut root, stops, &cx)?;
    // the entry's engine stays at the entry
    assert_eq!(root.status().frame, F0);
    Ok(Ran { out, waits: wait.0.load(Ordering::SeqCst) })
}

/// An entry's movie replayed on a fresh engine from the entry: every checkpoint reached with equal facts, tick and
/// random state (A0 in small).
fn replays(events: &[Value], cps: &[Checkpoint]) -> bool {
    let mut b = boxed(toy(F0));
    let evs: Vec<Event> = events.iter().filter_map(fmi_explore::events::event_of).collect();
    assert_eq!(evs.len(), events.len());
    b.inputs(&evs).unwrap();
    cps.iter().all(|c| {
        let st = b.run(&RunSpec { frame: Some(c.frame), ..Default::default() }).unwrap();
        let o = b.obs(&ObsReq::default()).unwrap();
        afford::checkpoint_of(&st, &o) == *c
    })
}

fn bits(x: &free::FreeOut) -> Vec<Vec<String>> {
    x.kept.iter().map(|k| k.novel.clone()).collect()
}

#[test]
fn the_free_try_offers_delays_and_goes_one_level_deeper() {
    let _g = env(false);
    let r = explore(&[stop_at(F0 + 4)], false).unwrap();
    let out = &r.out;
    // kept, in order: the door opened at the entry (room 7), the lever's bit at a delay of 2 frames (pushed on frame
    // 105, the only delay landing on a multiple of 5), then the chest's bit one level down from room 7
    assert_eq!(bits(out), vec![vec!["room=7".to_string()], vec!["bit:11=1".to_string()], vec!["bit:12=1".to_string()]]);
    assert!(out.kept.iter().all(|k| k.how == "free"));
    // the movies: the entry's clicks; the delayed clicks two frames later; the level-2 clicks after the level-1 ones
    let frames = |k: &fmi_explore::mech::Extra| k.events.iter().map(|e| e["frame"].as_u64().unwrap()).collect::<Vec<_>>();
    assert_eq!(frames(&out.kept[0]), vec![101, 103]);
    assert_eq!(frames(&out.kept[1]), vec![103, 105]);
    let f2 = frames(&out.kept[2]);
    assert_eq!(&f2[..2], &[101, 103]);
    assert!(f2[2] > out.kept[0].frame());
    assert_eq!(out.kept[2].cps.len(), 2);
    assert_eq!(out.kept[2].cps[0], out.kept[0].cps[0]);
    for k in &out.kept {
        assert!(replays(&k.events, &k.cps), "{:?}", k.events);
    }
    // the dead moves: the lever clicked alone quits, from the entry, the stop and the level-2 point at the lever's bit
    assert_eq!(out.dead.len(), 3);
    assert!(out.dead.iter().all(|d| d["reason"] == "quit" && d["how"] == "free"));
    // the record
    let rec = &out.record;
    assert_eq!(rec["n_points"], 4);
    let pts = rec["points"].as_array().unwrap();
    let wl: Vec<(String, i64, u64)> =
        pts.iter().map(|p| (p["where"].as_str().unwrap().to_string(), p["level"].as_i64().unwrap(), p["frame"].as_u64().unwrap())).collect();
    assert_eq!(wl[0], ("entry".into(), 1, F0));
    assert_eq!(wl[1], ("stop".into(), 1, F0 + 4));
    assert_eq!((wl[2].0.as_str(), wl[2].1), ("kept", 2));
    assert_eq!((wl[3].0.as_str(), wl[3].1), ("kept", 2));
    assert!(pts.iter().all(|p| p["record"]["trials"].as_u64().is_some() && p["seconds"].as_f64().is_some()));
    assert!(pts[0]["delays"].is_object() && pts[1]["delays"].is_object() && pts[2].get("delays").is_none());
    assert_eq!(rec["kept"], json!({"1": 1, "2": 1, "delay": 1}));
    assert_eq!(rec["kept_total"], 3);
    assert_eq!(rec["dead"], 3);
    // the delays: the push on the lever made two more draws than no input, at the entry and at the stop; at the stop
    // its bit came again (frame 110) but was kept already (IW(1))
    let d = &rec["delays"];
    assert_eq!(d["delayed_inputs"], 2);
    assert_eq!(d["unknown"], 0);
    assert_eq!(pts[0]["delays"]["inputs"], json!(["v3t102"]));
    assert!(d["seconds"].as_f64().is_some());
    // every trial that stopped ran the wait after it once (no dedupe without refuse)
    let stopped: u64 = rec["trials"].as_u64().unwrap() - pts.iter().map(|p| p["record"]["ended"].as_u64().unwrap_or(0)).sum::<u64>();
    assert_eq!(r.waits as u64 + rec["waits_reused"].as_u64().unwrap(), r.waits as u64);
    assert!(r.waits > 0 && rec["waits_run"] == r.waits);
    assert!(stopped >= r.waits as u64);
    // the offers: 2 verbs and 2 targets in room 3 (8 inputs), 2 verbs and 1 target in room 7 (5)
    assert_eq!(pts[0]["record"]["trials"], 8);
    assert_eq!(rec["offers"], 4);
}

#[test]
fn without_stops_or_kept_outcomes_one_point() {
    let _g = env(false);
    // a wait with no stops: the entry's point only, plus the levels below what it keeps
    let r = explore(&[], false).unwrap();
    let pts = r.out.record["points"].as_array().unwrap().clone();
    assert_eq!(pts[0]["where"], "entry");
    assert!(pts.iter().skip(1).all(|p| p["where"] == "kept"));
    assert_eq!(bits(&r.out)[0], vec!["room=7".to_string()]);
}

#[test]
fn the_guards_kill_stops_the_step() {
    let _g = env(false);
    let e = explore(&[stop_at(F0 + 4)], true).err().unwrap();
    assert_eq!(e, "the guard's kill");
}

#[test]
fn a_stop_not_reached_again_is_an_error() {
    let _g = env(false);
    let mut s = stop_at(F0 + 4);
    s.checkpoint.tick += 1;
    let e = explore(&[s], false).err().unwrap();
    assert!(e.contains("is not reached again from the entry"), "{e}");
}

#[test]
fn with_the_goal_end_a_scripts_end_is_also_an_end_entry() {
    // E014's FMI_GOAL_END=1: a trial that a script ended (the end's site named) is a dead move and an end entry, as in
    // the affordance step; no offers are made from an end entry
    let _g = env(true);
    let r = explore_with(&[stop_at(F0 + 4)], false, "quit@3:206:62:0AEF").unwrap();
    let out = &r.out;
    let hows: Vec<&str> = out.kept.iter().map(|k| k.how).collect();
    assert_eq!(hows.iter().filter(|h| **h == "free").count(), 3);
    assert_eq!(hows.iter().filter(|h| **h == "end").count(), 3);
    assert_eq!(out.dead.len(), 3);
    assert_eq!(out.record["kept"]["end"], 3);
    assert_eq!(out.record["n_points"], 4);
    for k in out.kept.iter().filter(|k| k.how == "end") {
        let c = k.cps.last().unwrap();
        assert_eq!(c.facts.val(Key::parse("end")), 1);
        assert_eq!(c.facts.val(Key::parse("end:kind")), 3);
        assert_eq!(k.novel, vec!["end".to_string()]);
        // its clicks end with the lever's, two frames before the end
        assert_eq!(k.events.last().unwrap()["frame"].as_u64().unwrap() + 2, k.frame());
    }
    // the same run without a site: dead moves only
    drop(_g);
    let _g = env(true);
    let r = explore_with(&[stop_at(F0 + 4)], false, "quit").unwrap();
    assert!(r.out.kept.iter().all(|k| k.how == "free"));
    assert_eq!(r.out.dead.len(), 3);
}

#[test]
fn g5_a_forked_global_restart_is_no_goal_entry_with_the_verb_site() {
    // E016 gate G5 (section E, FMI_GOAL_SITE=verb): a trial (a fork of the entry) that ends at a global script's
    // restart, as a death screen restarts on its override key (the site's `where` 2: a global's slot), is a dead move
    // and no end entry, so no goal entry; without the switch it is an end entry, as in E014
    let global = "restart@2:9:-1:0100";
    let _g = env_site(true, true);
    let r = explore_with(&[stop_at(F0 + 4)], false, global).unwrap();
    assert!(r.out.kept.iter().all(|k| k.how == "free"), "{:?}", r.out.kept.iter().map(|k| k.how).collect::<Vec<_>>());
    assert!(r.out.kept.iter().all(|k| k.cps.iter().all(|c| c.facts.get(Key::parse("end")).is_none())));
    assert_eq!(r.out.dead.len(), 3);
    assert!(r.out.dead.iter().all(|d| d["reason"] == global));
    assert!(r.out.record["kept"].get("end").is_none());
    drop(_g);
    let _g = env_site(true, false);
    let r = explore_with(&[stop_at(F0 + 4)], false, global).unwrap();
    assert_eq!(r.out.kept.iter().filter(|k| k.how == "end").count(), 3);
}

#[test]
fn g5_an_object_verb_handlers_restart_is_the_goal_entry_with_the_verb_site() {
    // E016 gate G5: the same trial ending at a restart that the clicked object's own verb handler runs (the lever,
    // object 102 in room 3; the site's `where` 1: a room object's slot) is an end entry whose checkpoint holds the
    // end's facts (`end` = 1, the stage goal end=1), with the switch on
    let _g = env_site(true, true);
    let r = explore_with(&[stop_at(F0 + 4)], false, "restart@1:102:3:0010").unwrap();
    let ends: Vec<&fmi_explore::mech::Extra> = r.out.kept.iter().filter(|k| k.how == "end").collect();
    assert_eq!(ends.len(), 3);
    assert_eq!(r.out.dead.len(), 3);
    for k in ends {
        let c = k.cps.last().unwrap();
        assert_eq!(c.facts.val(Key::parse("end")), 1);
        assert_eq!(c.facts.val(Key::parse("end:kind")), 1);
        assert_eq!(c.facts.val(Key::parse("end:where")), 1);
        assert_eq!(c.facts.val(Key::parse("end:number")), 102);
        assert_eq!(c.facts.val(Key::parse("end:room")), 3);
        assert_eq!(c.facts.val(Key::parse("end:offset")), 0x10);
        assert_eq!(k.novel, vec!["end".to_string()]);
    }
    // an inventory object's handler (`where` 0) and a floating object's (4) are verb handlers too; a room's local
    // script (3) and a global (2) are not
    for (reason, verb) in [("restart@0:102:-1:0010", true), ("quit@4:102:3:0010", true), ("restart@3:206:3:0010", false),
                           ("restart@2:9:-1:0100", false)] {
        let f = end_facts(reason).unwrap();
        assert_eq!(fmi_explore::mech::end_from_verb(&f), verb, "{reason}");
        assert_eq!(fmi_explore::mech::end_extra(reason, 200, 1200, vec![], &[]).is_some(), verb, "{reason}");
    }
}

// --- E016 D: a free try never reads (FMI_FREE_T=archive) ------------------------------------------------------------

/// A registered reading of another scope, as it stands in for the wait: its folder, a reading.json and no task facts.
struct Registered {
    dir: std::path::PathBuf,
    head: String,
    doms: Domains,
    start: Facts,
}

impl ReadingSource for Registered {
    fn id(&self) -> [u8; 20] {
        [7; 20]
    }
    fn dir(&self) -> &Path {
        &self.dir
    }
    fn head(&self) -> &str {
        &self.head
    }
    fn packed(&self) -> &[u8] {
        unimplemented!()
    }
    fn n_actions(&self) -> u32 {
        0
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

/// The readings: open gives a registered reading; a fresh reading is counted and refused.
struct Readings1 {
    reads: AtomicUsize,
}

impl fmi_explore::ports::Readings for Readings1 {
    fn open(&self, p: &Path) -> Result<Arc<dyn ReadingSource>, String> {
        let dir = p.parent().unwrap().to_path_buf();
        let head = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
        Ok(Arc::new(Registered { dir, head, doms: Domains::default(), start: Facts::new() }))
    }
    fn reuse_keys(&self, _rd: &dyn ReadingSource) -> Result<std::collections::HashSet<String>, String> {
        Ok(Default::default())
    }
    fn read(&self, _req: &fmi_explore::ports::ReadReq, _c: &AtomicBool) -> Result<std::path::PathBuf, String> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Err("this test reads nothing".into())
    }
}

struct ToyReturns;

impl fmi_explore::ports::Returns for ToyReturns {
    fn return_to(&mut self, e: &fmi_explore::ports::EntryRef, _out: &Path) -> EResult<Box<dyn Engine>> {
        assert_eq!(e.frame, F0);
        Ok(boxed(toy(F0)))
    }
}

struct NoPlanner;

impl PlannerHost for NoPlanner {
    fn planner(&self, _rd: &dyn ReadingSource) -> Result<Box<dyn Planner>, PlanError> {
        Err(PlanError::Taskcore("a free try plans nothing".into()))
    }
}

struct NoPlayer;

impl fmi_explore::ports::PlayerHost for NoPlayer {
    fn player(&self, _cfg: &fmi_explore::ports::WindowCfg) -> Box<dyn fmi_explore::ports::WindowPlayer> {
        unimplemented!("a free try plays no plan")
    }
}

struct TestedToy;

impl fmi_explore::mech::TestedPort for TestedToy {
    fn tested(&self, _rd: &dyn ReadingSource) -> Result<TestedT, String> {
        Ok(tested())
    }
}

/// A free try's job dir from a non-root entry at the toy's frame F0: no cached reading of its scope, and the registered
/// readings of other scopes `any`.
fn free_job(dir: &Path, any: &[std::path::PathBuf]) -> std::path::PathBuf {
    let mut b = boxed(toy(F0));
    let o = b.obs(&ObsReq::default()).unwrap();
    let cp = afford::checkpoint_of(b.status(), &o);
    let jd = dir.join("jobs").join("j00003");
    std::fs::create_dir_all(&jd).unwrap();
    let job = json!({"job": "j00003", "seed": 1, "stage": "test", "goal": {"bit:12": [1]},
                     "entry": {"id": 4, "root": false, "frame": F0, "events": [], "checkpoint": fmi_explore::harness::checkpoint_json(&cp)},
                     "scope": [3], "tried": [], "held": [], "readings": [],
                     "reading_dir": dir.join("readings").join("3-j00003").to_string_lossy(), "threads": 1, "workers": 1,
                     "kind": "free", "readings_room": [],
                     "readings_any": any.iter().map(|p| json!({"path": p.to_string_lossy(), "start_entry": 0, "scope": [7]})).collect::<Vec<_>>()});
    std::fs::write(jd.join("job.json"), job.to_string()).unwrap();
    jd
}

fn resident(reads: Arc<Readings1>) -> fmi_explore::tryjob::Resident {
    fmi_explore::tryjob::Resident {
        returns: Mutex::new(Box::new(ToyReturns)),
        readings: reads,
        planners: Mutex::new(fmi_explore::harness::PlannerCache::new(Arc::new(NoPlanner))),
        players: Arc::new(NoPlayer),
        search: Search::Gbfs,
        search_name: "gbfs".into(),
        forced_targets: None,
        tail: false,
        mech: fmi_explore::mech::Mech { tested: Some(Arc::new(TestedToy)), wait: Some(Arc::new(NoStops(AtomicUsize::new(0)))) },
    }
}

fn run_free(any: bool, t_archive: bool) -> (Value, usize) {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("e016-d-{}-{}-{}", std::process::id(), any, t_archive));
    let _ = std::fs::remove_dir_all(&dir);
    let rdir = dir.join("readings").join("7-j00001");
    std::fs::create_dir_all(&rdir).unwrap();
    let rj = rdir.join("reading.json");
    std::fs::write(&rj, json!({"scope": {"rooms": [7]}, "start": {"name": "s", "facts": {}}, "n_records": 1, "n_actions": 1}).to_string()).unwrap();
    let jd = free_job(&dir, if any { std::slice::from_ref(&rj) } else { &[] });
    let reads = Arc::new(Readings1 { reads: AtomicUsize::new(0) });
    let r = resident(reads.clone());
    if t_archive {
        std::env::set_var("FMI_FREE_T", "archive");
    }
    let log = fmi_explore::driver::JobLog::create(&jd.join("log")).unwrap();
    let out = fmi_explore::tryjob::run_job(&jd, &r, &log, &AtomicBool::new(false));
    std::env::remove_var("FMI_FREE_T");
    (out, reads.reads.load(Ordering::SeqCst))
}

#[test]
fn e016_d_a_free_try_reads_nothing_and_a_registered_reading_stands_in() {
    // E016 section D (FMI_FREE_T=archive): a free try whose scope has no cached reading starts no reading. A registered
    // reading of another scope stands in for the wait; T is the archive's union with the stand-in's own; the free step
    // runs and keeps the door's room as before
    let _g = env(false);
    let (out, reads) = run_free(true, true);
    assert_eq!(reads, 0);
    assert_eq!(out["error"], Value::Null, "{out}");
    assert!(out["reading"]["why"].as_str().unwrap().starts_with("T from the archive"), "{}", out["reading"]);
    assert_eq!(out["reading"]["fresh"], json!(false));
    assert!(out["reading"]["path"].as_str().unwrap().ends_with("7-j00001/reading.json"));
    assert_eq!(out["e012"]["t"]["free_t"], json!("archive"));
    assert!(out["free"]["trials"].as_u64().unwrap() > 0);
    assert!(out["new"].as_array().unwrap().iter().any(|n| n["novel"] == json!(["room=7"])));
    assert_eq!(out["exhausted"], json!(false));
}

#[test]
fn e016_d_with_no_registered_reading_the_free_try_keeps_nothing() {
    let _g = env(false);
    let (out, reads) = run_free(false, true);
    assert_eq!(reads, 0);
    assert_eq!(out["ok"], json!(true));
    assert!(out["reading"]["why"].as_str().unwrap().ends_with("no registered reading"), "{}", out["reading"]);
    assert!(out["new"].as_array().unwrap().is_empty());
    assert!(out.get("free").is_none());
}

#[test]
fn e016_d_off_the_free_try_reads_its_scope() {
    // the switch unset: a free try with no reading of its scope asks for a fresh one (E013), here refused
    let _g = env(false);
    let (out, reads) = run_free(true, false);
    assert_eq!(reads, 1);
    assert!(out["error"].as_str().unwrap().contains("this test reads nothing"), "{out}");
}
