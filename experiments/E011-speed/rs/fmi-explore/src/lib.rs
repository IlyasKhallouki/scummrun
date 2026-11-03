//! E011's explorer in Rust (experiments/E011-speed/plan.md, the `explore` component): explore.py's archive and driver,
//! step.py's try, guide.py, and replay.Harness's planning loop, on the ports of `fmi-core`.
//!
//! * `pyjson`, `files`: the reference's JSON and file writers, byte for byte.
//! * `archive`: explore.py `Archive`; `driver`: explore.py `run` and `Job`; `machine`: its thresholds and /proc.
//! * `fake`: fake_step.py, the try's stand-in in the driver's tests.
//! * `step`: step.py's parts; `tryjob`: step.py's run_job on the ports; `harness`: replay.Harness's planning loop with
//!   the window search; `start`: the harness's start rules; `wiring`: replay_part1's reach extender and refiner;
//!   `ports`: the ports beyond fmi-core's (IFACE); `guide`: guide.py; `check`: the differential checks.
//! * `afford`: E012's affordance step (the interface's own inputs, tried in forks; FMI_AFFORD).
//! * `mech`: E012's try: the mechanisms' switches and ports, the try's novelty, the entries it commits besides its
//!   attempt's, the dead moves and the affordance step's triggers (tryjob.rs runs them in the plan's order).

pub mod afford;
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
pub mod mech;
pub mod ports;
pub mod progress;
pub mod pyjson;
pub mod start;
pub mod step;
pub mod tryjob;
pub mod warm;
pub mod wiring;
