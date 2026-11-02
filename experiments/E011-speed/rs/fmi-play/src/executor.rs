//! The harness's input code as E009 runs it in a window's child: speedrun/executor.py's `Executor.play` for a click
//! or a key (`wait_gate`, `_click`, `_scroll_to`, `_approach`, `_floor_point`, `_stepping_stone` on a stone fork,
//! `_wait_walk_to`, `_wait_verb_visible`, `_wait_actor`, the hit tests), and replay.Harness's `_wait_camera`,
//! `_check_ego` and `_click_verb`, over a child's branch (`Br`: window._Cached, window._Stepper and
//! window._FirstWatched in one).

use crate::replay::{verb_point, InputStep, Move, EXECUTOR_EGO, INPUT_LIMIT, VAR_EGO};
use fmi_core::*;
use rustc_hash::FxHashMap;
use std::sync::Arc;

pub const SCREEN_W: i32 = 320;
pub const MAIN_H: i32 = 144;
pub const SCREEN_H: i32 = 200;
pub const DIALOG_FIRST: i32 = 120;
pub const DIALOG_LAST: i32 = 128;
pub const MAX_ACTOR: i64 = 12;
pub const STONE_FRAMES: u64 = 300;

/// executor.PROFILES["dos-cd"]
pub struct Profile {
    pub verb_var: usize,
    pub walk_to: i64,
    pub inv_first_verb: i32,
    pub inv_first_var: usize,
    pub inv_slots: usize,
    pub inv_up: i64,
    pub inv_down: i64,
    pub verbless_rooms: &'static [i32],
    pub hover_saved_verb_var: Option<usize>,
    pub inv_scroll_var: Option<usize>,
    pub inv_row: usize,
}

pub static DOS_CD: Profile = Profile {
    verb_var: 107,
    walk_to: 11,
    inv_first_verb: 200,
    inv_first_var: 133,
    inv_slots: 8,
    inv_up: 208,
    inv_down: 209,
    verbless_rooms: &[2, 3, 4, 5, 6, 85],
    hover_saved_verb_var: Some(394),
    inv_scroll_var: Some(118),
    inv_row: 4,
};

/// executor.RouteError, or an engine failure underneath.
#[derive(Debug)]
pub enum PlayErr {
    Route(String),
    Engine(EngineError),
}

impl From<EngineError> for PlayErr {
    fn from(e: EngineError) -> Self {
        PlayErr::Engine(e)
    }
}

pub type PResult<T> = Result<T, PlayErr>;

fn route<T>(s: String) -> PResult<T> {
    Err(PlayErr::Route(s))
}

/// What a child observes after each frame its input code runs (window._Stepper's `on_frame`).
pub trait FrameObs {
    /// `got`: the frame's runs and explain entries when the step already read them (StepObs), else None and the
    /// observer asks for them
    fn on_frame(&mut self, e: &mut dyn Engine, s: &Arc<Obs>, got: Option<(RunsReply, ExplainReply)>) -> EResult<()>;
}

/// `"end" in st`: the run ended where a stop was expected.
pub fn is_end(e: &EngineError) -> bool {
    matches!(e, EngineError::Ended(_))
}

/// A child's branch while its input code plays: the state dump and the hit tests kept until the engine runs again
/// (window._Cached; the first dump seeded with the parent's at the fork), runs played a frame at a time with an
/// observer after each (window._Stepper, when `obs` is set), and the frame and dump before the first input event
/// it schedules (window._FirstWatched).
pub struct Br<'a> {
    pub e: &'a mut Box<dyn Engine>,
    state: Option<Arc<Obs>>,
    at: FxHashMap<(i32, i32), At>,
    /// the hit tests' object answers across frames and branches (`HitCache`), and this frame's layout in it (None:
    /// not looked up yet; Some(None): a layout the cache does not cover)
    pub hc: Option<&'a HitCache>,
    layout: Option<Option<u32>>,
    pub obs: Option<&'a mut dyn FrameObs>,
    pub first_before: Option<(u64, Arc<Obs>)>,
    pub watch_first: bool,
    pub hits: [u64; 4], // state asked, state kept, at asked, at kept
    /// the AtMany commands sent (`prefetch`)
    pub batches: u64,
    /// the input code reads the state after almost every run: with batching each run brings the dump at its stop
    /// (RunObs), kept as this stop's state: the room's dump (`fmi_core::room_cut`: what the input code reads), or the
    /// full dump when an observer is set
    pub fuse: bool,
    /// `state` holds the room's dump, not the full one (`full` asks the full one)
    partial: bool,
    /// this thread's CPU ns in the engine calls: run, obs, at, and the stone trials' fork and quit
    pub cpu: [u64; 4],
}

/// This thread's CPU time, ns.
pub fn thread_ns() -> u64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}

impl<'a> Br<'a> {
    pub fn new(e: &'a mut Box<dyn Engine>, seed: Option<Arc<Obs>>) -> Br<'a> {
        Br {
            e,
            state: seed,
            at: FxHashMap::default(),
            hc: None,
            layout: None,
            obs: None,
            first_before: None,
            watch_first: false,
            hits: [0; 4],
            batches: 0,
            fuse: false,
            partial: false,
            cpu: [0; 4],
        }
    }
    pub fn frame(&self) -> u64 {
        self.e.status().frame
    }
    pub fn status(&self) -> &Status {
        self.e.status()
    }
    /// This stop's dump for the input code: the room's dump or the full one (the input code reads the room, the
    /// verbs, the variables, the inventory, the actors, the room's objects and their classes, which both hold alike).
    pub fn state(&mut self) -> EResult<Arc<Obs>> {
        if let Some(s) = &self.state {
            self.hits[1] += 1;
            return Ok(s.clone());
        }
        self.full()
    }
    /// This stop's full dump.
    pub fn full(&mut self) -> EResult<Arc<Obs>> {
        if let Some(s) = &self.state {
            if !self.partial {
                self.hits[1] += 1;
                return Ok(s.clone());
            }
        }
        let t = thread_ns();
        let s = Arc::new(self.e.obs(&ObsReq::default())?);
        self.cpu[1] += thread_ns() - t;
        self.hits[0] += 1;
        self.state = Some(s.clone());
        self.partial = false;
        Ok(s)
    }
    pub fn at(&mut self, x: i32, y: i32) -> EResult<At> {
        if let Some(a) = self.at.get(&(x, y)) {
            self.hits[3] += 1;
            return Ok(a.clone());
        }
        let t = thread_ns();
        let a = self.e.at(x, y)?;
        self.cpu[2] += thread_ns() - t;
        self.hits[2] += 1;
        self.at.insert((x, y), a.clone());
        Ok(a)
    }
    fn invalidate(&mut self) {
        self.state = None;
        self.partial = false;
        self.at.clear();
        self.layout = None;
    }
    /// This stop's layout in the hit cache (None: no cache, or a layout it does not cover).
    fn layout_id(&mut self) -> EResult<Option<u32>> {
        let Some(hc) = self.hc else {
            return Ok(None);
        };
        if self.layout.is_none() {
            let s = self.state()?;
            self.layout = Some(hc.layout_of(&s, self.e.status()));
        }
        Ok(self.layout.unwrap())
    }
    /// The hit tests at `pts` asked in one AtMany when batching is on (`batch`), so that the `at` and `at_obj` calls
    /// that follow at this stop read them from this frame's answers. With `obj_only` a point the hit cache answers
    /// is left out. The hit tests read the engine and change nothing, so asking a point early, or a point the calls
    /// then never read, changes no answer.
    pub fn prefetch(&mut self, pts: &[(i32, i32)], obj_only: bool) -> EResult<()> {
        if !batch() || pts.len() < 2 {
            return Ok(());
        }
        let lid = if obj_only { self.layout_id()? } else { None };
        let mut need: Vec<(i32, i32)> = Vec::with_capacity(pts.len());
        for &p in pts {
            if self.at.contains_key(&p) || need.contains(&p) {
                continue;
            }
            if let (Some(hc), Some(l)) = (self.hc, lid) {
                if hc.has(l, p.0, p.1) {
                    continue;
                }
            }
            need.push(p);
        }
        if need.len() < 2 {
            return Ok(());
        }
        let t = thread_ns();
        let r = self.e.at_many(&need)?;
        self.cpu[2] += thread_ns() - t;
        self.hits[2] += need.len() as u64;
        self.batches += 1;
        for (p, a) in need.into_iter().zip(r) {
            self.at.insert(p, a);
        }
        Ok(())
    }
    /// The object a click at screen (x, y) hits now (`at(x, y)`'s "obj", None off the main virtual screen), through
    /// the hit cache when one is set (its argument: `HitCache`).
    pub fn at_obj(&mut self, x: i32, y: i32) -> EResult<Option<i32>> {
        let Some(hc) = self.hc else {
            return Ok(self.at(x, y)?.main.map(|m| m.2));
        };
        let Some(lid) = self.layout_id()? else {
            return Ok(self.at(x, y)?.main.map(|m| m.2));
        };
        if let Some(v) = hc.get(lid, x, y) {
            self.hits[3] += 1;
            if check_hits() {
                let e = self.at(x, y)?.main.map(|m| m.2);
                assert_eq!(v, e, "FMI_CHECK_HITS: the cached hit at ({x}, {y}) is not the engine's");
            }
            return Ok(v);
        }
        let v = self.at(x, y)?.main.map(|m| m.2);
        hc.put(lid, x, y, v);
        Ok(v)
    }
    pub fn inputs(&mut self, events: &[Event]) -> EResult<()> {
        if self.watch_first && self.first_before.is_none() && !events.is_empty() && events.iter().any(is_input) {
            let s = self.full()?;
            self.first_before = Some((self.frame(), s));
        }
        self.e.inputs(events)
    }
    fn raw_run(&mut self, spec: &RunSpec) -> EResult<Status> {
        self.invalidate();
        let t = thread_ns();
        if self.fuse && batch() {
            let room = self.obs.is_none();
            let r = if room {
                self.e.run_room(spec)
            } else {
                self.e.run_obs(spec, &ObsReq::default())
            };
            self.cpu[0] += thread_ns() - t;
            let (st, o) = r?;
            self.hits[0] += 1;
            self.state = Some(Arc::new(o));
            self.partial = room;
            return Ok(st);
        }
        let r = self.e.run(spec);
        self.cpu[0] += thread_ns() - t;
        r
    }
    fn on(&mut self) -> EResult<()> {
        if self.obs.is_some() {
            let s = self.full()?;
            let o = self.obs.as_mut().unwrap();
            o.on_frame(&mut **self.e, &s, None)?;
        }
        Ok(())
    }
    /// One frame and its observation (window._Stepper's step with an observer): with batching one StepObs (the run,
    /// the runs, the explain entries and the full dump), else the run and then `on`.
    fn step_on(&mut self) -> EResult<Status> {
        let f = self.frame() + 1;
        if !batch() {
            let st = self.raw_run(&RunSpec {
                frame: Some(f),
                ..Default::default()
            })?;
            self.on()?;
            return Ok(st);
        }
        self.invalidate();
        let t = thread_ns();
        let r = self.e.step_observe(f, &ObsReq::default());
        self.cpu[0] += thread_ns() - t;
        let (st, rows, ents, o) = r?;
        let s = Arc::new(o);
        self.hits[0] += 1;
        self.state = Some(s.clone());
        let ob = self.obs.as_mut().unwrap();
        ob.on_frame(&mut **self.e, &s, Some((rows, ents)))?;
        Ok(st)
    }
    /// window._Stepper.run when an observer is set, else one command.
    pub fn run(&mut self, spec: &RunSpec) -> EResult<Status> {
        if self.obs.is_none() {
            return self.raw_run(spec);
        }
        let (frame, until, control, max_frame) = (spec.frame, &spec.until, spec.control, spec.max_frame);
        if spec.idle || !spec.until_any.is_empty() || spec.menu || (frame.is_none() && max_frame.is_none()) {
            let f0 = self.frame();
            let st = self.raw_run(spec)?;
            if self.frame() != f0 {
                self.on()?;
            }
            return Ok(st);
        }
        if let Some(fr) = frame {
            if until.is_empty() && !control {
                if fr <= self.frame() {
                    let f0 = self.frame();
                    let st = self.raw_run(&RunSpec {
                        frame: Some(fr),
                        ..Default::default()
                    })?;
                    if self.frame() != f0 {
                        self.on()?;
                    }
                    return Ok(st);
                }
                let mut st = self.status().clone();
                while self.frame() < fr {
                    st = self.step_on()?;
                }
                return Ok(st);
            }
        }
        let end = max_frame.or(frame).unwrap_or(0);
        loop {
            let st = self.raw_run(&RunSpec {
                until: until.clone(),
                control,
                max_frame: Some(self.frame() + 1),
                ..Default::default()
            })?;
            self.on()?;
            if st.reason == "until" || st.reason == "control" || self.frame() >= end {
                return Ok(st);
            }
        }
    }
    pub fn run_to(&mut self, frame: u64) -> EResult<Status> {
        self.run(&RunSpec {
            frame: Some(frame),
            ..Default::default()
        })
    }
    pub fn holds(&mut self, conds: &[Cond]) -> EResult<bool> {
        self.e.holds(conds)
    }
}

/// replay._is_input: a key, a click, a right click or a mouse move.
pub fn is_input(e: &Event) -> bool {
    !matches!(e.what, Input::Skips { .. })
}

/// One move's result (executor.MoveResult).
#[derive(Clone, Debug)]
pub struct MoveResult {
    pub issue: u64,
    pub done: u64,
    pub qf: u64,
}

/// executor.Executor with no fork (the harness's), its stone fork as the window search gives it.
pub struct Executor {
    pub movie: Vec<Event>,
    pub stone_fork: bool,
    verb_keys: FxHashMap<i64, String>,
    pub p: &'static Profile,
    /// the forks the stone trials made (a role's count, as Search.fork's)
    pub stone_trials: u32,
    pub stone_cpu: f64,
    /// the stone trials' wall, frames and hit tests asked
    pub stone_ns: u64,
    pub stone_frames: u64,
    pub stone_at: u64,
    /// the stone trials kept as the branch (`stone_keep`)
    pub stats_kept: u32,
}

impl Executor {
    pub fn new(stone_fork: bool) -> Executor {
        Executor {
            movie: Vec::new(),
            stone_fork,
            verb_keys: FxHashMap::default(),
            p: &DOS_CD,
            stone_trials: 0,
            stone_cpu: 0.0,
            stone_ns: 0,
            stone_frames: 0,
            stone_at: 0,
            stats_kept: 0,
        }
    }

    fn schedule(&mut self, b: &mut Br, events: Vec<Event>) -> PResult<()> {
        b.inputs(&events)?;
        self.movie.extend(events);
        Ok(())
    }

    fn run(&self, b: &mut Br, spec: &RunSpec) -> PResult<Status> {
        match b.run(spec) {
            Ok(st) => Ok(st),
            Err(EngineError::Ended(st)) => route(format!("the run ended: {}", st.reason)),
            Err(e) => Err(e.into()),
        }
    }

    fn run_to(&self, b: &mut Br, frame: u64) -> PResult<Status> {
        self.run(
            b,
            &RunSpec {
                frame: Some(frame),
                ..Default::default()
            },
        )
    }

    fn verb_key(&mut self, b: &mut Br, verb: i64) -> PResult<String> {
        if !self.verb_keys.contains_key(&verb) {
            let s = b.state()?;
            for v in &s.verbs {
                if v.key != 0 && (32..127).contains(&v.key) {
                    self.verb_keys
                        .entry(v.id as i64)
                        .or_insert_with(|| ((v.key as u8) as char).to_string());
                }
            }
        }
        match self.verb_keys.get(&verb) {
            Some(k) => Ok(k.clone()),
            None => route(format!("verb {verb} has no key in the verb table")),
        }
    }

    // --- geometry ---

    fn xstart(&self, b: &Br) -> i32 {
        let st = b.status();
        let xs = (st.cam.div_euclid(8) - SCREEN_W / 16) * 8;
        let rw = st.room_w;
        if rw != 0 {
            xs.min(rw - SCREEN_W).max(0)
        } else {
            xs
        }
    }

    fn main_h(&self, b: &Br) -> i32 {
        if self.p.verbless_rooms.contains(&b.status().room) {
            SCREEN_H
        } else {
            MAIN_H
        }
    }

    pub fn actor_points(&self, b: &mut Br, actor: i64, state: &Obs) -> PResult<Vec<(i32, i32)>> {
        let Some(a) = state.actors.iter().find(|x| x.id as i64 == actor) else {
            return Ok(vec![]);
        };
        let xs = self.xstart(b);
        let lag = xs - b.status().xstart;
        let mh = self.main_h(b);
        let pts: Vec<(i32, i32)> = [20, 35, 10, 50, 5, 1, 0]
            .into_iter()
            .map(|dy| (a.x - xs, a.y - a.elevation - dy))
            .filter(|p| 0 <= p.0 && p.0 < SCREEN_W && 0 <= p.1 && p.1 < mh)
            .collect();
        b.prefetch(&pts.iter().map(|p| (p.0 + lag, p.1)).collect::<Vec<_>>(), false)?;
        let mut hits = Vec::new();
        for p in pts {
            let at = b.at(p.0 + lag, p.1)?;
            if at.main.map(|m| m.3 as i64) == Some(actor) {
                hits.push(p);
            }
        }
        Ok(hits)
    }

    /// object_points' screen points for a room object's box (empty when the box is off screen), and the lag to add
    /// for the hit test.
    fn object_cands(&self, b: &Br, bx: &RoomObject) -> (Vec<(i32, i32)>, i32) {
        let xs = self.xstart(b);
        let lag = xs - b.status().xstart;
        let (x0, x1) = ((bx.x - xs).max(0), (bx.x + bx.w - xs).min(SCREEN_W) - 1);
        let (y0, y1) = (bx.y.max(0), (bx.y + bx.h).min(self.main_h(b)) - 1);
        if x0 > x1 || y0 > y1 {
            return (vec![], lag);
        }
        let fl = |a: i32, c: i32| (a + c).div_euclid(2);
        (
            vec![
                (fl(x0, x1), fl(y0, y1)),
                (x0, fl(y0, y1)),
                (x1, fl(y0, y1)),
                (fl(x0, x1), y0),
                (fl(x0, x1), y1),
            ],
            lag,
        )
    }

    pub fn object_points(&self, b: &mut Br, obj: i64, state: &Obs) -> PResult<Vec<(i32, i32)>> {
        let ros = state.room_objects.as_deref().unwrap_or(&[]);
        let Some(bx) = ros.iter().find(|o| o.id as i64 == obj) else {
            return if obj <= MAX_ACTOR {
                self.actor_points(b, obj, state)
            } else {
                Ok(vec![])
            };
        };
        let (cands, lag) = self.object_cands(b, bx);
        if cands.is_empty() {
            return Ok(vec![]);
        }
        b.prefetch(&cands.iter().map(|p| (p.0 + lag, p.1)).collect::<Vec<_>>(), true)?;
        let mut hits = Vec::new();
        for p in cands {
            if !hits.contains(&p) && b.at_obj(p.0 + lag, p.1)?.map(|o| o as i64) == Some(obj) {
                hits.push(p);
            }
        }
        Ok(hits)
    }

    fn inventory_point(&self, obj: i64, state: &Obs) -> Option<(i32, i32)> {
        for k in 0..self.p.inv_slots {
            if state.vars.get(self.p.inv_first_var + k).map(|&v| v as i64) == Some(obj) {
                let verb = state.verbs.iter().find(|v| v.id == self.p.inv_first_verb + k as i32);
                if let Some(v) = verb {
                    return Some((v.x + 4, v.y + 3));
                }
            }
        }
        None
    }

    fn in_inventory(state: &Obs, obj: i64) -> bool {
        state
            .inventory
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .any(|&o| o as i64 == obj)
    }

    fn target_point(&self, b: &mut Br, mv: &Move, obj: i64, state: &Obs) -> PResult<(i32, i32)> {
        if Self::in_inventory(state, obj) {
            return match self.inventory_point(obj, state) {
                Some(p) => Ok(p),
                None => route(format!("{}: inventory object {obj} is scrolled out of view", mv.name)),
            };
        }
        let pts = self.object_points(b, obj, state)?;
        match pts.first() {
            Some(p) => Ok(*p),
            None => route(format!("{}: object {obj} is not clickable on screen now", mv.name)),
        }
    }

    // --- moves ---

    /// executor.wait_gate
    pub fn wait_gate(&mut self, b: &mut Br, limit: u64) -> PResult<Status> {
        let end = b.frame() + limit;
        loop {
            let mut st = b.status().clone();
            if !(st.up > 0 && st.cs == 0 && st.room != 0) {
                st = self.run(
                    b,
                    &RunSpec {
                        control: true,
                        max_frame: Some(end),
                        ..Default::default()
                    },
                )?;
                if st.reason != "control" {
                    return route(format!(
                        "no control within {limit} frames (frame {}, room {})",
                        st.frame, st.room
                    ));
                }
            }
            if self.p.verbless_rooms.contains(&st.room) || !b.state()?.verbs.is_empty() {
                return Ok(b.status().clone());
            }
            if b.frame() >= end {
                return route(format!(
                    "no verbs on screen within {limit} frames (frame {})",
                    b.frame()
                ));
            }
            let f = b.frame() + 1;
            self.run_to(b, f)?;
        }
    }

    /// executor.play / _play for a harness move (a click with no until, or a key).
    pub fn play(&mut self, b: &mut Br, mv: &Move) -> PResult<MoveResult> {
        let issue;
        if let Some(k) = &mv.key {
            let f = b.frame() + 1;
            self.schedule(
                b,
                vec![Event {
                    frame: f,
                    what: Input::Key(k.clone()),
                }],
            )?;
            issue = f;
        } else {
            self.wait_gate(b, mv.max_frames)?;
            issue = self.click(b, mv)?;
        }
        let st = self.run_to(b, issue)?;
        Ok(MoveResult {
            issue,
            done: st.frame,
            qf: st.qf,
        })
    }

    fn scroll_to(&mut self, b: &mut Br, mv: &Move, obj: i64) -> PResult<()> {
        for _ in 0..16 {
            let state = b.state()?;
            if !Self::in_inventory(&state, obj) || self.inventory_point(obj, &state).is_some() {
                return Ok(());
            }
            let Some(sv) = self.p.inv_scroll_var else {
                return route(format!("{}: inventory object {obj} is scrolled out of view", mv.name));
            };
            let inv = state.inventory.as_deref().unwrap_or(&[]);
            let row = inv.iter().position(|&o| o as i64 == obj).unwrap_or(0) / self.p.inv_row;
            let arrow = if (row as i64) < state.vars.get(sv).copied().unwrap_or(0) as i64 {
                self.p.inv_up
            } else {
                self.p.inv_down
            };
            let Some(verb) = state.verbs.iter().find(|v| v.id as i64 == arrow) else {
                return route(format!("{}: inventory arrow {arrow} is not on screen", mv.name));
            };
            let f = b.frame() + 1;
            let ev = Event {
                frame: f,
                what: Input::Click(verb.x + 4, verb.y + 3),
            };
            self.schedule(b, vec![ev])?;
            self.run_to(b, f)?;
        }
        route(format!("{}: scrolling never showed inventory object {obj}", mv.name))
    }

    fn selected_verb(&self, mv: &Move, state: &Obs) -> i64 {
        let verb = state.vars.get(self.p.verb_var).copied().unwrap_or(0) as i64;
        let in_room = mv.point.is_some() || mv.obj.is_some_and(|o| !Self::in_inventory(state, o));
        if let Some(hv) = self.p.hover_saved_verb_var {
            let v = state.vars.get(hv).copied().unwrap_or(0);
            if in_room && v != 0 {
                return v as i64;
            }
        }
        verb
    }

    fn click(&mut self, b: &mut Br, mv: &Move) -> PResult<u64> {
        if mv.obj.is_none()
            && mv.point.is_none()
            && (mv.verb == Some(self.p.inv_up) || mv.verb == Some(self.p.inv_down))
        {
            let s = b.state()?;
            let Some(v) = s.verbs.iter().find(|v| Some(v.id as i64) == mv.verb) else {
                return route(format!(
                    "{}: inventory arrow {} is not on screen",
                    mv.name,
                    mv.verb.unwrap()
                ));
            };
            let f = b.frame() + 1;
            let ev = Event {
                frame: f,
                what: Input::Click(v.x + 4, v.y + 3),
            };
            self.schedule(b, vec![ev])?;
            return Ok(f);
        }
        let verb = mv.verb.unwrap_or(self.p.walk_to);
        let mut events: Vec<Event> = Vec::new();
        if let Some(o) = mv.obj {
            self.scroll_to(b, mv, o)?;
        }
        let mut state = b.state()?;
        if let Some(o2) = mv.obj2 {
            if !Self::in_inventory(&state, o2) && self.object_points(b, o2, &state)?.is_empty() {
                let mv2 = Move {
                    obj: Some(o2),
                    ..mv.clone()
                };
                state = self.approach(b, &mv2, &state)?;
            }
        }
        if verb == self.p.walk_to && self.selected_verb(mv, &state) != verb {
            state = self.wait_walk_to(b, mv, 120)?;
        }
        let mut f = b.frame() + 1;
        if self.selected_verb(mv, &state) != verb {
            state = self.wait_verb_visible(b, mv, verb)?;
            f = b.frame() + 1;
            let k = self.verb_key(b, verb)?;
            events.push(Event {
                frame: f,
                what: Input::Key(k),
            });
            f += 1;
        }
        let target = if let Some(pt) = mv.point {
            let x = pt.0 as i32 - self.xstart(b);
            let y = pt.1 as i32;
            if !(0 <= x && x < SCREEN_W && 0 <= y && y < self.main_h(b)) {
                return route(format!(
                    "{}: point ({}, {}) is off screen (xstart {})",
                    mv.name,
                    pt.0,
                    pt.1,
                    self.xstart(b)
                ));
            }
            (x, y)
        } else if let Some(o) = mv.obj {
            if !Self::in_inventory(&state, o) && self.object_points(b, o, &state)?.is_empty() {
                let ros = state.room_objects.as_deref().unwrap_or(&[]);
                if o <= MAX_ACTOR && ros.iter().all(|r| r.id as i64 != o) {
                    state = self.wait_actor(b, mv)?;
                } else {
                    state = self.approach(b, mv, &state)?;
                }
                f = b.frame() + 1;
                if !events.is_empty() {
                    events[0].frame = f;
                    f += 1;
                }
            }
            self.target_point(b, mv, o, &state)?
        } else {
            return route(format!("{}: a click move needs obj or point", mv.name));
        };
        events.push(Event {
            frame: f,
            what: Input::Click(target.0, target.1),
        });
        if events.len() == 2 {
            let first = events.remove(0);
            let kf = first.frame;
            self.schedule(b, vec![first])?;
            self.run_to(b, kf)?;
            let state = b.state()?;
            if state.vars.get(self.p.verb_var).copied().unwrap_or(0) as i64 != verb {
                return route(format!("{}: verb key did not select verb {verb}", mv.name));
            }
            if let Some(o) = mv.obj {
                let p = self.target_point(b, mv, o, &state)?;
                events[0].what = Input::Click(p.0, p.1);
            }
            self.schedule(b, events)?;
        } else {
            self.schedule(b, events)?;
        }
        if let Some(o2) = mv.obj2 {
            self.run_to(b, f)?;
            self.scroll_to(b, mv, o2)?;
            let g = b.frame() + 1;
            let s = b.state()?;
            let p = self.target_point(b, mv, o2, &s)?;
            self.schedule(
                b,
                vec![Event {
                    frame: g,
                    what: Input::Click(p.0, p.1),
                }],
            )?;
            return Ok(g);
        }
        Ok(f)
    }

    fn wait_walk_to(&mut self, b: &mut Br, mv: &Move, limit: u64) -> PResult<Arc<Obs>> {
        for _ in 0..limit {
            let f = b.frame() + 1;
            self.run_to(b, f)?;
            let state = b.state()?;
            if self.selected_verb(mv, &state) == self.p.walk_to {
                return Ok(state);
            }
        }
        route(format!(
            "{}: the verb did not return to Walk to within {limit} frames",
            mv.name
        ))
    }

    fn wait_verb_visible(&mut self, b: &mut Br, mv: &Move, verb: i64) -> PResult<Arc<Obs>> {
        for _ in 0..mv.max_frames {
            let state = b.state()?;
            if state.verbs.iter().any(|v| v.id as i64 == verb) {
                return Ok(state);
            }
            let f = b.frame() + 1;
            self.run_to(b, f)?;
        }
        route(format!("{}: verb {verb} never became visible", mv.name))
    }

    fn wait_actor(&mut self, b: &mut Br, mv: &Move) -> PResult<Arc<Obs>> {
        let o = mv.obj.unwrap_or(0);
        for _ in 0..mv.max_frames {
            let f = b.frame() + 1;
            self.run_to(b, f)?;
            let state = b.state()?;
            if !self.actor_points(b, o, &state)?.is_empty() {
                return Ok(state);
            }
        }
        route(format!("{}: actor {o} never came on screen", mv.name))
    }

    fn floor_point(&self, b: &mut Br, x: i32, y: i32, goal: Option<i64>) -> PResult<(i32, i32)> {
        let lag = self.xstart(b) - b.status().xstart;
        let step = if x > SCREEN_W / 2 { -1 } else { 1 };
        let mh = self.main_h(b);
        let mut pts = Vec::with_capacity(80);
        for dx in (0..64).step_by(4) {
            for dy in [0, -4, 4, -8, 8] {
                let (px, py) = (x + step * dx, y + dy);
                if 0 <= px && px < SCREEN_W && 0 <= py && py < mh {
                    pts.push((px, py));
                }
            }
        }
        // asked FLOOR_CHUNK points at a time (in order), so a point found early asks few more
        for chunk in pts.chunks(FLOOR_CHUNK) {
            b.prefetch(&chunk.iter().map(|p| (p.0 + lag, p.1)).collect::<Vec<_>>(), true)?;
            for &(px, py) in chunk {
                let hit = b.at_obj(px + lag, py)?.map(|o| o as i64).unwrap_or(0);
                if hit == 0 || Some(hit) == goal {
                    return Ok((px, py));
                }
            }
        }
        Ok((x, y))
    }

    fn ego_of(state: &Obs) -> Option<(i32, i32)> {
        state.actors.iter().find(|a| a.id == 1).map(|a| (a.x, a.y))
    }

    fn approach(&mut self, b: &mut Br, mv: &Move, state: &Obs) -> PResult<Arc<Obs>> {
        let obj = mv.obj.unwrap_or(0);
        let ros = state.room_objects.as_deref().unwrap_or(&[]);
        let Some(bx) = ros.iter().find(|o| o.id as i64 == obj).cloned() else {
            return route(format!("{}: object {obj} is not in room {}", mv.name, state.room));
        };
        let mut last: Option<(i32, i32)> = None;
        let mut stalls = 0;
        let mut seen: Option<(Option<i32>, Option<i32>, i32)> = None;
        for _ in 0..mv.max_frames {
            let xs = self.xstart(b);
            let (wx, wy) = bx.walk;
            let mh = self.main_h(b);
            let target = self.floor_point(b, (wx - xs).clamp(0, SCREEN_W - 1), wy.clamp(0, mh - 1), Some(obj))?;
            let virt = (target.0 + xs, target.1);
            let renew = Some(virt) != last;
            if renew || b.status().mv.unwrap_or(0) == 0 {
                let s = b.state()?;
                let ego = Self::ego_of(&s);
                let wh = (ego.map(|e| e.0), ego.map(|e| e.1), xs);
                stalls = if Some(wh) == seen && b.status().up > 0 {
                    stalls + 1
                } else {
                    0
                };
                seen = Some(wh);
                if stalls >= 3 {
                    return self.stepping_stone(b, mv, &bx);
                }
                let mut f = b.frame() + 1;
                let pm = Move {
                    name: mv.name.clone(),
                    point: Some((0, 0)),
                    obj: None,
                    obj2: None,
                    verb: None,
                    key: None,
                    max_frames: 3000,
                };
                if self.selected_verb(&pm, &s) != self.p.walk_to {
                    self.wait_walk_to(b, mv, 120)?;
                    f = b.frame() + 1;
                }
                self.schedule(
                    b,
                    vec![Event {
                        frame: f,
                        what: Input::Click(target.0, target.1),
                    }],
                )?;
                self.run_to(b, f)?;
                last = Some(virt);
            } else {
                let f = b.frame() + 1;
                self.run_to(b, f)?;
            }
            let state = b.state()?;
            if !self.object_points(b, obj, &state)?.is_empty() {
                return Ok(state);
            }
        }
        route(format!("{}: object {obj} never came on screen", mv.name))
    }

    fn stepping_stone(&mut self, b: &mut Br, mv: &Move, bx: &RoomObject) -> PResult<Arc<Obs>> {
        let state = b.state()?;
        let room = state.room;
        let obj = mv.obj.unwrap_or(0);
        let Some(ego) = state.actors.iter().find(|a| a.id == 1) else {
            return route(format!("{}: no actor 1 in the dump", mv.name)); // (Python: StopIteration)
        };
        let gx = bx.walk.0;
        let mut pre = Vec::new();
        for o in state.room_objects.as_deref().unwrap_or(&[]) {
            if o.id as i64 != obj && (o.walk.0 - gx).abs() < (ego.x - gx).abs() {
                let (c, lag) = self.object_cands(b, o);
                pre.extend(c.into_iter().map(|p| (p.0 + lag, p.1)));
            }
        }
        b.prefetch(&pre, true)?;
        let mut stones: Vec<RoomObject> = Vec::new();
        for o in state.room_objects.as_deref().unwrap_or(&[]) {
            if o.id as i64 != obj
                && (o.walk.0 - gx).abs() < (ego.x - gx).abs()
                && !self.object_points(b, o.id as i64, &state)?.is_empty()
            {
                stones.push(o.clone());
            }
        }
        stones.sort_by_key(|o| (o.walk.0 - gx).abs()); // (a stable sort, as Python's sorted)
        if self.stone_fork {
            // a trial that finds the object is kept as the branch (`stone_keep`) where no observer reads the frames
            let keep = stone_keep() && b.obs.is_none();
            for o in &stones {
                let t_trial = std::time::Instant::now();
                let c0 = thread_ns();
                let mut child = b.e.fork()?;
                b.cpu[3] += thread_ns() - c0;
                self.stone_trials += 1;
                let mut frames = None;
                let mut kept: Option<(Arc<Obs>, bool, Option<(u64, Arc<Obs>)>, Vec<Event>)> = None;
                {
                    let mut tb = Br::new(&mut child, None);
                    tb.hc = b.hc;
                    tb.fuse = b.fuse;
                    // the branch takes its dump before its first input event during the move it would play: the
                    // trial takes it where the branch would
                    tb.watch_first = keep && b.watch_first && b.first_before.is_none();
                    let mut trial = Executor::new(false);
                    let m = Move {
                        name: format!("{}: via {}", mv.name, o.id),
                        verb: Some(self.p.walk_to),
                        obj: Some(o.id as i64),
                        obj2: None,
                        point: None,
                        key: None,
                        max_frames: 3000,
                    };
                    let r: PResult<()> = (|| {
                        trial.play(&mut tb, &m)?;
                        let start = tb.frame();
                        while tb.frame() - start < STONE_FRAMES && tb.status().room == room {
                            let s = tb.state()?;
                            if !trial.object_points(&mut tb, obj, &s)?.is_empty() {
                                frames = Some(tb.frame() - start);
                                break;
                            }
                            let f = tb.frame() + 1;
                            trial.run_to(&mut tb, f)?;
                        }
                        Ok(())
                    })();
                    self.stone_frames += tb.frame().saturating_sub(b.frame());
                    self.stone_at += tb.hits[2];
                    for (a, x) in b.cpu.iter_mut().zip(tb.cpu) {
                        *a += x;
                    }
                    match r {
                        Ok(()) | Err(PlayErr::Route(_)) => {}
                        Err(PlayErr::Engine(e)) => {
                            self.stone_cpu += child.cpu_s();
                            child.quit();
                            return Err(PlayErr::Engine(e));
                        }
                    }
                    if keep && frames.is_some() {
                        let st = tb.state()?;
                        kept = Some((st, tb.partial, tb.first_before.take(), trial.movie.clone()));
                    }
                }
                if let Some((st, partial, first, movie)) = kept {
                    // The trial played this move and ran to the frame where the object shows: what the branch is
                    // about to do from the same state (the fork is exact), with the same commands (the same input
                    // code; its stone forks are not reached, as the trial needed none), except that the branch's last
                    // run is one command where the trial ran frame by frame, which ends in the same state (stops
                    // change nothing: play-wire split). So the trial's engine becomes the branch.
                    self.stats_kept += 1;
                    std::mem::swap(b.e, &mut child);
                    b.invalidate();
                    b.state = Some(st);
                    b.partial = partial;
                    if first.is_some() {
                        b.first_before = first;
                    }
                    self.movie.extend(movie);
                    self.stone_cpu += child.cpu_s();
                    let c0 = thread_ns();
                    child.quit();
                    b.cpu[3] += thread_ns() - c0;
                    self.stone_ns += t_trial.elapsed().as_nanos() as u64;
                    let state = b.state()?;
                    if !self.object_points(b, obj, &state)?.is_empty() {
                        return Ok(state);
                    }
                    continue;
                }
                self.stone_cpu += child.cpu_s();
                let c0 = thread_ns();
                child.quit();
                b.cpu[3] += thread_ns() - c0;
                self.stone_ns += t_trial.elapsed().as_nanos() as u64;
                if let Some(fr) = frames {
                    let m = Move {
                        name: format!("{}: via {}", mv.name, o.id),
                        verb: Some(self.p.walk_to),
                        obj: Some(o.id as i64),
                        obj2: None,
                        point: None,
                        key: None,
                        max_frames: 3000,
                    };
                    self.play(b, &m)?;
                    let f = b.frame() + fr;
                    self.run_to(b, f)?;
                    let state = b.state()?;
                    if !self.object_points(b, obj, &state)?.is_empty() {
                        return Ok(state);
                    }
                }
            }
        }
        route(format!(
            "{}: object {obj} never came on screen (no floor path, no stepping stone)",
            mv.name
        ))
    }
}

// --- replay.Harness's input code on a child (window._Ctx) ------------------------------------------------------------

/// The harness's input code for one input (window.Search._play: Harness._wait_camera and _check_ego for a click,
/// _click_verb for a dialog line, else the Executor's play).
pub fn play_input(ex: &mut Executor, b: &mut Br, inp: &InputStep, name: &str) -> PResult<()> {
    if inp.kind == "click" {
        wait_camera(ex, b, inp, INPUT_LIMIT)?;
        check_ego(ex, b, inp)?;
    }
    if inp.kind == "line" {
        return click_verb(ex, b, inp.verb.unwrap_or(0), name);
    }
    match crate::replay::to_move(inp, name) {
        Ok(Some(mv)) => {
            ex.play(b, &mv)?;
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(e) => route(e),
    }
}

fn advance(ex: &Executor, b: &mut Br) -> PResult<()> {
    let f = b.frame() + 1;
    match b.run_to(f) {
        Ok(_) => Ok(()),
        Err(EngineError::Ended(st)) => Err(PlayErr::Engine(EngineError::Ended(st))),
        Err(e) => Err(e.into()),
    }
    .map(|_| {
        let _ = ex;
    })
}

/// replay.Harness._wait_camera
fn wait_camera(ex: &mut Executor, b: &mut Br, inp: &InputStep, limit: u64) -> PResult<()> {
    let objs: Vec<i64> = [inp.a, inp.b].into_iter().flatten().collect();
    let end = b.frame() + limit;
    while b.frame() < end {
        let s = b.state()?;
        let mut all = true;
        for &o in &objs {
            if !(Executor::in_inventory(&s, o) || !ex.object_points(b, o, &s)?.is_empty()) {
                all = false;
                break;
            }
        }
        if all {
            break;
        }
        let cam = b.status().cam;
        advance(ex, b)?;
        if b.status().cam == cam {
            break;
        }
    }
    Ok(())
}

/// replay.Harness._check_ego
fn check_ego(ex: &mut Executor, b: &mut Br, inp: &InputStep) -> PResult<()> {
    let s = b.state()?;
    for o in [inp.a, inp.b].into_iter().flatten() {
        if Executor::in_inventory(&s, o) || !ex.object_points(b, o, &s)?.is_empty() {
            continue;
        }
        let ros = s.room_objects.as_deref().unwrap_or(&[]);
        if o <= MAX_ACTOR && ros.iter().all(|r| r.id as i64 != o) {
            continue;
        }
        let ego = s.vars.get(VAR_EGO).copied().unwrap_or(0);
        if ego != EXECUTOR_EGO {
            return route(format!(
                "interface: object {o} is off screen and the Executor's approach takes actor {EXECUTOR_EGO} as ego, but \
                 VAR_EGO is {ego}"
            ));
        }
    }
    Ok(())
}

/// replay.Harness._click_verb
fn click_verb(ex: &mut Executor, b: &mut Br, verb: i64, name: &str) -> PResult<()> {
    let end = b.frame() + INPUT_LIMIT;
    let p = loop {
        let p = if b.status().up > 0 {
            let s = b.state()?;
            b.prefetch(&crate::replay::verb_candidates(verb, &s), false)?;
            let mut at = |x: i32, y: i32| b.at(x, y);
            verb_point(verb, &s, &mut at)?
        } else {
            None
        };
        if let Some(p) = p {
            break p;
        }
        if b.frame() >= end {
            return route(format!(
                "{name}: verb {verb} cannot be clicked within {INPUT_LIMIT} frames (not listed in the verb table, not hit \
                 there, or user input off)"
            ));
        }
        advance(ex, b)?;
    };
    let ev = vec![Event {
        frame: b.frame() + 1,
        what: Input::Click(p.0, p.1),
    }];
    b.inputs(&ev)?;
    ex.movie.extend(ev);
    advance(ex, b)
}

/// The input code's hit tests are asked FLOOR_CHUNK at a time by `floor_point` (a batch of up to about 20 costs
/// about two single hit tests on the lean build: play-wire bench).
pub const FLOOR_CHUNK: usize = 10;

/// FMI_PLAY_BATCH=0: every engine query is its own command, as in round 1 (an observed frame as run, runs,
/// explain_log and obs; one `at` per hit test). On by default: `Engine::step_observe` and `Engine::at_many` give
/// what those commands give at the same stop.
pub fn batch() -> bool {
    static C: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *C.get_or_init(|| std::env::var("FMI_PLAY_BATCH").map_or(true, |v| v != "0"))
}

/// FMI_STONE_KEEP=0: a stone trial that finds the object is quit and its move played again on the branch, as in
/// round 1. On by default: the branch takes the trial's engine, which is where the move played again would end.
pub fn stone_keep() -> bool {
    static C: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *C.get_or_init(|| std::env::var("FMI_STONE_KEEP").map_or(true, |v| v != "0"))
}

/// FMI_CHECK_HITS=1: every hit the cache answers is asked of the engine too and compared.
pub fn check_hits() -> bool {
    static C: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *C.get_or_init(|| std::env::var("FMI_CHECK_HITS").is_ok_and(|v| v == "1"))
}

/// The hit test's object answer as a function of what it reads (experiments/E011-speed/play/HITCACHE.md).
///
/// The bridge's `at(x, y)` gives, for a point on the main virtual screen, `findObject(x + xstart, y - topline)`
/// (speedrun_control.cpp). findObject (object.cpp:557-598) walks the room's object table `_objs[1..n)` in order,
/// skips an entry whose obj_nr is 0 or whose object has the untouchable class (32), and for the others follows the
/// parent chain; for an entry whose parent is 0 it tests only the entry's box (x_pos, y_pos, width, height) and
/// returns its obj_nr when the point is inside. The state dump lists the table's entries with an obj_nr in table
/// order (id, x, y, w, h, parent) and every object's classes. So when every listed entry's parent is 0, the answer
/// at a point is the first listed entry, not untouchable, whose box holds the point: it depends only on the room,
/// the main screen's xstart, the screen layout (initScreens' b and h and the forced banner screen, which give the
/// virtual screen of y and the main screen's topline) and the listed (id, x, y, w, h, untouchable). The cache keys a
/// layout by exactly these, compared whole, and only for such tables, at a stop whose status reports the screen
/// layout (`Status::screen`, patch 0004); a table with a parent, or a stop without the screen layout, is asked of
/// the engine every time. On unless FMI_HIT_CACHE=0 (window::Search::new).
#[derive(Default)]
pub struct HitCache {
    layouts: std::sync::Mutex<FxHashMap<Layout, u32>>,
    hits: std::sync::Mutex<FxHashMap<(u32, i32, i32), Option<i32>>>,
    pub asked: std::sync::atomic::AtomicU64,
    pub kept: std::sync::atomic::AtomicU64,
    pub uncovered: std::sync::atomic::AtomicU64,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Layout {
    room: i32,
    xstart: i32,
    screen: (i32, i32, bool),
    objs: Vec<(i32, i32, i32, i32, i32, bool)>,
}

impl HitCache {
    fn layout_of(&self, s: &Obs, st: &Status) -> Option<u32> {
        let ros = s.room_objects.as_deref()?;
        let screen = st.screen?;
        if ros.iter().any(|o| o.parent != 0) {
            self.uncovered.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return None;
        }
        let objs = ros
            .iter()
            .map(|o| {
                let untouchable = s.classes.iter().any(|c| c.0 as i32 == o.id && c.1.contains(&32));
                (o.id, o.x, o.y, o.w, o.h, untouchable)
            })
            .collect();
        let l = Layout {
            room: s.room,
            xstart: st.xstart,
            screen,
            objs,
        };
        let mut m = self.layouts.lock().unwrap();
        let n = m.len() as u32;
        Some(*m.entry(l).or_insert(n))
    }
    fn get(&self, lid: u32, x: i32, y: i32) -> Option<Option<i32>> {
        let v = self.hits.lock().unwrap().get(&(lid, x, y)).copied();
        if v.is_some() {
            self.kept.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        v
    }
    /// whether the cache holds the answer (not counted)
    fn has(&self, lid: u32, x: i32, y: i32) -> bool {
        self.hits.lock().unwrap().contains_key(&(lid, x, y))
    }
    fn put(&self, lid: u32, x: i32, y: i32, v: Option<i32>) {
        self.asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.hits.lock().unwrap().insert((lid, x, y), v);
    }
    pub fn line(&self) -> String {
        use std::sync::atomic::Ordering::Relaxed;
        format!(
            "hit cache: {} layouts, {} asked, {} kept, {} frames with a parent (uncovered)",
            self.layouts.lock().unwrap().len(),
            self.asked.load(Relaxed),
            self.kept.load(Relaxed),
            self.uncovered.load(Relaxed)
        )
    }
}
