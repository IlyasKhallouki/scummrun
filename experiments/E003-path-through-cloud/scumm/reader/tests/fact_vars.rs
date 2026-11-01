//! The reader switch --fact-vars (table.rs `add_fact_vars`, experiments/E007-fact-refinement/plan.md) on the
//! kitchen's scope: the sentence script's var 103 (written from the sentence's first object, which the engine binds)
//! is a non-fact; listed, it is a fact whose domain holds the start's value and every further start's. A variable that
//! already is a fact, one the engine writes, one the settings fix and one no code of the scope writes are left as they
//! are, and an empty list changes nothing. MI1's numbers appear only here.

use scumm_reader::oracle::{self, Static};
use scumm_reader::table::Kind;
use scumm_reader::Label;
use std::sync::OnceLock;

fn game() -> &'static scumm_ir::Game {
    static G: OnceLock<scumm_ir::Game> = OnceLock::new();
    G.get_or_init(oracle::load_game)
}

#[test]
fn fact_vars_makes_a_non_fact_a_fact() {
    let st = Static::load();
    let mut b = oracle::builder_with(game(), &st, "kitchen", Default::default());
    let scope = b.scope.clone();
    assert_eq!(b.kind("var:103"), Kind::NonFact);
    assert!(b.domain("var:103").is_none());

    // off: no variable listed, nothing changes
    let extra0 = b.table.extra.clone();
    assert!(b.table.add_fact_vars(&scope, &[], &[]).is_empty());
    assert_eq!(b.table.extra, extra0);

    // a variable the kitchen's scope does not write: the first above 103 that no write of the scope reaches
    let unwritten = (104..800).find(|n| !scope.writes.has(&format!("var:{n}"))).expect("an unwritten variable");
    let start103 = b.table.start.get0("var:103");
    let mut later = b.table.start.clone();
    later.insert("var:103".into(), 428);
    let done = b.table.add_fact_vars(&scope, &[103, 102, 2, 39, unwritten], &[&later]);
    let what: Vec<(i32, &str)> = done.iter().map(|d| (d.n, d.what)).collect();
    assert_eq!(
        what,
        vec![
            (103, "added"),
            (102, "already a fact"),
            (2, "written by the engine"),
            (39, "fixed by the settings"),
            (unwritten, "the scope does not write it (a constant at its start value)"),
        ]
    );
    let mut seeds = vec![start103, 428];
    seeds.sort_unstable();
    seeds.dedup();
    assert_eq!(done[0].seeds, seeds);
    assert_eq!(b.kind("var:103"), Kind::Fact);
    let dom = b.domain("var:103").expect("a domain");
    assert_eq!(dom.values, seeds.iter().map(|&v| Label::Int(v)).collect::<Vec<_>>());
    assert!(dom.low.is_none() && dom.high.is_none(), "an exact domain: no value collapses");
    // the others keep their kinds
    assert_eq!(b.kind("var:102"), Kind::Fact);
    assert_eq!(b.kind("var:2"), Kind::NonFact);
    assert_eq!(b.kind("var:39"), Kind::Const);
    assert_eq!(b.kind(&format!("var:{unwritten}")), Kind::Const);
}
