//! The rough reading (experiments/E004-fast-exact/rough-reading.md): the switch step_limit reads every run with the
//! machine's step limit N, so a path past N steps ends there and havoc_cut makes it a havoc record. With the limit at
//! the machine's own (5,000), the records are the same as with the switch off.

use scumm_reader::builder::{Job, Switches};
use scumm_reader::machine::Reader;
use scumm_reader::oracle::{self, Static};
use scumm_reader::record::Record;
use scumm_reader::Builder;
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

fn static_oracle() -> &'static Static {
    static S: OnceLock<Static> = OnceLock::new();
    S.get_or_init(Static::load)
}

/// Room 33's builder with split_limit and havoc_cut, and the step limit `n` (0: the switch off).
fn builder(n: u32) -> Builder<'static> {
    let sw = Switches { split_limit: true, havoc_cut: true, step_limit: n, ..Default::default() };
    let mut b = oracle::builder_with(game(), static_oracle(), "room33", sw);
    b.table.split_limit = true;
    b.table.havoc_cut = true;
    b.table.step_limit = n;
    b
}

/// Every input job of room 33, run one after another by the Rust machine half.
fn read_inputs(n: u32) -> (Builder<'static>, Vec<Vec<Record>>) {
    let mut b = builder(n);
    let jobs: Vec<Job> = b.inputs();
    let mut m = Reader::new(game());
    let recs = jobs.iter().map(|j| b.run_job(&mut m, j)).collect();
    (b, recs)
}

fn texts(recs: &[Vec<Record>]) -> Vec<String> {
    recs.iter().flatten().map(|r| r.py().text()).collect()
}

fn havocs(recs: &[Vec<Record>]) -> usize {
    recs.iter().flatten().filter(|r| r.end.as_deref() == Some("havoc")).count()
}

#[test]
fn the_switch_by_name() {
    assert_eq!(Switches::parse("havoc_cut,step_limit=300").unwrap().step_limit, 300);
    assert_eq!(Switches::parse("havoc_cut").unwrap().step_limit, 0);
    assert!(Switches::parse("step_limit=many").is_err());
    // the names list is fix round 1's and the other boolean switches'; the limit is reported on its own
    assert_eq!(Switches::parse("havoc_cut,step_limit=300").unwrap().names(), vec!["havoc_cut"]);
}

#[test]
fn at_the_machines_own_limit_the_records_are_the_same() {
    let (_, off) = read_inputs(0);
    let (_, own) = read_inputs(scumm_machine::machine::STEP_LIMIT);
    assert_eq!(havocs(&off), 0, "room 33's inputs end within the machine's step limit");
    assert_eq!(texts(&off), texts(&own));
}

#[test]
fn a_small_limit_makes_havoc_records() {
    let (_, off) = read_inputs(0);
    let (b, rough) = read_inputs(20);
    let n = havocs(&rough);
    assert!(n > 0, "paths past 20 steps");
    let made: i64 = b.counts.0.iter().filter(|(k, _)| k.starts_with("havoc_cut: paths the step limit ended")).map(|(_, v)| *v).sum();
    assert_eq!(made as usize, n, "one havoc record per path the limit ended");
    // a job whose runs end within 20 steps keeps its records
    let same = off.iter().zip(&rough).filter(|(a, b)| havocs(&[b.to_vec()]) == 0 && texts(&[a.to_vec()]) == texts(&[b.to_vec()])).count();
    let short = rough.iter().filter(|r| havocs(&[r.to_vec()]) == 0).count();
    assert_eq!(same, short, "the jobs the limit cuts nothing of read the same");
}
