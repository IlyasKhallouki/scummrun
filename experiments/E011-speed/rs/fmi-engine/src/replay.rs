//! replay.py's engine functions, ported: `boot` (EngineHost::boot), `extra_of`, `compare_checkpoint` and `verify`.

use crate::host::{is_input, Host};
use fmi_core::*;

/// replay.extra_of with the facts: the checkpoint a harness keeps at a stop.
pub fn checkpoint_of(obs: &Obs, status: &Status) -> Checkpoint {
    let mut actors: Vec<(i32, i32, i32)> = obs.actors.iter().map(|a| (a.id, a.x, a.y)).collect();
    actors.sort();
    Checkpoint {
        frame: status.frame,
        facts: facts_of(obs),
        tick: status.tick,
        rng: status.rng,
        actors,
    }
}

/// replay.compare_checkpoint's result: the facts that differ (their count and the first 8 in the keys' text
/// order) and the extra fields that differ (tick, rng, actors: want, got).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CpDiff {
    pub facts: usize,
    pub first: Vec<(String, Val, Val)>,
    pub extra: Vec<(String, String, String)>,
}

impl CpDiff {
    pub fn is_empty(&self) -> bool {
        self.facts == 0 && self.extra.is_empty()
    }
}

/// replay.compare_checkpoint: the differences between a checkpoint and the engine's state and status.
pub fn compare_checkpoint(want: &Checkpoint, obs: &Obs, status: &Status) -> CpDiff {
    let got = checkpoint_of(obs, status);
    let d = want.facts.diff(&got.facts);
    let mut first: Vec<(String, Val, Val)> = d.iter().map(|&(k, a, b)| (k.text(), a, b)).collect();
    first.sort();
    first.truncate(8);
    let mut extra = Vec::new();
    if want.tick != got.tick {
        extra.push(("tick".into(), want.tick.to_string(), got.tick.to_string()));
    }
    if want.rng != got.rng {
        extra.push(("rng".into(), want.rng.to_string(), got.rng.to_string()));
    }
    if want.actors != got.actors {
        extra.push((
            "actors".into(),
            format!("{:?}", want.actors),
            format!("{:?}", got.actors),
        ));
    }
    CpDiff {
        facts: d.len(),
        first,
        extra,
    }
}

/// One checkpoint that differs (or the run that stopped elsewhere).
#[derive(Clone, Debug, PartialEq)]
pub struct Differ {
    pub frame: u64,
    pub error: Option<String>,
    pub diff: CpDiff,
}

/// replay.verify's result.
#[derive(Clone, Debug, Default)]
pub struct Verified {
    pub checkpoints: usize,
    pub differ: Vec<Differ>,
    pub inputs: usize,
    /// the goal keys' values at the last checkpoint
    pub final_: Vec<(Key, Val)>,
    pub same: bool,
    pub seconds: f64,
}

/// replay.verify: `events` played from boot in a fresh engine (no fork, no change to the random state), compared at
/// every checkpoint (in frame order) on facts, tick, random state and actor positions. `base` gives the seed, the
/// out dir and the switches; its events and frame are not used.
pub fn verify(
    host: &Host,
    base: &BootCfg,
    events: &[Event],
    checkpoints: &[Checkpoint],
    goal_keys: &[Key],
) -> EResult<Verified> {
    let t = std::time::Instant::now();
    let cfg = BootCfg {
        events: events.to_vec(),
        frame: None,
        ..base.clone()
    };
    let mut out = Verified {
        checkpoints: checkpoints.len(),
        inputs: events.iter().filter(|e| is_input(e)).count(),
        ..Default::default()
    };
    let mut e = host.spawn(&cfg)?;
    let full = ObsReq::default();
    for cp in checkpoints {
        let st = match e.run(&RunSpec {
            frame: Some(cp.frame),
            ..Default::default()
        }) {
            Ok(st) => st,
            Err(err) => {
                out.differ.push(Differ {
                    frame: cp.frame,
                    error: Some(err.to_string()),
                    diff: CpDiff::default(),
                });
                break;
            }
        };
        if st.frame != cp.frame {
            out.differ.push(Differ {
                frame: cp.frame,
                error: Some(format!("stopped at {}", st.frame)),
                diff: CpDiff::default(),
            });
            break;
        }
        let obs = e.obs(&full)?;
        let d = compare_checkpoint(cp, &obs, &st);
        if !d.is_empty() {
            out.differ.push(Differ {
                frame: cp.frame,
                error: None,
                diff: d,
            });
        }
    }
    if !checkpoints.is_empty() {
        // the goal keys where the engine stopped, as replay.verify reads them
        if let Ok(obs) = e.obs(&full) {
            let f = facts_of(&obs);
            out.final_ = goal_keys.iter().map(|&k| (k, f.val(k))).collect();
        }
    }
    out.same = out.differ.is_empty();
    out.seconds = t.elapsed().as_secs_f64();
    Ok(out)
}
