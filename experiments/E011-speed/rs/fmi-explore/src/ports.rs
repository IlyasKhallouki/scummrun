//! What the explorer needs from the other components beyond fmi-core's ports, as traits of its own (plan.md,
//! "Coordination": an interface change is an IFACE entry in COORD.md; meanwhile the builder adds it in its crate).
//! The integrator wires each to its provider through a thin adapter:
//!
//! * `Readings` (readio): open a reading's files as a `ReadingSource`, E009's reuse keys K, and make a reading
//!   (reader-scope writing its files, replay_part1.read_rust's inputs).
//! * `Returns` (engine): an engine at an entry's state, a fork of a kept one when the pool has it.
//! * `WindowPlayer` (play): `Player` plus what replay.Harness reads of window.Search besides play_plan: whether the goal
//!   came from the plan, the replan's cause and its detail, the plan's notes, the movie, the summary and the records.
//! * `PlayerHost` (play): a window search per attempt, with step.py's WindowConfig.
//!
//! fmi-core's `PlannerHost` and `Planner` are used as they are.

use fmi_core::{BootCfg, EResult, Engine, EngineHost, Event, PlanCtx, PlayOutcome, PlayState, Player, ReadingSource};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// replay_part1.read_rust's inputs: one reading of the Rust reader.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReadReq {
    pub rooms: Vec<i64>,
    pub seed: i64,
    pub goal_text: String,
    pub name: String,
    pub out: PathBuf,
    pub threads: u32,
    /// F10's slots read from their dumped locals (off in E009's settings)
    pub slot_locals: Vec<Value>,
    pub domain_goals: String,
    pub elsewhere: bool,
    pub live_locals: bool,
    pub split_limit: bool,
    pub switches: String,
    pub reach: bool,
    pub frame_costs: bool,
    pub reach_also: Vec<PathBuf>,
    /// the start's row from this file in place of results/start-part1.jsonl
    pub start_rows: Option<PathBuf>,
    pub fact_vars: Vec<i64>,
    pub stream: bool,
}

/// The readings (provider: readio).
pub trait Readings: Send + Sync {
    /// A reading's files, opened (packed_reading.load_reading).
    fn open(&self, reading_json: &Path) -> Result<Arc<dyn ReadingSource>, String>;
    /// E009's K of a reading (step.reuse_keys: its non-facts whose start value no site of the reader reads, less its
    /// fact variables), cached next to its reading.json.
    fn reuse_keys(&self, rd: &dyn ReadingSource) -> Result<HashSet<String>, String>;
    /// A reading made by the reader; returns its reading.json. `cancel`: the guard's kill.
    fn read(&self, req: &ReadReq, cancel: &AtomicBool) -> Result<PathBuf, String>;
}

/// An entry the try returns to.
#[derive(Clone, Debug)]
pub struct EntryRef {
    pub id: i64,
    pub root: bool,
    pub frame: u64,
    /// its movie from boot (absolute frames)
    pub events: Vec<Event>,
}

/// Returns to entries (provider: engine, the kept-entry pool).
pub trait Returns: Send {
    /// An engine at the entry's state (step.py step 1): a fork of a kept engine, else a boot replaying its movie.
    fn return_to(&mut self, entry: &EntryRef, out_dir: &Path) -> EResult<Box<dyn Engine>>;
    /// Keep an engine stopped at an entry's state for later returns (the pool may evict it).
    fn keep(&mut self, _id: i64, engine: Box<dyn Engine>) {
        engine.quit();
    }
}

/// Returns by booting a fresh engine every time (replay.boot / replay_part1.boot): the reference's way, and the
/// fallback where no pool is wired.
pub struct BootReturns {
    pub host: Arc<dyn EngineHost>,
    pub lean: bool,
}

impl Returns for BootReturns {
    fn return_to(&mut self, entry: &EntryRef, out_dir: &Path) -> EResult<Box<dyn Engine>> {
        let cfg = BootCfg {
            seed: 1,
            events: entry.events.clone(),
            frame: if entry.root { None } else { Some(entry.frame) },
            dump_locals: true,
            lean: self.lean,
            out_dir: out_dir.to_path_buf(),
        };
        self.host.boot(&cfg)
    }
}

/// window.Search as replay.Harness reads it (provider: play). `Player` is fmi-core's.
pub trait WindowPlayer: Player {
    /// the goal came from the plan's records (window.Search.goal_credited)
    fn goal_credited(&self) -> bool;
    /// the last play_plan's replan cause text (window.Search.replan_cause)
    fn replan_cause(&self) -> Option<String>;
    /// its detail, {"what", "cause"} (window.Search.replan_cause_detail), the cause as B1's dict
    fn replan_cause_detail(&self) -> Option<Value>;
    /// strict.short of a cause
    fn short(&self, cause: &Value) -> String;
    /// what play_plan wrote in the plan's entry (h.plans[-1]: "macro_steps", "goal_writers"), merged into it
    fn plan_notes(&mut self) -> Option<Value>;
    /// every input the search sent, absolute frames (h.ex.movie)
    fn movie(&self) -> Vec<Event>;
    /// window.Search.flush: the pending success checkpoint
    fn flush(&mut self, ps: &mut PlayState);
    /// window.Search.summary (with note_cpu of the last engine)
    fn summary(&mut self, ps: &PlayState) -> Value;
    /// window.Search.windows: the window records
    fn windows(&self) -> Vec<Value>;
}

/// step.py's WindowConfig for the strict search (walk's run.sh switches): workers, fork-each 7, strict, cache, lite,
/// split-wait; `ready`: E010's play when ready; `delta_lo` from the reading's frame costs.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowCfg {
    pub workers: u32,
    pub fork_each: u32,
    pub ready: bool,
    pub frame_costs: Value,
}

/// E012's settings of an attempt's window search for the delay fork (component delay, FMI_DELAY_FORK; plan.md 2),
/// from the try: T's keys (the outcome digest's keys) and the try's novelty of an end state (None: unknown). Default:
/// none, the window search's own.
#[derive(Clone, Default)]
pub struct PlayExtras {
    pub tested_keys: Option<Arc<Vec<fmi_core::Key>>>,
    pub novel_of: Option<Arc<dyn Fn(&fmi_core::Facts) -> Option<bool> + Send + Sync>>,
}

/// Makes the window search of an attempt (provider: play).
pub trait PlayerHost: Send + Sync {
    fn player(&self, cfg: &WindowCfg) -> Box<dyn WindowPlayer>;
    /// The attempt's window search with E012's settings (`PlayExtras`). Default: `player`, the settings unused.
    fn player_with(&self, cfg: &WindowCfg, x: &PlayExtras) -> Box<dyn WindowPlayer> {
        let _ = x;
        self.player(cfg)
    }
    /// The attempt's player when its attempt has ended (E011 round 2, FMI_PLAYER_DONE). Default: freed on a thread of
    /// its own, off the try's path (the window search's reading data and game: 20-90 ms of frees). A provider may keep
    /// what the next player can use.
    fn done(&self, p: Box<dyn WindowPlayer>) {
        std::thread::spawn(move || drop(p));
    }
}

/// Adapts fmi-core's PlayCtx calls (kept here so the harness reads one trait).
pub fn play(p: &mut dyn WindowPlayer, ctx: &PlanCtx<'_>, ps: &mut PlayState, n_plan: u32) -> PlayOutcome {
    p.play_plan(ctx, ps, n_plan)
}
