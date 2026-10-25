//! E003's reader in Rust, the half that does not depend on the Machine (decisions.md D7, layer 3): domains.py, and
//! reader.py's Scope, Record, Builder (inputs, events, records, passes), build_task, prune, task_start and
//! replan_start, with relevance.py, and a streaming writer into taskcore's packed format. The Python is the oracle;
//! tests/oracle.rs compares against it (PORT-NOTES.md).
//!
//! * [`domains::build`]: the finite domains of facts.
//! * [`scope::Scope::of`]: the rooms, units, global scripts, objects and verbs a scope reaches, and its write sets.
//! * [`builder::Builder`]: the inputs and events, the records made from a machine run's paths, the reading's passes.
//!   Machine runs come through [`builder::Machine`], the interface the machine-dependent half fills.
//! * [`task`]: build_task, relevance, prune, the starts. [`packed`]: taskcore's input, written streaming.
//! * [`replay::Replay`]: a [`builder::Machine`] that plays back the machine runs reader_port_oracle.py captured
//!   from the Python reading (the oracle for the Builder's half until the Rust machine lands).

pub mod builder;
pub mod domain;
pub mod domains;
pub mod facts;
pub mod machine;
pub mod oracle;
pub mod packed;
pub mod par;
pub mod py;
pub mod record;
pub mod replay;
pub mod scaling;
pub mod scope;
pub mod table;
pub mod task;

pub use builder::{Builder, Ctx, Job, Machine, RunOut};
pub use domain::{Domain, Label};
pub use facts::Facts;
pub use py::Py;
pub use record::{Eff, Event, PathOut, Record, Val, What};
pub use scope::Scope;
pub use table::FactTable;

/// Peak resident memory of this process so far, in kB (VmHWM).
pub fn peak_rss_kb() -> Option<u64> {
    scumm_ir::peak_rss_kb()
}

/// CPU seconds of this process so far, all its threads (utime + stime of /proc/self/stat, at 100 ticks a second).
pub fn cpu_secs() -> f64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let Some(i) = s.rfind(')') else { return 0.0 };
    let f: Vec<&str> = s[i + 2..].split_whitespace().collect();
    let tick = |j: usize| f.get(j).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);
    (tick(11) + tick(12)) / 100.0
}

/// Reads a gzip JSON-lines file (golden.py's and the oracle's), every gzip member, line by line.
pub fn gz_lines(path: &std::path::Path) -> impl Iterator<Item = String> {
    use std::io::BufRead;
    let f = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let r = std::io::BufReader::with_capacity(1 << 20, flate2::read::MultiGzDecoder::new(std::io::BufReader::new(f)));
    r.lines().map(|l| l.expect("a line"))
}
