//! E011's explorer in Rust (experiments/E011-speed/plan.md, the `explore` component): explore.py's archive and driver,
//! step.py's try, guide.py, and replay.Harness's planning loop, on the ports of `fmi-core`.
//!
//! * `pyjson`, `files`: the reference's JSON and file writers, byte for byte.
//! * `archive`: explore.py `Archive`; `driver`: explore.py `run` and `Job`; `machine`: its thresholds and /proc.
//! * `fake`: fake_step.py, the try's stand-in in the driver's tests.
//! * `step`: step.py's parts; `tryjob`: step.py's run_job on the ports; `harness`: replay.Harness's planning loop with
//!   the window search; `start`: the harness's start rules; `wiring`: replay_part1's reach extender and refiner;
//!   `ports`: the ports beyond fmi-core's (IFACE); `guide`: guide.py; `check`: the differential checks.
//! * `wait`: E012's wait move (plan.md section 1); `waitcheck`: its gates (`fmi check wait`).

pub mod archive;
pub mod check;
pub mod doubles;
pub mod driver;
pub mod events;
pub mod explorecmd;
pub mod fake;
pub mod files;
pub mod guide;
pub mod harness;
pub mod machine;
pub mod ports;
pub mod pyjson;
pub mod start;
pub mod step;
pub mod tryjob;
pub mod wait;
pub mod waitcheck;
pub mod warm;
pub mod wiring;
