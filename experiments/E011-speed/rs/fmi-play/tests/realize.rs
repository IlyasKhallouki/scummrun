//! E015 G1: realize.rs on a toy engine (plan.md, G1 "Tests"): a bare click gives the sentence; only a verb slot then
//! the object gives it; a target off the screen comes on after 2 floor clicks; a target that never comes on screen
//! misses with "approach"; no candidate gives the sentence ("no sentence", within the limit); a dialog verb; a
//! two-object sentence. The switch off is tested in tests/realize_off.rs (its own process).
//!
//! E016 A (experiments/E016-realize-run/plan.md, amendment (1)): the inventory extension on a toy with inventory slots
//! (an inventory object with a room object, with another inventory object, after an approach to b, a bounded miss),
//! and the run path's hook (`executor::play_input` with the switch on: the accepted events played on the branch; a
//! miss as a route error with realize's reason).

use fmi_core::*;
use fmi_play::realize::{self, Cfg, Step};

/// The toy's interface numbers: the default verb, and the verb slots (slot, id) of the sentence screen and of the
/// dialog.
const DEFAULT_VERB: i32 = 10;
const VERBS: [(i32, i32); 3] = [(1, 10), (2, 2), (3, 3)];
const DIALOG: [(i32, i32); 2] = [(1, 120), (2, 121)];
/// E016: the toy's inventory slots (slot, id, the object it shows), listed after the verbs when the toy has them.
const INV: [(i32, i32, i32); 3] = [(4, 100, 900), (5, 101, 901), (6, 102, 902)];
const ROOM_W: i32 = 640;
const MAIN_H: i32 = 144;

/// A toy game behind the Engine port. A 640-wide room; the camera follows a walk's end (xstart = end x - 160, kept in
/// 0..=320) when the walk is over. Walk boxes 1..=16 are 40-wide strips of floor (y 60..140). Objects: 500 (x 400),
/// 501 (x 20, off the screen from xstart 320), 502 (x 460), 503 (x 600, untouchable: never hit), 504 (x 520). A click
/// on an object queues sentence(selected verb, object) (a verb that composes, 3, takes a second object first), and
/// the selected verb returns to the default. A floor click in a box walks there. In a dialog, 120 ends it, 121 does
/// nothing the log counts.
#[derive(Clone)]
struct Toy {
    st: Status,
    pending: Vec<Event>,
    sel: i32,
    obj1: i32,
    dialog: bool,
    busy: u64,
    walk_to: Option<i32>,
    log: Vec<InputRecord>,
    done: Vec<(i32, i32, i32)>,
    /// E016: inventory slots shown (INV)
    inv: bool,
    /// E016 amendment (2): the inventory slots are image slots (the dump's type 1)
    inv_images: bool,
}

fn toy(xstart: i32, dialog: bool) -> Box<dyn Engine> {
    Box::new(toy_raw(xstart, dialog))
}

fn toy_raw(xstart: i32, dialog: bool) -> Toy {
    let mut t = Toy {
        st: Status { frame: 100, room: 5, reason: "idle".into(), xstart, screen: Some((0, MAIN_H, false)), room_w: ROOM_W, ..Default::default() },
        pending: vec![],
        sel: DEFAULT_VERB,
        obj1: 0,
        dialog,
        busy: 0,
        walk_to: None,
        log: vec![],
        done: vec![],
        inv: false,
        inv_images: false,
    };
    t.st.idle = t.idle();
    t
}

/// E016: the toy with its inventory slots.
fn toy_inv(xstart: i32) -> Box<dyn Engine> {
    let mut t = toy_raw(xstart, false);
    t.inv = true;
    Box::new(t)
}

fn inside(x: i32, y: i32, r: (i32, i32, i32, i32)) -> bool {
    r.0 <= x && x < r.0 + r.2 && r.1 <= y && y < r.1 + r.3
}

impl Toy {
    fn idle(&self) -> Option<String> {
        match (self.busy, self.dialog) {
            (0, true) => Some("dialog".into()),
            (0, false) => Some("sentence".into()),
            _ => None,
        }
    }
    fn verbs(&self) -> Vec<Verb> {
        let list: &[(i32, i32)] = if self.dialog { &DIALOG } else { &VERBS };
        let mut v: Vec<Verb> = list.iter().map(|&(slot, id)| Verb { slot, id, mode: 1, x: 10 + 50 * (slot - 1), y: 150, ..Default::default() }).collect();
        if self.inv && !self.dialog {
            let kind = if self.inv_images { realize::IMAGE_VERB } else { 0 };
            v.extend(INV.iter().map(|&(slot, id, _)| Verb { slot, id, mode: 1, kind, x: 10 + 50 * (slot - 1), y: 150, ..Default::default() }));
        }
        v
    }
    fn objects(&self) -> Vec<RoomObject> {
        let o = |id, x, y, w, h| RoomObject { id, x, y, w, h, walk: (x + w / 2, 70), ..Default::default() };
        vec![o(500, 400, 20, 20, 20), o(501, 20, 20, 30, 30), o(502, 460, 20, 20, 20), o(503, 600, 20, 20, 20), o(504, 520, 20, 20, 20)]
    }
    fn boxes_(&self) -> Vec<WalkBox> {
        let mut v = vec![WalkBox::default()];
        for i in 0..16 {
            let (x0, x1) = (40 * i, 40 * i + 39);
            v.push(WalkBox { ul: (x0, 60), ur: (x1, 60), lr: (x1, 139), ll: (x0, 139), flags: 0 });
        }
        v
    }
    fn at_(&self, x: i32, y: i32) -> At {
        let mut a = At { x, y, ..Default::default() };
        if y >= MAIN_H {
            a.zone = 2;
            a.verb_slot = self.verbs().iter().find(|v| inside(x, y, (v.x, v.y, 40, 8))).map(|v| v.slot).unwrap_or(0);
        } else {
            a.zone = 1;
            let (vx, vy) = (x + self.st.xstart, y);
            let obj = self.objects().iter().filter(|o| o.id != 503).find(|o| inside(vx, vy, (o.x, o.y, o.w, o.h))).map(|o| o.id).unwrap_or(0);
            a.main = Some((vx, vy, obj, 0));
        }
        a
    }
    fn click(&mut self, x: i32, y: i32) {
        let a = self.at_(x, y);
        let mut writes = Vec::new();
        // an inventory slot is clicked as the object it shows
        let inv_obj = INV.iter().find(|i| self.inv && !self.dialog && i.0 == a.verb_slot).map(|i| i.2);
        let main = match inv_obj {
            Some(o) => Some((0, 0, o, 0)),
            None => a.main,
        };
        if a.verb_slot > 0 && inv_obj.is_none() {
            let id = self.verbs().iter().find(|v| v.slot == a.verb_slot).unwrap().id;
            if self.dialog {
                if id == 120 {
                    self.dialog = false;
                    self.busy = 3;
                    writes.push(Touch { kind: TouchKind::Var, a: 165, b: 0, old: 0, now: id });
                }
            } else {
                writes.push(Touch { kind: TouchKind::Var, a: 107, b: 0, old: self.sel, now: id });
                self.sel = id;
                self.busy = 1;
            }
        } else if let (Some((vx, vy, obj, _)), false) = (main, self.dialog) {
            if obj != 0 {
                if self.sel == 3 && self.obj1 == 0 {
                    self.obj1 = obj;
                    self.busy = 1;
                } else {
                    let (o1, o2) = if self.obj1 != 0 { (self.obj1, obj) } else { (obj, 0) };
                    writes.push(Touch { kind: TouchKind::Sentence, a: self.sel, b: o1, old: o2, now: 0 });
                    self.done.push((self.sel, o1, o2));
                    self.obj1 = 0;
                    self.sel = DEFAULT_VERB;
                    self.busy = 4;
                }
            } else if self.boxes_().iter().skip(1).any(|b| inside(vx, vy, (b.ul.0, b.ul.1, b.ur.0 - b.ul.0 + 1, b.ll.1 - b.ul.1 + 1))) {
                writes.push(Touch { kind: TouchKind::Walk, a: 1, b: 0, old: vx, now: vy });
                self.walk_to = Some(vx);
                self.busy = 10;
            }
        }
        let verdict = if writes.is_empty() { Verdict::Ignored } else { Verdict::Taken };
        self.log.push(InputRecord {
            frame: self.st.frame,
            input: vec![Input::Click(x, y)],
            stat: 0,
            verdict,
            area: 0,
            value: 0,
            mode: 0,
            slot: a.verb_slot,
            zone: a.zone,
            key_match: false,
            handler: 4,
            writes,
            reads: vec![],
            ops: vec![],
            ops_dropped: 0,
            touches_dropped: 0,
        });
    }
    fn step(&mut self) {
        self.st.frame += 1;
        self.st.tick += 6;
        if self.busy > 0 {
            self.busy -= 1;
            if self.busy == 0 {
                if let Some(x) = self.walk_to.take() {
                    self.st.xstart = (x - 160).clamp(0, ROOM_W - 320);
                }
            }
        }
        let f = self.st.frame;
        let now: Vec<Event> = self.pending.iter().filter(|e| e.frame == f).cloned().collect();
        self.pending.retain(|e| e.frame != f);
        for e in now {
            if let Input::Click(x, y) = e.what {
                if self.busy == 0 || self.walk_to.is_some() {
                    self.click(x, y);
                }
            }
        }
        self.st.idle = self.idle();
    }
}

impl Engine for Toy {
    fn pid(&self) -> u32 {
        0
    }
    fn status(&self) -> &Status {
        &self.st
    }
    fn run(&mut self, spec: &RunSpec) -> EResult<Status> {
        let start = self.st.frame;
        loop {
            if let Some(f) = spec.frame {
                if self.st.frame >= f {
                    self.st.reason = "frame".into();
                    break;
                }
            }
            if let Some(m) = spec.max_frame {
                if self.st.frame >= m {
                    self.st.reason = "max_frame".into();
                    break;
                }
            }
            if spec.idle && self.st.frame > start && self.st.idle.is_some() {
                self.st.reason = "idle".into();
                break;
            }
            self.step();
        }
        Ok(self.st.clone())
    }
    fn inputs(&mut self, events: &[Event]) -> EResult<()> {
        self.pending.extend(events.iter().cloned());
        Ok(())
    }
    fn obs(&mut self, _req: &ObsReq) -> EResult<Obs> {
        Ok(Obs {
            frame: self.st.frame,
            room: self.st.room,
            verbs: self.verbs(),
            room_objects: Some(self.objects()),
            classes: vec![(503, vec![32])],
            vars: vec![0; 200],
            ..Default::default()
        })
    }
    fn facts(&mut self) -> EResult<Facts> {
        let o = self.obs(&ObsReq::default())?;
        Ok(facts_of(&o))
    }
    fn holds(&mut self, _conds: &[Cond]) -> EResult<bool> {
        Ok(false)
    }
    fn at(&mut self, x: i32, y: i32) -> EResult<At> {
        Ok(self.at_(x, y))
    }
    fn set_rng(&mut self, _state: u32) -> EResult<()> {
        Ok(())
    }
    fn segment(&mut self, _on: bool) -> EResult<()> {
        Ok(())
    }
    fn runs(&mut self) -> EResult<RunsReply> {
        Ok(RunsReply::default())
    }
    fn explain(&mut self, _arm: Option<&ExplainArm>) -> EResult<()> {
        Ok(())
    }
    fn explain_log(&mut self) -> EResult<ExplainReply> {
        Ok(ExplainReply::default())
    }
    fn fork(&mut self) -> EResult<Box<dyn Engine>> {
        Ok(Box::new(self.clone()))
    }
    fn fork_each(&mut self, _n: u32) -> EResult<(Status, Vec<Box<dyn Engine>>)> {
        Err(EngineError::Bridge("toy: no fork_each".into()))
    }
    fn state_json(&mut self, _req: &ObsReq) -> EResult<String> {
        Err(EngineError::Bridge("toy: no state_json".into()))
    }
    fn cpu_s(&self) -> f64 {
        0.0
    }
    fn quit(self: Box<Self>) {}
    fn boxes(&mut self) -> EResult<Boxes> {
        Ok(Boxes { room: self.st.room, boxes: self.boxes_() })
    }
    fn input_log(&mut self) -> EResult<InputLog> {
        Ok(InputLog { records: std::mem::take(&mut self.log), dropped: 0, frame: self.st.frame })
    }
}

fn setup() {
    std::env::set_var(realize::SWITCH, "1");
    std::env::set_var("FMI_BOXES", "1");
    std::env::set_var("FMI_OFFER_SCREEN", "1");
}

fn sentence(v: i32, a: i32, b: Option<i32>) -> Step {
    Step::Sentence { v, a, b }
}

/// The sentences the toy queued on the realized engine's way, by replaying its events on a fresh toy.
fn replayed(xstart: i32, dialog: bool, evs: &[Event]) -> Vec<(i32, i32, i32)> {
    replayed_on(toy_raw(xstart, dialog), evs)
}

fn replayed_on(mut t: Toy, evs: &[Event]) -> Vec<(i32, i32, i32)> {
    t.inputs(evs).unwrap();
    let last = evs.iter().map(|e| e.frame).max().unwrap_or(0);
    t.run(&RunSpec { frame: Some(last + 1), ..Default::default() }).unwrap();
    t.done.clone()
}

#[test]
fn bare_click_gives_the_sentence() {
    setup();
    let mut e = toy(320, false);
    let r = realize::realize(&mut e, &sentence(DEFAULT_VERB, 500, None), &Cfg::default()).unwrap().expect("realized");
    assert_eq!(r.approach, 0);
    assert_eq!(r.candidates, 1);
    assert_eq!(r.tries, 1);
    assert_eq!(r.events.len(), 1);
    assert_eq!(replayed(320, false, &r.events), vec![(DEFAULT_VERB, 500, 0)]);
    // `e` was not moved
    assert_eq!(e.status().frame, 100);
}

#[test]
fn verb_slot_then_object() {
    setup();
    let mut e = toy(320, false);
    let r = realize::realize(&mut e, &sentence(2, 502, None), &Cfg::default()).unwrap().expect("realized");
    // (i) bare click (3 points: each gives sentence(10, 502)); (ii) slot of verb 10 (3 points), then slot of verb 2
    assert_eq!(r.candidates, 3);
    assert_eq!(r.tries, 3 + 3 + 1);
    assert_eq!(r.events.len(), 2);
    assert_eq!(replayed(320, false, &r.events), vec![(2, 502, 0)]);
}

#[test]
fn off_screen_target_after_two_floor_clicks() {
    setup();
    let mut e = toy(320, false);
    let r = realize::realize(&mut e, &sentence(DEFAULT_VERB, 501, None), &Cfg::default()).unwrap().expect("realized");
    assert_eq!(r.approach, 2, "{:?}", r.attempts.iter().map(|a| (&a.name, &a.seen)).collect::<Vec<_>>());
    assert_eq!(r.candidates, 1);
    assert_eq!(r.events.len(), 3);
    assert!(r.attempts.iter().filter(|a| a.phase == "approach" && a.accepted).count() == 2);
    assert_eq!(r.engine.status().xstart, 19);
    assert_eq!(replayed(320, false, &r.events), vec![(DEFAULT_VERB, 501, 0)]);
}

#[test]
fn never_on_screen_misses_with_approach() {
    setup();
    let mut e = toy(0, false);
    let m = realize::realize(&mut e, &sentence(DEFAULT_VERB, 503, None), &Cfg::default()).unwrap().err().expect("a miss");
    assert_eq!(m.reason, "approach");
    assert!(m.approach <= Cfg::default().max_approach);
    assert_eq!(m.candidates, 0);
}

#[test]
fn no_candidate_misses_with_no_sentence() {
    setup();
    let mut e = toy(320, false);
    let m = realize::realize(&mut e, &sentence(7, 500, None), &Cfg::default()).unwrap().err().expect("a miss");
    assert_eq!(m.reason, "no sentence");
    // the bare click and each of the 3 verb slots, each at 3 hit points
    assert_eq!(m.candidates, 4);
    assert_eq!(m.tries, 12);
    let cfg = Cfg { max_candidates: 2, ..Default::default() };
    let m = realize::realize(&mut e, &sentence(7, 500, None), &cfg).unwrap().err().expect("a miss");
    assert_eq!((m.reason.as_str(), m.candidates), ("no sentence", 2));
}

#[test]
fn dialog_verb() {
    setup();
    let mut e = toy(320, true);
    let r = realize::realize(&mut e, &Step::Dialog { v: 120 }, &Cfg::default()).unwrap().expect("realized");
    assert_eq!((r.candidates, r.tries, r.events.len()), (1, 1, 1));
    assert_eq!(r.attempts[0].seen, "taken");
    let m = realize::realize(&mut e, &Step::Dialog { v: 121 }, &Cfg::default()).unwrap().err().expect("not taken");
    assert_eq!(m.reason, "not taken");
    let m = realize::realize(&mut e, &Step::Dialog { v: 122 }, &Cfg::default()).unwrap().err().expect("no slot");
    assert_eq!(m.reason, "no slot");
}

#[test]
fn two_object_sentence() {
    setup();
    let mut e = toy(320, false);
    let r = realize::realize(&mut e, &sentence(3, 500, Some(504)), &Cfg::default()).unwrap().expect("realized");
    assert_eq!(r.events.len(), 3);
    assert_eq!(replayed(320, false, &r.events), vec![(3, 500, 504)]);
}

#[test]
fn step_text_round_trips() {
    for s in ["d120", "s10:596", "s3:500:504"] {
        assert_eq!(Step::parse(s).unwrap().text(), s);
    }
    assert!(Step::parse("x1").is_none());
}

// --- E016 A: the inventory extension and the run path's hook ---------------------------------------------------------

fn inv_toy_raw(xstart: i32) -> Toy {
    let mut t = toy_raw(xstart, false);
    t.inv = true;
    t
}

#[test]
fn inventory_object_with_a_room_object() {
    setup();
    let mut e = toy_inv(320);
    let r = realize::realize(&mut e, &sentence(3, 901, Some(500)), &Cfg::default()).unwrap().expect("realized");
    // the verb choice is verb 3's slot; a's slot runs over slots 1, 2, 4, 5: 901 is the 4th
    assert_eq!((r.approach, r.candidates, r.tries), (0, 4, 4));
    assert!(r.attempts.iter().all(|a| a.phase == "inventory"));
    assert_eq!(r.events.len(), 3);
    assert_eq!(replayed_on(inv_toy_raw(320), &r.events), vec![(3, 901, 500)]);
}

#[test]
fn inventory_object_with_an_inventory_object() {
    setup();
    let mut e = toy_inv(320);
    let r = realize::realize(&mut e, &sentence(3, 900, Some(902)), &Cfg::default()).unwrap().expect("realized");
    // b has no hit point: each of the 6 visible slots is b's click; a's slot 900 is the 3rd after verb 3's own
    assert_eq!(r.candidates, 6 + 6 + 6);
    assert_eq!(r.events.len(), 3);
    assert_eq!(replayed_on(inv_toy_raw(320), &r.events), vec![(3, 900, 902)]);
}

#[test]
fn inventory_object_approaches_b() {
    setup();
    // 501 is off the screen from xstart 320: the approach goes to b, then the candidates as on the screen
    let mut e = toy_inv(320);
    let r = realize::realize(&mut e, &sentence(3, 901, Some(501)), &Cfg::default()).unwrap().expect("realized");
    assert_eq!(r.approach, 2);
    assert_eq!(r.candidates, 4);
    assert_eq!(r.events.len(), 2 + 3);
    assert_eq!(replayed_on(inv_toy_raw(320), &r.events), vec![(3, 901, 501)]);
}

#[test]
fn inventory_image_slots_first() {
    setup();
    // amendment (2): with the inventory slots of image type, a's slot runs over 900, 901, 902 before the verb slots
    let mut t = inv_toy_raw(320);
    t.inv_images = true;
    let mut e: Box<dyn Engine> = Box::new(t.clone());
    let r = realize::realize(&mut e, &sentence(3, 901, Some(500)), &Cfg::default()).unwrap().expect("realized");
    assert_eq!(r.candidates, 2);
    assert_eq!(replayed_on(t.clone(), &r.events), vec![(3, 901, 500)]);
    // b in the inventory: b's slots in the same order (902 is the third image slot)
    let r = realize::realize(&mut e, &sentence(3, 900, Some(902)), &Cfg::default()).unwrap().expect("realized");
    assert_eq!(r.candidates, 3);
    assert_eq!(replayed_on(t, &r.events), vec![(3, 900, 902)]);
}

#[test]
fn inventory_miss_is_bounded() {
    setup();
    let mut e = toy_inv(320);
    // no slot has verb 7: no verb click (6 slots), then each of the 6 slots (5 slots each)
    let m = realize::realize(&mut e, &sentence(7, 901, None), &Cfg::default()).unwrap().err().expect("a miss");
    assert_eq!((m.reason.as_str(), m.candidates, m.approach), ("no sentence", 6 + 6 * 5, 0));
    let cfg = Cfg { max_slots: 2, ..Default::default() };
    let m = realize::realize(&mut e, &sentence(7, 901, None), &cfg).unwrap().err().expect("a miss");
    assert_eq!(m.candidates, 2 + 6 * 2);
    // an object neither on the screen, in the room nor on a slot: every candidate tried (verb 3's slot then 5
    // slots, no verb click then 6, each other slot then 5), then "no sentence"; with max_candidates 10, 10
    let m = realize::realize(&mut e, &sentence(3, 999, None), &Cfg::default()).unwrap().err().expect("a miss");
    assert_eq!((m.reason.as_str(), m.candidates), ("no sentence", 5 + 6 + 5 * 5));
    let cfg = Cfg { max_candidates: 10, ..Default::default() };
    let m = realize::realize(&mut e, &sentence(3, 999, None), &cfg).unwrap().err().expect("a miss");
    assert_eq!(m.candidates, 10);
}

#[test]
fn hook_plays_the_accepted_events_on_the_branch() {
    use fmi_play::executor::{self, Br, Executor};
    use fmi_play::replay::InputStep;
    setup();
    let mut e = toy(320, false);
    let inp = InputStep { kind: "click", verb: Some(2), a: Some(502), b: None, key: None };
    let mut ex = Executor::new(false);
    {
        let mut br = Br::new(&mut e, None);
        br.watch_first = true;
        executor::play_input(&mut ex, &mut br, &inp, "sentence(2,502)#1").unwrap();
        // the first input's frame and dump are taken where the branch scheduled it
        assert_eq!(br.first_before.as_ref().map(|f| f.0), Some(100));
    }
    // the branch stands at the last event's frame with the events in the executor's movie
    assert_eq!(ex.movie.len(), 2);
    assert_eq!(e.status().frame, ex.movie.iter().map(|x| x.frame).max().unwrap());
    e.run(&RunSpec { frame: Some(e.status().frame + 1), ..Default::default() }).unwrap();
    let log = e.input_log().unwrap();
    let seen: Vec<(i32, i32, i32)> =
        log.records.iter().flat_map(|r| r.writes.iter()).filter(|t| t.kind == TouchKind::Sentence).map(|t| (t.a, t.b, t.old)).collect();
    assert_eq!(seen, vec![(2, 502, 0)]);
    assert_eq!(ex.check_var, None);
    // a dialog line
    let mut d = toy(320, true);
    let mut ex = Executor::new(false);
    let inp = InputStep { kind: "line", verb: Some(120), a: None, b: None, key: None };
    executor::play_input(&mut ex, &mut Br::new(&mut d, None), &inp, "dialog_line(120)#1").unwrap();
    assert_eq!((ex.movie.len(), d.status().frame), (1, 101));
}

#[test]
fn hook_waits_for_the_idle_stop_and_reports_a_miss() {
    use fmi_play::executor::{self, Br, Executor, PlayErr};
    use fmi_play::replay::InputStep;
    setup();
    // a branch that is not idle runs to the idle stop first
    let mut t = toy_raw(320, false);
    t.busy = 5;
    t.st.idle = None;
    let mut e: Box<dyn Engine> = Box::new(t);
    let mut ex = Executor::new(false);
    let inp = InputStep { kind: "click", verb: Some(DEFAULT_VERB as i64), a: Some(500), b: None, key: None };
    executor::play_input(&mut ex, &mut Br::new(&mut e, None), &inp, "s").unwrap();
    assert_eq!(ex.movie[0].frame, 106);
    // a miss is the child's interface outcome, with realize's reason
    let mut e = toy(320, false);
    let mut ex = Executor::new(false);
    let inp = InputStep { kind: "click", verb: Some(7), a: Some(500), b: None, key: None };
    match executor::play_input(&mut ex, &mut Br::new(&mut e, None), &inp, "sentence(7,500)#1") {
        Err(PlayErr::Route(m)) => assert!(m.starts_with("sentence(7,500)#1: realize: no sentence"), "{m}"),
        _ => panic!("not a route error"),
    }
    assert!(ex.movie.is_empty());
    assert_eq!(e.status().frame, 100);
}
