//! E012 afford's unit tests on a toy engine: the offer (hit points, the untouchable class, actors), the inputs (dialog,
//! sentence, not idle), composing and the third click, kept by novelty with IW(1) within the step, a run that ends,
//! M4's form (outcomes against no input), `ready`, and the `then` stops.

use fmi_core::*;
use fmi_explore::afford::{self, AffordCfg, Hit, Inputs, Novelty, PairNovelty};

const INPUT_SCRIPT: i32 = 4;
const SENTENCE_SCRIPT: i32 = 2;

/// A toy game behind the Engine port. Verbs "open" (id 2) and "use" (id 3) on the verb screen (y >= 144); objects 100
/// (a door: opening it, or walking to it, changes the room 6 frames later), 101 (untouchable, class 32) and 102; actor
/// 1. "use" composes: use 100 then 102 sets bit 9. Walking to 102 quits the game. Var 50 changes every frame. In a
/// dialog (`dialog`), lines 120 (ends it, bit 5) and 121 (bit 6, stays).
#[derive(Clone)]
struct Toy {
    st: Status,
    pending: Vec<Event>,
    vars: Vec<i32>,
    bits: Vec<u32>,
    busy: u64,
    verb: i32,
    obj1: i32,
    segment: bool,
    runs: Vec<Run>,
    room_at: Option<(u64, i32)>,
    end_at: Option<u64>,
    dialog: bool,
    /// Boxes commands answered
    boxes_asked: std::sync::Arc<std::sync::atomic::AtomicU32>,
    /// E013: the input log (a click that changes no state is "ignored")
    log: Vec<InputRecord>,
    /// E014 amendment (7): the bridge's input check (patch 0002: a click needs 0 <= x, y < 0x10000); off, every
    /// input is scheduled
    strict: bool,
}

fn toy(frame: u64, dialog: bool) -> Toy {
    let mut vars = vec![0; 60];
    vars[afford::VAR_VERB_SCRIPT] = INPUT_SCRIPT;
    vars[afford::VAR_SENTENCE_SCRIPT] = SENTENCE_SCRIPT;
    let mut t = Toy {
        st: Status { frame, room: 3, reason: "frame".into(), xstart: 0, screen: Some((0, 144, false)), ..Default::default() },
        pending: vec![],
        vars,
        bits: vec![],
        busy: 0,
        verb: 0,
        obj1: 0,
        segment: false,
        runs: vec![],
        room_at: None,
        end_at: None,
        dialog,
        boxes_asked: Default::default(),
        log: vec![],
        strict: false,
    };
    t.st.idle = t.idle();
    t
}

fn rect_hit(x: i32, y: i32, r: (i32, i32, i32, i32)) -> bool {
    r.0 <= x && x < r.0 + r.2 && r.1 <= y && y < r.1 + r.3
}

impl Toy {
    /// The state a digest reads (every field the toy's future depends on; the frame counters aside).
    fn state_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.st.room, &self.vars, &self.bits, self.busy, self.verb, self.obj1, self.dialog, self.room_at, self.end_at).hash(&mut h);
        self.pending.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>().hash(&mut h);
        h.finish()
    }
    fn idle(&self) -> Option<String> {
        if self.busy > 0 {
            None
        } else if self.dialog {
            Some("dialog".into())
        } else {
            Some("sentence".into())
        }
    }
    fn verbs(&self) -> Vec<Verb> {
        let v = |slot, id, x, y| Verb { slot, id, mode: 1, x, y, ..Default::default() };
        if self.dialog {
            vec![v(3, 120, 10, 160), v(4, 121, 10, 170)]
        } else {
            vec![v(1, 2, 10, 150), v(2, 3, 60, 150)]
        }
    }
    fn objects(&self) -> Vec<RoomObject> {
        let o = |id, x, y, w, h| RoomObject { id, x, y, w, h, ..Default::default() };
        vec![o(100, 50, 40, 20, 20), o(101, 100, 40, 20, 20), o(102, 200, 40, 10, 10)]
    }
    fn at_(&self, x: i32, y: i32) -> At {
        let mut a = At { x, y, zone: 0, ..Default::default() };
        if y >= 144 {
            a.zone = 2;
            a.verb_slot = self.verbs().iter().find(|v| rect_hit(x, y, (v.x, v.y, 40, 8))).map(|v| v.slot).unwrap_or(0);
        } else {
            // the main screen: room x = x + xstart, y = y - topline (the bridge's `at`)
            let (vx, vy) = (x + self.st.xstart, y - self.st.screen.map(|s| s.0).unwrap_or(0));
            let obj = self.objects().iter().filter(|o| o.id != 101).find(|o| rect_hit(vx, vy, (o.x, o.y, o.w, o.h))).map(|o| o.id).unwrap_or(0);
            let actor = if (145..=155).contains(&vx) && (80..120).contains(&vy) { 1 } else { 0 };
            a.main = Some((vx, vy, obj, actor));
        }
        a
    }
    fn run_(&mut self, number: i32) {
        if self.segment {
            self.runs.push(Run { frame: self.st.frame, where_: WIO_GLOBAL, number, ..Default::default() });
        }
    }
    fn click(&mut self, x: i32, y: i32) {
        let a = self.at_(x, y);
        self.run_(INPUT_SCRIPT);
        let f = self.st.frame;
        if a.verb_slot > 0 {
            let id = self.verbs().iter().find(|v| v.slot == a.verb_slot).unwrap().id;
            if self.dialog {
                if id == 120 {
                    self.dialog = false;
                    self.bits.push(5);
                    self.busy = 3;
                } else {
                    self.bits.push(6);
                    self.busy = 5;
                }
            } else {
                self.verb = id;
                self.busy = 1;
            }
            return;
        }
        let Some((vx, vy, obj, actor)) = a.main else { return };
        if obj == 0 && actor == 0 {
            // a floor click: a walk; ending in box 1 (y 100..140, x < 100) leaves by an exit 5 frames later
            self.busy = 4;
            if vx < 100 && (100..140).contains(&vy) {
                self.room_at = Some((f + 5, 9));
                self.busy = 7;
            }
            return;
        }
        if obj == 0 && actor == 1 {
            self.run_(SENTENCE_SCRIPT);
            self.vars[40] += 1;
            self.busy = 2;
            return;
        }
        match (self.verb, obj) {
            (2, 100) | (0, 100) => {
                self.run_(SENTENCE_SCRIPT);
                self.room_at = Some((f + 6, 7));
                self.busy = 8;
            }
            (3, o) if o != 0 && self.obj1 == 0 => {
                self.obj1 = o;
                self.busy = 1;
            }
            (3, o) if o != 0 => {
                self.run_(SENTENCE_SCRIPT);
                if (self.obj1, o) == (100, 102) {
                    self.bits.push(9);
                }
                self.obj1 = 0;
                self.verb = 0;
                self.busy = 3;
            }
            (0, 102) => {
                self.run_(SENTENCE_SCRIPT);
                self.end_at = Some(f + 2);
                self.busy = 5;
            }
            (_, o) if o != 0 => {
                self.run_(SENTENCE_SCRIPT);
                self.verb = 0;
                self.busy = 2;
            }
            _ => {}
        }
    }
    fn step(&mut self) -> EResult<()> {
        self.st.frame += 1;
        let f = self.st.frame;
        self.st.tick += 6;
        self.vars[50] = (f % 7) as i32;
        self.busy = self.busy.saturating_sub(1);
        if self.end_at == Some(f) {
            return Err(EngineError::Ended(Box::new(Status { reason: "quit".into(), ended: true, frame: f, ..Default::default() })));
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
                let before = self.state_key();
                self.click(x, y);
                let verdict = if self.state_key() == before { Verdict::Ignored } else { Verdict::Taken };
                self.log.push(InputRecord {
                    frame: f, input: vec![e.what.clone()], stat: 0, verdict, area: 0, value: 0, mode: 0, slot: 0, zone: 0,
                    key_match: false, handler: INPUT_SCRIPT, writes: vec![], reads: vec![], ops: vec![], ops_dropped: 0,
                    touches_dropped: 0,
                });
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
        if self.strict {
            if let Some(k) = events.iter().position(|e| matches!(e.what, Input::Click(x, y) if x < 0 || y < 0)) {
                return Err(EngineError::Bridge(format!(
                    "events: event {k}: expected frame > 0, and a click/rclick/move in range, a key name or skips")));
            }
        }
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
            vars: self.vars.clone(),
            bits_set: bits,
            classes: vec![(101, vec![32])],
            room_objects: Some(self.objects()),
            verbs: self.verbs(),
            actors: vec![ActorPos { id: 1, x: 150, y: 120, ..Default::default() }],
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
    fn segment(&mut self, on: bool) -> EResult<()> {
        self.segment = on;
        Ok(())
    }
    fn runs(&mut self) -> EResult<RunsReply> {
        Ok(RunsReply { runs: std::mem::take(&mut self.runs), ..Default::default() })
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
    fn input_log(&mut self) -> EResult<InputLog> {
        Ok(InputLog { records: std::mem::take(&mut self.log), dropped: 0, frame: self.st.frame })
    }
    fn digest(&mut self) -> EResult<Digest> {
        Ok(Digest { frame: self.st.frame, digest: self.state_key(), bytes: 0 })
    }
    fn boxes(&mut self) -> EResult<Boxes> {
        self.boxes_asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let b = |x0, y0, x1, y1, flags| WalkBox { ul: (x0, y0), ur: (x1, y0), lr: (x1, y1), ll: (x0, y1), flags };
        Ok(Boxes {
            room: self.st.room,
            boxes: vec![
                b(0, 0, 0, 0, 0),           // box 0: never a walk's end in v5
                b(0, 100, 100, 140, 0),     // the floor, by the exit
                b(200, 100, 300, 140, 0x40), // locked
                b(120, 100, 180, 140, 0x80), // invisible
                b(40, 30, 80, 70, 0x08),    // under object 100 at its centre: a later point is the floor
                b(400, 100, 450, 140, 0),   // off screen
            ],
        })
    }
}

fn boxed(t: Toy) -> Box<dyn Engine> {
    Box::new(t)
}

fn names(v: &[Inputs]) -> Vec<String> {
    v.iter().map(|i| i.name()).collect()
}

/// Novelty over the room and bits 5, 6 and 9 (the toy's world), nothing seen.
fn novelty() -> PairNovelty {
    let mut n = PairNovelty::new([Key::bit(5), Key::bit(6), Key::bit(9)]);
    n.add(&facts_of(&toy(0, false).obs(&ObsReq::default()).unwrap()));
    n
}

#[test]
fn offer_points_and_inputs() {
    let mut e = boxed(toy(100, false));
    let o = e.obs(&ObsReq::default()).unwrap();
    let of = afford::offer(&mut e, &o).unwrap();
    assert_eq!(of.idle.as_deref(), Some("sentence"));
    // verbs: the first candidate (2, 2) from the rectangle's top left hits
    assert_eq!(of.verbs.iter().map(|p| (p.x, p.y, p.hit)).collect::<Vec<_>>(),
               vec![(12, 152, Hit::Verb { slot: 1, id: 2 }), (62, 152, Hit::Verb { slot: 2, id: 3 })]);
    // objects: the untouchable one is left out; the grid's centre first; the actor 20 pixels above its feet
    let t: Vec<(i32, i32, Hit)> = of.targets.iter().map(|p| (p.x, p.y, p.hit)).collect();
    assert_eq!(t, vec![(59, 49, Hit::Object(100)), (204, 44, Hit::Object(102)), (150, 100, Hit::Actor(1))]);
    assert_eq!((of.n_objects, of.n_touchable, of.n_actors), (3, 2, 1));
    let inp = afford::inputs_of(&of);
    assert_eq!(names(&inp), vec!["v2", "v3", "v2t100", "v2t102", "v2t-1", "v3t100", "v3t102", "v3t-1", "t100", "t102", "t-1"]);
    assert_eq!(inp[2].events(100), vec![Event { frame: 101, what: Input::Click(12, 152) }, Event { frame: 103, what: Input::Click(59, 49) }]);
}

#[test]
fn offer_follows_camera_and_topline() {
    let mut t = toy(100, false);
    t.st.xstart = 40;
    t.st.screen = Some((8, 144, false));
    let mut e = boxed(t);
    let o = e.obs(&ObsReq::default()).unwrap();
    let of = afford::offer(&mut e, &o).unwrap();
    // the boxes on screen: x less the camera's xstart, y plus the main screen's topline
    let t: Vec<(i32, i32, Hit)> = of.targets.iter().map(|p| (p.x, p.y, p.hit)).collect();
    assert_eq!(t, vec![(19, 57, Hit::Object(100)), (164, 52, Hit::Object(102)), (110, 108, Hit::Actor(1))]);
}

#[test]
fn dialog_and_not_idle() {
    let mut e = boxed(toy(100, true));
    let o = e.obs(&ObsReq::default()).unwrap();
    let of = afford::offer(&mut e, &o).unwrap();
    assert_eq!(names(&afford::inputs_of(&of)), vec!["v120", "v121"]);
    let mut busy = toy(100, false);
    busy.busy = 4;
    busy.st.idle = busy.idle();
    let mut e = boxed(busy);
    let of = afford::offer(&mut e, &o).unwrap();
    assert!(afford::inputs_of(&of).is_empty());
    // ready: a fork run to the first idle stop, the engine itself left where it was
    let r = afford::ready(&mut e, 100).unwrap();
    let b = r.expect("not idle").expect("an idle stop");
    assert_eq!(b.status().frame, 104);
    assert_eq!(b.status().idle.as_deref(), Some("sentence"));
    assert_eq!(e.status().frame, 100);
    let mut idle = boxed(toy(100, false));
    assert!(afford::ready(&mut idle, 100).unwrap().is_none());
}

#[test]
fn composing_rule() {
    let r = |frame, where_, number| Run { frame, where_, number, ..Default::default() };
    let input = [INPUT_SCRIPT];
    assert!(afford::composing(Some("sentence"), &[r(103, WIO_GLOBAL, 4)], 103, &input, 2));
    // the input script's run before the click does not count
    assert!(!afford::composing(Some("sentence"), &[r(101, WIO_GLOBAL, 4)], 103, &input, 2));
    assert!(!afford::composing(Some("sentence"), &[r(103, WIO_GLOBAL, 4), r(104, WIO_GLOBAL, 2)], 103, &input, 2));
    assert!(!afford::composing(Some("sentence"), &[r(103, WIO_GLOBAL, 4), r(103, WIO_ROOM, 316)], 103, &input, 2));
    // the room's entry code is not an object's verb script
    assert!(afford::composing(Some("sentence"), &[r(103, WIO_GLOBAL, 4), r(103, WIO_ROOM, ENCD)], 103, &input, 2));
    // a local input script (the room's own)
    assert!(afford::composing(Some("sentence"), &[r(103, WIO_LOCAL, 202)], 103, &[4, 202], 2));
    assert!(!afford::composing(Some("dialog"), &[r(103, WIO_GLOBAL, 4)], 103, &input, 2));
    assert!(!afford::composing(None, &[r(103, WIO_GLOBAL, 4)], 103, &input, 2));
}

#[test]
fn trials_kept_by_novelty() {
    let mut e = boxed(toy(100, false));
    let o = e.obs(&ObsReq::default()).unwrap();
    let of = afford::offer(&mut e, &o).unwrap();
    let inp = afford::inputs_of(&of);
    let tested = [Key::bit(5), Key::bit(6), Key::bit(9)];
    let nov = novelty();
    let out = afford::try_all(&mut e, &of, &inp, &tested, &nov, &AffordCfg { horizon: 200, fixed: false, third: true, ..Default::default() }, None).unwrap();
    // the entry is untouched
    assert_eq!(e.status().frame, 100);
    let by: std::collections::HashMap<String, &afford::Trial> = out.trials.iter().map(|t| (t.inputs.name(), t)).collect();
    // use 100 composes: a third click over every target at its stop
    assert!(by["v3t100"].composing);
    assert!(!by["v2t100"].composing);
    assert!(by["v3t102"].composing);
    assert!(!by["v3t-1"].composing);
    let third: Vec<String> = out.trials.iter().filter(|t| t.inputs.clicks.len() == 3).map(|t| t.inputs.name()).collect();
    assert!(third.contains(&"v3t100t102".to_string()), "{third:?}");
    // walking to 102 quits: the trial ended, nothing kept from it
    assert!(by["t102"].ended);
    assert_eq!(by["t102"].reason, "quit");
    // kept: the room (first by open 100, not again by walking to 100: IW(1) within the step), bit 9 by use 100 with 102
    let kept: Vec<(String, Vec<String>)> = out.kept.iter().map(|k| (k.name.clone(), k.novel.clone())).collect();
    assert_eq!(kept, vec![("v2t100".to_string(), vec!["room=7".to_string()]), ("v3t100t102".to_string(), vec!["bit:9=1".to_string()])]);
    // a kept outcome's movie: the trial's clicks at their frames; the third click after its two-click trial's idle stop
    let k = &out.kept[1];
    let s = by["v3t100"].frame;
    assert_eq!(k.events.iter().map(|e| e.frame).collect::<Vec<_>>(), vec![101, 103, s + 1]);
    assert_eq!(k.checkpoint.frame, by["v3t100t102"].frame);
    assert_eq!(k.checkpoint.facts.val(Key::bit(9)), 1);
    // replaying the kept movie from the entry gives the kept checkpoint (the toy is deterministic)
    let mut again = boxed(toy(100, false));
    again.inputs(&k.events).unwrap();
    again.run(&RunSpec { frame: Some(k.checkpoint.frame), ..Default::default() }).unwrap();
    let o2 = again.obs(&ObsReq::default()).unwrap();
    assert_eq!(afford::checkpoint_of(again.status(), &o2), k.checkpoint);
    // outcomes: the animation var 50 is not a world key, so the trials that change nothing share one outcome
    assert!(by["v2"].outcome.is_empty());
    assert_eq!(by["v2t100"].outcome, vec![(Key::pack(K_ROOM, 0, 0, 0), 7)]);
    assert_eq!(by["v2t100"].back, Some(by["v2t100"].frame - 100));
}

#[test]
fn an_input_the_bridge_refuses_is_a_refused_trial_and_the_step_goes_on() {
    // the camera scrolled right: the actor stands left of the screen, so its hit point has x < 0, which the bridge's
    // input check refuses
    let mk = |strict: bool| {
        let mut t = toy(100, false);
        t.st.xstart = 160;
        t.strict = strict;
        boxed(t)
    };
    let mut e = mk(true);
    let o = e.obs(&ObsReq::default()).unwrap();
    let of = afford::offer(&mut e, &o).unwrap();
    let actor = *of.targets.iter().find(|p| p.hit == Hit::Actor(1)).expect("the actor's point");
    assert!(actor.x < 0, "{actor:?}");
    let inp = afford::inputs_of(&of);
    let tested = [Key::bit(5), Key::bit(6), Key::bit(9)];
    let nov = novelty();
    let cfg = AffordCfg { horizon: 200, refuse: true, dedupe: false, check: false, ..Default::default() };
    let out = afford::try_all(&mut e, &of, &inp, &tested, &nov, &cfg, None).expect("the step goes on");
    let off_screen = |t: &afford::Trial| t.inputs.clicks.iter().any(|p| p.x < 0 || p.y < 0);
    let refused: Vec<String> =
        out.trials.iter().filter(|t| t.verdict.as_deref() == Some(afford::BRIDGE_REFUSED)).map(|t| t.inputs.name()).collect();
    let expect: Vec<String> = out.trials.iter().filter(|t| off_screen(t)).map(|t| t.inputs.name()).collect();
    assert_eq!(refused, expect);
    // the trial's own clicks and a third click
    for n in ["v2t-1", "v3t-1", "t-1", "v3t102t-1"] {
        assert!(refused.contains(&n.to_string()), "{n}: {refused:?}");
    }
    for t in out.trials.iter().filter(|t| off_screen(t)) {
        assert_eq!((t.reason.as_str(), t.ended, t.kept), ("refused", false, 0), "{}", t.inputs.name());
    }
    // every other trial ran as it does with no input check
    let mut e2 = mk(false);
    let out2 = afford::try_all(&mut e2, &of, &inp, &tested, &nov, &cfg, None).unwrap();
    let ran = |o: &afford::AffordOut| -> Vec<(String, u64, String, Option<String>)> {
        o.trials.iter().filter(|t| !off_screen(t)).map(|t| (t.inputs.name(), t.frame, t.reason.clone(), t.verdict.clone())).collect()
    };
    assert_eq!(ran(&out), ran(&out2));
    assert!(out2.trials.iter().any(off_screen), "with no input check the actor's clicks run");
    let kept = |o: &afford::AffordOut| {
        o.kept.iter().filter(|k| !k.name.contains("t-1")).map(|k| (k.name.clone(), k.novel.clone())).collect::<Vec<_>>()
    };
    assert_eq!(kept(&out), kept(&out2));
    // the record: the refused trials by verdict, and each with its events and the bridge's text
    let rec = out.json();
    assert_eq!(rec["verdicts"][afford::BRIDGE_REFUSED], serde_json::json!(refused.len()));
    let br = rec["bridge_refused"].as_array().unwrap();
    assert_eq!(br.iter().map(|x| x["inputs"].as_str().unwrap().to_string()).collect::<Vec<_>>(), refused);
    assert!(br.iter().all(|x| x["why"].as_str().unwrap().starts_with("events: event ")));
    assert_eq!(br[0]["events"].as_array().unwrap().last().unwrap()["click"][0], serde_json::json!(actor.x));
    assert!(out2.json().get("bridge_refused").is_none());
}

#[test]
fn m4_form_runs_to_the_horizon() {
    let mut e = boxed(toy(100, false));
    let o = e.obs(&ObsReq::default()).unwrap();
    let of = afford::offer(&mut e, &o).unwrap();
    let inp = afford::inputs_of(&of);
    struct No;
    impl Novelty for No {
        fn novel(&self, _f: &Facts) -> Vec<String> {
            vec![]
        }
    }
    let out = afford::try_all(&mut e, &of, &inp, &[], &No, &AffordCfg { horizon: 40, fixed: true, third: true, ..Default::default() }, None).unwrap();
    assert!(out.kept.is_empty());
    assert!(out.trials.iter().all(|t| t.inputs.clicks.len() < 3), "no third click in M4's form");
    for t in &out.trials {
        if !t.ended {
            assert_eq!(t.frame, 140, "{}", t.inputs.name());
        }
    }
    let oc = out.outcomes();
    // nothing, the room, bit 9 is not reachable in two clicks, the actor's var 40 is not a world key
    assert_eq!(oc.len(), 2, "{oc:?}");
}

#[test]
fn then_stops_are_kept_with_the_trials_clicks() {
    let mut e = boxed(toy(100, true));
    let o = e.obs(&ObsReq::default()).unwrap();
    let of = afford::offer(&mut e, &o).unwrap();
    let inp = afford::inputs_of(&of);
    let tested = [Key::bit(5), Key::bit(6)];
    let nov = novelty();
    // the "wait": 30 frames later on a fork, a checkpoint holding bit 6 set by hand (stands in for a later change)
    let then = |b: &mut Box<dyn Engine>| -> Result<Vec<Checkpoint>, String> {
        let mut c = b.fork().map_err(|e| e.to_string())?;
        let to = c.status().frame + 30;
        let st = c.run(&RunSpec { frame: Some(to), ..Default::default() }).map_err(|e| e.to_string())?;
        let o = c.obs(&ObsReq::default()).map_err(|e| e.to_string())?;
        let mut cp = afford::checkpoint_of(&st, &o);
        cp.facts.set(Key::bit(6), 1);
        Ok(vec![cp])
    };
    let out = afford::try_all(&mut e, &of, &inp, &tested, &nov, &AffordCfg::default(), Some(&then)).unwrap();
    let kept: Vec<(String, u64, Vec<String>)> = out.kept.iter().map(|k| (k.name.clone(), k.checkpoint.frame, k.novel.clone())).collect();
    let s120 = out.trials[0].frame;
    // line 120: bit 5 at its idle stop, then bit 6 at the "wait"'s stop; line 121's bit 6 is no longer novel
    assert_eq!(kept, vec![("v120".into(), s120, vec!["bit:5=1".into()]), ("v120".into(), s120 + 30, vec!["bit:6=1".into()])]);
    assert_eq!(out.kept[1].events, out.kept[0].events);
}

#[test]
fn pair_novelty() {
    let mut n = PairNovelty::new([Key::var(7)]);
    let f = |v: i64, room: i64| Facts::from_pairs(vec![(Key::var(7), v), (Key::pack(K_ROOM, 0, 0, 0), room)]);
    n.add(&f(1, 3));
    assert!(n.novel(&f(1, 3)).is_empty());
    assert_eq!(n.novel(&f(2, 3)), vec!["var:7=2"]);
    assert_eq!(n.novel(&f(2, 4)), vec!["room=4", "var:7=2"]);
    // an absent key reads 0
    assert_eq!(n.novel(&Facts::from_pairs(vec![(Key::pack(K_ROOM, 0, 0, 0), 3)])), vec!["var:7=0"]);
}

#[test]
fn walk_boxes() {
    // the engine's walkable rule and the candidate points
    assert!(afford::walkable(0x00) && afford::walkable(0x08) && afford::walkable(0x20));
    assert!(!afford::walkable(0x40) && !afford::walkable(0x80) && !afford::walkable(0xC0));
    let w = WalkBox { ul: (40, 30), ur: (80, 30), lr: (80, 70), ll: (40, 70), flags: 0 };
    assert_eq!(afford::box_points(&w), vec![(60, 50), (50, 40), (70, 40), (70, 60), (50, 60)]);
    // switch off: no Boxes command, M4's inputs
    let t = toy(100, false);
    let asked = t.boxes_asked.clone();
    let mut e = boxed(t);
    let o = e.obs(&ObsReq::default()).unwrap();
    let off = afford::offer_with(&mut e, &o, false).unwrap();
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(off.walks.is_empty() && off.boxes.is_none());
    // on: boxes 1 and 4 (2 locked, 3 invisible, 5 off screen, 0 never); box 4's centre and first halfway point lie on
    // object 100, its second is the floor
    let on = afford::offer_with(&mut e, &o, true).unwrap();
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(on.boxes, Some((6, 3)));
    assert_eq!(on.walks.iter().map(|p| (p.x, p.y, p.hit)).collect::<Vec<_>>(), vec![(50, 120, Hit::Walk(1)), (70, 40, Hit::Walk(4))]);
    let inp_off = afford::inputs_of(&off);
    let inp_on = afford::inputs_of(&on);
    assert_eq!(names(&inp_on[..inp_off.len()]), names(&inp_off));
    assert_eq!(names(&inp_on[inp_off.len()..]), vec!["w1", "w4"]);
    // not in a dialog
    let mut d = boxed(toy(100, true));
    let od = d.obs(&ObsReq::default()).unwrap();
    assert!(afford::offer_with(&mut d, &od, true).unwrap().walks.is_empty());
    // the walk to box 1 leaves the room: kept
    let nov = novelty();
    let out = afford::try_all(&mut e, &on, &inp_on, &[], &nov, &AffordCfg::default(), None).unwrap();
    let w1 = out.kept.iter().find(|k| k.name == "w1").expect("w1 kept");
    assert_eq!(w1.novel, vec!["room=9".to_string()]);
    assert_eq!(w1.events, vec![Event { frame: 101, what: Input::Click(50, 120) }]);
}

/// E013's refused inputs (FMI_REFUSE's settings, given directly): a trial whose input changed nothing ends at the frame
/// after its click with no wait; a trial whose stop is in the state of an earlier trial's stop, or of the no-input
/// twin at the same frame, shares that wait; with the check (gate B4) every shared wait is run again and compared.
#[test]
fn refused_inputs_end_at_once_and_equal_states_share_a_wait() {
    use afford::Point;
    let mut e = boxed(toy(100, false));
    let o = e.obs(&ObsReq::default()).unwrap();
    let of = afford::offer(&mut e, &o).unwrap();
    let one = |x, y, hit| Inputs { clicks: vec![Point { x, y, hit }] };
    let inputs = vec![
        one(300, 190, Hit::Verb { slot: 9, id: 99 }), // the verb screen with no slot there: ignored
        one(130, 20, Hit::Walk(5)),                   // a walk that ends where it started: the twin's state
        one(12, 152, Hit::Verb { slot: 1, id: 2 }),   // "open" selected
        one(12, 152, Hit::Verb { slot: 1, id: 2 }),   // the same again: the state of the trial before
    ];
    let calls = std::cell::Cell::new(0);
    let then_f = |_b: &mut Box<dyn Engine>| -> Result<Vec<Checkpoint>, String> {
        calls.set(calls.get() + 1);
        Ok(vec![])
    };
    let cfg = AffordCfg { horizon: 200, fixed: false, third: false, refuse: true, dedupe: true, check: false };
    let out = afford::try_all(&mut e, &of, &inputs, &[Key::bit(9)], &novelty(), &cfg, Some(&then_f)).unwrap();
    let t = &out.trials;
    assert_eq!((t[0].reason.as_str(), t[0].verdict.as_deref(), t[0].frame), ("refused", Some("ignored"), 102));
    assert!(t[0].digest.is_none() && t[0].shared.is_none());
    assert_eq!(t[1].verdict.as_deref(), Some("taken"));
    assert_eq!(t[1].shared.as_deref(), Some("twin"));
    assert_eq!(t[2].shared, None);
    assert_eq!(t[3].shared.as_deref(), Some("trial 2"));
    assert_eq!(t[2].digest, t[3].digest);
    assert_eq!(calls.get(), 1, "only the trial with a new state runs the wait");
    let j = out.json();
    assert_eq!(j["refuse"]["ended_at_once"], 1);
    assert_eq!((j["refuse"]["shared_trial"].as_u64(), j["refuse"]["shared_twin"].as_u64()), (Some(1), Some(1)));
    assert_eq!(j["verdicts"]["ignored"], 1);
    assert_eq!(j["verdicts"]["taken"], 3);
    // gate B4's check: every shared wait run again (the twin's from a fork of it) and compared
    calls.set(0);
    let mut e = boxed(toy(100, false));
    let cfg = AffordCfg { check: true, ..cfg };
    let out = afford::try_all(&mut e, &of, &inputs, &[Key::bit(9)], &novelty(), &cfg, Some(&then_f)).unwrap();
    let r = out.refuse.as_ref().unwrap();
    assert_eq!(r.pairs.len(), 2);
    assert!(r.pairs.iter().all(|p| p.2));
    assert_eq!(calls.get(), 1 + 2 + 1, "the new state's wait, the two shared ones again, the twin's own");
    // off: as E012's step
    let mut e = boxed(toy(100, false));
    let cfg = AffordCfg { horizon: 200, fixed: false, third: false, refuse: false, dedupe: false, check: false };
    let out = afford::try_all(&mut e, &of, &inputs, &[Key::bit(9)], &novelty(), &cfg, None).unwrap();
    assert!(out.trials.iter().all(|t| t.verdict.is_none() && t.digest.is_none()));
    assert!(out.json().get("verdicts").is_none());
}
