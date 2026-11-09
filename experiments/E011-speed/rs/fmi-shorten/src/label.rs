//! E024 Part 2, items 2 and 3 (plan.md, "Anchors", "Draw keys"), and G2's re-placement.
//!
//! * **Anchors** (`anchor`): each click by what the engine's hit test answers at the stop before it (`observe`: `at`,
//!   the verb slots' ids, the walk box under a floor point) and the input log's record after it (`identity`, which
//!   names that hit, amendment 1: a click on another object is another click whatever its record's area). Kinds:
//!   Verb{id}, Dialog{id} (a verb slot in the bridge's dialog idle, or taken by an input handler other than the ones
//!   under which the movie queued sentences: with SPEEDRUN_IDLE=handler the bridge reports no dialog idle, and a
//!   game's dialog runs its own handler), Object{id, actor} (the actor the hit test also answers
//!   there, 0 for none: which of the two the game acts on is the game's choice, so a re-placed point must answer
//!   both), Actor{id}, Walk{room point, box}, Key; Raw only where the hit test answers nothing. The record's Sentence
//!   touch is in the identity.
//! * **Timing class** (`timing`): "next", as soon as possible after the last taken input (within `NEXT_GAP` frames
//!   of it, the interface's two-frame click spacing), or not at a control stop (amendment 1); else "idle", at an idle
//!   stop.
//! * **Re-placement** (`place`): the anchor's point at a stop, from the anchor alone (amendment 1: never the movie's
//!   point). A verb or dialog line by the offer (afford::offer_cfg); an object or actor by the offer's point for it,
//!   then for an object its actor's and its box's grid (afford::object_grid), the first that answers both; a walk at
//!   its room point less the camera's xstart, checked with `at`.
//! * **Draw keys** (`twin`, `twin_diff`): `TWINS` forks at the step's start, each with the RNG state changed its own
//!   way (set_rng, used for this analysis only), run to the step's end; the world keys whose values differ in any.
//! * **G2** (`check`): the movie again from boot, each click re-placed at its own frame by `place`; the hit at the
//!   placed point (`hit_id`) against the replay's, its record's identity against the replay's (a key's or a raw
//!   point's hit only by its record), and each step's keep at its completion frame.

use crate::chain::{self, Chain, Held, Met, Opts};
use crate::{e2s, Res};
use fmi_core::*;
use fmi_explore::afford::{self, Hit};
use fmi_explore::events::event_json;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The bridge's idle kind of a dialog (`Status::idle`).
pub const DIALOG: &str = "dialog";
/// The most frames after the last taken input at which an input counts as placed as soon as possible: the interface's
/// clicks go two frames apart (afford's M4 timing), and one frame more where a frame was spent between.
pub const NEXT_GAP: u64 = 3;
/// The twin forks' RNGs (amendment 1, three per step): the stop's state with each one's bits flipped.
pub const TWIN_FLIPS: [u32; 3] = [0x9E37_79B9, 0x85EB_CA6B, 0xC2B2_AE35];

/// What the stop before an input showed of it.
#[derive(Clone, Debug, Default)]
pub struct Seen {
    pub frame: u64,
    pub idle: Option<String>,
    pub xstart: i32,
    /// the main screen's topline
    pub top: i32,
    /// the hit test at the click's point
    pub at: Option<At>,
    /// the id of the verb slot hit
    pub verb_id: Option<i32>,
    /// the walk box under a floor point (FMI_BOXES)
    pub walkbox: Option<i32>,
    /// what the hit test answers at the click's point
    pub hit: HitId,
}

/// What the engine's hit test answers at a point (amendment 1): a verb slot by its verb id (an inventory slot is a
/// verb slot), a slot the dump does not list by its slot, an object with the actor the test also answers there (0:
/// none), an actor, the floor of the main screen, or nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HitId {
    #[default]
    None,
    Verb(i32),
    Slot(i32),
    Object(i32, i32),
    Actor(i32),
    Floor,
}

impl HitId {
    pub fn json(&self) -> Value {
        match *self {
            HitId::None => Value::Null,
            HitId::Verb(v) => json!({"verb": v}),
            HitId::Slot(s) => json!({"slot": s}),
            HitId::Object(o, a) => json!({"object": o, "actor": a}),
            HitId::Actor(a) => json!({"actor": a}),
            HitId::Floor => json!("floor"),
        }
    }
}

/// The hit `at` answers, with the stop's verb slots.
pub fn hit_id(at: &At, verbs: &[Verb]) -> HitId {
    if at.verb_slot > 0 {
        return verbs.iter().find(|v| v.slot == at.verb_slot).map(|v| HitId::Verb(v.id)).unwrap_or(HitId::Slot(at.verb_slot));
    }
    match at.main {
        Some((_, _, o, a)) if o != 0 => HitId::Object(o, a),
        Some((_, _, _, a)) if a != 0 => HitId::Actor(a),
        Some(_) => HitId::Floor,
        None => HitId::None,
    }
}

/// The click's point, for a click or a right click.
pub fn point(i: &Input) -> Option<(i32, i32)> {
    match *i {
        Input::Click(x, y) | Input::RClick(x, y) => Some((x, y)),
        _ => None,
    }
}

/// The stop's view of the input `ev` (reads only).
pub fn observe(e: &mut Box<dyn Engine>, ev: &Event) -> EResult<Seen> {
    let st = e.status().clone();
    let mut s = Seen { frame: st.frame, idle: st.idle.clone(), xstart: st.xstart, top: st.screen.map(|x| x.0).unwrap_or(0), ..Default::default() };
    let Some((x, y)) = point(&ev.what) else { return Ok(s) };
    let at = e.at(x, y)?;
    let mut verbs = Vec::new();
    if at.verb_slot > 0 {
        verbs = e.obs(&ObsReq::default())?.verbs;
        s.verb_id = verbs.iter().find(|v| v.slot == at.verb_slot).map(|v| v.id);
    } else if let Some((vx, vy, 0, 0)) = at.main {
        if afford::boxes_on() {
            s.walkbox = box_of(&e.boxes()?, (vx, vy));
        }
    }
    s.hit = hit_id(&at, &verbs);
    s.at = Some(at);
    Ok(s)
}

/// The first walk box (from afford::FIRST_VALID_BOX on) whose quadrangle holds the room point `p`.
pub fn box_of(bx: &Boxes, p: (i32, i32)) -> Option<i32> {
    bx.boxes.iter().enumerate().skip(afford::FIRST_VALID_BOX).find(|(_, b)| in_box(b, p)).map(|(i, _)| i as i32)
}

/// Whether `p` is inside the box or on its edge: on one side of each of its four edges in turn.
pub fn in_box(b: &WalkBox, p: (i32, i32)) -> bool {
    let cs = [b.ul, b.ur, b.lr, b.ll];
    let (mut pos, mut neg) = (false, false);
    for i in 0..4 {
        let (a, c) = (cs[i], cs[(i + 1) % 4]);
        let cross = (c.0 - a.0) as i64 * (p.1 - a.1) as i64 - (c.1 - a.1) as i64 * (p.0 - a.0) as i64;
        pos |= cross > 0;
        neg |= cross < 0;
    }
    !(pos && neg)
}

/// What an input acts on (plan.md item 2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Anchor {
    Verb { id: i32 },
    Dialog { id: i32 },
    /// an object, and the actor the hit test also answers at the point (0: none)
    Object { id: i32, actor: i32 },
    Actor { id: i32 },
    /// a floor point in room coordinates and its walk box
    Walk { x: i32, y: i32, walkbox: Option<i32> },
    Key { key: String },
    /// no hit: the screen point
    Raw { x: i32, y: i32 },
}

impl Anchor {
    pub fn kind(&self) -> &'static str {
        match self {
            Anchor::Verb { .. } => "verb",
            Anchor::Dialog { .. } => "dialog",
            Anchor::Object { .. } => "object",
            Anchor::Actor { .. } => "actor",
            Anchor::Walk { .. } => "walk",
            Anchor::Key { .. } => "key",
            Anchor::Raw { .. } => "raw",
        }
    }
    pub fn json(&self) -> Value {
        match self {
            Anchor::Verb { id } | Anchor::Dialog { id } | Anchor::Actor { id } => json!({"kind": self.kind(), "id": id}),
            Anchor::Object { id, actor } => json!({"kind": "object", "id": id, "actor": actor}),
            Anchor::Walk { x, y, walkbox } => json!({"kind": "walk", "room": [x, y], "box": walkbox}),
            Anchor::Key { key } => json!({"kind": "key", "key": key}),
            Anchor::Raw { x, y } => json!({"kind": "raw", "xy": [x, y]}),
        }
    }
}

/// The input handlers of the sentence interface: those under which the movie's records queued a sentence.
pub fn sentence_handlers(ch: &Chain) -> BTreeSet<i32> {
    ch.met
        .iter()
        .filter_map(|m| m.record.as_ref())
        .filter(|r| r.writes.iter().any(|t| t.kind == TouchKind::Sentence))
        .map(|r| r.handler)
        .collect()
}

/// Whether a verb slot's input is a dialog line (module docstring).
pub fn dialog_of(m: &Met, sentence: &BTreeSet<i32>) -> bool {
    m.seen.idle.as_deref() == Some(DIALOG)
        || (!sentence.is_empty() && m.record.as_ref().is_some_and(|r| r.verdict == Verdict::Taken && !sentence.contains(&r.handler)))
}

/// The anchor of input `ev` from the stop's view of it; a verb slot is a dialog line when `dialog`.
pub fn anchor(s: &Seen, ev: &Event, dialog: bool) -> Anchor {
    if let Input::Key(k) = &ev.what {
        return Anchor::Key { key: k.clone() };
    }
    let (x, y) = match ev.what {
        Input::Click(x, y) | Input::RClick(x, y) | Input::Move(x, y) => (x, y),
        _ => (0, 0),
    };
    let raw = Anchor::Raw { x, y };
    let Some(at) = s.at.as_ref().filter(|_| point(&ev.what).is_some()) else { return raw };
    if at.verb_slot > 0 {
        return match s.verb_id {
            Some(id) if dialog => Anchor::Dialog { id },
            Some(id) => Anchor::Verb { id },
            None => raw,
        };
    }
    match at.main {
        Some((_, _, o, a)) if o != 0 => Anchor::Object { id: o, actor: a },
        Some((_, _, _, a)) if a != 0 => Anchor::Actor { id: a },
        Some((vx, vy, _, _)) => Anchor::Walk { x: vx, y: vy, walkbox: s.walkbox },
        None => raw,
    }
}

/// Why an input has no anchor but Raw.
pub fn raw_why(s: &Seen, ev: &Event) -> &'static str {
    match (&ev.what, &s.at) {
        (Input::Move(..), _) => "a move",
        (_, None) => "no hit test",
        (_, Some(a)) if a.verb_slot > 0 => "a verb slot the dump does not list",
        _ => "no virtual screen hit",
    }
}

/// The timing class (plan.md item 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Timing {
    /// at an idle stop
    Idle,
    /// as soon as possible after the last taken input
    Next,
}

impl Timing {
    pub fn name(self) -> &'static str {
        match self {
            Timing::Idle => "idle",
            Timing::Next => "next",
        }
    }
}

/// The class of an input `gap` frames after the last taken input (None: no taken input before it), at a control stop
/// or not: "idle" needs one (amendment 1), with or without a taken input before (the review of stage A).
pub fn timing(gap: Option<u64>, control: bool) -> Timing {
    match gap {
        Some(g) if g <= NEXT_GAP => Timing::Next,
        _ if !control => Timing::Next,
        _ => Timing::Idle,
    }
}

/// An input's timing class as the replay met it: its stop before is a control stop when the bridge's idle rule held
/// there.
pub fn timing_of(m: &Met) -> Timing {
    timing(m.gap, m.seen.idle.is_some())
}

/// What an input did, by its record (plan.md item 4, "Click"): the verdict, the handler's arguments, the sentences
/// and walks it started; and what the hit test answered at its point at the frame before (amendment 1). No screen
/// point is in it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Identity {
    pub hit: HitId,
    pub verdict: String,
    pub area: i32,
    pub value: i32,
    pub mode: i32,
    pub handler: i32,
    /// (verb, object A, object B)
    pub sentences: Vec<(i32, i32, i32)>,
    /// (actor, x, y)
    pub walks: Vec<(i32, i32, i32)>,
}

pub fn identity(r: &InputRecord, hit: HitId) -> Identity {
    let of = |k: TouchKind| r.writes.iter().filter(move |t| t.kind == k);
    Identity {
        hit,
        verdict: r.verdict.text(),
        area: r.area,
        value: r.value,
        mode: r.mode,
        handler: r.handler,
        sentences: of(TouchKind::Sentence).map(|t| (t.a, t.b, t.old)).collect(),
        walks: of(TouchKind::Walk).map(|t| (t.a, t.old, t.now)).collect(),
    }
}

impl Identity {
    pub fn json(&self) -> Value {
        json!({"hit": self.hit.json(), "verdict": self.verdict, "area": self.area, "value": self.value, "mode": self.mode,
               "handler": self.handler, "sentences": self.sentences, "walks": self.walks})
    }
}

/// Whether the hit test `at` (with the stop's verb slots) gives the anchor.
pub fn same_hit(at: &At, verbs: &[Verb], a: &Anchor) -> bool {
    let slot_id = || verbs.iter().find(|v| v.slot == at.verb_slot).map(|v| v.id);
    match a {
        Anchor::Verb { id } | Anchor::Dialog { id } => at.verb_slot > 0 && slot_id() == Some(*id),
        Anchor::Object { id, actor } => at.verb_slot == 0 && at.main.map(|m| (m.2, m.3)) == Some((*id, *actor)),
        Anchor::Actor { id } => at.verb_slot == 0 && at.main.is_some_and(|m| m.2 == 0 && m.3 == *id),
        Anchor::Walk { x, y, .. } => at.verb_slot == 0 && at.main == Some((*x, *y, 0, 0)),
        Anchor::Key { .. } | Anchor::Raw { .. } => true,
    }
}

/// The input that acts on the anchor at this stop, from the anchor alone (None: no point gives it). `orig` is the
/// input as the movie had it: only its kind (click or right click) is read.
pub fn place(e: &mut Box<dyn Engine>, a: &Anchor, orig: &Input) -> EResult<Option<Input>> {
    Ok(place_hit(e, a, orig)?.map(|p| p.0))
}

/// `place`, with what the hit test answers at the placed point (HitId::None for a key).
pub fn place_hit(e: &mut Box<dyn Engine>, a: &Anchor, orig: &Input) -> EResult<Option<(Input, HitId)>> {
    let mk = |x: i32, y: i32| match orig {
        Input::RClick(..) => Input::RClick(x, y),
        _ => Input::Click(x, y),
    };
    match a {
        Anchor::Key { key } => Ok(Some((Input::Key(key.clone()), HitId::None))),
        Anchor::Raw { x, y } => {
            let at = e.at(*x, *y)?;
            let verbs = if at.verb_slot > 0 { e.obs(&ObsReq::default())?.verbs } else { Vec::new() };
            Ok(Some((mk(*x, *y), hit_id(&at, &verbs))))
        }
        Anchor::Walk { x, y, .. } => {
            let st = e.status().clone();
            let (px, py) = (x - st.xstart, y + st.screen.map(|s| s.0).unwrap_or(0));
            if !(0..afford::SCREEN_W).contains(&px) || !(0..afford::SCREEN_H).contains(&py) {
                return Ok(None);
            }
            let at = e.at(px, py)?;
            Ok(same_hit(&at, &[], a).then(|| (mk(px, py), hit_id(&at, &[]))))
        }
        Anchor::Verb { .. } | Anchor::Dialog { .. } | Anchor::Object { .. } | Anchor::Actor { .. } => {
            let o = e.obs(&ObsReq::default())?;
            let of = afford::offer_cfg(e, &o, false, afford::offer_screen_on())?;
            let want = |h: &Hit| match (a, *h) {
                (Anchor::Verb { id } | Anchor::Dialog { id }, Hit::Verb { id: v, .. }) => v == *id,
                (Anchor::Object { id, .. }, Hit::Object(v)) => v == *id,
                (Anchor::Object { actor, .. }, Hit::Actor(v)) => *actor != 0 && v == *actor,
                (Anchor::Actor { id }, Hit::Actor(v)) => v == *id,
                _ => false,
            };
            // the offer's points (an object's, then its actor's), then the object's box grid; the first held to the
            // anchor's own test (an object's actor too)
            let mut pts: Vec<(i32, i32)> = of.verbs.iter().chain(of.targets.iter()).filter(|p| want(&p.hit)).map(|p| (p.x, p.y)).collect();
            if let Anchor::Object { id, .. } = a {
                let st = e.status();
                let top = st.screen.map(|s| s.0).unwrap_or(0);
                if let Some(r) = o.room_objects.as_deref().unwrap_or(&[]).iter().find(|r| r.id == *id) {
                    pts.extend(afford::object_grid(r, st.xstart, top));
                }
            }
            let mut seen = BTreeSet::new();
            pts.retain(|p| seen.insert(*p));
            if pts.is_empty() {
                return Ok(None);
            }
            let ats = e.at_many(&pts)?;
            Ok(pts.iter().zip(ats.iter()).find(|(_, at)| same_hit(at, &o.verbs, a)).map(|(p, at)| (mk(p.0, p.1), hit_id(at, &o.verbs))))
        }
    }
}

/// The world keys (variables, bits, owners, states, classes, the room, ego): the keys a draw key is one of.
pub fn world(k: Key) -> bool {
    matches!(k.kind(), K_VAR | K_BIT | K_OWNER | K_STATE | K_CLASS | K_ROOM | K_EGO)
}

/// Item 3: a fork of `e` at its stop with its RNG state changed by `flip`.
pub fn twin(e: &mut Box<dyn Engine>, flip: u32) -> EResult<Box<dyn Engine>> {
    let s = (e.status().rng as u32) ^ flip;
    let mut t = e.fork()?;
    t.set_rng(s)?;
    Ok(t)
}

/// A step's twin forks' outcome at its end (amendment 1: marked when any twin differs).
#[derive(Clone, Debug, Default)]
pub struct Twin {
    pub frame: u64,
    /// a twin's end reason, when one ended before the frame
    pub ended: Option<String>,
    /// the world keys whose values differ there in some twin: (key, the replay's, the first differing twin's)
    pub keys: Vec<(Key, Val, Val)>,
    /// those of `keys` neither in I nor the interface's
    pub draw: Vec<Key>,
    /// the twins (by their index in TWIN_FLIPS) with a draw key
    pub by: Vec<usize>,
    /// a twin's ego stood elsewhere (ego's position is no key)
    pub ego: bool,
}

impl Twin {
    pub fn json(&self) -> Value {
        json!({"frame": self.frame, "ended": self.ended, "keys": self.keys.len(),
               "draw": self.draw.iter().map(|k| k.text()).collect::<Vec<_>>(), "by": self.by, "ego": self.ego})
    }

    /// Twin `i`'s outcome added to this one's.
    pub fn add(&mut self, i: usize, t: Twin) {
        if self.ended.is_none() {
            self.ended = t.ended;
        }
        for x in t.keys {
            if !self.keys.iter().any(|y| y.0 == x.0) {
                self.keys.push(x);
            }
        }
        self.keys.sort();
        if !t.draw.is_empty() || t.ego {
            self.by.push(i);
        }
        self.ego |= t.ego;
        let mut d: BTreeSet<Key> = self.draw.iter().copied().collect();
        d.extend(t.draw);
        self.draw = d.into_iter().collect();
    }
}

/// The twin run to `frame`, its world keys against `main`'s (the replay's facts there) and its ego's position against
/// `ego` (the replay's there); the twin is quit.
pub fn twin_diff(mut t: Box<dyn Engine>, frame: u64, main: &Facts, ego: Option<(i32, i32)>, held: &Held) -> Twin {
    let mut out = Twin { frame, ..Default::default() };
    let r = if t.status().frame < frame { t.run(&RunSpec { frame: Some(frame), ..Default::default() }).map(|_| ()) } else { Ok(()) };
    match r.and_then(|_| t.facts()) {
        Ok(f) => {
            out.keys = main.diff(&f).into_iter().filter(|(k, _, _)| world(*k)).collect();
            out.draw = out.keys.iter().map(|x| x.0).filter(|k| !held.is_idle(*k) && !held.iface.contains(k)).collect();
            out.ego = t.status().ego != ego;
            t.quit();
        }
        Err(EngineError::Ended(st)) => out.ended = Some(st.reason),
        Err(x) => {
            out.ended = Some(format!("error: {x}"));
            t.quit();
        }
    }
    out
}

/// Each input's anchor, timing class and identity, as the replay met them.
pub struct Labels {
    pub anchors: Vec<Anchor>,
    pub timing: Vec<Timing>,
    /// frames since the last taken input
    pub gap: Vec<Option<u64>>,
    pub ids: Vec<Option<Identity>>,
}

pub fn labels(ch: &Chain) -> Labels {
    let mut l = Labels { anchors: Vec::new(), timing: Vec::new(), gap: Vec::new(), ids: Vec::new() };
    let sh = sentence_handlers(ch);
    for m in &ch.met {
        l.anchors.push(anchor(&m.seen, &m.ev, dialog_of(m, &sh)));
        l.timing.push(timing_of(m));
        l.gap.push(m.gap);
        l.ids.push(m.record.as_ref().map(|r| identity(r, m.seen.hit)));
    }
    l
}

/// The steps as JSON lines (steps.jsonl): per step its inputs with their anchors, timing and identity, its progress,
/// keep and completion, its rooms and its twin.
pub fn steps_json(ch: &Chain, l: &Labels) -> Vec<Value> {
    ch.steps
        .iter()
        .map(|s| {
            let ins: Vec<Value> = s
                .inputs
                .clone()
                .map(|i| {
                    let m = &ch.met[i];
                    let hit = m.seen.at.as_ref().map(|a| json!({"main": a.main, "verb_slot": a.verb_slot, "zone": a.zone}));
                    json!({"i": i, "frame": m.ev.frame, "input": event_json(&m.ev), "anchor": l.anchors[i].json(), "hit": hit,
                           "timing": l.timing[i].name(), "gap": l.gap[i], "first_idle": m.first_idle, "idle": m.seen.idle,
                           "identity": l.ids[i].as_ref().map(Identity::json),
                           "raw_why": matches!(l.anchors[i], Anchor::Raw { .. }).then(|| raw_why(&m.seen, &m.ev))})
                })
                .collect();
            json!({"k": s.k, "inputs": ins, "start": s.start, "last": s.last, "end": s.end.json(),
                   "completion": s.completion.map(|(f, t)| json!({"frame": f, "tick": t})), "goal": s.goal,
                   "from": s.from.json(), "to": s.to.json(), "changed": s.changed.iter().map(|k| k.text()).collect::<Vec<_>>(),
                   "keep": s.keep.iter().map(|(k, v)| json!([k.text(), v])).collect::<Vec<_>>(), "rooms": s.rooms,
                   "twin": s.twin.as_ref().map(Twin::json)})
        })
        .collect()
}

/// The labels' summary: the steps, the anchor kinds, the inputs without an anchor and why, the timing classes.
pub fn summary(ch: &Chain, l: &Labels) -> Value {
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    let mut timing: BTreeMap<&str, usize> = BTreeMap::new();
    let mut raw = Vec::new();
    for (i, a) in l.anchors.iter().enumerate() {
        *kinds.entry(a.kind()).or_default() += 1;
        *timing.entry(l.timing[i].name()).or_default() += 1;
        if let Anchor::Raw { .. } = a {
            raw.push(json!({"i": i, "frame": ch.met[i].ev.frame, "why": raw_why(&ch.met[i].seen, &ch.met[i].ev)}));
        }
    }
    let verdicts: BTreeMap<String, usize> = l.ids.iter().fold(BTreeMap::new(), |mut m, x| {
        *m.entry(x.as_ref().map(|x| x.verdict.clone()).unwrap_or_else(|| "no record".into())).or_default() += 1;
        m
    });
    let with_actor = l.anchors.iter().filter(|a| matches!(a, Anchor::Object { actor, .. } if *actor != 0)).count();
    json!({"inputs": ch.met.len(), "steps": ch.steps.len(), "anchors": kinds, "objects_with_actor": with_actor, "raw": raw, "timing": timing,
           "verdicts": verdicts, "first": ch.first.json(),
           "end": ch.end.as_ref().map(|s| json!({"frame": s.frame, "tick": s.tick, "reason": s.reason})),
           "iface": {"at": ch.iface.at, "clicks": ch.iface.clicks.iter().map(|k| k.text()).collect::<Vec<_>>(),
                     "mouse": ch.iface.mouse.iter().map(|k| k.text()).collect::<Vec<_>>(), "probes": ch.iface.probes},
           "drift": ch.drift, "twin_differs": ch.twin_differs, "twin_draw_steps": ch.steps.iter().filter(|s| s.twin.as_ref().is_some_and(|t| !t.draw.is_empty())).map(|s| s.k).collect::<Vec<_>>()})
}

/// G2: the movie's inputs re-placed from their anchors at their own frames, from boot (no input scheduled but the
/// movie's skips). Per input: the point placed, the hit there, the record's identity against the replay's; per step:
/// keep(k) at its completion frame and at the frame before; the end against the replay's.
pub fn check(host: &dyn EngineHost, movie: &[Event], ch: &Chain, l: &Labels, o: &Opts) -> Res<Value> {
    let skips: Vec<Event> = movie.iter().filter(|e| matches!(e.what, Input::Skips { .. })).cloned().collect();
    let (mut e, mut sched) = chain::boot(host, &skips, o.seed, &o.out)?;
    // the stops: before each input; at each step's completion and the frame before it
    let mut at_frame: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (i, m) in ch.met.iter().enumerate() {
        at_frame.entry(m.ev.frame.saturating_sub(1).max(e.status().frame)).or_default().push(i);
    }
    let mut keeps: BTreeMap<u64, Vec<(usize, bool)>> = BTreeMap::new();
    for s in ch.steps.iter().filter(|s| s.goal.is_none()) {
        if let Some((c, _)) = s.completion {
            keeps.entry(c).or_default().push((s.k, true));
            keeps.entry(c.saturating_sub(1)).or_default().push((s.k, false));
        }
    }
    // and the last input's own frame, for its record before the game's end (amendment 1: a game that ends on the next
    // frame still gives it)
    let after = ch.met.last().map(|m| m.ev.frame).into_iter();
    let stops: BTreeSet<u64> = at_frame.keys().chain(keeps.keys()).copied().chain(after).collect();
    let n = ch.met.len();
    let mut placed: Vec<Option<Input>> = vec![None; n];
    // the hit at the placed point, by the stop's hit test alone (None for a key)
    let mut hit: Vec<Option<HitId>> = vec![None; n];
    let mut recs: BTreeMap<u64, InputRecord> = BTreeMap::new();
    let mut keep_at: BTreeMap<usize, (Option<bool>, Option<bool>)> = BTreeMap::new();
    // the keys of a keep that do not hold at its completion: (key, want, got)
    let mut keep_miss: BTreeMap<usize, Vec<(Key, Val, Val)>> = BTreeMap::new();
    let mut ended: Option<Status> = None;
    for &t in &stops {
        if e.status().frame < t {
            match e.run(&RunSpec { frame: Some(t), ..Default::default() }) {
                Ok(_) => {}
                Err(EngineError::Ended(st)) => {
                    ended = Some(*st);
                    break;
                }
                Err(x) => return Err(e2s(x)),
            }
        }
        for r in e.input_log().map_err(e2s)?.records {
            recs.entry(r.frame).or_insert(r);
        }
        if let Some(ks) = keeps.get(&t) {
            let f = e.facts().map_err(e2s)?;
            for &(k, at_c) in ks {
                let holds = ch.steps[k].keep.iter().all(|&(key, v)| f.val(key) == v);
                let x = keep_at.entry(k).or_default();
                if at_c {
                    x.0 = Some(holds);
                    let miss: Vec<(Key, Val, Val)> = ch.steps[k].keep.iter().filter(|&&(key, v)| f.val(key) != v).map(|&(key, v)| (key, v, f.val(key))).collect();
                    if !miss.is_empty() {
                        keep_miss.insert(k, miss);
                    }
                } else {
                    x.1 = Some(holds);
                }
            }
        }
        for &i in at_frame.get(&t).map(Vec::as_slice).unwrap_or(&[]) {
            let m = &ch.met[i];
            sched.upto(&mut e, m.ev.frame)?;
            let p = place(&mut e, &l.anchors[i], &m.ev.what).map_err(e2s)?;
            if let Some(inp) = &p {
                if let Some((x, y)) = point(inp) {
                    let verbs = e.obs(&ObsReq::default()).map_err(e2s)?.verbs;
                    let at = e.at(x, y).map_err(e2s)?;
                    hit[i] = Some(hit_id(&at, &verbs));
                }
                e.inputs(&[Event { frame: m.ev.frame, what: inp.clone() }]).map_err(e2s)?;
            }
            placed[i] = p;
        }
    }
    // on to the end
    if ended.is_none() {
        sched.upto(&mut e, u64::MAX)?;
        let to = ch.end.as_ref().map(|s| s.frame + 1).unwrap_or(e.status().frame).max(e.status().frame + 1);
        match e.run(&RunSpec { frame: Some(to), ..Default::default() }) {
            Ok(_) => {
                for r in e.input_log().map_err(e2s)?.records {
                    recs.entry(r.frame).or_insert(r);
                }
                e.quit();
            }
            Err(EngineError::Ended(st)) => ended = Some(*st),
            Err(x) => return Err(e2s(x)),
        }
    }
    let mut rows = Vec::new();
    let (mut n_placed, mut n_hit, mut n_id, mut n_moved) = (0, 0, 0, 0);
    for (i, m) in ch.met.iter().enumerate() {
        let got = recs.get(&m.ev.frame).map(|r| identity(r, hit[i].unwrap_or_default()));
        let same_id = got.is_some() && got == l.ids[i];
        // a key or a raw point counts by its record alone (amendment 1)
        let same = match l.anchors[i] {
            Anchor::Key { .. } | Anchor::Raw { .. } => same_id,
            _ => hit[i] == Some(m.seen.hit),
        };
        n_placed += placed[i].is_some() as usize;
        n_hit += same as usize;
        n_id += same_id as usize;
        let moved = placed[i].as_ref().is_some_and(|p| *p != m.ev.what);
        n_moved += moved as usize;
        if placed[i].is_none() || !same || !same_id || moved {
            rows.push(json!({"i": i, "frame": m.ev.frame, "anchor": l.anchors[i].json(),
                             "placed": placed[i].as_ref().map(|p| event_json(&Event { frame: m.ev.frame, what: p.clone() })),
                             "hit": hit[i].map(|h| h.json()), "same_hit": same, "identity": got.as_ref().map(Identity::json), "want": l.ids[i].as_ref().map(Identity::json)}));
        }
    }
    let keep_rows: Vec<Value> = keep_at
        .iter()
        .map(|(k, (c, b))| {
            let miss = keep_miss.get(k).map(|v| v.iter().map(|(key, w, g)| json!([key.text(), w, g])).collect::<Vec<_>>());
            json!({"k": k, "at_completion": c, "before": b, "miss": miss})
        })
        .collect();
    let n_keep = keep_at.values().filter(|x| x.0 == Some(true)).count();
    let n_keep_early = keep_at.values().filter(|x| x.1 == Some(true)).count();
    let want_end = ch.end.as_ref().map(|s| (s.frame, s.tick, s.reason.clone()));
    let got_end = ended.as_ref().map(|s| (s.frame, s.tick, s.reason.clone()));
    Ok(json!({"inputs": n, "placed": n_placed, "same_hit": n_hit, "same_identity": n_id, "moved": n_moved,
              "steps_checked": keep_at.len(), "keep_at_completion": n_keep, "keep_before_completion": n_keep_early,
              "end": got_end.as_ref().map(|e| json!([e.0, e.1, e.2])), "same_end": got_end.is_some() && got_end == want_end,
              "differ": rows, "keeps": keep_rows}))
}
