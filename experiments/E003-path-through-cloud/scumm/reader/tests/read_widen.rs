//! E014's switch FMI_READ_WIDEN=1 (builder.rs `read_widen_switch`; experiments/E014-fate-demo/plan.md, amendment
//! (6)): under split_limit, a run past the path limit is run again once with its loops widened (`Machine::set_widen`);
//! ended under the limit, that run is the run and nothing is split; still past it, the first run is split as before;
//! a run under the limit is never run again. Its own test binary: the switch is read once per process, set here
//! before the first run. The fake machine reads MI1's room 33 as split_limit.rs's does (test data only).

use scumm_reader::builder::{Job, Machine, RdCounts, RunOut, Switches, FORKS_ON_NONFACTS, FORKS_ON_READ};
use scumm_reader::domain::Label;
use scumm_reader::oracle::{self, Static};
use scumm_reader::record::{PathOut, Pins, Pre, Val, What};
use scumm_reader::table::Kind;
use scumm_reader::{Builder, Ctx};
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn static_oracle() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

const WHOLE: &str = "FMI_READ_WIDEN: runs past the path limit read whole with their loops widened";
const STILL: &str = "FMI_READ_WIDEN: runs past the path limit still past it with their loops widened (split as before)";
const SPLIT: &str = "runs past the path limit split on their first fork on a read";

/// A machine whose unwidened run passes the path limit while `key` is unpinned, its first fork `key`; widened (when it
/// `fits`), or with `key` pinned, it ends one path that writes `target`.
struct Fake {
    key: String,
    target: (String, i64),
    widen: bool,
    fits: bool,
    /// (widened, pinned) of each run
    runs: Vec<(bool, bool)>,
}

impl Machine for Fake {
    fn run(&mut self, _cx: &Ctx, _label: &str, pins: &Pins, pre: &Pre, _what: &What) -> RunOut {
        let mut rd = RdCounts::default();
        rd.counts.add(FORKS_ON_READ, 1);
        rd.counts.add(FORKS_ON_NONFACTS, 10);
        let pinned = pins.contains_key(&self.key);
        self.runs.push((self.widen, pinned));
        let done = pinned || (self.widen && self.fits);
        if !done {
            let first_fork = (!pinned).then(|| self.key.clone());
            let cut = PathOut { end: Some("limit".into()), sym: true, pre: pre.clone(), ..Default::default() };
            return RunOut { paths: vec![cut], limit: 3, first_fork, rd, ..Default::default() };
        }
        let writes = BTreeMap::from([(self.target.0.clone(), Val::Int(self.target.1))]);
        let path = PathOut { end: Some("end".into()), sym: true, pre: pre.clone(), writes, ..Default::default() };
        RunOut { paths: vec![path], rd, ..Default::default() }
    }

    fn set_widen(&mut self, on: bool) {
        self.widen = on;
    }
}

fn setup(fits: bool) -> (Builder<'static>, Fake) {
    std::env::set_var("FMI_READ_WIDEN", "1");
    let mut b = oracle::builder_with(game(), static_oracle(), "room33", Switches { split_limit: true, ..Default::default() });
    b.table.split_limit = true;
    let mut keys: Vec<String> = b
        .table
        .doms
        .doms
        .keys()
        .filter(|k| k.starts_with("var:") || k.starts_with("state:"))
        .filter(|k| b.kind(k) == Kind::Fact && b.domain(k).is_some_and(|d| d.values.len() == 2))
        .cloned()
        .collect();
    keys.sort();
    let target = keys.pop().unwrap();
    let value = b.domain(&target).unwrap().values.iter().find_map(|l| if let Label::Int(n) = l { Some(*n) } else { None }).unwrap();
    let key = keys.pop().unwrap();
    (b, Fake { key, target: (target, value), widen: false, fits, runs: Vec::new() })
}

fn job() -> Job {
    Job { kind: "sentence".into(), label: "fake".into(), pins: Pins::new(), pre: Pre::new(), what: What::Verb(1), checks: vec![] }
}

#[test]
fn a_cut_run_that_fits_widened_is_not_split() {
    let (mut b, mut m) = setup(true);
    let recs = b.run_job(&mut m, &job());
    assert_eq!(m.runs, vec![(false, false), (true, false)], "the run, then once widened");
    assert_eq!(recs.len(), 1);
    assert!(recs[0].pre.get(&m.key).is_none(), "no pin: the run was not split");
    assert_eq!(b.counts.get(WHOLE), 1);
    assert_eq!(b.counts.get(SPLIT), 0);
    assert!(!m.widen, "widening is off again after the run");
}

#[test]
fn a_run_still_cut_widened_is_split_as_before() {
    let (mut b, mut m) = setup(false);
    let recs = b.run_job(&mut m, &job());
    // the run and its widened run, then one part per value, each under the limit
    assert_eq!(m.runs, vec![(false, false), (true, false), (false, true), (false, true)]);
    assert_eq!(recs.len(), 2);
    assert_eq!(b.counts.get(STILL), 1);
    assert_eq!(b.counts.get(SPLIT), 1);
    assert_eq!(b.counts.get(WHOLE), 0);
}

#[test]
fn a_run_under_the_limit_is_not_run_again() {
    let (mut b, mut m) = setup(true);
    let mut pins = Pins::new();
    let mut pre = Pre::new();
    let v = b.domain(&m.key).unwrap().values[0];
    b.pin(&mut pins, &mut pre, &m.key.clone(), &[v]);
    let recs = b.run_job(&mut m, &Job { pins, pre, ..job() });
    assert_eq!(m.runs, vec![(false, true)]);
    assert_eq!(recs.len(), 1);
    assert_eq!(b.counts.get(WHOLE) + b.counts.get(STILL), 0);
}
