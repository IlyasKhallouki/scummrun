//! E012's affordance step (experiments/E012-game-mechanics/plan.md, section 4, "The interface's own inputs when the
//! model cannot act"; component `afford`, switch FMI_AFFORD, off by default).
//!
//! At a frame where the model cannot act, the inputs the interface itself offers there, each tried in a fork of the
//! engine, and the outcomes kept by IW(1) novelty:
//!
//! * **The offer** (`offer`): the visible verb slots (the dump lists the slots the engine's own hit and key tests see:
//!   verbid set, saveid 0, curmode 1), the room's objects not of class 32 (`findObject` skips
//!   kObjectClassUntouchable) and the actors in the room, each with a screen point at which the engine's hit test
//!   answers that verb slot, object or actor. The candidate points of every slot, object and actor go to the engine in
//!   one AtMany. No verb id, object id or game rule is named.
//! * **The inputs** (`affordances`): in a dialog idle one click per verb slot; in a sentence idle each verb slot alone,
//!   each target alone, and each verb then each target. In a trial, a third click over every target of the frame is
//!   added when the second click left the game composing (`composing`).
//! * **A trial** (`try_all`): a fork; the clicks two frames apart from the frame after the offer (M4's timing); then a
//!   run until the bridge's idle rule holds again or the horizon passes. M4's form (`AffordCfg::fixed`) runs every
//!   trial to a fixed horizon instead and takes its outcome against a no-input fork at the same frame.
//! * **Kept**: a stop whose state holds an atom of T or a room no archive entry holds (`Novelty`, the archive's), and
//!   none that a trial kept before it in this step. Each kept outcome is a real run: its movie is the entry's and the
//!   trial's clicks, so it verifies from boot like any entry (A0). A trial that is not kept can be made again.
//!
//! Every trial's *outcome* (for the measurements, M4) is its state's world keys (the tested keys, the room, every
//! owner and bit) against a no-input fork of the same entry at the same frame: the changes the inputs made there.

use crate::events;
use fmi_core::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

/// The switch (plan.md, "Components"): the try runs the affordance step only with FMI_AFFORD=1.
pub const SWITCH: &str = "FMI_AFFORD";

pub fn on() -> bool {
    std::env::var(SWITCH).is_ok_and(|v| v == "1")
}

/// The walk-box clicks' switch, end's FMI_BOXES (fmi-engine `boxes_on` reads the same variable): with it off the
/// Boxes command is never sent.
pub fn boxes_on() -> bool {
    std::env::var("FMI_BOXES").is_ok_and(|v| v == "1")
}

/// E014 amendment (8)'s switch `FMI_OFFER_SCREEN=1` (assumptions row 24): an actor's offer points are kept only on
/// the screen (0 <= x < SCREEN_W, 0 <= y < SCREEN_H), as the objects' points are cut to it and the walk boxes' to the
/// main screen. Off, an actor that stands off the screen is offered at a point the bridge's input check refuses
/// (x or y below 0) or that no player can click. Unset: as before.
pub fn offer_screen_on() -> bool {
    std::env::var("FMI_OFFER_SCREEN").is_ok_and(|v| v == "1")
}

/// SCUMM v5's screen (`_screenWidth`, `_screenHeight`): the hit test's coordinates.
pub const SCREEN_W: i32 = 320;
pub const SCREEN_H: i32 = 200;
/// The engine's variables that name the input script and the sentence script (scumm/vars.cpp, setupScummVars: v5).
pub const VAR_VERB_SCRIPT: usize = 32;
pub const VAR_SENTENCE_SCRIPT: usize = 33;
/// The trial's horizon when no wait gives one: E009's W, the measured length at which every idle set of 15 entries
/// equalled its 3,200-frame set (plan.md, section 1).
pub const W: u64 = 1600;

/// Points tried for a verb slot, from its rectangle's top left (`curRect.left`, `top`), in order (M4).
const VERB_DXY: [(i32, i32); 4] = [(2, 2), (4, 3), (1, 1), (0, 0)];
/// Fractions (in quarters) of an object's on-screen box tried for its point, in order (M4).
const GRID: [i32; 5] = [2, 1, 3, 0, 4];
/// Heights over an actor's feet tried for its point, in order (M4).
const ACTOR_DY: [i32; 4] = [-20, -30, -10, -40];
/// Walk boxes (boxes.h): a locked box ends a walk before it (actor.cpp:1284); an invisible one is skipped for the
/// player when a walk's end is put in a box (actor.cpp:2040); a v5 game's boxes start at 1 (actor.cpp:1990).
pub const BOX_LOCKED: u8 = 0x40;
pub const BOX_INVISIBLE: u8 = 0x80;
pub const FIRST_VALID_BOX: usize = 1;

/// What a click at a point hits, by the engine's hit test there.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Hit {
    Verb { slot: i32, id: i32 },
    Object(i32),
    Actor(i32),
    /// a floor point in a walk box (FMI_BOXES)
    Walk(i32),
}

impl Hit {
    /// The probe's name of the hit (M4's afford.tsv): v<verb id>, t<object id>, t-<actor id>.
    pub fn name(&self) -> String {
        match *self {
            Hit::Verb { id, .. } => format!("v{id}"),
            Hit::Object(o) => format!("t{o}"),
            Hit::Actor(a) => format!("t-{a}"),
            Hit::Walk(b) => format!("w{b}"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
    pub hit: Hit,
}

/// The interface at a frame: the bridge's idle kind there, the visible verb slots and the hit targets, each with a
/// point the engine's hit test confirms.
#[derive(Clone, Debug, Default)]
pub struct Offer {
    pub frame: u64,
    pub idle: Option<String>,
    pub verbs: Vec<Point>,
    pub targets: Vec<Point>,
    /// the walk boxes' floor points (FMI_BOXES; empty with it off)
    pub walks: Vec<Point>,
    /// the Boxes reply: the boxes, those the player's walk can end in (`walkable`); None with FMI_BOXES off
    pub boxes: Option<(usize, usize)>,
    /// what the dump listed: visible verb slots, room objects (all, and touchable with a box), actors in the room
    pub n_verbs: usize,
    pub n_objects: usize,
    pub n_touchable: usize,
    pub n_actors: usize,
    /// the hit tests asked (one AtMany)
    pub n_points: usize,
}

impl Offer {
    pub fn json(&self) -> Value {
        json!({"frame": self.frame, "idle": self.idle, "verbs": self.n_verbs, "verbs_hit": self.verbs.len(),
               "objects": self.n_objects, "touchable": self.n_touchable, "actors": self.n_actors,
               "targets_hit": self.targets.len(), "points": self.n_points,
               "boxes": self.boxes.map(|b| b.0), "walkable": self.boxes.map(|b| b.1), "walks": self.walks.len()})
    }
}

/// One trial's clicks, in order: click k at the offer's frame + 1 + 2k (M4's timing).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Inputs {
    pub clicks: Vec<Point>,
}

impl Inputs {
    /// The probe's name (M4's afford.tsv): the hits' names joined, e.g. "v8t316".
    pub fn name(&self) -> String {
        self.clicks.iter().map(|p| p.hit.name()).collect()
    }
    /// The clicks as input events at absolute frames, from the frame `f0` of the offer.
    pub fn events(&self, f0: u64) -> Vec<Event> {
        self.clicks
            .iter()
            .enumerate()
            .map(|(k, p)| Event { frame: f0 + 1 + 2 * k as u64, what: Input::Click(p.x, p.y) })
            .collect()
    }
}

/// Whether the player's walk can end in a box with these flags (the box's index from FIRST_VALID_BOX on).
pub fn walkable(flags: u8) -> bool {
    flags & (BOX_LOCKED | BOX_INVISIBLE) == 0
}

/// The candidate floor points of a walk box, in room coordinates: its centre (the corners' mean), then halfway from
/// the centre to each corner (ul, ur, lr, ll).
pub fn box_points(b: &WalkBox) -> Vec<(i32, i32)> {
    let cs = [b.ul, b.ur, b.lr, b.ll];
    let cx = cs.iter().map(|c| c.0).sum::<i32>().div_euclid(4);
    let cy = cs.iter().map(|c| c.1).sum::<i32>().div_euclid(4);
    let mut v = vec![(cx, cy)];
    v.extend(cs.iter().map(|c| ((cx + c.0).div_euclid(2), (cy + c.1).div_euclid(2))));
    v
}

/// The offer at the engine's stop, from its dump `o` (a full dump: verbs, room objects, classes, actors) and its
/// status (the camera's xstart, the screen layout, the idle kind); the walk boxes with FMI_BOXES.
pub fn offer(e: &mut Box<dyn Engine>, o: &Obs) -> EResult<Offer> {
    offer_with(e, o, boxes_on())
}

/// `offer`, the walk boxes asked when `boxes` (in a sentence idle only).
pub fn offer_with(e: &mut Box<dyn Engine>, o: &Obs, boxes: bool) -> EResult<Offer> {
    offer_cfg(e, o, boxes, offer_screen_on())
}

/// `offer_with`, an actor's points kept on the screen only when `screen` (`offer_screen_on`).
pub fn offer_cfg(e: &mut Box<dyn Engine>, o: &Obs, boxes: bool, screen: bool) -> EResult<Offer> {
    let st = e.status().clone();
    let xs = st.xstart;
    // the main screen's topline (patch 0004's layout in the status; 0 where the status has none)
    let top = st.screen.map(|s| s.0).unwrap_or(0);
    let untouchable: HashSet<u32> = o.classes.iter().filter(|(_, c)| c.contains(&32)).map(|(id, _)| *id).collect();
    let mut cands: Vec<(Hit, Vec<(i32, i32)>)> = Vec::new();
    for v in &o.verbs {
        let pts = VERB_DXY.iter().map(|&(dx, dy)| (v.x + dx, v.y + dy)).collect();
        cands.push((Hit::Verb { slot: v.slot, id: v.id }, pts));
    }
    let n_verbs = cands.len();
    let ros = o.room_objects.as_deref().unwrap_or(&[]);
    let mut n_touchable = 0;
    for r in ros {
        if untouchable.contains(&(r.id as u32)) || r.w <= 0 || r.h <= 0 {
            continue;
        }
        n_touchable += 1;
        let (x0, x1) = ((r.x - xs).max(0), (r.x + r.w - xs).min(SCREEN_W) - 1);
        let (y0, y1) = ((r.y + top).max(0), (r.y + r.h + top).min(SCREEN_H) - 1);
        if x0 > x1 || y0 > y1 {
            continue;
        }
        let mut pts = Vec::with_capacity(25);
        for fy in GRID {
            for fx in GRID {
                pts.push((x0 + (x1 - x0) * fx / 4, y0 + (y1 - y0) * fy / 4));
            }
        }
        cands.push((Hit::Object(r.id), pts));
    }
    for a in &o.actors {
        let pts: Vec<(i32, i32)> = ACTOR_DY
            .iter()
            .map(|&dy| (a.x - xs, a.y - a.elevation + dy + top))
            .filter(|&(x, y)| !screen || ((0..SCREEN_W).contains(&x) && (0..SCREEN_H).contains(&y)))
            .collect();
        if !pts.is_empty() {
            cands.push((Hit::Actor(a.id), pts));
        }
    }
    let mut box_counts = None;
    if boxes && st.idle.as_deref() == Some("sentence") {
        let bx = e.boxes()?;
        let (b, h) = st.screen.map(|s| (s.0, s.1)).unwrap_or((0, SCREEN_H));
        let mut n_walkable = 0;
        for (i, w) in bx.boxes.iter().enumerate().skip(FIRST_VALID_BOX) {
            if !walkable(w.flags) {
                continue;
            }
            n_walkable += 1;
            let pts: Vec<(i32, i32)> = box_points(w)
                .into_iter()
                .map(|(x, y)| (x - xs, y + top))
                .filter(|&(x, y)| (0..SCREEN_W).contains(&x) && (b..h).contains(&y))
                .collect();
            if !pts.is_empty() {
                cands.push((Hit::Walk(i as i32), pts));
            }
        }
        box_counts = Some((bx.boxes.len(), n_walkable));
    }
    let mut all: Vec<(i32, i32)> = cands.iter().flat_map(|(_, p)| p.iter().copied()).collect();
    all.sort_unstable();
    all.dedup();
    let ats = if all.is_empty() { Vec::new() } else { e.at_many(&all)? };
    let at_of = |p: (i32, i32)| all.binary_search(&p).ok().map(|i| &ats[i]);
    let mut out = Offer {
        frame: st.frame,
        idle: st.idle.clone(),
        boxes: box_counts,
        n_verbs,
        n_objects: ros.len(),
        n_touchable,
        n_actors: o.actors.len(),
        n_points: all.len(),
        ..Default::default()
    };
    for (hit, pts) in cands {
        let found = pts.into_iter().find(|&p| {
            at_of(p).is_some_and(|a| match hit {
                Hit::Verb { slot, .. } => a.verb_slot == slot,
                Hit::Object(id) => a.main.map(|m| m.2) == Some(id),
                Hit::Actor(id) => a.main.map(|m| m.3) == Some(id),
                // a floor click: the main screen, no object and no actor there
                Hit::Walk(_) => a.main.is_some_and(|m| m.2 == 0 && m.3 == 0),
            })
        });
        if let Some((x, y)) = found {
            let p = Point { x, y, hit };
            match hit {
                Hit::Verb { .. } => out.verbs.push(p),
                Hit::Walk(_) => out.walks.push(p),
                _ => out.targets.push(p),
            }
        }
    }
    Ok(out)
}

/// The inputs of an offer (plan.md, section 4, "Enumeration"): in a dialog idle each verb slot; in a sentence idle each
/// verb slot alone, each verb then each target, each target alone (M4's order), then each walk box's floor point
/// (FMI_BOXES). None when the bridge is not idle.
pub fn inputs_of(of: &Offer) -> Vec<Inputs> {
    let one = |p: &Point| Inputs { clicks: vec![*p] };
    match of.idle.as_deref() {
        Some("dialog") => of.verbs.iter().map(one).collect(),
        Some("sentence") => {
            let mut v: Vec<Inputs> = of.verbs.iter().map(one).collect();
            for a in &of.verbs {
                for t in &of.targets {
                    v.push(Inputs { clicks: vec![*a, *t] });
                }
            }
            v.extend(of.targets.iter().map(one));
            v.extend(of.walks.iter().map(one));
            v
        }
        _ => Vec::new(),
    }
}

/// The engine at a frame where the interface can be read: None when `e` is idle already (the offer is made on `e`),
/// else a fork of `e` run with no input to the bridge's first idle stop within `horizon` frames (Some(None): no idle
/// stop within them, or the run ended). No input is given on the way, so the fork's movie is `e`'s.
pub fn ready(e: &mut Box<dyn Engine>, horizon: u64) -> EResult<Option<Option<Box<dyn Engine>>>> {
    if e.status().idle.is_some() {
        return Ok(None);
    }
    let mut b = e.fork()?;
    let end = b.status().frame + horizon;
    match b.run(&RunSpec { idle: true, max_frame: Some(end), ..Default::default() }) {
        Ok(st) if st.reason == "idle" => Ok(Some(Some(b))),
        Ok(_) => {
            b.quit();
            Ok(Some(None))
        }
        Err(EngineError::Ended(_)) => Ok(Some(None)),
        Err(x) => Err(x),
    }
}

/// `affordances(engine, &Obs) -> Vec<Inputs>` (plan.md, "Components"): the offer at the engine's stop and its inputs.
pub fn affordances(e: &mut Box<dyn Engine>, o: &Obs) -> EResult<Vec<Inputs>> {
    Ok(inputs_of(&offer(e, o)?))
}

/// What the archive holds, as the step's novelty reads it (provider: the archive, its novelty over T and the rooms).
pub trait Novelty {
    /// The atoms of T and the room that this state holds and no archive entry holds, as texts (empty: not novel).
    fn novel(&self, f: &Facts) -> Vec<String>;
}

/// Novelty over (key, value) pairs: of the given keys and the room, the pairs no state in `seen` holds. The stand-in
/// the gate checks use for the archive's novelty over T's atoms.
pub struct PairNovelty {
    pub keys: HashSet<Key>,
    pub seen: HashSet<(Key, Val)>,
}

impl PairNovelty {
    pub fn new(keys: impl IntoIterator<Item = Key>) -> PairNovelty {
        let mut keys: HashSet<Key> = keys.into_iter().collect();
        keys.insert(Key::pack(K_ROOM, 0, 0, 0));
        PairNovelty { keys, seen: HashSet::new() }
    }
    pub fn add(&mut self, f: &Facts) {
        let ks: Vec<Key> = self.keys.iter().copied().collect();
        for k in ks {
            self.seen.insert((k, f.val(k)));
        }
    }
}

impl Novelty for PairNovelty {
    fn novel(&self, f: &Facts) -> Vec<String> {
        let mut out: Vec<String> =
            self.keys.iter().map(|&k| (k, f.val(k))).filter(|p| !self.seen.contains(p)).map(|(k, v)| format!("{k}={v}")).collect();
        out.sort();
        out
    }
}

/// More stops of a trial after its idle stop (the wait's stops, plan.md section 4 "then through the wait's stops"):
/// given the trial's engine at its stop, the checkpoints of later frames of the same run. The engine may be forked,
/// not moved.
pub type Then<'a> = &'a dyn Fn(&mut Box<dyn Engine>) -> Result<Vec<Checkpoint>, String>;

/// How the trials run.
#[derive(Clone, Debug)]
pub struct AffordCfg {
    /// frames after the offer's frame: the trial's last stop (the wait's horizon when the wait gives one, else W)
    pub horizon: u64,
    /// M4's form: every trial to the offer's frame + horizon, its outcome against a no-input fork there
    pub fixed: bool,
    /// the third click over every target of the frame when a verb-then-target trial left the game composing
    pub third: bool,
    /// E013's refused inputs (FMI_REFUSE=1): a trial whose every input the engine dropped or the handler ignored ends
    /// at the frame after its last click, with no wait
    pub refuse: bool,
    /// E013 (FMI_REFUSE=1, unless FMI_REFUSE_DEDUPE=0): a trial whose state at its stop has the digest of an earlier
    /// trial's stop, or of the no-input twin at the same frame, shares that state's wait: its stop and the wait's stops
    /// are not read again
    pub dedupe: bool,
    /// gate B4 (FMI_REFUSE_CHECK=1): a trial that shares a wait also runs its own, and the two are compared
    pub check: bool,
}

/// E013's switch (component refuse), read as fmi-play's `refuse::on`.
pub fn refuse_on() -> bool {
    std::env::var("FMI_REFUSE").is_ok_and(|v| v == "1")
}

impl Default for AffordCfg {
    fn default() -> Self {
        let refuse = refuse_on();
        AffordCfg {
            horizon: W,
            fixed: false,
            third: true,
            refuse,
            dedupe: refuse && std::env::var("FMI_REFUSE_DEDUPE").map_or(true, |v| v != "0"),
            check: refuse && std::env::var("FMI_REFUSE_CHECK").is_ok_and(|v| v == "1"),
        }
    }
}

/// One trial as run.
#[derive(Clone, Debug)]
pub struct Trial {
    pub inputs: Inputs,
    /// the clicks at absolute frames
    pub events: Vec<Event>,
    /// where the trial stopped, and why ("idle", "max_frame", "frame"); the end reason when the run ended
    pub frame: u64,
    pub reason: String,
    pub ended: bool,
    pub idle: Option<String>,
    pub room: i32,
    /// frames from the offer to the idle stop after the last click (None: no idle stop within the horizon)
    pub back: Option<u64>,
    /// the last click left the game composing (`composing`)
    pub composing: bool,
    /// the world keys' changes against no input at the same frame: (key, new value), in key order
    pub outcome: Vec<(Key, Val)>,
    /// the atoms the archive's novelty gave at the stop and the later stops, and whether a stop was kept
    pub novel: Vec<String>,
    pub kept: usize,
    pub seconds: f64,
    /// E014: the tick at the stop or at the end
    pub tick: u64,
    /// E013 (FMI_REFUSE): the verdict of the inputs this trial gave its engine ("taken" when any was taken, else the
    /// first one's: "dropped: gate", "ignored", ...); its stop's digest; the trial or the twin whose wait it shared
    pub verdict: Option<String>,
    pub digest: Option<u64>,
    pub shared: Option<String>,
}

/// A kept outcome: an entry to commit (its parent is the entry the step ran from).
#[derive(Clone, Debug)]
pub struct Kept {
    pub name: String,
    /// the trial's clicks at absolute frames: the entry's movie continues with them
    pub events: Vec<Event>,
    pub checkpoint: Checkpoint,
    /// the novel atoms it holds (those no earlier kept outcome of this step held)
    pub novel: Vec<String>,
    pub room: i32,
}

impl Kept {
    /// The entry's events as the archive's files write them.
    pub fn events_json(&self) -> Vec<Value> {
        self.events.iter().map(events::event_json).collect()
    }
}

/// The step's result.
#[derive(Clone, Debug, Default)]
pub struct AffordOut {
    pub offer: Offer,
    pub trials: Vec<Trial>,
    pub kept: Vec<Kept>,
    pub seconds: f64,
    /// the engines' CPU (the trials' forks and the control: /proc's ticks, read at each quit)
    pub engine_cpu_s: f64,
    /// this thread's CPU over the step (CLOCK_THREAD_CPUTIME_ID)
    pub own_cpu_s: f64,
    /// wall seconds by phase: the forks, the inputs and runs, the reads (facts, runs, dumps, hit tests), the quits,
    /// the no-input control
    pub phases: [f64; 5],
    /// E013 (FMI_REFUSE): the shared waits' counts and gate B4's pairs (FMI_REFUSE_CHECK)
    pub refuse: Option<RefuseOut>,
    /// E014 amendment (7): the trials whose inputs the bridge refused (its input check), each recorded as a refused
    /// trial: (the trial's index, the bridge's text)
    pub bridge_refused: Vec<(usize, String)>,
}

/// E014 amendment (7): a trial's verdict when the bridge refused its inputs (B's `dropped: ...`, "refused input" in
/// the stall table).
pub const BRIDGE_REFUSED: &str = "dropped: bridge";

/// E013's refused inputs in a step: digests taken, waits shared (with an earlier trial, with the no-input twin), the
/// twin's forks, and with FMI_REFUSE_CHECK the pairs whose own waits were run and compared (B4).
#[derive(Clone, Debug, Default)]
pub struct RefuseOut {
    pub digests: usize,
    pub shared_trial: usize,
    pub shared_twin: usize,
    pub twin_forks: usize,
    pub digest_s: f64,
    /// (trial, the trial or "twin" it shared with, same stops and last checkpoint)
    pub pairs: Vec<(usize, String, bool)>,
}

impl AffordOut {
    /// Distinct outcomes, each with its trials' names (BTreeMap: in outcome order).
    pub fn outcomes(&self) -> BTreeMap<Vec<(Key, Val)>, Vec<String>> {
        let mut m: BTreeMap<Vec<(Key, Val)>, Vec<String>> = BTreeMap::new();
        for t in &self.trials {
            m.entry(t.outcome.clone()).or_default().push(t.inputs.name());
        }
        m
    }
    /// Distinct outcomes on the world keys less the script-control keys (slot alive, position, arguments, freeze, the
    /// override: they cycle with animation scripts, plan.md section 6).
    pub fn outcomes_world(&self) -> usize {
        let mut s: HashSet<Vec<(Key, Val)>> = HashSet::new();
        for t in &self.trials {
            s.insert(t.outcome.iter().filter(|(k, _)| !control_key(*k)).copied().collect());
        }
        s.len()
    }
    /// The step's record (counts and timings; no game text).
    pub fn json(&self) -> Value {
        let n = self.trials.len();
        let mut rooms: BTreeMap<i32, usize> = BTreeMap::new();
        for t in &self.trials {
            *rooms.entry(t.room).or_default() += 1;
        }
        let mut backs: Vec<u64> = self.trials.iter().filter_map(|t| t.back).collect();
        backs.sort_unstable();
        let mut v = json!({"offer": self.offer.json(), "trials": n, "third": self.trials.iter().filter(|t| t.inputs.clicks.len() == 3).count(),
               "composing": self.trials.iter().filter(|t| t.composing).count(),
               "ended": self.trials.iter().filter(|t| t.ended).count(), "outcomes": self.outcomes().len(),
               "outcomes_world": self.outcomes_world(),
               "kept": self.kept.len(), "rooms": rooms.iter().map(|(r, c)| json!([r, c])).collect::<Vec<_>>(),
               "back_median": backs.get(backs.len() / 2), "seconds": self.seconds,
               "ms_per_trial": if n > 0 { 1e3 * self.seconds / n as f64 } else { 0.0 }, "engine_cpu_s": self.engine_cpu_s,
               "own_cpu_s": self.own_cpu_s,
               "cpu_ms_per_trial": if n > 0 { 1e3 * (self.engine_cpu_s + self.own_cpu_s) / n as f64 } else { 0.0 },
               "phases_s": {"fork": self.phases[0], "run": self.phases[1], "read": self.phases[2], "quit": self.phases[3],
                            "control": self.phases[4]}});
        if let Some(r) = &self.refuse {
            // E013: the trials by verdict, with their seconds (the stall table's rows), and the shared waits
            let mut by: BTreeMap<String, (usize, f64)> = BTreeMap::new();
            for t in &self.trials {
                let e = by.entry(t.verdict.clone().unwrap_or_else(|| "none".into())).or_default();
                e.0 += 1;
                e.1 += t.seconds;
            }
            v["verdicts"] = json!(by.iter().map(|(k, x)| (k.clone(), json!(x.0))).collect::<serde_json::Map<_, _>>());
            v["verdict_s"] = json!(by.iter().map(|(k, x)| (k.clone(), json!(x.1))).collect::<serde_json::Map<_, _>>());
            v["refuse"] = json!({"ended_at_once": self.trials.iter().filter(|t| t.reason == "refused").count(),
                                 "digests": r.digests, "shared_trial": r.shared_trial, "shared_twin": r.shared_twin,
                                 "twin_forks": r.twin_forks, "digest_s": r.digest_s,
                                 "waits_run": self.trials.iter().filter(|t| t.digest.is_some() && t.shared.is_none()).count(),
                                 "pairs": r.pairs.len(), "pairs_same": r.pairs.iter().filter(|p| p.2).count(),
                                 "pairs_differ": r.pairs.iter().filter(|p| !p.2).map(|p| json!([p.0, p.1])).collect::<Vec<_>>()});
        }
        if !self.bridge_refused.is_empty() {
            // E014 amendment (7): which inputs the bridge refused, at which frames, and its text
            v["bridge_refused"] = json!(self
                .bridge_refused
                .iter()
                .map(|(i, why)| {
                    let t = &self.trials[*i];
                    json!({"inputs": t.inputs.name(), "events": t.events.iter().map(crate::events::event_json).collect::<Vec<_>>(),
                           "why": why})
                })
                .collect::<Vec<_>>());
        }
        v
    }
}

/// A script-control key (plan.md, section 6): a slot alive, its position, its arguments, its freeze, the override.
pub fn control_key(k: Key) -> bool {
    matches!(k.kind(), K_SCRIPT | K_LOCAL | K_OBJSCRIPT | K_AT | K_FROZEN | K_POS | K_ESCPOS | K_ARGS | K_ESC)
}

/// The keys an outcome is read on: the tested keys, the room, every owner and every bit (M4's "world").
fn world(tested: &HashSet<Key>, k: Key) -> bool {
    tested.contains(&k) || matches!(k.kind(), K_ROOM | K_OWNER | K_BIT)
}

fn world_of(tested: &HashSet<Key>, f: &Facts) -> Facts {
    Facts(f.iter().filter(|(k, _)| world(tested, *k)).copied().collect())
}

/// The changes from `control` to `trial` on the world keys: (key, new value), in key order.
fn outcome_of(control: &Facts, trial: &Facts) -> Vec<(Key, Val)> {
    control.diff(trial).into_iter().map(|(k, _, new)| (k, new)).collect()
}

/// The checkpoint of a stop (replay.Harness.checkpoint: frame, facts, tick, rng, the actors sorted).
pub fn checkpoint_of(st: &Status, o: &Obs) -> Checkpoint {
    let mut actors: Vec<(i32, i32, i32)> = o.actors.iter().map(|a| (a.id, a.x, a.y)).collect();
    actors.sort_unstable();
    Checkpoint { frame: st.frame, facts: facts_of(o), tick: st.tick, rng: st.rng, actors }
}

/// The trial's last click left the game composing: the bridge is idle in a sentence again, and of the units a click
/// can start (the bridge's idle blockers: the input script, the sentence script, an object's verb script) only the
/// input script ran from the click's frame on. `runs`: the trial's slot runs (segment on); `input` and `sentence`:
/// the scripts VAR_VERB_SCRIPT and VAR_SENTENCE_SCRIPT named.
pub fn composing(idle: Option<&str>, runs: &[Run], from: u64, input: &[i32], sentence: i32) -> bool {
    if idle != Some("sentence") {
        return false;
    }
    let after: Vec<&Run> = runs.iter().filter(|r| r.frame >= from).collect();
    let ran_input = after.iter().any(|r| (r.where_ == WIO_GLOBAL || r.where_ == WIO_LOCAL) && input.contains(&r.number));
    let ran_sentence = after.iter().any(|r| r.where_ == WIO_GLOBAL && r.number == sentence);
    let ran_object = after.iter().any(|r| {
        matches!(r.where_, WIO_ROOM | WIO_INVENTORY | WIO_FLOBJECT) && r.number != ENCD && r.number != EXCD
    });
    ran_input && !ran_sentence && !ran_object
}

/// How a trial's run went: stopped (its status and the idle stop's frame, if any), ended (the end status), or, with
/// E013's refused inputs, stopped at the frame after its last click because the engine took none of its inputs.
enum Ran {
    Stopped(Status, Option<u64>),
    Ended(Status),
    Refused(Status),
}

/// The verdict of a trial's inputs on its engine (E013): "taken" when the handler took any of them, else the first
/// record's ("dropped: gate", "ignored", ...); "unread" with no record.
pub fn trial_verdict(recs: &[InputRecord]) -> String {
    if recs.iter().any(|r| r.verdict == Verdict::Taken) {
        return "taken".into();
    }
    recs.first().map(|r| r.verdict.text()).unwrap_or_else(|| "dropped: unread".into())
}

/// A trial's engine after its clicks: run to the frame after the last click, then to the bridge's idle stop or the
/// end frame `end`; in M4's form (`fixed`) then on to `end`. With `refuse` (E013) the input log is read at the frame
/// after the last click: its verdict is returned, and when no input was taken the trial stops there (Ran::Refused).
fn run_trial(b: &mut Box<dyn Engine>, last: u64, end: u64, fixed: bool, refuse: bool) -> EResult<(Ran, Option<String>)> {
    let go = |b: &mut Box<dyn Engine>, spec: RunSpec| -> EResult<Result<Status, Status>> {
        match b.run(&spec) {
            Ok(s) => Ok(Ok(s)),
            Err(EngineError::Ended(s)) => Ok(Err(*s)),
            Err(x) => Err(x),
        }
    };
    let mut st = match go(b, RunSpec { frame: Some(last + 1), ..Default::default() })? {
        Ok(s) => s,
        Err(s) => return Ok((Ran::Ended(s), None)),
    };
    let mut verdict = None;
    if refuse {
        let v = trial_verdict(&b.input_log()?.records);
        let stop = v != "taken";
        verdict = Some(v);
        if stop {
            return Ok((Ran::Refused(st), verdict));
        }
    }
    let mut back = None;
    if st.frame < end {
        st = match go(b, RunSpec { idle: true, max_frame: Some(end), ..Default::default() })? {
            Ok(s) => s,
            Err(s) => return Ok((Ran::Ended(s), verdict)),
        };
        if st.reason == "idle" {
            back = Some(st.frame);
        }
    }
    if fixed && st.frame < end {
        st = match go(b, RunSpec { frame: Some(end), ..Default::default() })? {
            Ok(s) => s,
            Err(s) => return Ok((Ran::Ended(s), verdict)),
        };
    }
    Ok((Ran::Stopped(st, back), verdict))
}

/// E013's shared waits (AffordCfg::dedupe): the digests of the trials' stops seen in this step, with the first trial
/// that stopped in each state and its stops (for gate B4's comparison), and the no-input twin: a fork of the offer's
/// engine run with no input to the frames asked, in order (forked again for an earlier frame).
#[derive(Default)]
struct Shared {
    seen: std::collections::HashMap<u64, (usize, u64, Vec<(u64, Checkpoint)>)>,
    twin: Option<Box<dyn Engine>>,
    twin_digest: std::collections::HashMap<u64, Option<u64>>,
}

impl Shared {
    /// The twin's digest at frame f (None: its run ended before f).
    fn twin_at(&mut self, e: &mut Box<dyn Engine>, f: u64, out: &mut RefuseOut) -> EResult<Option<u64>> {
        if let Some(d) = self.twin_digest.get(&f) {
            return Ok(*d);
        }
        if self.twin.as_ref().is_some_and(|t| t.status().frame > f) {
            self.twin.take().unwrap().quit();
        }
        if self.twin.is_none() {
            self.twin = Some(e.fork()?);
            out.twin_forks += 1;
        }
        let t = self.twin.as_mut().unwrap();
        let d = if t.status().frame == f {
            Some(t.digest()?.digest)
        } else {
            match t.run(&RunSpec { frame: Some(f), ..Default::default() }) {
                Ok(_) if t.status().frame == f => Some(t.digest()?.digest),
                Ok(_) => None,
                Err(EngineError::Ended(_)) => {
                    self.twin = None;
                    None
                }
                Err(x) => return Err(x),
            }
        };
        self.twin_digest.insert(f, d);
        Ok(d)
    }

    fn quit(&mut self) -> f64 {
        match self.twin.take() {
            Some(t) => {
                let c = t.cpu_s();
                t.quit();
                c
            }
            None => 0.0,
        }
    }
}

/// The stops of a wait relative to the trial's stop: (frames after it, checkpoint less its frame).
fn rel_stops(f: u64, cps: &[Checkpoint]) -> Vec<(u64, Checkpoint)> {
    cps.iter()
        .map(|c| (c.frame.saturating_sub(f), Checkpoint { frame: 0, facts: c.facts.clone(), tick: 0, rng: c.rng, actors: c.actors.clone() }))
        .collect()
}

/// The step's running state: the record, the atoms kept so far in this step (IW(1) within the step), and each trial's
/// world facts at its stop (for the outcomes, after the trials).
struct Run1<'a> {
    f0: u64,
    end: u64,
    cfg: &'a AffordCfg,
    tested: HashSet<Key>,
    novelty: &'a dyn Novelty,
    then: Option<Then<'a>>,
    out: AffordOut,
    kept_atoms: HashSet<String>,
    worlds: Vec<(u64, Option<Facts>)>,
    cpu: f64,
    shared: Shared,
}

impl Run1<'_> {
    /// E014 amendment (7): a trial whose inputs the bridge refused when they were scheduled on its fork (the bridge's
    /// input check): recorded as a refused trial (reason "refused", verdict `BRIDGE_REFUSED`, no run, no world, its
    /// seconds from `t0`), with the bridge's text in the step's record. The fork is not run; the step goes on.
    fn refused_by_bridge(&mut self, inputs: Inputs, events: Vec<Event>, st: &Status, why: String, t0: Instant) {
        let idx = self.out.trials.len();
        self.out.trials.push(Trial {
            inputs,
            events,
            frame: st.frame,
            reason: "refused".into(),
            ended: false,
            idle: None,
            room: st.room,
            back: None,
            composing: false,
            outcome: Vec::new(),
            novel: Vec::new(),
            kept: 0,
            seconds: t0.elapsed().as_secs_f64(),
            tick: st.tick,
            verdict: Some(BRIDGE_REFUSED.into()),
            digest: None,
            shared: None,
        });
        self.worlds.push((st.frame, None));
        self.out.bridge_refused.push((idx, why));
    }

    /// One trial on `b`, whose clicks `events` are scheduled: its run, its record, its world facts, and its stops kept
    /// by novelty. Returns the trial's index and its stop's status and facts (None: the run ended, or no input was
    /// taken). `e`: the offer's engine (the no-input twin's source, with E013's shared waits).
    fn trial(&mut self, e: &mut Box<dyn Engine>, b: &mut Box<dyn Engine>, inputs: Inputs, events: Vec<Event>)
             -> EResult<(usize, Option<(Status, Facts)>)> {
        let tt = Instant::now();
        let last = events.last().map(|x| x.frame).unwrap_or(self.f0);
        let mut tr = Trial {
            inputs,
            events,
            frame: 0,
            reason: String::new(),
            ended: false,
            idle: None,
            room: 0,
            back: None,
            composing: false,
            outcome: Vec::new(),
            novel: Vec::new(),
            kept: 0,
            seconds: 0.0,
            tick: 0,
            verdict: None,
            digest: None,
            shared: None,
        };
        let idx = self.out.trials.len();
        let tr0 = Instant::now();
        let (ran, verdict) = run_trial(b, last, self.end, self.cfg.fixed, self.cfg.refuse)?;
        tr.verdict = verdict;
        self.out.phases[1] += tr0.elapsed().as_secs_f64();
        let tr1 = Instant::now();
        let res = match ran {
            Ran::Ended(st) => {
                tr.ended = true;
                tr.reason = st.reason.clone();
                tr.frame = st.frame;
                tr.tick = st.tick;
                tr.room = st.room;
                self.worlds.push((st.frame, None));
                None
            }
            Ran::Refused(st) => {
                // E013: no input taken: the trial ends here, with no wait (its outcome still read against no input)
                tr.reason = "refused".into();
                tr.frame = st.frame;
                tr.tick = st.tick;
                tr.room = st.room;
                let f = b.facts()?;
                self.worlds.push((st.frame, Some(world_of(&self.tested, &f))));
                None
            }
            Ran::Stopped(st, back) => {
                tr.frame = st.frame;
                tr.tick = st.tick;
                tr.reason = st.reason.clone();
                tr.idle = st.idle.clone();
                tr.room = st.room;
                tr.back = back.map(|f| f - self.f0);
                let f = b.facts()?;
                self.worlds.push((st.frame, Some(world_of(&self.tested, &f))));
                // E013: the stop's digest; an earlier trial's stop or the no-input twin in the same state shares its wait
                let mut share: Option<(String, Option<Vec<(u64, Checkpoint)>>)> = None;
                if self.cfg.dedupe && !self.cfg.fixed {
                    let td = Instant::now();
                    let ro = self.out.refuse.get_or_insert_with(RefuseOut::default);
                    let d = b.digest()?.digest;
                    ro.digests += 1;
                    tr.digest = Some(d);
                    if let Some((k, _, stops)) = self.shared.seen.get(&d) {
                        share = Some((format!("trial {k}"), Some(stops.clone())));
                        ro.shared_trial += 1;
                    } else if self.shared.twin_at(e, st.frame, ro)? == Some(d) {
                        share = Some(("twin".into(), None));
                        ro.shared_twin += 1;
                    }
                    ro.digest_s += td.elapsed().as_secs_f64();
                }
                let mut stops: Vec<(Option<Checkpoint>, Facts)> = vec![(None, f.clone())];
                if share.is_none() || self.cfg.check {
                    if let (Some(th), false) = (self.then, self.cfg.fixed) {
                        let cps = th(b).map_err(EngineError::Bridge)?;
                        let rel = rel_stops(st.frame, &cps);
                        match &share {
                            // gate B4: the shared wait against this trial's own
                            Some((who, theirs)) => {
                                let same = match theirs {
                                    Some(t) => *t == rel,
                                    None => {
                                        // the twin's own wait from the same frame
                                        let mut tw = self.shared.twin.as_mut().unwrap().fork()?;
                                        let r = th(&mut tw).map_err(EngineError::Bridge)?;
                                        tw.quit();
                                        rel_stops(st.frame, &r) == rel
                                    }
                                };
                                self.out.refuse.get_or_insert_with(RefuseOut::default).pairs.push((idx, who.clone(), same));
                            }
                            None => {
                                if let Some(d) = tr.digest {
                                    self.shared.seen.insert(d, (idx, st.frame, rel));
                                }
                            }
                        }
                        if share.is_none() {
                            for c in cps {
                                let fc = c.facts.clone();
                                stops.push((Some(c), fc));
                            }
                        }
                    } else if let (None, Some(d)) = (&share, tr.digest) {
                        self.shared.seen.insert(d, (idx, st.frame, vec![]));
                    }
                }
                if let Some((who, _)) = &share {
                    // the stop's state is one this step has read (an earlier trial's) or the twin's (the try's wait
                    // reads the entry's no-input frames): neither the stop nor its wait gives anything new here
                    tr.shared = Some(who.clone());
                    stops.clear();
                }
                for (cp, fs) in stops {
                    let nov: Vec<String> = self.novelty.novel(&fs).into_iter().filter(|a| !self.kept_atoms.contains(a)).collect();
                    if nov.is_empty() {
                        continue;
                    }
                    let cp = match cp {
                        Some(c) => c,
                        None => checkpoint_of(&st, &b.obs(&ObsReq::default())?),
                    };
                    self.kept_atoms.extend(nov.iter().cloned());
                    tr.novel.extend(nov.iter().cloned());
                    tr.kept += 1;
                    let room = cp.facts.val(Key::pack(K_ROOM, 0, 0, 0)) as i32;
                    self.out.kept.push(Kept { name: tr.inputs.name(), events: tr.events.clone(), checkpoint: cp, novel: nov, room });
                }
                Some((st, f))
            }
        };
        self.out.phases[2] += tr1.elapsed().as_secs_f64();
        tr.seconds = tt.elapsed().as_secs_f64();
        self.out.trials.push(tr);
        Ok((idx, res))
    }
}

/// `try_all(engine, inputs, &Tested, archive) -> Vec<Kept>` (plan.md, "Components"): every input tried in a fork of
/// `e`, which is stopped at the offer's frame; each trial's outcome; the stops kept by novelty. `tested`: T's keys
/// (the outcomes' keys); `novelty`: the archive's; `then`: the stops after a trial's idle stop (the wait's).
pub fn try_all(
    e: &mut Box<dyn Engine>,
    of: &Offer,
    inputs: &[Inputs],
    tested: &[Key],
    novelty: &dyn Novelty,
    cfg: &AffordCfg,
    then: Option<Then<'_>>,
) -> EResult<AffordOut> {
    let t0 = Instant::now();
    let c0 = crate::machine::thread_cpu_s();
    let f0 = e.status().frame;
    let entry = e.facts()?;
    let sentence = entry.val(Key::var(VAR_SENTENCE_SCRIPT as u64)) as i32;
    let input0 = entry.val(Key::var(VAR_VERB_SCRIPT as u64)) as i32;
    let mut r = Run1 {
        f0,
        end: f0 + cfg.horizon,
        cfg,
        tested: tested.iter().copied().collect(),
        novelty,
        then,
        out: AffordOut { offer: of.clone(), refuse: cfg.refuse.then(RefuseOut::default), ..Default::default() },
        kept_atoms: HashSet::new(),
        worlds: Vec::new(),
        cpu: 0.0,
        shared: Shared::default(),
    };
    if cfg.refuse {
        // E013: the records the engine's movie left; every trial's fork starts with an empty input log
        e.input_log()?;
    }
    for inp in inputs {
        let tf = Instant::now();
        let mut b = e.fork()?;
        r.out.phases[0] += tf.elapsed().as_secs_f64();
        let evs = inp.events(f0);
        let check = cfg.third && !cfg.fixed && inp.clicks.len() == 2;
        let ti = Instant::now();
        if check {
            b.segment(true)?;
            b.runs()?;
        }
        match b.inputs(&evs) {
            Ok(()) => {}
            // E014 amendment (7): an input the bridge refuses is one refused trial, and the step goes on
            Err(EngineError::Bridge(why)) => {
                r.out.phases[1] += ti.elapsed().as_secs_f64();
                let st = b.status().clone();
                r.refused_by_bridge(inp.clone(), evs, &st, why, ti);
                r.cpu += b.cpu_s();
                let tq = Instant::now();
                b.quit();
                r.out.phases[3] += tq.elapsed().as_secs_f64();
                continue;
            }
            Err(x) => return Err(x),
        }
        r.out.phases[1] += ti.elapsed().as_secs_f64();
        let last = evs.last().map(|x| x.frame).unwrap_or(f0);
        let (i, res) = r.trial(e, &mut b, inp.clone(), evs.clone())?;
        if let (true, Some((st, f))) = (check, res) {
            let tr = Instant::now();
            let runs = b.runs()?;
            r.out.phases[2] += tr.elapsed().as_secs_f64();
            let input = [input0, f.val(Key::var(VAR_VERB_SCRIPT as u64)) as i32];
            let comp = composing(st.idle.as_deref(), &runs.runs, last, &input, sentence);
            r.out.trials[i].composing = comp;
            if comp && st.frame < r.end {
                // the third click: over every target the hit test finds at this stop
                let o = b.obs(&ObsReq::default())?;
                let of2 = offer_with(&mut b, &o, false)?;
                for t in &of2.targets {
                    let tf = Instant::now();
                    let mut c = b.fork()?;
                    r.out.phases[0] += tf.elapsed().as_secs_f64();
                    let mut evs3 = evs.clone();
                    evs3.push(Event { frame: st.frame + 1, what: Input::Click(t.x, t.y) });
                    let ti = Instant::now();
                    let inp3 = Inputs { clicks: vec![inp.clicks[0], inp.clicks[1], *t] };
                    match c.inputs(&evs3[evs3.len() - 1..]) {
                        Ok(()) => {
                            r.out.phases[1] += ti.elapsed().as_secs_f64();
                            r.trial(e, &mut c, inp3, evs3)?;
                        }
                        // E014 amendment (7): as the trial's first clicks
                        Err(EngineError::Bridge(why)) => {
                            r.out.phases[1] += ti.elapsed().as_secs_f64();
                            let st3 = c.status().clone();
                            r.refused_by_bridge(inp3, evs3, &st3, why, ti);
                        }
                        Err(x) => return Err(x),
                    }
                    r.cpu += c.cpu_s();
                    let tq = Instant::now();
                    c.quit();
                    r.out.phases[3] += tq.elapsed().as_secs_f64();
                }
            }
        }
        r.cpu += b.cpu_s();
        let tq = Instant::now();
        b.quit();
        r.out.phases[3] += tq.elapsed().as_secs_f64();
    }
    // the outcomes: each trial's world keys against a no-input fork at its stop frame (one fork, run through the frames)
    let mut frames: Vec<u64> = r.worlds.iter().filter(|w| w.1.is_some()).map(|w| w.0).collect();
    frames.sort_unstable();
    frames.dedup();
    let mut control: BTreeMap<u64, Facts> = BTreeMap::new();
    let tc = Instant::now();
    if !frames.is_empty() {
        let mut c = e.fork()?;
        for &fr in &frames {
            if fr > c.status().frame {
                match c.run(&RunSpec { frame: Some(fr), ..Default::default() }) {
                    Ok(_) => {}
                    Err(EngineError::Ended(_)) => break,
                    Err(x) => return Err(x),
                }
            }
            if c.status().frame == fr {
                control.insert(fr, world_of(&r.tested, &c.facts()?));
            }
        }
        r.cpu += c.cpu_s();
        c.quit();
    }
    r.out.phases[4] += tc.elapsed().as_secs_f64();
    r.cpu += r.shared.quit();
    for (t, (fr, w)) in r.out.trials.iter_mut().zip(r.worlds.iter()) {
        if let (Some(w), Some(c)) = (w, control.get(fr)) {
            t.outcome = outcome_of(c, w);
        }
    }
    r.out.engine_cpu_s = r.cpu;
    r.out.own_cpu_s = crate::machine::thread_cpu_s() - c0;
    r.out.seconds = t0.elapsed().as_secs_f64();
    Ok(r.out)
}
