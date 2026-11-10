//! E026 C1 (experiments/E026-part1/plan.md): the switch wait_queued, wait_both's passing path only on a path that
//! holds a sentence. On room 33's scope with F1c and E022's queue (FMI_READ_QUEUE=1, as ALLON: a doSentence holds its
//! sentence): every record of the reading without either switch is still there, every record is one wait_both's
//! reading has too (Lemma W: a subset of the passing paths), and a WaitFor op on a path with no sentence only yields.
//! Room 33's held sentences (96) meet no WaitFor op, so here the reading is the one without either switch; the demo's
//! room 48, where one does, is G3's. One test in this file: the queue's switch is read once a process.

use scumm_reader::builder::{Builder, Switches};
use scumm_reader::oracle::{self, Static};
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

fn read(extra: &str) -> Builder<'static> {
    let sw = Switches::parse(&format!("value_sets,draw_clears,actor_rooms,split_limit,input_settled,{extra}")).unwrap();
    let mut b = oracle::builder_with(game(), statics(), "room33", sw);
    b.table.value_sets = true;
    let mut m = scumm_reader::machine::Reader::new(game());
    b.read_all_into(&mut m, true, None, 1, None);
    b
}

fn lines(b: &Builder) -> BTreeSet<String> {
    b.records.iter().map(|r| r.py().text()).collect()
}

#[test]
fn between_both_off_and_wait_both() {
    std::env::set_var("FMI_READ_QUEUE", "1");
    let (off, queued, both) = (read(""), read("wait_queued"), read("wait_both"));
    let (a, q, w) = (lines(&off), lines(&queued), lines(&both));
    let lost: Vec<&String> = a.difference(&q).take(3).collect();
    assert!(lost.is_empty(), "records wait_queued lost: {lost:?}");
    let extra: Vec<&String> = q.difference(&w).take(3).collect();
    assert!(extra.is_empty(), "records wait_both has not: {extra:?}");
    assert!(w.len() > q.len(), "wait_both adds passing paths where no sentence is held");
    let c = &queued.rd.counts;
    assert!(c.get("wait_queued: WaitFor ops that only yield (the path holds no sentence)") > 0);
    assert!(c.get("FMI_READ_QUEUE: sentences a doSentence holds") > 0);
    assert_eq!(c.get("wait_queued: WaitFor ops that also pass (the path holds a sentence)"), 0);
    assert_eq!(a, q, "no held sentence meets a wait: the reading without either switch");
    assert_eq!(off.rd.counts.get("wait_queued: WaitFor ops that only yield (the path holds no sentence)"), 0);
    assert_eq!(both.rd.counts.get("wait_queued: WaitFor ops that also pass (the path holds a sentence)"), 0);
    eprintln!(
        "records: off {}, wait_queued {}, wait_both {}; wait_queued's ops that only yield {}; sentences held {}",
        a.len(),
        q.len(),
        w.len(),
        c.get("wait_queued: WaitFor ops that only yield (the path holds no sentence)"),
        c.get("FMI_READ_QUEUE: sentences a doSentence holds")
    );
}
