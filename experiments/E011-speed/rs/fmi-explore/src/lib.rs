//! E011's explorer in Rust (experiments/E011-speed/plan.md, the `explore` component): explore.py's archive and driver,
//! step.py's try, guide.py, and replay.Harness's planning loop, on the ports of `fmi-core`.
//!
//! * `pyjson`, `files`: the reference's JSON and file writers, byte for byte.
//! * `archive`: explore.py `Archive`; `driver`: explore.py `run` and `Job`; `machine`: its thresholds and /proc.
//! * `fake`: fake_step.py, the try's stand-in in the driver's tests.

pub mod archive;
pub mod check;
pub mod driver;
pub mod fake;
pub mod files;
pub mod machine;
pub mod pyjson;
pub mod step;
