//! A [`Machine`] that plays back the machine runs reader_port_oracle.py captured from the Python reading
//! (`capture-NAME.jsonl.gz`, in the order the parent process handled the jobs). Each run checks that the Builder
//! asked for the run Python made (label, pins, precondition, what): a split, a pin or an event the port gets wrong
//! stops the replay there.
//!
//! [`Compare`] runs the Rust machine half ([`crate::machine::Reader`]) on each run as well and compares what it
//! gives with the captured run, field by field; it hands the Builder the captured run, so one reading finds every
//! run that differs.

use crate::builder::{Ctx, Machine, RdCounts, RunOut};
use crate::machine::Reader;
use crate::py::{Counter, Py};
use crate::record::{pins_py, pre_py, PathOut, Pins, Pre, What};

pub struct Replay {
    lines: Box<dyn Iterator<Item = String>>,
    pub runs: usize,
}

impl Replay {
    pub fn open(path: &std::path::Path) -> Replay {
        Replay { lines: Box::new(crate::gz_lines(path)), runs: 0 }
    }

    /// Whether every captured run was asked for.
    pub fn done(&mut self) -> bool {
        self.lines.next().is_none()
    }

    /// The next captured run, checked against what the Builder asks for; also its place for messages.
    fn next(&mut self, label: &str, pins: &Pins, pre: &Pre, what: &What) -> (RunOut, String) {
        let line = self.lines.next().unwrap_or_else(|| panic!("run {}: no captured run left for {label}", self.runs));
        let p = Py::from_text(&line);
        let at = format!("run {} (pass {}, job {} {})", self.runs, p.get("pass").int(), p.get("job").int(), p.get("label").as_str());
        assert_eq!(p.get("label").as_str(), label, "{at}: the Builder asked for another job");
        let check = |field: &str, mine: Py| {
            let theirs = p.get(field).text();
            let mine = mine.text();
            assert!(theirs == mine, "{at}: {field} differs\n  python: {}\n  rust:   {}", short(&theirs), short(&mine));
        };
        check("pins", pins_py(pins));
        check("pre", pre_py(pre));
        check("what", what_py(what));
        self.runs += 1;
        let rd = p.get("rd");
        let out = RunOut {
            paths: p.get("paths").items().iter().map(PathOut::from_py).collect(),
            limit: p.get("limit").int(),
            value_caps: p.get("value_caps").int(),
            first_fork: (!p.get("first_fork").is_none()).then(|| p.get("first_fork").as_str().to_string()),
            rd: RdCounts {
                counts: Counter::from_py(rd.get("counts")),
                forks: Counter::from_py(rd.get("forks")),
                fork_keys: Counter::from_py(rd.get("fork_keys")),
            },
        };
        (out, at)
    }
}

fn what_py(w: &What) -> Py {
    match w {
        What::Sentence { verb, a, b } => Py::Tuple(vec![Py::str("run_sentence_sym"), Py::Int(*verb), Py::Int(*a), Py::opt_int(*b)]),
        What::Verb(v) => Py::Tuple(vec![Py::str("run_click_verb"), Py::Int(*v)]),
        What::Event(e) => Py::Tuple(vec![Py::str("run_event"), e.py()]),
    }
}

impl Machine for Replay {
    fn run(&mut self, _cx: &Ctx, label: &str, pins: &Pins, pre: &Pre, what: &What) -> RunOut {
        self.next(label, pins, pre, what).0
    }
}

fn short(s: &str) -> String {
    if s.len() > 600 {
        let mut i = 600;
        while !s.is_char_boundary(i) {
            i -= 1;
        }
        format!("{}...", &s[..i])
    } else {
        s.to_string()
    }
}

/// The captured runs against the Rust machine half's (module docstring).
pub struct Compare<'g> {
    pub replay: Replay,
    pub reader: Reader<'g>,
    /// the first differences (at most `max`)
    pub diffs: Vec<String>,
    /// how many runs differ
    pub differing: usize,
    pub max: usize,
    /// seconds in the Rust machine half
    pub secs: f64,
}

impl<'g> Compare<'g> {
    pub fn new(replay: Replay, reader: Reader<'g>, max: usize) -> Compare<'g> {
        Compare { replay, reader, diffs: Vec::new(), differing: 0, max, secs: 0.0 }
    }
}

/// The counter without its keys at 0 (a key the Python run touched with 0 stays only when it was new to the
/// worker's counter; the reading's totals are the same either way).
fn nonzero(c: &Counter) -> Vec<(&String, &i64)> {
    c.0.iter().filter(|(_, &v)| v != 0).collect()
}

/// The first field where two runs differ, or None.
pub fn run_diff(py: &RunOut, rs: &RunOut) -> Option<String> {
    if py.limit != rs.limit {
        return Some(format!("limit: python {}, rust {}", py.limit, rs.limit));
    }
    if py.value_caps != rs.value_caps {
        return Some(format!("value_caps: python {}, rust {}", py.value_caps, rs.value_caps));
    }
    if py.first_fork != rs.first_fork {
        return Some(format!("first_fork: python {:?}, rust {:?}", py.first_fork, rs.first_fork));
    }
    for (name, a, b) in [
        ("rd.counts", &py.rd.counts, &rs.rd.counts),
        ("rd.forks", &py.rd.forks, &rs.rd.forks),
        ("rd.fork_keys", &py.rd.fork_keys, &rs.rd.fork_keys),
    ] {
        if nonzero(a) != nonzero(b) {
            return Some(format!("{name}: python {:?}, rust {:?}", nonzero(a), nonzero(b)));
        }
    }
    let f = |x: &dyn std::fmt::Debug| short(&format!("{x:?}"));
    if py.paths.len() != rs.paths.len() {
        let ends = |r: &RunOut| r.paths.iter().map(|p| p.end.clone().unwrap_or_default()).collect::<Vec<_>>();
        return Some(format!("paths: python {} {}, rust {} {}", py.paths.len(), f(&ends(py)), rs.paths.len(), f(&ends(rs))));
    }
    for (i, (a, b)) in py.paths.iter().zip(&rs.paths).enumerate() {
        if a == b {
            continue;
        }
        let d = if a.end != b.end || a.sym != b.sym {
            format!("end/sym: python {:?} {}, rust {:?} {}", a.end, a.sym, b.end, b.sym)
        } else if a.pre != b.pre {
            format!("pre: python {}, rust {}", f(&a.pre), f(&b.pre))
        } else if a.writes != b.writes {
            let keys: std::collections::BTreeSet<&String> = a.writes.keys().chain(b.writes.keys()).collect();
            let k = keys.into_iter().find(|k| a.writes.get(*k) != b.writes.get(*k)).unwrap();
            format!("writes[{k}]: python {:?}, rust {:?}", a.writes.get(k), b.writes.get(k))
        } else if a.kept != b.kept {
            format!("kept: python {}, rust {}", f(&a.kept), f(&b.kept))
        } else if a.unknown_writes != b.unknown_writes {
            format!("unknown_writes: python {}, rust {}", f(&a.unknown_writes), f(&b.unknown_writes))
        } else if a.assumptions != b.assumptions {
            let n = a.assumptions.len().min(b.assumptions.len());
            let j = a.assumptions.iter().zip(&b.assumptions).position(|(x, y)| x != y).unwrap_or(n);
            format!(
                "assumptions ({} and {}) [{j}]: python {:?}, rust {:?}",
                a.assumptions.len(),
                b.assumptions.len(),
                a.assumptions.get(j),
                b.assumptions.get(j)
            )
        } else {
            format!("notes: python {}, rust {}", f(&a.notes), f(&b.notes))
        };
        return Some(format!("path {i} of {}: {d}", py.paths.len()));
    }
    None
}

impl Machine for Compare<'_> {
    fn run(&mut self, cx: &Ctx, label: &str, pins: &Pins, pre: &Pre, what: &What) -> RunOut {
        let (py, at) = self.replay.next(label, pins, pre, what);
        let t = std::time::Instant::now();
        let rs = self.reader.run(cx, label, pins, pre, what);
        self.secs += t.elapsed().as_secs_f64();
        if let Some(d) = run_diff(&py, &rs) {
            self.differing += 1;
            if self.diffs.len() < self.max {
                self.diffs.push(format!("{at}: {d}"));
            }
        }
        py
    }
}
