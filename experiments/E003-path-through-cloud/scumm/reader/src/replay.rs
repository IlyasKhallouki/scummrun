//! A [`Machine`] that plays back the machine runs reader_port_oracle.py captured from the Python reading
//! (`capture-NAME.jsonl.gz`, in the order the parent process handled the jobs). Each run checks that the Builder
//! asked for the run Python made (label, pins, precondition, what): a split, a pin or an event the port gets wrong
//! stops the replay there.

use crate::builder::{Ctx, Machine, RdCounts, RunOut};
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
        RunOut {
            paths: p.get("paths").items().iter().map(PathOut::from_py).collect(),
            limit: p.get("limit").int(),
            value_caps: p.get("value_caps").int(),
            first_fork: (!p.get("first_fork").is_none()).then(|| p.get("first_fork").as_str().to_string()),
            rd: RdCounts {
                counts: Counter::from_py(rd.get("counts")),
                forks: Counter::from_py(rd.get("forks")),
                fork_keys: Counter::from_py(rd.get("fork_keys")),
            },
        }
    }
}

fn short(s: &str) -> String {
    if s.len() > 600 {
        format!("{}...", &s[..600])
    } else {
        s.to_string()
    }
}
