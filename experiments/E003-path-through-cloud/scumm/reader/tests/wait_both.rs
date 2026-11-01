//! The switch wait_both (experiments/E004-fast-exact/wait-both.md, Lemma W) on room 33's scope, read with the Rust
//! machine in one pass (so both readings have the same domains): off, no record passes a WaitFor op except the one its
//! event resumes at; on, every record of the reading without the switch is still there, and the records that pass a
//! wait in the same run are added, each with the check "blocks" on the side false at that wait.

use scumm_reader::builder::{Builder, Switches};
use scumm_reader::oracle::{self, Static};
use scumm_reader::py::Py;
use scumm_reader::record::{Record, What};
use std::collections::BTreeSet;
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn statics() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

fn read(on: bool) -> Builder<'static> {
    let sw = Switches { wait_both: on, ..Default::default() };
    let mut b = oracle::builder_with(game(), statics(), "room33", sw);
    let mut m = scumm_reader::machine::Reader::new(game());
    b.read_all_into(&mut m, true, None, 1, None);
    b
}

/// The checks "<WaitFor op> blocks" on the side false that are not at the wait the record's event resumes at.
fn passed(r: &Record) -> Vec<String> {
    let resume = match &r.source {
        What::Event(e) => Some(e.w),
        _ => None,
    };
    r.checks
        .iter()
        .filter(|c| c.cond.ends_with(" blocks") && c.side == Py::Bool(false))
        .filter(|c| {
            let off = c.cite.rsplit_once('[').and_then(|(_, o)| i64::from_str_radix(o.trim_end_matches(']'), 16).ok());
            off != resume
        })
        .map(|c| c.cite.clone())
        .collect()
}

#[test]
fn off_no_record_passes_a_wait_it_does_not_resume_at() {
    let b = read(false);
    assert!(!b.records.is_empty());
    assert!(b.records.iter().all(|r| passed(r).is_empty()));
}

#[test]
fn on_adds_the_runs_that_pass_a_wait_and_keeps_every_record() {
    let off = read(false);
    let on = read(true);
    let lines = |b: &Builder| b.records.iter().map(|r| r.py().text()).collect::<BTreeSet<String>>();
    let (a, b) = (lines(&off), lines(&on));
    let lost: Vec<&String> = a.difference(&b).take(3).collect();
    assert!(lost.is_empty(), "records the switch lost: {lost:?}");
    let new: Vec<&Record> = on.records.iter().filter(|r| !passed(r).is_empty()).collect();
    assert!(!new.is_empty());
    assert!(b.len() > a.len());
    // the sentence script's walks: a sentence that passes the wait after its walk in the same run
    assert!(new.iter().any(|r| r.kind != "event" && passed(r).iter().any(|c| c.starts_with("global/script-002.txt"))));
}
