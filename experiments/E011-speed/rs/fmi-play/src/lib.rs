//! fmi-play (experiments/E011-speed/plan.md, "play"): the window search with play when ready, B1's strict matcher
//! and the harness's input code, ported from window.py's strict path, strict.py, the parts of trace_explain.py and
//! trace_check.py they use, executor.py's input code and replay.py's per-step functions. It provides
//! `fmi_core::Player` (`window::StrictPlayer`) and uses the engine port, the reading port and the plan result.
//!
//! The Python files are the frozen reference; `src/bin/play_fixture.rs` holds the differential tests against the
//! fixtures `experiments/E011-speed/play/capture.py` records from E009's step test (a). The test doubles of the ports
//! (`json_engine`, `file_reading`) are behind the cargo feature `json-double`, replaced at integration.

// the elif chains are the reference's, branch for branch; the types and argument lists its functions'
#![allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::if_same_then_else,
    clippy::unnecessary_unwrap,
    clippy::manual_range_contains,
    clippy::single_match,
    clippy::derivable_impls,
    clippy::manual_pattern_char_comparison,
    clippy::question_mark
)]

pub mod check;
pub mod code;
pub mod executor;
pub mod pyj;
pub mod reading;
pub mod recs;
pub mod replay;
pub mod strict;
pub mod val;
pub mod window;

#[cfg(feature = "json-double")]
pub mod file_reading;
#[cfg(feature = "json-double")]
pub mod json_engine;

pub use window::{Config, Search, StrictPlayer};
