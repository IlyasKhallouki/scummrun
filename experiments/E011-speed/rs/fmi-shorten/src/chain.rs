//! E024 Part 2, item 1 (plan.md, "Steps"): the steps of a movie from boot.
//!
//! * **Progress** (`Progress`): at a control stop (the bridge's idle rule holds), the room, the objects ego owns and
//!   the archive's held atoms (`Held`: Archive::held's rule, archive.rs, over the stage's guide atoms, its idle keys I
//!   and its root's facts, read from the archive folder; no chain of entries is needed), less the interface's keys.
//! * **The interface's keys** (`interface`): the guide's atoms include the keys the interface itself sets on every
//!   input (the mouse's position and its copies, the sentence being composed, a dialog's lines), so with Archive::held
//!   as it is nearly every input changes the progress. They are measured, not named: the variables and bits the input
//!   log's records say the input handler wrote over the movie, and the world keys a mouse move alone changes (two forks
//!   at the first stop, the mouse moved to two corners of the screen, one frame).
//! * **The replay** (`replay`): the movie from boot, frame by frame. At the stop before each input label.rs reads what
//!   the input will hit; that stop is also the control stop after the input before it, where its facts are kept and
//!   the input log's records so far taken. Every frame's changes to the progress keys are logged, so a step's
//!   completion is the first frame at which its keep holds. A second replay forks each step's twin (item 3).
//! * **Steps** (`Step`): the inputs between two progress changes. An input whose next stop shows no change, or is not
//!   a control stop, merges into the next step; so does one followed by an input of timing class "next" (label.rs):
//!   that input is placed as soon as possible after it (a verb, then its object), and the stop between holds the
//!   interface's own state (the sentence being composed), not progress. The last step ends at the game's end.
//! * **keep(k)** (plan.md item 4): the values at step k's end of every key the steps up to k changed.

use crate::label::{self, Seen, Twin};
use crate::{e2s, Res};
use fmi_core::*;
use fmi_explore::archive::{self, Atom, FactLook, FactMap};
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
}

impl Stop {
    pub fn of(s: &Status) -> Stop {
        Stop { frame: s.frame, tick: s.tick, idle: s.idle.clone(), room: s.room }
    }
    pub fn json(&self) -> Value {
        json!({"frame": self.frame, "tick": self.tick, "idle": self.idle, "room": self.room})
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
    /// the interface's keys measured (`interface`): by the input log, by the mouse
    pub iface: (BTreeSet<Key>, BTreeSet<Key>),
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

/// The world keys a mouse move alone changes: two forks of `e` with the mouse moved to two corners of the screen,
/// one frame each, against each other.
pub fn mouse_keys(e: &mut Box<dyn Engine>) -> EResult<BTreeSet<Key>> {
    let f = e.status().frame + 1;
    let mut out = Vec::new();
    for (x, y) in [(0, 0), (fmi_explore::afford::SCREEN_W - 1, fmi_explore::afford::SCREEN_H - 1)] {
        let mut t = e.fork()?;
        t.inputs(&[Event { frame: f, what: Input::Move(x, y) }])?;
        t.run(&RunSpec { frame: Some(f), ..Default::default() })?;
        out.push(t.facts()?);
        t.quit();
    }
    Ok(out[0].diff(&out[1]).into_iter().map(|d| d.0).filter(|k| label::world(*k)).collect())
}

/// The interface's keys of the input log: the variables and bits its records say the input handler wrote.
pub fn record_keys(met: &[Met]) -> BTreeSet<Key> {
    met.iter()
        .filter_map(|m| m.record.as_ref())
        .flat_map(|r| r.writes.iter())
        .filter_map(|t| match t.kind {
            TouchKind::Var => Some(Key::var(t.a as u64)),
            TouchKind::Bit => Some(Key::bit(t.a as u64)),
            _ => None,
        })
        .collect()
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
    let rec = record_keys(&ch.met);
    held.iface = rec.union(&ch.iface.1).copied().collect();
    ch.iface.0 = rec;
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
    let mouse = mouse_keys(&mut e).map_err(e2s)?;
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
    // after the last input: its record, then on to the game's end
    let last = inputs.last().unwrap().frame;
    let mut end: Option<Status> = None;
    sched.upto(&mut e, u64::MAX)?;
    let mut res = tr.run(&mut e, last + 1);
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
            Stop { frame: f, tick: tr.ticks.get(f as usize).copied().unwrap_or(0), idle: None, room: tr.facts.val(ROOM) as i32 }
        }
        None => Stop { frame: last + 1 + o.tail, tick: tr.ticks.last().copied().unwrap_or(0), idle: None, room: tr.facts.val(ROOM) as i32 },
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
    Ok(Chain { met, steps: Vec::new(), first, end, log: tr.log, init, start, ticks: tr.ticks, iface: (BTreeSet::new(), mouse), drift, twin_differs: 0 })
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
        if !last && (m.next.idle.is_none() || label::timing(ch.met[i + 1].gap) == label::Timing::Next) {
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
        });
        first = i + 1;
    }
    out
}

/// Item 3: the movie again from boot; at each step's start (the stop before its first input) a twin with its RNG
/// changed, run to the step's end and compared with this replay's facts there.
fn twins(host: &dyn EngineHost, movie: &[Event], ch: &mut Chain, held: &Held, o: &Opts) -> Res<()> {
    let (mut e, mut sched) = boot(host, movie, o.seed, &o.out.with_extension("twins"))?;
    sched.upto(&mut e, u64::MAX)?;
    for k in 0..ch.steps.len() {
        let (start, end) = (ch.steps[k].start, ch.steps[k].end.frame);
        let at = start.saturating_sub(1).max(e.status().frame);
        if e.status().frame < at {
            e.run(&RunSpec { frame: Some(at), ..Default::default() }).map_err(e2s)?;
        }
        let t = label::twin(&mut e).map_err(e2s)?;
        match e.run(&RunSpec { frame: Some(end), ..Default::default() }) {
            Ok(_) => {
                let f = e.facts().map_err(e2s)?;
                // this replay (the movie scheduled at the boot) against the first's (scheduled as it went) at the end
                let i = ch.steps[k].inputs.end - 1;
                ch.twin_differs += ch.met[i].facts.diff(&f).len();
                ch.steps[k].twin = Some(label::twin_diff(t, end, &f, held));
            }
            Err(EngineError::Ended(_)) => {
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
