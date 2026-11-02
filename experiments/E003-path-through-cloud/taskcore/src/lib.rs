//! taskcore as a library (E011, experiments/E011-speed/plan.md, component `plan`): the planner's task as flat
//! integer arrays, the fork fold, the exact reductions and Fast Downward's SAS file (README.md), used by the command
//! line (main.rs, `cli`) and kept warm in one process by E011's planner (`fmi-plan`).
//!
//! The command line's outputs (`sas`, `decode`, `validate`, `serve`) are the same bytes as before the library: its
//! modules are the binary's, moved, and `cli` is the old main.rs.

pub mod bans;
pub mod cache;
pub mod cli;
pub mod decode;
pub mod fold;
pub mod model;
pub mod packed;
pub mod predict;
pub mod reduce;
pub mod sas;
pub mod search;
pub mod serve;
pub mod tail;
pub mod warm;
// E011 route: the libstdc++ container order Fast Downward's landmark code depends on
pub mod stdhash;
