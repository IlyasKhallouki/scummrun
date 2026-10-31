//! The switch grow_limit: a run past the path limit with no fact left to split on is read again whole with the limit
//! 4 times larger, up to GROW_MAX, and the reading stops only past that. Off, split_limit's rule stops it at once.
//! The limit only ends paths, so a run it cuts nothing of is the whole run.

use scumm_reader::builder::{Job, Machine, RdCounts, RunOut, Switches, GROW_MAX};
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

/// A machine whose run has `paths` paths and no fork on a read: past the limit it hands over one cut path.
struct Fake {
    paths: usize,
    limit: usize,
    target: (String, i64),
    limits_seen: Vec<usize>,
}

impl Machine for Fake {
    fn run(&mut self, _cx: &Ctx, _label: &str, _pins: &Pins, pre: &Pre, _what: &What) -> RunOut {
        let limit = if self.limit == 0 { 1024 } else { self.limit };
        self.limits_seen.push(limit);
        if self.paths > limit {
            let cut = PathOut { end: Some("limit".into()), sym: true, pre: pre.clone(), ..Default::default() };
            return RunOut { paths: vec![cut], limit: (self.paths - limit) as i64, first_fork: None, rd: RdCounts::default(), ..Default::default() };
        }
        let writes = BTreeMap::from([(self.target.0.clone(), Val::Int(self.target.1))]);
        RunOut { paths: vec![PathOut { end: Some("end".into()), sym: true, pre: pre.clone(), writes, ..Default::default() }], ..Default::default() }
    }
    fn set_path_limit(&mut self, n: usize) {
        self.limit = n;
    }
}

fn setup(grow: bool, paths: usize) -> (Builder<'static>, Fake) {
    let sw = Switches { split_limit: true, grow_limit: grow, ..Default::default() };
    let mut b = oracle::builder_with(game(), static_oracle(), "room33", sw);
    b.table.split_limit = true;
    b.table.grow_limit = grow;
    let mut keys: Vec<String> = b
        .table
        .doms
        .doms
        .keys()
        .filter(|k| k.starts_with("var:") && b.kind(k) == Kind::Fact && b.domain(k).is_some_and(|d| d.values.len() >= 2))
        .cloned()
        .collect();
    keys.sort();
    let target = keys.pop().unwrap();
    let value = b.domain(&target).unwrap().values.iter().find_map(|l| if let Label::Int(n) = l { Some(*n) } else { None }).unwrap();
    (b, Fake { paths, limit: 0, target: (target, value), limits_seen: Vec::new() })
}

fn job() -> Job {
    Job { kind: "sentence".into(), label: "fake".into(), pins: Pins::new(), pre: Pre::new(), what: What::Verb(1), checks: vec![] }
}

#[test]
fn grow_limit_reads_the_run_whole() {
    let (mut b, mut m) = setup(true, 5000);
    let recs = b.run_job(&mut m, &job());
    assert_eq!(recs.len(), 1, "the whole run's one path");
    assert_eq!(m.limits_seen, vec![1024, 4096, 16384]);
    assert_eq!(b.counts.get("grow_limit: runs read again with the path limit at 4096"), 1);
    assert_eq!(b.counts.get("grow_limit: runs read again with the path limit at 16384"), 1);
    assert_eq!(b.counts.get("paths ended by the path limit"), 0);
    assert_eq!(b.path_limit_now, 0, "the machine's own limit after");
    assert_eq!(m.limit, 0);
}

#[test]
#[should_panic(expected = "fake passed the path limit (1 paths cut) with no fact left to split on")]
fn grow_limit_off_stops() {
    let (mut b, mut m) = setup(false, 1025);
    b.run_job(&mut m, &job());
}

#[test]
#[should_panic(expected = "with no fact left to split on")]
fn grow_limit_stops_past_the_largest_limit() {
    let (mut b, mut m) = setup(true, GROW_MAX + 1);
    b.run_job(&mut m, &job());
}
