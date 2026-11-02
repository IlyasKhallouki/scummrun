//! fmi-engine: the engine's Rust driver (experiments/E011-speed/plan.md, component "engine").
//!
//! * `wire`: F1's encoding; `block`: F2, its facts (equal to `facts_of`) and the StepReply's delta.
//! * `json`: the C7 dump's JSON text as `Obs` (the differential tests against the Python reference).
//!
//! The engine side is patch 0002 (patches/0002-scumm-speedrun-lean.patch); engine/WIRE.md describes both ends.

pub mod block;
pub mod json;
pub mod sys;
pub mod wire;
