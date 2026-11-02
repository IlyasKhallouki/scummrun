//! The ports of a run: the one place where `fmi` meets the other components' crates (plan.md, "Integration": the
//! integrator replaces each double with the real port, through thin adapters only). In round 1 the explore branch
//! builds alone, so the real ports are not here yet: `wire` says so, and `--doubles RECORDED_JOB_DIR` wires round 1's
//! doubles (a recorded try of the reference replayed through fmi-explore's code, fmi_explore::doubles).
//!
//! What the integrator wires here:
//! * `EngineHost` and `Returns`: fmi-engine's Host (lean, SPEEDRUN_WIRE=bin) and its kept-entry Pool;
//! * `Readings`: fmi-reading's ReadingSource over reader-scope --out-dir, its cache and its reuse keys;
//! * `PlannerHost`: fmi-plan's warm planner per reading (Search::Gbfs or FdLamaFirstCosts);
//! * `PlayerHost`: fmi-play's window search (strict, fork-each 7, ready per WindowCfg).

use fmi_core::{EngineHost, Search};
use fmi_explore::doubles::*;
use fmi_explore::files;
use fmi_explore::harness::PlannerCache;
use fmi_explore::tryjob::Resident;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct WireOpts {
    pub search: Search,
    pub doubles: Option<PathBuf>,
    pub forced_targets: Option<Vec<(String, i64)>>,
}

pub struct Wired {
    pub host: Option<Arc<dyn EngineHost>>,
    pub resident: Arc<Resident>,
}

pub fn search_of(s: Option<&str>) -> Result<Search, String> {
    match s.unwrap_or("fd") {
        "fd" | "lama-first-costs" => Ok(Search::FdLamaFirstCosts),
        "gbfs" => Ok(Search::Gbfs),
        x => Err(format!("--search {x}: fd (FdLamaFirstCosts) or gbfs")),
    }
}

pub fn search_name(s: Search) -> &'static str {
    match s {
        Search::FdLamaFirstCosts => "lama-first-costs",
        Search::Gbfs => "gbfs",
    }
}

const NOT_WIRED: &str = "the engine, reading, planner and play ports are wired at integration \
(experiments/E011-speed/rs/fmi/src/wire.rs); round 1 runs fmi try with --doubles RECORDED_JOB_DIR";

/// The run's ports.
pub fn wire(o: &WireOpts) -> Result<Wired, String> {
    let Some(rec) = &o.doubles else { return Err(NOT_WIRED.into()) };
    doubles(rec, o)
}

/// Round 1's doubles from a recorded job of the reference.
fn doubles(rec: &Path, o: &WireOpts) -> Result<Wired, String> {
    let job = files::read_json(&rec.join("job.json"))?;
    let want = files::read_json(&rec.join("result.json"))?;
    let mut runs = Vec::new();
    for k in 0..100 {
        let p = rec.join("run").join(format!("a{k:02}")).join("run.json");
        if !p.exists() {
            break;
        }
        runs.push(files::read_json(&p)?);
    }
    let played = runs.last().cloned().unwrap_or_default();
    let tl = Arc::new(timeline_of(rec, &job, &played)?);
    let mut readings: Vec<PathBuf> = want["reading"]["path"].as_str().map(PathBuf::from).into_iter().collect();
    for r in job["readings"].as_array().cloned().unwrap_or_default() {
        readings.push(PathBuf::from(r["path"].as_str().unwrap_or("")));
    }
    let run_dir = rec.join("run").join(format!("a{:02}", runs.len().saturating_sub(1)));
    if let Ok(rd) = std::fs::read_dir(&run_dir) {
        for d in rd.flatten() {
            if d.path().join("reading.json").exists() {
                readings.push(d.path().join("reading.json"));
            }
        }
    }
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let fresh = if files::truthy(want["reading"].get("fresh")) { want["reading"]["path"].as_str().map(PathBuf::from) } else { None };
    let rr = Arc::new(RecordedReadings::with_dumps(&home.join(".cache/fast-mi-e011/explore/fixtures/harness/domains"), &readings, fresh, run_dir));
    let plans = runs.iter().flat_map(|r| r["plans"].as_array().cloned().unwrap_or_default()).collect();
    Ok(Wired {
        host: None,
        resident: Arc::new(Resident {
            returns: Mutex::new(Box::new(ScriptReturns(tl.clone()))),
            readings: rr,
            planners: Mutex::new(PlannerCache::new(Arc::new(ScriptPlanners { plans, next: Arc::new(Mutex::new(0)) }))),
            players: Arc::new(ScriptPlayers { run: played, tl }),
            search: o.search,
            search_name: search_name(o.search).into(),
            forced_targets: o.forced_targets.clone(),
        }),
    })
}
