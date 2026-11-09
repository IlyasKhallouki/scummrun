//! E024 Part 2, item 1 (plan.md, "Steps"): the steps of a movie from boot.
//!
//! * **Progress** (`Progress`): at a control stop (the bridge's idle rule holds), the room, the objects ego owns and
//!   the archive's held atoms (`Held`: Archive::held's rule, archive.rs, over the stage's guide atoms, its idle keys I
//!   and its root's facts, read from the archive folder; no chain of entries is needed), less the interface's keys.
//! * **The interface's keys** (`interface`): the guide's atoms include the keys the interface itself sets on every
//!   input (the mouse's position and its copies, the sentence being composed, a dialog's lines), so with Archive::held
//!   as it is nearly every input changes the progress. They are measured, not named (amendment 1), from forks at the
//!   first control stop of each input handler the movie meets (VAR_VERB_SCRIPT's value there: a game whose first
//!   control is a dialog shows its sentence interface only later): the keys a click that makes nothing writes (the
//!   probes click on empty space, in the verbs' screen and in the main screen, and on a verb slot with no object after
//!   it: its record's variables and bits, and the world keys that differ from a no-input fork `PROBE_FRAMES` frames
//!   later) and those a mouse move alone changes (to two corners of the screen, onto a target). A click makes nothing
//!   (`made_nothing`, amendment 1(d)'s refused click) when the game refused it, or when its handler only recorded it:
//!   wrote variables and bits and started no script but its own run (at the sentence interface a click on empty space
//!   records its point, which the input log calls taken). A probe click that acted (a dialog line chosen, a walk) is
//!   reported, its keys not counted (the review of stage A). A key only a step's own taken input writes stays in its
//!   progress.
//! * **The replay** (`replay`): the movie from boot, frame by frame. At the stop before each input label.rs reads what
//!   the input will hit; that stop is also the control stop after the input before it, where its facts are kept and
//!   the input log's records so far taken (the last input's at its own frame's stop, so a game that ends on the next
//!   frame still gives it). Every frame's changes to the progress keys are logged, so a step's completion is the first
//!   frame at which its keep holds. A second replay forks each step's twins (item 3).
//! * **Steps** (`Step`): the inputs between two progress changes. An input whose next stop shows no change, or is not
//!   a control stop, merges into the next step; so does one followed by an input of timing class "next" (label.rs):
//!   that input is placed as soon as possible after it (a verb, then its object), and the stop between holds the
//!   interface's own state (the sentence being composed), not progress. The last step ends at the game's end.
//! * **keep(k)** (plan.md item 4): the values at step k's end of every key the steps up to k changed.

use crate::label::{self, Seen, Twin};
use crate::{e2s, Res};
use fmi_core::*;
use fmi_explore::archive::{self, Atom, FactLook, FactMap};
use fmi_explore::events::event_json;
use fmi_explore::files;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

pub const ROOM: Key = Key::pack(K_ROOM, 0, 0, 0);
pub const EGO: Key = Key::pack(K_EGO, 0, 0, 0);

/// The archive's held atoms (Archive::held, archive.rs:983) without the archive's rows: the guide's atoms, the idle
/// keys I and the root's facts; and the interface's keys, left out of the progress.
pub struct Held {
    pub atoms: BTreeMap<String, Atom>,
    pub idle: BTreeSet<String>,
    pub root: FactMap,
    pub iface: BTreeSet<Key>,
    /// the atoms' keys, packed (the frame log's filter)
    keys: HashSet<Key>,
}

impl Held {
    pub fn new(atoms: BTreeMap<String, Atom>, idle: BTreeSet<String>, root: FactMap) -> Held {
        let keys = atoms.keys().map(|k| Key::parse(k)).collect();
        Held { atoms, idle, root, iface: BTreeSet::new(), keys }
    }

    /// From an archive folder, read only: stage.json's guide; I as Archive::load makes it (the committed log lines'
    /// idle keys less the goal's, when the stage measures idle frames); entry 0's last checkpoint as the root's facts.
    pub fn load(root: &Path) -> Res<Held> {
        let stage = files::read_json(&root.join("stage.json"))?;
        let guide = archive::load_guide(Path::new(stage["guide"].as_str().ok_or("stage.json: guide")?))?;
        let atoms = archive::guide_atoms(&guide)?;
        let goal: HashSet<String> = stage["goal"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
        let mut idle = BTreeSet::new();
        if files::as_i64(stage["e009"].get("idle_frames")).unwrap_or(0) != 0 {
            let (logs, _) = files::read_jsonl(&root.join("log.jsonl"))?;
            for x in logs.iter().filter(|x| archive::committed(x) && files::truthy(x.get("idle"))) {
                for k in x["idle"]["keys"].as_array().into_iter().flatten().filter_map(|k| k.as_str()) {
                    if !goal.contains(k) {
                        idle.insert(k.to_string());
                    }
                }
            }
        }
        let e0 = files::read_gz(&root.join("entries").join("0.json.gz"))?;
        let cp = e0["checkpoints"].as_array().and_then(|a| a.last()).ok_or("entry 0: no checkpoint")?;
        Ok(Held::new(atoms, idle, archive::fact_map(&cp[1])))
    }

    /// The atoms `f` holds, as (key, value), sorted (Archive::held), less the interface's keys.
    pub fn held(&self, f: &Facts) -> Vec<(String, i64)> {
        let mut out = Vec::new();
        for (k, a) in &self.atoms {
            if self.idle.contains(k) || self.iface.contains(&Key::parse(k)) {
                continue;
            }
            let v = f.look(k).unwrap_or(0);
            let rv = self.root.get(k).copied().unwrap_or(0);
            if a.values.contains(&v) || (a.any && v != rv) {
                out.push((k.clone(), v));
            }
        }
        out.sort();
        out
    }

    /// A key the progress reads: the room, ego, an owner, an atom's key.
    pub fn watched(&self, k: Key) -> bool {
        matches!(k.kind(), K_ROOM | K_EGO | K_OWNER) || self.keys.contains(&k)
    }

    /// A key of I (measured idle at some entry).
    pub fn is_idle(&self, k: Key) -> bool {
        self.idle.contains(&k.text())
    }
}

/// The progress at a control stop (plan.md item 1).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Progress {
    pub room: i64,
    pub owned: Vec<i64>,
    pub held: Vec<(String, i64)>,
}

impl Progress {
    pub fn of(f: &Facts, h: &Held) -> Progress {
        let mut owned: Vec<i64> = match f.get(EGO) {
            Some(e) if e != 0 => f.owners().into_iter().filter(|(_, v)| *v == e).map(|(o, _)| o).collect(),
            _ => Vec::new(),
        };
        owned.sort_unstable();
        Progress { room: f.val(ROOM), owned, held: h.held(f) }
    }

    /// The keys whose part of the progress differs from `to`'s: the room, owner:O of each object that came or went,
    /// each atom key held with another value or not at all.
    pub fn changed(&self, to: &Progress) -> Vec<Key> {
        let mut ks: BTreeSet<Key> = BTreeSet::new();
        if self.room != to.room {
            ks.insert(ROOM);
        }
        let (a, b): (BTreeSet<&i64>, BTreeSet<&i64>) = (self.owned.iter().collect(), to.owned.iter().collect());
        ks.extend(a.symmetric_difference(&b).map(|&&o| Key::owner(o as u64)));
        let (a, b): (BTreeSet<&(String, i64)>, BTreeSet<&(String, i64)>) = (self.held.iter().collect(), to.held.iter().collect());
        ks.extend(a.symmetric_difference(&b).map(|(k, _)| Key::parse(k)));
        ks.into_iter().collect()
    }

    pub fn json(&self) -> Value {
        json!({"room": self.room, "owned": self.owned, "held": self.held.len()})
    }
}

/// A stop of the replay.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stop {
    pub frame: u64,
    pub tick: u64,
    pub idle: Option<String>,
    pub room: i32,
    /// ego's position
    pub ego: Option<(i32, i32)>,
}

impl Stop {
    pub fn of(s: &Status) -> Stop {
        Stop { frame: s.frame, tick: s.tick, idle: s.idle.clone(), room: s.room, ego: s.ego }
    }
    pub fn json(&self) -> Value {
        json!({"frame": self.frame, "tick": self.tick, "idle": self.idle, "room": self.room, "ego": self.ego})
    }
}

/// One input of the movie as the replay met it.
#[derive(Clone, Debug)]
pub struct Met {
    pub ev: Event,
    /// what label.rs read at the stop before it
    pub seen: Seen,
    /// the input log's record of its frame
    pub record: Option<InputRecord>,
    /// frames since the last input the input log says was taken (None: none before)
    pub gap: Option<u64>,
    /// the stop before it is the first control stop after the input before
    pub first_idle: bool,
    /// the stop before the next input (the last stop before the end for the last input), and the facts there
    pub next: Stop,
    pub facts: Facts,
}

/// One step (plan.md item 1).
#[derive(Clone, Debug)]
pub struct Step {
    pub k: usize,
    /// the indices of its inputs in `Chain::met`
    pub inputs: Range<usize>,
    pub from: Progress,
    pub to: Progress,
    pub changed: Vec<Key>,
    /// keep(k): every key the steps up to k changed, with its value at this step's end
    pub keep: Vec<(Key, Val)>,
    /// the frames of its first and last inputs; the stop where its progress was taken (the last stop before the end
    /// for the last step)
    pub start: u64,
    pub last: u64,
    pub end: Stop,
    /// the first (frame, tick) after its last input at which keep(k) holds; the end's for the last step
    pub completion: Option<(u64, u64)>,
    /// the rooms it was in
    pub rooms: Vec<i64>,
    /// the twin fork's keys (item 3)
    pub twin: Option<Twin>,
    /// the last step: the game's end reason
    pub goal: Option<String>,
    /// ego (its actor) at the step's end
    pub ego: Val,
}

/// The replay's result.
pub struct Chain {
    pub met: Vec<Met>,
    pub steps: Vec<Step>,
    /// the first stop (the boot's) and the game's end, when the run ended
    pub first: Stop,
    pub end: Option<Status>,
    /// the progress keys' changes: (frame, key, new value), in frame order
    pub log: Vec<(u64, Key, Val)>,
    /// the progress keys' values at the first stop
    pub init: Facts,
    /// the facts at the stop before the first input
    pub start: Facts,
    /// tick at each frame's stop (index: frame)
    pub ticks: Vec<u64>,
    /// the interface's keys measured (`interface`)
    pub iface: Iface,
    /// control stops where the facts kept from the frames' changes differed from the engine's (keys)
    pub drift: usize,
    /// the twins' replay against the first at the steps' ends (keys)
    pub twin_differs: usize,
}

/// The replay's settings.
#[derive(Clone, Debug)]
pub struct Opts {
    pub seed: u32,
    pub out: PathBuf,
    /// a twin fork per step (item 3)
    pub twins: bool,
    /// frames run after the last input before the run is said not to end
    pub tail: u64,
}

/// What the frames passed so far left behind.
struct Track<'a> {
    held: &'a Held,
    facts: Facts,
    log: Vec<(u64, Key, Val)>,
    ticks: Vec<u64>,
    /// the first control stop after `since`
    since: u64,
    first_idle: Option<u64>,
}

impl Track<'_> {
    /// Frames from the stop up to `to`, one at a time.
    fn run(&mut self, e: &mut Box<dyn Engine>, to: u64) -> EResult<()> {
        let n = to.saturating_sub(e.status().frame);
        if n == 0 {
            return Ok(());
        }
        e.wait(n, &mut |st, ch| {
            self.facts.patch(ch);
            for &(k, _, new) in ch {
                if self.held.watched(k) {
                    self.log.push((st.frame, k, new));
                }
            }
            put_tick(&mut self.ticks, st);
            if st.idle.is_some() && st.frame > self.since && self.first_idle.is_none() {
                self.first_idle = Some(st.frame);
            }
            false
        })?;
        Ok(())
    }
}

fn put_tick(ticks: &mut Vec<u64>, st: &Status) {
    let f = st.frame as usize;
    if ticks.len() <= f {
        ticks.resize(f + 1, 0);
    }
    ticks[f] = st.tick;
}

/// The input log's records so far, by frame.
fn take_log(e: &mut Box<dyn Engine>, recs: &mut BTreeMap<u64, Vec<InputRecord>>) -> Res<()> {
    for r in e.input_log().map_err(e2s)?.records {
        recs.entry(r.frame).or_default().push(r);
    }
    Ok(())
}

/// The seed the archive's run played on (stage.json's), the seed its movies were recorded on.
pub fn archive_seed(root: &Path) -> Option<u32> {
    files::read_json(&root.join("stage.json")).ok()?["seed"].as_u64().map(|s| s as u32)
}

/// An archive entry's movie from boot (Archive::movie's: the events of the chain of entries from the root to it),
/// read only from the archive's entries.jsonl and entries/N.json.gz.
pub fn archive_movie(root: &Path, entry: usize) -> Res<Vec<Event>> {
    let (rows, _) = files::read_jsonl(&root.join("entries.jsonl"))?;
    let parent: BTreeMap<u64, Option<u64>> = rows.iter().filter_map(|r| Some((r["id"].as_u64()?, r["parent"].as_u64()))).collect();
    let mut chain = Vec::new();
    let mut cur = Some(entry as u64);
    while let Some(i) = cur {
        chain.push(i);
        cur = *parent.get(&i).ok_or(format!("{}: no entry {i}", root.display()))?;
    }
    let mut out = Vec::new();
    for i in chain.into_iter().rev() {
        let doc = files::read_gz(&root.join("entries").join(format!("{i}.json.gz")))?;
        for v in doc["events"].as_array().into_iter().flatten() {
            out.push(fmi_explore::events::event_of(v).ok_or(format!("entry {i}: not an event: {v}"))?);
        }
    }
    Ok(out)
}

/// The movie's inputs (keys, clicks, moves), in frame order.
pub fn inputs_of(movie: &[Event]) -> Vec<Event> {
    let mut v: Vec<Event> = movie.iter().filter(|e| !matches!(e.what, Input::Skips { .. })).cloned().collect();
    v.sort_by_key(|e| e.frame);
    v
}

/// The first frame a boot runs to: the engine's first stop is at frame 1, and a boot's run goes past it.
pub const BOOT_FRAME: u64 = 2;

/// The engine from boot with the movie's events to BOOT_FRAME (SPEEDRUN_INPUTS), stopped at BOOT_FRAME; the later
/// ones go in through `Sched`.
pub fn boot(host: &dyn EngineHost, movie: &[Event], seed: u32, out: &Path) -> Res<(Box<dyn Engine>, Sched)> {
    let cfg = BootCfg { seed, events: movie.to_vec(), frame: Some(BOOT_FRAME), dump_locals: false, lean: true, out_dir: out.to_path_buf() };
    let e = host.boot(&cfg).map_err(e2s)?;
    let mut later: Vec<Event> = movie.iter().filter(|x| x.frame > BOOT_FRAME).cloned().collect();
    later.sort_by_key(|x| x.frame);
    Ok((e, Sched { later, next: 0 }))
}

/// A movie's events after the boot, scheduled in frame order as the run reaches them (the bridge takes no event
/// before one it holds, so a fork can still be given its own).
pub struct Sched {
    later: Vec<Event>,
    next: usize,
}

impl Sched {
    /// Schedules the events up to `frame`.
    pub fn upto(&mut self, e: &mut Box<dyn Engine>, frame: u64) -> Res<()> {
        let k = self.later[self.next..].iter().take_while(|x| x.frame <= frame).count();
        if k > 0 {
            e.inputs(&self.later[self.next..self.next + k]).map_err(e2s)?;
            self.next += k;
        }
        Ok(())
    }
}

/// Frames a probe of the interface runs after its input's frame: the handler's frame and the scripts that answer it.
pub const PROBE_FRAMES: u64 = 8;
/// The probe's grid for a point no slot answers: every GRID_STEP pixels of the screen.
const GRID_STEP: i32 = 4;

/// The interface's keys (module docstring), measured at control stops.
#[derive(Clone, Debug, Default)]
pub struct Iface {
    /// by the clicks that make nothing
    pub clicks: BTreeSet<Key>,
    /// by a mouse move
    pub mouse: BTreeSet<Key>,
    /// the stops measured: (frame, the input handler there)
    pub at: Vec<(u64, Val)>,
    /// each probe: its stop, its input, its record's verdict, its keys
    pub probes: Vec<Value>,
}

impl Iface {
    fn add(&mut self, handler: Val, x: Iface) {
        let frame = x.at.first().map(|a| a.0).unwrap_or(0);
        self.at.push((frame, handler));
        self.clicks.extend(x.clicks);
        self.mouse.extend(x.mouse);
        self.probes.extend(x.probes.into_iter().map(|mut p| {
            p["handler"] = json!(handler);
            p
        }));
    }
}

/// The variables and bits a record says the handler wrote.
pub fn written(r: &InputRecord) -> BTreeSet<Key> {
    r.writes
        .iter()
        .filter_map(|t| match t.kind {
            TouchKind::Var => Some(Key::var(t.a as u64)),
            TouchKind::Bit => Some(Key::bit(t.a as u64)),
            _ => None,
        })
        .collect()
}

/// Whether a probe click made nothing (module docstring) by its record (None: none, it never reached the handler):
/// dropped or ignored; or taken with writes to variables and bits alone and no script started but the handler's own
/// run. A sentence queued, a walk, a message, a verb slot, the cursor, the gate, an object, an actor or the camera
/// changed, or another script started, is an act.
pub fn made_nothing(r: Option<&InputRecord>) -> bool {
    r.is_none_or(|r| {
        r.verdict.refused()
            || r.writes.iter().all(|t| match t.kind {
                TouchKind::Var | TouchKind::Bit => true,
                TouchKind::Started => t.a == r.handler,
                _ => false,
            })
    })
}

/// A fork of `e` with `input` at the next frame, run `PROBE_FRAMES` frames past it: its facts there and the input's
/// record (None: the game ended first).
fn probe(e: &mut Box<dyn Engine>, input: Option<&Input>) -> EResult<Option<(Facts, Option<InputRecord>)>> {
    let f = e.status().frame + 1;
    let mut t = e.fork()?;
    if let Some(i) = input {
        t.inputs(&[Event { frame: f, what: i.clone() }])?;
    }
    let run = |t: &mut Box<dyn Engine>| -> EResult<(Facts, Option<InputRecord>)> {
        t.run(&RunSpec { frame: Some(f), ..Default::default() })?;
        let rec = t.input_log()?.records.into_iter().find(|r| r.frame == f);
        t.run(&RunSpec { frame: Some(f + PROBE_FRAMES), ..Default::default() })?;
        Ok((t.facts()?, rec))
    };
    match run(&mut t) {
        Ok(x) => {
            t.quit();
            Ok(Some(x))
        }
        Err(EngineError::Ended(_)) => Ok(None),
        Err(x) => {
            t.quit();
            Err(x)
        }
    }
}

/// The world keys that differ between two facts.
fn world_diff(a: &Facts, b: &Facts) -> BTreeSet<Key> {
    a.diff(b).into_iter().map(|d| d.0).filter(|k| label::world(*k)).collect()
}

/// The interface's keys at `e`'s stop (a control stop), against a no-input fork: forks with the mouse moved to two
/// corners of the screen and onto the offer's first target (the hover); with a click on empty space, on a point of
/// the verbs' screen no slot answers and on a point of the main screen no object or actor answers (a walk, which
/// changes nothing but where ego stands); and with a click on the offer's first verb slot with no object after it.
pub fn interface(e: &mut Box<dyn Engine>) -> EResult<Iface> {
    use fmi_explore::afford::{self, SCREEN_H, SCREEN_W};
    let frame = e.status().frame;
    let mut out = Iface { at: vec![(frame, 0)], ..Default::default() };
    let o = e.obs(&ObsReq::default())?;
    let of = afford::offer_cfg(e, &o, false, afford::offer_screen_on())?;
    let Some((none, _)) = probe(e, None)? else { return Ok(out) };
    let mut moves = vec![("corner", Input::Move(0, 0)), ("corner", Input::Move(SCREEN_W - 1, SCREEN_H - 1))];
    if let Some(t) = of.targets.first() {
        moves.push(("target", Input::Move(t.x, t.y)));
    }
    let mut prev: Option<Facts> = None;
    for (what, i) in moves {
        let Some((f, _)) = probe(e, Some(&i))? else { continue };
        let mut keys = world_diff(&none, &f);
        if let Some(p) = &prev {
            keys.extend(world_diff(p, &f));
        }
        out.probes.push(json!({"frame": frame, "input": what, "event": event_json(&Event { frame: frame + 1, what: i }),
                               "keys": keys.iter().map(|k| k.text()).collect::<Vec<_>>()}));
        out.mouse.extend(keys);
        prev = Some(f);
    }
    // the empty points: the grid's first that no slot, object or actor answers, in the verbs' screen (else any), and
    // the first in the main screen
    let pts: Vec<(i32, i32)> = (0..SCREEN_H / GRID_STEP)
        .flat_map(|j| (0..SCREEN_W / GRID_STEP).map(move |i| (i * GRID_STEP + GRID_STEP / 2, j * GRID_STEP + GRID_STEP / 2)))
        .collect();
    let ats = e.at_many(&pts)?;
    let vzone = ats.iter().find(|a| a.verb_slot > 0).map(|a| a.zone);
    let empty = |a: &&At| a.verb_slot == 0 && a.main.is_none();
    let at = ats.iter().filter(empty).find(|a| Some(a.zone) == vzone).or_else(|| ats.iter().find(empty));
    let floor = ats.iter().find(|a| a.verb_slot == 0 && a.main.is_some_and(|m| m.2 == 0 && m.3 == 0));
    let mut clicks = Vec::new();
    if let Some(a) = at {
        clicks.push(("empty", Input::Click(a.x, a.y)));
    }
    if let Some(a) = floor {
        clicks.push(("floor", Input::Click(a.x, a.y)));
    }
    if let Some(v) = of.verbs.first() {
        clicks.push(("verb", Input::Click(v.x, v.y)));
    }
    for (what, i) in clicks {
        let Some((f, rec)) = probe(e, Some(&i))? else { continue };
        let mut keys = world_diff(&none, &f);
        if let Some(r) = &rec {
            keys.extend(written(r));
        }
        let nothing = made_nothing(rec.as_ref());
        // the record's writes by kind, a script started with its number
        let kinds: BTreeSet<String> = rec
            .iter()
            .flat_map(|r| r.writes.iter())
            .map(|t| if t.kind == TouchKind::Started { format!("started:{}", t.a) } else { t.kind.name().to_string() })
            .collect();
        out.probes.push(json!({"frame": frame, "input": what, "event": event_json(&Event { frame: frame + 1, what: i }),
                               "verdict": rec.as_ref().map(|r| r.verdict.text()), "writes": kinds, "counted": nothing,
                               "keys": keys.iter().map(|k| k.text()).collect::<Vec<_>>()}));
        if nothing {
            out.clicks.extend(keys);
        }
    }
    Ok(out)
}

/// The rooms of the log's room changes in frames [from, to], after `first`.
fn rooms_in(log: &[(u64, Key, Val)], first: i64, from: u64, to: u64) -> Vec<i64> {
    let mut v = vec![first];
    for &(f, k, x) in log {
        if k == ROOM && f >= from && f <= to && v.last() != Some(&x) {
            v.push(x);
        }
    }
    v
}

/// Item 1: the movie replayed from boot, its inputs met, its steps; with `o.twins`, a second replay for each step's
/// twin. `held`'s interface keys are set here from the replay's measures.
pub fn replay(host: &dyn EngineHost, movie: &[Event], held: &mut Held, o: &Opts) -> Res<Chain> {
    let mut ch = gather(host, movie, held, o)?;
    held.iface = ch.iface.clicks.union(&ch.iface.mouse).copied().collect();
    ch.steps = steps_of(&ch, held);
    for k in 0..ch.steps.len() {
        let s = &ch.steps[k];
        ch.steps[k].completion = match (&s.goal, &ch.end) {
            (Some(_), Some(st)) => Some((st.frame, st.tick)),
            _ => ch.completion(&s.keep, s.last, s.end.frame),
        };
    }
    if o.twins {
        twins(host, movie, &mut ch, held, o)?;
    }
    Ok(ch)
}

/// The replay's first pass: every input met, the facts at the stop after each, the progress keys' log, the end.
fn gather(host: &dyn EngineHost, movie: &[Event], held: &Held, o: &Opts) -> Res<Chain> {
    let inputs = inputs_of(movie);
    if inputs.is_empty() {
        return Err("the movie has no input".into());
    }
    let (mut e, mut sched) = boot(host, movie, o.seed, &o.out)?;
    let first = Stop::of(e.status());
    let mut iface = Iface::default();
    let facts = e.facts().map_err(e2s)?;
    let init = Facts(facts.0.iter().filter(|(k, _)| held.watched(*k)).copied().collect());
    let mut tr = Track { held, facts, log: Vec::new(), ticks: Vec::new(), since: 0, first_idle: None };
    put_tick(&mut tr.ticks, e.status());
    let mut recs: BTreeMap<u64, Vec<InputRecord>> = BTreeMap::new();
    let mut met: Vec<Met> = Vec::new();
    let mut drift = 0;
    let mut start = Facts::new();
    for (i, ev) in inputs.iter().enumerate() {
        let before = ev.frame.saturating_sub(1).max(e.status().frame);
        tr.run(&mut e, before).map_err(|x| format!("input {i} (frame {}): {x}", ev.frame))?;
        let st = e.status().clone();
        take_log(&mut e, &mut recs)?;
        let now = e.facts().map_err(e2s)?;
        drift += tr.facts.diff(&now).len();
        tr.facts = now;
        // the interface at the first control stop of each input handler (the first stop's when it is not one)
        let handler = tr.facts.val(Key::var(fmi_explore::afford::VAR_VERB_SCRIPT as u64));
        if (i == 0 && iface.at.is_empty()) || (st.idle.is_some() && !iface.at.iter().any(|a| a.1 == handler)) {
            iface.add(handler, first_control(&mut e).map_err(e2s)?);
        }
        if i == 0 {
            start = tr.facts.clone();
        } else {
            met[i - 1].next = Stop::of(&st);
            met[i - 1].facts = tr.facts.clone();
        }
        let seen = label::observe(&mut e, ev).map_err(e2s)?;
        sched.upto(&mut e, ev.frame)?;
        met.push(Met {
            ev: ev.clone(),
            seen,
            record: None,
            gap: None,
            first_idle: i > 0 && tr.first_idle == Some(st.frame),
            next: Stop::default(),
            facts: Facts::new(),
        });
        tr.since = ev.frame;
        tr.first_idle = None;
    }
    // after the last input: its record at its own frame's stop (the handler runs before the decision point), then on
    // to the game's end
    let last = inputs.last().unwrap().frame;
    let mut end: Option<Status> = None;
    sched.upto(&mut e, u64::MAX)?;
    let mut res = tr.run(&mut e, last);
    if res.is_ok() {
        take_log(&mut e, &mut recs)?;
        res = tr.run(&mut e, last + 1 + o.tail);
    }
    match res {
        Ok(()) => e.quit(),
        Err(EngineError::Ended(st)) => end = Some(*st),
        Err(x) => return Err(e2s(x)),
    }
    let n = met.len();
    met[n - 1].next = match &end {
        Some(st) => {
            let f = st.frame.saturating_sub(1);
            Stop { frame: f, tick: tr.ticks.get(f as usize).copied().unwrap_or(0), idle: None, room: tr.facts.val(ROOM) as i32, ego: None }
        }
        None => Stop { frame: last + 1 + o.tail, tick: tr.ticks.last().copied().unwrap_or(0), idle: None, room: tr.facts.val(ROOM) as i32, ego: None },
    };
    met[n - 1].facts = tr.facts.clone();
    let mut taken: Option<u64> = None;
    for (m, r) in met.iter_mut().zip(inputs.iter()) {
        m.record = recs.get(&r.frame).and_then(|v| v.first().cloned());
        m.gap = taken.map(|f| m.ev.frame - f);
        if m.record.as_ref().is_some_and(|r| r.verdict == Verdict::Taken) {
            taken = Some(m.ev.frame);
        }
    }
    Ok(Chain { met, steps: Vec::new(), first, end, log: tr.log, init, start, ticks: tr.ticks, iface, drift, twin_differs: 0 })
}

/// The interface's keys (`interface`) at the stop before the first input when it is a control stop, else at the
/// first control stop of a fork run on from it with no input (at most afford::W frames).
fn first_control(e: &mut Box<dyn Engine>) -> EResult<Iface> {
    if e.status().idle.is_some() {
        return interface(e);
    }
    let mut t = e.fork()?;
    let end = t.status().frame + fmi_explore::afford::W;
    let st = t.run(&RunSpec { idle: true, max_frame: Some(end), ..Default::default() })?;
    let out = if st.reason == "idle" { interface(&mut t) } else { Ok(Iface::default()) };
    t.quit();
    out
}

/// The steps of the inputs met (module docstring).
fn steps_of(ch: &Chain, held: &Held) -> Vec<Step> {
    let n = ch.met.len();
    let mut out: Vec<Step> = Vec::new();
    let mut from = Progress::of(&ch.start, held);
    let mut first = 0;
    let mut cum: BTreeSet<Key> = BTreeSet::new();
    let mut room = from.room;
    for (i, m) in ch.met.iter().enumerate() {
        let last = i + 1 == n;
        if !last && (m.next.idle.is_none() || label::timing_of(&ch.met[i + 1]) == label::Timing::Next) {
            continue;
        }
        let to = Progress::of(&m.facts, held);
        if !last && to == from {
            continue;
        }
        let changed = from.changed(&to);
        cum.extend(changed.iter().copied());
        let rooms = rooms_in(&ch.log, room, ch.met[first].ev.frame, m.next.frame);
        room = to.room;
        out.push(Step {
            k: out.len(),
            inputs: first..i + 1,
            from: std::mem::replace(&mut from, to.clone()),
            to,
            changed,
            keep: cum.iter().map(|&k| (k, m.facts.val(k))).collect(),
            start: ch.met[first].ev.frame,
            last: m.ev.frame,
            end: m.next.clone(),
            completion: None,
            rooms,
            twin: None,
            goal: if last { ch.end.as_ref().map(|s| s.reason.clone()) } else { None },
            ego: m.facts.val(EGO),
        });
        first = i + 1;
    }
    out
}

/// Item 3: the movie again from boot; at each step's start (the stop before its first input) its twins, each with its
/// RNG changed its own way (label::TWIN_FLIPS), run to the step's end and compared with this replay's facts there.
fn twins(host: &dyn EngineHost, movie: &[Event], ch: &mut Chain, held: &Held, o: &Opts) -> Res<()> {
    let (mut e, mut sched) = boot(host, movie, o.seed, &o.out.with_extension("twins"))?;
    sched.upto(&mut e, u64::MAX)?;
    for k in 0..ch.steps.len() {
        let (start, end) = (ch.steps[k].start, ch.steps[k].end.frame);
        let at = start.saturating_sub(1).max(e.status().frame);
        if e.status().frame < at {
            e.run(&RunSpec { frame: Some(at), ..Default::default() }).map_err(e2s)?;
        }
        let mut ts = Vec::new();
        for flip in label::TWIN_FLIPS {
            ts.push(label::twin(&mut e, flip).map_err(e2s)?);
        }
        match e.run(&RunSpec { frame: Some(end), ..Default::default() }) {
            Ok(_) => {
                let f = e.facts().map_err(e2s)?;
                // this replay (the movie scheduled at the boot) against the first's (scheduled as it went) at the end
                let i = ch.steps[k].inputs.end - 1;
                ch.twin_differs += ch.met[i].facts.diff(&f).len();
                let mut tw = Twin { frame: end, ..Default::default() };
                let ego = e.status().ego;
                for (j, t) in ts.into_iter().enumerate() {
                    tw.add(j, label::twin_diff(t, end, &f, ego, held));
                }
                ch.steps[k].twin = Some(tw);
            }
            Err(EngineError::Ended(_)) => {
                for t in ts {
                    t.quit();
                }
                // the last step: its end is the frame before the game's
                ch.steps[k].twin = Some(Twin { frame: end, ended: Some("the replay ended first".into()), ..Default::default() });
                return Ok(());
            }
            Err(x) => return Err(e2s(x)),
        }
    }
    e.quit();
    Ok(())
}

impl Chain {
    /// The first frame in [from, to] at whose stop every (key, value) of `keep` holds, by the progress keys' log, and
    /// its tick.
    pub fn completion(&self, keep: &[(Key, Val)], from: u64, to: u64) -> Option<(u64, u64)> {
        let mut now: BTreeMap<Key, Val> = keep.iter().map(|&(k, _)| (k, self.init.val(k))).collect();
        let holds = |now: &BTreeMap<Key, Val>| keep.iter().all(|(k, v)| now.get(k) == Some(v));
        let mut i = 0;
        while i < self.log.len() && self.log[i].0 <= from {
            if let Some(x) = now.get_mut(&self.log[i].1) {
                *x = self.log[i].2;
            }
            i += 1;
        }
        let tick = |f: u64| self.ticks.get(f as usize).copied().unwrap_or(0);
        if holds(&now) {
            return Some((from, tick(from)));
        }
        while i < self.log.len() && self.log[i].0 <= to {
            let f = self.log[i].0;
            while i < self.log.len() && self.log[i].0 == f {
                if let Some(x) = now.get_mut(&self.log[i].1) {
                    *x = self.log[i].2;
                }
                i += 1;
            }
            if holds(&now) {
                return Some((f, tick(f)));
            }
        }
        None
    }
}
