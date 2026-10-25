//! E003's abstract SCUMM v5 machine over facts in Rust: the port of model.py's `Machine` (decisions.md
//! D7, layer 2), on top of `scumm-ir`. The Python is the reference: `machine-run` writes golden.py's
//! candidate files and `golden.py compare` must find no difference. PORT-NOTES.md says what is ported,
//! the Python behaviours kept on purpose, and how to run the comparison.
//!
//! * [`value`] holds ints and interned unknowns, value sets, `_decide`, `_arith`, `_join_values`.
//! * [`oracle`] holds `FactOracle`.
//! * [`path`] holds `Frame`, `Call`, `Step`, `Path`, `Run` and the join signature `Sig`.
//! * [`plist`] holds `PList`, the list a fork shares (a path's assumptions, notes, ...).
//! * [`machine`] holds `Machine` and `predict`; `machine/calls.rs` the calls and dispatch of row 1,
//!   `machine/rooms.rs` the room change (startScene) of row 2, `machine/settle.rs` the rounds of row 3,
//!   `machine/dialog.rs` the click on a verb of row 4.
//! * [`enc`] is golden.py's canonical encoding of all of these; [`trace`] is its turn-level trace.
//! * [`harness`] holds the ground truth's pre states, the jobs file, check.py's `run_model` dispatch.

pub mod enc;
pub mod harness;
pub mod heads;
pub mod machine;
pub mod oracle;
pub mod path;
pub mod plist;
pub mod trace;
pub mod value;

pub use machine::{predict, Features, Machine, Switches};
pub use oracle::FactOracle;
pub use path::{Path, Run};
pub use value::{Val, Vals};
