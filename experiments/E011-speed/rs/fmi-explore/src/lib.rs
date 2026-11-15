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
//! * `wait`: E012's wait move (plan.md section 1); `waitcheck`: its gates (`fmi check wait`).
//! * `bound`, `grow`: E014 amendment (4)'s bound on a try's fresh reading (FMI_READ_BOUND, FMI_READ_BOUND_BG) and
//!   its scopes grown as needed (FMI_SCOPE_GROW).
//! * `nogood`: E016's failure memory (FMI_NOGOOD): refutations at an exact entry, banned in later plans there, and
//!   the no-plan memory per reading.
//! * `stall`: E013's stall accounting (experiments/E013-robust/plan.md, section C): every second of a try in a row of
//!   the harness's own records and a class; stalls.json after each commit (FMI_STALLS); the free share's shift rules.

pub mod afford;
pub mod archive;
pub mod bound;
pub mod check;
pub mod ctx;
pub mod doubles;
pub mod driver;
pub mod enovel;
pub mod events;
pub mod explorecmd;
pub mod fake;
pub mod files;
pub mod focus;
pub mod free;
pub mod grow;
pub mod guide;
pub mod harness;
pub mod machine;
pub mod mech;
pub mod nogood;
pub mod ports;
pub mod progress;
pub mod pyjson;
pub mod stall;
pub mod start;
pub mod step;
pub mod tryjob;
pub mod wait;
pub mod waitcheck;
pub mod warm;
pub mod wiring;
