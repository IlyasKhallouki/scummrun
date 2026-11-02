//! fmi-engine: the engine's Rust driver (experiments/E011-speed/plan.md, component "engine").
//!
//! * `host`: `Host` starts engine processes as replay_part1.config does (EngineHost) and `BinEngine` drives one over
//!   the binary channel F1 (Engine), with `step` (one round trip per observed frame) and the F2 block.
//! * `wire`: F1's encoding; `block`: F2, its facts (equal to `facts_of`) and the StepReply's delta.
//! * `pool`: the kept-entry pool (a return is a fork of a kept process).
//! * `replay`: replay.py's `compare_checkpoint` and `verify`.
//! * `json`: the C7 dump's JSON text as `Obs` (the differential tests against the Python reference).
//!
//! The engine side is patch 0002 (patches/0002-scumm-speedrun-lean.patch); engine/WIRE.md describes both ends.

pub mod block;
pub mod host;
pub mod json;
pub mod pool;
pub mod replay;
pub mod sys;
pub mod wire;

pub use host::{BinEngine, Host, HostCfg, StepOut};
pub use pool::{EntrySpec, Pool};
