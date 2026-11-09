//! fmi-shorten on a toy engine: the progress (held atoms, I and the interface's keys left out), the steps (a verb and
//! its object in one step; a step that changes nothing merged), keep(k) and its completion, the twin's draw keys, the
//! anchors and their re-placement (G2's check), and the pure parts (anchor kinds, timing, walk boxes, completion).

use fmi_core::*;
use fmi_explore::archive::Atom;
use fmi_shorten::chain::{self, Held, Opts, Progress};
use fmi_shorten::label::{self, Anchor, Seen, Timing};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const VERB: i32 = 7;
const THING: i32 = 50;
const DOOR: i32 = 51;
/// toy variables: the composing verb and the click's copy (the handler writes them), the mouse (the engine), the draw
const V_VERB: usize = 30;
const V_CLICK: usize = 31;
const V_MOUSE: usize = 20;
const V_DRAW: usize = 40;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Effect {
    PickUp,
    Door,
}

/// A toy game: a verb slot at the bottom left; an object to pick up with the verb (3 frames after the click), a door
/// that changes the room 5 frames after a click on it, where the entry draws a value from the RNG.
#[derive(Clone)]
struct Toy {
    st: Status,
    vars: Vec<i32>,
    owner: i32,
    mouse: (i32, i32),
    pending: Vec<Event>,
    busy: Option<(u64, Effect)>,
    rng: u32,
    log: Vec<InputRecord>,
}

fn toy() -> Toy {
    Toy {
        st: Status { frame: 1, tick: 6, room: 1, rng: 4321, idle: Some("sentence".into()), ..Default::default() },
        vars: vec![0; 60],
        owner: 0,
        mouse: (0, 0),
        pending: Vec::new(),
        busy: None,
        rng: 4321,
        log: Vec::new(),
    }
}

impl Toy {
    fn hit(&self, x: i32, y: i32) -> At {
        if y >= 144 {
            return At { x, y, zone: 2, main: None, verb_slot: if x < 40 { 1 } else { 0 } };
        }
        let obj = if (100..120).contains(&x) && (50..70).contains(&y) {
            THING
        } else if (200..220).contains(&x) && (50..90).contains(&y) {
            DOOR
        } else {
            0
        };
        At { x, y, zone: 0, main: Some((x, y, obj, 0)), verb_slot: 0 }
    }

    fn click(&mut self, x: i32, y: i32) -> InputRecord {
        let mut writes = Vec::new();
        let at = self.hit(x, y);
        let mut w = |t: &mut Toy, k: usize, v: i32| {
            writes.push(Touch { kind: TouchKind::Var, a: k as i32, b: 0, old: t.vars[k], now: v });
            t.vars[k] = v;
        };
        if at.verb_slot == 1 {
            w(self, V_VERB, VERB);
        } else if let Some((vx, _, o, _)) = at.main {
            w(self, V_CLICK, vx);
            if o == THING && self.vars[V_VERB] == VERB && self.owner == 0 {
                w(self, V_VERB, 0);
                writes.push(Touch { kind: TouchKind::Sentence, a: VERB, b: THING, old: 0, now: 0 });
                self.busy = Some((self.st.frame + 3, Effect::PickUp));
            } else if o == DOOR {
                // the walk to the door's own spot, wherever on it the click was
                writes.push(Touch { kind: TouchKind::Walk, a: 1, b: 0, old: 210, now: 90 });
                self.busy = Some((self.st.frame + 5, Effect::Door));
            }
        }
        let verdict = if writes.is_empty() { Verdict::Ignored } else { Verdict::Taken };
        InputRecord {
            frame: self.st.frame, input: vec![Input::Click(x, y)], stat: 0, verdict, area: if at.verb_slot > 0 { 1 } else { 2 },
            value: if at.verb_slot > 0 { VERB } else { 0 }, mode: 1, slot: at.verb_slot, zone: at.zone, key_match: false,
            handler: 4, writes, reads: vec![], ops: vec![], ops_dropped: 0, touches_dropped: 0,
        }
    }

    fn step(&mut self) {
        self.st.frame += 1;
        self.st.tick += 6;
        let f = self.st.frame;
        let now: Vec<Event> = self.pending.iter().filter(|e| e.frame == f).cloned().collect();
        self.pending.retain(|e| e.frame != f);
        for e in now {
            match e.what {
                Input::Move(x, y) => self.mouse = (x, y),
                Input::Click(x, y) => {
                    self.mouse = (x, y);
                    let r = self.click(x, y);
                    self.log.push(r);
                }
                _ => {}
            }
        }
        self.vars[V_MOUSE] = self.mouse.0;
        if let Some((at, eff)) = self.busy {
            if at == f {
                self.busy = None;
                match eff {
                    Effect::PickUp => self.owner = 1,
                    Effect::Door => {
                        self.st.room = 2;
                        self.vars[V_DRAW] = (self.rng % 1000) as i32;
                        self.rng = self.rng.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    }
                }
            }
        }
        self.st.rng = self.rng as u64;
        self.st.idle = if self.busy.is_some() { None } else { Some("sentence".into()) };
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
        let to = spec.frame.ok_or_else(|| EngineError::Bridge("the toy runs to a frame".into()))?;
        if to <= self.st.frame {
            return Err(EngineError::Bridge(format!("frame {to} is not after {}", self.st.frame)));
        }
        while self.st.frame < to {
            self.step();
        }
        self.st.reason = "frame".into();
        Ok(self.st.clone())
    }
    fn inputs(&mut self, events: &[Event]) -> EResult<()> {
        self.pending.extend_from_slice(events);
        Ok(())
    }
    fn obs(&mut self, _req: &ObsReq) -> EResult<Obs> {
        Ok(Obs {
            frame: self.st.frame,
            room: self.st.room,
            ego: Some(1),
            vars: self.vars.clone(),
            owners: vec![(THING as u32, self.owner)],
            room_objects: Some(vec![
                RoomObject { id: THING, x: 100, y: 50, w: 20, h: 20, ..Default::default() },
                RoomObject { id: DOOR, x: 200, y: 50, w: 20, h: 40, ..Default::default() },
            ]),
            verbs: vec![Verb { slot: 1, id: VERB, mode: 1, x: 0, y: 146, ..Default::default() }],
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
        Ok(self.hit(x, y))
    }
    fn set_rng(&mut self, s: u32) -> EResult<()> {
        self.rng = s;
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
    fn input_log(&mut self) -> EResult<InputLog> {
        Ok(InputLog { records: std::mem::take(&mut self.log), dropped: 0, frame: self.st.frame })
    }
}

struct ToyHost;

impl EngineHost for ToyHost {
    fn boot(&self, cfg: &BootCfg) -> EResult<Box<dyn Engine>> {
        let mut t = toy();
        t.pending = cfg.events.iter().filter(|e| cfg.frame.is_none_or(|f| e.frame <= f)).cloned().collect();
        if let Some(f) = cfg.frame {
            t.run(&RunSpec { frame: Some(f), ..Default::default() })?;
        }
        Ok(Box::new(t))
    }
}

/// The guide's atoms: the interface's variables, the mouse, the object's owner, the draw, the room.
fn held() -> Held {
    let any = || Atom { values: Default::default(), any: true };
    let mut atoms = BTreeMap::new();
    for k in [V_VERB, V_CLICK, V_MOUSE, V_DRAW] {
        atoms.insert(format!("var:{k}"), any());
    }
    atoms.insert(format!("owner:{THING}"), Atom { values: [1].into_iter().collect(), any: false });
    atoms.insert("room".into(), Atom { values: [2].into_iter().collect(), any: false });
    Held::new(atoms, BTreeSet::new(), HashMap::new())
}

fn click(frame: u64, x: i32, y: i32) -> Event {
    Event { frame, what: Input::Click(x, y) }
}

/// The verb, then the object two frames later; a pause; the door.
fn movie() -> Vec<Event> {
    vec![click(10, 2, 150), click(12, 105, 55), click(30, 205, 60)]
}

fn opts() -> Opts {
    Opts { seed: 1, out: std::env::temp_dir().join("fmi-shorten-test"), twins: true, tail: 20 }
}

#[test]
fn replay_steps_keep_and_twin() {
    let mut h = held();
    let ch = chain::replay(&ToyHost, &movie(), &mut h, &opts()).unwrap();
    // the interface: the handler's variables by the records, the mouse by the two moves
    assert_eq!(ch.iface.0, [Key::var(V_VERB as u64), Key::var(V_CLICK as u64)].into_iter().collect());
    assert_eq!(ch.iface.1, [Key::var(V_MOUSE as u64)].into_iter().collect());
    assert_eq!(ch.drift, 0);
    // the verb and its object are one step (the object's timing is "next"); the door is the next
    assert_eq!(ch.steps.len(), 2);
    let (a, b) = (&ch.steps[0], &ch.steps[1]);
    assert_eq!(a.inputs, 0..2);
    assert_eq!(a.changed, vec![Key::owner(THING as u64)]);
    assert_eq!(a.keep, vec![(Key::owner(THING as u64), 1)]);
    assert_eq!(a.completion.map(|c| c.0), Some(15));
    assert_eq!(b.inputs, 2..3);
    assert!(b.changed.contains(&chain::ROOM) && b.changed.contains(&Key::var(V_DRAW as u64)));
    assert_eq!(b.completion.map(|c| c.0), Some(35));
    assert_eq!(b.rooms, vec![1, 2]);
    // the twin: the draw differs on the door's step only
    assert!(a.twin.as_ref().unwrap().draw.is_empty());
    assert_eq!(b.twin.as_ref().unwrap().draw, vec![Key::var(V_DRAW as u64)]);
    let l = label::labels(&ch);
    assert_eq!(l.anchors, vec![Anchor::Verb { id: VERB }, Anchor::Object { id: THING, actor: 0 }, Anchor::Object { id: DOOR, actor: 0 }]);
    assert_eq!(l.timing, vec![Timing::Idle, Timing::Next, Timing::Idle]);
    assert_eq!(l.ids[1].as_ref().unwrap().sentences, vec![(VERB, THING, 0)]);
}

#[test]
fn check_replaces_each_anchor() {
    let mut h = held();
    let ch = chain::replay(&ToyHost, &movie(), &mut h, &Opts { twins: false, ..opts() }).unwrap();
    let l = label::labels(&ch);
    let g = label::check(&ToyHost, &movie(), &ch, &l, &opts()).unwrap();
    for k in ["placed", "same_hit", "same_identity"] {
        assert_eq!(g[k], 3, "{k}: {g}");
    }
    // each is re-placed at the offer's point, not the movie's
    assert_eq!(g["moved"], 3, "{g}");
    assert_eq!(g["keep_at_completion"], 2);
    assert_eq!(g["keep_before_completion"], 0);
}

#[test]
fn a_step_that_changes_nothing_merges() {
    // a floor click first: no progress at its next stop, so it joins the verb's step
    let mut m = vec![click(5, 150, 100)];
    m.extend(movie());
    let mut h = held();
    let ch = chain::replay(&ToyHost, &m, &mut h, &Opts { twins: false, ..opts() }).unwrap();
    assert_eq!(ch.steps.len(), 2);
    assert_eq!(ch.steps[0].inputs, 0..3);
    assert_eq!(label::labels(&ch).anchors[0], Anchor::Walk { x: 150, y: 100, walkbox: None });
}

#[test]
fn anchors_by_the_hit() {
    let ev = |x, y| Event { frame: 9, what: Input::Click(x, y) };
    let seen = |at: At, verb: Option<i32>, idle: &str| Seen { at: Some(at), verb_id: verb, idle: Some(idle.into()), ..Default::default() };
    let verb = At { verb_slot: 3, ..Default::default() };
    assert_eq!(label::anchor(&seen(verb.clone(), Some(9), "sentence"), &ev(1, 1), false), Anchor::Verb { id: 9 });
    assert_eq!(label::anchor(&seen(verb.clone(), Some(9), "dialog"), &ev(1, 1), true), Anchor::Dialog { id: 9 });
    assert_eq!(label::anchor(&seen(verb, None, "sentence"), &ev(1, 1), false), Anchor::Raw { x: 1, y: 1 });
    let main = |o, a| At { main: Some((40, 50, o, a)), ..Default::default() };
    assert_eq!(label::anchor(&seen(main(5, 2), None, "sentence"), &ev(1, 1), false), Anchor::Object { id: 5, actor: 2 });
    assert_eq!(label::anchor(&seen(main(0, 2), None, "sentence"), &ev(1, 1), false), Anchor::Actor { id: 2 });
    assert_eq!(label::anchor(&seen(main(0, 0), None, "sentence"), &ev(1, 1), false), Anchor::Walk { x: 40, y: 50, walkbox: None });
    assert_eq!(label::anchor(&seen(At::default(), None, "sentence"), &ev(3, 4), false), Anchor::Raw { x: 3, y: 4 });
    let key = Event { frame: 9, what: Input::Key("t".into()) };
    assert_eq!(label::anchor(&Seen::default(), &key, false), Anchor::Key { key: "t".into() });
    // the hit an anchor wants: an object's actor too
    assert!(label::same_hit(&main(5, 2), &[], &Anchor::Object { id: 5, actor: 2 }));
    assert!(!label::same_hit(&main(5, 0), &[], &Anchor::Object { id: 5, actor: 2 }));
    assert!(!label::same_hit(&main(5, 2), &[], &Anchor::Actor { id: 2 }));
}

#[test]
fn timing_and_boxes() {
    assert_eq!(label::timing(None), Timing::Idle);
    assert_eq!(label::timing(Some(2)), Timing::Next);
    assert_eq!(label::timing(Some(label::NEXT_GAP)), Timing::Next);
    assert_eq!(label::timing(Some(label::NEXT_GAP + 1)), Timing::Idle);
    let b = |x0, y0, x1, y1| WalkBox { ul: (x0, y0), ur: (x1, y0), lr: (x1, y1), ll: (x0, y1), flags: 0 };
    let bx = Boxes { room: 1, boxes: vec![b(0, 0, 500, 500), b(0, 100, 100, 140), b(200, 100, 300, 140)] };
    // box 0 is never one (afford::FIRST_VALID_BOX)
    assert_eq!(label::box_of(&bx, (50, 120)), Some(1));
    assert_eq!(label::box_of(&bx, (300, 140)), Some(2));
    assert_eq!(label::box_of(&bx, (150, 120)), None);
    // a slanted box
    let s = WalkBox { ul: (0, 0), ur: (10, 0), lr: (20, 10), ll: (10, 10), flags: 0 };
    assert!(label::in_box(&s, (15, 8)) && !label::in_box(&s, (2, 8)));
}

#[test]
fn held_leaves_out_idle_and_interface_keys() {
    let mut h = held();
    let f = Facts::from_pairs(vec![(Key::var(V_VERB as u64), 7), (Key::var(V_DRAW as u64), 3), (chain::ROOM, 1)]);
    assert_eq!(h.held(&f), vec![(format!("var:{V_VERB}"), 7), (format!("var:{V_DRAW}"), 3)]);
    h.iface.insert(Key::var(V_VERB as u64));
    h.idle.insert(format!("var:{V_DRAW}"));
    assert!(h.held(&f).is_empty());
    let p = Progress { room: 1, owned: vec![5], held: vec![("var:9".into(), 1)] };
    let q = Progress { room: 2, owned: vec![6], held: vec![("var:9".into(), 2)] };
    assert_eq!(p.changed(&q), vec![Key::var(9), Key::owner(5), Key::owner(6), chain::ROOM]);
}

#[test]
fn a_verb_slot_taken_by_another_handler_is_a_dialog_line() {
    let rec = |handler, sentence: bool| InputRecord {
        frame: 9, input: vec![], stat: 0, verdict: Verdict::Taken, area: 1, value: 0, mode: 1, slot: 1, zone: 2,
        key_match: false, handler,
        writes: if sentence { vec![Touch { kind: TouchKind::Sentence, a: 1, b: 2, old: 0, now: 0 }] } else { vec![] },
        reads: vec![], ops: vec![], ops_dropped: 0, touches_dropped: 0,
    };
    let met = |r: InputRecord| chain::Met {
        ev: click(9, 1, 150), seen: Seen { idle: Some("sentence".into()), ..Default::default() }, record: Some(r), gap: None,
        first_idle: false, next: chain::Stop::default(), facts: Facts::new(),
    };
    let sh: BTreeSet<i32> = [4].into_iter().collect();
    assert!(!label::dialog_of(&met(rec(4, false)), &sh));
    assert!(label::dialog_of(&met(rec(17, false)), &sh));
    // no sentence in the movie: no handler is the sentence interface's, so none is told apart
    assert!(!label::dialog_of(&met(rec(17, false)), &BTreeSet::new()));
}
